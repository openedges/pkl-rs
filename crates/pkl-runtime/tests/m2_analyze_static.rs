//! Authored continuation invariants, not Java differential expectations.
//! Full pkl:analyze API/errors and original four cases remain unqualified.
use pkl_runtime::{
    modules::{analyze::*, *},
    source::SourceText,
    value::PklString,
};
use std::collections::BTreeMap;

fn run(
    roots: &[&str],
    sources: &[(&str, &str)],
) -> (Result<ImportGraph, Box<AnalysisFailure>>, Vec<String>) {
    let sources: BTreeMap<_, _> = sources
        .iter()
        .map(|(k, v)| (PklString::from(*k), *v))
        .collect();
    let mut run = ImportGraphAnalysis::new(roots.iter().map(|uri| PklString::from(*uri)));
    let mut events = Vec::new();
    for _ in 0..200 {
        let request = match run.poll() {
            AnalysisPoll::Ready(result) => {
                run.close();
                assert!(matches!(run.poll(), AnalysisPoll::Closed));
                return (result, events);
            }
            AnalysisPoll::Request(request) => request,
            AnalysisPoll::Closed => panic!("unexpected closed analysis"),
        };
        let completion = match &request.kind {
            ModuleRequestKind::SelectKey { id, uri, .. } => {
                events.push(format!("select:{uri}"));
                ModuleCompletion::KeySelected(Ok(ModuleKey {
                    id: *id,
                    uri: uri.clone(),
                    kind: ModuleKeyKind::Host,
                    handle: uri.clone(),
                }))
            }
            ModuleRequestKind::ResolveKey {
                key,
                resolved_key_id,
                ..
            } => {
                events.push(format!("resolve:{}", key.uri));
                ModuleCompletion::KeyResolved(if sources.contains_key(&key.uri) {
                    Ok(ResolvedModuleKey {
                        id: *resolved_key_id,
                        key: key.clone(),
                        handle: key.handle.clone(),
                        module: ResolvedModule {
                            original_uri: key.uri.clone(),
                            resolved_uri: key.uri.clone(),
                            source_uri: key.uri.clone(),
                        },
                    })
                } else {
                    Err(ModuleHostError::NotFound)
                })
            }
            ModuleRequestKind::LoadSource { key, .. } => {
                events.push(format!("load:{}", key.key.uri));
                ModuleCompletion::Source(Ok(ModuleSource {
                    text: SourceText::from(*sources.get(&key.key.uri).unwrap()),
                    inferred_name: "fixture".into(),
                }))
            }
            ModuleRequestKind::ResolveImportUri { key, authored_uri } => {
                events.push(format!("uri:{}:{authored_uri}", key.uri));
                // This profile delegates URI semantics to the existing shared
                // standard completion. No guest module is evaluated.
                default_completion(&request).expect("shared lexical URI resolution")
            }
            other => panic!("unexpected analysis capability {other:?}"),
        };
        run.complete(request.id, completion).unwrap();
    }
    panic!("authored finite graph did not terminate")
}
#[test]
fn branching_graph_parses_throw_trace_and_read_without_guest_execution() {
    let (graph, events) = run(
        &["file:///main.pkl"],
        &[
            (
                "file:///main.pkl",
                "import \"b.pkl\"\nimport \"a.pkl\"\nx = throw(\"unused\")\ny = trace(\"unused\")\nz = read(\"proto:unused\")\n",
            ),
            ("file:///a.pkl", "x=1\n"),
            ("file:///b.pkl", "x=2\n"),
        ],
    );
    let graph = graph.unwrap();
    assert_eq!(
        graph
            .nodes
            .iter()
            .map(|node| node.uri.clone())
            .collect::<Vec<_>>(),
        vec![
            PklString::from("file:///a.pkl"),
            "file:///b.pkl".into(),
            "file:///main.pkl".into()
        ]
    );
    assert_eq!(
        graph.nodes[2].imports,
        vec![PklString::from("file:///a.pkl"), "file:///b.pkl".into()]
    );
    assert!(!graph.traversal_order_qualified);
    let second_resolve = events
        .iter()
        .position(|e| e == "resolve:file:///a.pkl")
        .unwrap();
    let first_child_load = events
        .iter()
        .position(|e| e == "load:file:///b.pkl")
        .unwrap();
    assert!(
        second_resolve < first_child_load,
        "all direct keys resolve before descent"
    );
}
#[test]
fn cycle_resolves_back_edge_without_reloading_the_visited_node() {
    let (graph, events) = run(
        &["file:///a.pkl"],
        &[
            ("file:///a.pkl", "import \"b.pkl\"\n"),
            ("file:///b.pkl", "import \"a.pkl\"\n"),
        ],
    );
    assert_eq!(graph.unwrap().nodes.len(), 2);
    assert_eq!(
        events.iter().filter(|e| *e == "load:file:///a.pkl").count(),
        1
    );
    assert_eq!(
        events
            .iter()
            .filter(|e| *e == "resolve:file:///a.pkl")
            .count(),
        2
    );
}
#[test]
fn static_visitor_finds_dead_and_type_constraint_imports_but_stops_at_read() {
    let (graph, events) = run(
        &["file:///main.pkl"],
        &[
            (
                "file:///main.pkl",
                "x=if(false) import(\"a.pkl\") else null\nclass C { y: Int(import(\"b.pkl\").ok) = 1 }\nz=read(import(\"absent.pkl\").uri)\n",
            ),
            ("file:///a.pkl", "x=1"),
            ("file:///b.pkl", "ok=true"),
        ],
    );
    assert_eq!(graph.unwrap().nodes.len(), 3);
    assert!(!events.iter().any(|e| e.contains("absent.pkl")));
}
#[test]
fn complete_syntax_failure_precedes_import_resolution_and_keeps_source() {
    let text = "import \"absent.pkl\"\nx=(\n";
    let (result, events) = run(&["file:///main.pkl"], &[("file:///main.pkl", text)]);
    let error = result.unwrap_err();
    assert_eq!(error.phase, "parse");
    assert!(matches!(error.cause, FailureCause::Syntax(_)));
    assert_eq!(error.site.unwrap().source, SourceText::from(text));
    assert!(!events.iter().any(|e| e.contains("absent.pkl")));
}
#[test]
fn all_root_uri_validation_precedes_any_module_request() {
    let (result, events) = run(&["file:///main.pkl", "relative.pkl"], &[]);
    assert_eq!(result.unwrap_err().phase, "input URI");
    assert!(events.is_empty());
}
#[test]
fn missing_import_retains_authored_site_and_typed_failure() {
    let text = "import \"absent.pkl\"\nx=throw(\"unused\")\n";
    let (result, _) = run(&["file:///main.pkl"], &[("file:///main.pkl", text)]);
    let error = result.unwrap_err();
    assert_eq!(error.phase, "resolve");
    assert!(matches!(
        error.cause,
        FailureCause::Host(ModuleHostError::NotFound)
    ));
    let site = error.site.unwrap();
    assert_eq!(site.source_uri, "file:///main.pkl");
    assert_eq!(
        site.source.slice(site.span).unwrap().to_pkl_string(),
        "\"absent.pkl\""
    );
}
#[test]
fn stale_or_wrong_completion_does_not_consume_request_and_close_abandons_it() {
    let mut analysis = ImportGraphAnalysis::new(["file:///a.pkl".into()]);
    let AnalysisPoll::Request(first) = analysis.poll() else {
        panic!("request")
    };
    assert!(
        analysis
            .complete(first.id + 1, ModuleCompletion::ImportChecked(Ok(())))
            .is_err()
    );
    assert!(
        analysis
            .complete(first.id, ModuleCompletion::ImportChecked(Ok(())))
            .is_err()
    );
    let AnalysisPoll::Request(again) = analysis.poll() else {
        panic!("same request")
    };
    assert_eq!(first, again);
    analysis.close();
    assert!(matches!(analysis.poll(), AnalysisPoll::Closed));
    assert!(
        analysis
            .complete(first.id, ModuleCompletion::ImportChecked(Ok(())))
            .is_err()
    );
}
#[test]
fn empty_input_returns_an_empty_graph_without_host_requests() {
    let (graph, events) = run(&[], &[]);
    assert!(graph.unwrap().nodes.is_empty());
    assert!(events.is_empty());
}
#[test]
fn component_order_uses_numeric_ports_and_uri_equality_preserves_first_key_spelling() {
    let (graph, _) = run(
        &[
            "http://host:100/a",
            "http://host:9/a",
            "HTTP://HOST/a",
            "http://host/a",
        ],
        &[
            ("http://host:100/a", ""),
            ("http://host:9/a", ""),
            ("HTTP://HOST/a", ""),
            ("http://host/a", ""),
        ],
    );
    let nodes = graph.unwrap().nodes;
    assert_eq!(
        nodes
            .iter()
            .map(|node| node.uri.clone())
            .collect::<Vec<_>>(),
        vec![
            PklString::from("HTTP://HOST/a"),
            "http://host:9/a".into(),
            "http://host:100/a".into()
        ]
    );
    assert_eq!(nodes[0].resolved_uri, "http://host/a");
}

