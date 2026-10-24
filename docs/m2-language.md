# M2 language verification

Stage 61 adds shared `IntSeq` values and the prerequisites of unchanged original
programs. Its [implementation checkpoint](../reports/m2/stage61-implementation-checkpoint.json)
records all 474 selected string observations matching on each of native,
N-API, and Chromium. Bytes now matches all 600 observations per target;
Duration and DataSize retain 548 and 510 matches. Portable Any improves to
170 matches and two retained scalar Regex mismatches per target. Earlier
Stage 60 failures below remain historical evidence rather than current passes.

The immutable sequence carrier preserves start, end, step, aligned last value,
and reference identity. Java's method/spread iterator stops at the aligned last
value, while its `for` generator uses a wrapping count derived from
`abs((end - start) / step) + 1`; extreme integer inputs distinguish these paths.
Empty sequences compare equal and hash to one; nonempty equality and hashing
use start, aligned last, and step. Rendering retains the original end. IntSeq
inherits Any rather than Collection, so collection length/indexing/addition
are not exposed. Map, fold, and conversions share resumable VM continuations,
with callback, accumulator, and synthetic Listing environments retained by GC.

The [Java authority](../reports/m2/stage61-java-checkpoint.json) separates 237
string expressions, three typed-export rejection observations, 17 math constants,
and 21 List.add expressions. The [List supplement](../reports/m2/stage61-list-accessor-fold-java-checkpoint.json)
adds 65 accessor/fold expressions. Math strings and internal Rust numeric bits,
List prerequisites, and Host/GC lifecycle tests have focused native regressions;
they are not separate typed product API or portable supplement comparisons.
The pinned `pkl:math` source is bundled with all 17 constants; its external
functions remain explicitly unsupported.

The expanded original cohort preserves the prior 17 programs and every one of
the 47 pinned input snippets containing `IntSeq`, with two overlaps. It has
26 matches, 15 mismatches, and 21 unsupported results across 62 programs.
The prior cohort now has 15 matches and two Binary-path mismatches;
`basic/bytes.pkl`, `api/bytes.pkl`, and `api/duration.pkl` newly match, with
no previously passing case regressing. The original basic/API IntSeq programs
also match. Each original program ran once per runtime; offline verification
does not add executions. Remaining renderer, collection, builtin, and diagnostic
failures keep the selected and full gates failed. Inventory readiness, passing
focused tests, and stage numbers are not whole-language completion percentages.

Stage 60 adds shared `Bytes` operations and resumable `List.toBytes` conversion.
The [Java authority](../reports/m2/stage60-java-checkpoint.json) contains 300
ordered expressions in two fresh JVMs, with 223 successes and 77 ordinary Pkl
errors per session. Native, N-API, and Chromium each match 594 of 600 common
string observations. `list-intseq-all`, `list-intseq-hash`, and
`value-all-intseq` fail in both repetitions; their strict gates remain failed.
The full fixture retains these rows. At Stage 60, one ignored Rust regression
preserved all three known failures separately from the 297 passing focused cases.

Selected operations cover length/size, Base64, hex, MD5, SHA-1, SHA-256,
`sha256Int`, concatenation, indexing, `getOrNull`, `toList`, and strict UTF-8,
UTF-16, and ISO-8859-1 decoding. Java directly widens a signed byte for Bytes
indexing and `getOrNull`; its List conversion produces unsigned values.
`sha256Int` reads the first eight digest bytes as a signed little-endian integer.
The shared engine preserves these distinctions and first-error diagnostics.
Heap-local cache links retain the originating List through Host suspension and
GC; release clears those links. Value/hash observations do not establish Java
wrapper identity or mutable exported-array aliasing.

Comparisons require seven common raw channels, including UTF-16 code units,
and the complete session protocol. Java exception-class, exception-string, and
cause channels remain additional producer observations; the candidate string
boundary does not synthesize them. Portable capture requests the eleven
environment keys recorded by Java. Effective `en-US`/`UTC` comes from both
actual JVM session command lines, separately from the preflight `ko-KR`
properties. Actual Node environment and browser locale/timezone observations
are recorded. The Java capture does not record its complete OS-injected
environment, so complete process-environment identity is not claimed.

