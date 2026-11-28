# CI, Release Packaging, and Local Verification

GitHub Actions is split into two focused workflows to keep routine CI costs minimal while keeping cross-platform release packaging and distribution automated. All expensive compatibility, differential, cross-browser, fuzz, memory-soak, and benchmark verification suites execute in a local or dedicated verification host via [tools/local_verify.py](../tools/local_verify.py).

## 1. GitHub Actions scope (minimal tests, packaging, and release distribution)

1. **[.github/workflows/ci.yml](../.github/workflows/ci.yml)** (`push` to `main`/`release/**`, `pull_request` targeting `main`/`release/**`, `workflow_dispatch` — **2 Ubuntu `ubuntu-24.04` jobs only**, zero macOS/Windows runners):
   - `check-and-test`: `cargo xtask verify`, `cargo fmt --all -- --check`, workspace + `wasm32` `clippy -D warnings`, portable `wasm32-unknown-unknown` build checks, all 12 milestone manifest/contract `--check` verifiers (offline, no Java required), [tools/check_release_version.py](../tools/check_release_version.py) (Cargo/npm version lockstep), [tools/tests/test_local_verify.py](../tools/tests/test_local_verify.py), and fast Rust unit/product-crate smoke tests (`xtask`, `pkl-lang`, `pkl-formatter`, `pkl-server`, `pkl-repl`, `pkl-cli`, `pkl-doc`, `pkl-wasm`, `pkl-host-native`, and `pkl-runtime --lib`).
   - `package-smoke`: Builds and smoke-tests `@openedges/pkl-wasm` (`[profile.wasm-release]`, enforcing `pkl_wasm_bg.wasm` gzip-9 `<= 2.50 MiB` [`2,621,440 B`] plus `npm pack` clean-directory install), builds and smoke-tests `@openedges/pkl-napi`, and builds and smoke-tests release `pkl` and `pkldoc` CLI binaries.
2. **[.github/workflows/release.yml](../.github/workflows/release.yml)** (**ONLY on version tags `v*` or manual `workflow_dispatch`** — never runs on ordinary `push`/`pull_request`):
   - `verify-tag` (`ubuntu-24.04`, fast fail-first gate before cross-platform runners start): Runs [tools/check_release_version.py](../tools/check_release_version.py) to enforce strict SemVer tag syntax (`vMAJOR.MINOR.PATCH[-prerelease]`) and exact version lockstep across `Cargo.toml` (`[workspace.package]` + `[workspace.dependencies]`), `@openedges/pkl-wasm`, and `@openedges/pkl-napi`.
   - `package-linux-portable` (`ubuntu-24.04` `x86_64`, `ubuntu-24.04-arm` `aarch64`): Builds glibc `<= 2.28` (`manylinux_2_28`, checked via [tools/check_linux_glibc.py](../tools/check_linux_glibc.py)) and static `musl` (`alpine:3.21`, verified zero `INTERP`) `pkl`/`pkldoc` binaries and Linux `@openedges/pkl-napi` addons, then smoke-tests across Debian 12, Rocky Linux 9, Ubuntu 22.04, and Alpine 3.21.
   - `package-macos-windows` (`macos-15`, `macos-15-intel`, `windows-latest`): Builds and smoke-tests release `pkl`/`pkldoc` archives and `@openedges/pkl-napi` addons (`darwin-arm64`, `darwin-x64`, `win32-x64`).
   - `package-wasm` (`ubuntu-24.04`): Builds, gzip-checks (`<= 2.50 MiB`), tests, and packs `@openedges/pkl-wasm`.
   - `package-crates` (`ubuntu-24.04`): Runs full standalone `cargo package --locked` on `pkl-runtime` and `pkl-formatter` and `cargo package --no-verify --locked` on the 6 dependent workspace crates, verifying all 8 `.crate` tarballs stay below crates.io's `10 MiB` limit.
   - `assemble-and-release` (`ubuntu-24.04`): Collects all CLI archives (each bundling `LICENSE`, `NOTICE`, and `THIRD-PARTY-NOTICES.md`), `@openedges/pkl-wasm` tarball, and all 7 platform `.node` addons into a single multi-platform `@openedges/pkl-napi` tarball, computes `SHA256SUMS.txt`, renders and syntax-checks (`ruby -c`) `Formula/pkl-rs.rb` via [tools/render_homebrew_formula.py](../tools/render_homebrew_formula.py), uploads `release-bundle` (30-day retention), and publishes a GitHub Release on `v*` tags (or manual `publish_github_release=true`).
   - Downstream publish jobs (each isolated with least-privilege permissions and depending on `assemble-and-release`; triggered automatically on `v*` tag pushes or selectively via `workflow_dispatch`): `publish-npm` (`--tag next` on prereleases, `environment: npm`, `id-token: write` OIDC Trusted Publishing + `--provenance`, no static `NPM_TOKEN`), `publish-homebrew` (automatic on stable non-`-` `v*` tags or manual `publish_homebrew=true`, `openedges/homebrew-tap`, `HOMEBREW_TAP_TOKEN`), and `publish-crates` (topological order for all 8 crates via `environment: crates.io`, `id-token: write` + `rust-lang/crates-io-auth-action` OIDC Trusted Publishing, no static `CARGO_REGISTRY_TOKEN`). See [docs/versioning.md](versioning.md) for the release checklist.

