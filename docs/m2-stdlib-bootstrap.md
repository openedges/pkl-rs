# Standard modules and guest exception handling

The shared VM imports supported declarations from the pinned `pkl:base`,
`pkl:test`, and `pkl:ref` sources. Module identity, declaration owners, lazy
members, annotation initialization, and output evaluation use the evaluator's
existing code arena, heap, and Host continuations. External members bind to
Rust instructions; evaluation never invokes Java. The bootstrap is partial:
unsupported declarations remain explicit implementation boundaries and are not
reported as fully initialized standard modules.

`pkl:test.catch` and `catchOrNull` install handlers on the current invocation's
frames. The nearest applicable forcing or guest boundary determines unwinding.
Successful lazy values and partial initialization remain attached to their
owners. Subsequent uncaught diagnostics retain the required cached error state
and logical frames. Catch rendering follows the pinned Java profile without
hints, call frames, or power assertions, then applies Java line splitting and
whitespace rules to the UTF-16 string.

Ordinary Pkl errors and explicitly typed language exceptions from the Host can
reach these handlers. Cancellation, timeout, closure, unsupported operations,
Host protocol failures, logger failures, and arbitrary injected Host errors do
not become ordinary guest exceptions. `SecurityManagerException` is a distinct
typed resolve completion with an optional UTF-16 cause. An invalid source-load
completion of this kind preserves the pending token. This bridge is not an
implementation of Java's complete security policy.

The Stage 45 implementation also adds Boolean `xor`/`implies` and source-backed
`ModuleInfo` validation. Its Unicode decimal-digit table comes from the pinned
JDK observation. This narrow specialization does not replace the incomplete
general regex backend. See [ModuleInfo fixtures](m2-moduleinfo-fixtures.md) for
the distinction between ordinary calls, Java internal failures, and independent
root follow-ups that the current SourceProbe cannot execute.

## Verification

Fixture provenance and execution are separate checks. Preserve status, complete
UTF-16 units, Java UTF-8 serialization, and observable callback/read counts.
Neither matching text nor a passing fixture verifier establishes full API parity.

```sh
python3 tools/guest_catch_fixtures.py
python3 tools/moduleinfo_fixtures.py
python3 tools/security_denial_fixtures.py
cargo test -p pkl-runtime --test m2_guest_catch --test m2_guest_catch_oracle --test m2_moduleinfo_oracle --test m2_security_denial_oracle --locked
node --test tools/platform/guest-catch-session.test.mjs tools/platform/moduleinfo-session.test.mjs tools/platform/security-denial-session.test.mjs tools/platform/bootstrap-session.test.mjs
```

The portable runners consume actual recorded Wasm and N-API artifacts and the
pinned browser package. Use fresh output directories; the example paths below
refer to a separately acquired `tools/m1_platforms.py` result.

```sh
node tools/platform/compare-guest-catch.mjs target/m2/guest-catch-new target/m2/platforms-new/wasm target/m2/platforms-new/pkl_napi.node tests/fixtures/m2-guest-catch tools/platform/package.json
node tools/platform/compare-moduleinfo.mjs target/m2/moduleinfo-new target/m2/platforms-new/wasm target/m2/platforms-new/pkl_napi.node tests/fixtures/m2-moduleinfo tools/platform/package.json
node tools/platform/compare-security-denial.mjs target/m2/security-denial-new target/m2/platforms-new/wasm target/m2/platforms-new/pkl_napi.node tests/fixtures/m2-security-denial tools/platform/package.json
```

Guest-catch fixtures distinguish complete ordered calls from constructor-only
and terminal partial comparisons. ModuleInfo's 12 Java internal-error calls
remain explicit unsupported boundaries, and 56 same-evaluator independent-root
follow-ups remain outside the current adapter. Security fixtures preserve six
actual Java policy-denial causes across two JVMs; only the typed completion and
the recorded API calls are compared in Rust.

The broader bootstrap comparison preserves unsupported reflection and JSON
renderer calls, resource-read count differences, and JVM-global standard-module
cache differences. Exact agreement between Node and browser does not turn those
Java mismatches into passes. Original language cases requiring these features
remain M2 obligations. General reflection, standard modules, loading, output,
product APIs, platform coverage, and performance remain unfinished.
