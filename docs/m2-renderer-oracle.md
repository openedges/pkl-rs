# Pinned renderer API observations

This checkpoint publishes Java Pkl 0.32.0 observations and verification tooling. It does not establish native, browser, or Node compatibility, complete renderer support, full M2 completion, or performance results. Actual native and platform comparisons are separate evidence.

The self-contained fixture lives in `tests/fixtures/m2-renderer-portable`. It contains 14,995 payload files plus `manifest.json`. The original source URIs, inputs, expressions, `fixtures.json`, and `provenance/` bytes are preserved.

The bundle contains 860 API calls in 44 persistent evaluator sessions. Each authored case has two fresh JVM repeats, each containing two fresh evaluators. Expressions within a session share the evaluator and its lazy module/resource state. These are actual `Evaluator.evaluateExpressionString` observations; the original whole CLI inputs are separately captured and remain unchanged.

| Selected controls | APIs |
| --- | ---: |
| Fresh scalar conversion hint identity | 40 |
| JSON scalars, layout, unsupported values and errors | 116 |
| Renderer option getter/value forcing and typed reader retry | 36 |
| JSON UTF-16 escaping | 328 |
| Corrected document/value converter errors and hints | 72 |
| Corrected RenderDirective text, lazy bytes and undefined paths | 56 |
| Persistent primitive converter function history | 44 |
| Parent/child annotation order and null omission | 24 |
| Reference join callback and typed I/O retry | 24 |
| JSON Property, original path priority, transformers and nested renderValue | 40 |
| Collection join order, object forcing, errors and UTF-16 separator | 80 |

Run the offline verifier and its regression suite from the repository root:

```sh
python3 tools/renderer_fixtures.py --fixtures tests/fixtures/m2-renderer-portable
python3 -W error::ResourceWarning -m unittest discover -s tools/tests -p 'test_renderer_fixtures.py' -v
```

These commands need neither a JVM nor the original acquisition directories. The verifier pins the original producer report, inventory and closure SHA-256 values independently of the outer fixture manifest. It checks source/helper identities, every copied raw payload against the pinned inventory, both stdout/stderr channels as original Java UTF-8 and UTF-16BE, exact status bytes, and reader counts against both raw files and the pinned report counters. Reader plans are reconstructed from the actual Java reader trace and authored input table. No Rust result supplies an expected value. The `--output` exporter preserves the historical export procedure and requires the original acquisition directories; it does not launch fresh Java evaluations.

The verifier derives the exact payload paths and ancestor directories from authenticated Java evidence before walking the fixture. It rejects undeclared directories before descent, links, non-regular payloads, missing files and altered or extra payloads. Rewriting the outer manifest cannot authorize changes. The tests modify isolated copies, recompute their outer manifests and verify that the original fixture remains unchanged. `RENDERER_PORTABLE_GOLDEN` optionally selects a different fixture directory; ordinary test discovery uses the repository fixture.

The Java UTF-8 encoding of an unpaired UTF-16 surrogate is `?`; it must be compared independently of the preserved UTF-16 units. Trimming errors, sorting events, replacing diagnostic frames or normalizing strings would invalidate this comparison.

Java records explicit `System.gc()` requests. They do not prove that a value was collected. The native observer requests collection after each poll; each platform must record its own collection behavior and retained roots. The initial portable fixture does not represent evaluator-after-close operations, callback reentry, logger callbacks or cross-evaluator JVM static identity.

The original evidence preserves authored mistakes and corrective acquisitions. Oracle01 failed javac before any of its 648 planned APIs. Oracle02 includes 72 parser rejections from an invalid Dynamic declaration and 16 nested query lexical errors; its valid Directive calls were superseded for fixture selection by a complete corrected group. Oracle03 includes 44 `Any` converter observations that do not dispatch primitive type converters. Oracle05 preserves 120 reserved-identifier parser rejections, 48 direct query import-alias lookup failures and 36 associated policy sentinels. None of these is relabeled as intended renderer coverage.

Oracle06 also contains 84 actual stdlib security-policy APIs outside the default-policy fixture. Denying `pkl:json` produces an ordinary import error; the pinned base/ref/test singleton path succeeds under an allowlist excluding those singleton URIs. These policy profiles require a corresponding Host contract and remain unexecuted in the initial default-policy portable comparison.

Supplementary Oracle07/08 observations remain separate. They cover PCF Long cache boundaries, fresh equal-content String references, shared resource byte-array versus distinct Resource.bytes wrapper identity, captured primitive frames, and explicit evaluator closure. Oracle07's 16 mistaken bare function-property calls are preserved; Oracle08 uses `.apply` correctly. Post-close Java `PolyglotException` cancellation is an API boundary, not a guest-language error.

The bundle's historical field `unexecutedApiBoundaries` contains several dispositions: Java calls never executed, acquired but unselected observations, authored probes that failed before reaching their intended feature, and remaining implementation obligations. Read each entry's `kind` and `scope`; the field name does not mean every entry was unexecuted in Java.

`tools/platform/compare-renderer.mjs` consumes this verified bundle with separately built N-API and Wasm artifacts and pinned Playwright tooling. It compares full output/error units and Java UTF-8 bytes, status, reader counts and URI order, completion-payload ownership, and zero retained slots after close. It records policy-profile and GC-schedule boundaries explicitly. Actual execution results require a separate capture bound to the supplied build; adding the driver does not establish platform compatibility.

The staging verifier accepted undeclared empty directories and discovered unexpected files after traversal. The installed correction, traversal-spy regressions, repository-relative test default and original staging sources are recorded separately in the [verification record](../reports/m2/renderer-portable-fixtures.json). Native candidates, policy profiles, the complete renderer inventory and the full M2 gate remain separate obligations.
