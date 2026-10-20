# pkl-rs

This project reimplements [Pkl](https://pkl-lang.org/) 0.32.0 in Rust. It aims to share one engine across native Rust, WebAssembly, and Node.js N-API, with automated compatibility checks against the Java implementation.

The project is currently at the **repository and development environment setup stage**. Pkl parsing and evaluation, the CLI, REPL, server, and Wasm and N-API products are not implemented yet. Passing the current builds and tests does not establish Pkl compatibility or achievement of the performance targets.

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

The pinned Pantry revision defines the corpus inputs. The full corpus has not yet been validated by running it with Java 0.32.0 or Rust. `../pkl-rs.rev1` is earlier work used as a design and test reference, not a build dependency.

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

`cargo xtask verify` checks the reference source pins. The [Java oracle](docs/oracle.md) provides pinned Pkl 0.32.0 execution and exact replay checks. Rust evaluation is not implemented yet.

## Current structure

```text
.cargo/                Cargo aliases and shared configuration
.github/workflows/     CI for setup checks
crates/pkl-runtime/    Minimal Rust library for the shared engine
docs/                  Architecture documentation
external/pkl/          Pkl 0.32.0 reference submodule
external/pkl-pantry/   Pantry reference submodule
xtask/                 Repository verification tool
INTENT.md              Project goals and scope
PLAN.md                Roadmap, execution plan, and work status (Korean)
references.lock        Reference repositories and revisions
rust-toolchain.toml    Rust toolchain version
```

See [CONTRIBUTING.md](CONTRIBUTING.md) for the contribution workflow and [AGENTS.md](AGENTS.md) for instructions for agents working in this repository. This project uses the [Apache License 2.0](LICENSE). Reference repository licenses are included in their respective submodules.
