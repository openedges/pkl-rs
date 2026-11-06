# pkl-rs

The [Stage156 scoped baseline](reports/m2/stage156-baseline/README.md) is the current canonical checkpoint.
It separates the Rust and full-ready source epochs, retained failures, integrity
and strict completion results. Baseline acquisition is complete; M2 remains
in progress, with package fixture qualification and bounded shared metadata
acceptance next, followed by package Host integration. Full language/API, release-platform and performance
gates remain open.

The checkpoint descriptions below retain their historical source and execution
scope; they are not current-source reruns.

Stage153–155 adds exact original T40 adapters, native file-URI byte/text handling and bounded shared package URI validation. Two original CLI contracts and two separately counted Java cache-off controls match; file URI records 19 boundary matches/5 nonpasses and package API 6 raw value matches/31 nonpasses. Source06’s full workspace failed (2,190 passed, 1 failed, 6 ignored); source07 separately passes the corrected 15-test target, 83 Python tests and five prechecks. Fresh bounded M1 matches 6,078 results per Native/Node/Chromium target and 92 scenarios per Node/browser. Full M2/API, package/Host, release-platform and performance gates remain open. See the [Stage153–155 report](reports/m2/stage153-155-t40-uri/README.md) for exact source epochs, retained failures and scope.

Stage146–151 adds lossless scalar diagnostics, native file Glob support, selected authored settings/path behavior and bounded static import analysis. The workspace passes 2,172 tests with zero failures and six ignored; 13 nested child passes are excluded. Fresh original31 comparisons record 20 matches, six mismatches, three unsupported cases and two oracle errors. The separate NFC boundary records 13 UTF-8/UTF-16 path-value matches and five nonpasses. Six prechecks passed. One bounded M1 capture matched 6,078 results per Native/Node/Chromium target and 92 scenarios each for Node/browser; strict and architecture predicates pass from the same raw capture. Full M2/API, release-platform and performance requirements remain open. See the [Stage146–151 report](reports/m2/stage146-151-host-analyze/README.md) for retained failures and scope.

Stage145 adds shared Protobuf rendering (`textproto`) and GC root indexing. The current workspace passes 2,104 tests with zero failures and six ignored; the aggregate excludes 13 nested child passes. The 266-slot Protobuf development contract matches 240 renderer and eight pre-renderer observations, while 18 reference nonpasses remain unmet. All 57 selected original CLI contracts match Java. Bounded M1 matches 6,078 results per Native, Node and Chromium target, with 92 Node and 92 browser scenarios. Full M2/API, release-platform and performance requirements remain open. See the [Stage145 report](reports/m2/stage145-protobuf-gc/README.md) for raw evidence, retained failures and scope limits.

Stage144 adds shared Pkl binary rendering, bounded YAML parsing and authored shell escaping. The pre-lint focused run passed 38 native tests against 320 retained Java observations; 21 original CLI contracts separately matched their full public channels. After seven lint corrections, all five prechecks passed and the workspace aggregate records 1,947 passed, zero failed and five ignored across 323 groups, excluding 13 nested child passes. Fresh bounded M1 matched 6,078 results per Native, Node and Chromium target, with 92 Node and 92 browser scenarios; strict raw comparison and the architecture predicate pass from the same capture. Full inventory replay, M2/API, release-platform and performance requirements remain open. See the [Stage144 report](reports/m2/stage144-binary-yaml-shell/README.md) for failures, evidence scopes and aggregate limitations.

The Stage141 Jsonnet and XML checkpoint adds shared renderer support and qualifies 21 focused native tests against 172 Java observations, including four retained setup-error controls. Twenty-five original CLI cases matched their complete public channels. The full workspace aggregate records 1,909 passed, zero failed and five ignored across 313 harnesses; 13 nested child passes are excluded. Bounded M1 matched 6,078 results per Native, Node and Chromium target, with 92 Node and 92 browser scenarios. Full M2/API, release-platform and performance requirements remain open. See the [Stage141 report](reports/m2/stage141-jsonnet-xml/README.md) for the evidence and limits.

