//! Java 0.32 inferred-parent controls for direct and aliased unknown.
use pkl_runtime::evaluation::{EvaluationOptions, scalar_expression_with_options};
use std::{fs, path::Path};

#[test]
fn java_unknown_property_parent_matches_complete_results() {
    let root =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/m2-unknown-property");
    assert_eq!(
        fs::read_to_string(root.join("cases.txt")).unwrap(),
        "direct-unknown\nalias-unknown\n"
    );
    for case in ["direct-unknown", "alias-unknown"] {
        let input = root.join(case);
        let source = fs::read_to_string(input.join("module.pkl")).unwrap();
        let uri = fs::read_to_string(input.join("uri.txt")).unwrap();
        assert_eq!(uri, format!("file:///stage134-unknown-property/{case}.pkl"));
        assert_eq!(
            fs::read_to_string(input.join("expression.txt")).unwrap(),
            "output.text"
        );
        for (mode, power_assertions) in [("on", true), ("off", false)] {
            let expected = input.join(mode);
            let result = scalar_expression_with_options(
                &source,
                &uri,
                case,
                "output.text",
                EvaluationOptions { power_assertions },
            );
            let (status, stdout, stderr) = match result {
                Ok(value) => (b"success\n".as_slice(), value, String::new()),
                Err(error) => (b"error\n".as_slice(), String::new(), error),
            };
            assert_eq!(
                fs::read(expected.join("result.status.bin")).unwrap(),
                status,
                "{case}/{mode}"
            );
            for (stem, value) in [("result.stdout", stdout), ("result.stderr", stderr)] {
                assert_eq!(
                    fs::read(expected.join(format!("{stem}.bin"))).unwrap(),
                    value.as_bytes(),
                    "{case}/{mode}/{stem}"
                );
                let units: Vec<_> = value.encode_utf16().flat_map(u16::to_be_bytes).collect();
                assert_eq!(
                    fs::read(expected.join(format!("{stem}.code-units.bin"))).unwrap(),
                    units,
                    "{case}/{mode}/{stem}"
                );
            }
        }
    }
}
