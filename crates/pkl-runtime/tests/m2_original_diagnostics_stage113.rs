//! Complete raw Java stdout/stderr/exit observations for three unchanged originals.
//! Captures and original input hashes are recorded in the fixture provenance.
use pkl_runtime::evaluation::frontend::{
    ModuleEvalOptions, ModuleSourceInput, evaluate_module_output,
};

fn verify(text: &str, uri: &str, name: &str, exit_code: i32, stdout: &[u8], stderr: &[u8]) {
    let options = ModuleEvalOptions {
        properties: [
            ("name1", "value1"),
            ("name2", "value2"),
            ("/foo/bar", "foobar"),
        ]
        .map(|(key, value)| (key.into(), value.into()))
        .to_vec(),
        environment: [
            ("NAME1", "value1"),
            ("NAME2", "value2"),
            ("/foo/bar", "foobar"),
            ("foo bar", "foo bar"),
            ("file:///foo/bar", "file:///foo/bar"),
        ]
        .map(|(key, value)| (key.into(), value.into()))
        .to_vec(),
        ..Default::default()
    };
    let result = evaluate_module_output(
        ModuleSourceInput {
            text,
            uri,
            inferred_name: name,
        },
        &options,
    );
    assert_eq!(result.exit_code, exit_code, "{name}: {result:?}");
    assert_eq!(result.stdout, stdout, "{name}: stdout");
    assert_eq!(result.stderr, stderr, "{name}: stderr");
}

#[test]
fn constraint_lambdas_dispatch_get_class_on_the_captured_receiver() {
    verify(
        include_str!(
            "../../../tests/fixtures/m2-original-diagnostics-stage113/constraintsLambdaThis/input.pkl"
        ),
        include_str!(
            "../../../tests/fixtures/m2-original-diagnostics-stage113/constraintsLambdaThis/uri.txt"
        ),
        "constraintsLambdaThis",
        0,
        include_bytes!(
            "../../../tests/fixtures/m2-original-diagnostics-stage113/constraintsLambdaThis/stdout.bin"
        ),
        include_bytes!(
            "../../../tests/fixtures/m2-original-diagnostics-stage113/constraintsLambdaThis/stderr.bin"
        ),
    );
}

#[test]
fn quoted_member_scope_keeps_java_stack_frame_bytes() {
    verify(
        include_str!(
            "../../../tests/fixtures/m2-original-diagnostics-stage113/stackTraceWithQuotedMemberName/input.pkl"
        ),
        include_str!(
            "../../../tests/fixtures/m2-original-diagnostics-stage113/stackTraceWithQuotedMemberName/uri.txt"
        ),
        "stackTraceWithQuotedMemberName",
        1,
        include_bytes!(
            "../../../tests/fixtures/m2-original-diagnostics-stage113/stackTraceWithQuotedMemberName/stdout.bin"
        ),
        include_bytes!(
            "../../../tests/fixtures/m2-original-diagnostics-stage113/stackTraceWithQuotedMemberName/stderr.bin"
        ),
    );
}

#[test]
fn lambda_arity_failure_keeps_its_enclosing_property_owner() {
    verify(
        include_str!(
            "../../../tests/fixtures/m2-original-diagnostics-stage113/tooManyLambdaParams/input.pkl"
        ),
        include_str!(
            "../../../tests/fixtures/m2-original-diagnostics-stage113/tooManyLambdaParams/uri.txt"
        ),
        "tooManyLambdaParams",
        1,
        include_bytes!(
            "../../../tests/fixtures/m2-original-diagnostics-stage113/tooManyLambdaParams/stdout.bin"
        ),
        include_bytes!(
            "../../../tests/fixtures/m2-original-diagnostics-stage113/tooManyLambdaParams/stderr.bin"
        ),
    );
}
