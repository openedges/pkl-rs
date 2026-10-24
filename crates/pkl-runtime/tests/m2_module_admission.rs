//! Admission architecture regressions. Pinned Java policy observations are
//! consumed separately; these assertions do not manufacture oracle outputs.
#[path = "support/module_keys.rs"]
mod module_keys;
use pkl_runtime::{
    modules::{ModuleCompletion, ModuleRequest, ModuleRequestKind, ModuleSource, ResolvedModule},
    probe::{SourceEvent, SourceProbe},
};

fn probe(source: &str, expressions: &[&str]) -> SourceProbe {
    SourceProbe::new_expressions_text(
        source,
        "file:///admission/main.pkl",
        "main",
        &expressions.iter().map(|s| (*s).into()).collect::<Vec<_>>(),
        "compact",
        false,
        false,
        true,
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
    panic!("admission did not settle");
}

fn value(event: SourceEvent) -> String {
    assert_eq!(event.kind, "value", "{event:?}");
    String::from_utf16(&event.value.unwrap()).unwrap()
}

fn phase(request: &ModuleRequest) -> &'static str {
    use pkl_runtime::modules::SelectionPhase;
    match &request.kind {
        ModuleRequestKind::CheckImport { .. } => "check-import",
        ModuleRequestKind::SelectKey { phase, .. } => match phase {
            SelectionPhase::RuntimeImport => "select-runtime",
            SelectionPhase::ClausePreparation => "select-clause",
            SelectionPhase::Primary => "select-primary",
        },
        ModuleRequestKind::QueryCachePolicy { .. } => "cache-policy",
        ModuleRequestKind::QueryFragmentPaths { .. } => "fragment-paths",
        ModuleRequestKind::ResolveImportUri { .. } => "resolve-import-uri",
        ModuleRequestKind::Resolve { .. } => "resolve",
        ModuleRequestKind::ResolveKey { .. } => "resolve-key",
        ModuleRequestKind::LoadSource { .. } => "source",
        ModuleRequestKind::Capabilities { .. } => "capabilities",
        ModuleRequestKind::HasElement { .. } => "has-element",
    }
}

fn phases(requests: &[ModuleRequest]) -> Vec<&'static str> {
    requests.iter().map(phase).collect()
}

// Explicit stateless Host for the child used by pending-phase lifecycle tests.
// The caller asserts each phase before this supplies its matching completion.
fn child_completion(request: &ModuleRequest) -> ModuleCompletion {
    match &request.kind {
        ModuleRequestKind::Resolve { original_uri, .. } => {
            ModuleCompletion::Resolved(Ok(ResolvedModule {
                original_uri: original_uri.clone(),
                resolved_uri: original_uri.clone(),
                source_uri: original_uri.clone(),
            }))
        }
        ModuleRequestKind::LoadSource { .. } => ModuleCompletion::Source(Ok(ModuleSource {
            text: "value=7".into(),
            inferred_name: "child".into(),
        })),
        _ => module_keys::complete(request),
    }
}

const CHILD_PHASES: [&str; 6] = [
    "check-import",
    "select-runtime",
    "cache-policy",
    "resolve",
    "source",
    "fragment-paths",
];

fn run_default(
    probe: &mut SourceProbe,
    index: u32,
    requests: &mut Vec<ModuleRequest>,
) -> SourceEvent {
    probe.start_path(index, 0, None).unwrap();
    loop {
        let event = next(probe);
        if event.kind != "module-request" {
            return event;
        }
        let token = event.token.unwrap();
        requests.push(probe.module_request(&token).unwrap());
        assert!(probe.complete_default_module(&token, 0).unwrap());
    }
}

#[test]
fn successful_site_cache_precedes_admission_and_uri_cache_does_not_skip_other_sites() {
    let mut probe = probe(
        "function getJson() = import(\"pkl:json\")\nfirst=getJson()\nsecond=getJson()\nother=import(\"pkl:json\")\nbaseAlias=import(\"pkl:base\")\n",
        &[
            "first == first",
            "second == first",
            "other == first",
            "baseAlias.Module == Module",
        ],
    );
    let mut requests = Vec::new();
    assert_eq!(value(run_default(&mut probe, 0, &mut requests)), "true");
    assert_eq!(
        phases(&requests),
        ["check-import", "select-runtime", "cache-policy", "resolve"]
    );
    assert_eq!(value(run_default(&mut probe, 1, &mut requests)), "true");
    assert_eq!(
        requests.len(),
        4,
        "the same function import node is already successful"
    );
    assert_eq!(value(run_default(&mut probe, 2, &mut requests)), "true");
    assert_eq!(
        requests.len(),
        7,
        "a distinct import node checks its edge, selects a key and queries policy before the URI hit"
    );
    assert_eq!(
        &phases(&requests)[4..],
        ["check-import", "select-runtime", "cache-policy"]
    );
    assert_eq!(value(run_default(&mut probe, 3, &mut requests)), "true");
    assert_eq!(requests.len(), 9);
    assert_eq!(&phases(&requests)[7..], ["check-import", "select-runtime"]);
    assert!(
        matches!(&requests[7].kind, ModuleRequestKind::CheckImport { import_uri, .. } if import_uri.as_ref()=="pkl:base")
    );
    probe.close();
    assert_eq!(probe.collect().slots, 0);
}

