# Stage 159 M2 Compatibility Remediation & Genuine Verification Checkpoint (Iteration 4)

This canonical checkpoint report records the Iteration 4 remediation and genuine verification status against **Apple Pkl 0.32.0** in `pkl-rs.rev3`. It retracts the invalid `target/m2/stage159-final` closure claim, `tools/assemble_stage159.py`, baseline `id()` digest memoization, and Protobuf fixture/strict-gate bypasses from earlier drafts, and replaces them with genuine runtime implementations, audited requirement mappings, and unmasked local verification outcomes under `AGENTS.md`.

## Summary of Iteration 4 Remediations & Retractions

1. **Retraction of Invalid `stage159-final` Artifacts & `id()` Memoization**:
   - Removed `tools/assemble_stage159.py`, `target/m2/stage159-final`, and `target/m2/stage159-final-jvm-classpath-workspace`.
   - Replaced the five `target/m2/stage158-final-03-*-workspace` symlinks with physical workspace directories so `target/m2/` contains zero dangling symlinks.
   - Replaced `_BASELINE_DIGEST_CACHE` / `_cached_baseline_digest` (`id()`-keyed memoization in `tools/m2_module_eval.py`) with explicit precomputed `baseline_digest=None` passed from `tools/m2_language.py:check_report()`, alongside hoisting `historical_baseline` and `historical_cli_baseline` out of the per-case loop. Added regression test `test_verify_case_with_distinct_reallocated_baselines_never_aliases_digest` in `tools/tests/test_m2_module_eval.py`.

2. **Generalized Runtime Exception Projection & Dynamic Release Banner (`crates/pkl-runtime`)**:
   - Eliminated hardcoded fixture error strings, fixed carets, and the hardcoded `"Pkl 0.32.0 (macOS 27.0, Java 21.0.12.1)"` banner from `crates/pkl-runtime/src/java_exception/provenance.rs`.
   - Implemented generalized `ForeignOrigin` frame projection (`project_origin_diagnostic` / `project_pkl_bug_with_profile`), dynamic `GlobCharacterClassBounds { index, length }` (`vm/string_glob.rs`), dynamic `KeyValueHolderEqualitySpecialization { detail }` (`vm/equality.rs`), native stdlib `ForeignActivation` attachments (`vm/string_api.rs` `#String.isGlobPattern`, `vm/reference.rs` `#Any.toString`), and task-stack `CollectionKind::Set` lookup (`diagnostic_values.rs`).
   - Added `PlatformMetadata` propagation via `SourceProbe::set_platform_metadata`, `HostSession`, and `SourceSession`. `crates/pkl-runtime/examples/support/scalar_probe.rs` detects host OS/architecture and reads `PKL_ORACLE_JAVA_VERSION` (with fallback probing of `PKL_ORACLE_JAVA`), which `tools/m2_language.py` passes from the oracle's `jdk.properties['java.version']` without mutating `metadata['environment']`.
   - The four static-fixture sentinel suites (`m2_string_apis_oracle`, `m2_list_apis_oracle`, `m2_set_apis_oracle`, `m2_protobuf_java04_full`) explicitly configure `PlatformMetadata { os_name: "macOS", os_version: "27.0", cpu_architecture: "aarch64", java_version: "21.0.12.1" }` matching their pinned fixture captures.

3. **Protobuf Fixture & Strict Gate Restoration**:
   - Reverted `tests/fixtures/m2-protobuf-java04-full/observations.tsv` cleanly to `HEAD` (preserving the 18 documented reference nonpasses).
   - Removed the `uncaught-java-process-failure` exclusion from `crates/pkl-runtime/tests/m2_protobuf_java04_full.rs`, restored `assert_eq!((regular, pre_renderer, nonpasses, seen.len()), (240, 8, 18, 266))` and `totals.completed_reference()`, and restored `#[ignore = "explicit strict completion gate; 18 reference nonpasses remain unmet"]` on `java04_strict_full_compatibility_gate` per `TESTING.md`.

4. **Semantic Audit of `compatibility/m2-mappings.json`**:
   - Re-audited all 475 requirements (`198 specification`, `137 language_parser`, `109 language_token`, `31 semantic-family`): removed all 14 empty `""` titles and `:line<N>` artifacts using section titles from `external/pkl/docs/modules/language-reference/pages/index.adoc`, removed all 286 bulk `basic/as.pkl..constModifier.pkl` bucket mappings and fixed 6-tuples, and mapped requirements to genuine `crates/pkl-runtime/src/` symbols and content-relevant `.pkl` cases covering all 4,575 ready cases.
   - Regenerated `compatibility/m2-language.json` via `tools/m2_manifest.py` and verified `tools/m2_manifest.py --check` (Exit 0) and `tools/tests/test_m2_mappings.py` (8/8 passed).

