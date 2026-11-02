# Java 0.32.0 entry scopes fixtures

Object-scope entry numbering, nested keys, siblings and ordinary-prefix visitation before generator fallback.

These 8 authored cases have 16 captured observations with power assertions enabled and disabled. The 177 files below this directory, excluding this README, are exact copies of the recorded Java fixture tree. They contain authored inputs, the case roster, raw status, UTF-8 payloads and big-endian Java UTF-16 code units. Expected payloads were not produced by Rust.

The oracle uses `M2PowerAssertionOracle.java` and the pinned Java/Pkl identities recorded in `references.lock`, `oracle.lock.json` and the acquisition receipts. Capture: `target/m2/stage133-oracle-01/acquisition/report.json` (SHA-256 `f8239f277ba236a15cc758db849752e6800873d0f0ab3a2f1ab23a2dac8751bc`). Admission/readback: `target/m2/stage133-oracle-readback-01/report.json` (SHA-256 `2e773463d3655a1d83c4e926a911f6f96325e474c866c381eeb7151ca0575bb3`). These development evidence paths are local artifacts; publication must retain the receipts and raw captures or supply their final evidence location.

`crates/pkl-runtime/tests/m2_entry_scope_oracle.rs` compares complete status, output and diagnostic bytes and UTF-16 code units. Expression fields remain captured alongside the authored expression input. Captured Java exception class fields remain oracle metadata; this does not establish a corresponding Rust exception-class API. The cross-source test additionally checks ordinary Java error classification. No trimming, sorting or reserialization is applied to comparison payloads.

The initial generator-numbering hypothesis differed from Java; the captured prefix-revisit numbering is retained without rewriting oracle output.

The combined isolated source13 run passed this cohort as part of 160 selected observations (152 prior observations plus eight new Reference controls). That is bounded differential evidence, not full M2, whole-language, platform or performance compatibility. Compiler dependency closure is not established by these fixtures.
