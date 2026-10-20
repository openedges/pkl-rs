# M1 numeric and regular-expression experiment

The native experiment compares **6,078 primitive operations** with the pinned
Temurin 21 oracle used by Pkl 0.32.0. The current candidate matches 6,076; **two
regular-expression cases remain mismatches**. This establishes neither a complete
Pkl evaluator nor full numeric/Java Pattern compatibility. The complete raw
captures and per-case statuses are retained in
[`reports/m1/semantics-evidence.tar.gz`](../reports/m1/semantics-evidence.tar.gz);
[`semantics.json`](../reports/m1/semantics.json) summarizes the current evidence.
Platform execution is recorded separately in [platforms.json](../reports/m1/platforms.json); this
report's native counts must not be presented as browser or N-API results.

## Decisions and evidence

| Area | Decision | Native evidence |
| --- | --- | --- |
| Integer arithmetic | Share checked `i64` operations; preserve Java's `MIN_VALUE % -1 == 0` | 40/40 primitive outcomes match |
| `Double.toString` | Share decimal selection and Java layout; explicitly select the even decimal at exact ties | 4,128/4,128 byte strings match, including 4,096 generated bit patterns |
| Math | Preserve fdlibm operation order in shared Rust; reject unmodified modern libm as a blanket StrictMath implementation | 1,856/1,856 sampled mathematical bit results match after kernel corrections |
| Java Pattern | Select a shared Rust Java-dialect engine with **versioned Java 21 Unicode tables and UTF-16 matcher positions**; reject the present scalar-indexed dependency as a complete backend | 45/47 backend primitive cases match; 7/7 bounded UTF-16 cursor probes match; two required contracts remain unimplemented |

Pkl's pinned `AdditionNode`, `TruncatingDivisionNode`, `RemainderNode`,
`VmSafeMath`, `FloatNodes`, and `MathNodes` establish the numeric delegation.
`VmUtils.compilePattern`, `RegexNodes`, and `StringNodes` establish Pattern's
`UNICODE_CASE` default, matching, and Java replacement syntax. Consult these under
`external/pkl/pkl-core/src/main/java/org/pkl/core/` at the revision in
`references.lock`. The original sources are not modified.

`tools/java/M1SemanticsProbe.java` invokes the corresponding JDK primitives, not a
second Rust implementation. The integer probe uses explicit symbolic outcome
identifiers (`integerOverflow`, `divisionByZero`) agreed by its protocol. These
are **not Pkl diagnostic text**; `% 0` in the upstream helper throws a Java
arithmetic exception, whose eventual Pkl diagnostic remains outside this probe.
Full Pkl errors, stack frames, renderers, and mixed-type operator behavior need
later evaluator comparisons.

## Numeric findings

Rust's default shortest formatting differed from Java at two exact decimal ties:
`430fb5d1c70f9232` and `c28e032c5486eb40`. The adapter obtains the shortest digit
count, rounds to that precision using ties-to-even, enforces Java's minimum of
two significant digits for decimal selection, and uses Java's plain/scientific
thresholds. This also preserves negative zero, infinities, NaN text, the smallest
subnormals, and exponent spelling. The generated sample is not exhaustive over
binary64 and does not prove every decimal conversion.

Unmodified `libm 0.2.16` disagreed on 28 sampled cube roots and four sine/cosine
results. Porting the original fdlibm cube-root algorithm resolved the former.
Restoring the original sine polynomial evaluation order and cosine cancellation
compensation resolved the latter, while retaining the Rust libm argument
reducers. The reducers use checked Rust indexing and the 66-entry binary64 table
on every target. Thus the initial observation of discrepancies at very large
arguments did **not** establish that argument reduction itself caused them.

The probe compares `Double.doubleToLongBits` semantics: NaN is canonicalized by
that defined operation before emitting hexadecimal bits. This is an explicit
primitive contract, not post-hoc normalization of output differences. Negative
zero and every non-NaN result retain all 64 bits. Raw NaN payload propagation,
all StrictMath functions, dense rounding-boundary searches, mixed numeric types,
float-to-integer conversion errors, and mathematical performance targets remain
unverified. `exp`, `log`, `tan`, and `sqrt` currently use the pinned software
libm candidate; passing this sample is not a complete StrictMath guarantee.

