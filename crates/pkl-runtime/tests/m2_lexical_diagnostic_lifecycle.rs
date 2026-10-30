//! Lexical diagnostic locations and failure release on the bounded native stack.
//! Complete Java bytes are compared separately by the platform/language suites.
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use pkl_runtime::diagnostic::DiagnosticKind;
use pkl_runtime::parser::{parse_expression, parse_module};
use pkl_runtime::source::Span;
use pkl_runtime::value::Value;

fn isolated(name: &str, work: fn()) {
    if std::env::var("PKL_LEXICAL_DIAGNOSTIC_CHILD").as_deref() == Ok(name) {
        std::thread::Builder::new()
            .name("lexical-diagnostic-256k".into())
            .stack_size(256 * 1024)
            .spawn(work)
            .unwrap()
            .join()
            .unwrap();
        println!("lexical diagnostic lifecycle completed: {name}; stack=262144");
        return;
    }
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", name, "--nocapture"])
        .env("PKL_LEXICAL_DIAGNOSTIC_CHILD", name)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            assert!(
                status.success(),
                "isolated lexical diagnostic {name}: {status}"
            );
            return;
        }
        if Instant::now() >= deadline {
            child.kill().unwrap();
            let status = child.wait().unwrap();
            panic!("lexical diagnostic {name} exceeded 60 seconds: {status}");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn nested(prefix: &str, leaf: &str, suffix: &str, depth: usize) -> String {
    format!("{}{leaf}{}", prefix.repeat(depth), suffix.repeat(depth))
}

fn recover() {
    let expression = nested("\"\\(", "42", ")\"", 31);
    assert_eq!(
        pkl_runtime::probe::evaluate_member(format!("result = {expression}\n"), "result"),
        Ok(Value::String("42".into()))
    );
}

fn rejected(expression: &str, message: &str, span: Span) {
    let error = parse_expression(expression).unwrap_err();
    assert_eq!(error.kind, DiagnosticKind::Syntax);
    assert_eq!(error.message, message);
    assert_eq!(error.span, span);
    drop(error);
    // A completed deep sibling must be released when the following member fails.
    // Unicode before the failure keeps byte offsets distinct from UTF-16 columns.
    let header = format!(
        "stable = {}\nemoji = \"🙂\"\nresult = ",
        nested("\"\\(", "42", ")\"", 31)
    );
    let error = parse_module(&format!("{header}{expression}")).unwrap_err();
    assert_eq!(error.kind, DiagnosticKind::Syntax);
    assert_eq!(error.message, message);
    assert_eq!(
        error.span,
        Span::new(header.len() + span.start, header.len() + span.end)
    );
    drop(error);
    // Parser failures have no live evaluator. This is fresh successful work on
    // the same small-stack thread, not a retained-evaluator recovery assertion.
    recover();
}

#[test]
fn nested_lexical_error_priority_spans_and_eof_cleanup_on_small_stack() {
    isolated(
        "nested_lexical_error_priority_spans_and_eof_cleanup_on_small_stack",
        || {
            let invalid_escape = "Invalid character escape sequence `\\q`.\n\nValid character escape sequences are:  \\n  \\r  \\t  \\\"  \\\\ \\<newline>";
            for depth in [24, 31] {
                for (prefix, suffix, leaf, escape) in [
                    ("\"\\(", ")\"", "\"bad\\q\"", "\\q"),
                    ("#\"\\#(", ")\"#", "##\"bad\\##q\"##", "#q"),
                    ("\"\"\"\n\\(", ")\n\"\"\"", "\"bad\\q\"", "\\q"),
                ] {
                    // The invalid leaf wins over the missing outer delimiter.
                    let expression = nested(prefix, leaf, suffix, depth);
                    let expression = &expression[..expression.len() - suffix.len()];
                    let start = expression.find(escape).unwrap();
                    rejected(expression, invalid_escape, Span::new(start, start + 2));
                }
                // Newline errors retain the newline; comments instead point at
                // the zero-width original EOF, including its final newline.
                let expression = format!("{}\"inner\n", "\"\\(".repeat(depth - 1));
                rejected(
                    &expression,
                    "Missing `\"` delimiter.",
                    Span::new(expression.len() - 1, expression.len()),
                );
                let expression = format!("{}/* unterminated )\"\n", "\"\\(".repeat(depth));
                rejected(
                    &expression,
                    "Unexpected end of file.",
                    Span::new(expression.len(), expression.len()),
                );
            }
        },
    );
}

#[test]
fn malformed_multiline_diagnostic_spans_and_guard_cleanup_on_small_stack() {
    isolated(
        "malformed_multiline_diagnostic_spans_and_guard_cleanup_on_small_stack",
        || {
            for depth in [24, 31] {
                let expression = nested("\"\"\"x\\(", "42", ")\n\"\"\"", depth);
                let start = "\"\"\"x\\(".len() * (depth - 1) + 3;
                rejected(
                    &expression,
                    "The content of a multi-line string must begin on a new line.",
                    Span::new(start, start + 1),
                );
                let expression = nested("\"\"\"\n\\(", "42", ")x\"\"\"", depth);
                let start = "\"\"\"\n\\(".len() * (depth - 1);
                let end = expression.find("x\"\"\"").unwrap() + 4;
                rejected(
                    &expression,
                    "The closing delimiter of a multi-line string must begin on a new line.",
                    Span::new(start, end),
                );
                let expression = nested("\"\"\"\nx\\(", "42", ")\n  \"\"\"", depth);
                let start = "\"\"\"\nx\\(".len() * (depth - 1) + 4;
                rejected(
                    &expression,
                    "Line must match or exceed indentation of the String's last line.",
                    Span::new(start, start + 1),
                );
            }
            for (prefix, suffix) in [
                ("\"\\(", ")\""),
                ("#\"\\#(", ")\"#"),
                ("\"\"\"\n\\(", ")\n\"\"\""),
            ] {
                let expression = nested(prefix, "42", suffix, 64);
                let error = parse_expression(&expression).unwrap_err();
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
