# Pkl 0.32.0 amendment declaration Java authority

This fixture keeps eleven distinct module programs under `modules/`. Each program is evaluated at the public `test` expression in two fresh pinned Pkl 0.32.0 JVM sessions, using the declared URI `file:///stage64/amendment-declarations.pkl`. Three calls succeed and eight return ordinary Pkl errors in each repetition. The archive retains exact module source bytes, 21 per-row UTF-8/UTF-16BE and exception channels, complete process streams, input and source snapshots, and JAR/JDK identities.

Separate modules are necessary because an invalid class or property declaration would make a shared module unusable for every query. These programs cover typed annotation/body syntax, amendment of an `Int` property, unfilled abstract property reads, extending external `Int`, repeated class property bodies, `fixed` and `const` modifier requirements, a local property amendment, and later parser errors that precede earlier lazy class errors. Earlier authored attempts remain outside this canonical archive.

These are selected public expression calls, not a full original snippet or CLI evaluation. They establish neither internal VM reference identity nor Rust, Node, browser, full M2, or performance compatibility. All observed outcomes came from Java.