The [implementation evidence](../reports/m2/stage60-implementation-checkpoint.json)
also preserves unchanged original programs: 12 matches, four mismatches, and
one unsupported result among 17. `errors/invalidBytes4.pkl` newly matches;
`basic/bytes.pkl` and `api/bytes.pkl` reach missing `IntSeq` calls. Binary tests
still fail at `module.toMap()` before encoding, and the Duration API program
still requires `pkl:math`. Typed exports, full M2, product APIs, and performance
remain separate requirements.

The [DataSize contract](m2-data-size.md) separates shared value operations and
raw native/Node/Chromium string comparisons from required typed exports,
remaining Bytes behavior, and unchanged original-module failures.

The [Any.ifNonNull contract](m2-any-if-non-null.md) records shared VM lookup,
callback and null-default behavior, independent Java string authority, native
and portable raw comparison gates, and the remaining constructor gaps.

Stage 59 connects `Duration` to the shared value, VM, type, diagnostic, and
rendering paths. Its seven units are `ns`, `us`, `ms`, `s`, `min`, `h`, and `d`.
The pinned Java implementation converts as `value * sourceNanos /
targetNanos`; even a conversion to the same unit can overflow an intermediate
double. Addition and subtraction select the coarser unit. Ordering uses
`Double.compare`, while equality and hashing use values converted to
nanoseconds, with Java's distinct NaN and signed-zero behavior. Set and Map
observations include the transition from four to five keys. `isoString` uses
`value * (sourceNanos / 1e9)` and the pinned Java formatting and finite-value
checks; huge finite inputs can therefore format unusually or fail after
conversion. These details are required outcomes, including first-error order
for `isBetween` and argument/type failures.

The [sealed Java Duration string fixture](../tests/fixtures/m2-duration-values/README.md)
contains 274 ordered expressions in each of two fresh JVM sessions: 240
successes and 34 ordinary Pkl errors per session. A separate
[15-row Java exported-value supplement](../tests/fixtures/m2-duration-values-bits/README.md)
records raw double bits and units, also twice. Native, N-API, and Chromium
string comparisons must check complete status, expression, stdout, stderr,
UTF-16BE channels, and ordered process protocol without normalizing errors.
The Java exception class is producer metadata. Rust internal raw-bit tests
use the exported-value observations as authority; they do not test a typed
Rust, Wasm, or N-API Duration export. Typed product APIs and the original
upstream `basic/duration.pkl` and `api/duration.pkl` suite executions remain
separate obligations. The [Java checkpoint](../reports/m2/stage59-java-checkpoint.json)
records that the canonical source snapshot includes unrelated `reference.pkl`
files in place of those original Duration programs; their bytes are captured
in the separate validation source snapshot. The authored Java calls never used
the original snippet programs as inputs. Java evidence integrity is distinct
from a candidate compatibility result.

The [Class-value contract](m2-class-values.md) records declaration/prototype
identity, superclass and converter rules, and independent Java observations.
Common query/logger comparisons and the required provider/export supplements
have separate verdicts; a matching selected Class matrix does not close M2.

The [method-deprecation contract](m2-deprecation.md) adds independent Java logger
observations, including implicit method calls and sequential evaluator lifetimes.
Its reference checkpoint and ongoing candidate implementation do not close the
whole-language requirements or the strict completion gate below.

M2 extends the shared Rust engine established in M1. Its completion gate covers
the language scope in [PLAN.md](../PLAN.md), including classes, generators,
constraints, generics, `Reference`, and complete diagnostics. The first executable
selection exercises only part of that scope.

The [trace development contract](m2-tracing.md) separately records actual-source
logger callbacks, source-session lifecycles, and lossless platform transport.
Its focused comparisons do not replace the whole-language completion gate.

The [object-body parameter contract](m2-object-parameters.md) records function
amendment, Mixin defaults, lexical capture, and their Java-first fixtures.
Arbitrary-expression and Host comparisons are separate from the bounded
property-path adapter described below.

