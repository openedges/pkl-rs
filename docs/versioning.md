# Versioning, Branch & Tag Policy

`pkl-rs` maintains unified versioning, clean branch discipline, and immutable release tags across its Rust workspace, CLI binaries, npm packages, and GitHub Releases, anchored to the Apple Pkl compatibility baseline.

---

## 1. Unified Version Policy

`pkl-rs` aligns its `MAJOR.MINOR.PATCH` version with the **Apple Pkl compatibility baseline** (`0.32.0`) so users can immediately tell which Pkl language, standard library, CLI, and protocol specification a release implements.

| Component | Location | Current Version | Format / Runtime Output |
| --- | --- | --- | --- |
| Rust workspace crates (8 publishable crates + 3 internal/binding crates) | [Cargo.toml](../Cargo.toml) (`[workspace.package]` + `[workspace.dependencies]`) | `0.32.0-rc.2` | Cargo SemVer (`0.32.0-rc.2`); `crates/pkl` published as `pkl-rs` (`[lib] name = "pkl"`) |
| `pkl` CLI binary | `crates/pkl/src/release.rs` | `0.32.0` (language baseline) | `pkl-rs 0.32.0 (<os>, native)` |
| `pkldoc` CLI binary | `crates/pkl-doc/src/scope.rs`, `cli.rs` | `0.32.0` (language baseline) | `pkldoc-rs 0.32.0 (<os>, native)` |
| `@openedges/pkl-wasm` npm package | [packages/pkl-wasm/package.json](../packages/pkl-wasm/package.json) | `0.32.0-rc.2` | npm SemVer (`--tag next` on prereleases) + `PKL_VERSION = "0.32.0"` |
| `@openedges/pkl-napi` npm package | [packages/pkl-napi/package.json](../packages/pkl-napi/package.json) | `0.32.0-rc.2` | npm SemVer (`--tag next` on prereleases) + `PKL_VERSION = "0.32.0"` |
| Homebrew Tap formula | `openedges/homebrew-tap` (`Formula/pkl-rs.rb`) | `0.32.0-rc.2` | Rendered from `SHA256SUMS.txt` by [tools/render_homebrew_formula.py](../tools/render_homebrew_formula.py) |
| Git release tags | Git tags (`v*`) | `v0.32.0-rc.2` *(GA: `v0.32.0`)* | Triggers [.github/workflows/release.yml](../.github/workflows/release.yml) |

### Version Rules

1. **Pre-releases (`0.32.0-rc.N` / tag `v0.32.0-rc.N`)**:
   - Used for release candidates before all cross-platform release gates pass (`release_ready: true`).
   - Manifests (`Cargo.toml`, `packages/pkl-wasm/package.json`, `packages/pkl-napi/package.json`) **must** be bumped to `0.32.0-rc.N` before tagging `v0.32.0-rc.N` so npm and Cargo never publish a GA version from a prerelease tag.
   - Tags containing a hyphen automatically mark the GitHub Release as a prerelease in [.github/workflows/release.yml](../.github/workflows/release.yml).
2. **General Availability (`0.32.0` / tag `v0.32.0`)**:
   - Requires verified cross-platform binaries, npm tarballs, `SHA256SUMS.txt`, compliance with [TESTING.md](../TESTING.md), and resolution of pre-GA prerequisites in Section 5.
3. **Implementation-only fixes between upstream Pkl releases**:
   - Prior to GA, increment the prerelease counter (`0.32.0-rc.1` → `0.32.0-rc.2`).
   - Post-GA, if a `pkl-rs`-only maintenance release is needed before Apple ships Pkl `0.32.1`, increment the `pkl-rs` patch version per explicit release note while keeping `pkl::PKL_VERSION` (`"0.32.0"`) and `references.lock` (`baseline.pkl=0.32.0`) faithful to the exact Pkl language baseline, and record the baseline mapping explicitly in [CHANGELOG.md](../CHANGELOG.md).
4. **Upstream Pkl baseline upgrades** (e.g. `0.32.0` → `0.33.0`):
   - Always performed on a dedicated `baseline/pkl-<X.Y.Z>` branch, never mixed silently with unrelated feature changes.
   - Must update together:
     1. Git submodule gitlinks under `external/pkl` and `external/pkl-pantry`
     2. [references.lock](../references.lock) and [oracle.lock.json](../oracle.lock.json)
     3. Workspace `Cargo.toml`, `Cargo.lock`, `packages/pkl-wasm/package.json`, `packages/pkl-napi/package.json`, `crates/pkl/src/release.rs`, `crates/pkl-doc/src/scope.rs`, and `crates/pkl-runtime/src/stdlib/`
     4. All 12 milestone manifests under `compatibility/` and verification contracts in [TESTING.md](../TESTING.md)
     5. [CHANGELOG.md](../CHANGELOG.md), [INTENT.md](../INTENT.md), and [NOTICE](../NOTICE) / [THIRD-PARTY-NOTICES.md](../THIRD-PARTY-NOTICES.md)

