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
Selected uncolored recursive frame compression is implemented; general stack-overflow
and colored-output behavior remain open.
Cyclic key preparation
currently reports a Rust cycle error;
the observed Java cases fail inside the JVM. Full format-time wrapper checks,
function/class amendments, and the remaining types and metadata also remain
unimplemented or unverified. No new speed or memory parity claim is made.


Deferred Reference values retain the domain, data, normalized referent type,
access path, and required class objects in the shared heap. Synthetic property
and subscript access consult declaration metadata without evaluating referent
member bodies. Exported type normalization erases ordinary user constraints and
aliases and flattens nullable unions. Arrow function metadata remains opaque:
its parameter and result types preserve nested alias identity and authored
constraint text, including whitespace. Type checks and exported metadata equality
remain distinct operations.
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
become unevaluated and retry normally. Only active direct class or type-alias
declarations can return their published singleton before the ordinary forcing-cycle check;
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

### Type aliases and function metadata

User type aliases have a separate evaluator-owned declaration cache. Initialization
publishes the alias value before resolving its definition. A failed definition
leaves the value available for later identity, name, class, and rendering queries;
using that unfinished alias as a type reports the pinned cyclic-alias diagnostic.
Ordinary lazy wrappers still retry. Supported deferred annotations, constructors,
and superclass expressions consult the alias cache at their authored type use.
Alias singletons retain their first captured environment, remain strong GC roots,
and follow the same nonterminal preservation and terminal release rules as classes.
Shared Programs contain declaration keys rather than evaluator heap handles.

Named aliases retain declaration identity and generic arguments independently of
their expanded validation type. Omitted generic arguments use unknown types;
explicit argument counts are checked. Ordinary alias value mismatches report the
expanded type; Reference expected-type diagnostics retain qualified alias names
and type arguments. The scalar evaluation boundary renders alias values
through the common engine in native, Node, and Wasm.

Function annotations retain parameter and result metadata. Arrow and `FunctionN`
annotations check arity; `Function` accepts any supported function arity. An
annotation does not install argument or result wrappers around a lambda. A lambda's
own declared parameter checks still run when it is called.
The current class metadata recognizes `Function` and `Function0` through
`Function5`. Reference subtype checks follow exported metadata rules separately
from equality: arrow metadata is not structurally subtyped, while supported
Function class metadata checks exact type-argument counts and parameter/result
variance. Exported arrow metadata equality and hashing preserve nested alias
declarations and exact constraint text. The parsed module retains its source text
for these identities.

### Ordered type resolution

The compiler emits a resolution plan from the authored type syntax separately
from the expanded value check. Supported named types resolve their base before
their arguments. Class initialization and arity checks precede child resolution;
alias arguments resolve before the unfinished-alias and argument-count checks.
Union branches and function parameter/result types preserve authored order.
This prevents a later alias from being published when an earlier missing name
has already failed. Type errors in member signatures execute during owner
initialization, retaining completed declarations and their original error frames.

Generic parameters take precedence over declarations and builtins for bare type
names in the declaration scope. Explicit type arguments bypass this parameter
lookup, matching the pinned parser's type-name resolution. Nested alias actual
arguments resolve in the caller's parameter scope before substitution into the
callee's definition. Constraint compilation retains the active parameter scope.
Alias values awaiting argument resolution stay on the ordinary traced VM stack.
The plan uses the existing scheduler, declaration caches, and failure unwinding;
ordinary failed lazy members remain retryable.

Check metadata uses compiled templates with runtime actual-argument substitution.
These ordered execution plans do not establish a complete lazy runtime type-node model. General imported
types, annotation syntax, full module initialization/caching, structural types,
reflection, and the original upstream type/function suites remain unverified or
unimplemented. Unsupported builtin generic types remain explicit gaps.

### Recursive constraint compilation

