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
fn java_surface_all_raw_results_and_logs() {
    run(
        include_str!("../../../tests/fixtures/m2-regex-language/surface/module.pkl"),
        "file:///stage76/regex-surface.pkl",
        &[
            Row {
                id: "empty-pattern",
                expression: "Regex(\"\").pattern",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/000-empty-pattern/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/000-empty-pattern/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/000-empty-pattern/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/000-empty-pattern/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "literal-pattern",
                expression: "Regex(\"abc\").pattern",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/001-literal-pattern/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/001-literal-pattern/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/001-literal-pattern/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/001-literal-pattern/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "inline-flag-pattern",
                expression: "Regex(#\"(?i)AbC\"#).pattern",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/002-inline-flag-pattern/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/002-inline-flag-pattern/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/002-inline-flag-pattern/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/002-inline-flag-pattern/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "regex-render",
                expression: "Regex(\"a\").toString()",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/003-regex-render/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/003-regex-render/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/003-regex-render/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/003-regex-render/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "regex-type",
                expression: "Regex(\"a\") is Regex",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/004-regex-type/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/004-regex-type/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/004-regex-type/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/004-regex-type/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "qualified-constructor",
                expression: "qualifiedConstructor",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/005-qualified-constructor/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/005-qualified-constructor/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/005-qualified-constructor/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/005-qualified-constructor/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "equal-patterns",
                expression: "Regex(\"a\") == Regex(\"a\")",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/006-equal-patterns/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/006-equal-patterns/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/006-equal-patterns/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/006-equal-patterns/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "unequal-patterns",
                expression: "Regex(\"a\") == Regex(\"b\")",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/007-unequal-patterns/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/007-unequal-patterns/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/007-unequal-patterns/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/007-unequal-patterns/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "map-pattern-key",
                expression: "Map(Regex(\"a\"), 1)[Regex(\"a\")]",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/008-map-pattern-key/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/008-map-pattern-key/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/008-map-pattern-key/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/008-map-pattern-key/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "set-pattern-dedup",
                expression: "Set(Regex(\"a\"), Regex(\"a\")).length",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/009-set-pattern-dedup/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/009-set-pattern-dedup/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/009-set-pattern-dedup/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/009-set-pattern-dedup/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "group-count-zero",
                expression: "Regex(\"a\").groupCount",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/010-group-count-zero/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/010-group-count-zero/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/010-group-count-zero/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/010-group-count-zero/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "same-callsite-equality",
                expression: "makeRegex(\"a\") == makeRegex(\"a\")",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/011-same-callsite-equality/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/011-same-callsite-equality/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/011-same-callsite-equality/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/011-same-callsite-equality/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "invalid-open-group",
                expression: "Regex(\"(\")",
                kind: "error",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/012-invalid-open-group/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/012-invalid-open-group/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/012-invalid-open-group/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/012-invalid-open-group/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "invalid-star",
                expression: "Regex(\"*\")",
                kind: "error",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/013-invalid-star/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/013-invalid-star/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/013-invalid-star/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/013-invalid-star/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "constructor-wrong-type",
                expression: "Regex(42)",
                kind: "error",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/014-constructor-wrong-type/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/014-constructor-wrong-type/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/014-constructor-wrong-type/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/014-constructor-wrong-type/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "constructor-zero-arity",
                expression: "Regex()",
                kind: "error",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/015-constructor-zero-arity/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/015-constructor-zero-arity/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/015-constructor-zero-arity/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/015-constructor-zero-arity/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "constructor-two-arity",
                expression: "Regex(\"a\", \"b\")",
                kind: "error",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/016-constructor-two-arity/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/016-constructor-two-arity/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/016-constructor-two-arity/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/016-constructor-two-arity/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "unicode-pattern",
                expression: "Regex(\"é😀\").pattern",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/017-unicode-pattern/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/017-unicode-pattern/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/017-unicode-pattern/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/017-unicode-pattern/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "regex-get-class",
                expression: "Regex(\"a\").getClass()",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/018-regex-get-class/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/018-regex-get-class/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/018-regex-get-class/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/018-regex-get-class/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "regex-is-typed",
                expression: "Regex(\"a\") is Typed",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/019-regex-is-typed/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/019-regex-is-typed/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/019-regex-is-typed/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/019-regex-is-typed/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "find-empty-input",
                expression: "Regex(\"a\").findMatchesIn(\"\").length",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/020-find-empty-input/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/020-find-empty-input/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/020-find-empty-input/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/020-find-empty-input/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "find-none",
                expression: "Regex(\"a\").findMatchesIn(\"bbb\").length",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/021-find-none/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/021-find-none/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/021-find-none/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/021-find-none/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "find-one-value",
                expression: "Regex(\"a\").findMatchesIn(\"ba\").single.value",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/022-find-one-value/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/022-find-one-value/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/022-find-one-value/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/022-find-one-value/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "find-many-values",
                expression: "Regex(\"a\").findMatchesIn(\"aba\").map((it) -> it.value)",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/023-find-many-values/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/023-find-many-values/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/023-find-many-values/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/023-find-many-values/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "find-render",
                expression: "Regex(\"a\").findMatchesIn(\"a\").single.toString()",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/024-find-render/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/024-find-render/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/024-find-render/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/024-find-render/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "find-empty-pattern-empty",
                expression: "Regex(\"\").findMatchesIn(\"\").length",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/025-find-empty-pattern-empty/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/025-find-empty-pattern-empty/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/025-find-empty-pattern-empty/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/025-find-empty-pattern-empty/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "find-empty-pattern-text",
                expression: "Regex(\"\").findMatchesIn(\"ab\").length",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/026-find-empty-pattern-text/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/026-find-empty-pattern-text/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/026-find-empty-pattern-text/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/026-find-empty-pattern-text/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "find-dot-star",
                expression: "Regex(\".*\").findMatchesIn(\"ab\").length",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/027-find-dot-star/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/027-find-dot-star/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/027-find-dot-star/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/027-find-dot-star/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "find-group-count",
                expression: "Regex(\"(a)(b)\").groupCount",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/028-find-group-count/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/028-find-group-count/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/028-find-group-count/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/028-find-group-count/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "find-noncapturing-count",
                expression: "Regex(\"(?:a)(b)\").groupCount",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/029-find-noncapturing-count/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/029-find-noncapturing-count/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/029-find-noncapturing-count/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/029-find-noncapturing-count/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "whole-groups-length",
                expression: "Regex(\"(a)(b)\").findMatchesIn(\"ab\").single.groups.length",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/030-whole-groups-length/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/030-whole-groups-length/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/030-whole-groups-length/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/030-whole-groups-length/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "whole-group-zero",
                expression: "Regex(\"(a)(b)\").findMatchesIn(\"ab\").single.groups[0].value",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/031-whole-group-zero/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/031-whole-group-zero/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/031-whole-group-zero/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/031-whole-group-zero/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "whole-group-one",
                expression: "Regex(\"(a)(b)\").findMatchesIn(\"ab\").single.groups[1].value",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/032-whole-group-one/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/032-whole-group-one/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/032-whole-group-one/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/032-whole-group-one/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "whole-group-two",
                expression: "Regex(\"(a)(b)\").findMatchesIn(\"ab\").single.groups[2].value",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/033-whole-group-two/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/033-whole-group-two/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/033-whole-group-two/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/033-whole-group-two/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "group-start",
                expression: "Regex(\"(a)(b)\").findMatchesIn(\"ab\").single.groups[1].start",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/034-group-start/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/034-group-start/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/034-group-start/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/034-group-start/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "group-end",
                expression: "Regex(\"(a)(b)\").findMatchesIn(\"ab\").single.groups[1].end",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/035-group-end/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/035-group-end/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/035-group-end/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/035-group-end/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "group-own-groups-empty",
                expression: "Regex(\"(a)(b)\").findMatchesIn(\"ab\").single.groups[1].groups.length",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/036-group-own-groups-empty/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/036-group-own-groups-empty/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/036-group-own-groups-empty/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/036-group-own-groups-empty/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "unmatched-group-null",
                expression: "Regex(\"(a)?b\").findMatchesIn(\"b\").single.groups[1]",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/037-unmatched-group-null/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/037-unmatched-group-null/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/037-unmatched-group-null/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/037-unmatched-group-null/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "empty-matched-group",
                expression: "Regex(\"(a*)b\").findMatchesIn(\"b\").single.groups[1].value",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/038-empty-matched-group/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/038-empty-matched-group/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/038-empty-matched-group/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/038-empty-matched-group/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "entire-success-value",
                expression: "Regex(\"a\").matchEntire(\"a\").value",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/039-entire-success-value/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/039-entire-success-value/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/039-entire-success-value/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/039-entire-success-value/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "entire-failure-null",
                expression: "Regex(\"a\").matchEntire(\"ab\")",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/040-entire-failure-null/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/040-entire-failure-null/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/040-entire-failure-null/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/040-entire-failure-null/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "entire-group-length",
                expression: "Regex(\"(a)(b)\").matchEntire(\"ab\").groups.length",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/041-entire-group-length/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/041-entire-group-length/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/041-entire-group-length/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/041-entire-group-length/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "entire-empty",
                expression: "Regex(\"\").matchEntire(\"\").value",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/042-entire-empty/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/042-entire-empty/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/042-entire-empty/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/042-entire-empty/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "entire-start",
                expression: "Regex(\"a\").matchEntire(\"a\").start",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/043-entire-start/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/043-entire-start/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/043-entire-start/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/043-entire-start/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "entire-end",
                expression: "Regex(\"a\").matchEntire(\"a\").end",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/044-entire-end/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/044-entire-end/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/044-entire-end/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/044-entire-end/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "match-get-class",
                expression: "Regex(\"a\").findMatchesIn(\"a\").single.getClass()",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/045-match-get-class/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/045-match-get-class/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/045-match-get-class/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/045-match-get-class/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "match-is-regex-match",
                expression: "Regex(\"a\").findMatchesIn(\"a\").single is RegexMatch",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/046-match-is-regex-match/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/046-match-is-regex-match/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/046-match-is-regex-match/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/046-match-is-regex-match/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "match-value-equality",
                expression: "Regex(\"a\").findMatchesIn(\"a\").single == Regex(\"a\").findMatchesIn(\"a\").single",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/047-match-value-equality/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/047-match-value-equality/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/047-match-value-equality/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/047-match-value-equality/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "group-zero-own-groups",
                expression: "Regex(\"(a)\").findMatchesIn(\"a\").single.groups[0].groups.length",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/048-group-zero-own-groups/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/048-group-zero-own-groups/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/048-group-zero-own-groups/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/048-group-zero-own-groups/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "alternation-entire-long-branch",
                expression: "Regex(\"a|ab\").matchEntire(\"ab\").value",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/049-alternation-entire-long-branch/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/049-alternation-entire-long-branch/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/049-alternation-entire-long-branch/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/049-alternation-entire-long-branch/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "start-after-emoji",
                expression: "Regex(\"a\").findMatchesIn(\"😀a\").single.start",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/050-start-after-emoji/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/050-start-after-emoji/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/050-start-after-emoji/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/050-start-after-emoji/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "end-after-emoji",
                expression: "Regex(\"a\").findMatchesIn(\"😀a\").single.end",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/051-end-after-emoji/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/051-end-after-emoji/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/051-end-after-emoji/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/051-end-after-emoji/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "group-start-after-emoji",
                expression: "Regex(\"(a)\").findMatchesIn(\"😀a\").single.groups[1].start",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/052-group-start-after-emoji/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/052-group-start-after-emoji/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/052-group-start-after-emoji/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/052-group-start-after-emoji/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "group-end-after-emoji",
                expression: "Regex(\"(a)\").findMatchesIn(\"😀a\").single.groups[1].end",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/053-group-end-after-emoji/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/053-group-end-after-emoji/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/053-group-end-after-emoji/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/053-group-end-after-emoji/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "emoji-start",
                expression: "Regex(\"😀\").matchEntire(\"😀\").start",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/054-emoji-start/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/054-emoji-start/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/054-emoji-start/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/054-emoji-start/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "emoji-end",
                expression: "Regex(\"😀\").matchEntire(\"😀\").end",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/055-emoji-end/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/055-emoji-end/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/055-emoji-end/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/055-emoji-end/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "dot-emoji-count",
                expression: "Regex(\".\").findMatchesIn(\"😀\").length",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/056-dot-emoji-count/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/056-dot-emoji-count/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/056-dot-emoji-count/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/056-dot-emoji-count/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "unicode-word-accent",
                expression: "Regex(#\"\\w\"#).matchEntire(\"é\")",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/057-unicode-word-accent/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/057-unicode-word-accent/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/057-unicode-word-accent/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/057-unicode-word-accent/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "unicode-digit-arabic",
                expression: "Regex(#\"\\d\"#).matchEntire(\"٢\")",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/058-unicode-digit-arabic/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/058-unicode-digit-arabic/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/058-unicode-digit-arabic/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/058-unicode-digit-arabic/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "unicode-case-accent",
                expression: "Regex(#\"(?i)é\"#).matchEntire(\"É\")",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/059-unicode-case-accent/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/059-unicode-case-accent/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/059-unicode-case-accent/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/059-unicode-case-accent/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "zero-width-after-emoji",
                expression: "Regex(\"\").findMatchesIn(\"😀\").length",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/060-zero-width-after-emoji/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/060-zero-width-after-emoji/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/060-zero-width-after-emoji/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/060-zero-width-after-emoji/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "lone-high-dot",
                expression: "Regex(\".\").findMatchesIn(\"\\u{d800}\").length",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/061-lone-high-dot/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/061-lone-high-dot/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/061-lone-high-dot/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/061-lone-high-dot/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "lone-low-dot",
                expression: "Regex(\".\").findMatchesIn(\"\\u{dc00}\").length",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/062-lone-low-dot/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/062-lone-low-dot/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/062-lone-low-dot/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/062-lone-low-dot/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "paired-surrogates-dot",
                expression: "Regex(\".\").findMatchesIn(\"\\u{d800}\\u{dc00}\").length",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/063-paired-surrogates-dot/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/063-paired-surrogates-dot/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/063-paired-surrogates-dot/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/063-paired-surrogates-dot/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "start-after-lone-high",
                expression: "Regex(\"a\").findMatchesIn(\"\\u{d800}a\").single.start",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/064-start-after-lone-high/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/064-start-after-lone-high/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/064-start-after-lone-high/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/064-start-after-lone-high/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "unicode-space-nbsp",
                expression: "Regex(#\"\\s\"#).matchEntire(\"\\u{00a0}\")",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/065-unicode-space-nbsp/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/065-unicode-space-nbsp/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/065-unicode-space-nbsp/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/065-unicode-space-nbsp/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "ascii-word-override",
                expression: "Regex(#\"(?-U)\\w\"#).matchEntire(\"é\")",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/066-ascii-word-override/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/066-ascii-word-override/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/066-ascii-word-override/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/066-ascii-word-override/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "zero-width-emoji-offsets",
                expression: "Regex(\"\").findMatchesIn(\"😀\").map((m) -> m.start)",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/067-zero-width-emoji-offsets/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/067-zero-width-emoji-offsets/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/067-zero-width-emoji-offsets/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/067-zero-width-emoji-offsets/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "lone-high-pattern",
                expression: "Regex(\"\\u{d800}\").findMatchesIn(\"\\u{d800}\").length",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/068-lone-high-pattern/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/068-lone-high-pattern/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/068-lone-high-pattern/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/068-lone-high-pattern/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "lone-low-pattern",
                expression: "Regex(\"\\u{dc00}\").findMatchesIn(\"\\u{dc00}\").length",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/069-lone-low-pattern/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/069-lone-low-pattern/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/069-lone-low-pattern/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/069-lone-low-pattern/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "pattern-as-method",
                expression: "Regex(\"a\").pattern()",
                kind: "error",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/070-pattern-as-method/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/070-pattern-as-method/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/070-pattern-as-method/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/070-pattern-as-method/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "group-count-as-method",
                expression: "Regex(\"a\").groupCount()",
                kind: "error",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/071-group-count-as-method/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/071-group-count-as-method/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/071-group-count-as-method/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/071-group-count-as-method/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "find-as-property",
                expression: "Regex(\"a\").findMatchesIn",
                kind: "error",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/072-find-as-property/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/072-find-as-property/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/072-find-as-property/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/072-find-as-property/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "entire-as-property",
                expression: "Regex(\"a\").matchEntire",
                kind: "error",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/073-entire-as-property/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/073-entire-as-property/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/073-entire-as-property/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/073-entire-as-property/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "find-zero-arity",
                expression: "Regex(\"a\").findMatchesIn()",
                kind: "error",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/074-find-zero-arity/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/074-find-zero-arity/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/074-find-zero-arity/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/074-find-zero-arity/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "find-two-arity",
                expression: "Regex(\"a\").findMatchesIn(\"a\", \"b\")",
                kind: "error",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/075-find-two-arity/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/075-find-two-arity/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/075-find-two-arity/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/075-find-two-arity/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "find-wrong-type",
                expression: "Regex(\"a\").findMatchesIn(42)",
                kind: "error",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/076-find-wrong-type/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/076-find-wrong-type/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/076-find-wrong-type/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/076-find-wrong-type/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "entire-zero-arity",
                expression: "Regex(\"a\").matchEntire()",
                kind: "error",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/077-entire-zero-arity/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/077-entire-zero-arity/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/077-entire-zero-arity/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/077-entire-zero-arity/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "entire-wrong-type",
                expression: "Regex(\"a\").matchEntire(42)",
                kind: "error",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/078-entire-wrong-type/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/078-entire-wrong-type/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/078-entire-wrong-type/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/078-entire-wrong-type/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "wrong-receiver",
                expression: "42.findMatchesIn(\"a\")",
                kind: "error",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/079-wrong-receiver/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/079-wrong-receiver/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/079-wrong-receiver/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/079-wrong-receiver/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "constructor-throwing-argument",
                expression: "Regex(throw(\"argument\"))",
                kind: "error",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/080-constructor-throwing-argument/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/080-constructor-throwing-argument/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/080-constructor-throwing-argument/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/080-constructor-throwing-argument/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "find-throwing-argument",
                expression: "Regex(\"a\").findMatchesIn(throw(\"argument\"))",
                kind: "error",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/081-find-throwing-argument/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/081-find-throwing-argument/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/081-find-throwing-argument/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/081-find-throwing-argument/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "invalid-pattern-before-argument",
                expression: "Regex(\"(\").findMatchesIn(throw(\"argument\"))",
                kind: "error",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/082-invalid-pattern-before-argument/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/082-invalid-pattern-before-argument/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/082-invalid-pattern-before-argument/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/082-invalid-pattern-before-argument/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "new-external-regex",
                expression: "new Regex {}",
                kind: "error",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/083-new-external-regex/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/083-new-external-regex/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/083-new-external-regex/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/083-new-external-regex/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "amend-regex",
                expression: "(Regex(\"a\")) {}",
                kind: "error",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/084-amend-regex/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/084-amend-regex/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/084-amend-regex/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/084-amend-regex/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "regex-as-string",
                expression: "Regex(\"a\") as String",
                kind: "error",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/085-regex-as-string/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/085-regex-as-string/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/085-regex-as-string/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/085-regex-as-string/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "regex-is-string",
                expression: "Regex(\"a\") is String",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/086-regex-is-string/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/086-regex-is-string/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/086-regex-is-string/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/086-regex-is-string/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "missing-regex-property",
                expression: "Regex(\"a\").notPresent",
                kind: "error",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/087-missing-regex-property/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/087-missing-regex-property/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/087-missing-regex-property/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/087-missing-regex-property/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "contains-regex",
                expression: "\"abc\".contains(Regex(\"b\"))",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/088-contains-regex/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/088-contains-regex/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/088-contains-regex/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/088-contains-regex/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "matches-regex",
                expression: "\"abc\".matches(Regex(\"a.c\"))",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/089-matches-regex/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/089-matches-regex/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/089-matches-regex/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/089-matches-regex/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "starts-with-regex",
                expression: "\"abc\".startsWith(Regex(\"a\"))",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/090-starts-with-regex/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/090-starts-with-regex/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/090-starts-with-regex/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/090-starts-with-regex/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "ends-with-regex",
                expression: "\"abc\".endsWith(Regex(\"c\"))",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/091-ends-with-regex/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/091-ends-with-regex/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/091-ends-with-regex/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/091-ends-with-regex/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "index-of-regex",
                expression: "\"abc\".indexOf(Regex(\"b\"))",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/092-index-of-regex/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/092-index-of-regex/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/092-index-of-regex/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/092-index-of-regex/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "index-of-or-null",
                expression: "\"abc\".indexOfOrNull(Regex(\"z\"))",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/093-index-of-or-null/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/093-index-of-or-null/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/093-index-of-or-null/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/093-index-of-or-null/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "last-index-of-regex",
                expression: "\"ababa\".lastIndexOf(Regex(\"ba\"))",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/094-last-index-of-regex/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/094-last-index-of-regex/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/094-last-index-of-regex/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/094-last-index-of-regex/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "last-index-of-or-null",
                expression: "\"abc\".lastIndexOfOrNull(Regex(\"z\"))",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/095-last-index-of-or-null/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/095-last-index-of-or-null/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/095-last-index-of-or-null/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/095-last-index-of-or-null/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "split-regex",
                expression: "\"a,b,c\".split(Regex(\",\"))",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/096-split-regex/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/096-split-regex/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/096-split-regex/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/096-split-regex/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "split-limit-regex",
                expression: "\"a,b,c\".splitLimit(Regex(\",\"), 2)",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/097-split-limit-regex/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/097-split-limit-regex/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/097-split-limit-regex/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/097-split-limit-regex/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "replace-all-regex",
                expression: "\"aba\".replaceAll(Regex(\"a\"), \"x\")",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/098-replace-all-regex/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/098-replace-all-regex/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/098-replace-all-regex/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/098-replace-all-regex/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "replace-first-regex",
                expression: "\"aba\".replaceFirst(Regex(\"a\"), \"x\")",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/099-replace-first-regex/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/099-replace-first-regex/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/099-replace-first-regex/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/099-replace-first-regex/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "replace-last-regex",
                expression: "\"aba\".replaceLast(Regex(\"a\"), \"x\")",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/100-replace-last-regex/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/100-replace-last-regex/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/100-replace-last-regex/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/100-replace-last-regex/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "replace-all-mapped",
                expression: "\"aba\".replaceAllMapped(Regex(\"(a)\"), (m) -> m.groups[1].value)",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/101-replace-all-mapped/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/101-replace-all-mapped/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/101-replace-all-mapped/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/101-replace-all-mapped/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "replace-first-mapped",
                expression: "\"aba\".replaceFirstMapped(Regex(\"(a)\"), (m) -> m.groups[1].value)",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/102-replace-first-mapped/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/102-replace-first-mapped/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/102-replace-first-mapped/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/102-replace-first-mapped/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "replace-last-mapped",
                expression: "\"aba\".replaceLastMapped(Regex(\"(a)\"), (m) -> m.groups[1].value)",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/103-replace-last-mapped/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/103-replace-last-mapped/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/103-replace-last-mapped/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/103-replace-last-mapped/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "unicode-property-contains",
                expression: "\"é\".contains(Regex(#\"\\p{L}\"#))",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/104-unicode-property-contains/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/104-unicode-property-contains/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/104-unicode-property-contains/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/104-unicode-property-contains/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "replacement-group",
                expression: "\"ab\".replaceAll(Regex(\"(a)\"), \"$1\")",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/105-replacement-group/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/105-replacement-group/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/105-replacement-group/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/105-replacement-group/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "replacement-missing-group",
                expression: "\"ab\".replaceAll(Regex(\"a\"), \"$4\")",
                kind: "error",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/106-replacement-missing-group/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/106-replacement-missing-group/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/106-replacement-missing-group/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/106-replacement-missing-group/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "string-index-codepoint",
                expression: "\"😀x\".indexOf(Regex(\"x\"))",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/107-string-index-codepoint/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/107-string-index-codepoint/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/107-string-index-codepoint/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/surface/107-string-index-codepoint/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
        ],
    );
}

#[test]
fn java_shadow_all_raw_results_and_logs() {
    run(
        include_str!("../../../tests/fixtures/m2-regex-language/shadow/module.pkl"),
        "file:///stage76/regex-shadow.pkl",
        &[
            Row {
                id: "shadow-constructor",
                expression: "shadowConstructor",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/shadow/000-shadow-constructor/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/shadow/000-shadow-constructor/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/shadow/000-shadow-constructor/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/shadow/000-shadow-constructor/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "qualified-under-shadow",
                expression: "qualifiedUnderShadow",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/shadow/001-qualified-under-shadow/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/shadow/001-qualified-under-shadow/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/shadow/001-qualified-under-shadow/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/shadow/001-qualified-under-shadow/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "shadow-is-regex",
                expression: "shadowIsRegex",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/shadow/002-shadow-is-regex/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/shadow/002-shadow-is-regex/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/shadow/002-shadow-is-regex/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/shadow/002-shadow-is-regex/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "qualified-is-regex",
                expression: "qualifiedIsRegex",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/shadow/003-qualified-is-regex/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/shadow/003-qualified-is-regex/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/shadow/003-qualified-is-regex/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-regex-language/shadow/003-qualified-is-regex/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
        ],
    );
}
