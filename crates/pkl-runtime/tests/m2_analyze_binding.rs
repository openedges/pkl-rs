//! Authored structural integration controls, not Java-derived raw expectations.
//! Closed Java acquisition must qualify API bytes, effects and error order later.
use pkl_runtime::{
    modules::{
        ModuleCompletion, ModuleKey, ModuleKeyId, ModuleKeyKind, ModuleRequestKind, ModuleSource,
        ResolvedKeyId, ResolvedModule, ResolvedModuleKey, default_completion,
    },
    probe::{SourceProbe, SourceTextEvent},
    value::PklString,
};
use std::collections::BTreeMap;

const PRIMARY: &str = "file:///stage151/main.pkl";
struct ResultLog {
    event: SourceTextEvent,
    loads: Vec<String>,
    resolves: Vec<String>,
}
fn execute(source: &str, files: &[(&str, &str)], deny: Option<&str>, legacy: bool) -> ResultLog {
    let files: BTreeMap<_, _> = files
        .iter()
        .map(|(uri, text)| (PklString::from(*uri), *text))
        .collect();
    let mut keys = BTreeMap::<ModuleKeyId, PklString>::new();
    let mut resolved = BTreeMap::<ResolvedKeyId, ResolvedModuleKey>::new();
    let mut loads = Vec::new();
    let mut resolves = Vec::new();
    let mut probe = SourceProbe::new_module_evaluator("compact", true, false, false).unwrap();
    probe
        .evaluate_module_source(PRIMARY, Some(source), "output.text", 0, None)
        .unwrap();
    let mut terminal = None;
    for _ in 0..20_000 {
        let event = probe.poll_text(0, 1_000).unwrap();
        probe.collect();
        match event.kind {
            "yield" => {}
            "module-request" => {
                let token = event.token.as_deref().unwrap();
                let request = probe.module_request(token).unwrap();
                let completion = match &request.kind {
                    ModuleRequestKind::SelectKey { id, uri, .. } => {
                        assert!(
                            keys.insert(*id, uri.clone()).is_none(),
                            "global key identity reused"
                        );
                        if uri.starts_with("pkl:") {
                            default_completion(&request).unwrap()
                        } else {
                            assert!(
                                uri == PRIMARY || files.contains_key(uri),
                                "unexpected source lookup {uri}"
                            );
                            ModuleCompletion::KeySelected(Ok(ModuleKey {
                                id: *id,
                                uri: uri.clone(),
                                kind: ModuleKeyKind::Host,
                                handle: format!("selected:{}", id.0).into(),
                            }))
                        }
                    }
                    ModuleRequestKind::ResolveKey {
                        key,
                        resolved_key_id,
                        ..
                    } if files.contains_key(&key.uri) => {
                        assert_eq!(keys.get(&key.id), Some(&key.uri));
                        assert_eq!(
                            key.handle,
                            PklString::from(format!("selected:{}", key.id.0))
                        );
                        resolves.push(key.uri.as_str().unwrap().to_owned());
                        if deny.is_some_and(|uri| key.uri == uri) {
                            ModuleCompletion::KeyResolved(Err(
                                pkl_runtime::modules::ModuleHostError::SecurityManagerException {
                                    message: Some("authored Host denial".into()),
                                },
                            ))
                        } else {
                            let result = ResolvedModuleKey {
                                id: *resolved_key_id,
                                key: key.clone(),
                                handle: if legacy {
                                    "".into()
                                } else {
                                    format!("resolved:{}", resolved_key_id.0).into()
                                },
                                module: ResolvedModule {
                                    original_uri: key.uri.clone(),
                                    resolved_uri: PklString::from("resolved:").concat(&key.uri),
                                    source_uri: key.uri.clone(),
                                },
                            };
                            assert!(
                                resolved.insert(result.id, result.clone()).is_none(),
                                "resolved identity reused"
                            );
                            if legacy {
                                ModuleCompletion::Resolved(Ok(result.module))
                            } else {
                                ModuleCompletion::KeyResolved(Ok(result))
                            }
                        }
                    }
                    ModuleRequestKind::LoadSource {
                        key,
                        resolved: module,
                    } if files.contains_key(&key.key.uri) => {
                        assert_eq!(resolved.get(&key.id), Some(key));
                        assert_eq!(module, &key.module);
                        loads.push(key.key.uri.as_str().unwrap().to_owned());
                        ModuleCompletion::Source(Ok(ModuleSource {
                            text: files[&key.key.uri].into(),
                            inferred_name: "analysis-fixture".into(),
                        }))
                    }
                    _ => default_completion(&request)
                        .expect("only modeled module callbacks; no guest reads/logs"),
                };
                probe.complete_module(token, completion, 0).unwrap();
            }
            "error" | "value" => {
                terminal = Some(event);
                break;
            }
            other => panic!("unexpected guest evaluation effect {other}"),
        }
    }
    probe.close();
    assert_eq!(probe.collect().slots, 0, "closed analysis kept heap roots");
    assert_eq!(probe.context_depth(), 0, "closed analysis kept contexts");
    ResultLog {
        event: terminal.expect("bounded authored control terminated"),
        loads,
        resolves,
    }
}
fn success(log: &ResultLog) {
    assert_eq!(log.event.kind, "value", "{:?}", log.event.error);
    assert_eq!(log.event.value.as_deref(), Some(&[111, 107][..]));
}
#[test]
fn branching_static_graph_is_typed_sorted_and_never_evaluates_loaded_code() {
    let log = execute(
        r#"import "pkl:analyze"
g = analyze.importGraph(Set("file:///stage151/a.pkl"))
copy = (g) {}
output { text = if (g is analyze.ImportGraph &&
  g.imports.keys.toList() == List("file:///stage151/a.pkl", "file:///stage151/b.pkl", "file:///stage151/c.pkl") &&
  g.imports["file:///stage151/a.pkl"].first is analyze.Import &&
  g.imports["file:///stage151/a.pkl"].first.uri == "file:///stage151/b.pkl" &&
  copy.resolvedImports["file:///stage151/a.pkl"] == "resolved:file:///stage151/a.pkl") "ok" else throw("graph contract") }
"#,
        &[
            (
                "file:///stage151/a.pkl",
                "import \"c.pkl\"\nimport \"b.pkl\"\nbroken=throw(\"not evaluated\")\n",
            ),
            ("file:///stage151/b.pkl", "bad=read(\"forbidden:x\")\n"),
            (
                "file:///stage151/c.pkl",
                "bad=trace(throw(\"not evaluated\"))\n",
            ),
        ],
        None,
        false,
    );
    success(&log);
    assert_eq!(
        log.loads,
        [
            "file:///stage151/a.pkl",
            "file:///stage151/c.pkl",
            "file:///stage151/b.pkl"
        ]
    );
    assert_eq!(
        log.resolves,
        [
            "file:///stage151/a.pkl",
            "file:///stage151/c.pkl",
            "file:///stage151/b.pkl"
        ]
    );
    // Source-order sibling descent is this candidate's explicit condition; Java
    // HashSet identity-dependent sibling effect/error order remains unqualified.
}
#[test]
fn cycles_and_two_calls_keep_global_host_key_ownership_distinct() {
    let log = execute(
        r#"import "pkl:analyze"
a = analyze.importGraph(Set("file:///stage151/a.pkl"))
b = analyze.importGraph(Set("file:///stage151/a.pkl"))
output { text = if (a.imports == b.imports && a.imports.length == 2) "ok" else throw("cycle contract") }
"#,
        &[
            ("file:///stage151/a.pkl", "import \"b.pkl\"\n"),
            ("file:///stage151/b.pkl", "import \"a.pkl\"\n"),
        ],
        None,
        false,
    );
    success(&log);
    assert_eq!(
        log.loads,
        [
            "file:///stage151/a.pkl",
            "file:///stage151/b.pkl",
            "file:///stage151/a.pkl",
            "file:///stage151/b.pkl"
        ]
    );
    assert_eq!(
        log.resolves.len(),
        6,
        "backedges resolve before visited-node check"
    );
}
#[test]
fn empty_graph_factory_and_inherited_fields_survive_poll_collection() {
    let log = execute(
        r#"import "pkl:analyze"
g = analyze.importGraph(Set())
copy = (g) {}
output { text = if (copy is analyze.ImportGraph && copy.imports == Map() && copy.resolvedImports == Map()) "ok" else throw("empty contract") }
"#,
        &[],
        None,
        false,
    );
    success(&log);
    assert!(log.loads.is_empty());
    assert!(log.resolves.is_empty());
}
#[test]
fn host_resolution_denial_stops_before_loading_or_later_siblings() {
    let log = execute(
        r#"import "pkl:analyze"
g = analyze.importGraph(Set("file:///stage151/a.pkl"))
output { text = g.toString() }
"#,
        &[
            (
                "file:///stage151/a.pkl",
                "import \"b.pkl\"\nimport \"c.pkl\"\n",
            ),
            ("file:///stage151/b.pkl", "x=1\n"),
            ("file:///stage151/c.pkl", "x=1\n"),
        ],
        Some("file:///stage151/b.pkl"),
        false,
    );
    assert_eq!(log.event.kind, "error");
    assert_eq!(log.event.disposition, Some("unsupported"));
    assert_eq!(log.loads, ["file:///stage151/a.pkl"]);
    assert_eq!(
        log.resolves,
        ["file:///stage151/a.pkl", "file:///stage151/b.pkl"]
    );
    // This is deliberately NOT the Java SecurityManagerException raw diagnostic.
}

