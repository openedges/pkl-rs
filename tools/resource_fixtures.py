"""Hermetic verification of Java-derived portable Resource fixtures."""
from pathlib import Path
import argparse,hashlib,json,re,sys,importlib.util,os,stat
sys.dont_write_bytecode=True
spec=importlib.util.spec_from_file_location('export',Path(__file__).with_name('export_resource_portable.py'));export=importlib.util.module_from_spec(spec);spec.loader.exec_module(export)
sha=export.sha
SEALED_PINS={'resource': {'root': '/Users/cody.ij.hwang/Projects/pkl-rs.stage46/target/m2/stage46-resource-oracle-01', 'inventorySha256': '85887c02a40d83415f85138efaa299f1e08659c797e1f2e2cc2fb0e1baef3800', 'closureSha256': '27c123fdb66fa186f782a9a9a46936e96174c13959f23b7d76e058cff61ef167', 'reportSha256': 'a7b35392e82804c581a00ccc19689c376917c4552fb46639248e06b1033e60a2'}, 'authority': {'root': '/Users/cody.ij.hwang/Projects/pkl-rs.stage46/target/m2/stage46-uri-authority-oracle-01', 'inventorySha256': '1fdf383c485a675c3389066290199cb444fae906a8df970c7b2cc09adc2ff582', 'closureSha256': '928fb9da672070e3459f5dc00a6e3fcb39b1c3768990d618797363b10f67afe4', 'reportSha256': '6847d1a4e94dbae770da0e4bcb8f2ed9bd40ba8725cf5448f5966018dc5ffc2f'}, 'base64': {'root': '/Users/cody.ij.hwang/Projects/pkl-rs.stage46/target/m2/stage46-base64-oracle-01', 'inventorySha256': '301f7070d380a380f4055efd45d0041a799867881993ddb2952a722a9f3b780a', 'closureSha256': '2d011af0240dceca60a6dda97599402af0a7dc631194bc907097870f1edac81c', 'reportSha256': 'a297a1277f58a675a0780f8b5f0fd0893a48400b7bcc7c8c96f394e63e1474b0'}, 'extra': {'root': '/Users/cody.ij.hwang/Projects/pkl-rs.stage46/target/m2/stage46-resource-extra-oracle-01', 'inventorySha256': 'ad518b295ae11a1c7c1dd5ea21e5be7ec3af7f3b94ed4414919efc196a7fb764', 'closureSha256': 'aabd1290e4b66ab78f32fc5943dfc09e3fac1b96ce647669f6848baf4c2e6ca0', 'reportSha256': '5c4531248ead92e0ee27a48b5a2fef912b4e27318084824e32e65cf23216be31'}}
RAW_SUFFIXES=['status.bin','stdout.bin','stderr.bin','stdout.code-units.bin','stderr.code-units.bin','reads-before.bin','reads-after.bin','expression.bin','error-class.bin','counters-before.bin','counters-after.bin']

def exact_files(root,expected):
 # Derive the directory allowlist before descending; unexpected caches are never walked.
 directories={'.'}
 for name in expected:
  p=Path(name)
  if str(p)!=name or p.is_absolute() or '..' in p.parts:raise ValueError('unsafe/noncanonical manifest path')
  directories.update(str(q) for q in p.parents)
 found=set();pending=[root]
 while pending:
  directory=pending.pop()
  with os.scandir(directory)as entries:
   for entry in entries:
    relative=Path(entry.path).relative_to(root).as_posix();mode=entry.stat(follow_symlinks=False).st_mode
    if stat.S_ISDIR(mode):
     if relative not in directories:raise ValueError('unexpected fixture directory: '+relative)
     pending.append(Path(entry.path))
    elif stat.S_ISREG(mode):found.add(relative)
    else:raise ValueError('nonregular fixture entry: '+relative)
 if found!=expected:raise ValueError('actual regular-file coverage differs')


