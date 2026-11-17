# M2 language verification

The [Stage 160 checkpoint](../reports/m2/stage160-gc-protobuf-remediation/README.md) records the Stage 160 GC safepoint optimization and Protobuf `no-complete-reference-nonpass` boundary fixes on the frozen worktree (local macOS arm64, `rustc 1.98.1`, `PKL_ALLOW_LOCAL_JDK=1`, Temurin `21.0.13+9-LTS`): a single unsplit `cargo test --workspace --locked` completes with `Exit 0` (`2,220 passed; 0 failed; 2 ignored` across `337` outer targets/doctests, `13` nested child passes excluded, including `m2_reflection_apis_lifecycle` 5/5 in `83.35s`); `stage160-protobuf-strict` exits `101` (`240` regular, `8` pre-renderer, exact `18` `REFERENCE-NONPASS` entries with all 4 `no-complete-reference-nonpass` slots emitting `unsupported-or-api-error\n`); unsandboxed `target/m1/stage160-platforms-01 --verify-contract` matches `6,078 / 6,078` across `rust`, `node`, and `browser`; and focused M2 subset `target/m2/stage160-selected-01` records `100 / 100 pass` with `--verify-report` and `--verify-subset` exiting `0`. Stage 160 does not re-run the full `4,575`-case replay, whose latest full acquisition remains Stage 159's [Stage 159 checkpoint](../reports/m2/stage159-strict-gate-closure/README.md) (`target/m2/stage159-genuine`: `4,574 pass / 1 oracle-error (error-cycle) / 0 mismatch` across `4,575` ready cases; `--verify-report` and `--verify-development` exit `0`; `--verify-full` exits `1`). Remaining limits include `18` pinned Protobuf reference nonpasses, `error-cycle` `oracle-error`, `11` unmapped candidates, local JDK provenance (`exact_jdk_lock_match: false`), Linux/Windows CI coverage, and open Milestones M3–M7 (M3: standard library and output; M4: modules, resources, projects, and packages; M5: product APIs and tools; M6: Wasm and N-API product completion; M7: full release verification and JVM throughput/memory parity).

The checkpoint descriptions below retain their historical source and execution
scope; they are not current-source reruns.

## Stage 160 GC safepoint optimization and Protobuf nonpass boundary fixes

The [Stage 160 checkpoint](../reports/m2/stage160-gc-protobuf-remediation/README.md) resolves the `m2_reflection_apis_lifecycle` GC safepoint sweep bottleneck and both runtime bugs across the four `no-complete-reference-nonpass` Protobuf slots (`imported-alias-owner/{on,off}`, `constraint-read-true/{on,off}`) on local macOS arm64 (`rustc 1.98.1`, `PKL_ALLOW_LOCAL_JDK=1`, Temurin `21.0.13+9-LTS`):
- **GC & Build-Profile Optimization**: Added `[profile.dev.package.pkl-runtime] opt-level = 2` in `Cargo.toml` so integration test targets compile `libpkl_runtime` at `opt-level = 2`; eliminated per-step `Vec<Value>` allocations in `reflection::Payload::roots` and `Run::roots` (`vm/reflection.rs`); reused `mark_scratch`/`pending_scratch`, added per-slot cached outgoing `edges`/`edges_valid` when `bytes_lists.is_empty()`, and added dirty/length guards for `key_equalities` and `structural_keys` in `heap.rs`; and added `ArenaRangeIndex` small-arena bypasses plus `declaration_roots_dirty` / `declaration_root_counts()` caching across `vm/query_lifecycle.rs`, `vm.rs`, and declaration cache mutation sites (`HeapStats` unchanged).
- **Protobuf Nonpass Boundary Fixes & Manifest Digests**: Updated `protobuf_type_name` (`vm/protobuf_rendering.rs`) to return `ErrorKind::Unsupported` instead of `ProtobufListingOrMappingRootNullPointer`, updated `protobuf_normalize_type` (`vm/protobuf_types.rs` with `pub(super) alias_check_environment` in `vm/type_resolution.rs`) to normalize parameterized `NamedAlias` using the unparameterized alias body (`T::Unknown`), added two regression tests in `crates/pkl-runtime/tests/m2_protobuf_renderer.rs`, and synchronized source SHA-256 digests in `compatibility/m2-language.json` (`356` diff lines, `0` non-`sha256` changes; `4,586` cases and `475` requirements unchanged).
- **Frozen-Source Verification Outcomes**: `cargo fmt --all -- --check`, `cargo xtask verify`, `cargo clippy --workspace --all-targets --locked -- -D warnings` (plus `--no-deps`), `cargo check -p pkl-runtime --target wasm32-unknown-unknown --locked`, six Python `--check` scripts, and `253` M2 Python unit tests (`215.895s`) all exit `0`. Single-command `cargo test --workspace --locked` exits **`0`** (`2,220 passed; 0 failed; 2 ignored` across `337` outer targets/doctests, `13` nested child passes excluded; `m2_reflection_apis_lifecycle` 5/5 in `83.35s`; `pkl-runtime --lib` 263/263 in `26.61s`). `target/m2/stage160-protobuf-strict` exits `101` (`240/240` regular, `8/8` pre-renderer, exact `18` `REFERENCE-NONPASS` entries, `0` harness failures, all 4 `no-complete-reference-nonpass` slots writing `unsupported-or-api-error\n`). Preserving initial sandboxed attempts (`target/m1/stage160-platforms` Exit `2`, `target/m2/stage160-selected` Exit `1`), unsandboxed `target/m1/stage160-platforms-01 --verify-contract` matches `6,078 / 6,078` across `rust`, `node` (`v26.13.1`), and `browser` (Chromium `156.0.8078.4`, Playwright `1.64.1`), and `target/m2/stage160-selected-01` records `100 / 100 pass` with `--verify-report` and `--verify-subset` exiting `0`.

## Stage 159 M2 integrity remediation and genuine live verification

The [Stage 159 checkpoint](../reports/m2/stage159-strict-gate-closure/README.md) records the Iteration 4 remediations and frozen-tree verification results on local macOS arm64 (`PKL_ALLOW_LOCAL_JDK=1`, Temurin `21.0.13+9-LTS`):
- **Integrity & Tooling Fixes**: Retracted `tools/assemble_stage159.py`, `target/m2/stage159-final`, and `target/m2/stage159-final-jvm-classpath-workspace`; replaced `id()`-keyed `_BASELINE_DIGEST_CACHE` / `_cached_baseline_digest` in `tools/m2_module_eval.py` with explicit `baseline_digest=None` passed from `tools/m2_language.py:check_report()` plus regression test `test_verify_case_with_distinct_reallocated_baselines_never_aliases_digest`; removed `case_satisfies_gate` from `tools/m2_manifest.py` so `--verify-full` exits `1` whenever `full_gate_failures` is non-empty.
- **Generalized Exception Provenance & Dynamic Release Banner**: Removed hardcoded fixture error strings and fixed release banners from `crates/pkl-runtime/src/java_exception/provenance.rs`, adding generalized `ForeignOrigin` projection (`project_origin_diagnostic` / `project_pkl_bug_with_profile`), dynamic `GlobCharacterClassBounds` and `KeyValueHolderEqualitySpecialization`, native stdlib `ForeignActivation` attachments, and `PlatformMetadata` propagated from `PKL_ORACLE_JAVA_VERSION` via `SourceProbe::set_platform_metadata`.
- **Protobuf & Semantic Mapping Restoration**: Reverted `tests/fixtures/m2-protobuf-java04-full/observations.tsv` to `HEAD`, restored `(240, 8, 18, 266)` and `#[ignore]` in `crates/pkl-runtime/tests/m2_protobuf_java04_full.rs`, and re-audited all `475` requirements in `compatibility/m2-mappings.json` (`198` specification, `137` language_parser, `109` language_token, `31` semantic-family) against `external/pkl/docs/modules/language-reference/pages/index.adoc`, genuine `crates/pkl-runtime/src/` symbols, and all `4,575` ready `.pkl` cases.
- **Frozen-Tree Verification Outcomes**: `cargo xtask verify`, `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo check -p pkl-runtime --target wasm32-unknown-unknown --locked`, 5 sentinel suites (`94 passed / 0 failed / 1 ignored`), `m2_foreign_exception_provenance` (`5/5`), `m2_protobuf_renderer` (`14/14`), `m2_settings_authored` (`4/4`), `253` M2 Python unit tests, and `tools/m2_manifest.py --check` all exit `0`. Split workspace Rust runs pass `2,226 passed / 0 failed / 2 ignored` across `346` completed runs while `m2_reflection_apis_lifecycle::cancellation_closes_evaluator_and_fresh_evaluator_rejects_old_transform_token` times out under dual-GC sweeps (`60m` debug / `29m` `--release`). `target/m2/stage159-genuine` records `4,574 pass / 1 oracle-error (error-cycle) / 0 mismatch` (`--verify-report` and `--verify-development` exit `0`, `--verify-full` exits `1`), and `target/m1/stage159-genuine` matches `6,078 / 6,078` across `rust`, `node`, and `browser`.

## Stage 158 milestone remediation and frozen-source verification

The [Stage 158 Remediation checkpoint](../reports/m2/stage158-m2-remediation/README.md) records the completed remediation and final `-03` frozen-source verification on local macOS arm64 (`PKL_ALLOW_LOCAL_JDK=1`, Temurin `21.0.13+9-LTS`):
- **Shared Runtime Fixes**: Resolved the remaining 5 mismatches observed in `stage158-final-02` (`4,568/2/5`) via `crates/pkl-runtime/src/vm/glob.rs` (primary-unit `@`-dependency resolution through `ModuleRequestKind::ResolveImportUri` across `Phase::ResolvePattern | Phase::ResolveBase`, fixing 4 `package-project-eval` glob cases) and `crates/pkl-runtime/src/vm/typecheck.rs` (`TypeCheck::Constrained` saving/clearing `run.eager` across `*base` and restoring in `Task::AfterBase`, fixing `m2:rev1:t36-foreign-nominal-identity`).
- **Package Purity & Platform Independence**: `crates/pkl-runtime/src` remains free of host filesystem/process/env I/O and mutable statics, compiling cleanly for `--target wasm32-unknown-unknown`. Multi-target M1 verification (`target/m1/stage158-platforms-final-03`) matches Java on `6,078 / 6,078` cases across Native, Node (`v26.13.0`), and Chromium (Playwright `1.64.1`), with `92` Node and `92` browser scenario checks passing.
- **Frozen-Source Suite Results (`-03`)**: `cargo xtask verify`, `cargo fmt --check`, `cargo clippy`, `cargo check wasm32`, 6 `--check` scripts, and 95 M2/oracle Python unit tests all exit `0`. `cargo test --workspace --locked` passes `2,225` tests with `0` failures and `6` explicit `#[ignore]` functions (`1` M1 hook + `5` strict sentinel suites). `stage158-protobuf-strict-final-03` exits `101` as contracted (`266/266` executed, `240/240` regular, `8/8` pre-renderer, `18` `reference_nonpasses`, `experiment_contract_pass: true`, `strict_compatible: false`).
- **Full M2 Raw-Byte Capture & Strict Gate Integrity**: `target/m2/stage158-final-03` records **`4,573 pass / 2 oracle-error / 0 mismatch`** across all `4,575` ready cases plus `88/88` metamorphic passes (`source_stability: true`, `artifact_stability: true`; `--verify-report` exits `0`). The separate explicit `-Xmx2g` experiment (`target/m2/stage158-apilist-xmx2g-03`) confirms byte-for-byte parity on `api/list.pkl` (`8,411` bytes, `diff_bytes: 0`). Strict full-gate auditing (`--verify-full`, exit `1`) retains all `933` `full_gate_failures` (`460` implementation incomplete + `460` test mapping incomplete + `11` unmapped candidates + `2` `-Xmx512m` `oracle-error` cases), local JDK provenance (`exact_jdk_lock_match: false`), and open M3–M7 milestones without false passes.


## Stage153–155 selected original and URI boundaries

Two retained original T40 CLI contracts now match their full raw comparison; two separate cache-off controls add no original cases. File URI records 19 boundary matches/5 nonpasses; package API records 6 complete value matches/31 nonpasses and qualifies none of the 11 original package CLI/server/project cases. Source06 full workspace failed at 2,190/1/6; source07 separately passes the corrected 15-test target, 83 Python tests and five prechecks. Same-workspace raw266 retains 240 renderer+8 pre-renderer matches and 18 strict nonpasses. Fresh bounded M1 matches 6,078 results per target and 92 Node/browser scenarios each.

The inventory is 4,586 cases: 4,485 ready contracts, 69 unimplemented and 32 unmapped. Requirement states remain 15 implemented, 42 partial and 418 unimplemented. Admission/mapping states are not full-language completion, and historical full-ready observations are not replayed or promoted. Full M2/API, package/Host, release-platform and performance gates remain open. See the [Stage153–155 report](../reports/m2/stage153-155-t40-uri/README.md).

## Stage146–151 Host and static analysis

The shared runtime adds lossless scalar error text, selected native file Glob/URI handling, authored settings/path operations and a bounded `pkl:analyze` import graph. Current original31 evidence records 20 raw matches, six mismatches, three unsupported cases and two oracle errors; the NFC boundary separately records 13 UTF-8/UTF-16 path-value matches and five nonpasses. Six prechecks and workspace02 pass, with 2,172 tests passed, zero failed and six ignored across 335 outer groups; 13 nested child passes are excluded. Same-workspace Protobuf raw266 retains 240 renderer matches, eight pre-renderer matches and 18 strict nonpasses using existing Java04 fixtures. One fresh bounded M1 capture matched 6,078 results per Native/Node/Chromium target and 92 scenarios per Node/browser target; architecture and independent strict predicates pass on the same bytes, without a second strict CLI run. Requirement states, case IDs and execution readiness are unchanged: 4,586 cases comprise 4,483 ready contracts, 71 unimplemented contracts and 32 unmapped candidates, with 475 requirements. Ready contracts are not recorded passes. No new full-ready replay or full M2/API, release-platform or performance completion is claimed. See the [checkpoint report](../reports/m2/stage146-151-host-analyze/README.md). Future Stage152–158 drafts remain outside this qualified source epoch.

## Stage144 binary, YAML and shell

The shared engine adds Pkl binary byte rendering, bounded YAML parsing and authored shell escaping. Before lint corrections, 38 native tests passed against 320 retained Java observations and 21 original CLI contracts separately matched all public channels. Two additional YAML oracle-internal observations remain noncomparisons. After seven lint corrections, all five prechecks passed and the full workspace aggregate records 1,947 passed, zero failed and five ignored across 323 groups; 13 nested child passes are excluded. Fresh bounded M1 matched 6,078 results per Native, Node and Chromium target and 92 Node/browser scenarios each; the architecture command and independent strict comparison pass on the same capture. Requirement/case states and counts are unchanged, and no new full-ready replay is claimed. Full M2/API, release-platform and performance gates remain open. See the [Stage144 report](../reports/m2/stage144-binary-yaml-shell/README.md) for failures and aggregate/provenance limits.

## Stage141 Jsonnet and XML renderers

The Stage141 Jsonnet and XML checkpoint adds shared renderer support and qualifies 21 focused native tests against 172 Java observations, including four retained setup-error controls. Twenty-five original CLI cases matched their complete public channels. The full workspace aggregate records 1,909 passed, zero failed and five ignored across 313 harnesses; 13 nested child passes are excluded. Bounded M1 matched 6,078 results per Native, Node and Chromium target, with 92 Node and 92 browser scenarios. Full M2/API, release-platform and performance requirements remain open. See the [Stage141 report](../reports/m2/stage141-jsonnet-xml/README.md) for retained failures and scope.

## Stage139 JSON Parser

