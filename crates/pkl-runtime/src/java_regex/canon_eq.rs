//! Unicode 15.0.0 canonical equivalence (`Pattern.CANON_EQ` / `(?c)`),
//! NFD/NFC normalization, and UAX #29 grapheme cluster boundary tables.
//!
//! All tables are generated directly from the official Unicode Consortium
//! UCD 15.0.0 release (`Unicode-3.0`; see `UNICODE-15.0.0-PROVENANCE.md`) so
//! normalization and grapheme boundaries never drift with host Rust stdlib.

use super::input::Input;
use super::unicode::is_nonspacing_mark;
use super::{CompileError, CompileErrorKind};

const CCC_RUNS: &[u8] = include_bytes!("canon-ccc.bin");
const DECOMP_DATA: &[u8] = include_bytes!("canon-decomp.bin");
const COMP_PAIRS: &[u8] = include_bytes!("canon-comp.bin");
const GRAPHEME_RUNS: &[u8] = include_bytes!("grapheme-runs.bin");

// Hangul algorithmic constants (Unicode Standard §3.12)
const S_BASE: u32 = 0xAC00;
const L_BASE: u32 = 0x1100;
const V_BASE: u32 = 0x1161;
const T_BASE: u32 = 0x11A7;
const L_COUNT: u32 = 19;
const V_COUNT: u32 = 21;
const T_COUNT: u32 = 28;
const N_COUNT: u32 = V_COUNT * T_COUNT;
const S_COUNT: u32 = L_COUNT * N_COUNT;

#[inline]
fn u32_be(buf: &[u8], at: usize) -> u32 {
    u32::from_be_bytes([buf[at], buf[at + 1], buf[at + 2], buf[at + 3]])
}