All GitHub Actions third-party steps are commit-SHA pinned and mechanically guarded by [tools/tests/test_local_verify.py](../tools/tests/test_local_verify.py).

## 2. Local / dedicated verification environment ([tools/local_verify.py](../tools/local_verify.py))

Every verification layer omitted from minimal GitHub CI is covered without silent omissions by [tools/local_verify.py](../tools/local_verify.py) across **18 suites** and **5 presets**:

| Suite ID | Scope & Exact Coverage | External Prereqs |
| --- | --- | --- |
| `workspace` | `xtask verify`, `cargo fmt --check`, `clippy` (workspace, `wasm32`), `wasm32` check, and full `cargo test --workspace --locked` (**379 targets** / ~2,500+ tests, including internal `pkl_runtime::java_regex` unit and UTF-16 authority tests) | Rust `1.98.1` |
| `manifests` | All **12** milestone manifest `--check` scripts, M0/M7 benchmark contracts, JVM & `native-image` oracle verification, and all **59 active** Python unit test modules (`tools/tests/`, dynamically discovered) | Python `3.10+`, JVM + `native-image` oracles |
| `platform-js-tests` | All **41** Node.js platform/adapter/transport unit test files under `tools/platform/*.test.mjs` (**384 tests**, dynamically discovered) | Node `26+` |
| `packages` | Standalone `@openedges/pkl-wasm` build + gzip-9 `<= 2.50 MiB` + `wasm.test.mjs` (`npm pack`), `@openedges/pkl-napi` build + `run.mjs` + `pack-install.mjs`, and release `pkl`/`pkldoc` CLI smoke | Node `26+`, `wasm-bindgen 0.2.108` |
| `py-historical` | Opt-in (`--include-historical-tests`): all **75 historical** M2 fixture/compare Python modules (`59 + 75 = 134/134` files in `tools/tests/`; preserves 2 Stage-65 stale-`oracle.lock.json` modules honestly) | Python `3.10+`, JVM oracle |
| `protobuf-strict` | Opt-in: explicit `#[ignore]` Rust `java04_strict_full_compatibility_gate` with `PKL_PROTOBUF_CAPTURE_DIR` (tracks 18 upstream `REFERENCE-NONPASS` slots; contracted exit `101`) | Rust `1.98.1` |
| `m0` | Pinned Java oracle smoke, smoke corpus record/check, independent Java replay `compare`/`check`, and selected regressions (`--full-corpus` adds all discovered command cases) | JVM oracle |
| `m1` | M1 contracts, semantics, retained Graal archive SHA-256 verify + extract + `m1_graal.py --check-report`, frontend (invokes `#[ignore]` `frontend_probe`), runtime, Host, lifecycle, and native/Node/Chromium `m1_platforms.py --verify-contract` | JVM oracle, Playwright Chromium |
| `m2` | M2 language differential (6 fast adapters or `--full-corpus` all 7 = 4,575 cases + `--verify-full`), class lifecycle record/verify, identifier tables, development contract verify | JVM oracle |
| `m3` | M3 1,781-item mechanical coverage probe + 3-lane raw-byte differential (`118` cases, `--verify-report` + `--verify-strict`) | JVM oracle |
| `m4` | M4 offline differential (lanes 1–3 [`86` cases] or `--full-corpus` all 5 Pantry 29-package lanes [`429` cases] + `--verify-coverage`) + `--verify-strict` | JVM oracle |
| `m5` | M5 5-lane offline differential (`153` cases: Rust API/CLI, REPL, server, formatter, `pkldoc`) + `--verify-report` + `--verify-strict` + `--verify-coverage` | JVM oracle |
| `m6` | M6 5-lane Wasm/N-API differential (`80` executable + `6` `capability-blocked`) + `--verify-report`/`--verify-strict`/`--verify-coverage` + Playwright cross-browser smoke (`--browsers`) | JVM oracle, Playwright |
| `m7-fuzz` | M7 seeded differential fuzzer (`210` generated across 7 categories + `5` regressions + `--verify-strict`) | JVM oracle |
| `m7-soak` | M7 memory reclamation soak (native 6-pair or `--all-soak-targets` all 18 Native/N-API/Wasm pairs + `--verify-strict`) | Node `26+` (for all-18) |
| `m7-bench` | M7 benchmark suite (`--baseline native-image|both|jvm`, `--bench-profile full|quick`, `--verify-report`, optional `--strict-bench`) | Official `native-image` (+ JVM for `both`/`jvm`) |
| `m7-js` | M7 cross-browser + Node JS/Wasm performance and memory decomposition (`m7_js_analysis.py`) | JVM oracle, Playwright |
| `m7-release` | Unified `m7_manifest.py` release audit wired automatically to `m7-bench`/`m7-soak`/`m7-fuzz` outputs (or explicit `--release-dir`, optional `--local-checkpoint`) | M7 outputs |

