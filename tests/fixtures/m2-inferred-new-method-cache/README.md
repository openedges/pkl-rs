# Inferred method parent cache authority

This Java 0.32.0 fixture captures inherited `new {}` parent inference at two
compiled method sites: class methods and local object methods. Each canonical
profile evaluates eleven expressions in both receiver orders, twice in fresh
JVMs. The module, nullable module, and alias outcomes expose when Java caches
the first inferred parent. The object-method profile uses module and nullable
module results and verifies the returned marker separately.

The archive also preserves an exploratory local object-method alias program.
Its eight alias calls per repetition throw `PklBugException` caused by
`VmUtils.getFrame` receiving a null owner. Those are Java oracle errors, not
ordinary Pkl errors or compatibility passes. The archive's canonical profiles
exclude that authored shape while the verifier requires its exact raw history.

Run `python3 -B tools/inferred_new_method_cache_fixtures.py` to verify the
archive, every raw row channel, both JVM repetitions, source copies, and the
known oracle-error history offline. The raw output paths include the absolute
file URI used for each multi-module evaluation.
