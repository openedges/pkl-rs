# Contributing

See [README.md](README.md) for the product overview, [docs/installation.md](docs/installation.md) for source setup, [docs/versioning.md](docs/versioning.md) for release and versioning rules, [TESTING.md](TESTING.md) for the complete verification contract, [docs/ci.md](docs/ci.md) for the GitHub Actions vs. local verification split, and [docs/development-history.md](docs/development-history.md) for M0–M7 milestone records.

## Workflow

1. Follow [docs/installation.md](docs/installation.md) to preserve reference fixture line endings (`core.autocrlf=false`) and initialize the pinned Git submodules under `external/`, then check the reference pins with `cargo xtask verify`.
2. Review project goals and scope in [INTENT.md](INTENT.md), roadmap and current status in [PLAN.md](PLAN.md), and architecture guidance in [docs/architecture.md](docs/architecture.md). Establish semantics from the pinned `external/pkl` `0.32.0` sources and tests.
3. Keep changes focused and explain their effects on observable behavior. Do not add placeholders that return success for unimplemented APIs or add Java fallbacks to production Rust evaluation paths.
4. Update [CHANGELOG.md](CHANGELOG.md) for notable user-facing changes and update [NOTICE](NOTICE) / [THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md) whenever adding or modifying third-party dependencies or embedded assets.
5. Run checks appropriate to the changed area (see below) and record exact results, provenance, and any unverified scope. See [SECURITY.md](SECURITY.md) for reporting vulnerabilities privately.

## Branching & Tagging

Follow the full [Versioning, Branch & Tag Policy](docs/versioning.md):

- **Branches**: Develop on short-lived topic branches (`feat/*`, `fix/*`, `perf/*`, `docs/*`, `ci/*`, `chore/*`, or `baseline/pkl-<X.Y.Z>` for upstream baseline upgrades) and open pull requests against `main` (or `release/<MAJOR>.<MINOR>` for maintenance backports). Never force-push to `main` or `release/*`.
- **CI triggers**: [.github/workflows/ci.yml](.github/workflows/ci.yml) runs on `push` to `main`/`release/**` and on `pull_request` targeting `main`/`release/**`, so unmerged feature branches do not consume duplicate Actions runs before a PR is opened.
- **Release tags**: Only maintainers create annotated `v*` tags (`git tag -a vX.Y.Z[-rc.N] -m "..."`) on verified `main` or `release/*` commits after `python3 tools/check_release_version.py --tag vX.Y.Z[-rc.N]` passes. Published tags are immutable and must never be moved or deleted.

## Implementation principles

- Separate the shared pure-Wasm engine (`crates/pkl-runtime`, `crates/pkl-formatter`) from platform-specific Host capabilities (`crates/pkl-host-native`, `crates/pkl-wasm`, `crates/pkl-napi`). Keep evaluation semantics and error formats consistent across native, Wasm, and N-API targets.
- Maintain `unsafe_code = "deny"` across the workspace and keep `@openedges/pkl-wasm` (`pkl_wasm_bg.wasm`) within the `2.50 MiB` (`2,621,440 B`) gzip-9 budget.
- Support compatibility changes with actual pinned Pkl `0.32.0` oracle results (Temurin `21.0.13.1+2-LTS` + `pkl-tools-0.32.0.jar`, and Official Apple Pkl `0.32.0` `native-image` for CLI/server/`pkldoc` performance) and reproducible inputs. Compare raw bytes without trimming, sorting, or reserialization.
- Before adding dependencies, review their purpose, `wasm32-unknown-unknown` support, licensing, and binary/memory impact. Track `Cargo.lock` and always pass `--locked` in CI and verification commands.
- **Documentation language rules**: Write standard project documentation in English (`README.md`, `AGENTS.md`, `CONTRIBUTING.md`, `INTENT.md`, `TESTING.md`, `docs/*.md`). Write planning documents in Korean (`PLAN.md`, implementation plans, task plans). Preserve code, commands, identifiers, file paths, and upstream reference files unchanged.

## Verification and change descriptions

- **GitHub Actions** runs minimal Ubuntu checks and packaging smoke on `push`/`pull_request` ([ci.yml](.github/workflows/ci.yml)), and cross-platform packaging/publishing on `v*` tags or manual dispatch ([release.yml](.github/workflows/release.yml)).
- **Local / dedicated verification** covers the full 378-target Rust workspace, all 59 active (+ 75 historical) Python tests, all 41 platform Node.js tests, standalone `@openedges/pkl-wasm` and `@openedges/pkl-napi` package tests, M0–M6 Java differential gates, and M7 fuzz/soak/benchmark/JS suites via `tools/local_verify.py`:

```sh
# Fast default local gate (378-target Rust workspace + 12 manifests/oracles + 59 Python + 41 JS tests + packages)
python3 tools/local_verify.py --preset quick

# List all M0-M7 verification suites and presets
python3 tools/local_verify.py --list
```

Include the following in change descriptions:

- The problem being addressed and the observable behavior change.
- Relevant Pkl `0.32.0` source, oracle, or benchmark evidence.
- Exact checks run, provenance (local vs. hosted CI), and outcomes.
- Known mismatches, unexecuted checks, and remaining platform gaps.

Keep reference version updates separate from ordinary feature changes. Update submodule gitlinks, `references.lock`, `oracle.lock.json`, manifests, and documentation together. Never modify files under `external/` as a substitute for implementation changes.
