"""Offline raw Java 0.32.0 DataSize/Bytes additive authority, separate from A."""
from __future__ import annotations
from collections import Counter
import argparse
import hashlib
import json
from pathlib import Path
import re
import tarfile
import tempfile

DEFAULT = Path(__file__).resolve().parents[1] / 'tests/fixtures/m2-data-size-supplement'
KIND = 'pkl-java-data-size-supplement-v1'
ARCHIVE_SHA256 = 'ab8c59f0a42611aa8325d70ab2c1cc83ec59f19d12a848ee816829acbed6b6ad'
FILESET_SHA256 = 'f50819e7fd8bf79c444f65cc22146d5c4183368874d2f26104cb2eb6173e54b5'
JAR_SHA256 = 'c3b6f6215a08b31453b0fb0edd8c9da0aee31233f65abb8ab07f9c945151e059'
JDK_SHA256 = ('51ec0764f7225b40d5b286b01401aa6b3990d0d171b446d7dc81ca7542b45552',
              '40fcf7a32bada7e299e3513939a4472d27f8f61eb81cdd1675ea00648de7e7cf',
              'ab66b8afeea98b25f1869fbe7c0dde368d448e7cc0f7bab0359b322ade134fe1')
PROBES = {'split':'a4389bb16f2cc784288dd79859afb2991ac574ac84f63319ad56c583f91cabbb',
          'identity':'42a4988031b43667e916126ec662dfcacec4c0fb4f309f5d520946551bccc2f7'}
