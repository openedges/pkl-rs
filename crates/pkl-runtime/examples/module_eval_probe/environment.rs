//! Native environment acquisition for the development CLI profile.

pub(super) fn inherited() -> Result<Vec<(String, String)>, String> {
    std::env::vars_os()
        .map(|(name, value)| Ok((decode(name)?, decode(value)?)))
        .collect()
}

#[cfg(unix)]
fn decode(value: std::ffi::OsString) -> Result<String, String> {
    use std::os::unix::ffi::OsStrExt;
    Ok(decode_utf8(value.as_bytes()))
}

#[cfg(not(unix))]
fn decode(value: std::ffi::OsString) -> Result<String, String> {
    value.into_string().map_err(|_| {
        "module-eval adapter does not yet support non-Unicode environment entries on this platform"
            .into()
    })
}

/// The pinned Unix JDK uses its UTF-8 JNU charset for environment bytes.
/// Unlike Rust's lossy decoder, it consumes an encoded surrogate as one
/// malformed sequence, including a truncated two-byte surrogate prefix.
#[cfg(unix)]
fn decode_utf8(mut bytes: &[u8]) -> String {
    let mut result = String::with_capacity(bytes.len());
    loop {
        match std::str::from_utf8(bytes) {
            Ok(valid) => {
                result.push_str(valid);
                return result;
            }
            Err(error) => {
                result.push_str(
                    std::str::from_utf8(&bytes[..error.valid_up_to()])
                        .expect("validated UTF-8 prefix"),
                );
                bytes = &bytes[error.valid_up_to()..];
                let consumed = if bytes.first() == Some(&0xed)
                    && bytes
                        .get(1)
                        .is_some_and(|byte| (0xa0..=0xbf).contains(byte))
                {
                    2 + usize::from(
                        bytes
                            .get(2)
                            .is_some_and(|byte| (0x80..=0xbf).contains(byte)),
                    )
                } else {
                    error.error_len().unwrap_or(bytes.len())
                };
                result.push('\u{fffd}');
                bytes = &bytes[consumed..];
            }
        }
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::decode_utf8;

    #[test]
    fn malformed_surrogates_preserve_following_environment_bytes() {
        assert_eq!(decode_utf8(b"a\xed\xa0\x80b\xed\xbf"), "a\u{fffd}b\u{fffd}");
        assert_eq!(
            decode_utf8(b"\xed\xa0=\xc0\xaf"),
            "\u{fffd}=\u{fffd}\u{fffd}"
        );
        assert_eq!(decode_utf8("é😀=value".as_bytes()), "é😀=value");
    }
}
