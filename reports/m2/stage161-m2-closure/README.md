# Stage 161 M2 Development-Closure Checkpoint & Pinned-JDK Full Verification

This canonical checkpoint report records the Stage 161 runtime, tooling, and manifest updates and the complete frozen-source verification of Milestone **M2** against **Apple Pkl 0.32.0** using the exact pinned **Eclipse Adoptium Temurin `21.0.12.1+1-LTS`** JDK (`exact_jdk_lock_match: true`, without `PKL_ALLOW_LOCAL_JDK=1`).

Stage 161 closes the M2 development gate across workstreams **M2-01–M2-09** (`4,574 pass / 1 oracle-error (error-cycle) / 0 mismatch` across all `4,575` ready cases, `88/88` metamorphic checks, `6,078/6,078` Native/Node/Chromium M1 platform checks, `2,222 passed; 0 failed; 2 ignored` in a single unsplit `cargo test --workspace --locked`, and `422/422` executed Python unit tests) while preserving all strict-gate exceptions (`error-cycle` `oracle-error`, `18` pinned Protobuf reference nonpasses, `11` `required: false` `rev1` unmapped entries, unexecuted historical Python golden-fixture suites, Linux/Windows hosted CI, and Milestones **M3–M7**) honestly and without false passes.

---

## Summary of Changes

### 1. Lazy-Property Cycle Diagnostic Alignment (`crates/pkl-runtime`, `tools/m2_language.py`)
- **`crates/pkl-runtime/src/vm.rs` (`L3357–L3360`, `L3372–L3375`)**: Replaced the legacy M1 placeholder message `"cyclic forcing of " + name` at both lazy-member cycle guards with Pkl 0.32.0's `"A stack overflow occurred."`, preserving `ErrorKind::Cycle`, `LazyState::Unevaluated` rollback for retry, and `vm/guest_exceptions.rs` guest-exception classification.
- **`crates/pkl-runtime/tests/m2_runtime.rs` (`L1223–L1310`)**: Added two regression tests:
  - `lazy_property_cycle_reports_stack_overflow_and_resets_state_for_retry` (memoized `a = b; b = a` across retries and `const local` active-cycle path, asserting `ErrorKind::Cycle`, `"A stack overflow occurred."`, and `LazyState::Unevaluated`).
  - `pkl_test_catch_and_catch_or_null_capture_lazy_property_cycle_as_stack_overflow` (`test.catch(() -> a)`, `test.catchOrNull(() -> a)`, uncaught header `"–– Pkl Error ––\nA stack overflow occurred.\n"`, subsequent `stable = 42` evaluation, and GC reclamation to `0` slots).
- **`tools/m2_language.py` (`L817`) & `tools/tests/test_m2_language.py` (`L90`)**: Updated `development_passes()` and its mock fixture to expect `'–– Pkl Error ––\nA stack overflow occurred.\n'` on Rust's `error-cycle` stderr. `status()`, `subset_passes()`, `full_gate_passes()`, `check_report()`, `tools/m2_manifest.py::full_gate_failures()`, and `tests/fixtures/m2/cases.json` remain strictly unchanged, so `error-cycle` stays `oracle-error` and `--verify-full` honestly exits `1`.

### 2. Isolated `lib.pkl` Staging for `t43_043` & `t43_064` (`tools/m2_historical.py`)
- **`tools/m2_historical.py`**: Fixed the flat-directory staging defect where `t43_043_inherited_method_main.pkl` and `t43_064_imported_function_main.pkl` both executed `import "lib.pkl"` and previously matched only a missing-module error (`Cannot find module .../lib.pkl`).
  - Bumped `CLI_PROFILE` to `'pkl-0.32-historical-cli-no-cache-no-project-v2'` and added `CLI_STAGED_DIR = Path('staged')` and `CLI_ISOLATED_LIBS` staging `staged/t43_043_inherited_method/{t43_043_inherited_method_main.pkl,lib.pkl}` and `staged/t43_064_imported_function/{t43_064_imported_function_main.pkl,lib.pkl}` in the managed CLI workspace.
  - Preserved `cli_registry['resources']` as the exact 25 tracked files under `tests/fixtures/rev1-cli/` (zero edits to `tests/fixtures/` or `external/`), added `cli_registry['staged_resources']` (4 entries bound by `source_path`, `sha256`, `bytes`, and `original`), updated `command[-1]` / `resources` / `staged_input` only for those 2 cases (leaving the other 19 CLI cases untouched), added `_materialize_cli_tree` with a temporary `reference_root` for `m2_module_eval.acquire_case` / `compat.restore_changes`, and tightened `verify_cli_context` to verify `historical-cli-registry.json`, `registry_sha256`, `baseline_sha256`, and `cli_resource_baseline`.
