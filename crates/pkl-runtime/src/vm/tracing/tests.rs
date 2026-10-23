//! Source-language scheduler and ownership checks; Java captures establish compatibility.
use super::*;
use crate::host::{HostPoll, HostSession};
use crate::logging::EvaluationContext;
use crate::{parser::parse_module, resolver::resolve_module};

const URI: &str = "file:///stage37-trace.pkl";
fn setup(source: &str, names: &[&str]) -> (HostSession, Value, Vec<usize>) {
    let (mut host, entry, entries) = prepared(source, names);
    host.start(entry, vec![], 0, None).unwrap();
    let module = ready(&mut host, &[]).unwrap();
    (host, module, entries)
}

fn prepared(source: &str, names: &[&str]) -> (HostSession, usize, Vec<usize>) {
    let mut program =
        compile_module(&resolve_module(&parse_module(source).unwrap()).unwrap()).unwrap();
    let compiled = Arc::make_mut(&mut program);
    compiled.module_name = "stage37-trace".into();
    let entries = names
        .iter()
        .map(|name| {
            let entry = compiled.blocks.len();
            compiled.blocks.push(vec![
                Instruction::Input(0),
                Instruction::GetMember((*name).into()),
                Instruction::Return,
            ]);
            let metadata = BlockMetadata {
                spans: vec![None, Some(Span::new(0, name.len())), None],
                source: Some(Arc::new(RuntimeSource {
                    text: (*name).into(),
                    uri: "repl:text".into(),
                    module_name: "".into(),
                    line_anchors: false,
                })),
                ..BlockMetadata::default()
            };
            compiled.metadata.push(metadata);
            entry
        })
        .collect();
    let entry = program.entry;
    let host = HostSession::with_context(
        program,
        EvaluationContext {
            primary_source: Some(Arc::new(RuntimeSource {
                text: source.into(),
                uri: URI.into(),
                module_name: "stage37-trace".into(),
                line_anchors: false,
            })),
            log_delivery: LogDelivery::Host,
            ..EvaluationContext::default()
        },
    );
    (host, entry, entries)
}
fn next_event(host: &mut HostSession, roots: &[Value]) -> HostPoll {
    for _ in 0..300_000 {
        let event = host.poll(0, 1);
        host.collect(roots);
        if event != HostPoll::Yielded {
            return event;
        }
    }
    panic!("trace scheduler did not settle")
}
fn ready(host: &mut HostSession, roots: &[Value]) -> Result<Value, RuntimeError> {
    match next_event(host, roots) {
        HostPoll::Ready(result) => result,
        event => panic!("unexpected {event:?}"),
    }
}
fn log(host: &mut HostSession, roots: &[Value]) -> (String, Arc<LogEvent>) {
    match next_event(host, roots) {
        HostPoll::Log { token, event } => (token, event),
        event => panic!("unexpected {event:?}"),
    }
}
fn read(host: &mut HostSession, roots: &[Value]) -> String {
    match next_event(host, roots) {
        HostPoll::Read { token, .. } => token,
        event => panic!("unexpected {event:?}"),
    }
}
fn start(host: &mut HostSession, entry: usize, module: &Value) {
    host.start(entry, vec![module.clone()], 0, None).unwrap();
}

