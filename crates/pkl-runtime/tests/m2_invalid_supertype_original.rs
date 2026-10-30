//! Raw original Java CLI result for a constrained alias used as superclass.
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
fn constrained_alias_supertype_reports_the_authored_use_site() {
    verify(
        include_str!("../../../tests/fixtures/m2-invalid-supertype-original/input.pkl"),
        include_str!("../../../tests/fixtures/m2-invalid-supertype-original/uri.txt"),
        "extendTypeAlias",
        1,
        include_bytes!("../../../tests/fixtures/m2-invalid-supertype-original/stdout.bin"),
        include_bytes!("../../../tests/fixtures/m2-invalid-supertype-original/stderr.bin"),
    );
}
