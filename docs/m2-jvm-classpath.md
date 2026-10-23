# Original JVM-classpath snippet development profile

`tools/m2_jvm_classpath.py` provides the additive
`language-jvm-classpath-output` comparison profile recorded in
[`compatibility/m2-jvm-classpath.json`](../compatibility/m2-jvm-classpath.json).
It evaluates the unchanged original `basic/import1b.pkl` and its actual
`org/pkl/core/snippets/imported.pkl` resource. The profile does not replace the
historical CLI command or its blocked inventory row. Successful acquisition does
not automatically promote that row or close M2's whole-language gate.

The pinned JVM `LanguageSnippetTestsEngine` uses
`EvaluatorBuilder.preconfigured()` and `evaluateOutputBytes(ModuleSource.path(...))`.
That builder registers the defining class loader's `ModuleKeyFactories.classPath`.
The native upstream suite explicitly excludes `import1b` because it has no JVM
classpath. The JVM CLI also constructs a different builder with
`ModuleKeyFactories.modulePath`: adding `--module-path` to the recorded CLI
command is a different loader contract.

The Java development probe keeps the JVM engine's fixed environment and external
property maps, enabled power assertions, empty frame transformer and disabled
module cache directory. It retains the default security manager and actual
classpath factories. PackageServer TLS/test-port setup is not installed because
the selected original input and its resource perform no package or HTTP operation.
This omission is explicit in the profile; it is not a general JVM snippet runner.

The complete pinned test resource directory is on the Java classpath after the
official oracle JAR and compiled probe classes. Its original service descriptors
are retained. They reference two Kotlin test providers absent from the published
oracle JAR. Small, source-bound Java development companions preserve those
providers' package names, service classes and behavior so the original
`ServiceLoader` initialization can run without compiling the Kotlin test suite.
The selected input does not use either provider's `test:` scheme. Provider API
compatibility is not established by this comparison.

The Rust development executable receives exactly:

```text
jvm_snippet_probe <absolute-original-source-path> <absolute-classpath-root>
```

Platform I/O stays behind the shared runtime's module Host. The original key and
diagnostic source URI remain `modulepath:/org/pkl/core/snippets/imported.pkl`.
The resolved cache key is the class loader resource's file URL, without file
canonicalization. Missing resources fail during resolution. The same original
key and resolved key participate in the shared evaluator's existing two module
caches. Directory-classpath support for this selected input does not establish
JAR classpaths, arbitrary provider behavior, all URI/security cases, public JVM
APIs, or release platform compatibility.

The runner compiles the source-bound Java probe and companions against a frozen
copy of the pinned official oracle JAR. It records a fresh release build of the
Rust example by default. `--candidate` selects an explicitly unbound caller
artifact instead. Each of two repetitions uses a fresh Java process and fresh
Rust process in the same physical copied upstream workspace. `stdout`, `stderr`,
exit status, and generated/workspace file bytes are compared without normalization.
Java outcome metadata distinguishes successful output from JVM/internal failures.
The expected original exit remains zero; matching a missing-fixture failure is
never a pass. The upstream normalized golden is provenance only.

```sh
python3 tools/m2_jvm_classpath.py --check
python3 -m unittest discover -s tools/tests -p 'test_m2_jvm_classpath.py' -v
python3 tools/m2_jvm_classpath.py --output target/m2/classpath-new --record-only
python3 tools/m2_jvm_classpath.py --verify-report target/m2/classpath-new
python3 tools/m2_jvm_classpath.py --verify-contract target/m2/classpath-new
```

Choose new output and workspace directories. `--verify-report` checks evidence
integrity and exact command/profile/source bindings without invoking Java,
captured executables, or an existing workspace. `--verify-contract` additionally
requires every recorded comparison and fresh repetition to match. Captured
classes, compiler commands, original case hash, classpath tree, source snapshots,
outcome sidecar, raw captures and file changes remain in the evidence. Historical
CLI failures, missing platform verification, and the original blocked full gate
remain separate from this additive comparison.

## Recorded native checkpoint

The [checkpoint summary](../reports/m2/classpath-checkpoint-summary.json) and
[raw evidence](../reports/m2/classpath-evidence.tar.gz) preserve two matching fresh
Java/Rust repetitions of the single unchanged original `import1b` input in
`classpath-original-02`. These are two repetitions of one case. The earlier
`classpath-original-01` results and tooling remain separate historical evidence.
The second acquisition uses the strengthened directory-symlink checks, exact-root
Cargo configuration snapshot, and recorded native build-path verification.

All five required Rust commands passed locally on macOS with stable recorded
inputs. The native example's six tests passed. Initial tooling checks retain
their original 21 passing Python tests; the final tooling evidence runs the
strengthened checker's 15 regression tests and the current profile/inventory
and offline integrity/contract checks. These counts do not measure feature
coverage or establish hosted CI, Linux/Windows execution, or performance parity.

Six additional whole-module Java controls were captured once each, then compared
with native Rust at identical physical input and resource paths. Duplicate-key
class identity, symlink-alias class identity, and a classpath module's absolute
local-file import match. The missing-resource and imported-error controls retain
two complete-stderr mismatches: the current Rust `ApiIdentity` profile maps a
builtin frame to a source URL while the upstream empty frame transformer retains
`pkl:base`. A classpath module's import of `repl:text` remains unsupported in Rust
instead of producing Java's default module-trust denial. All three nonpasses
remain in the original raw captures; no frame or expected output is rewritten.

The [source binding](../reports/m2/classpath-source-binding.json) distinguishes
historical tooling from the final acquisition and records exact proposed-file
Git blob identities without writing Git objects. The
[publication audit](../reports/m2/classpath-publication-audit.json) checks archive
payloads and unchanged source evidence. Final implementation-commit publication
is a separate step. The original CLI inventory row remains blocked pending
explicit reviewed profile integration. General classpaths, full diagnostics and
module security, Wasm/N-API execution, and the full M2 completion gate remain open.
