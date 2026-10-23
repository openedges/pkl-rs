# Portable Resource reference fixtures

`tests/fixtures/m2-resource-portable` contains reference inputs and expected
results derived exclusively from four sealed Pkl 0.32.0 Java observations.
It selects 1,008 API calls in 20 sessions from 1,272 observed calls. The remaining
264 calls retain explicit exclusion records: independent ModuleSource calls in
the same evaluator, configured-property calls, and URI/policy or initial
capability boundaries outside this selection. These are unexecuted obligations
for the portable consumer, not successful compatibility checks.

The fixture contains 11,507 payload files plus `manifest.json`, for 11,508 files
in total. Original source URIs, input bytes, expression order, status, complete
stdout/stderr, UTF-16 code units and Java UTF-8 bytes remain unchanged. Each API
call also retains its reader counts and original Java report/capture identity.
Sixteen channels distinguish Java's unmatched-surrogate UTF-8 replacement from
JavaScript `TextEncoder`; both original byte and code-unit channels are retained.

The reader plan follows the observed Java reader URI, attempt and total count,
with return inputs derived from the pinned Java helper and resource payloads.
The event trace does not directly record return payloads. These plans are never
derived from Rust outputs. Java GC requests and policy/close callbacks are not
equivalent to portable per-poll collection or reader-count comparisons.

Run the offline verifier and its regression tests from the repository root:

```sh
python3 tools/resource_fixtures.py --fixtures tests/fixtures/m2-resource-portable
python3 -W error::ResourceWarning -m unittest discover -s tools/tests -p 'test_resource_portable_fixtures.py' -v
```

These commands need neither the original acquisition directories nor a JVM.
The historical absolute paths identify the original observations; the verifier
reads their preserved copies within the fixture. It independently pins each Java
report, inventory and closure, binds every selected input/call/event to those
pins, compares counters directly with the pinned report, and checks the exact
regular-file set. Rewriting the outer manifest cannot authorize changed oracle
bytes or extra files. Canonical paths are checked before directory traversal;
unexpected directories and links are rejected.

The regression suite mutates copies of real fixtures and recomputes their outer
manifest to test forged counts, reports, inventories, call/event provenance,
added or omitted files. Original fixture bytes are checked afterward.
`tools/export_resource_portable.py` preserves the original export procedure; it
requires the sealed acquisition directories and is not a fresh Java runner.

This checkpoint establishes reference-fixture integrity. Actual native, Node
and Chromium evaluation, the unselected boundaries, original programs and the
whole M2 gate require separate execution evidence. See the
[verification record](../reports/m2/resource-portable-fixtures.json).
