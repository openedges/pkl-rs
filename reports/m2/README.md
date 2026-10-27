# M2 language evidence

## Stage99 numeric and Set API checkpoint

Recorded local evidence: [Stage99 checkpoint](stage99-numeric-set-apis-checkpoint.json); archive prefix `stage99-numeric-set-apis-evidence` uses numbered parts of at most 45 MiB.

Stage99 integrates 13 numeric and 52 Set routes. Targeted04 on freeze04 passes
**25 tests, zero failures and one ignored test**, matching **2,430 ordinary
observations, 26,960 shared raw fields and 1,238 Logger events**. These are
1,215 ordinary contracts repeated twice; the full 1,218-contract Java matrix
also retains three required foreign-entry internal contracts / six observations.

The terminal workspace passes **1,418 tests, zero failures and three ignored
tests** across 245 test executables, 25 compile-only examples and two doctest
libraries. Thirteen nested summaries are separate. Formatting02, xtask02,
Clippy02, Wasm01 and Python01 pass; Python runs **180 tests**. The default driver
reuses actual same-source Clippy02 and executes seven new commands. **All eight
roles pass**, including manifest-final and inventory; missing/unexecuted lists
are empty and inputs remain stable.

The workspace reuses the actual targeted04 normal runtime as Cargo Fresh and
newly builds the module/semantics probes. Read-only platform schema admission
succeeds. Actual M1 matches **6,078 cases per Rust/Node/Chromium target** and
across the shared engine; strict and architecture gates pass. Selected native,
N-API and Chromium each complete 23 programs / 46 sessions / 2,436 observations.
All 2,430 ordinary observations and 1,238 Logger events match per target; three
required internal contracts / six observations remain unresolved. The platform
reader checks 31,820 shared raw fields including expressions and Logger counts.
The terminal window has stable sources and no acquisition errors, with
`comparable_pass=true` and `all_required_pass=false`. Current native-adapter rustc
exits 0 but retains a 686-byte LLVM-loader strip warning; stripping is not
claimed complete. This is distinct from the earlier Stage98 N-API warning.
Original Float/Int/Set native replay is terminal with **two of three raw matches**.
Float and Int pass; Set fails on `List(1).minWith(comparator)` at original line
249. Native stdout/stderr/exit are 0 bytes/931 bytes/1 versus Java
6,984 bytes/0 bytes/0. The required mismatch and unchanged-original replay
after full List integration remain open. Earlier compile, targeted02 **22/3/1**,
prelaunch and Clippy01 failures,
and their exact corrections remain preserved.

Actual default, M1, binding, selected and original replay records have
independent readbacks. Stage94's **4,268/4,424** aggregate and full ready-corpus/M2/API/release/
performance obligations are unchanged. Earlier String glob protocols remain
separate required unresolved cases.

## Stage98 shared String API checkpoint

The [checkpoint](stage98-string-apis-checkpoint.json) records 28 shared String
routes and uppercase result identity, with exact source/artifact/raw identities
and `stage98-string-apis-evidence` archive parts. Native, N-API and actual Chromium
each acquire all **2,418 observations** and match **2,412 comparable observations**
from 1,209 query/profile contracts repeated twice, including 160 Logger events.
Three glob contracts retain **six required unresolved observations per target**;
`all_required_pass` remains false.

The final workspace passes **1,393 tests, zero failures and two ignored tests**;
Python passes **180 tests**. All eight local check roles pass with scoped reuse
of unchanged formatter/xtask inputs. A fresh M1 acquisition passes **6,078 strict
comparisons per target** and the separate architecture contract, with 92 Node and
Chromium checks each. Original String-related native modules match **6/6**: four
successes and two expected Pkl errors, including public file/write maps. Native
private state is unchanged; Java Host-private cache bytes are not compared across
runtimes.

Earlier compiler/test/collector/Python/platform failures and the N-API debug-strip
warning are retained. The successful Wasm-link retry uses a process-local pinned
LLVM library path; reused unit hashes are successor admission-time identities,
not original compiler-time hash bookends. Stage94's **4,268/4,424** aggregate and
the remaining full ready-corpus, M2/API/release/performance obligations are
unchanged. This local selected evidence does not close full String compatibility.

## Stage97 expression adapter and token checkpoint

The [checkpoint](stage97-token-expression-checkpoint.json) records the native
complete-expression adapter and three finite token requirements. Actual token
argv comparisons improve from 24/36 to **36/36**; shared token tests match 54
observations and 324 raw fields. The complete affected scalar replay is
**3,537/3,545**, retaining seven mismatches and one Java oracle error with no
old-pass regressions. All 88 metamorphic groups pass. Strict compatibility remains
false; the successful recorder exit establishes evidence integrity only.

Final default Rust checks pass, including **1,371 workspace passes, zero failures
and one existing ignored probe**. Seven adapter/Host tests now run in that suite.
The new native example compilation and unchanged Stage96 runtime reuse retain
separate provenance. Earlier native/test/controller failures are preserved;
Python manifest command identity comes from argv, not the recorder's imprecise
`cargo_spawned` field. No fresh Node/browser or performance evidence is claimed.
Full M2 remains incomplete, and Stage94's historical whole-corpus aggregate is
unchanged. Six retained failures now use native unsupported exit 2 rather than 1;
their stdout/stderr remain identical, but full failing triples are not unchanged.

## Stage96 scalar, collection and function APIs

The [checkpoint](stage96-core-apis-checkpoint.json) records five shared APIs:
`Float.toInt`, `Int.toRadixString`, `List.getOrNull`, `Set.restOrNull` and
`Function.applyToList`. All default Rust checks pass; the complete workspace has
**1,358 passes, zero failures and one existing ignored probe**. Targeted17 tests
are included in this count. Actual compilation and Cargo Fresh reuse are distinct.

Native, N-API and Chromium each match **206 observations** from 103 Java
query/profile contracts repeated twice: 146 values, 60 ordinary errors and
12 ordered Logger events per target. M1 strict6078 comparisons and the separate
architecture contract pass on each target. Earlier failed attempts remain recorded.

Original native CLI results remain **2/5 matches**; missing `Float.toFloat`,
`Int.toInt` and `Set.any` produce three failures. The original List Java heap error
is retained without a Rust comparison. JVM identity-dependent hash limitations
and unpaired Java exception metadata remain explicit. Stage94's earlier-source
**4,268/4,424** full-corpus aggregate and full M2/API/release/performance gates
remain unchanged. Exact unchanged source restoration requires all three Stage95
source archive parts plus the Stage96 source delta; the checkpoint lists identities.

## Stage95 PListRenderer checkpoint

Shared PListRenderer document/value output is integrated with indentation,
XML/UTF-16 escaping, scalar and Bytes tags, null omission, converters,
annotations and the existing lazy/Host renderer continuation.

| Evidence | Recorded result |
| --- | --- |
| Canonical checks | Formatting, manifest refresh/freshness, xtask, Clippy and Wasm pass |
| One canonical workspace invocation | 1,341 passed, zero failed, one existing ignored; 236 test executables, 25 compile-only examples and two doctest libraries |
| Nine native original CLI-argv cases | 9/9 raw matches: eight successes and one ordinary error; empty file/write maps |
| M1 strict and separate architecture contract | 6,078 strict comparisons and the separate architecture contract pass on each target. |
| Selected Native/N-API/Chromium | 450/450 raw matches on each of Native, N-API and Chromium: 234 successes, 216 ordinary errors, 4,000 shared fields and 40 ordered Logger events per target. |

Workspace/xtask use Rust 1.98.1, opt1/debug0/strip-symbols, assertions and overflow
checks, jobs1 and incremental0; Clippy/Wasm use opt0/debug0. The 238 outer test
summaries exclude 13 nested children: 12 parse directly and one is preserved
with its interleaved diagnostic and raw-byte location. The prior isolated
131 passes across 16 targets and 18 original API byte vectors are not added to
the canonical totals.

Java authority has 97 queries and 194 observations (102 successes, 90 ordinary
errors, two internal errors). One recursive-converter query remains unresolved.
The 96 comparable queries preserve 80 reached renderer controls and 16
syntax-preempted errors. The selected platform comparison includes 129 Properties queries:
225 queries repeated twice, or 450 observations per target. Shared status,
UTF-8/UTF-16 and ordered Logger are the selected comparison channels; Java
exception metadata and nonempty-file writing remain outside that adapter.

The converter session compares 11 selected rows against a Java session containing
12; the unresolved row is retained but excluded. This does not establish full
original converter-process protocol parity.

The [checkpoint](stage95-plist-renderer-checkpoint.json) lists all evidence
parts and the separate [publication review](stage95-plist-renderer-publication-review.json).
Earlier failed attempts remain evidence. Stage94's earlier-source whole-corpus
**4,268/4,424** result is unchanged; full M2, APIs, release platforms and
performance/memory targets remain open.

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

The [checkpoint](stage94-full-ready-composition-checkpoint.json) and
[publication review](stage94-full-ready-composition-publication-review.json)
identify all three required companion archives:
[interrupted acquisition and recovered scalars](stage94-full-ready-composition-evidence-01.tar.gz),
[fresh original modules](stage94-full-ready-composition-evidence-02.tar.gz), and
[JVM, historical and supporting evidence](stage94-full-ready-composition-evidence-03.tar.gz).
Earlier sections retain their checkpoint-specific sources and results.

## Stage93 YamlRenderer integration

YamlRenderer document/value rendering is integrated in the shared engine. It
covers version-dependent scalar quoting, Unicode and numeric output, indentation,
streams, null omission, converters, annotations and lazy/Host evaluation order.
The isolated candidate passes 110 tests across 14 targets and compares 321
renderer queries twice, alongside 129 Properties regressions. Twenty-three
unchanged originals match 46 API byte-vector observations and a separate 46
native CLI-argv stdout/stderr/exit observations. Empty observed file maps do not
establish a nonempty-file writer or complete product CLI support.

Canonical formatting, manifest refresh/freshness, repository verification,
Clippy and Wasm compilation pass. The recorded canonical results are:

| Evidence | Actual result |
| --- | --- |
| Complete canonical workspace target coverage | 1,319 passed, zero failed, one existing ignored; 233 test executables, 25 examples, two doctest libraries in 36 invocations; 13 nested child summaries separate |
| M1 strict comparison and architecture contract | 6,078 strict comparisons and the separate architecture contract pass on each target |
| Native/N-API/Chromium, 900 observations each | 900 raw matches each: 642 successes, 258 ordinary errors, 8,040 shared fields and 84 ordered Logger events |

Independent source and execution readbacks passed. Public evidence paths are [checkpoint](stage93-yaml-renderer-checkpoint.json),
[evidence archive](stage93-yaml-renderer-evidence.tar.gz) and
[publication review](stage93-yaml-renderer-publication-review.json). Earlier
failed preparations and raw Java ordinary/oracle errors remain evidence. The
full Java authority has 332 queries; ten Parser queries and one oracle-internal
query are outside the selected 321, and eight Parser originals remain open.
At Stage93, readiness was 4,424/71/32 and the completed Stage86 whole-corpus
result was 4,214/4,416. Stage93 selected evidence did not update that aggregate
or establish complete APIs or performance parity.

## Stage86 complete ready-corpus replay

The [full replay checkpoint](stage86-full-replay-checkpoint.json) records all
4,416 ready cases and the completed separate integrity and regression audits.
The result is **4,214 matches, 61 mismatches, 139 unsupported cases, and 2 Java
oracle errors**, with no Rust internal error. Original modules match 694/888;
expression strings match 3,519/3,527 and the JVM classpath case matches.
There are 26 new matches and no prior-pass regressions against Stage76–78.

This recorded local run reuses sealed Stage67 Java authority and the exact
Stage86 candidate artifacts. It starts no new Java process and does not count
as a new workspace, browser, or Node test run. Raw observations, source/build
identities, failed attempts, and verification results remain distinguishable.
The replay-boundary 111 unready cases, 1,791 overlapping full-gate obligations, and all nine M2
workstreams remain open. Full API, platform-release, speed, and memory targets
are not established by these counts.

## Stage92 historical collection admission

The [checkpoint](stage92-historical-admission-checkpoint.json) and canonical
[registry](../../compatibility/m2-historical-collections.json)
connects eight pinned historical modules to a separate Pkl 0.32.0 CLI-argv
contract. All 4,527 case IDs/order and 475 requirement records remain intact;
the current readiness inventory is **4,424 ready, 71 unimplemented, 32 unmapped**.
The remaining historical candidates and three selected regression contracts
retain their original states. Source, dependency, license and notice bytes are
unchanged from the pinned rev1 input inventory; prior rev1 pass records are not
compatibility authority.

Canonical Python checks pass 76 tests in normal mode and 76 under `-O`, together
with inventory freshness and repository verification. A real finite mixed
canonical-runner acquisition compares eight historical originals and an upstream
Boolean module twice: **18/18 raw comparisons match**, including 16 historical
observations. Its case-list wrapper, actual Stage91 native compiler/dep-info/extern
binding, full CLI stdout/stderr/exit and managed file/write records are separate
from earlier Stage90-native evidence. The original schema6 mixed-profile defect,
its successor, and the old 16 comparisons remain preserved; new canonical tools
also revalidate those old raw comparisons without relabeling their producer.

Receipts remain under `target/m2/stage92-canonical-integration-01`,
`stage92-canonical-checks-01`, `stage92-canonical-mixed-01` and
`stage92-canonical-offline-cli-check-01`; the checkpoint records their identities. Actual file
and unexpected-write sets are empty: no nonempty generated-file capability,
global sandbox, full CLI/API/UTF-16/Logger or new JVM/platform result is claimed.
At Stage92, the completed whole-corpus aggregate was Stage86 **4,214/4,416**;
these eight admissions are not added to that older-source aggregate. Full M2,
release and performance gates remain open.

## Stage91 PropertiesRenderer checkpoint

The [checkpoint](stage91-properties-renderer-checkpoint.json) records shared
PropertiesRenderer support and its String code-point projections and List count
dependencies. It preserves option, converter, annotation, forcing and error
order, UTF-16 escaping, and Host/GC lifecycle controls.

Formatting, repository verification, Clippy and Wasm compilation pass. Complete
default workspace coverage spans 230 test executables, 25 compile-only examples
and two doctest libraries through 36 initial Cargo invocations plus one affected
target rerun. The initial run's unsupported-PropertiesRenderer assertion failure
is retained. Only its test source changed; actual dependency closure proves
that the other 229 targets and all ordinary library/example producers were
unaffected. Final composed results are **1,276 passed, zero failed and one
ignored**, with 13 nested child summaries counted separately. This is not a
claim that the original failed run or a single workspace command passed.

Native, N-API and Chromium each match 422 observations from 211 queries repeated
twice. One M1 acquisition matches 6,078 primitive results per target and passes
its separate architecture contract. The isolated candidate also compares 14
unchanged originals twice through a native CLI-argv module probe and twice
through API byte vectors. All raw compared channels and ordinary errors remain
intact; Java exception metadata and nonempty generated-file support are not
claimed. Generated payload dispositions retain exact hashes and producer/source
records without promising restoration of the original binaries.

The latest whole-corpus result remains Stage86. Historical admission is now
recorded separately in Stage92; YAML remains separate work. The Stage91
checkpoint retained 111 unready inventory cases. All nine M2 workstreams, full
product APIs, release platforms and performance targets remain open.

## Stage90 lazy collection checkpoint

The integrated collection changes add Listing endpoints, indexed folds,
membership/predicate visitors, distinctness selectors and joins, plus Mapping
folds and visitors. Required local Rust checks pass with **1,247 workspace tests
passed, zero failed, and one existing ignored probe**. Independent review
accounts for 13 nested summaries separately and verifies the source/producer
bindings of 225 test executables, 25 built examples, and two doctest groups.

The isolated candidate matches 139 Java queries twice and eight unchanged
historical modules twice. Query comparisons preserve represented UTF-8/UTF-16,
status and ordered Logger fields; module captures compare complete stdout,
stderr and exit status. Java type/exception/cause metadata, generated files and
full product CLI behavior remain separate obligations. Original failed attempts
and subsequent corrections are preserved.

The integrated source matches 278/278 observations on each of Native, N-API
and Chromium, with 14 independent evaluator programs and two repetitions.
One M1 acquisition matches 6,078 primitive results per target and passes its
separate architecture contract. The [checkpoint](stage90-lazy-collections-checkpoint.json)
retains the actual source/producer bindings, raw observations and failed attempts.

These observations do not replace the Stage86 whole-corpus aggregate or close
the historical inventory's missing execution contracts. All nine M2 workstreams
remain open.

## Stage87–89 integrated language checkpoint

The [checkpoint](stage87-89-language-checkpoint.json) records missing-key
suggestions, semicolon/comment continuation, substring operations, signed-literal
trace spans, and `super` method scope checks. All required local Rust checks
passed: the full workspace has 1,228 passes, zero failures, and one existing
ignored probe. Nested child test summaries are counted separately.

Native, N-API, and Chromium each match 294 observations from 147 queries repeated
twice, including full represented UTF-8/UTF-16 results, errors, ordered Logger
fields, and protocols. Each target also matches 28 standalone module-output
byte-vector observations. Five sibling-module cases remain outside that portable
adapter, and these observations do not establish full CLI process/file behavior.
A single M1 acquisition matches 6,078 Java results per target and passes its
separate architecture contract.

The first combined platform capture failed on an unserviced external-property
read. Its successor uses the same empty external-property profile as the pinned
Java helpers, with request/completion traces. Only the native driver was rebuilt;
core and portable artifacts and the M1 evidence were reused. Failed attempts,
raw authority, source maps, producer records, and independent readbacks are
retained. Generated binaries and build caches are excluded from the evidence
archive; release receipts retain their recorded identities.

These are local macOS arm64 results. The completed whole-corpus aggregate remains
the Stage86 result above; no new whole-corpus pass count is claimed. M2's nine
workstreams, full APIs, release platforms, speed, and memory targets remain open.
See [the verification scope](../../docs/m2-language.md).

## Stage86 List membership and constraint observations

The [Stage86 checkpoint](stage86-contains-hash-power-checkpoint.json) records shared
`List.contains`, Listing insertion-hash indexing, and literal/interpolation
observation fixes. All five required Rust checks passed; the complete workspace
recorded 1,149 passes, one ignored test, and zero failures.

Native, N-API, and Chromium each matched 104 observations from 35 new and 17
regression queries, with two repetitions. Raw public result, UTF-16, Logger,
and protocol channels are retained. A single M1 acquisition matched 6,078
results per target and passed the distinct architecture contract.

The earlier 76/104 platform result remains a failed acquisition. Its adapters
disabled power assertions; the successor uses explicit per-program settings
from the Java helpers and a separately recorded native adapter linked to the
unchanged core. Candidate and tooling failures are also preserved. The actual
readback report discloses its author's adapter work.

The completed successor replay establishes the 4,214/4,416 aggregate above;
M2, complete APIs, release platforms, and performance requirements remain open.
See [the verification scope](../../docs/m2-language.md).

## Stage85 distinct collections and reentrant hashing

The [Stage85 checkpoint](stage85-distinct-checkpoint.json) records
`List.distinct`, `Listing.isDistinct`, and completed-cache structural keys during
untyped reentrant hashing. All five required Rust checks pass. The full workspace
has 1,136 passing tests, one ignored test, and zero failures; nested child runs
are counted separately. Its earlier capacity interruption and the isolated
candidate's initial reentrant failure remain historical evidence.

The 17-query Java cohort matches twice on each of native, N-API, and Chromium,
including public UTF-8/UTF-16 results, status, and ordered Logger fields. One fresh
M1 acquisition matches 6,078 Java results per target and passes its separate
architecture judgment. The [language guide](../../docs/m2-language.md) describes
the comparison limits. The official full-corpus aggregate remains 4,188/4,416;
full M2, API, release-platform, and performance requirements remain open.

## Stage84 List search and transforms

The [Stage84 checkpoint](stage84-list-search-transform-checkpoint.json) records
`List.find`, `List.filter`, and `List.flatMap`, all five required Rust checks,
1,126 passing workspace tests, the separate formerly ignored Regex regression,
and 44 queries compared twice on each of native, N-API, and Chromium. Both fresh
M1 experiments match 6,078 Java results per target. The archive retains raw
failures and successful successors, source maps, artifact identities, and explicit
build profiles. See the [language guide](../../docs/m2-language.md) for comparison
boundaries. The full 4,416-case replay subsequently stopped at the sampled
disk-space floor. Its 3,972-case progress prefix is incomplete and does not
replace the previous aggregate. The remaining M2 requirements are unchanged.

## Stage79–83 focused implementation checkpoint

The [focused checkpoint](stage79-83-focused-checkpoint.json) records shared
Reference/version-frame, typed-equality, duplicate-owner, and type-conflict fixes,
qualified Pair construction, and common class-display rules. Final-source local
checks passed 109 distinct test functions across 24 integration targets, plus
formatting, reference verification, all-target Clippy, and Wasm compilation.
Two nested child reruns are not counted as additional tests. Java's 13-query
class-display cohort matches on all tested public channels over two repeats.

Full workspace tests, current platform execution, and the full ready-corpus
replay remain pending. Earlier failed attempts are preserved separately; the
Stage76–78 aggregate below was the latest completed full replay at this checkpoint. This is
a focused development checkpoint, not M2 completion or performance evidence.

The [source binding](stage79-83-focused-source-binding.json) connects all 2,363
recorded source paths to implementation commit `8d25b6f`. It adds no executions
and does not claim complete compiler-input coverage.

## Stage76–78 canonical implementation evidence

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
API, platform release, or performance parity. The [implementation checkpoint](stage76-78-implementation-checkpoint.json) retains the failed
attempts and exact source/artifact identities.

The [source binding](stage76-78-source-binding.json) connects the recorded
178,382 input paths and a separate 72-input appendix to implementation commit
`7ce4769`, pinned references, or preserved pre-change copies. Eight
documentation/tool transitions are recorded separately. The first binding
attempt failed with insufficient disk space; a fresh read-only completion
verified the retained maps and exited zero. This adds no compatibility passes
and does not claim that the recorded maps cover every compiler input.
The [independent publication review](stage76-78-source-binding-review.json)
checks the published maps, archive payloads, and separate completion records.

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
[candidate checkpoint](stage76-78-candidate-checkpoint.json).

## Stage 73–75 implementation evidence

The source integration, 3,270-path canonical freeze, five-command Rust
validation, native/N-API/Wasm build, three selected platform cohorts, and M1
experiment are recorded separately in the Stage 73–75 local evidence. The
required Rust commands exited zero. The workspace count audit reports 1,069
distinct passes and two ignored tests; 13 nested child reruns are retained
without counting them again as distinct tests.

For each of native, N-API, and Chromium, the platform cohorts comprise 310
distinct authored queries and 620 ordered observations. Selected public result
channels matched their Java authority. Ninety-one earlier queries have no
recorded Java logger authority, so their 182 observations per target carry no
logger compatibility verdict. The M1 experiment's contract passes, but its
two Regex differences per target remain compatibility failures.

The full 4,416-case Rust replay records **4,163 matches, 110 mismatches, 141 unsupported results, 2 Java oracle errors, and 0 Rust internal errors**. Original modules match 643/888; expression strings match 3,519/3,527. The JVM classpath case matches. Compared with Stage69–72, 5 previously nonpassing cases now match, with no prior-pass regressions.

The linked replay, separate integrity verifier, and independent regression audit completed successfully. The replay reuses sealed Stage67 Java outputs and starts no fresh Java process. The strict full gate retains 1,791 overlapping unmet conditions. The 111 unready inventory cases and all nine M2 workstreams remain open. Earlier mapping rejection, interruption, ENOSPC, and monitor failures remain recorded separately.

See the [implementation checkpoint](stage73-75-implementation-checkpoint.json) for raw evidence and execution provenance. These local records do not establish full M2, product API, release-platform, or performance compatibility.

### Actual implementation source binding

The [source binding](stage73-75-source-binding.json) connects 170,567 recorded
inputs to implementation commit `0202da8`, whose push to remote `main` was
verified, or to exact preserved copies. It preserves five post-validation
documentation transitions separately from the frozen runtime and fixtures.
Five historical target inputs are linked through their published captured
copies with exact bytes and modes; the first failed binding and its partial
output remain preserved.

The [independent publication review](stage73-75-source-binding-review.json)
verified the 8,023,673-byte archive, all 379 members, and all raw payload bytes
and modes. The [publication execution](stage73-75-source-binding-execution.json)
and its four raw/source files postdate the archive and are published separately.
Binding and publication exited zero with unchanged inputs. This evidence adds
no guest evaluations, Cargo checks, or compatibility passes and leaves all
remaining M2 conditions unchanged.

## Stage69–72 — shared runtime and full ready-corpus replay

The [implementation checkpoint](stage69-72-implementation-checkpoint.json) records shared super entry defaults,
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

The [source binding](stage69-72-source-binding.json) connects the verified inputs
to implementation commit `a7c4b02`, whose push to remote `main` was verified.
It accounts for 169,117 inputs: 167,589 Git-backed inputs and 1,528 captured files,
plus 1,824 frozen paths and five historical documentation versions. The initial
Git whitespace check exited two for trailing blank lines in two unchanged
upstream fixtures. Their exact pinned Git blobs justify that narrow exception;
the failed check is preserved. All 1,551 committed paths, modes, and object IDs
match the reviewed index.

The 7,628,060-byte archive passed full readback of all 160 members. The
[publication execution](stage69-72-source-binding-execution.json) and its raw
channels are published separately. Source binding runs no guest evaluations or
Cargo checks and leaves the full M2 failures unchanged.

### Stage69–72 Java authority checkpoint

The [Java authority checkpoint](stage69-72-java-checkpoint.json) preserves 235 distinct authored queries against Pkl 0.32.0: 29 super entry defaults, 30 import type clauses, 81 qualified/generic types, 14 VarArgs boundaries, 47 base alias/Mixin controls, and 34 const owner cases. Two JVM repetitions remain separate raw observations. The nine alias argument-order queries share an audit and archive with the VarArgs controls; they are counted once.

Six finite archives preserve full raw results and logger channels, source/input identities, pinned JDK/JAR identities, and earlier failed capture attempts. Every archived member is read back and checked for payload, size, and mode. Original CLI controls remain linked to the separately published Stage67 evidence. The [actual publication execution](stage69-72-java-publication-execution.json) records command arguments, raw stdout/stderr bytes encoded as Base64, exit codes, source bookends, and the assembled checkpoint identity; it postdates the archives.

This checkpoint publishes Java reference evidence only. The separate implementation checkpoint above records Rust, native/Node/browser, and full ready-corpus results. All nine M2 workstreams and known failures stay open.

### Stage68 implementation and complete ready-corpus replay

The [implementation checkpoint](stage68-implementation-checkpoint.json) records
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

### Stage68 source binding and implementation publication

The [source binding](stage68-source-binding.json) links 167,307 recorded inputs
to implementation commit `723687b`, whose remote `main` identity was verified.
It checks 166,700 Git-origin rows by commit/path/mode/OID, independently checks
607 copied inputs and all 315 frozen runtime/Cargo files against the committed
bytes, and preserves three documents changed after validation. The report README
is separately bound as a publication document. This audit runs no guest or build.

The [6,832,706-byte C archive](stage68-source-binding-evidence.tar.gz) preserves
the source audit, actual B publication/assembly and commit/push records, and
corrected reviews. Full member payload/hash/mode readback passed. The first
commit attempt failed on one EOF blank line in `tools/runtime_fixture_links.py`;
the retry preserved its validated bytes and disabled only Git's `blank-at-eof`
lint rule. Other whitespace checks passed. Twelve inactive generated test
binaries remain recoverable from a separately identified local cache archive;
frozen sources and artifacts were unchanged.

The [C publication process](stage68-source-binding-execution.json) exited zero
with stable sources. Its raw stdout, stderr, and source maps are published
separately because they postdate the archive. M2's known failures, nine open
workstreams, product API, release-platform, and performance requirements remain
unchanged.

### Stage68 Base64, collection predicates, and method semantics — Java authority checkpoint

The [Java checkpoint](stage68-java-checkpoint.json) publishes independent Pkl
0.32.0 authorities for 89 String Base64, 47 collection emptiness, and 64 `every`
expressions. Each uses two JVM sessions. Expected ordinary error outcomes are
retained; `every` also preserves 39 ordered logger events per session with
UTF-16 channels. Separate captures cover entry-parent Host diagnostics and
object-method signature timing, arity, and lexical ownership.

The [publication](stage68-java-publication.json) binds the
[5,044,550-byte archive](stage68-java-evidence.tar.gz) to 21,962 captured source
files plus its manifest. Full member payload, size, mode, and source bookends
match. Failed and superseded capture attempts remain in the archive. Seven
reused original-program inputs and Java outputs are linked to pinned upstream
sources and exact members of the separately committed Stage67 full-inventory
archive. Their prior Rust failures are unchanged by this publication.

The standalone fixture verifiers use tracked inputs. The installed-layout
regression passes without a `target/` directory; the twelve predicate tests
pass in both ordinary and optimized Python modes. These checks establish
reference integrity. At this Java-only checkpoint, candidate execution and source binding had not yet completed.
This checkpoint claims no candidate compatibility, M2 completion, or performance
result. The separate Stage68 implementation result is recorded above.

### Stage67 String properties — Java authority checkpoint

The [Java checkpoint](stage67-java-checkpoint.json) seals 146 expressions at
`file:///stage67/string-properties.pkl`: 117 successful results and 29 ordinary
Pkl errors, with equal raw channels in two fresh JVM sessions. The cases cover
String blankness, emptiness, code-point length and last index, implicit and
explicit constraints, synthetic Reference types, and property/method error
ordering. The original 77 blankness expressions remain an ordered prefix.

A separate direct Java supplement preserves the 8,192-byte BMP membership
bitset for Pkl's 25-character White_Space set and its differences from JDK
whitespace predicates. It is separate from the public Pkl expression boundary.
The [history archive](stage67-java-evidence.tar.gz) retains the 23 Java evidence
directories, including corrected exploration and seal attempts. Full payload,
mode, and SHA readback passed; the [actual publication receipt](stage67-java-publication.json)
retains the publisher's exit status and raw channels.

The fixture verifier, seven tests in normal and optimized Python, inventory
freshness, and independent source/fixture review passed. This checkpoint claims
zero candidate compatibility passes. Rust, actual platform, full current
corpus, and M2 completion results remain separate; existing failures stay open.

### Stage67 implementation and complete ready-corpus comparison

The [implementation checkpoint](stage67-implementation-checkpoint.json) records
String blankness, emptiness, and code-point index properties in the shared engine.
All five required Rust checks passed: 1,008 distinct tests passed, none failed,
and two were ignored. Native, actual N-API, and Chromium each match 2,454 raw
observations from the 15-profile, 1,227-expression catalog. Existing Any/M1 Regex
mismatches remain failures.

The full ready acquisition completed 4,416 comparisons: **4,142 matches,
128 mismatches, 143 unsupported results, two Java oracle errors, and one Rust
internal error**. Original modules account for 625 matches out of 888; expression
strings account for 3,516 out of 3,527, and the JVM classpath case matches in both
recorded repetitions. All 131 previous original outcomes are unchanged, including
81 passes. Three inherited scalar regressions remain failed; separate Stage 68
lab fixes are not included in this candidate's results.

Strict `--verify-full` exited 1 with 1,802 unmet gate entries. The additional
111 unready cases and all nine M2 workstreams remain open. Evidence integrity
and the selected platform comparisons do not establish full language/API or
performance compatibility.

Thirteen archives cover 80 evidence directories. Each publisher verified every
payload byte, file mode, member set, and source bookend; each archive is below
95 MiB. Their identities and individual publication records are linked from the
checkpoint. The full-review first attempt failed on an omitted scalar adapter
field; its corrected review uses the source-bound manifest without changing any
captured result. The Java A authority remains separately committed. The corrected
independent publication review verifies that exact A checkpoint against its Git
blob while requiring every other checkpoint input to occur in the B archive union;
the first review attempt and its failure remain preserved.

The [C source binding](stage67-source-binding.json) links all 165,550 required
inputs to implementation commit `90b1580`, which was pushed and matched remote
`main`. It checks 165,541 Git-origin rows by commit/path/mode/OID, the nine copied
inputs by bytes and Git blob identity, and all 293 runtime/Cargo entries against
actual implementation blob bytes. Changed captured versions of `PLAN.md`,
`TESTING.md`, and `docs/m2-language.md` retain their old bytes and both Git
identities; this report README is classified separately as a publication document.

The [C archive](stage67-source-binding-evidence.tar.gz) preserves the source
proofs and actual publication, assembly, review, commit, and push receipts. The
[actual audit process](stage67-source-binding-process.json) completed with exit 0,
stable sources and Git HEAD; it and its raw channels are published separately
because they postdate the archive. This audit performed no guest evaluations or
builds. All results are recorded local macOS arm64 evidence, and M2 remains incomplete.

### Stage66 implementation and local comparisons

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

The [implementation checkpoint](stage66-implementation-checkpoint.json) records fourteen single-module Java
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
remain incomplete. The [Java checkpoint](stage66-java-checkpoint.json) and its original history remain separate from this implementation evidence.

Stage66 implementation and evidence are committed as `959c2f7`; its push and
remote `main` identity were verified. The twelve B archives total 281,284,982
bytes. Each publisher read back every payload, mode, and SHA; the independent
publication review verified the archive identities and exact inventories.

The [source-binding report](stage66-source-binding.json) connects 165,540 recorded
required-check inputs and all 291 runtime/Cargo freeze entries to that actual
implementation commit. It checks 165,524 Git-origin rows by recorded
commit/path/mode/OID, and additionally checks the bytes of sixteen copied inputs
and all 291 frozen entries. Five recorded document inputs retain their historical
bytes. `reports/m2/README.md` has separate A/B document provenance because it was
not a required-check input.

The [88-member archive](stage66-source-binding-evidence.tar.gz) preserves the
source proof and actual publication, assembly, review, commit, and push records.
The [binding process](stage66-source-binding-process.json) exited zero with stable
sources and Git HEAD; its terminal receipt and raw channels are published
separately because they postdate that archive. The [copy record](stage66-source-binding-publication.json)
verifies their exact published bytes. This closes the Stage66 checkpoint without
adding guest executions or changing the known failures and open M2 gates.

### Stage66 reversal and union diagnostics — Java authority checkpoint

The [Java checkpoint](stage66-java-checkpoint.json) pins four single-module
string profiles: reversal values (78), constraint calls (8), subscript diagnostics
(19), and base constructor resolution (36). Each profile retains two fresh JVM
runs with equal raw channels. Forty separate union-marker modules and eight
direct VM identity observations have distinct evidence boundaries.

The [history archive](stage66-java-evidence.tar.gz) preserves canonical Java
acquisitions, earlier failed/provisional attempts, validation, and the prepared
131-program original selection. The [publication receipt](stage66-java-publication.json)
records its actual publishing process. No Rust or portable compatibility result
is claimed by the Java checkpoint. The separate Stage66 implementation record
above supplies the candidate comparisons and retains their known failures.

### Stage65 inferred construction — implementation and local comparisons

Stage 65 connects contextual and explicit `new` to the shared resumable
type-default engine, including method-parent caching, inherited module self
types, receiver-sensitive aliases, default-key calls, and missing-default frames.
Canonical constructor parents remain traced roots owned by their code. Dedicated
first-use, failure/retry, retirement, and close tests check their ownership;
lifecycle tests measure user-object deltas after warming those canonical roots.
Collection equality continues to compare collection members independently of
builtin prototype backlinks.

The [implementation checkpoint](stage65-implementation-checkpoint.json)
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
The [source binding](stage65-source-binding.json) links 165,390 recorded Git
inputs and 69 copied inputs to the implementation and pinned references; two
historical documents retain their original bytes. All 280 runtime/Cargo freeze
entries match actual commit blob contents. Its [evidence archive](stage65-source-binding-evidence.tar.gz)
preserves 83 members, including the 73 later publication, review, and assembly
receipt files and their three failed attempts. The binding's own
[terminal process receipt](stage65-source-binding-process.json),
[stdout](stage65-source-binding.stdout.bin), and
[stderr](stage65-source-binding.stderr.bin) are separate post-archive records.
No builds or guest evaluations run during source binding; the earlier
compatibility and performance limits remain unchanged.

### Stage65 inferred construction — independent Java authority

The [Java checkpoint](stage65-java-checkpoint.json) publishes three distinct
authorities against pinned Pkl 0.32.0. The normal fixture contains 72 expressions
with 49 successes and 23 ordinary Pkl errors per fresh JVM. The amending-module
fixture contains seven successful observations and retains the real sibling
base module. Each profile has two raw-equal fresh JVM sessions.

The separate module-receiver fixture runs two opposite first-force orders,
each with eleven queries in two fresh JVMs. Each session has eight successes
and three ordinary errors. Direct `new module` and nullable construction retain
the first resolved closed child class; aliases retain the current receiver's
class. Captured absolute file URIs, sibling sources, full error bytes, and both
UTF-8 and UTF-16 channels remain intact. These observations do not cover the
separate inferred-method parent cache now under investigation.

All three complete offline verifiers pass. Sixteen adversarial tests pass in
normal and optimized Python, and the source inventory check passes. M2 inventory
freshness and candidate compatibility are separate requirements. The original
selection preparation preserves the existing 116-program prefix and fourteen
additional original default/type programs. None of those 130 programs has been
rerun for this checkpoint, and the two blocked package/HTTP/TLS obligations remain.

The [evidence archive](stage65-java-evidence.tar.gz) preserves authoring failures,
corrected explorations, canonical acquisitions, raw process records, verification,
and both unexecuted original-selection preparations. The actual
[publication receipt](stage65-java-publication.json) is separate because it
postdates that archive. The [independent review](stage65-java-review.json) reads
back all 28,952 members and verifies the fixture, process, and original-selection
boundaries. Its [actual execution](stage65-java-review-execution.json) preserves
the reviewed scripts and raw process bytes separately. No Rust, native, N-API, browser, full-language, API, or
performance pass is claimed here. Stage 64 remains the completed implementation
checkpoint; all nine M2 workstreams remain open.

### Stage64 amendment construction — implementation and local comparisons

The [implementation checkpoint](stage64-implementation-checkpoint.json) records
repeated property, entry, and predicate amendment bodies, class-valued parents,
and empty-result identity in the shared evaluator. Long amendment chains use
parser and compiler worklists under the existing 256 KiB frontend test limit.
Class construction retains abstract/external error precedence, generic defaults,
and prototype identity. `Listing.fold` validates its callback, shallow-forces
receiver values before callbacks, and preserves pending values through Host
suspension, reentry, GC, cancellation, and retry. Nested values remain lazy.
Diagnostics preserve the first authored body span, synthetic source-section
fallback, real caller frames, and unqualified base-class names.

