#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
"""M7 Seeded Differential Fuzzer & Generative Corpus against pinned Java Pkl 0.32.0.

Generates a deterministic, seeded corpus (SplitMix64 PRNG) spanning 7 language & tool categories:
  1. expressions-numerics       : arithmetic, bitwise, comparisons, short-circuit logic, ??, !!, ?.,
                                  if/else, let, pipe |>, Int64 edge values, Float NaN/Inf/-0.0
  2. strings-regex-glob         : single/multiline/custom-delimiter strings, escapes, unicode/emoji,
                                  interpolations, string methods, Regex & glob patterns
  3. objects-amendments-generators : Dynamic, amendments, class/open/abstract/extends, hidden/local/
                                  fixed/const, for/when generators, spreads, member predicates
  4. collections-units          : List/Set/Map/Listing/Mapping/Pair/IntSeq/Bytes/Duration/DataSize,
                                  structural equality & hashing, collection pipelines
  5. types-closures             : type annotations, unions, defaults, nullable, constrained types,
                                  generic typealiases, lambdas/closures, is/as checks
  6. multi-format-rendering     : pcf, json, yaml, xml, properties, plist, and pkl-binary
                                  (raw byte-for-byte parity + Rust decode round-trip check)
  7. formatter-fuzz             : `pkl format -` raw byte parity vs Java 0.32.0 + 2nd-pass idempotency

Honesty & integrity invariants:
  - Compares raw stdout, stderr, and exit_code byte-for-byte with zero trimming/normalization.
  - Uses pinned Eclipse Adoptium Temurin 21.0.13.1+2-LTS (`exact_jdk_lock_match: true`, NO PKL_ALLOW_LOCAL_JDK).
  - Automatically delta-shrinks any discrepancy into minimized reproducers (optionally saved under
    `tests/fixtures/m7/fuzz-regressions/`).
  - Supports replaying saved regression fixtures (`--replay-regressions`).
  - Seals all generated inputs, raw captures, and verdicts under SHA-256 in `report.json` with
    strictly separated `--verify-report` and `--verify-strict`.
"""
from __future__ import annotations

import argparse
from collections import Counter
from concurrent.futures import ThreadPoolExecutor
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import signal
import subprocess
import sys
import time

import compat
import oracle

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_SEED = 0x4D37504B4C303332  # "M7PKL032"
DEFAULT_COUNT = 210
DEFAULT_WORKERS = 4
DEFAULT_TIMEOUT = 30.0
DEFAULT_MAX_SHRINK_STEPS = 48

ALL_CATEGORIES = (
    "expressions-numerics",
    "strings-regex-glob",
    "objects-amendments-generators",
    "collections-units",
    "types-closures",
    "multi-format-rendering",
    "formatter-fuzz",
)
RENDER_FORMATS = ("pcf", "json", "yaml", "xml", "properties", "plist", "pkl-binary")
DECODE_PROBE_EXAMPLE = "m3_pklbinary_decode_probe"


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def evidence_paths(directory: Path):
    for current, dirs, files in os.walk(directory, followlinks=False):
        dirs.sort()
        for name in dirs:
            p = Path(current) / name
            if p.is_symlink():
                raise ValueError(f"evidence directory contains symlink: {p}")
        for name in sorted(files):
            p = Path(current) / name
            if p.is_symlink() or not p.is_file():
                raise ValueError(f"evidence must contain regular files: {p}")
            if p != directory / "report.json":
                yield p


def snapshot_sources(root: Path) -> dict[str, str]:
    tracked_dirs = (
        "crates/pkl-runtime/src",
        "crates/pkl-runtime/examples",
        "crates/pkl-host-native/src",
        "crates/pkl/src",
        "crates/pkl-cli/src",
        "crates/pkl-formatter/src",
    )
    tracked_files = (
        "Cargo.toml",
        "Cargo.lock",
        "rust-toolchain.toml",
        "oracle.lock.json",
        "tools/m7_fuzz.py",
    )
    out: dict[str, str] = {}
    for rel in tracked_files:
        p = root / rel
        if p.is_file():
            out[rel] = oracle.sha256(p)
    for drel in tracked_dirs:
        d = root / drel
        if not d.is_dir():
            continue
        for current, dirs, files in os.walk(d, followlinks=False):
            dirs[:] = sorted(x for x in dirs if x != "__pycache__")
            for name in sorted(files):
                p = Path(current) / name
                if p.is_file() and not p.is_symlink():
                    out[p.relative_to(root).as_posix()] = oracle.sha256(p)
    return dict(sorted(out.items()))


# ---- Deterministic SplitMix64 PRNG ----

class SplitMix64:
    """Explicit 64-bit SplitMix64 PRNG for 100% cross-version determinism."""

    MASK = (1 << 64) - 1

    def __init__(self, seed: int) -> None:
        self.state = seed & self.MASK

    def next_u64(self) -> int:
        self.state = (self.state + 0x9E3779B97F4A7C15) & self.MASK
        z = self.state
        z = ((z ^ (z >> 30)) * 0xBF58476D1CE4E5B9) & self.MASK
        z = ((z ^ (z >> 27)) * 0x94D049BB133111EB) & self.MASK
        return (z ^ (z >> 31)) & self.MASK

    def randrange(self, bound: int) -> int:
        if bound <= 0:
            raise ValueError("bound must be positive")
        return self.next_u64() % bound

    def randint(self, lo: int, hi: int) -> int:
        return lo + self.randrange(hi - lo + 1)

    def choice(self, seq):
        return seq[self.randrange(len(seq))]

    def chance(self, num: int, den: int) -> bool:
        return self.randrange(den) < num


# ---- Generative Pkl Corpus (7 categories) ----

INT64_EDGES = (
    "0",
    "1",
    "-1",
    "2",
    "-2",
    "42",
    "-42",
    "2147483647",
    "-2147483648",
    "9223372036854775807",
    "(-9223372036854775807 - 1)",
)

FLOAT_EDGES = (
    "0.0",
    "-0.0",
    "1.5",
    "-3.25",
    "1e12",
    "1e-12",
    "(1.0 / 0.0)",
    "(-1.0 / 0.0)",
    "(0.0 / 0.0)",
    "1.7976931348623157e308",
    "4.9e-324",
)


def _gen_expressions_numerics(rng: SplitMix64, idx: int) -> dict:
    variant = idx % 6
    a = rng.randint(-500, 500)
    b = rng.randint(-500, 500)
    c = rng.randint(1, 31)
    edge_i = rng.choice(INT64_EDGES)
    edge_i2 = rng.choice(INT64_EDGES)
    edge_f = rng.choice(FLOAT_EDGES)
    edge_f2 = rng.choice(FLOAT_EDGES)

    if variant == 0:
        op1 = rng.choice(("+", "-", "*"))
        op2 = rng.choice(("+", "-", "*", "~/", "%"))
        d = rng.randint(1, 29) * rng.choice((-1, 1))
        src = (
            f"a = {a}\n"
            f"b = {b}\n"
            f"c = {d}\n"
            f"arith = (a {op1} b) {op2} c\n"
            f"powVal = {rng.randint(-5, 5)} ** {rng.randint(0, 6)}\n"
            f"fdiv = (a / c).toString()\n"
            f"bits_shl = ({a}).shl({c})\n"
            f"bits_shr = ({edge_i}).shr({c})\n"
            f"bits_ushr = ({edge_i}).ushr({c})\n"
            f"bits_logic = ({a}).and({b}).or({c}).xor({a + 7}).xor(-1)\n"
            f"radix = ({rng.randint(-10000, 10000)}).toRadixString({rng.choice((2, 8, 10, 16, 36))})\n"
            f"oddEven = ({a}).isOdd && !({a}).isEven\n"
        )
    elif variant == 1:
        # Int64 edge arithmetic & guarded boundary checks
        src = (
            f"e1 = {edge_i}\n"
            f"e2 = {edge_i2}\n"
            f"small1 = if (e1 > 1000000) 1000000 else if (e1 < -1000000) -1000000 else e1\n"
            f"small2 = if (e2 > 1000000) 1000000 else if (e2 < -1000000) -1000000 else e2\n"
            f"addSafe = small1 + small2\n"
            f"subSafe = small1 - small2\n"
            f"mulSafe = (small1 ~/ 1000) * (small2 ~/ 1000)\n"
            f"divSafe = if (e2 == 0 || (e1 == (-9223372036854775807 - 1) && e2 == -1)) 0 else e1 ~/ e2\n"
            f"remSafe = if (e2 == 0) 0 else e1 % e2\n"
            f"negSafe = if (e1 == (-9223372036854775807 - 1)) e1 else -e1\n"
            f"cmp = List(e1 == e2, e1 != e2, e1 < e2, e1 <= e2, e1 > e2, e1 >= e2)\n"
        )
    elif variant == 2:
        # Float NaN, Infinity, -0.0, comparisons & IEEE operations
        src = (
            f"f1 = {edge_f}\n"
            f"f2 = {edge_f2}\n"
            f"sum = (f1 + f2).toString()\n"
            f"diff = (f1 - f2).toString()\n"
            f"prod = (f1 * f2).toString()\n"
            f"quot = (f1 / f2).toString()\n"
            f"rem = (f1 % f2).toString()\n"
            f"flags = List(f1.isNaN, f1.isInfinite, f1.isFinite, f1.isPositive, f1.isNonZero)\n"
            f"cmp = List(f1 == f2, f1 != f2, f1 < f2, f1 <= f2, f1 > f2, f1 >= f2)\n"
            f"negZeroEq = (-0.0 == 0.0)\n"
            f"negZeroStr = (-0.0).toString()\n"
            f"toIntSafe = if (f1.isFinite && f1 > -1e15 && f1 < 1e15) f1.toInt() else -999\n"
        )
    elif variant == 3:
        # Null coalescing, optional chaining, non-null assertion, short-circuiting, let, pipe
        maybe_null = "null" if rng.chance(1, 2) else f'new {{ inner = "{a}" }}'
        src = (
            f"cond = {rng.choice(('true', 'false'))}\n"
            f"shortAnd = false && (throw(\"never\") == 1)\n"
            f"shortOr = true || (throw(\"never\") == 1)\n"
            f"obj = {maybe_null}\n"
            f"opt = obj?.inner ?? \"fallback-{b}\"\n"
            f"nonNull = if (obj != null) obj!!.inner else \"was-null\"\n"
            f"letPipe = let (x = {a}) let (y = x * 2 + {b}) y |> ((v) -> v - {c}) |> ((v) -> v.toString())\n"
            f"branch = if (cond) {a} + {b} else {a} - {b}\n"
        )
    elif variant == 4:
        # Nested let + pipe + ternary if-else + bitwise shift edges
        shift_amt = rng.choice((-65, -1, 0, 1, 63, 64, 65, 127))
        src = (
            f"s1 = ({edge_i}).shl({shift_amt})\n"
            f"s2 = ({edge_i}).shr({shift_amt})\n"
            f"s3 = ({edge_i}).ushr({shift_amt})\n"
            f"piped = {a} |> ((n) -> if (n > 0) n * 3 else -n + 1) |> ((n) -> n.isBetween(0, 1000))\n"
            f"chained = (null ?? (if ({a} > {b}) null else {a})) ?? {b}\n"
        )
    else:
        # Intentional error path (uncaught division by zero or overflow) to fuzz exact error diagnostics
        err_kind = rng.choice(("div0", "overflow", "nonnull"))
        if err_kind == "div0":
            src = f"a = {a}\nres = a ~/ (2 - 2)\n"
        elif err_kind == "overflow":
            src = "res = 9223372036854775807 + 1\n"
        else:
            src = f"x = null\nres = x!!\n"
    return {
        "id": f"fuzz-{idx:04d}-expr",
        "category": "expressions-numerics",
        "mode": "eval",
        "format": "pcf",
        "source": src,
    }


