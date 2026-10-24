# DataSize value verification

Pkl 0.32.0 is the compatibility authority. This work connects `DataSize` to the
shared Rust value model, VM, diagnostics and rendering paths. Native, Wasm and
N-API use that same implementation. The selected string comparison does not
establish full public API, whole-language, or performance compatibility.

## Implemented behavior

The value retains a double, one of the eleven upstream units, and reference
identity. Integer and Float unit members, `toDataSize`, unit conversions,
arithmetic, comparison, hashing, nominal type checks and `DataSizeUnit` alias
resolution use the shared runtime. Conversions retain Java's multiplication and
division order. Numerical equality, `Double.compare`, signed zero, NaN and
reference identity are separate operations; small and hashed collections retain
the observed Java distinctions.

DataSize strings are available through the bounded scalar adapters, including
the lossless text adapter, and the existing PCF, JSON-converter and diagnostic
paths. This does not implement every renderer or the typed value transport.
`probe::encode_value` still reports DataSize as unsupported at that boundary.

The related `Bytes(...)` construction and cached `Bytes.size` value use the same
engine. Argument range/type checks, evaluation order, unsigned string display,
empty-value identity and query-versus-module lexical lookup have independent
Java controls. These additions do not complete the Bytes API.

## Independent Java fixtures

[`m2-data-size`](../tests/fixtures/m2-data-size/README.md) preserves the initial
mixed-call experiments and the canonical split acquisition. Only the canonical
string-only sessions define the common candidate string boundary: 255 ordered
query IDs, executed twice in fresh JVMs. Each session contains 233 successful
results and 22 ordinary language errors. The separately traced export sessions
record Java wrapper types, double bits, units, hashes and identity observations;
they are not candidate export passes.

[`m2-data-size-supplement`](../tests/fixtures/m2-data-size-supplement/README.md)
preserves additional unit/default, Bytes, evaluation-order and lexical-scope
controls. The incomplete acquisition remains incomplete. Public exported-object
identity observations are separate from language value identity.

## Comparison boundaries

For every query, compare expression bytes and UTF-16 code units, result status,
full stdout and stderr bytes, and both result code-unit streams. Compare the
entire ordered session protocol as well. Java exception-class fields are producer
metadata outside this common boundary; the candidate never synthesizes them.
Unsupported evaluation remains a candidate failure, even if part of its error
text happens to match Java. No whitespace or diagnostic normalization is allowed.

`tools/data_size_compare.py` acquires two fresh native processes from an existing
recorded helper build and verifies input, source, artifact, process and channel
identities. Its integrity, strict string and full API verdicts are separate.
Use a new directory for each acquisition. The full API gate remains unsuccessful
while the required typed export comparison is unexecuted.

`tools/platform/capture-data-size.mjs` executes each complete ordered session in
a fresh N-API child or Chromium context/Wasm instance. It retains ownership,
collection and terminal heap/context observations. Chromium's protocol is a
page-returned byte stream and its error capture contains page/console errors;
neither is labeled as the browser operating-system stdout or stderr. The
Chromium executable is explicitly selected in the recorded launch options.

`tools/data_size_portable_compare.py` verifies those captures against the same
Java string authority, with the recorded platform build and outer process
wrapper. `--gate integrity`, `--gate string` and `--gate full-api` retain separate
verdicts. Actual platform execution is required; a Wasm build or Java replay
cannot replace it.

## Remaining scope

Typed public exports, Pkl Binary, remaining renderer and Bytes behavior, full
original upstream execution, other release platforms and performance remain
independent requirements. Original module comparisons preserve all CLI output,
errors, exit status and generated files; selected query matches never replace
those original programs. The nine M2 workstreams and strict full completion gate
remain open. See [the language contract](m2-language.md) and [PLAN.md](../PLAN.md).
