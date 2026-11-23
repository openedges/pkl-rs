# Development History

This document preserves the chronological milestone history (M0–M7), recorded verification checkpoints, key scope/contract decisions, and canonical evidence archive links for `pkl-rs`.

For user-facing installation, usage, and release notes, see [README.md](../README.md), [docs/installation.md](installation.md), and [CHANGELOG.md](../CHANGELOG.md). For active roadmap and Korean planning notes, see [PLAN.md](../PLAN.md).

---

## Milestone Checkpoints (M0–M7)

Historical checkpoint reports retain their original source epochs, sealed raw evidence archives, and preserved exceptions without overwriting earlier runs:

| Milestone | Scope | Canonical Report | Recorded Outcome (Local macOS `arm64`) |
| --- | --- | --- | --- |
| **M0** | Pinned `0.32.0` source/corpus/oracle tooling & GraalVM CE `25.0.1` baseline | [reports/m0/README.md](../reports/m0/README.md) | 11,624 inventory items, 1,112 command cases, 6-workload M0 GraalVM baseline |
| **M1** | Shared Rust frontend/VM/heap/Host across native, Node, and browser Wasm | [reports/m1/README.md](../reports/m1/README.md) | 6,078/6,078 cross-target parity, 92/92 Node & Chromium scenarios, Graal trace audit |
| **M2** | Whole-language semantics (475 requirements, 4,575 ready cases) | [reports/m2/stage161-m2-closure/README.md](../reports/m2/stage161-m2-closure/README.md) | 4,574 pass / 1 `oracle-error` (`error-cycle`) / 0 mismatch; Protobuf 240+8 (18 upstream `REFERENCE-NONPASS`) |
| **M3** | All 23 `pkl:*` stdlib modules, 542 builtins, 9 renderers, `output.files`, `PklBinary` | [reports/m3/README.md](../reports/m3/README.md) | 1,781/1,781 mechanical coverage (`failure_count: 0`), 118/118 3-lane differential |
| **M4** | Host I/O, HTTP/HTTPS/CA/proxy, `0x26–0x32` external readers, `PklProject`, Pantry | [reports/m4/README.md](../reports/m4/README.md) | 429/429 5-lane offline differential (all 29 Pantry packages in dev + packaged form) |
| **M5** | Public Rust API (`crates/pkl`), `pkl` CLI, REPL, server, formatter, `pkldoc` | [reports/m5/README.md](../reports/m5/README.md) | 142/142 5-lane differential (`1,767/1,767` `pkldoc` files + `13/13` symlinks byte-identical) |
| **M6** | Product `@pkl-rs/wasm` & `@pkl-rs/napi` bindings and clean packaging | [reports/m6/README.md](../reports/m6/README.md) | 80/80 executable PASS + 6/6 explicit `capability-blocked`, 4-way raw-byte parity |
| **M7** | Performance/peak RSS parity, memory soak, differential fuzzing, release matrix | [reports/m7/README.md](../reports/m7/README.md) | 18/18 soak `bounded`, 215/215 fuzz PASS, 15/15 Official `native-image` `<= 1.00x` (`2026-11-23`), `release_ready: false` |

---

## Key Recorded Engineering Decisions

### 1. Explicit Scope Exclusions (`2026-11-20`)

Recorded in [INTENT.md](../INTENT.md) and reflected in `compatibility/m7-release.json`:
- Java/Kotlin code generation (`pkl-codegen-java`, `pkl-codegen-kotlin`) and a JVM public API compatibility adapter are excluded from required product deliverables.
- Upstream `java_api`, `kotlin_api`, and codegen entries in `compatibility/inventory.json` and `compatibility/corpus.json` remain reference census records only.

### 2. Browser `@pkl-rs/wasm` Performance & Memory Contract (`2026-11-22`)

Recorded in [INTENT.md](../INTENT.md), [TESTING.md](../TESTING.md), and `benchmarks/m7_workloads.json`:
- Multi-process browser engines carry a fixed 760–1,180 MiB empty-page process-tree baseline regardless of Wasm allocation.
- Browser `@pkl-rs/wasm` workloads gate memory on **Wasm linear memory (`<= 0.15x` Java peak RSS)**, **JS heap (`<= 64 MiB`)**, and **compressed `.wasm` binary size (`<= 2.50 MiB` gzip-9)** while reporting full multi-process browser-tree RSS transparently.
- Execution time gates at `<= 1.00x` Java for Chromium/WebKit and Firefox `eval-text`, with an explicit SpiderMonkey boundary exception (`<= 1.50x`) for Firefox `typed-conversion`.
- `[profile.wasm-release]` (`opt-level = "s"`, `lto = "fat"`, `codegen-units = 1`, `panic = "abort"`, `strip = "symbols"`) plus RFC 3629 short UTF-8 fast decoding reduced `pkl_wasm_bg.wasm` to `8,022,629 B` raw / `2,619,729 B` (`2.498 MiB`) gzip-9 without requiring external `wasm-opt`.