Nine sealed Java string profiles contain 868 expressions. Native, actual N-API,
and Chromium each match all 1,736 observations across two sessions, comparing
seven raw channels and the whole process protocol. The independent acquisition
review checks 81,228 raw channel comparisons across those profiles and four
scalar groups. IntSeq, Bytes, Duration, and DataSize retain 474, 600, 548, and
510 observations per target. Any portable remains at 170 matches and two
`scalar-regex` mismatches per target; its strict comparison fails. M1's
architecture contract passes while its two Regex compatibility gaps remain.

The separate [prototype diagnostic fixture](../../tests/fixtures/m2-prototype-diagnostics/README.md)
contains 39 Java rows per fresh JVM: 35 ordinary errors and four successful
values. Two JVM captures retain 1,721 archive members and 70 copied source
inputs. The focused Rust run compares 259 Java rows across construction,
declarations, frontend controls, and prototype diagnostics, and passes 53
top-level tests. These distinct scopes are not additional whole-program passes.

All five required Rust checks pass. The workspace has 972 distinct passing
tests, zero failures, and two ignored tests. Its raw total of 985 contains
thirteen nested child summaries also represented by parent tests. The new
audit corrects the earlier distinct totals for Stage 62 from 964 to 956 and
Stage 63 from 971 to 963. Raw logs and historical checkpoint JSON are preserved;
this correction changes the accounting, not the observed test outcomes.
Tool validation passes 94 Python tests normally and under `-O`, 49 JavaScript
tests, eight mapping tests in both Python modes, and both inventory checks.

The original selection contains 116 programs: 52 match, 30 mismatch,
33 are unsupported, and one fails in Java under the unchanged `-Xmx512m`.
All 37 prior passes remain passes. `generators/spreadSyntaxDynamic.pkl` and
`basic/amendsChains.pkl` change from mismatch to pass; the added 25 have
thirteen matches, eleven mismatches, and one unsupported result. Two required
package/HTTP/TLS cases remain unexecuted. Independent review checks all 464
stdout/stderr streams and the first raw difference for every nonpass. The
selected original gate remains failed. New object-construction scope and
inference failures, including the unqualified `getClass` call in
`basic/newInAmendingModuleMethod.pkl`, remain implementation work.

The 29 reviewed mapping entries remain partial and expand to 42 partially
mapped requirements, with 433 requirements unimplemented. The inventory keeps
475 requirements and 4,527 case rows: 4,416 ready, 71 unimplemented, and
40 unmapped. Ready means connected to an execution adapter, not passed.

Publication preserves the first Rust semantic regression, the second run's
ENOSPC and incomplete report, the third run's damaged build artifact, and the
successful fourth run. The original damaged bytes and verified cache-recovery
metadata remain available. Unexecuted preparations remain unexecuted, and the
comparison command's initial argument error remains separate from its successful
corrected invocation. The later Java-authority publication receipts are included
explicitly in the supplement. Actual implementation publication, assembly, and
Git source binding have separate process evidence.

These are local macOS arm64 results. All nine M2 workstreams, full language
compatibility, typed exports, product APIs, release platform coverage, and
speed/memory targets remain open.

The implementation and evidence were committed and pushed as
`1afd1648daba3b31fe204bf1974fcb0b6fb2fb03`. The separate
[source binding](stage64-source-binding.json) checks 178,960 rows across ten
source categories: 178,889 match Git blobs associated with B, 69 match archived
execution inputs at nineteen distinct paths, and two preserve exact A-version
document bytes. It also checks the recorded source contracts of twelve native
and fourteen portable captures against that bound source set.

The [146-member binding archive](stage64-source-binding-evidence.tar.gz) retains
the actual publication and assembly records, four failed reviewer-tool attempts,
and both failed binding attempts. The first binding attempt rejected a legitimate
pinned Git symlink; the next failed while formatting a relative layout path.
The successful third attempt compares the symlink's raw target bytes and Git
mode without following it and resolves the layout path at entry. Reference
checkouts and recorded evaluation results are unchanged.

The [independent review](stage64-source-binding-review.json) checks every binding
archive member and all source rows against actual Git contents. The successful
[binding execution](stage64-source-binding-execution.json) and
[review execution](stage64-source-binding-review-execution.json) retain exact
process and stream bytes separately because they postdate the binding archive.
The review execution also preserves the executed review scripts. These checks
run no guest evaluations and do not close the remaining M2 or performance gates.

### Stage64 amendment construction — independent Java authority

The [Java checkpoint](stage64-java-checkpoint.json) records two separately sealed
Pkl 0.32.0 authorities. The [construction fixture](../../tests/fixtures/m2-amendment-construction/README.md)
contains 188 ordered string calls per fresh JVM: 102 successful values and 86
ordinary errors. The [declaration fixture](../../tests/fixtures/m2-amendment-declarations/README.md)
contains eleven separate modules, each evaluated in two fresh JVMs, with three
successful values and eight ordinary errors. Both fixtures retain all producer
UTF-8 and UTF-16 channels, process streams, pinned JAR/JDK identities, and 74
copied source inputs. Only the construction fixture joins the shared comparison
catalog, as its ninth profile.

The [evidence archive](stage64-java-evidence.tar.gz) preserves 54,117 members,
including exploratory failures, the canonical authorities, twelve direct VM
identity observations per JVM, and separate Host trace evidence. Four expression
queries use the unchanged upstream `basic/amendsChains.pkl` source; they do not
execute its complete CLI/generated-file contract. Separate local-amendment and
EOF controls contain eighteen and three rows respectively. Their two JVM
repetitions match, including actual LF transport and invalid source-section
coordinates. These supplements retain their distinct public-expression, direct
VM, and logger boundaries.

Five recorded Java/tool validation commands pass: both fixture verifiers,
37 Python tests normally and under `-O`, and ten JavaScript tests. The 199 copied
inputs remain stable. The separate M2 manifest freshness command exits one while
runtime changes are in progress; no freshness pass is claimed. Independent review
reads every archive member and checks its identity against the physical inputs.
The actual publication receipt is retained outside its own archive for later
implementation evidence. This checkpoint establishes Java authorities and tooling;
Rust, portable, full M2, product API, and performance results remain separate.

### Stage63 collection addition — implementation and local comparisons

The [implementation checkpoint](stage63-implementation-checkpoint.json) records
List/Set concatenation, Map merging, nonforcing invalid-operand diagnostics,
and Set `first`, `last`, `firstOrNull` and `lastOrNull`. Collection addition
preserves insertion order and duplicate/key representatives, returns the left
object for an empty right operand, and retains operands and pending hash or
equality work across Host suspension, reentry, GC, cancellation and retry.
New strict storage does not inherit the producer's executable environment or
cached structural metadata.

Eight sealed Java profiles contain 680 expressions. Native, actual N-API and
Chromium each match all 1,360 observations across two sessions, comparing seven
raw channels and whole process protocols. Existing scalar regressions retain
IntSeq 474, Bytes 600, Duration 548 and DataSize 510 matches on each target.
Any portable retains 170 matches and two `scalar-regex` mismatches on each
target, with strict comparison failed. The M1 architecture contract passes
while its two Java Regex compatibility differences remain failures.

All five required Rust checks pass. The raw workspace harness reports 976
passes; thirteen nested child summaries are also counted by their parent.
The corrected distinct result is 963 passed, zero failed and two ignored.
The Stage 64 audit corrects the earlier reported 971 without rewriting the
historical raw archive or checkpoint JSON. Tool validation
passes 75 Python tests normally and under `-O`, 49 JavaScript tests, eight
mapping tests in both Python modes, and both inventory checks. Its incomplete
first tracing attempt is preserved separately.

The original 91-program comparison has 37 matches, 21 mismatches,
32 unsupported results and one Java heap error. The prior 90 improve from
35 to 36 matches because `basic/set.pkl` now passes, with no prior pass
regressing. The newly selected `basic/map.pkl` passes. Java `api/list.pkl`
continues to fail under the unchanged `-Xmx512m`; two further package/HTTP/TLS
originals remain required and unexecuted. The selected original gate fails.

Two reviewed partial operator/collection mappings bring the mapping entries
from 27 to 29. The manifest changes 63 values and retains 4,527 case rows,
475 requirements, 4,416 ready rows, 71 unimplemented rows and 40 unmapped rows.
These overlapping accounting rows and repeated observations are not a whole
language completion percentage.

Six evidence groups preserve validation, native collections, native scalar
regressions, historical platform acquisition, final platform acquisition and
closeout. Platform01 used an absolute Python executable where the existing
verifiers require literal `python3`; comparison01 stopped before comparing
guest rows. Fresh platform02 and comparison02 satisfy that unchanged contract.
Original failures, focused development failures, producer histories and
independent review remain visible. Publication and checkpoint assembly have
separate actual process logs. The first checkpoint assembly rejected two later
Java publication receipts absent from the six archives. A separate
[assembly supplement](stage63-assembly-support-publication.json) preserves
those receipts and raw channels, the failed assembly process, and the corrected
preparer. The corrected checkpoint requires every target input to match the
union of all seven archives. Cache recovery preserves removed build binaries
in verified archives and claims no compatibility result.

These results are recorded local macOS arm64 evidence. Full M2, typed exports,
product APIs, release platform coverage and speed/memory targets remain open.

The implementation and evidence were committed and pushed as
`1f1142b6b8d61d09825e44ed0e7efd6f239d4020`. The separate
[source binding](stage63-source-binding.json),
[binding evidence](stage63-source-binding-evidence.tar.gz), and
[independent review](stage63-source-binding-review.json) connect actual Git
objects with the recorded inputs. Across 45 categories, 183,531 rows match B,
230 identify explicitly archived execution tools, and three retain verified
base-document bytes. These repeated source rows are not feature counts.
The 56-member binding archive includes the real publication and assembly
process logs, preserving the failed first assembly and successful successor.
This audit reruns no guest evaluations and does not close the remaining gates.

### Stage63 collection addition — independent Java authority

The [Java checkpoint](stage63-java-checkpoint.json) seals 125 ordered public
string calls per fresh JVM: 86 successes and 39 ordinary Pkl errors. Two sessions
match all raw result channels and the complete process protocol. The
[fixture](../../tests/fixtures/m2-collection-addition/README.md) covers List/Set
cross-kind addition, Map replacement order and key representatives, hash
structure boundaries, lazy values, and invalid-operand diagnostics. The source
archive retains all 5,308 members and 45 copied source inputs.

A separate direct Java runtime supplement records 14 reference observations in
each of two fresh JVMs. It distinguishes returning the left wrapper for an empty
right operand from creating a new wrapper for nonempty merges. These internal
observations are separate from public string and typed-export compatibility.

The [evidence archive](stage63-java-evidence.tar.gz) retains exploratory captures,
the authored module-local helper failure, both identity attempts, and nine
recorded validation commands. All nine commands pass with 421 stable copied
inputs: fixture verification and seven fixture tests normally and under `-O`,
24 shared comparator tests in both modes, ten JavaScript tests, the identity
supplement check, and source inventory freshness. Actual publication command,
stdout, stderr, and exit are recorded separately. M2 implementation-link
freshness waits for the concurrent runtime freeze. This Java authority claims
no Rust, Node, browser, full M2, or performance compatibility pass.

The separate [Set endpoint Java checkpoint](stage63-set-endpoints-java-checkpoint.json)
seals 70 string observations per session: 47 successes and 23 ordinary errors.
Its [fixture](../../tests/fixtures/m2-set-endpoints/README.md) covers `first`,
`last`, `firstOrNull`, and `lastOrNull`, including empty and null-containing Sets,
insertion order, duplicate representatives, unions, lazy values, and List
controls. Errors raised while constructing a Set are distinguished from endpoint
evaluation. Two fresh JVM sessions match all raw channels and process output.

The [endpoint evidence](stage63-set-endpoints-java-evidence.tar.gz) retains the
first canonical capture's incorrect archived producer copy and its corrected
successor. Eight recorded checks pass with 415 stable source copies. The final
fixture has 2,990 verified members; the separate evidence archive has 11,926.
This eighth comparison profile preserves the existing seven authorities and
claims no Rust candidate or full M2 pass.

### Stage62 collection conversions — implementation and local comparisons

The [implementation checkpoint](stage62-implementation-checkpoint.json) records
shared strict object materialization, lazy conversions, selector-based maps,
Dynamic property queries, Listing/Mapping optional and defaulted lookup,
Dynamic length, Set addition, and signed Bytes iteration versus unsigned spread.
The VM retains conversion and callback state across Host suspension, reentry,
GC, cancellation and retry. Present null values bypass fallback; missing keys
force the inherited Function1 default with its original diagnostic frames.

Six sealed Java profiles contain 485 expressions. Native, actual N-API and
Chromium each match all 970 observations across two repetitions, including
seven raw channels and complete process protocols. A late review reproduced
an authored-NUL/synthetic-surrogate property collision in the first candidate.
The final candidate uses an LF-prefixed injective lookup encoding and preserves
original PklString names for output. The rejected candidate, failed attempts,
and the test-only URI correction remain archived separately.

All five required local Rust checks pass. Workspace output contains 969 passed
harness executions and two ignored tests; removing thirteen nested child
summaries duplicated by their parent leaves 956 distinct passes, zero failures,
and two ignored tests. The Stage 64 audit corrects the earlier reported 964;
the historical raw archive and checkpoint JSON remain unchanged.
Python tooling has 61 passes normally and under `-O`,
JavaScript has 16 passes, and inventory checks pass. The final source inventory
contains 165,344 reconstructible entries with 22 inputs copied directly.
The M2 manifest changes 102 implementation-link hashes only; its 4,527 cases,
475 requirements, 4,416 ready rows, 71 unimplemented rows and 40 unmapped rows
are unchanged. These accounting rows are not a completion percentage.

Existing IntSeq, Bytes, Duration and DataSize string regressions retain 474,
600, 548 and 510 matches per target. Any portable retains 170 matches and two
Regex mismatches per target. The M1 experiment contract passes while strict
Java compatibility retains its two Regex gaps.

The unchanged original selection has 35 matches, 22 mismatches, 32 unsupported
results and one Java oracle error across 90 programs. The prior 62 improve
from 26 to 30 matches, with no previously passing case regressing. The four new
passes are element generation, Listing and Mapping spread, and object hashing.
The earlier 17-program subset still has 15 matches; two Binary-path mismatches
now reach unsupported `pkl:pklbinary` and remain failures. The historical
Stage60-to-Stage62 comparison is preserved separately from current progress.
`api/list.pkl` fails in Java with `OutOfMemoryError` under recorded `-Xmx512m`;
Rust reaches missing `List.isNotEmpty`. Neither outcome is counted as a pass.
Two package/HTTP/TLS-dependent original cases remain required and unexecuted.

These are recorded local macOS arm64 results. The four implementation evidence
groups retain failed and superseded captures, source/artifact identities and
independent review. Full M2, typed product APIs, other release platforms, and
the speed and memory targets remain incomplete.

The implementation and its evidence were committed and pushed as
`aac28e5b8d7a2182bc3e385410ff12228f246fb0`. The separate
[source binding](stage62-source-binding.json) and
[binding evidence](stage62-source-binding-evidence.tar.gz) compare actual Git
objects with the frozen inputs and committed archives. Of the 165,344 Rust
validation input rows, 165,341 match the implementation commit and three retain
verified historical document bytes. The 36 source categories, 14 execution
input proofs, and four capture-specific portable inputs retain their distinct
provenance; generated execution inputs are not claimed as tracked source files.

The failed first binding attempt remains preserved. Original checkpoint assembly
stdout and stderr were not saved as local files. A separately recorded later
read-only verification checks the existing checkpoint and publications; it does
not reconstruct the original assembly process or rerun language evaluations.
The [independent binding review](stage62-source-binding-review.json) found no
discrepancies. Source binding leaves the compatibility failures and full
milestone gates open.

### Stage62 collection conversions — independent Java authority

The [Java checkpoint](stage62-java-checkpoint.json) seals 236 ordered public
expression-string calls in each of two fresh JVM sessions: 158 successes and
78 ordinary Pkl errors per session. The raw repeats match across 4,958 files
per session. Inputs cover strict object materialization, selector-based maps,
lazy object conversions, Dynamic length, Set addition, and Bytes for/spread
iteration. They preserve forcing and callback order, inherited member behavior,
and the distinction between missing properties and explicit Pkl null.

The [fixture](../../tests/fixtures/m2-collection-conversions/README.md) records
55 source copies with the pinned Java runtime and execution environment. Its
21 producer channels include exception metadata; candidate string comparisons
use seven common raw channels and the complete ordered process protocol.
Typed export and wrapper identity comparisons are separate requirements.

The [validation archive](stage62-java-validation.tar.gz) preserves five passing
fixture/tooling and inventory checks, including seven Python regressions run
normally and under `-O`, with 70 stable source copies. The
[history archive](stage62-java-history.tar.gz) preserves authored captures
01–03 separately from canonical capture 04. Both archives passed full member
readback. The M2 implementation-link manifest check is deferred until the
concurrent Rust implementation is frozen. This checkpoint claims zero candidate
compatibility passes; Stage62 runtime, platform, and original-program validation
remain in progress.

The separate [converted-object controls checkpoint](stage62-controls-java-checkpoint.json)
records 58 additional Java string expressions: 44 successes and 14 ordinary
errors per fresh JVM, repeated twice. Its controls distinguish target classes,
the receiver used for inherited defaults, missing versus explicit null values,
property versus subscript access, and an unpaired UTF-16 surrogate from U+FFFD.
The [evidence archive](stage62-controls-java-evidence.tar.gz) preserves the
initial invalid-surrogate syntax capture, the corrected capture, and five
passing validation checks with 78 stable source copies. Six fixture tests pass
normally and under `-O`; all archive members passed independent readback.
The original 236-row authority is unchanged. Both checkpoints remain Java
evidence and do not claim candidate compatibility or typed export identity.

The [lookup checkpoint](stage62-lookups-java-checkpoint.json) adds 133 independent
Java expressions for Dynamic property queries and Listing/Mapping optional and
defaulted lookup. Each of two fresh JVM sessions records 82 successes and 51
ordinary errors; all 2,795 raw files per session match between repetitions.
The controls retain missing versus null values, callback types and first-error
order, hidden declarations, UTF-16 names, and Java's cache-dependent
`hasProperty` behavior for string entries. The original two fixtures are unchanged.
Five validation checks pass, including seven Python tests normally and under
`-O`, with 82 stable source copies. The [evidence archive](stage62-lookups-java-evidence.tar.gz)
preserves earlier authored captures and the first publication attempt; a history
description was corrected from four to five receiver-expression changes.
All 16,683 members passed independent readback. Candidate, platform, original
program, and full M2 verdicts remain separate from this Java authority.

The [default-function checkpoint](stage62-default-contracts-java-checkpoint.json)
seals 22 additional Java expressions: 12 successes and ten ordinary errors in
each of two fresh JVMs. Listing and Mapping defer inherited `Function1` checks
until a missing key invokes the default; present values, including null, bypass
invalid defaults. The cases preserve key/value error precedence and declaration
frames for nonfunctions and wrong function arities. Five fresh checks pass with
85 stable validation source copies, including six Python tests normally and
under `-O`. The [evidence archive](stage62-default-contracts-java-evidence.tar.gz)
also preserves a 16-row exploratory default probe and a separate ten-row
Dynamic rendering probe; these are not added to the canonical 22-row count.
Its 2,360 members passed independent readback. Candidate and full M2 gates
remain open.

The separate [Dynamic rendering checkpoint](stage62-dynamic-render-java-checkpoint.json)
seals ten successful expressions in each of two fresh JVMs, with 68 pinned
source files including the renderer and identifier lexer. Raw UTF-16 channels
distinguish high and low unpaired surrogates, U+FFFD, and NUL in property names;
Java UTF-8 replaces each unpaired surrogate with `?` and cannot establish that
distinction by itself. Five validation checks pass with 85 stable source copies,
including five Python tests normally and under `-O`. The
[evidence archive](stage62-dynamic-render-java-evidence.tar.gz) preserves the
earlier target-only probe separately. All 1,105 members passed independent
readback; candidate comparison and full rendering compatibility remain open.

The [property-name checkpoint](stage62-property-names-java-checkpoint.json) adds
26 successful expressions in each of two fresh JVMs. They distinguish authored
NUL identifiers from lone-surrogate properties and from LF, CR, backtick, U+2028
and U+2029 String keys. Pinned Java permits NUL inside quoted identifiers; the
first Rust candidate incorrectly treated NUL as an unrepresentable authored
name and used it to prefix synthetic identifiers. That candidate's passing
earlier matrices and required Rust checks remain superseded evidence.
The [Java evidence archive](stage62-property-names-java-evidence.tar.gz) preserves
the narrower 22-row capture and both exploratory collision probes separately.
Only their identical query/module bytes are claimed; exploratory Java and Rust
module URIs differed. Five validation checks pass with 90 stable source copies,
including six Python tests normally and under `-O`. All 3,251 archive members
passed independent readback. The six sealed profiles contain 485 selected
expressions; corrected-candidate, platform and whole-language verdicts remain
separate.

### Stage61 IntSeq values — implementation and local comparisons

The [implementation checkpoint](stage61-implementation-checkpoint.json) adds
shared IntSeq construction, step/map/fold/conversion, equality and hashing,
types, diagnostics, rendering, and streaming generator/spread behavior.
Iterator and generator overflow semantics remain distinct as in pinned Java.
Resumable continuations retain callback and accumulator roots, and synthetic
Listing environments survive Host suspension and GC. All 17 `pkl:math`
constants and the required List `add`, first/last accessors, nullable accessors,
and `fold` are implemented in the shared engine. External math functions remain
unsupported, and no new dependency was added.

The [Java authority](stage61-java-checkpoint.json), committed as
`8656c3f28fe6a664c7c744ff15f9f63362bf1ff8`, separates 237 string expressions
(198 successes and 39 ordinary errors), three export-rejection observations,
17 math constants, and 21 List.add expressions. The
[List accessor/fold supplement](stage61-list-accessor-fold-java-checkpoint.json),
committed as `2faf0b22b60a22af53e22af3d2cc1a9d27de667d`, records 65 expressions
(37 successes and 28 ordinary errors). Each Java fixture has two fresh sessions.
Producer exception/export channels remain distinct from candidate string passes.

Native, N-API, and actual Chromium each match all 474 IntSeq string observations
and the complete ordered protocol. The existing Bytes regression now matches
600 of 600 per target, including its three previously failing IntSeq cases in
both repetitions. Duration and DataSize retain 548 and 510 matches per target.
Portable Any retains 170 matches and two scalar Regex mismatches per target;
its strict gate fails. M1 retains 6,076 Java matches out of 6,078 per target,
with its two Regex differences and a passing architecture contract.

The original cohort includes the previous 17 programs plus all 47 pinned input
snippets containing `IntSeq`, with two overlaps: 62 programs, 26 matches,
15 mismatches, and 21 unsupported results. Its previous 17-program subset now
has 15 matches and two mismatches. `basic/bytes.pkl`, `api/bytes.pkl`, and
`api/duration.pkl` newly match; no previously passing case regresses.
The unchanged basic/API IntSeq programs also match. Each original ran once
per runtime; offline verification is not an additional guest execution.
The selected and full gates remain failed despite stable sources, artifacts,
oracle, and workspace. The closeout archive retains each nonpass's raw channels
and first failure, including missing Bytes iteration, Dynamic.length,
List.toMap in the mapping-spread program, hashCode-program collection methods,
and renderer dependencies.
Binary programs still fail at `module.toMap()` before encoding.

All five required checks pass locally with Rust 1.98.1: 953 passing workspace
test executions and two ignored tests. The Bytes IntSeq and Any IntSeq ignored
regressions now execute and pass; the existing probe and Regex ignore remain.
The original 256 KiB frontend checks retain their limit and inputs. Focused
native regressions cover 17 math string/numeric bit cases, 21 List.add cases,
65 List accessor/fold cases, and Host/GC/cancellation/retry lifecycles.
They are not separate portable supplement or typed product export comparisons.
Tool checks pass 37 Python tests normally and under `-O`, plus 13 JavaScript
tests. Early build, probe-input, and fixture-authoring failures are preserved
separately from final results.
Inventory regeneration changes only 101 implementation-link hashes; original
case IDs, requirements, and execution/mapping states remain unchanged.

The [validation](stage61-validation-evidence.tar.gz),
[native](stage61-native-evidence.tar.gz),
[platform](stage61-platform-evidence.tar.gz), and
[closeout](stage61-closeout-evidence.tar.gz) publications preserve recorded
sources, artifacts, raw outcomes, and complete member readback. These are local
macOS arm64 results, not hosted CI. All nine M2 workstreams retain unmet exit
criteria. Typed exports, complete APIs and standard library, release-platform
coverage, and execution-time/peak-memory targets remain open. The inventory's
4,416 ready rows out of 4,527 are execution mappings, not passing results or
a completion percentage.

Implementation and all four evidence publications were committed as
`9e524d2988caa57e9e16733aec8f3f4b17804b94` and pushed to remote `main`.
The [Git source audit](stage61-source-binding.json) verifies 165,289 runtime
validation inputs against that commit, with four captured documents separately
retained from the verified base. Native build inputs (269), M1 inputs (242),
platform wrapper inputs (573), original-module inputs (4,590), and tool inputs
(62) match the implementation. The comparison has 642 matching source inputs
and one generated wrapper proved by its exact committed archive bytes.
All four implementation archives and 6,408 copied sources were reread; six
execution wrappers have separate archive proofs. The
[binding archive](stage61-source-binding-evidence.tar.gz) also preserves the
checkpoint assembly's raw successful command result. This checks recorded
provenance and does not rerun evaluation or close any remaining M2 requirement.

### Stage60 Bytes values — implementation and local comparisons

The [implementation checkpoint](stage60-implementation-checkpoint.json) adds
shared Bytes hashes, encoding, strict charset decoding, indexing,
concatenation, List conversion, and resumable `List.toBytes` UInt8 checks.
Heap-local cache links preserve List ownership across Host suspension and GC.
The compiler also avoids retaining large generic expression frames while
compiling source-backed alias annotations and scalar literals.

The [Java checkpoint](stage60-java-checkpoint.json), committed in
`b2301be5fe074b423f3baddfaa701aad0ecd5641`, seals 300 ordered expressions in
each of two fresh JVM sessions: 223 successes and 77 ordinary errors per session.
Native, N-API, and actual Chromium each match 594 of 600 string observations.
Three IntSeq cases fail in both repetitions; strict string compatibility fails.
Seven common raw channels and the complete protocol are compared. Additional
Java exception/cause metadata remain producer-only observations. The requested
portable environment uses the Java capture keys and effective session flags;
full OS-injected Java environment identity is unverified.

Duration and DataSize string regressions match 548 and 510 observations on each
target. Portable Any retains 168 matches and four Regex/IntSeq mismatches per
target. M1 retains 6,076 Java matches out of 6,078 per target and two regex
differences; its architecture contract passes while compatibility fails.

The unchanged original selection has 12 matches, four mismatches, and one
candidate-unsupported result. `errors/invalidBytes4.pkl` newly matches. The
basic and API Bytes programs reach missing `IntSeq` calls at lines 68 and 3,
respectively. Both Binary programs still fail at `module.toMap()` before
encoding, and the Duration API program still requires `pkl:math`. There is
one Java and one Rust execution per original program, separate from repeated
authored expression sessions.

All five required Rust checks pass locally with Rust 1.98.1: 940 passing test
executions and four ignored tests. The ignored Bytes test retains all three
IntSeq failures, separately from the 297 exact focused comparisons. The original
256 KiB frontend test passes after the compiler fix without changing its limit
or input. The initial stack failure and subsequent disk-full evidence write
remain separate failures. Tool checks pass 25 Python tests normally and under
`-O`, plus 19 JavaScript tests. Inventory updates change only 69 implementation
link hashes.

The [validation](stage60-validation-evidence.tar.gz),
[native](stage60-native-evidence.tar.gz),
[platform](stage60-platform-evidence.tar.gz), and
[closeout](stage60-closeout-evidence.tar.gz) publications preserve source and
artifact identities, raw captures, earlier failed attempts, and complete member
readback. These are local macOS arm64 results. Typed exports and mutable Java
array aliasing, complete wrapper identity, full M2, product/release platforms,
and speed/peak-memory targets remain open.

The implementation and four evidence publications were committed as
`64df1bfbe3b620f2e9393d1f883fd48f5342b4f3` and pushed to remote `main`.
The [Git source audit](stage60-source-binding.json) verifies 165,254 runtime
validation inputs against that commit and retains four documents from the
recorded base commit. Native build inputs (263), M1 inputs (237), platform
wrapper inputs (522), original-module inputs (4,584), and tool inputs (218)
match the implementation. The comparison has 327 matching source inputs and
one generated execution wrapper verified by its exact committed archive bytes.
All four implementation archives were fully reread. The
[binding evidence](stage60-source-binding-evidence.tar.gz) preserves the failed
first audit and corrected result. This audit does not rerun tests or evaluation.

### Stage59 Duration values — implementation and local comparisons

Stage 59 adds the shared `Duration` value and VM paths for seven units,
conversion, arithmetic, ordering, equality and hashed keys, `isoString`,
`isBetween`, type checks, inherited `Any` methods, and complete selected
diagnostics. The [Duration contract](../../docs/m2-language.md) separates
Java string observations, exported raw-double observations, Rust internal
value checks, and the required typed product APIs.

The [Java checkpoint](stage59-java-checkpoint.json) seals 274 ordered public
expression-string calls in each of two fresh JVMs, with 240 successes and 34
ordinary Pkl errors per session. A separate 15-row Java export supplement
records raw double bits and units twice. The [validation archive](stage59-java-validation.tar.gz)
retains eight passing Java fixture/tooling and inventory checks; the
[historical archive](stage59-java-history.tar.gz) preserves the superseded
263-row exploration. Both archives passed full member readback. The original
upstream `basic/duration.pkl` and `api/duration.pkl` programs were reviewed,
but were not inputs to these authored Java API calls and have not been replayed
by this checkpoint. The canonical source snapshot includes unrelated
`reference.pkl` files in place of those Duration programs; the validation
snapshot records the actual Duration source files.

The shared Rust implementation passed the five required local checks with
Rust 1.98.1: 932 passing workspace test executions, no failures, and three
ignored tests. The [local validation publication](stage59-validation-publication.json)
preserves its raw outputs and source identities. Final complete-session
comparisons match all 548 of 548 Duration string calls and the whole protocol
on each of native, N-API, and actual Chromium. The existing DataSize string
regression matches 510 of 510 on each target. On N-API and Chromium, the existing
`Any.ifNonNull` string selection retains 168 matches and four Regex/IntSeq
constructor mismatches per target. The M1 platform regression retains 6,076
Java matches out of 6,078 and its two regex differences per target.

The 17 unchanged original Java/Rust module comparisons have 11 matches, five
mismatches, and one candidate-unsupported result. `basic/duration.pkl` newly
matches; `api/duration.pkl` reaches its line 77 `pkl:math` use and remains
unsupported. Three Bytes cases and two Pkl Binary `module.toMap()` cases keep
their earlier first failures. These original programs have one execution per
runtime and are distinct from the 274-query sessions. Tool validation passes
26 unique Python tests normally and under `-O`, plus 17 JavaScript tests;
the M2 inventory differs only in 49 implementation-link hashes.
Selected results do not establish typed export/API parity, full M2
compatibility, release-platform coverage, or performance targets.

The [implementation checkpoint](stage59-implementation-checkpoint.json) and
[validation](stage59-validation-evidence.tar.gz),
[native](stage59-native-evidence.tar.gz),
[platform](stage59-platform-evidence.tar.gz), and
[closeout](stage59-closeout-evidence.tar.gz) archives were committed in
`dd0c4492c635afacc286c1bbf5ac7877ba517f40`. Every archived member was read back
and verified. Earlier captures with incomplete tool-source catalogs and the
failed comparison admission remain separate from the final results.

The [Git source audit](stage59-source-binding.json) binds the final runtime,
build, platform, original-module and comparison inputs to that implementation
commit. Rust validation has 165,245 matching inputs and four documents retained
from the verified base commit. Tool validation has 339 matching inputs and one
ignored execution wrapper proved by its committed archive bytes, rather than
a source-tree entry. The failed first audit and corrected audit are preserved
in the [binding evidence](stage59-source-binding-evidence.tar.gz). This audit
checks recorded sources and artifacts; it does not rerun evaluation or tests.

The earlier [Stage58 implementation checkpoint](stage58-implementation-checkpoint.json)
adds inherited `Any.ifNonNull` to the shared runtime, with ordinary lookup,
const restrictions, callback checks, preserved null defaults and Host/GC
continuations. See the [development contract](../../docs/m2-any-if-non-null.md).

Native, N-API and Chromium each match 168 of 172 canonical query executions.
The Regex and IntSeq constructor rows mismatch in both repetitions, and the
whole session protocols retain those differences. Evidence integrity passes;
strict string compatibility fails. Thirteen additive Java observations pass
their Rust regression test; that supplement has no separate portable result.
The [Java checkpoint](stage58-java-checkpoint.json) records its Python acquisition
helper provenance limitation without weakening the sealed Java raw observations.

The unchanged original selection now has ten CLI matches and seven mismatches.
`api/any.pkl` passes after the shared method change. Bytes and Duration operations
and the Binary test's `module.toMap()` precondition remain failures. Each of the
17 original programs was executed once in Java and once in Rust; offline
verification does not add guest repetitions. This is not a current whole-language
verdict. DataSize string regressions match all 510 observations on each target.

All five required Rust checks pass locally with Rust 1.98.1: 929 passing test
executions, no failures and three ignored tests, including the two explicit
constructor gaps. The final tool checks cover 20 unique Python tests and 17 JS
tests. The initial Python invocation error is retained in the validation archive.
M1 still records 6,076 Java matches out of 6,078 per target and both known regex
differences. Its architecture contract passes while strict compatibility fails.

The [validation](stage58-validation-evidence.tar.gz),
[native](stage58-native-evidence.tar.gz),
[platform](stage58-platform-evidence.tar.gz), and
[closeout](stage58-closeout-evidence.tar.gz) archives include raw captures,
source/build identities, original programs and failed gates. Their publication
records verify every archived member. Full M2, typed exports, complete product
APIs, release platforms, speed and peak-memory targets remain open. These are
local macOS arm64 records, not hosted CI results.

The implementation and evidence were committed as
`b0b6143ac978e14e80e6c3f7229640a6734d7f59` and pushed to remote `main`.
[Actual Git source binding](stage58-source-binding.json) verifies every Rust
validation input origin; 165,227 match that implementation and four documentation
inputs retain verified earlier Git bytes. All 257 native build inputs, 232 M1
inputs, 253 platform wrapper inputs, 4,579 original comparison inputs and 25 final
comparison inputs match the implementation. The four published evidence archives
also match its Git blobs. The [binding archive](stage58-source-binding-evidence.tar.gz)
contains the detailed results. The original precommit checkpoint and audit remain
historical records; source binding does not rerun checks or establish full M2.

### Stage57 DataSize values

The [Stage57 implementation checkpoint](stage57-implementation-checkpoint.json)
adds the shared DataSize value, conversion, arithmetic, comparison, type,
rendering and diagnostic paths, plus related Bytes construction and cached size.
The [DataSize contract](../../docs/m2-data-size.md) defines the exact boundaries.

Native, Node N-API and Chromium each match all 510 selected string observations:
two ordered 255-query sessions, with 233 successful values and 22 ordinary errors
per session. Complete query bytes, UTF-16, status, result/error payloads and session
protocols match the independent Java string authority. Typed public exports remain
required and unexecuted; Java export observations are not Rust API passes.

The unchanged original selection has nine raw CLI matches and eight mismatches.
Both `basic/dataSize.pkl` and `api/dataSize.pkl` pass. Failures remain in Bytes and
Duration operations, `Any.ifNonNull`, and the Pkl Binary test's `module.toMap()`
precondition. These are 17 Java and 17 Rust executions, not a current whole-language
result. No encoding compatibility is inferred from failures before encoding.

All five required Rust checks pass locally with Rust 1.98.1: 921 passing test
executions and one ignored probe. Disk exhaustion, corrupted compiler-cache launch
failures, and the first browser HTTP404 capture remain preserved. The corrected
capture reuses the same frozen Wasm/N-API binaries and records its changed transport
source separately. M1 retains two Java regex mismatches per target despite a
passing architecture contract. Full M2, product APIs, other release platforms,
speed and peak-memory requirements remain open.

[Validation](stage57-validation-evidence.tar.gz),
[native comparison](stage57-native-evidence.tar.gz),
[earlier native candidates](stage57-native-history-evidence.tar.gz),
[platform comparison](stage57-platform-evidence.tar.gz), and
[original CLI](stage57-original-evidence.tar.gz) archives preserve the raw evidence.
Their publication reports verify complete member identities and state exclusions.
No hosted CI result is claimed. The Java authority commits and bindings are
separate from candidate implementation provenance.

The implementation is committed as
`0560c919c9fea6f7ea7a08d76e5d78e74a3e9d83` and was pushed to remote `main`.
[Actual Git source binding](stage57-source-binding.json) verifies all 252 final
runtime build inputs and the complete native, capture-wrapper and original source
catalogs. All 165,210 Rust-check input origins are verified; four documentation
inputs retain their earlier committed versions. The one earlier M1 capture script
is bound to its historical member inside the committed platform archive, rather
than mislabeled as current source. The
[binding evidence](stage57-source-binding-evidence.tar.gz) retains the complete
per-record results and actual Git-object checks. The original precommit checkpoint
is preserved with its pending-binding statement.

### Stage56 Class descriptors and inheritance

The [Stage56 implementation checkpoint](stage56-implementation-checkpoint.json)
records canonical Class descriptors, separate instance prototypes, actual
superclasses, shared type/converter lookup and corrected typed equality/hash
member traversal in `25aa0487603d80da17ca77011137139ca9c290b0`.
See the [Class contract](../../docs/m2-class-values.md) and
[independent final audit](stage56-final-audit.md).

Native, Node N-API and Chromium Wasm each record 78 passes, four mismatches and
six oracle errors across the original 88 Class sessions. The additive metadata
selection passes all 20 sessions per target. Provider SPI and exported-Class API
parity remain required and unexecuted. Six unchanged original CLI programs have
five raw matches and one mismatch: `api/any.pkl` first fails on DataSize value
construction. These are one Java and one Rust execution per input.

All five required Rust checks pass locally with Rust 1.98.1; workspace tests
report 910 passes and one ignored probe. The M1 platform regression retains
6,076 Java matches and both known regex failures per target. Its architecture
contract passes, while strict compatibility fails. Full M2, product APIs,
release-platform coverage, speed and peak-memory targets remain open.

