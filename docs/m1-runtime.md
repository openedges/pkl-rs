# M1 runtime experiment

This experiment implements a bounded Rust evaluator shared by native and hosted
callers. It establishes concrete ownership, suspension, and forcing contracts. It
does not establish Pkl 0.32.0 language compatibility, Java diagnostic compatibility,
or achievement of the project's performance targets.

## Values and ownership

`Value` represents null, booleans, signed 64-bit integers, doubles, strings, bytes,
and object handles. Strings and bytes use immutable shared allocations. Object
edges use heap identity, slot index, and generation, rather than `Rc` or `Arc`
references to objects. Handles from another evaluator and reclaimed generations
are rejected. Integer addition, subtraction, and multiplication use the shared
checked numeric helpers. Ordinary division produces an IEEE-754 double, including
division by zero; it is distinct from Pkl's integer `~/` operation.

Each `Vm` owns one tracing arena. Objects contain their parent, member definitions,
captured let values, captured lexical receiver/owner scopes, and lazy caches. The
collector traces every such edge, including completed object values and local
cache-owner identities. Marking uses an explicit work list, so cyclic graph
traversal does not consume the Rust call stack. Collection happens at an explicit
safepoint. Callers supply every externally retained object handle as a root;
the VM supplies operand values, inputs, active or suspended frames, current
receiver/owner pairs, and its last successful result. An external `Value` handle
alone is not an automatic GC root.

Unreachable self cycles and mutually referring objects are reclaimed. Reusable
slots retain generation checks. Closing a VM releases arena storage, execution
stacks, and its reference to compiled code. An independently shared compiled
program can outlive the evaluator. Collection is currently explicit, not an
automatic production GC policy. Allocation counts are evidence about this
ownership mechanism, not a substitute for whole-process RSS measurements.

## Lazy forcing and inheritance

The member cache states are `Unevaluated`, `Evaluating`, and `Completed`.
`Unevaluated` is represented by an absent cache entry. Forcing marks a member as
evaluating before executing its body; completion caches its value. Reentering an
active member reports a cycle. An error stops later operands, removes evaluating
entries owned by the current context, and remains the result of that evaluation.
A later evaluation retries failed members; completed values remain cached.
An inner failure must not remove an outer context's active forcing marker.

An ordinary property caches by name on its dynamic receiver. A local property
also records its definition owner, so an inherited local reference is not replaced
by an unrelated local declaration on the child. Each frame distinguishes receiver
from owner. An inherited body runs with the child receiver and its original
definition owner. Nested object allocation captures that lexical context and its
let values. Static decisions come from the frontend resolver; runtime lookup does
not dynamically search unrelated enclosing objects.

`super.property` starts lookup at the definition owner's parent and preserves the
receiver. It deliberately does not memoize the parent body. This follows pinned
[`ReadSuperPropertyNode`](../external/pkl/pkl-core/src/main/java/org/pkl/core/ast/expression/member/ReadSuperPropertyNode.java),
whose comment explicitly notes the absence of caching. Ordinary and local reads
follow the separate receiver/owner paths in
[`VmUtils`](../external/pkl/pkl-core/src/main/java/org/pkl/core/runtime/VmUtils.java)
and [`ReadLocalPropertyNode`](../external/pkl/pkl-core/src/main/java/org/pkl/core/ast/expression/member/ReadLocalPropertyNode.java).
Let initializers execute before their bodies, as in
[`LetExprNode`](../external/pkl/pkl-core/src/main/java/org/pkl/core/ast/expression/binary/LetExprNode.java).

The original prototype cached failures permanently. Independent review reproduced
a Java 0.32.0 reader that fails once, recovers on the second attempt in the same
evaluator, and then serves the cached success. The runtime now follows Java's
successful-return-only caching rule. The bounded retry and reentry differential
runner is `tools/m1_lifecycle.py`; it preserves diagnostic files separately from
its deterministic observation protocol. Constraints and the full error renderer
remain later work. Missing `super` members currently report an error; Java can
return an empty Dynamic object. This difference remains outside the verified subset.

## Bytecode, suspension, and caches

An immutable `Arc<Program>` contains instruction blocks and object member
templates. A separate `Vm` contains all mutable evaluation state. Compiled code
cannot contain evaluator-owned object handles. Two VMs can share compiled blocks
without sharing objects, inputs, failures, or memoized results. This is a lifetime
separation experiment, not a complete module cache or dependency invalidation
implementation. A production source/configuration/dependency cache key remains
required.

