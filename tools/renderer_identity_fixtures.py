"""Pinned Java renderer fixture export and independent raw/provenance verification."""
from pathlib import Path
import argparse
import hashlib
import json
import os
import shutil
import stat

BASE = Path('/Users/cody.ij.hwang/Projects/pkl-rs.stage47/target/m2')
PINS = {10: ('20a00c5fb207d8fe80308b849103a6a4bc2a153f1459201a749c62466f84d425', 'bc7a79974a4fd727a1e395221c0e7f91b2a9083d4a0d79083fe10427a8729084', '0dac930649a021e43b2ecb382a731bf3ab293a2747225298a9b65e20a533940f'), 11: ('dd1f3cbbfc4730cdac90bc6fb6c494fa1596d88d3fdb6080d5602d12b676c8ba', '2058b500c6421eedd9e343907c79473b9ccbc91e9b111ff765b4a375a90bfdd7', 'e78663a96efeaca77464b757dad841e829018ff7f376c3a867aed1ee10bbe62e')}
SELECT = {10: ['generator-ordinary-restored-old-frame', 'generator-typed-union-restored-old-frame'], 11: ['tagged-instances-numeric-constant-identity']}
LIMITS = [{'kind': 'instrumentation', 'scope': 'One actual Java powerAssertions=true baseline. Rust Observe off/on are two native consumers of identical raw Java expectations, never a Java off/on acquisition.'}, {'kind': 'bounded-identity', 'scope': 'Three authored cases,12 evaluator sessions240 API calls. No allslot/generator/type/valueidentity or fullM2 claim.'}, {'kind': 'lifecycle', 'scope': 'Explicit Java GC requests do not establish collected-object trace equality. policy84/close28 excluded; earlier1720 andwrongmethod16 raw evidence unchanged.'}]

def sha(data):
 return hashlib.sha256(data).hexdigest()

def check(condition, message):
 if not condition:
  raise ValueError(message)

def read_json(path):
 return json.loads(path.read_bytes())

def fixture_reader(root):
 check(stat.S_ISDIR(root.lstat().st_mode), 'fixture root must be a regular directory')
 def read(relative):
  parts = safe_relative(relative).parts
  path = root
  for part in parts[:-1]:
   path = path / part
   check(stat.S_ISDIR(path.lstat().st_mode), 'fixture parent must be a regular directory')
  path = path / parts[-1]
  check(stat.S_ISREG(path.lstat().st_mode), 'fixture payload must be a regular file')
  return path.read_bytes()
 return read

def safe_relative(text):
 p = Path(text)
 check(not p.is_absolute() and '..' not in p.parts and str(p) == text, 'unsafe relative path')
 return p

def root_name(group):
 return f'stage47-renderer-api-oracle-{group:02}'

def prefix(group):
 return f'provenance/{root_name(group)}'

def facts(group, read):
 p = prefix(group)
 docs = [read(f'{p}/{name}.json') for name in ['report', 'inventory', 'closure']]
 check(tuple(map(sha, docs)) == PINS[group], f'fixed provenance pin mismatch group {group}')
 report, inventory, closure = map(json.loads, docs)
 check(closure['closed'] is True and closure['allPayloadHashesVerified'] is True and closure['liveHandles'] == [], 'not closed')
 check(closure['reportSha256'] == PINS[group][0] and closure['inventorySha256'] == PINS[group][1], 'closure identities')
 rows = {}
 for row in inventory['files']:
  path = str(safe_relative(row['path']))
  check(path not in rows, 'duplicate inventory path')
  rows[path] = row
 check(len(rows) == closure['files'], 'inventory closure count')
 check(report['inputChanges'] == [] and report['workspacesUnchanged'] and report['jarStable'], 'producer instability')
 check(all(not row['rawPathDifferences'] for row in report['repeatDifferences']), 'producer repeat mismatch')
 return report, rows

def counters(text):
 pairs = [line.split('=', 1) for line in text.splitlines()]
 check(all(len(pair) == 2 for pair in pairs), 'counter syntax')
 result = {key: int(value) for key, value in pairs}
 check(len(result) == len(pairs), 'duplicate counters')
 return result

