//! Bounded Unicode diagnostic order and partial frontend release.
//! Complete pinned Java bytes are compared by the language/platform suites.
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use pkl_runtime::diagnostic::DiagnosticKind;
use pkl_runtime::parser::{parse_expression, parse_module};
use pkl_runtime::source::Span;
use pkl_runtime::value::Value;

fn isolated(name: &str, work: fn()) {
    if std::env::var("PKL_UNICODE_DIAGNOSTIC_CHILD").as_deref() == Ok(name) {
        std::thread::Builder::new()
            .name("unicode-diagnostic-256k".into())
            .stack_size(256 * 1024)
            .spawn(work)
            .unwrap()
            .join()
            .unwrap();
        println!("stage28 unicode diagnostic completed: {name}; stack=262144");
        return;
    }
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", name, "--nocapture"])
        .env("PKL_UNICODE_DIAGNOSTIC_CHILD", name)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            assert!(
                status.success(),
                "isolated Unicode diagnostic {name}: {status}"
            );
            return;
        }
        if Instant::now() >= deadline {
            child.kill().unwrap();
            let status = child.wait().unwrap();
            panic!("Unicode diagnostic {name} exceeded 60 seconds: {status}");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn nested(prefix: &str, leaf: &str, suffix: &str, depth: usize) -> String {
    format!("{}{leaf}{}", prefix.repeat(depth), suffix.repeat(depth))
}

fn recover() {
    // Each call constructs and releases a fresh evaluator. Parser setup errors
    // do not establish retained-evaluator or Host suspension/reentry behavior.
    for (prefix, suffix, depth) in [
        ("\"\\(", ")\"", 31),
        ("#\"\\#(", ")\"#", 31),
        ("\"\"\"\n\\(", ")\n\"\"\"", 24),
    ] {
        let expression = nested(prefix, "42", suffix, depth);
        assert_eq!(
            pkl_runtime::probe::evaluate_member(&format!("result = {expression}\n"), "result"),
            Ok(Value::String("42".into()))
        );
    }
}

fn invalid_unicode(text: &str, prefix: &str) -> String {
    format!(
        "Invalid Unicode escape sequence `{text}`.\n\nValid Unicode escape sequences are {prefix}{{0}} to {prefix}{{10FFFF}} (1-6 hexadecimal characters)."
    )
}

fn unterminated_unicode(text: &str) -> String {
    format!(
        "Unterminated Unicode escape sequence `{text}`.\n\nUnicode escape sequences must end with `}}`."
    )
}

fn rejected(expression: &str, message: &str, span: Span) {
    let error = parse_expression(expression).unwrap_err();
    assert_eq!(error.kind, DiagnosticKind::Syntax, "{expression:?}");
    assert_eq!(error.message, message, "{expression:?}");
    assert_eq!(error.span, span, "{expression:?}");
    drop(error);
    // A complete depth31 sibling and Unicode source prefix must be dropped
    // when the later member fails. Source positions remain UTF-8 byte spans.
    let header = format!(
        "stable = {}\nemoji = \"🙂\"\nresult = ",
        nested("\"\\(", "42", ")\"", 31)
    );
    let error = parse_module(&format!("{header}{expression}")).unwrap_err();
    assert_eq!(error.kind, DiagnosticKind::Syntax, "{expression:?}");
    assert_eq!(error.message, message, "{expression:?}");
    assert_eq!(
        error.span,
        Span::new(header.len() + span.start, header.len() + span.end)
    );
    drop(error);
    recover();
}

#[test]
fn unicode_semantic_priority_and_nested_cleanup_on_small_stack() {
    isolated(
        "unicode_semantic_priority_and_nested_cleanup_on_small_stack",
        || {
            for depth in [24, 31] {
                for (prefix, suffix, leaf, escape, escape_prefix) in [
                    ("\"\\(", ")\"", "\"\\u{}\"", "\\u{}", "\\u"),
                    (
                        "#\"\\#(",
                        ")\"#",
                        "##\"\\##u{xyz}\"##",
                        "\\##u{xyz}",
                        "\\##u",
                    ),
                    ("\"\"\"\n\\(", ")\n\"\"\"", "\"\\u{}\"", "\\u{}", "\\u"),
                    ("\"\\(", ")\"", "\"\"\"\n\\u{}\n\"\"\"", "\\u{}", "\\u"),
                    ("#\"\\#(", ")\"#", "\"\"\"\n\\u{}\n\"\"\"", "\\u{}", "\\u"),
                    (
                        "\"\"\"\n\\(",
                        ")\n\"\"\"",
                        "\"\"\"\n\\u{}\n\"\"\"",
                        "\\u{}",
                        "\\u",
                    ),
                ] {
                    let expression = nested(prefix, leaf, suffix, depth);
                    let start = expression.find(escape).unwrap();
                    rejected(
                        &expression,
                        &invalid_unicode(escape, escape_prefix),
                        Span::new(start, start + escape.len()),
                    );
                }
                // Ordinary Unicode validation precedes later escapes and missing
                // outer delimiters; an earlier interpolation syntax error wins.
                let expression = nested("\"\\(", "\"\\u{}\\q\"", ")\"", depth);
                let start = expression.find("\\u{}").unwrap();
                rejected(
                    &expression[..expression.len() - 2],
                    &invalid_unicode("\\u{}", "\\u"),
                    Span::new(start, start + 4),
                );
                let expression = nested("\"\\(", "\"\\(1 + )\\u{}\"", ")\"", depth - 1);
                let start = expression.find("1 + )").unwrap() + 4;
                rejected(
                    &expression,
                    "Unexpected token `)`. Expected `)`.",
                    Span::new(start, start + 1),
                );
            }
            for (expression, message, span) in [
                (
                    "\"\\u{}\\q\"",
                    invalid_unicode("\\u{}", "\\u"),
                    Span::new(1, 5),
                ),
                (
                    "\"\\u{}\\(/* unterminated",
                    invalid_unicode("\\u{}", "\\u"),
                    Span::new(1, 5),
                ),
                (
                    "\"\"\"x\\u{}\n\"\"\"",
                    "The content of a multi-line string must begin on a new line.".into(),
                    Span::new(3, 4),
                ),
                (
                    "\"\"\"\n\\u{}x\"\"\"",
                    "The closing delimiter of a multi-line string must begin on a new line.".into(),
                    Span::new(0, 12),
                ),
                (
                    "\"\"\"\n\\u{}\n  \"\"\"",
                    "Line must match or exceed indentation of the String's last line.".into(),
                    Span::new(4, 4),
                ),
            ] {
                rejected(expression, &message, span);
            }
            let expression = "\"\"\"\n\\u{}\\q\n\"\"\"";
            rejected(
                expression,
                "Invalid character escape sequence `\\q`.\n\nValid character escape sequences are:  \\n  \\r  \\t  \\\"  \\\\ \\<newline>",
                Span::new(8, 10),
            );
            let expression = "\"\"\"\n\\u{}\\(1 + )\n\"\"\"";
            let start = expression.find("1 + )").unwrap() + 4;
            rejected(
                expression,
                "Unexpected token `)`. Expected `)`.",
                Span::new(start, start + 1),
            );
            for expression in [
                "\"\"\"\n\\u{}\\(/* unterminated",
                "\"\"\"\n\\u{}\n\"\"\" /*",
            ] {
                rejected(
                    expression,
                    "Unexpected end of file.",
                    Span::new(expression.len(), expression.len()),
                );
            }
        },
    );
}

#[test]
fn unicode_lexical_boundaries_byte_spans_and_guard_cleanup_on_small_stack() {
    isolated(
        "unicode_lexical_boundaries_byte_spans_and_guard_cleanup_on_small_stack",
        || {
            for expression in ["\"😀\\u{xyz}\"", "\"é\\u{xyz}\"", "##\"漢\t\\##u{xyz}\"##"] {
                let start = expression.find('\\').unwrap();
                let end = expression[start..].find('}').unwrap() + start + 1;
                let prefix = if expression.starts_with("##") {
                    "\\##u"
                } else {
                    "\\u"
                };
                rejected(
                    expression,
                    &invalid_unicode(&expression[start..end], prefix),
                    Span::new(start, end),
                );
            }
            for (expression, message, span) in [
                (
                    "\"\\u1234\"",
                    "Unexpected character `1`. Did you mean `{`?".into(),
                    Span::new(3, 4),
                ),
                (
                    "\"\\u😀\"",
                    "Unexpected character `?`. Did you mean `{`?".into(),
                    Span::new(3, 7),
                ),
                (
                    "##\"\\##u!\"##",
                    "Unexpected character `!`. Did you mean `{`?".into(),
                    Span::new(7, 8),
                ),
                (
                    "\"\\u",
                    "Unexpected character `EOF`. Did you mean `{`?".into(),
                    Span::new(2, 2),
                ),
                (
                    "\"\\u{12!}\"",
                    unterminated_unicode("\\u{12"),
                    Span::new(1, 6),
                ),
                ("\"\\u{𝒜}\"", unterminated_unicode("\\u{"), Span::new(1, 4)),
                ("\"\\u{", unterminated_unicode("\\u{"), Span::new(1, 4)),
                (
                    "\"\\u{}\n",
                    "Missing `\"` delimiter.".into(),
                    Span::new(5, 6),
                ),
                (
                    "\"\\u{}x\n",
                    "Missing `\"` delimiter.".into(),
                    Span::new(6, 7),
                ),
            ] {
                rejected(expression, &message, span);
            }
            let error = parse_expression("\"\\u😀\"").unwrap_err();
            assert_eq!(error.highlight_length_utf16, Some(1));
            drop(error);
            for (prefix, suffix) in [
                ("\"\\(", ")\""),
                ("#\"\\#(", ")\"#"),
                ("\"\"\"\n\\(", ")\n\"\"\""),
            ] {
                // Preserve the existing guard; accepting Java depth64 remains a gap.
                let error = parse_expression(&nested(prefix, "42", suffix, 64)).unwrap_err();
                assert_eq!(error.kind, DiagnosticKind::Unsupported);
                assert_eq!(
                    error.message,
                    "String interpolation nesting limit exceeded."
                );
                drop(error);
                recover();
            }
        },
    );
}
