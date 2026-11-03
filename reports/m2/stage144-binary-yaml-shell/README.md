# Stage144 binary, YAML and shell checkpoint

Stage144 adds shared byte rendering for `pkl:pklbinary`, bounded YAML document parsing, and the authored `pkl:shell.escapeWithSingleQuotes` function. Binary module headers count regular properties correctly when `Module.output` is inherited and hidden. Class records recover a missing declaration URI from the declaring block, preserving the owner of imported classes. Shell escaping retains the observed apostrophe, control-character and UTF-16 behavior. YAML parsing builds document graphs while the shared VM constructs Pkl values and invokes converters.

These changes are integrated with seven subsequent lint corrections. The public `Instruction` enum adds `YamlParse` and `RenderPklBinary`; downstream exhaustive matches require updates. The five production prechecks, full workspace command and fresh bounded M1 comparison have passed locally. Full M2/API, release-platform and performance requirements remain open.

## Recorded validation

Before the lint corrections, the rebuilt engine passed **38 focused native tests with 320 retained Java observations**, with no failed or ignored tests. Ten integration harnesses and four exact unit filters cover binary, YAML parsing, shell escaping and related error/lifetime controls. These fixtures are not 320 fresh Java launches. Two additional YAML oracle-internal observations remain uncompared nonpasses; Java exception-class identity remains provenance where it is outside the modeled Rust fields.

A separate fresh acquisition matched **all 21 original Java/Native CLI contracts**, comparing status, stdout, stderr, public files, workspace changes and timeouts. These results precede the lint corrections and are not added to the focused totals. The earlier acquisition's ten passes, nine header mismatches and two missing-class-URI internal errors remain preserved.

After the seven lint corrections, prechecks05 passed formatting, xtask verification, warnings-denied workspace/all-target Clippy, Wasm checking and manifest freshness. Workspace10 then completed `cargo test --workspace --locked` with exit 0. Its independent reader qualifies **1,947 passed, zero failed and five ignored across 323 outer groups**. Thirteen nested child passes are excluded. Ignored tests are not passes. The reader validates aggregate counts under pinned child-source and marker assumptions; it does not independently prove every test's identity, absence of count-preserving interleaving, or complete dependency closure.

A fresh bounded M1 capture matched **6,078 Java results per Native, Node and Chromium target**, with **92 Node and 92 browser scenarios**. One `tools/m1_platforms.py --verify-contract` command ran; the independent reader derived strict raw compatibility from that same capture. No second strict CLI run occurred. Its first reader rejected the recorded Python command spelling. The successor validates the already established exact logical Homebrew launcher; no capture rerun or runtime change was needed.

Production integration covers 3,366 destinations. A separate refresh changes 88 implementation-link hashes without changing requirement or case states/counts. No fresh full-language inventory replay is claimed. Validation was local on macOS with Rust 1.98.1 and the pinned Java Pkl 0.32.0 oracle.

## Preserved attempts and provenance

Paths below are relative to `target/m2/`; complete identities remain in the selected receipts.

| Evidence | Receipt or retained limitation |
| --- | --- |
| Focused / original contracts | `stage144-binary-module-fix-readback-02/report.json`; `stage144-original-fixed-readback-01/report.json` |
| Production prechecks | `stage144-default-prechecks-readback-05/report.json` |
| Workspace10 | `stage144-native-default-workspace-10/report.json`; `stage144-native-default-workspace-readback-11/report.json` |
| Workspace05–07 | Source-bookend or child RSS exceeded the original 1.5 GiB cap. Attempts 06/07 retained matching source bookends. |
| Workspace08 /09 | Storage-copy reserve and final bookend failure / wrong admission SHA rejected before Cargo. |
| Workspace reader10 /11 | Archive-row metadata policy failed / opaque recorded-row handling corrected; no archive bodies opened. |
| M1 capture / reader | `stage144-platforms-01/report.json`; `stage144-platforms-readback-02/report.json`; failed reader01 retained. |
| Loader no-run probe | `stage144-storage-loader-readback-01/report.json`; successful compilation, no test functions executed. |

Workspace10 used a separately admitted 2 GiB child/supervisor and 2 GiB aggregate profile, a 1,216 MiB disk floor and 448 MiB growth reserve. Its source bookends match. Observed child/supervisor and aggregate peaks were 1,594,916,864 and 1,622,163,456 bytes; these are samples, not hard peak or performance guarantees. Default prechecks retain their original 1.5/2 GiB condition; M1 has a separate 3/3 GiB condition.

Closed maintenance preserved raw failures and sealed products. Removing 100 reproducible cache executables changed observed free space by +455,950,336 bytes; compressing one closed loose-JSON file changed it by +138,203,136 bytes. An independent storage reader confirms the selected deletion and byte/metadata checks. These figures do not prove exclusive-block reclaim. The 932-file M1 cache clone is provenance, separate from the actual platform comparison.

## Publication and remaining scope

The [evidence archive](evidence.tar.gz) contains 1,030 selected files (88,144,442 logical bytes; 14,444,156 compressed bytes). The [index](evidence-index.json) fixes every member identity, and the [independent integrity report](evidence-integrity.json) verifies exact member order, names, modes, lengths and SHA-256 hashes, together with gzip integrity. The [publication receipt](publication.json) confirms the published copies. A separate [source verification](source-verification.json) confirms all 3,367 selected source, fixture and manifest destinations still match their qualified bytes and modes. Compiled bodies, caches, executable tools and full workspace inventories remain outside the payload; no hermetic or executable restoration is claimed.

This bounded M1 result does not replay the full language inventory. Three historical groups retain a Java UTF-8 boundary with separate JavaScript-unit observations. Unsupported YAML Unicode/URI/internal branches, synchronous long-token cancellation responsiveness, complete renderer/API semantics and release/performance obligations remain open.
