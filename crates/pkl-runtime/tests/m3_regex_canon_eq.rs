//! Java 21 `Pattern.CANON_EQ` and inline `(?c)` canonical equivalence tests.
//! Every expected match span below was verified directly against the pinned
//! Eclipse Adoptium Temurin `21.0.13.1+2-LTS` JDK and Pkl 0.32.0 evaluator.

use java_regex::utf16::{CompileErrorKind, Limits, Utf16Regex};
use pkl_runtime::evaluation::frontend::{ModuleSourceInput, evaluate_module_output};

fn find_span(pattern: &str, flags: &str, input: &str) -> Option<(usize, usize)> {
    let regex = Utf16Regex::from_str(pattern, flags).expect("valid pattern");
    let units: Vec<u16> = input.encode_utf16().collect();
    regex
        .find(&units, Limits::default())
        .expect("matching succeeds")
        .map(|m| (m.start, m.end))
}

type InlineCase = (
    &'static str,
    &'static str,
    &'static str,
    Option<(usize, usize)>,
);
type FlagCase = (&'static str, &'static str, Option<(usize, usize)>);

#[test]
fn inline_canon_eq_matches_jdk21_nfc_char_property_semantics() {
    let cases: &[InlineCase] = &[
        // Basic inline (?c) on bare literals vs character classes (JDK 21 normalize() runs before parsing (?c))
        ("(?c)a", "", "a", Some((0, 1))),
        ("(?-c)a", "", "a", Some((0, 1))),
        ("(?c:a)", "", "a", Some((0, 1))),
        ("(?c)\u{00e9}", "", "e\u{0301}", None),
        ("(?c)e\u{0301}", "", "\u{00e9}", None),
        ("(?c)[\u{00e9}]", "", "e\u{0301}", Some((0, 2))),
        ("(?c)[\u{00e9}]", "", "\u{00e9}", Some((0, 1))),
        ("(?c)[^\u{00e9}]", "", "e\u{0301}", Some((1, 2))),
        ("(?c)[^\u{00e9}]", "", "\u{00e9}", None),
        ("(?c)[^\u{00e9}]", "", "a", Some((0, 1))),
        ("(?c)[\u{00e8}-\u{00ea}]", "", "e\u{0301}", Some((0, 2))),
        ("(?c)[a\u{00e9}]", "", "e\u{0301}", Some((0, 2))),
        ("(?c)[[\u{00e9}]]", "", "e\u{0301}", Some((0, 2))),
        (
            "(?c)[[\u{00e9}]&&[\u{00e9}]]",
            "",
            "e\u{0301}",
            Some((0, 2)),
        ),
        ("(?c)[\u{00e9}&&[e\u{0301}]]", "", "e\u{0301}", None),
        ("(?c)[\u{00e9}&&[\u{00e9}]]", "", "e\u{0301}", Some((0, 2))),
        ("(?c)[\u{00e9}]+", "", "e\u{0301}\u{00e9}", Some((0, 3))),
        ("(?c)[e]", "", "e\u{0301}", None),
        ("(?c)[e]", "", "e", Some((0, 1))),
        ("(?c)[\u{00e9}]", "", "e\u{0301}\u{0302}", Some((0, 2))),
        (
            "(?c)[\u{00e9}]\u{0302}",
            "",
            "e\u{0301}\u{0302}",
            Some((0, 3)),
        ),
        // Unicode properties use NFCCharProperty; \w and . do not
        (r"(?c)\p{Ll}", "", "e\u{0301}", Some((0, 2))),
        (r"(?c)\p{L}", "", "e\u{0301}", Some((0, 2))),
        (r"(?c)\P{L}", "", "e\u{0301}", Some((1, 2))),
        (
            r"(?c)\p{InLatin-1Supplement}",
            "",
            "e\u{0301}",
            Some((0, 2)),
        ),
        (r"(?c)\p{ASCII}", "", "e\u{0301}", None),
        (r"(?c)\w", "", "e\u{0301}", Some((0, 1))),
        (r"(?cU)\w", "", "e\u{0301}", Some((0, 1))),
        (r"(?c).", "", "e\u{0301}", Some((0, 1))),
        (r"(?c)a\p{Ll}", "", "ae\u{0301}", Some((0, 3))),
        (r"(?c)a\p{Ll}+", "", "ae\u{0301}\u{00e9}", Some((0, 4))),
        (r"(?c)\pL", "", "e\u{0301}", Some((0, 2))),
        (r"(?c)x\pL", "", "xe\u{0301}", Some((0, 3))),
        // Hangul syllable composition and multi-mark reordering
        ("(?c)[\u{ac00}]", "", "\u{1100}\u{1161}", Some((0, 2))),
        (
            "(?c)[\u{ac01}]",
            "",
            "\u{1100}\u{1161}\u{11a8}",
            Some((0, 3)),
        ),
        ("(?c)[\u{ac01}]", "", "\u{ac00}\u{11a8}", Some((0, 2))),
        ("(?c)[\u{1ec7}]", "", "e\u{0302}\u{0323}", Some((0, 3))),
        ("(?c)[\u{1ec7}]", "", "e\u{0323}\u{0302}", Some((0, 3))),
        ("(?c)[\u{1ec7}]", "", "\u{00ea}\u{0323}", Some((0, 2))),
        ("(?c)[\u{1ec7}]", "", "\u{1eb9}\u{0302}", Some((0, 2))),
        // Flag scoping & branch persistence
        ("(?c:[\u{00e9}])[\u{00e9}]", "", "e\u{0301}e\u{0301}", None),
        (
            "(?c)[\u{00e9}](?-c)[\u{00e9}]",
            "",
            "e\u{0301}e\u{0301}",
            None,
        ),
        (
            "(?c)[\u{00e9}](?-c)[\u{00e9}]",
            "",
            "e\u{0301}\u{00e9}",
            Some((0, 3)),
        ),
        ("(?c)[\u{00e9}]|[\u{00e8}]", "", "e\u{0300}", Some((0, 2))),
        ("((?c)[\u{00e9}])|[\u{00e8}]", "", "e\u{0300}", None),
        // Case-insensitive + UNICODE_CASE + (?c)
        ("(?ic)[\u{00e9}]", "u", "E\u{0301}", Some((0, 2))),
        ("(?ic)[\u{00e9}]", "u", "\u{00c9}", Some((0, 1))),
        ("(?ic)[\u{00e9}]", "", "E\u{0301}", None),
        // Quantifier backtracking & deterministic=false on NFCCharProperty
        (
            "(?c)[\u{00ea}\u{1ec7}]\u{0323}",
            "",
            "e\u{0302}\u{0323}",
            Some((0, 3)),
        ),
        (
            "(?c)[\u{00ea}\u{1ec7}]+\u{0323}",
            "",
            "e\u{0302}\u{0323}",
            None,
        ),
        (
            "(?c)[\u{00ea}\u{1ec7}]{1,2}\u{0323}",
            "",
            "e\u{0302}\u{0323}",
            None,
        ),
        (
            "(?c)(?:[\u{00ea}\u{1ec7}])+\u{0323}",
            "",
            "e\u{0302}\u{0323}",
            Some((0, 3)),
        ),
        (
            "(?c)[\u{00ea}\u{1ec7}]*?\u{0323}",
            "",
            "e\u{0302}\u{0323}",
            Some((2, 3)),
        ),
        (
            "(?c)[\u{00ea}\u{1ec7}]++\u{0323}",
            "",
            "e\u{0302}\u{0323}",
            None,
        ),
        // Lookbehind study() behavior (minLength += 1, maxLength += 0)
        ("(?c)(?<=[\u{00e9}])x", "", "e\u{0301}x", None),
        ("(?c)(?<=[\u{00e9}])x", "", "\u{00e9}x", None),
        ("(?c)(?<=[\u{00e9}]{0,2})x", "", "\u{00e9}x", Some((1, 2))),
        ("(?c)(?<=[\u{00e9}]+)x", "", "\u{00e9}x", None),
        // Composition exclusions (Angstrom U+212B, Musical Symbol U+1D15E)
        ("(?c)[\u{212b}]", "", "A\u{030a}", None),
        ("(?c)[\u{212b}]", "", "\u{00c5}", None),
        ("(?c)[\u{1d15e}]", "", "\u{1d157}\u{1d165}", None),
        ("(?c)[\u{1d15e}]", "", "\u{1d15e}", Some((0, 2))),
        // Supplementary chars in text before match
        (r"(?c)[\p{L}]", "", "\u{1f600}e\u{0301}", Some((2, 4))),
    ];

    for &(pat, flags, input, expected) in cases {
        let actual = find_span(pat, flags, input);
        assert_eq!(
            actual, expected,
            "mismatch for pat={pat:?} flags={flags:?} input={input:?}"
        );
    }
}

