# M7 Full Release Verification — Local macOS arm64 Checkpoint & Frozen-Source Evidence

This canonical checkpoint report records Milestone **M7 (Full Release Verification)** performance & peak-RSS optimization, memory-reclamation soak testing, seeded differential fuzzing, unified release manifest & CI matrix automation, and complete frozen-source cross-milestone regression verification against **Apple Pkl 0.32.0** using the exact pinned **Eclipse Adoptium Temurin `21.0.13.1+2-LTS`** JDK (`exact_jdk_lock_match: true`, without `PKL_ALLOW_LOCAL_JDK=1`), with separate cross-comparison against the archived **M0 GraalVM CE `25.0.1`** optimizing-JIT baseline (`reports/m0/benchmarks.json`).

All evidence in this report is **local macOS `arm64` (`darwin-arm64`) verification**, **NOT hosted CI**. On the frozen `#154`/`#156` source tree (`unsafe_code = "deny"`, zero production Java fallback, `external/pkl` and `external/pkl-pantry` untouched), Milestone M7 closes all local correctness, soak, fuzz, and non-browser performance/RSS targets:

- **Long-Running Memory Reclamation Soak (`target/m7/m7-final-soak-03`, `--profile standard`)**: **`18 / 18 bounded` STRICT PASS** (`--verify-report` Exit `0`, `--verify-strict` Exit `0`; `native` `6/6`, `napi-node` `6/6`, `wasm-node` `6/6`; `0` RSS leaks, `0` FD leaks, `0` orphan child PIDs, `0` second-half Wasm linear-memory page growth across all 6 `wasm-node` runs; direct `SourceSession` GC `heap_probe` `assertions_ok: true`).
- **Seeded Differential Fuzzing (`target/m7/m7-final-fuzz-03`, seed `0x4d37504b4c303332`)**: **`215 / 215 PASS` STRICT PASS** (`--verify-report` Exit `0`, `--verify-strict` Exit `0`; `210` generated across all 7 categories + `5/5` saved and fixed regressions `reg_01`–`reg_05`; `0` raw-byte mismatches across all 7 render formats, `pkl-binary` Rust decode round-trip, and `pkl format -` raw parity + 2nd-pass idempotency).
- **Performance & Peak RSS Benchmark Suite (`target/m7/m7-final-bench-02`, `--profile full`, 11 families / 34 workload-modes)**: **`34 / 34` raw-byte verified before timing & converged** (`0` inconclusive); **`34 / 34` faster than live Temurin 21** (`0.022x`–`0.904x`); **`32 / 34 PASS`** (**`32 / 32` non-browser PASS** on BOTH execution time and `os.wait4` peak RSS: `28/28` Native + `2/2` N-API + `2/2` Wasm-Node); **`18 / 18 PASS`** vs archived M0 GraalVM CE `25.0.1` (both time and peak RSS); **`2 / 34 FAIL`** solely on structural Chromium multi-process browser-tree peak RSS (`wasm-browser-eval-text` `5.415x`, `wasm-browser-typed-conversion` `5.581x`, both beating Java 2x–3x on execution time at `0.333x` and `0.538x`). `--verify-report` exits **`0`**; `--verify-strict` honestly exits **`1`**.
- **Unified M7 Release Gate (`tools/m7_manifest.py --verify-release`)**: Honestly exits **`1`** with **`integrity_passed: true`**, **`soak_strict_passed: true`**, **`fuzz_strict_passed: true`**, **`benchmark_strict_passed: false`**, **`local_strict_gates_passed: false`**, and **`release_ready: false`** because (1) the `2/34` Chromium multi-process browser-tree RSS rows exceed `1.00x` Java and (2) release platform targets outside local `darwin-arm64` / `browser-chromium` (`darwin-x64`, `linux-x64-gnu`, `linux-arm64-gnu`, `win32-x64-msvc`, `browser-firefox`, `browser-webkit`, `hosted-ci`) remain unverified.
- **Frozen-Source Cross-Milestone Regression Sweep (`M1`–`M6`, Rust workspace, Python suite, static/lint/`wasm32`/12 manifests)**: Zero regressions across M1 (`6,078/6,078`), Full M2 (`4,574 pass / 1 oracle-error [error-cycle] / 0 mismatch`), Protobuf strict (contracted Exit `101`, `240/8/18` in `9.14s`), M3 (`1,781/1,781` + `118/118`), Full M4 (`429/429`), Full M5 (`142/142`), Full M6 (`80/80 + 6/6 capability-blocked`), full Rust workspace (`378` outer targets, `2,471 passed; 0 failed; 2 ignored` with honest sandbox loopback split), and `132` Python test modules (`57/57` active `510/510` after clean `test_pantry` retry + `73/75` historical, preserving `2` pre-existing Stage-65 stale-`oracle.lock.json` failures).

---

## Summary of M7 Implementation & Verification Tracks

### 1. Shared Runtime Performance, GC Reclamation & JS/Wasm Optimization
- **Pre-Compiled `pkl:base` Snapshot & VM Hot Paths (`crates/pkl-runtime`)**: Added pre-compiled `pkl:base` snapshot loading (`vm/base_snapshot.rs::BaseSnapshot`) with `ExecutableRoots.base_overrides`, fast `stdlib_kind` literal dispatch, early `canonical_builtin_class` cache hits, boxed `typecheck::Task::RestoreEnvironment` (`600 B -> 144 B`), `Heap::allocate_function` / `allocate_strict_list` / `get_fast`, integer loop/recursion specialization (`vm/fast_int.rs::FastIntOp`, `FastIntBlock` + `FastIntTree` gated on `has_self_calls || arity == 2`), non-capturing lambda fast paths (`MakeFunction`), `StrictList` `map`/`fold` + `try_fast_member_chain_bulk`, compact `list_elements` strict `List` representation, and reflection/`pkldoc` hot-path elimination.
- **Bounded-Memory Tracing GC & Compact Tables (`crates/pkl-runtime`, `crates/pkl-host-native`, `crates/pkl-wasm`, `packages/pkl-wasm`)**: Enabled automatic inter-op tracing GC and uncached URI alias bounding; replaced monotonic block-ID-indexed sparse `Vec` tables in `ExecutableRoots` and `FastIntCache` with compact power-of-2 Fibonacci-hashed open-addressed tables (`O(live)` memory with shrinking rehash on GC, retaining the frozen dense `Arc` prefix for `pkl:base`); switched `staged_sources`/`staged_custom_sources` consumption to `.remove()` in native and Wasm hosts; and added `_inFlightCount`-guarded `_disposeWasmIfClosed()` (`packages/pkl-wasm/src/index.mjs`) freeing Wasm handles immediately on idle `close()` (or in-flight unwind) plus `analyzer.free()` in `analyzeImportGraph`.
- **Fast Binary, Columnar Transport & V8 Liftoff Outlining (`crates/pkl-runtime`, `crates/pkl-napi`, `crates/pkl-wasm`, `packages/pkl-{napi,wasm}`)**: Added zero-task `vm/pkl_binary_render.rs::try_fast_encode_pkl_binary` (exact wire parity vs resumable `RenderRun`), flat lossless pure-JS `tryDecodePklBinaryFast` (`packages/pkl-wasm/src/values.mjs`, `packages/pkl-napi/src/runtime.cjs`) with strict fallback, shared `vm/columnar_value.rs::ColumnarValueBuffer` with a 1-entry `gc_epoch`-guarded immutable cache (`<= 256 KiB`, cleared on `Vm::collect`/`Vm::close`) and `OP_COLUMNAR_LIST` `slot_perm` packing (falling back seamlessly to resumable `RenderRun` on unforced members, cycles, unpaired surrogates, annotations, or edge types), N-API bounded warm try-poll (`32_768` steps, `EvalOp::ContinueRunning`) + `BufferSlice::copy_from`, Wasm `pure_standard_host` + `DEFAULT_POLL_BUDGET = 65536`, and `#[cfg_attr(target_arch = "wasm32", inline(never))]` cold-bootstrap/large-function outlining reducing V8 Liftoff zone high-water mark by ~48 MiB.

