#![forbid(unsafe_code)]

use pkl_runtime::host::ReadFailure;
use pkl_runtime::logging::FrameTransformer;
use pkl_runtime::modules::{
    ModuleCompletion, ModuleKey, ModuleKeyKind, ModuleRequest, ModuleRequestKind, ModuleSource,
    ResolvedModule, ResolvedModuleKey,
};
use pkl_runtime::repl::{
    HELP, ReplCommandKind, ReplPoll, ReplRequest, ReplResponse, ReplSession, ReplSessionOptions,
    WELCOME, examples, format_ambiguous_commands, format_unknown_command, highlight_backticks,
    match_commands,
};
use pkl_runtime::resources::ReadResult;
use pkl_runtime::source::SourceText;
use pkl_runtime::value::PklString;
use std::collections::{BTreeMap, BTreeSet};

struct MockHost {
    env: BTreeMap<String, String>,
    props: BTreeMap<String, String>,
    modules: BTreeMap<String, String>,
    logs: Vec<String>,
}

impl MockHost {
    fn new() -> Self {
        let mut env = BTreeMap::new();
        env.insert("NAME1".into(), "value1".into());
        env.insert("NAME2".into(), "value2".into());
        let mut props = BTreeMap::new();
        props.insert("name1".into(), "value1".into());
        props.insert("name2".into(), "value2".into());
        Self {
            env,
            props,
            modules: BTreeMap::new(),
            logs: Vec::new(),
        }
    }

    fn run(&mut self, session: &mut ReplSession, req: ReplRequest) -> Vec<ReplResponse> {
        session.start_request(req, 0, None).expect("start_request");
        loop {
            match session.poll(0, 32_768) {
                ReplPoll::Ready(responses) => return responses,
                ReplPoll::Yielded => {}
                ReplPoll::Read { token, .. } => {
                    let read_req = session.read_request(&token).expect("read_request");
                    let uri = read_req.uri.to_java_utf8();
                    let res = if let Some(key) = uri.strip_prefix("env:") {
                        match self.env.get(key) {
                            Some(v) => Ok(ReadResult::Text(PklString::from(v.as_str()))),
                            None => Ok(ReadResult::Missing),
                        }
                    } else if let Some(key) = uri.strip_prefix("prop:") {
                        match self.props.get(key) {
                            Some(v) => Ok(ReadResult::Text(PklString::from(v.as_str()))),
                            None => Ok(ReadResult::Missing),
                        }
                    } else {
                        Err(ReadFailure::EvalError {
                            message: format!("unsupported mock resource {uri}").into(),
                            hint: None,
                        })
                    };
                    session
                        .complete_read(&token, res, 0)
                        .expect("complete_read");
                }
                ReplPoll::Module { token, request } => {
                    let completion = self.handle_module(&request);
                    session
                        .complete_module(&token, completion, 0)
                        .expect("complete_module");
                }
                ReplPoll::Glob { token, .. } => {
                    session
                        .complete_glob(
                            &token,
                            pkl_runtime::glob::GlobCompletion::Elements(Err(
                                pkl_runtime::glob::GlobHostError::Failure(
                                    pkl_runtime::modules::ModuleHostError::Unsupported(
                                        "no glob".into(),
                                    ),
                                ),
                            )),
                            0,
                        )
                        .expect("complete_glob");
                }
                ReplPoll::Log { token, event } => {
                    self.logs.push(format!(
                        "{:?}: {} ({})",
                        event.level,
                        event.message.to_java_utf8(),
                        event.frame.module_uri.to_java_utf8()
                    ));
                    session
                        .acknowledge_log(&token, Ok(()), 0)
                        .expect("acknowledge_log");
                }
                ReplPoll::TransformFrame { token, frame } => {
                    let transformed = FrameTransformer::Default.transform((*frame).clone());
                    session
                        .complete_frame_transform(&token, Ok(transformed), 0)
                        .expect("complete_frame_transform");
                }
            }
        }
    }

