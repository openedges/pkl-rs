"""Verify complete Java method-parent cache captures and oracle-bug history offline."""
from __future__ import annotations

import hashlib
import json
from pathlib import Path, PurePosixPath
import re
import tarfile

from inferred_new_fixtures import JDK_SHA256, ROW_CHANNELS, checked
from inferred_new_method_cache_oracle import INPUT_HASHES
from inferred_new_module_receiver_oracle import SOURCES as ENGINE_SOURCES, REVISION

ROOT = Path(__file__).resolve().parents[1]
FIXTURE = ROOT / 'tests/fixtures/m2-inferred-new-method-cache'
ARCHIVE_SHA256 = '82b0f357830033bf61be139ecc3b0ebc7570893427b9faeb958d0851128ab6ec'
FILESET_SHA256 = '0b1dc72d6ce1b0e7de36db8b9ba7d207a7c14925415afc3173db335cc7eabbc3'
CAPTURES = {
    'method-capture-01': 'target/m2/stage65-inferred-method-cache-method-oracle-01',
    'object-method-capture-01': 'target/m2/stage65-inferred-method-cache-object-method-oracle-01',
    'object-method-alias-bug-history-01':
        'target/m2/stage65-inferred-object-method-exploration-01/capture-01',
}
REPORT_SHA256 = {
    'method-capture-01': '9eed2ce4d3d141428dee7015b4f4ae1ec007bf01092258e8d1fa81531afaa0fc',
    'object-method-capture-01': 'a9d8e916a2d29318d47709d870bda14e40dcec6937fbfa3743e12e324008c6dd',
    'object-method-alias-bug-history-01': 'b5d1938662577cb6dc215b52053ebcb1e395ac82d193b6c65e2a997f75a63f76',
}
HISTORY = 'object-method-alias-bug-history-01'
PROFILE = {'method-capture-01': 'method', 'object-method-capture-01': 'object-method'}
SOURCES = set(ENGINE_SOURCES) | {'tools/inferred_new_method_cache_oracle.py'}
HISTORY_SOURCES = set(ENGINE_SOURCES) | {
    'target/m2/stage65-inferred-object-method-exploration-01/prepare.py',
    'target/m2/stage65-inferred-object-method-exploration-01/experiment.py',
}


def need(ok: bool, message: str) -> None:
    if not ok:
        raise ValueError(message)


def sha(raw: bytes) -> str:
    return hashlib.sha256(raw).hexdigest()


def expected(profile: str, scenario: str, row_id: str) -> tuple[str, str, str]:
    first = scenario.split('-')[0]
    second = 'beta' if first == 'alpha' else 'alpha'
    if row_id == 'explicit-alias-second':
        return ('success', 'true', '')
    if row_id == 'module-distinct':
        return ('success', 'false', '')
    if row_id == 'second-marker':
        return ('success', second, '')
    if profile == 'method':
        if row_id in {f'module-{second}', f'nullable-{second}'}:
            return ('error', '', f'Expected value of type `{second}`, but got type `{first}`.')
        if row_id.startswith(('alias-', 'nullable-alias-')):
            return ('success', 'true' if row_id.endswith(first) else 'false', '')
        if row_id.startswith(('module-', 'nullable-')):
            return ('success', 'true', '')
    elif profile == 'object-method':
        if row_id.startswith(('module-', 'nullable-')) and row_id.endswith('-class'):
            return ('success', 'true' if f'-{first}-' in row_id else 'false', '')
        if row_id.startswith('module-') and row_id.endswith('-marker'):
            return ('success', 'object-module', '')
        if row_id.startswith('nullable-') and row_id.endswith('-marker'):
            return ('success', 'object-nullable', '')
    elif profile == 'oracle-bug':
        if row_id.startswith(('alias-', 'nullablealias-')):
            return ('oracle-error', '', 'Cmd+Double-click the link below to open an issue.')
        if row_id.startswith(('module-', 'nullable-')):
            return ('success', 'true' if row_id.endswith(first) else 'false', '')
    raise ValueError(f'unexpected authored row: {profile}/{scenario}/{row_id}')


