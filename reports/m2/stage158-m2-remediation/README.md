# Stage 158 M2 Milestone Remediation and Full Serial Verification Checkpoint

This canonical checkpoint report records the complete remediation, root-cause bug fixes, and frozen-source serial verification of Milestone M2 against **Apple Pkl 0.32.0** in `pkl-rs.rev3`. It supersedes earlier preliminary drafts by resolving all integrity audit findings, fixing the remaining raw-byte differential mismatches in the shared runtime (`crates/pkl-runtime`), and completing all frozen-tree verification runs under `-03` output directories.

Evidence in this checkpoint reflects local macOS arm64 execution (`PKL_ALLOW_LOCAL_JDK=1`), which is distinct in provenance from hosted CI runs using the pinned Adoptium `21.0.12.1+1-LTS` JDK lock. Zero mismatches remain across all 4,575 ready M2 cases (`4,573 pass`, `2 oracle-error`), while strict completion gates (`11` unmapped support/duplicate candidates, `2` `-Xmx512m` JVM oracle errors, `920` requirement-state obligations totaling `933` `full_gate_failures`, `18` Protobuf reference nonpasses), hosted lock-JDK parity, and future milestones (M3–M7) remain explicitly open.

## Environment and Provenance

- **Host Architecture**: `macOS-27.2-arm64-arm-64bit-Mach-O` (Darwin arm64)
- **Rust Toolchain**: `rustc 1.98.1` (`48a229cea 2026-09-01`, LLVM `22.1.8`, `rust-toolchain.toml`)
- **JVM Oracle**: Eclipse Adoptium Temurin `21.0.13+9-LTS` (`/Library/Java/JavaVirtualMachines/temurin-21.jdk/Contents/Home/bin/java`) invoked with `PKL_ALLOW_LOCAL_JDK=1`, recording `"exact_jdk_lock_match": false` relative to pinned `oracle.lock.json` (`lock_sha256: 7b77395ff5b64f7a220db74975e4c3858a36b19c5c0f5bca92681e3b1bc6504b`, `executable_sha256: 0f642542ea1f0be6957557c0db96653ec128113bb5a4aaa7c2534d1d72ddb51f`, `runtime_modules_sha256: 6f5a5fecf53be74c142074b0f43ab177e64f50ba17cf4ed8f541ccfd4a39a31d`, `release_sha256: f74f9807460af9aa0f49d48af023e3536c2af90ece81aa336860f0877843ac2b`)
- **Pkl Reference**: Apple Pkl `0.32.0` (`external/pkl` @ `e51fb2f91774265666f94980ec630db794a4db2d`, `pkl-tools-0.32.0.jar` SHA-256 `c3b6f6215a08b31453b0fb0edd8c9da0aee31233f65abb8ab07f9c945151e059`)
- **Python**: `/opt/homebrew/bin/python3.14`
- **Node.js**: `v26.13.0` (`napi` 10, `modules` 147)
- **Browser Automation**: Playwright `1.64.1` (`playwrightPin: 1.64.1`) with pinned Chromium engine

## Remediation & Final Bug-Fix Trajectory

### Earlier Stage 158 Remediation Foundations (Tracks A–C & WS5)
- **Package & Project Purity (Track A)**: Removed global mutable statics (`OnceLock<Mutex>`), `std::fs`, `std::env`, `Command::new`, fake redirects, and fake local ZIP packers from `crates/pkl-runtime/src/packages/{http.rs,cache.rs,project.rs,resolver.rs}` in favor of `PackageHost` / `HttpHandler` traits, keeping `crates/pkl-runtime` free of host filesystem/process/env I/O and compatible with `wasm32-unknown-unknown`.
- **Oracle Honesty & Platform Abstraction (Track B)**: Restored `#[ignore]` on the 5 unresolved oracle integration tests plus the explicit M1 runner hook (`frontend.rs`), injected `PlatformMetadata` and monotonic clock abstractions, implemented `PklBugException` 22-frame projection, and reduced `api/list.pkl` Rust runtime via lazy modulo indexing in `crates/pkl-runtime/src/vm/set_sequence.rs`.
- **Manifest Synchronization & Caching (Track C & WS5)**: Synchronized `475` requirements and `4,586` cases (`4,575` ready, `11` unmapped) in `compatibility/m2-language.json`, and added stat-based snapshot caching in `tools/compat.py`.

