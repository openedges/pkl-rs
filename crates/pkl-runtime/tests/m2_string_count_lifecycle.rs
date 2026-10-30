//! Internal Host/GC contracts for String code-point Lists and count callbacks. These authored
//! lifecycle assertions are distinct from the Java byte comparisons.
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
        // A terminal value is borrowed by the Host until it is explicitly rooted.
        let mut live = roots.to_vec();
        if let HostPoll::Ready(Ok(value)) = &result {
            live.push(value.clone());
        }
        session.collect(&live);
        if !matches!(result, HostPoll::Yielded) {
            return result;
        }
    }
    panic!("String count Host poll bound")
}
fn ready(session: &mut HostSession, module: &Value) -> Result<Value, RuntimeError> {
    match event(session, std::slice::from_ref(module), 0) {
        HostPoll::Ready(result) => result,
        other => panic!("unexpected {other:?}"),
    }
}
fn read(session: &mut HostSession, module: &Value, uri: &str) -> String {
    match event(session, std::slice::from_ref(module), 0) {
        HostPoll::Read { token, uri: actual } => {
            assert_eq!(actual, uri);
            token
        }
        other => panic!("expected {uri}: {other:?}"),
    }
}
fn resume(session: &mut HostSession, module: &Value, uri: &str, value: Value) {
    let token = read(session, module, uri);
    session.resume(&token, Ok(value), 0).unwrap();
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
    let module = match event(&mut session, &[], 0) {
        HostPoll::Ready(Ok(value)) => value,
        other => panic!("module setup: {other:?}"),
    };
    (session, module, entries)
}

#[test]
fn count_retains_utf16_elements_and_accumulator_through_reentry_gc_and_retry() {
    let source = r#"
hidden items = "A😀\u{d800}".chars
result = items.count((c) -> read("mem:\(c.codePoints[0])"))
parent = read("mem:parent")
nested = read("mem:nested")
"#;
    let (mut session, module, entries) = setup(source, &["parent", "result", "nested"]);
    session
        .start(entries[0], vec![module.clone()], 0, None)
        .unwrap();
    let parent = read(&mut session, &module, "mem:parent");
    session
        .reenter(&parent, entries[1], vec![module.clone()], 0, None)
        .unwrap();
    resume(&mut session, &module, "mem:65", Value::Bool(true));
    let predicate = read(&mut session, &module, "mem:128512");
    session
        .reenter(&predicate, entries[2], vec![module.clone()], 0, None)
        .unwrap();
    resume(&mut session, &module, "mem:nested", Value::string("nested"));
    assert_eq!(ready(&mut session, &module), Ok(Value::string("nested")));
    assert_eq!(read(&mut session, &module, "mem:128512"), predicate);
    session
        .resume(
            &predicate,
            Err(RuntimeError::new(ErrorKind::Host, "retry count")),
            0,
        )
        .unwrap();
    assert_eq!(
        ready(&mut session, &module).unwrap_err().kind,
        ErrorKind::Host
    );
    assert_eq!(read(&mut session, &module, "mem:parent"), parent);
    session
        .reenter(&parent, entries[1], vec![module.clone()], 0, None)
        .unwrap();
    // A new count starts at zero and replays predicates over the retained List.
    for (uri, value) in [("mem:65", true), ("mem:128512", false), ("mem:55296", true)] {
        resume(&mut session, &module, uri, Value::Bool(value));
    }
    assert_eq!(ready(&mut session, &module), Ok(Value::Int(2.into())));
    session.resume(&parent, Ok(Value::Null), 0).unwrap();
    assert_eq!(ready(&mut session, &module), Ok(Value::Null));
    session.close();
    assert_eq!(session.collect(&[]).slots, 0);
}

#[test]
fn count_pending_after_accepted_prefix_cancels_closes_and_expires_without_roots() {
    for reason in [ErrorKind::Cancelled, ErrorKind::Closed, ErrorKind::Timeout] {
        let source = r#"hidden items = "A😀".chars
result = items.count((c) -> read("mem:\(c.codePoints[0])"))"#;
        let (mut session, module, entries) = setup(source, &["result"]);
        session
            .start(entries[0], vec![module.clone()], 0, Some(10))
            .unwrap();
        resume(&mut session, &module, "mem:65", Value::Bool(true));
        let pending = read(&mut session, &module, "mem:128512");
        match reason {
            ErrorKind::Cancelled => session.cancel(),
            ErrorKind::Closed => session.close(),
            ErrorKind::Timeout => (),
            _ => unreachable!(),
        }
        let now = if reason == ErrorKind::Timeout { 11 } else { 1 };
        let HostPoll::Ready(Err(error)) = event(&mut session, &[], now) else {
            panic!("terminal count error");
        };
        assert_eq!(error.kind, reason);
        assert!(
            session
                .resume(&pending, Ok(Value::Bool(true)), now)
                .is_err()
        );
        assert_eq!(
            session
                .start(entries[0], vec![module], now, None)
                .unwrap_err()
                .kind,
            ErrorKind::Closed
        );
        assert_eq!(session.collect(&[]).slots, 0);
    }
}
