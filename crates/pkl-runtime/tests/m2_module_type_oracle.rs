//! Exact pinned Java API observations. Fixture bytes are copied from closed Java captures.
use pkl_runtime::probe::SourceProbe;
fn compare(
    source: &str,
    child: &str,
    uri: &str,
    expressions: &[&str],
    expected: &[(&[u8], &[u8])],
) {
    let mut probe = SourceProbe::new_expressions(
        source,
        uri,
        "main",
        &expressions
            .iter()
            .map(|e| e.to_string())
            .collect::<Vec<_>>(),
        "compact",
        false,
        false,
        false,
    )
    .unwrap();
    for (index, (status, units)) in expected.iter().enumerate() {
        probe
            .start_path(index.try_into().unwrap(), 0, None)
            .unwrap();
        let mut terminal = false;
        for _ in 0..100_000 {
            let event = probe.poll(0, 1).unwrap();
            probe.collect();
            match event.kind {
                "yield" => {}
                "module-request" => {
                    if probe
                        .complete_default_module(event.token.as_deref().unwrap(), 0)
                        .unwrap()
                    {
                        continue;
                    }
                    let module = event.module.unwrap();
                    let token = event.token.unwrap();
                    if module.kind == "resolve" {
                        probe
                            .complete_module_resolution(
                                &token,
                                &module.original_uri,
                                &module.original_uri,
                                &module.original_uri,
                                0,
                            )
                            .unwrap();
                    } else {
                        probe
                            .complete_module_source(&token, child, "child", 0)
                            .unwrap();
                    }
                }
                "value" | "error" => {
                    assert_eq!(
                        if event.kind == "value" {
                            b"success
"
                            .as_slice()
                        } else {
                            b"error
"
                            .as_slice()
                        },
                        *status,
                        "call {index}: {event:?}"
                    );
                    let actual = if event.kind == "value" {
                        event.value.unwrap()
                    } else {
                        assert_eq!(event.disposition, Some("language-error"));
                        event.error.unwrap()
                    };
                    let expected = units
                        .as_chunks::<2>()
                        .0
                        .iter()
                        .map(|unit| u16::from_be_bytes(*unit))
                        .collect::<Vec<_>>();
                    assert_eq!(
                        actual,
                        expected,
                        "call {index} actual: {}",
                        String::from_utf16_lossy(&actual)
                    );
                    terminal = true;
                    break;
                }
                _ => panic!("unexpected {event:?}"),
            }
        }
        assert!(terminal, "continuation did not finish");
    }
    probe.close();
    assert_eq!(probe.collect().slots, 0);
}
#[test]
fn qualified_missing_import() {
    compare(include_str!("../../../tests/fixtures/m2-module-types/qualified-missing-import/main.pkl"), include_str!("../../../tests/fixtures/m2-module-types/qualified-missing-import/child.pkl"), include_str!("../../../tests/fixtures/m2-module-types/qualified-missing-import/uri.txt"), &["result","result"], &[(include_bytes!("../../../tests/fixtures/m2-module-types/qualified-missing-import/outer-0.status.bin").as_slice(), include_bytes!("../../../tests/fixtures/m2-module-types/qualified-missing-import/outer-0.stderr.code-units.bin").as_slice()),(include_bytes!("../../../tests/fixtures/m2-module-types/qualified-missing-import/outer-1.status.bin").as_slice(), include_bytes!("../../../tests/fixtures/m2-module-types/qualified-missing-import/outer-1.stderr.code-units.bin").as_slice())]);
}

