from pathlib import Path
import json,shutil
O=Path(__file__).resolve().parent;P=O.parent/'stage51-collection-oracle-03';cases=[]
def add(id,source,exprs):
 w=O/'workspace'/id;w.mkdir(parents=True,exist_ok=False);(w/'input.pkl').write_text(source);(w/'operations.tsv').write_text(''.join('evaluate\t'+e.encode().hex()+'\n'for e in exprs));(w/'reader-plan.tsv').write_text('');cases.append({'id':id,'workspace':str(w),'source':str(w/'input.pkl'),'sourceUri':(w/'input.pkl').as_uri(),'operations':[{'kind':'evaluate','value':e}for e in exprs],'readerPlan':[],'scope':'additive Null constructor/default binding, separate from original1060collection calls'})
source='stable=7\nclass C { n: Int }\n'
args=['new Dynamic { value=1 }','new Listing { 1;2 }','new Mapping { ["a"]=1 }','new C { n=1 }','()->new Dynamic { value=1 }','()->throw("function-default-not-forced")','(x)->new Dynamic {}','1','1.0','"text"','true','null','Pair(1,2)','List(1)','Set(1)','Map("a",1)','Int','1.s']
add('null-argument-guard',source,['stable',*['Null('+a+').toString()'for a in args],'Null().toString()','Null(1,2).toString()','stable'])
source='''stable=7
defaultObject = Null(new Dynamic { a=1 })
amended = defaultObject { a=2 }
defaultFunction = Null(()->new Dynamic { a=3 })
amendedFunction = defaultFunction { a=4 }
unforced = Null(new Dynamic { boom=throw("default-not-forced") })
functionUnforced = Null(()->throw("function-default-not-forced"))
'''
add('null-amendment-and-nonnull',source,['stable','defaultObject.toString()','amended.a.toString()','defaultFunction.toString()','amendedFunction.a.toString()','unforced.toString()','List(1,2).mapNonNull((x)->unforced).toString()','Set(1,2).mapNonNull((x)->functionUnforced).toString()','stable'])
add('null-authored-property-shadow','stable=7\nNull = (x)->"shadow-property"\nvalue=Null(new Dynamic {})\n',['stable','value','Null(new Dynamic {})','stable'])
add('null-authored-method-shadow','stable=7\nfunction Null(x) = "shadow-method"\nvalue=Null(new Dynamic {})\n',['stable','value','Null(new Dynamic {})','stable'])
(O/'cases.json').write_text(json.dumps(cases,indent=2)+'\n')
for n in ['CollectionProbe.java','original-binding.json','launch.py']:shutil.copyfile(P/n,O/n)
s=(P/'run.py').read_text().replace("'plannedApiCalls':32","'plannedApiCalls':156");(O/'run.py').write_text(s)
print({'expressions':sum(len(c['operations'])for c in cases),'plannedApiCalls':4*sum(len(c['operations'])for c in cases)})
