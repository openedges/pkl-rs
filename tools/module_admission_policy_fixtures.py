"""Exact Java06 policy84 fixture export; no evaluator or synthesized policy events."""
from pathlib import Path
import argparse,collections,hashlib,json,os,stat
ORIGINAL=Path('/Users/cody.ij.hwang/Projects/pkl-rs.stage47/target/m2/stage47-renderer-api-oracle-06')
PINS=('cba85cf833baeb7655f6a6cdc81fd9ee39fe0ee43f441e43d891a47c0089ebf8','b9f1243b213906d010998f87807388bee1dc2607ffb447f937a6fb9a3eb4cbc5','8a10197b44469e40af433399d3caa2833a75be9fac957a262047ff33979ed2db')
CASES=['stdlib-admission-allow','stdlib-admission-deny-json','stdlib-admission-deny-singletons']
PREFIX='provenance/stage47-renderer-api-oracle-06'
sha=lambda data:hashlib.sha256(data).hexdigest()
def check(condition, message):
 if not condition:
  raise ValueError(message)

def fixture_reader(root):
 check(stat.S_ISDIR(root.lstat().st_mode), 'fixture root must be a regular directory')
 def read(relative):
  parts = safe_relative(relative).parts
  path = root
  for part in parts[:-1]:
   path = path / part
   check(stat.S_ISDIR(path.lstat().st_mode), 'fixture parent must be a regular directory')
  path = path / parts[-1]
  check(stat.S_ISREG(path.lstat().st_mode), 'fixture payload must be a regular file')
  return path.read_bytes()
 return read

def safe_relative(text):
 p = Path(text)
 check(not p.is_absolute() and '..' not in p.parts and str(p) == text, 'unsafe relative path')
 return p

def file_set(root, expected_files=None):
 allowed_directories = None
 if expected_files is not None:
  allowed_directories = {str(parent) for relative in expected_files for parent in safe_relative(relative).parents}
 check(root.is_dir() and not root.is_symlink(), 'fixture root must be a regular directory')
 found = set()
 for current, directories, files in os.walk(root, followlinks=False):
  for name in directories + files:
   check(not (Path(current) / name).is_symlink(), 'symlink forbidden in fixture')
  if allowed_directories is not None:
   for name in directories:
    relative = str((Path(current) / name).relative_to(root))
    check(relative in allowed_directories, 'undeclared fixture directory')
  for name in files:
   path = Path(current) / name
   check(path.is_file(), 'non-regular fixture payload')
   found.add(str(path.relative_to(root)))
 return found

def counters(data):
 text=data.decode('ascii');pairs=[line.split('=',1)for line in text.splitlines()];check(len(pairs)==8 and all(len(pair)==2 for pair in pairs),'eightcounter syntax');result={k:int(v)for k,v in pairs};check(list(result)==['resolve','import','read','policyRead','resolveResource','securePath','denied','event'],'exact counter field/order');return result

def utf16(data):
 check(len(data)%2==0,'evenUTF16');return [int.from_bytes(data[i:i+2],'big')for i in range(0,len(data),2)]
