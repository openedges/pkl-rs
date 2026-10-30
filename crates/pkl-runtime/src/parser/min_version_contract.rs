use super::*;
use crate::value::ErrorKind;

#[test]
fn declaration_commit_and_public_syntax_diagnostics_remain_distinct() {
    let fixtures = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/m2-min-version-partial-oracle/ordinary");
    for (id, committed) in [
        ("version-high-syntax", true),
        ("header-later-lexical", true),
        ("declaration-import-parse-error", true),
        ("header-incomplete-name", false),
        ("header-incomplete-qualified-name", false),
        ("header-incomplete-annotation", false),
        ("header-incomplete-parent", false),
        ("header-lookahead-lexical", false),
        ("header-no-declaration", false),
    ] {
        let bytes = std::fs::read(fixtures.join(id).join("source.pkl")).unwrap();
        let source = SourceText::from(String::from_utf8(bytes).unwrap());
        let failure = parse_module_for_evaluation(&source).unwrap_err();
        assert_eq!(failure.completed_declaration.is_some(), committed, "{id}");
        assert_eq!(failure.source.original(), source.original(), "{id}");
        assert_eq!(
            parse_module(&source).unwrap_err(),
            failure.diagnostic,
            "{id}"
        );
    }
}

#[test]
fn partial_failure_retains_original_surrogates_without_a_second_header_parse() {
    let mut original: Vec<u16> = "// ".encode_utf16().collect();
    original.push(0xd800);
    original.extend(
        "\n@ModuleInfo { minPklVersion = \"99.9.9\" }\nmodule retained\nvalue = (\n".encode_utf16(),
    );
    let source = SourceText::from_utf16(original.clone());
    let failure = parse_module_for_evaluation(&source).unwrap_err();
    assert_eq!(
        failure.source.original().encode_utf16().collect::<Vec<_>>(),
        original
    );
    assert!(failure.completed_declaration.is_some());
    let error = failure.minimum_version_error(&"inferred".into()).unwrap();
    assert_eq!(error.kind, ErrorKind::Type);
    assert!(error.frames.is_empty());
    assert!(error.span.is_none());
    // This is an ownership invariant, not Java behavioral coverage for a new
    // unmatched-surrogate source. The recorded Stage122 source matrix is finite.
}

#[test]
fn java_substring_failure_remains_an_explicit_unsupported_api_contract() {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/m2-min-version-partial-oracle/required-internal/source.pkl");
    let source = std::fs::read_to_string(path).unwrap();
    let failure = parse_module_for_evaluation(source.as_str()).unwrap_err();
    let boundary = failure.minimum_version_error(&"inferred".into()).unwrap();
    assert_eq!(boundary.kind, ErrorKind::Unsupported);
    assert_eq!(
        parse_module(source.as_str()).unwrap_err(),
        failure.diagnostic
    );
}