CAPTURES = {
 '04':(2928,44,'EEEEEESSEESSSSSESEESESSSSEEEEEEEESSSSSSSSSSS',398,1061),
 '05':(2934,44,'EEEEEESSEESSSSSESEESESSSSEEEEEEEESSSSSSSSSSS',398,1061),
 '06':(471,8,'SSSSSEES',226,0),
 '07':(444,7,'EESSSEE',65,149),
 '08':(444,7,'SSESSEE',65,149),
 '09':(384,6,'SSSESS',56,128),
}
PINS = {
 '04':('0bc9da73daf03b6905d419acea3d7cbcf3d0e57d44010a98475cf3446f0c9b54','81c3b1fe318131b33d31e7d8394e17ec19842cfed4fce6c991955b01d622cfe3','91c54361bcabb2696d42e5d6e5323b83bcf01a74e28874faf528fa35065d5195','7a9c34fd2eee73df07c3e77bc2313f22bf41ae8e463a0e19ae579a06db701ea4',None,None),
 '05':('0bc9da73daf03b6905d419acea3d7cbcf3d0e57d44010a98475cf3446f0c9b54','81c3b1fe318131b33d31e7d8394e17ec19842cfed4fce6c991955b01d622cfe3','91c54361bcabb2696d42e5d6e5323b83bcf01a74e28874faf528fa35065d5195','f0136607c8cefabd309199067ec42223b99cacc7b37b6239797a11c6827f5359','9d5ac2871a8fc9e016affcd01cb0b5520c9f5dd94098707895c96a20523b0777','82aa52cdead91ae71c5476232e199b3bd0ad93f0a448e6bfd89dd7b9e55a4b4c'),
 '06':('6eb2d252959b7fcb4d98a2b55a60a766639d1ac5fe2745313c4c8df4a158e4cd','d6efd124ddc52fd8cf67b26528a89f3cee286dc5105d9c0d6c0951570e804497','e96a3378293194a772c9346e593bfb43590bbe86c7b52e4a80d19a1b1e5bf9cf','97c962e3de7fecd04bdbf958b70daa978de925556cccb26ea873426d220afcd7','34f4f8bbac8f35ec63865baa67fc5502aedd80577c09053ca92e13d838019d14','21fa75793efa893b42af30a2887224b47e740b751c2e875399d591a00784947f'),
 '07':('94f5cc46fc6d412382702b9c678ad14eba4d7ba1a36efd62276fdf3c0afaa2eb','6b787ec1bf6d1e5ab0c472e1567e65299be73fe3bd3b1626f2ed66a3eae76d63','ece0d64fba2e2587cd9724888618fe6581f71f816eaff61bffd216217ad194db','921c40d96a0004f226a1f11137428720aebee285d647182f54dc0b745e8880f2','13231bfa7e5a7aa832d2e9179c72639d0b86fb88e4d43a53049bb4d00dba42cc','e50bc58d6a1fda5bc2e63ef059ff7eb579ad65f2ec5e6127d75ae7c06e03769a'),
 '08':('cb25b8525adf7000aebb6c73ffa8efa85df49b75fef0800026f3ef4b9b2c24b9','6b787ec1bf6d1e5ab0c472e1567e65299be73fe3bd3b1626f2ed66a3eae76d63','1ec524712eba2f2262d7b03382ce75b5824ec0580fca13f36b6d91bb27f7cf23','f80f6b00b3fdfcc31e96db91163fa49cddc176c85e2813426da5adeb1cd65de4','77751aefe15aeac57f600a99e43562c03a258a8a507bca1f56b64d6e5d916736','ce5780d4e06b2e399627986c5a55089bfe9e610128c0fdff8a58c09f14892e63'),
 '09':('3d53b70ab1b1a04b909da8d7d4d2a8bc13cfb11a31419efd5dcf5e81a5d50f7e','47346f34be4265746c8c8fb9498eb412064767cc44b1e81b583fa6297b0903fc','874ca3a547b721d3ce3ad7d96a0685f14dfb5f44f9d41cd2580c458cdc6ca7ec','86f4286a4bf3291009fc1bf2b04bb64d1ad674a59a4905e5da98419e3e5ca7ba','2ee8c811dec24cec978a74027558eb4575e1d096188f985dfa17b6065e444c36','ce7f5576b086feeda783a093a7ae1c16df1a207d77d5fcb8a3ae2b18d39434ec'),
}
SOURCE_SHA256 = {'oracle.lock.json':'7b77395ff5b64f7a220db74975e4c3858a36b19c5c0f5bca92681e3b1bc6504b',
 'references.lock':'278f48fd7697459281aa78a706c9ca57222f36a8c0785d838a6449fbaf26d30b',
 'external/pkl/stdlib/base.pkl':'fddb53666b6e37bc5b70e2d7a2864730c1eab2a4edd90b5c2defefcfdd0b57fb',
 'external/pkl/pkl-core/src/main/java/org/pkl/core/runtime/VmDataSize.java':'8928edffbb2cd46549737760d5af87a11b91c838b392ce19e8d6ba9dc4145010',
 'external/pkl/pkl-core/src/main/java/org/pkl/core/stdlib/base/DataSizeNodes.java':'c1d8add5c47acca5c20610a22ebf6dea27f7b9b16e3b31a39ff9b93ac8a13fe7',
 'external/pkl/pkl-core/src/main/java/org/pkl/core/DataSizeUnit.java':'8aee8c8651a91fe7bd7d4327a72c3abf71fea75e953c43f0242b6854bda97fac',
 'external/pkl/pkl-core/src/main/java/org/pkl/core/DataSize.java':'ba435d35846ab8c817a069d2cb39c9741dd98bcd89903374098d5bad8a32fb6c'}

def need(ok,message):
 if not ok:raise ValueError(message)
def sha(raw):return hashlib.sha256(raw).hexdigest()
def digest(path):return sha(Path(path).read_bytes())
def read(path):return json.loads(Path(path).read_bytes())
def text(folder,name):return (folder/f'{name}.bin').read_text()
def checked(folder,name):
 raw=(folder/f'{name}.bin').read_bytes();units=(folder/f'{name}.code-units.bin').read_bytes()
 need(len(units)%2==0 and units.decode('utf-16-be','surrogatepass').encode('utf-8','replace')==raw,f'Java UTF-8/UTF-16 channel differs: {folder}/{name}')
 return raw.decode('utf-8')
def files(folder):return {p.relative_to(folder).as_posix():digest(p) for p in folder.rglob('*') if p.is_file()}

