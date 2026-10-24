# Class values and superclass verification

This contract covers the shared runtime's representation and observation of Pkl
0.32.0 Class values. The independently pinned
[Java fixture](../tests/fixtures/m2-class-values/README.md) supplies the reference
bytes. A matching source-query matrix does not establish complete public API
compatibility, full language support, or completion of M2.

## Identity, prototypes, and superclass semantics

A Class value identifies a declaration or builtin class. Its instance prototype
is a separate object that owns instance members and participates in inheritance.
Class identity, prototype identity, and the superclass relation must remain
distinct. Looking up `Person.foo` on a Class value must use the Class receiver;
it must not read the `foo` property from Person's instance prototype.

Builtin and user classes use canonical identities within their applicable
evaluation lifetime. Class values have the canonical `Class` runtime class.
`simpleName` is observable metadata, not an identity key: classes with the same
module and class names at different source URIs can have identical display names
while comparing unequal and occupying distinct Map/Set keys. Source identity
and module caching remain part of the contract.

User classes normally inherit from Typed, which inherits Object and Any. Builtin
classes retain their actual declared superclass chains. A plain module and an
extending module have different module Class identities; an amending module
retains the original module Class. Their simple names can all be `ModuleClass`.
Type aliases are values of runtime class `TypeAlias`; they must not create new
Class identities merely because they name an existing type.

`Class<T>` checks erase the represented type argument under the pinned Java
rules. Function classes are canonical by arity, from Function0 through
Function5, independently of parameter names or signatures. The six-parameter
literal control records an actual language error. Unsupported arities must not
be represented by fabricated Function6 class names.

The Java Class initializer publishes a cached Class before resolving its
superclass, evaluating annotations, and publishing properties and methods.
Initialization failures can therefore expose a partial Class to later queries
on the same evaluator. Reentry, declaration order, successful prefixes, failure
locations, and retained identity must be observed together. A failed
initialization must not be replaced with a fresh declaration or silently treated
as a fully initialized class.

The pinned superclass check permits external superclass extension when the
declaring module name begins with `pkl.`. Permission to declare an external
member is a separate check based on the module key. Neither rule grants general
standard-library trust to an arbitrary URI or module name. Declared-type
intrinsics also have a separate exact `pkl.base` module-name/simple-name dispatch:
the fixture observes a user Class named Int remaining a distinct value while
type/new expressions select builtin Int behavior.

These rules follow the pinned
[Class initializer](../external/pkl/pkl-core/src/main/java/org/pkl/core/ast/member/ClassNode.java),
[declared-type resolver](../external/pkl/pkl-core/src/main/java/org/pkl/core/ast/type/UnresolvedTypeNode.java),
and [base declarations](../external/pkl/stdlib/base.pkl). Namespace and display
metadata must not replace runtime declaration identity.

## Representation checks and converter selection

The superclass graph does not determine every builtin type check. Java's
TypedTypeNode accepts the `VmTyped` representation. The actual fixture includes
a class extending Any in a `pkl.`-named module whose instance still satisfies
`is Typed`. Representation, runtime Class identity, and subclass membership must
not be collapsed into one predicate. The pinned
[type nodes](../external/pkl/pkl-core/src/main/java/org/pkl/core/ast/type/TypeNode.java)
define the relevant specializations.

Explicit `value.getClass()` uses normal virtual dispatch when a user overrides
that method. Intrinsic runtime-class lookup used by type tests and converters
still observes the actual value class. The fixture's override returns String,
while the value remains an instance of its user class and selects that class's
renderer converter.

Converter matching is also representation dependent. Java's
[PklConverter](../external/pkl/pkl-core/src/main/java/org/pkl/core/stdlib/PklConverter.java)
walks the actual superclass chain for `VmTyped` values and chooses the nearest
available type converter. Primitive values, strict collections, Class values,
and Function values use exact runtime-class converter lookup. A Function
converter does not substitute for a Function1 converter, and Number/Collection
converters do not imply conversion of Int/Float/List/Set values. Path converters
take priority over type converters. Class metadata support must not be confused
with implementation of DataSize values, Bytes/IntSeq/Regex constructors, or
their operations.

## Independent Java authority

Java authority A was committed as
`31eef43ad2473e8977d7844c4fa5af087392072c`. Its
[checkpoint](../reports/m2/stage56-oracle-checkpoint.json) and
[validation archive](../reports/m2/stage56-oracle-validation-evidence.tar.gz)
preserve local provenance and previously recorded checks. The original
checkpoint's pending commit-binding statement is historical; the separate
[post-commit binding](../reports/m2/stage56-oracle-source-binding.json) records
the actual Git objects for all 47 frozen Java fixture/tool/test inputs.

