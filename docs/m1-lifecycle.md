# M1 closing lifecycle experiment

This experiment closes the failed-lazy-retry and callback-reentry questions
reopened by the M1 completion review. It compares bounded observations from the
pinned Java 0.32.0 evaluator and the Rust VM/Host. It does not establish complete
language, resource-cache, public API, diagnostic, or performance compatibility.

## Compared behavior

The retry fixture has two lazy members backed by readers that each fail once.
Four evaluations use the same evaluator and object. The first error prevents
the second reader from running. The next attempt retains the first member's
successful result when the second reader fails. The third attempt recovers, and
the fourth reuses both successful values without calling either reader again.
Failure is an evaluation result, not a permanent member-cache entry.

The supported reentry fixture prewarms a seed resource. During an outer reader
callback, Java evaluates a shared member on the same module; Rust enters a
nested context on the same object. The inner result returns before the outer
continuation resumes. Later reads reuse the member computed by the inner
evaluation. Both probes also observe idempotent close and post-close rejection.

Java parses the fixture source. Rust explicitly lowers the two exact, byte-checked
fixtures to its bounded bytecode because general `read(...)` parsing is outside
the current frontend. This checks evaluation behavior and state sharing; it is
not source-language coverage for resource reads.

The runner compares complete deterministic observation-protocol stdout bytes,
stderr and exit status. It also retains full error payloads and exception classes
separately, compares the error payload bytes, and reports their differences.
Protocol equality never converts a diagnostic mismatch into a compatibility pass.

## Observed Java resource-cache limitation

A separate Java-only negative observation keeps the initially attempted case:
an outer reader callback starts an evaluation that reads a new resource. Java
0.32.0 raises a `PklBugException` with a `ConcurrentModificationException` in the
captured diagnostic. The pinned `ResourceManager` populates its cache through
`computeIfAbsent`; nested population changes the same map. This source-based
explanation is separate from the preserved observed exception.

The successful reentry case above uses a previously cached resource. Rust's
bounded Host protocol can suspend nested new reads, but those platform scenarios
are adapter capability evidence, not parity with Java's complete resource cache.
M4 must account for the observed Java behavior when implementing that cache.

## Reproduction and evidence

```sh
python3 tools/m1_lifecycle.py --output target/m1/lifecycle-new
python3 tools/m1_lifecycle.py --check-report target/m1/lifecycle-new
python3 -m unittest discover -s tools/tests -p test_m1_lifecycle.py -v
```

Use a fresh output directory. The runner snapshots sources before compilation,
builds Rust in an isolated Cargo target directory, records the executable emitted
by that build, and verifies source stability before and after execution. It
retains the exact Java oracle identity, fixture bytes, artifacts, raw captures,
diagnostics and a complete evidence inventory. Report verification checks those
identities and recomputes observations from the captured bytes.

Native regressions separately check nested GC roots, failed-start rollback,
ancestor deadlines, first-error order, and complete context cleanup. Actual
Node and Chromium tests use abortable fake operations with real timers; they
observe cancellation, Promise settlement, and removal of timers, listeners and
active-operation records. See [the Host contract](m1-host.md) and
[platform experiment](m1-platforms.md).
