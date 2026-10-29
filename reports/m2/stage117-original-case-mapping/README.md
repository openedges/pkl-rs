# Original language case mappings

This checkpoint integrates the reviewed Stage117 mapping proposal after Stage120.
It adds 94 case-to-requirement links across 14 unchanged original snippets and
23 mapping rows. Nine semantic families gain explicit reviewed source mappings;
all 475 requirement states and all 4,545 original execution contracts remain
unchanged. The inventory still has 4,442 ready, 71 unimplemented and 32 unmapped
cases.

[The byte readback](case-evidence.json) rechecks retained Stage120 native and Java
stdout, stderr, exit codes and generated-file records: 13 successful evaluations
and one matching error. It records complete channel bytes in base64. This is a
new readback of one observation per case, not a new evaluator or Java execution.
The source join retains Stage120's two subsequent test-only Clippy changes.
Original acquisition provenance is in the
[Stage120 checkpoint](../stage120-shebang-quoted-identifier-checkpoint.json).

Matching `eval` output does not prove every authored fact true. Caught error
strings do not establish full diagnostic frames, and UTF-8 output does not
establish a separate UTF-16 channel. Each case records its narrower semantic
rationale; the [deferred reviews](deferred-source-requirements.json) remain open.

[Validation](validation.json) passes 42 Python tests and manifest freshness.
Applying the unchanged full-gate function to the new manifest and retained
Stage120 ready results leaves 1,768 overlapping unmet conditions: 472
implementation, 472 mapping and 824 required-case conditions. The reduction from
1,780 resolves 12 previously unlinked original cases; it fixes no runtime
mismatch. All nine M2 work areas and full compatibility remain open. No Rust,
Wasm, N-API, performance or hosted-CI execution is claimed by this mapping change.
