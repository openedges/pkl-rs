# Architecture

The goal is to reimplement Pkl 0.32.0 in Rust, using one shared engine for Native, Wasm, and N-API. Milestones M1–M6 and the **M7 local macOS `arm64` verification & runtime optimization checkpoint** are implemented across the shared pure-Wasm engine (`pkl-runtime` + `vendor/java_regex` + `pkl-formatter`, `unsafe_code = "deny"`), the dedicated native Host crate (`pkl-host-native`, `unsafe_code = "deny"`), the native product crates (`pkl`, `pkl-cli`, `pkl-repl`, `pkl-server`, `pkl-doc`, `unsafe_code = "deny"`), and the product WebAssembly and Node.js N-API bindings and distributable packages (`crates/pkl-wasm` + `packages/pkl-wasm` as `@pkl-rs/wasm@0.32.0`; `crates/pkl-napi` + `packages/pkl-napi` as `@pkl-rs/napi@0.32.0`, preserving 100% of M1 experimental exports), covering the language evaluator, all 23 `pkl:*` standard library modules and 542 external builtins, all 9 output renderers, `output.files` multi-file output, `PklBinaryDecoder`, `Reference`/`converters`, `Regex` `(?c)` / `CANON_EQ`, Host security/HTTP/HTTPS/CA/proxy/`modulepath`, external readers (`0x26–0x32`), `PklProject`/MVS/packaging/`package-2` caching (including all 29 Apple Pkl Pantry packages in dev and packaged form), the full M5 native product surface (public Rust Evaluator API, unified `pkl` CLI, interactive & pipe REPL, `0x20–0x32` MessagePack evaluator server, pure-Wasm CST formatter, and `pkldoc` generator/migrator with `17/17` vendored static web assets and live `stdlib/doc-package-info.pkl` support), unified browser/Node Wasm and Node N-API async evaluation, VFS, lossless JS<->Rust value transport, and M7 runtime optimization & memory-reclamation mechanisms (`BaseSnapshot`, `FastIntOp`, compact `list_elements`, GC-shrinking Fibonacci-hashed `ExecutableRoots`/`FastIntCache`, automatic inter-op tracing GC, `try_fast_encode_pkl_binary`, shared `ColumnarValueBuffer`, Wasm `_disposeWasmIfClosed()`, and `wasm32` `inline(never)` V8 Liftoff outlining). Full release readiness remains honestly `release_ready: false` pending cross-OS/browser/hosted-CI matrix verification (`darwin-x64`, `linux-x64-gnu`, `linux-arm64-gnu`, `win32-x64-msvc`, Firefox, WebKit, `hosted-ci`) and resolution of the `2/34` Chromium multi-process browser-tree RSS rows (`5.415x`, `5.581x`). See the [M1 evidence](../reports/m1/README.md), [M2 Stage 161 report](../reports/m2/stage161-m2-closure/README.md), [M3 report](../reports/m3/README.md), [M4 report](../reports/m4/README.md), [M5 report](../reports/m5/README.md), [M6 report](../reports/m6/README.md), and [M7 report](../reports/m7/README.md) for measured limits and decisions.

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

### Static analysis and native file capabilities

`modules::analyze` exposes typed key-selection, resolution and source-loading continuations. It parses imports without compiling or evaluating loaded guest module bodies. Original graph URI spelling, normalized factory requests and provider-owned loading identities remain distinct. The current graph builder is eager; Java's lazy graph allocation and identity-dependent sibling effect/error order remain unqualified. Static analyzer glob traversal is Unsupported, and complete standard-library source service remains unfinished.

Native development Hosts supply file Glob/HasElement metadata while matching, ordering and lazy value loading stay in the shared VM. Authorization and decoded file-URI validation precede filesystem I/O. Selected authored settings modules use explicit POSIX/Windows lexical path operations rather than invented Host descriptors; full project/package support, `platform.current`, `release.current` and Benchmark execution remain open. Public `Instruction` adds `ResolveEvaluatorSettingsPath` and `AnalyzeImportGraph`, requiring downstream exhaustive matches to handle them. These bounded shared-engine capabilities do not establish complete browser/Node Host or language/API coverage.

### Package URI validation and development file paths

`packages::PackageUri` and `PackageAssetUri` provide shared pure validation with typed input/reason ownership. Standard module/resource handling retains constructor/policy order; valid package transport, project resolution and full JVM diagnostic projection remain separate capabilities. Native development file handling distinguishes the exact Unix `file:///` byte route from Java `File(URI)` text decoding, sharing Unicode/UTF-8 primitives while keeping OS bytes distinct from display text. Non-UTF-8 module filename/source loading and Windows provider behavior remain unqualified. These bounded helpers preserve custom Host ownership and add no production Java fallback. See the [checkpoint scope](../reports/m2/stage153-155-t40-uri/README.md).

### Package loading, caching, and project resolution

`packages` implements pure package and project resolution for `package://...` and `projectpackage://...` URIs:
- `packages::metadata::accept_metadata`: Validates package metadata checksums, enforces Java string units, and checks ZIP URL scheme constraints.
- `packages::cache`: Package cache abstraction for packages under `~/.pkl/cache/package-2/` or user-configured cache directories, cleanly abstracted behind `PackageHost`.
- `packages::http`: Package transport protocol definition with checksum validation against corruption, decoupled from host networking via `HttpHandler`.
- `packages::zip`: Extracts and verifies ZIP archive contents safely, isolating package assets.
- `packages::project`: Discovers and parses `PklProject` configurations and `PklProject.deps.json`, managing active project dependencies and CLI `--project-dir` paths without static state.
- `packages::resolver`: Resolves dependency import URIs to local cached file paths via injected `PackageHost`.

All global mutable statics (`OnceLock<Mutex>`) and direct filesystem / process spawning operations have been completely purged from `crates/pkl-runtime/src/packages/`, preserving pure `wasm32-unknown-unknown` compilation while allowing host adapters (native CLI, Node, browser) to inject platform-specific I/O capabilities.

These capabilities are integrated into `modules::standard_keys` and `vm::uri_resolution`, ensuring standard URI resolution delegates package lookups transparently.

### Java exception provenance projection and nominal class identity

`java_exception::provenance` projects runtime evaluation errors into Java-compatible `PklBugException` and foreign exception structures:
- Preserves raw Java stack trace framing and class name conventions.
- Preserves foreign nominal type identity in `value.rs`, ensuring type mismatches involving foreign types produce exact matching diagnostics (`Expected value of type form#Label, but got type foreign#Label`).
- Prevents internal Rust panic leakage into guest evaluation output.

### Standard runtime modules: platform, release, and benchmark

The runtime exposes mirror implementations for built-in standard modules:
- `pkl:platform` (`vm/platform.rs`): Provides `platform.current` properties including `operatingSystem` (`name`, `version`), and `processor` (`architecture`), supplied via host metadata injection.
- `pkl:release` (`vm/release.rs`): Mirrors Pkl release information (`release.current.version`, `documentation.homepage`).
- `pkl:benchmark` (`stdlib/Benchmark.pkl` and `stdlib_sources.rs`): Integrates the standard Benchmark module into the evaluator's built-in stdlib registry with genuine expression evaluation.

See the [Stage 158 Remediation checkpoint](../reports/m2/stage158-m2-remediation/README.md) for full remediation and integration evidence.

### Typed property lookup, module identity and bounded annotation/const diagnostics

`Typed.getProperty` uses the existing String-checked property lookup continuation and actual Typed inheritance. It preserves lazy lookup, existing const-method restrictions, missing-member suggestions, receiver labels and UTF-16 property names. It shares the evaluator, diagnostic renderer and heap with native, Wasm and N-API.

`Module.relativePathTo` works with original selected ModuleKey identities through the common URI/module machinery. Finite verification Hosts supply pinned synthetic/file sources, capabilities and exact resolution descriptors; they do not reinterpret module URIs as filesystem or network permissions. Java normalizes factory lookup keys without decoding percent escapes. The selected Host recognizes the three recorded normalized synthetic identities and the one normalized primary identity while preserving authored evaluation/source bytes. Supplied primary text remains owned by the shared cached/supplied-text path.

Portable entry validation delegates the primary file/http/no-loader policy to the same finite Host validator before constructing a session. Unsupported configured `repl:` primaries receive a genuine no-loader completion. HTTP primaries retain nonlocal hierarchical capabilities. Unknown modules and unconfigured resource requests remain unavailable; only the recorded environment/property requests receive missing completions.

Annotation/const diagnostics are verified against their exact authored Java outcomes. The required Java const-lambda internal failure remains unresolved. These bounded probes and their raw results do not imply complete diagnostic, language or API compatibility, and correctness acquisition profiles are not performance measurements.

### Instantiated type sources and entry scopes

Type diagnostics retain immutable source sections through resolution. Diagnostic
value equality compares complete source provenance and span; union grouping uses
explicit source-node identity. Physical frame URI, diagnostic text and logical
declaration owner remain distinct. Entry counters belong to object scope; a scratch
compiler visits ordinary entries before a generator. Its Program-copy cost remains
to be measured against performance targets.

### Base failures and diagnostic ownership

`Undefined()` carries a receiverless undefined-value marker. The common VM
preserves undefined catching and inferred-parent behavior while reserving
renderer-path hints for failures associated with an undefined property receiver.
Unavailable source information suppresses only the original throwing frame;
an authored boundary that replaces the error remains visible. `TODO` and
`Undefined` use compiled base-module methods, so their reflection metadata and
execution bodies share the same declaration.

Diagnostic labels quote individual member names according to the parser's
identifier rules. Raw class names remain separate from these qualified scope
labels. Lambda arity validation occurs before entering a new function scope and
attributes the error to its enclosing declaration. Scalar custom receivers use
the same `Any.getClass` dispatch as other runtime values.

