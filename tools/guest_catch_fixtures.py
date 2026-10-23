"""Verify every Stage45 guest-catch source/call against immutable Java records."""
from pathlib import Path
import argparse, hashlib, json, os
PINS={
 'initial':{'report.json':'d20b07c0e269bb923c3b0a91c81adbfacc3314bd29cabaf5dec9fb3a60611fea','closure.json':'68b3a8a5a85fe17c62cc962cb93e92cc0b3a8fee6f2aa472b134d2d5a0f89d87','cases.json':'2187d66444c59fa80feabb7a9fb57fc9cd0e1045445b0d7a02d9d597e0e14688'},
 'corrected':{'report.json':'e6dc08feebf6e8c11c8a056116ebe3711797cbf991d6118284019078a6e782ce','closure.json':'a8961697df1901f1f97923d55a57d993f652330a473fb62298d0eb730f35e231','cases.json':'df7e6cf4841bd7c45f9de9d74027bbfd723e38d80a51422dac02f68e966ae593'}}
PINS.update({'lazy-return': {'report.json': '3e24d8658ebc4f08733e63b3400a813d15c2a5a6b42b4aaa4340321423505c3b', 'closure.json': '16291377580331e481e9716b123871e7c3e9ad030e5bab944682be99802bf02a', 'cases.json': 'eb68698f7fdcc8f8bebf5b666d1a22bfbee011f442ddd81877f3c7c6cb8847e9'}, 'initial-force': {'report.json': 'b45321a5740fc93e715fcfe379bb480c276e1e83a924b540ba36f25438bfb932', 'closure.json': 'a5eb5512509d9ef633865b5247bcb56c10a7df9f07f8fe168b37019196e04a82', 'cases.json': 'cf2e7f4a8ee4983c7938dc4083296adadd419154d63190bb739c1a58c10c292a'}, 'corrected-force': {'report.json': '4d7266cf7947b163024983fbb18827e04fcd85c0b7c1cfe63349a9200c42e64c', 'closure.json': '5340a5a1d23c2e89d77cb142679c33f26d468361207c69f59d890670e7940331', 'cases.json': '8da8c9c32c2aa86b8416b7c833d20b0bcf0277957b2f2a4c2ac84ca0a4a53ba2'}})
BLOCKED={'stdlib-singleton-and-builtin-identity','resolve-retry-after-guest-catch','source-failure-cache-after-guest-catch','source-callback-inner-publication-and-catch'}
def sha(raw):return hashlib.sha256(raw).hexdigest()
def need(value,message):
 if not value:raise ValueError(message)
def selected(name):
 return name in ['cases.json','report.json','profile.json','GuestCatchProbe.java','interpretation.json','correction-provenance.json']or name.startswith('workspaces/')or name.startswith('upstream/')or(name.startswith('cases/')and('/calls/'in name or name.endswith('/process.json')or name.endswith('/stdout.bin')or name.endswith('/stderr.bin')or name.endswith('/result.json')))
