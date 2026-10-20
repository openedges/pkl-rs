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

The local M0 checks passed on the recorded macOS host. At publication, SSH pushes were
available but this environment's GitHub CLI was not authenticated for the private
repository, so remote Actions results could not be inspected. Workflow configuration
and local results must not be reported as a successful remote run. See the
[M0 evidence report](../reports/m0/README.md) for this validation boundary.