`compile_module(&ResolvedModule)` connects the actual bounded parser and static
resolver to the bytecode evaluator. It supports integer, string and Boolean
literals; arithmetic; lexical lets; ordinary and local object members; `this`;
qualified `super`; and explicit object amendments within the frontend's supported
grammar. It does not implement the full Pkl type system, closures, generators,
imports, constraints, standard library, renderers, or formatted diagnostic stacks.

The compiler fuses addition of an integer literal into `AddIntConstant`. This
removes a constant push and one dispatch without changing checked integer
overflow, mixed floating arithmetic, operand evaluation order, or type errors.
`compile_module_with_options` can disable that fusion for paired correctness
experiments. Both paths have boundary and first-error regressions.

`Vm::poll(budget)` executes at most the requested number of instructions. It
returns a completed result, a yield, or an explicit resource request with a
monotonically increasing request ID. `resume` delivers a value or error to that
same suspended frame. The runtime performs no platform I/O. Suspended frames remain
GC roots. `HostSession::reenter` saves a waiting context and starts a callback
evaluation in the same VM. Nested completion restores the parent continuation;
the parent read can then resume. Every suspended context contributes GC roots.
Duplicate, stale, out-of-order and post-close completions are rejected. Ordinary
`start` still rejects an active evaluation; callback reentry requires its current
read token. A budget permits the Host to regain control even for an infinite loop.

The internal VM cancellation primitive clears an active computation and supports
a later independent start. The public Host wrapper uses permanent closure for
cancellation and timeout. It supplies the monotonic clock and deadline checks.
Timeout and cancellation are cooperative at Host/VM polling boundaries; this
experiment does not interrupt an individual instruction with a wall-clock timer.

## Verification and evidence

Run the runtime and collector regressions with:

```sh
cargo test -p pkl-runtime --test runtime --locked
python3 -m unittest discover -s tools/tests -p test_m1_runtime.py -v
```

The runtime tests cover actual frontend-to-VM evaluation, amended receivers,
definition-local reads, nested let captures, legal object cycles, forcing cycles,
first-error selection, successful memoization and retryable failures, nonmemoized super reads,
suspended-frame roots, stale/foreign handles, slot reuse, cancellation, and malformed
bytecode cleanup. The cycle-reclamation test allocates 10,000 mutually referring
pairs, collects each unrooted pair, and checks that only two arena slots are needed.

The collector uses the pinned Java oracle and preserves complete stdout, stderr,
exit code, source hashes, runtime identity, and raw timing data in a fresh output
directory:

```sh
python3 tools/m1_runtime.py --output target/m1/runtime/new-functional-run
python3 tools/m1_runtime.py --benchmark --output target/m1/runtime/new-measured-run
python3 tools/m1_runtime.py --benchmark --specialized --output target/m1/runtime/new-fused-run
```

The final six successful cases compare complete Java and Rust channels as bytes:
amendment/local/super, captured receiver, nested lexical lets, object cycles, and
an unused erroneous property, plus mixed numeric arithmetic. Three error cases
(first error, forcing cycle, and overflow before a later missing name) preserve diagnostic mismatches as
mismatches; internal first-error and cycle tests do not turn these into byte
compatibility passes. See the [initial functional results](../reports/m1/runtime-functional.json)
and [final specialized run](../reports/m1/runtime-specialized.json).

An earlier run exposed a contextual `new {}` input that Java rejected while the
bounded frontend accepted it. Its original captures remain under
`target/m1/runtime/functional-20261020-01`. The runtime fixture now uses an explicit
amendment parent, and the frontend rejects the unsupported contextual form. The
failed initial run is retained in the evidence archive.

## Performance contract and decision limits

The measured workload is 128 checked integer additions with a seed cycling through
all values 0 through 255. Every evaluation recomputes the arithmetic. All seeds are
validated before timing, and each returned decimal byte string is checked after
its timed evaluation. Generic Rust executes 258 instructions per evaluation and
the fused candidate executes 130; neither
returns a stored expected result or reuses an evaluated object property.