def _gen_strings_regex_glob(rng: SplitMix64, idx: int) -> dict:
    variant = idx % 5
    n = rng.randint(1, 50)
    sample_words = ("alpha", "Beta", "한글", "café", "🚀pk", "a_b-c", "foo/bar/baz.pkl", "  trimmed  ")
    w1 = rng.choice(sample_words)
    w2 = rng.choice(sample_words)

    if variant == 0:
        # Escapes, unicode code points, astral emoji, custom pound delimiters & interpolation
        src = (
            f's1 = "{w1}\\n\\t\\"{w2}\\"\\\\\\u{{1F600}}"\n'
            f's2 = #"raw \\n \\t #(not) \\#({n} + 1) {w1}"#\n'
            f's3 = ##"""\n'
            f'  line1: {w1}\n'
            f'  interp: \\##("{w2}" + "-{n}")\n'
            f'  quotes: """inside"""\n'
            f'  """##\n'
            f'lens = List(s1.length, s1.chars.length, s1.codePoints.length)\n'
            f'rev = s1.reverse()\n'
        )
    elif variant == 1:
        # String transformations, padding, slicing, digests, base64
        pad_len = rng.randint(2, 16)
        src = (
            f'raw = "{w1}-{w2}-{n}"\n'
            f'upper = raw.toUpperCase()\n'
            f'lower = raw.toLowerCase()\n'
            f'padded = raw.padStart({pad_len}, "*").padEnd({pad_len + 4}, ".")\n'
            f'parts = raw.split("-")\n'
            f'rep = raw.replaceAll("-", "::")\n'
            f'trimmed = "  \\t {w1} \\n ".trim()\n'
            f'sub = raw.substring(1, if (raw.length > 4) 4 else raw.length)\n'
            f'repRange = raw.replaceRange(0, if (raw.length >= 2) 2 else raw.length, "ZZ")\n'
            f'hashes = List(raw.md5, raw.sha256, raw.base64)\n'
        )
    elif variant == 2:
        # Regex matching, capture groups, flags (?i) (?m) (?s)
        pat = rng.choice((
            r"([a-z]+)-(\d+)",
            r"(?i)(beta|alpha)[_-]?(\d*)",
            r"(?m)^line(\d+):(.*)$",
            r"(?s)start.*end",
            r"\p{L}+",
        ))
        hay = rng.choice((
            f"alpha-{n} and beta-{n + 5}",
            f"line1:{w1}\nline2:{w2}\nline3:{n}",
            f"start\n{w1}\nend",
        ))
        hay_esc = hay.replace("\\", "\\\\").replace("\n", "\\n").replace('"', '\\"')
        src = (
            f'r = Regex(#"{pat}"#)\n'
            f'text = "{hay_esc}"\n'
            f'matches = r.findMatchesIn(text).map((m) -> List(m.value, m.start, m.end, m.groups.map((g) -> g?.value)))\n'
            f'entire = r.matchEntire(text)?.value\n'
            f'replaced = text.replaceAll(r, "[$0]")\n'
        )
    elif variant == 3:
        # Glob pattern classification and regex matching edge cases
        g1 = rng.choice(("*.pkl", "a/**/b?.pkl", "plain.txt", "[a-z]*.pkl", "{foo,bar}.pkl", r"\*.pkl", "dir/[!0-9].txt"))
        g2 = rng.choice(("normal_file.pkl", "a*b", "foo?bar", "x[1]y", "a\\b"))
        src = (
            f'g1 = #"{g1}"#\n'
            f'g2 = #"{g2}"#\n'
            f'isGlob1 = g1.isGlobPattern\n'
            f'isGlob2 = g2.isGlobPattern\n'
            f'contains = g1.contains("pkl")\n'
            f'startsEnds = List(g1.startsWith("a"), g1.endsWith(".pkl"))\n'
            f'matchesRe = "{w1}".matches(Regex(#"[\\p{{L}}\\p{{So}}/_ .-]+"#))\n'
        )
    else:
        # Invalid regex or out-of-bounds string error path (tested raw against Java!)
        bad_kind = rng.choice(("bad-regex", "bad-substring", "bad-pad"))
        if bad_kind == "bad-regex":
            src = 'r = Regex("[unclosed")\nx = r.pattern\n'
        elif bad_kind == "bad-substring":
            src = f'x = "{w1}".substring(50, 60)\n'
        else:
            src = f'x = "{w1}".padStart(10, "")\n'
    return {
        "id": f"fuzz-{idx:04d}-str",
        "category": "strings-regex-glob",
        "mode": "eval",
        "format": "pcf",
        "source": src,
    }