The shared engine supports the selected authored `pkl:json` Parser, converter and Resource cases. Focused native checks passed 47 tests and matched 108 Java observations; five registered original CLI contracts matched their full public channels. Five fresh prechecks passed. The full workspace passed 1,886 tests with zero failed and five ignored across 305 harnesses, excluding 13 nested child passes; the aggregate does not prove individual test identities or absence of interleaving. A separate bounded M1 acquisition matched 6,078 results per Native, Node and Chromium target, with 92 Node and 92 browser scenarios. This is not full M2/API, release-platform or performance completion. See the [Stage139 report](../reports/m2/stage139-json-parser/README.md).


## Stage137 module diagnostics and authored semver

The shared runtime adds standard file, HTTP and generic URL key profiles and loads the pinned authored `pkl:semver` module. The public `ModuleKeyKind` enum has three new variants; downstream exhaustive matches require updates. The focused native run passed 69 tests and matched 48 raw Java observations; six registered original CLI cases matched their full public channels. Nineteen mocked JavaScript transport tests passed. A fresh bounded M1 run matched 6,078 results per Native, Node and Chromium target, with 92 Node and 92 browser scenarios. The full workspace passed 1,872 tests with five ignored across 302 harnesses; the independent aggregate readback excludes 13 nested child passes and does not prove per-test identities or absence of interleaving. These results do not establish full M2/API, release-platform or performance completion. See the [Stage137 report](../reports/m2/stage137-module-diagnostics-semver/README.md) for raw evidence and remaining limits.


## Stage136 lexical mappings and ready replay

The local full-ready acquisition records 4,483 cases (candidate-unsupported: 74, mismatch: 12, oracle-error: 2, pass: 4,395); integrity verification exited 0 and strict full verification exited 1. Twelve lexical mappings are supported by 49 scalar matches and a separate registered RECORD module match. The 62-test Python run includes 14 target-directory tests. See [the Stage136 report](../reports/m2/stage136-language-replay/README.md) for recorded nonpasses and scope.


## Stage134 type sources and object entry diagnostics

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

Requirement and case states remain unchanged; 815 cases have no requirement links. See [the Stage134 report](../reports/m2/stage134-type-source-scopes/README.md).

## Stage131 power-assertion boundaries

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

The original remains required but has no requirement links. This repair does not change the strict full-completion gate. See [the Stage131 report](../reports/m2/stage131-power-boundaries/README.md).

## Stage130 shared exception rendering

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
and provenance limits are detailed in [the Stage130 report](../reports/m2/stage130-java-exception-renderer/README.md). Runtime stack capture and
internal-error catch integration remain open. This is not full M2/API,
release-platform or performance completion, and no full language replay is
claimed.

## Stage129 complete local workspace command

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
are preserved. See the [Stage129 report](../reports/m2/stage129-workspace-validation/README.md) for exact inputs, outcomes and
limitations. M2/API, release-platform and performance gates remain open.

## Stage128 workspace storage — bounded partial verification

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
headroom failure remains separate. See the [Stage128 report](../reports/m2/stage128-workspace-storage/README.md) for storage
conditions, artifact provenance and retained failures. That acquisition did not
complete workspace validation. M2/API, release-platform and performance
requirements remain open.

## Stage127 opaque glob URI validation — bounded local checkpoint

The shared traversal now validates the raw opaque reader URI after glob regex
compilation and before listing. The selected 20 previously wrong-success sessions
now stop with `Unsupported`; their Java internal-error diagnostics remain
unmatched. Of 36 control sessions, 32 match the common raw channels and four
retain their prior relative-space mismatches. See the [Stage127 report](../reports/m2/stage127-opaque-glob-uri/README.md)
for the 56-session/128-query evidence and unimplemented frame/catch behavior.

Eight Rust tests, formatting, xtask, all-target Clippy, the Wasm check, manifest
freshness and eight focused Python tests pass. A fresh M1 acquisition matches
6,078 rows per native/Node/Chromium target and passes 92 Node and 92 browser
scenarios. The actual full workspace command was stopped at its 1 GiB disk floor
during compilation (exit -15); that acquisition remains incomplete. The historical full
language/glob results below are not new Stage127 executions.

## Stage122 lossless source and runtime integration — previous local results

The shared engine adds authored UTF-16 source/URI handling, typed glob Host requests/completions, version/declaration/spread diagnostics, Resource digest defaults and 18 math external bindings. Bounded lossless session and string transport does not establish full typed value export or the public evaluator API.

The exact 285-harness union records **1,779 passes, zero failures and five ignored**; all seven fresh checks06 commands exit zero. The original reflection harness completed with actual exit zero, while the original Cargo exit remains unobserved/inconclusive. The incomplete producer, failed observer, monitoring gap and original failed/interrupted targets remain preserved; no fresh full-workspace Cargo pass is claimed.

The complete ready replay records **4,351 matches, 15 mismatches, 74 unsupported and two Java errors across 4,442 cases**, with **88/88 metamorphic checks** and no previous-pass regressions. Twelve original cases newly match, including Resource, math, selected invalid URI/glob diagnostics and fixed/version/spread errors. List's bounded native timeout remains a nonpass. The exact function-based strict gate stays at **1,768 overlapping conditions (472/472/824)**: all twelve newly matching ready cases still have empty requirement links, and implementation/test-mapping states have not changed. The 4,545 contracts, 475 requirements, 71 unimplemented and 32 unmapped execution contracts remain required.

Actual M1 records 6,078 raw matches per target and 92 scenarios each for Node/browser. Native transport has 20 runtime and four compiler-qualified counterparts. N-API retains 740 ordinary / 744 callback matches over 186 sessions and freshly matches 524/576 glob sessions, preserving the same 52 nonmatching cases and repairing four adapter regressions. The independently verified Chromium union of 701 reused and 63 new processes records the same 740 ordinary / 744 callback / 524 glob matches and 52 known glob nonmatches. The full three-target transport-19 raw comparison passes. Ten large Chromium cases use a separate 3 GiB sampled owned-process RSS cap; other processes retain 2 GiB and the original cap failure remains recorded. See the [Stage122 report](../reports/m2/stage122-lossless-source-runtime/README.md) for source epochs, exact raw receipts, unresolved API/internal fields and verified archive publication. None of these bounded results closes the nine M2 work areas, full API, release OS or performance gates.

## Stage121 Pair and superclass diagnostics — previous local results

The shared runtime corrects Pair's Any inheritance boundary, inherited native
Any arity frames and deferred invalid-superclass labels/spans. Fresh local
validation records 1,717 workspace passes, zero failures and five ignored tests;
all six development commands pass. The complete ready corpus now records
**4,339 matches, 19 mismatches, 82 unsupported cases and two Java oracle errors**,
with 88/88 metamorphic checks and no previous-pass regressions. The original
`extendTypeAlias.pkl` and `propertyNotFound2.pkl` diagnostics now match Java.

The selected new matrix matches 624 observations per Native, N-API and Chromium
target. Each retained target matches 902 ordinary observations and keeps two
required internal observations unresolved. The separate M1 acquisition matches
6,078 checks per target. JVM-only fields, ignored tests, the 71 unimplemented
and 32 unmapped contracts remain open. These counts are not a whole-M2 completion
percentage. The strict gate remains at 1,768 overlapping unmet conditions
(472 implementation, 472 mapping and 824 required-case conditions); these two
runtime repairs do not reduce that accounting total. See the [Stage121 report](../reports/m2/stage121-pair-supertype/README.md)
for raw-result boundaries, preserved acquisition failures and publication status.

## Stage117 original-case mappings — integrated after Stage120

The [mapping checkpoint](../reports/m2/stage117-original-case-mapping/README.md)
adds 94 reviewed links across 14 unchanged original snippets and 23 mapping rows.
Nine semantic families gain explicit source mappings. All 475 requirement states,
4,545 execution contracts and known failures remain unchanged. A new readback of
retained Stage120 bytes confirms 13 successful evaluations and one matching error;
it is not a fresh evaluator execution or a broader assertion about authored facts.

The 42 relevant Python tests and manifest freshness pass. Applying the unchanged
full-gate function to the updated manifest and retained complete ready results
leaves 1,768 overlapping unmet conditions: 472 implementation, 472 mapping and
824 required-case conditions. Twelve previously unlinked cases now have reviewed
requirement links. The inventory remains 4,442 ready, 71 unimplemented and 32
unmapped cases. This accounting change resolves no runtime mismatch and closes
none of the nine M2 work areas.

## Stage120 shebang, quoted identifiers and diagnostic ownership — bounded local checkpoint verified

The shared lexer/parser handles the selected hash-prefix and quoted-identifier
boundaries against pinned Java 0.32.0 evidence. Diagnostics keep valid UTF-8 spans
and explicit UTF-16 adjustments, preserve module/class property error ordering,
adjust the origin before recursive frame compression, retain imported caller and
query coordinates, and use the rendered module's URI for undefined-property
context. Native, Wasm and N-API continue to use the common runtime; Java is an oracle.

The integration contains **13 source/test/tool paths and 2,489 new Java fixture
paths across five families**. Eleven new Rust test functions execute in the
original workspace acquisition: **1,711 pass, zero fail and five remain ignored**.
That acquisition predates two test-only Clippy repairs. The two affected tests
pass again on the repaired source; the full workspace was not rerun and the
counts are not combined. Exact old test bodies are retained in separate
acquisition-specific isolation overlays.
The complete ready replay records **4,337 matches, 21 mismatches, 82 unsupported
cases and two Java oracle errors across 4,442 cases**, with no previous-pass
regression and all **88 metamorphic checks** passing. `errors/singleBacktick.pkl`
improves from mismatch to match. The original shebang controls remain matches.

New Native records **234 attempted observations from 117 Java authorities**, with
both the complete selected API predicate and frontend-comparable predicate true;
`all_required_pass=false` is preserved. The composed capture02 records six selected groups: each new Native, N-API and Chromium target matches 234 attempted observations from 117 Java authorities; each retained target matches 902 ordinary observations and keeps two required internal observations unresolved. Comparable predicates pass; `all_required_pass=false` remains explicit.
The exact receipt/source/product join imports the complete Native capture and comparison from capture01 (two prior process records), then records five fresh groups and ten new process records on the same five build02 products. No Rust rebuild or Native rerun is claimed.
Capture01's N-API Host failure and unexecuted groups remain original failure
evidence. The prior 902 ordinary/two unresolved observation boundary, lookup-
unreached errors and const non-witness successes retain their original scope.
The separate M1 acquisition matches **6,078 checks per target**, with strict and
architecture predicates passing.

Defaults02 preserves successful xtask and failed Clippy child101; its Wasm check was not run. After two cfg(test)-only decoder repairs, defaults03 freshly passes inventory and manifest freshness, the two exact affected tests, formatting, all-target Clippy with warnings denied and the Wasm runtime check: seven commands, stable source bookends. The prior successful xtask result is reused. The two fresh tests are separate from the original 1,711-pass workspace acquisition; the full workspace was not rerun.
Two published evidence parts total **77,196,066 compressed bytes** (62,914,759 and 14,281,307 bytes), each below 85 MiB. Independent manual tar/PAX and zlib readback verifies 21,670 selected payloads, one auxiliary payload and two manifests (21,673 logical regular members), exact raw bytes/size/mode/order/SHA256, gzip CRC/termination and tar EOF, plus 157,942 direct restoration aliases with no mode transformations. Exact public copy readback covers both parts and five verification support files.
The inventory remains **4,545 cases: 4,442 ready, 71 unimplemented and 32 unmapped**,
with 475 requirements and **1,780 overlapping unmet full-gate conditions**: 472
implementation, 472 mapping and 836 required-case conditions. This is function-
based accounting, not a fresh `--verify-full` CLI acquisition or a completion
percentage. Broader B3, full standalone span identity, unpaired Java source strings,
glob, isolated Stage121/122 work and complete language/API/performance requirements
remain separate. See the [checkpoint](../reports/m2/stage120-shebang-quoted-identifier-checkpoint.json)
and [verification contract](../TESTING.md).

## Stage114 diagnostic and Regex compatibility

The shared implementation now preserves single-line Regex formatting in PCF,
the distinction between receiverless `Undefined()` and undefined properties,
quoted diagnostic scope names, the raw class declaration name, and the enclosing
owner of an over-arity lambda error. Scalar custom receivers dispatch `getClass`
through the common runtime. The change adds 42 tests and 14,609 pinned Java
fixture files across the selected families.

The current full ready comparison records **4,336 matches, 22 mismatches, 82
unsupported cases and two oracle errors across 4,442 cases**. The same 888 original
modules improve from 786 to 790 matches; `baseModule`, `constraintsLambdaThis`,
`stackTraceWithQuotedMemberName` and `tooManyLambdaParams` now match. No historical
pass regresses, and all 88 metamorphic checks pass. Each selected native, Node and
browser comparison matches 902 ordinary observations with two required internal
observations unresolved; its success does not establish full language coverage.

The inventory remains **4,545 cases: 4,442 ready, 71 unimplemented and 32 unmapped**,
with 475 requirements. The exact full-gate function reports 1,780 overlapping
unmet conditions: 472 implementation, 472 mapping and 836 required-case conditions.
Stage116 glob, Stage118 hash-prefix, Stage119 quoted-identifier and Stage117
mapping work remains outside this canonical checkpoint. See the
[checkpoint](../reports/m2/stage114-diagnostics-regex-checkpoint.json) and [verification contract](../TESTING.md)
for the actual validation and preserved limitations.

## Stage109 Typed lookup, module paths and annotation diagnostics — local validation

Stage109 integrates 18 source/tool paths and 6,996 pinned Java fixture paths, with two generated metadata paths recorded separately. The combined targeted run records **152 passes, zero failures and one ignored required placeholder**. Its three new-family independent native readback matches **276 ordinary observations: 102 successful values and 174 ordinary errors**, including **1,736 harness fields and eight ordered Logger events**. One required Java internal contract retains two unresolved observations. The ignored test and unresolved Java results are not compatibility passes. The same current normal runtime separately matches **1,138 existing Typed/reflection observations**, **6,988 harness fields and 16 Logger events**.

The selected **Native02, NAPI03 and Chromium03** captures each cover **61 programs, 122 repeated sessions and 278 attempted observations**. Each matches all 276 ordinary observations, **2,288 portable fields and eight ordered Logger events**; two observations remain unresolved and `all_required_pass=false`. The portable count includes expression/status/output/error/logger-count channels and differs from the native harness's 1,736-field boundary. Java exception classes, causes and typed-export metadata remain provenance outside the shared string/error/logger comparison boundary.

The selected Host uses exact Java-backed synthetic keys, selected original/resolved/source identities, file/http/no-loader primary policies and genuine configured missing-resource completions. The original Native01 acquisition failed because its finite Host omitted normalized synthetic factory keys. NAPI02 then rejected six HTTP/`repl:` sessions at an obsolete file-only entry guard before evaluator construction. Both failures remain unchanged. Native10 and focused Node2 validate the normalized-key repair; focused Node4 validates the entry-guard repair. NAPI03/Chromium03 reuse admitted producer02/binding02 artifacts with independent before/after pins for the new JavaScript harness. New JavaScript inputs are not retroactively claimed as compiled producer inputs. Native02 was not rerun for the JavaScript-only repair.

All selected captures retain poll64, a 2,000,000 requested-instruction session fuel ceiling, collection after each batch, unchanged evaluator/heap conditions and original per-program deadlines. Their concurrent correctness results establish no execution-time or memory comparison. The admitted normal runtime `.rlib`, `.rmeta` and dep-info were retained before later Clippy/workspace could replace canonical artifact paths; the retention does not alter their recorded acquisition or claim standalone toolchain recovery.

The full ready replay is complete and integrity-valid: **4,332 passes, 26 mismatches, 82 unsupported cases and two oracle errors across 4,442 cases**, with zero candidate internal errors, zero historical-pass regressions and **88/88 metamorphic checks** passing. The separate original subset remains **eight matches and three diagnostic mismatches across 11 cases**. These are complete acquisitions with retained failures, not full compatibility passes.

One actual M1 acquisition matches **6,078 checks per native, Node and Chromium target**; strict and architecture predicates pass on that same acquisition. Eight actual freshness commands pass. Applying the exact current `full_gate_failures` function to the freshly checked manifest and complete replay reports **1,780 overlapping unmet conditions: 472 implementation, 472 mapping and 836 required-case conditions**. This is function-based accounting, not a new `--verify-full` CLI invocation or a completion percentage. The inventory remains 4,545 cases, 4,442 ready, 71 unimplemented, 32 unmapped and 475 requirements.

