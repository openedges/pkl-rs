# Stage 59 separate Java exported Duration bit observations

This supplement records 15 public `Evaluator.evaluateExpression` calls in each
of two fresh JVMs against pinned Pkl 0.32.0. Every returned value is a Java
`org.pkl.core.Duration`; the probe writes its `doubleToRawLongBits(getValue())`
hex digits and unit symbol without converting through rendered text. Both
sessions produced 15 successful calls and byte identical raw files.

The supplement covers integer to double widening, signed zero, subnormal
underflow, same unit conversion including huge finite overflow, NaN, and
fractional `StrictMath.pow`. Its raw archive contains the Java producer, Python
acquisition runner, complete input/source snapshots and pre/post hashes,
per-call UTF-8/UTF-16BE channels, process status/protocol, and JDK/artifact
identity. `tools/duration_values_bits_fixtures.py` checks this evidence offline.

These are Java public export observations used to guide Rust value semantics.
They do not establish Rust typed export, Node/Wasm transport, whole-language
compatibility, or performance. The canonical 274-row string fixture remains
unchanged in `tests/fixtures/m2-duration-values/`.
