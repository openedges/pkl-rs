//! Earlier frontend rejections retain their enclosing authored owner.
//! Full diagnostic bytes are checked separately against fresh pinned Java.
use pkl_runtime::diagnostic::{Diagnostic, DiagnosticKind};
use pkl_runtime::evaluation::scalar_expression;
use pkl_runtime::parser::parse_module;
use pkl_runtime::resolver::resolve_module;

fn rejection(source: &str) -> Diagnostic {
    resolve_module(&parse_module(source).unwrap()).unwrap_err()
}

fn assert_rejection(source: &str, message: &str, fragment: &str, owner: &str) {
    let error = rejection(source);
    assert_eq!(error.kind, DiagnosticKind::Syntax, "{source}");
    assert_eq!(error.message, message, "{source}");
    assert_eq!(
        &source[error.span.start..error.span.end],
        fragment,
        "{source}"
    );
    assert!(
        error
            .member
            .as_ref()
            .unwrap()
            .encode_utf16()
            .eq(owner.encode_utf16()),
        "{source}: {error:?}"
    );
    assert!(error.frames.is_empty(), "{source}");
}

#[test]
fn nonlocal_const_rejection_uses_the_enclosing_scope_before_the_property() {
    for (source, owner) in [
        ("x = new { const bad: Int = 1 }\nresult = 1\n", "x"),
        ("x = new { const bad = 1 }\nresult = 1\n", "x"),
        (
            "x = new { child = new { const bad: Int = 1 } }\nresult = 1\n",
            "x.child",
        ),
        (
            "`outer odd` = new { const `bad odd`: Int = 1 }\nresult = 1\n",
            "`outer odd`",
        ),
        (
            "function build() = new Dynamic { const bad: Int = 1 }\nresult = 1\n",
            "build",
        ),
        (
            "f = () -> new Dynamic { const bad: Int = 1 }\nresult = 1\n",
            "f.<function#1>",
        ),
        (
            "x = let (a = 1) new { const bad: Int = a }\nresult = 1\n",
            "x.<let expr>",
        ),
        (
            "x = new Mapping { [\"entry\"] = new { const bad: Int = 1 } }\nresult = 1\n",
            "x[\"entry\"]",
        ),
        (
            "function build() = new { const bad: Int = 1 }\nresult = 1\n",
            "build",
        ),
    ] {
        assert_rejection(
            source,
            "Modifier `const` can only be applied to object members that are also `local`.",
            "const",
            owner,
        );
    }
}

#[test]
fn inferred_method_body_reaches_annotation_but_lambda_parent_rejects_first() {
    for (source, owner) in [
        (
            "function build() = new { bad: Int = 1 }\nresult = build().bad\n",
            "build.bad",
        ),
        (
            "function unused() = new { bad: Int = 1 }\nresult = 1\n",
            "unused.bad",
        ),
        (
            "x = new { local function build() = new { bad: Int = 1 } }\nresult = 1\n",
            "x.build.bad",
        ),
        (
            "function build(): Dynamic = new { bad: Int = 1 }\nresult = build().bad\n",
            "build.bad",
        ),
        (
            "function build() = let (a = 1) new { bad: Int = a }\nresult = 1\n",
            "build.<let expr>.bad",
        ),
        (
            "function build() = new { [\"bad\"] = new { bad: Int = 1 } }\nresult = 1\n",
            "build[\"bad\"].bad",
        ),
    ] {
        assert_rejection(
            source,
            "A non-local object property cannot have a type annotation.",
            "Int",
            owner,
        );
    }
    for (source, owner) in [
        (
            "f = () -> new { bad: Int = 1 }\nresult = f().bad\n",
            "f.<function#1>",
        ),
        (
            "`odd lambda` = () -> new { `bad odd`: Int = 1 }\nresult = 1\n",
            "`odd lambda`.<function#1>",
        ),
        ("f = (a) -> a\nresult = f(new { bad: Int = 1 })\n", "result"),
    ] {
        assert_rejection(source, "Cannot tell which parent to amend.", "new", owner);
    }
    assert_rejection(
        "f = () -> new Dynamic { bad: Int = 1 }\nresult = f().bad\n",
        "A non-local object property cannot have a type annotation.",
        "Int",
        "f.<function#1>.bad",
    );
}

#[test]
fn lexical_failure_precedes_const_and_inferred_parent_checks() {
    for source in [
        "x = new { const bad: Int = \"\\q\" }\nresult = 1\n",
        "function build() = new { before = \"\\q\"; bad: Int = 1 }\nresult = 1\n",
        "f = () -> new { before = \"\\q\"; bad: Int = 1 }\nresult = 1\n",
    ] {
        let error = parse_module(source).unwrap_err();
        assert_eq!(
            error.message,
            r#"Invalid character escape sequence `\q`.

Valid character escape sequences are:  \n  \r  \t  \"  \\ \<newline>"#
        );
        assert!(error.member.is_none());
        assert!(error.frames.is_empty());
    }
}

#[test]
fn accepted_const_and_inferred_method_bodies_preserve_lazy_values() {
    for (source, expected) in [
        (
            "x = new { local const good: Int = 1; result = good }\nresult = x.result\n",
            "1",
        ),
        (
            "class C { const good: Int = 1 }\nresult = new C {}.good\n",
            "1",
        ),
        ("const good: Int = 1\nresult = good\n", "1"),
        (
            "function build() = new { good = 1 }\nresult = build().good\n",
            "1",
        ),
        (
            "function build(): Dynamic = new { good = 1 }\nresult = build().good\n",
            "1",
        ),
        (
            "function unused() = new { good = throw(\"unused\") }\nresult = 2\n",
            "2",
        ),
        (
            "x = new { local const unused: Int = throw(\"unused\"); value = 2 }\nresult = x.value\n",
            "2",
        ),
    ] {
        assert_eq!(
            scalar_expression(source, "file:///m2-platform.pkl", "m2-platform", "result"),
            Ok(expected.to_owned()),
            "{source}"
        );
    }
}
