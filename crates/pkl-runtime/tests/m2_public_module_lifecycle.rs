//! Actual per-operation ModuleSource and synthetic query paths share one VM.
use pkl_runtime::{modules::*, probe::SourceProbe};

fn settle(
    probe: &mut SourceProbe,
    host: &mut impl FnMut(&ModuleRequest) -> Option<ModuleCompletion>,
) -> Result<String, String> {
    for _ in 0..100_000 {
        let event = probe.poll(0, 1).unwrap();
        probe.collect();
        match event.kind {
            "yield" => {}
            "module-request" => {
                let token = event.token.unwrap();
                let request = probe.module_request(&token).unwrap();
                let completion = host(&request)
                    .or_else(|| default_completion(&request))
                    .expect("configured Host");
                probe.complete_module(&token, completion, 0).unwrap();
            }
            "value" => return Ok(String::from_utf16(&event.value.unwrap()).unwrap()),
            "error" => return Err(String::from_utf16_lossy(&event.error.unwrap())),
            _ => panic!("unexpected {event:?}"),
        }
    }
    panic!("poll bound")
}
fn select(request: &ModuleRequest) -> ModuleCompletion {
    let ModuleRequestKind::SelectKey { id, uri, .. } = &request.kind else {
        panic!("factory")
    };
    ModuleCompletion::KeySelected(Ok(ModuleKey {
        id: *id,
        uri: uri.clone(),
        kind: ModuleKeyKind::Host,
        handle: format!("key-{}", id.0).into(),
    }))
}
fn resolve(request: &ModuleRequest, uri: Option<&str>) -> ModuleCompletion {
    let (ModuleRequestKind::ResolveKey {
        key,
        resolved_key_id,
        ..
    }
    | ModuleRequestKind::Resolve {
        key,
        resolved_key_id,
        ..
    }) = &request.kind
    else {
        panic!("resolution")
    };
    ModuleCompletion::KeyResolved(Ok(ResolvedModuleKey {
        id: *resolved_key_id,
        key: key.clone(),
        handle: "resolved".into(),
        module: ResolvedModule {
            original_uri: key.uri.clone(),
            resolved_uri: uri.map_or_else(|| key.uri.clone(), Into::into),
            source_uri: key.uri.clone(),
        },
    }))
}

#[test]
fn parse_failure_infers_selected_key_name_before_caching_the_diagnostic() {
    for (selected, fragment_paths, expected, getter_count) in [
        ("memo:/selected/input.pkl", false, "input", 1),
        (
            "memo:/selected/ignored.pkl#/folder/fragment.pkl",
            true,
            "fragment",
            1,
        ),
        (
            "jar:file:///package.jar!/folder/archive.pkl",
            false,
            "archive",
            0,
        ),
    ] {
        let mut probe = SourceProbe::new_module_evaluator("compact", false, false, false).unwrap();
        let (mut names, mut loads) = (0, 0);
        let mut previous = None;
        for expression in ["stable", "other"] {
            probe
                .evaluate_module_source("memo:/requested.pkl", None, expression, 0, None)
                .unwrap();
            let error = settle(&mut probe, &mut |request| {
                Some(match &request.kind {
                    ModuleRequestKind::SelectKey { id, .. } => {
                        ModuleCompletion::KeySelected(Ok(ModuleKey {
                            id: *id,
                            uri: selected.into(),
                            kind: ModuleKeyKind::Host,
                            handle: "selected".into(),
                        }))
                    }
                    ModuleRequestKind::QueryCachePolicy { .. } => {
                        ModuleCompletion::CachePolicy(Ok(true))
                    }
                    ModuleRequestKind::ResolveKey {
                        key,
                        resolved_key_id,
                        ..
                    } => ModuleCompletion::KeyResolved(Ok(ResolvedModuleKey {
                        id: *resolved_key_id,
                        key: key.clone(),
                        handle: "resolved".into(),
                        module: ResolvedModule {
                            original_uri: key.uri.clone(),
                            resolved_uri: "memo:/canonical.pkl".into(),
                            source_uri: "memo:/source.pkl".into(),
                        },
                    })),
                    ModuleRequestKind::LoadSource { .. } => {
                        loads += 1;
                        ModuleCompletion::Source(Ok(ModuleSource {
                            text: "module declared\nstable=7\nimport \"pkl:test\"\n".into(),
                            inferred_name: "wrongHostName".into(),
                        }))
                    }
                    ModuleRequestKind::QueryFragmentPaths { key } => {
                        names += 1;
                        assert_eq!(key.uri, selected);
                        ModuleCompletion::FragmentPaths(Ok(fragment_paths))
                    }
                    _ => return None,
                })
            })
            .unwrap_err();
            assert!(
                error.contains(&format!("at {expected} (memo:/source.pkl")),
                "{error}"
            );
            assert!(!error.contains("wrongHostName"));
            if let Some(previous) = &previous {
                assert_eq!(&error, previous);
            } else {
                previous = Some(error);
            }
        }
        assert_eq!((names, loads), (getter_count, 1));
        probe.close();
        assert_eq!(probe.collect().slots, 0);
    }
}

