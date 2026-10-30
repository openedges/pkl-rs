//! ModuleKey object identity, cache policy and source ownership regressions.
use pkl_runtime::{modules::*, probe::SourceProbe, value::PklString};

fn probe(text: &str, expressions: &[&str]) -> SourceProbe {
    SourceProbe::new_expressions_text(
        text,
        "file:///keys/main.pkl",
        "main",
        &expressions.iter().map(|s| (*s).into()).collect::<Vec<_>>(),
        "compact",
        false,
        false,
        true,
    )
    .unwrap()
}
fn run(
    probe: &mut SourceProbe,
    index: u32,
    host: &mut impl FnMut(&ModuleRequest) -> Option<ModuleCompletion>,
) -> Result<String, String> {
    probe.start_path(index, 0, None).unwrap();
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
                    .expect("configured request");
                probe.complete_module(&token, completion, 0).unwrap();
            }
            "value" => return Ok(String::from_utf16(&event.value.unwrap()).unwrap()),
            "error" => return Err(PklString::from_utf16(event.error.unwrap()).to_java_utf8()),
            _ => panic!("unexpected event {event:?}"),
        }
    }
    panic!("poll bound")
}
fn key(request: &ModuleRequest, uri: Option<&str>) -> ModuleCompletion {
    let ModuleRequestKind::SelectKey {
        id, uri: original, ..
    } = &request.kind
    else {
        panic!("selection")
    };
    ModuleCompletion::KeySelected(Ok(ModuleKey {
        id: *id,
        uri: uri.map_or_else(|| original.clone(), Into::into),
        kind: ModuleKeyKind::Host,
        handle: format!("key-{}", id.0).into(),
    }))
}
fn resolved(request: &ModuleRequest, uri: Option<&str>) -> ModuleCompletion {
    let ModuleRequestKind::Resolve {
        key,
        resolved_key_id,
        ..
    } = &request.kind
    else {
        panic!("resolve")
    };
    ModuleCompletion::KeyResolved(Ok(ResolvedModuleKey {
        id: *resolved_key_id,
        key: key.clone(),
        handle: format!("resolved-{}", resolved_key_id.0).into(),
        module: ResolvedModule {
            original_uri: key.uri.clone(),
            source_uri: key.uri.clone(),
            resolved_uri: uri.map_or_else(|| key.uri.clone(), Into::into),
        },
    }))
}
fn source(text: &str) -> ModuleCompletion {
    ModuleCompletion::Source(Ok(ModuleSource {
        text: text.into(),
        inferred_name: "child".into(),
    }))
}

#[test]
fn custom_base_uses_its_source_and_builtin_singleton_keeps_its_own_receiver() {
    let mut p = probe(
        "function a()=import(\"pkl:base\")\nfunction b()=import(\"pkl:base\")\nfunction c()=import(\"pkl:base\")\n",
        &["a().tag", "b().Module == Module", "c().tag", "a().tag"],
    );
    let mut select_custom = true;
    let mut loads = 0;
    for (index, expected) in ["99", "true", "99", "99"].into_iter().enumerate() {
        let value = run(&mut p, index as u32, &mut |request| {
            Some(match &request.kind {
                ModuleRequestKind::ResolveKey { .. } => {
                    panic!("public lifecycle request in supplied-source test")
                }
                ModuleRequestKind::SelectKey { uri, .. } if uri == "pkl:base" && select_custom => {
                    key(request, None)
                }
                ModuleRequestKind::QueryCachePolicy { key } if key.kind == ModuleKeyKind::Host => {
                    ModuleCompletion::CachePolicy(Ok(true))
                }
                ModuleRequestKind::Resolve { key, .. } if key.kind == ModuleKeyKind::Host => {
                    resolved(request, None)
                }
                ModuleRequestKind::LoadSource { .. } => {
                    loads += 1;
                    source("tag=99\n")
                }
                _ => return None,
            })
        })
        .unwrap();
        assert_eq!(value, expected);
        select_custom = index != 0;
    }
    assert_eq!(loads, 1);
    p.close();
    assert_eq!(p.collect().slots, 0);
}

