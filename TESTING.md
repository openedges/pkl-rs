# Testing & Validation Contract

This document defines the complete test inventory and release validation contract for **Apple Pkl `0.32.0`** compatibility and performance.

- **End users**: See [docs/installation.md](docs/installation.md) for installation and checksum verification, and [README.md](README.md) for quick-start examples.
- **Milestone history & recorded evidence**: See [docs/development-history.md](docs/development-history.md) and `reports/m0/`–`reports/m7/`.
- **Release & versioning checklist**: See [docs/versioning.md](docs/versioning.md).

The release goals are behavioral and raw-byte compatibility with **Apple Pkl 0.32.0**, with execution time and peak RSS `<= 1.00x` against the pinned **Official Apple Pkl 0.32.0 `native-image`** executables for all 15 CLI/server/`pkldoc` workloads and against the pinned **Eclipse Adoptium Temurin `21.0.13.1+2-LTS`** JVM (plus the `2026-11-22` browser Wasm contract) for all 19 embedded API and JS/Wasm workloads.

## Verification architecture & complete test inventory

Every test and verification harness in the repository is explicitly accounted for across six layers, split cleanly between low-cost **GitHub Actions** ([ci.yml](.github/workflows/ci.yml) + [release.yml](.github/workflows/release.yml)) and the **local / dedicated verification runner** ([tools/local_verify.py](tools/local_verify.py); full command catalog in [docs/ci.md](docs/ci.md)):

| Layer | Complete Inventory | GitHub Actions | Local Verification (`tools/local_verify.py`) |
| --- | --- | --- | --- |
| **1. Static, pins & manifests** | `cargo xtask verify`, `cargo fmt --check`, `clippy` (`workspace`, `wasm32`), `wasm32` check, 12 milestone manifest `--check` scripts, M0/M7 benchmark contracts, JVM & `native-image` oracle checks | `ci.yml::check-and-test` (offline manifests/contracts; no Java) | `--suite workspace`, `--suite manifests` (includes JVM + `native-image` oracle pin verification) |
| **2. Rust tests** | **379 workspace targets** (~2,500+ tests across 10 crates + `xtask`, incl. `350` integration suites in `crates/pkl-runtime/tests/` and internal `pkl_runtime::java_regex` UTF-16 unit tests) + **2 explicit `#[ignore]` tests** (`frontend::frontend_probe`, `m2_protobuf_java04_full::java04_strict_full_compatibility_gate`) | `ci.yml::check-and-test` runs fast product crates + `pkl-runtime --lib` | `--suite workspace` (all 379 targets), `--suite m1` (`frontend_probe`), `--suite protobuf-strict` (`java04_strict_full_compatibility_gate`) |
| **3. Python tooling tests** | **134 files** under `tools/tests/test_*.py`: **59 active** (13 core + M1 `8` + M2 `22` + M3 `2` + M4 `3` + M5 `4` + M6 `2` + M7 `5`, incl. `check_linux_glibc.py` & workflow contract tests) + **75 historical** M2 fixture/compare suites | `ci.yml::check-and-test` runs `test_local_verify.py` | `--suite manifests` (all **59 active** modules), `--suite py-historical` / `--include-historical-tests` (all **75 historical** modules) |
| **4. Platform & package JS tests** | **41** Node.js adapter/session test files under `tools/platform/*.test.mjs` (**384 tests**) + **3** standalone package test scripts (`packages/pkl-wasm/test/wasm.test.mjs`, `packages/pkl-napi/test/{run,pack-install}.mjs`) + `.wasm` gzip-9 `<= 2.50 MiB` budget + release CLI smoke | `ci.yml::package-smoke` (Ubuntu); `release.yml` (all 7 native targets + 4 Linux distros on `v*` tags / manual dispatch) | `--suite platform-js-tests` (all **41 files / 384 tests**), `--suite packages` (Wasm + N-API + CLI smoke) |
| **5. M0–M6 Java `0.32.0` differential gates** | Raw-byte comparison vs pinned Temurin `21.0.13.1+2-LTS` + `pkl-tools-0.32.0.jar`: M0 smoke/replay/regressions, M1 (`6,078` cross-target + Graal archive check), M2 (`4,575` ready cases), M3 (`1,781` census + `118` diff), M4 (`429` offline + 29 Pantry packages), M5 (`142` product tools), M6 (`80 + 6` Wasm/N-API + 3-browser Playwright smoke) | — (moved to verification environment) | `--preset compat` (add `--full-corpus` for M0 full, M2 `4,575` + `--verify-full`, M4 Pantry all-5) |
| **6. M7 performance, memory & fuzzing gates** | Seeded differential fuzzer (`210 + 5` regressions), 18-pair memory reclamation soak (Native/N-API/Wasm), 34-workload benchmark suite (15 CLI/server/`pkldoc` vs Official `native-image` `<= 1.00x`, 19 embedded/JS/Wasm vs Temurin 21 + browser Wasm contract), cross-browser JS analysis, unified `m7_manifest.py --verify-release` | — (moved to verification environment) | `--preset m7` or `--preset full-strict --local-checkpoint` |

> **Note on `pkl_runtime::java_regex`**: The checked UTF-16 Java regular expression engine and official Unicode 15.0.0 UCD tables live directly inside `crates/pkl-runtime/src/java_regex/` (no excluded vendor crate or `[patch.crates-io]` override), so all regex unit tests, `unsafe_code = "deny"`, and `clippy -D warnings` execute as part of standard workspace checks.

### Quick verification commands

```sh
# 1) Fast local gate (full 378-target Rust + 12 manifests/oracles + 59 active Python + 41 platform JS + packages)
python3 tools/local_verify.py --preset quick

# 2) M0-M6 Java 0.32.0 compatibility differential suites (add --full-corpus for full M2 + Pantry)
python3 tools/local_verify.py --preset compat

# 3) M7 fuzz + soak + Official Apple Pkl 0.32.0 native-image benchmark + JS analysis + release audit
python3 tools/local_verify.py --preset m7 --baseline native-image

# 4) Complete strict local audit (forces --full-corpus, 18-pair soak, 34-workload --baseline both + --verify-strict)
python3 tools/local_verify.py --preset full-strict --local-checkpoint
```

## M2 behavior verification contract

