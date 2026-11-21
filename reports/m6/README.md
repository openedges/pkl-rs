# M6 Wasm and N-API Product Completion — Development-Closure Checkpoint & Frozen-Source Verification

This canonical checkpoint report records the Milestone **M6 (Wasm and N-API Product Completion)** implementation tracks, unified JS/TS API contract, distributable npm packaging, manifest linking, and complete frozen-source verification against **Apple Pkl 0.32.0** using the exact pinned **Eclipse Adoptium Temurin `21.0.13.1+2-LTS`** JDK (`exact_jdk_lock_match: true`, without `PKL_ALLOW_LOCAL_JDK=1`).

All evidence in this report is **local macOS `arm64` (`darwin-arm64`) verification**, not hosted CI. Milestone M6 completes the full product WebAssembly (`crates/pkl-wasm`, `packages/pkl-wasm` as `@pkl-rs/wasm@0.32.0`) and Node.js N-API (`crates/pkl-napi`, `packages/pkl-napi` as `@pkl-rs/napi@0.32.0`) bindings on top of the unchanged shared pure-Rust Apple Pkl 0.32.0 engine (`unsafe_code = "deny"`, zero production Java fallback, zero modifications to `crates/{pkl-runtime,pkl-host-native,pkl,pkl-cli,pkl-doc,pkl-formatter,pkl-repl,pkl-server}` or `external/`), preserves 100% of existing M1 experimental exports, and closes all Wave 3A/3B frozen-source gates: the 14-requirement / 83-link / 86-case 5-lane offline Java 0.32.0 differential and packaging gate (`target/m6/m6-full-strict-03`: **`80/80` executable PASS + `6/6` `capability-blocked`** across `wasm-browser` [`29/29` pass + `4/4` `capability-blocked`], `wasm-node` [`8/8` pass + `2/2` `capability-blocked`], `napi-node` [`26/26` pass], `cross-target-parity` [`12/12` pass across 4 Rust/JS targets vs Java 0.32.0], and `packaging-and-install` [`5/5` pass]; `--verify-report`, `--verify-strict`, and `tools/m6_manifest.py --verify-coverage` all Exit `0`), M1 multi-target regression verification (`target/m6/m6-m1-check-02`: **`6,078/6,078`** across Native, Node N-API, and Chromium Wasm with `92/92` scenarios and `browserErrors: []`), all static/Wasm/oracle/freshness prechecks (**11/11** Python `--check` scripts Exit `0`), the full Rust workspace suite (**`2,472` outer passed across `378` targets/doctests [`2,485` raw `test result:` passes including `13` nested child passes]; `0` failed; exact `2` ignored**, with the macOS sandbox loopback split and transient CPU-load retry documented transparently), **53** Python unit-test modules (**`492/492` PASS**), and standalone package smoke/install tests (`packages/pkl-wasm` `9/9`, `packages/pkl-napi` `11/11` + pack-install), while preserving all capability boundaries, platform coverage limits, conversion telemetry scope, and prior-milestone exceptions honestly.

---

## Summary of M6 Implementation Tracks

### 1. Unified Public JS/TS API Contract & Capability Honesty (`compatibility/m6-wasm-napi.json`)
- **Shared Cross-Target Surface (`19` common exports, `0.32.0`)**: Both `@pkl-rs/wasm` and `@pkl-rs/napi` expose the same ergonomic async JS/TS API and TypeScript declarations (`index.d.ts`): `PKL_VERSION`, `capabilities`, `createEvaluator`, `ModuleSource`, `PklError`, `decodePklBinary`, `PklDuration`, `PklDataSize`, `PklPair`, `PklRegex`, `PklObject`, `PklClass`, `PklTypeAlias`, `PklIntSeq`, `PklFunction`, `PklReference`, `format`, `createRepl`, and `analyzeImportGraph`, plus target-specific exports (`init`, `VirtualFileSystem` in `@pkl-rs/wasm`; `Project` in `@pkl-rs/napi`).
- **Explicit 19-Key Capability Matrix (`capabilities()`)**: Declares exact runtime capabilities (`vfs`, `asyncModuleLoader`, `asyncResourceLoader`, `nativeFs`, `nativeHttp`, `externalReaders`, `modulePathJar`, `projectResolution`, `packageCache`, `formatter`, `repl`, `schema`, `analyzeImports`, `bigint64`, `uint8ArrayBytes`, `clock64Bit`, `platform`, `runtime`, `arch`). Missing host capabilities (for example, OS filesystem, native TLS/HTTP sockets, `modulepath:` JARs, or OS external-reader subprocesses in Wasm) deterministically throw structured `PklError` with `code: "capability-unavailable"`, never silent skips.

