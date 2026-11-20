//! Source-derived internal contracts, distinct from the copied Pkl Java56 rows.
use super::{character_names, CompileErrorKind, Limits, MatchError, Utf16Regex};
use alloc::{format, vec::Vec};

fn compile(pattern: &str, flags: &str) -> Result<Utf16Regex, super::CompileError> {
    Utf16Regex::with_flags(&pattern.encode_utf16().collect::<Vec<_>>(), flags)
}

#[test]
fn syntax_validation_retains_flags_and_scans_past_supported_syntax() {
    assert_eq!(
        compile(r"\p{alpha}", "u").unwrap_err().kind,
        CompileErrorKind::Syntax
    );
    assert!(compile(r"\p{alpha}", "Uu").is_ok());
    assert!(compile(r"\p{GC=Lu}", "u").is_ok());
    for pattern in [r"\X", r"\b{g}"] {
        let regex = compile(pattern, "u").unwrap();
        assert!(matches!(
            regex.find(&[b'a' as u16], Limits::default()),
            Err(MatchError::Unsupported(_))
        ));
        assert_eq!(
            compile(&format!("{pattern}("), "u").unwrap_err().kind,
            CompileErrorKind::Syntax
        );
    }
    for pattern in [r"[\X]", r"[\b{g}]", r"\p{Lx", r"\p{\q}", r"\p{L\q"] {
        assert_eq!(
            compile(pattern, "u").unwrap_err().kind,
            CompileErrorKind::Syntax
        );
    }
    assert!(compile("(?c)", "u").is_ok());
    assert_eq!(
        compile("(?c)(", "u").unwrap_err().kind,
        CompileErrorKind::Syntax
    );
    let deep = format!("{}a{}", "(".repeat(257), ")".repeat(257));
    assert_eq!(
        compile(&deep, "u").unwrap_err().kind,
        CompileErrorKind::NestingBudgetExceeded
    );
}

#[test]
fn generic_names_use_pinned_full_normalization_and_canonical_fallback() {
    let lookup =
        |s: &str| character_names::code_point_of(&s.chars().map(u32::from).collect::<Vec<_>>());
    assert_eq!(lookup("\tlatin capital letter a\r"), Some(0x41));
    assert_eq!(lookup("LATıN CAPITAL LETTER A"), Some(0x41));
    assert_eq!(lookup("LATIN SMALL LIGATURE ﬄ"), Some(0xfb04));
    assert_eq!(lookup("\u{a0}LATIN CAPITAL LETTER A"), None);
    assert_eq!(lookup("PRIVATE USE AREA E000"), Some(0xe000));
    assert_eq!(lookup("PRIVATE USE AREA 0E000"), None);
    assert_eq!(lookup("BASIC LATIN 41"), None);
    assert_eq!(lookup("NO SUCH CHARACTER"), None);
    let regex = compile(r"\N{LATIN CAPITAL LETTER A}", "u").unwrap();
    assert!(regex
        .captures_entire(&[0x41], Limits::default())
        .unwrap()
        .is_some());
    assert!(regex
        .captures_entire(&[0x42], Limits::default())
        .unwrap()
        .is_none());
    assert_eq!(
        compile(r"\N{NO SUCH CHARACTER}", "u").unwrap_err().kind,
        CompileErrorKind::Syntax
    );
}
