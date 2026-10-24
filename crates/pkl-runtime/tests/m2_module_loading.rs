#[path = "support/module_keys.rs"]
mod module_keys;
use std::sync::Arc;

use pkl_runtime::evaluation::frontend::{
    ModuleEvalOptions, ModuleSourceInput, evaluate_module_output_with_host,
};
use pkl_runtime::evaluation::session::{SourceSession, SourceSessionOptions};
use pkl_runtime::host::HostPoll;
use pkl_runtime::modules::{
    ModuleCompletion, ModuleHostError, ModuleRequest, ModuleRequestKind, ModuleSource,
    ResolvedModule,
};
use pkl_runtime::value::{RuntimeError, Value};

fn resolve(request: &ModuleRequest, canonical: Option<&str>) -> ModuleCompletion {
    let ModuleRequestKind::Resolve { original_uri, .. } = &request.kind else {
        panic!("resolve request");
    };
    let resolved_uri: Arc<str> = canonical.map_or_else(|| original_uri.clone(), Arc::from);
    ModuleCompletion::Resolved(Ok(ResolvedModule {
        original_uri: original_uri.clone(),
        source_uri: resolved_uri.clone(),
        resolved_uri,
    }))
}

fn source(text: &str) -> ModuleCompletion {
    ModuleCompletion::Source(Ok(ModuleSource {
        text: text.into(),
        inferred_name: "imported".into(),
    }))
}

