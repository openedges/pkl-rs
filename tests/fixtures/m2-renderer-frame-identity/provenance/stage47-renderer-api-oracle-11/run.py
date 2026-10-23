from pathlib import Path
import collections,hashlib,json,platform,shutil,sys,time
sys.dont_write_bytecode=True
R=Path('/Users/cody.ij.hwang/Projects/pkl-rs.stage47');O=Path(__file__).parent
sys.path.insert(0,str(R/'tools'));import compat,oracle
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
def write(p,v):p.parent.mkdir(parents=True,exist_ok=True);p.write_text(json.dumps(v,indent=2,ensure_ascii=True)+'\n')
def capture(name,command,cwd,env,timeout=60):
 start=time.time_ns()
 try:v=compat.capture(command,cwd,env,timeout,O/'generated')
 except compat.CaptureTimeout as e:v=e.result
 row={'command':command,'cwd':str(cwd),'environment':env,'startedUnixNs':start,'finishedUnixNs':time.time_ns(),'result':compat.store_capture(O/name,v)};write(O/(name+'.process.json'),row);return row,v
lock=json.loads((R/'oracle.lock.json').read_bytes());oracle.verify_sources(R,lock)
jar=Path('/Users/cody.ij.hwang/Projects/pkl-rs.stage42/.cache/oracle/pkl-tools-0.32.0.jar');release=oracle.verify_artifact(jar,lock);env=oracle.deterministic_environment(O/'identity');jdk=oracle.verify_java(Path('/usr/bin/java'),lock,env)
write(O/'profile.json',{'jar':str(jar),'jarSha256':sha(jar),'jdk':jdk,'release':release,'environment':env,'jvmOptions':lock['jvm_options'],'evaluator':'preconfigured,cache=null,color=false,empty env+props,power=true;actual StandardSecurityManager delegate with ordered counting wrapper;one custom hierarchical res reader. Two sequential fresh evaluators per freshJVM; each explicit module source path in operations. GC is an explicit System.gc request only.'})
row,result=capture('javac',['/usr/bin/javac','-cp',str(jar),'-d',str(O/'classes'),str(O/'RendererProbe.java')],R,env);assert result['exit_code']==0
sources=['pkl-core/src/main/java/org/pkl/core/runtime/ResourceManager.java','pkl-core/src/main/java/org/pkl/core/resource/Resource.java','pkl-core/src/main/java/org/pkl/core/resource/ResourceReaders.java','pkl-core/src/main/java/org/pkl/core/SecurityManagers.java','pkl-core/src/main/java/org/pkl/core/ast/expression/unary/AbstractReadNode.java','pkl-core/src/main/java/org/pkl/core/util/IoUtils.java','pkl-core/src/main/java/org/pkl/core/module/ModuleKeys.java','pkl-core/src/main/java/org/pkl/core/ast/builder/AstBuilder.java','stdlib/base.pkl','stdlib/json.pkl','stdlib/ref.pkl','pkl-core/src/main/java/org/pkl/core/stdlib/AbstractRenderer.java','pkl-core/src/main/java/org/pkl/core/stdlib/PklConverter.java','pkl-core/src/main/java/org/pkl/core/stdlib/base/JsonRendererNodes.java','pkl-core/src/main/java/org/pkl/core/stdlib/base/PcfRendererNodes.java','pkl-core/src/main/java/org/pkl/core/util/json/JsonEscaper.java','pkl-core/src/main/java/org/pkl/core/runtime/VmCollection.java','pkl-core/src/main/java/org/pkl/core/runtime/VmObject.java','pkl-core/src/main/java/org/pkl/core/runtime/VmPair.java','pkl-core/src/main/java/org/pkl/core/runtime/VmReference.java','pkl-core/src/main/java/org/pkl/core/runtime/VmBytes.java']
sources.extend(['pkl-core/src/main/java/org/pkl/core/ast/type/TypeNode.java','pkl-core/src/main/java/org/pkl/core/ast/frame/WriteFrameSlotNode.java','pkl-core/src/main/java/org/pkl/core/ast/frame/ReadFrameSlotNode.java','pkl-core/src/main/java/org/pkl/core/ast/member/FunctionNode.java','pkl-core/src/main/java/org/pkl/core/ast/member/UnresolvedFunctionNode.java','pkl-core/src/main/java/org/pkl/core/runtime/VmUtils.java','pkl-parser/src/main/java/org/pkl/parser/ParserImpl.java'])
sources.extend(['pkl-core/src/main/java/org/pkl/core/stdlib/base/StringNodes.java', 'pkl-core/src/main/java/org/pkl/core/ast/expression/generator/GeneratorForNode.java', 'pkl-core/src/main/java/org/pkl/core/ast/expression/generator/RestoreForBindingsNode.java', 'pkl-core/src/main/java/org/pkl/core/EvaluatorBuilder.java'])
sources.extend(['pkl-core/src/main/java/org/pkl/core/ast/expression/literal/EmptyObjectLiteralNode.java', 'pkl-core/src/main/java/org/pkl/core/ast/expression/literal/PropertiesLiteralNode.java', 'pkl-core/src/main/java/org/pkl/core/ast/member/ObjectMember.java', 'pkl-core/src/main/java/org/pkl/core/ast/ConstantValueNode.java', 'pkl-core/src/main/java/org/pkl/core/ast/expression/literal/IntLiteralNode.java', 'pkl-core/src/main/java/org/pkl/core/ast/expression/literal/FloatLiteralNode.java'])
for n in sources:p=O/'upstream'/n;p.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(R/'external/pkl'/n,p)
for n in ['tools/compat.py','tools/oracle.py','references.lock','oracle.lock.json']:p=O/'tool-sources'/n;p.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(R/n,p)
cases=json.loads((O/'cases.json').read_bytes())
inputs=[O/n for n in ['RendererProbe.java','run.py','cases.json','profile.json']]+[p for p in (O/'classes').iterdir() if p.is_file()]+[O/'upstream'/n for n in sources]+[O/'tool-sources'/n for n in ['tools/compat.py','tools/oracle.py','references.lock','oracle.lock.json']]+[jar]
for case in cases:
 work=Path(case['workspace']);inputs.extend(p for p in work.rglob('*') if p.is_file())
