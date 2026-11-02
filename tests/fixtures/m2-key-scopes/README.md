# Java 0.32.0 key scopes fixtures

Literal entry key names, anonymous null keys and lambda names in nested scopes.

These 9 authored cases have 18 captured observations with power assertions enabled and disabled. The 190 files below this directory, excluding this README, are exact copies of the recorded Java fixture tree. They contain authored inputs, the case roster, raw status, UTF-8 payloads and big-endian Java UTF-16 code units. Expected payloads were not produced by Rust.

The oracle uses `M2PowerAssertionOracle.java` and the pinned Java/Pkl identities recorded in `references.lock`, `oracle.lock.json` and the acquisition receipts. Capture: `target/m2/stage133-key-scope-oracle-01/report.json` (SHA-256 `89b635532c70c087a2b04b65a22f29888fde498e17871c2e716875da7b8db340`). Admission/readback: `target/m2/stage133-key-scope-readback-02/report.json` (SHA-256 `dbc4d02eb7672711070a5b126f6f451c7621f4730aa7e4b364fa1fb2b2dc4f35`). These development evidence paths are local artifacts; publication must retain the receipts and raw captures or supply their final evidence location.

`crates/pkl-runtime/tests/m2_key_scope_oracle.rs` compares complete status, output and diagnostic bytes and UTF-16 code units. Expression fields remain captured alongside the authored expression input. Captured Java exception class fields remain oracle metadata; this does not establish a corresponding Rust exception-class API. The cross-source test additionally checks ordinary Java error classification. No trimming, sorting or reserialization is applied to comparison payloads.



The combined isolated source13 run passed this cohort as part of 160 selected observations (152 prior observations plus eight new Reference controls). That is bounded differential evidence, not full M2, whole-language, platform or performance compatibility. Compiler dependency closure is not established by these fixtures.