The Stage139 JSON Parser checkpoint matches 108 Java observations and five registered original CLI cases. Its focused native run passed 47 tests; the full workspace passed 1,886 tests with five ignored across 305 harnesses, excluding 13 nested child passes from the independent aggregate. A bounded M1 run matched 6,078 results per Native, Node and Chromium target. Full M2/API, release-platform and performance requirements remain open. See the [Stage139 report](reports/m2/stage139-json-parser/README.md) for the evidence and limits.


Stage137 extends module diagnostics and authored semver. The shared runtime adds standard module-key profiles and loads the authored `pkl:semver` module; `ModuleKeyKind` has three new public variants, so downstream exhaustive matches must be updated. Focused checks match 48 Java observations and six original CLI cases, and a bounded M1 run records 6,078 matches per target. The full workspace passed 1,872 tests with five ignored across 302 harnesses; the independent aggregate readback excludes 13 nested child passes. Full M2/API, release-platform and performance requirements remain open. See the [Stage137 report](reports/m2/stage137-module-diagnostics-semver/README.md) for raw evidence and remaining limits.


The [Stage136 report](reports/m2/stage136-language-replay/README.md) records a fresh local full-ready replay of 4,483 cases (candidate-unsupported: 74, mismatch: 12, oracle-error: 2, pass: 4,395). The integrity verifier exited 0; strict full verification exited 1. Twelve lexical requirements were promoted after 49 scalar comparisons and a separate RECORD module comparison. Complete M2/API, release-platform and performance requirements remain open.