#[test]
fn singleton_aliases_share_canonical_receivers_but_keep_authored_edge_spelling() {
    let mut probe = probe(
        "b=import(\"pkl:base\")\nbf=import(\"pkl:base#tag\")\nbe=import(\"pkl:%62ase\")\nr=import(\"pkl:ref\")\nrf=import(\"pkl:ref#tag\")\nre=import(\"pkl:%72ef\")\nt=import(\"pkl:test\")\ntf=import(\"pkl:test#tag\")\nte=import(\"pkl:%74est\")\njf=import(\"pkl:json#tag\")\nje=import(\"pkl:%6ason\")",
        &[
            "b.Module == Module",
            "bf == b",
            "be == b",
            "rf == r",
            "re == r",
            "tf == t",
            "te == t",
            "jf",
            "je",
        ],
    );
    let mut requests = Vec::new();
    for index in 0..7 {
        assert_eq!(value(run_default(&mut probe, index, &mut requests)), "true");
    }
    for index in 7..9 {
        let event = run_default(&mut probe, index, &mut requests);
        assert_eq!(
            event.kind, "error",
            "non-singleton catalog uses the whole URI"
        );
    }
    let edges: Vec<_> = requests
        .iter()
        .filter_map(|request| {
            if let ModuleRequestKind::CheckImport { import_uri, .. } = &request.kind {
                Some(import_uri.as_ref())
            } else {
                assert_eq!(phase(request), "select-runtime", "{request:?}");
                None
            }
        })
        .collect();
    assert_eq!(
        edges,
        [
            "pkl:base",
            "pkl:base#tag",
            "pkl:%62ase",
            "pkl:ref#tag",
            "pkl:ref",
            "pkl:%72ef",
            "pkl:test#tag",
            "pkl:test",
            "pkl:%74est",
            "pkl:json#tag",
            "pkl:%6ason"
        ]
    );
    assert_eq!(
        phases(&requests),
        ["check-import", "select-runtime"].repeat(11),
        "singleton and missing-catalog selection never query cache policy or resolve"
    );
    probe.close();
    assert_eq!(probe.collect().slots, 0);
}

#[test]
fn default_trust_decodes_only_the_nested_jar_marker_and_uses_resolved_importer() {
    use pkl_runtime::modules::{ModuleHostError, ModuleIdentity, default_completion};
    for (importer, target, allowed) in [
        ("jar:nest%65d:archive", "modulepath:/target", true),
        ("https://example.org/main", "jar:nest%65d:archive", false),
        ("jar:file:///archive", "file:///target", true),
        ("jar:file://host/archive", "file:///target", false),
    ] {
        let request = ModuleRequest {
            id: 1,
            kind: ModuleRequestKind::CheckImport {
                importer: ModuleIdentity {
                    original_uri: "file:///original".into(),
                    resolved_uri: importer.into(),
                },
                import_uri: target.into(),
            },
        };
        let Some(ModuleCompletion::ImportChecked(result)) = default_completion(&request) else {
            panic!("default edge policy missing");
        };
        assert_eq!(result.is_ok(), allowed, "{importer} -> {target}");
        if !allowed {
            assert!(matches!(
                result,
                Err(ModuleHostError::SecurityManagerException { .. })
            ));
        }
    }
}

#[test]
fn reference_singleton_aliases_use_the_same_external_type_binding_after_admission() {
    for uri in ["pkl:ref", "pkl:ref#tag", "pkl:%72ef"] {
        let mut probe = probe(
            &format!(
                "import \"{uri}\" as r\nclass D extends r.Domain {{}}\nx: r.Reference? = null\nresult = (new D {{}}) is r.Domain"
            ),
            &["x", "result"],
        );
        let mut requests = Vec::new();
        assert_eq!(value(run_default(&mut probe, 0, &mut requests)), "null");
        assert_eq!(value(run_default(&mut probe, 1, &mut requests)), "true");
        assert_eq!(
            phases(&requests),
            ["select-clause", "check-import", "select-runtime"]
        );
        assert!(
            matches!(&requests[1].kind, ModuleRequestKind::CheckImport { import_uri, .. } if import_uri.as_ref() == uri)
        );
        probe.close();
        assert_eq!(probe.collect().slots, 0);
    }
}

