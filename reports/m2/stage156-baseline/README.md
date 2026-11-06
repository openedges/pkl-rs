# Stage156 scoped M2 baseline

This checkpoint publishes the retained local Rust baseline and a fresh full-ready
language comparison. It closes baseline acquisition for the next M2 work item;
**M2 remains incomplete**. [result.json](result.json) records source/build/runtime
identities, retained raw-evidence references and hashes, nonpass channels and
separate verifier exits. [structural-audit.json](structural-audit.json) records
the bounded inventory and timing review. These publication files do not replace
the original evidence or constitute another execution.

## Qualified source epochs and outcomes

| Evidence | Qualified source | Recorded outcome |
| --- | --- | --- |
| Five required Rust checks | `2e56343aa84fc814101408f01ff53bf449fbc8a1` | xtask, formatting, all-target Clippy, workspace tests and Wasm check all exit 0 |
| Full workspace | Same Rust source | 2,191 passed, 0 failed, 6 ignored across 336 outer harnesses; 13 nested child passes excluded |
| Full-ready acquisition | `921f23e05b7435d832dc7d99963ca2c3c6851c10` | 4,485 selected: 4,474 pass, 6 mismatch, 2 oracle-error, 3 candidate-unsupported |
| Evidence integrity | Same full-ready capture | exit 0 |
| Strict full M2 gate | Same full-ready capture | exit 1; 1,741 overlapping unmet conditions |

The Rust command was `cargo test --workspace --locked --no-fail-fast
--message-format=json`; the other commands use the required arguments from
`AGENTS.md`. All actual argv, exit codes and durations are retained in the
[result metadata](result.json). The source transition after the Rust run changes
Python tooling/tests and documentation for the JVM workspace profile; the Rust
run is not relabelled to the later HEAD. Finite source bookends and artifact
joins do not establish hermetic compiler/runtime dependency closure. Aggregate
readback does not independently prove every test identity or rule out
count-preserving interleaving. Ignored tests receive no pass credit.

The local host is `research-macmini1.local`, Darwin 27.0.0 arm64, with Rust
1.98.1 and the recorded Temurin Java executable. The pinned Pkl 0.32.0 tools JAR
has SHA-256 `c3b6f6215a08b31453b0fb0edd8c9da0aee31233f65abb8ab07f9c945151e059`;
the Java executable pin is
`9be1d0a740ff6502df1a762145e62860f5de4b7e17658d9cb9498da3acf9d16c`.
Reference revisions remain Pkl `e51fb2f91774265666f94980ec630db794a4db2d`
and Pantry `904a68184e54da378b0a0e8a911a3fab3956f693`. The Rust run uses the
existing normal opt1/debug0 cache, serial tests, sampled 2 GiB process limits
and a 1,216 MiB disk floor. Full-ready uses recorded release builds, two
workers, a 512 MiB Java heap, sampled 3 GiB process limits and the same disk
floor. These are execution conditions, not performance evidence.

## Full-ready scope and retained failures

| Adapter | Selected | Pass | Mismatch | Oracle error | Unsupported |
| --- | ---: | ---: | ---: | ---: | ---: |
| Scalar API string | 3,586 | 3,579 | 6 | 1 | 0 |
| Original module evaluation | 888 | 884 | 0 | 1 | 3 |
| JVM classpath output profile | 1 | 1 | 0 | 0 | 0 |
| Historical collections | 8 | 8 | 0 | 0 | 0 |
| Selected regression | 2 | 2 | 0 | 0 | 0 |

All **4,586 contracts remain required**. The 101 unready contracts comprise
68 package/HTTP/TLS/project-fixture contracts, 32 historical candidates and one
T36 contract; their manifest execution states remain 69 unimplemented and 32
unmapped. They were not executed and are not passes. Requirement states remain
15 implemented, 42 partial and 418 unimplemented. The strict gate's 460
implementation, 460 mapping and 821 required-case conditions overlap; 1,741 is
not a unique bug count.

