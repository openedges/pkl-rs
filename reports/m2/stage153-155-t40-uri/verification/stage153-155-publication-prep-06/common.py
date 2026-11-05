"""Finite Stage153–155 publication identities; no discovery or workloads."""
import hashlib,json,os,re,stat
from pathlib import Path
ROOT=Path('/Users/cody.ij.hwang/Projects/pkl-rs.stage57')
HEAD='6d5d72f66a4a7e9343781d570600c8782f460d9c'
ROLE_NAMES={'package_uri74_boundary_raw_and_independent_readback', 'same_workspace_raw266_channels_and_reader', 'focused_native_source_regressions', 'm1_clone_capture_raw_channels_and_reader', 'terminal_workspace_guard_inline_source_bookends', 'workspace_aggregate_independent_readback', 'file_uri48_boundary_raw_and_independent_readback', 'default_prechecks_terminal_and_readback', 'final_docs_and_source_audit', 't40_original2_raw_and_separate_cache_controls'}
ARCHIVES=('.tar','.tar.gz','.tgz','.tar.zst','.zip','.partial')
BINARY=('.class','.rlib','.rmeta','.wasm','.node','.dylib','.so','.a','.o','.jar','.exe','.dll')
GUARD_HELPER_PINS=[{'path': '/Users/cody.ij.hwang/Projects/pkl-rs.stage57/target/m2/stage153-155-execution-prep-01/run.py', 'bytes': 7461, 'sha256': 'e7586e7f77c8fafdf9be575f41a4104cccfe3f32806d46cff17d627b43a51134', 'mode': 420}, {'path': '/Users/cody.ij.hwang/Projects/pkl-rs.stage57/target/m2/stage122-composed-lossless-glob576-prep-01/owned_processes.py', 'bytes': 3686, 'sha256': '542f1a5bbd2bba80795ca3b5dbe1fbe0beeec92dee02f716ec9b856e1b92e58d', 'mode': 420}, {'path': '/Users/cody.ij.hwang/Projects/pkl-rs.stage57/target/m2/stage122-composed-lossless-glob576-prep-01/child_gate.py', 'bytes': 254, 'sha256': 'f436bb983c45f832ba8bfb2203089ed18c676c83804cf942f8f88f15419a90df', 'mode': 420}]
EPOCH_RECEIPT={'path': '/Users/cody.ij.hwang/Projects/pkl-rs.stage57/target/m2/stage153-155-source07-transition-01/report.json', 'bytes': 5466, 'sha256': 'abb617c4de306492c5957581b38bd3c1aa2bdd8f6959226b20178dabe7fb7487', 'mode': 420}
EPOCH_CHANGES=[{'destination': '/Users/cody.ij.hwang/Projects/pkl-rs.stage57/crates/pkl-runtime/tests/m2_standard_module_diagnostics.rs', 'verified': {'path': '/Users/cody.ij.hwang/Projects/pkl-rs.stage57/crates/pkl-runtime/tests/m2_standard_module_diagnostics.rs', 'bytes': 14609, 'sha256': 'bb5a03fe912cb5bdac09eec9e97d02845ce33a87855016a6171f5801cc4ec04c', 'mode': 420}, 'before_alias': {'path': '/Users/cody.ij.hwang/Projects/pkl-rs.stage57/target/m2/stage153-155-source07-transition-01/before/crates/pkl-runtime/tests/m2_standard_module_diagnostics.rs', 'bytes': 14609, 'sha256': 'bb5a03fe912cb5bdac09eec9e97d02845ce33a87855016a6171f5801cc4ec04c', 'mode': 420}, 'unformatted': {'path': '/Users/cody.ij.hwang/Projects/pkl-rs.stage57/crates/pkl-runtime/tests/m2_standard_module_diagnostics.rs', 'bytes': 14844, 'sha256': 'd26c05dba0d4a8b0473f299dc26fec42df093ee04896ef8d32d76c39f343cde0', 'mode': 420}, 'current': {'path': '/Users/cody.ij.hwang/Projects/pkl-rs.stage57/crates/pkl-runtime/tests/m2_standard_module_diagnostics.rs', 'bytes': 14866, 'sha256': 'af7c7d5355a9fa3613fecf60cc288bde2307bd19d38b6b289095ea55783c1831', 'mode': 420}}, {'destination': '/Users/cody.ij.hwang/Projects/pkl-rs.stage57/tools/m2_module_eval.py', 'verified': {'path': '/Users/cody.ij.hwang/Projects/pkl-rs.stage57/tools/m2_module_eval.py', 'bytes': 22416, 'sha256': '270d2a3a46021e1c08a58a54db97d3b68b96028023aa7c298590af1542f7b11e', 'mode': 420}, 'before_alias': {'path': '/Users/cody.ij.hwang/Projects/pkl-rs.stage57/target/m2/stage153-155-source07-transition-01/before/tools/m2_module_eval.py', 'bytes': 22416, 'sha256': '270d2a3a46021e1c08a58a54db97d3b68b96028023aa7c298590af1542f7b11e', 'mode': 420}, 'unformatted': {'path': '/Users/cody.ij.hwang/Projects/pkl-rs.stage57/tools/m2_module_eval.py', 'bytes': 22583, 'sha256': '7f5a4fff88a998a7e14ef9f86c2ea733828d70d50239624d9b96ca05ae43b702', 'mode': 420}, 'current': {'path': '/Users/cody.ij.hwang/Projects/pkl-rs.stage57/tools/m2_module_eval.py', 'bytes': 22583, 'sha256': '7f5a4fff88a998a7e14ef9f86c2ea733828d70d50239624d9b96ca05ae43b702', 'mode': 420}}, {'destination': '/Users/cody.ij.hwang/Projects/pkl-rs.stage57/tools/tests/test_m2_selected_regressions.py', 'verified': {'path': '/Users/cody.ij.hwang/Projects/pkl-rs.stage57/tools/tests/test_m2_selected_regressions.py', 'bytes': 12422, 'sha256': '328b7cc12a47b2644ffc5fc6a513c1513c6a72aafcee113fcd6a0e22bdcfce20', 'mode': 420}, 'before_alias': {'path': '/Users/cody.ij.hwang/Projects/pkl-rs.stage57/target/m2/stage153-155-source07-transition-01/before/tools/tests/test_m2_selected_regressions.py', 'bytes': 12422, 'sha256': '328b7cc12a47b2644ffc5fc6a513c1513c6a72aafcee113fcd6a0e22bdcfce20', 'mode': 420}, 'unformatted': {'path': '/Users/cody.ij.hwang/Projects/pkl-rs.stage57/tools/tests/test_m2_selected_regressions.py', 'bytes': 16396, 'sha256': '2f01dc975de74a185dbb4d6eac8458b2cfbbc6417de19e0c136e47c682666410', 'mode': 420}, 'current': {'path': '/Users/cody.ij.hwang/Projects/pkl-rs.stage57/tools/tests/test_m2_selected_regressions.py', 'bytes': 16396, 'sha256': '2f01dc975de74a185dbb4d6eac8458b2cfbbc6417de19e0c136e47c682666410', 'mode': 420}}]
SOURCE_PLAN07={'path': '/Users/cody.ij.hwang/Projects/pkl-rs.stage57/target/m2/stage153-155-config-07/source-plan.json', 'bytes': 3352458, 'sha256': '1e04a12c369824d4a121ce9aa8365833e16f31bcefe11144c2bdc5e9fc7c937e', 'mode': 420}
ORIGINAL_WORKSPACE_GUARD={'path': '/Users/cody.ij.hwang/Projects/pkl-rs.stage57/target/m2/stage153-155-workspace-01/report.json', 'bytes': 6944633, 'sha256': '2fd0c877e100abfadfe0500454a4c14072684234781383d2f2363d190c6a647f', 'mode': 420}
ORIGINAL_WORKSPACE_READER={'path': '/Users/cody.ij.hwang/Projects/pkl-rs.stage57/target/m2/stage153-155-workspace-readback-01/report.json', 'bytes': 433111, 'sha256': 'a2f6088c8913f1c66ddd3c1703a20730eaef5fecf97ad7519dee37892cc60912', 'mode': 420}
ORIGINAL_T40_GUARD={'path': '/Users/cody.ij.hwang/Projects/pkl-rs.stage57/target/m2/stage153-155-t40-original-guard-01/report.json', 'bytes': 3372817, 'sha256': 'a56af501c65ff318a137acf5834ae198202d4bf6bb415a1718afdfd907a61eda', 'mode': 420}
FAILED_T40_SELECTION={'path': '/Users/cody.ij.hwang/Projects/pkl-rs.stage57/target/m2/stage153-155-t40-failed01-selection-01/files.json', 'bytes': 12779, 'sha256': '872eb68131ea798c041103098a7d1713ad2b5b1e1ea7a4c1d35a21a248085f78', 'mode': 420}
REQUIRED_PYTHON_VECTORS=[['/opt/homebrew/Cellar/python@3.14/3.14.8/Frameworks/Python.framework/Versions/3.14/bin/python3.14', '-B', '-m', 'unittest', 'discover', '-s', 'tools/tests', '-p', 'test_m2_selected_regressions.py', '-v'], ['/opt/homebrew/Cellar/python@3.14/3.14.8/Frameworks/Python.framework/Versions/3.14/bin/python3.14', '-B', '-m', 'unittest', 'discover', '-s', 'tools/tests', '-p', 'test_m2_language.py', '-v'], ['/opt/homebrew/Cellar/python@3.14/3.14.8/Frameworks/Python.framework/Versions/3.14/bin/python3.14', '-B', '-m', 'unittest', 'discover', '-s', 'tools/tests', '-p', 'test_m2_module_eval.py', '-v'], ['/opt/homebrew/Cellar/python@3.14/3.14.8/Frameworks/Python.framework/Versions/3.14/bin/python3.14', '-B', '-m', 'unittest', 'discover', '-s', 'tools/tests', '-p', 'test_m2_module_capture_failures.py', '-v'], ['/opt/homebrew/Cellar/python@3.14/3.14.8/Frameworks/Python.framework/Versions/3.14/bin/python3.14', '-B', '-m', 'unittest', 'discover', '-s', 'tools/tests', '-p', 'test_m2_historical.py', '-v']]
NATIVE_TARGET_ARGV=['/Users/cody.ij.hwang/.rustup/toolchains/1.98.1-aarch64-apple-darwin/bin/cargo', 'test', '-p', 'pkl-runtime', '--test', 'm2_standard_module_diagnostics', '--locked', '--message-format=json', '--', '--test-threads=1']
FAILED_T40_LICENSE_PINS=[{'path': '/Users/cody.ij.hwang/Projects/pkl-rs.stage57/target/m2/stage153-155-t40-workspace-01/tests/fixtures/rev1/LICENSE', 'bytes': 10765, 'sha256': '87f7f506593ec1c2b6a5b34bb449cd37f8ec7f040479f37a6697f6750ecc6296', 'mode': 420}, {'path': '/Users/cody.ij.hwang/Projects/pkl-rs.stage57/target/m2/stage153-155-t40-workspace-01/tests/fixtures/rev1/NOTICE', 'bytes': 1318, 'sha256': '70ad1559eebf49c6ffea8dba7ecb646289506f669dc52fbdcd01f0f13afd27ee', 'mode': 420}]
T40_PRIVATE_CACHE_IDENTITIES={'/Users/cody.ij.hwang/Projects/pkl-rs.stage57/target/m2/stage153-155-t40-original-02/cases/51bbcfe4ca6096dcb961bd17/java/private-state/home/Library/Caches/org.graalvm.polyglot/engine/libtruffleattach/03770ea6f62f0ff5b8205b1cb3a73672e0981ba6bf6c018b2b9fc6360fc98543/bin/libtruffleattach.dylib': {'path': '/Users/cody.ij.hwang/Projects/pkl-rs.stage57/target/m2/stage153-155-t40-original-02/cases/51bbcfe4ca6096dcb961bd17/java/private-state/home/Library/Caches/org.graalvm.polyglot/engine/libtruffleattach/03770ea6f62f0ff5b8205b1cb3a73672e0981ba6bf6c018b2b9fc6360fc98543/bin/libtruffleattach.dylib', 'bytes': 51363, 'sha256': '9c871271741bd5bdb8a39a7bb7ac1f2591446c03a52268a93266051f9b36d657', 'mode': 420}, '/Users/cody.ij.hwang/Projects/pkl-rs.stage57/target/m2/stage153-155-t40-original-02/cases/b01be896fe1b43f5c262dde8/java/private-state/home/Library/Caches/org.graalvm.polyglot/engine/libtruffleattach/03770ea6f62f0ff5b8205b1cb3a73672e0981ba6bf6c018b2b9fc6360fc98543/bin/libtruffleattach.dylib': {'path': '/Users/cody.ij.hwang/Projects/pkl-rs.stage57/target/m2/stage153-155-t40-original-02/cases/b01be896fe1b43f5c262dde8/java/private-state/home/Library/Caches/org.graalvm.polyglot/engine/libtruffleattach/03770ea6f62f0ff5b8205b1cb3a73672e0981ba6bf6c018b2b9fc6360fc98543/bin/libtruffleattach.dylib', 'bytes': 51363, 'sha256': '9c871271741bd5bdb8a39a7bb7ac1f2591446c03a52268a93266051f9b36d657', 'mode': 420}, '/Users/cody.ij.hwang/Projects/pkl-rs.stage57/target/m2/stage153-155-t40-cache-off-02/cases/51bbcfe4ca6096dcb961bd17/java/private-state/home/Library/Caches/org.graalvm.polyglot/engine/libtruffleattach/03770ea6f62f0ff5b8205b1cb3a73672e0981ba6bf6c018b2b9fc6360fc98543/bin/libtruffleattach.dylib': {'path': '/Users/cody.ij.hwang/Projects/pkl-rs.stage57/target/m2/stage153-155-t40-cache-off-02/cases/51bbcfe4ca6096dcb961bd17/java/private-state/home/Library/Caches/org.graalvm.polyglot/engine/libtruffleattach/03770ea6f62f0ff5b8205b1cb3a73672e0981ba6bf6c018b2b9fc6360fc98543/bin/libtruffleattach.dylib', 'bytes': 51363, 'sha256': '9c871271741bd5bdb8a39a7bb7ac1f2591446c03a52268a93266051f9b36d657', 'mode': 420}, '/Users/cody.ij.hwang/Projects/pkl-rs.stage57/target/m2/stage153-155-t40-cache-off-02/cases/b01be896fe1b43f5c262dde8/java/private-state/home/Library/Caches/org.graalvm.polyglot/engine/libtruffleattach/03770ea6f62f0ff5b8205b1cb3a73672e0981ba6bf6c018b2b9fc6360fc98543/bin/libtruffleattach.dylib': {'path': '/Users/cody.ij.hwang/Projects/pkl-rs.stage57/target/m2/stage153-155-t40-cache-off-02/cases/b01be896fe1b43f5c262dde8/java/private-state/home/Library/Caches/org.graalvm.polyglot/engine/libtruffleattach/03770ea6f62f0ff5b8205b1cb3a73672e0981ba6bf6c018b2b9fc6360fc98543/bin/libtruffleattach.dylib', 'bytes': 51363, 'sha256': '9c871271741bd5bdb8a39a7bb7ac1f2591446c03a52268a93266051f9b36d657', 'mode': 420}}
def lexical(value, body=True):
    assert isinstance(value,(str,Path))
    spelling=str(value)
    assert spelling and '\\' not in spelling and '\0' not in spelling
    path=Path(spelling)
    assert path.is_absolute() and path.is_relative_to(ROOT)
    assert spelling==path.as_posix() and all(p not in ('','.','..') for p in spelling.split('/')[1:])
    assert not any(p=='stage65-b-native-regressions-evidence.tar.gz.partial' or 'stage86' in p.lower() or 'private8' in p.lower() for p in path.parts)
    if body:
        assert not path.name.lower().endswith(ARCHIVES+BINARY)
        assert not any(p.startswith('stage142-yaml-native-cargo-') for p in path.parts)
    return path