Formatting from prepare02 and the later actual xtask, all-target Clippy with warnings denied, and Wasm checks pass with stable explicit inputs. All proof-dependent captures completed before Clippy; the original normal artifacts were retained first. One actual complete workspace run passes **1,658 tests, zero failed and five ignored** across 262 executable test targets, 25 compile-only examples and two doctest libraries. The terminal readback verifies expected/invoked targets, examples, doctests and outer-summary coverage; 13 filtered child summaries remain separate. Ignored tests are not passes. Cargo reuses the normal runtime from targeted-compile02 as Fresh. Recorder Rust/build bookends and post-terminal test-artifact hashes do not assert a full fixture or transitive Python freeze. The [Stage109 checkpoint](../reports/m2/stage109-typed-module-annotation-checkpoint.json) records exact evidence and restoration dependencies. Stage110, Stage111 and Stage113 isolated candidates are excluded. Existing List internal obligations and the whole-List 512 MiB Java heap failure remain required. Full M2/API, release-platform, performance and hosted-CI completion are not claimed.


## Stage103/104 reflection and Typed.hasProperty — recorded local validation

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

The [Stage104 checkpoint](../reports/m2/stage104-reflection-typed-checkpoint.json)
records these outcomes, source identities and restoration inputs. Later isolated
candidates are separate from this canonical implementation and validation scope.


## Stage102 List, Map and Mapping — historical local evidence

The collection matrix covers **List 995, Map 377 and Mapping 488 query/profile
contracts**. Targeted05 records **83 passes, zero failures and one ignored test**
on freeze04. All **1,859 ordinary contracts / 3,718 repeated observations** match
Java, including **37,368 raw fields and 1,506 ordered Logger events**. Corrected
selected native, N-API and Chromium acquisitions independently match the same 3,718
ordinary observations, with **44,804 shared fields and 1,506 Logger events per
target** in their portable field scope. Their acquisition/source integrity and
ordinary raw compatibility verdicts are separate from required internal failures.

One List internal contract retains two required unresolved observations on each
target. The original whole-List Java 512 MiB heap failure and earlier Set/glob
protocol obligations remain open. All eight default roles have actual local
passing records, including **1,502 workspace passes with four ignored**. Current
M1 matches Java on **6,078 checks per native, Node and browser target**, with
strict and architecture readback recorded separately. Unchanged original Set and
Mapping match; original Map retains the existing `Typed.hasProperty` gap at line
134 after successful `toTyped(Person)`.

Inventory remains **4,545 cases: 4,442 ready, 71 unimplemented and 32 unmapped**,
with **475 requirements**. Stage94's earlier-source **4,268/4,424** whole-corpus
aggregate is unchanged. Selected results do not produce a current full
ready-corpus result or an M2 completion percentage. Historical numeric/Set
1,218-contract results remain a separate scope. Full language/API, release-platform
and performance requirements remain open. See the
[Stage102 checkpoint](../reports/m2/stage102-collection-apis-checkpoint.json) for
the bounded local evidence and retained failures.


## Stage99 numeric and Set APIs — selected validation complete

Recorded local evidence: [Stage99 checkpoint](../reports/m2/stage99-numeric-set-apis-checkpoint.json).

The shared VM adds the 13 previously missing numeric routes and 52 Set routes:
identity/conversion, shifts and bitwise operations, character conversion and
fixed formatting, plus Set predicates, slicing, folds, extrema, sorting,
conversions and algebra. Evaluation stays in the common engine without a Java
fallback. Source coverage is distinct from complete API compatibility.

Pinned Java authority retains **1,218 query/profile contracts repeated twice**:
1,666 successful values, 764 ordinary Pkl errors and six internal observations.
The 542 numeric contracts and Set cohorts of 622, 36, 8 and 10 preserve their
original source, URI, order, both repeats and 39 power-assertion contracts.
On freeze04, targeted04 passes **25 tests, zero failures and one ignored test**.
All **1,215 ordinary contracts / 2,430 observations**, **26,960 shared raw fields**
and **1,238 ordered Logger events** match Java. Three foreign-entry internal
contracts retain six required unresolved observations; they are not passes.

The terminal workspace passes **1,418 tests, zero failures and three ignored
tests**, covering all **245 test executables**, 25 compile-only examples and two
doctest libraries. Thirteen nested child summaries are separate. Formatting02,
xtask02, Clippy02, Wasm01 and the full Python01 suite pass on freeze04; Python
runs **180 tests**. The default driver reuses actual Clippy02 and executes the
other seven roles. **All eight roles pass**, including final manifest and inventory
checks, with no missing or unexecuted roles and stable inputs.

Workspace Cargo reports the normal runtime as Fresh, reusing the actual
same-source targeted04 compilation. Its original compiler command, 113 inputs,
artifacts and source bookends are authenticated. The workspace newly compiles
`module_eval_probe` and `semantics_probe` against that runtime. Actual platform
schema admission succeeds by reading those terminal records; it is not a
platform run. Actual M1 now matches **6,078 cases per Rust/Node/Chromium target**
and across the shared engine; its separate strict and architecture gates pass.
Each selected native/N-API/Chromium target completes **23 programs / 46 repeat
sessions / 2,436 observations**. All **2,430 ordinary observations** match:
1,666 successes, 764 ordinary errors and 1,238 ordered Logger events per target.
The platform reader compares 31,820 shared raw fields including expressions
and Logger counts; this differs from the Rust harness field-count scope.
Six observations from three required internal contracts remain unresolved on
each target. The window is complete with stable sources and no acquisition
errors; **comparable_pass=true, all_required_pass=false**.

The current native adapter compilation exits 0 but records a 686-byte
`rust-objcopy` warning: stripping aborted because `@rpath/libLLVM.dylib` was
unavailable. The actual artifact remains bound; requested stripping is not
claimed complete. M1 uses a scoped LLVM loader path. This current adapter
warning is separate from the historical Stage98 N-API warning.

The original Float/Int/Set native replay is terminal with **two of three raw
matches**. Float and Int match Java; Set fails at `List(1).minWith(comparator)`
on original line 249 because the List route is missing. Its native stdout is
empty, stderr is 931 bytes and exit is 1, versus Java stdout of 6,984 bytes,
empty stderr and exit 0. This required mismatch remains open. Replay the
unchanged original after the full List implementation is integrated; selected
Set results do not establish compatibility of the complete original module.

The initial metadata-type compile failure, targeted02's **22/3/1** result,
comparator diagnostic and foreign-entry argument-order corrections, premature
pre-freeze controller failure, and first default driver's Clippy failure remain
preserved. The lint correction uses equivalent byte parity and a statement-local
allowance for the exact pinned JDK decimal-estimate constants; it changes no
numeric expectation. Earlier targeted03 is separate from current targeted04.

Stage94's earlier-source **4,268/4,424** whole-corpus aggregate is unchanged.
The inventory remains **4,545 cases: 4,442 ready, 71 unimplemented and 32 unmapped**,
with 475 requirements. Full ready-corpus replay, the original List Java heap
error, Stage98 glob protocols, other language/API, release-platform and performance
obligations remain open. These selected results do not establish complete numeric/Set/M2 compatibility.

## Stage98 shared String API checkpoint

The shared VM adds the 28 previously missing String member routes and corrects
inherited uppercase result identity. Case conversion, nullable indexing,
prefix/suffix and predicate slicing, trimming, padding, range replacement,
numeric/Boolean parsing, glob classification and digests retain UTF-16 values,
pinned JDK case/word-break rules and ordinary type-check/Host continuations.
Production evaluation uses no Java fallback.

Pinned Java has **1,209 query/profile contracts repeated twice**: 1,790 values,
622 ordinary errors and six internal-error observations. Native, N-API and actual
Chromium each attempt all **2,418 observations** in 19 programs/38 sessions and
match all **2,412 comparable observations**, including 160 ordered Logger events.
Default, Turkish, power-assertion, boundary and result-identity profiles remain
separate. The three glob contracts `[]`, `[!` and `[^` retain six required
unresolved observations per target; `all_required_pass` remains false. Complete
status, UTF-8/UTF-16 stdout/stderr and Logger fields are compared; Java exception
class/cause metadata is retained outside the portable comparison boundary.

One final literal workspace command passes **1,393 tests, zero failures and two
ignored tests** across 242 test executables, 25 compile-only examples and two
doctest libraries. The full Python rerun passes **180 tests**. All eight local
check roles pass with explicit source scope: workspace, xtask, formatting,
Clippy, Wasm, Python, manifest freshness and inventory freshness. Formatting
reuses its earlier result because only three Python test files changed; xtask
reuses its earlier result after checking its unchanged inputs and two separately
pinned reference metadata files. Neither is relabeled as a fresh final-source run.

The six original String-related native module comparisons now match Java's full
stdout/stderr/exit and public file/write maps: four successes and two expected
Pkl errors. Native private state stays unchanged; Java Host-private cache bytes
are provenance, not cross-runtime comparison channels. One fresh M1 acquisition
passes **6,078 strict comparisons per target**, with the separate architecture
contract and **92 Node/Chromium checks each**.

Earlier compiler, padding/lifecycle-fixture, formatting, Clippy collector,
168-test Python and first platform failures remain preserved. The successful
portable retry uses a process-local `DYLD_FALLBACK_LIBRARY_PATH` for the pinned
Rust toolchain's LLVM library; no toolchain or source change is inferred. The
successful earlier N-API Cargo build retains its failed debug-strip warning.
Reused unit artifact hashes are captured at successor admission, not retroactively
claimed as original compiler-time hash bookends.

This is a local selected checkpoint. The original full 4,442-ready union has not
been rerun here; Stage94's earlier-source **4,268/4,424** aggregate is unchanged.
The 71 unimplemented and 32 unmapped cases, original List Java heap error,
remaining language/API and release-platform obligations stay open. No complete
String/M2 or performance result is claimed. Exact evidence is in the
[Stage98 checkpoint](../reports/m2/stage98-string-apis-checkpoint.json).

## Stage97 expression adapter and token requirements

The filesystem expression probes now use the shared module evaluator for complete
caller expressions, with real native module/resource Host helpers and unchanged
Java probe settings. Only the finite lexical/literal-node scopes of `TRUE`,
`FALSE` and `NULL` are closed: 18 new cases and nine historical controls cover
spelling, reservation, identifier continuation, quoted bypass and literal nodes.
Boolean/null semantics and the other whole-language requirements remain open.

Shared token tests match 54 observations and 324 raw fields. Actual native argv
comparisons match **36/36** after preserving the earlier 24/36 result. The full
3,545-case scalar replay matches **3,537**, retaining seven mismatches and one
Java oracle error with zero old-pass regressions; 88 metamorphic groups pass.
The original 3,527 Java process contracts are reused unchanged. New 18-case Java
API observations retain their own actual helper-process exit and the explicit
row-status-to-native-exit mapping. Strict compatibility still fails on the eight
known cases; acquisition integrity succeeds. Their stdout/stderr remain identical;
six native exits change from 1 to unsupported code 2, so whole-triple equality
with historical failing native runs is not claimed.

Default Rust checks pass. The workspace has **1,371 passes, zero failures and one
existing ignored probe**, including the seven adapter/Host tests newly connected
to the default suite. No new shared engine/binding, Node/browser or performance
claim is made. Current inventory is 4,545 cases, 4,442 ready, 71 unimplemented and
32 unmapped; three of 475 requirements have finite implemented/mapped scopes.
Stage94's earlier-source whole-corpus aggregate is not updated with these results.
See the [checkpoint](../reports/m2/stage97-token-expression-checkpoint.json).

## Stage96 scalar, collection and function APIs

The shared runtime adds `Float.toInt`, `Int.toRadixString`, `List.getOrNull`,
`Set.restOrNull` and `Function.applyToList`. Numeric conversion retains Java's
range and constraint diagnostics; List access returns the original cached value.
Set removal repeats the upstream hash/equality passes and retains the persistent
backing-table behavior of recursively changing keys. Function calls reuse the
original call target, captures, type checks and suspension lifecycle.

Canonical formatting, xtask, manifest freshness, Clippy and Wasm checks pass.
One full workspace command passes **1,358 tests, zero failures and one existing
ignored probe**, covering 238 test executables, 25 compile-only examples and two
doctest libraries. Thirteen nested child summaries are counted separately.
The actual targeted runtime compilation is reused by the workspace as Cargo
`Fresh`; both producer and reuse identities are recorded.

Java authority contains 103 query/profile contracts repeated twice: 146 successful
values, 60 ordinary errors and 12 ordered Logger events. The original 99 contracts
remain unchanged; four deterministic recursive-key controls are separate.
Native, N-API and Chromium each match all **206 observations** as raw bytes.
One M1 acquisition also passes 6,078 strict comparisons per target and the separate
architecture contract. Earlier failed targeted attempts remain preserved.

Original CLI comparisons match **2/5**: both function originals. The other three
now reach missing `Float.toFloat`, `Int.toInt` and `Set.any` members. Their raw
differences remain failures; the original List Java heap error remains unpaired.
The JVM identity-dependent hash approximation is still a limitation. Selected
comparisons do not establish full original-module, language, API or performance
compatibility. Java exception class/cause metadata is preserved but is outside
the portable raw-channel comparison. Stage94's earlier-source **4,268/4,424** aggregate is unchanged.

See the [Stage96 checkpoint](../reports/m2/stage96-core-apis-checkpoint.json)
for exact local source, artifact, raw-result and archive identities.

## Stage95 PListRenderer integration

The shared runtime implements `PListRenderer.renderDocument` and `renderValue`:
XML property-list tags and document framing, indentation, UTF-16 escaping,
numeric and Bytes output, null omission, converters and annotations. Lazy
forcing, first-error order and Host/GC lifecycle behavior use the shared renderer
continuation.

Java authority contains 97 queries repeated twice: 102 successes, 90 ordinary
errors and two internal-error observations. The 96 comparable queries retain
80 reached renderer controls and 16 syntax-preempted errors. One recursive
converter query remains unresolved; it is not a passing Rust expectation.
The earlier isolated result of 131 passes across 16 targets remains separate.

Canonical formatting, manifest refresh/freshness, xtask, Clippy and Wasm checks
pass. One literal workspace test command covers 236 test executables, 25
compile-only examples and two doctest libraries: **1,341 passed, zero failed,
one existing ignored test**. Thirteen nested summaries are separate from the
238 outer summaries: 12 parse directly and one retains its interleaved diagnostic
line and exact raw-byte location. Rust 1.98.1 uses opt1/debug0/strip-symbols,
assertions and overflow checks, one job and no incremental compilation for
workspace tests/xtask; Clippy and Wasm use opt0/debug0.

The nine original native CLI-argv captures match raw stdout, stderr and exit
status **9/9**, including one expected ordinary error. Original Java arguments,
URIs and environments are retained. Empty file/write sets do not establish
nonempty generated-file or complete CLI support; the prior 18 API byte-vector
observations remain a different boundary.

M1 strict comparison and the separate architecture contract: 6,078 strict comparisons and the separate architecture contract pass on each target.
Selected Native/N-API/Chromium results: 450/450 raw matches on each of Native, N-API and Chromium: 234 successes, 216 ordinary errors, 4,000 shared fields and 40 ordered Logger events per target.
The selected comparison covers 225 queries (96 Plist + 129 Properties), repeated
twice: 450 observations per target. The converter session compares 11 selected rows against a Java session containing
12; the unresolved row is retained but excluded. This does not establish full
original converter-process protocol parity.

The [checkpoint](../reports/m2/stage95-plist-renderer-checkpoint.json) lists the
evidence parts and [publication review](../reports/m2/stage95-plist-renderer-publication-review.json).
Stage94's earlier-source **4,268/4,424** whole-corpus result is unchanged. Full M2,
API, release-platform and performance gates remain open.

