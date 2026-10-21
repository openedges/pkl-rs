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

### Constraint continuations

Constraint checks use a continuation in the checking VM frame. Primitive
classification remains a pure operation; predicate blocks and function-valued
predicates run on ordinary bytecode frames, return into the continuation, and
can suspend on the same Host interface. Lazy members publish cached values only
after their inherited and local checks succeed. Inherited constraints bind the
original declaration owner while retaining the actual object receiver.

Recoverable mismatch is separate from an evaluation failure. A false constraint
can select the next union alternative or produce `false` for `is`; an invalid
predicate result, thrown error, or Host failure propagates. Nullable wrapper
order is preserved. Union alternatives first run with assertion tracking
suppressed. With power assertions enabled, a failed check can execute again to
construct its diagnostic, including another tracked predicate invocation.

The resolver distinguishes a constraint's scalar-capable custom `this` from
ordinary object receivers. Closures capture that context; nested objects retain
their own `this`. Type names in generic arguments resolve at their use site.
The pinned alias checker then changes the checking frame's receiver and owner
to the alias declaration environment while preserving its local slots. That
runtime behavior also affects substituted constraints; it is different from
capturing an independent use-site receiver for every type argument.

Active and suspended continuations trace their values, predicate functions,
environments, mismatch values, and assertion observations. Diagnostic snapshots
contain rendered strings rather than heap handles. Value rendering reads only
completed caches, keeps inherited declaration order, and applies the pinned
UTF-16 length limit without forcing unevaluated members.

Power assertions default to disabled, as in the Java API. An explicitly
instrumented compilation adds observation instructions; ordinary compilation
does not add these dispatches. The shared source renderer associates executed
expression spans with operator and member positions, hides literals and
repeated observations, and preserves multiline and UTF-16 layout. Parser
grouping nodes retain both outer source syntax and inner execution spans.

These mechanisms remain subject to the selected differential and lifecycle
checks. Complete structural and function types, imported
aliases, and the full language corpus remain M2 requirements.

### Ordered collection construction

Object expressions retain properties, entries, elements, and generator clauses
in their syntax tree. Construction runs on ordinary VM frames. Entry keys and
generator headers execute during construction; member bodies remain lazy.
Each generated member captures its own local environment while retaining the
receiver used when that member is eventually forced. Amending a collection
keeps inherited positions and replaces matching members.

Properties and entries use separate cache namespaces. A tag distinguishes them
even when a quoted property contains the same bytes as an internal entry name;
a reserved string prefix alone cannot provide this distinction. Primitive
entry keys follow the pinned boxed-key rules: integer and floating-point keys
are distinct, floating-point signed zeros are distinct, and NaNs compare equal.
Object keys use the resumable structural comparison described below; function
and class keys retain identity comparisons.

`Dynamic`, `Listing`, and `Mapping` share ordered member metadata with the
prototype chain. Strict `List` construction supplies another iterable value.
Static object bodies and bodies containing generators preserve the different
ordering rules used by Java. Iteration first shallow-forces source members in
the pinned child-first order, then visits visible members in inherited order.
The special `default` property is excluded from iteration and spread.
Errors in that forcing pass therefore precede generated target keys, conditions,
or duplicate checks. Spread retains the evaluated source values.

Partially constructed objects and iterator cursors are GC roots of their VM
frames, including suspended frames. Iterator source values, captured member
environments, keys, and unpublished builder state remain live across Host reads,
nested evaluation, and cooperative yields. Failed construction does not publish
a successful lazy cache entry; retry and terminal cleanup use the existing
evaluator lifecycle.

Diagnostic rendering walks ordered metadata and completed caches without
forcing values. Subscript observations use the authored bracket position in
power assertions. These mechanisms serve native, Node, and Wasm through the
shared runtime. Complete map semantics and typed-object generator rules remain
open; the structural comparison section below describes the verified key scope.

### Generic collection checks