def canonical(value):
    assert isinstance(value,dict) and set(value)=={'path','bytes','sha256','mode'}
    lexical(value['path'])
    assert type(value['bytes']) is int and value['bytes']>=0
    assert type(value['mode']) is int and 0<=value['mode']<=0o7777
    assert isinstance(value['sha256'],str) and re.fullmatch('[0-9a-f]{64}',value['sha256'])
    return value

def chain(path):
    # The complete lexical check precedes all metadata, including ancestor checks.
    path = lexical(path)
    current = Path(path.anchor)
    for part in path.parts[1:]:
        current /= part
        mode = current.lstat().st_mode
        assert not stat.S_ISLNK(mode), f'symlink: {current}'
        if current != path:
            assert stat.S_ISDIR(mode), f'non-directory ancestor: {current}'
    return path

def identity(info):
    return (info.st_dev, info.st_ino, info.st_size, info.st_mtime_ns,
            info.st_ctime_ns, stat.S_IMODE(info.st_mode))

def pin(path, expected=None, keep=False):
    path = chain(path)
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW)
    with os.fdopen(fd, 'rb') as stream:
        before = os.fstat(stream.fileno())
        assert stat.S_ISREG(before.st_mode)
        digest = hashlib.sha256()
        chunks = [] if keep else None
        while chunk := stream.read(1024 * 1024):
            digest.update(chunk)
            if keep:
                chunks.append(chunk)
        assert identity(before) == identity(os.fstat(stream.fileno())), 'file changed while reading'
    assert identity(before) == identity(chain(path).lstat()), 'path changed while reading'
    actual = dict(path=str(path), bytes=before.st_size, sha256=digest.hexdigest(),
                  mode=stat.S_IMODE(before.st_mode))
    if expected is not None:
        assert actual == {key: expected[key] for key in actual}, f'pin mismatch: {path}'
    return actual, b''.join(chunks) if keep else None

