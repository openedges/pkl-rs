from pathlib import Path
import json,shutil,hashlib
O=Path(__file__).resolve().parent;R=O.parents[2];P=Path('/Users/cody.ij.hwang/Projects/pkl-rs.stage50/target/m2/stage51-collection-oracle-05');cases=[]
def add(id,source,exprs,scope):
 w=O/'workspace'/id;w.mkdir(parents=True,exist_ok=False);(w/'input.pkl').write_text(source);ops=[{'kind':'evaluate','value':e}if isinstance(e,str)else e for e in exprs];(w/'operations.tsv').write_text(''.join(op['kind']+'\t'+op.get('value','').encode().hex()+'\n'for op in ops));(w/'reader-plan.tsv').write_text('');cases.append({'id':id,'workspace':str(w),'source':str(w/'input.pkl'),'sourceUri':(w/'input.pkl').as_uri(),'operations':ops,'readerPlan':[],'scope':scope})
minimal=R/'target/m2/stage51-collection-c-review-02/empty-conversion-observer.pkl'
add('canonical-empty-source-observer',minimal.read_text(),['result','result',{'kind':'gc'},'result'],'C source-only observer input copied byte-identically, now actual evaluator boundary')
assert (O/'workspace/canonical-empty-source-observer/input.pkl').read_bytes()==minimal.read_bytes()
def matrix(id,kind,origEmpty,origOne,transforms):
 source='stable=7\nempty='+origEmpty+'\none='+origOne+'\n'
 for name,body in transforms:
  source+='r_'+name+' = new PcfRenderer { converters { ['+kind+'] = (value) -> '+body+' } }\n'
 expressions=['stable']
 expressions += ['r_'+name+'.renderDocument(empty)'for name,_ in transforms]
 for name in ['self','receiver','literalOne','mapSameOne']:
  if name in dict(transforms):expressions.append('r_'+name+'.renderDocument(one)')
 expressions += [expressions[1],{'kind':'gc'},expressions[1],'stable']
 add(id,source,expressions,'Empty canonical reference identity is observed only through actual public conversion-hint diagnostic; no heap-layout requirement. Nonempty same-reference versus same-content distinct controls.')
matrix('empty-list-reference-matrix','List','List()','List(1)',[
 ('self','value'),('receiver','value.toList()'),('literal','List()'),('fromSet','Set().toList()'),('fromMap','Map().values'),
 ('mapEmpty','List().map((x)->throw("unreachable"))'),('mapIndexedEmpty','List().mapIndexed((i,x)->throw("unreachable"))'),
 ('mapNonNullEmpty','List().mapNonNull((x)->throw("unreachable"))'),('dropAll','List(1,2).mapNonNull((x)->if(trace(x)>0)null else x)'),
 ('dropAllIndexed','List(1,2).mapNonNullIndexed((i,x)->if(trace(i)>=0)null else x)'),('literalOne','List(1)'),('mapSameOne','value.map((x)->x)')])
matrix('empty-set-reference-matrix','Set','Set()','Set(1)',[
 ('self','value'),('receiver','value.toSet()'),('literal','Set()'),('fromList','List().toSet()'),('fromMap','Map().keys'),
 ('mapEmpty','Set().map((x)->throw("unreachable"))'),('mapIndexedEmpty','Set().mapIndexed((i,x)->throw("unreachable"))'),
 ('mapNonNullEmpty','Set().mapNonNull((x)->throw("unreachable"))'),('dropAll','Set(1,2).mapNonNull((x)->if(trace(x)>0)null else x)'),
 ('dropAllIndexed','Set(1,2).mapNonNullIndexed((i,x)->if(trace(i)>=0)null else x)'),('literalOne','Set(1)'),('mapSameOne','value.map((x)->x)')])
matrix('empty-map-reference-matrix','Map','Map()','Map("a",1)',[
 ('self','value'),('literal','Map()'),('mapEmpty','Map().map((k,v)->throw("unreachable"))'),('keysEmpty','Map().mapKeys((k,v)->throw("unreachable"))'),
 ('valuesEmpty','Map().mapValues((k,v)->throw("unreachable"))'),('literalOne','Map("a",1)'),('mapSameOne','value.map((k,v)->Pair(k,v))')])
(O/'cases.json').write_text(json.dumps(cases,indent=2)+'\n')
for name in ['CollectionProbe.java','original-binding.json','launch.py']:shutil.copyfile(P/name,O/name)
planned=4*sum(sum(op['kind']=='evaluate'for op in case['operations'])for case in cases)
s=(P/'run.py').read_text().replace("'plannedApiCalls':56",f"'plannedApiCalls':{planned}")
s=s.replace("up=['stdlib/base.pkl'","up=['pkl-core/src/main/java/org/pkl/core/stdlib/AbstractRenderer.java','pkl-core/src/main/java/org/pkl/core/stdlib/base/PcfRenderer.java','pkl-core/src/main/java/org/pkl/core/runtime/VmCollection.java','stdlib/base.pkl'")
(O/'run.py').write_text(s)
(O/'source-observer-binding.json').write_text(json.dumps({'path':str(minimal),'sha256':hashlib.sha256(minimal.read_bytes()).hexdigest(),'copiedSource':str(O/'workspace/canonical-empty-source-observer/input.pkl'),'newJavaExpectedNotAssumed':True,'installed1272FixtureUnchanged':True},indent=2)+'\n')
print(json.dumps({'cases':len(cases),'plannedApiCalls':planned,'originalAdapterReexecuted':False}))
