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
    let mut failures = Vec::new();
    for repetition in 0..2 {
        let mut probe = SourceProbe::new_module_evaluator("compact", true, false, false).unwrap();
        let before = failures.len();
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
        eprintln!(
            "transform repetition {repetition}: {} rows, {} failures",
            rows.len(),
            failures.len() - before
        );
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn java_string_regex_transforms_all_raw_results_and_logs() {
    run(
        include_str!(
            "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/module.pkl"
        ),
        "file:///stage76/string-regex-transforms.pkl",
        &[
            Row {
                id: "transform-000-split-literal-ordinary",
                expression: "\"a,b,c\".split(\",\")",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/000-transform-000-split-literal-ordinary/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/000-transform-000-split-literal-ordinary/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/000-transform-000-split-literal-ordinary/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/000-transform-000-split-literal-ordinary/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-001-split-literal-leading-trailing",
                expression: "\",a,,b,\".split(\",\")",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/001-transform-001-split-literal-leading-trailing/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/001-transform-001-split-literal-leading-trailing/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/001-transform-001-split-literal-leading-trailing/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/001-transform-001-split-literal-leading-trailing/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-002-split-literal-all-separators",
                expression: "\",,,\".split(\",\")",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/002-transform-002-split-literal-all-separators/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/002-transform-002-split-literal-all-separators/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/002-transform-002-split-literal-all-separators/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/002-transform-002-split-literal-all-separators/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-003-split-literal-empty-input",
                expression: "\"\".split(\",\")",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/003-transform-003-split-literal-empty-input/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/003-transform-003-split-literal-empty-input/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/003-transform-003-split-literal-empty-input/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/003-transform-003-split-literal-empty-input/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-004-split-literal-empty-both",
                expression: "\"\".split(\"\")",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/004-transform-004-split-literal-empty-both/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/004-transform-004-split-literal-empty-both/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/004-transform-004-split-literal-empty-both/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/004-transform-004-split-literal-empty-both/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-005-split-literal-empty-pattern",
                expression: "\"ab\".split(\"\")",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/005-transform-005-split-literal-empty-pattern/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/005-transform-005-split-literal-empty-pattern/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/005-transform-005-split-literal-empty-pattern/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/005-transform-005-split-literal-empty-pattern/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-006-split-literal-supplementary-empty",
                expression: "\"😀x\".split(\"\")",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/006-transform-006-split-literal-supplementary-empty/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/006-transform-006-split-literal-supplementary-empty/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/006-transform-006-split-literal-supplementary-empty/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/006-transform-006-split-literal-supplementary-empty/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-007-split-literal-lone-high",
                expression: "\"\\u{d800}x\".split(\"\")",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/007-transform-007-split-literal-lone-high/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/007-transform-007-split-literal-lone-high/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/007-transform-007-split-literal-lone-high/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/007-transform-007-split-literal-lone-high/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-008-split-literal-pair-low",
                expression: "\"😀x\".split(\"\\u{de00}\")",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/008-transform-008-split-literal-pair-low/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/008-transform-008-split-literal-pair-low/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/008-transform-008-split-literal-pair-low/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/008-transform-008-split-literal-pair-low/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-009-split-literal-metacharacter",
                expression: "\"a.b.c\".split(\".\")",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/009-transform-009-split-literal-metacharacter/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/009-transform-009-split-literal-metacharacter/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/009-transform-009-split-literal-metacharacter/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/009-transform-009-split-literal-metacharacter/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-010-limit-literal-1",
                expression: "\",a,,b,\".splitLimit(\",\", 1)",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/010-transform-010-limit-literal-1/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/010-transform-010-limit-literal-1/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/010-transform-010-limit-literal-1/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/010-transform-010-limit-literal-1/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-011-limit-literal-2",
                expression: "\",a,,b,\".splitLimit(\",\", 2)",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/011-transform-011-limit-literal-2/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/011-transform-011-limit-literal-2/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/011-transform-011-limit-literal-2/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/011-transform-011-limit-literal-2/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-012-limit-literal-0",
                expression: "\",a,,b,\".splitLimit(\",\", 0)",
                kind: "error",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/012-transform-012-limit-literal-0/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/012-transform-012-limit-literal-0/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/012-transform-012-limit-literal-0/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/012-transform-012-limit-literal-0/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-013-limit-literal-negative-1",
                expression: "\",a,,b,\".splitLimit(\",\", -1)",
                kind: "error",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/013-transform-013-limit-literal-negative-1/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/013-transform-013-limit-literal-negative-1/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/013-transform-013-limit-literal-negative-1/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/013-transform-013-limit-literal-negative-1/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-014-limit-literal-2147483647",
                expression: "\",a,,b,\".splitLimit(\",\", 2147483647)",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/014-transform-014-limit-literal-2147483647/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/014-transform-014-limit-literal-2147483647/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/014-transform-014-limit-literal-2147483647/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/014-transform-014-limit-literal-2147483647/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-015-limit-literal-2147483648",
                expression: "\",a,,b,\".splitLimit(\",\", 2147483648)",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/015-transform-015-limit-literal-2147483648/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/015-transform-015-limit-literal-2147483648/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/015-transform-015-limit-literal-2147483648/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/015-transform-015-limit-literal-2147483648/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-016-limit-literal-4294967296",
                expression: "\",a,,b,\".splitLimit(\",\", 4294967296)",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/016-transform-016-limit-literal-4294967296/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/016-transform-016-limit-literal-4294967296/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/016-transform-016-limit-literal-4294967296/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/016-transform-016-limit-literal-4294967296/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-017-limit-literal-4294967297",
                expression: "\",a,,b,\".splitLimit(\",\", 4294967297)",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/017-transform-017-limit-literal-4294967297/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/017-transform-017-limit-literal-4294967297/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/017-transform-017-limit-literal-4294967297/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/017-transform-017-limit-literal-4294967297/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-018-limit-literal-9223372036854775807",
                expression: "\",a,,b,\".splitLimit(\",\", 9223372036854775807)",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/018-transform-018-limit-literal-9223372036854775807/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/018-transform-018-limit-literal-9223372036854775807/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/018-transform-018-limit-literal-9223372036854775807/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/018-transform-018-limit-literal-9223372036854775807/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-019-split-regex-ordinary",
                expression: "\"a,b,c\".split(Regex(\",\"))",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/019-transform-019-split-regex-ordinary/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/019-transform-019-split-regex-ordinary/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/019-transform-019-split-regex-ordinary/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/019-transform-019-split-regex-ordinary/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-020-split-regex-leading-trailing",
                expression: "\",a,,b,\".split(Regex(\",\"))",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/020-transform-020-split-regex-leading-trailing/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/020-transform-020-split-regex-leading-trailing/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/020-transform-020-split-regex-leading-trailing/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/020-transform-020-split-regex-leading-trailing/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-021-split-regex-all-separators",
                expression: "\",,,\".split(Regex(\",\"))",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/021-transform-021-split-regex-all-separators/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/021-transform-021-split-regex-all-separators/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/021-transform-021-split-regex-all-separators/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/021-transform-021-split-regex-all-separators/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-022-split-regex-empty-input",
                expression: "\"\".split(Regex(\",\"))",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/022-transform-022-split-regex-empty-input/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/022-transform-022-split-regex-empty-input/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/022-transform-022-split-regex-empty-input/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/022-transform-022-split-regex-empty-input/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-023-split-regex-empty-both",
                expression: "\"\".split(Regex(\"\"))",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/023-transform-023-split-regex-empty-both/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/023-transform-023-split-regex-empty-both/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/023-transform-023-split-regex-empty-both/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/023-transform-023-split-regex-empty-both/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-024-split-regex-empty-pattern",
                expression: "\"ab\".split(Regex(\"\"))",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/024-transform-024-split-regex-empty-pattern/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/024-transform-024-split-regex-empty-pattern/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/024-transform-024-split-regex-empty-pattern/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/024-transform-024-split-regex-empty-pattern/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-025-split-regex-supplementary-empty",
                expression: "\"😀x\".split(Regex(\"\"))",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/025-transform-025-split-regex-supplementary-empty/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/025-transform-025-split-regex-supplementary-empty/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/025-transform-025-split-regex-supplementary-empty/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/025-transform-025-split-regex-supplementary-empty/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-026-split-regex-lone-high",
                expression: "\"\\u{d800}x\".split(Regex(\"\"))",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/026-transform-026-split-regex-lone-high/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/026-transform-026-split-regex-lone-high/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/026-transform-026-split-regex-lone-high/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/026-transform-026-split-regex-lone-high/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-027-split-regex-pair-low",
                expression: "\"😀x\".split(Regex(\"\\u{de00}\"))",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/027-transform-027-split-regex-pair-low/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/027-transform-027-split-regex-pair-low/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/027-transform-027-split-regex-pair-low/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/027-transform-027-split-regex-pair-low/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-028-split-regex-metacharacter",
                expression: "\"a.b.c\".split(Regex(\".\"))",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/028-transform-028-split-regex-metacharacter/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/028-transform-028-split-regex-metacharacter/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/028-transform-028-split-regex-metacharacter/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/028-transform-028-split-regex-metacharacter/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-029-limit-regex-1",
                expression: "\",a,,b,\".splitLimit(Regex(\",\"), 1)",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/029-transform-029-limit-regex-1/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/029-transform-029-limit-regex-1/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/029-transform-029-limit-regex-1/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/029-transform-029-limit-regex-1/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-030-limit-regex-2",
                expression: "\",a,,b,\".splitLimit(Regex(\",\"), 2)",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/030-transform-030-limit-regex-2/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/030-transform-030-limit-regex-2/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/030-transform-030-limit-regex-2/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/030-transform-030-limit-regex-2/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-031-limit-regex-0",
                expression: "\",a,,b,\".splitLimit(Regex(\",\"), 0)",
                kind: "error",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/031-transform-031-limit-regex-0/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/031-transform-031-limit-regex-0/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/031-transform-031-limit-regex-0/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/031-transform-031-limit-regex-0/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-032-limit-regex-negative-1",
                expression: "\",a,,b,\".splitLimit(Regex(\",\"), -1)",
                kind: "error",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/032-transform-032-limit-regex-negative-1/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/032-transform-032-limit-regex-negative-1/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/032-transform-032-limit-regex-negative-1/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/032-transform-032-limit-regex-negative-1/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-033-limit-regex-2147483647",
                expression: "\",a,,b,\".splitLimit(Regex(\",\"), 2147483647)",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/033-transform-033-limit-regex-2147483647/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/033-transform-033-limit-regex-2147483647/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/033-transform-033-limit-regex-2147483647/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/033-transform-033-limit-regex-2147483647/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-034-limit-regex-2147483648",
                expression: "\",a,,b,\".splitLimit(Regex(\",\"), 2147483648)",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/034-transform-034-limit-regex-2147483648/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/034-transform-034-limit-regex-2147483648/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/034-transform-034-limit-regex-2147483648/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/034-transform-034-limit-regex-2147483648/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-035-limit-regex-4294967296",
                expression: "\",a,,b,\".splitLimit(Regex(\",\"), 4294967296)",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/035-transform-035-limit-regex-4294967296/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/035-transform-035-limit-regex-4294967296/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/035-transform-035-limit-regex-4294967296/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/035-transform-035-limit-regex-4294967296/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-036-limit-regex-4294967297",
                expression: "\",a,,b,\".splitLimit(Regex(\",\"), 4294967297)",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/036-transform-036-limit-regex-4294967297/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/036-transform-036-limit-regex-4294967297/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/036-transform-036-limit-regex-4294967297/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/036-transform-036-limit-regex-4294967297/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-037-limit-regex-9223372036854775807",
                expression: "\",a,,b,\".splitLimit(Regex(\",\"), 9223372036854775807)",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/037-transform-037-limit-regex-9223372036854775807/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/037-transform-037-limit-regex-9223372036854775807/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/037-transform-037-limit-regex-9223372036854775807/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/037-transform-037-limit-regex-9223372036854775807/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-038-split-zero-width-lookahead",
                expression: "\"aba\".split(Regex(\"(?=a)\"))",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/038-transform-038-split-zero-width-lookahead/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/038-transform-038-split-zero-width-lookahead/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/038-transform-038-split-zero-width-lookahead/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/038-transform-038-split-zero-width-lookahead/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-039-split-capturing-separator",
                expression: "\"a,b,,c\".split(Regex(\"(,)\"))",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/039-transform-039-split-capturing-separator/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/039-transform-039-split-capturing-separator/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/039-transform-039-split-capturing-separator/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/039-transform-039-split-capturing-separator/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-040-split-positive-limit-empty",
                expression: "\"😀x\".splitLimit(Regex(\"\"), 2)",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/040-transform-040-split-positive-limit-empty/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/040-transform-040-split-positive-limit-empty/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/040-transform-040-split-positive-limit-empty/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/040-transform-040-split-positive-limit-empty/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-041-split-wrong-pattern",
                expression: "\"a\".split(null)",
                kind: "error",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/041-transform-041-split-wrong-pattern/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/041-transform-041-split-wrong-pattern/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/041-transform-041-split-wrong-pattern/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/041-transform-041-split-wrong-pattern/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-042-split-wrong-limit-type",
                expression: "\"a\".splitLimit(\",\", null)",
                kind: "error",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/042-transform-042-split-wrong-limit-type/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/042-transform-042-split-wrong-limit-type/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/042-transform-042-split-wrong-limit-type/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/042-transform-042-split-wrong-limit-type/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-043-split-eager-argument-order",
                expression: "\"a\".splitLimit(null, throw(\"limit evaluated\"))",
                kind: "error",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/043-transform-043-split-eager-argument-order/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/043-transform-043-split-eager-argument-order/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/043-transform-043-split-eager-argument-order/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/043-transform-043-split-eager-argument-order/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-044-replaceall-literal-overlap",
                expression: "\"ababa\".replaceAll(\"aba\", \"x\")",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/044-transform-044-replaceall-literal-overlap/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/044-transform-044-replaceall-literal-overlap/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/044-transform-044-replaceall-literal-overlap/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/044-transform-044-replaceall-literal-overlap/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-045-replaceall-regex-overlap",
                expression: "\"ababa\".replaceAll(Regex(\"aba\"), \"x\")",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/045-transform-045-replaceall-regex-overlap/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/045-transform-045-replaceall-regex-overlap/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/045-transform-045-replaceall-regex-overlap/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/045-transform-045-replaceall-regex-overlap/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-046-replaceall-literal-empty-pair",
                expression: "\"😀x\".replaceAll(\"\", \"-\")",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/046-transform-046-replaceall-literal-empty-pair/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/046-transform-046-replaceall-literal-empty-pair/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/046-transform-046-replaceall-literal-empty-pair/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/046-transform-046-replaceall-literal-empty-pair/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-047-replaceall-regex-empty-pair",
                expression: "\"😀x\".replaceAll(Regex(\"\"), \"-\")",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/047-transform-047-replaceall-regex-empty-pair/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/047-transform-047-replaceall-regex-empty-pair/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/047-transform-047-replaceall-regex-empty-pair/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/047-transform-047-replaceall-regex-empty-pair/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-048-replaceall-literal-low-pair",
                expression: "\"😀x\".replaceAll(\"\\u{de00}\", \"x\")",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/048-transform-048-replaceall-literal-low-pair/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/048-transform-048-replaceall-literal-low-pair/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/048-transform-048-replaceall-literal-low-pair/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/048-transform-048-replaceall-literal-low-pair/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-049-replaceall-regex-low-pair",
                expression: "\"😀x\".replaceAll(Regex(\"\\u{de00}\"), \"x\")",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/049-transform-049-replaceall-regex-low-pair/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/049-transform-049-replaceall-regex-low-pair/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/049-transform-049-replaceall-regex-low-pair/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/049-transform-049-replaceall-regex-low-pair/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-050-replaceall-literal-dollar",
                expression: "\"aba\".replaceAll(\"a\", \"$1\")",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/050-transform-050-replaceall-literal-dollar/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/050-transform-050-replaceall-literal-dollar/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/050-transform-050-replaceall-literal-dollar/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/050-transform-050-replaceall-literal-dollar/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-051-replaceall-group",
                expression: "\"ab\".replaceAll(Regex(\"(a)\"), \"$1\")",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/051-transform-051-replaceall-group/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/051-transform-051-replaceall-group/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/051-transform-051-replaceall-group/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/051-transform-051-replaceall-group/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-052-replaceall-absent-group",
                expression: "\"b\".replaceAll(Regex(\"(a)?b\"), \"<$1>\")",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/052-transform-052-replaceall-absent-group/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/052-transform-052-replaceall-absent-group/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/052-transform-052-replaceall-absent-group/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/052-transform-052-replaceall-absent-group/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-053-replaceall-bad-dollar-match",
                expression: "\"a\".replaceAll(Regex(\"a\"), \"$\")",
                kind: "error",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/053-transform-053-replaceall-bad-dollar-match/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/053-transform-053-replaceall-bad-dollar-match/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/053-transform-053-replaceall-bad-dollar-match/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/053-transform-053-replaceall-bad-dollar-match/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-054-replaceall-bad-dollar-no-match",
                expression: "\"a\".replaceAll(Regex(\"z\"), \"$\")",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/054-transform-054-replaceall-bad-dollar-no-match/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/054-transform-054-replaceall-bad-dollar-no-match/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/054-transform-054-replaceall-bad-dollar-no-match/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/054-transform-054-replaceall-bad-dollar-no-match/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-055-replaceall-bad-slash-match",
                expression: "\"a\".replaceAll(Regex(\"a\"), \"\\\\\")",
                kind: "error",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/055-transform-055-replaceall-bad-slash-match/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/055-transform-055-replaceall-bad-slash-match/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/055-transform-055-replaceall-bad-slash-match/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/055-transform-055-replaceall-bad-slash-match/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-056-replaceall-bad-slash-no-match",
                expression: "\"a\".replaceAll(Regex(\"z\"), \"\\\\\")",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/056-transform-056-replaceall-bad-slash-no-match/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/056-transform-056-replaceall-bad-slash-no-match/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/056-transform-056-replaceall-bad-slash-no-match/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/056-transform-056-replaceall-bad-slash-no-match/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-057-replaceall-lookbehind",
                expression: "\"ab\".replaceAll(Regex(\"(?<=a)b\"), \"x\")",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/057-transform-057-replaceall-lookbehind/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/057-transform-057-replaceall-lookbehind/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/057-transform-057-replaceall-lookbehind/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/057-transform-057-replaceall-lookbehind/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-058-replaceall-lookahead",
                expression: "\"ab\".replaceAll(Regex(\"a(?=b)\"), \"x\")",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/058-transform-058-replaceall-lookahead/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/058-transform-058-replaceall-lookahead/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/058-transform-058-replaceall-lookahead/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/058-transform-058-replaceall-lookahead/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-059-replacefirst-literal-overlap",
                expression: "\"ababa\".replaceFirst(\"aba\", \"x\")",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/059-transform-059-replacefirst-literal-overlap/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/059-transform-059-replacefirst-literal-overlap/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/059-transform-059-replacefirst-literal-overlap/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/059-transform-059-replacefirst-literal-overlap/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-060-replacefirst-regex-overlap",
                expression: "\"ababa\".replaceFirst(Regex(\"aba\"), \"x\")",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/060-transform-060-replacefirst-regex-overlap/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/060-transform-060-replacefirst-regex-overlap/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/060-transform-060-replacefirst-regex-overlap/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/060-transform-060-replacefirst-regex-overlap/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-061-replacefirst-literal-empty-pair",
                expression: "\"😀x\".replaceFirst(\"\", \"-\")",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/061-transform-061-replacefirst-literal-empty-pair/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/061-transform-061-replacefirst-literal-empty-pair/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/061-transform-061-replacefirst-literal-empty-pair/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/061-transform-061-replacefirst-literal-empty-pair/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-062-replacefirst-regex-empty-pair",
                expression: "\"😀x\".replaceFirst(Regex(\"\"), \"-\")",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/062-transform-062-replacefirst-regex-empty-pair/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/062-transform-062-replacefirst-regex-empty-pair/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/062-transform-062-replacefirst-regex-empty-pair/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/062-transform-062-replacefirst-regex-empty-pair/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-063-replacefirst-literal-low-pair",
                expression: "\"😀x\".replaceFirst(\"\\u{de00}\", \"x\")",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/063-transform-063-replacefirst-literal-low-pair/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/063-transform-063-replacefirst-literal-low-pair/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/063-transform-063-replacefirst-literal-low-pair/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/063-transform-063-replacefirst-literal-low-pair/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-064-replacefirst-regex-low-pair",
                expression: "\"😀x\".replaceFirst(Regex(\"\\u{de00}\"), \"x\")",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/064-transform-064-replacefirst-regex-low-pair/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/064-transform-064-replacefirst-regex-low-pair/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/064-transform-064-replacefirst-regex-low-pair/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/064-transform-064-replacefirst-regex-low-pair/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-065-replacefirst-literal-dollar",
                expression: "\"aba\".replaceFirst(\"a\", \"$1\")",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/065-transform-065-replacefirst-literal-dollar/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/065-transform-065-replacefirst-literal-dollar/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/065-transform-065-replacefirst-literal-dollar/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/065-transform-065-replacefirst-literal-dollar/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-066-replacefirst-group",
                expression: "\"ab\".replaceFirst(Regex(\"(a)\"), \"$1\")",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/066-transform-066-replacefirst-group/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/066-transform-066-replacefirst-group/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/066-transform-066-replacefirst-group/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/066-transform-066-replacefirst-group/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-067-replacefirst-absent-group",
                expression: "\"b\".replaceFirst(Regex(\"(a)?b\"), \"<$1>\")",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/067-transform-067-replacefirst-absent-group/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/067-transform-067-replacefirst-absent-group/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/067-transform-067-replacefirst-absent-group/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/067-transform-067-replacefirst-absent-group/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-068-replacefirst-bad-dollar-match",
                expression: "\"a\".replaceFirst(Regex(\"a\"), \"$\")",
                kind: "error",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/068-transform-068-replacefirst-bad-dollar-match/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/068-transform-068-replacefirst-bad-dollar-match/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/068-transform-068-replacefirst-bad-dollar-match/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/068-transform-068-replacefirst-bad-dollar-match/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-069-replacefirst-bad-dollar-no-match",
                expression: "\"a\".replaceFirst(Regex(\"z\"), \"$\")",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/069-transform-069-replacefirst-bad-dollar-no-match/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/069-transform-069-replacefirst-bad-dollar-no-match/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/069-transform-069-replacefirst-bad-dollar-no-match/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/069-transform-069-replacefirst-bad-dollar-no-match/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-070-replacefirst-bad-slash-match",
                expression: "\"a\".replaceFirst(Regex(\"a\"), \"\\\\\")",
                kind: "error",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/070-transform-070-replacefirst-bad-slash-match/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/070-transform-070-replacefirst-bad-slash-match/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/070-transform-070-replacefirst-bad-slash-match/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/070-transform-070-replacefirst-bad-slash-match/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-071-replacefirst-bad-slash-no-match",
                expression: "\"a\".replaceFirst(Regex(\"z\"), \"\\\\\")",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/071-transform-071-replacefirst-bad-slash-no-match/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/071-transform-071-replacefirst-bad-slash-no-match/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/071-transform-071-replacefirst-bad-slash-no-match/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/071-transform-071-replacefirst-bad-slash-no-match/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-072-replacefirst-lookbehind",
                expression: "\"ab\".replaceFirst(Regex(\"(?<=a)b\"), \"x\")",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/072-transform-072-replacefirst-lookbehind/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/072-transform-072-replacefirst-lookbehind/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/072-transform-072-replacefirst-lookbehind/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/072-transform-072-replacefirst-lookbehind/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-073-replacefirst-lookahead",
                expression: "\"ab\".replaceFirst(Regex(\"a(?=b)\"), \"x\")",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/073-transform-073-replacefirst-lookahead/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/073-transform-073-replacefirst-lookahead/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/073-transform-073-replacefirst-lookahead/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/073-transform-073-replacefirst-lookahead/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-074-replacelast-literal-overlap",
                expression: "\"ababa\".replaceLast(\"aba\", \"x\")",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/074-transform-074-replacelast-literal-overlap/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/074-transform-074-replacelast-literal-overlap/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/074-transform-074-replacelast-literal-overlap/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/074-transform-074-replacelast-literal-overlap/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-075-replacelast-regex-overlap",
                expression: "\"ababa\".replaceLast(Regex(\"aba\"), \"x\")",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/075-transform-075-replacelast-regex-overlap/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/075-transform-075-replacelast-regex-overlap/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/075-transform-075-replacelast-regex-overlap/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/075-transform-075-replacelast-regex-overlap/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-076-replacelast-literal-empty-pair",
                expression: "\"😀x\".replaceLast(\"\", \"-\")",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/076-transform-076-replacelast-literal-empty-pair/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/076-transform-076-replacelast-literal-empty-pair/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/076-transform-076-replacelast-literal-empty-pair/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/076-transform-076-replacelast-literal-empty-pair/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-077-replacelast-regex-empty-pair",
                expression: "\"😀x\".replaceLast(Regex(\"\"), \"-\")",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/077-transform-077-replacelast-regex-empty-pair/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/077-transform-077-replacelast-regex-empty-pair/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/077-transform-077-replacelast-regex-empty-pair/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/077-transform-077-replacelast-regex-empty-pair/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-078-replacelast-literal-low-pair",
                expression: "\"😀x\".replaceLast(\"\\u{de00}\", \"x\")",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/078-transform-078-replacelast-literal-low-pair/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/078-transform-078-replacelast-literal-low-pair/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/078-transform-078-replacelast-literal-low-pair/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/078-transform-078-replacelast-literal-low-pair/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-079-replacelast-regex-low-pair",
                expression: "\"😀x\".replaceLast(Regex(\"\\u{de00}\"), \"x\")",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/079-transform-079-replacelast-regex-low-pair/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/079-transform-079-replacelast-regex-low-pair/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/079-transform-079-replacelast-regex-low-pair/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/079-transform-079-replacelast-regex-low-pair/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-080-replacelast-literal-dollar",
                expression: "\"aba\".replaceLast(\"a\", \"$1\")",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/080-transform-080-replacelast-literal-dollar/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/080-transform-080-replacelast-literal-dollar/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/080-transform-080-replacelast-literal-dollar/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/080-transform-080-replacelast-literal-dollar/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-081-replacelast-group",
                expression: "\"ab\".replaceLast(Regex(\"(a)\"), \"$1\")",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/081-transform-081-replacelast-group/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/081-transform-081-replacelast-group/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/081-transform-081-replacelast-group/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/081-transform-081-replacelast-group/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-082-replacelast-absent-group",
                expression: "\"b\".replaceLast(Regex(\"(a)?b\"), \"<$1>\")",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/082-transform-082-replacelast-absent-group/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/082-transform-082-replacelast-absent-group/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/082-transform-082-replacelast-absent-group/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/082-transform-082-replacelast-absent-group/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-083-replacelast-bad-dollar-match",
                expression: "\"a\".replaceLast(Regex(\"a\"), \"$\")",
                kind: "error",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/083-transform-083-replacelast-bad-dollar-match/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/083-transform-083-replacelast-bad-dollar-match/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/083-transform-083-replacelast-bad-dollar-match/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/083-transform-083-replacelast-bad-dollar-match/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-084-replacelast-bad-dollar-no-match",
                expression: "\"a\".replaceLast(Regex(\"z\"), \"$\")",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/084-transform-084-replacelast-bad-dollar-no-match/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/084-transform-084-replacelast-bad-dollar-no-match/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/084-transform-084-replacelast-bad-dollar-no-match/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/084-transform-084-replacelast-bad-dollar-no-match/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-085-replacelast-bad-slash-match",
                expression: "\"a\".replaceLast(Regex(\"a\"), \"\\\\\")",
                kind: "error",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/085-transform-085-replacelast-bad-slash-match/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/085-transform-085-replacelast-bad-slash-match/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/085-transform-085-replacelast-bad-slash-match/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/085-transform-085-replacelast-bad-slash-match/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-086-replacelast-bad-slash-no-match",
                expression: "\"a\".replaceLast(Regex(\"z\"), \"\\\\\")",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/086-transform-086-replacelast-bad-slash-no-match/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/086-transform-086-replacelast-bad-slash-no-match/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/086-transform-086-replacelast-bad-slash-no-match/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/086-transform-086-replacelast-bad-slash-no-match/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-087-replacelast-lookbehind",
                expression: "\"ab\".replaceLast(Regex(\"(?<=a)b\"), \"x\")",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/087-transform-087-replacelast-lookbehind/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/087-transform-087-replacelast-lookbehind/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/087-transform-087-replacelast-lookbehind/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/087-transform-087-replacelast-lookbehind/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-088-replacelast-lookahead",
                expression: "\"ab\".replaceLast(Regex(\"a(?=b)\"), \"x\")",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/088-transform-088-replacelast-lookahead/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/088-transform-088-replacelast-lookahead/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/088-transform-088-replacelast-lookahead/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/088-transform-088-replacelast-lookahead/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-089-named-group",
                expression: "\"ab\".replaceAll(Regex(\"(?<a>a)\"), \"${a}\")",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/089-transform-089-named-group/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/089-transform-089-named-group/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/089-transform-089-named-group/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/089-transform-089-named-group/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-090-missing-named-group",
                expression: "\"ab\".replaceAll(Regex(\"(a)\"), \"${missing}\")",
                kind: "error",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/090-transform-090-missing-named-group/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/090-transform-090-missing-named-group/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/090-transform-090-missing-named-group/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/090-transform-090-missing-named-group/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-091-malformed-named-group",
                expression: "\"ab\".replaceAll(Regex(\"(a)\"), \"${a\")",
                kind: "error",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/091-transform-091-malformed-named-group/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/091-transform-091-malformed-named-group/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/091-transform-091-malformed-named-group/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/091-transform-091-malformed-named-group/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-092-numeric-greedy-group",
                expression: "\"ab\".replaceAll(Regex(\"(a)\"), \"$12\")",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/092-transform-092-numeric-greedy-group/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/092-transform-092-numeric-greedy-group/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/092-transform-092-numeric-greedy-group/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/092-transform-092-numeric-greedy-group/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-093-numeric-leading-zero",
                expression: "\"ab\".replaceAll(Regex(\"(a)\"), \"$01\")",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/093-transform-093-numeric-leading-zero/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/093-transform-093-numeric-leading-zero/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/093-transform-093-numeric-leading-zero/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/093-transform-093-numeric-leading-zero/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-094-whole-match-group",
                expression: "\"ab\".replaceAll(Regex(\"(a)\"), \"$0\")",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/094-transform-094-whole-match-group/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/094-transform-094-whole-match-group/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/094-transform-094-whole-match-group/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/094-transform-094-whole-match-group/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-095-missing-numeric-group",
                expression: "\"ab\".replaceAll(Regex(\"(a)\"), \"$9\")",
                kind: "error",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/095-transform-095-missing-numeric-group/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/095-transform-095-missing-numeric-group/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/095-transform-095-missing-numeric-group/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/095-transform-095-missing-numeric-group/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-096-escaped-dollar",
                expression: "\"ab\".replaceAll(Regex(\"(a)\"), \"\\\\$1\")",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/096-transform-096-escaped-dollar/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/096-transform-096-escaped-dollar/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/096-transform-096-escaped-dollar/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/096-transform-096-escaped-dollar/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-097-escaped-backslash",
                expression: "\"ab\".replaceAll(Regex(\"(a)\"), \"\\\\\\\\\")",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/097-transform-097-escaped-backslash/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/097-transform-097-escaped-backslash/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/097-transform-097-escaped-backslash/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/097-transform-097-escaped-backslash/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-098-replacement-null-pattern",
                expression: "\"a\".replaceAll(null, \"x\")",
                kind: "error",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/098-transform-098-replacement-null-pattern/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/098-transform-098-replacement-null-pattern/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/098-transform-098-replacement-null-pattern/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/098-transform-098-replacement-null-pattern/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-099-replacement-null-value",
                expression: "\"a\".replaceAll(Regex(\"z\"), null)",
                kind: "error",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/099-transform-099-replacement-null-value/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/099-transform-099-replacement-null-value/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/099-transform-099-replacement-null-value/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/099-transform-099-replacement-null-value/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-100-replaceallmapped-literal",
                expression: "\"aba\".replaceAllMapped(\"a\", (m) -> m.value)",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/100-transform-100-replaceallmapped-literal/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/100-transform-100-replaceallmapped-literal/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/100-transform-100-replaceallmapped-literal/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/100-transform-100-replaceallmapped-literal/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-101-replaceallmapped-group",
                expression: "\"aba\".replaceAllMapped(Regex(\"(a)\"), (m) -> m.groups[1].value)",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/101-transform-101-replaceallmapped-group/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/101-transform-101-replaceallmapped-group/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/101-transform-101-replaceallmapped-group/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/101-transform-101-replaceallmapped-group/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-102-replaceallmapped-dollar",
                expression: "\"aba\".replaceAllMapped(Regex(\"a\"), (m) -> \"$\")",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/102-transform-102-replaceallmapped-dollar/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/102-transform-102-replaceallmapped-dollar/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/102-transform-102-replaceallmapped-dollar/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/102-transform-102-replaceallmapped-dollar/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-103-replaceallmapped-slash",
                expression: "\"aba\".replaceAllMapped(Regex(\"a\"), (m) -> \"\\\\\")",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/103-transform-103-replaceallmapped-slash/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/103-transform-103-replaceallmapped-slash/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/103-transform-103-replaceallmapped-slash/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/103-transform-103-replaceallmapped-slash/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-104-replaceallmapped-empty-pair",
                expression: "\"😀x\".replaceAllMapped(Regex(\"\"), (m) -> \"-\")",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/104-transform-104-replaceallmapped-empty-pair/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/104-transform-104-replaceallmapped-empty-pair/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/104-transform-104-replaceallmapped-empty-pair/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/104-transform-104-replaceallmapped-empty-pair/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-105-replaceallmapped-arity-no-match",
                expression: "\"a\".replaceAllMapped(Regex(\"z\"), (a, b) -> \"x\")",
                kind: "error",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/105-transform-105-replaceallmapped-arity-no-match/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/105-transform-105-replaceallmapped-arity-no-match/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/105-transform-105-replaceallmapped-arity-no-match/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/105-transform-105-replaceallmapped-arity-no-match/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-106-replaceallmapped-arity-match",
                expression: "\"a\".replaceAllMapped(Regex(\"a\"), (a, b) -> \"x\")",
                kind: "error",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/106-transform-106-replaceallmapped-arity-match/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/106-transform-106-replaceallmapped-arity-match/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/106-transform-106-replaceallmapped-arity-match/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/106-transform-106-replaceallmapped-arity-match/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-107-replaceallmapped-return-no-match",
                expression: "\"a\".replaceAllMapped(Regex(\"z\"), (m) -> 42)",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/107-transform-107-replaceallmapped-return-no-match/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/107-transform-107-replaceallmapped-return-no-match/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/107-transform-107-replaceallmapped-return-no-match/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/107-transform-107-replaceallmapped-return-no-match/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-108-replaceallmapped-return-match",
                expression: "\"a\".replaceAllMapped(Regex(\"a\"), (m) -> 42)",
                kind: "error",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/108-transform-108-replaceallmapped-return-match/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/108-transform-108-replaceallmapped-return-match/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/108-transform-108-replaceallmapped-return-match/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/108-transform-108-replaceallmapped-return-match/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-109-replaceallmapped-null-no-match",
                expression: "\"a\".replaceAllMapped(Regex(\"z\"), null)",
                kind: "error",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/109-transform-109-replaceallmapped-null-no-match/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/109-transform-109-replaceallmapped-null-no-match/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/109-transform-109-replaceallmapped-null-no-match/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/109-transform-109-replaceallmapped-null-no-match/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-110-replaceallmapped-lookbehind-trace",
                expression: "\"ab\".replaceAllMapped(Regex(\"(?<=a)b\"), (m) -> trace(m.value))",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/110-transform-110-replaceallmapped-lookbehind-trace/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/110-transform-110-replaceallmapped-lookbehind-trace/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/110-transform-110-replaceallmapped-lookbehind-trace/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/110-transform-110-replaceallmapped-lookbehind-trace/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[[
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/110-transform-110-replaceallmapped-lookbehind-trace/log-000.level.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/110-transform-110-replaceallmapped-lookbehind-trace/log-000.level.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/110-transform-110-replaceallmapped-lookbehind-trace/log-000.message.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/110-transform-110-replaceallmapped-lookbehind-trace/log-000.message.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/110-transform-110-replaceallmapped-lookbehind-trace/log-000.module-uri.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/110-transform-110-replaceallmapped-lookbehind-trace/log-000.module-uri.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/110-transform-110-replaceallmapped-lookbehind-trace/log-000.member.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/110-transform-110-replaceallmapped-lookbehind-trace/log-000.member.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/110-transform-110-replaceallmapped-lookbehind-trace/log-000.position.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/110-transform-110-replaceallmapped-lookbehind-trace/log-000.position.code-units.bin"
                        ) as &[u8],
                    ),
                ]],
            },
            Row {
                id: "transform-111-replaceallmapped-lookahead-trace",
                expression: "\"ab\".replaceAllMapped(Regex(\"a(?=b)\"), (m) -> trace(m.value))",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/111-transform-111-replaceallmapped-lookahead-trace/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/111-transform-111-replaceallmapped-lookahead-trace/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/111-transform-111-replaceallmapped-lookahead-trace/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/111-transform-111-replaceallmapped-lookahead-trace/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[[
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/111-transform-111-replaceallmapped-lookahead-trace/log-000.level.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/111-transform-111-replaceallmapped-lookahead-trace/log-000.level.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/111-transform-111-replaceallmapped-lookahead-trace/log-000.message.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/111-transform-111-replaceallmapped-lookahead-trace/log-000.message.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/111-transform-111-replaceallmapped-lookahead-trace/log-000.module-uri.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/111-transform-111-replaceallmapped-lookahead-trace/log-000.module-uri.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/111-transform-111-replaceallmapped-lookahead-trace/log-000.member.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/111-transform-111-replaceallmapped-lookahead-trace/log-000.member.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/111-transform-111-replaceallmapped-lookahead-trace/log-000.position.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/111-transform-111-replaceallmapped-lookahead-trace/log-000.position.code-units.bin"
                        ) as &[u8],
                    ),
                ]],
            },
            Row {
                id: "transform-112-replaceallmapped-ordered-trace",
                expression: "\"aba\".replaceAllMapped(Regex(\".\"), (m) -> trace(m.value))",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/112-transform-112-replaceallmapped-ordered-trace/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/112-transform-112-replaceallmapped-ordered-trace/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/112-transform-112-replaceallmapped-ordered-trace/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/112-transform-112-replaceallmapped-ordered-trace/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[
                    [
                        (
                            include_bytes!(
                                "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/112-transform-112-replaceallmapped-ordered-trace/log-000.level.bin"
                            ) as &[u8],
                            include_bytes!(
                                "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/112-transform-112-replaceallmapped-ordered-trace/log-000.level.code-units.bin"
                            ) as &[u8],
                        ),
                        (
                            include_bytes!(
                                "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/112-transform-112-replaceallmapped-ordered-trace/log-000.message.bin"
                            ) as &[u8],
                            include_bytes!(
                                "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/112-transform-112-replaceallmapped-ordered-trace/log-000.message.code-units.bin"
                            ) as &[u8],
                        ),
                        (
                            include_bytes!(
                                "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/112-transform-112-replaceallmapped-ordered-trace/log-000.module-uri.bin"
                            ) as &[u8],
                            include_bytes!(
                                "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/112-transform-112-replaceallmapped-ordered-trace/log-000.module-uri.code-units.bin"
                            ) as &[u8],
                        ),
                        (
                            include_bytes!(
                                "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/112-transform-112-replaceallmapped-ordered-trace/log-000.member.bin"
                            ) as &[u8],
                            include_bytes!(
                                "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/112-transform-112-replaceallmapped-ordered-trace/log-000.member.code-units.bin"
                            ) as &[u8],
                        ),
                        (
                            include_bytes!(
                                "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/112-transform-112-replaceallmapped-ordered-trace/log-000.position.bin"
                            ) as &[u8],
                            include_bytes!(
                                "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/112-transform-112-replaceallmapped-ordered-trace/log-000.position.code-units.bin"
                            ) as &[u8],
                        ),
                    ],
                    [
                        (
                            include_bytes!(
                                "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/112-transform-112-replaceallmapped-ordered-trace/log-001.level.bin"
                            ) as &[u8],
                            include_bytes!(
                                "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/112-transform-112-replaceallmapped-ordered-trace/log-001.level.code-units.bin"
                            ) as &[u8],
                        ),
                        (
                            include_bytes!(
                                "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/112-transform-112-replaceallmapped-ordered-trace/log-001.message.bin"
                            ) as &[u8],
                            include_bytes!(
                                "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/112-transform-112-replaceallmapped-ordered-trace/log-001.message.code-units.bin"
                            ) as &[u8],
                        ),
                        (
                            include_bytes!(
                                "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/112-transform-112-replaceallmapped-ordered-trace/log-001.module-uri.bin"
                            ) as &[u8],
                            include_bytes!(
                                "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/112-transform-112-replaceallmapped-ordered-trace/log-001.module-uri.code-units.bin"
                            ) as &[u8],
                        ),
                        (
                            include_bytes!(
                                "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/112-transform-112-replaceallmapped-ordered-trace/log-001.member.bin"
                            ) as &[u8],
                            include_bytes!(
                                "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/112-transform-112-replaceallmapped-ordered-trace/log-001.member.code-units.bin"
                            ) as &[u8],
                        ),
                        (
                            include_bytes!(
                                "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/112-transform-112-replaceallmapped-ordered-trace/log-001.position.bin"
                            ) as &[u8],
                            include_bytes!(
                                "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/112-transform-112-replaceallmapped-ordered-trace/log-001.position.code-units.bin"
                            ) as &[u8],
                        ),
                    ],
                    [
                        (
                            include_bytes!(
                                "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/112-transform-112-replaceallmapped-ordered-trace/log-002.level.bin"
                            ) as &[u8],
                            include_bytes!(
                                "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/112-transform-112-replaceallmapped-ordered-trace/log-002.level.code-units.bin"
                            ) as &[u8],
                        ),
                        (
                            include_bytes!(
                                "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/112-transform-112-replaceallmapped-ordered-trace/log-002.message.bin"
                            ) as &[u8],
                            include_bytes!(
                                "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/112-transform-112-replaceallmapped-ordered-trace/log-002.message.code-units.bin"
                            ) as &[u8],
                        ),
                        (
                            include_bytes!(
                                "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/112-transform-112-replaceallmapped-ordered-trace/log-002.module-uri.bin"
                            ) as &[u8],
                            include_bytes!(
                                "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/112-transform-112-replaceallmapped-ordered-trace/log-002.module-uri.code-units.bin"
                            ) as &[u8],
                        ),
                        (
                            include_bytes!(
                                "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/112-transform-112-replaceallmapped-ordered-trace/log-002.member.bin"
                            ) as &[u8],
                            include_bytes!(
                                "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/112-transform-112-replaceallmapped-ordered-trace/log-002.member.code-units.bin"
                            ) as &[u8],
                        ),
                        (
                            include_bytes!(
                                "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/112-transform-112-replaceallmapped-ordered-trace/log-002.position.bin"
                            ) as &[u8],
                            include_bytes!(
                                "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/112-transform-112-replaceallmapped-ordered-trace/log-002.position.code-units.bin"
                            ) as &[u8],
                        ),
                    ],
                ],
            },
            Row {
                id: "transform-113-replaceallmapped-later-throw",
                expression: "\"aba\".replaceAllMapped(Regex(\".\"), (m) -> if (m.value == \"b\") throw(\"mapped b\") else trace(m.value))",
                kind: "error",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/113-transform-113-replaceallmapped-later-throw/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/113-transform-113-replaceallmapped-later-throw/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/113-transform-113-replaceallmapped-later-throw/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/113-transform-113-replaceallmapped-later-throw/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[[
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/113-transform-113-replaceallmapped-later-throw/log-000.level.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/113-transform-113-replaceallmapped-later-throw/log-000.level.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/113-transform-113-replaceallmapped-later-throw/log-000.message.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/113-transform-113-replaceallmapped-later-throw/log-000.message.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/113-transform-113-replaceallmapped-later-throw/log-000.module-uri.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/113-transform-113-replaceallmapped-later-throw/log-000.module-uri.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/113-transform-113-replaceallmapped-later-throw/log-000.member.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/113-transform-113-replaceallmapped-later-throw/log-000.member.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/113-transform-113-replaceallmapped-later-throw/log-000.position.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/113-transform-113-replaceallmapped-later-throw/log-000.position.code-units.bin"
                        ) as &[u8],
                    ),
                ]],
            },
            Row {
                id: "transform-114-replaceallmapped-first-throw",
                expression: "\"aba\".replaceAllMapped(Regex(\".\"), (m) -> throw(\"mapped first\"))",
                kind: "error",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/114-transform-114-replaceallmapped-first-throw/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/114-transform-114-replaceallmapped-first-throw/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/114-transform-114-replaceallmapped-first-throw/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/114-transform-114-replaceallmapped-first-throw/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-115-replacefirstmapped-literal",
                expression: "\"aba\".replaceFirstMapped(\"a\", (m) -> m.value)",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/115-transform-115-replacefirstmapped-literal/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/115-transform-115-replacefirstmapped-literal/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/115-transform-115-replacefirstmapped-literal/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/115-transform-115-replacefirstmapped-literal/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-116-replacefirstmapped-group",
                expression: "\"aba\".replaceFirstMapped(Regex(\"(a)\"), (m) -> m.groups[1].value)",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/116-transform-116-replacefirstmapped-group/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/116-transform-116-replacefirstmapped-group/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/116-transform-116-replacefirstmapped-group/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/116-transform-116-replacefirstmapped-group/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-117-replacefirstmapped-dollar",
                expression: "\"aba\".replaceFirstMapped(Regex(\"a\"), (m) -> \"$\")",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/117-transform-117-replacefirstmapped-dollar/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/117-transform-117-replacefirstmapped-dollar/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/117-transform-117-replacefirstmapped-dollar/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/117-transform-117-replacefirstmapped-dollar/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-118-replacefirstmapped-slash",
                expression: "\"aba\".replaceFirstMapped(Regex(\"a\"), (m) -> \"\\\\\")",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/118-transform-118-replacefirstmapped-slash/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/118-transform-118-replacefirstmapped-slash/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/118-transform-118-replacefirstmapped-slash/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/118-transform-118-replacefirstmapped-slash/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-119-replacefirstmapped-empty-pair",
                expression: "\"😀x\".replaceFirstMapped(Regex(\"\"), (m) -> \"-\")",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/119-transform-119-replacefirstmapped-empty-pair/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/119-transform-119-replacefirstmapped-empty-pair/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/119-transform-119-replacefirstmapped-empty-pair/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/119-transform-119-replacefirstmapped-empty-pair/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-120-replacefirstmapped-arity-no-match",
                expression: "\"a\".replaceFirstMapped(Regex(\"z\"), (a, b) -> \"x\")",
                kind: "error",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/120-transform-120-replacefirstmapped-arity-no-match/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/120-transform-120-replacefirstmapped-arity-no-match/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/120-transform-120-replacefirstmapped-arity-no-match/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/120-transform-120-replacefirstmapped-arity-no-match/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-121-replacefirstmapped-arity-match",
                expression: "\"a\".replaceFirstMapped(Regex(\"a\"), (a, b) -> \"x\")",
                kind: "error",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/121-transform-121-replacefirstmapped-arity-match/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/121-transform-121-replacefirstmapped-arity-match/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/121-transform-121-replacefirstmapped-arity-match/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/121-transform-121-replacefirstmapped-arity-match/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-122-replacefirstmapped-return-no-match",
                expression: "\"a\".replaceFirstMapped(Regex(\"z\"), (m) -> 42)",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/122-transform-122-replacefirstmapped-return-no-match/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/122-transform-122-replacefirstmapped-return-no-match/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/122-transform-122-replacefirstmapped-return-no-match/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/122-transform-122-replacefirstmapped-return-no-match/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-123-replacefirstmapped-return-match",
                expression: "\"a\".replaceFirstMapped(Regex(\"a\"), (m) -> 42)",
                kind: "error",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/123-transform-123-replacefirstmapped-return-match/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/123-transform-123-replacefirstmapped-return-match/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/123-transform-123-replacefirstmapped-return-match/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/123-transform-123-replacefirstmapped-return-match/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-124-replacefirstmapped-null-no-match",
                expression: "\"a\".replaceFirstMapped(Regex(\"z\"), null)",
                kind: "error",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/124-transform-124-replacefirstmapped-null-no-match/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/124-transform-124-replacefirstmapped-null-no-match/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/124-transform-124-replacefirstmapped-null-no-match/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/124-transform-124-replacefirstmapped-null-no-match/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-125-replacefirstmapped-lookbehind-trace",
                expression: "\"ab\".replaceFirstMapped(Regex(\"(?<=a)b\"), (m) -> trace(m.value))",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/125-transform-125-replacefirstmapped-lookbehind-trace/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/125-transform-125-replacefirstmapped-lookbehind-trace/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/125-transform-125-replacefirstmapped-lookbehind-trace/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/125-transform-125-replacefirstmapped-lookbehind-trace/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[[
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/125-transform-125-replacefirstmapped-lookbehind-trace/log-000.level.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/125-transform-125-replacefirstmapped-lookbehind-trace/log-000.level.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/125-transform-125-replacefirstmapped-lookbehind-trace/log-000.message.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/125-transform-125-replacefirstmapped-lookbehind-trace/log-000.message.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/125-transform-125-replacefirstmapped-lookbehind-trace/log-000.module-uri.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/125-transform-125-replacefirstmapped-lookbehind-trace/log-000.module-uri.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/125-transform-125-replacefirstmapped-lookbehind-trace/log-000.member.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/125-transform-125-replacefirstmapped-lookbehind-trace/log-000.member.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/125-transform-125-replacefirstmapped-lookbehind-trace/log-000.position.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/125-transform-125-replacefirstmapped-lookbehind-trace/log-000.position.code-units.bin"
                        ) as &[u8],
                    ),
                ]],
            },
            Row {
                id: "transform-126-replacefirstmapped-lookahead-trace",
                expression: "\"ab\".replaceFirstMapped(Regex(\"a(?=b)\"), (m) -> trace(m.value))",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/126-transform-126-replacefirstmapped-lookahead-trace/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/126-transform-126-replacefirstmapped-lookahead-trace/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/126-transform-126-replacefirstmapped-lookahead-trace/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/126-transform-126-replacefirstmapped-lookahead-trace/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[[
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/126-transform-126-replacefirstmapped-lookahead-trace/log-000.level.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/126-transform-126-replacefirstmapped-lookahead-trace/log-000.level.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/126-transform-126-replacefirstmapped-lookahead-trace/log-000.message.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/126-transform-126-replacefirstmapped-lookahead-trace/log-000.message.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/126-transform-126-replacefirstmapped-lookahead-trace/log-000.module-uri.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/126-transform-126-replacefirstmapped-lookahead-trace/log-000.module-uri.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/126-transform-126-replacefirstmapped-lookahead-trace/log-000.member.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/126-transform-126-replacefirstmapped-lookahead-trace/log-000.member.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/126-transform-126-replacefirstmapped-lookahead-trace/log-000.position.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/126-transform-126-replacefirstmapped-lookahead-trace/log-000.position.code-units.bin"
                        ) as &[u8],
                    ),
                ]],
            },
            Row {
                id: "transform-127-replacefirstmapped-ordered-trace",
                expression: "\"aba\".replaceFirstMapped(Regex(\".\"), (m) -> trace(m.value))",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/127-transform-127-replacefirstmapped-ordered-trace/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/127-transform-127-replacefirstmapped-ordered-trace/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/127-transform-127-replacefirstmapped-ordered-trace/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/127-transform-127-replacefirstmapped-ordered-trace/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[[
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/127-transform-127-replacefirstmapped-ordered-trace/log-000.level.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/127-transform-127-replacefirstmapped-ordered-trace/log-000.level.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/127-transform-127-replacefirstmapped-ordered-trace/log-000.message.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/127-transform-127-replacefirstmapped-ordered-trace/log-000.message.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/127-transform-127-replacefirstmapped-ordered-trace/log-000.module-uri.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/127-transform-127-replacefirstmapped-ordered-trace/log-000.module-uri.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/127-transform-127-replacefirstmapped-ordered-trace/log-000.member.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/127-transform-127-replacefirstmapped-ordered-trace/log-000.member.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/127-transform-127-replacefirstmapped-ordered-trace/log-000.position.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/127-transform-127-replacefirstmapped-ordered-trace/log-000.position.code-units.bin"
                        ) as &[u8],
                    ),
                ]],
            },
            Row {
                id: "transform-128-replacefirstmapped-later-throw",
                expression: "\"aba\".replaceFirstMapped(Regex(\".\"), (m) -> if (m.value == \"b\") throw(\"mapped b\") else trace(m.value))",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/128-transform-128-replacefirstmapped-later-throw/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/128-transform-128-replacefirstmapped-later-throw/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/128-transform-128-replacefirstmapped-later-throw/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/128-transform-128-replacefirstmapped-later-throw/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[[
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/128-transform-128-replacefirstmapped-later-throw/log-000.level.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/128-transform-128-replacefirstmapped-later-throw/log-000.level.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/128-transform-128-replacefirstmapped-later-throw/log-000.message.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/128-transform-128-replacefirstmapped-later-throw/log-000.message.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/128-transform-128-replacefirstmapped-later-throw/log-000.module-uri.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/128-transform-128-replacefirstmapped-later-throw/log-000.module-uri.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/128-transform-128-replacefirstmapped-later-throw/log-000.member.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/128-transform-128-replacefirstmapped-later-throw/log-000.member.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/128-transform-128-replacefirstmapped-later-throw/log-000.position.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/128-transform-128-replacefirstmapped-later-throw/log-000.position.code-units.bin"
                        ) as &[u8],
                    ),
                ]],
            },
            Row {
                id: "transform-129-replacefirstmapped-first-throw",
                expression: "\"aba\".replaceFirstMapped(Regex(\".\"), (m) -> throw(\"mapped first\"))",
                kind: "error",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/129-transform-129-replacefirstmapped-first-throw/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/129-transform-129-replacefirstmapped-first-throw/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/129-transform-129-replacefirstmapped-first-throw/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/129-transform-129-replacefirstmapped-first-throw/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-130-replacelastmapped-literal",
                expression: "\"aba\".replaceLastMapped(\"a\", (m) -> m.value)",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/130-transform-130-replacelastmapped-literal/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/130-transform-130-replacelastmapped-literal/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/130-transform-130-replacelastmapped-literal/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/130-transform-130-replacelastmapped-literal/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-131-replacelastmapped-group",
                expression: "\"aba\".replaceLastMapped(Regex(\"(a)\"), (m) -> m.groups[1].value)",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/131-transform-131-replacelastmapped-group/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/131-transform-131-replacelastmapped-group/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/131-transform-131-replacelastmapped-group/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/131-transform-131-replacelastmapped-group/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-132-replacelastmapped-dollar",
                expression: "\"aba\".replaceLastMapped(Regex(\"a\"), (m) -> \"$\")",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/132-transform-132-replacelastmapped-dollar/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/132-transform-132-replacelastmapped-dollar/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/132-transform-132-replacelastmapped-dollar/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/132-transform-132-replacelastmapped-dollar/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-133-replacelastmapped-slash",
                expression: "\"aba\".replaceLastMapped(Regex(\"a\"), (m) -> \"\\\\\")",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/133-transform-133-replacelastmapped-slash/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/133-transform-133-replacelastmapped-slash/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/133-transform-133-replacelastmapped-slash/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/133-transform-133-replacelastmapped-slash/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-134-replacelastmapped-empty-pair",
                expression: "\"😀x\".replaceLastMapped(Regex(\"\"), (m) -> \"-\")",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/134-transform-134-replacelastmapped-empty-pair/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/134-transform-134-replacelastmapped-empty-pair/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/134-transform-134-replacelastmapped-empty-pair/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/134-transform-134-replacelastmapped-empty-pair/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-135-replacelastmapped-arity-no-match",
                expression: "\"a\".replaceLastMapped(Regex(\"z\"), (a, b) -> \"x\")",
                kind: "error",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/135-transform-135-replacelastmapped-arity-no-match/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/135-transform-135-replacelastmapped-arity-no-match/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/135-transform-135-replacelastmapped-arity-no-match/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/135-transform-135-replacelastmapped-arity-no-match/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-136-replacelastmapped-arity-match",
                expression: "\"a\".replaceLastMapped(Regex(\"a\"), (a, b) -> \"x\")",
                kind: "error",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/136-transform-136-replacelastmapped-arity-match/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/136-transform-136-replacelastmapped-arity-match/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/136-transform-136-replacelastmapped-arity-match/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/136-transform-136-replacelastmapped-arity-match/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-137-replacelastmapped-return-no-match",
                expression: "\"a\".replaceLastMapped(Regex(\"z\"), (m) -> 42)",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/137-transform-137-replacelastmapped-return-no-match/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/137-transform-137-replacelastmapped-return-no-match/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/137-transform-137-replacelastmapped-return-no-match/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/137-transform-137-replacelastmapped-return-no-match/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-138-replacelastmapped-return-match",
                expression: "\"a\".replaceLastMapped(Regex(\"a\"), (m) -> 42)",
                kind: "error",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/138-transform-138-replacelastmapped-return-match/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/138-transform-138-replacelastmapped-return-match/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/138-transform-138-replacelastmapped-return-match/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/138-transform-138-replacelastmapped-return-match/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-139-replacelastmapped-null-no-match",
                expression: "\"a\".replaceLastMapped(Regex(\"z\"), null)",
                kind: "error",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/139-transform-139-replacelastmapped-null-no-match/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/139-transform-139-replacelastmapped-null-no-match/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/139-transform-139-replacelastmapped-null-no-match/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/139-transform-139-replacelastmapped-null-no-match/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-140-replacelastmapped-lookbehind-trace",
                expression: "\"ab\".replaceLastMapped(Regex(\"(?<=a)b\"), (m) -> trace(m.value))",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/140-transform-140-replacelastmapped-lookbehind-trace/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/140-transform-140-replacelastmapped-lookbehind-trace/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/140-transform-140-replacelastmapped-lookbehind-trace/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/140-transform-140-replacelastmapped-lookbehind-trace/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-141-replacelastmapped-lookahead-trace",
                expression: "\"ab\".replaceLastMapped(Regex(\"a(?=b)\"), (m) -> trace(m.value))",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/141-transform-141-replacelastmapped-lookahead-trace/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/141-transform-141-replacelastmapped-lookahead-trace/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/141-transform-141-replacelastmapped-lookahead-trace/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/141-transform-141-replacelastmapped-lookahead-trace/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-142-replacelastmapped-ordered-trace",
                expression: "\"aba\".replaceLastMapped(Regex(\".\"), (m) -> trace(m.value))",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/142-transform-142-replacelastmapped-ordered-trace/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/142-transform-142-replacelastmapped-ordered-trace/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/142-transform-142-replacelastmapped-ordered-trace/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/142-transform-142-replacelastmapped-ordered-trace/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[[
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/142-transform-142-replacelastmapped-ordered-trace/log-000.level.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/142-transform-142-replacelastmapped-ordered-trace/log-000.level.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/142-transform-142-replacelastmapped-ordered-trace/log-000.message.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/142-transform-142-replacelastmapped-ordered-trace/log-000.message.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/142-transform-142-replacelastmapped-ordered-trace/log-000.module-uri.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/142-transform-142-replacelastmapped-ordered-trace/log-000.module-uri.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/142-transform-142-replacelastmapped-ordered-trace/log-000.member.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/142-transform-142-replacelastmapped-ordered-trace/log-000.member.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/142-transform-142-replacelastmapped-ordered-trace/log-000.position.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/142-transform-142-replacelastmapped-ordered-trace/log-000.position.code-units.bin"
                        ) as &[u8],
                    ),
                ]],
            },
            Row {
                id: "transform-143-replacelastmapped-later-throw",
                expression: "\"aba\".replaceLastMapped(Regex(\".\"), (m) -> if (m.value == \"b\") throw(\"mapped b\") else trace(m.value))",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/143-transform-143-replacelastmapped-later-throw/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/143-transform-143-replacelastmapped-later-throw/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/143-transform-143-replacelastmapped-later-throw/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/143-transform-143-replacelastmapped-later-throw/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[[
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/143-transform-143-replacelastmapped-later-throw/log-000.level.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/143-transform-143-replacelastmapped-later-throw/log-000.level.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/143-transform-143-replacelastmapped-later-throw/log-000.message.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/143-transform-143-replacelastmapped-later-throw/log-000.message.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/143-transform-143-replacelastmapped-later-throw/log-000.module-uri.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/143-transform-143-replacelastmapped-later-throw/log-000.module-uri.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/143-transform-143-replacelastmapped-later-throw/log-000.member.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/143-transform-143-replacelastmapped-later-throw/log-000.member.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/143-transform-143-replacelastmapped-later-throw/log-000.position.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/143-transform-143-replacelastmapped-later-throw/log-000.position.code-units.bin"
                        ) as &[u8],
                    ),
                ]],
            },
            Row {
                id: "transform-144-replacelastmapped-first-throw",
                expression: "\"aba\".replaceLastMapped(Regex(\".\"), (m) -> throw(\"mapped first\"))",
                kind: "error",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/144-transform-144-replacelastmapped-first-throw/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/144-transform-144-replacelastmapped-first-throw/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/144-transform-144-replacelastmapped-first-throw/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/144-transform-144-replacelastmapped-first-throw/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-145-mapped-eager-argument-order",
                expression: "\"a\".replaceAllMapped(null, throw(\"mapper evaluated\"))",
                kind: "error",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/145-transform-145-mapped-eager-argument-order/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/145-transform-145-mapped-eager-argument-order/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/145-transform-145-mapped-eager-argument-order/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/145-transform-145-mapped-eager-argument-order/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-146-implicit-split-constraint",
                expression: "\"a,b\" as String(split(\",\").length == 2)",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/146-transform-146-implicit-split-constraint/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/146-transform-146-implicit-split-constraint/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/146-transform-146-implicit-split-constraint/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/146-transform-146-implicit-split-constraint/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "transform-147-implicit-replacement-constraint",
                expression: "\"aba\" as String(replaceLast(Regex(\"a\"), \"x\") == \"abx\")",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/147-transform-147-implicit-replacement-constraint/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/147-transform-147-implicit-replacement-constraint/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/147-transform-147-implicit-replacement-constraint/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-transforms/string-regex-transforms/147-transform-147-implicit-replacement-constraint/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
        ],
    );
}