#[test]
fn original_three_import_sources_use_actual_output_and_shared_imported_scope() {
    // Original pinned inputs and expected PCF remain unchanged. Independent
    // current CLI acquisitions additionally establish the execution boundary.
    let cases = [
        (include_str!("../../../external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/basic/import1.pkl"), include_bytes!("../../../external/pkl/pkl-core/src/test/files/LanguageSnippetTests/output/basic/import1.pcf").as_slice()),
        (include_str!("../../../external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/basic/import2.pkl"), include_bytes!("../../../external/pkl/pkl-core/src/test/files/LanguageSnippetTests/output/basic/import2.pcf").as_slice()),
        (include_str!("../../../external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/basic/import3.pkl"), include_bytes!("../../../external/pkl/pkl-core/src/test/files/LanguageSnippetTests/output/basic/import3.pcf").as_slice()),
    ];
    let imported = include_str!(
        "../../../external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/basic/imported.pkl"
    );
    for (text, expected) in cases {
        let mut resolves = 0;
        let mut loads = 0;
        let result = evaluate_module_output_with_host(
            ModuleSourceInput {
                text,
                uri: "file:///corpus/basic/main.pkl",
                inferred_name: "main",
            },
            &ModuleEvalOptions::default(),
            |request| match &request.kind {
                ModuleRequestKind::ResolveKey { .. } => {
                    panic!("public lifecycle request in supplied-source test")
                }
                ModuleRequestKind::SelectKey { .. }
                | ModuleRequestKind::QueryCachePolicy { .. }
                | ModuleRequestKind::QueryFragmentPaths { .. }
                | ModuleRequestKind::ResolveImportUri { .. } => module_keys::complete(request),
                ModuleRequestKind::CheckImport { .. } => {
                    module_keys::defaults(request).expect("default import policy")
                }
                ModuleRequestKind::Capabilities { .. } | ModuleRequestKind::HasElement { .. } => {
                    panic!("unexpected ModuleKey preparation in this fixture")
                }
                ModuleRequestKind::Resolve { original_uri, .. } => {
                    assert_eq!(original_uri.as_ref(), "file:///corpus/basic/imported.pkl");
                    resolves += 1;
                    resolve(request, None)
                }
                ModuleRequestKind::LoadSource { .. } => {
                    loads += 1;
                    source(imported)
                }
            },
        );
        assert_eq!(
            result.exit_code,
            0,
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(result.stdout, expected);
        assert!(result.stderr.is_empty());
        assert_eq!((resolves, loads), (1, 1));
    }
}

fn session(text: &str, paths: &[&str]) -> SourceSession {
    SourceSession::compile_output(
        ModuleSourceInput {
            text,
            uri: "file:///loader/main.pkl",
            inferred_name: "main",
        },
        &paths.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
        &ModuleEvalOptions::default(),
        SourceSessionOptions::default(),
    )
    .unwrap()
}

fn settle(
    session: &mut SourceSession,
    host: &mut impl FnMut(&ModuleRequest) -> ModuleCompletion,
) -> Result<Value, RuntimeError> {
    for _ in 0..100_000 {
        match session.poll(0, 16) {
            HostPoll::Ready(value) => return value,
            HostPoll::Module { token, request } => {
                let completion = module_keys::defaults(&request).unwrap_or_else(|| host(&request));
                session.complete_module(&token, completion, 0).unwrap()
            }
            HostPoll::Read { token, .. } => {
                session.resume_default_read(&token, &[], &[], 0).unwrap()
            }
            HostPoll::Log { token, .. } => session.acknowledge_log(&token, Ok(()), 0).unwrap(),
            HostPoll::TransformFrame { .. } => panic!("unexpected default transformer callback"),
            HostPoll::Yielded => {}
        }
        session.collect(&[]);
    }
    panic!("module continuation did not settle");
}

#[test]
fn canonical_alias_hit_does_not_add_an_original_alias_and_cache_roots_survive_gc() {
    let mut session = session(
        "a=import(\"canonical.pkl\")\nb=import(\"alias.pkl\")\nc=import(\"alias.pkl\")\nresult=List(a == b,b == c).toString()\n",
        &["result"],
    );
    let mut resolves = 0;
    let mut loads = 0;
    session.start_path(0, 0, None).unwrap();
    let value = settle(&mut session, &mut |request| match &request.kind {
        ModuleRequestKind::ResolveKey { .. } => {
            panic!("public lifecycle request in supplied-source test")
        }
        ModuleRequestKind::SelectKey { .. }
        | ModuleRequestKind::QueryCachePolicy { .. }
        | ModuleRequestKind::QueryFragmentPaths { .. }
        | ModuleRequestKind::ResolveImportUri { .. } => module_keys::complete(request),
        ModuleRequestKind::CheckImport { .. } => {
            module_keys::defaults(request).expect("default import policy")
        }
        ModuleRequestKind::Capabilities { .. } | ModuleRequestKind::HasElement { .. } => {
            panic!("unexpected ModuleKey preparation in this fixture")
        }
        ModuleRequestKind::Resolve { .. } => {
            resolves += 1;
            resolve(request, Some("file:///loader/canonical.pkl"))
        }
        ModuleRequestKind::LoadSource { .. } => {
            loads += 1;
            source("x=1\n")
        }
    })
    .unwrap();
    assert_eq!(value, Value::string("List(true, true)"));
    assert_eq!((resolves, loads), (3, 1));
    session.close();
    assert_eq!(session.heap_stats().live, 0);
}

#[test]
fn resolution_failures_retry_but_source_and_frontend_failures_replay_the_first_error() {
    for failure in [0, 1, 2] {
        let mut session = session(
            "first=import(\"dep.pkl\").x\nsecond=import(\"dep.pkl\").x\n",
            &["first", "second"],
        );
        let mut resolves = 0;
        let mut loads = 0;
        let mut first = None;
        for index in [0, 1, 0] {
            session.start_path(index, 0, None).unwrap();
            let error = settle(&mut session, &mut |request| match &request.kind {
                ModuleRequestKind::ResolveKey { .. } => {
                    panic!("public lifecycle request in supplied-source test")
                }
                ModuleRequestKind::SelectKey { .. }
                | ModuleRequestKind::QueryCachePolicy { .. }
                | ModuleRequestKind::QueryFragmentPaths { .. }
                | ModuleRequestKind::ResolveImportUri { .. } => module_keys::complete(request),
                ModuleRequestKind::CheckImport { .. } => {
                    module_keys::defaults(request).expect("default import policy")
                }
                ModuleRequestKind::Capabilities { .. } | ModuleRequestKind::HasElement { .. } => {
                    panic!("unexpected ModuleKey preparation in this fixture")
                }
                ModuleRequestKind::Resolve { .. } => {
                    resolves += 1;
                    if failure == 0 {
                        ModuleCompletion::Resolved(Err(ModuleHostError::NotFound))
                    } else {
                        resolve(request, None)
                    }
                }
                ModuleRequestKind::LoadSource { .. } => {
                    loads += 1;
                    if failure == 1 {
                        ModuleCompletion::Source(Err(ModuleHostError::Io("broken source".into())))
                    } else {
                        source("x = (\n")
                    }
                }
            })
            .unwrap_err();
            if failure > 0 {
                if let Some(first) = &first {
                    assert_eq!(&error.frames, first);
                } else {
                    first = Some(error.frames.clone());
                }
                assert_eq!(error.frontend.is_some(), failure == 2);
            }
        }
        assert_eq!(
            (resolves, loads),
            if failure == 0 { (3, 0) } else { (1, 1) }
        );
        session.close();
        assert_eq!(session.heap_stats().live, 0);
    }
}

#[test]
fn malformed_unused_import_prepares_before_evaluation_but_missing_unused_file_is_lazy() {
    let mut requests = 0;
    for (text, status) in [
        ("import \"life:%GG\"\nresult=throw(\"body\")\n", 1),
        ("import \"missing.pkl\"\nresult=7\n", 0),
    ] {
        let result = evaluate_module_output_with_host(
            ModuleSourceInput {
                text,
                uri: "file:///loader/main.pkl",
                inferred_name: "main",
            },
            &ModuleEvalOptions::default(),
            |request| {
                if let ModuleRequestKind::SelectKey { .. } = &request.kind {
                    requests += 1;
                    module_keys::complete(request)
                } else {
                    module_keys::defaults(request)
                        .expect("unused import must not resolve or load source")
                }
            },
        );
        assert_eq!(
            result.exit_code,
            status,
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        if status == 0 {
            assert_eq!(result.stdout, b"result = 7\n");
        }
    }
    assert_eq!(requests, 1);
}

#[test]
fn appended_units_preserve_self_identity_and_distinguish_same_offset_declarations() {
    let text = "local a=import(\"a.pkl\")\nlocal b=import(\"b.pkl\")\nlocal selfModule=import(\"main.pkl\")\nresult=List(a == b,a.C == b.C,a.getClass() == b.getClass(),selfModule == module,a.value,b.value).toString()\n";
    let mut loads = 0;
    let result = evaluate_module_output_with_host(
        ModuleSourceInput {
            text,
            uri: "file:///loader/main.pkl",
            inferred_name: "main",
        },
        &ModuleEvalOptions::default(),
        |request| match &request.kind {
            ModuleRequestKind::ResolveKey { .. } => {
                panic!("public lifecycle request in supplied-source test")
            }
            ModuleRequestKind::SelectKey { .. }
            | ModuleRequestKind::QueryCachePolicy { .. }
            | ModuleRequestKind::QueryFragmentPaths { .. }
            | ModuleRequestKind::ResolveImportUri { .. } => module_keys::complete(request),
            ModuleRequestKind::CheckImport { .. } => {
                module_keys::defaults(request).expect("default import policy")
            }
            ModuleRequestKind::Capabilities { .. } | ModuleRequestKind::HasElement { .. } => {
                panic!("unexpected ModuleKey preparation in this fixture")
            }
            ModuleRequestKind::Resolve { .. } => resolve(request, None),
            ModuleRequestKind::LoadSource { .. } => {
                loads += 1;
                source("const bar=3\nclass C { x=bar }\nvalue=new C {}.x\n")
            }
        },
    );
    // Avoid recursively rendering selfModule: only the result is the output value.
    assert_eq!(loads, 2);
    assert_eq!(
        result.exit_code,
        0,
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(
        result.stdout,
        b"result = \"List(false, false, false, true, 3, 3)\"\n"
    );
}

fn next_module(session: &mut SourceSession) -> (String, ModuleRequest) {
    for _ in 0..100_000 {
        match session.poll(0, 8) {
            HostPoll::Module { token, request } => {
                if let Some(completion) = module_keys::defaults(&request) {
                    session.complete_module(&token, completion, 0).unwrap();
                } else {
                    return (token, request);
                }
            }
            HostPoll::TransformFrame { .. } => panic!("unexpected default transformer callback"),
            HostPoll::Yielded => {}
            other => panic!("expected module effect: {other:?}"),
        }
        session.collect(&[]);
    }
    panic!("missing module effect");
}

#[test]
fn source_callback_publication_survives_outer_failure_while_fresh_import_replays_failure() {
    // Fresh Java loader-oracle-03: source-in-flight is not a cache publication.
    // The callback independently initializes the module. Its member cache and
    // the outer attempt's later module-cache failure intentionally coexist.
    let mut session = session(
        "a=import(\"memory:dep\")\nvalue=a.x\nfresh=import(\"memory:dep\").x\n",
        &["value", "fresh"],
    );
    session.start_path(0, 0, None).unwrap();
    let (token, request) = next_module(&mut session);
    session
        .complete_module(&token, resolve(&request, None), 0)
        .unwrap();
    let (outer_token, request) = next_module(&mut session);
    assert!(matches!(request.kind, ModuleRequestKind::LoadSource { .. }));
    session.reenter_path(&outer_token, 0, 0, None).unwrap();
    let mut child_resolves = 0;
    let mut child_loads = 0;
    let child = settle(&mut session, &mut |request| match request.kind {
        ModuleRequestKind::ResolveKey { .. } => {
            panic!("public lifecycle request in supplied-source test")
        }
        ModuleRequestKind::SelectKey { .. }
        | ModuleRequestKind::QueryCachePolicy { .. }
        | ModuleRequestKind::QueryFragmentPaths { .. }
        | ModuleRequestKind::ResolveImportUri { .. } => module_keys::complete(request),
        ModuleRequestKind::CheckImport { .. } => {
            module_keys::defaults(request).expect("default import policy")
        }
        ModuleRequestKind::Capabilities { .. } | ModuleRequestKind::HasElement { .. } => {
            panic!("unexpected ModuleKey preparation in this fixture")
        }
        ModuleRequestKind::Resolve { .. } => {
            child_resolves += 1;
            resolve(request, None)
        }
        ModuleRequestKind::LoadSource { .. } => {
            child_loads += 1;
            source("x=9\n")
        }
    })
    .unwrap();
    assert_eq!(child, Value::string("9"));
    assert_eq!((child_resolves, child_loads), (1, 1));
    session.collect(&[]);
    session
        .complete_module(
            &outer_token,
            ModuleCompletion::Source(Err(ModuleHostError::Io("outer source failed".into()))),
            0,
        )
        .unwrap();
    let first_error = settle(&mut session, &mut |_| panic!("unexpected Host effect")).unwrap_err();
    session.start_path(0, 0, None).unwrap();
    assert_eq!(
        settle(&mut session, &mut |_| panic!("successful child cache lost")).unwrap(),
        Value::string("9")
    );
    session.start_path(1, 0, None).unwrap();
    let replay = settle(&mut session, &mut |_| {
        panic!("source failure was not cached")
    })
    .unwrap_err();
    assert_eq!(first_error.frames, replay.frames);
    session.close();
    assert_eq!(session.heap_stats().live, 0);
}

#[test]
fn closing_in_source_callback_finishes_issued_operation_and_rejects_new_entries() {
    let mut session = session(
        "result=import(\"memory:dep\").x\nstable=7\n",
        &["result", "stable"],
    );
    session.start_path(0, 0, None).unwrap();
    let (token, request) = next_module(&mut session);
    session
        .complete_module(&token, resolve(&request, None), 0)
        .unwrap();
    let (token, _) = next_module(&mut session);
    session.close_from_callback(&token).unwrap();
    assert!(session.reenter_path(&token, 1, 0, None).is_err());
    session.complete_module(&token, source("x=9\n"), 0).unwrap();
    let value = settle(&mut session, &mut |_| panic!("no additional module effect")).unwrap();
    assert_eq!(value, Value::string("9"));
    assert!(session.start_path(1, 0, None).is_err());
    assert_eq!(session.heap_stats().live, 0);
    assert_eq!(value, Value::string("9"));
}

#[test]
fn nested_module_cancellation_drains_each_owned_operation_and_reclaims_empty_receivers() {
    let mut session = session(
        "result=import(\"memory:outer\").x\ncallback=import(\"memory:inner\").x\n",
        &["result", "callback"],
    );
    session.start_path(0, 0, None).unwrap();
    let (token, request) = next_module(&mut session);
    session
        .complete_module(&token, resolve(&request, None), 0)
        .unwrap();
    let (outer, _) = next_module(&mut session);
    session.reenter_path(&outer, 1, 0, None).unwrap();
    let (inner, _) = next_module(&mut session);
    session.collect(&[]);
    session.cancel();
    assert_eq!(
        session.drain_cancelled_modules(),
        [outer.clone(), inner.clone()]
    );
    assert!(session.drain_cancelled_modules().is_empty());
    assert!(session.drain_cancelled_reads().is_empty());
    assert_eq!(session.heap_stats().live, 0);
    assert!(session.complete_module(&outer, source("x=1"), 0).is_err());
    assert!(
        session
            .complete_module(
                &inner,
                ModuleCompletion::Resolved(Err(ModuleHostError::NotFound)),
                0
            )
            .is_err()
    );
}

fn failed_append_keeps_existing_closures(bad_source: &str, kind: pkl_runtime::value::ErrorKind) {
    let mut session = session(
        "a=import(\"a.pkl\")\nfirst=a.f(2)\nbad=import(\"bad.pkl\").x\nlast=import(\"b.pkl\").f(3)+a.f(4)\n",
        &["first", "bad", "last"],
    );
    let mut host = |request: &ModuleRequest| match &request.kind {
        ModuleRequestKind::ResolveKey { .. } => {
            panic!("public lifecycle request in supplied-source test")
        }
        ModuleRequestKind::SelectKey { .. }
        | ModuleRequestKind::QueryCachePolicy { .. }
        | ModuleRequestKind::QueryFragmentPaths { .. }
        | ModuleRequestKind::ResolveImportUri { .. } => module_keys::complete(request),
        ModuleRequestKind::CheckImport { .. } => {
            module_keys::defaults(request).expect("default import policy")
        }
        ModuleRequestKind::Capabilities { .. } | ModuleRequestKind::HasElement { .. } => {
            panic!("unexpected ModuleKey preparation in this fixture")
        }
        ModuleRequestKind::Resolve { .. } => resolve(request, None),
        ModuleRequestKind::LoadSource { resolved, .. } => {
            if resolved.original_uri.ends_with("bad.pkl") {
                source(bad_source)
            } else {
                source("const base=7\nfunction f(x)=base+x\n")
            }
        }
    };
    session.start_path(0, 0, None).unwrap();
    assert_eq!(settle(&mut session, &mut host).unwrap(), Value::string("9"));
    session.start_path(1, 0, None).unwrap();
    let error = settle(&mut session, &mut host).unwrap_err();
    assert_eq!(error.kind, kind, "{error:?}");
    session.collect(&[]);
    session.start_path(2, 0, None).unwrap();
    assert_eq!(
        settle(&mut session, &mut host).unwrap(),
        Value::string("21")
    );
    session.close();
    assert_eq!(session.heap_stats().live, 0);
}

#[test]
fn imported_type_initialization_failure_keeps_existing_closures_and_later_units_valid() {
    // This exact Stage41 input now reaches real qualified-type resolution.
    failed_append_keeps_existing_closures(
        "import \"other.pkl\" as other\nfunction retained(x)=x+9\nx:other.C\n",
        pkl_runtime::value::ErrorKind::Type,
    );
}

#[test]
fn failed_compiler_append_keeps_existing_closures_and_later_units_valid() {
    // URI preparation fails in the later member after the function emitted
    // code into the shared append-only arenas.
    failed_append_keeps_existing_closures(
        "function retained(x)=x+9\nx=import(\"life:%GG\")\n",
        pkl_runtime::value::ErrorKind::Host,
    );
}

#[test]
fn failed_self_superclass_preserves_the_original_followup_method_error() {
    // Both complete Java errors and the original source are from the closed
    // loader-oracle-03 self-class-parent-error graph, including the second call.
    let mut session = SourceSession::compile_expressions(
        include_str!("../../../tests/fixtures/m2-module-loading/graphs/self-class-parent-error/primary.pkl"),
        "file:///Users/cody.ij.hwang/Projects/pkl-rs.stage41/target/m2/stage41-loader-oracle-03/workspace/self-class-parent-error/main.pkl",
        "main",
        &["answer".into()],
        SourceSessionOptions::default(),
    )
    .unwrap();
    for expected in [
        include_bytes!("../../../tests/fixtures/m2-module-loading/origins/stage41-loader-oracle-03/cases/self-class-parent-error/0/calls/outer-0.stderr.bin").as_slice(),
        include_bytes!("../../../tests/fixtures/m2-module-loading/origins/stage41-loader-oracle-03/cases/self-class-parent-error/0/calls/outer-1.stderr.bin").as_slice(),
    ] {
        session.start_path(0, 0, None).unwrap();
        let failure = settle(&mut session, &mut |request| match request.kind {
            ModuleRequestKind::ResolveKey { .. } => panic!("public lifecycle request in supplied-source test"),
                ModuleRequestKind::SelectKey { .. } | ModuleRequestKind::QueryCachePolicy { .. } | ModuleRequestKind::QueryFragmentPaths { .. } | ModuleRequestKind::ResolveImportUri { .. } => module_keys::complete(request),
                ModuleRequestKind::CheckImport { .. } => module_keys::defaults(request).expect("default import policy"),
            ModuleRequestKind::Capabilities { .. } | ModuleRequestKind::HasElement { .. } => panic!("unexpected ModuleKey preparation in this fixture"),
            ModuleRequestKind::Resolve { .. } => resolve(request, None),
            ModuleRequestKind::LoadSource { .. } => panic!("primary source already exists"),
        })
        .unwrap_err();
        assert_eq!(session.format_error(failure).as_bytes(), expected);
        session.collect(&[]);
    }
    session.close();
    assert_eq!(session.heap_stats().live, 0);
}

#[test]
fn imported_error_keeps_declared_module_and_original_display_source() {
    let mut session = session("first=import(\"alias.pkl\").value\n", &["first"]);
    session.start_path(0, 0, None).unwrap();
    let error = settle(&mut session, &mut |request| match &request.kind {
        ModuleRequestKind::ResolveKey { .. } => {
            panic!("public lifecycle request in supplied-source test")
        }
        ModuleRequestKind::SelectKey { .. }
        | ModuleRequestKind::QueryCachePolicy { .. }
        | ModuleRequestKind::QueryFragmentPaths { .. }
        | ModuleRequestKind::ResolveImportUri { .. } => module_keys::complete(request),
        ModuleRequestKind::CheckImport { .. } => {
            module_keys::defaults(request).expect("default import policy")
        }
        ModuleRequestKind::Capabilities { .. } | ModuleRequestKind::HasElement { .. } => {
            panic!("unexpected ModuleKey preparation in this fixture")
        }
        ModuleRequestKind::Resolve { original_uri, .. } => {
            ModuleCompletion::Resolved(Ok(ResolvedModule {
                original_uri: original_uri.clone(),
                resolved_uri: "file:///canonical/dep.pkl".into(),
                source_uri: original_uri.clone(),
            }))
        }
        ModuleRequestKind::LoadSource { .. } => {
            source("module declared.child\nclass C { x=throw(\"child\") }\nvalue=new C {}.x\n")
        }
    })
    .unwrap_err();
    let source = error.frames[0]
        .source
        .as_ref()
        .expect("foreign error source");
    assert_eq!(source.module_name.as_ref(), "declared.child");
    assert_eq!(source.uri.as_ref(), "file:///loader/alias.pkl");
    assert_eq!(error.frames[0].member.to_java_utf8(), "#C.x");
    assert!(
        error
            .frames
            .iter()
            .any(|frame| frame.source.is_none() && frame.member.to_java_utf8() == "#first")
    );
    session.close();
    assert_eq!(session.heap_stats().live, 0);
}

#[test]
fn original_unicode_primary_uri_keeps_the_complete_java_output_error() {
    let uri = pkl_runtime::modules::original_file_uri(
        "/Users/cody.ij.hwang/Projects/pkl-rs.stage42/target/m2/stage42-original-modules41-workspace-01/external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/modules/日本語_error.pkl",
    )
    .unwrap();
    let result = evaluate_module_output_with_host(
        ModuleSourceInput {
            text: include_str!(
                "../../../external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/modules/日本語_error.pkl"
            ),
            uri: &uri,
            inferred_name: "日本語_error",
        },
        &ModuleEvalOptions::default(),
        |_| panic!("this original input has no imports"),
    );
    assert_eq!(result.exit_code, 1);
    assert!(result.stdout.is_empty());
    assert_eq!(
        result.stderr,
        include_bytes!("../../../tests/fixtures/m2-module-uri/original-unicode-error/error.bin")
    );
}

#[test]
fn outer_source_success_replaces_callback_source_failure() {
    let mut session = session(
        "a=import(\"memory:dep\")\nvalue=a.x\nfresh=import(\"memory:dep\").x\n",
        &["value", "fresh"],
    );
    session.start_path(0, 0, None).unwrap();
    let (token, request) = next_module(&mut session);
    session
        .complete_module(&token, resolve(&request, None), 0)
        .unwrap();
    let (outer, _) = next_module(&mut session);
    session.reenter_path(&outer, 0, 0, None).unwrap();
    let error = settle(&mut session, &mut |request| match request.kind {
        ModuleRequestKind::ResolveKey { .. } => {
            panic!("public lifecycle request in supplied-source test")
        }
        ModuleRequestKind::SelectKey { .. }
        | ModuleRequestKind::QueryCachePolicy { .. }
        | ModuleRequestKind::QueryFragmentPaths { .. }
        | ModuleRequestKind::ResolveImportUri { .. } => module_keys::complete(request),
        ModuleRequestKind::CheckImport { .. } => {
            module_keys::defaults(request).expect("default import policy")
        }
        ModuleRequestKind::Capabilities { .. } | ModuleRequestKind::HasElement { .. } => {
            panic!("unexpected ModuleKey preparation in this fixture")
        }
        ModuleRequestKind::Resolve { .. } => resolve(request, None),
        ModuleRequestKind::LoadSource { .. } => {
            ModuleCompletion::Source(Err(ModuleHostError::Io("inner source failed".into())))
        }
    })
    .unwrap_err();
    assert_eq!(
        error.message,
        "I/O error loading module `memory:dep`.\nIOException: inner source failed"
    );
    session.collect(&[]);
    session.complete_module(&outer, source("x=9\n"), 0).unwrap();
    assert_eq!(
        settle(&mut session, &mut |_| panic!("outer already owns source")).unwrap(),
        Value::string("9")
    );
    session.start_path(1, 0, None).unwrap();
    assert_eq!(
        settle(&mut session, &mut |_| panic!(
            "outer publication must replace failure"
        ))
        .unwrap(),
        Value::string("9")
    );
    session.close();
    assert_eq!(session.heap_stats().live, 0);
}

#[test]
fn initialization_callback_observes_actual_empty_published_module() {
    let mut session = SourceSession::compile_output(
        ModuleSourceInput {
            text: "a=import(\"memory:dep\")\nvalue=a.x\n",
            uri: "file:///loader/main.pkl",
            inferred_name: "main",
        },
        &["value".into()],
        &ModuleEvalOptions::default(),
        SourceSessionOptions {
            log_delivery: pkl_runtime::logging::LogDelivery::Host,
            ..SourceSessionOptions::default()
        },
    )
    .unwrap();
    session.start_path(0, 0, None).unwrap();
    let (token, request) = next_module(&mut session);
    session
        .complete_module(&token, resolve(&request, None), 0)
        .unwrap();
    let (token, _) = next_module(&mut session);
    session
        .complete_module(
            &token,
            source("@Deprecated { when (trace(true)) { message=\"setup\" } }\nmodule child\nx=9\n"),
            0,
        )
        .unwrap();
    let token = loop {
        match session.poll(0, 8) {
            HostPoll::Log { token, .. } => break token,
            HostPoll::Yielded => {
                session.collect(&[]);
            }
            other => panic!("expected initialization trace, got {other:?}"),
        }
    };
    session.reenter_path(&token, 0, 0, None).unwrap();
    let missing = settle(&mut session, &mut |_| {
        panic!("published module must avoid another source load")
    })
    .unwrap_err();
    assert_eq!(missing.kind, pkl_runtime::value::ErrorKind::MissingMember);
    assert!(missing.message.starts_with("Cannot find property `x`"));
    session.collect(&[]);
    session.acknowledge_log(&token, Ok(()), 0).unwrap();
    assert_eq!(
        settle(&mut session, &mut |_| panic!("no extra source effect")).unwrap(),
        Value::string("9")
    );
    session.close();
    assert_eq!(session.heap_stats().live, 0);
}

#[test]
fn imported_resource_read_keeps_module_key_separate_from_display_and_resolved_uris() {
    let mut session = session("result=import(\"alias.pkl\").value\n", &["result"]);
    session.start_path(0, 0, None).unwrap();
    let (token, request) = next_module(&mut session);
    let ModuleRequestKind::Resolve { original_uri, .. } = request.kind else {
        panic!("resolve");
    };
    session
        .complete_module(
            &token,
            ModuleCompletion::Resolved(Ok(ResolvedModule {
                original_uri,
                resolved_uri: "file:///canonical/dep.pkl".into(),
                source_uri: "https://display.invalid/dep.pkl".into(),
            })),
            0,
        )
        .unwrap();
    let (token, _) = next_module(&mut session);
    session
        .complete_module(&token, source("value=read(\"../data/value.txt\")\n"), 0)
        .unwrap();
    let token = loop {
        match session.poll(0, 8) {
            HostPoll::Module { token, request } => {
                session
                    .complete_module(&token, module_keys::complete(&request), 0)
                    .unwrap();
            }
            HostPoll::Read { token, uri } => {
                assert_eq!(uri.as_ref(), "../data/value.txt");
                break token;
            }
            HostPoll::Yielded => {
                session.collect(&[]);
            }
            other => panic!("expected imported resource read, got {other:?}"),
        }
    };
    session.resume_default_read(&token, &[], &[], 0).unwrap();
    let error = settle(&mut session, &mut |_| panic!("no extra import")).unwrap_err();
    // The explicitly unsupported default file reader reports the URI after
    // shared resolution, proving it used the declaration key, not display URL.
    assert_eq!(error.kind, pkl_runtime::value::ErrorKind::Unsupported);
    assert_eq!(
        error.message,
        "The default resource Host does not yet implement allowed `file` reads (`file:///data/value.txt`)."
    );
    assert_eq!(
        error.frames[0].source.as_ref().unwrap().uri.as_ref(),
        "https://display.invalid/dep.pkl"
    );
    session.close();
    assert_eq!(session.heap_stats().live, 0);
}