Regex display and PCF rendering share the single-line custom-delimiter formatter
used by the pinned Java implementation. Renderer indentation and ordinary String
formatting options do not change the Regex pattern representation. These rules
live in the shared engine used by native, Wasm and N-API; their bounded evidence
is recorded in the Stage114 checkpoint.

### Pair dispatch and superclass use sites

`Pair` stores its fields in a specialized heap representation but inherits from
`Any`, not `Typed`. Method selection excludes Pair from Typed conversion and
lookup dispatch before evaluating arguments. Missing-member suggestions use
the pinned Pair and Any declarations. Native inherited Any calls record the
resolved base declaration for arity errors after evaluating authored arguments.

Superclass resolution uses a dedicated deferred instruction carrying the
authored type label. It retains the use-site span when an alias resolves to an
invalid superclass, without changing annotation or default-instance class
resolution. These behaviors are shared across native, Wasm and N-API; the
[Stage121 evidence](../reports/m2/stage121-pair-supertype/README.md) describes
their tested boundary.

### Lossless source, identities and glob suspension

`SourceText` retains authored UTF-16 units in `PklString`. Scalar text uses its
existing UTF-8 storage; a source containing unpaired surrogates also has a WTF-8
byte address space for the lexer. Surrogate encodings cannot collide with valid
Unicode scalars. `SourceSlice` and the source index preserve the relationship
between lexer spans, original code units and Java UTF-16 columns. Conversion to
scalar-only text is explicit and may fail; diagnostic rendering uses the
retained original units.

Module/resource URIs, source names and diagnostic metadata use the same lossless
representation across the runtime and both bindings. Development adapters that
require scalar filesystem paths reject unrepresentable inputs explicitly.
Java UTF-8 output and raw UTF-16 channels remain separate comparison surfaces.
Partial-module minimum-version errors retain the declarations available before
a later parse failure, and imported errors retain their authored source owner.

Glob expansion runs in the common engine and suspends through `HostPoll::Glob`.
Typed provider requests and completions share the session's cancellation,
reentry and close handling. Provider input order is retained, while expansion
ordering follows the Java UTF-16 contract. Native, N-API and Wasm transport this
shared state machine. Malformed URI/internal-error behavior and missing Host
fixture prerequisites remain required gaps; typed transport alone does not
establish complete glob or external-I/O compatibility.

Opaque glob traversal compiles the pattern before validating the URI passed to
the reader. Absolute patterns use the authored URI; relative patterns use the
enclosing scheme's dummy URI. A failed validation prevents `list-elements` from
being requested. The error retains the original UTF-16 input, syntax reason and
known UTF-16 index in rare diagnostic metadata. Java's internal exception
rendering and catch behavior remain unimplemented, so this path reports
`Unsupported` rather than claiming matching Java diagnostics.

### Java exception rendering component

`java_exception` renders an explicit throwable graph using shared `PklString`
values. Throwable identities preserve cause cycles and repeated suppressed
references; frame equality retains Java metadata distinctions while ignoring
display-only flags. Iterative traversal renders shared suffixes, descriptions,
URL encoding, and the Pkl bug envelope. Release information and line separators
are supplied by the caller. Ordinary guest catch uses the same extracted
single-line transformation as before.

The Stage130 component tests compare 54 synthetic graphs under LF and CRLF
against the pinned Java renderer in both Java UTF-8 and original UTF-16. They
do not establish runtime stack capture. The evaluator still needs typed bug
payloads, compiler-node ancestry, execution-state-aware frame projection and
explicit invocation profiles before recognized internal errors can use this
path. Unsupported URI failures remain outside ordinary guest catch. No Java
fallback is added to evaluation.

### Shared math and Resource defaults

The `pkl:math` bindings use shared Rust kernels with explicit fdlibm operation
order where required by the pinned Java behavior. Raw floating-point bits,
decimal rendering and complete errors are verified separately. Resource digest
defaults delegate to the existing shared Bytes operations through `toBytes`.
These paths contain no Java evaluation fallback. Bounded correctness results
do not establish the execution-time or memory targets.

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

### Native expression-string probes

`language_probe` and `runtime_probe` use the shared module evaluator with the
caller's unchanged expression. Native ModuleKey and ResourceReader helpers perform
file I/O; module initialization, query parsing, `output.text`, automatic `ToString`
and diagnostics stay in the shared runtime. The adapter uses a noop Logger,
default frame transformer, empty environment/property maps and the explicit power
assertion flag. It converts lossless UTF-16 results with the Java UTF-8 writer rule
only at the process boundary, and closes the evaluator on every terminal path.
Acquisition tools own process deadlines; the adapter adds no instruction ceiling.
The legacy scalar facade and portable `evaluateLanguage` API remain separate,
bounded experiments. See the [Stage97 evidence](../reports/m2/stage97-token-expression-checkpoint.json).

### Shared collection operations and suspended evaluation

List callbacks/searches, strict Map operations and lazy Mapping lookups/type
wrappers use the common VM, values, tracing heap, diagnostics and Host
continuations. Callback owners, partial builders, retained keys and typed lookup
state remain evaluator-owned across suspension, retry, reentry and collection.
Cast-insertion bookkeeping stays separate from observable error fields.

Lifecycle checks force nested lazy members when a pending Host read is required
and verify retry, token, cancellation, reentry and cleanup. These Rust invariance
checks remain separate from Java-derived expected bytes. Selected adapters use
the existing Host module continuations to supply the exact pinned auxiliary
source to two importing programs; the remaining 72 receive empty registries and
unknown URIs are rejected. Shared module resolution and caching remain in the
engine. Production evaluation has no Java fallback.

Current targeted, workspace, bounded M1 and selected native/N-API/Chromium
records cover their stated scopes. The original Map replay
exposes an existing missing `Typed.hasProperty` route after successful
`Map.toTyped`; implementing that API remains required work.


### Shared numeric and Set API operations

Recorded local evidence: [Stage99 checkpoint](../reports/m2/stage99-numeric-set-apis-checkpoint.json).

Stage99's 13 additional numeric and 52 Set routes use common values, builtin
frames and type-check/Host continuations. Numeric identity/conversion, bitwise
shifts, UTF-16 character results and fixed formatting use the pinned Java rules.
Set predicates, slicing, folds, extrema, sorting and algebra retain persistent
storage, callback order and roots across suspension and collection. No platform
adapter substitutes another evaluator.

Set comparator result checking accepts Boolean or deprecated Int results.
Invalid results use Java's two-class mismatch diagnostic (`Boolean` or `Int`),
separate from declared union-type diagnostics. The null branch omits a value
line. Foreign Map.Entry storage remains `[key, value]`; stringification loads
key at reverse-local depth 1, then value at depth 0, preserving `key=value`
order and both child continuations. Foreign-entry Pkl method/equality internal
protocols remain explicitly unresolved. The fixed-decimal estimate keeps the
exact JDK constants with a narrow Clippy allowance; byte parity uses the
equivalent `!is_multiple_of(2)` expression.

Freeze04 targeted04 matches 2,430 ordinary observations and 1,238 Logger events.
The complete workspace has 1,418 passes, zero failures and three ignored tests.
Its Fresh normal runtime originates in actual targeted04 compilation; the two
module/semantics probes are newly compiled. The source/artifact schema admission
is read-only and does not establish platform behavior. The terminal default
driver passes all eight roles, comprising seven new commands and prevalidated
Clippy02. Original Float/Int modules match Java; the original Set module still
fails on its List `minWith` call at line 249. This exposes a remaining shared
List route dependency and requires unchanged-original replay after integration.
M1 strict and architecture gates now pass with 6,078 matching cases per target
and across the shared engine. Each selected Native/N-API/Chromium target
completes 2,436 observations across 46 sessions: 2,430 ordinary observations
and 1,238 Logger events match, while six required internal observations remain
unresolved. Portable sessions close with zero retained slots/depth and stable
retained payloads; actual process ownership is terminal. Current native-adapter
rustc exits 0 despite a recorded LLVM-loader
strip warning, so successful stripping is not claimed. M1 uses its explicit
scoped loader environment. Three required foreign-entry internal
contracts, earlier glob protocols and whole-language/API/release/performance
obligations remain open.

### Shared String API operations

The 28 additional String routes use the common VM, lossless UTF-16 values and
ordinary builtin call/type-check frames. Predicate slicing keeps callback,
source and current values rooted across suspension, retry and collection.
Multiargument operations load declaration-order parameters through the VM's
reverse local-stack offsets. No platform adapter substitutes a Java evaluator.

Lowercasing uses pinned JDK Character data and an extracted word-break table for
the `Locale.ROOT` final-Sigma rule. Conversion preserves result identity, including
unchanged surrogate-bearing strings and uppercase no-change inputs. Parsing
retains Pkl underscore handling, Java numeric grammar, UTF-16 digit behavior and
hexadecimal overflow sequencing; trim/classification, padding and replacement
reuse common values, constraints and diagnostics.

The current source dictionary includes the word-break data/provenance and String
fixtures. Actual Native/N-API/Chromium comparisons match 2,412 comparable
observations per target after attempting all 2,418 rows. The three glob internal
protocols remain required unresolved. Runtime opt1, portable release cores, raw
Rust Wasm and generated bindgen Wasm/JS have distinct identities and producer
chains; reused artifact hashes are qualified as admission-time observations.
The process-scoped LLVM loader adjustment changes the local build environment,
not the shared evaluator or pinned toolchain. The earlier N-API debug-strip warning
is preserved. See the [Stage98 evidence](../reports/m2/stage98-string-apis-checkpoint.json).
Selected raw equality and lifecycle checks do not establish whole-API or
performance completion.

### Scalar conversion, Set removal and Function.applyToList

