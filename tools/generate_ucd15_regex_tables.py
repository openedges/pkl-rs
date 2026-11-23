#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
"""Generate all `crates/pkl-runtime/src/java_regex/` tables directly from official Unicode 15.0.0 UCD files.

Input files (official Unicode Consortium UCD 15.0.0, licensed under Unicode-3.0):
  https://www.unicode.org/Public/15.0.0/ucd/
    - UnicodeData.txt
    - NameAliases.txt
    - Blocks.txt
    - Scripts.txt
    - PropertyValueAliases.txt
    - PropList.txt
    - DerivedCoreProperties.txt
    - SpecialCasing.txt
    - CompositionExclusions.txt
    - auxiliary/GraphemeBreakProperty.txt
    - emoji/emoji-data.txt

No OpenJDK source files or binary resources (such as `uniName.dat`) are read by this generator.
Every generated binary table implements the public Unicode 15.0.0 specification and the public
Java SE 21 `java.lang.Character` / `java.util.regex.Pattern` specification.
"""
from __future__ import annotations

import argparse
from collections import defaultdict
import hashlib
import json
from pathlib import Path
import struct

MAX_CP = 0x10FFFF

# General_Category numeric IDs matching `crates/pkl-runtime/src/java_regex/unicode.rs`.
CATEGORY_ID: dict[str, int] = {
    "Cn": 0,
    "Lu": 1,
    "Ll": 2,
    "Lt": 3,
    "Lm": 4,
    "Lo": 5,
    "Mn": 6,
    "Me": 7,
    "Mc": 8,
    "Nd": 9,
    "Nl": 10,
    "No": 11,
    "Zs": 12,
    "Zl": 13,
    "Zp": 14,
    "Cc": 15,
    "Cf": 16,
    "Co": 18,
    "Cs": 19,
    "Pd": 20,
    "Ps": 21,
    "Pe": 22,
    "Pc": 23,
    "Po": 24,
    "Sm": 25,
    "Sc": 26,
    "Sk": 27,
    "So": 28,
    "Pi": 29,
    "Pf": 30,
}

# Java SE public `Character.UnicodeBlock` historical canonical enum names that differ
# from Unicode 15.0.0 `Blocks.txt` names (per Java SE 21 Character.UnicodeBlock Javadoc).
JAVA_CANONICAL_BLOCK_RENAMES: dict[str, str] = {
    "GREEK_AND_COPTIC": "GREEK",
    "COMBINING_DIACRITICAL_MARKS_FOR_SYMBOLS": "COMBINING_MARKS_FOR_SYMBOLS",
    "CYRILLIC_SUPPLEMENT": "CYRILLIC_SUPPLEMENTARY",
}

# GraphemeBreakProperty numeric IDs matching `crates/pkl-runtime/src/java_regex/canon_eq.rs`.
GRAPHEME_ID: dict[str, int] = {
    "Other": 0,
    "CR": 1,
    "LF": 2,
    "Control": 3,
    "Extend": 4,
    "ZWJ": 5,
    "Regional_Indicator": 6,
    "Prepend": 7,
    "SpacingMark": 8,
    "L": 9,
    "V": 10,
    "T": 11,
    "LV": 12,
    "LVT": 13,
    "Extended_Pictographic": 14,
}


def parse_range_property_file(path: Path) -> dict[str, list[tuple[int, int]]]:
    """Parse standard UCD `XXXX..YYYY ; Property` files."""
    out: dict[str, list[tuple[int, int]]] = defaultdict(list)
    for raw in path.read_text(encoding="utf-8").splitlines():
        line = raw.split("#", 1)[0].strip()
        if not line:
            continue
        parts = [p.strip() for p in line.split(";")]
        if len(parts) < 2:
            continue
        rng, prop = parts[0], parts[1]
        if ".." in rng:
            a, b = rng.split("..", 1)
            lo, hi = int(a, 16), int(b, 16)
        else:
            lo = hi = int(rng, 16)
        out[prop].append((lo, hi))
    return out


def expand_ranges(ranges: list[tuple[int, int]]) -> set[int]:
    result: set[int] = set()
    for lo, hi in ranges:
        result.update(range(lo, hi + 1))
    return result