#[test]
fn qualified_reference_type_forces_the_authored_import_before_applying_its_binding() {
    for denied in [false, true] {
        let mut probe = probe(
            "import \"pkl:ref\"\nx: ref.Reference? = null\nstable=7",
            &["x", "x", "stable"],
        );
        let mut checks = 0;
        let mut requests = Vec::new();
        let mut first_error = None;
        for index in 0..3 {
            probe.start_path(index, 0, None).unwrap();
            loop {
                let event = next(&mut probe);
                if event.kind != "module-request" {
                    if denied {
                        assert_eq!(
                            event.kind, "error",
                            "root type initialization must honor the edge denial"
                        );
                        if let Some(first) = &first_error {
                            assert_eq!(
                                event.error.as_ref().unwrap(),
                                first,
                                "failed root initialization is cached"
                            );
                        } else {
                            first_error = event.error;
                        }
                    } else {
                        assert_eq!(value(event), if index < 2 { "null" } else { "7" });
                    }
                    break;
                }
                let token = event.token.unwrap();
                let request = probe.module_request(&token).unwrap();
                requests.push(request.clone());
                if !matches!(request.kind, ModuleRequestKind::CheckImport { .. }) {
                    assert!(matches!(
                        phase(&request),
                        "select-clause" | "fragment-paths" | "select-runtime"
                    ));
                    assert!(probe.complete_default_module(&token, 0).unwrap());
                    continue;
                }
                assert!(
                    matches!(&request.kind, ModuleRequestKind::CheckImport { import_uri, .. } if import_uri.as_ref() == "pkl:ref")
                );
                checks += 1;
                if denied {
                    probe
                        .reject_module_import_check(
                            &token,
                            "security-manager-exception",
                            Some("type edge denied".into()),
                            0,
                        )
                        .unwrap();
                } else {
                    assert!(probe.complete_default_module(&token, 0).unwrap());
                }
            }
        }
        assert_eq!(checks, 1);
        assert_eq!(
            phases(&requests),
            if denied {
                vec!["select-clause", "fragment-paths", "check-import"]
            } else {
                vec![
                    "select-clause",
                    "fragment-paths",
                    "check-import",
                    "select-runtime",
                ]
            },
            "clause selection precedes the one runtime edge; a denial prevents runtime factory selection"
        );
        probe.close();
        assert_eq!(probe.collect().slots, 0);
    }
}

#[test]
fn typed_edge_denial_is_catchable_retryable_and_never_a_successful_site() {
    let mut probe = probe(
        "import \"pkl:test\"\nfunction run() = test.catchOrNull(() -> import(\"pkl:json\"))\nstable=7\n",
        &["run()", "run()", "run()", "stable"],
    );
    let mut denied = 0;
    for index in 0..4 {
        probe.start_path(index, 0, None).unwrap();
        loop {
            let event = next(&mut probe);
            if event.kind != "module-request" {
                assert_eq!(
                    value(event),
                    match index {
                        0 | 1 => "edge denied",
                        2 => "null",
                        _ => "7",
                    }
                );
                break;
            }
            let token = event.token.unwrap();
            let request = probe.module_request(&token).unwrap();
            if matches!(&request.kind, ModuleRequestKind::CheckImport { import_uri, .. } if import_uri.as_ref()=="pkl:json")
                && denied < 2
            {
                probe
                    .reject_module_import_check(
                        &token,
                        "security-manager-exception",
                        Some("edge denied".into()),
                        0,
                    )
                    .unwrap();
                denied += 1;
            } else {
                assert!(probe.complete_default_module(&token, 0).unwrap());
            }
        }
    }
    assert_eq!(denied, 2);
    probe.close();
    assert_eq!(probe.collect().slots, 0);
}