def verify_inventory(folder):
 inventory=folder/'inventory.tsv'
 actual=''.join(f'{name}\t{(folder/name).stat().st_size}\t{digest(folder/name)}\n' for name in sorted(files(folder)) if name!='inventory.tsv')
 need(inventory.read_text()==actual,'packet inventory differs')

def verify_provenance(folder,number,sources):
 p=read(folder/'provenance.json');m=read(folder/'cases.json');count,rows,statuses,string_files,export_files=CAPTURES[number]
 need(len(m['rows'])==rows and len({x['id'] for x in m['rows']})==rows and m['module_uri'].startswith('file:///stage57/') and m['baseline']=='Pkl 0.32.0','packet row/module scope differs')
 need((folder/'queries.tsv').read_text()==''.join(x['id']+'\t'+x['expression']+'\n' for x in m['rows']),'executed query order differs')
 need(p['input_before']==p['input_after'] and p['source_before']==p['source_after'] and p['stable'] is True,'packet input/source drift')
 probe='DataSizeBytesIdentity.java' if number=='06' else 'DataSizeSplitOracle.java'
 need(set(p['input_before'])=={probe,'module.pkl','queries.tsv','cases.json','run.py'},'packet input closure differs')
 need(set(p['source_before'])==set(SOURCE_SHA256)-{'oracle.lock.json','references.lock'},'packet source closure differs')
 for name,h in p['input_before'].items():need(digest(folder/name)==h,'packet executed input differs')
 for name,h in p['source_before'].items():need(digest(sources/name)==h==SOURCE_SHA256[name],'packet pinned source differs')
 need(p['artifact_sha256']==p['artifact_after_sha256']==JAR_SHA256 and p['reference_revision']=='e51fb2f91774265666f94980ec630db794a4db2d','packet JAR/revision differs')
 need(p['oracle_lock_sha256']==SOURCE_SHA256['oracle.lock.json'] and p['references_lock_sha256']==SOURCE_SHA256['references.lock'],'packet lock differs')
 jdk=p['jdk'];need((jdk['executable_sha256'],jdk['modules_sha256'],jdk['release_sha256'])==JDK_SHA256 and jdk['properties']['java.version']=='21.0.12.1' and jdk['properties']['java.vendor']=='Eclipse Adoptium','packet JDK differs')
 lock=read(sources/'oracle.lock.json')
 need(lock['version']=='0.32.0' and lock['artifact']['sha256']==JAR_SHA256 and lock['source']['revision']==p['reference_revision'],'oracle lock content differs')
 expected=[jdk['executable'],*lock['jvm_options'],f'-Duser.home={p["environment"]["HOME"]}',f'-Djava.io.tmpdir={p["environment"]["TMPDIR"]}','-cp',p['artifact'],str(Path(p['runs'][0]['cwd'])/probe) if number=='06' else str(Path(p['sessions'][0]['cwd'])/probe)]
 return p,m,expected

STRING_FILES={'result.status.bin'}|{name+suffix for name in ['expression','result.stdout','result.stderr','result.error-class'] for suffix in ['.bin','.code-units.bin']}
EXPORT_BASE={'repeat-invoked.bin','call-0.status.bin','call-0.payload-status.bin'}|{name+suffix for name in ['expression']+[f'call-0.{v}' for v in ['error','error-class','java-type','payload-error','text','raw-bits-hex','unit','hash']] for suffix in ['.bin','.code-units.bin']}
EXPORT_REPEAT={'call-1.status.bin','call-1.payload-status.bin','comparison.status.bin'}|{name+suffix for name in [f'call-1.{v}' for v in ['error','error-class','java-type','payload-error','text','raw-bits-hex','unit','hash']]+[f'comparison.{v}' for v in ['error','reference','equals','hash-equal']] for suffix in ['.bin','.code-units.bin']}

