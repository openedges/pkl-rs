//! Source-derived request/error-origin regressions, not Java differential evidence.
#[path = "../examples/support/module_host.rs"]
mod module_host;
use pkl_runtime::{
    evaluation::session::{SourceSession, SourceSessionOptions},
    glob::{GlobCompletion, GlobReader, GlobRequest, GlobRequestKind, configured_completion},
    host::HostPoll,
    modules::*,
    value::{RuntimeError, Value},
};

fn selection(uri: &str) -> ModuleRequest {
    ModuleRequest {
        id: 1,
        kind: ModuleRequestKind::SelectKey {
            id: ModuleKeyId(2),
            uri: uri.into(),
            phase: SelectionPhase::ClausePreparation,
        },
    }
}
fn selected(uri: &str) -> ModuleKey {
    let ModuleCompletion::KeySelected(Ok(key)) = module_host::complete(&selection(uri)) else {
        panic!("key selection: {uri}");
    };
    key
}

#[test]
fn factory_validation_uses_decoded_ssp_and_preserves_case_and_original_uri() {
    for uri in [
        "file:relative.pkl",
        "FILE:relative.pkl",
        "file:%72elative.pkl",
    ] {
        assert!(matches!(
            module_host::complete(&selection(uri)),
            ModuleCompletion::KeySelected(Err(ModuleHostError::UriSyntax { .. }))
        ));
    }
    for uri in ["file:/abs.pkl", "FILE:/abs.pkl", "file:%2Fabs.pkl"] {
        let key = selected(uri);
        assert_eq!(key.uri, uri);
        assert_eq!(key.kind, ModuleKeyKind::StandardFile);
    }
    // Factory admission does not assert that the filesystem can resolve an
    // opaque URI whose decoded scheme-specific part starts with a slash.
    for uri in [
        "birds:/child.pkl",
        "jar:file:/archive.zip!/child.pkl",
        "JAR:file:/archive.zip!/child.pkl",
    ] {
        assert_eq!(selected(uri).kind, ModuleKeyKind::StandardGenericUrl);
    }
    assert!(standard_key_completion(&selection("birds:opaque.pkl")).is_none());
    for uri in ["modulepath:/x.pkl", "jrt:/x"] {
        assert!(standard_key_completion(&selection(uri)).is_none());
    }
    // The package factory validates the version before fragment/transport.
    assert!(matches!(
        standard_key_completion(&selection("package://example/x")),
        Some(ModuleCompletion::KeySelected(Err(
            ModuleHostError::UriSyntax { .. }
        )))
    ));
}

#[test]
fn nonglobbable_url_capability_precedes_policy_and_never_requires_url_io() {
    for uri in [
        "https://example.invalid/*.pkl",
        "http://example.invalid/*.pkl",
        "birds:/x/*.pkl",
    ] {
        let key = selected(uri);
        let request = GlobRequest {
            id: 1,
            kind: GlobRequestKind::IsGlobbable {
                reader: GlobReader::Module(key),
            },
        };
        assert!(matches!(
            configured_completion(&request, &[], &[]),
            Some(GlobCompletion::Globbable(Ok(false)))
        ));
    }
}

#[test]
fn generic_url_policy_is_resolve_time_and_http_io_remains_explicitly_unsupported() {
    for (uri, security) in [
        ("birds:///foo.pkl", true),
        ("http://example.invalid/a.pkl", false),
        ("https://example.invalid/a.pkl", false),
    ] {
        let key = selected(uri);
        let request = ModuleRequest {
            id: 2,
            kind: ModuleRequestKind::ResolveKey {
                key,
                resolved_key_id: ResolvedKeyId(3),
                phase: ResolutionPhase::Query,
            },
        };
        let completion = module_host::complete(&request);
        if security {
            assert!(matches!(
                completion,
                ModuleCompletion::Resolved(Err(ModuleHostError::SecurityManagerException { .. }))
            ));
        } else {
            assert!(matches!(
                completion,
                ModuleCompletion::Resolved(Err(ModuleHostError::Unsupported(_)))
            ));
        }
    }
    // Custom Hosts retaining the old bundled/trust profile still own these keys.
    assert!(default_completion(&selection("birds:///foo.pkl")).is_none());
}

