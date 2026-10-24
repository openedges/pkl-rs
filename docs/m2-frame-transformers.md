# Stack frame transformation

Stage 55 extends the shared engine's frame model and Host continuation. This is
an experimental evaluator contract, not a completed product API or full M2
compatibility claim. The independently pinned
[Java fixture](../tests/fixtures/m2-frame-transformers/README.md) defines the
observations used for native, N-API, and Wasm comparison.

## Source identity and callback order

Runtime sources retain their authored module URI. Standard-library sources use
their original `pkl:` URI; the default frame policy produces the pinned GitHub
display URL and line anchor when a frame is delivered or rendered. Identity
policy preserves the original frame. A Host transformer receives the original
frame and returns a complete replacement, without an additional default
transformation afterward.

The transformer and logger are separate continuations. Message buffering precedes
transformation; the logger receives the returned frame. A noop logger still
allows an installed transformer to run. A deprecated call publishes its target
only after successful delivery. Rejection and reentry retain their original
ordering and pending roots.

Frame text is represented and transported as UTF-16 code units, including lone
surrogates. Module URI, nullable member name, source lines, and all four inclusive
source coordinates are independent fields. A replacement URI is not parsed or
normalized. An empty member name differs from a null member name.

Diagnostics compress repeated raw frames before invoking the transformer. Each
returned frame is validated for rendering before the next callback. A custom
frame can be valid for logging yet fail during diagnostic rendering; those
outcomes remain separate observations. Guest catch paths that omit stack frames
do not invoke the frame transformer.

## Verification boundary

The main fixture uses exact, unwrapped `evaluateExpressionString` queries. It
records all public result and error bytes, complete callback frames, UTF-16
payloads, and ordered callback entry/exit protocol. Default and builtin identity
policies are distinct from an observed custom identity callback. Replacement
presets and numbered URIs execute in the Host callback, not in a comparison
normalizer.

A separate Java-only experiment observes `evaluateTest` and `TestResults.logs()`.
It does not establish candidate test-tool support or buffer parity. Java internal
errors, API exceptions, nested test errors, and unexecuted boundaries remain
nonpasses or unverified requirements.

Candidate callback-origin metadata distinguishes log transformation, log
delegation, diagnostic transformation, and diagnostic rendering. It identifies
the actual failure phase without manufacturing Java exception class names.
Only metadata attached to an actual public call is admitted as provenance; all
public frame and protocol channels remain raw byte comparisons.

## Remaining obligations

The first native acquisition retains failures in callback-exception propagation
and diagnostic frame locations. Corrections require new captures; the original
results must remain intact. Actual Node and browser comparisons, the default
frame regressions, and the required repository checks are needed for checkpoint
closeout.

Package and service-provider default transformations, Host callbacks from eager
synchronous constructors, complete exception/API parity, and cross-evaluator
standard-library execution state remain explicit implementation obligations.
The main fixture is not the entire language, API, or performance gate.
