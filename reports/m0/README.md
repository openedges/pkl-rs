# M0 baseline evidence

These artifacts establish reference inputs, execution contracts, Java observations,
and verification-tool behavior for Pkl 0.32.0. The Rust evaluator is unimplemented.
Java recordings and Java-to-Java replay are not Rust compatibility results.

## Recorded scope

| Area | Observed result | Evidence |
| --- | --- | --- |
| Oracle | Exact artifact/runtime checks, independent success/error replay, and rejection of checksum tampering | [Raw oracle evidence](oracle-evidence.tar.gz), [contract](../../docs/oracle.md) |
| Source inventory | 985 files, 11,624 items; 542 external declarations and 542 builtin registration candidates matched | [Inventory](../../compatibility/inventory.json), [limitations](../../docs/inventory.md) |
| Discovery | 1,546 Pkl files, 73 projects, 1,112 command cases; 199 JVM test/support source entries separately inventoried | [Corpus](../../compatibility/corpus.json), [selection contract](../../docs/corpus.md) |
| Tooling smoke | Seven Java recordings and seven exact Java replay matches | [Recording](tooling-smoke-evidence.tar.gz), [replay](tooling-replay-evidence.tar.gz) |
| Selected rev1 regressions | Three fresh Java outcomes with exits 1, 0, 1; three exact replay matches | [Recording](regression-evidence.tar.gz), [replay](regression-replay-evidence.tar.gz), [provenance](../../docs/regressions.md) |
| Full command acquisition | 1,012 recorded, 99 blocked, one unexpected Java exit; all 1,013 ready commands invoked | [Summary](full-corpus-summary.json), [raw evidence](full-corpus-evidence.tar.gz) |
| Larger heap diagnostic | The failed list case succeeds twice with identical raw results at 2 GiB | [Separate experiment](../../compatibility/experiments/list-large-heap.json), [raw evidence](list-large-heap-evidence.tar.gz) |
| Pantry investigation | 29 package/API commands, 206 generated package modules, four multiple-file retries, two configured generators, and the tar pipeline executed | [Summary](pantry-summary.json), [raw evidence and dependency cache](pantry-evidence.tar.gz) |

The strict full-corpus recording/check commands both return exit 1. The 99 blocked
cases require package/project/TLS, HTTP, classpath, or formatter adapters. The remaining
failure is `api/list.pkl`, which constructs a list with 118,866,785 elements and exceeds
the locked 512 MiB heap. The separate 2 GiB experiment diagnoses that limit; it does not
replace the original failure or silently change the reference profile.

`recorded` means an allowed Java exit and raw capture, not a successful upstream
assertion or semantic review. Cases marked `observe`, including support/abstract
modules and some diagnostic inputs, retain that distinction. The 199 JVM source entries
have no command adapter here; their dynamic test/assertion count remains unknown.
The selected historical and tar-shell supplements are separate from the 1,112 commands.

All 29 Pantry package/API commands completed successfully. Generated modules yielded
142 successful scalar evaluations and 64 errors requiring configuration or other review.
Of four multiple-file retries, one succeeded and three reported missing configuration.
Both configured generators and the tar pipeline succeeded. Seven remote dependencies
were checksum-verified, and all 72 actual cache files are retained. The 585 test and
1,875 assertion invocations reported by package commands include repeated dependencies;
they are not distinct coverage counts. Live GitHub discovery paths remain unfixed.

## Performance acquisition

The six-workload [measurement contract](../../docs/benchmarks.md) covers cold Java CLI,
persistent evaluator, and fresh evaluator requests with exact-output checks and
per-process peak RSS. All 30 cold CLI samples and 24 independent API forks completed;
all 24 forks met the specified warmup convergence criterion. The [summary](benchmarks.json)
and [raw evidence](benchmark-evidence.tar.gz) retain per-workload latency/RSS distributions,
first requests, warmup, measurement windows, host context, and runtime provenance.

The separate recursive compilation trace observed guest compilation bailouts despite
the verified optimizing runtime. Stable warmup windows do not prove successful guest
compilation. This is evidence of the pinned runtime's actual behavior, not a claim of
fully optimized recursive performance. The required inventory still contains
unimplemented workloads, and no Rust comparison or performance verdict exists.

An initial attempt with a harness-added per-request timeout was rejected after queued
timeout tasks exhausted the fixed heap. Its [raw evidence](benchmark-timeout-attempt-evidence.tar.gz)
is retained separately. The completed CPU baseline disables that request timeout while
preserving the outer process deadline and the 64/512 MiB heap settings. Timeout and
cancellation remain separate pending workloads; see the measurement contract for the
upstream scheduler references and exact boundary.

## Validation and CI boundary

Eleven local validation commands passed: reference pins, Rust formatting/lint/tests,
Wasm compilation, all 79 Python tool regressions, four inventory/fixture freshness
checks, and the benchmark contract check. [Raw validation logs](validation-evidence.tar.gz)
record commands, exits, host context, and source hashes. Tests deliberately exercise missing runners,
empty/partial corpora, corrupted bytes, wrong source/runtime identities, timeouts,
incorrect memory units, insufficient warmup, and false convergence.

The [CI workflow](../../.github/workflows/ci.yml) integrates the workspace matrix and
Java tooling replay. [Run 39364517355](https://github.com/luuvish/pkl-rs/actions/runs/39364517355)
for code commit `2164ffa` completed successfully. All four jobs and every required step
passed: Linux/macOS/Windows workspace checks, 79 Python tool regressions, seven Java
smoke recordings, seven exact Java replay matches, and three selected regression
recordings with strict checks. The manual full-corpus step was intentionally skipped.

The downloaded artifact's ZIP digest matches GitHub's SHA-256. Stored report completeness,
raw capture hashes, and all seven Java replay byte comparisons were independently
rechecked after download. Exact Linux JDK properties match the reference lock.
[The summary](ci-summary.json) and [raw CI evidence](ci-evidence.tar.gz) retain metadata,
logs, the original artifact ZIP, and extracted captures. See [the CI contract](../../docs/ci.md)
for scope. Full-corpus failures and performance limitations elsewhere in this report remain.

## Inspecting artifacts

Each archive contains `EVIDENCE.json`, an inventory of relative paths, sizes, and SHA-256
digests. Validate an archive before extraction:

```sh
python3 tools/evidence.py verify reports/m0/full-corpus-evidence.tar.gz
```

Raw stdout/stderr, exit statuses, generated files, source manifests, input hashes, and
runtime provenance remain available without byte normalization. Explicit exclusions
remove duplicated source workspaces, not recorded outputs. Absolute paths and runtime
binary hashes describe the original host. Re-run in a fresh controlled environment for
future Java/Rust comparisons; old host-specific errors must not be rewritten.
