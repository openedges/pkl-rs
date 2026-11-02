# Stage130 Java exception renderer and Cargo jobserver forwarding

The shared runtime adds a Java throwable-graph rendering component for the Pkl
internal-error boundary. It models throwable descriptions, stack frames,
causes, suppressed exceptions, shared references and cycles, as well as URL
encoding and the caught-error text transformation. The component preserves
UTF-16 code units where the Java boundary exposes them. It is not yet a complete
projection of actual VM errors and logical stacks into that model; Java
internal-error rendering and catch behavior in real evaluations remain open.

## Oracle and focused checks

The definitive `stage130-java-03` acquisition uses the pinned Pkl 0.32.0 Java
implementation and Temurin 21 on local macOS arm64. It records **54 cases in
both LF and CRLF profiles**, including complete UTF-8 output and raw UTF-16 code
units. The Java observations are component evidence, not a claim that runtime
stack projection or end-to-end catch behavior matches Java. The focused Rust
run `stage130-targeted-04` completed with Cargo exit 0: **26 tests passed**
across the Java exception renderer and existing guest-catch targets, with no
failures or ignored tests in those three harnesses.

The workspace storage wrapper also fixes propagation of Cargo jobserver file
descriptors to its compiler child. The focused `stage130-wrapper-tests-01`
Python suite records **21 passing tests**. The fresh `stage130-wrapper-tests-02`
repeat also passes all 21, adding explicit test-source and Python-executable
bookends missing from the first minimal receipt; the two runs are not summed. Local `cargo xtask verify`, format,
workspace all-target Clippy with warnings denied, Wasm runtime check and M2
manifest freshness checks each completed with exit 0. The manifest result is
`stage130-manifest-02`; the earlier `stage130-manifest-01` failed in its child
gate because `python3` was passed as a non-absolute executable to `execve`, so
Python did not start. That failure remains part of the record.

The first Java attempt, `stage130-java-01`, failed at the identity command's
gate. `stage130-java-02` completed an earlier oracle acquisition but is
superseded by `stage130-java-03`, which captures the corrected helper. The
three initial targeted runs `stage130-targeted-01` through `-03` each exited
101; their original failures remain separate from the successful fourth run.

## Platform execution

`stage130-platforms-03` completes all eight commands with exit zero. Independent
raw readback verifies **6,078 Java matches per Native, Node and Chromium target**
and **92 scenarios each in Node and Chromium**. Both the strict predicate and
the separate architecture predicate pass on this one acquisition. The reviewed
known-gap set is empty; the two historical regex gaps are not reintroduced as
exceptions. The platform documentation now reflects that current contract.

The host uses Rust 1.98.1, Node 26.10.0, Playwright 1.64.1, Chromium 156.0.8078.4
and Temurin 21.0.12.1+1. Platform builds use a separate Cargo directory and
overlap the workspace tests; these runs are not performance measurements. The
existing bounded platform suite exercises the shared engine and adapters. It
does not expose or independently execute the new synthetic throwable matrix
through a public Wasm or N-API exception API.

Platform attempt 01 fails before compilation because the storage wrapper's
target directory is absent. Attempt 02 builds Native and N-API but fails while
linking Wasm: `rust-lld` cannot load `libLLVM.dylib`. Attempt 03 disables the
optional storage wrapper for the platform profile, as earlier platform runs
did, and completes. The original attempts and build logs remain preserved.

The producer's source snapshots match current source bytes and its recorded
stability checks. Of those source paths, 138 lack independent supervisor input
bookends; those two forms of provenance remain distinct. The exact oracle JAR
matches its lock, metadata and canonical input pin. Its known cache alias is
checked during readback, but acquisition-time alias identity was not captured.
Complete compiler, runtime and dynamic-library dependency closure is not claimed.

## Complete workspace and artifact readback

The complete local `cargo test --workspace --locked --message-format=json`
command exits **0**. Independent aggregate accounting records **287 completed
harnesses: 1,789 passed, zero failed and five ignored**, excluding 13 filtered
child passes. All 285 expected executable test targets and both zero-test
doctest crates complete. The focused 26-test run is part of this coverage and is
not added to the workspace total. The five existing ignored tests remain
non-passes; `all_required_pass` is false.

The source parent is `008e48c260122842d270d4d6c2310baa41aeaf19`; actual working-tree
source identities are bookended separately from that commit. Execution uses
Rust 1.98.1 on macOS 27 arm64, one build job, two test threads, test optimization
level 1 and no debug information. It takes 4,154.268 seconds. The 1 GiB disk
floor remains intact: minimum sampled free space is 12,965,552,128 bytes. Peak
sampled owned-process RSS is 1,194,557,440 bytes. No guard stops the command and
no owned child remains. These are supervisor observations, not product
performance comparisons.

Post-test reads identify 311 executable products: 285 tests and 26 compile-only
examples. Cargo reports 307 rebuilt products and four cached products. Exact
current bytes match Stage130 wrapper and signature receipts for 310 products;
the cached xtask unit-test executable has no matching current-stage wrapper
receipt. No new signature command is run during readback. The original cache
clone is reused without a claim of complete inherited cache provenance.

The harness qualifier correlates Cargo artifacts, source rosters, stderr
headings, raw summaries and pinned nested-child source proofs. It establishes
aggregate counts under those assumptions. It does not independently establish
exact individual test identities or exclude count-preserving interleaving.
Complete compiler, runtime and rustdoc input closure remains unestablished.

## Evidence and remaining work

The [checkpoint descriptor](../stage130-java-exception-renderer-checkpoint.json)
links the published evidence archive, independent streaming readback and copy
receipt. The finite archive contains original outcomes, source and fixture
bodies, compiler-wrapper receipts, verification helpers and retained failures.
It excludes Cargo caches/test executable bodies, installed JDK/JAR/browser
bodies and old evidence archive bodies. Thirteen synthetic archive-reader
controls are tooling checks, not Pkl tests.

Primary receipts include `stage130-java-03`, `stage130-targeted-04`,
`stage130-wrapper-tests-02`, `stage130-workspace-01`,
`stage130-workspace-readback-01`, `stage130-artifacts-01`,
`stage130-platforms-03` and `stage130-platforms-readback-01`, under `target/m2`.
Their earlier failed or superseded acquisitions remain separate. Java02 is
superseded specifically because the custom throwable's default description was
constructed by the helper; Java03 delegates it to `Throwable.toString()` and
validates actual class identities. Targeted attempts 01–03 retain a nonexistent
test-target request, a missing receipt directory, and Rust type-inference
errors plus the original jobserver warnings, respectively.

These are local macOS observations, not hosted CI or Linux/Windows release
results. CRLF is an explicit JVM profile on macOS, not Windows execution. The
new component does not capture actual Java implementation stacks, infer caller
frames, add a typed VM bug payload, or make unsupported URI errors catchable.
Its extracted ordinary catch transformation preserves existing behavior.

Full M2 language, API and performance requirements remain open. The whole
language/glob inventories are not replayed here. The existing unmapped and
unimplemented requirements remain required. Follow-up power-assertion work in
isolated source copies is not part of this checkpoint's implementation or
workspace results.
