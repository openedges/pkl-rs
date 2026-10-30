//! Whole retained Java exit/stdout/stderr for original spread diagnostics and controls.
#[path = "support/module_keys.rs"]
mod module_keys;

use pkl_runtime::evaluation::frontend::{
    ModuleEvalOptions, ModuleSourceInput, evaluate_module_output_with_host,
};
use pkl_runtime::modules::{ModuleCompletion, ModuleRequestKind, ModuleSource, ResolvedModule};

fn verify_module(
    text: &str,
    uri: &str,
    name: &str,
    parent: Option<(&str, &str, &str)>,
    expected: (i32, &[u8], &[u8]),
) {
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
    let mut loads = 0;
    let result = evaluate_module_output_with_host(
        ModuleSourceInput {
            text,
            uri,
            inferred_name: name,
        },
        &options,
        |request| {
            if let Some(completion) = module_keys::defaults(request) {
                return completion;
            }
            let (parent_uri, parent_text, parent_name) =
                parent.expect("unexpected imported module");
            match &request.kind {
                ModuleRequestKind::Resolve { original_uri, .. } => {
                    assert_eq!(original_uri.clone(), parent_uri);
                    ModuleCompletion::Resolved(Ok(ResolvedModule {
                        original_uri: original_uri.clone(),
                        resolved_uri: original_uri.clone(),
                        source_uri: original_uri.clone(),
                    }))
                }
                ModuleRequestKind::LoadSource { resolved, .. } => {
                    assert_eq!(resolved.source_uri.clone(), parent_uri);
                    loads += 1;
                    ModuleCompletion::Source(Ok(ModuleSource {
                        text: parent_text.into(),
                        inferred_name: parent_name.into(),
                    }))
                }
                other => panic!("unexpected fixture module request: {other:?}"),
            }
        },
    );
    assert_eq!(loads, usize::from(parent.is_some()));
    assert_eq!(result.exit_code, expected.0, "{name}: exit");
    assert_eq!(result.stdout, expected.1, "{name}: complete stdout");
    assert_eq!(result.stderr, expected.2, "{name}: complete stderr");
}

fn verify_scalar(
    text: &str,
    uri: &str,
    name: &str,
    expression: &str,
    expected: (i32, &[u8], &[u8]),
) {
    let actual = match pkl_runtime::evaluation::scalar_expression(text, uri, name, expression) {
        Ok(value) => (0, value.into_bytes(), Vec::new()),
        Err(error) => (1, Vec::new(), error.into_bytes()),
    };
    assert_eq!(actual.0, expected.0, "{name}: mapped API exit");
    assert_eq!(actual.1, expected.1, "{name}: complete stdout");
    assert_eq!(actual.2, expected.2, "{name}: complete stderr");
}

#[test]
fn cannot_assign_fixed_property3() {
    verify_module(
        include_str!(
            "../../../tests/fixtures/m2-fixed-spread-original/cannotAssignFixedProperty3/input.pkl"
        ),
        include_str!(
            "../../../tests/fixtures/m2-fixed-spread-original/cannotAssignFixedProperty3/uri.txt"
        ),
        "cannotAssignFixedProperty3",
        Some((
            include_str!(
                "../../../tests/fixtures/m2-fixed-spread-original/cannotAssignFixedProperty3/parent-uri.txt"
            ),
            include_str!(
                "../../../tests/fixtures/m2-fixed-spread-original/cannotAssignFixedProperty3/parent.pkl"
            ),
            "fixedProperty2",
        )),
        (
            1,
            include_bytes!(
                "../../../tests/fixtures/m2-fixed-spread-original/cannotAssignFixedProperty3/stdout.bin"
            ),
            include_bytes!(
                "../../../tests/fixtures/m2-fixed-spread-original/cannotAssignFixedProperty3/stderr.bin"
            ),
        ),
    );
}

#[test]
fn spread_syntax_unknown_typed_property() {
    verify_module(
        include_str!(
            "../../../tests/fixtures/m2-fixed-spread-original/spreadSyntaxUnknownTypedProperty/input.pkl"
        ),
        include_str!(
            "../../../tests/fixtures/m2-fixed-spread-original/spreadSyntaxUnknownTypedProperty/uri.txt"
        ),
        "spreadSyntaxUnknownTypedProperty",
        None,
        (
            1,
            include_bytes!(
                "../../../tests/fixtures/m2-fixed-spread-original/spreadSyntaxUnknownTypedProperty/stdout.bin"
            ),
            include_bytes!(
                "../../../tests/fixtures/m2-fixed-spread-original/spreadSyntaxUnknownTypedProperty/stderr.bin"
            ),
        ),
    );
}

#[test]
fn spread_syntax_duplicate_property1() {
    verify_module(
        include_str!(
            "../../../tests/fixtures/m2-fixed-spread-original/spreadSyntaxDuplicateProperty1/input.pkl"
        ),
        include_str!(
            "../../../tests/fixtures/m2-fixed-spread-original/spreadSyntaxDuplicateProperty1/uri.txt"
        ),
        "spreadSyntaxDuplicateProperty1",
        None,
        (
            1,
            include_bytes!(
                "../../../tests/fixtures/m2-fixed-spread-original/spreadSyntaxDuplicateProperty1/stdout.bin"
            ),
            include_bytes!(
                "../../../tests/fixtures/m2-fixed-spread-original/spreadSyntaxDuplicateProperty1/stderr.bin"
            ),
        ),
    );
}

