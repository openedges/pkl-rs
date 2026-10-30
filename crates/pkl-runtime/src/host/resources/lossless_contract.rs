//! Source-level controls for the shared URI representation. These are separate
//! from the retained Java acquisition: no synthetic result is oracle evidence.
use super::*;

fn raw(prefix: &str, units: &[u16], suffix: &str) -> PklString {
    let mut value = prefix.encode_utf16().collect::<Vec<_>>();
    value.extend_from_slice(units);
    value.extend(suffix.encode_utf16());
    PklString::from_utf16(value)
}

#[test]
fn literal_surrogates_are_not_percent_decoded_or_replaced() {
    for units in [&[0xd800][..], &[0xdfff][..], &[0x78, 0xd800, 0x79][..]] {
        let uri = raw("gdata:", units, "");
        assert_eq!(ResourceUri::parse(&uri).unwrap().text, uri);
        assert_eq!(
            decoded_scheme_specific_part(&uri),
            Some(PklString::from_utf16(units.to_vec()))
        );
        assert_eq!(
            glob::opaque_element(&"gdata".into(), &PklString::from_utf16(units.to_vec())).unwrap(),
            uri
        );
        assert_ne!(cache_key(&uri), cache_key(&"gdata:\u{fffd}".into()));
        assert_ne!(cache_key(&uri), cache_key(&"gdata:%ED%A0%80".into()));
    }
    // URI.decode sends only the escaped bytes through Java's replacement decoder.
    assert_eq!(
        decoded_scheme_specific_part(&"gdata:%ED%A0%80".into()),
        Some("\u{fffd}".into())
    );
    let mixed = raw("%61", &[0xd800], "%62");
    assert_eq!(decode_escapes(&mixed), raw("a", &[0xd800], "b"));
}

#[test]
fn decoder_bracket_mode_and_literal_pairs_are_distinct() {
    let input = raw("[%61]", &[0xdfff], "%62");
    assert_eq!(
        decode_uri_component(&input, true),
        raw("[%61]", &[0xdfff], "b")
    );
    assert_eq!(
        decode_uri_component(&input, false),
        raw("[a]", &[0xdfff], "b")
    );
    let pair = raw("gdata:", &[0xd800, 0xdc00], "");
    assert_eq!(
        decoded_scheme_specific_part(&pair),
        Some("\u{10000}".into())
    );
    assert_ne!(cache_key(&pair), cache_key(&raw("gdata:", &[0xd800], "")));
    assert_ne!(cache_key(&pair), cache_key(&raw("gdata:", &[0xdc00], "")));
}

#[test]
fn normalized_payload_preserves_raw_mapping_key_and_inferred_name() {
    let raw_key = raw("gmod:/a/../", &[0xd800], ".pkl");
    let normalized = raw("gmod:/", &[0xd800], ".pkl");
    assert_eq!(glob::normalize(&raw_key).unwrap(), normalized);
    assert_ne!(raw_key, normalized);
    assert_eq!(
        infer_module_name(&normalized, false).unwrap(),
        PklString::from_utf16(vec![0xd800])
    );
    assert_eq!(
        glob::resolve(
            &"gmod:/suite/main.pkl".into(),
            &raw("../", &[0xd800], ".pkl"),
            false
        )
        .unwrap(),
        normalized
    );
    assert_eq!(
        glob::path(&"gmod:/".into(), &raw("", &[0xd800], ".pkl"), true),
        normalized
    );
}

#[test]
fn source_echo_and_name_share_actual_loaded_java_units() {
    // These exact source and value channels are retained from the actual
    // successful Java supplemental import sessions, independently of this test.
    let bytes =
        include_bytes!("../../../tests/fixtures/m2-glob-lossless-source/high.source.utf16be");
    let units = bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|b| u16::from_be_bytes([b[0], b[1]]))
        .collect::<Vec<_>>();
    let source = SourceText::from(PklString::from_utf16(units.clone()));
    assert_eq!(source.original().encode_utf16().collect::<Vec<_>>(), units);
    assert!(crate::parser::parse_module(&source).is_ok());
    assert_eq!(
        glob::opaque_element(&"gmod".into(), &PklString::from_utf16(vec![0xd800])).unwrap(),
        raw("gmod:", &[0xd800], "")
    );
}