def java_utf8(data):return data.decode('utf-16-be','surrogatepass').encode('utf-8','replace')
def construct(read):
 docs=[read(PREFIX+'/'+name+'.json')for name in ['report','inventory','closure']];check(tuple(map(sha,docs))==PINS,'literal provenance pins');report,inventory,closure=map(json.loads,docs)
 check(closure['closed']and closure['allPayloadHashesVerified']and closure['liveHandles']==[]and closure['inventorySha256']==PINS[1]and closure['reportSha256']==PINS[0],'closedproducer binding')
 rows={r['path']:r for r in inventory['files']};check(len(rows)==closure['files'],'inventorypath/count');check(report['inputChanges']==[]and report['workspacesUnchanged']and report['jarStable'],'producer stable');check(all(not x['rawPathDifferences']for x in report['repeatDifferences']),'repeatrawmatch')
 required={PREFIX+'/'+name+'.json'for name in ['report','inventory','closure']};proofs={}
 def payload(rel):
  check(rel in rows,'not in pinned inventory: '+rel);data=read(PREFIX+'/'+rel);row=rows[rel];check(sha(data)==row['sha256']and len(data)==row['bytes'],'pinnedinventory payload: '+rel);required.add(PREFIX+'/'+rel);proofs[PREFIX+'/'+rel]={'originalRoot':str(ORIGINAL),'originalRelativePath':rel,'sha256':sha(data),'bytes':len(data)};return data
 for rel in ['RendererProbe.java','run.py','profile.json','cases.json','inputs-before.json','inputs-after.json']:payload(rel)
 for rel in ['RendererProbe.java','run.py','profile.json']:check(report['inputHashes'][str(ORIGINAL/rel)]==sha(payload(rel)),'source/profile report binding')
 authored=json.loads(payload('cases.json'));case_map={c['id']:c for c in authored};fixtures=[];event_counts=collections.Counter();api_counts=collections.Counter()
 for name in CASES:
  case=case_map[name];wp='workspace/'+name
  for rel in rows:
   if rel.startswith(wp+'/'):payload(rel)
  source=payload(wp+'/main.pkl');check(report['inputHashes'][str(ORIGINAL/wp/'main.pkl')]==sha(source),'source report binding')
  check(payload(wp+'/reader.tsv')==b'','no authored reader input inpolicy84')
  expressions=[op[1]for op in case['operations']if op!='GC'];expected_ops=''.join('GC\n'if op=='GC'else'\t'.join(op)+'\n'for op in case['operations']).encode();check(payload(wp+'/operations.tsv')==expected_ops,'authored operation order')
  for observation in report['observations']:
   if observation['case']!=name:continue
   repeat=observation['repeat'];proc=next(r for r in report['records']if r['case']==name and r['repeat']==repeat);check(proc['result']['exit_code']==0 and not proc['result']['timed_out']and proc['result']['workspace_changes']=={},'actual Java process boundary')
   run='cases/'+name+'/'+str(repeat);run_files={rel:payload(rel)for rel in rows if rel.startswith(run+'/')and'/calls/'not in rel};all_console=[]
   for session in [0,1]:
    rd=run+'/calls/session-'+str(session)
    for rel in rows:
     if rel.startswith(rd+'/'):payload(rel)
    final_bytes=payload(rd+'/counters.bin');final=counters(final_bytes);check(final['read']==0,'no policy84 readercallbacks')
    events=[]
    for ei in range(final['event']):
     data=payload(rd+'/event-'+str(ei)+'.bin');check(all(b<128 for b in data),'originalevents ASCII only');text=data.decode('ascii');kind=text.split(' ',1)[0];event_counts[kind]+=1;events.append({'index':ei,'utf8Hex':data.hex(),'independentUtf16Captured':False,'encodingEvidence':'originalJavaUTF8only;allASCII;noindependentJavaUTF16eventcapture'});all_console.append(data+b'\n')
    calls=[c for c in observation['calls']if c['session']==session];check([c['expression']for c in calls]==expressions,'API expression order');expected=[]
    for ci,call in enumerate(calls):
     raw=rd+'/outer-'+str(ci);check(call['rawPrefix']==raw,'API rawprefix');check(payload(raw+'.expression.bin')==expressions[ci].encode()and payload(raw+'.source.bin')==b'main.pkl','source/expression bytes')
     status=payload(raw+'.status.bin');check(status in[b'success\n',b'error\n']and status.decode()==call['status'],'exact ordinary status');api_counts[status.decode()[:-1]]+=1
     entry={'status':status.decode()[:-1]}
     for channel in ['stdout','stderr']:
      raw_bytes=payload(raw+'.'+channel+'.bin');unit_bytes=payload(raw+'.'+channel+'.code-units.bin');check(java_utf8(unit_bytes)==raw_bytes,'independentAPIUTF16/JavaUTF8 relationship');entry[channel+'Hex']=raw_bytes.hex();entry[channel+'Units']=utf16(unit_bytes)
      if channel==('stdout'if status==b'success\n'else'stderr'):check(call['utf8Hex']==raw_bytes.hex()and call['utf16BEHex']==unit_bytes.hex()and call['utf8Sha256']==sha(raw_bytes)and call['utf16BESha256']==sha(unit_bytes),'report fullAPIchannel binding')
     for side in ['Before','After']:
      data=payload(raw+'.counters-'+side.lower()+'.bin');check(data.decode()==call['counters'+side],'report/raw completecountertext');parsed=counters(data);check(payload(raw+'.reads-'+side.lower()+'.bin')==str(parsed['read']).encode(),'report-bound readcounter');entry['counters'+side]=parsed;entry['counterBytes'+side+'Hex']=data.hex();entry['reads'+side]=parsed['read']
     error_class=payload(raw+'.error-class.bin');check(error_class.decode()==call['errorClass'],'reporterrorclass');entry['errorClassUtf8Hex']=error_class.hex();entry['messageNullRawHex']=payload(raw+'.message-null.bin').hex();entry['eventStart']=entry['countersBefore']['event'];entry['eventEnd']=entry['countersAfter']['event'];check(entry['eventEnd']<=len(events),'call event range')
     expected.append(entry)
    fixtures.append({'id':name,'repeat':repeat,'session':session,'source':source.decode(),'uri':case['sourceUri'],'moduleName':'main','operations':case['operations'],'expressions':expressions,'readerPlan':[],'policyProfile':{'mode':case['mode'],'kind':'actualJavaStandardSecurityManagerdelegate','allowedModulesPattern':('(?!pkl:json$).*'if case['mode']=='deny-json'else'(?!pkl:(?:base|ref|test)$).*'if case['mode']=='deny-singletons'else'.*'),'allowedResourcesPattern':'.*','trustLevels':'SecurityManagers.defaultTrustLevels','rootDirectory':None,'rawSecurityManagerCauseMessage':'notcapturedbyoldhelper;freshcausecontrolrequired;do-not-stripfullPklstderr'},'expected':expected,'orderedEvents':events,'finalCountersHex':final_bytes.hex(),'finalCounters':final,'lifecycleBoundary':'JavaactualprimarypathandperAPIreplresolve observed. CurrentSourceSessionprimarysuppliedtext/replAPI lacksfullcallbacks; full280event/counterparity remainsfailed/unexecuted untilimplemented, no synthetic insertion.','eventEncodingBoundary':'Existingevents rawJavaUTF8only,ASCII; APIvalue/errorchannels independentlycapturedUTF16andJavaUTF8.'})
   check(b''.join(all_console)==payload(run+'/stdout.bin'),'full Java process console/event order');check(sha(payload(run+'/stdout.bin'))==proc['result']['sha256']['stdout']and sha(payload(run+'/stderr.bin'))==proc['result']['sha256']['stderr'],'actualprocessraw hashes')
 check(len(fixtures)==12 and sum(len(f['expected'])for f in fixtures)==84 and sum(len(f['orderedEvents'])for f in fixtures)==280,'policy84/280coverage');check(dict(api_counts)=={'success':80,'error':4},'80success4error baseline')
 bundle={'schema':1,'fixtures':fixtures,'javaApiCalls':84,'javaOrderedEvents':280,'javaApiStatusCounts':dict(api_counts),'eventKinds':dict(event_counts),'fullEventParityRequirement':'Full280 rawbytes/order/allcounters mandatory; admission-only/API-only matches are separate andcannotclosefullgate.','unexecutedBoundaries':['publicprimary/repllifecycle parity','customfactory/keykind/isCached=false profile','import* per-entryadmission','fullURIequality/cacheprofile','cross-evaluatorJVMsingletonlifetime']}
 return bundle,proofs,required

