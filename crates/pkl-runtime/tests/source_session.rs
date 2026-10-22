//! Actual parsed-source session and transport ownership regressions. Oracle raw
//! logger evidence is acquired independently; these assert shared API lifecycle.
use pkl_runtime::{
    evaluation::session::{SourceSession, SourceSessionOptions},
    host::HostPoll,
    logging::LogDelivery,
    probe::SourceProbe,
    value::Value,
};

fn settle(session: &mut SourceSession) -> HostPoll {
    settle_at(session, 0)
}
fn settle_at(session: &mut SourceSession, now: u64) -> HostPoll {
    for _ in 0..100_000 {
        let event = session.poll(now, 1);
        let roots = match &event {
            HostPoll::Ready(Ok(value)) => vec![value.clone()],
            _ => Vec::new(),
        };
        session.collect(&roots);
        if !matches!(event, HostPoll::Yielded) {
            return event;
        }
    }
    panic!("source session did not settle")
}
fn setup(session: &mut SourceSession) {
    session.start_module(0, None).unwrap();
    assert!(matches!(
        settle(session),
        HostPoll::Ready(Ok(Value::Object(_)))
    ));
}

#[test]
fn actual_source_read_reentry_retains_module_through_gc_and_retries() {
    let mut session = SourceSession::compile(
        "result=read(\"life:text\")\nstable=7\n",
        "file:///source-session.pkl",
        "source-session",
        &["result".into(), "stable".into()],
        SourceSessionOptions::default(),
    )
    .unwrap();
    setup(&mut session);
    session.start_path(0, 0, None).unwrap();
    let HostPoll::Read { token, .. } = settle(&mut session) else {
        panic!("source resource read");
    };
    session.reenter_path(&token, 1, 0, None).unwrap();
    let HostPoll::Ready(Ok(Value::String(value))) = settle(&mut session) else {
        panic!("nested scalar");
    };
    assert_eq!(value.as_str(), Some("7"));
    assert!(matches!(settle(&mut session),HostPoll::Read { token: pending,.. } if pending==token));
    session.resume(&token, Ok(Value::string("R")), 0).unwrap();
    let HostPoll::Ready(Ok(Value::String(value))) = settle(&mut session) else {
        panic!("outer string");
    };
    assert_eq!(value.as_str(), Some("R"));
    session.start_path(0, 0, None).unwrap();
    assert!(matches!(settle(&mut session), HostPoll::Ready(Ok(_))));
    session.close();
    assert_eq!(session.heap_stats().slots, 0);
}

#[test]
fn source_transport_preserves_utf16_log_snapshot_and_explicit_ack() {
    let mut probe = SourceProbe::new(
        "result=trace(\"\\u{D800}\")\nstable=7\n",
        "file:///trace.pkl",
        "trace",
        &["result".into(), "stable".into()],
        "compact",
        true,
        false,
        false,
    )
    .unwrap();
    probe.start_module(0, None).unwrap();
    loop {
        if probe.poll(0, 1).unwrap().kind == "module" {
            break;
        }
    }
    probe.start_path(0, 0, None).unwrap();
    assert!(probe.poll(0, 0).is_err());
    assert_eq!(probe.context_depth(), 1);
    let event = loop {
        let event = probe.poll(0, 1).unwrap();
        probe.collect();
        if event.kind != "yield" {
            break event;
        }
    };
    assert_eq!(event.kind, "log");
    let token = event.token.unwrap();
    let snapshot = event.log.unwrap();
    assert!(snapshot.message.contains(&0xd800));
    assert_eq!(snapshot.member_name, None);
    assert_eq!(
        snapshot.module_uri,
        "file:///trace.pkl".encode_utf16().collect::<Vec<_>>()
    );
    assert!(probe.respond(&token, Value::Int(9), 0).is_err());
    probe.reenter_path(&token, 1, 0, None).unwrap();
    let inner = loop {
        let event = probe.poll(0, 1).unwrap();
        probe.collect();
        if event.kind != "yield" {
            break event;
        }
    };
    assert_eq!(inner.kind, "value");
    assert_eq!(inner.value, Some(vec![b'7'.into()]));
    let pending = probe.poll(0, 1).unwrap();
    assert_eq!(pending.token.as_deref(), Some(token.as_str()));
    probe.acknowledge_log(&token, None, 0).unwrap();
    let result = loop {
        let event = probe.poll(0, 1).unwrap();
        probe.collect();
        if event.kind != "yield" {
            break event;
        }
    };
    assert_eq!(result.kind, "value");
    assert_eq!(result.value, Some(vec![0xd800]));
    assert!(probe.acknowledge_log(&token, None, 0).is_err());
    probe.close();
    assert_eq!(probe.heap_slots(), 0);
    assert!(snapshot.message.contains(&0xd800));
}