## Regex findings and remaining contract

The [`java_regex 0.1.0` candidate](https://docs.rs/crate/java_regex/0.1.0) is pure
Rust, uses `alloc`, and supports lookarounds, backreferences, possessive
quantifiers, intersection, captures, and Java-like syntax. Its upstream JDK 25
compatibility claim does not establish compatibility with this project's JDK 21.
The project adapter corrects capture positions to UTF-16, renders tested
structured compile-error categories, reserves unknown alphabetic escapes while
tracking quoting/comments/scoped flags, and implements fallible Java replacement
expansion. The dependency's own replacement implementation silently accepts
several malformed references; the project wrapper is therefore necessary.

Two preserved counterexamples prevent adopting the candidate unchanged:

1. `\p{L}+` on U+1C89 followed by U+11F02 matches only U+11F02 in the JDK 21 oracle,
   while the dependency includes the newer U+1C89 character. A generic remedy
   requires a pinned Unicode 15 property/case database and an audit of every
   predicate. Excluding this one character would not implement that contract.
2. An empty pattern on `😀` produces matches at UTF-16 offsets **0, 1, 2** in
   Java; the candidate produces only **0, 2**. Merely converting scalar result
   offsets to UTF-16 cannot restore searches starting inside a surrogate pair.
   The matcher needs UTF-16 storage and positions, code-point-aware consuming
   nodes, and one-code-unit advancement after an empty match. Injecting synthetic
   empty results would be incorrect for lookarounds, captures, and replacements.

A separate `Utf16Input` proof implements code-point decoding and Java's find
progress rule without the scalar backend. Seven `cursor` probes compare empty
and dot matching on empty, ASCII, supplementary, mixed, and line-break inputs
against real Java Pattern results. All seven match, including the empty match at
offset 1 inside a surrogate pair. This is a bounded text/cursor experiment, not a
replacement general matcher; the original empty-pattern backend case still fails.
Seven additional quote/comment/scoped-flag and error-order cases exercise reserved
escape validation and all match.

The selected architecture is therefore a Rust matcher with those two contracts;
the current dependency is an experimental comparison backend. A reviewed fork
or independent implementation must expose its parsed group-name table, pin its
Unicode data, and permit UTF-16 cursor execution before full adoption. The current
replacement wrapper identifies unmatched named groups from ordinary source
name declarations; quoted/commented pseudo-declarations require that parsed name
table. Exotic compile-error precedence, unpaired surrogates, catastrophic
backtracking/cancellation, and the complete upstream regex inventory remain
unverified. No native PCRE/JVM fallback is introduced.

## Selected numeric primitive costs

The separate [cost report](../reports/m1/semantics-cost.json) and
[cost evidence archive](../reports/m1/semantics-cost-evidence.tar.gz) measure a
fixed 24-input mixture for four operations. All **96 raw result payloads matched
before timing began**, and every timed process repeated that byte check. Three
process forks per runtime alternated execution order, each warmed all operations
for at least five seconds and required stable final windows (20-second cap).
All six processes converged in 5.004–5.013 seconds; each then recorded 12
measurement windows of 49,152 calls per operation. Warmup and measurement samples,
checksums, commands, exact sources, executable identities, and the pinned
optimizing GraalVM profile are retained. Stable means both the first/last
four-window median drift and relative median absolute deviation are at most 10%.

| Primitive | Rust ns/call | Java ns/call | Observed Rust/Java ratio |
| --- | ---: | ---: | ---: |
| Decimal formatting | 313.50 | 51.36 | **6.103** |
| Cube root | 6.52 | 8.53 | 0.764 |
| Sine | 31.91 | 47.09 | 0.678 |
| Checked integer addition | 0.73 | 2.54 | 0.287 |

**The current formatter fails this selected primitive cost target.** Preserve
its tested Java decimal-selection/layout contract, but replace or optimize the
conversion/allocation path in M2/M3 and repeat byte verification before measuring.
The other operations' lower ratios do not offset the formatting regression.
The loop boundary includes dispatch/checksum work; formatting hashes the returned
ASCII characters, and integer inputs include overflow outcomes. These are
primitive observations, not Pkl evaluation, diagnostic construction, startup,
FFI, peak RSS, or product performance evidence. JIT convergence was assessed by
recorded timing windows, without a separate compiler-log proof.

```sh
cargo build -p pkl-runtime --example semantics_bench --release --locked
python3 tools/m1_semantics_bench.py --output target/m1/semantics-cost/new-run
python3 -m unittest discover -s tools/tests -p 'test_m1_semantics_bench.py' -v
python3 tools/evidence.py verify reports/m1/semantics-cost-evidence.tar.gz
```

## Reproduction and evidence contract

```sh
cargo build -p pkl-runtime --example semantics_probe --locked
python3 tools/m1_semantics.py --check
python3 tools/m1_semantics.py --output target/m1/semantics/new-run
cargo test -p pkl-runtime --test semantics --locked
python3 -m unittest discover -s tools/tests -p 'test_m1_semantics.py' -v
python3 tools/evidence.py verify reports/m1/semantics-evidence.tar.gz
```

The differential command currently exits **1**, preserving the two mismatches.
An existing output directory is rejected. The fixed manifest contains input hex,
operation, family, identifier, and the deterministic generator seed. Each process
receives the same complete TSV payload and oracle allowlisted environment. The
transport emits one lowercase UTF-8 hexadecimal payload per case; the runner
validates exact coverage, final delimiters, stdout/stderr, and exit status before
comparing complete decoded bytes. Source snapshots, dependency/toolchain hashes,
Java artifact/runtime identity, commands, and candidate executable hash accompany
the captures. Hex is lossless transport, not a semantic normalization.

`run01` retained 42 discrepancies, `run02` retained 40, `run04`/`run05` retained
seven, `run06` retained three, and `run07` retained two. `run08` and `run09`
expanded the inventory to 6,078 cases and retained those same two mismatches. **`run03` is inconclusive**:
a concurrent parser edit caused the rebuild to fail and an old runtime rlib was
linked. Its `INVALIDATED.json` excludes that run from source-attributed claims;
its original bytes are retained. The first runs used an archived temporary
runner; later runs use the maintained Cargo example.

The 56 selected Rust golden regressions were extracted from fresh **Java**
`run09` output, not from Rust. Their exact source and hashes are in
`tests/fixtures/m1/semantics/regressions.provenance.json`. They do not replace the
full 6,078-case run or hide its failures. Python regressions reject incomplete or
noncanonical transport, stale inventories, and reused evidence directories.

## Adapted code and licenses

The original source bytes and license are retained under
`tests/fixtures/m1/semantics/references/`. Sun's permissive notice is preserved in
the Rust implementation. No GPL OpenJDK implementation was copied.

| Original | SHA-256 |
| --- | --- |
| [fdlibm `s_cbrt.c`](https://netlib.org/fdlibm/s_cbrt.c) | `7ce1c7299e60b9b998393312b140fa8738d13f11d89b88dd47d5c2df81eadfbb` |
| [fdlibm `k_sin.c`](https://netlib.org/fdlibm/k_sin.c) | `75d391f3c124806ce14893cd038ae5dd10663e40430ffa174d5fddcb33012167` |
| [fdlibm `k_cos.c`](https://netlib.org/fdlibm/k_cos.c) | `841086c75288f3f9fef03bae2a9dc24e99c3cdb2035e170c69452013a7b5befc` |
| libm 0.2.16 `rem_pio2.rs` | `5a040ab09dc356bca6d57712327151ed43003e2bac156553e3eb87d1cdefd2d1` |
| libm 0.2.16 `rem_pio2_large.rs` | `986b5b9ad2c8fba828aee47bf198eab97fab243359ab03eb09783d98ca5847b5` |
| libm 0.2.16 `LICENSE.txt` | `3823dda7cf046602f4b4e77ec8e227863dc4736037cc85bb33d9f19febe16bb7` |

libm's code is MIT licensed, with the retained Sun notices for these algorithms.
`java_regex` is MIT OR Apache-2.0. Cargo.lock pins all transitive dependencies;
Unicode crate versions are evidence of the candidate's data, not an approved
substitute for the required JDK 21 Unicode data version. Copied reducers omit
unneeded quad-precision tables, libm-only build configuration and unsafe indexing;
the kernels use exact binary64 coefficient bit patterns and preserve evaluation
order. These bounded cost measurements establish no product performance or total-memory compatibility claim.