#[test]
fn edge_security_cause_preserves_null_and_utf16_while_raw_denied_bypasses_guest_catch() {
    use pkl_runtime::value::PklString;
    for (kind, message, expected) in [
        (
            "security-manager-exception",
            None,
            Some(
                "None (cause has no message)"
                    .encode_utf16()
                    .collect::<Vec<_>>(),
            ),
        ),
        (
            "security-manager-exception",
            Some(PklString::from_utf16(vec![0xd800, 0x41])),
            Some(vec![0xd800, 0x41]),
        ),
        ("denied", Some("raw host rejection".into()), None),
    ] {
        let mut probe = probe(
            "import \"pkl:test\"\nfunction run()=test.catch(() -> import(\"pkl:json\"))",
            &["run()"],
        );
        probe.start_path(0, 0, None).unwrap();
        loop {
            let event = next(&mut probe);
            if event.kind != "module-request" {
                if let Some(expected) = expected.as_ref() {
                    assert_eq!(event.kind, "value", "{event:?}");
                    assert_eq!(event.value.as_ref().unwrap(), expected);
                } else {
                    assert_eq!(event.kind, "error", "raw Host text must bypass guest catch");
                }
                break;
            }
            let token = event.token.unwrap();
            if matches!(&probe.module_request(&token).unwrap().kind, ModuleRequestKind::CheckImport { import_uri, .. } if import_uri.as_ref()=="pkl:json")
            {
                probe
                    .reject_module_import_check(&token, kind, message.clone(), 0)
                    .unwrap();
            } else {
                assert!(probe.complete_default_module(&token, 0).unwrap());
            }
        }
        probe.close();
        assert_eq!(probe.collect().slots, 0);
    }
}

#[test]
fn import_admission_survives_callback_reentry_and_default_delegation_checks_deadlines() {
    let mut probe = probe("x=import(\"pkl:json\")\nstable=7", &["x==x", "stable"]);
    probe.start_path(0, 0, None).unwrap();
    let outer = next(&mut probe).token.unwrap();
    probe.reenter_path(&outer, 1, 0, None).unwrap();
    assert_eq!(value(next(&mut probe)), "7");
    assert_eq!(next(&mut probe).token.as_deref(), Some(outer.as_str()));
    assert!(probe.complete_default_module(&outer, 0).unwrap());
    for expected in ["select-runtime", "cache-policy", "resolve"] {
        let token = next(&mut probe).token.unwrap();
        assert_eq!(phase(&probe.module_request(&token).unwrap()), expected);
        assert!(probe.complete_default_module(&token, 0).unwrap());
    }
    assert_eq!(value(next(&mut probe)), "true");
    probe.close();
    assert_eq!(probe.collect().slots, 0);

    let mut timed = self::probe("x=import(\"child.pkl\")", &["x"]);
    timed.start_path(0, 0, Some(1)).unwrap();
    let token = next(&mut timed).token.unwrap();
    assert!(timed.complete_default_module(&token, 0).unwrap());
    let token = next(&mut timed).token.unwrap();
    assert_eq!(
        phase(&timed.module_request(&token).unwrap()),
        "select-runtime"
    );
    // External key selection is unsupported by default delegation, but an expired
    // request must still terminate instead of returning false.
    assert!(timed.complete_default_module(&token, 2).is_err());
    assert_eq!(timed.collect().slots, 0);
    assert!(timed.start_path(0, 2, None).is_err());
}

#[test]
fn wrong_phase_and_bundled_identity_cannot_consume_the_pending_token() {
    let mut probe = probe("x=import(\"pkl:json\")", &["x == x"]);
    probe.start_path(0, 0, None).unwrap();
    let event = next(&mut probe);
    assert_eq!(event.module.unwrap().kind, "check-import");
    let token = event.token.unwrap();
    assert!(probe.complete_module_source(&token, "", "json", 0).is_err());
    assert!(
        probe
            .reject_module_import_check(&token, "io", Some("not an edge cause".into()), 0)
            .is_err()
    );
    assert_eq!(next(&mut probe).token.as_deref(), Some(token.as_str()));
    assert!(probe.complete_default_module(&token, 0).unwrap());
    let event = next(&mut probe);
    let token = event.token.unwrap();
    let request = probe.module_request(&token).unwrap();
    assert_eq!(phase(&request), "select-runtime");
    let ModuleRequestKind::SelectKey { id, .. } = request.kind else {
        unreachable!()
    };
    let wrong_id = if id.0 == 0 { "1" } else { "0" };
    assert!(
        probe
            .complete_module_key(&token, wrong_id, "pkl:json", "standard-library", "", 0)
            .is_err()
    );
    assert!(
        probe
            .complete_module_cache_policy(&token, &id.0.to_string(), true, 0)
            .is_err()
    );
    assert_eq!(next(&mut probe).token.as_deref(), Some(token.as_str()));
    assert!(probe.complete_default_module(&token, 0).unwrap());
    let event = next(&mut probe);
    let token = event.token.unwrap();
    assert_eq!(
        phase(&probe.module_request(&token).unwrap()),
        "cache-policy"
    );
    assert!(
        probe
            .complete_module_cache_policy(&token, wrong_id, true, 0)
            .is_err()
    );
    assert!(probe.complete_module_source(&token, "", "json", 0).is_err());
    assert_eq!(next(&mut probe).token.as_deref(), Some(token.as_str()));
    assert!(probe.complete_default_module(&token, 0).unwrap());
    let event = next(&mut probe);
    let token = event.token.unwrap();
    assert_eq!(event.module.unwrap().kind, "resolve");
    assert!(
        probe
            .complete_module_resolution(
                &token,
                "pkl:json",
                "file:///replacement.pkl",
                "pkl:json",
                0
            )
            .is_err()
    );
    assert_eq!(next(&mut probe).token.as_deref(), Some(token.as_str()));
    assert!(probe.complete_default_module(&token, 0).unwrap());
    assert_eq!(value(next(&mut probe)), "true");
    assert!(probe.complete_default_module(&token, 0).is_err());
    probe.close();
    assert_eq!(probe.collect().slots, 0);
}

