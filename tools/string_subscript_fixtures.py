#!/usr/bin/env python3
"""Verify actual Java string-subscript API fixtures; export one ordered graph."""
from __future__ import annotations
import argparse
import hashlib
import json
from pathlib import Path
ROOT = Path(__file__).resolve().parents[1]
DEFAULT = ROOT / 'tests/fixtures/m2-string-subscripts'
PINS = {
 'cc7a5029ee4f39f4f3f9ef7b3ea4857b093428dc6de28e09f84d26a0b73a310f': ('44b01237b49913413e17c20bdb871c1e908b479c0b6e39ef014979cf7a569520', ['ascii-valid','supplementary-first','supplementary-second','supplementary-end','mixed-unpaired-high','mixed-supplementary','mixed-unpaired-low','mixed-tail','negative','end','empty','huge-integer','non-integer','parenthesized-negative']),
 '948d4fe90e7b44eabea00c9980666d4852d8b364bc5fc34dfd8b878ff79e8fa1': ('709fb260761e1b4700fe631791eaf75dcbb73f6e646fbb97a8b8a0c9c8078c8e', ['mixed-out-of-range','unpaired-index-type']),
}
def require(value, message):
 if not value: raise ValueError(message)
def digest(data): return hashlib.sha256(data).hexdigest()
def read(root, name):
 path=Path(name)
 require(name and not path.is_absolute() and '..' not in path.parts, 'unsafe fixture path')
 require(not any((root/p).is_symlink() for p in [path,*path.parents]), 'fixture symlink')
 return (root/path).read_bytes()