#[test]
fn qualified_not_import() {
    compare(include_str!("../../../tests/fixtures/m2-module-types/qualified-not-import/main.pkl"), include_str!("../../../tests/fixtures/m2-module-types/qualified-not-import/child.pkl"), include_str!("../../../tests/fixtures/m2-module-types/qualified-not-import/uri.txt"), &["result","result"], &[(include_bytes!("../../../tests/fixtures/m2-module-types/qualified-not-import/outer-0.status.bin").as_slice(), include_bytes!("../../../tests/fixtures/m2-module-types/qualified-not-import/outer-0.stderr.code-units.bin").as_slice()),(include_bytes!("../../../tests/fixtures/m2-module-types/qualified-not-import/outer-1.status.bin").as_slice(), include_bytes!("../../../tests/fixtures/m2-module-types/qualified-not-import/outer-1.stderr.code-units.bin").as_slice())]);
}

#[test]
fn qualified_missing_type() {
    compare(include_str!("../../../tests/fixtures/m2-module-types/qualified-missing-type/main.pkl"), include_str!("../../../tests/fixtures/m2-module-types/qualified-missing-type/child.pkl"), include_str!("../../../tests/fixtures/m2-module-types/qualified-missing-type/uri.txt"), &["result","result"], &[(include_bytes!("../../../tests/fixtures/m2-module-types/qualified-missing-type/outer-0.status.bin").as_slice(), include_bytes!("../../../tests/fixtures/m2-module-types/qualified-missing-type/outer-0.stderr.code-units.bin").as_slice()),(include_bytes!("../../../tests/fixtures/m2-module-types/qualified-missing-type/outer-1.status.bin").as_slice(), include_bytes!("../../../tests/fixtures/m2-module-types/qualified-missing-type/outer-1.stderr.code-units.bin").as_slice())]);
}

#[test]
fn qualified_not_type() {
    compare(include_str!("../../../tests/fixtures/m2-module-types/qualified-not-type/main.pkl"), include_str!("../../../tests/fixtures/m2-module-types/qualified-not-type/child.pkl"), include_str!("../../../tests/fixtures/m2-module-types/qualified-not-type/uri.txt"), &["result","result"], &[(include_bytes!("../../../tests/fixtures/m2-module-types/qualified-not-type/outer-0.status.bin").as_slice(), include_bytes!("../../../tests/fixtures/m2-module-types/qualified-not-type/outer-0.stderr.code-units.bin").as_slice()),(include_bytes!("../../../tests/fixtures/m2-module-types/qualified-not-type/outer-1.status.bin").as_slice(), include_bytes!("../../../tests/fixtures/m2-module-types/qualified-not-type/outer-1.stderr.code-units.bin").as_slice())]);
}

#[test]
fn qualified_local_type_hidden() {
    compare(include_str!("../../../tests/fixtures/m2-module-types/qualified-local-type-hidden/main.pkl"), include_str!("../../../tests/fixtures/m2-module-types/qualified-local-type-hidden/child.pkl"), include_str!("../../../tests/fixtures/m2-module-types/qualified-local-type-hidden/uri.txt"), &["result","result"], &[(include_bytes!("../../../tests/fixtures/m2-module-types/qualified-local-type-hidden/outer-0.status.bin").as_slice(), include_bytes!("../../../tests/fixtures/m2-module-types/qualified-local-type-hidden/outer-0.stderr.code-units.bin").as_slice()),(include_bytes!("../../../tests/fixtures/m2-module-types/qualified-local-type-hidden/outer-1.status.bin").as_slice(), include_bytes!("../../../tests/fixtures/m2-module-types/qualified-local-type-hidden/outer-1.stderr.code-units.bin").as_slice())]);
}

#[test]
fn class_arity_before_arguments() {
    compare(include_str!("../../../tests/fixtures/m2-module-types/class-arity-before-arguments/main.pkl"), include_str!("../../../tests/fixtures/m2-module-types/class-arity-before-arguments/child.pkl"), include_str!("../../../tests/fixtures/m2-module-types/class-arity-before-arguments/uri.txt"), &["result","result"], &[(include_bytes!("../../../tests/fixtures/m2-module-types/class-arity-before-arguments/outer-0.status.bin").as_slice(), include_bytes!("../../../tests/fixtures/m2-module-types/class-arity-before-arguments/outer-0.stderr.code-units.bin").as_slice()),(include_bytes!("../../../tests/fixtures/m2-module-types/class-arity-before-arguments/outer-1.status.bin").as_slice(), include_bytes!("../../../tests/fixtures/m2-module-types/class-arity-before-arguments/outer-1.stderr.code-units.bin").as_slice())]);
}

