# Pkl 0.32.0 collection lookup authority

This fixture records 133 ordered public Java `evaluateExpressionString` calls for
seven Stage62 lookup methods: Dynamic `hasProperty`, `getProperty`, and
`getPropertyOrNull`; Listing `getOrNull` and `getOrDefault`; and Mapping
`getOrNull` and `getOrDefault`. The canonical module URI is
`file:///stage62/collection-lookups.pkl`.

Capture 03 preserves two fresh JVM sessions with byte-identical raw results,
whole process output, Java exception and cause metadata, UTF-8 and UTF-16 code
unit channels, authored inputs, pinned source copies, and JAR and JDK identities.
It includes empty and present values, missing and explicit null values, inherited
and overridden members, hidden and local members, generic type behavior, custom
default callbacks, lazy failures, parameter type and arity errors, object keys,
and an unpaired UTF-16 property name. Captures 01 and 02 remain separate authored
history. Capture 01 had four inline object expressions that referenced a module
property through the wrong receiver; capture 02 corrected those and preceded
four external-method and hidden-default controls added in capture 03.

Java's Dynamic `hasProperty(name)` checks a cached **String entry key** before
checking a property Identifier. A string entry can therefore make `hasProperty`
change from false to true after the entry is read or the object is forced, while
`getPropertyOrNull` still returns null and `getProperty` still reports a missing
property. The fixture keeps before/after observations in the same ordered JVM
session and also in a single `let` expression. The hidden built-in `default`
property is reported by `hasProperty`, whereas external `length()` is a method
and is not reported as a property.

The rows establish public string values and diagnostics only. They do not assert
Java wrapper identity or typed export behavior and claim no Rust candidate
passes. Original upstream program replay, browser/Node, full M2, and performance
gates remain separate.

Run `python3 tools/collection_lookups_fixtures.py` for the offline archive,
source, runtime, complete-protocol, and raw-repetition check.