#[test]
fn resolved_importer_trust_is_independent_of_original_and_diagnostic_uri() {
    let mut probe = probe("child=import(\"child.pkl\")", &["child.value"]);
    probe.start_path(0, 0, None).unwrap();
    let mut sources = 0;
    let mut prepared_uris = 0;
    let mut observed_inner = false;
    loop {
        let event = next(&mut probe);
        if event.kind == "error" {
            let text = String::from_utf16(&event.error.unwrap()).unwrap();
            assert!(text.contains("insufficient trust level"), "{text}");
            break;
        }
        let token = event.token.expect("module request");
        match probe.module_request(&token).unwrap().kind {
            ModuleRequestKind::SelectKey { .. }
            | ModuleRequestKind::QueryCachePolicy { .. }
            | ModuleRequestKind::QueryFragmentPaths { .. } => {
                assert!(module_keys::complete_probe(&mut probe, &token, 0).unwrap());
            }
            ModuleRequestKind::ResolveImportUri { key, authored_uri } => {
                assert_eq!(key.uri.as_ref(), "file:///admission/child.pkl");
                assert_eq!(authored_uri.as_ref(), "file:///secret.pkl");
                assert!(
                    !observed_inner,
                    "AST URI preparation precedes the denied runtime edge"
                );
                prepared_uris += 1;
                assert!(module_keys::complete_probe(&mut probe, &token, 0).unwrap());
            }
            ModuleRequestKind::CheckImport {
                importer,
                import_uri,
            } => {
                if import_uri.as_ref() == "file:///secret.pkl" {
                    assert_eq!(
                        importer.original_uri.as_ref(),
                        "file:///admission/child.pkl"
                    );
                    assert_eq!(
                        importer.resolved_uri.as_ref(),
                        "https://example.test/child.pkl"
                    );
                    observed_inner = true;
                }
                assert!(probe.complete_default_module(&token, 0).unwrap());
            }
            ModuleRequestKind::Resolve { original_uri, .. } => {
                assert!(
                    !observed_inner,
                    "denied edge must stop before secret key resolution"
                );
                assert!(!probe.complete_default_module(&token, 0).unwrap());
                assert_eq!(next(&mut probe).token.as_deref(), Some(token.as_str()));
                probe
                    .complete_module_resolution(
                        &token,
                        &original_uri,
                        "https://example.test/child.pkl",
                        "file:///display/child.pkl",
                        0,
                    )
                    .unwrap();
            }
            ModuleRequestKind::LoadSource { .. } => {
                sources += 1;
                probe
                    .complete_module_source(
                        &token,
                        "value=import(\"file:///secret.pkl\")",
                        "child",
                        0,
                    )
                    .unwrap();
            }
            other => panic!("unexpected {other:?}"),
        }
    }
    assert!(observed_inner);
    assert_eq!(sources, 1);
    assert_eq!(
        prepared_uris, 1,
        "the original declaration key resolves even an absolute operand"
    );
    probe.close();
    assert_eq!(probe.collect().slots, 0);
}

