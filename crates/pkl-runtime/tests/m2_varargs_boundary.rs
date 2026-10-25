//! Exact independent Java VarArgs check/default/operand-error boundaries.
#[path = "support/module_keys.rs"]
mod module_keys;
use pkl_runtime::{probe::SourceProbe, value::PklString};
struct Row {
    id: &'static str,
    uri: &'static str,
    query: &'static str,
    source: &'static str,
    kind: &'static str,
    utf8: &'static [u8],
    utf16: &'static [u8],
}
fn units(bytes: &[u8]) -> Vec<u16> {
    let (pairs, tail) = bytes.as_chunks::<2>();
    assert!(tail.is_empty());
    pairs.iter().map(|pair| u16::from_be_bytes(*pair)).collect()
}
#[test]
fn java_varargs_checks_defaults_and_operand_order() {
    let rows = &[
        Row {
            id: "check-list-int",
            uri: "file:///stage70/varargs-boundary.pkl",
            query: "result",
            source: include_str!(
                "../../../tests/fixtures/m2-varargs-boundary/000-check-list-int.pkl"
            ),
            kind: "error",
            utf8: include_bytes!(
                "../../../tests/fixtures/m2-varargs-boundary/000-check-list-int.utf8.bin"
            ),
            utf16: include_bytes!(
                "../../../tests/fixtures/m2-varargs-boundary/000-check-list-int.utf16be.bin"
            ),
        },
        Row {
            id: "check-list-string",
            uri: "file:///stage70/varargs-boundary.pkl",
            query: "result",
            source: include_str!(
                "../../../tests/fixtures/m2-varargs-boundary/001-check-list-string.pkl"
            ),
            kind: "error",
            utf8: include_bytes!(
                "../../../tests/fixtures/m2-varargs-boundary/001-check-list-string.utf8.bin"
            ),
            utf16: include_bytes!(
                "../../../tests/fixtures/m2-varargs-boundary/001-check-list-string.utf16be.bin"
            ),
        },
        Row {
            id: "check-primitive",
            uri: "file:///stage70/varargs-boundary.pkl",
            query: "result",
            source: include_str!(
                "../../../tests/fixtures/m2-varargs-boundary/002-check-primitive.pkl"
            ),
            kind: "error",
            utf8: include_bytes!(
                "../../../tests/fixtures/m2-varargs-boundary/002-check-primitive.utf8.bin"
            ),
            utf16: include_bytes!(
                "../../../tests/fixtures/m2-varargs-boundary/002-check-primitive.utf16be.bin"
            ),
        },
        Row {
            id: "default-missing",
            uri: "file:///stage70/varargs-boundary.pkl",
            query: "result",
            source: include_str!(
                "../../../tests/fixtures/m2-varargs-boundary/003-default-missing.pkl"
            ),
            kind: "error",
            utf8: include_bytes!(
                "../../../tests/fixtures/m2-varargs-boundary/003-default-missing.utf8.bin"
            ),
            utf16: include_bytes!(
                "../../../tests/fixtures/m2-varargs-boundary/003-default-missing.utf16be.bin"
            ),
        },
        Row {
            id: "check-explicit-value",
            uri: "file:///stage70/varargs-boundary.pkl",
            query: "result",
            source: include_str!(
                "../../../tests/fixtures/m2-varargs-boundary/004-check-explicit-value.pkl"
            ),
            kind: "error",
            utf8: include_bytes!(
                "../../../tests/fixtures/m2-varargs-boundary/004-check-explicit-value.utf8.bin"
            ),
            utf16: include_bytes!(
                "../../../tests/fixtures/m2-varargs-boundary/004-check-explicit-value.utf16be.bin"
            ),
        },
        Row {
            id: "check-throwing-value",
            uri: "file:///stage70/varargs-boundary.pkl",
            query: "result",
            source: include_str!(
                "../../../tests/fixtures/m2-varargs-boundary/005-check-throwing-value.pkl"
            ),
            kind: "error",
            utf8: include_bytes!(
                "../../../tests/fixtures/m2-varargs-boundary/005-check-throwing-value.utf8.bin"
            ),
            utf16: include_bytes!(
                "../../../tests/fixtures/m2-varargs-boundary/005-check-throwing-value.utf16be.bin"
            ),
        },
        Row {
            id: "missing-argument",
            uri: "file:///stage70/varargs-boundary.pkl",
            query: "result",
            source: include_str!(
                "../../../tests/fixtures/m2-varargs-boundary/006-missing-argument.pkl"
            ),
            kind: "error",
            utf8: include_bytes!(
                "../../../tests/fixtures/m2-varargs-boundary/006-missing-argument.utf8.bin"
            ),
            utf16: include_bytes!(
                "../../../tests/fixtures/m2-varargs-boundary/006-missing-argument.utf16be.bin"
            ),
        },
        Row {
            id: "wrong-arity-with-missing",
            uri: "file:///stage70/varargs-boundary.pkl",
            query: "result",
            source: include_str!(
                "../../../tests/fixtures/m2-varargs-boundary/007-wrong-arity-with-missing.pkl"
            ),
            kind: "error",
            utf8: include_bytes!(
                "../../../tests/fixtures/m2-varargs-boundary/007-wrong-arity-with-missing.utf8.bin"
            ),
            utf16: include_bytes!(
                "../../../tests/fixtures/m2-varargs-boundary/007-wrong-arity-with-missing.utf16be.bin"
            ),
        },
        Row {
            id: "wrong-arity-known",
            uri: "file:///stage70/varargs-boundary.pkl",
            query: "result",
            source: include_str!(
                "../../../tests/fixtures/m2-varargs-boundary/008-wrong-arity-known.pkl"
            ),
            kind: "error",
            utf8: include_bytes!(
                "../../../tests/fixtures/m2-varargs-boundary/008-wrong-arity-known.utf8.bin"
            ),
            utf16: include_bytes!(
                "../../../tests/fixtures/m2-varargs-boundary/008-wrong-arity-known.utf16be.bin"
            ),
        },
        Row {
            id: "new-external-class",
            uri: "file:///stage70/varargs-boundary.pkl",
            query: "result",
            source: include_str!(
                "../../../tests/fixtures/m2-varargs-boundary/009-new-external-class.pkl"
            ),
            kind: "error",
            utf8: include_bytes!(
                "../../../tests/fixtures/m2-varargs-boundary/009-new-external-class.utf8.bin"
            ),
            utf16: include_bytes!(
                "../../../tests/fixtures/m2-varargs-boundary/009-new-external-class.utf16be.bin"
            ),
        },
        Row {
            id: "implicit-check",
            uri: "file:///stage70/varargs-boundary.pkl",
            query: "result",
            source: include_str!(
                "../../../tests/fixtures/m2-varargs-boundary/010-implicit-check.pkl"
            ),
            kind: "error",
            utf8: include_bytes!(
                "../../../tests/fixtures/m2-varargs-boundary/010-implicit-check.utf8.bin"
            ),
            utf16: include_bytes!(
                "../../../tests/fixtures/m2-varargs-boundary/010-implicit-check.utf16be.bin"
            ),
        },
        Row {
            id: "cast-throwing-operand",
            uri: "file:///stage70/varargs-cast-boundary.pkl",
            query: "result",
            source: include_str!(
                "../../../tests/fixtures/m2-varargs-boundary/011-cast-throwing-operand.pkl"
            ),
            kind: "error",
            utf8: include_bytes!(
                "../../../tests/fixtures/m2-varargs-boundary/011-cast-throwing-operand.utf8.bin"
            ),
            utf16: include_bytes!(
                "../../../tests/fixtures/m2-varargs-boundary/011-cast-throwing-operand.utf16be.bin"
            ),
        },
        Row {
            id: "is-throwing-operand",
            uri: "file:///stage70/varargs-cast-boundary.pkl",
            query: "result",
            source: include_str!(
                "../../../tests/fixtures/m2-varargs-boundary/012-is-throwing-operand.pkl"
            ),
            kind: "error",
            utf8: include_bytes!(
                "../../../tests/fixtures/m2-varargs-boundary/012-is-throwing-operand.utf8.bin"
            ),
            utf16: include_bytes!(
                "../../../tests/fixtures/m2-varargs-boundary/012-is-throwing-operand.utf16be.bin"
            ),
        },
        Row {
            id: "cast-list-value",
            uri: "file:///stage70/varargs-cast-boundary.pkl",
            query: "result",
            source: include_str!(
                "../../../tests/fixtures/m2-varargs-boundary/013-cast-list-value.pkl"
            ),
            kind: "error",
            utf8: include_bytes!(
                "../../../tests/fixtures/m2-varargs-boundary/013-cast-list-value.utf8.bin"
            ),
            utf16: include_bytes!(
                "../../../tests/fixtures/m2-varargs-boundary/013-cast-list-value.utf16be.bin"
            ),
        },
    ];
    let mut failures = Vec::new();
    for row in rows {
        // Each source has the captured virtual URI, supplied in memory. A fresh
        // evaluator prevents one module's cached source from replacing another.
        let mut probe = SourceProbe::new_module_evaluator("compact", true, false, false).unwrap();
        probe
            .evaluate_module_source(row.uri, Some(row.source), row.query, 0, None)
            .unwrap();
        let mut terminal = None;
        for _ in 0..100_000 {
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
            terminal = Some(event);
            break;
        }
        let event = terminal.expect("bounded import type evaluation");
        let actual = if event.kind == "value" {
            event.value.as_ref()
        } else {
            event.error.as_ref()
        };
        let expected = units(row.utf16);
        if event.kind != row.kind
            || actual.is_none_or(|value| {
                *value != expected
                    || PklString::from_utf16(value.clone())
                        .to_java_utf8()
                        .as_bytes()
                        != row.utf8
            })
        {
            failures.push(format!(
                "{}: expected {} {:?}, got {} {:?}",
                row.id,
                row.kind,
                String::from_utf16_lossy(&expected),
                event.kind,
                actual.map(|v| String::from_utf16_lossy(v))
            ));
        }
        probe.close();
        assert_eq!(
            probe.collect().slots,
            0,
            "{} close releases all roots",
            row.id
        );
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