### Final Mismatch Elimination (Progression: `-01` -> `-02` -> `-03`)
Across full M2 acquisitions leading to this frozen checkpoint:
- `target/m2/remediation-full-01`: `4,529 pass`, `3 oracle-error`, `24 mismatch`, `16 oracle-contract-mismatch`, `3 candidate-unsupported`.
- `target/m2/stage158-final-02`: `4,568 pass`, `2 oracle-error`, `5 mismatch` (`0 candidate-unsupported`, `0 oracle-contract-mismatch`):
  1. **Four primary-module `@`-dependency glob mismatches** (`package-project-eval`: `project1/globbing.pkl`, `project1/localProjectRead.pkl`, `project1/localProjectRead2.pkl`, `project7/globWildcards.pkl`): When evaluating `read*("@...")` (`Phase::ResolvePattern`) or `import*("@...")` after wildcard splitting (`Phase::ResolveBase`) inside a primary module (`run.declaration == None`), `crates/pkl-runtime/src/vm/glob.rs` skipped `ModuleRequestKind::ResolveImportUri` and treated the `@dep/...` URI as a relative path. Fixed in `crates/pkl-runtime/src/vm/glob.rs` by constructing a `ModuleKey` from `run.module_uri` when `run.declaration` is `None` and `text.starts_with("@")`, routing `@`-prefixed glob patterns and base paths through `ModuleRequestKind::ResolveImportUri` without mutating primary-unit bootstrap policy for `pkl:base`. Added unit regression test `vm::glob::tests::primary_module_read_glob_dependency_requests_import_uri_resolution`.
  2. **One constrained-type eagerness mismatch** (`selected-regression-module-eval`: `m2:rev1:t36-foreign-nominal-identity`): `TypeCheck::Constrained` in `crates/pkl-runtime/src/vm/typecheck.rs` left `run.eager` enabled while checking `*base`, whereas upstream `TypeNode.ConstrainedTypeNode.executeLazily` runs `childNode.executeLazily` before setting `shouldEagerTypecheck(true)` for constraints. Eagerly validating elements before constraint evaluation dropped the `form#Selection.items` (`toList()`) stack frame. Fixed in `crates/pkl-runtime/src/vm/typecheck.rs` by storing `parent_eager` in `Task::AfterBase`, clearing `run.eager = false` across `*base`, and restoring `run.eager = parent_eager` on entry to `Task::AfterBase`. Added unit regression test `vm::typecheck::source_identity_tests::constrained_base_clears_eager_flag_so_nested_constraint_frame_survives`.

## Frozen-Source Serial Verification Outcomes (`-03` Artifacts)

After regenerating and verifying `compatibility/m2-language.json`, all files in `tools/m2_source_inputs.py:source_paths` were frozen and every gate ran serially:

| Gate | Verification Step | Command / Evidence Directory | Exit | Observed Outcome |
| --- | --- | --- | --- | --- |
| **Manifest Freeze** | M2 manifest sync & check | `python3.14 tools/m2_manifest.py && python3.14 tools/m2_manifest.py --check` | `0` | `4,586` cases (`4,575` ready, `11` unmapped), `475` requirements |
| **Gate 1** | Reference pin verification | `cargo xtask verify` | `0` | `Reference pins verified for Pkl 0.32.0.` |
| **Gate 2** | Workspace formatting | `cargo fmt --all -- --check` | `0` | Clean across workspace |
| **Gate 3** | Workspace lint | `cargo clippy --workspace --all-targets --locked -- -D warnings` | `0` | 0 warnings across all targets |
| **Gate 4** | Pure Wasm build | `cargo check -p pkl-runtime --target wasm32-unknown-unknown --locked` | `0` | Clean `wasm32-unknown-unknown` compilation |
| **Gate 5** | Full Rust workspace suite | `CARGO_PROFILE_DEV_OPT_LEVEL=1 ... cargo test --workspace --locked` | `0` | **`2,225 passed; 0 failed; 6 ignored`** (`263/263` `pkl-runtime` unit tests; 6 `#[ignore]` functions preserved cleanly) |
| **Gate 6** | Protobuf strict contract | `PKL_PROTOBUF_CAPTURE_DIR=.../target/m2/stage158-protobuf-strict-final-03 cargo test -p pkl-runtime --test m2_protobuf_java04_full java04_strict_full_compatibility_gate -- --ignored --exact --nocapture` | `101` (expected) | `266/266` executed, `0` harness failures, `240/240` regular, `8/8` pre-renderer, **`18` `reference_nonpasses`**, `experiment_contract_pass: true`, `strict_compatible: false` |
| **Gate 7a** | Inventory & manifest `--check` scripts (6 scripts) | `tools/{inventory,discover,smoke_manifest,regression_manifest,m2_manifest,m2_module_eval}.py --check` | `0` | All 6 checks pass (`11,624` API items, `1,112` discover cases, `889` module-eval rows) |
| **Gate 7b** | Python M2 & oracle unit tests | `PYTHONPATH=tools python3.14 -m unittest tools/tests/test_{m2_language,m2_mappings,m2_execution_profiles,m2_source_inputs,m2_package_project_eval,m2_selected_regressions,oracle}.py -v` | `0` | **`95 passed`** (`89.798s`, `OK`) |
| **Gate 8** | Multi-target platform contract | `python3.14 tools/m1_platforms.py --output target/m1/stage158-platforms-final-03 --verify-contract` | `0` | **`6,078 / 6,078`** match Oracle across `rust`, `node`, `browser`; `shared_engine_passed: 6,078`; `92` Node & `92` Chromium checks; `source_stability: true` |
| **Gate 9** | Full M2 raw-byte capture | `python3.14 tools/m2_language.py --output target/m2/stage158-final-03 --record-only --workers 3` | `0` | **`4,573 pass` / `2 oracle-error` / `0 mismatch`**; **`88/88` metamorphic `pass`**; `source_stability: true`; `artifact_stability: true` |
| **Gate 10a** | Report integrity verifier | `python3.14 tools/m2_language.py --verify-report target/m2/stage158-final-03` | `0` | Verified all raw channels, workspaces, sources, and artifacts |
| **Gate 10b** | Development gate verifier | `python3.14 tools/m2_language.py --verify-development target/m2/stage158-final-03` | `1` (expected) | `error-cycle` development exception preserved; rejects `-Xmx512m` `oracle-error` on `api/list.pkl` |
| **Gate 10c** | Strict full-gate verifier | `python3.14 tools/m2_language.py --verify-full target/m2/stage158-final-03` | `1` (expected) | Preserves **`933` `full_gate_failures`** without false passes |
| **Gate 11** | Separate `-Xmx2g` `api/list.pkl` experiment | `target/m2/stage158-apilist-xmx2g-03/result.json` (Java `-Xmx2g` vs `stage158-final-03/artifacts/module_eval_probe`) | `0` | Both exit `0`, empty stderr, identical `8,411`-byte stdout (`SHA-256: 39e6cd3034f224c420fbe5fdd95f8611952ced8ffd0b14ed8608597ae7d06cf7`, **`diff_bytes: 0`, `match: true`**) |

### Full M2 Per-Adapter Breakdown (`target/m2/stage158-final-03/report.json`)