#[test]
fn pending_logger_keeps_value_roots_and_callback_errors_do_not_finish_parent() {
    let source = "obj=new Dynamic { x=read(\"life:value\"); y=2 }\nresult=trace(obj)==obj\ncallback=throw(\"callback\")\nsecond=trace(obj)==obj\n";
    let (mut host, module, entries) = setup(source, &["result", "callback", "second"]);
    let roots = [module.clone()];
    start(&mut host, entries[0], &module);
    let read_token = read(&mut host, &roots);
    assert!(host.acknowledge_log(&read_token, Ok(()), 0).is_err());
    host.resume(&read_token, Ok(Value::string("resource")), 0)
        .unwrap();
    let (token, snapshot) = log(&mut host, &roots);
    assert_eq!(
        snapshot.message,
        "obj = new Dynamic { x = \"resource\"; y = 2 }"
    );
    assert_eq!(
        next_event(&mut host, &roots),
        HostPoll::Log {
            token: token.clone(),
            event: snapshot.clone()
        }
    );
    assert!(host.resume(&token, Ok(Value::Int(99.into())), 0).is_err());
    host.reenter(&token, entries[1], vec![module.clone()], 0, None)
        .unwrap();
    let error = ready(&mut host, &roots).unwrap_err();
    assert_eq!(error.message, "callback");
    assert_eq!(error.frames.len(), 3);
    assert_eq!(
        error.frames[1].source.as_ref().unwrap().text.as_ref(),
        "callback"
    );
    assert_eq!(
        error.frames[2].source.as_ref().unwrap().text.as_ref(),
        "result"
    );
    assert_eq!(host.context_depth(), 1);
    host.acknowledge_log(&token, Ok(()), 0).unwrap();
    assert_eq!(ready(&mut host, &roots), Ok(Value::Bool(true)));
    assert!(host.acknowledge_log(&token, Ok(()), 0).is_err());
    start(&mut host, entries[0], &module);
    assert_eq!(ready(&mut host, &roots), Ok(Value::Bool(true)));
    start(&mut host, entries[2], &module);
    let (second, _) = log(&mut host, &roots);
    assert_ne!(second, token);
    host.acknowledge_log(&second, Ok(()), 0).unwrap();
    assert_eq!(ready(&mut host, &roots), Ok(Value::Bool(true)));
    host.close();
    assert_eq!(host.heap_stats().slots, 0);
    assert_eq!(snapshot.frame.module_uri, URI);
}

#[test]
fn pending_same_member_and_ancestor_publish_before_older_log_attempt_finishes() {
    // Java pending-review and publication-review: inner success remains cached
    // even if the older outer logger subsequently fails.
    for (source, value, expected) in [
        (
            "obj=new Dynamic { x=1 }\nresult=trace(obj).x\n",
            1,
            vec!["obj = new Dynamic { x = 1 }", "obj = new Dynamic { x = 1 }"],
        ),
        (
            "obj=new Dynamic { x=1 }\ninner=trace(obj).x\nresult=inner+3\n",
            4,
            vec!["obj = new Dynamic { x = 1 }", "obj = new Dynamic { x = 1 }"],
        ),
        (
            "obj=new Dynamic { x=trace(1) }\nresult=trace(obj).x\n",
            1,
            vec![
                "1 = 1",
                "obj = new Dynamic { x = ? }",
                "1 = 1",
                "obj = new Dynamic { x = 1 }",
            ],
        ),
    ] {
        for fail_outer in [false, true] {
            let (mut host, module, entries) = setup(source, &["result"]);
            let roots = [module.clone()];
            let mut messages = Vec::new();
            start(&mut host, entries[0], &module);
            let result = loop {
                match next_event(&mut host, &roots) {
                    HostPoll::Log { token, event } => {
                        messages.push(event.message.to_java_utf8());
                        host.reenter(&token, entries[0], vec![module.clone()], 0, None)
                            .unwrap();
                        loop {
                            match next_event(&mut host, &roots) {
                                HostPoll::Log { token, event } => {
                                    messages.push(event.message.to_java_utf8());
                                    host.acknowledge_log(&token, Ok(()), 0).unwrap();
                                }
                                HostPoll::Ready(result) => {
                                    assert_eq!(result, Ok(Value::Int(value.into())));
                                    break;
                                }
                                other => panic!("unexpected {other:?}"),
                            }
                        }
                        let completion = if fail_outer {
                            Err(LoggerFailure::new("outer logger failed"))
                        } else {
                            Ok(())
                        };
                        host.acknowledge_log(&token, completion, 0).unwrap();
                    }
                    HostPoll::Ready(result) => break result,
                    other => panic!("unexpected {other:?}"),
                }
            };
            if fail_outer {
                assert_eq!(result.unwrap_err().kind, ErrorKind::Host);
            } else {
                assert_eq!(result, Ok(Value::Int(value.into())));
            }
            let expected = if fail_outer && expected.len() == 4 {
                &expected[..3]
            } else {
                &expected[..]
            };
            assert_eq!(messages, expected);
            start(&mut host, entries[0], &module);
            assert_eq!(ready(&mut host, &roots), Ok(Value::Int(value.into())));
            host.close();
            assert_eq!(host.heap_stats().slots, 0);
        }
    }
}

