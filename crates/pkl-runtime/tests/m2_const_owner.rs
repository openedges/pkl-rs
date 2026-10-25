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
            let event = terminal.expect("bounded const-owner poll");
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
fn java_main_const_owner_results_and_logs() {
    run(
        include_str!("../../../tests/fixtures/m2-const-owner/main/module.pkl"),
        "file:///stage72/const-owner.pkl",
        &[
            Row {
                id: "const-owner-00",
                expression: r########"a.text"########,
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/main/000-const-owner-00/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/main/000-const-owner-00/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/main/000-const-owner-00/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/main/000-const-owner-00/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[[
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/main/000-const-owner-00/log-000.level.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/main/000-const-owner-00/log-000.level.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/main/000-const-owner-00/log-000.message.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/main/000-const-owner-00/log-000.message.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/main/000-const-owner-00/log-000.module-uri.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/main/000-const-owner-00/log-000.module-uri.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/main/000-const-owner-00/log-000.member.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/main/000-const-owner-00/log-000.member.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/main/000-const-owner-00/log-000.position.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/main/000-const-owner-00/log-000.position.code-units.bin"
                        ) as &[u8],
                    ),
                ]],
            },
            Row {
                id: "const-owner-01",
                expression: r########"b.text"########,
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/main/001-const-owner-01/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/main/001-const-owner-01/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/main/001-const-owner-01/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/main/001-const-owner-01/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "const-owner-02",
                expression: r########"c.text"########,
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/main/002-const-owner-02/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/main/002-const-owner-02/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/main/002-const-owner-02/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/main/002-const-owner-02/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "const-owner-03",
                expression: r########"d.text"########,
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/main/003-const-owner-03/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/main/003-const-owner-03/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/main/003-const-owner-03/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/main/003-const-owner-03/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "const-owner-04",
                expression: r########"a.typed"########,
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/main/004-const-owner-04/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/main/004-const-owner-04/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/main/004-const-owner-04/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/main/004-const-owner-04/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[[
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/main/004-const-owner-04/log-000.level.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/main/004-const-owner-04/log-000.level.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/main/004-const-owner-04/log-000.message.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/main/004-const-owner-04/log-000.message.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/main/004-const-owner-04/log-000.module-uri.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/main/004-const-owner-04/log-000.module-uri.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/main/004-const-owner-04/log-000.member.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/main/004-const-owner-04/log-000.member.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/main/004-const-owner-04/log-000.position.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/main/004-const-owner-04/log-000.position.code-units.bin"
                        ) as &[u8],
                    ),
                ]],
            },
            Row {
                id: "const-owner-05",
                expression: r########"b.typed"########,
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/main/005-const-owner-05/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/main/005-const-owner-05/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/main/005-const-owner-05/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/main/005-const-owner-05/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "const-owner-06",
                expression: r########"c.typed"########,
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/main/006-const-owner-06/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/main/006-const-owner-06/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/main/006-const-owner-06/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/main/006-const-owner-06/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "const-owner-07",
                expression: r########"d.typed"########,
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/main/007-const-owner-07/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/main/007-const-owner-07/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/main/007-const-owner-07/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/main/007-const-owner-07/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "const-owner-08",
                expression: r########"a.ordinary"########,
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/main/008-const-owner-08/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/main/008-const-owner-08/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/main/008-const-owner-08/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/main/008-const-owner-08/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[[
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/main/008-const-owner-08/log-000.level.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/main/008-const-owner-08/log-000.level.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/main/008-const-owner-08/log-000.message.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/main/008-const-owner-08/log-000.message.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/main/008-const-owner-08/log-000.module-uri.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/main/008-const-owner-08/log-000.module-uri.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/main/008-const-owner-08/log-000.member.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/main/008-const-owner-08/log-000.member.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/main/008-const-owner-08/log-000.position.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/main/008-const-owner-08/log-000.position.code-units.bin"
                        ) as &[u8],
                    ),
                ]],
            },
            Row {
                id: "const-owner-09",
                expression: r########"b.ordinary"########,
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/main/009-const-owner-09/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/main/009-const-owner-09/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/main/009-const-owner-09/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/main/009-const-owner-09/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[[
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/main/009-const-owner-09/log-000.level.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/main/009-const-owner-09/log-000.level.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/main/009-const-owner-09/log-000.message.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/main/009-const-owner-09/log-000.message.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/main/009-const-owner-09/log-000.module-uri.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/main/009-const-owner-09/log-000.module-uri.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/main/009-const-owner-09/log-000.member.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/main/009-const-owner-09/log-000.member.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/main/009-const-owner-09/log-000.position.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/main/009-const-owner-09/log-000.position.code-units.bin"
                        ) as &[u8],
                    ),
                ]],
            },
            Row {
                id: "const-owner-10",
                expression: r########"c.ordinary"########,
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/main/010-const-owner-10/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/main/010-const-owner-10/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/main/010-const-owner-10/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/main/010-const-owner-10/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "const-owner-11",
                expression: r########"d.ordinary"########,
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/main/011-const-owner-11/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/main/011-const-owner-11/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/main/011-const-owner-11/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/main/011-const-owner-11/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[[
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/main/011-const-owner-11/log-000.level.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/main/011-const-owner-11/log-000.level.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/main/011-const-owner-11/log-000.message.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/main/011-const-owner-11/log-000.message.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/main/011-const-owner-11/log-000.module-uri.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/main/011-const-owner-11/log-000.module-uri.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/main/011-const-owner-11/log-000.member.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/main/011-const-owner-11/log-000.member.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/main/011-const-owner-11/log-000.position.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/main/011-const-owner-11/log-000.position.code-units.bin"
                        ) as &[u8],
                    ),
                ]],
            },
            Row {
                id: "const-owner-12",
                expression: r########"failedA.value"########,
                kind: "error",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/main/012-const-owner-12/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/main/012-const-owner-12/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/main/012-const-owner-12/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/main/012-const-owner-12/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[[
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/main/012-const-owner-12/log-000.level.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/main/012-const-owner-12/log-000.level.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/main/012-const-owner-12/log-000.message.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/main/012-const-owner-12/log-000.message.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/main/012-const-owner-12/log-000.module-uri.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/main/012-const-owner-12/log-000.module-uri.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/main/012-const-owner-12/log-000.member.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/main/012-const-owner-12/log-000.member.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/main/012-const-owner-12/log-000.position.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/main/012-const-owner-12/log-000.position.code-units.bin"
                        ) as &[u8],
                    ),
                ]],
            },
            Row {
                id: "const-owner-13",
                expression: r########"failedB.value"########,
                kind: "error",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/main/013-const-owner-13/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/main/013-const-owner-13/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/main/013-const-owner-13/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/main/013-const-owner-13/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[[
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/main/013-const-owner-13/log-000.level.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/main/013-const-owner-13/log-000.level.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/main/013-const-owner-13/log-000.message.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/main/013-const-owner-13/log-000.message.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/main/013-const-owner-13/log-000.module-uri.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/main/013-const-owner-13/log-000.module-uri.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/main/013-const-owner-13/log-000.member.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/main/013-const-owner-13/log-000.member.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/main/013-const-owner-13/log-000.position.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/main/013-const-owner-13/log-000.position.code-units.bin"
                        ) as &[u8],
                    ),
                ]],
            },
            Row {
                id: "const-owner-14",
                expression: r########"failedA.value"########,
                kind: "error",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/main/014-const-owner-14/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/main/014-const-owner-14/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/main/014-const-owner-14/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/main/014-const-owner-14/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[[
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/main/014-const-owner-14/log-000.level.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/main/014-const-owner-14/log-000.level.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/main/014-const-owner-14/log-000.message.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/main/014-const-owner-14/log-000.message.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/main/014-const-owner-14/log-000.module-uri.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/main/014-const-owner-14/log-000.module-uri.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/main/014-const-owner-14/log-000.member.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/main/014-const-owner-14/log-000.member.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/main/014-const-owner-14/log-000.position.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/main/014-const-owner-14/log-000.position.code-units.bin"
                        ) as &[u8],
                    ),
                ]],
            },
            Row {
                id: "const-owner-15",
                expression: r########"wrongA.value"########,
                kind: "error",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/main/015-const-owner-15/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/main/015-const-owner-15/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/main/015-const-owner-15/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/main/015-const-owner-15/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[[
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/main/015-const-owner-15/log-000.level.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/main/015-const-owner-15/log-000.level.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/main/015-const-owner-15/log-000.message.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/main/015-const-owner-15/log-000.message.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/main/015-const-owner-15/log-000.module-uri.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/main/015-const-owner-15/log-000.module-uri.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/main/015-const-owner-15/log-000.member.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/main/015-const-owner-15/log-000.member.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/main/015-const-owner-15/log-000.position.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/main/015-const-owner-15/log-000.position.code-units.bin"
                        ) as &[u8],
                    ),
                ]],
            },
            Row {
                id: "const-owner-16",
                expression: r########"wrongB.value"########,
                kind: "error",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/main/016-const-owner-16/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/main/016-const-owner-16/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/main/016-const-owner-16/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/main/016-const-owner-16/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[[
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/main/016-const-owner-16/log-000.level.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/main/016-const-owner-16/log-000.level.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/main/016-const-owner-16/log-000.message.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/main/016-const-owner-16/log-000.message.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/main/016-const-owner-16/log-000.module-uri.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/main/016-const-owner-16/log-000.module-uri.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/main/016-const-owner-16/log-000.member.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/main/016-const-owner-16/log-000.member.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/main/016-const-owner-16/log-000.position.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/main/016-const-owner-16/log-000.position.code-units.bin"
                        ) as &[u8],
                    ),
                ]],
            },
            Row {
                id: "const-owner-17",
                expression: r########"wrongA.value"########,
                kind: "error",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/main/017-const-owner-17/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/main/017-const-owner-17/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/main/017-const-owner-17/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/main/017-const-owner-17/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[[
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/main/017-const-owner-17/log-000.level.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/main/017-const-owner-17/log-000.level.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/main/017-const-owner-17/log-000.message.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/main/017-const-owner-17/log-000.message.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/main/017-const-owner-17/log-000.module-uri.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/main/017-const-owner-17/log-000.module-uri.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/main/017-const-owner-17/log-000.member.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/main/017-const-owner-17/log-000.member.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/main/017-const-owner-17/log-000.position.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/main/017-const-owner-17/log-000.position.code-units.bin"
                        ) as &[u8],
                    ),
                ]],
            },
        ],
    );
}

