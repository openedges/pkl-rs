# Validation Contract

The goals are behavioral and byte compatibility with the Java implementation of Pkl 0.32.0, with execution time and memory usage at least as good as Java for every required workload. Source verification, inventories, a pinned Java oracle, and raw comparison tooling are available. M1 adds bounded Rust/Java architecture comparisons and actual browser/Node execution; complete language and product performance gates remain open.

## Execution environment

Development checkpoints may be verified locally. GitHub Actions automates the
checks and adds independent operating-system coverage; its availability is not a
prerequisite for continuing implementation or accepting a bounded checkpoint.
When local evidence is used, record the host, pinned toolchain and oracle, exact
implementation sources, build artifacts, commands, complete raw outcomes, and
unexecuted environments. Reuse unchanged verified evidence and run the checks
needed for changed behavior or unresolved concerns.

Local macOS execution does not establish Linux or Windows execution. Wasm
compilation remains separate from real browser execution. Hosted artifact-download
checks are applicable only when hosted artifacts exist; locally sealed evidence
must instead have its own integrity and source provenance checks. Preserve prior
CI failures as historical records. This execution choice changes neither raw byte
comparison requirements nor the full language, platform, or performance gates.

## Current Checks

The [Stage86 checkpoint](reports/m2/stage86-contains-hash-power-checkpoint.json) records
all five required Rust checks, including a complete 1,149-pass workspace run
with one ignored test and zero failures. Native, N-API, and Chromium each
matched 104 repeated observations from 35 new queries and 17 distinctness
regressions. One M1 acquisition matched 6,078 results per target and passed
its separate architecture contract. A prior capture used the wrong power
assertion setting and remains failed; its successor uses the exact Java
settings. Full-corpus replay completion and verification remain pending.

The [Stage85 checkpoint](reports/m2/stage85-distinct-checkpoint.json) records the
five required Rust checks, including a completed 1,136-pass workspace run with
one ignored test and zero failures. The earlier capacity-stopped attempt remains
separate. Selected `distinct` regressions compare four unchanged originals and
17 Java queries, with repeated native, N-API, and Chromium captures. One fresh M1
acquisition supplies distinct strict-byte and architecture-contract judgments;
it is not reported as two executions. These results do not close the full M2,
product API, release-platform, or speed/memory requirements.

```sh
cargo xtask verify
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo check -p pkl-runtime --target wasm32-unknown-unknown --locked
```

The first command checks reference pins. The remaining commands check Rust formatting, run static analysis and the current workspace tests, and verify Wasm compilation. Successful Wasm compilation does not establish browser evaluation or API compatibility. Do not use the number of passing `cargo test` cases as a measure of Pkl feature completion.

Python verification tools use the standard library; use Python 3.12 or newer. CI selects Python 3.12. The [oracle contract](docs/oracle.md) describes the exact JDK requirement, artifact and source pins, execution environment, and evidence. With that JDK installed:

```sh
python3 tools/oracle.py fetch
python3 tools/oracle.py verify
python3 tools/oracle.py smoke
python3 -m unittest discover -s tools/tests -p 'test_oracle.py'
```

Oracle smoke checks capture Java success and error bytes and verify exact Java replay. They do not execute a Rust evaluator. The separately pinned optimizing GraalVM profile is used for performance baselines.

The [source inventory](docs/inventory.md) tracks language, stdlib, builtin, renderer, CLI, protocol, and JVM source declarations. Regeneration detects missing entries and source drift:

```sh
python3 tools/inventory.py --check
python3 -m unittest discover -s tools/tests -p 'test_inventory.py'
```

Source declaration counts and builtin registration matches are not semantic coverage or proof of compiled JVM API compatibility. Implementation and behavioral test mappings remain explicit unimplemented/unmapped records.

The [corpus contract](docs/corpus.md) accounts for reference files, resources, runnable command cases, and suites requiring additional adapters:

```sh
python3 tools/discover.py --check
python3 -m unittest discover -s tools/tests -p 'test_discover.py'
```

Ready commands have not necessarily run. Blocked suites and cases with an outcome requiring observation remain visible; file counts are not assertion counts. Historical rev1 inputs are inventoried separately. [Three selected regressions](docs/regressions.md) retain their original input/license bytes and have fresh Java 0.32.0 outcomes; the remaining historical candidates stay unselected.

```sh
python3 tools/regression_manifest.py --check
python3 -m unittest discover -s tools/tests -p 'test_regressions.py'
```

The [execution tool](docs/compatibility-tooling.md) records and checks a required seven-case smoke corpus, including error bytes and multiple generated files:

```sh
python3 tools/smoke_manifest.py --check
python3 -m unittest discover -s tools/tests -p 'test_compat.py'
python3 tools/compat.py record --manifest compatibility/smoke.json --output target/m0/smoke-baseline
python3 tools/compat.py check --manifest compatibility/smoke.json --output target/m0/smoke-baseline
```

Use a new evidence directory for each run. Candidate comparison requires a complete Java baseline and the same workspace path; the tool verifies source/resource hashes, runner provenance, raw captures, and case coverage. Java-to-Java replay validates tooling only. The process adapter currently requires POSIX process groups. The full discovered corpus includes blocked cases and therefore fails the strict completion gate until all required adapters and execution conditions exist; the smoke corpus is not a substitute for full compatibility.

Reviewable evidence archives under `reports/m0/` retain raw outputs and a per-file checksum inventory. Verify an archive with `python3 tools/evidence.py verify <archive.tar.gz>`. Copied source workspaces can be excluded explicitly; the archive records its exclusions and retains the pinned source manifest.

Pantry has a separate [development and packaged-module contract](docs/corpus.md):

```sh
python3 -m unittest discover -s tools/tests -p 'test_pantry.py'
python3 tools/pantry.py --workers 4
python3 tools/evidence.py verify reports/m0/pantry-evidence.tar.gz
```

The runner requires a new `target/m0/pantry` evidence directory. It runs package/API tests, evaluates generated ZIP modules, checks configured generators and the tar pipeline, and records dependency payload hashes. Abstract or user-configured module errors remain provisional observations. Reported test/assertion invocation totals can repeat dependency suites; they are not distinct coverage counts. Unfixed live GitHub discovery inputs remain outside the hermetic baseline.

Run all verification-tool regressions with `python3 -m unittest discover -s tools/tests -v`. The [M0 report](reports/m0/README.md) links recorded outcomes and raw evidence. The [CI contract](docs/ci.md) distinguishes the required tooling smoke run from optional full-corpus acquisition and performance measurements.

## Compatibility Comparisons

The [Class-value contract](docs/m2-class-values.md) separates declaration identity,
instance prototypes, superclass links, and representation-based type/converter
rules. Its independent Java query/logger observations, ModuleKey SPI observations,
and exported-Class identity observations are distinct verification boundaries.
The common native/portable matrix cannot close the required provider/export
supplements. Run fixture integrity, strict query comparison, and full API checks
separately; retain unsupported values and callback/oracle failures.

The [object-body parameter contract](docs/m2-object-parameters.md) keeps Java
observations for Mixins, function amendments, and lexical captures independently
pinned. Native and portable comparisons use the original arbitrary expressions
and resource bytes through the public shared evaluator. Unsupported outcomes and
Java internal errors remain separate from matching ordinary values and language
errors. Lifecycle regressions cover suspended parameter checks, escaped curried
closures, reentry, retry, cancellation, and terminal code/heap release.

The [canonical diagnostic contract](docs/m2-diagnostics.md) keeps actual Java
UTF-16 strings separate from their Java UTF-8 serialization. Native and portable
checks preserve both channels; historical byte-only fixtures do not establish
string-unit equality. SourceSession, scalar helpers, frontend constructors, and
N-API error callbacks have distinct recorded boundaries.

The [portable Resource reference fixtures](docs/m2-resource-portable-fixtures.md)
retain 1,008 selected Java API observations and their original bytes, UTF-16
units, ordered reader inputs and counters. Their offline verifier and tamper
tests establish fixture integrity; they do not execute Rust, Node or a browser:

