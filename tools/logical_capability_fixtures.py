"""Verify compact actual Java logical-owner and ModuleKey capability raw fixtures."""
from pathlib import Path
import argparse,hashlib,json,os,re,stat,struct,sys
PINS={'logical': {'root': '/Users/cody.ij.hwang/Projects/pkl-rs.stage46/target/m2/stage46-logical-binding-oracle-01', 'reportSha256': 'db1b7305da5f564a3b670b8e3deee304e7188ee579138b7cf5d9187a4ce64765', 'inventorySha256': '281fa7f68de56960018e08b16cae1b57cf2f1977193945434afe0af36866c75b', 'closureSha256': 'b2f54f1e12c94b405d7a5ba1f6e2f06211b26e067dc27be0773e2d6dcb65c528'}, 'capability': {'root': '/Users/cody.ij.hwang/Projects/pkl-rs.stage46/target/m2/stage46-module-capability-oracle-03', 'reportSha256': 'eda49c0ed06b2320b33dd877391b64adb62a0611deac09d0a828da1d0e012740', 'inventorySha256': '0ea0c4fa48550771ca8ad0c313baca78b43a2f59a5999c50f8dbcc32a2010caf', 'closureSha256': 'a1b71530f9b68603d7fd47faceea08e0f6ac5380123d50434a2f22b1b538e1b1'}}
SELECTED={'logical':{'coalesce-lambda-owner'},'capability':{'triple-dot-nonlocal','triple-dot-nonhierarchical'}}
SUFFIXES=['status.bin','stdout.bin','stderr.bin','stdout.code-units.bin','stderr.code-units.bin','reads-before.bin','reads-after.bin','expression.bin','error-class.bin','counters-before.bin','counters-after.bin']
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def get(root,name):
 p=Path(name)
 if str(p)!=name or p.is_absolute()or'..'in p.parts:raise ValueError('unsafe fixture path')
 q=root/p
 if q.is_symlink()or not q.resolve().is_relative_to(root.resolve()):raise ValueError('unsafe fixture link')
 return q