#[test]
fn failed_callback_attempt_preserves_parent_evaluating_state_and_current_cycles() {
    let (mut host, module, entries) = setup("result=trace(1)\ncycle=cycle\n", &["result", "cycle"]);
    let roots = [module.clone()];
    start(&mut host, entries[0], &module);
    let (outer, _) = log(&mut host, &roots);
    host.reenter(&outer, entries[0], vec![module.clone()], 0, None)
        .unwrap();
    let (inner, _) = log(&mut host, &roots);
    host.acknowledge_log(&inner, Err(LoggerFailure::new("inner logger failed")), 0)
        .unwrap();
    assert_eq!(ready(&mut host, &roots).unwrap_err().kind, ErrorKind::Host);
    assert!(matches!(
        host.vm_for_test()
            .member_state(module.as_object().unwrap(), "result")
            .unwrap(),
        LazyState::Evaluating
    ));
    host.reenter(&outer, entries[1], vec![module.clone()], 0, None)
        .unwrap();
    assert_eq!(ready(&mut host, &roots).unwrap_err().kind, ErrorKind::Cycle);
    host.acknowledge_log(&outer, Ok(()), 0).unwrap();
    assert_eq!(ready(&mut host, &roots), Ok(Value::Int(1.into())));
}

#[test]
fn mixed_log_read_log_cancellation_aborts_only_the_correct_read_context() {
    let source = "first=trace(1)\ncallback=trace(read(\"life:nested\"))\nreaderChild=trace(7)\n";
    let (mut host, module, entries) = setup(source, &["first", "callback", "readerChild"]);
    let roots = [module.clone()];
    start(&mut host, entries[0], &module);
    let (outer, outer_snapshot) = log(&mut host, &roots);
    host.reenter(&outer, entries[1], vec![module.clone()], 0, None)
        .unwrap();
    let read_token = read(&mut host, &roots);
    host.reenter(&read_token, entries[2], vec![module.clone()], 0, None)
        .unwrap();
    let (inner, _) = log(&mut host, &roots);
    assert_eq!(host.context_depth(), 3);
    assert!(host.acknowledge_log(&outer, Ok(()), 0).is_err());
    assert!(
        host.resume(&read_token, Ok(Value::Int(1.into())), 0)
            .is_err()
    );
    host.cancel();
    assert_eq!(
        host.drain_cancelled_reads(),
        std::slice::from_ref(&read_token)
    );
    assert!(host.drain_cancelled_reads().is_empty());
    assert_eq!(host.context_depth(), 0);
    assert_eq!(host.heap_stats().slots, 0);
    assert!(host.acknowledge_log(&inner, Ok(()), 0).is_err());
    assert!(host.acknowledge_log(&outer, Ok(()), 0).is_err());
    assert!(
        host.resume(&read_token, Ok(Value::Int(1.into())), 0)
            .is_err()
    );
    assert_eq!(outer_snapshot.message, "1 = 1");
}

#[test]
fn callback_close_finishes_already_issued_ancestor_reads_and_logs() {
    for outer_read in [false, true] {
        let source = if outer_read {
            "first=read(\"life:outer\")+\"tail\"\ninner=trace(7)+2\n"
        } else {
            "first=trace(1)+2\ninner=trace(7)+2\n"
        };
        let (mut host, module, entries) = setup(source, &["first", "inner"]);
        let roots = [module.clone()];
        start(&mut host, entries[0], &module);
        let outer = if outer_read {
            read(&mut host, &roots)
        } else {
            log(&mut host, &roots).0
        };
        host.reenter(&outer, entries[1], vec![module.clone()], 0, None)
            .unwrap();
        let (inner, snapshot) = log(&mut host, &roots);
        host.close_from_callback(&inner).unwrap();
        assert!(
            host.reenter(&inner, entries[0], vec![module.clone()], 0, None)
                .is_err()
        );
        assert!(
            host.start(entries[0], vec![module.clone()], 0, None)
                .is_err()
        );
        host.acknowledge_log(&inner, Ok(()), 0).unwrap();
        assert_eq!(ready(&mut host, &roots), Ok(Value::Int(9.into())));
        assert_eq!(host.context_depth(), 1);
        if outer_read {
            host.resume(&outer, Ok(Value::string("R")), 0).unwrap();
        } else {
            host.acknowledge_log(&outer, Ok(()), 0).unwrap();
        }
        assert_eq!(
            ready(&mut host, &[]),
            Ok(if outer_read {
                Value::string("Rtail")
            } else {
                Value::Int(3.into())
            })
        );
        assert!(host.is_closed());
        assert_eq!(host.heap_stats().slots, 0);
        assert!(host.drain_cancelled_reads().is_empty());
        assert_eq!(snapshot.message, "7 = 7");
    }
}