The [resource-read development contract](m2-resource-reads.md) records `read` and
`read?` parsing, default scalar resource policy, nullable absence, declaration
source URIs, and the distinction between Java byte captures and lossless
portable string comparisons. Permitted I/O readers and complete URI semantics
remain open.

The [original module contract](m2-module-frontend.md) connects unchanged upstream
module commands to shared preparation and actual output evaluation. Its native
process comparisons retain the original arguments, resources, and physical
source path. The frontend-only API retains its unsupported output boundary;
portable comparisons select actual output separately with `output-bytes`.
Original setup-dependent cases remain incomplete when the observed Java outcome
does not satisfy the original expected exit status, even if raw channels agree.
The generated original-module registry includes all 888 original-ready snippets;
registration does not establish that they execute successfully. The explicit
`upstream-original-modules-v1` workspace profile retains complete pinned reference
trees while omitting unrelated project fixtures. Its filesystem boundary and
source inventory digests are recorded in schema-5 evidence, separately from
historical acquisitions using the default workspace.

The [original JVM-classpath profile](m2-jvm-classpath.md) separately compares the
unchanged `basic/import1b.pkl` input through the upstream JVM API/classloader
boundary. Its explicit reviewed execution mapping keeps the historical blocked
CLI corpus contract while requiring two successful raw JVM-profile repetitions
for that same original case ID in the selected/full M2 runner.

`compatibility/m2-language.json` keeps semantic families, pinned source
declarations, upstream language snippets, selected historical regressions, and
unmapped historical candidates visible. Declaration and heading counts are
inventory counts, not semantic coverage. An implemented parser node does not
establish its runtime behavior.

Some accounting rows intentionally overlap: selected rev1 execution contracts
also have historical candidate rows, parser declarations can share names, and
language-reference headings can cover the same behavior. Totals must not be
presented as distinct feature or test counts. Semantic mappings require review.

`compatibility/m2-mappings.json` is the reviewed input for linking existing
requirement IDs to Rust implementation symbols and existing case IDs. Its
baseline must match the pinned inventory. The manifest generator validates
source paths and symbols, records current source hashes, adds reverse case
links, and preserves previously selected supporting cases. A source symbol's
presence establishes provenance, not semantic completeness.

Each reviewed entry states the supported scope and remaining work. Partial
implementation or test mappings continue to fail the full gate. Completed
mapping labels still require all linked cases to execute and pass. Linking an
original upstream ID does not change its input, adapter, execution state, or
output boundary. Selected reproductions therefore cannot silently replace the
original snippet contract. Missing IDs, duplicate links, source drift, wrong
baselines, and contradictory scope claims fail validation. The reviewed entries
link bounded Reference, modifier, class, import, metadata, and diagnostic
implementations to selected cases. Every entry remains partial and records its
remaining scope.

```sh
python3 tools/m2_module_eval.py --check
python3 tools/m2_manifest.py --check
python3 tools/m2_identifier_tables.py --output target/m2/identifiers-new
python3 tools/m2_unicode_escape_tables.py --output target/m2/unicode-escapes-new
python3 tools/m2_uppercase_tables.py --output target/m2/uppercase-new
python3 -m unittest discover -s tools/tests -p 'test_m2_*.py' -v
python3 tools/m2_language.py --output target/m2/language-new --record-only
python3 tools/m2_language.py --verify-report target/m2/language-new
python3 tools/m2_language.py --verify-development target/m2/language-new
python3 tools/m2_language.py --verify-subset target/m2/language-new
python3 tools/m2_language.py --verify-full target/m2/language-new
```

Use a fresh output directory. By default, the runner acquires both expression
comparisons and registered original module commands. It records a release build
of `language_probe` and `module_eval_probe`, freezes their executables, compiles
the expression Java probe against the pinned oracle, and retains source snapshots
and raw process captures. `--adapter language-api-string` and
`--adapter language-module-eval` select a focused acquisition; omitted required
cases remain incomplete in the full gate. `--candidate` and `--module-candidate`
are explicit artifact overrides whose source bindings are unverified.

