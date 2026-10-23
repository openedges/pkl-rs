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
development adapters with scalar and selected Reference/type-alias/strict List/Map/Set
property-path results. SourceSession also compiles expression entries through
the common frontend and executes them against the actual module receiver;
declared-type recipes resolve against the actual module owner and can suspend
for typed module Host requests. This type context does not expose lexical imports
as ordinary value bindings in the synthetic expression scope. The complete
product expression API remains unfinished.
Typed module resolve/source Host requests load imported code into the same
VM and traced heap. Original and resolved URI caches retain distinct behavior,
including resolve retries, cached source/initialization failures and source
callback reentry. Imported classes and aliases retain their declaring unit and
owner. An `extends` module initializes its own class; an `amends` module adopts
its parent's class through the same Host continuation. Complete recursive-type
and inheritance behavior, general compound rendering and the remaining transport
capabilities still require implementation and corpus validation.

Supported `pkl:base`, `pkl:test`, and `pkl:ref` declarations use that same module
arena with pinned declaration sources and Rust external bindings. Guest catch
handlers belong to invocation frames and unwind to the nearest applicable
forcing or guest boundary; they preserve successful lazy publications and
partial cached errors. Typed language exceptions from the Host are distinct
from raw Host and terminal failures. Standard-module bootstrap and JVM-global
cache parity remain incomplete. See [standard modules and guest exception
handling](m2-stdlib-bootstrap.md) for the execution and verification boundaries.

Identifier predicates are generated from the pinned Java lexer and JDK.
The upstream lexer tests UTF-16 code units, so supplementary characters are
not accepted as unquoted identifiers even when their Unicode code point is
a Java identifier character. Source spans remain UTF-8, with explicit UTF-16
conversion for error columns and underlines.

Actual module output uses the same prepared program, evaluator-owned module,
and Host continuation stack. The operation reads `output`, checks its exact
`ModuleOutput` type, and evaluates `bytes`. Source-backed output classes use
ordinary lazy members; PCF traversal and converter calls retain their state
across Host suspension. Completed byte buffers outlive evaluator closure.
The frontend-only API remains a separate diagnostic development boundary.
See the [module comparison contract](m2-module-frontend.md) for its scope.

Selected differential checks establish only the behavior they execute. See
[the M2 contract](m2-language.md) and [raw results](../reports/m2/README.md)
for the remaining language and diagnostic obligations.

### Canonical diagnostic text

`RuntimeError`, diagnostic messages and hints, and logical frame names use
`PklString`. Diagnostic construction and rendering retain UTF-16 code units,
including unmatched surrogates. Convert to Java-compatible UTF-8 only at a
declared byte boundary. `Display` is such a conversion and must not be used to
compose canonical messages from Pkl strings.

`ModuleFrontendFailure` carries the frontend phase, disposition, optional
unsupported reason, and canonical text. Converting it to `ModuleFrontendResult`
produces the existing exit code and byte channels. The scalar `*_text_with_options`
helpers and session `*_text` constructors expose canonical text; legacy Rust
`String` helpers encode their values and errors explicitly. `SourceProbe` keeps
error units alongside byte output, deriving both from the same failure.

Node and Wasm scalar/session adapters return JS strings with the original units.
N-API creates the error's message as a UTF-16 string and throws the native Error
directly. Converting that object through `napi::Error::from` would read user-visible
`toString` and `cause` properties. Wasm constructs strings in bounded chunks to
avoid JavaScript's variadic argument limit, including when a surrogate pair spans
two chunks.

Module and typed resource Host failures retain `Option<PklString>` messages,
preserving absent and empty IOException causes. Completion validates the current
request token and uses its actual URI. The probe text methods and JS adapters
preserve rejection-message units; generic error injection remains a separate
operation. Converter-path, output-format, and abstract-member diagnostics also
compose canonical text. The exact expression request `output.text` uses the
shared ModuleOutput operation and module receiver, matching Java's direct API
dispatch without adding an expression REPL frame.

