# Stage145 Protobuf and GC checkpoint

Stage145 adds the shared VM path for selected `pkl:protobuf` rendering, including typed properties and collections, union/type-alias selection, renamed fields and converter paths. Errors retain the observed distinction between declaration/function owners and renderer frames. Direct Dynamic/DataSize/Bytes errors preserve Java's visitor-specific source-frame behavior, and top-level function labels remain distinct from nested function classes. The renderer is available through output format `textproto`. Native, Wasm and N-API share the engine source; Java is a verification oracle. The public `Instruction` enum adds `LoadProtobufModule`, `RenderProtobuf` and `ProtobufRenderType`; downstream exhaustive matches must handle these variants.

The current source passes the five default checks, separate inventory freshness, the full Cargo workspace, the native full266 development contract and a fresh bounded M1 comparison. Workspace results are **2,104 passed, zero failed and six ignored across 330 outer groups**. Full266 acquires every slot, with **240 renderer and eight pre-renderer raw matches**; **18 reference nonpasses remain unmet**, so strict compatibility is false. A fresh selected original57 replay also passes all public raw comparisons; publication/Git closeout remains separate. This is a local development checkpoint, not full M2/API compatibility or achievement of the performance targets.

GC root enumeration now builds per-collection arena/owner indexes, preserving first-match ownership and root order. Empty consumers skip unused indexes while empty publication still clears old owner edges. Existing collection calls remain in place. A later lint correction boxes Protobuf task payloads and replaces a non-repeating loop with an equivalent guarded branch; another removes one unused test import.

## Current verification

Post-lint prechecks03 pass formatting, xtask verification, warnings-denied workspace/all-target Clippy, Wasm checking and manifest freshness. Independent readback04 records five passed checks, zero failed and zero unexecuted, plus separately qualified inventory freshness on the same source epoch. Wasm checking establishes compilation, not renderer behavior in a browser.

Workspace03 completes `cargo test --workspace --locked --no-fail-fast --message-format=json` with exit 0. The independent aggregate reader excludes 13 nested child passes from the 2,104 outer passes. Six ignored tests remain ignored, including the explicit strict Protobuf completion gate. Current renderer13, owner26, origins12 and labels24 targets all pass inside this workspace; those 75 are already included in 2,104 and are not added again.

The earlier standalone 75 and four negative controls are pre-lint observations. Renderer13 contains source-derived development/lifecycle assertions, with a separate actual Java observation for the inherited-source control; the other 62 compare retained Java channels. Neither the combined75 nor repeated fixtures are 75 independent Java observations. The negative controls reject missing result/callback fields, same-length byte mutations, incomplete acquisition and mistaken nonpass promotion. They are validator checks, not four language compatibility cases. They also pass inside the current workspace.

The independent raw266 reader joins the workspace's new (`fresh=false`) normal-Cargo test artifact to compiler/storage/current-byte evidence. It compares projected result status, output, diagnostics and expression bytes in UTF-8 and UTF-16, plus ordered Host/Logger callbacks, against retained Java04 fixtures. Java exception-class channels remain provenance outside the native projection. There were no 266 new Java launches. All native terminals are present, with zero harness or structural failures.

A fresh bounded M1 capture matches **6,078 Java results per Rust/Native, Node and Chromium target**, with **92 Node and 92 browser scenarios**. Its architecture contract and independently derived strict raw comparison both pass. One `--verify-contract` architecture capture ran; there was no second strict CLI invocation. This uses the separately admitted 3 GiB/3 GiB sampled profile. The 18 Protobuf reference obligations remain unmet.

The fixed ordinary entry `java04_completed_reference_contract` attempts every slot and passes its development contract. Fourteen internal-error and four incomplete/process reference observations remain explicit nonpasses without compatibility credit. The separate `java04_strict_full_compatibility_gate` is ignored and unmet in default testing; its required release invocation is documented in `TESTING.md` with `--ignored --exact`. That strict CLI was not run, and strict compatibility derived from the same capture remains false. The existing 32 renderer fixtures overlap full266 and must not be added as unique coverage.

Verification is local on macOS arm64 with Rust 1.98.1 and the pinned Pkl 0.32.0 oracle. Workspace source bookends match and owned children settled under the separately admitted 2 GiB child/supervisor and 2 GiB aggregate sampled profile. The aggregate reader uses pinned child-source and marker assumptions; it does not independently prove every test's identity, absence of count-preserving interleaving or complete dependency closure. Resource samples are not hard peak guarantees, and these receipts are distinct from hosted CI.

## GC and retained failures

