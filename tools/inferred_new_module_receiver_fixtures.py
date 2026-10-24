"""Offline full-protocol verifier for inherited module receiver Java authority."""
from __future__ import annotations

import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path, PurePosixPath
import re
import tarfile

from inferred_new_fixtures import JDK_SHA256, ROW_CHANNELS, checked
from inferred_new_module_receiver_oracle import INPUT_HASHES, SOURCES, REVISION

ROOT = Path(__file__).resolve().parents[1]
FIXTURE = ROOT / 'tests/fixtures/m2-inferred-new-module-receiver'
ARCHIVE_SHA256 = '8d0e624f5474c292922064fc36f211d7050d9c9dbf11cd90cd381800dea9f7dc'
FILESET_SHA256 = '81f1601d7461c509708def1468d6cb3d079e2bd775ff18489f899bb68b8cf1d9'
ERRORS = {
    'alpha-first': {
        'beta-module-is-beta': 'Expected value of type `beta`, but got type `alpha`.',
        'beta-nullable-is-beta': 'Expected value of type `beta`, but got type `alpha`.',
        'cross-module-distinct': 'Expected value of type `beta`, but got type `alpha`.',
    },
    'beta-first': {
        'alpha-module-is-alpha': 'Expected value of type `alpha`, but got type `beta`.',
        'alpha-nullable-is-alpha': 'Expected value of type `alpha`, but got type `beta`.',
        'cross-module-distinct': 'Expected value of type `alpha`, but got type `beta`.',
    },
}


def need(ok: bool, message: str) -> None:
    if not ok:
        raise ValueError(message)


def sha(raw: bytes) -> str:
    return hashlib.sha256(raw).hexdigest()


