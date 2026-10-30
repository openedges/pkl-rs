//! Native counterparts for retained typed transport controls. No Java parity
//! is inferred from codec/phase checks or constructed lifecycle observations.
use pkl_runtime::{
    glob::PathElement,
    modules::{
        self, ModuleCompletion, ModuleKey, ModuleKeyKind, ModuleRequestKind, ResolvedModule,
        ResolvedModuleKey,
    },
    probe::{self, SourceProbe, SourceTextEvent},
    source::SourceText,
    value::{PklString, RuntimeError},
};

fn next(session: &mut SourceProbe, now: u32) -> SourceTextEvent {
    for _ in 0..100_000 {
        let event = session.poll_text(now, 1).unwrap();
        session.collect();
        match event.kind {
            "yield" => (),
            "module-request" => assert!(
                session
                    .complete_default_module(event.token.as_deref().unwrap(), now)
                    .unwrap()
            ),
            _ => return event,
        }
    }
    panic!("bounded native control did not settle")
}
fn make(timeout: Option<u32>) -> (SourceProbe, SourceTextEvent) {
    let mut session = SourceProbe::new_expressions_source(
        "marker = true\n",
        "repl:text",
        "globTransport",
        &[
            SourceText::from("read*(\"custom:**\").keys.toList().join(\"|\")"),
            SourceText::from("read*(\"custom:inner*\").keys.toList().join(\"|\")"),
        ],
        "compact",
        false,
        false,
        false,
    )
    .unwrap();
    session.start_path(0, 0, timeout).unwrap();
    let event = next(&mut session, 0);
    assert_eq!(event.glob.as_ref().unwrap().kind, "select-resource");
    (session, event)
}
fn listing(session: &mut SourceProbe, selected: SourceTextEvent) -> SourceTextEvent {
    let reader = selected.glob.as_ref().unwrap().reader_id.clone();
    session
        .complete_glob_reader(
            selected.token.as_deref().unwrap(),
            &reader,
            "custom",
            "transport-reader",
            0,
        )
        .unwrap();
    let event = next(session, 0);
    assert_eq!(event.glob.as_ref().unwrap().kind, "is-globbable");
    assert_eq!(event.glob.as_ref().unwrap().reader_id, reader);
    session
        .complete_glob_boolean(event.token.as_deref().unwrap(), "is-globbable", true, 0)
        .unwrap();
    let event = next(session, 0);
    assert_eq!(event.glob.as_ref().unwrap().kind, "is-hierarchical");
    session
        .complete_glob_boolean(event.token.as_deref().unwrap(), "is-hierarchical", false, 0)
        .unwrap();
    let event = next(session, 0);
    let descriptor = event.glob.as_ref().unwrap();
    assert_eq!(descriptor.kind, "list-elements");
    assert_eq!(descriptor.reader_id, reader);
    assert_eq!(descriptor.reader_scheme, Some("custom".into()));
    assert_eq!(descriptor.reader_handle, Some("transport-reader".into()));
    event
}

#[test]
fn production_codec_keeps_large_ids_full_names_duplicates_order_and_flags() {
    let names = [
        vec![0x61],
        vec![0xd800],
        vec![0xdc00],
        vec![0xd83d, 0xde00],
        vec![0x61, 0, 0xdc00, 0x62],
        vec![0x61],
    ];
    let records = probe::glob_transport_records(
        names
            .iter()
            .enumerate()
            .map(|(index, name)| PathElement {
                name: PklString::from_utf16(name.clone()),
                is_directory: index % 2 == 1,
            })
            .collect(),
    );
    assert_eq!(records.len(), names.len());
    for (index, event) in records.into_iter().enumerate() {
        let descriptor = event.glob.unwrap();
        assert_eq!(descriptor.id, "18446744073709551615");
        assert_eq!(descriptor.reader_id, "9007199254740993");
        assert_eq!(descriptor.kind, "list-elements");
        assert_eq!(descriptor.uri, Some("codec:**".into()));
        assert_eq!(descriptor.reader_kind, "resource");
        assert_eq!(descriptor.reader_scheme, Some("codec".into()));
        assert_eq!(descriptor.reader_handle, Some("codec-reader".into()));
        assert_eq!(descriptor.reader_uri, None);
        assert_eq!(descriptor.key_kind, None);
        assert_eq!(event.token, Some(format!("codec:{index}")));
        assert_eq!(event.value, Some(names[index].clone()));
        assert_eq!(
            event.reason,
            Some(if index % 2 == 1 { "directory" } else { "file" })
        );
    }
}

