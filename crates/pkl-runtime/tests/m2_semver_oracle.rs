//! Explicit-URI Java expression-string captures for semver authored controls.
//! The string adapter compares complete UTF-8 bytes and reconstructed UTF-16
//! code units; this corpus does not claim lossless unpaired-surrogate transport.
use pkl_runtime::evaluation::{EvaluationOptions, scalar_expression_with_options};
use std::fs;
use std::path::{Path, PathBuf};

const CASES: [&str; 12] = [
    "namespace",
    "defaults",
    "valid-roundtrip",
    "invalid-lexical",
    "precedence",
    "build-equality",
    "normal-stability",
    "invalid-constructor",
    "negative-major",
    "invalid-prerelease",
    "overflow-demand",
    "singleton-alias",
];

fn bytes(path: &Path, name: &str) -> Vec<u8> {
    fs::read(path.join(name))
        .unwrap_or_else(|error| panic!("{}: {error}", path.join(name).display()))
}

fn text(path: &Path, name: &str) -> String {
    String::from_utf8(bytes(path, name)).unwrap_or_else(|error| panic!("{name}: {error}"))
}

fn check_bytes(expected: &[u8], actual: &[u8], label: &str, mismatches: &mut Vec<String>) {
    if expected != actual {
        mismatches.push(format!("{label}: expected {expected:?}, actual {actual:?}"));
    }
}

fn check_text(
    folder: &Path,
    stem: &str,
    actual: &str,
    case: &str,
    mode: &str,
    mismatches: &mut Vec<String>,
) {
    let actual_units = actual
        .encode_utf16()
        .flat_map(u16::to_be_bytes)
        .collect::<Vec<_>>();
    check_bytes(
        &bytes(folder, &format!("{stem}.bin")),
        actual.as_bytes(),
        &format!("{case}/{mode}/{stem} UTF-8"),
        mismatches,
    );
    check_bytes(
        &bytes(folder, &format!("{stem}.code-units.bin")),
        &actual_units,
        &format!("{case}/{mode}/{stem} UTF-16"),
        mismatches,
    );
}

#[test]
fn java_authored_semver_matches_full_expression_results() {
    let root =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/m2-semver-authored");
    let roster = text(&root, "cases.txt");
    assert_eq!(roster.lines().collect::<Vec<_>>(), CASES);
    let mut mismatches = Vec::new();
    for case in CASES {
        let input = root.join(case);
        let source = text(&input, "module.pkl");
        let uri = text(&input, "uri.txt");
        let module = text(&input, "module-name.txt");
        let expression = text(&input, "expression.txt");
        assert_eq!(uri, format!("file:///stage138-semver/{case}/main.pkl"));
        assert_eq!(module, "main");
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
            check_bytes(
                &bytes(&oracle, "result.status.bin"),
                status,
                &format!("{case}/{mode} status"),
                &mut mismatches,
            );
            check_text(
                &oracle,
                "expression",
                &expression,
                case,
                mode,
                &mut mismatches,
            );
            check_text(
                &oracle,
                "result.stdout",
                &stdout,
                case,
                mode,
                &mut mismatches,
            );
            check_text(
                &oracle,
                "result.stderr",
                &stderr,
                case,
                mode,
                &mut mismatches,
            );
            // Java exception classes are retained as provenance; Rust has no
            // corresponding Java class to compare. Require both captured files.
            let _ = bytes(&oracle, "result.error-class.bin");
            let _ = bytes(&oracle, "result.error-class.code-units.bin");
        }
    }
    assert!(mismatches.is_empty(), "{}", mismatches.join("\n"));
}
