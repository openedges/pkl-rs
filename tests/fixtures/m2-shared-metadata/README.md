# Java 0.32.0 shared metadata fixtures

Generic Reference metadata forwarding, nullable and nested actuals, constraint erasure, and type-check preservation under conservative shared-parameter flags.

These 4 authored cases have 8 captured observations with power assertions enabled and disabled. The 89 files below this directory, excluding this README, are exact copies of the recorded Java fixture tree. They contain authored inputs, the case roster, raw status, UTF-8 payloads and big-endian Java UTF-16 code units. Expected payloads were not produced by Rust.

The oracle uses `M2PowerAssertionOracle.java` and the pinned Java/Pkl identities recorded in `references.lock`, `oracle.lock.json` and the acquisition receipts. Capture: `target/m2/stage132-shared-oracle-02/report.json` (SHA-256 `f2271bc4c28c3cca8b4e1ce0da22acde7dafb74747f03ff0420a18a8b889ec11`). Admission/readback: `target/m2/stage132-shared-oracle-independent-readback-01/report.json` (SHA-256 `76acf976abf68605b19942eb1afb22f42f5a5b5f37a93286ff4e15311a4ebc50`). These development evidence paths are local artifacts; publication must retain the receipts and raw captures or supply their final evidence location.

`crates/pkl-runtime/tests/m2_shared_metadata_oracle.rs` compares complete status, output and diagnostic bytes and UTF-16 code units. Expression fields remain captured alongside the authored expression input. Captured Java exception class fields remain oracle metadata; this does not establish a corresponding Rust exception-class API. The cross-source test additionally checks ordinary Java error classification. No trimming, sorting or reserialization is applied to comparison payloads.



The combined isolated source13 run passed this cohort as part of 160 selected observations (152 prior observations plus eight new Reference controls). That is bounded differential evidence, not full M2, whole-language, platform or performance compatibility. Compiler dependency closure is not established by these fixtures.
