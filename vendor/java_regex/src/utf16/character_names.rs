//! Character names from the pinned JDK 21 runtime resource; no host Unicode data.
//! See JDK-CHARACTER-NAMES.md and the retained preparation provenance.
use super::character_lookup::character_record;
use alloc::{format, string::String, vec::Vec};

const INDEX: &[u8] = include_bytes!("name-index.bin");
const POOL: &[u8] = include_bytes!("name-pool.bin");
const CODEPOINTS: &[u8] = include_bytes!("named-codepoints.bin");
const UPPER_ASCII: &[u8] = include_bytes!("name-upper-ascii.bin");

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
    let mut lo = 0;
    let mut hi = CODEPOINTS.len() / 4;
    while lo < hi {
        let mid = (lo + hi) / 2;
        if word(CODEPOINTS, mid * 4) < cp {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }
    lo < CODEPOINTS.len() / 4 && word(CODEPOINTS, lo * 4) == cp
}

pub(super) fn code_point_of(input: &[u32]) -> Option<u32> {
    // Java String.trim uses UTF-16 values <= U+0020, not Unicode White_Space.
    let first = input.iter().position(|&cp| cp > 0x20)?;
    let last = input.iter().rposition(|&cp| cp > 0x20)? + 1;
    let name = uppercase_ascii(&input[first..last])?;
    let mut lo = 0;
    let mut hi = INDEX.len() / 12;
    while lo < hi {
        let mid = (lo + hi) / 2;
        let p = mid * 12;
        let off = word(INDEX, p) as usize;
        let len = word(INDEX, p + 4) as usize;
        match POOL[off..off + len].cmp(name.as_slice()) {
            core::cmp::Ordering::Less => lo = mid + 1,
            core::cmp::Ordering::Greater => hi = mid,
            core::cmp::Ordering::Equal => return Some(word(INDEX, p + 8)),
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
