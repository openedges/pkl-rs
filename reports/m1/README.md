# M1 architecture evidence

M1 implements a bounded shared Rust frontend, lazy VM, traced heap and Host,
with actual Node N-API and Chromium Wasm execution. It establishes architecture
contracts and records rejected candidates. It does not establish full Pkl
0.32.0 language/API compatibility or release performance.

An independent completion review reopened M1-04 and M1-06. The closing changes
now recover failed lazy members, implement nested contexts in one VM and heap,
and propagate terminal cancellation to live platform operations. Closing code `6a5e4b5` passed all
five jobs in [run 39374668362](https://github.com/luuvish/pkl-rs/actions/runs/39374668362).
Its downloaded artifact digest, exact committed source snapshots and raw results
were independently reverified. See the [closing CI summary](ci-closing-summary.json)
and [retained evidence](ci-closing-evidence.tar.gz). The earlier green CI below
did not cover these missing contracts.

A subsequent review found the originally assigned Graal compilation investigation
still incomplete. The [closing investigation](../../docs/m1-graal.md) now identifies
the virtual/materialized frame merge in `VmUtils.skipInvisibleScopes`, with
compiler graphs and separate negative and causal controls. Actual arithmetic and
recursive Pkl functions compile through Tier 2 on the unchanged baseline when
their input is read through a receiver field. Semantic-bypass controls also
compile the original fixtures; those overrides are diagnostic only. This closes
the investigation requirement without replacing the original failing baseline
or claiming a new performance result.

The initial implementation is committed as
`ce99720` and experiment/CI integration as `e9ee8d5`. All five jobs in
[the initial code CI run](https://github.com/luuvish/pkl-rs/actions/runs/39372369602)
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

The table above preserves the initial experiment results, including its original
failure-cache and single-context limitations. Closing evidence supersedes those
two contracts without rewriting the historical captures:

| Closing check | Result | Evidence |
| --- | --- | --- |
| Failed lazy retry and callback reentry | Two complete Java/Rust observation protocols match. Four diagnostic payload differences remain explicit. A separate Java-only nested-new-resource read raises the observed upstream cache-mutation error. | [Lifecycle report](lifecycle.json), [archive](lifecycle-evidence.tar.gz), [contract](../../docs/m1-lifecycle.md) |
| Shared native/Node/browser runtime | All 6,078 primitive outputs agree across Rust targets; 6,076 match Java on each target. Fourteen required scenarios per JS environment verify nested reads and abort, Promise settlement, and timer/listener/map cleanup for single and nested operations. | [Closing platform report](platform-closing.json), [archive](platform-closing-evidence.tar.gz) |
| Integrated local checks | All 13 commands pass: 124 Python tests, 45 Rust tests, formatting, lint, Wasm compilation and inventory checks. One frontend probe is intentionally ignored by ordinary cargo test and invoked explicitly by its differential runner. Host and runtime suites contain 15 and 13 tests respectively. | [Closing validation](validation-closing.json), [raw logs](validation-closing-evidence.tar.gz) |
| Graal compilation investigation | Original arithmetic and recursive failures traced to an invisible-scope loop that merges virtual and materialized frames. Separate causal controls and unchanged-baseline receiver workloads demonstrate actual guest Tier 2 compilation; every result is checked against baseline bytes. | [Diagnostic report](graal.json), [raw cases](graal-evidence.tar.gz), [compiler graph evidence](graal-cause-evidence.tar.gz), [contract](../../docs/m1-graal.md) |
| Graal tooling integration | All nine diagnostic cases meet their declared contracts, retaining five expected compilation-failure cases. All 13 local checks pass, including 141 Python and 45 Rust tests. Relocated archived evidence is independently reverified. | [Validation](validation-graal.json), [raw check logs](validation-graal-evidence.tar.gz) |

The original [validation report](validation.json) and
[raw logs](validation-evidence.tar.gz) remain available. Earlier performance
measurements belong to their retained source snapshots; closing lifecycle work
does not claim a new performance measurement or achievement of release targets.

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
error messages, full module/resource cache behavior, and API surface remain
unfinished. Nested contexts and successful member-cache sharing are now verified.
The Java-only new-resource callback failure is retained as a later resource-cache
compatibility requirement; nested new reads in the Rust platform probe are not
a Java parity claim. Unpaired surrogate input is rejected explicitly
at the current JS boundary. The parser rejects unsupported constructs instead
of providing a complete language implementation. These remain release gates,
along with the full M0 inventories, product APIs and performance workloads.

Evidence archives retain source snapshots, provenance, raw successes and
failures. Reproduce with the linked contracts and fresh output directories.
Verify an archive with `python3 tools/evidence.py verify <archive.tar.gz>`.