    fn handle_module(&self, request: &ModuleRequest) -> ModuleCompletion {
        match &request.kind {
            ModuleRequestKind::SelectKey { id, uri, .. } => {
                let s = uri.to_java_utf8();
                if s.starts_with("pkl:") {
                    ModuleCompletion::KeySelected(Ok(ModuleKey {
                        id: *id,
                        uri: uri.clone(),
                        kind: ModuleKeyKind::StandardLibrary,
                        handle: "stdlib".into(),
                    }))
                } else {
                    ModuleCompletion::KeySelected(Ok(ModuleKey {
                        id: *id,
                        uri: uri.clone(),
                        kind: ModuleKeyKind::Host,
                        handle: "mock".into(),
                    }))
                }
            }
            ModuleRequestKind::QueryCachePolicy { .. } => ModuleCompletion::CachePolicy(Ok(true)),
            ModuleRequestKind::Resolve {
                key,
                resolved_key_id,
                ..
            }
            | ModuleRequestKind::ResolveKey {
                key,
                resolved_key_id,
                ..
            } => ModuleCompletion::KeyResolved(Ok(ResolvedModuleKey {
                id: *resolved_key_id,
                key: key.clone(),
                handle: key.handle.clone(),
                module: ResolvedModule {
                    original_uri: key.uri.clone(),
                    resolved_uri: key.uri.clone(),
                    source_uri: key.uri.clone(),
                },
            })),
            ModuleRequestKind::LoadSource { resolved, .. } => {
                let uri = resolved.source_uri.to_java_utf8();
                match self.modules.get(&uri) {
                    Some(text) => ModuleCompletion::Source(Ok(ModuleSource {
                        text: SourceText::from(text.as_str()),
                        inferred_name: "".into(),
                    })),
                    None => ModuleCompletion::Source(Err(
                        pkl_runtime::modules::ModuleHostError::NotFound,
                    )),
                }
            }
            _ => pkl_runtime::modules::default_completion(request)
                .unwrap_or(ModuleCompletion::CachePolicy(Ok(true))),
        }
    }
}

fn eval_single_ok(host: &mut MockHost, session: &mut ReplSession, id: &str, text: &str) -> String {
    let res = host.run(
        session,
        ReplRequest::Eval {
            id: id.into(),
            text: text.into(),
            eval_definitions: false,
            force_results: false,
        },
    );
    assert_eq!(res.len(), 1, "expected 1 response, got {res:?}");
    match &res[0] {
        ReplResponse::EvalSuccess { result } => result.clone(),
        other => panic!("expected EvalSuccess, got {other:?}"),
    }
}

fn eval_single_err(host: &mut MockHost, session: &mut ReplSession, id: &str, text: &str) -> String {
    let res = host.run(
        session,
        ReplRequest::Eval {
            id: id.into(),
            text: text.into(),
            eval_definitions: false,
            force_results: false,
        },
    );
    assert_eq!(res.len(), 1, "expected 1 response, got {res:?}");
    match &res[0] {
        ReplResponse::EvalError { message } => message.clone(),
        other => panic!("expected EvalError, got {other:?}"),
    }
}

#[test]
fn complete_members_of_local_property_matches_upstream() {
    let mut host = MockHost::new();
    let mut session = ReplSession::new(ReplSessionOptions::default());

    let r0 = host.run(
        &mut session,
        ReplRequest::Eval {
            id: "id".into(),
            text: "local foo = new { bar = 10 }".into(),
            eval_definitions: false,
            force_results: false,
        },
    );
    assert!(r0.is_empty());

    let res = host.run(
        &mut session,
        ReplRequest::Completion {
            id: "id".into(),
            text: "foo".into(),
        },
    );
    assert_eq!(res.len(), 1);
    let expected: BTreeSet<String> = [
        "default",
        "bar",
        "toList()",
        "toMap()",
        "getProperty(",
        "getPropertyOrNull(",
        "hasProperty(",
        "ifNonNull(",
        "length()",
        "getClass()",
        "toString()",
        "toTyped(",
    ]
    .into_iter()
    .map(String::from)
    .collect();
    assert_eq!(res[0], ReplResponse::Completion { members: expected });
}

