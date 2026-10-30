"""Independently verify only ten new Stage121 archives; no extraction or old archive reads."""
from pathlib import Path
import hashlib,io,json,os,stat,tarfile,time,zlib
R=Path('/Users/cody.ij.hwang/Projects/pkl-rs.stage57');C=R/'target/m2/stage121-publication-catalog-prep-01';A=R/'target/m2/stage121-evidence-archives-01';O=Path(__file__).parent
SEAL='07c215e4ad02514be6573d48e62bdace77f4a888c461b7dd8853c4ca2c236218';CAP=85*1024**2

def require(ok,message):
 if not ok:raise ValueError(message)
def relative(name,archive=False):
 p=Path(name)
 require(not p.is_absolute() and '..'not in p.parts and str(p)==name,'unsafe/noncanonical relative name')
 require(not any(x=='stage65-b-native-regressions-evidence.tar.gz.partial'or x.lower().startswith('stage86')or'private8'in x.lower()for x in p.parts),'excluded name before metadata')
 if not archive:require(not name.endswith('.tar.gz'),'old archive body excluded')
 return p
def read(p):
 relative(str(p.relative_to(R)),archive=p.parent==A and p.name.startswith('stage121-pair-supertype-evidence-'))
 fd=os.open(p,os.O_RDONLY|os.O_NOFOLLOW)
 with os.fdopen(fd,'rb')as f:
  before=os.fstat(f.fileno());require(stat.S_ISREG(before.st_mode),'regular file required');data=f.read();after=os.fstat(f.fileno())
 require(all(getattr(before,k)==getattr(after,k)for k in ('st_ino','st_size','st_mtime_ns','st_ctime_ns','st_mode')),'changed during read')
 return data,dict(path=str(p.relative_to(R)),bytes=len(data),mode=stat.S_IMODE(after.st_mode),sha256=hashlib.sha256(data).hexdigest())
def check(record):
 p=R/relative(record['path'],archive=record['path'].startswith(str(A.relative_to(R))+'/stage121-pair-supertype-evidence-'))
 data,pin=read(p)
 require(all(pin[k]==v for k,v in record.items() if k in pin),'pin mismatch '+record['path']);return data,pin