`Float.toInt` explicitly rejects non-finite or out-of-range values before Rust's
truncating conversion. `Int.toRadixString` executes the pinned constrained Int
parameter through the ordinary resumable type-check path, including power
assertions; unsigned magnitude handles the minimum signed integer. `List.getOrNull`
checks bounds before narrowing an index and returns the cached Value unchanged.

`Set.restOrNull` shares the equality continuation and repeats both upstream
contains/removal hash and equality passes. Persistent trie deletion preserves
collision ordering, packing and visible-vector/backing-table differences. Result
cache publication appends one value per resumed step so it cannot replace a
pending equality continuation. A transition to an unknown JVM identity-dependent
hash uses the existing conservative policy; it is not an exact identity-hash model.

`Function.applyToList` checks the List parameter in a pinned builtin frame and
invokes the original function target. Existing call frames retain captures,
arguments, type checks and Host/GC lifecycle behavior across suspension. Public
native methods are not assumed to be first-class Function properties.
See the [Stage96 evidence](../reports/m2/stage96-core-apis-checkpoint.json).

### Reversal and constraint constructor dispatch

String reversal operates on lossless UTF-16 units and repairs reversed surrogate
pairs according to the pinned Java operation. Malformed low/high sequences can
become pairs, so reversing twice is not assumed to restore every malformed
string. List/Set reversal uses the shared resumable projection builder, preserves
member identity without forcing nested values, and produces a neutral List.
Canonical empty results and retained elements have separate ownership tests;
source and producer scopes can be reclaimed after their last use.

Custom-this constraints keep lexical method lookup ahead of supported base
constructor lowering. Authored module/class-local methods and externally
supplied evaluator expressions retain distinct resolution contexts. Missing
scalar members and List bounds reuse common diagnostics; rendering the bounds
value does not force nested members. Union marker validation remains a parser
operation so its first-error ordering is shared across targets.

### Super defaults, qualified types, and const owners

The shared VM now includes the Stage69–72 integration for super entry defaults, import-qualified types, parameterized aliases and VarArgs, compiled `pkl:base` alias/Mixin identity, and successful const owner caching. A missing `super[key]` uses the receiver's default; a present parent member avoids that fallback. Type argument preparation preserves the pinned Java error order and declaration environment. Successful inherited const values are forced on their declaring owner and then cached on the receiver; failed or cancelled evaluation does not publish a receiver value.

The force continuation is boxed and retains receiver roots across Host suspension. The return path is separated from the main VM dispatch to preserve the existing 256 KiB stack controls. The integrated source passed all five required Rust checks and selected native/N-API/Chromium comparisons. The [implementation checkpoint](../reports/m2/stage69-72-implementation-checkpoint.json) records the full ready-corpus replay and its remaining failures. Reference alias diagnostics retain the authored alias frame when an existing error recipe is reused. Full language/API compatibility and performance remain open.

## Host and Public APIs

Provide filesystem, HTTP, package, process, and external reader capabilities through the Host interface. The core must not depend on CLI or Wasm adapters. Evaluation waiting on a Host operation must be resumable from the same execution state, with shared definitions for cancellation, timeout, and close behavior.

Wasm uses facilities such as a browser VFS and asynchronous loaders. Explicitly identify capabilities that the Host does not provide, and do not count them as passing full compatibility tests. N-API must transfer large integers and bytes without loss, and performance measurements must include conversion costs between Node and Rust.

The public scope includes the native Rust evaluator API, CLI, REPL, server protocol, project and package features, formatter, and pkldoc. Java/Kotlin code generation and a JVM public API compatibility adapter are explicitly out of scope (2026-11-20 intent update); Java 0.32.0 serves solely as the compatibility oracle and performance reference.

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

Classes and closures live in the evaluator's existing traced arena. A Class
descriptor traces its instance prototype and superclass descriptor; the prototype
owns the declaration's members, annotations, and partial initialization state.
Prototypes and instances trace their Class descriptor. Immutable programs retain
declaration keys, not evaluator heap handles. The VM roots its canonical builtin
classes while user classes follow their actual object/code ownership lifetimes.
Class identity and inherited member checks are shared across all three runtime
targets. Representation-based type checks and exact-class versus typed-ancestor
converter lookup remain distinct from the superclass graph; see the
[Class-value contract](m2-class-values.md). Type aliases resolve in
their declaration scope; generic arguments resolve at their use site. The
timing of class initialization and type validation is part of compatibility,
including declarations whose errors remain unobserved until they are used.

Object-body method signatures initialize when their method value is forced.
Local lexical calls evaluate arguments before obtaining that call target, while
module and class declarations retain their eager signature checks. Type lookup
uses the declaring lexical module, including an inherited member executing with
a different receiver. Diagnostic origins distinguish an inferred entry parent
from an explicitly authored parent expression.

Collection `every` uses a shared resumable cursor. Strict collections iterate
their stored values; Listing and Mapping force only the visited values and stop
after the first false result. The cursor retains the receiver, callback, keys,
and current argument across Host suspension and GC. It allocates no result
collection, and its boxed state keeps the common VM frame size bounded.

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

If, let, lambda, property-only object bodies, and amendment parent chains also
use continuations. Mixed-body amendments schedule their parents on the same
worklist; their member and generator compilation still uses recursive helpers. Type parsing
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
type. Inference follows `if` conditions and branches, `trace`, and let bodies;
parentheses, intervening operators, lambdas, and let initializers keep their own
boundaries. Type defaults are created only
when execution reaches `new`, before amendment members run. Literal `unknown`
retains the Dynamic default; `Any` and aliases of `unknown` do not acquire it.
Methods without a return annotation use the canonical empty Dynamic parent,
including its prototype. Assigned collection entries and elements obtain their
inferred parent from `receiver.default(key)`. Bare entry amendments instead read
an inherited same-key entry before falling back to the default function.

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

Parent defaults use the resumable `GenericDefaultRun` worklist. Successful
method-parent defaults are cached by compiled block and instruction position,
not by receiver. The cache is rooted through the owning code arena and retired
with unreachable query code; closing the evaluator releases it. Parent selection
can succeed and remain cached even when the later amendment body fails.
Explicit constructors only cache final type defaults, and parameter-dependent
constructor sites retain their invocation-specific arguments.

A successful builtin collection constructor can turn a weak canonical empty
Dynamic, Listing, or Mapping into a strong code-owned parent. This root is
distinct from the temporary amendment object and its captured environment.
Builtin collection prototype backlinks provide class and inherited-member
information; structural equality and hashing still use collection members,
while Typed and module values retain their class-declared member rules.

The pinned module-type node chooses finality at its first resolution: a closed
module caches its class, while an open or abstract module retains a receiver-
dependent self type. Alias checks use the declaration context; default creation
retains the invocation receiver. These are distinct operations even when an
alias has been expanded by the compiler and no alias singleton was forced.
The [module-receiver](../tests/fixtures/m2-inferred-new-module-receiver/README.md)
and [method-cache](../tests/fixtures/m2-inferred-new-method-cache/README.md)
Java authorities preserve opposite receiver orders and the difference between
explicit and inferred construction. The latter also retains a Java internal
error for local object-method aliases as unresolved oracle evidence. These
selected authorities do not establish complete module or alias compatibility.

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

Observation filtering resolves constructor names before classifying literals,
so authored shadows and qualified calls remain observable. The compiler
distinguishes parser literals from collection constructors that Java folds into
constant nodes. Synthetic string fragments do not create another observation
at the enclosing string's span. A string containing only one interpolation
follows the pinned parser's literal classification while its inner expression
remains observable. These rules affect complete constraint diagrams, including
their value placement and whitespace.

Visibility follows the closest Java parser boundary. A `for` iterable can be
visible even inside a hidden outer body, while its body observations are hidden;
a nearer nested boundary can change that decision. Member predicates suppress
their descendants, but the synthesized whole amendment is governed by enclosing
boundaries. Object methods inherit visibility; root and nested function literals
follow the lambda rule. The Stage131 oracle covers these distinctions with
assertions on and off. This filters rendered observations and does not change
evaluation order.

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

Repeated property bodies form an ordered amendment chain. Their syntax retains
the first body span and distinguishes an authored amendment declaration from an
explicit assignment. Class-valued parents are validated before runtime member
construction, then use the class prototype. Parser and compiler failures retain
their earlier precedence. Function amendments defer construction until invocation;
nullable and inferred parents retain their default and ownership rules.

An empty body or a generator body that produces no members returns its object
parent. For class-valued Dynamic, Listing and Mapping parents, it returns a
canonical empty object with a neutral environment and a prototype backlink;
user-class parents return the prototype. The lazy empty-object cache holds weak
generational handles, so a live result preserves identity without retaining a
discarded producer environment. Generic builders with attached defaults retain
their default metadata.

Listing.fold validates its Function2 callback, shallow-forces receiver member
values in child-to-parent order, then invokes the callback in inherited element
order using the receiver's cached override values. A later member failure
therefore precedes the first callback. The source, callback, accumulator and
pending force are traced across Host suspension, reentry and retry; nested
member objects remain lazy.

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

`List.contains` compares each stored element with the query in that operand
order, requests no hashes, and stops at the first match. Pending comparison
operands remain rooted through ordinary VM frames and the value stack.
`Listing.isDistinct` retains insertion-time hashes in an index and captures the
query hash before a comparison can suspend or reenter. Known-hash candidates
and entries without a known hash are merged in insertion order. This avoids
rescanning every stored value for distinct known hashes; unknown hashes and
collisions still use conservative searches. Complete Java collision-tree
ordering and execution-time or memory parity remain unverified.

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
uncovered function/class amendment cases, and the remaining types and metadata
remain unimplemented or unverified. No new speed or memory parity claim is made.


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