| Adapter | `pass` | `oracle-error` | `mismatch` | Notes |
| --- | ---: | ---: | ---: | --- |
| `scalar-probe` | 3,585 | 1 | 0 | `oracle-error`: `error-cycle` (`NoClassDefFoundError: VmStackOverflowException` on JVM) |
| `language-module-eval` | 887 | 1 | 0 | `oracle-error`: `api/list.pkl` (`OutOfMemoryError: Java heap space` under `-Xmx512m`; `api/list.pkl` matches byte-for-byte at `-Xmx2g` in Gate 11) |
| `package-project-eval` | 68 | 0 | 0 | Includes previously failing `globbing.pkl`, `localProjectRead.pkl`, `localProjectRead2.pkl`, `globWildcards.pkl` |
| `historical-cli-module-eval` | 21 | 0 | 0 | All `rev1-cli` contracts match raw bytes |
| `historical-collections-module-eval` | 8 | 0 | 0 | — |
| `selected-regression-module-eval` | 3 | 0 | 0 | Includes fixed `m2:rev1:t36-foreign-nominal-identity` |
| `language-jvm-classpath-output` | 1 | 0 | 0 | — |
| **Total selected ready** | **4,573** | **2** | **0** | Plus **88/88** metamorphic `pass` |

## Preserved Limits, Strict Gates, and Unverified Scope

1. **Two Preserved `oracle-error` Cases under Locked Oracle Profile**:
   - `error-cycle`: JVM exits `1` with `NoClassDefFoundError: Could not initialize class org.pkl.core.runtime.VmStackOverflowException` (`88,665` bytes stderr), whereas `language_probe` exits `1` cleanly with `–– Pkl Error ––\ncyclic forcing of a`.
   - `pkl:language-snippet:pkl-core/src/test/files/LanguageSnippetTests/input/api/list.pkl`: Under the locked `-Xmx512m` JVM profile, Java fails with `OutOfMemoryError: Java heap space`, whereas `module_eval_probe` exits `0`. Under the explicit separate `-Xmx2g` experiment (`target/m2/stage158-apilist-xmx2g-03`), both exit `0` and match identically (`diff_bytes: 0`).
2. **Strict `full_gate_failures` (`933` entries)**: `--verify-full` continues to exit `1` cleanly because it enforces whole-milestone promotion criteria: `460` `implementation incomplete` + `460` `test mapping incomplete` across `partial` requirement states + `13` `required case incomplete` (`11` `unmapped` `rev1-candidate` entries — 8 non-executable support libraries in `RESOURCE_ONLY_MAP` and 3 canonical `m2:rev1:t36`/`t40` duplicates — plus the `2` `oracle-error` cases above).
3. **Protobuf Strict Gate & Ignored Sentinel Tests**: `java04_strict_full_compatibility_gate` (`stage158-protobuf-strict-final-03`) retains `18` Java `reference_nonpasses` (`strict_compatible: false`). Six `#[ignore]` test functions remain in `crates/` (`1` M1 runner hook in `frontend.rs` and `5` unresolved oracle/strict protocol tests across `m2_annotation_const_oracle.rs`, `m2_protobuf_java04_full.rs`, `m2_string_apis_oracle.rs`, `m2_list_apis_oracle.rs`, and `m2_set_apis_oracle.rs`).
4. **Local JDK Provenance**: Runs used local Temurin `21.0.13+9-LTS` (`PKL_ALLOW_LOCAL_JDK=1`, `exact_jdk_lock_match: false`) rather than hosted CI runners matching the `21.0.12.1+1-LTS` lock entry in `oracle.lock.json`.
5. **Unverified Scope (M3–M7 & Release Targets)**: No completion is claimed for:
   - Milestone **M3**: Server / external evaluator protocol.
   - Milestone **M4**: Packaging, distribution tooling, and full release platform matrix.
   - Milestone **M5**: Interactive REPL.
   - Milestone **M6**: Language Server Protocol (LSP).
   - Milestone **M7**: Final JVM throughput and memory parity benchmarks.
