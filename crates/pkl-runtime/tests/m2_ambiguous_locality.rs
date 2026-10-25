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
            let event = terminal.expect("bounded ambiguous-locality poll");
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
fn java_original_raw_results_and_logs() {
    run(
        include_str!("../../../tests/fixtures/m2-ambiguous-locality/original/module.pkl"),
        "file:///stage73/original.pkl",
        &[
            Row {
                id: "res2-branch",
                expression: r########"res2.prop.theBranch"########,
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-ambiguous-locality/original/000-res2-branch/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-ambiguous-locality/original/000-res2-branch/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-ambiguous-locality/original/000-res2-branch/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-ambiguous-locality/original/000-res2-branch/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "res3-branch",
                expression: r########"res3.prop.theBranch"########,
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-ambiguous-locality/original/001-res3-branch/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-ambiguous-locality/original/001-res3-branch/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-ambiguous-locality/original/001-res3-branch/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-ambiguous-locality/original/001-res3-branch/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "res2-public",
                expression: r########"res2.prop.qux"########,
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-ambiguous-locality/original/002-res2-public/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-ambiguous-locality/original/002-res2-public/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-ambiguous-locality/original/002-res2-public/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-ambiguous-locality/original/002-res2-public/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "res3-local",
                expression: r########"res3.prop.qux"########,
                kind: "error",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-ambiguous-locality/original/003-res3-local/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-ambiguous-locality/original/003-res3-local/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-ambiguous-locality/original/003-res3-local/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-ambiguous-locality/original/003-res3-local/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
        ],
    );
}

#[test]
fn java_local_first_raw_results_and_logs() {
    run(
        include_str!("../../../tests/fixtures/m2-ambiguous-locality/local-first/module.pkl"),
        "file:///stage73/local-first.pkl",
        &[
            Row {
                id: "local-only",
                expression: r########"localOnly.answer"########,
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-ambiguous-locality/local-first/000-local-only/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-ambiguous-locality/local-first/000-local-only/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-ambiguous-locality/local-first/000-local-only/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-ambiguous-locality/local-first/000-local-only/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[[
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-ambiguous-locality/local-first/000-local-only/log-000.level.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-ambiguous-locality/local-first/000-local-only/log-000.level.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-ambiguous-locality/local-first/000-local-only/log-000.message.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-ambiguous-locality/local-first/000-local-only/log-000.message.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-ambiguous-locality/local-first/000-local-only/log-000.module-uri.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-ambiguous-locality/local-first/000-local-only/log-000.module-uri.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-ambiguous-locality/local-first/000-local-only/log-000.member.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-ambiguous-locality/local-first/000-local-only/log-000.member.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-ambiguous-locality/local-first/000-local-only/log-000.position.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-ambiguous-locality/local-first/000-local-only/log-000.position.code-units.bin"
                        ) as &[u8],
                    ),
                ]],
            },
            Row {
                id: "public-only",
                expression: r########"publicOnly.answer"########,
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-ambiguous-locality/local-first/001-public-only/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-ambiguous-locality/local-first/001-public-only/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-ambiguous-locality/local-first/001-public-only/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-ambiguous-locality/local-first/001-public-only/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[[
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-ambiguous-locality/local-first/001-public-only/log-000.level.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-ambiguous-locality/local-first/001-public-only/log-000.level.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-ambiguous-locality/local-first/001-public-only/log-000.message.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-ambiguous-locality/local-first/001-public-only/log-000.message.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-ambiguous-locality/local-first/001-public-only/log-000.module-uri.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-ambiguous-locality/local-first/001-public-only/log-000.module-uri.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-ambiguous-locality/local-first/001-public-only/log-000.member.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-ambiguous-locality/local-first/001-public-only/log-000.member.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-ambiguous-locality/local-first/001-public-only/log-000.position.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-ambiguous-locality/local-first/001-public-only/log-000.position.code-units.bin"
                        ) as &[u8],
                    ),
                ]],
            },
            Row {
                id: "local-direct",
                expression: r########"localOnly.x"########,
                kind: "error",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-ambiguous-locality/local-first/002-local-direct/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-ambiguous-locality/local-first/002-local-direct/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-ambiguous-locality/local-first/002-local-direct/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-ambiguous-locality/local-first/002-local-direct/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "public-direct",
                expression: r########"publicOnly.x"########,
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-ambiguous-locality/local-first/003-public-direct/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-ambiguous-locality/local-first/003-public-direct/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-ambiguous-locality/local-first/003-public-direct/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-ambiguous-locality/local-first/003-public-direct/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "a-direct",
                expression: r########"a.child.x"########,
                kind: "error",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-ambiguous-locality/local-first/004-a-direct/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-ambiguous-locality/local-first/004-a-direct/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-ambiguous-locality/local-first/004-a-direct/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-ambiguous-locality/local-first/004-a-direct/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "b-direct",
                expression: r########"b.child.x"########,
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-ambiguous-locality/local-first/005-b-direct/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-ambiguous-locality/local-first/005-b-direct/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-ambiguous-locality/local-first/005-b-direct/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-ambiguous-locality/local-first/005-b-direct/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[[
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-ambiguous-locality/local-first/005-b-direct/log-000.level.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-ambiguous-locality/local-first/005-b-direct/log-000.level.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-ambiguous-locality/local-first/005-b-direct/log-000.message.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-ambiguous-locality/local-first/005-b-direct/log-000.message.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-ambiguous-locality/local-first/005-b-direct/log-000.module-uri.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-ambiguous-locality/local-first/005-b-direct/log-000.module-uri.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-ambiguous-locality/local-first/005-b-direct/log-000.member.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-ambiguous-locality/local-first/005-b-direct/log-000.member.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-ambiguous-locality/local-first/005-b-direct/log-000.position.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-ambiguous-locality/local-first/005-b-direct/log-000.position.code-units.bin"
                        ) as &[u8],
                    ),
                ]],
            },
            Row {
                id: "a-answer",
                expression: r########"a.child.answer"########,
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-ambiguous-locality/local-first/006-a-answer/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-ambiguous-locality/local-first/006-a-answer/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-ambiguous-locality/local-first/006-a-answer/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-ambiguous-locality/local-first/006-a-answer/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[[
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-ambiguous-locality/local-first/006-a-answer/log-000.level.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-ambiguous-locality/local-first/006-a-answer/log-000.level.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-ambiguous-locality/local-first/006-a-answer/log-000.message.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-ambiguous-locality/local-first/006-a-answer/log-000.message.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-ambiguous-locality/local-first/006-a-answer/log-000.module-uri.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-ambiguous-locality/local-first/006-a-answer/log-000.module-uri.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-ambiguous-locality/local-first/006-a-answer/log-000.member.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-ambiguous-locality/local-first/006-a-answer/log-000.member.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-ambiguous-locality/local-first/006-a-answer/log-000.position.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-ambiguous-locality/local-first/006-a-answer/log-000.position.code-units.bin"
                        ) as &[u8],
                    ),
                ]],
            },
            Row {
                id: "b-answer",
                expression: r########"b.child.answer"########,
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-ambiguous-locality/local-first/007-b-answer/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-ambiguous-locality/local-first/007-b-answer/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-ambiguous-locality/local-first/007-b-answer/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-ambiguous-locality/local-first/007-b-answer/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[[
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-ambiguous-locality/local-first/007-b-answer/log-000.level.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-ambiguous-locality/local-first/007-b-answer/log-000.level.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-ambiguous-locality/local-first/007-b-answer/log-000.message.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-ambiguous-locality/local-first/007-b-answer/log-000.message.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-ambiguous-locality/local-first/007-b-answer/log-000.module-uri.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-ambiguous-locality/local-first/007-b-answer/log-000.module-uri.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-ambiguous-locality/local-first/007-b-answer/log-000.member.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-ambiguous-locality/local-first/007-b-answer/log-000.member.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-ambiguous-locality/local-first/007-b-answer/log-000.position.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-ambiguous-locality/local-first/007-b-answer/log-000.position.code-units.bin"
                        ) as &[u8],
                    ),
                ]],
            },
            Row {
                id: "a-again",
                expression: r########"a.child.answer"########,
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-ambiguous-locality/local-first/008-a-again/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-ambiguous-locality/local-first/008-a-again/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-ambiguous-locality/local-first/008-a-again/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-ambiguous-locality/local-first/008-a-again/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "b-again",
                expression: r########"b.child.answer"########,
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-ambiguous-locality/local-first/009-b-again/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-ambiguous-locality/local-first/009-b-again/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-ambiguous-locality/local-first/009-b-again/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-ambiguous-locality/local-first/009-b-again/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
        ],
    );
}