---

## 2. Branch Policy

`pkl-rs` follows a trunk-based workflow with short-lived topic branches and optional long-lived maintenance branches per supported Pkl minor line.

| Branch Pattern | Lifetime | Purpose | CI Trigger ([ci.yml](../.github/workflows/ci.yml)) |
| --- | --- | --- | --- |
| `main` | Permanent | Primary integration branch; always buildable and releasable | `push` + `pull_request` |
| `release/<MAJOR>.<MINOR>` (e.g. `release/0.32`) | Long-lived (while supported) | Maintenance/hotfix line for a shipped Pkl minor release after `main` moves to the next baseline | `push` + `pull_request` |
| `feat/<topic>` | Short-lived | New features or API additions | `pull_request` to `main`/`release/**` |
| `fix/<topic>` | Short-lived | Bug fixes and compatibility regressions | `pull_request` to `main`/`release/**` |
| `perf/<topic>` | Short-lived | Performance and memory optimizations | `pull_request` to `main`/`release/**` |
| `docs/<topic>` | Short-lived | Documentation-only updates | `pull_request` to `main`/`release/**` |
| `ci/<topic>` / `chore/<topic>` | Short-lived | CI, packaging, tooling, and maintenance tasks | `pull_request` to `main`/`release/**` |
| `baseline/pkl-<X.Y.Z>` | Short-lived | Dedicated upstream Apple Pkl reference baseline upgrade | `pull_request` to `main` |

### Branch Rules

1. **Protecting `main` and `release/**`**:
   - Never force-push (`git push --force`) or delete `main` or any `release/*` branch.
   - Recommended GitHub branch protection settings for `main` and `release/*`:
     - Require pull request reviews before merging.
     - Require status checks `check-and-test` and `package-smoke` ([ci.yml](../.github/workflows/ci.yml)) to pass.
     - Restrict force pushes and branch deletions.
2. **Pull Request & Merge Hygiene**:
   - Keep topic branches focused and short-lived; delete merged topic branches after merge.
   - Prefer clean atomic commits (or squash-merge for noisy work-in-progress branches) with clear scope in the commit title.
   - Because `ci.yml` restricts `push` triggers to `main` and `release/**`, pushing a topic branch does not burn duplicate Actions minutes until a PR targeting `main` or `release/**` is opened.
3. **Maintenance & Backporting (`release/<MAJOR>.<MINOR>`)**:
   - Cut `release/0.32` from the `v0.32.0` tag only when `main` begins targeting the next upstream Pkl baseline (e.g. `0.33.0`).
   - Land bug fixes and security patches on `main` first, then cherry-pick (`git cherry-pick -x`) onto `release/<MAJOR>.<MINOR>` via PR so fixes are never lost on `main`.

---

## 3. Tag Policy

Release tags are the authoritative trigger for [.github/workflows/release.yml](../.github/workflows/release.yml) and published artifacts.

### Tag Naming Format

- **General Availability (GA)**: `v<MAJOR>.<MINOR>.<PATCH>` (e.g. `v0.32.0`, `v0.32.1`)
- **Pre-release**: `v<MAJOR>.<MINOR>.<PATCH>-<prerelease>` (e.g. `v0.32.0-rc.1`, `v0.32.0-beta.1`)
- **Disallowed**:
  - Missing `v` prefix (e.g. `0.32.0`)
  - Build metadata `+...` in release tags (e.g. `v0.32.0+build1`)
  - Non-SemVer labels (e.g. `v0.32`, `latest`, `release-1`)

### Tag Rules

1. **Annotated tags only**:
   - Always create annotated tags with `-a` (and `-s` when GPG/SSH signing is configured):
     ```sh
     git tag -a v0.32.0-rc.1 -m "pkl-rs v0.32.0-rc.1"
     ```
   - Never use lightweight tags (`git tag v0.32.0`) for releases.
2. **Strict immutability**:
   - Once a `v*` tag is pushed to a public/shared remote or published to GitHub Releases / npm, **never delete, force-move, or reuse it**.
   - If a defect is found after tagging, fix it in a new commit and issue the next version (`v0.32.0-rc.2` or next patch).
3. **Tag origin**:
   - Create release tags **only** on verified commits already merged to `main` or `release/<MAJOR>.<MINOR>`, never on unmerged topic branches.
4. **Mechanical tag-to-manifest lockstep (`tools/check_release_version.py`)**:
   - Before any cross-platform macOS/Windows/Linux build starts, `release.yml::verify-tag` runs [tools/check_release_version.py](../tools/check_release_version.py) to guarantee exact equality across:
     - Git tag (stripped of `v`)
     - Root [Cargo.toml](../Cargo.toml) (`[workspace.package].version`)
     - [packages/pkl-wasm/package.json](../packages/pkl-wasm/package.json) (`version`)
     - [packages/pkl-napi/package.json](../packages/pkl-napi/package.json) (`version`)
   - Mismatched tags fail fast in seconds without consuming cross-platform runner minutes.