- **`tools/tests/test_m2_historical.py`**: Added 3 tests verifying the isolated staging layout, end-to-end `acquire_cli` / `verify_cli_context` / `verify_cli_case` sibling `lib.pkl` isolation and tamper rejection, and restoration/rejection when a staged `lib.pkl` is mutated.

### 3. Manifest Synchronization & Exact Pinned JDK Provisioning
- **`compatibility/m2-language.json`**: Regenerated via `tools/m2_manifest.py` (`4,586` cases [`4,575` ready, `11` unmapped], `475` requirements [`198` specification, `137` language_parser, `109` language_token, `31` semantic-family]), updating `CLI_PROFILE` to `v2`, the 2 staged `t43_*` contracts, and `crates/pkl-runtime/src/vm.rs` SHA-256 (`8cd908fa...` → `82ccbd0a...`).
- **Pinned Oracle JDK (`.cache/oracle/temurin-21.0.12.1+1-LTS/`, git-ignored)**: Provisioned official Eclipse Adoptium Temurin `21.0.12.1+1-LTS` (`OpenJDK21U-jdk_aarch64_mac_hotspot_21.0.12.1_1.tar.gz`, SHA-256 `3623232f33a9c3baadf304480b2535f9a3cba8a58d42ecbb438ba267315d9998`), verified via `tools/oracle.py verify` with **`exact_jdk_lock_match: true`** (no `PKL_ALLOW_LOCAL_JDK=1`).

---

## Environment and Provenance

- **Host Architecture**: `macOS-27.2-arm64-arm-64bit-Mach-O` (`Darwin arm64`, `os.name: Mac OS X`, `os.version: 27.2`, `os.arch: aarch64`)
- **Rust Toolchain**: `rustc 1.98.1 (48a229cea 2026-09-01)`, LLVM `22.1.8` (`rust-toolchain.toml`)
- **Pkl Reference**: Apple Pkl `0.32.0` (`external/pkl` @ `e51fb2f91774265666f94980ec630db794a4db2d`; `oracle.lock.json` `lock_sha256: 7b77395ff5b64f7a220db74975e4c3858a36b19c5c0f5bca92681e3b1bc6504b`; `pkl-tools-0.32.0.jar` `artifact_sha256: c3b6f6215a08b31453b0fb0edd8c9da0aee31233f65abb8ab07f9c945151e059`)
- **JVM Oracle (`exact_jdk_lock_match: true`, no `PKL_ALLOW_LOCAL_JDK=1`)**: Eclipse Adoptium Temurin `21.0.12.1+1-LTS` (`java.version: 21.0.12.1`, `.cache/oracle/temurin-21.0.12.1+1-LTS/jdk-21.0.12.1+1/Contents/Home/bin/java`; `javac 21.0.12.1`, `keytool` verified)
  - `executable_sha256`: `9be1d0a740ff6502df1a762145e62860f5de4b7e17658d9cb9498da3acf9d16c`
  - `runtime_modules_sha256`: `40fcf7a32bada7e299e3513939a4472d27f8f61eb81cdd1675ea00648de7e7cf`
  - `release_sha256`: `ab66b8afeea98b25f1869fbe7c0dde368d448e7cc0f7bab0359b322ade134fe1`
- **Node.js / Browser**: Node `v26.13.1`, Playwright `1.64.1`, Chromium `156.0.8078.4`
- **Python**: `/opt/homebrew/bin/python3.14`
- **Recorded Candidate Artifacts (`target/m2/stage161-full`, all `source_binding: recorded-build`)**:
  - `language_probe`: `47ff7b9eed04e2ec08200a00eaf1e92398818725db9dd42cc515fced63c17ccb`
  - `module_eval_probe`: `3e98415be2b29370298196500d66b6b8c714cd3a68ab0daa9e1dadb1b0065fa4`
  - `jvm_snippet_probe`: `eb6d008a938b81a614d07d5f28bfaa192e8ebbc53e86c6cc9a153a6d5a5ac379`
  - `target/m2/stage161-full/report.json`: `156,852,775` bytes, SHA-256 `d68c8c8b2c65a09ce3f4431cc6bc3ccf793573c22644aeb5798c8bd45e6804be`

