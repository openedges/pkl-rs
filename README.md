# pkl-rs

The [M6 Wasm and N-API product completion checkpoint](reports/m6/README.md) closes Milestone **M6** on local macOS `arm64` (`darwin-arm64`, `rustc 1.98.1`, pure `wasm32-unknown-unknown` clean on `pkl-runtime`, `pkl-formatter`, and `pkl-wasm`, `0` `unsafe` blocks workspace-wide, zero production Java fallback) against Apple Pkl **0.32.0** using the exact pinned **Eclipse Adoptium Temurin `21.0.13.1+2-LTS`** JDK (**`exact_jdk_lock_match: true`**, without `PKL_ALLOW_LOCAL_JDK=1`; local verification, not hosted CI), `Node.js v26.13.1`, `Playwright 1.64.1`, `Chromium 156.0.8078.4`, and `wasm-bindgen 0.2.108`, with zero modifications to `crates/{pkl-runtime,pkl-host-native,pkl,pkl-cli,pkl-doc,pkl-formatter,pkl-repl,pkl-server}` or `external/`. It delivers the unified JS/TS public API contract (`19` common exports, explicit `19`-key `capabilities()`) across both distributable npm packages while preserving 100% of existing M1 experimental exports: **`@pkl-rs/wasm@0.32.0`** (`crates/pkl-wasm`, `packages/pkl-wasm`, `13`-file `npm pack` tarball working identically in real Chromium ESM and Node.js, featuring `VirtualFileSystem`, cooperative async Host suspension for modules/resources/HTTP `fetch`/pure-Wasm `package://` ZIPs, 64-bit monotonic clocks, lossless `BigInt`/`Uint8Array`/UTF-16 boundaries, `cancel()`/`timeoutMs`/`close()`, and structured `code: "capability-unavailable"` errors for unsupported OS capabilities) and **`@pkl-rs/napi@0.32.0`** (`crates/pkl-napi`, `packages/pkl-napi`, `6`-file dual ESM+CJS `npm pack` tarball featuring non-blocking libuv `AsyncTask` evaluation, deadlock-free weak `ThreadsafeFunction` custom JS SPI callbacks, full `pkl-host-native` Node I/O including `file:`/`env:`/`prop:`/`modulepath:`/rustls HTTP+HTTPS+CA+proxy/`PklProject`/`package-2` cache/`0x26–0x32` external reader subprocesses, lossless `BigInt`/`Uint8Array`/UTF-16 transport, and deterministic lifecycle cleanup). On the final frozen tree, all Wave 3A/3B gates pass: static/Wasm/oracle/freshness prechecks (`cargo xtask verify`, `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo clippy -p java_regex --lib --locked -- -D warnings`, `cargo clippy -p pkl-wasm --target wasm32-unknown-unknown --locked -- -D warnings`, `cargo check -p {pkl-runtime,pkl-formatter,pkl-wasm} --target wasm32-unknown-unknown --locked`, `tools/oracle.py verify`, and all **11** Python `--check` scripts including `tools/m6_manifest.py`), full Rust workspace tests (**`2,472` outer passed across `378` targets/doctests [`2,485` raw `test result:` passes including `13` nested child passes]; `0` failed; exact `2` ignored**, transparently recording the macOS sandbox loopback split on `pkl-host-native` and one transient CPU-load retry on `m4_external_reader_process`), **53** Python unit-test modules (**`492/492` PASS**, including `9/9` across `test_m6_*.py`), standalone package tests (`packages/pkl-wasm` `9/9`, `packages/pkl-napi` `11/11` + `pack-install`), M1 multi-target regression (`target/m6/m6-m1-check-02`: **`6,078/6,078`** across Native, Node N-API, and Chromium Wasm with `92/92` scenarios and `browserErrors: []`), and the **86-case 5-lane offline M6 differential, packaging & coverage gate** (`target/m6/m6-full-strict-03`, all 4 `recorded-build` candidates `pkl`/`pkldoc`/`wasm_pkg.tgz`/`napi_pkg.tgz`, `834` sealed files, `source_stability: true` across `469` files, `browserErrors: []`: **`80/80` executable PASS + `6/6` `capability-blocked`** across `wasm-browser` `29/29 + 4`, `wasm-node` `8/8 + 2`, `napi-node` `26/26`, `cross-target-parity` `12/12` [4-way Java==Native==N-API==Wasm], and `packaging-and-install` `5/5`; `14/14` linked requirements, `83` links, `failure_count: 0`, `--verify-report`, `--verify-strict`, and `--verify-coverage` all Exit `0`). Explicit boundaries and unverified scope remain preserved honestly: `capability-blocked` (`6/6` Wasm cases reported separately, never counted in `compatibility_pass_count`), unverified platforms/browsers (`darwin-x64`, `linux-x64-gnu`, `linux-arm64-gnu`, `win32-x64-msvc`, Firefox/WebKit, hosted CI), unchanged M3–M5/Protobuf crates retaining their frozen M5/M4 baselines below, retained M2/M4/M5 exceptions, `75` unexecuted historical Python suites (out of `128`), M6 conversion telemetry (`m6:napi-node:conversion-telemetry` median `35,583 ns`, `m6:wasm-browser:conversion-telemetry` median `100,000 ns`) kept separate from M7's `<= 1.00` JVM release parity gate, and open Milestone **M7** (next priority: **M7** — full release verification, cross-platform/browser matrix, soak testing, and JVM throughput/memory parity).

