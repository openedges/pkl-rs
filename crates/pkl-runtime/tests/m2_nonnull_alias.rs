//! Full Java NonNull constraint diagnostics, including source frames and retries.
#[path = "support/module_keys.rs"]
mod module_keys;
use pkl_runtime::{
    evaluation::frontend::{ModuleEvalOptions, ModuleSourceInput},
    probe::SourceProbe,
    value::PklString,
};

struct Row {
    id: &'static str,
    source: &'static str,
    expression: &'static str,
    kind: &'static str,
    utf8: &'static [u8],
    utf16: &'static [u8],
}

#[test]
fn nonnull_constraints_match_java_after_gc_and_retry() {
    let rows = [
        Row {
            id: "typed-null",
            source: include_str!("../../../tests/fixtures/m2-nonnull-alias/00-typed-null.pkl"),
            expression: "value",
            kind: "error",
            utf8: include_bytes!("../../../tests/fixtures/m2-nonnull-alias/00-typed-null.utf8.bin"),
            utf16: include_bytes!(
                "../../../tests/fixtures/m2-nonnull-alias/00-typed-null.utf16be.bin"
            ),
        },
        Row {
            id: "typed-null-default",
            source: include_str!(
                "../../../tests/fixtures/m2-nonnull-alias/01-typed-null-default.pkl"
            ),
            expression: "value",
            kind: "error",
            utf8: include_bytes!(
                "../../../tests/fixtures/m2-nonnull-alias/01-typed-null-default.utf8.bin"
            ),
            utf16: include_bytes!(
                "../../../tests/fixtures/m2-nonnull-alias/01-typed-null-default.utf16be.bin"
            ),
        },
        Row {
            id: "typed-int",
            source: include_str!("../../../tests/fixtures/m2-nonnull-alias/02-typed-int.pkl"),
            expression: "value",
            kind: "value",
            utf8: include_bytes!("../../../tests/fixtures/m2-nonnull-alias/02-typed-int.utf8.bin"),
            utf16: include_bytes!(
                "../../../tests/fixtures/m2-nonnull-alias/02-typed-int.utf16be.bin"
            ),
        },
        Row {
            id: "typed-empty-string",
            source: include_str!(
                "../../../tests/fixtures/m2-nonnull-alias/03-typed-empty-string.pkl"
            ),
            expression: "value",
            kind: "value",
            utf8: include_bytes!(
                "../../../tests/fixtures/m2-nonnull-alias/03-typed-empty-string.utf8.bin"
            ),
            utf16: include_bytes!(
                "../../../tests/fixtures/m2-nonnull-alias/03-typed-empty-string.utf16be.bin"
            ),
        },
        Row {
            id: "wrapped-alias-null",
            source: include_str!(
                "../../../tests/fixtures/m2-nonnull-alias/04-wrapped-alias-null.pkl"
            ),
            expression: "value",
            kind: "error",
            utf8: include_bytes!(
                "../../../tests/fixtures/m2-nonnull-alias/04-wrapped-alias-null.utf8.bin"
            ),
            utf16: include_bytes!(
                "../../../tests/fixtures/m2-nonnull-alias/04-wrapped-alias-null.utf16be.bin"
            ),
        },
        Row {
            id: "qualified-alias-null",
            source: include_str!(
                "../../../tests/fixtures/m2-nonnull-alias/05-qualified-alias-null.pkl"
            ),
            expression: "value",
            kind: "error",
            utf8: include_bytes!(
                "../../../tests/fixtures/m2-nonnull-alias/05-qualified-alias-null.utf8.bin"
            ),
            utf16: include_bytes!(
                "../../../tests/fixtures/m2-nonnull-alias/05-qualified-alias-null.utf16be.bin"
            ),
        },
        Row {
            id: "function-argument-null",
            source: include_str!(
                "../../../tests/fixtures/m2-nonnull-alias/06-function-argument-null.pkl"
            ),
            expression: "value",
            kind: "error",
            utf8: include_bytes!(
                "../../../tests/fixtures/m2-nonnull-alias/06-function-argument-null.utf8.bin"
            ),
            utf16: include_bytes!(
                "../../../tests/fixtures/m2-nonnull-alias/06-function-argument-null.utf16be.bin"
            ),
        },
        Row {
            id: "function-return-null",
            source: include_str!(
                "../../../tests/fixtures/m2-nonnull-alias/07-function-return-null.pkl"
            ),
            expression: "value",
            kind: "error",
            utf8: include_bytes!(
                "../../../tests/fixtures/m2-nonnull-alias/07-function-return-null.utf8.bin"
            ),
            utf16: include_bytes!(
                "../../../tests/fixtures/m2-nonnull-alias/07-function-return-null.utf16be.bin"
            ),
        },
        Row {
            id: "is-null",
            source: include_str!("../../../tests/fixtures/m2-nonnull-alias/08-is-null.pkl"),
            expression: "value",
            kind: "value",
            utf8: include_bytes!("../../../tests/fixtures/m2-nonnull-alias/08-is-null.utf8.bin"),
            utf16: include_bytes!(
                "../../../tests/fixtures/m2-nonnull-alias/08-is-null.utf16be.bin"
            ),
        },
        Row {
            id: "as-null",
            source: include_str!("../../../tests/fixtures/m2-nonnull-alias/09-as-null.pkl"),
            expression: "value",
            kind: "error",
            utf8: include_bytes!("../../../tests/fixtures/m2-nonnull-alias/09-as-null.utf8.bin"),
            utf16: include_bytes!(
                "../../../tests/fixtures/m2-nonnull-alias/09-as-null.utf16be.bin"
            ),
        },
        Row {
            id: "user-constraint-null",
            source: include_str!(
                "../../../tests/fixtures/m2-nonnull-alias/10-user-constraint-null.pkl"
            ),
            expression: "value",
            kind: "error",
            utf8: include_bytes!(
                "../../../tests/fixtures/m2-nonnull-alias/10-user-constraint-null.utf8.bin"
            ),
            utf16: include_bytes!(
                "../../../tests/fixtures/m2-nonnull-alias/10-user-constraint-null.utf16be.bin"
            ),
        },
        Row {
            id: "caught-typed-null",
            source: include_str!(
                "../../../tests/fixtures/m2-nonnull-alias/11-caught-typed-null.pkl"
            ),
            expression: "value",
            kind: "value",
            utf8: include_bytes!(
                "../../../tests/fixtures/m2-nonnull-alias/11-caught-typed-null.utf8.bin"
            ),
            utf16: include_bytes!(
                "../../../tests/fixtures/m2-nonnull-alias/11-caught-typed-null.utf16be.bin"
            ),
        },
        Row {
            id: "caught-default-null",
            source: include_str!(
                "../../../tests/fixtures/m2-nonnull-alias/12-caught-default-null.pkl"
            ),
            expression: "value",
            kind: "value",
            utf8: include_bytes!(
                "../../../tests/fixtures/m2-nonnull-alias/12-caught-default-null.utf8.bin"
            ),
            utf16: include_bytes!(
                "../../../tests/fixtures/m2-nonnull-alias/12-caught-default-null.utf16be.bin"
            ),
        },
    ];
    let mut failures = Vec::new();
    for row in rows {
        let (pairs, tail) = row.utf16.as_chunks::<2>();
        assert!(tail.is_empty(), "{}: complete Java UTF-16", row.id);
        let expected: Vec<_> = pairs.iter().map(|pair| u16::from_be_bytes(*pair)).collect();
        let mut probe = SourceProbe::new_module_evaluator("compact", true, false, false).unwrap();
        for attempt in 0..2 {
            probe
                .evaluate_module_source(
                    "file:///stage75/nonnull-alias.pkl",
                    Some(row.source),
                    row.expression,
                    0,
                    None,
                )
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
            let event = terminal.expect("bounded NonNull evaluation");
            let actual = if event.kind == "value" {
                event.value.as_ref()
            } else {
                event.error.as_ref()
            };
            if event.kind != row.kind
                || actual.is_none_or(|units| {
                    *units != expected
                        || PklString::from_utf16(units.clone())
                            .to_java_utf8()
                            .as_bytes()
                            != row.utf8
                })
            {
                failures.push(format!(
                    "{} attempt {attempt}: expected {} {:?}, got {} {:?}",
                    row.id,
                    row.kind,
                    String::from_utf16_lossy(&expected),
                    event.kind,
                    actual.map(|units| String::from_utf16_lossy(units))
                ));
            }
        }
        probe.close();
        assert_eq!(probe.collect().slots, 0, "{}: release roots", row.id);
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn unchanged_original_type_alias_module_matches_java_output() {
    const URI: &str = "file:///Users/cody.ij.hwang/Projects/pkl-rs.stage57/target/m2/stage67-full-regression-01-workspace/external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/api/typeAliases.pkl";
    for _ in 0..2 {
        let mut probe = SourceProbe::new_output(
            ModuleSourceInput {
                text: include_str!("../../../tests/fixtures/m2-nonnull-alias/original/module.pkl"),
                uri: URI,
                inferred_name: "typeAliases",
            },
            &[],
            &ModuleEvalOptions::default(),
            SourceProbe::options("compact", true, false, false).unwrap(),
        )
        .unwrap();
        probe.start_output_bytes(0, None).unwrap();
        let mut terminal = None;
        for _ in 0..500_000 {
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
            if event.kind == "read" {
                probe
                    .respond_configured(event.token.as_deref().unwrap(), 0)
                    .unwrap();
                continue;
            }
            terminal = Some(event);
            break;
        }
        let event = terminal.expect("bounded unchanged original module");
        assert_eq!(event.kind, "bytes", "{event:?}");
        assert_eq!(
            event.bytes.as_deref(),
            Some(
                include_bytes!("../../../tests/fixtures/m2-nonnull-alias/original/stdout.bin")
                    .as_slice()
            )
        );
        // No Logger event is accepted by the loop; the captured Java stderr is empty.
        assert!(
            include_bytes!("../../../tests/fixtures/m2-nonnull-alias/original/stderr.bin")
                .is_empty()
        );
        probe.close();
        assert_eq!(probe.collect().slots, 0);
    }
}

#[test]
fn qualified_builtin_alias_identity_and_order_match_java_after_retry() {
    let rows = [
        Row {
            id: "nonnull-null",
            source: include_str!(
                "../../../tests/fixtures/m2-qualified-builtin-alias/00-nonnull-null/module.pkl"
            ),
            expression: "value",
            kind: "error",
            utf8: include_bytes!(
                "../../../tests/fixtures/m2-qualified-builtin-alias/00-nonnull-null/result.stderr.bin"
            ),
            utf16: include_bytes!(
                "../../../tests/fixtures/m2-qualified-builtin-alias/00-nonnull-null/result.stderr.code-units.bin"
            ),
        },
        Row {
            id: "nonnull-int",
            source: include_str!(
                "../../../tests/fixtures/m2-qualified-builtin-alias/01-nonnull-int/module.pkl"
            ),
            expression: "value",
            kind: "value",
            utf8: include_bytes!(
                "../../../tests/fixtures/m2-qualified-builtin-alias/01-nonnull-int/result.stdout.bin"
            ),
            utf16: include_bytes!(
                "../../../tests/fixtures/m2-qualified-builtin-alias/01-nonnull-int/result.stdout.code-units.bin"
            ),
        },
        Row {
            id: "int8-valid",
            source: include_str!(
                "../../../tests/fixtures/m2-qualified-builtin-alias/02-int8-valid/module.pkl"
            ),
            expression: "value",
            kind: "value",
            utf8: include_bytes!(
                "../../../tests/fixtures/m2-qualified-builtin-alias/02-int8-valid/result.stdout.bin"
            ),
            utf16: include_bytes!(
                "../../../tests/fixtures/m2-qualified-builtin-alias/02-int8-valid/result.stdout.code-units.bin"
            ),
        },
        Row {
            id: "int8-range-error",
            source: include_str!(
                "../../../tests/fixtures/m2-qualified-builtin-alias/03-int8-range-error/module.pkl"
            ),
            expression: "value",
            kind: "error",
            utf8: include_bytes!(
                "../../../tests/fixtures/m2-qualified-builtin-alias/03-int8-range-error/result.stderr.bin"
            ),
            utf16: include_bytes!(
                "../../../tests/fixtures/m2-qualified-builtin-alias/03-int8-range-error/result.stderr.code-units.bin"
            ),
        },
        Row {
            id: "uint8-valid",
            source: include_str!(
                "../../../tests/fixtures/m2-qualified-builtin-alias/04-uint8-valid/module.pkl"
            ),
            expression: "value",
            kind: "value",
            utf8: include_bytes!(
                "../../../tests/fixtures/m2-qualified-builtin-alias/04-uint8-valid/result.stdout.bin"
            ),
            utf16: include_bytes!(
                "../../../tests/fixtures/m2-qualified-builtin-alias/04-uint8-valid/result.stdout.code-units.bin"
            ),
        },
        Row {
            id: "uint8-range-error",
            source: include_str!(
                "../../../tests/fixtures/m2-qualified-builtin-alias/05-uint8-range-error/module.pkl"
            ),
            expression: "value",
            kind: "error",
            utf8: include_bytes!(
                "../../../tests/fixtures/m2-qualified-builtin-alias/05-uint8-range-error/result.stderr.bin"
            ),
            utf16: include_bytes!(
                "../../../tests/fixtures/m2-qualified-builtin-alias/05-uint8-range-error/result.stderr.code-units.bin"
            ),
        },
        Row {
            id: "nonnull-extra-known-argument",
            source: include_str!(
                "../../../tests/fixtures/m2-qualified-builtin-alias/06-nonnull-extra-known-argument/module.pkl"
            ),
            expression: "value",
            kind: "error",
            utf8: include_bytes!(
                "../../../tests/fixtures/m2-qualified-builtin-alias/06-nonnull-extra-known-argument/result.stderr.bin"
            ),
            utf16: include_bytes!(
                "../../../tests/fixtures/m2-qualified-builtin-alias/06-nonnull-extra-known-argument/result.stderr.code-units.bin"
            ),
        },
        Row {
            id: "nonnull-extra-missing-argument",
            source: include_str!(
                "../../../tests/fixtures/m2-qualified-builtin-alias/07-nonnull-extra-missing-argument/module.pkl"
            ),
            expression: "value",
            kind: "error",
            utf8: include_bytes!(
                "../../../tests/fixtures/m2-qualified-builtin-alias/07-nonnull-extra-missing-argument/result.stderr.bin"
            ),
            utf16: include_bytes!(
                "../../../tests/fixtures/m2-qualified-builtin-alias/07-nonnull-extra-missing-argument/result.stderr.code-units.bin"
            ),
        },
        Row {
            id: "nonnull-two-arguments",
            source: include_str!(
                "../../../tests/fixtures/m2-qualified-builtin-alias/08-nonnull-two-arguments/module.pkl"
            ),
            expression: "value",
            kind: "error",
            utf8: include_bytes!(
                "../../../tests/fixtures/m2-qualified-builtin-alias/08-nonnull-two-arguments/result.stderr.bin"
            ),
            utf16: include_bytes!(
                "../../../tests/fixtures/m2-qualified-builtin-alias/08-nonnull-two-arguments/result.stderr.code-units.bin"
            ),
        },
        Row {
            id: "int8-extra-missing-argument",
            source: include_str!(
                "../../../tests/fixtures/m2-qualified-builtin-alias/09-int8-extra-missing-argument/module.pkl"
            ),
            expression: "value",
            kind: "error",
            utf8: include_bytes!(
                "../../../tests/fixtures/m2-qualified-builtin-alias/09-int8-extra-missing-argument/result.stderr.bin"
            ),
            utf16: include_bytes!(
                "../../../tests/fixtures/m2-qualified-builtin-alias/09-int8-extra-missing-argument/result.stderr.code-units.bin"
            ),
        },
        Row {
            id: "user-shadow-success",
            source: include_str!(
                "../../../tests/fixtures/m2-qualified-builtin-alias/10-user-shadow-success/module.pkl"
            ),
            expression: "value",
            kind: "value",
            utf8: include_bytes!(
                "../../../tests/fixtures/m2-qualified-builtin-alias/10-user-shadow-success/result.stdout.bin"
            ),
            utf16: include_bytes!(
                "../../../tests/fixtures/m2-qualified-builtin-alias/10-user-shadow-success/result.stdout.code-units.bin"
            ),
        },
        Row {
            id: "user-shadow-null",
            source: include_str!(
                "../../../tests/fixtures/m2-qualified-builtin-alias/11-user-shadow-null/module.pkl"
            ),
            expression: "value",
            kind: "error",
            utf8: include_bytes!(
                "../../../tests/fixtures/m2-qualified-builtin-alias/11-user-shadow-null/result.stderr.bin"
            ),
            utf16: include_bytes!(
                "../../../tests/fixtures/m2-qualified-builtin-alias/11-user-shadow-null/result.stderr.code-units.bin"
            ),
        },
        Row {
            id: "user-shadow-int8",
            source: include_str!(
                "../../../tests/fixtures/m2-qualified-builtin-alias/12-user-shadow-int8/module.pkl"
            ),
            expression: "value",
            kind: "value",
            utf8: include_bytes!(
                "../../../tests/fixtures/m2-qualified-builtin-alias/12-user-shadow-int8/result.stdout.bin"
            ),
            utf16: include_bytes!(
                "../../../tests/fixtures/m2-qualified-builtin-alias/12-user-shadow-int8/result.stdout.code-units.bin"
            ),
        },
        Row {
            id: "qualified-under-shadow",
            source: include_str!(
                "../../../tests/fixtures/m2-qualified-builtin-alias/13-qualified-under-shadow/module.pkl"
            ),
            expression: "value",
            kind: "error",
            utf8: include_bytes!(
                "../../../tests/fixtures/m2-qualified-builtin-alias/13-qualified-under-shadow/result.stderr.bin"
            ),
            utf16: include_bytes!(
                "../../../tests/fixtures/m2-qualified-builtin-alias/13-qualified-under-shadow/result.stderr.code-units.bin"
            ),
        },
    ];
    let mut failures = Vec::new();
    for row in rows {
        let (pairs, tail) = row.utf16.as_chunks::<2>();
        assert!(tail.is_empty(), "{}: complete Java UTF-16", row.id);
        let expected: Vec<_> = pairs.iter().map(|pair| u16::from_be_bytes(*pair)).collect();
        let mut probe = SourceProbe::new_module_evaluator("compact", true, false, false).unwrap();
        for attempt in 0..2 {
            probe
                .evaluate_module_source(
                    "file:///stage75/qualified-builtin-alias.pkl",
                    Some(row.source),
                    row.expression,
                    0,
                    None,
                )
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
            let event = terminal.expect("bounded NonNull evaluation");
            let actual = if event.kind == "value" {
                event.value.as_ref()
            } else {
                event.error.as_ref()
            };
            if event.kind != row.kind
                || actual.is_none_or(|units| {
                    *units != expected
                        || PklString::from_utf16(units.clone())
                            .to_java_utf8()
                            .as_bytes()
                            != row.utf8
                })
            {
                failures.push(format!(
                    "{} attempt {attempt}: expected {} {:?}, got {} {:?}",
                    row.id,
                    row.kind,
                    String::from_utf16_lossy(&expected),
                    event.kind,
                    actual.map(|units| String::from_utf16_lossy(units))
                ));
            }
        }
        probe.close();
        assert_eq!(probe.collect().slots, 0, "{}: release roots", row.id);
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