def document(path, expected=None, sha=None):
    actual, body = pin(path, expected, keep=True)
    assert sha is None or actual['sha256'] == sha, f'authority hash: {path}'
    return actual, json.loads(body)

def expected_pin(value):
    canonical(value)
    return pin(value['path'],value)[0]

def read_pin(value):
    canonical(value)
    return document(value['path'],value)[1]

def json_assertions(document,clauses):
    # Explicit field paths only; no eval, arbitrary path discovery or normalization.
    assert isinstance(clauses,list)
    for clause in clauses:
        assert set(clause)=={'keys','equals'} and isinstance(clause['keys'],list) and clause['keys']
        value=document
        for key in clause['keys']:
            assert isinstance(key,(str,int)) and not isinstance(key,bool)
            value=value[key]
        assert type(value) is type(clause['equals']) and value==clause['equals'],clause['keys']

def validate_plan(plan):
    assert plan['schema_version']==1 and plan['head']==HEAD
    assert plan['draft'] is False and plan['all_workloads_terminal_reaped'] is True
    assert plan['source_frozen'] is True and plan['full_m2_claim'] is False
    roles=plan['roles'];assert len(roles)==9
    assert {x['role'] for x in roles}==ROLE_NAMES-{'final_docs_and_source_audit'}
    for row in roles:
        assert set(row)=={'role','actual_pin','assertions'}
        canonical(row['actual_pin']);assert row['actual_pin']['path'].endswith('.json')
        assert row['assertions'], 'Each actual reader/guard needs explicit root-reviewed contract assertions.'
    for key in ('records','payloads','source_rows','historical_dependencies','raw_maps','explicit_absences','excluded_body_roots'):
        assert isinstance(plan[key],list)
    assert plan['source_rows'] and plan['historical_dependencies']
    destinations=set()
    for row in plan['source_rows']:
        assert set(row)=={'destination','current'}
        canonical(row['current']);relative=Path(row['destination'])
        assert not relative.is_absolute() and row['destination']==relative.as_posix()
        assert row['current']['path']==str(lexical(ROOT/relative))
        assert row['destination'] not in destinations;destinations.add(row['destination'])
        assert relative.parts[0] in ('crates','tests','tools','compatibility','.cargo','vendor','xtask','external') or (relative.suffix=='.md' and row['destination'] in {t['destination']for t in plan['documentation_transitions']}) or str(relative) in ('Cargo.toml','Cargo.lock','TESTING.md','rust-toolchain.toml','references.lock','oracle.lock.json')
    canonical(plan['source_plan'])
    assert plan['source_plan']['path']==str(ROOT/'target/m2/stage153-155-config-06/source-plan.json')
    assert plan['source_plan']['sha256']=='ea54d1851c2d6da677523624d96b307820b9ca48f7eb34af9f33f4bda4ba7f77'
    assert isinstance(plan['documentation_transitions'],list)
    assert plan['source_epoch_transition']is None or isinstance(plan['source_epoch_transition'],dict)
    input_catalogue(plan['external_source_inputs'])
    assert isinstance(plan['identity_only_source_inputs'],list)
    canonical(plan['docs_manifest'])
    assert set(plan['docs'])=={'README','PLAN-insertion'}
    for item in plan['docs'].values():canonical(item)
    for item in plan['explicit_absences']:lexical(item)
    # Source/prep tables are finite inputs, not recursively expanded manifests.
    for row in plan['records']:
        canonical(row['pin']);assert row['pin']['path'].endswith('.json')
        assert isinstance(row['group'],str) and isinstance(row['assertions'],list)
    for row in plan['payloads']:
        canonical(row['pin']);assert isinstance(row['group'],str)
        assert row['kind'] in ('source','raw','authority')
    for value in plan['excluded_body_roots']:lexical(value,body=False)
    for row in plan['historical_dependencies']:
        assert row['fresh_java'] is False and row['body_selected'] is False
        assert isinstance(row['reason'],str) and row['reason']
        # Identity only: historical dependency bodies are never opened here.
        assert isinstance(row['identity'],dict)
    for row in plan['raw_maps']:
        assert row['kind'] in ('t40_inputs','raw266','m1_inputs')
        canonical(row['report']);assert isinstance(row['group'],str)
        lexical(row['root'])


