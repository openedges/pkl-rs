# pkl-rs

A pure-Rust reimplementation of **Apple Pkl 0.32.0** (`unsafe_code = "deny"`, zero production Java fallback) sharing a single evaluation engine, value model, diagnostic pipeline, and renderer suite across **native Rust** (`pkl`, `pkldoc`, `crates/pkl`), **WebAssembly** (`@pkl-rs/wasm`), and **Node.js N-API** (`@pkl-rs/napi`).

## Current status (`2026-11-23`, `release_ready: false`)

- **Official Apple Pkl `0.32.0` `native-image` performance parity (15 / 15 PASS on local macOS `arm64`)**: All 15 CLI, server, and `pkldoc` workloads (`cold-cli` 14 + `steady-server` 1) achieve **`<= 1.00x` execution time (`0.055x–0.935x`) and `<= 1.00x` `os.wait4` peak RSS (`0.136x–0.662x`)** against SHA-256 pinned `pkl-macos-aarch64` (`94.7 MiB`) and `pkldoc-macos-aarch64` (`94.7 MiB`) with 100% raw-byte parity (`pkl` `15 MiB`, `pkldoc` `14 MiB`; `6/6` Core workloads also pass vs archived M0 GraalVM CE `25.0.1`).
- **JS / Wasm & Browser contract (`2026-11-22`)**: `@pkl-rs/wasm` (`[profile.wasm-release]`) builds `pkl_wasm_bg.wasm` at **`8,022,629 B` raw / `2,619,729 B` (`2.498 MiB`) gzip-9 `<= 2.50 MiB`**, passing `10/10` package tests (including `npm pack` isolated install); `@pkl-rs/napi` passes `11/11` native tests. Browser Wasm targets follow the explicit `2026-11-22` contract in [TESTING.md](TESTING.md) (Chromium/WebKit `<= 1.00x`, Firefox `eval-text <= 1.00x` and `typed-conversion <= 1.50x`, Wasm linear memory `<= 0.15x` Java peak RSS, JS heap `<= 64 MiB`, `.wasm` gzip-9 `<= 2.50 MiB`).
- **CI, Release Packaging & Local Verification split (`2026-11-23`)**:
  - [.github/workflows/ci.yml](.github/workflows/ci.yml) runs minimal, low-cost Ubuntu checks and packaging smoke (`check-and-test`, `package-smoke`).
  - [.github/workflows/release.yml](.github/workflows/release.yml) builds and publishes cross-platform CLI archives (Linux glibc `<= 2.28` + static `musl` x64/arm64, macOS arm64/x64, Windows x64), `@pkl-rs/wasm`, 7-platform `@pkl-rs/napi`, and `SHA256SUMS.txt` **only** on `v*` tags or manual dispatch.
  - [tools/local_verify.py](tools/local_verify.py) runs all full workspace, Python (`59` active + `75` historical), platform Node.js (`41` files / `384` tests), M0–M6 Java `0.32.0` differential, Playwright cross-browser, and M7 fuzz/soak/benchmark suites locally or in a dedicated verification host (see [docs/ci.md](docs/ci.md)).
- **Honest release boundary (`release_ready: false`)**: Latest verification was performed on **local macOS `arm64`** (not hosted CI). Non-macOS OS/architecture release targets (`darwin-x64`, `linux-x64-gnu`, `linux-arm64-gnu`, `linux-x64-musl`, `linux-arm64-musl`, `win32-x64-msvc`), post-optimization Firefox/WebKit browser runs, `--baseline both` 34-workload unified release verification, and hosted CI execution remain unverified.

## Milestone checkpoints

Historical checkpoint reports retain their original source epochs, sealed raw evidence, and preserved exceptions without overwriting earlier runs:

| Milestone | Scope | Canonical Report | Key Recorded Outcome (Local macOS `arm64`) |
| --- | --- | --- | --- |
| **M0** | Pinned `0.32.0` source/corpus/oracle tooling & GraalVM CE `25.0.1` baseline | [reports/m0/README.md](reports/m0/README.md) | 11,624 inventory items, 1,112 command cases, 6-workload M0 GraalVM baseline |
| **M1** | Shared Rust frontend/VM/heap/Host across native, Node, and browser Wasm | [reports/m1/README.md](reports/m1/README.md) | 6,078/6,078 cross-target parity, 92/92 Node & Chromium scenarios, Graal trace audit |
| **M2** | Whole-language semantics (475 requirements, 4,575 ready cases) | [reports/m2/stage161-m2-closure/README.md](reports/m2/stage161-m2-closure/README.md) | 4,574 pass / 1 `oracle-error` (`error-cycle`) / 0 mismatch; Protobuf 240+8 (18 upstream `REFERENCE-NONPASS`) |
| **M3** | All 23 `pkl:*` stdlib modules, 542 builtins, 9 renderers, `output.files`, `PklBinary` | [reports/m3/README.md](reports/m3/README.md) | 1,781/1,781 mechanical coverage (`failure_count: 0`), 118/118 3-lane differential |
| **M4** | Host I/O, HTTP/HTTPS/CA/proxy, `0x26–0x32` external readers, `PklProject`, Pantry | [reports/m4/README.md](reports/m4/README.md) | 429/429 5-lane offline differential (all 29 Pantry packages in dev + packaged form) |
| **M5** | Public Rust API (`crates/pkl`), `pkl` CLI, REPL, server, formatter, `pkldoc` | [reports/m5/README.md](reports/m5/README.md) | 142/142 5-lane differential (`1,767/1,767` `pkldoc` files + `13/13` symlinks byte-identical) |
| **M6** | Product `@pkl-rs/wasm` & `@pkl-rs/napi` bindings and clean packaging | [reports/m6/README.md](reports/m6/README.md) | 80/80 executable PASS + 6/6 explicit `capability-blocked`, 4-way raw-byte parity |
| **M7** | Performance/peak RSS parity, memory soak, differential fuzzing, release matrix | [reports/m7/README.md](reports/m7/README.md) | 18/18 soak `bounded`, 215/215 fuzz PASS, 15/15 Official `native-image` `<= 1.00x` (`2026-11-23`), `release_ready: false` |

## Goals

- Compatibility with the Apple Pkl `0.32.0` language, standard library, CLI, REPL, server, formatter, `pkldoc`, and public Rust/Wasm/N-API surfaces (Java/Kotlin codegen and JVM public API adapters excluded by `2026-11-20` scope decision in [INTENT.md](INTENT.md)).
- Identical output bytes, generated files, error messages, source locations, and exit codes for the same inputs and environment.
- Execution time and peak RSS `<= 1.00x` against pinned Official Apple Pkl `0.32.0` `native-image` for all 15 CLI/server/`pkldoc` workloads, and against pinned Temurin 21 JVM (+ the `2026-11-22` browser Wasm contract) for all 19 embedded API and JS/Wasm workloads.
- Single shared Rust evaluation engine across native, Wasm, and N-API targets with platform-specific I/O isolated behind the Host interface.

See [INTENT.md](INTENT.md) for goals and scope, [PLAN.md](PLAN.md) for the roadmap and detailed status (Korean), [docs/architecture.md](docs/architecture.md) for design guidance, [TESTING.md](TESTING.md) for the complete verification contract, and [docs/ci.md](docs/ci.md) for CI/release/local verification workflows.

## Reference versions

Reference sources are pinned as Git submodules under `external/`. Git gitlinks and [references.lock](references.lock) must match the exact revisions below:

| Path | Purpose | Pinned commit |
| --- | --- | --- |
| `external/pkl` | Apple Pkl `0.32.0` language, API, standard library, and test reference | `e51fb2f91774265666f94980ec630db794a4db2d` |
| `external/pkl-pantry` | Candidate compatibility corpus of real packages and examples | `904a68184e54da378b0a0e8a911a3fab3956f693` |

## Getting started

