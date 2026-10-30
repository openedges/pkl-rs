//! Mapping operation lifecycle checks, distinct from the raw Java oracle regressions.
use pkl_runtime::{
    host::{HostPoll, HostSession},
    parser::parse_module,
    resolver::resolve_module,
    value::{ErrorKind, RuntimeError, Value},
    vm::{Instruction as I, compile_module},
};
use std::sync::Arc;

fn event(session: &mut HostSession, roots: &[Value], now: u64) -> HostPoll {
    for _ in 0..200_000 {
        session.collect(roots);
        let result = session.poll(now, 1);
        let mut live = roots.to_vec();
        if let HostPoll::Ready(Ok(value)) = &result {
            live.push(value.clone());
        }
        session.collect(&live);
        if !matches!(result, HostPoll::Yielded) {
            return result;
        }
    }
    panic!("Mapping operation lifecycle step bound")
}
fn ready(session: &mut HostSession, roots: &[Value]) -> Result<Value, RuntimeError> {
    match event(session, roots, 0) {
        HostPoll::Ready(result) => result,
        other => panic!("unexpected {other:?}"),
    }
}
fn setup(source: &str, names: &[&str]) -> (HostSession, Value, Vec<usize>) {
    let mut program = (*compile_module(&resolve_module(&parse_module(source).unwrap()).unwrap())
        .unwrap())
    .clone();
    let entries = names
        .iter()
        .map(|name| {
            let block = program.blocks.len();
            program
                .blocks
                .push(vec![I::Input(0), I::GetMember((*name).into()), I::Return]);
            block
        })
        .collect();
    let entry = program.entry;
    let mut session = HostSession::new(Arc::new(program));
    session.start(entry, vec![], 0, None).unwrap();
    let module = ready(&mut session, &[]).unwrap();
    (session, module, entries)
}
fn read(
    session: &mut HostSession,
    module: &Value,
    uri: impl Into<pkl_runtime::value::PklString>,
) -> String {
    let uri = uri.into();
    match event(session, std::slice::from_ref(module), 0) {
        HostPoll::Read { token, uri: actual } => {
            assert_eq!(actual, uri);
            token
        }
        other => panic!("expected {uri}: {other:?}"),
    }
}

fn pending_read(
    session: &mut HostSession,
    module: &Value,
    method: &str,
    phase: &str,
) -> (String, pkl_runtime::value::PklString) {
    match event(session, std::slice::from_ref(module), 0) {
        HostPoll::Read { token, uri } => (token, uri),
        other => panic!("{method} {phase}: expected a pending Host read, got {other:?}"),
    }
}

