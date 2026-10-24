"""Pinned Java 0.32.0 DataSize authority, split raw audit and fresh acquisition."""
import argparse
from pathlib import Path
from collections import Counter, defaultdict
import hashlib, json, re, os, platform, subprocess, sys, time
ARTIFACT_SHA256 = 'c3b6f6215a08b31453b0fb0edd8c9da0aee31233f65abb8ab07f9c945151e059'
JDK_EXE_SHA256 = '51ec0764f7225b40d5b286b01401aa6b3990d0d171b446d7dc81ca7542b45552'
JDK_MODULES_SHA256 = '40fcf7a32bada7e299e3513939a4472d27f8f61eb81cdd1675ea00648de7e7cf'
JDK_RELEASE_SHA256 = 'ab66b8afeea98b25f1869fbe7c0dde368d448e7cc0f7bab0359b322ade134fe1'
ROOT = Path(__file__).resolve().parents[1]
FIXTURE = ROOT / 'tests/fixtures/m2-data-size'

def audit_split(capture, sources, *, write_audit=False):
    from pathlib import Path
    from collections import Counter, defaultdict
    import hashlib, json, re
    O = Path(capture)
    ROOT = Path(sources)
    read = lambda p: json.loads(p.read_bytes())
    sha = lambda p: hashlib.sha256(p.read_bytes()).hexdigest()
    p = read(O / 'provenance.json')
    m = read(O / 'cases.json')
    if not (p['complete'] and p['stable'] and p['string_raw_equal'] and p['export_raw_equal']):
        raise ValueError("Java DataSize audit check 19 failed: p['complete'] and p['stable'] and p['string_raw_equal'] and p['export_raw_equal']")
    if not (m['schema'] == 2 and m['boundaries'] == ['string', 'export'] and (m['repetitions'] == 2) and (len(m['rows']) == 255) and (len({x['id'] for x in m['rows']}) == 255)):
        raise ValueError("Java DataSize audit check 20 failed: m['schema']==2 and m['boundaries']==['string','export'] and m['repetitions']==2 and len(m['rows'])==")
    if not (O / 'queries.tsv').read_text() == ''.join((x['id'] + '\t' + x['expression'] + '\n' for x in m['rows'])):
        raise ValueError("Java DataSize audit check 21 failed: (O/'queries.tsv').read_text()==''.join(x['id']+'\\t'+x['expression']+'\\n' for x in m['rows'])")
    if not (p['input_before'] == p['input_after'] and p['source_before'] == p['source_after'] and (p['artifact_after_sha256'] == p['artifact_sha256'] == 'c3b6f6215a08b31453b0fb0edd8c9da0aee31233f65abb8ab07f9c945151e059')):
        raise ValueError("Java DataSize audit check 22 failed: p['input_before']==p['input_after'] and p['source_before']==p['source_after'] and p['artifact_after_")
    if not (set(p['input_before']) == set(INPUT_NAMES) and set(p['source_before']) == set(SOURCE_NAMES)):
        raise ValueError("Java DataSize audit check 23 failed: set(p['input_before'])==set(INPUT_NAMES) and set(p['source_before'])==set(SOURCE_NAMES)")
    if not (p['oracle_lock_sha256'] == sha(ROOT / 'oracle.lock.json') and p['references_lock_sha256'] == sha(ROOT / 'references.lock')):
        raise ValueError("Java DataSize audit check 24 failed: p['oracle_lock_sha256']==sha(ROOT/'oracle.lock.json') and p['references_lock_sha256']==sha(ROOT/'ref")
    lock = read(ROOT / 'oracle.lock.json')
    if not (lock['version'] == '0.32.0' and lock['source']['revision'] == p['reference_revision'] == 'e51fb2f91774265666f94980ec630db794a4db2d'):
        raise ValueError("Java DataSize audit check 26 failed: lock['version']=='0.32.0' and lock['source']['revision']==p['reference_revision']=='e51fb2f917742656")
    for n, h in p['input_before'].items():
        if not sha(O / n) == h:
            raise ValueError('Java DataSize audit check 27 failed: sha(O/n)==h')
    for n, h in p['source_before'].items():
        if not sha(ROOT / n) == h:
            raise ValueError('Java DataSize audit check 28 failed: sha(ROOT/n)==h')
    if not p['artifact_sha256'] == ARTIFACT_SHA256:
        raise ValueError("Java DataSize audit check 29 failed: p['artifact_sha256']==ARTIFACT_SHA256")
    if not (p['jdk']['executable_sha256'] == JDK_EXE_SHA256 and p['jdk']['modules_sha256'] == JDK_MODULES_SHA256 and (p['jdk']['release_sha256'] == JDK_RELEASE_SHA256)):
        raise ValueError("Java DataSize audit check 30 failed: p['jdk']['executable_sha256']==JDK_EXE_SHA256 and p['jdk']['modules_sha256']==JDK_MODULES_SHA256 and")
    if not (p['jdk']['properties']['java.version'] == '21.0.12.1' and p['jdk']['properties']['java.vendor'] == 'Eclipse Adoptium'):
        raise ValueError("Java DataSize audit check 31 failed: p['jdk']['properties']['java.version']=='21.0.12.1' and p['jdk']['properties']['java.vendor']=='Ecli")
    if not (len(p['sessions']) == 4 and [(x['mode'], x['repeat']) for x in p['sessions']] == [('string', 0), ('string', 1), ('export', 0), ('export', 1)]):
        raise ValueError("Java DataSize audit check 32 failed: len(p['sessions'])==4 and [(x['mode'],x['repeat']) for x in p['sessions']]==[('string',0),('string',")
    if not (len({x['pid'] for x in p['sessions']}) == 4 and all((x['started_epoch'] < x['finished_epoch'] for x in p['sessions']))):
        raise ValueError("Java DataSize audit check 33 failed: len({x['pid'] for x in p['sessions']})==4 and all(x['started_epoch']<x['finished_epoch'] for x in p[")
    if not all((p['sessions'][i]['finished_epoch'] <= p['sessions'][i + 1]['started_epoch'] for i in range(3))):
        raise ValueError("Java DataSize audit check 34 failed: all(p['sessions'][i]['finished_epoch']<=p['sessions'][i+1]['started_epoch'] for i in range(3))")
    string_text = ['expression', 'result.stdout', 'result.stderr', 'result.error-class']
    string_status = ['result.status']
    export_text = ['expression']
    for ordinal in [0, 1]:
        export_text += [f'call-{ordinal}.' + x for x in ['error', 'error-class', 'java-type', 'payload-error', 'text', 'raw-bits-hex', 'unit', 'hash']]
    comparison_text = ['comparison.error', 'comparison.reference', 'comparison.equals', 'comparison.hash-equal']
    string_files = {f + '.bin' for f in string_status} | {f + s for f in string_text for s in ['.bin', '.code-units.bin']}
    base_export_files = {'repeat-invoked.bin', 'call-0.status.bin', 'call-0.payload-status.bin'} | {f + s for f in ['expression'] + [f'call-0.' + x for x in ['error', 'error-class', 'java-type', 'payload-error', 'text', 'raw-bits-hex', 'unit', 'hash']] for s in ['.bin', '.code-units.bin']}
    repeat_export_files = {'call-1.status.bin', 'call-1.payload-status.bin', 'comparison.status.bin'} | {f + s for f in [f'call-1.' + x for x in ['error', 'error-class', 'java-type', 'payload-error', 'text', 'raw-bits-hex', 'unit', 'hash']] + comparison_text for s in ['.bin', '.code-units.bin']}
    if not (len(string_files), len(base_export_files), len(repeat_export_files)) == (9, 21, 27):
        raise ValueError('Java DataSize audit check 42 failed: (len(string_files),len(base_export_files),len(repeat_export_files))==(9,21,27)')

    def text(d, name):
        return (d / (name + '.bin')).read_text()

    def status(d, name):
        return (d / (name + '.bin')).read_bytes().decode()

    def check_text(d, n):
        raw = (d / (n + '.bin')).read_bytes()
        units = (d / (n + '.code-units.bin')).read_bytes()
        if not len(units) % 2 == 0:
            raise ValueError('Java DataSize audit check 46 failed: len(units)%2==0')
        if not units.decode('utf-16-be', errors='surrogatepass').encode('utf-8', errors='replace') == raw:
            raise ValueError("Java DataSize audit check 47 failed: units.decode('utf-16-be',errors='surrogatepass').encode('utf-8',errors='replace')==raw")

    def files(folder):
        return {x.relative_to(folder).as_posix(): sha(x) for x in folder.rglob('*') if x.is_file()}
    for mode, count in [('string', 2297), ('export', 8975)]:
        a, b = (files(O / mode / 'repeat-0'), files(O / mode / 'repeat-1'))
        if not (a == b and len(a) == count and (p[mode + '_files'] == count) and (p[mode + '_raw_differences'] == [])):
            raise ValueError("Java DataSize audit check 50 failed: a==b and len(a)==count and p[mode+'_files']==count and p[mode+'_raw_differences']==[]")
    counts = defaultdict(Counter)
    exported = []
    errors = []
    sessions = []
    for entry in p['sessions']:
        mode = entry['mode']
        repeat = entry['repeat']
        folder = O / mode / f'repeat-{repeat}'
        if not (entry['exit_code'] == 0 and entry['query_directories'] == 255):
            raise ValueError("Java DataSize audit check 53 failed: entry['exit_code']==0 and entry['query_directories']==255")
        recorded = Path(entry['cwd'])
        if not (recorded.is_absolute() and entry['command'] == [p['jdk']['executable'], *lock['jvm_options'], f"-Duser.home={p['environment']['HOME']}", f"-Djava.io.tmpdir={p['environment']['TMPDIR']}", '-cp', p['artifact'], str(recorded / 'DataSizeSplitOracle.java'), mode, str(recorded / mode / f'repeat-{repeat}'), str(recorded), m['module_uri']]):
            raise ValueError("Java DataSize audit check 55 failed: recorded.is_absolute() and entry['command']==[\n      p['jdk']['executable'],*lock['jvm_options'],f'-")
        if not (sha(folder / 'process.stdout.bin') == entry['stdout_sha256'] and sha(folder / 'process.stderr.bin') == entry['stderr_sha256']):
            raise ValueError("Java DataSize audit check 60 failed: sha(folder/'process.stdout.bin')==entry['stdout_sha256'] and sha(folder/'process.stderr.bin')==entry")
        if not ((folder / 'process.stdout.bin').stat().st_size == entry['stdout_bytes'] and (folder / 'process.stderr.bin').stat().st_size == entry['stderr_bytes']):
            raise ValueError("Java DataSize audit check 61 failed: (folder/'process.stdout.bin').stat().st_size==entry['stdout_bytes'] and (folder/'process.stderr.bin'")
        if not (folder / 'process.stderr.bin').read_bytes() == b'':
            raise ValueError("Java DataSize audit check 62 failed: (folder/'process.stderr.bin').read_bytes()==b''")
        lines = [f'session-open mode={mode} rows=255']
        calls = 0
        success = 0
        failure = 0
        ds_rows = 0
        repeat_calls = 0
        for i, row in enumerate(m['rows']):
            id = row['id']
            d = folder / f'{i:03d}-{id}'
            if not d.is_dir():
                raise ValueError('Java DataSize audit check 65 failed: d.is_dir()')
            if not (d / 'expression.bin').read_bytes() == row['expression'].encode():
                raise ValueError("Java DataSize audit check 66 failed: (d/'expression.bin').read_bytes()==row['expression'].encode()")
            check_text(d, 'expression')
            intended = row['intended_outcome']
            if not intended in ['ordinary-value', 'type-error', 'undefined-member', 'renderer-error', 'invalid-operator-or-call', 'truncation-error']:
                raise ValueError("Java DataSize audit check 67 failed: intended in ['ordinary-value','type-error','undefined-member','renderer-error','invalid-operator-or-")
            if mode == 'string':
                if not {x.name for x in d.iterdir() if x.is_file()} == string_files:
                    raise ValueError('Java DataSize audit check 69 failed: {x.name for x in d.iterdir() if x.is_file()}==string_files')
                for n in string_text:
                    check_text(d, n)
                st = status(d, 'result.status')
                if not st in ['success\n', 'error\n', 'oracle-error\n', 'api-exception\n']:
                    raise ValueError("Java DataSize audit check 71 failed: st in ['success\\n','error\\n','oracle-error\\n','api-exception\\n']")
                if not st == ('success\n' if intended == 'ordinary-value' else 'error\n'):
                    raise ValueError("Java DataSize audit check 72 failed: st==('success\\n' if intended=='ordinary-value' else 'error\\n')")
                if st == 'success\n':
                    if not (text(d, 'result.stderr') == '' and text(d, 'result.error-class') == ''):
                        raise ValueError("Java DataSize audit check 73 failed: text(d,'result.stderr')=='' and text(d,'result.error-class')==''")
                elif not (text(d, 'result.stdout') == '' and text(d, 'result.stderr') and text(d, 'result.error-class').startswith('org.pkl.')):
                    raise ValueError("Java DataSize audit check 74 failed: text(d,'result.stdout')=='' and text(d,'result.stderr') and text(d,'result.error-class').startswith(")
                lines += [f'call-enter=string-{i}-{id}', f'call-exit=string-{i}-{id} status={st.strip()}']
                calls += 1
                success += st == 'success\n'
                failure += st != 'success\n'
                if repeat == 0:
                    counts[row['group']][st.strip()] += 1
                    if st != 'success\n':
                        errors.append({'id': id, 'group': row['group'], 'intended_outcome': intended, 'error_sha256': sha(d / 'result.stderr.bin'), 'producer_error_class': text(d, 'result.error-class')})
            else:
                for n in ['expression'] + [f'call-0.' + x for x in ['error', 'error-class', 'java-type', 'payload-error', 'text', 'raw-bits-hex', 'unit', 'hash']]:
                    check_text(d, n)
                st = status(d, 'call-0.status')
                ps = status(d, 'call-0.payload-status')
                if not st == ('success\n' if intended == 'ordinary-value' else 'error\n'):
                    raise ValueError("Java DataSize audit check 81 failed: st==('success\\n' if intended=='ordinary-value' else 'error\\n')")
                if not ps == ('success\n' if st == 'success\n' else 'not-applicable\n'):
                    raise ValueError("Java DataSize audit check 82 failed: ps==('success\\n' if st=='success\\n' else 'not-applicable\\n')")
                typ = text(d, 'call-0.java-type')
                repeat_flag = (d / 'repeat-invoked.bin').read_text()
                if not repeat_flag in ['true', 'false']:
                    raise ValueError("Java DataSize audit check 83 failed: repeat_flag in ['true','false']")
                if not (repeat_flag == 'true' if typ == 'org.pkl.core.DataSize' else repeat_flag == 'false'):
                    raise ValueError("Java DataSize audit check 84 failed: repeat_flag=='true' if typ=='org.pkl.core.DataSize' else repeat_flag=='false'")
                if not {x.name for x in d.iterdir() if x.is_file()} == (base_export_files | repeat_export_files if repeat_flag == 'true' else base_export_files):
                    raise ValueError('Java DataSize audit check 85 failed: {x.name for x in d.iterdir() if x.is_file()}==(base_export_files|repeat_export_files if repeat_flag=')
                if st == 'success\n':
                    if not (text(d, 'call-0.error') == '' and text(d, 'call-0.error-class') == ''):
                        raise ValueError("Java DataSize audit check 86 failed: text(d,'call-0.error')=='' and text(d,'call-0.error-class')==''")
                elif not (typ == '' and text(d, 'call-0.error') and text(d, 'call-0.error-class').startswith('org.pkl.')):
                    raise ValueError("Java DataSize audit check 87 failed: typ=='' and text(d,'call-0.error') and text(d,'call-0.error-class').startswith('org.pkl.')")
                call = f'export-{i}-{id}-0'
                lines += [f'call-enter={call}', f'call-exit={call} status={st.strip()}']
                calls += 1
                success += st == 'success\n'
                failure += st != 'success\n'
                if st == 'success\n':
                    lines += [f'payload-enter={call}', f'payload-exit={call} status={ps.strip()}']
                if repeat_flag == 'true':
                    ds_rows += 1
                    repeat_calls += 1
                    bits = text(d, 'call-0.raw-bits-hex')
                    unit = text(d, 'call-0.unit')
                    hashvalue = text(d, 'call-0.hash')
                    if not (re.fullmatch('[0-9a-f]{16}', bits) and unit in ['b', 'kb', 'kib', 'mb', 'mib', 'gb', 'gib', 'tb', 'tib', 'pb', 'pib'] and hashvalue.lstrip('-').isdigit()):
                        raise ValueError("Java DataSize audit check 93 failed: re.fullmatch('[0-9a-f]{16}',bits) and unit in ['b','kb','kib','mb','mib','gb','gib','tb','tib','pb',")
                    for n in [f'call-1.' + x for x in ['error', 'error-class', 'java-type', 'payload-error', 'text', 'raw-bits-hex', 'unit', 'hash']] + comparison_text:
                        check_text(d, n)
                    st1 = status(d, 'call-1.status')
                    ps1 = status(d, 'call-1.payload-status')
                    cs = status(d, 'comparison.status')
                    if not (st1 == 'success\n' and ps1 == 'success\n' and (cs == 'success\n') and (text(d, 'call-1.java-type') == 'org.pkl.core.DataSize')):
                        raise ValueError("Java DataSize audit check 96 failed: st1=='success\\n' and ps1=='success\\n' and cs=='success\\n' and text(d,'call-1.java-type')=='org.pkl.c")
                    if not (text(d, 'comparison.reference') in ['true', 'false'] and text(d, 'comparison.equals') in ['true', 'false'] and (text(d, 'comparison.hash-equal') in ['true', 'false'])):
                        raise ValueError("Java DataSize audit check 97 failed: text(d,'comparison.reference') in ['true','false'] and text(d,'comparison.equals') in ['true','false")
                    call1 = f'export-{i}-{id}-1'
                    compare = f'export-{i}-{id}-comparison'
                    lines += [f'call-enter={call1}', f'call-exit={call1} status=success', f'payload-enter={call1}', f'payload-exit={call1} status=success', f'comparison-enter={compare}', f'comparison-exit={compare} status=success']
                    calls += 1
                    success += 1
                    if repeat == 0:
                        exported.append({'id': id, 'expression': row['expression'], 'raw_bits_hex': bits, 'unit': unit, 'java_hash': int(hashvalue), 'export_repeat_reference': text(d, 'comparison.reference'), 'export_repeat_equals': text(d, 'comparison.equals'), 'export_repeat_hash_equal': text(d, 'comparison.hash-equal')})
                elif not all((text(d, 'call-0.' + n) == '' for n in ['raw-bits-hex', 'unit', 'hash'])):
                    raise ValueError("Java DataSize audit check 101 failed: all(text(d,'call-0.'+n)=='' for n in ['raw-bits-hex','unit','hash'])")
        lines.append(f'session-closed mode={mode} rows=255 public-calls={calls} success={success} errors={failure} datasize-rows={ds_rows} repeated-export-calls={repeat_calls}')
        if not (folder / 'process.stdout.bin').read_bytes() == ('\n'.join(lines) + '\n').encode():
            raise ValueError("Java DataSize audit check 103 failed: (folder/'process.stdout.bin').read_bytes()==('\\n'.join(lines)+'\\n').encode()")
        sessions.append({'mode': mode, 'repeat': repeat, 'pid': entry['pid'], 'public_evaluator_calls': calls, 'successes': success, 'ordinary_errors': failure, 'datasize_export_rows': ds_rows, 'repeated_export_calls': repeat_calls, 'stdout_sha256': entry['stdout_sha256'], 'stderr_sha256': entry['stderr_sha256'], 'raw_files': len(files(folder))})
    if not [(s['public_evaluator_calls'], s['successes'], s['ordinary_errors'], s['datasize_export_rows'], s['repeated_export_calls']) for s in sessions] == [(255, 233, 22, 0, 0), (255, 233, 22, 0, 0), (389, 367, 22, 134, 134), (389, 367, 22, 134, 134)]:
        raise ValueError("Java DataSize audit check 105 failed: [(s['public_evaluator_calls'],s['successes'],s['ordinary_errors'],s['datasize_export_rows'],s['repea")
    if not (len(exported) == 134 and len(errors) == 22 and (sum((sum(c.values()) for c in counts.values())) == 255)):
        raise ValueError('Java DataSize audit check 106 failed: len(exported)==134 and len(errors)==22 and sum(sum(c.values()) for c in counts.values())==255')
    result = {'scope': 'Independent offline integrity, exact protocol and authored-intent audit of split Java public API DataSize authority; no guest/test/build rerun or Rust candidate.', 'provenance_sha256': sha(O / 'provenance.json'), 'cases_sha256': sha(O / 'cases.json'), 'module_sha256': sha(O / 'module.pkl'), 'queries_sha256': sha(O / 'queries.tsv'), 'probe_sha256': sha(O / 'DataSizeSplitOracle.java'), 'oracle_artifact_sha256': p['artifact_sha256'], 'source_sha256': p['source_before'], 'jdk': p['jdk'], 'module_uri': m['module_uri'], 'query_rows': 255, 'groups': {g: dict(c) for g, c in counts.items()}, 'sessions': sessions, 'raw_repetition_equal': {'string': True, 'export': True}, 'exported_datasize_rows': 134, 'exported_datasize_observations': exported, 'intentional_error_rows': errors, 'error_count': 22, 'producer_metadata_boundary': 'error-class/java-type and DataSize export bits/unit/hash belong to actual Java producer API. Common string candidate comparison uses only string result status and full stdout/stderr UTF-8 plus UTF-16 channels and process protocol.', 'earlier_packets': 'Oracle01 and Oracle02 preserved as acquired; Oracle03 split sessions are the canonical common-string and exported-value authority.', 'remaining_required': ['Pkl Binary public codec API and malformed unit/length', 'Bytes.size producer and large Bytes diagnostic formatting', 'Yaml/Properties/PList/Jsonnet/Protobuf/XML DataSize renderer boundaries', 'Candidate native/Node/browser execution and API parity', 'Whole language, lifecycle, other operating systems and performance gates']}
    if write_audit:
        (O / 'audit.json').write_text(json.dumps(result, indent=2, sort_keys=True) + '\n')
    elif not read(O / 'audit.json') == result:
        raise ValueError("Java DataSize audit check 111 failed: read(O/'audit.json')==result")
    return result