#[test]
fn resolved_alias_hits_do_not_seed_original_alias_cache() {
    let mut probe = probe(
        "first=import(\"a.pkl\")\nfunction f()=import(\"b.pkl\")\nfunction g()=import(\"b.pkl\")",
        &["first.value", "f().value", "g().value", "f().value"],
    );
    let mut resolved = Vec::new();
    let mut loads = 0;
    for index in 0..4 {
        probe.start_path(index, 0, None).unwrap();
        loop {
            let event = next(&mut probe);
            if event.kind != "module-request" {
                assert_eq!(value(event), "7");
                break;
            }
            let token = event.token.unwrap();
            let request = probe.module_request(&token).unwrap();
            let completion = match request.kind {
                ModuleRequestKind::SelectKey { .. }
                | ModuleRequestKind::QueryCachePolicy { .. }
                | ModuleRequestKind::QueryFragmentPaths { .. }
                | ModuleRequestKind::ResolveImportUri { .. } => module_keys::complete(&request),
                ModuleRequestKind::CheckImport { .. } => ModuleCompletion::ImportChecked(Ok(())),
                ModuleRequestKind::Resolve { original_uri, .. } => {
                    resolved.push(original_uri.to_string());
                    ModuleCompletion::Resolved(Ok(ResolvedModule {
                        original_uri,
                        resolved_uri: "file:///canonical/a.pkl".into(),
                        source_uri: "file:///display/a.pkl".into(),
                    }))
                }
                ModuleRequestKind::LoadSource { .. } => {
                    loads += 1;
                    ModuleCompletion::Source(Ok(ModuleSource {
                        text: "value=7".into(),
                        inferred_name: "a".into(),
                    }))
                }
                other => panic!("unexpected {other:?}"),
            };
            probe.complete_module(&token, completion, 0).unwrap();
        }
    }
    assert_eq!(
        resolved,
        [
            "file:///admission/a.pkl",
            "file:///admission/b.pkl",
            "file:///admission/b.pkl"
        ]
    );
    assert_eq!(loads, 1);
    probe.close();
    assert_eq!(probe.collect().slots, 0);
}

#[test]
fn successful_cycle_site_keeps_its_partial_receiver_after_module_initialization_fails() {
    // Source text follows the separately captured Java eager-cycle control.
    // This is an architecture assertion; its raw Java diagnostic comparison
    // uses the original physical source URIs in the differential consumer.
    let mut probe = probe(
        "function a() = import(\"./a.pkl\")\nfunction b() = import(\"./b.pkl\")\nstable = 7\n",
        &[
            "stable",
            "a().Good",
            "b().saved().Good",
            "b().fresh().Good",
            "b().saved().Good",
            "a().Good",
            "b().saved().Good",
            "b().fresh().Good",
            "stable",
        ],
    );
    let mut sources = Vec::new();
    let mut requests = Vec::new();
    let mut phases_by_call = Vec::new();
    let mut partial_class = None;
    for index in 0..9 {
        let request_start = requests.len();
        probe.start_path(index, 0, None).unwrap();
        let settled = loop {
            let event = next(&mut probe);
            if event.kind != "module-request" {
                break event;
            }
            let token = event.token.unwrap();
            let request = probe.module_request(&token).unwrap();
            requests.push(request.clone());
            let completion = match request.kind {
                ModuleRequestKind::SelectKey { .. }
                | ModuleRequestKind::QueryCachePolicy { .. }
                | ModuleRequestKind::QueryFragmentPaths { .. }
                | ModuleRequestKind::ResolveImportUri { .. } => module_keys::complete(&request),
                ModuleRequestKind::CheckImport { .. } => ModuleCompletion::ImportChecked(Ok(())),
                ModuleRequestKind::Resolve { original_uri, .. } => {
                    ModuleCompletion::Resolved(Ok(ResolvedModule {
                        resolved_uri: original_uri.clone(),
                        source_uri: original_uri.clone(),
                        original_uri,
                    }))
                }
                ModuleRequestKind::LoadSource { resolved, .. } => {
                    let (name, text) = match resolved.original_uri.as_ref() {
                        "file:///admission/a.pkl" => (
                            "a",
                            "import \"./b.pkl\" as b\nopen class Good {}\np: b.B? = null\nq: b.Missing? = null\n",
                        ),
                        "file:///admission/b.pkl" => (
                            "b",
                            "import \"./a.pkl\" as a\nclass B {}\np: a.Good? = null\nfunction saved() = a\nfunction fresh() = import(\"./a.pkl\")\n",
                        ),
                        other => panic!("unexpected source {other}"),
                    };
                    sources.push(name);
                    ModuleCompletion::Source(Ok(ModuleSource {
                        text: text.into(),
                        inferred_name: name.into(),
                    }))
                }
                other => panic!("unexpected cycle effect {other:?}"),
            };
            probe.complete_module(&token, completion, 0).unwrap();
        };
        match index {
            0 | 8 => assert_eq!(value(settled), "7"),
            2 | 4 | 6 => {
                let class = value(settled);
                assert_eq!(class, "a#Good");
                assert_eq!(partial_class.get_or_insert(class.clone()), &class);
            }
            _ => assert_eq!(settled.kind, "error", "{settled:?}"),
        }
        phases_by_call.push(phases(&requests[request_start..]));
        probe.collect();
    }
    assert_eq!(sources, ["a", "b"], "failed initialization is cached");
    assert_eq!(
        phases_by_call,
        [
            vec![],
            vec![
                "check-import",
                "select-runtime",
                "cache-policy",
                "resolve",
                "source",
                "fragment-paths",
                "resolve-import-uri",
                "select-clause",
                "check-import",
                "select-runtime",
                "cache-policy",
                "resolve",
                "source",
                "fragment-paths",
                "resolve-import-uri",
                "select-clause",
                "resolve-import-uri",
                "check-import",
                "select-runtime",
                "cache-policy",
            ],
            vec!["check-import", "select-runtime", "cache-policy"],
            vec!["check-import", "select-runtime", "cache-policy"],
            vec![],
            vec!["check-import", "select-runtime", "cache-policy"],
            vec![],
            vec!["check-import", "select-runtime", "cache-policy"],
            vec![],
        ],
        "source name getters, declaration-key URI preparation and unused clause selection precede initialization; successful partial sites stay cached while fresh failed sites retry admission and key selection"
    );
    probe.close();
    assert_eq!(probe.collect().slots, 0);
}

