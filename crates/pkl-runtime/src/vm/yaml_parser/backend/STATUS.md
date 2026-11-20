# Implementation status and remaining obligations

This shared Rust backend reads UTF-16, scans YAML tokens and scalar forms, uses
an explicit parser production stack, resolves the Pkl mode-specific scalar tags,
and composes document graphs with alias identity, recursive edges and explicit
merge handling. The VM performs Pkl value construction and converter work. The
production evaluator has no Java fallback.

Selected local evidence goes beyond source inspection. The final direct graph
experiment compiled the formatted backend and matched all 32 admitted Java
observations: 22 successes, eight YAML engine errors and two foreign reader
errors, across 273 compared raw fields. Compiler stderr was empty in that final
experiment. Earlier graph mismatches and compiler warnings remain historical
records. Selected native YAML, float and shell harnesses also matched their
242 ordinary Java observations; two Java internal-error observations remain
uncompared nonpasses. These are bounded comparisons, not full YAML compatibility.

The Stage144 production checkpoint report and production default/platform
verification are pending. The source-only draft snapshots, raw failed attempts
and successful local acquisitions retain their own identities; the text in this
production document does not change those original records.

## Explicitly unsupported branches

- Exact PercentEscaper errors for invalid UTF-16 tag input, malformed tag-escape
  buffer overflow and unexpected internal parser state-list rendering.
- Nonterminating explicit-merge expansion, pending the pinned Java lifecycle
  observation.

These branches return `ErrorKind::Unsupported` and remain nonpasses. They must
not become apparently compatible ordinary YAML parse errors. `Foreign` preserves
exceptions outside the YamlEngineException hierarchy, which Pkl does not catch
at its YAML error boundary.

## Remaining verification and lifecycle work

The backend exposes a cooperative step-budgeted `Backend::step(budget)` interface
that yields across scanner lookahead, comment/whitespace runs, parser events,
document graph composition, and explicit-merge expansion so VM polls observe
cancellation and GC safepoints mid-document. Mid-scalar-token yielding inside a
single multi-megabyte scalar remains open.

Exhaustive and generated Java grammar, graph, resolver and diagnostic comparisons
remain necessary. Selected supplementary refill controls preserve complete
rolling-window snapshots; broader refill and lookahead boundaries remain open.
Engine 3.0.1 treats NEL asymmetrically: scanLineBreak may normalize it while
boundary sets, reader line counting and Mark slicing do not treat it as an
ordinary line separator. LS and PS are printable scalar characters.

Pkl number, Bytes and object construction, duplicate-key detection, converters,
key hashing and guest frames belong to the shared VM constructor. The graph
comparisons do not independently qualify those paths. Full language/API,
release-platform, time and memory targets remain open.
