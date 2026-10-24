# Stage 59 Java Duration value authority

This fixture records 274 ordered public `Evaluator.evaluateExpressionString` calls
against the pinned Pkl 0.32.0 Java artifact, twice in fresh JVMs. The module URI is
`file:///stage59/duration-values.pkl`. The independent Java result is an authority
for these expressions only; it is not a Rust compatibility result.

`cases.json`, `queries.tsv`, and `module.pkl` are the sealed inputs. The gzip archive
contains the actual Java source probe, its Python acquisition runner and complete
repository source snapshots, pre/post identities, process stdout/stderr, and nine
raw channels per call: expression UTF-8/UTF-16BE, status, result stdout
UTF-8/UTF-16BE, result stderr UTF-8/UTF-16BE, and Java error class
UTF-8/UTF-16BE. `tools/duration_values_fixtures.py` checks the independent
archive digest, every file digest and name, source and input closure, ordered
process protocol, channel consistency, all rows, and byte identical repetitions.

The cohort covers all seven Duration units and their 49 conversion pairs, number
construction, value/unit/ISO properties, arithmetic and truncation, mixed unit
ordering, NaN and signed zero behavior, Set/Map hash thresholds, type and arity
errors, and inherited `Any` behavior. The 274 rows yielded 240 successful calls
and 34 ordinary Pkl errors in each Java session. `Duration.round()` and Pkl
source identity `===` are not reachable Pkl 0.32.0 Duration operations. Public
typed Java `Duration` export, Rust/native/Node/browser comparisons, upstream
suite replay, full M2 compatibility, and performance gates remain open.

The first 263-row exploratory capture remains separately under
`target/m2/stage59-duration-oracle-01`; it was corrected before this canonical
acquisition. Do not replace this archive to add discovered cases. Use a separately
identified supplement and preserve the original Java evidence.
