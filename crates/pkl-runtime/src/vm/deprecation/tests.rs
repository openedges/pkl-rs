//! Source-driven Host scheduler assertions complement the separate Java observations.
use super::*;
use crate::host::{HostPoll, HostSession};
use crate::logging::{EvaluationContext, LogDelivery};
use crate::{parser::parse_module, resolver::resolve_module};

fn prepared(source: &str, expressions: &[&str]) -> (HostSession, usize, Vec<usize>) {
    let syntax = parse_module(source).unwrap();
    let mut program = compile_module(&resolve_module(&syntax).unwrap()).unwrap();
    Arc::make_mut(&mut program).module_name = "deprecation".into();
    let entries = expressions
        .iter()
        .map(|expression| {
            compile_session_expression(
                Arc::make_mut(&mut program),
                &syntax,
                expression,
                CompileOptions::default(),
                false,
            )
            .unwrap()
        })
        .collect();
    let entry = program.entry;
    let host = HostSession::with_context(
        program,
        EvaluationContext {
            primary_source: Some(Arc::new(crate::value::RuntimeSource {
                text: source.into(),
                uri: "file:///deprecation.pkl".into(),
                module_name: "deprecation".into(),
            })),
            log_delivery: LogDelivery::Host,
            ..EvaluationContext::default()
        },
    );
    (host, entry, entries)
}
fn next(host: &mut HostSession, roots: &[Value]) -> HostPoll {
    for _ in 0..200_000 {
        let result = host.poll(0, 1);
        host.collect(roots);
        if result != HostPoll::Yielded {
            return result;
        }
    }
    panic!("scheduler did not settle")
}
fn ready(host: &mut HostSession, roots: &[Value]) -> Result<Value, RuntimeError> {
    match next(host, roots) {
        HostPoll::Ready(value) => value,
        event => panic!("unexpected {event:?}"),
    }
}
fn setup(source: &str, expressions: &[&str]) -> (HostSession, Value, Vec<usize>) {
    let (mut host, entry, entries) = prepared(source, expressions);
    host.start(entry, vec![], 0, None).unwrap();
    let module = ready(&mut host, &[]).unwrap();
    (host, module, entries)
}
fn log(host: &mut HostSession, roots: &[Value]) -> (String, Arc<LogEvent>) {
    match next(host, roots) {
        HostPoll::Log { token, event } => (token, event),
        other => panic!("unexpected {other:?}"),
    }
}
fn start(host: &mut HostSession, entry: usize, module: &Value) {
    host.start(entry, vec![module.clone()], 0, None).unwrap();
}

#[test]
fn target_binding_survives_body_failure_but_not_logger_failure() {
    let source = "module deprecation\n@Deprecated { message=\"use g\" }\nfunction f(x:Int)=throw(\"body\")\nfunction invoke(x)=f(x)\n";
    let (mut host, module, entries) = setup(source, &["invoke(1)", "invoke(2)"]);
    let roots = [module.clone()];
    start(&mut host, entries[0], &module);
    let (token, event) = log(&mut host, &roots);
    assert_eq!(event.level, LogLevel::Warn);
    assert_eq!(event.message, "Method `deprecation#f` is deprecated: use g");
    assert_eq!(event.frame.member_name, None);
    assert_eq!(
        event.frame.source_lines.as_ref(),
        [PklString::from("function invoke(x)=f(x)")]
    );
    assert_eq!(
        (
            event.frame.start_line,
            event.frame.start_column,
            event.frame.end_line,
            event.frame.end_column
        ),
        (4, 20, 4, 23)
    );
    host.acknowledge_log(&token, Err(LoggerFailure::new("logger")), 0)
        .unwrap();
    assert_eq!(ready(&mut host, &roots).unwrap_err().message, "logger");
    start(&mut host, entries[1], &module);
    let (retry, _) = log(&mut host, &roots);
    host.acknowledge_log(&retry, Ok(()), 0).unwrap();
    assert_eq!(ready(&mut host, &roots).unwrap_err().message, "body");
    start(&mut host, entries[0], &module);
    assert_eq!(ready(&mut host, &roots).unwrap_err().message, "body");
    assert!(host.acknowledge_log(&token, Ok(()), 0).is_err());
    host.close();
    assert_eq!(host.heap_stats().slots, 0);
    assert_eq!(event.message, "Method `deprecation#f` is deprecated: use g");
}