#[test]
fn close_and_cancel_in_each_pending_phase_keep_stale_tokens_and_release_all_roots() {
    for (pending_phase, expected_phase) in CHILD_PHASES.iter().enumerate() {
        for terminal in [false, true] {
            let mut probe = probe(
                "child=import(\"child.pkl\")\nstable=7",
                &["child", "stable"],
            );
            probe.start_path(0, 0, None).unwrap();
            let mut event = next(&mut probe);
            for expected in CHILD_PHASES.iter().take(pending_phase) {
                let token = event.token.unwrap();
                let request = probe.module_request(&token).unwrap();
                assert_eq!(phase(&request), *expected);
                probe
                    .complete_module(&token, child_completion(&request), 0)
                    .unwrap();
                event = next(&mut probe);
            }
            let token = event.token.unwrap();
            assert_eq!(
                phase(&probe.module_request(&token).unwrap()),
                *expected_phase
            );
            if terminal {
                probe.close();
            } else {
                probe.cancel();
            }
            assert!(probe.complete_module_import_check(&token, 0).is_err());
            // Public SourceProbe cancellation is terminal, unlike Vm::cancel.
            assert!(probe.start_path(1, 0, None).is_err());
            probe.close();
            assert_eq!(probe.collect().slots, 0);
        }
    }
}

#[test]
fn successful_import_site_does_not_retain_temporary_importing_receivers() {
    use pkl_runtime::{
        modules::{ModuleIdentity, default_completion},
        value::{RuntimeSource, Value},
        vm::{Instruction as I, Vm, VmPoll, compile_module},
    };
    use std::sync::Arc;
    let source = "factory = () -> new Dynamic { child = import(\"pkl:json\") }";
    let parsed = pkl_runtime::parser::parse_module(source).unwrap();
    let mut program =
        compile_module(&pkl_runtime::resolver::resolve_module(&parsed).unwrap()).unwrap();
    let code = Arc::make_mut(&mut program);
    code.register_primary_source(
        Arc::new(RuntimeSource {
            text: source.into(),
            uri: "file:///site-root.pkl".into(),
            module_name: "main".into(),
        }),
        ModuleIdentity {
            original_uri: "file:///site-root.pkl".into(),
            resolved_uri: "file:///site-root.pkl".into(),
        },
    )
    .unwrap();
    let invoke = code.blocks.len();
    code.blocks.push(vec![
        I::Input(0),
        I::Call {
            arity: 0,
            method: false,
        },
        I::Return,
    ]);
    let mut vm = Vm::new(program.clone());
    fn settle(vm: &mut Vm, roots: &[Value], requests: &mut Vec<ModuleRequest>) -> Value {
        for _ in 0..100_000 {
            let event = vm.poll(1);
            vm.collect(roots);
            match event {
                VmPoll::Yielded => {}
                VmPoll::Module(request) => {
                    let completion =
                        default_completion(&request).expect("explicit default profile");
                    vm.complete_module(request.id, completion).unwrap();
                    requests.push(request);
                }
                VmPoll::Ready(value) => return value.unwrap(),
                other => panic!("unexpected event: {other:?}"),
            }
        }
        panic!("site GC test exceeded its instruction budget");
    }
    let mut requests = Vec::new();
    vm.start(program.entry, vec![]).unwrap();
    let module = settle(&mut vm, &[], &mut requests);
    vm.start_member(module.as_object().unwrap(), "factory")
        .unwrap();
    let function = settle(&mut vm, &[], &mut requests);
    let mut plateau = None;
    let mut imported = None;
    for _ in 0..12 {
        vm.start(invoke, vec![function.clone()]).unwrap();
        let temporary = settle(&mut vm, &[], &mut requests);
        let temporary_id = temporary.as_object().unwrap();
        vm.start_member(temporary_id, "child").unwrap();
        let child = settle(&mut vm, &[], &mut requests).as_object().unwrap();
        if let Some(imported) = imported {
            assert_eq!(child, imported);
        }
        imported = Some(child);
        let stats = vm.collect(&[]);
        assert!(
            vm.member_state(temporary_id, "child").is_err(),
            "site cache retained an importing activation"
        );
        assert!(
            vm.member_state(child, "Property").is_ok(),
            "site/cache must retain the imported module"
        );
        if let Some(plateau) = plateau {
            assert_eq!((stats.live, stats.executable_live), plateau);
        }
        plateau = Some((stats.live, stats.executable_live));
    }
    assert_eq!(
        phases(&requests),
        ["check-import", "select-runtime", "cache-policy", "resolve"],
        "one successful shared AST site, selected key and bundled resolution"
    );
    vm.close();
    assert_eq!(vm.collect(&[]).slots, 0);
}

