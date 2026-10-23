# Shared module loading

The current development implementation executes imported source in the same
Rust VM and heap as the importing module. Native, Wasm and N-API transport the
same typed Host requests. This is an initial module-loading implementation;
complete language, filesystem, package and product API compatibility remains
subject to the [whole-language gate](m2-language.md).

## Identity and Host operations

Module loading separates resolution from source acquisition. A `Resolve`
request carries the importing module's original and resolved identities, the
prepared import URI, and the original key for the requested module. Its Host
completion supplies a `ResolvedModule` with three distinct fields:

| Field | Purpose |
| --- | --- |
| `original_uri` | Original module cache identity, preserved by the resolver |
| `resolved_uri` | Resolved module cache identity and source acquisition key |
| `source_uri` | Source identity used in diagnostics |

A subsequent `LoadSource` request supplies that resolved record. Its completion
provides source text and an inferred module name. An authored module declaration
can supply the diagnostic module name. These requests are separate from resource
`read` operations; the Host supplies capabilities, not another evaluator.

URI preparation occurs in the shared runtime using the declaration's module
identity. A malformed import can fail before a member is evaluated, while source
acquisition remains lazy. The current URI implementation does not yet establish
complete `java.net.URI` equality or primary-module symlink identity semantics.

## Evaluation and cache behavior

Compiled units append to one evaluator-owned program. Block, template and
declaration identities remain distinct across source files, including declarations
at identical byte offsets. A failed compiler append rolls back its uncommitted
tail without invalidating existing closures or previously loaded units. Imported
frontend failures retain their original source and structured diagnostic until
the public rendering boundary.

The cache boundaries follow the recorded Java observations:

- A resolution failure can be retried. Source acquisition and subsequent
  initialization failures are cached with the first diagnostic frames.
- A resolved-key cache hit does not automatically add a new original-key alias.
- Before a source read completes, a Host callback can independently load the same
  key. The runtime does not coalesce these operations or expose a cached receiver
  prematurely.
- After source acquisition succeeds, the actual empty module is published before
  initialization. Initialization callbacks can observe that partial module.
- If a nested load succeeds and the outer source read then fails, an already
  obtained module value remains valid while a fresh import observes the cached
  outer failure. The converse source-completion order is also covered.

Module caches and pending receivers participate in the shared heap's roots.
Session tokens and request IDs reject stale or mismatched completions. Nested
callback operations retain their own cancellation ownership. Closing a session
releases its caches, heap and evaluator-owned code according to the existing
session lifecycle; Java post-close and timeout exception behavior still requires
separate compatibility comparisons.

## Native and portable capabilities

The native `module_eval_probe` uses the development filesystem Host in
`examples/support/module_host.rs`. It resolves absolute local file URIs,
canonicalizes imported paths, and reads source from the resolved file. Original
keys retain their diagnostic URI and inferred filename. This Host does not
implement the complete product resolver, access policy or Java I/O exception
subclass mapping. Other URI schemes remain explicit unsupported capabilities.

The portable fixture Host performs exact lookups in supplied module bundles.
`tools/platform/module-loader-session.mjs` forwards typed resolution and source
completions, finite callback plans and logger events to the shared session. It
does not implement Pkl evaluation or infer filesystem canonical identities in
JavaScript. Bundle observations therefore establish the supplied Host boundary,
not browser filesystem or native security behavior.

Result disposition remains separate from rendered error bytes. Unsupported,
internal and resource-limit completions cannot count as compatible language
errors merely because their text agrees. Expression API failures do not acquire
invented native process exit semantics.

## Expressions evaluated against a loaded module

`SourceSession::compile_expressions` adds fixed expression entries using the
shared parser, resolver and compiler. Each entry has its own `repl:text` source
and declaration identities, while execution uses the loaded module as receiver
in the same VM and heap. The synthetic expression scope does not expose the
module's lexical imports or private declarations. Public methods and closures
keep their original declaration scopes when called through that receiver.

Expression compilation failures are retained as per-entry failures. Starting an
entry initializes the module first, so an initialization error can precede an
invalid expression, and an invalid later entry does not prevent earlier entries
from running. A failed compiler append rolls back before the failure entry is
added. Frontend errors retain the synthetic `repl:text` member identity;
ordinary runtime expression frames keep their separate root name.

The existing property-path session entry remains available. Fixed expression
entries do not establish a mutable REPL or the complete embedding API. Resolution
of module-declared types from these expressions and the full synthetic-URI
resource/import policy still require implementation and differential checks.

## Verification and remaining scope

The [Java-origin fixtures](../tests/fixtures/m2-module-loading/README.md) contain
original import output, file-backed API controls, URI controls, cache behavior,
and source/logger reentry observations. Expected payloads and identity metadata
come from pinned Java 0.32.0 captures. The offline fixture verifier checks their
provenance and integrity; it does not execute an evaluator:

```sh
python3 tools/module_loading_fixtures.py
cargo test -p pkl-runtime --test m2_module_loading --test m2_module_transport --test m2_module_diagnostics --test m2_session_expressions --locked
```

The [import-frame fixtures](../tests/fixtures/m2-module-import-frames/README.md)
separately preserve paired declaration and inline imports across resolution,
source acquisition, parsing and initialization failures. Native diagnostic tests
compare their complete Java UTF-8 and UTF-16 payloads and the original graph
errors while exercising the shared Host, collection and session cleanup.

Original native process comparisons retain full exit status, stdout, stderr and
generated files as described in the [module command contract](m2-module-frontend.md).
Portable output-byte and expression comparisons have their own API boundaries.
Successful component or fixture checks cannot substitute for those executions.

Module `extends` and `amends`, qualified imported types, complete URI and module
identity semantics, remaining standard modules and external capabilities, and
the full upstream loading corpus remain unfinished. Constructor/setup errors,
deadline and post-close outcomes listed by the fixture package remain pending
comparisons. These are outstanding obligations, not exclusions from M2.
