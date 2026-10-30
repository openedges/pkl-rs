//! List callback lifecycle checks, distinct from the raw Java oracle regressions.
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
    panic!("List callback lifecycle step bound")
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

const CASES: &[(&str, &str)] = &[
    (
        "findLastIndex",
        "(x) -> read(\"mem:\" + x.toString()) == marker",
    ),
    (
        "filterIndexed",
        "(i,x) -> read(\"mem:\" + x.toString()) == marker && i % 2 == 0",
    ),
    (
        "distinctBy",
        "(x) -> new Dynamic { [\"x\"] = if (read(\"mem:\" + x.toString()) == marker) x else 0 }",
    ),
    ("filter", "(x) -> read(\"mem:\" + x.toString()) == marker"),
    (
        "sortBy",
        "(x) -> if (read(\"mem:\" + x.toString()) == marker) x else 0",
    ),
    (
        "sortWith",
        "(a,b) -> if (read(\"mem:\" + a.toString()) == marker) a < b else false",
    ),
    (
        "fold",
        "(a,x) -> if (read(\"mem:\" + x.toString()) == marker) a + x else a",
    ),
    (
        "groupBy",
        "(x) -> if (read(\"mem:\" + x.toString()) == marker) 0 else 1",
    ),
    (
        "flatMap",
        // Rendering a lazy Dynamic only prints an unforced entry as `?`.
        // Force that entry inside the callback while its nested List stays live.
        "(x) -> List(new Dynamic { [\"x\"] = if (read(\"mem:\" + x.toString()) == marker) x else 0 }).map((item) -> item[\"x\"])",
    ),
];
fn source(method: &str, callback: &str) -> String {
    let initial = if method == "fold" { "0, " } else { "" };
    format!(
        r#"
hidden make = () -> (new Dynamic {{
  marker = "yes"
  apply = {callback}
}}).apply
result = List(3, 1, 2, 4, 5, 6).{method}({initial}make.apply()).toString()
parent = read("mem:parent")
nested = read("mem:nested")
"#
    )
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
    for &(method, callback) in CASES {
        let text = source(method, callback);
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
        let failure = RuntimeError::new(ErrorKind::Host, "List callback retry\0\r\n🙂");
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
fn pending_list_callbacks_and_builders_are_released_on_cancel_timeout_and_close() {
    for &(method, callback) in CASES {
        for terminal in ["cancel", "timeout", "close"] {
            let (mut session, module, entries) =
                setup(&source(method, callback), &["parent", "result"]);
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
