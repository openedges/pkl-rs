"""Stage53 Java-first observations using the existing pinned oracle/probe."""
import concurrent.futures
import hashlib
import json
import re
from pathlib import Path
import shutil
import sys

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / 'tools'))
import compat
import m2_language
import oracle

OUT = Path(__file__).resolve().parent
def put(path, data):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(data, indent=2) + '\n')
def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

SOURCES = {
'basics.pkl': '''class Box { value: Int = 3 }
typealias Alias<T> = Mixin<T>
typealias FunctionAlias<T> = (T) -> T
local seed = new Dynamic { value = 2 }
local explicit = new Mixin<Dynamic> { arg: Dynamic, -> value = arg.value + 10; captured = arg.value }
local inferred: Mixin<Dynamic> = new { arg -> value = arg.value + 20 }
local alias: Alias<Dynamic> = new { arg -> value = arg.value + 30 }
local functionTyped: (Dynamic,) -> Dynamic = new Mixin { arg: Dynamic, -> value = arg.value + 40 }
local identity: Mixin<Box>
local typed = new Mixin<Box> { arg -> value = arg.value + 1 }
local underscore = new Mixin { _ -> value = 9 }
local shadow = new Mixin { arg -> arg = 99; value = arg }
local chained = (explicit) { arg -> capturedAgain = arg.value }
explicitValue = explicit(seed).value
inferredValue = inferred(seed).value
aliasValue = alias(seed).value
functionTypedValue = functionTyped(seed).value
defaultValue = identity(new Box {}).value
typedValue = typed(new Box {}).value
underscoreValue = underscore(seed).value
shadowValue = shadow(seed).value
pipelineValue = seed |> explicit |> inferred
chainedValue = chained(seed).capturedAgain
repeatedValue = explicit(seed).value + explicit(new Dynamic { value = 7 }).value
badArgument = typed(seed).value
badCallArity = explicit().value
badAlias = new FunctionAlias<Dynamic> { arg -> value = 1 }
''',
'arity.pkl': '''local p0 = () -> new Dynamic { value = 1 }
local p1 = (a) -> new Dynamic { value = a }
local p2 = (a, b) -> new Dynamic { value = a + b }
local p3 = (a, b, c) -> new Dynamic { value = a + b + c }
local a0 = (p0) { value = 8 }
local a1 = (p1) { a: Int -> value = a + 10 }
local a2 = (p2) { a: Int, b: Int, -> value = a * b }
local a3 = (p3) { a, b, c -> value = a * b * c }
local ignored = (p2) { value = 5 }
local wrong0 = (p0) { a -> value = a }
local wrong1 = (p1) { a, b -> value = a }
local wrong2 = (p2) { a: Missing -> value = a }
local unresolved = (p1) { a: Missing -> value = a }
local empty = (p2) { a: Missing -> }
local duplicate = (p2) { a, a -> value = a }
local discards = (p2) { _, _ -> value = 4 }
zero = a0().value
one = a1(2).value
two = a2(3, 4).value
three = a3(2, 3, 4).value
noNames = ignored(2, 3).value
wrongZero = wrong0
wrongOne = wrong1
wrongCountBeforeMissingType = wrong2
missingType = unresolved
emptyIgnoresTypeAndCount = empty(2, 3).value
duplicateName = duplicate(2, 3).value
repeatedDiscard = discards(2, 3).value
wrongTypedArgument = a1("bad").value
''',
'curried.pkl': '''local parent = (first) -> (second) -> new Dynamic { value = first + second }
local amended = (parent) { captured: Int -> first = captured; value = super.value + captured }
local tripleParent = (a) -> (b) -> (c) -> new Dynamic { value = a + b + c }
local triple = (tripleParent) { original -> captured = original }
local scalarParent = (a) -> a
local scalarAmended = (scalarParent) { a -> value = a }
local nullParent = (a) -> Null(new Dynamic { value = a })
local nullAmended = (nullParent) { a -> extra = a + 1 }
local nullFunctionParent = Null(parent)
local nullFunctionAmended = (nullFunctionParent) { original -> first = original }
firstCapture = amended(2)(100).first
curriedValue = amended(2)(100).value
tripleCapture = triple(2)(100)(1000).captured
tripleValue = triple(2)(100)(1000).value
nullObjectDefault = nullAmended(4).extra
nullFunctionDefault = nullFunctionAmended(3)(90).first
scalarReturnError = scalarAmended(7).value
wrongSecondArity = amended(2)(3, 4)
repeatedCurried = amended(2)(100).first + amended(9)(200).first
''',
'dispatch.pkl': '''class Box { value: Int = 3 }
direct = new Dynamic { unused: Missing -> value = 8 }
directUnbound = new Dynamic { arg -> value = arg }
typed = new Box { arg: Int -> value = 8 }
listing = new Listing { arg: Int -> 8 }
mapping = new Mapping { arg: Int -> ["x"] = 8 }
emptyTyped = new Box { arg: Missing -> }
emptyListing = new Listing { arg: Missing -> }
emptyMapping = new Mapping { arg: Missing -> }
generatorDynamic = new Dynamic { arg: Int -> when (true) { value = 8 } }
generatorUntyped = new Dynamic { arg -> when (true) { value = 8 } }
''',
'generators.pkl': '''local listMixin = new Mixin<Listing<Int>> { original ->
  original.length
  for (n in List(3, 4)) { original.length + n }
}
local mapMixin = new Mixin<Mapping<String, Int>> { original ->
  ["base"] = original.length
  for (key in List("a", "b")) { [key] = original.length + 10 }
}
local dynamicMixin = new Mixin { original ->
  when (original.flag) { chosen = original.value }
  ...new Dynamic { spread = original.value + 1 }
}
local entryMixin = new Mixin { original -> [original.key] = original.value }
local shadowMixin = new Mixin<Listing<Int>> { original ->
  for (original in List(3, 4)) { original }
}
local listResult = listMixin(new Listing { 10; 20 })
local mapResult = mapMixin(new Mapping { ["z"] = 1 })
listFirst = listResult[2]
listLoop = listResult[4]
mapEntry = mapResult["a"]
dynamicWhen = dynamicMixin(new Dynamic { flag = true; value = 6 }).chosen
dynamicSpread = dynamicMixin(new Dynamic { flag = true; value = 6 }).spread
entryCapture = entryMixin(new Dynamic { key = "answer"; value = 42 })["answer"]
loopShadow = shadowMixin(new Listing { 1; 2 })[3]
''',
'types-order.pkl': '''typealias Positive = Int(isPositive)
local parent = (arg) -> new Dynamic { value = arg }
local checked = (parent) { arg: Positive -> captured = arg }
local throws = (arg) -> throw("parent called")
local typedBeforeParent = (throws) { arg: Int -> value = arg }
local mappingParent = (m) -> new Dynamic { raw = m["x"] }
local mappingAmended = (mappingParent) { m: Mapping<String, Int> -> checked = m["x"] }
local mappingResult = mappingAmended(new Mapping { ["x"] = "oops" })
local functionParent = (f) -> new Dynamic { raw = f("x") }
local functionAmended = (functionParent) { f: (Int) -> Int -> checked = f("x") }
local functionResult = functionAmended((v) -> v)
constraintSuccess = checked(4).captured
constraintFailure = checked(-1).captured
typeBeforeParentFailure = typedBeforeParent("bad").value
parentFailure = typedBeforeParent(1).value
rawMapping = mappingResult.raw
checkedMapping = mappingResult.checked
rawFunction = functionResult.raw
checkedFunction = functionResult.checked
''',
'lexical.pkl': '''local outer = 40
local parent = (arg) -> new Dynamic { value = arg }
local amended = (parent) { arg -> value = super.value + outer; captured = arg; self = this.value }
local owner = new Dynamic {
  value = 100
  local parent = (arg) -> new Dynamic { value = this.value + arg }
  local patch = (parent) { arg -> result = super.value + arg; own = this.value }
  result = patch(3)
}
superValue = amended(2).value
selfValue = amended(2).self
lexicalCapture = amended(2).captured
parentReceiver = owner.result.result
bodyReceiver = owner.result.own
''',
}
EXPRESSIONS = {
'basics.pkl': ['explicitValue','inferredValue','aliasValue','functionTypedValue','defaultValue','typedValue','underscoreValue','shadowValue','pipelineValue.value','chainedValue','repeatedValue','badArgument','badCallArity','badAlias'],
'arity.pkl': ['zero','one','two','three','noNames','wrongZero','wrongOne','wrongCountBeforeMissingType','missingType','emptyIgnoresTypeAndCount','duplicateName','repeatedDiscard','wrongTypedArgument'],
'curried.pkl': ['firstCapture','curriedValue','tripleCapture','tripleValue','nullObjectDefault','nullFunctionDefault','scalarReturnError','wrongSecondArity','repeatedCurried'],
'dispatch.pkl': ['direct.value','directUnbound.value','typed.value','listing[0]','mapping["x"]','emptyTyped.value','emptyListing.length','emptyMapping.length','generatorDynamic.value','generatorUntyped.value'],
'generators.pkl': ['listFirst','listLoop','mapEntry','dynamicWhen','dynamicSpread','entryCapture','loopShadow'],
'types-order.pkl': ['constraintSuccess','constraintFailure','typeBeforeParentFailure','parentFailure','rawMapping','checkedMapping','rawFunction','checkedFunction'],
'lexical.pkl': ['superValue','selfValue','lexicalCapture','parentReceiver','bodyReceiver'],
}

