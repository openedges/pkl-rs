# Stage 56 final comparison evidence audit

This is an independent, read-only audit of preserved macOS arm64 evidence. It ran offline Python assertions only. No Java or Rust guest, browser capture, Cargo command, comparison driver, or source edit was performed. The earlier `stage56-final-comparison-audit-01` remains unchanged. Audit scripts and machine-readable results in this directory show the exact checks.

## Evidence identity and provenance

| Evidence | SHA-256 |
| --- | --- |
| `stage56-core-checks-04/core-freeze.json` | `80aa1d1b915f688c136bec9894bed41df155f59efa168d8ea40f87baa283897b` |
| `stage56-rust-validation-03/report.json` | `d23984983a94f271dfe023574130e28e9b0b1f6be8b4165021455bc5c2b9532b` |
| `stage56-platform-run-01/report.json` | `752947eb60da8df0c07cee15db0bd685051e05c8b6635bf08f9cf01dfe92509f` |
| `stage56-platforms-01/report.json` | `134a712eb5e56aef542427787a81ec81b01502cea213c90eef667d09d0b9f2a3` |
| `stage56-class-platform-01/report.json` | `d9427eb5ee2c60e71a2b3c8fda713c12fc45cae48e1195a37069fc0e34a2f773` |
| `stage56-metadata-platform-01/report.json` | `e15aab77ffcb2dff55d89aefbbb89d3d9061b44d57b11b29eaf9684244d8bf61` |
| `stage56-original-class-01/report.json` | `e952b63063d94e3809483b78439de54138a38e4c9a70f67ae613210fee1b0743` |

The frozen native Class probe is `84548c956e549404967a7ead18be5c93d34988fca51be630a41a9e56ec713a9e`. The separately built release `module_eval_probe` used for original CLI cases is `b0fbb60f86d0f80300e9c942cee43075437b295703a4d0b824281915628101ea`. Both artifact hashes and build bindings were checked. The 37 core freeze inputs match both packet source snapshots and the original CLI capture. Each Class packet's 4,570 frozen source files and complete inventory were hashed; 246 shared release build inputs match exactly. The one extra build manifest input is the independently hashed `tools/m1_platforms.py` driver (`38a16b7475d87e5ee1af98cb9f89aeeaa3c791087c22062cbdb587ea8d8234af`). All five supplemental base Class declarations occur in the frozen `crates/pkl-runtime/src/stdlib/base.pkl` source.

The original Class fixture archive is `a39060fbf155947212aaae5b2e950dc2cd04381a78cb2860a7f8b1675b3657fc`; the metadata fixture archive is `8b111f12f87e171106ce431c28a3267d027a2d24c927249d387348fd13f5f652`. Archive members, fixture inputs, report rows and packet file inventories were checked. Original Java raw channels are independently pinned by these archives. The original packet has 21,730 inventoried files; metadata has 11,557.

The M1 platform build's 222 frozen source hashes and six artifact hashes agree with its report and the portable Class packet copies. Actual pinned Node `v26.10.0` executable SHA-256 is `d4cd917f05a5730ffda8a203e32fb8f9b70f79213fff1fafb045e438db8a6c06`; Chromium `156.0.8078.4` executable SHA-256 is `c8509aee32abd6977968e9999769980b7a420dea05402d90042d9e5b41db15ef`. Their current executable bytes match the recorded identities. Both portable process exits, stdout/stderr hashes, isolated Node child and Chromium context/Wasm records, frozen Wasm/N-API hashes, input transports, session traces, and zero terminal slots/depth were checked. This is local macOS evidence only.

The recorded five required Rust checks all exited zero on unchanged sources with rustc 1.98.1; workspace tests record 910 passed, zero failed, one ignored. Their captured stdout/stderr hashes were checked. This audit did not rerun them.

## Raw comparison findings

| Packet | Sessions per target | Native | N-API | Chromium | Raw channels per target | UTF-16 channels per target |
| --- | ---: | --- | --- | --- | ---: | ---: |
| Original Class | 88 | 78 pass, 4 mismatch, 6 oracle-error | 78 pass, 4 mismatch, 6 oracle-error | 78 pass, 4 mismatch, 6 oracle-error | 3,008 | 1,156 |
| Additive metadata | 20 | 20 pass | 20 pass | 20 pass | 1,240 | 488 |

The original Class packet has 110 unequal raw channels per target. The four ordinary mismatches are the two repetitions each of `runtime-class-canonicality` and `builtin-ancestry`. The six oracle errors are the two repetitions each of `annotation-log-throw-retry`, `annotation-log-close`, and `annotation-log-reentry-throw`. The metadata packet has zero unequal channels. Exact channel names, lengths and SHA-256 values are retained in the audit JSON, with no normalization. Original Class strict common comparison fails; additive metadata strict common comparison passes. Both `--record-only` captures and offline integrity records are distinct from these strict outcomes.

Both packets explicitly set `full_public_api_comparison_passed=false` and `full_public_api_status=blocked-unexecuted-required`. Candidate provider SPI and exported Class API parity are `not-executed-required` in both packets. The original Java authority retains 578 factory creates, 10 resolves, and separate exported Class observations; the metadata Java supplement retains 284 creates and has no acquired exported Class observation. These are separate required boundaries, not common query passes.

The six original upstream CLI cases each had one Java and one Rust guest execution, 12 processes total. Exact pinned upstream input/resource bytes, argv, cwd, environment, baseline workspace entries (7,299), public output bytes, exit codes, generated public file set, private state provenance, release artifact, and 4,642 inventory files were checked. Two `verify_case` calls per case were offline checks of the same capture. Results are five pass and one mismatch: `api/any.pkl` exits 0 on Java and 1 on Rust, first failing at `5.mb.getClass().simpleName` on line 10 because DataSize value construction is unsupported. The original strict gate failure and 5,477 retained full gate failure records remain intact.

The M1 platform experiment has 6,078 comparisons: 6,076 Java matches each for Rust, Node and Chromium, and 6,078 shared-engine matches. The two regex cases `regex-unicode-15-vs-16` and `regex-zero-width-supplementary` remain strict compatibility failures. Its architecture experiment contract passes. Neither result establishes full Pkl compatibility or the execution-time and memory targets.

## Remaining scope

Provider SPI and public Class export parity, metadata supplement export acquisition, primitive DataSize/Bytes/IntSeq/Regex values and constructors, further lifecycle and policy behavior, whole-language/API coverage, other release operating systems, and per-workload performance and memory targets remain open. The current final evidence is sound for its selected local scope; the full M2 and release gates are not met.