Source text, URIs, and identifiers still use the UTF-8 input boundary. Other Java
exception classes and the complete Host failure surface remain unfinished.
Actual Java UTF-16 captures establish string equality;
older UTF-8-only fixtures establish only their declared serialization boundary.

### Resource reads and implicit method diagnostics

`read` and `read?` have dedicated expression nodes and VM instructions. Their
operands follow the ordinary iterative traversal and power-assertion paths;
quoted method names continue through normal method lookup. A pending read
retains its authored URI, declaration-source URI and nullable flag. The scalar
development profile resolves and checks the URI, then completes the request
through `Vm::resume`, preserving the current continuation and diagnostic frames.
Its environment/property maps are explicitly empty, and permitted I/O readers
are still unimplemented. Custom Host callbacks retain their separate contract.
See the [resource-read contract](m2-resource-reads.md) for the remaining URI,
absence-transport and resource-cache obligations.

Implicit method suggestions retain each lexical scope's property/method groups
before enclosing and base candidates, followed by receiver candidates. Similarity
and arity ranking is stable, and deduplication preserves the earliest visible
candidate of each kind and name. Explicit receivers keep their existing
property-before-method search contract. Candidate collection reads declaration
metadata without forcing values; diagnostic builtin signatures do not imply
that their callable bodies are implemented.

### Authored expression traversal

Ordinary authored expression paths use explicit parser, resolver and compiler
continuations. Their traversal storage grows on the heap; expression-tree cloning
and release also avoid recursion along those paths. Literal List keys use a flat
structural token stream for resolution, equality and diagnostic rendering.
Compiler continuations preserve child order, short-circuit targets, source spans
and power-assertion observations.

If, let, lambda and property-only object paths also use continuations. Type parsing
uses a work stack with a logical-depth and constructed-height limit of 32. The
65,536-step expression work budget is shared across interpolated expressions.
Guard rejection and successful processing below a guard are separate obligations.
String scanning and interpolation parsing also use explicit work frames. Stage 26
checks accepted depths 24/31 and safe rejection of the original depth-64 input on
256 KiB stacks. The nesting limit remains 32, and its rejection differs from Java.
Auxiliary lexical error validation can reparse ranges synchronously. Deep type compilation,
class/instance and mixed/generator body compilation retain recursive helpers.
Derived AST Debug and PartialEq also remain recursive. The selected checks do not
establish arbitrary authored depth, uniform compilation latency or complete Java
resource-error compatibility.

Scanned string parts retain byte spans, including empty text tokens used by
Java's multiline validation. Stage 27 aligns selected invalid-escape, EOF,
continuation and multiline diagnostics with Pkl 0.32.0, preserving interpolation
error order and explicit UTF-16 underline widths. Cached lexical failures retain
their original consumed end and source-end provenance. Parse-time recovery tests
use fresh evaluators because a failed module setup cannot produce a retained one.
Selected malformed Unicode diagnostics preserve Java's conversion timing and
UTF-16 token widths. Java's out-of-range scalar bug remains a separate
compatibility gap.

### Language string values

`PklString` preserves UTF-16 code units, including unmatched surrogates. A
well-formed value uses UTF-8 storage; a value with unmatched units uses UTF-16
storage. Construction and concatenation canonicalize well-formed pairs, so
escaped pairs and literal supplementary characters have the same value.
Source text, identifiers and source positions retain their separate UTF-8 types.

String literals, interpolation, literal types and defaults, collection keys,
comparison, Java string hashing, and the implemented `toString` paths retain
the lossless value. Code point length counts an unmatched surrogate as one
point. Reference type names and metadata keys also preserve literal units.
Host suspension and evaluator caches carry the same value representation.