Parameterized collection checks run through the same type-check continuation.
`Listing<T>` retains a lazy value cast; `Mapping<K,V>` checks keys before
installing a lazy value cast. `List<T>` checks its strict elements and stores
returned wrappers, including nested collections. Type tests, selected union
alternatives, and checks reached while evaluating a constraint can require eager
member checks. The constraint's eager mode remains active across Host reentry.

Wrappers retain the declaration environment and ordered parent chain. Forcing
checks applicable inherited casts before publishing a successful cache entry.
Cache stealing and adjacent cast reuse follow type-node equivalence; distinct
constraint nodes remain distinct even when their source text is identical.
Generic alias instantiations therefore cannot share a predicate check merely
because they point at the same declaration span. Union diagnostics retain
alternative labels separately from expanded alias check spans.

The heap traces wrapper environments and inferred default values. Continuations
also root temporary receivers, checked nested values, and successful results
awaiting delivery. Checked parameter values replace their original bindings.
Selected inferred constructors and class-valued collection defaults use the
same metadata and prototype paths as explicitly constructed objects.

This remains a bounded implementation. Full custom default functions, structural
types, complete key semantics, and the remaining constructor rules are open.
The pinned Java diagnostic formatter can steal parent caches and execute a
wrapper cast while displaying values; that can itself fail or invoke a
constraint. The Rust formatter currently reads completed caches only. Supporting
those format-time checks requires resumable diagnostic preparation, including
Host and GC handling. Exploratory failures remain recorded separately from the
selected passing wrapper diagnostics.

### Member predicates

Member predicates traverse the parent's declarations child first, suppressing
already visited keys and excluding properties and local members. Each candidate
value is evaluated immediately before its condition. This differs from the
shallow forcing pass used by `for` and spread. Newly declared members in the
target body do not become candidates.

A dedicated resumable cursor retains the parent receiver, declaration owners,
member keys, and previous custom `this`. The predicate binds custom `this` to the
candidate value while preserving surrounding lexical lookup. Selection restores
the previous context before capturing a lazy replacement body. Nested predicates,
closures, and generator locals use the same frame environments and heap tracing
as other expressions.

Candidate evaluation reads and publishes the declaration owner's cache while
executing the body against the parent receiver. It follows the pinned raw
declaration call path rather than the ordinary collection-wrapper cast path.
Selected members preserve the original element or entry kind, inherited index,
and duplicate checks; their bodies remain lazy. Diagnostics retain the authored
predicate label and condition span.

Typed constructors with generated bodies validate their class before executing
the body. A reached predicate rejects an ineligible typed parent, while an
inactive branch leaves it untouched. Null amendments create an empty Dynamic
parent. These paths are covered by selected probes; complete constructor,
modifier, and standard-library behavior remains open.

### Structural comparisons and keys

Object comparisons run on the ordinary VM scheduler. A continuation retains
pending comparisons, deep-force work, key searches, and their heap roots.
Identity and type checks precede forcing. Listing length differences skip
forcing; Dynamic and Mapping compare sizes after forcing both operands. Typed
objects require the same class prototype. Strict List comparisons stop at the
first unequal element. Nested values use boxed equality: integers and floats
remain distinct, NaNs compare equal, and signed zeros differ. Scalar operators
retain their separate numeric rules and direct execution path.

Deep forcing visits child declarations before ancestors and recursively visits
each value before the next member. Structural key lookup preserves query-first
comparison and inherited override behavior. Small declaration maps compare keys
without eagerly hashing every object. Adding a fifth distinct member prepares
hashes in declaration order, including generated properties, elements, and
spread. Mapping length prepares keys in ancestor order; equality performs this
after deep forcing. Prepared structural identities and successful key-equality
pairs are heap metadata, pruned when their objects are reclaimed.

Cache publications retain insertion slots separately from pending evaluation
markers. Regular Dynamic, Listing, and Mapping comparisons use the original
slot values; typed comparisons use class property declarations, visiting
the most derived class before its ancestors. Ordinary typed properties named
`default` participate in forcing, comparison, and structural key preparation.
The special collection default and hidden/local properties retain their
separate rules. Nonconstant locals use declaration identity as their cache key;
Listing and Mapping include those slots, producing asymmetric comparisons when
only one operand has a local cache. Literal constant locals bypass that cache.