def _gen_objects_amendments_generators(rng: SplitMix64, idx: int) -> dict:
    variant = idx % 5
    n = rng.randint(1, 20)
    flag = rng.choice(("true", "false"))

    if variant == 0:
        # Classes, inheritance, open/abstract/hidden/local/fixed/const + amendments
        src = (
            f"abstract class Base {{\n"
            f"  fixed kind: String = \"base\"\n"
            f"  hidden secret: Int = {n}\n"
            f"  name: String\n"
            f"  score: Int = secret * 2\n"
            f"}}\n"
            f"open class Mid extends Base {{\n"
            f"  const factor: Int = {rng.randint(2, 5)}\n"
            f"  local bonus = factor + 1\n"
            f"  total: Int = score + bonus\n"
            f"}}\n"
            f"class Leaf extends Mid {{\n"
            f"  extra: String = \"\\(kind):\\(name):\\(total)\"\n"
            f"}}\n"
            f"baseInst = new Leaf {{ name = \"n-{n}\" }}\n"
            f"amended = (baseInst) {{\n"
            f"  name = \"amended-{n}\"\n"
            f"  score = 100 + {n}\n"
            f"}}\n"
            f"result = List(baseInst.extra, amended.extra, amended.total)\n"
        )
    elif variant == 1:
        # for & when generators (nested, else branch) + spreads (list, nullable, map, dynamic)
        spread_null = "null" if rng.chance(1, 2) else f"List({n}, {n + 1})"
        src = (
            f"local xs = List(1, 2, 3, 4, 5)\n"
            f"local maybe = {spread_null}\n"
            f"local baseMap = Map(\"k1\", {n}, \"k2\", {n + 2})\n"
            f"dyn = new Dynamic {{\n"
            f"  for (x in xs) {{\n"
            f"    when (x % 2 == 0) {{\n"
            f"      x * 10 + {n}\n"
            f"    }} else {{\n"
            f"      x + {n}\n"
            f"    }}\n"
            f"  }}\n"
            f"  ...xs\n"
            f"  ...?maybe\n"
            f"  ...baseMap\n"
            f"}}\n"
            f"listing = new Listing {{\n"
            f"  when ({flag}) {{\n"
            f"    for (i, v in xs) {{\n"
            f"      i * 100 + v\n"
            f"    }}\n"
            f"  }} else {{\n"
            f"    -1\n"
            f"  }}\n"
            f"}}\n"
            f"mapping = new Mapping {{\n"
            f"  for (k, v in baseMap) {{\n"
            f"    [\"p_\" + k] = v * 2\n"
            f"  }}\n"
            f"}}\n"
        )
    elif variant == 2:
        # Member predicates on Listing & Mapping & nested amendment
        thresh = rng.randint(2, 4)
        src = (
            f"baseMap: Mapping<String, Dynamic> = new {{\n"
            f"  [\"alpha\"] {{ v = {n}; active = true }}\n"
            f"  [\"apricot\"] {{ v = {n + 1}; active = false }}\n"
            f"  [\"beta\"] {{ v = {n + 2}; active = true }}\n"
            f"}}\n"
            f"amendedMap = (baseMap) {{\n"
            f"  [[this.active]] {{\n"
            f"    v = {n + 100}\n"
            f"    tagged = true\n"
            f"  }}\n"
            f"}}\n"
            f"baseList: Listing<Int> = new {{ 1; 2; 3; 4; 5 }}\n"
            f"amendedList = (baseList) {{\n"
            f"  [[this >= {thresh}]] = {thresh * 10}\n"
            f"}}\n"
        )
    elif variant == 3:
        # Lexical outer/this scoping in nested object amendments
        src = (
            f"outerVal = {n}\n"
            f"container {{\n"
            f"  local c = outerVal + 5\n"
            f"  tag = \"c-\\(c)\"\n"
            f"  child {{\n"
            f"    local d = c + 1\n"
            f"    fromOuter = outer.tag\n"
            f"    fromModule = module.outerVal\n"
            f"    val = d * 2\n"
            f"  }}\n"
            f"}}\n"
            f"amended = (container) {{\n"
            f"  tag = \"override-{n}\"\n"
            f"  child {{\n"
            f"    val = outerVal * 10 + 7\n"
            f"  }}\n"
            f"}}\n"
        )
    else:
        # Intentional error: overriding fixed, instantiating abstract, or duplicate mapping key
        ek = rng.choice(("fixed", "abstract", "dupkey"))
        if ek == "fixed":
            src = "class A { fixed x: Int = 1 }\na = new A { x = 2 }\n"
        elif ek == "abstract":
            src = "abstract class A { x: Int = 1 }\na = new A {}\n"
        else:
            src = f"m = new Mapping {{ [\"k\"] = {n}; [\"k\"] = {n + 1} }}\n"
    return {
        "id": f"fuzz-{idx:04d}-obj",
        "category": "objects-amendments-generators",
        "mode": "eval",
        "format": "pcf",
        "source": src,
    }


def _gen_collections_units(rng: SplitMix64, idx: int) -> dict:
    variant = idx % 5
    a = rng.randint(1, 15)
    b = rng.randint(1, 15)
    c = rng.randint(-10, 10)

    if variant == 0:
        # List, Set, Map, Pair, IntSeq pipelines
        step = rng.choice((1, 2, 3))
        src = (
            f"seq = IntSeq({c}, {c + 12}).step({step})\n"
            f"xs = seq.toList()\n"
            f"mapped = xs.map((n) -> n * 2 + {a})\n"
            f"filtered = mapped.filter((n) -> n % 3 != 0)\n"
            f"flat = xs.take(4).flatMap((n) -> List(n, -n))\n"
            f"grouped = flat.groupBy((n) -> if (n >= 0) \"pos\" else \"neg\")\n"
            f"sorted = flat.sortBy((n) -> -n)\n"
            f"zipped = xs.take(3).zip(List(\"a\", \"b\", \"c\"))\n"
            f"folded = filtered.fold(0, (acc, v) -> acc + v)\n"
            f"joined = xs.take(5).join(\",\")\n"
            f"distinct = List({a}, {b}, {a}, {b}, {a}).distinct\n"
        )
    elif variant == 1:
        # Structural equality & hashing of composite values in Set & Map
        src = (
            f"p1 = Pair({a}, \"x\")\n"
            f"p2 = Pair({a}, \"x\")\n"
            f"d1 = new Dynamic {{ x = {a}; y = List(1, 2) }}\n"
            f"d2 = new Dynamic {{ x = {a}; y = List(1, 2) }}\n"
            f"b1 = Bytes({a % 256}, {b % 256}, 255)\n"
            f"b2 = Bytes({a % 256}, {b % 256}, 255)\n"
            f"eqChecks = List(p1 == p2, d1 == d2, b1 == b2, List(1, 2) == List(1, 2), Set(1, 2) == Set(2, 1))\n"
            f"dedupSet = Set(p1, p2, d1, d2, b1, b2)\n"
            f"compositeMap = Map(p1, \"pair\", d1, \"dyn\", b1, \"bytes\")\n"
            f"lookups = List(compositeMap[p2], compositeMap[d2], compositeMap[b2], dedupSet.length)\n"
        )
    elif variant == 2:
        # Duration & DataSize conversions, arithmetic, comparisons
        d_unit = rng.choice(("ns", "us", "ms", "s", "min", "h", "d"))
        s_unit = rng.choice(("b", "kb", "kib", "mb", "mib", "gb", "gib"))
        src = (
            f"dur1 = {a}.{d_unit}\n"
            f"dur2 = {b}.ms\n"
            f"durOps = List((dur1 + dur2).toUnit(\"ms\"), (dur1 * 2).toUnit(\"s\"), dur1 > dur2, dur1.isoString)\n"
            f"sz1 = {a}.{s_unit}\n"
            f"sz2 = {b}.kib\n"
            f"szOps = List((sz1 + sz2).toUnit(\"b\"), sz1.toBinaryUnit().toString(), sz1.toDecimalUnit().toString(), sz1 >= sz2)\n"
        )
    elif variant == 3:
        # Bytes & Listing/Mapping conversions
        src = (
            f"bs = Bytes({a % 256}, {(a + 17) % 256}, {(b * 7) % 256}, 0, 127, 128, 255)\n"
            f"bInfo = List(bs.length, bs.hex, bs.base64, bs.md5, bs.sha256, bs.toList())\n"
            f"lst: Listing<Int> = new {{ {a}; {b}; {a + b} }}\n"
            f"mp: Mapping<String, Int> = new {{ [\"a\"] = {a}; [\"b\"] = {b} }}\n"
            f"conv = List(lst.toList(), lst.toSet(), mp.toMap(), mp.keys, mp.toMap().values)\n"
            f"rtList = lst.toList().toListing()\n"
            f"rtMap = mp.toMap().toMapping()\n"
        )
    else:
        # Edge case: IntSeq descending / empty / step arithmetic
        step = rng.choice((-2, -1, 1, 2))
        src = (
            f"s1 = IntSeq({a + 5}, {a}).step({step})\n"
            f"s2 = IntSeq({a}, {a + 5}).step({step})\n"
            f"s3 = IntSeq({a}, {a}).step({step})\n"
            f"result = List(s1.toList(), s2.toList(), s3.toList(), s1 == s2, s1.start, s1.end, s1.step)\n"
        )
    return {
        "id": f"fuzz-{idx:04d}-coll",
        "category": "collections-units",
        "mode": "eval",
        "format": "pcf",
        "source": src,
    }


