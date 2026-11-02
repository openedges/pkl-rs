//! Authored-URI Java expression-string comparisons for object-scope entry numbering.
//! Java captures are the expected values; both complete UTF-8 and UTF-16
//! channels are checked. This adapter does not preserve unpaired surrogates.
use pkl_runtime::evaluation::{EvaluationOptions, scalar_expression_with_options};
use std::fs;
use std::path::{Path, PathBuf};

const CASES: [&str; 8] = [
    "original-minimal",
    "nested-key-two-elements",
    "sibling-entries",
    "sibling-objects",
    "nested-value",
    "literal-key-controls",
    "generator-parent-counter",
    "key-construction-error",
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
fn java_entry_scope_names_match_complete_results() {
    let root =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/m2-entry-scopes");
    assert_eq!(text(&root, "cases.txt").lines().collect::<Vec<_>>(), CASES);
    for case in CASES {
        let input = root.join(case);
        let source = text(&input, "module.pkl");
        let uri = text(&input, "uri.txt");
        let module = text(&input, "module-name.txt");
        let expression = text(&input, "expression.txt");
        assert_eq!(uri, format!("file:///stage133-entries/{case}.pkl"));
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
                "{case}/{mode} status (including oracle errors)"
            );
            check_text(&oracle, "expression", &expression, case, mode);
            check_text(&oracle, "result.stdout", &stdout, case, mode);
            check_text(&oracle, "result.stderr", &stderr, case, mode);
            // Java exception classes are provenance, not Rust class identities.
            let _ = bytes(&oracle, "result.error-class.bin");
            let _ = bytes(&oracle, "result.error-class.code-units.bin");
        }
    }
}
