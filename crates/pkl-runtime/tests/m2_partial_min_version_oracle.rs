//! Complete primary result/error bytes copied from the pinned Stage122 Java capture.
//! This selection does not establish imported callbacks or distinct-root session parity.
use pkl_runtime::{
    evaluation::{EvaluationOptions, scalar_expression_text_with_options},
    source::SourceText,
    value::PklString,
};
use std::{fs, path::PathBuf};

fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/m2-min-version-partial-oracle")
}

fn units(bytes: &[u8]) -> Vec<u16> {
    let (pairs, remainder) = bytes.as_chunks::<2>();
    assert!(remainder.is_empty());
    pairs.iter().map(|pair| u16::from_be_bytes(*pair)).collect()
}

#[test]
fn direct_primary_results_preserve_complete_java_error_and_string_channels() {
    let root = fixture_root();
    let programs = fs::read_to_string(root.join("ordinary-programs.tsv")).unwrap();
    assert_eq!(programs.lines().count(), 80);
    for id in programs.lines() {
        let path = root.join("ordinary").join(id);
        let input_units = units(&fs::read(path.join("source.code-units.bin")).unwrap());
        let source = SourceText::from_utf16(input_units.clone());
        assert_eq!(
            source.original().to_java_utf8().as_bytes(),
            fs::read(path.join("source.pkl")).unwrap(),
            "{id} source identity"
        );
        let actual = scalar_expression_text_with_options(
            source,
            fs::read_to_string(path.join("uri.txt")).unwrap(),
            fs::read_to_string(path.join("inferred-name.txt")).unwrap(),
            "result",
            EvaluationOptions::default(),
        );
        let status = fs::read(path.join("status.bin")).unwrap();
        let (actual, channel) = match (status.as_slice(), actual) {
            (b"success\n", Ok(value)) => (value, "string"),
            (b"error\n", Err(error)) => (error, "error"),
            (expected, actual) => panic!("{id}: status {expected:?}, actual {actual:?}"),
        };
        assert_eq!(
            actual.encode_utf16().collect::<Vec<_>>(),
            units(&fs::read(path.join(format!("{channel}.code-units.bin"))).unwrap()),
            "{id} complete UTF16 {channel}"
        );
        assert_eq!(
            actual.to_java_utf8().as_bytes(),
            fs::read(path.join(format!("{channel}.bin"))).unwrap(),
            "{id} complete Java UTF8 {channel}"
        );
    }
}

#[test]
fn copied_internal_java_rows_remain_required_nonordinary_evidence() {
    let root = fixture_root().join("required-internal");
    for profile in ["default", "identity"] {
        for round in 0..2 {
            let row = root.join(profile).join(format!("round-{round}"));
            assert_eq!(fs::read(row.join("status.bin")).unwrap(), b"oracle-error\n");
            assert_eq!(
                fs::read(row.join("exception-class.bin")).unwrap(),
                b"org.pkl.core.PklBugException"
            );
            assert_eq!(
                fs::read(row.join("cause-class.bin")).unwrap(),
                b"java.lang.StringIndexOutOfBoundsException"
            );
            let message = PklString::from_utf16(units(
                &fs::read(row.join("cause-message.code-units.bin")).unwrap(),
            ));
            assert_eq!(
                message.to_java_utf8().as_bytes(),
                fs::read(row.join("cause-message.bin")).unwrap()
            );
        }
    }
    // These copied statuses are not counted among the 80 ordinary comparisons.
    // Rust's explicit Unsupported boundary is tested in the parser contract;
    // it does not implement Java's public PklBugException/cause hierarchy.
}
