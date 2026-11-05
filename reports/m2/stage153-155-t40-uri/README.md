# Stage153–155 T40 and URI checkpoint

Stage153–155 adds exact original T40 adapters, native file-URI byte/text conversion and shared bounded `PackageUri`/`PackageAssetUri` validation. Both selected T40 CLI contracts match fresh Java observations; two separately counted Java cache-off controls also pass. File URI records **19 boundary matches and five nonpasses**; package URI records **six complete raw value matches and 31 nonpasses**. Fresh bounded M1 matches **6,078 results per Native/Node/Chromium target**, with 92 scenarios each for Node/browser.

The full source06 workspace **failed**: 2,190 passed, one failed and six ignored across 336 outer groups, excluding 13 nested child passes. After an exact test/tooling correction, source07 separately passes the affected 15-test target, 83 Python tests and five prechecks. This is not a successful rerun of the whole workspace. Full M2/API, package/Host, release-platform and performance requirements remain open.

## Behavior and verification

The T40 adapter preserves exactly two original IDs/contracts, including `eval --settings pkl:settings --color=never --no-project`, source bytes, cwd/environment, expected exits and complete stdout/stderr/exit/generated files. One successful evaluation and one expected ordinary error match their complete contracts. The two Java-only `--no-cache` controls retain the same physical workspace/URI/profile and are **not additional original cases**. Host-private files remain provenance outside the original public-file contract. T36/project remains unimplemented.

Native development file handling distinguishes the exact lowercase Unix `file:///` byte route from accepted Java `File(URI)` text-decoding spellings. Literal Unicode normalization precedes ASCII quoting; percent bytes, separators, NUL and shared Java-style UTF-8 replacement decoding retain their distinct behavior. The 24 Java/native pairs qualify path text and optional peer equality under the recorded UTF-8/unset-normalization profile. The five nonpasses are `nfc_hex_letter`, malformed percent/hex and text/byte NUL. Error/error is not full diagnostic compatibility. Native OS bytes are recorded separately; Java OS-byte identity, non-UTF-8 module filename/source loading, unpaired-surrogate transport and Windows provider behavior remain unqualified.

Shared package types preserve the pinned constructor order for scheme/authority/path, parent segments, version/checksum and absolute asset fragment. Fragment removal reconstructs decoded URI components and revalidates server authority. Pkl’s permissive version/checksum rules are retained. Typed input/reason ownership is shared; default resource policy remains before validation, and custom Hosts keep their own factory boundary. Valid package transport, metadata/ZIP/download/checksum verification, project resolution, configurable native denial and full Java Throwable/caller-diagnostic projection remain incomplete. No production Java fallback is added.

All 37 Java/native API pairs were acquired: six complete stdout value matches with empty stderr, 23 Java-error observations and eight native-Unsupported nonpasses. Raw UTF-8/UTF-16, complete Java exception/security fields and actual native dispositions remain retained. **Zero original package CLI cases are qualified by these API controls**; the 11 original CLI/server/project obligations remain separate. Expected results were not generated from Rust.

| Evidence | Actual outcome |
| --- | --- |
| Source06 Python05/focused | 136 Python tests plus freshness/format passed; 13 focused native tests passed before the final Clippy let-chain edit |
| Source06 prechecks02/normal02 | Five prechecks and three normal example builds passed |
| Source06 full workspace | Cargo101; 2,190 passed, 1 failed, 6 ignored; 336 outer groups; 13 nested passes excluded |
| Same-workspace Protobuf | All 266 slots complete; 240 renderer +8 pre-renderer matches; 18 strict reference nonpasses |
| Source07 validation/target | 83 Python tests, inventory and five prechecks passed; separate whole affected target 15/0/0 |
| Source07 normal03/binding02 | Three actual fresh=false normal Cargo example builds and retained source/storage/copy/signature-verification proof |
| T40 originals/cache controls | 2 complete original contracts match; 2 separate Java cache-off controls pass |
| File/package API boundary | File 19 matches/5 nonpasses; package 6 value matches/31 nonpasses |
| Bounded M1 | Native/Node/Chromium 6,078 matches each; shared 6,078; Node/browser 92 scenarios each |

## Failed workspace and exact source transition

The workspace had no resource stop, complete source bookends and empty supervisor errors. Its lone failure was a stale assertion that `package://example/x` had no standard factory. Pinned Java constructs `PackageAssetUri`, whose nested `PackageUri` rejects the missing version before fragment/transport. The corrected test expects typed `UriSyntax`; production Rust implementation did not change. The complete ordinary Protobuf contract passed inside the failed workspace; its 18 reference nonpasses remain strict-unmet. No new 266-case Java acquisition occurred. The aggregate does not prove every individual test identity or exclude count-preserving interleaving.

Source07 changes exactly three files: the Rust test, `tools/m2_module_eval.py` and its selected-regression tests. The tool passed the original contract to `case_execution` but retained/verified private HOME/TMP state under the manifest adapter ID. Three references now use original ID; evidence folders retain manifest ID. Actual before aliases, patches, formatting and source plans preserve the transition. The formatter only wraps one new pattern line. All other 11,962 declared input pins remain unchanged. The separate 15-test pass neither replaces the failed full-workspace receipt nor adds 15 unique passes to its total.

Python01–04, prechecks01, seal01, source07’s wrong-filename selector (no tests ran), T40’s initial state-key failure and subsequent reader-scope failure remain preserved. Reader03 keeps 653 compiler/source overlaps build-bound while classifying exactly 430 supplemental snapshots: 8 metadata, 415 incidental fixtures and 7 original resources/licenses. Copy/current hashes and the original-resource acquisition joins remain checked; compiler dependency closure is not claimed.

Maintenance retired 321 individually passing ordinary cache executables while protecting failed targets, Protobuf/probes/libraries, retained evidence and the reflection baseline. Independent readback records absence/protected identities and an observed free change of **1,452,425,216 bytes**. Failed-workspace status is retained. This is not exclusive APFS savings, historical executable restoration, additional test credit or guaranteed future capacity.

## Platform and completion scope

M1’s independent reader verifies the complete current raw capture, scenarios, source/artifact identities, exact commands and reviewed Python alias transitions. Strict and architecture predicates pass independently from **one capture**; no second strict CLI run occurred. Known gaps and missing supervisor bookends are empty. The explicit existing platform cache was reused in place under sampled 3 GiB/3 GiB caps, 512 MiB reserve and 1,216 MiB floor; no new clone/inventory proof is claimed. Bounded M1 is not a new Protobuf-specific browser/Node matrix or a performance comparison.

Inventory remains 4,586 cases: 4,485 ready contracts, 69 unimplemented and 32 unmapped. Requirements remain 15 implemented, 42 partial and 418 unimplemented; these labels are not full-language certification. Historical full-ready replay and unselected originals were not replayed. All nine M2 areas and full API/Host/package/project, strict-reference, release-platform and performance obligations remain open.

The accompanying [publication metadata](publication.json), [evidence index](evidence-index.json), [source verification](source-verification.json) and [archive integrity](evidence-integrity.json) identify the final selected payload and local provenance. Publication/archive/Git closeout use separate actual receipts; this report does not infer their success or hermetic restoration. Recorded local checks are not hosted CI.