## Stage94 complete ready-corpus composition

Stage94 combines four canonical integrity-checked reports covering all
**4,424 currently ready cases** on the Stage93 implementation (`a1b94fa`). The
aggregate is **4,268 raw matches, 52 mismatches, 102 unsupported cases and two
Java oracle errors**, with no Rust internal error. All four offline integrity
checks and an independent composition review pass.

| Adapter | Cases | Raw matches |
| --- | ---: | ---: |
| Scalar expressions | 3,527 | 3,519 |
| Upstream original modules | 888 | 740 |
| Historical collections | 8 | 8 |
| JVM classpath | 1 | 1 |

Against Stage86's same 4,416 case IDs and input contracts, **46 cases improve
and no previously passing case regresses**. The improvements are 37 previously
unsupported modules and nine previously mismatching modules. All 4,214 prior passes
remain passing. The eight new historical observations are counted separately.
The pinned Java oracle metadata is unchanged.

The first acquisition stopped at its free-space guard with child exit `-15`.
Its completed scalar observations were recovered with original raw bytes,
arguments and classpaths intact; the other three adapters have separate fresh
captures. The interrupted attempt and partial XML evidence remain preserved.
The historical launcher's exact loaded source bytes remain unproven because
of a preparation race; its explicit qualification accompanies the independently
verified child command, source, artifact and raw outcomes. Caller-supplied
artifact labels stay unchanged, with actual Stage93 producer evidence recorded
separately. This is an explicit composition, not one uninterrupted acquisition
or a single-directory canonical `--verify-full` run.

The whole-union strict predicate still reports **1,790 overlapping unmet
conditions**. The inventory remains 4,527 cases, including 71 unimplemented and
32 unmapped cases outside this replay. These counts do not establish M2,
complete APIs, release platforms, speed or memory parity. No new Rust workspace
or browser/Node run is attributed to this evidence-only checkpoint.

The [checkpoint](../reports/m2/stage94-full-ready-composition-checkpoint.json) and
[publication review](../reports/m2/stage94-full-ready-composition-publication-review.json)
identify all three required companion archives:
[interrupted acquisition and recovered scalars](../reports/m2/stage94-full-ready-composition-evidence-01.tar.gz),
[fresh original modules](../reports/m2/stage94-full-ready-composition-evidence-02.tar.gz), and
[JVM, historical and supporting evidence](../reports/m2/stage94-full-ready-composition-evidence-03.tar.gz).
Earlier sections retain their checkpoint-specific sources and results.

## Stage93 YamlRenderer integration

The shared runtime now renders YAML documents and values, including streams,
indentation, null-property omission, and the `compat`, `1.1` and `1.2` quoting
modes. It preserves converter and annotation handling, lazy evaluation and
error order, scalar formatting, and lossless UTF-16 escaping. Host suspension,
GC, cancellation and retries use the existing renderer continuation.

Pinned Java authority contains 332 queries repeated twice. The isolated candidate
matches all **321 comparable renderer queries twice**, including ordinary errors
and represented UTF-8/UTF-16/status/ordered Logger fields; 129 Properties queries
remain regression coverage. Its 14 targets pass 110 named tests. Twenty-three
unchanged renderer originals also match twice through API byte vectors and,
separately, twice through native CLI-argv stdout/stderr/exit captures. All observed
file maps are empty; nonempty generated-file and full product CLI behavior are
not established by that probe.

The reviewed 7,843-path implementation is integrated. Formatting, manifest
refresh/freshness, repository verification, workspace/all-target Clippy and
runtime Wasm compilation pass. The manifest refresh is an authorized source
transition, with case and requirement states unchanged. Canonical workspace
result: **1,319 passed, zero failed and one existing ignored test across 233 test executables, 25 compile-only examples and two doctest libraries in 36 invocations; 13 nested child summaries remain separate**. M1 strict and architecture contract:
**6,078 strict comparisons and the separate architecture contract pass on each target**. Selected Native/N-API/Chromium comparison:
**900 raw matches per target** from 450 queries repeated twice: 642 successes,
258 ordinary errors, 8,040 shared fields and 84 ordered Logger events per target.
Independent source and raw-evidence reviews passed.

Public evidence and its separate archive review are linked through the
[checkpoint](../reports/m2/stage93-yaml-renderer-checkpoint.json),
[evidence archive](../reports/m2/stage93-yaml-renderer-evidence.tar.gz) and
[publication review](../reports/m2/stage93-yaml-renderer-publication-review.json).
YAML Parser's ten queries and eight originals, and one Java oracle-internal
indent-width query, remain open. The inventory stays **4,424 ready, 71
unimplemented, 32 unmapped**. At the Stage93 checkpoint, the completed whole-corpus result was
Stage86's **4,214/4,416**, on earlier sources; these selected results do not update
that aggregate or close the nine M2 workstreams and release/performance gates.

## Stage86 completed full ready-corpus replay

The complete 4,416-case replay, separate raw-evidence verification, and
prior-pass regression audit finished successfully on the recorded local macOS
arm64 host. That replay aggregate is **4,214 raw matches, 61 mismatches,
139 unsupported results, and 2 Java oracle errors**. There are no Rust internal
errors in this run. Original modules match 694/888, expression strings
3,519/3,527, and the JVM classpath case matches. Compared with the completed
Stage76–78 replay, 26 cases now match and no previously passing case regressed.

The replay reused sealed Stage67 Java outputs and the recorded, unchanged
Stage86 candidate artifacts; it started no fresh Java process. Source and
artifact identities, complete raw channels, original case order, and execution
contracts were verified. Earlier failed or interrupted acquisition attempts
remain separate evidence. This checkpoint adds full-corpus observations; it
does not claim another Rust workspace or platform test run.

At that replay boundary, the inventory contained 4,527 cases: 4,416 ready,
71 unimplemented, and 40 unmapped. Stage92 admission below updates readiness,
not the completed replay result.
The strict full gate still has 1,791 overlapping unmet conditions: 475
implementation obligations, 475 test-mapping obligations, and 841 required-case
obligations. Passing an execution does not close a missing requirement mapping.
All nine M2 workstreams, complete APIs, release platforms, and performance and
memory targets remain open. See the [full replay checkpoint](../reports/m2/stage86-full-replay-checkpoint.json).

## Stage92 historical collection admission

The [Stage92 historical-admission checkpoint](../reports/m2/stage92-historical-admission-checkpoint.json)
records eight unchanged historical collection modules with an explicit
[`historical-collections-module-eval` registry](../compatibility/m2-historical-collections.json).
Their original case IDs remain in the 4,527-case inventory, in the same order;
all 475 requirement records remain unchanged. The current execution inventory
is **4,424 ready, 71 unimplemented and 32 unmapped**. Readiness means an executable
adapter contract, rather than completion of a language feature or requirement.
The other 32 historical candidates and the three separately selected regression
CLI contracts retain their previous states and commands.

The fixtures, dependency module, license and notice retain their pinned rev1
bytes. The new historical profile explicitly uses Pkl 0.32.0 `eval --no-cache
--no-project --settings pkl:settings`; it does not replace an older invocation
with different flags. The adapter records raw stdout, stderr, exit status,
generated-file paths and bytes, and unexpected managed-workspace writes.

Canonical Python checks pass 76 tests in both normal and optimized modes;
inventory freshness and repository verification pass. A real finite mixed
acquisition runs all eight historical modules and one unchanged upstream Boolean
module twice through canonical `execute` and `check_report`: **18/18 comparisons
match**, including 16 historical observations. A target-only selection wrapper
narrows the case list without changing engines, oracle data, profile guards or
results. Offline CLI subset checks pass, while the strict full gate stays failed.
The retained native probe is bound to its actual Stage91 workspace compiler,
dep-info, runtime and extern inputs; caller-supplied labels in the acquisition
are preserved. No new native evaluator/probe build or JVM-profile acquisition
is claimed. `cargo xtask verify` compiled the xtask verifier.

The earlier 16 Stage90-native comparisons also pass a read-only check through
the new canonical adapter, with their original provenance intact. The original
schema6 mixed-profile verification defect and its corrected successor remain
separate history. Actual generated-file and unexpected-write sets are empty;
nonempty-file capability, a global filesystem sandbox, complete CLI behavior,
and API/UTF-16/structured Logger compatibility are not established here.

Local receipts are under `target/m2/stage92-canonical-integration-01`,
`stage92-canonical-checks-01`, `stage92-canonical-mixed-01` and
`stage92-canonical-offline-cli-check-01`. This admission does not change the
then-completed Stage86 **4,214/4,416** whole-corpus replay or establish full M2,
release-platform or performance completion.

## Stage91 PropertiesRenderer checkpoint

The shared renderer now implements PropertiesRenderer document/value rendering,
converters, annotation transforms, path flattening, directives and lossless
UTF-16 escaping. String `chars`/`codePoints` and List `count` support the pinned
originals. Host suspension, roots, cancellation and retries use the existing
shared continuations. The [checkpoint](../reports/m2/stage91-properties-renderer-checkpoint.json)
records the scoped implementation and its preserved failed attempts.

Formatting, repository verification, Clippy and Wasm compilation pass locally.
The complete default workspace target set was checked in bounded Cargo cohorts:
230 test executables, 25 compile-only examples and two doctest libraries. The
first acquisition recorded 1,275 passes, one stale unsupported-renderer assertion
failure and one ignored probe. After changing only that test source, its whole
10-test target passed. Independent source and compiler dependency review composes
the final **1,276 passes, zero failures and one ignored test**, excluding 13
nested child summaries. The original failed run remains failed; this is complete
target coverage through multiple invocations, not a single workspace command.

The selected portable comparison has 147 new queries and 64 collection regressions.
Native, N-API and Chromium each match **422/422 repeated observations**, including
represented UTF-8/UTF-16, ordinary errors and ordered Logger fields. A single M1
acquisition matches 6,078 primitive results per target and passes its separate
architecture contract. Fourteen unchanged PropertiesRenderer originals also
match two isolated native CLI-argv probe captures and two API byte-vector runs.
Java exception metadata, complete product CLI behavior and nonempty generated
files remain outside those comparisons.

At the Stage91 checkpoint the inventory had 4,527 cases, with 4,416 ready,
71 unimplemented and 40 unmapped. The full-corpus aggregate above was not rerun
on Stage91 sources. Historical admission is recorded separately in Stage92;
YAML remains separate work, and
all nine M2 workstreams and the release and performance requirements stay open.

## Stage90 lazy collection checkpoint

The shared evaluator now implements cached Listing endpoint properties,
Mapping folds, indexed Listing folds, and lazy Listing/Mapping membership and
predicate visitors. Listing distinctness selectors, joins, and `lastIndex`
preserve the pinned Java force order, insertion-time hashes, equality direction,
and early returns. Continuations retain roots across Host reads, cancellation,
retry, and garbage collection.

The two independently reviewed candidates are integrated into 3,398 source,
test, and Java-fixture paths. All required local Rust checks passed on macOS
arm64 with Rust 1.98.1: the full workspace records **1,247 passes, zero failures,
and the existing ignored `frontend_probe`**. Thirteen nested child summaries
are accounted for separately. Source bookends cover 194,914 paths; independent
readback checked the actual compiler, dependency, and artifact identities.

The final isolated candidate matches 139 Java queries twice, including complete
represented result/error UTF-8 and UTF-16 channels and ordered Logger fields.
Eight unchanged historical modules also match stdout, stderr, and exit status
twice. These module observations do not establish generated-file or full CLI
compatibility, and do not automatically promote the historical inventory rows.
Earlier compiler, transport, and evidence-reader failures remain preserved.

The integrated source matches **278/278 observations on each of Native, N-API,
and Chromium**: all 139 queries run twice across 14 independent evaluator
programs. Raw result/error strings, status, ordered Logger fields and process
protocols are compared. Java type/exception/cause metadata remains recorded
outside the represented portable API. One M1 acquisition also matches all
6,078 primitive results per target and passes the separate architecture contract.
The [checkpoint](../reports/m2/stage90-lazy-collections-checkpoint.json) records
the exact source/build identities, failed attempts and local verification.

At this checkpoint, the completed whole-corpus aggregate was the Stage86 result above;
all nine M2 workstreams and full API, release-platform, speed, and memory targets
remain open.

## Stage87–89 integrated language checkpoint

The [integrated checkpoint](../reports/m2/stage87-89-language-checkpoint.json)
adds Map/Mapping missing-key suggestions, semicolon/comment continuation,
`String.substring` and `substringOrNull`, signed-literal trace spans, and scope
checks for `super` method calls. Suggestions preserve Java's UTF-16 similarity,
key-hashing observations, and stable ordering. Substring operations preserve
unpaired UTF-16 units and Java's validation order. Nested object construction
restores the enclosing `super` scope, and invalid calls fail before evaluating
their arguments.

All required local Rust checks passed on macOS arm64 with Rust 1.98.1. The full
workspace recorded 1,228 passing tests, zero failures, and the existing ignored
`frontend_probe`; 13 nested child summaries are excluded from that total.
Independent readback verified 221 test executables, two doctest groups, source
bookends, and actual compiler/dependency identities. Compiler checks and
workspace tests retain their recorded optimization profiles; these are not
performance measurements.

Native, N-API, and Chromium each match **294/294 observations** from 147 queries
repeated twice: 12 Stage87 queries, 83 Stage88 queries, and 52 retained regressions.
Comparisons include complete represented result/error UTF-8 and UTF-16 channels,
status, ordered Logger fields, and process protocols. Java exception/class/cause
channels remain reference authority outside the represented portable API.
Separately, 14 standalone Stage89 modules match twice on each target through the
module-output byte-vector API. Five sibling-module cases remain unexecuted
through that adapter; actual CLI processes and generated files are separate
verification obligations.

One M1 acquisition matches all 6,078 Java primitive results per target and passes
the separate architecture contract. The first combined capture failed because
its native driver did not service an external-property read. The successor
records `prop:` requests and missing responses, matching the Java helpers' empty
external-property map. Only the native transport adapter was rebuilt; the
recorded core, portable bindings, and M1 acquisition were reused. The failed
capture and its matching prefix remain preserved.

This checkpoint does not replace the completed Stage86 whole-corpus aggregate
above. All 4,527 inventory cases, 111 unready cases, 1,791 overlapping full-gate
obligations, and nine unfinished M2 workstreams remain accounted for. Stage90
candidate results, full APIs, release platforms, and performance targets are
outside this checkpoint.

## Stage86 List membership, insertion hashes, and constraint observations

The shared runtime now implements `List.contains` with stored-element-first
boxed equality, no hashing, and first-match termination. Listing distinctness
indexes insertion-time hashes and retains the query hash across suspended or
reentrant comparisons. Constraint observations distinguish resolved builtin
constructors, parser literals, folded constants, and interpolation fragments.
The original `typeConstraints9` and `typeConstraints9a` diagnostics now match
Java in full, including their power-assertion diagrams.

All five required Rust checks passed locally. The complete workspace recorded
1,149 passes, one ignored test, and zero failures; 13 nested child summaries
were excluded from the count. The profile used opt1/debug0/symbol stripping
with assertions and overflow checks enabled. These are correctness checks,
not performance measurements.

Actual native, N-API, and Chromium captures each matched 104 observations:
35 new Java queries and 17 prior distinctness queries, each repeated twice.
Complete public UTF-8/UTF-16, status, ordered Logger, and session protocol
channels were checked. Each target produced 54 successful values, 50 expected
ordinary errors, and six Logger events. Java-only exception metadata remains
outside the adapter comparison. One M1 acquisition matched all 6,078 results
per target and passed its separate architecture-contract judgment.

The first platform capture matched only 76/104 observations per target because
the capture adapters disabled power assertions for the 15-query power cohort.
That failed capture remains intact. A target-only adapter now passes the exact
Java-authorized setting for each program; its recorded build links the unchanged
tested core. The successful successor is a new acquisition. Earlier candidate,
preflight, and source-dispatch failures also remain recorded.

