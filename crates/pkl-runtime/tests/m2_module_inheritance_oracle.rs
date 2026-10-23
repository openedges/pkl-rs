//! Full Java bytes and actual callback identities from closed inheritance observations.
use pkl_runtime::probe::{SourceEvent, SourceProbe};
fn next(probe: &mut SourceProbe) -> SourceEvent {
    for _ in 0..100_000 {
        let event = probe.poll(0, 1).unwrap();
        probe.collect();
        if event.kind != "yield" {
            return event;
        }
    }
    panic!("continuation did not finish")
}
fn assert_result(event: SourceEvent, expected: (bool, &[u8])) {
    assert_eq!(
        event.kind,
        if expected.0 { "error" } else { "value" },
        "{event:?}"
    );
    let actual = if expected.0 {
        assert_eq!(event.disposition, Some("language-error"));
        event.error.unwrap()
    } else {
        event.value.unwrap()
    };
    let expected = expected
        .1
        .as_chunks::<2>()
        .0
        .iter()
        .map(|unit| u16::from_be_bytes(*unit))
        .collect::<Vec<_>>();
    assert_eq!(
        actual,
        expected,
        "actual: {}",
        String::from_utf16_lossy(&actual)
    );
}
fn compare(
    source: &str,
    parent: &str,
    uri: &str,
    expressions: &[&str],
    expected: &[(bool, &[u8])],
    callback: bool,
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
    let count = expressions.len() - usize::from(callback);
    let (mut resolves, mut loads) = (0, 0);
    for (index, result) in expected.iter().copied().take(count).enumerate() {
        probe
            .start_path(index.try_into().unwrap(), 0, None)
            .unwrap();
        loop {
            let event = next(&mut probe);
            if event.kind != "module-request" {
                assert_result(event, result);
                break;
            }
            let request = event.module.unwrap();
            let token = event.token.unwrap();
            if request.kind == "resolve" {
                resolves += 1;
                probe
                    .complete_module_resolution(
                        &token,
                        &request.original_uri,
                        &request.original_uri,
                        &request.original_uri,
                        0,
                    )
                    .unwrap();
            } else {
                loads += 1;
                if callback {
                    probe
                        .reenter_path(&token, count.try_into().unwrap(), 0, None)
                        .unwrap();
                    assert_result(next(&mut probe), expected[count]);
                    assert_eq!(next(&mut probe).token.as_deref(), Some(token.as_str()));
                }
                probe
                    .complete_module_source(&token, parent, "parent", 0)
                    .unwrap();
            }
        }
    }
    assert_eq!((resolves, loads), (1, 1));
    probe.close();
    assert_eq!(probe.collect().slots, 0);
}
#[test]
fn abstract_parent_amends_first_error() {
    compare(include_str!("../../../tests/fixtures/m2-module-inheritance/abstract-parent-amends-first-error/main.pkl"), include_str!("../../../tests/fixtures/m2-module-inheritance/abstract-parent-amends-first-error/parent.pkl"), include_str!("../../../tests/fixtures/m2-module-inheritance/abstract-parent-amends-first-error/uri.txt"), &["existing","existing"], &[(true,include_bytes!("../../../tests/fixtures/m2-module-inheritance/abstract-parent-amends-first-error/outer-0.stderr.code-units.bin").as_slice()),(true,include_bytes!("../../../tests/fixtures/m2-module-inheritance/abstract-parent-amends-first-error/outer-1.stderr.code-units.bin").as_slice())], false);
}

#[test]
fn amends_parent_source_primary_identity() {
    compare(include_str!("../../../tests/fixtures/m2-module-inheritance/amends-parent-source-primary-identity/main.pkl"), include_str!("../../../tests/fixtures/m2-module-inheritance/amends-parent-source-primary-identity/parent.pkl"), include_str!("../../../tests/fixtures/m2-module-inheritance/amends-parent-source-primary-identity/uri.txt"), &["marker","this.getClass() == import(\"life:/parent\").getClass()","marker","this == import(\"file:///Users/cody.ij.hwang/Projects/pkl-rs.stage42/target/m2/stage42-module-inheritance-oracle-01/workspace/amends-parent-source-primary-identity/main.pkl\")"], &[(false,include_bytes!("../../../tests/fixtures/m2-module-inheritance/amends-parent-source-primary-identity/outer-0.stdout.code-units.bin").as_slice()),(false,include_bytes!("../../../tests/fixtures/m2-module-inheritance/amends-parent-source-primary-identity/outer-1.stdout.code-units.bin").as_slice()),(false,include_bytes!("../../../tests/fixtures/m2-module-inheritance/amends-parent-source-primary-identity/outer-2.stdout.code-units.bin").as_slice()),(false,include_bytes!("../../../tests/fixtures/m2-module-inheritance/amends-parent-source-primary-identity/inner-0.stdout.code-units.bin").as_slice())], true);
}

#[test]
fn original_invalid_amendment_uses_the_adopted_module_diagnostic_name() {
    use pkl_runtime::evaluation::frontend::{
        ModuleEvalOptions, ModuleSourceInput, evaluate_module_output_with_host,
    };
    use pkl_runtime::modules::{ModuleCompletion, ModuleRequestKind, ModuleSource, ResolvedModule};
    let mut loads = 0;
    let result = evaluate_module_output_with_host(
        ModuleSourceInput {
            text: include_str!(
                "../../../external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/modules/invalidAmend2.pkl"
            ),
            uri: "file:///Users/cody.ij.hwang/Projects/pkl-rs.stage42/target/m2/stage42-original-modules41-workspace-01/external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/modules/invalidAmend2.pkl",
            inferred_name: "invalidAmend2",
        },
        &ModuleEvalOptions::default(),
        |request| match &request.kind {
            ModuleRequestKind::Resolve { original_uri, .. } => {
                assert!(original_uri.ends_with("/modules/library.pkl"));
                ModuleCompletion::Resolved(Ok(ResolvedModule {
                    original_uri: original_uri.clone(),
                    resolved_uri: original_uri.clone(),
                    source_uri: original_uri.clone(),
                }))
            }
            ModuleRequestKind::LoadSource { .. } => {
                loads += 1;
                ModuleCompletion::Source(Ok(ModuleSource {
                    text: include_str!("../../../external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/modules/library.pkl").into(),
                    inferred_name: "library".into(),
                }))
            }
        },
    );
    assert_eq!(loads, 1);
    assert_eq!(result.exit_code, 1);
    assert!(result.stdout.is_empty());
    assert_eq!(
        result.stderr,
        include_bytes!("../../../tests/fixtures/m2-module-inheritance/invalid-amend2/error.bin")
    );
}