### 2. Exact-Output-Verified M7 Benchmark, Memory Soak, Fuzzing & Release Manifest Harness
- **Benchmark Harness (`tools/m7_benchmark.py`, `benchmarks/m7_workloads.json`, `benchmarks/m7_fixtures/**`, `crates/pkl/examples/m7_bench_probe.rs`)**: Measures all **11 families / 34 workload-modes** (`6` core workloads across `cold-cli`/`persistent`/`fresh-evaluator`, plus `reflection-stdlib`, `renderers-files-binary`, `packages-http-cache` warm & cold, `repl`, `server`, `formatter`, `pkldoc`, `napi`, `wasm-node`, and real Playwright Chromium `wasm-browser` with COOP/COEP `cross_origin_isolated: true`). Verifies 100% raw-byte output/error/file parity against Java 0.32.0 **before** timing, records per-PID peak RSS via `os.wait4` `ru_maxrss` (plus full Chromium descendant-tree `/bin/ps` RSS for browser runs), compares live against pinned Temurin `21.0.13.1+2-LTS` and separately against archived M0 GraalVM CE `25.0.1`, and separates `--verify-report` from `--verify-strict`.
- **Memory Reclamation Soak Harness (`tools/m7_memory_soak.py`, `crates/pkl/examples/m7_memory_soak_probe.rs`, `tests/fixtures/m7/soak/run_{napi,wasm}_soak.mjs`)**: Runs all **6 scenarios** (`repeated-success`, `repeated-failure`, `unique-sources-churn`, `cyclic-graphs`, `lifecycle-cancel-timeout-close`, `cache-churn`) across **3 targets** (`native`, `napi-node`, `wasm-node` = **18 target-scenarios**), enforcing bounded RSS growth ratio, second-half OLS slope/delta/`R^2`, Wasm linear memory page plateau, zero FD leaks, zero orphan child PIDs, and direct `SourceSession` GC reclamation assertions (`heap_probe`).
- **Seeded Differential Fuzzer (`tools/m7_fuzz.py`, `tests/fixtures/m7/fuzz-regressions/`)**: Deterministic `splitmix64` seeded generator (`0x4d37504b4c303332`) across **7 categories**, all 7 render formats, `pkl-binary` Rust decode round-trip (`m3_pklbinary_decode_probe`), and `pkl format -` raw parity + 2nd-pass idempotency, with automatic delta shrinking and permanent saved regressions (`reg_01`–`reg_05` + Rust unit tests in `crates/pkl-runtime/tests/m7_fuzz_and_gc_regressions.rs`).
- **Unified Release Manifest & CI Workflow (`tools/m7_manifest.py`, `compatibility/m7-mappings.json`, `compatibility/m7-release.json`, `.github/workflows/ci.yml`)**: Aggregates all 6 prior milestone manifests (`M0`, `M2`–`M6`) with all 12 M7 requirement groups, 9-entry OS/browser/hosted-CI platform matrix, 3 explicit `2026-11-20` scope exclusions, and 1 unpinned optional reference (`native-image-reference`), plus 18 unit/tamper tests in `tools/tests/test_m7_{benchmark,memory_soak,fuzz,manifest}.py`.

---

## Environment, Provenance, and Frozen Identities

- **Provenance Type**: Local macOS `arm64` (`darwin-arm64`) verification (**NOT hosted CI**; `darwin-x64`, `linux-x64-gnu`, `linux-arm64-gnu`, `win32-x64-msvc`, Firefox, WebKit, and hosted GitHub Actions CI remain unverified).
- **Host**: `macOS-27.2-arm64-arm-64bit-Mach-O` (`Apple M1`, `8` CPUs, `8,589,934,592 B` [8 GiB] RAM, AC Power, zero thermal/performance warnings).
- **Base Git `HEAD` (before M7 commit)**: `8c3ab95b25a304d8c14bb99eb01f5f523be48aa3` (`8c3ab95b2`).
- **Toolchains & Pinned Browser/Wasm Tools**:
  - `rustc 1.98.1`, `cargo 1.98.1` (`rust-toolchain.toml`)
  - `Python 3.14.10` (`/opt/homebrew/bin/python3.14`)
  - `Node.js v26.13.1` (`/opt/homebrew/bin/node`)
  - `wasm-bindgen 0.2.108` (`.cache/m1-tools/bin/wasm-bindgen`)
  - `Playwright 1.64.1`, `Chromium 156.0.8078.4`
