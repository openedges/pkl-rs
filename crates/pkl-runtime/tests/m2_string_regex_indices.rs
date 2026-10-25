//! Pinned Java full result and Logger UTF-8/UTF-16BE comparisons in recorded query order.
#[path = "support/module_keys.rs"]
mod module_keys;
use pkl_runtime::{probe::SourceProbe, value::PklString};
type RawField = (&'static [u8], &'static [u8]);
struct Row {
    id: &'static str,
    expression: &'static str,
    kind: &'static str,
    stdout: RawField,
    stderr: RawField,
    logs: &'static [[RawField; 5]],
}
fn units(raw: &[u8]) -> Vec<u16> {
    let (pairs, tail) = raw.as_chunks::<2>();
    assert!(tail.is_empty());
    pairs.iter().map(|p| u16::from_be_bytes(*p)).collect()
}
fn matches(raw: RawField, actual: &[u16]) -> bool {
    units(raw.1) == actual
        && raw.0
            == PklString::from_utf16(actual.to_vec())
                .to_java_utf8()
                .as_bytes()
}
fn run(source: &str, uri: &str, rows: &[Row]) {
    for _ in 0..2 {
        let mut probe = SourceProbe::new_module_evaluator("compact", true, false, false).unwrap();
        let mut failures = Vec::new();
        for row in rows {
            probe
                .evaluate_module_source(uri, Some(source), row.expression, 0, None)
                .unwrap();
            let mut logs = 0;
            let mut terminal = None;
            for _ in 0..200_000 {
                probe.collect();
                let event = probe.poll(0, 1).unwrap();
                probe.collect();
                if event.kind == "yield" {
                    continue;
                }
                if event.kind == "module-request"
                    && module_keys::complete_probe(&mut probe, event.token.as_deref().unwrap(), 0)
                        .unwrap()
                {
                    continue;
                }
                if event.kind == "log" {
                    let log = event.log.as_ref().unwrap();
                    let position = format!(
                        "{}:{}-{}:{}",
                        log.start_line, log.start_column, log.end_line, log.end_column
                    );
                    let actual = [
                        log.level.encode_utf16().collect::<Vec<_>>(),
                        log.message.clone(),
                        log.module_uri.clone(),
                        log.member_name
                            .clone()
                            .unwrap_or_else(|| "null".encode_utf16().collect()),
                        position.encode_utf16().collect(),
                    ];
                    if row.logs.get(logs).is_none_or(|expected| {
                        expected
                            .iter()
                            .zip(actual.iter())
                            .any(|(e, a)| !matches(*e, a))
                    }) {
                        failures.push(format!("{} log {}: {:?}", row.id, logs, log));
                    }
                    logs += 1;
                    probe
                        .acknowledge_log(event.token.as_deref().unwrap(), None, 0)
                        .unwrap();
                    continue;
                }
                terminal = Some(event);
                break;
            }
            let event = terminal.expect("bounded Regex poll");
            let stdout = event.value.as_deref().unwrap_or(&[]);
            let stderr = event.error.as_deref().unwrap_or(&[]);
            if event.kind != row.kind
                || !matches(row.stdout, stdout)
                || !matches(row.stderr, stderr)
                || logs != row.logs.len()
            {
                failures.push(format!(
                    "{}: expected {} logs{}, got {:?} logs{}",
                    row.id,
                    row.kind,
                    row.logs.len(),
                    event,
                    logs
                ));
            }
        }
        probe.close();
        assert_eq!(probe.collect().slots, 0);
        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }
}

