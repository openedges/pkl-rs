//! Golden snippet tests, idempotency checks, grammar version tests, and parser/lexer edge cases
//! mirroring `FormatterSnippetTests`, `FormatterTest.kt`, and `LexerTest.kt`.

use pkl_formatter::{FormatError, Formatter, GrammarVersion, format_str, format_str_latest};
use std::fs;
use std::path::{Path, PathBuf};

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crates/")
        .parent()
        .expect("workspace root")
        .to_path_buf()
}

fn snippet_dirs() -> (PathBuf, PathBuf) {
    let base =
        workspace_root().join("external/pkl/pkl-formatter/src/test/files/FormatterSnippetTests");
    (base.join("input"), base.join("output"))
}

#[test]
fn all_30_upstream_formatter_snippets_match_raw_bytes_and_are_idempotent() {
    let (input_dir, output_dir) = snippet_dirs();
    let mut entries: Vec<PathBuf> = fs::read_dir(&input_dir)
        .expect("read FormatterSnippetTests/input")
        .map(|e| e.expect("dir entry").path())
        .filter(|p| p.extension().is_some_and(|ext| ext == "pkl"))
        .collect();
    entries.sort();

    assert_eq!(
        entries.len(),
        30,
        "expected exactly 30 upstream FormatterSnippetTests inputs"
    );

    let formatter = Formatter::default();
    let mut failures = Vec::new();

    for input_path in &entries {
        let file_name = input_path.file_name().expect("file name");
        let expected_path = output_dir.join(file_name);
        let input = fs::read_to_string(input_path).expect("read input");
        let expected = fs::read_to_string(&expected_path).expect("read expected output");

        match formatter.format(&input) {
            Ok(actual) => {
                if actual.as_bytes() != expected.as_bytes() {
                    failures.push(format!(
                        "MISMATCH in {}:\n--- EXPECTED ({} bytes) ---\n{}\n--- ACTUAL ({} bytes) ---\n{}",
                        file_name.to_string_lossy(),
                        expected.len(),
                        expected,
                        actual.len(),
                        actual,
                    ));
                    continue;
                }
            }
            Err(err) => {
                failures.push(format!(
                    "ERROR formatting {}: {err}",
                    file_name.to_string_lossy()
                ));
                continue;
            }
        }

        // Second-pass stability check matching FormatterTest.kt `snippet test output must be stable`
        match formatter.format(&expected) {
            Ok(reformatted) => {
                if reformatted.as_bytes() != expected.as_bytes() {
                    failures.push(format!(
                        "NOT IDEMPOTENT in {}:\n--- EXPECTED ---\n{}\n--- REFORMATTED ---\n{}",
                        file_name.to_string_lossy(),
                        expected,
                        reformatted,
                    ));
                }
            }
            Err(err) => {
                failures.push(format!(
                    "ERROR reformatting {}: {err}",
                    file_name.to_string_lossy()
                ));
            }
        }
    }

    assert!(
        failures.is_empty(),
        "{} formatter snippet failures:\n{}",
        failures.len(),
        failures.join("\n\n")
    );
}

