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

Source text, URI, and identifier inputs still use UTF-8. Probe Host-rejection
message inputs explicitly reject unmatched units. Invalid converter-path and
unknown output-format producers still need canonical composition. Java internal
errors from logger experiments remain oracle errors, not compatibility passes.
Full diagnostic coverage, original-corpus regression, complete Host callbacks,
and product/release performance gates remain required.
