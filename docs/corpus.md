# Reference corpus discovery

`python3 tools/discover.py` rebuilds [the manifest](../compatibility/corpus.json) from Git-tracked files at the revisions in `references.lock`. `python3 tools/discover.py --check` fails on a missing manifest or any byte difference. Python 3 and Git are the only discovery dependencies. Discovery does not evaluate Pkl or establish compatibility.

The inventory accounts for every tracked `.pkl` file and extensionless `PklProject` in both submodules, including production sources, documentation examples, expected formatter output, imported support modules, and executable inputs. Every tracked file, including resources with other extensions, has its byte count and SHA-256 in the repository's resource set. Git symlinks are hashed as their link text. Untracked build output is excluded. Repository revisions and resource-set digests preserve the input closure without pretending static import analysis can resolve computed imports.

At the pinned revisions there are 1,546 Pkl files (1,217 upstream and 329 Pantry), 73 `PklProject` files, and 1,112 selected command cases. Of these, 1,013 have a command adapter and 99 are blocked. These counts measure discovery, not successful executions or language coverage. The manifest also records 199 upstream JVM test or test-support source files; source files can define multiple classes, parameterized tests, generated cases, and support code. Their dynamic case and assertion counts remain unknown. No file count is substituted for an assertion count.

| Selected suite | Command cases | Selection contract |
| --- | ---: | --- |
| Language snippets | 957 | Tracked `.pkl` inputs under `LanguageSnippetTests/input` |
| Formatter snippets | 30 | Inputs under `FormatterSnippetTests/input`; API adapter blocked |
| Pantry project tests | 44 | Package `tests/**/*.pkl`, excluding `tests/fixtures/` |
| Pantry module names | 1 | `packages/moduleNameTest.pkl`; dynamically generated facts |
| Pantry example candidates | 51 | Package `examples/**/*.pkl`; support and abstract modules remain candidates |
| Pantry package/API tests | 29 | Every package `PklProject`, with its local project dependency closure |

The full upstream test-source census includes parser, Java/Kotlin code generation, documentation generation, REPL, CLI, server/protocol, executor, configuration, and runtime tests. It does not claim to have adapters for these JVM suites. Such records carry an explicit blocker. Importing a source file into a language snippet does not replace running its independent API, formatter, or protocol tests.

## Manifest contract

`schema_version` is 1. `sources` identifies repositories and pins all tracked resource bytes; `files` accounts for every Pkl/project file and links selected `case_ids`; `cases` contains executable or explicitly blocked contracts; `suites` records non-Pkl JVM test sources. `dependency_lockfiles` retains parsed Pantry dependency declarations and checksums. `optional_sources` links the historical inventory. `summary` reports discovery counts with unknown assertions and zero compatibility passes.

Each case has a stable `id`, source repository/revision/path/SHA-256, `command` arguments, root-relative `cwd`, environment allowlist, Host requirements, target applicability, expected exit set/outcome, raw comparison channels, and execution status/reason. Native, Wasm, and N-API target tags describe required eventual coverage, not present adapters. `resource_set` refers to the complete repository census. `resources` additionally identifies known immediate inputs and upstream golden files; it is not a complete static dependency graph.

Commands run from an isolated workspace that preserves repository-relative paths. The runner must expand `{output_dir}` to its controlled per-case output directory before executing package commands. Source checkouts must remain unchanged: test runners and packaging can create actual example results and artifacts, so execute these commands against copies. Pin paths through the execution environment when comparing runners; never replace paths in captured bytes.

`execution.status=ready` means a command can be attempted with its declared Host facilities. It is not a result. Blocked commands remain required inventory items and must be reported. The discovery file never stores a pass count derived from Java execution. Cases tagged `expected_outcome=observe` need outcome review before use as semantic acceptance tests; permitting an observed nonzero exit does not mean the input passed its upstream test.

## Upstream language snippets

The selection and output mapping follow `pkl-core/src/test/kotlin/org/pkl/core/LanguageSnippetTestsEngine.kt` and `pkl-commons-test/.../InputOutputTestEngine.kt`. An ordinary `.pkl` maps to a `.pcf` output; a hidden renderer extension such as `.json.pkl` maps to `.json`. A matching `.err` indicates either evaluation failure or logger output: upstream treats a nonblank diagnostic stream as failure even when evaluation succeeds. Therefore `.err` cases use `expected_outcome=observe` and allow CLI exits 0 or 1 until the raw CLI outcome is captured independently.

