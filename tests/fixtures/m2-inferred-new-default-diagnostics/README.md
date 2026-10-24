# Inferred Listing element defaults

This Pkl 0.32.0 Java authority evaluates four separate source modules. A
method returns a `Listing` whose element is an inferred `new {}` object.
Literal `unknown` and `Rows<unknown>` succeed. `Any` and `U` (where
`typealias U = unknown`) raise an ordinary undefined `default` error. The raw
error frames distinguish the method header, first Listing element, result
property, and expression query.

Each module is loaded by its own file URI in two fresh JVMs. The archive keeps
all result channels, process streams, source copies, and JDK/JAR provenance.
Run `python3 -B tools/inferred_new_default_diagnostics_fixtures.py` for the
offline full-protocol check.
