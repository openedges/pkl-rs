"""Serial bounded pinned Java authority; preserves all original authored queries."""
from __future__ import annotations
import argparse, base64, collections, hashlib, json, os, platform, re, shutil, signal, stat, subprocess, sys, time, traceback
from pathlib import Path
PREP=Path(__file__).resolve().parent
ROOT=PREP.parents[2]
OUT=CANDIDATE=None
COHORTS=PREP/'cohorts.json'
COHORTS_SHA='b25317beaee0160770b6a02daa3212356de651ebfdbfa1af297d7f83083dc6d1'
sys.path.insert(0,str(ROOT/'target/m2/stage98-platform-prep-01'))
from owned_processes import OwnedProcesses
HELPER=PREP/'helper-false/CollectionEveryOracle.java'
ARTIFACT=ROOT/'target/m2/stage95-plist-oracle-prep-03/artifact-inputs.json'
FLOOR=1024*1024**2
CAP=512*1024**2

def sha(p):
 h=hashlib.sha256()
 with Path(p).open('rb') as f:
  for b in iter(lambda:f.read(1024*1024),b''):h.update(b)
 return h.hexdigest()
def identity(p):
 p=Path(p);s=p.stat();return dict(path=str(p),bytes=s.st_size,mode=stat.S_IMODE(s.st_mode),sha256=sha(p))
def save(p,x):Path(p).write_text(json.dumps(x,indent=2,sort_keys=True)+'\n')
def own_start(pid):return subprocess.run(['/bin/ps','-p',str(pid),'-o','lstart='],capture_output=True,check=False).stdout.decode().strip()
def usage():
 allocated=logical=0
 for folder in [PREP,OUT]:
  if folder.exists():
   allocated+=folder.stat().st_blocks*512
   for p in folder.rglob('*'):
    try:s=p.lstat()
    except FileNotFoundError:continue # JVM transient cache entry vanished after enumeration.
    allocated+=s.st_blocks*512
    if stat.S_ISREG(s.st_mode):logical+=s.st_size
 return dict(free_bytes=shutil.disk_usage(ROOT).free,allocated_bytes=allocated,logical_bytes=logical)
def readrow(folder,q):
 status=(folder/'result.status.bin').read_bytes()
 if status not in [b'success\n',b'error\n',b'oracle-error\n',b'api-exception\n']:raise ValueError('status')
 count=(folder/'logs.count.bin').read_bytes()
 if not re.fullmatch(rb'[0-9]+\n',count):raise ValueError('logs count')
 n=int(count);names=['mode','expression','result.stdout','result.stderr','result.error-class','result.exception-to-string','result.cause-class','result.cause-message','result.cause-to-string','result.java-type']
 names += [f'log-{i:03d}.{suffix}' for i in range(n) for suffix in ['level','message','module-uri','member','position']]
 expected={'result.status.bin','logs.count.bin'}
 for name in names:
  raw=(folder/(name+'.bin')).read_bytes();units=(folder/(name+'.code-units.bin')).read_bytes()
  decoded=units.decode('utf-16-be',errors='surrogatepass')
  if decoded.encode('utf-8',errors='replace')!=raw:raise ValueError('UTF8/UTF16 '+name)
  expected.update([name+'.bin',name+'.code-units.bin'])
 if {p.name for p in folder.iterdir()}!=expected:raise ValueError('row files')
 if (folder/'expression.bin').read_bytes()!=q['expression'].encode() or (folder/'mode.bin').read_bytes()!=b'string':raise ValueError('row expression')
 return dict(id=q['id'],expression=q['expression'],status=status.decode().rstrip('\n'),logger_events=n,directory=str(folder.relative_to(ROOT)),raw_files={name:identity(folder/name) for name in sorted(expected)})
