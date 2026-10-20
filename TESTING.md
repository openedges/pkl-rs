# Validation Contract

The goals are behavioral and byte compatibility with the Java implementation of Pkl 0.32.0, with execution time and memory usage at least as good as Java for every required workload. **Only bootstrap checks are currently available.** Java differential tests, Pantry execution, and product benchmarks have not been implemented yet.

## Current Checks

```sh
cargo xtask verify
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo check -p pkl-runtime --target wasm32-unknown-unknown --locked
```

The first command checks reference pins. The remaining commands check Rust formatting, run static analysis and the current workspace tests, and verify Wasm compilation. Successful Wasm compilation does not establish browser evaluation or API compatibility. Do not use the number of passing `cargo test` cases as a measure of Pkl feature completion.

## Compatibility Comparisons

Run the pinned Java 0.32.0 oracle and Rust with identical inputs and environments. Compare:

- Raw stdout and stderr bytes, and exit codes.
- The set of generated files, their paths, and their raw bytes.
- Error messages, whitespace, source locations, underlines, call frames, hints, and error order.
- CLI options and defaults, REPL state, and API results, errors, cancellation, and close behavior.
- Server protocol encoding, callbacks, and session lifecycles.

Fix all inputs that affect results, including working paths, locale, time zone, terminal settings, environment variables, and network responses. Resolve path differences through the execution environment, such as identical mounts. Formal comparisons must not remove differences with `trim`, sorting, JSON reserialization, or arbitrary string substitutions.

Protocol fields that vary between runs, such as Java's random evaluator IDs, need a separate contract. Codec tests supplied with fixed IDs compare all bytes. Tests of actual sessions verify that issued IDs are referenced consistently and validate result payloads; they must not be described as byte equivalence for the entire session.

Generate expected results from the Java oracle and review their provenance and changes. Do not resolve mismatches by overwriting expected results with the current Rust output.

## Planned Test Layers

| Layer | Scope |
| --- | --- |
| Test tooling | Failure detection for empty corpora, missing expected results, unavailable runners, and byte differences |
| Parser and resolver | Syntax, invalid input, source spans, name resolution, and formatter round trips |
| Runtime and types | Lazy forcing, inheritance, closures, cycles, constraints, generics, and the first error |
| Standard library and renderers | Every public member, boundary values, output order, converters, and binary output |
| CLI, REPL, and server | Help and options, PTY sessions, state retention, protocol, timeouts, and close behavior |
| I/O and packages | HTTP fixtures, redirects, proxies, CAs, root restrictions, checksums, caches, and lockfiles |
| Pantry | Development trees, packaged modules, real examples, and generated files at the pinned revision |
| Platforms | Native, Wasm, and N-API results, errors, and Host capabilities |
| Generation and transformation | Grammar-based generation, differential fuzzing, property tests, and metamorphic tests |
| Long-running execution | Repeated successes and failures, growing sets of unique sources, cancellation, evaluator creation and shutdown, and memory reclamation |

Each test manifest records the case ID, source repository, revision and path, input and resource hashes, execution arguments and environment, required Host capabilities, expected success or error, channels and files to validate, and execution targets. Pinning a Pantry commit alone does not establish 0.32.0 compatibility, so first verify the selected cases and execution conditions with the Java oracle.

Distinguish passes, failures, tests not run, unimplemented cases, and environment errors. Do not count skips or known failures as passes. Add minimal reproductions of failures to the required regression cases, and compare the test inventory with actual execution results to detect omissions.

## Performance and Memory

Only runs that produce correct output are eligible for measurement. Every required workload must satisfy both conditions:

```text
Rust execution time / Java execution time ≤ 1.00
Rust peak RSS / Java peak RSS ≤ 1.00
```

Fix the Java executable artifacts, JDK and Graal/Truffle configuration, hardware, OS, and measurement method. Compare against a sufficiently warmed-up JVM and record results for the official native-image CLI separately. Do not judge the overall performance target solely by cold JVM startup time.

| Usage | Measurement Boundary |
| --- | --- |
| CLI | From process startup through completed output and process exit |
| Persistent evaluator | Initialization, first request, and repeated requests measured separately |
| REPL and server | Session creation, request latency, long-term memory usage, and shutdown |
| N-API | Includes JS↔Rust value conversion and result delivery |
| Wasm | Download, compilation, and initialization measured separately from requests to a persistent instance |
| Packages and large inputs | Cold and warm caches, import graphs, large outputs, and failure paths |

Record latency distributions and peak RSS across repeated runs for each workload. Report results as inconclusive when measurement uncertainty prevents a judgment. Do not use average improvements to conceal regressions in individual required workloads.

For Wasm and N-API, first define measurement boundaries that include the Host process. JVM heap usage, Rust allocations, and Wasm linear memory are diagnostic metrics; do not compare these different metrics directly to claim equivalent memory usage.

## Release Criteria

The required feature and API inventory must have no omissions, and there must be zero known mismatches within the required scope on target platforms. Completion cannot be declared while required tests remain unrun or unimplemented, or performance and memory measurements remain inconclusive. Do not document complete compatibility or guarantee performance equivalent to Java before these criteria are met.
