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
fn java_string_regex_predicates_all_raw_results_and_logs() {
    run(
        include_str!(
            "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/module.pkl"
        ),
        "file:///stage76/string-regex-predicates.pkl",
        &[
            Row {
                id: "predicate-000-contains-literal",
                expression: "\"ababa\".contains(\"bab\")",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/000-predicate-000-contains-literal/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/000-predicate-000-contains-literal/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/000-predicate-000-contains-literal/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/000-predicate-000-contains-literal/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "predicate-001-contains-regex",
                expression: "\"ababa\".contains(Regex(\"b.b\"))",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/001-predicate-001-contains-regex/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/001-predicate-001-contains-regex/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/001-predicate-001-contains-regex/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/001-predicate-001-contains-regex/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "predicate-002-contains-no-match",
                expression: "\"ababa\".contains(Regex(\"z\"))",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/002-predicate-002-contains-no-match/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/002-predicate-002-contains-no-match/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/002-predicate-002-contains-no-match/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/002-predicate-002-contains-no-match/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "predicate-003-contains-empty",
                expression: "\"\".contains(Regex(\"\"))",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/003-predicate-003-contains-empty/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/003-predicate-003-contains-empty/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/003-predicate-003-contains-empty/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/003-predicate-003-contains-empty/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "predicate-004-contains-unicode-ucc",
                expression: "\"é\".contains(Regex(\"\\\\w\"))",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/004-predicate-004-contains-unicode-ucc/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/004-predicate-004-contains-unicode-ucc/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/004-predicate-004-contains-unicode-ucc/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/004-predicate-004-contains-unicode-ucc/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "predicate-005-contains-supplementary",
                expression: "\"a😀b\".contains(Regex(\"😀\"))",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/005-predicate-005-contains-supplementary/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/005-predicate-005-contains-supplementary/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/005-predicate-005-contains-supplementary/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/005-predicate-005-contains-supplementary/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "predicate-006-matches-full-alternation",
                expression: "\"ab\".matches(Regex(\"a|ab\"))",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/006-predicate-006-matches-full-alternation/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/006-predicate-006-matches-full-alternation/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/006-predicate-006-matches-full-alternation/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/006-predicate-006-matches-full-alternation/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "predicate-007-matches-partial-false",
                expression: "\"ab\".matches(Regex(\"a\"))",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/007-predicate-007-matches-partial-false/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/007-predicate-007-matches-partial-false/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/007-predicate-007-matches-partial-false/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/007-predicate-007-matches-partial-false/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "predicate-008-matches-empty",
                expression: "\"\".matches(Regex(\"a*\"))",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/008-predicate-008-matches-empty/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/008-predicate-008-matches-empty/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/008-predicate-008-matches-empty/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/008-predicate-008-matches-empty/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "predicate-009-starts-literal",
                expression: "\"ab\".startsWith(\"a\")",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/009-predicate-009-starts-literal/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/009-predicate-009-starts-literal/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/009-predicate-009-starts-literal/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/009-predicate-009-starts-literal/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "predicate-010-starts-regex",
                expression: "\"ab\".startsWith(Regex(\"a|ab\"))",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/010-predicate-010-starts-regex/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/010-predicate-010-starts-regex/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/010-predicate-010-starts-regex/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/010-predicate-010-starts-regex/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "predicate-011-starts-reject-search",
                expression: "\"ba\".startsWith(Regex(\"a\"))",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/011-predicate-011-starts-reject-search/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/011-predicate-011-starts-reject-search/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/011-predicate-011-starts-reject-search/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/011-predicate-011-starts-reject-search/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "predicate-012-starts-empty",
                expression: "\"abc\".startsWith(Regex(\"\"))",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/012-predicate-012-starts-empty/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/012-predicate-012-starts-empty/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/012-predicate-012-starts-empty/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/012-predicate-012-starts-empty/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "predicate-013-ends-literal",
                expression: "\"ababa\".endsWith(\"aba\")",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/013-predicate-013-ends-literal/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/013-predicate-013-ends-literal/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/013-predicate-013-ends-literal/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/013-predicate-013-ends-literal/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "predicate-014-ends-overlap",
                expression: "\"ababa\".endsWith(Regex(\"aba\"))",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/014-predicate-014-ends-overlap/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/014-predicate-014-ends-overlap/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/014-predicate-014-ends-overlap/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/014-predicate-014-ends-overlap/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "predicate-015-ends-repeated-find",
                expression: "\"abaaba\".endsWith(Regex(\"aba\"))",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/015-predicate-015-ends-repeated-find/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/015-predicate-015-ends-repeated-find/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/015-predicate-015-ends-repeated-find/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/015-predicate-015-ends-repeated-find/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "predicate-016-ends-alternation",
                expression: "\"ab\".endsWith(Regex(\"a|ab\"))",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/016-predicate-016-ends-alternation/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/016-predicate-016-ends-alternation/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/016-predicate-016-ends-alternation/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/016-predicate-016-ends-alternation/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "predicate-017-ends-no-match",
                expression: "\"\".endsWith(Regex(\"a\"))",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/017-predicate-017-ends-no-match/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/017-predicate-017-ends-no-match/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/017-predicate-017-ends-no-match/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/017-predicate-017-ends-no-match/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "predicate-018-ends-empty",
                expression: "\"\".endsWith(Regex(\"\"))",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/018-predicate-018-ends-empty/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/018-predicate-018-ends-empty/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/018-predicate-018-ends-empty/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/018-predicate-018-ends-empty/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "predicate-019-literal-lone-high",
                expression: "\"\\u{d800}\".contains(\"\\u{d800}\")",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/019-predicate-019-literal-lone-high/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/019-predicate-019-literal-lone-high/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/019-predicate-019-literal-lone-high/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/019-predicate-019-literal-lone-high/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "predicate-020-literal-lone-low-prefix",
                expression: "\"\\u{dc00}x\".startsWith(\"\\u{dc00}\")",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/020-predicate-020-literal-lone-low-prefix/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/020-predicate-020-literal-lone-low-prefix/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/020-predicate-020-literal-lone-low-prefix/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/020-predicate-020-literal-lone-low-prefix/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "predicate-021-literal-lone-high-suffix",
                expression: "\"x\\u{d800}\".endsWith(\"\\u{d800}\")",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/021-predicate-021-literal-lone-high-suffix/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/021-predicate-021-literal-lone-high-suffix/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/021-predicate-021-literal-lone-high-suffix/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/021-predicate-021-literal-lone-high-suffix/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "predicate-022-regex-lone-input",
                expression: "\"\\u{d800}\".contains(Regex(\".\"))",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/022-predicate-022-regex-lone-input/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/022-predicate-022-regex-lone-input/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/022-predicate-022-regex-lone-input/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/022-predicate-022-regex-lone-input/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "predicate-023-regex-lone-pattern",
                expression: "\"x\".contains(Regex(\"\\u{d800}\"))",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/023-predicate-023-regex-lone-pattern/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/023-predicate-023-regex-lone-pattern/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/023-predicate-023-regex-lone-pattern/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/023-predicate-023-regex-lone-pattern/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "predicate-024-contains-wrong-type",
                expression: "\"a\".contains(1)",
                kind: "error",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/024-predicate-024-contains-wrong-type/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/024-predicate-024-contains-wrong-type/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/024-predicate-024-contains-wrong-type/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/024-predicate-024-contains-wrong-type/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "predicate-025-matches-wrong-type",
                expression: "\"a\".matches(\"a\")",
                kind: "error",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/025-predicate-025-matches-wrong-type/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/025-predicate-025-matches-wrong-type/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/025-predicate-025-matches-wrong-type/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/025-predicate-025-matches-wrong-type/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "predicate-026-starts-wrong-type",
                expression: "\"a\".startsWith(null)",
                kind: "error",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/026-predicate-026-starts-wrong-type/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/026-predicate-026-starts-wrong-type/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/026-predicate-026-starts-wrong-type/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/026-predicate-026-starts-wrong-type/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "predicate-027-ends-wrong-type",
                expression: "\"a\".endsWith(1)",
                kind: "error",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/027-predicate-027-ends-wrong-type/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/027-predicate-027-ends-wrong-type/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/027-predicate-027-ends-wrong-type/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/027-predicate-027-ends-wrong-type/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "predicate-028-wrong-arity",
                expression: "\"a\".matches()",
                kind: "error",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/028-predicate-028-wrong-arity/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/028-predicate-028-wrong-arity/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/028-predicate-028-wrong-arity/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/028-predicate-028-wrong-arity/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "predicate-029-argument-first",
                expression: "\"a\".matches(throw(\"argument first\"))",
                kind: "error",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/029-predicate-029-argument-first/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/029-predicate-029-argument-first/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/029-predicate-029-argument-first/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/029-predicate-029-argument-first/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "predicate-030-too-many-argument-first",
                expression: "\"a\".matches(Regex(\"a\"), throw(\"argument first\"))",
                kind: "error",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/030-predicate-030-too-many-argument-first/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/030-predicate-030-too-many-argument-first/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/030-predicate-030-too-many-argument-first/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/030-predicate-030-too-many-argument-first/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "predicate-031-method-as-property",
                expression: "\"a\".contains",
                kind: "error",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/031-predicate-031-method-as-property/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/031-predicate-031-method-as-property/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/031-predicate-031-method-as-property/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/031-predicate-031-method-as-property/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "predicate-032-receiver-first",
                expression: "(throw(\"receiver first\")).matches(throw(\"argument second\"))",
                kind: "error",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/032-predicate-032-receiver-first/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/032-predicate-032-receiver-first/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/032-predicate-032-receiver-first/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/032-predicate-032-receiver-first/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "predicate-033-constraint-contains",
                expression: "\"a\" as String(contains(Regex(\"a\")))",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/033-predicate-033-constraint-contains/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/033-predicate-033-constraint-contains/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/033-predicate-033-constraint-contains/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/033-predicate-033-constraint-contains/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "predicate-034-constraint-matches",
                expression: "\"ab\" as String(matches(Regex(\"a|ab\")))",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/034-predicate-034-constraint-matches/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/034-predicate-034-constraint-matches/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/034-predicate-034-constraint-matches/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/034-predicate-034-constraint-matches/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "predicate-035-constraint-prefix-suffix",
                expression: "\"ab\" as String(startsWith(Regex(\"a\")) && endsWith(Regex(\"b\")))",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/035-predicate-035-constraint-prefix-suffix/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/035-predicate-035-constraint-prefix-suffix/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/035-predicate-035-constraint-prefix-suffix/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/035-predicate-035-constraint-prefix-suffix/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "predicate-036-constraint-negative",
                expression: "\"ab\" as String(matches(Regex(\"a\")))",
                kind: "error",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/036-predicate-036-constraint-negative/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/036-predicate-036-constraint-negative/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/036-predicate-036-constraint-negative/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-regex-predicates/string-regex-predicates/036-predicate-036-constraint-negative/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
        ],
    );
}
