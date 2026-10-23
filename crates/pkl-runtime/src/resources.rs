//! Typed resource-reader results. I/O stays in the Host; values and caches are
//! owned by the shared evaluator. Absence is distinct from a raw null callback.
use std::sync::Arc;

use crate::value::PklString;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Resource {
    /// The reader's returned URI, which need not equal the requested cache key.
    pub uri: PklString,
    pub bytes: Arc<[u8]>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReadResult {
    Missing,
    Text(PklString),
    Resource(Resource),
}

/// Java UTF-8 decoder consumes a malformed surrogate encoding as one sequence,
/// unlike Rust's lossy decoder. Other malformed sequences consume only their
/// valid prefix, leaving the next byte for the following decoding step.
pub(crate) fn decode_utf8(bytes: &[u8]) -> PklString {
    if let Ok(text) = std::str::from_utf8(bytes) {
        return text.into();
    }
    let mut text = String::new();
    let mut index = 0;
    while index < bytes.len() {
        let first = bytes[index];
        if first < 128 {
            text.push(first as char);
            index += 1;
            continue;
        }
        let width = match first {
            0xc2..=0xdf => 2,
            0xe0..=0xef => 3,
            0xf0..=0xf4 => 4,
            _ => 1,
        };
        let mut consumed = 1;
        while consumed < width && index + consumed < bytes.len() {
            let next = bytes[index + consumed];
            if next & 0xc0 != 0x80
                || consumed == 1
                    && (first == 0xe0 && next < 0xa0
                        || first == 0xf0 && next < 0x90
                        || first == 0xf4 && next >= 0x90)
            {
                break;
            }
            consumed += 1;
        }
        let valid = width > 1 && consumed == width;
        if valid {
            let mut point = u32::from(first & (0x7f >> width));
            for &byte in &bytes[index + 1..index + width] {
                point = point << 6 | u32::from(byte & 63);
            }
            text.push(char::from_u32(point).unwrap_or('\u{fffd}'));
        } else {
            text.push('\u{fffd}');
        }
        index += consumed;
    }
    text.into()
}
