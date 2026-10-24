//! Source-driven callback sequencing and lifetime checks. Java captures remain
//! the separate compatibility authority for complete result and callback bytes.
use super::*;
use crate::host::{HostPoll, HostSession};
use crate::{parser::parse_module, resolver::resolve_module};

fn setup(
    source: &str,
    expressions: &[&str],
    delivery: LogDelivery,
) -> (HostSession, Value, Vec<usize>) {
    let syntax = parse_module(source).unwrap();
    let mut program = compile_module(&resolve_module(&syntax).unwrap()).unwrap();
    Arc::make_mut(&mut program).module_name = "frames".into();
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
    let mut host = HostSession::with_context(
        program,
        EvaluationContext {
            primary_source: Some(Arc::new(crate::value::RuntimeSource {
                text: source.into(),
                uri: "file:///frames.pkl".into(),
                module_name: "frames".into(),
            })),
            log_delivery: delivery,
            frame_transformer: FrameTransformer::Host,
            ..Default::default()
        },
    );
    host.start(entry, vec![], 0, None).unwrap();
    let HostPoll::Ready(Ok(module)) = next(&mut host, &[]) else {
        panic!("module initialization");
    };
    (host, module, entries)
}
fn next(host: &mut HostSession, roots: &[Value]) -> HostPoll {
    for _ in 0..100_000 {
        let event = host.poll(0, 1);
        host.collect(roots);
        if event != HostPoll::Yielded {
            return event;
        }
    }
    panic!("callback scheduler did not settle");
}
fn transform(host: &mut HostSession, roots: &[Value]) -> (String, Arc<LogFrame>) {
    let HostPoll::TransformFrame { token, frame } = next(host, roots) else {
        panic!("expected transform");
    };
    (token, frame)
}
fn finish_identity_diagnostic(host: &mut HostSession, roots: &[Value]) -> RuntimeError {
    loop {
        match next(host, roots) {
            HostPoll::TransformFrame { token, frame } => host
                .complete_frame_transform(&token, Ok((*frame).clone()), 0)
                .unwrap(),
            HostPoll::Ready(Err(error)) => return error,
            event => panic!("expected diagnostic, got {event:?}"),
        }
    }
}
fn replacement() -> LogFrame {
    LogFrame {
        module_uri: PklString::from_utf16(vec![0x70, 0x6b, 0x6c, 0x3a, 0xd800, 0x20]),
        member_name: Some(PklString::from_utf16(vec![0xdc00])),
        source_lines: vec![PklString::from_utf16(vec![32, 34, 0xd800, 34, 32])].into(),
        start_line: 9,
        start_column: 2,
        end_line: 9,
        end_column: 4,
    }
}

#[test]
fn trace_transform_is_separate_from_delegate_and_retains_complete_utf16_frame() {
    let (mut host, module, entries) = setup("value=trace(42)\n", &["value"], LogDelivery::Host);
    let roots = [module.clone()];
    host.start(entries[0], roots.to_vec(), 0, None).unwrap();
    let (transform_token, raw) = transform(&mut host, &roots);
    assert_eq!(raw.module_uri, "file:///frames.pkl");
    assert_eq!(raw.member_name, None);
    assert_eq!(
        host.vm_for_test().log_buffer,
        "42 = 42\n".encode_utf16().collect::<Vec<_>>()
    );
    let stale = host
        .acknowledge_log(&transform_token, Ok(()), 0)
        .unwrap_err();
    assert_eq!(stale.callback_origin(), None);
    assert!(
        host.set_frame_transformer(FrameTransformer::Default)
            .is_err()
    );
    let transformed = replacement();
    host.complete_frame_transform(&transform_token, Ok(transformed.clone()), 0)
        .unwrap();
    let HostPoll::Log { token, event } = next(&mut host, &roots) else {
        panic!("expected logger");
    };
    assert_ne!(token, transform_token);
    assert_eq!(event.frame, transformed);
    assert_eq!(event.message, "42 = 42");
    assert!(
        host.complete_frame_transform(&token, Ok((*raw).clone()), 0)
            .is_err()
    );
    assert!(
        host.complete_frame_transform(&transform_token, Ok((*raw).clone()), 0)
            .is_err()
    );
    host.acknowledge_log(&token, Ok(()), 0).unwrap();
    assert_eq!(
        next(&mut host, &roots),
        HostPoll::Ready(Ok(Value::string("42")))
    );
    host.close();
    assert_eq!(host.heap_stats().slots, 0);
    assert_eq!(event.frame, transformed);
}