Expression acquisition defaults to one worker. `--workers 4` runs up to four
expression cases concurrently; the accepted range is 1–32. Original module
commands run sequentially in their shared physical workspace. Every case retains fresh Java and Rust processes,
separate raw captures and the pinned environment. Results remain in manifest
order, including failures and the independent remainder-error repeat. The report
records the worker count. This option measures acquisition throughput; evaluator
performance and memory comparisons require the separate benchmark contract.

The expression adapter's Java boundary is `evaluateExpressionString` with a file module and an
expression. Both processes receive the same absolute source path. Successful
UTF-8 stdout, full uncolored `PklException.getMessage()` stderr, and exit codes are
compared without normalization. A separate outcome file records whether Java
returned normally or caught a particular `PklException` class. This distinguishes
a handled upstream bug report from an uncaught JVM failure without changing
stdout or stderr. The Rust probe accepts property paths returning scalars,
selected References and type aliases, or strict Lists, Maps and Sets. Their
stringification reads completed values without forcing nested objects. Map uses
the single-line layout and List/Set the multiline layout of the Java value renderer.
Java may rehash nested Map keys during iteration; those rendering side effects
are not fully reproduced by the cache-only renderer.
This boundary does not prove typed API compatibility or general
compound-value rendering.

Language strings retain UTF-16 code units internally, including unmatched
surrogates. Literal types, keys, concatenation, interpolation and the implemented
collection `toString` paths preserve those units. The development probes apply
Java's UTF-8 writer replacement only when producing their output. Matching that
byte boundary does not establish lossless public JavaScript string transport.
Unmatched-surrogate module/resource URIs and Java's out-of-range escape internal
error remain separate gaps. Diagnostic member paths retain these units through
scope counters, runtime frames and recursive compression, before the Java UTF-8
writer conversion. Selected nested entry/function numbering and compression
controls verify this identity separately from displayed bytes. The complete
string API and diagnostic corpus still require further verification.

`String.toUpperCase()` uses a generated table from the pinned JDK's
`String.toUpperCase(Locale.ROOT)`, rather than Rust's Unicode case tables.
The generator captures all 1,114,112 code-point inputs as raw UTF-16, including
unmatched surrogate units, and retains full multi-unit expansions. Its default
mode checks exact generated bytes; `--write` updates the table after reviewing
the oracle evidence. The runtime decodes valid surrogate pairs and preserves
unmatched units. This uppercase component and selected VM calls do not establish
the remaining string builtins or regular-expression compatibility.

Numeric value-frame cases compare direct negation and grouping across typed
properties, method returns and collection members. Typed locals and function
bodies use the executable expression's source span; other constant members retain
Java's source suffix after their header. Authored syntax and assertion observations
remain separate. Minimum integers, radix literals, signed zero, scalar constants
and nonliteral unary operations provide value and diagnostic controls.

Rejected non-local object property type annotations retain their authored member
paths, including nested objects, functions, let scopes, entries and generators.
The selected cases compare the complete rejection bytes and accepted module,
class and object-local controls. Diagnostic naming preserves UTF-16 identity,
literal entry labels and scope-local ordinals. Const-modifier errors retain their
enclosing owner, while inferred-parent errors retain the expression owner.
Method bodies infer contextual `new {}` from return-type defaults, including
classes, collections, aliases, nullable types and defaulted unions. Lambda and
parenthesized contexts retain their rejection. Inherited property prototypes
use declared type defaults; instance amendments read prototype initializers.
Literal `unknown`, aliases of `unknown`, and `Any` keep distinct default rules.
Selected cases cover lazy unused methods, first errors, and generated default
frames. Nested-negation overflow frames and unavailable source sections remain
exploratory differences.

The Stage 34 selection retains two failing `Dynamic.toString()` calls and adds
separate `is Dynamic` controls. The original failures are neither replaced nor
added to the development exceptions, so the development gate also fails until
they are resolved. The generic portable comparison consumes a fixture module
exporting `cases` and `context`, validates source bytes and hashes against the
selected files, and executes each case twice on Node and Chromium. It records
complete value/error bytes and returns nonzero for any mismatch. Counts come
from the supplied cases, rather than a fixed stage denominator. It complements
the existing platform scenarios:

