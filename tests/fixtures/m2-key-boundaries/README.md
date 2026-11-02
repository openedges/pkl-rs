# Java 0.32.0 key boundaries fixtures

AST constant lowering boundaries for grouped numeric values, negation and unit members.

These 6 authored cases have 12 captured observations with power assertions enabled and disabled. The 127 files below this directory, excluding this README, are exact copies of the recorded Java fixture tree. They contain authored inputs, the case roster, raw status, UTF-8 payloads and big-endian Java UTF-16 code units. Expected payloads were not produced by Rust.

The oracle uses `M2PowerAssertionOracle.java` and the pinned Java/Pkl identities recorded in `references.lock`, `oracle.lock.json` and the acquisition receipts. Capture: `target/m2/stage133-key-boundary-oracle-01/report.json` (SHA-256 `96725e0824dcdeedec2cdf344cc18af537abbeef9b75adcbd6429bcbf0c0e7c6`). Admission/readback: `target/m2/stage133-key-boundary-readback-01/report.json` (SHA-256 `029dc2eca7038614850c47ae3522e2a9649d11540671ed1314b76d3f37d6109b`). These development evidence paths are local artifacts; publication must retain the receipts and raw captures or supply their final evidence location.

`crates/pkl-runtime/tests/m2_key_boundary_oracle.rs` compares complete status, output and diagnostic bytes and UTF-16 code units. Expression fields remain captured alongside the authored expression input. Captured Java exception class fields remain oracle metadata; this does not establish a corresponding Rust exception-class API. The cross-source test additionally checks ordinary Java error classification. No trimming, sorting or reserialization is applied to comparison payloads.



The combined isolated source13 run passed this cohort as part of 160 selected observations (152 prior observations plus eight new Reference controls). That is bounded differential evidence, not full M2, whole-language, platform or performance compatibility. Compiler dependency closure is not established by these fixtures.