#[test]
fn wrong_completion_identity_phase_and_unavailable_default_keep_pending_token() {
    let (mut session, selected) = make(None);
    let token = selected.token.as_ref().unwrap().clone();
    let reader = selected.glob.as_ref().unwrap().reader_id.clone();
    assert!(
        session
            .complete_glob_boolean(&token, "is-globbable", true, 0)
            .is_err()
    );
    assert_eq!(next(&mut session, 0).token, Some(token.clone()));
    assert!(
        session
            .complete_glob_boolean(&token, "unknown", true, 0)
            .is_err()
    );
    assert_eq!(next(&mut session, 0).token, Some(token.clone()));
    assert!(
        session
            .complete_glob_reader(&token, "18446744073709551615", "custom", "wrong", 0)
            .is_err()
    );
    assert_eq!(next(&mut session, 0).token, Some(token.clone()));
    assert!(
        session
            .complete_glob_reader(&token, &reader, "other", "wrong", 0)
            .is_err()
    );
    assert_eq!(next(&mut session, 0).token, Some(token.clone()));
    assert!(
        session
            .reject_glob(&token, "io", Some("wrong-phase".into()), 0)
            .is_err()
    );
    assert_eq!(next(&mut session, 0).token, Some(token.clone()));
    assert!(session.set_glob_test_mode(true).is_err());
    assert_eq!(next(&mut session, 0).token, Some(token.clone()));
    session
        .complete_glob_reader(&token, &reader, "custom", "transport-reader", 0)
        .unwrap();
    let pending = next(&mut session, 0);
    assert!(
        session
            .complete_glob_reader(&token, &reader, "custom", "transport-reader", 0)
            .is_err()
    );
    assert_eq!(next(&mut session, 0).token, pending.token);
    assert!(
        session
            .reject_glob(
                pending.token.as_deref().unwrap(),
                "reader-not-found",
                None,
                0
            )
            .is_err()
    );
    assert_eq!(next(&mut session, 0).token, pending.token);
    session
        .complete_glob_boolean(pending.token.as_deref().unwrap(), "is-globbable", true, 0)
        .unwrap();
    let hierarchical = next(&mut session, 0);
    session
        .complete_glob_boolean(
            hierarchical.token.as_deref().unwrap(),
            "is-hierarchical",
            false,
            0,
        )
        .unwrap();
    let pending = next(&mut session, 0);
    assert_eq!(pending.glob.as_ref().unwrap().kind, "list-elements");
    assert!(
        !session
            .complete_default_glob(pending.token.as_deref().unwrap(), 0)
            .unwrap()
    );
    assert_eq!(next(&mut session, 0).token, pending.token);
    session
        .complete_glob_elements(pending.token.as_deref().unwrap(), vec![], 0)
        .unwrap();
    let event = next(&mut session, 0);
    assert_eq!(event.kind, "value");
    assert_eq!(event.value, Some(vec![]));
    session.close();
    assert_eq!(session.heap_slots(), 0);
}

