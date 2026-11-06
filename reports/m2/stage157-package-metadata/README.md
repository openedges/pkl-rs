# Stage157 package fixture and metadata acceptance

This draft records a bounded shared Rust metadata-acceptance implementation,
qualified package fixtures and selected Java/native observations. **Final-source
qualification, the required full Rust checks and M1 remain pending.** The source
commit is not frozen. [result.json](result.json) retains concise source/artifact
pins, raw receipt references, exact outcomes and remaining obligations.

The implementation adds `packages/metadata.rs` and reuses the existing JSON
scanner with minimal internal visibility changes. It accepts an already acquired
metadata response: checksum validation precedes decoding/parsing; required
metadata and dependency fields preserve Java string units; ZIP-URL scheme
validation follows parsing. Exact `$skipChecksumVerification` behavior depends
on test mode. Typed failures retain the selected complete Java messages;
unqualified annotation and exception paths remain explicitly unsupported.

This pure acceptance function performs no network or security admission, ZIP
loading, package caching or project resolution. Its typed result prepares a
later Host integration; the helper itself does not connect Host.

## Behavior, observation and result

| Behavior | Cases and complete observations | Recorded result and meaning |
| --- | --- | --- |
| Canonical fixture generation | Five pinned packages; Gradle-generated metadata, ZIP and checksum identities independently qualified against originals | Exit 0 in 4.875 s; 15 generated files. Qualification establishes these fixtures, not evaluator compatibility |
| Actual HTTPS fixture lifecycle | 14 GET, HEAD, missing-path and redirect route checks with status, body bytes and applicable Location | All 14 pass; server exit 0. The 161-byte HEAD warning remains in stderr. No candidate evaluator participates |
| Checksum before parsing | `malformed-wrong-checksum`, `malformed-correct-checksum`; exact, disabled and near-match skip-marker controls | Java/native protocol 2 matches complete selected error class/message and result files; no parsed success replaces a checksum failure |
| Required metadata and string decoding | Five `canonical-*` controls, six `missing-*` field controls and `invalid-utf8-name` | All 19 selected API cases match their exact named-file set and complete UTF-8/UTF-16 payloads, including the checksum controls above |
| Existing shared regressions | `m2_package_metadata`, `m2_package_uri`, `m2_json_parser` | Focused Rust run exits 0: 5 metadata, 6 URI and 9 JSON tests pass; no failed or ignored tests |
| Fixture/oracle setup for originals | Six existing original package IDs, retained below; bounded physical workspace and fixture-derived argv | Java-only observations meet their expected exits and request counts. No candidate comparisons, ready-state promotions or compatibility credit |

The five generated packages are `badImportsWithinPackage@1.0.0`,
`badMetadataJson@1.0.0`, `badPackageZipUrl@1.0.0`, `birds@0.5.0` and
`fruit@1.0.5`. ZIP payload identities are retained in the original qualification
manifest; this publication does not copy or reopen archive bodies.

The metadata API matrix uses an HTTP response stub, with each oracle case in a
separate pinned Pkl 0.32.0 JVM. It observes selected required public fields and
complete error class/message files. It does not qualify all optional fields,
annotation export, dependency-map iteration order, resolver cache lifecycle,
transport, CLI output or generated files. Success of the API matrix does not
establish an original package evaluator contract.

## Preliminary original Java observations

The rows below use the existing original case IDs, whose common prefix is
`pkl:language-snippet:pkl-core/src/test/files/LanguageSnippetTests/input/packages/`.
The original manifest rows remain unchanged.

| Original suffix | Java exit | HTTPS requests | Meaning |
| --- | ---: | ---: | --- |
| `packages1.pkl` | 0 | 6 | Oracle success under this bounded fixture profile |
| `badImport1.error.pkl` | 1 | 0 | Oracle error observed |
| `badImport6.error.pkl` | 1 | 1 | Oracle error observed |
| `badImport7.error.pkl` | 1 | 1 | Oracle error observed |
| `badImport8.error.pkl` | 1 | 2 | Oracle error observed |
| `badRead1.error.pkl` | 1 | 0 | Oracle error observed |

This is a preliminary 13-file physical workspace profile with derived fixture
arguments. It is distinct from historical whole-workspace environments and
from executing the unchanged original invocation. Raw process channels,
request logs, original-case hashes and derived-invocation hashes remain in the
Java-only report. The fixture is revalidated, source/tool identities remain
stable and the workspace is restored. No candidate ran for these six rows.

## Validation identities and retained attempts

The native probe is the actual Mach-O artifact
`0f0e0122a41226c7c838a6ede9b29f2cd70efbf0c74a51f479e12c744f0bd195`.
The comparator labels it caller supplied; the separate local build, compiler,
storage, source-bookend and dynamic-library receipts are retained through
`artifact-closure.json`. Transparent filesystem compression does not introduce
a launcher or a runtime sidecar. These bounded joins do not establish hermetic
compiler/runtime dependency closure. The recorded base HEAD is
`843ea3b81bdc1b2e32ee9b9ba8bbe982da0a90bf`; new source hashes identify the
actual uncommitted implementation, not a qualified commit at that HEAD.

The oracle uses Pkl 0.32.0 tools JAR SHA-256
`c3b6f6215a08b31453b0fb0edd8c9da0aee31233f65abb8ab07f9c945151e059`
and Temurin 21.0.12.1 on macOS arm64. The qualified fixture manifest hash is
`c56a310b6a52ef72bb392b41473089fbc7644350e873693f1cdba329605e7210`.
Complete identities remain in the receipts referenced by [result.json](result.json).

Formatting and the final focused probe build exit 0 with bounded source
bookends stable. Seventeen Python tooling tests, inventory freshness and
manifest freshness pass. The manifest update refreshes 53 implementation-link
hashes without changing requirement/case counts, states or readiness.

Preserve the earlier protocol 1 API capture separately: it lacks the exception
class's UTF-16 channel and receives no protocol 2 comparison credit. The first
native prebuild check failed because its receipt directory was missing before
Rust execution. The first original-Java preparation stopped on symlink metadata
preflight with zero cases and no JVM execution. Later successful attempts do
not rewrite those outcomes.

## Remaining checkpoint work

The applicable five required Rust commands and fresh M1 native/Node/browser
validation have not yet been recorded as a complete final-source checkpoint.
Focused formatting and tests above do not substitute for those pending checks.
Source freeze and the final implementation commit remain pending.

Follow-up implementation must connect the shared acceptance result to package
Host admission, actual acquisition and errors, then address ZIP/checksum,
cache/project resolution and evaluator lifecycle with original candidate
comparisons. The complete `packages1` Rust path remains a goal. All 4,586 M2
contracts, including 101 unready contracts, remain required. This work changes
neither the strict completion gate nor the open full language/API,
release-platform and performance requirements.