#[test]
fn grammar_version_v1_vs_v2_trailing_commas() {
    let input = "\
function longFunc(parameter1: Parameter1Type, parameter2: Parameter2Type, parameter3: Parameter3Type, parameter4: Parameter4Type): String = \"\"

args = fun(\"loooooooooooooooooooongString\", \"loooooooooooooooongString\", \"looooooooooooooooongString\", \"notTooLong\")

lambda = (fooooooooooooooooooooooooooooooooo, baaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaar, baaaaaaaaaaaaaaaaaaaaaz) -> 1

constrained: String(isSomethingSomethingSomething, isSomethingElse, isSomethingSomethingSomethingElse)

trailingCommaInTypeParameters: Mapping<SomethingSomethingSomethingSomething, SomethingSomething | SomethingElse>

class Generic<TypeParameterOneeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee, TypeParameterTwooooooooooooooooooooooooooooooooo> {}

funcType: (ReaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaalyLongType, ReaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaalyLongType2) -> String

trailingLambda = qux(bar, (a) -> helloLoooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooong)
";

    let v1 = format_str(input, GrammarVersion::V1).expect("V1 format");
    let v2 = format_str(input, GrammarVersion::V2).expect("V2 format");

    // V1 omits trailing commas in all 3 wrapped list categories
    assert!(v1.contains("  parameter4: Parameter4Type\n): String"));
    assert!(v1.contains("    \"notTooLong\"\n  )"));
    assert!(v1.contains("  baaaaaaaaaaaaaaaaaaaaaz\n) -> 1"));
    assert!(v1.contains("  isSomethingSomethingSomethingElse\n)"));
    assert!(v1.contains("  SomethingSomething | SomethingElse\n>"));
    assert!(v1.contains("  TypeParameterTwooooooooooooooooooooooooooooooooo\n> {}"));
    assert!(v1.contains("  ReaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaalyLongType2\n) ->"));

    // V2 emits trailing commas in all 3 wrapped list categories
    assert!(v2.contains("  parameter4: Parameter4Type,\n): String"));
    assert!(v2.contains("    \"notTooLong\",\n  )"));
    assert!(v2.contains("  baaaaaaaaaaaaaaaaaaaaaz,\n) -> 1"));
    assert!(v2.contains("  isSomethingSomethingSomethingElse,\n)"));
    assert!(v2.contains("  SomethingSomething | SomethingElse,\n>"));
    assert!(v2.contains("  TypeParameterTwooooooooooooooooooooooooooooooooo,\n> {}"));
    assert!(v2.contains("  ReaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaalyLongType2,\n) ->"));

    // Trailing lambda never gets trailing comma in either V1 or V2
    assert!(!v1.contains(
        "helloLoooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooong,"
    ));
    assert!(!v2.contains(
        "helloLoooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooong,"
    ));

    // Both V1 and V2 outputs are idempotent under their own version
    assert_eq!(format_str(&v1, GrammarVersion::V1).unwrap(), v1);
    assert_eq!(format_str(&v2, GrammarVersion::V2).unwrap(), v2);
}

#[test]
fn formatter_test_multiline_string_wrong_start_quote() {
    let err = format_str_latest("foo = \"\"\"line1\n  line 2\n  \"\"\"").unwrap_err();
    assert!(matches!(err, FormatError::Parser { .. }));
    assert!(
        err.to_string()
            .contains("The content of a multi-line string must begin on a new line.")
    );
}

#[test]
fn formatter_test_multiline_string_wrong_end_quote() {
    let err = format_str_latest("foo = \"\"\"\n  line1\n  line 2\"\"\"").unwrap_err();
    assert!(matches!(err, FormatError::Parser { .. }));
    assert!(
        err.to_string()
            .contains("The closing delimiter of a multi-line string must begin on a new line.")
    );
}

#[test]
fn formatter_test_multiline_string_wrong_indentation() {
    let err = format_str_latest("foo = \"\"\"\n  line1\nline 2\n  \"\"\"").unwrap_err();
    assert!(matches!(err, FormatError::Parser { .. }));
    assert!(
        err.to_string()
            .contains("Line must match or exceed indentation of the String's last line.")
    );
}

#[test]
fn formatter_test_whitespace_only() {
    for src in ["", ";;;", "\n", "\n\n\n", "\t"] {
        assert_eq!(
            format_str_latest(src).expect("format whitespace"),
            "\n",
            "input: {src:?}"
        );
    }
}

#[test]
fn formatter_test_format_to_streaming() {
    let mut output = String::new();
    Formatter::default()
        .format_to("  x  =  42", &mut output)
        .expect("format_to");
    assert_eq!(output, "x = 42\n");
}

#[test]
fn lexer_and_parser_error_formatting_matches_upstream() {
    // Lexer ParserError Display matches Java Throwable.toString()
    let err = format_str_latest("foo = \"/foo/\\${BAR}/baz\"").unwrap_err();
    assert!(matches!(err, FormatError::Lexer { .. }));
    assert_eq!(
        err.to_string(),
        "org.pkl.parser.ParserError: Invalid character escape sequence `\\$`.\n\nValid character escape sequences are:  \\n  \\r  \\t  \\\"  \\\\ \\<newline>"
    );

    // Single backtick
    let err = format_str_latest("`").unwrap_err();
    assert!(matches!(err, FormatError::Lexer { .. }));
    assert_eq!(
        err.to_string(),
        "org.pkl.parser.ParserError: Unexpected character `EOF`. Did you mean `backquote`?"
    );

    // Block comment at EOF without trailing newline matches upstream Lexer quirk
    let err = format_str_latest("/* comment */").unwrap_err();
    assert!(matches!(err, FormatError::Lexer { .. }));
    assert_eq!(
        err.to_string(),
        "org.pkl.parser.ParserError: Unexpected end of file."
    );

    // Parser GenericParserError Display matches `msg at (L:C - L:C)`
    let err = format_str_latest("foo = a { b = 1 }").unwrap_err();
    assert!(matches!(err, FormatError::Parser { .. }));
    assert_eq!(
        err.to_string(),
        "Unexpected token: `{`.\n\nIf you meant to write an amends expression, wrap the parent in parentheses. Try: `(a) { ... }` at (1:9 - 1:10)"
    );

    // Type annotation in amends
    let err = format_str_latest("obj: Bird { name = \"x\" }").unwrap_err();
    assert!(
        err.to_string()
            .contains("Properties with type annotations cannot have object bodies.")
    );
}

#[test]
fn crlf_and_cr_line_continuations_and_unicode_comments() {
    // CRLF line continuation
    let input = "x = \"\"\"\n  hello \\\r\n  world\r\n  \"\"\"";
    let formatted = format_str_latest(input).expect("CRLF continuation");
    assert_eq!(
        formatted,
        "x =\n  \"\"\"\n  hello \\\r\n  world\n  \"\"\"\n"
    );

    // Bare CR line continuation
    let input = "x = \"\"\"\n  hello \\\r  world\n  \"\"\"";
    let formatted = format_str_latest(input).expect("CR continuation");
    assert_eq!(formatted, "x =\n  \"\"\"\n  hello \\\n  world\n  \"\"\"\n");

    // Whitespace between \ and \r\n is an error
    let err = format_str_latest("x = \"\"\"\n  hello \\ \r\n  world\n  \"\"\"").unwrap_err();
    assert!(err.to_string().contains("Whitespace"));

    // Special codepoints in comments including U+0000, U+7FFF, U+FFFF
    for cp in [0x0000u32, 0x007f, 0x7fff, 0xffff] {
        let ch = char::from_u32(cp).unwrap();
        let src = format!("// Test {ch}\nmodule Test\n");
        let out = format_str_latest(&src).expect("unicode comment");
        assert_eq!(out, src);
    }

    // Unicode identifier (Devanagari from LexerTest.kt)
    let src = "जावास्क्रिप्ट = 1";
    assert_eq!(format_str_latest(src).unwrap(), "जावास्क्रिप्ट = 1\n");
}

#[test]
fn all_stdlib_pkl_files_format_and_are_idempotent() {
    let stdlib_dir = workspace_root().join("external/pkl/stdlib");
    let mut count = 0;
    for entry in fs::read_dir(&stdlib_dir).expect("read stdlib") {
        let path = entry.expect("entry").path();
        if path.extension().is_some_and(|e| e == "pkl") {
            let src = fs::read_to_string(&path).expect("read stdlib file");
            let first = format_str_latest(&src)
                .unwrap_or_else(|e| panic!("failed to format {}: {e}", path.display()));
            let second = format_str_latest(&first)
                .unwrap_or_else(|e| panic!("failed to reformat {}: {e}", path.display()));
            assert_eq!(
                first,
                second,
                "stdlib file not idempotent: {}",
                path.display()
            );
            count += 1;
        }
    }
    assert!(
        count >= 23,
        "expected at least 23 stdlib pkl files, got {count}"
    );
}

#[test]
fn m5_format_fixtures_match_exact_oracle_outputs_and_error_spans() {
    let fmt_dir = workspace_root().join("tests/fixtures/m5/format");

    // Exact V1 and V2 outputs for grammar_trailing_comma.pkl
    let tc_src = fs::read_to_string(fmt_dir.join("grammar_trailing_comma.pkl")).unwrap();
    let v1 = format_str(&tc_src, GrammarVersion::V1).unwrap();
    let v2 = format_str(&tc_src, GrammarVersion::V2).unwrap();
    assert_eq!(
        v1,
        "function veryLongFunctionNameForTestingTrailingCommaBehavior(\n  firstLongParameterName: String,\n  secondLongParameterName: Int,\n  thirdLongParameterName: Boolean\n): String =\n  someLongCallName(\n    firstLongParameterName,\n    secondLongParameterName,\n    thirdLongParameterName\n  )\n\ntypealias LongGenericType<\n  FirstVeryLongTypeParameterName,\n  SecondVeryLongTypeParameterName,\n  ThirdVeryLongTypeParameterName\n> =\n  Map<\n    FirstVeryLongTypeParameterName,\n    Pair<SecondVeryLongTypeParameterName, ThirdVeryLongTypeParameterName>\n  >\n"
    );
    assert_eq!(
        v2,
        "function veryLongFunctionNameForTestingTrailingCommaBehavior(\n  firstLongParameterName: String,\n  secondLongParameterName: Int,\n  thirdLongParameterName: Boolean,\n): String =\n  someLongCallName(\n    firstLongParameterName,\n    secondLongParameterName,\n    thirdLongParameterName,\n  )\n\ntypealias LongGenericType<\n  FirstVeryLongTypeParameterName,\n  SecondVeryLongTypeParameterName,\n  ThirdVeryLongTypeParameterName,\n> =\n  Map<\n    FirstVeryLongTypeParameterName,\n    Pair<SecondVeryLongTypeParameterName, ThirdVeryLongTypeParameterName>,\n  >\n"
    );

    // Exact multiline string error spans
    let start_src = fs::read_to_string(fmt_dir.join("multiline_err_start.pkl")).unwrap();
    let end_src = fs::read_to_string(fmt_dir.join("multiline_err_end.pkl")).unwrap();
    let indent_src = fs::read_to_string(fmt_dir.join("multiline_err_indent.pkl")).unwrap();
    assert_eq!(
        format_str_latest(&start_src).unwrap_err().to_string(),
        "The content of a multi-line string must begin on a new line. at (1:10 - 1:15)"
    );
    assert_eq!(
        format_str_latest(&end_src).unwrap_err().to_string(),
        "The closing delimiter of a multi-line string must begin on a new line. at (3:1 - 3:9)"
    );
    assert_eq!(
        format_str_latest(&indent_src).unwrap_err().to_string(),
        "Line must match or exceed indentation of the String's last line. at (3:1 - 3:7)"
    );
}

#[test]
fn language_snippet_tests_corpus_never_panics_and_formatted_outputs_are_idempotent() {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        for entry in fs::read_dir(dir).expect("read dir") {
            let path = entry.expect("entry").path();
            if path.is_dir() {
                walk(&path, out);
            } else if path.extension().is_some_and(|e| e == "pkl") {
                out.push(path);
            }
        }
    }

    let root =
        workspace_root().join("external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input");
    let mut files = Vec::new();
    walk(&root, &mut files);
    files.sort();

    let mut formatted_ok = 0usize;
    let mut non_idempotent = Vec::new();
    for path in &files {
        let Ok(src) = fs::read_to_string(path) else {
            continue;
        };
        if let Ok(first) = format_str_latest(&src) {
            formatted_ok += 1;
            match format_str_latest(&first) {
                Ok(second) => {
                    if first != second {
                        non_idempotent.push(path.display().to_string());
                    }
                }
                Err(err) => {
                    panic!(
                        "Formatted output failed to re-parse for {}: {err}\n--- OUTPUT ---\n{first}",
                        path.display()
                    );
                }
            }
        }
    }
    assert!(
        formatted_ok > 300,
        "expected >300 valid snippet files, got {formatted_ok}"
    );
    assert!(
        non_idempotent.is_empty(),
        "non-idempotent language snippets ({}): {:?}",
        non_idempotent.len(),
        &non_idempotent[..non_idempotent.len().min(10)]
    );
}