[Post-commit source binding](stage56-source-binding.json) compares 179,686
selected source records with actual Git objects in the implementation and
pinned submodule trees, with zero differences. This includes all 165,175 final
Rust-check inputs; overlapping catalogs are not distinct source or feature
counts. The [binding archive](stage56-source-binding-evidence.tar.gz) retains
per-record results, symlink modes and target bytes, and the audit script.

[Validation](stage56-validation-evidence.tar.gz),
[candidate](stage56-candidate-evidence.tar.gz), and
[original CLI](stage56-original-evidence.tar.gz) archives retain raw final and
failed acquisitions. Their publication reports record member verification and
explicit exclusions. Earlier native build manifests omitted five literal
stdlib inputs; the independent frozen-binary byte bridge remains separate from
the complete final build input capture. No hosted CI result is claimed.

### Stage55 diagnostic frames and transformers

The [Stage55 checkpoint](stage55-implementation-checkpoint.json) records complete
source-frame and custom transformer boundaries, including 246 passes and 42 raw
nonpasses across 288 executions. Its
[actual source binding](stage55-source-binding.json) retains the separate
implementation and verification provenance. Later Class results do not replace
those historical outcomes or close their remaining API and lifecycle scope.

### Stage54 method deprecation

The [Stage54 implementation checkpoint](stage54-implementation-checkpoint.json)
adds method-deprecation warning state to executable call sites, preserving
argument order, callback acknowledgement, reentry, cache roots and implicit calls.
See the [deprecation contract](../../docs/m2-deprecation.md).

The final native, Node N-API and Chromium Wasm comparison records 306 raw matches
and 30 oracle-error nonpasses across 336 executions. Five Java internal-error/API
sessions remain strict failures. Eight unchanged upstream snippets match all CLI
channels, with one Java and one Rust process per input. Required Rust checks pass;
workspace harnesses report 884 passes and one ignored probe. Python checks report
165 M2 plus 44 deprecation tests, and the final JS adapter has 13 passing tests.
The M1 experiment keeps both known Java regex differences.

[Postcommit source binding](stage54-source-binding.json) connects the 4,577 final
validation inputs and all 162,919 original Rust-check snapshots to implementation
commit `9f92573a1262729158103f2a32ddf254ad1cf631` and its pinned Git references.
These sets overlap. The [binding archive](stage54-source-binding-evidence.tar.gz)
retains the detailed per-file records and publication reviews; the original
precommit checkpoint remains unchanged.

[Candidate evidence](stage54-candidate-evidence.tar.gz),
[Java history](stage54-java-history-evidence.tar.gz), and
[validation and review evidence](stage54-validation-review-evidence-v2.tar.gz)
preserve final and failed acquisitions. The validation archive supplies the exact
missing transitive verifier dependency `benchmark.py` from the pinned Java
authority, separately from the original candidate source catalog. Its v2 changes
only the readability of the verification README; the original publication is
preserved locally and identified in the binding archive.

Cross-evaluator standard-library caches, custom frame transformation and callback
closure/API parity remain open, alongside full M2, platform and performance gates.
These are local development results; no hosted CI outcome is claimed.

### Stage53 object-body parameters

The [Stage53 implementation checkpoint](stage53-implementation-checkpoint.json)
adds object-body parameters, typed function amendments, retained curried captures,
and Mixin defaults through explicit and inferred aliases in the shared engine.
See the [language contract](../../docs/m2-object-parameters.md).

The [postcommit source binding](stage53-source-binding.json) compares all 4,589
final validation inputs with actual Git blob bytes at implementation commit
`0ab9da59201c6fbe59f507310f36b2e389fda4f2`, including 63 files reached through the
pinned Pkl gitlink. [Binding evidence](stage53-source-binding-evidence.tar.gz)
retains the Git batch requests, raw objects, and publication review. This closes
the checkpoint's historical pending-commit observation without rewriting it.

The final comparison matches all 85 ordinary Java cases twice on native, N-API,
and Chromium: 170 executions per target, 510 total. These comprise 56 value cases
and 29 language-error cases. Seven Java internal-error boundaries remain separate
unexecuted nonpasses. Three unchanged upstream module commands also match complete
stdout, stderr, exit codes, and generated files; the parser original has empty
output, while the two generator originals exercise actual rendered values.

All 17 recorded validation commands pass with 4,589 unchanged source inputs. Rust
workspace harnesses report 873 passes and one ignored frontend probe; the native
adapter has four additional tests. Python verification runs 207 tests and the
portable driver has 13 tests. Native lifecycle checks cover suspended parameter
checks, escaped curried closures, reentry, retry, cancellation, and code/heap
release. These counts are not distinct Pkl features or performance results.

The first candidate's six acquisition failures per target remain preserved: its
Host adapter incorrectly equated an authored relative URI with its resolved URI.
The corrected adapter records both and checks the exact declared resource bytes.
The engine and frozen Node/Wasm artifacts are unchanged between those candidate
runs. The separate M1 regression matches all 6,078 shared-engine observations;
Java matches 6,076 per target, retaining both known regex failures. Its architecture
contract and 92 Node/Chromium scenario labels per target pass.

[Java evidence](stage53-oracle-evidence.tar.gz),
[candidate evidence, including the first failure](stage53-candidate-evidence.tar.gz),
and [validation, original modules, and independent audits](stage53-validation-review-evidence.tar.gz)
retain raw bytes, source snapshots, artifacts, and provenance. Full M2, product/API
compatibility, release-platform coverage, and speed/peak-memory targets remain open.

### Stage52 original module admission

The [Stage52 tooling checkpoint](stage52-tooling-checkpoint.json) admits all
752 previously unconnected original-ready snippets. The generated module registry
contains all 888 original-ready cases and the preserved blocked `import1b` record.
Every original case ID, order, argument, input, resource and expected outcome is
preserved. The whole inventory now has 4,416 ready rows, 71 unimplemented rows and
40 unmapped rows. Readiness is execution connectivity, not compatibility.

An explicit original-module workspace retains the complete pinned Pkl and Pantry
trees while omitting unrelated project fixture copies. Schema-5 reports bind this
filesystem boundary to the authoritative corpus digest. Capture exceptions retain
their raw evidence and attempt workspace restoration. The default workspace is
unchanged. Local validation passed 184 Python tests and all nine recorded commands
with 3,955 inputs unchanged. [Raw evidence](stage52-tooling-evidence.tar.gz) includes
the audits, source snapshots, failed setup/inspection records and actual checks.
Rust evaluator code is unchanged. The all-original Java/Rust acquisition follows
this tooling checkpoint; no new language passes, whole M2, platform or performance
completion are claimed here.

### Stage51 strict collection transforms and callback lifecycles

The [implementation checkpoint](stage51-implementation-checkpoint.json) records a bounded local candidate04 result for the shared Rust engine. It adds eleven transforms: List and Set `map`, `mapIndexed`, `mapNonNull`, and `mapNonNullIndexed`, plus Map `map`, `mapKeys`, and `mapValues`. Supporting conversion/projection paths, `Null(default)` and function diagnostics, partial Duration values and reference identity, weak canonical empty strict collections, and explicit receiver selection were also corrected. Callback order, lazy Map value reads, hashing/insertion, Host suspension/reentry and GC use shared execution paths. Duration arithmetic/conversions, DataSize, general lazy collections and the remaining standard library are still incomplete.

Native, Node and Chromium each executed 152 sessions and 2,108 recorded calls: all 2,000 ordinary calls and 96 diagnostic-only calls matched, while all twelve post-close API calls remain raw failures. The 96 diagnostic-only calls did not reach their intended behavior and are not counted as collection or renderer coverage. All recorded read/log effects and policy outcomes matched; four denied reads had no reader side effects. The unchanged original SourceCode input also matched twice through the recorded language adapter. Candidate03's 1,984/2,000 ordinary result and its sixteen failures per target, candidate02's 1,156/1,168 ordinary result, the earlier native parser acquisition failure and corrected raw-only readback remain preserved. Repeated calls and successive corpora are not independent feature counts.

All five required Rust commands passed locally on macOS arm64 with Rust 1.98.1; the workspace test result was 864 passed, zero failed and one explicitly ignored frontend probe. Its recorded ignore reason says "invoked explicitly by the M1 differential evidence runner"; that reason does not establish that the ignored test executed in this checkpoint. The final M1 experiment matched all 6,078 comparisons across the three Rust targets and 6,076 against Java on each target. Its two existing regex mismatches remain failed; the architecture contract passes. Node and Chromium each completed 92 scenario assertion labels. These labels and historical Java UTF-8-only observations do not establish full public API or lossless Java UTF-16 compatibility.

The [capture index](stage51-implementation-captures.json) retains actual terminal receipts, raw outcomes, runtime/artifact hashes, frozen sources and historical failures. The candidate04 build captured 437 source files, including 225 unchanged freeze04 core paths; the required-check input set contains 439 files. The [staged source binding](stage51-candidate04-staged-source-binding.tar.gz) independently joins all 437 source files and 75,863 fixture files to staged Git blobs, including the pinned reference gitlinks. That archive preserves the original precommit evidence. The subsequent [commit binding](stage51-implementation-source-binding.json) joins all 76,302 paths, modes and object IDs to implementation commit `f5d1f7a`, which was pushed to `origin/main`. The original checkpoint retains its precommit observations; the later binding completes that provenance step. Recorded command/source/artifact provenance is not independent binary decompilation. Archive payload modes are normalized; the preserved selection tables retain original modes.

Acquisition integrity and the bounded ordinary development contract pass. Strict selected raw/API compatibility and full Java administrative lifecycle parity remain incomplete. This checkpoint does not replace a full selected-language or persistent evaluator regression at the final implementation commit, close any of M2's nine workstreams, establish hosted CI or full product/platform compatibility, or claim performance or memory targets. Stage48's broader selected regression is separate historical evidence and is not inherited by candidate04.

The [Stage50 implementation checkpoint](stage50-implementation-checkpoint.json)
reclaims unreachable dynamic code segments, their directory entries and nested
capacities while preserving committed logical addresses. Active frames, escaped
declarations and closures retain their executable owners. Compilation rollback
restores replaced module state, and URI preparation avoids copying unchanged
owners held by program snapshots. Storage tests compare 20 and 200 ordinary and
reentrant uncached evaluations. These are retained-storage checks, not peak RSS
measurements or proof of the Java performance targets.

All five required Rust checks pass on candidate02, with 86,455 recorded inputs
unchanged. The focused storage suite passes 190 tests. Native, Node and Chromium
each match 212 API calls under both supplied-primary and public-URI profiles.
The extended comparison now runs on all three targets: 936 of 944 raw outcomes
match, comprising 836 intended ordinary operations and 100 diagnostic-only
observations that did not reach the intended feature. Eight Java internal-error
boundaries remain unmatched. Full Java event/counter compatibility remains
unproved and its strict gates fail. Repeated profiles and targets are not
independent feature counts.

Actual Node and Chromium each retain 860 renderer, 1,008 Resource and 252
module-preparation matches. Loader and inheritance API/callback payloads match,
but 12 loader and six inheritance executions per target retain strict
request-counter mismatches. ModuleInfo retains 12 internal boundaries and 56
unexecuted shared-evaluator follow-ups per target; guest-catch separately records
24 full ordinary, ten constructor-only and eight partial terminal executions.
The primitive experiment matches 6,078 cases across the shared targets and
6,076 against Java, preserving both regex failures.

The [native/tooling captures](stage50-native-tooling-captures.json) and
[platform captures](stage50-platform-captures.json) preserve original failed
attempts, source snapshots, actual runtime artifacts, raw outcomes and independent
reviews. Earlier Stage49-artifact provider controls retain their original
identity and are separate from candidate02 results. The complete selected
language and persistent-evaluator regression for Stage50 remains to be acquired.
Stage48/49 acquisitions and Stage47's published result do not verify Stage50.
**M2, complete products, release platform coverage and performance targets remain
incomplete.**

The [Stage49 implementation checkpoint](stage49-implementation-checkpoint.json)
adds selected/resolved key identities, independent cache-policy requests, eager
import-clause factory selection, public ModuleSource operations, and traced
query/module executable ownership. Actual key kind distinguishes custom keys
from builtin singletons even when their URI spelling is identical. The shared
engine and native/Wasm/N-API bindings use the same continuation model.

All five required Rust checks pass on build05, with 53,487 captured inputs
unchanged. Native, Node and Chromium each match the original 212 API calls under
both supplied-primary and public-URI profiles. The full Java event/counter gate
still fails in all 36 sessions per profile and target. These repeated observations
are not independent features or a language completion rate.

The extended native comparison matches 936 of 944 Java observations. It fixes
all 32 ordinary build04 mismatches in module URI resolution, Ref module method
lookup and missing-property hints. Eight Java internal-error boundaries remain
unmatched. Of the 936 matches, 100 preserve diagnostics from inputs that did not
reach their intended alias behavior; 836 cover the intended ordinary operations.
These extended controls have not run on Node or Chromium.

Actual portable comparisons also retain 860 renderer, 1,008 Resource and 252
module-preparation matches per target. The corrected virtual File provider
selects hierarchical file keys before checking source existence. Loader and
inheritance result/callback payloads match Java, while 12 loader and six
inheritance executions per target still fail exact request-counter comparisons.
ModuleInfo, security and guest-catch retain their existing partial, internal and
unexecuted boundaries. The primitive experiment matches all 6,078 cases across
the shared engine targets and 6,076 against Java, preserving both regex failures.

[Core captures](stage49-core-captures.json), [oracle/native captures](stage49-oracle-captures.json)
and [platform/tooling captures](stage49-platform-captures.json) preserve earlier
failed candidates, exact inputs and raw channels, artifact/source bindings and
independent reviews. Unreachable executable payloads are reclaimed, but non-tail
slots and nested capacities can still accumulate. Bounded total memory, complete
ordered lifecycle behavior, import globs and the whole-language gate remain open.
No full selected regression has yet been acquired for this final candidate.
Stage48's separate acquisition and Stage47's published result are not build05
evidence. **M2, complete products and performance targets remain incomplete.**

The [Stage48 C01 implementation checkpoint](stage48-implementation-checkpoint.json)
adds shared typed import admission, successful import-site caches, explicit
default-policy delegation and authored imports for qualified Ref type guards.
All five required local Rust checks pass, with the recorded source qualifications.
The frozen candidate binds 53,236 files and 11 literal symbolic links; four early
intermediate Rust source versions remain hash-only.

Native policy comparisons match Java in 440 selected raw API calls. Another
212 custom-SPI calls remain unexecuted, and the complete event/counter gate fails
for all 52 supplied-primary sessions. Actual Node and Chromium each match the
84 policy API calls while retaining full lifecycle failures in all 12 sessions.
These overlapping observations are not unique features or completion rates.

Actual Node and Chromium also match 860 renderer, 1,008 Resource and 252 module
preparation calls per target. ModuleInfo, security and guest-catch comparisons
retain their individual boundary exclusions. Loader and inheritance API channels
match, but their strict counter gates fail because new CheckImport requests are
outside the original resolve/source-only fixture counter scope. Original requests
and failed outcomes remain intact. The primitive comparison retains both known
Java regex mismatches.

The typed bootstrap provider uses Resource text completions and the shared read
cache. On the unchanged 600-call Java input, each portable target has 402 passes,
144 unsupported observations and 54 mismatches. The earlier legacy-provider
result, 274/144/182, remains separate. The runtime artifacts are unchanged between
these two provider experiments.

[Core captures](stage48-core-captures.json) and
[platform/tooling captures](stage48-platform-captures.json) retain raw failures,
source and artifact bindings, and independent reviews. General ModuleKey
factories, key-kind and isCached rules, eager import-clause factory selection,
primary/query/repl lifecycle, URI equality and import globs remain required work.
The separate Stage48 full selected acquisition is now closed and recorded below.
It is bound to the Stage48 implementation commit and does not validate later
Stage49, Stage50 or Stage51 changes.
**M2, full product compatibility and the performance targets remain incomplete.**

The [Stage48 final selected regression](stage48-final-summary.json) is bound to
`5e68c4b`: 3,652 of 3,664 cases match, with two unsupported results, nine
mismatches and one oracle error. All 4,527 inventory IDs and their order remain
unchanged, including 823 unimplemented and 40 unmapped accounting rows. There
are no case-status transitions from Stage47. The development, strict selected
and full gates fail; the full gate retains 1,825 failure reasons. Acquisition
and evidence-integrity success do not override these compatibility failures.

All 88 metamorphic relations match. The persistent-evaluator comparison covers
334 sequences and 2,782 calls per implementation, with exact status, stdout and
stderr agreement. It separately retains 928 error-class metadata differences;
those fields are outside that comparison contract. Unexecuted groups remain
visible. These repeated and overlapping observations are not distinct feature
counts or a whole-language completion percentage.

The [independent audit](stage48-final-regression-audit.json) rereads 62,164
declared payloads and binds 4,413 captured source records, representing 4,191
unique paths, to the fixed implementation and pinned references. The
[captures](stage48-final-regression-captures.json) and
[publication closeout](stage48-final-publication-closeout.json) preserve raw
differences, actual terminal receipts, source identities and packaging checks
in four archives totalling 50,857,094 compressed bytes. Original acquisition
files remain unchanged. This is local evidence, not hosted CI or a performance
result. **All nine M2 workstreams and the full completion gate remain open.**

The [Stage47 C06 implementation checkpoint](stage47-implementation-checkpoint.json)
adds shared JSON rendering, RenderDirective and property converters, source-backed
JSON annotations, collection join, and reference identity used by selected
diagnostics. It also preserves descriptor and activation-slot history through
generator copies and compacts optional diagnostic metadata. All five required
local Rust checks pass. The unchanged upstream `moduleOutput2.pkl` and
`pcfRenderer9.pkl` each match Java twice across complete CLI channels.

Selected renderer API comparisons match Java in 1,840 calls per native, Node and
Chromium target. Actual Node and Chromium also pass 1,008 Resource and 252
module-preparation calls each. The shared primitive experiment retains 6,078
cross-target matches, 6,076 Java matches per target and the two known regex
failures. These overlapping calls are not unique features or a completion rate.
The permanent frame-identity fixture preserves 240 Java observations and is
consumed under two Rust instrumentation settings, yielding 480 comparisons.

Core, native/oracle and platform archives retain raw channels, source/artifact
bindings, reviews and earlier failures. All 51,745 C06 source inputs are bound;
seven early intermediate source versions remain explicitly hash-only. The
subsequent documentation and publication files are separate from the frozen
runtime candidate. The separate Stage47 final regression is recorded below;
remaining identity semantics and the strict M2 gate remain open. Later
standard-module admission changes have their own implementation evidence.

The [Stage47 C05 platform record](stage47-c05-platform-summary.json) preserves
an intermediate shared renderer/identity candidate: 860 renderer, 1,008 Resource
and 252 module-preparation API calls per actual Node/Chromium target. Baseline
strict comparison retains the two known regex failures. The six archives include
raw results, source/build binding, independent review and the original failed
workspace setup followed by pinned-reference symlink restoration. Later common
diagnostic layout changes and the final Stage47 checks require separate evidence.

M2 is **in progress**. Stage 1 added expressions, strings, recursive methods,
closures, primitive type checks, and source diagnostics. Stage 2 adds selected
class, inheritance, generic alias, default-value, and modifier semantics to the
shared engine. Stage 3 adds pipelines, resumable constraints, and optional power
assertions. Stage 4 adds ordered collections and resumable generators. Stage 5
adds generic collection checks and lazy wrappers. Stage 6 adds member predicates
and their scope, cache, and lifecycle behavior. Stage 7 adds structural equality,
nonprimitive keys, and related diagnostics. Stage 8 corrects typed defaults,
cache-sensitive comparison order, and deep-force/hash preparation order.
Stage 9 adds stable hashes,
partial map publication and retries, HashSet length counting, and let diagnostic
scopes. Stage 10 adds deferred References, declaration/type metadata, and
standard-library source frames. Stage 11 adds fixed declarations, ordered
override checks, and inherited modifier metadata. Stage 12 adds evaluator-owned
class publication, retained partial classes, and persistent sequence comparisons.
Stage 13 adds type-alias identity and publication, function metadata and arity checks,
and exported arrow metadata equality. Stage 14 adds ordered type resolution, generic
parameter scopes, and owner signature initialization. Stage 15 adds selected recursive
constraint graphs and erased `Class<T>` arguments. Stage 16 adds shared constraint
receiver slots, creation-time lambda captures, selected alias const boundaries
and recursive diagnostic compression. Stage 17 adds persistent evaluator-local
executable/type-use ownership, selected local const-property retries, and
const-method lexical scope corrections. Stage 18 adds finite recursive generic
argument environments, shared generic let roots, selected collection defaults
and Reference validation, and let/for const-scope boundaries. Stage 19 shares
runtime generic argument graphs, reclaims them iteratively, schedules nested
defaults, and makes deep Reference metadata traversal stack safe. Stage 20
preserves nullable defaults through amendments and evaluator lifecycles. Stage 21
adds selected direct generic constructors, argument-source diagnostics, and their
shared-engine lifecycle checks. Stage 22 adds selected strict Map construction,
lookup, typing, equality, hashing, iteration and stringification. Stage 23 adds
selected strict Set semantics, multiline strings and lifecycle checks. Stage 24 adds
bounded abstract Collection type checks, defaults and metadata. Stage 25 adds
continuations for selected authored frontend paths and small-stack checks.
Stage 26 extends those paths to bounded nested lexical strings and interpolation.
Stage 27 aligns selected lexical string diagnostics with Java.
Stage 28 aligns selected Unicode escape diagnostics and validation order.
Stage 29 preserves lossless UTF-16 string values through the shared engine.
Stage 30 preserves diagnostic frame identity and selected declaration ownership.
Stage 31 aligns executable value-frame locations and constant-member source fallbacks.
Stage 32 preserves declaration owners for rejected non-local object property annotations.
Stage 33 aligns non-local const owners and method/lambda inferred-parent boundaries.
Stage 34 uses return-type defaults and inherited property contexts for inferred parents.
Stage 35 adds annotations and declaration initialization. Stage 36 connects common
object forcing and string conversion. Stage 37 adds trace/logger events and keeps
initialization and first-path evaluation within one admitted Host context.
Stage 38 adds resource-read syntax, the shared scalar default policy,
and generic method-lookup diagnostics.
Stage 39 connects original module frontend commands and typed failure origins.
Stage 40 connects actual module output, PCF continuations and lossless string
output across the shared engine and bindings.
Stage 41 adds shared module imports, typed resolve/source Host requests,
expression sessions and imported diagnostic ownership.
Stage 42 connects module-owned class/alias types, inheritance and amendments,
original module validation order, and code-point string indexing.
Stage 43 preserves canonical diagnostic strings through the shared engine and
native/Node/Wasm boundaries, and corrects literal-union type-check equivalence.
These checkpoints do not complete the language milestone.

## Renderer reference-fixture checkpoint

The [renderer Java fixtures](../../docs/m2-renderer-oracle.md) preserve 860 selected
API observations in 44 persistent evaluator sessions, including complete output
and error UTF-16/Java UTF-8 channels and ordered reader inputs. Their 14,995
payload files and manifest retain the original bytes. Acquired but unselected
calls, authored probes that did not reach the intended feature, policy profiles
and subsequent identity/lifecycle observations have explicit separate dispositions.

The [verification record](renderer-portable-fixtures.json) records 27 Python
regressions, 12 shared JavaScript session unit tests, the fixture verifier,
driver syntax, inventory freshness and repository checks. Independent review
led to rejecting undeclared directories before traversal and links/non-regular
payloads before reads. Staging tools, the first installed run and corrective
verification remain separate evidence. This fixture/tool checkpoint does not
execute the renderer through Rust, N-API or Wasm and does not complete Stage 47
or M2.

## Stage 47 final selected regression

The [native result](stage47-final-summary.json) is bound to `0e13a58`: 3,652 of
3,664 selected cases match Java, with two unsupported cases, nine mismatches
and one oracle error retained. The unchanged original `moduleOutput2.pkl` and
`pcfRenderer9.pkl` improve from unsupported to passing. No previously passing
case regresses. All 4,527 inventory IDs, 475 requirements, selected order and
the existing JVM/classpath mapping remain unchanged.

All 88 metamorphic relations and 2,782 ordered calls across 334 persistent
sequence executions match their declared channels. The 928 producer error-class
differences and unexecuted lifecycle boundaries remain separate. Acquisition,
evidence integrity and the persistent contract pass; development, strict selected
and full-language gates still fail. The full gate retains 1,825 failure reasons.

The [independent audit](stage47-final-regression-audit.json) verifies 62,152
declared payloads and binds 4,405 captured source records across 4,187 unique
paths to the implementation and pinned reference Git blobs. The
[raw captures](stage47-final-regression-captures.json) retain the observed
wrapper exit zero, failed compatibility gate exits, and the explicitly late
terminal receipt alongside the original audit notice. Expression acquisition
used four workers instead of Stage46's two; module commands stayed sequential.
This scheduling change is not a performance comparison. This result does not
validate later Stage48/49 changes or complete M2, platform or performance goals.

## Stage 46 final selected regression

The [native result](stage46-final-summary.json) is bound to `a3eb363`: 3,650 of
3,664 selected cases match Java, with four unsupported cases, nine mismatches
and one oracle error retained. The unchanged original `basic/read.pkl`,
`modules/amendModule6.pkl` and `modules/invalidAmend6.pkl` improve from unsupported
to passing. No previously passing case regresses. All 4,527 inventory IDs,
475 requirements, selected order and the existing JVM/classpath mapping remain.

All 2,782 ordered calls across 334 persistent sequence executions match status,
stdout and stderr. The 928 producer error-class differences and unexecuted
lifecycle boundaries remain separate. Acquisition, evidence integrity and the
persistent contract pass; development, strict selected and full-language gates
still fail. The full gate retains 1,827 failure reasons.

The [independent audit](stage46-final-regression-audit.json) verifies 62,122
declared payloads and binds 4,385 captured source records across 4,177 unique
paths to the implementation and pinned reference Git blobs. Lifecycle schema 2
directly captures all four Cargo control inputs, including `.cargo/config.toml`.
The current 14 nonpassing cases are distinct from the union of 17 previously or
currently nonpassing IDs. The [raw captures](stage46-final-regression-captures.json)
preserve the actual wrapper exit zero and failed compatibility gate exits.
This result adds no Stage 47, platform or performance coverage and does not
complete M2.

## Stage 46 implementation checkpoint

The shared Resource value, evaluator-local successful/Missing cache and typed
module URI preparation are committed in `a3eb363`. The [local implementation
record](stage46-implementation-checkpoint.json) separates focused comparisons
from the full selected regression on that commit, reported above. This
checkpoint does not complete M2.

The unchanged original `basic/read.pkl` matches Java twice across output, errors,
exit status and generated files. Native focused comparisons match 1,724 selected
API calls: 1,008 Resource calls and 716 File/logical/capability calls. Sixteen
constructor-boundary calls and 264 Resource calls remain unexecuted or unselected.
The older File-policy checker also retains 24 capability-policy projection
mismatches; raw API matches do not reclassify those projection outcomes.

Actual Node and Chromium runs each match 1,008 Resource calls and 252 selected
module-preparation calls, including raw UTF-16/Java UTF-8 results, reader counts
and declared observable events. The current drivers explicitly launch the browser
executable whose hash is recorded. Older captures without that explicit selector
retain their browser-identity limitation. Failed supplemental HTTP acquisitions
are preserved; corrected runs use fresh directories and verify a missing asset
returns 404 without terminating the server.

All five required Rust checks pass on the frozen C06 runtime. The initial report's
incorrect C05 scope label is corrected by a separate erratum. Subsequent changes
affect six HTTP verification drivers, with no runtime or build-configuration
changes. The common platform baseline still has 6,078 equal Rust-target results,
6,076 Java matches and the two existing regex failures. Forced glob evaluation,
remaining Resource APIs and the full language, product, platform and performance
requirements remain open.

The [source binding](stage46-implementation-source-binding.json) connects the
32,847 captured regular inputs to the actual implementation commit, retaining
checkout transformations, symlinks and snapshot permission differences as
separate boundaries. The [published captures](stage46-implementation-captures.json)
contain 40 archives, including original failures and packaging receipts. Exact
Git blob mappings avoid duplicating identical historical source snapshots;
compiled artifacts and differing source bytes remain raw payloads. The
[core review](stage46-core-publication-review.json) and
[oracle/native closeout](stage46-oracle-native-publication-closeout.json) retain
their actual process exits and historical provenance limitations.

## Resource reference-fixture checkpoint

The [portable Java fixtures](../../docs/m2-resource-portable-fixtures.md) retain
1,008 selected API observations in 20 sessions, with 264 unselected boundaries
recorded separately. Their [verification record](resource-portable-fixtures.json)
covers independent oracle pins, original output/code-unit/counter bindings,
11,507 payloads plus the manifest, and 27 passing Python regressions including
real fixture forgeries. The promoted fixture CLI and inventory freshness check
also pass. This checkpoint adds no native, Node or Chromium evaluation result;
Stage 46 runtime work and the full M2 gate remain open.

## Stage 45 final selected regression

The [combined native result](stage45-final-summary.json) is bound to `01971d0`:
3,647 of 3,664 selected cases match Java, with seven unsupported cases, nine
mismatches and one oracle error retained. Six original modules and the explicit
JVM/classpath case improve from the Stage 44 result; no previously passing case
regresses. All 4,527 case IDs and their order remain unchanged. Only the recorded
`import1b` adapter/profile mapping changes, with its two successful raw repetitions
counted as one original case.

All 88 metamorphic relations and 2,782 ordered calls across 334 persistent
sequence executions match. Status, stdout and stderr are compared; producer
error-class metadata remains recorded separately. Evidence integrity and the
persistent contract pass. Development, strict selected and full-language gates
still fail; the full gate retains 1,830 failure reasons.

The [audit](stage45-final-regression-audit.json) checks 62,088 declared payloads
and binds 4,166 unique captured source paths to the implementation and pinned
references. Its historical `nonpass_rows` field is the union of 24 previously or
currently failing IDs; the current result has 17 nonpassing cases. The summary
makes that distinction explicit. The [archives](stage45-final-regression-captures.json)
preserve raw results, source maps, the original wrapper and its actual exit-zero
receipt. Wrapper success does not change the failed compatibility gates.

The schema 1 lifecycle capture did not directly include `.cargo/config.toml`.
Its bytes are bound separately by the preceding language capture, the commit
blob and a post-run check. The later [schema 2 tooling change](lifecycle-source-contract.json)
requires that file in future native captures; its 18 Python tests and inventory
check passed. Historical captures retain their original verifier and scope.
This native regression adds no platform or performance coverage and does not
close the broader standard-library bootstrap failures or M2.

## Stage 45 implementation checkpoint

The [implementation checkpoint](stage45-implementation-checkpoint.json) records
supported source-backed standard modules, guest catch continuations, ModuleInfo
validation, and typed security-denial completions. See the [execution contract](../../docs/m2-stdlib-bootstrap.md)
for the shared VM design and the boundaries that remain incomplete.

All five required Rust checks passed with stable source snapshots. Three fixture
verifiers, 27 Python tooling tests, and 22 JavaScript transport tests passed.
Manifest freshness was restored by updating 102 implementation-source hashes;
case definitions, execution states, counts, and ordering did not change.
The final native candidate matches both repetitions of six original modules.
Actual Node and Chromium match the declared guest-catch, ordinary ModuleInfo,
and typed security-denial comparisons. Constructor-only and terminal partial
comparisons, Java internal errors, and independent-root follow-ups retain their
separate outcomes and limitations.

Broader bootstrap comparisons retain 194 passes, 192 unsupported calls, and
214 mismatches per target. Missing reflection/JSON bindings, resource-read
counts, and JVM-global standard-module caches remain required work. The common
platform comparison has 6,078 shared-engine matches and 6,076 Java matches per
target; both known regex failures remain failures.

The M1 platform collector directly captured 76 of 77 production inputs. Its
omitted `stdlib/modules.txt` is bound separately by the complete source freeze
before platform acquisition and the identical native snapshot afterward. It is
not relabeled as an M1-captured input.

Implementation `97b4da0` is committed. The [published evidence](stage45-implementation-captures.json)
retains 26 verified archives, including initial failures and the original
historical inventory defect with its prior correction addendum. The
[committed-source binding](stage45-implementation-source-binding.json) and
[source map](stage45-implementation-source-map.json) connect 14,216 captured
project inputs and nine publication files to actual Git blobs. Repeated snapshots
are provenance, not distinct semantic cases. The combined `01971d0` native
regression is recorded separately above. Hosted CI, full compatibility, and
performance remain unverified.

## JVM profile in the M2 inventory

Integration `01971d0` connects the unchanged original `import1b` case to an
explicit JVM/classpath execution profile. Its historical blocked CLI contract
remains unchanged. The [combined focused result](classpath-m2-integration-checkpoint.json)
requires both raw repetitions to pass and counts the original case once. Case
IDs, ordering, and the 3,664-case default selection remain unchanged. The
remaining 3,663 selected cases are outside that focused result.

[Ten published archives](classpath-m2-integration-captures.json) retain the
original proposal, combined acquisition, independent review, failed validation
attempts, and metadata recovery. The [source binding](classpath-m2-integration-source-binding.json)
ties 3,823 combined inputs to actual commit/reference blobs and explicitly
records the older execution's source differences. The strict full gate still
fails; source identity and focused success do not establish M2 completion.

## Native JVM-classpath checkpoint

The [committed source binding](classpath-committed-source-binding.json) verifies
all eleven implementation, profile, tooling and contract paths against commit
`a3c45ba` and the unchanged reference gitlinks.

The [checkpoint](classpath-implementation-summary.json) evaluates the unchanged
original `basic/import1b.pkl` through the pinned JVM API and a Rust directory
classpath Host using the shared engine. Both fresh-process repetitions match
raw stdout, stderr, exit status and files. The historical CLI contract and its
blocked inventory row remain unchanged; this additive profile does not close
that inventory obligation automatically.

Six additional controls retain three matches, two diagnostic mismatches and one
unsupported trust-policy case. The missing-resource and imported-error controls
differ in the standard-library frame URI: the upstream snippet uses an empty
frame transformer. Full classpath/security behavior and portable adapters remain
unfinished. The five required Rust checks, 692 workspace harness invocations,
six example tests and 26 distinct Python tests pass; one workspace invocation
remains ignored. These counts do not measure feature coverage.

[Raw captures](classpath-captures.json), [source binding](classpath-source-binding.json)
and the [publication audit](classpath-publication-audit.json) preserve the initial
acquisition, three corrected verifier defects and all failed controls. The final
capture includes the root Cargo configuration and stricter path/symlink checks.
Historical verifier sources and later documentation changes are identified
separately. See the [profile contract](../../docs/m2-jvm-classpath.md).

## Stage 43/44 final selected regression

The [final result](stage44-final-summary.json) covers the combined diagnostic and
Host implementation at `c9b140a`, acquired from checkout `a19619c`. Of 3,664
selected cases, 3,640 match, 13 remain unsupported, nine mismatch, one fails its
oracle execution contract and one is an oracle error. All 88 metamorphic
relations and 2,782 persistent calls across 334 sequence executions match.
The 4,527 original case definitions and all selected statuses are unchanged
from the corrected Stage 42 run. Development, strict selected and full-language
gates still fail; the full gate retains 1,837 failure reasons.

The original wrapper has no captured terminal status, so its exit code remains
unknown. Its preserved process report does record the language acquisition child
returning zero. A separate continuation verified those existing records without
reacquisition, then acquired and verified the persistent sequences. The
[provenance correction](stage44-final-provenance-correction.json) distinguishes
the child result from the unknown wrapper result; the initial audit, recovery,
index and archives retain their original interpretation. Evidence integrity and
the persistent contract pass.

The [raw/source audit](stage44-final-regression-audit.json) verified 58,140 declared
payloads and bound 4,078 unique captured paths to the implementation/checkout and
pinned references. [Exact source mappings](stage44-final-source-map.json),
[raw differences](stage44-final-raw-changes.json) and
[captures](stage44-final-regression-captures.json) retain the original wrapper,
completed continuation and all failures. Error-class metadata is retained but
excluded from cross-runtime lifecycle equality. This closeout adds no platform
or performance coverage and does not complete M2.

The [independent review](stage44-final-independent-review.json) checked both
archives, raw outcomes and committed source bindings. The
[focused follow-up](stage44-final-review-followup.json) confirms that its single
child/wrapper finding is addressed in the final summary.

## Stage 44 implementation checkpoint

The [committed source binding](stage44-implementation-source-binding.json) ties
all 9,925 checked paths to implementation `c9b140a` and its fixed reference
commits, with no missing or mismatched inputs.

The [publication audit](stage44-publication-review.json) independently verifies
all 43,938 captured payloads, 9,925 staged/pinned source paths and the actual
portable Host calls. Its raw review and publisher records are preserved in the
[closeout captures](stage44-closeout-captures.tar.gz).

The [checkpoint](stage44-implementation-checkpoint.json) and
[raw captures](stage44-implementation-captures.json) retain canonical Host
IOException causes, the direct `output.text` API operation, and remaining
converter/output-format diagnostic fixes. Eleven original Java scenarios have
80 ordered calls across two fresh JVM repetitions; native, Node and Chromium
match complete UTF-16/UTF-8 results and the original reader/module counters.

The [required checks](stage44-required-checks.json) pass all five commands on the
real Git worktree, including 692 passing workspace harness invocations, no
failures and one ignored invocation. The
[platform summary](stage44-platform-summary.json) retains affected regressions,
two known primitive regex differences, and 32 trace oracle errors per target.
The [independent code review](stage44-code-review.json) found no blocking issue
within this scope. Initial failures remain in the raw captures. These results do
not close the full selected corpus or the remaining M2 language requirements.

## Stage 43 implementation checkpoint

The [committed source binding](stage43-implementation-source-binding.json) verifies
7,093 source, fixture, and configuration paths against implementation commit
`f201028`, including all recorded required-check and platform-build inputs.

The [publication review](stage43-publication-review.json) verifies all seven
implementation archives and the exact staged implementation sources. Its raw audit,
publisher records, and staged-source proof are retained in the
[closeout captures](stage43-closeout-captures.tar.gz).

The [checkpoint](stage43-implementation-checkpoint.json) and
[captures](stage43-implementation-captures.json) preserve the canonical diagnostic
migration, actual Java UTF-16 and UTF-8 records, initial failures, independent
code review, and corrected native/Node/Chromium results. The
[five required Rust checks](stage43-required-checks.json) pass on the corrected
source: 7,026 source/fixture/configuration inputs remain unchanged, with 685
passing workspace harness invocations, no failures, and one ignored invocation.
These counts are not distinct feature coverage.

