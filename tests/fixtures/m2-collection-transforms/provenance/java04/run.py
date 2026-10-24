from pathlib import Path
import collections,datetime,hashlib,json,os,platform,re,shutil,subprocess,sys,time
O=Path(__file__).resolve().parent;R=O.parents[2];sys.dont_write_bytecode=True;sys.path.insert(0,str(R/'tools'));import oracle
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
def write(p,v):p.parent.mkdir(parents=True,exist_ok=True);p.write_text(json.dumps(v,ensure_ascii=False,indent=2)+'\n')
def capture(name,cmd,cwd,env,timeout=120):
 p=O/name;p.mkdir(parents=True,exist_ok=False);row={'command':cmd,'cwd':str(cwd),'environment':env,'parentPid':os.getpid(),'startedUtc':datetime.datetime.now(datetime.timezone.utc).isoformat()}
 with (p/'stdout.bin').open('wb') as out,(p/'stderr.bin').open('wb') as err:
  proc=subprocess.Popen(cmd,cwd=cwd,env=env,stdout=out,stderr=err);row['pid']=proc.pid;write(p/'launch.json',row)
  try:row['exitCode']=proc.wait(timeout=timeout);row['timedOut']=False
  except subprocess.TimeoutExpired:proc.kill();row['exitCode']=proc.wait();row['timedOut']=True
 row.update(finishedUtc=datetime.datetime.now(datetime.timezone.utc).isoformat(),stdoutSha256=sha(p/'stdout.bin'),stderrSha256=sha(p/'stderr.bin'));write(p/'terminal.json',row);return row
lock=json.loads((R/'oracle.lock.json').read_bytes());jar=R/'.cache/oracle/pkl-tools-0.32.0.jar';release=oracle.verify_artifact(jar,lock)
def envfor(name):
 p=O/'runtime-private'/name
 for sub in ['home','tmp']: (p/sub).mkdir(parents=True,exist_ok=False)
 return {'PATH':os.defpath,'HOME':str(p/'home'),'TMPDIR':str(p/'tmp'),'TMP':str(p/'tmp'),'TEMP':str(p/'tmp'),'LC_ALL':'C','LANG':'C','TZ':'UTC','TERM':'dumb','NO_COLOR':'1','COLUMNS':'80'}
identityEnv=envfor('identity');identity=capture('identity',['/usr/bin/java','-XshowSettings:properties','-version'],R,identityEnv)
props=dict(re.findall(r'^\s+([\w.]+) = (.*)$',(O/'identity/stderr.bin').read_text(),re.MULTILINE));assert identity['exitCode']==0
for k,v in lock['jdk']['properties'].items():assert props.get(k)==v,(k,props.get(k),v)
jhome=Path(props['java.home']);profile={'sourcePin':'e51fb2f91774265666f94980ec630db794a4db2d','oracleVersion':'0.32.0','oracleJar':str(jar),'oracleJarSha256':sha(jar),'release':release,'jdkProperties':props,'runtimeFiles':{str(p):{'sha256':sha(p),'bytes':p.stat().st_size} for p in [Path('/usr/bin/java'),Path('/usr/bin/javac'),jhome/'bin/java',jhome/'bin/javac',jhome/'lib/modules',jhome/'release']},'powerAssertions':False,'color':False,'moduleCacheDir':None,'traceMode':'COMPACT','stackFrameTransformer':'preconfigured default (not replaced)','evaluatorApi':'evaluateExpressionString(ModuleSource.path(exact copied source), expression)','sameJvmSessions':2,'freshJvmRepeats':2,'logger':'Actual Logger.trace/warn raw message/frame UTF16 and JavaUTF8; no callback reentry','reader':'Explicit ct URI table; successful Text cached by ordinary ResourceManager; io-first throws configured IOException only at first actual URI read per evaluator. No Host semantic cache.','gc':'System.gc request only; not proof of GC completion; distinct from Rust per-poll GC','close':'Actual evaluator.close API, including post-close evaluation where declared; API/internal errors remain separate','scope':'Bounded collection transform oracle, not full stdlib, Java fallback or Rust comparison.'};write(O/'profile.json',profile)
up=['stdlib/base.pkl','pkl-core/src/main/java/org/pkl/core/stdlib/base/ListNodes.java','pkl-core/src/main/java/org/pkl/core/stdlib/base/SetNodes.java','pkl-core/src/main/java/org/pkl/core/stdlib/base/MapNodes.java','pkl-core/src/main/java/org/pkl/core/stdlib/base/BaseNodes.java','pkl-core/src/main/java/org/pkl/core/ast/lambda/ApplyVmFunction1Node.java','pkl-core/src/main/java/org/pkl/core/ast/lambda/ApplyVmFunction2Node.java','pkl-core/src/main/java/org/pkl/core/ast/type/TypeNode.java','pkl-core/src/main/java/org/pkl/core/runtime/VmList.java','pkl-core/src/main/java/org/pkl/core/runtime/VmSet.java','pkl-core/src/main/java/org/pkl/core/runtime/VmMap.java','pkl-core/src/main/java/org/pkl/core/runtime/VmTyped.java','pkl-core/src/main/java/org/pkl/core/runtime/VmDynamic.java','pkl-core/src/main/java/org/pkl/core/runtime/VmPair.java','pkl-core/src/main/java/org/pkl/core/runtime/VmNull.java','pkl-core/src/main/java/org/pkl/core/runtime/ResourceManager.java','pkl-core/src/main/java/org/pkl/core/Logger.java','pkl-core/src/main/java/org/pkl/core/StackFrame.java','pkl-core/src/main/java/org/pkl/core/resource/ResourceReader.java']
for rel in up:
 dest=O/'upstream'/rel;dest.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(R/'external/pkl'/rel,dest)
