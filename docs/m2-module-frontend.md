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

`compatibility/m2-module-eval.json` registers original upstream case IDs.
Each entry binds both the input bytes and the complete original corpus case,
including its arguments, working directory, environment, and resources. The
registry now includes all 888 original language snippets whose original execution
prerequisites are ready, plus the preserved blocked `basic/import1b` record.
The 137 earlier registrations retain their complete input and case identities;
the generated registry uses original corpus order. The 752 added registrations
do not change any original case, input, argument, environment, requirement or
expected outcome. Registration
means the comparison can be attempted; it does not mean
the candidate passes or that its original execution prerequisites are fulfilled.
The JVM-classpath-dependent `basic/import1b` case remains blocked in the original
corpus. A matching CLI failure cannot satisfy its expected successful outcome.
Reports retain raw channel equality separately from that original expectation;
an unmet expectation is `oracle-contract-mismatch`, even if both processes agree.

Regenerate or verify the complete registry with:

```sh
python3 tools/m2_module_eval.py
python3 tools/m2_module_eval.py --check
python3 tools/m2_manifest.py
python3 tools/m2_manifest.py --check
```

The check rejects omitted ready originals, changed case/input hashes, missing
declared resources, and unreviewed blocked registrations. The 68 other blocked
originals, three selected historical regressions and 40 unmapped accounting rows
retain their obligations. Readiness and execution coverage are distinct from
raw compatibility results and semantic requirement coverage.

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

If acquisition or restoration raises an exception, the tool retains an
`acquisition-failure.json` record, available raw channels and changed-file bytes.
It attempts restoration even after a failed launch or interruption. A restoration
failure is recorded separately without replacing the original capture exception.
Such an attempt does not produce a passing comparison and requires a new output
directory before retrying.

## Explicit original-corpus workspace

The default `legacy-full` workspace still copies both complete pinned reference
trees and the project's local `tests/fixtures` tree. A broad original-module
experiment can explicitly select a smaller filesystem boundary:

```sh
python3 tools/m2_language.py --adapter language-module-eval --module-workspace-profile upstream-original-modules-v1 --output target/m2/upstream-modules-new --record-only
```

This profile retains every declared file and symlink in the complete pinned
Pkl and Pantry trees. It omits only the additional project-owned fixture copy;
every selected original input and declared resource must remain inside the
retained inventories. Original arguments, working directories, environment,
source paths, raw output/error/file comparisons and process watchdogs are
unchanged. The profile is an explicit experimental filesystem condition, not a
claim of equivalent results under every possible surrounding directory tree or
of improved evaluator speed or memory use.

The workspace marker and schema-5 report bind the selected profile and source
inventory digests. Offline verification requires the exact retained baseline,
including directory paths and symlink identities, and rejects missing or extra
files. An existing workspace with a different profile cannot be reused. Each
Java and Rust capture still hashes the complete selected workspace before and
after execution; no timestamp cache replaces byte checks. Historical reports
retain their original workspace boundary and results.

The shared argument parser accepts the selected `eval` profile with `--no-cache`,
`--no-project`, `--settings pkl:settings`, ordered `--property` and `--env-var`
arguments, and one source path. Map values split only at the first `=`. Unsupported
options remain explicit adapter failures. The output operation passes the
ordered property and environment maps to the shared read policy. Other missing
module/resource capabilities remain explicit failures.

The native development probe inherits process environment entries before
applying explicit `--env-var` options. Unix values use the pinned JDK's UTF-8
replacement behavior, including malformed surrogate encodings. The portable API
continues to receive explicit environment inputs through its options.

Native environment compatibility remains incomplete when different malformed
raw names decode to the same string: Java's raw-key `HashMap` iteration can
select a different value from native environment order. This is a recorded
raw-output mismatch. Non-Unicode entries on other operating systems also remain
unsupported. These limitations are separate from verified value decoding and
ordinary inherited/explicit override behavior.

The [shared module-loading contract](m2-module-loading.md) describes typed
resolution and source requests, evaluator-owned module identities and caches,
the initial native file Host, and portable bundle comparisons.

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

Complete PCF behavior, imported types and module inheritance, remaining resource loading, other renderers and file
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

## Shared imports and expression sessions

The engine separates module URI resolution from source loading with typed Host
requests. Native development adapters supply file sources; portable adapters
supply explicit resolution/source bundles. Imported programs share the current
VM, traced heap, declaration identities and module caches. Source loading may
suspend or reenter that evaluator. The engine publishes the empty receiver before
initialization and preserves observed retry and failure-cache behavior.

`SourceSession::compile_expressions` and the portable
`SourceSession.forExpressions` compile each expression through the shared parser,
resolver and compiler. Expressions execute against the actual module receiver.
They retain the synthetic `repl:text` source and error frames. A compilation error
in a later expression is deferred until that entry executes, preserving module
initialization precedence and earlier successful calls. This is a development
session API; module-declared types in these expressions and mutable REPL behavior
are still incomplete.

The [Stage 41 platform captures](../reports/m2/stage41-platform-captures.json)
retain 22 unchanged Java-origin module graphs. Each Node/Chromium target has 36
matching and eight unsupported graph repetitions. Underlying call comparisons
contain 106 matches, eight unsupported calls and two mismatches; the two
follow-up mismatches remain visible inside an unsupported graph. Imported type
resolution, `extends`/`amends`, timeout/close boundaries and complete URI transport
remain required work.
