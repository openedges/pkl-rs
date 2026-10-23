# Module keys and public evaluation

The shared runtime selects module keys separately from resolving and loading
their sources. A key has an engine-issued identity, a kind and an opaque Host
handle; its URI alone does not identify its implementation. Cache-policy queries
are separate Host operations. The engine owns successful import-site caches and
the original/resolved URI caches, including the Java rule that uncached keys
bypass lookup but still publish their results.

Import-clause preparation performs factory selection and name inference before
the import is evaluated. Runtime selection remains a separate operation. Static
standard-module singletons are distinct from ordinary URI caches: a custom key
whose URI is `pkl:base` or `pkl:ref` must not become a builtin key because of its
spelling. Supplied contents use a cached wrapper with the delegate's relevant
capabilities and the wrapper's own fragment-path behavior.

`SourceSession::module_evaluator` creates a Rust evaluator without parsing a
primary module or doing I/O. `start_evaluation` accepts a `ModuleSourceSpec` and
an expression; `reenter_evaluation` supports a suspended Host callback. The Wasm
and N-API probe bindings expose `SourceSession.moduleEvaluator` and
`evaluateModuleSource(uri, contents, expression, now, timeout)`. Passing absent
contents requests URI loading. These development interfaces do not establish
compatibility of the planned CLI, REPL or server products.

Each public query receives fresh executable state while retaining the evaluator's
module caches. Active and captured environments retain their executable owners,
so escaped closures and declarations keep the code they need. Collection retires
unreachable module/query payloads, including source and type data. This is not a
proof of bounded total memory: retired non-tail vector slots and some nested
capacities still accumulate. Reclaiming that storage while preserving live code
identities remains required for long-running sessions.

## Java reference fixtures

`tests/fixtures/m2-modulekey-spi` preserves 212 original Pkl 0.32.0 Java API
observations in 36 evaluator sessions and 2,248 ordered events. Its two cohorts
cover custom key identity, resolved aliases, cache-policy changes, custom keys
with standard-library URI spelling, and eager versus lazy factory failures.
The 132-call key/cache cohort and 80-call factory cohort remain separately
identified. Repetitions and sessions are not distinct language features.

The fixture retains the original sources, operation order, factory configuration,
UTF-16 code units, Java UTF-8 output and errors, counters, event order, and JVM
exception metadata. Original absolute source URIs remain unchanged. Copied
payload paths support offline verification without rewriting diagnostic identities.

The verifier authenticates literal hashes of the original Java report, inventory
and closure before reconstructing the fixture. It rejects changed expected
results even when a caller regenerates the local manifest, as well as missing
payloads, symbolic links and undeclared directories. Verification reads only the
copied fixture; exporting requires the original sealed oracle directory.

```sh
python3 tools/modulekey_fixtures.py --fixtures tests/fixtures/m2-modulekey-spi
python3 -W error::ResourceWarning -m unittest discover -s tools/tests -p 'test_modulekey_fixtures.py' -v
```

Successful fixture verification proves integrity, not evaluator compatibility.
Runtime comparisons must separately report complete API channels, actual typed
requests and key identities, and the full Java event/counter contract. A
supplied-primary development session does not execute Java's primary-module and
per-query lifecycle. An actual public entrypoint also requires its own ordered
event comparison before that lifecycle can pass. Neither profile may manufacture
Java factory, primary, query or implementation-class events to fill missing work.

Source-derived Host configuration, including inferred names and file bindings,
is separate from observed Java expected results. Factory selection must precede
the later cache-policy getter; dynamic factory configuration is captured when a
key is created. URI and successful import-site caches belong to the shared
runtime. These fixtures do not complete ModuleKey, module loading, `import*`,
platform, or M2 compatibility. See [the language gate](m2-language.md) and
[import admission](m2-module-admission.md).

The separate `tests/fixtures/m2-modulekey-host` directory supplies nine configured
Host profiles and seven unmodified pinned Java source copies. Its source-derived
facts include cache snapshots, selected/resolved/source URIs, the authored
factory IOException, and exact inputs for which the configured registry has no
loader. It does not add expected results to the authenticated Java fixture.
Relative source-copy paths support a fresh checkout; historical absolute paths
remain provenance. Upstream license and notice files are included.

The portable comparison tool accepts explicit `spi132`, `factory80`, or `all212`
cohorts and supplied-primary or public-URI entrypoints. It records actual API
operations, delivered requests, Host completion calls and returns, explicit
fixture GC/cache operations, and close in one ordered diagnostic stream. A
default completion returning `false` leaves the request pending; a configured
NoLoader rejection completes that same token. Unknown Host key identities are
rejected before inherited default handling. These observations do not recreate
Java factory class labels or establish its full event/counter contract.

```sh
node --test tools/platform/modulekey-session.test.mjs tools/platform/compare-modulekey.test.mjs
```

These tests check adapters and evidence validation with mocked sessions. Actual
Node/Chromium acquisition requires separately recorded builds, source bindings,
and explicit artifact/browser hashes; a preflight-only run does not execute
either engine.

The older module-loader adapter also models a bounded virtual File factory:
hierarchical `file:/...` key selection does not check whether a source exists.
This allows an unused missing import and the supplied main module to be selected
without adding a resolve or load operation. Actual resolution and source lookup
still occur only when requested. Its accepted URI profile is narrower than all
Java File URI cases; it is not a complete filesystem Host. Additional typed
requests remain visible in the older fixtures' strict counter comparisons.
