//! Pinned Java 0.32.0 every(): exact UTF-16 result and raw UTF-8/UTF-16 logger fields.
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
fn java_every_results_and_logger_fields() {
    let rows = &[
        Row {
            id: "list-empty",
            expression: r########"List().every((x) -> false)"########,
            kind: "value",
            expected: (
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/000-list-empty-result.stdout.bin"
                ) as &[u8],
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/000-list-empty-result.stdout.code-units.bin"
                ) as &[u8],
            ),
            logs: &[],
        },
        Row {
            id: "list-all",
            expression: r########"List(1, 2, 3).every((x) -> x > 0)"########,
            kind: "value",
            expected: (
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/001-list-all-result.stdout.bin"
                ) as &[u8],
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/001-list-all-result.stdout.code-units.bin"
                ) as &[u8],
            ),
            logs: &[],
        },
        Row {
            id: "list-order-all",
            expression: r########"List(3, 1, 2).every((x) -> trace(x) > 0)"########,
            kind: "value",
            expected: (
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/002-list-order-all-result.stdout.bin"
                ) as &[u8],
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/002-list-order-all-result.stdout.code-units.bin"
                ) as &[u8],
            ),
            logs: &[
                [
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/002-list-order-all-log-000.level.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/002-list-order-all-log-000.level.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/002-list-order-all-log-000.message.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/002-list-order-all-log-000.message.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/002-list-order-all-log-000.module-uri.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/002-list-order-all-log-000.module-uri.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/002-list-order-all-log-000.member.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/002-list-order-all-log-000.member.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/002-list-order-all-log-000.position.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/002-list-order-all-log-000.position.code-units.bin"
                        ) as &[u8],
                    ),
                ],
                [
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/002-list-order-all-log-001.level.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/002-list-order-all-log-001.level.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/002-list-order-all-log-001.message.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/002-list-order-all-log-001.message.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/002-list-order-all-log-001.module-uri.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/002-list-order-all-log-001.module-uri.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/002-list-order-all-log-001.member.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/002-list-order-all-log-001.member.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/002-list-order-all-log-001.position.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/002-list-order-all-log-001.position.code-units.bin"
                        ) as &[u8],
                    ),
                ],
                [
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/002-list-order-all-log-002.level.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/002-list-order-all-log-002.level.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/002-list-order-all-log-002.message.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/002-list-order-all-log-002.message.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/002-list-order-all-log-002.module-uri.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/002-list-order-all-log-002.module-uri.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/002-list-order-all-log-002.member.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/002-list-order-all-log-002.member.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/002-list-order-all-log-002.position.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/002-list-order-all-log-002.position.code-units.bin"
                        ) as &[u8],
                    ),
                ],
            ],
        },
        Row {
            id: "list-order-stop",
            expression: r########"List(3, 1, 2, 4).every((x) -> trace(x) != 2)"########,
            kind: "value",
            expected: (
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/003-list-order-stop-result.stdout.bin"
                ) as &[u8],
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/003-list-order-stop-result.stdout.code-units.bin"
                ) as &[u8],
            ),
            logs: &[
                [
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/003-list-order-stop-log-000.level.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/003-list-order-stop-log-000.level.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/003-list-order-stop-log-000.message.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/003-list-order-stop-log-000.message.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/003-list-order-stop-log-000.module-uri.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/003-list-order-stop-log-000.module-uri.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/003-list-order-stop-log-000.member.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/003-list-order-stop-log-000.member.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/003-list-order-stop-log-000.position.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/003-list-order-stop-log-000.position.code-units.bin"
                        ) as &[u8],
                    ),
                ],
                [
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/003-list-order-stop-log-001.level.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/003-list-order-stop-log-001.level.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/003-list-order-stop-log-001.message.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/003-list-order-stop-log-001.message.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/003-list-order-stop-log-001.module-uri.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/003-list-order-stop-log-001.module-uri.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/003-list-order-stop-log-001.member.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/003-list-order-stop-log-001.member.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/003-list-order-stop-log-001.position.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/003-list-order-stop-log-001.position.code-units.bin"
                        ) as &[u8],
                    ),
                ],
                [
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/003-list-order-stop-log-002.level.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/003-list-order-stop-log-002.level.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/003-list-order-stop-log-002.message.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/003-list-order-stop-log-002.message.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/003-list-order-stop-log-002.module-uri.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/003-list-order-stop-log-002.module-uri.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/003-list-order-stop-log-002.member.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/003-list-order-stop-log-002.member.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/003-list-order-stop-log-002.position.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/003-list-order-stop-log-002.position.code-units.bin"
                        ) as &[u8],
                    ),
                ],
            ],
        },
        Row {
            id: "list-later-body",
            expression: r########"List(1, 2, 3).every((x) -> x != 2 && (x != 3 || throw("later body")))"########,
            kind: "value",
            expected: (
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/004-list-later-body-result.stdout.bin"
                ) as &[u8],
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/004-list-later-body-result.stdout.code-units.bin"
                ) as &[u8],
            ),
            logs: &[],
        },
        Row {
            id: "list-duplicate",
            expression: r########"List(2, 1, 2).every((x) -> trace(x) > 0)"########,
            kind: "value",
            expected: (
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/005-list-duplicate-result.stdout.bin"
                ) as &[u8],
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/005-list-duplicate-result.stdout.code-units.bin"
                ) as &[u8],
            ),
            logs: &[
                [
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/005-list-duplicate-log-000.level.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/005-list-duplicate-log-000.level.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/005-list-duplicate-log-000.message.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/005-list-duplicate-log-000.message.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/005-list-duplicate-log-000.module-uri.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/005-list-duplicate-log-000.module-uri.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/005-list-duplicate-log-000.member.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/005-list-duplicate-log-000.member.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/005-list-duplicate-log-000.position.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/005-list-duplicate-log-000.position.code-units.bin"
                        ) as &[u8],
                    ),
                ],
                [
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/005-list-duplicate-log-001.level.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/005-list-duplicate-log-001.level.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/005-list-duplicate-log-001.message.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/005-list-duplicate-log-001.message.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/005-list-duplicate-log-001.module-uri.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/005-list-duplicate-log-001.module-uri.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/005-list-duplicate-log-001.member.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/005-list-duplicate-log-001.member.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/005-list-duplicate-log-001.position.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/005-list-duplicate-log-001.position.code-units.bin"
                        ) as &[u8],
                    ),
                ],
                [
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/005-list-duplicate-log-002.level.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/005-list-duplicate-log-002.level.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/005-list-duplicate-log-002.message.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/005-list-duplicate-log-002.message.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/005-list-duplicate-log-002.module-uri.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/005-list-duplicate-log-002.module-uri.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/005-list-duplicate-log-002.member.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/005-list-duplicate-log-002.member.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/005-list-duplicate-log-002.position.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/005-list-duplicate-log-002.position.code-units.bin"
                        ) as &[u8],
                    ),
                ],
            ],
        },
        Row {
            id: "list-nonboolean",
            expression: r########"List(1).every((x) -> 42)"########,
            kind: "error",
            expected: (
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/006-list-nonboolean-result.stderr.bin"
                ) as &[u8],
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/006-list-nonboolean-result.stderr.code-units.bin"
                ) as &[u8],
            ),
            logs: &[],
        },
        Row {
            id: "list-empty-nonboolean",
            expression: r########"List().every((x) -> 42)"########,
            kind: "value",
            expected: (
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/007-list-empty-nonboolean-result.stdout.bin"
                ) as &[u8],
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/007-list-empty-nonboolean-result.stdout.code-units.bin"
                ) as &[u8],
            ),
            logs: &[],
        },
        Row {
            id: "list-arity-two",
            expression: r########"List(1).every((x, y) -> true)"########,
            kind: "error",
            expected: (
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/008-list-arity-two-result.stderr.bin"
                ) as &[u8],
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/008-list-arity-two-result.stderr.code-units.bin"
                ) as &[u8],
            ),
            logs: &[],
        },
        Row {
            id: "list-empty-arity-two",
            expression: r########"List().every((x, y) -> true)"########,
            kind: "error",
            expected: (
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/009-list-empty-arity-two-result.stderr.bin"
                ) as &[u8],
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/009-list-empty-arity-two-result.stderr.code-units.bin"
                ) as &[u8],
            ),
            logs: &[],
        },
        Row {
            id: "list-parameter-constraint",
            expression: r########"List(1).every((x: Int(this > 1)) -> true)"########,
            kind: "error",
            expected: (
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/010-list-parameter-constraint-result.stderr.bin"
                ) as &[u8],
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/010-list-parameter-constraint-result.stderr.code-units.bin"
                ) as &[u8],
            ),
            logs: &[],
        },
        Row {
            id: "list-callback-throw",
            expression: r########"List(1).every((x) -> throw("predicate"))"########,
            kind: "error",
            expected: (
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/011-list-callback-throw-result.stderr.bin"
                ) as &[u8],
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/011-list-callback-throw-result.stderr.code-units.bin"
                ) as &[u8],
            ),
            logs: &[],
        },
        Row {
            id: "list-empty-invalid",
            expression: r########"List().every(1)"########,
            kind: "error",
            expected: (
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/012-list-empty-invalid-result.stderr.bin"
                ) as &[u8],
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/012-list-empty-invalid-result.stderr.code-units.bin"
                ) as &[u8],
            ),
            logs: &[],
        },
        Row {
            id: "list-invalid",
            expression: r########"List(1).every(1)"########,
            kind: "error",
            expected: (
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/013-list-invalid-result.stderr.bin"
                ) as &[u8],
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/013-list-invalid-result.stderr.code-units.bin"
                ) as &[u8],
            ),
            logs: &[],
        },
        Row {
            id: "list-argument-throw",
            expression: r########"List(1).every(throw("argument first"))"########,
            kind: "error",
            expected: (
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/014-list-argument-throw-result.stderr.bin"
                ) as &[u8],
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/014-list-argument-throw-result.stderr.code-units.bin"
                ) as &[u8],
            ),
            logs: &[],
        },
        Row {
            id: "list-property",
            expression: r########"List(1).every"########,
            kind: "error",
            expected: (
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/015-list-property-result.stderr.bin"
                ) as &[u8],
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/015-list-property-result.stderr.code-units.bin"
                ) as &[u8],
            ),
            logs: &[],
        },
        Row {
            id: "set-empty",
            expression: r########"Set().every((x) -> false)"########,
            kind: "value",
            expected: (
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/016-set-empty-result.stdout.bin"
                ) as &[u8],
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/016-set-empty-result.stdout.code-units.bin"
                ) as &[u8],
            ),
            logs: &[],
        },
        Row {
            id: "set-all",
            expression: r########"Set(1, 2, 3).every((x) -> x > 0)"########,
            kind: "value",
            expected: (
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/017-set-all-result.stdout.bin"
                ) as &[u8],
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/017-set-all-result.stdout.code-units.bin"
                ) as &[u8],
            ),
            logs: &[],
        },
        Row {
            id: "set-order",
            expression: r########"Set(3, 1, 2).every((x) -> trace(x) > 0)"########,
            kind: "value",
            expected: (
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/018-set-order-result.stdout.bin"
                ) as &[u8],
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/018-set-order-result.stdout.code-units.bin"
                ) as &[u8],
            ),
            logs: &[
                [
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/018-set-order-log-000.level.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/018-set-order-log-000.level.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/018-set-order-log-000.message.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/018-set-order-log-000.message.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/018-set-order-log-000.module-uri.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/018-set-order-log-000.module-uri.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/018-set-order-log-000.member.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/018-set-order-log-000.member.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/018-set-order-log-000.position.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/018-set-order-log-000.position.code-units.bin"
                        ) as &[u8],
                    ),
                ],
                [
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/018-set-order-log-001.level.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/018-set-order-log-001.level.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/018-set-order-log-001.message.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/018-set-order-log-001.message.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/018-set-order-log-001.module-uri.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/018-set-order-log-001.module-uri.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/018-set-order-log-001.member.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/018-set-order-log-001.member.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/018-set-order-log-001.position.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/018-set-order-log-001.position.code-units.bin"
                        ) as &[u8],
                    ),
                ],
                [
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/018-set-order-log-002.level.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/018-set-order-log-002.level.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/018-set-order-log-002.message.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/018-set-order-log-002.message.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/018-set-order-log-002.module-uri.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/018-set-order-log-002.module-uri.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/018-set-order-log-002.member.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/018-set-order-log-002.member.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/018-set-order-log-002.position.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/018-set-order-log-002.position.code-units.bin"
                        ) as &[u8],
                    ),
                ],
            ],
        },
        Row {
            id: "set-stop",
            expression: r########"Set(3, 1, 2, 4).every((x) -> trace(x) != 2)"########,
            kind: "value",
            expected: (
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/019-set-stop-result.stdout.bin"
                ) as &[u8],
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/019-set-stop-result.stdout.code-units.bin"
                ) as &[u8],
            ),
            logs: &[
                [
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/019-set-stop-log-000.level.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/019-set-stop-log-000.level.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/019-set-stop-log-000.message.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/019-set-stop-log-000.message.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/019-set-stop-log-000.module-uri.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/019-set-stop-log-000.module-uri.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/019-set-stop-log-000.member.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/019-set-stop-log-000.member.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/019-set-stop-log-000.position.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/019-set-stop-log-000.position.code-units.bin"
                        ) as &[u8],
                    ),
                ],
                [
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/019-set-stop-log-001.level.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/019-set-stop-log-001.level.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/019-set-stop-log-001.message.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/019-set-stop-log-001.message.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/019-set-stop-log-001.module-uri.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/019-set-stop-log-001.module-uri.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/019-set-stop-log-001.member.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/019-set-stop-log-001.member.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/019-set-stop-log-001.position.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/019-set-stop-log-001.position.code-units.bin"
                        ) as &[u8],
                    ),
                ],
                [
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/019-set-stop-log-002.level.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/019-set-stop-log-002.level.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/019-set-stop-log-002.message.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/019-set-stop-log-002.message.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/019-set-stop-log-002.module-uri.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/019-set-stop-log-002.module-uri.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/019-set-stop-log-002.member.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/019-set-stop-log-002.member.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/019-set-stop-log-002.position.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/019-set-stop-log-002.position.code-units.bin"
                        ) as &[u8],
                    ),
                ],
            ],
        },
        Row {
            id: "set-later-body",
            expression: r########"Set(1, 2, 3).every((x) -> x != 2 && (x != 3 || throw("later body")))"########,
            kind: "value",
            expected: (
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/020-set-later-body-result.stdout.bin"
                ) as &[u8],
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/020-set-later-body-result.stdout.code-units.bin"
                ) as &[u8],
            ),
            logs: &[],
        },
        Row {
            id: "set-nonboolean",
            expression: r########"Set(1).every((x) -> 42)"########,
            kind: "error",
            expected: (
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/021-set-nonboolean-result.stderr.bin"
                ) as &[u8],
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/021-set-nonboolean-result.stderr.code-units.bin"
                ) as &[u8],
            ),
            logs: &[],
        },
        Row {
            id: "set-empty-nonboolean",
            expression: r########"Set().every((x) -> 42)"########,
            kind: "value",
            expected: (
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/022-set-empty-nonboolean-result.stdout.bin"
                ) as &[u8],
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/022-set-empty-nonboolean-result.stdout.code-units.bin"
                ) as &[u8],
            ),
            logs: &[],
        },
        Row {
            id: "set-arity-two",
            expression: r########"Set(1).every((x, y) -> true)"########,
            kind: "error",
            expected: (
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/023-set-arity-two-result.stderr.bin"
                ) as &[u8],
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/023-set-arity-two-result.stderr.code-units.bin"
                ) as &[u8],
            ),
            logs: &[],
        },
        Row {
            id: "set-empty-arity-two",
            expression: r########"Set().every((x, y) -> true)"########,
            kind: "error",
            expected: (
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/024-set-empty-arity-two-result.stderr.bin"
                ) as &[u8],
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/024-set-empty-arity-two-result.stderr.code-units.bin"
                ) as &[u8],
            ),
            logs: &[],
        },
        Row {
            id: "set-parameter-constraint",
            expression: r########"Set(1).every((x: Int(this > 1)) -> true)"########,
            kind: "error",
            expected: (
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/025-set-parameter-constraint-result.stderr.bin"
                ) as &[u8],
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/025-set-parameter-constraint-result.stderr.code-units.bin"
                ) as &[u8],
            ),
            logs: &[],
        },
        Row {
            id: "set-empty-invalid",
            expression: r########"Set().every(1)"########,
            kind: "error",
            expected: (
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/026-set-empty-invalid-result.stderr.bin"
                ) as &[u8],
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/026-set-empty-invalid-result.stderr.code-units.bin"
                ) as &[u8],
            ),
            logs: &[],
        },
        Row {
            id: "set-duplicate",
            expression: r########"Set(2, 1, 2, 3).every((x) -> trace(x) > 0)"########,
            kind: "value",
            expected: (
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/027-set-duplicate-result.stdout.bin"
                ) as &[u8],
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/027-set-duplicate-result.stdout.code-units.bin"
                ) as &[u8],
            ),
            logs: &[
                [
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/027-set-duplicate-log-000.level.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/027-set-duplicate-log-000.level.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/027-set-duplicate-log-000.message.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/027-set-duplicate-log-000.message.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/027-set-duplicate-log-000.module-uri.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/027-set-duplicate-log-000.module-uri.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/027-set-duplicate-log-000.member.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/027-set-duplicate-log-000.member.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/027-set-duplicate-log-000.position.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/027-set-duplicate-log-000.position.code-units.bin"
                        ) as &[u8],
                    ),
                ],
                [
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/027-set-duplicate-log-001.level.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/027-set-duplicate-log-001.level.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/027-set-duplicate-log-001.message.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/027-set-duplicate-log-001.message.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/027-set-duplicate-log-001.module-uri.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/027-set-duplicate-log-001.module-uri.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/027-set-duplicate-log-001.member.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/027-set-duplicate-log-001.member.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/027-set-duplicate-log-001.position.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/027-set-duplicate-log-001.position.code-units.bin"
                        ) as &[u8],
                    ),
                ],
                [
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/027-set-duplicate-log-002.level.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/027-set-duplicate-log-002.level.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/027-set-duplicate-log-002.message.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/027-set-duplicate-log-002.message.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/027-set-duplicate-log-002.module-uri.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/027-set-duplicate-log-002.module-uri.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/027-set-duplicate-log-002.member.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/027-set-duplicate-log-002.member.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/027-set-duplicate-log-002.position.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/027-set-duplicate-log-002.position.code-units.bin"
                        ) as &[u8],
                    ),
                ],
            ],
        },
        Row {
            id: "map-empty",
            expression: r########"Map().every((k, v) -> false)"########,
            kind: "value",
            expected: (
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/028-map-empty-result.stdout.bin"
                ) as &[u8],
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/028-map-empty-result.stdout.code-units.bin"
                ) as &[u8],
            ),
            logs: &[],
        },
        Row {
            id: "map-all",
            expression: r########"Map("a", 1, "b", 2).every((k, v) -> v > 0)"########,
            kind: "value",
            expected: (
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/029-map-all-result.stdout.bin"
                ) as &[u8],
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/029-map-all-result.stdout.code-units.bin"
                ) as &[u8],
            ),
            logs: &[],
        },
        Row {
            id: "map-keys-order",
            expression: r########"Map("c", 3, "a", 1, "b", 2).every((k, v) -> trace(k) != "never")"########,
            kind: "value",
            expected: (
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/030-map-keys-order-result.stdout.bin"
                ) as &[u8],
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/030-map-keys-order-result.stdout.code-units.bin"
                ) as &[u8],
            ),
            logs: &[
                [
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/030-map-keys-order-log-000.level.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/030-map-keys-order-log-000.level.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/030-map-keys-order-log-000.message.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/030-map-keys-order-log-000.message.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/030-map-keys-order-log-000.module-uri.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/030-map-keys-order-log-000.module-uri.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/030-map-keys-order-log-000.member.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/030-map-keys-order-log-000.member.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/030-map-keys-order-log-000.position.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/030-map-keys-order-log-000.position.code-units.bin"
                        ) as &[u8],
                    ),
                ],
                [
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/030-map-keys-order-log-001.level.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/030-map-keys-order-log-001.level.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/030-map-keys-order-log-001.message.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/030-map-keys-order-log-001.message.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/030-map-keys-order-log-001.module-uri.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/030-map-keys-order-log-001.module-uri.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/030-map-keys-order-log-001.member.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/030-map-keys-order-log-001.member.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/030-map-keys-order-log-001.position.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/030-map-keys-order-log-001.position.code-units.bin"
                        ) as &[u8],
                    ),
                ],
                [
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/030-map-keys-order-log-002.level.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/030-map-keys-order-log-002.level.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/030-map-keys-order-log-002.message.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/030-map-keys-order-log-002.message.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/030-map-keys-order-log-002.module-uri.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/030-map-keys-order-log-002.module-uri.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/030-map-keys-order-log-002.member.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/030-map-keys-order-log-002.member.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/030-map-keys-order-log-002.position.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/030-map-keys-order-log-002.position.code-units.bin"
                        ) as &[u8],
                    ),
                ],
            ],
        },
        Row {
            id: "map-values-order",
            expression: r########"Map("c", 3, "a", 1, "b", 2).every((k, v) -> trace(v) > 0)"########,
            kind: "value",
            expected: (
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/031-map-values-order-result.stdout.bin"
                ) as &[u8],
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/031-map-values-order-result.stdout.code-units.bin"
                ) as &[u8],
            ),
            logs: &[
                [
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/031-map-values-order-log-000.level.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/031-map-values-order-log-000.level.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/031-map-values-order-log-000.message.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/031-map-values-order-log-000.message.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/031-map-values-order-log-000.module-uri.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/031-map-values-order-log-000.module-uri.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/031-map-values-order-log-000.member.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/031-map-values-order-log-000.member.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/031-map-values-order-log-000.position.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/031-map-values-order-log-000.position.code-units.bin"
                        ) as &[u8],
                    ),
                ],
                [
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/031-map-values-order-log-001.level.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/031-map-values-order-log-001.level.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/031-map-values-order-log-001.message.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/031-map-values-order-log-001.message.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/031-map-values-order-log-001.module-uri.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/031-map-values-order-log-001.module-uri.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/031-map-values-order-log-001.member.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/031-map-values-order-log-001.member.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/031-map-values-order-log-001.position.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/031-map-values-order-log-001.position.code-units.bin"
                        ) as &[u8],
                    ),
                ],
                [
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/031-map-values-order-log-002.level.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/031-map-values-order-log-002.level.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/031-map-values-order-log-002.message.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/031-map-values-order-log-002.message.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/031-map-values-order-log-002.module-uri.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/031-map-values-order-log-002.module-uri.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/031-map-values-order-log-002.member.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/031-map-values-order-log-002.member.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/031-map-values-order-log-002.position.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/031-map-values-order-log-002.position.code-units.bin"
                        ) as &[u8],
                    ),
                ],
            ],
        },
        Row {
            id: "map-stop",
            expression: r########"Map("c", 3, "a", 1, "b", 2).every((k, v) -> trace(k) != "b")"########,
            kind: "value",
            expected: (
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/032-map-stop-result.stdout.bin"
                ) as &[u8],
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/032-map-stop-result.stdout.code-units.bin"
                ) as &[u8],
            ),
            logs: &[
                [
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/032-map-stop-log-000.level.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/032-map-stop-log-000.level.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/032-map-stop-log-000.message.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/032-map-stop-log-000.message.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/032-map-stop-log-000.module-uri.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/032-map-stop-log-000.module-uri.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/032-map-stop-log-000.member.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/032-map-stop-log-000.member.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/032-map-stop-log-000.position.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/032-map-stop-log-000.position.code-units.bin"
                        ) as &[u8],
                    ),
                ],
                [
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/032-map-stop-log-001.level.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/032-map-stop-log-001.level.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/032-map-stop-log-001.message.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/032-map-stop-log-001.message.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/032-map-stop-log-001.module-uri.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/032-map-stop-log-001.module-uri.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/032-map-stop-log-001.member.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/032-map-stop-log-001.member.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/032-map-stop-log-001.position.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/032-map-stop-log-001.position.code-units.bin"
                        ) as &[u8],
                    ),
                ],
                [
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/032-map-stop-log-002.level.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/032-map-stop-log-002.level.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/032-map-stop-log-002.message.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/032-map-stop-log-002.message.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/032-map-stop-log-002.module-uri.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/032-map-stop-log-002.module-uri.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/032-map-stop-log-002.member.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/032-map-stop-log-002.member.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/032-map-stop-log-002.position.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/032-map-stop-log-002.position.code-units.bin"
                        ) as &[u8],
                    ),
                ],
            ],
        },
        Row {
            id: "map-later-body",
            expression: r########"Map("a", 1, "b", 2, "c", 3).every((k, v) -> v != 2 && (v != 3 || throw("later body")))"########,
            kind: "value",
            expected: (
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/033-map-later-body-result.stdout.bin"
                ) as &[u8],
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/033-map-later-body-result.stdout.code-units.bin"
                ) as &[u8],
            ),
            logs: &[],
        },
        Row {
            id: "map-nonboolean",
            expression: r########"Map("a", 1).every((k, v) -> 42)"########,
            kind: "error",
            expected: (
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/034-map-nonboolean-result.stderr.bin"
                ) as &[u8],
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/034-map-nonboolean-result.stderr.code-units.bin"
                ) as &[u8],
            ),
            logs: &[],
        },
        Row {
            id: "map-empty-nonboolean",
            expression: r########"Map().every((k, v) -> 42)"########,
            kind: "value",
            expected: (
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/035-map-empty-nonboolean-result.stdout.bin"
                ) as &[u8],
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/035-map-empty-nonboolean-result.stdout.code-units.bin"
                ) as &[u8],
            ),
            logs: &[],
        },
        Row {
            id: "map-arity-one",
            expression: r########"Map("a", 1).every((k) -> true)"########,
            kind: "error",
            expected: (
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/036-map-arity-one-result.stderr.bin"
                ) as &[u8],
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/036-map-arity-one-result.stderr.code-units.bin"
                ) as &[u8],
            ),
            logs: &[],
        },
        Row {
            id: "map-empty-arity-one",
            expression: r########"Map().every((k) -> true)"########,
            kind: "error",
            expected: (
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/037-map-empty-arity-one-result.stderr.bin"
                ) as &[u8],
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/037-map-empty-arity-one-result.stderr.code-units.bin"
                ) as &[u8],
            ),
            logs: &[],
        },
        Row {
            id: "map-arity-three",
            expression: r########"Map("a", 1).every((k, v, extra) -> true)"########,
            kind: "error",
            expected: (
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/038-map-arity-three-result.stderr.bin"
                ) as &[u8],
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/038-map-arity-three-result.stderr.code-units.bin"
                ) as &[u8],
            ),
            logs: &[],
        },
        Row {
            id: "map-empty-arity-three",
            expression: r########"Map().every((k, v, extra) -> true)"########,
            kind: "error",
            expected: (
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/039-map-empty-arity-three-result.stderr.bin"
                ) as &[u8],
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/039-map-empty-arity-three-result.stderr.code-units.bin"
                ) as &[u8],
            ),
            logs: &[],
        },
        Row {
            id: "map-parameter-constraint",
            expression: r########"Map("a", 1).every((k, v: Int(this > 1)) -> true)"########,
            kind: "error",
            expected: (
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/040-map-parameter-constraint-result.stderr.bin"
                ) as &[u8],
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/040-map-parameter-constraint-result.stderr.code-units.bin"
                ) as &[u8],
            ),
            logs: &[],
        },
        Row {
            id: "map-empty-invalid",
            expression: r########"Map().every(1)"########,
            kind: "error",
            expected: (
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/041-map-empty-invalid-result.stderr.bin"
                ) as &[u8],
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/041-map-empty-invalid-result.stderr.code-units.bin"
                ) as &[u8],
            ),
            logs: &[],
        },
        Row {
            id: "map-duplicate",
            expression: r########"Map("a", 1, "b", 2, "a", 3).every((k, v) -> trace(v) > 0)"########,
            kind: "value",
            expected: (
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/042-map-duplicate-result.stdout.bin"
                ) as &[u8],
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/042-map-duplicate-result.stdout.code-units.bin"
                ) as &[u8],
            ),
            logs: &[
                [
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/042-map-duplicate-log-000.level.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/042-map-duplicate-log-000.level.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/042-map-duplicate-log-000.message.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/042-map-duplicate-log-000.message.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/042-map-duplicate-log-000.module-uri.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/042-map-duplicate-log-000.module-uri.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/042-map-duplicate-log-000.member.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/042-map-duplicate-log-000.member.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/042-map-duplicate-log-000.position.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/042-map-duplicate-log-000.position.code-units.bin"
                        ) as &[u8],
                    ),
                ],
                [
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/042-map-duplicate-log-001.level.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/042-map-duplicate-log-001.level.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/042-map-duplicate-log-001.message.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/042-map-duplicate-log-001.message.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/042-map-duplicate-log-001.module-uri.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/042-map-duplicate-log-001.module-uri.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/042-map-duplicate-log-001.member.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/042-map-duplicate-log-001.member.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/042-map-duplicate-log-001.position.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/042-map-duplicate-log-001.position.code-units.bin"
                        ) as &[u8],
                    ),
                ],
            ],
        },
        Row {
            id: "listing-empty",
            expression: r########"(new Listing {}).every((x) -> false)"########,
            kind: "value",
            expected: (
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/043-listing-empty-result.stdout.bin"
                ) as &[u8],
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/043-listing-empty-result.stdout.code-units.bin"
                ) as &[u8],
            ),
            logs: &[],
        },
        Row {
            id: "listing-stop-lazy",
            expression: r########"(new Listing { 1; 2; throw("later value") }).every((x) -> trace(x) < 2)"########,
            kind: "value",
            expected: (
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/044-listing-stop-lazy-result.stdout.bin"
                ) as &[u8],
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/044-listing-stop-lazy-result.stdout.code-units.bin"
                ) as &[u8],
            ),
            logs: &[
                [
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/044-listing-stop-lazy-log-000.level.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/044-listing-stop-lazy-log-000.level.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/044-listing-stop-lazy-log-000.message.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/044-listing-stop-lazy-log-000.message.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/044-listing-stop-lazy-log-000.module-uri.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/044-listing-stop-lazy-log-000.module-uri.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/044-listing-stop-lazy-log-000.member.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/044-listing-stop-lazy-log-000.member.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/044-listing-stop-lazy-log-000.position.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/044-listing-stop-lazy-log-000.position.code-units.bin"
                        ) as &[u8],
                    ),
                ],
                [
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/044-listing-stop-lazy-log-001.level.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/044-listing-stop-lazy-log-001.level.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/044-listing-stop-lazy-log-001.message.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/044-listing-stop-lazy-log-001.message.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/044-listing-stop-lazy-log-001.module-uri.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/044-listing-stop-lazy-log-001.module-uri.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/044-listing-stop-lazy-log-001.member.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/044-listing-stop-lazy-log-001.member.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/044-listing-stop-lazy-log-001.position.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/044-listing-stop-lazy-log-001.position.code-units.bin"
                        ) as &[u8],
                    ),
                ],
            ],
        },
        Row {
            id: "listing-order",
            expression: r########"(new Listing { 3; 1; 2 }).every((x) -> trace(x) > 0)"########,
            kind: "value",
            expected: (
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/045-listing-order-result.stdout.bin"
                ) as &[u8],
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/045-listing-order-result.stdout.code-units.bin"
                ) as &[u8],
            ),
            logs: &[
                [
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/045-listing-order-log-000.level.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/045-listing-order-log-000.level.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/045-listing-order-log-000.message.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/045-listing-order-log-000.message.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/045-listing-order-log-000.module-uri.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/045-listing-order-log-000.module-uri.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/045-listing-order-log-000.member.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/045-listing-order-log-000.member.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/045-listing-order-log-000.position.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/045-listing-order-log-000.position.code-units.bin"
                        ) as &[u8],
                    ),
                ],
                [
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/045-listing-order-log-001.level.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/045-listing-order-log-001.level.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/045-listing-order-log-001.message.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/045-listing-order-log-001.message.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/045-listing-order-log-001.module-uri.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/045-listing-order-log-001.module-uri.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/045-listing-order-log-001.member.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/045-listing-order-log-001.member.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/045-listing-order-log-001.position.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/045-listing-order-log-001.position.code-units.bin"
                        ) as &[u8],
                    ),
                ],
                [
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/045-listing-order-log-002.level.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/045-listing-order-log-002.level.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/045-listing-order-log-002.message.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/045-listing-order-log-002.message.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/045-listing-order-log-002.module-uri.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/045-listing-order-log-002.module-uri.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/045-listing-order-log-002.member.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/045-listing-order-log-002.member.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/045-listing-order-log-002.position.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/045-listing-order-log-002.position.code-units.bin"
                        ) as &[u8],
                    ),
                ],
            ],
        },
        Row {
            id: "mapping-empty",
            expression: r########"(new Mapping {}).every((k, v) -> false)"########,
            kind: "value",
            expected: (
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/046-mapping-empty-result.stdout.bin"
                ) as &[u8],
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/046-mapping-empty-result.stdout.code-units.bin"
                ) as &[u8],
            ),
            logs: &[],
        },
        Row {
            id: "mapping-stop-lazy",
            expression: r########"(new Mapping { ["a"] = 1; ["b"] = 2; ["c"] = throw("later value") }).every((k, v) -> trace(v) < 2)"########,
            kind: "value",
            expected: (
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/047-mapping-stop-lazy-result.stdout.bin"
                ) as &[u8],
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/047-mapping-stop-lazy-result.stdout.code-units.bin"
                ) as &[u8],
            ),
            logs: &[
                [
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/047-mapping-stop-lazy-log-000.level.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/047-mapping-stop-lazy-log-000.level.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/047-mapping-stop-lazy-log-000.message.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/047-mapping-stop-lazy-log-000.message.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/047-mapping-stop-lazy-log-000.module-uri.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/047-mapping-stop-lazy-log-000.module-uri.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/047-mapping-stop-lazy-log-000.member.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/047-mapping-stop-lazy-log-000.member.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/047-mapping-stop-lazy-log-000.position.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/047-mapping-stop-lazy-log-000.position.code-units.bin"
                        ) as &[u8],
                    ),
                ],
                [
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/047-mapping-stop-lazy-log-001.level.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/047-mapping-stop-lazy-log-001.level.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/047-mapping-stop-lazy-log-001.message.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/047-mapping-stop-lazy-log-001.message.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/047-mapping-stop-lazy-log-001.module-uri.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/047-mapping-stop-lazy-log-001.module-uri.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/047-mapping-stop-lazy-log-001.member.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/047-mapping-stop-lazy-log-001.member.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/047-mapping-stop-lazy-log-001.position.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/047-mapping-stop-lazy-log-001.position.code-units.bin"
                        ) as &[u8],
                    ),
                ],
            ],
        },
        Row {
            id: "mapping-order",
            expression: r########"(new Mapping { ["c"] = 3; ["a"] = 1; ["b"] = 2 }).every((k, v) -> trace(k) != "never")"########,
            kind: "value",
            expected: (
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/048-mapping-order-result.stdout.bin"
                ) as &[u8],
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/048-mapping-order-result.stdout.code-units.bin"
                ) as &[u8],
            ),
            logs: &[
                [
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/048-mapping-order-log-000.level.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/048-mapping-order-log-000.level.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/048-mapping-order-log-000.message.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/048-mapping-order-log-000.message.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/048-mapping-order-log-000.module-uri.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/048-mapping-order-log-000.module-uri.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/048-mapping-order-log-000.member.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/048-mapping-order-log-000.member.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/048-mapping-order-log-000.position.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/048-mapping-order-log-000.position.code-units.bin"
                        ) as &[u8],
                    ),
                ],
                [
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/048-mapping-order-log-001.level.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/048-mapping-order-log-001.level.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/048-mapping-order-log-001.message.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/048-mapping-order-log-001.message.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/048-mapping-order-log-001.module-uri.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/048-mapping-order-log-001.module-uri.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/048-mapping-order-log-001.member.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/048-mapping-order-log-001.member.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/048-mapping-order-log-001.position.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/048-mapping-order-log-001.position.code-units.bin"
                        ) as &[u8],
                    ),
                ],
                [
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/048-mapping-order-log-002.level.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/048-mapping-order-log-002.level.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/048-mapping-order-log-002.message.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/048-mapping-order-log-002.message.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/048-mapping-order-log-002.module-uri.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/048-mapping-order-log-002.module-uri.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/048-mapping-order-log-002.member.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/048-mapping-order-log-002.member.code-units.bin"
                        ) as &[u8],
                    ),
                    (
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/048-mapping-order-log-002.position.bin"
                        ) as &[u8],
                        include_bytes!(
                            "../../../tests/fixtures/m2-collection-every/expected/048-mapping-order-log-002.position.code-units.bin"
                        ) as &[u8],
                    ),
                ],
            ],
        },
        Row {
            id: "list-type-constraint",
            expression: r########"List(1, 2) is List(every((x) -> x > 0))"########,
            kind: "value",
            expected: (
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/049-list-type-constraint-result.stdout.bin"
                ) as &[u8],
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/049-list-type-constraint-result.stdout.code-units.bin"
                ) as &[u8],
            ),
            logs: &[],
        },
        Row {
            id: "set-type-constraint",
            expression: r########"Set(1, 2) is Set(every((x) -> x > 0))"########,
            kind: "value",
            expected: (
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/050-set-type-constraint-result.stdout.bin"
                ) as &[u8],
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/050-set-type-constraint-result.stdout.code-units.bin"
                ) as &[u8],
            ),
            logs: &[],
        },
        Row {
            id: "map-type-constraint",
            expression: r########"Map("a", 1) is Map(every((k, v) -> v > 0))"########,
            kind: "value",
            expected: (
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/051-map-type-constraint-result.stdout.bin"
                ) as &[u8],
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/051-map-type-constraint-result.stdout.code-units.bin"
                ) as &[u8],
            ),
            logs: &[],
        },
        Row {
            id: "wrong-receiver",
            expression: r########"(1).every((x) -> true)"########,
            kind: "error",
            expected: (
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/052-wrong-receiver-result.stderr.bin"
                ) as &[u8],
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/052-wrong-receiver-result.stderr.code-units.bin"
                ) as &[u8],
            ),
            logs: &[],
        },
        Row {
            id: "list-type-constraint-false",
            expression: r########"List(1, -2) is List(every((x) -> x > 0))"########,
            kind: "value",
            expected: (
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/053-list-type-constraint-false-result.stdout.bin"
                ) as &[u8],
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/053-list-type-constraint-false-result.stdout.code-units.bin"
                ) as &[u8],
            ),
            logs: &[],
        },
        Row {
            id: "set-type-constraint-false",
            expression: r########"Set(1, -2) is Set(every((x) -> x > 0))"########,
            kind: "value",
            expected: (
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/054-set-type-constraint-false-result.stdout.bin"
                ) as &[u8],
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/054-set-type-constraint-false-result.stdout.code-units.bin"
                ) as &[u8],
            ),
            logs: &[],
        },
        Row {
            id: "map-type-constraint-false",
            expression: r########"Map("a", 1, "b", -2) is Map(every((k, v) -> v > 0))"########,
            kind: "value",
            expected: (
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/055-map-type-constraint-false-result.stdout.bin"
                ) as &[u8],
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/055-map-type-constraint-false-result.stdout.code-units.bin"
                ) as &[u8],
            ),
            logs: &[],
        },
        Row {
            id: "listing-empty-arity-two",
            expression: r########"(new Listing {}).every((x, y) -> true)"########,
            kind: "error",
            expected: (
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/056-listing-empty-arity-two-result.stderr.bin"
                ) as &[u8],
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/056-listing-empty-arity-two-result.stderr.code-units.bin"
                ) as &[u8],
            ),
            logs: &[],
        },
        Row {
            id: "listing-arity-two",
            expression: r########"(new Listing { 1 }).every((x, y) -> true)"########,
            kind: "error",
            expected: (
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/057-listing-arity-two-result.stderr.bin"
                ) as &[u8],
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/057-listing-arity-two-result.stderr.code-units.bin"
                ) as &[u8],
            ),
            logs: &[],
        },
        Row {
            id: "listing-empty-nonboolean",
            expression: r########"(new Listing {}).every((x) -> 42)"########,
            kind: "value",
            expected: (
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/058-listing-empty-nonboolean-result.stdout.bin"
                ) as &[u8],
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/058-listing-empty-nonboolean-result.stdout.code-units.bin"
                ) as &[u8],
            ),
            logs: &[],
        },
        Row {
            id: "listing-nonboolean",
            expression: r########"(new Listing { 1 }).every((x) -> 42)"########,
            kind: "error",
            expected: (
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/059-listing-nonboolean-result.stderr.bin"
                ) as &[u8],
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/059-listing-nonboolean-result.stderr.code-units.bin"
                ) as &[u8],
            ),
            logs: &[],
        },
        Row {
            id: "mapping-empty-arity-one",
            expression: r########"(new Mapping {}).every((k) -> true)"########,
            kind: "error",
            expected: (
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/060-mapping-empty-arity-one-result.stderr.bin"
                ) as &[u8],
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/060-mapping-empty-arity-one-result.stderr.code-units.bin"
                ) as &[u8],
            ),
            logs: &[],
        },
        Row {
            id: "mapping-arity-one",
            expression: r########"(new Mapping { ["a"] = 1 }).every((k) -> true)"########,
            kind: "error",
            expected: (
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/061-mapping-arity-one-result.stderr.bin"
                ) as &[u8],
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/061-mapping-arity-one-result.stderr.code-units.bin"
                ) as &[u8],
            ),
            logs: &[],
        },
        Row {
            id: "mapping-empty-nonboolean",
            expression: r########"(new Mapping {}).every((k, v) -> 42)"########,
            kind: "value",
            expected: (
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/062-mapping-empty-nonboolean-result.stdout.bin"
                ) as &[u8],
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/062-mapping-empty-nonboolean-result.stdout.code-units.bin"
                ) as &[u8],
            ),
            logs: &[],
        },
        Row {
            id: "mapping-nonboolean",
            expression: r########"(new Mapping { ["a"] = 1 }).every((k, v) -> 42)"########,
            kind: "error",
            expected: (
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/063-mapping-nonboolean-result.stderr.bin"
                ) as &[u8],
                include_bytes!(
                    "../../../tests/fixtures/m2-collection-every/expected/063-mapping-nonboolean-result.stderr.code-units.bin"
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
                "file:///stage68/collection-every.pkl",
                Some(include_str!(
                    "../../../tests/fixtures/m2-collection-every/module.pkl"
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
        let event = terminal.expect("every poll bound");
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
