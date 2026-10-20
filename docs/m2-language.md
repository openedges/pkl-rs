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

```sh
python3 tools/m2_manifest.py --check
python3 tools/m2_identifier_tables.py --output target/m2/identifiers-new
python3 -m unittest discover -s tools/tests -p 'test_m2_*.py' -v
python3 tools/m2_language.py --output target/m2/language-new
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

The generated identifier tables reproduce the pinned lexer's UTF-16 code-unit
predicates. `m2_identifier_tables.py` records the raw Java intervals and checks
the generated Rust bytes. CI repeats this check against the pinned JDK.

M1's platform and Host contracts remain regression requirements. New language
features must retain captured values across VM suspension and collection and use
the same engine in native Rust, browser Wasm, and Node N-API.