#[test]
fn lexical_and_virtual_target_installation_straddle_argument_evaluation() {
    for open in [false, true] {
        let source = format!(
            "{}module deprecation\n@Deprecated\nfunction f(x)=x\nfunction invoke()=f(throw(\"argument\"))\n",
            if open { "open " } else { "" }
        );
        let (mut host, module, entries) = setup(&source, &["invoke()"]);
        let roots = [module.clone()];
        start(&mut host, entries[0], &module);
        if open {
            let (token, event) = log(&mut host, &roots);
            assert_eq!(event.level, LogLevel::Warn);
            host.acknowledge_log(&token, Ok(()), 0).unwrap();
        }
        assert_eq!(ready(&mut host, &roots).unwrap_err().message, "argument");
        start(&mut host, entries[0], &module);
        assert_eq!(ready(&mut host, &roots).unwrap_err().message, "argument");
        host.close();
    }
}

#[test]
fn nested_publication_survives_outer_logger_rejection_and_collection() {
    let source = "module deprecation\n@Deprecated\nfunction old()=throw(\"body\")\nfunction invoke()=old()\n";
    let (mut host, module, entries) = setup(source, &["invoke()"]);
    let roots = [module.clone()];
    start(&mut host, entries[0], &module);
    let (outer, snapshot) = log(&mut host, &roots);
    assert_eq!(
        next(&mut host, &roots),
        HostPoll::Log {
            token: outer.clone(),
            event: snapshot.clone()
        }
    );
    host.reenter(&outer, entries[0], vec![module.clone()], 0, None)
        .unwrap();
    let (inner, event) = log(&mut host, &roots);
    assert_ne!(outer, inner);
    assert_eq!(event, snapshot);
    assert!(host.acknowledge_log(&outer, Ok(()), 0).is_err());
    host.acknowledge_log(&inner, Ok(()), 0).unwrap();
    assert_eq!(ready(&mut host, &roots).unwrap_err().message, "body");
    host.acknowledge_log(&outer, Err(LoggerFailure::new("outer")), 0)
        .unwrap();
    assert_eq!(ready(&mut host, &roots).unwrap_err().message, "outer");
    start(&mut host, entries[0], &module);
    assert_eq!(ready(&mut host, &roots).unwrap_err().message, "body");
    host.close();
    assert_eq!(host.heap_stats().slots, 0);
}

#[test]
fn virtual_cache_uses_receiver_classes_and_drops_direct_targets_after_saturation() {
    let source = "module deprecation\nopen class A { @Deprecated function old()=1 }\nclass B extends A {}\nclass C extends A {}\nclass D extends A {}\nfunction invoke(x)=x.old()\n";
    let (mut host, module, entries) = setup(
        source,
        &[
            "invoke(new A {})",
            "invoke(new B {})",
            "invoke(new C {})",
            "invoke(new D {})",
        ],
    );
    let roots = [module.clone()];
    for (index, warnings) in [
        (0, true),
        (0, false),
        (1, true),
        (2, true),
        (3, false),
        (0, false),
        (1, false),
    ] {
        start(&mut host, entries[index], &module);
        if warnings {
            let (token, event) = log(&mut host, &roots);
            assert_eq!(event.message, "Method `deprecation#A.old` is deprecated");
            host.acknowledge_log(&token, Ok(()), 0).unwrap();
        }
        assert_eq!(ready(&mut host, &roots), Ok(Value::string("1")));
    }
    host.close();
    assert_eq!(host.heap_stats().slots, 0);
}

