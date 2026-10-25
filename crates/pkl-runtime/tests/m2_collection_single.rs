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
fn java_collection_single_all_raw_results_and_logs() {
    run(
        include_str!("../../../tests/fixtures/m2-collection-single/collection-single/module.pkl"),
        "file:///stage76/collection-single.pkl",
        &[
            Row {
                id: "list-empty-single",
                expression: "List().single",
                kind: "error",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-collection-single/collection-single/000-list-empty-single/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-collection-single/collection-single/000-list-empty-single/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-collection-single/collection-single/000-list-empty-single/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-collection-single/collection-single/000-list-empty-single/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "list-one-single",
                expression: "List(1).single",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-collection-single/collection-single/001-list-one-single/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-collection-single/collection-single/001-list-one-single/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-collection-single/collection-single/001-list-one-single/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-collection-single/collection-single/001-list-one-single/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "list-two-single",
                expression: "List(1, 2).single",
                kind: "error",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-collection-single/collection-single/002-list-two-single/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-collection-single/collection-single/002-list-two-single/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-collection-single/collection-single/002-list-two-single/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-collection-single/collection-single/002-list-two-single/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "list-empty-single-or-null",
                expression: "List().singleOrNull",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-collection-single/collection-single/003-list-empty-single-or-null/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-collection-single/collection-single/003-list-empty-single-or-null/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-collection-single/collection-single/003-list-empty-single-or-null/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-collection-single/collection-single/003-list-empty-single-or-null/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "list-one-single-or-null",
                expression: "List(1).singleOrNull",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-collection-single/collection-single/004-list-one-single-or-null/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-collection-single/collection-single/004-list-one-single-or-null/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-collection-single/collection-single/004-list-one-single-or-null/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-collection-single/collection-single/004-list-one-single-or-null/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "list-two-single-or-null",
                expression: "List(1, 2).singleOrNull",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-collection-single/collection-single/005-list-two-single-or-null/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-collection-single/collection-single/005-list-two-single-or-null/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-collection-single/collection-single/005-list-two-single-or-null/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-collection-single/collection-single/005-list-two-single-or-null/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "set-empty-single",
                expression: "Set().single",
                kind: "error",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-collection-single/collection-single/006-set-empty-single/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-collection-single/collection-single/006-set-empty-single/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-collection-single/collection-single/006-set-empty-single/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-collection-single/collection-single/006-set-empty-single/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "set-one-single",
                expression: "Set(1).single",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-collection-single/collection-single/007-set-one-single/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-collection-single/collection-single/007-set-one-single/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-collection-single/collection-single/007-set-one-single/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-collection-single/collection-single/007-set-one-single/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "set-two-single",
                expression: "Set(1, 2).single",
                kind: "error",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-collection-single/collection-single/008-set-two-single/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-collection-single/collection-single/008-set-two-single/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-collection-single/collection-single/008-set-two-single/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-collection-single/collection-single/008-set-two-single/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "set-two-single-or-null",
                expression: "Set(1, 2).singleOrNull",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-collection-single/collection-single/009-set-two-single-or-null/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-collection-single/collection-single/009-set-two-single-or-null/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-collection-single/collection-single/009-set-two-single-or-null/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-collection-single/collection-single/009-set-two-single-or-null/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "list-null-single",
                expression: "List(null).single",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-collection-single/collection-single/010-list-null-single/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-collection-single/collection-single/010-list-null-single/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-collection-single/collection-single/010-list-null-single/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-collection-single/collection-single/010-list-null-single/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "list-lazy-nested-nonforcing",
                expression: "List(new Dynamic { value = throw(\"nested\") }).single is Dynamic",
                kind: "value",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-collection-single/collection-single/011-list-lazy-nested-nonforcing/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-collection-single/collection-single/011-list-lazy-nested-nonforcing/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-collection-single/collection-single/011-list-lazy-nested-nonforcing/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-collection-single/collection-single/011-list-lazy-nested-nonforcing/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
            Row {
                id: "single-as-method-throwing-argument",
                expression: "List(1).single(throw(\"argument first\"))",
                kind: "error",
                stdout: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-collection-single/collection-single/012-single-as-method-throwing-argument/result.stdout.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-collection-single/collection-single/012-single-as-method-throwing-argument/result.stdout.code-units.bin"
                    ) as &[u8],
                ),
                stderr: (
                    include_bytes!(
                        "../../../tests/fixtures/m2-collection-single/collection-single/012-single-as-method-throwing-argument/result.stderr.bin"
                    ) as &[u8],
                    include_bytes!(
                        "../../../tests/fixtures/m2-collection-single/collection-single/012-single-as-method-throwing-argument/result.stderr.code-units.bin"
                    ) as &[u8],
                ),
                logs: &[],
            },
        ],
    );
}
