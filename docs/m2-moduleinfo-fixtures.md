# ModuleInfo Java regression fixtures

The pinned Pkl 0.32.0 oracle records 28 authored sources and 224 ordered calls
from two fresh JVMs. Each JVM uses one evaluator across all sources. The fixture
retains every source, original physical URI, expression, raw UTF-16 and Java
UTF-8 channel, Java error class, complete diagnostic, and independent-root
follow-up. The copied inventory and mandatory original bindings are checked
against fixed report, case, inventory, and closure digests.

```sh
python3 tools/moduleinfo_fixtures.py
python3 -m unittest discover -s tools/tests -p test_moduleinfo_fixtures.py
cargo test -p pkl-runtime --test m2_moduleinfo_oracle --locked
```

The shared native consumer compares 156 ordinary calls as complete status,
UTF-16 units, and Java UTF-8 bytes. It covers lazy construction and class access,
Unicode decimal digits, malformed constraints with power assertion diagrams,
wrong types, undefined fields, required module versions, leading zeros, repeated
failures, detached diagnostic values, and terminal heap release.

The remaining calls keep separate boundaries. Twelve non-ASCII module version
annotation calls raise a Java `PklBugException`; the Rust architecture control
requires explicit `Unsupported`, which does not establish Java error/API parity.
The 56 original independent-root follow-ups ran in the same Java evaluator.
`SourceProbe` cannot switch root modules, so these captures remain unexecuted as
a native shared-evaluator API contract. They are never counted as the 156 ordinary
comparisons. Fixture integrity and passing Rust architecture tests do not change
these compatibility outcomes.

The Java regex accepts Unicode decimal digits, including Arabic-Indic and
fullwidth digits. The pinned JDK digit specialization has separate exhaustive
raw evidence; these cases do not establish complete regex compatibility.
`ModuleInfo` construction validates its field lazily, while a module annotation
also checks the required runtime version. A future version string is valid in
an ordinary `ModuleInfo` value and can still reject an annotated module.

The scope excludes general reflection, arbitrary annotations and inheritance,
all standard library modules, JVM-global standard-module cache parity, product
API continuity, other platforms, and performance. These obligations remain open
under the M2 and full release contracts.
