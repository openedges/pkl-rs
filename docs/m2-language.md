# M2 language verification

M2 extends the shared Rust engine established in M1. Its completion gate covers
the language scope in [PLAN.md](../PLAN.md), including classes, generators,
constraints, generics, `Reference`, and complete diagnostics. The first executable
selection exercises only part of that scope.

`compatibility/m2-language.json` keeps semantic families, pinned source
declarations, upstream language snippets, selected historical regressions, and
unmapped historical candidates visible. Declaration and heading counts are
inventory counts, not semantic coverage. An implemented parser node does not
establish its runtime behavior.

Some accounting rows intentionally overlap: selected rev1 execution contracts
also have historical candidate rows, parser declarations can share names, and
language-reference headings can cover the same behavior. Totals must not be
presented as distinct feature or test counts. Semantic mappings require review.

`compatibility/m2-mappings.json` is the reviewed input for linking existing
requirement IDs to Rust implementation symbols and existing case IDs. Its
baseline must match the pinned inventory. The manifest generator validates
source paths and symbols, records current source hashes, adds reverse case
links, and preserves previously selected supporting cases. A source symbol's
presence establishes provenance, not semantic completeness.

Each reviewed entry states the supported scope and remaining work. Partial
implementation or test mappings continue to fail the full gate. Completed
mapping labels still require all linked cases to execute and pass. Linking an
original upstream ID does not change its input, adapter, execution state, or
output boundary. Selected reproductions therefore cannot silently replace the
original snippet contract. Missing IDs, duplicate links, source drift, wrong
baselines, and contradictory scope claims fail validation. The reviewed entries
link bounded Reference, modifier, class, import, metadata, and diagnostic
implementations to selected cases. Every entry remains partial and records its
remaining scope.

```sh
python3 tools/m2_manifest.py --check
python3 tools/m2_identifier_tables.py --output target/m2/identifiers-new
python3 -m unittest discover -s tools/tests -p 'test_m2_*.py' -v
python3 tools/m2_language.py --output target/m2/language-new --record-only
python3 tools/m2_language.py --verify-report target/m2/language-new
python3 tools/m2_language.py --verify-development target/m2/language-new
python3 tools/m2_language.py --verify-subset target/m2/language-new
python3 tools/m2_language.py --verify-full target/m2/language-new
```

Use a fresh output directory. The runner records a release build of
`language_probe`, freezes its executable, compiles the Java probe against the
pinned oracle, and retains source snapshots and raw process captures.
`--candidate` is an explicit artifact override whose source binding is unverified.

The Java boundary is `evaluateExpressionString` with a file module and an
expression. Both processes receive the same absolute source path. Successful
UTF-8 stdout, full uncolored `PklException.getMessage()` stderr, and exit codes are
compared without normalization. A separate outcome file records whether Java
returned normally or caught a particular `PklException` class. This distinguishes
a handled upstream bug report from an uncaught JVM failure without changing
stdout or stderr. The initial Rust probe accepts scalar property
paths. This stringification boundary does not prove typed API compatibility or
compound-value rendering.

The default uses Java's API setting with power assertions disabled. Selected
cases can opt in with `"power_assertions": true`; the runner then passes
`--power-assertions` to both probes and verifies that option in the recorded
commands. These cases compare the complete value graph, message, hints, and
frames as raw bytes. Enabling this profile does not change historical captures
or the default mode of other cases.

The commands have distinct meanings:

- `--verify-report` checks evidence integrity and recomputes recorded outcomes,
  including failures.
- `--verify-development` requires selected cases to match except for the explicitly
  tracked cycle diagnostic failure and integer remainder-by-zero upstream bug.
  Both remain failed in the report.
- `--verify-subset` additionally requires every selected comparison and
  metamorphic relation to pass.
- `--verify-full` additionally requires all language requirements to have
  implementation and test mappings, every required case to execute successfully,
  and the candidate to have a recorded build.

Unknown, unimplemented, unexecuted, unmapped, and mismatching requirements remain
failures of the full gate. Selected-case success and source inventory freshness
must never be reported as M2 completion.

CI acquires evidence with `--record-only`, which returns success only after a
complete, internally consistent acquisition, regardless of compatibility outcomes.
It then applies the named development contract. An optional full-corpus workflow
also runs the strict full gate.