#[test]
fn wave2_formatter_malformed_and_adversarial_inputs_do_not_panic() {
    // Previously panicked via OOB token slice, unreachable!(), or assert!(!self.no_newlines).
    for bad in ["a = (", "import \"a\\\n\"", "a = \"\\(1 // c\n)\""] {
        let _ = format_str_latest(bad);
    }

    // Deep nesting exceeds MAX_PARSE_DEPTH cleanly without stack overflow.
    let deep = format!("a = {}1{}\n", "(".repeat(400), ")".repeat(400));
    assert!(
        format_str_latest(&deep).is_err(),
        "deeply nested expression must fail cleanly with depth error"
    );

    // Moderate nesting within MAX_PARSE_DEPTH formats cleanly on the default thread stack.
    let valid_nested = format!("a = {}1{}\n", "(".repeat(64), ")".repeat(64));
    assert!(format_str_latest(&valid_nested).is_ok());

    // Nested object parameter lists must format in linear time (no exponential cloning).
    let mut nested = String::from("a = ");
    for i in 0..24 {
        nested.push_str(&format!("new {{ p{i} -> "));
    }
    nested.push('1');
    for _ in 0..24 {
        nested.push_str(" }");
    }
    nested.push('\n');
    let formatted = format_str_latest(&nested).expect("nested object param list must format");
    assert!(formatted.contains("p23"));
}

