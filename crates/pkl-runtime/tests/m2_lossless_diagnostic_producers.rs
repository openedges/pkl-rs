//! Whole UTF-16 and Java UTF-8 payloads from independently captured Java calls.
use pkl_runtime::{probe::SourceProbe, value::PklString};
use std::{fs, path::Path};

fn bytes(root: &Path, name: &str) -> Vec<u8> {
    fs::read(root.join(name)).unwrap()
}
fn units(root: &Path, name: &str) -> Vec<u16> {
    let data = bytes(root, name);
    let (chunks, remainder) = data.as_chunks::<2>();
    assert!(remainder.is_empty());
    chunks
        .iter()
        .map(|unit| u16::from_be_bytes(*unit))
        .collect()
}
fn compare_text(actual: &[u16], root: &Path, stem: &str) {
    assert_eq!(
        actual,
        units(root, &format!("{stem}.code-units.bin")),
        "{stem}"
    );
    assert_eq!(
        PklString::from_utf16(actual.to_vec())
            .to_java_utf8()
            .as_bytes(),
        bytes(root, &format!("{stem}.bin")),
        "{stem}"
    );
}
fn compare(group: &str, expressions: &[&str], expected_logs: usize) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/m2-lossless-diagnostics")
        .join(group);
    let source = fs::read_to_string(root.join("workspace/main.pkl")).unwrap();
    let uri = fs::read_to_string(root.join("uri.txt")).unwrap();
    for repeat in 0..2 {
        let raw = root.join(format!("repeats/{repeat}/calls"));
        let expressions = expressions
            .iter()
            .map(|s| (*s).to_owned())
            .collect::<Vec<_>>();
        let mut probe = SourceProbe::new_expressions_text(
            &source,
            &uri,
            "main",
            &expressions,
            "compact",
            true,
            false,
            true,
        )
        .unwrap();
        let mut log_index = 0;
        for (index, expression) in expressions.iter().enumerate() {
            assert_eq!(
                expression.as_bytes(),
                bytes(&raw, &format!("outer-{index}.expression.bin"))
            );
            probe.start_path(index as u32, 0, None).unwrap();
            let mut terminal = false;
            for _ in 0..100_000 {
                let event = probe.poll(0, 1).unwrap();
                probe.collect();
                match event.kind {
                    "yield" => {}
                    "log" => {
                        let log = event.log.unwrap();
                        let stem = format!("log-{log_index}");
                        compare_text(&log.message, &raw, &format!("{stem}.message"));
                        compare_text(&log.module_uri, &raw, &format!("{stem}.uri"));
                        if let Some(member) = log.member_name {
                            compare_text(&member, &raw, &format!("{stem}.member"));
                        }
                        for (line_index, line) in log.source_lines.iter().enumerate() {
                            compare_text(line, &raw, &format!("{stem}.source-{line_index}"));
                        }
                        probe
                            .acknowledge_log(&event.token.unwrap(), None, 0)
                            .unwrap();
                        log_index += 1;
                    }
                    "value" | "error" => {
                        let status = bytes(&raw, &format!("outer-{index}.status.bin"));
                        let (channel, actual) = if event.kind == "value" {
                            assert_eq!(status, b"success\n");
                            ("stdout", event.value.unwrap())
                        } else {
                            assert_eq!(status, b"error\n");
                            assert_eq!(event.disposition, Some("language-error"));
                            ("stderr", event.error.unwrap())
                        };
                        compare_text(&actual, &raw, &format!("outer-{index}.{channel}"));
                        terminal = true;
                        break;
                    }
                    other => panic!("unexpected effect {other}"),
                }
            }
            assert!(terminal, "operation did not terminate");
        }
        assert_eq!(log_index, expected_logs);
        probe.close();
        assert_eq!(probe.collect().slots, 0);
    }
}
#[test]
fn original_throw_type_constraint_union_and_logger_payloads_remain_lossless() {
    compare(
        "producers",
        &[
            "payload",
            "throwing",
            "badInt",
            "badConstraint",
            "badUnion",
            "throwing",
            "loggingThenThrow",
            "hinted",
        ],
        1,
    );
}
#[test]
fn original_renderer_path_hint_and_repeated_failure_remain_lossless() {
    compare("hint", &["hinted", "hinted"], 0);
}

#[test]
fn original_literal_types_nullable_and_literal_union_retain_expected_units() {
    compare("literal", &["single", "nullable", "union"], 0);
}

#[test]
fn grouped_and_alias_unions_preserve_authored_labels_and_canonical_children() {
    compare("literal-origin", &["grouped", "aliasUnion"], 0);
}

#[test]
fn original_literal_union_lazy_reuse_defaults_and_retries_match_java() {
    compare(
        "literal-reuse",
        &[
            "original[0]",
            "grouped[0]",
            "aliased[0]",
            "reordered[0]",
            "duplicate[0]",
            "orderedGeneral[0]",
            "validReordered[0]",
            "specialFromGrouped[0]",
            "defaultFirstListing[0]",
            "defaultSecondListing[0]",
            "defaultFirst",
            "defaultSecond",
            "original[0]",
            "grouped[0]",
            "aliased[0]",
            "reordered[0]",
            "duplicate[0]",
            "orderedGeneral[0]",
            "validReordered[0]",
            "specialFromGrouped[0]",
            "defaultFirstListing[0]",
            "defaultSecondListing[0]",
            "defaultFirst",
            "defaultSecond",
        ],
        0,
    );
}