#[test]
fn pending_warning_cancellation_retry_and_terminal_release() {
    let source = "module deprecation\n@Deprecated\nfunction old()=1\nfunction invoke()=old()\n";
    let (mut host, module, entries) = setup(source, &["invoke()"]);
    let roots = [module.clone()];
    start(&mut host, entries[0], &module);
    let (token, _) = log(&mut host, &roots);
    // VM cancellation is nonterminal; the public Host cancel below is terminal.
    host.vm_for_test_mut().cancel();
    assert_eq!(
        ready(&mut host, &roots).unwrap_err().kind,
        ErrorKind::Cancelled
    );
    assert!(host.acknowledge_log(&token, Ok(()), 0).is_err());
    start(&mut host, entries[0], &module);
    let (retry, _) = log(&mut host, &roots);
    host.acknowledge_log(&retry, Ok(()), 0).unwrap();
    assert_eq!(ready(&mut host, &roots), Ok(Value::string("1")));
    host.close();
    for terminal in 0..3 {
        let (mut host, module, entries) = setup(source, &["invoke()"]);
        host.start(entries[0], vec![module.clone()], 0, Some(1))
            .unwrap();
        let (token, snapshot) = log(&mut host, &[module]);
        match terminal {
            0 => host.close(),
            1 => host.cancel(),
            _ => {
                host.poll(1, 1);
            }
        }
        assert!(host.acknowledge_log(&token, Ok(()), 1).is_err());
        assert_eq!(host.heap_stats().slots, 0);
        assert_eq!(snapshot.level, LogLevel::Warn);
    }
}

#[test]
fn fresh_public_queries_release_warning_sites_while_retained_method_site_stays_bound() {
    use crate::modules::ModuleSourceSpec;
    let source =
        "module deprecation\n@Deprecated\nfunction old(x)=x\nfunction retained(x)=old(x)\n";
    let mut host = HostSession::with_context(
        Arc::new(Program::new(Vec::new())),
        EvaluationContext {
            log_delivery: LogDelivery::Host,
            ..EvaluationContext::default()
        },
    );
    let mut baseline = None;
    for iteration in 1..=100 {
        let expression = format!("List(old({iteration}), retained({iteration})).toString()");
        let entry = host
            .append_public_operation(
                ModuleSourceSpec {
                    uri: "file:///deprecation.pkl".into(),
                    contents: Some(source.into()),
                },
                expression.into(),
            )
            .unwrap();
        host.start(entry, vec![], 0, None).unwrap();
        let mut logs = 0;
        loop {
            match next(&mut host, &[]) {
                HostPoll::Log { token, event } => {
                    assert_eq!(event.level, LogLevel::Warn);
                    logs += 1;
                    host.acknowledge_log(&token, Ok(()), 0).unwrap();
                }
                HostPoll::Module { token, request } => {
                    use crate::modules::{
                        ModuleCompletion, ModuleKey, ModuleKeyKind, ModuleRequestKind,
                        ResolvedModule,
                    };
                    let completion = match &request.kind {
                        ModuleRequestKind::SelectKey { id, uri, .. }
                            if uri.starts_with("file:") =>
                        {
                            ModuleCompletion::KeySelected(Ok(ModuleKey {
                                id: *id,
                                uri: uri.clone(),
                                kind: ModuleKeyKind::Host,
                                handle: "deprecation-test".into(),
                            }))
                        }
                        ModuleRequestKind::QueryCachePolicy { .. } => {
                            ModuleCompletion::CachePolicy(Ok(true))
                        }
                        ModuleRequestKind::Resolve { key, .. } => {
                            ModuleCompletion::Resolved(Ok(ResolvedModule {
                                original_uri: key.uri.clone(),
                                resolved_uri: key.uri.clone(),
                                source_uri: key.uri.clone(),
                            }))
                        }
                        _ => crate::modules::default_completion(&request)
                            .unwrap_or_else(|| panic!("unexpected {request:?}")),
                    };
                    host.complete_module(&token, completion, 0).unwrap();
                }
                HostPoll::Ready(value) => {
                    assert!(value.is_ok(), "{value:?}");
                    break;
                }
                event => panic!("unexpected {event:?}"),
            }
        }
        assert_eq!(logs, if iteration == 1 { 2 } else { 1 });
        host.collect(&[]);
        let vm = host.vm_for_test();
        assert_eq!(vm.code_arenas.iter().filter(|arena| arena.query).count(), 0);
        let current = (
            (
                vm.program.blocks.len(),
                vm.program.blocks.capacity(),
                vm.program.blocks.resident_segments(),
            ),
            (
                vm.program.metadata.len(),
                vm.program.metadata.capacity(),
                vm.program.metadata.resident_segments(),
            ),
            (
                vm.program.units.len(),
                vm.program.units.capacity(),
                vm.program.units.resident_segments(),
            ),
            vm.program.blocks.iter().map(Vec::capacity).sum::<usize>(),
            vm.program
                .metadata
                .iter()
                .map(|metadata| metadata.spans.capacity())
                .sum::<usize>(),
            vm.code_arenas.len(),
            vm.executable_roots.len(),
            (
                vm.heap_stats().live,
                vm.heap_stats().executable_live,
                vm.heap_stats().slots,
            ),
        );
        if iteration == 10 {
            eprintln!("deprecation queries N=10 {current:?}");
            baseline = Some(current);
        }
        if iteration > 10 {
            assert_eq!(Some(current), baseline, "query {iteration}");
        }
        if iteration == 100 {
            eprintln!("deprecation queries N=100 {current:?}");
        }
    }
    host.close();
    assert_eq!(host.heap_stats().slots, 0);
    assert!(host.vm_for_test().code_arenas.is_empty());
    assert!(host.vm_for_test().program.units.iter().all(|unit| {
        unit.source
            .as_ref()
            .is_none_or(|source| source.uri.as_ref() != "file:///deprecation.pkl")
    }));
    assert!(host.vm_for_test().executable_roots.is_empty());
}