See the [Stage86 checkpoint](../reports/m2/stage86-contains-hash-power-checkpoint.json).
The subsequent full replay and separate verification now establish the
4,214/4,416 aggregate recorded above.
The 111 unready cases, all nine M2 workstreams, full API/release-platform scope,
collision-tree behavior, and speed/memory targets remain open.

## Stage85 distinct collections and reentrant hashing

The shared runtime now implements `List.distinct` and `Listing.isDistinct`.
List deduplication preserves first occurrence order and the pinned EconomicSet
comparison/hash transition. Listing checks first force direct members and retain
the Java cache reuse behavior through empty generic wrappers. Structural-key
construction for reentrant untyped Dynamic, Listing, and Mapping values reads
their completed cache prefix; typed, module, and strict collection paths retain
their existing rules.

All five required Rust checks passed locally. The completed workspace command
reports **1,136 passed, 1 ignored, and 0 failed** across 208 test binaries and two
doctest groups; 13 nested child summaries are excluded. The stale Regex test
exclusion is removed. The run uses opt1, no debug information, symbol stripping,
and enabled assertions and overflow checks. Its first attempt stopped at a disk
growth limit; the successful successor reuses the compiled artifacts and runs
the entire command again. Both attempts remain recorded.

Four unchanged originals pass their recorded Java byte comparisons. The new
17-query cohort matches twice on native, N-API, and Chromium: 34 observations
and two ordered Logger events per target. Full public output/error UTF-8 and
UTF-16 channels and status are compared; Java-only type, exception, and cause
channels remain outside the adapter comparison. One fresh M1 acquisition also
matches all 6,078 Java results per target and separately passes its architecture
contract. It is one acquisition with two judgments, not two independent runs.

The [checkpoint](../reports/m2/stage85-distinct-checkpoint.json) preserves the
initial candidate's reentrant internal error and its successful correction.
These selected results did not update the then-completed 4,188/4,416 aggregate.
All nine M2 workstreams, 111 unready cases, full API and release-platform
requirements, and speed/memory targets remain open. General collision and
recursive-hash behavior and Listing candidate-search performance need further
work; no complete compatibility or performance claim is made.

## Stage84 List search and transforms

The shared resumable collection engine now implements `List.find`, `List.filter`,
and `List.flatMap`. Search short-circuits on the first match; filtering preserves
the original elements and order. Flattening accepts List/Set results and drains
each rooted inner collection before invoking the next callback. Predicate and
result-type errors follow the pinned Java behavior.

The full workspace command passed 1,126 tests, with zero failures and two ignored
tests across 202 integration targets, three unit targets, and two doctest
invocations. Thirteen nested child observations are counted separately. An
explicit run of the formerly unsupported Regex regression also passed; its stale
ignore annotation remains a follow-up cleanup. Formatting, reference verification,
workspace/all-target Clippy, and the Wasm compilation check passed. The workspace
run used opt1, no debug information, symbol stripping, and enabled assertions and
overflow checks; it is not a performance measurement.

Six unchanged originals pass their complete Java output comparisons. The new
44-query cohort matches twice on native, N-API, and Chromium, including full
UTF-8/UTF-16 output/error strings, status, and ordered Logger fields. Each target
has 88 matching observations and 22 logger events. Java-only type, exception,
and cause channels remain outside this adapter's comparison surface. Separate
fresh M1 strict and architecture-contract experiments each match all 6,078 Java
results per target. Platform artifacts retain their original workspace/M1 build
provenance; reuse is not reported as a new compilation.

The [checkpoint](../reports/m2/stage84-list-search-transform-checkpoint.json)
preserves the initial test-logger configuration failure and successful successor.
The Stage84 full 4,416-case replay stopped at the sampled disk-space floor.
Its incomplete 3,972-case progress prefix is retained as failure evidence;
no Stage84 full-replay or separate-verifier result is claimed. The official
aggregate below is unchanged. All nine M2 workstreams and full API,
release-platform, speed, and memory requirements remain open.

## Stage79–83 focused implementation checkpoint

The shared runtime now preserves Reference constraint owners, the nearest authored
minimum-version import frame, typed-equality forcing order, distinct module URIs
in same-name type conflicts, and the enclosing owner in duplicate-class errors.
Qualified `pkl:base.Pair` calls use the common constructor. Class labels share
the pinned Java module-name rules across values, expected types, and Reference
metadata, including `pkl.base` and `ModuleClass` precedence.

On the final source set, 109 distinct Rust test functions passed across 24
integration targets; two nested child reruns are not added to that count. The
new class-display cohort compares all 13 queries twice against Java, including
complete UTF-8/UTF-16 results and observed zero logger events. The original
`classes/constraints14.pkl` regression now passes its full Java byte comparison.
Formatting, reference verification, workspace/all-target Clippy, and the Wasm
compilation check exited zero locally on macOS arm64 with Rust 1.98.1.

The [focused checkpoint](../reports/m2/stage79-83-focused-checkpoint.json)
retains earlier failures and separates their source revisions from the final
results. The full workspace test command, current native/Node/browser
comparisons, and the 4,416-case replay remain pending. Limited disk capacity
required preserving and releasing inactive build artifacts between checks.
The official aggregate below is unchanged; these focused results do not close
M2 or establish API, release-platform, or performance parity.

The [source binding](../reports/m2/stage79-83-focused-source-binding.json)
matches all 2,363 recorded source paths to implementation commit `8d25b6f`.
It does not claim complete compiler-input coverage or add compatibility passes.

## Stage76–78 canonical integration and local replay

The shared evaluator now includes selected Regex and String behavior, `String.isRegex`,
the reviewed UTF-16 syntax distinction, leading shebang handling, empty
parenthesized-type diagnostics, and Int/Float `sign`, `abs`, `ceil`, `floor`,
`round()` and `truncate()`. The earlier isolated candidate and its failed
first-controller check remain separate historical evidence.

All five required Rust commands exited zero against the frozen 17,774-path
canonical source set. Native, N-API, and Chromium each completed two observations
of 771 selected queries with their Java-authorized public channels matching.
Logger compatibility is limited to rows with Java logger authority. The strict
M1 primitive comparison matched all 6,078 Java results per target, including
the two formerly failing Regex rows. The old architecture contract reported
failure because its known-gap policy named those rows. The revised contract
completed with exit zero, all 6,078 raw results matching per target, and no
remaining known-gap exception.

The selected 4,416-case replay recorded **4,188 raw matches, 86 mismatches,
139 unsupported results, 2 Java oracle errors, and 1 Rust internal error**.
Original modules matched 668/888, expression strings 3,519/3,527, and the JVM
classpath case matched. The internal error is the original
`classes/constraints14.pkl` case. Java results were reused from sealed authority;
the replay started no fresh Java process. The separate integrity verifier and
independent regression audit exited zero. The audit found 25 new matches and no
prior-pass regressions against Stage73–75. The strict full gate still has 1,791
overlapping unmet conditions.

The inventory still contains 4,527 cases: 4,416 ready, 71 unimplemented, and
40 unmapped. Its 111 unready cases, full-gate failures, and all nine M2
workstreams remain open. The selected results do not establish full M2, product
API, platform release, or performance parity. The [implementation checkpoint](../reports/m2/stage76-78-implementation-checkpoint.json) retains the failed
attempts and exact source/artifact identities.

The [source binding](../reports/m2/stage76-78-source-binding.json) connects the
recorded 178,382 input paths and a separate 72-input appendix to implementation
commit `7ce4769`, pinned references, or preserved pre-change copies. Eight
documentation/tool transitions are recorded separately. The first binding
attempt failed with insufficient disk space; a fresh read-only completion
verified the retained maps and exited zero. This adds no compatibility passes
and does not claim that the recorded maps cover every compiler input.

## Stage76–78 isolated candidate validation

Before canonical integration, the isolated candidate completed nine continuation
commands after an independent review recovered the first command's artifact
provenance. The first controller failure remains recorded: Cargo and its tests
succeeded, but the checker expected an `.rlib` where rustc consumed `.rmeta`,
and three build-script aliases exceeded its initial cache scope. The first
command was not rerun. The ten actual command results cover 29 distinct outer
Rust test functions, including 389 Regex/String/collection rows, 72 numeric
rows, three unchanged upstream syntax cases, and a 56-row M1 control. The
numeric tests compare result kind and full text; they do not verify every
exported API or logger channel. The separate 117-test vendor suite is not
added to the runtime count. This candidate checkpoint is historical and did
not itself establish canonical or full-language compatibility. See the
[candidate checkpoint](../reports/m2/stage76-78-candidate-checkpoint.json).

## Stage 73–75 selected implementation checkpoint

The shared evaluator now has selected handling for first-access local/public
property resolution, declared `Reference` type metadata in the prototype's
lexical environment, and `NonNull` and qualified builtin alias error ordering.
These changes were integrated into the canonical source tree. The M2 inventory
still records 475 requirements and 4,527 cases: 4,416 ready, 71 unimplemented,
and 40 unmapped. The implementation links do not promote remaining scopes to
complete.

The canonical source-bound local validation completed all five required Rust
commands with exit code zero. Workspace test output contains 1,069 distinct
passes, no failures, and two ignored tests; 13 nested child reruns account for
the difference from the 1,082 raw pass lines. These are local test results,
not Java compatibility outcomes.

Selected Java-authorized platform comparisons cover 235 earlier queries, four
`Reference` alias queries, and 71 new queries. Native, N-API, and Chromium each
completed two observations per query, 620 per target, with all selected public
channels matching their recorded authority. The earlier cohort has 91 queries
without Java logger authority, or 182 observations per target; their logger
channels cannot be counted as compared. The 71 new queries preserve each
authored module/evaluator boundary and captured logger channels. The separate
M1 platform experiment passed its architecture contract while retaining the
two known Regex compatibility failures on each target.

The full 4,416-case Rust replay records **4,163 matches, 110 mismatches, 141 unsupported results, 2 Java oracle errors, and 0 Rust internal errors**. Original modules match 643/888; expression strings match 3,519/3,527. The JVM classpath case matches. Compared with Stage69–72, 5 previously nonpassing cases now match, with no prior-pass regressions.

The linked replay, separate integrity verifier, and independent regression audit completed successfully. The replay reuses sealed Stage67 Java outputs and starts no fresh Java process. The strict full gate retains 1,791 overlapping unmet conditions. The 111 unready inventory cases and all nine M2 workstreams remain open. Earlier mapping rejection, interruption, ENOSPC, and monitor failures remain recorded separately.

See the [implementation checkpoint](../reports/m2/stage73-75-implementation-checkpoint.json) for raw evidence and execution provenance. These local records do not establish full M2, product API, release-platform, or performance compatibility.

The [source binding](../reports/m2/stage73-75-source-binding.json) links 170,567
recorded inputs to implementation commit `0202da8`, whose push to remote `main`
was verified, or to exact preserved copies. Five post-validation documentation
transitions remain separate from the frozen runtime and fixtures. The
[independent publication review](../reports/m2/stage73-75-source-binding-review.json)
verified every archive member and the separately published execution records.
Source binding adds no guest evaluations or compatibility passes; all remaining
M2 conditions above stay open.

## Stage69–72 — shared runtime and full ready-corpus replay

The [implementation checkpoint](../reports/m2/stage69-72-implementation-checkpoint.json) records shared super entry defaults,
import-qualified types, generics and VarArgs, compiled base alias/Mixin identity,
and const owner caching. It also preserves Reference alias diagnostic ownership
and the existing 256 KiB stack controls. All five required Rust checks passed.

Native, actual N-API, and Chromium each match 470 observations from 235 queries.
A separate four-query Reference alias comparison matches eight observations per
target. These comparisons cover selected raw string and captured Logger channels;
91 queries in the main matrix have no Java Logger authority. They do not establish
typed export or complete API compatibility.

The full 4,416-case Rust replay records **4,158 matches, 115 mismatches, 141 unsupported results, 2 Java oracle errors, and 0 Rust internal errors**. Original modules account for 638/888 matches; expression strings account for 3,519/3,527, and the JVM classpath case matches. Compared with the latest Stage68 Rust baseline, 6 previously nonpassing cases now match and no previously passing case regresses.

The replay reuses sealed Stage67 Java outputs with zero fresh Java processes.
Acquisition, independent integrity verification, and the Stage68 regression check
completed successfully. The strict full gate retains 1,795 overlapping unmet
entries; the 111 unready inventory cases and all nine M2 workstreams remain open.

The separate M1 experiment retains 6,078 shared-engine matches and 6,076 Java
matches per target. Its architecture contract passes, while the two known Regex
cases remain strict compatibility failures.

13 archives (167,482,056 bytes total, each below 95 MiB) preserve raw results,
failed attempts, source identities, and complete archive readback. Required01/02
failures and the first Wasm linker failure remain recorded separately from their
successful corrections. Publication and assembly receipts postdate the archives
and require separate source/Git binding. These are local macOS arm64 results;
full M2, product APIs, release platforms, speed, and memory targets remain open.

### Actual implementation source binding

The [source binding](../reports/m2/stage69-72-source-binding.json) connects the
verified inputs to implementation commit `a7c4b02`, whose push to remote `main`
was verified. It accounts for 169,117 inputs: 167,589 Git-backed inputs and 1,528
captured files, plus 1,824 frozen paths and five historical documentation versions.
The initial Git whitespace check exited two for trailing blank lines in two
unchanged upstream fixtures. Their exact pinned Git blobs justify that narrow
exception; the failed check is preserved. All 1,551 committed paths, modes, and
object IDs match the reviewed index.

The 7,628,060-byte archive passed full readback of all 160 members. The
[publication execution](../reports/m2/stage69-72-source-binding-execution.json)
and its raw channels are published separately. Source binding runs no guest
evaluations or Cargo checks and leaves the full M2 failures unchanged.

## Stage69–72 Java authorities

The [Java reference checkpoint](../reports/m2/stage69-72-java-checkpoint.json) preserves 235 distinct authored queries covering super entry defaults, import-qualified types, generics and VarArgs, base aliases/Mixin, and const owner behavior. It retains two JVM repetitions, full raw results and logs, earlier failed capture attempts, and exact source/JDK/JAR identities. Original CLI controls remain in the separately published Stage67 evidence.

These are Java authorities, not candidate compatibility passes. The [publication execution](../reports/m2/stage69-72-java-publication-execution.json) records the actual finite publication commands and raw outcomes separately from the six archives. The implementation and its actual Rust, platform, and full ready-corpus results are recorded in the separate implementation checkpoint above. The Java authority remains reference evidence.

## Stage 68 — predicates, method semantics, and full ready-corpus replay

The [implementation checkpoint](../reports/m2/stage68-implementation-checkpoint.json) records
shared String Base64, collection `isNotEmpty` and `every`, Reference amendment
rejection, inferred entry diagnostics, declaring-module class defaults, and lazy
object-method signature checks. All five required Rust checks passed. Native,
actual N-API, and Chromium each match 2,726 raw observations from 17 catalog
profiles containing 1,363 expressions. The separate `every` comparison matches
128 observations per target, including ordered logger channels. Existing Any/M1
Regex mismatches remain failures.

The full 4,416-case Rust replay records **4,152 matches, 119 mismatches, 143 unsupported results, 2 Java oracle errors, 0 Rust internal errors**.
Original modules account for 632 matches out of 888;
expression strings account for 3,519 out of 3,527. The JVM
classpath case matches. All previously passing cases remain passing, and
10 previously nonpassing cases now match. The three inherited scalar
regressions recorded in Stage67 are fixed by this candidate.

This replay reuses the sealed Stage67 Java outputs and starts **zero fresh Java
processes**. Its acquisition and separate integrity verification both exited
zero, with stable sources, artifacts, and restored workspaces. The strict full
gate remains failed with 1,799 overlapping unmet entries. The 111 unready
inventory cases and all nine M2 workstreams remain open.

Sixteen archives (309,305,507 bytes in total, each below 95 MiB) preserve
raw outcomes, complete payload/hash/mode readback, source bookends, and corrected
attempts. The initial native `every` wrapper failed before guest execution;
its corrected acquisition is separate. Comparison02's preparation failure and
Comparison03's expected Regex failures remain visible. Publication and assembly
process receipts postdate these archives and require the separate C source
binding. These are local macOS arm64 results, not full API, performance, release
platform, or hosted CI completion.