#[test]
fn noop_warning_waits_for_transform_before_publishing_and_failed_transform_retries() {
    let (mut host, module, entries) = setup(
        "@Deprecated\nfunction old()=42\nfunction run()=old()\n",
        &["run()"],
        LogDelivery::Noop,
    );
    let roots = [module];
    host.start(entries[0], roots.to_vec(), 0, None).unwrap();
    let (token, _) = transform(&mut host, &roots);
    assert_eq!(
        host.vm_for_test().log_buffer,
        "Method `frames#old` is deprecated\n"
            .encode_utf16()
            .collect::<Vec<_>>()
    );
    host.complete_frame_transform(&token, Err(LoggerFailure::new("failed transform")), 0)
        .unwrap();
    let error = finish_identity_diagnostic(&mut host, &roots);
    assert_eq!(error.message, "failed transform");
    assert_eq!(
        error.callback_origin(),
        Some(CallbackOrigin::LogTransformer)
    );
    host.start(entries[0], roots.to_vec(), 0, None).unwrap();
    let (token, frame) = transform(&mut host, &roots);
    host.complete_frame_transform(&token, Ok((*frame).clone()), 0)
        .unwrap();
    assert_eq!(
        next(&mut host, &roots),
        HostPoll::Ready(Ok(Value::string("42")))
    );
    host.start(entries[0], roots.to_vec(), 0, None).unwrap();
    assert_eq!(
        next(&mut host, &roots),
        HostPoll::Ready(Ok(Value::string("42")))
    );
    assert!(host.vm_for_test().log_buffer.is_empty());
}

#[test]
fn transformer_reentry_publishes_inner_target_even_when_outer_transform_fails() {
    let (mut host, module, entries) = setup(
        "@Deprecated\nfunction old()=42\nfunction run()=old()\n",
        &["run()"],
        LogDelivery::Host,
    );
    let roots = [module];
    host.start(entries[0], roots.to_vec(), 0, None).unwrap();
    let (outer, _) = transform(&mut host, &roots);
    host.reenter(&outer, entries[0], roots.to_vec(), 0, None)
        .unwrap();
    let (inner, frame) = transform(&mut host, &roots);
    assert!(
        host.complete_frame_transform(&outer, Ok((*frame).clone()), 0)
            .is_err()
    );
    host.complete_frame_transform(&inner, Ok((*frame).clone()), 0)
        .unwrap();
    let HostPoll::Log { token, .. } = next(&mut host, &roots) else {
        panic!("nested logger");
    };
    host.acknowledge_log(&token, Ok(()), 0).unwrap();
    assert_eq!(
        next(&mut host, &roots),
        HostPoll::Ready(Ok(Value::string("42")))
    );
    host.complete_frame_transform(&outer, Err(LoggerFailure::new("outer failed")), 0)
        .unwrap();
    assert_eq!(
        finish_identity_diagnostic(&mut host, &roots).message,
        "outer failed"
    );
    host.start(entries[0], roots.to_vec(), 0, None).unwrap();
    assert_eq!(
        next(&mut host, &roots),
        HostPoll::Ready(Ok(Value::string("42")))
    );
}

#[test]
fn diagnostic_transforms_compressed_raw_frames_and_renders_replacement_units() {
    let (mut host, module, entries) = setup(
        "function recurse(n)=if(n==0) throw(\"boom\") else recurse(n-1)\n",
        &["recurse(5)"],
        LogDelivery::Noop,
    );
    let roots = [module];
    host.start(entries[0], roots.to_vec(), 0, None).unwrap();
    let mut transformed = 0;
    let error = loop {
        match next(&mut host, &roots) {
            HostPoll::TransformFrame { token, frame } => {
                assert!(frame.member_name.is_some());
                assert!(matches!(
                    frame.module_uri.as_str(),
                    Some("file:///frames.pkl" | "repl:text")
                ));
                transformed += 1;
                host.complete_frame_transform(&token, Ok(replacement()), 0)
                    .unwrap();
            }
            HostPoll::Ready(Err(error)) => break error,
            event => panic!("unexpected {event:?}"),
        }
    };
    assert!(
        transformed < 7,
        "raw recursion must compress before callbacks"
    );
    assert_eq!(error.callback_origin(), None);
    let rendered = error.rendered_diagnostic.as_ref().unwrap();
    let units: Vec<_> = rendered.encode_utf16().collect();
    assert!(units.windows(3).any(|units| units == [34, 0xd800, 34]));
    assert!(units.contains(&0xdc00));
    assert!(rendered.to_java_utf8().contains("repetitions of:"));
    assert!(!rendered.to_java_utf8().contains("github.com"));
}

