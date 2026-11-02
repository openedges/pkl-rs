# Java 0.32.0 generic labels fixtures

Same-source generic argument labels, nullable arguments and forwarding.

These 12 authored cases have 24 captured observations with power assertions enabled and disabled. The 265 files below this directory, excluding this README, are exact copies of the recorded Java fixture tree. They contain authored inputs, the case roster, raw status, UTF-8 payloads and big-endian Java UTF-16 code units. Expected payloads were not produced by Rust.

The oracle uses `M2PowerAssertionOracle.java` and the pinned Java/Pkl identities recorded in `references.lock`, `oracle.lock.json` and the acquisition receipts. Capture: `target/m2/stage132-generic-oracle-02/acquisition/report.json` (SHA-256 `968209ab47484ce5407a9aeba3a8829d43a39c73c7cf04da36f76f2c4e265140`). Admission/readback: `target/m2/stage132-generic-admission-02/report.json` (SHA-256 `598fbeedf3d157ca3e4fb46c9690923d273305430f795492d053035eac99e05b`). These development evidence paths are local artifacts; publication must retain the receipts and raw captures or supply their final evidence location.

`crates/pkl-runtime/tests/m2_generic_label_oracle.rs` compares complete status, output and diagnostic bytes and UTF-16 code units. Expression fields remain captured alongside the authored expression input. Captured Java exception class fields remain oracle metadata; this does not establish a corresponding Rust exception-class API. The cross-source test additionally checks ordinary Java error classification. No trimming, sorting or reserialization is applied to comparison payloads.



The combined isolated source13 run passed this cohort as part of 160 selected observations (152 prior observations plus eight new Reference controls). That is bounded differential evidence, not full M2, whole-language, platform or performance compatibility. Compiler dependency closure is not established by these fixtures.