SOURCES = {'lexical.pkl': SOURCES['lexical.pkl'].replace('outer', 'outside').replace('(arg) -> new Dynamic { value = this.value + arg }', '(arg) -> let (captured = this.value) new Dynamic { value = captured + arg }'), 'read-constraint.pkl': r'''typealias Gate = Int(read("gate.txt").text == "go\n")
typealias ClosedGate = Int(read("gate.txt").text == "stop")
local parent = (arg) -> new Dynamic { value = arg }
local patch = (parent) { arg: Gate -> captured = arg }
local closed = (parent) { arg: ClosedGate -> captured = arg }
local lazy = new Mixin { arg -> value = read("gate.txt").text + arg.toString() }
readConstraintSuccess = patch.apply(7).captured
readConstraintFailure = closed.apply(7).captured
readInLazyBody = lazy.apply(8).value
'''}
EXPRESSIONS = {'lexical.pkl': ['parentReceiver','bodyReceiver'], 'read-constraint.pkl': ['readConstraintSuccess','readConstraintFailure','readInLazyBody']}

def main():
    if (OUT / 'report.json').exists():
        raise ValueError('fresh output directory required')
    meta = oracle.ensure_oracle(ROOT, fetch=False)
    put(OUT / 'oracle.json', meta)
    classes = OUT / 'artifacts/java'
    classes.mkdir(parents=True)
    javac = str(Path(meta['java']).with_name('javac'))
    compiled = m2_language.capture([javac, '-encoding', 'UTF-8', '-cp', meta['artifact'], '-d', str(classes), str(ROOT / 'tools/java/M2LanguageProbe.java')], OUT, 'javac', meta['environment'])
    if compiled['exit_code'] != 0:
        raise ValueError('javac failed')
    (OUT / "inputs").mkdir(exist_ok=True)
    (OUT / "inputs/gate.txt").write_bytes(b"go\n")
    cases = []
    for name, source in SOURCES.items():
        names = re.findall(r"local (\w+)", source)
        for variable in names:
            source = re.sub(r"\b" + re.escape(variable) + r"\(", variable + ".apply(", source)
        source = source.replace("local identity: Mixin<Box>\n", "local identity = new Mixin<Box> {}\n")
        source = source.replace(")(", ").apply(")
        source = re.sub(r"\bf\(", "f.apply(", source)
        SOURCES[name] = source
    for name, source in SOURCES.items():
        path = OUT / 'inputs' / name
        path.parent.mkdir(exist_ok=True)
        path.write_text(source)
        for i, expression in enumerate(EXPRESSIONS[name]):
            cases.append(dict(id=name[:-4] + '-' + str(i).zfill(2), source=str(path), source_sha256=sha(path), expression=expression))
    put(OUT / 'cases.json', cases)
    sources = ['tools/java/M2LanguageProbe.java','tools/m2_language.py','tools/oracle.py','tools/compat.py','oracle.lock.json','references.lock', 'external/pkl/pkl-core/src/main/java/org/pkl/core/ast/expression/literal/AmendFunctionNode.java', 'external/pkl/pkl-core/src/main/java/org/pkl/core/ast/expression/literal/ObjectLiteralNode.java','external/pkl/pkl-core/src/main/java/org/pkl/core/ast/builder/AstBuilder.java']
    inputs = {str(ROOT / name): sha(ROOT / name) for name in sources}
    for name in sources:
        target = OUT / 'sources' / name
        target.parent.mkdir(parents=True,exist_ok=True)
        shutil.copyfile(ROOT / name,target)
    before = compat.snapshot(OUT / 'inputs')
    def acquire(case):
        results=[]
        for repeat in range(2):
            identifier=case['id']+'/repeat-'+str(repeat)
            directory=OUT/'cases'/identifier
            directory.mkdir(parents=True)
            process=m2_language.capture_java(meta, OUT/'artifacts/language_probe',identifier,Path(case['source']),case['expression'],directory)
            outcome=(directory/'java.outcome.bin').read_bytes()
            results.append(dict(repeat=repeat, directory=str(directory.relative_to(OUT)), process=process, outcome=outcome.decode()))
        replay=all((OUT/results[0]['directory']/('java.'+channel)).read_bytes()==(OUT/results[1]['directory']/('java.'+channel)).read_bytes()for channel in ['stdout.bin','stderr.bin','outcome.bin']) and results[0]['process']['exit_code']==results[1]['process']['exit_code']
        print(case['id'],case['expression'],results[0]['process']['exit_code'],results[0]['outcome'].rstrip(),flush=True)
        return dict(**case, observations=results,replay_equal=replay)
    with concurrent.futures.ThreadPoolExecutor(max_workers=4) as pool:
        results=list(pool.map(acquire,cases))
    after=compat.snapshot(OUT/'inputs')
    put(OUT/'report.json',dict(schema=1,scope='Java-only independent expression observations. No Rust execution or compatibility pass.',baseline='Pkl 0.32.0',cwd=str(ROOT),environment=meta['environment'],oracle_sha256=sha(OUT/'oracle.json'),tool_sources=inputs,input_files_before=before,input_files_after=after,input_files_unchanged=before==after,observations=results))
    put(OUT/'inventory.json',{'files':{str(p.relative_to(OUT)):{'sha256':sha(p),'bytes':p.stat().st_size}for p in sorted(OUT.rglob('*'))if p.is_file()and p.name!='inventory.json'}})

if __name__ == '__main__':
    main()