`to_java_utf8` is an explicit output operation: it maps unmatched units to `?`
as Java's UTF-8 writer does. Native byte adapters observe that writer boundary;
the canonical scalar/session APIs and JS bindings preserve units separately.
These development adapters do not establish the complete embedding API.
Module and resource URIs containing unmatched
units remain explicitly unsupported rather than being converted to a different
URI. Full string builtins, general Java regular expressions, and performance
and memory parity remain separate requirements.

### Inferred object parents

Methods construct the parent of contextual `new {}` from their declared return
type. Inference follows `if` expressions and let bodies; parentheses, lambdas,
and let initializers keep their own boundaries. Type defaults are created only
when execution reaches `new`, before amendment members run. Literal `unknown`
retains the Dynamic default; `Any` and aliases of `unknown` do not acquire it.

Typed properties use the same default construction. An inherited class
prototype uses the declared property type's default, while an instance amendment
reads the class prototype's initializer without applying its constraint before
the amendment. These reads use ordinary resumable VM frames and existing traced
default continuations. Undefined prototype defaults become an inference error
at `new`; other failures retain the prototype error and omit the inference read
frame. Cache cleanup still visits every active frame.

Generated collection default errors retain the method header or property type
span, as appropriate. Their metadata contains source information, not heap
handles. Selected Java comparisons and native Host/GC tests cover these paths;
general module inference, complete diagnostics and object stringification remain
separate requirements.

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

### Strict Map values

Strict `Map` uses a distinct collection kind in the evaluator-owned arena. Its
entries contain evaluated keys and values; nested objects retain their own lazy
members. Construction evaluates each key and value, then hashes and inserts
that pair before evaluating the next pair. Duplicate keys retain the original
insertion position and replace the value. Construction frames and resumable
hash/equality tasks retain unpublished builders and operands across Host reads,
reentry, GC, failure, and cancellation.

Strict Map lookup follows the pinned Paguro hash-trie routes separately from
the EconomicMap index used by object declarations and caches. Iteration and
diagnostic rendering use insertion order. Deep forcing and structural hashing
visit hash-trie order; hashing prepares each key and value without first deep
forcing the whole Map. Equality checks size before comparing entries and uses
boxed value semantics. Unknown JVM identity hashes remain an explicit ordering
limitation.

Paguro collision lookup and insertion have different hash guards. After an
object key's cache changes, insertion can add an internal table entry without
adding an insertion-order key. The index therefore stores table slots and
insertion order separately: public length follows key order, while equality,
hashing and deep forcing include the entire table. The heap traces every table
entry, including values absent from insertion order. Iteration hashes each
current key before executing its body; it does not prepare later keys first.

Completed key preparations accumulate across retries of one bytecode
instruction, including its probe-key and insertion-key phases. The scheduler
clears that state when the instruction completes, so later operations still
recompute strict collection hashes. A small scheduler dispatches collection
work and semantic continuations separately from the larger general instruction
handler to preserve bounded evaluator stacks.

`Map<K,V>` checks keys and values through the common type-check continuation.
Nested lazy collection casts can return wrappers; replacing such values creates
a distinct Map and clears inherited structural/hash metadata. Empty Map defaults
do not construct a default value for `V`. Map stringification reads completed
values without forcing nested objects, through the shared diagnostic value
renderer. General compound stringification and the remaining Map APIs
remain separate work.

Strict Set uses the same hash-trie index with separate table keys and
insertion-order elements. Constructor lookup precedes insertion; a duplicate
skips insertion, while a new element is prepared again in a separate bytecode
instruction. Both representations and pending builder/query continuations are
GC roots. Equality and structural hashing use the table, while public iteration
uses element order without rehashing. `Set<T>` eagerly checks elements and
retains the original Set rather than replacing its keys with lazy wrappers.
Set strings use the multiline non-forcing value renderer; Map strings and
diagnostic snapshots retain their single-line layout. Higher-order Set APIs and
JVM identity-dependent ordering remain separate work. Nested Map iteration can
rehash keys while Java renders a value; the cache-only snapshot renderer does
not yet reproduce every such side effect.

