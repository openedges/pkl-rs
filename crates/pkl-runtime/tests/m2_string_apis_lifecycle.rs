//! String callback lifecycle checks, distinct from the raw Java oracle regressions.
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
    panic!("String predicate lifecycle step bound")
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
fn member(session: &mut HostSession, module: &Value, entry: usize) -> Value {
    session.start(entry, vec![module.clone()], 0, None).unwrap();
    ready(session, std::slice::from_ref(module)).unwrap()
}
fn read(session: &mut HostSession, module: &Value, uri: &str) -> String {
    match event(session, std::slice::from_ref(module), 0) {
        HostPoll::Read { token, uri: actual } => {
            assert_eq!(&*actual, uri);
            token
        }
        other => panic!("expected {uri}: {other:?}"),
    }
}
fn source(method: &str) -> String {
    format!(
        r#"
hidden makePredicate = () -> (new Dynamic {{
  marker = "yes"
  accept = (ch: String) -> if (read("mem:" + ch) == marker) true else false
}}).accept
result = "A🙂B".{method}(makePredicate.apply())
parent = read("mem:parent")
nested = read("mem:nested")
"#
    )
}
#[test]
// The fresh object returned by makePredicate is not a module property: after
// the factory returns, its callback/frame/stack must retain the captured object.
fn predicate_cursor_retains_capture_utf16_and_short_circuits_across_host_reentry() {
    for (method, first, result) in [
        ("takeWhile", "A", "A"),
        ("dropWhile", "A", "🙂B"),
        ("takeLastWhile", "B", "B"),
        ("dropLastWhile", "B", "A🙂"),
    ] {
        let (mut session, module, entries) =
            setup(&source(method), &["parent", "result", "nested"]);
        session
            .start(entries[0], vec![module.clone()], 0, None)
            .unwrap();
        let parent = read(&mut session, &module, "mem:parent");
        session
            .reenter(&parent, entries[1], vec![module.clone()], 0, None)
            .unwrap();
        let gate = read(&mut session, &module, &format!("mem:{first}"));
        let failure = RuntimeError::new(ErrorKind::Host, "predicate retry\0\r\n🙂");
        session.resume(&gate, Err(failure.clone()), 0).unwrap();
        let actual = ready(&mut session, std::slice::from_ref(&module)).unwrap_err();
        assert_eq!(
            (actual.kind, actual.message),
            (failure.kind, failure.message)
        );
        session
            .reenter(&parent, entries[1], vec![module.clone()], 0, None)
            .unwrap();
        let retry = read(&mut session, &module, &format!("mem:{first}"));
        assert_ne!(gate, retry);
        assert_eq!(
            session.resume(&gate, Ok(Value::Null), 0).unwrap_err().kind,
            ErrorKind::Host
        );
        session.resume(&retry, Ok(Value::string("yes")), 0).unwrap();
        let emoji = read(&mut session, &module, "mem:🙂");
        session
            .reenter(&emoji, entries[2], vec![module.clone()], 0, None)
            .unwrap();
        let nested = read(&mut session, &module, "mem:nested");
        session
            .resume(&nested, Ok(Value::Int(7.into())), 0)
            .unwrap();
        assert_eq!(
            ready(&mut session, std::slice::from_ref(&module)),
            Ok(Value::Int(7.into()))
        );
        assert_eq!(read(&mut session, &module, "mem:🙂"), emoji);
        session.resume(&emoji, Ok(Value::string("no")), 0).unwrap();
        assert_eq!(
            ready(&mut session, std::slice::from_ref(&module)),
            Ok(Value::string(result))
        );
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
fn predicate_pending_capture_is_released_by_all_terminal_transitions() {
    for method in ["takeWhile", "dropWhile", "takeLastWhile", "dropLastWhile"] {
        for terminal in ["cancel", "timeout", "close"] {
            let (mut session, module, entries) = setup(&source(method), &["parent", "result"]);
            session
                .start(entries[0], vec![module.clone()], 0, Some(5))
                .unwrap();
            let parent = read(&mut session, &module, "mem:parent");
            session
                .reenter(&parent, entries[1], vec![module.clone()], 0, None)
                .unwrap();
            let first = if method.contains("Last") { "B" } else { "A" };
            let gate = read(&mut session, &module, &format!("mem:{first}"));
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
            let HostPoll::Ready(Err(error)) = event(&mut session, &[], 6) else {
                panic!("expected {terminal}")
            };
            assert_eq!(error.kind, expected);
            assert_eq!(session.context_depth(), 0);
            assert_eq!(session.heap_stats().slots, 0);
            assert_eq!(session.drain_cancelled_reads(), [parent, gate]);
        }
    }
}
#[test]
fn empty_predicates_check_function_arity_without_forcing_body() {
    let (mut session, module, entries) = setup(
        r#"
a = "".takeWhile((ch) -> throw("unused"))
b = "".dropLastWhile((ch) -> read("mem:unused"))
c = "".takeWhile((x,y) -> true)
"#,
        &["a", "b", "c"],
    );
    assert_eq!(member(&mut session, &module, entries[0]), Value::string(""));
    assert_eq!(member(&mut session, &module, entries[1]), Value::string(""));
    session
        .start(entries[2], vec![module.clone()], 0, None)
        .unwrap();
    assert_eq!(
        ready(&mut session, std::slice::from_ref(&module))
            .unwrap_err()
            .kind,
        ErrorKind::Type
    );
    session.close();
    assert_eq!(session.collect(&[]).slots, 0);
}