This project reimplements [Pkl](https://pkl-lang.org/) 0.32.0 in Rust. It aims to share one engine across native Rust, WebAssembly, and Node.js N-API, with automated compatibility checks against the Java implementation.

The earlier [Stage134 report](reports/m2/stage134-type-source-scopes/README.md) records source-aware type diagnostics,
object-entry naming, and repaired inferred-parent and diagnostic equality regressions.
The full local workspace passes with **1,848 passed, zero failed and 5
ignored across 298 harnesses**. Full M2/API, release-platform and performance
requirements remain open.

The preceding [Stage131 report](reports/m2/stage131-power-boundaries/README.md)
records corrected power-assertion visibility: 34 Java observations and the
original `typeConstraints11.pkl` CLI diagnostic now match. The complete local
workspace exits zero with **1,790 passed, zero failed and 5 ignored across
288 harnesses**. Native, Node and Chromium each match 6,078 Java results;
Node and Chromium each pass 92 platform scenarios. Full M2/API, release-platform
and performance requirements remain open.

The preceding [Stage130 report](reports/m2/stage130-java-exception-renderer/README.md)
adds a shared Java exception renderer checked against 54 graphs in LF and CRLF.
The full local workspace records **1,789 passed, zero failed and five ignored
across 287 harnesses**. Native, Node and Chromium each match 6,078 Java results;
Node and Chromium each pass 92 platform scenarios. Runtime stack projection and
internal-error catch integration remain unfinished, as do full M2/API and
performance requirements.

The preceding [Stage129 report](reports/m2/stage129-workspace-validation/README.md)
records a complete local workspace command with exit **0**: **1,786 passed,
zero failed and five ignored across 286 harnesses**. All test targets and both
doctest crates complete; 26 examples are compile-only. Java URI-catch evidence
adds 32 query observations, including different repeated uncaught-error bytes.
The engine is unchanged, and M2/API, artifact-provenance and performance gaps
remain open.

The preceding [Stage128 report](reports/m2/stage128-workspace-storage/README.md)
adds an opt-in macOS executable-compression wrapper. Its 18 focused tests and
compiler/self-spawn checks pass. The workspace retry completes 200 harnesses
with **1,265 passes, zero failures and three ignored**, then stops at its disk
floor during reflection. Another 83 executable targets and both doctest crates
remain unstarted; this is not a full-workspace pass.

The preceding [Stage127 report](reports/m2/stage127-opaque-glob-uri/README.md)
records opaque glob URI validation before reader enumeration. All 20 selected
wrong-success sessions now stop with explicit unsupported diagnostics; Java's
internal-error bytes remain unmatched. Eight Rust tests and the bounded M1
native/Node/browser acquisition pass. The full workspace command stopped during
compilation at its disk-space guard; that acquisition remains incomplete.

The preceding [Stage122 report](reports/m2/stage122-lossless-source-runtime/README.md)
records lossless UTF-16 source/URI handling, typed glob Host operations, selected
diagnostics, Resource digests and math bindings. Local verification joins 285
complete test harnesses with **1,779 passes, zero failures and five ignored**;
the original reflection Cargo exit remains unobserved, so this is not a fresh
full-workspace command pass. Seven final development commands pass. The complete
ready replay records **4,351 matches out of 4,442 cases**, with 15 mismatches,
74 unsupported cases and two Java oracle errors. All 88 metamorphic checks pass
and no previous passing case regresses. Selected portable comparisons and the three-target raw transport check are
verified, and their evidence archives are published and checked. Full M2/API, release-platform and performance
requirements remain open.

The project has completed its **M0 compatibility tooling and initial Java baselines** and **M1 architecture experiments**, including failed-lazy-retry, nested Host evaluation, cancellation, and the Graal compilation investigation. A bounded parser/resolver, lazy bytecode VM, tracing heap, numeric/regex probes, and resumable Host run through native Rust, Node N-API, and browser Wasm. These are prototypes; the complete language, CLI, REPL, server, and product APIs remain unimplemented. The [M0 report](reports/m0/README.md) and [M1 experiments](reports/m1/README.md) distinguish verified results, known differences, and performance limitations.

**M2 language implementation is in progress.** The shared engine is being extended
with operators, control flow, strings, typed methods, recursive calls, closures,
classes, inheritance, generic type aliases, resumable type constraints, pipelines,
ordered collection members, `for`/`when` generators, spread, member predicates,
generic collection checks and lazy wrappers, structural equality and object keys,
typed default properties and cache-sensitive comparison order, deferred
References through `pkl:ref`, evaluator-owned class and type-alias identity,
function type metadata and arity checks, ordered type resolution and generic
parameter scopes, selected recursive constraints and erased `Class<T>` arguments,
shared constraint receiver slots and creation-time lambda captures,
finite recursive generics with shared captured type arguments, nullable defaults,
selected direct generic constructors and their diagnostic origins, declaration
annotations and ordered initialization, resumable object forcing and string conversion,
Host callback diagnostic frames, strict Map
and Set construction, lookup, typing and structural comparison, persistent type-use
resolution, let/for const scopes, and structured source diagnostics with recursive frame compression with optional power assertions
and standard-library frames. The [latest checkpoint](reports/m2/README.md)
records the accepted local verification scope, known failures, and unexecuted
environments.

The [Stage109 checkpoint](reports/m2/stage109-typed-module-annotation-checkpoint.json) adds `Typed.getProperty`, `Module.relativePathTo` and annotation/const diagnostic coverage to the shared engine. Targeted validation records **152 passed tests and one ignored required placeholder**. Independent current-source native readbacks match **276 ordinary new observations** and **1,138 existing Typed/reflection observations**. The new portable matrix matches **276 ordinary observations per Native, N-API and Chromium target**, retaining two required Java internal observations unresolved on each target.

For that historical Stage109 checkpoint, the complete ready replay records **4,332 passes out of 4,442 cases**, with 26 mismatches, 82 unsupported cases, two oracle errors, no historical-pass regression and all 88 metamorphic checks passing. One bounded M1 acquisition separately matches 6,078 checks per target. Eight freshness commands pass; exact current strict-gate accounting still reports 1,780 overlapping unmet conditions. Formatting, xtask verification, all-target Clippy with warnings denied, and the Wasm check pass on the recorded current source. The complete workspace passes **1,658 tests with five ignored**, with test-target, example and doctest coverage verified. Full M2/API, release-platform and performance completion are not claimed.

The [Stage102 collection checkpoint](reports/m2/stage102-collection-apis-checkpoint.json)
is retained as historical evidence: 3,718 ordinary observations match Java in its
shared Rust and selected target captures, and 1,502 workspace tests pass with four
ignored. Its original Map failure predates the Typed implementation; its required
internal-error contracts and original whole-List Java heap failure remain explicit.

The [Stage66 checkpoint](reports/m2/stage66-implementation-checkpoint.json)
adds String/List/Set reversal, constructor resolution inside constraints, and
union-marker diagnostics. Its 1,081 selected expressions and forty separate
diagnostic modules match Java on native, N-API, and Chromium. The 131-program
original selection has 81 matches; failures, unsupported cases, and two blocked
originals remain explicit. These selected results do not close M2.

The [M2 verification contract](docs/m2-language.md)
keeps the complete language inventory separate from selected executable probes.
The [original module frontend adapter](docs/m2-module-frontend.md) also compares
registered upstream commands with their original inputs, arguments, resources,
and complete process output. The output operation evaluates actual `ModuleOutput`
properties and PCF through the shared VM; the separate frontend-only operation
still stops after preparation. Imports now use typed resolve/source Host requests
and share the evaluator's VM, heap, module cache and declaration identities.
Expression sessions use the common parser and compiler with the actual module
receiver. Module-owned class/alias types, `extends`, and `amends` share that
engine. [Canonical diagnostics](docs/m2-diagnostics.md) preserve UTF-16 strings
through native, Node, and Wasm APIs, including typed Host IOException messages
with distinct absent and empty causes. Java UTF-8 serialization is checked
separately. The exact `output.text` expression request shares the ModuleOutput
operation. The [Stage 46 selected regression](reports/m2/stage46-final-summary.json)
records 3,650 matching native cases out of 3,664 attempts at implementation
`a3eb363`, retaining four unsupported cases, nine mismatches and one oracle error.
All 2,782 persistent evaluator calls across 334 sequence executions match their
declared channels. Three previously unsupported original programs now pass, with
no previously passing case regressing. The strict full-language gate still fails.
The earlier [Stage 45](reports/m2/stage45-final-summary.json) and
[Stage 43/44 records](reports/m2/stage44-final-summary.json) remain unchanged.
The [standard-module checkpoint](docs/m2-stdlib-bootstrap.md) adds supported
source-backed `pkl:base`/`pkl:test` imports, guest exception handlers, ModuleInfo
validation, and typed security-denial completions. Broader bootstrap comparisons
still fail on missing bindings, resource-read counts, and standard-module caches.
Complete modifier and type semantics, remaining collection and Reference rules,
the remaining inheritance and imported-type rules, reflection metadata, and the remaining language corpus are
required before M2 can close.

The [shared JSON renderer](docs/m2-json-rendering.md) adds render directives,
property annotation transforms, collection joining and selected boxed-value
identity rules to the common engine. Its Java fixtures and bounded comparisons
remain separate from the Stage 46 whole-selection result above. Standard-module
admission policies, the JSON parser and complete renderer coverage remain open.

The [DataSize checkpoint](docs/m2-data-size.md) adds shared unit conversion,
value operations, type checks, rendering and related Bytes construction/size
behavior. Its selected string comparisons match Java on native, N-API and
Chromium; the unchanged basic/API DataSize programs also match. The expanded
Stage 57 original selection still had eight mismatches involving Bytes, Duration,
`Any.ifNonNull` and the Pkl Binary test path. Typed exports and complete standard
library coverage remain open; see the [recorded evidence](reports/m2/README.md).

The [Any.ifNonNull checkpoint](docs/m2-any-if-non-null.md) adds shared VM
method lookup and callback behavior, including null defaults and Host lifecycle
checks. Its Stage 58 Java fixtures and selected native comparison retained
four mismatched Regex and IntSeq constructor executions across two repeats.
The Stage 58 N-API and Chromium string comparisons matched 168 of 172 executions
each; their integrity gates passed and strict string gates failed on those rows.
The unchanged `api/any.pkl` original program passes raw comparison. At Stage 58,
ten of 17 original programs passed and seven mismatched. The DataSize native,
N-API, and Chromium string regressions each retain 510 of 510 matches.
Typed exports, full M2, and performance requirements remain open.

The [Duration checkpoint](reports/m2/stage59-implementation-checkpoint.json)
extends the shared engine with unit conversion, arithmetic, comparison,
`isoString`, and method/type diagnostics. Native, N-API, and Chromium each
match all 548 recorded Java string observations and complete session protocols.
The unchanged original selection now has 11 matches, five mismatches, and one
unsupported result: `basic/duration.pkl` newly passes, while `api/duration.pkl`
reaches the missing `pkl:math` module. Bytes and Binary test dependencies remain.
The separate 15-value raw-bit regression checks internal Rust values; typed
product exports, whole-language compatibility, and performance remain open.

The [Bytes checkpoint](reports/m2/stage60-implementation-checkpoint.json) adds
shared hashing, encoding, strict charset decoding, indexing, concatenation,
`toList`, and resumable `List.toBytes` checks. At Stage 60, each of native, N-API,
and Chromium matched 594 of 600 string observations; three IntSeq-dependent cases
failed in both repetitions. The 17 original programs had 12 matches, four
mismatches, and one unsupported result. `invalidBytes4.pkl` newly matches;
the basic/API Bytes programs reach their missing `IntSeq` dependency.
Typed exports, including Java byte-array aliasing, remain unverified.

The [IntSeq checkpoint](reports/m2/stage61-implementation-checkpoint.json) adds
shared sequence operations, streaming generators/spreads, all 17 `pkl:math`
constants, and the List accessors, `add`, and `fold` required by the original
programs. Native, N-API, and Chromium each match all 474 selected IntSeq string
observations. The Bytes regression now matches 600 of 600 per target; Duration
and DataSize retain 548 and 510 matches. Portable Any retains two Regex
mismatches per target. Among 62 unchanged original programs, 26 match, 15
mismatch, and 21 are unsupported. The previous 17-program cohort improves
from 12 to 15 matches, with no passing case regressing. These are selected
results; all nine M2 workstreams still have unmet completion criteria.

The [collection conversion checkpoint](reports/m2/stage62-implementation-checkpoint.json)
adds shared strict/lazy object conversions, optional and defaulted lookups,
Dynamic length, Set addition, and Bytes iteration/spread. Six independent Java
fixtures contain 485 expressions; native, N-API, and Chromium each match all
970 observations across two repetitions, including full UTF-16 and error bytes.
The implementation also separates authored NUL property names from synthetic
surrogate names. Existing scalar regressions remain unchanged, including the
known Regex failures. The expanded 90-program original selection has 35 matches,
22 mismatches, 32 unsupported results, and one Java heap error. Within the prior
62 programs, matches increase from 26 to 30 without a passing case regressing.
These results do not close full M2, typed product APIs, or performance targets.

## Goals

- Compatibility with the Pkl 0.32.0 language, standard library, CLI, REPL, server, and public APIs.
- Identical output bytes, generated files, error messages, source locations, and exit codes for the same inputs and environment.
- Execution time and peak RSS no greater than the Java baseline for each required workload.
- Native, Wasm, and N-API support through a single Rust evaluation engine.
- Coverage and implementation tracking through upstream, Pantry, regression, differential, and performance tests.

The [project intent](INTENT.md) defines goals and scope. The [execution plan (Korean)](PLAN.md) contains the phase overview, work items, dependencies, and phase gates. See the [architecture](docs/architecture.md) for design guidance and [TESTING.md](TESTING.md) for acceptance criteria and verification methods.

## Reference versions

Reference sources are pinned as Git submodules under `external/`. The Git gitlinks and `references.lock` must identify the same commits.

| Path | Purpose | Pinned commit |
| --- | --- | --- |
| `external/pkl` | Pkl 0.32.0 language, API, standard library, and test reference | `e51fb2f91774265666f94980ec630db794a4db2d` |
| `external/pkl-pantry` | Candidate compatibility corpus of real packages and examples | `904a68184e54da378b0a0e8a911a3fab3956f693` |

The pinned Pantry revision defines the corpus inputs. Java baseline acquisition has exercised its 29 package/API commands and 206 generated package modules; configuration-dependent errors and unfixed network inputs remain explicitly recorded. The full command corpus still has blocked requirements and a Java heap failure. See the [M0 evidence report](reports/m0/README.md) for counts and limitations. `../pkl-rs.rev1` is earlier work used as a design and test reference, not a build dependency.

## Getting started

Install Git and [rustup](https://rustup.rs/). The Rust toolchain is pinned in `rust-toolchain.toml`; rustup installs the specified tools. Replace the URL below with the repository URL.

```sh
git -c core.autocrlf=false clone --recurse-submodules <repository-url> pkl-rs
cd pkl-rs
git config --local core.autocrlf false
git submodule foreach --recursive 'git config --local core.autocrlf false'
```

For an existing clone, initialize the reference sources:

```sh
git -c core.autocrlf=false submodule update --init --recursive
git config --local core.autocrlf false
git submodule foreach --recursive 'git config --local core.autocrlf false'
```

Setting `core.autocrlf=false` prevents automatic line ending conversion of reference fixtures on Windows. Explicit upstream `.gitattributes` rules still apply. If an existing checkout has already been converted, preserve any local changes and create a fresh clone using the commands above.

Run the setup checks:

```sh
cargo xtask verify
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo check -p pkl-runtime --target wasm32-unknown-unknown --locked
```

`cargo xtask verify` checks the reference source pins. The [Java oracle](docs/oracle.md) provides pinned Pkl 0.32.0 execution and exact replay checks. The [comparison tool](docs/compatibility-tooling.md) captures raw output and generated files and rejects missing or failing cases. See [TESTING.md](TESTING.md) for tooling commands and [the platform experiment](docs/m1-platforms.md) for actual Node/browser execution.

## Current structure

```text
.cargo/                Cargo aliases and shared configuration
.github/workflows/     Workspace checks and Java tooling replay
benchmarks/            Initial Java workloads and sampling contract
compatibility/         Source, corpus, smoke, and regression manifests
crates/pkl-runtime/    Shared frontend, VM, heap, diagnostics, primitives, and Host
crates/pkl-wasm/       Experimental browser bindings to the shared engine
crates/pkl-napi/       Experimental Node bindings to the shared engine
docs/                  Architecture and verification tool contracts
external/pkl/          Pkl 0.32.0 reference submodule
external/pkl-pantry/   Pantry reference submodule
reports/m0/            Baseline summaries and checksummed raw evidence
reports/m1/            Architecture experiment decisions and raw evidence
reports/m2/            Incremental language results and raw evidence
tests/fixtures/        Owned smoke inputs and selected historical regressions
tools/                 Python verification and Java measurement helpers
xtask/                 Repository verification tool
INTENT.md              Project goals and scope
PLAN.md                Roadmap, execution plan, and work status (Korean)
references.lock        Reference repositories and revisions
oracle.lock.json       Java artifact, runtime, and execution profiles
rust-toolchain.toml    Rust toolchain version
```

See [CONTRIBUTING.md](CONTRIBUTING.md) for the contribution workflow and [AGENTS.md](AGENTS.md) for instructions for agents working in this repository. This project uses the [Apache License 2.0](LICENSE). Reference repository licenses are included in their respective submodules.