- **Pinned References & Oracles**:
  - Apple Pkl `0.32.0` (`external/pkl` @ `e51fb2f91774265666f94980ec630db794a4db2d`, clean)
  - Apple Pkl Pantry (`external/pkl-pantry` @ `904a68184e54da378b0a0e8a911a3fab3956f693`, clean)
  - `references.lock`: `278f48fd7697459281aa78a706c9ca57222f36a8c0785d838a6449fbaf26d30b`
  - `oracle.lock.json`: `035fa94a2706344b29d94c8cd3d233d2447007bad0a1a52f8911756e460f0bbc`
  - `pkl-tools-0.32.0.jar`: `c3b6f6215a08b31453b0fb0edd8c9da0aee31233f65abb8ab07f9c945151e059`
  - **Live Head-to-Head JDK (`exact_jdk_lock_match: true`, NO `PKL_ALLOW_LOCAL_JDK=1`)**: Eclipse Adoptium Temurin `21.0.13.1+2-LTS` (`jdk.executable_sha256`: `8261edc44ac1643e7aba9f0df5155b6d81f7977cc8a0374bb7630d838d2a3910`, `jdk.runtime_modules_sha256`: `c09beb71c3d4ac1fd9dd754e88d2b96c0eca603ce134756e25bd4528df0d6a45`, `jdk.release_sha256`: `2f1cd602bf21d35e6e2976d6a351693ff609bfcb285762263a54bf20e8a40afd`)
  - **Archived Optimizing-JIT Reference (Strictly Separated)**: M0 GraalVM CE `25.0.1` (`reports/m0/benchmarks.json`, SHA-256 `ecab898e432e0fcd0ae81e1cda44fa2789df20c6a158531b6d42389173251914`; preserves M0's Tier-1 `FrameWithoutBoxing` bailout limitation on `recursive-compute`)
  - **2026-11-20 Scope Exclusions**: `jvm-adapter` (`java_api: 8,239`, `kotlin_api: 597`, `jvm_spi: 2`), `pkl-codegen-java`, `pkl-codegen-kotlin`. **Unpinned Optional Reference**: `native-image-reference` (`unverified-not-pinned`).

### Frozen M7 Tools, Contracts & Synchronized Manifest SHA-256s (`#156`)

| File | Size | SHA-256 |
| --- | ---: | --- |
| `tools/m7_benchmark.py` | `114,306 B` | `3048d16bcf8941703081c96d7372f110702803bc88de397e266f6004f061e1d6` |
| `tools/m7_memory_soak.py` | `51,262 B` | `f770515d27e09bff1a88948e43ca0ef2402636b97b649848e9d339982cd56380` |
| `tools/m7_fuzz.py` | `69,115 B` | `44323cd9e5245d076049f8bcce977a5bd6bfa93d3f03369cea9e038dc3e997b` |
| `tools/m7_manifest.py` | `25,563 B` | `f4a6b0f8352f2cd0500266522fed75898c574cf2ca7381385ff5c8ba6a9a4c5b` |
| `benchmarks/m7_workloads.json` | `8,272 B` | `b498064d67d0823dd9893d8bcc73af5a38b37affbe4143a2a616b9453f8ad42b` |
| `crates/pkl/examples/m7_bench_probe.rs` | `10,771 B` | `63b57f30bdaa87a03abc8e4ac4e880b8ecef52aab92fcd65cf993be07d5b64a4` |
| `crates/pkl/examples/m7_memory_soak_probe.rs` | `46,281 B` | `4beeac4e1c11f300db53e7cc10ec7556ffc404fd216f7d255af6d720123b6be9` |
| `.github/workflows/ci.yml` | `23,648 B` | `b95958a6b5b1e23a80b90302df59f537f458fb85a1c020d3c34d5226dba42a1c` |
| `compatibility/m2-language.json` | resynced (`non_sha256_lines=0`) | `5bbd56de0180fc2b6b34d627645e676443157a3d3c8f8fbfccb69560bde12dc9` |
| `compatibility/m3-stdlib-output.json` | resynced (`non_sha256_lines=0`) | `d5331f3fc114ab53fe28e4597f8fed90dffbc636b4155d26fd6d32b6249a33fa` |
| `compatibility/m4-io-packages.json` | resynced (`non_sha256_lines=0`) | `3b9d42ea5c83a8277de92d0746126a40d9ededac6bebcf6d2e20ba566a4e0558` |
| `compatibility/m5-product-tools.json` | resynced (`non_sha256_lines=0`) | `8e95dfe52708b72ed1bc1af7d65ac2ec75649955f58a0333a3b7fac98d9c1c6d` |
| `compatibility/m6-wasm-napi.json` | resynced (`non_sha256_lines=0`) | `bf8c1912ff3426e637beae9f4c09cad81d89dcaf87a72dffc862356b5efcb7db` |
| `compatibility/m7-mappings.json` | `12` req groups | `6b2a109a4a26523851d0c4db1d24299b6bc506d397161f764e18d021f3efd316` |
| `compatibility/m7-release.json` | unified M7 manifest | `24f8e52e8d4b203ad64fc5dae0765beda9693ecbf442d28a52462906c6e7df4f` |

### Frozen Release Candidate Artifact SHA-256s (`#154`, Verified Bit-for-Bit Unchanged Through `#157`, `#159`, `#185`)

| Artifact | Size / Binding | SHA-256 |
| --- | ---: | --- |
| `target/release/pkl` | `15,023,744 B` (`recorded-build`) | `856a126af002193adb1ccd388bd8f88350794bcb597b1151ecc27f91d5adaf5c` |
| `target/release/pkldoc` | `13,808,800 B` (`recorded-build`) | `bbac3c9eb62007415294a762823f2adf23931b64cacf5b22b48a3601f9bf40c3` |
| `target/release/examples/m7_bench_probe` | `12,061,424 B` (`recorded-build`) | `33ec6e411f7ac31d1ed088726e448d3a44e757a16bfde2f02e9804b21faab755` |
| `target/release/examples/m7_memory_soak_probe` | `12,128,608 B` (`recorded-build`) | `653a1bdfa72b2a640cb70b1aff2c2318f53aae4f64fcc71dfce5915f4472aa75` |
| `target/release/examples/m3_pklbinary_decode_probe` | `2,454,336 B` (`recorded-build`) | `5c01077970d42a95daf51791c4a1c7f6813abd4502856edfaba68f6fe68eec5a` |
| `packages/pkl-napi/native/pkl_napi.darwin-arm64.node` | `14,652,272 B` (`recorded-build`) | `20b44567eafd1b6b41b7d5271eaed5024662bca7340689286ea58210717af83d` |
| `packages/pkl-napi/dist/index.cjs` | `workspace-dist` | `bce127d37b255388055f389f8333e16ac7ebcbdd566a873d7bfbba7ceaf7a68f` |
| `packages/pkl-napi/dist/index.mjs` | `workspace-dist` | `a25cc23a89f55af0f90aeff79ddf67bca7814fb61e46b2f3ca9dbe7f9cbb8c7d` |
| `packages/pkl-wasm/dist/wasm/pkl_wasm_bg.wasm` | `10,287,040 B` (`recorded-build`) | `93f4f4879527391358911481735497b332b88655d7404ada94c5ba25d8e5f404` |
| `packages/pkl-wasm/dist/wasm/pkl_wasm.js` | `workspace-dist` | `5dca8a1f9b7e38b0e6cd373bc1ff66bf89ca7e0fdaf5333ec1bde3e94c535217` |
| `packages/pkl-wasm/dist/index.mjs` | `workspace-dist` | `2889592b6aeeb783d0969abb2dd5d05f83a51f6eb44a1ffb54d383f0a23ca480` |
| `packages/pkl-wasm/dist/values.mjs` | `workspace-dist` | `52ed0c84b427d1baa63c5285a1350daca3f250f94956e54a0aefa248f41ff475` |
| `packages/pkl-wasm/dist/runtime.mjs` | `workspace-dist` | `b2adeb0ce5fca7e347f71a6419aef839b986b71c78ca05a8d890dc00875746fb` |
| `packages/pkl-wasm/dist/host.mjs` | `workspace-dist` | `652fa4cf8f6dd03f50e9e170b1b96fc57d55f9f3b95bfef47148531ce19fc8b0` |

---

## Performance & Peak RSS Benchmarks (`target/m7/m7-final-bench-02`)

- **Sealed Report**: `target/m7/m7-final-bench-02/report.json` (`ea54bb9b00ffed22947f3aa51a54135d65e209a41bf1a4c6afd75ce8cde02653`), `sources.snapshot.json` (`9c0b72776b1e4a69e95dc4cdd9c2ba305fa9b7aa863a4093fc78626fa52bf51f`, `source_stability: true`, `349` source files, `1,154` sealed evidence files).
- **Sampling Contract (`--profile full`, `strict_eligible: true`)**: Cold CLI `5` runs; persistent/fresh API `2` independent forks (`10s–45s` warmup, `8` convergence windows, `<= 20%` relative spread, `9` x `0.25s` measured samples); steady server `100` sequential requests after warmup; JS/Wasm `3s–20s` warmup, `8` convergence windows, `9` x `0.25s` samples. All **`34 / 34`** workload-modes passed pre-timing raw-byte verification and converged cleanly (`0` inconclusive).
- **Pre-Optimization Baseline -> Final Progression**:
  - Phase 1 baseline `target/m7/bench-baseline-all-02` (`report.json` SHA-256 `2e1daa4e4770fa48337b4476278362a579e9206fa0db3628e05072518095e973`): **`13 / 34 PASS`**, **`21 / 34 FAIL`** vs live Temurin 21 (`6 / 18 PASS`, `12 / 18 FAIL` vs M0 GraalVM CE `25.0.1`).
  - Final frozen `target/m7/m7-final-bench-02`: **`32 / 34 PASS`**, **`2 / 34 FAIL`** vs live Temurin 21 (**`34 / 34` PASS on execution time**, **`32 / 32` non-browser PASS** on both execution time and peak RSS), and **`18 / 18 PASS`** vs archived M0 GraalVM CE `25.0.1`.
  - Headline improvements (Rust/Java Temurin 21 ratios): `scalar:persistent` `578.904x -> 0.433x` (RSS `1.135x -> 0.027x`), `scalar:fresh-evaluator` `1065.612x -> 0.833x`, `stdlib-collections:persistent` `229.129x -> 0.198x` (RSS `2.029x -> 0.147x`), `recursive-compute:persistent` `29.101x -> 0.074x`, `large-output:persistent` `658.067x -> 0.860x`, `reflection-stdlib:persistent` `51.603x -> 0.636x` (RSS `6.539x -> 0.478x`), `pkldoc-single-package:cold-cli` `4.625x -> 0.161x` (RSS `2.220x -> 0.445x`), `server-session:steady-server` `6.316x -> 0.122x`, `napi-eval-text` `222.437x -> 0.204x`, `napi-typed-conversion` `73.554x -> 0.546x`, `wasm-node-eval-text` `439.736x -> 0.338x`, `wasm-node-typed-conversion` `100.128x -> 0.499x` (RSS `0.977x -> 0.951x`), `wasm-browser-eval-text` `550.429x -> 0.333x`, `wasm-browser-typed-conversion` `97.343x -> 0.538x`.

### Complete 34-Workload Table (`target/m7/m7-final-bench-02`)

Every workload below was verified byte-for-byte against Java `0.32.0` before timing and achieved convergence (`target <= 1.00x` on both time and peak RSS):

| Workload (`id:mode`) | Rust Time (med) | Java Time (T21 med) | Time Ratio (Baseline) | Rust Peak RSS | Java Peak RSS (T21) | RSS Ratio (Baseline) | Verdict (T21) | vs M0 GraalVM 25.0.1 (Time / RSS) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | :---: | --- |
| `scalar:cold-cli` | `21.84 ms` | `834.79 ms` | **`0.026x`** (`0.024x`) | `16.8 MiB` | `169.9 MiB` | **`0.099x`** (`0.104x`) | `pass` | `0.024x / 0.075x` (`pass`) |
| `scalar:persistent` | `384 ns` | `886 ns` | **`0.433x`** (`578.904x`) | `17.7 MiB` | `649.5 MiB` | **`0.027x`** (`1.135x`) | `pass` | `0.437x / 0.024x` (`pass`) |
| `scalar:fresh-evaluator` | `48.18 us` | `57.83 us` | **`0.833x`** (`1065.612x`) | `18.2 MiB` | `204.6 MiB` | **`0.089x`** (`0.087x`) | `pass` | `0.755x / 0.050x` (`pass`) |
| `stdlib-collections:cold-cli` | `22.65 ms` | `863.62 ms` | **`0.026x`** (`0.031x`) | `17.0 MiB` | `168.9 MiB` | **`0.101x`** (`0.117x`) | `pass` | `0.022x / 0.058x` (`pass`) |
| `stdlib-collections:persistent` | `45.62 us` | `230.64 us` | **`0.198x`** (`229.129x`) | `31.2 MiB` | `212.8 MiB` | **`0.147x`** (`2.029x`) | `pass` | `0.244x / 0.070x` (`pass`) |
| `stdlib-collections:fresh-evaluator` | `95.76 us` | `260.48 us` | **`0.368x`** (`440.034x`) | `18.1 MiB` | `245.2 MiB` | **`0.074x`** (`0.074x`) | `pass` | `0.297x / 0.042x` (`pass`) |
| `recursive-compute:cold-cli` | `27.72 ms` | `852.11 ms` | **`0.033x`** (`0.289x`) | `16.9 MiB` | `172.5 MiB` | **`0.098x`** (`0.103x`) | `pass` | `0.026x / 0.057x` (`pass`)* |
| `recursive-compute:persistent` | `888.96 us` | `12.04 ms` | **`0.074x`** (`29.101x`) | `17.7 MiB` | `197.6 MiB` | **`0.089x`** (`0.090x`) | `pass` | `0.098x / 0.048x` (`pass`)* |
| `recursive-compute:fresh-evaluator` | `941.57 us` | `12.14 ms` | **`0.078x`** (`32.125x`) | `17.7 MiB` | `205.8 MiB` | **`0.086x`** (`0.080x`) | `pass` | `0.095x / 0.041x` (`pass`)* |
| `large-output:cold-cli` | `25.64 ms` | `836.23 ms` | **`0.031x`** (`0.054x`) | `18.6 MiB` | `170.6 MiB` | **`0.109x`** (`0.185x`) | `pass` | `0.024x / 0.064x` (`pass`) |
| `large-output:persistent` | `1.38 ms` | `1.61 ms` | **`0.860x`** (`658.067x`) | `33.6 MiB` | `222.8 MiB` | **`0.151x`** (`0.935x`) | `pass` | `0.852x / 0.081x` (`pass`) |
| `large-output:fresh-evaluator` | `1.42 ms` | `1.87 ms` | **`0.762x`** (`459.976x`) | `20.5 MiB` | `252.5 MiB` | **`0.081x`** (`0.110x`) | `pass` | `0.668x / 0.049x` (`pass`) |
| `failure-diagnostics:cold-cli` | `23.72 ms` | `818.39 ms` | **`0.029x`** (`0.025x`) | `17.2 MiB` | `170.0 MiB` | **`0.101x`** (`0.105x`) | `pass` | `0.026x / 0.078x` (`pass`) |
| `failure-diagnostics:persistent` | `2.51 us` | `4.58 us` | **`0.547x`** (`125.223x`) | `17.9 MiB` | `242.4 MiB` | **`0.074x`** (`2.292x`) | `pass` | `0.195x / 0.052x` (`pass`) |
| `failure-diagnostics:fresh-evaluator` | `50.38 us` | `63.88 us` | **`0.789x`** (`1032.827x`) | `18.3 MiB` | `207.8 MiB` | **`0.088x`** (`0.093x`) | `pass` | `0.706x / 0.067x` (`pass`) |
| `import-graph:cold-cli` | `22.58 ms` | `803.03 ms` | **`0.028x`** (`0.027x`) | `17.0 MiB` | `168.0 MiB` | **`0.101x`** (`0.105x`) | `pass` | `0.025x / 0.075x` (`pass`) |
| `import-graph:persistent` | `1.05 us` | `1.16 us` | **`0.904x`** (`563.329x`) | `17.7 MiB` | `421.3 MiB` | **`0.042x`** (`1.564x`) | `pass` | `0.970x / 0.027x` (`pass`) |
| `import-graph:fresh-evaluator` | `180.20 us` | `205.38 us` | **`0.877x`** (`321.885x`) | `18.2 MiB` | `200.7 MiB` | **`0.091x`** (`0.090x`) | `pass` | `0.796x / 0.053x` (`pass`) |
| `reflection-stdlib:cold-cli` | `83.73 ms` | `888.84 ms` | **`0.094x`** (`0.094x`) | `74.6 MiB` | `171.6 MiB` | **`0.435x`** (`0.473x`) | `pass` | — |
| `reflection-stdlib:persistent` | `9.30 us` | `14.62 us` | **`0.636x`** (`51.603x`) | `131.7 MiB` | `275.6 MiB` | **`0.478x`** (`6.539x`) | `pass` | — |
| `renderers-files:cold-cli` | `120.99 ms` | `887.55 ms` | **`0.136x`** (`0.131x`) | `23.9 MiB` | `171.0 MiB` | **`0.140x`** (`0.145x`) | `pass` | — |
| `renderers-binary:cold-cli` | `40.35 ms` | `855.47 ms` | **`0.047x`** (`0.047x`) | `28.4 MiB` | `169.6 MiB` | **`0.167x`** (`0.181x`) | `pass` | — |
| `packages-http-warm:cold-cli` | `73.56 ms` | `1.385 s` | **`0.053x`** (`0.044x`) | `17.3 MiB` | `180.6 MiB` | **`0.096x`** (`0.100x`) | `pass` | — |
| `packages-http-cold:cold-cli` | `86.39 ms` | `1.468 s` | **`0.059x`** (`0.051x`) | `18.0 MiB` | `184.9 MiB` | **`0.097x`** (`0.101x`) | `pass` | — |
| `repl-session:cold-cli` | `34.19 ms` | `990.78 ms` | **`0.035x`** (`0.034x`) | `17.0 MiB` | `171.5 MiB` | **`0.099x`** (`0.103x`) | `pass` | — |
| `server-session:steady-server` | `79.97 us` | `657.67 us` | **`0.122x`** (`6.316x`) | `28.6 MiB` | `174.9 MiB` | **`0.164x`** (`0.301x`) | `pass` | — |
| `formatter-large:cold-cli` | `7.58 ms` | `338.77 ms` | **`0.022x`** (`0.020x`) | `3.1 MiB` | `83.8 MiB` | **`0.037x`** (`0.038x`) | `pass` | — |
| `pkldoc-single-package:cold-cli` | `164.77 ms` | `1.024 s` | **`0.161x`** (`4.625x`) | `81.7 MiB` | `183.6 MiB` | **`0.445x`** (`2.220x`) | `pass` | — |
| `napi-eval-text:napi` | `46.96 us` | `230.64 us` | **`0.204x`** (`222.437x`) | `75.5 MiB` | `212.8 MiB` | **`0.355x`** (`0.740x`) | `pass` | — |
| `napi-typed-conversion:napi` | `17.61 us` | `32.26 us` | **`0.546x`** (`73.554x`) | `140.0 MiB` | `211.4 MiB` | **`0.662x`** (`0.404x`) | `pass` | — |
| `wasm-node-eval-text:wasm-node` | `78.01 us` | `230.64 us` | **`0.338x`** (`439.736x`) | `141.1 MiB` | `212.8 MiB` | **`0.663x`** (`0.859x`) | `pass` | — |
| `wasm-node-typed-conversion:wasm-node` | `16.11 us` | `32.26 us` | **`0.499x`** (`100.128x`) | `201.1 MiB` | `211.4 MiB` | **`0.951x`** (`0.977x`) | `pass` | — |
| `wasm-browser-eval-text:wasm-browser` | `76.82 us` | `230.64 us` | **`0.333x`** (`550.429x`) | `1152.5 MiB` | `212.8 MiB` | **`5.415x`** (`4.997x`) | **`fail`** | — |
| `wasm-browser-typed-conversion:wasm-browser` | `17.34 us` | `32.26 us` | **`0.538x`** (`97.343x`) | `1179.5 MiB` | `211.4 MiB` | **`5.581x`** (`5.490x`) | **`fail`** | — |

*\*M0 GraalVM CE `25.0.1` note: as archived in `reports/m0/benchmarks.json`, the pinned GraalVM CE `25.0.1` runtime recorded `FrameWithoutBoxing` escape-related Tier-1 bailouts on `recursive.pkl` (`fib`/`run`); that limitation is preserved transparently in `m7-final-bench-02/report.json`.*

### JS/Wasm Lifecycle Breakdown & Preserved Browser-Tree RSS Failures

| Workload | Init (`wasm` init) | First Request | Steady Med / P95 | Host Peak RSS | Memory Boundary & Verdict |
| --- | ---: | ---: | ---: | ---: | --- |
| `napi-eval-text:napi` | `503.76 ms` (`—`) | `290.67 us` | `46.96 us` / `47.42 us` | `75.5 MiB` | Node `os.wait4` `ru_maxrss` (`pass`) |
| `napi-typed-conversion:napi` | `19.80 ms` (`—`) | `769.08 us` | `17.61 us` / `17.63 us` | `140.0 MiB` | Node `os.wait4` `ru_maxrss` (`pass`) |
| `wasm-node-eval-text:wasm-node` | `156.02 ms` (`64.86 ms`) | `1.36 ms` | `78.01 us` / `78.35 us` | `141.1 MiB` | Node `os.wait4` `ru_maxrss` (`pass`) |
| `wasm-node-typed-conversion:wasm-node` | `107.59 ms` (`14.52 ms`) | `4.15 ms` | `16.11 us` / `16.29 us` | `201.1 MiB` | Node `os.wait4` `ru_maxrss` (`pass`) |
| `wasm-browser-eval-text:wasm-browser` | `133.03 ms` (`49.30 ms`) | `1.43 ms` | `76.82 us` / `77.19 us` | `1,152.5 MiB` (driver `322.5 MiB`) | Chromium descendant-tree `/bin/ps` RSS, `cross_origin_isolated: true` (**`fail`: `5.415x > 1.00`**) |
| `wasm-browser-typed-conversion:wasm-browser` | `110.71 ms` (`34.05 ms`) | `2.03 ms` | `17.34 us` / `17.75 us` | `1,179.5 MiB` (driver `349.7 MiB`) | Chromium descendant-tree `/bin/ps` RSS, `cross_origin_isolated: true` (**`fail`: `5.581x > 1.00`**) |

- **Exact Benchmark & Release Gate Exits**:
  - `tools/m7_benchmark.py --verify-report target/m7/m7-final-bench-02`: **Exit `0`**
  - `tools/m7_benchmark.py --verify-strict target/m7/m7-final-bench-02`: **Exit `1`** (strictly rejects the 2 Chromium browser-tree RSS rows `> 1.00x` without relaxing thresholds)
  - `tools/m7_manifest.py --verify-release target/m7 --benchmark-dir target/m7/m7-final-bench-02 --soak-dir target/m7/m7-final-soak-03 --fuzz-dir target/m7/m7-final-fuzz-03`: **Exit `1`** (`integrity_passed: true`, `soak_strict_passed: true`, `fuzz_strict_passed: true`, `benchmark_strict_passed: false`, `local_strict_gates_passed: false`, `release_ready: false`, `non_bounded_soak_runs: []`, `non_pass_fuzz_cases: []`, `unmet_performance_workloads_m0_graalvm_25_0_1: []`, `verified_platforms: []`, `observed_non_passing_platforms: ["browser-chromium", "darwin-arm64"]`, `unverified_platforms: ["darwin-arm64", "darwin-x64", "linux-x64-gnu", "linux-arm64-gnu", "win32-x64-msvc", "browser-chromium", "browser-firefox", "browser-webkit", "hosted-ci"]`).

---

## Long-Running Memory Reclamation Soak (`target/m7/m7-final-soak-03`)

- **Sealed Report**: `target/m7/m7-final-soak-03/report.json` (`6d6bc0adb9e359cb6eb3ac897043668605acef3bbc17b9e18b18b32881158e84`), `sources.snapshot.json` (`57d8a84092d7fcc1cc4a0504c4735b5c694b9a1d10b3dff5bd60b5e51d1834b9`, `source_stability: true`, `307` source files).
- **Verifiers**: `--verify-report` **Exit `0`**, `--verify-strict` **Exit `0`** (**`18 / 18 bounded`**, `0` leaks, `0` FD leaks, `0` orphan child PIDs).
- **Remediation History (Original Failures Preserved)**:
  1. `target/m7/soak-baseline-02` (`45a0b6c1c47ef9b75725eeec48b9ccca7b9e58afb91458f504d6d6e93c3e403c`): **`7 / 18 bounded`, `11 / 18 leak-detected`** (unbounded persistent heap growth across `repeated-success`, `repeated-failure`, `unique-sources-churn`, and `cyclic-graphs` up to `+504.97 MiB` / `8.648x` on `native/cyclic-graphs` and `+1,876` Wasm pages on `wasm-node/cyclic-graphs`). Fixed by automatic inter-op tracing GC and uncached URI alias bounding.
  2. `target/m7/m7-final-soak-01` (`3b8f4bcd231a3df2341145e4963057699b7c22f56a9992c7afc75cdf7e5ae57a`): **`16 / 18 bounded`, `2 / 18 leak-detected`** (`native/cyclic-graphs` `sh_slope=+9,404.4 B/iter`, `wasm-node/unique-sources-churn` `w_slope=+3,813.0 B/iter`). Root cause: sparse monotonic block-ID-indexed `Vec`s in `ExecutableRoots::local` and `Vm::fast_int_cache` plus deferred Wasm handle disposal. Fixed in `#142` via compact GC-shrinking Fibonacci-hashed tables, `.remove()` staged-source cleanup, and `_disposeWasmIfClosed()`.
  3. `target/m7/m7-final-soak-02` (`dc09f0eec0caa16742de685cc6227c3681b6834f25ac632919ee5873b06a7fc4`): **`18 / 18 bounded` STRICT PASS** (`wasm-node` peak RSS `182–206 MiB`, `0` second-half Wasm page growth).
  4. Final frozen **`target/m7/m7-final-soak-03`**: **`18 / 18 bounded` STRICT PASS**, with V8 Liftoff outlining additionally reducing `wasm-node` peak RSS by ~44–62 MiB (`133.8–148.4 MiB`) while maintaining `0` second-half Wasm page growth across all 6 `wasm-node` scenarios.

### Complete 18-Run Soak Table (`target/m7/m7-final-soak-03`, `--profile standard`)

| Target / Scenario | Iters (Warmup) | RSS: Base -> Mid -> Final (MiB) | Growth Ratio | 2nd-Half Delta / Slope / R² | Wasm Pages (Base->Mid->Final; 2nd-Half) | FDs (Base->Final) | Orphans | Verdict |
| --- | :---: | ---: | ---: | ---: | --- | :---: | :---: | :---: |
| `native/repeated-success` | `1500` (`150`) | `25.92 -> 26.19 -> 26.28` | `1.014x` | `+96 KiB` / `+128.0 B/it` / `0.967` | — | `3 -> 3` | `0` | `bounded` |
| `native/repeated-failure` | `1500` (`150`) | `19.23 -> 19.45 -> 19.56` | `1.017x` | `+112 KiB` / `+168.5 B/it` / `0.880` | — | `3 -> 3` | `0` | `bounded` |
| `native/unique-sources-churn` | `500` (`50`) | `20.36 -> 21.50 -> 21.52` | `1.057x` | `+16 KiB` / `+46.8 B/it` / `0.429` | — | `3 -> 3` | `0` | `bounded` |
| `native/cyclic-graphs` | `1000` (`100`) | `36.69 -> 36.95 -> 37.28` | `1.016x` | `+336 KiB` / `+538.3 B/it` / `0.585` | — | `3 -> 3` | `0` | `bounded` |
| `native/lifecycle-cancel-timeout-close` | `120` (`12`) | `18.45 -> 18.75 -> 18.75` | `1.016x` | `0 B` / `0.0 B/it` / `0.000` | — | `3 -> 3` | `0` | `bounded` |
| `native/cache-churn` | `300` (`30`) | `19.02 -> 19.22 -> 19.23` | `1.012x` | `+16 KiB` / `+78.0 B/it` / `0.429` | — | `3 -> 3` | `0` | `bounded` |
| `napi-node/repeated-success` | `1000` (`100`) | `67.09 -> 67.72 -> 68.13` | `1.015x` | `+416 KiB` / `+795.8 B/it` / `0.909` | — | `12 -> 12` | `0` | `bounded` |
| `napi-node/repeated-failure` | `1000` (`100`) | `62.91 -> 63.48 -> 66.05` | `1.050x` | `+2,624 KiB` / `+6,052.7 B/it` / `0.727` | — | `13 -> 12` | `0` | `bounded` |
| `napi-node/unique-sources-churn` | `400` (`40`) | `64.41 -> 69.66 -> 69.92` | `1.086x` | `+272 KiB` / `+1,556.5 B/it` / `0.919` | — | `12 -> 12` | `0` | `bounded` |
| `napi-node/cyclic-graphs` | `600` (`60`) | `79.45 -> 80.47 -> 81.28` | `1.023x` | `+832 KiB` / `+2,652.6 B/it` / `0.878` | — | `12 -> 12` | `0` | `bounded` |
| `napi-node/lifecycle-cancel-timeout-close` | `100` (`10`) | `62.30 -> 63.03 -> 63.19` | `1.014x` | `+160 KiB` / `+3,651.3 B/it` / `0.952` | — | `13 -> 12` | `0` | `bounded` |
| `napi-node/cache-churn` | `240` (`24`) | `66.45 -> 67.38 -> 67.58` | `1.017x` | `+208 KiB` / `+1,931.0 B/it` / `0.941` | — | `12 -> 12` | `0` | `bounded` |
| `wasm-node/repeated-success` | `600` (`60`) | `142.83 -> 145.97 -> 146.47` | `1.025x` | `+512 KiB` / `+1,919.3 B/it` / `0.970` | `194 -> 201 -> 201` (`+0`, `0.0 B/it`) | `12 -> 12` | `0` | `bounded` |
| `wasm-node/repeated-failure` | `600` (`60`) | `136.52 -> 147.73 -> 148.38` | `1.087x` | `+656 KiB` / `+2,192.3 B/it` / `0.646` | `154 -> 157 -> 157` (`+0`, `0.0 B/it`) | `12 -> 12` | `0` | `bounded` |
| `wasm-node/unique-sources-churn` | `300` (`30`) | `134.17 -> 136.94 -> 137.19` | `1.022x` | `+256 KiB` / `+1,279.5 B/it` / `0.330` | `159 -> 177 -> 177` (`+0`, `0.0 B/it`) | `12 -> 12` | `0` | `bounded` |
| `wasm-node/cyclic-graphs` | `400` (`40`) | `146.73 -> 147.59 -> 148.11` | `1.009x` | `+528 KiB` / `+2,504.4 B/it` / `0.963` | `320 -> 327 -> 327` (`+0`, `0.0 B/it`) | `13 -> 12` | `0` | `bounded` |
| `wasm-node/lifecycle-cancel-timeout-close` | `100` (`10`) | `131.44 -> 132.31 -> 133.83` | `1.018x` | `+1,552 KiB` / `+38,619.4 B/it` / `0.857` | `146 -> 149 -> 149` (`+0`, `0.0 B/it`) | `13 -> 12` | `0` | `bounded` |
| `wasm-node/cache-churn` | `240` (`24`) | `136.22 -> 137.27 -> 137.77` | `1.011x` | `+512 KiB` / `+4,661.6 B/it` / `0.975` | `152 -> 155 -> 155` (`+0`, `0.0 B/it`) | `13 -> 12` | `0` | `bounded` |

- **Direct `SourceSession` GC Probe (`native/cyclic-graphs` `heap_probe`, `160` iterations + `20` warmup)**: `assertions_ok: true` — `live` objects steady at `122 -> 122 -> 122`, `executable_live` steady at `16 -> 16 -> 16`, `slots` plateaued at `563 -> 861 -> 861`, `reclaimed` objects increased monotonically `438 -> 2,278 -> 4,118`, `collections` increased `1 -> 4 -> 7`.

---

## Seeded Differential Fuzzing (`target/m7/m7-final-fuzz-03`)

- **Sealed Report**: `target/m7/m7-final-fuzz-03/report.json` (`5623029f8c04119e7e91944c0ce2b9477aead9ed2c57f950970b517321d50896`), `sources.snapshot.json` (`2fa329c2a4ad7b7580e619270a3085ff27e6e23a1cfa8f7cd9d44f20c5d5fa76`, `source_stability: true`, `340` source files).
- **Verifiers**: `--verify-report` **Exit `0`**, `--verify-strict` **Exit `0`** (**`215 / 215 PASS`**, `0` mismatches, `0` errors, `0` minimized).
- **Seed & PRNG**: `0x4d37504b4c303332` (`splitmix64`), `210` generated cases + `5` permanent saved regressions.
- **Remediation History**: Phase 1 baseline `target/m7/fuzz-baseline-01` (`561be167560bffcc6869c8744435921a05b2d8e6aff4f8c865367fc5cbe674da`) uncovered `19` raw mismatches out of `210` (`191 pass / 19 mismatch`, `19` minimized) collapsing into 4 distinct root bugs (`reg_01`–`reg_04`), plus a 5th live regression (`reg_05`) caught during Wave 2 optimization; all 5 were fixed and permanently verified across `m7-final-fuzz-01` (`e77745a8...`), `m7-final-fuzz-02` (`7ce36d43...`), and final frozen `m7-final-fuzz-03` (`5623029f...`).

### Per-Category Outcomes (`215 / 215 PASS`)

| Category | Cases | Pass | Mismatch / Error | Scope Verified |
| --- | :---: | :---: | :---: | --- |
| `expressions-numerics` | `30` | `30` | `0 / 0` | Checked integer, float, boolean, control-flow, and arithmetic/error raw parity |
| `strings-regex-glob` | `30` | `30` | `0 / 0` | Unicode, escapes, interpolation, `Regex`, and glob raw parity |
| `objects-amendments-generators` | `32` | `32` | `0 / 0` | `30` generated + `reg_01`, `reg_02`: amendments, `for`/`when`, predicates, scopes |
| `collections-units` | `31` | `31` | `0 / 0` | `30` generated + `reg_05`: `List`/`Set`/`Map`/`Listing`/`Mapping`/`Duration`/`DataSize` |
| `types-closures` | `30` | `30` | `0 / 0` | Generics, unions, constraints, lambdas, closures, type errors |
| `multi-format-rendering` | `32` | `32` | `0 / 0` | `30` generated + `reg_03`, `reg_04`: all 7 render formats (`pcf`, `json`, `yaml`, `xml`, `properties`, `plist`, `pkl-binary`) + `m3_pklbinary_decode_probe` round-trip |
| `formatter-fuzz` | `30` | `30` | `0 / 0` | `pkl format -` raw stdout/stderr/exit parity vs Java + 2nd-pass idempotency |

### Permanent Saved & Fixed Fuzz Regressions (`tests/fixtures/m7/fuzz-regressions/`)

| Regression ID | Category | Discrepancy Found & Fixed (Rust Test: `crates/pkl-runtime/tests/m7_fuzz_and_gc_regressions.rs`) |
| --- | --- | --- |
| `reg_01_top_level_outer` | `objects-amendments-generators` | Aligned stderr message to Java 0.32.0: `"Top-level scope does not have an outer scope."` (was `"No outer scope is available."`) |
| `reg_02_predicate_method_not_found` | `objects-amendments-generators` | Aligned predicate-member missing-method stderr to Java 0.32.0: `"Cannot find method \`startsWith\`."` without extraneous `Dynamic` class/available-methods suffix |
| `reg_03_lexer_before_parser_escape` | `multi-format-rendering` (`yaml`) | Aligned parser-vs-lexer diagnostic ordering so earlier parser error at `and` precedes later invalid escape `\ ` |
| `reg_04_lexer_before_parser_missing_quote` | `multi-format-rendering` (`xml`) | Aligned parser-vs-lexer diagnostic ordering so earlier parser error at `chars` precedes later missing `"` delimiter at EOL |
| `reg_05_list_constructor_order` | `collections-units` | Caught live during M7 optimization: fixed `List(1, 2, 3)` element reversal (`List(3, 2, 1)`) in compact `list_elements` construction |

---

## Frozen-Source Cross-Milestone Verification (`#185`)

All checks below were executed read-only on the exact frozen `#154`/`#156` tree on local macOS `arm64` using pinned Temurin `21.0.13.1+2-LTS` (`exact_jdk_lock_match: true`, NO `PKL_ALLOW_LOCAL_JDK=1`):

| Verification Target | Command / Evidence Path | Exit | Observed Outcome & Sealed SHA-256 |
| --- | --- | :---: | --- |
| **Static, Lint, Wasm32 & Oracle** | `cargo xtask verify`, `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo clippy -p java_regex --lib --locked -- -D warnings`, `cargo clippy -p pkl-wasm --target wasm32-unknown-unknown --locked -- -D warnings`, `cargo check -p pkl-runtime -p pkl-formatter -p pkl-wasm --target wasm32-unknown-unknown --locked`, `tools/oracle.py verify` | `0` (all) | Zero warnings, `unsafe_code = "deny"`, `Reference pins verified for Pkl 0.32.0.`, `exact_jdk_lock_match: true` |
| **All 12 Manifests & 2 Benchmark Contracts** | `tools/{inventory,discover,smoke_manifest,regression_manifest,m2_manifest,m2_module_eval,m2_jvm_classpath,m3_manifest,m4_manifest,m5_manifest,m6_manifest,m7_manifest}.py --check` + `tools/{benchmark,m7_benchmark}.py --check-contract` | `0` (14/14) | All 12 manifests and both benchmark contracts verified current |
| **Full Rust Workspace Tests (Honest Loopback Split)** | Sandboxed `cargo test --workspace --exclude pkl-host-native --locked` (`374` targets, `2,463 passed; 0 failed; 2 ignored`; plus `13` nested subprocess summaries [`22` child passes; `2,485` raw `test result:` sum]) + sandboxed `-p pkl-host-native` (Exit `101`, `5/8` solely on 3 loopback tests in `tests/m4_native_host_io.rs`) + unsandboxed `cargo test -p pkl-host-native --locked` (Exit `0`, `4` targets, `8/8`) | `0` (combined) | **Combined: `378` outer targets (`369` test + `9` doc-tests), `2,471 passed; 0 failed; 2 ignored`** (`frontend_probe`, `java04_strict_full_compatibility_gate`) |
| **Full Python Unit-Test Suite (`132` Modules, `1,294` Tests)** | All `tools/tests/test_*.py` with pinned `PKL_ORACLE_JAVA`: Batch A (`57` core + M1–M7 modules, `510` tests) + Batch B (`75` historical fixture/compare modules, `784` tests) | `0` (active) / `1` (2 hist) | **Batch A**: **`57/57` active modules (`510/510` PASS)** after clean standalone retry of `test_pantry` (`5/5` in `0.189s` following one transient `100ms` CPU-load flake). **Batch B**: **`73/75` PASS (`775/784`)**; exact **`2` pre-existing Stage-65 stale-`oracle.lock.json` modules FAIL (`7 FAIL, 2 ERROR = 9` non-pass)**: `test_inferred_new_default_diagnostics_fixtures.py` & `test_inferred_new_method_cache_fixtures.py` (tarballs assert old Temurin `21.0.12.1+1-LTS` SHA `7b77395ff5b6...` vs current `035fa94a2706...`). **Combined: `130/132` modules (`1,285/1,294` tests) PASS**. |
| **M1 Multi-Target Regression** | `target/m7/m7-final-m1-02` (unsandboxed; preserves sandboxed `-01` Exit `2` `net::ERR_ACCESS_DENIED`) | `0` | **`6,078 / 6,078`** (`rust`, `node v26.13.1`, Chromium `156.0.8078.4`), `92/92` scenarios, `browserErrors: []`, `source_stability: true` (`476` files). `report.json`: `dcec9d8dd6c970ae5e36f17c66c3b1b68e246e290f017f28b0fd69ec169a206f`; `sources.json`: `aa938703932efa892f2f1fa659078d94991cc92545767d7200cb811a59748097` |
| **Full M2 Language Replay (`4,575` Ready Cases)** | `target/m7/m7-final-m2-full-01` (unsandboxed; preserves sandboxed `m7-final-m2-sandbox-check-01` Exit `1`): `--record-only` (incl. built-in `check_report()`) + single-process `--verify-development` | `0` (2/2) | **`4,574 pass / 1 oracle-error (error-cycle) / 0 mismatch`** across all 7 adapters (`language-api-string: 3,585+1`, `language-module-eval: 888`, `package-project-eval: 68`, `historical-cli: 21`, `historical-collections: 8`, `selected-regression: 3`, `jvm-classpath: 1`), `88/88` metamorphic, `source_stability: true` (`246,731` paths), `artifact_stability: true`, `532,320` sealed files, all 3 candidates `recorded-build`. Standalone `--verify-report` and `--verify-full` were not re-run as redundant 532k-file passes (`#182`); `report.json` honestly records `full_gate_failures: ["requirement execution incomplete: semantic:lazy-evaluation", "required case incomplete: error-cycle"]` (`2` failures). `report.json` (`157,066,172 B`): `0ee17152fa94217ea23756fbb08bf285438eb9363f1d5b2f0f2aa7f7ebb07637`; child `jvm-classpath/report.json`: `c59fa992e7d4054c09a97be0bcf95f3cbbfe1dcf365e0f325ab58ccbfd639a86` |
| **M2 Protobuf Strict Gate** | `target/m2/m7-final-protobuf-strict` | `101` (contracted) | **`9.14s`** (vs `535.96s` in Stage 161): `266/266` attempted, `0` harness failures, `240/240` regular, `8/8` pre-renderer, exact `18` `REFERENCE-NONPASS`, `0` byte diffs (`strict_compatible: false`, `experiment_contract_pass: true`). `contract.json`: `f34aa558bfb638cbd671fc7635eb55a5f248f07560b82f35bce49324aa856187`; `strict-failures.txt`: `4370fbabe1fbeca5bd76f5dc055e55134ea2c9411498d10f334744131ade448b`; `native-outcomes.tsv`: `3cb92cd07b321cc6fa7a73a18f99e0b37d46221e24d3c59e0bf4b49b28169dae` |
| **M3 Stdlib Census + 3-Lane Diff** | `target/m7/m7-final-m3-census.json` & `target/m7/m7-final-m3-diff` (sandboxed) | `0` (all) | Census **`1,781 / 1,781`** (`failure_count: 0`, `stdlib: 1214 + 8 deferred`, `builtin: 542`, `renderer: 18`); Diff **`118 / 118 pass`** (`formats-renderers: 77`, `output-files: 13`, `pklbinary-decode: 28`, `749` sealed files, `--verify-report` + `--verify-strict` Exit `0`). Census: `4d796db1241ae507f7f343274ff6d7ee909aa401aa8b27bd7142b0842079008e`; diff `report.json`: `301e1c552ce5666e775e530de45ee17cfb7a416f486b047c3486e23da821183f` |
| **Full M4 All 5 Lanes (incl. All 29 Pantry Packages)** | `target/m7/m7-final-m4-all5-02` (unsandboxed; preserves sandboxed `-01` Exit `1` `EPERM`) | `0` (all) | **`429 / 429 pass`** (`host-io-http-security: 48`, `external-readers: 11`, `projects-packages-cache: 27`, `pantry-dev: 126`, `pantry-packaged: 217` incl. `5/5` cold HTTPS mirror), `16/16` reqs, `3,049` sealed files, `--verify-report` + `--verify-strict` + `--verify-coverage` ALL Exit `0`. `report.json`: `e5da4bf1540bcc4b471ab7428fee6bb1c621717ad13353b9ef96dd26336bebc1`; `sources.snapshot.json`: `8db313e6a2f1654f2c0711e704fb61758fe559af7e9e8ea143dfbc8d007a77ac` |
| **Full M5 All 5 Lanes** | `target/m7/m7-final-m5-all5-02` (unsandboxed; preserves sandboxed `-01` Exit `1` `EPERM`) | `0` (all) | **`142 / 142 pass`** (`rust-api-and-cli: 50`, `repl: 12`, `server: 22`, `formatter: 43`, `pkldoc: 15`), `16/16` reqs, `4,453` sealed files, all 3 candidates `recorded-build`, `--verify-report` + `--verify-strict` + `--verify-coverage` ALL Exit `0`. `report.json`: `30d266e58e27d8646be569c449044791c5f7e8f5620ff6b42e47564d8ac35983`; `sources.snapshot.json`: `09e7764ee7eccee604bf8dd4a095fa77c9974b5fad58ed13a915c6fa47077c6e` |
| **Full M6 All 5 Lanes** | `target/m7/m7-final-m6-all5-02` (unsandboxed; preserves sandboxed `-01` Exit `1` `EPERM`) | `0` (all) | **`80 / 80` executable PASS + `6 / 6` `capability-blocked`** (`wasm-browser: 29+4`, `wasm-node: 8+2`, `napi-node: 26`, `cross-target-parity: 12`, `packaging-and-install: 5`), `14/14` reqs, `834` sealed files, all 4 candidates `recorded-build`, `--verify-report` + `--verify-strict` + `--verify-coverage` ALL Exit `0`. Conversion telemetry (`398 B`): N-API median `20,750 ns` (mean `22,793 ns`, `25` iters; down from `35,583 ns` in M6); Wasm Browser median `0 ns` / mean `10,000 ns` / p95 `100,000 ns` (`20` iters; down from `100,000 ns` median). `report.json`: `c0124d3f297bcd5655c2d6b0fae048c734f38ca472ae2effc650ab1100ff6f50`; `sources.snapshot.json`: `890f4720a228e71db0e81f4a3f23dc195507131731c9f896ea037bc4df8865d0` |

---

## CI Workflow Updates (`.github/workflows/ci.yml`)

Updated `.github/workflows/ci.yml` (`23,648 B`, SHA-256 `b95958a6b5b1e23a80b90302df59f537f458fb85a1c020d3c34d5226dba42a1c`, validated locally with Ruby YAML parser) to retain all 4 existing jobs and wire 5 new milestone jobs (**9 jobs total**):

1. `compatibility-tooling` (preserved M0 inventory, discovery, smoke, regressions, benchmark contract, Java replay)
2. `m1-experiments` (preserved M1 frontend/runtime/Host/lifecycle/platforms contract & Graal evidence check)
3. `workspace` (preserved cross-OS Rust `xtask verify`, `fmt`, `clippy`, `test`, `wasm32` check)
4. `m2-language` (preserved M2 inventory, differential recording, development gate, and strict full gate)
5. `m3-stdlib-output` (NEW: M3 manifest/unit tests, 1,781-item mechanical coverage probe, 118-case 3-lane differential gate)
6. `m4-io-packages` (NEW: M4 manifest/unit tests, offline Lanes 1–3 on push/PR and all 5 Pantry lanes on `full_corpus`)
7. `m5-product-tools` (NEW: M5 manifest/unit tests, 142-case 5-lane offline differential & coverage gate)
8. `m6-wasm-napi` (NEW: pinned Playwright/wasm-bindgen install, M6 manifest/Wasm lint/unit tests, 86-case 5-lane differential & packaging gate)
9. `m7-release-gates` (NEW: OS matrix checking all 12 milestone manifests + M7 unit tests, seeded differential fuzzer, native memory soak, and full M7 benchmark suite with evidence upload)

*Note*: `.github/workflows/ci.yml` has been validated locally only; no hosted GitHub Actions run has been executed yet (`hosted-ci` remains unverified).

---

## Unverified Release Scope, `release_ready: false`, and Retained Exceptions

1. **Why `release_ready` Is Honestly `false` (`m7_manifest.py --verify-release` Exit `1`)**:
   - **2 / 34 Benchmark RSS Failures (Structural Chromium Multi-Process Tree)**: `wasm-browser-eval-text:wasm-browser` (`1,152.5 MiB` Chromium tree vs `212.8 MiB` JVM = **`5.415x`**) and `wasm-browser-typed-conversion:wasm-browser` (`1,179.5 MiB` vs `211.4 MiB` = **`5.581x`**). Both beat Java 2x–3x on execution time (`0.333x`, `0.538x`), and all `32/32` non-browser workloads (Native, N-API, Wasm-Node) pass both time and `os.wait4` peak RSS `<= 1.00x`. Because `INTENT.md` and `TESTING.md` forbid silently relaxing the `<= 1.00` RSS threshold or hiding per-workload failures, these 2 rows stay recorded as `fail`.
   - **Unverified Platform, Browser & CI Matrix (`0 / 9` Release Matrix Targets Verified Passing)**:
     - `observed_non_passing_platforms`: `darwin-arm64`, `browser-chromium` (observed locally, failing solely due to the 2 Chromium browser-tree RSS rows above).
     - `unverified_platforms` (not yet executed): `darwin-x64`, `linux-x64-gnu`, `linux-arm64-gnu`, `win32-x64-msvc`, `browser-firefox`, `browser-webkit`, `hosted-ci` (plus the 2 non-passing local platforms above).
2. **Unpinned Optional Reference & Explicit `2026-11-20` Scope Exclusions**:
   - Official upstream Pkl `native-image` CLI binary (`native-image-reference`) is not pinned in `oracle.lock.json` (`unverified-not-pinned`) and never replaces the warmed JVM reference.
   - `jvm-adapter` (`java_api: 8,239`, `kotlin_api: 597`, `jvm_spi: 2`), `pkl-codegen-java`, and `pkl-codegen-kotlin` remain explicitly excluded per `2026-11-20` scope decisions in [INTENT.md](../../INTENT.md) and [PLAN.md](../../PLAN.md).
3. **Retained Prior-Milestone & Historical Exceptions**:
   - **M2**: `error-cycle` `oracle-error` (`full_gate_failures: 2`; standalone `--verify-full` not re-run separately in Step 3 per `#182`), `18` upstream Protobuf `REFERENCE-NONPASS` slots (`target/m2/m7-final-protobuf-strict` contracted Exit `101`, `0` byte diffs on projected channels), and `11` `required: false` `rev1` unmapped entries.
   - **Historical Python Suites**: `2` pre-existing Stage-65 fixture modules (`test_inferred_new_default_diagnostics_fixtures.py` and `test_inferred_new_method_cache_fixtures.py`, `9` non-pass tests out of `1,294` total) whose archived tarballs assert the older Temurin `21.0.12.1+1-LTS` `oracle.lock.json` SHA-256 (`7b77395ff5b6...`) rather than current `21.0.13.1+2-LTS` (`035fa94a2706...`).
   - **M4 / M5 / M6 / M0 Contracts**: Pure-Rust Wasm DEFLATE vs OpenJDK C zlib on non-empty `package-archive` ZIPs (`raw_zip_bytes_match` recorded alongside live Java cross-interop), `3` M5 `native-version` cases, `9` M5 `pkldoc-progress` stdout nondeterminism cases (`0` file/symlink/stderr/exit diffs), `6/6` M6 Wasm `capability-blocked` boundary cases (never folded into `compatibility_pass_count`), and M0 GraalVM CE `25.0.1` Tier-1 `FrameWithoutBoxing` bailouts on `recursive-compute`.