#[test]
fn java_string_regex_indices_all_raw_results_and_logs() {
    run(
        include_str!(
            "../../../tests/fixtures/m2-string-regex-indices/string-regex-indices/module.pkl"
        ),
        "file:///stage76/string-regex-indices.pkl",
        &[
            Row {
                id: "index-000-literal-first",
                expression: "\"ababa\".indexOf(\"aba\")",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-indices/string-regex-indices/000-index-000-literal-first/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-indices/string-regex-indices/000-index-000-literal-first/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-indices/string-regex-indices/000-index-000-literal-first/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-indices/string-regex-indices/000-index-000-literal-first/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "index-001-literal-last-overlap",
                expression: "\"ababa\".lastIndexOf(\"aba\")",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-indices/string-regex-indices/001-index-001-literal-last-overlap/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-indices/string-regex-indices/001-index-001-literal-last-overlap/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-indices/string-regex-indices/001-index-001-literal-last-overlap/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-indices/string-regex-indices/001-index-001-literal-last-overlap/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "index-002-regex-last-nonoverlap",
                expression: "\"ababa\".lastIndexOf(Regex(\"aba\"))",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-indices/string-regex-indices/002-index-002-regex-last-nonoverlap/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-indices/string-regex-indices/002-index-002-regex-last-nonoverlap/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-indices/string-regex-indices/002-index-002-regex-last-nonoverlap/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-indices/string-regex-indices/002-index-002-regex-last-nonoverlap/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "index-003-regex-code-point-index",
                expression: "\"😀x\".indexOf(Regex(\"x\"))",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-indices/string-regex-indices/003-index-003-regex-code-point-index/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-indices/string-regex-indices/003-index-003-regex-code-point-index/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-indices/string-regex-indices/003-index-003-regex-code-point-index/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-indices/string-regex-indices/003-index-003-regex-code-point-index/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "index-004-literal-code-point-index",
                expression: "\"😀x\".indexOf(\"x\")",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-indices/string-regex-indices/004-index-004-literal-code-point-index/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-indices/string-regex-indices/004-index-004-literal-code-point-index/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-indices/string-regex-indices/004-index-004-literal-code-point-index/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-indices/string-regex-indices/004-index-004-literal-code-point-index/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "index-005-literal-empty-first",
                expression: "\"😀x\".indexOf(\"\")",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-indices/string-regex-indices/005-index-005-literal-empty-first/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-indices/string-regex-indices/005-index-005-literal-empty-first/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-indices/string-regex-indices/005-index-005-literal-empty-first/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-indices/string-regex-indices/005-index-005-literal-empty-first/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "index-006-literal-empty-last",
                expression: "\"😀x\".lastIndexOf(\"\")",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-indices/string-regex-indices/006-index-006-literal-empty-last/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-indices/string-regex-indices/006-index-006-literal-empty-last/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-indices/string-regex-indices/006-index-006-literal-empty-last/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-indices/string-regex-indices/006-index-006-literal-empty-last/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "index-007-literal-low-inside-pair",
                expression: "\"😀\".indexOf(\"\\u{de00}\")",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-indices/string-regex-indices/007-index-007-literal-low-inside-pair/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-indices/string-regex-indices/007-index-007-literal-low-inside-pair/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-indices/string-regex-indices/007-index-007-literal-low-inside-pair/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-indices/string-regex-indices/007-index-007-literal-low-inside-pair/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "index-008-literal-lone-high",
                expression: "\"\\u{d800}x\".lastIndexOf(\"x\")",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-indices/string-regex-indices/008-index-008-literal-lone-high/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-indices/string-regex-indices/008-index-008-literal-lone-high/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-indices/string-regex-indices/008-index-008-literal-lone-high/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-indices/string-regex-indices/008-index-008-literal-lone-high/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "index-009-regex-empty-last",
                expression: "\"😀\".lastIndexOf(Regex(\"\"))",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-indices/string-regex-indices/009-index-009-regex-empty-last/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-indices/string-regex-indices/009-index-009-regex-empty-last/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-indices/string-regex-indices/009-index-009-regex-empty-last/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-indices/string-regex-indices/009-index-009-regex-empty-last/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "index-010-missing-literal-first",
                expression: "\"abc\".indexOf(\"z\")",
                kind: "error",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-indices/string-regex-indices/010-index-010-missing-literal-first/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-indices/string-regex-indices/010-index-010-missing-literal-first/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-indices/string-regex-indices/010-index-010-missing-literal-first/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-indices/string-regex-indices/010-index-010-missing-literal-first/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "index-011-missing-regex-first",
                expression: "\"abc\".indexOf(Regex(\"z\"))",
                kind: "error",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-indices/string-regex-indices/011-index-011-missing-regex-first/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-indices/string-regex-indices/011-index-011-missing-regex-first/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-indices/string-regex-indices/011-index-011-missing-regex-first/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-indices/string-regex-indices/011-index-011-missing-regex-first/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "index-012-missing-literal-last",
                expression: "\"abc\".lastIndexOf(\"z\")",
                kind: "error",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-indices/string-regex-indices/012-index-012-missing-literal-last/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-indices/string-regex-indices/012-index-012-missing-literal-last/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-indices/string-regex-indices/012-index-012-missing-literal-last/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-indices/string-regex-indices/012-index-012-missing-literal-last/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "index-013-missing-regex-last",
                expression: "\"abc\".lastIndexOf(Regex(\"z\"))",
                kind: "error",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-indices/string-regex-indices/013-index-013-missing-regex-last/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-indices/string-regex-indices/013-index-013-missing-regex-last/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-indices/string-regex-indices/013-index-013-missing-regex-last/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-indices/string-regex-indices/013-index-013-missing-regex-last/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "index-014-nullable-literal-first",
                expression: "\"abc\".indexOfOrNull(\"z\")",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-indices/string-regex-indices/014-index-014-nullable-literal-first/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-indices/string-regex-indices/014-index-014-nullable-literal-first/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-indices/string-regex-indices/014-index-014-nullable-literal-first/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-indices/string-regex-indices/014-index-014-nullable-literal-first/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "index-015-nullable-regex-last",
                expression: "\"abc\".lastIndexOfOrNull(Regex(\"z\"))",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-indices/string-regex-indices/015-index-015-nullable-regex-last/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-indices/string-regex-indices/015-index-015-nullable-regex-last/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-indices/string-regex-indices/015-index-015-nullable-regex-last/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-indices/string-regex-indices/015-index-015-nullable-regex-last/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "index-016-wrong-null",
                expression: "\"a\".indexOf(null)",
                kind: "error",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-indices/string-regex-indices/016-index-016-wrong-null/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-indices/string-regex-indices/016-index-016-wrong-null/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-indices/string-regex-indices/016-index-016-wrong-null/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-indices/string-regex-indices/016-index-016-wrong-null/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "index-017-wrong-int",
                expression: "\"a\".lastIndexOfOrNull(42)",
                kind: "error",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-indices/string-regex-indices/017-index-017-wrong-int/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-indices/string-regex-indices/017-index-017-wrong-int/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-indices/string-regex-indices/017-index-017-wrong-int/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-indices/string-regex-indices/017-index-017-wrong-int/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "index-018-argument-first",
                expression: "\"a\".indexOf(throw(\"argument first\"))",
                kind: "error",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-indices/string-regex-indices/018-index-018-argument-first/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-indices/string-regex-indices/018-index-018-argument-first/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-indices/string-regex-indices/018-index-018-argument-first/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-indices/string-regex-indices/018-index-018-argument-first/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "index-019-wrong-arity",
                expression: "\"a\".indexOf()",
                kind: "error",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-indices/string-regex-indices/019-index-019-wrong-arity/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-indices/string-regex-indices/019-index-019-wrong-arity/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-indices/string-regex-indices/019-index-019-wrong-arity/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-indices/string-regex-indices/019-index-019-wrong-arity/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "index-020-method-property",
                expression: "\"a\".lastIndexOf",
                kind: "error",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-indices/string-regex-indices/020-index-020-method-property/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-indices/string-regex-indices/020-index-020-method-property/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-indices/string-regex-indices/020-index-020-method-property/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-indices/string-regex-indices/020-index-020-method-property/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "index-021-implicit-constraint",
                expression: "\"😀x\" as String(indexOf(Regex(\"x\")) == 1)",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-indices/string-regex-indices/021-index-021-implicit-constraint/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-indices/string-regex-indices/021-index-021-implicit-constraint/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-indices/string-regex-indices/021-index-021-implicit-constraint/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-indices/string-regex-indices/021-index-021-implicit-constraint/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "index-022-no-match-render-width",
                expression: "\"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\".indexOf(Regex(\"z\"))",
                kind: "error",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-indices/string-regex-indices/022-index-022-no-match-render-width/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-indices/string-regex-indices/022-index-022-no-match-render-width/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-indices/string-regex-indices/022-index-022-no-match-render-width/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-indices/string-regex-indices/022-index-022-no-match-render-width/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
        ],
    );
}
