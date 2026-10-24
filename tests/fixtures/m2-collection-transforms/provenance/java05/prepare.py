from pathlib import Path
import json,shutil,hashlib
O=Path(__file__).resolve().parent;P=O.parent/'stage51-collection-oracle-04';w=O/'workspace/null-amendment-parenthesized';w.mkdir(parents=True,exist_ok=False)
source='''stable=7
defaultObject = Null(new Dynamic { a=1 })
amended = (defaultObject) { a=2 }
defaultFunction = Null(()->new Dynamic { a=3 })
amendedFunction = (defaultFunction) { a=4 }
defaultFunction1 = Null((x)->new Dynamic { a=x })
amendedFunction1 = (defaultFunction1) { a=5 }
unforced = Null(new Dynamic { boom=throw("default-not-forced") })
amendedBoom = (unforced) { a=1 }
functionUnforced = Null(()->throw("function-default-not-forced"))
'''
exprs=['stable','defaultObject.toString()','amended.a.toString()','defaultFunction.toString()','amendedFunction.toString()','amendedFunction.a','amendedFunction.apply().a.toString()','amendedFunction1.apply(9).a.toString()','amendedFunction1.apply().a','unforced.toString()','amendedBoom.a.toString()','List(1,2).mapNonNull((x)->unforced).toString()','Set(1,2).mapNonNull((x)->functionUnforced).toString()','stable']
(w/'input.pkl').write_text(source);(w/'operations.tsv').write_text(''.join('evaluate\t'+e.encode().hex()+'\n'for e in exprs));(w/'reader-plan.tsv').write_text('');case={'id':'null-amendment-parenthesized','workspace':str(w),'source':str(w/'input.pkl'),'sourceUri':(w/'input.pkl').as_uri(),'operations':[{'kind':'evaluate','value':e}for e in exprs],'readerPlan':[],'scope':'corrected separate Null object/function amendment and default-not-forced semantics;04parser36 preserved'}
(O/'cases.json').write_text(json.dumps([case],indent=2)+'\n')
for n in ['CollectionProbe.java','original-binding.json','launch.py']:shutil.copyfile(P/n,O/n)
s=(P/'run.py').read_text().replace("'plannedApiCalls':156","'plannedApiCalls':56");(O/'run.py').write_text(s)
(O/'predecessor.json').write_text(json.dumps({'root':str(P),'reportSha256':hashlib.sha256((P/'report.json').read_bytes()).hexdigest(),'preservedParserIntentFailures':36,'original01to03Unchanged':True,'correction':'Wrap amendment parents in parentheses; separately inspect amended Function value versus invoking it.'},indent=2)+'\n')
