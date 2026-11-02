# Inspecting the Stage139 evidence

Verify the published `evidence.tar.gz` size and SHA-256 against [publication.json](publication.json) before opening it. The archive member roster and per-file hashes are in [members.json](members.json); [archive-readback.json](archive-readback.json) records the independent raw-member verification. Extract only into a fresh empty directory with adequate free disk space, never over a working checkout or earlier evidence.

Archive paths are relative to the recorded checkout root. The archive preserves finite raw captures, selected source and verification records. It does not provide a complete compiler, Cargo cache, Java installation, reference checkout or browser runtime. Receipts can contain historical absolute paths. Re-execution requires the recorded toolchain, oracle and pinned reference revisions; extraction and archive integrity alone do not establish a successful rerun. Historical native binaries from removed caches that were not separately sealed require rebuilding for renewed binary checks.

The original CLI Java/Rust stdout, stderr, exit status, public files and workspace changes are comparison channels. Captured Host-private state is provenance, not a public file comparison channel. The separate syntax utility and bounded M1 experiment have the distinct scopes described in [the report](README.md).
