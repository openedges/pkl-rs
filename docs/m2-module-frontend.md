# Original module comparisons

The shared module frontend prepares an original module through lexing, parsing,
deferred AST validation, and name resolution. It retains the source URI and
module name needed by the common diagnostic renderer. The frontend-only API
continues to return an explicit unsupported result when preparation succeeds.
The separate output API compiles the module, reads its actual `output` property,
checks the exact `ModuleOutput` type, and evaluates `bytes` through the shared VM.
Native `module_eval_probe` now uses this output operation.

This is a development boundary for implementing and verifying the language. It
does not establish the product CLI, renderer, module loader, or embedding API
contracts. The [whole-language gate](m2-language.md) continues to account for
those dependencies and all unexecuted language cases.

## Original inputs and process comparisons

`compatibility/m2-module-eval.json` registers reviewed original upstream case IDs.
Each entry binds both the input bytes and the complete original corpus case,
including its arguments, working directory, environment, and resources. The
registry selects the original 72 frontend error cases and 30 output/loading
cases. Registration means the comparison can be attempted; it does not mean
the candidate passes or that its original execution prerequisites are fulfilled.
The JVM-classpath-dependent `basic/import1b` case remains blocked in the original
corpus. A matching CLI failure cannot satisfy its expected successful outcome.
Reports retain raw channel equality separately from that original expectation;
an unmet expectation is `oracle-contract-mismatch`, even if both processes agree.

`tools/m2_language.py` acquires both the scalar expression adapter and the
original module adapter by default. A focused acquisition can select either
adapter explicitly:

```sh
python3 tools/m2_language.py --adapter language-module-eval --output target/m2/module-frontend-new --record-only
python3 tools/m2_language.py --verify-report target/m2/module-frontend-new
python3 tools/m2_language.py --verify-subset target/m2/module-frontend-new
python3 tools/m2_language.py --verify-full target/m2/module-frontend-new
```

Use a new output directory. The tool records a release build of
`module_eval_probe`, freezes its executable, and runs fresh Java and Rust
processes with the original command profile. `--module-candidate` supplies an
explicit alternative artifact without a verified source binding.
`--module-workspace` selects an exclusively owned, managed workspace. Both
processes use the same physical input path; diagnostics are not rewritten to
replace one path with another. Module comparisons execute sequentially even
when scalar acquisition uses multiple workers.

Each comparison retains complete stdout, stderr, exit status, generated files,
and unexpected workspace changes. Before and after snapshots record workspace
state. Declared inputs and resources are checked against the original manifest;
offline verification also binds their paths, kinds, and hashes to the captured
baseline and input. Host-private home and temporary files are retained separately
as provenance. They are outside the original public generated-files comparison.
Unexpected public workspace writes are failed observations and are restored
before the next process.

The shared argument parser accepts the selected `eval` profile with `--no-cache`,
`--no-project`, `--settings pkl:settings`, ordered `--property` and `--env-var`
arguments, and one source path. Map values split only at the first `=`. Unsupported
options remain explicit adapter failures. The output operation passes the
ordered property and environment maps to the shared read policy. Other missing
module/resource capabilities remain explicit failures.

The native development probe currently supplies only the explicit `--env-var`
map. It does not merge the inherited process environment as the Java CLI does.
The portable API likewise has no inherited native process environment. Captures
retain each actual environment, but the current adapter does not establish full
CLI environment inheritance compatibility.

## Diagnostic meaning and ordering

Diagnostics carry their producer phase and failure disposition separately from
the legacy diagnostic topic. An ordinary invalid keyword can therefore remain a
language error even if its old topic was named `Unsupported`. Actual missing
capabilities, resource limits, and internal failures cannot become compatibility
passes merely because their rendered text resembles an expected error.

The development probe uses exit/result code `1` for ordinary language errors,
`2` for unsupported capabilities, `3` for resource limits, and `4` for internal
failures. Native input/output acquisition failures also use `4`. These codes
distinguish failed development observations; they are not a claim that the
product CLI implements these failure semantics. Unsupported or internal results
never pass the ordinary language comparison.