before={str(p):sha(p) for p in inputs};write(O/'inputs-before.json',before)
records=[];observations=[];repeat_differences=[]
for case in cases:
 work=Path(case['workspace']);baseline=compat.snapshot(work);repeats=[]
 for repeat in [0,1]:
  private=O/'runtime-private'/case['id']/str(repeat);(private/'home').mkdir(parents=True);(private/'tmp').mkdir();actualenv={**env,'HOME':str(private/'home'),'TMPDIR':str(private/'tmp'),'TMP':str(private/'tmp'),'TEMP':str(private/'tmp')};target=O/'cases'/case['id']/str(repeat)
  command=['/usr/bin/java',*lock['jvm_options'],'-Duser.home='+str(private/'home'),'-Djava.io.tmpdir='+str(private/'tmp'),'-cp',str(O/'classes')+':'+str(jar),'RendererProbe',str(work),str(target/'calls'),case['mode']]
  row,value=capture(str(target.relative_to(O)),command,work,actualenv);row.update(case=case['id'],repeat=repeat,workspaceBefore=baseline,workspaceAfter=compat.snapshot(work));write(target/'process.json',row);records.append(row)
  assert value['exit_code']==0 and not value['timed_out'],row
  calls=[]
  for session in [0,1]:
   directory=target/'calls'/('session-'+str(session));index=0
   for op in case['operations']:
    if op in ['GC','CLOSE']:continue
    prefix='outer-'+str(index);index+=1;status=(directory/(prefix+'.status.bin')).read_text();channel='stdout' if status=='success\n' else 'stderr';raw=directory/(prefix+'.'+channel+'.bin');units=directory/(prefix+'.'+channel+'.code-units.bin')
    assert units.read_bytes().decode('utf-16-be','surrogatepass').encode('utf-8','replace')==raw.read_bytes()
    calls.append({'session':session,'prefix':prefix,'source':op[0],'expression':op[1],'status':status,'errorClass':(directory/(prefix+'.error-class.bin')).read_text(),'rawPrefix':str((directory/prefix).relative_to(O)),'utf8Sha256':sha(raw),'utf16BESha256':sha(units),'utf8Hex':raw.read_bytes().hex(),'utf16BEHex':units.read_bytes().hex(),'countersBefore':(directory/(prefix+'.counters-before.bin')).read_text(),'countersAfter':(directory/(prefix+'.counters-after.bin')).read_text()})
  observations.append({'case':case['id'],'repeat':repeat,'calls':calls});repeats.append({str(p.relative_to(target/'calls')):sha(p) for p in (target/'calls').rglob('*.bin')});write(O/'progress.json',{'completedProcesses':len(records),'calls':len(calls),'lastCase':case['id'],'repeat':repeat});print(json.dumps({'case':case['id'],'repeat':repeat,'calls':len(calls),'counts':dict(collections.Counter(c['status']for c in calls))}),flush=True)
 differences=[p for p in sorted(repeats[0].keys()|repeats[1].keys()) if repeats[0].get(p)!=repeats[1].get(p)];repeat_differences.append({'case':case['id'],'rawPathDifferences':differences})
after={str(p):sha(p) for p in inputs};write(O/'inputs-after.json',after)
report={'scope':'Pinned Java continuous renderer API/options/hints/UTF16/directive observations only;not native compatibility. Original whole input remains separate unchanged. Same-evaluator secondModuleSource and fresh-evaluator sessions are separate;explicitGC request does not establish observed collection.','freshJvmCount':len(records),'evaluatorSessions':len(records)*2,'apiCalls':sum(len(o['calls']) for o in observations),'counts':dict(collections.Counter(c['status'] for o in observations for c in o['calls'])),'records':records,'observations':observations,'repeatDifferences':repeat_differences,'inputHashes':before,'inputChanges':[p for p in before if before[p]!=after[p]],'workspacesUnchanged':all(r['workspaceBefore']==r['workspaceAfter']for r in records),'jarStable':sha(jar)==before[str(jar)],'platform':platform.platform()};write(O/'report.json',report);print(json.dumps({'apiCalls':report['apiCalls'],'counts':report['counts'],'inputChanges':report['inputChanges'],'repeatDifferences':repeat_differences}),flush=True);assert not report['inputChanges']and report['jarStable']and report['workspacesUnchanged']