#[test]
fn alias_arguments_before_arity() {
    compare(include_str!("../../../tests/fixtures/m2-module-types/alias-arguments-before-arity/main.pkl"), include_str!("../../../tests/fixtures/m2-module-types/alias-arguments-before-arity/child.pkl"), include_str!("../../../tests/fixtures/m2-module-types/alias-arguments-before-arity/uri.txt"), &["result","result"], &[(include_bytes!("../../../tests/fixtures/m2-module-types/alias-arguments-before-arity/outer-0.status.bin").as_slice(), include_bytes!("../../../tests/fixtures/m2-module-types/alias-arguments-before-arity/outer-0.stderr.code-units.bin").as_slice()),(include_bytes!("../../../tests/fixtures/m2-module-types/alias-arguments-before-arity/outer-1.status.bin").as_slice(), include_bytes!("../../../tests/fixtures/m2-module-types/alias-arguments-before-arity/outer-1.stderr.code-units.bin").as_slice())]);
}

#[test]
fn alias_arguments_left_to_right() {
    compare(include_str!("../../../tests/fixtures/m2-module-types/alias-arguments-left-to-right/main.pkl"), include_str!("../../../tests/fixtures/m2-module-types/alias-arguments-left-to-right/child.pkl"), include_str!("../../../tests/fixtures/m2-module-types/alias-arguments-left-to-right/uri.txt"), &["result","result"], &[(include_bytes!("../../../tests/fixtures/m2-module-types/alias-arguments-left-to-right/outer-0.status.bin").as_slice(), include_bytes!("../../../tests/fixtures/m2-module-types/alias-arguments-left-to-right/outer-0.stderr.code-units.bin").as_slice()),(include_bytes!("../../../tests/fixtures/m2-module-types/alias-arguments-left-to-right/outer-1.status.bin").as_slice(), include_bytes!("../../../tests/fixtures/m2-module-types/alias-arguments-left-to-right/outer-1.stderr.code-units.bin").as_slice())]);
}

#[test]
fn alias_handle_and_nullable_default() {
    compare(include_str!("../../../tests/fixtures/m2-module-types/alias-handle-and-nullable-default/main.pkl"), include_str!("../../../tests/fixtures/m2-module-types/alias-handle-and-nullable-default/child.pkl"), include_str!("../../../tests/fixtures/m2-module-types/alias-handle-and-nullable-default/uri.txt"), &["handle","handle.getClass()","result","handle == handle"], &[(include_bytes!("../../../tests/fixtures/m2-module-types/alias-handle-and-nullable-default/outer-0.status.bin").as_slice(), include_bytes!("../../../tests/fixtures/m2-module-types/alias-handle-and-nullable-default/outer-0.stdout.code-units.bin").as_slice()),(include_bytes!("../../../tests/fixtures/m2-module-types/alias-handle-and-nullable-default/outer-1.status.bin").as_slice(), include_bytes!("../../../tests/fixtures/m2-module-types/alias-handle-and-nullable-default/outer-1.stdout.code-units.bin").as_slice()),(include_bytes!("../../../tests/fixtures/m2-module-types/alias-handle-and-nullable-default/outer-2.status.bin").as_slice(), include_bytes!("../../../tests/fixtures/m2-module-types/alias-handle-and-nullable-default/outer-2.stdout.code-units.bin").as_slice()),(include_bytes!("../../../tests/fixtures/m2-module-types/alias-handle-and-nullable-default/outer-3.status.bin").as_slice(), include_bytes!("../../../tests/fixtures/m2-module-types/alias-handle-and-nullable-default/outer-3.stdout.code-units.bin").as_slice())]);
}