Synthetic super reads in authored amendment declarations have unavailable source
sections. Parent-validation failures omit that unavailable innermost frame while
retaining real callers. A synthetic UTF-16 source section extending past the
source end uses Truffle's line-1/column-1 fallback; valid EOF sections retain their
actual position. Nested expression scopes use the synthetic root's dotted name,
while real module declarations retain the separate `#` boundary.

Diagnostic type names use simple names for canonical `pkl.base` class identities,
including objects reached through a class prototype. User classes and `pkl.ref`
classes retain their qualified names; displaying a type does not force its members.

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

### Properties rendering and scalar projections

PropertiesRenderer uses the shared renderer continuation for options, converters,
annotation transforms, lazy members and Host suspension. Document rendering
flattens property paths and emits escaped key/value lines; value rendering keeps
its separate top-level and scalar rules. Deferred keys and render directives
retain their receiver and path roots across evaluation, cancellation and retry.
The emitter operates on lossless UTF-16 units, preserving Java escaping and
`restrictCharset` behavior, including unpaired surrogates. Runtime wrapper class
identity and renderer method signatures also feed the shared diagnostics.

String `chars` and `codePoints` traverse Java-compatible code points, combining
valid surrogate pairs while preserving unpaired units. List `count` uses the
existing resumable callback visitor, so predicate order, empty-list behavior
and roots during Host suspension remain shared across targets. These operations
support the pinned PropertiesRenderer originals; their selected comparisons do
not establish complete renderer, product CLI or generated-file compatibility.

### YAML rendering

YamlRenderer shares the existing frame-owned renderer continuation with JSON,
PCF and Properties. A UTF-16 emitter handles plain, quoted and multiline strings,
Unicode escapes, scalar numbers, indentation and stream document separators.
The `compat`, `1.1` and `1.2` modes affect ambiguous string quoting. Document and
value rendering retain their distinct root and stream behavior; options include
`indentWidth`, `omitNullProperties` and `isStream`.

Converter lookup, `pkl:yaml` property annotations and render directives run
through ordinary VM calls. Pending options, values, keys, converter paths and
stream state stay rooted across lazy forcing, key hashing, callbacks and Host
suspension. Error order, cancellation and retry follow that shared continuation;
platform adapters do not contain separate YAML evaluators or a Java fallback.

The Java-backed native, N-API and Chromium comparisons cover renderer behavior only. YAML parsing
remains separate work, including ten query controls and eight original modules.
One Java internal error for an oversized `indentWidth` remains an unresolved comparison. This shared
renderer does not establish full CLI file writing or performance compatibility.

### XML property-list rendering

PListRenderer uses the same frame-owned renderer continuation as PCF, JSON,
Properties and YAML. It evaluates indentation and converter mappings in the
pinned Java order, then applies converters and annotation transforms while
preserving member paths, lazy forcing and first-error selection. Deferred keys,
values and paths remain rooted through Host suspension, GC, cancellation and
retry; platform adapters do not provide separate Plist evaluators.

A small UTF-16 emitter supplies XML property-list tags, document framing,
five-character escaping, Java-compatible number spelling and base64 Bytes.
Document and value rendering retain distinct root rules. Null properties and
entries are omitted, while collection ordering and empty-container formatting
follow the Java observations. Render directives preserve their separate key
and value treatment. UTF-16 units are not silently repaired to make XML valid.

Canonical workspace and nine native original CLI-argv checks pass within their
recorded scope. M1/platform closeout: M1 matches 6,078 strict comparisons and passes its separate architecture contract on each target; the selected comparison matches 450 observations on each of Native, N-API and Chromium.
One recursive-converter Java internal error remains an unresolved comparison;
16 syntax-preempted controls remain distinct from the 80 reached renderer
controls. Nonempty file writing, complete product APIs and performance parity
are not established by these renderer checks. Evidence parts are linked through
the [Stage95 checkpoint](../reports/m2/stage95-plist-renderer-checkpoint.json).

### Strict transforms and quantity identity

Collection transforms retain their source, callback, iteration cursor and
pending operands in ordinary VM frames. Calls, key lookup, hashing, insertion
and Host suspension use the shared continuations. Completed strict results use
neutral builder environments so they do not retain an otherwise unreachable
source module or callback capture. Evaluator-local weak handles canonicalize
eligible empty List, Set and Map values; live values preserve reference identity,
while unreachable empty values remain reclaimable. Cast wrappers and custom
parents are not replaced by a structurally equal empty value.

`this` and implicit property/method access use an explicit frame receiver first,
including an explicit Null value. They consult the lexical scope only when that
receiver is absent. Builtin functions with neutral environments therefore do not
need an otherwise unused lexical scope to return their captured receiver.

Duration stores a double, one of seven units, and a semantic reference token.
Cloning preserves the token. Equality checks reference identity before normalized
nanoseconds, which preserves equality of the same NaN-valued instance; hashing
follows Java's normalized double bits, including its signed-zero behavior.
Literal unit members, including nullable member syntax, retain executable
constant identity. The seven unit-symbol String values are shared independently
of authored strings with equal contents. These rules are visible through
collection keys and renderer conversion diagnostics, not just scalar text.

This Duration foundation supplies value/unit/positivity properties, unary
negation, type checks and shared formatting. Remaining arithmetic, conversion,
ISO output, DataSize and typed product transport require further implementation.
The [collection transform contract](m2-collection-transforms.md) separates
these language behaviors from capture-Host policy and evidence boundaries.

### Object conversions and optional lookup

Object-to-strict-collection conversions first shallow-force visible members from
child to parent, including Dynamic member kinds that the result later omits.
They then enumerate declarations from parent to child, retaining the first
declaration's kind and position while reading the final receiver's cached value.
List/Set selector maps call the key extractor, then the value extractor, before
hashing and insertion. Duplicate keys retain their first position and last value.
Pending source, builder, key and callback values remain traced across Host
suspension, reentry, cancellation and retry; completed strict results release
unneeded source environments.

Lazy conversion objects retain a traced source delegate and the appropriate
receiver. Absent source members can use the target prototype's default; explicit
Pkl null remains a present value. Listing/Mapping `getOrDefault` force and check
the inherited `Function1` default only when a key is absent. Its type error keeps
the standard declaration frame and authored default frame. Dynamic property
queries preserve Java's distinction between member declarations and cached
string entries, including the cache-dependent `hasProperty` result.

Synthetic property identifiers are internal lookup keys. Member descriptors keep
the original `PklString` name for conversion, diagnostics and rendering, so lone
UTF-16 surrogates, U+FFFD and NUL are not conflated. Authored quoted identifiers
can contain NUL. Synthetic names use an LF prefix, which authored identifiers
cannot contain; LF-bearing String keys use the same injective UTF-16 encoding
so they cannot collide with synthetic surrogate names. The six independent Stage62
Java fixtures exercise these paths at the public string boundary; full typed
export, wrapper identity and whole-language compatibility remain separate gates.

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


## Method deprecation call sites

The shared VM retains warning state under executable owners and invocation sites.
Direct lexical sites bind after argument evaluation; virtual and super sites
bind before arguments. A warning acknowledgement publishes the target binding,
while callback rejection leaves it available for retry. Reentrant publication
retains the expected cache chain and traces its class/declaration roots until the
pending call completes. A direct target roots its declaring owner; it does not
retain an arbitrary receiver subclass or instance.

Virtual sites retain actual receiver classes up to the pinned polymorphic cache
limit, then release old roots before indirect lookup. Intrinsic Function.apply
uses its separate path. Retiring an executable site releases its retained graph;
collector tests also cover cancellation, query release and final closure. See
[the deprecation contract](m2-deprecation.md) for actual Java comparisons and the
remaining cross-evaluator standard-library cache obligations.
This evaluator-local representation does not implement Java's static singleton
standard-library execution lifetime.

## Original frames and Host transformation

Runtime sources retain authored URIs, including `pkl:` standard-library URIs.
`FrameTransformer` selects the default display policy, builtin identity, or a
Host continuation. A `LogFrame` owns UTF-16 text for its URI, nullable member,
and source lines, with independent inclusive source coordinates. Replacement
frames cross native, Wasm, and N-API through the same typed transport.

The shared VM buffers a message, transforms its frame, then delivers the logger
callback. Transformation remains observable with a noop logger. Distinct typed
requests preserve token validation, reentry, cancellation, and pending roots.
A deprecation site publishes its successful target only after delivery completes.

Diagnostics compress raw frames before transformation and validate each returned
frame before requesting the next callback. Cached language errors stay raw;
the escaping invocation applies its current policy. Rendered UTF-16 diagnostics
are retained with the error. Optional callback-origin metadata distinguishes
logger, log-transformer, diagnostic-transformer, and diagnostic-renderer failures
without constructing Java exception class names. Guest catch paths that omit
frames do not invoke the transformer.

See the [frame contract](m2-frame-transformers.md) for the actual comparisons and
remaining exception/API, eager-constructor, package/service-provider, and
cross-evaluator obligations.

## Selected property and type-resolution owners

The Stage 73–75 shared VM records the first local/public property decision at
the executable owner and call position, so later reads use that same decision
without changing the receiver that owns the value. `Reference` class-property
metadata resolves declared types in the prototype's lexical environment while
retaining the access receiver and key across suspension and retry. `NonNull`
and qualified builtin aliases retain their source declaration and argument
order for error selection. These paths use the shared engine for native, N-API,
and Wasm; the Host remains responsible for platform I/O.

The selected platform comparisons and local lifecycle tests exercise those
owners, but the model does not yet cover every Pkl declaration, module
initialization, reflection, or error path. The linked full-corpus replay completed separate integrity verification and independent regression review. Its remaining compatibility failures are recorded in the implementation checkpoint.

See the [implementation checkpoint](../reports/m2/stage73-75-implementation-checkpoint.json) for raw evidence and execution provenance. These local records do not establish full M2, product API, release-platform, or performance compatibility.

## Selected Stage76–78 shared primitives and frontend

