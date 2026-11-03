#!/usr/bin/env python3
"""Copy only freshly produced, independently verified Stage141 evidence."""
from pathlib import Path
import hashlib,json,os,shutil,stat,subprocess,sys,time,traceback
ROOT=Path(__file__).absolute().parents[3]; PREP=Path(__file__).absolute().parent; DEST=ROOT/'reports/m2/stage141-jsonnet-xml'; INDEX=ROOT/'target/m2/stage141-publication-partition-01/report.json'; SEQUENCE=ROOT/'target/m2/stage141-publication-sequence-01/report.json'; AGG=ROOT/'target/m2/stage141-publication-parts-readback-01/report.json'; HEAD='508a82c35abac3d4c73e4f59a6fdfd076aba51b4';FLOOR=1024**3

def safe(path):
 p=Path(path)
 if not p.is_absolute() or '..' in p.parts or not p.is_relative_to(ROOT) or any(x=='stage65-b-native-regressions-evidence.tar.gz.partial' or 'stage86' in x.lower() or 'private8' in x.lower() or x=='stage140-native-concurrency-admission-01' for x in p.parts):raise ValueError('excluded path')
 return p

def pin(path):
 p=safe(path);cur=ROOT
 for part in p.relative_to(ROOT).parts:
  cur=cur/part;st=os.lstat(cur)
  if stat.S_ISLNK(st.st_mode):raise ValueError('symlink path')
 if not stat.S_ISREG(st.st_mode):raise ValueError('regular file required')
 with p.open('rb') as f:h=hashlib.file_digest(f,'sha256').hexdigest()
 q=os.lstat(p)
 if (st.st_dev,st.st_ino,st.st_size,st.st_mtime_ns,st.st_ctime_ns)!=(q.st_dev,q.st_ino,q.st_size,q.st_mtime_ns,q.st_ctime_ns):raise ValueError('file changed during hash')
 return {'path':str(p.relative_to(ROOT)),'bytes':st.st_size,'sha256':h,'mode':stat.S_IMODE(st.st_mode)}
def matches(row,p):
 recorded=Path(row['path']); recorded=recorded if recorded.is_absolute() else ROOT/recorded
 if safe(recorded)!=safe(p):raise ValueError('pinned path differs: '+str(p))
 actual=pin(p)
 if row['bytes']!=actual['bytes'] or row['sha256']!=actual['sha256']:raise ValueError('pinned input differs: '+str(p))
 return actual