The [corrected platform candidate](stage43-platform-summary.json) matches its
original Java literal-reuse API calls, producer diagnostics and logs, constructor
messages, module/inheritance/owner graphs, string cases, and PCF output. The two
previous string diagnostic cases now match both actual UTF-16 and Java UTF-8.
Known primitive regex differences remain two of 6,078 comparisons, and trace
retains 58 matched calls plus 32 Java oracle errors per portable target.

The [code review](stage43-code-review.json) found a missing literal-union
classification in lazy collection type-check reuse. Direct Java internal
predicate observations establish the corrected equivalence rules; reverting only
the old arm makes the new test fail. Separate actual evaluator calls retain their
original underlying-cast failures, so no prior API byte mismatch is inferred from
the internal predicate difference.

Historical byte-only JS fixtures retain their original expectations and actual
JS units as separate observations. They do not establish expected unit equality.
See the [diagnostic contract](../../docs/m2-diagnostics.md). Full selected-corpus
closeout, remaining diagnostic producers and Host inputs, and the complete M2,
product, platform, speed, and memory requirements remain open.

## Stage 42 final selected regression

The [publication audit](stage42-final-publication-review.json) verifies both
archives, all 58,233 selected payloads, summary links and the preserved failure
scope. Its review and publisher records are in the
[closeout captures](stage42-final-closeout-captures.tar.gz).

The [final result](stage42-final-summary.json) closes the corrected full selected
run at implementation `7544699`, separately from the newer Stage 43/44 changes.
Of 3,664 selected cases, 3,640 match Java; 13 remain unsupported, nine mismatch,
one fails the original oracle contract, and one is an oracle error. All 88
metamorphic relations and 2,782 ordered evaluator calls across 334 sequences
match. Acquisition, evidence integrity and the persistent-session contract pass;
development, strict selected and full-language gates still fail. The full gate
retains 1,837 failure reasons.

The [independent regression review](stage42-final-regression-review.json) verifies
unchanged case definitions, inputs and order. The Reference frame-owner error is
the only status improvement from the preceding candidate. All persistent process
and per-call payloads remain unchanged. The language acquisitions retain 97
changed Java and 109 changed Rust records: exact path changes, the corrected
Reference owner, and a different frame count in the existing Java cycle bug.
Those differences remain raw; none is normalized into a pass.

The [source binding](stage42-final-source-binding.json) and
[exact source map](stage42-final-source-map.json) bind 4,070 captured paths to both
the implementation and recorded checkout commits. Original module inputs also
match the pinned upstream blobs. [Raw captures](stage42-final-regression-captures.json)
preserve the corrected runs, terminal commands, independent review and original
failures. The preceding candidate remains in the implementation captures below.
M2's whole-language, platform and performance obligations remain open.

## Stage 42 implementation checkpoint

The [implementation checkpoint](stage42-implementation-checkpoint.json) and
[implementation captures](stage42-implementation-captures.json) preserve
the complete pre-correction language and persistent-evaluator runs, the
Reference frame correction, corrected native/Node/Chromium observations, five
required Rust checks, and exact source promotion evidence. All 5,810 source and
fixture files used by the corrected required checks match the promoted checkout.
The [committed source binding](stage42-implementation-source-binding.json)
checks those files against implementation commit `7544699`.
The [independent publication review](stage42-implementation-publication-review.json)
verifies all 92,880 payload files across four archives.
The isolated check workspace omits the CI workflow file; these local commands
do not consume it, and no hosted CI run is claimed.

The pre-correction language run matches 3,639 of 3,664 selected cases, retaining
13 unsupported cases, ten mismatches, one oracle contract failure and one
oracle error. All 88 metamorphic relations and 2,782 persistent-evaluator calls
match. Compared with Stage 41, four original inheritance cases improve to pass;
the sole prior-pass regression is the Reference constraint frame owner.
All 3,629 prior cases retain their definitions, inputs and relative order; the
35 additions follow original inventory positions and yield 30 passes and five
unsupported cases. Raw cross-run path differences remain recorded.

The corrected implementation preserves the declaring frame's module owner.
Its focused native and actual Node/Chromium owner controls pass, and all five
required Rust commands pass. Existing module, inheritance, string and PCF
controls remain successful. Two string diagnostic cases still lose unmatched
surrogate units; existing regex mismatches and Java trace errors remain visible.
Stage 43 subsequently added canonical UTF-16 diagnostics, with its own evidence
above. These historical implementation captures precede the corrected full
selection described above and do not establish M2 completion.

## Stage 42 development captures

The [development captures](stage42-checkpoint-captures.json) preserve 72,288
files across five checked archives, including candidate source snapshots,
Java originals, native and actual Node/Chromium executions, failed intermediate
runs and independent reviews. Publication records exact Cargo cache exclusions
before traversal. Forty nested provenance files omitted from three historical
inner inventories are present at their original paths in these outer archives;
the original inventories and the audits documenting their scope are retained.
The [independent publication audit](stage42-checkpoint-publication-review.json)
checks the complete archive union against all selected originals and source maps.

The recorded candidate matches 35 of 41 original module programs, with five
unsupported and one mismatched program; two repetitions retain all 82 outcomes.
The original 22 module graphs, three added inheritance graphs and 14 string
indexing expressions match Java on native, Node and Chromium. Two additional
string error cases still lose unmatched surrogate units in diagnostic payloads,
even when their Java UTF-8 bytes agree. All five required Rust commands pass.
These component results leave the full language gate open.

At the time of these historical captures, whole-selection and persistent-evaluator
regression acquisition was in progress. It exposed a previous passing Reference constraint diagnostic
whose qualified frame uses the caller's module name instead of `pkl.ref`.
The captured candidate and raw failures remain unchanged. The owner fix was
verified in a separate source copy. These archives precede that correction and
do not establish Stage42 closeout or M2 completion.

## Stage 41 local module-loader checkpoint

The [implementation checkpoint](stage41-implementation-checkpoint.json) records
commit `23fe3e8`: imports use one VM, heap and code space with original/resolved
URI caches and source callback reentry. Expression sessions use the common
frontend and actual module receiver. Imported errors preserve their declaration
sources and logical frames. All five required local Rust checks pass, with 633
passing harness invocations and one ignored invocation; these are not distinct
feature counts.

The [final summary](stage41-final-summary.json) records 3,606 matching native
cases out of 3,629 attempts. Original `import1`, `import2` and `import3` improve
from unsupported to passing. All other statuses, all 4,527 inventory definitions,
and the selected order remain unchanged. Twelve unsupported cases, nine
mismatches, one unmet original Java execution contract and one Java oracle error
remain. All 88 metamorphic relations pass. Cross-stage raw differences in 3,501
channels are individually preserved, including physical URIs, the Java cycle
error and changed unsupported diagnostics; no output normalization is applied.

All 2,782 calls across 334 persistent sequences match Java and the previous
stage's raw status/output channels. The separate 30-original-module comparison
has 32 matching, 24 unsupported, two mismatched and two unmet-oracle-contract
calls across two repetitions; every result matches the preceding corrected
candidate. The [final regression archive](stage41-final-regression-captures.json)
preserves these runs, command logs, artifacts and independent audits. Exact
source bindings cover the final required/component/portable maps and the final
native/persistent maps against `23fe3e8` and pinned reference Git blobs.

The [platform captures](stage41-platform-captures.json) retain actual Node and
Chromium execution of 22 Java-origin graphs. Per target, 36 graph repetitions
match and eight are unsupported. Underlying calls include 106 matches, eight
unsupported results and two follow-up mismatches within an unsupported graph.
The common platform corpus still has 6,076 Java matches out of 6,078 results,
with the two regex gaps retained. Each portable target matches 384 PCF output
calls; 90 trace calls contain 58 matches and 32 Java oracle errors.

The [Java captures](stage41-oracle-captures.json) preserve original loader,
cache, reentry, expression and diagnostic observations. Historical failures,
later corrections and previously omitted adjacent logs remain explicitly
identified in the archive indices. Local report integrity passes; development,
strict selected and full language gates fail, retaining 1,871 full-gate reasons.
The [independent closeout review](stage41-closeout-review.json) checks the final
archive, source bindings, original logs and reported limits.
Imported types, module inheritance, remaining language and Host boundaries,
product interfaces, release platforms and speed/memory targets remain open.

## Stage 40 local module-output checkpoint

The [implementation checkpoint](stage40-implementation-checkpoint.json) records
commit `7c60a2c`, with all five required local Rust checks passing. The
[check archive](stage40-required-checks.tar.gz) preserves 3,897 source files and
raw command results. The 601 passing Rust harness invocations include child
invocations and are not a distinct feature or test count.

The [output comparison captures](stage40-output-captures.json) preserve 21,951
files from original Java/native module runs, actual Node and Chromium execution,
the full output trace selection and independent audits. The earlier native
candidate's failures remain alongside the corrected candidate. Of 60 original
module calls per target, 26 match, 30 are unsupported, two differ at `read*`
syntax, and two retain unmet original Java execution prerequisites. Each portable
target matches all 384 repeated UTF-16 output cases.

The [component and development captures](stage40-component-captures.json)
preserve 6,742 files from focused output, primitive, amendment, formatter,
uppercase and tooling checks. The archive retains earlier failed checks and
Java internal errors alongside corrected runs. Component comparisons establish
only their recorded boundaries; they do not replace full evaluator comparisons.

The standard platform corpus retains 6,076 Java matches out of 6,078 results,
including the two existing regex gaps, and 92 passing lifecycle scenarios per
portable target. Each target's 90 full-trace executions contain 58 matches and
32 Java oracle errors. Integrity success does not change these failures into
compatibility passes.

The comparison candidate predates the final test-only corrections. Archived
audits retain the exact changed file hashes and identify the differences; they
do not claim current whole-source equality. Required checks ran the final files.
The [final summary](stage40-final-summary.json) records 3,603 raw native matches
out of 3,629 attempts, with 15 unsupported cases, nine mismatches, one unmet
original Java execution contract and one Java oracle error retained. All 3,599
previous result statuses and their order are unchanged. The audit enumerates
3,473 cross-checkout raw-channel differences without normalizing physical URIs.
All 88 metamorphic relations and 2,782 calls across 334 persistent sequences
match. The persistent comparison also preserves all 16,692 prior/current raw
status and output channels exactly.

The [final regression archive](stage40-final-regression-captures.json) retains
57,814 files, including source snapshots, artifacts, full failed observations,
actual gate exits and independent audits. The [committed source binding](stage40-final-source-binding.json)
connects 8,105 source copies to 4,259 exact files in the implementation commit
and pinned submodules. Archive integrity passes; development, strict selected
and full language gates still fail, with 1,874 full-gate reasons.
The [independent closeout review](stage40-closeout-review.json) verifies those
raw bytes, archive membership, committed sources, counts and gate results.
Archive members use canonical mode `0644`; executable permission preservation
is not claimed. The review's initial mode assertion is retained with its correction.
Stage 40's local module-output checkpoint is verified. M2 and the remaining
language, product, release-platform and performance gates remain open.

## Stage 39 local verification checkpoint

The [original module frontend contract](../../docs/m2-module-frontend.md)
registers 72 unchanged upstream error cases for whole native process comparison.
The default language runner now also selects those commands. Inventory status is
3,599 executable rows, 888 unimplemented rows and 40 unmapped rows; executable
status remains separate from a passing observation and feature coverage.

[Original Java observations](stage39-oracle-captures.json) retain two CLI runs
per case, separate parser phase observations and input bindings to pinned Git
blobs. [Development, platform and regression captures](stage39-checkpoint-captures.json)
preserve the first implementation, including initial failures and successful
bounded checks. An independent review subsequently reproduced a keyword-headed
group/lambda commitment regression. The [interrupted captures](stage39-interrupted-captures.json)
retain both intentionally stopped native acquisitions, including incomplete
process captures. They are not complete suites.

The parser correction preserves ordinary keyword grouping and real identifier,
quoted identifier, underscore and empty-parameter lambda commitment. Corrected
native controls include 26 complete error matches and 14 explicit unsupported
output results. The [implementation checkpoint](stage39-implementation-checkpoint.json)
and its [source binding](stage39-implementation-source-binding.json) connect
completed local Rust checks to 4,121 exact source paths, including 72 pinned
submodule inputs. Rust logs contain 575 passing harness invocations and one
ignored invocation; child reruns are included, so this is not a distinct test count.
The [correction archive](stage39-correction-captures.json) retains the review,
actual Java controls, the test-authoring failure, the fix, and completed checks.
The [independent checkpoint audit](stage39-checkpoint-audit.json) verifies all
65,250 payload files in six archives and the source-tree bindings.
The [final summary](stage39-final-summary.json) records the corrected implementation
`5526ec808b9ee9bfd743baa33bbc1a7d2364471b`: 3,590 of 3,599 selected native cases
match, with eight existing mismatches and one cycle oracle error retained.
All 72 original frontend commands match complete native output, exit status and
generated-file observations. Actual Node and Chromium each match 144 original
frontend API calls; their API result status is separate from native process exit
and file I/O. Twelve distinct internal producer-phase differences remain visible.
Four accepted output boundaries remain explicitly unsupported on both portable
targets, rather than being counted as successful evaluation.

The [regression comparison](stage39-regression-comparison.json) preserves the
definitions, input bytes, order and outcomes of all 3,527 existing scalar cases.
The new checkout has a different physical URI: 1,664 Java and 1,665 Rust captures
differ byte-for-byte from Stage 38 and are individually retained without path
normalization. No cross-checkout byte-identity claim is made. All 88 metamorphic
relations and 2,782 selected calls across 334 persistent sequences match.
Actual trace observations retain 54 matches and 30 Java oracle errors. The standard
platform comparison retains 6,078 shared results, 6,076 Java matches per target,
92 architecture scenarios and the two existing regex differences.

The [final source binding](stage39-source-binding.json) ties 8,512 source copies
across five corrected suites to 4,289 distinct paths in the implementation commit
and pinned upstream Git blobs. Corrected [platform](stage39-final-platform-captures.json),
[persistent/trace](stage39-final-regression-captures.json) and
[native language](stage39-final-language-captures.json) archives retain full raw
results, source snapshots, artifacts, gate failures and publication records.
The [independent archive audit](stage39-final-archive-audit.json) verifies the
corrected platform/regression archives, including terminal logs adjacent to their
audit directories and the preserved earlier audit inventory failure. Only exact
Cargo cache paths are pruned before traversal; ordinary build logs are included.
The [final independent audit](stage39-post-seal-audit.json) also verifies the
native archive, committed source bindings, unchanged original scalar contracts
and exact cross-checkout differences. [Completion captures](stage39-completion-captures.json)
retain its raw observations, terminal records and publication helpers.

Report integrity passes. Development, strict selected and full language gates
still fail, with 1,887 full-gate reasons and no new exceptions. The earlier
candidate's passing checks remain historical evidence, not final candidate
verification. **Stage 39's local frontend checkpoint is verified; M2 remains in
progress.** Module output, loading, remaining language semantics, product APIs,
release environments and execution-time/peak-RSS goals remain unfinished.

## Stage 38 local verification checkpoint

Implementation and final acquisitions are committed and pushed as
`5515462f773c676f4c9652e86ac0c6439f385127`. The
[source binding](stage38-source-binding.json) verifies 4,219 distinct source paths
against exact Git blobs. [Publication receipts](stage38-completion-captures.json)
and their [archive](stage38-completion-evidence.tar.gz) retain the implementation
commit, push and remote-head verification.

The [resource-read contract](../../docs/m2-resource-reads.md) describes dedicated
`read`/`read?` expressions, nullable absence, declaration-source URI handling and
the current empty environment/property profile. Permitted I/O readers and
lossless unmatched-surrogate URI transport remain unfinished.

Independent review found and preserved a portable comparison-boundary error and
a cross-scope method-suggestion ordering difference. The corrected portable
driver retains original Java UTF-8 bytes separately from exact JS string-unit
comparison. The corrected runtime matches 45 focused Java observations,
including nine new general ordering controls.

The [final summary](stage38-final-summary.json) records 3,527 selected cases:
3,518 matches, eight mismatches and one cycle oracle error. All original 3,427
definitions, input bytes and order are preserved. Two former trace/resource
diagnostic mismatches now pass; the other 3,425 native raw outcomes are unchanged.
Java cycle-error byte variation is retained separately. All 88 metamorphic
relations and 2,782 persistent calls match. Report integrity passes, while
development, strict selected and full language gates fail; the full gate has
1,959 reasons. The 960 unimplemented and 40 unmapped rows remain unchanged.

Actual Node and Chromium each match 87 of 91 resource/method cases, retaining
one unmatched-surrogate URI mismatch and three explicit Java internal errors.
All nine method-order cases and 18 trace expressions match. The separate 20
repeat cases retain two legacy mismatches. Each target's 42 actual-source trace
definitions run twice, with 54 passing and 30 oracle-error experiments. Standard
shared-engine comparisons match 6,078 cases; 6,076 match Java, retaining two
regex gaps. These overlapping case, call and repetition counts are not additive
feature coverage. Source-path scalar HTTP internal errors retain their legacy
mismatch classification; the portable driver reports them as oracle errors.

Required local Rust, Wasm, manifest and seven portable-driver checks pass. The
first complete Python run failed one test that expects child startup and output
within 200 ms. The unchanged single test and unchanged full 235-test discovery
subsequently pass. Exact original child progress was not retained, so scheduling
is a plausible cause rather than a proven diagnosis. Both runs remain available.

[Origin captures](stage38-origin-captures.json) identify two verified archives:
[Java observations](stage38-oracle-evidence.tar.gz) and
[development evidence before the ordering correction](stage38-development-evidence.tar.gz).
Their inventories preserve original failures, intermediate source/artifact
identities and the earlier comparator contract. The original helper mistakenly
excluded four subprocess capture directories named `build`, omitting 12 files.
The [candidate archive](stage38-candidate-evidence.tar.gz) retains those exact
files and the corrected final worktree evidence. Its [independent verification](stage38-candidate-captures.json)
confirms union coverage of all 37,571 files declared by 56 available original
closure manifests. The two original archives remain unchanged.

The [final archive](stage38-final-evidence.tar.gz) preserves final MAIN results,
prior integrated attempts, the Python failure/retry, independent audits and
closeout helpers. Only explicitly identified Cargo cache trees are pruned before
traversal. The [source binding](stage38-source-binding.json) connects final native,
Node and Chromium source snapshots to the implementation commit.

**M2 remains in progress.** Allowed resource I/O, complete URI transport,
module loading/cache and remaining language, API, release-platform and performance
conditions remain open. The next work targets 72 original upstream frontend
error `eval` contracts, preserving stdout, stderr, exit codes and generated files.

## Stage 37 local verification checkpoint

Implementation and final acquisitions are committed and pushed as
`646cf78a9b3775bf25e63e09146856c34225665e`. The
[source binding](stage37-source-binding.json) verifies 4,113 distinct source paths
from final checks and native/Node/Chromium acquisitions against exact Git blobs.
The captured check helper is retained separately rather than counted as a
committed production source. [Completion captures](stage37-completion-captures.json)
retain independent archive/source audits and the implementation commit, push and
remote verification records.

The shared runtime now connects trace evaluation, source-frame construction,
UTF-16 logger transport, and callback completion. Fresh source-path evaluation
keeps module initialization and the first path within one admitted Host context
and deadline. Later starts still observe callback closure. See the
[trace development contract](../../docs/m2-tracing.md) for interfaces, acquisition
commands, and remaining scope.

The [final summary](stage37-final-summary.json) records **3,420 raw matches out of
3,427 selected cases** and all 88 metamorphic relations. The original 3,389 input
definitions, ordering, statuses and native result bytes are unchanged. Java differs
only in the existing cycle oracle-error stderr; both original captures are retained.
The new 38 cases yield 34 matches. Two default-resource diagnostic differences and
two Java String.repeat internal-error cases remain failures, alongside the existing
cycle oracle error, remainder mismatch and SourceCode mapped-list mismatch.
Integrity verification passes; development, strict selected and full language gates
fail without new exceptions. The full gate retains 1,957 failure reasons, which are
overlapping accounting obligations rather than independent feature counts.

All **2,782 requested calls** across 167 persistent definitions and two fresh
executions match Java status/value/error bytes. This includes the formerly
unexecuted 54 trace-dependent calls. The prior 2,728 native and all 2,782 Java
observations remain unchanged. The selected sequence contract passes; its five
unexecuted groups still prevent whole persistent compatibility completion.

The separate 42-contract actual-source logger suite runs twice per contract.
Native, Node and Chromium each record **54 passes and 30 oracle errors**, with no
ordinary-result mismatches. Bootstrap-close first calls return the exact value
`8`; later Java internal errors still make their complete experiments fail.
Real session cleanup and retained UTF-16 event snapshots are verified separately.
No Java internal error is relabeled as a passing language result.

All seven required local checks pass, including 235 Python tests. The first
full-check failure from a stale semantic-family count and the corrected complete
rerun are preserved. Actual Node and Chromium each pass 92 lifecycle scenarios;
all 6,078 shared primitive results agree, while the same two Java regex differences
remain. Each platform matches 16 of 18 scalar trace cases and 18 of 20 repeat cases,
retaining the same four new differences as native.

The [origin archive](stage37-origins-evidence.tar.gz) and
[final archive](stage37-final-evidence.tar.gz) preserve original observations,
intermediate failures, final acquisitions, sources, artifacts and independent
audits. Named Cargo caches are excluded before traversal. This is local macOS
and actual Node/Chromium evidence, not hosted CI or Linux/Windows validation.
All nine M2 workstreams remain open; complete module loading, resource caching,
REPL, warning delivery, frame transformers, public APIs and performance targets
retain their original obligations.

## Stage 36 local verification checkpoint

Implementation `462ae5d333da629cd871eb48a55be58d6cc0a5ff` is committed and pushed.
The [source binding](stage36-source-binding.json) connects 4,016 distinct source
paths from the completed checks and acquisitions to their exact Git blobs.
The [completion captures](stage36-completion-captures.json) preserve the final
archive, commit, push, remote verification and source-binding process records.

Object string conversion now uses resumable deep forcing, persistent completed
force markers, scoped undefined handling and inherited render order. Object-key
hashing/equality runs through the shared VM during runtime formatting. Typed
interpolation dispatches authored `toString` overrides. Host callback errors retain
suspended caller source frames, and Dynamic missing-member errors use the shared
name-suggestion path.

The [final summary](stage36-final-summary.json) records **3,386 raw matches out of
3,389 selected cases** and all 88 metamorphic relations. The existing remainder
mismatch, cycle oracle error and SourceCode mapped-list mismatch remain failures.
Three earlier object-string-dependent mismatches are repaired; none of the old
passing cases regressed. All 83 new cases match native Java comparisons, compared
with five matches for the frozen Stage35 baseline. Node and Chromium each match
all 83 cases in two fresh executions per case.

The persistent suite retains 167 definitions and 334 executions. All 2,728 paired
calls match; 54 requested native calls are unexecuted because six original new
definitions require unsupported `trace`. Their inputs and setup errors remain
selected. Integrity gates pass, while development, strict selected, full language
and persistent compatibility gates fail. The full language gate has 1,951 failure
reasons. Its 474 requirements, 21 partial mappings and 4,389 overlapping accounting
rows (3,389 ready, 960 unimplemented, 40 unmapped) are not completion percentages.

All seven required local checks and 208 Python tests pass. Rust logs contain 506
passing harness invocations, including child repeats, and one ignored test; this
is not a distinct-test count. Standard Node/browser checks retain 92 scenarios
per target, 6,078 shared primitive matches and 6,076 Java matches per target, with
the two known regex differences. M1 frontend comparisons match 15/15, runtime
comparisons 8/9, and lifecycle protocols match while four complete diagnostics
remain different. No new evaluator performance or peak-RSS result is claimed.

Ordinary actual-source Host experiments match 14/14 repetitions and all 72 calls.
Separate diagnostic-key experiments retain two mismatches and two truncation
control passes. All eight nested-read experiments remain Java oracle errors,
although 16 complete inner language-error payloads match. Additional callback
controls match all 270 expected raw channels for 90 calls, but a resource-cache
gap produces eight extra native callbacks; that suite fails its whole protocol.

The [original observations](stage36-origins-evidence.tar.gz) preserve intermediate
failures, Java bugs, source snapshots and independent reviews. The
[final evidence](stage36-final-evidence.tar.gz) preserves completed main checks,
full raw acquisitions, baseline replays and gate exits. Cargo build caches are
excluded before archive traversal. **M2 remains in progress**: trace, diagnostic
Host rendering, resource caching and all remaining language/upstream obligations
stay open, alongside product API, release-platform and performance requirements.

## Stage 36 acquisition tooling checkpoint

The language runner accepts an optional `--workers` count while retaining fresh
Java/Rust processes, separate raw captures and manifest-order results. Serial
execution remains the default. The [tooling checkpoint](stage36-tooling-checkpoint.json)
records 208 passing Python tests, current manifest validation, and six fresh
serial/parallel cases with identical raw channels, including the existing
remainder mismatch. The [raw archive](stage36-tooling-evidence.tar.gz) preserves
the initial driver failure, worktree freshness failures and final main checks.
This improves acquisition throughput. At that tooling checkpoint, Stage 36 runtime
work was still in progress; no evaluator speed or memory result was claimed.

## Stage 35 local verification checkpoint

Declaration annotations now run through the shared parser, resolver and VM,
including ordered initialization, retained class/alias annotation prefixes,
member publication, seven builtin classes and source-driven Host suspension.
Implementation `62919b7d233689dc5d4ff8210b23e998357d0ac2` is committed and pushed.
The [final summary](stage35-final-summary.json) records **3,300 raw matches out of
3,306 selected cases**, with five mismatches and one cycle oracle error retained.
All 88 metamorphic relations, 2,720 persistent calls and 68 actual-source Host
call comparisons match. The 105 new cases improve from zero to 103 matches on
native; Node and Chromium also match 103 cases, executing each case twice.
General object `toString` and `List.map` failures remain selected, without new
exceptions. Original upstream annotation inputs match 7/17 at the tested API
boundary; their full snippet/output contracts remain open.

Seven required local checks and 200 Python tests pass. The Rust test log records
495 passing harness invocations, including child repeats, and one ignored test;
this is not a distinct-test count. Source and artifact stability checks pass.
Report integrity passes, while development, strict subset and full completion
gates fail. The full gate retains 1,954 failure reasons. The overlapping inventory
contains 474 requirements, 20 partial mappings and 4,306 accounting rows:
3,306 ready, 960 unimplemented and 40 unmapped. These are not completion percentages.

Independent audits bind the final sources to the exact implementation commit and
verify raw evidence. All original 3,201 case definitions, inputs, ordering and
statuses are preserved, with identical Rust output/error bytes. The already-failed
Java cycle diagnostic changes its guest-frame count; both captures remain
unaltered and classified as an oracle error. The [origins archive](stage35-origins-evidence.tar.gz)
and [final archive](stage35-final-evidence.tar.gz) retain failed candidates,
Java observations, completed checks and audits; their
[origins identity](stage35-origins-archive.json) and
[final identity](stage35-final-archive.json) record caches pruned before traversal.
The earlier [implementation checkpoint](stage35-implementation-checkpoint.json)
remains an unchanged historical record from before the full acquisition finished.

**M2 remains in progress.** All nine workstreams retain completion conditions.
Local macOS checks do not establish full upstream execution, release-platform
coverage, public API completeness, execution-time parity or peak-RSS parity.

## Stage 34 local verification checkpoint

Implementation `b63163dd1bd6376fbbe400fc38611b5495629643` is committed and pushed.
The [final summary](stage34-final-summary.json) and
[raw evidence archive](stage34-final-evidence.tar.gz) record this bounded checkpoint.
Core implementation used `gpt-6-astra / xhigh`; Java comparison and independent
review each used `gpt-6.1-sol / high`. Verification ran locally on macOS arm64.

Contextual `new {}` now uses declared method return-type defaults, including
class, Listing, Mapping, alias, nullable and default-union cases. Literal
`unknown` remains distinct from `Any` and aliases to unknown. Inherited class
prototypes use declared type defaults, while instance amendments read prototype
initializers. Generated default source locations, first-error selection and VM
cleanup preserve the checked Java behavior.

The complete selected corpus has **3,197 raw matches out of 3,201 cases**:
three mismatches and one cycle oracle error remain. All 88 metamorphic relations
and 2,622 persistent class/alias calls pass. The 79 additions contain 46 Java
successes and 33 expected errors. They improve from 16 to 77 raw matches,
repairing 61 cases with no baseline-pass regressions. The same final native
artifact also matches seven selected API expressions against the original
upstream `newInsideLet.pkl`; this does not complete its original snippet renderer
contract.

Node and Chromium each execute all 79 additions twice: 77 matching cases and
two mismatching cases per target. The two original `Dynamic.toString()` cases
remain selected and failed; separate type controls do not replace them. After
parent inference is fixed, their first error advances from a Listing/Dynamic
type mismatch to missing `toString`. Both old and new raw errors are preserved.
The portable comparison exits 1. No new development exception was added, so the
language development, strict subset and full completion gates fail. The full
gate reports 1,952 failure reasons. The inventory retains 474 requirements,
16 partial mappings and 4,201 overlapping accounting rows: 3,201 ready,
960 unimplemented and 40 unmapped. These counts are not completion percentages.

Seven required local checks pass with **473 distinct Rust tests and 200 Python
tests**. The Rust log has 486 passes including 13 isolated child repetitions,
plus one ignored test. Six new native tests cover parent inference, boundaries,
first errors, prototype/instance behavior, default origins and Host/GC/retry/
cancel/close behavior. Existing Node/browser development checks pass 92 scenarios
per target; the two existing Java regex differences remain failed compatibility
comparisons. M1 frontend comparisons pass 15/15; runtime comparisons match 8/9,
and lifecycle protocol success retains its full diagnostic differences.

The [independent audit](stage34-independent-audit.json) binds recorded sources
and artifacts to the implementation and checks raw outcomes. The archive includes
final suites, Java origins, intermediate failures and the first incomplete
validation/language runs, which were interrupted to correct diagnostic differences.
Initial audit assertion assumptions are retained alongside their corrected
results. The Cargo build cache is excluded before traversal, as recorded in the
[archive identity](stage34-archive.json). Java-only prototype trace sequences are
observations, not evidence of Rust trace equivalence.

**M2 remains in progress.** All nine workstreams retain completion conditions,
including annotation/module initialization, remaining type/collection/Reference
semantics, complete diagnostics and original upstream execution. Local checks do
not establish release-platform coverage, public API completeness, execution-time
parity or peak-RSS parity.

## Stage 33 local verification checkpoint

Implementation `571d46a69d22a373e14022b582459f9bb50738d8` is committed and pushed. The
[final summary](stage33-final-summary.json), [independent audit](stage33-post-seal-audit.json)
and [completion captures](stage33-completion-captures.json) close this bounded
frontend checkpoint. Core work used `gpt-6-astra / xhigh`; Java evidence and
native/platform review each used `gpt-6.1-sol / high`. Root handled integration,
final validation, commits and sealing on local macOS arm64.

Non-local object `const` modifier errors now name the enclosing declaration
before the rejected member scope is entered. Method bodies permit contextual
`new {}` through the existing `if` and let-body paths; lambdas and parentheses
retain the Java rejection boundaries. The existing diagnostic owner replay
handles these two errors alongside forbidden non-local annotations. It adopts
ownership only when every other diagnostic field matches, retaining the original
owner on divergence. Valid resolution allocates no ownership stack. The checked
untyped and `Dynamic` return methods use the existing compiler behavior; broader
return-type parent inference remains incomplete.

All **36 additions match Java: 27 expected errors and
9 successful values**. The full selected corpus contains
3,122 cases, with 3,120 raw matches and the existing cycle oracle
error and remainder-by-zero mismatch. 88 metamorphic relations and
2,622 sequence calls pass. The prior 3,086 selected
inputs/definitions and 151 sequence definitions remain
preserved. The inventory has 474 requirements and
4,122 accounting rows: 3,122 ready, 960
unimplemented and 40 unmapped. Inventory freshness, the development
contract and the strict full gate are separate; the full gate retains
1,950 failure reasons and 16 partial mappings.

[Seven required local checks](validation-stage33-final.json) passed with
467 distinct Rust tests and 200 Python tests. The Rust harness reports
480 passes, including the existing 13 isolated child repetitions, with
one ignored test. Four new ordinary native tests cover const owners,
method/lambda first errors, earlier lexical errors and accepted lazy controls.
The existing predicate-owner expectation was corrected from `#3` to `#5` after
fresh Java confirmed the builder's prefix revisit when it switches to generator
handling. These checks make no new Host/lifecycle, small-stack or memory claim.

Node and Chromium each passed 92 development scenarios. The new
scenario compares all 36 complete Java error/value payloads twice per
target using fresh evaluators. Existing primitive results and the two known
Java regex gaps remain. A fresh baseline matches 12 of the
new selected cases; the exact final language artifact matches all 36.
The broader 41-case comparison improves from
15 to 39 raw matches. All five original
Stage 32 failures are repaired in these observations; inferred parents from
class and `Listing` method return types remain mismatched. Selected, broader and
platform comparisons overlap and are not additive feature counts.

Original failures and incomplete runs remain preserved. The first native run
exited 101 because a new lexical assertion omitted the diagnostic's second
paragraph; it had no complete source snapshot and is exploratory evidence.
A fresh helper named `select.py` shadowed Python stdlib `select` during a
subprocess import and exited 1 after duplicating selection rows. The helper was
renamed `select_fixtures.py` with a single-use guard, and baseline input bytes
and the corrected 36 additions were verified again. A completed
native run for the earlier owner candidate remains separate from its rerun
after the fallback-owner refinement. The first workspace validation exited 1
when Cargo reported 101 for the stale predicate-owner expectation. The first
language and class-sequence processes were interrupted with exit `-15`; their
incomplete observations are not passes. Fresh final directories retain the
corrected checks and comparisons. The first frontend invocation checked only its contract; subsequent actual
frontend comparisons are separate. Completed initial platform/frontend runs
retain their own source identities rather than being relabeled as final reruns.

The [sealed archive](stage33-final-evidence.tar.gz) and
[identity record](stage33-archive.json) preserve 85,390 original files under
147 explicit roots. The independent audit binds 7,718 final
source copies to 3,705 implementation files and checks raw outcomes,
failed captures, fixed-URI Java payloads and immutable origins. The completed
origin binding connects the final focused artifact to full language evidence.
Seal/audit captures and current closeout source snapshots are preserved after
sealing. Explicit Cargo cache roots are omitted before enumeration;
7 direct historical dependency identities remain
separate from the new evidence.

M2 remains in progress. Typed method parent inference, overflow frame ownership,
empty-collection unavailable source sections, existing frontend/runtime/lifecycle
gaps, URI and public string transport, complete APIs and depth/work limits remain
open. Local verification does not establish Linux, Windows, hosted CI, release
platform coverage, execution-time parity or peak-RSS parity.

## Stage 32 local verification checkpoint

Implementation `af1917d65a3043b9259343cb334d7c6431f8e4e6` is committed and pushed. The
[final summary](stage32-final-summary.json), [independent audit](stage32-post-seal-audit.json)
and [completion captures](stage32-completion-captures.json) close this bounded
frontend diagnostic checkpoint. Core work used `gpt-6-astra / xhigh`; Java
evidence and native/platform review each used `gpt-6.1-sol / high`. Root handled
integration, final validation, commits and sealing on local macOS arm64.

Forbidden non-local object property type annotations now name their declaration
owner. Nested properties, methods, lambdas, lets, entries and generators preserve
the pinned Java scope names, including flattened lambda numbering, generator
prefix revisits, quoted identifiers and UTF-16 entry keys. One resolver replay
collects this metadata after the rejection; it adopts the owner only when every
other diagnostic field equals the original error. Valid resolutions allocate no
ownership stack. Annotation acceptance, earlier error ordering and numeric
evaluation remain unchanged.

All **36 additions match Java: 32 expected errors and
4 successful values**. The complete selected corpus contains
3,086 cases, with 3,084 raw matches and the existing cycle oracle
error and remainder-by-zero mismatch. 88 metamorphic relations and
2,622 sequence calls pass. The prior 3,050 selected
definitions/input bytes and 151 sequence definitions remain
preserved. Inventory freshness, the development contract and the strict full
gate remain separate; the full gate retains 1,950 failure reasons.

[Seven required local checks](validation-stage32-final.json) passed with
463 distinct Rust tests and 200 Python tests. The Rust harness reports
476 passes, including the existing 13 isolated child repetitions, with
one ignored test. Four new ordinary native tests verify rejected owners, first
error ordering, UTF-16 identity and accepted lazy annotation controls. They make
no new Host/lifecycle, small-stack or memory guarantee.

Node and Chromium each passed 91 development scenarios. The new
scenario compares all 36 complete Java error/value payloads twice per
target using fresh evaluators. Existing primitive results and the two known
Java regex gaps remain. A fresh baseline matches 5 of the
new selected cases; the exact final language artifact matches all 36.
Exploration changes from 11 to 53 matches
among 58 cases, retaining five existing inferred-new and earlier
const-modifier diagnostics. These overlapping observations are not additive
feature counts.

Original failures remain preserved. Baseline and early native owner checks
failed before the corrected implementation and explicit-type controls. The
first naming helper overclassified double-negated integer/unit keys; its raw
mismatches and successful superseded build remain separate from the final
result. A missing browser route for the fixture module caused the first platform
run to fail after Node passed. Its one-line correction and the resulting source
change during the first validation run required new validation/platform output
directories. The failed runs and superseded reviews are not counted as passes.

The [sealed archive](stage32-final-evidence.tar.gz) and
[identity record](stage32-archive.json) preserve 97,159 original files under
176 explicit roots. The independent audit binds 7,640 final
source copies to 3,667 implementation files and checks raw outcomes,
failed captures, fixed-URI Java payloads and immutable origins. A completed
origin binding connects the final focused artifact to full language evidence.
Seal/audit captures and current closeout source snapshots are preserved after
sealing. Explicit Cargo cache roots are omitted before enumeration; their
contents are not counted or hashed. 7 direct historical
dependency identities remain separate from the new evidence.

M2 remains in progress. Overflow frame ownership, empty-collection unavailable
source sections, existing frontend/runtime/lifecycle gaps, URI and public string
transport, complete APIs and depth/work limits remain open. Local verification
does not establish Linux, Windows, hosted CI, release platform coverage,
execution-time parity or peak-RSS parity.

## Stage 31 local verification checkpoint

Implementation `92b87ccbd999b4d1648b15b43e853643fdb62f94` is committed and pushed. The
[final summary](stage31-final-summary.json), [independent audit](stage31-post-seal-audit.json)
and [completion captures](stage31-completion-captures.json) close this bounded
value-frame source-ownership checkpoint. Core work used `gpt-6-astra / xhigh`;
Java evidence and native/platform review each used `gpt-6.1-sol / high`. Root
handled integration, final validation, commits and sealing. Verification ran
locally on macOS arm64.