#[test]
fn java_public_first_raw_results_and_logs() {
    run(
        include_str!("../../../tests/fixtures/m2-ambiguous-locality/public-first/module.pkl"),
        "file:///stage73/public-first.pkl",
        &[
            Row {
                id: "local-only",
                expression: r########"localOnly.answer"########,
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-ambiguous-locality/public-first/000-local-only/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-ambiguous-locality/public-first/000-local-only/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-ambiguous-locality/public-first/000-local-only/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-ambiguous-locality/public-first/000-local-only/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[[
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-ambiguous-locality/public-first/000-local-only/log-000.level.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-ambiguous-locality/public-first/000-local-only/log-000.level.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-ambiguous-locality/public-first/000-local-only/log-000.message.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-ambiguous-locality/public-first/000-local-only/log-000.message.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-ambiguous-locality/public-first/000-local-only/log-000.module-uri.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-ambiguous-locality/public-first/000-local-only/log-000.module-uri.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-ambiguous-locality/public-first/000-local-only/log-000.member.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-ambiguous-locality/public-first/000-local-only/log-000.member.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-ambiguous-locality/public-first/000-local-only/log-000.position.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-ambiguous-locality/public-first/000-local-only/log-000.position.code-units.bin"
                        ) as &[u8],
                    ),
                ]],
            },
            Row {
                id: "public-only",
                expression: r########"publicOnly.answer"########,
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-ambiguous-locality/public-first/001-public-only/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-ambiguous-locality/public-first/001-public-only/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-ambiguous-locality/public-first/001-public-only/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-ambiguous-locality/public-first/001-public-only/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[[
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-ambiguous-locality/public-first/001-public-only/log-000.level.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-ambiguous-locality/public-first/001-public-only/log-000.level.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-ambiguous-locality/public-first/001-public-only/log-000.message.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-ambiguous-locality/public-first/001-public-only/log-000.message.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-ambiguous-locality/public-first/001-public-only/log-000.module-uri.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-ambiguous-locality/public-first/001-public-only/log-000.module-uri.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-ambiguous-locality/public-first/001-public-only/log-000.member.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-ambiguous-locality/public-first/001-public-only/log-000.member.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-ambiguous-locality/public-first/001-public-only/log-000.position.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-ambiguous-locality/public-first/001-public-only/log-000.position.code-units.bin"
                        ) as &[u8],
                    ),
                ]],
            },
            Row {
                id: "local-direct",
                expression: r########"localOnly.x"########,
                kind: "error",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-ambiguous-locality/public-first/002-local-direct/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-ambiguous-locality/public-first/002-local-direct/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-ambiguous-locality/public-first/002-local-direct/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-ambiguous-locality/public-first/002-local-direct/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "public-direct",
                expression: r########"publicOnly.x"########,
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-ambiguous-locality/public-first/003-public-direct/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-ambiguous-locality/public-first/003-public-direct/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-ambiguous-locality/public-first/003-public-direct/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-ambiguous-locality/public-first/003-public-direct/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "a-direct",
                expression: r########"a.child.x"########,
                kind: "error",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-ambiguous-locality/public-first/004-a-direct/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-ambiguous-locality/public-first/004-a-direct/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-ambiguous-locality/public-first/004-a-direct/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-ambiguous-locality/public-first/004-a-direct/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "b-direct",
                expression: r########"b.child.x"########,
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-ambiguous-locality/public-first/005-b-direct/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-ambiguous-locality/public-first/005-b-direct/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-ambiguous-locality/public-first/005-b-direct/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-ambiguous-locality/public-first/005-b-direct/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[[
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-ambiguous-locality/public-first/005-b-direct/log-000.level.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-ambiguous-locality/public-first/005-b-direct/log-000.level.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-ambiguous-locality/public-first/005-b-direct/log-000.message.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-ambiguous-locality/public-first/005-b-direct/log-000.message.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-ambiguous-locality/public-first/005-b-direct/log-000.module-uri.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-ambiguous-locality/public-first/005-b-direct/log-000.module-uri.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-ambiguous-locality/public-first/005-b-direct/log-000.member.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-ambiguous-locality/public-first/005-b-direct/log-000.member.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-ambiguous-locality/public-first/005-b-direct/log-000.position.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-ambiguous-locality/public-first/005-b-direct/log-000.position.code-units.bin"
                        ) as &[u8],
                    ),
                ]],
            },
            Row {
                id: "b-answer",
                expression: r########"b.child.answer"########,
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-ambiguous-locality/public-first/006-b-answer/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-ambiguous-locality/public-first/006-b-answer/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-ambiguous-locality/public-first/006-b-answer/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-ambiguous-locality/public-first/006-b-answer/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "a-answer",
                expression: r########"a.child.answer"########,
                kind: "error",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-ambiguous-locality/public-first/007-a-answer/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-ambiguous-locality/public-first/007-a-answer/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-ambiguous-locality/public-first/007-a-answer/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-ambiguous-locality/public-first/007-a-answer/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "b-again",
                expression: r########"b.child.answer"########,
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-ambiguous-locality/public-first/008-b-again/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-ambiguous-locality/public-first/008-b-again/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-ambiguous-locality/public-first/008-b-again/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-ambiguous-locality/public-first/008-b-again/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "a-again",
                expression: r########"a.child.answer"########,
                kind: "error",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-ambiguous-locality/public-first/009-a-again/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-ambiguous-locality/public-first/009-a-again/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-ambiguous-locality/public-first/009-a-again/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-ambiguous-locality/public-first/009-a-again/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
        ],
    );
}
