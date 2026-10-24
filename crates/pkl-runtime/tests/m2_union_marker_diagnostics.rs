//! Source-backed parser diagnostics for Java 0.32.0 union default markers.
//! The fixture's raw Java channels are verified independently by the Python tool.
use std::fs;
use std::path::PathBuf;

use pkl_runtime::diagnostic::{DiagnosticKind, DiagnosticOrigin, FailureDisposition};
use pkl_runtime::parser::parse_module;
use pkl_runtime::source::Span;

const NONUNION: &str = "Only type unions can have a default marker (*).";
const MULTIPLE: &str = "A type union cannot have more than one default type.";

fn module(name: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/m2-union-marker-diagnostics/modules")
        .join(name);
    fs::read_to_string(path).unwrap()
}

#[test]
fn single_default_marker_covers_the_complete_first_type_and_declaration_context() {
    // Byte spans correspond to the pinned Java caret locations. In particular,
    // the astral String occupies four UTF-8 bytes but three rendered UTF-16 units.
    for (file, start, end) in [
        ("00-nonunion-int.pkl", 14, 18),
        ("04-nonunion-parenthesized.pkl", 14, 20),
        ("05-nonunion-nested-union.pkl", 14, 29),
        ("06-nonunion-function.pkl", 14, 32),
        ("07-nonunion-generic.pkl", 14, 24),
        ("33-property-context.pkl", 7, 11),
        ("34-parameter-context.pkl", 20, 24),
        ("35-return-context.pkl", 20, 24),
        ("36-class-parent-context.pkl", 16, 20),
        ("37-nonunion-comment-gap.pkl", 14, 27),
        ("38-nonunion-emoji.pkl", 14, 21),
        ("39-nonunion-crlf.pkl", 14, 18),
    ] {
        let error = parse_module(&module(file)).unwrap_err();
        assert_eq!(error.kind, DiagnosticKind::Syntax, "{file}");
        assert_eq!(error.provenance.origin, DiagnosticOrigin::Parser, "{file}");
        assert_eq!(
            error.provenance.disposition,
            FailureDisposition::LanguageError,
            "{file}"
        );
        assert_eq!(error.message, NONUNION, "{file}");
        assert_eq!(error.span, Span::new(start, end), "{file}");
    }
}

#[test]
fn second_default_marker_wins_before_parsing_a_later_invalid_atom() {
    for (file, start) in [
        ("20-duplicate-first-second.pkl", 21),
        ("27-duplicate-before-eof.pkl", 30),
        ("28-duplicate-before-bad-generic.pkl", 21),
    ] {
        let error = parse_module(&module(file)).unwrap_err();
        assert_eq!(error.kind, DiagnosticKind::Syntax, "{file}");
        assert_eq!(error.message, MULTIPLE, "{file}");
        assert_eq!(error.span, Span::new(start, start + 1), "{file}");
    }
}

#[test]
fn malformed_first_atom_and_later_source_error_keep_java_first_error_order() {
    for (file, message) in [
        (
            "29-first-bad-generic.pkl",
            "Unexpected token `>`. Expected a type.",
        ),
        (
            "30-missing-union-atom.pkl",
            "Unexpected token ``. Expected a type.",
        ),
        (
            "31-bad-union-atom.pkl",
            "Unexpected token `|`. Expected a type.",
        ),
        ("32-notunion-before-later-error.pkl", NONUNION),
    ] {
        let error = parse_module(&module(file)).unwrap_err();
        assert_eq!(error.message, message, "{file}");
    }
}

#[test]
fn one_default_marker_in_a_real_union_remains_valid() {
    for file in [
        "11-union-first.pkl",
        "12-union-second.pkl",
        "13-union-third.pkl",
        "14-union-nullable.pkl",
        "15-union-constraint.pkl",
        "16-union-generic.pkl",
        "17-union-nested.pkl",
        "18-union-function.pkl",
        "19-union-alias.pkl",
    ] {
        parse_module(&module(file)).unwrap_or_else(|error| panic!("{file}: {error:?}"));
    }
}