def _gen_types_closures(rng: SplitMix64, idx: int) -> dict:
    variant = idx % 5
    n = rng.randint(1, 50)

    if variant == 0:
        # Unions, string-literal unions, default unions, nullable, constrained types
        mode = rng.choice(('"fast"', '"slow"', '"auto"'))
        src = (
            f'typealias Mode = *"auto" | "fast" | "slow"\n'
            f'typealias PosInt = Int(this > 0, this < 1000)\n'
            f'typealias NonEmptyStr = String(!isEmpty, length <= 32)\n'
            f'class Config {{\n'
            f'  mode: Mode\n'
            f'  retries: PosInt | *"unlimited"\n'
            f'  label: NonEmptyStr? = null\n'
            f'}}\n'
            f'c1: Config = new {{}}\n'
            f'c2: Config = new {{ mode = {mode}; retries = {n}; label = "cfg-{n}" }}\n'
            f'c3: Config = new {{ retries = "unlimited" }}\n'
            f'checks = List(c2.retries is PosInt, c3.retries is String, c1.label is NonEmptyStr?, null is PosInt?)\n'
        )
    elif variant == 1:
        # Parameterized stdlib generic typealiases + constraints + is/as
        src = (
            f'typealias IntBox = Mapping<String, List<Int(this >= 0)>>\n'
            f'typealias StrIntPairs = List<Pair<String, Int>>(!isEmpty)\n'
            f'b: IntBox = new {{\n'
            f'  ["evens"] = List(0, 2, 4, {n * 2})\n'
            f'  ["odds"] = List(1, 3, {n * 2 + 1})\n'
            f'}}\n'
            f'pl: StrIntPairs = List(Pair("a", {n}), Pair("b", {n + 1}))\n'
            f'castOk = ({n} as Int(this > 0)) + 1\n'
            f'isFail = (-5 is Int(this > 0))\n'
        )
    elif variant == 2:
        # Higher-order closures capturing let, module properties, and nested lambdas
        src = (
            f'base = {n}\n'
            f'local function makeAdder(k: Int): (Int) -> Int = (x) -> x + k + base\n'
            f'local function compose(f: (Int) -> Int, g: (Int) -> Int): (Int) -> Int = (x) -> g.apply(f.apply(x))\n'
            f'local add5 = makeAdder(5)\n'
            f'local dbl: (Int) -> Int = (x) -> x * 2\n'
            f'local f = compose(add5, dbl)\n'
            f'results = List(1, 2, 3, 4).map(f)\n'
            f'curried = let (m = {n % 7 + 1}) ((a) -> (b) -> a * m + b).apply(3).apply(4)\n'
        )
    elif variant == 3:
        # Type checking edge cases: union short-circuit, nullable, FunctionN, Any/Unknown
        src = (
            f'local vals: List<Any?> = List(null, {n}, {n}.5, "str", true, List(1), (x) -> x + 1)\n'
            f'kinds = vals.map((v) ->\n'
            f'  if (v is Null) "null"\n'
            f'  else if (v is Int(this > 25)) "big-int"\n'
            f'  else if (v is Int) "small-int"\n'
            f'  else if (v is Float) "float"\n'
            f'  else if (v is String) "str"\n'
            f'  else if (v is Boolean) "bool"\n'
            f'  else if (v is List) "list"\n'
            f'  else if (v is Function1) "fn1"\n'
            f'  else "other"\n'
            f')\n'
        )
    else:
        # Intentional type or constraint violation (tested raw byte-for-byte against Java!)
        fk = rng.choice(("union-fail", "constraint-fail", "generic-fail"))
        if fk == "union-fail":
            src = f'x: Int | Boolean = "nope-{n}"\n'
        elif fk == "constraint-fail":
            src = f'x: Int(this > 100 && this % 2 == 0) = {n}\n'
        else:
            src = f'xs: List<Int> = List(1, "bad-{n}", 3)\n'
    return {
        "id": f"fuzz-{idx:04d}-type",
        "category": "types-closures",
        "mode": "eval",
        "format": "pcf",
        "source": src,
    }


def _gen_multi_format_rendering(rng: SplitMix64, idx: int) -> dict:
    fmt = RENDER_FORMATS[idx % len(RENDER_FORMATS)]
    n = rng.randint(-100, 500)
    b_val = rng.choice(("true", "false"))
    text_sample = rng.choice((
        "simple-value",
        r'quotes \"and\" \\ slashes',
        "multi\\nline\\ttext",
        "unicode-한글-🚀",
        'xml<&>\'\\"chars',
    ))

    if fmt == "properties":
        # Properties renderer renders flat scalar key-value pairs cleanly
        src = (
            f'app_name = "{text_sample}"\n'
            f'app_port = {abs(n) + 1000}\n'
            f'app_enabled = {b_val}\n'
            f'app_ratio = {n}.25\n'
            f'app_tag = "b-{abs(n)}"\n'
        )
    elif fmt == "xml":
        src = (
            f'name = "{text_sample}"\n'
            f'count = {n}\n'
            f'enabled = {b_val}\n'
            f'nested {{\n'
            f'  child = "val-{abs(n)}"\n'
            f'  items = new Listing {{ 1; 2; 3 }}\n'
            f'}}\n'
        )
    elif fmt == "plist":
        src = (
            f'title = "{text_sample}"\n'
            f'num = {n}\n'
            f'ratio = {n}.5\n'
            f'flag = {b_val}\n'
            f'items = List(1, "two", {b_val})\n'
            f'dict = new Mapping {{ ["k1"] = {n}; ["k2"] = "{text_sample}" }}\n'
        )
    elif fmt == "pkl-binary":
        # Rich Pkl value graph exercising many PklBinary tags (Int, Float, Bool, Null, String,
        # Bytes, Duration, DataSize, Pair, IntSeq, Regex, List, Set, Map, Listing, Mapping, Object)
        src = (
            f'intVal = {n}\n'
            f'bigInt = 9223372036854775807\n'
            f'floatVal = {n}.75\n'
            f'boolVal = {b_val}\n'
            f'nullVal = null\n'
            f'strVal = "{text_sample}"\n'
            f'bytesVal = Bytes(0, 1, 127, 128, 255, {abs(n) % 256})\n'
            f'durVal = {abs(n) + 1}.ms\n'
            f'sizeVal = {abs(n) + 2}.kib\n'
            f'pairVal = Pair("k", {n})\n'
            f'reVal = Regex("[a-z]+")\n'
            f'listVal = List(1, "a", {b_val})\n'
            f'setVal = Set("x", "y", "x")\n'
            f'mapVal = Map("one", 1, "two", 2)\n'
            f'nested {{\n'
            f'  listing = new Listing {{ 10; 20 }}\n'
            f'  mapping = new Mapping {{ ["m"] = "{text_sample}" }}\n'
            f'}}\n'
        )
    else:
        # pcf, json, yaml
        include_null = rng.choice(("null", f'"{text_sample}"'))
        src = (
            f'title = "{text_sample}"\n'
            f'number = {n}\n'
            f'fraction = {n}.125\n'
            f'flag = {b_val}\n'
            f'maybe = {include_null}\n'
            f'items = List(1, 2, {n}, "{text_sample}")\n'
            f'mapping = new Mapping {{\n'
            f'  ["alpha"] = {n}\n'
            f'  ["beta"] = new Listing {{ "x"; "y-{abs(n)}" }}\n'
            f'}}\n'
            f'nested {{\n'
            f'  inner = "ok-{abs(n)}"\n'
            f'  empty = new Dynamic {{}}\n'
            f'}}\n'
        )

    return {
        "id": f"fuzz-{idx:04d}-fmt-{fmt}",
        "category": "multi-format-rendering",
        "mode": "eval",
        "format": fmt,
        "source": src,
    }


def _gen_formatter_fuzz(rng: SplitMix64, idx: int) -> dict:
    variant = idx % 6
    n = rng.randint(1, 99)

    if variant == 0:
        # Cramped operators, inconsistent indentation, comments, blank lines
        src = (
            f"// leading comment {n}\n\n\n"
            f"a={n}+2*3-1\n"
            f"   b  =  if(a>10)   a+1   else   a-1\n"
            f"c=let(x=a+1)x|>((v)->v*2)\n"
            f"/* block comment */\n"
            f"d   =   List(  1,2,  3,{n} )\n"
        )
    elif variant == 1:
        # Classes, annotations, doc comments, generics, typealiases, modifiers
        src = (
            f"/// Doc comment for mod {n}\n"
            f"@Deprecated{{message=\"old\"}}\n"
            f"open class   Item<out T> {{\n"
            f"  /// prop doc\n"
            f"    fixed   id:Int={n}\n"
            f"  name :  String  =  \"item-{n}\"\n"
            f"  function  compute( x : Int , y : Int ) : Int = x + y * id\n"
            f"}}\n"
            f"typealias   Box<A,B> = Mapping<A,List<B>>\n"
            f"inst=new Item<String>{{name=\"hi\"}}\n"
        )
    elif variant == 2:
        # Multiline strings, custom pound delimiters, interpolations, chained calls
        src = (
            f's1 = """\n'
            f'  hello {n}\n'
            f'    indented \\({n}+1)\n'
            f'  """\n'
            f's2=#"raw   {n}   \\#(1+2)"#\n'
            f'chained=List(1,2,3,4,5).filter((x)->x>1).map((x)->x*{n}).fold(0,(a,b)->a+b)\n'
        )
    elif variant == 3:
        # Generators, spreads, member predicates, nested amendments
        src = (
            f"obj{{\n"
            f"  for(k,v in Map(\"a\",1,\"b\",{n})){{\n"
            f"    when(v>0){{[k]=v*2}}else{{[k]=0}}\n"
            f"  }}\n"
            f"  ...List(1,2)\n"
            f"  [[this>0]]{{tagged=true}}\n"
            f"}}\n"
            f"am=(obj){{extra={n}}}\n"
        )
    elif variant == 4:
        # Shebang + module header + imports (unsorted/spaced) + trailing comments
        src = (
            f"#!/usr/bin/env pkl\n"
            f"module   demo.Mod{n}\n"
            f"import   \"pkl:math\"\n"
            f"import \"pkl:base\" as b\n"
            f"x = math.max({n}, 42) // trailing\n"
            f"y = b.List(1,2,3)   \n\n"
        )
    else:
        # Complex nested expressions / edge whitespace / occasional malformed input
        if rng.chance(1, 3):
            src = f"x = (1 + 2 * \n  y = {n}\n"
        else:
            src = (
                f"a = (1 + 2) * (3 - 4) ~/ 2\n"
                f"b = null ?? (if (true) \"a\" else \"b\")\n"
                f"m = new Mapping {{ [\"k1\"] = {n}; [\"k2\"] = List(1, 2, 3) }}\n"
                f"f = (x, y) -> let (z = x + y) z * z\n"
            )
    return {
        "id": f"fuzz-{idx:04d}-format",
        "category": "formatter-fuzz",
        "mode": "format",
        "format": "format",
        "source": src,
    }


