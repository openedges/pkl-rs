//! Module output uses the same actual-source session, context and byte ownership.
//! These lifecycle assertions do not replace the independent Java raw oracle.
use pkl_runtime::{
    evaluation::{
        frontend::{DiagnosticProfile, ModuleEvalOptions, ModuleSourceInput},
        session::{SourceSession, SourceSessionOptions},
    },
    host::HostPoll,
    logging::LogDelivery,
    probe::SourceProbe,
    value::{ErrorKind, PklString, Value},
};

fn session(text: &str, paths: &[&str], logs: bool) -> SourceSession {
    SourceSession::compile_output(
        ModuleSourceInput {
            text,
            uri: "file:///module-output-session.pkl",
            inferred_name: "module-output-session",
        },
        &paths
            .iter()
            .map(|path| (*path).to_owned())
            .collect::<Vec<_>>(),
        &ModuleEvalOptions {
            diagnostic_profile: DiagnosticProfile::ApiIdentity,
            ..Default::default()
        },
        SourceSessionOptions {
            log_delivery: if logs {
                LogDelivery::Host
            } else {
                LogDelivery::Noop
            },
            ..Default::default()
        },
    )
    .unwrap()
}

fn settle(session: &mut SourceSession, now: u64) -> HostPoll {
    for _ in 0..100_000 {
        let event = session.poll(now, 1);
        let roots = match &event {
            HostPoll::Ready(Ok(value)) => vec![value.clone()],
            _ => Vec::new(),
        };
        session.collect(&roots);
        // The actual default renderer reads its configured outputFormat before
        // text/bytes overrides. Complete ordinary profile reads through the
        // shared pending-request policy; expose custom Host reads to each test.
        if let HostPoll::Read { token, uri } = &event
            && (uri.starts_with("prop:") || uri.starts_with("env:"))
        {
            session.resume_default_read(token, &[], &[], now).unwrap();
            continue;
        }
        if !matches!(event, HostPoll::Yielded) {
            return event;
        }
    }
    panic!("module output did not settle");
}

#[test]
fn output_bytes_suspend_reenter_and_survive_gc_and_close() {
    let mut session = session(
        "stable=7\noutput { text=read(\"life:output\") }\n",
        &["stable"],
        false,
    );
    session.start_output_bytes(0, None).unwrap();
    let HostPoll::Read { token, .. } = settle(&mut session, 0) else {
        panic!("real output resource read");
    };
    session.reenter_path(&token, 0, 0, None).unwrap();
    let HostPoll::Ready(Ok(value)) = settle(&mut session, 0) else {
        panic!("same-module callback");
    };
    assert_eq!(
        SourceSession::result_string(value).unwrap().as_str(),
        Some("7")
    );
    assert!(
        matches!(settle(&mut session, 0), HostPoll::Read { token: pending, .. } if pending==token)
    );
    session
        .resume(
            &token,
            Ok(Value::String(PklString::from_utf16(vec![0xd800, 0, 120]))),
            0,
        )
        .unwrap();
    let HostPoll::Ready(Ok(value)) = settle(&mut session, 0) else {
        panic!("complete bytes");
    };
    let bytes = SourceSession::result_bytes(value).unwrap();
    // Java encodeToBytes replaces an isolated surrogate with '?', preserving NUL.
    assert_eq!(bytes.as_ref(), b"?\0x");
    session.close();
    session.collect(&[]);
    assert_eq!(session.heap_stats().slots, 0);
    assert_eq!(bytes.as_ref(), b"?\0x");
    assert_eq!(
        session.start_output_bytes(0, None).unwrap_err().kind,
        ErrorKind::Closed
    );
    assert_eq!(
        session.start_path(usize::MAX, 0, None).unwrap_err().kind,
        ErrorKind::Closed
    );
}

#[test]
fn output_can_reenter_from_a_property_callback_in_the_same_session() {
    let mut session = session(
        "probe=read(\"life:probe\")\noutput { text=\"OUT\" }\n",
        &["probe"],
        false,
    );
    session.start_path(0, 0, None).unwrap();
    let HostPoll::Read { token, .. } = settle(&mut session, 0) else {
        panic!("outer property callback");
    };
    session.reenter_output_bytes(&token, 0, None).unwrap();
    let HostPoll::Ready(Ok(value)) = settle(&mut session, 0) else {
        panic!("nested output completion");
    };
    let bytes = SourceSession::result_bytes(value).unwrap();
    assert_eq!(bytes.as_ref(), b"OUT");
    session.resume(&token, Ok(Value::string("IN")), 0).unwrap();
    let HostPoll::Ready(Ok(value)) = settle(&mut session, 0) else {
        panic!("outer property completion");
    };
    assert_eq!(
        SourceSession::result_string(value).unwrap().as_str(),
        Some("IN")
    );
    session.close();
    assert_eq!(bytes.as_ref(), b"OUT");
}