for rel in ['tools/oracle.py','oracle.lock.json','references.lock','tools/java/M2LanguageProbe.java']:
 dest=O/'tool-sources'/rel;dest.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(R/rel,dest)
(O/'classes').mkdir();compiled=capture('javac',['/usr/bin/javac','-cp',str(jar),'-d',str(O/'classes'),str(O/'CollectionProbe.java'),str(O/'tool-sources/tools/java/M2LanguageProbe.java')],R,identityEnv)
if compiled['exitCode']!=0:write(O/'compile-failure.json',compiled);raise SystemExit(compiled['exitCode'])
cases=json.loads((O/'cases.json').read_bytes())
inputs=[O/n for n in ['CollectionProbe.java','prepare.py','run.py','launch.py','cases.json','original-binding.json','profile.json','PLAN.ko.md']]
inputs += [Path(c['workspace'])/n for c in cases for n in ['input.pkl','operations.tsv','reader-plan.tsv']]
inputs += [O/'upstream'/p for p in up]+[O/'tool-sources'/p for p in ['tools/oracle.py','oracle.lock.json','references.lock','tools/java/M2LanguageProbe.java']]+list((O/'classes').iterdir())+[jar,jhome/'bin/java',jhome/'bin/javac',jhome/'lib/modules',jhome/'release',Path(json.loads((O/'original-binding.json').read_bytes())['path'])]
def snapshot():return {str(p):{'sha256':sha(p),'bytes':p.stat().st_size} for p in inputs}
before=snapshot();write(O/'inputs-before.json',before)
base=[] # Original adapter baseline reused from sealed01; no repeated acquisition.
records=[];callcounts=collections.Counter()
for case in cases:
 for repeat in range(2):
  env=envfor(case['id']+'-'+str(repeat));caseOut=O/'cases'/case['id']/str(repeat)
  cmd=['/usr/bin/java',*lock['jvm_options'],'-Duser.home='+env['HOME'],'-Djava.io.tmpdir='+env['TMPDIR'],'-cp',str(O/'classes')+os.pathsep+str(jar),'CollectionProbe',case['source'],case['workspace']+'/operations.tsv',case['workspace']+'/reader-plan.tsv',str(caseOut/'raw')]
  row=capture('cases/'+case['id']+'/'+str(repeat),cmd,Path(case['workspace']),env);row.update(case=case['id'],repeat=repeat,calls=[])
  if row['exitCode']==0:
   for session in range(2):
    raw=caseOut/'raw'/('session-'+str(session));index=0
    for op in case['operations']:
     if op['kind']!='evaluate':continue
     prefix='call-'+str(index);read=lambda n:(raw/(prefix+'.'+n+'.bin')).read_bytes()
     status=read('status').decode();callcounts[status]+=1
     call={'session':session,'index':index,'expression':op['value'],'status':status,'statusHex':read('status').hex(),'stdoutHex':read('stdout').hex(),'stderrHex':read('stderr').hex(),'stdoutUTF16BEHex':(raw/(prefix+'.stdout.utf16be.bin')).read_bytes().hex(),'stderrUTF16BEHex':(raw/(prefix+'.stderr.utf16be.bin')).read_bytes().hex(),'errorClass':read('error-class').decode(),'messageNull':read('message-null').decode(),'countsBefore':[int(x)for x in read('counts-before').decode().split('\t')],'countsAfter':[int(x)for x in read('counts-after').decode().split('\t')],'rawPrefix':str(raw/prefix)}
     row['calls'].append(call);index+=1
  records.append(row);write(O/'progress.json',{'completedProcesses':len(records),'apiCalls':sum(callcounts.values()),'statusCounts':callcounts});print('closed '+case['id']+' repeat'+str(repeat)+' exit'+str(row['exitCode']),flush=True)
after=snapshot();write(O/'inputs-after.json',after);changes=[p for p in before if before[p]!=after[p]];assert not changes,changes
report={'scope':profile['scope'],'originalAdapterProcesses':base,'processes':records,'plannedApiCalls':156,'acquiredApiCalls':sum(callcounts.values()),'statusCounts':callcounts,'caseCount':len(cases),'freshEvaluatorProcesses':len(records),'sameJvmSessions':2,'sourceInputChanges':changes,'rawEncodings':'Independent actual Java String.getBytes(UTF_8) and writeChar UTF16BE for API channels and event fields. No Rust expected values.','unexecutedBoundary':'Rust poll/reentry/cancel/timeouts/stale tokens and new native/Node/Wasm comparisons remain unexecuted; post-close errors are public API boundary, not ordinary collection compatibility.','profileSha256':sha(O/'profile.json')};write(O/'report.json',report)
print(json.dumps({'acquiredApiCalls':sum(callcounts.values()),'statusCounts':callcounts,'processExitCodes':[p['exitCode']for p in records],'sourceInputChanges':changes}),flush=True)
