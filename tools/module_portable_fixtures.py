"""Export/verify Java-only portable ModuleKey provider facts and observable raw traces."""
from pathlib import Path
import argparse,hashlib,json,os,re,stat,struct,sys,zipfile
PINS={'groups': {'triple-dot': {'root': '/Users/cody.ij.hwang/Projects/pkl-rs.stage46/target/m2/stage46-triple-dot-oracle-01', 'reportSha256': '0caca24eb7ae82eb75e3fab71dc43d3d39827fd86766ecf5cefb41602406e52d', 'inventorySha256': None, 'closureSha256': None}, 'logical': {'root': '/Users/cody.ij.hwang/Projects/pkl-rs.stage46/target/m2/stage46-logical-binding-oracle-01', 'reportSha256': 'db1b7305da5f564a3b670b8e3deee304e7188ee579138b7cf5d9187a4ce64765', 'inventorySha256': '281fa7f68de56960018e08b16cae1b57cf2f1977193945434afe0af36866c75b', 'closureSha256': 'b2f54f1e12c94b405d7a5ba1f6e2f06211b26e067dc27be0773e2d6dcb65c528'}, 'capability': {'root': '/Users/cody.ij.hwang/Projects/pkl-rs.stage46/target/m2/stage46-module-capability-oracle-03', 'reportSha256': 'eda49c0ed06b2320b33dd877391b64adb62a0611deac09d0a828da1d0e012740', 'inventorySha256': '0ea0c4fa48550771ca8ad0c313baca78b43a2f59a5999c50f8dbcc32a2010caf', 'closureSha256': 'a1b71530f9b68603d7fd47faceea08e0f6ac5380123d50434a2f22b1b538e1b1'}}, 'extras': {'javaErrorProperties': {'sha256': 'fe93e838a3278091b0be0d02cb4519f0720ad66319e76f2b1741d65ab217292b', 'jar': '/Users/cody.ij.hwang/Projects/pkl-rs.stage42/.cache/oracle/pkl-tools-0.32.0.jar', 'jarSha256': 'c3b6f6215a08b31453b0fb0edd8c9da0aee31233f65abb8ab07f9c945151e059', 'entry': 'org/pkl/core/errorMessages.properties'}, 'javaFileResolver': {'path': '/Users/cody.ij.hwang/Projects/pkl-rs.stage46/external/pkl/pkl-core/src/main/java/org/pkl/core/module/FileResolver.java', 'sha256': 'ce44618947d8af815f9aaf72f6993603c88289bcd70cb8ab90082013ed5172a1'}}}
SELECTED={'triple-dot':{'file-tree-allow','file-tree-deny-module-hasElement','file-tree-deny-resource-read'},'logical':{'coalesce-lambda-owner'},'capability':{'triple-dot-nonlocal','triple-dot-nonhierarchical'}}
SUFFIXES=['status.bin','stdout.bin','stderr.bin','stdout.code-units.bin','stderr.code-units.bin','reads-before.bin','reads-after.bin','expression.bin','error-class.bin','counters-before.bin','counters-after.bin']
KEYS={'capabilities':['kind','originalUri','importerOriginalUri','importerResolvedUri'],'has-element':['kind','originalUri','importerOriginalUri','importerResolvedUri'],'resolve':['kind','originalUri','importUri','importerOriginalUri','importerResolvedUri'],'load-source':['kind','originalUri','resolvedUri','sourceUri']}
def digest(data):return hashlib.sha256(data).hexdigest()
def sha(path):return digest(path.read_bytes())
def put(path,value):path.parent.mkdir(parents=True,exist_ok=True);path.write_text(json.dumps(value,indent=2)+'\n')
def units(raw):
 if len(raw)%2:raise ValueError('odd Java UTF16')
 return list(struct.unpack('>'+'H'*(len(raw)//2),raw))
def counter(raw):return dict(x.split('=',1)for x in raw.splitlines())
def projection(events,primary,group):
 values=[];outside=[]
 for event in events:
  if event in {'policy-resolve-module '+primary,'policy-resolve-module repl:text','secure-path '+primary+' resource=false'}:outside.append({'event':event,'reason':'Java primary/query source admission differs from supplied SourceProbe source'});continue
  if group=='capability'and event in {'resolve '+primary,'resolve repl:text'}:outside.append({'event':event,'reason':'Custom Java ModuleKey primary/query admission; supplied SourceProbe source has no equivalent resolve callback'});continue
  if re.fullmatch(r'outer-\d+ (success|error org\.pkl\.core\.PklException)',event):outside.append({'event':event,'reason':'API status/channels compared independently'});continue
  if event.startswith(('policy-resolve-module ','policy-import ','policy-read-resource ','actual-file-reader ','actual-module-denied ','actual-resource-denied ','secure-path ')):values.append(event)
  else:raise ValueError('unmapped original Java event: '+event)
 return values,outside

def derive(read):
 # read(original, expectedSha) is bound either to exact original files or captured immutable copies.
 props=read('$javaErrorProperties',PINS['extras']['javaErrorProperties']['sha256']).decode();read('$javaFileResolver',PINS['extras']['javaFileResolver']['sha256'])
 def template(key):
  marker=key+'=\\\n';start=props.index(marker)+len(marker);return props[start:props.index('\n',start)]
 templates={n:template(n)for n in ['moduleNotInAllowList','resourceNotInAllowList']};fixtures=[];unselected=[];all_api=0
 for group,pin in PINS['groups'].items():
  oracle=Path(pin['root']);report=json.loads(read(str(oracle/'report.json'),pin['reportSha256']));all_api+=report['apiCalls'];cases=json.loads(read(str(oracle/'cases.json'),report['inputHashes'][str(oracle/'cases.json')]));profile=json.loads(read(str(oracle/'profile.json'),report['inputHashes'][str(oracle/'profile.json')]))
  if profile['jarSha256']!=PINS['extras']['javaErrorProperties']['jarSha256']:raise ValueError('Java artifact identity differs')
  helper='CapabilityProbe.java'if group=='capability'else'TripleProbe.java';read(str(oracle/helper),report['inputHashes'][str(oracle/helper)])
  for n,key in [('inventory.json','inventorySha256'),('closure.json','closureSha256')]:
   if pin[key]is not None:read(str(oracle/n),pin[key])
  for original,h in report['inputHashes'].items():
   if original.startswith(str(oracle/'upstream')+'/')or original.endswith('/tool-sources/references.lock'):read(original,h)
  for c in cases:
   observations=[o for o in report['observations']if o['case']==c['id']]
   if c['id']not in SELECTED[group]:
    for ob in observations:
     for call in ob['calls']:unselected.append({'group':group,'case':c['id'],'repeat':ob['repeat'],'session':call['session'],'prefix':call['prefix'],'source':call['source'],'expression':call['expression'],'rawOriginalPrefix':str(oracle/call['rawPrefix']),'reason':'Not selected in initial typed ModuleKey portable fixture; full original remains a separate obligation'})
    continue
   work=Path(c['workspace']);physical_source=work/c['operations'][0][0];graph={};source_snapshot=next(r['workspaceBefore']for r in report['records']if r['case']==c['id']);input_files={}
   for original,h in report['inputHashes'].items():
    path=Path(original)
    if path.is_relative_to(work):
     payload=read(original,h);uri=path.as_uri();kind=source_snapshot.get(path.relative_to(work).as_posix(),{}).get('kind','file');item={'uri':uri,'originalPath':original,'sha256':h,'bytes':list(payload),'kind':kind}
     if kind=='symlink':item['literalSymlinkTarget']=read('$symlink:'+original,source_snapshot[path.relative_to(work).as_posix()]['sha256']).decode()
     graph[uri]=item;input_files[original]=payload
   source=input_files[str(physical_source)].decode();primary=c['sourceUri'];denied=(work/'tree/a/resource.txt').as_uri()if group=='triple-dot'else None
   module_message=templates['moduleNotInAllowList'].replace('{0}',denied)if denied else None;resource_message=templates['resourceNotInAllowList'].replace('{0}',denied)if denied else None
   owners=[{'originalUri':primary,'resolvedUri':primary}]
   for item in graph.values():
    path=Path(item['originalPath'])
    if path.suffix=='.pkl'and path!=physical_source:
     if item['kind']!='file':raise ValueError('Source symlink identity needs a separately captured provider fact')
     owner={'originalUri':item['uri'],'resolvedUri':item['uri']}
     if owner not in owners:owners.append(owner)
   for ob in observations:
    process_events=bytearray()
    for session in [0,1]:
     calls=[x for x in ob['calls']if x['session']==session];expected=[];all_events=[]
     for call in calls:
      prefix=str(oracle/call['rawPrefix']);raw={n:read(prefix+'.'+n,None)for n in SUFFIXES};status=raw['status.bin']
      if status not in {b'success\n',b'error\n'}or status!=call['status'].encode():raise ValueError('exact Java status differs')
      target='stdout'if status==b'success\n'else'stderr';other='stderr'if target=='stdout'else'stdout'
      if raw[target+'.bin'].hex()!=call['utf8Hex']or raw[target+'.code-units.bin'].hex()!=call['utf16BEHex']or raw[other+'.bin']or raw[other+'.code-units.bin']:raise ValueError('full Java output channels differ')
      if raw['expression.bin']!=call['expression'].encode()or raw['error-class.bin']!=call['errorClass'].encode()or call['errorClass']not in {'','org.pkl.core.PklException'}:raise ValueError('Java expression/error class differs')
      before=counter(call['countersBefore']);after=counter(call['countersAfter'])
      for label,suffix,count_suffix in [('countersBefore','counters-before.bin','reads-before.bin'),('countersAfter','counters-after.bin','reads-after.bin')]:
       if raw[suffix]!=call[label].encode()or raw[count_suffix]!=counter(call[label])['read'].encode():raise ValueError('raw/pinned Java counters differ')
      directory=Path(prefix).parent;events=[read(str(directory/f'event-{i}.bin'),None).decode()for i in range(int(before['event']),int(after['event']))];all_events.extend(events);process_events.extend(b''.join(e.encode()+b'\n'for e in events));observed,outside=projection(events,primary,group)
      expected.append({'status':status[:-1].decode(),'stdoutUnits':units(raw['stdout.code-units.bin']),'stderrUnits':units(raw['stderr.code-units.bin']),'stdoutHex':raw['stdout.bin'].hex(),'stderrHex':raw['stderr.bin'].hex(),'readsBefore':int(raw['reads-before.bin']),'readsAfter':int(raw['reads-after.bin']),'javaEvents':events,'observableEvents':observed,'outsideContract':outside})
     candidate_uris=list(dict.fromkeys(e[len('policy-resolve-module '):]for e in all_events if e.startswith('policy-resolve-module file:')));providers=[];keys=set()
     def add(request,result,events):
      if set(request)!=set(KEYS[request['kind']]):raise ValueError('incomplete provider request key')
      key=json.dumps(request,sort_keys=True)
      if key in keys:raise ValueError('duplicate module provider key')
      keys.add(key);providers.append({'request':request,'result':result,'observableEvents':events})
     for owner in owners:
      context={'importerOriginalUri':owner['originalUri'],'importerResolvedUri':owner['resolvedUri']}
      add({'kind':'capabilities','originalUri':owner['originalUri'],**context},{'kind':'capabilities','local':c['mode']!='cap-nonlocal','hierarchical':c['mode']!='cap-nonhierarchical','fragmentPaths':False},[])
      for uri in candidate_uris:
       events=['policy-resolve-module '+uri]
       if c['mode']=='deny-module'and uri==denied:result={'kind':'failure','errorKind':'security-manager-exception','message':module_message};events.append('actual-module-denied '+uri)
       else:result={'kind':'has-element','exists':uri in graph}
       add({'kind':'has-element','originalUri':uri,**context},result,events)
     imports=list(dict.fromkeys(e[len('policy-import '):]for e in all_events if e.startswith('policy-import ')));loaded=set()
     for imported in imports:
      importer,uri=imported.split(' -> ',1);owner=next((o for o in owners if o['originalUri']==importer),None)
      if owner is None or uri not in graph:raise ValueError('module provider lacks captured File/source identity')
      add({'kind':'resolve','originalUri':uri,'importUri':uri,'importerOriginalUri':importer,'importerResolvedUri':owner['resolvedUri']},{'kind':'resolve','resolvedUri':uri,'sourceUri':uri},['policy-import '+imported,'policy-resolve-module '+uri,'secure-path '+uri+' resource=false'])
      if uri not in loaded:
       loaded.add(uri);entry=graph[uri]
       if entry['kind']!='file':raise ValueError('Aliased module source resolution not selected without extra identity evidence')
       add({'kind':'load-source','originalUri':uri,'resolvedUri':uri,'sourceUri':uri},{'kind':'source','text':bytes(entry['bytes']).decode(),'inferredName':Path(entry['originalPath']).stem},['policy-resolve-module '+uri])
     read_uris=list(dict.fromkeys(e[len('actual-file-reader '):]for e in all_events if e.startswith('actual-file-reader ')))
     read_uris.extend(uri for uri in (e[len('actual-resource-denied '):]for e in all_events if e.startswith('actual-resource-denied '))if uri not in read_uris);reads=[]
     for uri in read_uris:
      if c['mode']=='deny-resource'and uri==denied:reads.append({'uri':uri,'result':{'kind':'security-manager-exception','message':resource_message},'observableEvents':['policy-read-resource '+uri,'actual-resource-denied '+uri],'countedReader':False})
      else:
       if uri not in graph:raise ValueError('Resource payload not present in captured File graph')
       reads.append({'uri':uri,'result':{'kind':'resource','resourceUri':uri,'bytes':graph[uri]['bytes']},'observableEvents':['policy-read-resource '+uri,'actual-file-reader '+uri,'secure-path '+uri+' resource=true'],'countedReader':True})
     readmap={x['uri']:x for x in reads};reader_plan=[{'uri':e[len('actual-file-reader '):],'result':readmap[e[len('actual-file-reader '):]]['result']}for e in all_events if e.startswith('actual-file-reader ')]
     if len(reader_plan)!=int(counter(calls[-1]['countersAfter'])['read']):raise ValueError('actual Java reader plan count differs')
     for item in expected:
      if 'actual-module-denied '+str(denied)in item['javaEvents']and not bytes.fromhex(item['stderrHex']).startswith(('–– Pkl Error ––\n'+module_message+'\n\n').encode()):raise ValueError('Java module policy cause differs')
      if 'actual-resource-denied '+str(denied)in item['javaEvents']and not bytes.fromhex(item['stderrHex']).startswith(('–– Pkl Error ––\n'+resource_message+'\n\n').encode()):raise ValueError('Java resource policy cause differs')
     fixtures.append({'id':group+'/'+c['id'],'repeat':ob['repeat'],'session':session,'source':source,'uri':primary,'moduleName':'main','expressions':[x['expression']for x in calls],'expected':expected,'readerPlan':reader_plan,'moduleProviders':providers,'readProviders':reads,'allowedModuleIdentities':owners,'filePolicy':{'mode':c['mode'],'deniedUri':denied,'moduleMessage':module_message,'resourceMessage':resource_message},'constructorBoundaryExpected':False,'constructorJavaApiCallsUnexecuted':[],'providerProof':{'mode':'Java-source/input-derived reusable lookup facts; not an observed SPI request sequence','missingExistence':'Captured File input graph + actual Java search continuation/missing diagnostics; Files.exists itself was not instrumented','symlinks':[{'uri':v['uri'],'literalTarget':v['literalSymlinkTarget']}for v in graph.values()if v['kind']=='symlink'],'denialCause':'Pinned Java JAR errorMessages template and actual deniedURI; original helper did not separately record bare SecurityManagerException cause; full Java errors remain exact','noNativeDerivedExpectations':True}})
    process=next(r for r in report['records']if r['case']==c['id']and r['repeat']==ob['repeat'])
    if digest(process_events)!=process['result']['sha256']['stdout']:raise ValueError('all original Java event order differs from pinned process stdout')
 data={'schema':1,'fixtures':fixtures,'javaApiCalls':all_api,'selectedJavaApiCalls':sum(len(f['expected'])for f in fixtures),'unexecutedApiBoundaries':unselected,'requestKeyFields':KEYS,'qualification':'Initial typed ModuleKey transport selection only. Provider facts are reusable unordered Java-source/input rules; no expected request IDs/count/order or provider-full-consumption requirement. Actual Java observable traces are independently pinned and exact.','remainingWholeScope':['Other File/order432 Java APIs (including16 constructor-boundary calls) remain outside this initial portable selection','Other logical48 calls preserved in unexecutedApiBoundaries','Whole original read, full resource/API/lifecycle/GC/platform/M2 obligations remain separate'],'historicalOracleSeals':{g:{'inventorySha256':v['inventorySha256'],'closureSha256':v['closureSha256']}for g,v in PINS['groups'].items()}}
 if all_api!=300 or data['selectedJavaApiCalls']!=252 or len(fixtures)!=24 or len(unselected)!=48:raise ValueError('initial Java selection coverage differs')
 return data

def safe(root,name):
 path=Path(name)
 if str(path)!=name or path.is_absolute()or'..'in path.parts:raise ValueError('unsafe fixture path')
 result=root/path
 if result.is_symlink()or not result.resolve().is_relative_to(root.resolve()):raise ValueError('unsafe fixture link')
 return result

def prepare(output):
 output.mkdir(parents=True,exist_ok=False);bindings={}
 def read(original,expected):
  if original=='$javaErrorProperties':
   pin=PINS['extras']['javaErrorProperties'];jar=Path(pin['jar'])
   if sha(jar)!=pin['jarSha256']:raise ValueError('pinned Java artifact changed')
   with zipfile.ZipFile(jar)as z:payload=z.read(pin['entry'])
  elif original=='$javaFileResolver':payload=Path(PINS['extras']['javaFileResolver']['path']).read_bytes()
  elif original.startswith('$symlink:'):payload=os.readlink(original[len('$symlink:'):]).encode()
  else:payload=Path(original).read_bytes()
  h=digest(payload)
  if expected is not None and h!=expected:raise ValueError('pinned original input differs: '+original)
  local='captured/'+h+'.bin';p=output/local;p.parent.mkdir(exist_ok=True)
  if not p.exists():p.write_bytes(payload)
  bindings[original]={'path':local,'sha256':h,'bytes':len(payload),'independentlyPinnedSha256':expected};return payload
 data=derive(read);put(output/'fixtures.json',data);put(output/'provenance.json',{'pins':PINS,'originalPaths':bindings,'source':'Existing Java raw, input snapshots and pinned Java source rules only; no native result used'});put(output/'manifest.json',{'schema':1,'files':{p.relative_to(output).as_posix():{'sha256':sha(p),'bytes':p.stat().st_size}for p in sorted(output.rglob('*'))if p.is_file()and p.name!='manifest.json'}});return verify(output)

def verify(root):
 provenance=json.loads((root/'provenance.json').read_bytes());manifest=json.loads((root/'manifest.json').read_bytes());bindings=provenance['originalPaths'];used=set()
 if provenance['pins']!=PINS:raise ValueError('independent original Java pins differ')
 def read(original,expected):
  record=bindings.get(original)
  if record is None:raise ValueError('required raw/source provenance missing: '+original)
  if record['path']!='captured/'+record['sha256']+'.bin'or record['independentlyPinnedSha256']!=expected:raise ValueError('original/source pin mapping differs')
  p=safe(root,record['path']);payload=p.read_bytes()
  if digest(payload)!=record['sha256']or len(payload)!=record['bytes']or(expected is not None and digest(payload)!=expected):raise ValueError('pinned raw/source bytes differ')
  used.add(original);return payload
 expected=derive(read);data=json.loads((root/'fixtures.json').read_bytes())
 if data!=expected:raise ValueError('portable fixture/provider/raw expectation differs from pinned Java derivation')
 if used!=set(bindings):raise ValueError('unused/added original path provenance')
 files={'fixtures.json','provenance.json'}|{v['path']for v in bindings.values()}
 if manifest['schema']!=1 or set(manifest['files'])!=files:raise ValueError('manifest semantic coverage differs')
 # Authenticate all source/raw/provider paths before walking; only one captured blob directory is admitted.
 actual=set();pending=[root]
 while pending:
  directory=pending.pop()
  with os.scandir(directory)as entries:
   for entry in entries:
    path=Path(entry.path);name=path.relative_to(root).as_posix();mode=entry.stat(follow_symlinks=False).st_mode
    if stat.S_ISDIR(mode):
     if name!='captured':raise ValueError('unexpected fixture directory')
     pending.append(path)
    elif stat.S_ISREG(mode):actual.add(name)
    else:raise ValueError('nonregular fixture entry')
 if actual!=files|{'manifest.json'}:raise ValueError('actual file coverage differs')
 for name,record in manifest['files'].items():
  p=safe(root,name)
  if sha(p)!=record['sha256']or p.stat().st_size!=record['bytes']:raise ValueError('fixture file digest differs')
 return {'sessions':24,'selectedJavaApiCalls':252,'allJavaApiCalls':300,'unselectedJavaApiCalls':48,'moduleProviders':sum(len(f['moduleProviders'])for f in data['fixtures']),'actualJavaObservableEvents':sum(len(e['observableEvents'])for f in data['fixtures']for e in f['expected']),'manifestSha256':sha(root/'manifest.json')}
def main():
 parser=argparse.ArgumentParser(description=__doc__);group=parser.add_mutually_exclusive_group(required=True);group.add_argument('--fixtures',type=Path);group.add_argument('--output',type=Path);args=parser.parse_args()
 try:result=verify(args.fixtures)if args.fixtures else prepare(args.output);print(json.dumps(result,indent=2));return 0
 except(OSError,ValueError,KeyError,StopIteration)as error:print('Module portable fixture verification failed: '+str(error),file=sys.stderr);return 1
if __name__=='__main__':sys.exit(main())
