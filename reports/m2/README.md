# M2 language evidence

M2 is **in progress**. Stage 1 added expressions, strings, recursive methods,
closures, primitive type checks, and source diagnostics. Stage 2 adds selected
class, inheritance, generic alias, default-value, and modifier semantics to the
shared engine. Stage 3 adds pipelines, resumable constraints, and optional power
assertions. Stage 4 adds ordered collections and resumable generators. Stage 5
adds generic collection checks and lazy wrappers. Stage 6 adds member predicates
and their scope, cache, and lifecycle behavior. Stage 7 adds structural equality,
nonprimitive keys, and related diagnostics. These checkpoints do not complete the
language milestone.

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
