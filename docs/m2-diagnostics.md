# Canonical diagnostic comparison

Stage 43 extends the shared engine's diagnostic representation to retain Java
UTF-16 code units. This is a development boundary; complete language, Host,
product API, and platform compatibility remain unfinished.

`RuntimeError`, diagnostic messages and hints, logical frame names, and the
canonical renderer use `PklString`. The scalar `*_text_with_options` helpers,
session `*_text` constructors, and `ModuleFrontendFailure.text` retain that
representation. Legacy Rust `String` methods and native byte output encode
Java-compatible UTF-8 explicitly. Node and Wasm scalar/session adapters return
the original units in JS strings and error messages.

## Independent comparison channels

The fixtures under `tests/fixtures/m2-lossless-diagnostics` retain actual Java
0.32.0 calls, original source and physical URI, call order, repeated evaluators,
complete UTF-16 payloads, and Java UTF-8 payloads. Compare both channels. Lone
surrogates and a literal question mark can have equal Java UTF-8 bytes while
remaining different Java strings. Never infer a unit expectation from those
bytes or normalize an actual diagnostic before unit comparison.

The native checks include throw, type/constraint/union failures, power details,
renderer hints, logs, setup errors, sequential recovery, and collection during
suspension. They preserve the selected original calls, including errors.

```sh
cargo test -p pkl-runtime --locked --test m2_lossless_diagnostic_renderer \
  --test m2_lossless_diagnostic_producers --test m2_lossless_diagnostic_transport \
  --test m2_lossless_frontend --test m2_lossless_scalar
```

`m2-literal-union-predicate` separately captures the pinned Java internal
`TypeNode.isEquivalentTo` predicate. It verifies optimized literal unions versus
general unions, order, duplicates, defaults, and unmatched units. These internal
predicate results are separate from the actual evaluator calls in
`m2-lossless-diagnostics/literal-reuse`; neither substitutes for the other.

## Actual JS execution

`tools/m1_platforms.py` builds and runs native, Node, and Chromium artifacts.
The lossless constructor/scalar comparison tools require source and context
bindings to hard-pinned original Java records. Additional recorded SourceSession
acquisitions compare complete ordered producer calls, logger fields, GC, and
closure against those records. The N-API error-transport probe checks that
creating an error does not invoke user-visible `toString` or `cause` hooks.
Long-string and input-policy controls are architecture checks, not Java results.

Three older platform fixture groups (`string-representation`,
`diagnostic-frame-identity`, and `frontend-owner`) contain only Java UTF-8
expectations. Their checks retain actual JS units and compare the declared Java
UTF-8 serialization separately. They establish no expected frame-unit equality.
Their original expected payloads remain unchanged; new UTF-16 checks reject
surrogate substitutions even when the serialized bytes agree.

## Remaining scope

Source text, URI, and identifier inputs still use UTF-8. Java internal errors
from logger experiments remain oracle errors, not compatibility passes.
Full diagnostic coverage, original-corpus regression, complete Host callbacks,
and product/release performance gates remain required.

## Host failures and output text

Stage 44 extends canonical text to module and resource Host failures, invalid
converter paths, unknown output formats, and abstract member names. `IoFailure`
stores an `Option<PklString>`: an absent IOException message omits its cause line,
while an empty message retains `IOException: `. Module resolve/source failures
and typed resource failures use the same shared error composition. The original
Java records also distinguish source-failure caching from resolve/read retries.

`HostSession::reject_read` checks the current request token and obtains the URI
from the pending read. `SourceProbe::reject_read_io` and the JS `rejectReadIo`
method expose that typed resource completion. Generic rejection injects an
already constructed runtime error; it does not represent a Java IOException.
Probe text rejection methods and Node/Wasm messages retain the original units.
Other Java exception classes and security/URI failure variants remain separate
implementation obligations.

The exact expression API request `output.text` enters the actual ModuleOutput
operation, matching the pinned Java dispatch. It uses the existing module
receiver, Host lifecycle, and VM continuation without adding a REPL frame or
converting an arbitrary expression result to text.

```sh
cargo test -p pkl-runtime --locked --test m2_lossless_host
```

`tests/fixtures/m2-lossless-host` preserves eleven actual Java scenarios and two
fresh JVM repetitions, with 80 ordered calls, complete UTF-16/UTF-8 payloads and
resolve/source/read counters. The native test and actual Node/Chromium runs
compare those original calls, including null/empty messages, retries, and
subsequent successful evaluations. `tools/lossless_host_fixtures.py` exports the
same pinned inputs for the portable driver; its property-format events are
retained separately from the Java custom-reader counters.
