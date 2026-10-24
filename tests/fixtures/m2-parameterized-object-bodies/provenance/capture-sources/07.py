"""Focused nested object-body checks discovered during source review."""
import importlib.util
from pathlib import Path
p=Path(__file__).resolve().parent
spec=importlib.util.spec_from_file_location('prior_capture',p.parent/'stage53-oracle-parameterized-03/capture.py')
m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m)
m.OUT=p
m.SOURCES={'nested.pkl':'''class Box { value: Int = 1 }
local parent = (arg) -> new Mapping {}
local listing = (parent) { arg -> [(new Listing { p: Int -> 1 }).length] = arg }
local mapping = (parent) { arg -> [(new Mapping { p: Int -> ["x"] = 1 }).length] = arg }
local typed = (parent) { arg -> [(new Box { p: Int -> value = 1 }).value] = arg }
local dynamic = (parent) { arg -> [(new Dynamic { p: Int -> value = 1 }).value] = arg }
nestedListing = listing.apply(3)[0]
nestedMapping = mapping.apply(3)[0]
nestedTyped = typed.apply(3)[1]
nestedDynamic = dynamic.apply(3)[1]
'''}
m.EXPRESSIONS={'nested.pkl':['nestedListing','nestedMapping','nestedTyped','nestedDynamic']}
m.main()
