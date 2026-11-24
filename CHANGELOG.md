# Changelog

All notable user-facing changes to `pkl-rs` are documented in this file.

This changelog follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the versioning policy defined in [docs/versioning.md](docs/versioning.md). For detailed internal engineering milestone records (M0–M7), see [docs/development-history.md](docs/development-history.md).

## [Unreleased]

## [0.32.0-rc.2] — 2026-11-24

### Changed

- **npm Scope Alignment (`@openedges`)**: Renamed the two npm packages from `@pkl-rs/wasm` and `@pkl-rs/napi` to **`@openedges/pkl-wasm`** and **`@openedges/pkl-napi`**, updated [release.yml](.github/workflows/release.yml) tarball publish globs (`openedges-pkl-wasm-*.tgz`, `openedges-pkl-napi-*.tgz`), clean-install tests, M6 packaging runner (also accepting `0.32.0-*` prerelease versions), and release documentation.
- Bumped all workspace crates (`Cargo.toml` / `Cargo.lock`) and npm manifests in lockstep to **`0.32.0-rc.2`** (preserving strict tag immutability after `v0.32.0-rc.1`; language baseline `PKL_VERSION = "0.32.0"` unchanged).

### Fixed

- **Windows Verbatim Paths & `file:///` Drive URIs (`crates/pkl-host-native`, `crates/pkl-runtime`)**: Stripped Windows `\\?\<drive>:` extended-length prefixes emitted by `std::fs::canonicalize` in `path_to_file_uri`, `file_uri_from_path`, and `normalize_lexical_path`, and stripped leading `/` on Windows drive URIs (`/C:/...`) inside `file_uri_to_path`, `check_uri`, and `resolve_secure_path` (fixing `Project.load` and `rootDir` confinement failures on `win32-x64` observed during `v0.32.0-rc.1` CI).

## [0.32.0-rc.1] — 2026-11-24

First release candidate targeting **Apple Pkl `0.32.0`** across all 4 distribution channels (GitHub Releases, npm, crates.io, and Homebrew).

### Compatibility Baseline

- Targets **Apple Pkl `0.32.0`** (`external/pkl` @ `e51fb2f91774265666f94980ec630db794a4db2d`, `external/pkl-pantry` @ `904a68184e54da378b0a0e8a911a3fab3956f693`).
- Explicit scope exclusions: `pkl-codegen-java`, `pkl-codegen-kotlin`, and JVM public API adapters are out of scope (see [INTENT.md](INTENT.md)).

### Added

- **Pure-Rust Pkl `0.32.0` Engine (`crates/pkl-runtime`, `crates/pkl-formatter`)**
  - Single shared pure-Wasm evaluation engine, value model, diagnostics, and CST code formatter with `unsafe_code = "deny"` and zero Java runtime dependency.
  - Full Pkl `0.32.0` language semantics, all 23 `pkl:*` standard library modules (542 builtins), all 9 output renderers (`pcf`, `json`, `yaml`, `xml`, `properties`, `plist`, `jsonnet`, `textproto`, `pkl-binary`), `output.bytes`, `output.files`, `pkl:test`, and `pkl:Command`.
- **Native CLI, Tooling & Rust API (`crates/pkl` [`pkl-rs`], `crates/pkl-cli`, `crates/pkl-doc`, `crates/pkl-repl`, `crates/pkl-server`, `crates/pkl-host-native`)**
  - `pkl` CLI supporting `eval`, `repl`, `server`, `test`, `project resolve`, `project package`, `download-package`, `analyze imports`, `format`, `run`, and `shell-completion` (`bash`, `zsh`, `fish`).
  - `pkldoc` standalone HTML documentation generator and migrator.
  - Public ergonomic Rust API published as **`pkl-rs`** (`[lib] name = "pkl"`, so Rust callers keep `use pkl::...`): `Evaluator`, `EvaluatorBuilder`, `ModuleSource`, `PklValue`/`FromPklValue`, `ModuleSchema`, `Project`, `Analyzer`, `SecurityManager`, `HttpClient`, custom `ModuleKeyFactory`/`ResourceReader`, cancellation, and timeouts.
  - Native Host I/O supporting `file:`, `env:`, `prop:`, `modulepath:` (directories and JAR/ZIP), HTTP/HTTPS (rustls + bundled `PklCARoots.pem`, custom CA certs, proxy/`noProxy`, headers, rewrites), external readers (`0x26–0x32`), `PklProject`, and `package:`/`projectpackage:` disk cache.
- **WebAssembly & Node.js N-API Distributions (`@pkl-rs/wasm`, `@pkl-rs/napi`)**
  - `@pkl-rs/wasm`: ESM browser and Node.js Wasm distribution (`pkl_wasm_bg.wasm` `<= 2.50 MiB` gzip-9), `VirtualFileSystem`, cooperative async Host callbacks, `createRepl()`, `analyzeImportGraph()`, `evaluateSchema()`, `evaluateTest()`, `format()`, `decodePklBinary()`, lossless `BigInt`/`Uint8Array` boundaries, and explicit `capabilities()` reporting.
  - `@pkl-rs/napi`: Dual ESM/CJS Node.js native addon with TypeScript declarations, non-blocking libuv worker threads (`Promise`) and `*Sync` APIs, full native Host I/O, and custom sync/async JS extensions across 7 target platforms.