#[test]
fn primary_selection_runs_each_call_and_queries_have_fresh_keys_and_sites() {
    let mut p = SourceProbe::new_module_evaluator("compact", false, false, false).unwrap();
    let mut primary_select = 0;
    let mut primary_resolve = 0;
    let mut primary_load = 0;
    let mut query_keys = Vec::new();
    let mut edges = 0;
    for expression in [
        "stable",
        "stable",
        "import(\"memo:/child.pkl\").tag",
        "import(\"memo:/child.pkl\").tag",
        "output.text",
    ] {
        p.evaluate_module_source("memo:/main.pkl", None, expression, 0, None)
            .unwrap();
        let value = settle(&mut p, &mut |request| {
            Some(match &request.kind {
                ModuleRequestKind::SelectKey { phase, .. } => {
                    if *phase == SelectionPhase::Primary {
                        primary_select += 1;
                    }
                    select(request)
                }
                ModuleRequestKind::QueryCachePolicy { .. } => {
                    ModuleCompletion::CachePolicy(Ok(true))
                }
                ModuleRequestKind::ResolveKey {
                    phase: ResolutionPhase::Primary,
                    ..
                } => {
                    primary_resolve += 1;
                    resolve(request, Some("file:///canonical/main.pkl"))
                }
                ModuleRequestKind::ResolveKey {
                    phase: ResolutionPhase::Query,
                    key,
                    ..
                } => {
                    query_keys.push(key.id);
                    return None;
                }
                ModuleRequestKind::Resolve { .. } => resolve(request, None),
                ModuleRequestKind::CheckImport { importer, .. } => {
                    edges += 1;
                    assert_eq!(importer.resolved_uri, "repl:text");
                    return None;
                }
                ModuleRequestKind::LoadSource { resolved, .. } => {
                    ModuleCompletion::Source(Ok(ModuleSource {
                        text: if resolved.original_uri == "memo:/main.pkl" {
                            primary_load += 1;
                            "stable=7\noutput { text=\"OUT\" }\n"
                        } else {
                            "tag=99\n"
                        }
                        .into(),
                        inferred_name: "ignored".into(),
                    }))
                }
                _ => return None,
            })
        })
        .unwrap();
        assert_eq!(
            value,
            if expression == "output.text" {
                "OUT"
            } else if expression == "stable" {
                "7"
            } else {
                "99"
            }
        );
    }
    assert_eq!(
        (primary_select, primary_resolve, primary_load, edges),
        (5, 1, 1, 2)
    );
    assert_eq!(query_keys.len(), 4);
    query_keys.sort();
    query_keys.dedup();
    assert_eq!(query_keys.len(), 4);
    p.close();
    assert_eq!(p.collect().slots, 0);
}

