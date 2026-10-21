# Validation Contract

The goals are behavioral and byte compatibility with the Java implementation of Pkl 0.32.0, with execution time and memory usage at least as good as Java for every required workload. Source verification, inventories, a pinned Java oracle, and raw comparison tooling are available. M1 adds bounded Rust/Java architecture comparisons and actual browser/Node execution; complete language and product performance gates remain open.

## Current Checks

```sh
cargo xtask verify
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo check -p pkl-runtime --target wasm32-unknown-unknown --locked
```

The first command checks reference pins. The remaining commands check Rust formatting, run static analysis and the current workspace tests, and verify Wasm compilation. Successful Wasm compilation does not establish browser evaluation or API compatibility. Do not use the number of passing `cargo test` cases as a measure of Pkl feature completion.

Python verification tools use the standard library; use Python 3.12 or newer. CI selects Python 3.12. The [oracle contract](docs/oracle.md) describes the exact JDK requirement, artifact and source pins, execution environment, and evidence. With that JDK installed:

```sh
python3 tools/oracle.py fetch
python3 tools/oracle.py verify
python3 tools/oracle.py smoke
python3 -m unittest discover -s tools/tests -p 'test_oracle.py'
```

Oracle smoke checks capture Java success and error bytes and verify exact Java replay. They do not execute a Rust evaluator. The separately pinned optimizing GraalVM profile is used for performance baselines.

The [source inventory](docs/inventory.md) tracks language, stdlib, builtin, renderer, CLI, protocol, and JVM source declarations. Regeneration detects missing entries and source drift:

```sh
python3 tools/inventory.py --check
python3 -m unittest discover -s tools/tests -p 'test_inventory.py'
```

Source declaration counts and builtin registration matches are not semantic coverage or proof of compiled JVM API compatibility. Implementation and behavioral test mappings remain explicit unimplemented/unmapped records.

The [corpus contract](docs/corpus.md) accounts for reference files, resources, runnable command cases, and suites requiring additional adapters:

```sh
python3 tools/discover.py --check
python3 -m unittest discover -s tools/tests -p 'test_discover.py'
```

Ready commands have not necessarily run. Blocked suites and cases with an outcome requiring observation remain visible; file counts are not assertion counts. Historical rev1 inputs are inventoried separately. [Three selected regressions](docs/regressions.md) retain their original input/license bytes and have fresh Java 0.32.0 outcomes; the remaining historical candidates stay unselected.

```sh
python3 tools/regression_manifest.py --check
python3 -m unittest discover -s tools/tests -p 'test_regressions.py'
```

The [execution tool](docs/compatibility-tooling.md) records and checks a required seven-case smoke corpus, including error bytes and multiple generated files:

```sh
python3 tools/smoke_manifest.py --check
python3 -m unittest discover -s tools/tests -p 'test_compat.py'
python3 tools/compat.py record --manifest compatibility/smoke.json --output target/m0/smoke-baseline
python3 tools/compat.py check --manifest compatibility/smoke.json --output target/m0/smoke-baseline
```

Use a new evidence directory for each run. Candidate comparison requires a complete Java baseline and the same workspace path; the tool verifies source/resource hashes, runner provenance, raw captures, and case coverage. Java-to-Java replay validates tooling only. The process adapter currently requires POSIX process groups. The full discovered corpus includes blocked cases and therefore fails the strict completion gate until all required adapters and execution conditions exist; the smoke corpus is not a substitute for full compatibility.

Reviewable evidence archives under `reports/m0/` retain raw outputs and a per-file checksum inventory. Verify an archive with `python3 tools/evidence.py verify <archive.tar.gz>`. Copied source workspaces can be excluded explicitly; the archive records its exclusions and retains the pinned source manifest.

Pantry has a separate [development and packaged-module contract](docs/corpus.md):

```sh
python3 -m unittest discover -s tools/tests -p 'test_pantry.py'
python3 tools/pantry.py --workers 4
python3 tools/evidence.py verify reports/m0/pantry-evidence.tar.gz
```

The runner requires a new `target/m0/pantry` evidence directory. It runs package/API tests, evaluates generated ZIP modules, checks configured generators and the tar pipeline, and records dependency payload hashes. Abstract or user-configured module errors remain provisional observations. Reported test/assertion invocation totals can repeat dependency suites; they are not distinct coverage counts. Unfixed live GitHub discovery inputs remain outside the hermetic baseline.