Install Git and [rustup](https://rustup.rs/). The Rust toolchain is pinned in [rust-toolchain.toml](rust-toolchain.toml) (`1.98.1`).

```sh
git -c core.autocrlf=false clone --recurse-submodules <repository-url> pkl-rs
cd pkl-rs
git config --local core.autocrlf false
git submodule foreach --recursive 'git config --local core.autocrlf false'
```

For an existing clone, initialize submodules without CRLF rewriting:

```sh
git -c core.autocrlf=false submodule update --init --recursive
git config --local core.autocrlf false
git submodule foreach --recursive 'git config --local core.autocrlf false'
```

### Default verification commands

```sh
# 1) Standard Rust checks
cargo xtask verify
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo check -p pkl-runtime -p pkl-formatter -p pkl-wasm --target wasm32-unknown-unknown --locked

# 2) Unified local verification runner (covers every check outside minimal GitHub CI)
python3 tools/local_verify.py --list
python3 tools/local_verify.py --preset quick
python3 tools/local_verify.py --preset full-strict --local-checkpoint
```

See [docs/oracle.md](docs/oracle.md) for bootstrapping the pinned Temurin 21 JVM and Official Apple Pkl `0.32.0` `native-image` oracles, [docs/ci.md](docs/ci.md) for all 18 local verification suites and GitHub Actions workflows, and [TESTING.md](TESTING.md) for full acceptance criteria.

## Repository structure

```text
.cargo/                  Cargo aliases and shared configuration
.github/workflows/       Minimal Ubuntu CI (`ci.yml`) and tag/manual cross-platform release packaging (`release.yml`)
benchmarks/              M0 Java workloads + M7 34-workload benchmark contract (`m7_workloads.json`, `m7_fixtures/`)
compatibility/           Source, corpus, smoke, regression, M2-M6 coverage, and M7 release manifests
crates/pkl/              Public native Rust Evaluator API (plus `examples/m7_{bench,memory_soak}_probe.rs`)
crates/pkl-cli/          Unified product `pkl` CLI binary
crates/pkl-doc/          `pkldoc` documentation site generator, migrator, and vendored web assets
crates/pkl-formatter/    Pure-Wasm CST Pkl 0.32.0 source formatter
crates/pkl-host-native/  Native Host I/O, TLS/HTTP, CA roots, modulepath, package cache, and extensions
crates/pkl-napi/         Product Node.js N-API bindings (preserving M1 experimental probes)
crates/pkl-repl/         Interactive PTY and pipe REPL driver
crates/pkl-runtime/      Shared pure-Wasm frontend, VM, heap, stdlib, renderers, Host, REPL, and server core
crates/pkl-server/       MessagePack evaluator server (`pkl server`)
crates/pkl-wasm/         Product WebAssembly bindings (preserving M1 experimental probes)
docs/                    Architecture, oracle, CI/release, and verification tool contracts
external/pkl/            Pkl 0.32.0 reference submodule
external/pkl-pantry/     Pantry reference submodule
packages/pkl-napi/       Distributable `@pkl-rs/napi` dual ESM/CJS Node.js package and TypeScript types
packages/pkl-wasm/       Distributable `@pkl-rs/wasm` browser/Node ESM Wasm package, VFS, and TypeScript types
reports/m0/..m7/         Canonical milestone checkpoint summaries and checksummed raw evidence
tests/fixtures/          Owned smoke, regression, M2-M6 differential, and M7 soak/fuzz fixtures
tools/                   Verification harnesses (`local_verify.py`, `oracle.py`, `m1..m7_*.py`, `tests/` [134], `platform/` [41 tests])
vendor/java_regex/       Patched pure-Rust JDK-compatible regex engine (`CANON_EQ` / `(?c)`)
xtask/                   Repository pin verification tool
INTENT.md                Project goals and scope
PLAN.md                  Roadmap, execution plan, and work status (Korean)
TESTING.md               Complete validation contract and test inventory
references.lock          Reference repositories and revisions
oracle.lock.json         Pinned JVM, GraalVM CE, and Official Apple Pkl 0.32.0 native-image profiles
rust-toolchain.toml      Pinned Rust toolchain version (`1.98.1`)
```

See [CONTRIBUTING.md](CONTRIBUTING.md) for the contribution workflow and [AGENTS.md](AGENTS.md) for repository rules. Licensed under the [Apache License 2.0](LICENSE). Reference repository licenses are included in their respective submodules.
