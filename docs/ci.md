# CI, Release Packaging, and Local Verification

GitHub Actions is split into two focused workflows to keep routine CI costs low while keeping release distribution automated, and all expensive differential, browser-matrix, fuzz, soak, and benchmark verification runs in a local or dedicated verification environment via [tools/local_verify.py](../tools/local_verify.py).

## GitHub Actions scope (minimal tests, packaging, and release distribution)

1. **[ci.yml](../.github/workflows/ci.yml)** (`push`, `pull_request`, `workflow_dispatch` — **2 Ubuntu `ubuntu-24.04` jobs only**, zero macOS/Windows runners):
   - `check-and-test`: `cargo xtask verify`, `cargo fmt --all -- --check`, workspace + `java_regex` + `wasm32` `clippy -D warnings`, portable `wasm32-unknown-unknown` build checks, all 12 milestone manifest/contract `--check` verifiers (offline, no Java required), `tools/tests/test_local_verify.py`, and fast Rust unit/product-crate smoke tests (`xtask`, `pkl-formatter`, `pkl-server`, `pkl-repl`, `pkl-cli`, `pkl-doc`, `pkl-wasm`, `pkl-host-native`, and `pkl-runtime --lib`).
   - `package-smoke`: Builds and smoke-tests `@pkl-rs/wasm` (`[profile.wasm-release]`, enforcing `pkl_wasm_bg.wasm` gzip-9 `<= 2.50 MiB` (`2,621,440 B`) plus `npm pack` clean-directory install), builds and smoke-tests `@pkl-rs/napi`, and builds and smoke-tests release `pkl` and `pkldoc` CLI binaries.
2. **[release.yml](../.github/workflows/release.yml)** (**ONLY on version tags `v*` or manual `workflow_dispatch`** — never runs on ordinary `push`/`pull_request`):
   - `package-linux-portable` (`ubuntu-24.04` `x86_64`, `ubuntu-24.04-arm` `aarch64`): Builds glibc `<= 2.28` (`manylinux_2_28`, checked via `tools/check_linux_glibc.py`) and static `musl` (`alpine:3.21`, verified zero `INTERP`) `pkl`/`pkldoc` binaries and Linux `@pkl-rs/napi` addons, then smoke-tests across Debian 12, Rocky Linux 9, Ubuntu 22.04, and Alpine 3.21.
   - `package-macos-windows` (`macos-15`, `macos-15-intel`, `windows-latest`): Builds and smoke-tests release `pkl`/`pkldoc` archives and `@pkl-rs/napi` addons (`darwin-arm64`, `darwin-x64`, `win32-x64`).
   - `package-wasm` (`ubuntu-24.04`): Builds, gzip-checks (`<= 2.50 MiB`), tests, and packs `@pkl-rs/wasm`.
   - `assemble-and-release` (`ubuntu-24.04`): Collects all CLI archives, `@pkl-rs/wasm` tarball, and all 7 platform `.node` addons into a single multi-platform `@pkl-rs/napi` tarball, computes `SHA256SUMS.txt`, uploads `release-bundle` (30-day retention), publishes a GitHub Release on `v*` tags (or manual `publish_github_release=true`), and optionally publishes npm tarballs only when `publish_npm=true` with `NPM_TOKEN` configured.

All GitHub Actions third-party steps remain commit-SHA pinned, and `ci.yml`/`release.yml` contracts are checked automatically by [tools/tests/test_local_verify.py](../tools/tests/test_local_verify.py).

## Local / dedicated verification environment (`tools/local_verify.py`)

Expensive verification suites — full 378-target Rust workspace tests, M0–M6 Java 0.32.0 differential suites (including M2 4,575-case replay and M4 all-29 Pantry packages), Playwright cross-browser matrices (Chromium/Firefox/WebKit), M7 seeded differential fuzzing, 18-pair memory reclamation soak, and 34-workload benchmarks against Official Apple Pkl 0.32.0 `native-image` and pinned Temurin 21 — run outside GitHub Actions via [tools/local_verify.py](../tools/local_verify.py):

```sh
# List all available suites and presets
python3 tools/local_verify.py --list

# Preview commands without executing
python3 tools/local_verify.py --preset full --dry-run

# Quick local gate (full 378-target Rust workspace + all 12 manifests/oracles/Python tests)
python3 tools/local_verify.py --preset quick

# Compatibility differential suites (M0-M6; add --full-corpus for M2 4,575 + M4 Pantry all-5)
python3 tools/local_verify.py --preset compat

# M7 fuzz + soak + benchmark (official native-image or both) + JS/Wasm analysis
python3 tools/local_verify.py --suite m7-fuzz --suite m7-soak --suite m7-bench --baseline native-image
```

