"""Verify all raw Java channels and provenance for inferred Listing defaults."""
from __future__ import annotations

import hashlib
import json
from pathlib import Path, PurePosixPath
import re
import tarfile

from inferred_new_fixtures import JDK_SHA256, ROW_CHANNELS, checked
from inferred_new_default_diagnostics_oracle import SOURCES, REVISION

ROOT=Path(__file__).resolve().parents[1]
FIXTURE=ROOT/'tests/fixtures/m2-inferred-new-default-diagnostics'
ARCHIVE_SHA256='d15f914ffb2a72fce86862ab30fea2b087644be0594303b1f705a8f801a191fc'
FILESET_SHA256='8cb85d9194872c5a37a92fd3f85689c86ac9431b9398a09586383e05f9dbdd3e'
REPORT_SHA256='516edd5927a371b978feceb785264c3a54f5a2e2e04825c26009c53a566e734f'
LABELS=('literal-unknown','any','alias-unknown','generic-alias-unknown')
ERROR='Tried to read property `default` but its value is undefined.'


def need(ok:bool,message:str)->None:
    if not ok:raise ValueError(message)


def sha(raw:bytes)->str:
    return hashlib.sha256(raw).hexdigest()


def verify_capture(files:dict[str,bytes])->dict:
    prefix='capture-01/'
    def get(name:str)->bytes:return files[prefix+name]
    report=json.loads(get('report.json'))
    cases=json.loads(get('inputs/cases.json'))
    need(sha(get('report.json'))==REPORT_SHA256,'capture report identity differs')
    need(cases=={'schema':1,'baseline':'Pkl 0.32.0','cases':list(LABELS)},
         'four authored programs differ')
    need(report['schema']==1 and report['complete'] is True
         and report['source_stable'] is True and report['input_stable'] is True
         and report['raw_equal'] is True and report['candidate_compatibility_passes_claimed']==0
         and report['artifact_before']==report['artifact_after']
         =='c3b6f6215a08b31453b0fb0edd8c9da0aee31233f65abb8ab07f9c945151e059'
         and report['input_before']==report['input_after']
         and report['source_before']==report['source_after']==report['source_copies_after']
         and set(report['source_before'])==set(SOURCES),
         'complete/source/input claim differs')
    need(get('run.py')==get('sources/tools/inferred_new_default_diagnostics_oracle.py')
         and get('CollectionConversionsOracle.java')==
         get('sources/tools/java/CollectionConversionsOracle.java'),
         'producer/Java helper bytes differ')
    expected={prefix+name for name in ('report.json','run.py','CollectionConversionsOracle.java',
                                      'java-identity.stdout.bin','java-identity.stderr.bin')}
    for name,identity in report['input_before'].items():
        need(sha(get('inputs/'+name))==identity
             and get('inputs/'+name)==(FIXTURE/name).read_bytes(),
             'authored input bytes differ')
        expected.add(prefix+'inputs/'+name)
    for name,identity in report['source_before'].items():
        need(sha(get('sources/'+name))==identity
             and identity==sha((ROOT/name).read_bytes()),
             'pinned source copy differs')
        expected.add(prefix+'sources/'+name)
    lock=json.loads(get('sources/oracle.lock.json'))
    need(sha(get('sources/oracle.lock.json'))==report['oracle_lock_sha256']
         and sha(get('sources/references.lock'))==report['references_lock_sha256']
         and lock['version']=='0.32.0' and lock['source']['revision']==REVISION
         and lock['artifact']['sha256']==report['artifact_before'],
         'pinned Java lock differs')
    jdk=report['jdk']
    stderr=get('java-identity.stderr.bin')
    properties=dict(re.findall(r'^\s+([\w.]+) = (.*)$',stderr.decode(),re.M))
    need(get('java-identity.stdout.bin')==b'' and properties==jdk['properties']
         and b'openjdk version "21.0.12.1"' in stderr
         and (jdk['executable_sha256'],jdk['modules_sha256'],jdk['release_sha256'])==JDK_SHA256
         and all(properties.get(k)==v for k,v in lock['jdk']['properties'].items()),
         'raw pinned JDK identity differs')
    sessions=report['sessions']
    need(len(sessions)==8 and [(s['case'],s['repeat']) for s in sessions]==
         [(label,repeat) for label in LABELS for repeat in (0,1)]
         and len({s['pid'] for s in sessions})==8
         and all(s['pid']>0 and s['started_epoch']<s['finished_epoch'] for s in sessions)
         and all(a['finished_epoch']<=b['started_epoch'] for a,b in zip(sessions,sessions[1:])),
         'eight fresh ordered JVMs differ')
    observations={}
    for session in sessions:
        label,repeat=session['case'],session['repeat']
        source=Path(session['cwd'])/'inputs'/label
        uri=(source/'module.pkl').as_uri()
        argv=[jdk['executable'],*lock['jvm_options'],
              f"-Duser.home={report['environment']['HOME']}",
              f"-Djava.io.tmpdir={report['environment']['TMPDIR']}",'-cp',
              str(Path(report['environment']['HOME']).parent/lock['artifact']['filename']),
              str(Path(session['cwd'])/'CollectionConversionsOracle.java'),
              str(source/f'repeat-{repeat}'),str(source),uri]
        need(Path(session['cwd']).is_absolute() and session['argv']==argv
             and session['module_uri']==uri and session['exit_code']==0
             and session['timed_out'] is False and session['row_directories']==1,
             'Java process argv/URI/exit differs')
        base=prefix+'inputs/'+label+f'/repeat-{repeat}/'
        out=get('inputs/'+label+f'/repeat-{repeat}/process.stdout.bin')
        err=get('inputs/'+label+f'/repeat-{repeat}/process.stderr.bin')
        need(sha(out)==session['stdout_sha256'] and sha(err)==session['stderr_sha256']
             and err==b'', 'raw Java process stream differs')
        expected|={base+'process.stdout.bin',base+'process.stderr.bin'}
        folder=base+'000-result/'
        actual={name[len(folder):] for name in files if name.startswith(folder)}
        need(actual==ROW_CHANNELS,'raw 21-channel row set differs')
        need(checked(files,folder,'mode')=='string'
             and checked(files,folder,'expression')=='result',
             'raw query/UTF-16 echo differs')
        values={name:checked(files,folder,'result.'+name) for name in
                ('stdout','stderr','error-class','exception-to-string','cause-class',
                 'cause-message','cause-to-string','java-type')}
        status=files[folder+'result.status.bin'].decode()
        wanted='error' if label in ('any','alias-unknown') else 'success'
        first=values['stderr'].splitlines()
        actual_first=first[1] if len(first)>1 else ''
        need(status==wanted+'\n' and values['stdout']==('' if wanted=='error' else 'true')
             and actual_first==(ERROR if wanted=='error' else ''),
             'ordinary error/success boundary differs')
        if wanted=='success':
            need(all(not value for key,value in values.items() if key!='stdout'),
                 'successful producer metadata differs')
        else:
            frames=re.findall(r'^at (.*)$',values['stderr'],re.M)
            need(values['error-class']=='org.pkl.core.PklException'
                 and values['java-type']=='' and frames[:3]==[
                     'module#make.default ('+uri+')',
                     'module#make[#1] ('+uri+')',
                     'module#result ('+uri+')'] and frames[3:]==[' (repl:text)']
                 and f'function make(): Listing<{("Any" if label=="any" else "U")}>'
                 in values['stderr'], 'complete undefined-default frames differ')
        protocol=('session-open mode=string rows=1\n'
                  'call-enter=string-0-result\n'
                  f'call-exit=string-0-result status={wanted}\n'
                  f'session-closed mode=string rows=1 public-calls=1 '
                  f'success={int(wanted=="success")} errors={int(wanted=="error")}\n')
        need(out==protocol.encode(), 'whole Java process protocol differs')
        expected|={folder+name for name in ROW_CHANNELS}
        observation={'repeat':repeat,'status':wanted,'stdout':values['stdout'],
                     'stderr_sha256':sha(files[folder+'result.stderr.bin']),
                     'first_error_line':actual_first}
        observations.setdefault(label,[]).append(observation)
        if repeat==1:
            other=prefix+'inputs/'+label+'/repeat-0/'
            old={name[len(other):]:sha(body) for name,body in files.items()
                 if name.startswith(other)}
            now={name[len(base):]:sha(body) for name,body in files.items()
                 if name.startswith(base)}
            need(old==now and len(now)==23,'two raw JVM repetitions differ')
    need(report['observations']==observations,'derived raw observations differ')
    need(set(files)==expected,'whole capture file set differs')
    return {'programs':4,'fresh_jvms':8,'statuses_per_repetition':{'success':2,'error':2},
            'source_copies':len(SOURCES)}


