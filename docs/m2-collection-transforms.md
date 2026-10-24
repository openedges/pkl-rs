# Collection transform verification

This development contract extends the shared M2 evaluator. It does not replace
the [whole-language completion gate](m2-language.md) or establish complete
collection, quantity, public API, platform, or performance compatibility.
Implementation and checkpoint status are tracked in [PLAN.md](../PLAN.md).

## Shared runtime scope

List and Set transforms cover `map`, `mapIndexed`, `mapNonNull`, and
`mapNonNullIndexed`. Map transforms cover `map`, `mapKeys`, and `mapValues`.
Callbacks use ordinary VM function calls, type checks, and resumable Host
operations. The source, callback, current operands, and result builder remain
reachable while a call or hash operation is suspended. Each result is validated
and inserted before the next callback runs. Non-null transforms discard language
nulls without forcing their retained default.

Map transforms look up each value using the current key before invoking the
callback. Set and Map result construction preserves the Java hash/equality
protocol, duplicate representatives, and iteration order. `Map.keys` constructs
its hash table through the mutable-set insertion path and retains the original
key order separately; `Map.values` performs ordered key lookups. `List.toSet`
and `Set.toList` exercise those values through the same shared engine.

Strict completed results must not retain unnecessary callback environments or
source modules. Tests distinguish caller-owned roots from VM-owned roots, collect
before and after polling, and exercise cancellation, retry, nested Host calls,
escaped closures, and stale handles. Empty result identity is observable through
renderer conversion diagnostics. Structural equality is not a substitute for
reference identity.

The `Null` constructor retains an Object or function default without forcing it.
Its type errors, function amendments, and lookup/call diagnostics are part of
the comparison. Duration values are a shared scalar foundation for real values
such as `1.s`, including numeric unit properties, type checks, representation,
equality, hashing, and identity. This foundation does not establish the remaining
Duration arithmetic, conversion methods, ISO formatting, DataSize, or typed
product API transport. Direct M1 primitive-probe transport remains separate
from the evaluator's string-result boundary.

## Java authority and scope accounting

`tests/fixtures/m2-collection-transforms` contains independently acquired Java
0.32.0 source, input operations, explicit reader rules, raw API channels, logger
frames, and ordered events. Every string retains Java UTF-16 units and Java
UTF-8 bytes. `tools/collection_transform_fixtures.py` authenticates literal
authority hashes, reconstructs the derived fixture from those records, and
rejects missing, extra, altered, or inconsistently encoded payloads.

Each API call retains one of three scope labels:

- `ordinary`: the original input reached the intended operation, including
  intended language errors. Matching errors count only with the expected
  language-error classification and matching effects.
- `intent-not-reached`: an authored input failed before exercising its intended
  feature. Its complete raw outcome remains compared, but it is not positive
  evidence for that feature. Corrective inputs are additive cohorts.
- `public-api-boundary`: post-close API observations remain separate. Java
  exception text may contain process-specific identities; these bytes are
  preserved, never normalized into ordinary compatibility passes.

The expected reader event stream is evidence, not a script for the Host to replay.
Providers respond from original input rules and their actual attempt counts.
They preserve typed resource completions and use a public module-URI evaluation
entrypoint. The primary source is served by an explicitly configured virtual
provider; bundled imports use the shared module policy. These controls do not
prove a general filesystem provider or complete Java ModuleKey lifecycle parity.

The original, hash-pinned `CollectionProbe.java` configures the resource
allowlist as `Pattern.compile("ct:.*")`. This input is part of the Java helper
provenance; it is not inferred from expected errors or added retroactively to
the derived fixture profile. The native and portable capture Hosts apply this
same bounded policy before looking up a reader or incrementing its counters.
The pinned Java `SecurityManagers.Standard.checkRead` uses `lookingAt()`, so
this particular pattern accepts a case-sensitive `ct:` prefix. This adapter
does not implement general Java regular-expression policies.

Denied requests complete through the shared typed security-manager failure
path, using the upstream allowlist message and actual requested URI. Their
policy decisions are recorded separately from reader/log effects and Java
administrative events. In particular, the four Java07 `output.text` calls that
stop at `prop:pkl.outputFormat` remain diagnostic-only observations; Java08's
explicit renderer controls provide separate successful output evidence.

## Native and portable acquisition

Use fresh output directories and the pinned Rust/browser tooling. A build proof
identifies the exact native capture executable, N-API addon, Wasm JavaScript and
Wasm binary, their hashes, and a stable source snapshot. Cargo controls and the
shared runtime sources are part of that snapshot. Build success alone is not
compatibility evidence.

The implemented acquisition tools are:

- `tools/collection_transform_native.py`: invokes the frozen native capture
  executable for each original process input and reconstructs its sessions from
  raw files. It checks UTF-16/UTF-8 agreement, effect/counter joins, retained
  outputs, and terminal cleanup, and rejects unconsumed raw records. Exit zero
  means acquisition integrity; inspect `ordinaryContractPassed` separately.
- `tools/platform/compare-collection-transforms.mjs`: runs the same fixture on
  the actual N-API addon and Chromium/Wasm. Arguments explicitly identify Python,
  the browser binary/hash, the build proof/hash, and the pinned Playwright
  package. Node records its actual executable. `--preflight-only` authenticates
  inputs without evaluating Pkl and cannot establish compatibility.

Both tools preserve complete API results and ordered read/log effects. The
portable helper also records actual operation sequencing and module requests.
Its administrative observations are not fabricated Java events. The independent
gates cover acquisition integrity, raw API equality, ordinary compatibility,
and ordered effects; full Java event/counter and whole-product compatibility
remain unproved. The portable strict selected gate retains post-close and other
raw failures even when the ordinary contract passes.

If a reader or verifier defect is discovered, preserve the initial failure.
An independently recorded readback may reconstruct existing raw observations
without rerunning the evaluator, provided it authenticates unchanged original
payloads and records the corrected reader. Such a readback is distinct from a
new runtime execution. Unit tests of scripted Hosts and verifier rejection paths
are tooling evidence, not additional Java compatibility cases.

An unchanged original M2 input must also pass through its original language
adapter. Success in these focused sessions does not silently update the original
case's status in a full selected acquisition.