#[test]
fn output_failure_does_not_become_a_module_setup_failure_or_repl_error() {
    let mut session = session(
        "stable=7\noutput { text=throw(\"output failure\") }\n",
        &["stable"],
        false,
    );
    session.start_output_bytes(0, None).unwrap();
    let HostPoll::Ready(Err(error)) = settle(&mut session, 0) else {
        panic!("actual output failure");
    };
    let rendered = session.output_failure(error);
    assert!(
        !rendered
            .stderr
            .windows(b"repl:text".len())
            .any(|part| part == b"repl:text")
    );
    assert!(rendered.stdout.is_empty());
    assert!(!session.setup_failed());
    session.start_path(0, 0, None).unwrap();
    let HostPoll::Ready(Ok(value)) = settle(&mut session, 0) else {
        panic!("module remains usable after output failure");
    };
    assert_eq!(
        SourceSession::result_string(value).unwrap().as_str(),
        Some("7")
    );
}

#[test]
fn bootstrap_callback_close_rejects_new_work_and_cleans_terminal_context() {
    let mut session = session(
        "@Deprecated { when(trace(true)) { message=\"setup\" } }\nresult=8\noutput { text=\"8\" }\n",
        &["result"],
        true,
    );
    session.start_output_bytes(0, Some(10)).unwrap();
    let HostPoll::Log { token, .. } = settle(&mut session, 0) else {
        panic!("actual setup logger callback");
    };
    session.close_from_callback(&token).unwrap();
    session.acknowledge_log(&token, Ok(()), 0).unwrap();
    // This is an admission/cleanup assertion, not an output compatibility claim:
    // pinned Java's exact cold-output close source raises a PklBugException.
    // Its full raw comparison remains a separate required unresolved obligation.
    let bytes = match settle(&mut session, 0) {
        HostPoll::Ready(Ok(value)) => Some(SourceSession::result_bytes(value).unwrap()),
        HostPoll::Ready(Err(_)) => None,
        _ => panic!("admitted output must reach a terminal result"),
    };
    let owned = bytes.as_ref().map(|bytes| bytes.to_vec());
    assert_eq!(
        session.start_output_bytes(0, None).unwrap_err().kind,
        ErrorKind::Closed
    );
    assert_eq!(
        session.reenter_path(&token, 0, 0, None).unwrap_err().kind,
        ErrorKind::Closed
    );
    session.collect(&[]);
    assert_eq!(session.context_depth(), 0);
    assert_eq!(session.heap_stats().slots, 0);
    assert_eq!(bytes.as_deref(), owned.as_deref());
}

#[test]
fn output_timeout_cancels_its_read_without_caching_a_setup_failure() {
    let mut session = session("output { text=read(\"life:pending\") }\n", &[], false);
    session.start_output_bytes(0, Some(5)).unwrap();
    let HostPoll::Read { token, .. } = settle(&mut session, 0) else {
        panic!("pending output read");
    };
    let HostPoll::Ready(Err(error)) = settle(&mut session, 5) else {
        panic!("output deadline");
    };
    assert_eq!(error.kind, ErrorKind::Timeout);
    assert!(!session.setup_failed());
    assert_eq!(session.drain_cancelled_reads(), vec![token.clone()]);
    assert!(
        session
            .resume(&token, Ok(Value::string("late")), 5)
            .is_err()
    );
    session.close();
    assert_eq!(session.heap_stats().slots, 0);
}

#[test]
fn cached_setup_failure_replays_without_reentering_the_logger() {
    let mut session = session(
        "@Deprecated { when(trace(throw(\"setup failure\"))) { message=\"never\" } }\noutput { text=\"never\" }\n",
        &[],
        true,
    );
    session.start_output_bytes(0, None).unwrap();
    let HostPoll::Ready(Err(first)) = settle(&mut session, 0) else {
        panic!("setup fails before logging");
    };
    assert!(session.setup_failed());
    let first = session.output_failure(first).stderr;
    session.start_output_bytes(0, None).unwrap();
    let HostPoll::Ready(Err(second)) = settle(&mut session, 0) else {
        panic!("cached setup failure");
    };
    assert_eq!(first, session.output_failure(second).stderr);
}

