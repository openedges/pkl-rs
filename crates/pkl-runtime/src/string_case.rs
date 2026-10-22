//! Pinned Java ROOT-uppercase mappings, independent of Rust's Unicode version.
use crate::value::PklString;

mod uppercase;

pub(crate) fn uppercase(value: &PklString) -> PklString {
    if let Some(text) = value.as_str()
        && text.is_ascii()
    {
        return text.to_ascii_uppercase().into();
    }
    let mut output = Vec::new();
    for decoded in char::decode_utf16(value.encode_utf16()) {
        match decoded {
            Ok(point) => {
                match uppercase::UPPERCASE.binary_search_by_key(&u32::from(point), |row| row.0) {
                    Ok(index) => output.extend_from_slice(uppercase::UPPERCASE[index].1),
                    Err(_) => output.extend_from_slice(point.encode_utf16(&mut [0; 2])),
                }
            }
            Err(unit) => output.push(unit.unpaired_surrogate()),
        }
    }
    PklString::from_utf16(output)
}

#[cfg(test)]
mod tests {
    use super::uppercase;
    use crate::value::PklString;

    #[test]
    fn root_case_preserves_surrogates_expansions_and_pinned_unicode_version() {
        // Java 21 ROOT: isolated units survive, sharp-s expands, and the paired
        // Deseret lowercase maps to uppercase. U+1C8A is uncased in this JDK,
        // although newer Unicode versions add a corresponding uppercase letter.
        let input = PklString::from_utf16(vec![0xd800, 0x61, 0xdc00, 0xdf, 0xd801, 0xdc28, 0x1c8a]);
        let expected = PklString::from_utf16(vec![
            0xd800, 0x41, 0xdc00, 0x53, 0x53, 0xd801, 0xdc00, 0x1c8a,
        ]);
        assert_eq!(uppercase(&input), expected);
        assert_eq!(
            uppercase(&PklString::from("iıİ σς Straße")),
            "IIİ ΣΣ STRASSE".into()
        );
    }
}
