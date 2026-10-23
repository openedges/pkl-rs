"""Verify compact typed Resource fixtures against immutable independent Java evidence."""
from pathlib import Path
import argparse,hashlib,json,sys
PINS={'report.json':'a7b35392e82804c581a00ccc19689c376917c4552fb46639248e06b1033e60a2','cases.json':'42106b664f5f1cb1c209d41445e9a970b7d5a78e3b6be045c55540d89deda7fd','inventory.json':'85887c02a40d83415f85138efaa299f1e08659c797e1f2e2cc2fb0e1baef3800','closure.json':'27c123fdb66fa186f782a9a9a46936e96174c13959f23b7d76e058cff61ef167'}
SELECTED=['cache-text-resource-missing-retry','resource-bytes-utf8']
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def path(root,relative):
 p=Path(relative)
 if p.is_absolute()or'..'in p.parts:raise ValueError('unsafe fixture path')
 q=root/p
 if not q.resolve().is_relative_to(root.resolve())or q.is_symlink():raise ValueError('unsafe fixture link')
 return q
def verify(root):
 m=json.loads((root/'manifest.json').read_bytes())
 if m['schemaVersion']!=1 or m['baseline']!='Pkl0.32.0'or m['provenance']!=PINS:raise ValueError('fixture provenance declaration differs')
 for name,pin in PINS.items():
  if sha(root/'provenance'/name)!=pin:raise ValueError('independent Java provenance bytes differ')
 report=json.loads((root/'provenance/report.json').read_bytes());originals=json.loads((root/'provenance/cases.json').read_bytes());inventory=json.loads((root/'provenance/inventory.json').read_bytes());index={r['path']:r for r in inventory['files']}
 if [c['id']for c in m['cases']]!=SELECTED:raise ValueError('selected fixture cases missing/reordered')
 expected=[]
 for c in m['cases']:
  original=next(v for v in originals if v['id']==c['id']);source=path(root,c['source'])
  if sha(source)!=original['sourceSha256']or c['sourceSha256']!=original['sourceSha256']or c['sourceUri']!=original['sourceUri']or c['operations']!=original['operations']:raise ValueError('source/URI/sequence identity differs')
  obs=[o for o in report['observations']if o['case']==c['id']];calls=[q for q in obs[0]['calls']if q['session']==0 and q['source']=='main.pkl']
  if len(obs)!=2 or c['primaryApiCalls']!=len(calls):raise ValueError('missing Java repeat or call')
  for i,call in enumerate(calls):
   status=call['status'].encode();ch='stdout'if status==b'success\n'else'stderr'
   if status not in [b'success\n',b'error\n']or'PklBugException'in call['errorClass']:raise ValueError('internal/unexecuted outcome cannot be ordinary fixture')
   raw=bytes.fromhex(call['utf8Hex']);units=bytes.fromhex(call['utf16BEHex'])
   if len(units)%2 or units.decode('utf-16-be','surrogatepass').encode('utf-8','replace')!=raw:raise ValueError('raw Java encodings disagree')
   before=dict(x.split('=',1)for x in call['countersBefore'].splitlines())['read'];after=dict(x.split('=',1)for x in call['countersAfter'].splitlines())['read']
   row=[c['id'],str(i),call['expression'],status.hex(),raw.hex()if ch=='stdout'else'',raw.hex()if ch=='stderr'else'',units.hex()if ch=='stdout'else'',units.hex()if ch=='stderr'else'',before,after];expected.append(row)
   for observation in obs:
    for session in [0,1]:
     other=next(v for v in observation['calls']if v['session']==session and v['prefix']==call['prefix']);otherbefore=dict(x.split('=',1)for x in other['countersBefore'].splitlines())['read'];otherafter=dict(x.split('=',1)for x in other['countersAfter'].splitlines())['read']
     if (other['status'],other['utf8Hex'],other['utf16BEHex'],otherbefore,otherafter)!=(call['status'],call['utf8Hex'],call['utf16BEHex'],before,after):raise ValueError('four Java execution payloads/readcounts differ')
     stem=other['rawPrefix'];target='stdout'if other['status']=='success\n'else'stderr';opposite='stderr'if target=='stdout'else'stdout'
     checks={'.status.bin':other['status'].encode(),'.'+target+'.bin':bytes.fromhex(other['utf8Hex']),'.'+target+'.code-units.bin':bytes.fromhex(other['utf16BEHex']),'.'+opposite+'.bin':b'','.'+opposite+'.code-units.bin':b'','.reads-before.bin':otherbefore.encode(),'.reads-after.bin':otherafter.encode()}
     for suffix,rawbytes in checks.items():
      record=index[stem+suffix]
      if record['sha256']!=hashlib.sha256(rawbytes).hexdigest()or record['bytes']!=len(rawbytes):raise ValueError('Java report raw fields disagree with pinned payload inventory')
  expected_payloads=24 if c['id']=='resource-bytes-utf8'else 0
  if len(c['payloads'])!=expected_payloads:raise ValueError('missing bytevector inputs')
  for i,p in enumerate(c['payloads']):
   q=path(root,p['path']);originalpath=Path(original['workspace'])/'payloads'/f'vector{i}.bin';pin=report['inputHashes'][str(originalpath)]
   if q.name!=f'vector{i}.bin'or sha(q)!=pin or p['sha256']!=pin or p['bytes']!=q.stat().st_size:raise ValueError('typed Resource payload bytes differ')
 actual=[line.split('\t')for line in(root/'expected.tsv').read_text().splitlines()]
 if actual!=expected:raise ValueError('expected raw sequence differs from pinned Java')
 if m['primaryCallsPerEvaluator']!=129 or len(expected)!=129 or m['capturedPrimaryCallsAcross2Jvms2EvaluatorsEach']!=516 or m['fullJavaApiCalls']!=740 or m['initialNativeConsumerUnexecutedApiCalls']!=224:raise ValueError('coverage accounting differs')
 return {'verifiedPrimarySequenceCalls':len(expected),'independentJavaExecutionsPerCall':4,'selectedJavaApiCalls':516,'unexecutedByInitialConsumer':224,'baseline':'Pkl0.32.0'}
def main():
 parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('fixture',nargs='?',type=Path,default=Path(__file__).resolve().parents[1]/'tests/fixtures/m2-resource-oracle');a=parser.parse_args()
 try:print(json.dumps(verify(a.fixture),indent=2));return 0
 except(OSError,ValueError,KeyError,StopIteration)as e:print('Resource fixture verification failed: '+str(e),file=sys.stderr);return 1
if __name__=='__main__':sys.exit(main())