def main():
 global OUT,CANDIDATE
 parser=argparse.ArgumentParser(description=__doc__)
 parser.add_argument('--cohort',choices=['regex'],required=True)
 parser.add_argument('--root-window-authorized',action='store_true')
 args=parser.parse_args()
 if not args.root_window_authorized:raise ValueError('explicit root Java window handoff required')
 if sha(COHORTS)!=COHORTS_SHA:raise ValueError('sealed cohort transport policy changed')
 profile=json.loads(COHORTS.read_bytes())[args.cohort]
 OUT=ROOT/profile['output'];CANDIDATE=ROOT/profile['matrix']
 if shutil.disk_usage(ROOT).free<FLOOR+CAP:raise ValueError('need512MiB owned budget plus1GiB disk reserve before starting')
 if OUT.exists():raise ValueError('fresh output required')
 OUT.mkdir()
 report=dict(schema_version=1,kind='Stage110-regex-PCF-pinned-Java-authority',cohort=args.cohort,contracts=profile['contracts'],planned_sessions=profile['programs']*2,acquisition_mode='root-authorized serial pinned Java',canonical_Rust_source_binding=False,complete=False,started_epoch=time.time(),sessions=[],events=[],platform=platform.platform(),acquisition_started=False,Rust_evaluations=0,resource_policy=dict(live_floor_bytes=FLOOR,owned_cap_bytes=CAP,serial_Java_limit=1,watchdog_seconds=180,poll_seconds=0.1),power_assertions_enabled="per-program",powerfalse_queries=profile['contracts']-profile['power_true'],powertrue_queries=profile['power_true'],evaluator_environment={},evaluator_external_properties={},expression_wire_encoding='base64-of-exact-original-UTF8')
 minimum=None;peak=0
 def guard():
  nonlocal minimum,peak
  u=usage()
  if time.time()-report['started_epoch']>3600:raise RuntimeError('total acquisition3600s watchdog')
  minimum=u['free_bytes'] if minimum is None else min(minimum,u['free_bytes']);peak=max(peak,u['allocated_bytes'],u['logical_bytes'])
  if u['free_bytes']<FLOOR or max(u['allocated_bytes'],u['logical_bytes'])>CAP:raise RuntimeError('resource bound '+str(u))
  return u
 try:
  if sha(CANDIDATE)!=profile['matrix_sha256']:raise ValueError('sealedmatrixpin')
  document=json.loads(CANDIDATE.read_bytes());artifact=json.loads(ARTIFACT.read_bytes());lock=json.loads((ROOT/'oracle.lock.json').read_bytes())
  programs=document['programs'];report['programs']=programs
  if sum(len(p['queries']) for p in programs if p['power_assertions'])!=profile['power_true'] or len(programs)!=profile['programs']:raise ValueError('Regex power/program count')
  if sum(len(p['queries']) for p in programs)!=profile['contracts']:raise ValueError('Regex cohort count')
  report['preserved_historical_authorities']=json.loads((PREP/'historical-originals.json').read_text())
  report['auxiliary_inputs']=profile.get('dependencies',[])
  for dependency in report['auxiliary_inputs']:
   dep=ROOT/dependency['path']; actual=identity(dep)
   if dep.is_symlink() or not dep.is_file() or actual['mode']!=dependency['mode'] or actual['sha256']!=dependency['sha256'] or actual['bytes']!=dependency['bytes'] or dep.as_uri()!=dependency['source_uri']:raise ValueError('auxiliary module admission failed')
  for p in programs:
   if p['repetitions']!=2:raise ValueError('repetitions')
   for q in p['queries']:
    if q['expected'] is not None or q['executed'] or not re.fullmatch('[a-z0-9-]+',q['id']):raise ValueError('authored query contract')
  if sha(HELPER)!='b01c67a9c5789abff436227a4dad632d6d1bb69a7af8506f3d7af2c632485399':raise ValueError('helper pin')
  jar=ROOT/artifact['jar']['path'];java=Path(artifact['jdk_tools']['jdk_java']['path'])
  if sha(jar)!=lock['artifact']['sha256']:raise ValueError('jar pin')
  for v in artifact['jdk_tools'].values():
   if sha(v['path'])!=v['sha256']:raise ValueError('JDK pin '+v['path'])
  paths=[Path(__file__),PREP/'PLAN.md',CANDIDATE,HELPER,ARTIFACT,ROOT/'oracle.lock.json',ROOT/'references.lock',jar]+[Path(v['path']) for v in artifact['jdk_tools'].values()]
  paths += [COHORTS,PREP/'CollectionEveryOracle.java',PREP/'preparation.json',PREP/'transport-roundtrip.json',ROOT/'target/m2/stage98-platform-prep-01/owned_processes.py',PREP/'historical-originals.json']
  paths += [ROOT/'external/pkl/stdlib/base.pkl']+[ROOT/'external/pkl/pkl-core/src/main/java/org/pkl/core/runtime'/n for n in ['VmClass.java','VmTyped.java','VmObject.java','VmUtils.java','ModuleCache.java']]
  paths += [ROOT/'external/pkl/pkl-core/src/main/java/org/pkl/core'/n for n in ['EvaluatorBuilder.java','SecurityManagers.java','module/ModuleKeyFactories.java','stdlib/base/TypedNodes.java','stdlib/base/MapNodes.java','stdlib/base/DynamicNodes.java','ast/builder/AstBuilder.java']]
  paths += [ROOT/'external/pkl/pkl-core/src/main/resources/org/pkl/core/errorMessages.properties', ROOT/'external/pkl/pkl-core/src/main/java/org/pkl/core/ast/builder/SymbolTable.java']
  paths += [ROOT/item['source']['path'] for item in report['preserved_historical_authorities']['originals']]
  paths += [ROOT/p['path'] for item in report['preserved_historical_authorities']['originals'] for p in item['historical_Java_CLI']]
  paths += [ROOT/'external/pkl/pkl-core/src/main/java/org/pkl/core'/n for n in ['ValueFormatter.java','runtime/VmRegex.java','stdlib/base/PcfRenderer.java']]
  paths += [PREP/'module.pkl',PREP/'pattern-index.json',ROOT/'target/m2/stage110-regex-pcf-candidate-01/evidence-pins.json',ROOT/'target/m2/stage110-regex-pcf-candidate-01/copy-plan.json']
  if sha(PREP/'CollectionEveryOracle.java')!='56defcb9f990729eedc3a3b98854c07319859a683cae4a404fedaf821cc2a98f':raise ValueError('truehelperpin')
  report['input_before']={str(p):identity(p) for p in paths}
  report['admission']=guard()
  report['concurrency']=dict(root_serialized_Java_handoff=True,canonical_Rust_sources_are_not_bound_Java_inputs=True,context='Root must explicitly hand off the serialized Java window. Immutable selected matrix, exact auxiliary inputs, helper/JDK/JAR/lock/reference identities are bound. Canonical Rust files are not Java inputs. Stage109 GC1 correctness tests may overlap only under explicit root window; no Rust compatibility or performance claim.',performance_evidence_claimed=False)
  for name in ['home','tmp']:(OUT/name).mkdir()
  env=dict(PATH=os.defpath,HOME=str(OUT/'home'),TMPDIR=str(OUT/'tmp'),TMP=str(OUT/'tmp'),TEMP=str(OUT/'tmp'),LC_ALL='C',LANG='C',TZ='UTC',TERM='dumb',NO_COLOR='1',COLUMNS='80')
  report['environment']=env
  options=lock['jvm_options']+['-Duser.home='+env['HOME'],'-Djava.io.tmpdir='+env['TMPDIR']]
  command=[str(java),*options,'-XshowSettings:properties','-version']
  pre=subprocess.run(command,cwd=OUT,env=env,capture_output=True,timeout=30)
  (OUT/'java-preflight.stdout.bin').write_bytes(pre.stdout);(OUT/'java-preflight.stderr.bin').write_bytes(pre.stderr)
  props=dict(re.findall(r'^\s*([^\s=]+) = (.*)$',pre.stderr.decode(),re.M))
  if pre.returncode or any(props.get(k)!=v for k,v in lock['jdk']['properties'].items()):raise ValueError('actual JDK properties differ')
  report['jdk_preflight']=dict(argv=command,exit_code=pre.returncode,properties={k:props[k] for k in lock['jdk']['properties']},stdout=identity(OUT/'java-preflight.stdout.bin'),stderr=identity(OUT/'java-preflight.stderr.bin'))
  shutil.copyfile(HELPER,OUT/'CollectionEveryOracle.java')
  (OUT/'helper-true').mkdir()
  shutil.copyfile(PREP/'CollectionEveryOracle.java',OUT/'helper-true/CollectionEveryOracle.java')
  for p in programs:
   group=OUT/'programs'/p['id'];group.mkdir(parents=True)
   (group/'module.pkl').write_bytes(p['source'].encode())
   (group/'queries.tsv').write_bytes(''.join('string\t'+q['id']+'\t'+base64.b64encode(q['expression'].encode()).decode('ascii')+'\n' for q in p['queries']).encode())
   p['actual_inputs']={n:identity(group/n) for n in ['module.pkl','queries.tsv']}
  materialized=[OUT/'CollectionEveryOracle.java',OUT/'helper-true/CollectionEveryOracle.java']+[OUT/'programs'/p['id']/n for p in programs for n in ['module.pkl','queries.tsv']]
  report['materialized_before']={str(p):identity(p) for p in materialized}
  report['programs']=programs
  save(OUT/'report.json',report)
  for p in programs:
   group=OUT/'programs'/p['id']
   for repeat in range(2):
    guard();folder=group/f'repeat-{repeat}';folder.mkdir()
    selected_helper=OUT/('helper-true/CollectionEveryOracle.java' if p['power_assertions'] else 'CollectionEveryOracle.java')
    argv=[str(java),*options,'-cp',str(jar),str(selected_helper),str(folder),str(group),p['source_uri']]
    session=dict(program=p['id'],repeat=repeat,power_assertions=p['power_assertions'],helper=identity(selected_helper),argv=argv,cwd=str(OUT),environment=env,started_epoch=time.time(),cleanup_events=[])
    with (folder/'process.stdout.bin').open('wb') as stdout,(folder/'process.stderr.bin').open('wb') as stderr:
     proc=subprocess.Popen(argv,cwd=OUT,env=env,stdout=stdout,stderr=stderr,start_new_session=True)
     owned=OwnedProcesses(proc.pid)
     session.update(pid=proc.pid,pgid=proc.pid,owned_start=own_start(proc.pid));report['acquisition_started']=True
     save(folder/'process.json',session)
     try:
      while proc.poll() is None:
       owned.observe();guard()
       if time.time()-session['started_epoch']>180:raise RuntimeError('Java watchdog')
       time.sleep(.1)
     except BaseException:
      if proc.poll() is None:
       if own_start(proc.pid)!=session['owned_start'] or os.getpgid(proc.pid)!=proc.pid:raise RuntimeError('owned child identity changed')
       os.killpg(proc.pid,signal.SIGTERM);session['cleanup_events'].append('SIGTERM')
       try:proc.wait(timeout=3)
       except subprocess.TimeoutExpired:
        if own_start(proc.pid)!=session['owned_start'] or os.getpgid(proc.pid)!=proc.pid:raise RuntimeError('owned child identity changed')
        os.killpg(proc.pid,signal.SIGKILL);session['cleanup_events'].append('SIGKILL');proc.wait(timeout=5)
      raise
     finally:
      if owned.observe():
       session['cleanup_events'].append('owned-descendant-termination')
       owned.terminate()
      session['owned_processes']=owned.receipt()
      session.update(exit_code=proc.poll(),finished_epoch=time.time(),owned_child_reaped=proc.poll() is not None)
      save(folder/'process.json',session);report['sessions'].append(session);save(OUT/'report.json',report)
    guard()
    if not session['owned_processes']['root_observed'] or session['owned_processes']['terminal_live'] or session['cleanup_events']:raise RuntimeError('owned Java lifecycle incomplete or forced')
    if proc.returncode:raise RuntimeError('Java process exit '+str(proc.returncode))
    session['rows']=[readrow(folder/f'{i:03d}-{q["id"]}',q) for i,q in enumerate(p['queries'])]
    success=sum(r['status']=='success' for r in session['rows']);n=len(p['queries'])
    protocol=f'session-open mode=string rows={n}\n'+''.join(f'call-enter=string-{i}-{r["id"]}\ncall-exit=string-{i}-{r["id"]} status={r["status"]}\n' for i,r in enumerate(session['rows']))+f'session-closed mode=string rows={n} public-calls={n} success={success} errors={n-success}\n'
    if (folder/'process.stdout.bin').read_bytes()!=protocol.encode() or (folder/'process.stderr.bin').read_bytes():raise ValueError('raw process protocol')
    session['stdout']=identity(folder/'process.stdout.bin');session['stderr']=identity(folder/'process.stderr.bin')
    save(folder/'process.json',session);save(OUT/'report.json',report)
    print(json.dumps(dict(program=p['id'],repeat=repeat,statuses=dict(collections.Counter(r['status'] for r in session['rows'])))),flush=True)
  report['input_after']={p:identity(p) for p in report['input_before']};report['source_and_runtime_stable']=report['input_before']==report['input_after']
  report['materialized_after']={p:identity(p) for p in report['materialized_before']};report['materialized_stable']=report['materialized_before']==report['materialized_after']
  report['repeat_differences']=[]
  for p in programs:
   left,right=[s for s in report['sessions'] if s['program']==p['id']]
   for a,b in zip(left['rows'],right['rows']):
    if {k:v['sha256'] for k,v in a['raw_files'].items()}!={k:v['sha256'] for k,v in b['raw_files'].items()}:report['repeat_differences'].append(p['id']+'/'+a['id'])
  report['statuses']=dict(collections.Counter(r['status'] for s in report['sessions'] for r in s['rows']))
  guard()
  report['complete']=report['source_and_runtime_stable'] and report['materialized_stable'] and len(report['sessions'])==profile['programs']*2 and not report['repeat_differences']
 except BaseException as e:
  report['failure']=dict(type=type(e).__name__,message=str(e));(OUT/'failure.txt').write_text(traceback.format_exc())
 # Failure bookends and explicit per-query unrun/partial accounting do not invent observations.
 for before,after,stable in [('input_before','input_after','source_and_runtime_stable'),('materialized_before','materialized_after','materialized_stable')]:
  if before in report and after not in report:
   try:
    report[after]={p:identity(p) for p in report[before]};report[stable]=report[before]==report[after]
   except BaseException as error:report[stable]=False;report.setdefault('bookend_errors',[]).append(repr(error))
 dispositions=[]
 for p in report.get('programs',[]):
  for repeat in range(2):
   session=next((s for s in report['sessions'] if s['program']==p['id'] and s['repeat']==repeat),None)
   rows={r['id']:r for r in (session or {}).get('rows',[])}
   for i,q in enumerate(p['queries']):
    folder=OUT/'programs'/p['id']/f'repeat-{repeat}'/f'{i:03d}-{q["id"]}'
    if q['id'] in rows:
     dispositions.append(dict(program=p['id'],repeat=repeat,id=q['id'],disposition='recorded',status=rows[q['id']]['status']))
    elif folder.exists():
     partial=dict(program=p['id'],repeat=repeat,id=q['id'],disposition='acquisition-incomplete',raw_directory=str(folder.relative_to(ROOT)),available_files={f.name:identity(f) for f in folder.iterdir() if f.is_file()})
     dispositions.append(partial)
    else:dispositions.append(dict(program=p['id'],repeat=repeat,id=q['id'],disposition='unexecuted',reason='No complete query observation was produced before acquisition stopped.'))
 report['query_dispositions']=dispositions
 report['query_disposition_counts']=dict(collections.Counter(r['disposition'] for r in dispositions))
 report['process_timeout_or_resource_failure_is_not_ordinary_Pkl_error']=True
 try:guard()
 except BaseException as error:
  report['complete']=False;report.setdefault('terminal_resource_errors',[]).append(repr(error))
 report.update(finished_epoch=time.time(),minimum_free_seen_bytes=minimum,peak_owned_bytes=peak,terminal_usage=usage())
 save(OUT/'report.json',report)
 save(OUT/'capture-process.json',dict(pid=os.getpid(),argv=sys.argv,cwd=os.getcwd(),exit_code=0 if report['complete'] else 1,report=identity(OUT/'report.json')))
 print(json.dumps({k:report.get(k) for k in ['complete','statuses','failure','repeat_differences']}),flush=True)
 return 0 if report['complete'] else 1
if __name__=='__main__':raise SystemExit(main())
