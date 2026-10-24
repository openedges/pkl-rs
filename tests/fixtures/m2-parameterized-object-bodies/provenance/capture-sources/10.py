"""Alias/default-wrapper supplement; durable cohort admission remains separate."""
import importlib.util
from pathlib import Path
p=Path(__file__).resolve().parent
spec=importlib.util.spec_from_file_location('prior_capture',p.parent/'stage53-oracle-parameterized-03/capture.py')
m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m);m.OUT=p
m.SOURCES={'wrapper-alias.pkl':'''typealias Maybe<T> = Mixin<T>?
typealias Pick<T> = *Mixin<T> | String
typealias Checked<T> = Mixin<T>(false)
local maybe = new Maybe<Dynamic> { arg -> value = arg.value + 1 }
local pick = new Pick<Dynamic> { arg -> value = arg.value + 2 }
local checked = new Checked<Dynamic> { arg -> value = 1 }
maybeValue = maybe.apply(new Dynamic { value = 4 }).value
pickValue = pick.apply(new Dynamic { value = 4 }).value
checkedError = checked
'''}
m.EXPRESSIONS={'wrapper-alias.pkl':['maybeValue','pickValue','checkedError']}
m.main()
