# Selected historical regression inputs

[compatibility/regressions.json](../compatibility/regressions.json) selects three entrypoints from the historical `pkl-rs.rev1` revision `008930df07f457fefd5d6f4ac9308ff3063daabc`. Eight Pkl files are vendored as unchanged Git-object bytes under [tests/fixtures/rev1](../tests/fixtures/rev1). The selection supplements the canonical upstream/Pantry corpus; it does not modify that inventory or require the historical checkout during builds or CI.

| Entrypoint | Historical test intent | Input closure |
| --- | --- | --- |
| `diagnostics/t36/app/negative.pkl` | Reject a foreign nominal class inside a constrained collection and preserve declaration diagnostics | Three original Pkl files and two derived local-project settings files |
| `class_authority/t40/repro.pkl` | Accept the producer's collection item using its declaring class authority | Two original Pkl files |
| `class_authority/t40/foreign_namesake/repro.pkl` | Reject a foreign class with the same name and preserve declaration frames | Three original Pkl files |

The selection follows `foreign_class_constraint_diagnostic` and `imported_collection_member_declaration_authority` in the historical `class_surface_execution.rs`, plus the CLI `foreign_namesake_bytecode_declaration_frame_matches_apple` test. Their exact source revision and hashes are linked through the checked-in historical source census. Historical Rust assertions describe why each reproduction matters; they are not expected results for this project. Older `.error` and `.stderr` golden files are deliberately not imported.

The diagnostic reproduction originally creates temporary sibling `app` and `dep` directories in Rust test code. Its Pkl inputs are preserved byte for byte in that layout. Two derived fixtures recreate the same project declaration and local dependency map, using a relative `../dep` path instead of a temporary absolute path. This adaptation is recorded separately from the original Pkl source provenance. The supporting `form.pkl`, `foreign.pkl`, and library files are dependencies, not independent test cases.

Every case uses the shared runner's schema: imported path/hash, separate original repository/revision/path/hash, command arguments, project scope, environment, Host requirements, full immediate resource closure, raw stdout/stderr/exit/file channels, execution targets, and status. Fresh pinned Java 0.32.0 executions were reviewed before promoting the cases to strict exit expectations: the diagnostic and foreign-namesake cases require exit 1; the producer-class case requires exit 0. The positive case emits exactly `result = List(1)` followed by a newline, with empty stderr. Both negative cases report the intended incompatible nominal classes at the expected declaration/evaluation sites, rather than a missing dependency or environment failure. The manifest preserves the Java artifact/source revision, JDK identity hashes, original raw stream hashes, and reviewed outcomes separately from the historical assertions. Those stream hashes identify the recorded environment; formal comparisons capture their own raw oracle bytes without rewriting locations. These Java results do not establish Rust compatibility.

## Reproduction and checks

```sh
python3 tools/regression_manifest.py --check
python3 -m unittest discover -s tools/tests -p test_regressions.py -v
python3 tools/compat.py record \
  --manifest compatibility/regressions.json \
  --workspace target/m0/regression-workspace \
  --output target/m0/regression-baseline --jobs 1
python3 tools/compat.py check \
  --manifest compatibility/regressions.json \
  --output target/m0/regression-baseline
```

The manifest check validates every imported fixture against its original record in `compatibility/rev1-inventory.json`, verifies the pinned historical revision, checks LICENSE and NOTICE hashes, validates the two derived settings fixtures, and rejects missing or uninventoried vendored files. Normal checks never invoke Git or read a sibling checkout. The regression tests deliberately corrupt Pkl input, license bytes, project settings, revision provenance, and the file census to verify failure detection.

To explicitly recreate the vendored inputs from an available historical repository, use `python3 tools/regression_manifest.py --import-from /path/to/historical/repository`. It reads the pinned Git objects and verifies all original hashes before writing any files. The old checkout's current working tree and passing test records are not used.

The Java runner stages an isolated workspace, retains full raw streams and file sets, and records oracle provenance. Keep that evidence when reviewing an input or expectation change. Reusing this fixture bundle changes the staged input snapshot, so regenerate earlier smoke baselines before comparing runners against a newly staged workspace.

## License and attribution

The historical repository publishes an Apache License 2.0 [LICENSE](../tests/fixtures/rev1/LICENSE) and [NOTICE](../tests/fixtures/rev1/NOTICE). Both are preserved as exact Git-object bytes. Its notice identifies historical `pkl-rs` (2025, OPENEDGES Technology Inc., originally recorded in the frozen Git object as `OpenEdges Inc.`) and acknowledges upstream Pkl test-fixture material from Apple Inc. and the Pkl project authors, Copyright 2024–2025. The historical notice's version and product statements remain original attribution text for those frozen fixtures; this open-source project is licensed under Apache-2.0 by The pkl-rs Authors, is not owned by OPENEDGES Technology Inc., and targets Pkl 0.32.0 compatibility.

These eight files carry their historical repository license and notice; their inclusion is not a claim of new authorship. The selected bundle has no old generated expected results. Fresh Java evidence and the current project's implementation remain separate.