#[test]
fn supplied_wrappers_cache_but_repl_text_primary_is_uncached() {
    let mut p = SourceProbe::new_module_evaluator("compact", false, false, false).unwrap();
    let mut selections = 0;
    for (uri, contents, expression, expected) in [
        (
            "memo:/main.pkl",
            "value=\"one\"\noutput { text=\"ONE\" }",
            "value",
            "one",
        ),
        (
            "memo:/main.pkl",
            "value=\"two\"\noutput { text=\"TWO\" }",
            "value",
            "one",
        ),
        (
            "memo:/main.pkl",
            "value=\"two\"\noutput { text=\"TWO\" }",
            "output.text",
            "ONE",
        ),
        ("repl:text", "value=\"one\"", "value", "one"),
        ("repl:text", "value=\"two\"", "value", "two"),
    ] {
        p.evaluate_module_source(uri, Some(contents), expression, 0, None)
            .unwrap();
        let value = settle(&mut p, &mut |request| match &request.kind {
            ModuleRequestKind::SelectKey { .. } => {
                selections += 1;
                Some(select(request))
            }
            ModuleRequestKind::QueryCachePolicy { .. } | ModuleRequestKind::LoadSource { .. } => {
                panic!("wrapper/synthetic work must use its actual local text and getter")
            }
            ModuleRequestKind::ResolveKey { key, .. } if key.kind != ModuleKeyKind::Synthetic => {
                panic!("wrapper must not delegate resolution")
            }
            _ => None,
        })
        .unwrap();
        assert_eq!(value, expected);
    }
    assert_eq!(selections, 3);
    p.close();
    assert_eq!(p.collect().slots, 0);
}

#[test]
fn query_policy_denial_precedes_query_parse_and_output_text_uses_direct_operation() {
    let mut p = SourceProbe::new_module_evaluator("compact", false, false, false).unwrap();
    for expression in ["(", "output.text"] {
        p.evaluate_module_source(
            "memo:/main.pkl",
            Some("output { text=\"OK\" }"),
            expression,
            0,
            None,
        )
        .unwrap();
        let result = settle(&mut p, &mut |request| match &request.kind {
            ModuleRequestKind::SelectKey { .. } => Some(select(request)),
            ModuleRequestKind::ResolveKey {
                phase: ResolutionPhase::Query,
                ..
            } => Some(ModuleCompletion::Resolved(Err(
                ModuleHostError::SecurityManagerException {
                    message: Some("query denied".into()),
                },
            ))),
            _ => None,
        });
        if expression == "(" {
            assert!(result.unwrap_err().contains("query denied"));
        } else {
            assert_eq!(result.unwrap(), "OK");
        }
    }
}

#[test]
fn declaration_resolve_uri_override_runs_for_absolute_authored_imports() {
    let mut p = SourceProbe::new_module_evaluator("compact", false, false, false).unwrap();
    p.evaluate_module_source("memo:/main.pkl", None, "child().tag", 0, None)
        .unwrap();
    let mut overrides = 0;
    let result = settle(&mut p, &mut |request| {
        Some(match &request.kind {
            ModuleRequestKind::SelectKey { .. } => select(request),
            ModuleRequestKind::QueryCachePolicy { .. } => ModuleCompletion::CachePolicy(Ok(true)),
            ModuleRequestKind::ResolveKey {
                phase: ResolutionPhase::Primary,
                ..
            } => resolve(request, Some("file:///canonical/main.pkl")),
            ModuleRequestKind::Resolve { .. } => resolve(request, None),
            ModuleRequestKind::ResolveImportUri { key, authored_uri } => {
                assert_eq!(key.uri, "memo:/main.pkl");
                assert_eq!(authored_uri, "file:///authored.pkl");
                overrides += 1;
                ModuleCompletion::ImportUri(Ok("memo:/redirected.pkl".into()))
            }
            ModuleRequestKind::CheckImport {
                importer,
                import_uri,
            } => {
                assert_eq!(importer.resolved_uri, "file:///canonical/main.pkl");
                assert_eq!(import_uri, "memo:/redirected.pkl");
                return None;
            }
            ModuleRequestKind::LoadSource { resolved, .. } => {
                ModuleCompletion::Source(Ok(ModuleSource {
                    text: if resolved.original_uri == "memo:/main.pkl" {
                        "function child()=import(\"file:///authored.pkl\")\n"
                    } else {
                        "tag=99\n"
                    }
                    .into(),
                    inferred_name: "ignored".into(),
                }))
            }
            _ => return None,
        })
    })
    .unwrap();
    assert_eq!(result, "99");
    assert_eq!(overrides, 1);
}