def verify_split(folder,number,sources):
 p,m,command=verify_provenance(folder,number,sources);rows=m['rows'];n=len(rows);want=CAPTURES[number]
 need(m['boundaries']==['string','export'] and m['repetitions']==2,'split boundary mode differs')
 need(p['complete'] is (number!='04') and p['string_raw_equal'] is True and p['export_raw_equal'] is True,'split completion/repeat claim differs')
 sessions=p['sessions'];need(len(sessions)==4 and [(x['mode'],x['repeat']) for x in sessions]==[('string',0),('string',1),('export',0),('export',1)] and len({x['pid'] for x in sessions})==4,'split process coverage differs')
 need(all(x['started_epoch']<x['finished_epoch'] for x in sessions) and all(sessions[i]['finished_epoch']<=sessions[i+1]['started_epoch'] for i in range(3)),'split process times/order differ')
 observed=[];summary=[];actual_statuses=''
 for session in sessions:
  mode=session['mode'];repeat=session['repeat'];recorded=Path(session['cwd']);directory=folder/mode/f'repeat-{repeat}'
  need(session['exit_code']==0 and session['query_directories']==n and recorded.is_absolute(),'split JVM result differs')
  need(session['command']==[ *command,mode,str(recorded/mode/f'repeat-{repeat}'),str(recorded),m['module_uri']],'split exact command differs')
  for channel in ['stdout','stderr']:
   payload=(directory/f'process.{channel}.bin').read_bytes()
   need(sha(payload)==session[f'{channel}_sha256'] and len(payload)==session[f'{channel}_bytes'],'split process channel differs')
  need((directory/'process.stderr.bin').read_bytes()==b'','split process stderr differs')
  protocol=[f'session-open mode={mode} rows={n}'];calls=success=errors=datasizes=repeats=0
  for i,row in enumerate(rows):
   id=row['id'];d=directory/f'{i:03d}-{id}'
   need(d.is_dir() and checked(d,'expression')==row['expression'],'split executed expression differs')
   if mode=='string':
    need({x.name for x in d.iterdir() if x.is_file()}==STRING_FILES,'split string channel set differs')
    for name in ['result.stdout','result.stderr','result.error-class']:checked(d,name)
    status=text(d,'result.status');need(status in ('success\n','error\n'),'split string status differs')
    if status=='success\n':need(text(d,'result.stderr')==text(d,'result.error-class')=='','split successful error channel differs')
    else:need(text(d,'result.stdout')=='' and text(d,'result.stderr') and text(d,'result.error-class')=='org.pkl.core.PklException','split ordinary error channel differs')
    protocol.extend([f'call-enter=string-{i}-{id}',f'call-exit=string-{i}-{id} status={status.strip()}']);calls+=1;success+=status=='success\n';errors+=status=='error\n'
    if repeat==0:
     actual_statuses+='S' if status=='success\n' else 'E'
     observed.append({'id':id,'expression':row['expression'],'string_status':status.strip(),'string_stdout':text(d,'result.stdout'),'string_stderr':text(d,'result.stderr'),'producer_error_class':text(d,'result.error-class')})
   else:
    repeatflag=text(d,'repeat-invoked');need(repeatflag in ('true','false'),'split repeat flag differs')
    need({x.name for x in d.iterdir() if x.is_file()}==(EXPORT_BASE|EXPORT_REPEAT if repeatflag=='true' else EXPORT_BASE),'split export channel set differs')
    for name in ['expression']+[f'call-0.{v}' for v in ['error','error-class','java-type','payload-error','text','raw-bits-hex','unit','hash']]:checked(d,name)
    status=text(d,'call-0.status');payloadstatus=text(d,'call-0.payload-status')
    need(status in ('success\n','error\n') and status.strip()==observed[i]['string_status'],'split public status differs')
    need(payloadstatus==('success\n' if status=='success\n' else 'not-applicable\n'),'split payload status differs')
    need(repeatflag==('true' if text(d,'call-0.java-type')=='org.pkl.core.DataSize' else 'false'),'split DataSize repeat cause differs')
    if status=='error\n':need(text(d,'call-0.error') and text(d,'call-0.error-class')=='org.pkl.core.PklException','split export ordinary error differs')
    else:need(text(d,'call-0.error')==text(d,'call-0.error-class')=='','split export success metadata differs')
    call=f'export-{i}-{id}-0';protocol.extend([f'call-enter={call}',f'call-exit={call} status={status.strip()}']);calls+=1;success+=status=='success\n';errors+=status=='error\n'
    if status=='success\n':protocol.extend([f'payload-enter={call}',f'payload-exit={call} status=success'])
    if repeatflag=='true':
     datasizes+=1;repeats+=1
     need(re.fullmatch('[0-9a-f]{16}',text(d,'call-0.raw-bits-hex')) and text(d,'call-0.unit') in ('b','kb','kib','mb','mib','gb','gib','tb','tib','pb','pib'),'split DataSize payload differs')
     for name in [f'call-1.{v}' for v in ['error','error-class','java-type','payload-error','text','raw-bits-hex','unit','hash']]+[f'comparison.{v}' for v in ['error','reference','equals','hash-equal']]:checked(d,name)
     need(text(d,'call-1.status')==text(d,'call-1.payload-status')==text(d,'comparison.status')=='success\n','split repeated export status differs')
     call1=f'export-{i}-{id}-1';compare=f'export-{i}-{id}-comparison';protocol.extend([f'call-enter={call1}',f'call-exit={call1} status=success',f'payload-enter={call1}',f'payload-exit={call1} status=success',f'comparison-enter={compare}',f'comparison-exit={compare} status=success']);calls+=1;success+=1
    if repeat==0:observed[i].update({'export_status':status.strip(),'producer_java_type':text(d,'call-0.java-type'),'export_raw_bits':text(d,'call-0.raw-bits-hex'),'export_unit':text(d,'call-0.unit'),'export_hash':text(d,'call-0.hash'),'export_text':text(d,'call-0.text'),'export_repeat':repeatflag})
  protocol.append(f'session-closed mode={mode} rows={n} public-calls={calls} success={success} errors={errors} datasize-rows={datasizes} repeated-export-calls={repeats}')
  need((directory/'process.stdout.bin').read_bytes()==('\n'.join(protocol)+'\n').encode(),'split full process protocol differs')
  summary.append({'mode':mode,'repeat':repeat,'public_calls':calls,'success':success,'ordinary_errors':errors,'datasize_export_rows':datasizes,'raw_files':len(files(directory))})
 for mode,count in [('string',want[3]),('export',want[4])]:
  left,right=files(folder/mode/'repeat-0'),files(folder/mode/'repeat-1')
  need(left==right and len(left)==count==p[f'{mode}_files'] and p[f'{mode}_raw_differences']==[],'split exact raw repetition differs')
 need(actual_statuses==want[2],'split independent status vector differs')
 need([x['public_calls'] for x in summary]==([44,44,49,49] if number in ('04','05') else [n]*4),'split public call count differs')
 if number!='04':
  audit=read(folder/'audit.json');need(audit['compatibility_passes_claimed']==0 and audit['queries']==n and len(audit['observations'])==n,'split acquired audit scope differs')
  for actual,recorded in zip(observed,audit['observations']):
   for key in actual:need(recorded[key]==actual[key],f'split audit raw field differs: {number}/{key}')
 return {'rows':n,'module_uri':m['module_uri'],'string_calls_per_session':n,'export_calls_per_session':summary[2]['public_calls'],'statuses':dict(Counter('success' if x=='S' else 'error' for x in actual_statuses)),'raw_repetitions_equal':True,'complete_acquisition':number!='04','candidate_compatibility_passes_claimed':0}