Regex parsing and `String.isRegex` use the shared runtime across native, Wasm,
and N-API. The frontend consumes a leading shebang and reports the selected
empty parenthesized-type error at its authored site. Numeric Int/Float `sign`,
`abs`, `ceil`, `floor`, `round()` and `truncate()` use the shared value model;
Java-first cases include overflow, signed zero, NaN, Infinity, arity, and
Reference metadata controls. Platform I/O remains behind Host.

These implementations were integrated into canonical sources and exercised by
the local required Rust checks, selected platform comparisons, and a full
ready-corpus replay. The replay retains one Rust internal error, unsupported
cases, ordinary mismatches, and Java oracle errors. Its separate integrity
verification and independent regression audit succeeded without a prior-pass
regression. This architecture covers selected paths only and does
not imply complete regex syntax, numeric API, frontend grammar, or M2 parity.

## Reflection metadata and membership boundaries

Reflection uses the shared evaluator's declaration identities and source-backed
standard-library modules. Trusted standard-library resolution is an internal
compilation context; a user-supplied URI does not grant `external` declarations
or change public module restrictions. Declaration locations come from the
owning compilation unit when explicit metadata is absent, including source
registered through expression sessions and the CLI.

`Typed.hasProperty` consults class metadata and does not force property values.
Initialized classes include inherited, hidden and required properties and exclude
local members and methods. Pinned Java fixtures also preserve class publication
failures and repeated queries; they do not justify an additional module
initialization flag. Reference access exposes lazy property/key values through
the existing traced object representation. GC tests retain escaped mirrors while
needed, then replace module-cache aliases before asserting collection. Uncached
loads preserve Java's distinction between bypassing lookup and publishing aliases.
Cancellation closes a Host session; stale completion tokens are rejected, and
continuation after cancellation uses a fresh evaluator.

The selected native, N-API and Chromium captures use an explicit poll64/GC-after-
batch profile with the original two-million requested-step ceiling per session.
Byte-preserving comparisons of these same captures match the selected 569-contract
matrix on all three targets. Earlier poll1 and comparator failures retain their
original outcomes. Correcting source-byte decoding in verification tooling does
not change the evaluator, fixture bytes or captured payloads. This selected profile
is distinct from the workspace suite and full original-corpus execution.

## M3 standard library, renderers, binary decoding, and multi-file output

Milestone M3 extends the shared pure-Wasm engine (`crates/pkl-runtime` and `vendor/java_regex`, `unsafe_code = "deny"`, zero production Java fallback) across five architectural areas (full local verification recorded in the [M3 checkpoint report](../reports/m3/README.md)):

### 1. Vendored 23/23 `pkl:*` standard library and cooperative runtime modules
- **`stdlib_sources.rs` & `crates/pkl-runtime/src/stdlib/`**: Bundles exact byte-for-byte copies of all 23 upstream Pkl `0.32.0` `pkl:*` modules (adding `Command.pkl`, `DocPackageInfo.pkl`, `DocsiteInfo.pkl`, and `settings.pkl`; `external/pkl/stdlib/doc-package-info.pkl` is excluded from `pkl:*` by upstream `pkl-core.gradle.kts:106` and explicitly deferred to M5 `pkldoc`).
- **`vm/stdlib_modules.rs` & `vm/modules.rs`**: Registers all 23 `pkl:*` modules plus the lowercase `"pkl:project"` singleton alias (`ModuleCache.java:105`), tracking `ModuleUnit::singleton_cache_key` (`"pkl:project"`) separately from canonical `ModuleIdentity` (`"pkl:Project"`, matching `ProjectModule.java:24`) so reflection URI, source location, and distinct singleton/cached module identities match Java; uses `lazy_class_bootstrap` for `pkl:DocPackageInfo`, `pkl:DocsiteInfo`, and `pkl:Benchmark` so module imports do not eagerly require `platform_metadata`, binds qualified `base.List`/`Set`/`Map`/`Bytes` as 1-arg `VarArgs`-checked functions matching `BaseNodes.java`, retains `pkl:base` `import "pkl:*"` clauses for reflection (`reflect.Module(import("pkl:base")).imports`), and emits `PublishMemberAnnotations` for annotated synthetically bound module members (`pkl:math#atan2`, `pkl:base#Bytes`).
- **`vm/platform.rs`, `vm/release.rs`, `vm/modules.rs`**: Keeps `platform.current` and `release.current` Host-injected (`PlatformMetadata`), updates `release.commitId` (`e51fb2f`) with cooperative `pkl:semver` loading, and runs `Benchmark.{Micro,Output,Parser}benchmark.run` as a GC-rooted, Host-suspension-safe cooperative state machine (1-2-5 warmup scaling, Knuth statistics, uncached `OutputBenchmark` reload with `ModuleCache::remove` restoring previously absent cache slots cleanly without leaking temporary roots).

### 2. All 9 renderers, `PcfRenderDirective`, `Reference`, `converters`, and Protobuf types
- **`vm/output.rs`, `vm.rs`, `vm/rendering.rs`, `vm/xml_rendering.rs`, `vm/pkl_binary_render.rs`**: Connects all 9 `output.renderer` formats (`json`, `jsonnet`, `pcf`, `plist`, `properties`, `textproto`, `xml`, `yaml`, `pkl-binary`) via `Instruction::LoadPklBinaryModule` and `MakeBuiltinInstance` with exact `base.pkl:105-128` spans, declares abstract `ValueRenderer`/`BytesRenderer` methods, fixes `ModuleOutput.files`'s default to `Mapping<String, FileOutput>?` with `SetCollectionTypeDefault(FILE_OUTPUT)` + `WrapNullableDefault { defined: true }`, wires implicit `Listing.join`, implements resumable GC-rooted `PcfRenderDirective` (`before`/`value`/`after`, cycle-checked, standalone `"new "` prefix), and unifies foreign `Map.Entry` projection.
- **`vm/reference.rs` & `vm/converters.rs`**: Supports all 69 `base.pkl` class properties on `Reference` matching Pkl `0.32.0` (`VmModifier.java:174` leaves `PClass.Property.isExternal()` false, while `Module.output`, `Dynamic.default`, and `Listing`/`Mapping` members remain rejected), routes inherited `pkl:ref` module members cleanly through standard lookup, and matches `SpecPart::AnyEntry` (`[*]`) against `PathPart::WildcardProperty` in converter path resolution.
- **`vm/protobuf_types.rs`, `java_exception/provenance.rs`, `evaluation.rs`, & `vm.rs`**: Adds eager `MappingTypeNode` key checking, generalizes `protobuf_caller_frames(origin, profile)` across `CliBundledSettings` and `ApiIdentity` (scanning all `origin.activations` for `#Module.output.bytes`), projects `error.foreign_exception` through `project_pkl_bug_with_profile` even when `error.rendered_diagnostic` was pre-rendered by Host frame transformation (`evaluation.rs:408`), and populates `ForeignOrigin.primary_source` from `self.public_primary_source()` before falling back to `self.context.primary_source` (`vm.rs:2298`).

### 3. Wasm-pure `output.files`, fast-path `output.bytes`, and multi-file CLI execution
- **`evaluation/output_files.rs`, `evaluation/session.rs`, `evaluation/frontend.rs`, `probe.rs`**: Separates pure multi-file evaluation (`OutputFileEntry`, `ModuleOutputFilesResult`, POSIX/Windows `check_path_spec`, lossless UTF-16 + base64 VM framing, and lazy `output_file_keys` / `output_file_bytes` streaming) from platform filesystem I/O, fast-paths `"output.bytes"` alongside `"output.text"` in `SourceSession`, and exposes `SourceProbe::{start_output_files,reenter_output_files}`.
- **`evaluation/frontend/arguments.rs` & `examples/module_eval_probe.rs`**: Parses `-m`/`--multiple-file-output-path`, `-f`/`--format` (overriding `-p pkl.outputFormat=...` regardless of flag order), `-o`/`--output-path` placeholders, `-x`/`--expression`, `--module-output-separator`, and multi-module argv; implements lazy streaming multi-file output with component-wise symlink-aware `resolve_real_path` checking escapes, intra-/cross-module collisions, and directory targets before forcing `FileOutput.bytes`.

### 4. Iterative 19-tag `PklBinaryDecoder`
- **`crates/pkl-runtime/src/pkl_binary_decode.rs`**: Pure-Rust, Wasm-compatible, iterative MessagePack decoder supporting all 19 `PklBinaryCode` tags (`0x01`–`0x0F`, `0x10`–`0x12`, `0x20` `Reference`) with `DecodeOptions::strict()` (matching `org.pkl.core.PklBinaryDecoder.decode(byte[])`, non-exportable type errors, `REFERENCE` rejection, exact `offset`/`currPath` unwind, Java `Character.isWhitespace` blank checks, and JDK 21 `UTF_8$Decoder` replacement stepping via `decode_java_utf8_replace`) and `DecodeOptions::lossless()`.

### 5. Regex `(?c)` / `CANON_EQ` and cooperative YAML parser yielding
- **`vendor/java_regex/src/utf16/canon_eq.rs` & `canon-*.bin` / `grapheme-runs.bin`**: Embeds compact (~58.8 KB) JDK 21 Unicode 15.0.0 NFD/NFC/CCC and `Grapheme.getType` tables verified across all 1,114,112 code points, implementing both compile-time `CANON_EQ` (`Pattern.normalize` / `produceEquivalentAlternation` / `producePermutations` / `composeOneStep`) and inline `(?c)`/`(?-c)`/`(?c:...)` `NFCCharProperty` (`Node::NfcCharClass` + `Grapheme.nextBoundary`).
- **`crates/pkl-runtime/src/vm/yaml_parser*`**: Closes remaining scalar, duplicate non-scalar key, non-ASCII Unicode 15 digit, and JDK 21 UTF-8 malformed-input `Unsupported` paths, and adds resumable cooperative stepping (`Scanner::step_peek`/`step_skip`, `Parser::step_peek`, `Backend::step(budget)` with `BACKEND_STEP_BUDGET = 32`) so large YAML documents and `!!merge` expansions yield safely to VM budgets, cancellation, and GC.