`Collection<T>` has a distinct abstract type node and no corresponding heap
collection kind. It accepts concrete List and Set values. Parameterized checks
reuse the common continuation: List elements can acquire nested lazy wrappers,
while Set elements are checked eagerly without replacing hash keys. Bare
`Collection` checks only the concrete class. Defaults are empty Lists; element
defaults are not constructed. Type names and normalized Reference metadata retain
the abstract `Collection` identity, including its covariant element type.
Executable checker equivalence remains separate from exported structural metadata.

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
Strict List and Map recompute their hashes on later operations. Lazy objects
recompute zero hashes, matching Java's cache sentinel.
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
the superclass and evaluating declaration annotations. Successful property
declarations publish in order, followed by methods; only `FinishClass` marks
initialization complete. A failure preserves
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
outside verified compatibility. Annotation construction can suspend class
initialization through an authored resource read. This source-language path now
uses the shared VM continuation; the earlier tests that inject instructions
remain separate scheduler evidence.

### Declaration annotations

Module, class, type-alias, property, and method declarations retain authored
annotation expressions. Bare annotations and annotations with bodies construct
ordinary typed objects after resolving the annotation type and checking its
`Annotation` ancestry. Class-valued aliases can supply that type; nullable,
constrained, and union types do not become annotation classes. The abstract
`Annotation` base cannot be instantiated.

Class initialization evaluates annotations after the superclass and before
properties and methods. Type-alias initialization publishes its singleton before
annotations and then resolves the definition. Both retain a successfully
constructed annotation prefix after failure. Property and method annotations
run before their signatures; a member's temporary annotation list is published
only with that member. Metadata stores traced heap values, so retained
annotations and captured environments survive collection without putting heap
handles in a shared `Program`.

Annotation fields stay lazy. Generator headers and spread sources run during
construction, and member validation can fail before a field is read. Annotation
scope uses the module const barrier, including eager construction expressions;
class-member annotations use the enclosing declaration frame rather than a class
instance. After resolving a method signature, initialization reads `message`
from the first annotation whose class is exactly `Deprecated`. Other annotation
fields remain deferred. This implements initialization timing, not the logger's
complete deprecation-warning behavior.

The shared builtin declarations cover `Annotation`, `Deprecated`, `Since`,
`Unlisted`, `AlsoKnownAs`, `DocExample`, and `SourceCode`. Deprecated's nullable
strings and SourceCode's nullable prefix/suffix default to null. The two
`Listing<String>` fields default to empty lazy listings. `Since.version` and
`SourceCode.language` remain undefined without assignment; the latter retains
the complete literal/String union, with no implicit Go default. Their checks use
the pinned `base.pkl` source for declaration frames. This does not synthesize
all upstream builtin metadata: the `@SourceCode` annotation on
`Deprecated.replaceWith`, full reflection, `ModuleInfo`, and converters remain
open.

Actual-source annotation Host tests retain partially initialized declarations,
completed annotation prefixes, and pending builders across collection and nested
contexts. They distinguish nonterminal VM cancellation from terminal Host
cancel, close, and timeout, including stale completions and release of outstanding
reads. The `annotation_host_probe` development adapter additionally compares
source-driven callback reentry and ordered results with Java. These paths do not
extend the scalar Node/Wasm adapters with Host I/O or establish complete module
initialization, module-cache, logger, general object `toString`, or `List.map`
compatibility.

### Object forcing, strings, and callback diagnostics

Object string conversion uses evaluator-owned continuations in `vm/forcing.rs`.
A persistent force marker is published before visiting members. Unfinished
ancestors clear their markers after failure or cancellation; completed children
and successful lazy member caches survive. An undefined value is caught only
inside the member-read boundary where Java permits it. Missing names and unrelated
callback failures do not become undefined values. Equality and hashing use the
same force engine with undefined values forbidden.