```sh
node tools/platform/compare-language-cases.mjs target/m2/annotations-platform-new target/m2/platforms-new/wasm target/m2/platforms-new/pkl_napi.node tools/platform/annotation-cases.mjs
node tools/platform/compare-language-cases.mjs target/m2/method-parents-platform-new target/m2/platforms-new/wasm target/m2/platforms-new/pkl_napi.node tools/platform/method-parent-cases.mjs
```

The historical Stage 34 wrapper remains available:

```sh
node tools/platform/compare-method-parents.mjs target/m2/method-parents-wrapper-new target/m2/platforms-new/wasm target/m2/platforms-new/pkl_napi.node
```

Each fixture supplies its source, source hash, property-path expression, and
exactly one complete Java-derived `expectedValue` or `expectedError`. The context
records URI, module name, power-assertion mode, and oracle acquisition identity.
The runner preserves fixtures, runtime and artifact identities, raw payloads,
and missing or failed executions. The fixture's Java acquisition and the portable
execution are separate evidence; creating a fixture or documenting the command
does not establish that a platform run passed.

Stage 35 adds declaration annotations on modules, classes, aliases, properties,
and methods. Selected comparisons distinguish bare construction, lazy fields,
eager generators and spread, const scope, type validation, first-error order,
and partial publication after initialization failure. After resolving its
signature, a method reads the message of the first annotation whose class is
exactly `Deprecated`. Unused
class and alias declarations remain lazy, and failed ordinary member reads
retain their retry behavior.

Builtin controls cover `Annotation`, `Deprecated`, `Since`, `Unlisted`,
`AlsoKnownAs`, `DocExample`, and `SourceCode`. Nullable string defaults and empty
`Listing<String>` defaults are distinct from required fields. In particular,
`SourceCode.language` has no implicit Go default: its complete union accepts
the listed literals and other strings, but a missing value remains undefined.
Collection element checks stay lazy and retain the standard-library declaration
frame when forced. Untyped parameterized annotation generators remain explicitly
unsupported: the pinned Java implementation fails internally when its diagnostic
location is null. This is an oracle error, not a matched language error. Original
exploratory parser failures and Java internal errors remain recorded separately
from language-error comparisons.

Full reflection and builtin annotation metadata remain open. The upstream
`@SourceCode` metadata on `Deprecated.replaceWith` is not synthesized.
`ModuleInfo`, converters, deprecation logging, module initialization/cache
semantics, general object `toString`, and `List.map` remain dependencies.
The original object-stringification and mapped-list failures retain their raw
inputs and outcomes alongside independent controls. Authored scalar fixtures do
not replace the original upstream annotation snippets or their renderer contract.

Stage 36 adds object force/string comparisons for `Dynamic`, `Listing`, `Mapping`,
typed objects and modules. Cases cover child-first forcing, inherited render order,
undefined members, hidden declaration owners, strict collections nested inside
objects, virtual `toString` dispatch, equal entry keys and guaranteed String-key hash
collisions. Portable cases preserve fresh Java results for native, Node and browser
comparison, including full errors. The older property-key case named as a collision
is an unequal-key control: its Identifier hashes do not prove a numeric collision.

Seven additional persistent definitions retain their original inputs. Six require
`trace`, which remains unsupported; failed setup is recorded rather than replaced
with trace-free input. The trace-free module-self sequence tests cached string
results in one evaluator. Actual-source Host comparisons separately cover ordinary
forcing, callback errors and nested callbacks. Matching expected error payloads
does not override extra callback/protocol differences or Java internal errors.
Diagnostic formatting that needs Host reads during key hashing remains a failed
comparison. These obligations stay outside claims about the passing scalar cases.

```sh
node tools/platform/compare-language-cases.mjs target/m2/object-string-platform-new target/m2/platforms-new/wasm target/m2/platforms-new/pkl_napi.node tools/platform/object-string-cases.mjs
```

The default uses Java's API setting with power assertions disabled. Selected
cases can opt in with `"power_assertions": true`; the runner then passes
`--power-assertions` to both probes and verifies that option in the recorded
commands. These cases compare the complete value graph, message, hints, and
frames as raw bytes. Enabling this profile does not change historical captures
or the default mode of other cases.

The commands have distinct meanings:

