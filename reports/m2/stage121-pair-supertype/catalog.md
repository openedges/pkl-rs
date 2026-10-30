# Stage121 publication catalog

This finite catalog records fresh checkpoint evidence and source snapshots. It is not an archive and does not claim full compatibility. Root owns archive creation, verification and publication.

`paths.jsonl` records 222,571 logical paths, byte identities and per-path modes. `parts.json` assigns 13,514 unique byte payloads to ten parts using an 80 MiB uncompressed tar estimate per part. Objects are deduplicated by byte identity; their archive-header mode does not represent every alias. Restoration must apply each alias mode from `paths.jsonl`.

Include this catalog's metadata and the seven exact late full-ready readback files in `supplemental-metadata.json` in part 01. Verify actual compressed parts are at most 85 MiB, all unique objects match, and every alias resolves. Final checkpoint prose and Git-tracked reports can remain outside the archives. `compressed-estimate.json` is an optional read-only planning estimate; the retained ten-part map remains the root's chosen plan.

The source closure includes the final six production changes, three tests, 21,895 fixture files, actual private compiler source snapshot, portable source snapshot, finite controller inputs, emitted dependency records and exact selected build products. Failed private attempts, the failed initial portable build qualification, failed Chromium acquisition and their successful continuations are preserved. No Cargo target cache was recursively selected.

The fresh Java, native, N-API and Chromium channels are included without normalization. The portable independent reader verifies 624 new cases and 19,716 fields per target, plus 902 retained ordinary cases and two required internal outcomes per target. Its full API flag remains false and 2,020 JVM-only fields remain required.

Terminal full-ready results are 4,339 passes, 19 mismatches, 82 unsupported cases and two oracle errors; 88 metamorphic relations pass. There are no previous-pass regressions. The two repaired runtime cases are distinct from Stage117's 12 accounting-link changes; all 103 unready requirements remain required.

Prior published Stage114 and Stage120 checkpoints are explicit restoration prerequisites, not copied old archive payloads. Their checkpoint/binding/readback metadata is included. The catalog does not promise a self-contained toolchain or complete historical acquisition closure: installed Rust/JDK/Node/browser bodies and historical authorities must satisfy their retained fingerprints. No old archive body was read. Temporary home/tmp/cache directories and nonregular paths are excluded before traversal; `excluded-paths.json` records those choices.

The catalog is a publication path selection, not a replacement for individual acquisition admission checks. The final fresh archive verification must validate these exact bytes once and preserve all limitations above.