#[test]
fn diagnostic_transform_can_reenter_and_rejection_does_not_repeat_formatter() {
    let (mut host, module, entries) = setup(
        "bad=throw(\"boom\")\ngood=7\n",
        &["bad", "good"],
        LogDelivery::Noop,
    );
    let roots = [module];
    host.start(entries[0], roots.to_vec(), 0, None).unwrap();
    let (outer, _) = transform(&mut host, &roots);
    host.reenter(&outer, entries[1], roots.to_vec(), 0, None)
        .unwrap();
    assert_eq!(
        next(&mut host, &roots),
        HostPoll::Ready(Ok(Value::string("7")))
    );
    host.complete_frame_transform(
        &outer,
        Err(LoggerFailure::new(PklString::from_utf16(vec![0xd800]))),
        0,
    )
    .unwrap();
    let HostPoll::Ready(Err(error)) = next(&mut host, &roots) else {
        panic!("formatter failure must escape");
    };
    assert_eq!(
        error.callback_origin(),
        Some(CallbackOrigin::DiagnosticTransformer)
    );
    assert_eq!(
        error
            .rendered_diagnostic
            .as_ref()
            .unwrap()
            .encode_utf16()
            .collect::<Vec<_>>(),
        [0xd800]
    );
    host.start(entries[1], roots.to_vec(), 0, None).unwrap();
    assert_eq!(
        next(&mut host, &roots),
        HostPoll::Ready(Ok(Value::string("7")))
    );
}

#[test]
fn raw_stdlib_sources_default_and_identity_frames_have_distinct_boundaries() {
    let source = crate::stdlib_sources::reference();
    assert_eq!(&*source.uri, "pkl:ref");
    let offset = source.text.find("renderReference(this)").unwrap();
    let raw = tracing::source_frame(
        &source,
        Span::new(offset, offset + "renderReference(this)".len()),
    )
    .unwrap();
    assert_eq!(raw.module_uri, "pkl:ref");
    assert_eq!(FrameTransformer::Identity.transform(raw.clone()), raw);
    let transformed = FrameTransformer::Default.transform(raw.clone());
    assert_eq!(
        transformed.module_uri,
        format!(
            "https://github.com/apple/pkl/blob/0.32.0/stdlib/ref.pkl#L{}",
            raw.start_line
        )
    );
    assert_eq!(transformed.source_lines, raw.source_lines);
}

#[test]
fn transform_timeout_and_cancel_release_tree_and_reject_both_callback_stages() {
    for timeout in [false, true] {
        let (mut host, module, entries) = setup("value=trace(42)\n", &["value"], LogDelivery::Host);
        let roots = [module];
        host.start(entries[0], roots.to_vec(), 0, Some(10)).unwrap();
        let (token, frame) = transform(&mut host, &roots);
        if timeout {
            let HostPoll::Ready(Err(error)) = host.poll(10, 1) else {
                panic!("deadline");
            };
            assert_eq!(error.kind, ErrorKind::Timeout);
        } else {
            host.cancel();
        }
        assert_eq!(host.heap_stats().slots, 0);
        assert!(
            host.complete_frame_transform(&token, Ok((*frame).clone()), 10)
                .is_err()
        );
        assert!(host.acknowledge_log(&token, Ok(()), 10).is_err());
    }
}