### 2. Full Product Wasm Bindings & Distribution (`crates/pkl-wasm`, `packages/pkl-wasm`)
- **Rust Wasm Product Layer (`crates/pkl-wasm/src/product/{mod,util,decode,schema,host_core,evaluator,repl,analyzer,test_runner}.rs`)**: Preserves 100% of existing M1 exports in `crates/pkl-wasm/src/lib.rs` (+2 lines `pub mod product;`), compiles cleanly for `wasm32-unknown-unknown` with `0` warnings and `unsafe_code = "deny"`, and binds `pkl-runtime` + `pkl-formatter` (pure workspace path dependencies, zero new crates.io dependencies) across all 23 `pkl:*` stdlib modules, all 542 external builtins, all 9 output renderers (`pcf`, `json`, `yaml`, `xml`, `properties`, `plist`, `jsonnet`, `textproto`, `pkl-binary`), `output.text`, `output.bytes` (`Uint8Array`), `output.files`, expression evaluation (string & `pkl-binary`), `evaluateSchema`, `analyzeImportGraph` (with full security-checked `pkl:*` stdlib resolution), `pkl:test` (`spec`/`minimal` reports with colored summary), `format(source, grammarVersion)`, `decodePklBinary(bytes)`, and stateful `WasmReplSession` (`eval`, `load`, `reset`, `complete`, `close`).
- **Cooperative Async Host, 64-Bit Clock & Lifecycle**: Suspends cleanly on typed Host requests (`ReadModule`, `ListModules`, `ReadResource`, `ListResources`, `HttpGet`, `PackageMetadata`, `PackageArchive`, `Log`, `TransformFrame`) while enforcing `SecurityPolicy` (`allowedModules`, `allowedResources`, `rootDir`) inside `pkl-runtime`; removes M1's 49.7-day `u32` clock ceiling via validated 64-bit monotonic timestamps (`u64` / safe integer ms); supports exact signed `i64` `BigInt` (`i64::MIN..=i64::MAX`), owned `Uint8Array` copies, lossless UTF-16 `PklString`, idempotent `close()`, `cancel()`, and `timeoutMs` draining active Host operation IDs to abort pending `AbortController` / timer resources without leaks.
- **Self-Contained Dual Browser + Node ESM Package (`packages/pkl-wasm/`, `@pkl-rs/wasm@0.32.0`)**: Provides `VirtualFileSystem` (in-memory files, directories, path normalization, `rootDir` confinement, glob listing, `file:///` and custom scheme mounting), async module/resource/HTTP (`fetch`) loaders, pure-Wasm `package://` ZIP archive support, and `scripts/build.mjs` invoking pinned `wasm-bindgen 0.2.108` to produce a 13-file `npm pack` tarball working identically in real Chromium ESM (`init(urlOrBytes)`) and Node.js (auto-loading bundled `.wasm` bytes).

### 3. Full Product N-API Bindings & Distribution (`crates/pkl-napi`, `packages/pkl-napi`)
- **Rust N-API Product Layer (`crates/pkl-napi/src/{bridge,error,product,repl,schema,values}.rs`)**: Preserves 100% of existing M1 exports in `crates/pkl-napi/src/lib.rs`, links the shared `pkl-runtime`, `pkl-host-native`, `pkl`, and `pkl-formatter` crates (`unsafe_code = "deny"`), and executes asynchronous evaluations off the JS event loop via libuv worker threads (`AsyncTask`) alongside optional `*Sync` helpers for pure-native evaluations.
- **Deadlock-Free JS SPI Bridge & Full Native Host**: Bridges custom sync/async JS `moduleReaders`, `resourceReaders`, `logger`, `frameTransformer`, and `securityManager` hooks safely via weak `ThreadsafeFunction` callbacks; exposes full native Node I/O (`file:`, `env:`, `prop:`, `modulepath:` directories + JAR/ZIP, rustls HTTP/HTTPS with bundled `PklCARoots.pem`, custom CA certificates, proxy/`noProxy`, custom headers, rewrites, `PklProject` / `PklProject.deps.json` via `Project.load`, `package:` / `projectpackage:` with `package-2` disk cache, and `0x26–0x32` external module/resource reader subprocesses), plus lossless `BigInt`/`Uint8Array`/UTF-16 transport, 64-bit monotonic clocks, `cancel()`, `timeoutMs`, and deterministic `close()`.
- **Dual ESM + CommonJS Package (`packages/pkl-napi/`, `@pkl-rs/napi@0.32.0`)**: Provides `dist/index.mjs` (ESM), `dist/index.cjs` (CommonJS), `dist/index.d.ts` (TypeScript declarations), platform-named native binary `native/pkl_napi.darwin-arm64.node`, and `scripts/build.mjs` producing a clean 6-file `npm pack` tarball verified via clean-directory ESM and CJS installation.