def contained(root,name):
 p=Path(name)
 if p.is_absolute()or'..'in p.parts:raise ValueError('unsafe fixture path')
 q=root/p
 if not q.resolve().is_relative_to(root.resolve())or q.is_symlink():raise ValueError('unsafe fixture link')
 return q

def derive(group,case,trace,root,bindings):
 uri=trace['uri'];attempt=trace['attempt'];total=trace['total'];work=Path(case['workspace'])
 if group=='authority':return {'kind':'resource','resourceUri':uri,'bytes':list(('payload-'+str(total)).encode())}
 if uri=='res:/missing':return {'kind':'missing'}
 if uri=='res:/retry'and attempt==1:return {'kind':'io','message':'reader-first'}
 if uri=='res:/resource':return {'kind':'resource','resourceUri':'res:/different-returned-uri','bytes':[65,13,10,0,66]}
 if re.fullmatch(r'res:/vector\d+',uri):
  n=uri[len('res:/vector'):];p=work/'payloads'/('vector'+n+'.bin');return {'kind':'resource','resourceUri':'res:/returned/vector'+n,'bytes':list(contained(root,bindings[str(p)]['path']).read_bytes())}
 if re.fullmatch(r'res:/b64-\d+',uri):
  n=uri[len('res:/b64-'):];p=work/'payloads'/('b64-'+n+'.units.bin');return {'kind':'text','units':export.units(contained(root,bindings[str(p)]['path']).read_bytes())}
 if uri in {'res:/key','res:/retry'}:return {'kind':'text','units':export.units(('payload-'+str(total)+' '+uri).encode('utf-16-be'))}
 raise ValueError('unknown captured Java reader rule')