def verify(root):
 manifest=json.loads((root/'manifest.json').read_bytes());data=json.loads((root/'fixtures.json').read_bytes());provenance=json.loads((root/'provenance.json').read_bytes());bindings=provenance['originalPaths'];used=set();reports={};cases={};inventories={}
 if provenance['pins']!=PINS:raise ValueError('independent Java pins differ')
 if set(manifest['files'])!=({'fixtures.json','provenance.json','cases.tsv','expected.tsv'}|{r['path']for r in bindings.values()}):raise ValueError('manifest/provenance coverage differs')
 def require(original,local):
  record=bindings.get(str(original))
  if record is None or record['path']!=local:raise ValueError('required original/raw path differs')
  used.add(str(original));return get(root,local)
 for group,pin in PINS.items():
  original=Path(pin['root'])
  for name,key in [('report.json','reportSha256'),('inventory.json','inventorySha256'),('closure.json','closureSha256')]:
   p=require(original/name,'provenance/'+group+'/'+name)
   if sha(p)!=pin[key]:raise ValueError('pinned Java report/inventory/closure differs')
  report=json.loads((root/'provenance'/group/'report.json').read_bytes());reports[group]=report;inv=json.loads((root/'provenance'/group/'inventory.json').read_bytes());inventories[group]={r['path']:r for r in inv['files']};closed=json.loads((root/'provenance'/group/'closure.json').read_bytes())
  if not closed['closed']or closed['inventorySha256']!=pin['inventorySha256']or closed['reportSha256']!=pin['reportSha256']:raise ValueError('Java closure differs')
  p=require(original/'cases.json','provenance/'+group+'/cases.json')
  if sha(p)!=report['inputHashes'][str(original/'cases.json')]:raise ValueError('original Java cases differ')
  cases[group]=json.loads(p.read_bytes())
 for original,record in bindings.items():
  groups=[g for g,p in PINS.items()if Path(original).is_relative_to(Path(p['root']))]
  if len(groups)!=1:raise ValueError('original outside pinned roots')
  group=groups[0];relative=Path(original).relative_to(Path(PINS[group]['root'])).as_posix();pin=inventories[group].get(relative)
  if relative in {'inventory.json','closure.json'}:
   if record['sha256']!=PINS[group]['inventorySha256'if relative=='inventory.json'else'closureSha256']:raise ValueError('seal original binding differs')
  elif pin is None or record['sha256']!=pin['sha256']or record['bytes']!=pin['bytes']:raise ValueError('pinned inventory raw binding differs')
  if relative in {'report.json','cases.json','inventory.json','closure.json'}:expected='provenance/'+group+'/'+relative
  elif relative.startswith('cases/'):expected='raw/'+record['sha256']+'.bin'
  else:
   owners=[c for c in cases[group]if str(Path(c['workspace'])/c['operations'][0][0])==original and c['id']in SELECTED[group]]
   if len(owners)!=1 or reports[group]['inputHashes'][original]!=record['sha256']:raise ValueError('selected source identity differs')
   expected='sources/'+owners[0]['id']+'.pkl'
  if record['path']!=expected or manifest['files'][expected]!={'sha256':record['sha256'],'bytes':record['bytes']}:raise ValueError('original/local binding differs')
 # Authenticate original/local paths before allowing descent into fixture directories.
 expected_files=set(manifest['files'])|{'manifest.json'};directories={'.'}
 for name in expected_files:
  get(root,name);directories.update(str(q)for q in Path(name).parents)
 pending=[root];actual=set()
 while pending:
  directory=pending.pop()
  with os.scandir(directory)as entries:
   for e in entries:
    path=Path(e.path);name=path.relative_to(root).as_posix();mode=e.stat(follow_symlinks=False).st_mode
    if stat.S_ISDIR(mode):
     if name not in directories:raise ValueError('unexpected fixture directory')
     pending.append(path)
    elif stat.S_ISREG(mode):actual.add(name)
    else:raise ValueError('nonregular fixture entry')
 if actual!=expected_files:raise ValueError('actual fixture file coverage differs')
 for name,record in manifest['files'].items():
  p=get(root,name)
  if sha(p)!=record['sha256']or p.stat().st_size!=record['bytes']:raise ValueError('fixture bytes differ')
 selected_cases=[(g,c['id'])for g in PINS for c in cases[g]if c['id']in SELECTED[g]]
 if [(c['group'],c['id'])for c in data['cases']]!=selected_cases:raise ValueError('selected case coverage differs')
 count=0
 for case in data['cases']:
  group=case['group'];original=next(c for c in cases[group]if c['id']==case['id']);source=Path(original['workspace'])/original['operations'][0][0];require(source,case['sourcePath'])
  if case['sourcePath']!='sources/'+case['id']+'.pkl'or case['source'].encode()!=get(root,case['sourcePath']).read_bytes()or case['sourceUri']!=original['sourceUri']or case['moduleName']!='main':raise ValueError('source/URI differs')
  if case['capabilities']!={'local':original['mode']!='cap-nonlocal','hierarchical':original['mode']!='cap-nonhierarchical','fragmentPaths':False}:raise ValueError('capability input differs')
  observations=[o for o in reports[group]['observations']if o['case']==case['id']];keys=[(o['repeat'],session)for o in observations for session in [0,1]]
  if [(o['repeat'],o['session'])for o in case['observations']]!=keys:raise ValueError('Java observation coverage differs')
  for observation in case['observations']:
   calls=[c for o in observations if o['repeat']==observation['repeat']for c in o['calls']if c['session']==observation['session']]
   if len(calls)!=len(case['expected'])or len(calls)!=len(observation['calls'])or case['expressions']!=[c['expression']for c in calls]:raise ValueError('API sequence differs')
   for expected,rawref,call in zip(case['expected'],observation['calls'],calls):
    prefix=str(Path(PINS[group]['root'])/call['rawPrefix'])
    if rawref['prefix']!=call['prefix']or rawref['originalPrefix']!=prefix or set(rawref['refs'])!=set(SUFFIXES):raise ValueError('raw API provenance differs')
    def read(suffix):return require(prefix+'.'+suffix,rawref['refs'][suffix]).read_bytes()
    status=read('status.bin')
    if status not in {b'success\n',b'error\n'}or status!=call['status'].encode():raise ValueError('exact Java status differs')
    target='stdout'if status==b'success\n'else'stderr'
    if read(target+'.bin').hex()!=call['utf8Hex']or read(target+'.code-units.bin').hex()!=call['utf16BEHex']or read('expression.bin')!=call['expression'].encode()or read('error-class.bin')!=call['errorClass'].encode():raise ValueError('raw Java call differs')
    for label,suffix,raw_count in [('countersBefore','counters-before.bin','reads-before.bin'),('countersAfter','counters-after.bin','reads-after.bin')]:
     if read(suffix)!=call[label].encode()or read(raw_count)!=dict(x.split('=',1)for x in call[label].splitlines())['read'].encode():raise ValueError('pinned Java counters differ')
    def units(channel):
     raw=read(channel+'.code-units.bin')
     if len(raw)%2:raise ValueError('odd UTF16 units')
     return list(struct.unpack('>'+'H'*(len(raw)//2),raw))
    actual={'status':status[:-1].decode(),'stdoutUnits':units('stdout'),'stderrUnits':units('stderr'),'stdoutHex':read('stdout.bin').hex(),'stderrHex':read('stderr.bin').hex(),'readsBefore':int(read('reads-before.bin')),'readsAfter':int(read('reads-after.bin'))}
    if expected!=actual or type(expected['readsBefore'])is not int or type(expected['readsAfter'])is not int:raise ValueError('raw expected outputs/counts differ')
    count+=1
 if used!=set(bindings):raise ValueError('unexpected/unused original bindings')
 if data['schema']!=1 or data['selectedJavaApiCalls']!=count or count!=36 or data['allJavaApiCalls']!=sum(r['apiCalls']for r in reports.values())or data['allJavaApiCalls']!=84 or data['unselectedJavaApiCalls']!=48:raise ValueError('API coverage differs')
 case_lines=''.join('\t'.join([c['id'],c['sourcePath'],c['sourceUri'],c['moduleName']]+[str(c['capabilities'][k]).lower()for k in ['local','hierarchical','fragmentPaths']])+'\n'for c in data['cases'])
 expected_lines=''.join('\t'.join([c['id'],str(i),expr,e['status'],e['stdoutHex'],e['stderrHex'],''.join(f'{w:04x}'for w in e['stdoutUnits']),''.join(f'{w:04x}'for w in e['stderrUnits']),str(e['readsBefore']),str(e['readsAfter'])])+'\n'for c in data['cases']for i,(expr,e)in enumerate(zip(c['expressions'],c['expected'])))
 if(root/'cases.tsv').read_bytes()!=case_lines.encode()or(root/'expected.tsv').read_bytes()!=expected_lines.encode():raise ValueError('TSV/raw-verified JSON differs')
 return {'cases':3,'selectedJavaApiCalls':36,'uniqueExpectedCalls':9,'allJavaApiCalls':84,'unselectedJavaApiCalls':48,'manifestSha256':sha(root/'manifest.json')}
def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--fixtures',required=True,type=Path);a=p.parse_args()
 try:print(json.dumps(verify(a.fixtures),indent=2));return 0
 except(OSError,ValueError,KeyError,StopIteration)as error:print('Logical/capability fixture verification failed: '+str(error),file=sys.stderr);return 1
if __name__=='__main__':sys.exit(main())