#[test]
fn public_query_callback_reentry_restores_parent_and_close_releases_every_root() {
    let mut p = SourceProbe::new_module_evaluator("compact", false, false, false).unwrap();
    p.evaluate_module_source(
        "repl:text",
        Some("value=read(\"prop:x\")"),
        "value",
        0,
        None,
    )
    .unwrap();
    let token = loop {
        let event = p.poll(0, 1).unwrap();
        p.collect();
        match event.kind {
            "yield" => {}
            "module-request" => assert!(
                p.complete_default_module(event.token.as_deref().unwrap(), 0)
                    .unwrap()
            ),
            "read" => break event.token.unwrap(),
            _ => panic!("unexpected {event:?}"),
        }
    };
    p.reenter_module_source(&token, "repl:text", Some("value=9"), "value", 0, None)
        .unwrap();
    assert_eq!(settle(&mut p, &mut |_| None).unwrap(), "9");
    let pending = p.poll(0, 1).unwrap();
    assert_eq!(pending.token.as_deref(), Some(token.as_str()));
    p.complete_read_text(&token, "parent".into(), 0).unwrap();
    assert_eq!(settle(&mut p, &mut |_| None).unwrap(), "parent");
    p.close();
    assert_eq!(p.collect().slots, 0);
    assert!(p.complete_read_text(&token, "late".into(), 0).is_err());
}

#[test]
fn supplied_wrapper_delegates_capability_getters_with_actual_underlying_key() {
    let mut p = SourceProbe::new_module_evaluator("compact", false, false, false).unwrap();
    p.evaluate_module_source(
        "memo:/dir/main.pkl",
        Some("value=read(\".../data\")"),
        "value",
        0,
        None,
    )
    .unwrap();
    let mut delegate = None;
    let error = settle(&mut p, &mut |request| {
        Some(match &request.kind {
            ModuleRequestKind::SelectKey { id, .. } => {
                delegate = Some(*id);
                select(request)
            }
            ModuleRequestKind::QueryFragmentPaths { key } => {
                assert_eq!(key.kind, ModuleKeyKind::SuppliedText);
                return None;
            }
            ModuleRequestKind::Capabilities { key: Some(key), .. } => {
                assert_eq!(Some(key.id), delegate);
                assert_eq!(key.kind, ModuleKeyKind::Host);
                ModuleCompletion::Capabilities(Err(ModuleHostError::Denied(
                    "capability callback".into(),
                )))
            }
            _ => return None,
        })
    })
    .unwrap_err();
    assert!(error.contains("capability callback"));
}

#[test]
fn completed_query_sites_release_old_uncached_receivers_after_collection() {
    let mut p = SourceProbe::new_module_evaluator("compact", false, false, false).unwrap();
    let mut baseline = None;
    let mut loads = 0;
    for iteration in 0..12 {
        p.evaluate_module_source(
            "memo:/main.pkl",
            Some("stable=7"),
            "import(\"memo:/child.pkl\").tag",
            0,
            None,
        )
        .unwrap();
        let value = settle(&mut p, &mut |request| {
            Some(match &request.kind {
                ModuleRequestKind::SelectKey { .. } => select(request),
                ModuleRequestKind::QueryCachePolicy { .. } => {
                    ModuleCompletion::CachePolicy(Ok(false))
                }
                ModuleRequestKind::Resolve { .. } => resolve(request, None),
                ModuleRequestKind::LoadSource { .. } => {
                    loads += 1;
                    ModuleCompletion::Source(Ok(ModuleSource {
                    text: format!("class C {{ value={loads} }}\ntypealias A=C\nx: A=new C {{}}\ntag=x.value").into(),
                        inferred_name: "ignored".into(),
                    }))
                }
                _ => return None,
            })
        })
        .unwrap();
        assert_eq!(value, (iteration + 1).to_string());
        let stats = p.collect();
        let live = (stats.live, stats.executable_live);
        if let Some(expected) = baseline {
            assert_eq!(
                live, expected,
                "completed query {iteration} retained heap state"
            );
        } else {
            baseline = Some(live);
        }
    }
    assert_eq!(loads, 12);
    p.close();
    assert_eq!(p.collect().slots, 0);
}