fn evaluate(text: &str, path: &str) -> (Result<Value, RuntimeError>, Vec<ModuleRequestKind>) {
    let mut session = SourceSession::compile(
        text,
        "file:///stage137/main.pkl",
        "main",
        &[path.into()],
        SourceSessionOptions::default(),
    )
    .unwrap();
    session.start_path(0, 0, None).unwrap();
    let mut requests = Vec::new();
    for _ in 0..100_000 {
        let event = session.poll(0, 1);
        session.collect(&[]);
        match event {
            HostPoll::Yielded => {}
            HostPoll::Module { token, request } => {
                requests.push(request.kind.clone());
                session
                    .complete_module(&token, module_host::complete(&request), 0)
                    .unwrap();
            }
            HostPoll::Glob { token, request } => {
                let completion =
                    configured_completion(&request, &[], &[]).expect("no glob I/O for this test");
                session.complete_glob(&token, completion, 0).unwrap();
            }
            HostPoll::Ready(result) => {
                session.close();
                return (result, requests);
            }
            other => panic!("unexpected effect: {other:?}"),
        }
    }
    panic!("poll bound");
}

#[test]
fn clause_selection_remains_eager_but_generic_url_rejection_is_lazy_and_has_import_frames() {
    let text = "import \"birds:///foo.pkl\"\nstable=7\nres=foo\n";
    let (result, requests) = evaluate(text, "stable");
    assert_eq!(result.unwrap(), Value::String("7".into()));
    assert!(requests.iter().any(|r| matches!(r, ModuleRequestKind::SelectKey { phase: SelectionPhase::ClausePreparation, uri, .. } if uri == "birds:///foo.pkl")));
    assert!(!requests.iter().any(|r| matches!(
        r,
        ModuleRequestKind::Resolve { .. } | ModuleRequestKind::LoadSource { .. }
    )));
    let (result, requests) = evaluate(text, "res");
    let error = result.unwrap_err();
    assert_eq!(
        error.message,
        "Refusing to load module `birds:///foo.pkl` because it does not match any entry in the module allowlist (`--allowed-modules`)."
    );
    assert!(error.frames.iter().any(|f| f.member == "#foo"));
    assert!(error.frames.iter().any(|f| f.member == "#res"));
    assert!(requests.iter().any(
        |r| matches!(r, ModuleRequestKind::Resolve { key, .. } if key.uri == "birds:///foo.pkl")
    ));
    assert!(
        !requests
            .iter()
            .any(|r| matches!(r, ModuleRequestKind::LoadSource { .. }))
    );
}

#[test]
fn invalid_file_factory_errors_keep_runtime_expression_origin_without_late_resolution() {
    for expression in [
        "import(\"file:path/to/foo.pkl\")",
        "import*(\"file:path/to/foo.pkl\")",
    ] {
        let (result, requests) = evaluate(&format!("res={expression}\n"), "res");
        let error = result.unwrap_err();
        assert_eq!(
            error.message,
            "Module URI `file:path/to/foo.pkl` has invalid syntax."
        );
        assert_eq!(
            error.hint.as_ref().unwrap(),
            "File URIs must have a path that starts with `/` (e.g. file:/path/to/my_module.pkl).\nTo resolve relative paths, remove the scheme prefix (remove \"file:\")."
        );
        assert!(error.frames.iter().any(|f| f.member == "#res"));
        assert!(!requests.iter().any(|r| matches!(
            r,
            ModuleRequestKind::Resolve { .. } | ModuleRequestKind::LoadSource { .. }
        )));
    }
}

#[test]
fn invalid_file_clause_is_still_a_factory_failure_without_fabricated_runtime_frames() {
    let (result, requests) = evaluate(
        "import \"file:relative.pkl\" as child\nstable=7\n",
        "stable",
    );
    let error = result.unwrap_err();
    assert_eq!(
        error.message,
        "Module URI `file:relative.pkl` has invalid syntax."
    );
    assert!(error.frames.is_empty());
    assert!(requests.iter().any(|r| matches!(
        r,
        ModuleRequestKind::SelectKey {
            phase: SelectionPhase::ClausePreparation,
            ..
        }
    )));
}

#[test]
fn custom_host_handles_do_not_opt_into_standard_key_capabilities() {
    for handle in ["builtin:file", "builtin:url", "builtin:generic-url"] {
        let key = ModuleKey {
            id: ModuleKeyId(7),
            uri: "birds:///x.pkl".into(),
            kind: ModuleKeyKind::Host,
            handle: handle.into(),
        };
        let request = ModuleRequest {
            id: 1,
            kind: ModuleRequestKind::ResolveKey {
                key: key.clone(),
                resolved_key_id: ResolvedKeyId(8),
                phase: ResolutionPhase::Query,
            },
        };
        assert!(standard_key_completion(&request).is_none());
        let request = GlobRequest {
            id: 2,
            kind: GlobRequestKind::IsGlobbable {
                reader: GlobReader::Module(key),
            },
        };
        assert!(configured_completion(&request, &[], &[]).is_none());
    }
}

