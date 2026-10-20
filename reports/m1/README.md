# M1 architecture evidence

M1 implements a bounded shared Rust frontend, lazy VM, traced heap and Host,
with actual Node N-API and Chromium Wasm execution. It establishes architecture
contracts and records rejected candidates. It does not establish full Pkl
0.32.0 language/API compatibility or release performance.

M1 architecture verification is complete. The implementation is committed as
`ce99720` and experiment/CI integration as `e9ee8d5`. All five jobs in
[the final code CI run](https://github.com/luuvish/pkl-rs/actions/runs/39372369602)
succeeded. The downloaded artifact digest, exact source snapshots and raw
comparison results were independently verified; see the [CI summary](ci-summary.json)
and [471-file retained evidence](ci-evidence.tar.gz). Known compatibility and
performance failures below remain open requirements for subsequent milestones.

| Experiment | Observed result | Evidence and decision |
| --- | --- | --- |
| SourceMap, parsing, resolution | 15/15 exact bounded Java/Rust probe comparisons; 10 Rust regressions | [Frontend report](frontend.json), [archive](frontend-evidence.tar.gz), [contract](../../docs/m1-frontend.md). UTF-8 spans, UTF-16 locations, lexical owner/receiver separation selected; full grammar/error renderer remains unfinished. |
| Numeric and regex primitives | 6,076/6,078 Java/native matches; all sampled numeric cases match; two regex backend differences | [Semantic report](semantics.json), [archive](semantics-evidence.tar.gz), [contract](../../docs/m1-semantics.md). Shared checked arithmetic/Java formatting/audited fdlibm selected. Current general regex backend rejected; versioned Unicode tables and UTF-16 matching required. |
| Selected numeric costs | 96 exact pre-timing output matches and six converged processes; formatting about 6.1 times Java cost, cbrt/sin/checked-add lower | [Cost report](semantics-cost.json), [archive](semantics-cost-evidence.tar.gz). The formatter needs optimization before performance acceptance; these primitive loops do not establish evaluator performance. |
| Lazy VM and heap | Six success fixtures match raw channels; three diagnostic byte comparisons fail in the expanded experiment; 12 runtime regressions | [Expanded report](runtime-specialized.json), [initial functional report](runtime-functional.json), [runtime contract](../../docs/m1-runtime.md). Generation-tagged traced heap, suspended roots, memoization and separate compiled/evaluation lifetimes selected. |
| VM performance | Generic VM takes about 1.27 times Java API time; fused integer addition takes about 0.46 times on the same bounded workload; whole-process RSS is lower | [Generic measurement](runtime-performance.json), [specialization](runtime-specialized.json). Select checked scalar specialization as the bounded M2 baseline. Java warmups converge but target compilation bails out; this is not fully optimized Truffle parity or release performance. |
| Host lifecycle | Five real Java observation suites; seven native Host regressions | [Java report](host-java.json), [archive](host-java-evidence.tar.gz), [Host contract](../../docs/m1-host.md). Java callback reentry succeeds; single-continuation Rust Busy behavior remains a prototype difference. Product contexts must support nested evaluation. |
| Actual browser and Node | 6,078/6,078 shared-engine primitive outputs agree; each target has the same 6,076 Java matches and two failures; 12 required scenarios per environment | [Platform report](platforms.json), [archive](platform-evidence.tar.gz), [contract](../../docs/m1-platforms.md). BigInt/bytes/Unicode/real asynchronous calls and timers run in Node 26.10.0 and Chromium 156.0.8078.4. |

All 13 local validation commands passed, including the complete Rust workspace
checks, 114 Python tooling regressions, and source/corpus/probe freshness checks.
See the [validation report](validation.json) and [raw logs](validation-evidence.tar.gz).

Every primitive result is compared through a complete lossless hex transport.
Parser probes compare their explicitly declared message/location payloads;
they do not establish full CLI diagnostic rendering. The three runtime error
fixtures retain complete differing stderr bytes. The two general regex gaps
are `regex-unicode-15-vs-16` and `regex-zero-width-supplementary`; the seven
separate empty/dot cursor proofs support the selected UTF-16 model without
claiming those backend failures were fixed.

The platform runner's strict mode exits 1 for these mismatches. The separately
named architecture contract passes only when all targets/scenarios execute,
all shared-engine bytes agree, and the exact known-gap inventory is retained.
It still reports compatibility `fail`. This distinction is also explicit in CI.

The native and JS Host prototypes use cooperative cancellation. Their public
error messages, resource cache behavior, nested evaluation, and API surface
are not Java-compatible yet. Unpaired surrogate input is rejected explicitly
at the current JS boundary. The parser rejects unsupported constructs instead
of providing a complete language implementation. These remain release gates,
along with the full M0 inventories, product APIs and performance workloads.

Evidence archives retain source snapshots, provenance, raw successes and
failures. Reproduce with the linked contracts and fresh output directories.
Verify an archive with `python3 tools/evidence.py verify <archive.tar.gz>`.