The renderer receives the original source and logical frame. The selected CLI
profile uses the pinned bundled-settings file frame format, including
`, line N`; the API identity profile remains separate. No trimming, path
replacement, partial-message comparison, or golden-file replay is permitted.
The complete syntax is parsed before name resolution. Deferred AST validation
and explicit unsupported results preserve the implemented frontend ordering;
they do not prove the remaining module evaluation phases.

Independent calls to the pinned Java parser can establish where an upstream
error originated and retain its UTF-16 span. Such component observations help
diagnose a mismatch. Only the original CLI process capture establishes the
expected whole CLI bytes for this adapter.

Phase labels describe the component that actually produced each implementation's
error. In the original 72 portable comparisons, 12 string cases originate in the
Java parser and the Rust lexer while their complete rendered bytes agree. Keep
those producer differences visible; do not relabel them to manufacture phase
equality or treat internal phase equality as a substitute for output comparison.

## Native, Node, and browser boundaries

Native `module_eval_probe` reads the actual source file and writes the common
result bytes to the process streams. N-API and Wasm expose
`evaluateModuleFrontend(source, uri, moduleName, arguments)` and
`evaluateModuleOutput(source, uri, moduleName, arguments)`, using the same
argument parser and engine. Their results contain `phase`, `disposition`,
`reason`, byte-vector `stdout` and `stderr`, and `exitCode`. Output success has
phase `module-output`, code `0`, and no failure disposition. Portable observations
retain whether optional fields are absent, undefined, or null.

The portable API transports supplied source and identity; it does not execute a
native process, read an operating-system file, or create generated files.
`tools/platform/compare-module-eval.mjs` compares its complete byte vectors and
result status with separately captured original Java CLI observations on actual
Node and Chromium. A matching API `exitCode` is distinct from an observed native
process exit. Transport failures remain failed calls, and phase observations
are separate from raw byte equality.

The portable comparator defaults to the historical frontend operation. Select
the actual output operation explicitly, supplying a separately captured Java
observation directory with the original module contracts:

```sh
node tools/platform/compare-module-eval.mjs --operation output-bytes target/m2/module-output-portable-new target/m2/platforms-new/wasm target/m2/platforms-new/pkl_napi.node target/m2/module-output-java-new
```

## Output execution and lifecycle

`SourceSession::compile_output` prepares the actual module and optional callback
property paths in one program. `start_output_bytes` and `reenter_output_bytes`
share the existing Host context stack, deadlines, logging, suspension, and
collection. N-API and Wasm expose this mode through `SourceSession.forOutput`.
Completed bytes are owned independently of the evaluator. A nested callback's
property-path error retains its expression frame even when the suspended outer
call is an output operation.

The current output implementation includes lazy `ModuleOutput` properties, text
and bytes overrides, PCF traversal, and converter continuations. Renderers invoke
ordinary VM functions, so their reads and property transformers can suspend.
Output bytes are published only after successful evaluation. Logs already
delivered before a failure remain in stderr before the rendered error.

The [UTF-16 output fixtures](../tests/fixtures/m2-pcf-string-output/metadata.json)
contain 192 whole-output expectations acquired from the pinned Java evaluator,
including Host-supplied unmatched surrogates, custom delimiters, indentation,
and nested properties. The runtime test compares complete bytes and retains the
owned result after evaluator close. Component formatter checks are separate
evidence and cannot replace actual evaluator execution.

Complete PCF behavior, imports and resource loading, other renderers and file
outputs, remaining language rules, and all product CLI options retain their
original milestone obligations. The output-operation logger-close control also
retains a Java internal error and a candidate mismatch. Focused results do not
close the full M2 gate or establish performance parity.

The [Stage 39 original observation archive](../reports/m2/stage39-oracle-captures.json)
retains 144 fresh Java CLI executions for the original 72 cases, separate public
parser phase observations, and independent input bindings to the pinned
submodule's Git blobs. The CLI repeats have identical raw results. This archive
contains no Rust candidate executions and does not establish candidate
compatibility.