fn public_run(
    host: &mut HostSession,
    source: &str,
    expression: &str,
    child: &str,
) -> (Result<Value, RuntimeError>, usize) {
    use crate::modules::{
        ModuleCompletion, ModuleKey, ModuleKeyKind, ModuleRequestKind, ModuleSource,
        ModuleSourceSpec, ResolvedModule,
    };
    let entry = host
        .append_public_operation(
            ModuleSourceSpec {
                uri: "file:///deprecation.pkl".into(),
                contents: Some(source.into()),
            },
            expression.into(),
        )
        .unwrap();
    host.start(entry, vec![], 0, None).unwrap();
    let mut warnings = 0;
    loop {
        match next(host, &[]) {
            HostPoll::Log { token, event } => {
                assert_eq!(event.level, LogLevel::Warn);
                warnings += 1;
                host.acknowledge_log(&token, Ok(()), 0).unwrap();
            }
            HostPoll::Module { token, request } => {
                let completion = match &request.kind {
                    ModuleRequestKind::SelectKey { id, uri, .. } if uri.starts_with("file:") => {
                        ModuleCompletion::KeySelected(Ok(ModuleKey {
                            id: *id,
                            uri: uri.clone(),
                            kind: ModuleKeyKind::Host,
                            handle: "deprecation-test".into(),
                        }))
                    }
                    ModuleRequestKind::QueryCachePolicy { key } => {
                        ModuleCompletion::CachePolicy(Ok(!key.uri.contains("child")))
                    }
                    ModuleRequestKind::Resolve { key, .. } => {
                        ModuleCompletion::Resolved(Ok(ResolvedModule {
                            original_uri: key.uri.clone(),
                            resolved_uri: key.uri.clone(),
                            source_uri: key.uri.clone(),
                        }))
                    }
                    ModuleRequestKind::LoadSource { key, .. } if key.key.uri.contains("child") => {
                        ModuleCompletion::Source(Ok(ModuleSource {
                            text: child.into(),
                            inferred_name: "child".into(),
                        }))
                    }
                    _ => crate::modules::default_completion(&request)
                        .unwrap_or_else(|| panic!("unexpected {request:?}")),
                };
                host.complete_module(&token, completion, 0).unwrap();
            }
            HostPoll::Ready(result) => return (result, warnings),
            event => panic!("unexpected {event:?}"),
        }
    }
}
fn child_units(host: &HostSession) -> usize {
    host.vm_for_test()
        .program
        .units
        .iter()
        .filter(|unit| {
            unit.source
                .as_ref()
                .is_some_and(|source| source.uri.contains("child"))
        })
        .count()
}