def export(output):
 check(not output.exists(),'fresh output required')
 def original(relative):return (ORIGINAL/safe_relative(relative.removeprefix(PREFIX+'/'))).read_bytes()
 bundle,proofs,required=construct(original);output.mkdir(parents=True)
 for rel in required:
  p=output/rel;p.parent.mkdir(parents=True,exist_ok=True);p.write_bytes(original(rel))
 (output/'fixtures.json').write_text(json.dumps(bundle,indent=2)+'\n');manifest={'schema':1,'fixedPins':list(PINS),'sourceProvenance':proofs,'files':{}}
 for rel in sorted(file_set(output)):
  data=(output/rel).read_bytes();manifest['files'][rel]={'sha256':sha(data),'bytes':len(data)}
 (output/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n');return verify(output)
def verify(root):
 read=fixture_reader(root);manifest=json.loads(read('manifest.json'));bundle,proofs,required=construct(read);expected=required|{'fixtures.json','manifest.json'};actual=file_set(root,expected_files=expected);check(actual==set(manifest['files'])|{'manifest.json'}and actual==expected,'exact manifest/regularpathset')
 for rel,row in manifest['files'].items():
  data=read(rel);check(row=={'sha256':sha(data),'bytes':len(data)},'outermanifest payload')
 check(json.loads(read('fixtures.json'))==bundle,'fixture differs pinnedJava fullraw/events/counters');check(manifest['fixedPins']==list(PINS)and manifest['sourceProvenance']==proofs,'sourceprovenance mapping')
 return {'schema':1,'fixtures':12,'javaApiCalls':84,'javaOrderedEvents':280,'statusCounts':bundle['javaApiStatusCounts'],'eventKinds':bundle['eventKinds'],'payloadFiles':len(actual)-1,'manifestSha256':sha(read('manifest.json')),'fullRawAndCounterAndEventBindingsVerified':True,'eventUtf16IndependentCapture':False,'evaluatorExecutions':0}
if __name__=='__main__':
 p=argparse.ArgumentParser();g=p.add_mutually_exclusive_group(required=True);g.add_argument('--output',type=Path);g.add_argument('--fixtures',type=Path);a=p.parse_args();print(json.dumps(export(a.output)if a.output else verify(a.fixtures),indent=2))
