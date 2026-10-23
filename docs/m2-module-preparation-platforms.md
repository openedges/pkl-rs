# Module preparation platform comparisons

The module preparation fixture selects 252 Java API calls in 24 sessions from
300 observations. It covers File triple-dot lookup, module and resource policy
denials, nonlocal/nonhierarchical ModuleKey capabilities, and a lambda diagnostic
owner control. The 48 unselected calls remain explicit obligations. These inputs
do not replace the original program corpus or the whole M2 completion gate.

`tests/fixtures/m2-module-portable` retains the source graph, reader input bytes,
complete Java status/output/error bytes, UTF-16 units, reader counts and callback
events. The offline verifier derives the expected selection from pinned Java
reports and inputs. It binds event order to captured Java process output, checks
both output encodings and opposite empty channels, and rejects changed provider
facts or expectations even if the outer manifest is rewritten. The historical
triple-dot record has no inventory/closure pins; its original null fields remain
unchanged and its report, input and process-output hashes provide the recorded
bindings. This does not retroactively add a seal to that observation.

```sh
python3 tools/module_portable_fixtures.py --fixtures tests/fixtures/m2-module-portable
python3 -W error::ResourceWarning -m unittest discover -s tools/tests -p 'test_module_portable_fixtures.py' -v
node --test tools/platform/resource-session.test.mjs tools/platform/module-preparation-session.test.mjs
```

Module providers are unordered reusable facts derived from the pinned Java
helper, File inputs, sources and policy rules. Java did not directly record
capabilities/has-element SPI request order or return values. The consumer checks
complete request URI context, performs exact table lookup, and retains every
actual request. It neither asserts a Java request count/order nor requires every
provider to be used. It does not implement URI search, caching or evaluation.

Each requested Host operation emits the observable policy events specified by
those input rules. Their per-API projection is compared with original Java events
in order and without text normalization. Primary/query admission and API completion
events outside this supplied-source boundary remain separately recorded with
reasons; unknown event kinds are rejected. This projected contract is distinct
from complete Java SPI/public API equality or physical filesystem I/O in a browser.

A denied resource request occurs before the Java reader callback. The consumer
therefore records that actual Host request but does not increment reader counts
or consume the ordered reader plan. Successful reads use typed Resource payloads.
Module preparation and denied-read suspension are collected before completion;
the existing Resource helper checks owned input/output data, full UTF-16 and Java
UTF-8 channels, completion disposition and heap release after close/free.

After building and recording the actual shared-engine bindings, acquire a fresh
Node and Chromium comparison with the pinned browser tooling:

```sh
node tools/platform/compare-module-preparation.mjs OUTPUT WASM_DIR ADDON tests/fixtures/m2-module-portable tools/platform/package.json
```

`OUTPUT` must not exist. The driver verifies the fixture, records exact tool,
runtime and artifact identities, and preserves raw results and policy traces.
Its artifact arguments require a separate immutable build/source binding. The
recorded browser executable is passed explicitly to Playwright's launch options.
Earlier C05 captures used Playwright's default headless selector while recording
the default Chromium installation path; they do not prove the exact executed
browser binary hash. Their raw comparisons remain preserved with that provenance
qualification, separately from subsequent explicit-executable runs.
`selectedContractPassed` describes this selection only; full compatibility is
always recorded separately as incomplete. A successful C05 exploration remains
evidence for C05, not a later source candidate or complete platform support.