#[test]
fn actual_raw_listing_outputs_survive_collection_and_nested_abort_drains_once() {
    let (mut session, selected) = make(None);
    let pending = listing(&mut session, selected);
    let names = [
        vec![0xd800],
        vec![0xdfff],
        vec![0x78, 0xd800, 0x79],
        vec![0xd83d, 0xde00],
    ];
    session
        .complete_glob_elements(
            pending.token.as_deref().unwrap(),
            names
                .iter()
                .map(|name| PathElement {
                    name: PklString::from_utf16(name.clone()),
                    is_directory: false,
                })
                .collect(),
            0,
        )
        .unwrap();
    let event = next(&mut session, 0);
    assert_eq!(event.kind, "value");
    let mut expected = vec![];
    // GlobResolver.resolveOpaqueGlob sorts by PathElement.comparator, whose
    // same-kind names use Java String.compareTo (unsigned UTF-16 code units).
    // The transport codec test above separately preserves provider input order.
    let expected_names = [&names[2], &names[0], &names[3], &names[1]];
    for (index, name) in expected_names.into_iter().enumerate() {
        if index != 0 {
            expected.push(0x7c);
        }
        expected.extend("custom:".encode_utf16());
        expected.extend(name);
    }
    assert_eq!(event.value.as_ref().unwrap(), &expected);
    session.collect();
    session.close();
    session.collect();
    assert_eq!(event.value.as_ref().unwrap(), &expected);
    assert_eq!(session.heap_slots(), 0);
    let (mut session, outer) = make(None);
    session
        .reenter_path(outer.token.as_deref().unwrap(), 1, 0, None)
        .unwrap();
    let inner = next(&mut session, 0);
    assert_ne!(outer.token, inner.token);
    session.cancel();
    assert_eq!(
        session.drain_cancelled_globs(),
        vec![outer.token.unwrap(), inner.token.unwrap()]
    );
    assert!(session.drain_cancelled_globs().is_empty());
    session.close();
    assert_eq!(session.heap_slots(), 0);
    for close in [false, true] {
        let (mut session, event) = make(None);
        if close {
            session.close();
        } else {
            session.cancel();
        }
        assert_eq!(session.drain_cancelled_globs(), vec![event.token.unwrap()]);
        assert!(session.drain_cancelled_globs().is_empty());
        session.close();
    }
    let (mut session, event) = make(Some(1));
    assert_eq!(next(&mut session, 1).kind, "error");
    assert_eq!(session.drain_cancelled_globs(), vec![event.token.unwrap()]);
    assert!(session.drain_cancelled_globs().is_empty());
    session.close();
}

#[test]
fn native_transport19_emits_complete_error_units_for_required_cross_target_comparison() {
    let (mut session, selected) = make(None);
    session
        .complete_glob_reader(
            selected.token.as_deref().unwrap(),
            &selected.glob.unwrap().reader_id,
            "custom",
            "transport-reader",
            0,
        )
        .unwrap();
    let pending = next(&mut session, 0);
    session
        .reject_glob(
            pending.token.as_deref().unwrap(),
            "io",
            Some(PklString::from_utf16(vec![0x65e5, 0xd800, 0, 0xdc00])),
            0,
        )
        .unwrap();
    let event = next(&mut session, 0);
    assert_eq!(event.kind, "error");
    let units = event.error.expect("complete diagnostic code units");
    // Root must compare this entire array against both actual platform control19
    // arrays. Its emission alone is not cross-target or Java compatibility.
    println!("TRANSPORT19_INPUT={:?}", [0x65e5_u16, 0xd800, 0, 0xdc00]);
    println!("TRANSPORT19={units:?}");
    println!("TRANSPORT19_DISPOSITION={:?}", event.disposition);
    session.close();
    assert_eq!(session.heap_slots(), 0);
}