Forcing visits child declarations before parents and consults each declaration
owner's hidden metadata. Formatting visits inherited declarations in parent-first
order. The iterative shared renderer requests key hashes and equality comparisons
as keys are visited; runtime string conversion schedules those operations through
the VM, retaining pending values as GC roots across Host suspension. Standalone
strict collection strings retain their existing force behavior. Typed interpolation
dispatches the receiver's `toString` method, including authored overrides. Exact
class/type-alias modifier bits exclude definitions without confusing the import bit
produced by Java's repeated-modifier arithmetic.

Suspended callback contexts retain detached caller source frames, ordered from
the nearest context outward. Each context omits the waiting reader's unavailable
callsite. Reporting an inner failure does not unwind its suspended parent or reset
the parent's caches or force markers. The development Host adapter supplies real
REPL expression metadata and accepts additional expression files for deeper
callbacks while preserving its original invocation form. Dynamic missing-member
diagnostics reuse the shared suggestions helper and pinned inherited signatures;
diagnostic signature availability does not imply implementation of every method.

Synchronous diagnostic formatting still cannot suspend for incidental object-key
hashing that reads a resource. Resource caching also remains incomplete: repeated
method reads can deliver extra native callbacks. Successful nested Java reads can
fail inside `ResourceManager.computeIfAbsent` with a concurrent-modification bug;
those whole experiments remain oracle errors even when inner language-error frames
match. Trace logging, cyclic-rendering oracle errors, full module behavior, and
unknown JVM identity hashes remain separate compatibility obligations.

### Type aliases and function metadata

User type aliases have a separate evaluator-owned declaration cache. Initialization
publishes the alias value before evaluating annotations and resolving its
definition. A failed initialization leaves the value available for later
identity, name, class, and rendering queries;
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
These ordered execution plans do not establish a complete lazy runtime type-node
model. General imported types, complete annotation metadata, full module
initialization/caching, structural types, reflection, and the original upstream
type/function suites remain unverified or
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
collections derive their value defaults through a resumable VM task; each descent,
allocation, and unwind consumes fuel and traces unpublished objects. Typed defaults reuse the
already resolved evaluator-local class declaration, keeping its members lazy.
This avoids losing class defaults while compiling a reusable generic body.

Reference referent validation that depends on a symbolic type argument also
runs after substitution, at its authored resolution position. It precedes the
associated value evaluation and retains short-circuiting inside constraint
bodies. The validator walks structural metadata without entering executable
constraint bodies or their argument environments. Existing constraint-origin
frames remain available to the runtime; broader diagnostic gaps remain open.

Instantiated argument metadata uses immutable shared nodes. Substitution walks
the finite authored template and reuses already instantiated arguments. Cloning
an environment copies handles; iterative last-owner reclamation drains metadata
children without recursively dropping the graph. Concurrent last owners use
`Arc::into_inner` so exactly one owner reclaims each node. These nodes contain
declaration keys and type metadata, never evaluator heap handles. Cached
classification avoids revisiting an accumulated argument graph at every call.

Metadata comparison, validation, normalization, and formatting use explicit work
stacks. Reference normalization memoizes graph nodes and shares its output.
Exported metadata identity uses deterministic structural records instead of
recursively escaping nested strings; identity is independent of pointer sharing
and preserves alias declarations, arrow syntax, and exact constraint text.
Recursive type checking continues on the ordinary VM scheduler.

The original `this is A<List<T>>` abort remains preserved as historical evidence.
Small-stack tests now exercise deep graphs, concurrent release, extended running
evaluations, cancellation, and terminal cleanup without that abort. The adapter's
existing instruction budget also stops the original input. These are resource
and lifecycle results, not Java-compatible stack-overflow diagnostics. Reference
normalization, validation, and some metadata comparisons still run within one VM
instruction; textual rendering can expand a shared graph. Uniform wall-time
cancellation bounds and execution-time/peak-memory parity remain unproven.

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