Run all verification-tool regressions with `python3 -m unittest discover -s tools/tests -v`. The [M0 report](reports/m0/README.md) links recorded outcomes and raw evidence. The [CI contract](docs/ci.md) distinguishes the required tooling smoke run from optional full-corpus acquisition and performance measurements.

## Compatibility Comparisons

M2 language implementation and verification are in progress. The
[M2 contract](docs/m2-language.md) separates whole-language requirement accounting
from the executable selection:

```sh
python3 tools/m2_manifest.py --check
python3 tools/m2_language.py --output target/m2/language-new --record-only
python3 tools/m2_language.py --verify-report target/m2/language-new
python3 tools/m2_language.py --verify-development target/m2/language-new
python3 tools/m2_language.py --verify-subset target/m2/language-new
python3 tools/m2_language.py --verify-full target/m2/language-new
```

The full gate must fail while required language implementations, mappings,
adapters, or passing executions are missing. It cannot be replaced with the
selected-case gate.

Class and type-alias initialization have a separate persistent-evaluator contract:

```sh
python3 tools/m2_class_lifecycle.py --output target/m2/class-lifecycle-new --record-only
python3 tools/m2_class_lifecycle.py --verify-report target/m2/class-lifecycle-new
python3 tools/m2_class_lifecycle.py --verify-contract target/m2/class-lifecycle-new
```

