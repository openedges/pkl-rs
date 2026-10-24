# Stage57 A2 supplement authority review

Reviewed frozen A2 packet `target/m2/stage57-data-size-supplement-checks-01/freeze.json` (SHA-256 `e28e133f2f329f9dcf0b601b455f73e8ef0673683e56830dc4937e6a6c9fd003`) and the fixed raw fixture archive (SHA-256 `ab8c59f0a42611aa8325d70ab2c1cc83ec59f19d12a848ee816829acbed6b6ad`, 7,612 regular members). Independent read-only audit code and per-cohort observations are in this review directory. No guest, build, or candidate comparison was run for this review.

## Result

No publication blocker found. The raw archive has exact query TSV order and expression bytes for all six captures; module bytes and URI are bound per session. Java process stdout/stderr byte lengths and SHA-256 hashes match provenance. Four separate JVM sessions occur in each split capture (two string, two export), and two in the public identity capture. PIDs are distinct within each capture. Pinned JAR SHA-256 `c3b6f6215a08b31453b0fb0edd8c9da0aee31233f65abb8ab07f9c945151e059`, Java executable, runtime `release` and `lib/modules` hashes, seven source/lock snapshots, and captured input before/after hashes agree with actual bytes. Five pinned upstream source files also match the local reference checkout. Each raw row has full UTF-8 and UTF-16 expression channels. Raw row channels agree byte for byte across repetitions. Error channels were retained and nonempty for observed errors.

| Capture | Boundary | Ordered rows | Actual calls per session | Success/error per session | State |
| --- | --- | ---: | ---: | ---: | --- |
| 04 | string/export | 44 | 44/49 | 24/20 each | Historical incomplete wrapper, `complete=false` |
| 05 | string/export | 44 | 44/49 | 24/20 each | Corrected fresh split capture |
| 06 | public Java identity | 8 | 16 | 6/2 row statuses | Separate producer observation |
| 07 | string/export | 7 | 7/7 | 3/4 each | Separate module shadow cohort |
| 08 | string/export | 7 | 7/7 | 4/3 each | Separate qualified shadow cohort |
| 09 | string/export | 6 | 6/6 | 5/1 each | Separate lexical shadow cohort |

The five extra export calls in 04/05 are traced repeats, not five additional cases. Capture06 calls each row twice inside each of its two JVMs. The authored hypotheses in cases.json are exploratory; observed statuses and raw messages, including ordinary syntax/type/member failures, are the authority. Neither an error nor producer metadata is counted as candidate compatibility. The common candidate boundary is ordered string queries with status, complete UTF-8 stdout/stderr, and process protocol; exported Java type, DataSize bits/hash/unit and public Bytes identity are separate producer/public API evidence. No parity claim is made from same-status string/export rows.

The offline verifier uses explicit `ValueError` checks, so Python `-O` does not remove its validation. The frozen 18-test suite passed and includes corrupted raw status under `python -O`, plus source, command, file-set, UTF-16, process, status, history, and false compatibility-claim tamper cases. The frozen offline verifier exited zero. These are fixture integrity checks, not evaluator compatibility checks. Source/log sizes and digests still match the freeze. All 11 canonical A files still match their separate frozen manifest; its raw archive SHA-256 remains `51e14a2c80836141c61a9bbb18aaa708e6a963c238132854e55e7bf6493c6b8e`.

The A2 freeze records source and fixture hashes after final validation, without an independently timestamped pre-test Python source snapshot. Captured Java packets independently record source, input, and JAR before/after identity. A2 actual Git object binding remains pending. Full public export candidate comparison, native/Node/browser comparison, whole API/M2 completion, and performance remain unverified here.