const CASES: &[&str] = &[
    "keys",
    "containsKey",
    "getOrNull",
    "getOrDefault",
    "fold",
    "probe",
    "probe-subscript",
    "probe-parent",
    "cache-probe",
    "cache-steal",
];
fn source(method: &str) -> String {
    let body = match method {
        "keys" => {
            r#"
hidden make = () -> (new Mapping {
  [new Dynamic { ["x"] = read("mem:parent-key") }] = throw("parent value must stay lazy")
}) {
  [new Dynamic { ["x"] = read("mem:child-key") }] = throw("child value must stay lazy")
}
result = make.apply().keys.toString()
"#
        }
        "containsKey" => {
            r#"
hidden make = () -> new Dynamic { ["x"] = read("mem:query") }
result = (new Mapping { [new Dynamic { ["x"] = "yes" }] = throw("value must stay lazy") }).containsKey(make.apply()).toString()
"#
        }
        "getOrNull" => {
            r#"
hidden make = () -> (new Dynamic {
  base = new Mapping { [1] = read("mem:value") }
  local typed: Mapping<Int, String> = base
  value = typed
}).value
result = make.apply().getOrNull(1).toString()
"#
        }
        "getOrDefault" => {
            r#"
hidden make = () -> new Mapping {
  default = (k) -> read("mem:default-" + k.toString())
  [1] = throw("unselected value")
}
result = make.apply().getOrDefault(9).toString()
"#
        }
        "fold" => {
            r#"
hidden make = () -> (new Dynamic {
  marker = "yes"
  callback = (a,k,v) -> if (read("mem:callback-" + k.toString()) == marker) List(a,k,v) else a
}).callback
result = (new Mapping { [31] = read("mem:value-31")
  [0] = read("mem:value-0") }).fold(new Dynamic { initial = 42 },make.apply()).toString()
"#
        }
        "probe" => {
            r#"
hidden make = () -> new Dynamic { ["x"] = "yes" }
result = (new Mapping {
  default = (key) -> if (read("mem:probe") == "yes") new Dynamic { captured = key } else throw("response")
  [new Dynamic { ["x"] = "yes" }] {}
}).getOrNull(make.apply()).captured.toString()
"#
        }
        "probe-subscript" => {
            r#"
hidden make = () -> new Dynamic { ["x"] = "yes" }
result = (new Dynamic {
  default = (key) -> if (read("mem:probe") == "yes") new Dynamic { captured = key } else throw("response")
  [new Dynamic { ["x"] = "yes" }] {}
})[make.apply()].captured.toString()
"#
        }
        "probe-parent" => {
            r#"
hidden make = () -> new Dynamic { ["x"] = "yes" }
result = ((new Mapping {
  default = (key) -> if (read("mem:probe") == "yes") new Dynamic { captured = key } else throw("response")
  [new Dynamic { ["x"] = "yes" }] {}
}) { [new Dynamic { ["x"] = "yes" }] { extra = 1 } })[make.apply()].captured.toString()
"#
        }
        "cache-probe" => {
            r#"
hidden values = new Mapping { [new Dynamic { ["x"] = "yes" }] = "stored" }
local function warm() = values.getOrNull(new Dynamic { ["x"] = read("mem:key") })
result = if (warm() == "stored" && read("mem:after") == "yes") values.containsKey(new Dynamic { ["x"] = "yes" }).toString() else "bad"
"#
        }
        "cache-steal" => {
            r#"
hidden raw = new Mapping { [1] = "yes" }
hidden cast: Mapping<Int,String(read("mem:cast") == "yes")> = raw
result = if (raw.getOrNull(1) == "yes") cast.getOrNull(1) else "bad"
"#
        }
        _ => unreachable!(),
    };
    format!("{body}\nparent = read(\"mem:parent\")\nnested = read(\"mem:nested\")\n")
}
fn finish(
    session: &mut HostSession,
    module: &Value,
    reads: &mut Vec<String>,
    method: &str,
) -> Value {
    loop {
        match event(session, std::slice::from_ref(module), 0) {
            HostPoll::Read { token, uri } => {
                reads.push(uri.to_string());
                session.resume(&token, Ok(Value::string("yes")), 0).unwrap();
            }
            HostPoll::Ready(result) => {
                return result
                    .unwrap_or_else(|error| panic!("{method} completion failed: {error:?}"));
            }
            other => panic!("unexpected callback event {other:?}"),
        }
    }
}
#[test]
fn callback_and_nested_builder_roots_survive_host_failure_retry_and_reentry() {
    for &method in CASES {
        let text = source(method);
        // This comparison tests lifecycle invariance, not Java compatibility.
        // Raw Java fixtures separately determine method semantics and diagnostics.
        let (mut baseline, module, entries) = setup(&text, &["result"]);
        baseline
            .start(entries[0], vec![module.clone()], 0, None)
            .unwrap();
        let mut expected_reads = Vec::new();
        let expected = finish(&mut baseline, &module, &mut expected_reads, method);
        assert!(
            !expected_reads.is_empty(),
            "{method}: baseline must exercise a real Host suspension"
        );
        baseline.close();
        assert_eq!(baseline.collect(&[]).slots, 0);
        let (mut session, module, entries) = setup(&text, &["parent", "result", "nested"]);
        session
            .start(entries[0], vec![module.clone()], 0, None)
            .unwrap();
        let parent = read(&mut session, &module, "mem:parent");
        session
            .reenter(&parent, entries[1], vec![module.clone()], 0, None)
            .unwrap();
        let (failed, uri) = pending_read(&mut session, &module, method, "first callback");
        assert_eq!(uri, expected_reads[0]);
        let failure = RuntimeError::new(ErrorKind::Host, "Mapping operation retry\0\r\n🙂");
        session.resume(&failed, Err(failure.clone()), 0).unwrap();
        let actual = ready(&mut session, std::slice::from_ref(&module)).unwrap_err();
        assert_eq!(
            (actual.kind, actual.message),
            (failure.kind, failure.message)
        );
        session
            .reenter(&parent, entries[1], vec![module.clone()], 0, None)
            .unwrap();
        let (retry, uri) = pending_read(&mut session, &module, method, "retry callback");
        assert_ne!(failed, retry);
        assert_eq!(
            session
                .resume(&failed, Ok(Value::Null), 0)
                .unwrap_err()
                .kind,
            ErrorKind::Host
        );
        session
            .reenter(&retry, entries[2], vec![module.clone()], 0, None)
            .unwrap();
        let nested = read(&mut session, &module, "mem:nested");
        session
            .resume(&nested, Ok(Value::Int(7.into())), 0)
            .unwrap();
        assert_eq!(
            ready(&mut session, std::slice::from_ref(&module)),
            Ok(Value::Int(7.into()))
        );
        assert_eq!(read(&mut session, &module, &uri), retry);
        let mut actual_reads = vec![uri.to_string()];
        session.resume(&retry, Ok(Value::string("yes")), 0).unwrap();
        assert_eq!(
            finish(&mut session, &module, &mut actual_reads, method),
            expected
        );
        assert_eq!(actual_reads, expected_reads);
        session.resume(&parent, Ok(Value::Null), 0).unwrap();
        assert_eq!(
            ready(&mut session, std::slice::from_ref(&module)),
            Ok(Value::Null)
        );
        session.close();
        assert_eq!(session.collect(&[]).slots, 0);
    }
}
#[test]
fn pending_mapping_read_operations_and_builders_are_released_on_cancel_timeout_and_close() {
    for &method in CASES {
        for terminal in ["cancel", "timeout", "close"] {
            let (mut session, module, entries) = setup(&source(method), &["parent", "result"]);
            session
                .start(entries[0], vec![module.clone()], 0, Some(5))
                .unwrap();
            let parent = read(&mut session, &module, "mem:parent");
            session
                .reenter(&parent, entries[1], vec![module.clone()], 0, None)
                .unwrap();
            let (pending, _) = pending_read(&mut session, &module, method, terminal);
            let expected = match terminal {
                "cancel" => {
                    session.cancel();
                    ErrorKind::Cancelled
                }
                "close" => {
                    session.close();
                    ErrorKind::Closed
                }
                _ => ErrorKind::Timeout,
            };
            let HostPoll::Ready(Err(actual)) = event(&mut session, &[], 6) else {
                panic!("expected terminal state")
            };
            assert_eq!(actual.kind, expected);
            assert_eq!(session.context_depth(), 0);
            assert_eq!(session.heap_stats().slots, 0);
            assert_eq!(session.drain_cancelled_reads(), [parent, pending]);
        }
    }
}

