# Changelog

All notable user-facing changes to `pkl-rs` are documented in this file.

This changelog follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the versioning policy defined in [docs/versioning.md](docs/versioning.md). For detailed internal engineering milestone records (M0–M7), see [docs/development-history.md](docs/development-history.md).

## [Unreleased] — `0.32.0` Release Preparation

### Compatibility Baseline

- Targets **Apple Pkl `0.32.0`** (`external/pkl` @ `e51fb2f91774265666f94980ec630db794a4db2d`, `external/pkl-pantry` @ `904a68184e54da378b0a0e8a911a3fab3956f693`).
- Explicit scope exclusions: `pkl-codegen-java`, `pkl-codegen-kotlin`, and JVM public API adapters are out of scope (see [INTENT.md](INTENT.md)).

### Added

- **Pure-Rust Pkl `0.32.0` Engine (`crates/pkl-runtime`, `crates/pkl-formatter`)**
  - Single shared pure-Wasm evaluation engine, value model, diagnostics, and CST code formatter with `unsafe_code = "deny"` and zero Java runtime dependency.
  - Full Pkl `0.32.0` language semantics, all 23 `pkl:*` standard library modules (542 builtins), all 9 output renderers (`pcf`, `json`, `yaml`, `xml`, `properties`, `plist`, `jsonnet`, `textproto`, `pkl-binary`), `output.bytes`, `output.files`, `pkl:test`, and `pkl:Command`.
- **Native CLI, Tooling & Rust API (`crates/pkl`, `crates/pkl-cli`, `crates/pkl-doc`, `crates/pkl-repl`, `crates/pkl-server`, `crates/pkl-host-native`)**
  - `pkl` CLI supporting `eval`, `repl`, `server`, `test`, `project resolve`, `project package`, `download-package`, `analyze imports`, `format`, `run`, and `shell-completion` (`bash`, `zsh`, `fish`).
  - `pkldoc` standalone HTML documentation generator and migrator.
  - Public ergonomic Rust API (`crates/pkl`): `Evaluator`, `EvaluatorBuilder`, `ModuleSource`, `PklValue`/`FromPklValue`, `ModuleSchema`, `Project`, `Analyzer`, `SecurityManager`, `HttpClient`, custom `ModuleKeyFactory`/`ResourceReader`, cancellation, and timeouts.
  - Native Host I/O supporting `file:`, `env:`, `prop:`, `modulepath:` (directories and JAR/ZIP), HTTP/HTTPS (rustls + bundled `PklCARoots.pem`, custom CA certs, proxy/`noProxy`, headers, rewrites), external readers (`0x26–0x32`), `PklProject`, and `package:`/`projectpackage:` disk cache.
- **WebAssembly & Node.js N-API Distributions (`@pkl-rs/wasm`, `@pkl-rs/napi`)**
  - `@pkl-rs/wasm`: ESM browser and Node.js Wasm distribution (`pkl_wasm_bg.wasm` `<= 2.50 MiB` gzip-9), `VirtualFileSystem`, cooperative async Host callbacks, `createRepl()`, `analyzeImportGraph()`, `evaluateSchema()`, `evaluateTest()`, `format()`, `decodePklBinary()`, lossless `BigInt`/`Uint8Array` boundaries, and explicit `capabilities()` reporting.
  - `@pkl-rs/napi`: Dual ESM/CJS Node.js native addon with TypeScript declarations, non-blocking libuv worker threads (`Promise`) and `*Sync` APIs, full native Host I/O, and custom sync/async JS extensions across 7 target platforms.
- **Distribution, Packaging & Verification Infrastructure**
  - Unified `0.32.0` versioning across Rust workspace crates, `@pkl-rs/wasm`, `@pkl-rs/napi`, and CLI binaries.
  - Cross-platform release workflow ([.github/workflows/release.yml](.github/workflows/release.yml)) producing Linux (`glibc <= 2.28` + static `musl`, `x64`/`arm64`), macOS (`arm64`/`x64`), and Windows (`x64`) CLI archives, `@pkl-rs/wasm`, 7-platform `@pkl-rs/napi`, and `SHA256SUMS.txt`, with `LICENSE`, `NOTICE`, and `THIRD-PARTY-NOTICES.md` bundled into every archive and npm tarball.
  - Local verification runner ([tools/local_verify.py](tools/local_verify.py)) covering 18 suites across Rust, Python, Node.js, M0–M6 differential gates, and M7 fuzz/soak/benchmark suites.

### Performance (Local macOS `arm64`)

- Achieved **15 / 15 `<= 1.00x` execution time (`0.055x–0.935x`) and `<= 1.00x` peak RSS (`0.136x–0.662x`)** against SHA-256 pinned Official Apple Pkl `0.32.0` `native-image` across all CLI, server, and `pkldoc` workloads with 100% raw-byte parity.
- `@pkl-rs/wasm` compressed binary reduced to `2.498 MiB` gzip-9 (`<= 2.50 MiB` budget) via `[profile.wasm-release]`.

### Known Pre-Release Boundaries

- Prebuilt cross-platform release artifacts (`darwin-x64`, `linux-x64-gnu`, `linux-arm64-gnu`, `linux-x64-musl`, `linux-arm64-musl`, `win32-x64-msvc`) and hosted GitHub Actions release verification have not yet been captured for GA (`release_ready: false`).
- `vendor/java_regex` character name tables carry OpenJDK `uniName.dat` extraction notices (see [THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md)); regenerating directly from Unicode Consortium `UnicodeData.txt` is tracked prior to GA.
- Crates remain `publish = false` until `vendor/java_regex` is resolved for crates.io publication.