#[test]
fn source_session_setup_can_suspend_for_real_resource_io() {
    let source = "class Marker extends Annotation { value:String }\n@Marker { when (read(\"life:setup\")==\"go\") { value=\"ready\" } }\nresult=1\n";
    let mut session = SourceSession::compile(
        source,
        "file:///setup.pkl",
        "setup",
        &["result".into()],
        SourceSessionOptions {
            log_delivery: LogDelivery::Host,
            ..Default::default()
        },
    )
    .unwrap();
    session.start_module(0, None).unwrap();
    let HostPoll::Read { token, .. } = settle(&mut session) else {
        panic!("setup should expose actual Host event");
    };
    session.resume(&token, Ok(Value::string("go")), 0).unwrap();
    assert!(matches!(
        settle(&mut session),
        HostPoll::Ready(Ok(Value::Object(_)))
    ));
    session.start_path(0, 0, None).unwrap();
    assert!(matches!(
        settle(&mut session),
        HostPoll::Ready(Ok(Value::String(_)))
    ));
    session.close();
    assert_eq!(session.heap_stats().slots, 0);
}

#[test]
fn bootstrap_logger_reentry_observes_empty_module_then_actual_publication() {
    let source = "@Deprecated { when(trace(true)) { message=\"setup\" } }\nmodule setup\nstable=trace(7)\nresult=8\n";
    let mut session = SourceSession::compile(
        source,
        "file:///setup.pkl",
        "inferred",
        &["stable".into(), "result".into()],
        SourceSessionOptions {
            log_delivery: LogDelivery::Host,
            ..Default::default()
        },
    )
    .unwrap();
    session.start_module(0, None).unwrap();
    let HostPoll::Log { token, .. } = settle(&mut session) else {
        panic!("actual module annotation trace");
    };
    session.reenter_path(&token, 0, 0, None).unwrap();
    let HostPoll::Ready(Err(error)) = settle(&mut session) else {
        panic!("bootstrap declarations are unavailable");
    };
    assert_eq!(
        session.format_error(error),
        "–– Pkl Error ––\nCannot find property `stable`.\n\n1 | stable\n    ^^^^^^\nat  (repl:text)\n"
    );
    session.acknowledge_log(&token, Ok(()), 0).unwrap();
    assert!(matches!(
        settle(&mut session),
        HostPoll::Ready(Ok(Value::Object(_)))
    ));
    session.start_path(1, 0, None).unwrap();
    let HostPoll::Ready(Ok(Value::String(value))) = settle(&mut session) else {
        panic!("published result");
    };
    assert_eq!(value.as_str(), Some("8"));
    session.close();
    assert_eq!(session.heap_stats().slots, 0);
}