- `--verify-report` checks evidence integrity and recomputes recorded outcomes,
  including failures.
- `--verify-development` requires selected cases to match except for the explicitly
  tracked cycle diagnostic failure and integer remainder-by-zero upstream bug.
  Both remain failed in the report.
- `--verify-subset` additionally requires every selected comparison and
  metamorphic relation to pass.
- `--verify-full` additionally requires all language requirements to have
  implementation and test mappings, every required case to execute successfully,
  and the candidate to have a recorded build.

Unknown, unimplemented, unexecuted, unmapped, and mismatching requirements remain
failures of the full gate. Selected-case success and source inventory freshness
must never be reported as M2 completion.

CI acquires evidence with `--record-only`, which returns success only after a
complete, internally consistent acquisition, regardless of compatibility outcomes.
It then applies the named development contract. An optional full-corpus workflow
also runs the strict full gate.

The initial cyclic property probe exposes an oracle limitation: cold Java calls
can fail while initializing `VmStackOverflowException` on an exhausted stack.
A separate diagnostic experiment preinitializes that class without changing the
JAR; it produces controlled Pkl errors, but the rendered repetition count varies
across runs. Both conditions and all raw bytes are retained in
`reports/m2/cycle-evidence.tar.gz`. Preinitialization is not applied to the normal
oracle, and no repetition counts are normalized. This unresolved case prevents
strict selected-case completion.

For integer `1 % 0`, the pinned Java implementation reports a caught
`PklBugException` caused by `ArithmeticException` in `VmSafeMath.remainder`.
The development exception is limited to that exact input and failure shape;
it additionally requires a separate cold Java capture with identical complete
raw bytes. The Rust diagnostic must retain its recorded complete division-error
form. No stack traces are normalized or treated as matches. A changed failure,
timeout, or different output fails the development gate. Resolving the raw
diagnostic difference is still required by the strict selected and full gates.

Runtime diagnostics carry authored source spans and logical frames separately
from their message. Instruction spans are parallel metadata, preserving the
bytecode dispatch count. The text renderer is shared runtime code; native probe
code supplies source URIs and the expression boundary frame. The same
experimental scalar boundary is available as `evaluateLanguage` in the Node
and Wasm development bindings, including complete diagnostics. It accepts a
source string, URI, inferred module name, and scalar property path; explicit
module declarations take precedence over the inferred name.
An optional fifth Boolean argument enables power assertions, with the same
disabled default as the native probe and Java API.
Returned Reference values also pass through their domain renderer, matching the
Java expression-string boundary's automatic conversion and its separate error
stack. This adapter still accepts property paths only. It does not supply Host
I/O, general compound-value conversion, or the full expression API.

The generated identifier tables reproduce the pinned lexer's UTF-16 code-unit
predicates. `m2_identifier_tables.py` records the raw Java intervals and checks
the generated Rust bytes. CI repeats this check against the pinned JDK.

Unicode escape scanning and conversion use a separate generated table.
`m2_unicode_escape_tables.py` records all BMP UTF-16 units accepted by Java's
`Character.isLetterOrDigit` and their `Character.digit(unit, 16)` values, then
verifies the generated Rust bytes. Its evidence includes the pinned JDK,
probe sources, raw output, and source stability. Use a fresh output directory;
`--write` regenerates the table after reviewing a deliberate predicate change.
These predicates do not establish complete Unicode string compatibility.

M1's platform and Host contracts remain regression requirements. New language
features must retain captured values across VM suspension and collection and use
the same engine in native Rust, browser Wasm, and Node N-API.

## Persistent class initialization sequences

Independent expression processes cannot verify state retained after an error.
The same contract also covers type-alias publication and function metadata;
the historical tool and directory names remain stable.
The separate [sequence manifest](../compatibility/m2-class-lifecycle/manifest.json)
declares ordered property-path calls within one evaluator and module. Each
sequence starts with a fresh evaluator and repeats in a second fresh evaluator;
errors within a sequence do not reset its evaluator. Fixture helper properties
contain the expressions being observed. The Java and Rust adapters use the same
source bytes, fixed `file:///m2-platform.pkl` URI, and disabled power assertions.

