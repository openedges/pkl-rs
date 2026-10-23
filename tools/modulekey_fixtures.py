"""Hermetic original Java SPI212 fixture exporter/verifier; no evaluator invocation."""
from pathlib import Path
import argparse, collections, hashlib, json, os, stat

PINS = {'report.json': 'dcaa95e8eab8b1a7efda01d2494758d44e5045abe7c75eb79224e0079d768efc', 'inventory.json': '11f1dbb9c1fc0b57761075c6ef9bf618210462f9f7ecefe32f02402cc7e41b29', 'closure.json': '40750b468c747e60b09f35ede8839983edd5400883b8731a2a2f85f3d173fcfd'}
FIRST = {'custom-resolved-importer', 'resolved-cache-alias-no-original-seed', 'uncached-policy-toggle', 'custom-key-pkl-uri-not-stdlib-kind', 'resolved-nested-jar-default-trust'}
SECOND = {'clause-unused-no-loader', 'expr-unused-no-loader', 'clause-unused-factory-throw', 'expr-unused-factory-throw'}
DEFAULT_ORACLE = Path('/Users/cody.ij.hwang/Projects/pkl-rs.stage48/target/m2/stage48-module-admission-oracle-01')
sha = lambda b: hashlib.sha256(b).hexdigest()
dump = lambda obj: (json.dumps(obj, indent=2) + '\n').encode()
def check(ok, message):
    if not ok:
        raise ValueError(message)
def safe(rel):
    p = Path(rel)
    check(not p.is_absolute() and '..' not in p.parts and str(p) == rel and rel != '.', 'unsafe relative payload')
    return p
def regular(root, rel):
    check(stat.S_ISDIR(root.lstat().st_mode), 'regular fixture root')
    parts = safe(rel).parts
    p = root
    for part in parts[:-1]:
        p = p / part
        check(stat.S_ISDIR(p.lstat().st_mode), 'regular payload parent')
    p = p / parts[-1]
    check(stat.S_ISREG(p.lstat().st_mode), 'regular payload leaf')
    return p.read_bytes()
def authorities(root, prefix):
    docs = {name: regular(root, prefix + name) for name in PINS}
    check(all(sha(docs[n]) == h for n, h in PINS.items()), 'literal original report/inventory/closure pins')
    r, inv, closure = (json.loads(docs[n]) for n in PINS)
    check(closure['sealed'] and closure['reportSha256'] == PINS['report.json'] and closure['inventorySha256'] == PINS['inventory.json'] and closure['liveHandles'] == [], 'original producer sealed')
    check(not r['inputChanges'] and r['workspacesUnchanged'] and r['jarStable'], 'original stable inputs')
    return docs, r, inv['files']
