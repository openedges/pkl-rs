"""A user typealias spelling Mixin must not acquire the builtin identity default."""
import importlib.util
from pathlib import Path
p=Path(__file__).resolve().parent
spec=importlib.util.spec_from_file_location('prior_capture',p.parent/'stage53-oracle-parameterized-03/capture.py')
m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m);m.OUT=p
m.SOURCES={'shadow-mixin.pkl':'typealias Mixin<T> = (T) -> T\nresult = new Mixin<Dynamic> { arg -> value = 1 }\n'}
m.EXPRESSIONS={'shadow-mixin.pkl':['result']};m.main()
