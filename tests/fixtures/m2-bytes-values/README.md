# Stage 60 Java Bytes value authority

This fixture records 300 ordered public `Evaluator.evaluateExpressionString` calls
against the pinned Pkl 0.32.0 Java artifact, twice in fresh JVMs. The module URI is
`file:///stage60/bytes-values.pkl`. These Java observations are an authority for
the listed expressions only; they do not establish Rust compatibility.

`cases.json`, `queries.tsv`, and `module.pkl` are the sealed inputs. The archive
contains the executed Java source probe, Python acquisition runner, pinned
reference source snapshots, input and runtime identities before and after the
calls, process stdout and stderr, and complete raw output for each row. Each row
has a status byte channel and eight strings captured as both UTF-8 bytes and
UTF-16BE code units: expression, result, error, Java exception class and
`toString`, and cause class, message, and `toString`. The offline verifier checks
the archive digest, exact file set, source and input closure, JDK and artifact
identities, ordered process protocol, every channel, and byte-identical repeats.

The cohort covers the eight Bytes properties, empty and all 256 byte values,
signed direct indexing and `getOrNull` versus unsigned `toList`, construction,
concatenation, List conversion and first invalid element, UTF-8/UTF-16/ISO-8859-1
encoding and strict decoding, equality and Set/Map hashing, inherited `Any`
members, and type, arity, member, and `Charset` alias errors. The pinned Java
sessions each yielded 223 successes and 77 ordinary Pkl errors. The original
`IntSeq(0, 255).toList().toBytes()` controls are retained in the rows; Rust
support is a separate requirement. `List(1, -1, 256).toBytes()` and
`List(1, 256, -1).toBytes()` observe the first conversion error; rows containing
`throw("second")` in a `List` argument observe eager List construction first.

The discarded authored attempts are preserved separately under
`target/m2/stage60-bytes-oracle-01` through `-04`: module-local visibility,
invalid NUL string escape, and case-label corrections. The independent review
found no unexpected authored failure in the final 77 error rows. Java wrapper
identity, VM cache/backlink identity, mutable typed-export aliasing, Rust
native/browser/Node raw comparison, full M2 language compatibility, and
performance gates remain unverified.
