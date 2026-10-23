# M2 language evidence

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
are provenance, not distinct semantic cases. Full selected/lifecycle regression
is running separately at combined commit `01971d0`; no result from that run is
claimed here. Hosted CI, full compatibility, and performance remain unverified.

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