#[test]
fn shared_output_transport_consumes_configured_reads_and_exports_owned_bytes() {
    let mut probe = SourceProbe::new_output(
        ModuleSourceInput {
            text: "output { text=read(\"prop:configured\") }\n",
            uri: "file:///module-output-session.pkl",
            inferred_name: "module-output-session",
        },
        &[],
        &ModuleEvalOptions {
            properties: vec![("configured".into(), "configured value".into())],
            diagnostic_profile: DiagnosticProfile::ApiIdentity,
            ..Default::default()
        },
        SourceSessionOptions::default(),
    )
    .unwrap();
    probe.start_output_bytes(0, None).unwrap();
    let mut seen = Vec::new();
    let mut completed = None;
    for _ in 0..100_000 {
        let event = probe.poll(0, 1).unwrap();
        probe.collect();
        match event.kind {
            "yield" => {}
            "read" => {
                seen.push(event.uri.unwrap());
                probe.respond_configured(&event.token.unwrap(), 0).unwrap();
            }
            "bytes" => {
                completed = event.bytes;
                break;
            }
            _ => panic!("expected actual byte completion"),
        }
    }
    let bytes = completed.expect("bounded output completion");
    assert!(seen.iter().any(|uri| uri == "prop:configured"));
    assert_eq!(bytes, b"configured value");
    probe.close();
    probe.collect();
    assert_eq!(probe.heap_slots(), 0);
    assert_eq!(bytes, b"configured value");
}

#[test]
fn shared_output_failure_transport_uses_raw_diagnostic_bytes() {
    let mut probe = SourceProbe::new_output(
        ModuleSourceInput {
            text: "output { text=throw(\"output transport error\") }\n",
            uri: "file:///module-output-session.pkl",
            inferred_name: "module-output-session",
        },
        &[],
        &ModuleEvalOptions::default(),
        SourceSessionOptions::default(),
    )
    .unwrap();
    probe.start_output_bytes(0, None).unwrap();
    let mut completed = None;
    for _ in 0..100_000 {
        let event = probe.poll(0, 1).unwrap();
        probe.collect();
        match event.kind {
            "yield" => {}
            "read" => probe.respond_configured(&event.token.unwrap(), 0).unwrap(),
            "error" => {
                assert!(event.bytes.is_none());
                let units = event.error.expect("canonical output diagnostic units");
                let bytes = event.error_bytes.expect("explicit output diagnostic bytes");
                assert_eq!(
                    units,
                    String::from_utf8(bytes.clone())
                        .expect("this original ASCII-message diagnostic is UTF-8")
                        .encode_utf16()
                        .collect::<Vec<_>>()
                );
                completed = Some(bytes);
                break;
            }
            _ => panic!("expected actual output diagnostic"),
        }
    }
    let error = completed.expect("bounded output failure");
    assert!(error.starts_with("–– Pkl Error ––\n".as_bytes()));
    assert!(
        !error
            .windows(b"repl:text".len())
            .any(|part| part == b"repl:text")
    );
    probe.close();
    assert!(!error.is_empty());
}

#[test]
fn output_session_callback_error_retains_its_expression_boundary() {
    let mut probe = SourceProbe::new_output(
        ModuleSourceInput {
            text: "bad=throw(\"callback failure\")\noutput { text=read(\"life:output\") }\n",
            uri: "file:///module-output-session.pkl",
            inferred_name: "module-output-session",
        },
        &["bad".into()],
        &ModuleEvalOptions::default(),
        SourceSessionOptions::default(),
    )
    .unwrap();
    let settle = |probe: &mut SourceProbe| {
        for _ in 0..100_000 {
            let event = probe.poll(0, 1).unwrap();
            probe.collect();
            if event.kind == "yield" {
                continue;
            }
            if event.kind == "read" && event.uri.as_ref().unwrap().starts_with("prop:") {
                probe
                    .respond_configured(event.token.as_ref().unwrap(), 0)
                    .unwrap();
                continue;
            }
            return event;
        }
        panic!("bounded actual output transport");
    };
    probe.start_output_bytes(0, None).unwrap();
    let pending = settle(&mut probe);
    assert_eq!(pending.kind, "read");
    let token = pending.token.unwrap();
    probe.reenter_path(&token, 0, 0, None).unwrap();
    let callback = settle(&mut probe);
    assert_eq!(callback.kind, "error");
    let error = callback.error_bytes.unwrap();
    assert!(
        error
            .windows(b"repl:text".len())
            .any(|part| part == b"repl:text")
    );
    probe
        .respond(&token, Value::string("output continues"), 0)
        .unwrap();
    let output = settle(&mut probe);
    assert_eq!(output.kind, "bytes");
    assert_eq!(output.bytes.unwrap(), b"output continues");
}