#[test]
fn complete_members_of_module_import_this_and_empty() {
    let mut host = MockHost::new();
    let mut session = ReplSession::new(ReplSessionOptions::default());

    host.run(
        &mut session,
        ReplRequest::Eval {
            id: "id".into(),
            text: "import \"pkl:test\"".into(),
            eval_definitions: false,
            force_results: false,
        },
    );
    let res = host.run(
        &mut session,
        ReplRequest::Completion {
            id: "id".into(),
            text: "test".into(),
        },
    );
    let ReplResponse::Completion { members } = &res[0] else {
        panic!("expected completion, got {res:?}");
    };
    assert!(members.contains("relativePathTo("), "members: {members:?}");
    assert!(members.contains("catch("), "members: {members:?}");

    // `this` completion
    let mut session2 = ReplSession::new(ReplSessionOptions::default());
    let def = host.run(
        &mut session2,
        ReplRequest::Eval {
            id: "id".into(),
            text: "x = 1; function f() = 3".into(),
            eval_definitions: false,
            force_results: false,
        },
    );
    assert!(def.is_empty());
    let this_res = host.run(
        &mut session2,
        ReplRequest::Completion {
            id: "id".into(),
            text: "this".into(),
        },
    );
    let expected_this: BTreeSet<String> = [
        "output",
        "toDynamic()",
        "toMap()",
        "f()",
        "x",
        "ifNonNull(",
        "getClass()",
        "getProperty(",
        "getPropertyOrNull(",
        "hasProperty(",
        "relativePathTo(",
        "toString()",
    ]
    .into_iter()
    .map(String::from)
    .collect();
    assert_eq!(
        this_res[0],
        ReplResponse::Completion {
            members: expected_this
        }
    );

    // empty expression completion
    let empty_res = host.run(
        &mut session2,
        ReplRequest::Completion {
            id: "id".into(),
            text: "".into(),
        },
    );
    let ReplResponse::Completion { members } = &empty_res[0] else {
        panic!("expected completion");
    };
    assert!(members.contains("x"));
    assert!(members.contains("Any"));
    assert!(members.contains("Regex("));
    assert!(members.contains("Infinity"));
}