#[test]
fn uncached_load_bypasses_lookups_but_publishes_for_later_cached_keys_and_sites() {
    let mut p = probe(
        "function a()=import(\"memo:/same.pkl\")\nfunction b()=import(\"memo:/same.pkl\")\nfunction c()=import(\"memo:/same.pkl\")\n",
        &["a().tag", "b().tag", "c().tag", "a().tag"],
    );
    let mut loads = 0;
    let mut policies = 0;
    for (index, expected) in ["1", "2", "2", "1"].into_iter().enumerate() {
        let value = run(&mut p, index as u32, &mut |request| {
            Some(match &request.kind {
                ModuleRequestKind::ResolveKey { .. } => {
                    panic!("public lifecycle request in supplied-source test")
                }
                ModuleRequestKind::SelectKey { .. } => key(request, None),
                ModuleRequestKind::QueryCachePolicy { .. } => {
                    policies += 1;
                    ModuleCompletion::CachePolicy(Ok(index >= 2))
                }
                ModuleRequestKind::Resolve { .. } => resolved(request, Some("memo:/resolved.pkl")),
                ModuleRequestKind::LoadSource { key, .. } => {
                    assert_eq!(key.handle, format!("resolved-{}", key.id.0));
                    assert_eq!(key.key.handle, format!("key-{}", key.key.id.0));
                    loads += 1;
                    source(&format!(
                        "class C {{ value={loads} }}\ntypealias A=C\nx: A=new C {{}}\ntag=x.value\n"
                    ))
                }
                _ => return None,
            })
        })
        .unwrap();
        assert_eq!(value, expected);
    }
    assert_eq!((loads, policies), (2, 3));
    p.close();
    assert_eq!(p.collect().slots, 0);
}

#[test]
fn clause_factory_selection_is_eager_and_selected_uri_infers_the_member_name() {
    let mut p = probe(
        "import \"memo:/requested.pkl\"\nfunction tag()=chosen.tag\nstable=7\n",
        &["stable", "tag()"],
    );
    let mut phases = Vec::new();
    let mut fragment_getters = 0;
    for (index, expected) in ["7", "99"].into_iter().enumerate() {
        let value = run(&mut p, index as u32, &mut |request| {
            Some(match &request.kind {
                ModuleRequestKind::ResolveKey { .. } => {
                    panic!("public lifecycle request in supplied-source test")
                }
                ModuleRequestKind::SelectKey { phase, .. } => {
                    phases.push(*phase);
                    key(request, Some("memo:/chosen.pkl"))
                }
                ModuleRequestKind::QueryFragmentPaths { .. } => {
                    fragment_getters += 1;
                    ModuleCompletion::FragmentPaths(Ok(false))
                }
                ModuleRequestKind::QueryCachePolicy { .. } => {
                    ModuleCompletion::CachePolicy(Ok(true))
                }
                ModuleRequestKind::Resolve { key, .. } => {
                    assert_eq!(key.uri, "memo:/chosen.pkl");
                    resolved(request, None)
                }
                ModuleRequestKind::LoadSource { .. } => source("tag=99\n"),
                _ => return None,
            })
        })
        .unwrap();
        assert_eq!(value, expected);
    }
    assert_eq!(
        phases,
        [
            SelectionPhase::ClausePreparation,
            SelectionPhase::RuntimeImport
        ]
    );
    assert_eq!(fragment_getters, 2);
}

#[test]
fn explicit_clause_alias_selects_key_without_inferred_name_getter() {
    let mut p = probe(
        "import \"memo:/requested.pkl\" as chosen\nstable=7\n",
        &["stable"],
    );
    let mut selections = 0;
    let value = run(&mut p, 0, &mut |request| {
        Some(match &request.kind {
            ModuleRequestKind::ResolveKey { .. } => {
                panic!("public lifecycle request in supplied-source test")
            }
            ModuleRequestKind::SelectKey { phase, .. } => {
                assert_eq!(*phase, SelectionPhase::ClausePreparation);
                selections += 1;
                key(request, None)
            }
            ModuleRequestKind::QueryFragmentPaths { .. } => {
                panic!("explicit alias must not infer a name")
            }
            _ => return None,
        })
    })
    .unwrap();
    assert_eq!(value, "7");
    assert_eq!(selections, 1);
}

