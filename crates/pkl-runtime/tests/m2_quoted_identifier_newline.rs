//! Complete raw Java channels for the unchanged upstream singleBacktick original.
use pkl_runtime::evaluation::frontend::{
    ModuleEvalOptions, ModuleSourceInput, evaluate_module_output,
};

#[test]
fn quoted_identifier_line_terminator_preserves_java_error_and_source_span() {
    let result = evaluate_module_output(
        ModuleSourceInput {
            text: include_str!("../../../tests/fixtures/m2-quoted-identifier-newline/input.pkl"),
            uri: include_str!("../../../tests/fixtures/m2-quoted-identifier-newline/uri.txt"),
            inferred_name: "singleBacktick",
        },
        &ModuleEvalOptions::default(),
    );
    assert_eq!(result.exit_code, 1);
    assert_eq!(
        result.stdout,
        include_bytes!("../../../tests/fixtures/m2-quoted-identifier-newline/stdout.bin"),
    );
    assert_eq!(
        result.stderr,
        include_bytes!("../../../tests/fixtures/m2-quoted-identifier-newline/stderr.bin"),
    );
}