#[test]
fn compile_time_canon_eq_flag_matches_jdk21_normalize_semantics() {
    let cases: &[FlagCase] = &[
        ("\u{00e9}", "e\u{0301}", Some((0, 2))),
        ("e\u{0301}", "\u{00e9}", Some((0, 1))),
        ("[\u{00e9}]", "e\u{0301}", Some((0, 2))),
        ("[^\u{00e9}]", "e\u{0301}", Some((1, 2))),
        ("[\u{00e8}-\u{00ea}]", "e\u{0301}", Some((0, 2))),
        ("[e\u{0301}]", "\u{00e9}", Some((0, 1))),
        ("a\u{0301}\u{0327}", "a\u{0327}\u{0301}", Some((0, 3))),
        ("a\u{0301}\u{0327}", "\u{00e1}\u{0327}", Some((0, 2))),
        ("\u{00e1}\u{0327}", "a\u{0327}\u{0301}", Some((0, 3))),
        ("\u{1ec7}", "e\u{0302}\u{0323}", Some((0, 3))),
        ("\u{1ec7}", "e\u{0323}\u{0302}", Some((0, 3))),
        ("\u{1ec7}", "\u{00ea}\u{0323}", Some((0, 2))),
        ("\u{1ec7}", "\u{1eb9}\u{0302}", Some((0, 2))),
        ("(?-c)\u{00e9}", "e\u{0301}", Some((0, 2))),
        ("(?-c)[\u{00e9}]", "e\u{0301}", None),
        ("\u{212b}", "\u{00c5}", Some((0, 1))),
        ("[\u{212b}]", "\u{00c5}", Some((0, 1))),
        ("\u{00c5}", "\u{212b}", None),
        ("(?<=e\u{0301})x", "\u{00e9}x", Some((1, 2))),
        ("(?<=\u{00e9})x", "e\u{0301}x", Some((2, 3))),
        (r"\Q\u00e9\E", "e\u{0301}", None),
        ("\\\\[\u{00e9}]", "\\e\u{0301}", Some((0, 3))),
        ("\u{1d15e}", "\u{1d157}\u{1d165}", Some((0, 4))),
    ];

    for &(pat, input, expected) in cases {
        let actual = find_span(pat, "c", input);
        assert_eq!(
            actual, expected,
            "CANON_EQ mismatch for pat={pat:?} input={input:?}"
        );
    }

    // Syntax error uses normalized pattern and index (JDK 21: "(?:é|é)[" index 8)
    let err = Utf16Regex::from_str("\u{00e9}[", "c").unwrap_err();
    assert_eq!(err.kind, CompileErrorKind::Syntax);
    assert_eq!(err.message, "Unclosed character class");

    // > 12 combining marks triggers bounded budget failure instead of factorial blowup
    let mut many_marks = String::from("a");
    for offset in 0..13u32 {
        many_marks.push(char::from_u32(0x0300 + offset).unwrap());
    }
    let err = Utf16Regex::from_str(&many_marks, "c").unwrap_err();
    assert_eq!(err.kind, CompileErrorKind::NestingBudgetExceeded);
}