#[test]
fn java_reverse_const_owner_results_and_logs() {
    run(
        include_str!("../../../tests/fixtures/m2-const-owner/reverse/module.pkl"),
        "file:///stage72/const-owner.pkl",
        &[
            Row {
                id: "reverse-00",
                expression: r########"child.constValue"########,
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/reverse/000-reverse-00/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/reverse/000-reverse-00/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/reverse/000-reverse-00/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/reverse/000-reverse-00/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[[
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/reverse/000-reverse-00/log-000.level.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/reverse/000-reverse-00/log-000.level.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/reverse/000-reverse-00/log-000.message.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/reverse/000-reverse-00/log-000.message.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/reverse/000-reverse-00/log-000.module-uri.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/reverse/000-reverse-00/log-000.module-uri.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/reverse/000-reverse-00/log-000.member.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/reverse/000-reverse-00/log-000.member.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/reverse/000-reverse-00/log-000.position.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/reverse/000-reverse-00/log-000.position.code-units.bin"
                        ) as &[u8],
                    ),
                ]],
            },
            Row {
                id: "reverse-01",
                expression: r########"base.constValue"########,
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/reverse/001-reverse-01/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/reverse/001-reverse-01/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/reverse/001-reverse-01/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/reverse/001-reverse-01/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "reverse-02",
                expression: r########"sibling.constValue"########,
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/reverse/002-reverse-02/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/reverse/002-reverse-02/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/reverse/002-reverse-02/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/reverse/002-reverse-02/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "reverse-03",
                expression: r########"child.constValue"########,
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/reverse/003-reverse-03/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/reverse/003-reverse-03/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/reverse/003-reverse-03/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/reverse/003-reverse-03/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "reverse-04",
                expression: r########"child.fixedValue"########,
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/reverse/004-reverse-04/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/reverse/004-reverse-04/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/reverse/004-reverse-04/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/reverse/004-reverse-04/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[[
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/reverse/004-reverse-04/log-000.level.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/reverse/004-reverse-04/log-000.level.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/reverse/004-reverse-04/log-000.message.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/reverse/004-reverse-04/log-000.message.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/reverse/004-reverse-04/log-000.module-uri.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/reverse/004-reverse-04/log-000.module-uri.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/reverse/004-reverse-04/log-000.member.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/reverse/004-reverse-04/log-000.member.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/reverse/004-reverse-04/log-000.position.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/reverse/004-reverse-04/log-000.position.code-units.bin"
                        ) as &[u8],
                    ),
                ]],
            },
            Row {
                id: "reverse-05",
                expression: r########"base.fixedValue"########,
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/reverse/005-reverse-05/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/reverse/005-reverse-05/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/reverse/005-reverse-05/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/reverse/005-reverse-05/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[[
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/reverse/005-reverse-05/log-000.level.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/reverse/005-reverse-05/log-000.level.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/reverse/005-reverse-05/log-000.message.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/reverse/005-reverse-05/log-000.message.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/reverse/005-reverse-05/log-000.module-uri.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/reverse/005-reverse-05/log-000.module-uri.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/reverse/005-reverse-05/log-000.member.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/reverse/005-reverse-05/log-000.member.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/reverse/005-reverse-05/log-000.position.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/reverse/005-reverse-05/log-000.position.code-units.bin"
                        ) as &[u8],
                    ),
                ]],
            },
            Row {
                id: "reverse-06",
                expression: r########"sibling.fixedValue"########,
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/reverse/006-reverse-06/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/reverse/006-reverse-06/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/reverse/006-reverse-06/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/reverse/006-reverse-06/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "reverse-07",
                expression: r########"child.typedFixed"########,
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/reverse/007-reverse-07/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/reverse/007-reverse-07/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/reverse/007-reverse-07/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/reverse/007-reverse-07/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[[
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/reverse/007-reverse-07/log-000.level.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/reverse/007-reverse-07/log-000.level.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/reverse/007-reverse-07/log-000.message.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/reverse/007-reverse-07/log-000.message.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/reverse/007-reverse-07/log-000.module-uri.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/reverse/007-reverse-07/log-000.module-uri.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/reverse/007-reverse-07/log-000.member.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/reverse/007-reverse-07/log-000.member.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/reverse/007-reverse-07/log-000.position.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/reverse/007-reverse-07/log-000.position.code-units.bin"
                        ) as &[u8],
                    ),
                ]],
            },
            Row {
                id: "reverse-08",
                expression: r########"base.typedFixed"########,
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/reverse/008-reverse-08/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/reverse/008-reverse-08/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/reverse/008-reverse-08/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/reverse/008-reverse-08/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[[
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/reverse/008-reverse-08/log-000.level.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/reverse/008-reverse-08/log-000.level.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/reverse/008-reverse-08/log-000.message.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/reverse/008-reverse-08/log-000.message.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/reverse/008-reverse-08/log-000.module-uri.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/reverse/008-reverse-08/log-000.module-uri.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/reverse/008-reverse-08/log-000.member.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/reverse/008-reverse-08/log-000.member.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/reverse/008-reverse-08/log-000.position.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/reverse/008-reverse-08/log-000.position.code-units.bin"
                        ) as &[u8],
                    ),
                ]],
            },
            Row {
                id: "reverse-09",
                expression: r########"sibling.typedFixed"########,
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/reverse/009-reverse-09/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/reverse/009-reverse-09/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/reverse/009-reverse-09/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/reverse/009-reverse-09/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "reverse-10",
                expression: r########"child.localRead"########,
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/reverse/010-reverse-10/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/reverse/010-reverse-10/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/reverse/010-reverse-10/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/reverse/010-reverse-10/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[[
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/reverse/010-reverse-10/log-000.level.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/reverse/010-reverse-10/log-000.level.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/reverse/010-reverse-10/log-000.message.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/reverse/010-reverse-10/log-000.message.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/reverse/010-reverse-10/log-000.module-uri.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/reverse/010-reverse-10/log-000.module-uri.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/reverse/010-reverse-10/log-000.member.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/reverse/010-reverse-10/log-000.member.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/reverse/010-reverse-10/log-000.position.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/reverse/010-reverse-10/log-000.position.code-units.bin"
                        ) as &[u8],
                    ),
                ]],
            },
            Row {
                id: "reverse-11",
                expression: r########"base.localRead"########,
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/reverse/011-reverse-11/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/reverse/011-reverse-11/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/reverse/011-reverse-11/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/reverse/011-reverse-11/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[[
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/reverse/011-reverse-11/log-000.level.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/reverse/011-reverse-11/log-000.level.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/reverse/011-reverse-11/log-000.message.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/reverse/011-reverse-11/log-000.message.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/reverse/011-reverse-11/log-000.module-uri.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/reverse/011-reverse-11/log-000.module-uri.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/reverse/011-reverse-11/log-000.member.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/reverse/011-reverse-11/log-000.member.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/reverse/011-reverse-11/log-000.position.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-const-owner/reverse/011-reverse-11/log-000.position.code-units.bin"
                        ) as &[u8],
                    ),
                ]],
            },
            Row {
                id: "reverse-12",
                expression: r########"sibling.localRead"########,
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/reverse/012-reverse-12/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/reverse/012-reverse-12/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/reverse/012-reverse-12/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/reverse/012-reverse-12/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "reverse-13",
                expression: r########"child.localValue"########,
                kind: "error",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/reverse/013-reverse-13/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/reverse/013-reverse-13/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/reverse/013-reverse-13/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/reverse/013-reverse-13/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "reverse-14",
                expression: r########"base.localValue"########,
                kind: "error",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/reverse/014-reverse-14/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/reverse/014-reverse-14/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/reverse/014-reverse-14/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/reverse/014-reverse-14/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "reverse-15",
                expression: r########"child.marker"########,
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/reverse/015-reverse-15/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/reverse/015-reverse-15/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/reverse/015-reverse-15/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-const-owner/reverse/015-reverse-15/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
        ],
    );
}