def payload_path(value, kind='raw', excluded_roots=()):
    # Identity-only JSON metadata may describe excluded products. This function
    # admits the body itself, under an explicit source/raw/authority role.
    assert kind in ('source','raw','authority')
    p=lexical(value)
    for root in excluded_roots:
        root=lexical(root,body=False)
        assert not p.is_relative_to(root), 'Body lies in an explicitly excluded cache/artifact prefix.'
    if kind=='authority':assert p.suffix=='.json'
    if kind=='raw':
        # A raw extensionless/generated representation needs explicit handling;
        # never silently omit it or mistake it for an executable/cache product.
        assert p.suffix, 'Extensionless raw evidence requires a narrow reviewed representation.'
    # Explicit source rows may include LICENSE/NOTICE/PklProject/gradlew, without
    # inventing a text-filename whitelist. Exact final pins remain authoritative.
    return p

def process_channel_pin(value):
    # No non-pin stdout/stderr summary schema is currently established. Unknown
    # shapes fail closed rather than recursively discarding recognized fields.
    assert isinstance(value,dict)
    assert set(value) in ({'path','bytes','sha256'},{'path','bytes','sha256','mode'})
    payload_path(value['path'])
    assert type(value['bytes']) is int and value['bytes']>=0
    assert isinstance(value['sha256'],str) and re.fullmatch('[0-9a-f]{64}',value['sha256'])
    if 'mode' in value:assert type(value['mode']) is int and 0<=value['mode']<=0o7777
    return value