### 4. M6 Mechanical Coverage Manifest & Offline 5-Lane Differential Harness
- **Manifest & Requirement Mapping (`tools/m6_manifest.py`, `compatibility/m6-mappings.json`, `compatibility/m6-wasm-napi.json`)**: Maps all **14/14** M6 requirement groups to **83** verified Rust/JS `implementation_links` spanning the M6 product crates/packages (`crates/pkl-{wasm,napi}`, `packages/pkl-{wasm,napi}`) and the shared/native crates (`crates/{pkl-runtime,pkl-formatter,pkl-host-native,pkl,pkl-cli,pkl-doc}`), enumerating all **86** cases (**80** executable + **6** explicit capability-boundary cases), **19** common JS exports, and **19** capability keys. `--check` verifies manifest/mapping/fixture freshness only (`coverage_gate: null`); `--verify-coverage <dir>` enforces `100%` passing executable coverage and exact `capability-blocked` accounting (`strict: true`, `failure_count: 0`).
- **5-Lane Offline Differential & Packaging Harness (`tools/m6_differential.py`, `tests/fixtures/m6/{cases.json,oracle/M6OracleProbe.java,runners/*.mjs}`)**: Builds and seals all 4 `recorded-build` candidates (`pkl`, `pkldoc`, `wasm_pkg.tgz`, `napi_pkg.tgz`), snapshots all `469` Rust/JS/manifest/fixture source files before and after execution (`source_stability`), verifies `M4HttpFixtureServer.self_test()` (`7/7`), runs **86** cases completely offline across 5 lanes against pinned Eclipse Adoptium Temurin `21.0.13.1+2-LTS` (`exact_jdk_lock_match: true`) and pinned Playwright `1.64.1` Chromium `156.0.8078.4`, records conversion telemetry for both N-API and browser Wasm, and enforces strictly separated `--verify-report`, `--verify-strict`, and `tools/m6_manifest.py --verify-coverage` gates.
- **Tooling Tests**: Added `tools/tests/test_m6_manifest.py` and `tools/tests/test_m6_differential.py` (`9/9` pass, covering manifest integrity, tamper detection, capability-blocked separation, and strict gate rejection).

---

## Environment and Provenance (Local Verification)

- **Provenance Type**: Local macOS `arm64` (`darwin-arm64`) development checkpoint verification (**NOT hosted CI**; Linux, Windows, `darwin-x64`, and non-Chromium browser runs remain separate).
- **Host**: `macOS-27.2-arm64-arm-64bit-Mach-O` (`arm64`)
- **Base Git `HEAD` (before M6 checkpoint commit)**: `7616ed880565e4d80bc457cc24f77fd3061ab9aa` (`7616ed880`)
- **Frozen Diff Scope (before Wave 3C doc/gitignore closeout)**: Only `Cargo.lock`, `crates/pkl-wasm/{Cargo.toml,src/lib.rs}`, and `crates/pkl-napi/{Cargo.toml,src/lib.rs}` were modified relative to `7616ed880`, alongside new M6 files under `crates/pkl-wasm/src/product/`, `crates/pkl-napi/src/`, `packages/pkl-{wasm,napi}/`, `compatibility/m6-*.json`, `tests/fixtures/m6/`, and `tools/{m6_*.py,tests/test_m6_*.py}`. Zero modifications to `crates/{pkl-runtime,pkl-host-native,pkl,pkl-cli,pkl-doc,pkl-formatter,pkl-repl,pkl-server}` or `external/`.
- **Toolchains & Pinned Browser/Wasm Tools**:
  - `rustc 1.98.1 (48a229cea 2026-09-01)`, `cargo 1.98.1 (797e8a9bc 2026-08-05)`
  - `Python 3.14` (`/opt/homebrew/bin/python3.14`)
  - `Node.js v26.13.1`
  - `wasm-bindgen 0.2.108` (`.cache/m1-tools/bin/wasm-bindgen`, `wasm_bindgen_sha256`: `b72ed0a81b26005a7c85ff4f06670988f028c3092fb7072ea8d18969047560c1`)
  - `Playwright 1.64.1` (`playwright == playwrightCore == playwrightPin == 1.64.1`)
  - `Chromium 156.0.8078.4` (`browserExecutableSha256`: `c8509aee32abd6977968e9999769980b7a420dea05402d90042d9e5b41db15ef`)
