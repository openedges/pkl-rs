//! Unicode 15.0.0 character metadata and simple case-delta tables generated
//! directly from official Unicode Consortium UCD 15.0.0 files (`Unicode-3.0`).
//! See `UNICODE-15.0.0-PROVENANCE.md`.
const RUNS: &[u8] = include_bytes!("metadata-runs.bin");
const CASES: &[u8] = include_bytes!("case-deltas.bin");
#[derive(Clone, Copy)]
pub struct CharacterRecord {
    pub category: u8,
    pub script_id: u16,
    pub block_id: u16,
    pub predicate_bits: u32,
    pub lower: u32,
    pub upper: u32,
    pub title: u32,
}
fn u32_be(buf: &[u8], at: usize) -> u32 {
    u32::from_be_bytes(buf[at..at + 4].try_into().unwrap())
}
fn u16_be(buf: &[u8], at: usize) -> u16 {
    u16::from_be_bytes(buf[at..at + 2].try_into().unwrap())
}
fn i32_be(buf: &[u8], at: usize) -> i32 {
    i32::from_be_bytes(buf[at..at + 4].try_into().unwrap())
}
pub fn character_record(cp: u32) -> Option<CharacterRecord> {
    if cp > 0x10ffff {
        return None;
    }
    let mut lo = 0usize;
    let mut hi = RUNS.len() / 13;
    while lo < hi {
        let mid = (lo + hi) / 2;
        if u32_be(RUNS, mid * 13) < cp {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }
    let p = lo * 13;
    let mut deltas = (0i32, 0i32, 0i32);
    lo = 0;
    hi = CASES.len() / 16;
    while lo < hi {
        let mid = (lo + hi) / 2;
        if u32_be(CASES, mid * 16) < cp {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }
    if lo < CASES.len() / 16 && u32_be(CASES, lo * 16) == cp {
        let q = lo * 16;
        deltas = (
            i32_be(CASES, q + 4),
            i32_be(CASES, q + 8),
            i32_be(CASES, q + 12),
        );
    }
    let mapped = |delta: i32| (i64::from(cp) + i64::from(delta)) as u32;
    Some(CharacterRecord {
        category: RUNS[p + 4],
        script_id: u16_be(RUNS, p + 5),
        block_id: u16_be(RUNS, p + 7),
        predicate_bits: u32_be(RUNS, p + 9),
        lower: mapped(deltas.0),
        upper: mapped(deltas.1),
        title: mapped(deltas.2),
    })
}