/// Canonical Combining Class (0..=254) from pinned JDK 21 Unicode 15.0.0.
pub fn combining_class(cp: u32) -> u8 {
    if cp > 0x10FFFF {
        return 0;
    }
    let count = CCC_RUNS.len() / 5;
    let mut lo = 0usize;
    let mut hi = count;
    while lo < hi {
        let mid = (lo + hi) / 2;
        if u32_be(CCC_RUNS, mid * 5) < cp {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }
    if lo < count { CCC_RUNS[lo * 5 + 4] } else { 0 }
}

fn decompose_codepoint_nfd(cp: u32, out: &mut Vec<u32>) {
    if (S_BASE..S_BASE + S_COUNT).contains(&cp) {
        let s_index = cp - S_BASE;
        let l = L_BASE + s_index / N_COUNT;
        let v = V_BASE + (s_index % N_COUNT) / T_COUNT;
        let t = T_BASE + s_index % T_COUNT;
        out.push(l);
        out.push(v);
        if t != T_BASE {
            out.push(t);
        }
        return;
    }
    let count = u32_be(DECOMP_DATA, 0) as usize;
    let mut lo = 0usize;
    let mut hi = count;
    while lo < hi {
        let mid = (lo + hi) / 2;
        let key = u32_be(DECOMP_DATA, 4 + mid * 8);
        if key < cp {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }
    if lo < count && u32_be(DECOMP_DATA, 4 + lo * 8) == cp {
        let meta = u32_be(DECOMP_DATA, 4 + lo * 8 + 4);
        let len = (meta >> 24) as usize;
        let off = (meta & 0x00FF_FFFF) as usize;
        let pool_base = 4 + count * 8;
        for i in 0..len {
            out.push(u32_be(DECOMP_DATA, pool_base + (off + i) * 4));
        }
    } else {
        out.push(cp);
    }
}

fn compose_pair(a: u32, b: u32) -> Option<u32> {
    if (L_BASE..L_BASE + L_COUNT).contains(&a) && (V_BASE..V_BASE + V_COUNT).contains(&b) {
        let l_idx = a - L_BASE;
        let v_idx = b - V_BASE;
        return Some(S_BASE + (l_idx * V_COUNT + v_idx) * T_COUNT);
    }
    if (S_BASE..S_BASE + S_COUNT).contains(&a)
        && (a - S_BASE).is_multiple_of(T_COUNT)
        && b > T_BASE
        && b < T_BASE + T_COUNT
    {
        return Some(a + (b - T_BASE));
    }
    let target = (u64::from(a) << 32) | u64::from(b);
    let count = COMP_PAIRS.len() / 12;
    let mut lo = 0usize;
    let mut hi = count;
    while lo < hi {
        let mid = (lo + hi) / 2;
        let p = mid * 12;
        let key = (u64::from(u32_be(COMP_PAIRS, p)) << 32) | u64::from(u32_be(COMP_PAIRS, p + 4));
        if key < target {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }
    if lo < count {
        let p = lo * 12;
        let key = (u64::from(u32_be(COMP_PAIRS, p)) << 32) | u64::from(u32_be(COMP_PAIRS, p + 4));
        if key == target {
            return Some(u32_be(COMP_PAIRS, p + 8));
        }
    }
    None
}

fn canonical_order(cps: &mut [u32]) {
    // Sort each maximal contiguous run of non-zero combining classes stably in O(N log N)
    // (preserving relative order of identical combining classes and avoiding O(N^2) DoS on
    // adversarial combining-mark runs).
    let mut i = 0usize;
    while i < cps.len() {
        if combining_class(cps[i]) == 0 {
            i += 1;
            continue;
        }
        let start = i;
        i += 1;
        while i < cps.len() && combining_class(cps[i]) != 0 {
            i += 1;
        }
        if i - start > 1 {
            cps[start..i].sort_by_key(|&cp| combining_class(cp));
        }
    }
}

fn decode_utf16_points(units: &[u16]) -> Vec<u32> {
    let input = Input(units);
    let mut out = Vec::with_capacity(units.len());
    let mut pos = 0;
    while pos < units.len() {
        out.push(input.at(pos));
        pos = input.next(pos);
    }
    out
}

fn encode_utf16_points(cps: &[u32], out: &mut Vec<u16>) {
    for &cp in cps {
        if cp >= 0x10000 {
            let v = cp - 0x10000;
            out.push((0xD800 + (v >> 10)) as u16);
            out.push((0xDC00 + (v & 0x3FF)) as u16);
        } else {
            out.push(cp as u16);
        }
    }
}

/// Computes NFD code points of a UTF-16 slice (preserving unpaired surrogates).
pub fn nfd_points(units: &[u16]) -> Vec<u32> {
    let input = Input(units);
    let mut out = Vec::with_capacity(units.len());
    let mut pos = 0;
    while pos < units.len() {
        decompose_codepoint_nfd(input.at(pos), &mut out);
        pos = input.next(pos);
    }
    canonical_order(&mut out);
    out
}

/// Computes NFC code points of a UTF-16 slice (preserving unpaired surrogates).
pub fn nfc_points(units: &[u16]) -> Vec<u32> {
    let mut list = nfd_points(units);
    let mut starter_pos: Option<usize> = None;
    let mut last_cc: i16 = -1;
    let mut out_len = 0usize;
    for i in 0..list.len() {
        let ch = list[i];
        let cc = i16::from(combining_class(ch));
        if let Some(s_pos) = starter_pos
            && (last_cc < cc || last_cc == 0)
            && let Some(composed) = compose_pair(list[s_pos], ch)
        {
            list[s_pos] = composed;
            continue;
        }
        if cc == 0 {
            starter_pos = Some(out_len);
            last_cc = 0;
        } else {
            last_cc = cc;
        }
        list[out_len] = ch;
        out_len += 1;
    }
    list.truncate(out_len);
    list
}

/// Computes NFC of `units` and returns `Some(cp)` iff NFC has exactly 1 code point.
pub fn nfc_single_codepoint(units: &[u16]) -> Option<u32> {
    let nfc = nfc_points(units);
    if nfc.len() == 1 { Some(nfc[0]) } else { None }
}

pub fn nfc_utf16(units: &[u16]) -> Vec<u16> {
    let cps = nfc_points(units);
    let mut out = Vec::with_capacity(units.len());
    encode_utf16_points(&cps, &mut out);
    out
}

// ==================== JDK 21 Grapheme.nextBoundary ====================

const G_CR: u8 = 1;
const G_LF: u8 = 2;
const G_CONTROL: u8 = 3;
const G_EXTEND: u8 = 4;
const G_ZWJ: u8 = 5;
const G_RI: u8 = 6;
const G_PREPEND: u8 = 7;
const G_SPACINGMARK: u8 = 8;
const G_L: u8 = 9;
const G_V: u8 = 10;
const G_T: u8 = 11;
const G_LV: u8 = 12;
const G_LVT: u8 = 13;
const G_EXTENDED_PICTOGRAPHIC: u8 = 14;

fn grapheme_type(cp: u32) -> u8 {
    if cp > 0x10FFFF {
        return 0;
    }
    let count = GRAPHEME_RUNS.len() / 5;
    let mut lo = 0usize;
    let mut hi = count;
    while lo < hi {
        let mid = (lo + hi) / 2;
        if u32_be(GRAPHEME_RUNS, mid * 5) < cp {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }
    if lo < count {
        GRAPHEME_RUNS[lo * 5 + 4]
    } else {
        0
    }
}

/// Exact JDK 21 `Grapheme.rules[t0][t1]` break table (`true` = break).
const fn grapheme_break_rule(t0: u8, t1: u8) -> bool {
    // GB3: CR x LF
    if t0 == G_CR && t1 == G_LF {
        return false;
    }
    // GB11 base (before ZWJ + ExtPic check): ExtPic x Extend / ZWJ
    if t0 == G_EXTENDED_PICTOGRAPHIC && (t1 == G_EXTEND || t1 == G_ZWJ) {
        return false;
    }
    // GB4 & GB5: Control | CR | LF break on both sides
    if matches!(t0, G_CR | G_LF | G_CONTROL) || matches!(t1, G_CR | G_LF | G_CONTROL) {
        return true;
    }
    // GB9, GB9a, GB9b
    if matches!(t1, G_EXTEND | G_ZWJ | G_SPACINGMARK) || t0 == G_PREPEND {
        return false;
    }
    // GB6, GB7, GB8 (Hangul)
    if (t0 == G_L && matches!(t1, G_L | G_V | G_LV | G_LVT))
        || (matches!(t0, G_LV | G_V) && matches!(t1, G_V | G_T))
        || (matches!(t0, G_LVT | G_T) && t1 == G_T)
    {
        return false;
    }
    true
}

/// Exact port of `jdk.internal.util.regex.Grapheme.nextBoundary(src, off, limit)`.
pub fn next_grapheme_boundary(src: &[u16], off: usize, limit: usize) -> usize {
    let input = Input(&src[..limit]);
    let ch0 = input.at(off);
    let mut ret = input.next(off);
    let mut t0 = grapheme_type(ch0);
    let mut ri_count: usize = if t0 == G_RI { 1 } else { 0 };
    let mut gb11 = t0 == G_EXTENDED_PICTOGRAPHIC;
    while ret < limit {
        let ch1 = input.at(ret);
        let t1 = grapheme_type(ch1);
        if gb11 && t0 == G_ZWJ && t1 == G_EXTENDED_PICTOGRAPHIC {
            // GB11: do not break
        } else if ri_count % 2 == 1 && t0 == G_RI && t1 == G_RI {
            // GB12/13: do not break
        } else if grapheme_break_rule(t0, t1) {
            if ret > off {
                break;
            }
            gb11 = t1 == G_EXTENDED_PICTOGRAPHIC;
            ri_count = 0;
        }
        if t1 == G_RI {
            ri_count += 1;
        }
        t0 = t1;
        ret = input.next(ret);
    }
    ret
}

// ==================== JDK 21 Pattern.normalize ====================

/// Exact port of `java.util.regex.Pattern.normalize(String)` for compile-time
/// `Pattern.CANON_EQ` (when `!LITERAL`).
pub fn normalize_pattern(source: &[u16]) -> Result<Vec<u16>, CompileError> {
    let plen = source.len();
    let mut pbuf = Vec::with_capacity(plen);
    let mut last: u16 = 0;
    let mut lasto = 0usize;
    let mut in_class: u16 = 0;
    let mut i = 0usize;
    while i < plen {
        let c = source[i];
        if in_class == 0 && c == b'\\' as u16 && i + 1 < plen && source[i + 1] == b'\\' as u16 {
            i += 2;
            last = 0;
            continue;
        }
        if c == b'[' as u16 && last != b'\\' as u16 {
            if in_class == 0 {
                if lasto < i {
                    normalize_slice(source, lasto, i, &mut pbuf)?;
                }
                lasto = i;
            }
            in_class = in_class.wrapping_add(1);
        } else if c == b']' as u16 && last != b'\\' as u16 {
            in_class = in_class.wrapping_sub(1);
            if in_class == 0 {
                let nfc = nfc_utf16(&source[lasto..=i]);
                pbuf.extend_from_slice(&nfc);
                lasto = i + 1;
            }
        }
        last = c;
        i += 1;
    }
    if lasto < plen {
        normalize_slice(source, lasto, plen, &mut pbuf)?;
    }
    Ok(pbuf)
}

fn is_meta_char(cp: u32) -> bool {
    matches!(
        cp,
        0x2e | 0x24
            | 0x7c
            | 0x28
            | 0x29
            | 0x5b
            | 0x5d
            | 0x7b
            | 0x7d
            | 0x5e
            | 0x3f
            | 0x2a
            | 0x2b
            | 0x5c
    )
}

fn normalize_slice(
    src: &[u16],
    mut off: usize,
    limit: usize,
    dst: &mut Vec<u16>,
) -> Result<(), CompileError> {
    let last = off;
    while off < limit && src[off] < 0x80 {
        off += 1;
    }
    if off == limit {
        dst.extend_from_slice(&src[last..limit]);
        return Ok(());
    }
    if off > last {
        off -= 1;
        dst.extend_from_slice(&src[last..off]);
    }
    let input = Input(&src[..limit]);
    while off < limit {
        let ch0 = input.at(off);
        if is_meta_char(ch0) {
            dst.push(ch0 as u16);
            off += 1;
            continue;
        }
        let j = next_grapheme_boundary(src, off, limit);
        let seq = &src[off..j];
        let nfd_cps = nfd_points(seq);
        off = j;

        if nfd_cps.len() > 1 && is_nonspacing_mark(nfd_cps[1]) {
            let mut set: Vec<Vec<u16>> = Vec::new();
            push_unique(&mut set, seq.to_vec());
            let mut nfd_u16 = Vec::with_capacity(nfd_cps.len());
            encode_utf16_points(&nfd_cps, &mut nfd_u16);
            produce_equivalent_alternation(&nfd_u16, &mut set, src)?;
            dst.extend_from_slice(&[b'(' as u16, b'?' as u16, b':' as u16]);
            for (idx, alt) in set.iter().enumerate() {
                if idx > 0 {
                    dst.push(b'|' as u16);
                }
                dst.extend_from_slice(alt);
            }
            dst.push(b')' as u16);
        } else {
            let mut nfd_u16 = Vec::with_capacity(nfd_cps.len());
            encode_utf16_points(&nfd_cps, &mut nfd_u16);
            let nfc_u16 = nfc_utf16(seq);
            if seq != nfc_u16.as_slice() && nfd_u16 != nfc_u16 {
                dst.extend_from_slice(&[b'(' as u16, b'?' as u16, b':' as u16]);
                dst.extend_from_slice(seq);
                dst.push(b'|' as u16);
                dst.extend_from_slice(&nfd_u16);
                dst.push(b'|' as u16);
                dst.extend_from_slice(&nfc_u16);
                dst.push(b')' as u16);
            } else if seq != nfd_u16.as_slice() {
                dst.extend_from_slice(&[b'(' as u16, b'?' as u16, b':' as u16]);
                dst.extend_from_slice(seq);
                dst.push(b'|' as u16);
                dst.extend_from_slice(&nfd_u16);
                dst.push(b')' as u16);
            } else {
                dst.extend_from_slice(seq);
            }
        }
    }
    Ok(())
}

fn push_unique(set: &mut Vec<Vec<u16>>, item: Vec<u16>) {
    if !set.iter().any(|existing| existing == &item) {
        set.push(item);
    }
}

fn count_chars(units: &[u16], index: usize, length_in_code_points: usize) -> usize {
    let input = Input(units);
    let mut pos = index;
    for _ in 0..length_in_code_points {
        if pos >= units.len() {
            break;
        }
        pos = input.next(pos);
    }
    pos - index
}

const MAX_CANON_EQ_VARIANTS: usize = 1024;

fn produce_equivalent_alternation(
    source: &[u16],
    dst: &mut Vec<Vec<u16>>,
    original_pattern: &[u16],
) -> Result<(), CompileError> {
    if dst.len() >= MAX_CANON_EQ_VARIANTS {
        return Err(CompileError {
            kind: CompileErrorKind::NestingBudgetExceeded,
            message: "Pattern too complex".into(),
            pattern: original_pattern.to_vec(),
            index: 0,
        });
    }
    let len = count_chars(source, 0, 1);
    if source.len() == len {
        push_unique(dst, source.to_vec());
        return Ok(());
    }
    let base = &source[..len];
    let combining = &source[len..];
    let mut perm_budget = MAX_CANON_EQ_VARIANTS;
    let perms = produce_permutations(combining, original_pattern, &mut perm_budget)?;
    for perm in perms {
        if dst.len() >= MAX_CANON_EQ_VARIANTS {
            return Err(CompileError {
                kind: CompileErrorKind::NestingBudgetExceeded,
                message: "Pattern too complex".into(),
                pattern: original_pattern.to_vec(),
                index: 0,
            });
        }
        let mut candidate = Vec::with_capacity(base.len() + perm.len());
        candidate.extend_from_slice(base);
        candidate.extend_from_slice(&perm);
        push_unique(dst, candidate.clone());
        if let Some(composed) = compose_one_step(&candidate) {
            produce_equivalent_alternation(&composed, dst, original_pattern)?;
        }
    }
    Ok(())
}

fn produce_permutations(
    input: &[u16],
    original_pattern: &[u16],
    budget: &mut usize,
) -> Result<Vec<Vec<u16>>, CompileError> {
    let c1 = count_chars(input, 0, 1);
    if input.len() == c1 {
        return Ok(vec![input.to_vec()]);
    }
    let c2 = count_chars(input, 0, 2);
    if input.len() == c2 {
        let inp = Input(input);
        let cp0 = inp.at(0);
        let cp1 = inp.at(c1);
        if combining_class(cp1) == combining_class(cp0) {
            return Ok(vec![input.to_vec()]);
        }
        let mut swapped = Vec::with_capacity(input.len());
        swapped.extend_from_slice(&input[c1..]);
        swapped.extend_from_slice(&input[..c1]);
        return Ok(vec![input.to_vec(), swapped]);
    }
    let cps = decode_utf16_points(input);
    let length = cps.len();
    if length > 8 {
        return Err(CompileError {
            kind: CompileErrorKind::NestingBudgetExceeded,
            message: "Pattern too complex".into(),
            pattern: original_pattern.to_vec(),
            index: 0,
        });
    }
    let classes: Vec<u8> = cps.iter().map(|&cp| combining_class(cp)).collect();
    let mut results = Vec::new();
    let mut offset = 0usize;
    for x in 0..length {
        let len = count_chars(input, offset, 1);
        let mut skip = false;
        for y in (0..x).rev() {
            if classes[y] == classes[x] {
                skip = true;
                break;
            }
        }
        if !skip {
            let mut remaining = Vec::with_capacity(input.len() - len);
            remaining.extend_from_slice(&input[..offset]);
            remaining.extend_from_slice(&input[offset + len..]);
            let sub = produce_permutations(&remaining, original_pattern, budget)?;
            let ch_slice = &input[offset..offset + len];
            for perm in sub {
                if *budget == 0 {
                    return Err(CompileError {
                        kind: CompileErrorKind::NestingBudgetExceeded,
                        message: "Pattern too complex".into(),
                        pattern: original_pattern.to_vec(),
                        index: 0,
                    });
                }
                *budget -= 1;
                let mut combined = Vec::with_capacity(input.len());
                combined.extend_from_slice(ch_slice);
                combined.extend_from_slice(&perm);
                results.push(combined);
            }
        }
        offset += len;
    }
    Ok(results)
}

fn compose_one_step(input: &[u16]) -> Option<Vec<u16>> {
    let len = count_chars(input, 0, 2);
    let first_two = &input[..len];
    let result = nfc_utf16(first_two);
    if result.as_slice() == first_two {
        None
    } else {
        let mut out = Vec::with_capacity(result.len() + input.len() - len);
        out.extend_from_slice(&result);
        out.extend_from_slice(&input[len..]);
        Some(out)
    }
}
