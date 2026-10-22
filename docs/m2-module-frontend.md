# Original module frontend comparisons

The shared module frontend prepares an original module through lexing, parsing,
deferred AST validation, and name resolution. It retains the source URI and
module name needed by the common diagnostic renderer. If preparation succeeds,
the current adapter returns an explicit unsupported result at the module-output
phase. It does not evaluate output, select an expression, or stringify a value
to manufacture a successful module result.

This is a development boundary for implementing and verifying the language. It
does not establish the product CLI, renderer, module loader, or embedding API
contracts. The [whole-language gate](m2-language.md) continues to account for
those dependencies and all unexecuted language cases.

## Original inputs and process comparisons

`compatibility/m2-module-eval.json` registers reviewed original upstream case IDs.
Each entry binds both the input bytes and the complete original corpus case,
including its arguments, working directory, environment, and resources. The
registry currently selects 72 upstream frontend error cases. Registration means
the comparison can execute; it does not mean the candidate passes.

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
options remain explicit adapter failures. The frontend retains these options;
it does not claim resource or declaration evaluation that would consume them.

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
`evaluateModuleFrontend(source, uri, moduleName, arguments)`, using the same
argument parser and frontend. Their result contains `phase`, `disposition`,
`reason`, byte-vector `stdout` and `stderr`, and `exitCode`.

The portable API transports supplied source and identity; it does not execute a
native process, read an operating-system file, or create generated files.
`tools/platform/compare-module-eval.mjs` compares its complete byte vectors and
result status with separately captured original Java CLI observations on actual
Node and Chromium. A matching API `exitCode` is distinct from an observed native
process exit. Transport failures remain failed calls, and phase observations
are separate from raw byte equality.

This adapter has no successful output evaluation yet. Complete module output,
imports and resource loading, the remaining syntax and semantic rules, and all
product CLI options retain their original milestone obligations. Focused
frontend results do not close the full M2 gate or establish performance parity.

The [Stage 39 original observation archive](../reports/m2/stage39-oracle-captures.json)
retains 144 fresh Java CLI executions for the original 72 cases, separate public
parser phase observations, and independent input bindings to the pinned
submodule's Git blobs. The CLI repeats have identical raw results. This archive
contains no Rust candidate executions and does not establish candidate
compatibility.
