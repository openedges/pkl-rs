# Pkl 0.32.0 String property authority

This fixture records 146 independent public Java expression calls at
`file:///stage67/string-properties.pkl`. The first 77 expressions retain the
reviewed `isBlank`/`isNotBlank` matrix. The remaining calls cover type
constraints, `pkl:ref.Reference` property metadata, `isNotEmpty`, `lastIndex`,
and the related `length`/`isEmpty` paths. Two fresh JVM sessions preserve every
raw UTF-8 and UTF-16 channel, full errors, and process output. The observed
status vector is 117 successes and 29 ordinary Pkl errors.

`pkl-whitespace-bmp.bitset.bin` is an 8,192-byte table produced by the
separate direct Java `StringUtils` probe. Bit `n & 7` of byte `n >> 3` records
whether BMP code unit `n` is in Pkl's pinned 25-character `White_Space` table.
The evidence archive includes the direct probe's source, JDK comparison
bitset, two complete JVM sessions, and provenance. The direct bitsets are
producer evidence; they are not additional public Pkl expression rows.

The `api/stringUnicode.pkl` expressions in this fixture are isolated public
controls. This fixture does not claim that the full original program, a Rust
candidate, native/Node/browser targets, or all of M2 pass.