def head():return subprocess.check_output(['git','-c','maintenance.auto=false','-c','gc.auto=0','rev-parse','HEAD'],cwd=ROOT,text=True).strip()
def main():
 if len(sys.argv)!=1:raise ValueError('no arguments')
 for parent in [ROOT/'reports',ROOT/'reports/m2']:
  st=os.lstat(parent)
  if not stat.S_ISDIR(st.st_mode) or stat.S_ISLNK(st.st_mode):raise ValueError('unsafe report parent')
 if os.path.lexists(DEST):raise ValueError('fresh report directory required')
 if head()!=HEAD:raise ValueError('HEAD differs')
 if shutil.disk_usage(ROOT).free<FLOOR:raise ValueError('disk floor')
 indexpin=pin(INDEX);index=json.loads(INDEX.read_bytes()); seqpin=pin(SEQUENCE);seq=json.loads(SEQUENCE.read_bytes());aggpin=pin(AGG);agg=json.loads(AGG.read_bytes())
 if not index.get('complete') or index.get('archive_created') is not False or indexpin['sha256']!='bf2be71b50e09b123643f04a47e602024e083c440a21779cc3fee9b96dc8a997' or index.get('head')!=HEAD:raise ValueError('partition invalid')
 if not seq.get('complete') or seq.get('parts')!=20 or seq.get('head_before')!=HEAD or seq.get('head_after')!=HEAD:raise ValueError('sequence incomplete')
 if seq.get('inputs_before')!=seq.get('inputs_after') or not seq.get('inputs_before'):raise ValueError('sequence input bookends differ')
 for row in seq['inputs_before']:
  path=Path(row['path']);matches(row,path)
 if str(ROOT/'target/m2/stage141-publication-sequence-prep-01/run.py') not in {r['path'] for r in seq['inputs_before']}:raise ValueError('sequence runner not pinned')
 matches(seq['index'],INDEX);matches(seq['aggregate'],AGG)
 if not agg.get('complete') or not agg.get('archive_bytes_rehashed_after_independent_readback') or len(agg.get('parts',[]))!=20:raise ValueError('aggregate incomplete')
 matches(agg['partition'],INDEX)
 parts=index['parts'];steps=seq['steps']
 if [x['number'] for x in parts]!=list(range(1,21)) or len(steps)!=41:raise ValueError('part or step roster differs')
 names=[z for i in range(1,21) for z in (f'publish-{i:04}',f'readback-{i:04}')]+['verify-parts']
 if [x['name'] for x in steps]!=names or any(x.get('exit_code')!=0 for x in steps):raise ValueError('sequence subprocess roster failed')
 files=[]
 def add(src,name):
  path=safe(Path(src));record=pin(path)
  if '/' in name or name in {x['destination'] for x in files}:raise ValueError('duplicate or unsafe destination')
  files.append({'source':record,'destination':name})
 add(INDEX,'partition.json');add(AGG,'parts-readback.json');add(SEQUENCE,'sequence.json');add(ROOT/'target/m2/stage141-publication-sequence-prep-01/run.py','runner.py');add(ROOT/'target/m2/stage141-publication-sequence-prep-01/PLAN.md','runner-PLAN.md');add(PREP/'copy.py','final-copy.py');add(PREP/'PLAN.md','final-copy-PLAN.md')
 for step in steps:
  name=step['name']
  if name=='verify-parts':expected_receipt=AGG
  elif name.startswith('publish-'):expected_receipt=ROOT/('target/m2/stage141-publication-part-'+name[-4:])/'report.json'
  else:expected_receipt=ROOT/('target/m2/stage141-publication-readback-part-'+name[-4:])/'report.json'
  matches(step['receipt'],expected_receipt)
  for stream in ['stdout','stderr']:
   src=ROOT/'target/m2/stage141-publication-sequence-01'/step['name']/(stream+'.bin');matches(step[stream],src);add(src,step['name']+'-'+stream+'.bin')
 for part,entry in zip(parts,agg['parts']):
  i=part['number'];tag=f'part-{i:04}';p=ROOT/f'target/m2/stage141-publication-part-{i:04}';r=ROOT/f'target/m2/stage141-publication-readback-part-{i:04}'
  if entry['number']!=i:raise ValueError('aggregate order differs')
  pub=json.loads((p/'report.json').read_bytes());reader=json.loads((r/'report.json').read_bytes())
  if not pub.get('complete') or not pub.get('bookends_stable') or not reader.get('complete'):raise ValueError('part not qualified')
  matches(entry['publication'],p/'report.json');matches(entry['readback'],r/'report.json');matches(reader['publisher'],p/'report.json');matches(reader['manifest'],p/'members.json')
  archive=p/'evidence.tar.gz';ap=matches(pub['archive'],archive);matches(reader['archive'],archive)
  if ap['bytes']>=50*1024**2 or reader['archive']['sha256']!=pub['archive']['sha256']:raise ValueError('archive hash or limit differs')
  selection=safe(ROOT/part['selection']['path']);matches(part['selection'],selection)
  if pub['selection']['sha256']!=part['selection']['sha256'] or pub['selection']['path']!=part['selection']['path']:raise ValueError('selection mismatch')
  if pub['manifest_sha256']!=pin(p/'members.json')['sha256']:raise ValueError('manifest mismatch')
  for key,src in [('evidence.tar.gz',archive),('selection.json',selection),('publisher.json',p/'report.json'),('readback.json',r/'report.json'),('members.json',p/'members.json'),('members-verified.json',r/'members-verified.json'),('inputs-before.json',p/'inputs-before.json'),('inputs-after.json',p/'inputs-after.json')]:add(src,tag+'-'+key)
  matches(reader['inputs_before'],p/'inputs-before.json');matches(reader['inputs_after'],p/'inputs-after.json')
 # Index, process receipts, members and archive bytes are all pinned before copying.
 if head()!=HEAD or pin(INDEX)!=indexpin or pin(SEQUENCE)!=seqpin or pin(AGG)!=aggpin:raise ValueError('authorities changed')
 DEST.mkdir();records=[];report={'complete':False,'head_before':HEAD,'parts':20,'files':records,'scope':'Exact copy of verified new Stage141 archive parts and receipts; no complete source/compiler/cache restoration claim.','errors':[],'started_epoch':time.time()}
 def save():
  p=DEST/'publication.next';p.write_text(json.dumps(report,indent=2)+'\n');p.replace(DEST/'publication.json')
 save()
 try:
  for row in files:
   source=ROOT/row['source']['path'];dest=DEST/row['destination']
   if pin(source)!=row['source']:raise ValueError('source changed before copy')
   if shutil.disk_usage(ROOT).free<FLOOR:raise ValueError('disk floor')
   proc=subprocess.run(['/bin/cp','-c','-p',str(source),str(dest)],capture_output=True,timeout=120)
   if proc.returncode!=0 or proc.stdout or proc.stderr:raise ValueError('APFS copy failed: '+row['destination'])
   copied=pin(dest);before=row['source']
   if any(copied[k]!=before[k] for k in ('bytes','sha256','mode')) or pin(source)!=before:raise ValueError('copied bytes or source differ')
   records.append({'source':before,'copy':copied});save()
  for row in records:
   if pin(ROOT/row['copy']['path'])!=row['copy'] or pin(ROOT/row['source']['path'])!=row['source']:raise ValueError('post-copy identity changed')
  if head()!=HEAD:raise ValueError('HEAD changed')
  report.update(complete=True,head_after=HEAD,copied_files=len(records),finished_epoch=time.time())
 except BaseException as e:report['errors'].append(repr(e));report['traceback']=traceback.format_exc();report['finished_epoch']=time.time()
 save();print(json.dumps({'complete':report['complete'],'copied_files':len(records),'errors':report['errors']}));return 0 if report['complete'] else 1
if __name__=='__main__':sys.exit(main())
