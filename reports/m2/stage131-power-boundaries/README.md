# Stage131 power-assertion visibility — local checkpoint

The shared Rust renderer now follows the pinned Java power-assertion visibility boundaries for the selected generator, predicate, lambda and object-method cases. A fresh production probe matches Java's complete raw CLI result for the original `errors/power/typeConstraints11.pkl` case; a fresh Stage130 baseline probe mismatches that same contract. This is a bounded repair. The full workspace command completes with exit **0**; independent raw accounting qualifies **1,790 passed, zero failed and 5 ignored across 288 harnesses**.

Java 0.32.0 independently captured 17 authored programs with power assertions on and off: 34 expression-string observations at explicit stable module URIs. The definitive Java05 readback records 32 ordinary errors and two successful results, 306 raw result files and 69 fixture inputs. The production Rust regression test compares status and complete UTF-8 and UTF-16 fields for all 34 observations. Its `String` adapter does not establish lossless unpaired-surrogate transport. Java exception class fields are retained as Java provenance, not represented as Rust class matches.

The repair distinguishes the closest Java parser boundary: generator iterable expressions may be visible while generator bodies are suppressed; nested boundaries can change that decision. A predicate's synthesized whole amendment can be visible while its condition and body descendants remain suppressed. Object methods inherit surrounding visibility; a function literal follows the lambda rule. This concerns diagnostic observations, not evaluation order or full language compatibility.

## Original-case evidence

The Stage130 baseline acquisition `stage131-original-baseline-01` is terminal and records a raw mismatch for the unchanged upstream original CLI case. `stage131-production-original-03` uses the separately built production `module_eval_probe` with the explicitly selected pinned Temurin JDK. Both Java and Rust return the expected diagnostic exit 1, with empty stdout, identical complete stderr bytes (SHA-256 `933f468672871a53ebcaa7404d8fa82d8b8c3b2fbae3ec32af2a6d79d2eb89b4`), no public generated files and no public workspace changes. The case status is `pass`, with `raw_channels_match=true` and neither process timed out. The original case remains unlinked to requirements; this result does not close the strict full-M2 gate.

The production build and isolated source05 build have matching executable hashes, but their actual build commands and Cargo cache provenance are separate. The production artifact is pinned by `stage131-production-module-artifact-01`; a finite comparison in `stage131-production-equivalence-01` independently joins 241 source files, the regression test and 375 Java04/Java05/production fixture files. Source or binary hash equality does not itself establish execution, full compiler inputs or runtime closure; original03 is the separate production execution.

## Local verification and provenance

The production targeted command completes with **14 passed, zero failed**, across four test targets. One of those tests checks all 34 Java fixture observations; those observations are not 34 additional Rust tests. A separately filtered library invocation records **five passed, 207 filtered out**; its results are not added to workspace totals. Formatting, `cargo xtask verify`, all-target warnings-denied Clippy, the wasm32 runtime check and `tools/m2_manifest.py --check` each complete with actual exit zero. These are local results on macOS arm64 with Rust 1.98.1; the associated command-specific reports preserve exact arguments, source bookends, limits, runtime and output pins. The main cache uses a recorded clone and these results are not hosted CI evidence.

The independently read back bounded M1 acquisition records **6,078 raw matches for each of Rust, Node and Chromium**, **92 scenarios each for Node and Chromium**, and both strict compatibility and experiment-contract statuses `pass` on that same acquisition. The independently reported `known_gaps` list is empty, and no producer-snapshot source is missing a supervisor bookend in this run. The cache clone and runtime profile are recorded separately; this result is not full Pkl or public API compatibility, release-platform validation, or a performance claim.

The complete workspace acquisition has stable source and HEAD bookends, no guard stop and no live owned children at termination. All 286 executable test targets and two doctest crates complete; 26 examples are compile-only. The independent aggregate reader excludes 13 nested child passes. Its totals depend on pinned child-source and marker assumptions; exact per-test identity and absence of count-preserving interleaving are not proven. The targeted and filtered invocations are not added to these totals.

Post-test artifact readback records 312 executable products, 6 Cargo-cache events and 311 products with matching current wrapper/signature receipts. One product lacks a current wrapper receipt. This is stable per-file observation, not an atomic snapshot or full compiler/runtime closure. No signature commands were rerun. The workspace took 4222.0 seconds under build-jobs=1, test-threads=2, test optimization=1 and debug information disabled; concurrent isolated work makes this unsuitable as performance evidence.

## Retained failures and limits

- Java oracle01 produced 24 observations but only 20 reached intended outcomes: four parser failures came from reserved identifiers. The corrected 24-observation oracle02 and later acquisitions do not erase the original results.
- Isolated candidate v1's test exited 101 on `predicate-simple/on`: it hid the whole synthesized predicate amendment that Java displayed. The v2 predicate correction and v3 object-method control were evaluated separately before production integration.
- Java04 has an acquisition-time fixture-copy bookend gap. Its current bytes can be compared, but current equality cannot prove historical copy identity. Java05 adds the copied-input bookends and is the authoritative fixture capture; its independent readback joins 306 result files to Java04 without retroactively repairing that gap.
- Production original01 stopped before evaluation because the driver derived `/usr/release` from the `/usr/bin/java` launcher. Original02 completed using that launcher; original03 is the separate explicitly pinned JDK run. The first failure remains recorded.
- The Java acquisition inherits a process environment without a complete environment snapshot; the evaluator explicitly receives empty guest environment and properties. Recorded JDK files do not establish its full transitive runtime closure. API expression-string captures and the original CLI case are different contracts.
- Full language/API compatibility, the strict full completion gate, Linux and Windows release execution, hosted CI, and speed/memory targets remain open. Historical failures, unsupported cases, ignored tests and unmapped requirements are not passes. Stage132 work is outside this checkpoint.

## Evidence packaging

The accompanying checkpoint descriptor identifies the finite evidence archive and
its independent integrity readback. The archive retains original failed attempts,
Java input/result captures, isolated candidate snapshots, production source pins,
raw local command outputs, supervisor helpers and selected predecessor metadata.
Cargo caches, installed runtimes, unrelated original-workspace resources and old
archive bodies are excluded. Archive integrity does not establish execution or
complete transitive dependency closure. Java source and the normalized upstream
fixture are provenance; the fresh Java CLI acquisition is the raw result authority.