The fixture pins Pkl source revision
`e51fb2f91774265666f94980ec630db794a4db2d` and tools JAR SHA256
`c3b6f6215a08b31453b0fb0edd8c9da0aee31233f65abb8ab07f9c945151e059`.
The canonical archive SHA256 is
`a39060fbf155947212aaae5b2e950dc2cd04381a78cb2860a7f8b1675b3657fc`.
Independent archive and file-set pins prevent replacing the reference by
refreshing local manifest hashes.

| Observation boundary | Actual Java acquisition | Candidate obligation |
| --- | --- | --- |
| Common query/logger protocol | 44 sessions, two fresh JVM repetitions; 558 evaluation calls: 462 successes, 86 language errors, 6 oracle errors, 4 API exceptions | Compare every requested session and every common channel |
| Module-provider SPI | 578 actual factory-create and 10 resolve callbacks, with key properties and raw provider protocol | Required; current candidate provider API parity is not executed |
| Public exported Class identity | Two JVMs, 14 evaluation calls and 8 reference/equals comparisons; 12 successful evaluations and 2 post-close API exceptions | Required; current candidate public export API parity is not executed |

All 44 main repetition pairs and the exported-Class repetition pair have equal
raw observations. The archive also preserves three earlier complete main
acquisitions, including invalid inputs and query-scope outcomes. These histories
are not additional independent feature coverage or candidate passes.

The main uses the exact supplied `evaluateExpressionString` expressions and
module URI. Each session reuses one evaluator and its module state; each public
query compiles its own query AST. Precompiled path-query caches or wrapper
expressions must not change that boundary. Authored support modules retain their
exact source bytes and URI mappings.

## Raw channels and callback ordering

The common boundary includes complete result/error UTF-8 bytes, UTF-16BE code
units, statuses, logger fields, and the ordered process protocol. Logger frames
retain URI, nullable member name, every source line, and all four inclusive
positions. Empty and null members differ. Lone surrogates must remain intact in
the code-unit channels; Java's UTF-8 encoding is retained separately. Comparison
must not trim, sort, normalize, or reserialize public payloads.

Calls use `outer-N` and `log-callback-N` prefixes. The protocol records call and
logger entry/exit, logger rejection or evaluator closure, and terminal counts.
The logger supports capture/noop modes and none/throw-once/close actions. The
callback threshold is a zero-based logger entry index. Callback use is marked
before reentry; the one-shot throw or close is selected after that callback
returns. A nested callback can consume the action before an outer entry resumes.
Subsequent queries are attempted after closure so actual API failures remain
observable.

The Java provider's separate `provider/` family records actual public
ModuleKeyFactory.create and ModuleKey.resolve calls, including unmatched factory
requests. Its support keys are hierarchical, nonglobbable, cached, nonlocal,
retain their original URI when resolved, and perform the security check.
Candidate Host requests are recorded as actual candidate traces; they must not
be relabelled as Java SPI events or used to invent cache-hit observations.

Java `*.error-class.bin` fields and exported Java value-class fields are producer
provenance. Candidates must not synthesize Java class names. Per-call candidate
disposition, phase, reason, API-error kind, and callback-origin fields are
separate metadata. They must belong to actual public calls and agree with their
statuses. Missing, extra, partial, or ghost channels remain failures.

## Candidate acquisition and verdicts

`tools/class_values_compare.py` verifies the independent fixture before executing
the complete main selection. It accepts separately built native and paired
Wasm/N-API artifacts; it does not build candidates or infer a recorded-build
claim from a supplied binary. A separate build/source record must bind those
artifacts to the implementation.

Every capture needs a fresh output directory, frozen artifact copies, an exact
source-catalog snapshot, source/artifact hashes before and after execution,
actual runtime/environment identities, complete process output, and timeout or
launch-failure accounting. Source catalog and artifact closure are verified
independently of report-declared results. Browser tooling follows the pinned
Playwright package/lock configuration.

Native sessions run in fresh processes. N-API runs use a fresh Node process per
case and repetition; Chromium uses a fresh browser context and Wasm instance.
Architecture checks include owned event buffers, admitted execution profiles,
poll/collection observations, and zero heap slots/context depth after closure
where a session was created. These checks do not establish API compatibility,
RSS bounds, or performance targets.

