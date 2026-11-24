# pkl-rs

A pure-Rust implementation of **[Apple Pkl 0.32.0](https://pkl-lang.org/)** — zero Java runtime required, `unsafe_code = "deny"` across the entire workspace, and a single shared evaluation engine powering native CLI tools, a Rust library, WebAssembly, and Node.js N-API bindings.

> [!NOTE]
> **Pre-release status (`0.32.0` preparation)**: Language, standard library, CLI, `pkldoc`, Rust API, `@pkl-rs/wasm`, `@pkl-rs/napi`, and Official Apple Pkl `0.32.0` `native-image` performance parity (`15/15 <= 1.00x` time & peak RSS) have been verified on local macOS `arm64`. Cross-platform hosted CI release verification (`linux-*`, `darwin-x64`, `win32-x64`) and final GA sign-off remain in progress (`release_ready: false`). See [CHANGELOG.md](CHANGELOG.md) and [docs/development-history.md](docs/development-history.md) for exact verification scope.

## Products & Capabilities

- **`pkl` CLI & `pkldoc`** (`crates/pkl-cli`, `crates/pkl-doc`): Drop-in native binaries (~14–15 MiB) supporting `eval`, `repl`, `server`, `test`, `project resolve`, `project package`, `download-package`, `analyze imports`, `format`, `run`, `shell-completion`, and HTML documentation generation (`pkldoc`).
- **Rust Evaluator API** (`crates/pkl`): Ergonomic embedding API (`Evaluator`, `EvaluatorBuilder`, `ModuleSource`, `PklValue`/`FromPklValue`, `ModuleSchema`, `Project`, `Analyzer`, `SecurityManager`, `HttpClient`, custom `ModuleKeyFactory`/`ResourceReader`, cancellation, and timeouts).
- **Node.js Native Addon** (`@pkl-rs/napi`): Dual ESM/CJS bindings with TypeScript types, non-blocking libuv worker threads (`Promise`) plus `*Sync` APIs, full native Host I/O (filesystem, HTTP/HTTPS, `PklProject`, `package:`, external readers), and custom JS extensions across 7 target platforms.
- **WebAssembly Package** (`@pkl-rs/wasm`): Browser and Node.js ESM package (`<= 2.50 MiB` gzip-9 `.wasm`), pure-Wasm `VirtualFileSystem`, cooperative async Host callbacks, `createRepl()`, `analyzeImportGraph()`, `evaluateSchema()`, `evaluateTest()`, `format()`, and lossless `decodePklBinary()`.
- **Full Pkl `0.32.0` Core**: All 23 `pkl:*` standard library modules (542 builtins), all 9 renderers (`pcf`, `json`, `yaml`, `xml`, `properties`, `plist`, `jsonnet`, `textproto`, `pkl-binary`), `output.bytes`, `output.files`, `pkl:test`, and `pkl:Command`.

## Installation

For full platform matrices, checksum verification, shell completions, and build options, see [docs/installation.md](docs/installation.md).

### Prebuilt CLI Binaries (`pkl` & `pkldoc`)

Download the archive for your platform from [GitHub Releases](https://github.com/openedges/pkl-rs/releases), verify against `SHA256SUMS.txt`, and place `pkl` / `pkldoc` on your `PATH`:

```sh
# Example: macOS Apple Silicon (replace version/target for your platform)
curl -fSLO https://github.com/openedges/pkl-rs/releases/download/v0.32.0/pkl-darwin-arm64.tar.gz
curl -fSLO https://github.com/openedges/pkl-rs/releases/download/v0.32.0/SHA256SUMS.txt
shasum -a 256 --check --ignore-missing SHA256SUMS.txt
tar -xzf pkl-darwin-arm64.tar.gz
./pkl --version
```

Supported prebuilt CLI targets:
- **Linux (`x64`, `arm64`)**: portable glibc `>= 2.28` (`*-gnu.tar.gz`) and static musl (`*-musl.tar.gz`)
- **macOS (`arm64`, `x64`)**: `*-darwin-{arm64,x64}.tar.gz`
- **Windows (`x64`)**: `*-win32-x64.zip`

### Node.js (`@pkl-rs/napi`) & WebAssembly (`@pkl-rs/wasm`)

```sh
# Node.js native bindings (Node >= 18, full native Host I/O)
npm install @pkl-rs/napi

# Browser & Node.js WebAssembly bindings (Node >= 20.16, sandboxed VFS + async Host)
npm install @pkl-rs/wasm
```

*(Packages are published via [.github/workflows/release.yml](.github/workflows/release.yml) on release; until initial npm publication, build local tarballs from source as described in [docs/installation.md](docs/installation.md).)*

### Build from Source

Requires Git and [rustup](https://rustup.rs/) (toolchain pinned to `1.98.1` in [rust-toolchain.toml](rust-toolchain.toml)):

```sh
git -c core.autocrlf=false clone https://github.com/openedges/pkl-rs.git
cd pkl-rs
cargo build --release --locked -p pkl-cli --bin pkl -p pkl-doc --bin pkldoc
./target/release/pkl --version
```

*(Building `pkl` and `pkldoc` does not require initializing the upstream test submodules. For contributor verification and Java differential testing, initialize submodules with `git -c core.autocrlf=false submodule update --init --recursive`.)*

## Quick Start

### CLI

```sh
# Evaluate Pkl source to JSON
printf 'name = "pkl-rs"\nport = 8080\n' | pkl eval -f json -

# Format Pkl files and run Pkl tests
pkl format config.pkl
pkl test tests.pkl

# Generate HTML documentation
pkldoc -o docs-out/ docsite-info.pkl PklProject
```

### Rust API (`crates/pkl`)

```rust
use pkl::{Evaluator, ModuleSource};

fn main() -> pkl::Result<()> {
    let mut evaluator = Evaluator::preconfigured()?;
    let source = ModuleSource::text(r#"
        name = "pkl-rs"
        port = 8080
    "#);
    let output = evaluator.evaluate_output_text(&source)?;
    println!("{output}");
    evaluator.close();
    Ok(())
}
```

### Node.js (`@pkl-rs/napi`)

```js
import { createEvaluator, ModuleSource } from '@pkl-rs/napi';

const evaluator = await createEvaluator({ outputFormat: 'json' });
try {
  const json = await evaluator.evaluateOutputText(
    ModuleSource.text('name = "pkl-rs"\nport = 8080\n'),
  );
  console.log(json);
} finally {
  await evaluator.close();
}
```

### Browser & Node.js (`@pkl-rs/wasm`)

```js
import { init, createEvaluator, ModuleSource, VirtualFileSystem } from '@pkl-rs/wasm';

await init(); // Required once in browsers; optional in Node.js

const vfs = new VirtualFileSystem(
  { '/app/config.pkl': 'name = "pkl-wasm"\nport = 8080\n' },
  { rootDir: '/app' },
);
const evaluator = await createEvaluator({ vfs, outputFormat: 'json' });
try {
  console.log(await evaluator.evaluateOutputText(ModuleSource.path('/app/config.pkl')));
} finally {
  evaluator.close();
}
```

## Compatibility & Performance

- **Compatibility baseline**: Targets **Apple Pkl `0.32.0`** raw-byte output, diagnostics, source positions, generated files, and exit codes. Java/Kotlin code generation (`pkl-codegen-java`, `pkl-codegen-kotlin`) and JVM public API adapters are explicitly excluded from scope ([INTENT.md](INTENT.md)).
- **Performance (local macOS `arm64`)**: All 15 CLI, server, and `pkldoc` workloads run at **`0.073x–0.761x` execution time** and **`0.130x–0.551x` peak RSS** compared against Official Apple Pkl `0.32.0` `native-image` (`<= 1.00x` across every workload) with 100% raw-byte parity. `@pkl-rs/wasm` compiles to `2.03 MiB` (`2,124,981 B`) gzip-9 (`<= 2.50 MiB` budget).
- Full milestone metrics, differential test counts, soak/fuzz records, and raw evidence archives are preserved separately in [docs/development-history.md](docs/development-history.md).

## Testing & Verification

Standard Rust checks:

```sh
cargo xtask verify
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo check -p pkl-runtime -p pkl-formatter -p pkl-wasm --target wasm32-unknown-unknown --locked
```

Unified local verification runner (covering Rust workspace, manifests, Python unit tests, platform Node.js tests, standalone npm packages, M0–M6 Java `0.32.0` differential suites, and M7 fuzz/soak/benchmark suites):

```sh
python3 tools/local_verify.py --list
python3 tools/local_verify.py --preset quick
python3 tools/local_verify.py --preset full-strict --local-checkpoint
```

See [TESTING.md](TESTING.md) for the complete verification contract and [docs/ci.md](docs/ci.md) for the GitHub Actions vs. local verification split.

## Versioning

All distributable components (`Cargo.toml` workspace crates, `pkl`/`pkldoc` binaries, `@pkl-rs/wasm`, `@pkl-rs/napi`, and Git release tags) follow a unified `0.32.0` versioning policy aligned with the Apple Pkl compatibility baseline. See [docs/versioning.md](docs/versioning.md) and [CHANGELOG.md](CHANGELOG.md).

## Documentation

| Category | Document | Description |
| --- | --- | --- |
| **Users & Release** | [docs/installation.md](docs/installation.md) | Prebuilt binaries, checksum verification, npm packages, source builds |
| | [docs/versioning.md](docs/versioning.md) | Versioning, branch & tag policy, upgrade rules, release checklist |
| | [CHANGELOG.md](CHANGELOG.md) | User-facing release notes and known pre-release boundaries |
| | [SECURITY.md](SECURITY.md) | Vulnerability reporting and Pkl security boundaries |
| | [THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md) | Embedded assets, vendored regex data, and crate licenses |
| **Contributors** | [CONTRIBUTING.md](CONTRIBUTING.md) | Development workflow, principles, and PR guidelines |
| | [TESTING.md](TESTING.md) | Complete test inventory and compatibility verification contract |
| | [docs/ci.md](docs/ci.md) | GitHub Actions CI/Release workflows and `local_verify.py` suites |
| | [docs/architecture.md](docs/architecture.md) | Engine, VM, heap, Host, and cross-target architecture |
| | [docs/oracle.md](docs/oracle.md) | Bootstrapping pinned JVM, Native Image, and GraalVM oracles |
| **History & Roadmap** | [docs/development-history.md](docs/development-history.md) | M0–M7 milestone checkpoints, metrics, and evidence archives |
| | [docs/README.md](docs/README.md) | Complete index of all repository documentation |
| | [INTENT.md](INTENT.md) | Project goals, scope, and performance requirements |
| | [PLAN.md](PLAN.md) | Roadmap, execution plan, and work status (Korean) |

## License & Trademarks

`pkl-rs` is licensed under the [Apache License, Version 2.0](LICENSE) (Copyright 2025–2026 The pkl-rs Authors).

`pkl-rs` is an independent open-source project and is not affiliated with, sponsored by, or endorsed by Apple Inc. "Pkl" is a trademark or product name of Apple Inc., referenced solely to describe language and specification compatibility.

Attribution and third-party notices for embedded Apple Pkl `0.32.0` standard library modules, `pkldoc` web assets/fonts, FDLIBM, SnakeYAML Engine, TimSort, Unicode 15.0.0 UCD data, and statically linked crate dependencies are provided in [NOTICE](NOTICE) and [THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md).
