# Stage121 Pair, inherited Any calls and superclass diagnostics

The shared runtime treats `Pair` as an `Any` value when selecting methods and
diagnostic suggestions. A Pair no longer enters Typed conversion or property
lookup dispatch. Native inherited `Any.getClass` and `Any.toString` arity errors
retain the base declaration frame after evaluating authored arguments. Deferred
superclass resolution preserves the authored type label and use-site span,
including aliases that do not resolve to a valid class.

The canonical change contains six production files, three regression test files
and 21,895 pinned Java fixture files. The generated language manifest preserves
the existing 475 requirements and 4,545 execution contracts.

## Actual local verification

| Check | Recorded result |
| --- | --- |
| Complete workspace | 1,717 passed, zero failed, five ignored |
| Default development checks | xtask, formatting, warnings-denied all-target Clippy, workspace tests, Wasm runtime check and manifest freshness passed |
| Complete ready corpus | 4,339 matches, 19 mismatches, 82 unsupported cases, two Java oracle errors across 4,442 cases |
| Metamorphic checks | 88/88 passed; no previous-pass regressions |
| New selected Native, N-API and Chromium matrix | 624 matching observations per target |
| Retained selected matrix | 902 ordinary matches and two required unresolved internal observations per target |
| Separate M1 acquisition | 6,078 raw matches per target; strict comparison and architecture contract passed |

The strict completion function still reports **1,768 overlapping unmet
conditions**: 472 implementation, 472 mapping and 824 required-case conditions.
The two runtime repairs do not reduce that accounting total. The original
whole-List contract retains its Java oracle error; the fresh native attempt also
reaches its 60-second deadline and is stopped with exit -15. It is neither a
native pass nor an RSS-triggered stop.

The original `errors/extendTypeAlias.pkl` and `errors/propertyNotFound2.pkl`
change from mismatch to match. The ready-corpus replay uses the preserved Java
authorities and freshly executes the current native candidate; reading those
authorities is not a new Java acquisition.

The new portable matrix compares 19,716 exposed raw fields per target. Its 2,020
JVM-only fields remain outside that comparison boundary. The retained matrix's
two internal observations remain required nonpasses. Matching a selected error
does not establish that a later operation was reached.

The workspace count excludes 13 filtered child-test repeats. It includes 273
executable test targets and two zero-test doctest targets. The plain Cargo log
does not establish a complete example-build or compiler-event inventory.
Cargo returned zero while reporting 299 `rust-objcopy` stripping failures caused
by a missing `@rpath/libLLVM.dylib`; their 299 warning summaries and raw stderr
are retained. Successful stripping is not claimed. Later portable and M1 builds
record their explicit library-search environment separately.

## Preserved failures and continuation

The original portable build completed all four build commands but its final
qualifier rejected the unhashed cdylib/rlib artifact names. A read-only correction
qualified the same seven actual compiler invocations and products; no rebuild
was needed for that correction.

The original portable capture completed new Native and N-API results, then its
allocation monitor rejected a browser temporary symlink. That failed capture
remains unchanged. Nine filesystem controls verify the corrected monitor's
isolated browser-temporary boundary. The continuation imports four successful
process records for Native/N-API and executes eight fresh process records for
Chromium and the three retained target groups. It does not relabel the failed
browser attempt as a pass or rerun successful Native/N-API captures.

The first M1 wrapper failed before launching a compiler, Java or a guest. Its
failure remains distinct from the successful second wrapper and acquisition.

## Scope and remaining work

This is recorded macOS development evidence. It does not establish hosted CI,
Linux/Windows, full language/API compatibility or speed/memory targets. Five
ignored tests, the remaining raw nonpasses, 71 unimplemented and 32 unmapped
contracts remain required. All nine M2 work areas remain open.

Stage122's composed UTF-16/glob/diagnostic runtime and the Resource/math work are
separate candidates; their builds or tests are not part of this checkpoint.
The [checkpoint descriptor](../stage121-pair-supertype-checkpoint.json) records
the exact acquisition identities and publication status.

## Published evidence

Ten evidence parts total **185,526,489 compressed bytes**; the largest is
25,392,656 bytes. Independent readback validates all 13,514 unique objects,
19 catalog/supplemental metadata members, 222,571 alias resolutions, complete
ordered tar contents and gzip CRC/EOF. Two aliases require the recorded mode
from `paths.jsonl` instead of the deduplicated object's header mode. Exact public
copies were checked after publication.

The [publication record](publication.json) and [independent readback](archive-readback.json)
bind these parts. Prior Stage114/Stage120 evidence and installed toolchain/runtime
identities are explicit [restoration prerequisites](prerequisites.json). Old
archive bodies were not read or copied. Final documentation is tracked in Git
and is outside the evidence archives. This publication does not claim a complete
standalone toolchain or historical-evidence bundle.