---

## Frozen-Source Verification Outcomes

All checks below ran on the identical frozen Stage 161 source tree (`2026-11-19`, local macOS arm64, exact pinned Temurin `21.0.12.1+1-LTS`, no `PKL_ALLOW_LOCAL_JDK=1`):

| Verification Target | Command / Evidence Path | Result | Observed Output & Notes |
| --- | --- | --- | --- |
| **Reference Pins** | `cargo xtask verify` | `Exit 0` | `Reference pins verified for Pkl 0.32.0.` |
| **Formatting** | `cargo fmt --all -- --check` | `Exit 0` | Clean across workspace |
| **Clippy Lints** | `cargo clippy --workspace --all-targets --locked -- -D warnings` | `Exit 0` | `0` warnings (`1m 53s`) |
| **Wasm Target Check** | `cargo check -p pkl-runtime --target wasm32-unknown-unknown --locked` | `Exit 0` | Clean pure-Wasm check (`4.79s`) |
| **Oracle Pin Verification** | `tools/oracle.py verify` (pinned `21.0.12.1+1-LTS`, no `PKL_ALLOW_LOCAL_JDK=1`) | `Exit 0` | `exact_jdk_lock_match: true`, `Pkl 0.32.0 (macOS 27.2, Java 21.0.12.1)` |
| **Python Freshness (`--check`, 7)** | `tools/{inventory,discover,smoke_manifest,regression_manifest,m2_manifest,m2_module_eval,m2_jvm_classpath}.py --check` | `Exit 0` (7/7) | `4,586` M2 cases (`4,575` ready, `11` unmapped), `475` requirements, `888` original-ready module-eval |
| **Single-Command Workspace Suite** | `cargo test --workspace --locked` | `Exit 0` | **`2,222` passed; `0` failed; `2` ignored** (`frontend::frontend_probe`, `m2_protobuf_java04_full::java04_strict_full_compatibility_gate`) across `337` outer targets/doctests (`13` nested child passes excluded; `2,235` raw `test result:` passes). Key targets: `pkl-runtime --lib` `263/263` (`21.22s`), `m2_reflection_apis_lifecycle` `5/5` (`45.63s`), `m2_protobuf_renderer` `16/16` (`46.53s`), `m2_protobuf_java04_full` `5/5 + 1 ignored` (`530.07s`), `m2_runtime` `43/43` (`1.50s`). |
| **Python Unit Tests (Executed)** | `test_m2_*.py` (`256`, `367.384s`) + core shared tooling subset (`166`, `45.550s`: `test_m1_*`, `oracle`, `compat`, `inventory`, `discover`, `regressions`, `evidence`, `pantry`, `vendor_sources`, `runtime_source_inputs`, `workspace_storage`) | `Exit 0` | **`422 / 422` passed** (`0` failed, `0` skipped). Untouched historical golden-fixture Python suites were explicitly not run in this epoch (see Preserved Limits). |
| **M1 Multi-Target Platform Sweep** | `tools/m1_platforms.py --output target/m1/stage161-platforms --verify-contract` (unsandboxed) | `Exit 0` | **`6,078 / 6,078`** match across `rust`, `node` (`v26.13.1`), and `browser` (Chromium `156.0.8078.4`, Playwright `1.64.1`); `shared_engine_passed: 6078`; `92` Node and `92` browser scenarios (`browserErrors: []`); `compatibility_status: "pass"`, `experiment_contract_status: "pass"`, `source_stability: true`, `exact_jdk_lock_match: true`. |
| **Focused CLI Staging Check** | `tools/m2_language.py --output target/m2/stage161-cli-check --adapter historical-cli-module-eval` + `--verify-report` + `--verify-subset` | `Exit 0` (3/3) | **`21 / 21` pass** (`exact_jdk_lock_match: true`, `source_stability: true`, `artifact_stability: true`). Confirmed byte-exact match on isolated `lib.pkl` imports: `t43_043_inherited_method_main.pkl` (`973/973 B`, `boom` at `lib#A.f`) and `t43_064_imported_function_main.pkl` (`915/915 B`, `Function1` PCF error at `lib.pkl:6`). |
| **Protobuf Strict Gate Capture** | `PKL_PROTOBUF_CAPTURE_DIR="$PWD/target/m2/stage161-protobuf-strict" cargo test -p pkl-runtime --test m2_protobuf_java04_full --locked java04_strict_full_compatibility_gate -- --ignored --exact --test-threads=1 --nocapture` | `Exit 101` (contracted strict fail) | `266/266` attempted (`535.96s`), `266` native terminals, `0` harness failures, `240/240` regular, `8/8` pre-renderer, exact `18` `REFERENCE-NONPASS` entries in `strict-failures.txt` (`experiment_contract_pass: true`, `strict_compatible: false`; all 4 `no-complete-reference-nonpass` slots record `unsupported-or-api-error`, `events=2`, `stdout_bytes=0`). |
| **Live Caught-Cycle Check** | Ad-hoc `/tmp` Java `21.0.12.1+1-LTS` vs Rust `module_eval_probe` (`pkl:test` `test.catch(() -> a)` with `local a = b; local b = a`, after `init = test.catch(() -> throw("init"))`) | Byte-exact match | Both exit `0`, `stderr=""`, identical stdout `'init = "init"\nres = "A stack overflow occurred."\n'`. Uncaught public `a = b; b = a` after `init`: both exit `1` with header `'–– Pkl Error ––\nA stack overflow occurred.\n'` (Java prints `┌─ 711 repetitions of:`; Rust detects cycle directly). |
| **Full 4,575-Case M2 Replay** | `tools/m2_language.py --output target/m2/stage161-full --workers 4 --record-only` (unsandboxed, pinned `21.0.12.1+1-LTS`) | `Exit 0` (`5h 52m`) | **`4,574 pass / 1 oracle-error (error-cycle) / 0 mismatch`** across all `4,575` ready cases (`schema_version: 7`, `88/88` metamorphic pass, `source_stability: true` [`246,570` paths], `artifact_stability: true`, `evidence_sha256: 531,998` files, all 3 candidates `recorded-build`, `exact_jdk_lock_match: true`). |
| **Full Replay Verifiers (3)** | `tools/m2_language.py --verify-report`, `--verify-development`, `--verify-full` on `target/m2/stage161-full` (no `PKL_ALLOW_LOCAL_JDK=1`) | `--verify-report`: `Exit 0` (`~46m`)<br>`--verify-development`: `Exit 0` (`~45m`)<br>`--verify-full`: `Exit 1` (`~45m`, honest) | All 3 confirm `{"full_gate_failures": 2, "selected": {"oracle-error": 1, "pass": 4574}}`; exact `full_gate_failures`: `["requirement execution incomplete: semantic:lazy-evaluation", "required case incomplete: error-cycle"]`. (`--verify-subset` was omitted because `--verify-full` already executes `check_report()`, `subset_passes()`, and `full_gate_passes()`.) |