```sh
python3 tools/resource_fixtures.py --fixtures tests/fixtures/m2-resource-portable
python3 -W error::ResourceWarning -m unittest discover -s tools/tests -p 'test_resource_portable_fixtures.py' -v
```

The [renderer reference fixtures](docs/m2-renderer-oracle.md) preserve 860 Java
API calls in 44 persistent sessions. Their offline verifier authenticates the
original inputs, raw UTF-16/Java UTF-8 channels and reader plans before accepting
the exact fixture file set. The regression suite exercises re-signed fixture
forgeries and rejection of undeclared directories before traversal:

```sh
python3 tools/renderer_fixtures.py --fixtures tests/fixtures/m2-renderer-portable
python3 -W error::ResourceWarning -m unittest discover -s tools/tests -p 'test_renderer_fixtures.py' -v
```

The renderer platform driver requires separately recorded binding artifacts.
Offline fixture success does not establish runtime or platform compatibility.

The compact frame-identity fixture preserves three unchanged Java programs,
12 evaluator sessions and 240 complete API observations. It covers ordinary and
typed-union generator frame restoration and numeric constants in distinct tagged
instances. Its verifier authenticates the original Java records and both TSV
indexes before accepting their exact file set:

```sh
python3 tools/renderer_identity_fixtures.py --fixtures tests/fixtures/m2-renderer-frame-identity
python3 -m unittest discover -s tools/tests -p 'test_renderer_identity_fixtures.py' -v
cargo test -p pkl-runtime --locked --test m2_renderer_frame_identity_oracle
```

The Rust regression consumes the same 240 observations with Observe disabled and
enabled, comparing status, complete UTF-16 channels and Java UTF-8 bytes. The
actual Java acquisition used power assertions enabled only. These are two Rust
configurations, not two Java baselines. Every unexpected Host effect fails the
test; collection runs after each poll and closure must release all slots. The
fixture is a subset of separately acquired renderer observations and does not
add independent whole-language coverage. Policy profiles, explicit-close API
parity, complete producer identity and equivalent GC event traces remain separate.

The [module preparation platform contract](docs/m2-module-preparation-platforms.md)
adds exact provider-input lookup and ordered Java-observable policy comparisons.
Its 252 selected calls distinguish resource policy denial from a counted reader
callback and retain the 48 unselected API boundaries. Run its offline checks with:

```sh
python3 tools/module_portable_fixtures.py --fixtures tests/fixtures/m2-module-portable
python3 -W error::ResourceWarning -m unittest discover -s tools/tests -p 'test_module_portable_fixtures.py' -v
node --test tools/platform/resource-session.test.mjs tools/platform/module-preparation-session.test.mjs
```

Actual Node/Chromium results require the separately documented driver and frozen
binding artifacts. New browser acquisitions explicitly launch the executable
whose identity is recorded. Historical default-headless captures do not establish
that their recorded Chromium installation path was the executed binary.

The [module admission policy fixtures](docs/m2-module-admission.md) preserve
84 original Java API calls and 280 ordered events, including lifecycle events
that the current supplied-primary Rust session does not reproduce:

```sh
python3 tools/module_admission_policy_fixtures.py --fixtures tests/fixtures/m2-module-admission-policy
python3 -m unittest discover -s tools/tests -p 'test_module_admission_policy_fixtures.py' -v
```

Fixture integrity, raw API equality and full policy event/counter equality are
separate checks. Missing lifecycle events remain failed comparisons.

The [ModuleKey reference fixtures](docs/m2-module-keys.md) separately preserve
212 Java API calls, 36 sessions and 2,248 ordered events. Their key/cache and
factory-failure cohorts retain original inputs, raw output/error channels,
counter bytes and exception metadata:

```sh
python3 tools/modulekey_fixtures.py --fixtures tests/fixtures/m2-modulekey-spi
python3 -W error::ResourceWarning -m unittest discover -s tools/tests -p 'test_modulekey_fixtures.py' -v
```

Offline fixture integrity does not establish native or portable execution.
Supplied-primary and public-entrypoint experiments must retain distinct results;
raw API agreement cannot replace full ordered lifecycle observations.

The [extended ModuleKey contract](docs/m2-modulekey-extended.md) separately
preserves 944 API calls and 10,124 ordered Java events across URI, contents and
REPL sources, changing factory/source/policy state, URI overrides and fragment
capabilities. Its fixture verifier and transport regressions are:

```sh
python3 tools/modulekey_extended_fixtures.py --fixtures tests/fixtures/m2-modulekey-extended
python3 -W error::ResourceWarning -m unittest discover -s tools/tests -p 'test_modulekey_extended_fixtures.py' -v
node --test tools/platform/modulekey-extended-session.test.mjs tools/platform/compare-modulekey-extended.test.mjs
```

Keep ordinary compatibility, diagnostic-only matches, Java internal-error
boundaries, raw equality, acquisition/cleanup integrity and the full Java
event/counter gate separate. A completed capture with preserved mismatches is
not a strict compatibility pass. An older frozen engine used to debug a new
provider does not verify a subsequent Rust runtime candidate.

The [collection transform contract](docs/m2-collection-transforms.md) records
Java-authenticated callback, conversion, identity, and quantity controls through
the native and actual Node/Chromium evaluators. Ordinary operations, inputs that
missed their intended feature, and post-close API boundaries remain separate.
Raw failures and full Java lifecycle gaps are retained even when acquisition
integrity passes.

M2 language implementation and verification are in progress. The
[M2 contract](docs/m2-language.md) separates whole-language requirement accounting
from the executable selection:

```sh
python3 tools/m2_manifest.py --check
python3 tools/m2_language.py --output target/m2/language-new --record-only
python3 tools/m2_language.py --verify-report target/m2/language-new
python3 tools/m2_language.py --verify-development target/m2/language-new
python3 tools/m2_language.py --verify-subset target/m2/language-new
python3 tools/m2_language.py --verify-full target/m2/language-new
```

The full gate must fail while required language implementations, mappings,
adapters, or passing executions are missing. It cannot be replaced with the
selected-case gate.

The [Stage69–72 Java authority checkpoint](reports/m2/stage69-72-java-checkpoint.json) separately preserves 235 authored queries for super entry defaults, import/generic type lookup, VarArgs, base aliases/Mixin, and const owner behavior. Its six archives retain complete raw repetitions, logger channels, earlier failed attempts, and source/JDK/JAR identities. The [publication process record](reports/m2/stage69-72-java-publication-execution.json) contains actual commands, exits, and raw channels; Java publication is not a Rust compatibility pass. The separate [implementation checkpoint](reports/m2/stage69-72-implementation-checkpoint.json) records all five required Rust checks, 235+4 selected native/N-API/Chromium query comparisons, the full 4,416-case replay, and a regression comparison against Stage68. The full 4,416-case Rust replay records **4,158 matches, 115 mismatches, 141 unsupported results, 2 Java oracle errors, and 0 Rust internal errors**. Original modules account for 638/888 matches; expression strings account for 3,519/3,527, and the JVM classpath case matches. Compared with the latest Stage68 Rust baseline, 6 previously nonpassing cases now match and no previously passing case regresses. The strict full gate and the two known M1 Regex failures remain failed; the 111 unready inventory cases remain unready. Java Logger authority is absent for 91 queries in the main selected matrix.

Stage68 adds independent Java authorities for String Base64, collection emptiness,
and `every`. Verify their exact archives, inputs, raw repetitions, and provenance:

```sh
python3 -B tools/string_base64_fixtures.py
python3 -B tools/collection_emptiness_fixtures.py
python3 -B tools/collection_every_fixtures.py
```

