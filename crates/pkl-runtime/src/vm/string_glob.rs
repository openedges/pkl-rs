//! Validation phase of pinned GlobResolver.toRegexString, not Pattern.compile.
use super::*;
pub(super) fn valid(text: &PklString) -> Result<bool, RuntimeError> {
    let units = text.encode_utf16().collect::<Vec<_>>();
    let mut group = false;
    let mut i = 0;
    while i < units.len() {
        let next = units.get(i + 1).copied().unwrap_or(0);
        match units[i] {
            0x7b => {
                if group {
                    return Ok(false);
                }
                group = true;
            }
            0x7d => group = false,
            0x5c => {
                if !matches!(next, 0x3f | 0x2a | 0x5b | 0x7b | 0x5c) {
                    return Ok(false);
                }
                i += 1;
            }
            0x5b => {
                if next == 0 {
                    return Ok(false);
                }
                if matches!(next, 0x5e | 0x21 | 0x5d) {
                    i += 1;
                }
                i += 1;
                if i == units.len() {
                    return Err(RuntimeError::new(
                        ErrorKind::Unsupported,
                        "String.isGlobPattern encountered the pinned Java character-class index bug.",
                    ));
                }
                while units[i] != 0x5d {
                    if units[i] == 0x2f
                        || units[i] == 0x5b && matches!(units.get(i + 1), Some(0x3a | 0x3d | 0x2e))
                    {
                        return Ok(false);
                    }
                    i += 1;
                    if i == units.len() {
                        return Ok(false);
                    }
                }
            }
            0x3f | 0x2a | 0x2b | 0x40 | 0x21 => {
                if next == 0x28 {
                    return Ok(false);
                }
                if units[i] == 0x2a && next == 0x2a {
                    i += 1;
                }
            }
            _ => {}
        }
        i += 1;
    }
    Ok(!group)
}