#[test]
fn pretty_preserves_authored_crlf_and_compact_truncates_only_rendered_value() {
    let source = "result=trace(new Dynamic {\r\n  x=1\r\n}).x\r\n";
    let (mut host, module, entries) = setup(source, &["result"]);
    host.vm_for_test_mut().context.trace_mode = TraceMode::Pretty;
    start(&mut host, entries[0], &module);
    let (token, event) = log(&mut host, std::slice::from_ref(&module));
    assert_eq!(
        event.message,
        "\n  new Dynamic {\r\n    x=1\r\n  }\n=\n  new Dynamic {\n    x = 1\n  }\n"
    );
    assert_eq!(
        event.frame.source_lines.as_ref(),
        ["result=trace(new Dynamic {", "  x=1", "}).x"]
    );
    host.acknowledge_log(&token, Ok(()), 0).unwrap();
    assert_eq!(ready(&mut host, &[module]), Ok(Value::Int(1.into())));

    let (mut host, module, entries) = setup(
        "value=read(\"life:long\")\nresult=trace(value).length\n",
        &["result"],
    );
    start(&mut host, entries[0], &module);
    let token = read(&mut host, std::slice::from_ref(&module));
    host.resume(&token, Ok(Value::string("a".repeat(1_000_010))), 0)
        .unwrap();
    let (token, event) = log(&mut host, std::slice::from_ref(&module));
    let actual: Vec<_> = event.message.encode_utf16().collect();
    assert_eq!(actual.len(), 1_000_008);
    assert_eq!(&actual[..8], &"value = ".encode_utf16().collect::<Vec<_>>());
    assert_eq!(
        &actual[actual.len() - 3..],
        &"...".encode_utf16().collect::<Vec<_>>()
    );
    host.acknowledge_log(&token, Ok(()), 0).unwrap();
    assert_eq!(
        ready(&mut host, &[module]),
        Ok(Value::Int(1_000_010.into()))
    );
}

#[test]
fn noop_logger_still_delivers_custom_transformer_and_timeout_releases_active_tree() {
    let source = "first=trace(1)\ncallback=read(\"life:pending\")\n";
    let (mut host, module, entries) = setup(source, &["first", "callback"]);
    host.vm_for_test_mut().context.log_delivery = LogDelivery::Noop;
    host.vm_for_test_mut().context.host_frame_transformer = true;
    host.start(entries[0], vec![module.clone()], 0, Some(10))
        .unwrap();
    let (log_token, snapshot) = log(&mut host, std::slice::from_ref(&module));
    host.reenter(&log_token, entries[1], vec![module.clone()], 0, None)
        .unwrap();
    let read_token = read(&mut host, &[module]);
    let HostPoll::Ready(Err(error)) = host.poll(10, 1) else {
        panic!("ancestor timeout must terminate nested reader");
    };
    assert_eq!(error.kind, ErrorKind::Timeout);
    assert_eq!(
        host.drain_cancelled_reads(),
        std::slice::from_ref(&read_token)
    );
    assert_eq!(host.context_depth(), 0);
    assert_eq!(host.heap_stats().slots, 0);
    assert!(host.acknowledge_log(&log_token, Ok(()), 10).is_err());
    assert!(
        host.resume(&read_token, Ok(Value::Int(8.into())), 10)
            .is_err()
    );
    assert_eq!(snapshot.message, "1 = 1");
    assert_eq!(snapshot.frame.source_lines.as_ref(), ["first=trace(1)"]);
}

