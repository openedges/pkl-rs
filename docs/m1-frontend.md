# M1 frontend experiment

The shared runtime now contains a small parser, an explicit source-coordinate map,
a static resolver, and structured diagnostics. This is an architectural experiment
for Pkl 0.32.0. It does not implement the full language or the product diagnostic
renderer. Its successful cases do not establish release compatibility.

`tools/m1_frontend.py` executes the pinned Java artifact and the actual Rust
frontend/bytecode VM. The Java helper uses `org.pkl.parser.Parser` for source spans
and parser messages, Java string/code-point APIs for coordinates, and the Java
`Evaluator` for five scalar results. Java supplies every compared reference byte.

```sh
python3 tools/m1_frontend.py --check-contract
python3 tools/m1_frontend.py --output target/m1/frontend-new-run
python3 tools/m1_frontend.py --check-report target/m1/frontend-new-run
cargo test -p pkl-runtime --locked --test frontend
python3 -m unittest discover -s tools/tests -p test_m1_frontend.py -v
```

The output directory must be new. It retains fixture copies, untouched Java
stdout/stderr, the Rust protocol result file, Rust test-harness stdout/stderr,
commands, exit statuses, source snapshots, source/compiler/artifact identities,
and per-file hashes. Sources changing during acquisition make the report
inconclusive. `--check-report` requires the complete fixed case inventory,
nonempty protocol results, successful probe processes, unchanged evidence, and
exact raw protocol bytes; it does not trim, sort, rewrite paths, or reserialize
an implementation's results. The Rust test harness emits protocol bytes to a
separate file so libtest progress text cannot contaminate the comparison.

The 15-case protocol compares all scalar-boundary coordinates; top-level property
and expression spans; complete `ParserError.message` bytes and first-error spans;
and five scalar evaluation results. It does **not** compare complete ASTs, CLI
stdout/stderr, formatted evaluation errors, call stacks, output renderers, or
all resolver cases. The compared parser errors include an astral character and a
combining mark before the error, CRLF, an early syntax error followed by a later
invalid token, EOF with/without trailing whitespace, and missing delimiters.

## Coordinates and diagnostics

`source::Span` is a half-open UTF-8 byte range. `SourceMap` maps scalar boundaries
explicitly to UTF-16 offsets, Unicode scalar indices, and one-based lexer line and
UTF-16 column. It rejects UTF-8 continuation bytes and UTF-16 offsets inside a
surrogate pair. Combining marks count as separate scalars, rather than grapheme
clusters. LF advances the line; CR contributes a column until LF, matching the
pinned lexer. Rust strings cannot represent isolated UTF-16 surrogates; that
separate JVM input space remains unimplemented.

The pinned parser's `Span.charIndex` and `length` index Java `char[]`, hence UTF-16,
not bytes. EOF diagnostics can underline one UTF-16 unit beyond the last token,
including a virtual position beyond the source. Rust keeps the byte span empty
at that source boundary and records `Diagnostic.highlight_length_utf16`
separately. A rendering layer must use both fields, rather than fabricate a
nonexistent source byte. `DiagnosticKind`, message, source span, and logical-frame
records remain separate from product-specific rendering. The complete Java
error format and runtime-frame population remain future work.

The present SourceMap stores one record per scalar. That makes conversions clear
but has significant memory overhead; sparse indexing and large-source costs
have not been measured. This experiment makes no SourceMap performance claim.

## Bounded grammar

Implemented syntax comprises ASCII identifiers, decimal signed 64-bit integer
literals, booleans, single-line strings with basic escapes, parentheses, `+`, `-`,
`*`, `/`, member access, `this`, `super.member`, `let (name = value) body`, module
and object property assignments, `local` properties, `new { ... }`, explicit
`(parent) { ... }` amendments, semicolons, and line comments. Newline-sensitive
minus follows the pinned parser. Reserved keywords cannot become identifiers.

The prototype limits recursive expression nesting to 32 and its expression/node
budget to 512. Exceeding either returns an explicit unsupported diagnostic.
The earlier limit of 128 overflowed the Rust debug test-thread stack in a hostile
nesting test; the lower bound and a regression preserve that finding. These
limits intentionally differ from the eventual product contract.

Unicode identifiers, backtick identifiers, block/doc comments, annotations,
imports, declarations, parameterized objects, functions, typed properties,
floats, numeric bases/separators, interpolation, multiline strings, and other
operators remain unsupported. Overflow is currently rejected while constructing
the bounded AST; exact Java static-error phase/order for oversized literals is
unverified. Unsupported constructs must not be interpreted by a fallback parser
or evaluated by Java in production.

## Static binding and receiver contract

The resolver registers declared members before resolving member expressions.
It emits `Binding::Lexical { depth, slot }` for let bindings,
`Binding::Property { owner_depth, local }` for lexical member declarations, and
`Binding::ImplicitThis` when no lexical declaration exists. In this prototype,
let depth counts captured let bindings, while owner depth counts object scopes.
Object creation captures its let environment without adding a let level.

Nonlocal lexical properties dispatch virtually on the receiver associated with
the selected lexical scope. Local properties read from their exact owner.
`this` is the current receiver. `super.member` starts at the defining owner's
parent while preserving that receiver. Unknown names search the current
receiver; they do not search enclosing objects dynamically. For example, with a
module `x = 1`, a base object's `result = x` continues to read the module's `x`
even if an amendment introduces `x = 2`. The five Java/Rust value comparisons
exercise this distinction, local owner lookup, explicit `this` and `super`, let
capture, and fallback to the current receiver.

Implicit base-module properties/classes/type aliases are explicitly unsupported
unless shadowed by a lexical declaration. The rejection set follows the pinned
`BaseModuleMembersGenerator` inputs, rather than accidentally treating builtin
classes as missing receiver properties. Full class/base-module resolution,
const rules, ambiguous locality, constraints, and invisible amendment scopes are
not implemented. Contextual untyped `new` is permitted only as a member RHS,
including a let body; other placements return unsupported until full inferred
parent semantics exist. A real Java comparison found `new { ... }.member`
invalid where the initial Rust prototype returned a value; its negative
regression prevents that false success.

Upstream evidence is pinned in:

- `external/pkl/pkl-parser/src/main/java/org/pkl/parser/{Span,Lexer,ParserImpl}.java`
- `external/pkl/pkl-core/src/main/java/org/pkl/core/ast/builder/{SymbolTable,AstBuilder}.java`
- `external/pkl/pkl-core/src/main/java/org/pkl/core/ast/MemberLookupMode.java`
- `external/pkl/pkl-core/src/main/java/org/pkl/core/ast/member/FunctionNode.java`
- `external/pkl/pkl-core/src/main/java/org/pkl/core/ast/expression/member/ReadSuperPropertyNode.java`
- `external/pkl/pkl-core/src/generator/java/org/pkl/core/generator/BaseModuleMembersGenerator.java`

The first differential attempt also exposed a reserved-keyword acceptance bug:
`local hidden = 7` was incorrectly accepted by Rust and rejected by Java. The
fixture now uses `privateValue`, while regression tests reject `hidden` as an
identifier. Earlier mismatched and build-blocked acquisition directories are
retained separately from the final run. The report records their outcomes;
they are not counted as passing cases.