### Full Replay Per-Adapter Breakdown (`target/m2/stage161-full`)

| Adapter | Cases | Result | Notes |
| --- | ---: | --- | --- |
| `language-api-string` | 3,586 | `3,585 pass / 1 oracle-error / 0 mismatch` | Sole nonpass: `error-cycle` (`oracle-error`); `error-remainder-zero` passes byte-for-byte; `88/88` metamorphic relations pass |
| `language-module-eval` | 888 | `888 / 888 pass` | Includes `api/list.pkl` under reviewed `-Xmx2g` `execution_profile` (`compatibility/experiments/list-large-heap.json`) |
| `package-project-eval` | 68 | `68 / 68 pass` | `188` live HTTP fixture requests recorded in `fixture-server/requests.jsonl` |
| `historical-cli-module-eval` | 21 | `21 / 21 pass` | `CLI_PROFILE v2`; `t43_043_inherited_method_main.pkl` (`963/963 B`) & `t43_064_imported_function_main.pkl` (`910/910 B`) exact raw-byte match |
| `historical-collections-module-eval` | 8 | `8 / 8 pass` | Complete 4-channel raw-byte match |
| `selected-regression-module-eval` | 3 | `3 / 3 pass` | `t36-foreign-nominal-identity`, `t40-imported-declaration-authority`, `t40-foreign-namesake` |
| `language-jvm-classpath-output` | 1 | `1 / 1 pass` | `basic/import1b.pkl` (`['pass', 'pass']` across both repetitions) |
| **Total Ready Cases** | **4,575** | **`4,574 pass / 1 oracle-error / 0 mismatch`** | `475 / 475` requirements implemented & mapped (`474` fully passing; `semantic:lazy-evaluation` blocked solely by `error-cycle`) |

---

## Preserved Limits and Open Scope