def load(root,name): return json.loads(read(root,name))
def verify_package(root=DEFAULT):
 root=Path(root)
 manifest,data,provenance=[load(root,n) for n in ('manifest.json','cases.json','provenance.json')]
 actual={p.relative_to(root).as_posix() for p in root.rglob('*') if p.is_file() and p.name!='manifest.json'}
 require(actual==set(manifest['files']), 'fixture membership differs')
 for path,expected in manifest['files'].items(): require(digest(read(root,path))==expected,'fixture payload differs: '+path)
 report_raw,closure_raw=read(root,'origin/report.json'),read(root,'origin/closure.json'); report_hash=digest(report_raw)
 require(report_hash in PINS and digest(closure_raw)==PINS[report_hash][0], 'pinned oracle identity differs')
 require(provenance['reportSha256']==report_hash and provenance['closureSha256']==digest(closure_raw),'declared oracle identity differs')
 report,closure=json.loads(report_raw),json.loads(closure_raw); inventory_raw=read(root,'origin/inventory.json')
 require(digest(inventory_raw)==closure['inventorySha256'],'oracle inventory differs')
 entries={row['path']:row['sha256'] for row in json.loads(inventory_raw)}
 entries.update({'inventory.json':digest(inventory_raw),'closure.json':digest(closure_raw)})
 bindings={row['path']:row for row in provenance['bindings']}
 require(len(bindings)==len(provenance['bindings']),'duplicate origin binding')
 require({'origin/cases.json',data['sourceFile']}<=set(bindings),'missing mandatory declaration/source binding')
 for path,row in bindings.items(): require(entries.get(row['originPath'])==row['sha256']==digest(read(root,path)),'origin binding differs')
 original=load(root,'origin/cases.json');ids=[c['id'] for c in data['cases']]
 require(ids==[c['id'] for c in original]==manifest['orderedIds']==PINS[report_hash][1],'ordered selection differs')
 require(manifest['caseCount']==len(ids) and data['schema']==manifest['schema']==1,'case accounting differs')
 require(not report['repeatDifferences'] and not report['inputChanges'],'oracle unstable')
 require(report['processCount']==len(report['records'])==2 and report['evaluationCount']==2*len(ids),'oracle process count differs')
 for record in report['records']:
  require(record['result']['exit_code']==0 and not record['result']['timed_out'],'oracle process failed')
  require(record['workspaceBefore']==record['workspaceAfter'],'oracle workspace changed')
 source=read(root,data['sourceFile']);source_hash=digest(source);expected=[]
 # Copied declarations are not independent authority for the actual API context.
 # Anchor source/URI directly to every hardpinned Java process record too.
 for record in report['records']:
  require(record['moduleUri']==data['moduleUri'] and record['sourceSha256']==source_hash,'pinned process source/URI context differs')
 for i,(case,declaration) in enumerate(zip(data['cases'],original)):
  require(case['sourceFile']==data['sourceFile'] and case['sourceSha256']==declaration['sourceSha256']==source_hash
   and source==declaration['source'].encode() and case['moduleUri']==declaration['moduleUri']==data['moduleUri']
   and case['moduleName']=='main' and case['expression']==declaration['expression']
   and case['callPrefix']==declaration['callPrefix']==f'outer-{i}','source/expression API context differs')
  e=case['expected'];require(e['kind'] in ('value','error') and len(case['repeats'])==2,'Java API kind/repeats differ')
  channel='stdout' if e['kind']=='value' else 'stderr'; captured=[]
  for repeat,row in enumerate(case['repeats']):
   prefix=f'raw/{repeat}/outer-{i}';status=b'success\n' if channel=='stdout' else b'error\n'
   paths={'statusFile':prefix+'.status.bin','expressionFile':prefix+'.expression.bin','stdoutFile':prefix+'.stdout.bin',
    'stderrFile':prefix+'.stderr.bin','utf8File':prefix+'.'+channel+'.bin','utf16BEFile':prefix+'.'+channel+'.code-units.bin','errorClassFile':prefix+'.error-class.bin'}
   require(row['repeat']==repeat and all(row[k]==v and v in bindings for k,v in paths.items()),'ordered raw call differs')
   raw,units=read(root,row['utf8File']),read(root,row['utf16BEFile']); error_class=read(root,row['errorClassFile']).decode()
   require(read(root,row['statusFile'])==status and read(root,row['expressionFile'])==case['expression'].encode()
    and read(root,row['stderrFile'] if channel=='stdout' else row['stdoutFile'])==b''
    and len(units)%2==0 and row['javaErrorClass']==error_class,'actual Java channels differ')
   observed=report['observations'][repeat]['calls'][i]
   require(observed['id']==case['id'] and observed['expression']==case['expression'] and observed['utf8Hex']==raw.hex()
    and observed['utf16BEHex']==units.hex() and observed['status'].encode()==status and observed['errorClass']==error_class,'oracle call linkage differs')
   captured.append((raw,units,error_class))
  require(captured[0]==captured[1],'raw Java repeats differ');raw,units,error_class=captured[0];oracle_error='PklBugException' in error_class
  require(e['utf8File']==case['repeats'][0]['utf8File'] and e['utf16BEFile']==case['repeats'][0]['utf16BEFile']
   and e['javaStatus']==('success' if channel=='stdout' else 'error') and e['javaErrorClass']==error_class
   and e['oracleError']==oracle_error,'expected channel/classification differs')
  expected.append({'actionIndex':i,'kind':e['kind'],'javaStatus':'oracle-error' if oracle_error else e['javaStatus'],
   'javaErrorClass':error_class,'javaUtf8Hex':raw.hex(),
   'javaValueUnits' if channel=='stdout' else 'javaErrorUnits':[int.from_bytes(units[n:n+2],'big') for n in range(0,len(units),2)]})
 graph={'id':'string-subscripts-'+str(len(ids)),'operation':'expressions','source':source.decode(),'sourceSha256':source_hash,
  'uri':data['moduleUri'],'moduleName':'main','paths':[c['expression'] for c in data['cases']],
  'actions':[{'kind':'path','index':i} for i in range(len(ids))],'resolutions':[],'sources':[],'powerAssertions':False,'logger':{'callbackPaths':[]},
  'expected':{'results':expected,'callbacks':[],'logs':[]},'provenance':{'oracleReportSha256':report_hash,'orderedCaseIds':ids}}
 return {'schema':1,'scope':'One evaluator per ordered Java source batch; fresh evaluator twice. Exact UTF16 and separate Java UTF8 serialization.',
  'caseCount':len(ids),'javaProcesses':2,'actualJavaEvaluations':2*len(ids),'graphFixtures':[graph]}
if __name__=='__main__':
 parser=argparse.ArgumentParser();parser.add_argument('--fixtures',type=Path,default=DEFAULT)
 args=parser.parse_args();print(json.dumps(verify_package(args.fixtures),indent=2))
