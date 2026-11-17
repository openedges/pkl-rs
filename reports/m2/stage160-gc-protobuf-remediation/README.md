# Stage 160 GC Safepoint Optimization & Protobuf Boundary Remediation

This canonical checkpoint report records the Stage 160 runtime changes and frozen-source verification results in `pkl-rs.rev3` against **Apple Pkl 0.32.0**. Stage 160 eliminates the GC safepoint bottleneck so the unsplit single-command `cargo test --workspace --locked` suite completes cleanly with `Exit 0` (including `m2_reflection_apis_lifecycle` 5/5), and fixes the two runtime bugs underlying all four `no-complete-reference-nonpass` Protobuf slots (`imported-alias-owner` and `constraint-read-true` under assertion-on and assertion-off modes) without modifying any pinned compatibility fixture or weakening any test budget.

---

## Summary of Changes

### 1. Build Profile & Exact GC Safepoint Hot-Path Optimization
- **`Cargo.toml`**: Added `[profile.dev.package.pkl-runtime]` (`opt-level = 2`) alongside `[profile.test.package.pkl-runtime]`. Integration test binaries (`crates/pkl-runtime/tests/*.rs`) compile `libpkl_runtime` using `[profile.dev]`, which previously left integration runs at `opt-level = 0`.
- **`crates/pkl-runtime/src/vm/reflection.rs` & `crates/pkl-runtime/src/vm.rs`**: Replaced fresh `Vec<Value>` allocations and element clones in `reflection::Payload::roots` and `reflection::Run::roots` with push-into `&mut Vec<Value>` and zero-allocation `Payload::for_each_edge`, updating the root collection call site in `Vm::collect`.
- **`crates/pkl-runtime/src/heap.rs`**:
  - Reused `mark_scratch` and `pending_scratch` buffers across `Heap::collect` invocations instead of reallocating them per safepoint.
  - Added per-slot cached outgoing `edges` / `edges_valid` (used and populated only when `self.bytes_lists.is_empty()` so `Value::Bytes` backlinks cannot stale cached edges; invalidated on `link_superclass`, `set_custom_this`, `executable_mut`, `allocate_entry`, `get_mut`, and sweep).
  - Extracted shared `trace_environment_edges` and `trace_entry_edges` helpers, guarded `Value::Bytes` backlink lookup behind `!self.bytes_lists.is_empty()`, and added dirty/length guards for `key_equalities` and `structural_keys` retention while keeping `HeapStats` (`live`, `executable_live`, `reclaimed`, `slots`, `collections`) unchanged.
- **`crates/pkl-runtime/src/vm.rs`, `crates/pkl-runtime/src/vm/query_lifecycle.rs`, `modules.rs`, `uri_resolution.rs`, `platform.rs`, `type_resolution.rs`, `generics.rs`**:
  - Bypassed `ArenaRangeIndex` and `aliases` `BTreeMap` allocations for small arena counts (`<= 8` and `<= 16`), guarded `public_operation_entries` retirement behind `!is_empty()`, and cached `unowned_declaration_roots` / `ExecutableState.code_roots` behind `declaration_roots_dirty` and `declaration_root_counts()` (`Vm::declaration_code_roots` is skipped when none of the six declaration tables changed).

### 2. Protobuf Nonpass Boundary Fixes (`no-complete-reference-nonpass`)
- **`crates/pkl-runtime/src/vm/protobuf_rendering.rs`**: Updated `protobuf_type_name` so `None` from `self.protobuf_type_class(ty)?` returns `ErrorKind::Unsupported` (`"Failed to resolve type required for rendering protobuf.\n\nConsider adding a type annotation.\nInternal TypeNode diagnostic value projection remains unresolved."`) rather than emitting `ForeignOperation::ProtobufListingOrMappingRootNullPointer` (which had misclassified `constraint-read-true` as `oracle-error`). Updated `ProtoTask::Field` (`phase == 1`) to pass `&env` to `protobuf_normalize_type`, retry cleanly if async resolution suspends (`None`), and retain both `resolved` and `normalized` handles in `context.retained`.
- **`crates/pkl-runtime/src/vm/type_resolution.rs`**: Changed `alias_check_environment` visibility to `pub(super)`.
- **`crates/pkl-runtime/src/vm/protobuf_types.rs`**: Updated `protobuf_normalize_type` (`Result<Option<Option<TypeCheck>>, RuntimeError>`) so parameterized `T::NamedAlias` normalizes the unparameterized alias body with `T::Unknown` for declared type parameters via `alias_check_environment`, matching upstream `RendererNodes.resolveType` (`typeAliasType.getVmTypeAlias().getTypeNode()`) where `TypeVariableNode.getVmClass()` is `null`. Previously `imported-alias-owner` expanded `Choice<Int>` to `Int | String` and falsely succeeded with `it_String: "child"`.
- **`crates/pkl-runtime/tests/m2_protobuf_renderer.rs`**: Added two regression tests:
  - `parameterized_type_alias_union_uses_unparameterized_alias_body_and_rejects_type_variable`
  - `constrained_union_alternative_evaluates_constraint_before_rejecting_null_type_class`
