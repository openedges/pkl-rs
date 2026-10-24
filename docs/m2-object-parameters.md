# Object-body parameters and Mixins

The shared evaluator preserves parameters on object and `new` bodies and uses
them when amending a function. Parameters are checked on the first call. The
parent function receives the original arguments; the amended body captures the
checked values. Further calls to a curried parent retain those first-call
bindings. Both kinds of values are traced while evaluation is suspended.

Object properties take precedence over captured body parameters. Eager generator
headers, computed keys, spreads, and predicates resolve outside the object-body
parameter scope; lazy member values retain that scope. Empty bodies, nonfunction
parents, parameter counts, and nested typed or collection bodies have separate
validation and error-order rules.

`Mixin` defaults depend on the actual builtin alias identity. A structurally
similar function type does not acquire that default merely because its argument
and result types match. Explicit and inferred `new` expressions require separate
comparisons, including aliases that delegate their defaults.

## Java evidence and candidate comparisons

The fixture directory `tests/fixtures/m2-parameterized-object-bodies` preserves
Java 0.32.0 source inputs, expressions, original source URIs, resources, complete
stdout and stderr, process outcomes, and repeated captures. Its standalone
verifier pins the independent Java reports and probe artifacts as well as the
fixture file set. It does not require the original acquisition directories.

```sh
python3 tools/parameterized_object_fixtures.py
python3 -m unittest discover -s tools/tests -p 'test_parameterized_object_fixtures.py'
```

The current fixture contains 85 ordinary cases (56 values and 29 language
errors), seven separate oracle-error boundaries, and two original CLI captures.
Two Java processes back each ordinary case. Historical superseded and unreached
observations remain recorded separately and do not increase semantic coverage.

Build the native probe and use the binding artifacts from a fresh recorded
`tools/m1_platforms.py` run:

```sh
cargo build -p pkl-runtime --example expression_fixture_probe --release --locked
python3 tools/parameterized_object_compare.py --output target/m2/parameters-new --native target/release/examples/expression_fixture_probe --wasm target/m2/platforms-new/wasm --napi target/m2/platforms-new/pkl_napi.node
```

The comparison runs two fresh sessions per ordinary case and target, preserving
170 observations each for native, N-API, and Chromium. Its requested-target
verdict excludes the seven explicitly retained Java internal-error boundaries;
it is not the full compatibility verdict. Omitting a target leaves that target
unexecuted. The native probe and portable adapters use compact tracing, a noop
logger, the default frame transformer, and disabled power assertions, matching
the captured Java helper profile. Resource callbacks supply only the declared
raw fixture bytes and record their actual requests.

Fixture integrity is separate from a Rust compatibility result. Comparisons must
evaluate the original arbitrary expressions through the public shared evaluator;
the older property-path language probe cannot cover the whole fixture. Retain
every candidate value, error, Host request, and execution failure in a fresh
output directory. Compare complete channels without rewriting source locations,
trimming whitespace, or replacing expressions with simpler property accesses.

The fixture keeps Java internal errors separate from ordinary language errors.
These include parameterized `for` and `when` bodies for which the pinned Java
runtime cannot find its diagnostic message key. They remain incomplete
compatibility boundaries, including a `when` body whose condition is false.
Successful original CLI captures with empty output establish only that original
command's behavior; expression cases provide separate observable semantics.

## Lifecycle and remaining scope

Native regression tests exercise actual resource reads during parameter checks
and parent calls, lazy closures escaping currying, nested public evaluation,
failed-read retry, cancellation, and collection around each poll. A repeated
query test checks retained code, source, and heap ownership at two iteration
counts and requires terminal release. These checks do not measure performance.

Whole-language completion still requires the remaining callable, object,
generator, type, diagnostic, and original-module contracts. Selected fixture
matches and lifecycle checks do not close the full M2 gate or establish full
CLI, browser, or Node API compatibility.
