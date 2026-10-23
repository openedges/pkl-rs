# Java-origin module loading fixtures

These 22 fixtures retain exact Java-derived module graphs, source identities and
complete result payloads. They contain three original CLI output contracts
(`basic/import1`, `import2`, `import3`), ten file-backed API controls, three
percent-path CLI success controls, two ModuleKey SPI controls, and four full
source/logger reentry controls. Fixture integrity is separate from execution or
compatibility success.

Original CLI bytes come from both closed Stage40 Java captures. No CLI input was
reevaluated for this export. The remaining results come from closed Stage41
Evaluator observations. A supplemental capture records complete logger frames
and UTF-16 value/error units for the four reentry controls. A pure pinned Java
ModuleResolver/ModuleKey probe records original, canonical resolved and display
source URIs without evaluating the original CLI programs. Exact origin reports,
selected raw payloads, source snapshots and closed inventory bindings accompany
the data.

The absolute URI strings preserve the actual captured inputs. The portable Host
performs exact bundle lookups; it does not claim browser filesystem access,
symlink resolution or native file security enforcement. Original CLI success
bytes are compared at the output-byte API boundary. Original process status and
files remain provenance; the byte API is not an operating-system CLI process.
API results captured only as Java UTF-8 retain that serialization boundary;
uncaptured Java UTF-16 units are not invented. Fresh captures include exact units
where declared. Failure producer names and Java exception classes are separate
metadata, not translated into compatible phases or Rust exception types.

The fixture set preserves partial initialization errors, cached first-error
frames, source-load failures and callback errors. Finite Host completion plans
model the observed per-attempt outcomes without implementing language semantics
in JavaScript. Request counters are expectations only where the Java SPI actually
observed them. Complete log fields and callback results remain ordered.

The constructor/setup-error, deadline, post-close exception and CLI-error-profile
controls listed in `cases.json` retain explicit pending boundaries. They are not
passes or waivers. These fixtures do not replace the other 27 original30 CLI
contracts, the complete source corpus, or the remaining loader/API obligations.

Run the offline integrity verifier before a fresh native/Node/browser comparison:

```sh
python3 tools/module_loading_fixtures.py
```

All expected payloads originate in Java. The verifier performs no evaluation,
build, normalization, output trimming or compatibility classification.