- **4-Channel Distribution, Packaging & Verification Infrastructure**
  - Unified `0.32.0-rc.1` version lockstep across root `Cargo.toml` (`[workspace.package]` + `[workspace.dependencies]`), all 8 publishable crates (`pkl-runtime`, `pkl-formatter`, `pkl-host-native`, `pkl-rs`, `pkl-repl`, `pkl-server`, `pkl-doc`, `pkl-cli`), `@pkl-rs/wasm`, and `@pkl-rs/napi`, mechanically enforced by [tools/check_release_version.py](tools/check_release_version.py).
  - Cross-platform release workflow ([.github/workflows/release.yml](.github/workflows/release.yml)) producing:
    - **GitHub Releases**: Linux (`glibc <= 2.28` + static `musl`, `x64`/`arm64`), macOS (`arm64`/`x64`), and Windows (`x64`) CLI archives, `@pkl-rs/wasm`, 7-platform `@pkl-rs/napi`, and `SHA256SUMS.txt`, with `LICENSE`, `NOTICE`, and `THIRD-PARTY-NOTICES.md` bundled into every archive and npm tarball.
    - **npm**: Automated `@pkl-rs/wasm` and `@pkl-rs/napi` publishing with `--tag next` protection for prerelease versions (`*-*`).
    - **crates.io**: Standalone `.crate` verification (`package-crates`) and topological publishing of all 8 publishable crates (`pkl-runtime` → `pkl-formatter` → `pkl-host-native` → `pkl-rs` / `pkl-repl` / `pkl-server` / `pkl-doc` → `pkl-cli`).
    - **Homebrew**: Deterministic Formula renderer ([tools/render_homebrew_formula.py](tools/render_homebrew_formula.py)) emitting `Formula/pkl-rs.rb` (`pkl` + `pkldoc` + bash/zsh/fish completions + notices) validated with `ruby -c` and published to `openedges/homebrew-tap`.
  - Local verification runner ([tools/local_verify.py](tools/local_verify.py)) covering 18 suites across Rust, Python, Node.js, M0–M6 differential gates, and M7 fuzz/soak/benchmark suites.

### Performance & Stability (Local macOS `arm64`)

- Achieved **15 / 15 `<= 1.00x` execution time (`0.073x–0.761x`) and `<= 1.00x` peak RSS (`0.130x–0.551x`)** against SHA-256 pinned Official Apple Pkl `0.32.0` `native-image` across all CLI, server, and `pkldoc` workloads (`18 / 18 PASS` vs archived M0 GraalVM CE `25.0.1` Core-6; `33 / 34 PASS` vs live Temurin `21.0.13.1+2-LTS`, with `wasm-node-typed-conversion` faster at `0.386x` time and `5.9 MiB` Wasm linear memory while Node v26.13.1 process peak RSS measured `230.2 MiB` vs `207.3 MiB` JVM = `1.111x`).
- `@pkl-rs/wasm` compressed binary reduced to `2.027 MiB` (`2,124,981 B`) gzip-9 (`6,703,884 B` raw, 18.9% under the `2.50 MiB` budget) via `[profile.wasm-release]` (`opt-level = "z"`, fat LTO, single codegen unit).
- Verified **18 / 18 bounded** long-running memory reclamation soak runs (`native` 6/6, `napi-node` 6/6, `wasm-node` 6/6; 0 leaks, 0 FD/orphan leaks), **215 / 215** seeded differential fuzzing cases, **889 / 889** upstream `LanguageSnippetTests` raw-byte parity on product `pkl` CLI (including triple-dot `read(".../...")` across Native, N-API, REPL, and Wasm hosts), **118 / 118** M3, **429 / 429** M4 (all 29 Pantry packages), **153 / 153** M5, and **80 / 80** (+ 6 explicit `capability-blocked`) M6 differential gates.

### Legal, Licensing & Brand Separation

- Regenerated all 12 Unicode 15.0.0 data tables directly from official Unicode Consortium UCD 15.0.0 files (`Unicode-3.0`) via [tools/generate_ucd15_regex_tables.py](tools/generate_ucd15_regex_tables.py), eliminating all OpenJDK `uniName.dat` / GPLv2 legal files, and absorbed the self-contained UTF-16 Java regex engine into `crates/pkl-runtime/src/java_regex/` (removing `vendor/java_regex/`, `[patch.crates-io]`, and 4 unused upstream Unicode crates from `Cargo.lock`).
- Replaced Apple's Pkl `pkldoc` favicons with original `pkl-rs` Apache-2.0 SVG/PNG artwork (`crates/pkl-doc/assets/images/`), updated CLI version banners to `pkl-rs 0.32.0` / `pkldoc-rs 0.32.0` and HTTP `User-Agent` (including `pkldoc`) to `pkl-rs/0.32.0`, and added explicit Apple Pkl non-affiliation trademark notices.
- Completed [NOTICE](NOTICE) and [THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md) with verbatim notices/texts for Sun FDLIBM (1993), SnakeYAML Engine 3.0.1, AOSP/Apache Spark TimSort, SIL OFL 1.1, Unicode-3.0, MIT, ISC, BSD-3-Clause, and `ring`.

### Known Pre-Release Boundaries

- Prebuilt cross-platform release artifacts (`darwin-x64`, `linux-x64-gnu`, `linux-arm64-gnu`, `linux-x64-musl`, `linux-arm64-musl`, `win32-x64-msvc`), post-optimization Firefox/WebKit browser captures, and hosted GitHub Actions release verification remain to be completed before GA `0.32.0` (`release_ready: false`).
- On Node.js v26.13.1 (`darwin-arm64`), `wasm-node-typed-conversion` beats Temurin 21 on execution time (`0.386x`, `12.60 us` vs `32.67 us`) and Wasm linear memory (`5.9 MiB`, `0.030x`), while whole-process V8 RSS reached `230.2 MiB` vs `207.3 MiB` JVM (`1.111x`, `heapTotal` ~`80 MiB` / `heapUsed` `23.6 MiB`).
- Binding/internal crates (`pkl-wasm`, `pkl-napi`, `xtask`) stay `publish = false`; `@pkl-rs/wasm` and `@pkl-rs/napi` are distributed via npm.
