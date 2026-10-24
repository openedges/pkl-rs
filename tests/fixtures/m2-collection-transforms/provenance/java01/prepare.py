from pathlib import Path
import json,hashlib,shutil
O=Path(__file__).resolve().parent;R=O.parents[2]
cases=[]
def add(id,source,exprs,reader=None,scope='collection-transform'):
 p=O/'workspace'/id;p.mkdir(parents=True,exist_ok=False);(p/'input.pkl').write_text(source)
 ops=[{'kind':'evaluate','value':e} if isinstance(e,str) else e for e in exprs]
 (p/'operations.tsv').write_text(''.join(x['kind']+'\t'+x.get('value','').encode().hex()+'\n' for x in ops))
 rp=[{'uri':u,'kind':k,'value':v} for u,k,v in (reader or [])]
 (p/'reader-plan.tsv').write_text(''.join(x['uri']+'\t'+x['kind']+'\t'+x['value'].encode().hex()+'\n' for x in rp))
 cases.append({'id':id,'workspace':str(p),'source':str(p/'input.pkl'),'sourceUri':(p/'input.pkl').as_uri(),'operations':ops,'readerPlan':rp,'scope':scope})
original=R/'tests/fixtures/m2/annotation-stage35-builtin-source-code-union-and-string.pkl'
add('original-source-code',original.read_text(),['result','result',{'kind':'gc'},'result'],scope='unchanged-whole-original-source-with-result-expression')
assert (O/'workspace/original-source-code/input.pkl').read_bytes()==original.read_bytes()
base='stable = 7\n'
add('list-core',base,['stable','List().map((x)->throw("unreachable")).toString()','List(3,1,2).map((x)->trace(x*10)).toString()','List(1,2).map((x)->List(x,x+1)).toString()','List(4,5,6).mapIndexed((i,x)->trace(Pair(i,x))).toString()','List().mapIndexed((i,x)->throw("unreachable")).toString()','List(1,2,3).mapNonNull((x)->if(x==2)null else trace(x)).toString()','List(1,2).mapNonNull((x)->Null(new Dynamic { marker=throw("default-not-forced") })).toString()','List(4,5,6).mapNonNullIndexed((i,x)->if(i==1)null else trace(Pair(i,x))).toString()','List().mapNonNullIndexed((i,x)->throw("unreachable")).toString()','List(1,2).map((x)->null).toString()','List(1,2).mapNonNull((x)->null).toString()','List("é","😀","\\u{d800}").map((x)->x).toString()','stable'])
add('set-core',base,['stable','Set().map((x)->throw("unreachable")).toString()','Set(3,1,2).map((x)->trace(x*10)).toString()','Set(3,1,2).map((x)->trace(x%2)).toString()','Set(4,5,6).mapIndexed((i,x)->trace(Pair(i,x))).toString()','Set(4,5,6).mapNonNullIndexed((i,x)->if(i==1)null else trace(Pair(i,x))).toString()','Set(1,2,3).mapNonNull((x)->if(x==2)null else x%2).toString()','Set(1,2).mapNonNull((x)->Null(new Dynamic { marker=throw("default-not-forced") })).toString()','Set(1,2).map((x)->null).toString()','Set(1,2).mapNonNull((x)->null).toString()','Set(1,2).map((x)->if(x==1)1 else 1.0).toString()','Set("Aa","BB","AaAa","BBBB").map((x)->trace(x)).toString()','stable'])
add('map-core',base,['stable','Map().map((k,v)->throw("unreachable")).toString()','Map().mapKeys((k,v)->throw("unreachable")).toString()','Map().mapValues((k,v)->throw("unreachable")).toString()','Map("c",3,"a",1,"b",2).map((k,v)->trace(Pair(k,v*10))).toString()','Map("c",3,"a",1,"b",2).mapKeys((k,v)->trace(k+v.toString())).toString()','Map("c",3,"a",1,"b",2).mapValues((k,v)->trace(k+v.toString())).toString()','Map("a",1,"b",2,"c",3).map((k,v)->Pair("same",trace(v))).toString()','Map("a",1,"b",2,"c",3).mapKeys((k,v)->trace("same")).toString()','Map("a",1,"b",2).mapValues((k,v)->null).toString()','Map("a",1,"b",2).map((k,v)->Pair(v,k)).toString()','Map("Aa",1,"BB",2).mapKeys((k,v)->trace(k)).toString()','stable'])
for coll in ['List','Set','Map']:
 exprs=['stable'];methods=['map','mapIndexed','mapNonNull','mapNonNullIndexed'] if coll!='Map' else ['map','mapKeys','mapValues']
 for method in methods:
  for input in ([f'{coll}()',f'{coll}(1)'] if coll!='Map' else ['Map()','Map("a",1)']):
   for callback in ['42','()->1','(x)->x','(x,y)->x','(x,y,z)->x']:
    exprs.append(f'{input}.{method}({callback}).toString()')
 exprs+=['stable'];add(coll.lower()+'-callback-guard',base,exprs)