For rejected non-local object annotations, const modifiers and inferred parents, the resolver obtains
the diagnostic owner through a single replay of its iterative worklist. Ordinary
resolution allocates no diagnostic-owner stack. The replay preserves the
original error unless every diagnostic field except the captured member is
identical. Its private naming scopes retain UTF-16 paths, quoted identifiers,
literal entry labels, lambda and entry ordinals, and Java's generator-prefix
revisit behavior. This is diagnostic metadata; it does not evaluate rejected
types or values, change numeric operations, or replace existing runtime owners.
Const-modifier rejection uses the enclosing owner, before entering the property
scope. Inferred-parent rejection retains the current expression owner. Method
bodies permit contextual `new {}`, while lambda bodies and parenthesized object
expressions retain their rejection. Untyped and explicit `Dynamic` method returns
use the existing Dynamic default; class and collection return-type inference
remains incomplete.

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

### Nullable defaults

A nullable value can retain the inner type's default without exposing it as the
value itself. `Value::NullWithDefault` references an evaluator-owned heap entry;
the collector traces its payload through objects, closures, pending defaults,
and suspended frames. Equality, key hashing, type checks, and rendered output
treat both null representations as `null`. No reference-counted value chain is
introduced.

An empty amendment returns one attached default. An amendment with members
unwraps nested nullable defaults until it reaches the parent to amend. A null
with no attached default uses an empty Dynamic parent. Null-propagating member
and method operations return plain null, discarding the attachment. Nullable
generic collection defaults use the existing resumable default constructor;
each wrapper construction consumes VM fuel and retains its payload as a GC root.
These rules follow the pinned `VmNull`, nullable type node, and object literal
implementations. Parent unwrapping still runs within one instruction, so these
changes do not establish a uniform wall-time cancellation bound. General generic
constructor cases and synchronous metadata resource bounds remain separate requirements.

### Generic constructor parents

A constructor such as `new T { ... }` inside a generic constraint resolves its
parent from that invocation's actual type argument. It uses the resumable default
constructor, then the ordinary amendment path. Intermediate defaults remain GC
roots and descent, allocation, and unwind consume VM fuel. Captured functions
retain the argument environment through Host suspension and retry. Inline and
lambda constructor nodes receive cloned alias arguments; lazy member call
targets retain the unbound interpretation used by Java. Their remaining logical
frame-name differences are tracked separately.

Obtaining a parent does not evaluate the type's constraints or introduce a
collection cast. Pinned Java cases distinguish `new T` with an actual
`Listing<Int>` or `Mapping<String, Int>` from an explicitly checked collection:
the constructor can contain values outside those arguments. Class property
annotations still apply when their members are evaluated. Empty and nonempty
amendments retain the nullable-default rules above.
Explicit generic alias syntax, such as `new Identity<T>`, additionally checks
the constructed result against that alias's type.

Diagnostic origins travel separately from type identity. Generic argument
source locations allow constructor failures to point to the actual type when
Java reports an abstract or external class, while retaining the authored
constructor context for other failures. These source locations do not become
heap roots or participate in type equality. Selected constructor comparisons
and lifecycle checks do not establish complete generic alias identity, imported
type behavior, or resource and performance bounds.

### Recursive diagnostic frames

Uncolored frame rendering follows the pinned Java compressor, including partial
patterns and count-one loops. Frame equality uses the URI, member name, complete
source lines and UTF-16 positions, before display transformations. Loop margins
and the first hint are rendered together. Selected recursive and mutual-alias
failures exercise this path with power assertions enabled and disabled; general
stack overflow, colored output and the complete diagnostic corpus remain open.

