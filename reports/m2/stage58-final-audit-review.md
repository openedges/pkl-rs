# Stage58 independent pre-commit review

- Pkl 0.32.0 Java authority: canonical 86 rows in two JVM sessions; additive 13 rows in two JVM sessions. The supplement retains its documented Python helper provenance limit.
- Canonical native, N-API, and Chromium: 168/172 raw matches each; Regex and IntSeq rows mismatch in both repeats. Integrity passes; strict string comparison fails. DataSize string regression: 510/510 on each target.
- Original unchanged modules: 17 Java executions and 17 Rust executions, one of each per case; 10 raw matches and seven mismatches. `api/any.pkl` now matches. First-error labels for three `expected an object` cases were corrected to the stored Rust diagnostics.
- Five Rust commands exit 0; workspace test logs total 929 passed, zero failed, three ignored. Python tool validation02 records 19 tests, final native checks nine; 20 distinct tests. JS validation records 17. Historical tool validation01 invocation failure remains evidence, not a pass.
- M1 architecture experiment passes while strict compatibility remains failed at 6,076/6,078 per target. Four local evidence publications preserve raw failures, source identities, and full member readback.
- Typed exports, full API/whole-language gates, other release platforms, and performance targets remain open. The checkpoint explicitly awaits implementation Git commit binding. No factual blocker remains for the bounded implementation commit after the original first-error correction.
