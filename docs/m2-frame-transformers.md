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

## Recorded checkpoint

Implementation `f82060d395d6c1cb16aef2b94a48a2d8f28440b1` has a recorded
[local checkpoint](../reports/m2/stage55-implementation-checkpoint.json).
Native, N-API, and Chromium each executed 48 sessions twice: 82 whole-session
raw matches, 10 Java internal-error observations, and 4 API-exception observations
per target. The strict main comparison remains failed. All 288 sessions have
matching transformer/logger filename sets and bytes across 12,408 channel
comparisons. The 42 nonpassing sessions still differ in public status, stderr
UTF-8/UTF-16, and ordered protocol channels.

The first native capture preserves incorrect callback-failure frame locations.
The correction uses the authored root declaration span and retains the actual
trace caller. Both the original failure and corrected captures are included in
the candidate evidence archive. No original result was rewritten.

Default-policy regressions also ran on all three targets. Deprecation has
306 matches and 30 Java internal-error observations across 336 sessions; trace
has 174 matches and 96 Java internal-error observations across 270 sessions.
Neither matrix introduced a new mismatch or acquisition failure. Their strict
nonpasses remain recorded. Required Rust checks passed, including a separate
unchanged-input Wasm retry after a recorded disk-space failure. The independent
audit verified the exact captured inputs and actual Git-backed source bytes.

The [post-commit binding](../reports/m2/stage55-source-binding.json) connects
those original manifests to the actual implementation commit without modifying
their historical provenance. The archives preserve exploratory failures,
successful reruns, ignored tests, and the two existing shared-primitive regex gaps.

For a new capture, supply separately recorded native, Wasm, and N-API builds:

```sh
python3 tools/frame_transformer_fixtures.py
python3 tools/frame_transformer_compare.py --output target/m2/frame-new --native target/release/examples/frame_transformer_fixture_probe --wasm target/m2/platforms-new/wasm --napi target/m2/platforms-new/pkl_napi.node --record-only
python3 tools/frame_transformer_compare.py --verify-report target/m2/frame-new
python3 tools/frame_transformer_compare.py --verify-full target/m2/frame-new
```

`--verify-report` checks evidence integrity while retaining nonpasses.
`--verify-full` requires the complete requested main matrix to match. The
Java-only test-buffer supplement and whole-language M2 gate are separate.

## Remaining obligations

Package and service-provider default transformations, Host callbacks from eager
synchronous constructors, complete exception/API parity, and cross-evaluator
standard-library execution state remain explicit implementation obligations.
Candidate `evaluateTest`/`TestResults.logs()` parity is unexecuted. The main
fixture covers a bounded evaluator contract; the whole language, API, and
performance gates remain open.