#[test]
fn pkl_language_regex_inline_canon_eq_matches_java_0_32_0() {
    let source = r#"
a = Regex("(?c)[\u{00e9}]").findMatchesIn("e\u{0301}").length
b = Regex("(?c)[\u{00e9}]").matchEntire("e\u{0301}")?.value
c = Regex("(?c)\u{00e9}").findMatchesIn("e\u{0301}").length
d = Regex("(?c)\\p{Ll}").findMatchesIn("e\u{0301}").length
e = Regex("(?ic)[\u{00e9}]").findMatchesIn("E\u{0301}").length
f = Regex("(?c)[\u{1ec7}]").findMatchesIn("e\u{0323}\u{0302}").length
g = Regex("(?c)[\u{ac01}]").findMatchesIn("\u{1100}\u{1161}\u{11a8}").length
h = Regex("(?-c)a").findMatchesIn("a").length
i = Regex("(?c:[\u{00e9}])[\u{00e9}]").findMatchesIn("e\u{0301}e\u{0301}").length
j = Regex("(?c)[\u{00e9}](?-c)[\u{00e9}]").findMatchesIn("e\u{0301}\u{00e9}").length
"#;
    let res = evaluate_module_output(
        ModuleSourceInput {
            text: source,
            uri: "file:///m3-regex-canon-eq/main.pkl",
            inferred_name: "main",
        },
        &Default::default(),
    );
    assert_eq!(
        res.exit_code,
        0,
        "stderr: {}",
        String::from_utf8_lossy(&res.stderr)
    );
    let out = String::from_utf8(res.stdout).unwrap();
    assert_eq!(
        out,
        "a = 1\nb = \"e\u{0301}\"\nc = 0\nd = 1\ne = 1\nf = 1\ng = 1\nh = 1\ni = 0\nj = 1\n"
    );
}
