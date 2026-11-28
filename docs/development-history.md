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
| **M5** | Public Rust API (`crates/pkl`), `pkl` CLI, REPL, server, formatter, `pkldoc` | [reports/m5/README.md](../reports/m5/README.md) | 153/153 5-lane differential (`1,767/1,767` `pkldoc` files + `13/13` symlinks byte-identical) |
| **M6** | Product `@openedges/pkl-wasm` & `@openedges/pkl-napi` bindings and clean packaging | [reports/m6/README.md](../reports/m6/README.md) | 80/80 executable PASS + 6/6 explicit `capability-blocked`, 4-way raw-byte parity |
| **M7** | Performance/peak RSS parity, memory soak, differential fuzzing, release matrix | [reports/m7/README.md](../reports/m7/README.md) | 18/18 soak `bounded`, 215/215 fuzz PASS, 15/15 Official `native-image` `<= 1.00x` (`0.073x–0.761x` time, `0.130x–0.551x` RSS), `release_ready: false` |

---

## Key Recorded Engineering Decisions

### 1. Explicit Scope Exclusions (`2026-11-20`)

Recorded in [INTENT.md](../INTENT.md) and reflected in `compatibility/m7-release.json`:
- Java/Kotlin code generation (`pkl-codegen-java`, `pkl-codegen-kotlin`) and a JVM public API compatibility adapter are excluded from required product deliverables.
- Upstream `java_api`, `kotlin_api`, and codegen entries in `compatibility/inventory.json` and `compatibility/corpus.json` remain reference census records only.

### 2. Browser `@openedges/pkl-wasm` Performance & Memory Contract (`2026-11-22`)

Recorded in [INTENT.md](../INTENT.md), [TESTING.md](../TESTING.md), and `benchmarks/m7_workloads.json`:
- Multi-process browser engines carry a fixed 760–1,180 MiB empty-page process-tree baseline regardless of Wasm allocation.
- Browser `@openedges/pkl-wasm` workloads gate memory on **Wasm linear memory (`<= 0.15x` Java peak RSS)**, **JS heap (`<= 64 MiB`)**, and **compressed `.wasm` binary size (`<= 2.50 MiB` gzip-9)** while reporting full multi-process browser-tree RSS transparently.
- Execution time gates at `<= 1.00x` Java for Chromium/WebKit and Firefox `eval-text`, with an explicit SpiderMonkey boundary exception (`<= 1.50x`) for Firefox `typed-conversion`.
- `[profile.wasm-release]` (`opt-level = "z"`, `lto = "fat"`, `codegen-units = 1`, `panic = "abort"`, `strip = "symbols"`) plus RFC 3629 short UTF-8 fast decoding reduced `pkl_wasm_bg.wasm` to `6,703,884 B` raw / `2,124,981 B` (`2.027 MiB`) gzip-9 without requiring external `wasm-opt`.

### 3. Official Apple Pkl `0.32.0` `native-image` Primary CLI Baseline (`2026-11-23` / `2026-11-24`)

Recorded in [oracle.lock.json](../oracle.lock.json), [docs/oracle.md](oracle.md), and [TESTING.md](../TESTING.md):
- All 15 CLI, server, and `pkldoc` workloads (`cold-cli` 14 + `steady-server` 1) use SHA-256 pinned Official Apple Pkl `0.32.0` `native-image` (`pkl-macos-aarch64` `94.7 MiB`, `pkldoc-macos-aarch64` `94.7 MiB`) as their primary `<= 1.00x` execution time and `<= 1.00x` `os.wait4` peak RSS gate.
- On local macOS `arm64` (`2026-11-24` post-optimization full 34-workload capture `target/m7/release-032-final/bench2`), `pkl-rs` achieved **15 / 15 PASS** vs Official Apple Pkl `0.32.0` `native-image` (**`0.073x–0.761x` time, `0.130x–0.551x` peak RSS**; Rust binaries `pkl` `15 MiB`, `pkldoc` `14 MiB`) with 100% raw-byte parity across all 15 workloads (`18/18 PASS` vs archived M0 GraalVM CE `25.0.1` Core-6; `33/34 PASS` vs live Temurin `21.0.13.1+2-LTS`, with `wasm-node-typed-conversion` at `0.386x` time / `5.9 MiB` Wasm linear memory and `1.111x` Node v26.13.1 whole-process V8 RSS).
- The remaining 19 embedded API and JS/Wasm workloads continue to gate against pinned Temurin 21 JVM (since Official Apple `native-image` ships CLI executables only).

### 4. Lean GitHub Actions vs. Local Verification Split (`2026-11-23`)

