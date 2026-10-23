# Extended ModuleKey verification

The extended controls preserve the Java 0.32.0 observations originally acquired
for Stage49. They contain 35 authored cases, 140 evaluator sessions, 944 API
calls and 10,124 ordered Java events. The checked-in fixture is
`tests/fixtures/m2-modulekey-extended`; its expected results come from the
original Java captures. The exporter does not evaluate Rust to generate them.

The ledger distinguishes 836 ordinary calls, 100 calls whose initial query-local
alias prevented the intended behavior from being reached, and eight invalid
fragment calls that produced Java internal errors. A matching diagnostic in the
second group does not prove the originally intended behavior. The third group
retains its original exception class and raw result even when Rust differs.

## Host and public evaluation boundary

`tools/platform/modulekey-extended-session.mjs` supplies the configured Host
inputs to the shared evaluator through `SourceSession.moduleEvaluator` and
`evaluateModuleSource`. Every operation retains its URI-only, supplied-contents
or `repl:text` source profile. Absent contents and an empty string are distinct.
The Rust engine creates wrappers, resolves inherited URIs and owns semantic
module caches. There is no Java evaluation fallback.

The provider retains descriptor ownership and authored key snapshots. A key's
cache policy and factory-time source remain fixed after selection; a load-time
source observes the state at its actual source request. Disabling the custom
factory affects future selection, while existing keys remain usable. File
sources are exact, preconfigured virtual inputs. This does not implement all
Java File URI or secure-path callbacks.

Ownership checks follow the request phase. A selected key URI, an import URI
and an element candidate URI are different fields with different meanings.
Custom Synthetic keys retain their registered opaque handle. Engine-created
query keys and supplied-source wrappers use separately checked internal
contexts. A wrapper inherits URI resolution without invoking its delegate's
custom override. Capabilities and element membership are supplied only from
explicit configured facts; missing membership is not implicitly false.

The recorded `deny-repl` and `deny-supplied` modes affect module resolution.
They do not turn an import-edge trust check into an allowlist check. Resolution
uses the selected key's URI and an exact original raw policy cause. Internal
Synthetic resolution performs that policy check; supplied-source wrapper
resolution has the upstream bypass. A missing cause for a known denial is an
acquisition error, rather than an invented diagnostic.

## Separate evidence and gates

Each run records the actual public calls, delivered requests, completion
arguments and returns, configuration changes, explicit GC operations and
closure. Automatic per-poll collections have a separate count. These events
are not renamed or expanded into Java callbacks.

Result strings are compared as UTF-16 units and Java-compatible UTF-8 bytes
without normalization. The result event also projects to `success\n` or
`error\n` for comparison with the original Java status file. This projection is
an adapter protocol, not an operating-system process exit status. Original
Java exception class, null-message flag, counter bytes and ordered events remain
available in the fixture.

The comparison reports raw byte equality, ordinary compatible calls,
diagnostic-only equality, internal-error equality, acquisition integrity and
cleanup separately. Cleanup checks retained result ownership, terminal heap
slots and release of provider key registries. The registries retain handles
for the session; these bounded controls do not prove that whole-process memory
is bounded for arbitrarily long sessions.

An ordinary error requires the shared engine's `language-error`
classification. `unsupported`, `limit-exceeded` and `internal-failure` results
retain their raw comparison but do not become ordinary compatibility passes.
Missing or unknown error classifications fail acquisition after the raw
delivered result has been retained.

Full ordered Java event/counter parity remains an unsatisfied gate. Neither
fixture integrity nor selected API equality completes that gate, M2, product
compatibility or the performance requirements.

## Development checks

```sh
python3 tools/modulekey_extended_fixtures.py --fixtures tests/fixtures/m2-modulekey-extended
python3 -W error::ResourceWarning -m unittest discover -s tools/tests -p 'test_modulekey_extended_fixtures.py' -v
node --test tools/platform/modulekey-extended-session.test.mjs tools/platform/compare-modulekey-extended.test.mjs
```

Actual native, Node and browser captures must bind their own executable
artifacts, source snapshots, runtime identities and unchanged inputs. Each rerun
uses a fresh output directory and preserves failed or incomplete earlier runs.
Running a new provider against an older frozen engine is useful diagnostic
evidence, with that older engine's identity; it does not verify a new runtime
candidate.