def verify_capture(files: dict[str, bytes], label: str) -> dict:
    need(label in CAPTURES, 'unknown capture label')
    prefix = label + '/'
    def get(name: str) -> bytes:
        return files[prefix + name]
    report = json.loads(get('report.json'))
    cases = json.loads(get('cases.json'))
    profile = 'oracle-bug' if label == HISTORY else PROFILE[label]
    need(sha(get('report.json')) == REPORT_SHA256[label], 'captured report identity differs')
    need(cases['schema'] == 1 and cases['baseline'] == 'Pkl 0.32.0'
         and cases['boundary'] == 'public-evaluateExpressionString'
         and cases['repetitions'] == 2
         and cases['module_uri_policy'] == 'captured-absolute-file-uri-with-three-sibling-modules'
         and set(cases['scenarios']) == {'alpha-first', 'beta-first'}
         and all(len(rows) == 11 and len({r['id'] for r in rows}) == 11
                 and all(set(r) == {'id', 'mode', 'expression'} and r['mode'] == 'string'
                         and re.fullmatch('[a-z0-9-]+', r['id']) for r in rows)
                 for rows in cases['scenarios'].values()), 'authored cases differ')
    for scenario, rows in cases['scenarios'].items():
        need(get(scenario + '.queries.tsv') == ''.join(
            f"string\t{r['id']}\t{r['expression']}\n" for r in rows).encode(),
            'ordered query bytes differ')
    source_names = HISTORY_SOURCES if label == HISTORY else SOURCES
    input_hashes = report['input_before'] if label == HISTORY else INPUT_HASHES[profile]
    need(report['schema'] == 1 and report['candidate_compatibility_passes_claimed'] == 0
         and report['complete'] == (label != HISTORY)
         and report['source_stable'] is True and report['input_stable'] is True
         and report['raw_equal'] == {'alpha-first': True, 'beta-first': True}
         and report['artifact_before'] == report['artifact_after']
         == 'c3b6f6215a08b31453b0fb0edd8c9da0aee31233f65abb8ab07f9c945151e059'
         and report['input_before'] == report['input_after'] == input_hashes
         and report['source_before'] == report['source_after'] == report['source_copies_after']
         and set(report['source_before']) == source_names,
         'completion/source/input boundary differs')
    for name, identity in input_hashes.items():
        need(sha(get(name)) == identity, 'captured authored input differs')
        if label != HISTORY:
            need(get(name) == (FIXTURE / profile / name).read_bytes(),
                 'fixture authored input differs')
    for name, identity in report['source_before'].items():
        need(sha(get('sources/' + name)) == identity,
             'pinned source copy differs')
        if label != HISTORY:
            need(identity == sha((ROOT / name).read_bytes()), 'current producer source differs')
    need(get('run.py') == get('sources/tools/inferred_new_module_receiver_oracle.py')
         and get('CollectionConversionsOracle.java') ==
         get('sources/tools/java/CollectionConversionsOracle.java'),
         'producer/Java helper copy differs')
    lock = json.loads(get('sources/oracle.lock.json'))
    need(sha(get('sources/oracle.lock.json')) == report['oracle_lock_sha256']
         and sha(get('sources/references.lock')) == report['references_lock_sha256']
         and lock['version'] == '0.32.0' and lock['source']['revision'] == REVISION
         and lock['artifact']['sha256'] == report['artifact_before'],
         'pinned Java lock differs')
    jdk = report['jdk']
    stderr = get('java-identity.stderr.bin')
    properties = dict(re.findall(r'^\s+([\w.]+) = (.*)$', stderr.decode(), re.M))
    need(get('java-identity.stdout.bin') == b'' and properties == jdk['properties']
         and b'openjdk version "21.0.12.1"' in stderr
         and (jdk['executable_sha256'], jdk['modules_sha256'], jdk['release_sha256']) == JDK_SHA256
         and all(properties.get(k) == v for k,v in lock['jdk']['properties'].items()),
         'raw pinned JDK identity differs')
    sessions = report['sessions']
    need(len(sessions) == 4 and [(s['scenario'], s['repeat']) for s in sessions] ==
         [('alpha-first',0),('alpha-first',1),('beta-first',0),('beta-first',1)]
         and len({s['pid'] for s in sessions}) == 4
         and all(s['pid'] > 0 and s['started_epoch'] < s['finished_epoch'] for s in sessions)
         and all(a['finished_epoch'] <= b['started_epoch'] for a,b in zip(sessions,sessions[1:])),
         'fresh ordered Java sessions differ')
    expected_files = {prefix + name for name in {
        'report.json','run.py','CollectionConversionsOracle.java','java-identity.stdout.bin',
        'java-identity.stderr.bin',*input_hashes}}
    expected_files |= {prefix+'sources/'+name for name in source_names}
    observations = {}
    totals = {'success':0,'error':0,'oracle-error':0}
    for session in sessions:
        scenario,repeat = session['scenario'],session['repeat']
        cwd = Path(session['cwd'])
        source = cwd / scenario
        uri = (source/'module.pkl').as_uri()
        argv = [jdk['executable'],*lock['jvm_options'],
                f"-Duser.home={report['environment']['HOME']}",
                f"-Djava.io.tmpdir={report['environment']['TMPDIR']}",'-cp',
                str(Path(report['environment']['HOME']).parent / lock['artifact']['filename']),
                str(cwd/'CollectionConversionsOracle.java'),str(source/f'repeat-{repeat}'),
                str(source),uri]
        need(cwd.is_absolute() and session['cwd'] == str(cwd) and session['argv'] == argv
             and session['module_uri'] == uri and session['exit_code'] == 0
             and session['timed_out'] is False and session['row_directories'] == 11,
             'Java process argv/URI/exit differs')
        for name in ('base.pkl','alpha.pkl','beta.pkl','module.pkl'):
            need(get(scenario+'/'+name) == get(name), 'sibling module copy differs')
            expected_files.add(prefix+scenario+'/'+name)
        need(get(scenario+'/queries.tsv') == get(scenario+'.queries.tsv'),
             'scenario query copy differs')
        expected_files.add(prefix+scenario+'/queries.tsv')
        base = prefix+scenario+f'/repeat-{repeat}/'
        process_stdout = files[base+'process.stdout.bin']
        process_stderr = files[base+'process.stderr.bin']
        need(sha(process_stdout) == session['stdout_sha256']
             and len(process_stdout) == session['stdout_bytes']
             and sha(process_stderr) == session['stderr_sha256']
             and len(process_stderr) == session['stderr_bytes']
             and process_stderr == b'', 'raw process streams differ')
        protocol = ['session-open mode=string rows=11']
        counts = {'success':0,'error':0,'oracle-error':0}
        first_rows = []
        for index,row in enumerate(cases['scenarios'][scenario]):
            folder = base+f"{index:03d}-{row['id']}/"
            actual = {name[len(folder):] for name in files if name.startswith(folder)}
            need(actual == ROW_CHANNELS, 'complete 21-channel row set differs')
            need(checked(files,folder,'mode') == 'string'
                 and checked(files,folder,'expression') == row['expression'],
                 'raw expression/UTF-16 transport differs')
            values = {name:checked(files,folder,'result.'+name) for name in
                      ('stdout','stderr','error-class','exception-to-string','cause-class',
                       'cause-message','cause-to-string','java-type')}
            status = files[folder+'result.status.bin'].decode()
            wanted,stdout,first = expected(profile,scenario,row['id'])
            lines = values['stderr'].splitlines()
            actual_first = lines[1] if len(lines) > 1 else ''
            need(status == wanted+'\n' and values['stdout'] == stdout
                 and actual_first == first, 'order-specific raw observation differs')
            if wanted == 'success':
                need(all(not value for key,value in values.items() if key != 'stdout'),
                     'success producer metadata differs')
            elif wanted == 'error':
                need(values['error-class'] == 'org.pkl.core.PklException'
                     and values['java-type'] == '', 'ordinary Pkl error class differs')
            else:
                need(values['error-class'] == 'org.pkl.core.PklBugException'
                     and values['cause-class'] == 'org.pkl.core.runtime.VmBugException'
                     and values['cause-message'] ==
                     'Cannot invoke "org.pkl.core.runtime.VmObjectLike.isParseTimeInvisibleScope()" because "owner" is null',
                     'preserved Java oracle bug metadata differs')
            counts[wanted] += 1
            if repeat == 0:
                first_rows.append({'id':row['id'],'expression':row['expression'],
                                   'status':wanted,'stdout':stdout,'first_error_line':first,
                                   'stdout_sha256':sha(files[folder+'result.stdout.bin']),
                                   'stderr_sha256':sha(files[folder+'result.stderr.bin'])})
            call=f"string-{index}-{row['id']}"
            protocol += [f'call-enter={call}',f'call-exit={call} status={wanted}']
            expected_files |= {folder+name for name in ROW_CHANNELS}
        protocol.append(f"session-closed mode=string rows=11 public-calls=11 "
                        f"success={counts['success']} errors={counts['error'] + counts['oracle-error']}")
        need(process_stdout == ('\n'.join(protocol)+'\n').encode(),
             'whole Java process protocol differs')
        expected_files |= {base+'process.stdout.bin',base+'process.stderr.bin'}
        raw = {name[len(base):]:sha(body) for name,body in files.items() if name.startswith(base)}
        need(len(raw) == 2 + 21*11, 'whole session raw file count differs')
        if repeat == 0:
            observations[scenario] = first_rows
            for key,value in counts.items():totals[key] += value
        else:
            other = prefix+scenario+'/repeat-0/'
            first_map = {name[len(other):]:sha(body) for name,body in files.items()
                         if name.startswith(other)}
            need(first_map == raw, 'two Java repetitions differ byte-for-byte')
    need(report['observations'] == observations
         and report['statuses_per_repetition'] == {k:v for k,v in totals.items() if v}
         and totals == ({'success':14,'error':0,'oracle-error':8} if label == HISTORY
                        else {'method-capture-01':{'success':18,'error':4,'oracle-error':0},
                              'object-method-capture-01':{'success':22,'error':0,'oracle-error':0}}[label]),
         'derived status/observation report differs')
    need(set(files) & {name for name in files if name.startswith(prefix)} == expected_files,
         'whole capture file set differs')
    return {'sessions':4,'rows_per_scenario':11,
            'statuses_per_repetition':{k:v for k,v in totals.items() if v},
            'source_copies':len(source_names)}


