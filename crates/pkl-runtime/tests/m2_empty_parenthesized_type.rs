//! Source-only draft: exact original Java error and bounded type grammar checks.
use pkl_runtime::evaluation::frontend::{
    ModuleEvalOptions, ModuleSourceInput, evaluate_module_output,
};
use pkl_runtime::parser::parse_module;

const INPUT: &str = include_str!("../../../tests/fixtures/m2-empty-parenthesized-type/input.pkl");
const URI: &str = include_str!("../../../tests/fixtures/m2-empty-parenthesized-type/uri.txt");
const JAVA_STDOUT: &[u8] =
    include_bytes!("../../../tests/fixtures/m2-empty-parenthesized-type/stdout.bin");
const JAVA_STDERR: &[u8] =
    include_bytes!("../../../tests/fixtures/m2-empty-parenthesized-type/stderr.bin");

#[test]
fn original_empty_parenthesized_type_matches_complete_java_error() {
    let actual = evaluate_module_output(
        ModuleSourceInput {
            text: INPUT,
            uri: URI,
            inferred_name: "emptyParenthesizedTypeAnnotation",
        },
        &ModuleEvalOptions::default(),
    );
    assert_eq!(actual.exit_code, 1);
    assert_eq!(actual.stdout, JAVA_STDOUT);
    assert_eq!(actual.stderr, JAVA_STDERR);
}

#[test]
fn empty_nonfunction_types_point_at_closing_paren() {
    // ParserImpl.parseTypeAtom checks the empty list only after consuming ')'
    // and finding no arrow. These additional cases are grammar checks, not
    // separately captured Java compatibility results.
    for source in [
        "foo: () = 1",
        "foo: ()? = 1",
        "foo: Int|()|String = 1",
        "foo: List<()> = 1",
        "foo: (()) = 1",
    ] {
        let error = parse_module(source).unwrap_err();
        assert_eq!(error.message, "Unexpected token `)`. Expected a type.");
        assert_eq!(&source[error.span.start..error.span.end], ")");
        assert_eq!(error.span.start, source.find("()").unwrap() + 1);
    }
}

#[test]
fn empty_function_parameters_and_other_parentheses_keep_their_grammar() {
    for source in [
        "foo: () -> Int = 1",
        "foo: (() -> Int)? = null",
        "foo: Int|(() -> String) = 1",
        "foo: (Int) = 1",
        "foo: (Int, String) -> Int = 1",
    ] {
        parse_module(source).unwrap();
    }
    let error = parse_module("foo: (Int, String) = 1").unwrap_err();
    assert!(error.message.contains("Expected `->`"), "{error:?}");
}