- **`tests/fixtures/m2-protobuf-java04-full/observations.tsv` & `crates/pkl-runtime/tests/m2_protobuf_java04_full.rs`**: Preserved untouched (all 18 reference nonpasses and `#[ignore]` on `java04_strict_full_compatibility_gate` remain intact per `TESTING.md`).

### 3. Manifest Digest Synchronization
- **`compatibility/m2-language.json`**: Updated modified Rust source `sha256` digests (`356` diff lines, `0` non-`sha256` changes; `4,586` cases [`4,575` ready, `11` unmapped] and `475` requirements unchanged).

---

## Environment and Provenance

- **Host Architecture**: `macOS-27.2-arm64-arm-64bit-Mach-O` (Darwin arm64, `os.version: 27.2`)
- **Rust Toolchain**: `rustc 1.98.1 (48a229cea 2026-09-01)` (`rust-toolchain.toml`)
- **JVM Oracle**: Eclipse Adoptium Temurin `21.0.13+9-LTS` (`java.version: 21.0.13`, `/Library/Java/JavaVirtualMachines/temurin-21.jdk/Contents/Home/bin/java`) invoked with `PKL_ALLOW_LOCAL_JDK=1` (`exact_jdk_lock_match: false` relative to `21.0.12.1+1-LTS` in `oracle.lock.json`)
- **Pkl Reference**: Apple Pkl `0.32.0` (`external/pkl` @ `e51fb2f91774265666f94980ec630db794a4db2d`)
- **Node.js / Browser**: Node `v26.13.1`, Playwright `1.64.1`, Chromium `156.0.8078.4` (`Google Chrome for Testing`, executable SHA-256 `c8509aee32abd6977968e9999769980b7a420dea05402d90042d9e5b41db15ef`)
- **Python**: `/opt/homebrew/bin/python3.14`

---

## Frozen-Source Verification Outcomes