GENERATORS = {
    "expressions-numerics": _gen_expressions_numerics,
    "strings-regex-glob": _gen_strings_regex_glob,
    "objects-amendments-generators": _gen_objects_amendments_generators,
    "collections-units": _gen_collections_units,
    "types-closures": _gen_types_closures,
    "multi-format-rendering": _gen_multi_format_rendering,
    "formatter-fuzz": _gen_formatter_fuzz,
}


def generate_corpus(seed: int, count: int, categories: list[str] | None = None) -> list[dict]:
    """Deterministically generate `count` Pkl fuzz cases stratified across `categories`."""
    cats = list(categories) if categories else list(ALL_CATEGORIES)
    if not cats:
        raise ValueError("categories must not be empty")
    rng = SplitMix64(seed)
    cases: list[dict] = []
    cat_counts = {cat: 0 for cat in cats}
    for idx in range(count):
        cat = cats[idx % len(cats)]
        cat_idx = cat_counts[cat]
        cat_counts[cat] = cat_idx + 1
        case = GENERATORS[cat](rng, cat_idx)
        # Preserve globally unique sequential ID across all categories
        suffix = case["id"].split("-", 2)[-1]
        case["id"] = f"fuzz-{idx:04d}-{suffix}"
        case["seed"] = f"0x{seed:016x}"
        case["source_sha256"] = digest(case["source"].encode("utf-8"))
        cases.append(case)
    return cases


def load_saved_regressions(reg_dir: Path) -> list[dict]:
    """Load saved minimized regression fixtures (`*.pkl` + `*.meta.json`) for replay."""
    if not reg_dir.is_dir():
        return []
    cases: list[dict] = []
    for pkl_path in sorted(reg_dir.glob("*.pkl")):
        meta_path = pkl_path.with_suffix(".meta.json")
        meta = json.loads(meta_path.read_bytes()) if meta_path.is_file() else {}
        src = pkl_path.read_text(encoding="utf-8")
        cases.append({
            "id": f"regression-{pkl_path.stem}",
            "category": meta.get("category", "expressions-numerics"),
            "mode": meta.get("mode", "eval"),
            "format": meta.get("format", "pcf"),
            "source": src,
            "seed": meta.get("seed", "regression"),
            "source_sha256": digest(src.encode("utf-8")),
            "regression_fixture": pkl_path.name,
        })
    return cases


# ---- Process execution & comparison ----

def capture_process(
    command: list[str],
    cwd: Path,
    env: dict[str, str],
    timeout: float,
    *,
    stdin_bytes: bytes | None = None,
) -> dict:
    before = time.perf_counter_ns()
    proc = subprocess.Popen(
        command,
        cwd=cwd,
        env=env,
        stdin=subprocess.PIPE if stdin_bytes is not None else subprocess.DEVNULL,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        start_new_session=True,
    )
    timed_out = False
    try:
        try:
            stdout, stderr = proc.communicate(input=stdin_bytes, timeout=timeout)
        except subprocess.TimeoutExpired:
            timed_out = True
            try:
                os.killpg(proc.pid, signal.SIGKILL)
            except (ProcessLookupError, PermissionError):
                pass
            try:
                stdout, stderr = proc.communicate(timeout=5)
            except subprocess.TimeoutExpired as err:
                stdout, stderr = err.output or b"", err.stderr or b""
                proc.wait(timeout=5)
    finally:
        try:
            os.killpg(proc.pid, signal.SIGKILL)
        except (ProcessLookupError, PermissionError):
            pass
    return {
        "stdout": stdout,
        "stderr": stderr,
        "exit_code": proc.returncode,
        "files": {},
        "timed_out": timed_out,
        "workspace_changes": {},
        "elapsed_ns": time.perf_counter_ns() - before,
    }


def java_prefix_for_state(prefix: list[str], state_dir: Path) -> list[str]:
    home = state_dir / "home"
    tmp = state_dir / "tmp"
    out = []
    for arg in prefix:
        if arg.startswith("-Duser.home="):
            out.append(f"-Duser.home={home}")
        elif arg.startswith("-Djava.io.tmpdir="):
            out.append(f"-Djava.io.tmpdir={tmp}")
        else:
            out.append(arg)
    return out


def build_state_env(base_env: dict[str, str], state_dir: Path) -> dict[str, str]:
    home = state_dir / "home"
    tmp = state_dir / "tmp"
    home.mkdir(parents=True, exist_ok=True)
    tmp.mkdir(parents=True, exist_ok=True)
    return {
        **base_env,
        "HOME": str(home),
        "TMPDIR": str(tmp),
        "TMP": str(tmp),
        "TEMP": str(tmp),
        "PYTHONDONTWRITEBYTECODE": "1",
    }


def classify_fuzz_case(
    case: dict,
    java_cap: dict,
    cand_cap: dict,
    *,
    pass2_cap: dict | None = None,
    decode_meta: dict | None = None,
) -> tuple[str, list[str]]:
    """Classify one fuzz case strictly from raw bytes (zero trimming/normalization)."""
    diffs: list[str] = []
    if java_cap.get("timed_out"):
        return "oracle-error", ["java-timeout"]
    if cand_cap.get("timed_out"):
        return "candidate-error", ["candidate-timeout"]

    raw_diffs = compat.differences(java_cap, cand_cap)
    if raw_diffs:
        return "mismatch", raw_diffs

    if case["mode"] == "format" and cand_cap.get("exit_code") in (0, 11) and cand_cap.get("stdout"):
        if pass2_cap is None:
            return "candidate-error", ["missing-format-pass2"]
        if pass2_cap.get("timed_out"):
            return "candidate-error", ["format-pass2-timeout"]
        if pass2_cap.get("exit_code") != 0:
            diffs.append(f"format-pass2-exit:{pass2_cap.get('exit_code')}")
        if pass2_cap.get("stderr") != b"":
            diffs.append("format-pass2-stderr-nonempty")
        if pass2_cap.get("stdout") != cand_cap.get("stdout"):
            diffs.append("format-pass2-stdout-differs")
        if diffs:
            return "format-not-idempotent", diffs

    if case["mode"] == "eval" and case.get("format") == "pkl-binary" and cand_cap.get("exit_code") == 0:
        if decode_meta is None:
            return "candidate-error", ["missing-pkl-binary-decode-check"]
        if not decode_meta.get("ok"):
            return "pkl-binary-decode-error", [f"decode:{decode_meta.get('error', 'unknown')}"]

    return "pass", []


# ---- Deterministic Delta-Shrinking / Minimization ----