#[derive(Default)]
struct ImportControl {
    key: Option<ModuleKey>,
    resolved_key: Option<ResolvedModuleKey>,
    phases: Vec<&'static str>,
}
fn complete_non_import(session: &mut SourceProbe, token: &str, control: &mut ImportControl) {
    let request = session.module_request(token).unwrap();
    let completion = match &request.kind {
        ModuleRequestKind::SelectKey { id, uri, phase } => {
            assert!(control.phases.is_empty());
            assert_eq!(*phase, modules::SelectionPhase::Primary);
            assert_eq!(uri, &PklString::from("gmain:/parent.pkl"));
            let key = ModuleKey {
                id: *id,
                uri: uri.clone(),
                kind: ModuleKeyKind::Host,
                handle: "module-control".into(),
            };
            control.key = Some(key.clone());
            control.phases.push("select-key");
            ModuleCompletion::KeySelected(Ok(key))
        }
        ModuleRequestKind::QueryCachePolicy { key } => {
            assert_eq!(control.phases, ["select-key"]);
            assert_eq!(Some(key), control.key.as_ref());
            control.phases.push("cache-policy");
            ModuleCompletion::CachePolicy(Ok(true))
        }
        ModuleRequestKind::ResolveKey {
            key,
            resolved_key_id,
            phase,
        } => {
            assert_eq!(control.phases, ["select-key", "cache-policy"]);
            assert_eq!(*phase, modules::ResolutionPhase::Primary);
            assert_eq!(Some(key), control.key.as_ref());
            let resolved = ResolvedModuleKey {
                id: *resolved_key_id,
                key: key.clone(),
                handle: key.handle.clone(),
                module: ResolvedModule {
                    original_uri: key.uri.clone(),
                    resolved_uri: key.uri.clone(),
                    source_uri: key.uri.clone(),
                },
            };
            control.resolved_key = Some(resolved.clone());
            control.phases.push("resolve-key");
            ModuleCompletion::KeyResolved(Ok(resolved))
        }
        ModuleRequestKind::LoadSource { key, resolved } => {
            assert_eq!(
                control.phases,
                ["select-key", "cache-policy", "resolve-key"]
            );
            assert_eq!(Some(key), control.resolved_key.as_ref());
            assert_eq!(resolved, &key.module);
            control.phases.push("load-source");
            ModuleCompletion::Source(Ok(modules::ModuleSource {
                text: "result = import(\"gmod:child\").name\n".into(),
                inferred_name: "parent".into(),
            }))
        }
        ModuleRequestKind::QueryFragmentPaths { key } => {
            assert_eq!(
                control.phases,
                ["select-key", "cache-policy", "resolve-key", "load-source"]
            );
            assert_eq!(Some(key), control.key.as_ref());
            control.phases.push("fragment-paths");
            ModuleCompletion::FragmentPaths(Ok(false))
        }
        other => panic!("unconfigured import control effect: {other:?}"),
    };
    session.complete_module(token, completion, 0).unwrap();
}
#[test]
fn reject_import_uri_checks_key_and_failure_phase_before_consuming_request() {
    for kind in ["io", "security-manager-exception"] {
        let mut session =
            SourceProbe::new_module_evaluator("compact", false, false, false).unwrap();
        // Supplied contents create Java's CachedModuleKey wrapper, which does
        // not delegate resolveUri to the custom key. Load the declaration from
        // this Host provider so its own resolver is the intended failure site.
        session
            .evaluate_module_source_text("gmain:/parent.pkl", None, "result", 0, None)
            .unwrap();
        let mut control = ImportControl::default();
        let mut rejected = false;
        let mut terminal = false;
        for _ in 0..100_000 {
            let event = session.poll_text(0, 64).unwrap();
            session.collect();
            match event.kind {
                "yield" => (),
                "module-request" => {
                    let token = event.token.unwrap();
                    let request = session.module_request(&token).unwrap();
                    if let ModuleRequestKind::ResolveImportUri { key, authored_uri } = &request.kind
                    {
                        assert!(!rejected);
                        assert_eq!(
                            control.phases,
                            [
                                "select-key",
                                "cache-policy",
                                "resolve-key",
                                "load-source",
                                "fragment-paths"
                            ]
                        );
                        assert_eq!(Some(key), control.key.as_ref());
                        assert_eq!(authored_uri, &PklString::from("gmod:child"));
                        control.phases.push("resolve-import-uri");
                        let id = key.id.0.to_string();
                        assert_eq!(event.module.unwrap().key_id.as_deref(), Some(id.as_str()));
                        assert!(
                            session
                                .reject_import_uri(&token, "18446744073709551615", kind, None, 0)
                                .is_err()
                        );
                        assert_eq!(session.module_request(&token).unwrap().id, request.id);
                        for invalid_kind in ["no-loader", "not-found"] {
                            assert!(
                                session
                                    .reject_import_uri(&token, &id, invalid_kind, None, 0)
                                    .is_err()
                            );
                            assert_eq!(session.module_request(&token).unwrap().id, request.id);
                        }
                        session
                            .reject_import_uri(&token, &id, kind, Some("fixture".into()), 0)
                            .unwrap();
                        rejected = true;
                    } else {
                        complete_non_import(&mut session, &token, &mut control);
                    }
                }
                "error" => {
                    assert!(rejected);
                    assert!(event.error.is_some());
                    terminal = true;
                    break;
                }
                other => panic!("unexpected import control result: {other}"),
            }
        }
        assert!(terminal);
        assert_eq!(
            control.phases,
            [
                "select-key",
                "cache-policy",
                "resolve-key",
                "load-source",
                "fragment-paths",
                "resolve-import-uri"
            ]
        );
        session.close();
        assert_eq!(session.heap_slots(), 0);
    }
    // Native bool arguments are Rust bools; JS strict-type controls09/20 remain
    // real platform obligations, not replaced by this compile-time signature.
    let _: fn(&mut SourceProbe, &str, &str, bool, u32) -> Result<(), RuntimeError> =
        SourceProbe::complete_glob_boolean;
}
