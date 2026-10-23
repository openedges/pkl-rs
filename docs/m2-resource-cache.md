# Shared Resource and URI preparation contract

The bounded evaluator implements Resource values, resource caching and triple-dot
URI preparation in the shared Rust runtime. Native, Wasm and N-API use the same
value model, heap, continuations and diagnostics. The Host supplies reader results
and ModuleKey operations; filesystem access and policy decisions stay outside the
engine. This is ongoing M2 work against the pinned Pkl 0.32.0 sources.

`ReadResult` distinguishes `Missing`, lossless UTF-16 `Text`, and `Resource` with
an independently returned URI and owned bytes. A typed Resource becomes an
instance of the canonical `pkl:base` Resource class. Its lazy `uri`, `text`,
`base64` and hidden `bytes` properties retain the standard-library declarations
and type-check locations, including the `Uri` alias. Text decoding and Base64
conversion use the shared Java-compatible primitives. The selected malformed
UTF-8 and Base64 fixtures preserve the actual Java results and complete errors.
Resource digest defaults and the complete Bytes API are not implemented.

Each VM owns its resource cache. Successful Text/Resource results and Missing
are cached; reader failures are not. A cached Missing is absence, not a cached
exception: ordinary `read` creates a diagnostic for the current authored URI and
call site, while `read?` returns null. Nullable reads do not suppress I/O,
security, URI-resolution or triple-dot-search failures. Imported modules and
queries using the same VM share successful entries. A fresh evaluator has a
fresh cache; separate SourceProbe instances do not establish same-evaluator
independent-root API compatibility.

Cache identity is separate from the URI spelling sent to the reader. Relative
resolution and hierarchical dot normalization happen first. The structured key
then follows the implemented Java URI equality rules: scheme and server-host
case folding, percent-escape hex case folding, numeric port equality, and
case-sensitive registry authorities. Escaped characters remain distinct from
their decoded spellings; fragments and opaque paths remain significant. For
example, server ports `080` and `80` share a key, while registry authorities
`A.1` and `a.1` do not. Key folding does not change reader selection on a miss.
The first successful value is retained, and Resource.uri remains the reader's
returned URI even when it differs from the requested URI.

Typed completions use the existing Host admission and one-use request tokens.
`ReadFailure::SecurityManagerException` explicitly carries guest-exception
provenance. Arbitrary Host rejection and the legacy raw value completion path do
not acquire that provenance or typed resource-cache behavior; raw null is not a
substitute for `ReadResult::Missing`. Module preparation similarly distinguishes
capabilities, has-element, resolution and source-loading completions. Invalid
completion phases are rejected before consuming their tokens. Cached values and
suspended computations are GC roots; closing the evaluator clears the cache and
releases the heap. Per-poll collection, callback retry and completion-payload
ownership are checked separately from Java policy and reader-lifecycle claims.

Triple-dot preparation uses the declaring ModuleKey. The engine obtains its
capabilities and asks the Host about candidates in ancestor order, skipping the
current directory and the current module itself. Bare `...` uses the current
filename. This search runs before every relevant ResourceManager lookup,
including a resource-cache hit, so a cache hit cannot bypass ModuleKey policy.
Search failure remains an error for `read?`. The Host does not implement another
search or cache. Capability errors preserve the distinction between an
unavailable read origin and the import operand location attached by AstBuilder.

Import URI preparation follows AST construction, including unused and nested
imports. A single resolver traversal records ordered preparation effects and
the first static failure, preserving declaration categories, nested visitation
order and generator fallback revisits. Earlier effects complete before a later
static failure; a failure stops subsequent effects. Parser syntax errors remain
a whole-source barrier, and runtime type initialization remains lazy. A selected
query has its own preparation sequence; unrequested queries do not run effects.
Compiled lambda diagnostics reuse AST-assigned owners even when bytecode follows
a different short-circuit evaluation order.

The C05 development comparison matched the untouched original `basic/read.pkl`
twice, including its helper sources, arguments, environment, resources, complete
stdout/stderr, exit status and generated files. That result does not establish
complete Resource, filesystem policy, evaluator API or language compatibility.
Lazy `read*` helper syntax is accepted, but forcing resource glob evaluation is
explicitly Unsupported and remains an M2 obligation. Parameterized object-body
forms and separately recorded API boundaries also remain open. Earlier failed
candidates and unexecuted comparisons are preserved; the full M2 gate is not
complete. See [Resource fixture scope](m2-resource-fixtures.md) and the
[whole-language verification contract](m2-language.md).