#[test]
fn lexical_target_keeps_declaration_without_retaining_first_receiver_subclass() {
    let source = "module deprecation\nopen class A {\n @Deprecated local function old()=1\n function call()=old()\n}\n";
    let child = "import \"file:///deprecation.pkl\" as root\nclass B extends root.A {}\nvalue=new B {}.call()\n";
    let mut host = HostSession::with_context(
        Arc::new(Program::new(Vec::new())),
        EvaluationContext {
            log_delivery: LogDelivery::Host,
            ..EvaluationContext::default()
        },
    );
    for iteration in 0..10 {
        let (result, warnings) = public_run(
            &mut host,
            source,
            "import(\"file:///child.pkl\").value",
            child,
        );
        assert_eq!(result, Ok(Value::string("1")));
        assert_eq!(warnings, usize::from(iteration == 0));
        host.collect(&[]);
        assert_eq!(
            child_units(&host),
            1,
            "old subclass retained after replacement {iteration}"
        );
    }
    host.close();
    assert_eq!(host.heap_stats().slots, 0);
}

#[test]
fn virtual_cache_keeps_class_code_then_releases_it_before_fourth_lookup_failure() {
    let source = "module deprecation\nopen class A { @Deprecated function old()=1 }\nfunction invoke(x)=x.old()\n";
    let child =
        "import \"file:///deprecation.pkl\" as root\nclass B extends root.A {}\nvalue=new B {}\n";
    let mut host = HostSession::with_context(
        Arc::new(Program::new(Vec::new())),
        EvaluationContext {
            log_delivery: LogDelivery::Host,
            ..EvaluationContext::default()
        },
    );
    for iteration in 0..4 {
        let (result, warnings) = public_run(
            &mut host,
            source,
            "invoke(import(\"file:///child.pkl\").value)",
            if iteration == 3 {
                "class B {}\nvalue=new B {}\n"
            } else {
                child
            },
        );
        if iteration == 3 {
            assert_eq!(result.unwrap_err().kind, ErrorKind::MissingMember);
        } else {
            assert_eq!(result, Ok(Value::string("1")));
        }
        assert_eq!(warnings, usize::from(iteration < 3));
        host.collect(&[]);
        assert_eq!(
            child_units(&host),
            if iteration == 3 { 1 } else { iteration + 1 }
        );
    }
    host.close();
    assert_eq!(host.heap_stats().slots, 0);
}

#[test]
fn virtual_logger_reentry_retries_publication_and_can_restore_a_retired_chain() {
    let source = "module deprecation\nopen class A { @Deprecated function old()=1 }\nclass B extends A {}\nclass C extends A {}\nclass D extends A {}\nclass E extends A {}\nclass F extends A {}\nfunction invoke(x)=x.old()\n";
    let expressions = [
        "invoke(new A {})",
        "invoke(new B {})",
        "List(invoke(new B {}),invoke(new C {}),invoke(new D {}),invoke(new E {}))",
        "List(invoke(new C {}),invoke(new D {}),invoke(new E {}))",
        "invoke(new F {})",
    ];
    for mode in 0..3 {
        let (mut host, module, entries) = setup(source, &expressions);
        let roots = [module.clone()];
        let mut warnings = 0;
        if mode == 2 {
            start(&mut host, entries[0], &module);
            let (token, _) = log(&mut host, &roots);
            warnings += 1;
            host.acknowledge_log(&token, Ok(()), 0).unwrap();
            ready(&mut host, &roots).unwrap();
        }
        start(&mut host, entries[usize::from(mode == 2)], &module);
        let (outer, _) = log(&mut host, &roots);
        warnings += 1;
        let child = match mode {
            0 => 1,
            1 => 2,
            _ => 3,
        };
        host.reenter(&outer, entries[child], vec![module.clone()], 0, None)
            .unwrap();
        loop {
            match next(&mut host, &roots) {
                HostPoll::Log { token, .. } => {
                    warnings += 1;
                    host.acknowledge_log(&token, Ok(()), 0).unwrap();
                }
                HostPoll::Ready(result) => {
                    result.unwrap();
                    break;
                }
                event => panic!("unexpected {event:?}"),
            }
        }
        host.acknowledge_log(&outer, Ok(()), 0).unwrap();
        loop {
            match next(&mut host, &roots) {
                HostPoll::Log { token, .. } => {
                    warnings += 1;
                    host.acknowledge_log(&token, Ok(()), 0).unwrap();
                }
                HostPoll::Ready(result) => {
                    result.unwrap();
                    break;
                }
                event => panic!("unexpected {event:?}"),
            }
        }
        let tail: &[usize] = if mode == 0 { &[0, 1] } else { &[4, 0, 1] };
        for &index in tail {
            start(&mut host, entries[index], &module);
            loop {
                match next(&mut host, &roots) {
                    HostPoll::Log { token, .. } => {
                        warnings += 1;
                        host.acknowledge_log(&token, Ok(()), 0).unwrap();
                    }
                    HostPoll::Ready(result) => {
                        result.unwrap();
                        break;
                    }
                    event => panic!("unexpected {event:?}"),
                }
            }
        }
        assert_eq!(warnings, [3, 6, 7][mode]);
        host.close();
        assert_eq!(host.heap_stats().slots, 0);
    }
}