#[test]
fn external_synthetic_keys_keep_their_policy_and_supplied_sources_wrap_them() {
    let mut p = SourceProbe::new_module_evaluator("compact", false, false, false).unwrap();
    let mut policies = 0;
    let mut loads = 0;
    for (contents, expected) in [
        (None, "underlying"),
        (None, "underlying"),
        (Some("value=\"supplied\""), "underlying"),
    ] {
        p.evaluate_module_source("memo:/synthetic.pkl", contents, "value", 0, None)
            .unwrap();
        let value = settle(&mut p, &mut |request| {
            Some(match &request.kind {
                ModuleRequestKind::SelectKey { id, uri, .. } => {
                    ModuleCompletion::KeySelected(Ok(ModuleKey {
                        id: *id,
                        uri: uri.clone(),
                        kind: ModuleKeyKind::Synthetic,
                        handle: "external".into(),
                    }))
                }
                ModuleRequestKind::QueryCachePolicy { .. } => {
                    policies += 1;
                    ModuleCompletion::CachePolicy(Ok(true))
                }
                ModuleRequestKind::ResolveKey {
                    phase: ResolutionPhase::Primary,
                    ..
                } => resolve(request, None),
                ModuleRequestKind::LoadSource { .. } => {
                    loads += 1;
                    ModuleCompletion::Source(Ok(ModuleSource {
                        text: "value=\"underlying\"".into(),
                        inferred_name: "ignored".into(),
                    }))
                }
                _ => return None,
            })
        })
        .unwrap();
        assert_eq!(value, expected);
    }
    assert_eq!((policies, loads), (2, 1));
}

#[test]
fn wrapper_triple_search_uses_its_own_path_policy_and_the_delegates_element_callback() {
    let mut p = SourceProbe::new_module_evaluator("compact", false, false, false).unwrap();
    p.evaluate_module_source(
        "memo:/outer/dir/main.pkl#/inner/dir/main.pkl",
        Some("function child()=import(\".../child.pkl\")"),
        "child().tag",
        0,
        None,
    )
    .unwrap();
    let mut delegate = None;
    let mut searched = Vec::new();
    let value = settle(&mut p, &mut |request| {
        Some(match &request.kind {
            ModuleRequestKind::SelectKey { id, phase, .. } => {
                if *phase == SelectionPhase::Primary {
                    delegate = Some(*id);
                }
                select(request)
            }
            ModuleRequestKind::Capabilities { key: Some(key), .. } => {
                assert_eq!(Some(key.id), delegate);
                ModuleCompletion::Capabilities(Ok(ModuleCapabilities {
                    local: true,
                    hierarchical: true,
                    fragment_paths: true,
                }))
            }
            ModuleRequestKind::HasElement {
                key: Some(key),
                element_uri,
                ..
            } => {
                assert_eq!(Some(key.id), delegate);
                searched.push(element_uri.to_string());
                ModuleCompletion::HasElement(Ok(true))
            }
            ModuleRequestKind::QueryCachePolicy { .. } => ModuleCompletion::CachePolicy(Ok(true)),
            ModuleRequestKind::Resolve { .. } => resolve(request, None),
            ModuleRequestKind::LoadSource { .. } => ModuleCompletion::Source(Ok(ModuleSource {
                text: "tag=99".into(),
                inferred_name: "ignored".into(),
            })),
            _ => return None,
        })
    })
    .unwrap();
    assert_eq!(value, "99");
    assert_eq!(searched, ["memo:/outer/child.pkl"]);
}

#[test]
fn supplied_wrapper_uses_selected_memo_uri_and_skips_delegate_overrides() {
    // Actual Stage49 Java05 contents profile: the wrapper inherits URI
    // resolution and uses the selected key URI, not request or resolved URI.
    let mut p = SourceProbe::new_module_evaluator("compact", false, false, false).unwrap();
    let source = "function relative() = import(\"./child.pkl\")\nfunction absolute() = import(\"memo:/authored-absolute.pkl\")\nstable = 7\n";
    let mut loads = Vec::new();
    for (expression, expected) in [
        ("stable", "7"),
        ("relative().tag", "default-relative"),
        ("absolute().tag", "default-absolute"),
        ("relative().tag", "default-relative"),
    ] {
        p.evaluate_module_source(
            "memo:/holder-request.pkl",
            Some(source),
            expression,
            0,
            None,
        )
        .unwrap();
        let result = settle(&mut p, &mut |request| {
            Some(match &request.kind {
                ModuleRequestKind::SelectKey { id, uri, .. } => {
                    ModuleCompletion::KeySelected(Ok(ModuleKey {
                        id: *id,
                        uri: if uri == "memo:/holder-request.pkl" {
                            "memo:/selected/holder.pkl".into()
                        } else {
                            uri.clone()
                        },
                        kind: ModuleKeyKind::Host,
                        handle: "actual-delegate".into(),
                    }))
                }
                ModuleRequestKind::ResolveImportUri { .. } => {
                    panic!("wrapper must not call the delegate override")
                }
                ModuleRequestKind::QueryCachePolicy { .. } => {
                    ModuleCompletion::CachePolicy(Ok(true))
                }
                ModuleRequestKind::Resolve { .. } => resolve(request, None),
                ModuleRequestKind::LoadSource { resolved, .. } => {
                    loads.push(resolved.original_uri.to_string());
                    ModuleCompletion::Source(Ok(ModuleSource {
                        text: match resolved
                            .original_uri
                            .as_str()
                            .expect("fixture URI must be scalar")
                        {
                            "memo:/selected/child.pkl" => "tag=\"default-relative\"",
                            "memo:/authored-absolute.pkl" => "tag=\"default-absolute\"",
                            uri => panic!("wrong wrapper resolution {uri}"),
                        }
                        .into(),
                        inferred_name: "ignored".into(),
                    }))
                }
                _ => return None,
            })
        })
        .unwrap();
        assert_eq!(result, expected);
    }
    assert_eq!(
        loads,
        ["memo:/selected/child.pkl", "memo:/authored-absolute.pkl"]
    );
}

