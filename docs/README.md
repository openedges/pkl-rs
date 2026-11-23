# Documentation Index

Documentation in `pkl-rs` is organized into three tiers so end users, contributors, and release maintainers can find the exact information they need without sifting through historical engineering logs.

---

## 1. User & Release Documentation

| Document | Audience | Purpose |
| --- | --- | --- |
| [README.md](../README.md) | End users | Project overview, quick start (CLI, Rust, Node.js, Wasm), compatibility summary |
| [docs/installation.md](installation.md) | End users / Ops | Prebuilt binaries, checksum verification, shell completions, npm packages, source builds |
| [docs/versioning.md](versioning.md) | Users / Maintainers | Unified `0.32.0` versioning policy, upstream upgrade rules, release checklist |
| [CHANGELOG.md](../CHANGELOG.md) | Users / Maintainers | User-facing release notes and pre-release boundaries |
| [SECURITY.md](../SECURITY.md) | Users / Security | Vulnerability disclosure and Pkl security model / trust boundaries |
| [LICENSE](../LICENSE) | Legal / Distributors | Apache License, Version 2.0 |
| [NOTICE](../NOTICE) | Legal / Distributors | Required Apache-2.0 attribution notices |
| [THIRD-PARTY-NOTICES.md](../THIRD-PARTY-NOTICES.md) | Legal / Distributors | Complete embedded asset, font, vendored regex, and Rust crate license inventory |
| [packages/pkl-wasm/README.md](../packages/pkl-wasm/README.md) | Web / Node users | `@pkl-rs/wasm` npm package guide and Wasm capability boundaries |
| [packages/pkl-napi/README.md](../packages/pkl-napi/README.md) | Node.js users | `@pkl-rs/napi` npm package guide and supported native platforms |

---

## 2. Architecture, Verification & Contributor Guides

| Document | Audience | Purpose |
| --- | --- | --- |
| [CONTRIBUTING.md](../CONTRIBUTING.md) | Contributors | Contribution workflow, coding principles, and PR checklist |
| [TESTING.md](../TESTING.md) | Contributors / QA | Complete 6-layer test inventory and compatibility verification contract |
| [docs/ci.md](ci.md) | Maintainers / QA | Minimal GitHub Actions CI, tagged release workflow, and `tools/local_verify.py` (18 suites / 5 presets) |
| [docs/architecture.md](architecture.md) | Contributors | Shared pure-Wasm engine, VM, heap, Host interface, and cross-target design |
| [docs/oracle.md](oracle.md) | Contributors / QA | Bootstrapping pinned Temurin 21 JVM, Official Apple `native-image`, and GraalVM CE oracles |
| [docs/benchmarks.md](benchmarks.md) | Performance / QA | Benchmark harness design and methodology |
| [INTENT.md](../INTENT.md) | Maintainers | Durable project goals, scope exclusions, and performance requirements |
| [AGENTS.md](../AGENTS.md) | Contributors / Agents | Repository engineering and verification rules |

---

## 3. Development History & Historical Milestone Specifications

| Document / Directory | Purpose |
| --- | --- |
| [docs/development-history.md](development-history.md) | Chronological M0–M7 milestone checkpoints, key decisions, and evidence links |
| [PLAN.md](../PLAN.md) | Roadmap, execution plan, and detailed work status (Korean) |
| [reports/m0/](../reports/m0/README.md) – [reports/m7/](../reports/m7/README.md) | Canonical milestone reports and checksummed raw evidence archives |
| [docs/inventory.md](inventory.md), [docs/corpus.md](corpus.md), [docs/compatibility-tooling.md](compatibility-tooling.md), [docs/regressions.md](regressions.md), [docs/workspace-storage.md](workspace-storage.md) | M0 compatibility tooling and corpus manifest design contracts |
| `docs/m1-*.md` (8 files) | Historical M1 architecture prototype and Graal trace analysis notes |
| `docs/m2-*.md` (24 files) | Historical M2 language semantics, stdlib bootstrap, module/resource, and diagnostic design notes |