def verify(fixtures):
 root=Path(fixtures).resolve();manifest=json.loads((root/'manifest.json').read_bytes());need(manifest['schema']==1,'fixture schema');names=set()
 for directory,dirs,files in os.walk(root,topdown=True,followlinks=False):
  for name in dirs:need(not(Path(directory)/name).is_symlink(),'fixture directory symlink')
  for name in files:
   path=Path(directory)/name;need(not path.is_symlink(),'fixture symlink');rel=str(path.relative_to(root))
   if rel!='manifest.json':names.add(rel)
 need(names==set(manifest['files']),'fixture membership')
 for name,row in manifest['files'].items():
  raw=(root/name).read_bytes();need(len(raw)==row['bytes']and sha(raw)==row['sha256'],'fixture payload: '+name)
 provenance=json.loads((root/'provenance.json').read_bytes());selection=json.loads((root/'selection.json').read_bytes());need(selection['schema']==1,'selection schema');actual=selection['cases'];expected_order=[];bindings={};outer=callbacks=0
 for group,pins in PINS.items():
  base=root/group
  for name,digest in pins.items():need(sha((base/name).read_bytes())==digest,'pinned Java '+group+'/'+name)
  closure=json.loads((base/'closure.json').read_bytes());need(sha((base/'inventory.json').read_bytes())==closure['inventorySha256'],'pinned original inventory');inventory={r['path']:r for r in json.loads((base/'inventory.json').read_bytes())['files']};report=json.loads((base/'report.json').read_bytes());cases=json.loads((base/'cases.json').read_bytes());origin=provenance['origins'][group];original=Path(cases[0]['workspace']).parents[1];need(origin=={'originalRoot':str(original),'reportSha256':pins['report.json'],'closureSha256':pins['closure.json'],'manifestSha256':pins['cases.json'],'inventorySha256':closure['inventorySha256']},'origin context')
  required={n for n in inventory if selected(n)}|{'inventory.json','closure.json'}
  for name in required:
   value=(base/name).read_bytes();digest=sha(value)
   if name in inventory:need(digest==inventory[name]['sha256']and len(value)==inventory[name]['bytes'],'original copied bytes '+name)
   bindings[str(Path(group)/name)]={'originalPath':str(original/name),'fixturePath':str(Path(group)/name),'bytes':len(value),'sha256':digest}
  need(sha((base/'profile.json').read_bytes())==report['profileSha256'],'original profile')
  for case in cases:
   identity=case['id'];expected_order.append((group,identity));row=actual[len(expected_order)-1];need(row['group']==group and row['id']==identity,'case order/coverage');source=base/'workspaces'/identity/'main.pkl';need(sha(source.read_bytes())==case['sourceSha256']and source.read_text()==case['source'],'original source')
   need(row['sourceFile']==str(source.relative_to(root))and row['sourceSha256']==case['sourceSha256'],'selected source');need(row['moduleUri']==case['moduleUri']==(Path(case['workspace'])/'main.pkl').as_uri(),'original physical URI');need((source.parent/'uri.txt').read_text()==case['moduleUri']and(source.parent/'mode.txt').read_text()==case['mode'],'derived context');need(row['mode']==case['mode']and row['expressions']==case['expressions']and row['callbacks']==case['callbacks'],'ordered inputs')
   scope='frontend-constructor-only'if (group=='initial'and identity in BLOCKED)or group=='initial-force' else'terminal-partial-parity'if case['mode'].endswith('-close')or case['mode'].endswith('-timeout')else'full-ordered-api';need(row['comparisonScope']==scope,'boundary scope cannot be promoted')
   for repeat in [0,1]:
    observation=next(o for o in report['observations']if o['case']==identity and o['repeat']==repeat);calls=observation['calls'];need(observation['executedOuterCalls']==len(case['expressions']),'original complete outercalls');outer+=len(case['expressions']);callbacks+=observation['executedCallbackCalls'];record=next(o for o in report['records']if o['case']==identity and o['repeat']==repeat);need(record['moduleUri']==case['moduleUri']and record['sourceSha256']==case['sourceSha256'],'process context');need(record['workspaceBefore']==record['workspaceAfter'],'original workspace mutation')
    for call in calls:
     prefix=call['rawPrefix'];p=base/prefix;index=int(call['prefix'].split('-')[-1]);expression=case['expressions'][index]if call['prefix'].startswith('outer-')else case['callbacks'][index];need(call['expression']==expression and p.with_name(p.name+'.expression.bin').read_text()==expression,'original call expression');status=p.with_name(p.name+'.status.bin').read_text();need(status==call['status']and status in ['success\n','error\n'],'original call status');channel='stdout'if status=='success\n'else'stderr';other='stderr'if channel=='stdout'else'stdout';raw=p.with_name(p.name+'.'+channel+'.bin').read_bytes();units=p.with_name(p.name+'.'+channel+'.code-units.bin').read_bytes();need(sha(raw)==call['utf8Sha256']and raw.hex()==call['utf8Hex'],'pinned expected UTF8');need(len(units)%2==0 and sha(units)==call['utf16BESha256']and units.hex()==call['utf16BEHex'],'pinned expected UTF16');need(units.decode('utf-16-be','surrogatepass').encode('utf-8','replace')==raw,'Java serialization boundary');need(not p.with_name(p.name+'.'+other+'.bin').read_bytes()and not p.with_name(p.name+'.'+other+'.code-units.bin').read_bytes(),'complete opposite channels')
 need(len(actual)==len(expected_order)==21,'all twenty-one actual sources');rows=provenance['bindings'];need(len(rows)==len(bindings)and len({r['fixturePath']for r in rows})==len(rows),'mandatory binding cardinality');need(all(bindings.get(r['fixturePath'])==r for r in rows),'mandatory original copied bindings')
 return {'passed':True,'cases':len(actual),'orderedOuterCalls':outer,'actualCallbacks':callbacks,'copiedOriginalBindings':len(bindings),'scopeCounts':{scope:sum(row['comparisonScope']==scope for row in actual)for scope in ['full-ordered-api','frontend-constructor-only','terminal-partial-parity']},'scope':'All initial14, additivecorrected4 and supplemental03/04/05 preserved. Complete Java raw unit/byte strings, followups/logs/events/counters bound. Early constructor and terminal/internal followups are distinct from full API compatibility.'}
if __name__=='__main__':
 p=argparse.ArgumentParser();p.add_argument('--fixtures',type=Path,default=Path(__file__).resolve().parents[1]/'tests/fixtures/m2-guest-catch');print(json.dumps(verify(p.parse_args().fixtures),indent=2))