#[test]
fn implicit_typed_string_calls_keep_authored_sites_and_skip_primitive_classes() {
    let source = "module deprecation\nclass A { @Deprecated function toString()=\"A\" }\nfunction format(x)=\"\\(x)\"\n";
    let (mut host, module, entries) = setup(
        source,
        &[
            "format(1)",
            "format(true)",
            "format(\"s\")",
            "format(new A {})",
            "new A {}",
        ],
    );
    let roots = [module.clone()];
    for (iteration, &index) in [0, 1, 2, 3, 3, 4].iter().enumerate() {
        start(&mut host, entries[index], &module);
        if index == 3 && !host.vm_for_test().log_buffer.is_empty() {
            panic!("new operation must clear log buffer");
        }
        let mut warnings = 0;
        loop {
            match next(&mut host, &roots) {
                HostPoll::Log { token, event } => {
                    warnings += 1;
                    if index == 4 {
                        assert_eq!(event.frame.module_uri, "source:unavailable");
                        assert_eq!(event.frame.source_lines.as_ref(), [PklString::default()]);
                        assert_eq!(
                            (
                                event.frame.start_line,
                                event.frame.start_column,
                                event.frame.end_line,
                                event.frame.end_column
                            ),
                            (1, 1, 1, 1)
                        );
                    } else {
                        assert_eq!(event.frame.module_uri, "file:///deprecation.pkl");
                        assert_eq!(
                            event.frame.source_lines.as_ref(),
                            [PklString::from("function format(x)=\"\\(x)\"")]
                        );
                        assert_eq!(
                            (
                                event.frame.start_line,
                                event.frame.start_column,
                                event.frame.end_line,
                                event.frame.end_column
                            ),
                            (3, 20, 3, 25)
                        );
                    }
                    host.acknowledge_log(&token, Ok(()), 0).unwrap();
                }
                HostPoll::Ready(result) => {
                    result.unwrap();
                    break;
                }
                event => panic!("unexpected {event:?}"),
            }
        }
        assert_eq!(warnings, usize::from(matches!(iteration, 3 | 5)));
    }
    host.close();
}

#[test]
fn noop_warning_still_buffers_and_custom_transformer_waits_before_binding() {
    let source = "module deprecation\n@Deprecated\nfunction old()=1\nfunction invoke()=old()\n";
    for transformer in [false, true] {
        let (mut host, module, entries) = setup(source, &["invoke()"]);
        host.vm_for_test_mut().context.log_delivery = LogDelivery::Noop;
        host.vm_for_test_mut().context.frame_transformer = if transformer {
            crate::logging::FrameTransformer::Host
        } else {
            crate::logging::FrameTransformer::Default
        };
        let roots = [module.clone()];
        start(&mut host, entries[0], &module);
        if transformer {
            let HostPoll::TransformFrame { token, frame } = next(&mut host, &roots) else {
                panic!("expected noop logger transformation");
            };
            host.complete_frame_transform(&token, Ok((*frame).clone()), 0)
                .unwrap();
        }
        assert_eq!(ready(&mut host, &roots), Ok(Value::string("1")));
        assert_eq!(
            host.vm_for_test().log_buffer,
            "Method `deprecation#old` is deprecated\n"
                .encode_utf16()
                .collect::<Vec<_>>()
        );
        start(&mut host, entries[0], &module);
        assert_eq!(ready(&mut host, &roots), Ok(Value::string("1")));
        assert!(host.vm_for_test().log_buffer.is_empty());
        host.close();
        assert_eq!(host.heap_stats().slots, 0);
    }
}
