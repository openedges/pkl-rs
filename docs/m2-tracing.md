# Trace and source-session development contract

The shared runtime implements the `trace` expression against the pinned Pkl
0.32.0 reference. This development contract covers source evaluation, logger
events, callback reentry, and selected lifecycle behavior. It does not establish
complete evaluator, module-loader, REPL, or public API compatibility.

## Evaluation and delivery

`trace` evaluates its operand once, forces the resulting value through the common
runtime, renders it, delivers the log event, and returns the original value.
An operand or forcing error prevents delivery. A no-op logger still performs
forcing, rendering, and source-frame construction. A configured Host frame
transformer still receives the event when the logger sink is a no-op.

The shared renderer supplies COMPACT and PRETTY messages. Source frames describe
the operand and retain its source lines, URI, optional member name, and one-based
UTF-16 positions. Message text and frame fields use `PklString`; native, N-API,
and Wasm transport preserves isolated UTF-16 surrogates. A null member name and
an empty member name are distinct values.

`HostPoll::Log` carries an owned event and a request token. The caller completes
that request through `acknowledge_log`, optionally supplying `LoggerFailure`.
Read requests and log requests have distinct completion and cancellation paths.
Callbacks execute outside an outstanding mutable Rust borrow. Retained event
text remains usable after another call, collection, or evaluator closure.

## Source sessions

`evaluation::session::SourceSession` owns the compiled source, module receiver,
heap, and Host context stack. Its development interface accepts property paths
registered before execution. It is not an arbitrary-expression REPL.

A fresh `start_path` performs module initialization and the first requested path
within one admitted Host context and deadline. Closing from a callback rejects
new calls while allowing an already admitted call to finish. An explicit
`start_module` remains available as a separate operation. Initialization errors
are retained for subsequent calls; a property error does not become a cached
initialization error. `setup_failed` reports the initialization outcome even
after the associated diagnostic and heap roots are released. Host-wide timeout,
cancellation, or closure before initialization completes is a terminal Host
outcome, not a cached initialization-language error.

The development N-API and Wasm bindings expose the same source session and typed
polling events. Successful values, errors, and textual log fields cross that
boundary as UTF-16 unit arrays. Both bindings delegate evaluation and error
rendering to the shared Rust runtime.

## Raw comparisons

`compatibility/m2-trace.json` records original source inputs, ordered expressions,
logger and reader callbacks, rendering mode, and logger action. The acquisition
tool runs fresh Java and native processes twice per selected contract:

```sh
python3 tools/m2_trace.py --output target/m2/trace-new --record-only
python3 tools/m2_trace.py --verify-report target/m2/trace-new
python3 tools/m2_trace.py --verify-contract target/m2/trace-new
```

Each acquisition retains the pinned oracle identity, source snapshots, native
artifact, build commands, process captures, complete value/error bytes, callback
protocol, and log fields. Text has both Java-compatible UTF-8 capture and separate
UTF-16 code-unit capture where applicable. No comparison trims or normalizes
payloads. JVM exception class names remain provenance rather than Rust type-name
requirements. Java internal errors remain failed compatibility observations.

`--verify-report` checks recorded evidence integrity. `--verify-contract` also
requires the complete selection, a recorded native build, stable sources and
artifact, and all comparisons passing. A supplied artifact or partial selection
cannot establish that contract. Cargo build output is excluded before evidence
directory traversal.

Actual Node and Chromium comparisons use the same Java captures:

```sh
node tools/platform/compare-trace-sessions.mjs \
  target/m2/trace-platform-new WASM_DIRECTORY NAPI_BINARY JAVA_ACQUISITION
```

The platform driver retains complete payloads, artifact identities, source
identities, runtime versions, and cleanup observations. A source compilation
error has no session or terminal heap measurement. A created session must release
its heap on close. Architecture checks and byte compatibility are separate
results; neither implies evaluator performance or memory parity.

## Remaining scope

Full module loading and resource caching, general expression sessions, warning
delivery, all frame-transformer behavior, complete lifecycle diagnostics, and
the original upstream language corpus remain open. Selected `String.repeat`
cases support the trace fixtures; they do not establish the complete String API.
Its pinned Java count-narrowing internal errors and allocation-failure behavior
remain explicit limits. The complete M2 gate is described in
[the language verification contract](m2-language.md) and [PLAN.md](../PLAN.md).
