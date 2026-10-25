//! Pinned Java 0.32.0 super-entry defaults: exact UTF-16 result and raw UTF-8/UTF-16 logger fields.
#[path = "support/module_keys.rs"]
mod module_keys;
use pkl_runtime::{probe::SourceProbe, value::PklString};
type RawField = (&'static [u8], &'static [u8]);
struct Row {
    id: &'static str,
    expression: &'static str,
    kind: &'static str,
    expected: RawField,
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
#[test]
fn java_super_entry_results_and_logger_fields() {
    let rows = &[
        Row {
            id: "plain-list-zero",
            expression: r########"plainListZero"########,
            kind: "value",
            expected: (
                include_bytes!(
                    "../../../tests/fixtures/m2-super-entry-defaults/expected/000-plain-list-zero-result.stdout.bin"
                ) as &[u8],
                include_bytes!(
                    "../../../tests/fixtures/m2-super-entry-defaults/expected/000-plain-list-zero-result.stdout.code-units.bin"
                ) as &[u8],
            ),
            logs: &[],
        },
        Row {
            id: "plain-list-three",
            expression: r########"plainListThree"########,
            kind: "value",
            expected: (
                include_bytes!(
                    "../../../tests/fixtures/m2-super-entry-defaults/expected/001-plain-list-three-result.stdout.bin"
                ) as &[u8],
                include_bytes!(
                    "../../../tests/fixtures/m2-super-entry-defaults/expected/001-plain-list-three-result.stdout.code-units.bin"
                ) as &[u8],
            ),
            logs: &[],
        },
        Row {
            id: "plain-mapping",
            expression: r########"plainMapping"########,
            kind: "value",
            expected: (
                include_bytes!(
                    "../../../tests/fixtures/m2-super-entry-defaults/expected/002-plain-mapping-result.stdout.bin"
                ) as &[u8],
                include_bytes!(
                    "../../../tests/fixtures/m2-super-entry-defaults/expected/002-plain-mapping-result.stdout.code-units.bin"
                ) as &[u8],
            ),
            logs: &[],
        },
        Row {
            id: "receiver-list",
            expression: r########"receiverList"########,
            kind: "value",
            expected: (
                include_bytes!(
                    "../../../tests/fixtures/m2-super-entry-defaults/expected/003-receiver-list-result.stdout.bin"
                ) as &[u8],
                include_bytes!(
                    "../../../tests/fixtures/m2-super-entry-defaults/expected/003-receiver-list-result.stdout.code-units.bin"
                ) as &[u8],
            ),
            logs: &[],
        },
        Row {
            id: "receiver-list-negative",
            expression: r########"receiverListNegative"########,
            kind: "value",
            expected: (
                include_bytes!(
                    "../../../tests/fixtures/m2-super-entry-defaults/expected/004-receiver-list-negative-result.stdout.bin"
                ) as &[u8],
                include_bytes!(
                    "../../../tests/fixtures/m2-super-entry-defaults/expected/004-receiver-list-negative-result.stdout.code-units.bin"
                ) as &[u8],
            ),
            logs: &[],
        },
        Row {
            id: "receiver-list-ordinary-missing",
            expression: r########"receiverListOrdinaryMissing"########,
            kind: "error",
            expected: (
                include_bytes!(
                    "../../../tests/fixtures/m2-super-entry-defaults/expected/005-receiver-list-ordinary-missing-result.stderr.bin"
                ) as &[u8],
                include_bytes!(
                    "../../../tests/fixtures/m2-super-entry-defaults/expected/005-receiver-list-ordinary-missing-result.stderr.code-units.bin"
                ) as &[u8],
            ),
            logs: &[],
        },
        Row {
            id: "present-list",
            expression: r########"presentList"########,
            kind: "value",
            expected: (
                include_bytes!(
                    "../../../tests/fixtures/m2-super-entry-defaults/expected/006-present-list-result.stdout.bin"
                ) as &[u8],
                include_bytes!(
                    "../../../tests/fixtures/m2-super-entry-defaults/expected/006-present-list-result.stdout.code-units.bin"
                ) as &[u8],
            ),
            logs: &[],
        },
        Row {
            id: "parent-list",
            expression: r########"parentList"########,
            kind: "value",
            expected: (
                include_bytes!(
                    "../../../tests/fixtures/m2-super-entry-defaults/expected/007-parent-list-result.stdout.bin"
                ) as &[u8],
                include_bytes!(
                    "../../../tests/fixtures/m2-super-entry-defaults/expected/007-parent-list-result.stdout.code-units.bin"
                ) as &[u8],
            ),
            logs: &[],
        },
        Row {
            id: "receiver-mapping",
            expression: r########"receiverMapping"########,
            kind: "value",
            expected: (
                include_bytes!(
                    "../../../tests/fixtures/m2-super-entry-defaults/expected/008-receiver-mapping-result.stdout.bin"
                ) as &[u8],
                include_bytes!(
                    "../../../tests/fixtures/m2-super-entry-defaults/expected/008-receiver-mapping-result.stdout.code-units.bin"
                ) as &[u8],
            ),
            logs: &[],
        },
        Row {
            id: "receiver-mapping-ordinary-missing",
            expression: r########"receiverMappingOrdinaryMissing"########,
            kind: "error",
            expected: (
                include_bytes!(
                    "../../../tests/fixtures/m2-super-entry-defaults/expected/009-receiver-mapping-ordinary-missing-result.stderr.bin"
                ) as &[u8],
                include_bytes!(
                    "../../../tests/fixtures/m2-super-entry-defaults/expected/009-receiver-mapping-ordinary-missing-result.stderr.code-units.bin"
                ) as &[u8],
            ),
            logs: &[],
        },
        Row {
            id: "present-mapping",
            expression: r########"presentMapping"########,
            kind: "value",
            expected: (
                include_bytes!(
                    "../../../tests/fixtures/m2-super-entry-defaults/expected/010-present-mapping-result.stdout.bin"
                ) as &[u8],
                include_bytes!(
                    "../../../tests/fixtures/m2-super-entry-defaults/expected/010-present-mapping-result.stdout.code-units.bin"
                ) as &[u8],
            ),
            logs: &[],
        },
        Row {
            id: "key-error",
            expression: r########"keyError"########,
            kind: "error",
            expected: (
                include_bytes!(
                    "../../../tests/fixtures/m2-super-entry-defaults/expected/011-key-error-result.stderr.bin"
                ) as &[u8],
                include_bytes!(
                    "../../../tests/fixtures/m2-super-entry-defaults/expected/011-key-error-result.stderr.code-units.bin"
                ) as &[u8],
            ),
            logs: &[],
        },
        Row {
            id: "default-error",
            expression: r########"defaultError"########,
            kind: "error",
            expected: (
                include_bytes!(
                    "../../../tests/fixtures/m2-super-entry-defaults/expected/012-default-error-result.stderr.bin"
                ) as &[u8],
                include_bytes!(
                    "../../../tests/fixtures/m2-super-entry-defaults/expected/012-default-error-result.stderr.code-units.bin"
                ) as &[u8],
            ),
            logs: &[],
        },
        Row {
            id: "lazy-missing",
            expression: r########"lazyMissing"########,
            kind: "value",
            expected: (
                include_bytes!(
                    "../../../tests/fixtures/m2-super-entry-defaults/expected/013-lazy-missing-result.stdout.bin"
                ) as &[u8],
                include_bytes!(
                    "../../../tests/fixtures/m2-super-entry-defaults/expected/013-lazy-missing-result.stdout.code-units.bin"
                ) as &[u8],
            ),
            logs: &[],
        },
        Row {
            id: "lazy-present",
            expression: r########"lazyPresent"########,
            kind: "error",
            expected: (
                include_bytes!(
                    "../../../tests/fixtures/m2-super-entry-defaults/expected/014-lazy-present-result.stderr.bin"
                ) as &[u8],
                include_bytes!(
                    "../../../tests/fixtures/m2-super-entry-defaults/expected/014-lazy-present-result.stderr.code-units.bin"
                ) as &[u8],
            ),
            logs: &[],
        },
        Row {
            id: "wrong-arity",
            expression: r########"wrongArity"########,
            kind: "error",
            expected: (
                include_bytes!(
                    "../../../tests/fixtures/m2-super-entry-defaults/expected/015-wrong-arity-result.stderr.bin"
                ) as &[u8],
                include_bytes!(
                    "../../../tests/fixtures/m2-super-entry-defaults/expected/015-wrong-arity-result.stderr.code-units.bin"
                ) as &[u8],
            ),
            logs: &[],
        },
        Row {
            id: "wrong-value",
            expression: r########"wrongValue"########,
            kind: "error",
            expected: (
                include_bytes!(
                    "../../../tests/fixtures/m2-super-entry-defaults/expected/016-wrong-value-result.stderr.bin"
                ) as &[u8],
                include_bytes!(
                    "../../../tests/fixtures/m2-super-entry-defaults/expected/016-wrong-value-result.stderr.code-units.bin"
                ) as &[u8],
            ),
            logs: &[],
        },
        Row {
            id: "null-default",
            expression: r########"nullDefault"########,
            kind: "error",
            expected: (
                include_bytes!(
                    "../../../tests/fixtures/m2-super-entry-defaults/expected/017-null-default-result.stderr.bin"
                ) as &[u8],
                include_bytes!(
                    "../../../tests/fixtures/m2-super-entry-defaults/expected/017-null-default-result.stderr.code-units.bin"
                ) as &[u8],
            ),
            logs: &[],
        },
        Row {
            id: "trace-missing",
            expression: r########"traceMissing"########,
            kind: "value",
            expected: (
                include_bytes!(
                    "../../../tests/fixtures/m2-super-entry-defaults/expected/018-trace-missing-result.stdout.bin"
                ) as &[u8],
                include_bytes!(
                    "../../../tests/fixtures/m2-super-entry-defaults/expected/018-trace-missing-result.stdout.code-units.bin"
                ) as &[u8],
            ),
            logs: &[
                [
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-super-entry-defaults/expected/018-trace-missing-log-000.level.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-super-entry-defaults/expected/018-trace-missing-log-000.level.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-super-entry-defaults/expected/018-trace-missing-log-000.message.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-super-entry-defaults/expected/018-trace-missing-log-000.message.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-super-entry-defaults/expected/018-trace-missing-log-000.module-uri.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-super-entry-defaults/expected/018-trace-missing-log-000.module-uri.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-super-entry-defaults/expected/018-trace-missing-log-000.member.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-super-entry-defaults/expected/018-trace-missing-log-000.member.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-super-entry-defaults/expected/018-trace-missing-log-000.position.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-super-entry-defaults/expected/018-trace-missing-log-000.position.code-units.bin"
                        ) as &[u8],
                    ),
                ],
                [
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-super-entry-defaults/expected/018-trace-missing-log-001.level.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-super-entry-defaults/expected/018-trace-missing-log-001.level.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-super-entry-defaults/expected/018-trace-missing-log-001.message.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-super-entry-defaults/expected/018-trace-missing-log-001.message.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-super-entry-defaults/expected/018-trace-missing-log-001.module-uri.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-super-entry-defaults/expected/018-trace-missing-log-001.module-uri.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-super-entry-defaults/expected/018-trace-missing-log-001.member.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-super-entry-defaults/expected/018-trace-missing-log-001.member.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-super-entry-defaults/expected/018-trace-missing-log-001.position.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-super-entry-defaults/expected/018-trace-missing-log-001.position.code-units.bin"
                        ) as &[u8],
                    ),
                ],
            ],
        },
        Row {
            id: "trace-present",
            expression: r########"tracePresent"########,
            kind: "value",
            expected: (
                include_bytes!(
                    "../../../tests/fixtures/m2-super-entry-defaults/expected/019-trace-present-result.stdout.bin"
                ) as &[u8],
                include_bytes!(
                    "../../../tests/fixtures/m2-super-entry-defaults/expected/019-trace-present-result.stdout.code-units.bin"
                ) as &[u8],
            ),
            logs: &[[
                (
                    include_bytes!(
                        "../../../tests/fixtures/m2-super-entry-defaults/expected/019-trace-present-log-000.level.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-super-entry-defaults/expected/019-trace-present-log-000.level.code-units.bin"
                    ) as &[u8],
                ),
                (
                    include_bytes!(
                        "../../../tests/fixtures/m2-super-entry-defaults/expected/019-trace-present-log-000.message.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-super-entry-defaults/expected/019-trace-present-log-000.message.code-units.bin"
                    ) as &[u8],
                ),
                (
                    include_bytes!(
                        "../../../tests/fixtures/m2-super-entry-defaults/expected/019-trace-present-log-000.module-uri.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-super-entry-defaults/expected/019-trace-present-log-000.module-uri.code-units.bin"
                    ) as &[u8],
                ),
                (
                    include_bytes!(
                        "../../../tests/fixtures/m2-super-entry-defaults/expected/019-trace-present-log-000.member.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-super-entry-defaults/expected/019-trace-present-log-000.member.code-units.bin"
                    ) as &[u8],
                ),
                (
                    include_bytes!(
                        "../../../tests/fixtures/m2-super-entry-defaults/expected/019-trace-present-log-000.position.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-super-entry-defaults/expected/019-trace-present-log-000.position.code-units.bin"
                    ) as &[u8],
                ),
            ]],
        },
        Row {
            id: "function-owner",
            expression: r########"functionOwner"########,
            kind: "value",
            expected: (
                include_bytes!(
                    "../../../tests/fixtures/m2-super-entry-defaults/expected/020-function-owner-result.stdout.bin"
                ) as &[u8],
                include_bytes!(
                    "../../../tests/fixtures/m2-super-entry-defaults/expected/020-function-owner-result.stdout.code-units.bin"
                ) as &[u8],
            ),
            logs: &[],
        },
        Row {
            id: "wrong-key",
            expression: r########"wrongKey"########,
            kind: "value",
            expected: (
                include_bytes!(
                    "../../../tests/fixtures/m2-super-entry-defaults/expected/021-wrong-key-result.stdout.bin"
                ) as &[u8],
                include_bytes!(
                    "../../../tests/fixtures/m2-super-entry-defaults/expected/021-wrong-key-result.stdout.code-units.bin"
                ) as &[u8],
            ),
            logs: &[],
        },
        Row {
            id: "object-key-empty",
            expression: r########"objectKeyEmpty"########,
            kind: "error",
            expected: (
                include_bytes!(
                    "../../../tests/fixtures/m2-super-entry-defaults/expected/022-object-key-empty-result.stderr.bin"
                ) as &[u8],
                include_bytes!(
                    "../../../tests/fixtures/m2-super-entry-defaults/expected/022-object-key-empty-result.stderr.code-units.bin"
                ) as &[u8],
            ),
            logs: &[],
        },
        Row {
            id: "object-key-small",
            expression: r########"objectKeySmall"########,
            kind: "error",
            expected: (
                include_bytes!(
                    "../../../tests/fixtures/m2-super-entry-defaults/expected/023-object-key-small-result.stderr.bin"
                ) as &[u8],
                include_bytes!(
                    "../../../tests/fixtures/m2-super-entry-defaults/expected/023-object-key-small-result.stderr.code-units.bin"
                ) as &[u8],
            ),
            logs: &[],
        },
        Row {
            id: "object-key-hashed",
            expression: r########"objectKeyHashed"########,
            kind: "error",
            expected: (
                include_bytes!(
                    "../../../tests/fixtures/m2-super-entry-defaults/expected/024-object-key-hashed-result.stderr.bin"
                ) as &[u8],
                include_bytes!(
                    "../../../tests/fixtures/m2-super-entry-defaults/expected/024-object-key-hashed-result.stderr.code-units.bin"
                ) as &[u8],
            ),
            logs: &[],
        },
        Row {
            id: "object-key-default-throw",
            expression: r########"objectKeyDefaultThrow"########,
            kind: "error",
            expected: (
                include_bytes!(
                    "../../../tests/fixtures/m2-super-entry-defaults/expected/025-object-key-default-throw-result.stderr.bin"
                ) as &[u8],
                include_bytes!(
                    "../../../tests/fixtures/m2-super-entry-defaults/expected/025-object-key-default-throw-result.stderr.code-units.bin"
                ) as &[u8],
            ),
            logs: &[],
        },
        Row {
            id: "object-key-callback-read",
            expression: r########"objectKeyCallbackRead"########,
            kind: "error",
            expected: (
                include_bytes!(
                    "../../../tests/fixtures/m2-super-entry-defaults/expected/026-object-key-callback-read-result.stderr.bin"
                ) as &[u8],
                include_bytes!(
                    "../../../tests/fixtures/m2-super-entry-defaults/expected/026-object-key-callback-read-result.stderr.code-units.bin"
                ) as &[u8],
            ),
            logs: &[],
        },
        Row {
            id: "object-key-constructed",
            expression: r########"objectKeyConstructed"########,
            kind: "value",
            expected: (
                include_bytes!(
                    "../../../tests/fixtures/m2-super-entry-defaults/expected/027-object-key-constructed-result.stdout.bin"
                ) as &[u8],
                include_bytes!(
                    "../../../tests/fixtures/m2-super-entry-defaults/expected/027-object-key-constructed-result.stdout.code-units.bin"
                ) as &[u8],
            ),
            logs: &[],
        },
        Row {
            id: "valid-object-callback",
            expression: r########"validObjectCallback"########,
            kind: "value",
            expected: (
                include_bytes!(
                    "../../../tests/fixtures/m2-super-entry-defaults/expected/028-valid-object-callback-result.stdout.bin"
                ) as &[u8],
                include_bytes!(
                    "../../../tests/fixtures/m2-super-entry-defaults/expected/028-valid-object-callback-result.stdout.code-units.bin"
                ) as &[u8],
            ),
            logs: &[],
        },
    ];
    let mut probe = SourceProbe::new_module_evaluator("compact", true, false, false).unwrap();
    let mut failures = Vec::new();
    for row in rows {
        probe
            .evaluate_module_source(
                "file:///stage69/super-entry-default.pkl",
                Some(include_str!(
                    "../../../tests/fixtures/m2-super-entry-defaults/module.pkl"
                )),
                row.expression,
                0,
                None,
            )
            .unwrap();
        let mut log_index = 0;
        let mut terminal = None;
        for _ in 0..200_000 {
            probe.collect();
            let event = probe.poll(0, 16).unwrap();
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
                if row.logs.get(log_index).is_none_or(|expected| {
                    expected
                        .iter()
                        .zip(actual.iter())
                        .any(|(expected, actual)| !matches(*expected, actual))
                }) {
                    failures.push(format!("{} log {}: {:?}", row.id, log_index, log));
                }
                log_index += 1;
                probe
                    .acknowledge_log(event.token.as_deref().unwrap(), None, 0)
                    .unwrap();
                continue;
            }
            terminal = Some(event);
            break;
        }
        let event = terminal.expect("super entry poll bound");
        let actual = if event.kind == "value" {
            event.value.as_ref()
        } else {
            event.error.as_ref()
        };
        if event.kind != row.kind
            || actual.is_none_or(|value| !matches(row.expected, value))
            || log_index != row.logs.len()
        {
            failures.push(format!(
                "{}: expected {} {:?} logs{}, got {} {:?} logs{}",
                row.id,
                row.kind,
                String::from_utf16_lossy(&units(row.expected.1)),
                row.logs.len(),
                event.kind,
                actual.map(|v| String::from_utf16_lossy(v)),
                log_index
            ));
        }
    }
    probe.close();
    assert_eq!(probe.collect().slots, 0);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