source=base+'function bad(x): Int = "wrong-return"\nfunction typed(x: String): String = x\nfunction badPair(k,v): Pair = 42\n'
add('callback-types-first-error',source,['stable','List().map(typed).toString()','List(1,2).map(typed).toString()','List(1,2).map(bad).toString()','Map("a",1).map(badPair).toString()','Map("a",1).map((k: Int,v)->Pair(k,v)).toString()',*[(f'List(1,2,3).map((x)->if(trace(x)=={n})throw("at-{n}") else x).toString()') for n in [1,2,3]],'List(1,2,3).map((x)->if(trace(x)==2)throw("again") else x).toString()','Map(1,10,2,20,3,30).map((k,v)->if(trace(k)==2)42 else Pair(k,v)).toString()','Map(1,10).map((k,v)->new Dynamic { first=k; second=v }).toString()','Map(1,10).map((k,v)->null).toString()','stable'])
source=base+'lazyList = List(1,2).map((x)->new Dynamic { value = read("ct:/lazy-\\(x)") })\nlazyMap = Map("a",1,"b",2).mapValues((k,v)->new Dynamic { value=read("ct:/map-\\(k)") })\n'
add('lazy-list-map-values',source,['stable','lazyList.length','lazyMap.length','lazyList[0].value','lazyList.length','lazyList[1].value','lazyMap["a"].value','lazyMap.length','lazyMap["b"].value','stable'],[(f'ct:/{k}','text',k) for k in ['lazy-1','lazy-2','map-a','map-b']])
for coll,call in [('list','List(1,2,3).map((x)->if(trace(x)==2)read("ct:/retry") else read("ct:/cached-\\(x)"))'),('set','Set(1,2,3).map((x)->if(trace(x)==2)read("ct:/retry") else read("ct:/cached-\\(x)"))'),('map','Map(1,10,2,20,3,30).map((x,v)->Pair(x,if(trace(x)==2)read("ct:/retry") else read("ct:/cached-\\(x)")))')]:
 source=base+'function work() = '+call+'.toString()\nmemo = work()\n'
 add(coll+'-retry-lifecycle',source,['stable','work()','stable','work()','work()','memo','memo',{'kind':'gc'},'work()',{'kind':'close'},'stable',{'kind':'close'}],[('ct:/retry','io-first','stage51-reader-first'),('ct:/cached-1','text','one'),('ct:/cached-3','text','three')],scope='persistent-reader-retry-then-close-public-API-boundary')
source=base+'import "pkl:test"\n'
add('catch-boundaries',source,['stable','List(1,2,3).map((x)->test.catchOrNull(()->if(trace(x)==2)throw("inner") else x)).toString()','test.catch(()->List(1,2,3).map((x)->if(trace(x)==2)throw("outer") else x)).toString()','Set(1,2,3).mapNonNull((x)->test.catchOrNull(()->if(trace(x)==2)throw("inner") else x)).toString()','test.catch(()->Map(1,10,2,20,3,30).map((k,v)->if(trace(k)==2)42 else Pair(k,v))).toString()','stable'])
source=base+'class Key { id: Int; hidden tag: Int; marker = trace(id+100) }\nfunction key(wantedId: Int, wantedTag: Int): Key = if(trace(wantedTag)>0)new Key { id=wantedId; tag=wantedTag } else throw("tag")\nclass FailKey { id: Int; marker = if(trace(id)==2)throw("hash-two") else id }\nfunction failKey(x): FailKey = if(trace(x)>0)new FailKey { id=x } else throw("x")\nsetKeys=Set(1,2,3).map((x)->key(x%2,x))\nmapKeys=Map(1,10,2,20,3,30).mapKeys((k,v)->key(k%2,k))\n'
add('hash-insertion-representatives',source,['stable','setKeys.length','setKeys.toList().map((k)->k.tag).toString()','mapKeys.length','mapKeys.keys.toList().map((k)->k.tag).toString()','mapKeys.values.toString()','Set(1,2,3).map((x)->failKey(x)).length','Set(1,2,3).map((x)->failKey(x)).length','Map(1,10,2,20,3,30).mapKeys((k,v)->failKey(k)).length','Map(1,10,2,20,3,30).map((k,v)->Pair(failKey(k),v)).length','List(1,2,3).map((x)->failKey(x)).length','stable'])
# Distinct key objects with hidden tags, equal visible zero id: inspect representative without defining expectations.
source=base+'class Zero { id=0; hidden tag: Int }\nfunction zero(x): Zero = if(trace(x)>0)new Zero { tag=x } else throw("x")\n'
add('hash-zero-collision-rehash',source,['stable','Set(1,2,3).map((x)->zero(x)).toList().map((k)->k.tag).toString()','Map(1,10,2,20,3,30).mapKeys((k,v)->zero(k)).keys.toList().map((k)->k.tag).toString()','Map(1,10,2,20,3,30).mapKeys((k,v)->zero(k)).values.toString()','List('+','.join(str(i)for i in range(1,41))+').map((x)->trace(x)).toSet().length.toString()','Set('+','.join(str(i)for i in range(1,41))+').map((x)->trace(x%17)).toString()','stable'])
# All four variants on empty indexed/non-null and nonempty typed argument constraints are already separate guard rows.
(O/'cases.json').write_text(json.dumps(cases,ensure_ascii=False,indent=2)+'\n')
(O/'original-binding.json').write_text(json.dumps({'path':str(original),'sha256':hashlib.sha256(original.read_bytes()).hexdigest(),'copiedPath':str(O/'workspace/original-source-code/input.pkl'),'unchanged':True,'originalAdapter':'tools/java/M2LanguageProbe.java','originalPowerAssertions':False},indent=2)+'\n')
print(json.dumps({'cases':len(cases),'expressionsPerFourSessions':sum(sum(op['kind']=='evaluate' for op in c['operations']) for c in cases),'plannedApiCalls':4*sum(sum(op['kind']=='evaluate' for op in c['operations']) for c in cases)}))