Separate GC verification passed three baseline filters, 12 candidate focused tests and all five tests of the whole reflection target. Constructed owner/root-vector tests alone do not establish compiler-reachable whole-VM retirement; existing compiled lifecycle tests provide separate coverage. Short paired timings were inconclusive. The whole reflection target took 2645.76 seconds versus a historical 2641.40-second observation, with different ordering/warmth and a one-second sampling observer. No causal speedup, Java performance comparison or performance-target qualification is established.

Paths below are relative to `target/m2/`. Historical outcomes remain recorded; later successes do not rewrite them.

| Preserved outcome | Receipt or scope |
| --- | --- |
| First broad native capture stopped at 196/266 | `stage145-protobuf-native-01/report.json`; incomplete wall-limited acquisition. |
| Native02: 228/240 renderer, 8/8 pre, 12 ordinary mismatches, 18 reference nonpasses | `stage145-protobuf-native-readback-02/report.json`; current 240+8 matches are a separate result. |
| Original collecting allowance 200,000 failed | `stage145-protobuf-lifecycle-extended-readback-01/report.json` retains the comparison. The later 400,000 allowance is a changed condition; its diagnostic completed at 237,679 polls, then the normal renderer target passed under the revised allowance. |
| First and second Clippy attempts failed | `stage145-default-prechecks-readback-02/report.json` and `stage145-default-prechecks-readback-03/report.json`; each retained two passed, one failed and two unexecuted checks before fresh prechecks03. |
| Invalid authored class-local alias and missing primary Host setup | Original source/setup errors remain retained; corrected controls require their actual Java authority rather than invented Rust expectations. |
| Supplemental named-helper Java NPEs | Oracle errors retained outside ordinary controls; no compatibility credit or addition to the 18-slot ledger. |
| Original57 staging and reader setup failures | Attempt01 rejected the real `pkl.tar` directory before case execution. Acquisition02 completed, but the first reader configuration incorrectly parsed progress plus summary as one JSON document. The directory and exact-progress reader repairs preserve both failures. |

Rejected binding/readback attempts also remain in local receipts. The current qualified readers describe their successors' scope without relabeling failed or incomplete attempts. The hash-only manifest refresh changes implementation links without changing requirement/case states, counts or mappings; it is not a full-language replay.

## Closeout receipts and remaining scope

| Scope | Independent receipt / status |
| --- | --- |
| Current defaults and inventory | `stage145-default-prechecks-readback-04/report.json` |
| Current workspace | `stage145-native-default-workspace-readback-03/report.json` |
| Current native raw266 | `stage145-workspace-protobuf-readback-03/report.json` |
| Pre-lint standalone 75 / negative 4 | `stage145-protobuf-final-focused-readback-01/report.json`; `stage145-protobuf-fixed-gates-controls-readback-01/report.json` |
| GC focused / whole reflection | `stage145-gc-focused-independent-readback-01/report.json`; `stage145-gc-reflection-independent-readback-01/report.json` |
| Fresh bounded M1 | `stage145-platforms-readback-01/report.json`: 6,078 matches per target, 92 Node/browser scenarios, strict raw and architecture-contract pass from one capture. |
| Selected original57 | `stage145-original57-readback-02/report.json`: 57/57 pass after unchanged canonical raw verification, with the current source-bound normal Cargo example. |
| Source verification / publication / Git | Separate final source verification, finite payload, integrity/publication receipts and actual commit/push complete the checkpoint; their results are recorded in the linked publication files. |

M1 reuses the existing 6,078-per-target primitive corpus and 92 Node/browser scenarios. That is regression/platform evidence, not a new Protobuf-specific browser/Node matrix. The Protobuf comparisons above are native development/raw-Java evidence; neither Wasm compilation nor M1 establishes full Protobuf API/platform equivalence.

Original57 uses fresh pinned-Java/native CLI observations under `upstream-original-modules-v1`, excluding project fixtures and preserving the actual managed-workspace URI. Canonical acquisition and independent verification both complete, with raw output, error and generated-file comparison unchanged. Its subset full gate remains false. The selection contains 41 API cases, 11 `pklbinary` cases and five import/resource policy cases. These overlap capabilities developed in prior phases; they are regression replays, not 57 additional Protobuf cases or unique coverage added to earlier totals. The other 31 previously nonpassing originals and the full ready inventory are unselected/unexecuted in this acquisition, with historical statuses retained.

The finite publication is described by the [evidence index](evidence-index.json), [integrity readback](evidence-integrity.json), [source verification](source-verification.json) and [publication receipt](publication.json), alongside the [evidence archive](evidence.tar.gz). This documentation draft is selected before packing; the published README is a byte-identical copy only after the actual archive checks. Publication integrity is separate from language compatibility.

Full language/API semantics, 18 reference obligations and unprojected diagnostics, release-platform scope and speed/memory targets remain open. Stage146–151 candidates remain outside this checkpoint unless separately integrated and qualified.
