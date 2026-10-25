//! Four unchanged upstream CLI originals, compared with the complete Stage67 Java streams.
use pkl_runtime::evaluation::frontend::{
    DiagnosticProfile, ModuleEvalOptions, ModuleSourceInput, evaluate_module_output,
};

struct Original<'a> {
    number: u8,
    source: &'a str,
    uri: &'a str,
    java_stdout: &'a [u8],
    java_stderr: &'a [u8],
}

#[test]
fn reference_constraint_cast_originals_match_full_java_cli_bytes() {
    let originals = [
        Original {
            number: 8,
            source: include_str!(
                "../../../tests/fixtures/m2-reference-constraint-originals/reference8/input.pkl"
            ),
            uri: include_str!(
                "../../../tests/fixtures/m2-reference-constraint-originals/reference8/uri.txt"
            ),
            java_stdout: include_bytes!(
                "../../../tests/fixtures/m2-reference-constraint-originals/reference8/stdout.bin"
            ),
            java_stderr: include_bytes!(
                "../../../tests/fixtures/m2-reference-constraint-originals/reference8/stderr.bin"
            ),
        },
        Original {
            number: 9,
            source: include_str!(
                "../../../tests/fixtures/m2-reference-constraint-originals/reference9/input.pkl"
            ),
            uri: include_str!(
                "../../../tests/fixtures/m2-reference-constraint-originals/reference9/uri.txt"
            ),
            java_stdout: include_bytes!(
                "../../../tests/fixtures/m2-reference-constraint-originals/reference9/stdout.bin"
            ),
            java_stderr: include_bytes!(
                "../../../tests/fixtures/m2-reference-constraint-originals/reference9/stderr.bin"
            ),
        },
        Original {
            number: 10,
            source: include_str!(
                "../../../tests/fixtures/m2-reference-constraint-originals/reference10/input.pkl"
            ),
            uri: include_str!(
                "../../../tests/fixtures/m2-reference-constraint-originals/reference10/uri.txt"
            ),
            java_stdout: include_bytes!(
                "../../../tests/fixtures/m2-reference-constraint-originals/reference10/stdout.bin"
            ),
            java_stderr: include_bytes!(
                "../../../tests/fixtures/m2-reference-constraint-originals/reference10/stderr.bin"
            ),
        },
        Original {
            number: 11,
            source: include_str!(
                "../../../tests/fixtures/m2-reference-constraint-originals/reference11/input.pkl"
            ),
            uri: include_str!(
                "../../../tests/fixtures/m2-reference-constraint-originals/reference11/uri.txt"
            ),
            java_stdout: include_bytes!(
                "../../../tests/fixtures/m2-reference-constraint-originals/reference11/stdout.bin"
            ),
            java_stderr: include_bytes!(
                "../../../tests/fixtures/m2-reference-constraint-originals/reference11/stderr.bin"
            ),
        },
    ];
    let options = ModuleEvalOptions {
        no_cache: true,
        no_project: true,
        settings: "pkl:settings".into(),
        properties: vec![
            ("name1".into(), "value1".into()),
            ("name2".into(), "value2".into()),
            ("/foo/bar".into(), "foobar".into()),
        ],
        environment: vec![
            ("NAME1".into(), "value1".into()),
            ("NAME2".into(), "value2".into()),
            ("/foo/bar".into(), "foobar".into()),
            ("foo bar".into(), "foo bar".into()),
            ("file:///foo/bar".into(), "file:///foo/bar".into()),
        ],
        diagnostic_profile: DiagnosticProfile::CliBundledSettings,
    };
    for original in originals {
        let inferred_name = format!("reference{}", original.number);
        let actual = evaluate_module_output(
            ModuleSourceInput {
                text: original.source,
                uri: original.uri,
                inferred_name: &inferred_name,
            },
            &options,
        );
        assert_eq!(actual.exit_code, 1, "reference{} exit", original.number);
        assert_eq!(
            actual.stdout, original.java_stdout,
            "reference{} complete stdout bytes",
            original.number
        );
        assert_eq!(
            actual.stderr, original.java_stderr,
            "reference{} complete stderr bytes",
            original.number
        );
    }
}