The implementation owners, domain scenarios, Host dependencies and work-item
definition are maintained in [PLAN.md](PLAN.md#m2-구현과-테스트-구조).
Use the existing M2-01–09 workstreams, requirement IDs and original case IDs.
The structure organizes implementation and verification; it grants no new
implementation, mapping, execution or compatibility status.

### Test layers and their claims

| Layer | Required observation | Existing entry points |
| --- | --- | --- |
| Local invariants | Parser/source structure, value/type identity, ownership and GC roots, error provenance, and other implementation invariants affected by the change | Inline runtime tests and focused tests in `crates/pkl-runtime/tests` |
| Shared engine behavior | Values, evaluation order, lazy forcing, repeated calls, failure/retry, Host suspension and cleanup within the same evaluator | Existing runtime/lifecycle tests and the applicable Java sequence probes |
| Original Java differential | Complete original input, invocation, environment, result/error channels and generated files, with the pinned 0.32.0 oracle | `tools/m2_language.py` and its original-module, JVM-profile, historical and selected-regression adapters |
| Platform boundary | Shared semantic behavior through actual native, Node and browser adapters, including declared Host capabilities and lossless transport | `tools/m1_platforms.py`, `tools/platform/compare-language-cases.mjs` and existing scenario-specific runners |
| Generated and aggregate checks | Reproducible seeds, valid metamorphic domains, minimized discrepancies, inventory completeness and strict completion accounting | `tools/m2_generate.py`, `tools/m2_manifest.py`, existing report and strict verifiers |

Select the existing tests that observe the claimed behavior. Add cases for an
unobserved requirement or a discovered regression. Normal, boundary, error,
ordering and lifecycle scenarios are applicable axes, not a mandatory Cartesian
product for every feature. Generated relations must specify their valid domain;
for example, integer associativity under bounded operands does not establish
floating-point associativity. Fuzz findings become retained regression inputs.

### Per-behavior coverage matrix

For each active behavior, keep one row in the existing work item or canonical
checkpoint report. Use the existing requirement IDs, M2-01–09 owner and original
case IDs; this is a review view, not a new manifest or a new completion score.

| Row field | Required review |
| --- | --- |
| Behavior and owner | Name the observable rule and primary existing workstream; cite its requirement IDs and affected implementation boundary |
| Applicable scenarios | Select normal, boundary, error, ordering, lazy/repeated evaluation and lifecycle scenarios that can change this rule; explain a material non-applicable axis |
| Existing evidence | Link original cases, minimized regressions, local invariants, generated relations and platform observations with their source/input identities |
| Required observation | State complete value/error/file channels, ordered events and Host capabilities actually needed to establish the claim |
| Remaining gap | Distinguish test gap (no adequate assertion), execution gap (a defined check not run), mapping gap (evidence not reviewed/linked) and observation gap (executed boundary cannot expose the claimed behavior) |
| Disposition | Record each applicable comparison and unresolved obligation; a passing subset does not close the row or promote unrelated mappings |

For example, the M2-08 error-projection review can retain
`error-remainder-zero`, both `trace-stage37-repeat-empty-*` IDs and the three
`read-stage38-edge-http-*` IDs as distinct original observations under the
existing requirements. Their similar unsupported outcomes do not prove one
implementation cause. The package/Host work item must select actual existing
original package IDs and capabilities before implementation; URI controls do
not supply missing original I/O observations.

Record oracle execution, candidate execution, raw comparison and evidence
integrity separately. An oracle process failure and a candidate timeout can
coexist on the same case. A completed acquisition or successful integrity check
does not make either execution or its compatibility comparison pass.

Apparent duplication is purposeful when it observes a different diagnostic mode,
public API, original invocation/environment, platform transport, ordering or
lifecycle invariant, or preserves an original case alongside its minimized
reproducer. Before consolidating, compare those purposes and complete channels;
preserve original IDs and evidence. Equal input hashes, expressions and expected
outcomes alone do not authorize deletion. The bounded inventory review is
recorded in the [canonical checkpoint](reports/m2/stage156-baseline/README.md).

### Observation boundaries

| Boundary | Comparison contract |
| --- | --- |
| Original CLI/module contract | Preserve input/resources, URI, argv, cwd and environment; compare complete stdout, stderr, exit code, generated-file path set and each file's raw bytes |
| Expression or typed API | Compare the exact public value/error representation supplied by that API. A string-expression result qualifies that boundary; typed value export needs its own observation |
| UTF-16 and UTF-8 | Preserve original Java bytes and any captured code units. A JS-string comparison has its declared encoding contract; it cannot silently replace Java UTF-8 or native OS-path-byte comparison |
| Trace, Host and persistent sessions | Compare ordered requests/events/callbacks, payloads and terminal results where exposed. Exercise retry, reentry, cancellation, timeout and close for stateful behavior; use runtime assertions for internal lifetime invariants |
| Errors | Observe the first failure, complete diagnostic text and applicable source/frame/hint data. Compare caught and uncaught behavior at their original boundaries; missing values or channels remain failures |

Native filesystem/network, Node I/O and browser VFS/loader capabilities must be
identified per execution contract. Missing required capabilities remain blocked
or unimplemented. A shared-engine build or bounded M1 result qualifies only its
recorded scope; full product bindings and release-platform coverage retain their
separate requirements. Relevant performance and memory checks retain the
measurement contract below, including correctness before timing.

### Work-item and checkpoint acceptance

Use focused affected checks while iterating. Group related fixes into a source
checkpoint, then run the applicable required full checks on its final source
state, including platform checks for shared primitives or bindings. Repeat a
passing check when relevant inputs change or an unresolved concern requires it;
documentation publication alone does not require another runtime acquisition.


Before implementing a change, record its behavior, primary workstream,
requirement/case IDs, existing implementation, observed discrepancy or missing
capability, intended source boundary and required observations. Use one active
work item or canonical checkpoint report; no additional manifest format is
required. An unexplored area stays unverified until investigated.

Internal invariants may use Rust assertions. Observable compatibility expected
results come from the pinned Java oracle. Tooling tests for missing results,
duplicate IDs, stale identities and byte differences belong to the shared tooling
suite and need not be duplicated for each language feature.

Close a work item after its original/regression comparisons, affected boundary
tests, applicable platform checks, reviewed source/test links and the required
development checks pass on the qualified source state. Preserve prior failures
and any explicitly unresolved scope. Reuse unchanged evidence only with its
original source, input, artifact, environment and comparison-boundary identities;
do not present an old observation as a new execution.

The full M2 gate still requires every required implementation/test mapping and
complete passing execution, together with recorded build and applicable platform
verification. A successful work item does not waive unselected cases, known
failures, oracle errors, ignored tests or missing adapters. The design tables
themselves are not new runtime evidence.

## M3 standard library and output verification contract

Milestone M3 verifies the complete Pkl `0.32.0` standard library, external builtin, renderer, binary decoder, and `output.files` surface using two independent automated gates that never treat missing, deferred, or unexecuted items as passes:

1. **Mechanical 1,781-Item Coverage Gate (`tools/m3_manifest.py`, `crates/pkl-runtime/examples/m3_coverage_probe.rs`, `compatibility/m3-mappings.json`, `compatibility/m3-stdlib-output.json`)**:
   - Reads every `stdlib` (`1,222`), `builtin` (`542`), and `renderer` (`17`) entry directly from `compatibility/inventory.json` and emits a deterministic 560-row TSV probe plan (`542` builtins + `17` inventory renderers + synthetic `pkl-binary` `output-format`).
   - Checks byte-for-byte SHA-256 parity between `external/pkl/stdlib/*.pkl` and `crates/pkl-runtime/src/stdlib/*.pkl` across all 24 upstream files, loads and reflects all 23 `pkl:*` runtime modules live via `pkl:reflect`, and probes every external builtin and renderer class/output-format in the shared Rust runtime.
   - Explicitly accounts for `external/pkl/stdlib/doc-package-info.pkl` (`8` declarations, excluded from runtime `pkl:*` modules by upstream `pkl-core.gradle.kts:106`) as `deferred` to **M5 (`pkldoc`)** rather than counting it as `implemented`.
   - `tools/m3_manifest.py --check` verifies manifest/mapping/fixture freshness only (`coverage_gate: null`, never claiming runtime passes); `--run-probe <bin>` and `--verify-coverage <census.json>` enforce 1:1 zero-omission coverage (`failure_count == 0`).
2. **3-Lane Raw-Byte & Generated-File Differential Gate (`tools/m3_differential.py`, `tests/fixtures/m3/cases.json`, `tools/java/M3PklBinaryDecodeProbe.java`, `crates/pkl-runtime/examples/m3_pklbinary_decode_probe.rs`)**:
   - Reuses `tools/oracle.py` (pinned Eclipse Adoptium Temurin `21.0.13.1+2-LTS`, `exact_jdk_lock_match: true`) and `tools/compat.py` across **118 cases** in 3 lanes: `formats-renderers` (`77`), `output-files` (`13`), and `pklbinary-decode` (`28`).
   - Compares complete raw `stdout`, `stderr`, `exit_code`, and generated file path sets + raw bytes without trimming, sorting, or path normalization: each case runs Java and Rust sequentially against an identical freshly wiped & re-staged output directory path, checks declared setup symlinks while rejecting undeclared symlinks, and captures any parent-directory escape writes under `escaped/...`.
   - Seals all evidence files under SHA-256 in `report.json` with strictly separate verifiers: `--verify-report <dir>` verifies integrity while preserving recorded failures, whereas `--verify-strict <dir>` additionally requires `100%` `pass`, `exact_jdk_lock_match == true`, and `source_binding == "recorded-build"`.
   - Unit and tamper-detection tests live in `tools/tests/test_m3_manifest.py` and `tools/tests/test_m3_differential.py`.

## M4 modules, resources, projects, and packages verification contract

Milestone M4 verifies Pkl `0.32.0` Host I/O, HTTP/HTTPS/CA/redirects/headers/rewrites/proxy/`noProxy`, security allowlists & `rootDir`, `modulepath:` (directories + JAR/ZIP), external module & resource readers (`0x26–0x32` MessagePack + OS process lifecycle), `PklProject`/settings precedence/MVS resolution/package creation/`package-2` caching, and real Apple Pkl Pantry packages in **both** development-tree form and packaged/distributed form via two independent, strictly offline gates:

1. **Mechanical M4 Inventory & Provenance Gate (`tools/m4_manifest.py`, `compatibility/m4-mappings.json`, `compatibility/m4-io-packages.json`)**:
   - Accounts with zero silent omissions for all M4-scoped `compatibility/inventory.json` categories (`resource` `27`, `protocol_tag` `19`, `protocol_field` `119`, `cli_command` `17`, `cli_option` `86`) plus `104` functional stdlib items across `EvaluatorSettings.pkl`, `Project.pkl`, `analyze.pkl`, `settings.pkl`, and `test.pkl`, separating M5-deferred and 2026-11-20 excluded entries explicitly so they never count as passes.
   - Maps all **16** M4 requirement groups to verified Rust `implementation_links` and concrete static/dynamic differential cases (`429` total), verifies byte-for-byte SHA-256 parity between `crates/pkl-host-native/assets/PklCARoots.pem` and upstream `PklCARoots.pem`, and cryptographically validates all 7 offline remote Pantry packages inside `reports/m0/pantry-evidence.tar.gz`.
   - `tools/m4_manifest.py --check` checks manifest, mapping, fixture, and asset freshness only (`coverage_gate: null`, never claiming passes); `--verify-oracle-coverage <dir>` validates Java-only oracle recordings (`strict: false`), while `--verify-coverage <dir>` enforces `100%` passing strict differential coverage across all 16 requirement groups and all 429 cases (`strict: true`, `failure_count: 0`).
2. **Offline 5-Lane Raw-Byte Differential Gate (`tools/m4_differential.py`, `tools/m4_pantry_offline.py`, `tools/java/M4HttpFixtureServer.java`, `tests/fixtures/m4/cases.json`, `tests/fixtures/m4/readers/{msgpack_ipc,m4_ext_reader}.py`)**:
   - Builds and seals `cargo build --release -p pkl-host-native --bin pkl_m4_cli --locked` as a `recorded-build` candidate, snapshots all Rust runtime/host-native/regex sources before and after the run (`source_stability`), and executes **429 cases** across 5 lanes against pinned Eclipse Adoptium Temurin `21.0.13.1+2-LTS` (`exact_jdk_lock_match: true`, `stdin=DEVNULL` with POSIX process-group cleanup, zero internet calls):
     - `host-io-http-security` (`48`), `external-readers` (`11`, verifying `0x32` graceful close and zero orphan PIDs), `projects-packages-cache` (`27`), `pantry-dev` (`126` across all 29 Pantry packages: `29` `package-api-tests`, `44` `project-test`, `1` `module-names`, `51` examples, `1` `pkl.tar` shell pipeline; with `pkl.experimental.structuredRead`'s 2 pinned GitHub blobs verified by SHA-256 and served from localhost HTTPS via `--http-rewrite`), and `pantry-packaged` (`217` across all 29 generated Pantry packages: `206` `package://` modules, `4` `-m` multi-file retries, `2` configured generators, and `5` cold-cache localhost HTTPS mirror targets asserting server hits > 0).
   - Compares raw `stdout`, `stderr`, `exit_code`, and captured files without trimming, sorting, or path normalization. For `pkl project package` (`comparison_mode: "package-archive"`), pure-Rust Wasm DEFLATE bitstreams may differ from OpenJDK C zlib on non-empty ZIPs; the harness records `raw_zip_bytes_match` honestly while strictly verifying exact stdout/stderr/exit, exact `DependencyMetadata` JSON structure/key order (`java_hashmap_order`)/formatting/no-trailing-newline (aligning only verified self-zip and co-packaged local-dep metadata SHA-256s, never remote deps), exact ZIP entries/CRCs/DOS `1980-02-01` mtime/no-dir-entries, exact empty-ZIP SHA-256 (`8739c76e681f900923b900c9df0ef75cf421d39cabb54650c4b9ad19b6a76d85`), `.sha256` sidecars, two-run idempotency, and live pinned Java `0.32.0` cross-interop loading Rust-built archives.
   - Seals all evidence under SHA-256 in `report.json` with strictly separated `--verify-report` (integrity check preserving recorded failures, cross-checking candidate artifact/build logs, `cross_interop_detail`, and mirror hit assertions) and `--verify-strict` (requiring `100%` pass, `recorded-build`, `exact_jdk_lock_match == true`, `sources_snapshot_sha256` present with `source_stability == true`, and cold-mirror hits > 0).
   - Unit, server, IPC, and tamper-detection tests live in `tools/tests/test_m4_{manifest,differential,servers}.py`.

## M5 product APIs and tools verification contract

Milestone M5 verifies the complete Pkl `0.32.0` product surface — public Rust Evaluator API (`crates/pkl`), full product `pkl` CLI (`crates/pkl-cli`: `eval`, `repl`, `server`, `test`, `project`, `download-package`, `analyze`, `format`, `run`, `shell-completion`), interactive and pipe REPL (`crates/pkl-repl`), `0x20–0x32` MessagePack evaluator server (`crates/pkl-server`), pure-Wasm CST formatter (`crates/pkl-formatter`), and `pkldoc` documentation site generator & migrator (`crates/pkl-doc` + 17 vendored static web assets + `stdlib/doc-package-info.pkl`) — via two independent, strictly offline gates:

1. **Mechanical M5 Inventory, Mapping & Asset Gate (`tools/m5_manifest.py`, `compatibility/m5-mappings.json`, `compatibility/m5-product-tools.json`)**:
   - Accounts with zero silent omissions for all M5-scoped `compatibility/inventory.json` categories (`cli_command` `17`, `cli_option` `86`, `cli_argument` `6`, `protocol_tag` `19`, `protocol_field` `119`, `resource` `27`), `121` functional stdlib items across `Command.pkl`, `DocPackageInfo.pkl`, `DocsiteInfo.pkl`, `EvaluatorSettings.pkl`, `settings.pkl`, and `test.pkl`, plus the `8` `stdlib/doc-package-info.pkl` items deferred from M3, while explicitly accounting for 2026-11-20 excluded Java/Kotlin codegen & JVM adapter census items (`java_api` `8,239`, `kotlin_api` `597`, `2` codegen commands, `19` codegen options, `2` JVM SPI resources) so they are neither dropped nor counted as passes.
   - Maps all **16** M5 requirement groups to **75** verified Rust `implementation_links` across all 8 workspace crates (`pkl`, `pkl-cli`, `pkl-doc`, `pkl-formatter`, `pkl-host-native`, `pkl-repl`, `pkl-runtime`, `pkl-server`) and checks byte-for-byte SHA-256 parity across all **17/17** `crates/pkl-doc/assets/**` static web resources against `external/pkl/pkl-doc/src/main/resources/org/pkl/doc/**`.
   - `tools/m5_manifest.py --check` checks manifest, mapping, fixture, and asset freshness only (`coverage_gate: null`, never claiming passes); `--verify-oracle-coverage <dir>` validates Java-only oracle recordings (`strict: false`), while `--verify-coverage <dir>` enforces `100%` passing strict differential coverage across all 16 requirement groups, all 142 cases, and all 17 exact-match assets (`strict: true`, `failure_count: 0`).
2. **Offline 5-Lane Differential Gate (`tools/m5_differential.py`, `tools/m5_pkldoc_tree.py`, `tools/m5_repl_driver.py`, `tools/m5_server_client.py`, `tools/java/M5ApiOracleProbe.java`, `tests/fixtures/m5/cases.json`)**:
   - Builds and seals all 3 `recorded-build` release candidates (`pkl`, `pkldoc`, `pkl_m5_api_probe`), snapshots all `429` Rust/manifest/asset sources before and after execution (`source_stability`), verifies `M4HttpFixtureServer.self_test()` (`7/7`), and executes **142 cases** (`112` static + `30` dynamic upstream formatter corpus snippets) across 5 lanes against pinned Eclipse Adoptium Temurin `21.0.13.1+2-LTS` (`exact_jdk_lock_match: true`, zero internet access):
     - `rust-api-and-cli` (`50`: `13` `m5:api:*` paired against `M5ApiOracleProbe.java` + `37` `m5:cli:*`), `repl` (`12`), `server` (`22`), `formatter` (`43`: `30` corpus snippets with two-pass idempotency + `13` CLI modes), and `pkldoc` (`15`: docsite generation, incremental updates, older-version preservation, `--no-symlinks`, `stdlib/doc-package-info.pkl`, release mode, `--migrate`, `--help`, `--version`, and 4 error paths).
   - Uses raw-byte comparison across `stdout`, `stderr`, `exit_code`, and generated file/symlink trees (`raw`: `130` cases), with only two narrow, explicitly recorded comparison modes:
     - `native-version` (`3` cases: `m5:cli:version-long`, `m5:cli:version-short`, `m5:pkldoc:version`): verifies exact exit code `0`, empty `stderr`, exact Pkl `0.32.0` + OS string, and honest `native` flavor (`NATIVE_VERSION_RE`) without spoofing `Java <ver>`.
     - `pkldoc-progress` (`9` `pkldoc` generation/migration cases): records `raw_stdout_bytes_match` honestly (upstream Java `newVirtualThreadPerTaskExecutor`/`Files.walk` emits nondeterministic `\r` progress ordering even across back-to-back Java runs) while strictly enforcing **100% raw byte-for-byte parity on every generated regular file and symlink**, exact raw `stderr`, exact `exit_code`, exact non-progress milestone lines in order, and exact URI-encoded progress-segment multisets.
   - Seals all evidence under SHA-256 in `report.json` with strictly separated `--verify-report` (integrity check preserving recorded failures) and `--verify-strict` (requiring `100%` pass, all 3 `recorded-build` candidates, `exact_jdk_lock_match == true`, `source_stability == true`, and `server_self_test.passed == true`).
   - Unit, codec, tree-comparison, and tamper-detection tests live in `tools/tests/test_m5_{manifest,differential,repl_driver,server_client}.py`.

## M6 Wasm and N-API product verification contract

Milestone M6 verifies the complete product WebAssembly (`crates/pkl-wasm`, `packages/pkl-wasm` as `@openedges/pkl-wasm@0.32.0`) and Node.js N-API (`crates/pkl-napi`, `packages/pkl-napi` as `@openedges/pkl-napi@0.32.0`) bindings, four-way cross-target raw-byte parity (Java 0.32.0 == Native == N-API == Wasm Node == Wasm Browser), clean-directory packaging and installation, and explicit capability-gap honesty via two independent, strictly offline gates while preserving 100% of existing M1 exports:

1. **Mechanical M6 Manifest & Requirement Coverage Gate (`tools/m6_manifest.py`, `compatibility/m6-mappings.json`, `compatibility/m6-wasm-napi.json`)**:
   - Defines the unified public JS/TS API contract (`19` common exports, `@openedges/pkl-wasm` exports `init`/`VirtualFileSystem`, `@openedges/pkl-napi` export `Project`), the explicit `19`-key capability matrix across `wasm-browser`, `wasm-node`, and `napi-node`, and all **14** M6 requirement groups linked to **83** verified Rust/JS `implementation_links` spanning both the M6 product crates/packages (`crates/pkl-{wasm,napi}`, `packages/pkl-{wasm,napi}`) and the shared/native crates (`crates/{pkl-runtime,pkl-formatter,pkl-host-native,pkl,pkl-cli,pkl-doc}`).
   - Separates executable compatibility cases (`80`) from explicit capability-boundary cases (`6`: `4` in `wasm-browser`, `2` in `wasm-node`) in manifest accounting so missing platform capabilities are never counted as compatibility passes.
   - `tools/m6_manifest.py --check` checks manifest, mapping, fixture, and runner freshness only (`coverage_gate: null`, never claiming passes); `--verify-coverage <dir>` enforces `100%` passing executable coverage (`80/80`), exact `capability-blocked` accounting (`6/6`), and all 14 linked requirement groups (`strict: true`, `failure_count: 0`).
2. **Offline 5-Lane Differential, Packaging & Capability Gate (`tools/m6_differential.py`, `tests/fixtures/m6/{cases.json,oracle/M6OracleProbe.java,runners/*.mjs}`)**:
   - Builds and seals all 4 `recorded-build` candidates (`pkl`, `pkldoc`, `wasm_pkg.tgz` [`13` tarball files], `napi_pkg.tgz` [`6` tarball files]), snapshots all `469` Rust/JS/manifest/fixture source files before and after execution (`source_stability`), verifies `M4HttpFixtureServer.self_test()` (`7/7`), and executes **86 cases** completely offline against pinned Eclipse Adoptium Temurin `21.0.13.1+2-LTS` (`exact_jdk_lock_match: true`) and pinned Playwright `1.64.1` Chromium `156.0.8078.4` (`browserErrors: []`):
     - `wasm-browser` (`33`: `29` executable + `4` capability-boundary), `wasm-node` (`10`: `8` executable + `2` capability-boundary), `napi-node` (`26`), `cross-target-parity` (`12`, asserting raw byte parity across Native, N-API, Wasm Node, Wasm Browser, and Java 0.32.0), and `packaging-and-install` (`5`: native `pkl`/`pkldoc` binaries, standalone downstream Rust crate linking `crates/pkl`, packed `@openedges/pkl-wasm` clean install in Node & Chromium, packed `@openedges/pkl-napi` clean install in ESM & CJS, and platform support matrix metadata).
   - Reports `compatibility_pass_count` (`80`) and `capability_blocked_count` (`6`) strictly separately; capability-boundary cases must fail deterministically with structured `PklError` (`code: "capability-unavailable"`) and are never folded into `compatibility_pass_count`.
   - Records JS<->Rust conversion latency telemetry (`m6:napi-node:conversion-telemetry`, `m6:wasm-browser:conversion-telemetry`, `398 B` payload) as M6 observability baselines (separate from M7's `<= 1.00` JVM release throughput/RSS gate).
   - Seals all evidence under SHA-256 in `report.json` with strictly separated `--verify-report` (integrity check preserving recorded failures) and `--verify-strict` (requiring `80/80` executable passes, `6/6` `capability-blocked`, all 4 `recorded-build` candidates, `exact_jdk_lock_match == true`, `source_stability == true`, `server_self_test.passed == true`, and `browserErrors == []`).
   - Unit and tamper-detection tests live in `tools/tests/test_m6_manifest.py` and `tools/tests/test_m6_differential.py`.

## M7 full release verification contract

Milestone M7 adds four automated release-verification harnesses, a lean Ubuntu-only packaging/smoke CI workflow (`.github/workflows/ci.yml`), a tag/manual cross-platform release packaging workflow (`.github/workflows/release.yml`), and a unified local/verification-environment runner (`tools/local_verify.py`) while strictly separating manifest freshness (`--check`, `coverage_gate: null`), report integrity (`--verify-report`), local strict gates (`--verify-strict`), and full multi-platform release readiness (`--verify-release`):

1. **Performance & Peak RSS Benchmark Gate (`tools/m7_benchmark.py`, `tools/m7_js_analysis.py`, `tools/oracle.py`, `oracle.lock.json`, `benchmarks/m7_workloads.json`, `benchmarks/m7_fixtures/**`, `crates/pkl/examples/m7_bench_probe.rs`)**:
   - Benchmarks all **11 families / 34 workload-modes**: the 6 core workloads (`scalar`, `stdlib-collections`, `recursive-compute`, `large-output`, `failure-diagnostics`, `import-graph`) across `cold-cli`, `persistent`, and `fresh-evaluator`, plus `reflection-stdlib` (`cold-cli`, `persistent`), `renderers-files` & `renderers-binary`, `packages-http-warm` & `packages-http-cold` (via offline `M4FixtureServers`), `repl-session`, `server-session` (`steady-server`), `formatter-large`, `pkldoc-single-package`, `napi` (`eval-text`, `typed-conversion`), `wasm-node` (`eval-text`, `typed-conversion`), and real Playwright browser `wasm-browser` (`eval-text`, `typed-conversion` under COOP/COEP `cross_origin_isolated: true`).
   - Enforces **100% raw-byte stdout/stderr/exit/file correctness verification against pinned Pkl 0.32.0 BEFORE timing**, builds and seals per-artifact `recorded-build` binaries (`pkl`, `pkldoc`, `m7_bench_probe`, `@openedges/pkl-napi`, `@openedges/pkl-wasm` built via `[profile.wasm-release]`) alongside pre/post source snapshots (`source_stability`), measures real per-process peak RSS via `os.wait4` `ru_maxrss` (plus full browser descendant-tree `/bin/ps` RSS, per-role breakdown, Wasm linear memory, JS heap, and `.wasm` gzip-9 size for browser runs), and supports `--baseline {both,native-image,jvm}` (default `both`, required for `--verify-strict`).
   - Compares head-to-head against **pinned Official Apple Pkl 0.32.0 `native-image`** (`oracle.lock.json::native_image`, verified by `tools/oracle.py::ensure_native_image_oracle` with `exact_native_image_lock_match: true`, supporting `PKL_ORACLE_NATIVE_PKL`/`PKL_ORACLE_NATIVE_PKLDOC` or `.cache/oracle/native-image-0.32.0/{pkl,pkldoc}-macos-aarch64`) AND live pinned **Eclipse Adoptium Temurin `21.0.13.1+2-LTS`** (`exact_jdk_lock_match: true`, NO `PKL_ALLOW_LOCAL_JDK=1`), plus archived **M0 GraalVM CE `25.0.1`** (`reports/m0/benchmarks.json`) recorded separately.
   - Evaluates per-workload targets independently without cross-workload averaging:
     - **All 15 CLI / Server / `pkldoc` workloads (`cold-cli` and `steady-server`, `2026-11-23` contract in `benchmarks/m7_workloads.json::native_image_targets`)**: **Primary strict gate** is `vs_official_native_image` `time_ratio <= 1.00` AND `os.wait4` peak `rss_ratio <= 1.00` against pinned Official Apple Pkl 0.32.0 `native-image`, while also passing `vs_live_temurin_21` (`<= 1.00` time & RSS) and preserving M0 GraalVM CE `25.0.1` cross-checks on Core 6. On `repl-session`, official native-image `pkl repl` may emit benign JLine history log lines to stderr; the harness validates they contain only JLine history messages and records `native_image.raw_stderr_bytes_match` / byte counts verbatim, while Rust `pkl repl` remains strictly verified for `stderr == b""` and 100% raw-byte stdout parity.
     - **All 13 Embedded API (`persistent`, `fresh-evaluator`), 2 Node `@openedges/pkl-napi`, and 2 Node `@openedges/pkl-wasm` workloads**: Official `native-image` ships CLI executables only (`pkl`, `pkldoc`), so these 17 non-browser workloads remain strictly gated at `time_ratio <= 1.00` AND full-process `os.wait4` `rss_ratio <= 1.00` against live pinned Temurin 21 JVM (plus M0 GraalVM CE `25.0.1` cross-check for Core 6).
     - **Browser `@openedges/pkl-wasm` (`2/2`, `2026-11-22` explicit contract in `benchmarks/m7_workloads.json::browser_wasm_targets`)**: `time_ratio <= 1.00` vs pinned JVM for Chromium and WebKit (and Firefox `eval-text`; explicit Firefox SpiderMonkey `typed-conversion` exception `<= 1.50` only), `wasm_linear_memory_ratio <= 0.15` vs Java peak RSS, `js_heap_used_bytes <= 64 MiB` (where exposed), and `wasm_binary_gzip_bytes <= 2.50 MiB`, while preserving raw multi-process browser-tree RSS and per-role RSS verbatim in every report.
   - `tools/m7_benchmark.py --check-contract` validates the workload catalog, `native_image_targets`, and `oracle.lock.json::native_image` SHA-256/byte pins, rejecting any unauthorized threshold relaxation (including any `> 1.00` CLI/server/`pkldoc` threshold or non-Firefox browser time exception); `--verify-report <dir>` verifies report/source/evidence/oracle-lock integrity while preserving recorded failures; `--verify-strict <dir>` requires `baseline_mode == "both"`, `exact_native_image_lock_match == true`, `exact_jdk_lock_match == true`, `recorded-build`, `source_stability == true`, `15/15` CLI/server/`pkldoc` passing `vs_official_native_image` (`<= 1.00x` time & RSS), and `34/34` raw-verified converged `pass` verdicts under `vs_live_temurin_21` + browser Wasm contract.
2. **Long-Running Memory Reclamation Soak Gate (`tools/m7_memory_soak.py`, `crates/pkl/examples/m7_memory_soak_probe.rs`, `tests/fixtures/m7/soak/run_{napi,wasm}_soak.mjs`)**:
   - Exercises all **6 scenarios** (`repeated-success`, `repeated-failure`, `unique-sources-churn`, `cyclic-graphs`, `lifecycle-cancel-timeout-close`, `cache-churn`) across **3 targets** (`native`, `napi-node`, `wasm-node` = **18 target-scenarios**).
   - Verifies bounded RSS growth ratio, second-half OLS slope/delta/`R^2`, bounded Wasm linear memory pages (`second_half_delta_pages`, `second_half_slope_bytes_per_iter`), zero FD leaks, zero orphan child PIDs, and direct `SourceSession` GC reclamation assertions (`heap_probe`) on `native/cyclic-graphs`.
   - `--verify-report <dir>` checks integrity and artifact/source seals; `--verify-strict <dir>` requires `18/18` `bounded` verdicts (`0` leaks, `0` errors).
3. **Seeded Differential Fuzzing Gate (`tools/m7_fuzz.py`, `tests/fixtures/m7/fuzz-regressions/`)**:
   - Deterministic `splitmix64` seeded generator (`0x4d37504b4c303332`) across all **7 categories** (`expressions-numerics`, `strings-regex-glob`, `objects-amendments-generators`, `collections-units`, `types-closures`, `multi-format-rendering` across all 7 render formats + `m3_pklbinary_decode_probe` round-trip, and `formatter-fuzz` raw parity + 2nd-pass idempotency) plus all saved regressions (`reg_01`–`reg_05`), with automatic delta shrinking for any discrepancy.
   - `--verify-report <dir>` checks evidence integrity and coverage; `--verify-strict <dir>` requires `0` raw-byte mismatches, `0` errors, `recorded-build`, `exact_jdk_lock_match == true`, and `source_stability == true`.
4. **Unified Release Manifest, Lean CI/Release Workflows & Local Verification Runner (`tools/m7_manifest.py`, `tools/local_verify.py`, `compatibility/m7-mappings.json`, `compatibility/m7-release.json`, `.github/workflows/ci.yml`, `.github/workflows/release.yml`)**:
   - Aggregates all 6 prior milestone manifests (`M0`, `M2`–`M6`) with 12 linked M7 requirement groups, 11-entry platform/browser/hosted-CI matrix (`darwin-arm64`, `darwin-x64`, `linux-x64-gnu`, `linux-arm64-gnu`, `linux-x64-musl`, `linux-arm64-musl`, `win32-x64-msvc`, `browser-chromium`, `browser-firefox`, `browser-webkit`, `hosted-ci`), 3 explicit `2026-11-20` exclusions, and the promoted pinned required release baseline (`pinned_native_image_reference`, `status: "pinned-required-release-baseline"`, covering all 15 CLI/server/`pkldoc` workloads; `unpinned_optional_references: []`).
   - `.github/workflows/ci.yml` executes only low-cost Ubuntu checks, manifest freshness, fast Rust unit/product tests, and packaging smoke (`@openedges/pkl-wasm`, `@openedges/pkl-napi`, `pkl`, `pkldoc`); `.github/workflows/release.yml` executes cross-platform packaging, multi-distro smoke, `SHA256SUMS.txt` generation, and optional GitHub Release/npm publishing on `v*` tags or manual dispatch. All expensive compatibility, differential, cross-browser, fuzz, soak, and benchmark suites execute in the local or dedicated verification environment via `tools/local_verify.py` (with provenance recorded as `local-verification-environment-not-hosted-ci`, never relabeled as hosted CI).
   - `tools/m7_manifest.py --check` checks freshness and symbol links only (`coverage_gate: null`); `--verify-release <dir>` runs integrity and strict gates across benchmark (including explicit `unmet_performance_workloads_official_native_image` enumeration), soak, and fuzz reports and sets `release_ready: true` ONLY when all strict local gates AND all 11 release matrix targets pass cleanly.
   - Tooling unit, workflow-separation, and tamper-detection tests live in `tools/tests/test_oracle.py`, `tools/tests/test_local_verify.py`, and `tools/tests/test_m7_{benchmark,memory_soak,fuzz,manifest,js_analysis}.py`.

## Current recorded baseline

### `2026-11-23` Official Apple Pkl `0.32.0` `native-image` parity & lean CI/local verification checkpoint (local macOS `arm64`, `release_ready: false`)

On local macOS `arm64` (`darwin-arm64`, `rustc 1.98.1`, **NOT hosted CI**), the `2026-11-23` performance and CI-split checkpoints record:
- **Official Apple Pkl `0.32.0` `native-image` benchmark (`tools/m7_benchmark.py --profile full --baseline native-image`, `--verify-report` Exit `0`, `exact_native_image_lock_match: true`, `correctness_verified: 15`)**: **`15 / 15 PASS`** across all CLI, server, and `pkldoc` workloads — every workload achieves **`<= 1.00x` execution time (`0.055x–0.935x`) and `<= 1.00x` `os.wait4` peak RSS (`0.136x–0.662x`)** against SHA-256 pinned `pkl-macos-aarch64` (`94.7 MiB`) and `pkldoc-macos-aarch64` (`94.7 MiB`), plus **`6 / 6 PASS`** vs archived M0 GraalVM CE `25.0.1` (`scalar` `0.628x/0.468x`, `stdlib-collections` `0.371x/0.281x`, `recursive-compute` `0.411x/0.136x`, `large-output` `0.441x/0.275x`, `failure-diagnostics` `0.599x/0.465x`, `import-graph` `0.618x/0.474x`, `reflection-stdlib` `0.904x/0.662x`, `renderers-files` `0.704x/0.519x`, `renderers-binary` `0.742x/0.662x`, `packages-http-warm` `0.457x/0.329x`, `packages-http-cold` `0.669x/0.309x`, `repl-session` `0.055x/0.330x`, `server-session` `0.279x/0.153x`, `formatter-large` `0.493x/0.162x`, `pkldoc-single-package` `0.935x/0.650x`).
- **Frozen-source regression & tooling checks (`6bdae09a1`..`352df2c9b`)**: `cargo xtask verify`, `cargo fmt --all -- --check`, workspace/`java_regex`/`wasm32` `clippy -D warnings`, `wasm32` checks, full Rust workspace tests (**`378/378` targets + `pkl-host-native` `9/9`, `0` failed**), `@openedges/pkl-wasm` (`10/10 PASS`, `pkl_wasm_bg.wasm` `8,022,629 B` raw / **`2,619,729 B` [`2.498 MiB`] gzip-9 `<= 2.50 MiB`**), `@openedges/pkl-napi` (`11/11 PASS`), all **41** `tools/platform/*.test.mjs` files (**`384/384 PASS`**), all **59** active Python test modules (**`525/525 PASS`**), all **12/12** milestone manifest `--check` scripts, both benchmark contracts, `tools/oracle.py verify --profile native-image`, and YAML/contract validation of `.github/workflows/{ci,release}.yml` all pass.
- **Preserved limits (`release_ready: false`)**: Non-macOS OS/architecture release targets (`darwin-x64`, `linux-x64-gnu`, `linux-arm64-gnu`, `linux-x64-musl`, `linux-arm64-musl`, `win32-x64-msvc`), post-optimization Firefox/WebKit browser runs, unified `--baseline both` 34-workload verification, and hosted CI execution remain unverified.

### Prior M7 local macOS `arm64` frozen verification checkpoint (`#154`/`#156`, Temurin 21 baseline)

The [M7 local macOS arm64 verification checkpoint](reports/m7/README.md) records the complete frozen-source (`#154`/`#156`) verification of Milestone **M7** on local macOS `arm64` (`darwin-arm64`, `macOS-27.2-arm64`, Apple M1, `rustc 1.98.1`, **NOT hosted CI**) against pinned **Eclipse Adoptium Temurin `21.0.13.1+2-LTS`** (**`exact_jdk_lock_match: true`**, without `PKL_ALLOW_LOCAL_JDK=1`) plus separate cross-comparison against archived M0 **GraalVM CE `25.0.1`** (`reports/m0/benchmarks.json`):
- **M7 Gate Outcomes (Frozen `#154`/`#156` Tree)**:
  - **Memory Reclamation Soak (`target/m7/m7-final-soak-03`, `--profile standard`)**: **`18 / 18 bounded` STRICT PASS** (`--verify-report` and `--verify-strict` Exit `0`; `native` `6/6`, `napi-node` `6/6`, `wasm-node` `6/6`; `0` RSS/FD/PID leaks, `0` second-half Wasm page growth on all 6 `wasm-node` runs, direct `SourceSession` `heap_probe` `live: 122->122->122`, `reclaimed: 438->2,278->4,118`, `assertions_ok: true`).
  - **Seeded Differential Fuzzing (`target/m7/m7-final-fuzz-03`, seed `0x4d37504b4c303332`)**: **`215 / 215 PASS` STRICT PASS** (`--verify-report` and `--verify-strict` Exit `0`; `210` generated across all 7 categories + `5/5` saved & fixed regressions `reg_01`–`reg_05`; `0` raw-byte mismatches across 7 render formats, `pkl-binary` decode round-trip, and `pkl format -` raw parity + 2nd-pass idempotency).
  - **Performance & Peak RSS Benchmarks (`target/m7/m7-final-bench-02`, `--profile full`)**: **`34 / 34` raw-byte verified & converged**, **`34 / 34` faster than Temurin 21** (`0.022x`–`0.904x`), **`32 / 34 PASS`** (**`32 / 32` non-browser PASS** on time & `os.wait4` peak RSS: `28/28` Native + `2/2` N-API + `2/2` Wasm-Node), **`18 / 18 PASS`** vs archived M0 GraalVM CE `25.0.1`; exact `2/34` failures on structural Chromium multi-process browser-tree RSS (`wasm-browser-eval-text` `5.415x`, `wasm-browser-typed-conversion` `5.581x`). `--verify-report` Exit `0`; `--verify-strict` Exit `1`.
  - **Unified Release Gate (`tools/m7_manifest.py --verify-release`)**: Exit **`1`** (`integrity_passed: true`, `soak_strict_passed: true`, `fuzz_strict_passed: true`, `benchmark_strict_passed: false`, `local_strict_gates_passed: false`, `release_ready: false`, `verified_platforms: []`, `observed_non_passing_platforms: ["browser-chromium", "darwin-arm64"]`, `unverified_platforms: ["darwin-arm64", "darwin-x64", "linux-x64-gnu", "linux-arm64-gnu", "win32-x64-msvc", "browser-chromium", "browser-firefox", "browser-webkit", "hosted-ci"]`).
- **Frozen-Source Cross-Milestone Regression Outcomes (`#185`)**: `cargo xtask verify`, `cargo fmt --all -- --check`, workspace/`java_regex`/`wasm32` `clippy -D warnings`, `wasm32` checks (`pkl-runtime`, `pkl-formatter`, `pkl-wasm`), `tools/oracle.py verify`, all **12/12** Python `--check` manifests, `tools/benchmark.py --check-contract`, and `tools/m7_benchmark.py --check-contract` exit `0`. Full Rust workspace tests pass **`2,471` outer passed across `378` targets (`369` test + `9` doc-tests); `0` failed; `2` ignored** (honest macOS sandbox loopback split on `pkl-host-native`: `5/8` sandboxed -> `8/8` unsandboxed; `13` nested subprocess summaries [`22` child passes; `2,485` raw `test result:` sum]). Full `132`-module Python test suite (`1,294` tests) passes **`57/57` active modules (`510/510`** after clean standalone retry of `test_pantry` `5/5`) + `73/75` historical (`775/784`), preserving exact `2` pre-existing Stage-65 stale-`oracle.lock.json` failures (`9` non-pass tests). Cross-milestone frozen gates: M1 (`target/m7/m7-final-m1-02`) **`6,078/6,078`**; Full M2 (`target/m7/m7-final-m2-full-01`, `--record-only` + single-process `--verify-development` Exit `0`; standalone `--verify-full` not re-run per `#182`, `full_gate_failures: 2`) **`4,574 pass / 1 oracle-error (error-cycle) / 0 mismatch`**; Protobuf strict (`target/m2/m7-final-protobuf-strict`) contracted Exit `101` (`240/8/18` in `9.14s`); M3 (`m7-final-m3-census.json` `1,781/1,781` + `m7-final-m3-diff` `118/118`); Full M4 (`m7-final-m4-all5-02`) **`429/429`**; Full M5 (`m7-final-m5-all5-02`) **`142/142`**; Full M6 (`m7-final-m6-all5-02`) **`80/80` executable PASS + `6/6` `capability-blocked`**.
- **Preserved limits**: `release_ready: false` (`2/34` Chromium browser-tree RSS rows `> 1.00x`, unverified `darwin-x64` / `linux-x64-gnu` / `linux-arm64-gnu` / `win32-x64-msvc` / Firefox / WebKit / hosted CI); unpinned optional `native-image-reference`; `2026-11-20` JVM/codegen exclusions; and retained M2/M4/M5/M6/Stage-65/M0-GraalVM exceptions recorded in [reports/m7/README.md](reports/m7/README.md).

The [M6 development-closure checkpoint](reports/m6/README.md) records the complete frozen-source verification of Milestone **M6** on local macOS `arm64` (`darwin-arm64`, `rustc 1.98.1`, `macOS-27.2-arm64`, NOT hosted CI) against the exact pinned **Eclipse Adoptium Temurin `21.0.13.1+2-LTS`** JDK (**`exact_jdk_lock_match: true`**, without `PKL_ALLOW_LOCAL_JDK=1`), `Node.js v26.13.1`, `Playwright 1.64.1`, `Chromium 156.0.8078.4`, and `wasm-bindgen 0.2.108` with zero modifications to `crates/{pkl-runtime,pkl-host-native,pkl,pkl-cli,pkl-doc,pkl-formatter,pkl-repl,pkl-server}` or `external/`:
- **Frozen-source outcomes (Wave 3A & Wave 3B)**: `cargo xtask verify`, `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo clippy -p java_regex --lib --locked -- -D warnings`, `cargo clippy -p pkl-wasm --target wasm32-unknown-unknown --locked -- -D warnings`, `cargo check -p {pkl-runtime,pkl-formatter,pkl-wasm} --target wasm32-unknown-unknown --locked`, `tools/oracle.py verify`, and all **11** Python `--check` scripts (`inventory`, `discover`, `smoke_manifest`, `regression_manifest`, `m2_manifest`, `m2_module_eval`, `m2_jvm_classpath`, `m3_manifest`, `m4_manifest`, `m5_manifest`, `m6_manifest`) exit `0`. Full Rust workspace tests pass **`2,472` outer passed across `378` targets/doctests** (**`2,485` raw `test result:` passes** including `13` nested child passes, `+9` vs M5 from new `pkl-wasm` unit tests; **`0` failed; exact `2` ignored** [`frontend::frontend_probe`, `m2_protobuf_java04_full::java04_strict_full_compatibility_gate`]), transparently recording (a) the macOS sandbox loopback split on `pkl-host-native` (`5/8` sandboxed -> **`8/8`** unsandboxed) and (b) one transient CPU-load `1000ms` init timeout on `m4_external_reader_process::real_subprocess_hang_on_read_unblocks_on_cancel_and_timeout` (`9/10` initial -> **`10/10`** in `1.27s` on clean retry alongside the full remaining tail). Sandboxed Python unit tests pass **`492 / 492`** across **53** modules (`test_m6_*.py` `2` mods `9/9`, `test_m5_*.py` `4` mods `16/16`, `test_m4_*.py` `3` mods `15/15`, `test_m3_*.py` `2` mods `12/12`, `test_m2_*.py` `22` mods `256/256`, `18` core shared & runtime-link mods `158/158`, `test_pantry` + `test_workspace_storage` `2` mods `26/26`; exact `75` untouched historical suites out of `128` total unexecuted). Standalone package tests pass (`packages/pkl-wasm/test/wasm.test.mjs` `9/9`, `packages/pkl-napi/test/run.mjs` `11/11`, `packages/pkl-napi/test/pack-install.mjs` PASS). M1 multi-target regression (`target/m6/m6-m1-check-02 --verify-contract`, unsandboxed retry preserving sandboxed `m6-m1-check-01`) matches **`6,078 / 6,078`** across `rust`, `node` (`v26.13.1`), and Chromium (`156.0.8078.4`) with `92/92` scenarios (`browserErrors: []`, `source_stability: true`, `473/473` `source_hashes`). Final frozen M6 5-lane verification (`target/m6/m6-full-strict-03`, `834` sealed files, all 4 candidates `recorded-build`, `exact_jdk_lock_match: true`, `source_stability: true` across `469` files, `server_self_test.passed: true`, `browserErrors: []`) records **`80 / 80` executable PASS + `6 / 6` `capability-blocked`** (`wasm-browser`: `29/29 + 4`, `wasm-node`: `8/8 + 2`, `napi-node`: `26/26`, `cross-target-parity`: `12/12` [`native: 12`, `napi_node: 12`, `wasm_node: 12`, `wasm_browser: 12`, `shared_engine_passed: 12`], `packaging-and-install`: `5/5`) with `--verify-report`, `--verify-strict`, and `tools/m6_manifest.py --verify-coverage` (`14/14` linked requirements, `83` links, `failure_count: 0`, `strict: true`) all Exit `0`. Conversion telemetry (`398 B` payload) records N-API median `35,583 ns` / mean `36,742 ns` (`25` iters) and browser Wasm median `100,000 ns` / mean `110,000 ns` (`20` iters, Chromium `100 us` timer granularity).
- **Preserved limits**: `capability-blocked` (`6/6` Wasm cases reported separately, never counted in `compatibility_pass_count`); unverified OS/architecture and browser targets (`darwin-x64`, `linux-x64-gnu`, `linux-arm64-gnu`, `win32-x64-msvc`, Firefox/WebKit, hosted CI); unchanged M3–M5/Protobuf crates retaining their frozen M5/M4 baselines below without redundant re-run; retained M2/M4/M5 exceptions; `75` unexecuted historical Python suites; M6 conversion telemetry strictly separated from M7's `<= 1.00` JVM throughput/RSS parity gate; and open Milestone **M7**.

The [M5 development-closure checkpoint](reports/m5/README.md) records the complete frozen-source verification of Milestone **M5** on local macOS arm64 (`rustc 1.98.1`, `macOS-27.2-arm64`, NOT hosted CI) against the exact pinned **Eclipse Adoptium Temurin `21.0.13.1+2-LTS`** JDK (**`exact_jdk_lock_match: true`**, without `PKL_ALLOW_LOCAL_JDK=1`):
- **Frozen-source outcomes (Wave 3A & Wave 3B)**: `cargo xtask verify`, `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo clippy -p java_regex --lib --locked -- -D warnings`, `cargo check -p pkl-runtime --target wasm32-unknown-unknown --locked`, `cargo check -p pkl-formatter --target wasm32-unknown-unknown --locked`, `tools/oracle.py verify`, and all **10** Python `--check` scripts (`inventory`, `discover`, `smoke_manifest`, `regression_manifest`, `m2_manifest`, `m2_module_eval`, `m2_jvm_classpath`, `m3_manifest`, `m4_manifest`, `m5_manifest`; `m2-mappings.json` updated for 1 renamed symbol `render` -> `render_ansi`, `m3-stdlib-output.json` and `m4-io-packages.json` resynced with `non_sha256_lines == 0`) exit `0`. Full Rust workspace tests pass **`2,463` outer passed across `377` outer targets/doctests** (**`2,476` raw `test result:` passes** including `13` nested child passes; **`0` failed; exact `2` ignored** [`frontend::frontend_probe`, `m2_protobuf_java04_full::java04_strict_full_compatibility_gate`]), executed via sandboxed `cargo test --workspace --exclude pkl-host-native --locked` (`rc=0`, `373` targets, `2,455` outer) + unsandboxed `cargo test -p pkl-host-native --locked` (`rc=0`, `4` targets, `8` passed) after initial sandboxed `cargo test --workspace --locked` exited `101` solely on 3 loopback tests in `pkl-host-native::m4_native_host_io`. Sandboxed Python unit tests pass **`483 / 483`** across **51** modules (`test_m5_*.py` `16`, `test_m4_*.py` `15`, `test_m3_*.py` `12`, `test_m2_*.py` `256`, `18` core shared tooling modules `174`, `2` runtime-link modules `10`; `75` untouched historical suites out of `126` total unexecuted). Multi-target M1 (`target/m5/m5-closeout-platforms-03 --verify-contract`, unsandboxed retry preserving sandboxed `-01`/`-02` Exit `2`) matches **`6,078 / 6,078`** across `rust`, `node` (`v26.13.1`), and Chromium (`156.0.8078.4`) with `92/92` scenarios (`browserErrors: []`). M3 regression gates pass cleanly (`target/m5/m5-closeout-m3-census.json`: `failure_count: 0`, `1,781/1,781`; `target/m5/m5-closeout-m3-diff`: `118/118 pass`, `--verify-report` and `--verify-strict` Exit `0`). M4 Lanes 1–3 regression gate (`target/m5/m5-closeout-m4-lanes123-02`, unsandboxed retry preserving sandboxed `m5-closeout-m4-lanes123` Exit `1`) records **`86 / 86 pass`** (`host-io-http-security`: `48/48`, `external-readers`: `11/11`, `projects-packages-cache`: `27/27`; `--verify-report` and `--verify-strict` Exit `0`). Protobuf strict capture (`target/m2/m5-closeout-protobuf-strict`) exits **`101`** as contracted (`240/240` regular, `8/8` pre-renderer, exact `18` `REFERENCE-NONPASS` lines with `0` byte-diff lines, `0` harness failures). Final frozen M5 5-lane verification (`target/m5/m5-final-strict-02`, `4,453` sealed files, all 3 candidates `recorded-build`, `exact_jdk_lock_match: true`, `source_stability: true` across `429` files, `server_self_test.passed: true`) records **`142 / 142 pass`** (`rust-api-and-cli`: `50/50`, `repl`: `12/12`, `server`: `22/22`, `formatter`: `43/43`, `pkldoc`: `15/15` with `1,767/1,767` regular files [`15,227,047 B`] and `13/13` symlinks at `0` diffs; `raw: 130`, `native-version: 3`, `pkldoc-progress: 9` with `raw_stdout_bytes_match` honestly recorded as `false` on all `9`) with `--verify-report`, `--verify-strict`, and `tools/m5_manifest.py --verify-coverage` (`16/16` requirements, `75` links, `17/17` exact vendored assets, `strict: true`) all Exit `0`.
- **Preserved limits**: `native-version` honest native-flavor contract (`3` cases); `pkldoc-progress` stdout progress nondeterminism contract (`9` cases, `raw_stdout_bytes_match: false` honestly recorded alongside `0` file/symlink/stderr/exit/milestone/URI-encoded-multiset diffs); 2026-11-20 scope exclusions (Java/Kotlin codegen, JVM public API adapters, JVM SPI descriptors); retained M2 exceptions (`error-cycle`, `18` Protobuf `REFERENCE-NONPASS`, `11` `required: false` `rev1` unmapped, Stage 161 4,575-case replay baseline); M4 Lanes 4–5 Pantry `343` cases unexecuted in this closeout slice (M4 `429/429` baseline preserved below); `75` unexecuted historical Python suites; Linux/Windows native execution & hosted CI; M7 performance/RSS tracking (`<= 1.00`); and open Milestones **M6–M7**.

The [M4 development-closure checkpoint](reports/m4/README.md) records the complete frozen-source verification of Milestone **M4** on local macOS arm64 (`rustc 1.98.1`, `macOS-27.2-arm64`, NOT hosted CI) against the exact pinned **Eclipse Adoptium Temurin `21.0.13.1+2-LTS`** JDK (**`exact_jdk_lock_match: true`**, without `PKL_ALLOW_LOCAL_JDK=1`):
- **Frozen-source outcomes (Wave 3A & Wave 3B, all 7 gates pass)**: `cargo xtask verify`, `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo clippy -p java_regex --lib --locked -- -D warnings`, `cargo check -p pkl-runtime --target wasm32-unknown-unknown --locked`, `tools/oracle.py verify`, and all **9** Python `--check` scripts (`inventory`, `discover`, `smoke_manifest`, `regression_manifest`, `m2_manifest`, `m2_module_eval`, `m2_jvm_classpath`, `m3_manifest`, `m4_manifest`; `m2-language.json` and `m3-stdlib-output.json` resynced with `non_sha256_lines == 0`) exit `0`. Full Rust workspace tests pass **`2,377` outer passed across `354` outer targets/doctests** (**`2,390` raw `test result:` passes** including `13` nested child passes; **`0` failed; exact `2` ignored** [`frontend::frontend_probe`, `m2_protobuf_java04_full::java04_strict_full_compatibility_gate`]), executed via sandboxed `cargo test --workspace --exclude pkl-host-native --locked` (`rc=0`, `350` targets, `2,369` outer) + unsandboxed `cargo test -p pkl-host-native --locked` (`rc=0`, `4` targets, `8` passed) after initial sandboxed `cargo test --workspace --locked` exited `101` solely on 3 loopback tests in `pkl-host-native::m4_native_host_io`. Sandboxed Python unit tests pass **`449 / 449`** across **45** modules (`test_m4_*.py` `15`, `test_m3_*.py` `12`, `test_m2_*.py` `256`, `18` core shared tooling modules `166`; `77` untouched historical suites out of `122` total unexecuted). Multi-target M1 (`target/m1/m4-closeout-platforms-02 --verify-contract`, unsandboxed retry preserving sandboxed `target/m1/m4-closeout-platforms` Exit `2`) matches **`6,078 / 6,078`** across `rust`, `node` (`v26.13.1`), and Chromium with `92/92` scenarios (`browserErrors: []`). M3 regression gates pass cleanly (`target/m3/m4-closeout-census.json`: `failure_count: 0`, `1,781/1,781`; `target/m3/m4-closeout-diff`: `118/118 pass`, `--verify-report` and `--verify-strict` Exit `0`). Protobuf strict capture (`target/m2/m4-closeout-protobuf-strict`) exits **`101`** as contracted (`240/240` regular, `8/8` pre-renderer, exact `18` `REFERENCE-NONPASS` lines with `0` byte-diff lines, `0` harness failures, `observations.tsv` untouched). Final frozen M4 5-lane verification (`target/m4/wave2-all5-strict-03`, `3,049` sealed files, `source_binding: "recorded-build"`, `exact_jdk_lock_match: true`, `source_stability: true`, `current_tree_matches_snapshot_after: true`) records **`429 / 429 pass`** (`host-io-http-security`: `48/48`, `external-readers`: `11/11`, `projects-packages-cache`: `27/27`, `pantry-dev`: `126/126`, `pantry-packaged`: `217/217`; `5/5` cold-mirror exact hits; `32/32` package-archive cases (`3/3` static `idempotency_ok: true`, `2/2` Java cross-interop, `1/32` empty-ZIP `raw_zip_bytes_match: true`)) with `--verify-report`, `--verify-strict`, and `tools/m4_manifest.py --verify-coverage` (`16/16` requirements, `strict: true`) all Exit `0`.
- **Preserved limits**: Pure-Rust Wasm DEFLATE vs OpenJDK C zlib on non-empty `package-archive` ZIPs (`raw_zip_bytes_match` honestly recorded alongside Java cross-interop); `CliProjectResolver.kt:32,48-55` CLI 0-byte stderr parity (library `warnings` preserved in `packages/mvs.rs`); explicit M5 deferrals (`stdlib/doc-package-info.pkl` [8], `pkl-doc` resources [17], `pkl-server` `0x20–0x25` [6 tags / 82 fields], full CLI & Rust evaluator API) and 2026-11-20 scope exclusions (Java/Kotlin codegen, JVM adapter); retained M2 exceptions (`error-cycle`, `18` Protobuf `REFERENCE-NONPASS`, `11` `required: false` `rev1` unmapped, Stage 161 4,575-case replay baseline); `77` unexecuted historical Python suites; M7 performance tracking (`import "pkl:reflect"` ~5.2s release, `pkl.tar` ~97s under `--timeout 180`); Linux/Windows native host & hosted CI; and open Milestones **M5–M7**.

The [M3 development-closure checkpoint](reports/m3/README.md) records the complete frozen-source verification of Milestone **M3** on local macOS arm64 (`rustc 1.98.1`, `macOS-27.2-arm64`, NOT hosted CI) against the exact pinned **Eclipse Adoptium Temurin `21.0.13.1+2-LTS`** JDK (**`exact_jdk_lock_match: true`**, without `PKL_ALLOW_LOCAL_JDK=1`):
- **Frozen-source outcomes**: `cargo xtask verify`, `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo clippy -p java_regex --lib --locked -- -D warnings`, `cargo check -p pkl-runtime --target wasm32-unknown-unknown --locked`, `tools/oracle.py verify`, and all 8 Python `--check` scripts (`inventory`, `discover`, `smoke_manifest`, `regression_manifest`, `m2_manifest`, `m2_module_eval`, `m2_jvm_classpath`, `m3_manifest`) exit `0`. Single-command `cargo test --workspace --locked` exits **`0`** (**`2,298` outer passed** across `345` outer targets/doctests [`13` nested child passes excluded / **`2,311` raw `test result:` passes** across `358` summaries]; **`0` failed; `2` ignored** [`frontend::frontend_probe`, `m2_protobuf_java04_full::java04_strict_full_compatibility_gate`]). Post-remediation Python tests pass `test_m3_*.py` (`12/12`) and `test_m2_mappings.py` (`8/8`) (with Wave 3B full 42-module sweep on unchanged Python tooling passing **`434 / 434`**: `test_m3_*.py` `12`, `test_m2_*.py` `256`, core shared tooling `166`; `77` untouched historical stage/fixture-authority suites not re-run). Multi-target M1 (`target/m1/m3-closeout-platforms-04 --verify-contract`, unsandboxed retry preserving sandboxed `target/m1/m3-closeout-platforms-03`) matches **`6,078 / 6,078`** across `rust`, `node` (`26.13.1`), and Chromium (`156.0.8078.4`, Playwright `1.64.1`) with `92/92` scenarios (`browserErrors: []`). M3 mechanical coverage (`target/m3/m3-closeout-census-02.json`) exits **`0`** with **`failure_count: 0`** (`1,781/1,781`: `vendored_stdlib_files: {deferred: 1, exact_match: 23}`, `stdlib: {deferred: 8, implemented: 1214}`, `builtin: {implemented: 542}`, `renderer: {implemented: 18}`). M3 3-lane differential (`target/m3/m3-closeout-diff-02`, `749` sealed files, `source_binding: "recorded-build"`, `exact_jdk_lock_match: true`) records **`118 / 118 pass`** (`formats-renderers: 77`, `output-files: 13`, `pklbinary-decode: 28`) with `--verify-report` and `--verify-strict` Exit `0`. Protobuf strict capture (`target/m2/m3-closeout-protobuf-strict-02`) exits **`101`** as contracted (`240/240` regular, `8/8` pre-renderer, exact `18` `REFERENCE-NONPASS` lines with `0` byte-diff lines on projected channels, `0` harness failures, `observations.tsv` untouched). Wave 3B pre-remediation focused 4-adapter M2 regressions (`target/m2/m3-closeout-selected-02`, unsandboxed retry preserving sandboxed `target/m2/m3-closeout-selected`) record **`100 / 100 pass`** (`--verify-report` and `--verify-subset` Exit `0`).
- **Preserved limits**: `external/pkl/stdlib/doc-package-info.pkl` (`8` items) explicitly deferred to M5 (`pkldoc`); `18` upstream Protobuf `REFERENCE-NONPASS` slots (`strict_compatible: false`, `experiment_contract_pass: true`); Wave 3B pre-remediation provenance for `target/m2/m3-closeout-selected-02` and the full 42-module Python sweep; Stage 161's full `4,575`-case M2 replay (`target/m2/stage161-full`), `error-cycle` strict-gate exception, and `11` `required: false` `rev1` unmapped entries; `77` unexecuted historical Python golden-fixture suites; Windows host symlink execution (verified via pure `check_path_spec(..., true)` unit tests only); Linux/Windows hosted CI; and open Milestones **M4–M7**.

The [Stage 161 M2 development-closure checkpoint](reports/m2/stage161-m2-closure/README.md) records the complete frozen-source verification of Milestone M2 on local macOS arm64 (`rustc 1.98.1`, `macOS-27.2-arm64`) against the exact pinned **Eclipse Adoptium Temurin `21.0.12.1+1-LTS`** JDK (**`exact_jdk_lock_match: true`**, without `PKL_ALLOW_LOCAL_JDK=1`):
- **Contract updates**: `crates/pkl-runtime/src/vm.rs` aligns lazy-property cycle diagnostics to `"A stack overflow occurred."` (preserving `ErrorKind::Cycle`, `LazyState::Unevaluated` rollback, and `tools/m2_language.py::status()` / `tools/m2_manifest.py::full_gate_failures()` strictly unchanged; `development_passes()` at `tools/m2_language.py:817` now expects `'–– Pkl Error ––\nA stack overflow occurred.\n'`), and `tools/m2_historical.py` bumps `CLI_PROFILE` to `pkl-0.32-historical-cli-no-cache-no-project-v2` to stage `t43_043_inherited_method_main.pkl` and `t43_064_imported_function_main.pkl` with isolated sibling `lib.pkl` copies (`staged/t43_*/`) while keeping `tests/fixtures/rev1-cli/` untouched and the other 19 CLI cases unchanged; `compatibility/m2-language.json` is synchronized (`4,586` cases: `4,575` ready, `11` `required: false` unmapped; `475` requirements).
- **Frozen-source outcomes**: `cargo xtask verify`, `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo check -p pkl-runtime --target wasm32-unknown-unknown --locked`, `tools/oracle.py verify`, and all seven Python `--check` scripts (`inventory`, `discover`, `smoke_manifest`, `regression_manifest`, `m2_manifest`, `m2_module_eval`, `m2_jvm_classpath`) exit `0`. Single-command `cargo test --workspace --locked` exits **`0`** (**`2,222 passed; 0 failed; 2 ignored`** [`frontend::frontend_probe`, `m2_protobuf_java04_full::java04_strict_full_compatibility_gate`] across `337` outer targets/doctests, `13` nested child passes excluded / `2,235` raw `test result:` passes; `m2_reflection_apis_lifecycle` `5/5` in `45.63s`, `m2_runtime` `43/43` in `1.50s`). Executed Python unit tests pass **`422 / 422`** (`test_m2_*.py` `256` in `367.384s` + core shared tooling `166` in `45.550s`; untouched historical golden-fixture Python suites were not run in this epoch). Multi-target M1 (`target/m1/stage161-platforms --verify-contract`, unsandboxed) matches **`6,078 / 6,078`** across `rust`, `node` (`v26.13.1`), and `browser` (Chromium `156.0.8078.4`, Playwright `1.64.1`) with `92` Node and `92` browser scenarios (`exact_jdk_lock_match: true`). Protobuf strict capture (`target/m2/stage161-protobuf-strict`) exits **`101`** as contracted (`240/240` regular, `8/8` pre-renderer, exact `18` `REFERENCE-NONPASS` entries, `0` harness failures). Fresh full `4,575`-case M2 replay (`target/m2/stage161-full`, `--workers 4 --record-only`, unsandboxed, all 3 candidates `recorded-build`, `exact_jdk_lock_match: true`, `report.json` SHA-256 `d68c8c8b...`) records **`4,574 pass / 1 oracle-error (error-cycle) / 0 mismatch`**, `88/88` metamorphic passes, `21/21` `historical-cli-module-eval` (including byte-exact `t43_043` `963/963 B` and `t43_064` `910/910 B`), `68/68` `package-project-eval` (`188` live HTTP requests), `source_stability: true`, and `artifact_stability: true`; sequential canonical verifiers confirm `--verify-report` Exit `0`, `--verify-development` Exit `0`, and `--verify-full` honest Exit `1` (`full_gate_failures: ["requirement execution incomplete: semantic:lazy-evaluation", "required case incomplete: error-cycle"]`).
- **Preserved limits**: `error-cycle` strict-gate exception (cold-JVM `NoClassDefFoundError` and nondeterministic preinitialized repetition counts in `reports/m2/cycle-complete.json`), `18` pinned Protobuf `REFERENCE-NONPASS` entries (`java04_strict_full_compatibility_gate` `#[ignore]`, carried to M3), `11` `required: false` `rev1` unmapped entries (`8` support libs + `3` deduplicated aliases), unexecuted historical golden-fixture Python suites, Linux/Windows hosted CI coverage, and open Milestones M3–M7 (M3: standard library and output; M4: modules, resources, projects, and packages; M5: product APIs and tools; M6: Wasm and N-API product completion; M7: full release verification and JVM throughput/memory parity) remain open.

The retained Stage 160 and earlier checkpoint sections below describe their original source epochs; they are historical observations, not current-source reruns.

## Stage 160 GC safepoint optimization and Protobuf boundary remediation

The [Stage 160 checkpoint](reports/m2/stage160-gc-protobuf-remediation/README.md) eliminates the per-step GC safepoint bottleneck (`Cargo.toml` dev profile `opt-level = 2`, per-slot edge caching and scratch buffer reuse in `Heap`, zero-allocation reflection root traversal, and dirty-guarded VM declaration-root caching) and fixes the two runtime bugs across the four `no-complete-reference-nonpass` Protobuf slots (`imported-alias-owner` and `constraint-read-true` under `on`/`off`, yielding `unsupported-or-api-error\n` rather than false success or null-root `oracle-error`) on local macOS arm64 (`rustc 1.98.1`, `PKL_ALLOW_LOCAL_JDK=1`, Temurin `21.0.13+9-LTS`). Formatting, `cargo xtask verify`, `cargo clippy --workspace --all-targets --locked -- -D warnings` (plus `--no-deps`), `cargo check -p pkl-runtime --target wasm32-unknown-unknown --locked`, six Python `--check` scripts, and `253` M2 Python unit tests (`215.895s`) all exit `0`. A single unsplit `cargo test --workspace --locked` exits **`0`** (`2,220 passed; 0 failed; 2 ignored` [`frontend_probe`, `java04_strict_full_compatibility_gate`] across `337` outer targets/doctests, `13` nested child passes excluded / `2,233` raw `test result:` passes), with `m2_reflection_apis_lifecycle` completing `5/5` in `83.35s` (`90.63s` solo) and `pkl-runtime --lib` completing `263/263` in `26.61s` (`20.71s` solo). Explicit Protobuf strict capture (`target/m2/stage160-protobuf-strict`) exits `101` (`240` regular, `8` pre-renderer, exact `18` `REFERENCE-NONPASS` entries, `0` harness failures); unsandboxed M1 (`target/m1/stage160-platforms-01 --verify-contract`, preserving sandboxed `target/m1/stage160-platforms`) matches `6,078 / 6,078` across `rust`, `node` (`v26.13.1`), and `browser` (Chromium `156.0.8078.4`, Playwright `1.64.1`); and focused M2 4-adapter capture (`target/m2/stage160-selected-01`, preserving sandboxed `target/m2/stage160-selected`) records `100 / 100 pass` with `--verify-report` and `--verify-subset` exiting `0`. Stage 160 does not re-run the full `4,575`-case replay: the latest full-ready acquisition remains Stage 159's [Stage 159 checkpoint](reports/m2/stage159-strict-gate-closure/README.md) (`target/m2/stage159-genuine`, `4,574 pass / 1 oracle-error (error-cycle) / 0 mismatch` across `4,575` ready cases; `--verify-report` and `--verify-development` exit `0`; `--verify-full` exits `1`). The [Stage156 canonical report](reports/m2/stage156-baseline/README.md) separates the earlier Rust and full-ready source epochs. Eighteen Protobuf reference nonpasses (`java04_strict_full_compatibility_gate` `#[ignore]`), `error-cycle` `oracle-error`, `11` unmapped candidates, local JDK provenance (`exact_jdk_lock_match: false`), Linux/Windows hosted CI coverage, and Milestones M3–M7 (M3: standard library and output; M4: modules, resources, projects, and packages; M5: product APIs and tools; M6: Wasm and N-API product completion; M7: full release verification and JVM throughput/memory parity) remain open.

## Stage 159 M2 integrity remediation and genuine live verification

The [Stage 159 checkpoint](reports/m2/stage159-strict-gate-closure/README.md)
records Iteration 4 integrity remediation and genuine live verification on the frozen Stage 159 worktree (local macOS arm64, `PKL_ALLOW_LOCAL_JDK=1`, Temurin `21.0.13+9-LTS`). Genuine full M2 acquisition (`target/m2/stage159-genuine`) records `4,574 pass / 1 oracle-error (error-cycle) / 0 mismatch` across `4,575` ready cases (`--verify-report` and `--verify-development` exit `0`; `--verify-full` exits `1`). Multi-target M1 (`target/m1/stage159-genuine`) matches `6,078 / 6,078` across `rust`, `node`, and `browser`, the 5 sentinel oracle suites pass `94 passed / 0 failed / 1 ignored`, and 2 explicit `#[ignore]` functions remain workspace-wide (`frontend_probe` and `java04_strict_full_compatibility_gate`). Split workspace Rust runs pass `2,226` tests with `0` failures and `2` ignored across `346` completed runs, while `m2_reflection_apis_lifecycle::cancellation_closes_evaluator_and_fresh_evaluator_rejects_old_transform_token` remained unresolved within the `60m` debug / `29m` `--release` timeout window prior to Stage 160.

A validation-contract revision must state what scope or observation changed,
why, and which current claims require new evidence. In review, distinguish the
historical contract and identities from applicability to the current contract.
The existing report verifier still binds the live current manifest and combines
these concerns; this review rule does not claim an independently implemented
historical-contract verifier. Never rewrite an old failure as a pass or reuse a
formerly sufficient observation for a stronger new claim without qualification.

## Stage 158 milestone remediation and serial verification

The remediated Stage158 source integrates:
1. **Packages and Projects (Track A)**: Pure package metadata acceptance, HTTP transport decoupled from host via `HttpHandler`, ZIP archive extraction, and project-based package import resolution (`package://...`, `projectpackage://...`, `--project-dir`) without global mutable statics or direct host I/O in `crates/pkl-runtime`. Pure `wasm32-unknown-unknown` compilation verified with 0 errors.
2. **Java Exception Projection & Runtime Modules (Track B)**: `PklBugException` 22-frame Truffle representation, foreign nominal type identity projection, standard runtime mirrors `pkl:platform`, `pkl:release`, and `pkl:benchmark`, genuine microbenchmark execution, algorithmic O(base_len) repeat evaluation in `api/list.pkl`, and restored honest `#[ignore]` annotations on 8 protocol-unresolved oracle tests.
3. **Requirement Mappings & Historical Cases (Track C)**: 475 requirements mapped across 9 M2 families, 4,575 ready cases synchronized, 21 `rev1-cli` historical fixtures admitted, and source hashes synchronized in `compatibility/m2-language.json`. Verified via 1,205 Python unit tests.

The serial acceptance suite qualifies the remediated source:
- `cargo xtask verify`: exit 0 (all reference pins match `references.lock`).
- `cargo fmt --all -- --check`: exit 0.
- `cargo clippy --workspace --all-targets --locked -- -D warnings`: exit 0 (0 warnings).
- `cargo check -p pkl-runtime --target wasm32-unknown-unknown --locked`: exit 0.
- `PYTHONPATH=tools /opt/homebrew/bin/python3.14 -m unittest discover -s tools/tests -v`: exit 0 (1,205/1,205 tests pass).
- `tools/m1_platforms.py`: passes contract across Native, Node, and Chromium targets (`experiment_contract_status: pass`).
- `tools/m2_language.py`: `--verify-report` exits 0; `--verify-full` exits 1 preserving strict gate integrity without false passes.
- Protobuf strict gate: exits 101, preserving 18 reference nonpasses while meeting the 240+8 renderer development contract.

Milestones M3–M7 (M3: standard library and output; M4: modules, resources, projects, and packages; M5: product APIs and tools; M6: Wasm and N-API product completion; M7: full release verification and JVM throughput/memory parity) remain open.

## Stage153–155 original contracts and URI boundaries

The two original T40 CLI contracts preserve original argv/URI/environment and complete stdout/stderr/exit/generated files. Both match; two Java-only cache-off controls are separately counted and do not add original coverage. Host-private files remain provenance outside the original public-file comparison. File URI 24 and package API 37 inputs retain 48/74 raw Java/native processes with normal Cargo source/artifact/storage and fresh Java helper/runtime joins. File 19 matches/5 nonpasses and package 6 value matches/31 nonpasses do not establish full error/Host/API compatibility; the 11 original package CLI/server/project cases remain unqualified by these controls.

The source06 full workspace records Cargo101 with 2,190 passed, 1 failed and 6 ignored; 13 nested passes are excluded. A source07 test/tooling correction is verified by a separate whole affected target (15 passed), 83 Python tests and five prechecks. Preserve the failed full-workspace receipt; the later target pass is not a whole-workspace rerun. Same-workspace raw266 retains 18 strict nonpasses. Fresh bounded M1’s strict and architecture predicates pass from one actual capture; no second strict CLI or new Protobuf-specific browser/Node matrix is implied. See the [checkpoint report](reports/m2/stage153-155-t40-uri/README.md) and accompanying publication metadata for source transitions, raw evidence and preserved setup failures.

## Protobuf development contract and strict completion gate

The ordinary workspace entry `java04_completed_reference_contract` attempts all
266 Java04 observations. Its development contract requires 266 native terminals,
no harness failures, exact result/callback projection membership, 240 complete
renderer matches and eight pre-renderer matches. Missing or extra projected
channels, invalid terminals and inconsistent event counts are harness failures
for every slot, including the 18 reference nonpasses; missing values are never
filled in. Java exception-class channels remain unprojected provenance.
All 18 internal/process reference nonpasses remain explicit and receive no
compatibility credit. A comparable raw mismatch still fails this entry. The
retained native02 result (228/240 renderer matches, eight pre-renderer matches,
12 ordinary mismatches and 18 reference nonpasses) remains historical evidence;
changing test entrypoints does not reclassify it.

`java04_strict_full_compatibility_gate` is an explicit ignored test in an ordinary
workspace run. Count it as ignored and unmet, never as a compatibility pass. This
temporary development exception does not waive the release completion gate.
The named strict invocation is required for full-completion assessment and must
use a fresh capture path that does not already exist:

```sh
mkdir -p target/m2
PKL_PROTOBUF_CAPTURE_DIR="$PWD/target/m2/stage145-protobuf-strict-01" \
cargo test -p pkl-runtime --test m2_protobuf_java04_full --locked \
  java04_strict_full_compatibility_gate -- \
  --ignored --exact --test-threads=1 --nocapture
```

Use a new path for each rerun and the applicable root resource admission. The
retained strict predicate is false because of the 18 reference nonpasses. The new
named strict invocation remains unexecuted until recorded separately. The broader
M2 language/API, platform and performance requirements remain separate and open.
A green ordinary workspace is a development checkpoint, not strict completion.

Each entry fixes its assertion mode in source. A present nonempty
`PKL_PROTOBUF_JAVA04_MODE` is rejected; it cannot switch the expectation of the
same test. `PKL_PROTOBUF_CAPTURE_DIR` controls only capture output. It is optional
for ordinary workspace execution and required for the explicit strict gate.
For evidence publication, run the named ordinary entry with a fresh capture too.
The same complete result/effect channels, all observations and strict-failure
ledger are preserved whenever capture is enabled. `contract.json` retains its
existing mode/count/strict/experiment fields.

## Execution environment

Development checkpoints may be verified locally. GitHub Actions automates the
checks and adds independent operating-system coverage; its availability is not a
prerequisite for continuing implementation or accepting a bounded checkpoint.
When local evidence is used, record the host, pinned toolchain and oracle, exact
implementation sources, build artifacts, commands, complete raw outcomes, and
unexecuted environments. Reuse unchanged verified evidence and run the checks
needed for changed behavior or unresolved concerns.

Local macOS execution does not establish Linux or Windows execution. Wasm
compilation remains separate from real browser execution. Hosted artifact-download
checks are applicable only when hosted artifacts exist; locally sealed evidence
must instead have its own integrity and source provenance checks. Preserve prior
CI failures as historical records. This execution choice changes neither raw byte
comparison requirements nor the full language, platform, or performance gates.

## Current Checks

### Stage146–151 Host and static analysis

All six prechecks passed: xtask, formatting, warnings-denied workspace all-target Clippy, Wasm, manifest freshness and inventory freshness. Workspace02 exited Cargo 0 under its recorded normal opt1/debug0, serial-test condition. The independent aggregate records 2,172 passed, zero failed and six ignored across 335 outer groups, excluding 13 nested child passes; it does not prove every individual test identity or absence of count-preserving interleaving. Same-workspace raw266 is complete: 240 renderer and eight pre-renderer matches against retained Java04 fixtures, with 18 strict reference nonpasses and zero harness/structural failures. No new 266-case Java acquisition or separate strict CLI run occurred. Selected original31 records 20 matches and 11 nonpasses; NFC records 13 UTF-8/UTF-16 path-value matches and five nonpasses. One fresh bounded M1 capture matched 6,078 results per Native/Node/Chromium target and 92 scenarios per Node/browser target; architecture and independent strict predicates pass on the same bytes, without a second strict CLI run. See the [Stage146–151 report](reports/m2/stage146-151-host-analyze/README.md) for the failed storage attempt, raw scopes and provenance limits. These checks leave the full M2/API, platform and performance gates open.

### Stage144 binary, YAML and shell

The pre-lint focused run passed 38 native tests with 320 retained Java observations; two additional YAML oracle-internal observations remain noncomparisons. Twenty-one original CLI contracts separately matched their complete public channels. After seven lint corrections, formatting, xtask, warnings-denied all-target Clippy, Wasm checking and manifest freshness passed. Workspace10 exited zero: the independent aggregate qualifies 1,947 passed, zero failed and five ignored across 323 outer groups, excluding 13 nested child passes. It does not independently prove individual test identities, absence of count-preserving interleaving or complete dependency closure. One fresh `--verify-contract` M1 capture matched 6,078 results per Native, Node and Chromium target and 92 scenarios each for Node/browser; the independent strict predicate passes on those same bytes, without a second strict CLI run. The full language inventory was not replayed and requirement/case states are unchanged. See the [Stage144 report](reports/m2/stage144-binary-yaml-shell/README.md) for retained failures and local provenance.

### Stage141 Jsonnet and XML renderers

The Stage141 Jsonnet and XML checkpoint adds shared renderer support and qualifies 21 focused native tests against 172 Java observations, including four retained setup-error controls. Twenty-five original CLI cases matched their complete public channels. The full workspace aggregate records 1,909 passed, zero failed and five ignored across 313 harnesses; 13 nested child passes are excluded. Bounded M1 matched 6,078 results per Native, Node and Chromium target, with 92 Node and 92 browser scenarios. Full M2/API, release-platform and performance requirements remain open. The aggregate does not establish individual test identities or exclude count-preserving interleaving. See the [Stage141 report](reports/m2/stage141-jsonnet-xml/README.md) for retained failures and provenance limits.

### Stage139 JSON Parser

The focused native run passed 47 tests and matched 108 Java observations in status and complete expression/stdout/stderr UTF-8 and UTF-16 fields; five registered original CLI cases matched their full public channels. Five fresh prechecks passed. The full workspace passed 1,886 tests with zero failed and five ignored across 305 harnesses; the aggregate excludes 13 nested child passes and does not prove individual test identities or absence of interleaving. The separate M1 acquisition matched 6,078 results per Native, Node and Chromium target, with 92 Node and 92 browser scenarios. See the [Stage139 report](reports/m2/stage139-json-parser/README.md) for retained failures and provenance limits.


### Stage137 module diagnostics and authored semver

The focused native run passed 69 tests and matched 48 Java observations in status and complete UTF-8/UTF-16 expression/stdout/stderr fields. Six original CLI cases matched every recorded public channel. The full workspace exited zero: the independent aggregate reader qualifies 1,872 passed, zero failed and five ignored across 302 outer harnesses, excluding 13 nested child passes. Formatting, xtask, Clippy and Wasm were four earlier successful runs; the refreshed manifest and workspace were two fresh runs. The bounded M1 acquisition matched 6,078 results per Native/Node/Chromium target and 92 scenarios per Node/browser target. See the [Stage137 report](reports/m2/stage137-module-diagnostics-semver/README.md) for retained failed attempts, raw evidence and provenance limits. This does not rerun the full language inventory or complete M2/API, release-platform or performance requirements.


### Stage136 lexical mappings and ready replay

The local full-ready acquisition records 4,483 cases (candidate-unsupported: 74, mismatch: 12, oracle-error: 2, pass: 4,395); integrity verification exited 0 and strict full verification exited 1. Twelve lexical mappings are supported by 49 scalar matches and a separate registered RECORD module match. The 62-test Python run includes 14 target-directory tests. See [the Stage136 report](reports/m2/stage136-language-replay/README.md) for recorded nonpasses and scope.


### Stage134 type sources and object entry diagnostics

The selected corpus contains 164 Java observations: 160 prior source and entry
controls plus four fresh direct/aliased-unknown property controls. The final
focused production acquisition runs 24 integration targets. Fresh production
Listing and Mapping CLI acquisitions match Java in their complete raw channels.

The final full workspace exits zero with **1,848 passed, zero failed and
5 ignored across 298 harnesses**; the aggregate reader excludes
13 nested child passes. Formatting, xtask, all-target Clippy, Wasm and
manifest freshness pass. The fresh bounded M1 acquisition matches 6,078 results
per Native, Node and Chromium target, with 92 Node and 92 browser scenarios and
both strict and architecture predicates passing. These are local, bounded
results; full M2/API, release-platform and performance requirements remain open.

See [the Stage134 report](reports/m2/stage134-type-source-scopes/README.md) for failed attempts and provenance limits.

### Stage131 power-assertion boundaries — local checkpoint

The shared power-assertion renderer follows the selected Java generator,
predicate, lambda and object-method boundaries. It matches all **34 Java
observations across 17 programs** with assertions on and off, and the unchanged
upstream `errors/power/typeConstraints11.pkl` now matches the complete Java CLI
result. A separately acquired Stage130 baseline retains the original mismatch.

The unchanged manifest distinguishes **71 unimplemented case execution contracts**
from **430 requirements marked unimplemented**, alongside 42 partial and three
implemented requirements. Its 4,442 ready cases are executable contracts, not
passes; 815 cases have no requirement links. These counts match the manifest
pinned by the successful freshness check, and do not recompute the historical
strict full-completion gate.

The full local workspace exits **0** with **1,790 passed, zero failed and
5 ignored across 288 harnesses**; 13 nested child passes are excluded.
Formatting, xtask, all-target Clippy, Wasm checking and manifest freshness pass.
Native, Node and Chromium each match 6,078 Java results; Node and Chromium each
pass 92 platform scenarios. These are bounded local results. Full language/API,
release-platform and performance requirements remain open, and the full
language inventory has not been replayed in this checkpoint.

See [the Stage131 report](reports/m2/stage131-power-boundaries/README.md) for retained failed attempts, raw evidence and provenance limits.

### Stage130 shared exception rendering — local checkpoint

Stage130 adds a shared throwable-graph renderer with full UTF-8/UTF-16 Java
comparisons for 54 graphs under LF and CRLF. The complete local workspace exits
zero with **1,789 passed, zero failed and five ignored across 287 harnesses**;
13 nested child passes are excluded. All 285 executable targets and two doctest
crates complete; 26 examples are compile-only. The focused Rust and Python
counts are not added to the workspace count.

Formatting, xtask, all-target Clippy, Wasm checking and manifest freshness pass.
The separate actual Native/Node/Chromium experiment matches **6,078 results per
target**, with **92 Node and 92 browser scenarios** and both strict and
architecture predicates passing. Preserved setup/linker failures, raw evidence
and provenance limits are detailed in [the Stage130 report](reports/m2/stage130-java-exception-renderer/README.md). Runtime stack capture and
internal-error catch integration remain open. This is not full M2/API,
release-platform or performance completion, and no full language replay is
claimed.

### Stage129 complete local workspace command

Stage129 completes the local `cargo test --workspace --locked` command with
actual exit **0**. Independent raw accounting records **286 complete harnesses:
1,786 passed, zero failed and five ignored**, excluding 13 nested child passes.
All 284 test executables and both doctest crates complete; 26 examples are
compile-only. The unchanged engine uses a separate cache clone and a 1 GiB
free-space floor. All 310 executable byte identities match prior evidence,
while 20 inherited compiler-receipt gaps and eight unavailable depfiles remain.

The new Java-only URI-catch acquisition records 16 sessions and 32 queries.
All eight between-round raw comparisons match; the uncaught URI control retains
different first/second error bytes in both sessions. No Rust compatibility pass
is inferred. Original readback failures and the corrected reader's provenance
are preserved. See the [Stage129 report](reports/m2/stage129-workspace-validation/README.md) for exact inputs, outcomes and
limitations. M2/API, release-platform and performance gates remain open.

### Stage128 workspace storage — bounded partial verification

Stage128 adds an opt-in macOS compiler wrapper that checks raw executable byte
identity and signatures before transparent compression. Eighteen focused Python
tests, actual compiler/self-spawn checks, fresh manifest checking and formatting
pass. The Pkl engine is unchanged from Stage127; its xtask, Clippy, Wasm and
bounded M1 results are reused as historical evidence, not new executions.

The full workspace retry ends with Cargo **-15** after the 768 MiB disk floor is
crossed. Independent raw readback qualifies **200 completed harnesses: 1,265
passed, zero failed and three ignored**, excluding seven nested child repeats.
`m2_reflection_apis_lifecycle` is interrupted; 83 compiled executable targets and
two doctest crates never start. All 284 test executables and 26 examples have
compiler records; examples are not claimed executed. The earlier exit-101 copy
headroom failure remains separate. See the [Stage128 report](reports/m2/stage128-workspace-storage/README.md) for storage
conditions, artifact provenance and retained failures. That acquisition did not
complete workspace validation. M2/API, release-platform and performance
requirements remain open.

### Stage127 opaque glob URI validation — bounded local checkpoint

The shared traversal now validates the raw opaque reader URI after glob regex
compilation and before listing. The selected 20 previously wrong-success sessions
now stop with `Unsupported`; their Java internal-error diagnostics remain
unmatched. Of 36 control sessions, 32 match the common raw channels and four
retain their prior relative-space mismatches. See the [Stage127 report](reports/m2/stage127-opaque-glob-uri/README.md)
for the 56-session/128-query evidence and unimplemented frame/catch behavior.

Eight Rust tests, formatting, xtask, all-target Clippy, the Wasm check, manifest
freshness and eight focused Python tests pass. A fresh M1 acquisition matches
6,078 rows per native/Node/Chromium target and passes 92 Node and 92 browser
scenarios. The actual full workspace command was stopped at its 1 GiB disk floor
during compilation (exit -15); that acquisition remains incomplete. The historical full
language/glob results below are not new Stage127 executions.

### Stage122 lossless source and runtime integration — verified local checkpoint

Independent readback records **1,779 passing tests, zero failures and five ignored across 285 harnesses**, composed from 198 checks04, 85 checks05 and two repaired checks06 harnesses. Nested child repeats are excluded and ignored tests remain gaps. This is functional harness coverage, not a successful fresh full-workspace Cargo command. Reflection's actual test process exited zero with five raw passes; its original Cargo exit was not captured and remains null/inconclusive. The original incomplete producer, failed observer and monitoring gap are preserved.

All seven checks06 commands have actual exit zero: five fresh non-workspace checks (xtask, formatting, all-target Clippy with warnings denied, Wasm runtime check and manifest freshness) plus both complete repaired test targets. Exact source deltas and inherited Cargo-cache provenance remain separate from fresh compilation claims.

The complete ordered 4,442-case replay records **4,351 matches, 15 mismatches, 74 unsupported and two Java oracle errors**, with no Stage121 previous-pass regressions and **88/88 metamorphic checks**. List's native 60-second timeout remains explicit; complete result coverage includes stopped evaluations. The exact current full-gate function retains **1,768 overlapping unmet conditions (472 implementation, 472 mapping, 824 required-case)**. All twelve newly matching originals still have empty requirement links, so their runtime improvement does not resolve linkage conditions. No full-gate CLI invocation or new Java execution is claimed for the readback.

M1 actual03 matches 6,078 comparisons on each target, with 92 Node and 92 browser scenarios and separate strict/architecture passes. Native transport qualifies 20 runtime controls plus four compiler counterparts. N-API's qualified full744/186 subset is reused; its fresh glob576 subset matches 524 and preserves 52 nonmatches, repairing four previous adapter regressions. Chromium reuses 701 independently qualified processes and completes 63 new processes: its combined results match 740 ordinary outcomes, 744 callbacks and 524/576 glob sessions, preserving 52 known nonmatches. The full Native/N-API/Chromium transport-19 raw join passes. Ten large Chromium cases use a separately recorded 3 GiB sampled owned-process RSS cap; the other processes retain 2 GiB, and the original cap failure remains recorded. All 18 evidence archive parts and their published copies are independently checked. Full typed binding/API, M2, release OS and performance requirements remain open. See the [Stage122 report](reports/m2/stage122-lossless-source-runtime/README.md) and its source-aware descriptor for exact local receipts and remaining requirements.

### Stage121 Pair and superclass diagnostics — previous local checks

The fresh complete workspace records **1,717 passed, zero failed and five
ignored**, excluding 13 filtered child repeats. All six recorded commands pass:
xtask, formatting, all-target Clippy with warnings denied, workspace tests, the
Wasm runtime check and manifest freshness. The workspace stderr retains 299
`rust-objcopy` stripping failures; Cargo exited zero, but successful stripping
and a complete compiler/example inventory are not claimed.

The complete ready replay records **4,339 matches, 19 mismatches, 82 unsupported
cases and two Java oracle errors across 4,442 cases**, with no previous-pass
regressions and **88/88 metamorphic checks**. `extendTypeAlias.pkl` and
`propertyNotFound2.pkl` now match their complete original Java contracts.

Each selected Native, N-API and Chromium target matches **624 new observations**
and **902 retained ordinary observations**. Two retained internal observations
and 2,020 new JVM-only fields remain unresolved. One separate actual M1
acquisition matches **6,078 checks per target**, passing both its strict and
architecture predicates. Original wrapper, qualifier and browser-monitor
failures remain preserved; successful reused processes are identified explicitly.

See the [Stage121 report](reports/m2/stage121-pair-supertype/README.md) and its
checkpoint descriptor for exact source, artifacts, raw outcomes and evidence
publication status. These local checks do not close M2, the full API, release
platform or performance requirements.

### Stage120 shebang, quoted identifiers and diagnostic ownership — bounded local checkpoint verified

The original complete workspace acquisition records **1,711 passed, zero failed
and five ignored**, including all 11 new test functions. Terminal readback verifies
270 executable test targets, 25 compile-only examples and two doctest libraries;
13 filtered child summaries remain separate. These are actual workspace results,
not the earlier isolated 27-test candidate run. This acquisition predates the two
test-only Clippy repairs. The two affected tests pass on the repaired source; the
full workspace was not rerun and those two results are not added to 1,711. Exact
old test bodies remain available in acquisition-specific isolation overlays.
Ignored tests remain required gaps.

The full ordered ready replay records **4,337 matches, 21 mismatches, 82 unsupported
cases and two Java oracle errors across 4,442 cases**, with no previous-pass
regressions and **88/88 metamorphic checks** passing. The original
`errors/singleBacktick.pkl` now matches; both selected original shebang cases remain
matches. The exact current full-gate function retains **1,780 overlapping unmet
conditions**. Inventory freshness, complete acquisition and whole-language
compatibility are separate verdicts.

The recorded new Native capture compares **234 attempted observations from 117
Java authorities** and has `api_full_match=true` and
`frontend_comparable_match=true`; `all_required_pass=false` remains explicit.
The composed capture02 records six selected groups: each new Native, N-API and Chromium target matches 234 attempted observations from 117 Java authorities; each retained target matches 902 ordinary observations and keeps two required internal observations unresolved. Comparable predicates pass; `all_required_pass=false` remains explicit.
The original capture01 remains failed: N-API's supplied-text Host adapter failed,
and the controller did not execute Chromium or the three retained groups.
The exact receipt/source/product join imports the complete Native capture and comparison from capture01 (two prior process records), then records five fresh groups and ten new process records on the same five build02 products. No Rust rebuild or Native rerun is claimed.
New adapter source identities are separate from the unchanged compiled product
identities. Saved comparison or restoration checks do not count as new execution.

One actual M1 acquisition matches **6,078 checks per native, Node and Chromium
target** and passes the strict comparison and separate architecture contract.
Formatting, three affected Python suites and eight freshness commands pass in
prepare02. Inventory/manifest regeneration refreshes two metadata files and 42
SHA-only implementation links without changing the inventory or selected contract.
Defaults02 preserves successful xtask and failed Clippy child101; its Wasm check was not run. After two cfg(test)-only decoder repairs, defaults03 freshly passes inventory and manifest freshness, the two exact affected tests, formatting, all-target Clippy with warnings denied and the Wasm runtime check: seven commands, stable source bookends. The prior successful xtask result is reused. The two fresh tests are separate from the original 1,711-pass workspace acquisition; the full workspace was not rerun.
The original workspace acquisition is reused with its original source identity.
Defaults03 freshly runs formatting and the two affected tests on the repaired
source; no fresh complete workspace acquisition is claimed.

The [checkpoint](reports/m2/stage120-shebang-quoted-identifier-checkpoint.json)
binds exact sources, artifacts, raw channels, original failures and restoration
prerequisites. Two published evidence parts total **77,196,066 compressed bytes** (62,914,759 and 14,281,307 bytes), each below 85 MiB. Independent manual tar/PAX and zlib readback verifies 21,670 selected payloads, one auxiliary payload and two manifests (21,673 logical regular members), exact raw bytes/size/mode/order/SHA256, gzip CRC/termination and tar EOF, plus 157,942 direct restoration aliases with no mode transformations. Exact public copy readback covers both parts and five verification support files.
Local macOS evidence establishes no hosted CI, Linux/Windows, complete product API
or performance result. Required ignored tests, raw nonpasses, Java internal
observations and the whole-List 512 MiB Java heap failure remain open.

### Stage114 recorded local verification

The integrated source passes one complete workspace acquisition: **1,700 tests
passed, zero failed and five ignored**, including all 42 new tests across five
targets. Formatting, xtask verification, all-target Clippy with warnings denied,
the Wasm runtime check, 43 affected Python tests and eight freshness commands pass.
Ignored tests remain required gaps.

The full ordered ready replay records **4,336 matches, 22 mismatches, 82 unsupported
cases and two Java oracle errors across 4,442 cases**, with no previous-pass
regressions and 88/88 metamorphic checks passing. Each selected Native, N-API and
Chromium acquisition matches 902 ordinary observations from 95 programs and 190
sessions; two required internal observations remain unresolved. Matched errors
that never reach the intended operation are not semantic coverage. The separate
M1 matrix matches 6,078 checks per target.

The [checkpoint](reports/m2/stage114-diagnostics-regex-checkpoint.json) and [evidence report](reports/m2/README.md)
record the exact source, artifacts, raw channels, restoration dependencies and
preserved failed acquisitions. Local macOS validation establishes no hosted CI,
Linux/Windows, whole-language or performance result. The strict M2 gate still
reports 1,780 overlapping unmet conditions; this is not a completion percentage.

### Stage109 Typed lookup, module paths and annotation diagnostics — local validation

Stage109 integrates 18 source/tool paths and 6,996 pinned Java fixture paths, with two generated metadata paths recorded separately. The combined targeted run records **152 passes, zero failures and one ignored required placeholder**. Its three new-family independent native readback matches **276 ordinary observations: 102 successful values and 174 ordinary errors**, including **1,736 harness fields and eight ordered Logger events**. One required Java internal contract retains two unresolved observations. The ignored test and unresolved Java results are not compatibility passes. The same current normal runtime separately matches **1,138 existing Typed/reflection observations**, **6,988 harness fields and 16 Logger events**.

The selected **Native02, NAPI03 and Chromium03** captures each cover **61 programs, 122 repeated sessions and 278 attempted observations**. Each matches all 276 ordinary observations, **2,288 portable fields and eight ordered Logger events**; two observations remain unresolved and `all_required_pass=false`. The portable count includes expression/status/output/error/logger-count channels and differs from the native harness's 1,736-field boundary. Java exception classes, causes and typed-export metadata remain provenance outside the shared string/error/logger comparison boundary.

The selected Host uses exact Java-backed synthetic keys, selected original/resolved/source identities, file/http/no-loader primary policies and genuine configured missing-resource completions. The original Native01 acquisition failed because its finite Host omitted normalized synthetic factory keys. NAPI02 then rejected six HTTP/`repl:` sessions at an obsolete file-only entry guard before evaluator construction. Both failures remain unchanged. Native10 and focused Node2 validate the normalized-key repair; focused Node4 validates the entry-guard repair. NAPI03/Chromium03 reuse admitted producer02/binding02 artifacts with independent before/after pins for the new JavaScript harness. New JavaScript inputs are not retroactively claimed as compiled producer inputs. Native02 was not rerun for the JavaScript-only repair.

All selected captures retain poll64, a 2,000,000 requested-instruction session fuel ceiling, collection after each batch, unchanged evaluator/heap conditions and original per-program deadlines. Their concurrent correctness results establish no execution-time or memory comparison. The admitted normal runtime `.rlib`, `.rmeta` and dep-info were retained before later Clippy/workspace could replace canonical artifact paths; the retention does not alter their recorded acquisition or claim standalone toolchain recovery.

The full ready replay is complete and integrity-valid: **4,332 passes, 26 mismatches, 82 unsupported cases and two oracle errors across 4,442 cases**, with zero candidate internal errors, zero historical-pass regressions and **88/88 metamorphic checks** passing. The separate original subset remains **eight matches and three diagnostic mismatches across 11 cases**. These are complete acquisitions with retained failures, not full compatibility passes.

One actual M1 acquisition matches **6,078 checks per native, Node and Chromium target**; strict and architecture predicates pass on that same acquisition. Eight actual freshness commands pass. Applying the exact current `full_gate_failures` function to the freshly checked manifest and complete replay reports **1,780 overlapping unmet conditions: 472 implementation, 472 mapping and 836 required-case conditions**. This is function-based accounting, not a new `--verify-full` CLI invocation or a completion percentage. The inventory remains 4,545 cases, 4,442 ready, 71 unimplemented, 32 unmapped and 475 requirements.

Formatting from prepare02 and the later actual xtask, all-target Clippy with warnings denied, and Wasm checks pass with stable explicit inputs. All proof-dependent captures completed before Clippy; the original normal artifacts were retained first. One actual complete workspace run passes **1,658 tests, zero failed and five ignored** across 262 executable test targets, 25 compile-only examples and two doctest libraries. The terminal readback verifies expected/invoked targets, examples, doctests and outer-summary coverage; 13 filtered child summaries remain separate. Ignored tests are not passes. Cargo reuses the normal runtime from targeted-compile02 as Fresh. Recorder Rust/build bookends and post-terminal test-artifact hashes do not assert a full fixture or transitive Python freeze. The [Stage109 checkpoint](reports/m2/stage109-typed-module-annotation-checkpoint.json) records exact evidence and restoration dependencies. Stage110, Stage111 and Stage113 isolated candidates are excluded. Existing List internal obligations and the whole-List 512 MiB Java heap failure remain required. Full M2/API, release-platform, performance and hosted-CI completion are not claimed.


### Stage103/104 reflection and Typed.hasProperty — recorded local validation

The shared engine implements source-backed reflection, lazy Reference access
metadata and non-forcing `Typed.hasProperty`. The selected Java authority contains
419 reflection and 150 Typed query/profile contracts. Under the explicit poll64
profile, each native, N-API and Chromium target now matches **1,138 observations**
from **67 program/profile groups and 134 repeated sessions**: **606 successful
values and 532 ordinary errors**, with **9,264 shared raw fields and 16 ordered
Logger events**. No contract in this selected matrix remains unresolved. Java
exception-class, cause and typed-export provenance stays separate from the shared
string/error/logger equality boundary.

These results use fresh byte-preserving readbacks of the existing selected02
captures. Native02's original report remains successful. The original N-API02
and Chromium02 reports remain `complete=false` and `success=false`: their first
comparator used universal newline decoding on source text. The corrected reader
preserves CRLF and CR bytes; six transport controls pass. No source payload,
expected output or old report was rewritten, and no compiler, Java or guest was
rerun for the corrected comparison. The direct second row/hash read shares the
comparison primitive; it is not an independently implemented comparator.

The selected profile requests at most 64 steps per poll, charges requested fuel
against the existing 2,000,000-step ceiling per session, and collects after each
batch. Evaluator constructors, heap settings and original per-program deadlines
are unchanged. The earlier poll1 timeout and admission failures remain separate
failed acquisitions. These profiles and concurrent correctness runs establish no
performance or memory comparison.

Final-source compile05, formatting03, xtask03, Clippy03 and Wasm01 pass with stable
source records. The Reference diagnostic unit test passes once, Mapping lifecycle
passes five tests, two unchanged Reference originals match Java, and the nine
focused scalar regressions match complete Java exit/stdout/stderr channels. The
nine previous stale-manifest Python errors pass on rerun, and eight read-only
freshness checks pass. Earlier Python acquisitions keep their own provenance.
Workspace validation passes **1,596 tests, zero failed and four ignored** across
257 executable test targets, 25 compile-only examples and two doctest libraries.
The terminal readback verifies expected/invoked targets, examples, doctests and
outer-summary coverage; 13 filtered child summaries remain separate. These counts
come from one actual `cargo test --workspace` acquisition. Metadata inventory
cohorts are not additional command invocations. Recorder Rust/build bookends and
post-terminal test-artifact hashes do not assert a full fixture or transitive
Python freeze.

The final-source original-module replay02 is terminal: **779 passes, 26
mismatches, 82 unsupported cases and one oracle error across 888 cases**, with no
regression from its historical passing baseline. The eight historical cases and
the JVM-profile boundary also pass; the JVM case retains two observations. The full ordered replay02 is complete and integrity-valid: **4,325 passes, 33
mismatches, 82 unsupported cases and two oracle errors across 4,442 cases**, with
zero candidate internal errors, no unexecuted cases and no regressions from the
historical passing baseline. All **88 metamorphic checks** pass. Complete
acquisition preserves these incompatibilities; it is not a compatibility pass.

The earlier compile04 command still records four failed library tests despite
matching 838 reflection and 300 Typed observations, 6,988 harness fields and 16
Logger events. Its first full replay remains **4,314 passes, 42 mismatches, 82
unsupported cases, two candidate internal errors and two oracle errors**, including
11 regressions. Final repairs and focused matches do not relabel those failures.

One actual M1 acquisition separately matches **6,078 checks per native, Node and
Chromium target**; strict and architecture predicates pass on that same capture.
M1 is a bounded primitive/scenario matrix, distinct from the selected feature
matrix and the full corpus. The inventory remains **4,545 cases: 4,442 ready, 71
unimplemented and 32 unmapped**, with **475 requirements**. The exact current full-gate function reports **1,780 overlapping unmet
conditions**: 472 implementation rows, 472 test-mapping rows and 836 required-case
conditions. This is function-based strict accounting after manifest freshness,
not a new `tools/m2_language.py --verify-full` CLI invocation or a completion
percentage. The strict full M2 gate remains failed.
Legacy List internal obligations and the whole-List 512 MiB Java heap failure
remain required and unresolved. Full M2/API, release-platform, performance and
hosted CI completion are not claimed.

The [Stage104 checkpoint](reports/m2/stage104-reflection-typed-checkpoint.json)
records these outcomes, source identities and restoration inputs. Later isolated
candidates are separate from this canonical implementation and validation scope.


### Stage102 collection APIs — actual local and selected target checks

List 995, Map 377 and Mapping 488 form **1,860 query/profile contracts**, each
observed twice. On freeze04, targeted05 records **83 passes, zero failures and one
ignored test**. Its **1,859 ordinary contracts / 3,718 observations** match pinned
Java: **2,338 successes, 1,380 ordinary errors, 37,368 shared raw fields and 1,506
ordered Logger events**. One List internal contract retains two required
unresolved observations; its ignored placeholder is not a pass. The original
whole-List 512 MiB Java heap failure and earlier Set/glob protocol obligations
remain open.

The corrected selected **native, N-API and Chromium acquisitions each match all 3,718
ordinary observations, 44,804 shared raw fields and 1,506 ordered Logger events**
across 74 programs and 148 repeat sessions. Their two required internal
observations remain unresolved, so `all_required_pass` is false. This portable
field count includes eight expression/status/stdout/stderr/Logger-count fields
per ordinary observation and ten fields per Logger event; the Rust harness uses
a distinct six-field observation scope. Output/error UTF-8 and UTF-16 bytes and
Logger order are compared directly. Java exception class/cause metadata remains
provenance. Each acquisition is complete, source-stable and tracked-input-stable.

The first native acquisition was incomplete because its Host omitted a required
auxiliary module; it supplies no engine-comparison verdict. The corrected Host
uses the exact Java-pinned auxiliary source for only the two importing programs,
with empty registries for the other 72. Actual checks cover nine mocked Node
Host/transport tests and five native adapter tests. Failed captures and approved
raw removals retain their original status and exact archive/restoration maps.

All eight default roles have actual local passing evidence. The complete
`stage102-workspace-deferred-01` run records **1,502 passes, zero failures and four
ignored tests**, covering **252 test targets, 25 compile-only examples and two
doctest libraries**. Python runs **180 tests**. Actual formatting and Clippy
command records have **414 and 712 recorded input identities**, respectively,
matched to freeze04 bytes/SHA; manifest04 binds the same catalog directly.
Current xtask, Wasm and inventory checks complete the default roles. These joins
preserve the recorded scope and do not invent historical modes or broad fixture
bookends.

The workspace used a separately recorded **1.5 GiB start admission / sticky
1 GiB floor** alternative to the retained 2 GiB baseline, with unchanged heap,
profile, workload, timeout and raw cap. Its deferred-evidence controller and
observer remain separate records. Earlier disk-floor, stale-Fresh, compilation,
development and lint failures keep their original outcomes. Observed disk changes
do not establish a cause, peak memory or performance.

One current M1 acquisition matches Java on **6,078 checks per native, Node and
browser target**. Strict comparison and the architecture contract pass through
separate readback of that same acquisition, with 92 Node and 92 browser checks.
This bounded experiment is separate from the selected collection matrix.
Unchanged original Set and Mapping match; original Map fails at `map.pkl:134`:
`toTyped(Person)` succeeds, then the existing `Typed.hasProperty` gap prevents a
match. The replay is integrity-valid and source-stable, with two of three raw
matches; the remaining original is a required failure.

The [Stage102 checkpoint](reports/m2/stage102-collection-apis-checkpoint.json)
provides the evidence scope, input/artifact identities and recovery references.
These local development results do not complete the full language/API,
release-platform or performance gates.


### Stage99 numeric/Set validation — actual default and selected results

Recorded local evidence: [Stage99 checkpoint](reports/m2/stage99-numeric-set-apis-checkpoint.json).

Targeted04 on freeze04 records **25 passes, zero failures and one ignored test**.
The Java-derived fixtures compare **1,215 ordinary contracts repeated twice**:
**2,430 observations, 26,960 shared raw fields and 1,238 ordered Logger events**.
Status, UTF-8/UTF-16 result/error bytes and ordered Logger fields are exact;
Java class/cause metadata remains preserved provenance outside this comparison.
The full matrix has 1,218 contracts / 2,436 observations, including three required
foreign-entry internal contracts / six observations that are not ordinary passes.
The 39 numeric power=true contracts and separate Set followups remain distinct.

One actual `cargo test --workspace --locked --no-fail-fast` invocation passes
**1,418 tests, zero failures and three ignored tests**. All 245 test executables
are invoked; 25 examples are compile-only and two libraries run doctests.
Thirteen nested child summaries are separate from the outer total. Ignored
frontend-probe, Set foreign-entry and String glob tests remain required in their
respective execution/protocol scopes. Rust 1.98.1 uses the recorded workspace
opt1/debug0/strip-symbols/checks-on/jobs1/incremental0 profile.

Formatting02, xtask02, Clippy02, Wasm01 and Python01 pass on freeze04. Python01
runs **180 tests**; Clippy and Wasm retain their recorded opt0/debug0 profiles.
The current default driver admits terminal same-source Clippy02 as prevalidated
and executes seven other roles. **The terminal driver passes all eight roles**,
including manifest-final and inventory, with no missing or unexecuted roles.
Its prevalidated Clippy result remains separate from the seven new commands.

Actual workspace normal-runtime evidence authenticates Fresh reuse of targeted04,
including original rustc/event/.d/input/artifact records and source bookends.
The workspace newly builds `module_eval_probe` and `semantics_probe`. Platform
schema admission is successful read-only validation of those actual records;
it neither rebuilds the core nor establishes guest execution. The actual M1
acquisition now matches **6,078 cases per Rust/Node/Chromium target** and across
the shared engine; strict and architecture gates both pass. Selected native,
N-API and Chromium each complete **23 programs / 46 sessions / 2,436 observations**.
All **2,430 ordinary observations** match: 1,666 successes, 764 ordinary errors
and 1,238 ordered Logger events. The platform comparator checks 31,820 shared
raw fields including expressions and Logger counts, a different count scope
from the Rust harness. Six required internal observations remain unpassed.

Each target retains 41,612 row files / 1,275,234 bytes, with 13,498 UTF-16/UTF-8
pairs checked. Forty complete session protocols match; six sessions containing
required Java internal statuses retain their actual differing protocols.
Native/Node process streams and Chromium page-returned protocol bytes retain
distinct origins. All five window commands and 50 selected commands exit 0
with no timeout, forced teardown, capacity stop or terminal owned processes.
Sources remain stable; acquisition/comparable success does not change
`all_required_pass=false`.

Current native-adapter rustc exits 0 with a 686-byte stderr warning that
`rust-objcopy` stripping aborted on unavailable `@rpath/libLLVM.dylib`. Preserve
that warning and actual artifact identity; the strip flag does not prove
stripping completed. The scoped LLVM loader environment belongs to the M1
process tree and was not applied retroactively to the native adapter.

Original `api/float.pkl`, `api/int.pkl` and `api/set.pkl` retain exact Java
argv/cwd/physical URI/environment/raw outputs and public file/write maps.
**Their actual Stage99 native replay matches two of three originals.** Float
and Int match stdout/stderr/exit exactly. Set fails at original line 249,
`List(1).minWith(comparator)`: native stdout/stderr/exit are 0 bytes/931 bytes/1,
versus Java 6,984 bytes/0 bytes/0. Preserve that required mismatch and replay
the unchanged original after full List integration. The original 60-second
deadline, source, argv and environment are retained; no timeout, resource stop
or forced teardown occurred. Native-private before/after invariance remains
separate from Java/native private-state comparison.

Retain the failed initial compile, targeted02 **22/3/1**, exact diagnostic/load
corrections, prelaunch FileNotFoundError with no Cargo invocation, and default01 /
Clippy01 exit101. The parity/local lint correction and later passing Clippy02 do
not rewrite those outcomes. No expected bytes come from Rust; full M2, current
whole-corpus, release-platform and performance gates remain open.

### Stage98 shared String API validation

The final source-bound workspace passes **1,393 tests, zero failures and two
ignored tests**, covering all 242 test executables, 25 compile-only examples and
two doctest libraries. Thirteen nested child summaries are separate. The 19 String
oracle groups and three lifecycle tests are included; the unresolved-glob ignored
test is not a pass. The full Python suite passes **180 tests** after test-only
accounting/source-catalog corrections. Its earlier 168-test attempt retains three
failures and two errors; the earlier narrow 34-test result does not replace it.

The eight local roles are workspace, xtask, formatting, all-target Clippy, Wasm,
Python, manifest freshness and inventory freshness. Driver05 reuses terminal
Python02/Clippy03, then executes workspace02/Wasm01/final freshness checks.
Formatting04 remains earlier-source evidence: only three Python tests differ,
so its Rust/build/formatter inputs are unchanged. Xtask01 is also earlier-source
evidence, admitted by its unchanged relevant inputs; the exact two reference
metadata files outside the selected dictionary are checked against both original
broad bookends and current bytes. Clippy02's Cargo exit zero with a collector
failure and absent after-bookend remains failed evidence, separate from Clippy03.
Workspace uses opt1/debug0/strip-symbols/checks-on/jobs1/incremental0; Clippy and
Wasm use their recorded opt0/debug0 profiles with Rust 1.98.1.

String authority covers 1,209 contracts twice. Native, N-API and actual Chromium
each acquire **2,418 rows** and match **2,412 comparable rows**, with 160 ordered
Logger events. Full result status, UTF-8/UTF-16 output/error bytes and Logger
order/fields are checked. The three upstream glob internal protocols remain
required unresolved (**six observations per target**), so comparable success does
not imply `all_required_pass`. Default/Turkish/power profiles and all 38 program
sessions per target are retained. Canonical String fixtures use Java repeat0;
Java repeat1 remains in the separately checked original authority. Java
class/cause records are preserved but are not fabricated as Rust outputs.

The original native module-output adapter matches all six selected originals:
`api/string.pkl`, `api/stringUnicode.pkl`, `listings/listing5.pkl` and
`mappings/typeCheck.pkl` succeed; `errors/power/typeConstraints5.pkl` and
`typeConstraints6.pkl` reproduce the expected Pkl errors with exit 1. Original
argv, physical input URI, environment and public file/write maps are unchanged.
Native private before/after equality is a lifecycle check; Java Host-private
cache contents are excluded from cross-runtime comparison.

A fresh M1 acquisition passes strict **6,078/6,078 per target** and the separate
`--verify-contract` architecture gate, including 92 checks each in Node and
Chromium. Actual normal runtime, native helper, N-API, raw Rust Wasm and generated
bindgen Wasm/JS retain distinct compiler/source/artifact records. The initial
portable attempt failed to load the pinned LLVM library during Wasm linking;
the successful retry adds only a process-local `DYLD_FALLBACK_LIBRARY_PATH`.
The earlier successful N-API build retains the `rust-objcopy` debug-strip warning.
For reused units, current hashes are admission-time identities; the failed
acquisition did not originally record artifact hash bookends.

Earlier failures and all six unresolved glob observations are retained. The
[checkpoint](reports/m2/stage98-string-apis-checkpoint.json) identifies local raw
evidence and the `stage98-string-apis-evidence` archive parts. Stage94's whole
aggregate is unchanged; full ready-corpus, language/API, release-platform and
performance gates remain incomplete.

### Stage97 expression adapter and token requirements

The native filesystem probes now evaluate the original complete expression through
shared `SourceProbe` module evaluation and the existing native Host helpers. The
legacy scalar facade and portable `evaluateLanguage` bindings retain their bounded
property-path contract. No shared engine or binding implementation changed.

The final workspace passes **1,371 tests, zero failures and one existing ignored
probe** across 240 test executables, 25 compile-only examples and two doctest
libraries. Thirteen nested reruns are separate. Seven adapter/Host tests are now
included in the default suite; their targeted run is not added to the total.
Formatting, xtask, all-target Clippy, Wasm compilation and manifest freshness pass.
The separate unchanged Python tooling suite passed 34 tests before adapter integration.

The shared token suite matches 54 observations and 324 raw fields. Actual native
process comparisons improve from 24/36 to **36/36**, preserving the failed run.
The complete affected scalar set matches **3,537/3,545** with the original seven
mismatches and one Java oracle error; no previously passing case regresses.
All eight old failing stdout/stderr channels remain identical, while six native
exits change from 1 to the explicit unsupported code 2; their full triples are not
claimed unchanged. All 88 metamorphic groups pass. The acquisition command's exit zero means evidence
integrity; its strict compatibility result remains false. Original Java process
outcomes and the new 18 API-row-to-native-exit contracts have distinct provenance.

The final source dictionary has 32,535 paths. A new example compilation links the
unchanged Stage96 runtime with explicit prior producer evidence. Earlier workspace,
stale Host assertion and controller prelaunch results remain preserved. In the
quickcheck recorder, `cargo_spawned` is an imprecise generic child-launch label;
actual argv identifies the Python manifest commands. No new Node/browser run or
performance result is claimed. See the [checkpoint](reports/m2/stage97-token-expression-checkpoint.json).

### Stage96 scalar, collection and function APIs

All five required Rust checks pass locally with Rust 1.98.1. The literal workspace
command, with `--no-fail-fast`, passes **1,358 tests, zero failures and one existing
ignored probe** across all 238 test executables and two doctest libraries;
25 examples are compiled. Thirteen nested summaries remain separate. The opt1,
debug0, stripped, assertions/overflow-enabled, jobs1/incremental0 workspace profile
is unchanged; Clippy and Wasm use opt0/debug0. Actual targeted compilation and
workspace `Fresh` reuse have distinct source/dep-info/artifact records.

The 103 Java query/profile contracts run twice on each of Native, N-API and
Chromium: **206 raw matches per target**, including 60 ordinary errors and
12 ordered Logger events. Status, UTF-8/UTF-16 stdout/stderr and ordered Logger
fields are compared directly; Java exception class/cause metadata is retained
without a Rust equality claim. M1 strict comparison passes 6,078 cases per target;
its architecture contract is checked separately. Targeted 17 tests are included
in the workspace total, not added to it. Both earlier targeted failures remain
recorded, along with their fixes.

Original native CLI comparisons remain **2/5 matches**: the three remaining
failures expose `Float.toFloat`, `Int.toInt` and `Set.any`. The original List Java
heap error is retained without a Rust pass. Empty file/write maps do not establish
nonempty generated-file support. The earlier full-corpus aggregate and full M2,
API, platform-release and performance gates remain open.
See the [checkpoint](reports/m2/stage96-core-apis-checkpoint.json).

### Stage95 PListRenderer

Canonical Rust 1.98.1 formatting, manifest refresh/freshness, xtask verification,
workspace/all-target Clippy (`-D warnings`) and runtime Wasm compilation pass.
Manifest refresh changes source-link hashes while preserving cases, counts and
requirement states; it is an authorized transition, not an unchanged bookend.

A single `cargo +1.98.1 test --workspace --locked -vv --message-format=json-render-diagnostics` invocation passes **1,341 tests with
zero failures and one existing ignored probe**. All 236 test executables,
25 compile-only examples and two doctest libraries are covered. The 238 outer
summaries exclude 13 nested child summaries: 12 parse directly; one is qualified
by its exact raw bytes interleaved with a lifecycle diagnostic. The recorded
profile is opt1/debug0/strip-symbols, assertions/overflow enabled, jobs1 and
incremental0; xtask shares that profile, while Clippy/Wasm use opt0/debug0.
The 25,309-path source dictionary and 208,667-path broad bookends remain stable
through compilation and test execution. Actual compiler, artifact and dep-info
records bind the canonical runtime.

The nine native original CLI-argv captures match all raw comparison channels
9/9, including one ordinary error. Empty observed file/write maps do not verify
a nonempty-file writer. The previous isolated 131 passes and 18 original API
byte-vector observations retain their own provenance. Java97 authority includes
one unresolved recursive-converter query; 96 comparable queries comprise
80 renderer-reached controls and 16 preserved syntax-preempted errors.

M1 strict/contract: 6,078 strict comparisons and the separate architecture contract pass on each target.
Selected platform result: 450/450 raw matches on each of Native, N-API and Chromium: 234 successes, 216 ordinary errors, 4,000 shared fields and 40 ordered Logger events per target.
The selected comparison covers 225 queries repeated twice on each target, not a new
whole-corpus replay. The converter session compares 11 selected rows against a Java session containing
12; the unresolved row is retained but excluded. This does not establish full
original converter-process protocol parity.

See the [Stage95 checkpoint](reports/m2/stage95-plist-renderer-checkpoint.json)
and [publication review](reports/m2/stage95-plist-renderer-publication-review.json);
evidence part identities are listed by the checkpoint. Stage94's historical
4,268/4,424 aggregate and the open full-M2/API/release/performance gates remain unchanged.

### Earlier checkpoint results

Stage94 adds an evidence-only full ready-corpus composition on the unchanged
Stage93 runtime: **4,268/4,424 raw matches, 52 mismatches, 102 unsupported cases
and two Java oracle errors**. All four canonical offline integrity checks and
an independent full-union comparison pass. The same 4,416 prior input contracts
show 46 improvements and no prior-pass regressions; eight newly admitted
historical cases match separately. The strict whole-union predicate remains
failed with 1,790 overlapping unmet conditions and 103 unready inventory cases.

This composes recovered scalar observations from an interrupted acquisition
with fresh original-module, historical and JVM captures. Original failures,
raw bytes, source/artifact identities, caller-supplied labels and the historical
launcher-source qualification are preserved. It is not a literal single-directory
`--verify-full` execution or a new engine/workspace/platform test run. The JVM
fixture mirror was released only after exact contract checks; its failed first
release and successful post-release offline verification remain recorded.
See the [Stage94 checkpoint](reports/m2/stage94-full-ready-composition-checkpoint.json)
and [evidence index](reports/m2/README.md). Stage93 implementation checks below
retain their original provenance.

Stage93 integrates the reviewed shared YamlRenderer implementation. Local
Rust 1.98.1 formatting, manifest refresh/freshness, repository verification,
workspace/all-target Clippy (`-D warnings`) and runtime `wasm32-unknown-unknown`
compilation pass. Manifest refresh preserves case/count/requirement states while
updating source links; it is recorded as an authorized transition rather than
an unchanged source bookend.

Actual Cargo metadata declares **233 test executables, 25 compile-only examples
and two doctest libraries**. The complete target union was executed in 36 bounded
invocations under opt1/debug0/strip-symbols, enabled debug assertions and overflow
checks, one build job and no incremental compilation. Canonical result:
**1,319 passed, zero failed and one existing ignored test; 13 nested child summaries are retained separately**. Outer harness results, ignored tests and nested children are counted separately.
Source, producer and dep-info readback preceded release of consumed test executables. This is not one literal workspace
command or a performance experiment.

M1 strict/contract result: **6,078 strict comparisons and the separate architecture contract pass on each of Native, N-API and Chromium**. The selected portable cohort
has 321 comparable YAML and 129 Properties queries, repeated twice: **900 observations per Native/N-API/Chromium target**, all raw-equal: 642 successes
and 258 ordinary errors, with 8,040 shared fields and 84 Logger events per target. Raw status, UTF-8/UTF-16 and ordered Logger fields
are compared without normalization; Java exception metadata and nonempty file
output remain outside this adapter. Evidence and the separate publication review are linked from the
[Stage93 checkpoint](reports/m2/stage93-yaml-renderer-checkpoint.json).
The Parser10/8-original obligations and one oracle-internal query remain open;
current readiness of 4,424/71/32 and the earlier Stage86 full replay are separate.

The [Stage92 historical-admission checkpoint](reports/m2/stage92-historical-admission-checkpoint.json)
changes tooling, registry and fixtures only.
It preserves all 4,527 case IDs and 475 requirement records while connecting
eight historical originals to an explicit Pkl 0.32.0 CLI-argv adapter. Inventory
readiness is now 4,424 ready, 71 unimplemented and 32 unmapped; a ready case is
not an automatic compatibility pass. The other historical candidates and three
selected regression CLI contracts remain unchanged.

Relevant canonical Python tests pass **76 normal and 76 optimized** checks.
`tools/m2_manifest.py --check`, `tools/m2_module_eval.py --check` and
`cargo xtask verify` pass. A finite mixed capture invokes the real canonical
runner/checker for eight historical originals and one upstream module twice:
18 comparisons match raw stdout, stderr, exit and managed file/write channels.
The target-only case selector and the actual Stage91 native producer/.d/extern
binding are recorded separately; no fresh native evaluator/probe build is
claimed for capture. `cargo xtask verify` compiled the xtask verifier. Offline CLI
subset verification passes and full-M2 verification remains failed. The prior
Stage90-native 16-comparison evidence is revalidated without changing its source
provenance. Empty actual file/write sets do not establish nonempty generated-file
support or a global filesystem sandbox. No new JVM-profile, API, release-platform
or whole-corpus result is claimed. Local evidence is in
`target/m2/stage92-canonical-{integration,checks,mixed}-01` and
`target/m2/stage92-canonical-offline-cli-check-01`.

The [Stage91 PropertiesRenderer checkpoint](reports/m2/stage91-properties-renderer-checkpoint.json)
records passing formatting, repository verification, Clippy and Wasm compilation
on macOS arm64 with Rust 1.98.1. Complete default workspace test coverage was
obtained through 36 bounded Cargo invocations and one affected-target rerun:
**1,276 passed, zero failed, and one existing ignored probe**. All 230 distinct
test executables, 25 compile-only examples and two doctest libraries are covered;
13 nested child summaries are excluded from the total. This was not one literal
`cargo test --workspace` invocation. The first acquisition's stale unsupported
PropertiesRenderer assertion remains a recorded failure. Only that test source
changed, and actual compiler dependencies establish that the other targets'
results remain applicable.

Native, N-API and Chromium each match 422 observations from 211 queries repeated
twice. One M1 acquisition matches 6,078 primitive results per target and passes
the separate architecture contract. Generated test and completed private-cache
payloads were reclaimed only after recorded identity and consumer checks;
source, raw results, dep-info and producer evidence remain. These results do not
update the Stage86 whole-corpus aggregate or establish full CLI, generated-file,
release-platform, speed or memory compatibility.

The [Stage90 collection checkpoint](reports/m2/stage90-lazy-collections-checkpoint.json)
completes all five required Rust checks locally on macOS arm64 with Rust 1.98.1.
The workspace records 1,247 passing tests, zero failures, and one existing
ignored probe; 13 nested child summaries are excluded from that total. Native,
N-API and Chromium each match 278 repeated query observations. A single M1
acquisition matches 6,078 primitive results per target and passes its separate
architecture contract. The [language evidence](docs/m2-language.md) distinguishes
these scoped results from the latest completed whole-corpus replay and the
remaining release requirements.

The [Stage86 checkpoint](reports/m2/stage86-contains-hash-power-checkpoint.json) records
all five required Rust checks, including a complete 1,149-pass workspace run
with one ignored test and zero failures. Native, N-API, and Chromium each
matched 104 repeated observations from 35 new queries and 17 distinctness
regressions. One M1 acquisition matched 6,078 results per target and passed
its separate architecture contract. A prior capture used the wrong power
assertion setting and remains failed; its successor uses the exact Java
settings. The subsequent [full-corpus replay](reports/m2/stage86-full-replay-checkpoint.json)
completed with 4,214 matches among 4,416 ready cases. Its remaining failures and
111 unready cases are preserved; this aggregate does not cover later sources.

The [Stage85 checkpoint](reports/m2/stage85-distinct-checkpoint.json) records the
five required Rust checks, including a completed 1,136-pass workspace run with
one ignored test and zero failures. The earlier capacity-stopped attempt remains
separate. Selected `distinct` regressions compare four unchanged originals and
17 Java queries, with repeated native, N-API, and Chromium captures. One fresh M1
acquisition supplies distinct strict-byte and architecture-contract judgments;
it is not reported as two executions. These results do not close the full M2,
product API, release-platform, or speed/memory requirements.

```sh
cargo xtask verify
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo check -p pkl-runtime --target wasm32-unknown-unknown --locked
```

The first command checks reference pins. The remaining commands check Rust formatting, run static analysis and the current workspace tests, and verify Wasm compilation. Successful Wasm compilation does not establish browser evaluation or API compatibility. Do not use the number of passing `cargo test` cases as a measure of Pkl feature completion.

Python verification tools use the standard library; use Python 3.12 or newer. CI selects Python 3.12. The [oracle contract](docs/oracle.md) describes the exact JDK requirement, artifact and source pins, execution environment, and evidence. With that JDK installed:

```sh
python3 tools/oracle.py fetch
python3 tools/oracle.py verify
python3 tools/oracle.py smoke
python3 -m unittest discover -s tools/tests -p 'test_oracle.py'
```

Oracle smoke checks capture Java success and error bytes and verify exact Java replay. They do not execute a Rust evaluator. The separately pinned optimizing GraalVM profile is used for performance baselines.

The [source inventory](docs/inventory.md) tracks language, stdlib, builtin, renderer, CLI, protocol, and JVM source declarations. Regeneration detects missing entries and source drift:

```sh
python3 tools/inventory.py --check
python3 -m unittest discover -s tools/tests -p 'test_inventory.py'
```

Source declaration counts and builtin registration matches are not semantic coverage or proof of compiled JVM API compatibility. Implementation and behavioral test mappings remain explicit unimplemented/unmapped records.

The [corpus contract](docs/corpus.md) accounts for reference files, resources, runnable command cases, and suites requiring additional adapters:

```sh
python3 tools/discover.py --check
python3 -m unittest discover -s tools/tests -p 'test_discover.py'
```

Ready commands have not necessarily run. Blocked suites and cases with an outcome requiring observation remain visible; file counts are not assertion counts. Historical rev1 inputs are inventoried separately. [Three selected regressions](docs/regressions.md) retain their original input/license bytes and have fresh Java 0.32.0 outcomes; the remaining historical candidates stay unselected.

```sh
python3 tools/regression_manifest.py --check
python3 -m unittest discover -s tools/tests -p 'test_regressions.py'
```

The [execution tool](docs/compatibility-tooling.md) records and checks a required seven-case smoke corpus, including error bytes and multiple generated files:

```sh
python3 tools/smoke_manifest.py --check
python3 -m unittest discover -s tools/tests -p 'test_compat.py'
python3 tools/compat.py record --manifest compatibility/smoke.json --output target/m0/smoke-baseline
python3 tools/compat.py check --manifest compatibility/smoke.json --output target/m0/smoke-baseline
```

Use a new evidence directory for each run. Candidate comparison requires a complete Java baseline and the same workspace path; the tool verifies source/resource hashes, runner provenance, raw captures, and case coverage. Java-to-Java replay validates tooling only. The process adapter currently requires POSIX process groups. The full discovered corpus includes blocked cases and therefore fails the strict completion gate until all required adapters and execution conditions exist; the smoke corpus is not a substitute for full compatibility.

Reviewable evidence archives under `reports/m0/` retain raw outputs and a per-file checksum inventory. Verify an archive with `python3 tools/evidence.py verify <archive.tar.gz>`. Copied source workspaces can be excluded explicitly; the archive records its exclusions and retains the pinned source manifest.

Pantry has a separate [development and packaged-module contract](docs/corpus.md):

```sh
python3 -m unittest discover -s tools/tests -p 'test_pantry.py'
python3 tools/pantry.py --workers 4
python3 tools/evidence.py verify reports/m0/pantry-evidence.tar.gz
```

The runner requires a new `target/m0/pantry` evidence directory. It runs package/API tests, evaluates generated ZIP modules, checks configured generators and the tar pipeline, and records dependency payload hashes. Abstract or user-configured module errors remain provisional observations. Reported test/assertion invocation totals can repeat dependency suites; they are not distinct coverage counts. Unfixed live GitHub discovery inputs remain outside the hermetic baseline.

Run all verification-tool regressions with `python3 -m unittest discover -s tools/tests -v`. The [M0 report](reports/m0/README.md) links recorded outcomes and raw evidence. The [CI contract](docs/ci.md) distinguishes the required tooling smoke run from optional full-corpus acquisition and performance measurements.

## Compatibility Comparisons

The [Class-value contract](docs/m2-class-values.md) separates declaration identity,
instance prototypes, superclass links, and representation-based type/converter
rules. Its independent Java query/logger observations, ModuleKey SPI observations,
and exported-Class identity observations are distinct verification boundaries.
The common native/portable matrix cannot close the required provider/export
supplements. Run fixture integrity, strict query comparison, and full API checks
separately; retain unsupported values and callback/oracle failures.

The [object-body parameter contract](docs/m2-object-parameters.md) keeps Java
observations for Mixins, function amendments, and lexical captures independently
pinned. Native and portable comparisons use the original arbitrary expressions
and resource bytes through the public shared evaluator. Unsupported outcomes and
Java internal errors remain separate from matching ordinary values and language
errors. Lifecycle regressions cover suspended parameter checks, escaped curried
closures, reentry, retry, cancellation, and terminal code/heap release.

The [canonical diagnostic contract](docs/m2-diagnostics.md) keeps actual Java
UTF-16 strings separate from their Java UTF-8 serialization. Native and portable
checks preserve both channels; historical byte-only fixtures do not establish
string-unit equality. SourceSession, scalar helpers, frontend constructors, and
N-API error callbacks have distinct recorded boundaries.

The [portable Resource reference fixtures](docs/m2-resource-portable-fixtures.md)
retain 1,008 selected Java API observations and their original bytes, UTF-16
units, ordered reader inputs and counters. Their offline verifier and tamper
tests establish fixture integrity; they do not execute Rust, Node or a browser:

```sh
python3 tools/resource_fixtures.py --fixtures tests/fixtures/m2-resource-portable
python3 -W error::ResourceWarning -m unittest discover -s tools/tests -p 'test_resource_portable_fixtures.py' -v
```

The [renderer reference fixtures](docs/m2-renderer-oracle.md) preserve 860 Java
API calls in 44 persistent sessions. Their offline verifier authenticates the
original inputs, raw UTF-16/Java UTF-8 channels and reader plans before accepting
the exact fixture file set. The regression suite exercises re-signed fixture
forgeries and rejection of undeclared directories before traversal:

```sh
python3 tools/renderer_fixtures.py --fixtures tests/fixtures/m2-renderer-portable
python3 -W error::ResourceWarning -m unittest discover -s tools/tests -p 'test_renderer_fixtures.py' -v
```

The renderer platform driver requires separately recorded binding artifacts.
Offline fixture success does not establish runtime or platform compatibility.

The compact frame-identity fixture preserves three unchanged Java programs,
12 evaluator sessions and 240 complete API observations. It covers ordinary and
typed-union generator frame restoration and numeric constants in distinct tagged
instances. Its verifier authenticates the original Java records and both TSV
indexes before accepting their exact file set:

```sh
python3 tools/renderer_identity_fixtures.py --fixtures tests/fixtures/m2-renderer-frame-identity
python3 -m unittest discover -s tools/tests -p 'test_renderer_identity_fixtures.py' -v
cargo test -p pkl-runtime --locked --test m2_renderer_frame_identity_oracle
```

The Rust regression consumes the same 240 observations with Observe disabled and
enabled, comparing status, complete UTF-16 channels and Java UTF-8 bytes. The
actual Java acquisition used power assertions enabled only. These are two Rust
configurations, not two Java baselines. Every unexpected Host effect fails the
test; collection runs after each poll and closure must release all slots. The
fixture is a subset of separately acquired renderer observations and does not
add independent whole-language coverage. Policy profiles, explicit-close API
parity, complete producer identity and equivalent GC event traces remain separate.

The [module preparation platform contract](docs/m2-module-preparation-platforms.md)
adds exact provider-input lookup and ordered Java-observable policy comparisons.
Its 252 selected calls distinguish resource policy denial from a counted reader
callback and retain the 48 unselected API boundaries. Run its offline checks with:

```sh
python3 tools/module_portable_fixtures.py --fixtures tests/fixtures/m2-module-portable
python3 -W error::ResourceWarning -m unittest discover -s tools/tests -p 'test_module_portable_fixtures.py' -v
node --test tools/platform/resource-session.test.mjs tools/platform/module-preparation-session.test.mjs
```

Actual Node/Chromium results require the separately documented driver and frozen
binding artifacts. New browser acquisitions explicitly launch the executable
whose identity is recorded. Historical default-headless captures do not establish
that their recorded Chromium installation path was the executed binary.

The [module admission policy fixtures](docs/m2-module-admission.md) preserve
84 original Java API calls and 280 ordered events, including lifecycle events
that the current supplied-primary Rust session does not reproduce:

```sh
python3 tools/module_admission_policy_fixtures.py --fixtures tests/fixtures/m2-module-admission-policy
python3 -m unittest discover -s tools/tests -p 'test_module_admission_policy_fixtures.py' -v
```

Fixture integrity, raw API equality and full policy event/counter equality are
separate checks. Missing lifecycle events remain failed comparisons.

The [ModuleKey reference fixtures](docs/m2-module-keys.md) separately preserve
212 Java API calls, 36 sessions and 2,248 ordered events. Their key/cache and
factory-failure cohorts retain original inputs, raw output/error channels,
counter bytes and exception metadata:

```sh
python3 tools/modulekey_fixtures.py --fixtures tests/fixtures/m2-modulekey-spi
python3 -W error::ResourceWarning -m unittest discover -s tools/tests -p 'test_modulekey_fixtures.py' -v
```

Offline fixture integrity does not establish native or portable execution.
Supplied-primary and public-entrypoint experiments must retain distinct results;
raw API agreement cannot replace full ordered lifecycle observations.

The [extended ModuleKey contract](docs/m2-modulekey-extended.md) separately
preserves 944 API calls and 10,124 ordered Java events across URI, contents and
REPL sources, changing factory/source/policy state, URI overrides and fragment
capabilities. Its fixture verifier and transport regressions are:

```sh
python3 tools/modulekey_extended_fixtures.py --fixtures tests/fixtures/m2-modulekey-extended
python3 -W error::ResourceWarning -m unittest discover -s tools/tests -p 'test_modulekey_extended_fixtures.py' -v
node --test tools/platform/modulekey-extended-session.test.mjs tools/platform/compare-modulekey-extended.test.mjs
```

Keep ordinary compatibility, diagnostic-only matches, Java internal-error
boundaries, raw equality, acquisition/cleanup integrity and the full Java
event/counter gate separate. A completed capture with preserved mismatches is
not a strict compatibility pass. An older frozen engine used to debug a new
provider does not verify a subsequent Rust runtime candidate.

The [collection transform contract](docs/m2-collection-transforms.md) records
Java-authenticated callback, conversion, identity, and quantity controls through
the native and actual Node/Chromium evaluators. Ordinary operations, inputs that
missed their intended feature, and post-close API boundaries remain separate.
Raw failures and full Java lifecycle gaps are retained even when acquisition
integrity passes.

M2 language implementation and verification are in progress. The
[M2 contract](docs/m2-language.md) separates whole-language requirement accounting
from the executable selection:

```sh
python3 tools/m2_manifest.py --check
python3 tools/m2_language.py --output target/m2/language-new --record-only
python3 tools/m2_language.py --verify-report target/m2/language-new
python3 tools/m2_language.py --verify-development target/m2/language-new
python3 tools/m2_language.py --verify-subset target/m2/language-new
python3 tools/m2_language.py --verify-full target/m2/language-new
```

The full gate must fail while required language implementations, mappings,
adapters, or passing executions are missing. It cannot be replaced with the
selected-case gate.

The [Stage69–72 Java authority checkpoint](reports/m2/stage69-72-java-checkpoint.json) separately preserves 235 authored queries for super entry defaults, import/generic type lookup, VarArgs, base aliases/Mixin, and const owner behavior. Its six archives retain complete raw repetitions, logger channels, earlier failed attempts, and source/JDK/JAR identities. The [publication process record](reports/m2/stage69-72-java-publication-execution.json) contains actual commands, exits, and raw channels; Java publication is not a Rust compatibility pass. The separate [implementation checkpoint](reports/m2/stage69-72-implementation-checkpoint.json) records all five required Rust checks, 235+4 selected native/N-API/Chromium query comparisons, the full 4,416-case replay, and a regression comparison against Stage68. The full 4,416-case Rust replay records **4,158 matches, 115 mismatches, 141 unsupported results, 2 Java oracle errors, and 0 Rust internal errors**. Original modules account for 638/888 matches; expression strings account for 3,519/3,527, and the JVM classpath case matches. Compared with the latest Stage68 Rust baseline, 6 previously nonpassing cases now match and no previously passing case regresses. The strict full gate and the two known M1 Regex failures remain failed; the 111 unready inventory cases remain unready. Java Logger authority is absent for 91 queries in the main selected matrix.

Stage68 adds independent Java authorities for String Base64, collection emptiness,
and `every`. Verify their exact archives, inputs, raw repetitions, and provenance:

```sh
python3 -B tools/string_base64_fixtures.py
python3 -B tools/collection_emptiness_fixtures.py
python3 -B tools/collection_every_fixtures.py
```

These commands use tracked fixture roots and work without historical `target/`
directories. The authorities contain 89, 47, and 64 expressions respectively;
`every` also retains ordered logger events and their UTF-16 units. They prove
Java reference integrity, not Rust compatibility. The separate
[Stage68 implementation checkpoint](reports/m2/stage68-implementation-checkpoint.json)
records all five required Rust checks and 2,726 exact catalog observations per
native/N-API/Chromium target, plus 128 logged `every` observations per target.
The full ready replay has 4,152 matches, 119 mismatches, 143 unsupported results, 2 Java oracle errors, 0 Rust internal errors;
its 1,799 overlapping unmet full-gate entries remain failures. It reuses the
sealed Stage67 Java evidence and executes Rust anew, with no fresh Java process.
Replay integrity and strict compatibility remain separate verdicts. The 111
unready cases and Any/M1 Regex failures are preserved. The Stage67 results below
describe the earlier frozen candidate.

Stage67 adds the String property authority (`python3 tools/string_properties_fixtures.py`)
and a fifteenth collection catalog profile. The Stage67 catalog has 1,227
expressions and requires two complete raw observations per expression on each
target. Its selected comparisons are separate from the full ready corpus.
The [Stage67 checkpoint](reports/m2/stage67-implementation-checkpoint.json)
records 4,416 ready comparisons: 4,142 matches, 128 mismatches, 143 unsupported
results, two Java oracle errors, and one Rust internal error. Strict full
verification exits 1 with 1,802 unmet gate entries. Preserve the 111 unready
cases, inherited scalar regressions, and Any/M1 Regex failures. A successful
record-only acquisition verifies evidence integrity, not M2 completion.

Stage66 reversal and constructor authorities are four distinct single-module
string fixtures; union-marker diagnostics use forty separate modules:

```sh
python3 tools/reversal_values_fixtures.py
python3 tools/reversal_implicit_fixtures.py
python3 tools/reversal_subscript_fixtures.py
python3 tools/base_constructor_constraints_fixtures.py
python3 tools/union_marker_fixtures.py
```

Fixture integrity is separate from candidate compatibility. The Stage66 collection
catalog snapshot contains fourteen profiles and 1,081 expressions; native and portable
comparators require two full raw repetitions per profile. The separate
`tools/union_marker_compare.py` and
`tools/platform/capture-union-marker-modules.mjs` run each exact diagnostic module
in fresh native or actual N-API/Chromium sessions. Their string gates compare all
seven common channels and whole process output; Java exception metadata and full
API requirements remain separate.

The recorded Stage66 union portable invocation used a Node symlink while
`process.execPath` recorded its resolved binary. The original failed verification
and unchanged capture remain preserved. The versioned adapter under
`target/m2/stage66-node-path-verification-amendment-02` verifies both paths and
recorded before/after executable hashes, then forwards a temporary locator
metadata copy to the original source-pinned verifier. It changes no captured
file or guest byte. Restore its published closeout evidence to replay that
historical invocation; its strict string gate passes and full-API gate fails.
New capture wrappers should record the resolved executable directly. The
[implementation evidence](reports/m2/stage66-implementation-checkpoint.json)
retains this admission boundary, the earlier failure, known Regex differences,
and the separate 131-program original results.

The inferred-construction authorities retain five separate observation scopes:
72 ordinary expressions, seven real amending-module queries, two opposite
eleven-query module-receiver orders, 44 method-cache queries, and four separate
default-diagnostic modules. Verify their sealed Java captures with:

```sh
python3 tools/inferred_new_fixtures.py --profile values
python3 tools/inferred_new_fixtures.py --profile amends
python3 tools/inferred_new_module_receiver_fixtures.py
python3 tools/inferred_new_method_cache_fixtures.py
python3 tools/inferred_new_default_diagnostics_fixtures.py
```

Only the ordinary 72-expression profile belongs to the single-module comparison
catalog. Preserve sibling modules, receiver order, evaluator lifetime, and Host
requests in the other scopes. The method-cache archive also retains an earlier
object-method alias experiment with eight Java internal errors per repetition;
those observations remain oracle failures. Fixture integrity does not establish
Rust compatibility. Candidate comparisons must retain complete raw channels,
and native multi-module regressions do not replace the corresponding portable
execution requirements.

Lifecycle tests that measure user-object heap deltas use the test-only
[constructor baseline](crates/pkl-runtime/tests/support/constructor_baseline.rs).
It initializes
the three canonical collection parents through compiled constructor sites
before taking the baseline, without forcing authored members. Production
initialization and compatibility captures remain lazy. Separate VM tests cover
first-use parent caching, cache identity, code ownership, and terminal cleanup;
the lifecycle tests retain exact temporary-object and executable-node deltas.

Stage 64's independent amendment authorities have separate offline verifiers:

```sh
python3 tools/amendment_construction_fixtures.py
python3 tools/amendment_declarations_fixtures.py
python3 tools/prototype_diagnostics_fixtures.py
python3 -m unittest tools.tests.test_amendment_construction_fixtures tools.tests.test_amendment_declarations_fixtures
python3 -m unittest tools.tests.test_prototype_diagnostics_fixtures
```

The construction fixture contains 188 ordered public string queries and is
registered in the shared comparison catalog. The declaration fixture contains
eleven distinct modules and remains outside that single-module profile. Direct
VM identity, Host logs, and the additional local-amendment/EOF controls in the
[Java checkpoint](reports/m2/stage64-java-checkpoint.json) retain their own
observation boundaries. Fixture verification establishes the Java authority;
candidate process, native, Node, browser, and full M2 checks remain required.

The separate [prototype diagnostic fixture](tests/fixtures/m2-prototype-diagnostics/README.md)
contains 39 queries in two fresh Java sessions. It retains complete operator,
missing-method and class-name observations for builtin and user prototypes,
including unforced member displays. It does not change the construction or
declaration authorities and is not registered in the shared comparison catalog.

The [DataSize contract](docs/m2-data-size.md) defines two complete ordered string
sessions per target against the independent split Java fixture. Verify native
capture integrity with `tools/data_size_compare.py --verify-report`, strict
strings with `--verify-string`, and the separate required API gate with
`--verify-full`. Each option takes an existing capture directory. A recorded
helper build is required for a fresh `--output ... --build-record ...` acquisition.

The portable verifier takes `--capture`, `--m1`, `--build-wrapper`, and
`--capture-wrapper` evidence paths,
with `--gate integrity|string|full-api`. It binds a frozen M1 build separately
from any explicitly recorded capture-only transport change. Browser page protocol
and console errors remain distinct from operating-system process channels. Both
native and portable full API gates remain unsuccessful while typed exports are
unexecuted. Original upstream programs retain their own complete CLI contract.

The [Any.ifNonNull contract](docs/m2-any-if-non-null.md) uses separately sealed
86-row and 13-row Java authorities. Its native and portable verifiers compare
exact ordered expression-string channels; `--gate integrity` checks complete
source-bound evidence, while `--gate string` requires every selected row in both
repetitions. The portable verifier takes `--capture` and `--m1`; both verifiers
retain the unsupported Regex and IntSeq constructor rows as failures. The
required typed export and full M2 gates remain open. The Stage 58 recorded
comparisons have 168/172 matches per native, N-API, and Chromium target; the
DataSize regression retains 510/510 on each target. Of 17 unchanged original
upstream programs, ten pass complete raw comparison and seven mismatch,
including Bytes, Duration, and Pkl Binary cases.

The Stage 59 `Duration` contract is recorded in
[the M2 language guide](docs/m2-language.md). Verify the separately sealed
274-row Java string authority and 15-row exported-value bit supplement before
candidate comparison:

```sh
python3 tools/duration_values_fixtures.py
python3 tools/duration_values_bits_fixtures.py
python3 -m unittest tools.tests.test_duration_values_fixtures tools.tests.test_duration_values_bits_fixtures
cargo test -p pkl-runtime --test m2_duration --locked
```

The Rust test's raw-bit assertions are internal value checks against actual
Java exported `Duration` observations; they do not establish typed product
API parity. Use fresh directories and an existing recorded native build for
the two complete ordered expression-string sessions:

```sh
python3 tools/duration_values_compare.py --acquire target/m2/duration-native-new --build target/m2/duration-build-record
python3 tools/duration_values_compare.py --verify target/m2/duration-native-new --gate string
node tools/platform/capture-duration-values.mjs target/m2/duration-portable-new WASM_DIR NAPI_BINARY tests/fixtures/m2-duration-values
python3 tools/duration_values_portable_compare.py --capture target/m2/duration-portable-new --m1 M1_PLATFORM_RECORD --native-build target/m2/duration-build-record --build-wrapper M1_BUILD_WRAPPER --gate string
```

The portable verifier requires both `--native-build` and `--build-wrapper` to
bind the supplied N-API and Wasm artifacts to recorded sources. The native
and portable `--gate integrity` options check evidence independently of exact
string equality. The portable `--gate full-api` remains unsuccessful while
typed exports are unexecuted; the native runner has no full-API gate. The
string gate compares the entire status, expression,
stdout, stderr, UTF-16BE, and process-protocol bytes across two repetitions.
Original upstream Duration snippets retain their separate CLI and generated
file comparison contract. The [Java-only checkpoint](reports/m2/stage59-java-checkpoint.json)
does not claim native, Node, or browser compatibility.
The later recorded Stage 59 local comparisons match all 548 Duration string
executions on each of native, N-API, and Chromium, including complete process
protocols. The unchanged DataSize string regression matches 510 on each
target. Portable `Any.ifNonNull` still has four Regex/IntSeq constructor
mismatches per target. Of 17 separately executed original modules, 11 match,
five mismatch, and one is candidate-unsupported; `basic/duration.pkl` newly
matches while `api/duration.pkl` reaches an unimplemented `pkl:math` use.
These selected results and internal raw-bit assertions do not satisfy the
typed export, whole-language, product API, release-platform, or performance
gates. The [M2 evidence report](reports/m2/README.md) tracks the distinct
recorded scopes.

The Stage 60 Bytes fixture contains 300 independent Java expressions in two
fresh JVM sessions. Its 17 producer channels include exception and cause
metadata; the current candidate string boundary compares seven common raw
channels and the complete ordered protocol. Do not count producer-only metadata
as candidate API passes. At Stage 60, the strict native and portable comparisons
retained six IntSeq mismatches per target, with 594 matches. Its ignored Rust
test preserved the three underlying cases without counting them as passes.

```sh
python3 tools/bytes_values_fixtures.py
python3 tools/bytes_values_compare.py --acquire target/m2/bytes-native-new --build target/m2/bytes-build-new --gate string
python3 tools/bytes_values_compare.py --verify target/m2/bytes-native-new --gate string
python3 tools/bytes_values_portable_compare.py --capture target/m2/bytes-portable-new --m1 target/m2/bytes-platforms-new --native-build target/m2/bytes-build-new --build-wrapper target/m2/bytes-platform-wrapper-new --gate string
```

Acquisition requires an existing recorded build; portable verification also
requires the matching actual N-API/Chromium capture, M1 artifact set, and source
wrapper. The [Stage 60 evidence](reports/m2/stage60-implementation-checkpoint.json)
preserves those exact inputs and commands. Strict comparisons exit nonzero for
the retained failures even when acquisition and evidence integrity pass.
The Java session flags establish effective `en-US` and `UTC`, independently
of the preflight locale. Portable capture checks the recorded requested process
environment and actual browser observations without claiming a complete
OS-injected Java environment observation.

Stage 60 passed the five required local Rust checks: 940 test executions passed
and four were ignored. Its initial unchanged 256 KiB frontend stack test failed;
the compiler fix and subsequent pass retain the original stack limit and input.
The later disk-full evidence-write failure is recorded separately. Tool checks
pass 25 Python tests normally and under `-O`, plus 19 JavaScript tests.
Duration and DataSize string regressions match 548 and 510 observations on each
target. Portable Any and the M1 regex experiment retain their known failures.
Original modules have 12 matches, four mismatches, and one unsupported result
among 17; each program has one Java and one Rust execution. Offline verification
does not add evaluation repetitions. Typed exports, mutable Java array aliasing,
whole-language and product compatibility, and performance remain unverified.

The [Stage 61 IntSeq checkpoint](reports/m2/stage61-implementation-checkpoint.json)
uses 237 ordered string expressions in each of two fresh Java sessions, with
198 successes and 39 ordinary errors per session. Three typed-export rejection
rows are captured separately and are not candidate string comparisons.
The shared native probe accepts the same three-column `string`, ID, expression
input as Java, while preserving the existing two-column probe format. It rejects
other modes, extra columns, and duplicate IDs. The comparator checks all seven
common raw channels and the full ordered protocol, including failed expressions.

```sh
python3 tools/int_seq_values_fixtures.py
python3 tools/int_seq_values_compare.py --acquire target/m2/intseq-native-new --build target/m2/intseq-build-new --gate string
python3 tools/int_seq_values_compare.py --verify target/m2/intseq-native-new --gate string
python3 tools/int_seq_values_portable_compare.py --capture target/m2/intseq-portable-new --m1 target/m2/intseq-platforms-new --native-build target/m2/intseq-build-new --build-wrapper target/m2/intseq-platform-wrapper-new --gate string
```

As with Bytes, acquisition and portable verification require the recorded build,
actual platform capture, M1 artifacts, and source wrapper. All 474 IntSeq
observations match on each target. The unchanged Bytes regression now matches
600 of 600, and its formerly ignored IntSeq-dependent cases execute and pass.
Duration and DataSize retain 548 and 510 matches per target. Portable Any retains
170 matches and two Regex differences per target; its strict gate still fails.
All five required Rust checks pass: 953 test executions pass and two remain
ignored. Tool checks pass 37 Python tests normally and under `-O`, plus 13
JavaScript tests. Native prerequisite regressions cover 17 math strings and
numeric bit values, 21 List.add cases, and 65 List accessor/fold cases; they do
not establish typed export or separate portable supplement compatibility.

The expanded 62-program original comparison has 26 matches, 15 mismatches,
and 21 unsupported results. Its previous 17-program subset has 15 matches and
two mismatches, with no passing regression. These are one execution per runtime
per original, separate from repeated authored string sessions. Source, artifact,
oracle, and workspace integrity pass while selected/full compatibility fails.
Full M2 completion, external math functions, typed APIs, release-platform
coverage, and performance targets remain separate outstanding requirements.

The [method-deprecation contract](docs/m2-deprecation.md) records the pinned Java
logger matrix and a separate experiment with sequential evaluators in one JVM.
Its independent fixture verifier checks raw UTF-8, UTF-16, frame, and callback
observations. Java internal failures and API exceptions remain nonpasses;
evidence integrity does not establish candidate compatibility. Recorded candidate
comparisons retain Java internal-error nonpasses and the separate, unexecuted
cross-evaluator lifetime obligations.

The [frame-transformation contract](docs/m2-frame-transformers.md) compares
complete original and replacement frames, nullable members, arbitrary UTF-16,
public result/error payloads, and ordered callback protocol on native, N-API,
and Chromium. `tools/frame_transformer_compare.py --verify-report` checks the
captured evidence; `--verify-full` is the strict requested main matrix. It does
not execute the Java-only test-buffer supplement or replace the whole M2 gate.
The [Stage55 checkpoint](reports/m2/stage55-implementation-checkpoint.json)
retains all 42 exception-related main nonpasses, the initial frame-location
failure, and the recorded default trace/deprecation regressions. Required Rust
checks combine four original passes with an unchanged-input Wasm retry after
disk exhaustion; the initial failure remains part of the evidence.

The [standard-module and guest-catch contract](docs/m2-stdlib-bootstrap.md)
records Java-origin catch, ModuleInfo, and typed security-denial fixtures, native
consumers, and actual Node/browser comparisons. It distinguishes full ordered
API calls from constructor-only, terminal partial, internal-error, and
unexecuted independent-root boundaries. Broader bootstrap comparisons require
observable resource-read counts as well as complete result channels; shared
Rust-target agreement does not erase Java failures.

The [original module contract](docs/m2-module-frontend.md) describes the
additional `language-module-eval` adapter, included in default language
acquisition. It preserves original upstream argv, environment, resources, and
physical paths, comparing complete native process and generated-file channels.
Focused adapter selection leaves all omitted cases required by the full gate.
Node and browser byte vectors are checked separately from native process
execution. The native probe evaluates actual module output. Portable output
comparisons explicitly select `output-bytes`; the retained frontend-only API
continues to report successful preparation as unsupported output evaluation.
An original case's unmet execution prerequisites or expected exit status cannot
be satisfied by raw agreement with a setup-dependent Java failure.

The [JVM execution-profile integration](docs/m2-jvm-classpath.md) explicitly maps
the original `import1b` ID to its JVM/classpath contract while preserving the
historical blocked CLI record. The parent records the native build and binds its
frozen bytes, sources, and oracle to two child repetitions. Those repetitions
count as one original case. Focused adapter success leaves every omitted case
required by the full gate. Both platform and M2 source collectors explicitly
capture the compile-time standard-module inventory; historical captures that
omitted it retain their separate provenance qualification.

The [shared module-loading contract](docs/m2-module-loading.md) adds Java-origin
module graph fixtures and typed Host continuation checks. Verify fixture
provenance separately from actual evaluator execution:

```sh
python3 tools/module_loading_fixtures.py
cargo test -p pkl-runtime --test m2_module_loading --test m2_module_transport --test m2_module_diagnostics --test m2_session_expressions --locked
```

These checks do not replace original native process comparisons or actual
Node/browser execution. Remaining module inheritance, URI and external-capability
requirements stay in the full language gate.

Class and type-alias initialization have a separate persistent-evaluator contract:

```sh
python3 tools/m2_class_lifecycle.py --output target/m2/class-lifecycle-new --record-only
python3 tools/m2_class_lifecycle.py --verify-report target/m2/class-lifecycle-new
python3 tools/m2_class_lifecycle.py --verify-contract target/m2/class-lifecycle-new
```

New captures use report schema 2. Native source snapshots must include the root
`.cargo/config.toml`, Cargo manifests and lockfile, and the pinned Rust toolchain;
missing build controls fail acquisition. Schema 1 records omitted the root Cargo
configuration. Verify those immutable records with their captured
`sources/tools/m2_class_lifecycle.py`; do not upgrade their schema or describe a
separate configuration binding as a directly captured input. The current verifier
rejects schema 1 explicitly. Java-only captures do not require Rust build inputs.

Each sequence retains one module and evaluator across ordered calls, including
errors, and repeats with a fresh evaluator. Every selected call must match its
raw Java status, stdout, and stderr without exceptions. These sequences have
separate execution/call counts and do not close the full language gate. Native
GC and Host lifecycle tests separately check class and alias roots, temporary reclamation,
ordinary lazy retries, and terminal release. See the
[sequence contract](docs/m2-language.md#persistent-class-initialization-sequences)
for source/build provenance, adapters, and remaining scope.

Type-resolution regressions compare authored child order, base-name failures,
class versus alias arity checks, generic parameter shadowing, nested substitution,
and owner signature initialization. Repeated calls preserve failed declaration
state while ordinary lazy expressions retry. Native Host/GC tests separately
exercise nested generic constraints, suspended values, heap isolation, temporary
reclamation, nonterminal cancellation, and terminal cleanup. Node and browser
sequence scenarios retain the same Java-derived complete payloads.

Recursive constraint regressions distinguish unused declarations, short-circuiting,
finite direct/mutual recursion, lexical type scope, power assertions, and known
shared-receiver or generic gaps. Separate Host tests force collection during every
yield, nested reentry and failure retry, stale completion rejection, nonterminal
cancellation, and complete terminal release. `Class<T>` cases verify erased
arguments without publishing broken declarations, while preserving alias argument
resolution and wrong-arity errors.

Finite generic recursion additionally compares changing `List<T>` arguments,
mutual recursion, parameter shadowing, first failures, and inline/lambda versus
shared let targets. Persistent sequences use different actual arguments within
one evaluator. Native tests carry these argument environments through real Host
suspension, child reentry, failure retry, GC at each yield, cancellation, and
terminal release. The original infinitely expanding generic predicate retains
its historical native-stack abort evidence. Shared metadata regressions exercise
20,000-level graphs on 256 KiB stacks, concurrent last-owner release, deep
Reference metadata, and cancellation while collection defaults descend and
unwind. Separate process-isolated evaluator tests retain the original infinite
source and verify extended bounded execution, GC, cancellation, and teardown.
The unchanged development adapter's instruction budget is not a Java-compatible
stack-overflow diagnostic. These checks do not establish uniform wall-time
cancellation or peak-memory parity. Let/for const cases distinguish body scope
from initializers, annotations, eager generator inputs, lambdas, and sibling
members. Generic collection defaults retain original Java and pre-change Rust
controls. Reference metadata checks separately cover constrained actual arguments,
validation before a throwing value, deferred branches, shared let roots, and
persistent retries; existing inline diagnostic gaps remain failed observations.

Nullable-default regressions compare attached class and collection defaults,
empty versus nonempty amendments, nested nulls, and null-propagating operations.
Equality, structural keys, coalescing, type checks, and diagnostics must retain
ordinary null behavior. Native tests trace attached defaults through captured
closures, Host failure and retry, cancellation, evaluator isolation, and terminal
release. These checks do not establish bounded wall-time cancellation or full
generic-constructor compatibility.

Constraint receiver tests distinguish same-call mutation from separate function,
member, let-root, and Host reentry frames. Lambda receiver capture is checked at
creation, with GC between instructions and explicit cycle reclamation after
cancellation. Boolean and function-predicate failure values are compared with
power assertions off and on. Alias const tests separate first-read diagnostics
from unresolved persistent local-property resolution caches; retained retry
witnesses must not be counted as passing first-read cases.

Collection regressions cover lazy bodies, eager keys and source forcing,
inheritance order, primitive key equality, generator captures, spread, and
complete subscript diagnostics. Native lifecycle tests also force collection
construction through nested Host suspension, GC, retry, cancellation, and
timeouts. Shared Node/browser scenarios exercise the same collection engine and
Java-derived error bytes. Generic collection regressions additionally cover
lazy `Listing<T>` values, eager `Mapping<K,V>` keys, strict `List<T>` members,
nested wrappers, casts, unions, aliases, defaults, cache reuse, and first errors.
Lifecycle checks retain temporary and successful wrapped values across GC,
Host suspension, reentry, failure retry, and terminal cleanup. Complete custom
defaults, format-time wrapper checks, and the full collection corpus remain
unverified.

Member predicate regressions compare parent-only selection, interleaved forcing
and condition evaluation, declaration-owner caches, replacement versus amendment,
lexical and custom `this` scopes, nested generators, duplicates, const checks,
and complete diagnostics. Dedicated lifecycle tests retain the parent cursor,
prior custom receiver, and unpublished target across Host suspension, nested
evaluation, GC, retry, cancellation, close, and timeouts.

Structural equality regressions distinguish scalar numeric comparisons from
boxed nested values and keys, including NaN and signed zero. They cover object
kind and class identity, empty-body identity, deep forcing order, inherited
overrides, wrapper checks, and strict List shortcuts. Key cases include lazy
small maps, the fifth-member hash transition, ancestor lookup, Mapping length,
and complete duplicate/missing-key diagnostics. Constant List keys are checked
before evaluation only when every key in a specialized body is constant;
dynamic keys, generators, and shadowed constructors retain runtime behavior.
Signed literal token spans are preserved separately from expression spans.
Source-line rendering retains the pinned highlighter's interpolation whitespace
behavior and whole-line fallback on lexical errors; underlines still use
authored UTF-16 positions.
Lifecycle tests exercise temporary operands and keys through Host suspension,
GC, reentry, retry, and terminal cleanup. Cyclic keys and complete Java
hash-bucket/cache behavior remain outside the verified selection.

Additional comparison regressions cover ordinary typed `default` properties,
inherited and hidden properties, completed regular-member cache order, and
class declaration order independently of amendment order. Key preparation
cases distinguish whole-object deep forcing from subsequent recursive member
hash preparation. Host lifecycle tests cross this phase boundary and preserve
completed values while retrying failed reads. Actual Node and Chromium scenarios
compare values and complete diagnostics captured from the pinned Java oracle at
the identical module URI.

Map regressions distinguish structural equality from numeric hashes and bucket
membership. Pinned JAR observations supply collision order, signed hash mixing,
four-to-five entry transitions, load growth, capacity boundaries, and partial
rehash visibility. Language cases cover warmed Listing keys, constant and
nonconstant local caches, scalar hashes, and class qualified names. Host tests
publish a value before suspending in key hashing, then fail, retry, traverse old
and new cache slots, and collect between VM steps. They separately exercise
cancel, timeout, close, and stale completions. These lifecycle checks are not
raw Java byte comparisons or proof of same-cache cyclic reentry compatibility.
Mapping length regressions separately check HashSet full-hash filtering,
directional local-cache equality, inherited structural overrides, and boxed
numeric distinctions. Its Host test retains ancestor values across a failed
child hash and retries the count without forcing entry values.

Let diagnostics compare complete Java output for nested bodies, initializer
failures, typed bindings, captured members/functions, and constraints with their
own logical root. Recursive call controls distinguish inserted let frames and
selected uncolored repeated-frame compression. General stack-overflow and
colored-output behavior remain open. Failed exploratory captures remain
retained alongside promoted regressions.

Type-use ownership tests distinguish persistent executable nodes from per-call
constraint receiver slots. Retained-heap assertions count ordinary values and
executable entries separately, preserve exact historical value counts, and
require zero total entries after terminal cleanup. They cover source-level Host
reentry, cancellation before and after local resolution, independent evaluator
state, recursive retries, and lazy collection members. Persistent Java sequences
and Node/browser scenarios compare complete ordered outcomes separately from
these internal lifetime checks. Finite generic instantiation and selected
let/for boundaries are included; complete generic instantiation, alias-root
identity, and the full lexical scope model remain outside this bounded contract.

Direct generic constructor comparisons distinguish bare type parameters from
explicit generic aliases, nullable defaults, actual-type diagnostic origins,
and unused versus forced member failures. Ordered calls alternate successful
and invalid actual types without resetting the evaluator. Native tests separately
check Host reentry and failed-read retry, 24 nonterminal cancellations, captured
functions, evaluator isolation, and terminal release. A process-isolated test
constructs 768-deep defaults on a 256 KiB evaluator stack with a 60-second parent
deadline; compilation runs on the ordinary stack. That test collects during
bounded execution and does not establish peak-RSS or wall-time parity with Java.

Strict Map comparisons cover per-pair key/value evaluation and immediate key
hashing, duplicate replacement with preserved insertion positions, boxed keys,
lookup, parameterized checks, nested Maps, and selected equality/hash ordering.
Map rendering is non-forcing; its observations are separate from deep forcing
and hashing. Persistent sequences retain successful and failed calls in one
evaluator. Native tests collect during cooperative yields and exercise pending
builders, query keys, value comparison, Host reentry, failed-read retry,
cancellation, evaluator isolation, stale handles, and terminal cleanup. Nested
Map hashing also runs at depth 768 on a 256 KiB evaluator stack in a child
process with a 60-second parent deadline. JVM identity-dependent hash order,
general compound output and the remaining Map APIs stay outside this
bounded development contract.

Strict Set comparisons additionally distinguish lookup from insertion hashing, duplicate
elimination, table order from element order, eager element checking, and non-forcing
multiline strings. Scalar observations cover generated and spread elements; original
general compound result failures remain recorded outside the adapter's selected scope.
Native tests cover pending construction, membership queries and comparison operands,
nested Host failure and retry, repeated cancellation, evaluator isolation, and terminal
release. Ordinary cases collect at each yield. The separate 768-deep Set test uses a 256
KiB evaluator stack, a 60-second parent deadline, one instruction per poll, collection
every 128 polls, and a limit of 4,000,000 polls per event. Earlier every-yield timeout
and failures at 200,000 polls per event are retained; these different conditions do not
establish the same resource contract or Java performance parity. Alias tests distinguish
retained ordinary values from executable metadata, verify repeated-call plateaus, and
require zero terminal heap entries. Existing small-stack Map and generic tests keep
their conditions.

Abstract `Collection<T>` comparisons distinguish List and Set from Listing,
Mapping, Map and scalar values. They cover bare and parameterized types, nested
lazy List wrappers versus eager Set checks, aliases, defaults, nullable attached
defaults, casts, union checking and Reference metadata. Empty explicit type
argument lists are compared at their original source locations, with other
collection and alias syntax as controls. Native checks collect at every yield
and pending read, use one instruction per poll with at most 200,000 polls per
event, and exercise retained receivers, Host reentry, failed-read retry, 24
nonterminal cancellations, evaluator isolation and terminal cleanup. Separate
deep metadata checks run on a 256 KiB stack; they do not exercise deeply authored
parser input or establish Java speed or peak-RSS parity. Historical small-stack
tests retain their original sources and execution conditions.

Authored frontend regressions exercise source nesting separately from deep values
constructed by the VM. Java comparisons retain the original deep Set equality and
Map-key inputs, operator associativity, short-circuiting, wide expressions, error
order and complete source diagnostics. Native process-isolated checks parse, clone,
resolve, compile and release authored trees on the same 256 KiB thread stack, with
a 60-second parent deadline. Partial parse/resolution/compilation failures must
release completed subtrees on that stack as well. Instrumented compilation and
constant-key comparison/rendering are separate reachable paths. Ordinary deep
chains, shallow scope boundaries around them, retained recursive-boundary guards
and accepted inputs near those guards require distinct checks. Deep type-boundary
checks cover parsing, tree copying and release; compilation of deeply authored
types and nested lexical string interpolation have separate observation records.
The recorded depth-64 lexical-string stack overflow remains a known failure;
removing it from a passing checkpoint never turns it into a successful guard test.
These tests do not establish arbitrary syntax depth, Java stack-overflow diagnostics, uniform
cancellation latency or workload-level speed and memory parity.

Lossless string regressions distinguish UTF-16 value identity from UTF-8 output.
The pinned Java API probe records code units, code points, hash codes and output
bytes for lone, paired and reversed surrogates. Differential cases cover
concatenation, interpolation, literal types/defaults, Map/Set keys, strict
collection stringification, Reference metadata and malformed-escape error order.
Native checks inspect actual units through Host suspension, reentry, failed-read
retry, collection, cancellation and terminal release. The Node and Chromium
scenarios compare complete Java-derived output payloads. These output adapters
observe Java's UTF-8 writer; they do not establish lossless public JavaScript
string transport. Diagnostic frame regressions additionally distinguish unmatched
surrogates from literal question marks in nested entry/function counters and
recursive compression. Native tests inspect internal names through Host reentry,
failed retries, collection and detached errors after evaluator closure; Java
captures provide the independent numbering and compression reference. Delayed
integer-literal errors retain their entry/element scope. Typed-property controls
distinguish object-local inline checks from module/class declarations, including
local declarations and aliases that retain their separate call frames. Surrogate
URI handling, the complete diagnostic corpus and full string builtins remain
separate requirements.

Numeric value-frame regressions compare complete Java diagnostics and successful
values for direct negation, parentheses, radix/minimum integers and signed zero.
Native checks retain authored observation spans while inspecting executable
value spans, declaration ownership, suspended constraints, reentry, retry,
per-instruction collection and detached errors after close. Node and Chromium
run the same fresh Java-derived cases, including successful value controls.
Exploratory nested-negation overflow differences remain recorded separately;
these checks do not complete numeric diagnostics.

Forbidden object property annotation regressions compare full Java diagnostics
at the actual fixture URI, with accepted module, class and local declarations as
controls. Native checks cover authored owners, first-error order, literal and
computed entry labels, generator prefix revisits, supplementary positions and
unmatched-surrogate string keys. Node and Chromium compare the complete
Java-derived payloads through fresh evaluators. These frontend checks establish
neither Host lifecycle behavior nor a new small-stack or performance result.
Const-modifier and inferred-parent regressions additionally compare enclosing
versus expression owners and method-versus-lambda error ordering. Successful
untyped and explicit Dynamic method calls, unused lazy methods and earlier
lexical errors are controls. Typed method return defaults and inherited property
inference now have selected Java comparisons, including class and collection
parents, nullable/defaulted unions, aliases, lexical owners, lazy failures and
complete default frames. Native tests collect between VM steps and exercise
Host failure, retry, cancellation and terminal release. Prototype type defaults
and instance initializers have separate expectations. The exhaustive portable
runner `tools/platform/compare-language-cases.mjs` records each complete payload
twice per Node/browser target and fails on any mismatch. The historical
`compare-method-parents.mjs` command delegates to that runner. Two original
`Dynamic.toString()` failures remain selected alongside separate type controls;
the development exception list is unchanged.

Declaration annotation regressions compare module, class, alias, property, and
method initialization order, lazy fields, eager generators/spread, const scope,
type errors, and complete diagnostic frames. Persistent sequences distinguish
published class/alias singletons and successful member prefixes after failure
from retryable ordinary properties. Builtin cases verify nullable and empty
listing defaults, required Since/SourceCode fields, hierarchy checks, and lazy
element errors with pinned `base.pkl` frames. SourceCode's union has no default;
missing `language` must not be replaced with Go.

Native annotation tests use actual Pkl resource reads to exercise suspension,
collection, nested contexts, nonterminal cancellation, stale completions,
evaluator isolation, and terminal cleanup. Separate Java/native Host observations
through `annotation_host_probe` compare source-driven callback reentry and full
ordered status/stdout/stderr. Earlier injected-instruction checks remain
architectural evidence with a separate scope. Neither contract establishes Host
I/O through the scalar Node/Wasm language adapters or complete module setup/cache
behavior.

The generic portable runner takes a fixture module exporting `cases` and
`context`; each case carries exact source bytes/hash, its property-path expression,
and one complete Java-derived value or error. It validates fixture source
identity, records artifact/runtime identities, and derives its counts from the
selected array. Use a fresh output directory and the pinned browser tooling:

```sh
node tools/platform/compare-language-cases.mjs target/m2/annotations-platform-new target/m2/platforms-new/wasm target/m2/platforms-new/pkl_napi.node tools/platform/annotation-cases.mjs
```

An acquisition, source-hash check, or documented command is not a passing
platform result. Full reflection and builtin metadata remain open, including the
unsynthesized `@SourceCode` metadata on `Deprecated.replaceWith`. `ModuleInfo`,
converters, deprecation logging, module-cache semantics, general object
`toString`, and `List.map` retain their implementation or verification gaps.
Preserve the original failing inputs and oracle internal errors; independent
controls do not replace their contracts or close the full language gate.

M1 experiments have narrower, explicit contracts:

```sh
python3 tools/m1_frontend.py --check-contract
python3 tools/m1_semantics.py --check
python3 tools/m1_frontend.py --output target/m1/frontend-new
python3 tools/m1_runtime.py --output target/m1/runtime-new
python3 tools/m1_host.py --output target/m1/host-new
python3 tools/m1_lifecycle.py --output target/m1/lifecycle-new
python3 tools/m1_platforms.py --output target/m1/platforms-new
```

See [frontend](docs/m1-frontend.md), [runtime](docs/m1-runtime.md),
[semantics](docs/m1-semantics.md), [Host](docs/m1-host.md), and
[platform](docs/m1-platforms.md) contracts, plus the
[closing lifecycle comparison](docs/m1-lifecycle.md). Primitive comparisons are not full
Pkl renderer/error comparisons. The latest M2 regression capture matches two of
the three original runtime diagnostics; the cycle difference remains. The
platform command returns nonzero for its two known Java regex
backend differences. CI uses the separately named architecture contract mode,
which still reports failed compatibility cases and requires the exact gap
inventory. A green experiment job cannot establish release conformance.

Run `python3 tools/m1_runtime.py --benchmark --output target/m1/performance-new`
on a quiet host with the separately pinned optimizing Java profile. Preserve
negative results, convergence data, compilation traces, and the different API
boundaries; do not interpret a prototype ratio as a product performance claim.

Run `python3 tools/m1_graal.py --output target/m1/graal-new` to reproduce the
[Graal compilation investigation](docs/m1-graal.md) on the pinned macOS arm64 JDK.
Use `python3 tools/m1_graal.py --check-report target/m1/graal-new` to verify the
stored raw evidence without executing that JDK. The diagnostic class override
is an explicitly separate causal control, never an oracle or performance update.
The unchanged-JAR positive workload requires actual guest Tier 2 compilation
and per-call output checks. These checks do not measure performance.

Run the pinned Java 0.32.0 oracle and Rust with identical inputs and environments. Compare:

- Raw stdout and stderr bytes, and exit codes.
- The set of generated files, their paths, and their raw bytes.
- Error messages, whitespace, source locations, underlines, call frames, hints, and error order.
- CLI options and defaults, REPL state, and API results, errors, cancellation, and close behavior.
- Server protocol encoding, callbacks, and session lifecycles.

Fix all inputs that affect results, including working paths, locale, time zone, terminal settings, environment variables, and network responses. Resolve path differences through the execution environment, such as identical mounts. Formal comparisons must not remove differences with `trim`, sorting, JSON reserialization, or arbitrary string substitutions.

Protocol fields that vary between runs, such as Java's random evaluator IDs, need a separate contract. Codec tests supplied with fixed IDs compare all bytes. Tests of actual sessions verify that issued IDs are referenced consistently and validate result payloads; they must not be described as byte equivalence for the entire session.

Generate expected results from the Java oracle and review their provenance and changes. Do not resolve mismatches by overwriting expected results with the current Rust output.

## Planned Test Layers

Stage63's collection-addition and Set-endpoint fixtures extend the shared
catalog to eight Java authorities. Native, N-API and Chromium comparisons
check all seven common raw channels and whole process protocols for 680
expressions in two sessions per target. UTF-16 comparisons preserve lone
surrogates. The Java-only internal identity supplement remains a distinct
boundary from public string results and typed exports.

The [Stage63 checkpoint](reports/m2/stage63-implementation-checkpoint.json)
separately records the unchanged original 91-program CLI/output-file contract,
whose nonpasses remain failures, and the known Any/M1 Regex differences.
Its initial platform acquisition failed a command-provenance admission check
because the M1 launcher used an absolute Python executable instead of the
recorded literal `python3` contract. Preserve that capture and failed preflight;
the fresh acquisition uses the existing verifier without changing expected
bytes or rewriting the original execution record. Replaying saved comparisons
does not count as another guest execution.

| Layer | Scope |
| --- | --- |
| Test tooling | Failure detection for empty corpora, missing expected results, unavailable runners, and byte differences |
| Parser and resolver | Syntax, invalid input, source spans, name resolution, and formatter round trips |
| Runtime and types | Lazy forcing, inheritance, closures, cycles, constraints, generics, and the first error |
| Standard library and renderers | Every public member, boundary values, output order, converters, and binary output |
| CLI, REPL, and server | Help and options, PTY sessions, state retention, protocol, timeouts, and close behavior |
| I/O and packages | HTTP fixtures, redirects, proxies, CAs, root restrictions, checksums, caches, and lockfiles |
| Pantry | Development trees, packaged modules, real examples, and generated files at the pinned revision |
| Platforms | Native, Wasm, and N-API results, errors, and Host capabilities |
| Generation and transformation | Grammar-based generation, differential fuzzing, property tests, and metamorphic tests |
| Long-running execution | Repeated successes and failures, growing sets of unique sources, cancellation, evaluator creation and shutdown, and memory reclamation |

Each test manifest records the case ID, source repository, revision and path, input and resource hashes, execution arguments and environment, required Host capabilities, expected success or error, channels and files to validate, and execution targets. Pinning a Pantry commit alone does not establish 0.32.0 compatibility, so first verify the selected cases and execution conditions with the Java oracle.

Distinguish passes, failures, tests not run, unimplemented cases, and environment errors. Do not count skips or known failures as passes. Add minimal reproductions of failures to the required regression cases, and compare the test inventory with actual execution results to detect omissions.

Reference regressions compare import identity, constructor kinds, declaration
metadata, unforced referent bodies, normalized types, synthetic paths, equality,
key behavior, and complete diagnostics, including frames from the pinned
`pkl:ref` source. Native lifecycle tests exercise domain rendering across Host
suspension, nested evaluation, GC, retry, cancellation, close, and timeout.
Node and Chromium execute the same engine for Reference values and errors.
Original upstream files evaluated through a selected API expression remain
exploratory evidence; those calls do not replace their original renderer or
snippet-suite contracts. Reviewed implementation/test links preserve these
unexecuted cases and the full completion gate.

Resource-read regressions preserve keyword and quoted-method distinctions,
operand forcing, nullable absence, URI validation and policy order, declaration
source resolution, and complete diagnostics. The original controlled filesystem
matrix retains unsupported I/O and unmatched-surrogate cases. Source-path
language probes acquire their own Java diagnostics at the identical recorded
URI; fixed portable URI observations do not substitute for those captures.

Portable fixtures with Java UTF-16 captures compare exact code-unit bytes and
Node UTF-8 derived from those captured units. Original Java UTF-8 bytes remain
separate evidence, with cross-encoder differences reported explicitly. The API
returns JS strings; this comparison does not claim that Java and Node serialize
an unmatched surrogate to identical UTF-8 bytes. Legacy fixtures without unit
captures keep their raw UTF-8 contract. An explicitly marked Java internal error
always remains `oracle-error`, even when its payload matches. Per-case power
assertion flags override the shared context and are recorded in each execution.
See the [resource-read contract](docs/m2-resource-reads.md).

### Stage 73–75 local verification boundary

The integrated evaluator's five required Rust commands ran against a frozen
3,270-path canonical source snapshot and all exited zero. The workspace test
count is 1,069 distinct passes, zero failures, and two ignored; 13 nested
child reruns also appear in raw harness output and are not additional tests.
Selected native, N-API, and Chromium comparisons each cover 310 queries with
two observations, 620 per target. Preserve the 91 earlier queries whose Java
capture lacks logger channels as public-result comparisons only. The separate
M1 experiment passed its architecture contract and retained two Regex
compatibility failures per target.

The full 4,416-case Rust replay records **4,163 matches, 110 mismatches, 141 unsupported results, 2 Java oracle errors, and 0 Rust internal errors**. Original modules match 643/888; expression strings match 3,519/3,527. The JVM classpath case matches. Compared with Stage69–72, 5 previously nonpassing cases now match, with no prior-pass regressions.

The linked replay, separate integrity verifier, and independent regression audit completed successfully. The replay reuses sealed Stage67 Java outputs and starts no fresh Java process. The strict full gate retains 1,791 overlapping unmet conditions. The 111 unready inventory cases and all nine M2 workstreams remain open. Earlier mapping rejection, interruption, ENOSPC, and monitor failures remain recorded separately.

See the [implementation checkpoint](reports/m2/stage73-75-implementation-checkpoint.json) for raw evidence and execution provenance. These local records do not establish full M2, product API, release-platform, or performance compatibility.

### Stage76–78 canonical local verification boundary

The integrated 17,774-path source snapshot passed the five required Rust
commands locally. Native, N-API and Chromium selected comparisons cover 771
queries with two observations per target; only channels present in Java authority
receive a compatibility verdict. Strict M1 raw comparison matched 6,078/6,078
per target. Its prior `--verify-contract` policy failed because it listed
the two now-matching Regex cases as known gaps. The revised contract completed
with exit zero and an empty known-gap set. Strict matching and architecture-
contract acceptance remain distinct gates.

The full selected replay recorded 4,188 matches, 86 mismatches, 139 unsupported,
2 Java oracle errors, and 1 Rust internal error among 4,416 ready cases. The
separate verifier and independent regression audit exited zero. The audit found
25 improvements, zero prior-pass regressions, and 1,791 overlapping full-gate
conditions. The `classes/constraints14.pkl` internal error, all other
nonpasses, and 111 unready cases remain in the release gate. No complete API,
cross-platform release, or performance result follows from these local checks.

### Stage84 local verification boundary

The complete workspace command passed 1,126 tests with zero failures and two
ignored tests; 13 nested child observations and one explicit formerly ignored
Regex regression are separate. Its opt1/debug0/strip profile enables assertions
and overflow checks. All five required Rust commands passed on the frozen local
source set. Native, N-API, and Chromium each match 88 observations of the new
44-query List cohort, including ordered Logger fields. Both fresh M1 experiments
match all 6,078 Java results per target. The
[checkpoint](reports/m2/stage84-list-search-transform-checkpoint.json) retains
the exact profiles, reused artifact provenance, raw failures, and comparison
boundaries. The still-running full replay and complete M2/API/performance gates
are separate from these results.

## Performance and Memory

The [initial Java benchmark contract](docs/benchmarks.md) implements cold CLI, persistent evaluator, and fresh evaluator measurements for six fixtures. Use a quiet host with the pinned optimizing runtime:

```sh
python3 tools/oracle.py fetch --profile performance
python3 tools/benchmark.py --check-contract
python3 tools/benchmark.py
python3 tools/benchmark.py --check-report target/m0/benchmarks
```

Every run uses a new evidence directory. Raw bytes, per-process RSS, first requests, warmup windows, and measurement windows are retained. Nonconverged forks remain inconclusive. These initial Java measurements do not complete the required workload inventory or provide a Rust performance verdict.

Only runs that produce correct output are eligible for measurement. Every required workload must satisfy both conditions against its pinned Pkl `0.32.0` baseline:

```text
Rust execution time / Pkl 0.32.0 reference execution time ≤ 1.00
Rust peak RSS / Pkl 0.32.0 reference peak RSS ≤ 1.00
```

Fix the pinned Official Apple Pkl `0.32.0` `native-image` CLI executables (`oracle.lock.json::native_image`), Java executable artifacts, JDK and Graal/Truffle configuration, hardware, OS, and measurement method. All 15 CLI, server, and `pkldoc` workloads (`cold-cli` and `steady-server`) are strictly gated `<= 1.00x` time and `os.wait4` peak RSS against pinned Official Apple Pkl `0.32.0` `native-image` (with live Temurin 21 JVM and archived M0 GraalVM CE `25.0.1` preserved separately). All 19 embedded API (`persistent`, `fresh-evaluator`) and JS/Wasm (`napi`, `wasm-node`, `wasm-browser`) workloads remain strictly gated against a sufficiently warmed-up pinned JVM (plus the `2026-11-22` browser Wasm contract for browser targets), because official `native-image` ships CLI executables only. Do not judge embedded or steady-state performance solely by cold JVM startup time.

| Usage | Measurement Boundary |
| --- | --- |
| CLI | From process startup through completed output and process exit |
| Persistent evaluator | Initialization, first request, and repeated requests measured separately |
| REPL and server | Session creation, request latency, long-term memory usage, and shutdown |
| N-API | Includes JS↔Rust value conversion and result delivery |
| Wasm | Download, compilation, and initialization measured separately from requests to a persistent instance |
| Packages and large inputs | Cold and warm caches, import graphs, large outputs, and failure paths |

Record latency distributions and peak RSS across repeated runs for each workload. Report results as inconclusive when measurement uncertainty prevents a judgment. Do not use average improvements to conceal regressions in individual required workloads.

For Wasm and N-API, first define measurement boundaries that include the Host process. JVM heap usage, Rust allocations, and Wasm linear memory are diagnostic metrics; do not compare these different metrics directly to claim equivalent memory usage.

## Release Criteria

The required feature and API inventory must have no omissions, and there must be zero known mismatches within the required scope on target platforms. Completion cannot be declared while required tests remain unrun or unimplemented, or performance and memory measurements remain inconclusive. Do not document complete compatibility or guarantee performance equivalent to Java before these criteria are met.

**Explicit scope exclusions (2026-11-20, matching [INTENT.md](INTENT.md) and [PLAN.md](PLAN.md)):** Java/Kotlin code generation (`pkl-codegen-java`, `pkl-codegen-kotlin`) and a JVM public API compatibility adapter are excluded from required product scope and release criteria. Upstream `java_api`, `kotlin_api`, and code-generation census entries retained in `compatibility/inventory.json` and `compatibility/corpus.json` remain reference provenance only and do not block release completion. The pinned Java 0.32.0 oracle remains the compatibility and performance reference for all required surfaces.
