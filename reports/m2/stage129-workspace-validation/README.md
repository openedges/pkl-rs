# Stage129 workspace validation and Java URI-catch observations

The full local workspace command completes with actual Cargo exit **0**. Independent raw accounting records **286 completed harnesses: 1,786 passed, zero failed and five ignored**. Thirteen nested child passes are excluded. This closes the workspace interruption retained in Stage128. The Pkl engine and production APIs are unchanged; full M2 compatibility and performance remain open.

## Workspace execution

The source HEAD is `76ac2c376aed04a11a3df50e2b5634e6b5cc3be6`, unchanged across execution. The host is macOS 27 arm64, with Rust 1.98.1. The command is `cargo test --workspace --locked --message-format=json`, using one compilation job, two test threads, debug information disabled, test optimization level 1 and the unchanged opt-in executable storage wrapper.

All 284 test executables start and complete, covering 281 integration sources and three unit-test targets. Both doctest crates, `pkl_runtime` and `pkl_wasm`, complete with zero doctests. No expected target is missing or duplicated. The 26 example artifacts are compiled but are not claimed executed. The 212 runtime unit tests and five reflection lifecycle tests are included in the total, not added again.

The five ignored tests remain non-passes: the frontend probe invoked separately by the M1 runner, the const-lambda internal error, List foreign-entry rendering, Set foreign-entry errors and glob internal-error handling. M1 is not rerun in this checkpoint. `all_required_pass` remains false.

Execution uses a separate APFS clone, `stage129-cargo-01`. Its 6,960-entry comparison verifies unchanged source-cache content and metadata, matching clone content and metadata, and distinct regular-file inodes. The original Stage128 cache remains intact. Cargo reports all 310 executable products as `Fresh` (cache reuse), with zero new wrapper receipts; this is not fresh compilation.

The explicit disk floor is 1 GiB, separate from Stage128's 768 MiB experiment. Limits are 3 GiB sampled owned-process RSS, four hours and 512 MiB of combined logs and receipts. The command takes 3,883.817 seconds; minimum sampled free space is 5,145,874,432 bytes and peak sampled owned RSS is 603,701,248 bytes. No guard is triggered, forced teardown is false and no owned process remains. These supervisor observations are not product benchmarks or Java performance comparisons.

## Source and artifact provenance

The runner bookends 6,151 selected inputs and preserves the original 4,319 input aliases. The 1,830 dependency paths first discovered after Stage128 are included before this execution. All 9,956 observed dependency rows reconcile to prebookended inputs; no newly discovered unbookended path remains. Eight build-script depfiles are still unavailable. They differ from the earlier eight omitted probe inputs, now included among the 159 pinned probe dependencies. Complete transitive compiler, runtime and rustdoc provenance remains unestablished.

A separate post-test acquisition hashes exactly 284 test and 26 example executables. All 310 match previously verified signature bytes, allowing reuse of those signature observations; no new signature command runs. The earlier 290 wrapper-backed products and 20 products without original compiler receipts retain their distinction. The latter comprise four test and 16 example products. Clone and signature matches do not supply missing compiler evidence, so `artifact_provenance_complete` remains false. Hashes describe stable per-file reads, not an atomic snapshot of all products.

## Java URI-catch evidence

The unchanged eight Stage128 programs and their 80 input files are evaluated with the pinned Pkl 0.32.0 JAR and Temurin 21.0.12.1+1, with a 512 MiB Java heap. Eighteen commands complete with exit zero: JDK identity, one fresh helper compilation and 16 independent JVM sessions. Each evaluator queries `result` twice, giving 32 queries across two rounds. Both rounds reuse the one freshly compiled helper; they are not independent compiler builds.

The helper's `evaluateExpressionString` outcomes are 24 returned strings (`success`), four ordinary Pkl exceptions (`error`) and four internal Pkl exceptions (`oracle-error`). These are Java observations, not Rust compatibility passes. Complete UTF-8 bytes, original UTF-16 code units, logger fields and ordered callbacks are retained, with source/runtime, snapshot and helper-class bookends.

| Cases | Observed behavior |
| --- | --- |
| Four opaque URI `catch` / `catchOrNull` cases | Return the rendered internal-error string, including release information and Java stack frames |
| Nested catch control | The outer catch reports that no exception was thrown after the inner catch returns a string |
| Ordinary glob catch control | Returns the ordinary invalid-glob diagnostic |
| Successful `catchOrNull` control | Returns the stringified `null` through this helper API |
| Uncaught opaque URI control | Throws `PklBugException`; the first and second query have different complete error bytes |

All eight between-round raw comparisons match. Within-session query fields match in 14 of 16 sessions; both uncaught-control sessions retain different first/second error strings. Repeat equality is an observation, not an evidence-admission condition. The first unexpected-error banner line is removed by upstream `catch` rendering; a literal phrase flag is not a semantic verdict. No expected string is synthesized from Rust, no typed value export is claimed and no Java fallback is added.

The first independent readback rejects the existing oracle-cache symlink and remains a recorded failure. The second completes with a self-pinned reader variant, but its source path is subsequently restored during agent coordination. Its report is preserved. Root reconstructs that exact executed variant in a separate `prep03` directory and runs a final successful readback with its own source bookends. Only the exact pinned oracle JAR may traverse the known cache alias. Alias metadata is bookended during final readback; acquisition-time alias identity was not recorded and is not retroactively established.

## Evidence and remaining work

The checkpoint descriptor is [stage129-workspace-validation-checkpoint.json](../stage129-workspace-validation-checkpoint.json). The evidence archive, publication receipt, independent archive readback and published-copy receipt are stored beside this report. The finite archive contains raw outcomes, acquisition/readback scripts, reviews, selected source and fixture bodies, and explicit historical metadata dependencies. It excludes Cargo binaries, host JDK/JAR bodies and old archive bodies. Stage128's published archive remains a named historical dependency, without a new body read.

Primary receipts inside the archive are:

- `target/m2/stage129-workspace-checks-01/report.json` and its original stdout/stderr.
- `target/m2/stage129-workspace-independent-readback-01/report.json` and qualified accounting.
- `target/m2/stage129-artifact-independent-readback-01/report.json`.
- `target/m2/stage129-uri-catch-java-01/report.json` and raw observations.
- `target/m2/stage129-uri-catch-readback-03/report.json`, retaining attempts 01 and 02 separately.

Stage128's Cargo 101 copy-headroom failure and Cargo -15 disk-floor interruption remain historical outcomes. Their partial counts are not added to this run. The unchanged Stage127 xtask, Clippy, Wasm and bounded M1 results, and Stage128 formatting, manifest and wrapper checks, remain historical evidence; they are not newly executed here. Pure controls for the new acquisition, accounting and publication scripts are recorded separately from Pkl tests.

Next implementation work is Java internal-error rendering and catch boundaries, including evaluator-state-dependent stack output, followed by the remaining language differences. This checkpoint does not rerun the full language/glob inventories, close the five ignored tests, implement product APIs, validate Linux/Windows release execution or establish the speed and memory targets. All nine M2 work areas remain open.
