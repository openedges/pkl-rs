"""One corrected lazy-read intent; previous scalar amendment failure remains raw."""
import importlib.util
from pathlib import Path
p=Path(__file__).resolve().parent
spec=importlib.util.spec_from_file_location('prior_capture',p.parent/'stage53-oracle-parameterized-05/capture.py')
m=importlib.util.module_from_spec(spec)
spec.loader.exec_module(m)
m.OUT=p
m.SOURCES={'read-constraint.pkl':m.SOURCES['read-constraint.pkl'].replace('arg.toString()', 'arg.value.toString()').replace('lazy.apply(8)', 'lazy.apply(new Dynamic { value = 8 })')}
m.EXPRESSIONS={'read-constraint.pkl':['readInLazyBody']}
m.main()