started=time.monotonic();report=dict(complete=False,errors=[],old_archive_body_reads=0,compiler_Java_guest_runs=0,parts=[]);initial=[]
try:
 raw,seal=read(C/'readiness.json');require(seal['sha256']==SEAL,'sealed catalog changed');ready=json.loads(raw)
 raw,producer_report_pin=read(A/'report.json');producer=json.loads(raw);require(producer['complete']is True,'producer incomplete');initial.append(producer_report_pin)
 require(producer['catalog']==seal,'producer catalog mismatch');check(producer['producer']);initial.append(producer['producer'])
 metadata=[seal,*ready['files'],*ready['supplemental_files']];require(len(metadata)==19,'metadata count');metabytes={}
 for p in metadata:
  data,actual=check(p);metabytes[p['path']]=data;initial.append(actual)
 parts=json.loads(metabytes[str((C/'parts.json').relative_to(R))]);require(len(parts)==10,'ten parts')
 aliases={}
 for line in metabytes[str((C/'paths.jsonl').relative_to(R))].splitlines():
  row=json.loads(line);relative(row['path']);require(row['path']not in aliases,'duplicate alias');require(type(row['mode'])is int and 0<=row['mode']<=0o7777,'alias mode');aliases[row['path']]=row
 require(len(aliases)==222571,'exact alias count')
 objects={}
 for part in parts:
  for x in part['members']:
   relative(x['member']);require(x['member']=='objects/'+x['sha256'],'content addressed object');require(x['member']not in objects,'duplicate object');objects[x['member']]=x
 require(len(objects)==13514,'exact object count')
 for x in aliases.values():
  obj=objects.get('objects/'+x['sha256']);require(obj is not None and obj['bytes']==x['bytes'],'unresolved alias')
 seen=set();metadata_seen=set();header_mode_differences=0
 require(len(producer['parts'])==10,'producer exact part count')
 for n,(part,record)in enumerate(zip(parts,producer['parts']),1):
  require(part['number']==n,'part ordering');expected_name=f'target/m2/stage121-evidence-archives-01/stage121-pair-supertype-evidence-{n:02d}.tar.gz';require(record['path']==expected_name,'exact fresh archive path')
  compressed,pin=check(record);require(pin['bytes']<CAP,'actual85MiB cap');initial.append(pin)
  inflater=zlib.decompressobj(31);plain=inflater.decompress(compressed)+inflater.flush()
  require(inflater.eof and inflater.unused_data==b'' and inflater.unconsumed_tail==b'','single complete gzip stream with CRC/ISIZE and exact EOF')
  allowed=(metadata if n==1 else[]);expected=[x['path']for x in allowed]+[x['member']for x in part['members']]
  seen_names=[];last_end=0
  with tarfile.open(fileobj=io.BytesIO(plain),mode='r:')as tf:
   for member in tf:
    relative(member.name);require(member.isfile()and not member.issym()and not member.islnk(),'only regular files');require(member.uid==member.gid==member.mtime==0,'deterministic ownership/time');require(not member.uname and not member.gname,'empty owner names');seen_names.append(member.name)
    data=tf.extractfile(member).read();sha=hashlib.sha256(data).hexdigest();require(len(data)==member.size,'complete member')
    if member.name.startswith('objects/'):
     x=objects[member.name];require(member.name not in seen,'duplicate archived object');source=aliases[x['source']]
     require((len(data),sha,member.mode)==(x['bytes'],x['sha256'],source['mode']),'object byte/source-mode mismatch');seen.add(member.name)
    else:
     x=next(x for x in allowed if x['path']==member.name)
     require((len(data),sha,member.mode)==(x['bytes'],x['sha256'],x['mode']),'metadata identity/mode');require(data==metabytes[member.name],'metadata full raw mismatch');metadata_seen.add(member.name)
    last_end=max(last_end,member.offset_data+((member.size+511)//512)*512)
  require(seen_names==expected,'complete ordered member roster')
  require(len(plain)%10240==0 and len(plain)-last_end>=1024 and not any(plain[last_end:]),'tar exact zero end markers/padding')
  report['parts'].append(dict(**pin,members=len(expected),objects=len(part['members']),metadata=len(allowed),uncompressed_bytes=len(plain),gzip_CRC_ISIZE_EOF_verified=True,tar_headers_padding_EOF_verified=True))
  print('verified part',n,len(expected),'members',flush=True)
 require(seen==set(objects)and metadata_seen==set(metabytes),'full cross-part closure')
 for row in aliases.values():
  origin=aliases[objects['objects/'+row['sha256']]['source']]
  if row['mode']!=origin['mode']:header_mode_differences+=1
 for p in initial:check(p)
 source_data,source_pin=read(Path(__file__))
 report.update(complete=True,errors=[],catalog=seal,producer_report=producer_report_pin,verifier=source_pin,unique_objects=len(seen),metadata_members=len(metadata_seen),logical_aliases=len(aliases),alias_modes_validated=True,aliases_requiring_mode_override=header_mode_differences,aliases_freshly_rehashed=False,alias_resolution='Every alias bytes/SHA resolves to an independently verified object; restore each alias mode from paths.jsonl.',compressed_bytes=sum(p['bytes']for p in report['parts']),bookends_stable=True,prior_published_dependencies_explicit=True,full_API_pass=False,full_M2_pass=False,final_documents_archived=False,limitations=['Prior Stage114/Stage120 published checkpoints remain required; their archive bodies were not read.','This verifies the finite archived checkpoint payload and exact alias restoration metadata, not a fresh run of compatibility tests or a self-contained installed toolchain.'])
except BaseException as e:
 report['errors'].append(repr(e))
finally:
 report['elapsed_seconds']=time.monotonic()-started
 with (O/'report.json').open('x')as f:json.dump(report,f,indent=2);f.write('\n')
 print(json.dumps({k:report.get(k)for k in ('complete','errors','unique_objects','metadata_members','logical_aliases','compressed_bytes','elapsed_seconds')}),flush=True)
raise SystemExit(0 if report['complete'] else 1)
