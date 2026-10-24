# Pkl 0.32.0 collection conversion authority

This fixture records 236 ordered public `evaluateExpressionString` calls against the
pinned Java 0.32.0 oracle. The canonical module URI is
`file:///stage62/collection-conversions.pkl`. Each call was made in each of two
fresh JVM sessions. The archive preserves both full raw sessions, Java exception
and cause metadata, UTF-8 and UTF-16 code unit channels, whole process output,
executed inputs, source copies, and actual JAR and JDK identities.

The string boundary covers Typed and module `toMap`, Dynamic `length`, `toMap`,
and `toList`, Mapping `toMap`, Listing `toList` and `toSet`, Map `toMap`,
List and Set selector `toMap`, List and Set `toListing` and `toDynamic`,
Map `toMapping` and `toDynamic`, Typed `toDynamic`, Dynamic and Map `toTyped`,
Set `add`, and Bytes generator and spread iteration. It includes empty and
inherited objects, visible and hidden properties, key and element ordering,
duplicate keys, generic type checks, callback and arity errors, shallow forcing,
lazy nested values, and missing versus explicit null values.

Java observations include a distinction between Bytes iteration forms:
`for (b in Bytes(128, 255))` produces `-128, -1`, while spread produces
`128, 255`. Dynamic conversion shallow-forces visible members before it filters
their kinds. A child member error can therefore precede a parent error, and a
discarded element can make `toMap` fail. An overridden element index remains an
element for filtering while carrying its overridden value.

The published string rows establish value and diagnostic behavior. They do not
establish Java wrapper reference identity, typed export behavior, native/browser/
Node compatibility, full original-program success, the full M2 gate, or any
performance target. Superseded authored captures 01–03 remain in separate raw
history; capture 01 had inaccessible local module helpers, and capture 03 had an
invalid `hidden` modifier on a Dynamic member. The canonical capture is 04.

Run `python3 tools/collection_conversions_fixtures.py` for the offline
provenance and raw-protocol check. The verifier does not invoke Java or Rust.