SOURCE_NAMES = ['external/pkl/stdlib/base.pkl', 'external/pkl/pkl-core/src/main/java/org/pkl/core/runtime/VmDataSize.java', 'external/pkl/pkl-core/src/main/java/org/pkl/core/stdlib/base/DataSizeNodes.java', 'external/pkl/pkl-core/src/main/java/org/pkl/core/DataSizeUnit.java', 'external/pkl/pkl-core/src/main/java/org/pkl/core/DataSize.java']
INPUT_NAMES = ['DataSizeSplitOracle.java', 'run.py', 'module.pkl', 'queries.tsv', 'cases.json']

def acquire(output):
    """Create one fresh, trace-complete pair of Java string and export sessions."""
    import shutil
    output = Path(output).resolve()
    if output.exists():
        raise ValueError(f'fresh output required: {output}')
    lock = json.loads((ROOT / 'oracle.lock.json').read_bytes())
    jar = ROOT / '.cache/oracle' / lock['artifact']['filename']
    if hashlib.sha256(jar.read_bytes()).hexdigest() != ARTIFACT_SHA256:
        raise ValueError('pinned Java JAR differs')
    if lock['version'] != '0.32.0' or lock['source']['revision'] != 'e51fb2f91774265666f94980ec630db794a4db2d':
        raise ValueError('Pkl baseline differs')
    if 'pkl.revision=' + lock['source']['revision'] not in (ROOT / 'references.lock').read_text():
        raise ValueError('reference lock differs')

    def sha(path):
        return hashlib.sha256(Path(path).read_bytes()).hexdigest()
    if sha(ROOT / 'tools/java/DataSizeSplitOracle.java') != 'a4389bb16f2cc784288dd79859afb2991ac574ac84f63319ad56c583f91cabbb':
        raise ValueError('executed probe bytes differ')
    for name, value in [('cases.json', '0e6f86ac0c9da84e7dce1b622bd99e2577b24cae4cf921ece7b6ef02a6a18a39'), ('module.pkl', 'fba4e81343683bbe433660e168905537fbd7d6b0dd44699e2b61b321b0a23982'), ('queries.tsv', '2946ae533b25cb6ca76ab8694cd866cfb1781dfc9ec29b0c9584a28229ac4728')]:
        if sha(FIXTURE / name) != value:
            raise ValueError(f'authored {name} differs')
    java = Path(os.environ.get('PKL_ORACLE_JAVA', '/usr/bin/java')).resolve()
    env = {'PATH': os.defpath, 'HOME': str(ROOT / '.cache/oracle/home'), 'TMPDIR': str(ROOT / '.cache/oracle/tmp'), 'TMP': str(ROOT / '.cache/oracle/tmp'), 'TEMP': str(ROOT / '.cache/oracle/tmp'), 'LC_ALL': 'C', 'LANG': 'C', 'TZ': 'UTC', 'TERM': 'dumb', 'NO_COLOR': '1', 'COLUMNS': '80'}
    output.mkdir(parents=True)
    for name, source in [('DataSizeSplitOracle.java', ROOT / 'tools/java/DataSizeSplitOracle.java'), ('run.py', ROOT / 'tools/data_size_oracle.py'), ('module.pkl', FIXTURE / 'module.pkl'), ('queries.tsv', FIXTURE / 'queries.tsv'), ('cases.json', FIXTURE / 'cases.json')]:
        shutil.copyfile(source, output / name)
    for name in [*SOURCE_NAMES, 'oracle.lock.json', 'references.lock']:
        target = output / 'sources' / name
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(ROOT / name, target)
    identity = subprocess.run([str(java), '-XshowSettings:properties', '-version'], cwd=output, env=env, capture_output=True, timeout=60)
    (output / 'java-identity.stdout.bin').write_bytes(identity.stdout)
    (output / 'java-identity.stderr.bin').write_bytes(identity.stderr)
    if identity.returncode != 0:
        raise ValueError('Java identity command failed; raw attempt retained')
    props = dict(re.findall('^\\s+([\\w.]+) = (.*)$', identity.stderr.decode(), re.M))
    for name, value in lock['jdk']['properties'].items():
        if props.get(name) != value:
            raise ValueError(f'Java property {name} differs; raw identity retained')
    home = Path(props['java.home'])
    jdk = {'executable': str(java), 'executable_sha256': sha(java), 'modules_sha256': sha(home / 'lib/modules'), 'release_sha256': sha(home / 'release'), 'properties': {name: props[name] for name in ['java.vendor', 'java.version', 'java.runtime.version', 'java.vm.name', 'java.vm.version', 'java.home', 'os.name', 'os.arch', 'os.version']}}
    if (jdk['executable_sha256'], jdk['modules_sha256'], jdk['release_sha256']) != (JDK_EXE_SHA256, JDK_MODULES_SHA256, JDK_RELEASE_SHA256):
        raise ValueError('Java runtime bytes differ; raw identity retained')
    m = json.loads((output / 'cases.json').read_bytes())
    if m['schema'] != 2 or m['repetitions'] != 2 or m['boundaries'] != ['string', 'export'] or (len(m['rows']) != 255):
        raise ValueError('case shape differs')
    if (output / 'queries.tsv').read_text() != ''.join((x['id'] + '\t' + x['expression'] + '\n' for x in m['rows'])):
        raise ValueError('query order differs')
    options = [*lock['jvm_options'], f"-Duser.home={env['HOME']}", f"-Djava.io.tmpdir={env['TMPDIR']}"]
    p = {'schema': 2, 'scope': 'Canonical separate Java 0.32.0 public string-query and exported-value evaluator sessions; no Rust candidate.', 'host': platform.platform(), 'oracle_lock_sha256': sha(ROOT / 'oracle.lock.json'), 'references_lock_sha256': sha(ROOT / 'references.lock'), 'reference_revision': lock['source']['revision'], 'artifact': str(jar), 'artifact_sha256': sha(jar), 'jdk': jdk, 'environment': env, 'input_before': {name: sha(output / name) for name in INPUT_NAMES}, 'source_before': {name: sha(ROOT / name) for name in SOURCE_NAMES}, 'sessions': []}

    def save():
        (output / 'provenance.json').write_text(json.dumps(p, indent=2, sort_keys=True) + '\n')
    save()
    for mode in m['boundaries']:
        for repeat in range(2):
            folder = output / mode / f'repeat-{repeat}'
            folder.mkdir(parents=True)
            cmd = [str(java), *options, '-cp', str(jar), str(output / 'DataSizeSplitOracle.java'), mode, str(folder), str(output), m['module_uri']]
            started = time.time()
            proc = subprocess.Popen(cmd, cwd=output, env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
            try:
                stdout, stderr = proc.communicate(timeout=1200)
            except subprocess.TimeoutExpired:
                proc.kill()
                stdout, stderr = proc.communicate()
                (folder / 'process.stdout.bin').write_bytes(stdout)
                (folder / 'process.stderr.bin').write_bytes(stderr)
                raise ValueError('Java session timed out; raw attempt retained')
            (folder / 'process.stdout.bin').write_bytes(stdout)
            (folder / 'process.stderr.bin').write_bytes(stderr)
            entry = {'mode': mode, 'repeat': repeat, 'pid': proc.pid, 'command': cmd, 'cwd': str(output), 'exit_code': proc.returncode, 'started_epoch': started, 'finished_epoch': time.time(), 'stdout_sha256': sha(folder / 'process.stdout.bin'), 'stderr_sha256': sha(folder / 'process.stderr.bin'), 'stdout_bytes': len(stdout), 'stderr_bytes': len(stderr), 'query_directories': sum((x.is_dir() for x in folder.iterdir()))}
            p['sessions'].append(entry)
            save()
            print(json.dumps({'mode': mode, 'repeat': repeat, 'exit_code': proc.returncode, 'rows': entry['query_directories'], 'stdout_bytes': len(stdout), 'stderr_bytes': len(stderr)}), flush=True)
            if proc.returncode != 0 or entry['query_directories'] != 255:
                raise ValueError('Java session failed; raw attempt retained')
    p['input_after'] = {name: sha(output / name) for name in INPUT_NAMES}
    p['source_after'] = {name: sha(ROOT / name) for name in SOURCE_NAMES}
    p['artifact_after_sha256'] = sha(jar)
    p['stable'] = p['input_before'] == p['input_after'] and p['source_before'] == p['source_after'] and (p['artifact_sha256'] == p['artifact_after_sha256'])
    for mode in m['boundaries']:

        def files(folder):
            return {x.relative_to(folder).as_posix(): sha(x) for x in folder.rglob('*') if x.is_file()}
        a, b = (files(output / mode / 'repeat-0'), files(output / mode / 'repeat-1'))
        p[mode + '_raw_equal'] = a == b
        p[mode + '_files'] = len(a)
        p[mode + '_raw_differences'] = [n for n in sorted(set(a) | set(b)) if a.get(n) != b.get(n)]
    p['complete'] = len(p['sessions']) == 4 and all((x['exit_code'] == 0 and x['query_directories'] == 255 for x in p['sessions'])) and p['string_raw_equal'] and p['export_raw_equal'] and p['stable']
    save()
    if not p['complete']:
        raise ValueError('Java acquisition incomplete; raw attempt retained')
    return audit_split(output, output / 'sources', write_audit=True)
if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    group = parser.add_mutually_exclusive_group(required=True)
    group.add_argument('--output', type=Path)
    group.add_argument('--audit-capture', type=Path)
    args = parser.parse_args()
    result = acquire(args.output) if args.output else audit_split(args.audit_capture, args.audit_capture / 'sources')
    print(json.dumps({'rows': result['query_rows'], 'sessions': len(result['sessions']), 'intentional_errors': result['error_count'], 'exported_datasize_rows': result['exported_datasize_rows'], 'compatibility_passes_claimed': 0}, sort_keys=True))