def utf16(data):
 check(len(data) % 2 == 0, 'odd UTF16BE')
 return [int.from_bytes(data[i:i+2], 'big') for i in range(0, len(data), 2)]

def java_utf8(data):
 return data.decode('utf-16-be', 'surrogatepass').encode('utf-8', 'replace')

def construct(read):
 fixtures, proofs, required = [], {}, set()
 for group, chosen in SELECT.items():
  report, rows = facts(group, read)
  gp = prefix(group)
  required.update(f'{gp}/{name}.json' for name in ['report', 'inventory', 'closure'])
  def payload(rel):
   check(rel in rows, f'not in pinned inventory: {rel}')
   target = f'{gp}/{rel}'
   data = read(target)
   row = rows[rel]
   check(len(data) == row['bytes'] and sha(data) == row['sha256'], f'inventory payload mismatch: {target}')
   proofs[target] = {'originalRoot': str(BASE / root_name(group)), 'originalRelativePath': rel, 'sha256': sha(data), 'bytes': len(data)}
   required.add(target)
   return data
  for rel in ['RendererProbe.java', 'run.py', 'profile.json', 'inputs-before.json', 'inputs-after.json']:
   data = payload(rel)
   if rel in ['RendererProbe.java', 'run.py', 'profile.json']:
    check(report['inputHashes'][str(BASE / root_name(group) / rel)] == sha(data), 'producer source/profile not report-bound')
  cases = json.loads(payload('cases.json'))
  case_map = {case['id']: case for case in cases}
  for name in chosen:
   case = case_map[name]
   workspace_rel = f'workspace/{name}'
   workspace = {path: payload(path) for path in rows if path.startswith(workspace_rel + '/')}
   source_path = workspace_rel + '/main.pkl'
   source_bytes = workspace[source_path]
   check(report['inputHashes'][str(BASE / root_name(group) / source_path)] == sha(source_bytes), 'source not report-bound')
   source = source_bytes.decode('utf-8')
   plan_rows = {}
   for line in workspace[workspace_rel + '/reader.tsv'].decode().splitlines():
    uri, kind, file = line.split('\t')
    check(uri not in plan_rows, 'duplicate input reader URI')
    plan_rows[uri] = (kind, file)
   expressions = [op[1] for op in case['operations'] if op != 'GC']
   check(all(op == 'GC' or op[0] == 'main.pkl' for op in case['operations']), 'unsupported multi-root fixture')
   for observation in report['observations']:
    if observation['case'] != name:
     continue
    repeat = observation['repeat']
    for session in [0, 1]:
     calls = [call for call in observation['calls'] if call['session'] == session]
     check([call['expression'] for call in calls] == expressions, 'API expression sequence')
     raw_dir = f'cases/{name}/{repeat}/calls/session-{session}'
     for path in rows:
      if path.startswith(raw_dir + '/'):
       payload(path)
     expected = []
     for index, call in enumerate(calls):
      raw = f'{raw_dir}/outer-{index}'
      check(call['rawPrefix'] == raw and call['prefix'] == f'outer-{index}', 'raw prefix binding')
      check(payload(raw + '.expression.bin') == expressions[index].encode(), 'raw expression')
      status_bytes = payload(raw + '.status.bin')
      check(status_bytes in [b'success\n', b'error\n'], 'nonordinary API boundary')
      check(status_bytes.decode() == call['status'], 'report/raw status')
      values = {}
      for channel in ['stdout', 'stderr']:
       data = payload(raw + f'.{channel}.bin')
       units = payload(raw + f'.{channel}.code-units.bin')
       check(java_utf8(units) == data, 'Java UTF8 independent conversion')
       values[channel + 'Units'] = utf16(units)
       values[channel + 'Hex'] = data.hex()
       if channel == ('stdout' if status_bytes == b'success\n' else 'stderr'):
        check(call['utf8Hex'] == data.hex() and call['utf16BEHex'] == units.hex() and call['utf8Sha256'] == sha(data) and call['utf16BESha256'] == sha(units), 'report/raw complete channel')
      for side in ['Before', 'After']:
       raw_counters = payload(raw + f'.counters-{side.lower()}.bin').decode()
       check(raw_counters == call['counters' + side], 'report/raw full counter text')
       count = counters(call['counters' + side])['read']
       check(payload(raw + f'.reads-{side.lower()}.bin') == str(count).encode(), 'report-bound read counter')
       values['reads' + side] = count
      expected.append({'status': status_bytes.decode()[:-1], **values})
     reader_plan = []
     for index in range(expected[-1]['readsAfter']):
      raw = f'{raw_dir}/read-{index}'
      uri = payload(raw + '.uri.bin').decode()
      kind = payload(raw + '.kind.bin').decode()
      attempt = int(payload(raw + '.attempt.bin'))
      check(uri in plan_rows, 'unknown original Java reader URI')
      authored_kind, file = plan_rows[uri]
      if kind == 'io':
       check(authored_kind == 'io-first' and attempt == 1, 'IO return provenance')
       message = payload(raw + '.message.bin').decode()
       check(message == 'renderer-reader-first', 'IO helper rule')
       result = {'kind': 'io', 'message': message}
      elif kind == 'text':
       check(authored_kind == 'text' or (authored_kind == 'io-first' and attempt > 1), 'text helper rule')
       data = payload(raw + '.value.code-units.bin')
       check(data == workspace[workspace_rel + '/' + file] and java_utf8(data) == payload(raw + '.value.bin'), 'actual Java reader return units and bytes')
       result = {'kind': 'text', 'units': utf16(data)}
      elif kind == 'resource':
       check(authored_kind == 'shared-resource', 'resource helper rule')
       resource_uri = payload(raw + '.resource-uri.bin').decode()
       resource_bytes = payload(raw + '.bytes.bin')
       check(resource_uri == uri and resource_bytes == bytes([1,2,3]), 'actual shared Resource bytes and URI')
       helper = payload('RendererProbe.java')
       check(b'static final byte[] SHARED_BYTES = new byte[]{1,2,3};' in helper and b'new org.pkl.core.resource.Resource(uri,SHARED_BYTES)' in helper, 'shared backing input helper source')
       result = {'kind': 'resource', 'resourceUri': resource_uri, 'bytes': list(resource_bytes)}
      elif kind == 'missing':
       check(authored_kind == 'missing', 'missing helper rule')
       result = {'kind': 'missing'}
      else:
       raise ValueError('unimplemented reader return')
      reader_plan.append({'uri': uri, 'result': result})
     fixtures.append({'id': f'{root_name(group)}/{name}', 'repeat': repeat, 'session': session, 'source': source, 'uri': case['sourceUri'], 'moduleName': 'main', 'expressions': expressions, 'readerPlan': reader_plan, 'expected': expected, 'readerBackingGroups': [{'id':'java-shared-static-byte-array','uris':['res:/shared-A','res:/shared-B'],'scope':'one backing allocation per independent native session; wrapper identities remain engine-owned'}] if name == 'shared-resource-backing-wrapper-identity' else []})
 check(sum(len(f['expected']) for f in fixtures) == 240 and len(fixtures) == 12, 'fixed selected coverage')
 bundle = {'schema': 1, 'fixtures': fixtures, 'javaApiCalls': 240, 'unexecutedApiBoundaries': LIMITS}
 return bundle, proofs, required

