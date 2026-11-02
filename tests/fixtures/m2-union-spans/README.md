# Java 0.32.0 union spans fixtures

Union alternative grouping, aliases, nullable nodes, constraints and direct/function-parameter diagnostics.

These 26 authored cases have 52 captured observations with power assertions enabled and disabled. The 573 files below this directory, excluding this README, are exact copies of the recorded Java fixture tree. They contain authored inputs, the case roster, raw status, UTF-8 payloads and big-endian Java UTF-16 code units. Expected payloads were not produced by Rust.

The oracle uses `M2PowerAssertionOracle.java` and the pinned Java/Pkl identities recorded in `references.lock`, `oracle.lock.json` and the acquisition receipts. Capture: `target/m2/stage132-union-oracle-04/acquisition/report.json` (SHA-256 `054243e52c387d56df0668119456075383a6cf7444d60598b8445c5efa0510b7`). Admission/readback: `target/m2/stage132-union-admission-03/report.json` (SHA-256 `a3432c8dbdd1ab78107ab0e9463816c2611b106679bb4b0baf6c9add7cee9165`). These development evidence paths are local artifacts; publication must retain the receipts and raw captures or supply their final evidence location.

`crates/pkl-runtime/tests/m2_union_span_oracle.rs` compares complete status, output and diagnostic bytes and UTF-16 code units. Expression fields remain captured alongside the authored expression input. Captured Java exception class fields remain oracle metadata; this does not establish a corresponding Rust exception-class API. The cross-source test additionally checks ordinary Java error classification. No trimming, sorting or reserialization is applied to comparison payloads.

The two lambda-result success observations preserve Java behavior; they are not expected function-return type failures.

The combined isolated source13 run passed this cohort as part of 160 selected observations (152 prior observations plus eight new Reference controls). That is bounded differential evidence, not full M2, whole-language, platform or performance compatibility. Compiler dependency closure is not established by these fixtures.
