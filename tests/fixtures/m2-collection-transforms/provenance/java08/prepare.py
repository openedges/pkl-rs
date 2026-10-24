from pathlib import Path
import json,shutil
O=Path(__file__).resolve().parent;P=O.parent/'stage51-collection-oracle-07';cases=[]
def add(id,source,expressions):
 w=O/'workspace'/id;w.mkdir(parents=True,exist_ok=False);(w/'input.pkl').write_text(source);ops=[{'kind':'evaluate','value':e}if isinstance(e,str)else e for e in expressions];(w/'operations.tsv').write_text(''.join(op['kind']+'\t'+op.get('value','').encode().hex()+'\n'for op in ops));(w/'reader-plan.tsv').write_text('');cases.append({'id':id,'workspace':str(w),'source':str(w/'input.pkl'),'sourceUri':(w/'input.pkl').as_uri(),'operations':ops,'readerPlan':[],'scope':'Separate actual Duration.unit String-reference diagnostic and explicit output renderer corrective controls; no preassumed interning or semantic identity result.'})
units=['ns','us','ms','s','min','h','d'];src='stable=7\n'
for u in units:
 src+=f'a_{u}=1.{u}\nb_{u}=2.{u}\nr_{u}=new PcfRenderer {{ converters {{ [String]=(value)->b_{u}.unit }} }}\n'
expr=['stable']
for u in units:expr += [f'r_{u}.renderDocument(a_{u}.unit)',f'r_{u}.renderDocument(a_{u}.unit)']
expr += [{'kind':'gc'}]+[f'r_{u}.renderDocument(a_{u}.unit)'for u in units]+['stable'];add('duration-unit-same-enum-symbol',src,expr)
src='stable=7\n'
for i,u in enumerate(units):
 v=units[(i+1)%len(units)];src+=f'd_{u}=1.{u}\nl_{u}="{u}"\nrLiteral_{u}=new PcfRenderer {{ converters {{ [String]=(value)->l_{u} }} }}\nrDifferent_{u}=new PcfRenderer {{ converters {{ [String]=(value)->1.{v}.unit }} }}\nrToString_{u}=new PcfRenderer {{ converters {{ [String]=(value)->d_{u}.unit.toString() }} }}\n'
expr=['stable']
for u in units:expr += [f'rLiteral_{u}.renderDocument(d_{u}.unit)',f'rDifferent_{u}.renderDocument(d_{u}.unit)',f'rToString_{u}.renderDocument(d_{u}.unit)']
expr+=['stable'];add('duration-unit-literal-different-tostring',src,expr)
src='''stable=7
document=new Dynamic { integer=1.s; fractional=1.25.min; negative=(-1.25).h; zero=(-0.0).s; positive=Infinity.s; nan=NaN.s }
output { value=document; renderer=new PcfRenderer {} }
'''
add('duration-output-explicit-renderer',src,['stable','output.text','output.text',{'kind':'gc'},'output.text','stable'])
(O/'cases.json').write_text(json.dumps(cases,indent=2)+'\n')
for n in ['CollectionProbe.java','original-binding.json','launch.py']:shutil.copyfile(P/n,O/n)
planned=4*sum(sum(op['kind']=='evaluate'for op in c['operations'])for c in cases)
(O/'run.py').write_text((P/'run.py').read_text().replace("'plannedApiCalls':404",f"'plannedApiCalls':{planned}"))
(O/'PLAN.ko.md').write_text('# Java08 추가 관찰\n\nJava07 원문을 유지한다. 실제 enum unit getter의 동일 단위/다른 Duration 객체, source 문자열 literal, 다른 unit, toString의 참조 동작을 PCF 전체 오류 hint로 관찰한다. 문자열 interning이나 heap 배치를 예상값으로 가정하지 않는다. 명시 PCF output.renderer로 이전 resource allowlist 선행 실패와 출력 경로를 분리한다. Java default powerAssertions=false 단일 설정이며 Rust Observe 설정을 Java에 투영하지 않는다. 기존 corpus와 별도 획득한다.\n')
print(json.dumps({'cases':len(cases),'plannedApiCalls':planned}))
