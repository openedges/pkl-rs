# Java full-frame transformer observations

These are selected source-driven public Java API observations from pinned Pkl
0.32.0. They are independent Java authority, not Rust compatibility results.
The JVM conditions remain `-Xms64m -Xmx512m`, the pinned Temurin runtime, empty
explicit guest environment/properties, disabled color and disabled power
assertions. No production evaluator calls Java.

The main matrix contains 48 sessions, each acquired twice in a fresh JVM and
one reused evaluator: 96 processes, 180 public calls, 164 transformer callbacks,
82 logger callbacks (68 WARN). Public call outcomes are 142 successes, 24 guest
errors, 10 Java internal errors and four API exceptions. All 48 process-repeat
pairs match complete raw bytes. Counts describe recorded observations, not
independent language features or compatibility completion.

`cases.json` records exact, unwrapped source queries and independent transformer
and logger profiles. `default` uses the real builder default without an observer.
`builtin-identity` directly installs `StackFrameTransformers.empty` without an
observer. `identity` is an actual observed user callback that delegates to that
identity transformer; `full` returns the complete recorded replacement frame;
`numbered` returns that frame with `#` and the zero-based callback index appended
to its URI. The last profile observes that Java compresses original diagnostic
frames before calling the transformer on retained leaves.

Replacement strings are canonical unsigned UTF-16 unit arrays. The probe reads
UTF-16BE replacement files as individual Java chars, preserving unmatched
surrogates. It records all original and returned frame fields, nullable versus
empty member names, empty source-line lists, all source lines and all four
inclusive positions. Every captured string has its original Java UTF-8 bytes
and exact UTF-16BE code units. Java UTF-8 replaces unmatched surrogates with `?`;
these bytes do not establish equality with the JavaScript UTF-8 encoder.

Callbacks are source-driven public evaluator reentry, not invented VM events.
Each callback is used once when its same-kind zero-based entry index reaches
`callback_after`; that guard is set before reentry. A deliberate `throw-once`
is selected after the callback returns, so a nested same-kind callback may
consume the throw before its outer callback resumes. The protocol records
actual enter, return and deliberate throw events without fabricated successful
exits. Its depth counts all active observed transformer and logger callbacks.
The Java API supplies no log-versus-diagnostic purpose flag to a transformer;
this fixture does not invent one.

Java wraps logger/warning-transformer failures as `PklBugException` and invokes
the transformer again while rendering those errors. Those calls remain
`oracle-error`. A transformer that throws while rendering a guest diagnostic
instead escapes as an actual `IllegalStateException` API exception. An empty
replacement source list succeeds for logging but diagnostic rendering throws
an actual `ArrayIndexOutOfBoundsException`. Full original error bytes and actual
exception classes are preserved as producer provenance, never manufactured by
other targets.

`buffer-cases.json` is a separate Java-only supplement using the real
`Evaluator.evaluateTest(...).logs()` result. Its seven sessions run twice in
fresh JVMs: 14 processes, 32 public calls and 50 transformer callbacks. All seven
repeat pairs match. Four nested `PklBugException` test results remain recorded;
a returned `TestResults` object is not a claim that its assertions passed.
The observations expose internal buffer append before transformer/delegate
failure, retained attempted messages, clearing on the next public call, and
clearing by public evaluator reentry. The noop delegate still receives no logger
callback while custom transformations and buffered source messages remain
observable. This is not evidence that the candidate implements the test-tool
API; that supplement remains a separate unexecuted obligation.

The compact `oracle-evidence.tar.gz` keeps `main/` and `buffer/` acquisitions,
complete process stdout/stderr/status, compiled Java driver classes, exact
commands/runtime identity, all input/replacement bytes, captured sources and
raw callback/result channels. Independent archive and complete file-set pins
prevent locally refreshed fixture metadata from replacing Java authority.
No Java installation or original `target/` directory is needed for offline
verification. The verifier uses standard-library-only imports in this mode.

```sh
python3 tools/frame_transformer_fixtures.py
python3 -m unittest discover -s tools/tests -p 'test_frame_transformer_fixtures.py'
```

Fresh acquisition, always to new directories:

```sh
python3 tools/frame_transformer_oracle.py --output target/m2/new-frame-main
python3 tools/frame_transformer_oracle.py --output target/m2/new-frame-buffer --kind buffer
python3 tools/frame_transformer_oracle.py --verify-report target/m2/new-frame-main
```

The earlier 47-session `stage55-frame-oracle-01` acquisition is retained unchanged
as historical evidence. The final matrix adds the empty diagnostic source-list
control in a new acquisition directory. Prior Stage54 boundary/cycle evidence
was read without modification and is not counted as new Stage55 execution.
Some authored source bytes are reused unchanged from its fixtures; source URIs,
profiles and full-frame callback observations here have their own provenance.

Package URL/source schemes, service-provider composition, malformed signed frame
coordinates, cross-evaluator stdlib executable lifetime, complete lifecycle/API
parity, full M2 and performance/memory requirements remain open. Partial default
URI or full-frame matches do not close those requirements.
