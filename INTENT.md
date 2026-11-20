# Project Intent

## Purpose

Reimplement Pkl **0.32.0** in Rust so that existing Pkl programs and integrations preserve their observable behavior, while execution time and memory usage meet or improve on the Java implementation. Deliver native Rust, WebAssembly, and Node.js N-API products built on one evaluation engine.

This document defines the project's durable goals and scope. [PLAN.md](PLAN.md) contains the milestone overview, implementation order, and milestone gates in Korean; [TESTING.md](TESTING.md) defines the verification contract. The [architecture](docs/architecture.md) describes the intended design.

The repository currently contains Java baseline verification tooling and a bounded shared Rust evaluator with native, browser, and Node experiments. M2 is expanding language support; the complete evaluator and product interfaces remain unfinished. Selected differential results establish compatibility only for the recorded cases, not the full language or the performance targets below.

## Reference Baseline

The compatibility authority is the Java implementation and sources of Pkl 0.32.0. Use the exact revisions recorded in [references.lock](references.lock) and the submodule gitlinks.

| Reference | Revision | Role |
| --- | --- | --- |
| `external/pkl` | `e51fb2f91774265666f94980ec630db794a4db2d` | Language, standard library, diagnostics, tools, public APIs, and upstream tests |
| `external/pkl-pantry` | `904a68184e54da378b0a0e8a911a3fab3956f693` | Real packages, examples, and integration test inputs |
| `../pkl-rs.rev1` | Local reference material | Prior implementation ideas, test inputs, and regression reproductions |

Pantry pinning establishes the corpus inputs; Java execution outcomes and remaining limitations are recorded in the [M0 evidence report](reports/m0/README.md). Earlier Rust implementations and their passing test records are not evidence that this project is compatible. Reused code and tests require verification against the pinned baseline.

Pin the Java executable artifacts and execution configuration before generating reference results or performance baselines. Later upstream releases do not silently change this project's target.

## Required Outcomes

### Behavioral and Byte Compatibility

For identical inputs and environments, preserve all externally observable behavior covered by Pkl 0.32.0:

- Language evaluation, types, standard library behavior, member order, and lazy evaluation order.
- Raw stdout and stderr, exit codes, generated file paths, and generated file bytes.
- Error text, whitespace, source locations, underlines, call frames, hints, and the first reported error.
- CLI arguments and defaults, REPL state, server protocol, callbacks, and public API behavior.
- Evaluation lifecycle, including cancellation, timeouts, errors, and closure.

Inputs include paths, locale, time zone, terminal configuration, environment variables, resources, and network responses. Control these through the execution environment. Do not erase differences through trimming, sorting, reserialization, or partial error comparisons.

Some protocol fields vary by design, such as randomly allocated evaluator IDs. Codec tests with supplied fixed IDs must compare complete bytes. Actual session tests verify ID issuance and reference consistency alongside result payloads; they do not establish byte equality of independently generated IDs. Such contracts must be explicit and must not mask deterministic mismatches.

### Performance and Memory

Each required workload must meet both targets after correctness has been established:

```text
Rust execution time / Java execution time <= 1.00
Rust peak RSS / Java peak RSS <= 1.00
```

Define and fix workloads, measurement boundaries, environments, and uncertainty handling before assessing results. Include cold startup, sufficiently warmed-up persistent evaluation, large inputs, failure paths, and long-running sessions. Record comparisons with the official native-image CLI separately from JVM results.

Measure the complete applicable product boundary, including Node value conversion and the Host process for Wasm. JVM heap size, Rust allocation counts, and Wasm linear memory help diagnosis but are not interchangeable measures of total memory. Do not conceal a required workload's regression through averages or quietly relax a target for one platform.

### Shared Products

Native Rust, Wasm, and N-API must share language semantics, the value model, diagnostics, and renderers. Platform adapters provide capabilities through a Host interface rather than independent evaluators.