The initial cyclic property probe exposes an oracle limitation: cold Java calls
can fail while initializing `VmStackOverflowException` on an exhausted stack.
A separate diagnostic experiment preinitializes that class without changing the
JAR; it produces controlled Pkl errors, but the rendered repetition count varies
across runs. Both conditions and all raw bytes are retained in
`reports/m2/cycle-evidence.tar.gz`. Preinitialization is not applied to the normal
oracle, and no repetition counts are normalized. This unresolved case prevents
strict selected-case completion.

For integer `1 % 0`, the pinned Java implementation reports a caught
`PklBugException` caused by `ArithmeticException` in `VmSafeMath.remainder`.
The development exception is limited to that exact input and failure shape;
it additionally requires a separate cold Java capture with identical complete
raw bytes. The Rust diagnostic must retain its recorded complete division-error
form. No stack traces are normalized or treated as matches. A changed failure,
timeout, or different output fails the development gate. Resolving the raw
diagnostic difference is still required by the strict selected and full gates.

Runtime diagnostics carry authored source spans and logical frames separately
from their message. Instruction spans are parallel metadata, preserving the
bytecode dispatch count. The text renderer is shared runtime code; native probe
code supplies source URIs and the expression boundary frame. The same
experimental scalar boundary is available as `evaluateLanguage` in the Node
and Wasm development bindings, including complete diagnostics. It accepts a
source string, URI, inferred module name, and scalar property path; explicit
module declarations take precedence over the inferred name.
An optional fifth Boolean argument enables power assertions, with the same
disabled default as the native probe and Java API.
Returned Reference values also pass through their domain renderer, matching the
Java expression-string boundary's automatic conversion and its separate error
stack. This adapter still accepts property paths only. It does not supply Host
I/O, general compound-value conversion, or the full expression API.

The generated identifier tables reproduce the pinned lexer's UTF-16 code-unit
predicates. `m2_identifier_tables.py` records the raw Java intervals and checks
the generated Rust bytes. CI repeats this check against the pinned JDK.

M1's platform and Host contracts remain regression requirements. New language
features must retain captured values across VM suspension and collection and use
the same engine in native Rust, browser Wasm, and Node N-API.

## Persistent class initialization sequences

Independent expression processes cannot verify state retained after an error.
The separate [sequence manifest](../compatibility/m2-class-lifecycle/manifest.json)
declares ordered property-path calls within one evaluator and module. Each
sequence starts with a fresh evaluator and repeats in a second fresh evaluator;
errors within a sequence do not reset its evaluator. Fixture helper properties
contain the expressions being observed. The Java and Rust adapters use the same
source bytes, fixed `file:///m2-platform.pkl` URI, and disabled power assertions.

```sh
python3 tools/m2_class_lifecycle.py --output target/m2/class-lifecycle-new --record-only
python3 tools/m2_class_lifecycle.py --verify-report target/m2/class-lifecycle-new
python3 tools/m2_class_lifecycle.py --verify-contract target/m2/class-lifecycle-new
```

The runner retains source snapshots, the recorded Rust build and frozen
executable, Java classes and oracle identity, ordered process markers, and each
call's complete status/stdout/stderr bytes. Java exception class names are
provenance metadata; they are not compared to Rust implementation type names.
The report verifier recomputes comparisons from the captured bytes. The bounded
contract requires every selected call to match, with no mismatch allowance.
`--java-only` acquires exploratory oracle observations and cannot pass that
contract. Unexecuted groups and nonterminating oracle observations remain
explicit gaps. Sequence executions and call comparisons have their own counts;
they are not added to the independent-expression selection or treated as full
language coverage.

The shared `evaluateLanguageSequence` development binding returns ordered
`value\t...` or `error\t...` strings in Node and Wasm, preserving complete value
and diagnostic payloads. Native acquisition uses the same runtime boundary.
Rust collects between calls while retaining the module root. Dedicated native
tests separately verify evaluator-owned class roots, disposable object
reclamation, isolated heaps, ordinary failed-property retries, and terminal
cleanup. Internal bytecode suspension tests establish scheduler behavior;
they do not establish that the same suspension path is available in Pkl source.

The collection-type selection compares lazy and eager checking, wrapper
composition, aliases, defaults, constraint identity, cache reuse, and complete
diagnostics at the actual fixture URI. Exploratory captures remain separate:
unimplemented format-time cache stealing can cause further checks while Java
renders a diagnostic, including uncaught Java rendering failures. These
observations do not add development exceptions or establish full generic
collection compatibility. See the [checkpoint evidence](../reports/m2/README.md).
