//! Exact independent Java import-clause/type-namespace boundaries.
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
fn java_import_clauses_are_types_but_import_values_are_not() {
    let rows = &[
        Row {
            id: "upstream-original",
            uri: "file:///stage70/import-type-oracle.pkl",
            query: "listing",
            source: include_str!(
                "../../../tests/fixtures/m2-import-type-clauses/000-upstream-original.pkl"
            ),
            kind: "error",
            utf8: include_bytes!(
                "../../../tests/fixtures/m2-import-type-clauses/000-upstream-original.utf8.bin"
            ),
            utf16: include_bytes!(
                "../../../tests/fixtures/m2-import-type-clauses/000-upstream-original.utf16be.bin"
            ),
        },
        Row {
            id: "clause-inferred",
            uri: "file:///stage70/import-type-oracle.pkl",
            query: "result",
            source: include_str!(
                "../../../tests/fixtures/m2-import-type-clauses/001-clause-inferred.pkl"
            ),
            kind: "value",
            utf8: include_bytes!(
                "../../../tests/fixtures/m2-import-type-clauses/001-clause-inferred.utf8.bin"
            ),
            utf16: include_bytes!(
                "../../../tests/fixtures/m2-import-type-clauses/001-clause-inferred.utf16be.bin"
            ),
        },
        Row {
            id: "clause-alias",
            uri: "file:///stage70/import-type-oracle.pkl",
            query: "result",
            source: include_str!(
                "../../../tests/fixtures/m2-import-type-clauses/002-clause-alias.pkl"
            ),
            kind: "value",
            utf8: include_bytes!(
                "../../../tests/fixtures/m2-import-type-clauses/002-clause-alias.utf8.bin"
            ),
            utf16: include_bytes!(
                "../../../tests/fixtures/m2-import-type-clauses/002-clause-alias.utf16be.bin"
            ),
        },
        Row {
            id: "local-expression",
            uri: "file:///stage70/import-type-oracle.pkl",
            query: "result",
            source: include_str!(
                "../../../tests/fixtures/m2-import-type-clauses/003-local-expression.pkl"
            ),
            kind: "error",
            utf8: include_bytes!(
                "../../../tests/fixtures/m2-import-type-clauses/003-local-expression.utf8.bin"
            ),
            utf16: include_bytes!(
                "../../../tests/fixtures/m2-import-type-clauses/003-local-expression.utf16be.bin"
            ),
        },
        Row {
            id: "plain-expression",
            uri: "file:///stage70/import-type-oracle.pkl",
            query: "result",
            source: include_str!(
                "../../../tests/fixtures/m2-import-type-clauses/004-plain-expression.pkl"
            ),
            kind: "error",
            utf8: include_bytes!(
                "../../../tests/fixtures/m2-import-type-clauses/004-plain-expression.utf8.bin"
            ),
            utf16: include_bytes!(
                "../../../tests/fixtures/m2-import-type-clauses/004-plain-expression.utf16be.bin"
            ),
        },
        Row {
            id: "const-local-expression",
            uri: "file:///stage70/import-type-oracle.pkl",
            query: "result",
            source: include_str!(
                "../../../tests/fixtures/m2-import-type-clauses/005-const-local-expression.pkl"
            ),
            kind: "error",
            utf8: include_bytes!(
                "../../../tests/fixtures/m2-import-type-clauses/005-const-local-expression.utf8.bin"
            ),
            utf16: include_bytes!(
                "../../../tests/fixtures/m2-import-type-clauses/005-const-local-expression.utf16be.bin"
            ),
        },
        Row {
            id: "ordinary-import-value",
            uri: "file:///stage70/import-type-oracle.pkl",
            query: "result",
            source: include_str!(
                "../../../tests/fixtures/m2-import-type-clauses/006-ordinary-import-value.pkl"
            ),
            kind: "value",
            utf8: include_bytes!(
                "../../../tests/fixtures/m2-import-type-clauses/006-ordinary-import-value.utf8.bin"
            ),
            utf16: include_bytes!(
                "../../../tests/fixtures/m2-import-type-clauses/006-ordinary-import-value.utf16be.bin"
            ),
        },
        Row {
            id: "function-wrapper",
            uri: "file:///stage70/import-type-oracle.pkl",
            query: "result",
            source: include_str!(
                "../../../tests/fixtures/m2-import-type-clauses/007-function-wrapper.pkl"
            ),
            kind: "error",
            utf8: include_bytes!(
                "../../../tests/fixtures/m2-import-type-clauses/007-function-wrapper.utf8.bin"
            ),
            utf16: include_bytes!(
                "../../../tests/fixtures/m2-import-type-clauses/007-function-wrapper.utf16be.bin"
            ),
        },
        Row {
            id: "condition-wrapper",
            uri: "file:///stage70/import-type-oracle.pkl",
            query: "result",
            source: include_str!(
                "../../../tests/fixtures/m2-import-type-clauses/008-condition-wrapper.pkl"
            ),
            kind: "error",
            utf8: include_bytes!(
                "../../../tests/fixtures/m2-import-type-clauses/008-condition-wrapper.utf8.bin"
            ),
            utf16: include_bytes!(
                "../../../tests/fixtures/m2-import-type-clauses/008-condition-wrapper.utf16be.bin"
            ),
        },
        Row {
            id: "unused-invalid",
            uri: "file:///stage70/import-type-oracle.pkl",
            query: "result",
            source: include_str!(
                "../../../tests/fixtures/m2-import-type-clauses/009-unused-invalid.pkl"
            ),
            kind: "value",
            utf8: include_bytes!(
                "../../../tests/fixtures/m2-import-type-clauses/009-unused-invalid.utf8.bin"
            ),
            utf16: include_bytes!(
                "../../../tests/fixtures/m2-import-type-clauses/009-unused-invalid.utf16be.bin"
            ),
        },
        Row {
            id: "missing-import",
            uri: "file:///stage70/import-type-oracle.pkl",
            query: "result",
            source: include_str!(
                "../../../tests/fixtures/m2-import-type-clauses/010-missing-import.pkl"
            ),
            kind: "error",
            utf8: include_bytes!(
                "../../../tests/fixtures/m2-import-type-clauses/010-missing-import.utf8.bin"
            ),
            utf16: include_bytes!(
                "../../../tests/fixtures/m2-import-type-clauses/010-missing-import.utf16be.bin"
            ),
        },
        Row {
            id: "missing-type",
            uri: "file:///stage70/import-type-oracle.pkl",
            query: "result",
            source: include_str!(
                "../../../tests/fixtures/m2-import-type-clauses/011-missing-type.pkl"
            ),
            kind: "error",
            utf8: include_bytes!(
                "../../../tests/fixtures/m2-import-type-clauses/011-missing-type.utf8.bin"
            ),
            utf16: include_bytes!(
                "../../../tests/fixtures/m2-import-type-clauses/011-missing-type.utf16be.bin"
            ),
        },
        Row {
            id: "non-type",
            uri: "file:///stage70/import-type-oracle.pkl",
            query: "result",
            source: include_str!("../../../tests/fixtures/m2-import-type-clauses/012-non-type.pkl"),
            kind: "error",
            utf8: include_bytes!(
                "../../../tests/fixtures/m2-import-type-clauses/012-non-type.utf8.bin"
            ),
            utf16: include_bytes!(
                "../../../tests/fixtures/m2-import-type-clauses/012-non-type.utf16be.bin"
            ),
        },
        Row {
            id: "alias-expression",
            uri: "file:///stage70/import-type-oracle.pkl",
            query: "result",
            source: include_str!(
                "../../../tests/fixtures/m2-import-type-clauses/013-alias-expression.pkl"
            ),
            kind: "error",
            utf8: include_bytes!(
                "../../../tests/fixtures/m2-import-type-clauses/013-alias-expression.utf8.bin"
            ),
            utf16: include_bytes!(
                "../../../tests/fixtures/m2-import-type-clauses/013-alias-expression.utf16be.bin"
            ),
        },
        Row {
            id: "alias-clause",
            uri: "file:///stage70/import-type-oracle.pkl",
            query: "result",
            source: include_str!(
                "../../../tests/fixtures/m2-import-type-clauses/014-alias-clause.pkl"
            ),
            kind: "value",
            utf8: include_bytes!(
                "../../../tests/fixtures/m2-import-type-clauses/014-alias-clause.utf8.bin"
            ),
            utf16: include_bytes!(
                "../../../tests/fixtures/m2-import-type-clauses/014-alias-clause.utf16be.bin"
            ),
        },
        Row {
            id: "generic-clause",
            uri: "file:///stage70/import-type-oracle.pkl",
            query: "result",
            source: include_str!(
                "../../../tests/fixtures/m2-import-type-clauses/015-generic-clause.pkl"
            ),
            kind: "value",
            utf8: include_bytes!(
                "../../../tests/fixtures/m2-import-type-clauses/015-generic-clause.utf8.bin"
            ),
            utf16: include_bytes!(
                "../../../tests/fixtures/m2-import-type-clauses/015-generic-clause.utf16be.bin"
            ),
        },
        Row {
            id: "nullable-expression",
            uri: "file:///stage70/import-type-oracle.pkl",
            query: "result",
            source: include_str!(
                "../../../tests/fixtures/m2-import-type-clauses/016-nullable-expression.pkl"
            ),
            kind: "error",
            utf8: include_bytes!(
                "../../../tests/fixtures/m2-import-type-clauses/016-nullable-expression.utf8.bin"
            ),
            utf16: include_bytes!(
                "../../../tests/fixtures/m2-import-type-clauses/016-nullable-expression.utf16be.bin"
            ),
        },
        Row {
            id: "qualifier-before-body",
            uri: "file:///stage70/import-type-oracle.pkl",
            query: "result",
            source: include_str!(
                "../../../tests/fixtures/m2-import-type-clauses/017-qualifier-before-body.pkl"
            ),
            kind: "error",
            utf8: include_bytes!(
                "../../../tests/fixtures/m2-import-type-clauses/017-qualifier-before-body.utf8.bin"
            ),
            utf16: include_bytes!(
                "../../../tests/fixtures/m2-import-type-clauses/017-qualifier-before-body.utf16be.bin"
            ),
        },
        Row {
            id: "qualifier-before-value",
            uri: "file:///stage70/import-type-oracle.pkl",
            query: "result",
            source: include_str!(
                "../../../tests/fixtures/m2-import-type-clauses/018-qualifier-before-value.pkl"
            ),
            kind: "error",
            utf8: include_bytes!(
                "../../../tests/fixtures/m2-import-type-clauses/018-qualifier-before-value.utf8.bin"
            ),
            utf16: include_bytes!(
                "../../../tests/fixtures/m2-import-type-clauses/018-qualifier-before-value.utf16be.bin"
            ),
        },
        Row {
            id: "lexical-shadow",
            uri: "file:///stage70/import-type-oracle.pkl",
            query: "result.value",
            source: include_str!(
                "../../../tests/fixtures/m2-import-type-clauses/019-lexical-shadow.pkl"
            ),
            kind: "value",
            utf8: include_bytes!(
                "../../../tests/fixtures/m2-import-type-clauses/019-lexical-shadow.utf8.bin"
            ),
            utf16: include_bytes!(
                "../../../tests/fixtures/m2-import-type-clauses/019-lexical-shadow.utf16be.bin"
            ),
        },
        Row {
            id: "simple-clause",
            uri: "file:///stage70/import-type-oracle.pkl",
            query: "result",
            source: include_str!(
                "../../../tests/fixtures/m2-import-type-clauses/020-simple-clause.pkl"
            ),
            kind: "value",
            utf8: include_bytes!(
                "../../../tests/fixtures/m2-import-type-clauses/020-simple-clause.utf8.bin"
            ),
            utf16: include_bytes!(
                "../../../tests/fixtures/m2-import-type-clauses/020-simple-clause.utf16be.bin"
            ),
        },
        Row {
            id: "simple-expression",
            uri: "file:///stage70/import-type-oracle.pkl",
            query: "result",
            source: include_str!(
                "../../../tests/fixtures/m2-import-type-clauses/021-simple-expression.pkl"
            ),
            kind: "error",
            utf8: include_bytes!(
                "../../../tests/fixtures/m2-import-type-clauses/021-simple-expression.utf8.bin"
            ),
            utf16: include_bytes!(
                "../../../tests/fixtures/m2-import-type-clauses/021-simple-expression.utf16be.bin"
            ),
        },
        Row {
            id: "qualified-nontype",
            uri: "file:///stage70/import-type-oracle.pkl",
            query: "result",
            source: include_str!(
                "../../../tests/fixtures/m2-import-type-clauses/022-qualified-nontype.pkl"
            ),
            kind: "error",
            utf8: include_bytes!(
                "../../../tests/fixtures/m2-import-type-clauses/022-qualified-nontype.utf8.bin"
            ),
            utf16: include_bytes!(
                "../../../tests/fixtures/m2-import-type-clauses/022-qualified-nontype.utf16be.bin"
            ),
        },
        Row {
            id: "qualified-missing-math",
            uri: "file:///stage70/import-type-oracle.pkl",
            query: "result",
            source: include_str!(
                "../../../tests/fixtures/m2-import-type-clauses/023-qualified-missing-math.pkl"
            ),
            kind: "error",
            utf8: include_bytes!(
                "../../../tests/fixtures/m2-import-type-clauses/023-qualified-missing-math.utf8.bin"
            ),
            utf16: include_bytes!(
                "../../../tests/fixtures/m2-import-type-clauses/023-qualified-missing-math.utf16be.bin"
            ),
        },
        Row {
            id: "nontype-alias",
            uri: "file:///stage70/import-type-oracle.pkl",
            query: "result",
            source: include_str!(
                "../../../tests/fixtures/m2-import-type-clauses/024-nontype-alias.pkl"
            ),
            kind: "error",
            utf8: include_bytes!(
                "../../../tests/fixtures/m2-import-type-clauses/024-nontype-alias.utf8.bin"
            ),
            utf16: include_bytes!(
                "../../../tests/fixtures/m2-import-type-clauses/024-nontype-alias.utf16be.bin"
            ),
        },
        Row {
            id: "valid-json-alias",
            uri: "file:///stage70/import-type-oracle.pkl",
            query: "result",
            source: include_str!(
                "../../../tests/fixtures/m2-import-type-clauses/025-valid-json-alias.pkl"
            ),
            kind: "value",
            utf8: include_bytes!(
                "../../../tests/fixtures/m2-import-type-clauses/025-valid-json-alias.utf8.bin"
            ),
            utf16: include_bytes!(
                "../../../tests/fixtures/m2-import-type-clauses/025-valid-json-alias.utf16be.bin"
            ),
        },
        Row {
            id: "unused-missing",
            uri: "file:///stage70/import-type-oracle.pkl",
            query: "result",
            source: include_str!(
                "../../../tests/fixtures/m2-import-type-clauses/026-unused-missing.pkl"
            ),
            kind: "value",
            utf8: include_bytes!(
                "../../../tests/fixtures/m2-import-type-clauses/026-unused-missing.utf8.bin"
            ),
            utf16: include_bytes!(
                "../../../tests/fixtures/m2-import-type-clauses/026-unused-missing.utf16be.bin"
            ),
        },
        Row {
            id: "typed-clause",
            uri: "file:///stage70/import-type-oracle.pkl",
            query: "result",
            source: include_str!(
                "../../../tests/fixtures/m2-import-type-clauses/027-typed-clause.pkl"
            ),
            kind: "value",
            utf8: include_bytes!(
                "../../../tests/fixtures/m2-import-type-clauses/027-typed-clause.utf8.bin"
            ),
            utf16: include_bytes!(
                "../../../tests/fixtures/m2-import-type-clauses/027-typed-clause.utf16be.bin"
            ),
        },
        Row {
            id: "typed-expression-rhs",
            uri: "file:///stage70/import-type-oracle.pkl",
            query: "result",
            source: include_str!(
                "../../../tests/fixtures/m2-import-type-clauses/028-typed-expression-rhs.pkl"
            ),
            kind: "error",
            utf8: include_bytes!(
                "../../../tests/fixtures/m2-import-type-clauses/028-typed-expression-rhs.utf8.bin"
            ),
            utf16: include_bytes!(
                "../../../tests/fixtures/m2-import-type-clauses/028-typed-expression-rhs.utf16be.bin"
            ),
        },
        Row {
            id: "ref-generic",
            uri: "file:///stage70/import-type-oracle.pkl",
            query: "result",
            source: include_str!(
                "../../../tests/fixtures/m2-import-type-clauses/029-ref-generic.pkl"
            ),
            kind: "value",
            utf8: include_bytes!(
                "../../../tests/fixtures/m2-import-type-clauses/029-ref-generic.utf8.bin"
            ),
            utf16: include_bytes!(
                "../../../tests/fixtures/m2-import-type-clauses/029-ref-generic.utf16be.bin"
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