```sh
python3 tools/m2_class_lifecycle.py --output target/m2/class-lifecycle-new --record-only
python3 tools/m2_class_lifecycle.py --verify-report target/m2/class-lifecycle-new
python3 tools/m2_class_lifecycle.py --verify-contract target/m2/class-lifecycle-new
```

The runner retains source snapshots, the recorded Rust build and frozen
executable, Java classes and oracle identity, ordered process markers, and each
call's complete status/stdout/stderr bytes. Java exception class names are
provenance metadata; they are not compared to Rust implementation type names.
The report verifier recomputes comparisons from the captured bytes. The bounded
contract requires every selected call to match, with no mismatch allowance.
`--java-only` acquires exploratory oracle observations and cannot pass that
contract. Unexecuted groups and nonterminating oracle observations remain
explicit gaps. Sequence executions and call comparisons have their own counts;
they are not added to the independent-expression selection or treated as full
language coverage.

The shared `evaluateLanguageSequence` development binding returns ordered
`value\t...` or `error\t...` strings in Node and Wasm, preserving complete value
and diagnostic payloads. Native acquisition uses the same runtime boundary.
Rust collects between calls while retaining the module root. Dedicated native
tests separately verify evaluator-owned class and alias roots, disposable object
reclamation, isolated heaps, ordinary failed-property retries, and terminal
cleanup. Internal bytecode suspension tests retain their scheduler-only scope.
Stage 35 also exercises actual annotation source containing `read` in eager
construction and Deprecated message evaluation. The `annotation_host_probe`
adapter retains the acquired module, performs nested callback evaluations, and
compares complete ordered Java/native status, stdout, and stderr. Native tests
separately inspect annotation roots, temporary-builder reclamation, stale
completion rejection, evaluator isolation, nonterminal cancellation, and terminal
release. These contracts do not establish Host I/O in `evaluateLanguage`, full
module-cache behavior, or arbitrary retries after failed module setup.

Alias sequences cover value identity, generic metadata, failed definition reuse,
and later type uses in properties, deferred annotations, constructors, and
superclasses. They preserve the original class sequence definitions and raw
call outcomes. Their counts remain separate from independent expressions and
native GC/Host tests. Ordered-resolution sequences additionally distinguish
missing names before and after alias initialization, generic parameter shadowing,
nested substitutions, and unfinished aliases with failing arguments. Independent
cases compare class and alias argument-count error ordering, member signatures,
and complete diagnostic frames. Parameter renaming and nested alias expansion
provide separate metamorphic relations. Native and shared platform scenarios
check retained declaration state after the same failures.

Type-use ownership sequences preserve local const-property resolution after a
failed first read. They distinguish independent inline and lambda uses, shared
non-generic let roots, recursive depths, inherited annotation declarations, and
successive members of lazy Listing and Mapping wrappers. Repeated calls resolve
the current receiver and owner while retaining the member definition. Separate
native tests cover Host reentry, cancellation, GC, evaluator isolation, and
terminal release. Const-method lexical barriers and contextual grouped-callee
syntax errors also have independent raw Java comparisons. Stage 18 extends this
contract to finite recursive generics with changing arguments, actual-argument
capture, shared generic let roots and their unbound type variables, and let/for
const barriers. Original observations and failed hypotheses remain separate
from the selected additions. Stage 19 shares instantiated argument graphs,
reclaims them iteratively, and schedules nested defaults with VM fuel and GC
roots. Deep small-stack and process-isolated lifecycle tests verify bounded
execution and cancellation of the original abort input; its historical failure
bytes remain preserved. The adapter instruction limit does not match Java's
stack-overflow diagnostic. Reference metadata traversal within one instruction
and output expansion retain resource and cancellation-latency limitations. The
complete generic, const, and diagnostic corpus remains open.

Stage 20 retains a nullable type's inner default in the evaluator-owned heap.
Selected cases distinguish empty amendments, which expose one attached default,
from amendments with members, which unwrap nested defaults. Null equality,
hashing, type checks, and output remain independent of the attachment;
null-propagating operations discard it. Generic collection defaults use the
existing scheduler and GC roots. Captured closures and Host failure, retry,
cancellation, isolation, and terminal cleanup have separate lifecycle checks.
Direct generic constructors and remaining shared-let and Reference differences
remain outside this selected contract.