- **Pkl References**:
  - Apple Pkl `0.32.0` (`external/pkl` @ `e51fb2f91774265666f94980ec630db794a4db2d`)
  - Apple Pkl Pantry (`external/pkl-pantry` @ `904a68184e54da378b0a0e8a911a3fab3956f693`)
  - `references.lock` (`648 B`): `278f48fd7697459281aa78a706c9ca57222f36a8c0785d838a6449fbaf26d30b`
  - `oracle.lock.json` (`4,421 B`): `035fa94a2706344b29d94c8cd3d233d2447007bad0a1a52f8911756e460f0bbc`
  - `pkl-tools-0.32.0.jar` `artifact_sha256`: `c3b6f6215a08b31453b0fb0edd8c9da0aee31233f65abb8ab07f9c945151e059`
- **Pinned JVM Oracle (`exact_jdk_lock_match: true`, NO `PKL_ALLOW_LOCAL_JDK=1`)**: Eclipse Adoptium Temurin `21.0.13.1+2-LTS` (`java.version: 21.0.13.1`, `.cache/oracle/temurin-21.0.13.1+2-LTS/jdk-21.0.13.1+2/Contents/Home`)
  - `jdk.executable_sha256`: `8261edc44ac1643e7aba9f0df5155b6d81f7977cc8a0374bb7630d838d2a3910`
  - `jdk.runtime_modules_sha256`: `c09beb71c3d4ac1fd9dd754e88d2b96c0eca603ce134756e25bd4528df0d6a45`
  - `jdk.release_sha256`: `2f1cd602bf21d35e6e2976d6a351693ff609bfcb285762263a54bf20e8a40afd`
- **Committed Manifest, Fixture, Oracle Probe, and Tool Identities**:
  - `compatibility/m6-mappings.json` (`22,026 B`): `933a7c0a0a3d8945a6dea1c0538da04bae4c5d427f8b38e05d11d039bbd74a4a`
  - `compatibility/m6-wasm-napi.json` (`37,108 B`): `c8774339b4930f9a72241ab46cf9dcc874fccf04fa68c9dc0c2a2dba317732a2`
  - `tests/fixtures/m6/cases.json` (`97,798 B`): `1d403337fca5aeea4c7ff9863d6e9804ecc0baf50b8cc936a31f6b734bca6f04`
  - `tests/fixtures/m6/oracle/M6OracleProbe.java` (`40,573 B`): `9e2525a08382b98fcd5ee5cd8a9a8d2a035db925ae973641220869311ed76a1f`
  - `tools/m6_manifest.py` (`17,259 B`): `882cb8b1e0e1a9ca906328320fc9bb36b5a21e76077a92d02b4a2363dd60d88b`
  - `tools/m6_differential.py` (`81,187 B`): `9178e8874ac17f242d5e9597f590a09eeb65eb4311d42175f92b0c1190d826c8`
- **Recorded Candidate Artifacts (`target/m6/m6-full-strict-03/artifacts/`, `source_binding: "recorded-build"`)**:
  - `pkl` (`14,230,864 B`): `6baa33336e23f6a279a42a4dd0daf69c42768c888e1e6f0bda86bc5b843370ba` (byte-identical to M5)
  - `pkldoc` (`13,016,864 B`): `793fe11152a8806acb2bee111e16eb66c245ce2984de02b7cce240af3261bb72` (byte-identical to M5)
  - `wasm_pkg.tgz` (`@pkl-rs/wasm@0.32.0`, `3,065,214 B`, `13` tarball files): `cdf2cb8441f184a124845aeaf03f67671af5d720a8a3d78bbbb19e706effae50`
  - `napi_pkg.tgz` (`@pkl-rs/napi@0.32.0`, `5,764,592 B`, `6` tarball files): `3c1637920320c94d7c1851ba5d6ce5efd093a28215bc0fbda249ee2a2cfc3842`

