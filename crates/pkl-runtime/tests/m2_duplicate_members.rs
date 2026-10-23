//! Exact original duplicate-declaration Java CLI diagnostics, through shared AST validation.
use pkl_runtime::evaluation::frontend::{
    DiagnosticProfile, FailureDisposition, ModuleEvalOptions, ModuleSourceInput,
    evaluate_module_frontend,
};

struct CliObservation {
    name: &'static str,
    source: &'static str,
    uri: &'static str,
    stdout: &'static [u8],
    stderr: &'static [u8],
}

#[test]
fn original_module_duplicates_match_complete_java_channels() {
    let options = ModuleEvalOptions {
        diagnostic_profile: DiagnosticProfile::CliBundledSettings,
        ..Default::default()
    };
    let cases: &[CliObservation] = &[
        CliObservation {
            name: "duplicateFunction",
            source: include_str!(
                "../../../tests/fixtures/m2-module-duplicates/duplicateFunction/input.pkl"
            ),
            uri: "file:///Users/cody.ij.hwang/Projects/pkl-rs.stage42/target/m2/stage42-original-modules41-workspace-01/external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/modules/duplicateFunction.pkl",
            stdout: include_bytes!(
                "../../../tests/fixtures/m2-module-duplicates/duplicateFunction/0/stdout.bin"
            ),
            stderr: include_bytes!(
                "../../../tests/fixtures/m2-module-duplicates/duplicateFunction/0/stderr.bin"
            ),
        },
        CliObservation {
            name: "duplicateProperty1",
            source: include_str!(
                "../../../tests/fixtures/m2-module-duplicates/duplicateProperty1/input.pkl"
            ),
            uri: "file:///Users/cody.ij.hwang/Projects/pkl-rs.stage42/target/m2/stage42-original-modules41-workspace-01/external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/modules/duplicateProperty1.pkl",
            stdout: include_bytes!(
                "../../../tests/fixtures/m2-module-duplicates/duplicateProperty1/0/stdout.bin"
            ),
            stderr: include_bytes!(
                "../../../tests/fixtures/m2-module-duplicates/duplicateProperty1/0/stderr.bin"
            ),
        },
        CliObservation {
            name: "duplicateProperty2",
            source: include_str!(
                "../../../tests/fixtures/m2-module-duplicates/duplicateProperty2/input.pkl"
            ),
            uri: "file:///Users/cody.ij.hwang/Projects/pkl-rs.stage42/target/m2/stage42-original-modules41-workspace-01/external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/modules/duplicateProperty2.pkl",
            stdout: include_bytes!(
                "../../../tests/fixtures/m2-module-duplicates/duplicateProperty2/0/stdout.bin"
            ),
            stderr: include_bytes!(
                "../../../tests/fixtures/m2-module-duplicates/duplicateProperty2/0/stderr.bin"
            ),
        },
        CliObservation {
            name: "duplicateProperty3",
            source: include_str!(
                "../../../tests/fixtures/m2-module-duplicates/duplicateProperty3/input.pkl"
            ),
            uri: "file:///Users/cody.ij.hwang/Projects/pkl-rs.stage42/target/m2/stage42-original-modules41-workspace-01/external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/modules/duplicateProperty3.pkl",
            stdout: include_bytes!(
                "../../../tests/fixtures/m2-module-duplicates/duplicateProperty3/0/stdout.bin"
            ),
            stderr: include_bytes!(
                "../../../tests/fixtures/m2-module-duplicates/duplicateProperty3/0/stderr.bin"
            ),
        },
        CliObservation {
            name: "invalidModule1",
            source: include_str!(
                "../../../tests/fixtures/m2-module-duplicates/invalidModule1/input.pkl"
            ),
            uri: "file:///Users/cody.ij.hwang/Projects/pkl-rs.stage42/target/m2/stage42-original-modules41-workspace-01/external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/modules/invalidModule1.pkl",
            stdout: include_bytes!(
                "../../../tests/fixtures/m2-module-duplicates/invalidModule1/0/stdout.bin"
            ),
            stderr: include_bytes!(
                "../../../tests/fixtures/m2-module-duplicates/invalidModule1/0/stderr.bin"
            ),
        },
    ];
    let mut failures = Vec::new();
    for CliObservation {
        name,
        source,
        uri,
        stdout,
        stderr,
    } in cases
    {
        let result = evaluate_module_frontend(
            ModuleSourceInput {
                text: source,
                uri,
                inferred_name: name,
            },
            &options,
        );
        assert_eq!(
            result.disposition,
            FailureDisposition::LanguageError,
            "{name}"
        );
        assert_eq!(result.exit_code, 1, "{name}");
        assert_eq!(&result.stdout, stdout, "{name}");
        if &result.stderr != stderr {
            failures.push(format!(
                "{name}: expected {stderr:?}, actual {:?}",
                result.stderr
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn member_validation_and_grouping_match_complete_java_api_errors() {
    let options = ModuleEvalOptions {
        diagnostic_profile: DiagnosticProfile::ApiIdentity,
        ..Default::default()
    };
    let cases: &[(&str, &str, &str, &[u8])] = &[
        (
            "existing-local-property-header",
            include_str!(
                "../../../tests/fixtures/m2-module-duplicates/existing-local-property-header/input.pkl"
            ),
            "file:///Users/cody.ij.hwang/Projects/pkl-rs.stage42/target/m2/stage42-duplicate-members-oracle-01/workspace/existing-local-property-header/main.pkl",
            include_bytes!(
                "../../../tests/fixtures/m2-module-duplicates/existing-local-property-header/0/calls/outer-0.stderr.bin"
            ),
        ),
        (
            "method-validation-before-duplicate",
            include_str!(
                "../../../tests/fixtures/m2-module-duplicates/method-validation-before-duplicate/input.pkl"
            ),
            "file:///Users/cody.ij.hwang/Projects/pkl-rs.stage42/target/m2/stage42-duplicate-members-oracle-01/workspace/method-validation-before-duplicate/main.pkl",
            include_bytes!(
                "../../../tests/fixtures/m2-module-duplicates/method-validation-before-duplicate/0/calls/outer-0.stderr.bin"
            ),
        ),
        (
            "class-body-validation-before-duplicate",
            include_str!(
                "../../../tests/fixtures/m2-module-duplicates/class-body-validation-before-duplicate/input.pkl"
            ),
            "file:///Users/cody.ij.hwang/Projects/pkl-rs.stage42/target/m2/stage42-duplicate-members-oracle-01/workspace/class-body-validation-before-duplicate/main.pkl",
            include_bytes!(
                "../../../tests/fixtures/m2-module-duplicates/class-body-validation-before-duplicate/0/calls/outer-0.stderr.bin"
            ),
        ),
        (
            "class-property-before-method-validation",
            include_str!(
                "../../../tests/fixtures/m2-module-duplicates/class-property-before-method-validation/input.pkl"
            ),
            "file:///Users/cody.ij.hwang/Projects/pkl-rs.stage42/target/m2/stage42-duplicate-members-oracle-01/workspace/class-property-before-method-validation/main.pkl",
            include_bytes!(
                "../../../tests/fixtures/m2-module-duplicates/class-property-before-method-validation/0/calls/outer-0.stderr.bin"
            ),
        ),
        (
            "lambda-property-header",
            include_str!(
                "../../../tests/fixtures/m2-module-duplicates/lambda-property-header/input.pkl"
            ),
            "file:///Users/cody.ij.hwang/Projects/pkl-rs.stage42/target/m2/stage42-duplicate-members-kind-oracle-01/workspace/lambda-property-header/main.pkl",
            include_bytes!(
                "../../../tests/fixtures/m2-module-duplicates/lambda-property-header/0/calls/outer-0.stderr.bin"
            ),
        ),
        (
            "import-expression-property-order",
            include_str!(
                "../../../tests/fixtures/m2-module-duplicates/import-expression-property-order/input.pkl"
            ),
            "file:///Users/cody.ij.hwang/Projects/pkl-rs.stage42/target/m2/stage42-duplicate-members-kind-oracle-01/workspace/import-expression-property-order/main.pkl",
            include_bytes!(
                "../../../tests/fixtures/m2-module-duplicates/import-expression-property-order/0/calls/outer-0.stderr.bin"
            ),
        ),
        (
            "property-body-before-type-and-duplicate",
            include_str!(
                "../../../tests/fixtures/m2-module-duplicates/property-body-before-type-and-duplicate/input.pkl"
            ),
            "file:///Users/cody.ij.hwang/Projects/pkl-rs.stage42/target/m2/stage42-member-body-type-order-oracle-01/workspace/property-body-before-type-and-duplicate/main.pkl",
            include_bytes!(
                "../../../tests/fixtures/m2-module-duplicates/property-body-before-type-and-duplicate/0/calls/outer-0.stderr.bin"
            ),
        ),
        (
            "method-body-before-parameter-type-and-duplicate",
            include_str!(
                "../../../tests/fixtures/m2-module-duplicates/method-body-before-parameter-type-and-duplicate/input.pkl"
            ),
            "file:///Users/cody.ij.hwang/Projects/pkl-rs.stage42/target/m2/stage42-member-body-type-order-oracle-01/workspace/method-body-before-parameter-type-and-duplicate/main.pkl",
            include_bytes!(
                "../../../tests/fixtures/m2-module-duplicates/method-body-before-parameter-type-and-duplicate/0/calls/outer-0.stderr.bin"
            ),
        ),
    ];
    let mut failures = Vec::new();
    for (name, source, uri, stderr) in cases {
        let result = evaluate_module_frontend(
            ModuleSourceInput {
                text: source,
                uri,
                inferred_name: "main",
            },
            &options,
        );
        assert_eq!(
            result.disposition,
            FailureDisposition::LanguageError,
            "{name}"
        );
        assert_eq!(result.exit_code, 1, "{name}");
        assert!(result.stdout.is_empty(), "{name}");
        if &result.stderr != stderr {
            failures.push(format!(
                "{name}: expected {stderr:?}, actual {:?}",
                result.stderr
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn import_call_property_keeps_property_registration_order() {
    // Import expressions and import clauses share ExprKind, but only clauses
    // precede classes in AstBuilder. No module I/O is needed for AST validation.
    let source = "same = import(\"life:/child\")\nclass same {}\n";
    let module = pkl_runtime::parser::parse_module(source).unwrap();
    let error = pkl_runtime::resolver::resolve_module(&module).unwrap_err();
    assert_eq!(
        error.kind,
        pkl_runtime::diagnostic::DiagnosticKind::DuplicateMember
    );
    assert_eq!(error.span, pkl_runtime::source::Span::new(0, 4));
}

#[test]
fn lambda_property_duplicate_uses_property_header() {
    let source = "value = (x) -> x\nlocal value = (y) -> y\n";
    let module = pkl_runtime::parser::parse_module(source).unwrap();
    let error = pkl_runtime::resolver::resolve_module(&module).unwrap_err();
    assert_eq!(&source[error.span.start..error.span.end], "local value");
}