These commands use tracked fixture roots and work without historical `target/`
directories. The authorities contain 89, 47, and 64 expressions respectively;
`every` also retains ordered logger events and their UTF-16 units. They prove
Java reference integrity, not Rust compatibility. The separate
[Stage68 implementation checkpoint](reports/m2/stage68-implementation-checkpoint.json)
records all five required Rust checks and 2,726 exact catalog observations per
native/N-API/Chromium target, plus 128 logged `every` observations per target.
The full ready replay has 4,152 matches, 119 mismatches, 143 unsupported results, 2 Java oracle errors, 0 Rust internal errors;
its 1,799 overlapping unmet full-gate entries remain failures. It reuses the
sealed Stage67 Java evidence and executes Rust anew, with no fresh Java process.
Replay integrity and strict compatibility remain separate verdicts. The 111
unready cases and Any/M1 Regex failures are preserved. The Stage67 results below
describe the earlier frozen candidate.

Stage67 adds the String property authority (`python3 tools/string_properties_fixtures.py`)
and a fifteenth collection catalog profile. The Stage67 catalog has 1,227
expressions and requires two complete raw observations per expression on each
target. Its selected comparisons are separate from the full ready corpus.
The [Stage67 checkpoint](reports/m2/stage67-implementation-checkpoint.json)
records 4,416 ready comparisons: 4,142 matches, 128 mismatches, 143 unsupported
results, two Java oracle errors, and one Rust internal error. Strict full
verification exits 1 with 1,802 unmet gate entries. Preserve the 111 unready
cases, inherited scalar regressions, and Any/M1 Regex failures. A successful
record-only acquisition verifies evidence integrity, not M2 completion.

Stage66 reversal and constructor authorities are four distinct single-module
string fixtures; union-marker diagnostics use forty separate modules:

```sh
python3 tools/reversal_values_fixtures.py
python3 tools/reversal_implicit_fixtures.py
python3 tools/reversal_subscript_fixtures.py
python3 tools/base_constructor_constraints_fixtures.py
python3 tools/union_marker_fixtures.py
```

Fixture integrity is separate from candidate compatibility. The Stage66 collection
catalog snapshot contains fourteen profiles and 1,081 expressions; native and portable
comparators require two full raw repetitions per profile. The separate
`tools/union_marker_compare.py` and
`tools/platform/capture-union-marker-modules.mjs` run each exact diagnostic module
in fresh native or actual N-API/Chromium sessions. Their string gates compare all
seven common channels and whole process output; Java exception metadata and full
API requirements remain separate.

The recorded Stage66 union portable invocation used a Node symlink while
`process.execPath` recorded its resolved binary. The original failed verification
and unchanged capture remain preserved. The versioned adapter under
`target/m2/stage66-node-path-verification-amendment-02` verifies both paths and
recorded before/after executable hashes, then forwards a temporary locator
metadata copy to the original source-pinned verifier. It changes no captured
file or guest byte. Restore its published closeout evidence to replay that
historical invocation; its strict string gate passes and full-API gate fails.
New capture wrappers should record the resolved executable directly. The
[implementation evidence](reports/m2/stage66-implementation-checkpoint.json)
retains this admission boundary, the earlier failure, known Regex differences,
and the separate 131-program original results.

The inferred-construction authorities retain five separate observation scopes:
72 ordinary expressions, seven real amending-module queries, two opposite
eleven-query module-receiver orders, 44 method-cache queries, and four separate
default-diagnostic modules. Verify their sealed Java captures with:

```sh
python3 tools/inferred_new_fixtures.py --profile values
python3 tools/inferred_new_fixtures.py --profile amends
python3 tools/inferred_new_module_receiver_fixtures.py
python3 tools/inferred_new_method_cache_fixtures.py
python3 tools/inferred_new_default_diagnostics_fixtures.py
```

Only the ordinary 72-expression profile belongs to the single-module comparison
catalog. Preserve sibling modules, receiver order, evaluator lifetime, and Host
requests in the other scopes. The method-cache archive also retains an earlier
object-method alias experiment with eight Java internal errors per repetition;
those observations remain oracle failures. Fixture integrity does not establish
Rust compatibility. Candidate comparisons must retain complete raw channels,
and native multi-module regressions do not replace the corresponding portable
execution requirements.

Lifecycle tests that measure user-object heap deltas use the test-only
[constructor baseline](crates/pkl-runtime/tests/support/constructor_baseline.rs).
It initializes
the three canonical collection parents through compiled constructor sites
before taking the baseline, without forcing authored members. Production
initialization and compatibility captures remain lazy. Separate VM tests cover
first-use parent caching, cache identity, code ownership, and terminal cleanup;
the lifecycle tests retain exact temporary-object and executable-node deltas.

Stage 64's independent amendment authorities have separate offline verifiers:

```sh
python3 tools/amendment_construction_fixtures.py
python3 tools/amendment_declarations_fixtures.py
python3 tools/prototype_diagnostics_fixtures.py
python3 -m unittest tools.tests.test_amendment_construction_fixtures tools.tests.test_amendment_declarations_fixtures
python3 -m unittest tools.tests.test_prototype_diagnostics_fixtures
```

The construction fixture contains 188 ordered public string queries and is
registered in the shared comparison catalog. The declaration fixture contains
eleven distinct modules and remains outside that single-module profile. Direct
VM identity, Host logs, and the additional local-amendment/EOF controls in the
[Java checkpoint](reports/m2/stage64-java-checkpoint.json) retain their own
observation boundaries. Fixture verification establishes the Java authority;
candidate process, native, Node, browser, and full M2 checks remain required.

The separate [prototype diagnostic fixture](tests/fixtures/m2-prototype-diagnostics/README.md)
contains 39 queries in two fresh Java sessions. It retains complete operator,
missing-method and class-name observations for builtin and user prototypes,
including unforced member displays. It does not change the construction or
declaration authorities and is not registered in the shared comparison catalog.

The [DataSize contract](docs/m2-data-size.md) defines two complete ordered string
sessions per target against the independent split Java fixture. Verify native
capture integrity with `tools/data_size_compare.py --verify-report`, strict
strings with `--verify-string`, and the separate required API gate with
`--verify-full`. Each option takes an existing capture directory. A recorded
helper build is required for a fresh `--output ... --build-record ...` acquisition.

The portable verifier takes `--capture`, `--m1`, `--build-wrapper`, and
`--capture-wrapper` evidence paths,
with `--gate integrity|string|full-api`. It binds a frozen M1 build separately
from any explicitly recorded capture-only transport change. Browser page protocol
and console errors remain distinct from operating-system process channels. Both
native and portable full API gates remain unsuccessful while typed exports are
unexecuted. Original upstream programs retain their own complete CLI contract.

The [Any.ifNonNull contract](docs/m2-any-if-non-null.md) uses separately sealed
86-row and 13-row Java authorities. Its native and portable verifiers compare
exact ordered expression-string channels; `--gate integrity` checks complete
source-bound evidence, while `--gate string` requires every selected row in both
repetitions. The portable verifier takes `--capture` and `--m1`; both verifiers
retain the unsupported Regex and IntSeq constructor rows as failures. The
required typed export and full M2 gates remain open. The Stage 58 recorded
comparisons have 168/172 matches per native, N-API, and Chromium target; the
DataSize regression retains 510/510 on each target. Of 17 unchanged original
upstream programs, ten pass complete raw comparison and seven mismatch,
including Bytes, Duration, and Pkl Binary cases.

The Stage 59 `Duration` contract is recorded in
[the M2 language guide](docs/m2-language.md). Verify the separately sealed
274-row Java string authority and 15-row exported-value bit supplement before
candidate comparison:

```sh
python3 tools/duration_values_fixtures.py
python3 tools/duration_values_bits_fixtures.py
python3 -m unittest tools.tests.test_duration_values_fixtures tools.tests.test_duration_values_bits_fixtures
cargo test -p pkl-runtime --test m2_duration --locked
```

The Rust test's raw-bit assertions are internal value checks against actual
Java exported `Duration` observations; they do not establish typed product
API parity. Use fresh directories and an existing recorded native build for
the two complete ordered expression-string sessions:

```sh
python3 tools/duration_values_compare.py --acquire target/m2/duration-native-new --build target/m2/duration-build-record
python3 tools/duration_values_compare.py --verify target/m2/duration-native-new --gate string
node tools/platform/capture-duration-values.mjs target/m2/duration-portable-new WASM_DIR NAPI_BINARY tests/fixtures/m2-duration-values
python3 tools/duration_values_portable_compare.py --capture target/m2/duration-portable-new --m1 M1_PLATFORM_RECORD --native-build target/m2/duration-build-record --build-wrapper M1_BUILD_WRAPPER --gate string
```

The portable verifier requires both `--native-build` and `--build-wrapper` to
bind the supplied N-API and Wasm artifacts to recorded sources. The native
and portable `--gate integrity` options check evidence independently of exact
string equality. The portable `--gate full-api` remains unsuccessful while
typed exports are unexecuted; the native runner has no full-API gate. The
string gate compares the entire status, expression,
stdout, stderr, UTF-16BE, and process-protocol bytes across two repetitions.
Original upstream Duration snippets retain their separate CLI and generated
file comparison contract. The [Java-only checkpoint](reports/m2/stage59-java-checkpoint.json)
does not claim native, Node, or browser compatibility.
The later recorded Stage 59 local comparisons match all 548 Duration string
executions on each of native, N-API, and Chromium, including complete process
protocols. The unchanged DataSize string regression matches 510 on each
target. Portable `Any.ifNonNull` still has four Regex/IntSeq constructor
mismatches per target. Of 17 separately executed original modules, 11 match,
five mismatch, and one is candidate-unsupported; `basic/duration.pkl` newly
matches while `api/duration.pkl` reaches an unimplemented `pkl:math` use.
These selected results and internal raw-bit assertions do not satisfy the
typed export, whole-language, product API, release-platform, or performance
gates. The [M2 evidence report](reports/m2/README.md) tracks the distinct
recorded scopes.

The Stage 60 Bytes fixture contains 300 independent Java expressions in two
fresh JVM sessions. Its 17 producer channels include exception and cause
metadata; the current candidate string boundary compares seven common raw
channels and the complete ordered protocol. Do not count producer-only metadata
as candidate API passes. At Stage 60, the strict native and portable comparisons
retained six IntSeq mismatches per target, with 594 matches. Its ignored Rust
test preserved the three underlying cases without counting them as passes.

```sh
python3 tools/bytes_values_fixtures.py
python3 tools/bytes_values_compare.py --acquire target/m2/bytes-native-new --build target/m2/bytes-build-new --gate string
python3 tools/bytes_values_compare.py --verify target/m2/bytes-native-new --gate string
python3 tools/bytes_values_portable_compare.py --capture target/m2/bytes-portable-new --m1 target/m2/bytes-platforms-new --native-build target/m2/bytes-build-new --build-wrapper target/m2/bytes-platform-wrapper-new --gate string
```

Acquisition requires an existing recorded build; portable verification also
requires the matching actual N-API/Chromium capture, M1 artifact set, and source
wrapper. The [Stage 60 evidence](reports/m2/stage60-implementation-checkpoint.json)
preserves those exact inputs and commands. Strict comparisons exit nonzero for
the retained failures even when acquisition and evidence integrity pass.
The Java session flags establish effective `en-US` and `UTC`, independently
of the preflight locale. Portable capture checks the recorded requested process
environment and actual browser observations without claiming a complete
OS-injected Java environment observation.

Stage 60 passed the five required local Rust checks: 940 test executions passed
and four were ignored. Its initial unchanged 256 KiB frontend stack test failed;
the compiler fix and subsequent pass retain the original stack limit and input.
The later disk-full evidence-write failure is recorded separately. Tool checks
pass 25 Python tests normally and under `-O`, plus 19 JavaScript tests.
Duration and DataSize string regressions match 548 and 510 observations on each
target. Portable Any and the M1 regex experiment retain their known failures.
Original modules have 12 matches, four mismatches, and one unsupported result
among 17; each program has one Java and one Rust execution. Offline verification
does not add evaluation repetitions. Typed exports, mutable Java array aliasing,
whole-language and product compatibility, and performance remain unverified.

The [Stage 61 IntSeq checkpoint](reports/m2/stage61-implementation-checkpoint.json)
uses 237 ordered string expressions in each of two fresh Java sessions, with
198 successes and 39 ordinary errors per session. Three typed-export rejection
rows are captured separately and are not candidate string comparisons.
The shared native probe accepts the same three-column `string`, ID, expression
input as Java, while preserving the existing two-column probe format. It rejects
other modes, extra columns, and duplicate IDs. The comparator checks all seven
common raw channels and the full ordered protocol, including failed expressions.

```sh
python3 tools/int_seq_values_fixtures.py
python3 tools/int_seq_values_compare.py --acquire target/m2/intseq-native-new --build target/m2/intseq-build-new --gate string
python3 tools/int_seq_values_compare.py --verify target/m2/intseq-native-new --gate string
python3 tools/int_seq_values_portable_compare.py --capture target/m2/intseq-portable-new --m1 target/m2/intseq-platforms-new --native-build target/m2/intseq-build-new --build-wrapper target/m2/intseq-platform-wrapper-new --gate string
```

As with Bytes, acquisition and portable verification require the recorded build,
actual platform capture, M1 artifacts, and source wrapper. All 474 IntSeq
observations match on each target. The unchanged Bytes regression now matches
600 of 600, and its formerly ignored IntSeq-dependent cases execute and pass.
Duration and DataSize retain 548 and 510 matches per target. Portable Any retains
170 matches and two Regex differences per target; its strict gate still fails.
All five required Rust checks pass: 953 test executions pass and two remain
ignored. Tool checks pass 37 Python tests normally and under `-O`, plus 13
JavaScript tests. Native prerequisite regressions cover 17 math strings and
numeric bit values, 21 List.add cases, and 65 List accessor/fold cases; they do
not establish typed export or separate portable supplement compatibility.

The expanded 62-program original comparison has 26 matches, 15 mismatches,
and 21 unsupported results. Its previous 17-program subset has 15 matches and
two mismatches, with no passing regression. These are one execution per runtime
per original, separate from repeated authored string sessions. Source, artifact,
oracle, and workspace integrity pass while selected/full compatibility fails.
Full M2 completion, external math functions, typed APIs, release-platform
coverage, and performance targets remain separate outstanding requirements.

The [method-deprecation contract](docs/m2-deprecation.md) records the pinned Java
logger matrix and a separate experiment with sequential evaluators in one JVM.
Its independent fixture verifier checks raw UTF-8, UTF-16, frame, and callback
observations. Java internal failures and API exceptions remain nonpasses;
evidence integrity does not establish candidate compatibility. Recorded candidate
comparisons retain Java internal-error nonpasses and the separate, unexecuted
cross-evaluator lifetime obligations.

The [frame-transformation contract](docs/m2-frame-transformers.md) compares
complete original and replacement frames, nullable members, arbitrary UTF-16,
public result/error payloads, and ordered callback protocol on native, N-API,
and Chromium. `tools/frame_transformer_compare.py --verify-report` checks the
captured evidence; `--verify-full` is the strict requested main matrix. It does
not execute the Java-only test-buffer supplement or replace the whole M2 gate.
The [Stage55 checkpoint](reports/m2/stage55-implementation-checkpoint.json)
retains all 42 exception-related main nonpasses, the initial frame-location
failure, and the recorded default trace/deprecation regressions. Required Rust
checks combine four original passes with an unchanged-input Wasm retry after
disk exhaustion; the initial failure remains part of the evidence.

The [standard-module and guest-catch contract](docs/m2-stdlib-bootstrap.md)
records Java-origin catch, ModuleInfo, and typed security-denial fixtures, native
consumers, and actual Node/browser comparisons. It distinguishes full ordered
API calls from constructor-only, terminal partial, internal-error, and
unexecuted independent-root boundaries. Broader bootstrap comparisons require
observable resource-read counts as well as complete result channels; shared
Rust-target agreement does not erase Java failures.

