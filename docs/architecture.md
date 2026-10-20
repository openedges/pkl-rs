# Architecture

The goal is to reimplement Pkl 0.32.0 in Rust, using one shared engine for Native, Wasm, and N-API. M1 now implements bounded frontend, VM, heap, primitive and Host experiments in `pkl-runtime`, with working `pkl-wasm` and `pkl-napi` probes. The complete language, standard library, renderers and product interfaces below remain planned work. See the [M1 evidence](../reports/m1/README.md) for measured limits and decisions.

## Processing Flow

```text
Pkl source
  → parser + SourceMap
  → name resolver + IR + compiler
  → shared runtime + value model + heap
  → standard library + renderer + structured diagnostics
  → evaluator API
  → CLI / REPL / server / Native API / Wasm / N-API

shared runtime ↔ Host interface ↔ filesystem / HTTP / packages / external reader
                              ↔ browser VFS / asynchronous loader
```

The conceptual areas are `syntax`, `parser`, `compiler`, `runtime`, `stdlib`, `host`, `api`, `cli`, `server`, `wasm`, and `napi`. Decide actual crate boundaries once dependencies and implementation scope are understood. Do not create every area as an empty crate at the outset.

## Shared Runtime

Maintain one product evaluator and value model. A bytecode VM is the first candidate, but early prototypes must establish whether it can meet the execution time and memory targets against warmed-up Java. Semantics, diagnostics, and output implementations remain shared even if the execution strategy changes.

Incorporate Pkl 0.32 name resolution into compilation. Distinguish lexical bindings, receivers, owners, `this`, and `super`, and preserve error order across parsing, name resolution, and execution.

Represent lazy members with unevaluated, evaluating, and completed cache states. Keep a failed evaluation's result separate from the member cache so a later attempt can retry. Design inheritance, amendments, closures, type constraints, and memoization together. An evaluator-owned heap must support reclamation, including cyclic objects, in long-running REPL and server sessions.

## Compatibility Boundaries

| Boundary | Design Requirement |
| --- | --- |
| Strings and SourceMap | Distinguish UTF-8 positions, Java UTF-16 positions, and Pkl code point operations |
| Numbers | Match overflow, floating-point string conversion, NaN, negative zero, rounding, and mathematical results |
| Regular expressions | Match Java Pattern syntax, Unicode behavior, replacement, and diagnostics |
| Objects and types | Preserve member order, forcing order, overrides, constraint checks, and selection of the first error |
| Diagnostics | Structure error kinds, arguments, source locations, and logical call frames before rendering them in the 0.32 format |
| Rendering | Match bytes, including line endings, whitespace, escaping, order, and binary encoding |

Use Pkl standard library sources from the pinned upstream and implement external builtins in Rust. Compare public member inventories with builtin registrations to detect omissions. The compatibility scope includes 0.32 features such as `Reference<Domain, Type>`.

## Host and Public APIs

Provide filesystem, HTTP, package, process, and external reader capabilities through the Host interface. The core must not depend on CLI or Wasm adapters. Evaluation waiting on a Host operation must be resumable from the same execution state, with shared definitions for cancellation, timeout, and close behavior.

Wasm uses facilities such as a browser VFS and asynchronous loaders. Explicitly identify capabilities that the Host does not provide, and do not count them as passing full compatibility tests. N-API must transfer large integers and bytes without loss, and performance measurements must include conversion costs between Node and Rust.

The public scope includes the evaluator, CLI, REPL, server protocol, project and package features, formatter, pkldoc, and Java/Kotlin code generation. An adapter preserving existing JVM public APIs is a separate planned deliverable and will also use the Rust evaluation engine.

## Caching and Validation

Give compiled results and evaluation state separate reuse lifecycles. Cache keys must account for source code, configuration, and dependencies that affect results. Verify that caching does not change errors or output.

The Java oracle is a development and validation tool. Product evaluation must not fall back to Java. Evaluate architectural decisions using minimal reproductions, byte comparisons, and execution time and memory measurements that include warmup. Detailed acceptance criteria are defined in [TESTING.md](../TESTING.md).

## M1 contract decisions

- Source offsets are UTF-8 bytes, with explicit conversion to Java UTF-16 and
  code point positions. Synthetic diagnostic underline widths are separate
  from source spans. Resolution records lexical depth, owner and receiver
  choices before execution; product diagnostic rendering remains unfinished.
