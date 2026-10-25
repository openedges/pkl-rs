use super::PklString;

#[test]
fn every_bmp_unit_matches_pinned_java_whitespace() {
    // Captured from the pinned Java StringUtils, not a Rust Unicode table.
    // The fixture verifier binds this file to both Java producer sessions.
    const EXPECTED: &[u8; 8192] = include_bytes!(
        "../../../../tests/fixtures/m2-string-properties/pkl-whitespace-bmp.bitset.bin"
    );
    for unit in 0..=u16::MAX {
        let expected = EXPECTED[usize::from(unit) >> 3] & (1 << (unit & 7)) != 0;
        let text = PklString::from_utf16(vec![unit]);
        assert_eq!(text.is_blank(), expected, "UTF-16 unit U+{unit:04X}");
    }
}
