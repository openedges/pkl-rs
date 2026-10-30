//! Source controls for the shared parser migration. These have not been run.
//! The loaded-source literals come from the actual supplemental Java callbacks;
//! the other cases are synthetic controls, not additional oracle observations.
use super::*;

fn units(bytes: &[u8]) -> Vec<u16> {
    assert_eq!(bytes.len() % 2, 0);
    bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u16::from_be_bytes([pair[0], pair[1]]))
        .collect()
}

#[test]
fn actual_java_loaded_literal_bodies_parse_without_rewriting_the_source() {
    for (source, value) in [
        (
            include_bytes!("../../tests/fixtures/m2-glob-lossless-source/high.source.utf16be")
                .as_slice(),
            include_bytes!("../../tests/fixtures/m2-glob-lossless-source/high.value.utf16be")
                .as_slice(),
        ),
        (
            include_bytes!("../../tests/fixtures/m2-glob-lossless-source/low.source.utf16be")
                .as_slice(),
            include_bytes!("../../tests/fixtures/m2-glob-lossless-source/low.value.utf16be")
                .as_slice(),
        ),
        (
            include_bytes!("../../tests/fixtures/m2-glob-lossless-source/mixed.source.utf16be")
                .as_slice(),
            include_bytes!("../../tests/fixtures/m2-glob-lossless-source/mixed.value.utf16be")
                .as_slice(),
        ),
    ] {
        let original = units(source);
        let source = SourceText::from_utf16(original.clone());
        let module = parse_module_for_evaluation(&source).unwrap();
        assert!(module.deferred_diagnostics.is_empty());
        assert_eq!(
            module.source.original().encode_utf16().collect::<Vec<_>>(),
            original
        );
        assert_eq!(module.members.len(), 1);
        assert_eq!(module.members[0].name, "name");
        let ExprKind::String(actual) = &module.members[0].expr.kind else {
            panic!("literal body");
        };
        assert_eq!(actual.encode_utf16().collect::<Vec<_>>(), units(value));
    }
}

#[test]
fn lone_units_in_comments_strings_and_quoted_names_are_never_eof() {
    let mut original: Vec<_> = "//".encode_utf16().collect();
    original.extend([
        0xd800, 13, 10, 0x60, 0xdfff, 0x60, 0x20, 0x3d, 0x20, 0x22, 0xd800, 0x22,
    ]);
    let source = SourceText::from_utf16(original.clone());
    let module = parse_module_for_evaluation(&source).unwrap();
    assert_eq!(module.members.len(), 1);
    assert_eq!(
        module.members[0].name.encode_utf16().collect::<Vec<_>>(),
        vec![0xdfff]
    );
    let ExprKind::String(value) = &module.members[0].expr.kind else {
        panic!("literal body");
    };
    assert_eq!(value.encode_utf16().collect::<Vec<_>>(), vec![0xd800]);
    assert_eq!(
        module.source.original().encode_utf16().collect::<Vec<_>>(),
        original
    );
}

#[test]
fn invalid_identifier_reports_the_original_first_java_unit_at_its_real_span() {
    for original in [
        vec![0xd800],
        vec![0xdfff],
        vec![0xd83d, 0xde00],
        vec![0xe000],
        vec![0xfffd],
    ] {
        let source = SourceText::from_utf16(original.clone());
        let error = parse_module_for_evaluation(&source).unwrap_err().diagnostic;
        let mut expected: Vec<_> = "Invalid identifier `".encode_utf16().collect();
        expected.push(original[0]);
        expected.extend("`.".encode_utf16());
        assert_eq!(error.message.encode_utf16().collect::<Vec<_>>(), expected);
        assert_eq!(error.span, Span::new(0, source.len_bytes()));
        assert_eq!(error.highlight_length_utf16, Some(1));
    }
}

#[test]
fn raw_surrogate_and_escape_are_semantically_equal_but_keep_distinct_source_spans() {
    let source = SourceText::from_utf16(vec![0x22, 0xd800, 0x22]);
    let raw = parse_expression_source(source.entire()).unwrap();
    let escaped = parse_expression("\"\\u{d800}\"").unwrap();
    assert_eq!(raw.kind, escaped.kind);
    assert_eq!(raw.span, Span::new(0, 5));
    assert_eq!(escaped.span, Span::new(0, 10));
    let real = parse_expression("\"\u{e000}\"").unwrap();
    assert_ne!(raw.kind, real.kind);
    assert_eq!(
        source.original().encode_utf16().collect::<Vec<_>>(),
        vec![0x22, 0xd800, 0x22]
    );
}

#[test]
fn active_escape_reports_one_java_unit_even_for_a_scalar_pair() {
    for original in [vec![0xd800], vec![0xdfff], vec![0xd83d, 0xde00]] {
        let mut source_units = vec![0x22, 0x5c];
        source_units.extend(&original);
        source_units.push(0x22);
        let source = SourceText::from_utf16(source_units);
        let error = parse_expression_source(source.entire()).unwrap_err();
        let mut expected: Vec<_> = "Invalid character escape sequence `\\"
            .encode_utf16()
            .collect();
        expected.push(original[0]);
        expected.extend(
            "`.\n\nValid character escape sequences are:  \\n  \\r  \\t  \\\"  \\\\ \\<newline>"
                .encode_utf16(),
        );
        assert_eq!(error.message.encode_utf16().collect::<Vec<_>>(), expected);
        assert_eq!(error.highlight_length_utf16, Some(2));
        assert_eq!(error.span.start, 1);
    }
}