| Mode | Meaning |
| --- | --- |
| `--output ... --record-only` | Acquire and retain the complete requested main matrix, including nonpasses |
| `--verify-report` | Offline evidence integrity and accounting; recorded failures remain failures |
| `--verify-main` | Require every main session on every requested target to pass the common query/logger/protocol comparison |
| `--verify-full` | Full public API gate; currently fails while provider/export supplements remain `not-executed-required` |

Actual Java oracle errors and API exceptions remain nonpasses even when common
bytes agree. Candidate unsupported, limits, internal errors, acquisition failures,
and unmatched bytes never pass. The report retains all nonpass reasons as well
as its primary status. A record-only or integrity exit zero is not a strict
comparison pass. The current full API status is
`blocked-unexecuted-required`, with `full_public_api_comparison_passed=false`;
common comparison success cannot promote it.

The preserved native development packet
`target/m2/stage56-class-native-dev-01` contains 88 session executions: 58 pass,
14 mismatch, 10 candidate-unsupported, and 6 oracle-error. It is an observation
of that frozen artifact, not a final result for subsequent core revisions.
At this documentation checkpoint, the new portable capture drivers exist but
the current Stage56 binding artifacts have not executed their Class matrix.
Earlier platform builds or Java replay cannot substitute for that execution.

The second native packet, `target/m2/stage56-class-native-dev-02`, records 78
passes, 4 mismatches, and 6 oracle errors across the same 88 executions. The
remaining ordinary mismatches are in value construction for DataSize, Bytes,
IntSeq, and Regex; the annotation callback nonpasses remain recorded. This is
the second frozen candidate's result, not verification of later descriptor
additions or platform artifacts. A separate complete base-declaration review
also identified missing Class metadata outside the original selected names.

## Available commands

Offline authority verification needs no JVM, network, or original target packet:

```sh
python3 -B tools/class_values_fixtures.py
python3 -B -m unittest discover -s tools/tests -p test_class_values_fixtures.py
```

New Java observations require the existing pinned oracle cache. They create
independent packets and do not automatically replace the committed authority:

```sh
python3 -B tools/class_values_oracle.py --output target/m2/class-java-new
python3 -B tools/class_values_oracle.py --verify-report target/m2/class-java-new
python3 -B tools/class_values_exports.py --output target/m2/class-exports-new
```

For a new native comparison, record the build with the repository toolchain:

```sh
cargo build -p pkl-runtime --example class_values_fixture_probe --release --locked
python3 -B tools/class_values_compare.py --output target/m2/class-native-new --native target/release/examples/class_values_fixture_probe --record-only
python3 -B tools/class_values_compare.py --verify-report target/m2/class-native-new
python3 -B tools/class_values_compare.py --verify-main target/m2/class-native-new
python3 -B tools/class_values_compare.py --verify-full target/m2/class-native-new
```

With separately recorded compatible Wasm/N-API builds, execute all three targets:

```sh
python3 -B tools/class_values_compare.py --output target/m2/class-all-new --native target/release/examples/class_values_fixture_probe --wasm target/m2/platforms-new/wasm --napi target/m2/platforms-new/pkl_napi.node --record-only
```

The native helper's direct interface is
`DIRECTORY URI LOGGER_MODE LOGGER_ACTION CALLBACK CALLBACK_AFTER`.
`input.pkl`, newline-delimited `expressions.txt`, and `modules.tsv` with exact
`URI<TAB>support-N.pkl` mappings define its inputs. The comparator and portable
runner generate these inputs from the unchanged fixture rather than deriving
expected results from the candidate.

## Remaining completion obligations

Provider SPI and public Class export parity, complete lifecycle behavior,
cross-evaluator standard-library state, unimplemented primitive values and
constructors, reflection, Typed/Dynamic projections, module policies, and the
remaining language/API inventory stay open. Retained Class/prototype/source/code
roots, suspension, reentry, rejection, cancellation, closure, and eventual
reclamation require actual lifecycle verification on the shared engine.

The [whole-language contract](m2-language.md), [verification criteria](../TESTING.md),
and [project intent](../INTENT.md) remain authoritative. Required Rust checks,
inventory freshness, source-query comparisons, actual platform execution,
original-source regressions, and the strict whole M2 gate are separate
obligations. Neither allocation counters nor matching selected strings prove
the speed/RSS targets or full product compatibility.