#[test]
fn legacy_resolved_completion_retains_source_identity_and_returns_typed_graph() {
    let log = execute(
        r#"import "pkl:analyze"
g = analyze.importGraph(Set("file:///stage151/a.pkl"))
output { text = if (g is analyze.ImportGraph && g.imports["file:///stage151/a.pkl"] == Set()) "ok" else throw("legacy contract") }
"#,
        &[("file:///stage151/a.pkl", "bad=throw(\"not evaluated\")\n")],
        None,
        true,
    );
    success(&log);
    assert_eq!(log.loads, ["file:///stage151/a.pkl"]);
}

#[test]
fn typed_graph_keeps_original_dot_uri_and_normalized_factory_edges() {
    let log = execute(
        r#"import "pkl:analyze"
g = analyze.importGraph(Set("file:///stage151/dir/../a.pkl"))
output { text = if (g is analyze.ImportGraph &&
  g.imports.keys.toList() == List("file:///stage151/dir/../a.pkl", "file:///stage151/nested/../b.pkl") &&
  g.imports["file:///stage151/dir/../a.pkl"].first.uri == "file:/stage151/b.pkl" &&
  g.resolvedImports["file:///stage151/dir/../a.pkl"] == "resolved:file:/stage151/a.pkl") "ok" else throw("URI identity contract") }
"#,
        &[
            (
                "file:/stage151/a.pkl",
                "import \"file:///stage151/nested/../b.pkl\"\nunused=throw(\"not evaluated\")\n",
            ),
            ("file:/stage151/b.pkl", "unused=read(\"forbidden:x\")\n"),
        ],
        None,
        false,
    );
    success(&log);
    assert_eq!(log.loads, ["file:/stage151/a.pkl", "file:/stage151/b.pkl"]);
    assert_eq!(
        log.resolves,
        ["file:/stage151/a.pkl", "file:/stage151/b.pkl"]
    );
}
