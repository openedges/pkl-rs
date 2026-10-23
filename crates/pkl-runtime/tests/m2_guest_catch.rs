//! Same-VM handler and standard-library bootstrap development regressions.
//! Fresh Java controls separately establish byte compatibility.
use pkl_runtime::probe::{SourceEvent, SourceProbe};

fn next(probe: &mut SourceProbe) -> SourceEvent {
    for _ in 0..100_000 {
        let event = probe.poll(0, 1).unwrap();
        probe.collect();
        if event.kind == "module-request"
            && probe
                .complete_default_module(event.token.as_deref().unwrap(), 0)
                .unwrap()
        {
            continue;
        }
        if event.kind != "yield" {
            return event;
        }
    }
    panic!("guest catch did not settle");
}

#[test]
fn ordinary_catch_returns_in_same_module_and_retains_successful_values() {
    let source = "import \"pkl:test\"\ninner = test.catch(() -> throw(\"inner\"))\ncaughtOuter = test.catch(() -> test.catch(() -> 1))\nnormal = test.catchOrNull(() -> 7)\nstable = 9\n";
    let expressions = ["inner", "caughtOuter", "normal", "stable", "inner"];
    let mut probe = SourceProbe::new_expressions_text(
        source,
        "file:///guest-catch.pkl",
        "main",
        &expressions.map(String::from),
        "compact",
        false,
        false,
        true,
    )
    .unwrap();
    for (index, expression) in expressions.iter().enumerate() {
        probe.start_path(index as u32, 0, None).unwrap();
        let event = next(&mut probe);
        assert_eq!(event.kind, "value", "{} {:?}", expression, event);
        if index == 0 || index == 4 {
            assert_eq!(
                event.value.unwrap(),
                "inner".encode_utf16().collect::<Vec<_>>()
            );
        }
    }
    probe.close();
    assert_eq!(probe.collect().slots, 0);
}

#[test]
fn base_module_aliases_and_implicit_class_share_actual_identity() {
    let source = "import \"pkl:base\" as a\nimport \"pkl:base\" as b\nresult = a == b && a.Int == Int && a.Module == Module\n";
    let mut probe = SourceProbe::new_expressions_text(
        source,
        "file:///base-identity.pkl",
        "main",
        &["result".into()],
        "compact",
        false,
        false,
        false,
    )
    .unwrap();
    probe.start_path(0, 0, None).unwrap();
    let event = next(&mut probe);
    assert_eq!(event.kind, "value", "{event:?}");
    assert_eq!(
        event.value.unwrap(),
        "true".encode_utf16().collect::<Vec<_>>()
    );
    probe.close();
    assert_eq!(probe.collect().slots, 0);
}

#[test]
fn imported_builtin_constructor_materializes_its_shared_class_default() {
    let mut probe = SourceProbe::new_expressions_text(
        "child = import(\"child.pkl\")\nresult = child.renderer.getClass() == PcfRenderer\n",
        "file:///builtin-default/main.pkl",
        "main",
        &["result".into()],
        "compact",
        false,
        false,
        false,
    )
    .unwrap();
    probe.start_path(0, 0, None).unwrap();
    let resolve = next(&mut probe);
    let uri = resolve.module.as_ref().unwrap().original_uri.clone();
    probe
        .complete_module_resolution(resolve.token.as_deref().unwrap(), &uri, &uri, &uri, 0)
        .unwrap();
    let load = next(&mut probe);
    probe
        .complete_module_source(
            load.token.as_deref().unwrap(),
            "renderer = new PcfRenderer {}\n",
            "child",
            0,
        )
        .unwrap();
    let result = next(&mut probe);
    assert_eq!(result.kind, "value", "{result:?}");
    assert_eq!(
        result.value.unwrap(),
        "true".encode_utf16().collect::<Vec<_>>()
    );
    probe.close();
    assert_eq!(probe.collect().slots, 0);
}

#[test]
fn typed_missing_module_is_catchable_but_raw_host_rejection_is_not() {
    let source = "import \"pkl:test\"\ncaught = test.catch(() -> import(\"missing.pkl\"))\nraw = test.catch(() -> read(\"life:raw\"))\nstable = 9\n";
    let mut probe = SourceProbe::new_expressions_text(
        source,
        "file:///guest-catch.pkl",
        "main",
        &["caught".into(), "raw".into(), "stable".into()],
        "compact",
        false,
        false,
        false,
    )
    .unwrap();
    probe.start_path(0, 0, None).unwrap();
    let request = next(&mut probe);
    probe
        .reject_module(request.token.as_deref().unwrap(), false, "not-found", "", 0)
        .unwrap();
    let caught = next(&mut probe);
    assert_eq!(caught.kind, "value", "{caught:?}");
    probe.start_path(1, 0, None).unwrap();
    let request = next(&mut probe);
    assert_eq!(request.kind, "read");
    probe
        .reject(request.token.as_deref().unwrap(), "raw host failure", 0)
        .unwrap();
    assert_eq!(next(&mut probe).kind, "error");
    probe.start_path(2, 0, None).unwrap();
    assert_eq!(next(&mut probe).value.unwrap(), vec![u16::from(b'9')]);
    probe.close();
    assert_eq!(probe.collect().slots, 0);
}