#[test]
fn graph_uris_keep_dot_segments_but_factory_and_load_use_normalized_keys() {
    let (graph, events) = run(
        &["file:///dir/../a.pkl"],
        &[
            ("file:/a.pkl", "import \"file:///nested/../b.pkl\"\n"),
            ("file:/b.pkl", "unused=throw(\"not evaluated\")\n"),
        ],
    );
    let nodes = graph.unwrap().nodes;
    assert_eq!(
        nodes
            .iter()
            .map(|node| node.uri.clone())
            .collect::<Vec<_>>(),
        vec![
            PklString::from("file:///dir/../a.pkl"),
            "file:///nested/../b.pkl".into()
        ]
    );
    assert_eq!(nodes[0].resolved_uri, "file:/a.pkl");
    assert_eq!(nodes[0].imports, vec![PklString::from("file:/b.pkl")]);
    assert_eq!(nodes[1].resolved_uri, "file:/b.pkl");
    assert_eq!(
        events,
        vec![
            "select:file:/a.pkl",
            "resolve:file:/a.pkl",
            "load:file:/a.pkl",
            "uri:file:/a.pkl:file:///nested/../b.pkl",
            "select:file:/b.pkl",
            "resolve:file:/b.pkl",
            "load:file:/b.pkl"
        ]
    );
}
#[test]
fn normalization_keeps_all_root_validation_before_any_host_effect() {
    let (result, events) = run(&["file:///dir/../a.pkl", "relative.pkl"], &[]);
    assert_eq!(result.unwrap_err().phase, "input URI");
    assert!(events.is_empty());
}
#[test]
fn selected_key_owns_loading_uri_without_replacing_graph_uri() {
    let mut analysis = ImportGraphAnalysis::new(["file:///dir/../a.pkl".into()]);
    let AnalysisPoll::Request(selected) = analysis.poll() else {
        panic!("select")
    };
    let ModuleRequestKind::SelectKey { id, uri, .. } = &selected.kind else {
        panic!("select kind")
    };
    assert_eq!(uri, "file:/a.pkl");
    let key = ModuleKey {
        id: *id,
        uri: "memory:/owned.pkl".into(),
        kind: ModuleKeyKind::Host,
        handle: "provider-owned".into(),
    };
    let mut invalid = key.clone();
    invalid.uri = "relative.pkl".into();
    assert!(
        analysis
            .complete(selected.id, ModuleCompletion::KeySelected(Ok(invalid)))
            .is_err()
    );
    let AnalysisPoll::Request(retry) = analysis.poll() else {
        panic!("retry")
    };
    assert_eq!(selected, retry);
    analysis
        .complete(selected.id, ModuleCompletion::KeySelected(Ok(key.clone())))
        .unwrap();
    let AnalysisPoll::Request(resolve) = analysis.poll() else {
        panic!("resolve")
    };
    let ModuleRequestKind::ResolveKey {
        key: actual,
        resolved_key_id,
        ..
    } = &resolve.kind
    else {
        panic!("resolve kind")
    };
    assert_eq!(actual, &key);
    let resolved = ResolvedModuleKey {
        id: *resolved_key_id,
        key: key.clone(),
        handle: "resolved-owned".into(),
        module: ResolvedModule {
            original_uri: key.uri.clone(),
            resolved_uri: "physical:/module.pkl".into(),
            source_uri: "display:/module.pkl".into(),
        },
    };
    analysis
        .complete(
            resolve.id,
            ModuleCompletion::KeyResolved(Ok(resolved.clone())),
        )
        .unwrap();
    let AnalysisPoll::Request(load) = analysis.poll() else {
        panic!("load")
    };
    let ModuleRequestKind::LoadSource {
        key: actual,
        resolved: module,
    } = &load.kind
    else {
        panic!("load kind")
    };
    assert_eq!(actual, &resolved);
    assert_eq!(module, &resolved.module);
    analysis
        .complete(
            load.id,
            ModuleCompletion::Source(Ok(ModuleSource {
                text: "unused=throw(\"not evaluated\")".into(),
                inferred_name: "owned".into(),
            })),
        )
        .unwrap();
    let AnalysisPoll::Ready(Ok(graph)) = analysis.poll() else {
        panic!("graph")
    };
    assert_eq!(graph.nodes.len(), 1);
    assert_eq!(graph.nodes[0].uri, "file:///dir/../a.pkl");
    assert_eq!(graph.nodes[0].resolved_uri, "physical:/module.pkl");
    assert!(graph.nodes[0].imports.is_empty());
}
