from pathlib import Path
import json,shutil
O=Path(__file__).resolve().parent;P=O.parent/'stage51-collection-oracle-06';cases=[]
def add(id,source,expressions,reader=None):
 w=O/'workspace'/id;w.mkdir(parents=True,exist_ok=False);(w/'input.pkl').write_text(source);ops=[{'kind':'evaluate','value':e}if isinstance(e,str)else e for e in expressions];(w/'operations.tsv').write_text(''.join(op['kind']+'\t'+op.get('value','').encode().hex()+'\n'for op in ops));rules=[{'uri':u,'kind':k,'value':v}for u,k,v in (reader or [])];(w/'reader-plan.tsv').write_text(''.join(r['uri']+'\t'+r['kind']+'\t'+r['value'].encode().hex()+'\n'for r in rules));cases.append({'id':id,'workspace':str(w),'source':str(w/'input.pkl'),'sourceUri':(w/'input.pkl').as_uri(),'operations':ops,'readerPlan':rules,'scope':'separate Duration foundation Java-first; constructor/type/format/equality/hash/transform/refidentity only, no toUnit/ISO/DataSize/other arithmetic'})
units=['ns','us','ms','s','min','h','d'];source='stable=7\n'+''.join('function dyn_'+u+'(x)=x.'+u+'\n'for u in units);ops=['stable']
for u in units:
 ops += ['List(1.'+u+',1.25.'+u+',dyn_'+u+'(1),dyn_'+u+'(1.25)).toString()',
         'List(1.'+u+'.value,1.25.'+u+'.value,1.'+u+'.unit,1.25.'+u+'.unit).toString()',
         'List(1.'+u+'.value.getClass()==Int,1.25.'+u+'.value.getClass()==Float,1.'+u+'.getClass()==Duration).toString()']
ops+=['stable'];add('duration-seven-units',source,ops)
source='stable=7\nfunction dyn(x)=x.s\n'
values=['9007199254740993.s','9223372036854775807.s','(-9223372036854775808).s','(-1.25).min','(-0.0).s','Infinity.s','(-Infinity).s','NaN.s','1e300.d']
ops=['stable','List('+','.join(values)+').toString()','List('+','.join('('+v+').value'for v in values)+').toString()',
 'List('+','.join('('+v+').value.getClass()==Int'for v in values)+').toString()',
 'List('+','.join('('+v+').isPositive'for v in values)+').toString()',
 'List(dyn(9007199254740993),dyn(9223372036854775807),dyn(-9223372036854775808),dyn(-0.0),dyn(NaN),dyn(Infinity)).toString()',
 'List((-1.s),(-1.25.s),(-(-0.0).s)).toString()','List(1e300.d==Infinity.s,1e300.d.value).toString()','stable'];add('duration-numeric-boundaries',source,ops)
source='''stable=7
typed: Duration=1.s
wrong: Duration=1
function echo(x: Duration): Duration=x
document=new Dynamic { integer=1.s; fractional=1.25.min; negative=(-1.25).h; zero=(-0.0).s; positive=Infinity.s; nan=NaN.s }
r=new PcfRenderer {}
output { value=document }
'''
add('duration-type-render',source,['stable','typed.toString()','wrong','echo(1.25.h).toString()','echo(1)','List(typed is Duration,typed is Any,typed is Number).toString()','typed.getClass()==Duration','r.renderValue(typed)','r.renderDocument(document)','output.text','"duration=\\(typed); fractional=\\(1.25.h)"','Null(typed).toString()','stable'])
source='''stable=7
nan=NaN.s
nanOther=NaN.s
zero=(-0.0).s
positiveZero=0.s
'''
ops=['stable','List(1.s==1000.ms,1.min==60.s,1.h==3600.s,0.1.s==100.ms).toString()','nan==nan','nan==nanOther','NaN.s==NaN.s','zero==0.s','Set(nan,nan,NaN.s,NaN.s).length.toString()',
 'Set(zero,0.s).toString()','Set(zero,0.s).length.toString()',
 'Map(1.min,"first",60.s,"last").keys.toList().map((d)->d.unit).toString()',
 'Map(1.min,"first",60.s,"last").values.toString()',
 'Map(60.s,"first",1.min,"last").keys.toList().map((d)->d.unit).toString()',
 'Map(zero,"negative",0.min,"positive").toString()','Map(zero,"negative",0.min,"positive").length.toString()',
 'Map(nan,1,nan,2,nanOther,3).toString()','Map(nan,1,nan,2,nanOther,3).length.toString()',
 'Set(1e300.d,Infinity.s).toString()','Map(1e300.d,"first",Infinity.s,"last").toString()','stable'];add('duration-equality-hash',source,ops)