5. **Strict `AGENTS.md` Gate Restoration & Honest `error-cycle` Accounting**:
   - Removed `case_satisfies_gate` from `tools/m2_manifest.py` so `full_gate_failures()` strictly requires `status == 'pass'` for every ready case and requirement.
   - Restored `tools/m2_language.py` CLI exit logic so `--verify-full` returns exit code 1 whenever `full_gate_failures(report)` is non-empty.
   - `error-cycle` passes `--verify-development` (Exit 0), while `--verify-full` honestly exits 1 due to `error-cycle` being `oracle-error` under cold Java 0.32.0 (`NoClassDefFoundError: VmStackOverflowException`, empty `java.outcome.bin`).

---

## Environment and Provenance

- **Host Architecture**: `macOS-27.2-arm64-arm-64bit-Mach-O` (Darwin arm64, `os.version: 27.2`)
- **Rust Toolchain**: `rustc 1.98.1` (`48a229cea 2026-09-01`, LLVM `22.1.8`, `rust-toolchain.toml`)
- **JVM Oracle**: Eclipse Adoptium Temurin `21.0.13+9-LTS` (`java.version: 21.0.13`, `/Library/Java/JavaVirtualMachines/temurin-21.jdk/Contents/Home/bin/java`) invoked with `PKL_ALLOW_LOCAL_JDK=1`, recording `"exact_jdk_lock_match": false` relative to pinned `oracle.lock.json`
- **Pkl Reference**: Apple Pkl `0.32.0` (`external/pkl` @ `e51fb2f91774265666f94980ec630db794a4db2d`, `pkl-tools-0.32.0.jar` SHA-256 `c3b6f6215a08b31453b0fb0edd8c9da0aee31233f65abb8ab07f9c945151e059`)
- **Python**: `/opt/homebrew/bin/python3.14`
- **Node.js / Browser**: `tools/m1_platforms.py` (`rust`, `node`, `browser` targets)

---

## Verification Outcomes (Iteration 4 Final Freeze)

| Verification Target | Command | Result | Notes |
| --- | --- | --- | --- |
| **Reference Pins** | `cargo xtask verify` | `Exit 0` | `Reference pins verified for Pkl 0.32.0.` |
| **Formatting** | `cargo fmt --all -- --check` | `Exit 0` | Clean across workspace |
| **Clippy Lints** | `cargo clippy --workspace --all-targets --locked -- -D warnings` | `Exit 0` | 0 warnings across workspace |
| **Wasm Target Check** | `cargo check -p pkl-runtime --target wasm32-unknown-unknown --locked` | `Exit 0` | Clean pure-Wasm compilation |
| **Sentinel Suites (5)** | `cargo test -p pkl-runtime --test m2_annotation_const_oracle --test m2_list_apis_oracle --test m2_set_apis_oracle --test m2_string_apis_oracle --test m2_protobuf_java04_full --locked` | `Exit 0` | **94 passed; 0 failed; 1 ignored** (`38 + 14 + 17 + 20 + 5`, `java04_strict_full_compatibility_gate` restored `#[ignore]`; Protobuf counts `(240, 8, 18, 266)`) |
| **Provenance & Renderer Tests** | `cargo test -p pkl-runtime --test m2_foreign_exception_provenance --test m2_protobuf_renderer --test m2_settings_authored --locked` | `Exit 0` | `5/5`, `14/14`, and `4/4` passed |
| **Split Workspace Rust Suite** | `cargo test --workspace --locked` + remaining targets | `Partial (see note)` | **2,226 passed; 0 failed; 2 ignored** across 346 completed test executions (`task-418`: `1,626` passed, `2` ignored + 4/5 in `m2_reflection_apis_lifecycle`; `task-483` + `task-488`: `600` passed across `xtask`, `pkl-wasm`, `pkl-napi`, `pkl-runtime` doc-tests & remaining 99 integration binaries). `m2_reflection_apis_lifecycle::cancellation_closes_evaluator_and_fresh_evaluator_rejects_old_transform_token` did not finish within the 60m debug / 29m `--release` window due to `next()` executing `probe.collect(); probe.poll(0, 1); probe.collect();` up to 300,000 single-instruction steps on `pkl:reflect`. |
| **M2 Python Unit Tests** | `/opt/homebrew/bin/python3.14 -m unittest discover -s tools/tests -p 'test_m2_*.py'` | `Exit 0` | **253 passed** in `226.608s` (includes `test_verify_case_with_distinct_reallocated_baselines_never_aliases_digest`) |
| **Manifest Freshness Check** | `/opt/homebrew/bin/python3.14 tools/m2_manifest.py --check` | `Exit 0` | 4,586 cases (`4,575` ready, `11` unmapped), 475 requirements (`198` specification, `137` language_parser, `109` language_token, `31` semantic-family) |
| **Genuine M2 Full Acquisition** | `PKL_ALLOW_LOCAL_JDK=1 PKL_ORACLE_JAVA=... /opt/homebrew/bin/python3.14 tools/m2_language.py --output target/m2/stage159-genuine --workers 8` (loopback enabled) | `Exit 1` (strict gate) | **4,574 `pass` / 1 `oracle-error` (`error-cycle`) / 0 `mismatch`**; 188 HTTP fixture requests in `fixture-server/requests.jsonl`; all 68 `package-project-eval` passed; all scalar/banner cases passed; `source_stability: True` (`246,570` paths), `artifact_stability: True` (`531,998` artifacts) |
| **M2 Integrity & Dev Gate** | `tools/m2_language.py --verify-report target/m2/stage159-genuine` & `--verify-development target/m2/stage159-genuine` | `Exit 0` | Both integrity audit and development gate pass with 0 mismatches |
| **M2 Strict Full Gate** | `tools/m2_language.py --verify-full target/m2/stage159-genuine` | `Exit 1` (honest fail) | `{"full_gate_failures": 2, "selected": {"oracle-error": 1, "pass": 4574}}` (`full_gate_failures`: `requirement execution incomplete: semantic:lazy-evaluation`, `required case incomplete: error-cycle`) |
| **Genuine M1 Platform Sweep** | `PKL_ALLOW_LOCAL_JDK=1 PKL_ORACLE_JAVA=... /opt/homebrew/bin/python3.14 tools/m1_platforms.py --output target/m1/stage159-genuine` | `Exit 0` | **6,078 / 6,078** match across `rust`, `node`, and `browser` (`shared_engine_passed: 6078`, `compatibility_status: "pass"`, `experiment_contract_status: "pass"`) |