The [original module contract](docs/m2-module-frontend.md) describes the
additional `language-module-eval` adapter, included in default language
acquisition. It preserves original upstream argv, environment, resources, and
physical paths, comparing complete native process and generated-file channels.
Focused adapter selection leaves all omitted cases required by the full gate.
Node and browser byte vectors are checked separately from native process
execution. The native probe evaluates actual module output. Portable output
comparisons explicitly select `output-bytes`; the retained frontend-only API
continues to report successful preparation as unsupported output evaluation.
An original case's unmet execution prerequisites or expected exit status cannot
be satisfied by raw agreement with a setup-dependent Java failure.

The [JVM execution-profile integration](docs/m2-jvm-classpath.md) explicitly maps
the original `import1b` ID to its JVM/classpath contract while preserving the
historical blocked CLI record. The parent records the native build and binds its
frozen bytes, sources, and oracle to two child repetitions. Those repetitions
count as one original case. Focused adapter success leaves every omitted case
required by the full gate. Both platform and M2 source collectors explicitly
capture the compile-time standard-module inventory; historical captures that
omitted it retain their separate provenance qualification.

The [shared module-loading contract](docs/m2-module-loading.md) adds Java-origin
module graph fixtures and typed Host continuation checks. Verify fixture
provenance separately from actual evaluator execution:

```sh
python3 tools/module_loading_fixtures.py
cargo test -p pkl-runtime --test m2_module_loading --test m2_module_transport --test m2_module_diagnostics --test m2_session_expressions --locked
```

These checks do not replace original native process comparisons or actual
Node/browser execution. Remaining module inheritance, URI and external-capability
requirements stay in the full language gate.

Class and type-alias initialization have a separate persistent-evaluator contract:

```sh
python3 tools/m2_class_lifecycle.py --output target/m2/class-lifecycle-new --record-only
python3 tools/m2_class_lifecycle.py --verify-report target/m2/class-lifecycle-new
python3 tools/m2_class_lifecycle.py --verify-contract target/m2/class-lifecycle-new
```

New captures use report schema 2. Native source snapshots must include the root
`.cargo/config.toml`, Cargo manifests and lockfile, and the pinned Rust toolchain;
missing build controls fail acquisition. Schema 1 records omitted the root Cargo
configuration. Verify those immutable records with their captured
`sources/tools/m2_class_lifecycle.py`; do not upgrade their schema or describe a
separate configuration binding as a directly captured input. The current verifier
rejects schema 1 explicitly. Java-only captures do not require Rust build inputs.