> **Note on `pkl_runtime::java_regex`**: The checked UTF-16 Java regular expression engine and official Unicode 15.0.0 UCD tables live directly inside `crates/pkl-runtime/src/java_regex/` (with zero `[patch.crates-io]` overrides or excluded vendor crates), so all regex unit tests, `unsafe_code = "deny"` lints, and `clippy -D warnings` run automatically inside standard workspace checks.

### Presets and usage

```sh
# List all 18 suites, discovered test counts (59 active + 75 historical Python, 41 platform JS), and presets
python3 tools/local_verify.py --list

# Preview commands without executing
python3 tools/local_verify.py --preset full-strict --dry-run

# Quick local gate: workspace (378 targets) + manifests (12 checks + 59 active Python tests) + platform-js-tests (41 files / 384 tests) + packages
python3 tools/local_verify.py --preset quick

# Compatibility differential suites (M0-M6; add --full-corpus for M2 4,575 + M4 Pantry all-5)
python3 tools/local_verify.py --preset compat

# M7 fuzz + soak + benchmark (official native-image or both) + JS analysis + release audit
python3 tools/local_verify.py --preset m7 --baseline native-image

# Full strict local gate (forces --full-corpus, --all-soak-targets, --baseline both, --strict-bench)
python3 tools/local_verify.py --preset full-strict --local-checkpoint

# Optional: run all 75 historical M2 Python suites or the ignored strict Protobuf gate
python3 tools/local_verify.py --suite py-historical
python3 tools/local_verify.py --suite protobuf-strict
```

Each run creates a fresh timestamped output directory under `target/local-verify/<UTC-timestamp>/` (or `--output-root <dir>`) and writes `summary.json` tagged `provenance: "local-verification-environment-not-hosted-ci"`. Failures are preserved honestly and always return non-zero.

## 3. Local checkpoint acceptance

Hosted CI is an automation and packaging/release pipeline, not the sole transport for compatibility or performance evidence. A checkpoint may close with recorded local verification of its applicable contract by recording exact commit, source/build identities, oracle settings, commands, host/runtime versions, raw results, known failures, and unexecuted platform coverage. Never relabel local evidence as hosted CI, and never claim `release_ready: true` while required OS, browser, or hosted-CI targets remain unverified.

## 4. Historical M0/M1 CI verification record

Initialize the pinned submodules as described in [README.md](../README.md). Run the workspace and Python checks in [TESTING.md](../TESTING.md). Install the exact reference JDK before following [the oracle bootstrap](oracle.md) and the record/compare/check commands in [the comparison contract](compatibility-tooling.md). New runs require fresh evidence directories. Candidate replay must use the same absolute workspace path as its Java baseline so error locations can be compared without rewriting bytes.

Recorded evidence is reviewable on another machine, but its absolute diagnostic paths and native JDK binary hashes belong to the original host. Generate a fresh Java baseline in the candidate's controlled environment for a formal comparison. Do not normalize historical output to make a different host appear identical.

The local M0 checks passed on the recorded macOS host. After authenticated access became available, historical pre-organization development-fork [run 39364517355](https://github.com/luuvish/pkl-rs/actions/runs/39364517355) was verified for the final M0 code commit `2164ffa`. All four jobs succeeded: the three workspace platforms and Linux Java tooling. The downloaded artifact ZIP matches GitHub's SHA-256 digest; [reports/m0/ci-summary.json](../reports/m0/ci-summary.json) and [reports/m0/ci-evidence.tar.gz](../reports/m0/ci-evidence.tar.gz) retain the evidence.

The M1 code commits `e9ee8d5`, `6a5e4b5`, and `bb80685`/`4cd9c08` passed all jobs in runs [39372369602](https://github.com/luuvish/pkl-rs/actions/runs/39372369602), [39374668362](https://github.com/luuvish/pkl-rs/actions/runs/39374668362), and [39378401720](https://github.com/luuvish/pkl-rs/actions/runs/39378401720), with verified artifact digests preserved in `reports/m1/ci-*-summary.json` and `reports/m1/ci-*-evidence.tar.gz`.
