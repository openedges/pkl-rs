from pathlib import Path
import sys,json,hashlib,time,base64,concurrent.futures,subprocess
R=Path('/Users/cody.ij.hwang/Projects/pkl-rs.stage42'); O=R/'target/m2/stage42-module-extends-oracle-01';sys.path.insert(0,str(R/'tools'));import compat
sha=lambda b:hashlib.sha256(b).hexdigest()
def write(p,d):p.parent.mkdir(parents=True,exist_ok=True);p.write_text(json.dumps(d,indent=2,ensure_ascii=False)+'\n')
origin=Path('/Users/cody.ij.hwang/Projects/pkl-rs.stage41/target/m2/stage41-expression-context-oracle-01/ExpressionContextProbe.java')
helper=O/'AmendingMembersProbe.java';helper.write_bytes(origin.read_bytes().replace(b'public final class ExpressionContextProbe',b'public final class AmendingMembersProbe'))
home=O/'identity/home';tmp=O/'identity/tmp';home.mkdir(parents=True);tmp.mkdir(parents=True);jar=R/'.cache/oracle/pkl-tools-0.32.0.jar';env={'PATH':'/bin:/usr/bin','HOME':str(home),'TMPDIR':str(tmp),'LANG':'C.UTF-8','LC_ALL':'C.UTF-8','TERM':'dumb','NO_COLOR':'1','TZ':'UTC'}
def capture(name,cmd,cwd,timeout=60):
 result=compat.capture(cmd,cwd,env,timeout,O/'generated');meta=compat.store_capture(O/name,result);write(O/(name+'.process.json'),{'command':cmd,'cwd':str(cwd),'environment':env,'result':meta});return result
assert capture('javac',['/usr/bin/javac','-cp',str(jar),'-d',str(O/'classes'),str(helper)],R)['exit_code']==0
capture('jdk',['/usr/bin/java','-XshowSettings:properties','-version'],R)
base=['/usr/bin/java','-Xms64m','-Xmx512m','-Dfile.encoding=UTF-8','-Dstdout.encoding=UTF-8','-Dstderr.encoding=UTF-8','-Duser.language=en','-Duser.country=US','-Duser.timezone=UTC','-Duser.name=oracle','-Duser.home='+str(home),'-Djava.io.tmpdir='+str(tmp),'-cp',str(O/'classes')+':'+str(jar),'AmendingMembersProbe']
cases=[{'id': 'extends-open-parent-values-and-class-identity', 'source': 'extends "life:/child"\nmarker = 9\nchildOnly = 11\n', 'child': 'open module parent\nmarker = 7\nparentOnly = 3\n', 'expressions': ['marker', 'parentOnly', 'childOnly', 'this.getClass() == import("life:/child").getClass()']}]
for c in cases:
 w=O/'workspace'/c['id'];w.mkdir(parents=True);c['workspace']=str(w);c['moduleUri']=(w/'main.pkl').as_uri();c['childUri']='life:/child';c['sourceSha256']=sha(c['source'].encode());c['childSha256']=sha(c['child'].encode())
 for fn,text in [('main.pkl',c['source']),('source.pkl',c['child']),('callback.txt',''),('plan.tsv',''.join('eval\t'+base64.b64encode(e.encode()).decode()+'\n' for e in c['expressions']))]:(w/fn).write_text(text)
write(O/'cases.json',cases)
paths=[jar,helper,Path(__file__),O/'cases.json',R/'references.lock',R/'oracle.lock.json',R/'tools/compat.py']+list((O/'classes').glob('*.class'))
for c in cases:paths.extend(Path(c['workspace']).iterdir())
inputs={str(p):sha(p.read_bytes()) for p in paths};write(O/'inputs-before.json',inputs)
def execute(c):
 w=Path(c['workspace']);before=compat.snapshot(w);records=[]
 for repeat in [0,1]:
  out=O/'cases'/c['id']/str(repeat);cmd=base+[str(w),str(out/'calls'),'plain'];start=time.time_ns();result=compat.capture(cmd,w,env,30,O/'generated');after=compat.snapshot(w);meta=compat.store_capture(out,result)
  row={'id':c['id'],'repeat':repeat,'command':cmd,'cwd':str(w),'environment':env,'moduleUri':c['moduleUri'],'sourceSha256':c['sourceSha256'],'startedUnixNs':start,'finishedUnixNs':time.time_ns(),'result':meta,'workspaceBefore':before,'workspaceAfter':after};write(out/'process.json',row);records.append(row);print(json.dumps({'id':c['id'],'repeat':repeat,'exit':result['exit_code'],'protocol':result['stdout'].decode(),'processStderr':result['stderr'].decode()}),flush=True)
 return records
with concurrent.futures.ThreadPoolExecutor(max_workers=1) as pool:records=[r for group in pool.map(execute,cases) for r in group]
observations=[];repeatdiff=[];rawinventory=[]
for c in cases:
 payloads=[]
 for repeat in [0,1]:
  f=O/'cases'/c['id']/str(repeat)/'calls';payloads.append({p.name:p.read_bytes() for p in f.glob('*.bin')});calls=[]
  for p in f.glob('*.bin'):rawinventory.append({'path':str(p.relative_to(O)),'bytes':p.stat().st_size,'sha256':sha(p.read_bytes())})
  for i,e in enumerate(c['expressions']):
   pre=f'outer-{i}';status=(f/(pre+'.status.bin')).read_bytes();ch='stdout' if status==b'success\n' else 'stderr';b=(f/(pre+'.'+ch+'.bin')).read_bytes();u=(f/(pre+'.'+ch+'.code-units.bin')).read_bytes()
   calls.append({'expression':e,'status':status.decode(),'errorClass':(f/(pre+'.error-class.bin')).read_text(),'utf8Sha256':sha(b),'utf16BESha256':sha(u),'text':b.decode(),'rawPrefix':str(f/pre)})
  observations.append({'id':c['id'],'repeat':repeat,'calls':calls,'counters':(f/'counters.bin').read_text()})
 if payloads[0]!=payloads[1]:repeatdiff.append({'id':c['id'],'names':[n for n in payloads[0].keys()|payloads[1].keys() if payloads[0].get(n)!=payloads[1].get(n)]})
changed=[p for p,h in inputs.items() if sha(Path(p).read_bytes())!=h]
write(O/'raw-inventory.json',rawinventory)
write(O/'report.json',{'scope':'One fixed fresh pinned Java successful module-extends API graph with four unchanged ordered expressions, two fresh JVMs per source. Fixed complete input set retained; no Rust evaluation or compatibility claim.','caseCount':len(cases),'processCount':len(records),'records':records,'observations':observations,'repeatDifferences':repeatdiff,'inputHashes':inputs,'inputChanges':changed,'sourceArtifactStability':not changed,'maxConcurrentProcesses':1,'profile':{'api':'evaluateExpressionString(ModuleSource.path(main.pkl), exact expression)','moduleCache':'one evaluator retained across calls, fresh evaluator/JVM per repeat','evaluator':'preconfigured;color=false;emptyenvprop;defaultmodulefactories+lifeSPI;defaultstackframetransformer;real Logger','helperOrigin':str(origin),'helperOriginSha256':sha(origin.read_bytes()),'helperChange':'Class name only','utf16':'actual Java chars via DataOutputStream.writeChar, separate untouched UTF8 getBytes payloads'}})
print(json.dumps({'reportSha256':sha((O/'report.json').read_bytes()),'repeatDifferences':repeatdiff,'inputChanges':changed}),flush=True)
