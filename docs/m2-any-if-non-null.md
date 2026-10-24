# Any.ifNonNull development contract

Pkl 0.32.0 is the compatibility authority. Stage 58 adds `Any.ifNonNull` to
the shared Rust VM for the value kinds that the bounded evaluator can create.
Native, Wasm, and N-API use this implementation. The selected expression-string
comparison is one part of M2; typed exports, complete public APIs, other
language behavior, release platforms, and performance remain open.

## Shared behavior

The method is inherited by values including scalars, collections, functions,
classes, modules, `Reference`, `DataSize`, `Null`, and `Null` with a default.
The VM evaluates the argument and checks the `Function1` boundary before the
method body tests the receiver. Callback parameter and return checks happen
when the callback is invoked; for example, `null.ifNonNull((x: String) -> x)`
succeeds because the callback is skipped. For a null receiver, the method
returns that same value without invoking the callback. Retaining
the `Null` value also retains its attached default and lazy amendment behavior.
For a non-null receiver, the VM calls the callback once with that receiver and
uses the ordinary call and return-check paths. Callback errors retain their
source frames and first-error order.

Objects search declared and inherited methods before the shared `Any` method.
Overrides, `super`, local method lookup, and implicit calls retain their normal
lookup origins. Const restrictions depend on that lookup path: a const literal
initializer, an implicit module call in a const method, and `this.ifNonNull`
inside a const class method succeed in the canonical Java controls, while an
explicit `module.ifNonNull(...)` call from a const method produces the normal
non-const method diagnostic. Same-named properties do not replace method
lookup. Function and `DataSize` diagnostics include this method among
applicable suggestions.

Native VM and Host lifecycle tests cover resource reads during argument and
callback evaluation, cooperative yields and collection, captured and escaped
closures, null-default identity, skipped callbacks, retries, and terminal heap
cleanup. These tests exercise the shared engine and Host boundary; they do not
stand in for a complete platform API comparison.

## Java authority and selected comparisons

The [canonical Java fixture](../tests/fixtures/m2-any-if-non-null/README.md)
contains 86 ordered public `evaluateExpressionString` queries, each run in two
fresh JVM sessions. Each session has 61 successes and 25 complete Java errors.
The [additive fixture](../tests/fixtures/m2-any-if-non-null-supplement/README.md)
contains 13 more queries per session: 12 successes and one expected type error.
It covers return checks, implicit receivers, function defaults, and escaped
closures. The supplement does not change the canonical selection. The
[Java checkpoint](../reports/m2/stage58-java-checkpoint.json) binds sealed raw
archives, source, JAR and JDK identities, offline fixture checks, and inventory
freshness. Its supplemental archived `run.py` imported a Python helper that
was not historically copied or hashed; complete historical Python helper
closure is therefore not claimed. The Java probe, authored inputs, runtime
identities, and raw observations remain validated.

The native, N-API, and Chromium canonical comparisons each match 168 of 172
query executions across two repetitions. Four executions per target, the
repeated Regex and IntSeq constructor rows, remain mismatches because those
constructors are unsupported in the bounded evaluator. The portable verifier
passes its integrity gate, while its strict string gate fails on these four
rows per target; the entire session protocol differs at the corresponding
failure statuses. The 13 supplemental observations also pass their focused
Rust test. The existing DataSize string comparison still matches 510 of 510
selected executions on each of native, N-API, and Chromium after this change.
The original upstream module selection completed 17 unchanged programs: ten
raw passes and seven mismatches. `api/any.pkl` now passes its original raw
stdout, stderr, exit, and generated-file comparison; Bytes, Duration, and Pkl
Binary cases remain among the mismatches. The recorded Rust validation completed
with 929 tests passed, none failed, and three ignored (the two new unsupported
constructor cases and one existing probe). The final tooling checks cover 20
unique Python tests (six fixture, nine native, five portable) and 17 JavaScript
tests. These counts are selected checks, not a whole-language completion
fraction.

Each common query comparison uses seven exact raw channels: expression UTF-8
and UTF-16 code units, status, stdout UTF-8 and UTF-16 code units, and stderr
UTF-8 and UTF-16 code units. The verifier also checks the entire ordered
session protocol and candidate lifecycle trace. Java error class is producer
metadata; the candidate does not synthesize it. Candidate failures remain
failures even when some error text resembles Java output. No trimming,
reserialization, or partial diagnostic comparison is used.

## Replaying the bounded checks

Verify Java fixtures offline and run their six integrity tests with:

```sh
python3 tools/any_if_non_null_fixtures.py
python3 tools/any_if_non_null_supplement_fixtures.py
python3 -m unittest discover -s tools/tests -p 'test_any_if_non_null*fixtures.py' -v
```

For an already recorded native acquisition, use a fresh output directory for
new captures and the matching recorded helper build:

```sh
python3 tools/any_if_non_null_compare.py --acquire target/m2/any-native-new --build target/m2/any-build-new --gate integrity
python3 tools/any_if_non_null_compare.py --verify target/m2/any-native-new --gate string
```

For the portable string boundary, first supply a fresh `tools/m1_platforms.py`
report containing the actual N-API and Wasm build artifacts. Then capture two
complete sessions per target with the pinned Playwright/Chromium installation:

```sh
node tools/platform/capture-any-if-non-null.mjs target/m2/any-portable-new target/m2/m1-platform-new/wasm target/m2/m1-platform-new/pkl_napi.node tests/fixtures/m2-any-if-non-null
python3 tools/any_if_non_null_portable_compare.py --capture target/m2/any-portable-new --m1 target/m2/m1-platform-new --gate integrity
python3 tools/any_if_non_null_portable_compare.py --capture target/m2/any-portable-new --m1 target/m2/m1-platform-new --gate string
```

The portable verifier checks the supplied M1 source/build report and exact
artifact hashes. The Chromium protocol is returned from a browser page, while
page and console errors are captured separately; they are not browser process
stdout or stderr. `integrity` accepts complete, source-bound evidence even
when rows mismatch. `string` requires all 86 rows to match in both repetitions
on each target. `full-api` remains unsuccessful because typed export comparison
has not run. The four constructor mismatches keep the strict string gate
unsuccessful for native, N-API, and Chromium.
The strict M2 gate and performance targets remain open; see the
[whole-language contract](m2-language.md) and [testing contract](../TESTING.md).