#[test]
fn initialization_failure_is_replayed_without_new_module_or_log() {
    let source = "@Deprecated { when(trace(true)) { message=\"setup\" }; when(throw(\"setup-after-log\")) { message=\"after\" } }\nmodule setup\nstable=trace(7)\nresult=8\n";
    let mut session = SourceSession::compile(
        source,
        "file:///setup.pkl",
        "inferred",
        &["stable".into(), "result".into()],
        SourceSessionOptions {
            log_delivery: LogDelivery::Host,
            ..Default::default()
        },
    )
    .unwrap();
    session.start_module(0, None).unwrap();
    let HostPoll::Log { token, .. } = settle(&mut session) else {
        panic!("setup trace");
    };
    session.acknowledge_log(&token, Ok(()), 0).unwrap();
    let HostPoll::Ready(Err(error)) = settle(&mut session) else {
        panic!("setup failure");
    };
    let initial = session.format_error(error);
    for index in [0, 1, 0] {
        session.start_path(index, 0, None).unwrap();
        let HostPoll::Ready(Err(error)) = settle(&mut session) else {
            panic!("cached initialization failure");
        };
        assert_eq!(session.format_error(error), initial);
    }
    assert!(session.start_module(0, None).is_err());
    session.close();
    assert_eq!(session.heap_stats().slots, 0);
}

#[test]
fn source_compile_error_keeps_complete_oracle_diagnostic_before_host_creation() {
    // Pinned Java frontend observation: grouped new remains an inference boundary.
    let source = "class C { x=7 }\nfoo: C=trace((new {}))\nresult=foo.x\n";
    let error = match SourceSession::compile(
        source,
        "file:///stage37-trace.pkl",
        "stage37-trace",
        &["result".into(), "foo".into()],
        SourceSessionOptions::default(),
    ) {
        Err(error) => error,
        Ok(_) => panic!("invalid inferred parent must fail compilation"),
    };
    assert_eq!(
        error,
        "–– Pkl Error ––\nCannot tell which parent to amend.\n\n2 | foo: C=trace((new {}))\n                  ^^^\nat stage37-trace#foo (file:///stage37-trace.pkl)\n\nPlease specify a parent explicitly, for example `new Person { ... }` instead of `new { ... }`.\n"
    );
}

#[test]
fn cached_setup_error_keeps_host_clock_admission_and_terminal_timeout() {
    use pkl_runtime::value::ErrorKind;
    let source = "@Deprecated { when(throw(\"setup-error\")) { message=\"after\" } }\nmodule setup\nresult=8\n";
    let mut session = SourceSession::compile(
        source,
        "file:///setup.pkl",
        "setup",
        &["result".into()],
        SourceSessionOptions::default(),
    )
    .unwrap();
    session.start_module(5, None).unwrap();
    let HostPoll::Ready(Err(error)) = settle_at(&mut session, 5) else {
        panic!("initial setup error");
    };
    let original = session.format_error(error);
    assert_eq!(
        session.start_path(0, 4, None).unwrap_err().kind,
        ErrorKind::Host
    );
    session.start_path(0, 6, None).unwrap();
    let HostPoll::Ready(Err(error)) = settle_at(&mut session, 6) else {
        panic!("cached setup error after rejected clock");
    };
    assert_eq!(session.format_error(error), original);
    session.start_path(0, 7, Some(0)).unwrap();
    let HostPoll::Ready(Err(error)) = session.poll(7, 1) else {
        panic!("cached-error evaluation deadline");
    };
    assert_eq!(error.kind, ErrorKind::Timeout);
    assert!(session.is_closed());
    assert_eq!(session.heap_stats().slots, 0);
}

