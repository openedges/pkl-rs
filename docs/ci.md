# CI and local reproduction

[The workflow](../.github/workflows/ci.yml) has two independent scopes. The workspace
matrix checks reference pins, formatting, lint, workspace tests, and Wasm compilation
on Linux, macOS, and Windows. The Linux compatibility job checks inventories and all
tooling regressions and the benchmark contract, installs the exact reference JDK, fetches the checksummed Java
oracle, records seven smoke cases, compares an independent Java replay, and records
three selected historical regression cases. Java replay verifies the comparison tool;
the workflow does not execute a Rust Pkl evaluator.

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
