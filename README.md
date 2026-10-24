# pkl-rs

This project reimplements [Pkl](https://pkl-lang.org/) 0.32.0 in Rust. It aims to share one engine across native Rust, WebAssembly, and Node.js N-API, with automated compatibility checks against the Java implementation.

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
checks. Its independent Java fixtures and selected native comparison retain
four mismatched Regex and IntSeq constructor executions across two repeats.
The N-API and Chromium string comparisons also match 168 of 172 executions
each; their integrity gates pass and strict string gates fail on those rows.
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
