# Stage58 final evidence audit

This review binds the bounded Stage58 checkpoint to the exact local documents and four published evidence bundles. The independent review found no remaining factual blocker after correcting three original first-error descriptions. The implementation commit binding is still pending at this pre-commit point.

## Document identities

| File | SHA-256 |
| --- | --- |
| `README.md` | `75e6af900dddd8475266cedd3e25037ed5e6ea42962aff97af9bf9e5e7623520` |
| `TESTING.md` | `2a64fafc44fc01740d0468fb6404356edddb79c830f092332a59eea9288cd18a` |
| `PLAN.md` | `5ebe39fd48432fc2086f0a04a4875102c5a7cc6a78ccbf083a330a65df01523a` |
| `docs/m2-any-if-non-null.md` | `deb63a2d03145b87a99580cd35f02851ec3753263906e83bab818af6fed70775` |
| `reports/m2/README.md` | `93c0db07b17191c8c68d26df3d48c05e1fcaedd7512bc48cc9f21638382d275a` |
| `reports/m2/stage58-implementation-checkpoint.json` | `cde5005c2273bb74eb08904ea849bbbdb9fecfc8f574dd4ddc8d17fab5f78d15` |
| `target/m2/stage58-final-audit-01/review.md` | `9de78e9b427ae31ede3d82a01b009ae7b3d1ac51aa9196ae6d87b00396919185` |

## Published evidence

Each publication records every archive member SHA-256, size and mode, a full member readback, and stable source file-set identities before and after publication.

| Bundle | Archive SHA-256 | Publication SHA-256 | Members |
| --- | --- | --- | ---: |
| `validation` | `33fb97a3f59b79b57f5be39a8ab69703a3cf74a590fdd48bc1ae3387232763bb` | `4cc37981553e4eb49ce56d73c5e8a76c3aade13f9c0c86e0c659afa475494b7e` | 123 |
| `native` | `8d3a65d7936534ec297f9d9f55055b16cdcc7e22f828412836219f01faeaa47b` | `06ba666f115955a8ec1d0ce0954c94ce4df9d5ad7748cfacc199883d00864517` | 7,796 |
| `platform` | `9502de123f58faf07998d5453af3150e0661a41e9ca1f38dfa5f53d2919d512a` | `1e51d9d01b2bb98e0cd46dc45aa33c88a38d0018295ae1076ee2fbdfe7468c95` | 15,605 |
| `closeout` | `3c890d93e9ee02a9be3a5e50000e3c22d3ec72fae6e8c2863ae4feabfcc5d535` | `83d1b454310e58c3fef564d98f367c6d324ae2af1857de1844911d164a82439b` | 4,817 |

## Recorded outcome

- Canonical Java authority: 86 rows, two fresh JVM sessions. Native, N-API, and Chromium each match 168 of 172 raw string executions. Regex and IntSeq constructor rows mismatch twice per target; strict string compatibility fails while integrity passes.
- Additive Java authority: 13 rows, two sessions; all 13 exact results are covered by Rust regression tests. Separate portable supplement comparison was not run, and the historical Python helper input closure is incomplete.
- DataSize regression: 510 of 510 raw string executions match per native, N-API and Chromium target. Typed exports remain unexecuted.
- Original unchanged modules: 17 Java and 17 Rust executions, one each per case; 10 match and seven mismatch. The original `api/any.pkl` now matches.
- Five required Rust commands passed locally with 929 test executions passed, zero failed, and three ignored. Tool checks cover 20 distinct Python tests and 17 JavaScript tests; the initial failed Python invocation remains recorded.
- M1 architecture contract passes, while strict compatibility fails at 6,076/6,078 Java matches per target. Full M2, complete public APIs, release platforms, speed and peak-memory gates remain open.

The [independent review](../../target/m2/stage58-final-audit-01/review.md) records the count and claim checks against raw capture reports. This is local macOS arm64 evidence, not hosted CI.
