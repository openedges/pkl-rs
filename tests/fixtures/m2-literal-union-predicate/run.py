from pathlib import Path
import json,hashlib,sys
R=Path.cwd();O=Path(__file__).resolve().parent;sys.path.insert(0,str(R/'tools'));import compat
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
def put(p,obj):p.parent.mkdir(parents=True,exist_ok=True);p.write_text(json.dumps(obj,indent=2,ensure_ascii=True)+'\n')
jar=Path('/Users/cody.ij.hwang/Projects/pkl-rs/.cache/oracle/pkl-tools-0.32.0.jar');lock=json.loads((R/'oracle.lock.json').read_bytes());assert sha(jar)==lock['artifact']['sha256']
home=O/'home';tmp=O/'tmp';home.mkdir();tmp.mkdir();env=dict(PATH='/bin:/usr/bin',HOME=str(home),TMPDIR=str(tmp),LANG='C.UTF-8',LC_ALL='C.UTF-8',TZ='UTC',NO_COLOR='1',TERM='dumb')
def cap(name,cmd):
 try:r=compat.capture(cmd,R,env,30,O/'generated')
 except compat.CaptureTimeout as e:r=e.result
 rec=dict(command=cmd,cwd=str(R),env=env,result=compat.store_capture(O/name,r));put(O/(name+'.process.json'),rec);return r,rec
r,_=cap('javac',['/usr/bin/javac','-cp',str(jar),'-d',str(O/'classes'),str(O/'UnionPredicateProbe.java')]);assert r['exit_code']==0
r,jdk=cap('jdk',['/usr/bin/java','-XshowSettings:properties','-version']);props={}
for line in r['stderr'].decode().splitlines():
 if ' = ' in line.strip():k,v=line.strip().split(' = ',1);props[k]=v
assert all(props[k]==v for k,v in lock['jdk']['properties'].items())
files=[Path(__file__),O/'UnionPredicateProbe.java',jar,R/'oracle.lock.json',R/'references.lock',R/'tools/compat.py',*sorted((O/'classes').rglob('*.class'))];before={str(p):sha(p) for p in files};put(O/'inputs.before.json',before)
base=['/usr/bin/java','-Xms64m','-Xmx512m','-Dfile.encoding=UTF-8','-Dstdout.encoding=UTF-8','-Dstderr.encoding=UTF-8','-Duser.language=en','-Duser.country=US','-Duser.timezone=UTC','-Djava.awt.headless=true','-Duser.name=oracle','-Duser.home='+str(home),'-Djava.io.tmpdir='+str(tmp),'-cp',str(O/'classes')+':'+str(jar),'org.pkl.core.ast.type.UnionPredicateProbe'];records=[];raw=[]
for repeat in (0,1):
 out=O/'repeats'/str(repeat)/'calls';r,rec=cap('repeats/'+str(repeat),base+[str(out)]);records.append(rec);assert r['exit_code']==0;raw.append({p.name:p.read_bytes() for p in out.glob('*.bin')});print(r['stdout'].decode(),flush=True)
after={str(p):sha(p) for p in files};put(O/'inputs.after.json',after);assert before==after;diff=[p for p in raw[0].keys()|raw[1].keys() if raw[0].get(p)!=raw[1].get(p)];assert not diff
answers={p.removesuffix('.stdout.bin'):b.decode() for p,b in raw[0].items() if p.endswith('.stdout.bin')}
put(O/'report.json',dict(boundary='Internal pinned Java TypeNode.isEquivalentTo predicate, NOT language/API evaluation or Rust compatibility',pairs=len(answers),freshJVMs=2,calls=len(answers)*2,answers=answers,records=records,jdkProperties=props,jarSha256=sha(jar),jdkReleaseSha256=sha(Path(props['java.home'])/'release'),jdkModulesSha256=sha(Path(props['java.home'])/'lib/modules'),repeatDifferences=diff,inputsUnchanged=True));print('report',sha(O/'report.json'))