#[test]
fn raw_factory_error_uri_is_distinct_from_normalized_factory_input() {
    let mut p = probe("value=import(\"memo:/a/../missing.pkl\")\n", &["value"]);
    let error = run(&mut p, 0, &mut |request| match &request.kind {
        ModuleRequestKind::ResolveKey { .. } => {
            panic!("public lifecycle request in supplied-source test")
        }
        ModuleRequestKind::SelectKey { uri, .. } => {
            assert_eq!(uri, "memo:/missing.pkl");
            Some(ModuleCompletion::KeySelected(Err(
                ModuleHostError::NoLoader,
            )))
        }
        _ => None,
    })
    .unwrap_err();
    assert!(error.contains("No module loader is registered for module `memo:/a/../missing.pkl`."));
}

#[test]
fn custom_ref_uri_uses_actual_declared_types_without_builtin_reference_rules() {
    let mut p = probe(
        "import \"pkl:ref\"\nx: ref.Reference? = null\ny: ref.Domain? = null\nfunction get() = new ref.Reference {}\n",
        &["x", "get().tag"],
    );
    for (index, expected) in ["null", "99"].into_iter().enumerate() {
        let value = run(&mut p, index as u32, &mut |request| {
            Some(match &request.kind {
                ModuleRequestKind::ResolveKey { .. } => {
                    panic!("public lifecycle request in supplied-source test")
                }
                ModuleRequestKind::SelectKey { uri, .. } if uri == "pkl:ref" => key(request, None),
                ModuleRequestKind::QueryCachePolicy { key } if key.kind == ModuleKeyKind::Host => {
                    ModuleCompletion::CachePolicy(Ok(true))
                }
                ModuleRequestKind::Resolve { key, .. } if key.kind == ModuleKeyKind::Host => {
                    resolved(request, None)
                }
                ModuleRequestKind::LoadSource { .. } => {
                    source("class Reference { tag=99 }\nclass Domain { tag=77 }\n")
                }
                _ => return None,
            })
        })
        .unwrap();
        assert_eq!(value, expected);
    }
}

#[test]
fn rethrowing_a_cached_parent_does_not_replace_a_new_failure_at_the_same_uri() {
    // Java03 observes independent exception objects for these two failed loads.
    let mut p = probe(
        "function parent()=import(\"memo:/parent.pkl\")\nfunction uncached()=import(\"memo:/u.pkl\")\nfunction cached()=import(\"memo:/u.pkl\")\n",
        &[
            "parent().tag",
            "uncached().tag",
            "parent().tag",
            "cached().tag",
            "parent().tag",
            "cached().tag",
        ],
    );
    let mut loads = 0;
    let mut first_parent_error = None;
    for (index, expected) in [
        "MissingOne",
        "MissingTwo",
        "MissingOne",
        "MissingTwo",
        "MissingOne",
        "MissingTwo",
    ]
    .into_iter()
    .enumerate()
    {
        let error = run(&mut p, index as u32, &mut |request| {
            Some(match &request.kind {
                ModuleRequestKind::SelectKey { .. } => key(request, None),
                ModuleRequestKind::QueryCachePolicy { .. } => {
                    ModuleCompletion::CachePolicy(Ok(index != 1))
                }
                ModuleRequestKind::Resolve { .. } => resolved(request, None),
                ModuleRequestKind::LoadSource { key, .. } if key.key.uri == "memo:/parent.pkl" => {
                    source("import \"memo:/u.pkl\" as u\np: u.Absent? = null\ntag=\"parent\"\n")
                }
                ModuleRequestKind::LoadSource { .. } => {
                    loads += 1;
                    source(if loads == 1 {
                        "p: MissingOne = 1\ntag=\"first\"\n"
                    } else {
                        "p: MissingTwo = 1\ntag=\"second\"\n"
                    })
                }
                _ => return None,
            })
        })
        .unwrap_err();
        assert!(error.contains(expected), "{error}");
        assert!(
            !error.contains(if expected == "MissingOne" {
                "MissingTwo"
            } else {
                "MissingOne"
            }),
            "{error}"
        );
        if index == 0 {
            first_parent_error = Some(error);
        } else if index % 2 == 0 {
            assert_eq!(Some(error), first_parent_error);
        }
    }
    assert_eq!(loads, 2);
    p.close();
    assert_eq!(p.collect().slots, 0);
}