def file_set(root, expected_files=None):
 allowed_directories = None
 if expected_files is not None:
  allowed_directories = {str(parent) for relative in expected_files for parent in safe_relative(relative).parents}
 check(root.is_dir() and not root.is_symlink(), 'fixture root must be a regular directory')
 found = set()
 for current, directories, files in os.walk(root, followlinks=False):
  for name in directories + files:
   check(not (Path(current) / name).is_symlink(), 'symlink forbidden in fixture')
  if allowed_directories is not None:
   for name in directories:
    relative = str((Path(current) / name).relative_to(root))
    check(relative in allowed_directories, 'undeclared fixture directory')
  for name in files:
   path = Path(current) / name
   check(path.is_file(), 'non-regular fixture payload')
   found.add(str(path.relative_to(root)))
 return found

def tsv_data(bundle):
 cases, expected = {}, []
 for fixture in bundle['fixtures']:
  group, name = fixture['id'].split('/')
  cases[fixture['id']] = [fixture['id'], f"provenance/{group}/workspace/{name}/main.pkl", fixture['uri'], fixture['moduleName']]
  for index, row in enumerate(fixture['expected']):
   fields = [fixture['id'], str(fixture['repeat']), str(fixture['session']), str(index), fixture['expressions'][index].encode('utf-8').hex(), row['status'], ''.join(f'{u:04x}' for u in row['stdoutUnits']), ''.join(f'{u:04x}' for u in row['stderrUnits']), row['stdoutHex'], row['stderrHex'], str(row['readsBefore']), str(row['readsAfter'])]
   check(all('\t' not in field and '\n' not in field for field in fields), 'TSV field syntax')
   expected.append('\t'.join(fields))
 return {'cases.tsv': ('\n'.join('\t'.join(fields) for fields in cases.values()) + '\n').encode(), 'expected.tsv': ('\n'.join(expected) + '\n').encode()}

