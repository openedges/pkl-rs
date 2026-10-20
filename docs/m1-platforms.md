# M1 Node and browser experiment

`pkl-wasm` and `pkl-napi` call the same `pkl-runtime` parser, resolver, compiler,
VM, primitive numeric/regex operations, and Host session. These crates expose
experimental probes, not complete evaluator products or the final public API.
No production evaluation path invokes Java.

The adapters transfer strings through validated UTF-16 conversion, signed
64-bit integers through BigInt, and bytes through owned Uint8Array copies.
Integers outside i64 are rejected rather than rounded or wrapped. Unpaired
UTF-16 surrogates are explicitly unsupported at this prototype boundary;
silently replacing them would conceal a compatibility gap. Full Java string
semantics remains a release requirement. Successful scalar evaluation returns
a small typed probe transport, not Pkl's product renderer.

The shared `HostProbe` class exposes start/reenter/poll/respond/reject/cancel/close
and a cancelled-read drain for
the bounded resource-read and arithmetic bytecode programs. `tools/platform/
scenarios.mjs` runs identical assertions in Node and inside a Chromium page:
actual frontend evaluation, Unicode strings, i64 endpoints and overflow, owned
bytes, delayed resource callbacks, suspended arithmetic, reentry, duplicate and
cross-session completions, error bytes, cancellation, close, and real timer
expiry. Fourteen required scenarios include nested reads in one evaluator and
live-operation cleanup. `host-adapter.mjs` gives each fake read an AbortController,
a real timer, and a Promise; it consumes the engine's cancellation queue to abort
the operation, remove its timer/listener and active-map entry, and reject its
Promise. Tests require zero retained operations, timers and listeners after
cancel, close and timeout, including two simultaneously outstanding nested reads.
Successful nested completion returns before the preserved outer read resumes.
This bounded adapter is not a production filesystem or HTTP implementation.
`loop` exercises cooperative cancellation after an instruction yield.
Its clock argument is elapsed milliseconds as u32, not an epoch timestamp.
Clock, timeout and budget inputs reject nonfinite, negative, fractional and
out-of-range JS numbers before narrowing. This probe clock range is about
49.7 days; the core accepts u64 time. The product adapter needs a longer-lived
clock API before long-running sessions can be claimed.

Reproduce with Node/npm, Python 3.12+, the pinned Rust toolchain and Java oracle:

```sh
python3 tools/oracle.py fetch
cargo install wasm-bindgen-cli --version 0.2.108 --locked --root .cache/m1-tools
npm ci --prefix tools/platform --ignore-scripts
npm exec --prefix tools/platform -- playwright install chromium
python3 tools/m1_platforms.py --output target/m1/platforms-new
```

The runner requires a fresh output directory. It builds the native probe and
Node addon, builds Wasm, generates web bindings with exactly matching
wasm-bindgen, and launches an actual headless Chromium browser through pinned
Playwright. Installed Playwright and playwright-core versions must match both
the package manifest and lock; the runner checks the recorded pin again.
It snapshots sources, rejects source drift, retains build/process
logs, exact executable artifacts, Java/native/Node/browser output captures,
and Node/browser version metadata. Primitive result payloads are compared as
raw bytes through a lossless hex-line transport; missing output is a failure.

The strict command returns nonzero for any Java mismatch. The separate
`--verify-contract` mode verifies that all four targets ran, all shared-engine
results agree, all platform scenarios passed, and the exact two documented
regex backend gaps remain accounted for. Its report still says compatibility
`fail` and counts those inputs as mismatches. It also fails when an expected
gap disappears, requiring review and an explicit contract update. CI uses this
architecture-experiment gate; it is not a language conformance gate.

The regex gaps are Unicode database version selection and zero-width matching
inside supplementary UTF-16 pairs. See [semantic decisions](m1-semantics.md)
for the rejected backend and the shared replacement strategy. A prototype
passing this platform contract establishes execution and transport evidence,
not complete language, API, performance, browser support, or npm packaging.