Recorded in [docs/ci.md](ci.md) and [tools/local_verify.py](../tools/local_verify.py):
- [.github/workflows/ci.yml](../.github/workflows/ci.yml) runs minimal Ubuntu checks and packaging smoke (`check-and-test`, `package-smoke`).
- [.github/workflows/release.yml](../.github/workflows/release.yml) builds and publishes cross-platform CLI archives, `@openedges/pkl-wasm`, 7-platform `@openedges/pkl-napi`, and `SHA256SUMS.txt` only on `v*` tags or manual dispatch.
- [tools/local_verify.py](../tools/local_verify.py) executes all 18 full verification suites (378 Rust workspace targets, 12 manifests/oracles, 59 active + 75 historical Python test modules, 41 platform Node.js test files / 384 tests, standalone packages, M0–M6 Java differential suites, Playwright cross-browser matrices, and M7 fuzz/soak/benchmark suites).

### 5. Distribution Readiness & Release Documentation Restructuring (`2026-11-23`)

- Unified workspace crate versions (`Cargo.toml` `0.1.0 -> 0.32.0`) with `@openedges/pkl-wasm` (`0.32.0`), `@openedges/pkl-napi` (`0.32.0`), and CLI version strings (`0.32.0`).
- Bundled `LICENSE`, `NOTICE`, and `THIRD-PARTY-NOTICES.md` into all CLI release archives and npm tarballs for Apache-2.0 §4 compliance.
- Separated user/release documentation ([README.md](../README.md), [docs/installation.md](installation.md), [docs/versioning.md](versioning.md), [CHANGELOG.md](../CHANGELOG.md), [SECURITY.md](../SECURITY.md), [THIRD-PARTY-NOTICES.md](../THIRD-PARTY-NOTICES.md)) from historical milestone tracking (this document and `reports/m0/`–`reports/m7/`).

### 6. Pre-GA `v0.32.0` Hardening, Unicode Name Packing, API Unification & Sealed Local Checkpoints (`2026-11-26` – `2026-11-28`)

