#!/usr/bin/env python3
"""Publish and independently read back the exact terminal Stage141 partition in order."""
from pathlib import Path
import hashlib,json,os,subprocess,sys,time,traceback
ROOT=Path(__file__).absolute().parents[3];PREP=Path(__file__).absolute().parent;TOOLS=ROOT/'target/m2/stage141-publication-prep-05';INDEX=ROOT/'target/m2/stage141-publication-partition-01/report.json';OUT=ROOT/'target/m2/stage141-publication-sequence-01';VERIFY=ROOT/'target/m2/stage141-publication-parts-readback-01';HEAD='508a82c35abac3d4c73e4f59a6fdfd076aba51b4';CANDIDATE_SHA='5991379e19c925df14593e9e805f35b93a0198f616795709d5cb6e5ea5dca168'
def safe(path):
 p=Path(path)
 if '..' in p.parts or any(x=='stage65-b-native-regressions-evidence.tar.gz.partial' or 'stage86' in x.lower() or 'private8' in x.lower() or x=='stage140-native-concurrency-admission-01' for x in p.parts):raise ValueError('forbidden path')
 return p
def pin(path):
 p=safe(path)
 if p.is_symlink() or not p.is_file():raise ValueError('regular input required')
 with p.open('rb') as f:h=hashlib.file_digest(f,'sha256').hexdigest()
 return {'path':str(p),'bytes':p.stat().st_size,'sha256':h}
def head():return subprocess.check_output(['git','-c','maintenance.auto=false','-c','gc.auto=0','rev-parse','HEAD'],cwd=ROOT,text=True).strip()
def main():
 if len(sys.argv)!=1:raise ValueError('no arguments accepted')
 if OUT.exists() or OUT.is_symlink():raise ValueError('fresh sequence output required')
 OUT.mkdir();r={'complete':False,'errors':[],'steps':[],'scope':'Sequential finite publication and independent readbacks. Final published-copy verification and Git blob checks remain separate.','started_epoch':time.time()}
 def save():
  p=OUT/'report.next';p.write_text(json.dumps(r,indent=2)+'\n');p.replace(OUT/'report.json')
 def run(name,argv,output):
  directory=OUT/name;directory.mkdir();started=time.monotonic()
  with (directory/'stdout.bin').open('xb') as stdout,(directory/'stderr.bin').open('xb') as stderr:
   result=subprocess.run([str(x) for x in argv],cwd=ROOT,stdin=subprocess.DEVNULL,stdout=stdout,stderr=stderr)
  row={'name':name,'argv':[str(x) for x in argv],'cwd':str(ROOT),'exit_code':result.returncode,'elapsed_seconds':time.monotonic()-started,'stdout':pin(directory/'stdout.bin'),'stderr':pin(directory/'stderr.bin')}
  receipt=output/'report.json'
  if receipt.is_file() and not receipt.is_symlink():row['receipt']=pin(receipt)
  r['steps'].append(row);save()
  if result.returncode!=0:raise RuntimeError(name+' failed; raw output retained')
  if not receipt.is_file() or not json.loads(receipt.read_bytes()).get('complete'):raise ValueError(name+' has no complete report')
 try:
  if head()!=HEAD:raise ValueError('HEAD differs')
  idxpin=pin(INDEX);idx=json.loads(INDEX.read_bytes());parts=idx['parts']
  if idxpin['sha256']!='bf2be71b50e09b123643f04a47e602024e083c440a21779cc3fee9b96dc8a997' or idx.get('member_count')!=6236 or idx.get('selected_payload_bytes')!=827923900 or not idx.get('complete') or idx.get('archive_created') is not False or idx.get('head')!=HEAD or len(parts)!=20 or [x['number'] for x in parts]!=list(range(1,21)):raise ValueError('partition contract differs')
  candidate=ROOT/safe(idx['candidate']['path']);cp=pin(candidate)
  if cp['sha256']!=CANDIDATE_SHA or cp['sha256']!=idx['candidate']['sha256'] or cp['bytes']!=idx['candidate']['bytes']:raise ValueError('candidate identity differs')
  files=[INDEX,candidate,PREP/'run.py']+[TOOLS/n for n in ['publish.py','readback.py','verify-parts.py']]
  for part in parts:
   expected=f'target/m2/stage141-publication-partition-01/selection-{part["number"]:04}.json'
   if part['selection']['path']!=expected:raise ValueError('unexpected part selection path')
   path=ROOT/safe(expected);actual=pin(path)
   if actual['sha256']!=part['selection']['sha256'] or actual['bytes']!=part['selection']['bytes']:raise ValueError('part selection differs')
   files.append(path)
  outputs=[]
  for i in range(1,21):outputs.extend([ROOT/f'target/m2/stage141-publication-part-{i:04}',ROOT/f'target/m2/stage141-publication-readback-part-{i:04}'])
  outputs.append(VERIFY)
  for path in outputs:
   safe(path)
   if os.path.lexists(path):raise ValueError('output already exists: '+str(path))
  before=[pin(p) for p in files];r.update(index=idxpin,inputs_before=before,head_before=HEAD,expected_parts=20);save()
  pairs=[]
  for part in parts:
   i=part['number'];pub=ROOT/f'target/m2/stage141-publication-part-{i:04}';reader=ROOT/f'target/m2/stage141-publication-readback-part-{i:04}'
   run(f'publish-{i:04}',[sys.executable,'-B',TOOLS/'publish.py','--selection',part['selection']['path'],'--output',str(pub.relative_to(ROOT))],pub)
   run(f'readback-{i:04}',[sys.executable,'-B',TOOLS/'readback.py','--publication',str(pub.relative_to(ROOT)),'--output',str(reader.relative_to(ROOT))],reader)
   pairs.append((pub,reader))
  argv=[sys.executable,'-B',TOOLS/'verify-parts.py','--partition',INDEX]
  for pub,reader in pairs:argv.extend(['--pair',pub,reader])
  argv.extend(['--output',VERIFY]);run('verify-parts',argv,VERIFY)
  after=[pin(p) for p in files]
  if before!=after or head()!=HEAD:raise ValueError('sequence inputs or HEAD changed')
  verified=json.loads((VERIFY/'report.json').read_bytes())
  if len(verified.get('parts',[]))!=20 or not verified.get('archive_bytes_rehashed_after_independent_readback'):raise ValueError('incomplete aggregate verification')
  r.update(complete=True,inputs_after=after,head_after=HEAD,parts=20,aggregate=pin(VERIFY/'report.json'))
 except BaseException as e:r['errors'].append(repr(e));r['traceback']=traceback.format_exc()
 finally:r['finished_epoch']=time.time();save()
 print(json.dumps({k:r.get(k) for k in ['complete','parts','errors']}));return 0 if r['complete'] else 1
if __name__=='__main__':sys.exit(main())
