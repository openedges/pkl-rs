# M2 language evidence

M2 is **in progress**. Stage 1 adds expressions, strings, recursive methods,
closures, primitive type checks, and source diagnostics to the shared engine.
It does not complete the language milestone.

The [stage 1 summary](stage1-summary.json) records 119 selected Java/Rust
comparisons: 118 raw-byte matches and one unresolved Java cyclic-property
diagnostic failure. All 12 metamorphic relations passed in both runtimes. Sixteen
seeded malformed-input cases are included in the selected count.

The [language archive](language-stage1-evidence.tar.gz) retains build output,
compiler identity, source snapshots, frozen artifacts, exact inputs, process
commands, and raw stdout/stderr. Source and artifact identities remained stable
during acquisition. The strict selected and full gates fail; only the explicitly
limited development contract passes.

The inventory has 474 requirement records and 1,119 accounting rows. Of those
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
with the two existing regex differences preserved.

Known remaining scope includes classes and modifiers, complete type-name
resolution, constraints and generics, collection members and generators,
`Reference`, reflection metadata, full source grammar, compound values,
complete error selection/rendering, and the upstream semantic corpus.
Floating power is not yet proven equivalent to Java's numeric behavior.
No complete language/API or speed/memory parity claim is made.

Verify archives with `python3 tools/evidence.py verify <archive>`. See
[the M2 contract](../../docs/m2-language.md) for fresh acquisition and strict gate
commands, and [PLAN.md](../../PLAN.md) for the unchanged milestone exit criteria.
