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
            let event = terminal.expect("bounded Reference type poll");
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
fn java_main_raw_results_and_logs() {
    run(
        include_str!("../../../tests/fixtures/m2-reference-declared-type/main/module.pkl"),
        "file:///stage74/reference-type.pkl",
        &[
            Row {
                id: "optional-eq",
                expression: r########"holderRef.optional["k"] == holderRef.optional["k"]"########,
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-reference-declared-type/main/000-optional-eq/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-reference-declared-type/main/000-optional-eq/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-reference-declared-type/main/000-optional-eq/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-reference-declared-type/main/000-optional-eq/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "optional-type",
                expression: r########"holderRef.optional["k"] is ref.Reference<D, Int?>"########,
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-reference-declared-type/main/001-optional-type/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-reference-declared-type/main/001-optional-type/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-reference-declared-type/main/001-optional-type/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-reference-declared-type/main/001-optional-type/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "optional-is-wrong",
                expression: r########"holderRef.optional["k"] is ref.Reference<D, String>"########,
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-reference-declared-type/main/002-optional-is-wrong/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-reference-declared-type/main/002-optional-is-wrong/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-reference-declared-type/main/002-optional-is-wrong/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-reference-declared-type/main/002-optional-is-wrong/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "optional-path",
                expression: r########"holderRef.optional["k"].getPath().length"########,
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-reference-declared-type/main/003-optional-path/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-reference-declared-type/main/003-optional-path/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-reference-declared-type/main/003-optional-path/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-reference-declared-type/main/003-optional-path/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "optional-data",
                expression: r########"holderRef.optional["k"].getData()"########,
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-reference-declared-type/main/004-optional-data/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-reference-declared-type/main/004-optional-data/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-reference-declared-type/main/004-optional-data/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-reference-declared-type/main/004-optional-data/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "aliased-type",
                expression: r########"holderRef.aliased["k"] is ref.Reference<D, Int?>"########,
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-reference-declared-type/main/005-aliased-type/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-reference-declared-type/main/005-aliased-type/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-reference-declared-type/main/005-aliased-type/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-reference-declared-type/main/005-aliased-type/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "qualified-type",
                expression: r########"holderRef.qualified["k"] is ref.Reference<D, Int?>"########,
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-reference-declared-type/main/006-qualified-type/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-reference-declared-type/main/006-qualified-type/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-reference-declared-type/main/006-qualified-type/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-reference-declared-type/main/006-qualified-type/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "nested-listing",
                expression: r########"holderRef.nested["k"][0].getPath().length"########,
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-reference-declared-type/main/007-nested-listing/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-reference-declared-type/main/007-nested-listing/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-reference-declared-type/main/007-nested-listing/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-reference-declared-type/main/007-nested-listing/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "inner-optional",
                expression: r########"holderRef.inner.optional["k"].getPath().length"########,
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-reference-declared-type/main/008-inner-optional/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-reference-declared-type/main/008-inner-optional/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-reference-declared-type/main/008-inner-optional/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-reference-declared-type/main/008-inner-optional/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "lazy-reference",
                expression: r########"holderRef.lazy["k"].getPath().length"########,
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-reference-declared-type/main/009-lazy-reference/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-reference-declared-type/main/009-lazy-reference/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-reference-declared-type/main/009-lazy-reference/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-reference-declared-type/main/009-lazy-reference/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "path-first",
                expression: r########"holderRef.optional["k"].getPath()[0].property"########,
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-reference-declared-type/main/010-path-first/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-reference-declared-type/main/010-path-first/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-reference-declared-type/main/010-path-first/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-reference-declared-type/main/010-path-first/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "path-second",
                expression: r########"holderRef.optional["k"].getPath()[1].key"########,
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-reference-declared-type/main/011-path-second/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-reference-declared-type/main/011-path-second/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-reference-declared-type/main/011-path-second/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-reference-declared-type/main/011-path-second/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "wrong-key",
                expression: r########"holderRef.optional[1]"########,
                kind: "error",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-reference-declared-type/main/012-wrong-key/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-reference-declared-type/main/012-wrong-key/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-reference-declared-type/main/012-wrong-key/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-reference-declared-type/main/012-wrong-key/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "aliased-wrong-key",
                expression: r########"holderRef.aliased[1]"########,
                kind: "error",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-reference-declared-type/main/013-aliased-wrong-key/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-reference-declared-type/main/013-aliased-wrong-key/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-reference-declared-type/main/013-aliased-wrong-key/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-reference-declared-type/main/013-aliased-wrong-key/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "qualified-wrong-key",
                expression: r########"holderRef.qualified[1]"########,
                kind: "error",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-reference-declared-type/main/014-qualified-wrong-key/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-reference-declared-type/main/014-qualified-wrong-key/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-reference-declared-type/main/014-qualified-wrong-key/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-reference-declared-type/main/014-qualified-wrong-key/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "throwing-key",
                expression: r########"holderRef.optional[throw("key-failure")]"########,
                kind: "error",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-reference-declared-type/main/015-throwing-key/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-reference-declared-type/main/015-throwing-key/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-reference-declared-type/main/015-throwing-key/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-reference-declared-type/main/015-throwing-key/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "missing-property",
                expression: r########"holderRef.missing[throw("key-failure")]"########,
                kind: "error",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-reference-declared-type/main/016-missing-property/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-reference-declared-type/main/016-missing-property/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-reference-declared-type/main/016-missing-property/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-reference-declared-type/main/016-missing-property/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "lazy-ref",
                expression: r########"lazyRef.lazy["k"].getPath().length"########,
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-reference-declared-type/main/017-lazy-ref/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-reference-declared-type/main/017-lazy-ref/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-reference-declared-type/main/017-lazy-ref/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-reference-declared-type/main/017-lazy-ref/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "lazy-direct",
                expression: r########"lazyInstance.lazy"########,
                kind: "error",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-reference-declared-type/main/018-lazy-direct/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-reference-declared-type/main/018-lazy-direct/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-reference-declared-type/main/018-lazy-direct/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-reference-declared-type/main/018-lazy-direct/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[[
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-reference-declared-type/main/018-lazy-direct/log-000.level.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-reference-declared-type/main/018-lazy-direct/log-000.level.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-reference-declared-type/main/018-lazy-direct/log-000.message.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-reference-declared-type/main/018-lazy-direct/log-000.message.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-reference-declared-type/main/018-lazy-direct/log-000.module-uri.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-reference-declared-type/main/018-lazy-direct/log-000.module-uri.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-reference-declared-type/main/018-lazy-direct/log-000.member.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-reference-declared-type/main/018-lazy-direct/log-000.member.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-reference-declared-type/main/018-lazy-direct/log-000.position.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-reference-declared-type/main/018-lazy-direct/log-000.position.code-units.bin"
                        ) as &[u8],
                    ),
                ]],
            },
            Row {
                id: "lazy-owner",
                expression: r########"holderRef.lazy.getPath().length"########,
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-reference-declared-type/main/019-lazy-owner/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-reference-declared-type/main/019-lazy-owner/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-reference-declared-type/main/019-lazy-owner/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-reference-declared-type/main/019-lazy-owner/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
        ],
    );
}