Diagnostic member paths retain `PklString` values from parser/resolver/compiler scope
tracking through block metadata, suspended runtime frames and compression keys.
Counters for numbered entries and functions use the same lossless parent name.
An unmatched surrogate and a literal `?` therefore have independent child
numbering and distinct compression identities, even when the final Java UTF-8
writer displays both as `?`. Valid surrogate pairs and literal supplementary
characters remain equal. Source URIs and ordinary identifiers keep their existing
UTF-8 representation. Detached errors own their names without retaining heap roots.
Delayed integer-literal validation records its authored entry or element scope
without changing when the error is reported. Local typed-property checks in
anonymous object bodies run inline in the property frame. Module and class
declarations retain the separate value-call frame, including local declarations.
Compiler metadata records this declaration origin; the local lookup flag alone
does not determine diagnostic call structure.

Value-call frames use the executable expression's source span separately from
its authored syntax. Removing outer parentheses and folding a directly negated
numeric literal follow the pinned Java AST builder: the folded value retains
the unsigned numeric token's span. A unary operation on a parenthesized operand
keeps the unary expression's span. Typed local properties and function bodies use
this executable span. Other constant members follow Java's optimization that
stores a value without a member node: their diagnostic span is reconstructed from
the source suffix after the header. This preserves authored grouping, signs and
comments in public properties, overrides and entries. Element headers use the
lowered expression, so a grouped constant can leave only closing parentheses in
that suffix. Scalar constants and the implemented constant List/Set/Map
constructors share this metadata policy. Authored AST spans, resolution keys,
instruction locations and power-assertion observations retain their original
identities. This metadata rule does not change arithmetic, evaluation order or
declaration ownership. Java's unavailable source sections for empty collection
calls remain a separate diagnostic gap.

### Shared JSON rendering and reference identity

JSON and PCF use the same frame-owned renderer continuation, lazy forcing,
converter calls, Host suspension and tracing heap. Render directives force their
text through that path. The pinned `pkl:json.Property` and `Value` declarations
provide annotation and converter behavior through the ordinary module arena.
The [renderer contract](m2-json-rendering.md) records ordering, UTF-16 handling
and the remaining parser and module-admission boundaries.

Numeric, String and Bytes carriers retain a semantic reference token separately
from payload equality and hashing. Shared Bytes storage does not imply the same
language wrapper. Primitive local-slot representation belongs to an activation;
descriptor history belongs to its executable. Boxing a primitive read follows
the pinned Java Long-cache and Double-allocation rules. Generalizing a descriptor
does not retrospectively replace old captured primitive slots. Generator copies
preserve the applicable descriptor and physical-slot distinction. Constant
property references are independent of optional Observe instructions.

Private source-backed base initialization warms ordinary PCF output before user
module execution. Architectural GC checks distinguish this retained base graph
from reclaimable user modules and transient values. They separately check
executable retention, stale user handles and zero slots after closure. Source
module initialization may yield before the first user operation; its polling
conditions are recorded independently of later Host deadlines and stack limits.

Optional runtime hints and synthetic diagnostic underline widths live in the
existing boxed metadata types. A hint-only runtime error can allocate its optional
details box; the diagnostic provenance box already exists. These layout changes
do not establish an execution-time or memory improvement. The detached logical
error fields determine equality;
allocating an empty metadata box does not change it. This representation preserves
the shared diagnostic text, source spans, UTF-16 widths and hint precedence.
Selected differential regressions do not establish complete reference identity,
standard-module lifecycle, renderer coverage or the performance targets.

### Import admission and module cache ownership

Authored imports execute a typed `CheckImport` Host request before consulting
module URI caches. The request retains the importing module's original and
resolved identities and the prepared target URI. Trust checks use the resolved
importer. A successful import node caches the returned module in its executable
state; a later invocation of that same site returns the saved receiver before
another admission request. Distinct sites still check their edges even when the
URI cache already contains the module. Failure does not publish a successful
site result. A site that returned a partial receiver during a cycle can retain
that success after another initialization path stores a module failure.

