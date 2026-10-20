# CI and local reproduction

[The workflow](../.github/workflows/ci.yml) has three independent scopes. The workspace
matrix checks reference pins, formatting, lint, workspace tests, and Wasm compilation
on Linux, macOS, and Windows. The Linux compatibility job checks inventories and all
tooling regressions and the benchmark contract, installs the exact reference JDK, fetches the checksummed Java
oracle, records seven smoke cases, compares an independent Java replay, and records
three selected historical regression cases. Java replay verifies the comparison tool;
that M0 job does not execute a Rust Pkl evaluator.

The separate M1 experiment job executes bounded Java/Rust frontend and runtime
comparisons, actual Java Host lifecycle observations, exact Java/Rust retry and
nested-evaluation observation protocols, and the shared primitive
corpus through native Rust, a Node addon, and real Chromium Wasm. It pins Node,
Playwright and wasm-bindgen, builds artifacts, and uploads captures, artifacts,
source snapshots and failures. Its architecture contract requires complete
execution and cross-target consistency while preserving the exact known regex
backend mismatches as failed compatibility cases. Three complete runtime
diagnostic byte mismatches are also recorded explicitly. A green M1 job means
the experiments and known-gap accounting reproduced, not full conformance.
Lifecycle diagnostics are retained and compared separately from the observation
protocol; protocol equality does not establish Java error rendering. Node and
browser scenarios also require live Host operations to abort and release their
resources on cancellation, close, and timeout.
Host-sensitive VM performance measurements remain local, separately recorded
experiments on a quiet host.

The M1 job also checks the retained Graal compilation archive and rederives its
diagnostic verdict from raw captures, source/artifact identities, correctness
records, and exact guest target/tier events. This is portable verification of a
macOS arm64 acquisition, not a fresh Linux Graal execution or a benchmark.
Missing target compilation, changed bytes, and incomplete cases fail the check.
The separate diagnostic class override remains outside the oracle baseline.
See the [compilation investigation](m1-graal.md) for acquisition commands.

The Java job pins Python 3.12, Temurin `21.0.12.1+1`, and action revisions. Oracle
verification also checks the actual JDK vendor/build and Pkl artifact identity. A
missing artifact, incompatible JDK, failed inventory check, missing case, corrupted
capture, or byte mismatch fails the job. Reference checkouts are checked again after
execution. Raw oracle, recording, replay, and regression evidence is uploaded even
after failure, including hidden captured output files, with 14-day artifact retention. Missing all evidence also fails the
upload step.

Manual dispatch with `full_corpus=true` additionally attempts every discovered command
case. It currently returns failure for 99 blocked requirements and the list workload's
512 MiB heap failure. Those outcomes are preserved in the uploaded report. Ordinary
push and pull-request runs use the explicitly limited smoke/regression scope; a green
smoke run cannot establish full-corpus completion. The separate Pantry investigation
and host-sensitive performance runs are documented local commands, not CI jobs.

## Reproduction

Initialize the pinned submodules as described in [README.md](../README.md). Run the
workspace and Python checks in [TESTING.md](../TESTING.md). Install the exact reference
JDK before following [the oracle bootstrap](oracle.md) and the record/compare/check
commands in [the comparison contract](compatibility-tooling.md). New runs require
fresh evidence directories. Candidate replay must use the same absolute workspace
path as its Java baseline so error locations can be compared without rewriting bytes.

Recorded evidence is reviewable on another machine, but its absolute diagnostic paths
and native JDK binary hashes belong to the original host. Generate a fresh Java baseline
in the candidate's controlled environment for a formal comparison. Do not normalize
historical output to make a different host appear identical.

The local M0 checks passed on the recorded macOS host. After authenticated access became
available, [run 39364517355](https://github.com/luuvish/pkl-rs/actions/runs/39364517355)
was verified for the final M0 code commit `2164ffa`. All four jobs succeeded: the three
workspace platforms and Linux Java tooling. All required steps ran successfully; only
the manual full-corpus step was intentionally skipped on this push event.

The downloaded artifact ZIP matches GitHub's SHA-256 digest. Its three reports and raw
captures passed completeness/integrity checks, and all seven Java replay cases were
compared again directly from the downloaded bytes. Exact JDK properties match the
reference lock. The [CI summary](../reports/m0/ci-summary.json) and
[retained evidence](../reports/m0/ci-evidence.tar.gz) preserve run/step metadata, logs,
the original artifact ZIP, and extracted raw captures beyond GitHub's retention period.
This verifies workspace and Java tooling behavior; the full corpus, Pantry investigation,
performance acquisition, and Rust evaluation are outside this CI run's scope.

The M1 code commit `e9ee8d5` passed all five jobs in
[run 39372369602](https://github.com/luuvish/pkl-rs/actions/runs/39372369602),
including the real Node/Chromium experiment. Its downloaded artifact ZIP matches
GitHub's SHA-256 digest. Recomputed raw results confirm 15 frontend matches,
five Java Host suites, six runtime value matches and three diagnostic byte
failures, and 6,078 shared-engine primitive results with the same two Java regex
backend failures on every target. All 12 required platform scenarios ran in
each JS environment. Captured source snapshots match the exact checked commit.
The [M1 CI summary](../reports/m1/ci-summary.json) and
[retained raw archive](../reports/m1/ci-evidence.tar.gz) preserve the artifact,
logs, verification procedure and result beyond GitHub artifact retention.

The closing code commit `6a5e4b5` passed all five jobs in
[run 39374668362](https://github.com/luuvish/pkl-rs/actions/runs/39374668362).
The added lifecycle step compares two complete Java/Rust observation protocols,
preserves four diagnostic payload differences, and records Java's nested new-read
cache mutation failure as a separate negative observation. Fourteen scenarios
ran in both Node and Chromium, including live nested-operation cleanup. The
downloaded ZIP digest, exact committed sources, raw results and Java identity were
reverified. The [closing summary](../reports/m1/ci-closing-summary.json) and
[archive](../reports/m1/ci-closing-evidence.tar.gz) retain the original artifact,
logs and verification procedure. Unpacked Cargo intermediates are excluded from
the archive; the original artifact ZIP and all evidence inputs remain retained.
