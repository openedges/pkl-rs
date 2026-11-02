# Validation Contract

The goals are behavioral and byte compatibility with the Java implementation of Pkl 0.32.0, with execution time and memory usage at least as good as Java for every required workload. Source verification, inventories, a pinned Java oracle, and raw comparison tooling are available. M1 adds bounded Rust/Java architecture comparisons and actual browser/Node execution; complete language and product performance gates remain open.

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

Only runs that produce correct output are eligible for measurement. Every required workload must satisfy both conditions:

```text
Rust execution time / Java execution time ≤ 1.00
Rust peak RSS / Java peak RSS ≤ 1.00
```

Fix the Java executable artifacts, JDK and Graal/Truffle configuration, hardware, OS, and measurement method. Compare against a sufficiently warmed-up JVM and record results for the official native-image CLI separately. Do not judge the overall performance target solely by cold JVM startup time.

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