def normalize_enum_name(ucd_name: str) -> str:
    return ucd_name.upper().replace(" ", "_").replace("-", "_")


def parse_name_aliases(path: Path) -> dict[int, dict[str, list[str]]]:
    """Parse UCD NameAliases.txt -> `{cp: {alias_type: [names...]}}`."""
    aliases: dict[int, dict[str, list[str]]] = defaultdict(lambda: defaultdict(list))
    for raw in path.read_text(encoding="utf-8").splitlines():
        line = raw.split("#", 1)[0].strip()
        if not line:
            continue
        parts = [p.strip() for p in line.split(";")]
        if len(parts) >= 3:
            cp = int(parts[0], 16)
            aliases[cp][parts[2]].append(parts[1])
    return aliases


def generate_tables(ucd_dir: Path) -> dict[str, bytes]:
    # 1. Parse UnicodeData.txt & NameAliases.txt
    cat = [0] * (MAX_CP + 1)
    ccc = [0] * (MAX_CP + 1)
    mirrored = [False] * (MAX_CP + 1)
    simple_upper = list(range(MAX_CP + 1))
    simple_lower = list(range(MAX_CP + 1))
    simple_title = list(range(MAX_CP + 1))
    raw_canon_decomp: dict[int, list[int]] = {}
    explicit_names: dict[int, str] = {}
    name_aliases = parse_name_aliases(ucd_dir / "NameAliases.txt")

    range_start: tuple[int, list[str]] | None = None
    for raw in (ucd_dir / "UnicodeData.txt").read_text(encoding="utf-8").splitlines():
        if not raw.strip():
            continue
        fields = raw.split(";")
        cp = int(fields[0], 16)
        name = fields[1]
        gc = CATEGORY_ID[fields[2]]
        comb = int(fields[3])
        decomp = fields[5].strip()
        is_mir = fields[9] == "Y"
        unicode_1_name = fields[10].strip()
        up = int(fields[12], 16) if fields[12] else cp
        lo = int(fields[13], 16) if fields[13] else cp
        ti = int(fields[14], 16) if fields[14] else up

        if name.endswith(", First>"):
            range_start = (cp, fields)
            continue
        if name.endswith(", Last>"):
            assert range_start is not None
            start_cp, start_fields = range_start
            range_start = None
            r_gc = CATEGORY_ID[start_fields[2]]
            r_comb = int(start_fields[3])
            r_mir = start_fields[9] == "Y"
            for c in range(start_cp, cp + 1):
                cat[c] = r_gc
                ccc[c] = r_comb
                mirrored[c] = r_mir
            continue

        cat[cp] = gc
        ccc[cp] = comb
        mirrored[cp] = is_mir
        simple_upper[cp] = up
        simple_lower[cp] = lo
        simple_title[cp] = ti
        if not name.startswith("<"):
            explicit_names[cp] = name
        elif name == "<control>":
            # Per Unicode Standard & Java SE Character.getName/codePointOf:
            # C0/C1 control codes use Unicode_1_Name (field 10), with U+0007 taking its
            # NameAliases.txt abbreviation ("BEL") and C1 figments (U+0080, U+0081, U+0099)
            # taking their NameAliases.txt `figment` names. U+0084 has no formal name.
            if cp == 0x0007 and name_aliases[cp].get("abbreviation"):
                explicit_names[cp] = name_aliases[cp]["abbreviation"][0]
            elif unicode_1_name:
                explicit_names[cp] = unicode_1_name
            elif name_aliases[cp].get("figment"):
                explicit_names[cp] = name_aliases[cp]["figment"][0]
        if decomp and not decomp.startswith("<"):
            raw_canon_decomp[cp] = [int(tok, 16) for tok in decomp.split()]

    # 2. Parse PropList.txt, DerivedCoreProperties.txt, emoji-data.txt
    proplist = parse_range_property_file(ucd_dir / "PropList.txt")
    derived = parse_range_property_file(ucd_dir / "DerivedCoreProperties.txt")
    emoji_props = parse_range_property_file(ucd_dir / "emoji" / "emoji-data.txt")

    alphabetic = expand_ranges(derived["Alphabetic"])
    other_lower = expand_ranges(proplist["Other_Lowercase"])
    other_upper = expand_ranges(proplist["Other_Uppercase"])
    ideographic = expand_ranges(proplist["Ideographic"])
    other_id_start = expand_ranges(proplist["Other_ID_Start"])
    other_id_continue = expand_ranges(proplist["Other_ID_Continue"])
    emoji_set = expand_ranges(emoji_props["Emoji"])
    emoji_comp = expand_ranges(emoji_props["Emoji_Component"])
    emoji_mod = expand_ranges(emoji_props["Emoji_Modifier"])
    emoji_mod_base = expand_ranges(emoji_props["Emoji_Modifier_Base"])
    emoji_pres = expand_ranges(emoji_props["Emoji_Presentation"])
    ext_picto = expand_ranges(emoji_props["Extended_Pictographic"])

    # 3. Parse Scripts.txt & PropertyValueAliases.txt
    scripts_raw = parse_range_property_file(ucd_dir / "Scripts.txt")
    script_names_set: set[str] = {"UNKNOWN"}
    for s_name in scripts_raw:
        script_names_set.add(normalize_enum_name(s_name))
    sorted_scripts = sorted(script_names_set)
    script_name_to_id = {name: idx for idx, name in enumerate(sorted_scripts)}

    script_id = [script_name_to_id["UNKNOWN"]] * (MAX_CP + 1)
    for s_name, ranges in scripts_raw.items():
        sid = script_name_to_id[normalize_enum_name(s_name)]
        for lo, hi in ranges:
            for c in range(lo, hi + 1):
                script_id[c] = sid

    # 4. Parse Blocks.txt (including sentinel "NONE" for unassigned blocks)
    blocks_raw = parse_range_property_file(ucd_dir / "Blocks.txt")
    block_enum_names: set[str] = {"NONE"}
    block_ucd_to_enum: dict[str, str] = {}
    for b_ucd in blocks_raw:
        norm = normalize_enum_name(b_ucd)
        canonical = JAVA_CANONICAL_BLOCK_RENAMES.get(norm, norm)
        block_ucd_to_enum[b_ucd] = canonical
        block_enum_names.add(canonical)
    sorted_blocks = sorted(block_enum_names)
    block_name_to_id = {name: idx for idx, name in enumerate(sorted_blocks)}
    none_block_id = block_name_to_id["NONE"]

    block_id = [none_block_id] * (MAX_CP + 1)
    for b_ucd, ranges in blocks_raw.items():
        bid = block_name_to_id[block_ucd_to_enum[b_ucd]]
        for lo, hi in ranges:
            for c in range(lo, hi + 1):
                block_id[c] = bid

    # 5. Build character-dictionary.tsv & property-aliases.tsv
    dict_lines = ["schema\t1", "record\t21-byte-big-endian-deltas"]
    for idx, s_name in enumerate(sorted_scripts):
        dict_lines.append(f"script\t{idx}\t{s_name}")
    for idx, b_name in enumerate(sorted_blocks):
        dict_lines.append(f"block\t{idx}\t{b_name}")
    character_dictionary_tsv = ("\n".join(dict_lines) + "\n").encode("utf-8")

    alias_pairs: set[tuple[str, str, int]] = set()
    # Java SE `Character.UnicodeBlock.forName` accepts:
    #   (1) Canonical Java enum name (underscores)
    #   (2) Canonical Unicode Blocks.txt name (uppercased, preserving spaces/hyphens)
    #   (3) Canonical Unicode Blocks.txt name with spaces removed (preserving hyphens)
    #   (4) Deprecated SURROGATES_AREA sentinel (65535 -> matches no code points)
    for b_ucd, canonical in block_ucd_to_enum.items():
        bid = block_name_to_id[canonical]
        ucd_upper = b_ucd.upper()
        variants = [
            canonical,
            ucd_upper,
            ucd_upper.replace(" ", ""),
        ]
        # Historical Java enum renames also accept space / space-removed forms of the enum name
        if canonical != normalize_enum_name(b_ucd) and "_" in canonical:
            variants.extend([canonical.replace("_", " "), canonical.replace("_", "")])
        for variant in variants:
            alias_pairs.add(("block", variant, bid))
    alias_pairs.add(("block", "SURROGATES_AREA", 65535))

    # Script aliases from PropertyValueAliases.txt (`sc ; Short ; Long`)
    for s_name, sid in script_name_to_id.items():
        alias_pairs.add(("script", s_name, sid))
    for raw in (ucd_dir / "PropertyValueAliases.txt").read_text(encoding="utf-8").splitlines():
        line = raw.split("#", 1)[0].strip()
        if not line:
            continue
        parts = [p.strip() for p in line.split(";")]
        if len(parts) >= 3 and parts[0] == "sc":
            short_name = parts[1].upper()
            long_name = normalize_enum_name(parts[2])
            if long_name in script_name_to_id:
                sid = script_name_to_id[long_name]
                alias_pairs.add(("script", short_name, sid))
                alias_pairs.add(("script", long_name, sid))

    sorted_aliases = sorted(alias_pairs, key=lambda x: (x[0], x[1], x[2]))
    property_aliases_tsv = (
        "\n".join(f"{k}\t{name}\t{idx}" for k, name, idx in sorted_aliases) + "\n"
    ).encode("utf-8")

    # 6. Build metadata-runs.bin & case-deltas.bin
    meta_runs = bytearray()
    case_deltas = bytearray()
    prev_tuple: tuple[int, int, int, int] | None = None

    LETTER_CATS = {1, 2, 3, 4, 5}
    SPACE_CATS = {12, 13, 14}
    NON_BREAK_SPACES = {0x00A0, 0x2007, 0x202F}
    EXTRA_WS = {0x0009, 0x000A, 0x000B, 0x000C, 0x000D, 0x001C, 0x001D, 0x001E, 0x001F}

    for cp in range(MAX_CP + 1):
        gc = cat[cp]
        is_letter = gc in LETTER_CATS
        is_digit = gc == 9
        is_alpha = cp in alphabetic
        is_defined = gc != 0
        is_iso_ctrl = (0x0000 <= cp <= 0x001F) or (0x007F <= cp <= 0x009F)
        is_id_ignorable = (
            (0x0000 <= cp <= 0x0008)
            or (0x000E <= cp <= 0x001B)
            or (0x007F <= cp <= 0x009F)
            or gc == 16
        )
        is_ideo = cp in ideographic
        is_java_start = is_letter or gc in (26, 23, 10)
        is_java_part = is_java_start or gc in (9, 8, 6) or is_id_ignorable
        is_uni_start = is_letter or gc == 10 or cp in other_id_start
        is_uni_part = (
            is_uni_start
            or gc in (23, 9, 8, 6)
            or is_id_ignorable
            or cp in other_id_continue
        )
        is_lower = gc == 2 or cp in other_lower
        is_upper = gc == 1 or cp in other_upper
        is_title = gc == 3
        is_space = gc in SPACE_CATS
        is_ws = (is_space and cp not in NON_BREAK_SPACES) or (cp in EXTRA_WS)

        bits = 0
        if is_alpha:
            bits |= 1 << 0
        if is_defined:
            bits |= 1 << 1
        if is_digit:
            bits |= 1 << 2
        if cp in emoji_set:
            bits |= 1 << 3
        if cp in emoji_comp:
            bits |= 1 << 4
        if cp in emoji_mod:
            bits |= 1 << 5
        if cp in emoji_mod_base:
            bits |= 1 << 6
        if cp in emoji_pres:
            bits |= 1 << 7
        if cp in ext_picto:
            bits |= 1 << 8
        if is_iso_ctrl:
            bits |= 1 << 9
        if is_id_ignorable:
            bits |= 1 << 10
        if is_ideo:
            bits |= 1 << 11
        if is_java_part:
            bits |= 1 << 12
        if is_java_start:
            bits |= 1 << 13
        if is_letter:
            bits |= 1 << 14
        if is_letter or is_digit:
            bits |= 1 << 15
        if is_lower:
            bits |= 1 << 16
        if mirrored[cp]:
            bits |= 1 << 17
        if is_space:
            bits |= 1 << 18
        if is_title:
            bits |= 1 << 19
        if is_uni_part:
            bits |= 1 << 20
        if is_uni_start:
            bits |= 1 << 21
        if is_upper:
            bits |= 1 << 22
        if is_ws:
            bits |= 1 << 23

        cur = (gc, script_id[cp], block_id[cp], bits)
        if prev_tuple is not None and cur != prev_tuple:
            meta_runs.extend(struct.pack(">IBHHI", cp - 1, *prev_tuple))
        prev_tuple = cur

        dl = simple_lower[cp] - cp
        du = simple_upper[cp] - cp
        dt = simple_title[cp] - cp
        if dl or du or dt:
            case_deltas.extend(struct.pack(">Iiii", cp, dl, du, dt))

    assert prev_tuple is not None
    meta_runs.extend(struct.pack(">IBHHI", MAX_CP, *prev_tuple))

    # 7. Build name-index.bin, name-pool.bin, named-codepoints.bin, name-upper-ascii.bin
    sorted_named_cps = sorted(explicit_names.keys())
    named_codepoints = bytearray()
    for cp in sorted_named_cps:
        named_codepoints.extend(struct.pack(">I", cp))

    name_items = sorted(
        ((name.encode("ascii"), cp) for cp, name in explicit_names.items()),
        key=lambda item: item[0],
    )
    name_pool = bytearray()
    name_index = bytearray()
    for name_bytes, cp in name_items:
        off = len(name_pool)
        name_pool.extend(name_bytes)
        name_index.extend(struct.pack(">III", off, len(name_bytes), cp))

    # Full unconditional uppercase mappings from SpecialCasing.txt + UnicodeData.txt where result is pure ASCII
    upper_ascii_map: dict[int, bytes] = {}
    for cp in range(128, MAX_CP + 1):
        up = simple_upper[cp]
        if up < 128:
            upper_ascii_map[cp] = bytes([up])
    for raw in (ucd_dir / "SpecialCasing.txt").read_text(encoding="utf-8").splitlines():
        line = raw.split("#", 1)[0].strip()
        if not line:
            continue
        parts = [p.strip() for p in line.split(";")]
        if len(parts) >= 5 and parts[4]:
            continue  # Skip conditional (locale/context-dependent) mappings
        cp = int(parts[0], 16)
        if cp < 128:
            continue
        up_cps = [int(tok, 16) for tok in parts[3].split()]
        if up_cps and all(c < 128 for c in up_cps) and len(up_cps) <= 3:
            upper_ascii_map[cp] = bytes(up_cps)

    name_upper_ascii = bytearray()
    for cp in sorted(upper_ascii_map.keys()):
        b = upper_ascii_map[cp]
        padded = b + b"\x00" * (3 - len(b))
        name_upper_ascii.extend(struct.pack(">IB", cp, len(b)) + padded)

    # 8. Build canon-ccc.bin, canon-decomp.bin, canon-comp.bin
    canon_ccc = bytearray()
    prev_ccc = ccc[0]
    for cp in range(1, MAX_CP + 1):
        if ccc[cp] != prev_ccc:
            canon_ccc.extend(struct.pack(">IB", cp - 1, prev_ccc))
            prev_ccc = ccc[cp]
    canon_ccc.extend(struct.pack(">IB", MAX_CP, prev_ccc))

    def full_nfd(c: int) -> list[int]:
        if c not in raw_canon_decomp:
            return [c]
        out: list[int] = []
        for sub in raw_canon_decomp[c]:
            out.extend(full_nfd(sub))
        i = 1
        while i < len(out):
            cc = ccc[out[i]]
            if cc == 0:
                i += 1
                continue
            j = i
            while j > 0 and ccc[out[j - 1]] > cc:
                out[j - 1], out[j] = out[j], out[j - 1]
                j -= 1
            i += 1
        return out

    sorted_decomp_cps = sorted(raw_canon_decomp.keys())
    decomp_pool = bytearray()
    decomp_header = bytearray(struct.pack(">I", len(sorted_decomp_cps)))
    pool_words = 0
    for cp in sorted_decomp_cps:
        seq = full_nfd(cp)
        assert len(seq) < 256 and pool_words < (1 << 24)
        meta = (len(seq) << 24) | pool_words
        decomp_header.extend(struct.pack(">II", cp, meta))
        for sub in seq:
            decomp_pool.extend(struct.pack(">I", sub))
        pool_words += len(seq)
    canon_decomp = bytes(decomp_header + decomp_pool)

    exclusions: set[int] = set()
    for raw in (ucd_dir / "CompositionExclusions.txt").read_text(encoding="utf-8").splitlines():
        line = raw.split("#", 1)[0].strip()
        if not line:
            continue
        if ".." in line:
            a, b = line.split("..", 1)
            exclusions.update(range(int(a, 16), int(b, 16) + 1))
        else:
            exclusions.add(int(line, 16))

    comp_pairs_list: list[tuple[int, int, int]] = []
    for cp, seq in raw_canon_decomp.items():
        if cp in exclusions or len(seq) != 2 or ccc[seq[0]] != 0:
            continue
        comp_pairs_list.append((seq[0], seq[1], cp))
    comp_pairs_list.sort(key=lambda item: (item[0], item[1]))
    canon_comp = bytearray()
    for a, b, composed in comp_pairs_list:
        canon_comp.extend(struct.pack(">III", a, b, composed))

    # 9. Build grapheme-runs.bin from official Unicode 15.0.0 UAX #29 GraphemeBreakProperty + emoji-data
    gbp_raw = parse_range_property_file(ucd_dir / "auxiliary" / "GraphemeBreakProperty.txt")
    gtype = [0] * (MAX_CP + 1)
    # Unassigned (Cn=0) and Surrogates (Cs=19) classify as Control (3) unless given a more specific break class
    for cp in range(MAX_CP + 1):
        if cat[cp] in (0, 19):
            gtype[cp] = GRAPHEME_ID["Control"]
    # Extended_Pictographic overrides Control/Other for reserved emoji ranges per UAX #29 GB11
    for cp in ext_picto:
        gtype[cp] = GRAPHEME_ID["Extended_Pictographic"]
    # Explicit Grapheme_Cluster_Break values from official GraphemeBreakProperty.txt override
    for prop_name, ranges in gbp_raw.items():
        gid = GRAPHEME_ID[prop_name]
        for lo, hi in ranges:
            for c in range(lo, hi + 1):
                gtype[c] = gid

    grapheme_runs = bytearray()
    prev_g = gtype[0]
    for cp in range(1, MAX_CP + 1):
        if gtype[cp] != prev_g:
            grapheme_runs.extend(struct.pack(">IB", cp - 1, prev_g))
            prev_g = gtype[cp]
    grapheme_runs.extend(struct.pack(">IB", MAX_CP, prev_g))

    return {
        "name-index.bin": bytes(name_index),
        "name-pool.bin": bytes(name_pool),
        "named-codepoints.bin": bytes(named_codepoints),
        "name-upper-ascii.bin": bytes(name_upper_ascii),
        "metadata-runs.bin": bytes(meta_runs),
        "case-deltas.bin": bytes(case_deltas),
        "canon-ccc.bin": bytes(canon_ccc),
        "canon-decomp.bin": canon_decomp,
        "canon-comp.bin": bytes(canon_comp),
        "grapheme-runs.bin": bytes(grapheme_runs),
        "character-dictionary.tsv": character_dictionary_tsv,
        "property-aliases.tsv": property_aliases_tsv,
    }


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--ucd-dir", type=Path, required=True, help="Directory with official Unicode 15.0.0 UCD files")
    parser.add_argument(
        "--out-dir",
        type=Path,
        default=Path("crates/pkl-runtime/src/java_regex"),
        help="Destination directory for generated tables",
    )
    parser.add_argument(
        "--compare",
        action="store_true",
        help="Compare generated tables against existing files in --out-dir without writing",
    )
    parser.add_argument("--write", action="store_true", help="Write generated tables into --out-dir")
    args = parser.parse_args(argv)

    tables = generate_tables(args.ucd_dir)
    summary = {}
    mismatches = []
    for name, data in sorted(tables.items()):
        sha = hashlib.sha256(data).hexdigest()
        dest = args.out_dir / name
        existing = dest.read_bytes() if dest.exists() else None
        same = existing == data
        summary[name] = {
            "bytes": len(data),
            "sha256": sha,
            "matches_existing": same,
        }
        if not same:
            mismatches.append((name, len(existing) if existing is not None else -1, len(data)))
        if args.write:
            dest.write_bytes(data)

    print(json.dumps({"tables": summary, "mismatches": mismatches}, indent=2, sort_keys=True))
    if args.compare and mismatches:
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