The [M5 product APIs and tools completion checkpoint](reports/m5/README.md) closes Milestone **M5** on local macOS arm64 (`rustc 1.98.1`, pure `wasm32-unknown-unknown` clean on `pkl-runtime` and `pkl-formatter`, `0` `unsafe` blocks workspace-wide, zero production Java fallback) against Apple Pkl **0.32.0** using the exact pinned **Eclipse Adoptium Temurin `21.0.13.1+2-LTS`** JDK (**`exact_jdk_lock_match: true`**, without `PKL_ALLOW_LOCAL_JDK=1`; local verification, not hosted CI). It completes the native product surface across eight workspace crates (`crates/pkl`, `crates/pkl-cli`, `crates/pkl-formatter`, `crates/pkl-repl`, `crates/pkl-server`, `crates/pkl-doc`, `crates/pkl-runtime`, and `crates/pkl-host-native`): the public Rust Evaluator API (`EvaluatorBuilder`, `Evaluator`, `ModuleSource`, `PklValue`, `ModuleReader`/`ResourceReader`, `Logger`, `StackFrameTransformer`, `evaluate_schema`, deterministic `close()`), the unified product `pkl` CLI (`eval`, `repl`, `server`, `test`, `project resolve`/`package`, `download-package`, `analyze imports`, `format`, `run` [`pkl:Command`], `shell-completion`, `--color` ANSI diagnostics and power assertions, `--help`, honest native-flavor `--version`), interactive PTY & pipe REPL (`pkl-repl` backed by Wasm-pure `pkl-runtime::repl`), MessagePack evaluator server (`pkl-server` backed by Wasm-pure `pkl-runtime::server`, `0x20–0x25` + `0x26–0x32` client/external reader callbacks), pure-Wasm CST Pkl formatter (`pkl-formatter`, `--grammar-version 1|2|auto`, strict two-pass idempotency), and `pkldoc` documentation site generator & v1->v2 `--migrate` migrator (`pkl-doc`, bundling `17/17` byte-identical vendored static web assets and live `stdlib/doc-package-info.pkl` support closing the 8-item M3 deferral), backed by shared Wasm-pure runtime extensions (`schema`, `repl`, `server`, `evaluation/command`, `diagnostic/ansi`) and `NativeHostExtensions` in `crates/pkl-host-native`. On the final frozen tree, all Wave 3B gates pass: static/Wasm/freshness prechecks (`cargo xtask verify`, `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo clippy -p java_regex --lib --locked -- -D warnings`, `cargo check -p pkl-runtime --target wasm32-unknown-unknown --locked`, `cargo check -p pkl-formatter --target wasm32-unknown-unknown --locked`, `tools/oracle.py verify`, and all **10** Python `--check` scripts including `tools/m5_manifest.py`), full Rust workspace tests (**`2,463` outer passed across `377` targets/doctests [`2,476` raw `test result:` passes including `13` nested child passes]; `0` failed; exact `2` ignored**, with the macOS sandbox loopback split documented transparently), **51** Python unit-test modules (**`483/483` pass**, including `16/16` across `test_m5_*.py`), multi-target M1 (`target/m5/m5-closeout-platforms-03`: **`6,078/6,078`** across `rust`, `node` `v26.13.1`, and Chromium `156.0.8078.4` with `92/92` scenarios), M3 regressions (`target/m5/m5-closeout-m3-census.json` `1,781/1,781` `failure_count: 0` + `target/m5/m5-closeout-m3-diff` `118/118 pass`), M4 Lanes 1–3 regressions (`target/m5/m5-closeout-m4-lanes123-02`: **`86/86 pass`** across `host-io-http-security` `48/48`, `external-readers` `11/11`, and `projects-packages-cache` `27/27`), Protobuf strict capture (`target/m2/m5-closeout-protobuf-strict` contracted Exit `101`, `240/240` regular, `8/8` pre-renderer, exact `18` `REFERENCE-NONPASS` lines with `0` byte-diff lines on projected channels), and the **142-case 5-lane offline M5 differential & coverage gate** (`target/m5/m5-final-strict-02`, all 3 `recorded-build` candidates `pkl`/`pkldoc`/`pkl_m5_api_probe`, `4,453` sealed files, `source_stability: true` across `429` files: **`142/142 pass`** across `rust-api-and-cli` `50/50` [`13/13` `m5:api:*` + `37/37` `m5:cli:*`], `repl` `12/12`, `server` `22/22`, `formatter` `43/43` [`30` snippets + 2nd-pass idempotency + `13` CLI], and `pkldoc` `15/15` [`1,767/1,767` regular files totaling `15,227,047 B` and `13/13` symlinks with `0` file/symlink/stderr diffs]; `16/16` requirements, `75` links, `17/17` exact vendored assets, `failure_count: 0`, `--verify-report`, `--verify-strict`, and `--verify-coverage` Exit `0`). Explicit boundaries, exclusions, and unverified scope remain preserved honestly: `comparison_mode: "native-version"` (`3` cases honestly emitting `Pkl 0.32.0 (<os>, native)` / `pkldoc 0.32.0 (<os>, native)` without spoofing `Java <ver>`), `comparison_mode: "pkldoc-progress"` (`9` generation/migration cases honestly recording `raw_stdout_bytes_match` `0/9` / `progress_order_differs: true` due to upstream Java `newVirtualThreadPerTaskExecutor` + `Files.walk` stdout nondeterminism while strictly enforcing 100% raw byte parity on all `1,767` generated files and `13` symlinks, exact raw stderr, exact exit code, ordered milestone lines, and exact URI-encoded progress-segment multisets; remaining `130` cases are strict `raw`), 2026-11-20 JVM/codegen exclusions preserved as census inventory in `compatibility/m5-product-tools.json` (`pkl-codegen-java`/`pkl-codegen-kotlin` [`2` commands, `19` options], `java_api` [`8,239`], `kotlin_api` [`597`], `jvm_binding_spec_headings` [`73`], `excluded_corpus_jvm_test_files` [`58`], JVM ServiceLoader SPI [`2` resources]), retained M2/M4 exceptions and baselines (`error-cycle`, `18` Protobuf `REFERENCE-NONPASS`, `11` `required: false` `rev1` unmapped, Stage 161 4,575-case replay; M4 pure-Rust DEFLATE vs OpenJDK zlib on non-empty `package-archive` ZIPs, `CliProjectResolver.kt` 0-byte stderr parity, and M4's full 5-lane `429/429` Pantry baseline), the unexecuted M4 Lanes 4–5 Pantry slice (`343` cases not re-run in this closeout slice), `75` unexecuted historical Python suites (out of `126` total), Linux/Windows native execution and hosted CI, M7 performance benchmarking against the JVM `<= 1.00` target, and open Milestones **M6–M7** (next priority: **M6** — Wasm and N-API product completion).

The [M4 modules, resources, projects, and packages completion checkpoint](reports/m4/README.md) closes Milestone **M4** on local macOS arm64 (`rustc 1.98.1`, pure `wasm32-unknown-unknown` clean on `pkl-runtime`, `0` `unsafe` blocks workspace-wide) against Apple Pkl **0.32.0** using the exact pinned **Eclipse Adoptium Temurin `21.0.13.1+2-LTS`** JDK (**`exact_jdk_lock_match: true`**, without `PKL_ALLOW_LOCAL_JDK=1`; local verification, not hosted CI). It introduces the dedicated native Host and unified CLI crate `crates/pkl-host-native` (`pkl_m4_cli`, bundled byte-identical `PklCARoots.pem`, pure-Rust HTTP/1.1 + rustls HTTPS/CA + HTTP/`CONNECT` proxy + `TimeoutWatchdog`, zero `/usr/bin/curl`) backed by Wasm-pure `crates/pkl-runtime/src/{host,external_reader,packages}/` primitives covering allowlists & `rootDir` confinement, HTTP custom headers/redirects/downgrade denial/rewrites/proxy/`noProxy`, `modulepath:` (dirs + JAR/ZIP), external module & resource readers (`0x26–0x32` MessagePack + OS process lifecycle), `PklProject`/settings precedence/MVS resolution/import cycles/`ProjectPackager` (`java_hashmap_order`, fixed DOS `1980-02-01` mtime, exact empty-ZIP SHA-256)/`package-2` cache & corruption recovery/`pkl:test`, plus core VM fixes surfaced by real Pantry packages. On the final frozen tree, all 7 Wave 3B gates pass: static/freshness prechecks (`cargo xtask verify`, `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo clippy -p java_regex --lib --locked -- -D warnings`, `cargo check -p pkl-runtime --target wasm32-unknown-unknown --locked`, `tools/oracle.py verify`, and all **9** Python `--check` scripts including `tools/m4_manifest.py`), full Rust workspace tests (**`2,377` outer passed across `354` targets/doctests [`2,390` raw `test result:` passes including `13` nested child passes]; `0` failed; exact `2` ignored**, with the macOS sandbox loopback split documented transparently), **45** Python unit-test modules (**`449/449` pass**), multi-target M1 (`target/m1/m4-closeout-platforms-02`: **`6,078/6,078`** across `rust`, `node`, and Chromium with `92/92` scenarios), M3 regressions (`target/m3/m4-closeout-census.json` `1,781/1,781` `failure_count: 0` + `target/m3/m4-closeout-diff` `118/118 pass`), Protobuf strict capture (`target/m2/m4-closeout-protobuf-strict` contracted Exit `101`, `240/240` regular, `8/8` pre-renderer, exact `18` `REFERENCE-NONPASS` lines with `0` byte-diff lines), and the **429-case 5-lane offline M4 differential & coverage gate** (`target/m4/wave2-all5-strict-03`, recorded-build `pkl_m4_cli`: **`429/429 pass`** across `host-io-http-security` `48/48`, `external-readers` `11/11`, `projects-packages-cache` `27/27`, `pantry-dev` `126/126`, and `pantry-packaged` `217/217` — executing all 29 Pantry packages in both development-tree and distributed form; `16/16` requirements `failure_count: 0`, `--verify-strict` and `--verify-coverage` Exit `0`). Explicit boundaries, deferrals, and unverified scope remain preserved honestly: pure-Rust Wasm DEFLATE vs OpenJDK C zlib on non-empty `package-archive` ZIPs (`raw_zip_bytes_match` recorded honestly alongside live pinned Java 0.32.0 cross-interop verification), `CliProjectResolver.kt` CLI 0-byte stderr parity (library `warnings` retained), M5 deferrals (`stdlib/doc-package-info.pkl` [8], `pkl-doc` resources [17], `pkl-server` `0x20–0x25` [6 tags / 82 fields], full CLI `repl`/`server`/`format`/`pkldoc`/`run`/`shell-completion` and Rust evaluator API; Java/Kotlin codegen & JVM adapter excluded per 2026-11-20 decision), retained M2 exceptions (`error-cycle`, `18` Protobuf `REFERENCE-NONPASS`, `11` `required: false` `rev1` unmapped, Stage 161 4,575-case replay baseline), `77` unexecuted historical Python suites, Linux/Windows native host & hosted CI, M7 performance tracking (`import "pkl:reflect"` ~5.2s release, `pkl.tar` ~97s under `--timeout 180`), and open Milestones **M5–M7** (next priority: **M5** — product APIs and tools).

The [M3 standard-library and output completion checkpoint](reports/m3/README.md) closes Milestone **M3** on local macOS arm64 (`rustc 1.98.1`, pure `wasm32-unknown-unknown` clean, `0` `unsafe` blocks) against Apple Pkl **0.32.0** using the exact pinned **Eclipse Adoptium Temurin `21.0.13.1+2-LTS`** JDK (**`exact_jdk_lock_match: true`**, without `PKL_ALLOW_LOCAL_JDK=1`; local verification, not hosted CI). It completes byte-for-byte vendoring and live reflection across all `23/23` `pkl:*` stdlib modules (`Command`, `DocPackageInfo`, `DocsiteInfo`, `settings` added; `stdlib/doc-package-info.pkl` [8 items] explicitly deferred to M5 `pkldoc`; lowercase `"pkl:project"` singleton aligned to canonical `"pkl:Project"` reflection URI), all `542/542` external builtins, all `9` `output.renderer` formats (`json`, `jsonnet`, `pcf` with `PcfRenderDirective`, `plist`, `properties`, `textproto`, `xml`, `yaml`, `pkl-binary`), Wasm-pure `output.files` multi-file output and fast-path `output.bytes`, iterative 19-tag `PklBinaryDecoder`, `Reference` 69 `base.pkl` class properties and `converters` `[*]` wildcards, cooperative `Benchmark` execution, JDK 21 Unicode 15.0.0 `Regex` `(?c)` / `CANON_EQ`, and cooperative YAML parser yielding. On the final frozen tree, all static/freshness checks (`cargo xtask verify`, `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo clippy -p java_regex --lib --locked -- -D warnings`, `cargo check -p pkl-runtime --target wasm32-unknown-unknown --locked`, `tools/oracle.py verify`, and all 8 Python `--check` scripts including `tools/m3_manifest.py`), single-command `cargo test --workspace --locked` (**`2,298` outer passed across `345` targets/doctests [`2,311` raw `test result:` passes including `13` nested child passes]; `0` failed; `2` ignored**), post-remediation `test_m3_*.py` (`12/12`) & `test_m2_mappings.py` (`8/8`) (with Wave 3B `434/434` executed Python unit tests on unchanged Python tooling), M1 multi-target verification (`target/m1/m3-closeout-platforms-04`: **`6,078/6,078`** across `rust`, `node`, and Chromium with `92/92` scenarios), the 1,781-item mechanical M3 coverage gate (`target/m3/m3-closeout-census-02.json`: **`failure_count: 0`**), and the 118-case 3-lane M3 raw-byte/generated-file differential gate (`target/m3/m3-closeout-diff-02`: **`118/118 pass`**, `--verify-report` and `--verify-strict` Exit `0`) all pass cleanly. Explicit exceptions and unverified scope remain preserved honestly: `stdlib/doc-package-info.pkl` (8 items, deferred to M5), `18` upstream Protobuf `REFERENCE-NONPASS` slots (`target/m2/m3-closeout-protobuf-strict-02` contracted Exit `101`, `strict-failures.txt` exact `18` lines with `0` byte-diff lines, `experiment_contract_pass: true`, `strict_compatible: false`), Wave 3B pre-remediation focused 4-adapter M2 snapshot (`target/m2/m3-closeout-selected-02`: `100/100 pass`), Stage 161's `error-cycle` strict-gate exception and full `4,575`-case replay baseline, `77` unexecuted historical Python golden-fixture suites, Windows host symlink verification, Linux/Windows hosted CI, and Milestones **M4–M7** (next priority: **M4** — modules, resources, projects, and packages).

The [Stage 161 M2 development-closure checkpoint](reports/m2/stage161-m2-closure/README.md) aligns lazy-property cycle diagnostics in `crates/pkl-runtime/src/vm.rs` to Apple Pkl 0.32.0's `"A stack overflow occurred."` (preserving `ErrorKind::Cycle`, `LazyState::Unevaluated` rollback, and strict-gate honesty), fixes isolated `lib.pkl` staging for `t43_043_inherited_method_main.pkl` and `t43_064_imported_function_main.pkl` (`CLI_PROFILE` `pkl-0.32-historical-cli-no-cache-no-project-v2` in `tools/m2_historical.py`, leaving `tests/fixtures/` untouched), synchronizes `compatibility/m2-language.json`, and runs complete frozen-source verification on local macOS arm64 (`rustc 1.98.1`) against the exact pinned **Eclipse Adoptium Temurin `21.0.12.1+1-LTS`** JDK (**`exact_jdk_lock_match: true`**, without `PKL_ALLOW_LOCAL_JDK=1`). All prechecks (`cargo xtask verify`, `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo check -p pkl-runtime --target wasm32-unknown-unknown --locked`, `tools/oracle.py verify`, and 7 Python `--check` scripts), `422/422` executed Python unit tests (`test_m2_*.py` `256` + core shared tooling `166`; untouched historical golden-fixture suites not run), a single unsplit `cargo test --workspace --locked` (**`2,222 passed; 0 failed; 2 ignored`** across `337` outer targets/doctests, `13` nested child passes excluded; `m2_reflection_apis_lifecycle` `5/5` in `45.63s`), M1 multi-target platforms (`target/m1/stage161-platforms`: **`6,078 / 6,078`** across `rust`, `node` `v26.13.1`, and Chromium `156.0.8078.4` with `92` scenarios each), and the fresh full `4,575`-case M2 replay (`target/m2/stage161-full`: **`4,574 pass / 1 oracle-error (error-cycle) / 0 mismatch`**, `88/88` metamorphic, `21/21` `historical-cli-module-eval` with isolated `t43_043`/`t43_064` byte-exact parity, `68/68` `package-project-eval` with `188` live HTTP requests, `--verify-report` and `--verify-development` Exit `0`) pass cleanly. Explicit strict-gate exceptions remain preserved: `--verify-full` honestly exits `1` on `error-cycle` (cold-JVM `NoClassDefFoundError` and nondeterministic preinitialized repetition counts in `reports/m2/cycle-complete.json`), `target/m2/stage161-protobuf-strict` exits `101` as contracted (`240` regular, `8` pre-renderer, `18` `REFERENCE-NONPASS`, `java04_strict_full_compatibility_gate` `#[ignore]`, carried to M3), `11` `required: false` `rev1` unmapped entries (8 support libs + 3 deduplicated aliases) stay recorded, and Linux/Windows hosted CI and Milestones **M3–M7** remain open.

The [Stage 160 checkpoint](reports/m2/stage160-gc-protobuf-remediation/README.md) optimizes the GC safepoint hot path (`Cargo.toml` dev profile `opt-level = 2`, per-slot edge caching and scratch buffer reuse in `Heap`, zero-allocation reflection root traversal, and dirty-guarded VM declaration-root caching) and fixes two runtime boundary bugs across the four Protobuf `no-complete-reference-nonpass` slots (`imported-alias-owner` and `constraint-read-true` under `on`/`off`, returning `unsupported-or-api-error\n` rather than false success or null-root `oracle-error`), adding two regression tests in `m2_protobuf_renderer` (`16/16`) while leaving `tests/fixtures/m2-protobuf-java04-full/observations.tsv` and `#[ignore]` strict gates intact. On the frozen source (local macOS arm64, `rustc 1.98.1`, `PKL_ALLOW_LOCAL_JDK=1`, Temurin `21.0.13+9-LTS`), formatting, `cargo xtask verify`, `cargo clippy --workspace --all-targets --locked -- -D warnings` (plus `--no-deps`), `cargo check -p pkl-runtime --target wasm32-unknown-unknown --locked`, six Python `--check` scripts, and `253` M2 Python unit tests (`215.895s`) all exit `0`; a **single unsplit `cargo test --workspace --locked`** completes cleanly with **`Exit 0`** (**`2,220 passed; 0 failed; 2 ignored`** across `337` outer targets/doctests, `13` nested child passes excluded / `2,233` raw `test result:` passes), including **`m2_reflection_apis_lifecycle` 5/5 in `83.35s`** (`90.63s` solo vs >60m debug / >29m `--release` timeout in Stage 159) and `pkl-runtime --lib` `263/263` in `26.61s` (`20.71s` solo vs `1359.67s`). Explicit Protobuf strict capture (`target/m2/stage160-protobuf-strict`) exits `101` as contracted (`240` regular, `8` pre-renderer, exact `18` `REFERENCE-NONPASS` entries, `0` harness failures); unsandboxed M1 (`target/m1/stage160-platforms-01 --verify-contract`, preserving initial sandboxed attempt `target/m1/stage160-platforms`) matches `6,078 / 6,078` across `rust`, `node` (`v26.13.1`), and `browser` (Chromium `156.0.8078.4`, Playwright `1.64.1`); and focused M2 4-adapter capture (`target/m2/stage160-selected-01`, preserving sandboxed `target/m2/stage160-selected`) records `100 / 100 pass` with `--verify-report` and `--verify-subset` exiting `0`. Stage 160 does not re-run the full `4,575`-case replay (`target/m2/stage159-genuine` at Stage 159 commit `2c25dd656` remains the latest full replay: `4,574 pass / 1 oracle-error (error-cycle) / 0 mismatch`); `18` Protobuf reference nonpasses, `error-cycle` `oracle-error`, `11` unmapped candidates, local JDK provenance (`exact_jdk_lock_match: false`), Linux/Windows CI coverage, and Milestones M3–M7 remain open.

The [Stage 159 checkpoint](reports/m2/stage159-strict-gate-closure/README.md) completes Iteration 4 integrity remediation and genuine live verification on the frozen worktree (local macOS arm64, `PKL_ALLOW_LOCAL_JDK=1`, Temurin `21.0.13+9-LTS`). It retracts invalid `stage159-final` claims and `id()` digest memoization, replaces hardcoded exception/banner literals with generalized `ForeignOrigin` projection and dynamic `PlatformMetadata` (`PKL_ORACLE_JAVA_VERSION`), restores the pinned Protobuf fixture counts (`240` regular, `8` pre-renderer, `18` reference nonpasses across `266` observations) and `#[ignore]` strict gate, re-audits all `475` requirements in `compatibility/m2-mappings.json`, and restores strict `--verify-full` exit semantics. Genuine full M2 acquisition (`target/m2/stage159-genuine`) records **`4,574 pass / 1 oracle-error (error-cycle) / 0 mismatch`** across all `4,575` ready cases (including `68/68` live `package-project-eval` HTTP fixture cases; `--verify-report` and `--verify-development` exit `0`, `--verify-full` exits `1`). Multi-target M1 (`target/m1/stage159-genuine`) matches `6,078 / 6,078` across `rust`, `node`, and `browser`; `cargo xtask verify`, `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo check -p pkl-runtime --target wasm32-unknown-unknown --locked`, the 5 sentinel oracle suites (`94 passed / 0 failed / 1 ignored`; 2 explicit `#[ignore]` functions workspace-wide: `frontend_probe` and `java04_strict_full_compatibility_gate`), and `253` M2 Python unit tests pass cleanly. Split workspace Rust execution completes `2,226 passed / 0 failed / 2 ignored` across `346` completed test runs, with `m2_reflection_apis_lifecycle::cancellation_closes_evaluator_and_fresh_evaluator_rejects_old_transform_token` timing out under 300,000-step dual-GC sweep overhead (`60m` debug / `29m` `--release`). Remaining limits include that GC stress timeout, `18` Protobuf reference nonpasses, `error-cycle` `oracle-error`, local JDK provenance (`exact_jdk_lock_match: false`), and open Milestones M3–M7 (M3: standard library and output; M4: modules, resources, projects, and packages; M5: product APIs and tools; M6: Wasm and N-API product completion; M7: full release verification and JVM throughput/memory parity).

The checkpoint descriptions below retain their historical source and execution
scope; they are not current-source reruns.

The [Stage 158 Remediation checkpoint](reports/m2/stage158-m2-remediation/README.md) marks
the completion of Milestone M2 remediation and serial verification, resolving all integrity audit
findings. It consolidates package and project loading without mutable statics or host I/O, restores honest
oracle test attributes (`#[ignore]` on 8 protocol-unresolved tests), guarantees pure WebAssembly target
compilation, integrates standard runtime mirrors (`pkl:platform`, `pkl:release`, `pkl:benchmark`), synchronizes
requirement mappings (4,575 ready cases, 11 screened unmapped candidates, 475 requirements), and completes
full serial verification. All required serial Rust checks, 1,205 Python unit tests, and multi-target platform checks
pass cleanly. The strict language gate preserves unmapped/incomplete items without false passes, setup-dependent
oracle errors remain preserved, and 18 Protobuf reference nonpasses remain unmet. Full language/API, release-platform,
and performance gates (M3–M7) remain open.

The [Stage157 checkpoint](reports/m2/stage157-package-metadata/README.md) adds
shared package metadata acceptance and qualified fixtures, with local Rust and
bounded native/Node/browser verification. The [Stage156 baseline](reports/m2/stage156-baseline/README.md)
retains the full-ready and strict completion results.

Stage153–155 adds exact original T40 adapters, native file-URI byte/text handling and bounded shared package URI validation. Two original CLI contracts and two separately counted Java cache-off controls match; file URI records 19 boundary matches/5 nonpasses and package API 6 raw value matches/31 nonpasses. Source06’s full workspace failed (2,190 passed, 1 failed, 6 ignored); source07 separately passes the corrected 15-test target, 83 Python tests and five prechecks. Fresh bounded M1 matches 6,078 results per Native/Node/Chromium target and 92 scenarios per Node/browser. Full M2/API, package/Host, release-platform and performance gates remain open. See the [Stage153–155 report](reports/m2/stage153-155-t40-uri/README.md) for exact source epochs, retained failures and scope.

Stage146–151 adds lossless scalar diagnostics, native file Glob support, selected authored settings/path behavior and bounded static import analysis. The workspace passes 2,172 tests with zero failures and six ignored; 13 nested child passes are excluded. Fresh original31 comparisons record 20 matches, six mismatches, three unsupported cases and two oracle errors. The separate NFC boundary records 13 UTF-8/UTF-16 path-value matches and five nonpasses. Six prechecks passed. One bounded M1 capture matched 6,078 results per Native/Node/Chromium target and 92 scenarios each for Node/browser; strict and architecture predicates pass from the same raw capture. Full M2/API, release-platform and performance requirements remain open. See the [Stage146–151 report](reports/m2/stage146-151-host-analyze/README.md) for retained failures and scope.

Stage145 adds shared Protobuf rendering (`textproto`) and GC root indexing. The current workspace passes 2,104 tests with zero failures and six ignored; the aggregate excludes 13 nested child passes. The 266-slot Protobuf development contract matches 240 renderer and eight pre-renderer observations, while 18 reference nonpasses remain unmet. All 57 selected original CLI contracts match Java. Bounded M1 matches 6,078 results per Native, Node and Chromium target, with 92 Node and 92 browser scenarios. Full M2/API, release-platform and performance requirements remain open. See the [Stage145 report](reports/m2/stage145-protobuf-gc/README.md) for raw evidence, retained failures and scope limits.

Stage144 adds shared Pkl binary rendering, bounded YAML parsing and authored shell escaping. The pre-lint focused run passed 38 native tests against 320 retained Java observations; 21 original CLI contracts separately matched their full public channels. After seven lint corrections, all five prechecks passed and the workspace aggregate records 1,947 passed, zero failed and five ignored across 323 groups, excluding 13 nested child passes. Fresh bounded M1 matched 6,078 results per Native, Node and Chromium target, with 92 Node and 92 browser scenarios; strict raw comparison and the architecture predicate pass from the same capture. Full inventory replay, M2/API, release-platform and performance requirements remain open. See the [Stage144 report](reports/m2/stage144-binary-yaml-shell/README.md) for failures, evidence scopes and aggregate limitations.

The Stage141 Jsonnet and XML checkpoint adds shared renderer support and qualifies 21 focused native tests against 172 Java observations, including four retained setup-error controls. Twenty-five original CLI cases matched their complete public channels. The full workspace aggregate records 1,909 passed, zero failed and five ignored across 313 harnesses; 13 nested child passes are excluded. Bounded M1 matched 6,078 results per Native, Node and Chromium target, with 92 Node and 92 browser scenarios. Full M2/API, release-platform and performance requirements remain open. See the [Stage141 report](reports/m2/stage141-jsonnet-xml/README.md) for the evidence and limits.

The Stage139 JSON Parser checkpoint matches 108 Java observations and five registered original CLI cases. Its focused native run passed 47 tests; the full workspace passed 1,886 tests with five ignored across 305 harnesses, excluding 13 nested child passes from the independent aggregate. A bounded M1 run matched 6,078 results per Native, Node and Chromium target. Full M2/API, release-platform and performance requirements remain open. See the [Stage139 report](reports/m2/stage139-json-parser/README.md) for the evidence and limits.


Stage137 extends module diagnostics and authored semver. The shared runtime adds standard module-key profiles and loads the authored `pkl:semver` module; `ModuleKeyKind` has three new public variants, so downstream exhaustive matches must be updated. Focused checks match 48 Java observations and six original CLI cases, and a bounded M1 run records 6,078 matches per target. The full workspace passed 1,872 tests with five ignored across 302 harnesses; the independent aggregate readback excludes 13 nested child passes. Full M2/API, release-platform and performance requirements remain open. See the [Stage137 report](reports/m2/stage137-module-diagnostics-semver/README.md) for raw evidence and remaining limits.


The [Stage136 report](reports/m2/stage136-language-replay/README.md) records a fresh local full-ready replay of 4,483 cases (candidate-unsupported: 74, mismatch: 12, oracle-error: 2, pass: 4,395). The integrity verifier exited 0; strict full verification exited 1. Twelve lexical requirements were promoted after 49 scalar comparisons and a separate RECORD module comparison. Complete M2/API, release-platform and performance requirements remain open.


This project reimplements [Pkl](https://pkl-lang.org/) 0.32.0 in Rust. It aims to share one engine across native Rust, WebAssembly, and Node.js N-API, with automated compatibility checks against the Java implementation.

The earlier [Stage134 report](reports/m2/stage134-type-source-scopes/README.md) records source-aware type diagnostics,
object-entry naming, and repaired inferred-parent and diagnostic equality regressions.
The full local workspace passes with **1,848 passed, zero failed and 5
ignored across 298 harnesses**. Full M2/API, release-platform and performance
requirements remain open.

The preceding [Stage131 report](reports/m2/stage131-power-boundaries/README.md)
records corrected power-assertion visibility: 34 Java observations and the
original `typeConstraints11.pkl` CLI diagnostic now match. The complete local
workspace exits zero with **1,790 passed, zero failed and 5 ignored across
288 harnesses**. Native, Node and Chromium each match 6,078 Java results;
Node and Chromium each pass 92 platform scenarios. Full M2/API, release-platform
and performance requirements remain open.

The preceding [Stage130 report](reports/m2/stage130-java-exception-renderer/README.md)
adds a shared Java exception renderer checked against 54 graphs in LF and CRLF.
The full local workspace records **1,789 passed, zero failed and five ignored
across 287 harnesses**. Native, Node and Chromium each match 6,078 Java results;
Node and Chromium each pass 92 platform scenarios. Runtime stack projection and
internal-error catch integration remain unfinished, as do full M2/API and
performance requirements.

The preceding [Stage129 report](reports/m2/stage129-workspace-validation/README.md)
records a complete local workspace command with exit **0**: **1,786 passed,
zero failed and five ignored across 286 harnesses**. All test targets and both
doctest crates complete; 26 examples are compile-only. Java URI-catch evidence
adds 32 query observations, including different repeated uncaught-error bytes.
The engine is unchanged, and M2/API, artifact-provenance and performance gaps
remain open.

The preceding [Stage128 report](reports/m2/stage128-workspace-storage/README.md)
adds an opt-in macOS executable-compression wrapper. Its 18 focused tests and
compiler/self-spawn checks pass. The workspace retry completes 200 harnesses
with **1,265 passes, zero failures and three ignored**, then stops at its disk
floor during reflection. Another 83 executable targets and both doctest crates
remain unstarted; this is not a full-workspace pass.

The preceding [Stage127 report](reports/m2/stage127-opaque-glob-uri/README.md)
records opaque glob URI validation before reader enumeration. All 20 selected
wrong-success sessions now stop with explicit unsupported diagnostics; Java's
internal-error bytes remain unmatched. Eight Rust tests and the bounded M1
native/Node/browser acquisition pass. The full workspace command stopped during
compilation at its disk-space guard; that acquisition remains incomplete.

The preceding [Stage122 report](reports/m2/stage122-lossless-source-runtime/README.md)
records lossless UTF-16 source/URI handling, typed glob Host operations, selected
diagnostics, Resource digests and math bindings. Local verification joins 285
complete test harnesses with **1,779 passes, zero failures and five ignored**;
the original reflection Cargo exit remains unobserved, so this is not a fresh
full-workspace command pass. Seven final development commands pass. The complete
ready replay records **4,351 matches out of 4,442 cases**, with 15 mismatches,
74 unsupported cases and two Java oracle errors. All 88 metamorphic checks pass
and no previous passing case regresses. Selected portable comparisons and the three-target raw transport check are
verified, and their evidence archives are published and checked. Full M2/API, release-platform and performance
requirements remain open.

The project has completed its **M0 compatibility tooling and initial Java baselines** and **M1 architecture experiments**, including failed-lazy-retry, nested Host evaluation, cancellation, and the Graal compilation investigation. A bounded parser/resolver, lazy bytecode VM, tracing heap, numeric/regex probes, and resumable Host run through native Rust, Node N-API, and browser Wasm. These are prototypes; the complete language, CLI, REPL, server, and product APIs remain unimplemented. The [M0 report](reports/m0/README.md) and [M1 experiments](reports/m1/README.md) distinguish verified results, known differences, and performance limitations.

**M2 language implementation is in progress.** The shared engine is being extended
with operators, control flow, strings, typed methods, recursive calls, closures,
classes, inheritance, generic type aliases, resumable type constraints, pipelines,
ordered collection members, `for`/`when` generators, spread, member predicates,
generic collection checks and lazy wrappers, structural equality and object keys,
typed default properties and cache-sensitive comparison order, deferred
References through `pkl:ref`, evaluator-owned class and type-alias identity,
function type metadata and arity checks, ordered type resolution and generic
parameter scopes, selected recursive constraints and erased `Class<T>` arguments,
shared constraint receiver slots and creation-time lambda captures,
finite recursive generics with shared captured type arguments, nullable defaults,
selected direct generic constructors and their diagnostic origins, declaration
annotations and ordered initialization, resumable object forcing and string conversion,
Host callback diagnostic frames, strict Map
and Set construction, lookup, typing and structural comparison, persistent type-use
resolution, let/for const scopes, and structured source diagnostics with recursive frame compression with optional power assertions
and standard-library frames. The [latest checkpoint](reports/m2/README.md)
records the accepted local verification scope, known failures, and unexecuted
environments.

The [Stage109 checkpoint](reports/m2/stage109-typed-module-annotation-checkpoint.json) adds `Typed.getProperty`, `Module.relativePathTo` and annotation/const diagnostic coverage to the shared engine. Targeted validation records **152 passed tests and one ignored required placeholder**. Independent current-source native readbacks match **276 ordinary new observations** and **1,138 existing Typed/reflection observations**. The new portable matrix matches **276 ordinary observations per Native, N-API and Chromium target**, retaining two required Java internal observations unresolved on each target.

For that historical Stage109 checkpoint, the complete ready replay records **4,332 passes out of 4,442 cases**, with 26 mismatches, 82 unsupported cases, two oracle errors, no historical-pass regression and all 88 metamorphic checks passing. One bounded M1 acquisition separately matches 6,078 checks per target. Eight freshness commands pass; exact current strict-gate accounting still reports 1,780 overlapping unmet conditions. Formatting, xtask verification, all-target Clippy with warnings denied, and the Wasm check pass on the recorded current source. The complete workspace passes **1,658 tests with five ignored**, with test-target, example and doctest coverage verified. Full M2/API, release-platform and performance completion are not claimed.

The [Stage102 collection checkpoint](reports/m2/stage102-collection-apis-checkpoint.json)
is retained as historical evidence: 3,718 ordinary observations match Java in its
shared Rust and selected target captures, and 1,502 workspace tests pass with four
ignored. Its original Map failure predates the Typed implementation; its required
internal-error contracts and original whole-List Java heap failure remain explicit.

The [Stage66 checkpoint](reports/m2/stage66-implementation-checkpoint.json)
adds String/List/Set reversal, constructor resolution inside constraints, and
union-marker diagnostics. Its 1,081 selected expressions and forty separate
diagnostic modules match Java on native, N-API, and Chromium. The 131-program
original selection has 81 matches; failures, unsupported cases, and two blocked
originals remain explicit. These selected results do not close M2.

The [M2 verification contract](docs/m2-language.md)
keeps the complete language inventory separate from selected executable probes.
The [original module frontend adapter](docs/m2-module-frontend.md) also compares
registered upstream commands with their original inputs, arguments, resources,
and complete process output. The output operation evaluates actual `ModuleOutput`
properties and PCF through the shared VM; the separate frontend-only operation
still stops after preparation. Imports now use typed resolve/source Host requests
and share the evaluator's VM, heap, module cache and declaration identities.
Expression sessions use the common parser and compiler with the actual module
receiver. Module-owned class/alias types, `extends`, and `amends` share that
engine. [Canonical diagnostics](docs/m2-diagnostics.md) preserve UTF-16 strings
through native, Node, and Wasm APIs, including typed Host IOException messages
with distinct absent and empty causes. Java UTF-8 serialization is checked
separately. The exact `output.text` expression request shares the ModuleOutput
operation. The [Stage 46 selected regression](reports/m2/stage46-final-summary.json)
records 3,650 matching native cases out of 3,664 attempts at implementation
`a3eb363`, retaining four unsupported cases, nine mismatches and one oracle error.
All 2,782 persistent evaluator calls across 334 sequence executions match their
declared channels. Three previously unsupported original programs now pass, with
no previously passing case regressing. The strict full-language gate still fails.
The earlier [Stage 45](reports/m2/stage45-final-summary.json) and
[Stage 43/44 records](reports/m2/stage44-final-summary.json) remain unchanged.
The [standard-module checkpoint](docs/m2-stdlib-bootstrap.md) adds supported
source-backed `pkl:base`/`pkl:test` imports, guest exception handlers, ModuleInfo
validation, and typed security-denial completions. Broader bootstrap comparisons
still fail on missing bindings, resource-read counts, and standard-module caches.
Complete modifier and type semantics, remaining collection and Reference rules,
the remaining inheritance and imported-type rules, reflection metadata, and the remaining language corpus are
required before M2 can close.

The [shared JSON renderer](docs/m2-json-rendering.md) adds render directives,
property annotation transforms, collection joining and selected boxed-value
identity rules to the common engine. Its Java fixtures and bounded comparisons
remain separate from the Stage 46 whole-selection result above. Standard-module
admission policies, the JSON parser and complete renderer coverage remain open.

The [DataSize checkpoint](docs/m2-data-size.md) adds shared unit conversion,
value operations, type checks, rendering and related Bytes construction/size
behavior. Its selected string comparisons match Java on native, N-API and
Chromium; the unchanged basic/API DataSize programs also match. The expanded
Stage 57 original selection still had eight mismatches involving Bytes, Duration,
`Any.ifNonNull` and the Pkl Binary test path. Typed exports and complete standard
library coverage remain open; see the [recorded evidence](reports/m2/README.md).

The [Any.ifNonNull checkpoint](docs/m2-any-if-non-null.md) adds shared VM
method lookup and callback behavior, including null defaults and Host lifecycle
checks. Its Stage 58 Java fixtures and selected native comparison retained
four mismatched Regex and IntSeq constructor executions across two repeats.
The Stage 58 N-API and Chromium string comparisons matched 168 of 172 executions
each; their integrity gates passed and strict string gates failed on those rows.
The unchanged `api/any.pkl` original program passes raw comparison. At Stage 58,
ten of 17 original programs passed and seven mismatched. The DataSize native,
N-API, and Chromium string regressions each retain 510 of 510 matches.
Typed exports, full M2, and performance requirements remain open.

The [Duration checkpoint](reports/m2/stage59-implementation-checkpoint.json)
extends the shared engine with unit conversion, arithmetic, comparison,
`isoString`, and method/type diagnostics. Native, N-API, and Chromium each
match all 548 recorded Java string observations and complete session protocols.
The unchanged original selection now has 11 matches, five mismatches, and one
unsupported result: `basic/duration.pkl` newly passes, while `api/duration.pkl`
reaches the missing `pkl:math` module. Bytes and Binary test dependencies remain.
The separate 15-value raw-bit regression checks internal Rust values; typed
product exports, whole-language compatibility, and performance remain open.

The [Bytes checkpoint](reports/m2/stage60-implementation-checkpoint.json) adds
shared hashing, encoding, strict charset decoding, indexing, concatenation,
`toList`, and resumable `List.toBytes` checks. At Stage 60, each of native, N-API,
and Chromium matched 594 of 600 string observations; three IntSeq-dependent cases
failed in both repetitions. The 17 original programs had 12 matches, four
mismatches, and one unsupported result. `invalidBytes4.pkl` newly matches;
the basic/API Bytes programs reach their missing `IntSeq` dependency.
Typed exports, including Java byte-array aliasing, remain unverified.

The [IntSeq checkpoint](reports/m2/stage61-implementation-checkpoint.json) adds
shared sequence operations, streaming generators/spreads, all 17 `pkl:math`
constants, and the List accessors, `add`, and `fold` required by the original
programs. Native, N-API, and Chromium each match all 474 selected IntSeq string
observations. The Bytes regression now matches 600 of 600 per target; Duration
and DataSize retain 548 and 510 matches. Portable Any retains two Regex
mismatches per target. Among 62 unchanged original programs, 26 match, 15
mismatch, and 21 are unsupported. The previous 17-program cohort improves
from 12 to 15 matches, with no passing case regressing. These are selected
results; all nine M2 workstreams still have unmet completion criteria.

The [collection conversion checkpoint](reports/m2/stage62-implementation-checkpoint.json)
adds shared strict/lazy object conversions, optional and defaulted lookups,
Dynamic length, Set addition, and Bytes iteration/spread. Six independent Java
fixtures contain 485 expressions; native, N-API, and Chromium each match all
970 observations across two repetitions, including full UTF-16 and error bytes.
The implementation also separates authored NUL property names from synthetic
surrogate names. Existing scalar regressions remain unchanged, including the
known Regex failures. The expanded 90-program original selection has 35 matches,
22 mismatches, 32 unsupported results, and one Java heap error. Within the prior
62 programs, matches increase from 26 to 30 without a passing case regressing.
These results do not close full M2, typed product APIs, or performance targets.

## Goals

- Compatibility with the Pkl 0.32.0 language, standard library, CLI, REPL, server, and public APIs.
- Identical output bytes, generated files, error messages, source locations, and exit codes for the same inputs and environment.
- Execution time and peak RSS no greater than the Java baseline for each required workload.
- Native, Wasm, and N-API support through a single Rust evaluation engine.
- Coverage and implementation tracking through upstream, Pantry, regression, differential, and performance tests.

The [project intent](INTENT.md) defines goals and scope. The [execution plan (Korean)](PLAN.md) contains the phase overview, work items, dependencies, and phase gates. See the [architecture](docs/architecture.md) for design guidance and [TESTING.md](TESTING.md) for acceptance criteria and verification methods.

## Reference versions

Reference sources are pinned as Git submodules under `external/`. The Git gitlinks and `references.lock` must identify the same commits.

| Path | Purpose | Pinned commit |
| --- | --- | --- |
| `external/pkl` | Pkl 0.32.0 language, API, standard library, and test reference | `e51fb2f91774265666f94980ec630db794a4db2d` |
| `external/pkl-pantry` | Candidate compatibility corpus of real packages and examples | `904a68184e54da378b0a0e8a911a3fab3956f693` |

The pinned Pantry revision defines the corpus inputs. Java baseline acquisition has exercised its 29 package/API commands and 206 generated package modules; configuration-dependent errors and unfixed network inputs remain explicitly recorded. The full command corpus still has blocked requirements and a Java heap failure. See the [M0 evidence report](reports/m0/README.md) for counts and limitations. `../pkl-rs.rev1` is earlier work used as a design and test reference, not a build dependency.

## Getting started

Install Git and [rustup](https://rustup.rs/). The Rust toolchain is pinned in `rust-toolchain.toml`; rustup installs the specified tools. Replace the URL below with the repository URL.

```sh
git -c core.autocrlf=false clone --recurse-submodules <repository-url> pkl-rs
cd pkl-rs
git config --local core.autocrlf false
git submodule foreach --recursive 'git config --local core.autocrlf false'
```

For an existing clone, initialize the reference sources:

```sh
git -c core.autocrlf=false submodule update --init --recursive
git config --local core.autocrlf false
git submodule foreach --recursive 'git config --local core.autocrlf false'
```

Setting `core.autocrlf=false` prevents automatic line ending conversion of reference fixtures on Windows. Explicit upstream `.gitattributes` rules still apply. If an existing checkout has already been converted, preserve any local changes and create a fresh clone using the commands above.

Run the setup checks:

```sh
cargo xtask verify
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo check -p pkl-runtime --target wasm32-unknown-unknown --locked
```

`cargo xtask verify` checks the reference source pins. The [Java oracle](docs/oracle.md) provides pinned Pkl 0.32.0 execution and exact replay checks. The [comparison tool](docs/compatibility-tooling.md) captures raw output and generated files and rejects missing or failing cases. See [TESTING.md](TESTING.md) for tooling commands and [the platform experiment](docs/m1-platforms.md) for actual Node/browser execution.

## Current structure

```text
.cargo/                  Cargo aliases and shared configuration
.github/workflows/       Workspace checks and Java tooling replay
benchmarks/              Initial Java workloads and sampling contract
compatibility/           Source, corpus, smoke, regression, and M2-M6 coverage manifests
crates/pkl/              Public native Rust Evaluator API
crates/pkl-cli/          Unified product `pkl` CLI binary
crates/pkl-doc/          `pkldoc` documentation site generator, migrator, and vendored web assets
crates/pkl-formatter/    Pure-Wasm CST Pkl 0.32.0 source formatter
crates/pkl-host-native/  Native Host I/O, TLS/HTTP, CA roots, modulepath, package cache, and extensions
crates/pkl-napi/         Product Node.js N-API bindings (preserving M1 experimental probes)
crates/pkl-repl/         Interactive PTY and pipe REPL driver
crates/pkl-runtime/      Shared pure-Wasm frontend, VM, heap, stdlib, renderers, Host, REPL, and server core
crates/pkl-server/       MessagePack evaluator server (`pkl server`)
crates/pkl-wasm/         Product WebAssembly bindings (preserving M1 experimental probes)
docs/                    Architecture and verification tool contracts
external/pkl/            Pkl 0.32.0 reference submodule
external/pkl-pantry/     Pantry reference submodule
packages/pkl-napi/       Distributable `@pkl-rs/napi` dual ESM/CJS Node.js package and TypeScript types
packages/pkl-wasm/       Distributable `@pkl-rs/wasm` browser/Node ESM Wasm package, VFS, and TypeScript types
reports/m0/              Baseline summaries and checksummed raw evidence
reports/m1/              Architecture experiment decisions and raw evidence
reports/m2/              Incremental language results and raw evidence
reports/m3/              Standard library, renderers, binary decoder, and output.files checkpoint report
reports/m4/              Host I/O, external readers, projects, packages, and Pantry checkpoint report
reports/m5/              Product APIs, CLI, REPL, server, formatter, and pkldoc checkpoint report
reports/m6/              Wasm and N-API product bindings, cross-target parity, and packaging checkpoint report
tests/fixtures/          Owned smoke inputs, historical regressions, and M2-M6 differential fixtures
tools/                   Python verification, differential harnesses, and Java measurement helpers
xtask/                   Repository verification tool
INTENT.md                Project goals and scope
PLAN.md                  Roadmap, execution plan, and work status (Korean)
references.lock          Reference repositories and revisions
oracle.lock.json         Java artifact, runtime, and execution profiles
rust-toolchain.toml      Rust toolchain version
```

See [CONTRIBUTING.md](CONTRIBUTING.md) for the contribution workflow and [AGENTS.md](AGENTS.md) for instructions for agents working in this repository. This project uses the [Apache License 2.0](LICENSE). Reference repository licenses are included in their respective submodules.
