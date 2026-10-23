//! Typed module Host transport uses the existing evaluator and callback stack.
//! Java loader02 provides the separate cache/reentry compatibility observations.
use pkl_runtime::{
    modules::{ModuleRequest, ModuleRequestKind},
    probe::{SourceEvent, SourceModuleRequest, SourceProbe},
};

fn probe() -> SourceProbe {
    SourceProbe::new(
        "child=import(\"child.pkl\")\nstable=9\nresult=child.value\nother=child.value\n",
        "file:///modules/main.pkl",
        "main",
        &["result".into(), "stable".into(), "other".into()],
        "compact",
        false,
        false,
        false,
    )
    .unwrap()
}

fn next(probe: &mut SourceProbe) -> SourceEvent {
    for _ in 0..100_000 {
        let event = probe.poll(0, 1).unwrap();
        probe.collect();
        if event.kind != "yield" {
            return event;
        }
    }
    panic!("source session did not settle");
}

fn resolve(probe: &mut SourceProbe, event: &SourceEvent) {
    let request = event.module.as_ref().unwrap();
    assert_eq!(event.kind, "module-request");
    assert_eq!(request.kind, "resolve");
    assert_eq!(
        request.importer_original_uri.as_deref(),
        Some("file:///modules/main.pkl")
    );
    assert_eq!(
        request.importer_resolved_uri.as_deref(),
        Some("file:///modules/main.pkl")
    );
    assert_eq!(
        request.import_uri.as_deref(),
        Some("file:///modules/child.pkl")
    );
    probe
        .complete_module_resolution(
            event.token.as_deref().unwrap(),
            &request.original_uri,
            "file:///canonical/child.pkl",
            "file:///display/child.pkl",
            0,
        )
        .unwrap();
}

#[test]
fn typed_module_requests_reenter_resume_and_cache_in_one_session() {
    let mut probe = probe();
    probe.start_path(0, 0, None).unwrap();
    let request = next(&mut probe);
    let first = request.token.clone().unwrap();
    // A source completion cannot consume a pending resolution.
    assert!(
        probe
            .complete_module_source(&first, "value=0", "child", 0)
            .is_err()
    );
    assert_eq!(next(&mut probe).token.as_deref(), Some(first.as_str()));
    resolve(&mut probe, &request);
    let loaded = next(&mut probe);
    let module = loaded.module.as_ref().unwrap();
    assert_eq!(module.kind, "load-source");
    assert_eq!(
        module.resolved_uri.as_deref(),
        Some("file:///canonical/child.pkl")
    );
    assert_eq!(
        module.source_uri.as_deref(),
        Some("file:///display/child.pkl")
    );
    let token = loaded.token.unwrap();
    probe.reenter_path(&token, 1, 0, None).unwrap();
    assert_eq!(next(&mut probe).value.unwrap(), vec![u16::from(b'9')]);
    assert_eq!(next(&mut probe).token.as_deref(), Some(token.as_str()));
    probe
        .complete_module_source(&token, "value=7\n", "child", 0)
        .unwrap();
    assert_eq!(next(&mut probe).value.unwrap(), vec![u16::from(b'7')]);
    assert!(
        probe
            .complete_module_source(&token, "value=8\n", "child", 0)
            .is_err()
    );
    probe.start_path(2, 0, None).unwrap();
    assert_eq!(next(&mut probe).value.unwrap(), vec![u16::from(b'7')]);
    probe.close();
    assert_eq!(probe.collect().slots, 0);
}

#[test]
fn pending_module_cancellation_is_separate_and_exactly_once() {
    for loading in [false, true] {
        let mut probe = probe();
        probe.start_path(0, 0, None).unwrap();
        let mut event = next(&mut probe);
        if loading {
            resolve(&mut probe, &event);
            event = next(&mut probe);
        }
        let token = event.token.unwrap();
        probe.cancel();
        assert_eq!(probe.drain_cancelled_modules(), vec![token.clone()]);
        assert!(probe.drain_cancelled_modules().is_empty());
        assert!(probe.drain_cancelled_reads().is_empty());
        assert!(
            probe
                .reject_module(&token, loading, "not-found", "", 0)
                .is_err()
        );
        assert_eq!(probe.collect().slots, 0);
    }
}

