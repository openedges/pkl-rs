//! Four unchanged upstream CLI originals, including two module-owner controls.
//! Authored structured checks below are source-derived, not new Java observations.
use pkl_runtime::diagnostic::{Diagnostic, DiagnosticKind};
use pkl_runtime::evaluation::frontend::{
    DiagnosticProfile, ModuleEvalOptions, ModuleSourceInput, evaluate_module_output,
};
use pkl_runtime::parser::parse_module;
use pkl_runtime::resolver::resolve_module;
use pkl_runtime::value::PklString;

struct Original<'a> {
    label: &'a str,
    inferred_name: &'a str,
    source: &'a str,
    uri: &'a str,
    java_exit: &'a str,
    java_stdout: &'a [u8],
    java_stderr: &'a [u8],
}

#[test]
fn duplicate_class_and_module_originals_match_full_java_cli_bytes() {
    let originals = [
        Original {
            label: "class-function",
            inferred_name: "duplicateFunction",
            source: include_str!(
                "../../../tests/fixtures/m2-duplicate-class-owner-originals/class-function/input.pkl"
            ),
            uri: include_str!(
                "../../../tests/fixtures/m2-duplicate-class-owner-originals/class-function/uri.txt"
            ),
            java_exit: include_str!(
                "../../../tests/fixtures/m2-duplicate-class-owner-originals/class-function/exit.txt"
            ),
            java_stdout: include_bytes!(
                "../../../tests/fixtures/m2-duplicate-class-owner-originals/class-function/stdout.bin"
            ),
            java_stderr: include_bytes!(
                "../../../tests/fixtures/m2-duplicate-class-owner-originals/class-function/stderr.bin"
            ),
        },
        Original {
            label: "class-property",
            inferred_name: "duplicateProperty",
            source: include_str!(
                "../../../tests/fixtures/m2-duplicate-class-owner-originals/class-property/input.pkl"
            ),
            uri: include_str!(
                "../../../tests/fixtures/m2-duplicate-class-owner-originals/class-property/uri.txt"
            ),
            java_exit: include_str!(
                "../../../tests/fixtures/m2-duplicate-class-owner-originals/class-property/exit.txt"
            ),
            java_stdout: include_bytes!(
                "../../../tests/fixtures/m2-duplicate-class-owner-originals/class-property/stdout.bin"
            ),
            java_stderr: include_bytes!(
                "../../../tests/fixtures/m2-duplicate-class-owner-originals/class-property/stderr.bin"
            ),
        },
        Original {
            label: "module-function",
            inferred_name: "duplicateFunction",
            source: include_str!(
                "../../../tests/fixtures/m2-duplicate-class-owner-originals/module-function/input.pkl"
            ),
            uri: include_str!(
                "../../../tests/fixtures/m2-duplicate-class-owner-originals/module-function/uri.txt"
            ),
            java_exit: include_str!(
                "../../../tests/fixtures/m2-duplicate-class-owner-originals/module-function/exit.txt"
            ),
            java_stdout: include_bytes!(
                "../../../tests/fixtures/m2-duplicate-class-owner-originals/module-function/stdout.bin"
            ),
            java_stderr: include_bytes!(
                "../../../tests/fixtures/m2-duplicate-class-owner-originals/module-function/stderr.bin"
            ),
        },
        Original {
            label: "module-property",
            inferred_name: "duplicateProperty1",
            source: include_str!(
                "../../../tests/fixtures/m2-duplicate-class-owner-originals/module-property/input.pkl"
            ),
            uri: include_str!(
                "../../../tests/fixtures/m2-duplicate-class-owner-originals/module-property/uri.txt"
            ),
            java_exit: include_str!(
                "../../../tests/fixtures/m2-duplicate-class-owner-originals/module-property/exit.txt"
            ),
            java_stdout: include_bytes!(
                "../../../tests/fixtures/m2-duplicate-class-owner-originals/module-property/stdout.bin"
            ),
            java_stderr: include_bytes!(
                "../../../tests/fixtures/m2-duplicate-class-owner-originals/module-property/stderr.bin"
            ),
        },
    ];
    let options = ModuleEvalOptions {
        no_cache: true,
        no_project: true,
        settings: "pkl:settings".into(),
        properties: vec![
            ("name1".into(), "value1".into()),
            ("name2".into(), "value2".into()),
            ("/foo/bar".into(), "foobar".into()),
        ],
        environment: vec![
            ("NAME1".into(), "value1".into()),
            ("NAME2".into(), "value2".into()),
            ("/foo/bar".into(), "foobar".into()),
            ("foo bar".into(), "foo bar".into()),
            ("file:///foo/bar".into(), "file:///foo/bar".into()),
        ],
        diagnostic_profile: DiagnosticProfile::CliBundledSettings,
    };
    for original in originals {
        for repetition in 0..2 {
            let actual = evaluate_module_output(
                ModuleSourceInput {
                    text: original.source,
                    uri: original.uri,
                    inferred_name: original.inferred_name,
                },
                &options,
            );
            assert_eq!(
                actual.exit_code,
                original.java_exit.parse::<i32>().unwrap(),
                "{} repeat {repetition} exit",
                original.label
            );
            assert_eq!(
                actual.stdout, original.java_stdout,
                "{} repeat {repetition} complete stdout bytes",
                original.label
            );
            assert_eq!(
                actual.stderr, original.java_stderr,
                "{} repeat {repetition} complete stderr bytes",
                original.label
            );
        }
    }
}

