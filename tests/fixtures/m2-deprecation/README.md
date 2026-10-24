# Java method-deprecation observations

These fixtures record selected source-driven observations from the pinned Pkl
0.32.0 public Java `EvaluatorBuilder` and `Logger` APIs. They are Java authority,
not Rust compatibility results. The pinned reference uses unchanged `-Xms64m`
and `-Xmx512m`; no production evaluator calls Java.

`cases.json` records exact, unwrapped ordered query strings, module URIs, sources,
and logger actions. Each session uses one evaluator; each query calls
`evaluateExpressionString` and creates a fresh query AST. Module method and
closure bodies persist within that evaluator. Each session runs twice in fresh
JVM processes and evaluators. The Java query frame URI `repl:text` remains
unchanged; module-source frames retain `file:///stage54-deprecation.pkl`.

The main matrix has 56 sessions and 112 fresh JVM processes: 310 public calls
(228 successes, 66 guest errors, 10 oracle errors, and 6 API exceptions), 158
warning callbacks and 118 trace callbacks. All 56 fresh-process repeat pairs
match complete raw bytes. These are observations, not compatibility passes.

The compact `oracle-evidence.tar.gz` preserves complete process stdout/stderr,
exit status, callback ordering, Java exception classes, UTF-8 bytes and UTF-16
code units for result strings, messages, module URIs, member identities, and
source lines. Positions include all four inclusive Java frame coordinates.
Original Java UTF-8 uses `?` for unmatched surrogates; the captured UTF-16 units
remain the string authority for APIs that return strings. Java and JavaScript
UTF-8 encoders are not claimed to agree for these strings.

Logger callbacks evaluate the recorded callback expression once per session
through the same public evaluator, at the first outermost warning whose zero-based
index is at least `callback_after`. The driver marks the callback used before
reentering. `throw-once` throws an actual `IllegalStateException` from the
first warning callback. Java wraps it as `PklBugException`; these calls remain
`oracle-error`, even if another implementation reproduces their bytes. Callback
entry is recorded before action execution, so a thrown callback has no fabricated
successful exit. `close` and `close-reenter` use real `Evaluator.close()` calls.
Pending Host responses, explicit suspend/resume, GC stress, and independent
cancellation tokens have no matching public synchronous Java protocol here and
remain separate Rust lifecycle obligations. Close-induced cancellation is
observed through the actual public API.

The method cache controls include retained versus fresh query sites, lexical
versus virtual/super argument ordering, type/arity/body failure, inherited
receiver-class specializations, builtins consuming specialization slots, retained
closure and generator code, annotation message forcing, and nonmethod/local
annotation boundaries. No deprecated implicit-base method exists in the pinned
`base.pkl`; its direct invocation warning path is source-reviewed without a
fabricated positive Java event.

`cycles.json` is a separate six-session Java-only supplement. Each of its 12
fresh JVM processes builds and closes four evaluators sequentially while reusing
the same `ModuleSource`; each evaluator receives two exact queries. It retains
96 successful calls across 48 evaluator instances. The warning counts in the
four evaluators are `[1,1,1,1]` for a retained user method and typed interpolation,
`[1,1,1,0]` for Reference rendering, FileOutput text and ModuleOutput bytes, and
`[1,1,0,0]` for ModuleOutput text. The last pattern is preserved without inferring
which earlier receiver occupied another specialization slot. These are real
JVM-global stdlib cache observations. Candidate same-process evaluator parity is
not established here and remains a compatibility obligation. The archive keeps
`main/` and `cycles/` provenance separate; no main matrix case was rewritten.

A second deprecated call after a logger closes the evaluator remains unverified;
the recorded close cases instead encounter a real Java internal error before
that boundary. No successful callback is invented for an unreachable state.

The eight upstream original CLI annotation cases are acquired separately with
their exact corpus arguments, environment, resources, paths, and filesystem
channels. This public API fixture neither substitutes for nor depends on that
separate comparison packet. Initial failed authoring probes remain in fresh
Stage54 acquisition directories and the checkpoint evidence archive.

Run the standalone offline audit from any checkout containing these files:

```sh
python3 tools/deprecation_fixtures.py
python3 -m unittest discover -s tools/tests -p 'test_deprecation_fixtures.py'
```

The verifier checks an independent archive digest, complete file coverage,
pinned Java authority, captured source/query identities, every callback and call
channel, and fresh-process repeat bytes. It needs neither Java nor the original
`target/` acquisition directory. New Java acquisition uses a fresh directory:

```sh
python3 tools/m2_deprecation.py --output target/m2/new-deprecation-oracle
python3 tools/m2_deprecation.py --verify-report target/m2/new-deprecation-oracle
python3 tools/m2_deprecation.py --cycles-output target/m2/new-deprecation-cycles
```

Successful acquisition or repeat equality establishes evidence integrity, not
whole-language, product API, platform, or performance completion. No known
failure or unmatched lifecycle contract is promoted to a pass.
