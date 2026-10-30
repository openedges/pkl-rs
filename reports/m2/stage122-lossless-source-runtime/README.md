# Stage122: lossless source, glob Host and shared runtime diagnostics

Local evidence now records **1,779 passing tests, zero failures and five ignored** across an exact union of **285 test harnesses**, and **4,351 raw matches, 15 mismatches, 74 unsupported cases and two Java oracle errors** across the complete ordered **4,442-case ready replay**. No Stage121 passing case regresses; all **88 metamorphic checks** pass. The composed Chromium acquisition and full three-target transport-19 comparison are independently verified; the evidence archives and published copies are verified. This is a Pkl **0.32.0** development checkpoint; full M2, public API, release-platform and performance requirements remain open.

## Implementation and newly matching originals

The shared engine integrates authored UTF-16 source and URI handling, typed glob Host requests/completions, partial-module version diagnostics, declaration owners, fixed-property and typed spread diagnostics, Resource digest defaults and 18 `pkl:math` external bindings. Native, N-API and Wasm share evaluation and diagnostics; platform I/O stays behind Host. `SourceText` keeps authored code units separate from the lexer's internal byte address space. Those internal bytes are not a serialized UTF-8 source format.

Bindings expose bounded lossless source/session transport and exact UTF-16 roundtrips. `evaluate` still returns the probe scalar wire encoding, including Java UTF-8 string bytes and unsupported markers for complex values. This does not establish full typed value export or a complete public evaluator API.

Twelve original cases now match Java's complete raw contract. All paths in the table are relative to `external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/`; the descriptor records their full paths and case IDs.

| Implementation benefit established by these cases | Newly matching original paths |
| --- | --- |
| Resource digest/default behavior and the original math-module program | `api/Resource.pkl`, `api/mathModule.pkl` |
| Selected file-URI and invalid glob import/read diagnostics | `errors/invalidFileUri2.pkl`, `errors/invalidGlobImport4.pkl`, `errors/invalidGlobImport7.pkl`, `errors/invalidGlobImport8.pkl`, `errors/invalidGlobRead1.pkl`, `errors/invalidGlobRead2.pkl`, `errors/invalidGlobRead3.pkl` |
| Fixed-property spread errors, the minimum-version error before invalid syntax, and unknown typed spread-property errors | `errors/cannotAssignFixedProperty3.pkl`, `errors/moduleWithHighMinPklVersionAndParseErrors.pkl`, `errors/spreadSyntaxUnknownTypedProperty.pkl` |

These are observed program/diagnostic improvements, not claims that every Resource, math, glob or spread operation is compatible. Original module matches increase from 793 to 805 out of 888. Historical observations remain 8/8 matches; the JVM-classpath case retains both matching observations. The scalar cohort remains 3,537 matches, seven mismatches and one oracle error across 3,545 cases.

## Harness and command evidence

The harness aggregate joins **198 complete passing checks04 harnesses, 85 passing checks05 harnesses and two freshly repaired checks06 harnesses**. Failed/interrupted original targets are excluded as whole targets. Filtered child repeats are excluded from the outer count, and ignored tests are not counted as passes. The three verification-source epochs preserve their exact input inventories and approved deltas; this six-path continuation delta is not the complete Stage122 implementation inventory.

The original checks04 command was forced to stop. The checks05 controller disappeared during reflection. The reflection test process actually exited zero, and its full raw summary records all five tests passing in 3151.57 seconds. The observer then failed at a process-identity assertion before recording Cargo's exit; its cause remains unproven. **The original Cargo exit stays null/unobserved and its command outcome inconclusive.** The original incomplete producer and failed observer reports, monitoring gap and lack of continuous original resource-bound evidence remain preserved. The aggregate establishes `composed_harnesses_pass`; it does not establish a successful original or fresh full-workspace Cargo command.

Checks06 freshly executes seven commands, all with actual exit zero: xtask, formatting, warnings-denied all-target Clippy, the complete repaired `m2_resources` and `m2_unicode_diagnostic_lifecycle` targets, the Wasm runtime check and manifest freshness. The five non-workspace checks and two repaired targets are separate from the reused harness results. The checks explicitly reuse the checks05 Cargo cache; cache reuse is not fresh compiler provenance. Original source-inventory omissions and the absence of a complete compiler/example inventory remain explicit.

## Ready replay and strict completion gate

Every ready case has a recorded result, including known failures. There are 4,443 native observations because the JVM case has two observations: 4,442 complete evaluations and one stopped List evaluation. The original `api/list.pkl` remains a Java oracle error; its new native attempt times out at the unchanged 60-second watchdog, exits after forced teardown and has `resource_stop=null`. Its peak sampled owned RSS is 82,771,968 bytes, below the 469,762,048-byte List trigger; this was not an RSS stop. Sampled RSS is not Java heap equivalence or performance evidence.

The unchanged exact current `m2_manifest.full_gate_failures` function, applied to the current manifest and actual composed replay, records **1,768 overlapping unmet conditions: 472 implementation, 472 test-mapping and 824 required-case conditions**. This is a function-based calculation, not a new full-gate CLI execution. All twelve newly matching cases remain required and ready with empty `requirements` lists. The gate checks linkage separately from runtime status, so these required-case conditions remain unmet. Implementation and test-mapping states are unchanged; twelve raw matches do not resolve twelve inventory conditions.