Constraint bytecode can contain a back edge to an active constraint block. The
compiler reserves a block before compiling its expression and reuses that block
for recursive type uses. Constraint templates remain finite because
constraint bodies refer to block IDs. Unused and short-circuited recursive checks
therefore do not expand indefinitely during compilation. Constraint type names
retain the alias declaration's lexical scope even when instantiated in a nested
owner. Existing instruction metadata, Host scheduling, heap roots, and terminal
cleanup continue to apply to the resulting graph.

Builtin `Class<T>` validates its argument count and erases the argument without
resolving its names, aliases, or constraints. User declarations named `Class` and
arguments supplied to an enclosing alias retain their own resolution policy.

Generic constraints compile finite body templates using symbolic parameters
identified by constraint block and parameter name. Recursive back edges reuse the
active template; separate authored uses can compile distinct templates.
Each invocation captures its actual type
arguments in the lexical environment; type checks substitute those arguments when
executed. Back edges can therefore change arguments, including `A<List<T>>`,
without expanding bytecode or imposing a compilation depth cutoff. Deferred
branches, base checks, argument ordering, and declaration scope retain their
existing behavior. Finite direct and mutual recursion have raw Java controls.

Collection element defaults that depend on a symbolic parameter are resolved
after substituting the actual arguments. They preserve the existing distinction
between an unconstrained dynamic default and a type with no default. Nested
collections derive their value defaults recursively; typed defaults reuse the
already resolved evaluator-local class declaration, keeping its members lazy.
This avoids losing class defaults while compiling a reusable generic body.

Reference referent validation that depends on a symbolic type argument also
runs after substitution, at its authored resolution position. It precedes the
associated value evaluation and retains short-circuiting inside constraint
bodies. The validator walks structural metadata without entering executable
constraint bodies or their argument environments. Existing constraint-origin
frames remain available to the runtime; broader diagnostic gaps remain open.

Argument metadata currently uses recursively owned values and cloning. An
unbounded predicate such as `this is A<List<T>>` can exhaust the native stack;
its captured abort remains an explicit failure. The original source also has a
bounded yield/cancel regression, which does not establish safe arbitrary-depth
execution, Java-compatible stack diagnostics, or time/memory parity. Structural
sharing and bounded host-stack work remain follow-up requirements.

### Constraint receiver slots

A logical call frame's auxiliary `this` value lives in a typed, evaluator-owned
heap slot. Inline constraints, nested `is`/`as`, and alias owner changes share that
slot. A constrained type writes its input after the base check, once before its
constraint list; later constraints and diagnostic replay observe mutations made
by nested checks. Successful checks still return the original checked value.
Boolean constraint failures read the current slot, while function predicates
retain the value passed to the function for a later failure diagnostic.

Constraints that write local slots execute in a separate root with a snapshot of
the parent's receiver. Real function and lazy member calls have separate receiver
slots. A lambda captures the custom receiver value when it is created, rather
than observing future mutations in the slot when called. Host suspension retains
the active frame, while reentry creates an independent call. Internal slots and
their values participate in the same tracing arena as objects, including captured
cycles and suspended contexts. They cannot be read as ordinary Pkl objects.
The arena boxes object payloads so adding small receiver-slot entries does not
reserve a full object payload for each slot. This adds object indirection;
retention checks establish lifecycle behavior, not speed or memory parity.

Type aliases apply a module const barrier to lexical property and method reads.
The barrier remains deferred until the read executes; unused aliases, alias
identity, and short-circuited reads need not fail. Stage 17 adds persistent
resolution state for the selected non-generic retry paths described below.

### Persistent type-use and executable ownership

Immutable `Program` blocks describe code templates. Mutable resolution state
belongs to evaluator-local `ExecutableOwnerId` and `TypeNodeId` handles in the
tracing heap. Each executable owns child sites for authored type uses, property
annotations, member bodies, function creation, and constraints. Type-tree child
ordinals distinguish alias expansions, union alternatives, and collection type
arguments. A block or instruction offset identifies a site only within its
executable owner; it is not a global resolution-cache key. An environment with
no explicit cloned owner keeps its canonical static template implicit. Reaching
a check resolves that template's state without changing what subsequent object
or function literals capture, so ownership cannot depend on the earlier branch
history of an invocation. Type nodes are allocated as reached and reused across
failures and retries. Recursive inline
`is`/`as` therefore builds a persistent prefix of distinct type-use nodes instead
of merging every depth into the recursive bytecode block.