#[test]
fn read_env_and_external_property() {
    let mut host = MockHost::new();
    let mut session = ReplSession::new(ReplSessionOptions::default());
    assert_eq!(
        eval_single_ok(&mut host, &mut session, "pkl0", r#"read("env:NAME1")"#),
        "\"value1\""
    );
    assert_eq!(
        eval_single_ok(&mut host, &mut session, "pkl1", r#"read("prop:name1")"#),
        "\"value1\""
    );
}

#[test]
fn replace_untyped_property_and_method_with_typed() {
    let mut host = MockHost::new();
    let mut session = ReplSession::new(ReplSessionOptions::default());

    assert_eq!(
        eval_single_ok(&mut host, &mut session, "pkl0", "timeout = 5.ms; timeout"),
        "5.ms"
    );
    assert_eq!(
        eval_single_ok(&mut host, &mut session, "pkl1", "timeout = 10; timeout"),
        "10"
    );
    assert_eq!(
        eval_single_ok(
            &mut host,
            &mut session,
            "pkl2",
            "timeout: Duration = 8.ms; timeout"
        ),
        "8.ms"
    );
    let err = eval_single_err(&mut host, &mut session, "pkl3", "timeout = 12; timeout");
    assert!(
        err.contains("Expected value of type `Duration`, but got type `Int`."),
        "got: {err}"
    );
    assert!(err.contains("at repl#timeout (repl:pkl2)"), "got: {err}");
    assert!(err.contains("at repl#timeout (repl:pkl3)"), "got: {err}");

    assert_eq!(
        eval_single_ok(
            &mut host,
            &mut session,
            "pkl4",
            r#"function greet(name) = "Hello, \(name)!"; greet("Pigeon")"#
        ),
        "\"Hello, Pigeon!\""
    );
    assert_eq!(
        eval_single_ok(&mut host, &mut session, "pkl5", "greet(42)"),
        "\"Hello, 42!\""
    );
    assert_eq!(
        eval_single_ok(
            &mut host,
            &mut session,
            "pkl6",
            r#"function greet(name: String): String = "Hello, \(name)!"; greet("Pigeon")"#
        ),
        "\"Hello, Pigeon!\""
    );
    let merr = eval_single_err(&mut host, &mut session, "pkl7", "greet(44)");
    assert!(
        merr.contains("Expected value of type `String`, but got type `Int`."),
        "got: {merr}"
    );
}

#[test]
fn syntax_highlighting_and_preamble_stripping_match_upstream() {
    let mut host = MockHost::new();
    let mut colored = ReplSession::new(ReplSessionOptions {
        color: true,
        ..ReplSessionOptions::default()
    });
    assert_eq!(
        eval_single_ok(&mut host, &mut colored, "id", "5.ms"),
        "\x1b[32m5\x1b[0m.ms"
    );

    let mut plain = ReplSession::new(ReplSessionOptions::default());
    let err = eval_single_err(&mut host, &mut plain, "id", "foo");
    assert_eq!(
        err,
        "–– Pkl Error ––\nCannot find property `foo`.\n\n1 | foo\n    ^^^\nat  (repl:id)\n"
    );
}

#[test]
fn lazy_display_force_late_binding_local_drop_load_and_reset() {
    let mut host = MockHost::new();
    host.modules.insert(
        "file:///sample.pkl".into(),
        "loadedGreeting = \"hello from file\"\nbaseVal = 99\n".into(),
    );
    let mut session = ReplSession::new(ReplSessionOptions::default());

    // Lazy rendering vs :force deep forcing & cache update
    host.run(
        &mut session,
        ReplRequest::Eval {
            id: "pkl0".into(),
            text: "b { c = 1 + 2 }".into(),
            eval_definitions: false,
            force_results: false,
        },
    );
    assert_eq!(
        eval_single_ok(&mut host, &mut session, "pkl1", "b"),
        "new Dynamic {\n  c = ?\n}"
    );
    let forced = host.run(
        &mut session,
        ReplRequest::Eval {
            id: "pkl2".into(),
            text: "b".into(),
            eval_definitions: false,
            force_results: true,
        },
    );
    assert_eq!(
        forced,
        vec![ReplResponse::EvalSuccess {
            result: "new Dynamic {\n  c = 3\n}".into()
        }]
    );
    // Cache was updated by :force!
    assert_eq!(
        eval_single_ok(&mut host, &mut session, "pkl3", "b"),
        "new Dynamic {\n  c = 3\n}"
    );

    // Late-binding redefinition + local dropped across child module
    assert_eq!(
        eval_single_ok(
            &mut host,
            &mut session,
            "pkl4",
            "local k = 99; a = 1; d = a + 1; d"
        ),
        "2"
    );
    assert_eq!(
        eval_single_ok(&mut host, &mut session, "pkl5", "a = 10; d"),
        "11"
    );
    let k_err = eval_single_err(&mut host, &mut session, "pkl6", "k");
    assert!(k_err.contains("Cannot find property `k`."), "got: {k_err}");

    // ModuleDecl is silently ignored
    let md = host.run(
        &mut session,
        ReplRequest::Eval {
            id: "pkl7".into(),
            text: "amends \"pkl:base\"".into(),
            eval_definitions: false,
            force_results: false,
        },
    );
    assert!(md.is_empty());

    // :load adopts loaded module, :reset clears it
    let load_res = host.run(
        &mut session,
        ReplRequest::Load {
            id: "pkl8".into(),
            uri: "sample.pkl".into(),
        },
    );
    assert!(load_res.is_empty(), "load_res: {load_res:?}");
    assert_eq!(
        eval_single_ok(&mut host, &mut session, "pkl9", "loadedGreeting"),
        "\"hello from file\""
    );
    assert_eq!(
        eval_single_ok(&mut host, &mut session, "pkl10", "baseVal * 2"),
        "198"
    );

    let reset_res = host.run(&mut session, ReplRequest::Reset { id: "pkl11".into() });
    assert!(reset_res.is_empty());
    let after_reset = eval_single_err(&mut host, &mut session, "pkl0", "loadedGreeting");
    assert!(after_reset.contains("Cannot find property `loadedGreeting`."));
}

#[test]
fn repl_commands_and_backtick_highlighting_match_upstream() {
    assert_eq!(match_commands(":h").len(), 1);
    assert_eq!(match_commands(":h")[0].kind, ReplCommandKind::Help);
    assert_eq!(match_commands(":q")[0].kind, ReplCommandKind::Quit);
    assert_eq!(match_commands(":f   x + 1")[0].arg, "x + 1");
    assert!(match_commands(":bogus").is_empty());
    assert_eq!(match_commands(":").len(), 7);
    assert_eq!(
        format_ambiguous_commands(&match_commands(":")),
        "Which of the following did you mean?  `:clear`  `:examples`  `:force`  `:help`  `:load`  `:quit`  `:reset`"
    );
    assert_eq!(format_unknown_command(":bogus"), "Unknown command: `bogus`");

    assert_eq!(
        highlight_backticks(WELCOME),
        "Welcome to Pkl 0.32.0.\nType an expression to have it evaluated.\nType \x1b[1m:help\x1b[22m or \x1b[1m:examples\x1b[22m for more information."
    );
    assert!(HELP.contains("`:reset`"));
    let ex = highlight_backticks(&examples(true));
    assert!(ex.contains("Objects:\n\x1b[1m\x1b[22m\x1b[1mpigeon {"));
    assert!(ex.contains("Command+Double-click"));
    assert!(ex.contains("https://pkl-lang.org/main/0.32.0/language-reference/"));
}

#[test]
fn repl_redefining_locals_after_cache_hash_rebuilds_index_without_panic() {
    let mut host = MockHost::new();
    let mut session = ReplSession::new(ReplSessionOptions::default());

    for i in 1..=5 {
        let res = host.run(
            &mut session,
            ReplRequest::Eval {
                id: format!("def_{i}"),
                text: format!("local x{i} = new Dynamic {{ v = {i} }}.v"),
                eval_definitions: false,
                force_results: false,
            },
        );
        assert!(res.is_empty(), "unexpected def_{i} response: {res:?}");
    }
    assert_eq!(
        eval_single_ok(&mut host, &mut session, "sum5", "x1 + x2 + x3 + x4 + x5"),
        "15"
    );

    // Redefine x1 (removing its cached slot after cache_index has hashed > 4 slots) and force.
    let redef = host.run(
        &mut session,
        ReplRequest::Eval {
            id: "redef_x1".into(),
            text: "local x1 = new Dynamic { v = 99 }.v".into(),
            eval_definitions: false,
            force_results: false,
        },
    );
    assert!(redef.is_empty(), "unexpected redef response: {redef:?}");
    assert_eq!(
        eval_single_ok(&mut host, &mut session, "get_x1", "x1"),
        "99"
    );

    // Define and force enough additional locals to trigger cache_index rehashing.
    for i in 6..=49 {
        let res = host.run(
            &mut session,
            ReplRequest::Eval {
                id: format!("def_{i}"),
                text: format!("local x{i} = new Dynamic {{ v = {i} }}.v"),
                eval_definitions: false,
                force_results: false,
            },
        );
        assert!(res.is_empty(), "unexpected def_{i} response: {res:?}");
        assert_eq!(
            eval_single_ok(
                &mut host,
                &mut session,
                &format!("eval_{i}"),
                &format!("x{i}")
            ),
            i.to_string()
        );
    }
    assert_eq!(
        eval_single_ok(&mut host, &mut session, "final_check", "x1 + x49"),
        "148"
    );
}
