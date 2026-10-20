# Roadmap

The baseline is Pkl **0.32.0**. Completed work currently covers Git setup, the Rust workspace, reference submodules, development documentation, and bootstrap checks. The Java oracle, Pkl implementation, full compatibility corpus, and performance baseline have not been established yet.

| Phase | Main Deliverables | Completion Criteria |
| --- | --- | --- |
| M0 Baseline | Pinned references and oracle artifacts, API/stdlib/test inventories, Java runner, performance baseline | Reproducible comparison environment, detection of omissions and execution errors, baseline results |
| M1 Architecture validation | Parser, SourceMap, resolver, diagnostics, and regex/number/heap/VM experiments | Key semantic risks identified; shared engine and performance strategy validated |
| M2 Language | Objects, lazy evaluation, inheritance, closures, types, Reference | Byte equivalence for results and errors in the relevant language corpus |
| M3 Standard library and output | All builtins, renderers, reflection, Pkl Binary | Zero missing public members; matching output and errors |
| M4 External capabilities | Module/resource Host, HTTP, projects, packages, external readers | Passing real I/O and package corpora |
| M5 Product APIs | CLI, REPL, server, Rust API, formatter, documentation generation, code generation, JVM adapter | Compatibility with existing calls, clients, and lifecycles |
| M6 Wasm and N-API | Asynchronous Host, adapters, distribution packages | Passing target corpora in actual browsers and Node |
| M7 Release | Full Pantry corpus, platform validation, execution time and memory reports | Zero known mismatches within the required scope; performance and memory criteria met |

Wasm builds and small execution tests in browsers and Node begin in M1. M6 expands these to the full feature set. Performance and memory measurements also begin in M1 and continue alongside implementation.

## Next Work: M0

1. Pin Java 0.32.0 executable artifacts, JDK and Graal/Truffle configuration, and hashes. A pinned source commit alone does not establish an execution baseline.
2. Inventory language features, standard library members, CLI commands and options, protocol, and public APIs, and track their implementation and validation status.
3. Inspect regression inputs from upstream, Pantry, and rev1 to build test manifests with provenance and execution contracts. Distinguish discovered file counts from executed test counts.
4. Implement Java/Rust differential execution tooling that preserves and compares raw stdout, stderr, exit codes, and generated files.
5. Verify that the test tooling detects missing expected results, empty corpora, unavailable runners, and byte differences as failures.
6. Establish execution time and memory baselines, separating complete CLI runs from first and repeated requests to a persistent evaluator.

Currently, `cargo xtask verify` validates reference pins. Add differential execution and benchmark commands to the documentation after they are implemented.

## Decisions for M1

- How to reproduce Java's string, regular expression, and numeric boundary behavior on Native and Wasm.
- Representations for name resolution, lazy objects, type checks, and error order.
- Heap lifecycles, including cyclic objects, and memory reclamation in long-running sessions.
- The performance potential of a bytecode VM and the need for specialization and caching.
- Contracts for suspending and resuming asynchronous Host calls, cancellation, and evaluator shutdown.

Estimate the schedule and scope of parallel work after obtaining the M0 inventories and M1 experiment results. Explicitly identify any exclusion of required features or relaxation of performance criteria as a change to the goals.
