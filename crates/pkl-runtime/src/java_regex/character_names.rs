//! Unicode 15.0.0 character names generated directly from official UCD 15.0.0
//! (`UnicodeData.txt`, `NameAliases.txt`, `SpecialCasing.txt`); see
//! `UNICODE-15.0.0-PROVENANCE.md` and `LICENSE-UNICODE`.
use super::character_lookup::character_record;
use std::sync::OnceLock;

const PACKED_NAMES: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/character_names_packed.bin"));
const PACKED_CODEPOINTS: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/named_codepoints_rle.bin"));
const UPPER_ASCII: &[u8] = include_bytes!("name-upper-ascii.bin");

struct DecodedNames {
    pool: Vec<u8>,
    entries: Vec<(u32, u32, u32)>,
}

static DECODED_NAMES: OnceLock<DecodedNames> = OnceLock::new();
static NAMED_RUNS: OnceLock<Vec<(u32, u32)>> = OnceLock::new();

fn read_varint(data: &[u8], pos: &mut usize) -> u32 {
    let mut value = 0u32;
    let mut shift = 0u32;
    loop {
        let byte = data[*pos];
        *pos += 1;
        value |= u32::from(byte & 0x7f) << shift;
        if byte & 0x80 == 0 {
            return value;
        }
        shift += 7;
    }
}

fn decoded_names() -> &'static DecodedNames {
    DECODED_NAMES.get_or_init(|| {
        let mut pos = 0usize;
        let count = read_varint(PACKED_NAMES, &mut pos) as usize;
        let mut pool = Vec::with_capacity(920_000);
        let mut entries = Vec::with_capacity(count);
        let mut prev_start = 0usize;
        for _ in 0..count {
            let prefix = usize::from(PACKED_NAMES[pos]);
            pos += 1;
            let suffix_len = read_varint(PACKED_NAMES, &mut pos) as usize;
            let suffix = &PACKED_NAMES[pos..pos + suffix_len];
            pos += suffix_len;
            let cp = read_varint(PACKED_NAMES, &mut pos);
            let start = pool.len();
            if prefix > 0 {
                pool.extend_from_within(prev_start..prev_start + prefix);
            }
            pool.extend_from_slice(suffix);
            let total_len = prefix + suffix_len;
            entries.push((start as u32, total_len as u32, cp));
            prev_start = start;
        }
        DecodedNames { pool, entries }
    })
}

fn named_runs() -> &'static [(u32, u32)] {
    NAMED_RUNS
        .get_or_init(|| {
            let mut pos = 0usize;
            let count = read_varint(PACKED_CODEPOINTS, &mut pos) as usize;
            let mut runs = Vec::with_capacity(count);
            let mut prev_end = 0u32;
            for _ in 0..count {
                let gap = read_varint(PACKED_CODEPOINTS, &mut pos);
                let len = read_varint(PACKED_CODEPOINTS, &mut pos);
                let start = prev_end + gap;
                let end = start + len;
                runs.push((start, end));
                prev_end = end;
            }
            runs
        })
        .as_slice()
}

fn word(data: &[u8], at: usize) -> u32 {
    u32::from_be_bytes(data[at..at + 4].try_into().expect("generated table record"))
}

/// Exact ROOT uppercase restricted to results expressible in ASCII. Character
/// names are ASCII; every non-ASCII input that can produce ASCII is generated
/// from the pinned simple mapping plus full CharacterData00/Latin1 expansions.
pub(super) fn uppercase_ascii(input: &[u32]) -> Option<Vec<u8>> {
    let mut result = Vec::new();
    for &cp in input {
        if cp < 128 {
            result.push((cp as u8).to_ascii_uppercase());
            continue;
        }
        let mut lo = 0;
        let mut hi = UPPER_ASCII.len() / 8;
        while lo < hi {
            let mid = (lo + hi) / 2;
            if word(UPPER_ASCII, mid * 8) < cp {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        if lo == UPPER_ASCII.len() / 8 || word(UPPER_ASCII, lo * 8) != cp {
            return None;
        }
        let p = lo * 8;
        result.extend_from_slice(&UPPER_ASCII[p + 5..p + 5 + usize::from(UPPER_ASCII[p + 4])]);
    }
    Some(result)
}

pub(super) fn uppercase_name(input: &str) -> Option<String> {
    String::from_utf8(uppercase_ascii(
        &input.chars().map(u32::from).collect::<Vec<_>>(),
    )?)
    .ok()
}

fn named(cp: u32) -> bool {
    let runs = named_runs();
    let idx = runs.partition_point(|&(start, _)| start <= cp);
    idx > 0 && cp < runs[idx - 1].1
}

pub(super) fn code_point_of(input: &[u32]) -> Option<u32> {
    // Java String.trim uses UTF-16 values <= U+0020, not Unicode White_Space.
    let first = input.iter().position(|&cp| cp > 0x20)?;
    let last = input.iter().rposition(|&cp| cp > 0x20)? + 1;
    let name = uppercase_ascii(&input[first..last])?;
    let table = decoded_names();
    let mut lo = 0;
    let mut hi = table.entries.len();
    while lo < hi {
        let mid = (lo + hi) / 2;
        let (off, len, cp) = table.entries[mid];
        let start = off as usize;
        let end = start + len as usize;
        match table.pool[start..end].cmp(name.as_slice()) {
            core::cmp::Ordering::Less => lo = mid + 1,
            core::cmp::Ordering::Greater => hi = mid,
            core::cmp::Ordering::Equal => return Some(cp),
        }
    }
    // Character.codePointOf's algorithmic fallback accepts only the canonical
    // Character.getName spelling. Explicitly named points cannot use block+hex.
    let split = name.iter().rposition(|&b| b == b' ')?;
    let hex = core::str::from_utf8(&name[split + 1..]).ok()?;
    let cp = u32::from_str_radix(hex, 16).ok()?;
    let record = character_record(cp)?;
    if record.category == 0 || named(cp) {
        return None;
    }
    let block = super::unicode::block_name(record.block_id)?;
    let canonical = format!("{} {cp:X}", block.replace('_', " "));
    (canonical.as_bytes() == name).then_some(cp)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packed_unicode_name_tables_match_ucd_sources_exactly() {
        let raw_index = include_bytes!("name-index.bin");
        let raw_pool = include_bytes!("name-pool.bin");
        let raw_cps = include_bytes!("named-codepoints.bin");

        let table = decoded_names();
        assert_eq!(table.entries.len(), raw_index.len() / 12);
        for (i, &(off, len, cp)) in table.entries.iter().enumerate() {
            let p = i * 12;
            let raw_off = word(raw_index, p) as usize;
            let raw_len = word(raw_index, p + 4) as usize;
            let raw_cp = word(raw_index, p + 8);
            assert_eq!(cp, raw_cp, "codepoint mismatch at entry {i}");
            assert_eq!(
                &table.pool[off as usize..off as usize + len as usize],
                &raw_pool[raw_off..raw_off + raw_len],
                "name mismatch at entry {i}"
            );
        }

        let mut expected_named = vec![false; 0x11_0000];
        for &chunk in raw_cps.as_chunks::<4>().0 {
            let cp = u32::from_be_bytes(chunk) as usize;
            expected_named[cp] = true;
        }
        for (cp, &is_named) in expected_named.iter().enumerate() {
            assert_eq!(named(cp as u32), is_named, "named({cp:#X}) mismatch");
        }
    }
}