- Immutable compiled programs and evaluator-owned traced heaps have separate
  lifetimes. Lazy caches retain successful values; failed forcing unwinds only
  the current context's evaluating entries. Active and suspended contexts
  contribute roots, and generation-tagged handles reject stale objects.
- Numeric compatibility uses shared checked integer operations, Java decimal
  formatting rules and audited fdlibm operations where generic libm differs.
  Float equality evidence uses the declared primitive protocol; it is not a
  complete renderer or all-IEEE-input proof.
- The current regex candidate is rejected as a complete backend. The product
  requires versioned Java Unicode tables and a UTF-16 cursor, including
  zero-width progress inside supplementary pairs. The bounded cursor proof
  grounds this model; the general matcher replacement remains implementation
  work and the candidate's known mismatches remain failed cases.
- Host operations suspend and resume the same continuation, with evaluator
  identity, exactly-once request delivery, cooperative deadlines and terminal
  cleanup. Callback reentry uses nested contexts in the same VM and heap,
  sharing successful member caches while preserving each continuation and
  deadline. Terminal transitions identify outstanding reads for platform
  adapters to abort. Full module/resource caches and product APIs remain later
  work. See [the Host contract](m1-host.md).
- The generic bytecode prototype's measured slowdown rejects unconditional
  adoption on performance grounds. Checked scalar-specialized bytecode is
  selected as the bounded M2 baseline after a separate measured improvement,
  with the generic path retained for fallback and equivalence regressions.
  The Java target compilation bailout and broader workloads remain open gates.
  No M1 microbenchmark establishes the complete release performance target.

## M2 implementation in progress

Classes and closures live in the evaluator's existing traced arena. Class
prototypes use the same parent chain as amended objects, and their captured
environments participate in collection. Class identity and inherited member
checks are shared across all three runtime targets. Type aliases resolve in
their declaration scope; generic arguments resolve at their use site. The
timing of class initialization and type validation is part of compatibility,
including declarations whose errors remain unobserved until they are used.

Instruction source spans and authored member names are metadata beside the
bytecode. VM errors retain these logical frames before unwinding. A shared
scalar evaluation boundary combines them with source URIs and module names
and renders complete uncolored diagnostics. Native `language_probe`, Node
`evaluateLanguage`, and Wasm `evaluateLanguage` use this boundary. These are
development adapters with scalar property-path results; full expression APIs,
compound rendering, and external module loading remain separate requirements.

Identifier predicates are generated from the pinned Java lexer and JDK.
The upstream lexer tests UTF-16 code units, so supplementary characters are
not accepted as unquoted identifiers even when their Unicode code point is
a Java identifier character. Source spans remain UTF-8, with explicit UTF-16
conversion for error columns and underlines.

Selected differential checks establish only the behavior they execute. See
[the M2 contract](m2-language.md) and [raw results](../reports/m2/README.md)
for the remaining language and diagnostic obligations.

### Constraint evaluation design (not implemented)

Constraints will execute as continuations in the existing VM. The current
synchronous `TypeCheck::accepts` can classify primitive leaves, but cannot
execute a predicate, wait on a Host, or recover a union alternative. Predicate
blocks and function-valued predicates must use ordinary VM call frames with
an explicit return destination for the pending type check. A lazy member may
publish its cached value only after every inherited and local check succeeds.

Recoverable type mismatch must be separate from evaluation failure. A false
constraint can select the next union alternative or produce `false` for `is`;
an invalid predicate result, thrown error, or Host failure must propagate.
The pinned union implementation first tries alternatives with assertion
tracking suppressed, then may execute them again to construct a failure.
A failing constraint can execute again under value tracking. Those repetitions,
their side effects, and diagnostic values are observable compatibility behavior.

The resolver must distinguish a constraint's scalar-capable custom `this` from
ordinary object receivers. Closures capture that context, while nested objects
keep their own `this`. Alias bodies require their declaration environment;
actual generic arguments retain their use-site environment. Immutable programs
must contain environment lookup plans rather than evaluator-owned heap handles.

Active and suspended type-check continuations must retain their input and
checked values, predicate closures, bound environments, mismatch values, and
assertion observations as GC roots. Host suspension, nested evaluation, cancel,
close, failure retry, and logical diagnostics must use the same lifecycle as
ordinary VM frames. Nullable wrapper order and union failure selection require
fresh Java comparisons before this design can count as implemented behavior.