#[test]
fn reader_callback_publication_survives_older_attempt_failure() {
    // These exact sources are from stage37-read-reentry-oracle-02. Java's
    // ResourceManager later throws CME; the native Host failure below tests
    // cache ownership without reproducing that unrelated implementation bug.
    for (source, expected) in [
        (
            "child=read(\"life:resource\")\nresult=child+\"-tail\"\nstable=\"ok\"\n",
            "resource-tail",
        ),
        (
            "shared=read(\"life:shared\")\npending=read(\"life:outer\")+shared\nresult=pending\nstable=\"ok\"\n",
            "resourceresource",
        ),
        (
            "shared=read(\"life:resource\")\npending=read(\"life:resource\")+shared\nresult=pending\nstable=\"ok\"\n",
            "resourceresource",
        ),
    ] {
        let (mut host, module, entries) = setup(source, &["result", "stable"]);
        let roots = [module.clone()];
        start(&mut host, entries[0], &module);
        let outer = read(&mut host, &roots);
        host.reenter(&outer, entries[0], vec![module.clone()], 0, None)
            .unwrap();
        loop {
            match next_event(&mut host, &roots) {
                HostPoll::Read { token, .. } => host
                    .resume(&token, Ok(Value::string("resource")), 0)
                    .unwrap(),
                HostPoll::Ready(value) => {
                    assert_eq!(value, Ok(Value::string(expected)));
                    break;
                }
                event => panic!("unexpected {event:?}"),
            }
        }
        host.resume(
            &outer,
            Err(RuntimeError::new(ErrorKind::Host, "older reader failed")),
            0,
        )
        .unwrap();
        assert_eq!(ready(&mut host, &roots).unwrap_err().kind, ErrorKind::Host);
        start(&mut host, entries[0], &module);
        assert_eq!(ready(&mut host, &roots), Ok(Value::string(expected)));
        start(&mut host, entries[1], &module);
        assert_eq!(ready(&mut host, &roots), Ok(Value::string("ok")));
        host.close();
        assert_eq!(host.heap_stats().slots, 0);
    }
}

#[test]
fn string_repeat_retains_receiver_across_host_count_and_preserves_utf16_boundaries() {
    let source = "result=trace(\"\\u{DC00}\\u{D800}\".repeat(read(\"life:count\"))).length\n";
    let (mut host, module, entries) = setup(source, &["result"]);
    let roots = [module.clone()];
    start(&mut host, entries[0], &module);
    let count = read(&mut host, &roots);
    host.resume(&count, Ok(Value::Int(2.into())), 0).unwrap();
    let (token, event) = log(&mut host, &roots);
    let units: Vec<_> = event.message.encode_utf16().collect();
    assert_eq!(
        &units[units.len() - 6..],
        &[34, 0xdc00, 0xd800, 0xdc00, 0xd800, 34]
    );
    host.acknowledge_log(&token, Ok(()), 0).unwrap();
    // Repeating pairs the middle high/low units without replacing the unmatched ends.
    assert_eq!(ready(&mut host, &roots), Ok(Value::Int(3.into())));
    start(&mut host, entries[0], &module);
    assert_eq!(ready(&mut host, &roots), Ok(Value::Int(3.into())));
}

#[test]
fn initial_path_continuation_preserves_log_buffer_and_the_published_module() {
    let source =
        "@Deprecated { when(trace(true)) { message=\"setup\" } }\nmodule setup\nresult=trace(8)\n";
    let (mut host, module_entry, entries) = prepared(source, &["result"]);
    host.start_then(module_entry, vec![], entries[0], 0, None)
        .unwrap();
    let (token, _) = log(&mut host, &[]);
    host.acknowledge_log(&token, Ok(()), 0).unwrap();
    let module = loop {
        assert_eq!(host.poll(0, 1), HostPoll::Yielded);
        host.collect(&[]);
        if let Some(result) = host.take_intermediate_result() {
            break result.unwrap();
        }
    };
    assert_eq!(host.context_depth(), 1);
    assert_eq!(
        String::from_utf16(&host.vm_for_test().log_buffer).unwrap(),
        "true = true\n"
    );
    let (token, _) = log(&mut host, std::slice::from_ref(&module));
    assert_eq!(
        String::from_utf16(&host.vm_for_test().log_buffer).unwrap(),
        "true = true\n8 = 8\n"
    );
    host.acknowledge_log(&token, Ok(()), 0).unwrap();
    assert_eq!(
        ready(&mut host, std::slice::from_ref(&module)),
        Ok(Value::Int(8.into()))
    );
    start(&mut host, entries[0], &module);
    assert!(host.vm_for_test().log_buffer.is_empty());
    assert_eq!(ready(&mut host, &[module]), Ok(Value::Int(8.into())));
}