def verify(root):
 manifest=json.loads((root/'manifest.json').read_bytes())
 if manifest['schema']!=1 or not manifest['files']:raise ValueError('empty/unknown manifest')
 data=json.loads((root/'fixtures.json').read_bytes());provenance=json.loads((root/'provenance.json').read_bytes());bindings=provenance['originalPaths']
 semantic={'fixtures.json','provenance.json'}|{v['path']for v in bindings.values()}
 if len(semantic)!=len(bindings)+2 or set(manifest['files'])!=semantic:raise ValueError('manifest/provenance file coverage differs')
 reports={};cases={};inventories={};used_bindings=set()
 if set(provenance['groups'])!=set(export.PINS):raise ValueError('Java provenance group coverage differs')
 def require_binding(original,local,kind):
  record=bindings.get(str(original))
  if record is None or record['path']!=local or record['kind']!=kind:raise ValueError('original raw/input path coverage differs: '+str(original))
  used_bindings.add(str(original))
 for group,(name,pin)in export.PINS.items():
  p=root/'provenance'/group/'report.json'
  if sha(p)!=pin or provenance['groups'][group]['reportSha256']!=pin:raise ValueError('independent Java report pin differs')
  report=json.loads(p.read_bytes());reports[group]=report;casepath=root/'provenance'/group/'cases.json';originalroot=Path(provenance['groups'][group]['originalRoot'])
  if originalroot.name!=name or sha(casepath)!=report['inputHashes'][str(originalroot/'cases.json')]:raise ValueError('Java original cases differ')
  sealed=SEALED_PINS[group];inventory=root/'provenance'/group/'inventory.json';closure=root/'provenance'/group/'closure.json'
  if str(originalroot)!=sealed['root'] or sha(inventory)!=sealed['inventorySha256'] or sha(closure)!=sealed['closureSha256']:raise ValueError('independent sealed inventory/closure pin differs')
  closed=json.loads(closure.read_bytes())
  if closed['inventorySha256']!=sealed['inventorySha256'] or closed['reportSha256']!=pin or not closed['closed']:raise ValueError('original Java closure differs')
  cases[group]={c['id']:c for c in json.loads(casepath.read_bytes())};inventories[group]={r['path']:r for r in json.loads(inventory.read_bytes())['files']}
  for filename in ['report.json','cases.json','inventory.json','closure.json','profile.json']+(['interpretation.json']if 'interpretation.json'in inventories[group]else[]):require_binding(originalroot/filename,'provenance/'+group+'/'+filename,'provenance')
  helper={'resource':'ResourceProbe.java','authority':'AuthorityProbe.java','base64':'Base64Probe.java','extra':'ExtraProbe.java'}[group];helperpath=root/'provenance'/group/helper
  if sha(helperpath)!=report['inputHashes'][str(originalroot/helper)] or provenance['groups'][group]['helperSha256']!=sha(helperpath) or provenance['groups'][group]['javaApiCalls']!=report['apiCalls']:raise ValueError('Java reader rule source differs')
  require_binding(originalroot/helper,'provenance/'+group+'/'+helper,'Java helper')
 for original,record in bindings.items():
  p=contained(root,record['path'])
  if manifest['files'][record['path']]!={'sha256':record['sha256'],'bytes':record['bytes']}:raise ValueError('original path binding differs')
  matches=[g for g,v in provenance['groups'].items()if Path(original).is_relative_to(Path(v['originalRoot']))]
  if len(matches)!=1 or str(Path(original))!=original:raise ValueError('original path outside pinned roots')
  group=matches[0];relative=Path(original).relative_to(Path(provenance['groups'][group]['originalRoot'])).as_posix()
  if relative in {'inventory.json','closure.json'}:
   digest=SEALED_PINS[group]['inventorySha256'if relative=='inventory.json'else'closureSha256']
   if record['sha256']!=digest:raise ValueError('original seal binding differs')
  else:
   pin=inventories[group].get(relative)
   if pin is None or record['sha256']!=pin['sha256'] or record['bytes']!=pin['bytes']:raise ValueError('raw Java inventory binding differs')
  kind=record['kind']
  if kind in {'provenance','Java helper'}:
   expected_local='provenance/'+group+'/'+relative
   if '/'in relative:raise ValueError('noncanonical provenance source path')
  elif kind in {'Java API raw','actual Java reader event'}:
   if not relative.startswith('cases/'):raise ValueError('noncanonical raw source path')
   expected_local='raw/'+group+'/'+relative
  elif kind in {'Pkl source','Java expression sequence','raw resource bytes','raw UTF16 String reader input'}:
   if record['sha256']!=reports[group]['inputHashes'][original]:raise ValueError('Java source/input identity differs')
   owners=[c for c in cases[group].values()if Path(original).is_relative_to(Path(c['workspace']))]
   if len(owners)!=1:raise ValueError('input original workspace differs')
   expected_local='sources/'+group+'/'+owners[0]['id']+'/'+Path(original).relative_to(Path(owners[0]['workspace'])).as_posix()
  else:raise ValueError('unknown original path provenance kind')
  if record['path']!=expected_local:raise ValueError('canonical original/local path mapping differs')
 # Authenticate every path against independent pins before deriving the traversal allowlist.
 exact_files(root,semantic|{'manifest.json'})
 for name,record in manifest['files'].items():
  p=contained(root,name)
  if sha(p)!=record['sha256']or p.stat().st_size!=record['bytes']:raise ValueError('fixture file differs: '+name)

 expected_keys=[];excluded=[]
 for group in export.PINS:
  for ob in reports[group]['observations']:
   for session in [0,1]:
    calls=[c for c in ob['calls']if c['session']==session];selected=[c for c in calls if export.eligible(ob['case'],c)]
    if selected:expected_keys.append((group+'/'+ob['case'],ob['repeat'],session))
    for c in calls:
     if not export.eligible(ob['case'],c):excluded.append((group,ob['case'],ob['repeat'],session,c['prefix'],c['source'],c['expression']))
 if [(f['id'],f['repeat'],f['session'])for f in data['fixtures']]!=expected_keys:raise ValueError('session ordering/coverage differs')
 if [(r['group'],r['id'],r['repeat'],r['session'],r['prefix'],r['source'],r['expression'])for r in data['unexecutedApiBoundaries']]!=excluded:raise ValueError('unexecuted boundaries differ')
 selected_total=0;encoding_differences=0
 for f in data['fixtures']:
  group,case_id=f['id'].split('/',1);case=cases[group][case_id];report=reports[group];ob=next(o for o in report['observations']if o['case']==case_id and o['repeat']==f['repeat']);calls=[c for c in ob['calls']if c['session']==f['session']and export.eligible(case_id,c)];session=next(s for s in provenance['sessions']if(s['id'],s['repeat'],s['session'])==(f['id'],f['repeat'],f['session']))
  if f['source'].encode()!=contained(root,session['sourcePath']).read_bytes()or sha(contained(root,session['sourcePath']))!=report['inputHashes'][str(Path(case['workspace'])/'main.pkl')]or f['uri']!=case['sourceUri']or f['moduleName']!='main':raise ValueError('source/URI identity differs')
  source_original=str(Path(case['workspace'])/'main.pkl');source_local='sources/'+group+'/'+case_id+'/main.pkl'
  if session['sourcePath']!=source_local or session['sourceOriginalPath']!=source_original or session['sourceSha256']!=report['inputHashes'][source_original]:raise ValueError('session source provenance differs')
  require_binding(source_original,source_local,'Pkl source');require_binding(str(Path(case['workspace'])/'operations.tsv'),'sources/'+group+'/'+case_id+'/operations.tsv','Java expression sequence')
  if f['expressions']!=[c['expression']for c in calls]or len(f['expected'])!=len(calls)or len(session['selectedRawJava'])!=len(calls):raise ValueError('API sequence differs')
  expected_reads=set();event_ranges=[]

  for actual,c,rawref in zip(f['expected'],calls,session['selectedRawJava']):
   originalroot=Path(provenance['groups'][group]['originalRoot']);stem='raw/'+group+'/'+c['rawPrefix'];originalstem=str(originalroot/c['rawPrefix'])
   if rawref!={'prefix':c['prefix'],'rawOriginalPrefix':originalstem,'rawLocalPrefix':stem}:raise ValueError('selected raw call provenance differs')
   for suffix in RAW_SUFFIXES:require_binding(originalstem+'.'+suffix,stem+'.'+suffix,'Java API raw')
   read=lambda suffix:contained(root,stem+'.'+suffix).read_bytes();status=export.parse_status(read('status.bin'));target='stdout'if status=='success'else'stderr'
   counters=[]
   for suffix,label,count_suffix in [('counters-before.bin','countersBefore','reads-before.bin'),('counters-after.bin','countersAfter','reads-after.bin')]:
    if read(suffix)!=c[label].encode():raise ValueError('raw counters differ from pinned Java report')
    values=dict(x.split('=',1)for x in c[label].splitlines());counters.append(values)
    if read(count_suffix)!=values['read'].encode():raise ValueError('read count differs from pinned Java report')
   expected_reads.update(range(int(counters[0]['read'])+1,int(counters[1]['read'])+1));event_ranges.append((int(counters[0]['event']),int(counters[1]['event']),int(counters[0]['read']),int(counters[1]['read'])))
   if read('expression.bin')!=c['expression'].encode()or read('error-class.bin')!=c['errorClass'].encode():raise ValueError('raw API identity differs')
   if read('status.bin')!=c['status'].encode()or read(target+'.bin').hex()!=c['utf8Hex']or read(target+'.code-units.bin').hex()!=c['utf16BEHex']or c['errorClass']not in {'','org.pkl.core.PklException'}:raise ValueError('raw Java call/status differs')
   expected={'status':status,'stdoutUnits':export.units(read('stdout.code-units.bin')),'stderrUnits':export.units(read('stderr.code-units.bin')),'stdoutHex':read('stdout.bin').hex(),'stderrHex':read('stderr.bin').hex(),'readsBefore':int(read('reads-before.bin')),'readsAfter':int(read('reads-after.bin'))}
   if type(actual['readsBefore'])is not int or type(actual['readsAfter'])is not int or actual!=expected:raise ValueError('portable expected raw outputs/counts differ from Java')
   for channel in ['stdout','stderr']:
    if export.java_utf8(actual[channel+'Units']).hex()!=actual[channel+'Hex']:raise ValueError('Java UTF16/UTF8 relationship differs')
    node=bytes.fromhex(actual[channel+'Hex']);words=actual[channel+'Units'];js=''.join(chr(w)for w in words).encode('utf-16-le','surrogatepass').decode('utf-16-le','replace').encode('utf-8')
    if js!=node:encoding_differences+=1
   selected_total+=1
  if len(f['readerPlan'])!=len(session['readerTrace'])or expected_reads!=set(range(1,len(session['readerTrace'])+1)):raise ValueError('reader trace count differs')
  last_event=-1
  for index,(plan,trace)in enumerate(zip(f['readerPlan'],session['readerTrace'])):
   directory=Path(calls[0]['rawPrefix']).parent;localdirectory=Path('raw')/group/directory;event_path=Path(trace['rawPath']);event_match=re.fullmatch(r'event-(\d+)\.bin',event_path.name)
   if event_path.parent!=localdirectory or not event_match:raise ValueError('reader event original path differs')
   event_index=int(event_match.group(1))
   if event_index<=last_event or not any(a<=event_index<b and r0<trace['total']<=r1 for a,b,r0,r1 in event_ranges):raise ValueError('reader event order/call range differs')
   last_event=event_index;require_binding(originalroot/directory/event_path.name,trace['rawPath'],'actual Java reader event')
   if re.fullmatch(r'res:/vector\d+',trace['uri']):
    filename='vector'+trace['uri'][len('res:/vector'):]+'.bin';require_binding(Path(case['workspace'])/'payloads'/filename,'sources/'+group+'/'+case_id+'/payloads/'+filename,'raw resource bytes')
   if re.fullmatch(r'res:/b64-\d+',trace['uri']):
    filename='b64-'+trace['uri'][len('res:/b64-'):]+'.units.bin';require_binding(Path(case['workspace'])/'payloads'/filename,'sources/'+group+'/'+case_id+'/payloads/'+filename,'raw UTF16 String reader input')
   value=contained(root,trace['rawPath']).read_bytes();match=re.fullmatch(rb'reader (.*?) attempt=(\d+) total=(\d+)',value)
   if not match or(trace['uri'],trace['attempt'],trace['total'])!=(match.group(1).decode(),int(match.group(2)),int(match.group(3)))or trace['total']!=index+1:raise ValueError('actual Java ordered reader event differs')
   if plan!={'uri':trace['uri'],'result':derive(group,case,trace,root,bindings)}:raise ValueError('reader input plan differs from Java helper rule')
 if used_bindings!=set(bindings):raise ValueError('unexpected/unused original path provenance')
 if [(s['id'],s['repeat'],s['session'])for s in provenance['sessions']]!=expected_keys:raise ValueError('provenance session coverage differs')
 if data['schema']!=1 or data['javaApiCalls']!=sum(r['apiCalls']for r in reports.values())or data['javaApiCalls']!=1272 or selected_total!=1008 or len(excluded)!=264 or len(data['fixtures'])!=20:raise ValueError('coverage counts differ')
 return {'sessions':20,'selectedJavaApiCalls':1008,'allJavaApiCalls':1272,'unexecuted':264,'javaVsJsUtf8DifferentChannels':encoding_differences,'manifestSha256':sha(root/'manifest.json')}
def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--fixtures',required=True,type=Path);a=p.parse_args()
 try:print(json.dumps(verify(a.fixtures),indent=2));return 0
 except(OSError,ValueError,KeyError,StopIteration)as error:print('Resource portable fixture verification failed: '+str(error),file=sys.stderr);return 1
if __name__=='__main__':sys.exit(main())