def input_catalogue(value):
    # Current M1 PINS.values() is an ordered canonical list, including identity-
    # only outsideROOT tools. Validate strings before any scope/body decision.
    assert isinstance(value,list)
    seen={}
    for row in value:
        assert isinstance(row,dict) and set(row)=={'path','bytes','sha256','mode'}
        spelling=row['path'];assert isinstance(spelling,str) and spelling and '\\' not in spelling and '\0' not in spelling
        p=Path(spelling);assert p.is_absolute() and spelling==p.as_posix()
        assert all(s not in ('','.','..') for s in spelling.split('/')[1:])
        assert not any(s=='stage65-b-native-regressions-evidence.tar.gz.partial' or 'stage86' in s.lower() or 'private8' in s.lower() for s in p.parts)
        assert type(row['bytes']) is int and row['bytes']>=0
        assert type(row['mode']) is int and 0<=row['mode']<=0o7777
        assert isinstance(row['sha256'],str) and re.fullmatch('[0-9a-f]{64}',row['sha256'])
        assert spelling not in seen, 'Duplicate or conflicting input catalogue path.'
        seen[spelling]=row
    return value


def m1_payload_kind(path,spec):
    root=lexical(spec['root']);p=lexical(path,body=False)
    assert p.is_relative_to(root)
    prefixes=spec.get('source_prefixes',[])
    assert isinstance(prefixes,list)
    if prefixes:
        expected={str(root/'experiment/sources'),str(root/'experiment/oracle-native/sources')}
        assert len(prefixes)==2 and set(prefixes)==expected, 'Only the exact two captured source-copy prefixes are admitted.'
        for value in prefixes:lexical(value,body=False)
    return 'source' if any(p.is_relative_to(Path(value)) for value in prefixes) else 'raw'


def source_coverage(plan, source_plan):
    """Join finite verified inputs to current bodies without rewriting old pins."""
    assert source_plan['head']==HEAD and source_plan['source_dependency_closure_established'] is False
    inputs=input_catalogue(source_plan['files'])
    assert len(inputs)==source_plan['file_count']
    outside=[r for r in inputs if not Path(r['path']).is_relative_to(ROOT)]
    assert input_catalogue(plan['external_source_inputs'])==outside, 'External sourceplan identities must remain exact metadata-only rows.'
    identities=identity_only_inputs(plan,inputs)
    original={r['path']:r for r in inputs if Path(r['path']).is_relative_to(ROOT)and r['path']not in identities}
    current={r['current']['path']:r['current'] for r in plan['source_rows']}
    assert len(current)==len(plan['source_rows']) and set(current)<=set(original)
    routes={path:'source-row' for path in current}
    for row in plan['payloads']:
        if row['kind']in ('source','authority')and row['pin']['path']in original:
            assert row['pin']['path']not in current, 'Sourceplan input must have exactly one coverage route.'
            current[row['pin']['path']]=row['pin'];routes[row['pin']['path']]=row['kind']+'-payload'
    for row in plan['records']:
        if row['pin']['path']in original:
            assert row['pin']['path']not in current, 'Sourceplan input must have exactly one coverage route.'
            current[row['pin']['path']]=row['pin'];routes[row['pin']['path']]='authority-record'
    transitions={}
    for row in plan['documentation_transitions']:
        assert set(row)=={'destination','verified','before_alias','current','receipt','assertions'}
        destination=row['destination'];assert isinstance(destination,str)
        p=lexical(ROOT/destination);assert p.suffix=='.md' and str(p.relative_to(ROOT))==destination
        for key in ('verified','before_alias','current','receipt'):canonical(row[key])
        assert row['verified']['path']==row['current']['path']==str(p)
        assert original.get(str(p))==row['verified'], 'Verified documentation pin must remain the exact sourceplan pin.'
        assert row['before_alias']['path'] not in original and row['receipt']['path'].endswith('.json')
        assert all(row['before_alias'][key]==row['verified'][key] for key in ('bytes','sha256','mode'))
        assert row['current']!=row['verified'] and current.get(str(p))==row['current']
        assert isinstance(row['assertions'],list) and row['assertions']
        assert str(p) not in transitions;transitions[str(p)]=row
    epoch=plan['source_epoch_transition']
    if epoch is not None:
        epoch_schema(plan,original,current)
        for row in epoch['changes']:
            path=row['verified']['path'];assert path not in transitions
            transitions[path]=dict(row,receipt=epoch['receipt'])
    assert set(current)==set(original), 'Every sourceplan input needs an exact finite coverage route.'
    coverage=[]
    for path,verified in original.items():
        actual=current[path]
        if path in transitions:
            row=transitions[path]
            item=dict(destination=str(Path(path).relative_to(ROOT)),route=routes[path],verified=verified,current=actual,before_alias=row['before_alias'],transition_receipt=row['receipt'])
            if epoch is not None and path in {r['verified']['path']for r in epoch['changes']}:item.update(transition_kind='exact-source07-test-tooling',current_source_plan=epoch['current_source_plan'],original_full_workspace_qualified=False)
            coverage.append(item)
        else:
            assert actual==verified, 'Non-document sourceplan bytes changed after verification.'
            coverage.append(dict(destination=str(Path(path).relative_to(ROOT)),route=routes[path],verified=verified,current=actual))
    return coverage


