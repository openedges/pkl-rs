# Java 0.32.0 prefix order fixtures

Generator fallback prefix visitation, error ordering and function scope naming.

These 4 authored cases have 8 captured observations with power assertions enabled and disabled. The 85 files below this directory, excluding this README, are exact copies of the recorded Java fixture tree. They contain authored inputs, the case roster, raw status, UTF-8 payloads and big-endian Java UTF-16 code units. Expected payloads were not produced by Rust.

The oracle uses `M2PowerAssertionOracle.java` and the pinned Java/Pkl identities recorded in `references.lock`, `oracle.lock.json` and the acquisition receipts. Capture: `target/m2/stage133-prefix-order-oracle-01/report.json` (SHA-256 `9856a1a47821ab718be9c19201e4418b6effd0c8f50e670990916bc06c53d1a6`). Admission/readback: `target/m2/stage133-prefix-order-readback-01/report.json` (SHA-256 `39f3db340e97c2126e2c3ad1083ae54c43c1378ed4547a9afb8bd0926b33e31f`). These development evidence paths are local artifacts; publication must retain the receipts and raw captures or supply their final evidence location.

`crates/pkl-runtime/tests/m2_prefix_order_oracle.rs` compares complete status, output and diagnostic bytes and UTF-16 code units. Expression fields remain captured alongside the authored expression input. Captured Java exception class fields remain oracle metadata; this does not establish a corresponding Rust exception-class API. The cross-source test additionally checks ordinary Java error classification. No trimming, sorting or reserialization is applied to comparison payloads.



The combined isolated source13 run passed this cohort as part of 160 selected observations (152 prior observations plus eight new Reference controls). That is bounded differential evidence, not full M2, whole-language, platform or performance compatibility. Compiler dependency closure is not established by these fixtures.