---

## Preserved Capture Progression (Wave 1 -> Wave 2 -> Wave 3A -> Wave 3B)

Original intermediate and sandboxed captures are preserved untouched in fresh directories under `target/m6/` rather than overwritten:

1. **Wave 1 Java-Only Oracle Baseline (`target/m6/wave1-oracle-baseline-01..06`)**:
   - `wave1-oracle-baseline-01` (initial sandboxed loopback attempt, no `report.json`), `wave1-oracle-baseline-02` (`report.json` SHA-256 `12d4fa64db628fb88e1ef92f741d0df872cecb1a8a63bdd870a357270d3dd164`: `72 oracle-recorded`, `14 oracle-contract-mismatch`), `wave1-oracle-baseline-03` (`6b5a5c7b3e2340518657d75ae23a12d5960aab0ae4d4bc2d02e65cb4f677d66c`: `83 oracle-recorded`, `3 oracle-contract-mismatch`), `wave1-oracle-baseline-04` (`9d4e8ca42a75c463b098587a4988fe851b2a5b9eb071c0f00d6f8529f0a586fa`: `86/86 oracle-recorded`, `270` sealed files), and `wave1-oracle-baseline-05` (`8d7184f62da1498b838a85bfb5c1bff2bc5a6b73f85a58a0241fdd593e95fabe`: `86/86 oracle-recorded`, `356` sealed files) iteratively validated `M6OracleProbe.java` and fixture contracts.
   - Final Wave 1 baseline `target/m6/wave1-oracle-baseline-06` (`report.json` SHA-256 `d4cc2b40c907bb72f8765527db7629d68a2a98b349924c0f4af8da1c568f6ba4`, `356` sealed files) recorded **`86/86 oracle-recorded`** (`--verify-report` Exit `0`, `--verify-strict` and `--verify-coverage` honestly Exit `1` rejecting Java-only recordings).
2. **Wave 2 Per-Lane Bring-Up & Parity Fixes (`target/m6/wave2-wasm-03..05`, `target/m6/wave2-napi-01..02`)**:
   - **Wasm lanes (`wasm-browser`, `wasm-node`)**: `wave2-wasm-03` (sandboxed loopback denial preserved) and `wave2-wasm-04` (`report.json` SHA-256 `082926098c7a82667e4bf67edd74355780044e3d63df1c9162c7a196f76d5878`: `36 pass`, `6 capability-blocked`, `1 mismatch` on `m6:wasm-browser:analyze-import-graph`) surfaced missing static `pkl:*` stdlib resolution inside `WasmImportAnalyzer` (`crates/pkl-wasm/src/product/host_core.rs`). After wiring security-checked `pkl:*` resolution and static import stubs plus regression test `import_graph_analysis_resolves_builtin_stdlib_modules`, `target/m6/wave2-wasm-05` (`8096134a1f71a1b76a2ad514029fb10229d64e061ad7c31c7f6e8bfb68d23cb7`) reached **`37/37` executable PASS + `6/6` `capability-blocked`** (`wasm-browser` `29/29 + 4`, `wasm-node` `8/8 + 2`).
   - **N-API lane (`napi-node`)**: `wave2-napi-01` (sandboxed loopback denial preserved) and `wave2-napi-02` (`report.json` SHA-256 `fbeb46335ce6d5a273aafbe5f64ea1c77b2da85a45e76e06dee23fcd9027ecc2`: `24 pass`, `2 mismatch` on `m6:napi-node:evaluate-test-spec` and `m6:napi-node:analyze-import-graph-native`) surfaced `pkl:test` report formatting and native import-graph analysis edge cases resolved in `crates/pkl-napi/src/product.rs`.