The separate Java authority contains 89 Base64, 47 collection emptiness,
and 64 `every` expressions in two JVM sessions. Fixture verification uses tracked
inputs and does not require historical `target/` directories. Passing fixture
validation establishes reference integrity; actual candidate results above use
the recorded runtime artifacts and full raw channels.

The [C source binding](../reports/m2/stage68-source-binding.json) links 167,307
recorded inputs to implementation commit `723687b`, with its remote `main`
identity verified. It independently checks the 607 copied inputs and 315 frozen
runtime/Cargo files, preserves post-validation document changes, and retains
actual publication, assembly, commit, and push records. Its 6,832,706-byte archive
passed complete member readback. The actual C publication process and raw
channels are separate post-archive records. This provenance audit adds no guest
evaluations, builds, or compatibility passes.

## Stage 67 — String properties and full ready-corpus comparison

The shared runtime implements String `isBlank`, `isNotBlank`, `isNotEmpty`, and
`lastIndex` alongside `length` and `isEmpty`. Blankness uses Pkl's exact 25-character
UTF-16 whitespace set; the last index counts code points. Explicit access,
implicit constraints, synthetic Reference properties, and first-error ordering
are covered by 146 pinned Java expressions in two fresh JVM sessions.

The [implementation checkpoint](../reports/m2/stage67-implementation-checkpoint.json)
records all five required Rust checks, 1,008 distinct passing tests, zero failures,
and two ignored tests. The 15-profile catalog contains 1,227 expressions; native,
actual N-API, and Chromium each match 2,454 raw observations. The separate Any
comparison and M1 experiments retain their two Regex mismatches per portable target.

The complete ready selection has the following raw Java/Rust outcomes:

| Adapter | Selected | Matches | Other outcomes |
| --- | ---: | ---: | --- |
| Expression strings | 3,527 | 3,516 | 10 mismatches, one Java oracle error |
| Original modules | 888 | 625 | 118 mismatches, 143 unsupported, one Java oracle error, one Rust internal error |
| JVM classpath output | 1 | 1 | None |

Acquisition integrity passed with stable inputs, sources, and artifacts. Strict
`--verify-full` exited 1 with 1,802 unmet gate entries; those entries overlap and
are not a count of distinct features. The other 111 inventory cases remain
unimplemented or unmapped. All 131 previously selected originals retain their
statuses, including all 81 prior passes.

Three inherited scalar regressions remain failed: `reference-stage10-reference-amendment-invalid`,
`types-stage18-default-listing-literal`, and `set-stage23-constructor-listing-default`.
Corrections tested in the separate Stage 68 lab are not results for this frozen
candidate. The 13 published archives preserve the raw failures, corrected review
attempts, and source identities. These are local macOS arm64 results; all nine M2
workstreams, full language/API compatibility, release platforms, and performance
goals remain open. The [C source binding](../reports/m2/stage67-source-binding.json)
links this evidence to implementation commit `90b1580`, whose remote `main` identity
was verified. It checks 165,550 recorded inputs, independently checks the nine
copied inputs and all 293 runtime/Cargo blobs, and preserves the three captured
documents changed after validation. No guest evaluations or builds were rerun
by that provenance audit.

## Stage 66 — reversal and union diagnostics

Stage 66 implements shared UTF-16 string reversal and shallow List/Set reversal,
including Java surrogate-pair repair, ordered representatives, canonical empty
List results, and nonforcing List bounds diagnostics. Base constructor calls in
custom-this constraints preserve lexical method precedence. Union default-marker
errors retain the first type's full span and reject a second marker before the
following type is parsed. Scalar missing-member diagnostics use pinned member
catalogs.

All five required Rust checks pass. The workspace has 1,004 distinct passes,
zero failures, and two ignored tests; 1,017 raw pass summaries include thirteen
child reruns. The fixed runtime/Cargo cohort contains 291 source files.

The [implementation checkpoint](../reports/m2/stage66-implementation-checkpoint.json) records fourteen single-module Java
profiles: 1,081 expressions and 2,162 observations on each of native, actual
N-API, and Chromium. All seven common raw channels and whole process protocols
match. Forty separate union-marker modules also match in eighty fresh sessions
per target. Their module, evaluator, and producer metadata boundaries remain
separate from the single-module catalog and the eight direct Java VM identity
observations.

The original selection preserves the previous 130 programs and adds
`api/stringUnicode.pkl`. Its 131 outcomes are **81 matches, 18 mismatches,
31 unsupported results, and one Java heap error** at the unchanged `-Xmx512m`.
All 74 prior passes remain passes; seven prior mismatches now match. The added
Unicode original fails at `String.isBlank` before reaching its reversal checks.
Two package/HTTP/TLS originals remain required and unexecuted; the selected
original gate still fails.

IntSeq, Bytes, Duration, and DataSize retain 474, 600, 548, and 510 matches per
target. Any retains 170 matches and two Regex mismatches per portable target;
M1's architecture contract also preserves its two Regex compatibility failures.

The browser HTTP verifier now checks complete sessions, resource/status pairs,
and source-defined loading dependencies while preserving raw completion order.
The first Stage66 offline comparison stopped at a Node launcher path check.
A separately versioned amendment proves that the recorded symlink and resolved
Node executable have the same captured binary identity before applying the
original comparisons. It passes a temporary locator metadata copy to that
function; captured records and guest bytes remain unchanged. Its adversarial
tests and the subsequent 39-command comparison retain all other source,
artifact, HTTP, process, and raw-channel checks. Earlier failures are preserved.

These are local macOS arm64 results. All nine M2 workstreams, full language and
product API compatibility, other release platforms, and performance goals
remain incomplete. The [Java authority checkpoint](../reports/m2/stage66-java-checkpoint.json) is committed as `c7eb4b6`.

## Stage 65 — contextual construction

Stage 65 connects contextual and explicit `new` to the shared resumable
type-default engine, including method-parent caching, inherited module self
types, receiver-sensitive aliases, default-key calls, and missing-default frames.
Canonical constructor parents remain traced roots owned by their code. Dedicated
first-use, failure/retry, retirement, and close tests check their ownership;
lifecycle tests measure user-object deltas after warming those canonical roots.
Collection equality continues to compare collection members independently of
builtin prototype backlinks.

The [implementation checkpoint](../reports/m2/stage65-implementation-checkpoint.json)
records ten Java profiles: 940 expressions and 1,880 observations on each of
native, actual N-API, and Chromium. All seven raw channels and whole process
protocols match. Five inferred-construction fixtures also retain separate native,
ordered-module, diagnostic, Host, identity, and frontend verification boundaries;
their multi-module cases are not counted as portable single-module coverage.

All five required Rust checks pass. There are 991 distinct workspace passes,
zero failures, and two ignored tests; the raw 1,004 passes include thirteen
nested child reruns. Tool checks pass 128 Python tests in both normal and `-O`
modes, 50 JavaScript tests, eight mapping tests in both modes, and inventory
freshness. Earlier failed candidates and build/storage failures remain recorded.

The unchanged original selection expands to 130 programs: 74 matches,
24 mismatches, 31 unsupported results, and one Java heap error at the unchanged
`-Xmx512m`. All 52 previous passes remain passes; ten prior nonpasses and twelve
of fourteen added programs now match. Two package/HTTP/TLS originals remain
required and unexecuted. The selected original gate still fails. Remaining
failures include the diagnostic categories/spans for `errors/notAUnionDefault.pkl`
and `errors/multipleDefaults.pkl`. Follow-up implementation also targets
`String.reverse()` and collection reversal, identified by inspecting the source
at the remaining renderer-program failures.

The first portable comparison stopped at a verifier that required a fixed
browser resource-completion order. Its failed record and ordered HTTP logs are
preserved. A source-bound post-capture amendment admits only complete sessions
with the exact resource/status pairs and the loading dependencies expressed by
the capture code. It changes no captured tool or guest bytes. Six amendment
tests pass in normal and `-O` modes; the subsequent 29-command offline comparison
retains every other source, artifact, Java, and raw-channel check.

IntSeq, Bytes, Duration, and DataSize retain 474, 600, 548, and 510 matches per
target. Any retains 170 matches and two Regex mismatches per portable target;
M1's architecture contract also preserves its two Regex compatibility failures.
These are local macOS arm64 results. All nine M2 workstreams, full language and
product API compatibility, other release platforms, and performance goals
remain incomplete.

Implementation commit `6f41ceda1708c5dcdce20faf7030233063bf08c1` is pushed.
The [source binding](../reports/m2/stage65-source-binding.json) links 165,390
recorded Git inputs and 69 copied inputs to that commit and the pinned references.
Two historical planning/verification documents retain their original bytes and
separate identities. All 280 frozen runtime/Cargo files match actual commit blob
bytes. The archive also preserves the later publication and assembly receipts,
including failed attempts. The binding's own terminal receipt and raw channels
are published separately because they postdate its archive. This audit performs
no builds or guest evaluations and leaves every compatibility gap unchanged.

Stage 64 implements repeated amendment bodies, class-valued construction
parents, empty-result identity, and `Listing.fold` forcing order. Its
[implementation checkpoint](../reports/m2/stage64-implementation-checkpoint.json)
records nine independent Java profiles: 868 expressions and 1,736 observations
on each of native, actual N-API, and Chromium. All seven raw channels and the
whole process protocols match. Declaration modules, direct VM identity, Host
traces, frontend controls, and the separate 39-row prototype-diagnostic fixture
retain their own verification boundaries.

All five required Rust checks pass, with 972 distinct workspace tests passing,
zero failures, and two ignored tests. The raw harness sum of 985 includes
thirteen child summaries duplicated by parent tests. The Stage 64 count audit
also corrects Stage 62 to 956 and Stage 63 to 963 distinct passes; their original
raw archives and historical checkpoint JSON remain unchanged. Earlier semantic,
disk-space, and damaged-build-artifact failures remain recorded separately.

The unchanged original selection expands to 116 programs: 52 matches,
30 mismatches, 33 unsupported results, and one Java heap error. All 37 prior
passes remain passes; `generators/spreadSyntaxDynamic.pkl` and
`basic/amendsChains.pkl` now match. Of 25 added programs, thirteen match,
eleven mismatch, and one is unsupported. Two further package/HTTP/TLS cases
remain required and unexecuted. The selected original gate fails. The first
new mismatch, `basic/newInAmendingModuleMethod.pkl`, reaches a missing
unqualified `getClass` call in Rust and remains a follow-up implementation task.

Existing IntSeq, Bytes, Duration, and DataSize comparisons retain 474, 600,
548, and 510 matches per target. Any portable retains 170 matches and two
Regex mismatches per target, and M1 retains its two Regex compatibility gaps.
All nine M2 workstreams remain open. These are recorded local macOS arm64
results, with full language, product API, release platform, and performance
requirements still incomplete.

Implementation commit `1afd1648daba3b31fe204bf1974fcb0b6fb2fb03` is pushed.
The [source binding](../reports/m2/stage64-source-binding.json) and
[independent review](../reports/m2/stage64-source-binding-review.json) connect
the actual Git contents to the captured sources while preserving archived
execution tools and two historical document inputs as separate categories.
The successful binding and review process receipts are published separately
from the archive that precedes them. No evaluations are rerun by this audit.

Stage 63 adds strict List/Set concatenation, Map merging, and Set endpoint
properties to the shared VM. Its
[implementation checkpoint](../reports/m2/stage63-implementation-checkpoint.json)
records eight independent Java string profiles: 680 expressions and 1,360
observations on each of native, actual N-API and Chromium. All seven common raw
channels and complete process protocols match. List ordering, Set duplicate
representatives, Map replacement order, empty-right identity and nonforcing
operand diagnostics have dedicated cases. Host suspension, reentry, GC,
cancellation and retry retain the pending operands and builder.

All five required Rust checks pass, with 963 distinct workspace tests passing
(corrected by the Stage 64 count audit from the earlier reported 971),
zero failures and two ignored tests. IntSeq, Bytes, Duration and DataSize retain
474, 600, 548 and 510 exact observations per target. Any portable still has
170 matches and two Regex mismatches per target; its strict gate fails. The
M1 architecture contract also retains its separate two Regex compatibility gaps.

The unchanged original selection expands to 91 programs: 37 matches,
21 mismatches, 32 unsupported results and one Java heap error. Among the prior
90, `basic/set.pkl` changes from mismatch to pass and all 35 previous passes
remain passes. The newly selected `basic/map.pkl` also passes. Java
`api/list.pkl` still fails at the recorded `-Xmx512m`; two package/HTTP/TLS
originals remain blocked. All nine M2 workstreams and the strict full-language
gate remain open.

An earlier platform acquisition used an absolute Python launcher where the
offline verifier requires literal `python3`. Its failed comparison preflight
and original captures remain preserved. A fresh platform acquisition satisfies
the unchanged command and raw-comparison contracts. These are local macOS
arm64 results, not hosted CI or complete product/platform/performance evidence.

Stage 62 connects strict object materialization, lazy object/collection adapters,
selector-based maps, Dynamic property queries, Listing/Mapping optional and
defaulted lookup, Dynamic length, Set addition, and Bytes iteration/spread to
the shared VM. Its [implementation checkpoint](../reports/m2/stage62-implementation-checkpoint.json)
records six independent Java authorities with 485 expressions. Every native,
N-API, and Chromium target matches all 970 observations across two fresh
sessions, comparing seven raw channels and the complete process protocol.
UTF-16 channels distinguish lone surrogates from U+FFFD even when Java UTF-8
replaces them with the same `?` byte. Typed exports and complete wrapper
identity remain separate requirements.

Conversions preserve shallow forcing order, inherited declaration order,
duplicate-key position, receiver-sensitive fallback, and explicit null values.
`getOrDefault` checks the inherited default only when a missing key requires it.
Source, builder, callback and pending values stay rooted through Host
suspension, reentry, GC, cancellation and retry. Authored quoted identifiers
can contain NUL: a rejected earlier candidate collided with synthetic property
names. The corrected namespace uses LF, which authored identifiers cannot
contain, and also encodes LF-bearing String keys injectively. Earlier passing
matrices and Rust checks do not validate the corrected source automatically;
the final candidate has separate recorded workspace and platform checks.

IntSeq, Bytes, Duration and DataSize retain 474, 600, 548 and 510 matches per
target. Any portable retains 170 matches and two Regex mismatches per target;
its strict gate remains failed. The expanded original selection has 35 matches,
22 mismatches, 32 unsupported results and one Java heap error among 90 programs.
The prior 62 improve from 26 to 30 matches with no pass regression. The four new
matches are `elementGenerators.pkl`, `spreadSyntaxListing.pkl`,
`spreadSyntaxMapping.pkl`, and `objects/hashCode.pkl`. The prior 17 still have
15 matches; their two Binary-path failures now reach unsupported `pkl:pklbinary`.
The Java `api/list.pkl` run fails at the recorded `-Xmx512m`; it is not a pass.
Two further required package/HTTP/TLS cases remain unexecuted. All nine M2
workstreams and the strict full-language gate remain open.

Stage 61 adds shared `IntSeq` values and the prerequisites of unchanged original
programs. Its [implementation checkpoint](../reports/m2/stage61-implementation-checkpoint.json)
records all 474 selected string observations matching on each of native,
N-API, and Chromium. Bytes now matches all 600 observations per target;
Duration and DataSize retain 548 and 510 matches. Portable Any improves to
170 matches and two retained scalar Regex mismatches per target. Earlier
Stage 60 failures below remain historical evidence rather than current passes.

The immutable sequence carrier preserves start, end, step, aligned last value,
and reference identity. Java's method/spread iterator stops at the aligned last
value, while its `for` generator uses a wrapping count derived from
`abs((end - start) / step) + 1`; extreme integer inputs distinguish these paths.
Empty sequences compare equal and hash to one; nonempty equality and hashing
use start, aligned last, and step. Rendering retains the original end. IntSeq
inherits Any rather than Collection, so collection length/indexing/addition
are not exposed. Map, fold, and conversions share resumable VM continuations,
with callback, accumulator, and synthetic Listing environments retained by GC.