#[test]
fn first_path_finishes_after_module_or_member_annotation_callback_close() {
    use pkl_runtime::value::ErrorKind;
    // Fresh pinned Java observations: bootstrap-close/{module,member}-annotation-close.
    for source in [
        "@Deprecated { when(trace(true)) { message=\"setup\" } }\nmodule setup\nstable=7\nresult=8\n",
        "stable=7\n@Deprecated { when(trace(true)) { message=\"setup\" } }\nresult=8\n",
    ] {
        let mut session = SourceSession::compile(
            source,
            "file:///stage37-trace.pkl",
            "stage37-trace",
            &["result".into(), "stable".into()],
            SourceSessionOptions {
                log_delivery: LogDelivery::Host,
                ..Default::default()
            },
        )
        .unwrap();
        session.start_path(0, 0, None).unwrap();
        let HostPoll::Log { token, .. } = settle(&mut session) else {
            panic!("annotation logger callback");
        };
        assert_eq!(session.context_depth(), 1);
        session.close_from_callback(&token).unwrap();
        assert_eq!(
            session.reenter_path(&token, 1, 0, None).unwrap_err().kind,
            ErrorKind::Closed
        );
        session.acknowledge_log(&token, Ok(()), 0).unwrap();
        let HostPoll::Ready(Ok(Value::String(value))) = settle(&mut session) else {
            panic!("original requested property must finish before close");
        };
        assert_eq!(value.as_str(), Some("8"));
        assert!(!session.setup_failed());
        assert!(session.is_closed());
        assert_eq!(session.context_depth(), 0);
        assert_eq!(session.heap_stats().slots, 0);
        assert_eq!(
            session.start_path(1, 0, None).unwrap_err().kind,
            ErrorKind::Closed
        );
    }
}

#[test]
fn first_path_distinguishes_cached_setup_failure_from_property_failure() {
    let setup_error = "@Deprecated { when(throw(\"setup-error\")) { message=\"after\" } }\nmodule setup\nresult=8\nstable=7\n";
    let path_error = "result=throw(\"path-error\")\nstable=7\n";
    for (source, initialization_fails) in [(setup_error, true), (path_error, false)] {
        let mut session = SourceSession::compile(
            source,
            "file:///setup.pkl",
            "setup",
            &["result".into(), "stable".into()],
            SourceSessionOptions::default(),
        )
        .unwrap();
        session.start_path(0, 0, None).unwrap();
        let HostPoll::Ready(Err(error)) = settle(&mut session) else {
            panic!("first requested expression fails");
        };
        let original = session.format_error(error);
        assert_eq!(session.setup_failed(), initialization_fails);
        session.collect(&[]);
        session.start_path(1, 0, None).unwrap();
        match settle(&mut session) {
            HostPoll::Ready(Err(error)) if initialization_fails => {
                assert_eq!(session.format_error(error), original);
            }
            HostPoll::Ready(Ok(Value::String(value))) if !initialization_fails => {
                assert_eq!(value.as_str(), Some("7"));
            }
            event => panic!("setup cache must not capture a later property failure: {event:?}"),
        }
        session.close();
        assert_eq!(session.setup_failed(), initialization_fails);
        assert_eq!(session.heap_stats().slots, 0);
    }
}

#[test]
fn first_path_deadline_spans_setup_and_property_evaluation() {
    use pkl_runtime::value::ErrorKind;
    for source in [
        "@Deprecated { when(trace(true)) { message=\"setup\" } }\nmodule setup\nresult=8\n",
        "result=trace(8)\n",
    ] {
        let mut session = SourceSession::compile(
            source,
            "file:///setup.pkl",
            "setup",
            &["result".into()],
            SourceSessionOptions {
                log_delivery: LogDelivery::Host,
                ..Default::default()
            },
        )
        .unwrap();
        session.start_path(0, 5, Some(2)).unwrap();
        let HostPoll::Log { token, .. } = settle_at(&mut session, 5) else {
            panic!("trace before or after initialization completed");
        };
        session.close_from_callback(&token).unwrap();
        session.acknowledge_log(&token, Ok(()), 6).unwrap();
        let HostPoll::Ready(Err(error)) = settle_at(&mut session, 7) else {
            panic!("the original first-call deadline must remain active");
        };
        assert_eq!(error.kind, ErrorKind::Timeout);
        assert!(
            !session.setup_failed(),
            "Host termination is not a failed initialization result"
        );
        assert!(session.is_closed());
        assert_eq!(session.context_depth(), 0);
        assert_eq!(session.heap_stats().slots, 0);
    }
}

