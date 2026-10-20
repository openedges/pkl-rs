# M1 Graal compilation investigation

This investigation closes the M1 requirement to explain the original recursive
workload's compilation bailout and identify an actually compiled Pkl workload.
It is a compiler diagnostic experiment, not a new performance measurement.
The Pkl 0.32.0 reference, official tools JAR, GraalVM 25.0.1 distribution,
heap limits, and existing performance evidence remain pinned.

## Failure and causal control

The original arithmetic function reaches `VmUtils.skipInvisibleScopes` through
`ReadFrameSlotNode` → `GetEnclosingFrameNode.executeGeneric` → `VmUtils.getFrame`.
The original M0 recursive fixture also reaches that loop through
`AbstractInvokeMethodLexicalNode.executeGeneric` when resolving recursive calls.
`-Djdk.graal.TrackNodeSourcePosition=true` exposes both source paths in the
compiler bailout stacks.

`getFrame` calls `skipInvisibleScopes` even when `levelsUp` is zero. The loop's
`frame` variable initially holds the current virtual call frame, but a loop
iteration assigns `owner.getEnclosingFrame()`, a stored `MaterializedFrame`.
The retained compiler graph identifies the resulting failure mechanism:

- Virtual `FrameWithoutBoxing` node 194 is forced into allocation node 4270,
  whose `ensureVirtual` flag for that object remains `true`.
- Frame phi node 1239 merges allocated current frame 4271 with the enclosing
  frame load 1395 at loop header 1238.
- Allocation 4270 precedes loop-entry `EndNode` 1237, the node named in the
  bailout. Source positions locate the loop at `VmUtils.java:192`.

These node identifiers belong to the archived graph, not to all future runs.
Graal's `PartialEscapeClosure.processPhi` materializes virtual inputs when the
phi cannot merge them as compatible virtual objects.
`CommitAllocationNode.lower` rejects an allocation whose object must remain
virtual. The graph therefore explains the bailout as an incompatible merge of
virtual and materialized frames in Pkl's invisible-scope traversal; the error
message alone would not establish this mechanism. See the
[graph evidence](../reports/m1/graal-cause-evidence.tar.gz), including the raw
binary graph, full parser output, pinned source snapshots, and exact commands.

The diagnostic controls isolate both callers. A replacement
`GetEnclosingFrameNode` that returns the current frame at zero lexical depth
lets the unchanged arithmetic function compile through Tier 2. The original
recursive fixture still fails through `AbstractInvokeMethodLexicalNode`, which
calls `getFrame` directly. Replacing only `VmUtils.skipInvisibleScopes` with
`return frame` then lets both original recursive targets compile through Tier 2.
These overrides deliberately omit invisible-scope semantics. They demonstrate
the causal path for these fixtures and are not a general Pkl fix, compatibility
oracle, performance baseline, or Rust production code. Reference checkouts and
the official JAR remain unchanged. This investigation does not establish a
general Graal compiler defect or validate a production repair.

Recompiling each original class without the bypass still reproduces the failure.
The `VmUtils` pair uses the same import relocation needed by the official shaded
JAR; only the bypass changes the method body. Class-loading logs confirm which
class actually executes in each case.

## Positive workload and interpretation

A second arithmetic workload supplies the varying input through an object
receiver field and performs the same 128 checked additions in a method. The
original pinned JAR and GraalVM compile the actual Pkl computation through
Tier 2. Every returned UTF-8 byte string is checked against a baseline preflight
for that seed. Compilation attribution requires the exact guest source URI,
source line, target identity, and tier; compilation of the Java helper or an
unrelated standard-library target does not qualify.

The receiver-field recursive control computes the same Fibonacci recurrence
for inputs 24 and 25 and returns the original bytes `46368\n` and `75025\n`.
Its recursive Pkl method also compiles through Tier 2 on the unchanged baseline.

The receiver-field workloads change the Pkl programs' input access paths.
They are positive compiler controls, not interchangeable performance denominators
for the previous Rust/Java arithmetic ratios. Failures of other targets in the
same execution remain visible in the raw trace.

The formal acquisition records these outcomes. Counts represent calls checked
for exact result bytes, not benchmark samples or a throughput comparison.

| Case | Checked calls | Selected Pkl target |
| --- | ---: | --- |
| Original arithmetic | 9,278,606 | Tier 1 bailout |
| Arithmetic, original node recompiled | 9,863,346 | Tier 1 bailout |
| Arithmetic, parameter-read bypass | 15,953,146 | Tiers 1 and 2 completed |
| Receiver arithmetic, unchanged JAR | 14,842,541 | Tiers 1 and 2 completed |
| Original recursive fixture | 1,227 | Tier 1 bailout |
| Recursive fixture, parameter-read bypass | 1,288 | Tier 1 bailout through lexical call |
| Receiver recursion, unchanged JAR | 1,690 | Tiers 1 and 2 completed |
| Recursive fixture, original scope helper recompiled | 1,237 | Tier 1 bailout |
| Recursive fixture, scope-loop bypass | 29,590 | Tiers 1 and 2 completed |

Each receiver run also records a captured-property target failure. Those failures
remain in both the report and full stderr; successful computation targets do not
make every target in those programs compilable.

The original measurements remain measurements of the public Java API boundary
with the recorded target bailouts. They do not establish performance against a
fully compiled version of the original workload. Future performance comparisons
must validate their own target compilation, warmup, correctness, source identity,
and matched invocation boundaries.

## Reproduction and verification

The diagnostic runner uses a fresh output directory, snapshots sources before
execution, records exact oracle and override identities, and preserves stdout,
stderr, exit codes, commands, per-seed expected bytes, and compilation traces.
Its report verifier rejects missing execution, wrong-target compilation,
incomplete correctness checking, and changed evidence bytes.

```sh
python3 tools/m1_graal.py --output target/m1/graal-new
python3 tools/m1_graal.py --check-report target/m1/graal-new
python3 -m unittest discover -s tools/tests -p test_m1_graal.py -v
```

Acquisition requires the pinned macOS arm64 performance JDK. Stored evidence
verification and the tooling regressions run without executing that JDK.

The [diagnostic report](../reports/m1/graal.json) and
[raw archive](../reports/m1/graal-evidence.tar.gz) retain the fixed case inventory.
Extract the archive into a fresh directory after verifying its inventory, then
pass that directory to `--check-report`; relocation preserves the recorded
absolute source URIs and commands without rewriting any diagnostic bytes.
The separate graph archive contains its own `reproduce.py`, exact command
captures, graph parser, and source provenance.