IDENTITY_FIELDS=['expression']+[f'call-{ordinal}.{name}' for ordinal in range(2) for name in ['status','error','java-type','text','hash']]+[f'comparison.{name}' for name in ['reference','equals','hash-equal']]
IDENTITY_FILES={name+suffix for name in IDENTITY_FIELDS for suffix in ['.bin','.code-units.bin']}

def verify_identity(folder,sources):
 p,m,command=verify_provenance(folder,'06',sources);rows=m['rows'];n=len(rows)
 need(p['complete'] is True and p['raw_equal'] is True and len(p['runs'])==2 and [x['repeat'] for x in p['runs']]==[0,1] and len({x['pid'] for x in p['runs']})==2,'identity process coverage differs')
 observed=[];summaries=[];status_vector=''
 for run in p['runs']:
  repeat=run['repeat'];recorded=Path(run['cwd']);directory=folder/f'repeat-{repeat}'
  need(run['exit_code']==0 and run['query_directories']==n and run['command']==[*command,str(recorded/f'repeat-{repeat}'),str(recorded),m['module_uri']],'identity exact command/result differs')
  for channel in ['stdout','stderr']:
   raw=(directory/f'process.{channel}.bin').read_bytes();need(sha(raw)==run[f'{channel}_sha256'] and len(raw)==run[f'{channel}_bytes'],'identity process raw differs')
  need((directory/'process.stderr.bin').read_bytes()==b'','identity stderr differs')
  protocol=[f'session-open rows={n}'];calls=errors=0
  for index,row in enumerate(rows):
   d=directory/f'{index:03d}-{row["id"]}'
   need(d.is_dir() and {x.name for x in d.iterdir() if x.is_file()}==IDENTITY_FILES,'identity row/channel set differs')
   for name in IDENTITY_FIELDS:checked(d,name)
   need(text(d,'expression')==row['expression'],'identity query differs')
   statuses=[]
   for ordinal in range(2):
    status=text(d,f'call-{ordinal}.status');need(status in ('success\n','error\n'),'identity status differs');statuses.append(status.strip())
    if status=='success\n':need(text(d,f'call-{ordinal}.error')=='','identity success error differs')
    else:need(text(d,f'call-{ordinal}.error') and text(d,f'call-{ordinal}.java-type')=='org.pkl.core.PklException','identity ordinary error differs');errors+=1
    call=f'export-{index}-{row["id"]}-{ordinal}';protocol.extend([f'call-enter={call}',f'call-exit={call} status={status.strip()}']);calls+=1
   for name in ['reference','equals','hash-equal']:
    value=text(d,f'comparison.{name}');need(value in ('true','false') if statuses==['success','success'] else value=='','identity comparison field differs')
   if repeat==0:
    status_vector+='S' if statuses[0]=='success' else 'E'
    observed.append({'id':row['id'],'expression':row['expression'],'statuses':statuses,'java_type':text(d,'call-0.java-type'),'text':text(d,'call-0.text'),'error':text(d,'call-0.error'),'repeat_reference':text(d,'comparison.reference'),'repeat_java_equals':text(d,'comparison.equals'),'repeat_java_hash_equal':text(d,'comparison.hash-equal')})
  protocol.append(f'session-closed rows={n} public-calls={calls} errors={errors}')
  need((directory/'process.stdout.bin').read_bytes()==('\n'.join(protocol)+'\n').encode(),'identity full process protocol differs')
  summaries.append({'repeat':repeat,'public_calls':calls,'ordinary_errors':errors,'raw_files':len(files(directory))})
 need(status_vector==CAPTURES['06'][2] and [x['public_calls'] for x in summaries]==[16,16] and [x['ordinary_errors'] for x in summaries]==[4,4],'identity statuses/call counts differ')
 left,right=files(folder/'repeat-0'),files(folder/'repeat-1');need(left==right and len(left)==p['raw_files']==226 and p['raw_differences']==[],'identity raw repetition differs')
 audit=read(folder/'audit.json');need(audit['compatibility_passes_claimed']==0 and len(audit['observations'])==n,'identity acquired audit scope differs')
 for actual,recorded in zip(observed,audit['observations']):
  for key in actual:need(recorded[key]==actual[key],f'identity acquired audit field differs: {key}')
 return {'rows':n,'module_uri':m['module_uri'],'public_export_calls_per_session':16,'ordinary_errors_per_session':4,'raw_repetitions_equal':True,'boundary':'Java public array/DataSize wrapper identity; separate from Pkl language equality and candidate parity','candidate_compatibility_passes_claimed':0}

