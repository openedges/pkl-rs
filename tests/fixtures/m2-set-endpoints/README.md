# Pkl 0.32.0 Set and List endpoint Java authority

This fixture records 70 ordered public `evaluateExpressionString` calls at `file:///stage63/set-endpoints.pkl`. Two fresh Java 0.32.0 JVM sessions each produced 47 values and 23 ordinary Pkl errors. The raw record retains UTF-8 and UTF-16BE values, full diagnostics, exception and cause metadata, and complete process streams. The offline verifier checks the entire archive, ordered input and output protocol, 37 pinned source copies, JAR/JDK identity, and raw byte equality between sessions.

The cases cover `Set.first`, `last`, `firstOrNull`, and `lastOrNull` on empty, singleton, duplicate and ordered sets; Set/List cross-kind union endpoints; explicit null; Duration representative units; function values; nested lazy objects; Set construction errors; property-as-method lookup order; and List endpoint crosschecks. A throwing element that fails while building a Set is a constructor control, not an endpoint evaluation failure. Equal observed values do not establish Java wrapper identity.

These are Java string observations. The earlier 58- and 70-row explorations remain separate target evidence. This fixture makes no Rust candidate, typed export, native, Node, browser, full M2, or performance pass claim.