#[test]
fn wrong_phase_failures_and_key_identities_preserve_the_pending_token() {
    let mut p = probe("value=import(\"memo:/child.pkl\").tag", &["value"]);
    p.start_path(0, 0, None).unwrap();
    let mut tested = 0;
    loop {
        let event = p.poll(0, 1).unwrap();
        p.collect();
        match event.kind {
            "yield" => {}
            "module-request" => {
                let token = event.token.unwrap();
                let request = p.module_request(&token).unwrap();
                let bad = match &request.kind {
                    ModuleRequestKind::CheckImport { .. } => Some(ModuleCompletion::ImportChecked(
                        Err(ModuleHostError::NoLoader),
                    )),
                    ModuleRequestKind::SelectKey { id, uri, .. } => {
                        assert!(
                            p.complete_module(
                                &token,
                                ModuleCompletion::KeySelected(Err(
                                    ModuleHostError::SecurityManagerException {
                                        message: Some("wrong phase".into())
                                    }
                                )),
                                0
                            )
                            .is_err()
                        );
                        Some(ModuleCompletion::KeySelected(Ok(ModuleKey {
                            id: ModuleKeyId(id.0 + 1),
                            uri: uri.clone(),
                            kind: ModuleKeyKind::Host,
                            handle: "wrong".into(),
                        })))
                    }
                    ModuleRequestKind::QueryCachePolicy { key } => {
                        assert!(
                            p.complete_module_cache_policy(
                                &token,
                                &(key.id.0 + 1).to_string(),
                                true,
                                0
                            )
                            .is_err()
                        );
                        Some(ModuleCompletion::CachePolicy(Err(
                            ModuleHostError::SecurityManagerException {
                                message: Some("wrong phase".into()),
                            },
                        )))
                    }
                    ModuleRequestKind::Resolve { .. } => {
                        let ModuleCompletion::KeyResolved(Ok(mut resolved)) =
                            resolved(&request, None)
                        else {
                            unreachable!()
                        };
                        resolved.id.0 += 1;
                        Some(ModuleCompletion::KeyResolved(Ok(resolved)))
                    }
                    _ => None,
                };
                if let Some(bad) = bad {
                    tested += 1;
                    assert!(p.complete_module(&token, bad, 0).is_err());
                    assert_eq!(p.module_request(&token).unwrap(), request);
                    assert_eq!(p.poll(0, 1).unwrap().token.as_deref(), Some(token.as_str()));
                }
                let good = match &request.kind {
                    ModuleRequestKind::SelectKey { .. } => key(&request, None),
                    ModuleRequestKind::QueryCachePolicy { .. } => {
                        ModuleCompletion::CachePolicy(Ok(true))
                    }
                    ModuleRequestKind::Resolve { .. } => resolved(&request, None),
                    ModuleRequestKind::LoadSource { .. } => source("tag=99"),
                    _ => default_completion(&request).unwrap(),
                };
                p.complete_module(&token, good, 0).unwrap();
            }
            "value" => {
                assert_eq!(String::from_utf16(&event.value.unwrap()).unwrap(), "99");
                break;
            }
            _ => panic!("unexpected {event:?}"),
        }
    }
    assert_eq!(tested, 4);
}