Value-call frames now distinguish authored syntax from the executable expression.
Direct numeric negation retains the unsigned numeric token's location; grouping
around the complete expression is removed, while a unary operator on a grouped
operand keeps its expression span. Typed locals and function bodies use these
executable locations. Constant members follow Java's separate source fallback
after their declaration header, preserving authored signs, groups and comments
in public properties, class defaults, overrides and entries. Constant element
headers can leave a closing-parenthesis suffix, matching the pinned Listing
diagnostic. Authored AST, resolution and assertion-observation identities remain
unchanged, as do arithmetic, evaluation order and declaration-origin checks.

The first candidate applied executable locations too broadly to public
constants. Fresh class/module/override/collection controls and a native test
exposed that regression. The first focused comparison, supplemental mismatches,
native failure and superseded review remain preserved. An initial baseline build
also failed because the extracted workspace omitted `xtask`; its corrected build
used a fresh directory. Final results use the corrected source and exact final
language artifact. The initial baseline build and native control exited 101;
the first fixture-generation command exited 1. These three process failures are
retained explicitly. The initial static auditor review also required stronger
focused-source and exploratory-origin checks; its findings and the corrected
review remain preserved. Production evaluation has no Java fallback.

The selected corpus has **3,050 cases: 3,048 raw matches**, the existing cycle
oracle error and integer remainder-by-zero mismatch. All **48 additions match:
43 expected errors and five successful values**. All **88 metamorphic relations**
pass. The prior 3,002 definitions and input bytes, 151 sequence definitions and
89 platform scenarios remain preserved. Persistent evaluation retains **2,622
matching calls** across 302 executions. The inventory has 474 requirements and
4,050 accounting rows (3,050 ready, 960 unimplemented, 40 unmapped). The 16
reviewed mappings remain partial, and the full gate retains 1,950 failure reasons.
No development compatibility exception was added.

[Seven required local checks](validation-stage31-final.json) passed with
**459 distinct Rust tests and 200 Python tests**. The raw Rust harness
reports 472 passes, including 13 existing isolated child repetitions, and
one intentionally ignored test. Three new ordinary native tests check value-frame
ownership, signed values, per-instruction GC, suspended constraints, reentry,
retry, shared programs and detached errors after close. Two compiler units retain
authored observation spans and check scalar/collection/comment/Unicode source
suffixes. These checks establish no new small-stack or memory-parity guarantee.
Archive verification now indexes previously validated members; duplicate-name,
inventory, length and digest checks remain intact.

Node and Chromium each passed **90 development scenarios**. The new scenario
compares 48 complete Java-derived error/value payloads twice per target (96 calls),
using fresh evaluators. Native Host tests separately cover recovery within a
session. All 6,078 shared primitive results match across targets; each target
matches Java on 6,076, retaining the two known regex gaps. Existing runtime cycle
and full lifecycle diagnostic differences remain.

A fresh baseline build matches **28 of 48 new selected cases**; the exact final
language artifact matches **48 of 48**. Broader exploration matches **119 of 124**,
compared with **78 of 124** for the baseline. Four invalid public-object annotation
frame differences and one nested-minimum-negation overflow frame difference remain.
Selected, platform and exploratory observations overlap and are not additive
feature counts. Java's unavailable source sections for empty collection calls
remain a separate diagnostic gap.

The [sealed archive](stage31-final-evidence.tar.gz) and its
[identity record](stage31-archive.json) preserve 59776 original files under
89 explicit roots. The independent audit checks inventory and original
bytes, Git provenance, raw outcomes and the final focused artifact. It binds
7555 final suite source copies to 3628 distinct
implementation files. A completed-origin binding separately connects the focused
results to the finished full-language report, source snapshots, artifact and raw
captures. Completed seal/audit captures and current closeout source snapshots
are preserved separately after sealing. Only explicitly listed Cargo build caches
are excluded before enumeration; their payload sizes and hashes are not claimed.
The initial baseline checkout and fresh baseline source snapshots are retained;
the absent initial build-output directory is recorded separately.
7 historical dependency
identities are retained without recursively packaging prior evidence.

M2 remains in progress. Complete numeric/constant-source diagnostics, imported
and surrogate URI handling, out-of-range Unicode internal errors, public JS string
transport, colored/stack-overflow output, full string APIs and existing depth/work
limits remain open. Local macOS verification does not establish Linux, Windows,
hosted CI, release APIs, execution-time parity or peak-RSS parity.

## Stage 30 local verification checkpoint

Implementation `efd891f109466a08cc8e3c281a0aa814c5feb0dd` is committed and pushed. The
[final summary](stage30-final-summary.json), [independent audit](stage30-post-seal-audit.json)
and [completion captures](stage30-completion-captures.json) close this bounded
diagnostic identity and declaration ownership checkpoint. Core work used
`gpt-6-astra / xhigh`; Java evidence and native/platform review each used
`gpt-6.1-sol / high`. Verification ran locally on macOS arm64.

Diagnostic member names now retain UTF-16 units through parser/resolver/compiler
scope counters, block metadata, runtime frames and recursive compression.
Different unmatched surrogates and literal question marks have independent
numbered children and distinct frame identities. Valid surrogate pairs and
literal supplementary characters remain equal. Java UTF-8 replacement occurs
when the frame is displayed. Delayed integer errors retain their authored
entry/element scope, while key expressions retain their enclosing scope.
Production evaluation has no Java fallback.

Typed-property checks now record their declaration origin. Object-local checks
run inline; module/class declarations retain a separate value-call frame,
including local declarations. An initial implementation suppressed that frame
too broadly. The existing platform function-metadata scenario detected the
regression; fresh Java controls and independent source review established the
correct distinction. The failed platform run and deliberately interrupted first
validation/language/sequence acquisitions are preserved. Corrected sources ran
through all final suites in fresh directories.

The selected corpus has **3,002 cases: 3,000 raw matches**, the existing cycle
oracle error and integer remainder-by-zero mismatch. All **23 additions are
expected errors with complete matching diagnostics**; all **88 metamorphic
relations** pass. The prior 2,979 definitions/input bytes, 151 sequence definitions
and 88 platform scenarios remain preserved. Existing native assertions received
constructor/accessor adaptations for the new name type. Persistent evaluation
retains **2,622 matching calls** across 302 executions. The inventory has 474
requirements and 4,002 accounting rows (3,002 ready, 960 unimplemented, 40 unmapped).
All 16 reviewed mappings remain partial; the full gate retains 1,950 failure
reasons. No development exception was added.

[Seven required local checks](validation-stage30-final.json) passed with
**454 distinct Rust tests and 198 Python tests**. The raw Rust harness
reports 467 passes, including 13 existing isolated child repetitions, and
one intentionally ignored test. Four new ordinary native tests inspect internal
UTF-16 names, canonical pairs, numbered children, delayed diagnostics, declaration
ownership and Host reentry/retry/GC/shared-program/close behavior. A new compression
unit checks distinct names and repeated patterns before writer replacement.
These additions do not establish new small-stack or memory-parity guarantees.

Node and Chromium each passed **89 development scenarios**. The new scenario
checks 23 complete Java-derived error payloads twice per target (46 calls).
All 6,078 shared primitive results match across targets; each target matches
Java on 6,076, retaining the two known regex gaps. Existing runtime cycle and
full lifecycle diagnostic differences remain. Public JS string transport and
release API behavior were not established by these development adapters.

A fresh build of the baseline commit matches **12 of the 23 new cases**; the
corrected focused build and exact final language artifact match **23 of 23**.
The separate ownership matrix matches **15 of 16 calls**. Its remaining
negative-literal value-frame underline difference also occurs in the fresh
baseline, with identical baseline/candidate output; a nonnegative control passes.
That preexisting gap remains explicit. Synthetic pinned-JAR probes separately
check 36 frame-equality pairs and five compression/rendering sequences. These
observations and platform cases overlap selected coverage and are not additive
feature counts. Java class identity sidecars are clearly post-execution
observations, not retroactive execution-time identity claims.

The [sealed archive](stage30-final-evidence.tar.gz) and its
[identity record](stage30-archive.json) preserve 83,858 original files under
116 explicit roots. The independent audit checks the checksum inventory,
original bytes, Git provenance, raw outcomes and the exact final artifact used
for the focused comparison. It binds 7,453 final suite source copies
to 3,578 distinct implementation files. Completed seal/audit captures
are preserved separately because those processes finish after archive creation.
Direct build caches and the unused baseline checkout are excluded before archive
enumeration; their payload sizes/hashes are not claimed. Seven direct Stage 29
dependency identities are retained without recursively packaging history.

M2 remains in progress. The complete diagnostic corpus, imported/surrogate URI
handling, out-of-range Unicode internal errors, signed-literal diagnostic spans,
colored/stack-overflow output, full string APIs and existing depth/work limits
remain. Local macOS verification does not establish Linux, Windows, hosted CI,
release APIs, execution-time parity or peak-RSS parity.

## Stage 29 local verification checkpoint

Implementation `cb95430d89606d820108907b9d41b3d3f5c2ccc8` is committed and pushed.
The [final summary](stage29-final-summary.json), [independent audit](stage29-post-seal-audit.json),
and [completion captures](stage29-completion-captures.json) close this bounded
lossless UTF-16 string checkpoint. Core implementation used `gpt-6-astra / xhigh`;
Java evidence and native/platform review each used `gpt-6.1-sol / high`.
Verification ran locally on macOS arm64.

The shared `PklString` representation retains unmatched UTF-16 surrogates while
keeping well-formed strings in canonical UTF-8. Concatenation and interpolation
can form surrogate pairs across fragment boundaries. Equality, Java ordering
and hash, code point length, literal types/defaults, collection keys, `toString`,
and Reference metadata preserve the internal units. The explicit Java UTF-8 writer
conversion replaces unmatched units with ASCII `?` only at that output boundary.
Strict List rendering uses the existing bounded collection renderer. Production
evaluation has no Java fallback.

The complete selected corpus has **2,979 cases: 2,977 raw matches**, the existing
cycle oracle error, and the existing integer remainder-by-zero mismatch. All
**54 additions (40 successes and 14 expected errors)** and all **88 metamorphic
relations** pass. The original 2,925 fixture definitions and input bytes,
151 persistent sequence definitions, 87 platform scenarios, and reference pins
were preserved. Two older native tests received constructor-only adaptations;
their assertion semantics were preserved. Persistent evaluation retains
**2,622 matching calls** across 302 executions. The inventory has 474 requirements
and 3,979 accounting rows (2,979 ready, 960 unimplemented, 40 unmapped); all 16
reviewed mappings remain partial. The full gate retains **1,950 failure reasons**.
No development exception was added.

[Seven required local checks](validation-stage29-final.json) passed with
**449 distinct Rust tests and 198 Python tests**. The Rust harness reports
462 passes, including 13 existing isolated child repetitions, plus one
intentionally ignored test. Four new ordinary native tests check internal units,
Reference metadata identity, Host suspension/reentry/retry, collection on every
instruction, captured values, shared programs, and cancel/close/timeout cleanup.
These four tests do not establish 256 KiB process-isolated stack behavior;
the older isolated regressions remain in the workspace suite.

Node and Chromium each passed **88 development scenarios**, including 24
Java-derived string cases repeated twice in the new scenario. All 6,078 shared
primitive results match across targets; each target matches Java on 6,076,
retaining the same two regex failures. Node v26.10.0, Chromium 156.0.8078.4 and
Playwright 1.64.1 were recorded, with no browser errors. Existing M1 cycle and
full lifecycle diagnostic differences remain. These development calls observe
the Java UTF-8 writer boundary; public JS input/JavaString transport remains open.

Fresh Java acquisitions, the focused artifact and the exact final language
artifact compare **66 exploratory observations: 62 matches and four mismatches**.
All 54 selected cases and eight additional List controls match. The remaining
differences are out-of-range Unicode, two resource controls and surrogate import.
The `read(prop:?)` control is a general unsupplied Host limitation, not surrogate
identity collapse. The four original Stage 28 surrogate controls now match;
the original out-of-range control still differs. Four overlapping Java API
observations separately confirm actual UTF-16 units. Exploratory and API rows
are not additive feature coverage.

The [evidence archive](stage29-final-evidence.tar.gz) retains 59,795 original files
under 105 explicit roots. The [archive identity](stage29-archive.json) records
31,614,865 bytes and SHA-256
`bce1d62f388b94c1ac2c69b4783608890cb26f637ed492cf19b0a64e5904d9e2`.
Its checksum inventory verifies 59,801 files, including packaging metadata and
Git provenance. The independent audit binds 7,401 final suite source copies to
3,553 distinct files at the implementation commit and recomputes raw outcomes.
Completed seal/audit process captures are preserved separately because they
finish after archive creation. Three direct build caches were pruned before
enumeration; their payload sizes and hashes are not claimed. Fourteen direct
Stage 28 dependency identities remain references rather than recursively copied
history. Earlier failed preparation and review attempts remain preserved.

This checkpoint does not complete M2. Unmatched-surrogate module/resource URIs,
arbitrary surrogate-containing diagnostic member labels/frame identity, full
string builtins/regex, general compound/Listing rendering and nested Map rehash
effects remain. The existing depth-32/work-65,536 guards and unexecuted sequence
groups remain. Linux, Windows, hosted CI, release APIs, speed parity and peak-RSS
parity were not established by this local checkpoint.

## Stage 28 local verification checkpoint

Implementation `370a668a8534ae42f4fc2a24ae62977df08779c2` is committed and pushed. The [final summary](stage28-final-summary.json),
[independent audit](stage28-post-seal-audit.json), and [completion captures](stage28-completion-captures.json)
close this bounded Unicode escape diagnostic checkpoint. Core parser work used
`gpt-6-astra / xhigh`; Java evidence and native/platform review each used
`gpt-6.1-sol / high`. Verification ran locally on macOS arm64.

Unicode escape scanning now preserves Java's token spans and validation phases.
Ordinary strings defer conversion until the following text token; multiline
strings validate interpolation syntax and delimiter structure before converting
escapes in indentation order. Following-token lookahead preserves source-end and
consumed-range cache visibility. Pinned Java tables contain 401 letter-or-digit
ranges and 41 hexadecimal digit mappings, including non-ASCII decimal digits
and fullwidth hexadecimal letters. Numeric conversion retains signed-32-bit
overflow behavior and arbitrary leading zeros. Production evaluation has no
Java fallback.

The complete selected corpus has **2,925 cases: 2,923 raw matches**,
the existing cycle oracle error, and the existing integer remainder-by-zero
mismatch. All **94 additions (11 successes and 83 expected errors)** and all
88 metamorphic relations pass. The original 2,831 definitions and input bytes,
151 persistent sequence definitions, 86 platform scenarios, native test files,
and reference pins remain unchanged. Persistent evaluation retains
2,622 matching calls. The inventory has 474 requirements and 3,925 accounting
rows (2,925 ready, 960 unimplemented, 40 unmapped); all 16 reviewed mappings remain
partial. The full gate retains 1,950 failure reasons. No development exception was added.

[Seven required local checks](validation-stage28-final.json) passed with
**445 distinct Rust tests and 198 Python tests**. The raw Rust harness reports
458 passes, including 13 isolated child repetitions, plus one intentionally ignored test.
Two new native tests run on 256 KiB stacks with a 60-second parent deadline per
test. They cover failure cleanup, original byte spans, UTF-16 highlights, and
actual malformed multiline leaves nested at depths 24 and 31. Successful work
after parser errors creates fresh evaluators; this does not establish retained
evaluator or Host lifecycle behavior.

Node and Chromium each passed **87 development scenarios**, including 72 calls in
the new scenario. All 6,078 shared primitive results match across targets; each
target matches Java on 6,076, retaining the same two regex failures. Existing
M1 cycle and lifecycle diagnostic gaps remain. Linux, Windows, hosted CI, and
release API behavior were not established by this local checkpoint.

Fresh Java acquisitions and both the focused release artifact and the exact full
language artifact compare **96 exploratory observations: 91 matches and five
mismatches**. The remaining controls are four surrogate representations and one
out-of-range Java internal error. They are retained outside the selected additions;
no replacement-character approximation or exception hides them. The original
three Unicode diagnostic differences now match at their original source URIs.
These exploratory observations overlap selected cases and are not additive coverage.
The depth-32 guard and 65,536-work budget remain, as do arbitrary-depth, synchronous
lexical reparse latency, resource-error, and complete Unicode representation gaps.

The independent audit verified **10 new archives / 53,467,465 bytes**, 7,338 final
source copies representing 3,496 project files against the exact implementation
commit, and 64,377 original acquisition files. The 4 explicitly named direct
Cargo build caches were omitted without enumerating payloads. Stage 27 archives,
summary, audit, and completion records remain immutable direct dependencies.
The completion companion preserves 12 original sealing/audit process files.
Initial native assertion and platform-route failures, rejected acquisition source
drift, intermediate comparisons, and intentional fixture whitespace remain in
the evidence. **M2, full product compatibility, release platform coverage, speed
parity, and peak-RSS parity remain incomplete.**

## Stage 27 local verification checkpoint

Implementation `5383d84c894bc19c90c5b5a3cf9bf54aeb3381d0` is committed and pushed.
The [final summary](stage27-final-summary.json), [independent audit](stage27-post-seal-audit.json)
and [completion captures](stage27-completion-captures.json) close this bounded
lexical diagnostic checkpoint. M2 remains in progress.

String parts now retain exact byte spans, including empty tokens used by Java's
multiline validation. Selected invalid escapes, missing delimiters, block-comment
EOF, interpolation EOF, line continuations and multiline indentation errors match
Pkl 0.32.0 messages and locations. Raw-string escapes follow Java's required hash
prefix and final-hash underline. Explicit UTF-16 widths preserve whitespace and
newline underlines. Earlier interpolation errors retain priority; cached failures
still require the original consumed end and source-end provenance.

Core implementation used `gpt-6-astra / xhigh`. Separate `gpt-6.1-sol / high`
agents handled Java evidence, native/platform tests and independent reviews.
Review scopes exclude each author's own semantic changes. The original 2,724
selected definitions and input bytes, 151 persistent sequences, 85 platform
scenarios, reference pins and existing native test files remain preserved.

The full selected comparison records **2,831 expressions: 2,829 raw matches,
the original cycle oracle error and integer `% 0` diagnostic mismatch**.
All **107 additions** match: five successful inputs and 102 expected errors.
All **88 metamorphic relations** pass. The strict full gate retains **1,950
failure reasons**; no development exception was added. Inventory accounting keeps
474 requirements and 3,831 case rows: 2,831 ready, 960 unimplemented and 40 unmapped.
The 16 reviewed requirement mappings remain partial. These overlapping inventories
do not measure feature completion.

All seven [required local checks](validation-stage27-final.json) pass: **443 distinct
Rust tests and 194 Python tests**, with one existing ignored probe. Raw Rust output
contains 454 harness passes; 11 isolated child reports also appear in their parent
counts. Exact markers, invocation names and numeric summaries identify those
duplicates without rewriting interleaved output. Two new isolated native tests
retain **256 KiB stacks and 60-second deadlines**, checking exact spans, error
order, partial frontend cleanup and fresh successful work after rejection.

The complete persistent suite retains **2,622 matching calls across 151 sequences
run twice**. Actual Node and Chromium each pass **86 development scenarios**.
The new scenario makes 60 calls per target, alternating the 15 historical setup
errors with successful inputs. Failed parsing cannot create a retained evaluator;
these are fresh-evaluator recovery checks on the same binding API. Existing
same-evaluator Host, GC, reentry, retry, cancellation and terminal tests remain
separate. Shared primitive results agree in all 6,078 cases; each target matches
Java in 6,076 and preserves the two known regex failures. M1 frontend, runtime and
lifecycle results retain their previous successes and diagnostic gaps.

Fresh Java replay at the original URIs reproduces all 15 historical diagnostic
failures byte for byte. Focused and final language artifacts each replay **158
exploratory observations: 144 matches, 12 mismatches and two Java process errors**.
The original 15 diagnostic differences now match. Remaining observations include
seven Java-valid deep strings rejected by Rust's guard, the historical Unicode
internal-error case, four new Unicode controls, and two Java failures on deep unary
inputs. The selection overlaps this exploration; counts must not be added as
distinct coverage. The other 47 historical Java observations use retained captures,
not a fresh Java run.

Three new Unicode controls expose ordinary diagnostics for empty, nonhexadecimal
and missing-brace escapes. The fourth retains Java's out-of-range scalar internal
error versus Rust's unsupported-range diagnostic. Surrogate behavior remains
unverified. The lexical nesting limit remains 32, and scope and type depth guards are
unchanged. The expression work budget remains 65,536. Auxiliary lexical validation can synchronously reparse
ranges. Arbitrary nesting, deep type/class/mixed-body compilation, uniform latency,
complete diagnostics, product APIs, release platforms and speed/peak-RSS parity
remain open.

The ten new archives total **38,213,047 bytes**. All **7,131 final source
copies from 3,396 unique project files** match the implementation commit.
The audit verifies **60,233 original acquisition files**, source/Git proof,
raw new and known-failure language controls, platform primitive results, reviews
and six distinct gates. 3 direct Cargo build caches are omitted without
enumerating their payloads; no payload size or hash is claimed. Stage 26 archives,
summary, audit and completion companion remain direct immutable dependencies;
inherited evidence is not recursively rerun or audited. The completion companion
preserves 12 original sealing/audit process files after archive creation.

Preliminary builds and failed development comparisons retain their own provenance.
An early formatting failure has conversation output only; filesystem bytes were
not reconstructed. Intentional spaces, tabs, CRLF and missing terminators in
malformed fixtures preserve Java input bytes. These are local macOS captures;
no hosted CI, Linux or Windows run is claimed. As before, the recorded full-Chrome
identity does not directly establish the launched binary; headless-shell selection
is inferred from pinned Playwright code.

## Stage 26 local verification checkpoint

Implementation `51b132bba82587e3cb013c46f5c452c22fa01ccf` is committed and pushed.
The [final summary](stage26-final-summary.json) and
[independent audit](stage26-post-seal-audit.json) close this bounded checkpoint.
M2 remains in progress.

String scanning uses explicit string/interpolation frames. Interpolation parsing
suspends and restores parser and expression-machine state on the heap, preserving
source spans, deferred integer diagnostics and the shared expression-work budget.
Failed nested string tokens retain their consumed range and original source end.
They can be replayed within a shorter visible prefix when all diagnostic locations
remain visible; truncated-EOF failures are not reused for longer inputs. Earlier
interpolation syntax errors now precede later escape, comment, EOF or multiline
validation errors on the compared paths.

Core implementation used `gpt-6-astra / xhigh`; Java evidence and native/lifecycle
work used separate `gpt-6.1-sol / high` agents. Independent finishing reviews covered
the production parser, tests, Java platform payloads and browser module route.
The original 2,688 selected definitions, 150 sequences, 84 platform scenarios and
existing native test files remain unchanged; new cases are appended.

All seven [required local checks](validation-stage26-final.json) pass with stable source identities: **441 distinct
Rust tests and 194 Python tests**, plus the original ignored probe. The Rust output
contains 450 harness passes, including nine isolated child reports duplicated by
their parents. Exact completion markers and test names distinguish those reports;
one child summary contains an interleaved phase line on the shared output stream.
The original raw output remains unchanged.

The full language comparison records **2,724 independent expressions: 2,722 raw
matches, the original cycle oracle error and the original integer `% 0`
diagnostic mismatch**. All 36 additions and all **88 metamorphic relations** pass.
The strict full gate retains 1,950 failure reasons. Requirement accounting keeps
474 requirements and 3,724 case rows: 2,724 ready, 960 unimplemented and 40 unmapped.
The 16 reviewed requirement mappings remain partial. These inventories overlap
and do not measure feature completion; no development exception was added.

Four isolated native tests pass with unchanged **256 KiB stacks and 60-second
parent deadlines**. Ordinary, raw-hash and multiline strings at depths 24/31 pass
parse, Module clone/drop, resolution, ResolvedModule clone/drop and compilation.
Partial failures release their trees; wide strings and the shared 65,536-step
expression budget have separate checks. A nested authored string also exercises
real Host suspension, reentry, failed-read retry, cancellation and terminal cleanup
with one instruction per poll and GC at every yield and pending read.

The original depth-64 input now returns the existing nesting-limit diagnostic
without a native abort on that stack. **Java accepts this input, so the Rust guard
remains a compatibility mismatch.** The nesting limit remains 32. Six malformed
multiline inputs at depths 24/31 originally exposed a second failure in cache reuse
across shorter interpolation ranges. Their original failed captures are retained;
the corrected implementation returns Syntax for the unchanged inputs through both
expression and module entry points. Eight original malformed expression inputs
also have a separate successful small-stack replay. Separate Java comparisons
retain diagnostic byte mismatches; native replay establishes safe rejection only.

Focused frozen artifacts match **36 new selected cases: 23 successes and 13
expected errors**, plus **30 persistent calls**. Broader exploration retains
**37 raw matches, 23 mismatches and two oracle errors among 62 observations**,
overlapping the selection. These include valid deep strings rejected by the Rust
guard, remaining invalid-escape/unclosed/multiline diagnostics, a Java Unicode bug
and two Java process failures on deep unary inputs. Six multiline Java inputs
explicitly wrap the preserved expression bytes in a module; this changes their
source offsets and is not an expression-API byte comparison.

The complete persistent suite matches **2,622 calls across 151 sequences run
twice**, retaining the previous 150 definitions and their execution conditions.
Actual Node and Chromium each pass **85 development scenarios**, including the new
15-call sequence twice per target. The browser runner now serves its new imported
fixture module. All 6,078 primitive results agree across the shared engines; each
target matches Java in 6,076 cases and retains the same two regex failures. The
first browser acquisition failed because that module route was missing; its raw
failure remains separate from the successful rerun. M1 frontend, runtime and
lifecycle captures retain their previous successes and diagnostic differences.
These are local macOS captures; no new hosted CI, Linux or Windows execution is
claimed. Browser execution does not directly prove that the recorded full-Chrome
executable was launched; headless-shell selection is inferred from pinned
Playwright code, as in the preceding checkpoint.

Failed native drafts, a helper compile failure, an invalid acquisition command and
a source-drift build remain recorded. No failed or incomplete capture counts as a
pass. The first sealing attempt rejected unchanged historical archives because
the helper compared identity fields with full metadata rows. That failed process
and its exact helper snapshot are retained; the corrected identity comparison has
an independent review. Normal interpolation parsing shares its work allowance;
auxiliary lexical
error validation can reparse ranges and does not establish a uniform work or
latency bound. Arbitrary nesting, full type/class/mixed-body compilation, complete
Java diagnostics, product APIs, release platforms and speed/peak-RSS parity remain
outside this checkpoint.

The [sealed summary](stage26-final-summary.json) records **10 new archives totaling
58,909,100 bytes**, covering eight final suites, support captures and source proof.
All **6,911 final source copies from 3,287 unique project files** match the exact
implementation commit. Support evidence retains 15,423 original files. Five
explicit direct Cargo build caches are omitted without enumerating their payloads;
no payload count, size or hash is claimed. The 15 Stage 25 archives remain direct
size/hash dependencies; inherited evidence is not recursively rerun or re-audited.
Earliest exploratory Java runners lack saved outer Python stdout captures; their
inner raw process files and source/runtime identities remain retained.

The independent audit verifies **67,236 original acquisition files**, all final
source snapshots, review/helper identities, six gate results, public test counts,
the 36 new and two known-failure raw language controls, and all platform primitive
comparisons. Its first attempt incorrectly expected complete parent test lines in
serial native output; a reviewed supplementary auditor checks each original
parent/child block without changing or normalizing the bytes. The original auditor,
failed process and all ten sealed archives remain unchanged. The
[completion companion](stage26-completion-captures.json) preserves 26 original
process and supplementary files, including the failed first audit, successful
corrected audit, exact helper sources, preparation diff and independent reviews.
No retained evaluator or historical helper was executed by the audit.

## Stage 25 local verification checkpoint

Implementation `fc45bea329c4fdbc536eaef5070a0c03ed642d5a` is committed and pushed.
The [final summary](stage25-final-summary.json) and
[independent final audit](stage25-post-seal-audit.json) close this bounded checkpoint.

The parser, resolver and compiler use explicit work stacks for ordinary expressions
and selected if/let/lambda/property-only object paths. Expression cloning and
release, constant List-key processing and power-assertion anchors avoid recursion
along the selected paths. Type parsing has a work stack and a composed height
limit; the 65,536-step expression budget is shared across interpolated expressions.
This removes the former ordinary-expression depth-32/node-512 restrictions without
claiming arbitrary grammar depth. Scope and type limits remain 32.

Core frontend implementation used `gpt-6-astra / xhigh`; Java evidence and
resolver/native/platform work used separate `gpt-6.1-sol / high` agents. Root
implemented type parsing and integration. Independent reviews explicitly exclude
the reviewer's own changes. An omitted `Uninitialized` resolver leaf found during
review was fixed and covered by existing tests plus a native default-property
control.

The full language comparison records **2,688 independent expressions: 2,686 raw
matches, the original cycle oracle error, and the original integer `% 0`
diagnostic mismatch**. All 36 additions and all **88 metamorphic relations** pass.
The strict full gate still has 1,950 failure reasons. Requirement accounting keeps
474 requirements and 3,688 case rows: 2,688 ready, 960 unimplemented and 40 unmapped.
These overlapping inventories do not measure feature completion.

All seven [required local checks](validation-stage25-final.json) pass:
**437 distinct Rust tests and 194 Python tests**, with the original ignored probe
reported separately. Rust output contains
442 harness passes because each of the five isolated native child tests is also
reported by its parent; those five duplicate reports are explicitly excluded from
the distinct test count.

The five native tests use unchanged **256 KiB stacks and 60-second parent
deadlines**. They exercise authored ordinary depth 768, 1,024 independent properties,
accepted if/let/lambda/property-only Dynamic depth 24/31, 64 amendment parents,
partial-tree cleanup, work/scope/type guards, instrumented compilation and constant
List-key diagnostics. A compiled authored program also exercises real Host
suspension, reentry, failed-read retry, cancellation and terminal release with GC
at every yield and pending read. Type depth 24/31 checks cover parse/Module
clone/drop only.

The complete persistent suite matches **2,592 calls across 150 sequences run
twice**. Actual Node and Chromium each pass **84 development scenarios**; the two
new groups compare 60 calls per target against complete Java payloads. All 6,078
shared primitive results agree; each target matches Java in 6,076 cases and retains
the same two regex failures. These development scenarios do not establish complete
bindings. Native Host/GC and stack tests have separate contracts. Existing M1
frontend/runtime/lifecycle successes and cycle/complete-diagnostic differences are
preserved. No new hosted CI, Linux or Windows execution is claimed. The logged
full-Chrome executable identity is distinct from the headless-shell selection
inferred from pinned Playwright code; observed browser execution does not directly
prove that recorded executable was the launched binary.

Focused final artifacts match all **36 new selected expressions** (30 successes,
6 expected errors), including the original depth-64 Set equality and Map-key inputs,
and all 60 new persistent calls. Broader exploration preserves **56 matches and
12 mismatches among 68 observations**, overlapping the 36 selected cases. Guarded
scope depth, an invalid direct-lambda input, existing type diagnostics, object-body
syntax and generator duplicate/type-error ordering retain their original outcomes.
These observations add no development exception.

The original 2,652 selected input definitions and bytes, 88 metamorphic relations,
147 sequence definitions and 82 platform scenarios remain preserved. All 22 prior
lifecycle test files are byte-identical. Two other existing test files required
nine mechanical borrow adaptations for explicit Expr destruction. Two obsolete
ordinary-depth rejection assertions intentionally became success checks on the
same 129-parenthesis/600-member inputs, verifying literal value, full spans and
every member in the chain. Their original drafts, compiler failure and correction
are preserved separately; this is not a claim that all old tests are unchanged.
The new selection conservatively derives a partial `semantic:shared-engine` family
record, with empty implementation links and no new reviewed mapping or completed
requirement. The 16 reviewed mappings remain partial; two resolver source-symbol
links now point to their continuation implementation.

**Nested lexical strings at depth 64 still overflow the same 256 KiB stack.**
The original SIGABRT and the explicit checkpoint exclusion remain recorded; no
larger-stack success replaces that failure. Drafted accepted nested-string and
full deep-type compilation checks were not run and are not passes. Full type
compilation, class/instance and mixed/generator body compiler helpers, generator
and Type AST recursion, and derived AST Debug/PartialEq retain separate limits.
Earlier native failures, failed test builds, and source-drift acquisitions stopped
as inconclusive remain preserved. Source-bound final captures are distinguished
from those earlier attempts. This checkpoint does not establish complete language
compatibility, exact Java resource errors, uniform latency or workload speed and
peak-RSS parity.

The final summary records **15 archives totaling 73,849,365 bytes**. The
[source and gate evidence](source-stage25-final-evidence.tar.gz) binds 6,813 source
snapshot copies and 3,245 unique project files to the exact implementation commit.
Six exploration archives retain 32,289 original paths; their inventories contain
32,313 files including partition metadata. Two explicitly named Cargo build-cache
directories were omitted without traversing their contents; their payload counts,
sizes and hashes are unknown. No enumerated ordinary file was excluded. Eight
authoritative suite directories are separately archived. Historical evidence stays
an exact-hash dependency. Completed exploration configuration/sealing outer captures
remain in `target/m2`; the configuration, sealer, index and independent audit are
retained in the sealed evidence.

The independent final audit verifies archive inventories, exact Git source bytes,
review and helper provenance, raw gate results, the 36 new and two known-failure
controls, test accounting and historical identities. It executes no retained
evaluator or helper. The [completion companion](stage25-completion-captures.json)
preserves 13 exact files from the completed packaging and audit processes, including
their runners, invocations, raw outputs, helpers and the audit preparation manifest.
Those completion bytes are separate from the sealed archives.

**Stage 25's bounded authored frontend checkpoint is closed. M2, complete product
APIs, release platforms and speed/peak-RSS targets remain incomplete.**

## Stage 24 local verification checkpoint

Implementation `123f41660286787bc46418d6a81d984f48ba22ef` is committed and pushed.
Runtime implementation used `gpt-6-astra / xhigh`; Java comparisons and independent
lifecycle/platform review used `gpt-6.1-sol / high`. Root reviewed the implementation
and tests and coordinated local macOS verification. Runtime self-review, independent
source review, raw comparisons and archive audits have separate scopes.

`Collection<T>` is an abstract type accepting List and Set values. List elements can
receive nested lazy wrappers; Set elements remain eagerly checked without replacing
their keys. Bare types, aliases, empty List defaults, nullable attached defaults,
Collection-selected union defaults, casts, constraints and Reference metadata have
selected Java comparisons. Empty explicit generic argument lists now report the pinned
parser error, with List, Set, Map and alias controls. Existing parser depth and node
limits are unchanged.

The [final summary](stage24-final-summary.json) records **2,652 independent expressions:
2,650 raw matches, the original cycle oracle error, and the original integer `% 0`
diagnostic mismatch**.
All 304 additions match: 196 successes and 108 expected errors. All **88 metamorphic
relations** pass. The [persistent suite](class-lifecycle-stage24-final-evidence.tar.gz)
executes **147 sequences twice, with 2,532 matching calls**. The previous 2,348
independent inputs, 82 relations, 137 sequences, 21 lifecycle test files and 80 platform
scenarios are preserved. Root also verified
all 51 previous native test files and the existing internal metadata test bodies are
byte-identical. No development exception was added.

All seven [required local checks](validation-stage24-final.json) pass: **432 Rust tests
and 194 Python tests**, with the original ignored Rust probe reported separately.
Seven [native Collection lifecycle tests](native-collection-type-stage24-final-evidence.tar.gz)
cover lazy/eager timing, suspended constraint context, retained temporary receivers,
Host reentry and failed-read retry, 24 nonterminal cancellations, evaluator isolation,
stale handles and terminal cleanup.
They use one instruction per poll, GC at every yield and pending read, and at most
200,000 polls per event. A separate 20,000-level metadata test exercises traversal,
normalization, subtype checks, exported names and release on a 256 KiB stack. It does
not exercise deeply authored syntax or establish Java speed or peak-RSS parity.
Historical Map, Set and generic small-stack conditions remain unchanged.

[Actual Node and Chromium](platforms-stage24-final-evidence.tar.gz) each pass
**82 development scenarios**. The two new groups compare 206 ordered calls per target
against complete Java payloads. All 6,078 shared primitive results agree across engines;
each target matches Java in 6,076 cases and
retains the same two regex failures. The architecture contract passes while strict
primitive compatibility fails. M1 frontend, runtime and lifecycle comparisons retain
their earlier successes and known cycle/complete-diagnostic differences. No hosted CI,
Linux or Windows execution is claimed. The logged full-Chrome path is distinct from
the headless-shell selection inferred from pinned Playwright code; its hash does not
prove which browser executable was launched. Browser version and scenario outcomes
were observed during actual execution.

Focused final artifacts match all 304 selected programs at their original exploratory
Java URIs and all 103 new persistent calls. Broader exploration retains **337 matches
and 26 mismatches among 363 observations**, overlapping all 304 selected programs and
including 59 unselected controls. Original unsupported API/compound-output inputs,
invalid syntax and declarations, failed hypotheses and corrected-source observations
retain their own bytes. Two String-selected alias-union constructors still differ in
their complete diagnostics. Same/different-function wrapper chains establish selected
values and errors, not wrapper identity or successful predicate execution counts.

Early exploratory builds retain their artifact, input and outcome identities but lack
complete recorded build/project-source closure. No source identity is inferred from
timestamps or the later checkout. The final recorded builds and original-URI replays
have separate exact source binding. Preliminary receiver/alias assertions and metadata
test compilation failures remain preserved; the passing final captures do not replace
them.

The final evidence consists of 16 archives totaling 87,514,114 bytes. The
[source and gate archive](source-stage24-final-evidence.tar.gz) binds 6,707 source
snapshot copies across 3,199 unique project paths to the exact implementation commit.
The seven exploration partitions preserve 19,929 original paths without exclusions;
their inventories contain 19,957 entries including per-partition metadata. The
[independent postseal audit](stage24-post-seal-audit.json) verifies archive inventories,
Git source binding, reviews, the actual packaging helper and process, recorded gates,
and historical artifact identities. It passes without executing retained evaluators.
The [completion capture companion](stage24-completion-captures.json) preserves exact
runner, invocation, process, output and helper bytes from the completed packaging and
audit processes. These later records are separate from the sixteen sealed archives.

The inventory retains 474 requirements, 16 partial mappings and 3,652 accounting rows:
2,652 ready, 960 unimplemented and 40 unmapped. These overlapping rows are not a
completion percentage. The strict full gate retains 1,950 failure reasons. Authored
frontend stack safety, remaining alias-constructor diagnostics and type/Reference/module
rules, full collection APIs, compound rendering, original upstream suites, product APIs,
release platforms and workload-level speed/peak-RSS requirements remain open.
**The bounded Stage 24 Collection contract is closed. M2 as a whole remains in progress.**