Singleton classification and authored edge identity are separate. For selected
standard-library keys, the pinned static-module list includes `pkl:base`,
`pkl:ref` and `pkl:test`; those modules bypass ModuleKey resolution, but authored
imports still check their import edges. Decoded scheme-specific names select the
canonical singleton receiver, so supported escaped names and fragments do not
create another singleton. A custom Host key with the same URI spelling remains
a custom key and uses the ordinary cache path.
General bundled modules such as `pkl:json` use original-key cache lookup,
`Resolve`, resolved-key cache lookup, and initialization from the internal pinned
source arena. A resolved-key cache hit does not seed a new original alias.
Bundled source bytes are not supplied by an external `LoadSource` callback.
Qualified external Ref types force their authored import alias before applying
the existing specialized type metadata and constraints.

The native, N-API and Wasm surfaces use the same requests and continuations.
`completeModuleImportCheck` and `rejectModuleImportCheck` complete admission.
`completeDefaultModule` is an explicit opt-in to the shared default import trust
and supported bundled-resolution profile. It returns `false` for an unsupported
pending request without consuming its token; invalid tokens, deadlines and
terminal sessions still fail normally. Custom providers run before delegation.
JavaScript adapters do not implement trust rules or silently allow every import.
They preserve actual module requests separately from their existing projected
observations. Raw Host rejection remains distinct from typed
`SecurityManagerException` completion. Typed security failures from admission,
resolution and `HasElement` retain guest exception provenance; source and
capability phases reject that completion kind without consuming the request.

Import continuations and successful executable-site entries trace the imported
receiver. They do not retain a temporary importing activation. Module URI caches
and VM-owned static executable descriptors have separate roots. Architectural
regressions exercise temporary receiver reclamation, repeatable retained graph
sizes, reentry and error retry, pending-phase cancellation, stale tokens and zero
slots after closure. Source-specific lifecycle baselines account for qualified
Ref type initialization occurring before the first user operation. The four-value
bootstrap attribution is derived from source (module, two annotations and Access
class), not an object-ID dump. Actual tests separately verify the exact retained
counts and unchanged Host/deadline/close assertions.

The Stage48 supplied-primary profile does not reproduce the complete Java
primary-module and per-expression module lifecycle. Stage49 adds separate key
selection and cache-policy requests, key-kind dispatch independent of URI
spelling, eager clause-factory preparation, and public ModuleSource operations.
Clause preparation can select a key even when the runtime import edge is never
executed. Query and module executable owners follow active and captured heap
environments; unreachable payloads can be retired without invalidating surviving
declarations. The Stage49 representation still retained non-tail storage slots.
See [module keys and public evaluation](m2-module-keys.md).

Stage50 replaces that representation with a fixed prepared prefix and dynamic
resident segments. Logical addresses advance independently of resident element
counts; retiring a segment removes its directory entry and owned vectors without
reusing committed addresses. A traced executable owner may retain several
segments, including a module's clause-preparation shell and compiled body.
Public-operation entries have independent retirement boundaries. Active and
suspended frames, escaped classes, aliases and closures retain their code until
their final root is gone.

Compilation checkpoints seal allocation boundaries and restore replaced module
units and module metadata on rollback. Only unpublished compiler addresses can
be reused. Copy-on-write segments keep surviving program snapshots intact;
mutable suffix traversal avoids cloning excluded prefixes, and failed lookups
do not clone an unrelated segment. URI preparation first identifies blocks with
actual URI changes, then mutates only those blocks. This includes earlier module
shell/body segments without copying unrelated owners held by a snapshot.
Resident storage and nested-capacity plateau tests compare 20 and 200 ordinary
and reentrant uncached module evaluations. They are separate from whole-process
RSS and Java performance comparisons. The frozen candidate has completed the
five required Rust checks and focused native, Node and Chromium captures; its
full selected language and persistent-evaluator regression remains separate.

Complete factory behavior, URI semantics, import globs and public evaluator
policy counters remain required work. Full ordered event traces are separate
from selected API output comparisons. This implementation does not establish
full module-policy or M2 compatibility.