#[test]
fn original_super_method_failure_describes_the_parent_lookup_module() {
    use pkl_runtime::evaluation::frontend::{
        ModuleEvalOptions, ModuleSourceInput, evaluate_module_output_with_host,
    };
    use pkl_runtime::modules::{ModuleCompletion, ModuleRequestKind, ModuleSource, ResolvedModule};
    let sources = [
        (
            "supercalls1",
            include_str!(
                "../../../external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/modules/supercalls1.pkl"
            ),
        ),
        (
            "supercalls2",
            include_str!(
                "../../../external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/modules/supercalls2.pkl"
            ),
        ),
        (
            "supercalls3",
            include_str!(
                "../../../external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/modules/supercalls3.pkl"
            ),
        ),
    ];
    let result = evaluate_module_output_with_host(
        ModuleSourceInput {
            text: sources[2].1,
            uri: "file:///corpus/modules/supercalls3.pkl",
            inferred_name: "supercalls3",
        },
        &ModuleEvalOptions::default(),
        |request| match &request.kind {
            ModuleRequestKind::CheckImport { .. } => {
                pkl_runtime::modules::default_completion(request).expect("default import policy")
            }
            ModuleRequestKind::Capabilities { .. } | ModuleRequestKind::HasElement { .. } => {
                panic!("unexpected ModuleKey preparation in this fixture")
            }
            ModuleRequestKind::Resolve { original_uri, .. } => {
                ModuleCompletion::Resolved(Ok(ResolvedModule {
                    original_uri: original_uri.clone(),
                    resolved_uri: original_uri.clone(),
                    source_uri: original_uri.clone(),
                }))
            }
            ModuleRequestKind::LoadSource { resolved, .. } => {
                let name = resolved
                    .original_uri
                    .rsplit('/')
                    .next()
                    .unwrap()
                    .strip_suffix(".pkl")
                    .unwrap();
                ModuleCompletion::Source(Ok(ModuleSource {
                    text: sources
                        .iter()
                        .find(|(key, _)| *key == name)
                        .unwrap()
                        .1
                        .into(),
                    inferred_name: name.into(),
                }))
            }
        },
    );
    assert_eq!(
        result.exit_code,
        0,
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(result.stderr.is_empty());
    assert_eq!(
        result.stdout,
        include_bytes!(
            "../../../external/pkl/pkl-core/src/test/files/LanguageSnippetTests/output/modules/supercalls3.pcf"
        )
    );
}

#[test]
fn typed_module_security_denials_reach_both_guest_handlers_and_leave_the_session_open() {
    let source = "import \"pkl:test\"\ncaught = test.catch(() -> import(\"denied.pkl\"))\nnullable = test.catchOrNull(() -> import(\"denied.pkl\"))\nraw = test.catch(() -> import(\"raw.pkl\"))\nstable = 9\n";
    let mut probe = SourceProbe::new_expressions_text(
        source,
        "file:///guest-denied.pkl",
        "main",
        &[
            "caught".into(),
            "nullable".into(),
            "raw".into(),
            "stable".into(),
        ],
        "compact",
        false,
        false,
        true,
    )
    .unwrap();
    for index in 0..2 {
        probe.start_path(index, 0, None).unwrap();
        let request = next(&mut probe);
        assert_eq!(request.kind, "module-request");
        probe
            .reject_module(
                request.token.as_deref().unwrap(),
                false,
                "security-manager-exception",
                "Access denied",
                0,
            )
            .unwrap();
        let caught = next(&mut probe);
        assert_eq!(caught.kind, "value", "{caught:?}");
        assert_eq!(
            caught.value.unwrap(),
            "Access denied".encode_utf16().collect::<Vec<_>>()
        );
    }
    probe.start_path(2, 0, None).unwrap();
    let request = next(&mut probe);
    assert_eq!(request.kind, "module-request");
    probe
        .reject_module(
            request.token.as_deref().unwrap(),
            false,
            "denied",
            "Access denied",
            0,
        )
        .unwrap();
    assert_eq!(next(&mut probe).kind, "error");
    probe.start_path(3, 0, None).unwrap();
    assert_eq!(next(&mut probe).value.unwrap(), vec![u16::from(b'9')]);
    probe.close();
    assert_eq!(probe.collect().slots, 0);
}

#[test]
fn typed_security_completion_is_resolution_only_and_preserves_a_rejected_source_token() {
    let mut probe = SourceProbe::new_expressions_text(
        "value = import(\"child.pkl\").value\n",
        "file:///security-stage/main.pkl",
        "main",
        &["value".into()],
        "compact",
        false,
        false,
        true,
    )
    .unwrap();
    probe.start_path(0, 0, None).unwrap();
    let resolution = next(&mut probe);
    let uri = resolution.module.as_ref().unwrap().original_uri.clone();
    probe
        .complete_module_resolution(resolution.token.as_deref().unwrap(), &uri, &uri, &uri, 0)
        .unwrap();
    let source = next(&mut probe);
    let token = source.token.as_deref().unwrap();
    let error = probe
        .reject_module_text(
            token,
            true,
            "security-manager-exception",
            Some("Access denied".into()),
            0,
        )
        .unwrap_err();
    assert_eq!(error.kind, pkl_runtime::value::ErrorKind::Host);
    let pending = next(&mut probe);
    assert_eq!(pending.token.as_deref(), Some(token));
    probe
        .complete_module_source(token, "value = 7\n", "child", 0)
        .unwrap();
    assert_eq!(next(&mut probe).value.unwrap(), vec![u16::from(b'7')]);
    probe.close();
    assert_eq!(probe.collect().slots, 0);
}