#[test]
fn closing_during_source_callback_finishes_current_call_and_rejects_new_calls() {
    let mut probe = probe();
    probe.start_path(0, 0, None).unwrap();
    let request = next(&mut probe);
    resolve(&mut probe, &request);
    let loading = next(&mut probe);
    let token = loading.token.unwrap();
    probe.close_from_callback(&token).unwrap();
    assert!(probe.reenter_path(&token, 1, 0, None).is_err());
    probe
        .complete_module_source(&token, "value=9\n", "child", 0)
        .unwrap();
    assert_eq!(next(&mut probe).value.unwrap(), vec![u16::from(b'9')]);
    assert!(probe.start_path(0, 0, None).is_err());
    assert_eq!(probe.collect().slots, 0);
}

#[test]
fn imported_parser_error_preserves_child_source_and_first_call_cache() {
    let mut probe = probe();
    probe.start_path(0, 0, None).unwrap();
    let request = next(&mut probe);
    resolve(&mut probe, &request);
    let event = next(&mut probe);
    probe
        .complete_module_source(event.token.as_deref().unwrap(), "value = )\n", "child", 0)
        .unwrap();
    let first = next(&mut probe);
    assert_eq!(first.kind, "error");
    let error = first.error.unwrap();
    let text = String::from_utf16(&error).unwrap();
    assert!(text.contains("file:///display/child.pkl"));
    assert!(text.contains("file:///modules/main.pkl"));
    assert!(text.contains("repl:text"));
    probe.start_path(2, 0, None).unwrap();
    assert_eq!(next(&mut probe).error.unwrap(), error);
    probe.close();
    assert_eq!(probe.collect().slots, 0);
}

#[test]
fn request_identifier_transport_preserves_all_u64_bits() {
    let request = SourceModuleRequest::from(ModuleRequest {
        id: u64::MAX,
        kind: ModuleRequestKind::LoadSource {
            resolved: pkl_runtime::modules::ResolvedModule {
                original_uri: "life:/original".into(),
                resolved_uri: "life:/resolved".into(),
                source_uri: "life:/display".into(),
            },
        },
    });
    assert_eq!(request.id, "18446744073709551615");
    assert_eq!(request.id.parse::<u64>().unwrap(), u64::MAX);
}

#[test]
fn output_transport_keeps_shared_language_and_unsupported_exit_classification() {
    use pkl_runtime::evaluation::frontend::{
        DiagnosticProfile, ModuleEvalOptions, ModuleSourceInput,
    };
    for (kind, exit, disposition) in [
        ("not-found", 1, "language-error"),
        ("unsupported", 2, "unsupported"),
    ] {
        let mut probe = SourceProbe::new_output(
            ModuleSourceInput {
                text: "child=import(\"child.pkl\")\noutput { text=child.value.toString() }\n",
                uri: "file:///modules/main.pkl",
                inferred_name: "main",
            },
            &[],
            &ModuleEvalOptions {
                diagnostic_profile: DiagnosticProfile::ApiIdentity,
                ..Default::default()
            },
            SourceProbe::options("compact", false, false, false).unwrap(),
        )
        .unwrap();
        probe.start_output_bytes(0, None).unwrap();
        let request = loop {
            let event = next(&mut probe);
            if event.kind == "read" {
                probe
                    .respond_configured(event.token.as_deref().unwrap(), 0)
                    .unwrap();
            } else {
                break event;
            }
        };
        assert_eq!(request.kind, "module-request");
        probe
            .reject_module(
                request.token.as_deref().unwrap(),
                false,
                kind,
                "missing capability",
                0,
            )
            .unwrap();
        let failure = next(&mut probe);
        assert_eq!(failure.kind, "error");
        assert_eq!(failure.exit_code, Some(exit));
        assert_eq!(failure.disposition, Some(disposition));
        assert_eq!(failure.phase, Some("module-output"));
        assert!(!failure.error_bytes.unwrap().is_empty());
        probe.close();
        assert_eq!(probe.collect().slots, 0);
    }
}

#[test]
fn expression_unsupported_retains_raw_units_without_inventing_a_java_api_exit() {
    let mut probe = probe();
    probe.start_path(0, 0, None).unwrap();
    let request = next(&mut probe);
    probe
        .reject_module(
            request.token.as_deref().unwrap(),
            false,
            "unsupported",
            "missing \0é capability",
            0,
        )
        .unwrap();
    let failure = next(&mut probe);
    assert_eq!(failure.kind, "error");
    assert_eq!(failure.disposition, Some("unsupported"));
    assert_eq!(failure.exit_code, None);
    assert!(failure.error_bytes.is_none());
    let units = failure.error.unwrap();
    assert!(units.windows(2).any(|part| part == [0, 0xe9]));
    assert!(String::from_utf16(&units).unwrap().contains("repl:text"));
    probe.close();
    assert_eq!(probe.collect().slots, 0);
}