The [Java authority](../reports/m2/stage61-java-checkpoint.json) separates 237
string expressions, three typed-export rejection observations, 17 math constants,
and 21 List.add expressions. The [List supplement](../reports/m2/stage61-list-accessor-fold-java-checkpoint.json)
adds 65 accessor/fold expressions. Math strings and internal Rust numeric bits,
List prerequisites, and Host/GC lifecycle tests have focused native regressions;
they are not separate typed product API or portable supplement comparisons.
The pinned `pkl:math` source is bundled with all 17 constants; its external
functions remain explicitly unsupported.

The expanded original cohort preserves the prior 17 programs and every one of
the 47 pinned input snippets containing `IntSeq`, with two overlaps. It has
26 matches, 15 mismatches, and 21 unsupported results across 62 programs.
The prior cohort now has 15 matches and two Binary-path mismatches;
`basic/bytes.pkl`, `api/bytes.pkl`, and `api/duration.pkl` newly match, with
no previously passing case regressing. The original basic/API IntSeq programs
also match. Each original program ran once per runtime; offline verification
does not add executions. Remaining renderer, collection, builtin, and diagnostic
failures keep the selected and full gates failed. Inventory readiness, passing
focused tests, and stage numbers are not whole-language completion percentages.

Stage 60 adds shared `Bytes` operations and resumable `List.toBytes` conversion.
The [Java authority](../reports/m2/stage60-java-checkpoint.json) contains 300
ordered expressions in two fresh JVMs, with 223 successes and 77 ordinary Pkl
errors per session. Native, N-API, and Chromium each match 594 of 600 common
string observations. `list-intseq-all`, `list-intseq-hash`, and
`value-all-intseq` fail in both repetitions; their strict gates remain failed.
The full fixture retains these rows. At Stage 60, one ignored Rust regression
preserved all three known failures separately from the 297 passing focused cases.

Selected operations cover length/size, Base64, hex, MD5, SHA-1, SHA-256,
`sha256Int`, concatenation, indexing, `getOrNull`, `toList`, and strict UTF-8,
UTF-16, and ISO-8859-1 decoding. Java directly widens a signed byte for Bytes
indexing and `getOrNull`; its List conversion produces unsigned values.
`sha256Int` reads the first eight digest bytes as a signed little-endian integer.
The shared engine preserves these distinctions and first-error diagnostics.
Heap-local cache links retain the originating List through Host suspension and
GC; release clears those links. Value/hash observations do not establish Java
wrapper identity or mutable exported-array aliasing.

Comparisons require seven common raw channels, including UTF-16 code units,
and the complete session protocol. Java exception-class, exception-string, and
cause channels remain additional producer observations; the candidate string
boundary does not synthesize them. Portable capture requests the eleven
environment keys recorded by Java. Effective `en-US`/`UTC` comes from both
actual JVM session command lines, separately from the preflight `ko-KR`
properties. Actual Node environment and browser locale/timezone observations
are recorded. The Java capture does not record its complete OS-injected
environment, so complete process-environment identity is not claimed.

The [implementation evidence](../reports/m2/stage60-implementation-checkpoint.json)
also preserves unchanged original programs: 12 matches, four mismatches, and
one unsupported result among 17. `errors/invalidBytes4.pkl` newly matches;
`basic/bytes.pkl` and `api/bytes.pkl` reach missing `IntSeq` calls. Binary tests
still fail at `module.toMap()` before encoding, and the Duration API program
still requires `pkl:math`. Typed exports, full M2, product APIs, and performance
remain separate requirements.

The [DataSize contract](m2-data-size.md) separates shared value operations and
raw native/Node/Chromium string comparisons from required typed exports,
remaining Bytes behavior, and unchanged original-module failures.

The [Any.ifNonNull contract](m2-any-if-non-null.md) records shared VM lookup,
callback and null-default behavior, independent Java string authority, native
and portable raw comparison gates, and the remaining constructor gaps.

Stage 59 connects `Duration` to the shared value, VM, type, diagnostic, and
rendering paths. Its seven units are `ns`, `us`, `ms`, `s`, `min`, `h`, and `d`.
The pinned Java implementation converts as `value * sourceNanos /
targetNanos`; even a conversion to the same unit can overflow an intermediate
double. Addition and subtraction select the coarser unit. Ordering uses
`Double.compare`, while equality and hashing use values converted to
nanoseconds, with Java's distinct NaN and signed-zero behavior. Set and Map
observations include the transition from four to five keys. `isoString` uses
`value * (sourceNanos / 1e9)` and the pinned Java formatting and finite-value
checks; huge finite inputs can therefore format unusually or fail after
conversion. These details are required outcomes, including first-error order
for `isBetween` and argument/type failures.

The [sealed Java Duration string fixture](../tests/fixtures/m2-duration-values/README.md)
contains 274 ordered expressions in each of two fresh JVM sessions: 240
successes and 34 ordinary Pkl errors per session. A separate
[15-row Java exported-value supplement](../tests/fixtures/m2-duration-values-bits/README.md)
records raw double bits and units, also twice. Native, N-API, and Chromium
string comparisons must check complete status, expression, stdout, stderr,
UTF-16BE channels, and ordered process protocol without normalizing errors.
The Java exception class is producer metadata. Rust internal raw-bit tests
use the exported-value observations as authority; they do not test a typed
Rust, Wasm, or N-API Duration export. Typed product APIs and the original
upstream `basic/duration.pkl` and `api/duration.pkl` suite executions remain
separate obligations. The [Java checkpoint](../reports/m2/stage59-java-checkpoint.json)
records that the canonical source snapshot includes unrelated `reference.pkl`
files in place of those original Duration programs; their bytes are captured
in the separate validation source snapshot. The authored Java calls never used
the original snippet programs as inputs. Java evidence integrity is distinct
from a candidate compatibility result.

The [Class-value contract](m2-class-values.md) records declaration/prototype
identity, superclass and converter rules, and independent Java observations.
Common query/logger comparisons and the required provider/export supplements
have separate verdicts; a matching selected Class matrix does not close M2.

The [method-deprecation contract](m2-deprecation.md) adds independent Java logger
observations, including implicit method calls and sequential evaluator lifetimes.
Its reference checkpoint and ongoing candidate implementation do not close the
whole-language requirements or the strict completion gate below.

M2 extends the shared Rust engine established in M1. Its completion gate covers
the language scope in [PLAN.md](../PLAN.md), including classes, generators,
constraints, generics, `Reference`, and complete diagnostics. The first executable
selection exercises only part of that scope.

The [trace development contract](m2-tracing.md) separately records actual-source
logger callbacks, source-session lifecycles, and lossless platform transport.
Its focused comparisons do not replace the whole-language completion gate.

The [object-body parameter contract](m2-object-parameters.md) records function
amendment, Mixin defaults, lexical capture, and their Java-first fixtures.
These comparisons retain their own profiles. The native filesystem probes now
accept complete expressions; the legacy portable scalar boundary remains bounded.

The [resource-read development contract](m2-resource-reads.md) records `read` and
`read?` parsing, default scalar resource policy, nullable absence, declaration
source URIs, and the distinction between Java byte captures and lossless
portable string comparisons. Permitted I/O readers and complete URI semantics
remain open.

The [original module contract](m2-module-frontend.md) connects unchanged upstream
module commands to shared preparation and actual output evaluation. Its native
process comparisons retain the original arguments, resources, and physical
source path. The frontend-only API retains its unsupported output boundary;
portable comparisons select actual output separately with `output-bytes`.
Original setup-dependent cases remain incomplete when the observed Java outcome
does not satisfy the original expected exit status, even if raw channels agree.
The generated original-module registry includes all 888 original-ready snippets;
registration does not establish that they execute successfully. The explicit
`upstream-original-modules-v1` workspace profile retains complete pinned reference
trees while omitting unrelated project fixtures. Its filesystem boundary and
source inventory digests are recorded in schema-5 evidence, separately from
historical acquisitions using the default workspace.

The [original JVM-classpath profile](m2-jvm-classpath.md) separately compares the
unchanged `basic/import1b.pkl` input through the upstream JVM API/classloader
boundary. Its explicit reviewed execution mapping keeps the historical blocked
CLI corpus contract while requiring two successful raw JVM-profile repetitions
for that same original case ID in the selected/full M2 runner.

`compatibility/m2-language.json` keeps semantic families, pinned source
declarations, upstream language snippets, selected historical regressions, and
unmapped historical candidates visible. Declaration and heading counts are
inventory counts, not semantic coverage. An implemented parser node does not
establish its runtime behavior.

Some accounting rows intentionally overlap: selected rev1 execution contracts
also have historical candidate rows, parser declarations can share names, and
language-reference headings can cover the same behavior. Totals must not be
presented as distinct feature or test counts. Semantic mappings require review.

`compatibility/m2-mappings.json` is the reviewed input for linking existing
requirement IDs to Rust implementation symbols and existing case IDs. Its
baseline must match the pinned inventory. The manifest generator validates
source paths and symbols, records current source hashes, adds reverse case
links, and preserves previously selected supporting cases. A source symbol's
presence establishes provenance, not semantic completeness.

Each reviewed entry states the supported scope and remaining work. Partial
implementation or test mappings continue to fail the full gate. Completed
mapping labels still require all linked cases to execute and pass. Linking an
original upstream ID does not change its input, adapter, execution state, or
output boundary. Selected reproductions therefore cannot silently replace the
original snippet contract. Missing IDs, duplicate links, source drift, wrong
baselines, and contradictory scope claims fail validation. The reviewed entries
link bounded Reference, modifier, class, import, metadata, and diagnostic
implementations to selected cases. Every entry remains partial and records its
remaining scope.

```sh
python3 tools/m2_module_eval.py --check
python3 tools/m2_manifest.py --check
python3 tools/m2_identifier_tables.py --output target/m2/identifiers-new
python3 tools/m2_unicode_escape_tables.py --output target/m2/unicode-escapes-new
python3 tools/m2_uppercase_tables.py --output target/m2/uppercase-new
python3 -m unittest discover -s tools/tests -p 'test_m2_*.py' -v
python3 tools/m2_language.py --output target/m2/language-new --record-only
python3 tools/m2_language.py --verify-report target/m2/language-new
python3 tools/m2_language.py --verify-development target/m2/language-new
python3 tools/m2_language.py --verify-subset target/m2/language-new
python3 tools/m2_language.py --verify-full target/m2/language-new
```

Use a fresh output directory. By default, the runner acquires both expression
comparisons and registered original module commands. It records a release build
of `language_probe` and `module_eval_probe`, freezes their executables, compiles
the expression Java probe against the pinned oracle, and retains source snapshots
and raw process captures. `--adapter language-api-string` and
`--adapter language-module-eval` select a focused acquisition; omitted required
cases remain incomplete in the full gate. `--candidate` and `--module-candidate`
are explicit artifact overrides whose source bindings are unverified.


For internally built candidates, `--cargo-target-dir` selects a fresh Cargo target
directory separate from the evidence output and all workspaces. The directory
must not already exist or traverse symlinks, and at least one selected candidate
must be built internally. For example:

```sh
python3 tools/m2_language.py --output target/m2/language-new --cargo-target-dir target/m2/language-cargo-new --record-only
```

Supplied candidates remain `unbound-caller-supplied`, including when other
selected candidates use the separate recorded build. This option does not relax
`--verify-full` source-binding or completion requirements.


Expression acquisition defaults to one worker. `--workers 4` runs up to four
expression cases concurrently; the accepted range is 1–32. Original module
commands run sequentially in their shared physical workspace. Every case retains fresh Java and Rust processes,
separate raw captures and the pinned environment. Results remain in manifest
order, including failures and the independent remainder-error repeat. The report
records the worker count. This option measures acquisition throughput; evaluator
performance and memory comparisons require the separate benchmark contract.

The expression adapter's Java boundary is `evaluateExpressionString` with a file module and an
expression. Both processes receive the same absolute source path. Successful
UTF-8 stdout, full uncolored `PklException.getMessage()` stderr, and exit codes are
compared without normalization. A separate outcome file records whether Java
returned normally or caught a particular `PklException` class. This distinguishes
a handled upstream bug report from an uncaught JVM failure without changing
stdout or stderr. The native Rust probe passes the complete original expression
to the shared module evaluator, which applies the shared `ToString` operation and
the special `output.text` operation. Native Host helpers supply file loading;
noop Logger, default frames and empty environment/property maps match this profile.
Selected collection stringification reads completed values without forcing nested
objects. Map uses the single-line layout and List/Set the multiline layout of the
Java value renderer.
Java may rehash nested Map keys during iteration; those rendering side effects
are not fully reproduced by the cache-only renderer.
This boundary does not prove typed API compatibility or general
compound-value rendering.

Language strings retain UTF-16 code units internally, including unmatched
surrogates. Literal types, keys, concatenation, interpolation and the implemented
collection `toString` paths preserve those units. The development probes apply
Java's UTF-8 writer replacement only when producing their output. Matching that
byte boundary does not establish lossless public JavaScript string transport.
Unmatched-surrogate module/resource URIs and Java's out-of-range escape internal
error remain separate gaps. Diagnostic member paths retain these units through
scope counters, runtime frames and recursive compression, before the Java UTF-8
writer conversion. Selected nested entry/function numbering and compression
controls verify this identity separately from displayed bytes. The complete
string API and diagnostic corpus still require further verification.

`String.toUpperCase()` uses a generated table from the pinned JDK's
`String.toUpperCase(Locale.ROOT)`, rather than Rust's Unicode case tables.
The generator captures all 1,114,112 code-point inputs as raw UTF-16, including
unmatched surrogate units, and retains full multi-unit expansions. Its default
mode checks exact generated bytes; `--write` updates the table after reviewing
the oracle evidence. The runtime decodes valid surrogate pairs and preserves
unmatched units. This uppercase component and selected VM calls do not establish
the remaining string builtins or regular-expression compatibility.

Numeric value-frame cases compare direct negation and grouping across typed
properties, method returns and collection members. Typed locals and function
bodies use the executable expression's source span; other constant members retain
Java's source suffix after their header. Authored syntax and assertion observations
remain separate. Minimum integers, radix literals, signed zero, scalar constants
and nonliteral unary operations provide value and diagnostic controls.

Rejected non-local object property type annotations retain their authored member
paths, including nested objects, functions, let scopes, entries and generators.
The selected cases compare the complete rejection bytes and accepted module,
class and object-local controls. Diagnostic naming preserves UTF-16 identity,
literal entry labels and scope-local ordinals. Const-modifier errors retain their
enclosing owner, while inferred-parent errors retain the expression owner.
Method bodies infer contextual `new {}` from return-type defaults, including
classes, collections, aliases, nullable types and defaulted unions. Lambda and
parenthesized contexts retain their rejection. Inherited property prototypes
use declared type defaults; instance amendments read prototype initializers.
Literal `unknown`, aliases of `unknown`, and `Any` keep distinct default rules.
Selected cases cover lazy unused methods, first errors, and generated default
frames. Nested-negation overflow frames and unavailable source sections remain
exploratory differences.

The Stage 34 selection retains two failing `Dynamic.toString()` calls and adds
separate `is Dynamic` controls. The original failures are neither replaced nor
added to the development exceptions, so the development gate also fails until
they are resolved. The generic portable comparison consumes a fixture module
exporting `cases` and `context`, validates source bytes and hashes against the
selected files, and executes each case twice on Node and Chromium. It records
complete value/error bytes and returns nonzero for any mismatch. Counts come
from the supplied cases, rather than a fixed stage denominator. It complements
the existing platform scenarios:

```sh
node tools/platform/compare-language-cases.mjs target/m2/annotations-platform-new target/m2/platforms-new/wasm target/m2/platforms-new/pkl_napi.node tools/platform/annotation-cases.mjs
node tools/platform/compare-language-cases.mjs target/m2/method-parents-platform-new target/m2/platforms-new/wasm target/m2/platforms-new/pkl_napi.node tools/platform/method-parent-cases.mjs
```

