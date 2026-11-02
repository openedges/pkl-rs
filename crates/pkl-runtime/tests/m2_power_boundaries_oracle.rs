//! Explicit-URI Java expression-string captures for power assertion boundaries.
//! The string adapter compares complete UTF-8 bytes and reconstructed UTF-16
//! code units; this corpus does not claim lossless unpaired-surrogate transport.
use pkl_runtime::evaluation::{EvaluationOptions, scalar_expression_with_options};
use std::fs;
use std::path::{Path, PathBuf};

const CASES: [&str; 17] = [
    "generator-once",
    "generator-iterable-call",
    "generator-nested",
    "generator-nested-when",
    "lambda-root",
    "lambda-parenthesized-root",
    "lambda-nonroot",
    "lambda-inside-generator",
    "generator-inside-lambda",
    "predicate-simple",
    "predicate-generator",
    "generator-success",
    "predicate-explicit-value",
    "predicate-in-outer-for",
    "object-method-plain",
    "object-method-root-lambda",
    "object-method-for-iterable",
];

fn bytes(path: &Path, name: &str) -> Vec<u8> {
    fs::read(path.join(name))
        .unwrap_or_else(|error| panic!("{}: {error}", path.join(name).display()))
}

fn text(path: &Path, name: &str) -> String {
    String::from_utf8(bytes(path, name)).unwrap_or_else(|error| panic!("{name}: {error}"))
}

fn check_text(folder: &Path, stem: &str, actual: &str, case: &str, mode: &str) {
    let actual_units = actual
        .encode_utf16()
        .flat_map(u16::to_be_bytes)
        .collect::<Vec<_>>();
    assert_eq!(
        bytes(folder, &format!("{stem}.bin")),
        actual.as_bytes(),
        "{case}/{mode}/{stem} UTF-8"
    );
    assert_eq!(
        bytes(folder, &format!("{stem}.code-units.bin")),
        actual_units,
        "{case}/{mode}/{stem} UTF-16"
    );
}

#[test]
fn java_expression_string_power_boundaries_match_full_diagnostics() {
    let root =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/m2-power-boundaries");
    let roster = text(&root, "cases.txt");
    assert_eq!(roster.lines().collect::<Vec<_>>(), CASES);
    for case in CASES {
        let input = root.join(case);
        let source = text(&input, "module.pkl");
        let uri = text(&input, "uri.txt");
        let module = text(&input, "module-name.txt");
        let expression = text(&input, "expression.txt");
        assert_eq!(uri, format!("file:///stage131-power/{case}.pkl"));
        assert_eq!(module, case);
        assert_eq!(expression, "output.text");
        for (mode, power_assertions) in [("on", true), ("off", false)] {
            let oracle = input.join(mode);
            let actual = scalar_expression_with_options(
                &source,
                &uri,
                &module,
                &expression,
                EvaluationOptions { power_assertions },
            );
            let (status, stdout, stderr) = match actual {
                Ok(value) => (b"success\n".as_slice(), value, String::new()),
                Err(error) => (b"error\n".as_slice(), String::new(), error),
            };
            assert_eq!(
                bytes(&oracle, "result.status.bin"),
                status,
                "{case}/{mode} status"
            );
            check_text(&oracle, "expression", &expression, case, mode);
            check_text(&oracle, "result.stdout", &stdout, case, mode);
            check_text(&oracle, "result.stderr", &stderr, case, mode);
            // Java exception classes are retained as provenance; Rust has no
            // corresponding Java class to compare. Require both captured files.
            let _ = bytes(&oracle, "result.error-class.bin");
            let _ = bytes(&oracle, "result.error-class.code-units.bin");
        }
    }
}
