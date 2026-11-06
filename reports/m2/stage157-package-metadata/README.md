# Stage157 package fixture and metadata acceptance

This checkpoint records the shared package metadata acceptance change at
source commit `a2aa18cddc72799b1eb20ee617446b5faa2f3c57`, following initial
implementation commit `15e77ad7f42d970a381aedfc6db6ebb9e2b2dea8`. The selected
Java/native API matrix, all five required Rust commands and bounded M1 capture
are qualified for this development checkpoint. [result.json](result.json) retains
exact receipt paths, SHA-256 identities and unresolved obligations. Evidence is
local macOS arm64 execution, separate from hosted CI. Full M2 remains incomplete.

The work belongs to M2-04 `semantic:modules` and M2-08
`semantic:diagnostics`. It adds `packages/metadata.rs` and reuses the existing
JSON scanner through minimal internal visibility changes. The function accepts
an already acquired response: checksum validation precedes decoding/parsing;
required metadata and dependency fields preserve Java string units; ZIP-URL
scheme validation follows parsing. Exact `$skipChecksumVerification` behavior
depends on test mode. Typed failures retain the selected complete Java messages;
unqualified annotation and exception paths remain explicitly unsupported.
The final fix boxes the checksum-mismatch arguments to satisfy Clippy's
`result_large_err` check without changing the selected public messages.

The helper performs no network/security admission, ZIP loading, package cache
or project resolution. Its typed result prepares later Host integration.

## Behavior and observations

| Behavior | Observation | Result and scope |
| --- | --- | --- |
| Canonical fixtures | Five packages; Gradle-generated metadata, ZIP and checksum identities qualified against originals | Generation exit 0; 15 files; fixture qualification only |
| HTTPS fixture lifecycle | 14 GET, HEAD, missing-path and redirect route checks, including complete bodies and applicable Location | 14 pass; server exit 0; 161-byte HEAD warning retained; no candidate evaluator |
| Metadata acceptance and errors | 19 protocol 2 Java/native cases; five canonical controls, checksum-before-parse, exact/near skip markers, six missing fields, invalid UTF-8 name | 19 exact named-file-set and complete UTF-8/UTF-16 payload matches with final native02/compare02 |
| Required current-source prechecks | `cargo xtask verify`, formatting, Clippy workspace/all-targets/locked with warnings denied, runtime Wasm check | All four exit 0 at `a2aa18cd`; full workspace results follow below |
| Shared-engine platforms | One actual native/Node/browser capture; strict raw and architecture predicates independently derived from the same bytes | 6,078/6,078 Java matches per target; 6,078 shared; 92 Node and 92 browser scenarios; both gates pass |
| Focused regressions and tooling | 5 metadata, 6 URI, 9 JSON Rust tests; 17 Python tooling tests and inventory/manifest freshness | Native02 focused 20 tests use final matching source contents at captured HEAD `15e77ad7`; Python/freshness observations remain historical unchanged-source evidence |
| Original package setup | Six original package IDs, bounded 13-file physical workspace, fixture-derived argv | Java-only observations; no candidate execution, readiness promotion or original compatibility credit |

The five packages are `badImportsWithinPackage@1.0.0`,
`badMetadataJson@1.0.0`, `badPackageZipUrl@1.0.0`, `birds@0.5.0` and
`fruit@1.0.5`. Qualified fixture manifest SHA-256 is
`c56a310b6a52ef72bb392b41473089fbc7644350e873693f1cdba329605e7210`.
Archive body identities stay in their original qualification metadata; this
publication does not copy or reopen archive bodies.

The selected metadata API uses an HTTP response stub and a separate pinned
Pkl 0.32.0 JVM per oracle case. It observes required public fields and complete
error class/message files. Optional fields, annotation export, dependency-map
iteration order, resolver cache lifecycle, transport, CLI output and generated
files remain outside this observation. Its 19 successes give no original package
evaluator compatibility credit. The M1 run exercises existing shared-engine
regressions; it does not export the new metadata API through platform bindings
or qualify package networking capabilities.

## Source, artifact and execution identities

The final actual Mach-O metadata probe SHA-256 is
`ee331add79b596c0f4867d7523d2acc63081ec893ea331a7dc082a8f20c8c239`;
its preserved executable and build/storage/dynamic-library receipts are linked
through native02 `artifact-closure.json`. The comparator describes a caller
supplied executable; the separate local build binding is retained. Transparent
filesystem compression introduces no launcher or runtime sidecar. These joins
do not establish hermetic compiler/runtime dependency closure.

Native02 built and ran the focused 20 Rust tests with recorded HEAD
`15e77ad7` and changed source bytes later
committed as `a2aa18cd`. All 14 bounded bookended files match the current bytes
and that final commit's Git blobs. The build's captured HEAD remains 15e;
this source-content join does not relabel the historical capture. Current-source
prechecks02 and M1capture02 themselves record final HEAD `a2aa18cd`.

