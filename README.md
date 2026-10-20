# pkl-rs

This project reimplements [Pkl](https://pkl-lang.org/) 0.32.0 in Rust. It aims to share one engine across native Rust, WebAssembly, and Node.js N-API, with automated compatibility checks against the Java implementation.

The project has completed its **M0 compatibility tooling and initial Java baselines** and **M1 architecture experiments**, including failed-lazy-retry, nested Host evaluation, cancellation, and the Graal compilation investigation. A bounded parser/resolver, lazy bytecode VM, tracing heap, numeric/regex probes, and resumable Host run through native Rust, Node N-API, and browser Wasm. These are prototypes; the complete language, CLI, REPL, server, and product APIs remain unimplemented. The [M0 report](reports/m0/README.md) and [M1 experiments](reports/m1/README.md) distinguish verified results, known differences, and performance limitations.

**M2 language implementation is in progress.** The shared engine is being extended
with operators, control flow, strings, typed methods, recursive calls, closures,
classes, inheritance, generic type aliases, resumable type constraints, pipelines,
ordered collection members, `for`/`when` generators, spread, member predicates,
generic collection checks and lazy wrappers, structural equality and object keys,
and structured source diagnostics with optional power assertions.
The [M2 verification contract](docs/m2-language.md)
keeps the complete language inventory separate from selected executable probes.
Complete modifier and type semantics, remaining collection rules,
`Reference`, and the remaining language corpus are required before M2 can close.

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
