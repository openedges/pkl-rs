# Local Pkl runtime extensions

This directory originates from java_regex 0.1.0, crate archive SHA-256
`1f3b3ff81a66205722b636dae12fc5cb2e77147569e8968f38a1d73b2b05fbe6`.
The original 28-file payload and licenses are retained in the Stage76 backend
provenance record. Existing scalar modules remain available; src/lib.rs adds
`Regex::group_count`, `Regex::captures_entire`, and the checked `utf16` module.
The root Cargo patch selects this local crate and excludes its upstream optional
development dependencies and examples from workspace checks.

The shared Pkl language and M1 adapters call the checked raw UTF-16 engine.
Captures use UTF-16 ranges; syntax failures, unsupported execution and explicit
work/depth failures remain distinct. The parser uses heap continuations for
nested groups/classes, and class Clone/Drop/matching use iterative continuations.
The existing compile nesting limit256 and small-stack test conditions remain.

Character category/case tables come from the pinned JDK21 Unicode15 Character
capture. Generic Character names use the complete pinned runtime name resource,
recorded uppercase mappings and Java's algorithmic fallback. Data provenance and
JDK/Unicode notices accompany the Stage76 evidence and this directory's
`JDK-CHARACTER-NAMES.md` / `jdk-character-names-legal` files.

Recorded isolated validation includes117 vendor unit tests and candidate10's
bounded389 Java-derived Regex/String/singleton queries, plus lifecycle/cache and
small-stack controls. These are separate from the Stage77 three original frontend
cases, Stage78 numeric72, and older M1raw56 controls. They do not establish full
Pkl/JDK regex, public API, target-platform or performance compatibility.

Known unsupported/grapheme/canonical-equivalence and remaining parser/matcher
edge cases retain explicit failures. No historical M1 platform6078 mismatch is
reclassified without a fresh actual comparison. Canonical required checks,
Native/Node/Chromium captures and complete ready-corpus replay remain separate.