#[test]
fn close_from_transform_finishes_current_delegate_and_rejects_new_evaluation() {
    let (mut host, module, entries) = setup("value=trace(42)\n", &["value"], LogDelivery::Host);
    let roots = [module];
    host.start(entries[0], roots.to_vec(), 0, None).unwrap();
    let (token, frame) = transform(&mut host, &roots);
    host.close_from_callback(&token).unwrap();
    assert!(
        host.reenter(&token, entries[0], roots.to_vec(), 0, None)
            .is_err()
    );
    host.complete_frame_transform(&token, Ok((*frame).clone()), 0)
        .unwrap();
    let HostPoll::Log {
        token: delegate, ..
    } = next(&mut host, &roots)
    else {
        panic!("existing delegate");
    };
    host.acknowledge_log(&delegate, Ok(()), 0).unwrap();
    assert_eq!(
        next(&mut host, &roots),
        HostPoll::Ready(Ok(Value::string("42")))
    );
    assert!(host.is_closed());
    assert_eq!(host.heap_stats().slots, 0);
    assert!(host.start(entries[0], roots.to_vec(), 0, None).is_err());
}

#[test]
fn logger_failure_does_not_cache_the_transformed_frame_or_bind_warning_target() {
    let (mut host, module, entries) = setup(
        "@Deprecated\nfunction old()=42\nfunction run()=old()\n",
        &["run()"],
        LogDelivery::Host,
    );
    let roots = [module];
    host.start(entries[0], roots.to_vec(), 0, None).unwrap();
    let (token, raw) = transform(&mut host, &roots);
    host.complete_frame_transform(&token, Ok(replacement()), 0)
        .unwrap();
    let HostPoll::Log { token, event } = next(&mut host, &roots) else {
        panic!("logger");
    };
    assert_eq!(event.frame, replacement());
    host.acknowledge_log(&token, Err(LoggerFailure::new("logger rejected")), 0)
        .unwrap();
    let error = finish_identity_diagnostic(&mut host, &roots);
    assert_eq!(error.message, "logger rejected");
    assert_eq!(error.callback_origin(), Some(CallbackOrigin::LogDelegate));
    host.start(entries[0], roots.to_vec(), 0, None).unwrap();
    let (token, retry) = transform(&mut host, &roots);
    assert_eq!(retry, raw);
    host.complete_frame_transform(&token, Ok((*retry).clone()), 0)
        .unwrap();
    let HostPoll::Log { token, event } = next(&mut host, &roots) else {
        panic!("retry logger");
    };
    assert_eq!(event.frame, *raw);
    host.acknowledge_log(&token, Ok(()), 0).unwrap();
    assert_eq!(
        next(&mut host, &roots),
        HostPoll::Ready(Ok(Value::string("42")))
    );
}

#[test]
fn identity_policy_preserves_raw_stdlib_uri_for_logs_and_diagnostics() {
    let (mut host, module, entries) = setup(
        "value=trace(42)\nbad=throw(\"boom\")\n",
        &["value", "bad"],
        LogDelivery::Host,
    );
    let roots = [module];
    if let Some(source) = host.vm_for_test_mut().context.primary_source.as_mut() {
        Arc::make_mut(source).uri = "pkl:sample".into();
    }
    host.set_frame_transformer(FrameTransformer::Identity)
        .unwrap();
    host.start(entries[0], roots.to_vec(), 0, None).unwrap();
    let HostPoll::Log { token, event } = next(&mut host, &roots) else {
        panic!("identity delegate");
    };
    assert_eq!(event.frame.module_uri, "pkl:sample");
    host.acknowledge_log(&token, Ok(()), 0).unwrap();
    assert_eq!(
        next(&mut host, &roots),
        HostPoll::Ready(Ok(Value::string("42")))
    );
    host.start(entries[1], roots.to_vec(), 0, None).unwrap();
    let HostPoll::Ready(Err(error)) = next(&mut host, &roots) else {
        panic!("identity diagnostic");
    };
    let mut sources = crate::source::SourceMap::default();
    let source = host.vm_for_test().context.primary_source.as_ref().unwrap();
    let id = sources.add(source.uri.as_ref(), source.text.as_ref());
    let rendered = crate::evaluation::runtime_error_text(
        sources.get(id).unwrap(),
        &source.uri,
        &source.module_name,
        "",
        error,
        None,
    );
    assert!(rendered.to_java_utf8().contains("(pkl:sample)"));
    assert!(!rendered.to_java_utf8().contains("github.com"));
}