Inline alias constraints execute with their type node's executable owner.
Lambda creation inside such a constraint retains a separate body owner in the
function's captured environment. Repeated function calls reuse that body state
while receiving fresh logical receiver slots. Constraints containing
local-writing lets retain a canonical compiler-selected call target; cloned
wrappers share its executable state. These choices are independent of the
`CustomThisSlot` invocation rules above. A lazy collection wrapper retains its
actual argument `TypeNodeId`, so different elements use the same argument node.
Inherited annotation checks use the declaring property's annotation site,
including synthetic standard-library declarations without an authored
`CheckType` instruction. Distinct constrained casts remain distinct even when
they share bytecode: wrapper elision requires the same type-node identity.
Semantic type equivalence remains sufficient for types without executable
constraints.

A local const-boundary read publishes the actual resolved `Member` definition
before checking its const modifier, matching
`ReadLocalPropertyNode.getProperty` in the pinned Java source. The first const
failure leaves that definition available to the same read node on retry. The
read still takes its lexical owner and receiver from the current frame; it does
not capture the first instance. Missing resolution is not published. This state
is separate from successful lazy-value caching, so a later failure in the
property body remains retryable. Ordinary property reads and both lexical method
kinds retain their repeated const failures. Independent inline and lambda type
uses have separate read caches; the selected recursive value-two control has
three failures followed by three successful reads.

Executable entries retain child identities and immutable member definitions,
not captured objects or receiver values. Static call-target roots retain their
reached resolution prefixes for the evaluator lifetime. Objects, functions,
collection casts, active type runs, pending member casts, and suspended frames
trace the executable handles they actually reference. This lets an unreachable
receiver or closure be collected even while its code state remains reachable.
Generation and evaluator checks reject stale or foreign handles. Nonterminal
cancellation unwinds invocation state while preserving published resolution;
close, timeout, and terminal cancellation release the complete heap and static
root table. `HeapStats.executable_live` identifies executable/type entries within
`live`, allowing tests to assert both the original object/receiver counts and
exact persistent-node counts. These are retention checks, not performance or
peak-memory parity evidence.

The const-scope resolver distinguishes object, method, lambda, let, and for
scopes. Methods, let bodies, and for bodies contribute a const barrier without
increasing lexical lookup depth; lambdas increase lookup depth without adding a
barrier. Let annotations and initializers execute before entering that barrier.
Eager generator lookup can skip an object while retaining its const barrier.
Every scope preserves the inherited const level, including generator scopes.
Qualified and standalone `outer` checks inspect the immediate represented
lexical parent. Separate Java controls cover initializers, nested lambdas,
generator inputs and bodies, explicit `module` reads, and scope exit.
Remaining outer and other diagnostic differences are retained in exploration.

Generic inline and lambda constraints receive actual type arguments. A generic
let root remains a shared declaration call target: the Java clone traversal does
not enter that target, so its type variables retain their unbound `unknown`
meaning. Selected Java sequences distinguish this behavior across arguments,
properties, `is`/`as`, recursive depths, and retry order. Native tests retain
argument environments across Host suspension, child reentry, collection,
cancellation, and separate evaluators. General imported types, alias-root
substitution identity, recursive argument resource bounds, and the full type
and const corpus remain open. These cases do not establish complete const,
type, Host, module-cache, or language compatibility.

### Recursive diagnostic frames

Uncolored frame rendering follows the pinned Java compressor, including partial
patterns and count-one loops. Frame equality uses the URI, member name, complete
source lines and UTF-16 positions, before display transformations. Loop margins
and the first hint are rendered together. Selected recursive and mutual-alias
failures exercise this path with power assertions enabled and disabled; general
stack overflow, colored output and the complete diagnostic corpus remain open.