5. **Recommended GitHub tag protection**:
   - Configure a GitHub Ruleset / Tag Protection rule on `v*` restricting creation to release maintainers and blocking tag updates and deletions.

---

## 4. Release Checklist

Before creating and pushing a release tag (`vX.Y.Z` or `vX.Y.Z-rc.N`):

1. **Version & manifest lockstep**
   - Update `[workspace.package].version` in [Cargo.toml](../Cargo.toml), sync [Cargo.lock](../Cargo.lock) (`cargo update --workspace --offline`), and align [packages/pkl-wasm/package.json](../packages/pkl-wasm/package.json) and [packages/pkl-napi/package.json](../packages/pkl-napi/package.json).
   - Verify exact lockstep locally before tagging:
     ```sh
     python3 tools/check_release_version.py --tag v0.32.0-rc.1
     cargo xtask verify
     ```
2. **Changelog & legal notices**
   - Move `[Unreleased]` entries in [CHANGELOG.md](../CHANGELOG.md) under the tagged version with the release date.
   - Confirm [LICENSE](../LICENSE), [NOTICE](../NOTICE), and [THIRD-PARTY-NOTICES.md](../THIRD-PARTY-NOTICES.md) accurately reflect all embedded assets and `Cargo.lock` dependencies.
3. **Verification gates**
   - Run full verification in a local or dedicated verification host:
     ```sh
     python3 tools/local_verify.py --preset full-strict
     ```
   - Confirm [.github/workflows/ci.yml](../.github/workflows/ci.yml) is green on the target commit on `main` (or `release/<MAJOR>.<MINOR>`).
4. **Dry-run & 4-channel tag publication**
   - Optionally dry-run [.github/workflows/release.yml](../.github/workflows/release.yml) via `workflow_dispatch` (`publish_github_release=false`, `publish_npm=false`, `publish_crates=false`, `publish_homebrew=false`) to verify all 7 CLI targets, `@openedges/pkl-wasm` (`<= 2.50 MiB` gzip-9), 7-addon `@openedges/pkl-napi`, all 8 `.crate` packages (`<= 10 MiB`), container smoke tests (Debian 12, Rocky 9, Ubuntu 22.04, Alpine 3.21), `SHA256SUMS.txt`, and `Formula/pkl-rs.rb` (`ruby -c`).
   - Create and push the annotated tag from the verified commit on `main` (or `release/<MAJOR>.<MINOR>`):
     ```sh
     git tag -a v0.32.0-rc.1 -m "pkl-rs v0.32.0-rc.1"
     git push github v0.32.0-rc.1
     ```
     Pushing a `v*` tag automatically publishes the **GitHub Release** (marking `-*` tags as prereleases).
   - Once the GitHub Release and `SHA256SUMS.txt` are verified, run `workflow_dispatch` on the tagged commit with the desired channel toggles enabled:
     - `publish_npm=true` (requires `NPM_TOKEN` secret; prereleases automatically publish with `--tag next`)
     - `publish_crates=true` (requires `CARGO_REGISTRY_TOKEN` secret; publishes all 8 crates in topological order)
     - `publish_homebrew=true` (requires `HOMEBREW_TAP_TOKEN` secret with write access to `openedges/homebrew-tap`; downloads the authoritative `SHA256SUMS.txt` from the live GitHub Release when `publish_github_release=false`)

---

## 5. Multi-Channel Publishing Configuration

- **Zero `[patch.crates-io]` blockers**: The checked UTF-16 Java regular expression engine and official Unicode 15.0.0 UCD tables (`Unicode-3.0`) are embedded directly inside `crates/pkl-runtime/src/java_regex/` with zero external regex/Unicode crate dependencies and zero OpenJDK GPLv2 files.
- **crates.io crates (8 publishable, `publish = true`)**: Published in strict dependency order: `pkl-runtime` → `pkl-formatter` → `pkl-host-native` → `pkl-rs` (`crates/pkl`, `[lib] name = "pkl"`) → `pkl-repl` → `pkl-server` → `pkl-doc` → `pkl-cli`. Binding/internal crates (`pkl-wasm`, `pkl-napi`, `xtask`) remain `publish = false`.
- **npm packages (2 scoped packages)**: `@openedges/pkl-wasm` and `@openedges/pkl-napi` (requires npm org `@openedges` and `NPM_TOKEN`). Prereleases (`*-*`) always pass `--tag next` so they never overwrite `latest`.
- **Homebrew tap (`openedges/homebrew-tap`)**: `Formula/pkl-rs.rb` installs both `pkl` and `pkldoc` from the GitHub Release tarballs (`darwin-arm64`, `darwin-x64`, `linux-arm64-gnu`, `linux-x64-gnu`) with verified SHA-256 checksums, shell completions (`bash`, `zsh`, `fish`), and license/notice files.
