from pathlib import Path
import hashlib,json,sys,platform
R=Path.cwd();O=Path(__file__).resolve().parent;sys.path.insert(0,str(R/'tools'))
import compat
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
def write(p,v):p.parent.mkdir(parents=True,exist_ok=True);p.write_text(json.dumps(v,indent=2,ensure_ascii=True)+'\n')
write(O/'exclusions.json',{'enumeration':'This small oracle root only; no Cargo caches or repository target traversal.','exactPrunedDirectories':[]})
controls=[]
payload=r'\u{d83d}|\u{dc00}|\u{1f600}|?|\u{fffd}|\u{0}'
controls.append(dict(id='invalid-converter',mode='converter',cause='units',source='payload = "'+payload+'."\nresult = (new PcfRenderer { converters { [payload] = (x) -> x } }).renderValue(new Dynamic { x = 1 })\n',expressions=['result','result']))
controls.append(dict(id='unknown-output-format',mode='unknown-format',cause='units',source='result = 1\n',expressions=['output.text','output.text']))
for mode in ['module-resolve','module-source','resource']:
 for cause in ['units','null','empty']:
  source='import "lossless:/child" as child\nvalue = child.value\nstable = 7\n' if mode.startswith('module-') else 'value = read("lossless:/resource")\nstable = 7\n'
  controls.append(dict(id=mode+'-'+cause,mode=mode,cause=cause,source=source,expressions=['stable','value','value','stable']))
for c in controls:
 work=O/'workspaces'/c['id'];work.mkdir(parents=True);(work/'main.pkl').write_text(c['source']);(work/'expressions.txt').write_text('\n'.join(c['expressions'])+'\n');c['workspace']=str(work);c['moduleUri']=(work/'main.pkl').as_uri();c['sourceSha256']=sha(work/'main.pkl')
write(O/'cases.json',controls)
jar=R/'.cache/oracle/pkl-tools-0.32.0.jar';lock=json.loads((R/'oracle.lock.json').read_text());assert sha(jar)==lock['artifact']['sha256']
home=O/'identity/home';tmp=O/'identity/tmp';home.mkdir(parents=True);tmp.mkdir(parents=True)
env={'PATH':'/bin:/usr/bin','HOME':str(home),'TMPDIR':str(tmp),'LANG':'C.UTF-8','LC_ALL':'C.UTF-8','TERM':'dumb','NO_COLOR':'1','TZ':'UTC'}
def capture(name,command,cwd):
 try:result=compat.capture(command,cwd,env,30,O/'generated')
 except compat.CaptureTimeout as error:result=error.result
 metadata=compat.store_capture(O/name,result);record=dict(command=command,cwd=str(cwd),environment=env,result=metadata);write(O/(name+'.process.json'),record);return result,record
result,_=capture('javac',['/usr/bin/javac','-cp',str(jar),'-d',str(O/'classes'),str(O/'LosslessHostProbe.java')],R);assert result['exit_code']==0
capture('jdk',['/usr/bin/java','-XshowSettings:properties','-version'],R)
paths=[jar,O/'LosslessHostProbe.java',Path(__file__),O/'cases.json',R/'oracle.lock.json',R/'references.lock',R/'tools/compat.py',*sorted((O/'classes').glob('*.class'))]
for c in controls:paths.extend(sorted(Path(c['workspace']).iterdir()))
before={str(p):sha(p) for p in paths};write(O/'inputs-before.json',before)
base=['/usr/bin/java','-Xms64m','-Xmx512m','-Dfile.encoding=UTF-8','-Dstdout.encoding=UTF-8','-Dstderr.encoding=UTF-8','-Duser.language=en','-Duser.country=US','-Duser.timezone=UTC','-Djava.awt.headless=true','-Duser.name=oracle','-Duser.home='+str(home),'-Djava.io.tmpdir='+str(tmp),'-cp',str(O/'classes')+':'+str(jar),'LosslessHostProbe']
records=[];observations=[];diffs=[]
for c in controls:
 raws=[];work=Path(c['workspace']);snapshot=compat.snapshot(work)
 for repeat in [0,1]:
  name='cases/'+c['id']+'/'+str(repeat);calls=O/name/'calls';result,record=capture(name,base+[str(work),str(calls),c['mode'],c['cause']],work);record.update(case=c['id'],repeat=repeat,workspaceBefore=snapshot,workspaceAfter=compat.snapshot(work));write(O/name/'process.json',record);records.append(record)
  assert result['exit_code']==0 and record['workspaceAfter']==snapshot
  raws.append({p.name:p.read_bytes() for p in calls.iterdir() if p.is_file()})
  rows=[]
  for index,expression in enumerate(c['expressions']):
   prefix='call-'+str(index);status=(calls/(prefix+'.status.bin')).read_bytes();klass=(calls/(prefix+'.error-class.bin')).read_text();rows.append(dict(index=index,expression=expression,status=status.decode(),errorClass=klass,oracleError=bool(klass and klass!='org.pkl.core.PklException'),rawPrefix=str((calls/prefix).relative_to(O))))
  observations.append(dict(case=c['id'],repeat=repeat,calls=rows,counters=(calls/'counters.bin').read_text()))
  print(json.dumps({'case':c['id'],'repeat':repeat,'kinds':[r['status'] for r in rows],'counters':observations[-1]['counters']}),flush=True)
 diffs.extend(c['id']+'/'+name for name in raws[0].keys()|raws[1].keys() if raws[0].get(name)!=raws[1].get(name))
after={p:sha(Path(p)) for p in before};write(O/'inputs-after.json',after)
report=dict(scope='Actual Java UTF-16/UTF-8 producer and base IOException observations; not Rust compatibility.',caseCount=len(controls),freshJvmCount=len(records),callCount=sum(len(c['expressions'])*2 for c in controls),powerAssertions=True,host=platform.platform(),records=records,observations=observations,inputChanges=[p for p in before if before[p]!=after[p]],repeatDifferences=diffs,jarSha256=sha(jar),jdkReleaseSha256=sha(Path('/Library/Java/JavaVirtualMachines/temurin-21.jdk/Contents/Home/release')),jdkModulesSha256=sha(Path('/Library/Java/JavaVirtualMachines/temurin-21.jdk/Contents/Home/lib/modules')))
write(O/'report.json',report);assert not report['inputChanges'] and not diffs
print('report '+sha(O/'report.json'),flush=True)
