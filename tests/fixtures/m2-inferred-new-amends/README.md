# Pkl 0.32.0 inferred `new` in an amending module

This separate fixture records seven authored public Java 0.32.0 string observations in a module that `amends "base.pkl"`. The sibling `base.pkl` is a real source file beside `module.pkl` in the captured input directory. Two fresh JVMs returned the same raw files and seven successes per session. The Java command and provenance record the actual absolute `file:` URI used for relative import resolution.

The cases observe the amending module's `this` class, typed, absent-return-type, and `unknown` method results, their Dynamic class names, and the lazy list result. `oracle-evidence.tar.gz` preserves all 21 raw channels per row, both Java processes, JDK and JAR identities, and 92 pinned source copies. Run `python3 tools/inferred_new_fixtures.py --profile amends` for offline full-protocol verification.

The source-file relation is part of this authority. A native or portable candidate replay must provide a real equivalent Host/source map and compare its actual raw channels; this fixture alone does not claim that a browser or Node target can resolve the captured file URI. No Rust candidate, complete API, or performance pass is claimed.
