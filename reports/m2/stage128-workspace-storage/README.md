# Stage128 workspace storage and validation

This checkpoint addresses the disk-space obstruction to the full workspace test command after Stage127. It adds an opt-in macOS arm64/APFS compiler wrapper; the shared Pkl engine and Java 0.32.0 compatibility baseline are unchanged. The full workspace retry was interrupted at its disk floor; the independently qualified partial result is 1,265 passes, zero failures and three ignored across 200 completed harnesses. No full-workspace pass is claimed.

## Storage implementation

`tools/workspace_storage.py` uses rustc's link-artifact notifications to identify newly linked Mach-O executables in an explicitly isolated target directory. It preserves compiler streams, checks byte identity, mode, owner/group, ACL, extended attributes and code signatures, and atomically replaces an executable with a transparently compressed copy before returning to Cargo. It retains failure status and available raw streams, including stream-pump failures. Explicit test/binary links without notifications fail closed.

The default disk floor is 1,024 MiB. The `PKL_STORAGE_MIN_FREE_MIB` override accepts 512–65,536 MiB and is recorded per invocation. The wrapper does not impose a total workspace RSS, time or log bound; the acquisition supervisor supplies those limits. Inode identity and timestamps are not preservation guarantees. See the [operator guide](../../../docs/workspace-storage.md).

One standalone experiment retained all 23,805,872 executable bytes while reported allocation changed from 46,496 to 12,992 blocks of 512 bytes. Both strict code-signature checks passed. A separate compiled harness successfully spawned itself from the compressed executable. These are storage and tool checks, not runtime performance or language-compatibility measurements.

## Preserved attempts and execution conditions

- The initial experiment could not inspect attributes through this host Python's `os.listxattr`; it failed before invoking compression. The following experiment used macOS `xattr`.
- Workspace01 used one compilation job, default test threading and a 1 GiB disk floor. It produced 269 test-executable artifact records but started no test targets. The compiler succeeded for `lifecycle_probe`; the wrapper then refused a copy lacking headroom above its floor. Cargo exited 101. This outcome remains failed.
- Rebuildable compiler intermediates were removed from the idle default debug cache and two obsolete canonical caches. Evidence-pinned portable-build files, original cache05, source, executables, dependency/fingerprint metadata and raw evidence were retained. About 186 MB of actual free space was recovered from the default cache; removing approximately 304 MB of logical files in the two cloned caches recovered no measurable space. Logical file sizes are not reclaimed APFS capacity.
- Workspace02 was a separate acquisition using the previously built cache, a repaired wrapper, one compilation job, two test threads and a 768 MiB disk floor. Both attempts retain a 3 GiB sampled owned-process RSS limit, a four-hour wall limit and a 512 MiB combined log/receipt cap. No test selection or test output is changed. The earlier 1 GiB failure is not relabeled successful.

Wrapper versions and the exact prior product identities are recorded separately. The original wrapper source was reconstructed after workspace01 and its hash was checked against the pre-build source receipt; it is not described as a pre-build source snapshot. The final wrapper's focused mocked suite records 18 passes, and its actual compiler/self-spawn experiment exits zero.

## Verification scope

Workspace02 compiled successfully, then its supervisor stopped the actual Cargo process after free space crossed 768 MiB. Cargo exited -15 after 1,392.78 seconds; the tracked process set was empty at termination. The exact triggering free-byte sample was not retained, so the evidence establishes only that it was below the configured floor. A later host observation showed approximately 877 MiB available and 3,490.5 MiB swap in use; this does not establish the cause of the earlier drop.

Independent raw readback pairs 200 completed harnesses with **1,265 passes, zero failures and three ignored**. Seven nested child passes are kept separate. `m2_reflection_apis_lifecycle` started five tests but was interrupted before its terminal summary. Another 83 executable targets and both doctest crates (`pkl_runtime`, `pkl_wasm`) never started. Compiler records cover all 281 integration sources, 284 test executables and 26 examples. Example compilation is not execution. Ignored tests remain gaps; the partial count is not added to earlier workspace counts.

All 212 runtime unit tests are within the 1,265 passing results. The observed ignored cases are the separately invoked M1 frontend probe and two unresolved Java internal-error contracts (`java_supplement_const_lambda` and `java_foreign_entry_renderer_protocol_remains_required`). Unstarted targets can contain additional ignored requirements; this checkpoint does not reduce the historical five-gap inventory to three.

Fresh manifest checking and formatting pass. The final wrapper has 18 focused Python passes and a real compiler/self-spawn pass. Stage127's xtask, all-target Clippy, Wasm and bounded M1 checks apply to the unchanged Rust source; they were not rerun here. Source bookends cover 4,319 selected inputs, including the 159 earlier probe dependencies and all eight previously missing early pins. Post-build dependency discovery still lists 1,830 unbookended paths and eight unavailable depfiles. Complete transitive compiler, fixture and runtime-input closure is not claimed.

The separate post-run artifact acquisition reads and hashes all **310 exact Cargo executable paths (284 tests and 26 examples)** and verifies every Mach-O signature. It has no acquisition errors and stable source bookends. Of those products, 290 match successful compression receipts; the other 20 (four tests and sixteen examples) are unchanged Cargo `Fresh` products matching the original clone's bytes and mode, with no compression flag. Their absence of new wrapper receipts remains explicit: `artifact_provenance_complete=false` and the acquisition's stricter `evidence_integrity_valid=false` are preserved. This gap is not a signature or byte mismatch, and is not promoted into full historical compiler provenance. Artifact attempt01 rejected legitimate Cargo `..` input spellings before reading products; the repaired attempt02 preserves the recorded traversal and explicitly bookends path aliases.

The compiled executables remain in the isolated local cache. Publication includes exact identities and three small binary witnesses, rather than every compiled executable. The original attribute-inspection failure, workspace01 copy-headroom failure, workspace02 interruption and any failed readback attempts remain separate evidence.

The earlier selected Java/native comparisons and M1 native/Node/browser results remain Stage127 observations. This checkpoint does not rerun the full language or glob inventories, implement Java internal-error rendering/catch behavior, complete M2/product APIs, validate all release operating systems or establish the performance targets.

## Evidence and follow-up

The [checkpoint descriptor](../stage128-workspace-storage-checkpoint.json) records
final archive identities and publication readback. The evidence package keeps
both failed workspace attempts, the qualified partial counts, compiler streams,
source and artifact identities, storage experiments, cleanup receipts, and
independent reviews. It excludes prior archive bodies, Cargo caches, toolchain
and JDK bodies, and the 310 local workspace executable bodies. Three designated
storage/self-spawn witnesses are included. The archive is not a portable compiler
environment.

Next, establish enough sustained disk headroom to finish reflection, the 83
unstarted executable targets and both doctest crates, then capture one complete
workspace command with its actual exit. These partial counts must not be summed
with a later retry. Java internal-error rendering and catch behavior, the full
language/glob replay, product APIs, release platforms and performance remain
required follow-up work.