def export(output):
 check(not output.exists(), 'export requires fresh directory')
 def original(relative):
  group_name, rel = relative.removeprefix('provenance/').split('/', 1)
  return (BASE / group_name / safe_relative(rel)).read_bytes()
 bundle, proofs, required = construct(original)
 output.mkdir(parents=True)
 for relative in required:
  path = output / relative
  path.parent.mkdir(parents=True, exist_ok=True)
  path.write_bytes(original(relative))
 (output / 'fixtures.json').write_text(json.dumps(bundle, indent=2, ensure_ascii=True) + '\n')
 for name, data in tsv_data(bundle).items():
  (output / name).write_bytes(data)
 manifest = {'schema': 1, 'sourceProvenance': proofs, 'fixedPins': {str(k): list(v) for k, v in PINS.items()}, 'files': {}}
 for relative in sorted(file_set(output)):
  data = (output / relative).read_bytes()
  manifest['files'][relative] = {'sha256': sha(data), 'bytes': len(data)}
 (output / 'manifest.json').write_text(json.dumps(manifest, indent=2) + '\n')
 return verify(output)

def verify(root):
 read = fixture_reader(root)
 manifest = json.loads(read('manifest.json'))
 bundle, proofs, required = construct(read)
 expected_files = required | {'manifest.json', 'fixtures.json', 'cases.tsv', 'expected.tsv'}
 for name, data in tsv_data(bundle).items():
  check(read(name) == data, 'TSV differs from pinned Java raw: ' + name)
 actual = file_set(root, expected_files=expected_files)
 check(actual == set(manifest['files']) | {'manifest.json'}, 'manifest actual regular-file coverage')
 for relative, row in manifest['files'].items():
  data = read(relative)
  check(row == {'sha256': sha(data), 'bytes': len(data)}, 'outer manifest payload mismatch')
 check(json.loads(read('fixtures.json')) == bundle, 'fixture differs from pinned Java source/raw/counters')
 check(manifest['sourceProvenance'] == proofs and manifest['fixedPins'] == {str(k): list(v) for k, v in PINS.items()}, 'provenance mapping differs')
 check(actual == expected_files, 'fixed payload coverage')
 return {'schema': 1, 'fixtures': len(bundle['fixtures']), 'javaApiCalls': bundle['javaApiCalls'], 'payloadFiles': len(actual) - 1, 'manifestSha256': sha(read('manifest.json')), 'allPinnedRawAndCounterBindingsVerified': True}

def main():
 parser = argparse.ArgumentParser()
 choice = parser.add_mutually_exclusive_group(required=True)
 choice.add_argument('--fixtures', type=Path)
 choice.add_argument('--output', type=Path)
 args = parser.parse_args()
 print(json.dumps(export(args.output) if args.output else verify(args.fixtures), indent=2))

if __name__ == '__main__':
 main()