def verify(fixture: Path = FIXTURE) -> dict:
    root = Path(fixture)
    need(root.is_dir() and not root.is_symlink(), 'fixture root missing/symlinked')
    expected_root = {'README.md','archive-identity.json','oracle-evidence.tar.gz','manifest.json'}
    for profile in ('method','object-method'):
        expected_root |= {profile+'/'+name for name in INPUT_HASHES[profile]}
    paths = [p for p in root.rglob('*') if p.is_file()]
    need(all(not p.is_symlink() for p in root.rglob('*'))
         and {p.relative_to(root).as_posix() for p in paths} == expected_root,
         'fixture root file set differs')
    root_files = {p.relative_to(root).as_posix():sha(p.read_bytes()) for p in paths
                  if p.name != 'manifest.json'}
    manifest = json.loads((root/'manifest.json').read_bytes())
    need(manifest == {'schema':1,'baseline':'Pkl 0.32.0','files':root_files},
         'fixture manifest differs')
    identity = json.loads((root/'archive-identity.json').read_bytes())
    need(identity['schema'] == 1 and identity['archive_sha256'] == ARCHIVE_SHA256
         and identity['fileset_sha256'] == FILESET_SHA256
         and identity['captures'] == CAPTURES
         and root_files['oracle-evidence.tar.gz'] == ARCHIVE_SHA256
         and identity['candidate_compatibility_passes_claimed'] == 0,
         'fixed archive identity differs')
    files = {};fileset = hashlib.sha256()
    with tarfile.open(root/'oracle-evidence.tar.gz','r:gz') as stream:
        for member in stream:
            path = PurePosixPath(member.name)
            need(member.isfile() and not member.issym() and not member.islnk()
                 and not path.is_absolute() and '..' not in path.parts
                 and path.parts[0] in CAPTURES and member.name not in files,
                 'unsafe or duplicate archive member')
            reader = stream.extractfile(member)
            need(reader is not None, 'unreadable archive member')
            body = reader.read()
            need(len(body) == member.size, 'archive member size differs')
            files[member.name] = body
            fileset.update(member.name.encode()+b'\0'+sha(body).encode()+b'\n')
    need(len(files) == identity['members'] and fileset.hexdigest() == FILESET_SHA256,
         'full archive file set differs')
    results = {label:verify_capture(files,label) for label in CAPTURES}
    return {'baseline':'Pkl 0.32.0','captures':results,
            'archive_sha256':ARCHIVE_SHA256,'fileset_sha256':FILESET_SHA256,
            'candidate_compatibility_passes_claimed':0}


if __name__ == '__main__':
    print(json.dumps(verify(),indent=2,sort_keys=True))