The historical Stage 34 wrapper remains available:

```sh
node tools/platform/compare-method-parents.mjs target/m2/method-parents-wrapper-new target/m2/platforms-new/wasm target/m2/platforms-new/pkl_napi.node
```

Each fixture supplies its source, source hash, property-path expression, and
exactly one complete Java-derived `expectedValue` or `expectedError`. The context
records URI, module name, power-assertion mode, and oracle acquisition identity.
The runner preserves fixtures, runtime and artifact identities, raw payloads,
and missing or failed executions. The fixture's Java acquisition and the portable
execution are separate evidence; creating a fixture or documenting the command
does not establish that a platform run passed.

Stage 35 adds declaration annotations on modules, classes, aliases, properties,
and methods. Selected comparisons distinguish bare construction, lazy fields,
eager generators and spread, const scope, type validation, first-error order,
and partial publication after initialization failure. After resolving its
signature, a method reads the message of the first annotation whose class is
exactly `Deprecated`. Unused
class and alias declarations remain lazy, and failed ordinary member reads
retain their retry behavior.

Builtin controls cover `Annotation`, `Deprecated`, `Since`, `Unlisted`,
`AlsoKnownAs`, `DocExample`, and `SourceCode`. Nullable string defaults and empty
`Listing<String>` defaults are distinct from required fields. In particular,
`SourceCode.language` has no implicit Go default: its complete union accepts
the listed literals and other strings, but a missing value remains undefined.
Collection element checks stay lazy and retain the standard-library declaration
frame when forced. Untyped parameterized annotation generators remain explicitly
unsupported: the pinned Java implementation fails internally when its diagnostic
location is null. This is an oracle error, not a matched language error. Original
exploratory parser failures and Java internal errors remain recorded separately
from language-error comparisons.

Full reflection and builtin annotation metadata remain open. The upstream
`@SourceCode` metadata on `Deprecated.replaceWith` is not synthesized.
`ModuleInfo`, converters, deprecation logging, module initialization/cache
semantics, general object `toString`, and `List.map` remain dependencies.
The original object-stringification and mapped-list failures retain their raw
inputs and outcomes alongside independent controls. Authored scalar fixtures do
not replace the original upstream annotation snippets or their renderer contract.

Stage 36 adds object force/string comparisons for `Dynamic`, `Listing`, `Mapping`,
typed objects and modules. Cases cover child-first forcing, inherited render order,
undefined members, hidden declaration owners, strict collections nested inside
objects, virtual `toString` dispatch, equal entry keys and guaranteed String-key hash
collisions. Portable cases preserve fresh Java results for native, Node and browser
comparison, including full errors. The older property-key case named as a collision
is an unequal-key control: its Identifier hashes do not prove a numeric collision.

Seven additional persistent definitions retain their original inputs. Six require
`trace`, which remains unsupported; failed setup is recorded rather than replaced
with trace-free input. The trace-free module-self sequence tests cached string
results in one evaluator. Actual-source Host comparisons separately cover ordinary
forcing, callback errors and nested callbacks. Matching expected error payloads
does not override extra callback/protocol differences or Java internal errors.
Diagnostic formatting that needs Host reads during key hashing remains a failed
comparison. These obligations stay outside claims about the passing scalar cases.

```sh
node tools/platform/compare-language-cases.mjs target/m2/object-string-platform-new target/m2/platforms-new/wasm target/m2/platforms-new/pkl_napi.node tools/platform/object-string-cases.mjs
```

The default uses Java's API setting with power assertions disabled. Selected
cases can opt in with `"power_assertions": true`; the runner then passes
`--power-assertions` to both probes and verifies that option in the recorded
commands. These cases compare the complete value graph, message, hints, and
frames as raw bytes. Enabling this profile does not change historical captures
or the default mode of other cases.

The commands have distinct meanings:

- `--verify-report` checks evidence integrity and recomputes recorded outcomes,
  including failures.
- `--verify-development` requires selected cases to match except for the explicitly
  tracked cycle diagnostic failure and integer remainder-by-zero upstream bug.
  Both remain failed in the report.
- `--verify-subset` additionally requires every selected comparison and
  metamorphic relation to pass.
- `--verify-full` additionally requires all language requirements to have
  implementation and test mappings, every required case to execute successfully,
  and the candidate to have a recorded build.

Unknown, unimplemented, unexecuted, unmapped, and mismatching requirements remain
failures of the full gate. Selected-case success and source inventory freshness
must never be reported as M2 completion.

CI acquires evidence with `--record-only`, which returns success only after a
complete, internally consistent acquisition, regardless of compatibility outcomes.
It then applies the named development contract. An optional full-corpus workflow
also runs the strict full gate.

The initial cyclic property probe exposes an oracle limitation: cold Java calls
can fail while initializing `VmStackOverflowException` on an exhausted stack.
A separate diagnostic experiment preinitializes that class without changing the
JAR; it produces controlled Pkl errors, but the rendered repetition count varies
across runs. Both conditions and all raw bytes are retained in
`reports/m2/cycle-evidence.tar.gz`. Preinitialization is not applied to the normal
oracle, and no repetition counts are normalized. This unresolved case prevents
strict selected-case completion.

For integer `1 % 0`, the pinned Java implementation reports a caught
`PklBugException` caused by `ArithmeticException` in `VmSafeMath.remainder`.
The development exception is limited to that exact input and failure shape;
it additionally requires a separate cold Java capture with identical complete
raw bytes. The Rust diagnostic must retain its recorded complete division-error
form. No stack traces are normalized or treated as matches. A changed failure,
timeout, or different output fails the development gate. Resolving the raw
diagnostic difference is still required by the strict selected and full gates.

Runtime diagnostics carry authored source spans and logical frames separately
from their message. Instruction spans are parallel metadata, preserving the
bytecode dispatch count. The text renderer is shared runtime code; native probe
code supplies source URIs and the expression boundary frame. The same
experimental scalar boundary is available as `evaluateLanguage` in the Node
and Wasm development bindings, including complete diagnostics. It accepts a
source string, URI, inferred module name, and scalar property path; explicit
module declarations take precedence over the inferred name.
An optional fifth Boolean argument enables power assertions, with the same
disabled default as the native probe and Java API. The portable scalar facade is
separate from the native complete-expression adapter introduced in Stage97.
Returned Reference values also pass through their domain renderer, matching the
Java expression-string boundary's automatic conversion and its separate error
stack. This adapter still accepts property paths only. It does not supply Host
I/O, general compound-value conversion, or the full expression API.

The generated identifier tables reproduce the pinned lexer's UTF-16 code-unit
predicates. `m2_identifier_tables.py` records the raw Java intervals and checks
the generated Rust bytes. CI repeats this check against the pinned JDK.

Unicode escape scanning and conversion use a separate generated table.
`m2_unicode_escape_tables.py` records all BMP UTF-16 units accepted by Java's
`Character.isLetterOrDigit` and their `Character.digit(unit, 16)` values, then
verifies the generated Rust bytes. Its evidence includes the pinned JDK,
probe sources, raw output, and source stability. Use a fresh output directory;
`--write` regenerates the table after reviewing a deliberate predicate change.
These predicates do not establish complete Unicode string compatibility.

M1's platform and Host contracts remain regression requirements. New language
features must retain captured values across VM suspension and collection and use
the same engine in native Rust, browser Wasm, and Node N-API.

## Persistent class initialization sequences

Independent expression processes cannot verify state retained after an error.
The same contract also covers type-alias publication and function metadata;
the historical tool and directory names remain stable.
The separate [sequence manifest](../compatibility/m2-class-lifecycle/manifest.json)
declares ordered property-path calls within one evaluator and module. Each
sequence starts with a fresh evaluator and repeats in a second fresh evaluator;
errors within a sequence do not reset its evaluator. Fixture helper properties
contain the expressions being observed. The Java and Rust adapters use the same
source bytes, fixed `file:///m2-platform.pkl` URI, and disabled power assertions.

```sh
python3 tools/m2_class_lifecycle.py --output target/m2/class-lifecycle-new --record-only
python3 tools/m2_class_lifecycle.py --verify-report target/m2/class-lifecycle-new
python3 tools/m2_class_lifecycle.py --verify-contract target/m2/class-lifecycle-new
```

The runner retains source snapshots, the recorded Rust build and frozen
executable, Java classes and oracle identity, ordered process markers, and each
call's complete status/stdout/stderr bytes. Java exception class names are
provenance metadata; they are not compared to Rust implementation type names.
The report verifier recomputes comparisons from the captured bytes. The bounded
contract requires every selected call to match, with no mismatch allowance.
`--java-only` acquires exploratory oracle observations and cannot pass that
contract. Unexecuted groups and nonterminating oracle observations remain
explicit gaps. Sequence executions and call comparisons have their own counts;
they are not added to the independent-expression selection or treated as full
language coverage.

The shared `evaluateLanguageSequence` development binding returns ordered
`value\t...` or `error\t...` strings in Node and Wasm, preserving complete value
and diagnostic payloads. Native acquisition uses the same runtime boundary.
Rust collects between calls while retaining the module root. Dedicated native
tests separately verify evaluator-owned class and alias roots, disposable object
reclamation, isolated heaps, ordinary failed-property retries, and terminal
cleanup. Internal bytecode suspension tests retain their scheduler-only scope.
Stage 35 also exercises actual annotation source containing `read` in eager
construction and Deprecated message evaluation. The `annotation_host_probe`
adapter retains the acquired module, performs nested callback evaluations, and
compares complete ordered Java/native status, stdout, and stderr. Native tests
separately inspect annotation roots, temporary-builder reclamation, stale
completion rejection, evaluator isolation, nonterminal cancellation, and terminal
release. These contracts do not establish Host I/O in `evaluateLanguage`, full
module-cache behavior, or arbitrary retries after failed module setup.

Alias sequences cover value identity, generic metadata, failed definition reuse,
and later type uses in properties, deferred annotations, constructors, and
superclasses. They preserve the original class sequence definitions and raw
call outcomes. Their counts remain separate from independent expressions and
native GC/Host tests. Ordered-resolution sequences additionally distinguish
missing names before and after alias initialization, generic parameter shadowing,
nested substitutions, and unfinished aliases with failing arguments. Independent
cases compare class and alias argument-count error ordering, member signatures,
and complete diagnostic frames. Parameter renaming and nested alias expansion
provide separate metamorphic relations. Native and shared platform scenarios
check retained declaration state after the same failures.

Type-use ownership sequences preserve local const-property resolution after a
failed first read. They distinguish independent inline and lambda uses, shared
non-generic let roots, recursive depths, inherited annotation declarations, and
successive members of lazy Listing and Mapping wrappers. Repeated calls resolve
the current receiver and owner while retaining the member definition. Separate
native tests cover Host reentry, cancellation, GC, evaluator isolation, and
terminal release. Const-method lexical barriers and contextual grouped-callee
syntax errors also have independent raw Java comparisons. Stage 18 extends this
contract to finite recursive generics with changing arguments, actual-argument
capture, shared generic let roots and their unbound type variables, and let/for
const barriers. Original observations and failed hypotheses remain separate
from the selected additions. Stage 19 shares instantiated argument graphs,
reclaims them iteratively, and schedules nested defaults with VM fuel and GC
roots. Deep small-stack and process-isolated lifecycle tests verify bounded
execution and cancellation of the original abort input; its historical failure
bytes remain preserved. The adapter instruction limit does not match Java's
stack-overflow diagnostic. Reference metadata traversal within one instruction
and output expansion retain resource and cancellation-latency limitations. The
complete generic, const, and diagnostic corpus remains open.

Stage 20 retains a nullable type's inner default in the evaluator-owned heap.
Selected cases distinguish empty amendments, which expose one attached default,
from amendments with members, which unwrap nested defaults. Null equality,
hashing, type checks, and output remain independent of the attachment;
null-propagating operations discard it. Generic collection defaults use the
existing scheduler and GC roots. Captured closures and Host failure, retry,
cancellation, isolation, and terminal cleanup have separate lifecycle checks.
Direct generic constructors and remaining shared-let and Reference differences
remain outside this selected contract.

Stage 21 adds selected direct generic constructors, including distinct class
actuals, nullable and union defaults, deferred failures, captured functions,
and complete errors for invalid actuals. Separate source-origin metadata keeps
authored alias occurrences and forwarded parameters available to diagnostics.
Collection-body controls preserve Java's absence of an added collection cast
when obtaining the parent for `new T`. Persistent sequences alternate actual
types and retry failed reads within the same evaluator. Native checks exercise
Host reentry, GC, cancellation, isolation, terminal release, and 768-deep default
construction on a 256 KiB evaluator stack. Lazy member frame names, unsupported
builtin types, some missing-property diagnostics, and generator boundaries
remain recorded exploratory failures rather than new development exceptions.

Stage 22 adds selected strict Map construction, lookup, defaults, type checks,
iteration, equality, hashing, and non-forcing stringification. Raw cases separate
insertion order from hash-trie traversal and include boxed numeric keys, duplicate
replacement, nested Map keys, lazy failures, and full diagnostics. Native
lifecycle checks and ordered Java sequences retain their separate contracts.
Complete Map APIs, JVM identity-dependent hash order, and general compound
rendering remain open.

Stage 23 adds selected strict Set construction, duplicate elimination, contains,
defaults, element type checks, equality, hashing, iteration, spread, and
non-forcing multiline strings. Constructor lookup and insertion are separate
resumable phases, including repeated zero-hash preparation. Raw cases and
persistent sequences observe boxed numeric elements, trie boundaries, nested
collections, failed-read retry, and cache-sensitive element order. General
compound result controls remain unselected failures; separate scalar observations
check Set iteration without extending the adapter to general object output.
Higher-order Set APIs and the complete collection corpus remain open.
Nested Map key-rehash side effects during Java value rendering also remain
outside the selected stringification contract.

Stage 24 adds selected `Collection<T>` type semantics. It distinguishes List and
Set values from other collections, preserves nested lazy List checks and eager
Set checks, and covers bare types, aliases, defaults, nullable attached defaults,
casts, union checking and normalized Reference metadata. Explicit empty generic
argument lists now produce the pinned parser diagnostic, including List, Set,
Map and alias controls. Stage 24 left the original parser depth and node limits unchanged.
Persistent sequences and native Host/GC checks retain independent contracts.
Unselected controls preserve String-selected alias-union constructor diagnostic
differences, unsupported member APIs and general compound outputs. These are
remaining gaps, not additional development exceptions.

Stage 25 addresses selected authored frontend depth. Ordinary expression parsing,
resolution and emission use explicit work stacks, with iterative expression-tree
copying and release. Constant List keys and power-assertion anchors have their own
iterative traversals. The selected Java corpus and process-isolated native checks
cover different contracts: exact output and diagnostics versus bounded stack and
lifecycle behavior. Deep type compilation and nested lexical string interpolation
retain separate limitations: nested lexical strings at depth 64 still abort on the
recorded 256 KiB stack. Type-depth 24/31 checks cover parsing, copying and release,
not full type compilation. If/let/lambda and property-only Dynamic object paths
have separate accepted-depth 24/31 full-frontend checks. Class/instance and
mixed/generator compiler paths, and recursive AST Debug/PartialEq, remain outside
that success claim. The historical runtime depth-768 tests construct values after
compilation and do not replace authored-source frontend validation.

A complete runtime type-resolution model, general imported types, complete
annotation metadata/reflection, and full module initialization/caching remain
open requirements. The
persistent adapter requires an acquired module; module-entry failures do not
establish later module retry or cache behavior.

The collection-type selection compares lazy and eager checking, wrapper
composition, aliases, defaults, constraint identity, cache reuse, and complete
diagnostics at the actual fixture URI. Exploratory captures remain separate:
unimplemented format-time cache stealing can cause further checks while Java
renders a diagnostic, including uncaught Java rendering failures. These
observations do not add development exceptions or establish full generic
collection compatibility. See the [checkpoint evidence](../reports/m2/README.md).