Dynamic, Listing, Mapping, and typed key preparation completes deep value forcing
before recursively preparing member keys and values. That second phase uses
cache or class property order. Both phases run on the same scheduler and retain their operands across
Host suspension, nested evaluation, GC, and failure recovery. The resulting
canonical identities describe prepared value graphs, separately from Java's
32-bit hashes. They do not bypass later object comparison: nested cache warming
can change directional equality. Successful comparison memos are directional
and are invalidated by local or duplicate cache publication.
Stable hashes include scalar values, UTF-16 strings, class
qualified names, and eligible structural values. Strict List starts its
sequential element hash at one; Listing and typed objects start at zero.
Zero hashes are recomputed on later operations, matching Java's cache sentinel.
JVM identity-dependent hashes remain explicitly unknown.

The map index models forward linear comparison through four entries, followed
by bucket lookup with newest collisions first. Bucket matching uses the mixed
low bits without filtering by the full hash. Stable declaration maps preserve
their index and insertion hashes; an actual rebuild refreshes those hashes.
Member caches retain their actual index, keys, and values.
The fifth cache value is published before its keys are hashed. A failed rebuild
leaves all published slots and only the completed bucket prefix visible. A
retry may append another slot for a hidden key; its old value remains a GC
root and participates in cursor traversal. Capacity changes and load-triggered
rebuilds preserve their distinct publication order. Hashing and comparison can
suspend through the shared VM and Host continuation.

Mapping length uses a separate ancestor-first unique-key pass, matching Java's
HashSet boundary. Each key is hashed and compared before the next key; stable
full hashes must match before query-first equality is evaluated. Insertion
hashes stay attached to retained keys even if a later key warms an earlier
cache. Only a completed pass publishes the cached length, and entry values
remain unforced. The same continuation supports equality's length check.

An empty object body preserves the parent's identity; empty typed construction
uses the class prototype. Compile-time constant entry validation recognizes
scalar and nested List literals after name resolution. It runs only for
specialized bodies whose keys are all constant. Folded signed integers retain
their numeric token locations so diagnostic underlines exclude the unary
operator without changing expression spans.

Uncolored diagnostic frames replay the pinned source highlighter's token
traversal. Its interpolation handling can omit whitespace after an inner
closing parenthesis; rendering preserves that behavior while underlines use
the original source positions. A lexical error restores the complete source
line. This display path does not rewrite source inputs or comparison evidence.

Let-body ranges are source metadata, adding no VM dispatch instructions.
When an evaluation error unwinds, the outermost active let contributes one
inserted frame per call target. Initializers and recoverable type mismatches
follow Java's distinct catch boundaries; explicit casts convert a mismatch
before an enclosing let catches it. Constraints that write local slots
have a separate logical root; lexical let names remain attached to captured
objects and functions after the let returns. Inline constraints without named
bindings do not contribute their own inserted let frame.

The selected cases do not establish complete Java map behavior. Identifier,
local declaration, and function identity hashes cannot be reproduced with
portable numeric hashes. Declaration maps with unknown hashes use a linear
comparison fallback. Member caches use evaluator-local bucket assignments for
those keys; these assignments do not establish JVM identity hash parity.
Same-cache cyclic reentry during a partial rebuild, unpublished
declaration-map failure state, and cyclic structural hashes remain open.
Repeated recursive diagnostic frame compression also remains unimplemented.
Cyclic key preparation
currently reports a Rust cycle error;
the observed Java cases fail inside the JVM. Full format-time wrapper checks,
function/class amendments, and the remaining types and metadata also remain
unimplemented or unverified. No new speed or memory parity claim is made.


Deferred Reference values retain the domain, data, normalized referent type,
access path, and required class objects in the shared heap. Synthetic property
and subscript access consult declaration metadata without evaluating referent
member bodies. Exported type normalization erases user constraints and aliases,
flattens nullable unions, and removes authored spans from parameterized type
identity. Type checks and structural equality remain distinct operations.
Reference forcing, hashing, and domain rendering use the existing VM scheduler
and GC roots. Domain rendering runs ordinary method bytecode and can suspend for
Host input, reenter, fail, retry, or terminate through the same lifecycle.