Each invocation writes to a fresh timestamped directory under `target/local-verify/<UTC-timestamp>/` (or `--output-root <dir>`) with a `summary.json` marked `provenance: "local-verification-environment-not-hosted-ci"`, preserving all raw evidence and failures without relabeling local runs as hosted CI.

## Local checkpoint acceptance

Hosted CI is an automation and packaging/release pipeline, not the sole transport for compatibility or performance evidence. A checkpoint can close with recorded local verification of its applicable development contract. Identify the exact commit, source/build identities, oracle settings, commands, host and runtime versions, raw results, known failures, and unexecuted platform coverage. Audit sealed local archives independently when accepting retained results. Do not repeat unchanged full acquisitions merely to imitate a hosted workflow.

If a remote workflow is not run or produces no artifact, retain that gap honestly and mark remote checks unexecuted. Local archive integrity and exact-commit source checks provide their own provenance; all required target environments still need execution before a `release_ready: true` claim.

## Historical M0/M1 CI verification record

Initialize the pinned submodules as described in [README.md](../README.md). Run the workspace and Python checks in [TESTING.md](../TESTING.md). Install the exact reference JDK before following [the oracle bootstrap](oracle.md) and the record/compare/check commands in [the comparison contract](compatibility-tooling.md). New runs require fresh evidence directories. Candidate replay must use the same absolute workspace path as its Java baseline so error locations can be compared without rewriting bytes.

Recorded evidence is reviewable on another machine, but its absolute diagnostic paths and native JDK binary hashes belong to the original host. Generate a fresh Java baseline in the candidate's controlled environment for a formal comparison. Do not normalize historical output to make a different host appear identical.

The local M0 checks passed on the recorded macOS host. After authenticated access became
available, [run 39364517355](https://github.com/luuvish/pkl-rs/actions/runs/39364517355)
was verified for the final M0 code commit `2164ffa`. All four jobs succeeded: the three
workspace platforms and Linux Java tooling. All required steps ran successfully; only
the manual full-corpus step was intentionally skipped on this push event.

The downloaded artifact ZIP matches GitHub's SHA-256 digest. Its three reports and raw
captures passed completeness/integrity checks, and all seven Java replay cases were
compared again directly from the downloaded bytes. Exact JDK properties match the
reference lock. The [CI summary](../reports/m0/ci-summary.json) and
[retained evidence](../reports/m0/ci-evidence.tar.gz) preserve run/step metadata, logs,
the original artifact ZIP, and extracted raw captures beyond GitHub's retention period.
This verifies workspace and Java tooling behavior; the full corpus, Pantry investigation,
performance acquisition, and Rust evaluation are outside this CI run's scope.

The M1 code commit `e9ee8d5` passed all five jobs in
[run 39372369602](https://github.com/luuvish/pkl-rs/actions/runs/39372369602),
including the real Node/Chromium experiment. Its downloaded artifact ZIP matches
GitHub's SHA-256 digest. Recomputed raw results confirm 15 frontend matches,
five Java Host suites, six runtime value matches and three diagnostic byte
failures, and 6,078 shared-engine primitive results with the same two Java regex
backend failures on every target. All 12 required platform scenarios ran in
each JS environment. Captured source snapshots match the exact checked commit.
The [M1 CI summary](../reports/m1/ci-summary.json) and
[retained raw archive](../reports/m1/ci-evidence.tar.gz) preserve the artifact,
logs, verification procedure and result beyond GitHub artifact retention.

The closing code commit `6a5e4b5` passed all five jobs in
[run 39374668362](https://github.com/luuvish/pkl-rs/actions/runs/39374668362).
The added lifecycle step compares two complete Java/Rust observation protocols,
preserves four diagnostic payload differences, and records Java's nested new-read
cache mutation failure as a separate negative observation. Fourteen scenarios
ran in both Node and Chromium, including live nested-operation cleanup. The
downloaded ZIP digest, exact committed sources, raw results and Java identity were
reverified. The [closing summary](../reports/m1/ci-closing-summary.json) and
[archive](../reports/m1/ci-closing-evidence.tar.gz) retain the original artifact,
logs and verification procedure. Unpacked Cargo intermediates are excluded from
the archive; the original artifact ZIP and all evidence inputs remain retained.

The Graal investigation and CI shell correction are committed as `bb80685` and
`4cd9c08`. All five jobs in
[run 39378401720](https://github.com/luuvish/pkl-rs/actions/runs/39378401720)
passed. Linux verified the retained macOS acquisition's nine diagnostic cases,
then reran the bounded frontend, runtime, Host, lifecycle, Node and Chromium
experiments. The artifact digest, exact committed sources, and raw outcomes
were independently verified after download. The [final CI summary](../reports/m1/ci-graal-summary.json)
and [archive](../reports/m1/ci-graal-evidence.tar.gz) retain the original ZIP,
workflow logs, and verification procedure. The Graal step verifies stored
compilation evidence; it does not acquire a new Linux performance baseline.