### 3. Official Apple Pkl `0.32.0` `native-image` Primary CLI Baseline (`2026-11-23`)

Recorded in [oracle.lock.json](../oracle.lock.json), [docs/oracle.md](oracle.md), and [TESTING.md](../TESTING.md):
- All 15 CLI, server, and `pkldoc` workloads (`cold-cli` 14 + `steady-server` 1) use SHA-256 pinned Official Apple Pkl `0.32.0` `native-image` (`pkl-macos-aarch64` `94.7 MiB`, `pkldoc-macos-aarch64` `94.7 MiB`) as their primary `<= 1.00x` execution time and `<= 1.00x` `os.wait4` peak RSS gate.
- On local macOS `arm64`, `pkl-rs` achieved **15 / 15 PASS** (`0.055x–0.935x` time, `0.136x–0.662x` peak RSS; Rust binaries `pkl` `15 MiB`, `pkldoc` `14 MiB`) with 100% raw-byte parity across all 15 workloads (`6/6` Core workloads also pass vs archived M0 GraalVM CE `25.0.1`).
- The remaining 19 embedded API and JS/Wasm workloads continue to gate against pinned Temurin 21 JVM (since Official Apple `native-image` ships CLI executables only).

### 4. Lean GitHub Actions vs. Local Verification Split (`2026-11-23`)

Recorded in [docs/ci.md](ci.md) and [tools/local_verify.py](../tools/local_verify.py):
- [.github/workflows/ci.yml](../.github/workflows/ci.yml) runs minimal Ubuntu checks and packaging smoke (`check-and-test`, `package-smoke`).
- [.github/workflows/release.yml](../.github/workflows/release.yml) builds and publishes cross-platform CLI archives, `@pkl-rs/wasm`, 7-platform `@pkl-rs/napi`, and `SHA256SUMS.txt` only on `v*` tags or manual dispatch.
- [tools/local_verify.py](../tools/local_verify.py) executes all 18 full verification suites (378 Rust workspace targets, 12 manifests/oracles, 59 active + 75 historical Python test modules, 41 platform Node.js test files / 384 tests, standalone packages, M0–M6 Java differential suites, Playwright cross-browser matrices, and M7 fuzz/soak/benchmark suites).

### 5. Distribution Readiness & Release Documentation Restructuring (`2026-11-23`)

- Unified workspace crate versions (`Cargo.toml` `0.1.0 -> 0.32.0`) with `@pkl-rs/wasm` (`0.32.0`), `@pkl-rs/napi` (`0.32.0`), and CLI version strings (`0.32.0`).
- Bundled `LICENSE`, `NOTICE`, and `THIRD-PARTY-NOTICES.md` into all CLI release archives and npm tarballs for Apache-2.0 §4 compliance.
- Separated user/release documentation ([README.md](../README.md), [docs/installation.md](installation.md), [docs/versioning.md](versioning.md), [CHANGELOG.md](../CHANGELOG.md), [SECURITY.md](../SECURITY.md), [THIRD-PARTY-NOTICES.md](../THIRD-PARTY-NOTICES.md)) from historical milestone tracking (this document and `reports/m0/`–`reports/m7/`).

---

## Remaining Unverified Scope for GA (`release_ready: false`)

- Hosted GitHub Actions execution of [ci.yml](../.github/workflows/ci.yml) and [release.yml](../.github/workflows/release.yml).
- Non-macOS `arm64` target matrix verification (`darwin-x64`, `linux-x64-gnu`, `linux-arm64-gnu`, `linux-x64-musl`, `linux-arm64-musl`, `win32-x64-msvc`).
- Post-optimization Firefox and WebKit browser benchmark captures (`m7-js`) and unified `--baseline both` 34-workload release verification (`m7_manifest.py --verify-release`).
- Pre-GA crates.io publishing prerequisite (`vendor/java_regex`) and OpenJDK `uniName.dat` table regeneration directly from Unicode Consortium `UnicodeData.txt` (see [docs/versioning.md](versioning.md) and [THIRD-PARTY-NOTICES.md](../THIRD-PARTY-NOTICES.md)).