#[test]
fn alias_constraint_declaration_owner() {
    compare(include_str!("../../../tests/fixtures/m2-module-types/alias-constraint-declaration-owner/main.pkl"), include_str!("../../../tests/fixtures/m2-module-types/alias-constraint-declaration-owner/child.pkl"), include_str!("../../../tests/fixtures/m2-module-types/alias-constraint-declaration-owner/uri.txt"), &["accepted","rejected","rejected","accepted"], &[(include_bytes!("../../../tests/fixtures/m2-module-types/alias-constraint-declaration-owner/outer-0.status.bin").as_slice(), include_bytes!("../../../tests/fixtures/m2-module-types/alias-constraint-declaration-owner/outer-0.stdout.code-units.bin").as_slice()),(include_bytes!("../../../tests/fixtures/m2-module-types/alias-constraint-declaration-owner/outer-1.status.bin").as_slice(), include_bytes!("../../../tests/fixtures/m2-module-types/alias-constraint-declaration-owner/outer-1.stderr.code-units.bin").as_slice()),(include_bytes!("../../../tests/fixtures/m2-module-types/alias-constraint-declaration-owner/outer-2.status.bin").as_slice(), include_bytes!("../../../tests/fixtures/m2-module-types/alias-constraint-declaration-owner/outer-2.stderr.code-units.bin").as_slice()),(include_bytes!("../../../tests/fixtures/m2-module-types/alias-constraint-declaration-owner/outer-3.status.bin").as_slice(), include_bytes!("../../../tests/fixtures/m2-module-types/alias-constraint-declaration-owner/outer-3.stdout.code-units.bin").as_slice())]);
}

#[test]
fn alias_initialization_failure_retry() {
    compare(include_str!("../../../tests/fixtures/m2-module-types/alias-initialization-failure-retry/main.pkl"), include_str!("../../../tests/fixtures/m2-module-types/alias-initialization-failure-retry/child.pkl"), include_str!("../../../tests/fixtures/m2-module-types/alias-initialization-failure-retry/uri.txt"), &["result","result"], &[(include_bytes!("../../../tests/fixtures/m2-module-types/alias-initialization-failure-retry/outer-0.status.bin").as_slice(), include_bytes!("../../../tests/fixtures/m2-module-types/alias-initialization-failure-retry/outer-0.stderr.code-units.bin").as_slice()),(include_bytes!("../../../tests/fixtures/m2-module-types/alias-initialization-failure-retry/outer-1.status.bin").as_slice(), include_bytes!("../../../tests/fixtures/m2-module-types/alias-initialization-failure-retry/outer-1.stderr.code-units.bin").as_slice())]);
}

#[test]
fn alias_class_default_declaring_owner() {
    compare(include_str!("../../../tests/fixtures/m2-module-types/alias-class-default-declaring-owner/main.pkl"), include_str!("../../../tests/fixtures/m2-module-types/alias-class-default-declaring-owner/child.pkl"), include_str!("../../../tests/fixtures/m2-module-types/alias-class-default-declaring-owner/uri.txt"), &["result.x","sameClass","result.x"], &[(include_bytes!("../../../tests/fixtures/m2-module-types/alias-class-default-declaring-owner/outer-0.status.bin").as_slice(), include_bytes!("../../../tests/fixtures/m2-module-types/alias-class-default-declaring-owner/outer-0.stdout.code-units.bin").as_slice()),(include_bytes!("../../../tests/fixtures/m2-module-types/alias-class-default-declaring-owner/outer-1.status.bin").as_slice(), include_bytes!("../../../tests/fixtures/m2-module-types/alias-class-default-declaring-owner/outer-1.stdout.code-units.bin").as_slice()),(include_bytes!("../../../tests/fixtures/m2-module-types/alias-class-default-declaring-owner/outer-2.status.bin").as_slice(), include_bytes!("../../../tests/fixtures/m2-module-types/alias-class-default-declaring-owner/outer-2.stdout.code-units.bin").as_slice())]);
}