#[test]
fn builtin_ref_after_custom_selection_inherits_module_to_string() {
    let mut p = SourceProbe::new_module_evaluator("compact", false, false, false).unwrap();
    for (custom, expression, expected) in [
        (true, "import(\"pkl:ref\").label", "custom-ref"),
        (
            false,
            "import(\"pkl:ref\").toString()",
            "new ModuleClass {}",
        ),
        (true, "import(\"pkl:ref\").label", "custom-ref"),
    ] {
        p.evaluate_module_source("memo:/main.pkl", Some("stable=7"), expression, 0, None)
            .unwrap();
        let result = settle(&mut p, &mut |request| {
            Some(match &request.kind {
                ModuleRequestKind::SelectKey { uri, .. } if uri.starts_with("memo:") || custom => {
                    select(request)
                }
                ModuleRequestKind::QueryCachePolicy { .. } => {
                    ModuleCompletion::CachePolicy(Ok(true))
                }
                ModuleRequestKind::Resolve { .. } => resolve(request, None),
                ModuleRequestKind::LoadSource { .. } => {
                    ModuleCompletion::Source(Ok(ModuleSource {
                        text: "label=\"custom-ref\"".into(),
                        inferred_name: "ignored".into(),
                    }))
                }
                _ => return None,
            })
        })
        .unwrap();
        assert_eq!(result, expected);
    }
}

#[test]
fn cached_custom_json_missing_property_uses_module_method_suggestions() {
    // Exact Java01 error bytes for the custom-first ordinary URI cache.
    let mut p = SourceProbe::new_module_evaluator("compact", false, false, false).unwrap();
    let source =
        "function one() = import(\"pkl:json\")\nfunction two() = import(\"pkl:json\")\nstable=7\n";
    for (custom, expression) in [(true, "one().label"), (false, "two().Property.toString()")] {
        p.evaluate_module_source("memo:/main.pkl", Some(source), expression, 0, None)
            .unwrap();
        let result = settle(&mut p, &mut |request| {
            Some(match &request.kind {
                ModuleRequestKind::SelectKey { uri, .. } if uri.starts_with("memo:") || custom => {
                    select(request)
                }
                ModuleRequestKind::QueryCachePolicy { .. } => {
                    ModuleCompletion::CachePolicy(Ok(true))
                }
                ModuleRequestKind::Resolve { .. } => resolve(request, None),
                ModuleRequestKind::LoadSource { .. } => {
                    ModuleCompletion::Source(Ok(ModuleSource {
                        text: "label=\"custom-json\"".into(),
                        inferred_name: "ignored".into(),
                    }))
                }
                _ => return None,
            })
        });
        if custom {
            assert_eq!(result.unwrap(), "custom-json");
        } else {
            assert_eq!(
                result.unwrap_err(),
                "–– Pkl Error ––\nCannot find property `Property` in module `json`.\n\n1 | two().Property.toString()\n    ^^^^^^^^^^^^^^\nat  (repl:text)\n\nDid you mean any of the following?\nhasProperty(name)\ngetProperty(name)\n"
            );
        }
    }
}
