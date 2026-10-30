//! Raw original Java CLI result for Pair's missing-property diagnostics.
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
fn pair_missing_property_lists_its_actual_declared_fields() {
    verify(
        include_str!("../../../tests/fixtures/m2-pair-diagnostics-original/input.pkl"),
        include_str!("../../../tests/fixtures/m2-pair-diagnostics-original/uri.txt"),
        "propertyNotFound2",
        1,
        include_bytes!("../../../tests/fixtures/m2-pair-diagnostics-original/stdout.bin"),
        include_bytes!("../../../tests/fixtures/m2-pair-diagnostics-original/stderr.bin"),
    );
}