Two independent forks per implementation alternate their order. Each has at least
10 seconds of measured warmup and nine 250 ms measurement windows. Java also
requires eight recent windows within 15% relative spread, with a 30 second wall
limit; a nonconverged run remains inconclusive. The pinned optimizing GraalVM
configuration is unchanged. Its compilation trace is retained so a completed
workload compilation can be distinguished from merely running on an optimizing
runtime. Peak RSS comes from the exact independent child process, including
initialization, correctness checks, warmup and samples.

The boundaries differ: Rust reuses a compiled program and times fresh evaluation
state, dispatch, and decimal conversion; Java calls the public
`evaluateExpressionString` API and parses `run(seed)` per request. Both execute the
same bounded arithmetic, but their ratios measure these embedding boundaries.
They do not isolate bytecode dispatch against optimized Truffle function calls.
They cannot establish whole-product, cold-start, browser, Node, full-corpus,
large-object, diagnostic, or long-session performance parity.

## Measured choice

Adopt scalar-specialized bytecode as the bounded M2 baseline, keep the generic
fallback, and reject generic boxed dispatch as sufficient for this workload.
Revisit the choice with fully optimized Java targets and large lazy object graphs.
This is an execution-strategy choice supported by a narrow experiment, not a
claim that the project's performance requirements are met.

| Candidate and independent fork | Rust median, µs | Java median, µs | Rust / Java time | Rust / Java peak RSS |
| --- | ---: | ---: | ---: | ---: |
| Generic 0 | 1.771 | 1.395 | 1.270 | 0.00488 |
| Generic 1 | 1.770 | 1.393 | 1.271 | 0.00447 |
| Fused 0 | 0.644 | 1.408 | 0.457 | 0.00444 |
| Fused 1 | 0.644 | 1.393 | 0.462 | 0.00445 |

All warmup checks converged. Fusion improved the observed Rust operation median
by approximately 2.75 times. The generic candidate failed the time-ratio target
even at the stated public Java API boundary. The fused candidate's observed time
and RSS ratios are below one at that boundary. These are empirical distributions
from two independent forks, not significance bounds or release verdicts. Raw
series, exact source snapshots, executable copies and runtime identities are in
the [generic report](../reports/m1/runtime-performance.json),
[fused report](../reports/m1/runtime-specialized.json), and
[decision summary](../reports/m1/runtime-decision.json).

A final control executes the generic mode using the exact same copied executable
as the fused experiment. Its two medians are 1.876 µs, compared with 0.644 µs for
fusion, confirming an approximately 2.91 times improvement within the final
binary. Both control warmups converged. See the
[same-binary control](../reports/m1/runtime-control.json); it is a Rust dispatch
control, not a newly paired Java measurement. The measured native `Value` occupies
32 bytes and an instruction occupies 40 bytes. These target-specific sizes do not
establish a production object-layout or memory policy.

No measured Java workload target completed Truffle compilation in the timing
runs above. The pinned GraalVM reported `FrameWithoutBoxing` materialization
bailouts. The initial typed integer, untyped integer, and untyped floating controls
all failed, while an initial `engine.MultiTier` attempt was rejected. Those
observations remain in the [initial investigation](../reports/m1/runtime-compilation.json).

The [closing investigation](m1-graal.md) localizes the failure to
`VmUtils.skipInvisibleScopes`: a loop merges the virtual current call frame with
an enclosing materialized frame, forcing allocation that Graal's virtual-frame
contract forbids. Compiler graphs, source-position traces, recompiled-original
negative controls, and explicitly separate semantic-bypass controls establish
this path for the original arithmetic and recursive fixtures. The narrow bypass
of parameter reads leaves the recursive lexical-call path failing; bypassing the
shared loop also permits the original recursive target to compile through Tier 2.

Separate receiver-field arithmetic and recursive fixtures compile actual Pkl
functions through Tier 2 using the unchanged pinned JAR. Their outputs match
baseline per-seed bytes. They establish positive compilation controls, not a new
performance denominator: their input access and allocation differ from the old
workloads. The diagnostic overrides omit invisible-scope semantics and are not
production fixes. The baseline remains unchanged. Fully compiled Java/Rust
performance comparisons still require new, matched measurements.

The [runtime evidence archive](../reports/m1/runtime-evidence.tar.gz) preserves
both performance experiments, the initial failed fixture run, diagnostic
investigations, raw channels, exact source snapshots, and copied executables.
Verify it with `python3 tools/evidence.py verify reports/m1/runtime-evidence.tar.gz`.
