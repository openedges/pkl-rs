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
fn java_main_full_raw_result_and_logger() {
    run(
        include_str!("../../../tests/fixtures/m2-string-is-regex/main/module.pkl"),
        "file:///stage76/string-is-regex.pkl",
        &[
            Row {
                id: "empty",
                expression: "\"\".isRegex",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/000-empty/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/000-empty/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/000-empty/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/000-empty/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "ascii",
                expression: "\"abc\".isRegex",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/001-ascii/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/001-ascii/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/001-ascii/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/001-ascii/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "valid-class",
                expression: "#\"\\w*[0-9]\"#.isRegex",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/002-valid-class/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/002-valid-class/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/002-valid-class/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/002-valid-class/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "unclosed-group",
                expression: "\"(abc\".isRegex",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/003-unclosed-group/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/003-unclosed-group/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/003-unclosed-group/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/003-unclosed-group/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "dangling-star",
                expression: "\"*\".isRegex",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/004-dangling-star/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/004-dangling-star/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/004-dangling-star/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/004-dangling-star/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "unicode-family",
                expression: "#\"\\p{L}\"#.isRegex",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/005-unicode-family/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/005-unicode-family/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/005-unicode-family/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/005-unicode-family/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "unclosed-family",
                expression: "#\"\\p{L\"#.isRegex",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/006-unclosed-family/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/006-unclosed-family/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/006-unclosed-family/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/006-unclosed-family/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "empty-family",
                expression: "#\"\\p{}\"#.isRegex",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/007-empty-family/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/007-empty-family/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/007-empty-family/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/007-empty-family/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "unknown-family",
                expression: "#\"\\p{NotAPklProperty}\"#.isRegex",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/008-unknown-family/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/008-unknown-family/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/008-unknown-family/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/008-unknown-family/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "unknown-category",
                expression: "#\"\\p{gc=NotACategory}\"#.isRegex",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/009-unknown-category/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/009-unknown-category/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/009-unknown-category/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/009-unknown-category/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "unknown-script",
                expression: "#\"\\p{sc=NotAScript}\"#.isRegex",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/010-unknown-script/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/010-unknown-script/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/010-unknown-script/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/010-unknown-script/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "alphabetic-property",
                expression: "#\"\\p{IsAlphabetic}\"#.isRegex",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/011-alphabetic-property/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/011-alphabetic-property/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/011-alphabetic-property/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/011-alphabetic-property/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "grapheme-cluster-syntax",
                expression: "#\"\\X\"#.isRegex",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/012-grapheme-cluster-syntax/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/012-grapheme-cluster-syntax/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/012-grapheme-cluster-syntax/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/012-grapheme-cluster-syntax/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "grapheme-boundary-syntax",
                expression: "#\"\\b{g}\"#.isRegex",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/013-grapheme-boundary-syntax/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/013-grapheme-boundary-syntax/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/013-grapheme-boundary-syntax/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/013-grapheme-boundary-syntax/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "bad-grapheme-boundary",
                expression: "#\"\\b{bad}\"#.isRegex",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/014-bad-grapheme-boundary/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/014-bad-grapheme-boundary/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/014-bad-grapheme-boundary/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/014-bad-grapheme-boundary/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "quoted-property-text",
                expression: "#\"\\Q\\p{Missing}\\E\"#.isRegex",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/015-quoted-property-text/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/015-quoted-property-text/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/015-quoted-property-text/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/015-quoted-property-text/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "reserved-escape",
                expression: "#\"\\q\"#.isRegex",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/016-reserved-escape/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/016-reserved-escape/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/016-reserved-escape/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/016-reserved-escape/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "lone-high",
                expression: "\"\\u{D800}\".isRegex",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/017-lone-high/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/017-lone-high/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/017-lone-high/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/017-lone-high/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "lone-low",
                expression: "\"\\u{DC00}\".isRegex",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/018-lone-low/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/018-lone-low/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/018-lone-low/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/018-lone-low/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "surrogate-pair",
                expression: "\"😀\".isRegex",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/019-surrogate-pair/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/019-surrogate-pair/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/019-surrogate-pair/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/019-surrogate-pair/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "comment-syntax",
                expression: "#\"(?x)a # \\q\"#.isRegex",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/020-comment-syntax/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/020-comment-syntax/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/020-comment-syntax/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/020-comment-syntax/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "implicit-valid",
                expression: "\"abc\" as String(isRegex)",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/021-implicit-valid/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/021-implicit-valid/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/021-implicit-valid/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/021-implicit-valid/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "implicit-invalid",
                expression: "\"(\" as String(isRegex)",
                kind: "error",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/022-implicit-invalid/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/022-implicit-invalid/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/022-implicit-invalid/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/022-implicit-invalid/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "this-valid",
                expression: "\"abc\" as String(this.isRegex)",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/023-this-valid/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/023-this-valid/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/023-this-valid/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/023-this-valid/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "property-as-method",
                expression: "\"a\".isRegex(throw(\"argument\"))",
                kind: "error",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/024-property-as-method/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/024-property-as-method/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/024-property-as-method/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/024-property-as-method/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "receiver-first",
                expression: "(throw(\"receiver\") as String).isRegex",
                kind: "error",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/025-receiver-first/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/025-receiver-first/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/025-receiver-first/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/025-receiver-first/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "reference-type",
                expression: "refIsRegexType",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/026-reference-type/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/026-reference-type/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/026-reference-type/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/026-reference-type/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "reference-path",
                expression: "refString.isRegex.getPath()[0].property",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/027-reference-path/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/027-reference-path/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/027-reference-path/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/027-reference-path/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "reference-as-method",
                expression: "refString.isRegex(throw(\"argument\"))",
                kind: "error",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/028-reference-as-method/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/028-reference-as-method/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/028-reference-as-method/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/028-reference-as-method/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "constructor-unclosed-family",
                expression: "Regex(#\"\\p{L\"#)",
                kind: "error",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/029-constructor-unclosed-family/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/029-constructor-unclosed-family/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/029-constructor-unclosed-family/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/029-constructor-unclosed-family/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "constructor-unknown-family",
                expression: "Regex(#\"\\p{NotAPklProperty}\"#)",
                kind: "error",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/030-constructor-unknown-family/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/030-constructor-unknown-family/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/030-constructor-unknown-family/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/030-constructor-unknown-family/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "constructor-unknown-category",
                expression: "Regex(#\"\\p{gc=NotACategory}\"#)",
                kind: "error",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/031-constructor-unknown-category/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/031-constructor-unknown-category/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/031-constructor-unknown-category/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/main/repeat-0/031-constructor-unknown-category/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
        ],
    );
}

#[test]
fn java_supplement_full_raw_result_and_logger() {
    run(
        include_str!("../../../tests/fixtures/m2-string-is-regex/supplement/module.pkl"),
        "file:///stage76/string-is-regex-supplement.pkl",
        &[
            Row {
                id: "lowercase-posix-property",
                expression: "#\"\\p{alpha}\"#.isRegex",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/supplement/repeat-0/000-lowercase-posix-property/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/supplement/repeat-0/000-lowercase-posix-property/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/supplement/repeat-0/000-lowercase-posix-property/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/supplement/repeat-0/000-lowercase-posix-property/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "lowercase-posix-constructor",
                expression: "Regex(#\"\\p{alpha}\"#)",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/supplement/repeat-0/001-lowercase-posix-constructor/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/supplement/repeat-0/001-lowercase-posix-constructor/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/supplement/repeat-0/001-lowercase-posix-constructor/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/supplement/repeat-0/001-lowercase-posix-constructor/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "unclosed-unknown-family-property",
                expression: "#\"\\p{Lx\"#.isRegex",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/supplement/repeat-0/002-unclosed-unknown-family-property/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/supplement/repeat-0/002-unclosed-unknown-family-property/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/supplement/repeat-0/002-unclosed-unknown-family-property/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/supplement/repeat-0/002-unclosed-unknown-family-property/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "unclosed-unknown-family-constructor",
                expression: "Regex(#\"\\p{Lx\"#)",
                kind: "error",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/supplement/repeat-0/003-unclosed-unknown-family-constructor/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/supplement/repeat-0/003-unclosed-unknown-family-constructor/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/supplement/repeat-0/003-unclosed-unknown-family-constructor/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/supplement/repeat-0/003-unclosed-unknown-family-constructor/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "escaped-unknown-family-property",
                expression: "#\"\\p{\\q}\"#.isRegex",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/supplement/repeat-0/004-escaped-unknown-family-property/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/supplement/repeat-0/004-escaped-unknown-family-property/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/supplement/repeat-0/004-escaped-unknown-family-property/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/supplement/repeat-0/004-escaped-unknown-family-property/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "escaped-unknown-family-constructor",
                expression: "Regex(#\"\\p{\\q}\"#)",
                kind: "error",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/supplement/repeat-0/005-escaped-unknown-family-constructor/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/supplement/repeat-0/005-escaped-unknown-family-constructor/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/supplement/repeat-0/005-escaped-unknown-family-constructor/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/supplement/repeat-0/005-escaped-unknown-family-constructor/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "unclosed-escaped-family-property",
                expression: "#\"\\p{L\\q\"#.isRegex",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/supplement/repeat-0/006-unclosed-escaped-family-property/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/supplement/repeat-0/006-unclosed-escaped-family-property/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/supplement/repeat-0/006-unclosed-escaped-family-property/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/supplement/repeat-0/006-unclosed-escaped-family-property/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "unclosed-escaped-family-constructor",
                expression: "Regex(#\"\\p{L\\q\"#)",
                kind: "error",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/supplement/repeat-0/007-unclosed-escaped-family-constructor/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/supplement/repeat-0/007-unclosed-escaped-family-constructor/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/supplement/repeat-0/007-unclosed-escaped-family-constructor/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/supplement/repeat-0/007-unclosed-escaped-family-constructor/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "grapheme-cluster-invalid-tail-property",
                expression: "#\"\\X(\"#.isRegex",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/supplement/repeat-0/008-grapheme-cluster-invalid-tail-property/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/supplement/repeat-0/008-grapheme-cluster-invalid-tail-property/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/supplement/repeat-0/008-grapheme-cluster-invalid-tail-property/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/supplement/repeat-0/008-grapheme-cluster-invalid-tail-property/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "grapheme-cluster-invalid-tail-constructor",
                expression: "Regex(#\"\\X(\"#)",
                kind: "error",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/supplement/repeat-0/009-grapheme-cluster-invalid-tail-constructor/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/supplement/repeat-0/009-grapheme-cluster-invalid-tail-constructor/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/supplement/repeat-0/009-grapheme-cluster-invalid-tail-constructor/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/supplement/repeat-0/009-grapheme-cluster-invalid-tail-constructor/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "grapheme-boundary-invalid-tail-property",
                expression: "#\"\\b{g}(\"#.isRegex",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/supplement/repeat-0/010-grapheme-boundary-invalid-tail-property/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/supplement/repeat-0/010-grapheme-boundary-invalid-tail-property/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/supplement/repeat-0/010-grapheme-boundary-invalid-tail-property/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/supplement/repeat-0/010-grapheme-boundary-invalid-tail-property/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "grapheme-boundary-invalid-tail-constructor",
                expression: "Regex(#\"\\b{g}(\"#)",
                kind: "error",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/supplement/repeat-0/011-grapheme-boundary-invalid-tail-constructor/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/supplement/repeat-0/011-grapheme-boundary-invalid-tail-constructor/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/supplement/repeat-0/011-grapheme-boundary-invalid-tail-constructor/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/supplement/repeat-0/011-grapheme-boundary-invalid-tail-constructor/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "class-grapheme-cluster-property",
                expression: "#\"[\\X]\"#.isRegex",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/supplement/repeat-0/012-class-grapheme-cluster-property/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/supplement/repeat-0/012-class-grapheme-cluster-property/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/supplement/repeat-0/012-class-grapheme-cluster-property/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/supplement/repeat-0/012-class-grapheme-cluster-property/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "class-grapheme-cluster-constructor",
                expression: "Regex(#\"[\\X]\"#)",
                kind: "error",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/supplement/repeat-0/013-class-grapheme-cluster-constructor/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/supplement/repeat-0/013-class-grapheme-cluster-constructor/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/supplement/repeat-0/013-class-grapheme-cluster-constructor/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/supplement/repeat-0/013-class-grapheme-cluster-constructor/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "class-grapheme-boundary-property",
                expression: "#\"[\\b{g}]\"#.isRegex",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/supplement/repeat-0/014-class-grapheme-boundary-property/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/supplement/repeat-0/014-class-grapheme-boundary-property/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/supplement/repeat-0/014-class-grapheme-boundary-property/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/supplement/repeat-0/014-class-grapheme-boundary-property/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "class-grapheme-boundary-constructor",
                expression: "Regex(#\"[\\b{g}]\"#)",
                kind: "error",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/supplement/repeat-0/015-class-grapheme-boundary-constructor/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/supplement/repeat-0/015-class-grapheme-boundary-constructor/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/supplement/repeat-0/015-class-grapheme-boundary-constructor/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/supplement/repeat-0/015-class-grapheme-boundary-constructor/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "named-character-valid-property",
                expression: "#\"\\N{LATIN CAPITAL LETTER A}\"#.isRegex",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/supplement/repeat-0/016-named-character-valid-property/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/supplement/repeat-0/016-named-character-valid-property/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/supplement/repeat-0/016-named-character-valid-property/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/supplement/repeat-0/016-named-character-valid-property/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "named-character-valid-constructor",
                expression: "Regex(#\"\\N{LATIN CAPITAL LETTER A}\"#)",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/supplement/repeat-0/017-named-character-valid-constructor/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/supplement/repeat-0/017-named-character-valid-constructor/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/supplement/repeat-0/017-named-character-valid-constructor/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/supplement/repeat-0/017-named-character-valid-constructor/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "named-character-unknown-property",
                expression: "#\"\\N{NO SUCH CHARACTER}\"#.isRegex",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/supplement/repeat-0/018-named-character-unknown-property/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/supplement/repeat-0/018-named-character-unknown-property/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/supplement/repeat-0/018-named-character-unknown-property/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/supplement/repeat-0/018-named-character-unknown-property/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "named-character-unknown-constructor",
                expression: "Regex(#\"\\N{NO SUCH CHARACTER}\"#)",
                kind: "error",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/supplement/repeat-0/019-named-character-unknown-constructor/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/supplement/repeat-0/019-named-character-unknown-constructor/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/supplement/repeat-0/019-named-character-unknown-constructor/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/supplement/repeat-0/019-named-character-unknown-constructor/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "named-character-unclosed-property",
                expression: "#\"\\N{LATIN CAPITAL LETTER A\"#.isRegex",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/supplement/repeat-0/020-named-character-unclosed-property/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/supplement/repeat-0/020-named-character-unclosed-property/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/supplement/repeat-0/020-named-character-unclosed-property/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/supplement/repeat-0/020-named-character-unclosed-property/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "named-character-unclosed-constructor",
                expression: "Regex(#\"\\N{LATIN CAPITAL LETTER A\"#)",
                kind: "error",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/supplement/repeat-0/021-named-character-unclosed-constructor/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/supplement/repeat-0/021-named-character-unclosed-constructor/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/supplement/repeat-0/021-named-character-unclosed-constructor/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/supplement/repeat-0/021-named-character-unclosed-constructor/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "uppercase-property-key-property",
                expression: "#\"\\p{GC=Lu}\"#.isRegex",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/supplement/repeat-0/022-uppercase-property-key-property/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/supplement/repeat-0/022-uppercase-property-key-property/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/supplement/repeat-0/022-uppercase-property-key-property/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/supplement/repeat-0/022-uppercase-property-key-property/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "uppercase-property-key-constructor",
                expression: "Regex(#\"\\p{GC=Lu}\"#)",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/supplement/repeat-0/023-uppercase-property-key-constructor/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/supplement/repeat-0/023-uppercase-property-key-constructor/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/supplement/repeat-0/023-uppercase-property-key-constructor/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-string-is-regex/supplement/repeat-0/023-uppercase-property-key-constructor/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
        ],
    );
}