#[test]
fn stolen_cache_cast_error_survives_gc_reentry_and_retry_without_publication() {
    for text in [
        r#"
hidden raw = new Mapping { [1] = "stored" }
hidden typed: Mapping<Int,String(read("mem:cast") == "accept")> = raw
result = if (raw.getOrNull(1) == "stored") typed.getOrNull(1) else "bad"
nested = read("mem:nested")
"#,
        r#"
hidden raw = new Mapping { [1] = 1
  [2] = 2 }
hidden typed: Mapping<Int,Int(if (this == 1) typed.getOrNull(2) > 0 else read("mem:cast") == "accept")> = raw
result = if (raw.getOrNull(1) == 1 && raw.getOrNull(2) == 2) typed.getOrNull(1) else -1
nested = read("mem:nested")
"#,
    ] {
        let (mut baseline, module, entries) = setup(text, &["result"]);
        baseline
            .start(entries[0], vec![module.clone()], 0, None)
            .unwrap();
        let token = read(&mut baseline, &module, "mem:cast");
        baseline
            .resume(&token, Ok(Value::string("reject")), 0)
            .unwrap();
        let expected = ready(&mut baseline, std::slice::from_ref(&module)).unwrap_err();
        assert_eq!(expected.kind, ErrorKind::Type);
        baseline.close();
        assert_eq!(baseline.collect(&[]).slots, 0);

        let (mut session, module, entries) = setup(text, &["result", "nested"]);
        for attempt in 0..2 {
            session
                .start(entries[0], vec![module.clone()], 0, None)
                .unwrap();
            let cast = read(&mut session, &module, "mem:cast");
            if attempt == 0 {
                session
                    .reenter(&cast, entries[1], vec![module.clone()], 0, None)
                    .unwrap();
                let nested = read(&mut session, &module, "mem:nested");
                session
                    .resume(&nested, Ok(Value::Int(7.into())), 0)
                    .unwrap();
                assert_eq!(
                    ready(&mut session, std::slice::from_ref(&module)),
                    Ok(Value::Int(7.into()))
                );
                assert_eq!(read(&mut session, &module, "mem:cast"), cast);
            }
            session
                .resume(&cast, Ok(Value::string("reject")), 0)
                .unwrap();
            // Polling collects after every VM instruction, including the retained
            // exception's declaration search. This is lifecycle invariance, not a
            // Java-derived diagnostic expectation.
            let actual = ready(&mut session, std::slice::from_ref(&module)).unwrap_err();
            assert_eq!(actual, expected);
        }
        session.close();
        assert_eq!(session.collect(&[]).slots, 0);
    }
}

#[test]
fn missing_mapping_subscript_forces_object_key_before_missing_key_diagnostic() {
    let source =
        include_str!("../../../tests/fixtures/m2/equality-supplement-key-lookup-force-error.pkl");
    let (mut session, module, entries) = setup(source, &["result"]);
    session
        .start(entries[0], vec![module.clone()], 0, None)
        .unwrap();
    let error = ready(&mut session, std::slice::from_ref(&module)).unwrap_err();
    assert_eq!(error.message, "lookup key");
    session.close();
    assert_eq!(session.collect(&[]).slots, 0);
}

#[test]
fn stolen_mapping_constraint_error_does_not_add_subscript_guest_frame() {
    let source =
        include_str!("../../../tests/fixtures/m2/collection-type-mapping-cache-steal-fail.pkl");
    let (mut session, module, entries) = setup(source, &["result"]);
    session
        .start(entries[0], vec![module.clone()], 0, None)
        .unwrap();
    let error = ready(&mut session, std::slice::from_ref(&module)).unwrap_err();
    assert_eq!(error.kind, ErrorKind::Type);
    assert!(
        error
            .frames
            .iter()
            .any(|frame| frame.member.ends_with("#x"))
    );
    assert!(
        error
            .frames
            .iter()
            .any(|frame| frame.member.ends_with("#source[\"k\"]"))
    );
    assert!(
        !error
            .frames
            .iter()
            .any(|frame| frame.member.ends_with("#result"))
    );
    session.close();
    assert_eq!(session.collect(&[]).slots, 0);
}