The inventory remains 4,545 contracts: 4,442 ready, 71 unimplemented and 32 unmapped, with 475 requirements. Ready-but-unlinked cases and unmapped execution contracts are separate accounting axes. The full gate is not a completion percentage. All nine M2 work areas stay open.

## Shared-product observations

| Actual acquisition | Recorded result | Remaining boundary |
| --- | --- | --- |
| Final M1 actual03, independently rechecked | 6,078 raw comparisons match on each of Native, N-API and Chromium; Node and browser each pass 92 platform scenarios | Both strict and architecture predicates pass within this finite M1 experiment |
| Native lossless capture | 740 ordinary matches and 744 callback matches across 186 lifecycle sessions | Four required internal outcomes remain unresolved |
| Native glob capture | 524 matches and 52 nonmatches across 576 sessions / 1,204 queries | Known API/internal, prerequisite and non-witness boundaries remain required |
| Qualified Native transport | 20 runtime controls and four typed compiler counterparts | This is not 24 dynamic API observations or full Java transport parity; the full three-target transport-19 raw comparison passes |
| Reused N-API capture02 subset | 740 ordinary matches, 744 callback matches and 186 lifecycle sessions, plus frame-policy8 and transport24 cohorts | Exactly 188 already qualified process records are reused; four internal outcomes remain unresolved |
| Fresh N-API capture03 glob subset | 524 matches and the same 52 nonmatching cases/classifications across 576 sessions, 1,204 queries and 9,632 raw field comparisons | All four additional capture02 adapter regressions are repaired; no newly failing case, but full feature parity remains false |
| Composed Chromium04/05, independently rechecked | 740 ordinary matches, 744 callback matches, 186 lifecycle sessions and 524/576 glob matches; transport24 controls pass | 701 qualified processes are reused and 63 are fresh; 52 glob nonmatches and four required internal outcomes remain |
| Math bindings, selected typed/rendered observations | 824 observations match across 412 repeat pairs and 9,032 fields | 92 Java implementation-class fields remain uncovered; this is not full module API coverage |

The N-API glob readback also preserves 72 Native-versus-N-API channel-presence differences across eight already nonpassing sessions, with 40 per-field comparison classifications differing. It does not claim every common field equals Native. Original portable02/03 Chromium accountant failures and whole-capture incomplete verdicts remain preserved. A compiled product or repaired driver never substitutes for actual guest results. Transport-19's complete four-unit input, 167-unit error, Java UTF-8 projections and disposition match across Native, N-API and Chromium. This comparison is not an independent Java transport parity claim.

Capture04 stopped at its 2 GiB sampled owned-process RSS guard on the 702nd process. The stopped case's exact peak was not recorded. Its 701 completed process records were independently qualified; capture05 reuses those and all 764 qualified N-API records, then runs only the remaining 63 Chromium processes. Ten specified large-listing cases use a separate 3 GiB cap; the other 52 glob processes and transport retain 2 GiB. Products, inputs, engine heap/GC settings and time limits are unchanged. All ten resource-experiment cases match Java, with a maximum sampled process RSS sum of 2,370,224,128 bytes. Process RSS sums may include shared pages; this is functional evidence under a different execution limit, not a memory-performance result. Original failures remain preserved. The final capture wrapper exits one because the 52 required glob nonmatches remain, while acquisition completeness and integrity pass.

## Source provenance and publication

The [checkpoint descriptor](../stage122-lossless-source-runtime-checkpoint.json) binds the final 1,888-file implementation roster, including 1,835 changed implementation/fixture paths, to exact Git index blobs. All 150 runtime files and both binding files retain the frozen core identity. Each acquisition keeps its own declared source, product and raw-output scope; the final roster does not make older checks fresh or fill their documented compiler/source-inventory gaps.

The evidence is published in **18 archive parts totaling 67,637,208 compressed bytes**. Fresh admission authenticated 155,178 physical input files, including every one of the 155,162 selected source/raw aliases. Independent archive readback verified all 18,686 payload objects, 155,089 logical aliases, 633 required historical aliases and all 1,888 implementation aliases. Published copies match every verified archive's complete bytes, SHA256 and mode. The descriptor embeds the exact producer, independent-readback and publication-copy receipt bytes; these post-archive receipts stay outside the immutable archive set.

Part 01 contains the catalog and alias maps. Later parts contain `objects/<sha256>` payloads; original paths and file modes are recorded in the logical and historical maps. Source epochs with the same logical filename remain distinct. Historical source restorations are admitted against their recorded full hashes and are not new execution results. One predecessor-manifest ancestry identity has no payload claim; required historical payload gaps are zero. Reused compiled cache bodies remain identity-only where documented, while source/dependency evidence and selected deliverable products are retained.

Preparation records preserve their original pending statuses. In particular, the inherited `source-epochs.json` note for `portable_capture03` still says Chromium is pending; the descriptor's explicit addendum and completed capture05/independent-readback receipts provide the final state. No sealed preparation or failed acquisition was rewritten to claim success.

This local macOS arm64 evidence does not establish hosted CI, Linux/Windows execution, complete product interfaces or either speed/memory target. Known mismatches, unsupported cases, Java errors, ignored tests and all unimplemented/unmapped requirements remain release obligations.