The current module bridge recognizes `pkl:ref`. Import aliases remain local const
members, qualified type resolution uses the import scope, and class values retain
a declaration kind separate from the builtin `NonNull` type alias. The bridge does
not implement general module loading or the complete reflection API. Class and
member metadata preserve inherited declaration owners and types for Reference
access; public reflection and other standard-library modules remain work in
progress.

Logical frames may carry a detached source identity for a declaration outside
the primary module. The shared renderer uses that source's text, module name,
URI, and line anchor. The pinned `ref.pkl` source is bundled unchanged with its
license and included in build-evidence snapshots. This supplies authored source
locations for native constructor checks, inherited Access checks, and the domain
rendering call. It does not imply that the entire standard-library module is
interpreted by the current evaluator.

Fixed property flags remain attached to shared heap declarations alongside const
flags and declared types. Class initialization resolves each property's type and
checks fixed agreement before const agreement, visiting properties before methods.
A subclass may replace a fixed declaration when it retains the fixed modifier;
instance assignments and property amendments reject inherited fixed declarations.
These checks inspect declaration metadata without forcing property values. Fixed
initializers retain ordinary virtual receiver dispatch, lazy caching, Host
suspension, and failure recovery. Reference access uses the same inherited
metadata and leaves referent initializers deferred, including hidden fixed values.

Authored property amendments retain a separate source marker from explicit
assignment expressions. This distinguishes assignment and amendment diagnostics
and permits abstract amendment declarations while rejecting abstract assignment
bodies. Modifier errors before declaration scope entry use the enclosing frame;
abstract body errors use the declaration frame and header range. Class override
modifier locations follow the pinned header substring lookup, including comments
and repeated modifier flag carries.

### Class publication and evaluator ownership

Class declarations have an evaluator-owned singleton cache keyed by the compiled
declaration template. `BeginClass` publishes an empty prototype before resolving
the superclass. Successful property declarations publish in order, followed by
methods; only `FinishClass` marks initialization complete. A failure preserves
the singleton, its parent if already resolved, and the successful declaration
prefix. Later access returns that same partial class without resuming its
initialization. The immutable shared `Program` never owns heap handles.

This cache is separate from ordinary member caches. Failed lazy properties still
become unevaluated and retry normally. Only an active direct class declaration
can return its published singleton before the ordinary forcing-cycle check;
arbitrary properties and functions keep their cycle detection. Recursive class
types resolve a real class handle instead of a placeholder null value.

An unfinished class allows property assignments even when a declaration has not
been published. Fixed/const checks and Reference metadata lookup use the actual
published and inherited definitions. Untyped prototype overrides preserve the
inherited class property definition while value lookup uses the overriding
initializer. Hidden declarations and unpublished tails therefore have distinct
metadata and value lookup behavior.

Singletons are strong GC roots for the lifetime of their VM. Their captured
environment and already cached values remain reachable; unrelated temporary
instances and failed builders remain collectible. Nonterminal VM cancellation
unwinds the active evaluation while preserving published classes. Host session
cancel, timeout, and close are terminal and release the singleton cache with the
heap. Tests distinguish retained declaration graphs from temporary allocations
and require terminal heap release, rather than weakening collection assertions.

The same compiled declaration retains its first captured environment in one VM.
Separate VMs sharing a Program have separate singletons and reject foreign heap
handles. This is not a complete module loader/cache or source-replacement policy.
The persistent scalar sequence adapter keeps one module across calls; full module
URI/source identity remains a later integration requirement.

Indirect cyclic superclass declarations can preserve class identity. Java
property traversal of an observed cyclic hierarchy did not terminate; Rust's
visited-set guards avoid an unbounded traversal. Those traversal results remain
outside verified compatibility. Java can also suspend class initialization while
evaluating an annotation containing a resource read. Annotation syntax is not yet
supported by the Rust frontend. Internal instruction suspension tests verify VM
reentry and cleanup separately from that unimplemented source-language path.
