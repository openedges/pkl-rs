"""Read already captured Java channels/effects only; no evaluator execution."""
from pathlib import Path
import collections,hashlib,json
O=Path(__file__).resolve().parent
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
report=json.loads((O/'report.json').read_bytes());cases={c['id']:c for c in json.loads((O/'cases.json').read_bytes())}
checks=collections.Counter();repeats=[];intent=collections.Counter();eventsTotal=0
for rec in report['processes']:
 assert rec['exitCode']==0 and not rec['timedOut'];case=cases[rec['case']]
 for session in range(2):
  raw=O/'cases'/rec['case']/str(rec['repeat'])/'raw'/('session-'+str(session))
  n=int((raw/'session-counts.bin').read_text().split('\t')[3]);events=[]
  for i in range(n):
   p='event-'+str(i);kind=(raw/(p+'.kind.bin')).read_text();ci=int((raw/(p+'.call-index.bin')).read_text());events.append((kind,ci))
  eventsTotal+=n
  # Every independently captured UTF16 field must encode to the separately captured actual Java UTF8 bytes.
  for p in raw.iterdir():
   if p.name.endswith('.utf16be.bin'):
    original=raw/(p.name[:-len('.utf16be.bin')]+'.bin');units=p.read_bytes();assert len(units)%2==0
    assert units.decode('utf-16-be','surrogatepass').encode('utf-8','replace')==original.read_bytes(),p;checks['independentStringEncodingPairs']+=1
  calls=[c for c in rec['calls'] if c['session']==session]
  exprs=[op['value']for op in case['operations']if op['kind']=='evaluate'];assert len(calls)==len(exprs)
  for c,expression in zip(calls,exprs):
   i=c['index'];p='call-'+str(i);assert c['expression']==expression==(raw/(p+'.expression.bin')).read_text()
   for field,name in [('stdoutHex','stdout'),('stderrHex','stderr'),('statusHex','status')]:assert c[field]==(raw/(p+'.'+name+'.bin')).read_bytes().hex()
   for field,name in [('stdoutUTF16BEHex','stdout'),('stderrUTF16BEHex','stderr')]:assert c[field]==(raw/(p+'.'+name+'.utf16be.bin')).read_bytes().hex()
   assert c['status']in ['success\n','error\n','oracle-error\n','api-error\n']
   before=c['countsBefore'];after=c['countsAfter'];assert events[before[2]][0]=='api-call' and events[after[2]-1][0]=='api-result'
   assert all(ci==i for _,ci in events[before[2]:after[2]])
   assert before[:2]==[sum(k=='reader-call'for k,_ in events[:before[2]]),sum(k=='logger'for k,_ in events[:before[2]])]
   assert after[:2]==[sum(k=='reader-call'for k,_ in events[:after[2]]),sum(k=='logger'for k,_ in events[:after[2]])]
   scope='intended-collection-control'
   if rec['case']=='catch-boundaries' or (rec['case']=='callback-types-first-error'and i in [1,2,3,4]):scope='authored-intent-not-reached'
   elif c['status']=='api-error\n':scope='post-close-public-api-boundary'
   intent[scope]+=1;checks['rawApiRows']+=1
   # Compare repeated strings raw; post-close executor identities are not normalized or hidden.
   if rec['repeat']!=0 or session!=0:
    initial=next(r for r in report['processes']if r['case']==rec['case']and r['repeat']==0)['calls'][i]
    fields=['statusHex','stdoutHex','stderrHex','stdoutUTF16BEHex','stderrUTF16BEHex','errorClass','messageNull','countsBefore','countsAfter']
    diff=[f for f in fields if c[f]!=initial[f]]
    if diff:repeats.append({'case':rec['case'],'repeat':rec['repeat'],'session':session,'index':i,'fields':diff,'scope':scope})
assert checks['rawApiRows']==940
assert all(x['scope']=='post-close-public-api-boundary'for x in repeats),repeats
original=O/'workspace/original-source-code/input.pkl';binding=json.loads((O/'original-binding.json').read_bytes());assert sha(original)==binding['sha256']==sha(Path(binding['path']))
expected=(O/'cases/original-source-code/0/raw/session-0/call-0.stdout.bin').read_bytes()
for repeat in range(2):
 p=O/'original-adapter'/str(repeat);assert (p/'stdout.bin').read_bytes()==expected and (p/'stderr.bin').read_bytes()==b'' and (p/'outcome.bin').read_bytes()==b'success\n'
for p,row in json.loads((O/'inputs-before.json').read_bytes()).items():assert sha(Path(p))==row['sha256']
v={'scope':'Independent capture readback only; Java oracle coverage, not Rust compatibility.','checks':checks,'orderedActualEvents':eventsTotal,'scopeLedger':intent,'originalAdapterFullRawMatches':2,'repeatDifferences':repeats,'qualification':'Post-close timeout executor RejectedExecutionException includes per-JVM object identity strings; retain each actual string, no normalization/ordinary compatibility claim. Catch parser24 and bare method ref16 are original authored intent failures; require fresh corrected root.','sourceInputChanges':[]}
(O/'readback-report.json').write_text(json.dumps(v,indent=2)+'\n');print(json.dumps(v,indent=2))
