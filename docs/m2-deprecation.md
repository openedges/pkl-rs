# Method deprecation warnings

Stage 54 is implementing Pkl 0.32.0 method-deprecation warnings in the shared
engine. The [Java fixture](../tests/fixtures/m2-deprecation/README.md) establishes
the reference observations. It does not establish Rust compatibility, completion
of the full M2 language gate, or achievement of the performance targets.

## Java reference checkpoint

The independent authority archive contains two separately acquired experiments:

| Experiment | Sessions | JVM processes | Public calls | Observations |
| --- | ---: | ---: | ---: | --- |
| Main logger matrix | 56 | 112 | 310 | 228 successes, 66 language errors, 10 oracle errors, 6 API exceptions |
| Sequential evaluators in one JVM | 6 | 12 | 96 | 48 evaluator instances; all calls succeed |

Each experiment has two fresh-process repetitions per session. All 62 repetition
pairs have identical raw observations. The main matrix records 158 warning and
118 trace callbacks; the evaluator-cycle experiment records 38 warning callbacks.
These are execution counts, not independent language features or Rust passes.

The main matrix uses `Evaluator.evaluateExpressionString` with the exact supplied
module contents, URI, and ordered expressions. Each public query creates a fresh
query AST while module method bodies retain their executable call sites. The
logger captures UTF-16 code units and Java UTF-8 bytes for messages, results,
errors, module URIs, nullable member names, and source lines. It also captures
inclusive source positions and the order of calls and callback entry/exit.

Cases cover direct lexical, virtual, and super calls; argument, arity, parameter,
return-type, and body failures; repeated module sites and fresh query sites;
closure and generator instances; polymorphic receiver caches; logger rejection,
reentry, and closure; annotation forcing; and implicit calls from interpolation,
API string conversion, Reference rendering, and output renderers.

Logger rejection and callback closure can produce actual Java `PklBugException`
results. Later calls after closure can produce `IllegalStateException`. The raw
messages, stack traces, and exception-class provenance are retained. These
observations remain nonpasses for compatibility; ordinary error messages or
synthetic Java exception names must not replace them.

## Invocation and lifetime obligations

The warning belongs to an executable invocation site and its selected method
target. A global set of method names or source spans does not implement the
reference behavior. Lexical target binding occurs after argument evaluation;
virtual and super binding occurs before arguments. A successful logger callback
can publish a target even if the later method call fails.

Virtual sites cache receiver classes, including distinct classes that inherit
the same method. The pinned generated JVM bytecode and actual callback controls
also demonstrate observable publication retries when a logger reenters the same
site with another receiver class. Empty-cache and warm-cache saturation during
reentry require the original cache chain and publication order to be respected.
Receiver instances and arbitrary activations must not become permanent cache
roots. Obsolete class and code roots must be released when their sites retire.

Implicit `toString` and standard-library calls have their own authored sites.
The Java default frame transformer exposes standard-library GitHub source URLs
with line anchors; API result string conversion can use `source:unavailable`.
Exact positions, source lines, and URIs remain part of the raw comparison.

## Remaining scope and candidate verification

The shared Rust implementation and final native, Node, and Chromium comparisons
are in progress. Exploratory adapter failures and candidate mismatches are kept
in their original output directories. A fresh final acquisition, required Rust
checks, platform checks, and the eight original deprecated-language snippets are
still required before closing the implementation checkpoint.

The supplemental Java experiment proves that standard-library invocation caches
can survive evaluator closure within one JVM. Warning counts across four
sequential evaluators are:

| Invocation | Warning counts |
| --- | --- |
| Module method | 1, 1, 1, 1 |
| Typed interpolation | 1, 1, 1, 1 |
| `Reference` domain renderer | 1, 1, 1, 0 |
| `ModuleOutput.text` renderer | 1, 1, 0, 0 |
| `FileOutput.text` renderer | 1, 1, 1, 0 |
| `ModuleOutput.bytes` renderer | 1, 1, 1, 0 |

Do not infer the initial cache occupancy from these counts. Candidate parity
across evaluator instances remains an open M2 requirement. Fresh processes or
Wasm instances for the main matrix do not verify that separate lifecycle.
A separate boundary acquisition records 11 cases in 22 fresh JVM runs, with
identical repetition bytes. When closing in an outer deprecated method's logger,
a subsequent deprecated inner call produces a Java internal error before its
warning reaches Logger. A plain inner method completes successfully; without
closure, both deprecated calls warn and complete. These controls preserve the
internal-error boundary rather than treating it as an ordinary guest exception.

The boundary acquisition also confirms that a custom frame transformer receives
raw `pkl:ref` or `pkl:base` URIs. Logger receives the frame returned by that
transformer, while the default transformer produces the GitHub URLs above.
Candidate custom-transformer parity for these standard-library frames remains
unimplemented. The main matrix uses the default transformer and cannot establish
this separate contract.

## Offline verification

The committed fixture can be checked without Java, a prior `target/` directory,
or the original-snippet packet:

```sh
python3 tools/deprecation_fixtures.py
python3 -m unittest discover -s tools/tests -p 'test_deprecation_fixtures.py'
```

The verifier pins the authority archive independently of the fixture manifest,
checks archive member and fixture file coverage, verifies the captured Java
artifact and source revisions, and recomputes observations from the original
bytes. Its success proves evidence integrity only. Missing candidate execution,
known differences, Java internal failures, and incomplete full-language mappings
remain visible in the M2 completion accounting.
