# Module admission and policy evidence

Authored imports suspend through a shared `CheckImport` request before module
resolution or URI-cache lookup. The request carries the original and resolved
importer identities and the prepared target URI. Successful import sites retain
their receiver; a repeated invocation of the same site uses that receiver before
another admission check. A different site checks its edge independently.

The native Host interface and the N-API/Wasm `SourceSession` bindings share this
state machine. `completeModuleImportCheck` accepts a pending edge;
`rejectModuleImportCheck` preserves the distinction between a typed Java security
exception and a raw Host rejection. `completeDefaultModule` explicitly delegates
supported requests to the Rust default trust and bundled-resolution profile. It
returns `false` without consuming a valid unsupported request. Stale tokens and
expired or closed sessions remain errors. Custom providers choose whether to
delegate; JavaScript does not implement a second trust policy.

Selected standard-library keys in the pinned singleton list bypass key resolution,
while authored import edges still require admission. Custom keys with the same
URI spelling retain their own key kind. General bundled modules such as
`pkl:json` resolve before their internal source is initialized. Qualified Ref type
guards force their authored import. The [architecture](architecture.md#import-admission-and-module-cache-ownership)
describes cache ownership, partial initialization and remaining boundaries.

## Pinned Java policy fixtures

`tests/fixtures/m2-module-admission-policy` preserves 12 sessions, 84 API calls
(80 successful results and four errors), and 280 ordered events from the pinned
Pkl 0.32.0 Java oracle. The fixture has 1,443 payload files and one manifest.
The original policy profiles are allow, deny-json and deny-singletons. They
delegate to the actual Java security manager; their expected data is not derived
from Rust.

API text retains independently captured UTF-16 and Java UTF-8 channels. The
ordered event channel was captured as UTF-8 and contains ASCII text; there is no
independent UTF-16 event capture. Primary-module and per-expression lifecycle
events, full counters and process output remain present in the fixture even when
a Rust adapter cannot reproduce them.

Verify the installed fixture and its tamper regressions with:

```sh
python3 tools/module_admission_policy_fixtures.py --fixtures tests/fixtures/m2-module-admission-policy
python3 -m unittest discover -s tools/tests -p 'test_module_admission_policy_fixtures.py' -v
```

Verification uses the installed fixture without reading the original acquisition
directory. It authenticates the pinned source provenance, exact file set, raw
payloads, API outcomes, event order and counters. Rewriting the local manifest
does not bless altered expectations. Unknown directories are rejected before
descent, and symlinks are rejected before their contents are read. The separate
`--output` export mode still requires the original acquisition directory.

## Scope of compatibility claims

Fixture verification establishes integrity of Java evidence. Runtime comparisons
must separately bind the exact native or portable artifact, inputs, provider
configuration and complete actual channels. Selected API output equality does
not establish full ordered policy events or counter equality.

The supplied-primary policy fixture does not establish Java's complete primary
and query/repl lifecycle. The separate [ModuleKey work](m2-module-keys.md) adds
key-kind dispatch, explicit cache-policy queries, eager import-clause factory
selection and public evaluation operations. Its selected results do not close
the full factory, URI equality, import-glob or ordered-event requirements.
Unsupported profiles and missing lifecycle events remain unexecuted or failed
comparisons. Neither this fixture nor the current admission implementation
completes M2 or establishes the performance targets.