def verify(root=DEFAULT, *, expected_archive=ARCHIVE_SHA256, expected_fileset=FILESET_SHA256):
 root=Path(root);manifest=read(root/'manifest.json')
 for path in root.rglob('*'):need(not path.is_symlink(),'supplement fixture symlink differs')
 actual={p.relative_to(root).as_posix():digest(p) for p in root.rglob('*') if p.is_file() and p.name!='manifest.json'}
 need(manifest=={'schema':1,'kind':KIND,'baseline':'Pkl 0.32.0','files':actual},'supplement fixture manifest differs')
 expected_root={'README.md','archive-identity.json','oracle-evidence.tar.gz'}|{f'inputs/{n}/{name}' for n in ('05','06','07','08','09') for name in ('cases.json','module.pkl','queries.tsv')}
 need(set(actual)==expected_root,'supplement fixture root file set differs')
 archive=root/'oracle-evidence.tar.gz';need(digest(archive)==expected_archive,'independent supplement archive SHA differs')
 identity=read(root/'archive-identity.json')
 need(identity['archive_sha256']==expected_archive and identity['fileset_sha256']==expected_fileset and identity['members']==7612,'supplement identity report differs')
 with tempfile.TemporaryDirectory(prefix='pkl-data-size-supplement-') as temporary:
  destination=Path(temporary);filenames=set();file_set=hashlib.sha256();counts={n:0 for n in CAPTURES}
  with tarfile.open(archive,'r:gz') as stream:
   for member in stream:
    name=member.name
    need(member.isfile() and not Path(name).is_absolute() and '..' not in Path(name).parts and name not in filenames,'unsafe/duplicate supplement archive member')
    filenames.add(name);raw=stream.extractfile(member).read();file_set.update(name.encode()+b'\0'+sha(raw).encode()+b'\n')
    for n in counts:
     if name.startswith(f'capture-{n}/'):counts[n]+=1
    path=destination/name;path.parent.mkdir(parents=True,exist_ok=True);path.write_bytes(raw)
  need(file_set.hexdigest()==expected_fileset and counts=={n:CAPTURES[n][0] for n in CAPTURES} and len(filenames)==7612,'independent supplement file-set differs')
  need(identity['captures']==counts and identity['sources']==SOURCE_SHA256 and identity['executed_probes']=={'DataSizeSplitOracle.java':PROBES['split'],'DataSizeBytesIdentity.java':PROBES['identity']},'supplement archive report/source/probe seal differs')
  sources=destination/'sources'
  for name,h in SOURCE_SHA256.items():need(digest(sources/name)==h,'supplement pinned source/lock bytes differ')
  results={}
  for n in CAPTURES:
   folder=destination/f'capture-{n}'
   pins=PINS[n]
   for name,h in zip(('cases.json','module.pkl','queries.tsv','provenance.json','audit.json','inventory.tsv'),pins):
    if h is not None:need(digest(folder/name)==h,f'supplement independent capture pin differs: {n}/{name}')
   probe='DataSizeBytesIdentity.java' if n=='06' else 'DataSizeSplitOracle.java'
   need(digest(folder/probe)==PROBES['identity' if n=='06' else 'split'],'supplement executed probe bytes differ')
   if n!='04':
    verify_inventory(folder)
    for name in ('cases.json','module.pkl','queries.tsv'):
     need(digest(root/'inputs'/n/name)==digest(folder/name),'supplement authored input differs from execution')
   results[n]=verify_identity(folder,sources) if n=='06' else verify_split(folder,n,sources)
  need(results['04']['complete_acquisition'] is False and results['05']['complete_acquisition'] is True,'historical wrapper result relabeled')
  return {'kind':KIND,'baseline':'Pkl 0.32.0','archive_sha256':expected_archive,'fileset_sha256':expected_fileset,
          'common_string_cohorts':{n:results[n] for n in ('05','07','08','09')},'public_identity_06':results['06'],
          'preserved_incomplete_history_04':results['04'],'candidate_compatibility_passes_claimed':0,
          'remaining_required':['Pkl Binary codec and malformed cases','Broader Bytes producers, errors and internal size-cache identity','Other DataSize renderer/export APIs','Native/Node/browser candidate comparison','Whole-language, lifecycle, OS and performance gates']}

if __name__=='__main__':
 parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--fixture',type=Path,default=DEFAULT)
 args=parser.parse_args();print(json.dumps(verify(args.fixture),sort_keys=True))