## M4 modules, resources, projects, packages, and native host

Milestone M4 keeps all protocol, policy, URI, MVS, packaging, and caching logic pure and Wasm-compatible inside `crates/pkl-runtime` (`cargo check -p pkl-runtime --target wasm32-unknown-unknown --locked`, `unsafe_code = "deny"`, zero production Java fallback) while isolating all OS filesystem, TLS/HTTP socket, CA store, module-path archive, and subprocess management inside a dedicated native Host crate `crates/pkl-host-native` (`unsafe_code = "deny"`, zero `/usr/bin/curl`, zero `thread_local!` caches; full verification recorded in the [M4 checkpoint report](../reports/m4/README.md)):

### 1. Native Host boundary (`crates/pkl-host-native`)
- **Strict crate separation**: `crates/pkl-runtime` never opens sockets, touches OS filesystems directly, or spawns processes in Wasm builds; `crates/pkl-host-native` depends on `pkl-runtime` and supplies concrete Host implementations behind `pkl-runtime`'s Host, `HttpTransport`, `PackageTransport`, `PackageCacheStore`, and `PackagerHost` traits.
- **`certs.rs` & `assets/PklCARoots.pem`**: Embeds an exact byte-for-byte copy of upstream `PklCARoots.pem` (`218,384 B`), layers `--ca-certificates` and `~/.pkl/cacerts/` PEM bundles into `rustls` root stores, and projects PEM parsing / TLS handshake errors (`No certificate data found`, `Incomplete data`, `unable to find valid certification path to requested target`) with Java-compatible exception classes (`IoFailure::with_class`).
- **`http.rs`, `fs.rs`, `modulepath.rs`, `zip_archive.rs`**: Implements pure-Rust HTTP/1.1 + `rustls` HTTPS + HTTP forward proxy + HTTPS `CONNECT` tunneling with cooperative `CancelFlag` cancellation (`NativeHttpClient` / `NativeHttpTransport`), symlink-aware `NativeFs` (`open_file_nofollow`, `NotFound -> Ok(None)` for resource reads), non-symlink-following `NativeModulePathResolver` across multi-entry directories and JAR/ZIP archives (with exact 39-byte `EXECUTABLE_JAR_HEADER` detection and `.class` filtering), and pure-Rust `ZipReader`.
- **`driver.rs`, `package_host.rs`, `cli.rs`, `bin/pkl_m4_cli.rs`**: Wires session-owned `NativeHost` (integrating `SecurityPolicy`, `NativeFs`, `NativeHttpClient`, `NativeModulePathResolver`, `ExternalReaderRegistry`, `PackageResolver`, `ProjectDependenciesManager`, and `TimeoutWatchdog` with full `close()`/`Drop` cleanup), `NativePackageTransport`, atomic `0444` `DiskPackageCache` (`package-2/` layout with readonly-safe corrupt `.json` deletion), `NativePackagerHost`, `StackFrameTransformers.replacePackageUriWithSourceCodeUrl`, `package:`/`projectpackage:` fragment-path capabilities, and the unified native CLI `pkl_m4_cli` (`eval`, `test`, `project resolve`, `project package`, `download-package`, `analyze imports`, `settings`).

### 2. Wasm-pure Host security, HTTP policy, and `modulepath` (`crates/pkl-runtime/src/host/`)
- **`host/security.rs`**: `SecurityPolicy` enforces module/resource allowlist regexes (`DEFAULT_ALLOWED_MODULES` including `jar:file:` and `jar:nested:`, `DEFAULT_ALLOWED_RESOURCES`), checks allowlists before unregistered-scheme reader lookup, and confines `file:` imports/reads/globs to `rootDir` (including non-existent paths and symlink escapes).
- **`host/http.rs`**: `ParsedHttpUri`, `RewritingHttpClient` (`get_with_uri`), `RewriteRule` (longest-prefix across redirects), `HeaderRule` / `HeaderGlobMatcher` (glob-scoped per-hop custom headers and `User-Agent` override), `ProxyConfig`, `NoProxyRule` (CIDR+port and hostname fallback), lock-free `Arc<T>` `LazyHttpTransport`, 20-hop redirect enforcement, HTTP->HTTPS upgrade, and HTTPS->HTTP downgrade denial.
- **`host/modulepath.rs`, `host/errors.rs`, & `host/resources/lossless_uri.rs`**: Pure `modulepath:/...` URI validation, `IoFailure::with_class`, `package:`/`projectpackage:` fragment-path relative module resolution (`IoUtils.java:344-348`), and relative resource resolution from hierarchical external-reader modules.

### 3. External module and resource readers (`crates/pkl-runtime/src/external_reader/`)
- **`messages.rs`, `codec.rs`, `transport.rs`, `process.rs`**: Implements all 13 Pkl `0x26–0x32` MessagePack external-reader messages with raw concatenated `[code, map]` framing, key-omission nullability, signed 64-bit `requestId` values shared across a single process-wide `Arc<AtomicI64>` per deduplicated `ExternalReaderProcessHandle`, `ReadResourceResponse` Missing semantics (omitting both `contents` and `error`), `shlex` argument parsing, `ExternalReaderRegistry` process deduplication, graceful `0x32` `CloseExternalProcess` shutdown, and prompt `cancel()` force-kill+reap under `TimeoutWatchdog` (OS process spawning cleanly gated behind `#[cfg(not(target_arch = "wasm32"))]`).

### 4. Projects, MVS resolution, deterministic packaging, `package-2` cache, and `pkl:test` (`crates/pkl-runtime/src/packages/`)
- **`mod.rs`, `project.rs`, `mvs.rs`, `cycles.rs`**: `CanonicalPackageUri`, `Project` / `PklProject.deps.json` v1 model, `SettingsPrecedence` / `PklSettings` / `EvaluatorSettings` (with unified `EvaluatorExternalReader = ExternalReaderConfig`, CLI replacement, `--omit-project-settings`, empty `env {}` non-fallback, and `Project.resolvedEvaluatorSettings`), Minimal Version Selection (`ProjectDependenciesResolver`), and `find_import_cycles`.
- **`cache.rs`, `resolver.rs`, `http.rs`, `zip.rs`, `metadata.rs`, `packager.rs`, `test_runner.rs`**: `package-2` `encode_cache_path`, full-URI same-session checksum enforcement in `PackageCache`, non-writing `MetadataChecksumProvider`, strict `ZipArchive` EOCD/central-directory verification, deterministic `ProjectPackager` (`build_deterministic_package_zip` with DOS `1980-02-01` mtime, exact empty-ZIP SHA-256, import-escape `VmException` diagnostics, `.sha256` sidecars, and Wasm-pure two-stage JDK 21 `HashMap` bucket ordering `java_hashmap_order` matching `ProjectPackager.java:229`), and `test_runner.rs` (`render_spec_report`, `evaluate_facts_decoded_with_source`).
- **VM Pantry semantics (`vm.rs`, `vm/collections.rs`, `vm/equality.rs`, `vm/modules.rs`, `vm/analyze.rs`)**: `Member::hidden()`/`definition()` and `left_class.is_none()` `is_module` equality alignment, `active_cached_import_receiver` cached import cycle-breaking, non-constant `VmDynamic` `default` property type-checking parity, and `>\t` syntax error snippets in `analyze imports`.

## M5 product APIs, CLI, REPL, server, formatter, and pkldoc

Milestone M5 builds the full native Pkl 0.32.0 product surface on top of the shared pure-Wasm core (`crates/pkl-runtime` and `crates/pkl-formatter` both compile cleanly for `wasm32-unknown-unknown` with `unsafe_code = "deny"`, zero production Java fallback; full local macOS arm64 verification recorded in the [M5 checkpoint report](../reports/m5/README.md)):

### 1. Pure-Wasm shared engine additions (`crates/pkl-runtime`, `crates/pkl-formatter`)
- **Schema introspection (`crates/pkl-runtime/src/schema.rs`, `vm/schema.rs`)**: Cooperative `SchemaExtractor` exposing module metadata, doc comments, annotations, classes, type aliases, properties, methods, supertypes, and `PType` descriptors shared by `pkldoc`, `pkl-repl`, and `crates/pkl`.
- **REPL state machine (`crates/pkl-runtime/src/repl/`, `vm/repl.rs`)**: Pure-Wasm stateful `ReplSession` (`ReplRequest::{Eval, Load, Completion, Reset}` with `force_results: bool`, `ReplResponse`, `:examples`/`:force`/`:load`/`:reset`/`:help`/`:quit`, lexical scope persistence, incomplete-input detection, and identifier/member completion).
- **Server protocol core (`crates/pkl-runtime/src/server/`)**: Pure-Wasm `0x20–0x25` (`CreateEvaluatorRequest/Response`, `CloseEvaluator`, `EvaluateRequest/Response`, `LogMessage`) plus `0x26–0x32` client/external reader callback messages (`ReadResource`, `ReadModule`, `ListResources`, `ListModules`, `CloseExternalProcess`), MessagePack framing, evaluator registry, request concurrency, cancellation, and structured log/error transport.
- **Command evaluation, ANSI diagnostics, and binary encoding (`crates/pkl-runtime/src/{evaluation/command.rs,diagnostic/ansi.rs,diagnostic/frames.rs,power_assertions.rs,vm/pkl_binary_render.rs}`)**: Adds `pkl:Command` evaluation for `pkl run`, ANSI color diagnostic rendering (`render_ansi`, `--color`), power-assertion highlighting aligned with Pkl 0.32.0 CLI output, and shared `EncodePklBinary` / `PklBinary` value encoding across `pkl-server` and `crates/pkl`.
- **Pure-Wasm CST formatter (`crates/pkl-formatter`)**: Pure-Rust, Wasm-compatible (`cargo check -p pkl-formatter --target wasm32-unknown-unknown --locked`, `unsafe_code = "deny"`) CST Pkl 0.32.0 source formatter implementing grammar-version-aware formatting (`--grammar-version 1|2|auto`), trivia/comment/blank-line preservation, stdin/file/`--write`/`--diff-name-only`/`--silent` modes, and strict two-pass idempotency.