3. **Wave 3A Manifest Linking & Full 5-Lane Bring-Up (`target/m6/m6-full-strict-01..03`, `target/m6/m6-m1-check-01..02`)**:
   - Linked all **14/14** M6 requirement groups to **83** verified Rust/JS symbols in `compatibility/m6-mappings.json` and regenerated `compatibility/m6-wasm-napi.json` (`tools/m6_manifest.py --check` Exit `0`).
   - `m6-full-strict-01` (sandboxed loopback attempt preserved) and `m6-full-strict-02` (`report.json` SHA-256 `09687610c1f261914ebb6bff2cc39d6e125ca225604fc8c1fcfd21e96e749838`: `79 pass`, `6 capability-blocked`, `1 mismatch` on `m6:pack:downstream-rust-crate-links-pkl`) surfaced offline Cargo lock/path handling in the downstream Rust crate packaging check.
   - Final canonical capture **`target/m6/m6-full-strict-03`** (`report.json` SHA-256 `d38af692c30aa3b4edc01865224aa628a1b925002cb4958ad6d336ae7acdd2fe`, `sources_snapshot_sha256` `f4ea28f02b3425c894ce3cbcf9c501e031ae132d57656ba790f5e1967ecd1896`, `834` sealed files, `source_stability: true` across `469` files) closed all 5 lanes with **`80/80` executable PASS + `6/6` `capability-blocked`** (`failure_count: 0`).
   - M1 regression capture `m6-m1-check-01` (sandboxed Chromium loopback denial preserved) was retried unsandboxed into fresh **`target/m6/m6-m1-check-02`** (`report.json` SHA-256 `a27ddac1a46c5807020d293463df2cc9a3bca10abb2b948591dd0c877619bfea`), confirming **`6,078/6,078`** across `rust`, `node`, and Chromium (`92/92` scenarios, `browserErrors: []`, `source_stability: true`, `473/473` `source_hashes`).
4. **Wave 3B Final Frozen-Source Verification (Strictly Read-Only)**:
   - Preserved honest environment splits transparently:
     - Sandboxed `cargo test --workspace --locked` hit macOS sandbox loopback denial solely on 3 tests in `pkl-host-native::m4_native_host_io` (`5/8`), verified **`8/8` PASS** unsandboxed via `cargo test -p pkl-host-native --locked`.
     - One transient CPU-load `1000ms` init timeout occurred on `m4_external_reader_process::real_subprocess_hang_on_read_unblocks_on_cancel_and_timeout` (`9/10` initial run), verified **`10/10` (`1.27s`)** cleanly on retry alongside the full remaining test tail.

---

## Final Frozen-Source Verification Outcomes

All checks below ran on the final frozen, formatted source tree (`2026-11-21`, local macOS `arm64`, exact pinned Temurin `21.0.13.1+2-LTS`, `exact_jdk_lock_match: true`, NO `PKL_ALLOW_LOCAL_JDK=1`):

