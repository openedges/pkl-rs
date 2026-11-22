//! UTF-16 lexical rules of the pinned Pkl XML renderer and XML validators.
use super::*;
include!("xml_names.rs");
#[inline]
pub(super) fn valid_name_ascii(s: &str) -> bool {
    let mut bytes = s.bytes();
    let Some(first) = bytes.next() else {
        return false;
    };
    if !(first == b':' || first == b'_' || first.is_ascii_alphabetic()) {
        return false;
    }
    bytes.all(|b| b == b'-' || b == b'.' || b == b':' || b == b'_' || b.is_ascii_alphanumeric())
}

#[cfg(not(target_arch = "wasm32"))]
pub(super) fn valid_name_str(s: &str, version11: bool) -> bool {
    if s.is_ascii() {
        return valid_name_ascii(s);
    }
    valid_name_units(&s.encode_utf16().collect::<Vec<_>>(), version11)
}

pub(super) fn valid_name(text: &PklString, version11: bool) -> bool {
    if let Some(s) = text.as_str()
        && s.is_ascii()
    {
        return valid_name_ascii(s);
    }
    let units: Vec<_> = text.encode_utf16().collect();
    valid_name_units(&units, version11)
}

fn valid_name_units(units: &[u16], version11: bool) -> bool {
    if units.is_empty() {
        return false;
    }
    let mut i = 0;
    while i < units.len() {
        let table = match (version11, i == 0) {
            (false, true) => XML10_START,
            (false, false) => XML10_REST,
            (true, true) => XML11_START,
            (true, false) => XML11_REST,
        };
        let ch = units[i];
        if table
            .get(table.partition_point(|(_, b)| *b < ch))
            .is_some_and(|(a, _)| *a <= ch)
        {
            i += 1;
            continue;
        }
        if version11
            && (0xd800..=0xdb7f).contains(&ch)
            && units
                .get(i + 1)
                .is_some_and(|x| (0xdc00..=0xdfff).contains(x))
        {
            i += 2;
        } else {
            return false;
        }
    }
    true
}
pub(super) fn cdata(text: &PklString) -> PklString {
    let mut out = crate::pkl_string::PklStringBuilder::default();
    out.push_str("<![CDATA[");
    let units: Vec<_> = text.encode_utf16().collect();
    let mut i = 0;
    while i < units.len() {
        if units[i..].starts_with(&[0x5d, 0x5d, 0x3e]) {
            out.push_str("]]]]><![CDATA[>");
            i += 3;
        } else {
            out.push_value(&PklString::from_utf16(vec![units[i]]));
            i += 1;
        }
    }
    out.push_str("]]>");
    out.finish()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn xml_name_versions_and_raw_utf16_boundaries() {
        for name in ["a", "a:b", "_", "a-1"] {
            assert!(valid_name(&name.into(), false));
            assert!(valid_name(&name.into(), true));
        }
        for name in ["", "1a", "a b"] {
            assert!(!valid_name(&name.into(), false));
            assert!(!valid_name(&name.into(), true));
        }
        assert!(!valid_name(
            &PklString::from_utf16(vec![0xd800, 0xdc00]),
            false
        ));
        assert!(valid_name(
            &PklString::from_utf16(vec![0xdb7f, 0xdfff]),
            true
        ));
        for units in [
            vec![0xdb80, 0xdc00],
            vec![0xd800],
            vec![0xdc00],
            vec![0xd800, 0x61],
        ] {
            assert!(!valid_name(&PklString::from_utf16(units), true));
        }
        assert_eq!(cdata(&"a]]>b".into()), "<![CDATA[a]]]]><![CDATA[>b]]>");
        assert_eq!(
            super::super::plist::escape(&PklString::from_utf16(vec![0xd800, 0x26]))
                .encode_utf16()
                .collect::<Vec<_>>(),
            vec![0xd800, 0x26, 0x61, 0x6d, 0x70, 0x3b]
        );
    }
    #[test]
    fn complete_java_xml_name_bitsets_and_sequences() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/m2-xml-name-validator");
        for version in [10, 11] {
            for (domain, first, last) in [("bmp", 0_u32, 0xffff_u32), ("supp", 0x10000, 0x10ffff)] {
                for (position, continuation) in [("start", false), ("continue", true)] {
                    let name = format!("xml{version}-{domain}-{position}.bits");
                    let expected = std::fs::read(root.join(&name)).unwrap();
                    let count = (last - first + 1) as usize;
                    let mut actual = vec![0_u8; count.div_ceil(8)];
                    for cp in first..=last {
                        let mut units = Vec::with_capacity(3);
                        if continuation {
                            units.push(0x61);
                        }
                        if cp < 0x10000 {
                            units.push(cp as u16);
                        } else {
                            let n = cp - 0x10000;
                            units.extend([0xd800 + (n >> 10) as u16, 0xdc00 + (n & 0x3ff) as u16]);
                        }
                        if valid_name(&PklString::from_utf16(units), version == 11) {
                            let bit = (cp - first) as usize;
                            actual[bit / 8] |= 0x80 >> (bit % 8);
                        }
                    }
                    assert_eq!(actual, expected, "complete Java bitset {name}");
                }
            }
            let input =
                std::fs::read_to_string(root.join(format!("xml{version}-sequences.tsv"))).unwrap();
            let mut count = 0;
            for line in input.lines() {
                let (text, flag) = line.split_once('\t').unwrap();
                let units = if text == "-" {
                    vec![]
                } else {
                    text.split(',')
                        .map(|x| u16::from_str_radix(x, 16).unwrap())
                        .collect()
                };
                assert!(matches!(flag, "0" | "1"));
                assert_eq!(
                    valid_name(&PklString::from_utf16(units), version == 11),
                    flag == "1",
                    "XML{version} {text}"
                );
                count += 1;
            }
            assert_eq!(count, 19);
        }
    }
}