#[test]
fn vm_nonterminal_cancellation_retries_an_unpublished_import_site() {
    use pkl_runtime::{
        modules::ModuleIdentity,
        value::{RuntimeSource, Value},
        vm::{Vm, VmPoll, compile_module},
    };
    use std::sync::Arc;
    let source = "x=import(\"file:///child.pkl\")\nstable=7";
    let parsed = pkl_runtime::parser::parse_module(source).unwrap();
    let mut program =
        compile_module(&pkl_runtime::resolver::resolve_module(&parsed).unwrap()).unwrap();
    Arc::make_mut(&mut program)
        .register_primary_source(
            Arc::new(RuntimeSource {
                text: source.into(),
                uri: "file:///main.pkl".into(),
                module_name: "main".into(),
            }),
            ModuleIdentity {
                original_uri: "file:///main.pkl".into(),
                resolved_uri: "file:///main.pkl".into(),
            },
        )
        .unwrap();
    fn settle(vm: &mut Vm, roots: &[Value]) -> VmPoll {
        for _ in 0..100_000 {
            let event = vm.poll(1);
            vm.collect(roots);
            if !matches!(event, VmPoll::Yielded) {
                return event;
            }
        }
        panic!("VM did not settle");
    }
    for (pending_phase, expected_phase) in CHILD_PHASES.iter().enumerate() {
        let mut vm = Vm::new(program.clone());
        vm.start(program.entry, vec![]).unwrap();
        let VmPoll::Ready(Ok(module)) = settle(&mut vm, &[]) else {
            panic!("module setup")
        };
        let roots = [module.clone()];
        vm.start_member(module.as_object().unwrap(), "x").unwrap();
        let VmPoll::Module(mut request) = settle(&mut vm, &roots) else {
            panic!("import admission")
        };
        for expected in CHILD_PHASES.iter().take(pending_phase) {
            assert_eq!(phase(&request), *expected);
            vm.complete_module(request.id, child_completion(&request))
                .unwrap();
            let VmPoll::Module(next) = settle(&mut vm, &roots) else {
                panic!("next module phase")
            };
            request = next;
        }
        assert_eq!(phase(&request), *expected_phase);
        let old = request.id;
        vm.cancel();
        vm.collect(&roots);
        assert!(!vm.is_closed());
        assert!(
            vm.complete_module(old, ModuleCompletion::ImportChecked(Ok(())))
                .is_err()
        );
        vm.start_member(module.as_object().unwrap(), "stable")
            .unwrap();
        assert!(
            matches!(settle(&mut vm, &roots), VmPoll::Ready(Ok(Value::Int(value))) if *value==7)
        );
        vm.start_member(module.as_object().unwrap(), "x").unwrap();
        let VmPoll::Module(retry) = settle(&mut vm, &roots) else {
            panic!("retry")
        };
        assert_ne!(retry.id, old);
        assert!(matches!(retry.kind, ModuleRequestKind::CheckImport { .. }));
        vm.close();
        assert_eq!(vm.collect(&[]).slots, 0);
    }
}
