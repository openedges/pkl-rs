from pathlib import Path
import json,hashlib,shutil
O=Path(__file__).resolve().parent;P=O.parent/'stage51-collection-oracle-02';A=O.parent/'stage51-collection-oracle-01'
id='catch-source-properties';w=O/'workspace'/id;w.mkdir(parents=True,exist_ok=False)
s='''import "pkl:test"
stable = 7
insideList = List(1,2,3).map((x)->test.catchOrNull(()->if(trace(x)==2)throw("inner") else x))
outerList = test.catch(()->List(1,2,3).map((x)->if(trace(x)==2)throw("outer") else x))
insideSet = Set(1,2,3).mapNonNull((x)->test.catchOrNull(()->if(trace(x)==2)throw("inner") else x))
outerMap = test.catch(()->Map(1,10,2,20,3,30).map((k,v)->if(trace(k)==2)42 else Pair(k,v)))
'''
(w/'input.pkl').write_text(s);exprs=['stable','insideList.toString()','outerList.toString()','insideSet.toString()','outerMap.toString()','insideList.toString()','outerList.toString()'];ops=[{'kind':'evaluate','value':e}for e in exprs]+[{'kind':'gc'},{'kind':'evaluate','value':'stable'}]
(w/'operations.tsv').write_text(''.join(op['kind']+'\t'+op.get('value','').encode().hex()+'\n'for op in ops));(w/'reader-plan.tsv').write_text('')
cases=[{'id':id,'workspace':str(w),'source':str(w/'input.pkl'),'sourceUri':(w/'input.pkl').as_uri(),'operations':ops,'readerPlan':[],'scope':'Actual module-owned imports/callback catch; source/query ownership intentionally differs from failed01/02 setup.'}]
(O/'cases.json').write_text(json.dumps(cases,indent=2)+'\n')
for name in ['CollectionProbe.java','original-binding.json','launch.py']:shutil.copyfile(P/name,O/name)
s=(P/'run.py').read_text().replace("'plannedApiCalls':88","'plannedApiCalls':32");(O/'run.py').write_text(s)
(O/'predecessor.json').write_text(json.dumps({'root':str(P),'reportSha256':hashlib.sha256((P/'report.json').read_bytes()).hexdigest(),'preservedAdditionalIntentNotReachedApiCalls':16,'original01Root':str(A),'originalAdapterNotReexecuted':True,'change':'Import-first remains. Actual catch calculations now module properties; API queries exported property names, not private import binding.'},indent=2)+'\n')