- Completed five full pre-GA code, security, cross-platform, stability, API, and performance audit rounds on pinned **`rustc 1.98.1`**, preserving strict Apple Pkl `0.32.0` raw-byte compatibility throughout (including `ResourceReaders.FileResource` non-`rootDir` symlink semantics, `PackageResolvers.java` `https:` `allowedResources` checks on `package://` fetches, `Lexer`/`AstBuilder` uppercase radix handling, CRLF `StringContinuation`, JDK 21 `Pattern.hasSupplementary`/`NfcCharProperty.study()` parity, `Builder.java:1451` custom-delimited import sorting, and `DocGenerator.kt:347` UTF-16 prefix slicing).
- Highlights across Rounds 1–5: build-time front-coding + RLE compression (`1,460 KB` → `354 KB`, `-1.11 MB` raw) for `\N{...}` Unicode name tables, V8 Liftoff Wasm outlining (`182.8 KiB` → `34.8 KiB` max function size), opt-in `--wasm-opt` gating after 5-way Binaryen 133 A/B benchmarking (`173.4 → 347.8–350.6 MiB` V8 RSS regression avoided), JS/Wasm columnar `viewCache` + lazy `PklObject`, clean-room Unicode 15.0.0 `string_lower` (`0` mismatches across `8,912,896` + Greek Final Sigma cases), full `@openedges/pkl-wasm` / `@openedges/pkl-napi` JS/TS API unification, real host OS/arch `PlatformMetadata` detection, Windows drive-path URI & symlink/reparse-point portability, POSIX `\` `rootDir` confinement in both Rust and Wasm `VirtualFileSystem` (`%5c`), Wasm `package://` `allowedResources` & `http:`/`https:`-only `packageZipUrl` enforcement, N-API `projectDir` + custom `securityManager` fail-closed handling, `CustomSecurityChecker` enforcement (`env:`/`prop:` globs, `pkl:*`, canonicalized symlinks), full raw HTML escaping in `pkldoc` Markdown, non-poisoning `Vm::cancel()`, pointer-memoized `TypeCheck::Shared` union DAG & generic constrained type equivalence budgets, `List.repeat` chain equality & `String.chars` OOM guards, zero-allocation borrowed `java_regex` continuation stack + inline-flag / zero-width / split-surrogate `looking_at` bounds, strict weak/total-order `pkl-formatter` `compare_natural`, iterative `PklValue`/`FormatNode`/`Expr` traversals, `pkl-server` live-ID-preserving cancelled-callback eviction, `import-graph` warm-query/lazy-cancel/pure-host fast paths, and moving the internal `pkl_m4_cli` harness from `crates/pkl-host-native/src/bin/` into `crates/pkl-host-native/examples/`.
- Retained earlier Round 2 (`target/ga-0.32.0-r2/m7-sealed`), Round 3 (`target/ga-0.32.0-r3/m7-sealed`), Round 4 (`target/ga-0.32.0-r4/m7-sealed`), and the initial Round 5 attempt (`target/ga-0.32.0-r5/m7-sealed`, preserved when P0-2 exposed missing `"https:"` in `tests/fixtures/m7/soak/run_wasm_soak.mjs`) and captured the final Round 5 atomic sealed local verification bundle (`target/ga-0.32.0-r5/m7-sealed-v2`, macOS `arm64`, pinned `rustc 1.98.1` — distinct from hosted CI):
  - `pkl_wasm_bg.wasm`: **`5,698,133 B` raw / `1,960,812 B` (`1.870 MiB`) gzip-9** (`<= 2.50 MiB` budget, `wasm_opt_applied: false`); `wasm-node` linear memory `4.81 MiB` (`typed-conversion`) / `8.44 MiB` (`eval-text`).
  - Benchmarks (`--profile full --baseline both --verify-strict` Exit `0`): **15 / 15 PASS** vs. Official Apple Pkl `0.32.0` `native-image` (`0.075x–0.835x` time, `0.110x–0.552x` peak RSS), **34 / 34 PASS** vs. live Temurin `21.0.13.1+2-LTS` (`0.012x–0.858x` time, `0.020x–0.968x` non-browser RSS, including `import-graph:persistent` `0.97 us` [`0.858x`], `import-graph:fresh-evaluator` `198.97 us` [`0.847x`], `stdlib-collections:fresh-evaluator` `103.76 us` [`0.378x`], and `wasm-node-typed-conversion` `8.17 us` [`0.254x`] / `0.968x`), **18 / 18 PASS** vs. archived M0 GraalVM CE `25.0.1` (`0.011x–0.894x` time, `0.016x–0.059x` RSS; `import-graph:persistent` `0.894x`, `import-graph:fresh-evaluator` `0.879x`).
  - 3-Browser JS/Wasm analysis (`--profile full`): **10 / 10 PASS** (`napi-node` `0.258x`/`0.485x`, `wasm-node` `0.416x`/`0.257x` [RSS `0.579x`/`0.970x`], `chromium` `0.420x`/`0.254x`, `firefox` `0.570x`/`0.425x`, `webkit` `0.346x`/`0.201x`) + 3-browser functional smoke PASS.
  - Stability & Compatibility: **18 / 18 bounded** soak (`--verify-strict` Exit `0`, all 6 Wasm linear memory `second_half_delta == 0`), **215 / 215 PASS** fuzz (`--verify-strict` Exit `0`), M2 `4,574/4,574` language + `LanguageSnippetTests` + `package-project-eval` + `language-jvm-classpath-output` (`3,585` `api-string` + `32` `module-eval` + `888` `LanguageSnippetTests` + `69` `package-project-eval`/`jvm-classpath`, `0` Rust mismatches, `1` known `error-cycle` `oracle-error`, plus `2,782/2,782` `m2_class_lifecycle` and `m2_identifier_tables` PASS via pure-Python Gradle-free reproducible ZIP fixtures in `tools/m2_package_fixtures.py`; `target/ga-0.32.0-r5/compat-m2` + `compat-m2-snippets` + `compat-m2-remaining`), M3 `118/118` (+ `1,781` stdlib probe), M4 `429/429` (via `--example pkl_m4_cli`), M5 `153/153` (`target/ga-0.32.0-r5/compat-m3-m6`), M6 `80/80 + 6/6` + 3-browser smoke (`target/ga-0.32.0-r5/compat-m6`), unified `m7_manifest.py --local-checkpoint` **Exit `0`** (`release-audit-local.json`, `verified_platforms: ["browser-chromium", "browser-firefox", "browser-webkit", "darwin-arm64"]`; global `release-audit-global.json` Exit `1` / `release_ready: false` with 7 cross-OS hosted-CI targets remaining).

---

## Remaining Unverified Scope for GA (`release_ready: false`)

- Hosted GitHub Actions execution of [ci.yml](../.github/workflows/ci.yml) and [release.yml](../.github/workflows/release.yml) (`hosted-ci` attestation).
- Cross-OS target matrix verification (`darwin-x64`, `linux-x64-gnu`, `linux-arm64-gnu`, `linux-x64-musl`, `linux-arm64-musl`, `win32-x64-msvc`), built and attested by `.github/workflows/release.yml` on tag `v0.32.0`.

*(Resolved on `2026-11-23`: regenerated all 12 Unicode 15.0.0 regex tables directly from official Unicode Consortium UCD 15.0.0 files under `Unicode-3.0`, removed all OpenJDK `uniName.dat` / GPLv2 files, absorbed the UTF-16 Java regex engine directly into `crates/pkl-runtime/src/java_regex/` removing `vendor/java_regex` and `[patch.crates-io]`, and replaced Apple Pkl `pkldoc` favicons / CLI banners / HTTP User-Agent with original `pkl-rs` branding.)*
