# M2 language evidence

M2 is **in progress**. Stage 1 added expressions, strings, recursive methods,
closures, primitive type checks, and source diagnostics. Stage 2 adds selected
class, inheritance, generic alias, default-value, and modifier semantics to the
shared engine. Neither stage completes the language milestone.

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

The current inventory has 474 requirement records and 1,191 accounting rows:
191 executable selections and 1,000 pending upstream/historical obligations.
Declarations and historical rows overlap; these are not distinct feature
counts. The 1,950 full-gate reasons may include multiple reasons for one
requirement.

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
semantics, imported and structural/function types, constraints and full generics,
collection members and generators,
`Reference`, reflection metadata, full source grammar, compound values,
complete error selection/rendering, and the upstream semantic corpus.
Floating power is not yet proven equivalent to Java's numeric behavior.
No complete language/API or speed/memory parity claim is made.

Verify archives with `python3 tools/evidence.py verify <archive>`. See
[the M2 contract](../../docs/m2-language.md) for fresh acquisition and strict gate
commands, and [PLAN.md](../../PLAN.md) for the unchanged milestone exit criteria.