def paired_channels(reader, capture, count):
    """Exact paired API process/channel membership; no semantic pass requirement."""
    assert reader['readback_complete'] is True and reader['acquisition_complete'] is True
    assert reader['java_calls']==reader['native_calls']==count
    assert len(reader['cases'])==count and len(reader['processes'])==len(capture['commands'])==2*count
    ids=[row['id']for row in reader['cases']];assert len(set(ids))==count
    seen=set()
    for index,(process,command) in enumerate(zip(reader['processes'],capture['commands'])):
        identifier=ids[index//2];kind=('java','native')[index%2]
        assert process['id']==identifier and process['kind']==kind and command['name']==kind+'-'+identifier
        assert type(process['exit_code']) is int and process['exit_code']==command['exit_code']
        for channel in ('stdout','stderr'):
            canonical(process[channel]);canonical(command[channel])
            assert process[channel]==command[channel] and process[channel]['path'] not in seen
            seen.add(process[channel]['path'])
    return seen


def t40_catalogue(reader, cache, original, controls, spec):
    """Validate two closed reader lists; return finite requests, never open paths."""
    assert reader['readback_complete'] is True and reader['acquisition_complete'] is True
    assert reader['original_count']==original['selected_count']==2 and reader['full_m2_pass'] is False
    ids=['m2:rev1:t40-imported-declaration-authority','m2:rev1:t40-foreign-namesake']
    assert [r['id']for r in original['results']]==ids
    assert [(r['id'],r['status'])for r in original['results']]==[(r['id'],r['status'])for r in reader['observations']]
    assert cache['readback_complete'] is True and cache['acquisition_complete'] is True
    assert cache['selected_original'] is False and cache['control_count']==2 and cache['full_m2_pass'] is False
    assert cache['original_report']==reader['original_report']
    assert controls['selected_original'] is False and controls['acquisition_complete'] is True
    assert [r['original_id']for r in controls['controls']]==ids
    assert all(r['selected_original'] is False for r in controls['controls'])
    root=lexical(spec['root']);cache_root=lexical(spec['cache_root'])
    assert root==Path(reader['original_report']['path']).parent
    assert cache_root==Path(cache['control_report']['path']).parent and root!=cache_root
    assert spec['source_prefixes']==[str(root/'sources')]
    exclusions=spec['compiled_products'];assert exclusions==[str(root/'artifacts/module_eval_probe')]
    for path in exclusions:lexical(path,body=False)
    catalogue={}
    for row in input_catalogue(reader['inputs'])+input_catalogue(cache['inputs']):
        if row['path'] in catalogue:assert catalogue[row['path']]==row
        catalogue[row['path']]=row
    captured={p:r for p,r in catalogue.items()if Path(p).is_relative_to(root)}
    expected={str(root/'report.json'):reader['original_report']['sha256']}
    for name,digest in original['evidence_sha256'].items():
        assert isinstance(name,str) and name and not Path(name).is_absolute()
        p=lexical(str(root)+'/'+name,body=False)
        assert p.is_relative_to(root) and isinstance(digest,str) and re.fullmatch('[0-9a-f]{64}',digest)
        assert str(p) not in expected;expected[str(p)]=digest
    assert set(captured)==set(expected), 'Original reader catalogue must cover the complete canonical evidence map.'
    assert all(captured[p]['sha256']==digest for p,digest in expected.items())
    assert catalogue.get(cache['control_report']['path'])==cache['control_report']
    requested=[];omitted=[];seen_products=set()
    for p,row in catalogue.items():
        path=Path(p)
        if not(path.is_relative_to(root)or path.is_relative_to(cache_root)):continue
        if p in exclusions:
            seen_products.add(p);omitted.append(dict(row,reason='compiled-t40-product'));continue
        if p in T40_PRIVATE_CACHE_IDENTITIES:
            assert row==T40_PRIVATE_CACHE_IDENTITIES[p], 'Exact private cache library identity differs.'
            omitted.append(dict(row,reason='host-private-runtime-cache-library',body_selected=False,source_selected=False));continue
        kind='source' if path.is_relative_to(root/'sources')else'raw'
        payload_path(p,kind);requested.append((row,kind))
    assert seen_products==set(exclusions)
    return requested,omitted


def source_identity_required(path, excluded_roots):
    # Exact declared cache roots plus the project's .cache namespace. This is
    # metadata classification only; no referenced path is queried.
    p=Path(path)
    if not p.is_relative_to(ROOT):return True
    relative=p.relative_to(ROOT)
    return relative.parts[0]=='.cache' or p.name.lower().endswith(ARCHIVES+BINARY) or any(p.is_relative_to(lexical(root,body=False))for root in excluded_roots)


def identity_only_inputs(plan, inputs):
    original={r['path']:r for r in inputs};found={}
    for row in plan['identity_only_source_inputs']:
        assert set(row)=={'pin','category','reason','body_selected','acquisition_authority','assertions'}
        input_catalogue([row['pin']]);p=Path(row['pin']['path'])
        assert row['body_selected'] is False and isinstance(row['reason'],str)and row['reason']
        assert row['category']==('project-runtime-toolchain'if p.is_relative_to(ROOT)else'external-runtime-toolchain')
        assert source_identity_required(p,plan['excluded_body_roots']), 'Authored source cannot be omitted under a runtime category.'
        canonical(row['acquisition_authority']);assert row['acquisition_authority']['path'].endswith('.json')
        assert isinstance(row['assertions'],list)and row['assertions']
        assert original.get(str(p))==row['pin']and str(p)not in found
        found[str(p)]=row
    for path in original:
        if source_identity_required(path,plan['excluded_body_roots']):
            assert path in found, 'Runtime/cache/archive sourceplan row needs an exact metadata-only category and acquisition join.'
    return found


def guard_input_catalogue(authority, configured):
    # Config/sourceplan/M1 lists remain unique. Only the guard's prescribed
    # config/run/owned/child appends can repeat already-configured identities.
    rows=input_catalogue(configured['inputs']);canonical(authority['config'])
    assert authority['config']['path'].endswith('.json')
    appended=[authority['config'],*GUARD_HELPER_PINS]
    for pin_row in appended:canonical(pin_row)
    expected=rows+appended
    assert authority['inputs_before']==authority['inputs_after']==expected, 'Exact configured plus four guard-appended input rows required.'
    lookup={}
    for pin_row in expected:
        path=pin_row['path']
        if path in lookup:assert lookup[path]==pin_row, 'Conflicting prescribed guard input repeat.'
        lookup[path]=pin_row
    return lookup


def identity_acquisition(row, authority, configured):
    # The explicit authority is an actual closed guard and its exact pinned
    # config, never a sourceplan mistaken for acquisition. No semantic promotion.
    assert authority['complete'] is True and authority['errors']==[]
    assert authority['selected_source_bookends_stable'] is True
    lookup=guard_input_catalogue(authority,configured)
    assert lookup.get(row['pin']['path'])==row['pin']
    json_assertions(authority,row['assertions'])


def epoch_schema(plan, original, current):
    epoch=plan['source_epoch_transition']
    assert set(epoch)=={'prior_source_plan','current_source_plan','receipt','changes','python_guards','native_guard','native_reader','normal_binding'}
    assert epoch['prior_source_plan']==plan['source_plan'] and epoch['current_source_plan']==SOURCE_PLAN07
    assert epoch['receipt']==EPOCH_RECEIPT and epoch['changes']==EPOCH_CHANGES
    assert isinstance(epoch['python_guards'],list)and epoch['python_guards']
    for value in epoch['python_guards']+[epoch['native_guard'],epoch['native_reader'],epoch['normal_binding']]:canonical(value);assert value['path'].endswith('.json')
    assert len({v['path']for v in epoch['python_guards']})==len(epoch['python_guards'])
    for row in epoch['changes']:
        path=row['destination'];assert path==row['verified']['path']==row['current']['path']
        assert original.get(path)==row['verified'] and current.get(path)==row['current']
        assert row['before_alias']['path']not in original
        assert all(row['before_alias'][k]==row['verified'][k]for k in ('bytes','sha256','mode'))
        for key in ('verified','before_alias','unformatted','current'):canonical(row[key])
    roles={r['role']:r['actual_pin']for r in plan['roles']}
    assert roles['terminal_workspace_guard_inline_source_bookends']==ORIGINAL_WORKSPACE_GUARD
    assert roles['workspace_aggregate_independent_readback']==ORIGINAL_WORKSPACE_READER


def epoch_guard(load, value, post, epoch, require_pass):
    guard=load(value);assert guard['head']==HEAD and guard['complete']is True and guard['errors']==[]
    assert guard['selected_source_bookends_stable']is True
    configured=load(guard['config']);lookup=guard_input_catalogue(guard,configured)
    assert lookup.get(SOURCE_PLAN07['path'])==SOURCE_PLAN07
    assert all(lookup.get(r['path'])==r for r in post['files'])
    assert guard['environment']==configured['environment']and guard['limits']==configured['limits']and guard['scope']==configured['scope']
    assert len(guard['commands'])==len(configured['commands'])and guard['commands']
    for actual,wanted in zip(guard['commands'],configured['commands']):
        assert actual['name']==wanted['name']and actual['argv']==wanted['argv']and actual['command_environment']==wanted.get('environment',{})
        assert type(actual['exit_code'])is int and actual['exit_code']in wanted.get('accepted_exit_codes',[0])
        assert actual['stop_reason']is None and actual['owned']['root_observed']and not actual['owned']['terminal_live']
        if require_pass:assert actual['exit_code']==0
    assert guard['commands_succeeded']is all(c['exit_code']==0 for c in guard['commands'])
    assert guard['started_epoch']>=load(epoch['receipt'])['finished_epoch']
    return guard,configured


def verify_source_epoch(plan, before, load):
    """Validate only the exact reviewed06→07 bridge; never rewrite old guards."""
    epoch=plan['source_epoch_transition']
    if epoch is None:return None
    receipt=load(epoch['receipt']);assert receipt['complete']is True and receipt['head']==HEAD
    assert receipt['source_plan_before']==plan['source_plan']and receipt['transitions']==EPOCH_CHANGES
    assert receipt['all_other_selected_input_pins_unchanged']is True and receipt['runtime_rust_changed']is False
    assert receipt['original_workspace_pass']is False and receipt['whole_workspace_reexecuted']is False
    assert receipt['all_launched_source06_workloads_terminal_reaped']is True and receipt['source06_unlaunched_captures']==['file-uri','package-uri','M1']
    application=load(receipt['application']);assert application['complete']is True and application['source_plan_before']==plan['source_plan']and application['runtime_rust_changed']is False
    assert application['test_patch']['sha256']=='d439d1231d60011d71430ec4c338063387a58d9855259e97a42d3bade85a442c'
    assert application['tooling_candidate']['sha256']=='e19d908f2ca9cba365005e21b0d2fe7a497898cd0bebe1b8be59257ef5bd8fd8'
    assert application['transitions']==[{k:v for k,v in r.items()if k not in ('unformatted','current')}|{'current':r['unformatted']}for r in EPOCH_CHANGES]
    formatting=load(receipt['format_guard']);assert formatting['complete']is True and formatting['errors']==[]and formatting['selected_source_bookends_stable']is True and formatting['commands_succeeded']is True
    old_guard=load(ORIGINAL_WORKSPACE_GUARD);old_reader=load(ORIGINAL_WORKSPACE_READER);old_t40=load(ORIGINAL_T40_GUARD)
    assert old_guard['commands'][0]['exit_code']==101 and old_guard['commands_succeeded']is False
    assert old_reader['readback_complete']is True and old_reader['acquisition_qualified']is True and old_reader['qualified']is False and old_reader['all_tests_pass']is False and old_reader['cargo_exit_code']==101
    assert old_reader['workspace_report']==ORIGINAL_WORKSPACE_GUARD and old_reader['source_plan']==plan['source_plan']
    assert old_reader['totals']==dict(passed=2190,failed=1,ignored=6,measured=0,filtered=0)and old_reader['outer_harness_count']==336
    assert old_t40['complete']is False and old_t40['commands'][0]['exit_code']==1
    assert max(old_guard['finished_epoch'],old_t40['finished_epoch'])<=receipt['started_epoch']
    post=load(epoch['current_source_plan']);assert post['head']==HEAD and post['source_dependency_closure_established']is False
    replacements={r['destination']:r['current']for r in EPOCH_CHANGES}
    assert input_catalogue(post['files'])==[replacements.get(r['path'],r)for r in input_catalogue(before['files'])]
    assert post['file_count']==before['file_count']==len(post['files'])
    seen=[]
    for value in epoch['python_guards']:
        guard,_=epoch_guard(load,value,post,epoch,True);seen.extend(c['argv']for c in guard['commands'])
    assert all(seen.count(vector)==1 for vector in REQUIRED_PYTHON_VECTORS)
    assert not any('test'in vector and'--workspace'in vector for vector in seen)
    native,config=epoch_guard(load,epoch['native_guard'],post,epoch,True)
    assert len(native['commands'])==1 and native['commands'][0]['argv']==NATIVE_TARGET_ARGV
    reader=load(epoch['native_reader']);assert reader['readback_complete']is True and reader['qualified']is True and reader['cargo_exit_code']==0 and reader['all_tests_pass']is True
    assert reader['target']=='m2_standard_module_diagnostics'and reader['guard_report']==epoch['native_guard']and reader['configuration']==native['config']and reader['source_plan']==SOURCE_PLAN07
    assert reader['test_source']==EPOCH_CHANGES[0]['current']and reader['whole_workspace_rerun']is False and reader['original_full_workspace_qualified']is False and reader['full_workspace_pass_claim']is False
    assert reader['original_workspace_guard']==ORIGINAL_WORKSPACE_GUARD and reader['original_workspace_reader']==ORIGINAL_WORKSPACE_READER
    totals=reader['totals'];assert set(totals)=={'passed','failed','ignored','measured','filtered'}and type(totals['passed'])is int and totals['passed']>0
    assert all(type(totals[k])is int and totals[k]==0 for k in ('failed','ignored','measured','filtered'))
    binding=load(epoch['normal_binding']);assert binding['complete']is True and binding['head']==HEAD and binding['source_plan']==SOURCE_PLAN07
    normal,normal_config=epoch_guard(load,binding['guard_report'],post,epoch,True)
    assert binding['build_config']==normal['config']and load(binding['source_inputs'])==normal_config['inputs']
    assert set(binding['artifacts'])==set(binding['normal_artifacts'])=={'module_eval_probe','file_uri_boundary_probe','package_uri_boundary_probe'}
    # Future runtime acquisitions must use07; old06 workspace/raw266 stay separate.
    roles={r['role']:load(r['actual_pin'])for r in plan['roles']}
    t40=roles['t40_original2_raw_and_separate_cache_controls'];assert t40['native_authority']['root_binding']==epoch['normal_binding']
    epoch_guard(load,t40['original_guard'],post,epoch,True)
    spec=next(s for s in plan['raw_maps']if s['kind']=='t40_inputs');cache=load(spec['cache_report']);epoch_guard(load,cache['capture_guard'],post,epoch,True)
    for role in ('file_uri48_boundary_raw_and_independent_readback','package_uri74_boundary_raw_and_independent_readback'):
        doc=roles[role];assert load(doc['native_binding'])['root_binding']==epoch['normal_binding'];epoch_guard(load,doc['capture_guard'],post,epoch,True)
    epoch_guard(load,roles['m1_clone_capture_raw_channels_and_reader']['guard'],post,epoch,False)
    return dict(current_source_plan=SOURCE_PLAN07,receipt=epoch['receipt'],original_workspace_cargo_exit_code=101,original_full_workspace_qualified=False,source06_unlaunched_captures=['file-uri','package-uri','M1'],whole_workspace_reexecuted=False)
