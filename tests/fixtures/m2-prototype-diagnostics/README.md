# Pkl 0.32.0 prototype diagnostic controls

This fixture records 39 public Java expression calls in two fresh JVMs using
`file:///stage64/prototype-diagnostics.pkl`. It covers empty and nonempty
`Dynamic`, `Listing`, and `Mapping` values made from class parents, no-member
generators, ordinary constructors, and user classes. It records invalid `|>`
and `+` operands, missing methods, and `getClass().simpleName`. The existing
`m2_runtime.rs` test case `1 |> new Dynamic {}` motivated this independent
authority; Rust results did not supply expected bytes.

The capture has 35 ordinary Pkl errors and four successes per JVM. The archive
preserves full UTF-8 and UTF-16BE row channels, Java exception and cause
metadata, ordered process streams, actual argv and environment, pinned Java
0.32.0 JAR and JDK identities, and copied upstream sources. Empty built-in
objects display the simple names `Dynamic`, `Listing`, and `Mapping` in the
tested diagnostics. User classes display their qualified module names. The
initial target-only 39-row capture is preserved separately; the sealed archive
comes from a fresh canonical capture with the tracked producer.

`tools/prototype_diagnostics_fixtures.py` verifies the archive offline. These
Java observations are a diagnostic reference, not a claim of Rust, Node,
browser, complete M2, typed export, or performance compatibility. The fixture
is separate from the Stage64 188-row amendment string profile and eleven
declaration programs and is not registered as a shared comparison profile.
