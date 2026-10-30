# Stage127 opaque glob URI validation

This bounded checkpoint prevents opaque globs from enumerating an invalid raw URI. It does not complete Java internal-error compatibility or M2. The full workspace check was stopped by its disk-space guard during compilation and remains unverified.

## Implementation

The shared traversal now compiles the glob expression before validating the URI passed to the opaque reader, following the pinned Pkl 0.32.0 order. Absolute patterns use their original URI; relative patterns validate the enclosing scheme's dummy URI. Validation failure prevents a `list-elements` request. Rare diagnostic metadata retains the original UTF-16 input, syntax reason and known UTF-16 index.

Java's complete internal exception rendering, frame fidelity and catch semantics remain unimplemented. These cases return `Unsupported`; changing an incorrect success into an explicit unsupported outcome does not count as byte compatibility.

The platform verification tool now reads native, N-API and Wasm products from `CARGO_TARGET_DIR`. Absolute and repository-relative paths are supported; the unset default remains `target`.

## Recorded local verification

All acquisition runs use macOS arm64, Rust 1.98.1 and the pinned Pkl 0.32.0 oracle. Java uses Temurin 21.0.12.1+1 and a 512 MiB heap. Exact sources, runtime identities, commands, products and raw streams are retained in the [evidence archive](evidence.tar.gz) and [checkpoint descriptor](../stage127-opaque-glob-uri-checkpoint.json). These are local results, not hosted CI results or performance measurements. Source receipts cover their stated files; eight dependencies listed by the Native probe dependency file were absent from the early build bookend. Later identities and source comparisons are retained separately, without asserting complete transitive compiler provenance.

| Check | Recorded result |
| --- | --- |
| Formatting | Exit 0 |
| Opaque unit filter | Five passes, including four new unit tests |
| New opaque Host integration target | Three passes; repeated evaluation, no listing after failure, relative dummy URI and close coverage |
| Native probe build | Exit 0; copied product pinned separately |
| `cargo xtask verify` | Exit 0 |
| Workspace/all-target Clippy with warnings denied | Exit 0 |
| `cargo test --workspace --locked` | Stopped during compilation at the 1 GiB disk floor; actual exit -15, no test execution recorded |
| Wasm runtime check and manifest freshness | Both exit 0 |
| Focused platform-tool Python suite | Eight tests pass; exit 0 |
| Actual M1 native/Node/Chromium acquisition | 6,078 raw matches per target; 92 Node and 92 browser scenarios; separate strict and architecture predicates pass |
| Direct Java URI experiment | 30 input vectors, twice; complete raw repeat equality, including UTF-16 input and cause fields |
| New Pkl regex-before-URI witnesses | Four read/import programs, twice, with two queries per session; complete raw repeat equality |
| Native replay of the new Pkl witnesses | All eight sessions / 16 queries match the eight common raw channels and logs |

The actual workspace failure and its logs are preserved. Cleanup removed only 146 newly compiled, unexecuted test executables from the new Stage127 cache, recovering about 1.56 GB. Earlier evidence, earlier caches, source files and the early-tested artifacts were retained. The supplemental checks do not replace a successful full workspace run.

## Selected native differential results

The 56-session retained cohort contains 128 queries. All 20 previously wrong-success escaped opaque sessions now return `unsupported` for both queries. Their completed and closed Rust typed-event traces contain no `list-elements` request. These 40 queries still differ from Java's `oracle-error` status and complete diagnostics.

Of the 36 control sessions, 32 match Java's common raw channels (80 queries). Four relative-space read/import sessions retain eight mismatching queries. Their affected status and diagnostic bytes match the separately pinned Stage122 native observations. No new mismatch was found in these selected controls. Rust typed-event observations do not establish Java SPI callback equivalence.

The separate eight-session Native replay of the new Pkl witnesses matches all 16 queries against their new Java results. These are ordinary glob errors, establishing the selected error-order behavior. This does not resolve the internal-error cases above or compare Java-only exception-class metadata.

The direct JDK vectors establish `URI.create` behavior only. They do not prove every vector can reach the same stage through Pkl's initial URI admission. The Pkl witnesses are recorded separately.

## Remaining work

- Complete and differentially verify Java internal-error diagnostics, source frames and catch behavior.
- Obtain a successful full workspace run with adequate disk space. The interrupted command remains failed/incomplete.
- Extend the new feature comparisons to the full glob and language inventories and the product boundaries. The Stage122 full-inventory numbers remain historical results; this checkpoint did not rerun all 4,442 ready cases or all 576 glob sessions.
- Close the remaining M2 work areas, full APIs, release operating systems and per-workload speed/memory requirements. The existing requirement and execution classifications are unchanged.

The [plan](../../../PLAN.md) records the next work. Historical native status comparisons depend on the separately preserved [Stage122 evidence](../stage122-lossless-source-runtime/README.md); the selected Java input/output bytes are included with this checkpoint.