#[test]
fn round2_formatter_semicolon_import_deep_chains_and_width_bounded() {
    // 1. Semicolon-terminated import must not panic and must be idempotent.
    let imp = format_str_latest("import \"a.pkl\";\n").expect("semicolon import formats");
    assert_eq!(imp, "import \"a.pkl\"\n");
    assert_eq!(format_str_latest(&imp).unwrap(), imp);

    // 2. Deep left-leaning / right-leaning chains must not stack overflow.
    let non_null = format!("a = x{}\n", "!!".repeat(300));
    let _ = format_str_latest(&non_null);
    let nullable_type = format!("a: Int{}\n", "?".repeat(300));
    let _ = format_str_latest(&nullable_type);
    let unary_chain = format!("a = {}true\n", "!".repeat(300));
    let _ = format_str_latest(&unary_chain);
    let amends_chain = format!("a = base{}\n", " {}".repeat(300));
    let _ = format_str_latest(&amends_chain);

    // 3. Deep nested groups stay linear with short-circuiting width_bounded.
    let nested_lists = format!("a = {}1{}\n", "List(".repeat(80), ")".repeat(80));
    let out = format_str_latest(&nested_lists).expect("nested lists format quickly");
    assert!(out.contains("List("));
}

#[test]
fn round3_formatter_interleaved_chains_do_not_stack_overflow() {
    // P1-9: Deeply interleaved unary/postfix/member/subscript/coalesce/pipe chains must not overflow stack.
    let mut interleaved = String::from("a = ");
    for _ in 0..350 {
        interleaved.push('!');
    }
    interleaved.push('x');
    for _ in 0..350 {
        interleaved.push_str(".a[0]!!");
    }
    interleaved.push('\n');
    let _ = format_str_latest(&interleaved);

    let mut binary_chain = String::from("b = 1");
    for _ in 0..350 {
        binary_chain.push_str(" ?? 2 + 3 |> f");
    }
    binary_chain.push('\n');
    let _ = format_str_latest(&binary_chain);
}

#[test]
fn round4_formatter_deep_left_chain_and_custom_delim_import_sorting() {
    // P1-12: Deep left-leaning arithmetic chain must format without recursive clone stack overflow.
    let mut deep = String::from("x = 1");
    for _ in 0..4_000 {
        deep.push_str(" + 1");
    }
    deep.push('\n');
    let out = format_str_latest(&deep).expect("deep left-leaning chain must format");
    assert!(out.starts_with("x ="));

    // P2-7: Custom-delimited import `#"pkl:json"#` matches Java `Builder.java:1451`
    // (`substring(1, len-1)` preserves `#`, so it is NOT classified as stdlib `pkl:`).
    let imports = "import #\"pkl:json\"#\nimport \"pkl:base\"\n";
    let formatted = format_str_latest(imports).expect("custom delim import formats");
    assert_eq!(
        formatted, "import \"pkl:base\"\n\nimport #\"pkl:json\"#\n",
        "stdlib import must precede custom-delimited non-stdlib import"
    );
}