| Verification Target | Command / Evidence Path | Exit | Observed Outcome & Artifact SHA-256 |
| --- | --- | :---: | --- |
| **Reference Pins** | `cargo xtask verify` | `0` | `Reference pins verified for Pkl 0.32.0.` |
| **Formatting** | `cargo fmt --all -- --check` | `0` | Clean across workspace |
| **Workspace Clippy** | `cargo clippy --workspace --all-targets --locked -- -D warnings` | `0` | `0` warnings across all workspace crates & targets (incl. `pkl-wasm`, `pkl-napi`) |
| **Regex & Wasm Clippy (2/2)** | `cargo clippy -p java_regex --lib --locked -- -D warnings` + `cargo clippy -p pkl-wasm --target wasm32-unknown-unknown --locked -- -D warnings` | `0` (2/2) | `0` warnings (`unsafe_code = "deny"`) |
| **Wasm Target Checks (3/3)** | `cargo check -p {pkl-runtime,pkl-formatter,pkl-wasm} --target wasm32-unknown-unknown --locked` | `0` (3/3) | Clean pure-Wasm builds (`0` warnings, `unsafe_code = "deny"`) |
| **Oracle Pin Verify** | `/opt/homebrew/bin/python3.14 tools/oracle.py verify` | `0` | `exact_jdk_lock_match: true`, `Pkl 0.32.0`, Temurin `21.0.13.1+2-LTS` |
| **Python Freshness (11/11)** | `tools/{inventory,discover,smoke_manifest,regression_manifest,m2_manifest,m2_module_eval,m2_jvm_classpath,m3_manifest,m4_manifest,m5_manifest,m6_manifest}.py --check` | `0` (11/11) | All 11 manifests current (incl. M6: `14` linked reqs, `83` links, `86` cases [`80` exec + `6` capability-boundary], `19` common exports, `19` capability keys) |
| **Full Rust Workspace Suite (Sandbox Loopback Split + Transient Retry)** | `cargo test --workspace --locked` with honest splits: unsandboxed `cargo test -p pkl-host-native --locked` (`8/8`, vs `5/8` sandboxed loopback denial) and clean retry of `m4_external_reader_process` (`10/10` in `1.27s` after transient `1000ms` CPU-load init timeout at `9/10`) | `0` | **Combined: `2,472` outer passed across `378` targets/doctests** (**`2,485` raw `test result:` passes** including `13` nested child passes; `+9` vs M5 from new `pkl-wasm` unit tests); **`0` failed**; exact **`2` ignored** (`frontend::frontend_probe`, `m2_protobuf_java04_full::java04_strict_full_compatibility_gate`) |
| **Python Unit Tests (53 Modules)** | `test_m6_*.py` (`2` mods, `9/9`) + `test_m5_*.py` (`4` mods, `16/16`) + `test_m4_*.py` (`3` mods, `15/15`) + `test_m3_*.py` (`2` mods, `12/12`) + `test_m2_*.py` (`22` mods, `256/256`) + `18` core shared & runtime-link mods (`158/158`) + `test_pantry.py` & `test_workspace_storage.py` (`2` mods, `26/26`), sandboxed with pinned JDK | `0` (53/53) | **`492 / 492` PASS** (`75` untouched historical suites out of `128` total unexecuted) |
| **Package Unit & Clean-Install Tests (3/3)** | `packages/pkl-wasm/test/wasm.test.mjs` (`9/9`) + `packages/pkl-napi/test/run.mjs` (`11/11`) + `packages/pkl-napi/test/pack-install.mjs` (PASS) | `0` (3/3) | `@pkl-rs/wasm` (`9/9` PASS) and `@pkl-rs/napi` (`11/11` PASS + clean-dir ESM/CJS tarball install PASS) |
| **M1 Multi-Target Regression Sweep** | `/opt/homebrew/bin/python3.14 tools/m1_platforms.py --output target/m6/m6-m1-check-02 --verify-contract` *(unsandboxed retry; sandboxed `m6-m1-check-01` preserved)* | `0` | **`6,078 / 6,078`** across `rust`, `node` (`v26.13.1`), and `browser` (Chromium `156.0.8078.4`, Playwright `1.64.1`); `92/92` Node + `92/92` Chromium scenarios (`browserErrors: []`); `source_stability: true` (`473/473` `source_hashes`). `report.json` SHA-256: `a27ddac1a46c5807020d293463df2cc9a3bca10abb2b948591dd0c877619bfea`. |
| **Final M6 5-Lane Differential, Packaging & Coverage Gate (86 cases)** | `tools/m6_differential.py --verify-report target/m6/m6-full-strict-03` + `--verify-strict` + `tools/m6_manifest.py --verify-coverage target/m6/m6-full-strict-03` | `0` (3/3) | **`80 / 80` executable PASS + `6 / 6` `capability-blocked`** (`86/86` total): `wasm-browser` **`29/29` pass + `4/4` `capability-blocked`**, `wasm-node` **`8/8` pass + `2/2` `capability-blocked`**, `napi-node` **`26/26` pass**, `cross-target-parity` **`12/12` pass** (`native: 12`, `napi_node: 12`, `wasm_node: 12`, `wasm_browser: 12`, `shared_engine_passed: 12`), `packaging-and-install` **`5/5` pass**; **`14/14` linked requirements (`83` links, `failure_count: 0`, `strict: true`)**; `834` sealed evidence files; `server_self_test.passed: true` (`7/7`); `exact_jdk_lock_match: true`; `source_stability: true` (`469` files); `browserErrors: []`; all 4 candidates `recorded-build`. `report.json` SHA-256: `d38af692c30aa3b4edc01865224aa628a1b925002cb4958ad6d336ae7acdd2fe`; `sources_snapshot_sha256`: `f4ea28f02b3425c894ce3cbcf9c501e031ae132d57656ba790f5e1967ecd1896`. |

### Conversion Telemetry Baseline (`target/m6/m6-full-strict-03`, `398 B` Payload)

Recorded strictly as **M6 JS<->Rust conversion baseline telemetry** (NOT an M7 `<= 1.00x` JVM release throughput/memory parity claim):