## Stage 23 local verification checkpoint

Implementation `84d7044e7b9b83350b895a3b91a92663aedb73df` is committed and pushed.
Runtime implementation used `gpt-6-astra / xhigh`; Java comparisons and independent
lifecycle/platform review used `gpt-6.1-sol / high`. Root added the shared Set
stringification boundary, reviewed the runtime and native tests, and coordinated local
macOS verification. Self-review, independent source review, raw comparison, and
packaging audits retain their separate scopes.

Strict Set construction now evaluates each element, checks membership, and hashes new
elements during insertion. Empty membership skips hashing; a nonempty lookup and
insertion prepare their hashes separately. Duplicate elements preserve the first
insertion position. The shared VM supports selected `contains`, `length`, `isEmpty`,
defaults, eager element type checks, equality, hashing, iteration, spread, and multiline
strings. Internal table keys and insertion-order elements have separate representations
and GC roots. Strings read completed member caches; direct Map strings and diagnostic
snapshots retain their single-line layout. Nested Map key-rehash side effects during
Java rendering remain a known boundary.

The [final summary](stage23-final-summary.json) records **2,348 independent expressions:
2,346 raw matches, the original cycle oracle error, and the original integer `% 0`
diagnostic mismatch**. All 217 additions match: 158 successes and 59 expected errors.
All 82 metamorphic relations pass.

The [persistent suite](class-lifecycle-stage23-final-evidence.tar.gz) runs 137 sequences
twice, with **2,326 matching calls**. The previous 2,131 independent inputs, 76
relations, 129 sequences, 20 lifecycle test files and 78 platform scenarios remain
unchanged. No development exception was added.

All seven [required local checks](validation-stage23-final.json) pass: **424 Rust tests
and 194 Python tests**, with the original ignored Rust probe excluded. Eight [native
lifecycle tests](native-set-stage23-final-evidence.tar.gz) cover pending builders,
membership queries and comparison operands, nested Host failure and retry, 24
cancellations, independent evaluators, stale handles, retained declaration and cache
plateaus, and terminal release. Seven cases collect at every cooperative yield, with one
instruction per poll and at most 200,000 polls per event. The separate deep test uses
depth 768, a 256 KiB evaluator stack, a 60-second parent deadline, one instruction per
poll, GC every 128 polls, and a limit of 4,000,000 polls per event. It records 901,299
polls and 7,042 periodic/pending collections plus one final rooted collection.
Compilation uses an ordinary stack.

The original deep every-yield experiment timed out, and a later periodic-GC run hit its
lower limit of 200,000 polls per event. Both remain failed evidence. The passing test
has explicitly different conditions; it does not establish the earlier resource contract
or Java performance parity. Initial alias assertions also failed: one source had a parse
error, another reproduced Java's alias receiver scope, and zero-live assertions
overlooked intentionally retained declaration/module caches. The revised test separates
ordinary and executable entries, verifies repeated-call plateaus, and requires zero
entries after terminal cleanup. The first preliminary acquisition also had source drift;
its before/after sources are preserved, but it has no single exact source/binary
binding. Historical Map and generic small-stack tests remain byte-identical and pass
unchanged.

[Actual Node and Chromium](platforms-stage23-final-evidence.tar.gz) each pass all 80
development scenarios. The two new Set groups compare 160 ordered calls per target
against complete Java payloads. All 6,078 shared primitive results agree across engines;
each target matches Java in 6,076 cases and retains the same two regex failures. The
architecture contract passes while strict primitive compatibility fails. M1 frontend,
runtime and lifecycle comparisons retain their prior successes and known
cycle/complete-diagnostic differences. No new hosted CI, Linux or Windows execution is
claimed. The logged browser executable path identifies full Chrome; selection of
headless-shell is inferred from pinned Playwright launch code, so that path hash is not
proof of the launched browser binary. Browser version and scenario outcomes were
observed during actual execution.

The [exploration archive](exploration-stage23-evidence.tar.gz), [interim
comparisons](exploration-stage23-superseded-evidence.tar.gz), and [native
preparations](exploration-stage23-native-preliminary-evidence.tar.gz) retain original
inputs, failed hypotheses, original conditions, helpers, and final recorded-artifact
replays. The final artifacts match all 217 selected original exploratory URIs and 80
focused persistent calls. Broader exploration retains 224 matches and **22 mismatches
among 246 inputs**, overlapping the 217 selected cases; three additional alias/Host
observations retain one match and two mismatches. Six original compound-return proposals
and the original iteration sequence remain unselected observations. Fresh sources return
scalar observations of indexed members and lengths to preserve order, deduplication and
forced-member checks without claiming general compound output support. No selected
baseline failure or temporary exception was removed.

The [source and gate evidence](source-stage23-final-evidence.tar.gz) binds 6,053
captured project-source copies across 2,873 unique paths to the exact implementation
commit. Reference pins and the oracle retain separate provenance; no upstream
source-copy binding is claimed. The selected development gate passes; strict selected
and full gates fail as expected, retaining 1,950 full-gate failure reasons. Twelve final
archives total 108,225,352 bytes. The exploration audit checks 56,234 original paths and
56,246 inventory entries without cache exclusions.

The [independent postseal audit](stage23-post-seal-audit.json) verifies all twelve
archive inventories, exact source/Git objects, reviews, gates, approved packaging helper
and execution provenance. A [capture preservation
check](stage23-sealing-capture-audit.json) verifies the original, copied and archived
sealing-process bytes. Historical artifacts retain their original identities; no
captured evaluator binary is executed during these audits.

The inventory retains 474 requirements, 16 partial mappings, and 3,348 accounting rows:
2,348 ready, 960 unimplemented and 40 unmapped. These overlapping rows are not a
completion percentage. Full Set/Map APIs, including higher-order operations, JVM
identity hash ordering, general compound output, remaining type/Reference/module rules,
original upstream suites, product APIs, release platforms and workload-level
speed/peak-RSS requirements remain open. **Only the bounded Stage 23 development
contract is closed; M2 as a whole remains in progress.**

## Stage 22 local verification checkpoint

Implementation `bb30657aecc0be64cdbf1510e949053794b1ca8d` is committed and pushed.
Runtime implementation used `gpt-6-astra / xhigh`; Java comparisons and independent
lifecycle/platform review used `gpt-6.1-sol / high`. Root implemented Map diagnostic
rendering and the expression-string adapter, independently reviewed the remaining
runtime changes, and coordinated local macOS verification. Author, independent
source, comparison, and packaging reviews retain their separate scopes.

Strict Map construction evaluates each key and value before hashing and inserting
that pair. Duplicate keys preserve their first position and replace their value.
The shared VM now supports selected lookup, typing, defaults, equality, hashing,
iteration, and non-forcing stringification. Its hash trie follows Java's pinned
Paguro implementation, while insertion order remains separate from internal table
order. This separation also preserves heap values omitted from the insertion-order
list when structurally equal keys acquire different cache-order-dependent hashes.
Builder, lookup, and comparison continuations remain rooted across Host
suspension, re-entry, failure, and retry.

The [final summary](stage22-final-summary.json) records **2,131 independent
expressions: 2,129 raw matches and the original cycle
oracle error and integer `% 0` diagnostic mismatch**. All 191 additions match:
108 successes and 83 expected errors. All 76 metamorphic relations pass. The
[persistent suite](class-lifecycle-stage22-final-evidence.tar.gz) runs 129 sequences
twice, with **2,166 matching calls**. The previous 1,940
definitions, 72 relations, 122 sequences, 2,026 calls, 19 lifecycle test files,
and 76 platform scenarios are preserved. No development exception was added.

All seven [required local checks](validation-stage22-final.json) pass:
**415 Rust tests and 194 Python tests**, formatting, Clippy, Wasm compilation,
reference pins, and manifest freshness. One existing ignored Rust probe remains
excluded. Nine [native lifecycle tests](native-map-stage22-final-evidence.tar.gz)
cover unpublished builders, pending queries and comparisons, Host failure/retry,
24 nonterminal cancellations, evaluator isolation, stale handles, terminal release,
hash-trie expansion, and retention of values present only in the internal table.
Eight tests use ordinary test-thread stacks. The process-isolated test hashes a
768-deep Map on a 256 KiB evaluator thread with a 60-second parent deadline;
compilation uses the ordinary stack. These establish bounded lifetime and stack
behavior, not Java-equivalent resource limits or peak-RSS parity.

[Actual Node and Chromium](platforms-stage22-final-evidence.tar.gz) each pass
78 development scenarios. All 6,078 shared primitive results agree across engines;
each target matches Java in 6,076 cases, preserving the two regex failures.
The architecture contract passes and strict primitive compatibility fails.
M1 frontend, runtime, and lifecycle comparisons retain their earlier successful
results and known cycle/complete-diagnostic differences. No new hosted CI,
Linux, or Windows execution is claimed.

Preparation exposed a small-stack Map regression; the first full acquisition then
failed the unchanged generic-metadata small-stack test and the existing Node
scenario with 23 List keys in a Mapping. Optional boxed Map metadata and a smaller
VM scheduler remove the extra stack frames. Completed hash preparations now
accumulate within one bytecode instruction, preventing repeated preparation of
different key sets. The original stacks, depths, deadlines, and instruction
budgets remain unchanged. All final results use the corrected `final-02` sources.

The [exploration archive](exploration-stage22-evidence.tar.gz),
[interim candidates](exploration-stage22-superseded-evidence.tar.gz), and
[superseded first acquisition](exploration-stage22-superseded-final01-evidence.tar.gz) retain
original inputs, failed hypotheses, superseded acquisition, helpers, and final
artifact replays. The final artifacts match the 191 selected original exploratory
URIs and 70 focused persistent calls. Broader 201-case exploration retains 186 raw
matches and **15 mismatches**, covering missing/builtin diagnostics and remaining
compound output and API behavior. Twelve separate late observations retain nine
matches and three Java internal-error outcomes; five matches also belong to the
selected set. These observations overlap selected coverage and are not additional
coverage denominators or new development exceptions.

Failed preparations include an initial compiler error without a separate raw
capture and small-stack runs without retained per-run binary/source closures.
Stopping the first full acquisition also interrupted its outer capture wrappers:
the language and persistent runs have no final outer process record. Their raw
child output, stop ledger, and tool-returned status are retained; truncated tool
output is identified explicitly. None of these superseded or incomplete captures
counts as final passing evidence. The initial two-archive packaging attempt
exceeded its unchanged 80 MB limit. Its receipt and exact helper/configuration
snapshots are published; the failed derived archive bytes remain local under
`target/m2/stage22-failed-seal-artifacts-01`. All original raw captures are retained
in the three corrected archives. The exploration audit verifies
39,437 original paths and all 39,449 inventory records, with no cache exclusions,
without executing retained artifacts.

The [source and gate audit](source-stage22-final-evidence.tar.gz) binds
**5,557 captured source copies across 2,629 unique
project paths** to the exact implementation commit. No upstream source copies
are claimed; reference pins and oracle provenance are verified separately.
All 474 requirements and 16 partial mappings remain. The 3,131 overlapping
accounting rows contain 2,131 ready, 960 unimplemented, and 40 unmapped entries.
Integrity and development gates pass; strict selected and full gates fail with
the same 1,950 full-gate failure reasons. These counts are not completion
percentages. The [post-seal audit](stage22-post-seal-audit.json) checks all
12 final archives, source bindings, review records, gate
outcomes, immutable packaging execution, and historical identities without
running retained artifacts.

The first final packaging attempt rejected the four new metamorphic relations
because they were sorted between existing records. Its prefix-only check was
incorrect: all 72 prior complete records and their relative order were unchanged.
The corrected check rejects duplicate identities and compares the complete prior
records in their original relative order. The revised helper received a separate
independent review; the failed immutable helper, invocation, raw output, and
process record remain in the source archive's `helper-history` directory.

**Stage 22 closes only the bounded strict-Map development contract.**
Set, remaining Map APIs, JVM identity-dependent key order, mutable zero-hash key
boundaries, general compound stringification, remaining diagnostics, annotation
and module behavior, Java stack-error bytes, complete language/API/platform
coverage, and execution-time/peak-RSS parity remain open. The small-stack hashing
test does not establish resource bounds for synchronous compound diagnostic
rendering.

## Stage 21 local verification checkpoint

Implementation `6530fc7066779781485e1e74dae47fff9a5ac819` is committed and pushed.
Runtime implementation used `gpt-6-astra / high`; Java comparisons and independent
lifecycle/platform review used `gpt-6.1-sol / high`. Root reviewed the production
changes and coordinated local macOS verification. Production author review,
independent source review, comparison review, and packaging review retain their
separate scopes.

Direct generic constructors now obtain their parent from the actual type's
default through the existing resumable VM scheduler and amendment path. Bare
`new T` selects a parent without adding a collection or constraint cast;
explicit `new Alias<T>` additionally checks its result. Nullable defaults,
class member checks, lazy failures, and captured functions retain their separate
semantics. Argument source locations and authored alias spelling travel outside
type identity, including forwarding and recursive uses. Inline/lambda nodes
receive actual arguments while lazy member call targets retain Java's unbound
interpretation; a remaining logical frame-name difference stays recorded.

The [final summary](stage21-final-summary.json) records **1,940 independent
expressions: 1,938 raw matches and the original cycle oracle error and integer
`% 0` diagnostic mismatch**. All 80 additions match: 39 successes and 41 expected
errors. All 72 metamorphic relations pass. The
[persistent suite](class-lifecycle-stage21-final-evidence.tar.gz) runs 122
sequences twice, with **2,026 matching calls**. The previous 1,860 definitions,
69 relations, 118 sequences, 1,936 calls, and 18 lifecycle test files are
preserved. No new development exception was added.

All seven [required local checks](validation-stage21-final.json) pass:
**406 Rust tests and 194 Python tests**, formatting, Clippy, Wasm compilation,
reference pins, and manifest freshness. One existing ignored Rust probe remains
excluded. Six [native lifecycle tests](native-constructors-stage21-final-evidence.tar.gz)
cover Host child failure/retry, 24 nonterminal cancellations and retained-object
counts, evaluator isolation, captured functions, and terminal release. Five use
ordinary test-thread stacks; the sixth constructs 768-deep defaults on a 256 KiB
evaluator thread in a child process with a 60-second parent deadline. Compilation
uses the ordinary stack. These checks establish bounded lifetime behavior, not
Java-equivalent resource limits or peak-RSS parity.

[Actual Node and Chromium](platforms-stage21-final-evidence.tar.gz) each pass
76 development scenarios. All 6,078 shared primitive results agree across
engines; each target matches Java in 6,076 cases, preserving the two regex
failures. The architecture contract passes and strict primitive compatibility
fails. M1 frontend, runtime, and lifecycle comparisons retain their earlier
successful results and known cycle/complete-diagnostic differences. No new
hosted CI, Linux, or Windows execution is claimed.

The [exploration archive](exploration-stage21-evidence.tar.gz) retains original
inputs, failed hypotheses, superseded captures, helper sources, and final
artifact replays. The final artifact matches the 80 new fixture URIs, four exact
historical Stage 20 inputs at their original URIs, three recursive controls, and
45 focused persistent calls. Broader 85-case exploration retains 71 raw matches
and **14 mismatches**, including missing-property and builtin diagnostics,
generator boundaries, a Listing-key message, and the lazy member frame name.
These observations overlap the selected cases and are not additional coverage.
A preparation build interrupted by concurrent edits and notes about uncaptured
initial test failures remain separate from final passing evidence. An
independent read-only audit verifies 5,806 original paths and all 5,810 archive
inventory records, with no cache exclusions. Historical Stage 20 evidence is
linked by hash rather than copied.

The [source and gate audit](source-stage21-final-evidence.tar.gz) binds
**5,133 captured source copies across 2,421 unique project paths**
to the exact implementation commit. No upstream source copies are claimed;
reference pins and oracle provenance are verified separately. All 474
requirements and 16 partial mappings remain. The 2,940 overlapping accounting
rows contain 1,940 ready, 960 unimplemented, and 40 unmapped entries. Integrity
and development gates pass; strict selected and full gates fail with the same
1,950 full-gate failure reasons. These counts are not completion percentages.
The [post-seal audit](stage21-post-seal-audit.json) checks all ten final archives,
source bindings, review records, gate outcomes, immutable packaging helper, and
historical identities without running retained artifacts.

**Stage 21 closes only the bounded generic-constructor development contract.**
Remaining constructor and Reference diagnostics, strict Map/Set semantics,
annotation and module behavior, Java stack-error bytes, synchronous metadata
and parent-unwrapping latency, complete language/API/platform coverage, and
execution-time/peak-RSS parity remain open.

## Stage 20 local verification checkpoint

Implementation `c4e2a41e8391fe5ef031fc6fd5880cf46112c66e` is committed and pushed.
Runtime implementation used `gpt-6-astra / high`; Java comparisons and independent
lifecycle/platform review used `gpt-6.1-sol / high`. The root independently
reviewed the production changes and coordinated local macOS verification.
Author self-review, independent source review, and evidence review retain their
separate scopes.

Defaults of nullable types now preserve their inner defaults in the evaluator-owned tracing
heap. Empty amendments expose one attached default; amendments with members
unwrap nested defaults before selecting their parent. Null equality, hashing,
type checks, rendering, and coalescing remain independent of the attachment.
Null-propagating member and method calls discard it. Static and generic default
construction retain class and collection defaults, including the absence of an
abstract class default. Generic construction uses the existing scheduler and GC
roots.

Independent review found that VM inputs and Host completions incorrectly treated
the new internal null slot as an ordinary object. Typed value validation now
accepts a live slot from the same evaluator and rejects foreign or stale slots.
The first full workspace run then exposed a stack regression missed in initial
review: empty amendments recursively entered the large collection dispatcher.
The corrected implementation uses nonrecursive dispatch and a small finalization
helper. All four unchanged Stage 19 metadata tests pass with their original
256 KiB stack and workload limits. The failing SIGABRT, interrupted acquisitions,
completed superseded runs, and initial review records remain in the
[superseded evidence](exploration-stage20-superseded-evidence.tar.gz).

The [final summary](stage20-final-summary.json) records **1,860 independent
expressions: 1,858 raw matches and the original cycle oracle error and
integer `% 0` diagnostic mismatch**. All 39 additions match, comprising 34
successes and five expected errors. All 69 metamorphic relations pass.
The [persistent suite](class-lifecycle-stage20-final-evidence.tar.gz) runs 118
sequences twice, with **1,936 matching calls**. Existing 1,821 case definitions,
66 relations, 115 sequences, 1,870 calls, and 17 lifecycle test files remain
preserved. No new development exception was added.

The [required local checks](validation-stage20-final.json) pass all seven
commands: **400 Rust tests and 194 Python tests**, formatting, Clippy, Wasm
compilation, reference pins, and manifest freshness. One existing ignored Rust
probe remains excluded. Two new heap tests cover null-slot isolation and
20,000-level tracing/release on a 256 KiB thread. Six new
[native lifecycle tests](native-defaults-stage20-final-evidence.tar.gz) cover
nested defaults, captured closures, Host child failure/retry, 24 cancellations,
separate evaluators, stale/foreign value boundaries, and all three terminal
transitions. These six use ordinary test-thread stacks and do not inherit the
small-stack claim from the separate stress tests.

[Actual Node and Chromium](platforms-stage20-final-evidence.tar.gz) each pass
74 development scenarios. Shared primitive results match across 6,078 cases;
each target matches Java in 6,076 cases, retaining the two known regex failures.
The architecture contract passes while strict primitive compatibility fails.
M1 frontend, runtime, and lifecycle regressions retain their previous successful
comparisons and known cycle/complete-diagnostic differences. No new hosted CI,
Linux, or Windows result is claimed.

The [exploration archive](exploration-stage20-evidence.tar.gz) retains fresh Java
captures for 62 exploratory attempts and 39 final fixture-URI cases; these
include repeated hypotheses and are not an additional coverage denominator.
Corrected final artifacts also match six exact historical nullable inputs at
their original URIs and 33 new ordered calls. Invalid syntax attempts, unsupported
Map construction, direct `new T`, and an abstract-default missing-property
diagnostic remain recorded separately. One supplemental lifecycle-tool unittest
command timed out; the final complete 194-test Python run passes independently.
The two exploration archives preserve 16,744 original paths with 178 explicit
disposable Cargo-cache exclusions. Their independent audit verifies all 16,752
inventory payload records and the historical reference identities without
executing retained artifacts.

The [source and gate audit](source-stage20-final-evidence.tar.gz) binds
**4,951 captured project-source copies across 2,331 unique
paths** to the exact implementation commit. Upstream source copies are absent;
reference pins and oracle provenance are verified separately. All 474
requirements and 16 partial mappings remain. The 2,860 overlapping accounting
rows contain 1,860 ready, 960 unimplemented, and 40 unmapped entries. Integrity
and development gates pass; strict selected and full gates fail, retaining
1,950 full-gate failure reasons. These counts are not completion
percentages.

The [post-seal audit](stage20-post-seal-audit.json) independently verifies the
11 final archives, source bindings, review records, six gate outcomes, immutable
packaging helper, and historical reference identities. It executes no retained
artifacts.

**Stage 20 closes only the bounded nullable-default development contract.**
Direct generic constructors, remaining shared-let/Reference/default diagnostics,
Java stack-error bytes, synchronous metadata and parent-unwrapping cancellation
latency, complete language/API/platform coverage, and execution-time/peak-RSS
parity remain open.

## Stage 19 local verification checkpoint

Implementation `a0801b9b4d614f6d936d9c410f67a05be59e7733` is committed and pushed.
Core implementation and independent Reference review used `gpt-6-astra / high`;
Java comparisons, lifecycle tests, actual platform execution, and evidence audits
used `gpt-6.1-sol / high`. The root agent implemented Reference traversal and
reviewed the core ownership/default changes. Verification ran locally on macOS;
M2 remains in progress.

Runtime generic arguments now form an immutable shared graph. Substitution
traverses the finite authored template and reuses instantiated arguments.
Environment clones copy handles, and last-owner destruction drains metadata
iteratively. Concurrent releases use `Arc::into_inner` to preserve reclamation.
Nested collection defaults run as VM tasks, consuming fuel and tracing unfinished
objects during descent and unwind. Reference normalization memoizes graph nodes;
comparison and formatting use explicit work stacks. Exported metadata identity
preserves alias declarations, arrow syntax, and exact constraint text without
recursively escaping or expanding shared subgraphs.

Independent review found a lost equal-class-ID shortcut in Reference subtype
comparison; it was restored before final acquisition. The earlier candidate and
its successful comparisons remain separate historical evidence. The prior
constraint-equivalence rule was preserved because no Java witness established a
correction for the suspected wrapper argument-identity issue.

The [final summary](stage19-final-summary.json) records **1,821 independent
cases, 1,819 raw matches, and the same two failures**, with **66 passing
metamorphic relations**. All 23 additions match Java at their final fixture URIs:
19 successful values and four expected errors. Original 1,798 definitions, input
bytes, and 63 relations remain unchanged. The separate prior-selection replay
retains 1,796 raw matches and the original cycle oracle error and remainder-zero
diagnostic mismatch. No development exception was added.

The [persistent contract](class-lifecycle-stage19-final-evidence.tar.gz) passes
**1,870 calls across 230 executions of 115 sequences**. The original 108
sequences and 1,782 calls remain preserved. All 16 historical lifecycle test
files retain their assertions. New process-isolated tests use a 256 KiB evaluator
stack and a 60-second timeout; child aborts, failures, and timeouts fail the
parent test. They cover 768-deep arguments/defaults/Reference validation,
captured closures, Host reentry, retries, GC, evaluator isolation, and terminal
cleanup. Internal tests exercise 20,000-level shared graphs, concurrent last
release, 2,048-level Reference metadata, and cancellation during both default
construction phases. These depth controls are test workloads, not language
compatibility limits.

All [seven required checks](validation-stage19-final.json) pass, including
**392 Rust tests and 194 Python tests**. One existing ignored Rust probe is
excluded, and isolated child invocations are not counted again. Actual
[Node and Chromium execution](platforms-stage19-final-evidence.tar.gz) passes
**71 development scenarios per target**, preserving the prior 68 scenarios.
The shared primitive corpus retains 6,078 engine matches and 6,076 Java matches
per target, with the same two regex failures. M1 regressions preserve the prior
frontend/value matches and the existing cycle, Host diagnostic, and upstream
Java failures.

The [exploration archive](exploration-stage19-evidence.tar.gz) and its
[support archive](exploration-stage19-support-01-evidence.tar.gz) retain
**13,900 original paths using 5,906 unique payload blobs**. Complete rehydration
verifies original bytes, sizes, and modes. Historical Stage 18 archives remain
explicit hash dependencies rather than duplicated payloads. The original 166
observations retain identical Rust outcome bytes and all 25 nonmatches; one
Java internal failure is now explicitly classified as an oracle error. Separate
focused exploration records 66 matches, ten mismatches, and one oracle error
across 77 observations. These overlapping counts are not added to coverage.

The original 125-byte infinitely growing predicate is preserved unchanged.
Its Stage 18 native-stack SIGABRT remains historical evidence. The new isolated
VM test executes 150,000 VM steps three times with cancellation and
reuse, then also drops a running VM. These steps include type/default scheduler
work, not only bytecode instructions. The unchanged development adapter now
reaches its instruction-budget error on the original input. Java instead raises
an internal stack failure; this input remains outside passing compatibility
claims. Reference metadata work within one instruction, expanded diagnostic
text, arbitrary authored parser/compiler nesting, and uniform wall-time
cancellation bounds remain open.

The [source audit and gate captures](source-stage19-final-evidence.tar.gz) bind
**5,059 captured project source copies across 2,284 unique paths** to the exact
implementation commit. No upstream source copies are included; reference pins
and oracle provenance are separate checks. The independent
[post-seal audit](stage19-post-seal-audit.json) verifies all 13 new archives,
15 historical archive dependencies, captured source identities, and gate results.
All 474 requirements and 16 partial
mappings remain; 2,821 overlapping accounting rows contain 1,821 ready, 960
unimplemented, and 40 unmapped entries. Exact source-symbol migrations point to
the resumable default constructor and retain the original obligations. Integrity
and development gates pass; strict selected and full compatibility
gates fail, retaining 1,950 full-gate failure reasons. Inventory counts are not
completion percentages. Remaining language/type/default/scope
compatibility, product APIs, release platforms, and execution-time/peak-RSS
parity remain open. **Stage 19 closes only its bounded local development
contract.**

## Stage 18 local verification checkpoint

Implementation `e918efdf718f0bebe1b5184e10421b1b1ffaa730` is committed and pushed.
Runtime implementation and independent type-consumer review used
`gpt-6-astra / high`; Java comparisons, lifecycle tests, actual platform execution,
and evidence auditing used `gpt-6.1-sol / high`. All final checks ran locally on
macOS. M2 remains in progress.

Constraint bytecode now carries symbolic parameters and receives actual type
arguments at runtime. Recursive back edges reuse active templates, allowing
finite recursion with changing arguments such as `A<List<T>>`. Inline and lambda
constraints capture their actual arguments; generic let roots share Java's
declaration call target and retain its `unknown` parameter behavior. Let and for
bodies enforce their inherited const boundary while annotations, initializers,
iterables, and lambda lookup retain their respective evaluation scopes.

Independent review found and corrected two regressions before the final build:
symbolic collection element types lost their defaults, and deferred generic
`Reference<D,T>` uses skipped constrained-referent validation. Defaults now use
the resolved element class after substitution; Reference validation occurs at
the authored type-resolution point and preserves short-circuited constraint
bodies. Original Java and Stage 17 outcomes, superseded builds, interrupted
acquisitions, and pre-fix failures remain in exploratory evidence.

The [final summary](stage18-final-summary.json) records **1,798 independent cases,
1,796 raw matches, and the same two failures**, with **63 passing metamorphic
relations**. All 139 additions match: 85 successful values and 54 expected
errors. Original 1,659 case definitions, input bytes, and 56 relations are
preserved. `error-cycle` remains an oracle error and `error-remainder-zero`
remains a raw diagnostic mismatch. No new development exception was added.
The separate prior-selection replay retains 1,657 matches and those two failures.

The [persistent contract](class-lifecycle-stage18-final-evidence.tar.gz) passes
**1,782 calls across 216 executions of 108 sequences**. The original 67
sequences and their 6,312 Java/Rust status, stdout, and stderr files remain
byte-identical. Native tests cover argument capture, Host suspension and reentry,
GC, evaluator isolation, cancellation, retries, and terminal cleanup. Historical
value-count assertions remain intact, executable entries are counted separately,
and terminal cleanup requires zero total heap entries.

All [seven required checks](validation-stage18-final.json) pass, including
**382 Rust tests and 194 Python tests**. One existing ignored Rust probe is
excluded. Actual [Node and Chromium execution](platforms-stage18-final-evidence.tar.gz)
passes **68 development scenarios per target**. The shared primitive corpus
retains 6,078 engine matches and 6,076 Java matches per target, with the original
two regex failures. M1 regressions retain 15 frontend matches, six runtime value
and two diagnostic matches, the runtime cycle difference, two Host protocol
matches, and the historical full-diagnostic/upstream failures.

The [exploration archive](exploration-stage18-evidence.tar.gz) requires all three
support archives for offline review: [1](exploration-stage18-support-01-evidence.tar.gz),
[2](exploration-stage18-support-02-evidence.tar.gz), and
[3](exploration-stage18-support-03-evidence.tar.gz). Their inventories and complete
rehydration verify **74,806 original paths** against **10,671 unique payload
blobs**, preserving every original byte, size, and mode. Broader exploration
retains 141 matches and 25 mismatches across 166 single-case observations, plus
529 matches and 69 mismatches across 598 overlapping exploratory calls. These
counts are separate from the selected and persistent contracts.

The [source audit and captured gates](source-stage18-final-evidence.tar.gz) bind
**4,996 captured project source copies across 2,244 unique paths** to the exact
implementation commit. This count includes no upstream source copies; reference
pins and oracle provenance are verified separately. The inventory retains all 474
requirements and 16 partial mappings; 2,798 overlapping accounting rows contain
1,798 ready, 960 unimplemented, and 40 unmapped entries. Integrity and development
gates pass. Strict selected and full gates retain their failures, including
1,950 full-gate failure reasons. These counts are not a completion percentage.
The [post-seal audit](stage18-seal-audit.json) rechecks archive identities, the
executed packaging helper, four independent reviews, and six raw gate captures.

Unbounded growth of owned type-argument metadata can still exhaust the native
stack. The original `this is A<List<T>>` SIGABRT is preserved; bounded yielding
and cancellation tests do not establish safe unbounded execution. Remaining
type/default/Reference and scope diagnostics, strict Map/Set, annotations,
module/cache behavior, original upstream adapters, product APIs, and release
platforms remain open. No execution-time or peak-RSS parity is claimed.
**Stage 18 closes only its bounded local development contract.**

## Stage 17 local verification checkpoint

Implementation `43db2bc4664ae91521204021deac8b7ce1641f3b` is committed and pushed.
Runtime work and independent ownership review used `gpt-6-astra / high`; Java evidence, lifecycle
checks, actual platform execution, and evidence auditing used `gpt-6.1-sol / high`.
All checkpoint verification ran locally on macOS. M2 remains in progress.

Evaluator-owned executable and type-use nodes now preserve local const-property
resolution across failed first reads and later retries. Inline uses and cloned
lambdas retain separate nodes; selected non-generic let roots and lazy collection
members share their appropriate nodes. Cached member definitions use the current
receiver, and inherited annotations retain declaration identity. Const methods,
lambda lookup depth, immediate `outer` scope checks, and grouped-callee parser
errors have bounded raw Java comparisons.

Independent review caught an execution-order bug: reaching a type check could
change ownership of a subsequently created lambda or object. The corrected model
keeps implicit canonical ownership stable. Both branch orders now have persistent
Java controls, native Host reentry, cancellation, and GC regressions. Superseded
successful checks and the interrupted first whole-selection acquisition remain
preserved; they are excluded from final implementation claims.

The [final summary](stage17-final-summary.json) records **1,659 independent cases,
1,657 raw matches, and the same two failures**, with **56 passing metamorphic
relations**. The 62 additions comprise 31 successful values and 31 expected
errors. The original 1,597 definitions, inputs, and 54 relations remain unchanged.
`error-cycle` remains an oracle error and `error-remainder-zero` remains a raw
diagnostic mismatch. No new development exception was introduced.

Persistent comparison separately covers **67 sequences, 134 executions, and
1,052 matching calls**. Actual Node and Chromium
each pass **61 development scenarios**. The shared primitive corpus retains
6,078 engine matches and 6,076 Java matches per target, with the same two regex
mismatches. Required Rust, Python, inventory and reference checks pass; see
[validation](validation-stage17-final.json). Native retention tests preserve exact
historical value counts, separately count persistent executable entries, and
require zero total heap entries on terminal cleanup. M1 regressions preserve all
15 frontend matches, six runtime value matches, the existing runtime cycle
diagnostic difference, and the historical Host diagnostic/upstream failures.
These bounded regression contracts do not establish full API compatibility.

[Exploratory evidence](exploration-stage17-evidence.tar.gz) retains initial
hypotheses, invalid controls, discovered mismatches, superseded builds, and final
replays. This evidence is partitioned into the main archive and four indexed
history archives: [1](exploration-stage17-history-01-evidence.tar.gz),
[2](exploration-stage17-history-02-evidence.tar.gz),
[3](exploration-stage17-history-03-evidence.tar.gz), and
[4](exploration-stage17-history-04-evidence.tar.gz). An offline review requires
all five parts. Each has a verified inventory; the index proves disjoint paths
and complete original file/hash/size conservation. The original Stage 16
single-case exploration improves from 84/94 to 93/94 matches; recursive generic instantiation remains unresolved. All original
82 persistent retry calls now match. Remaining observations include generic
let-root sharing, const let-body scope behavior, local typed-property frames,
and other parser or missing-member diagnostic differences. Failed exploratory
cases are not selected as passing compatibility evidence.

The [source audit](source-stage17-final-evidence.tar.gz) binds **2,409 captured
project source copies across 1,946 unique paths**
to the exact implementation commit. No upstream source blobs are included in
that count; reference pins and oracle provenance were verified separately.
Historical explorations retain their own provenance. The inventory preserves all 474 requirements and 16 partial reviewed
mappings: 2,659 overlapping accounting rows contain 1,659 ready, 960 unimplemented,
and 40 unmapped entries. Integrity and development gates pass; strict selected
and full gates retain their failures, including 1,950 full-gate failure reasons.
These inventory counts are not a completion percentage.

Recursive generics, the remaining const-scope and diagnostic corpus, strict
Map/Set, annotations, modules and cache behavior, original upstream adapters,
product APIs and release platforms remain open. No execution-time or peak-RSS
parity is claimed. **Stage 17 closes only its bounded local development contract.**

## Stage 16 local verification checkpoint

Implementation `540d81818a0c5c18fab6d6354356621dfea9f962` is committed and pushed.
Runtime implementation and independent review used `gpt-6-astra / high`; Java
evidence, lifecycle tests, platform execution and evidence auditing used
`gpt-6.1-sol / high`. Verification ran locally on macOS; no hosted CI or
unexecuted Linux/Windows result is claimed.

Constraint evaluation now shares a mutable auxiliary receiver slot within each
logical call. Nested `is`/`as` and recursive alias checks can change the receiver
seen by later constraints and failure rendering. Lambdas capture the receiver
value at creation, while function, local-writing let, lazy member and Host reentry
calls have separate slots. Typed slots and captured cycles are traced by the
existing heap. Selected first reads in alias bodies enforce the deferred module
const boundary, and method errors cover the complete call span. Uncolored diagnostic rendering
follows Java's recursive frame compression, including partial patterns,
count-one loops, margins and hint placement.

The [exploration archive](exploration-stage16-evidence.tar.gz) replays all five
Stage 15 gaps at their original inputs and URIs. Four now match: three shared-slot
failures and the non-const alias capture failure. Recursive generic instantiation
remains unimplemented. The archive retains 7,629 files, including failed
hypotheses, superseded builds and unchanged original raw outputs. Its SHA-256 is
`197de42db32f6be50a4c74a605ba5cc734a060a3240b2bf6e82190ad67f6b673`.

The 84 new selected fixtures match Java at their final fixture URIs, giving
**1,597 independent cases and 54 metamorphic relations**. Original 1,513 case
definitions, input bytes, 52 relations and requirement links are preserved.
[Fresh whole-selection evidence](language-stage16-final-evidence.tar.gz) records
**1,595 raw matches and the same two failures**: `error-cycle` is an oracle error
and `error-remainder-zero` is a diagnostic mismatch. All 54 relations pass.
[Prior-selection replay](regressions-stage16-final-evidence.tar.gz)
separately retains the previous Java captures and the same 1,511 matches and two
known failures. Neither comparison normalizes output or grants a new exception.

The [persistent contract](class-lifecycle-stage16-final-evidence.tar.gz) passes
**502 calls across 72 executions of 36 unique sequences**. Two new sequences add
48 calls; the original 454 calls preserve all 2,724 Java/Rust
status/stdout/stderr files byte for byte. Native tests separately cover Host
suspension, child reentry, creation-time captures, failed retries, stale tokens,
shared-Program evaluator isolation, cancellation and terminal cleanup. This is
lifecycle evidence, not a claim of Java Host API equivalence.

The [seven required checks](validation-stage16-final.json) pass: **338 Rust tests
and 194 Python tests**, formatting, workspace Clippy, Wasm compilation, reference
pins and inventory freshness. One existing ignored Rust probe is excluded.
Actual [Node and Chromium execution](platforms-stage16-final-evidence.tar.gz)
passes **56 scenarios per target**. All 6,078 primitive results agree across Rust
targets; 6,076 match Java per target and the same two regex differences remain
failed. Existing M1 evidence retains 15 frontend matches, six runtime values and
two complete diagnostics matching, one runtime cycle diagnostic mismatch, two
lifecycle protocol matches and four lifecycle full diagnostic mismatches.

The [summary](stage16-final-summary.json), [source proof](source-stage16-final-evidence.tar.gz)
and [gate captures](gates-stage16-final-evidence.tar.gz) distinguish source
identity, evidence integrity, development acceptance and strict compatibility.
All 2,199 captured final source copies across 1,818 unique paths equal the
exact implementation commit; the language acquisition retains 1,698 snapshots.
This source proof contains no upstream submodule source copies. Reference pins
were verified separately by `cargo xtask verify` and pinned oracle metadata.
The original baseline snapshot repair and exact Git reads of omitted generated
metadata remain explicit. A first packaging-readiness attempt
incorrectly required previous bytes for three intentionally extended platform
sources. Its failed capture and executed helper are retained; the corrected
check preserves prior scenario identities and order and binds updated sources
to the implementation commit. All 474 requirements remain;
16 reviewed mappings are partial, and the 2,597 overlapping accounting rows
contain 1,597 ready, 960 unimplemented and 40 unmapped entries. These counts are
not a completion percentage. The strict full gate retains **1,950 failure
reasons**; integrity and development checks pass, while strict selected and full compatibility gates fail.