Each sequence retains one module and evaluator across ordered calls, including
errors, and repeats with a fresh evaluator. Every selected call must match its
raw Java status, stdout, and stderr without exceptions. These sequences have
separate execution/call counts and do not close the full language gate. Native
GC and Host lifecycle tests separately check class and alias roots, temporary reclamation,
ordinary lazy retries, and terminal release. See the
[sequence contract](docs/m2-language.md#persistent-class-initialization-sequences)
for source/build provenance, adapters, and remaining scope.

Type-resolution regressions compare authored child order, base-name failures,
class versus alias arity checks, generic parameter shadowing, nested substitution,
and owner signature initialization. Repeated calls preserve failed declaration
state while ordinary lazy expressions retry. Native Host/GC tests separately
exercise nested generic constraints, suspended values, heap isolation, temporary
reclamation, nonterminal cancellation, and terminal cleanup. Node and browser
sequence scenarios retain the same Java-derived complete payloads.

Collection regressions cover lazy bodies, eager keys and source forcing,
inheritance order, primitive key equality, generator captures, spread, and
complete subscript diagnostics. Native lifecycle tests also force collection
construction through nested Host suspension, GC, retry, cancellation, and
timeouts. Shared Node/browser scenarios exercise the same collection engine and
Java-derived error bytes. Generic collection regressions additionally cover
lazy `Listing<T>` values, eager `Mapping<K,V>` keys, strict `List<T>` members,
nested wrappers, casts, unions, aliases, defaults, cache reuse, and first errors.
Lifecycle checks retain temporary and successful wrapped values across GC,
Host suspension, reentry, failure retry, and terminal cleanup. Complete custom
defaults, format-time wrapper checks, and the full collection corpus remain
unverified.

Member predicate regressions compare parent-only selection, interleaved forcing
and condition evaluation, declaration-owner caches, replacement versus amendment,
lexical and custom `this` scopes, nested generators, duplicates, const checks,
and complete diagnostics. Dedicated lifecycle tests retain the parent cursor,
prior custom receiver, and unpublished target across Host suspension, nested
evaluation, GC, retry, cancellation, close, and timeouts.

Structural equality regressions distinguish scalar numeric comparisons from
boxed nested values and keys, including NaN and signed zero. They cover object
kind and class identity, empty-body identity, deep forcing order, inherited
overrides, wrapper checks, and strict List shortcuts. Key cases include lazy
small maps, the fifth-member hash transition, ancestor lookup, Mapping length,
and complete duplicate/missing-key diagnostics. Constant List keys are checked
before evaluation only when every key in a specialized body is constant;
dynamic keys, generators, and shadowed constructors retain runtime behavior.
Signed literal token spans are preserved separately from expression spans.
Source-line rendering retains the pinned highlighter's interpolation whitespace
behavior and whole-line fallback on lexical errors; underlines still use
authored UTF-16 positions.
Lifecycle tests exercise temporary operands and keys through Host suspension,
GC, reentry, retry, and terminal cleanup. Cyclic keys and complete Java
hash-bucket/cache behavior remain outside the verified selection.

Additional comparison regressions cover ordinary typed `default` properties,
inherited and hidden properties, completed regular-member cache order, and
class declaration order independently of amendment order. Key preparation
cases distinguish whole-object deep forcing from subsequent recursive member
hash preparation. Host lifecycle tests cross this phase boundary and preserve
completed values while retrying failed reads. Actual Node and Chromium scenarios
compare values and complete diagnostics captured from the pinned Java oracle at
the identical module URI.

Map regressions distinguish structural equality from numeric hashes and bucket
membership. Pinned JAR observations supply collision order, signed hash mixing,
four-to-five entry transitions, load growth, capacity boundaries, and partial
rehash visibility. Language cases cover warmed Listing keys, constant and
nonconstant local caches, scalar hashes, and class qualified names. Host tests
publish a value before suspending in key hashing, then fail, retry, traverse old
and new cache slots, and collect between VM steps. They separately exercise
cancel, timeout, close, and stale completions. These lifecycle checks are not
raw Java byte comparisons or proof of same-cache cyclic reentry compatibility.
Mapping length regressions separately check HashSet full-hash filtering,
directional local-cache equality, inherited structural overrides, and boxed
numeric distinctions. Its Host test retains ancestor values across a failed
child hash and retries the count without forcing entry values.

Let diagnostics compare complete Java output for nested bodies, initializer
failures, typed bindings, captured members/functions, and constraints with their
own logical root. Recursive call controls distinguish inserted let frames from
the still-open repeated-frame compression requirement. Failed exploratory
captures remain retained alongside promoted regressions.

M1 experiments have narrower, explicit contracts:

```sh
python3 tools/m1_frontend.py --check-contract
python3 tools/m1_semantics.py --check
python3 tools/m1_frontend.py --output target/m1/frontend-new
python3 tools/m1_runtime.py --output target/m1/runtime-new
python3 tools/m1_host.py --output target/m1/host-new
python3 tools/m1_lifecycle.py --output target/m1/lifecycle-new
python3 tools/m1_platforms.py --output target/m1/platforms-new
```

See [frontend](docs/m1-frontend.md), [runtime](docs/m1-runtime.md),
[semantics](docs/m1-semantics.md), [Host](docs/m1-host.md), and
[platform](docs/m1-platforms.md) contracts, plus the
[closing lifecycle comparison](docs/m1-lifecycle.md). Primitive comparisons are not full
Pkl renderer/error comparisons. The latest M2 regression capture matches two of
the three original runtime diagnostics; the cycle difference remains. The
platform command returns nonzero for its two known Java regex
backend differences. CI uses the separately named architecture contract mode,
which still reports failed compatibility cases and requires the exact gap
inventory. A green experiment job cannot establish release conformance.

Run `python3 tools/m1_runtime.py --benchmark --output target/m1/performance-new`
on a quiet host with the separately pinned optimizing Java profile. Preserve
negative results, convergence data, compilation traces, and the different API
boundaries; do not interpret a prototype ratio as a product performance claim.

Run `python3 tools/m1_graal.py --output target/m1/graal-new` to reproduce the
[Graal compilation investigation](docs/m1-graal.md) on the pinned macOS arm64 JDK.
Use `python3 tools/m1_graal.py --check-report target/m1/graal-new` to verify the
stored raw evidence without executing that JDK. The diagnostic class override
is an explicitly separate causal control, never an oracle or performance update.
The unchanged-JAR positive workload requires actual guest Tier 2 compilation
and per-call output checks. These checks do not measure performance.

Run the pinned Java 0.32.0 oracle and Rust with identical inputs and environments. Compare:

- Raw stdout and stderr bytes, and exit codes.
- The set of generated files, their paths, and their raw bytes.
- Error messages, whitespace, source locations, underlines, call frames, hints, and error order.
- CLI options and defaults, REPL state, and API results, errors, cancellation, and close behavior.
- Server protocol encoding, callbacks, and session lifecycles.

Fix all inputs that affect results, including working paths, locale, time zone, terminal settings, environment variables, and network responses. Resolve path differences through the execution environment, such as identical mounts. Formal comparisons must not remove differences with `trim`, sorting, JSON reserialization, or arbitrary string substitutions.

Protocol fields that vary between runs, such as Java's random evaluator IDs, need a separate contract. Codec tests supplied with fixed IDs compare all bytes. Tests of actual sessions verify that issued IDs are referenced consistently and validate result payloads; they must not be described as byte equivalence for the entire session.

Generate expected results from the Java oracle and review their provenance and changes. Do not resolve mismatches by overwriting expected results with the current Rust output.

## Planned Test Layers

| Layer | Scope |
| --- | --- |
| Test tooling | Failure detection for empty corpora, missing expected results, unavailable runners, and byte differences |
| Parser and resolver | Syntax, invalid input, source spans, name resolution, and formatter round trips |
| Runtime and types | Lazy forcing, inheritance, closures, cycles, constraints, generics, and the first error |
| Standard library and renderers | Every public member, boundary values, output order, converters, and binary output |
| CLI, REPL, and server | Help and options, PTY sessions, state retention, protocol, timeouts, and close behavior |
| I/O and packages | HTTP fixtures, redirects, proxies, CAs, root restrictions, checksums, caches, and lockfiles |
| Pantry | Development trees, packaged modules, real examples, and generated files at the pinned revision |
| Platforms | Native, Wasm, and N-API results, errors, and Host capabilities |
| Generation and transformation | Grammar-based generation, differential fuzzing, property tests, and metamorphic tests |
| Long-running execution | Repeated successes and failures, growing sets of unique sources, cancellation, evaluator creation and shutdown, and memory reclamation |

Each test manifest records the case ID, source repository, revision and path, input and resource hashes, execution arguments and environment, required Host capabilities, expected success or error, channels and files to validate, and execution targets. Pinning a Pantry commit alone does not establish 0.32.0 compatibility, so first verify the selected cases and execution conditions with the Java oracle.

Distinguish passes, failures, tests not run, unimplemented cases, and environment errors. Do not count skips or known failures as passes. Add minimal reproductions of failures to the required regression cases, and compare the test inventory with actual execution results to detect omissions.

Reference regressions compare import identity, constructor kinds, declaration
metadata, unforced referent bodies, normalized types, synthetic paths, equality,
key behavior, and complete diagnostics, including frames from the pinned
`pkl:ref` source. Native lifecycle tests exercise domain rendering across Host
suspension, nested evaluation, GC, retry, cancellation, close, and timeout.
Node and Chromium execute the same engine for Reference values and errors.
Original upstream files evaluated through a selected API expression remain
exploratory evidence; those calls do not replace their original renderer or
snippet-suite contracts. Reviewed implementation/test links preserve these
unexecuted cases and the full completion gate.

## Performance and Memory

The [initial Java benchmark contract](docs/benchmarks.md) implements cold CLI, persistent evaluator, and fresh evaluator measurements for six fixtures. Use a quiet host with the pinned optimizing runtime:

```sh
python3 tools/oracle.py fetch --profile performance
python3 tools/benchmark.py --check-contract
python3 tools/benchmark.py
python3 tools/benchmark.py --check-report target/m0/benchmarks
```

Every run uses a new evidence directory. Raw bytes, per-process RSS, first requests, warmup windows, and measurement windows are retained. Nonconverged forks remain inconclusive. These initial Java measurements do not complete the required workload inventory or provide a Rust performance verdict.

Only runs that produce correct output are eligible for measurement. Every required workload must satisfy both conditions:

```text
Rust execution time / Java execution time ≤ 1.00
Rust peak RSS / Java peak RSS ≤ 1.00
```

Fix the Java executable artifacts, JDK and Graal/Truffle configuration, hardware, OS, and measurement method. Compare against a sufficiently warmed-up JVM and record results for the official native-image CLI separately. Do not judge the overall performance target solely by cold JVM startup time.

| Usage | Measurement Boundary |
| --- | --- |
| CLI | From process startup through completed output and process exit |
| Persistent evaluator | Initialization, first request, and repeated requests measured separately |
| REPL and server | Session creation, request latency, long-term memory usage, and shutdown |
| N-API | Includes JS↔Rust value conversion and result delivery |
| Wasm | Download, compilation, and initialization measured separately from requests to a persistent instance |
| Packages and large inputs | Cold and warm caches, import graphs, large outputs, and failure paths |

Record latency distributions and peak RSS across repeated runs for each workload. Report results as inconclusive when measurement uncertainty prevents a judgment. Do not use average improvements to conceal regressions in individual required workloads.

For Wasm and N-API, first define measurement boundaries that include the Host process. JVM heap usage, Rust allocations, and Wasm linear memory are diagnostic metrics; do not compare these different metrics directly to claim equivalent memory usage.

## Release Criteria

The required feature and API inventory must have no omissions, and there must be zero known mismatches within the required scope on target platforms. Completion cannot be declared while required tests remain unrun or unimplemented, or performance and memory measurements remain inconclusive. Do not document complete compatibility or guarantee performance equivalent to Java before these criteria are met.