The oracle is Pkl 0.32.0 tools JAR SHA-256
`c3b6f6215a08b31453b0fb0edd8c9da0aee31233f65abb8ab07f9c945151e059`
with Temurin 21.0.12.1. Rust 1.98.1, canonical Python 3.14.8, Node 26.10.0_1,
pinned Playwright/Chromium and wasm-bindgen 0.2.108 identities are retained in
the receipts. The M1 capture reused `stage142-yaml-native-cargo-01` in place
under an explicit different-cache condition, with release opt 3/thin LTO/one
codegen unit and one Cargo job. No installation, cache clone, cache inventory
or cache equivalence is claimed.

All eight M1 commands exited 0 in 400.835 s. Guard sampled owned/aggregate RSS
peaks were 2,579,447,808/2,581,577,728 bytes; minimum free disk was 6,903,185,408
bytes, above the 1,275,068,416-byte floor. These are sampled observations, not
hard peaks or performance qualification. The guard observed 67 owned processes
and terminal live set was empty; later process closure was checked.

## Full workspace and retained strict obligations

The final `cargo test --workspace --locked --no-fail-fast --message-format=json`
command exited 0 in 8,098.7467305 s at `a2aa18cd`. Independent aggregate
readback records **2,196 passed, 0 failed and 6 ignored** across 337 outer
harnesses. The 13 nested child passes are excluded from those outer totals.
Finite source bookends are stable; the terminal owned-process set and final
Cargo/Rust/Java/Node scan are empty. Aggregate parser limitations remain in the
reader receipt: the counts do not independently prove every individual test
identity or exclude count-preserving interleaving.

The six ordinary-run ignored entries are unchanged from the baseline and are
not counted as passes. Compatibility gates among them remain unmet:

- `frontend_probe` (`frontend.rs`)
- `java_supplement_const_lambda` (`m2_annotation_const_oracle.rs`)
- `java_foreign_entry_renderer_protocol_remains_required` (`m2_list_apis_oracle.rs`)
- `java04_strict_full_compatibility_gate` (`m2_protobuf_java04_full.rs`)
- `java_foreign_entry_internal_protocol_remains_required` (`m2_set_apis_oracle.rs`)
- `java_glob_internal_error_protocol_is_unresolved` (`m2_string_apis_oracle.rs`)

A separate raw Protobuf reader qualifies all 266 actual native terminals:
240 renderer matches, 8 pre-renderer matches, 18 retained reference nonpasses,
0 harness failures and 0 structural-failure slots. The completed experiment
contract passes; strict compatibility remains false and the named strict gate
was ignored. Retained historical Java fixture bytes are the reference authority;
there was no fresh Java 266 acquisition, no separate strict CLI invocation and
no Java exception-class projection. The 18 nonpasses remain failures of full
compatibility accounting.

The later publication changes only documentation. Runtime qualification keeps
its `a2aa18cd` source identity; documentation publication does not relabel the
capture HEAD or require another runtime acquisition.

## Preliminary original Java observations

Original IDs share prefix
`pkl:language-snippet:pkl-core/src/test/files/LanguageSnippetTests/input/packages/`.
The unchanged rows and full raw Java channels/request logs remain in the
Java-only report.

| Original suffix | Java exit | HTTPS requests |
| --- | ---: | ---: |
| `packages1.pkl` | 0 | 6 |
| `badImport1.error.pkl` | 1 | 0 |
| `badImport6.error.pkl` | 1 | 1 |
| `badImport7.error.pkl` | 1 | 1 |
| `badImport8.error.pkl` | 1 | 2 |
| `badRead1.error.pkl` | 1 | 0 |

This fixture-derived profile differs from unchanged original invocation and
historical whole-workspace environments. All six remain Java-only.

## Retained attempts and remaining work

Protocol 1 API capture lacks the exception-class UTF-16 channel and receives no
protocol 2 comparison credit. The initial native prebuild failed on a missing
receipt directory before Rust execution; initial original-Java preparation
failed symlink/metadata preflight with zero cases and no JVM execution.
Prechecks01 retained Clippy exit 101 for `result_large_err` at recorded HEAD 15e;
the final boxed-failure fix and prechecks02 pass do not erase it. M1output01
retained child exit 1 before the inner driver because its selected old platform
cache was absent, although its supervisor reached terminal completion.
M1output02 records a distinct existing-cache condition. Earlier native01 and
compare01 artifact/source observations remain historical.

The next bounded step targets original `badImport6.error.pkl` and
`badImport7.error.pkl` with real Host HTTPS acquisition through metadata
acceptance, ending before ZIP loading. A normal accepted-metadata case should
then reach an explicit unsupported ZIP boundary. Record full original
input/argv/environment, ordered requests and raw error/result channels; this
does not establish full Host/package resolution compatibility. Later work still
requires ZIP/checksum behavior, cache/project resolution, evaluator lifecycle
and original candidate comparisons. The complete `packages1` Rust path remains a goal. The
4,586 required M2 contracts and 101 unready contracts are unchanged; no full
language/API, release-platform or performance gate closes through this work.
