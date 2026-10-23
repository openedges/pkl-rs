"""Pinned Java renderer fixture export and independent raw/provenance verification."""
from pathlib import Path
import argparse
import hashlib
import json
import os
import shutil
import stat

BASE = Path('/Users/cody.ij.hwang/Projects/pkl-rs.stage47/target/m2')
PINS = {
 2: ('cd60e8293425b61c50068f359b8ddc0024d85f7230851283b025a140cfc43eb6', '8c67ae9d42367a37d795f4d9538bfcc14f04f7d70c3c7f34995220c40c365a46', '3728f827e09e9a697a0f8685fefaafd9f509586e9d911d7e247b5cec8a347182'),
 3: ('5675a4b9edb61b5df8be36b66ad2ca5b84c33b0f2fb2a54f3aa5b5c89ca12e07', 'd0ce9c5fdb8b15819c2279e348069c0823603a5bd623cd9b0954785129100a94', 'cab807771dc9b8c2838adfd3dc0d5839711c82d19c86f0f2310c58ff7c5485e1'),
 4: ('59c2dfaddf8e9750317b21c39d02f67872bb66a547f627587fa5b86de73a8683', '343bf6eb65203dd527082bb6e072f6c25f9c8bb61ca23fcbb327861d279cb0aa', 'c7faf1a7c5bc6e38a5d0d52dacf8ff3e06a19adda328191c1865f4617ec8de31'),
 5: ('52219434922083dea069dd0808565d8e342127a92658303f5001429af8280ddb', 'e134d13b8d1e3a70efec1cd258c6855f947fae3c321463111c78add1a4fa05b4', 'b7fdc81e51d92669f8c30fcc56716c10348761a58f98d4339c34cefbdf96e623'),
 6: ('cba85cf833baeb7655f6a6cdc81fd9ee39fe0ee43f441e43d891a47c0089ebf8', 'b9f1243b213906d010998f87807388bee1dc2607ffb447f937a6fb9a3eb4cbc5', '8a10197b44469e40af433399d3caa2833a75be9fac957a262047ff33979ed2db'),
}
SELECT = {
 2: ['scalar-boxed-identity-minimal', 'json-scalars-and-errors', 'options-getters-entry-force-retry', 'json-utf16-escaper'],
 3: ['document-value-hints-identity', 'directives-text-lazy-bytes-undefined'],
 4: ['primitive-polymorphic-function-history'],
 5: ['annotation-parent-order-and-omission', 'reference-join-callback'],
 6: ['property-path-class-null-transformer', 'collection-join-force-order'],
}
LIMITS = [
 {'root': 'stage47-renderer-api-oracle-01', 'apiCalls': 648, 'kind': 'unexecuted-javac-failure'},
 {'root': 'stage47-renderer-api-oracle-02', 'apiCalls': 72, 'kind': 'authored-parser-rejection-not-renderer'},
 {'root': 'stage47-renderer-api-oracle-02', 'apiCalls': 16, 'kind': 'nested-query-lexical-rejection-not-directive-path'},
 {'root': 'stage47-renderer-api-oracle-02', 'apiCalls': 40, 'kind': 'other-directive-calls-superseded-selection-only-original-retained'},
 {'root': 'stage47-renderer-api-oracle-03', 'apiCalls': 44, 'kind': 'Any-converter-does-not-dispatch-primitives'},
 {'root': 'stage47-renderer-api-oracle-05', 'apiCalls': 120, 'kind': 'reserved-identifier-parser-rejections-not-renderer'},
 {'root': 'stage47-renderer-api-oracle-05', 'apiCalls': 48, 'kind': 'query-import-alias-rejections-not-policy'},
 {'root': 'stage47-renderer-api-oracle-05', 'apiCalls': 36, 'kind': 'policy-stable-sentinels-original-retained'},
 {'root': 'stage47-renderer-api-oracle-06', 'apiCalls': 84, 'kind': 'actual-policy-profiles-acquired-not-selected-for-default-policy-portable'},
 {'kind': 'separate-followup-required', 'scope': 'Policy profiles portable/native, Bytes wrapper/resource identity, captured primitive frames, close/reentry, complete renderer inventory; original whole CLI is independently acquired; no complete renderer or M2 claim'},
 {'kind': 'gc-observation-boundary', 'scope': 'Java System.gc requests do not establish observed collection; portable runner collection policy recorded independently'},
]

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
      elif kind == 'missing':
       check(authored_kind == 'missing', 'missing helper rule')
       result = {'kind': 'missing'}
      else:
       raise ValueError('unimplemented reader return')
      reader_plan.append({'uri': uri, 'result': result})
     fixtures.append({'id': f'{root_name(group)}/{name}', 'repeat': repeat, 'session': session, 'source': source, 'uri': case['sourceUri'], 'moduleName': 'main', 'expressions': expressions, 'readerPlan': reader_plan, 'expected': expected})
 check(sum(len(f['expected']) for f in fixtures) == 860 and len(fixtures) == 44, 'fixed selected coverage')
 bundle = {'schema': 1, 'fixtures': fixtures, 'javaApiCalls': 860, 'unexecutedApiBoundaries': LIMITS}
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
 expected_files = required | {'manifest.json', 'fixtures.json'}
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