#[test]
fn spread_syntax_duplicate_property2() {
    verify_module(
        include_str!(
            "../../../tests/fixtures/m2-fixed-spread-original/spreadSyntaxDuplicateProperty2/input.pkl"
        ),
        include_str!(
            "../../../tests/fixtures/m2-fixed-spread-original/spreadSyntaxDuplicateProperty2/uri.txt"
        ),
        "spreadSyntaxDuplicateProperty2",
        None,
        (
            1,
            include_bytes!(
                "../../../tests/fixtures/m2-fixed-spread-original/spreadSyntaxDuplicateProperty2/stdout.bin"
            ),
            include_bytes!(
                "../../../tests/fixtures/m2-fixed-spread-original/spreadSyntaxDuplicateProperty2/stderr.bin"
            ),
        ),
    );
}

#[test]
fn spread_syntax_duplicate_entry1() {
    verify_module(
        include_str!(
            "../../../tests/fixtures/m2-fixed-spread-original/spreadSyntaxDuplicateEntry1/input.pkl"
        ),
        include_str!(
            "../../../tests/fixtures/m2-fixed-spread-original/spreadSyntaxDuplicateEntry1/uri.txt"
        ),
        "spreadSyntaxDuplicateEntry1",
        None,
        (
            1,
            include_bytes!(
                "../../../tests/fixtures/m2-fixed-spread-original/spreadSyntaxDuplicateEntry1/stdout.bin"
            ),
            include_bytes!(
                "../../../tests/fixtures/m2-fixed-spread-original/spreadSyntaxDuplicateEntry1/stderr.bin"
            ),
        ),
    );
}

#[test]
fn spread_syntax_duplicate_entry2() {
    verify_module(
        include_str!(
            "../../../tests/fixtures/m2-fixed-spread-original/spreadSyntaxDuplicateEntry2/input.pkl"
        ),
        include_str!(
            "../../../tests/fixtures/m2-fixed-spread-original/spreadSyntaxDuplicateEntry2/uri.txt"
        ),
        "spreadSyntaxDuplicateEntry2",
        None,
        (
            1,
            include_bytes!(
                "../../../tests/fixtures/m2-fixed-spread-original/spreadSyntaxDuplicateEntry2/stdout.bin"
            ),
            include_bytes!(
                "../../../tests/fixtures/m2-fixed-spread-original/spreadSyntaxDuplicateEntry2/stderr.bin"
            ),
        ),
    );
}

#[test]
fn spread_syntax_cannot_have_property() {
    verify_module(
        include_str!(
            "../../../tests/fixtures/m2-fixed-spread-original/spreadSyntaxCannotHaveProperty/input.pkl"
        ),
        include_str!(
            "../../../tests/fixtures/m2-fixed-spread-original/spreadSyntaxCannotHaveProperty/uri.txt"
        ),
        "spreadSyntaxCannotHaveProperty",
        None,
        (
            1,
            include_bytes!(
                "../../../tests/fixtures/m2-fixed-spread-original/spreadSyntaxCannotHaveProperty/stdout.bin"
            ),
            include_bytes!(
                "../../../tests/fixtures/m2-fixed-spread-original/spreadSyntaxCannotHaveProperty/stderr.bin"
            ),
        ),
    );
}

#[test]
fn const_assign() {
    verify_module(
        include_str!("../../../tests/fixtures/m2-fixed-spread-original/constAssign/input.pkl"),
        include_str!("../../../tests/fixtures/m2-fixed-spread-original/constAssign/uri.txt"),
        "constAssign",
        Some((
            include_str!(
                "../../../tests/fixtures/m2-fixed-spread-original/constAssign/parent-uri.txt"
            ),
            include_str!("../../../tests/fixtures/m2-fixed-spread-original/constAssign/parent.pkl"),
            "constModifier",
        )),
        (
            1,
            include_bytes!(
                "../../../tests/fixtures/m2-fixed-spread-original/constAssign/stdout.bin"
            ),
            include_bytes!(
                "../../../tests/fixtures/m2-fixed-spread-original/constAssign/stderr.bin"
            ),
        ),
    );
}

#[test]
fn types_stage18_const_generator_spread() {
    verify_scalar(
        include_str!(
            "../../../tests/fixtures/m2-fixed-spread-original/types-stage18-const-generator-spread/input.pkl"
        ),
        include_str!(
            "../../../tests/fixtures/m2-fixed-spread-original/types-stage18-const-generator-spread/uri.txt"
        ),
        "types-stage18-const-generator-spread",
        include_str!(
            "../../../tests/fixtures/m2-fixed-spread-original/types-stage18-const-generator-spread/expression.txt"
        ),
        (
            0,
            include_bytes!(
                "../../../tests/fixtures/m2-fixed-spread-original/types-stage18-const-generator-spread/stdout.bin"
            ),
            include_bytes!(
                "../../../tests/fixtures/m2-fixed-spread-original/types-stage18-const-generator-spread/stderr.bin"
            ),
        ),
    );
}

#[test]
fn types_stage18_const_generator_spread_lambda() {
    verify_scalar(
        include_str!(
            "../../../tests/fixtures/m2-fixed-spread-original/types-stage18-const-generator-spread-lambda/input.pkl"
        ),
        include_str!(
            "../../../tests/fixtures/m2-fixed-spread-original/types-stage18-const-generator-spread-lambda/uri.txt"
        ),
        "types-stage18-const-generator-spread-lambda",
        include_str!(
            "../../../tests/fixtures/m2-fixed-spread-original/types-stage18-const-generator-spread-lambda/expression.txt"
        ),
        (
            1,
            include_bytes!(
                "../../../tests/fixtures/m2-fixed-spread-original/types-stage18-const-generator-spread-lambda/stdout.bin"
            ),
            include_bytes!(
                "../../../tests/fixtures/m2-fixed-spread-original/types-stage18-const-generator-spread-lambda/stderr.bin"
            ),
        ),
    );
}
