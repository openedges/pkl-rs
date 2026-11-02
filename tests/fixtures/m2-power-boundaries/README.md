# Power assertion visibility oracle fixtures

These fixtures retain complete Java Pkl 0.32.0 expression-string observations
for 17 generator, lambda, member-predicate and object-method cases. Each case
runs with power assertions enabled and disabled. The regression compares all
34 statuses and complete stdout/stderr text as UTF-8 bytes and UTF-16 code units.
Java exception class names are retained as provenance, not mapped to Rust classes.

`module.pkl`, `uri.txt`, `module-name.txt` and `expression.txt` define each input.
The authored URI is `file:///stage131-power/<case>.pkl`; the expression is
`output.text`. Each `on`/`off` directory contains the status and four text fields,
with big-endian UTF-16 companions named `*.code-units.bin`. No output is trimmed,
reserialized or generated from Rust. `cases.txt` fixes the complete case roster.

The authoritative acquisition is `target/m2/stage131-power-oracle-05` using
`tools/M2PowerAssertionOracle.java`, Temurin 21.0.12.1+1 and a 512 MiB JVM heap
on macOS arm64. The oracle JAR SHA-256 is
`c3b6f6215a08b31453b0fb0edd8c9da0aee31233f65abb8ab07f9c945151e059`.
The actual copied inputs, source inputs, helper, selected runtime files and
compiled helper have stable before/after identities. This does not establish
complete runtime dependency closure. Java05 reproduces all 306 raw Java04 fields;
Java04's missing acquisition bookends for its copied inputs remain a historical
evidence limitation. Earlier attempts and failed candidate comparisons are
preserved separately.

Run the regression with:

```sh
cargo test -p pkl-runtime --locked --test m2_power_boundaries_oracle
```

The UTF-8 string adapter reconstructs UTF-16 for comparison. These valid-scalar
inputs do not establish unpaired-surrogate transport. This expression API suite
is separate from original upstream CLI executions and from full language,
public API, platform and performance completion.