source='''stable=7
function work()=List(1,2,3).map((x)->if(trace(x)==2)if(read("ct:/duration-retry")=="recovered")x.s else x.s else x.s).toString()
memo=work()
'''
ops=['stable','List(1,2,3).map((x)->trace(x).s).toString()','Set(1,2).map((x)->if(x==1)1.min else 60.s).toString()',
 'Map(1,"one",2,"two").mapKeys((k,v)->if(k==1)1.min else 60.s).toString()',
 'Map(1,"one",2,"two").map((k,v)->Pair(k.s,v)).toString()','work()','stable','work()','work()','memo','memo',{'kind':'gc'},'work()','stable'];add('duration-transforms-retry',source,ops,[('ct:/duration-retry','io-first','duration-reader-first')])
source='''stable=7
function literal()=1.s
function dyn(x)=x.s
literalValue=literal()
dynamicValue=dyn(1)
rSelf=new PcfRenderer { converters { [Duration]=(value)->value } }
rLiteral=new PcfRenderer { converters { [Duration]=(value)->literal() } }
rOtherLiteral=new PcfRenderer { converters { [Duration]=(value)->1.s } }
rDynamic=new PcfRenderer { converters { [Duration]=(value)->dyn(1) } }
class LiteralHolder { tag: Int; quantity=1.s }
class ComputedHolder { tag: Int; quantity=(tag-tag+1).s }
a=new LiteralHolder { tag=1 }
b=new LiteralHolder { tag=2 }
c=new ComputedHolder { tag=1 }
d=new ComputedHolder { tag=2 }
rB=new PcfRenderer { converters { [Duration]=(value)->b.quantity } }
rD=new PcfRenderer { converters { [Duration]=(value)->d.quantity } }
'''
add('duration-literal-dynamic-identity',source,['stable','List(a.tag,b.tag,c.tag,d.tag).toString()','rSelf.renderDocument(literalValue)','rLiteral.renderDocument(literalValue)','rLiteral.renderDocument(literalValue)','rOtherLiteral.renderDocument(literalValue)','rSelf.renderDocument(dynamicValue)','rDynamic.renderDocument(dynamicValue)','rB.renderDocument(a.quantity)','rD.renderDocument(c.quantity)',{'kind':'gc'},'rLiteral.renderDocument(literalValue)','rDynamic.renderDocument(dynamicValue)','stable'])
source='''stable=7
nullableInt=()->1?.s
nullableIntParen=()->(1)?.s
nullableFloat=()->1.25?.s
nullableFloatParen=()->(1.25)?.s
nullableDynamic=(x)->x?.s
qi=nullableInt.apply()
qip=nullableIntParen.apply()
qf=nullableFloat.apply()
qfp=nullableFloatParen.apply()
qd=nullableDynamic.apply(1)
rI=new PcfRenderer { converters { [Duration]=(value)->nullableInt.apply() } }
rIP=new PcfRenderer { converters { [Duration]=(value)->nullableIntParen.apply() } }
rF=new PcfRenderer { converters { [Duration]=(value)->nullableFloat.apply() } }
rFP=new PcfRenderer { converters { [Duration]=(value)->nullableFloatParen.apply() } }
rD=new PcfRenderer { converters { [Duration]=(value)->nullableDynamic.apply(1) } }
'''
add('duration-nullable-literal-identity',source,['stable','List(qi,qip,qf,qfp,qd).toString()','rI.renderDocument(qi)','rI.renderDocument(qi)','rIP.renderDocument(qip)','rF.renderDocument(qf)','rFP.renderDocument(qfp)','rD.renderDocument(qd)',{'kind':'gc'},'rI.renderDocument(qi)','rD.renderDocument(qd)','stable'])
(O/'cases.json').write_text(json.dumps(cases,indent=2)+'\n')
for n in ['CollectionProbe.java','original-binding.json','launch.py']:shutil.copyfile(P/n,O/n)
planned=4*sum(sum(op['kind']=='evaluate'for op in c['operations'])for c in cases)
s=(P/'run.py').read_text().replace("'plannedApiCalls':228",f"'plannedApiCalls':{planned}")
s=s.replace("up=['pkl-core/src/main/java/org/pkl/core/stdlib/AbstractRenderer.java'","up=['pkl-core/src/main/java/org/pkl/core/runtime/VmDuration.java','pkl-core/src/main/java/org/pkl/core/stdlib/base/DurationNodes.java','pkl-core/src/main/java/org/pkl/core/stdlib/base/IntNodes.java','pkl-core/src/main/java/org/pkl/core/stdlib/base/FloatNodes.java','pkl-core/src/main/java/org/pkl/core/DurationUnit.java','pkl-core/src/main/java/org/pkl/core/util/DurationUtils.java','pkl-core/src/main/java/org/pkl/core/util/MathUtils.java','pkl-core/src/main/java/org/pkl/core/ast/builder/AstBuilder.java','pkl-core/src/main/java/org/pkl/core/stdlib/AbstractRenderer.java'")
(O/'run.py').write_text(s);print(json.dumps({'cases':len(cases),'plannedApiCalls':planned,'originalAdapterReexecuted':False}))