### 2. Native Host extensions (`crates/pkl-host-native`)
- **`NativeHostExtensions` (`crates/pkl-host-native/src/{driver.rs,lib.rs}`)**: Extends `NativeHost` without `thread_local!` state or Java fallback to inject custom Rust `ModuleReader`/`ResourceReader` SPIs, custom `Logger` callbacks, custom `StackFrameTransformer` hooks, and server-mode client reader bridges (`0x26–0x32`).

### 3. Six native product crates (`unsafe_code = "deny"`)
- **`crates/pkl`**: Ergonomic public Rust Evaluator API (`EvaluatorBuilder`, `EvaluatorBuilder::preconfigured()`, `Evaluator`, `ModuleSource`, `PklValue`, typed decoding, `ModuleReader`/`ResourceReader` SPI traits, `Logger`, `StackFrameTransformer`, `evaluate_output_{text,bytes,files,value}`, `evaluate_expression`, `evaluate_schema`, and deterministic `close()` lifecycle cleanup).
- **`crates/pkl-cli` (binary `pkl`)**: Unified product CLI covering `eval`, `repl`, `server`, `test` (`spec`/`minimal` reporters, `--junit-reports`, `--overwrite`), `project resolve`, `project package`, `download-package`, `analyze imports`, `format`, `run` (`pkl:Command`), `shell-completion` (`bash`/`zsh`/`fish`), `--help`, honest native-flavor `--version`, `--color`, `--format`, `--multiple-file-output-path`, `--expression`, and `stdlib/doc-package-info.pkl` evaluation.
- **`crates/pkl-formatter`**: Pure-Wasm CST formatter library shared with `pkl format` and future Wasm/N-API integrations.
- **`crates/pkl-repl`**: Native interactive PTY line-editor and non-interactive stdin pipe driver bridging `pkl_runtime::repl::ReplSession` for `pkl repl`.
- **`crates/pkl-server`**: Native stdin/stdout MessagePack RPC server (`pkl server`) driving `pkl_runtime::server` across concurrent evaluator IDs, client/external reader callbacks, timeouts, log notifications, and clean EOF shutdown.
- **`crates/pkl-doc` (binary `pkldoc`)**: Full documentation site generator and v1->v2 `--migrate` migrator bundling all **17/17** byte-identical upstream static web assets (`crates/pkl-doc/assets/`), supporting `stdlib/doc-package-info.pkl` (closing the 8-item M3 deferral), incremental docsite updates and older-version preservation, symlink vs `--no-symlinks` directory copies, JDK 21 `ZipFileSystem.buildNodeTree()` LIFO module ordering, and URI-encoded progress output (`path_to_file_uri`).

### 4. Explicit verification boundaries and 2026-11-20 exclusions
- **Honest native-flavor `--version` (`comparison_mode: "native-version"`, `3` cases)**: `pkl --version`, `pkl -v`, and `pkldoc --version` honestly emit `Pkl 0.32.0 (<os>, native)` / `pkldoc 0.32.0 (<os>, native)` matching Apple Pkl's official native-image flavor string without spoofing `Java <ver>`.
- **Honest `pkldoc` progress nondeterminism (`comparison_mode: "pkldoc-progress"`, `9` cases)**: Upstream Java `pkldoc` emits `\r`-delimited stdout progress lines concurrently using `Executors.newVirtualThreadPerTaskExecutor()` and filesystem-order `Files.walk()`, making raw stdout byte order nondeterministic even between identical Java 0.32.0 runs. Differential verification records `raw_stdout_bytes_match` honestly while strictly enforcing 100% raw byte-for-byte parity on all `1,767` generated files (`15,227,047 B`) and `13` symlinks, exact raw `stderr`, exact `exit_code`, ordered non-progress milestone lines, and exact URI-encoded progress-segment multisets.
- **2026-11-20 JVM/codegen exclusions**: `pkl-codegen-java`, `pkl-codegen-kotlin` (`2` CLI commands, `19` CLI options), JVM public API adapters (`java_api`: `8,239`, `kotlin_api`: `597`, `jvm_binding_spec_headings`: `73`, `excluded_corpus_jvm_test_files`: `58`), and JVM ServiceLoader SPI descriptors (`2` resources) remain explicitly excluded from required product scope and are preserved solely as reference inventory census in `compatibility/m5-product-tools.json`.

## M6 Wasm and N-API product bindings and distribution

Milestone M6 connects the unchanged shared pure-Rust Apple Pkl 0.32.0 engine (`pkl-runtime`, `pkl-formatter`, and `pkl-host-native` + `pkl` on Node; `unsafe_code = "deny"`, zero production Java fallback) to production browser/Node WebAssembly and Node.js N-API packages while preserving 100% of existing M1 experimental exports (full local macOS `arm64` and Chromium verification recorded in the [M6 checkpoint report](../reports/m6/README.md)):

### 1. Unified JS/TS API Contract and Capability Matrix
- **Shared Public Surface (`19` common exports, `0.32.0`)**: Both `@pkl-rs/wasm` and `@pkl-rs/napi` export identical ergonomic async APIs and TypeScript declarations (`index.d.ts`): `PKL_VERSION`, `capabilities`, `createEvaluator`, `ModuleSource`, `PklError`, `decodePklBinary`, `PklDuration`, `PklDataSize`, `PklPair`, `PklRegex`, `PklObject`, `PklClass`, `PklTypeAlias`, `PklIntSeq`, `PklFunction`, `PklReference`, `format`, `createRepl`, and `analyzeImportGraph`, plus target-specific extensions (`init` and `VirtualFileSystem` on `@pkl-rs/wasm`; `Project` on `@pkl-rs/napi`).
- **Lossless Cross-Language Transport and 64-Bit Clock**: Exact signed 64-bit integers cross the boundary as JS `BigInt` (`i64::MIN..=i64::MAX`, rejecting out-of-range values); binary payloads (`output.bytes`, `output.files`, `pkl-binary`, `Bytes`) cross as owned `Uint8Array` copies; `PklString` preserves UTF-16 semantics without silent corruption; and monotonic clock/timeout timestamps use validated 64-bit millisecond counters (`u64` / safe integer ms), removing M1's 49.7-day `u32` ceiling.
- **Explicit Capability Reporting (`capabilities()`)**: A 19-key capability descriptor explicitly distinguishes supported vs. unsupported host capabilities across `wasm-browser`, `wasm-node`, and `napi-node`. Invoking an unsupported capability (such as OS filesystem, native HTTP/TLS sockets, `modulepath:` JAR archives, or OS external-reader subprocesses in Wasm) deterministically raises a structured `PklError` (`code: "capability-unavailable"`) rather than silently skipping or returning false passes.

### 2. WebAssembly Product Bindings (`crates/pkl-wasm`, `packages/pkl-wasm`, `@pkl-rs/wasm@0.32.0`)
- **Pure-Wasm Rust Binding Layer (`crates/pkl-wasm/src/product/{mod,util,decode,schema,host_core,evaluator,repl,analyzer,test_runner}.rs`)**: Preserves all M1 probe exports in `crates/pkl-wasm/src/lib.rs`, compiles cleanly for `wasm32-unknown-unknown` with `0` warnings and `unsafe_code = "deny"`, and wraps `SourceSession`, `SecurityPolicy`, `WasmPackageCache`/`ZipArchive`, `SchemaExtractor`, `WasmImportAnalyzer`/`ImportGraphAnalysis` (with security-checked resolution for all 23 `pkl:*` stdlib modules), `test_runner`, `ReplSession`, `PklBinaryDecoder`, and `pkl-formatter`.
- **Cooperative Async Host Suspension & Lifecycle**: Yields typed Host requests (`ReadModule`, `ListModules`, `ReadResource`, `ListResources`, `HttpGet`, `PackageMetadata`, `PackageArchive`, `Log`, `TransformFrame`) to JavaScript so `Promise`, `fetch`, `VirtualFileSystem`, and custom async readers resolve without blocking the main thread while `SecurityPolicy` (`allowedModules`, `allowedResources`, `rootDir`) remains enforced inside `pkl-runtime`. Supports `cancel()`, `timeoutMs`, and idempotent `close()`, draining cancelled Host operation IDs so JS aborts pending `AbortController` / timer handles without leaks.
- **Browser + Node ESM Distribution (`packages/pkl-wasm/`)**: Provides `VirtualFileSystem` (in-memory files/directories, path normalization, `rootDir` confinement, glob listing, `file:///` and custom scheme mounting), async HTTP/module/resource loaders, pure-Wasm `package://` archive loading, and a pinned `wasm-bindgen 0.2.108` build (`scripts/build.mjs`) producing a self-contained 13-file `npm pack` tarball working identically in real Chromium ESM (`init(urlOrBytes)`) and Node.js (auto-loading bundled `.wasm` bytes).