def verify_capture(files: dict[str, bytes]) -> dict:
    prefix = 'capture-01/'
    def capture(name: str) -> bytes:
        return files[prefix + name]
    cases = json.loads(capture('cases.json'))
    report = json.loads(capture('report.json'))
    need(cases['schema'] == 1 and cases['baseline'] == 'Pkl 0.32.0'
         and cases['boundary'] == 'public-evaluateExpressionString'
         and cases['module_uri_policy'] == 'captured-absolute-file-uri-with-three-sibling-modules'
         and cases['repetitions'] == 2
         and set(cases['scenarios']) == {'alpha-first', 'beta-first'}
         and all(len(rows) == 11 and len({row['id'] for row in rows}) == 11
                 and all(set(row) == {'id','mode','expression'} and row['mode']=='string'
                         and re.fullmatch('[a-z0-9-]+',row['id']) for row in rows)
                 for rows in cases['scenarios'].values()),
         'authored scenario cases differ')
    for scenario, rows in cases['scenarios'].items():
        need(capture(scenario+'.queries.tsv') == ''.join(
            f"string\t{row['id']}\t{row['expression']}\n" for row in rows).encode(),
            'ordered scenario query bytes differ')
    need(report['schema'] == 1 and report['complete'] is True
         and report['source_stable'] is True and report['input_stable'] is True
         and report['candidate_compatibility_passes_claimed'] == 0
         and report['raw_equal'] == {'alpha-first':True,'beta-first':True}
         and report['artifact_before'] == report['artifact_after']
         == 'c3b6f6215a08b31453b0fb0edd8c9da0aee31233f65abb8ab07f9c945151e059',
         'captured completion/artifact claim differs')
    need(report['input_before'] == report['input_after']
         and set(report['input_before']) == set(INPUT_HASHES)
         and report['source_before'] == report['source_after'] == report['source_copies_after']
         and set(report['source_before']) == set(SOURCES),
         'complete input/source closure differs')
    for name, expected in INPUT_HASHES.items():
        need(sha(capture(name)) == expected == report['input_before'][name],
             f'authored captured input differs: {name}')
    for name, expected in report['source_before'].items():
        need(sha(capture('sources/'+name)) == expected,
             f'pinned source copy differs: {name}')
    need(capture('run.py') == (ROOT/'tools/inferred_new_module_receiver_oracle.py').read_bytes()
         and capture('CollectionConversionsOracle.java') ==
         (ROOT/'tools/java/CollectionConversionsOracle.java').read_bytes(),
         'tracked producer/helper closure differs')
    lock = json.loads(capture('sources/oracle.lock.json'))
    need(sha(capture('sources/oracle.lock.json')) == report['oracle_lock_sha256']
         and sha(capture('sources/references.lock')) == report['references_lock_sha256']
         and lock['version'] == '0.32.0' and lock['source']['revision'] == REVISION
         and lock['artifact']['sha256'] == report['artifact_before']
         and ('pkl.revision='+REVISION).encode() in capture('sources/references.lock'),
         'pinned lock identity differs')
    jdk = report['jdk']
    stderr = capture('java-identity.stderr.bin')
    properties = dict(re.findall(r'^\s+([\w.]+) = (.*)$', stderr.decode(), re.M))
    need(capture('java-identity.stdout.bin') == b''
         and properties == jdk['properties']
         and b'openjdk version "21.0.12.1"' in stderr
         and (jdk['executable_sha256'], jdk['modules_sha256'], jdk['release_sha256'])
         == JDK_SHA256
         and all(properties.get(k)==v for k,v in lock['jdk']['properties'].items()),
         'raw JDK identity differs')
    sessions = report['sessions']
    need(len(sessions)==4 and [(r['scenario'],r['repeat']) for r in sessions] ==
         [('alpha-first',0),('alpha-first',1),('beta-first',0),('beta-first',1)]
         and len({r['pid'] for r in sessions})==4
         and all(r['pid']>0 and r['started_epoch']<r['finished_epoch'] for r in sessions)
         and all(a['finished_epoch']<=b['started_epoch'] for a,b in zip(sessions,sessions[1:])),
         'four fresh ordered JVMs absent')
    expected_files = {prefix+n for n in (
        {'run.py','CollectionConversionsOracle.java','report.json',
         'java-identity.stdout.bin','java-identity.stderr.bin',*INPUT_HASHES})}
    expected_files |= {prefix+'sources/'+name for name in SOURCES}
    expected_observations = {}
    repetitions = {}
    status_total = Counter()
    for session in sessions:
        scenario,repeat=session['scenario'],session['repeat']
        cwd=Path(session['cwd'])
        source=cwd/scenario
        uri=(source/'module.pkl').as_uri()
        argv=[jdk['executable'],*lock['jvm_options'],
              f"-Duser.home={report['environment']['HOME']}",
              f"-Djava.io.tmpdir={report['environment']['TMPDIR']}",'-cp',
              str(Path(report['environment']['HOME']).parent / lock['artifact']['filename']),
              str(cwd/'CollectionConversionsOracle.java'),
              str(source/f'repeat-{repeat}'),str(source),uri]
        need(cwd.is_absolute() and session['argv']==argv and session['module_uri']==uri
             and session['cwd']==str(cwd) and session['exit_code']==0
             and session['timed_out'] is False and session['row_directories']==11,
             'session argv/URI/exit differs')
        for name in ('base.pkl','alpha.pkl','beta.pkl','module.pkl'):
            need(capture(scenario+'/'+name)==capture(name),
                 'real sibling module source differs')
            expected_files.add(prefix+scenario+'/'+name)
        need(capture(scenario+'/queries.tsv')==capture(scenario+'.queries.tsv'),
             'scenario query copy differs')
        expected_files.add(prefix+scenario+'/queries.tsv')
        base=prefix+scenario+f'/repeat-{repeat}/'
        for channel in ('stdout','stderr'):
            raw=files[base+'process.'+channel+'.bin']
            need(sha(raw)==session[channel+'_sha256']
                 and len(raw)==session[channel+'_bytes'],
                 'raw JVM process stream differs')
        need(files[base+'process.stderr.bin']==b'', 'Java process stderr differs')
        protocol=['session-open mode=string rows=11']
        counts=Counter()
        observed=[]
        for index,row in enumerate(cases['scenarios'][scenario]):
            folder=base+f"{index:03d}-{row['id']}/"
            actual={name[len(folder):] for name in files if name.startswith(folder)}
            need(actual==ROW_CHANNELS, 'raw 21-channel row set differs')
            need(checked(files,folder,'mode')=='string'
                 and checked(files,folder,'expression')==row['expression'],
                 'raw expression echo differs')
            values={name:checked(files,folder,'result.'+name) for name in
                    ('stdout','stderr','error-class','exception-to-string','cause-class',
                     'cause-message','cause-to-string','java-type')}
            status=files[folder+'result.status.bin'].decode()
            first=values['stderr'].splitlines()
            first_line=first[1] if len(first)>1 else ''
            if row['id'] in ERRORS[scenario]:
                need(status=='error\n' and first_line==ERRORS[scenario][row['id']]
                     and values['error-class']=='org.pkl.core.PklException'
                     and values['stdout']==values['java-type']=='',
                     'expected order-specific ordinary error differs')
            else:
                need(status=='success\n' and all(not values[name] for name in values
                                                   if name!='stdout'),
                     'expected order-specific success differs')
            counts[status.strip()]+=1
            call=f"string-{index}-{row['id']}"
            protocol.extend((f'call-enter={call}',f'call-exit={call} status={status.strip()}'))
            if repeat==0:
                observed.append({'id':row['id'],'expression':row['expression'],
                                 'status':status.strip(),'stdout':values['stdout'],
                                 'first_error_line':first_line,
                                 'stdout_sha256':sha(files[folder+'result.stdout.bin']),
                                 'stderr_sha256':sha(files[folder+'result.stderr.bin'])})
            expected_files |= {folder+name for name in ROW_CHANNELS}
        protocol.append(f"session-closed mode=string rows=11 public-calls=11 "
                        f"success={counts['success']} errors={counts['error']}")
        need(files[base+'process.stdout.bin']==('\n'.join(protocol)+'\n').encode(),
             'complete process protocol differs')
        expected_files |= {base+'process.stdout.bin',base+'process.stderr.bin'}
        raw_set={name[len(base):]:sha(raw) for name,raw in files.items()
                 if name.startswith(base)}
        need(len(raw_set)==2+21*11 and counts=={'success':8,'error':3},
             'session raw count/status differs')
        repetitions.setdefault(scenario,[]).append(raw_set)
        if repeat==0:
            expected_observations[scenario]=observed
            status_total.update(counts)
    need(all(pair[0]==pair[1] for pair in repetitions.values()),
         'raw repetitions differ')
    need(report['observations']==expected_observations
         and report['statuses_per_repetition']==dict(status_total)
         =={'success':16,'error':6},
         'derived observations/statuses differ')
    need(set(files)==expected_files, 'whole capture file set differs')
    return {'scenarios':2,'rows_per_scenario':11,'sessions':4,
            'statuses_per_repetition':dict(status_total),'source_copies':len(SOURCES)}


