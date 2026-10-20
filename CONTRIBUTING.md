# Contributing

See [README.md](README.md) for the development environment and default checks, and [TESTING.md](TESTING.md) for compatibility criteria. The shared runtime and verification tools are currently at the initial setup stage.

## Workflow

1. Follow the README instructions for preserving line endings and initializing submodules, then check the reference pins with `cargo xtask verify`.
2. Review the scope and prerequisites in the [roadmap](docs/roadmap.md). Establish the semantics of new implementations from the pinned `external/pkl` sources and tests.
3. Keep changes small and explain their effects on observable behavior. Do not add placeholders that return success for unimplemented APIs.
4. Run checks appropriate to the changed area and record the results and unverified scope.

## Implementation principles

Separate the shared engine from platform-specific Host capabilities. Keep evaluation semantics and error formats consistent across native, Wasm, and N-API targets. Running the Rust implementation must not require a Java runtime.

Support compatibility changes with actual Java 0.32.0 results and reproducible inputs. Regression cases need their inputs, environment, and expected output provenance. Preserve original licenses and copyright notices when importing upstream code.

Before adding dependencies, review their purpose, Wasm support, licensing, and memory and performance impact. Track `Cargo.lock` in the repository and use `--locked` in CI and verification commands.

Write standard project documentation in English, including the README, repository instructions, contribution guide, testing contract, architecture documentation, and API references. Write planning documents in Korean, including `docs/roadmap.md`, implementation plans, and task plans. Apply these language rules to new documents and future updates.

Keep code, commands, identifiers, and file paths unchanged when translating prose. Preserve the original contents of upstream references and compatibility fixtures.

## Verification and change descriptions

Run the default checks in the README after changing Rust code or development configuration. For documentation changes, check links, paths, commands, and the accuracy of implementation status statements. Do not claim to have run Java differential tests or performance measurements that have not been implemented yet.

Include the following in change descriptions:

- The problem being addressed and the resulting behavior.
- Relevant Pkl 0.32.0 source and test evidence.
- Checks run and their results.
- Known mismatches, unexecuted checks, and follow-up work.

Keep reference version updates separate from ordinary feature changes. Review both submodule pins, `references.lock`, documentation, and oracle and corpus baselines together when updating references. Ad hoc edits under `external/` are not a substitute for implementation changes in this project.