fn rejection(source: &str) -> Diagnostic {
    resolve_module(&parse_module(source).unwrap()).unwrap_err()
}

// These assertions exercise the existing owner protocol at the changed producer.
// They are pinned-source-derived controls, not additional Java-captured cases.
#[test]
fn duplicate_owner_restores_enclosing_scope_and_quotes_authored_names() {
    for (source, owner, header, name) in [
        (
            "class Person { function f() = new Dynamic { child = new { value = 1 } }; function f() = new Dynamic { child = new { value = 2 } } }",
            "Person",
            "function f()",
            "f",
        ),
        (
            "class `outer.name` { `field.name`: Int; `field.name`: String }",
            "`outer.name`",
            "`field.name`: String",
            "field.name",
        ),
        (
            "class `😀` { function `do.work`() = 1; function `do.work`() = new Dynamic { child = new { value = 2 } } }",
            "`😀`",
            "function `do.work`()",
            "do.work",
        ),
    ] {
        let error = rejection(source);
        assert_eq!(error.kind, DiagnosticKind::DuplicateMember, "{source}");
        assert_eq!(
            error.member.as_deref(),
            Some(&PklString::from(owner)),
            "{source}"
        );
        assert_eq!(
            error.message,
            format!("Duplicate definition of member `{name}`."),
            "{source}"
        );
        let start = source.rfind(header).unwrap();
        assert_eq!(error.span.start, start, "{source}");
        assert_eq!(error.span.end, start + header.len(), "{source}");
        assert!(error.frames.is_empty());
        assert!(error.hint().is_none());
    }
}

#[test]
fn duplicate_checks_preserve_module_owner_and_body_error_precedence() {
    let source = "function f() = 1; function f() = new Dynamic { child = new { value = 2 } }";
    let error = rejection(source);
    assert_eq!(error.kind, DiagnosticKind::DuplicateMember);
    assert!(error.member.is_none());
    assert_eq!(&source[error.span.start..error.span.end], "function f()");

    let source = "class Person { function f() = 1; function f() = new Dynamic { child = new { bad: Int = 1 } } }";
    let error = rejection(source);
    assert_eq!(error.kind, DiagnosticKind::Syntax);
    assert_eq!(
        error.message,
        "A non-local object property cannot have a type annotation."
    );
    assert_eq!(
        error.member.as_deref(),
        Some(&PklString::from("Person.f.child.bad"))
    );
    assert_eq!(&source[error.span.start..error.span.end], "Int");
}