### 3. Node.js N-API Product Bindings (`crates/pkl-napi`, `packages/pkl-napi`, `@pkl-rs/napi@0.32.0`)
- **Non-Blocking Native Worker Execution (`crates/pkl-napi/src/{bridge,error,product,repl,schema,values}.rs`)**: Preserves all M1 probe exports in `crates/pkl-napi/src/lib.rs` and drives `pkl-runtime`, `pkl-host-native`, `pkl`, and `pkl-formatter` off the JS event loop via libuv `AsyncTask` worker threads alongside optional `*Sync` helpers for pure-native evaluations.
- **Deadlock-Free JS SPI Bridge & Full Native Host**: Routes custom sync/async JS `moduleReaders`, `resourceReaders`, `logger`, `frameTransformer`, and `securityManager` callbacks through weak `ThreadsafeFunction` bridges without stalling Node event-loop teardown; exposes full native Node host capabilities (`file:`, `env:`, `prop:`, `modulepath:` directories + JAR/ZIP, rustls HTTP/HTTPS with bundled `PklCARoots.pem`, custom CA certificates, proxy/`noProxy`, headers, rewrites, `Project.load` for `PklProject`/`PklProject.deps.json`, `package:`/`projectpackage:` with `package-2` disk cache, and `0x26–0x32` external reader subprocesses), plus `cancel()`, `timeoutMs`, and deterministic `close()`.
- **Dual ESM + CommonJS Distribution (`packages/pkl-napi/`)**: Ships `dist/index.mjs`, `dist/index.cjs`, `dist/index.d.ts`, and platform-named native binary `native/pkl_napi.darwin-arm64.node` in a clean 6-file `npm pack` tarball verified via clean-directory ESM and CJS installs. Cross-OS N-API binaries (`darwin-x64`, `linux-x64-gnu`, `linux-arm64-gnu`, `win32-x64-msvc`), Firefox/WebKit Wasm execution, hosted CI, and M7 `<= 1.00` JVM throughput/RSS parity remain tracked for **M7**.

## M7 runtime optimization and memory reclamation architecture

Milestone M7 optimizes the shared pure-Rust evaluation engine and its Native, N-API, and Wasm bindings while maintaining `unsafe_code = "deny"`, pure `wasm32-unknown-unknown` compilation, zero production Java fallback, and 100% raw-byte parity with Pkl `0.32.0` (full local macOS `arm64` benchmark, soak, fuzz, and cross-milestone regression evidence recorded in the [M7 checkpoint report](../reports/m7/README.md)):

### 1. Pre-compiled `pkl:base` snapshot and VM dispatch specialization
- **`crates/pkl-runtime/src/vm/base_snapshot.rs::BaseSnapshot`**: Pre-compiles and freezes the `pkl:base` bootstrap module bytecode, metadata, and `ExecutableRoots` base prefix once so fresh evaluators clone an immutable `Arc`-backed snapshot instead of re-lexing/re-parsing/re-compiling `pkl:base` per evaluator (`scalar:fresh-evaluator` `1065.612x -> 0.833x` vs live Temurin 21). `ExecutableRoots.base_overrides` overlays dynamic modifications without unfreezing the shared base slice.
- **Dispatch & allocation tightening (`crates/pkl-runtime/src/{vm.rs,heap.rs,vm/typecheck.rs}`)**: Replaces linear stdlib checks with literal `stdlib_kind` matching and early `canonical_builtin_class` cache hits, boxes `typecheck::Task::RestoreEnvironment` (`600 B -> 144 B`) to shrink VM continuation frames, and adds specialized `Heap::allocate_function`, `Heap::allocate_strict_list`, and `Heap::get_fast` helpers.
- **Integer & collection hot paths (`crates/pkl-runtime/src/vm/fast_int.rs::FastIntOp`)**: Compiles self-recursive and binary integer kernels (`has_self_calls || arity == 2`) into `FastIntBlock` / `FastIntTree` executors (`recursive-compute:persistent` `29.101x -> 0.074x`), fast-paths non-capturing lambdas (`MakeFunction`), optimizes `StrictList` `map`/`fold` and `try_fast_member_chain_bulk`, and stores strict `List` elements in compact `list_elements` while preserving full `forcing_members`, `force_collection_member`, `Task::Read`, and `source_member` semantics for `for`/`...` spreads (verified by fixture `tests/fixtures/m7/fuzz-regressions/reg_05_list_constructor_order.pkl` and Rust regression test `fuzz_reg_05_list_constructor_preserves_element_order` in `crates/pkl-runtime/tests/m7_fuzz_and_gc_regressions.rs`).

### 2. Bounded-memory tracing GC, compact tables, and host lifecycle cleanup
- **Automatic inter-op tracing GC & URI alias bounding (`crates/pkl-runtime/src/vm/query_lifecycle.rs`, `vm.rs`)**: Triggers automatic tracing collection between evaluator operations and bounds uncached module URI aliases so persistent native, N-API, and Wasm sessions reclaim cyclic object graphs and ephemeral modules continuously (verified by direct `SourceSession` `heap_probe`: steady `live: 122 -> 122 -> 122`, `executable_live: 16 -> 16 -> 16`, `reclaimed: 438 -> 2,278 -> 4,118`).
- **GC-shrinking Fibonacci-hashed `ExecutableRoots` & `FastIntCache` (`crates/pkl-runtime/src/vm.rs`, `vm/base_snapshot.rs`, `vm/query_lifecycle.rs`)**: Replaces dense monotonic block-ID-indexed `Vec` tables (whose leading retired `None` prefix grew unboundedly whenever a low block remained pinned during ephemeral churn) with compact power-of-2 Fibonacci-hashed open-addressed tables (`O(live)` memory with shrinking rehash on GC, retaining the frozen dense `Arc` prefix for `pkl:base`).
- **Staged-source consumption & deterministic Wasm disposal (`crates/pkl-host-native/src/driver.rs`, `crates/pkl-wasm/src/product/host_core.rs`, `packages/pkl-wasm/src/index.mjs`)**: Consumes `staged_sources` and `staged_custom_sources` via `.remove()` on `LoadSource` and clears host state on `WasmHostCore::close()`. In `@pkl-rs/wasm`, `_inFlightCount`-guarded `_disposeWasmIfClosed()` invokes `wasm.free()` immediately on idle `close()` (or when an in-flight call unwinds) for `Evaluator` and `ReplSession`, alongside `analyzer.free()` in `analyzeImportGraph`, holding second-half Wasm linear-memory growth to `0` pages across all 6 `wasm-node` soak scenarios.

### 3. Fast binary encoding, shared columnar JS/Wasm transport, and V8 Liftoff outlining
- **Direct zero-task binary rendering (`crates/pkl-runtime/src/vm/pkl_binary_render.rs::try_fast_encode_pkl_binary`)**: Encodes cached modules, strict lists, and plain `Dynamic` objects directly to exact Pkl `0.32.0` MessagePack bytes in a single poll without allocating resumable renderer tasks, falling back to `RenderRun` for lazy/custom/cyclic values.
- **Flat pure-JS decoding & shared `ColumnarValueBuffer` (`crates/pkl-runtime/src/vm/columnar_value.rs::ColumnarValueBuffer`, `packages/pkl-wasm/src/values.mjs::tryDecodePklBinaryFast`, `packages/pkl-napi/src/runtime.cjs::tryDecodePklBinaryFast`)**: Wires `ColumnarValueBuffer` into `Vm`/`SourceSession` with a 1-entry `gc_epoch`-guarded immutable snapshot cache (`<= 256 KiB`, cleared on `Vm::collect`/`Vm::close`) and `OP_COLUMNAR_LIST` O(1) `slot_perm` packing, falling back seamlessly to resumable `RenderRun` on any unforced member, cycle, unpaired surrogate, annotation, or edge type. Both `@pkl-rs/napi` and `@pkl-rs/wasm` unpack columnar buffers into fresh owned JS values (`PklObject`, `Array`, `Uint8Array`, `BigInt`, `String`) while `evaluateExpressionPklBinary` and `decodePklBinary` preserve exact Pkl `0.32.0` wire bytes.
- **N-API & Wasm boundary tuning (`crates/pkl-napi/src/product.rs`, `crates/pkl-wasm/src/product/evaluator.rs`, `packages/pkl-wasm/src/host.mjs`)**: Adds warm-cached bounded try-poll (`32_768` steps with non-cancelling `EvalOp::ContinueRunning` fallback to libuv `AsyncTask`) and V8-managed `BufferSlice::copy_from` in N-API (preventing deferred external `ArrayBuffer` finalizer accumulation), plus `pure_standard_host` and `DEFAULT_POLL_BUDGET = 65536` in Wasm.
- **`wasm32` `inline(never)` V8 Liftoff outlining, `[profile.wasm-release]`, and UTF-8 / columnar `viewCache` fast paths**: Marks cold bootstrap and giant runtime dispatch functions with `#[cfg_attr(target_arch = "wasm32", inline(never))]` (~48 MiB Liftoff zone reduction), builds `@pkl-rs/wasm` with `[profile.wasm-release]` (`opt-level = "s"`, `lto = "fat"`, `panic = "abort"`, `strip = "symbols"`, plus optional `wasm-opt -Os` in `packages/pkl-wasm/scripts/build.mjs`), caches immutable decoded `valBigStr` in `unpackColumnarBuffer`'s generation-guarded `viewCache`, and uses a pure-JS RFC 3629 strict short-string UTF-8 decoder (`<= 64 B`) in `fastReadStr` to avoid per-string WebIDL `TextDecoder.decode()` crossings. Under the `2026-11-22` browser Wasm contract (`INTENT.md`, `TESTING.md`, `benchmarks/m7_workloads.json`), Native/N-API/Wasm-Node enforce `<= 1.00x` time and `<= 1.00x` full-process RSS, while browser `@pkl-rs/wasm` enforces `<= 1.00x` time for Chromium/WebKit (Firefox `typed-conversion` explicit SpiderMonkey exception `<= 1.50x`), Wasm linear memory `<= 0.15x` Java peak RSS, JS heap `<= 64 MiB`, and `.wasm` gzip-9 `<= 2.50 MiB` while continuing to record raw multi-process browser-tree RSS verbatim.
