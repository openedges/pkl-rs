"""Focused forbidden parameters inside generator bodies."""
import importlib.util
from pathlib import Path
p=Path(__file__).resolve().parent
spec=importlib.util.spec_from_file_location('prior_capture',p.parent/'stage53-oracle-parameterized-03/capture.py')
m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m);m.OUT=p
m.SOURCES={'for-body.pkl':'value = new Listing { for (n in List(1)) { p -> n } }\n','when-body.pkl':'value = new Listing { when (true) { p -> 1 } }\n','false-when-body.pkl':'value = new Listing { when (false) { p -> 1 } }\n'}
m.EXPRESSIONS={n:['value.length']for n in m.SOURCES}
m.main()
