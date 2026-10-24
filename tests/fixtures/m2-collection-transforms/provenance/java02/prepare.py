from pathlib import Path
import hashlib,json,shutil
O=Path(__file__).resolve().parent;P=O.parent/'stage51-collection-oracle-01';old={c['id']:c for c in json.loads((P/'cases.json').read_bytes())};cases=[]
def add(id,source,exprs,predecessor=None):
 w=O/'workspace'/id;w.mkdir(parents=True,exist_ok=False);(w/'input.pkl').write_text(source);(w/'operations.tsv').write_text(''.join('evaluate\t'+e.encode().hex()+'\n' for e in exprs));(w/'reader-plan.tsv').write_text('')
 cases.append({'id':id,'workspace':str(w),'source':str(w/'input.pkl'),'sourceUri':(w/'input.pkl').as_uri(),'operations':[{'kind':'evaluate','value':e}for e in exprs],'readerPlan':[],'scope':'fresh corrected/necessary collection controls; no original01 rewrite','predecessorCase':predecessor})
source=Path(old['callback-types-first-error']['source']).read_text()
add('typed-callback-corrected',source,['stable','List().map((x)->typed(x)).toString()','List(1,2).map((x)->typed(x)).toString()','List(1,2).map((x)->bad(x)).toString()','Map("a",1).map((k,v)->badPair(k,v)).toString()','List().map((x: String)->x).toString()','List(1,2).map((x: String)->x).toString()','stable'],'callback-types-first-error')
source='import "pkl:test"\nstable = 7\n'
add('catch-boundaries-import-first',source,[op['value']for op in old['catch-boundaries']['operations']],'catch-boundaries')
exprs=['stable']
for n in [1,2,3]:exprs.append(f'Set(1,2,3).map((x)->if(trace(x)=={n})throw("set-at-{n}") else x).toString()')
for n in [1,2,3]:exprs.append(f'Map(1,10,2,20,3,30).map((k,v)->if(trace(k)=={n})throw("map-at-{n}") else Pair(k,v)).toString()')
exprs.append('stable');add('set-map-callback-first-failure','stable = 7\n',exprs)
(O/'cases.json').write_text(json.dumps(cases,indent=2)+'\n')
shutil.copyfile(P/'CollectionProbe.java',O/'CollectionProbe.java');shutil.copyfile(P/'original-binding.json',O/'original-binding.json');shutil.copyfile(P/'launch.py',O/'launch.py')
s=(P/'run.py').read_text();start=s.index('base=[]\nfor repeat in range(2):');end=s.index('records=[];callcounts=',start);s=s[:start]+'base=[] # Original adapter baseline reused from sealed01; no repeated acquisition.\n'+s[end:];s=s.replace("'plannedApiCalls':940","'plannedApiCalls':88");(O/'run.py').write_text(s)
(O/'predecessor.json').write_text(json.dumps({'root':str(P),'reportSha256':hashlib.sha256((P/'report.json').read_bytes()).hexdigest(),'retainedIntentNotReachedApiCalls':40,'retainedPostCloseApiBoundaries':12,'originalAdapterNotReexecuted':True,'changedInputs':'Only new typed callback query wrapper expressions and import-first source; original01 unchanged. New Set/Map throw-position control is additive.'},indent=2)+'\n')
print(json.dumps({'cases':3,'plannedApiCalls':88,'originalAdapterReexecutions':0}))