def shrink_source(
    source: str,
    still_fails,
    *,
    max_steps: int = DEFAULT_MAX_SHRINK_STEPS,
) -> tuple[str, int]:
    """Delta-debug (`ddmin`) + line/token shrinker minimizing a failing Pkl reproducer.

    `still_fails(candidate_str) -> bool` returns True iff `candidate_str` reproduces a failure.
    Returns `(minimized_source, steps_used)`.
    """
    best = source
    steps = 0

    def try_candidate(cand: str) -> bool:
        nonlocal best, steps
        if cand == best or not cand.strip() or steps >= max_steps:
            return False
        steps += 1
        if still_fails(cand):
            best = cand
            return True
        return False

    # Phase 1: Line-level ddmin
    lines = best.splitlines(keepends=True)
    n = 2
    while len(lines) >= 2 and steps < max_steps:
        chunk_size = max(1, (len(lines) + n - 1) // n)
        chunks = [lines[i : i + chunk_size] for i in range(0, len(lines), chunk_size)]
        reduced = False
        # Try keeping a single chunk first
        for chunk in chunks:
            cand = "".join(chunk)
            if try_candidate(cand):
                lines = best.splitlines(keepends=True)
                n = 2
                reduced = True
                break
        if reduced:
            continue
        # Try removing each chunk
        for idx in range(len(chunks)):
            cand = "".join(c for j, chunk in enumerate(chunks) if j != idx for c in chunk)
            if try_candidate(cand):
                lines = best.splitlines(keepends=True)
                n = max(n - 1, 2)
                reduced = True
                break
        if not reduced:
            if n >= len(lines):
                break
            n = min(len(lines), n * 2)

    # Phase 2: Strip comment/blank lines
    if steps < max_steps:
        stripped = "".join(
            ln for ln in best.splitlines(keepends=True)
            if ln.strip() and not ln.lstrip().startswith("//")
        )
        try_candidate(stripped)

    # Phase 3: Single-line deletion fixed-point pass
    changed = True
    while changed and steps < max_steps:
        changed = False
        lines = best.splitlines(keepends=True)
        if len(lines) <= 1:
            break
        for idx in range(len(lines) - 1, -1, -1):
            if steps >= max_steps:
                break
            cand = "".join(lines[:idx] + lines[idx + 1 :])
            if try_candidate(cand):
                changed = True
                break

    # Phase 4: Simple within-line whitespace & argument trimming
    if steps < max_steps:
        compact = "\n".join(ln.rstrip() for ln in best.splitlines()) + "\n"
        try_candidate(compact)

    return best, steps


def verify_pkl_binary_decode_batch(
    decode_probe_bin: Path,
    work_dir: Path,
    payloads: dict[str, bytes],
) -> dict[str, dict]:
    """Decode emitted pkl-binary MessagePack payloads via recorded-build `m3_pklbinary_decode_probe`."""
    if not payloads:
        return {}
    work_dir.mkdir(parents=True, exist_ok=True)
    results: dict[str, dict] = {}
    for cid, raw in payloads.items():
        msg_path = work_dir / f"{compat.case_directory(cid)}.bin"
        msg_path.write_bytes(raw)
        proc = subprocess.run(
            [str(decode_probe_bin), str(msg_path)],
            capture_output=True,
            timeout=30,
        )
        if proc.returncode != 0:
            err_msg = proc.stderr.decode("utf-8", errors="replace").strip()[:400]
            results[cid] = {"ok": False, "error": f"decode_probe rc={proc.returncode}: {err_msg}"}
            continue
        try:
            decoded = json.loads(proc.stdout.decode("utf-8"))
        except (UnicodeDecodeError, json.JSONDecodeError) as err:
            results[cid] = {"ok": False, "error": f"invalid decode_probe JSON: {err}"}
            continue
        is_obj = isinstance(decoded, dict) and decoded.get("type") in ("module", "object")
        class_info = decoded.get("class") if isinstance(decoded, dict) else None
        props = decoded.get("properties") if isinstance(decoded, dict) else None
        results[cid] = {
            "ok": is_obj,
            "root_type": decoded.get("type") if isinstance(decoded, dict) else None,
            "class_name": (
                class_info.get("simpleName")
                if isinstance(class_info, dict)
                else (decoded.get("name") if isinstance(decoded, dict) else None)
            ),
            "property_count": len(props) if isinstance(props, list) else 0,
            "decoded_sha256": digest(proc.stdout),
            "error": None if is_obj else f"unexpected decoded root type: {decoded.get('type') if isinstance(decoded, dict) else type(decoded).__name__}",
        }
    return results


def build_candidate_binary(
    root: Path,
    output: Path,
    cargo_target_dir: Path | None,
    *,
    package: str,
    kind_flag: str,
    name: str,
    rel_built_path: str,
) -> tuple[Path, dict]:
    cmd = ["cargo", "build", "--release", "-p", package, kind_flag, name, "--locked"]
    env = dict(os.environ)
    if cargo_target_dir is not None:
        env["CARGO_TARGET_DIR"] = str(cargo_target_dir)
    tdir = cargo_target_dir if cargo_target_dir is not None else (root / "target")
    t0 = time.perf_counter_ns()
    proc = subprocess.run(cmd, cwd=root, env=env, capture_output=True, timeout=600)
    elapsed_ns = time.perf_counter_ns() - t0
    (output / f"build-{name}.stdout.bin").write_bytes(proc.stdout)
    (output / f"build-{name}.stderr.bin").write_bytes(proc.stderr)
    if proc.returncode != 0:
        raise RuntimeError(
            f"failed building {name} (rc={proc.returncode}): "
            f"{proc.stderr.decode('utf-8', errors='replace')[-2000:]}"
        )
    built = tdir / rel_built_path
    if not built.is_file():
        raise RuntimeError(f"built {name} binary missing: {built}")
    art_dir = output / "artifacts"
    art_dir.mkdir(parents=True, exist_ok=True)
    sealed = art_dir / name
    shutil.copy2(built, sealed)
    meta = {
        "name": name,
        "sha256": oracle.sha256(sealed),
        "bytes": sealed.stat().st_size,
        "source_binding": "recorded-build",
        "build": {
            "bin": name,
            "command": cmd,
            "exit_code": proc.returncode,
            "elapsed_ns": elapsed_ns,
            "stdout_sha256": digest(proc.stdout),
            "stderr_sha256": digest(proc.stderr),
        },
    }
    return sealed, meta


def _eval_one_pair(
    source_text: str,
    mode: str,
    fmt: str,
    work_dir: Path,
    oracle_meta: dict,
    pkl_bin: Path,
    timeout: float,
) -> tuple[dict, dict, dict | None]:
    """Run Java 0.32.0 and Rust `pkl` against identical staged file/stdin and return raw captures."""
    work_dir.mkdir(parents=True, exist_ok=True)
    java_state = work_dir / "java_state"
    rust_state = work_dir / "rust_state"
    java_env = build_state_env(oracle_meta["environment"], java_state)
    rust_env = build_state_env(oracle_meta["environment"], rust_state)
    java_prefix = java_prefix_for_state(oracle_meta["command"], java_state)

    if mode == "format":
        raw_in = source_text.encode("utf-8")
        java_cmd = [*java_prefix, "format", "-"]
        rust_cmd = [str(pkl_bin), "format", "-"]
        java_cap = capture_process(java_cmd, work_dir, java_env, timeout, stdin_bytes=raw_in)
        rust_cap = capture_process(rust_cmd, work_dir, rust_env, timeout, stdin_bytes=raw_in)
        pass2_cap = None
        if not rust_cap["timed_out"] and rust_cap["exit_code"] in (0, 11) and rust_cap["stdout"]:
            pass2_cap = capture_process(rust_cmd, work_dir, rust_env, timeout, stdin_bytes=rust_cap["stdout"])
        return java_cap, rust_cap, pass2_cap

    # Evaluation mode: stage identical file `input.pkl` in `work_dir`
    input_path = work_dir / "input.pkl"
    input_path.write_bytes(source_text.encode("utf-8"))
    eval_args = ["eval", "--no-project", "--no-cache", "--color=never"]
    if fmt and fmt != "pcf":
        eval_args.extend(["-f", fmt])
    eval_args.append("input.pkl")

    java_cmd = [*java_prefix, *eval_args]
    rust_cmd = [str(pkl_bin), *eval_args]
    java_cap = capture_process(java_cmd, work_dir, java_env, timeout)
    rust_cap = capture_process(rust_cmd, work_dir, rust_env, timeout)
    return java_cap, rust_cap, None


def execute(
    root: Path,
    output: Path,
    seed: int,
    count: int,
    categories: list[str],
    workers: int,
    timeout: float,
    max_shrink_steps: int,
    save_regressions_dir: Path | None,
    replay_regressions_dir: Path | None,
    cargo_target_dir: Path | None,
) -> dict:
    if output.exists() and any(output.iterdir()):
        raise ValueError(f"output directory already exists and is non-empty: {output}")
    output.mkdir(parents=True, exist_ok=True)

    sources_before = snapshot_sources(root)
    oracle_meta = oracle.ensure_oracle(root, fetch=False, profile="reference")
    compat.write_json(output / "oracle.json", oracle_meta)

    pkl_bin, cand_meta = build_candidate_binary(
        root,
        output,
        cargo_target_dir,
        package="pkl-cli",
        kind_flag="--bin",
        name="pkl",
        rel_built_path="release/pkl",
    )
    decode_probe_bin, decode_probe_meta = build_candidate_binary(
        root,
        output,
        cargo_target_dir,
        package="pkl-runtime",
        kind_flag="--example",
        name=DECODE_PROBE_EXAMPLE,
        rel_built_path=f"release/examples/{DECODE_PROBE_EXAMPLE}",
    )

    cases = generate_corpus(seed, count, categories)
    if replay_regressions_dir is not None:
        cases.extend(load_saved_regressions(replay_regressions_dir))

    work_root = output / ".work"
    work_root.mkdir(parents=True, exist_ok=True)

    def run_case(case: dict) -> tuple[dict, dict, dict, dict | None]:
        cdir = compat.case_directory(case["id"])
        j_cap, r_cap, p2_cap = _eval_one_pair(
            case["source"],
            case["mode"],
            case["format"],
            work_root / cdir,
            oracle_meta,
            pkl_bin,
            timeout,
        )
        return case, j_cap, r_cap, p2_cap

    try:
        with ThreadPoolExecutor(max_workers=max(1, workers)) as pool:
            raw_outcomes = list(pool.map(run_case, cases))

        # Batch-verify pkl-binary decode round-trip for all successful pkl-binary emissions
        bin_payloads = {
            case["id"]: r_cap["stdout"]
            for case, _j, r_cap, _p2 in raw_outcomes
            if case["mode"] == "eval"
            and case["format"] == "pkl-binary"
            and not r_cap["timed_out"]
            and r_cap["exit_code"] == 0
        }
        decode_results = verify_pkl_binary_decode_batch(decode_probe_bin, work_root / "decode_batch", bin_payloads)

        results: list[dict] = []
        minimized_regressions: list[dict] = []

        for case, j_cap, r_cap, p2_cap in raw_outcomes:
            cdir = compat.case_directory(case["id"])
            folder = output / "cases" / cdir
            folder.mkdir(parents=True, exist_ok=True)
            (folder / "input.pkl").write_bytes(case["source"].encode("utf-8"))
            j_stored = compat.store_capture(folder / "java", j_cap)
            r_stored = compat.store_capture(folder / "candidate", r_cap)
            p2_stored = compat.store_capture(folder / "candidate_pass2", p2_cap) if p2_cap is not None else None
            dec_meta = decode_results.get(case["id"])
            if dec_meta is not None:
                compat.write_json(folder / "decode_roundtrip.json", dec_meta)

            status, diffs = classify_fuzz_case(
                case, j_cap, r_cap, pass2_cap=p2_cap, decode_meta=dec_meta
            )

            minimized_info = None
            if status in ("mismatch", "format-not-idempotent", "pkl-binary-decode-error") and max_shrink_steps > 0:
                shrink_work = work_root / f"shrink_{cdir}"

                def _err_headline(cap: dict) -> str:
                    lines = cap.get("stderr", b"").decode("utf-8", errors="replace").splitlines()
                    return lines[1] if len(lines) > 1 and "Pkl Error" in lines[0] else (lines[0] if lines else "")

                orig_sig = (status, tuple(diffs), _err_headline(j_cap), _err_headline(r_cap))

                def predicate(cand_src: str) -> bool:
                    shutil.rmtree(shrink_work, ignore_errors=True)
                    sj, sr, sp2 = _eval_one_pair(
                        cand_src, case["mode"], case["format"], shrink_work, oracle_meta, pkl_bin, timeout
                    )
                    sdec = None
                    if case["mode"] == "eval" and case["format"] == "pkl-binary" and not sr["timed_out"] and sr["exit_code"] == 0:
                        sdec = verify_pkl_binary_decode_batch(
                            decode_probe_bin, shrink_work / "decode", {case["id"]: sr["stdout"]}
                        ).get(case["id"])
                    st, sdiffs = classify_fuzz_case(case, sj, sr, pass2_cap=sp2, decode_meta=sdec)
                    if st != status:
                        return False
                    # Preserve the original failure signature so shrinking does not jump to an unrelated error
                    cand_sig = (st, tuple(sdiffs), _err_headline(sj), _err_headline(sr))
                    return cand_sig == orig_sig

                min_src, shrink_steps = shrink_source(case["source"], predicate, max_steps=max_shrink_steps)
                shutil.rmtree(shrink_work, ignore_errors=True)
                mj, mr, mp2 = _eval_one_pair(
                    min_src, case["mode"], case["format"], shrink_work, oracle_meta, pkl_bin, timeout
                )
                min_dir = folder / "minimized"
                min_dir.mkdir(parents=True, exist_ok=True)
                (min_dir / "minimized.pkl").write_bytes(min_src.encode("utf-8"))
                compat.store_capture(min_dir / "java", mj)
                compat.store_capture(min_dir / "candidate", mr)
                if mp2 is not None:
                    compat.store_capture(min_dir / "candidate_pass2", mp2)
                minimized_info = {
                    "original_bytes": len(case["source"].encode("utf-8")),
                    "original_lines": len(case["source"].splitlines()),
                    "minimized_bytes": len(min_src.encode("utf-8")),
                    "minimized_lines": len(min_src.splitlines()),
                    "minimized_sha256": digest(min_src.encode("utf-8")),
                    "shrink_steps": shrink_steps,
                    "minimized_file": f"cases/{cdir}/minimized/minimized.pkl",
                }
                compat.write_json(min_dir / "summary.json", minimized_info)
                minimized_regressions.append({
                    "id": case["id"],
                    "category": case["category"],
                    "mode": case["mode"],
                    "format": case["format"],
                    "status": status,
                    "differences": diffs,
                    **minimized_info,
                })

                if save_regressions_dir is not None and not case.get("regression_fixture"):
                    save_regressions_dir.mkdir(parents=True, exist_ok=True)
                    stem = f"{seed:016x}_{case['id']}"
                    (save_regressions_dir / f"{stem}.pkl").write_bytes(min_src.encode("utf-8"))
                    compat.write_json(
                        save_regressions_dir / f"{stem}.meta.json",
                        {
                            "id": case["id"],
                            "seed": f"0x{seed:016x}",
                            "category": case["category"],
                            "mode": case["mode"],
                            "format": case["format"],
                            "status": status,
                            "differences": diffs,
                            "original_sha256": case["source_sha256"],
                            "minimized_sha256": minimized_info["minimized_sha256"],
                            "shrink_steps": shrink_steps,
                        },
                    )

            results.append({
                "id": case["id"],
                "category": case["category"],
                "mode": case["mode"],
                "format": case["format"],
                "seed": case["seed"],
                "directory": cdir,
                "source_sha256": case["source_sha256"],
                "status": status,
                "differences": diffs,
                "java": j_stored,
                "candidate": r_stored,
                "candidate_pass2": p2_stored,
                "decode_roundtrip": dec_meta,
                "minimized": minimized_info,
            })
    finally:
        shutil.rmtree(work_root, ignore_errors=True)

    sources_after = snapshot_sources(root)
    source_stability = sources_before == sources_after
    compat.write_json(output / "sources.snapshot.json", {"before": sources_before, "after": sources_after})

    status_counts = dict(sorted(Counter(r["status"] for r in results).items()))
    category_status_counts = {
        cat: dict(sorted(Counter(r["status"] for r in results if r["category"] == cat).items()))
        for cat in categories
    }
    pass_count = status_counts.get("pass", 0)
    mismatch_count = (
        status_counts.get("mismatch", 0)
        + status_counts.get("format-not-idempotent", 0)
        + status_counts.get("pkl-binary-decode-error", 0)
    )
    error_count = status_counts.get("oracle-error", 0) + status_counts.get("candidate-error", 0)

    git_head: str | None = None
    try:
        head_proc = subprocess.run(
            ["git", "-C", str(root), "rev-parse", "HEAD"],
            capture_output=True,
            text=True,
            check=False,
        )
        cand = head_proc.stdout.strip()
        if head_proc.returncode == 0 and len(cand) == 40:
            git_head = cand
    except OSError:
        git_head = None

    report = {
        "schema_version": 1,
        "source": {"git_head": git_head} if git_head else {},
        "suite": "m7-seeded-differential-fuzz",
        "baseline": "0.32.0",
        "prng_algorithm": "splitmix64",
        "seed": f"0x{seed:016x}",
        "requested_count": count,
        "total_count": len(results),
        "categories": categories,
        "host": {
            "platform": platform.platform(),
            "system": platform.system(),
            "machine": platform.machine(),
            "python": platform.python_version(),
        },
        "oracle": oracle_meta,
        "candidate": cand_meta,
        "decode_probe": decode_probe_meta,
        "sources_snapshot_sha256": oracle.sha256(output / "sources.snapshot.json"),
        "sources_file_count": len(sources_before),
        "source_stability": source_stability,
        "pass_count": pass_count,
        "mismatch_count": mismatch_count,
        "error_count": error_count,
        "status_counts": status_counts,
        "category_status_counts": category_status_counts,
        "minimized_regressions": minimized_regressions,
        "results": results,
    }
    report["evidence_sha256"] = {
        str(p.relative_to(output).as_posix()): oracle.sha256(p) for p in sorted(evidence_paths(output))
    }
    compat.write_json(output / "report.json", report)
    verify_report(output, strict=False)
    return report


def verify_report(directory: Path, *, strict: bool = False) -> dict:
    report = json.loads((directory / "report.json").read_bytes())
    if report.get("schema_version") != 1 or report.get("suite") != "m7-seeded-differential-fuzz":
        raise ValueError("unsupported M7 fuzz report schema_version or suite")
    if json.loads((directory / "oracle.json").read_bytes()) != report["oracle"]:
        raise ValueError("oracle.json differs from report.json")

    recorded_evidence = report.get("evidence_sha256", {})
    actual_files = {str(p.relative_to(directory).as_posix()) for p in evidence_paths(directory)}
    if not recorded_evidence or set(recorded_evidence) != actual_files:
        raise ValueError("evidence file set differs from recorded evidence_sha256")
    for rel, expected_sha in recorded_evidence.items():
        if oracle.sha256(directory / rel) != expected_sha:
            raise ValueError(f"evidence file modified: {rel}")

    src_snap = directory / "sources.snapshot.json"
    if not src_snap.is_file() or oracle.sha256(src_snap) != report.get("sources_snapshot_sha256"):
        raise ValueError("sources.snapshot.json missing or sha256 disagrees with report.json")
    snap_doc = json.loads(src_snap.read_bytes())
    if (snap_doc.get("before") == snap_doc.get("after")) != report.get("source_stability"):
        raise ValueError("recomputed source_stability disagrees with report.json")

    cand = report.get("candidate") or {}
    decode_probe = report.get("decode_probe") or {}
    for role_label, rec in (("candidate", cand), ("decode_probe", decode_probe)):
        cname = rec.get("name")
        crel = f"artifacts/{cname}"
        if not cname or crel not in recorded_evidence or rec.get("sha256") != recorded_evidence[crel]:
            raise ValueError(f"{role_label} artifact sha256 mismatch: {crel}")
        brec = rec.get("build") or {}
        for stream, key in (("stdout", "stdout_sha256"), ("stderr", "stderr_sha256")):
            brel = f"build-{cname}.{stream}.bin"
            if brel not in recorded_evidence or brec.get(key) != recorded_evidence[brel]:
                raise ValueError(f"{role_label} build {stream} sha256 mismatch: {brel}")

    results = report.get("results", [])
    if len(results) != report.get("total_count"):
        raise ValueError("total_count disagrees with results length")

    for row in results:
        expected_dir = compat.case_directory(row["id"])
        if row["directory"] != expected_dir:
            raise ValueError(f"unexpected case directory for {row['id']}")
        folder = directory / "cases" / expected_dir
        in_path = folder / "input.pkl"
        if not in_path.is_file() or oracle.sha256(in_path) != row["source_sha256"]:
            raise ValueError(f"input.pkl missing or sha256 mismatch for {row['id']}")
        j_cap = compat.load_capture(folder / "java")
        r_cap = compat.load_capture(folder / "candidate")
        if (row.get("java") or {}).get("exit_code") != j_cap.get("exit_code"):
            raise ValueError(f"java exit_code tampered for {row['id']}")
        if (row.get("candidate") or {}).get("exit_code") != r_cap.get("exit_code"):
            raise ValueError(f"candidate exit_code tampered for {row['id']}")
        p2_cap = compat.load_capture(folder / "candidate_pass2") if (folder / "candidate_pass2").is_dir() else None
        if (p2_cap is not None) != (row.get("candidate_pass2") is not None):
            raise ValueError(f"candidate_pass2 presence tampered for {row['id']}")
        dec_path = folder / "decode_roundtrip.json"
        dec_meta = json.loads(dec_path.read_bytes()) if dec_path.is_file() else None
        if dec_meta != row.get("decode_roundtrip"):
            raise ValueError(f"decode_roundtrip.json tampered for {row['id']}")
        status, diffs = classify_fuzz_case(row, j_cap, r_cap, pass2_cap=p2_cap, decode_meta=dec_meta)
        if row["status"] != status or row["differences"] != diffs:
            raise ValueError(
                f"recomputed status/differences disagree for {row['id']}: "
                f"{row['status']}/{row['differences']} vs {status}/{diffs}"
            )

    recomp_status = dict(sorted(Counter(r["status"] for r in results).items()))
    if recomp_status != report.get("status_counts"):
        raise ValueError("recomputed status_counts disagree with report.json")
    recomp_cats = {
        cat: dict(sorted(Counter(r["status"] for r in results if r["category"] == cat).items()))
        for cat in report["categories"]
    }
    if recomp_cats != report.get("category_status_counts"):
        raise ValueError("recomputed category_status_counts disagree with report.json")
    expected_pass = recomp_status.get("pass", 0)
    expected_mismatch = (
        recomp_status.get("mismatch", 0)
        + recomp_status.get("format-not-idempotent", 0)
        + recomp_status.get("pkl-binary-decode-error", 0)
    )
    expected_err = recomp_status.get("oracle-error", 0) + recomp_status.get("candidate-error", 0)
    if (
        report.get("pass_count") != expected_pass
        or report.get("mismatch_count") != expected_mismatch
        or report.get("error_count") != expected_err
    ):
        raise ValueError("summary pass/mismatch/error counts disagree with recomputed results")

    if strict:
        if not report["oracle"].get("exact_jdk_lock_match", False):
            raise ValueError("strict fuzz gate requires exact_jdk_lock_match == true")
        if cand.get("source_binding") != "recorded-build":
            raise ValueError("strict fuzz gate requires recorded-build candidate")
        if decode_probe.get("source_binding") != "recorded-build":
            raise ValueError("strict fuzz gate requires recorded-build decode_probe")
        if report.get("source_stability") is not True:
            raise ValueError("strict fuzz gate requires source_stability == true")
        if set(report.get("categories", [])) != set(ALL_CATEGORIES):
            raise ValueError(f"strict fuzz gate requires all 7 categories: {ALL_CATEGORIES}")
        if report["total_count"] <= 0:
            raise ValueError("strict fuzz gate requires at least one executed fuzz case")
        for cat in ALL_CATEGORIES:
            cat_rows = [r for r in results if r["category"] == cat]
            if not cat_rows:
                raise ValueError(f"strict fuzz gate requires >= 1 executed case in category: {cat}")
        for fmt in RENDER_FORMATS:
            ok_fmt = [
                r for r in results
                if r["category"] == "multi-format-rendering"
                and r["format"] == fmt
                and r["status"] == "pass"
                and (r.get("java") or {}).get("exit_code") == 0
                and (r.get("candidate") or {}).get("exit_code") == 0
            ]
            if not ok_fmt:
                raise ValueError(f"strict fuzz gate requires >= 1 exit-0 passing render for format: {fmt}")
        ok_decode = [
            r for r in results
            if r["format"] == "pkl-binary"
            and r["status"] == "pass"
            and (r.get("decode_roundtrip") or {}).get("ok") is True
        ]
        if not ok_decode:
            raise ValueError("strict fuzz gate requires >= 1 passing pkl-binary decode round-trip")
        ok_fmt_idem = [
            r for r in results
            if r["mode"] == "format"
            and r["status"] == "pass"
            and r.get("candidate_pass2") is not None
        ]
        if not ok_fmt_idem:
            raise ValueError("strict fuzz gate requires >= 1 passing formatter 2nd-pass idempotency check")
        if expected_mismatch != 0 or expected_err != 0 or expected_pass != report["total_count"]:
            bad = [r for r in results if r["status"] != "pass"]
            summary = ", ".join(f"{r['id']}={r['status']}({','.join(r['differences'])})" for r in bad[:10])
            raise ValueError(f"strict fuzz gate failed ({len(bad)} non-pass case(s)): {summary}")

    return report


def parse_seed(raw: str) -> int:
    return int(raw, 0)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    action = parser.add_mutually_exclusive_group(required=True)
    action.add_argument("--output", type=Path, help="fresh evidence directory for live fuzz sweep")
    action.add_argument("--verify-report", type=Path, help="verify integrity & recompute verdicts (preserves mismatches)")
    action.add_argument("--verify-strict", type=Path, help="verify integrity AND enforce 100% raw-byte pass M7 gate")
    parser.add_argument("--root", type=Path, default=ROOT)
    parser.add_argument("--seed", type=parse_seed, default=DEFAULT_SEED)
    parser.add_argument("--count", type=int, default=DEFAULT_COUNT)
    parser.add_argument("--category", action="append", choices=ALL_CATEGORIES)
    parser.add_argument("--workers", type=int, default=DEFAULT_WORKERS)
    parser.add_argument("--timeout", type=float, default=DEFAULT_TIMEOUT)
    parser.add_argument("--max-shrink-steps", type=int, default=DEFAULT_MAX_SHRINK_STEPS)
    parser.add_argument("--save-regressions", type=Path, help="directory to save minimized regression fixtures")
    parser.add_argument("--replay-regressions", type=Path, help="directory of saved regression fixtures to also replay")
    parser.add_argument("--record-only", action="store_true", help="exit 0 after sealing even if mismatches found")
    parser.add_argument("--cargo-target-dir", type=Path, help="Cargo target directory for recorded build")
    args = parser.parse_args()

    try:
        if args.output:
            cats = list(dict.fromkeys(args.category)) if args.category else list(ALL_CATEGORIES)
            report = execute(
                args.root.resolve(),
                args.output.resolve(),
                args.seed,
                args.count,
                cats,
                args.workers,
                args.timeout,
                args.max_shrink_steps,
                args.save_regressions.resolve() if args.save_regressions else None,
                args.replay_regressions.resolve() if args.replay_regressions else None,
                args.cargo_target_dir,
            )
            print(json.dumps({
                "suite": report["suite"],
                "seed": report["seed"],
                "total_count": report["total_count"],
                "pass_count": report["pass_count"],
                "mismatch_count": report["mismatch_count"],
                "error_count": report["error_count"],
                "status_counts": report["status_counts"],
                "category_status_counts": report["category_status_counts"],
                "minimized_count": len(report["minimized_regressions"]),
            }, sort_keys=True))
            if args.record_only:
                return 0
            verify_report(args.output.resolve(), strict=True)
            return 0
        target_dir = (args.verify_report or args.verify_strict).resolve()
        report = verify_report(target_dir, strict=bool(args.verify_strict))
        print(json.dumps({
            "suite": report["suite"],
            "seed": report["seed"],
            "total_count": report["total_count"],
            "pass_count": report["pass_count"],
            "mismatch_count": report["mismatch_count"],
            "error_count": report["error_count"],
            "status_counts": report["status_counts"],
            "category_status_counts": report["category_status_counts"],
        }, sort_keys=True))
        return 0
    except (OSError, ValueError, KeyError, RuntimeError, subprocess.SubprocessError) as error:
        print(f"M7 fuzz error: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
