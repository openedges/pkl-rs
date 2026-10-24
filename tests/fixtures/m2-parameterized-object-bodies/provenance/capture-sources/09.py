"""Explicit alias construction supplement; frozen ordinary selection is unchanged."""
import importlib.util
from pathlib import Path
p=Path(__file__).resolve().parent
spec=importlib.util.spec_from_file_location('prior_capture',p.parent/'stage53-oracle-parameterized-03/capture.py')
m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m);m.OUT=p
m.SOURCES={'explicit-alias.pkl':'''typealias Alias<T> = Mixin<T>
typealias Chained<T> = Alias<T>
typealias Structural<T> = (T) -> T
local direct = new Alias<Dynamic> { arg -> value = arg.value + 1 }
local chained = new Chained<Dynamic> { arg -> value = arg.value + 2 }
local structural = new Structural<Dynamic> { arg -> value = 1 }
directValue = direct.apply(new Dynamic { value = 4 }).value
chainedValue = chained.apply(new Dynamic { value = 4 }).value
structuralError = structural
'''}
m.EXPRESSIONS={'explicit-alias.pkl':['directValue','chainedValue','structuralError']}
m.main()