The six scalar mismatches preserve Java `PklBugException`/exit 1 versus Rust
unsupported diagnostics/exit 2: `error-remainder-zero`,
`trace-stage37-repeat-empty-cast-negative`,
`trace-stage37-repeat-empty-max-count`, `read-stage38-edge-http-opaque`,
`read-stage38-edge-http-invalid-host` and
`read-stage38-edge-http-nonnumeric-port`. Their complete stderr and exit
channels differ. `error-cycle` retains a Java `NoClassDefFoundError` during
`VmStackOverflowException` initialization. Original `api/list.pkl` retains
Java heap exhaustion and a candidate watchdog timeout (exit -9, empty output).
Original `api/benchmarkModule.pkl`, `api/platformModule.pkl` and
`api/releaseModule.pkl` retain missing Benchmark or explicit platform/release
Host capabilities. Neither oracle errors nor candidate timeouts are passes.

Original modules and the schema 2 JVM child use the explicitly retained-upstream
`upstream-original-modules-v1` filesystem profile, excluding unrelated local
fixtures. Their physical environments differ from historical `legacy-full`
captures. This run is a new scoped baseline, not a controlled regression or
performance comparison against those historical environments. The JVM parent
records its build; the child records caller-supplied binding and two repeated
passing observations for one selected contract. Historical CLI provenance does
not become execution of that CLI boundary.

Same-workspace Protobuf separately acquires all 266 terminals: 240 renderer
and eight pre-renderer observations match, with zero harness/structural failures
and **18 strict reference nonpasses**. Its development contract passes and its
strict compatibility predicate remains false. It reuses Java04 fixtures, adds
zero Java executions, and does not execute the named strict CLI. This special
contract is distinct from the 4,485-case full-ready run and the full M2 gate.

## Structural review and next work

The static audit groups 3,586 scalar rows by input SHA-256, expression and
expected outcome. It finds 65 two-row groups; 48 differ in `power_assertions`.
The remaining 17 are review candidates, **not approved deletions**. Identical
keys do not establish identical mode, environment, channels, lifecycle or
coverage purpose. The existing corpus also has 42 fixed-seed generated rows
and 88 unique metamorphic relations; these inventory counts are not new passes.

One recorded serial Rust run takes 8,233.51 seconds. Its largest outer harnesses
are reflection lifecycle (2,828.07 s), Protobuf full (1,667.88 s) and reflection
oracle (806.89 s), together 64.4% of workspace wall time. These observations
identify where to investigate validation cost; they establish no speedup and
are not a Java performance comparison. Keep focused checks during iteration and
complete the applicable full checks on the final source checkpoint. Preserve
purposeful diagnostic-mode, original/minimized, platform-transport and lifecycle
coverage when reviewing apparent duplicates.

The next M2 work item qualifies the existing canonical package fixture, then
implements bounded shared metadata acceptance: checksum validation, parsing and
error ordering, using existing requirement and original-case IDs. Package Host
integration follows separately; a pure acceptance helper does not connect Host.
Retain success/error and capability observations before expanding fixture scope.
The complete original `packages1` source-resolution path remains a later target;
it is not delivered by fixture qualification, URI parsing or a mock-only pass.
Mapping review follows the behavior actually observed; no generic evidence
framework or speculative completion date is introduced.

No new Rust, M1, browser, Node, Linux, Windows, performance or runtime checks were
run for this documentation publication. Previous failed preparation/inspection
attempts and earlier checkpoint failures remain retained. Historical integrity
and current contract applicability must be distinguished in review. The existing
report verifier still binds the live manifest; this checkpoint introduces no
independent historical-contract verifier. A current contract revision must
identify affected claims without rewriting old outcomes.
The 156 MB canonical report, raw files, build receipts and source snapshots stay
at their retained locations referenced by [result.json](result.json); they are
not copied into this report. Full language/API, release-platform and performance
gates remain open.