Wasm hosts supply facilities such as virtual filesystems and asynchronous resource loaders. Specify the capabilities needed by each API and test. A missing Host capability remains an explicit limitation or unexecuted requirement; it cannot become a silent skip or a claim of full compatibility. Test real browser and Node execution as well as compilation.

Preserve integers and bytes without loss across API boundaries. Implement the same lifecycle contracts for suspension, resumption, cancellation, timeout, and close across supported targets.

## Product Scope

The initial release target covers the 0.32.0 language, standard library, output, CLI, tooling, and Rust/Wasm/N-API product surface, not only source evaluation.

| Area | Required scope |
| --- | --- |
| Language | Parser, name resolution, lazy evaluation, objects, amendments, inheritance, closures, types, constraints, generics, generators, and reflection |
| Standard library | All pinned `pkl:` modules and external builtins, including 0.32.0 `Reference<Domain, Type>` behavior |
| Output | Every upstream renderer and binary encoding, converters, output ordering, `output.bytes`, and `output.files` |
| Modules and resources | Imports, reads, globs, URI handling, files, HTTP, module paths, and external readers |
| Projects and packages | Project configuration, evaluator settings, dependency resolution, lockfiles, checksums, caches, package creation, and retrieval |
| CLI | All 0.32.0 commands, options, defaults, help text, completion, diagnostics, and exit behavior |
| Interactive and server use | REPL sessions, server protocol, callbacks, request handling, and evaluator lifecycles |
| Developer tools | Formatter, Pkl tests and reporters, and documentation generation (`pkldoc`) |
| Embedding | Native Rust evaluator API, and Wasm and N-API bindings |

**Explicit scope exclusions (2026-11-20):** Java/Kotlin code generation (`pkl-codegen-java`, `pkl-codegen-kotlin`) and a JVM public API compatibility adapter preserving existing Java/Kotlin public APIs are explicitly excluded from the required product scope and release criteria. Upstream `java_api`, `kotlin_api`, and code-generation entries recorded in `compatibility/inventory.json` and `compatibility/corpus.json` remain reference census data only, not required implementation deliverables.

Use inventories derived from the pinned sources to enumerate this scope. In particular, implement 0.32.0 name resolution, multiline string continuation, HTTP custom headers, resolved evaluator settings, external reader resolution, dependency notation, and test reporter behavior. Experimental features present in the chosen baseline remain in scope unless the project intent is explicitly changed.

## Engineering Constraints

- Production evaluation runs in Rust. Java serves as the compatibility oracle and performance reference; it is not a fallback evaluator.
- Share the engine and semantic components across products. Keep platform I/O behind the Host interface.
- Preserve upstream semantics when optimizing. Execution strategy, crate boundaries, and internal representations may evolve based on evidence.
- Treat the bytecode VM as a candidate to validate early, not as a proven route to warmed-up Java performance.
- Design source positions, numerical behavior, Java-compatible regular expressions, error ordering, and cyclic memory reclamation as core compatibility concerns.
- Keep reference sources and fixtures faithful to their pinned contents. Update gitlinks, locks, documentation, and verification criteria together when deliberately changing the baseline.

## Evidence and Completion

Build a systematic suite covering upstream cases, standard library inventories, Pantry packages, prior regression inputs, Java differential execution, protocol and API behavior, target environments, generated cases, and long-running memory behavior. The test tooling must detect missing inputs, expected results, runners, and execution coverage.

Generate reference results from the pinned Java oracle and preserve their provenance. Add minimized reproductions for discovered discrepancies. Report passes, failures, unimplemented cases, tests not run, and environment errors separately. Counts of discovered files and passing setup tests are not feature completion metrics.

A release satisfying this intent requires complete required feature and API inventories, zero known mismatches in that scope, execution of all required checks on target platforms, and conclusive compliance with both performance targets for every required workload. [TESTING.md](TESTING.md) governs how that evidence is collected and evaluated.

An implementation milestone may be complete while the product remains incomplete. Reports must state that distinction. Changes to the version baseline, required surface, platform coverage, or performance targets are explicit changes to project intent, recorded consistently in the plan and verification contract. Temporary limitations do not redefine the release goals.
