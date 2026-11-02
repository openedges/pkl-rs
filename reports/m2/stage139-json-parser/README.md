# Stage139 JSON Parser

Stage139 implements the authored `pkl:json` Parser path in the shared engine, including selected `useMapping`, converter and Resource behavior. Java remains the verification oracle. The public `Instruction` enum gains `JsonParse`; downstream exhaustive matches must be updated.

Trusted standard-library class and module methods follow Java's non-test return-check policy while retaining parameter checks and return-type information used for inferred parents. Object methods continue to enforce return checks. Native regression controls cover both contexts.

The focused native sequence and independent readback record **47 passed, zero failed and zero ignored** across ten integration targets and three unit invocations. Two differential harnesses matched **108 Java observations**: 48 authored programs and six boundary programs, each with power assertions on and off. They compare status and complete expression/stdout/stderr UTF-8 and UTF-16 fields; Java exception-class fields are provenance rather than a Rust class comparison. The two fixture READMEs were added after acquisition and are not captured Java output.

Five registered upstream original CLI contracts, `jsonParser1.pkl` through `jsonParser5.pkl`, matched Java in the recorded exit status, stdout, stderr, public files and workspace-change channels. Their requirement links remain empty. Private Host state is retained as provenance and excluded from public file comparison. The independent reader verifies selected source, candidate and oracle runtime identities; it does not rewalk the final workspace or prove complete transitive compiler/cache closure.

A separate syntax utility comparison matched **303** Java/Rust grammar-acceptance and complete error-hint UTF-16 observations using a 64 MiB Java heap. It is distinct from the focused native totals and does not establish evaluator, VM, converter, whole-inventory or performance compatibility. The comparison record itself does not establish process-exit or runtime/source/artifact closure; the enclosing supervisor separately records command exits and bookends. Long-token scan responsiveness and speed/memory targets remain unproven.

Five fresh default prechecks passed: format, xtask, all-target Clippy, Wasm and manifest freshness. The first precheck attempt stopped at formatting because a boundary harness assertion required line wrapping; the recorded correction changed only formatting. A separate post-format boundary test matched 12 Java observations and is not added to the 47-test focused total. Both the failure and correction remain recorded.

The full workspace exited zero. Its independent aggregate readback qualifies **1,886 passed, zero failed and five ignored across 305 harnesses**, excluding 13 nested child passes. It uses source-pinned child-output grammars and does not prove individual test identities or the absence of interleaving. Ignored tests are not passes. A separate fresh bounded M1 acquisition matched **6,078 results per Native, Node and Chromium target** and **92 Node and 92 browser scenarios**; strict compatibility and the architecture contract are separately reported as passing.

The language manifest refresh changed 84 implementation-link hashes without changing cases, requirement states or mappings. Released historical Cargo caches were removed for disk space. Raw reports, source and captured results were preserved, but historical native binaries from removed caches that were not separately sealed require rebuilding for renewed binary checks. A selection attempt also rejected harmless cleanup receipt filenames before archive creation; that failed attempt and the narrowly scoped successor are retained.

Stage136's full-ready replay remains historical; it was not rerun against this engine. This checkpoint does not establish full M2/API, release-platform or performance completion. Local macOS results are not hosted-CI or other operating-system results.

## Evidence publication

The independently verified archive contains **5,910 members** and is **36,307,521 bytes**. Its SHA-256 is `1b90a55183ffb6a30b211507965d5328835daa7810a56f4e55836f83d5b1d586`. [publication.json](publication.json) binds the copied archive and receipts; [source-files.json](source-files.json) records the final implementation, fixture and documentation bytes. All 14 selected terminal gates passed. The archive is a finite evidence selection, not a complete build or runtime restoration closure.

See [README.restore.md](README.restore.md) for inspection and restoration limits.