#[test]
fn callback_completion_cannot_consume_parent_continuation_and_close_allows_existing_path() {
    for source in [
        "@Deprecated { when(trace(true)) { message=\"setup\" } }\nmodule setup\nstable=7\nresult=8\n",
        "stable=7\n@Deprecated { when(trace(true)) { message=\"setup\" } }\nresult=8\n",
    ] {
        let (mut host, module_entry, entries) = prepared(source, &["result", "stable"]);
        host.start_then(module_entry, vec![], entries[0], 0, None)
            .unwrap();
        let (token, _) = log(&mut host, &[]);
        let module = host.primary_module().unwrap();
        host.reenter(&token, entries[1], vec![module.clone()], 0, None)
            .unwrap();
        // The module annotation precedes all properties; the member annotation
        // follows stable's publication. Neither callback owns the initial path.
        let callback = ready(&mut host, &[]);
        if source.starts_with("stable") {
            assert_eq!(callback, Ok(Value::Int(7.into())));
        } else {
            assert_eq!(callback.unwrap_err().kind, ErrorKind::MissingMember);
        }
        assert_eq!(host.context_depth(), 1);
        assert!(host.take_intermediate_result().is_none());
        host.close_from_callback(&token).unwrap();
        assert_eq!(
            host.reenter(&token, entries[1], vec![module], 0, None)
                .unwrap_err()
                .kind,
            ErrorKind::Closed
        );
        host.acknowledge_log(&token, Ok(()), 0).unwrap();
        let published = loop {
            assert_eq!(host.poll(0, 1), HostPoll::Yielded);
            host.collect(&[]);
            if let Some(result) = host.take_intermediate_result() {
                break result.unwrap();
            }
        };
        assert_eq!(ready(&mut host, &[published]), Ok(Value::Int(8.into())));
        assert!(host.is_closed());
        assert_eq!(host.context_depth(), 0);
        assert_eq!(host.heap_stats().slots, 0);
        assert_eq!(
            host.start(entries[0], vec![], 0, None).unwrap_err().kind,
            ErrorKind::Closed
        );
    }
}

#[test]
fn continuation_intermediate_roots_survive_path_failure_and_deadline_stays_shared() {
    let source = "@Deprecated { when(trace(true)) { message=\"setup\" } }\nmodule setup\nresult=throw(\"path failed\")\nstable=7\npending=read(\"life:pending\")\n";
    let (mut host, module_entry, entries) = prepared(source, &["result", "stable", "pending"]);
    host.start_then(module_entry, vec![], entries[0], 0, None)
        .unwrap();
    let (token, _) = log(&mut host, &[]);
    host.acknowledge_log(&token, Ok(()), 0).unwrap();
    assert_eq!(ready(&mut host, &[]).unwrap_err().message, "path failed");
    // fail() drops the VM inputs. Only the Host intermediate retains this module.
    host.collect(&[]);
    let module = host.take_intermediate_result().unwrap().unwrap();
    start(&mut host, entries[1], &module);
    assert_eq!(
        ready(&mut host, std::slice::from_ref(&module)),
        Ok(Value::Int(7.into()))
    );

    for cancel in [false, true] {
        let (mut host, module_entry, entries) = prepared(source, &["pending"]);
        host.start_then(module_entry, vec![], entries[0], 0, Some(10))
            .unwrap();
        let (token, _) = log(&mut host, &[]);
        host.acknowledge_log(&token, Ok(()), 9).unwrap();
        let token = loop {
            match host.poll(9, 100) {
                HostPoll::Yielded => {
                    host.collect(&[]);
                }
                HostPoll::Read { token, .. } => break token,
                other => panic!("unexpected {other:?}"),
            }
        };
        if cancel {
            host.cancel();
        }
        let HostPoll::Ready(Err(error)) = host.poll(10, 100) else {
            panic!("terminal expected")
        };
        assert_eq!(
            error.kind,
            if cancel {
                ErrorKind::Cancelled
            } else {
                ErrorKind::Timeout
            }
        );
        assert_eq!(host.drain_cancelled_reads(), vec![token.clone()]);
        assert!(host.take_intermediate_result().is_none());
        assert!(host.resume(&token, Ok(Value::Int(1.into())), 10).is_err());
        assert_eq!(host.heap_stats().slots, 0);
    }
}