1. **`error-cycle` Strict-Gate Exception (`--verify-full` Exit `1`)**:
   - Under cold pinned Java `21.0.12.1+1-LTS` (`-Xmx512m`), `error-cycle` exhausts stack before `org.pkl.core.runtime.VmStackOverflowException` initializes and throws `NoClassDefFoundError` (`java.outcome.bin`: `0 B`, `java.stderr.bin`: `88,665 B`, SHA-256 `352580a8b73dab37de88992e96d5b6a3df815fd13325011ebc1c6abe8f56c883`), classified as `oracle-error`. Rust exits `1` with `474 B` stderr (SHA-256 `93667baa727cd16a5044570fc21fa4a77abc3d815db00a283172594f16146847`, header `–– Pkl Error ––\nA stack overflow occurred.`).
   - Even when `VmStackOverflowException` is preinitialized (`reports/m2/cycle-complete.json`), Java's uncaught stack-overflow repetition count (`┌─ <N> repetitions of:`) varies nondeterministically across repeats (`695`, `699`, `705`; `within_variant_raw_stability.preinitialize: false`), making exact raw-byte equality structurally impossible for an uncaught property cycle. `error-cycle` therefore remains an explicit, honest strict-gate failure (`full_gate_failures: ["requirement execution incomplete: semantic:lazy-evaluation", "required case incomplete: error-cycle"]`) rather than weakening `full_gate_failures()`.
2. **Protobuf `18` `REFERENCE-NONPASS` Slots (Carried to M3)**:
   - `tests/fixtures/m2-protobuf-java04-full/observations.tsv` retains all `18` reference nonpasses (`14` `internal-reference-nonpass` from upstream 0.32.0 NPE bugs in `RendererNodes.java` / `TypeNode.java` where Rust matches Java with `0`-byte diff across all projected channels and events + `4` `no-complete-reference-nonpass` [`imported-alias-owner/{on,off}`, `constraint-read-true/{on,off}`] where Java 0.32.0 crashes with uncaught `IllegalArgumentException` inside `VmException.toPklException()` leaving no `result.*`/`events/*` files, while Rust reports `unsupported-or-api-error\n`). `java04_strict_full_compatibility_gate` stays `#[ignore]` and exits `101`; per `PLAN.md`, `pkl:protobuf` belongs to **M3** and remains tracked in release criteria.
3. **`11` `required: false` Unmapped `rev1` Candidates**:
   - All `11` non-ready entries in `compatibility/m2-language.json` originate from `compatibility/rev1-inventory.json` and are screened as `required: false` in `tools/m2_historical.py`: `8` non-standalone support library modules (`RESOURCE_ONLY_MAP`, including `t43_043_inherited_method_lib.pkl` and `t43_064_imported_function_lib.pkl`, now genuinely imported as sibling `lib.pkl` by their ready main cases) and `3` deduplicated aliases (`TRACKED_REGRESSIONS`) byte-identical to the `3` ready `selected-regression-module-eval` cases.
4. **Unexecuted Historical Python Suites & Platform Provenance**:
   - Stage 161 executed `422/422` Python unit tests (`test_m2_*.py` `256` + core shared tooling `166`); untouched historical golden-fixture Python suites (separate `1,224`-test discovery run canceled at `250/1,224` with `0` failures) remain unexecuted in this epoch.
   - All Stage 161 runs are local **macOS arm64** verification (`exact_jdk_lock_match: true`); **Linux and Windows** hosted CI verification remains open.
5. **Open Milestones M3–M7**:
   - **M3** (next milestone): Standard library and output (`1,222` stdlib / `542` builtin inventory closure, all `17` renderers including Protobuf's `18` reference nonpasses, regex `(?c)` canonical equivalence).
   - **M4**: Modules, resources, projects, and packages (full Host/I/O, HTTP headers/redirects/proxy, external readers, package caches/lockfiles/creation including Java's callback new-resource `ConcurrentModificationException`, Pantry packages, example allowlist UTF-16 regex cleanup).
   - **M5**: Product APIs and tools (CLI commands including `format` [30 `formatter-snippet` cases], REPL, server, Rust evaluator API including `after-close` API exceptions, pkldoc, Java/Kotlin codegen, JVM adapter).
   - **M6**: Wasm and N-API product completion (JS unpaired surrogates, long clock range, distribution packages).
   - **M7**: Full release verification on target OSes and JVM throughput/memory parity (`<= 1.00`, including M1 carryovers `numeric::double_to_string` 6.1x cost and warmed-up Graal Tier-2 VM workloads).
