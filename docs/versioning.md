# Versioning & Release Management

`pkl-rs` maintains unified versioning across its Rust workspace, CLI binaries, npm packages, and Git release tags, anchored to the Apple Pkl compatibility baseline.

---

## 1. Unified Version Policy

`pkl-rs` aligns its `MAJOR.MINOR.PATCH` version with the **Apple Pkl compatibility baseline** (`0.32.0`) so users can immediately tell which Pkl language, standard library, CLI, and protocol specification a release implements.

| Component | Location | Current Version | Format / Runtime Output |
| --- | --- | --- | --- |
| Rust workspace crates (10 crates + `xtask`) | [Cargo.toml](../Cargo.toml) (`[workspace.package]`) | `0.32.0` | Cargo SemVer (`0.32.0`) |
| `pkl` CLI binary | `crates/pkl/src/release.rs` | `0.32.0` | `Pkl 0.32.0 (<os>, native)` |
| `pkldoc` CLI binary | `crates/pkl-doc/src/scope.rs` | `0.32.0` | `pkldoc 0.32.0 (<os>, native)` |
| `@pkl-rs/wasm` npm package | [packages/pkl-wasm/package.json](../packages/pkl-wasm/package.json) | `0.32.0` | npm SemVer + `PKL_VERSION = "0.32.0"` |
| `@pkl-rs/napi` npm package | [packages/pkl-napi/package.json](../packages/pkl-napi/package.json) | `0.32.0` | npm SemVer + `PKL_VERSION = "0.32.0"` |
| Git release tags | Git tags (`v*`) | `v0.32.0` *(or `v0.32.0-rc.N`)* | Triggers [.github/workflows/release.yml](../.github/workflows/release.yml) |

### Version Rules

1. **Pre-releases (`0.32.0-rc.N` / tag `v0.32.0-rc.N`)**:
   - Used for release candidates before all cross-platform release gates pass (`release_ready: true`).
   - Tags containing a hyphen (e.g. `v0.32.0-rc.1`) automatically mark the GitHub Release as a prerelease in [.github/workflows/release.yml](../.github/workflows/release.yml).
2. **General Availability (`0.32.0` / tag `v0.32.0`)**:
   - Requires verified cross-platform binaries, npm tarballs, `SHA256SUMS.txt`, compliance with [TESTING.md](../TESTING.md), and resolution of pre-GA legal/publishing blockers noted below.
3. **Implementation-only fixes between upstream Pkl releases**:
   - Because Cargo and npm strictly order SemVer versions without build-metadata precedence, `pkl-rs`-specific post-GA fixes targeting the same Apple Pkl `0.32.0` baseline either:
     - ship as pre-releases prior to GA (`0.32.0-rc.2`, ...), or
     - if a post-GA maintenance release is needed before Apple releases Pkl `0.32.1`, increment the `pkl-rs` patch or prerelease tag per explicit release note while keeping `pkl::PKL_VERSION` (`"0.32.0"`) and `references.lock` (`baseline.pkl=0.32.0`) faithful to the exact Pkl language baseline. Any deviation from lockstep `MAJOR.MINOR.PATCH` must be recorded explicitly in [CHANGELOG.md](../CHANGELOG.md).
4. **Upstream Pkl baseline upgrades** (e.g. `0.32.0` → `0.33.0`):
   - Always performed in a dedicated baseline-upgrade checkpoint, never mixed silently with unrelated feature PRs.
   - Must update all of the following together:
     1. Git submodule gitlinks under `external/pkl` and `external/pkl-pantry`
     2. [references.lock](../references.lock) and [oracle.lock.json](../oracle.lock.json)
     3. Workspace `Cargo.toml`, `Cargo.lock`, `packages/pkl-wasm/package.json`, `packages/pkl-napi/package.json`, `crates/pkl/src/release.rs`, `crates/pkl-doc/src/scope.rs`, and `crates/pkl-runtime/src/stdlib/`
     4. All 12 milestone manifests under `compatibility/` and verification contracts in [TESTING.md](../TESTING.md)
     5. [CHANGELOG.md](../CHANGELOG.md), [INTENT.md](../INTENT.md), and [NOTICE](../NOTICE) / [THIRD-PARTY-NOTICES.md](../THIRD-PARTY-NOTICES.md)

---

## 2. Release Checklist

Before tagging a release (`vX.Y.Z` or `vX.Y.Z-rc.N`):

1. **Version & metadata lockstep**
   - Verify `[workspace.package].version` in [Cargo.toml](../Cargo.toml), `Cargo.lock`, [packages/pkl-wasm/package.json](../packages/pkl-wasm/package.json), and [packages/pkl-napi/package.json](../packages/pkl-napi/package.json) match the release version.
   - Confirm `cargo xtask verify` passes with zero drift against `references.lock`.
2. **Changelog & legal notices**
   - Move `[Unreleased]` entries in [CHANGELOG.md](../CHANGELOG.md) under the tagged version with release date.
   - Confirm [LICENSE](../LICENSE), [NOTICE](../NOTICE), and [THIRD-PARTY-NOTICES.md](../THIRD-PARTY-NOTICES.md) accurately reflect all embedded assets and `Cargo.lock` dependencies.
3. **Verification gates**
   - Run local full verification in a dedicated verification host:
     ```sh
     python3 tools/local_verify.py --preset full-strict
     ```
   - Confirm `.github/workflows/ci.yml` is green on `main`.
4. **Cross-platform packaging & publication**
   - Dry-run [.github/workflows/release.yml](../.github/workflows/release.yml) via `workflow_dispatch` (`publish_github_release=false`, `publish_npm=false`) to verify all 7 CLI targets, `@pkl-rs/wasm` (`<= 2.50 MiB` gzip-9), 7-addon `@pkl-rs/napi`, container smoke tests (Debian 12, Rocky 9, Ubuntu 22.04, Alpine 3.21), and `SHA256SUMS.txt`.
   - Push the version tag (`git tag -a v0.32.0-rc.1 -m "pkl-rs v0.32.0-rc.1" && git push github v0.32.0-rc.1`) to create the GitHub Release and upload all verified artifacts.
   - When ready to publish to npm, run `workflow_dispatch` with `publish_npm=true` (requires `NPM_TOKEN` repository secret).

---

## 3. Known Pre-GA Publishing Prerequisites

Two explicit engineering/compliance items remain tracked before general availability (GA) publication to crates.io and public registries:

1. **crates.io publication (`publish = false`)**: Workspace crates currently keep `publish = false` because `crates/pkl-runtime` patches `java_regex` locally via `[patch.crates-io] java_regex = { path = "vendor/java_regex" }`. crates.io disallows publishing crates that depend on unpublished path patches. Before publishing to crates.io, either upstream the UTF-16 patches to `java_regex` or publish the patched engine under an owned crate name (e.g. `pkl-java-regex`).
2. **OpenJDK `uniName.dat` character table regeneration**: As disclosed in [THIRD-PARTY-NOTICES.md](../THIRD-PARTY-NOTICES.md), `vendor/java_regex` currently includes character name tables decoded from OpenJDK Temurin 21's `uniName.dat` (carrying OpenJDK GPLv2+Classpath-Exception notices alongside the Unicode 15.0 License). Regenerating these tables directly from the official Unicode Consortium `UnicodeData.txt` is recommended prior to GA under pure Apache-2.0.