Stage 21 adds selected direct generic constructors, including distinct class
actuals, nullable and union defaults, deferred failures, captured functions,
and complete errors for invalid actuals. Separate source-origin metadata keeps
authored alias occurrences and forwarded parameters available to diagnostics.
Collection-body controls preserve Java's absence of an added collection cast
when obtaining the parent for `new T`. Persistent sequences alternate actual
types and retry failed reads within the same evaluator. Native checks exercise
Host reentry, GC, cancellation, isolation, terminal release, and 768-deep default
construction on a 256 KiB evaluator stack. Lazy member frame names, unsupported
builtin types, some missing-property diagnostics, and generator boundaries
remain recorded exploratory failures rather than new development exceptions.

Stage 22 adds selected strict Map construction, lookup, defaults, type checks,
iteration, equality, hashing, and non-forcing stringification. Raw cases separate
insertion order from hash-trie traversal and include boxed numeric keys, duplicate
replacement, nested Map keys, lazy failures, and full diagnostics. Native
lifecycle checks and ordered Java sequences retain their separate contracts.
Complete Map APIs, JVM identity-dependent hash order, and general compound
rendering remain open.

Stage 23 adds selected strict Set construction, duplicate elimination, contains,
defaults, element type checks, equality, hashing, iteration, spread, and
non-forcing multiline strings. Constructor lookup and insertion are separate
resumable phases, including repeated zero-hash preparation. Raw cases and
persistent sequences observe boxed numeric elements, trie boundaries, nested
collections, failed-read retry, and cache-sensitive element order. General
compound result controls remain unselected failures; separate scalar observations
check Set iteration without extending the adapter to general object output.
Higher-order Set APIs and the complete collection corpus remain open.
Nested Map key-rehash side effects during Java value rendering also remain
outside the selected stringification contract.

Stage 24 adds selected `Collection<T>` type semantics. It distinguishes List and
Set values from other collections, preserves nested lazy List checks and eager
Set checks, and covers bare types, aliases, defaults, nullable attached defaults,
casts, union checking and normalized Reference metadata. Explicit empty generic
argument lists now produce the pinned parser diagnostic, including List, Set,
Map and alias controls. Stage 24 left the original parser depth and node limits unchanged.
Persistent sequences and native Host/GC checks retain independent contracts.
Unselected controls preserve String-selected alias-union constructor diagnostic
differences, unsupported member APIs and general compound outputs. These are
remaining gaps, not additional development exceptions.

Stage 25 addresses selected authored frontend depth. Ordinary expression parsing,
resolution and emission use explicit work stacks, with iterative expression-tree
copying and release. Constant List keys and power-assertion anchors have their own
iterative traversals. The selected Java corpus and process-isolated native checks
cover different contracts: exact output and diagnostics versus bounded stack and
lifecycle behavior. Deep type compilation and nested lexical string interpolation
retain separate limitations: nested lexical strings at depth 64 still abort on the
recorded 256 KiB stack. Type-depth 24/31 checks cover parsing, copying and release,
not full type compilation. If/let/lambda and property-only Dynamic object paths
have separate accepted-depth 24/31 full-frontend checks. Class/instance and
mixed/generator compiler paths, and recursive AST Debug/PartialEq, remain outside
that success claim. The historical runtime depth-768 tests construct values after
compilation and do not replace authored-source frontend validation.

A complete runtime type-resolution model, general imported types, complete
annotation metadata/reflection, and full module initialization/caching remain
open requirements. The
persistent adapter requires an acquired module; module-entry failures do not
establish later module retry or cache behavior.

The collection-type selection compares lazy and eager checking, wrapper
composition, aliases, defaults, constraint identity, cache reuse, and complete
diagnostics at the actual fixture URI. Exploratory captures remain separate:
unimplemented format-time cache stealing can cause further checks while Java
renders a diagnostic, including uncaught Java rendering failures. These
observations do not add development exceptions or establish full generic
collection compatibility. See the [checkpoint evidence](../reports/m2/README.md).