The final exploratory single-case comparison has **84 matches and 10 failures**:
one recursive generic case, one grouped-callee parser diagnostic, and eight
variants where Java permits a const function's local property read but Rust
rejects it. Separate persistent exploration has **69 matches and 13 failures
across 82 calls**. Java sometimes caches a local property's resolution before
raising a const error, allowing a later retry to succeed. Inline alias copies,
recursive depths, lambda bodies and shared let call targets have different cache
identities; a flag per alias or bytecode block would merge distinct states.
These failures remain recorded obligations and were not promoted as passing
selected fixtures. Complete runtime type-use identity is the next dependency.

Object payloads are boxed to keep small receiver-slot arena entries compact.
Lifecycle checks do not measure peak RSS or execution speed; no performance or
memory parity is claimed. Original upstream suites, recursive generics and
remaining constraint/modifier semantics, strict Map/Set, annotations,
module/cache behavior, product APIs and release platforms remain open.
**Stage 16 is closed within its bounded local development contract; M2 remains
in progress.**

## Stage 15 local verification checkpoint

Implementation `7eca4046a4a40519d8a53348deb94ed98827b5a7` is committed and pushed.
Runtime work used `gpt-6-astra / high`; Java evidence, platform verification, and
independent review used `gpt-6.1-sol / high`.

Stage 15 replaces infinite compile-time expansion of supported non-generic recursive
constraints with a finite graph of shared constraint bytecode blocks. Unused
declarations and short-circuited recursive uses can finish without executing the
recursive branch. Selected direct and mutual finite recursion, alias declaration type
scope, and native Host/GC lifecycle checks use the shared engine. Builtin `Class<T>`
checks its single-argument arity and erases the argument without resolving it. Missing
types, failed or cyclic aliases, constrained types, and invalid class signatures inside
an erased argument therefore remain unforced. Generic aliases surrounding `Class<T>`
retain their own ordinary argument-resolution behavior. This is bounded support;
expanded type checks remain statically represented.

The final recorded release artifact matches **41/41 additions** against pinned Java
0.32.0, comprising **33 successful values and eight expected errors**, with complete
status/stdout/stderr comparisons. Two equal-stdout relations bring the selected
inventory to **1,513 cases and 52 relations**. The original 1,472 case definitions,
input bytes, and 50 relation definitions remain unchanged. The independent fixture audit
also confirms that all 2,472 prior inventory rows retain their execution states, inputs,
required flags, and requirement links. The [fresh full
acquisition](language-stage15-final-evidence.tar.gz) records **1,511 raw matches among
1,513 independent cases**, retaining `error-cycle` as an oracle error and
`error-remainder-zero` as a mismatch. All **52 metamorphic relations pass**. The
[prior-selection replay](regressions-stage15-final-evidence.tar.gz) independently
preserves the original 1,470 matches and two failures against retained Stage 14 Java
bytes; it is separate from the fresh acquisition.

The [final persistent capture](class-lifecycle-stage15-final-evidence.tar.gz) records
**34 sequences, 68 fresh-evaluator executions, and 454 matching raw calls**. The two
additions contribute 48 comparisons across failure/retry, finite recursive checks, alias
publication, erased Class arguments, wrong values, and lazy arity failures. The original
32 sequences, ordered call lists, and 406 raw outcomes are unchanged; the independent
audit compares all 2,436 original Java/Rust status/stdout/stderr capture files byte for
byte. Native Host/GC tests separately exercise suspension, recursive reentry, temporary
retention and reclamation, cancellation/retry, and terminal cleanup. They remain an
architectural contract rather than a raw Java Host-equivalence claim.

The [seven required local checks](validation-stage15-final.json) pass, including **315
Rust tests and 194 Python tests**, formatting, workspace Clippy, pinned references, Wasm
compilation, and inventory freshness. One existing ignored Rust probe remains excluded
from the passing count. Actual [Node and
Chromium](platforms-stage15-final-evidence.tar.gz) each pass **54 scenarios**, including
the two new Java-derived persistent sequences at the identical module URI. All 6,078
primitive results agree across Rust targets; 6,076 match Java per target, preserving the
two failed regex comparisons. The user-approved acceptance route is local verification.
No Stage 15 remote CI result or unexecuted Linux/Windows result is claimed.

The [final local summary](stage15-final-summary.json) records the bounded acceptance
scope. The language acquisition retains 1,610 source snapshots. The [source
proof](source-stage15-final-evidence.tar.gz) binds **2,091 captured source copies across
1,725 unique paths** to the exact implementation commit. This acquisition contains no
upstream submodule source snapshots; reference versions were checked separately by
`cargo xtask verify` and the pinned oracle metadata.
Raw command captures and build identities remain separate from this byte comparison;
the audit does not execute captured evaluator binaries. Existing M1 regressions retain
15 frontend matches, six runtime values and two complete diagnostics matching, one
cycle diagnostic mismatch, two lifecycle protocol matches, and four complete lifecycle
diagnostic mismatches.

The [source-scope amendment](source-stage15-final-amendment-evidence.tar.gz) corrects
an unconditional upstream-blob flag in the original packaging record; it does not
change any captured source or result. It also preserves the first packaging attempt,
whose guard incorrectly required the expanded inventory to match its old file bytes.
The corrected guard verifies original rows and requirement links while allowing the
recorded additions. All original sealed archives remain unchanged.

The [exploration archive](exploration-stage15-evidence.tar.gz) preserves **3,995 files**
and 46 final observations: 41 raw matches and the following **five known mismatches**.
Its SHA-256 is `5c49edd4105cf5a9a68c8cee95e6696b87a74af703a1ca4769954ce2913f734a`
(10,869,925 bytes). Original exploratory expectations, failed sequence drafts, the
native lifecycle draft failure, frozen baseline inputs, build provenance, and the
complete Java/Rust outputs remain intact. The final focused and exploratory replays use
the same recorded release artifact, SHA-256
`4a27dfe9a2bb8fd8e4b11e6e39f9900322d410aa0a17f3a59e3c03f0eff78fb6`.

| Exploratory case | Remaining mismatch |
| --- | --- |
| `recursive-finite-rejection` | Both runtimes reject the value, but Java reports constraint `Value: 0` and Rust reports `Value: 2`. |
| `recursive-as-in-constraint` | Java rejects the recursive cast constraint with `Value: -1`; Rust incorrectly succeeds with `3`. |
| `recursive-this-after-is` | Both runtimes reject the value, but Java reports `Value: -1` and Rust reports `Value: 2`. |
| `recursive-generic-shadow` | Java succeeds with `2`; Rust explicitly reports that recursive generic constraints are unimplemented. |
| `recursive-lexical-capture` | Java rejects a non-const `threshold` property captured by an alias constraint; Rust incorrectly succeeds with `2`. |

The first three failures expose Java's shared custom-`this` frame slot: a nested
recursive check overwrites the enclosing constraint receiver and the diagnostic reads
the overwritten slot. The current Rust constraint calls retain separate receiver values.
Matching this behavior needs frame-slot identity and capture/lifecycle handling beyond
reuse of bytecode block IDs. Recursive generic constraints need a parameterized
recursive type representation; the current bounded graph explicitly rejects an active
recursive use with generic substitutions. The non-const capture failure remains a
lexical/const-scope semantic gap; the separate const-capture control succeeds in both
runtimes. These failures are retained exploratory obligations, without new development
exceptions or claims that stronger draft sequences passed.

All 474 requirements, 15 partial reviewed mappings, 960 unimplemented accounting rows,
40 unmapped rows, and previously unexecuted sequence groups remain explicit. The [gate
captures](gates-stage15-final-evidence.tar.gz) preserve passing integrity and
development checks and failing strict selected/full gates; the strict full gate retains
**1,950 failure reasons**. The inventory has 2,513 overlapping accounting rows: 1,513
ready, 960 unimplemented, and 40 unmapped. These are not feature completion rates.
Complete generic/constraint semantics, strict Map/Set behavior, annotation and
module/cache behavior, original upstream suites, product APIs, and speed/memory targets
remain unfinished. **Stage 15 is closed within its bounded local development contract;
M2 remains in progress.**

## Stage 14 checkpoint accepted with local verification

Implementation `4b27b7ecf2c6133e363ed228606b289e85c2e627` is committed and pushed.
Runtime and type-resolution work used `gpt-6-astra / high`; Java evidence,
lifecycle, platform, and independent review work used `gpt-6.1-sol / high`.

The shared compiler now emits ordered resolution plans separately from expanded
value checks. Missing names stop later declaration publication. Class initialization
and argument-count checks precede argument resolution; alias arguments precede
unfinished-alias and argument-count checks. Bare generic parameters shadow module
and builtin names, while parameterized names such as `T<String>` resolve the named
declaration. Nested substitutions retain caller scope, and owner initialization
preserves signature error order and authored diagnostic spans. Expanded metadata
remains statically compiled; this is a bounded implementation of ordered resolution.

The [final local summary](stage14-final-summary.json) and [fresh Java
capture](language-stage14-final-evidence.tar.gz) record **1,470 matches among 1,472
independent cases**, retaining `error-cycle` as an oracle error and
`error-remainder-zero` as a mismatch. All **50 metamorphic relations pass**.
The 92 additions comprise 31 successful values and 61 expected errors, all matching
complete Java status/stdout/stderr bytes. The original 1,380 definitions, input bytes,
and 48 relations remain unchanged. The [prior replay](regressions-stage14-final-evidence.tar.gz)
uses retained Stage 13 Java bytes and preserves its 1,378 matches and two failures;
it is separate from the fresh full acquisition.

The [persistent evaluator capture](class-lifecycle-stage14-final-evidence.tar.gz)
records **32 sequences, 64 fresh-evaluator executions, and 406 matching raw calls**.
Five additions contribute 104 calls; the original 27 sequences and 302 raw call
outcomes remain unchanged. Error order, generic shadowing, nested arguments, and
unfinished alias arguments are observed across repeated calls in one evaluator.
Native GC/Host assertions remain a separate architectural contract.

[Required checks](validation-stage14-final.json) pass: **303 Rust tests, 194 Python
tests**, formatting, workspace Clippy, reference pins, Wasm build, and inventory
freshness. One existing ignored probe is excluded from the ordinary passing count.
The 15 new Rust tests comprise eight core regressions and seven lifecycle tests.
Actual [Node and Chromium](platforms-stage14-final-evidence.tar.gz) each pass **52
scenarios**. All 6,078 shared-engine primitives agree; 6,076 match Java on each
platform and the two known regex differences remain failures. Existing M1 captures
retain 15 frontend matches, six runtime values and two complete diagnostics matching,
one cycle diagnostic mismatch, two lifecycle protocol matches, and four complete
lifecycle diagnostic mismatches.

The language acquisition retains **1,567 source snapshots**. The [exact-commit
source proof](source-stage14-final-evidence.tar.gz) binds **1,961 captured copies
spanning 1,677 unique paths** across seven final suites to implementation bytes,
including pinned submodule blobs. Source and executable identities remain stable.
The [superseded evidence](superseded-stage14-evidence.tar.gz) preserves the first
workspace failure: historical tests expected compilation errors that now occur at
owner initialization. The corrected tests assert both phase and exact messages.
An in-flight language acquisition was explicitly stopped and restarted; pre-correction
captures were retained. Unaffected frontend/runtime/platform source snapshots remain
valid, and changed-source suites were reacquired in fresh directories.

The [exploration archive](exploration-stage14-evidence.tar.gz) retains 11,722 files,
including pre-fix failures and successful pre-correction captures. The final discovery
replay has 96 matches among 98 observations; two compound-output adapter limitations
remain unselected. Final focused additions match 92/92. The earlier 191 Reference
observations retain 173 matches and 18 mismatches. Exploratory caller-supplied artifact
labels remain unchanged; separate hashes bind final replays to the stable recorded
release artifact. These observations do not replace original upstream suite contracts.

The [gates](gates-stage14-final-evidence.tar.gz) distinguish passing integrity and
development checks from failing strict selected/full gates. Fifteen reviewed mappings
remain partial. Inventory accounting retains 474 requirements and 2,472 overlapping
rows: 1,472 ready, 960 unimplemented, and 40 unmapped. The strict full gate still has
**1,950 failure reasons**. These are accounting counts, not feature completion rates.
No development exception was added.

