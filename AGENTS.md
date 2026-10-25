# Repository instructions

These instructions apply to agents making changes in this repository. Explicit user instructions take precedence.

## Scope and current status

- The compatibility baseline is **Pkl 0.32.0**. Use the sources pinned in `references.lock` and the Git submodules.
- M0 baseline tooling and initial Java evidence are available. M1 adds a bounded shared Rust frontend/VM/heap/Host with native, browser, and Node experiments, not the full evaluator. See `reports/m0/README.md` and `reports/m1/README.md` for scope and known gaps. Do not describe builds, Java replay, or bounded experiment success as complete language/API compatibility or achievement of the performance targets.
- M2 is in progress. Its whole-language inventory and selected raw comparisons are described in `docs/m2-language.md`. Inventory freshness, evidence integrity, and the selected development contract are separate from the strict full completion gate. Preserve known failures and unmapped requirements.
- Read `INTENT.md` for goals and scope, `PLAN.md` for the phase overview, execution order, and work status, and `TESTING.md` for the verification contract. Use `docs/architecture.md` for design guidance. Maintain the roadmap and detailed execution plan together in `PLAN.md`.
- `../pkl-rs.rev1` is reference material. Its implementation and passing test records are not verification results for this project.
- Write standard project documentation in English, including `README.md`, `AGENTS.md`, `CONTRIBUTING.md`, `INTENT.md`, `TESTING.md`, architecture documentation, and API references.
- Write planning documents in Korean, including `PLAN.md`, implementation plans, and task plans. Apply these language rules to new documents and future updates.
- Keep code, commands, identifiers, and file paths unchanged when translating prose. Preserve the original contents of upstream references and compatibility fixtures.

## Compatibility and design

- Differential verification against the Java 0.32.0 oracle determines compatibility. Do not generate expected results from the Rust implementation and claim compatibility.
- Compare output, errors, and generated files as raw bytes. Do not hide differences with `trim`, sorting, reserialization, or partial message comparisons.
- Do not count missing tests, unexecuted checks, unimplemented features, or known failures as passes. Record the reason and scope of temporary exceptions and account for them in release criteria.
- Share the evaluation engine, value model, diagnostics, and renderers across native, Wasm, and N-API targets. Keep platform-specific I/O behind the Host interface.
- Do not add a Java fallback to production evaluation paths. Use Java for reference verification and performance comparisons.
- Check upstream semantics and edge cases when changing name resolution, lazy evaluation, error ordering, string positions, numeric operations, or regular expressions.
- Speed and memory improvements must preserve correct results. Do not hide regressions in required workloads behind average improvements elsewhere.

## Changes and verification

- Recorded local verification is an accepted path for development checkpoints. Do not block implementation or checkpoint closeout solely on hosted CI availability, billing, or artifact transport. Run the applicable checks locally and record the actual host, toolchain, sources, artifacts, raw outcomes, and remaining platform gaps. A hosted CI result and local evidence are distinct provenance; never relabel one as the other. Release platform, compatibility, and performance requirements still apply.
- Do not modify reference checkouts or gitlinks under `external/` unless the task requests a reference version update. Consult the reference sources for evidence, then make implementation changes in this project.
- When updating reference versions, update the gitlinks, `references.lock`, documentation, and verification criteria together.
- Implement only the changes needed for the task. Document features and commands as available only after they are implemented.
- Use the Rust version in `rust-toolchain.toml`. Review `Cargo.lock` when changing dependencies.
- Run checks appropriate to the change. The default checks for Rust code or development configuration changes are:

```sh
cargo xtask verify
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo check -p pkl-runtime --target wasm32-unknown-unknown --locked
```

- Add meaningful regression cases for behavior changes. Do not add tests for simple documentation edits or tests that merely repeat the implementation.
- For Python verification tooling changes, run the relevant tests under `tools/tests` and the affected inventory freshness checks. Use the pinned Java oracle for integration checks; preserve failed, blocked, and inconclusive outcomes. A small CI corpus never replaces the full inventory.
- For shared primitives or bindings, run `tools/m1_platforms.py` in a fresh output directory with the pinned browser tooling. Strict raw comparison and the `--verify-contract` architecture gate are separate. Preserve any observed mismatches; never remove or normalize failures to make CI green.
- Preserve original evidence and use new output directories for reruns. Record exact artifact/input/runtime identities and compare raw bytes. Changes to heap limits or other execution conditions require separate, explicit experiment evidence.
- In completion reports, distinguish changes made, checks run, unverified scope, and known issues. Do not report unexecuted checks as passing.