| Verification Target | Command | Result | Observed Output & Notes |
| --- | --- | --- | --- |
| **Formatting** | `cargo fmt --all -- --check` | `Exit 0` | Clean across workspace |
| **Reference Pins** | `cargo xtask verify` | `Exit 0` | `Reference pins verified for Pkl 0.32.0.` |
| **Clippy Lints** | `cargo clippy --workspace --all-targets --locked -- -D warnings` (plus `--no-deps`) | `Exit 0` | 0 warnings across `pkl-runtime`, `pkl-napi`, `xtask`, `pkl-wasm` (`--no-deps` in `1m 39s`) |
| **Wasm Target Check** | `cargo check -p pkl-runtime --target wasm32-unknown-unknown --locked` | `Exit 0` | Clean pure-Wasm check |
| **Single-Command Workspace Suite** | `cargo test --workspace --locked` | `Exit 0` | **`2,220` passed; `0` failed; `2` ignored** (`frontend_probe`, `java04_strict_full_compatibility_gate`) across `337` outer test/doctest runs, plus `13` nested child passes in `m2_unicode_diagnostic_lifecycle` (`2,233` raw `test result:` passes). Includes **`m2_reflection_apis_lifecycle` 5/5 in `83.35s`** (`90.63s` solo), `pkl-runtime --lib` 263/263 in `26.61s` (`20.71s` solo vs `1359.67s` pre-Stage 160), `m2_protobuf_renderer` 16/16 (`106.86s`), and `m2_protobuf_java04_full` 5/5 + 1 ignored (`1426.47s`). |
| **Protobuf Strict Gate Capture** | `PKL_PROTOBUF_CAPTURE_DIR="$PWD/target/m2/stage160-protobuf-strict" cargo test -p pkl-runtime --test m2_protobuf_java04_full --locked java04_strict_full_compatibility_gate -- --ignored --exact --test-threads=1 --nocapture` | `Exit 101` (expected strict fail) | `266/266` attempted, `266` native terminals, `0` harness failures, `240/240` regular, `8/8` pre-renderer, `18` `REFERENCE-NONPASS`, exact `18` lines in `strict-failures.txt` (`640.10s`). All 4 `no-complete-reference-nonpass` slots (`imported-alias-owner/{on,off}`, `constraint-read-true/{on,off}`) now record `result.status.bin == "unsupported-or-api-error\n"`, `events.count.bin == "2\n"`, and 0-byte `stdout`. |
| **Python Freshness (`--check`, 6)** | `/opt/homebrew/bin/python3.14 tools/{inventory,discover,smoke_manifest,regression_manifest,m2_manifest,m2_module_eval}.py --check` (run individually) | `Exit 0` (6/6) | All 6 manifests verified (`4,586` M2 cases: `4,575` ready, `11` unmapped; `475` requirements) |
| **M2 Python Unit Tests** | `PYTHONPATH=tools /opt/homebrew/bin/python3.14 -m unittest discover -s tools/tests -p 'test_m2_*.py'` | `Exit 0` | **253 passed** in `215.895s` |
| **M1 Platforms (Sandboxed Attempt)** | `PKL_ALLOW_LOCAL_JDK=1 PKL_ORACLE_JAVA=... /opt/homebrew/bin/python3.14 tools/m1_platforms.py --output target/m1/stage160-platforms --verify-contract` (sandboxed) | `Exit 2` (preserved) | Loopback denied in sandbox (`page.goto: net::ERR_ACCESS_DENIED at http://127.0.0.1:50241/` in `platforms.stderr.bin`); raw directory preserved untouched |
| **M1 Platforms (Unsandboxed)** | Same command into `target/m1/stage160-platforms-01 --verify-contract` (unsandboxed) | `Exit 0` | **`6,078 / 6,078`** match across `rust`, `node` (`v26.13.1`), and `browser` (Chromium `156.0.8078.4`), `92` Node and `92` browser scenarios passing (`compatibility_status: "pass"`, `experiment_contract_status: "pass"`, `source_stability: True`) |
| **Focused M2 Subset (Sandboxed Attempt)** | `PKL_ALLOW_LOCAL_JDK=1 PKL_ORACLE_JAVA=... /opt/homebrew/bin/python3.14 tools/m2_language.py --output target/m2/stage160-selected --adapter selected-regression-module-eval --adapter historical-collections-module-eval --adapter historical-cli-module-eval --adapter package-project-eval` (sandboxed) | `Exit 1` (preserved) | `76 pass`, `14 oracle-contract-mismatch`, `9 mismatch`, `1 oracle-error` from sandbox loopback HTTP fixture blocking; directory preserved untouched |
| **Focused M2 Subset (Unsandboxed)** | Same 4-adapter command into `target/m2/stage160-selected-01` (unsandboxed) | `Exit 0` | **`100 / 100 pass`** (`68` `package-project-eval`, `21` `historical-cli-module-eval`, `8` `historical-collections-module-eval`, `3` `selected-regression-module-eval`; `188` HTTP fixture requests; `source_stability: True` [`246,570`], `artifact_stability: True` [`247,812`]) |
| **Focused M2 Integrity & Subset Gate** | `tools/m2_language.py --verify-report target/m2/stage160-selected-01` & `--verify-subset target/m2/stage160-selected-01` | `Exit 0` (2/2) | `{"full_gate_failures": 4942, "selected": {"pass": 100}}` (`100/100` pass with raw-byte integrity verified) |

---

## Preserved Limits and Unverified Scope

1. **Full M2 Replay Epoch Separation**: Stage 160 verifies the frozen source via the `100`-case 4-adapter subset (`target/m2/stage160-selected-01`) alongside the full Rust workspace suite and M1 sweep; it does **not** re-run the full `4,575`-case replay, whose latest full capture remains Stage 159's [`target/m2/stage159-genuine`](../stage159-strict-gate-closure/README.md) (`4,574 pass / 1 oracle-error (error-cycle) / 0 mismatch` at commit `2c25dd656`).
2. **Protobuf 18 Reference Nonpasses**: `tests/fixtures/m2-protobuf-java04-full/observations.tsv` retains `18` reference nonpasses (`14` `internal-reference-nonpass` matching Java byte-for-byte + `4` `no-complete-reference-nonpass` where cold Java crashes with uncaught `IllegalArgumentException` while Rust now consistently reports `unsupported-or-api-error\n`). `java04_strict_full_compatibility_gate` stays `#[ignore]`d and exits `101`.
3. **Cold-JVM `error-cycle` & Unmapped Candidates**: `error-cycle` (`oracle-error` under cold Java 0.32.0 due to `NoClassDefFoundError: VmStackOverflowException`) and `11` screened unmapped candidates remain open; `--verify-full` remains blocked accordingly.
4. **Environment & Open Milestones M3–M7**: Runs used local macOS arm64 Temurin `21.0.13+9-LTS` (`PKL_ALLOW_LOCAL_JDK=1`, `exact_jdk_lock_match: false`); Linux and Windows hosted CI coverage and Milestones **M3** (standard library and output), **M4** (modules, resources, projects, and packages), **M5** (product APIs and tools), **M6** (Wasm and N-API product completion), and **M7** (full release verification and JVM throughput/memory parity) remain open.