def verify(fixture: Path=FIXTURE) -> dict:
    root=Path(fixture)
    need(root.is_dir() and not root.is_symlink(), 'fixture root missing/symlinked')
    names={'README.md','archive-identity.json','oracle-evidence.tar.gz','manifest.json',
           *INPUT_HASHES}
    entries=list(root.iterdir())
    need(all(p.is_file() and not p.is_symlink() for p in entries)
         and {p.name for p in entries}==names, 'fixture root file set differs')
    root_files={p.name:sha(p.read_bytes()) for p in entries if p.name!='manifest.json'}
    manifest=json.loads((root/'manifest.json').read_bytes())
    need(manifest=={'schema':1,'baseline':'Pkl 0.32.0','files':root_files},
         'fixture manifest differs')
    identity=json.loads((root/'archive-identity.json').read_bytes())
    need(identity['schema']==1 and identity['archive_sha256']==ARCHIVE_SHA256
         and identity['fileset_sha256']==FILESET_SHA256
         and root_files['oracle-evidence.tar.gz']==ARCHIVE_SHA256
         and identity['captures']=={'capture-01':'stage65-module-receiver-oracle-01'},
         'fixed archive identity differs')
    files={}
    file_set=hashlib.sha256()
    with tarfile.open(root/'oracle-evidence.tar.gz','r:gz') as stream:
        for member in stream:
            path=PurePosixPath(member.name)
            need(member.isfile() and not member.issym() and not member.islnk()
                 and not path.is_absolute() and '..' not in path.parts
                 and member.name.startswith('capture-01/') and member.name not in files,
                 'unsafe or duplicate archive member')
            handle=stream.extractfile(member)
            need(handle is not None, 'unreadable archive member')
            raw=handle.read()
            need(len(raw)==member.size, 'archive member size differs')
            files[member.name]=raw
            file_set.update(member.name.encode()+b'\0'+sha(raw).encode()+b'\n')
    need(len(files)==identity['members'] and file_set.hexdigest()==FILESET_SHA256,
         'full archive file set differs')
    for name in INPUT_HASHES:
        need(files['capture-01/'+name]==(root/name).read_bytes(),
             'fixture authored input differs')
    return {'baseline':'Pkl 0.32.0',**verify_capture(files),
            'archive_sha256':ARCHIVE_SHA256,'fileset_sha256':FILESET_SHA256,
            'candidate_compatibility_passes_claimed':0}


if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--fixture',type=Path,default=FIXTURE)
    args=parser.parse_args()
    print(json.dumps(verify(args.fixture),indent=2,sort_keys=True))