| Case ID | Iterations | Payload | Min | Median | Mean | P95 | Max | Notes |
| --- | :---: | :---: | ---: | ---: | ---: | ---: | ---: | --- |
| `m6:napi-node:conversion-telemetry` | `25` | `398 B` | `33,000 ns` | `35,583 ns` | `36,742 ns` | `47,625 ns` | `50,792 ns` | Node.js `v26.13.1` (`darwin-arm64`) |
| `m6:wasm-browser:conversion-telemetry` | `20` | `398 B` | `0 ns` | `100,000 ns` | `110,000 ns` | `400,000 ns` | `400,000 ns` | Chromium `156.0.8078.4` (`100 us` `performance.now()` timer granularity) |

---

## Preserved Boundaries, Nuances, and Unverified Scope

1. **Explicit Capability-Blocked Boundary Separation (`6/6` cases, NEVER counted in `compatibility_pass_count`)**:
   - Six Wasm boundary cases verify that capabilities absent in sandboxed Wasm environments fail deterministically with structured `PklError` (`code: "capability-unavailable"`) rather than silently skipping or returning false passes:
     - `wasm-browser` (`4`): `m6:wasm-browser:cap-gap-native-fs`, `m6:wasm-browser:cap-gap-external-readers`, `m6:wasm-browser:cap-gap-native-http`, `m6:wasm-browser:cap-gap-modulepath-jar`.
     - `wasm-node` (`2`): `m6:wasm-node:cap-gap-native-fs`, `m6:wasm-node:cap-gap-external-readers`.
   - `tools/m6_differential.py` and `tools/m6_manifest.py` report `compatibility_pass_count: 80` (`executable_case_count: 80`) and `capability_blocked_count: 6` (`capability_boundary_case_count: 6`) separately and forbid adding capability-blocked cases into `compatibility_pass_count`.
2. **Verified vs. Unverified Platform and Browser Matrix**:
   - **Verified in M6**: Local **macOS `arm64` (`darwin-arm64`)** native Rust (`pkl`, `pkldoc`, `crates/pkl`), Node.js `v26.13.1` (`@pkl-rs/napi` ESM+CJS and `@pkl-rs/wasm`), and pinned Playwright `1.64.1` **Chromium `156.0.8078.4`** (`@pkl-rs/wasm`).
   - **Unverified / Open for M7**: Native/N-API builds and execution on `darwin-x64`, `linux-x64-gnu`, `linux-arm64-gnu`, and `win32-x64-msvc`; non-Chromium browsers (Firefox, WebKit); and hosted CI matrix runs.
3. **Unchanged M3–M5 and Protobuf Crates**:
   - Because M6 made zero modifications to `crates/{pkl-runtime,pkl-host-native,pkl,pkl-cli,pkl-doc,pkl-formatter,pkl-repl,pkl-server}`, `vendor/java_regex`, or `external/`, the frozen M5 verification baselines (`reports/m5/README.md`: M5 5-lane `142/142`, M3 coverage `1,781/1,781` + diff `118/118`, M4 Lanes 1–3 `86/86`, Protobuf strict capture contracted Exit `101` with exact `18` `REFERENCE-NONPASS` lines) and M4's full 5-lane Pantry baseline (`reports/m4/README.md`: `429/429`) remain retained without redundant re-run (and are not relabeled as newly re-executed in M6).
4. **Retained Prior-Milestone Exceptions & Unexecuted Historical Suites**:
   - **M2**: Stage 161's explicit exceptions (`error-cycle` strict-gate Exit `1`, `18` upstream Protobuf `REFERENCE-NONPASS` slots, `11` `required: false` `rev1` unmapped entries, Stage 161 `4,575`-case replay baseline) remain preserved unchanged.
   - **M4 / M5**: Pure-Rust DEFLATE vs OpenJDK zlib on non-empty `package-archive` ZIPs, `CliProjectResolver.kt` 0-byte stderr parity, `native-version` (`3` cases), `pkldoc-progress` (`9` cases), and 2026-11-20 JVM/codegen scope exclusions remain preserved unchanged.
   - **Historical Python Suites**: Exact `75` untouched historical stage/fixture-authority `tools/tests/test_*.py` modules (out of `128` total) were not re-run.
5. **Performance Tracking and Next Priority (M7)**:
   - M6 records JS<->Rust conversion telemetry (`m6:napi-node:conversion-telemetry`, `m6:wasm-browser:conversion-telemetry`) as baseline observability only; full cross-workload execution time and peak-RSS parity against warmed-up JVM (`<= 1.00`), cross-OS/browser release matrix execution, long-running GC/lifecycle soak tests, and final release verification remain tracked for **M7 — Full Release Verification** (**next priority**).
