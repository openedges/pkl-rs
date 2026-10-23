//! The bounded scalar adapters preserve original Java values and diagnostics.
use pkl_runtime::evaluation::{
    EvaluationOptions, scalar_expression_text_with_options, scalar_expressions_text_with_options,
    scalar_expressions_with_options,
};
use std::{fs, path::Path};

fn units(bytes: &[u8]) -> Vec<u16> {
    let (chunks, remainder) = bytes.as_chunks::<2>();
    assert!(remainder.is_empty());
    chunks
        .iter()
        .map(|unit| u16::from_be_bytes(*unit))
        .collect()
}

#[test]
fn ordered_java_value_throw_and_type_prefix_matches_canonical_and_legacy_adapters() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/m2-lossless-diagnostics/producers");
    let source = fs::read_to_string(root.join("workspace/main.pkl")).unwrap();
    let uri = fs::read_to_string(root.join("uri.txt")).unwrap();
    // This is the original six-call prefix, before the Host logger call.
    // The complete source and every original Java call remain in the fixture.
    let expressions = [
        "payload",
        "throwing",
        "badInt",
        "badConstraint",
        "badUnion",
        "throwing",
    ]
    .map(String::from);
    let options = EvaluationOptions {
        power_assertions: true,
    };
    for repeat in 0..2 {
        let raw = root.join(format!("repeats/{repeat}/calls"));
        let canonical =
            scalar_expressions_text_with_options(&source, &uri, "main", &expressions, options)
                .unwrap();
        let legacy =
            scalar_expressions_with_options(&source, &uri, "main", &expressions, options).unwrap();
        assert_eq!(canonical.len(), expressions.len());
        assert_eq!(legacy.len(), expressions.len());
        for (index, (text, encoded)) in canonical.into_iter().zip(legacy).enumerate() {
            let read =
                |suffix: &str| fs::read(raw.join(format!("outer-{index}.{suffix}.bin"))).unwrap();
            assert_eq!(expressions[index].as_bytes(), read("expression"));
            let (channel, text, encoded) = if index == 0 {
                assert_eq!(read("status"), b"success\n");
                ("stdout", text.unwrap(), encoded.unwrap())
            } else {
                assert_eq!(read("status"), b"error\n");
                ("stderr", text.unwrap_err(), encoded.unwrap_err())
            };
            let expected_units = units(&read(&format!("{channel}.code-units")));
            assert_eq!(text.encode_utf16().collect::<Vec<_>>(), expected_units);
            assert_eq!(text.to_java_utf8().as_bytes(), read(channel));
            assert_eq!(encoded.as_bytes(), read(channel));
            assert_ne!(encoded.encode_utf16().collect::<Vec<_>>(), expected_units);
        }
        let single =
            scalar_expression_text_with_options(&source, &uri, "main", "payload", options).unwrap();
        assert_eq!(
            single.encode_utf16().collect::<Vec<_>>(),
            units(&fs::read(raw.join("outer-0.stdout.code-units.bin")).unwrap())
        );
    }
}