#[test]
fn invalid_diagnostic_replacement_fails_before_the_next_transform_callback() {
    for empty_lines in [false, true] {
        let (mut host, module, entries) =
            setup("bad=throw(\"boom\")\n", &["bad"], LogDelivery::Noop);
        let roots = [module];
        host.start(entries[0], roots.to_vec(), 0, None).unwrap();
        let (token, _) = transform(&mut host, &roots);
        let mut frame = replacement();
        if empty_lines {
            frame.source_lines = Vec::new().into();
        } else {
            frame.start_column = 10;
            frame.end_column = 1;
        }
        host.complete_frame_transform(&token, Ok(frame), 0).unwrap();
        let HostPoll::Ready(Err(error)) = next(&mut host, &roots) else {
            panic!("invalid replacement must stop rendering immediately");
        };
        assert_eq!(error.kind, ErrorKind::Host);
        assert_eq!(
            error.callback_origin(),
            Some(CallbackOrigin::DiagnosticRenderer)
        );
        assert!(error.rendered_diagnostic.is_some());
        assert_eq!(host.context_depth(), 0);
    }
}

/// Complete frames from sealed Pkl 0.32.0 main/cases/{transformer-throw-once,
/// logger-throw-once,diagnostic-logger-throw-once}/0/transform-*.input.*.
/// These callback exceptions originate at MemberNode's header, whereas ordinary
/// guest failures continue to originate at their throwing expression.
#[test]
fn foreign_log_failures_use_enclosing_member_header_and_keep_trace_root() {
    use crate::evaluation::session::{SourceSession, SourceSessionOptions};
    let source = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/m2-frame-transformers/inputs/basic.pkl"),
    )
    .unwrap();
    let uri = "file:///stage55-frame.pkl";
    let expected = |expression: &str, trace: bool| {
        vec![
            LogFrame {
                module_uri: uri.into(),
                member_name: Some(
                    if trace {
                        "stage55Frames#traceThenFail"
                    } else {
                        "stage55Frames#retained"
                    }
                    .into(),
                ),
                source_lines: vec![
                    if trace {
                        "function traceThenFail() = trace(1) + fail()"
                    } else {
                        "function retained(x) = old(x)"
                    }
                    .into(),
                ]
                .into(),
                start_line: if trace { 10 } else { 4 },
                start_column: 1,
                end_line: if trace { 10 } else { 4 },
                end_column: if trace { 24 } else { 20 },
            },
            LogFrame {
                module_uri: "repl:text".into(),
                member_name: Some("".into()),
                source_lines: vec![expression.into()].into(),
                start_line: 1,
                start_column: 1,
                end_line: 1,
                end_column: expression.len(),
            },
        ]
    };
    for trace in [false, true] {
        for delegate_failure in [false, true] {
            let expression = if trace {
                "traceThenFail()"
            } else {
                "retained(1)"
            };
            let mut session = SourceSession::compile_expressions_text(
                &source,
                uri,
                "stage55Frames",
                &[expression.into()],
                SourceSessionOptions {
                    frame_transformer: FrameTransformer::Host,
                    log_delivery: LogDelivery::Host,
                    ..Default::default()
                },
            )
            .unwrap();
            session.start_path(0, 0, None).unwrap();
            let mut callback_failed = false;
            let mut frames = Vec::new();
            let error = loop {
                let event = session.poll(0, 1);
                session.collect(&[]);
                match event {
                    HostPoll::Yielded => (),
                    HostPoll::TransformFrame { token, frame } => {
                        if !callback_failed && !delegate_failure {
                            callback_failed = true;
                            session
                                .complete_frame_transform(
                                    &token,
                                    Err(LoggerFailure::new("foreign callback")),
                                    0,
                                )
                                .unwrap();
                        } else {
                            if callback_failed {
                                frames.push((*frame).clone());
                            }
                            session
                                .complete_frame_transform(&token, Ok((*frame).clone()), 0)
                                .unwrap();
                        }
                    }
                    HostPoll::Log { token, .. } => {
                        assert!(delegate_failure && !callback_failed);
                        callback_failed = true;
                        session
                            .acknowledge_log(&token, Err(LoggerFailure::new("foreign callback")), 0)
                            .unwrap();
                    }
                    HostPoll::Ready(Err(error)) => break error,
                    event => panic!("unexpected {event:?}"),
                }
            };
            assert_eq!(frames, expected(expression, trace));
            assert_eq!(
                error.callback_origin(),
                Some(if delegate_failure {
                    CallbackOrigin::LogDelegate
                } else {
                    CallbackOrigin::LogTransformer
                })
            );
            session.close();
            assert_eq!(session.heap_stats().slots, 0);
        }
    }
}
