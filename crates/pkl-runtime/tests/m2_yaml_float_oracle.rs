//! Twelve admitted Java observations for six YAML float constructor controls.
//! Compare Java UTF-8 replacement bytes and the original UTF-16 code units.
use pkl_runtime::evaluation::{EvaluationOptions, scalar_expression_text_with_options};
use pkl_runtime::value::PklString;
use std::fs;
use std::path::{Path, PathBuf};

const CASES: [&str; 6] = [
    "empty",
    "whitespace",
    "invalid-surrounded",
    "valid-surrounded",
    "yaml-special-surrounded",
    "underscore-whitespace",
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
    actual: &PklString,
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
        actual.to_java_utf8().as_bytes(),
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
fn java_yaml_float_matches_full_expression_results() {
    let root =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/m2-yaml-float-oracle");
    let roster = text(&root, "cases.txt");
    assert_eq!(roster, format!("{}\n", CASES.join("\n")));
    let mut mismatches = Vec::new();
    for case in CASES {
        let input = root.join(case);
        let source = text(&input, "module.pkl");
        let uri = text(&input, "uri.txt");
        let module = text(&input, "module-name.txt");
        let expression = text(&input, "expression.txt");
        assert_eq!(uri, format!("file:///stage142-yaml-float/{case}/main.pkl"));
        assert_eq!(module, "main");
        assert_eq!(expression, "output.text");
        for (mode, power_assertions) in [("on", true), ("off", false)] {
            let oracle = input.join(mode);
            let actual = scalar_expression_text_with_options(
                source.as_str(),
                uri.as_str(),
                module.as_str(),
                expression.as_str(),
                EvaluationOptions { power_assertions },
            );
            let (status, stdout, stderr) = match actual {
                Ok(value) => (b"success\n".as_slice(), value, PklString::default()),
                Err(error) => (b"error\n".as_slice(), PklString::default(), error),
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
                &PklString::from(expression.as_str()),
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
            // These retain Java process provenance; this library call has no
            // corresponding subprocess stdout/stderr channel.
            let _ = bytes(&oracle, "process.stdout.bin");
            let _ = bytes(&oracle, "process.stderr.bin");
        }
    }
    assert!(mismatches.is_empty(), "{}", mismatches.join("\n"));
}