[Implementation CI](https://github.com/luuvish/pkl-rs/actions/runs/39561429923)
was **blocked before any job started**. GitHub reported failed account payments or
a spending limit requiring adjustment. The [CI summary](ci-stage14-summary.json)
and [raw blocked-run evidence](ci-stage14-blocked-evidence.tar.gz) retain all six
not-started jobs and their annotations; zero artifacts were produced. No CI tests,
original ZIP digest checks, remote source checks, or offline artifact replays passed
or executed. This blocked run remains a historical CI failure. The preceding
report-only commit's separate CI completed all six jobs successfully.

The user subsequently authorized local verification as the development acceptance
path. The [independent local acceptance report](local-acceptance-stage14.json) and
[sealed audit](local-acceptance-stage14-evidence.tar.gz) verify the original archives,
exact implementation sources, and raw outcomes without executing captured binaries.
A fresh pinned Java identifier probe also matches the committed Rust table bytes.
Unchanged baseline tooling is distinguished from fresh checks; Linux and Windows
execution remains unverified for this implementation. No hosted ZIP is required
when none exists. **Stage 14 is closed within its bounded local development contract.**

At the Stage 14 boundary, recursive constraint/type resolution and erased
`Class<T>` arguments remained open; Stage 15 above addresses a bounded subset.
Strict Map/Set, annotation syntax, general module loading/cache behavior, full
reflection, and the remaining upstream corpus still require work. Full language/API
compatibility and speed/memory parity are not established by these checkpoints.

## Stage 13 checkpoint

Implementation `f90d7b09124bb5a7d73dd5b2bfcddf2a90d2761f` is pushed. Stage 13 adds
evaluator-owned type-alias declaration values, qualified diagnostic names, and function
parameter/result metadata to the shared engine. Alias values preserve declaration
identity independently of their expanded checks. A failed definition can retain its
published singleton; later value reads and type uses have different observable outcomes.
The selected repairs preserve those outcomes, including cyclic-alias errors at the later
authored use and partial class publication after a failed typed member.

Runtime and type-resolution work used `gpt-6-astra / high`; Java differential evidence,
lifecycle tests, and independent review used `gpt-6.1-sol / high`.

Arrow and FunctionN annotations check runtime function arity while retaining argument
and result types as metadata. They do not introduce parameter/result wrappers around an
existing function; the function's own declared checks still apply when called. Exported
arrow metadata preserves nested alias identity and exact constraint source text.
Equality and referent subtyping remain separate operations: identical arrow metadata can
compare equal while its Reference subtype check returns false. The selected bare
Function/FunctionN variance and argument-count cases reproduce the pinned Java behavior.

The independent selection contains **1,380 cases and 48 metamorphic relations**. The 89
additions comprise 61 successful values and 28 expected diagnostics; two new relations
cover alias expansion and function-result annotations. All **1,291 original case
definitions and input bytes**, and all 46 previous relations, remain preserved. Focused
execution of the additions matches all 89 complete Java outcomes. These comparisons use
the pinned Pkl 0.32.0 oracle without trimming, sorting, or reserialization.

The recorded-build declaration lifecycle capture retains **27 sequences, 54
fresh-evaluator sequence executions, and 302 matching raw calls**. Its subtotal is **194
original class comparisons plus 108 new alias/function comparisons**. A sequence retains
its evaluator and module across errors, repeats in a fresh evaluator, and compares
status, stdout, and stderr. New calls exercise failed alias reuse followed by class
property checks, `is`, `as`, typed lets and lambdas, construction, and superclass
resolution. Original class fixtures and unexecuted-group records remain unchanged. The
[verified lifecycle archive](class-lifecycle-stage13-final-evidence.tar.gz) retains the
original raw calls, recorded build, and source snapshots.

The [required checks](validation-stage13-final.json) record **288 passing Rust tests and
194 Python tooling tests**; one existing ignored differential probe remains separate.
The 18 new Rust regressions comprise 12 core tests and six GC/Host lifecycle tests.
Actual Node and Chromium each pass **49 development scenarios**; the two existing Java
regex differences remain failed compatibility cases. Native GC, Host, reentry, and
terminal-cleanup assertions retain their separate architectural denominator.

The original 191-case Reference exploration now has **173 matches and 18 remaining
mismatches**, closing four previous failures without deleting cases. Remaining
observations require strict Map/Set support, Duration metadata, module
inheritance/loading and ModuleOutput integration, additional library operations, or
complete primitive missing-method diagnostics. This exploration does not replace the
original upstream suite contracts.

The [verified exploration archive](exploration-stage13-provisional-evidence.tar.gz)
preserves 14,770 files, including draft parser failures, the malformed first lifecycle
inventory, source drift, and pre-fix diagnostic differences. Its SHA-256 is
`b506dbbf333d67ea1a0e9243495e78d354477f7235a8b964cdb8b98c0b6053ea`. The final focused
replays use frozen candidate SHA-256
`3e9cca27b862f2e228db4c643a7abd89559d1b16a8646e069a3f1230cabffa94`, identical to the
retained recorded release build with stable before/after source hashes. Earlier
caller-supplied artifact labels remain explicit.

The [final local summary](stage13-final-summary.json) links the [fresh Java
acquisition](language-stage13-final-evidence.tar.gz): **1,378 of 1,380 expressions
match**, with the existing `error-cycle` oracle error and `error-remainder-zero`
mismatch retained. All **48 relations pass**. Recorded-build candidate and source
identities remain stable throughout acquisition. The language capture retains **1,473
source snapshots**; **1,852 copies spanning 1,572 unique paths** across seven final
evidence suites match exact implementation commit bytes. The [source
proof](source-stage13-final-evidence.tar.gz) retains this audit and the original
packaging failure: its first helper expected relation appearance order instead of the
language harness's canonical case-ID tuple order. The correction changes no raw outcome
or implementation source.

The [regression archive](regressions-stage13-final-evidence.tar.gz) preserves the
previous 1,291-case replay (1,289 matches and the same two failures), 15 frontend
matches, six runtime value and two full diagnostic matches, one runtime cycle diagnostic
mismatch, two lifecycle protocol matches, and four full lifecycle diagnostic mismatches.
The [platform archive](platforms-stage13-final-evidence.tar.gz) retains all 6,078
shared-engine primitive matches, 6,076 Java matches per platform, and the two existing
regex gaps.

The [gate archive](gates-stage13-final-evidence.tar.gz) separates passing
integrity/development checks from failing strict subset/full gates. Fifteen reviewed
mappings remain partial. The inventory retains 474 requirements and 2,380 overlapping
accounting rows: 1,380 ready, 960 unimplemented, and 40 unmapped. These counts are not
independent feature counts or completion percentages. The full gate retains **1,950
failure reasons**, with no new development exceptions.

The [implementation CI](https://github.com/luuvish/pkl-rs/actions/runs/39554130320)
completed all six jobs successfully. [Independent verification](ci-stage13-summary.json)
checked both original GitHub ZIP digests and **1,860 source copies spanning 1,575 unique
paths** against the exact implementation commit. Remote results retain 1,378 matching
expressions, the same two known failures, 48 relations, 302 separate ordered calls, and
49 Node/Chromium scenarios each. The [original CI package](ci-stage13-evidence.tar.gz)
and [two offline replays](ci-stage13-offline-evidence.tar.gz) pass without executing
captured binaries; the sealed 182-file package remains unchanged. Artifact download
required retries after timeout/TLS failures. The evidence preserves sanitized transport
records and original capture hashes; the downloaded ZIPs remain byte-identical to
GitHub's recorded digests.

**M2 remains in progress.** Complete runtime type resolution, mixed missing-name/type
error ordering, and generic parameter shadowing failures remain open, alongside
annotations, module caching, broader reflection, and the remaining required language
corpus. Selected deferred checks do not establish universally lazy type resolution.
Strict full completion and product API, performance, and memory targets remain
unachieved.

## Stage 12 checkpoint

The Stage 11 lifecycle mismatch is resolved within the currently supported class
declaration scope. A declaration publishes its evaluator-owned singleton before
superclass resolution, publishes successful properties followed by methods, and
retains its parent and completed declaration prefix after an initialization
error. Later accesses return that partial class without restarting initialization.
Ordinary failed lazy properties remain retryable. Published class handles are
strong GC roots until terminal cleanup; temporary builders and unrelated values
remain collectible. Missing typed property and method diagnostics enumerate the
published definitions and inherited candidates.

Implementation `50fc60b2aeac6fa9bb43c6c6043fc8f9976cb287` is pushed.
Runtime, GC, and evaluation-order work
used `gpt-6-astra / high`; evidence, lifecycle, and independent review work used
`gpt-6.1-sol / high`.

The [persistent sequence summary](stage12-class-lifecycle.json) and
[verified raw archive](stage12-class-lifecycle-evidence.tar.gz) retain **18 unique
sequences, 36 fresh-evaluator executions, and 194 matching raw call comparisons**.
Each sequence keeps one Java evaluator and one Rust VM/module across successful
and failed calls, then repeats in a fresh evaluator. The calls cover failed
superclass, property type, modifier, and method initialization; retry identity;
published and absent declarations; partial assignments; wrapper retry; recursive
types; and partial Reference metadata without forcing referent bodies. The
archive also retains the earlier 22 full-diagnostic mismatches, a source-drift
rejection before comparison, Java-only explorations, and timeout observations.
There is no new mismatch allowance.

Final local checks include **270 passing Rust tests and 194 Python
tests**; the existing ignored differential probe remains separate. Nineteen new
Rust regressions comprise nine core, eight lifecycle, and two sequence-API tests;
fifteen new Python tests cover protocol and provenance failure detection.
Dedicated native tests verify exact retained graphs, temporary handle
reclamation, isolated heaps, ordinary retries, and terminal release. These
architecture assertions have a separate denominator from Java language calls.
Actual Node and Chromium each pass **46 development scenarios**. All 6,078
primitive results agree across Rust targets, with 6,076 Java matches and the two
existing regex failures preserved.

The **1,291 independent-expression definitions and inputs and 46 metamorphic
relations are unchanged**. Both the replay against prior Java captures and the
fresh Java/recorded-build acquisition retain 1,289 matches and the same two
failures; all 46 relations pass. The language acquisition retains 1,382 source
snapshots. Across the final language, class-sequence, and M1 regression captures,
1,738 source copies spanning 1,462 unique paths match the exact implementation
commit bytes.

The [final local summary](stage12-final-summary.json) links the
[fresh language acquisition](language-stage12-final-evidence.tar.gz),
[required checks](validation-stage12-final-evidence.tar.gz), and
[actual platform execution](platforms-stage12-final-evidence.tar.gz).
The [source audit](source-stage12-final-evidence.tar.gz) retains exact-commit
checks and an independent review of result IDs, retained execution metadata,
and counts recomputed from raw captures. It adds no compatibility test cases.

The [regression archive](regressions-stage12-final-evidence.tar.gz) retains
15 frontend matches, six runtime value and two complete diagnostic matches,
the existing runtime cycle diagnostic mismatch, two matching lifecycle
protocols, and four complete lifecycle diagnostic mismatches. It also preserves
the original-selection replay and a relocated class-contract recheck that
executes no captured binaries. The [gate archive](gates-stage12-final-evidence.tar.gz)
records passing integrity and development checks and failing strict selected
and full completion gates.

The [implementation CI](https://github.com/luuvish/pkl-rs/actions/runs/39548215484)
completed all six jobs successfully. [Independent verification](ci-stage12-summary.json)
checked both original GitHub ZIP digests and **1,746 captured source copies
spanning 1,465 unique paths** against the exact implementation commit. It confirms
the same 1,289 matching expressions and two existing failures, 46 relations,
194 separate class calls, and 46 Node/Chromium development scenarios each.
The [original CI package](ci-stage12-evidence.tar.gz) retains the captures and
verification. Two [offline replays](ci-stage12-offline-evidence.tar.gz) pass
without executing captured binaries; the final sealed 574-file package remains
byte-identical. The [previous report's CI](https://github.com/luuvish/pkl-rs/actions/runs/39546025680)
also completed all six jobs; its evidence belongs to the preceding commit.

The [provisional archive](exploration-stage12-provisional-evidence.tar.gz) retains
2,536 content files and 79 complete process pairs, including the interrupted
first language acquisition, a failing Node candidate, and superseded checks and
M1 captures. They are not counted as final passes. The separate
[review-only archive](stage12-class-review-evidence.tar.gz) retains a call to a
local declaration beyond the failed publication boundary: Java raises an
internal `PklBugException`, while Rust reports a normal missing-property error.
That diagnostic scope is outside the 18 selected sequences and remains open;
it receives neither a pass nor a development exception.

Actual Java annotation construction can call a Host reader and reenter the same
partial class. Rust annotation syntax remains unsupported, so injected bytecode
suspension tests establish scheduler behavior separately from that source path.
Java-only observations also distinguish same-URI cache retention, changed source
at the same URI, different URIs, and different evaluators; full Rust module-cache
identity remains an M4 integration gap. Indirect cyclic superclass identity is
compared, but Java property traversal timed out twice after 30 seconds, while
Rust visited-set guards terminate traversal. This remains unverified traversal
compatibility.

Next priorities are alias identity and function-type metadata, strict Map/Set
semantics, annotations and the remaining class/reflection rules, and module
inheritance and loading/cache integration. **M2 remains in progress**. The full
gate's 1,950 failure reasons and remaining required unimplemented and unexecuted
scope remain open. No new performance or memory result is claimed.

## Stage 11 checkpoint

**Persistent retry after failed class initialization remains a confirmed lifecycle
mismatch.** In one retained evaluator, the first invalid class access produces the
same complete error bytes in Java and Rust. Java then returns its partially
initialized class on the next two accesses, while Rust repeats the initialization
error. Java preserves declarations initialized before the failure, inherits the
parent's fixed declaration at the failed property, and leaves later declarations
absent. The same-session protocol, full raw channels, source and binary
identities, and the additional Java observations are preserved in the
[exploration archive](exploration-stage11-evidence.tar.gz). This protocol is
separate from the independent-expression selection and remains a failure without
a development exception. Successful Host read retries do not establish class
initialization retry compatibility.

The [stage 11 summary](stage11-final-summary.json) records **1,291 selected
comparisons: 1,289 raw-byte matches, one remainder-zero mismatch, and one cycle
oracle failure**. All 46 metamorphic relations pass. The 89 additions and two new
relations preserve every previous 1,202 definition and input byte. Implementation
`52cabbe01a8392ee44bf55889346c6f692d3c595` is pushed. The
[language archive](language-stage11-final-evidence.tar.gz) retains a recorded
release build, fresh pinned Java captures at actual fixture URIs, complete output
channels, and 1,378 source snapshots. Source and artifact identities stayed stable
during acquisition, and every snapshot matches that exact implementation commit.

The shared engine now carries fixed-property metadata and performs ordered
override checks. An overriding property must preserve fixed and const declaration
flags. Property types resolve before those checks; fixed agreement precedes const
agreement, and properties initialize before methods. Instance assignment and
property amendment reject inherited fixed properties without forcing their
initializers. Reference synthesis reads the same inherited metadata, including
hidden fixed properties, while referent bodies stay deferred. Abstract assignment
bodies and authored abstract amendments retain their distinct rules. Diagnostics
preserve enclosing/declaration frames, header ranges, and the pinned header
substring lookup for repeated modifier flag carries.

The [validation record](validation-stage11-final.json) and
[raw checks](validation-stage11-final-evidence.tar.gz) retain reference pin checks,
formatting, workspace Clippy, **251 passing Rust tests and 179 Python tests**,
Wasm compilation, and inventory freshness. One existing differential probe remains
ignored by ordinary unit-test execution and retains its separate adapter contract.
The [platform archive](platforms-stage11-final-evidence.tar.gz) records **45
scenarios in each of actual Node and Chromium**, including three new fixed-property
scenarios whose full expected values and diagnostics came from pinned Java at the
exact adapter URI. All 6,078 primitive results agree across Rust targets; each
matches Java on 6,076, preserving the same two regex failures. Three new native
lifecycle tests cover inherited fixed receiver dispatch, hidden fixed Reference
metadata, Host suspension, reentry, GC, failed reads and retries, stale
completions, cancellation, close, and deadlines.

The [regression archive](regressions-stage11-final-evidence.tar.gz) retains the
1,202 unchanged-input replays and fresh pre-push comparisons of all 89 additions.
Those exploratory frozen candidates remain separate from the recorded-build full
acquisition above. Existing bounded regressions retain 15 frontend matches, six
runtime value matches, two runtime diagnostic matches, one cycle diagnostic
mismatch, two matching lifecycle protocols, and four full lifecycle diagnostic
mismatches.

The [exploration archive](exploration-stage11-evidence.tar.gz) retains 111
independent-expression Java observations; the final exploratory candidate matches
100 and preserves **11 nonselected mismatches**. That count excludes the separate
persistent-evaluator and platform-URI protocols. A replay of the previous 191
Reference observations now matches 169 and retains 22 mismatches; `property-fixed`
changed from mismatch to match. Fourteen original fixed-related snippet
observations through selected API expressions do not execute their original
snippet, renderer, or assertion contracts. The exploratory candidate's source
binding remains unverified; its recorded source differs from the implementation
only by the later `fixed: false` field correction in
`crates/pkl-runtime/examples/lifecycle_probe.rs`. This limitation does not apply
to the separately verified recorded-build acquisition. Earlier candidates, an
interrupted actual-URI acquisition, corrected draft expectation metadata, and
caught upstream `PklBugException` results remain visible. The
[build-failure archive](exploration-stage11-build-failures-evidence.tar.gz)
separately retains the initial lifecycle compilation failure and the superseded
runtime acquisition; those records are not passing final validation.

Eleven [reviewed mappings](../../compatibility/m2-mappings.json) link partial
implementation and test scope to Rust symbols and selected cases. They preserve
all original upstream execution contracts and do not claim complete requirements.
The inventory has 474 requirement records and 2,291 overlapping accounting rows:
1,291 executable selections, 960 unimplemented rows, and 40 unmapped rows. These
are not distinct feature counts or a completion percentage. The
[gate captures](gates-stage11-final-evidence.tar.gz) retain integrity and
development success, strict selected/full compatibility failure, and 1,950 full
gate failure reasons. No new development exception was added.

The [implementation CI run](https://github.com/luuvish/pkl-rs/actions/runs/39543295843)
completed successfully across all six jobs. The [independent CI verification](ci-stage11-summary.json)
checks both original ZIPs against GitHub artifact digests and binds 1,615 source
snapshot copies, covering 1,418 unique paths, to implementation commit `52cabbe`.
Remote results retain 1,289 matches and the same two failures, 46 passing
relations, and passing development contracts for 45 scenarios in each of Node and
Chromium. The [CI archive](ci-stage11-evidence.tar.gz) preserves the original
artifacts and verification records. Two [offline verification passes](ci-stage11-offline-evidence.tar.gz)
passed without executing captured binaries; the final sealed package remained
unchanged.

Complete class publication/retry
semantics, alias identity and diagnostics, function types, strict Map/Set values,
general imports and module inheritance, reflection, and the original upstream
contracts remain open. The next priority is evaluator-owned class singleton
publication before superclass resolution, incremental member publication, and
GC/lifecycle retention of partial classes, followed by alias/function metadata and
strict Map/Set support. No new speed or memory parity measurement was made.
**M2 remains in progress.**

## Stage 10 checkpoint

The [stage 10 summary](stage10-final-summary.json) records **1,202 selected
comparisons: 1,200 raw-byte matches, one remainder-zero mismatch, and one cycle
oracle failure**. All 44 metamorphic relations pass. The 143 additions and three
new relations preserve every original 1,059 definition and input byte.
Implementation `adcfc44` is pushed. The
[language archive](language-stage10-final-evidence.tar.gz) retains a recorded
release build, 1,286 source snapshots, actual fixture URIs, pinned Java identity,
outcome sidecars, and complete output channels. Source and artifact identities
remained stable during acquisition, and every source snapshot matches `adcfc44`.

The shared engine recognizes `pkl:ref` imports and aliases, qualified types, and
class values separately from the `NonNull` type alias. Reference values retain
their domain, data, normalized referent type, and access path. Synthetic property
and subscript access consult inherited declaration metadata without evaluating
referent bodies. Selected cases cover alias and constraint erasure, preserved
integer aliases, nullable unions, domain/referent checks, equality, and structural
keys. General module loading and complete builtin metadata remain unfinished.

Diagnostics retain authored name and interpolation spans, alias validation
frames, and source identity for constructor, Access, and rendering errors. The
pinned `ref.pkl` source is bundled unchanged and included in source snapshots.
Four native Reference lifecycle tests cover domain rendering and constraints,
domain/data equality, Host suspension, nested evaluation, GC, failed reads and
retries, stale completions, cancellation, close, and timeout.

The [validation record](validation-stage10-final.json) and
[raw checks](validation-stage10-final-evidence.tar.gz) retain reference pin checks,
formatting, workspace Clippy, **236 Rust tests and 179 Python tests**, Wasm
compilation, and inventory freshness. One existing differential probe remains
intentionally ignored by ordinary unit tests. The
[platform archive](platforms-stage10-final-evidence.tar.gz) records **42 scenarios
in each of actual Node and Chromium**, including three new Reference scenarios.
Their value and complete diagnostic expectations come from fresh pinned Java
captures at the exact adapter URI. All 6,078 primitive results agree across Rust
targets; each target matches Java on 6,076, retaining the same two regex failures.

The [regression archive](regressions-stage10-final-evidence.tar.gz) retains the
1,059 unchanged-input replays and fresh pre-push comparisons of all 143 additions.
Those exploratory frozen candidates are separate from the recorded-build full
acquisition above. Existing bounded regressions retain 15 frontend matches, six
runtime value matches, two runtime diagnostic matches, one cycle diagnostic
mismatch, two matching lifecycle protocols, and four full lifecycle diagnostic
mismatches.

The [initial exploration archive](exploration-stage10-evidence.tar.gz) preserves
158 Java calls. The [supplemental archive](exploration-stage10-followup-evidence.tar.gz)
retains all 191 calls, earlier failures, invalid drafts, fixes, and provisional
replays. The final exploratory
candidate matches 168 of 191 observations and retains **23 nonselected
mismatches** in remaining language, type, and diagnostic scope. Some early debug
replays lack a retained candidate identity or complete source binding; they remain
provisional observations. Executing original `basic/reference.pkl`,
`errors/reference1..27.pkl`, or `api/reference.pkl` through selected API
expressions does not execute their original renderer or snippet-suite contracts.

Six [reviewed mappings](../../compatibility/m2-mappings.json) link partial
implementation and test scope to Rust symbols and selected cases. They preserve
all original case contracts and do not claim complete requirements. The inventory
has 474 requirement records and 2,202 overlapping accounting rows: 1,202 executable
selections, 960 unimplemented rows, and 40 unmapped rows. These are not distinct
feature counts or a completion percentage. The
[gate captures](gates-stage10-final-evidence.tar.gz) retain integrity and
development success, strict selected/full compatibility failure, and 1,950 full
gate failure reasons. General imports and module inheritance, remaining
collection/type rules, fixed-member and alias diagnostics, reflection, and the
original upstream contracts remain open. No new speed or memory parity
measurement was made. **M2 remains in progress.**

All six jobs in the [implementation CI run](https://github.com/luuvish/pkl-rs/actions/runs/39538489442)
for `adcfc44` succeeded. The [independent CI verification](ci-stage10-summary.json)
checks the original GitHub ZIP digests and 1,520 source snapshot copies (1,326
unique paths) against that exact commit. Remote results also retain 1,200
matches, the two original failures, 44 relations, and 42 scenarios on each
platform. The [CI evidence](ci-stage10-evidence.tar.gz) preserves those original
ZIPs, terminal job outcomes, and the first offline verification. A
[second offline replay](ci-stage10-offline-evidence.tar.gz) passed against the
sealed package without changing any package bytes. Strict selected/full gates
and the performance goals remain incomplete.

## Stage 9 checkpoint

The [stage 9 summary](stage9-final-summary.json) records **1,059 selected
comparisons: 1,057 raw-byte matches, one remainder-zero mismatch, and one cycle
oracle failure**. All 41 metamorphic relations pass. The 152 additions and nine
new relations preserve every original 907 definition and input byte. The
[language archive](language-stage9-final-evidence.tar.gz) retains the recorded
release build, 1,137 source snapshots, exact fixture URIs, pinned Java
identity, outcome sidecars, and complete output channels. Source and artifact
identities stayed stable, and every snapshot matches implementation `2a69a63`.

Stable Java hashes are separate from structural value identities. Declaration
maps retain insertion buckets; cached members retain insertion slots and partial
rehash state. A fifth cached value is published before its keys are hashed.
Failed rebuilding can hide a published slot, and retry can append another value
for its key. Both values remain visible to cursor comparisons and GC. Local
cache comparisons preserve direction; zero hashes are recomputed while nonzero
hashes remain cached. Mapping length separately uses ancestor-first HashSet
counting, including full-hash filtering, query-first equality, and success-only
publication of the count.

Let diagnostics use source metadata without adding dispatch instructions.
Selected cases cover nested scopes, captured objects/functions, initializer and
typed binding failures, explicit casts, and constraints with separate or inline
logical roots, including wildcard bindings. All ten held Stage 8 cases now
match, while repeated recursive frame compression remains a recorded gap.

The [validation record](validation-stage9-final.json) and
[raw checks](validation-stage9-final-evidence.tar.gz) retain reference pin checks,
formatting, workspace Clippy, **221 Rust tests and 171 Python tests**, Wasm
compilation, and inventory freshness. One existing differential probe remains
intentionally ignored by ordinary unit tests. Five map-index tests use pinned
JAR observations; four new Host lifecycle tests cover partial publication,
duplicate cursor values, zero hashes, generic checks, length counting, nested
evaluation, GC, failure/retry, stale completions, and terminal transitions.

The [platform archive](platforms-stage9-final-evidence.tar.gz) records **39
scenarios in each of actual Node and Chromium**. Twenty-five new Java captures
at the identical adapter URI supply values and complete diagnostic expectations.
All 6,078 primitive results agree across Rust targets; each target matches Java
on 6,076, retaining the same two regex failures. The
[regression archive](regressions-stage9-final-evidence.tar.gz) includes the final
907 unchanged-input replays and fresh pre-push comparison of all 152 additions.
It also retains 15 frontend matches, six runtime value matches, two runtime
diagnostic matches, one cycle diagnostic mismatch, two matching lifecycle
protocols, and four full lifecycle diagnostic mismatches.

The initial [exploration archive](exploration-stage9-evidence.tar.gz) preserves
185 language observations, internal pinned-JAR map/VM/class observations,
provisional replays, invalid drafts, and the first 137 promotions. Its final
provisional replay records 158 matches and 27 nonselected mismatches. Of the
158 matches, 21 are redundant controls. These observation and rerun
counts are not distinct language requirements.

The [correction archive](correction-stage9-evidence.tar.gz) preserves the
subsequent inherited Mapping length regression, including Java `1` versus Rust
`2`, 257 complete pairs and one interrupted case without a full report, and
superseded validation/platform captures. It adds the corrected Java/Rust `1`
result, 15 focused HashSet controls,
the final 200-row exploratory replay, and unchanged-input replay after the fix.
The final exploration has 173 matches and the same 27 nonselected mismatches;
all 152 promoted cases pass. The original archive remains immutable. Partial
captures and failed drafts are never counted as final compatibility passes.

The [gate captures](gates-stage9-final-evidence.tar.gz) retain integrity and
development success, with strict selected and full compatibility failure.
The inventory has 474 requirement records and 2,059 overlapping accounting rows:
1,059 executable selections, 960 unimplemented rows, and 40 unmapped rows.
These are not distinct feature counts or a completion percentage. The full gate
still reports 1,950 failure reasons. JVM identity-dependent bucket assignments,
same-cache cyclic reentry, cyclic keys, remaining types and amendments,
`Reference`, metadata, repeated-frame compression, and the full language corpus
remain open. No new speed or memory parity measurement was made.
**M2 remains in progress.**

All six jobs of the [implementation CI run](https://github.com/luuvish/pkl-rs/actions/runs/39529055834)
passed for `2a69a63`. The [independent verification](ci-stage9-summary.json)
and [CI package](ci-stage9-evidence.tar.gz) retain both original artifact ZIPs
and verify their GitHub SHA-256 digests. Verification matches
1,357 snapshot copies covering 1,177 distinct paths to the exact
implementation commit. Remote results retain all 1,059 comparisons, the same
two known failures, all 41 relations, all four identifier tables, and 39
scenarios per JS target. Two offline checks pass; the final
[sealed replay](ci-stage9-offline-evidence.tar.gz) reproduces outcomes and verifier
identities while the packaged contents remain unchanged.

## Stage 8 checkpoint

The [stage 8 summary](stage8-final-summary.json) records **907 selected
comparisons: 905 raw-byte matches, one remainder-zero mismatch, and one cycle
oracle failure**. All 32 metamorphic relations pass. The 33 additions and six
new relations preserve the original 874 definitions and input bytes. The
[language archive](language-stage8-final-evidence.tar.gz) retains a recorded
release build, 983 source snapshots, exact fixture URIs, Java identity,
outcome sidecars, and complete output channels. Source and artifact identities
remained stable; every source snapshot matches implementation commit `5024708`.

Ordinary typed properties named `default` now participate in forcing, equality,
and structural key preparation. Eligible collection members compare in successful
cache insertion order; typed objects compare in class declaration order, including
inheritance. Dynamic, Listing, Mapping, and typed key preparation finishes deep
forcing before recursively hashing member keys and values. Strict List preserves
sequential element hashing. Completed cache values survive retries, while failed
evaluations do not reserve an insertion position. These changes preserve the shared VM scheduler
across native, Wasm, and N-API.

The [validation record](validation-stage8-final.json) and
[raw checks](validation-stage8-final-evidence.tar.gz) retain reference pin checks,
formatting, workspace Clippy, **212 Rust tests and 171 Python tests**, Wasm
compilation, and inventory freshness. One existing differential probe remains
intentionally ignored by ordinary unit tests. Ten structural equality lifecycle
tests cover Host suspension, nested evaluation, GC, failed reads and retries,
stale completions, cancellation, close, and both timeout scopes. The new cases
also cross the deep-force/hash boundary.

The [platform archive](platforms-stage8-final-evidence.tar.gz) records **35
scenarios in each of actual Node and Chromium**. Java captures at the identical
module URI supply values and complete diagnostic expectations. All 6,078
primitive results agree across Rust targets; each target matches Java on 6,076,
retaining two regex failures. The [regression archive](regressions-stage8-final-evidence.tar.gz)
retains all 874 unchanged-input replays, the pre-push acquisition of 33 additions
at their actual fixture URIs, 15 frontend matches, six runtime value matches,
two runtime diagnostic matches, one cycle diagnostic mismatch, two matching
lifecycle protocols, and four full lifecycle diagnostic mismatches.

The [exploration archive](exploration-stage8-evidence.tar.gz) preserves 1,877
payload files, plus its evidence manifest. It contains 56 original Java/frozen
Stage 7 observations, two candidate replay rounds (52 and 56 captures), and
eight Java platform captures. The final exploratory replay has 46 raw matches
and ten retained mismatches: four cached-map fifth insertions, three warmed
Listing hash cases, one nested-let diagnostic frame, and two nonconstant local
cache cases. Platform captures include an additional observation of the held
nested-let error. Captures and repeated observations are not distinct language
requirements. Provisional replays remain separate from source-bound final evidence.
No failure is normalized, removed, or converted into a new development exception.

The [gate captures](gates-stage8-final-evidence.tar.gz) retain integrity and
development success, with strict selected and full compatibility failure. The
inventory has 474 requirement records and 1,907 overlapping accounting rows:
907 executable selections, 960 unimplemented rows, and 40 unmapped rows. These
are not distinct feature counts or a completion percentage. The full gate still
reports 1,950 failure reasons. Complete hash/cache behavior, cyclic keys, remaining
types and amendments, `Reference`, metadata, diagnostics, and the full language
corpus remain open. No new speed or memory parity measurement was made.
**M2 remains in progress.**

All six jobs of the [implementation CI run](https://github.com/luuvish/pkl-rs/actions/runs/39435577349)
passed for commit `5024708`. The [independent verification](ci-stage8-summary.json)
and [CI package](ci-stage8-evidence.tar.gz) retain both original artifact ZIPs
and verify their GitHub SHA-256 digests. Verification matched
1,198 snapshot copies covering 1,023 distinct paths to the exact
implementation commit. Remote results retain 905 matches, the same two known
failures, all 32 relations, all four identifier tables, and 35 scenarios per JS
target. Two offline checks passed; the final [sealed replay](ci-stage8-offline-evidence.tar.gz)
verified unchanged package contents and reproduced outcomes and verifier identities.

## Stage 7 checkpoint

The [stage 7 summary](stage7-final-summary.json) records **874 selected
comparisons: 872 raw-byte matches, one remainder-zero mismatch, and one cycle
oracle failure**. All 26 metamorphic relations pass. The 201 additions preserve
the original 673 definitions and input bytes; 44 cases use explicit power
assertions. The [language archive](language-stage7-final-evidence.tar.gz) retains
the recorded release build, 950 source snapshots, exact fixture URIs,
oracle identity, outcome sidecars, and complete output channels. Source and
artifact identities remained stable throughout acquisition.

Structural comparisons distinguish scalar numeric coercion from boxed nested
values, including NaN and signed zero. Identity, object kind, class identity,
and Listing length checks preserve their forcing shortcuts. Dynamic, Mapping,
and Typed comparisons deep-force in the pinned order; strict List comparisons
stop at the first unequal element. The resumable key path covers construction,
lookup, duplicates, inherited overrides, amendments, and Mapping length. Small
declaration maps keep keys lazy until comparison requires them; adding a fifth
distinct member prepares hashes in declaration order.

Constant scalar and nested List keys receive early duplicate checks only when
all keys in a specialized body are constant. Generators, dynamic keys, and
shadowed constructors retain runtime behavior. Signed integer expressions keep
separate numeric-token spans. Function diagnostics retain arity. Source-line
rendering follows the pinned highlighter's interpolation whitespace behavior,
with lexical-error rollback and authored caret coordinates. Its explicit
traversal stack avoids recursive Rust calls on deeply nested malformed lines.

The [validation record](validation-stage7-final.json) and
[raw checks](validation-stage7-final-evidence.tar.gz) retain reference pin
verification, formatting, workspace Clippy, **210 Rust tests, 171 Python tests**,
Wasm compilation, and inventory freshness. One existing differential probe
remains intentionally ignored by ordinary unit tests. Nineteen new Rust tests
cover frontend scopes and token spans, deferred duplicates, temporary operands
and keys, Host suspension and reentry, GC, failure retry, cancellation, close,
timeouts, and renderer traversal, including a 20,000-level malformed input.
The [identifier archive](identifier-stage7-evidence.tar.gz) verifies all four
unchanged tables against pinned Java.

The [platform archive](platforms-stage7-final-evidence.tar.gz) records **32
scenarios in each of actual Node and Chromium**. New scenarios cover structural
values, keys, complete errors, and power assertions. All 6,078 primitive results
agree across Rust targets; each target matches Java on 6,076, retaining two regex
failures. The [M1 regressions](regressions-stage7-final-evidence.tar.gz) retain
15 frontend matches, six runtime value matches, two complete runtime diagnostic
matches, one cycle diagnostic mismatch, two matching lifecycle protocols, and
four failed full lifecycle diagnostics. The archive also retains the replay of
unchanged inputs against prior Java captures; the language archive separately
contains the fresh Java acquisition. No new speed or memory parity measurement
was made.

The [exploration archive](exploration-stage7-evidence.tar.gz) preserves 14,196
payload files and 1,502 raw Java captures: 281 exploratory captures and 1,221 repeated
actual-fixture URI captures. The exploratory captures reconcile to 253 curated
observations, 19 superseded inputs, and nine repeated review captures. Curated
observations comprise 201 selected additions, five held main inputs, nine held
renderer inputs, 28 redundant unselected inputs, and ten platform inputs.
All provisional Rust replays remain explicitly unbound to a recorded build.
The final provisional replay matches all 201 additions and nine curated renderer
probes; full selected evidence comes from the separate recorded-build archive.
Two renderer character predicates were exhaustively checked against pinned Java
for all 65,536 UTF-16 units, with raw output and probe sources retained.

Three held cyclic structural hash/equality inputs produce pinned Java
`PklBugException` outcomes; two held Dynamic hidden-modifier inputs are invalid
upstream. The superseded renderer `@` probe retains an unrelated parser
diagnostic difference. Corrected inputs and earlier failures are preserved
separately. Manual captures with missing process metadata remain unclassified.
None of these observations becomes a passing selection or a development
exception.

Complete Java hash-bucket collision order, cached-member map transitions,
typed properties named `default`, cyclic structural keys, full custom defaults,
function/class-valued amendments, remaining typed generators and function/
structural types, `Reference`, metadata, format-time wrapper checks, complete
diagnostics, and the full language corpus remain open.

The [gate captures](gates-stage7-final-evidence.tar.gz) retain integrity and
development success with strict selected and full compatibility failure.
The inventory has 474 requirement records and 1,874 overlapping accounting rows:
874 executable selections, 960 unimplemented rows, and 40 unmapped rows. These
are not distinct feature counts or a completion percentage. The full gate still
reports 1,950 failure reasons. **M2 remains in progress.**

All six jobs of the [implementation CI run](https://github.com/luuvish/pkl-rs/actions/runs/39427102753)
passed for commit `90f7df4`. The
[independent verification](ci-stage7-summary.json) and
[CI package](ci-stage7-evidence.tar.gz) retain both original artifact ZIPs and
verify their GitHub SHA-256 digests. Verification matched 1,165 snapshot copies
covering 990 distinct source paths to the exact implementation commit.
Remote language results retain 872 matches and the same two known failures,
all 26 relations, all four identifier tables, and 32 scenarios per JS target.
Two offline checks passed; the final [sealed replay](ci-stage7-offline-evidence.tar.gz)
verified unchanged package contents and reproduced comparison outcomes and
verifier identities. M1's existing failures remain visible.

## Stage 6 checkpoint

The [stage 6 summary](stage6-final-summary.json) records **673 selected
comparisons: 671 raw-byte matches, one remainder-zero mismatch, and one cycle
oracle failure**. All 23 metamorphic relations pass. The 113 additions preserve
the original 560 definitions and input bytes; 39 cases use explicit power
assertions. The [language archive](language-stage6-final-evidence.tar.gz) retains
the recorded release build, 744 source snapshots, exact fixture URIs,
oracle identity, outcome sidecars, and complete output channels. Source and
artifact identities remained stable throughout acquisition.

Member predicates now select parent elements and entries for lazy replacement
or amendment. Traversal visits child declarations first and evaluates each value
immediately before its condition, using the declaration owner's cache. Properties,
locals, and newly declared target members are excluded. Nested predicates restore
custom `this`; generator locals and lexical captures remain available to lazy
bodies. Const checks distinguish immediate predicate lookup from closure lookup.
Selected elements retain their element kind and index, while diagnostics retain
the authored predicate label and condition span. Reached typed predicates and
primitive/List amendments preserve the selected rejection order.

The [validation record](validation-stage6-final.json) and
[raw checks](validation-stage6-final-evidence.tar.gz) retain reference pin
verification, formatting, workspace Clippy, **191 Rust tests, 171 Python tests**,
Wasm compilation, and inventory freshness. One existing differential probe
remains intentionally ignored by ordinary unit tests. Nineteen new Rust tests
cover frontend scopes, forcing order, nested captures, temporary parents, Host
suspension and reentry, GC, failure retry, cancellation, close, and timeouts.
The [identifier archive](identifier-stage6-evidence.tar.gz) verifies all four
unchanged tables against pinned Java.

The [platform archive](platforms-stage6-final-evidence.tar.gz) records **29
scenarios in each of actual Node and Chromium**. New scenarios cover predicate
values, nested custom receivers, declaration-owner caches, and complete errors
and power assertions. All 6,078 primitive results agree across Rust targets;
each target matches Java on 6,076, retaining two regex failures. The
[M1 regressions](regressions-stage6-final-evidence.tar.gz) retain 15 frontend
matches, six runtime value matches, two complete runtime diagnostic matches,
one cycle diagnostic mismatch, two matching lifecycle protocols, and four
failed full lifecycle diagnostics. The same archive preserves the pre-push
replay of unchanged inputs against prior Java captures; the full language
archive separately contains the fresh Java acquisition. No new speed or memory
parity measurement was made.

The [exploration archive](exploration-stage6-evidence.tar.gz) preserves 2,802
files: 127 language observations, six platform Java observations, preliminary
candidate replays, and fresh actual-URI Java captures for all 113 additions.
Provisional replays remain explicitly unbound to a recorded build. Eight held
cases and six superseded exploratory inputs remain visible. These include
unimplemented compound-value stringification and modifier/diagnostic paths,
a pinned Java null-pointer failure, and the original erroneous amendment probes
that produced upstream stack-overflow initialization failures. Corrected inputs
were acquired separately; their earlier bytes were not replaced.

Structural object keys and equality, function/class-valued amendments, full
custom defaults, remaining typed generator rules, structural/function types,
`Reference`, metadata, format-time wrapper checks, and the full language corpus
remain open. No new development exceptions were added. The M2 CI job allowance
increased from 20 to 30 minutes for the larger serial Java corpus; per-case
execution limits and compatibility verdicts are unchanged.

The [gate captures](gates-stage6-final-evidence.tar.gz) retain integrity and
development success with strict selected and full compatibility failure. The
inventory has 474 requirement records and 1,673 overlapping accounting rows:
673 executable selections, 960 unimplemented rows, and 40 unmapped rows. These
are not distinct feature counts or a completion percentage. The full gate still
reports 1,950 failure reasons. **M2 remains in progress.**

All six jobs of the [implementation CI run](https://github.com/luuvish/pkl-rs/actions/runs/39417418349)
passed for commit `ca751a1`. The
[independent verification](ci-stage6-summary.json) and
[CI package](ci-stage6-evidence.tar.gz) retain both original artifact ZIPs and
verify their GitHub SHA-256 digests. Verification matched 945 snapshot
copies covering 784 distinct files to the exact implementation
commit. Remote language results retain 671 matches and the same two known
failures, all 23 relations, all four identifier tables, and 29 scenarios per
JS target. An [offline replay](ci-stage6-offline-evidence.tar.gz) verifies the
sealed package without changing its contents; M1's existing failures remain
visible.

## Stage 5 checkpoint

The [stage 5 summary](stage5-final-summary.json) records **560 selected
comparisons: 558 raw-byte matches, one remainder-zero mismatch, and one cycle
oracle failure**. All 20 metamorphic relations pass. The 145 additions preserve
the original 415 definitions and input bytes; 37 cases use explicit power
assertions. The [language archive](language-stage5-final-evidence.tar.gz) retains
the recorded release build, 627 source snapshots, actual fixture URIs,
oracle identity, outcome sidecars, and full output channels. Source and artifact
identities stayed stable throughout acquisition.

The shared runtime now checks `Listing<T>`, `Mapping<K,V>`, and `List<T>`.
Listing values remain lazy; Mapping keys are checked eagerly; strict List
members retain nested wrappers returned by their checks. Selected type tests,
unions, aliases, defaults, inferred constructors, inherited casts, and cache
reuse follow pinned Java evaluation order. Function parameters retain the
checked value. Constraint reuse follows compiled node identity, keeping separate
generic alias instantiations distinct. Union errors preserve use-site labels
and expanded check locations independently.

The [validation record](validation-stage5-final.json) and
[raw checks](validation-stage5-final-evidence.tar.gz) retain reference pin
verification, formatting, workspace Clippy, **172 Rust tests, 171 Python tests**,
Wasm compilation, and manifest freshness. One existing differential probe remains
intentionally ignored by ordinary unit tests. Nine new Rust frontend/lifecycle
tests exercise temporary receivers, nested checked results, parameter bindings,
cached failure retry, Host suspension, nested evaluation, GC, cancellation,
close, and timeouts. A constraint's eager checking mode remains active through
Host reentry and ends when its parent check completes. The
[identifier archive](identifier-stage5-evidence.tar.gz) verifies all four
unchanged tables against pinned Java.

The [platform archive](platforms-stage5-final-evidence.tar.gz) records **26
scenarios in each of actual Node and Chromium**. New scenarios include lazy
collection values, strict List wrapping, eager key errors, and full power
assertions. All 6,078 primitive results agree across Rust targets; each target
matches Java on 6,076, retaining two regex failures. The
[M1 regressions](regressions-stage5-final-evidence.tar.gz) retain 15 frontend
matches, six runtime value matches, two complete runtime diagnostic matches,
one cycle diagnostic mismatch, two matching lifecycle protocols, and four
failed full lifecycle diagnostics. No new speed or memory parity measurement
was made.

The [exploration archive](exploration-stage5-evidence.tar.gz) preserves 5,028
files: Java studies, initial mismatches, provisional candidate replays, actual
fixture-URI captures for all 145 additions, and platform expectations. Preliminary
replays remain explicitly unbound to a recorded build. Some first independent
captures lack complete process metadata; formal captures reacquire every
promoted case with complete identities and outcomes. The final full acquisition
is the source-bound validation of the committed implementation.

Unselected observations remain visible. Java can steal a parent's cache and
execute a wrapper cast while formatting a diagnostic; the Rust formatter does
not yet perform these checks. Some such Java failures escape diagnostic
rendering entirely. A deferred Java cast referencing method-local slots can
also fail inside the JVM. Those observations, and the remaining unsupported
outer/String property probes, are preserved without new development exceptions.
Full custom default functions, member predicates, object keys and structural
equality, typed constructor generators, structural/function types, `Reference`,
metadata, complete diagnostics, and the full language corpus remain open.

The [gate captures](gates-stage5-final-evidence.tar.gz) retain integrity and
development success alongside strict selected and full compatibility failure.
The inventory has 474 requirement records and 1,560 overlapping accounting rows:
560 executable selections, 960 unimplemented rows, and 40 unmapped rows. These
are not distinct feature counts or a completion percentage. The full gate still
reports 1,950 failure reasons. **M2 remains in progress.**

All six jobs of the [implementation CI run](https://github.com/luuvish/pkl-rs/actions/runs/39411357487)
passed for commit `3a70e03`. The
[independent verification](ci-stage5-summary.json) and
[CI package](ci-stage5-evidence.tar.gz) preserve both original artifact ZIPs and
verify their GitHub SHA-256 digests. Verification matched 824 snapshot
copies covering 667 distinct files to the exact implementation
commit. Remote language results retain 558 matches and the same two known
failures, all 20 relations, all four identifier tables, and 26 scenarios per
JS target. An [offline replay](ci-stage5-offline-evidence.tar.gz) verifies the
sealed package without changing its contents; M1's existing failures remain
visible.

## Stage 4 checkpoint

The [stage 4 summary](stage4-final-summary.json) records **415 selected
comparisons: 413 raw-byte matches, one remainder-zero mismatch, and one cycle
oracle failure**. All 20 metamorphic relations pass. The 121 new selections
extend the unchanged 294-case baseline; 29 cases now use explicit power-assertion
profiles. The [language archive](language-stage4-final-evidence.tar.gz) contains
the recorded release build, 480 source snapshots, exact inputs and commands,
oracle identity, outcome sidecars, and complete output channels. Source and
artifact identities remained stable during acquisition.

The shared runtime now constructs ordered properties, entries, and elements for
`Dynamic`, `Listing`, and `Mapping`, with indexing, amendments, `for`, `when`,
spread, and strict `List` construction. Lazy bodies retain generator variables
and the eventual receiver. Source iteration first shallow-forces members in the
pinned child-first order, then visits inherited positions. Static and generated
object bodies preserve their distinct ordering and collision behavior.
Primitive keys distinguish integers, floating-point values, and signed zeros,
while canonicalizing NaNs. Tagged cache namespaces prevent literal NUL
characters in quoted properties from colliding with internal entry names.

The [validation record](validation-stage4-final.json) and
[raw checks](validation-stage4-final-evidence.tar.gz) retain reference pin
verification, formatting, workspace Clippy, **163 Rust tests, 171 Python tests**,
Wasm runtime compilation, and inventory freshness. One existing probe remains
intentionally ignored by ordinary unit tests and executes through its
differential adapter. Collection lifecycle tests exercise unpublished builders,
temporary iterables, captured closures, nested Host reentry, active and suspended
GC roots, retry, cancellation, close, timeouts, and late callback rejection.
The [identifier archive](identifier-stage4-evidence.tar.gz) repeats the pinned
Java verification of all four unchanged identifier tables.

The [platform archive](platforms-stage4-final-evidence.tar.gz) records **23
scenarios in each of actual Node and Chromium**, including collection values,
amendments, generator closures, bounds errors, and complete power-assertion
graphs. All 6,078 primitive results agree across Rust targets; each target matches
Java on 6,076. The two known regex failures remain recorded. The
[M1 regressions](regressions-stage4-final-evidence.tar.gz) retain 15 frontend
matches, six runtime value matches, two full runtime diagnostic matches, one
cycle diagnostic mismatch, two matching lifecycle protocols, and four failed
full lifecycle diagnostic comparisons. No new speed or memory parity
measurement was made.

The [exploration archive](exploration-stage4-evidence.tar.gz) preserves 7,942
files, including the initial mismatches and their corrections, pinned Java
studies, actual fixture-URI captures, platform expectations, and source
snapshots. Preliminary candidate replays remain explicitly unbound to a
recorded build. The independent review's first captures lack full process
identity metadata; that limitation is retained, and its promoted regressions
were reacquired through the formal runner. Reviews corrected spread captures,
source forcing order, typed/module classification, local duplicate definitions,
constructor name resolution, inherited entry bodies, and diagnostic frames.

The [gate captures](gates-stage4-final-evidence.tar.gz) retain integrity and
development success, with strict selected and full compatibility failure.
The inventory has 474 requirement records and 1,415 accounting rows: 415
executable selections and the unchanged 960 unimplemented and 40 unmapped
upstream/historical obligations. These overlapping rows are not distinct feature
counts. The full gate still reports 1,950 failure reasons.

Generic collection wrappers, member predicates, object keys and structural
equality, complete custom-default behavior, typed constructor generators, and
the remaining collection builtins are unfinished. Function/structural types,
`Reference`, metadata, complete diagnostics, and the full language corpus also
remain M2 requirements. No new development exceptions were added.

All six jobs of the [implementation CI run](https://github.com/luuvish/pkl-rs/actions/runs/39403541822)
passed for commit `4512488`. The
[independent verification record](ci-stage4-summary.json) and
[complete CI evidence package](ci-stage4-evidence.tar.gz) retain both original
artifact ZIPs and their verified GitHub SHA-256 digests. Verification matched
675 snapshot copies, covering 520 distinct files, to that exact commit.
Remote language results also contain 413 matches, one remainder mismatch, and
one cycle oracle failure. All 20 relations, four identifier tables, and 23
scenarios per JS target verify. The original regex, runtime cycle, and lifecycle
diagnostic failures remain visible in the M1 artifact.
An [offline replay of the sealed package](ci-stage4-offline-evidence.tar.gz)
passed without changing its contents and reproduced the verified outcomes and
verifier hashes. Its raw process output is retained separately to avoid changing
the package being verified.

## Stage 3 checkpoint

The [final stage 3 summary](stage3-final-summary.json) records **294 selected
comparisons: 292 raw-byte matches and two mismatches**, for the Java
remainder-zero bug and cyclic property diagnostics. All 17 metamorphic relations pass. The selection
includes 23 explicit power-assertion profiles; the other cases retain the
Java API's disabled default. The [language archive](language-stage3-final-evidence.tar.gz)
contains the recorded release build, complete source snapshots, oracle identity,
exact commands and inputs, outcome sidecars, and full stdout/stderr. Sources and
artifacts stayed stable during acquisition.

The shared VM now executes Boolean and function-valued constraints as resumable
continuations. The checks cover base-first evaluation, nullable wrapper order,
union alternatives, casts and type tests, captured receivers, inherited
declaration owners, and alias receiver changes that preserve frame locals.
Pipelines use ordinary function calls with the pinned precedence, evaluation
order, arity checks, and authored diagnostics.

Power assertions use an explicitly instrumented compilation. The ordinary
compilation does not add observation instructions. Failure construction retains
the pinned conditional re-execution order and renders complete value graphs,
including grouping, multiline expressions, comments, and UTF-16 columns.
Diagnostic value snapshots do not force members; they preserve inherited
declaration order, completed caches, Pkl string escapes, and UTF-16 truncation,
including the Java writer's replacement of a split surrogate.

The [validation record](validation-stage3-final.json) and
[raw checks](validation-stage3-final-evidence.tar.gz) contain reference verification,
formatting, workspace Clippy, **140 Rust tests, 170 Python tests**, Wasm runtime
compilation, and inventory freshness. One existing Rust probe is intentionally
ignored by the unit suite and runs through its differential adapter. Constraint
lifecycle tests exercise Host suspension, nested evaluation, GC, cancellation,
failure retry, successful caching, union replay order, pending close, ancestor
timeout, resume-time timeout, and rejection of late callbacks. These unit
protocols do not establish the complete Java Host/resource API contract.

The [platform archive](platforms-stage3-final-evidence.tar.gz) records **20 scenarios
in each of actual Node and Chromium**, including constraint failures with power
assertions both disabled and enabled through the shared boundary. All 6,078
primitive results agree across Rust targets; each target matches Java on 6,076.
The two known regex differences remain failures. The
[M1 regression archive](regressions-stage3-final-evidence.tar.gz) retains 15 frontend
matches, six runtime value matches, two full runtime diagnostic matches, the
unresolved cycle diagnostic, two matching lifecycle protocols, and four failed
full lifecycle diagnostic comparisons. No new performance parity claim is made.

The [identifier evidence](identifier-stage3-evidence.tar.gz) verifies the
unchanged UTF-16 lexer intervals and additional full code-point intervals for
diagnostic identifier quoting. A supplementary character can require quoting
in source while the pinned diagnostic renderer displays it unquoted.

The [gate captures](gates-stage3-final-evidence.tar.gz) retain integrity and development
success, with strict selected and full compatibility failure. The inventory has
474 requirement records and 1,294 accounting rows: 294 executable selections
and 1,000 pending upstream/historical obligations. These overlap and are not
distinct feature counts. The full gate retains 1,950 failure reasons.

The [exploration archive](exploration-stage3-evidence.tar.gz) preserves Java
specimens, intermediate Rust mismatches, and independent adversarial reviews.
Those exploratory directories do not claim a frozen whole-source acquisition.
Confirmed inherited-owner, generic-alias receiver, grouped-source, method-frame,
pipeline object-value, and inherited fatal-frame defects were corrected and
added to the final selection. A review also retained Java alias JVM bugs and
unsupported Dynamic-method diagnostic differences outside that selection;
they are unresolved scope, without new development exceptions.

The [final inherited-constraint review](constraint-review-stage3-evidence.tar.gz)
retains four pre-fix mismatches and six corrected raw comparisons. Invalid direct
predicate results must preserve both the declaration and overriding value frames;
invalid function results preserve the value frame without inventing a function
frame. The VM now keeps the checking continuation available during both fatal
error paths. Two representative fixtures were added to the selection; null and
power-mode variants were also checked. Repeated failed checks do not publish a
cached value.

The [earlier stage 3 checkpoint](stage3-summary.json) and its
[language](language-stage3-evidence.tar.gz),
[validation](validation-stage3-evidence.tar.gz),
[platform](platforms-stage3-evidence.tar.gz),
[regression](regressions-stage3-evidence.tar.gz), and
[gate](gates-stage3-evidence.tar.gz) archives are retained separately. Its 292-case
selection had 290 matches, a remainder mismatch, and a cycle oracle failure.
In the final cold Java acquisition, the cycle emitted a Pkl stack-overflow
diagnostic with 697 repetitions; it still differs from Rust. The oracle was not
preinitialized or normalized. This observed variation does not resolve the
cycle requirement.

All six jobs of the [final stage 3 CI run](https://github.com/luuvish/pkl-rs/actions/runs/39396439537)
passed for implementation commit `5d58e76`. The
[independent verification record](ci-stage3-summary.json) and
[complete evidence package](ci-stage3-evidence.tar.gz) retain both original
GitHub artifact ZIPs, their verified SHA-256 digests, metadata, and repeatable
checks. Verification matched 541 snapshot copies, covering 394 distinct source
files, to that exact commit; the language acquisition contains 354 source files.
The ZIPs retain 3,331 payload files in total.

Remote language results are 292 matches, one remainder mismatch, and one cycle
oracle failure across the same 294 selections. All 17 relations and the four
identifier tables verify. The M1 artifact independently preserves the same
20 scenarios per JS target, primitive comparisons, frontend/runtime results,
and known lifecycle diagnostic failures. The full completion gate remains
failed. The evidence package includes instructions for checking these records
without rerunning captured binaries. A captured offline rerun of the unchanged
package succeeded and reproduced all comparison outcomes, artifact identities,
source counts, and verifier hashes; its raw process output is also retained.

## Stage 2 checkpoint

The [stage 2 summary](stage2-summary.json) records **191 selected comparisons:
189 raw-byte matches, one Java remainder-zero bug mismatch, and one cyclic
property oracle failure**. All 12 metamorphic relations pass. The
[language archive](language-stage2-evidence.tar.gz) preserves the exact candidate,
source snapshots, oracle identity, commands, input files, sidecar exception
outcomes, and full stdout/stderr. Sources and artifacts stayed stable throughout
acquisition.

Classes share the existing traced arena and prototype chain. The implementation
now checks inherited property types, class initialization timing, superclass
calls, const captures, abstract member forcing, alias declaration scope, generic
substitution, and type defaults. Explicit module names and source locations flow
through the same diagnostic boundary in native, Node, and Wasm. The generated
identifier tables reproduce the pinned Java lexer's UTF-16 code-unit predicates;
[their raw intervals and regeneration evidence](identifier-stage2-evidence.tar.gz)
are retained.

The [development checks](validation-stage2.json) and
[raw validation captures](validation-stage2-evidence.tar.gz) record reference
verification, formatting, Clippy, **111 Rust tests, 164 Python tests**, Wasm
compilation, inventory freshness, and Java identifier regeneration. One existing
Rust probe is intentionally ignored by ordinary unit tests and executes through
the differential runner. The [gate captures](gates-stage2-evidence.tar.gz) show
integrity and development success, with strict selected/full compatibility
failure.

The [platform archive](platforms-stage2-evidence.tar.gz) records **18 scenarios
in each of actual Node and Chromium**, including full class and arithmetic
diagnostics through the shared boundary. All 6,078 primitive results agree
across Rust targets; each target matches Java on 6,076. The two known regex
differences remain failed compatibility cases.

The [M1 regression archive](regressions-stage2-evidence.tar.gz) retains 15 frontend
matches, six runtime value matches, and two newly matching full runtime
diagnostics. The runtime cycle diagnostic still differs. The two lifecycle
observation protocols still match, while their four full diagnostic differences
remain failed comparisons. No new speed or memory parity claim is made.

The stage 2 inventory had 474 requirement records and 1,191 accounting rows:
191 executable selections and 1,000 pending upstream/historical obligations.
Declarations and historical rows overlap; these are not distinct feature
counts. The 1,950 full-gate reasons may include multiple reasons for one
requirement.

All six jobs of the [stage 2 CI run](https://github.com/luuvish/pkl-rs/actions/runs/39386841661)
passed for implementation commit `281ef6c`. The
[downloaded M2 verification record](ci-stage2-summary.json) and
[complete artifact archive](ci-stage2-evidence.tar.gz) verify the GitHub ZIP
digest, all 1,790 language evidence files, 246 source snapshots against the
exact commit, raw comparison outcomes, metamorphic relations, and generated
identifier bytes. Remote results retain the same two failed language cases.
The separate [M1 artifact verification](ci-stage2-m1-summary.json) and
[raw archive](ci-stage2-m1-evidence.tar.gz) independently recheck 167 committed
snapshot copies, all primitive comparisons, 18 scenarios per JS target, and
frontend/runtime/lifecycle outcomes. Known regex, cycle, and lifecycle diagnostic
differences remain failures in those records.

## Preserved corrections and oracle limitations

The [first stage 2 acquisition](language-stage2-initial-evidence.tar.gz) recorded
165 matches, 12 mismatches, and one oracle failure across 178 cases
([summary](stage2-initial-summary.json)). After diagnostic corrections, a
[184-case acquisition](language-stage2-corrections-evidence.tar.gz) recorded
181 matches and three mismatches
([summary](stage2-corrections-summary.json)). A reserved-keyword fixture was kept
as its own parser case when a legal unknown-property case was added.

An independent [frontend/modifier review](frontend-stage2-review-evidence.tar.gz)
and [follow-up](frontend-stage2-followup-evidence.tar.gz) retained false successes
for abstract defaults and repeated modifiers, plus abstract method, class-name,
and missing-body diagnostics. Corrections and counterexamples are included in
the latest selection. The review's frozen candidate is retained in the
184-case archive. Hidden/fixed support remains explicitly unimplemented.

Integer `1 % 0` produces a caught upstream `PklBugException`. The
[remainder investigation](remainder-summary.json) and
[three repeated raw captures](remainder-evidence.tar.gz) establish its
reproducibility under the pinned oracle. Each new acquisition also takes an
independent cold Java capture and requires all error bytes to agree for the
narrow development exception. The Rust/Java comparison remains a mismatch.
No stack trace is normalized.

## Stage 1 record

The [stage 1 summary](stage1-summary.json) records 119 selected Java/Rust
comparisons: 118 raw-byte matches and one unresolved Java cyclic-property
diagnostic failure. All 12 metamorphic relations passed in both runtimes. Sixteen
seeded malformed-input cases are included in the selected count.

The [language archive](language-stage1-evidence.tar.gz) retains build output,
compiler identity, source snapshots, frozen artifacts, exact inputs, process
commands, and raw stdout/stderr. Source and artifact identities remained stable
during acquisition. The strict selected and full gates fail; only the explicitly
limited development contract passes.

The stage 1 inventory had 474 requirement records and 1,119 accounting rows. Of those
rows, 119 have the initial executable adapter. The remaining 1,000 retain
upstream snippet obligations and historical candidates/contracts. Some
declarations and historical rows overlap; these are not distinct feature or
test counts. The 1,949 full-gate failure reasons can include multiple reasons
for one requirement.

The first acquisition is retained in
[the initial archive](language-initial-evidence.tar.gz): 103 matches, 11
diagnostic mismatches, and one oracle failure. After corrections, a second run
observed 114 matches and one oracle failure, but verification detected a tool
edit during acquisition. Its [inconclusive archive](language-inconclusive-evidence.tar.gz)
is preserved and is not accepted compatibility evidence.

The [cycle investigation](cycle-investigation.json) and
[complete repeat](cycle-complete.json), with their
[original](cycle-evidence.tar.gz) and [repeat](cycle-complete-evidence.tar.gz)
raw archives, distinguish a cold Java class-initialization failure from a
separate diagnostic preinitialization control. Preinitialization produces
controlled Pkl errors but their repetition counts vary. The normal oracle and
its raw error bytes are unchanged.

[Local validation](validation-stage1.json) and
[its captures](validation-stage1-evidence.tar.gz) record reference verification,
formatting, Clippy, 78 Rust tests, 156 Python tooling tests, Wasm compilation,
inventory freshness, and the development contract. One existing Rust frontend
probe is intentionally ignored by ordinary unit-test execution and is invoked
by its differential runner. Strict selected/full failures remain compatibility
failures, even though their expected exit codes are checked.

The [platform archive](platforms-stage1-evidence.tar.gz) records actual Node and
Chromium execution. Sixteen scenarios pass in each JS environment, including
new recursive-method, closure, control-flow, and interpolation cases. All 6,078
primitive results agree across Rust targets; each target matches Java on 6,076,
with the two existing regex differences preserved. All six jobs of the
[stage 1 CI run](https://github.com/luuvish/pkl-rs/actions/runs/39382215737)
passed. The [CI verification record](ci-stage1-summary.json) and
[downloaded artifact evidence](ci-stage1-evidence.tar.gz) verify the archive
digest, raw results, and source snapshots against the exact commit.

## Remaining milestone scope

Known remaining scope includes complete class initialization/retry and modifier
semantics, imported and structural/function types, complete constraint/generic
and collection-wrapper semantics,
collection members and generators,
`Reference`, reflection metadata, full source grammar, compound values,
complete error selection/rendering, and the upstream semantic corpus.
Floating power is not yet proven equivalent to Java's numeric behavior.
No complete language/API or speed/memory parity claim is made.

Verify archives with `python3 tools/evidence.py verify <archive>`. See
[the M2 contract](../../docs/m2-language.md) for fresh acquisition and strict gate
commands, and [PLAN.md](../../PLAN.md) for the unchanged milestone exit criteria.