#[test]
fn cancelled_first_path_does_not_continue_setup_or_retain_module() {
    use pkl_runtime::value::ErrorKind;
    for source in [
        "@Deprecated { when(read(\"boot:condition\") == \"R\") { message=\"setup\" } }\nmodule setup\nresult=8\n",
        "result=read(\"boot:condition\")\n",
    ] {
        let mut session = SourceSession::compile(
            source,
            "file:///setup.pkl",
            "setup",
            &["result".into()],
            SourceSessionOptions::default(),
        )
        .unwrap();
        session.start_path(0, 0, None).unwrap();
        let HostPoll::Read { token, .. } = settle(&mut session) else {
            panic!("read before or after initialization completed");
        };
        session.cancel();
        assert_eq!(session.drain_cancelled_reads(), vec![token.clone()]);
        assert!(session.drain_cancelled_reads().is_empty());
        assert_eq!(session.heap_stats().slots, 0);
        assert_eq!(session.context_depth(), 0);
        assert!(!session.setup_failed());
        assert_eq!(
            session
                .resume(&token, Ok(Value::string("R")), 0)
                .unwrap_err()
                .kind,
            ErrorKind::Closed
        );
        assert!(
            matches!(session.poll(0, 1), HostPoll::Ready(Err(error)) if error.kind==ErrorKind::Cancelled)
        );
        assert!(!session.setup_failed());
    }
}

#[test]
fn callback_close_preserves_initialization_failure_classification() {
    for (source, setup_failed) in [
        (
            "@Deprecated { when(trace(true)) { message=\"setup\" }; when(throw(\"setup-error\")) { message=\"after\" } }\nmodule setup\nresult=8\n",
            true,
        ),
        (
            "@Deprecated { when(trace(true)) { message=\"setup\" } }\nmodule setup\nresult=throw(\"path-error\")\n",
            false,
        ),
    ] {
        let mut session = SourceSession::compile(
            source,
            "file:///setup.pkl",
            "setup",
            &["result".into()],
            SourceSessionOptions {
                log_delivery: LogDelivery::Host,
                ..Default::default()
            },
        )
        .unwrap();
        session.start_path(0, 0, None).unwrap();
        let HostPoll::Log { token, .. } = settle(&mut session) else {
            panic!("setup logger");
        };
        session.close_from_callback(&token).unwrap();
        session.acknowledge_log(&token, Ok(()), 0).unwrap();
        assert!(matches!(settle(&mut session), HostPoll::Ready(Err(_))));
        assert_eq!(session.setup_failed(), setup_failed);
        assert!(session.is_closed());
        assert_eq!(session.heap_stats().slots, 0);
        session.close();
        assert_eq!(session.setup_failed(), setup_failed);
    }
}

#[test]
fn explicit_module_setup_remains_a_separate_operation_after_callback_close() {
    use pkl_runtime::value::ErrorKind;
    let mut session = SourceSession::compile(
        "@Deprecated { when(trace(true)) { message=\"setup\" } }\nmodule setup\nresult=8\n",
        "file:///setup.pkl",
        "setup",
        &["result".into()],
        SourceSessionOptions {
            log_delivery: LogDelivery::Host,
            ..Default::default()
        },
    )
    .unwrap();
    session.start_module(0, None).unwrap();
    let HostPoll::Log { token, .. } = settle(&mut session) else {
        panic!("explicit setup trace");
    };
    session.close_from_callback(&token).unwrap();
    session.acknowledge_log(&token, Ok(()), 0).unwrap();
    assert!(matches!(
        settle(&mut session),
        HostPoll::Ready(Ok(Value::Object(_)))
    ));
    assert!(session.is_closed());
    assert!(!session.setup_failed());
    assert_eq!(session.heap_stats().slots, 0);
    assert_eq!(
        session.start_path(0, 0, None).unwrap_err().kind,
        ErrorKind::Closed
    );
}
