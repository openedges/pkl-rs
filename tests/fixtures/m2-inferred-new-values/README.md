# Pkl 0.32.0 inferred `new` Java value evidence

This fixture records 72 authored public `evaluateExpressionString` calls against the pinned Java 0.32.0 oracle. Two fresh JVMs returned the same raw files: 49 successes and 23 ordinary Pkl errors per session. The canonical module URI is `file:///stage65/inferred-new-values.pkl`.

The cases cover inferred parents in module, class, and object methods; typed properties; `if`, `let`, `trace`, and lambda boundaries; assigned collection entries versus bare entry amendments; custom `default(key)` and first errors; and explicit nullable, union, alias, generic, constrained, external, and abstract types. `cases.json`, `module.pkl`, and `queries.tsv` are the authored inputs. `oracle-evidence.tar.gz` contains every raw Java result and process channel, JDK and JAR identities, two session commands, and 92 pinned source copies. Run `python3 tools/inferred_new_fixtures.py --profile values` to verify the full archive and protocol offline.

This is Java authority for these string observations. It does not establish Rust candidate compatibility, direct Java object identity, the original CLI program results, complete API behavior, or performance targets.