#[test]
fn public_probe_round_trips_standard_key_kinds_through_real_host_requests() {
    use pkl_runtime::{probe::SourceProbe, value::PklString};

    for (kind_name, expected_kind, uri) in [
        (
            "standard-file",
            ModuleKeyKind::StandardFile,
            "file:///stage137-probe/main.pkl",
        ),
        (
            "standard-http",
            ModuleKeyKind::StandardHttp,
            "https://example.invalid/main.pkl",
        ),
        (
            "standard-generic-url",
            ModuleKeyKind::StandardGenericUrl,
            "custom:///main.pkl",
        ),
    ] {
        let mut probe = SourceProbe::new_module_evaluator("compact", false, false, false).unwrap();
        // This is a Host transport test, not URL I/O or default-policy evidence.
        // The Host supplies the module below; no filesystem/network is consulted.
        let handle = PklString::from_utf16(vec![0x68, 0x3a, 0xd800, 0x61, 0xdfff]);
        probe
            .evaluate_module_source(uri, None, "stable", 0, None)
            .unwrap();
        let mut selected_id = None;
        let mut boundaries = Vec::new();
        let mut terminal = false;
        for _ in 0..100_000 {
            let event = probe.poll_text(0, 1).unwrap();
            probe.collect();
            match event.kind {
                "yield" => {}
                "module-request" => {
                    let token = event.token.as_deref().unwrap();
                    let transport = event.module.as_ref().unwrap();
                    let request = probe.module_request(token).unwrap();
                    if let ModuleRequestKind::SelectKey {
                        uri: requested,
                        phase: SelectionPhase::Primary,
                        ..
                    } = &request.kind
                    {
                        assert_eq!(requested, uri);
                        assert_eq!(transport.kind, "select-key");
                        assert!(selected_id.is_none());
                        let id = transport.key_id.as_deref().unwrap();
                        selected_id = Some(id.to_owned());
                        // Decode the public string kind here. Supplying a typed
                        // ModuleCompletion would bypass the boundary under test.
                        probe
                            .complete_module_key(token, id, uri, kind_name, handle.clone(), 0)
                            .unwrap();
                        continue;
                    }
                    let key = match &request.kind {
                        ModuleRequestKind::QueryCachePolicy { key }
                        | ModuleRequestKind::ResolveKey {
                            key,
                            phase: ResolutionPhase::Primary,
                            ..
                        } => Some(key),
                        ModuleRequestKind::LoadSource { key, .. } => Some(&key.key),
                        _ => None,
                    };
                    if let Some(key) = key.filter(|key| key.uri == uri) {
                        // Check both the decoded engine value and the encoded
                        // subsequent event, retaining evaluator ID and raw units.
                        assert_eq!(key.kind, expected_kind);
                        assert_eq!(key.id.0.to_string(), selected_id.as_ref().unwrap().as_str());
                        assert_eq!(key.handle, handle);
                        assert_eq!(transport.key_kind, Some(kind_name));
                        assert_eq!(transport.key_id.as_deref(), selected_id.as_deref());
                        assert_eq!(transport.original_uri, uri);
                        assert_eq!(transport.key_handle.as_ref(), Some(&handle));
                        boundaries.push(transport.kind);
                        match &request.kind {
                            ModuleRequestKind::QueryCachePolicy { .. } => {
                                probe
                                    .complete_module_cache_policy(
                                        token,
                                        selected_id.as_deref().unwrap(),
                                        false,
                                        0,
                                    )
                                    .unwrap();
                            }
                            ModuleRequestKind::ResolveKey { .. } => {
                                probe
                                    .complete_module_resolution(token, uri, uri, uri, 0)
                                    .unwrap();
                            }
                            ModuleRequestKind::LoadSource { .. } => {
                                probe
                                    .complete_module_source(token, "stable=7\n", "main", 0)
                                    .unwrap();
                            }
                            _ => unreachable!(),
                        }
                    } else {
                        assert!(
                            probe.complete_default_module(token, 0).unwrap(),
                            "unexpected request: {request:?}"
                        );
                    }
                }
                "value" => {
                    assert_eq!(event.value.unwrap(), vec![u16::from(b'7')]);
                    terminal = true;
                    break;
                }
                other => panic!("unexpected event {other}: {event:?}"),
            }
        }
        assert!(terminal, "probe did not finish for {kind_name}");
        assert_eq!(
            boundaries,
            ["query-cache-policy", "resolve-key", "load-source"]
        );
        probe.close();
        assert_eq!(probe.collect().slots, 0);
    }
}