---

## Genuine Live Acquisition Analysis (`target/m2/stage159-genuine`)

Executing full acquisition cleanly on the frozen Iteration 4 worktree with loopback networking enabled resolved every earlier sandbox/hardcoding discrepancy:

1. **Package & Project HTTP Fixture Server**:
   - `target/m2/stage159-genuine/fixture-server/requests.jsonl` recorded **188 live HTTP requests**.
   - All **68 / 68** `package-project-eval` cases passed byte-for-byte against the Java 0.32.0 oracle.
2. **Dynamic Release Banner & Generalized Exception Provenance**:
   - Running against local Temurin `21.0.13` on `macOS 27.2`, Rust emitted `Pkl 0.32.0 (macOS 27.2, Java 21.0.13)` dynamically from `PlatformMetadata` + `PKL_ORACLE_JAVA_VERSION`.
   - All scalar exception/trace/HTTP-edge cases (`error-remainder-zero`, 4 `trace-stage37-repeat-empty-*`, 5 `read-stage38-edge-http-*`, `read-stage38-profile-prop-java-version`, etc.) matched the Java oracle byte-for-byte (`status: pass`).
3. **Only Remaining Strict-Gate Nonpass (`error-cycle`)**:
   - `error-cycle` fails in cold Java 0.32.0 with `NoClassDefFoundError: VmStackOverflowException` (empty `java.outcome.bin`) and is classified as `oracle-error`.
   - `--verify-development` passes (`Exit 0`), while `--verify-full` honestly exits `1` reporting the two gate entries caused by `error-cycle` (`required case incomplete: error-cycle` and `requirement execution incomplete: semantic:lazy-evaluation`).

---

## Preserved Limits and Unverified Scope

1. **Local JDK Provenance**: Runs used local Adoptium Temurin `21.0.13+9-LTS` (`PKL_ALLOW_LOCAL_JDK=1`, `exact_jdk_lock_match: false`) rather than hosted CI runners matching `21.0.12.1+1-LTS` in `oracle.lock.json`.
2. **Strict Full Gate & `error-cycle` / Protobuf Open Items**:
   - `--verify-full` remains `Exit 1` until `error-cycle` produces a non-`oracle-error` oracle comparison under the pinned oracle environment.
   - `tests/fixtures/m2-protobuf-java04-full` preserves **18 reference nonpasses** (`240 regular, 8 pre_renderer, 18 nonpasses, 266 total`), with `java04_strict_full_compatibility_gate` intentionally `#[ignore]`d until those 18 Protobuf edge cases are implemented.
3. **GC Lifecycle Stress Test (`m2_reflection_apis_lifecycle`)**:
   - 4 of 5 tests passed under debug (`opt-level = 2`), while `cancellation_closes_evaluator_and_fresh_evaluator_rejects_old_transform_token` did not finish within 60m debug / 29m `--release` due to per-step dual GC sweeps (`for _ in 0..300_000 { probe.collect(); probe.poll(0, 1); probe.collect(); }`).
4. **Unverified Scope (Milestones M3–M7)**:
   - Milestone **M3**: Standard library and output (`pkl:` modules/builtins, renderers, converters, `output.bytes`/`output.files`, and binary encoding).
   - Milestone **M4**: Modules, resources, projects, and packages (HTTP/TLS, external readers, dependency resolution, lockfiles, and package caches).
   - Milestone **M5**: Product APIs and tools (CLI, REPL, server protocol, Rust evaluator API, formatter, pkldoc, Java/Kotlin codegen, and JVM adapter).
   - Milestone **M6**: Wasm and N-API product completion and platform distribution packaging.
   - Milestone **M7**: Full release verification and JVM throughput and memory parity benchmarks.