def verify(fixture:Path=FIXTURE)->dict:
    root=Path(fixture)
    need(root.is_dir() and not root.is_symlink(),'fixture root missing/symlinked')
    expected_root={'README.md','cases.json','archive-identity.json',
                   'oracle-evidence.tar.gz','manifest.json'}
    expected_root|={label+'/'+name for label in LABELS for name in ('module.pkl','queries.tsv')}
    paths=[p for p in root.rglob('*') if p.is_file()]
    need(all(not p.is_symlink() for p in root.rglob('*'))
         and {p.relative_to(root).as_posix() for p in paths}==expected_root,
         'fixture root file set differs')
    root_files={p.relative_to(root).as_posix():sha(p.read_bytes()) for p in paths
                if p.name!='manifest.json'}
    need(json.loads((root/'manifest.json').read_bytes())==
         {'schema':1,'baseline':'Pkl 0.32.0','files':root_files},
         'fixture manifest differs')
    identity=json.loads((root/'archive-identity.json').read_bytes())
    need(identity['schema']==1 and identity['archive_sha256']==ARCHIVE_SHA256
         and identity['fileset_sha256']==FILESET_SHA256
         and identity['captures']=={'capture-01':'stage65-default-diagnostics-oracle-01'}
         and root_files['oracle-evidence.tar.gz']==ARCHIVE_SHA256
         and identity['candidate_compatibility_passes_claimed']==0,
         'fixed archive identity differs')
    files={};fileset=hashlib.sha256()
    with tarfile.open(root/'oracle-evidence.tar.gz','r:gz') as stream:
        for member in stream:
            path=PurePosixPath(member.name)
            need(member.isfile() and not member.issym() and not member.islnk()
                 and not path.is_absolute() and '..' not in path.parts
                 and path.parts[0]=='capture-01' and member.name not in files,
                 'unsafe or duplicate archive member')
            reader=stream.extractfile(member)
            need(reader is not None,'unreadable archive member')
            body=reader.read()
            need(len(body)==member.size,'archive member size differs')
            files[member.name]=body
            fileset.update(member.name.encode()+b'\0'+sha(body).encode()+b'\n')
    need(len(files)==identity['members'] and fileset.hexdigest()==FILESET_SHA256,
         'full archive file set differs')
    return {'baseline':'Pkl 0.32.0',**verify_capture(files),
            'archive_sha256':ARCHIVE_SHA256,'fileset_sha256':FILESET_SHA256,
            'candidate_compatibility_passes_claimed':0}


if __name__=='__main__':
    print(json.dumps(verify(),indent=2,sort_keys=True))