Each sequence retains one module and evaluator across ordered calls, including
errors, and repeats with a fresh evaluator. Every selected call must match its
raw Java status, stdout, and stderr without exceptions. These sequences have
separate execution/call counts and do not close the full language gate. Native
GC and Host lifecycle tests separately check class and alias roots, temporary reclamation,
ordinary lazy retries, and terminal release. See the
[sequence contract](docs/m2-language.md#persistent-class-initialization-sequences)
for source/build provenance, adapters, and remaining scope.

Type-resolution regressions compare authored child order, base-name failures,
class versus alias arity checks, generic parameter shadowing, nested substitution,
and owner signature initialization. Repeated calls preserve failed declaration
state while ordinary lazy expressions retry. Native Host/GC tests separately
exercise nested generic constraints, suspended values, heap isolation, temporary
reclamation, nonterminal cancellation, and terminal cleanup. Node and browser
sequence scenarios retain the same Java-derived complete payloads.

Recursive constraint regressions distinguish unused declarations, short-circuiting,
finite direct/mutual recursion, lexical type scope, power assertions, and known
shared-receiver or generic gaps. Separate Host tests force collection during every
yield, nested reentry and failure retry, stale completion rejection, nonterminal
cancellation, and complete terminal release. `Class<T>` cases verify erased
arguments without publishing broken declarations, while preserving alias argument
resolution and wrong-arity errors.

Finite generic recursion additionally compares changing `List<T>` arguments,
mutual recursion, parameter shadowing, first failures, and inline/lambda versus
shared let targets. Persistent sequences use different actual arguments within
one evaluator. Native tests carry these argument environments through real Host
suspension, child reentry, failure retry, GC at each yield, cancellation, and
terminal release. The original infinitely expanding generic predicate retains
its historical native-stack abort evidence. Shared metadata regressions exercise
20,000-level graphs on 256 KiB stacks, concurrent last-owner release, deep
Reference metadata, and cancellation while collection defaults descend and
unwind. Separate process-isolated evaluator tests retain the original infinite
source and verify extended bounded execution, GC, cancellation, and teardown.
The unchanged development adapter's instruction budget is not a Java-compatible
stack-overflow diagnostic. These checks do not establish uniform wall-time
cancellation or peak-memory parity. Let/for const cases distinguish body scope
from initializers, annotations, eager generator inputs, lambdas, and sibling
members. Generic collection defaults retain original Java and pre-change Rust
controls. Reference metadata checks separately cover constrained actual arguments,
validation before a throwing value, deferred branches, shared let roots, and
persistent retries; existing inline diagnostic gaps remain failed observations.

Nullable-default regressions compare attached class and collection defaults,
empty versus nonempty amendments, nested nulls, and null-propagating operations.
Equality, structural keys, coalescing, type checks, and diagnostics must retain
ordinary null behavior. Native tests trace attached defaults through captured
closures, Host failure and retry, cancellation, evaluator isolation, and terminal
release. These checks do not establish bounded wall-time cancellation or full
generic-constructor compatibility.

Constraint receiver tests distinguish same-call mutation from separate function,
member, let-root, and Host reentry frames. Lambda receiver capture is checked at
creation, with GC between instructions and explicit cycle reclamation after
cancellation. Boolean and function-predicate failure values are compared with
power assertions off and on. Alias const tests separate first-read diagnostics
from unresolved persistent local-property resolution caches; retained retry
witnesses must not be counted as passing first-read cases.

Collection regressions cover lazy bodies, eager keys and source forcing,
inheritance order, primitive key equality, generator captures, spread, and
complete subscript diagnostics. Native lifecycle tests also force collection
construction through nested Host suspension, GC, retry, cancellation, and
timeouts. Shared Node/browser scenarios exercise the same collection engine and
Java-derived error bytes. Generic collection regressions additionally cover
lazy `Listing<T>` values, eager `Mapping<K,V>` keys, strict `List<T>` members,
nested wrappers, casts, unions, aliases, defaults, cache reuse, and first errors.
Lifecycle checks retain temporary and successful wrapped values across GC,
Host suspension, reentry, failure retry, and terminal cleanup. Complete custom
defaults, format-time wrapper checks, and the full collection corpus remain
unverified.

Member predicate regressions compare parent-only selection, interleaved forcing
and condition evaluation, declaration-owner caches, replacement versus amendment,
lexical and custom `this` scopes, nested generators, duplicates, const checks,
and complete diagnostics. Dedicated lifecycle tests retain the parent cursor,
prior custom receiver, and unpublished target across Host suspension, nested
evaluation, GC, retry, cancellation, close, and timeouts.

Structural equality regressions distinguish scalar numeric comparisons from
boxed nested values and keys, including NaN and signed zero. They cover object
kind and class identity, empty-body identity, deep forcing order, inherited
overrides, wrapper checks, and strict List shortcuts. Key cases include lazy
small maps, the fifth-member hash transition, ancestor lookup, Mapping length,
and complete duplicate/missing-key diagnostics. Constant List keys are checked
before evaluation only when every key in a specialized body is constant;
dynamic keys, generators, and shadowed constructors retain runtime behavior.
Signed literal token spans are preserved separately from expression spans.
Source-line rendering retains the pinned highlighter's interpolation whitespace
behavior and whole-line fallback on lexical errors; underlines still use
authored UTF-16 positions.
Lifecycle tests exercise temporary operands and keys through Host suspension,
GC, reentry, retry, and terminal cleanup. Cyclic keys and complete Java
hash-bucket/cache behavior remain outside the verified selection.

Additional comparison regressions cover ordinary typed `default` properties,
inherited and hidden properties, completed regular-member cache order, and
class declaration order independently of amendment order. Key preparation
cases distinguish whole-object deep forcing from subsequent recursive member
hash preparation. Host lifecycle tests cross this phase boundary and preserve
completed values while retrying failed reads. Actual Node and Chromium scenarios
compare values and complete diagnostics captured from the pinned Java oracle at
the identical module URI.

Map regressions distinguish structural equality from numeric hashes and bucket
membership. Pinned JAR observations supply collision order, signed hash mixing,
four-to-five entry transitions, load growth, capacity boundaries, and partial
rehash visibility. Language cases cover warmed Listing keys, constant and
nonconstant local caches, scalar hashes, and class qualified names. Host tests
publish a value before suspending in key hashing, then fail, retry, traverse old
and new cache slots, and collect between VM steps. They separately exercise
cancel, timeout, close, and stale completions. These lifecycle checks are not
raw Java byte comparisons or proof of same-cache cyclic reentry compatibility.
Mapping length regressions separately check HashSet full-hash filtering,
directional local-cache equality, inherited structural overrides, and boxed
numeric distinctions. Its Host test retains ancestor values across a failed
child hash and retries the count without forcing entry values.

Let diagnostics compare complete Java output for nested bodies, initializer
failures, typed bindings, captured members/functions, and constraints with their
own logical root. Recursive call controls distinguish inserted let frames and
selected uncolored repeated-frame compression. General stack-overflow and
colored-output behavior remain open. Failed exploratory captures remain
retained alongside promoted regressions.

Type-use ownership tests distinguish persistent executable nodes from per-call
constraint receiver slots. Retained-heap assertions count ordinary values and
executable entries separately, preserve exact historical value counts, and
require zero total entries after terminal cleanup. They cover source-level Host
reentry, cancellation before and after local resolution, independent evaluator
state, recursive retries, and lazy collection members. Persistent Java sequences
and Node/browser scenarios compare complete ordered outcomes separately from
these internal lifetime checks. Finite generic instantiation and selected
let/for boundaries are included; complete generic instantiation, alias-root
identity, and the full lexical scope model remain outside this bounded contract.

Direct generic constructor comparisons distinguish bare type parameters from
explicit generic aliases, nullable defaults, actual-type diagnostic origins,
and unused versus forced member failures. Ordered calls alternate successful
and invalid actual types without resetting the evaluator. Native tests separately
check Host reentry and failed-read retry, 24 nonterminal cancellations, captured
functions, evaluator isolation, and terminal release. A process-isolated test
constructs 768-deep defaults on a 256 KiB evaluator stack with a 60-second parent
deadline; compilation runs on the ordinary stack. That test collects during
bounded execution and does not establish peak-RSS or wall-time parity with Java.

Strict Map comparisons cover per-pair key/value evaluation and immediate key
hashing, duplicate replacement with preserved insertion positions, boxed keys,
lookup, parameterized checks, nested Maps, and selected equality/hash ordering.
Map rendering is non-forcing; its observations are separate from deep forcing
and hashing. Persistent sequences retain successful and failed calls in one
evaluator. Native tests collect during cooperative yields and exercise pending
builders, query keys, value comparison, Host reentry, failed-read retry,
cancellation, evaluator isolation, stale handles, and terminal cleanup. Nested
Map hashing also runs at depth 768 on a 256 KiB evaluator stack in a child
process with a 60-second parent deadline. JVM identity-dependent hash order,
general compound output and the remaining Map APIs stay outside this
bounded development contract.

Strict Set comparisons additionally distinguish lookup from insertion hashing, duplicate
elimination, table order from element order, eager element checking, and non-forcing
multiline strings. Scalar observations cover generated and spread elements; original
general compound result failures remain recorded outside the adapter's selected scope.
Native tests cover pending construction, membership queries and comparison operands,
nested Host failure and retry, repeated cancellation, evaluator isolation, and terminal
release. Ordinary cases collect at each yield. The separate 768-deep Set test uses a 256
KiB evaluator stack, a 60-second parent deadline, one instruction per poll, collection
every 128 polls, and a limit of 4,000,000 polls per event. Earlier every-yield timeout
and failures at 200,000 polls per event are retained; these different conditions do not
establish the same resource contract or Java performance parity. Alias tests distinguish
retained ordinary values from executable metadata, verify repeated-call plateaus, and
require zero terminal heap entries. Existing small-stack Map and generic tests keep
their conditions.

Abstract `Collection<T>` comparisons distinguish List and Set from Listing,
Mapping, Map and scalar values. They cover bare and parameterized types, nested
lazy List wrappers versus eager Set checks, aliases, defaults, nullable attached
defaults, casts, union checking and Reference metadata. Empty explicit type
argument lists are compared at their original source locations, with other
collection and alias syntax as controls. Native checks collect at every yield
and pending read, use one instruction per poll with at most 200,000 polls per
event, and exercise retained receivers, Host reentry, failed-read retry, 24
nonterminal cancellations, evaluator isolation and terminal cleanup. Separate
deep metadata checks run on a 256 KiB stack; they do not exercise deeply authored
parser input or establish Java speed or peak-RSS parity. Historical small-stack
tests retain their original sources and execution conditions.

Authored frontend regressions exercise source nesting separately from deep values
constructed by the VM. Java comparisons retain the original deep Set equality and
Map-key inputs, operator associativity, short-circuiting, wide expressions, error
order and complete source diagnostics. Native process-isolated checks parse, clone,
resolve, compile and release authored trees on the same 256 KiB thread stack, with
a 60-second parent deadline. Partial parse/resolution/compilation failures must
release completed subtrees on that stack as well. Instrumented compilation and
constant-key comparison/rendering are separate reachable paths. Ordinary deep
chains, shallow scope boundaries around them, retained recursive-boundary guards
and accepted inputs near those guards require distinct checks. Deep type-boundary
checks cover parsing, tree copying and release; compilation of deeply authored
types and nested lexical string interpolation have separate observation records.
The recorded depth-64 lexical-string stack overflow remains a known failure;
removing it from a passing checkpoint never turns it into a successful guard test.
These tests do not establish arbitrary syntax depth, Java stack-overflow diagnostics, uniform
cancellation latency or workload-level speed and memory parity.

Lossless string regressions distinguish UTF-16 value identity from UTF-8 output.
The pinned Java API probe records code units, code points, hash codes and output
bytes for lone, paired and reversed surrogates. Differential cases cover
concatenation, interpolation, literal types/defaults, Map/Set keys, strict
collection stringification, Reference metadata and malformed-escape error order.
Native checks inspect actual units through Host suspension, reentry, failed-read
retry, collection, cancellation and terminal release. The Node and Chromium
scenarios compare complete Java-derived output payloads. These output adapters
observe Java's UTF-8 writer; they do not establish lossless public JavaScript
string transport. Diagnostic frame regressions additionally distinguish unmatched
surrogates from literal question marks in nested entry/function counters and
recursive compression. Native tests inspect internal names through Host reentry,
failed retries, collection and detached errors after evaluator closure; Java
captures provide the independent numbering and compression reference. Delayed
integer-literal errors retain their entry/element scope. Typed-property controls
distinguish object-local inline checks from module/class declarations, including
local declarations and aliases that retain their separate call frames. Surrogate
URI handling, the complete diagnostic corpus and full string builtins remain
separate requirements.

Numeric value-frame regressions compare complete Java diagnostics and successful
values for direct negation, parentheses, radix/minimum integers and signed zero.
Native checks retain authored observation spans while inspecting executable
value spans, declaration ownership, suspended constraints, reentry, retry,
per-instruction collection and detached errors after close. Node and Chromium
run the same fresh Java-derived cases, including successful value controls.
Exploratory nested-negation overflow differences remain recorded separately;
these checks do not complete numeric diagnostics.

Forbidden object property annotation regressions compare full Java diagnostics
at the actual fixture URI, with accepted module, class and local declarations as
controls. Native checks cover authored owners, first-error order, literal and
computed entry labels, generator prefix revisits, supplementary positions and
unmatched-surrogate string keys. Node and Chromium compare the complete
Java-derived payloads through fresh evaluators. These frontend checks establish
neither Host lifecycle behavior nor a new small-stack or performance result.
Const-modifier and inferred-parent regressions additionally compare enclosing
versus expression owners and method-versus-lambda error ordering. Successful
untyped and explicit Dynamic method calls, unused lazy methods and earlier
lexical errors are controls. Typed method return defaults and inherited property
inference now have selected Java comparisons, including class and collection
parents, nullable/defaulted unions, aliases, lexical owners, lazy failures and
complete default frames. Native tests collect between VM steps and exercise
Host failure, retry, cancellation and terminal release. Prototype type defaults
and instance initializers have separate expectations. The exhaustive portable
runner `tools/platform/compare-language-cases.mjs` records each complete payload
twice per Node/browser target and fails on any mismatch. The historical
`compare-method-parents.mjs` command delegates to that runner. Two original
`Dynamic.toString()` failures remain selected alongside separate type controls;
the development exception list is unchanged.

Declaration annotation regressions compare module, class, alias, property, and
method initialization order, lazy fields, eager generators/spread, const scope,
type errors, and complete diagnostic frames. Persistent sequences distinguish
published class/alias singletons and successful member prefixes after failure
from retryable ordinary properties. Builtin cases verify nullable and empty
listing defaults, required Since/SourceCode fields, hierarchy checks, and lazy
element errors with pinned `base.pkl` frames. SourceCode's union has no default;
missing `language` must not be replaced with Go.

Native annotation tests use actual Pkl resource reads to exercise suspension,
collection, nested contexts, nonterminal cancellation, stale completions,
evaluator isolation, and terminal cleanup. Separate Java/native Host observations
through `annotation_host_probe` compare source-driven callback reentry and full
ordered status/stdout/stderr. Earlier injected-instruction checks remain
architectural evidence with a separate scope. Neither contract establishes Host
I/O through the scalar Node/Wasm language adapters or complete module setup/cache
behavior.

The generic portable runner takes a fixture module exporting `cases` and
`context`; each case carries exact source bytes/hash, its property-path expression,
and one complete Java-derived value or error. It validates fixture source
identity, records artifact/runtime identities, and derives its counts from the
selected array. Use a fresh output directory and the pinned browser tooling:

```sh
node tools/platform/compare-language-cases.mjs target/m2/annotations-platform-new target/m2/platforms-new/wasm target/m2/platforms-new/pkl_napi.node tools/platform/annotation-cases.mjs
```

An acquisition, source-hash check, or documented command is not a passing
platform result. Full reflection and builtin metadata remain open, including the
unsynthesized `@SourceCode` metadata on `Deprecated.replaceWith`. `ModuleInfo`,
converters, deprecation logging, module-cache semantics, general object
`toString`, and `List.map` retain their implementation or verification gaps.
Preserve the original failing inputs and oracle internal errors; independent
controls do not replace their contracts or close the full language gate.

M1 experiments have narrower, explicit contracts:

```sh
python3 tools/m1_frontend.py --check-contract
python3 tools/m1_semantics.py --check
python3 tools/m1_frontend.py --output target/m1/frontend-new
python3 tools/m1_runtime.py --output target/m1/runtime-new
python3 tools/m1_host.py --output target/m1/host-new
python3 tools/m1_lifecycle.py --output target/m1/lifecycle-new
python3 tools/m1_platforms.py --output target/m1/platforms-new
```

See [frontend](docs/m1-frontend.md), [runtime](docs/m1-runtime.md),
[semantics](docs/m1-semantics.md), [Host](docs/m1-host.md), and
[platform](docs/m1-platforms.md) contracts, plus the
[closing lifecycle comparison](docs/m1-lifecycle.md). Primitive comparisons are not full
Pkl renderer/error comparisons. The latest M2 regression capture matches two of
the three original runtime diagnostics; the cycle difference remains. The
platform command returns nonzero for its two known Java regex
backend differences. CI uses the separately named architecture contract mode,
which still reports failed compatibility cases and requires the exact gap
inventory. A green experiment job cannot establish release conformance.

Run `python3 tools/m1_runtime.py --benchmark --output target/m1/performance-new`
on a quiet host with the separately pinned optimizing Java profile. Preserve
negative results, convergence data, compilation traces, and the different API
boundaries; do not interpret a prototype ratio as a product performance claim.

Run `python3 tools/m1_graal.py --output target/m1/graal-new` to reproduce the
[Graal compilation investigation](docs/m1-graal.md) on the pinned macOS arm64 JDK.
Use `python3 tools/m1_graal.py --check-report target/m1/graal-new` to verify the
stored raw evidence without executing that JDK. The diagnostic class override
is an explicitly separate causal control, never an oracle or performance update.
The unchanged-JAR positive workload requires actual guest Tier 2 compilation
and per-call output checks. These checks do not measure performance.

Run the pinned Java 0.32.0 oracle and Rust with identical inputs and environments. Compare:

- Raw stdout and stderr bytes, and exit codes.
- The set of generated files, their paths, and their raw bytes.
- Error messages, whitespace, source locations, underlines, call frames, hints, and error order.
- CLI options and defaults, REPL state, and API results, errors, cancellation, and close behavior.
- Server protocol encoding, callbacks, and session lifecycles.

Fix all inputs that affect results, including working paths, locale, time zone, terminal settings, environment variables, and network responses. Resolve path differences through the execution environment, such as identical mounts. Formal comparisons must not remove differences with `trim`, sorting, JSON reserialization, or arbitrary string substitutions.

Protocol fields that vary between runs, such as Java's random evaluator IDs, need a separate contract. Codec tests supplied with fixed IDs compare all bytes. Tests of actual sessions verify that issued IDs are referenced consistently and validate result payloads; they must not be described as byte equivalence for the entire session.

Generate expected results from the Java oracle and review their provenance and changes. Do not resolve mismatches by overwriting expected results with the current Rust output.

## Planned Test Layers

Stage63's collection-addition and Set-endpoint fixtures extend the shared
catalog to eight Java authorities. Native, N-API and Chromium comparisons
check all seven common raw channels and whole process protocols for 680
expressions in two sessions per target. UTF-16 comparisons preserve lone
surrogates. The Java-only internal identity supplement remains a distinct
boundary from public string results and typed exports.

The [Stage63 checkpoint](reports/m2/stage63-implementation-checkpoint.json)
separately records the unchanged original 91-program CLI/output-file contract,
whose nonpasses remain failures, and the known Any/M1 Regex differences.
Its initial platform acquisition failed a command-provenance admission check
because the M1 launcher used an absolute Python executable instead of the
recorded literal `python3` contract. Preserve that capture and failed preflight;
the fresh acquisition uses the existing verifier without changing expected
bytes or rewriting the original execution record. Replaying saved comparisons
does not count as another guest execution.

| Layer | Scope |
| --- | --- |
| Test tooling | Failure detection for empty corpora, missing expected results, unavailable runners, and byte differences |
| Parser and resolver | Syntax, invalid input, source spans, name resolution, and formatter round trips |
| Runtime and types | Lazy forcing, inheritance, closures, cycles, constraints, generics, and the first error |
| Standard library and renderers | Every public member, boundary values, output order, converters, and binary output |
| CLI, REPL, and server | Help and options, PTY sessions, state retention, protocol, timeouts, and close behavior |
| I/O and packages | HTTP fixtures, redirects, proxies, CAs, root restrictions, checksums, caches, and lockfiles |
| Pantry | Development trees, packaged modules, real examples, and generated files at the pinned revision |
| Platforms | Native, Wasm, and N-API results, errors, and Host capabilities |
| Generation and transformation | Grammar-based generation, differential fuzzing, property tests, and metamorphic tests |
| Long-running execution | Repeated successes and failures, growing sets of unique sources, cancellation, evaluator creation and shutdown, and memory reclamation |

Each test manifest records the case ID, source repository, revision and path, input and resource hashes, execution arguments and environment, required Host capabilities, expected success or error, channels and files to validate, and execution targets. Pinning a Pantry commit alone does not establish 0.32.0 compatibility, so first verify the selected cases and execution conditions with the Java oracle.

Distinguish passes, failures, tests not run, unimplemented cases, and environment errors. Do not count skips or known failures as passes. Add minimal reproductions of failures to the required regression cases, and compare the test inventory with actual execution results to detect omissions.

Reference regressions compare import identity, constructor kinds, declaration
metadata, unforced referent bodies, normalized types, synthetic paths, equality,
key behavior, and complete diagnostics, including frames from the pinned
`pkl:ref` source. Native lifecycle tests exercise domain rendering across Host
suspension, nested evaluation, GC, retry, cancellation, close, and timeout.
Node and Chromium execute the same engine for Reference values and errors.
Original upstream files evaluated through a selected API expression remain
exploratory evidence; those calls do not replace their original renderer or
snippet-suite contracts. Reviewed implementation/test links preserve these
unexecuted cases and the full completion gate.

Resource-read regressions preserve keyword and quoted-method distinctions,
operand forcing, nullable absence, URI validation and policy order, declaration
source resolution, and complete diagnostics. The original controlled filesystem
matrix retains unsupported I/O and unmatched-surrogate cases. Source-path
language probes acquire their own Java diagnostics at the identical recorded
URI; fixed portable URI observations do not substitute for those captures.

Portable fixtures with Java UTF-16 captures compare exact code-unit bytes and
Node UTF-8 derived from those captured units. Original Java UTF-8 bytes remain
separate evidence, with cross-encoder differences reported explicitly. The API
returns JS strings; this comparison does not claim that Java and Node serialize
an unmatched surrogate to identical UTF-8 bytes. Legacy fixtures without unit
captures keep their raw UTF-8 contract. An explicitly marked Java internal error
always remains `oracle-error`, even when its payload matches. Per-case power
assertion flags override the shared context and are recorded in each execution.
See the [resource-read contract](docs/m2-resource-reads.md).

### Stage 73–75 local verification boundary

The integrated evaluator's five required Rust commands ran against a frozen
3,270-path canonical source snapshot and all exited zero. The workspace test
count is 1,069 distinct passes, zero failures, and two ignored; 13 nested
child reruns also appear in raw harness output and are not additional tests.
Selected native, N-API, and Chromium comparisons each cover 310 queries with
two observations, 620 per target. Preserve the 91 earlier queries whose Java
capture lacks logger channels as public-result comparisons only. The separate
M1 experiment passed its architecture contract and retained two Regex
compatibility failures per target.

The full 4,416-case Rust replay records **4,163 matches, 110 mismatches, 141 unsupported results, 2 Java oracle errors, and 0 Rust internal errors**. Original modules match 643/888; expression strings match 3,519/3,527. The JVM classpath case matches. Compared with Stage69–72, 5 previously nonpassing cases now match, with no prior-pass regressions.

The linked replay, separate integrity verifier, and independent regression audit completed successfully. The replay reuses sealed Stage67 Java outputs and starts no fresh Java process. The strict full gate retains 1,791 overlapping unmet conditions. The 111 unready inventory cases and all nine M2 workstreams remain open. Earlier mapping rejection, interruption, ENOSPC, and monitor failures remain recorded separately.

See the [implementation checkpoint](reports/m2/stage73-75-implementation-checkpoint.json) for raw evidence and execution provenance. These local records do not establish full M2, product API, release-platform, or performance compatibility.

### Stage76–78 canonical local verification boundary

The integrated 17,774-path source snapshot passed the five required Rust
commands locally. Native, N-API and Chromium selected comparisons cover 771
queries with two observations per target; only channels present in Java authority
receive a compatibility verdict. Strict M1 raw comparison matched 6,078/6,078
per target. Its prior `--verify-contract` policy failed because it listed
the two now-matching Regex cases as known gaps. The revised contract completed
with exit zero and an empty known-gap set. Strict matching and architecture-
contract acceptance remain distinct gates.

The full selected replay recorded 4,188 matches, 86 mismatches, 139 unsupported,
2 Java oracle errors, and 1 Rust internal error among 4,416 ready cases. The
separate verifier and independent regression audit exited zero. The audit found
25 improvements, zero prior-pass regressions, and 1,791 overlapping full-gate
conditions. The `classes/constraints14.pkl` internal error, all other
nonpasses, and 111 unready cases remain in the release gate. No complete API,
cross-platform release, or performance result follows from these local checks.

### Stage84 local verification boundary

The complete workspace command passed 1,126 tests with zero failures and two
ignored tests; 13 nested child observations and one explicit formerly ignored
Regex regression are separate. Its opt1/debug0/strip profile enables assertions
and overflow checks. All five required Rust commands passed on the frozen local
source set. Native, N-API, and Chromium each match 88 observations of the new
44-query List cohort, including ordered Logger fields. Both fresh M1 experiments
match all 6,078 Java results per target. The
[checkpoint](reports/m2/stage84-list-search-transform-checkpoint.json) retains
the exact profiles, reused artifact provenance, raw failures, and comparison
boundaries. The still-running full replay and complete M2/API/performance gates
are separate from these results.

## Performance and Memory

The [initial Java benchmark contract](docs/benchmarks.md) implements cold CLI, persistent evaluator, and fresh evaluator measurements for six fixtures. Use a quiet host with the pinned optimizing runtime:

```sh
python3 tools/oracle.py fetch --profile performance
python3 tools/benchmark.py --check-contract
python3 tools/benchmark.py
python3 tools/benchmark.py --check-report target/m0/benchmarks
```

Every run uses a new evidence directory. Raw bytes, per-process RSS, first requests, warmup windows, and measurement windows are retained. Nonconverged forks remain inconclusive. These initial Java measurements do not complete the required workload inventory or provide a Rust performance verdict.

Only runs that produce correct output are eligible for measurement. Every required workload must satisfy both conditions:

```text
Rust execution time / Java execution time ≤ 1.00
Rust peak RSS / Java peak RSS ≤ 1.00
```

Fix the Java executable artifacts, JDK and Graal/Truffle configuration, hardware, OS, and measurement method. Compare against a sufficiently warmed-up JVM and record results for the official native-image CLI separately. Do not judge the overall performance target solely by cold JVM startup time.

| Usage | Measurement Boundary |
| --- | --- |
| CLI | From process startup through completed output and process exit |
| Persistent evaluator | Initialization, first request, and repeated requests measured separately |
| REPL and server | Session creation, request latency, long-term memory usage, and shutdown |
| N-API | Includes JS↔Rust value conversion and result delivery |
| Wasm | Download, compilation, and initialization measured separately from requests to a persistent instance |
| Packages and large inputs | Cold and warm caches, import graphs, large outputs, and failure paths |

Record latency distributions and peak RSS across repeated runs for each workload. Report results as inconclusive when measurement uncertainty prevents a judgment. Do not use average improvements to conceal regressions in individual required workloads.

For Wasm and N-API, first define measurement boundaries that include the Host process. JVM heap usage, Rust allocations, and Wasm linear memory are diagnostic metrics; do not compare these different metrics directly to claim equivalent memory usage.

## Release Criteria

The required feature and API inventory must have no omissions, and there must be zero known mismatches within the required scope on target platforms. Completion cannot be declared while required tests remain unrun or unimplemented, or performance and memory measurements remain inconclusive. Do not document complete compatibility or guarantee performance equivalent to Java before these criteria are met.