The upstream harness configures fixed properties and environment variables, disables the module cache, enables power assertions, and supplies a PackageServer plus a generated localhost certificate. Project snippets apply project settings; the JVM suite excludes native-only inputs and has platform/JVM-dependent exclusions. The manifest conservatively blocks native-only, project/package-server, and classpath fixture inputs when their Host adapter is unavailable. The remaining CLI commands carry the fixed properties, environment variables, `--no-cache`, `--no-project`, and `pkl:settings`. These commands provide a raw CLI baseline; they do not reproduce the JVM evaluator API test harness in full.

Upstream goldens are **provenance only**. Their harness rewrites source paths, declaration/error line numbers, website/version/stdlib locations, and Windows line endings; MessagePack output is converted into a debug rendering. This project's formal comparison must generate independent raw oracle stdout, stderr, exit status, and file sets. It must not compare raw CLI bytes against transformed `.pcf`/`.err` as if they were raw expectations, nor apply those transformations to conceal differences.

## Pantry development and package execution

`packages/basePklProject.pkl` computes all tests with `import*("**/tests/**.pkl")`, excludes `tests/fixtures/`, filters each package's tests, and shares `tests` with `package.apiTests`. `build.gradle.kts` configures package creation with `pkl-pantry.testMode=1` and file, Pkl, HTTPS, property, package, and projectpackage resource schemes. `moduleNameTest.pkl` dynamically checks module headers across the tree. `pkl.experimental.syntax/PklProject` overrides its API-test selection. The tar examples additionally have `test_examples.sh`, which pipes generated archive bytes to `tar tv -v`; evaluating those modules alone does not validate that shell integration.

The command cases use the enclosing project and `pkl-pantry.testMode=1`. Examples are candidates because some are imported support modules or abstract templates requiring user configuration. Their exact observed outcome is recorded; arbitrary evaluation errors are not accepted as successful example tests.

`project package --skip-publish-check --output-path ...` runs API tests against development modules, checks imports, and produces dependency metadata, ZIP, and two checksum files. It requires every local dependency project as an explicit positional argument. Discovery computes that transitive closure from the checked-in lockfiles. `--skip-publish-check` avoids comparing new archives to a live published release; it does not skip API tests. Creating an archive does not evaluate its contents.

Run `python3 tools/pantry.py --workers 4` to record package/API cases in separate writable copies under `target/m0/pantry`, retain raw streams and generated artifacts, verify generated metadata/ZIP checksums, seed them into a private package cache, and evaluate every `.pkl` member of the generated archives through a `package://` URI. This is a separate Java baseline investigation, not a Rust comparison. Abstract or parameterized packaged modules can produce errors; reports label them observed errors and retain their bytes. Missing archives and failed package/API tests remain failures. The helper refuses to overwrite an existing evidence directory.

Pantry's 31 checked-in dependency lockfiles (including `.github` and `scripts` projects) include local project paths and remote package metadata checksums. The lock hashes and declared dependency maps are preserved. Java verifies downloaded remote packages; `pantry.py` records all actual cache-file hashes so acquired bytes can be audited separately from declarations. Locking a dependency URI is not the same as archiving its HTTP responses. The Dependabot lockfile generator reads live GitHub git-upload-pack refs; the manifest records this unfixed external input. Additional computed network inputs can still require investigation. Their absence from static discovery does not prove hermetic execution.

## Historical regression inventory

[rev1-inventory.json](../compatibility/rev1-inventory.json) is a checked-in optional snapshot at revision `008930df07f457fefd5d6f4ac9308ff3063daabc`. It records historical Pkl input hashes, Rust/Python test-source hashes, and license provenance from Git objects. It includes historical source/support files as well as regression candidates. No historical expected output or passing test record is a Java 0.32.0 verification result.

The sibling checkout is never accessed by normal discovery or CI. To explicitly refresh the historical census, use `python3 tools/discover.py --snapshot-rev1 /path/to/historical/repository`. Selecting or importing particular regressions, preserving their full resource closure and license, and obtaining fresh Java 0.32.0 results remain separate work. Historical entries stay `not-selected`/blocked until then. Their snapshot inventory is not a production or build dependency.

## Tool verification

`python3 -m unittest discover -s tools/tests -p 'test_discover.py' -v` checks census completeness, case/source/resource consistency, duplicate IDs, golden extension mapping, local dependency cycles and escapes, optional-history independence, and failure on a missing or byte-stale manifest. These are harness regression tests. They are not Pkl compatibility assertions.