def construct(root, source=False):
    prefix = '' if source else 'provenance/'
    docs, report, inventory = authorities(root, prefix)
    files = {'provenance/' + n: b for n, b in docs.items()}
    origins = {}
    def get(rel):
        safe(rel)
        row = inventory[rel]
        check(row['kind'] == 'regular', 'selected regular original payload')
        b = regular(root, rel if source else 'original/' + rel)
        check(sha(b) == row['sha256'] and len(b) == row['bytes'], 'pinned original inventory payload')
        files['original/' + rel] = b
        origins['original/' + rel] = {'originalRelativePath': rel, 'sha256': row['sha256'], 'bytes': row['bytes']}
        return b
    declared = json.loads(get('cases.json'))
    for rel in ['AdmissionProbe.java', 'profile.json', 'outer-terminal.json']:
        get(rel)
    fixtures = []
    for c in declared:
        if c['id'] not in FIRST | SECOND:
            continue
        cohort = 'spi132' if c['id'] in FIRST else 'factory80'
        input_base = 'workspace/' + c['id'] + '/'
        for rel in c['inputPaths']:
            b = get(input_base + rel)
            check(sha(b) == report['inputHashes'][str(Path(c['workspace']) / rel)], 'producer authored input binding')
        providers = []
        for line in get(input_base + 'module-plan.tsv').decode().splitlines():
            requested, resolved, rel, cache = line.split('\t')
            payload = get(input_base + rel)
            providers.append({'requestedUri': requested, 'selectedUri': requested, 'resolvedUri': resolved, 'source': 'original/' + input_base + rel, 'sourceText': payload.decode(), 'originalAbsoluteSource': str(Path(c['workspace']) / rel), 'cacheMode': cache, 'keyKind': 'host', 'getterContract': 'Snapshot cached bool on actual factory construction; observe isCached getter separately. No Host URI/site cache.'})
        for obs in [o for o in report['observations'] if o['case'] == c['id']]:
            capture_prefix = f"cases/{c['id']}/{obs['repeat']}/"
            for rel in inventory:
                if rel.startswith(capture_prefix):
                    get(rel)
            for s in obs['sessions']:
                calls = [x for x in obs['calls'] if x['session'] == s['session']]
                expected = []
                for call in calls:
                    p = call['rawPrefix']
                    check(not call['missingFields'], 'complete original API capture')
                    status = get(p + '.status.bin')
                    check(status in [b'success\n', b'error\n'] and status.decode() == call['status'], 'exact original status')
                    e = {'status': {b'success\n':'success', b'error\n':'error'}[status], 'statusRawHex': status.hex(), 'javaRawPrefix': 'original/' + p, 'errorClass': call['errorClass'], 'messageNull': call['messageNull']}
                    for channel in ['stdout', 'stderr']:
                        b = get(p + '.' + channel + '.bin')
                        u = get(p + '.' + channel + '.code-units.bin')
                        check(b.hex() == call[channel+'Utf8Hex'] and u.hex() == call[channel+'Utf16BEHex'], 'report independent UTF16/JavaUTF8 binding')
                        e[channel+'Hex'] = b.hex()
                        e[channel+'Units'] = [int.from_bytes(u[i:i+2], 'big') for i in range(0,len(u),2)]
                    for when in ['Before', 'After']:
                        b = get(p + '.counters-' + when.lower() + '.bin')
                        check(b.decode() == call['counters'+when], 'report raw counter binding')
                        e['counters'+when] = dict((k,int(v)) for k,v in (line.split('=',1) for line in b.decode().splitlines()))
                        e['counterBytes'+when+'Hex'] = b.hex()
                    expected.append(e)
                for event in s['events']:
                    p = event['rawPrefix']
                    check(get(p+'.bin').hex() == event['utf8Hex'] and get(p+'.code-units.bin').hex() == event['utf16BEHex'], 'report ordered event binding')
                fixtures.append({'id': c['id'], 'cohort': cohort, 'repeat': obs['repeat'], 'session': s['session'], 'source': get(input_base+c['entry']).decode(), 'sourcePath': 'original/'+input_base+c['entry'], 'uri':c['sourceUri'], 'moduleName':Path(c['entry']).stem, 'initialPolicy':c['mode'], 'operations':c['operations'], 'expressions':[op['expression'] for op in c['operations'] if op['kind']=='evaluate'], 'moduleProviders':providers, 'expected':expected, 'orderedEvents':s['events'], 'finalCountersRawHex':s['counters'].encode().hex(), 'scope':c['scope']})
    count = sum(len(f['expected']) for f in fixtures)
    events = sum(len(f['orderedEvents']) for f in fixtures)
    check(count == 212 and events == 2248 and len(fixtures) == 36, 'exact selected original212 accounting')
    bundle = {'schema':1, 'fixtures':fixtures, 'javaApiCalls':count, 'javaOrderedEvents':events, 'unexecutedApiBoundaries':[], 'previousNativeBoundary':'Stage48 C01 all212 unexecuted; export does not retroactively execute them.', 'scope':['Raw API, actual typed request/handle and full original event/counter gates remain separate.','No synthetic primary/repl/factory events or Java implementation-class labels as Rust observations.','Java System.gc request is not proof of actual collection; native per-poll collect is a separate condition.','These212 original SPI calls are not whole ModuleKey/lifecycle/import* or full M2 coverage.']}
    files['fixtures.json'] = dump(bundle)
    files['provenance-map.json'] = dump({'schema':1,'pins':PINS,'originRoot':str(DEFAULT_ORACLE),'files':origins})
    return files, bundle
def coverage(root, expected):
    allowed = {str(p) for rel in expected for p in safe(rel).parents if str(p) != '.'}
    found = set()
    for current, dirs, names in os.walk(root, followlinks=False):
        for name in dirs:
            p = Path(current) / name
            rel = str(p.relative_to(root))
            check(rel in allowed and stat.S_ISDIR(p.lstat().st_mode), 'undeclared directory rejected before descent')
        for name in names:
            p = Path(current) / name
            check(stat.S_ISREG(p.lstat().st_mode), 'regular declared fixture payload')
            found.add(str(p.relative_to(root)))
    check(found == expected, 'exact actual payload coverage')
def verify(root):
    files, bundle = construct(root)
    coverage(root, set(files) | {'manifest.json'})
    manifest = json.loads(regular(root,'manifest.json'))
    check(manifest == {'schema':1, 'files':{rel:{'sha256':sha(b),'bytes':len(b)} for rel,b in files.items()}}, 'manifest exact reconstructed coverage')
    for rel, b in files.items():
        check(regular(root,rel) == b, 'reconstructed fixture bytes')
    return {'files':len(files)+1,'javaApiCalls':bundle['javaApiCalls'],'sessions':len(bundle['fixtures']),'javaOrderedEvents':bundle['javaOrderedEvents'],'manifestSha256':sha(regular(root,'manifest.json')),'evaluations':0}
def export(source, target):
    check(not target.exists(), 'fresh export directory')
    files, _ = construct(source, True)
    target.mkdir(parents=True)
    for rel, b in files.items():
        p=target/safe(rel);p.parent.mkdir(parents=True,exist_ok=True);p.write_bytes(b)
    (target/'manifest.json').write_bytes(dump({'schema':1,'files':{rel:{'sha256':sha(b),'bytes':len(b)} for rel,b in files.items()}}))
    return verify(target)
if __name__ == '__main__':
    p=argparse.ArgumentParser();p.add_argument('--fixtures',type=Path);p.add_argument('--export',type=Path);p.add_argument('--oracle',type=Path,default=DEFAULT_ORACLE);a=p.parse_args()
    check(bool(a.fixtures) != bool(a.export), 'choose --fixtures or --export')
    print(json.dumps(verify(a.fixtures) if a.fixtures else export(a.oracle,a.export),indent=2))
