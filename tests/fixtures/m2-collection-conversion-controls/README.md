# Pkl 0.32.0 converted-object controls

This separate fixture records 58 public Java `evaluateExpressionString` calls against
the pinned Pkl 0.32.0 oracle. Its module URI is
`file:///stage62/collection-conversion-controls.pkl`. Capture 02 contains two fresh,
ordered JVM sessions with byte-identical raw calls, full process output, Java
exception and cause metadata, UTF-8 and UTF-16 code unit channels, executed
inputs, source copies, and JAR and JDK identities. Capture 01 remains preserved
as authored history: its surrogate-key expressions used invalid `\uD800` syntax.
Capture 02 uses Pkl's `\u{d800}` escape.

The rows examine converted List/Set/Map/Typed class names, missing Listing and
Mapping members and defaults, property versus entry lookup on Map-to-Dynamic,
and Dynamic/Map-to-Typed fallback. A default property body referring to
`this.name` observes the target class prototype even when the source contains
`name = "source"`; direct reading of the converted `name` observes the source.
Missing and explicit null source values have separate controls.

The surrogate-key rows retain raw Java UTF-16 units. An unpaired high surrogate
and U+FFFD remain distinct Map keys after conversion to Dynamic; Java UTF-8
encoding of the unpaired surrogate is lossy, so consumers must compare the
UTF-16 channel as well. String Map keys become Dynamic properties, so property
lookup can succeed while string subscript entry lookup fails.

These are public string observations, not Java wrapper reference identity or
typed export checks. They claim no Rust candidate compatibility passes. Original
upstream program replay and full platform, M2, and performance gates remain
separate.

Run `python3 tools/collection_conversion_controls_fixtures.py` for an offline
archive, source, runtime, whole-protocol, and raw-repetition verification.
