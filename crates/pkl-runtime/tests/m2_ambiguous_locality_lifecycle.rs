//! Rust Host scheduling/ownership controls; Java public bytes are tested separately.
use pkl_runtime::{
    host::{HostPoll, HostSession},
    parser::parse_module,
    resolver::resolve_module,
    value::{ErrorKind, RuntimeError, Value},
    vm::{Instruction as I, compile_module},
};
use std::sync::Arc;

const SOURCE: &str = r#"
hidden proto {
  flag = true
  child {
    when (flag) { local x = read("mem:local") }
    else { x = read("mem:public") }
    answer = x
    nested { answer = x }
  }
}
a = (proto) { flag = true }
b = (proto) { flag = false }
first = a.child.answer
second = b.child.answer
nestedFirst = a.child.nested.answer
nestedSecond = b.child.nested.answer
stable = 7
"#;

fn setup(source: &str, names: &[&str]) -> (HostSession, Value, Vec<usize>) {
    let mut program = (*compile_module(&resolve_module(&parse_module(source).unwrap()).unwrap())
        .unwrap())
    .clone();
    let entries = names
        .iter()
        .map(|name| {
            let entry = program.blocks.len();
            program
                .blocks
                .push(vec![I::Input(0), I::GetMember((*name).into()), I::Return]);
            entry
        })
        .collect();
    let main = program.entry;
    let mut session = HostSession::new(Arc::new(program));
    session.start(main, vec![], 0, None).unwrap();
    let module = ready(&mut session, &[]).unwrap();
    (session, module, entries)
}
fn next(session: &mut HostSession, roots: &[Value]) -> HostPoll {
    for _ in 0..200_000 {
        session.collect(roots);
        match session.poll(0, 1) {
            HostPoll::Yielded => {}
            event => return event,
        }
    }
    panic!("bounded ambiguous locality continuation");
}
fn ready(session: &mut HostSession, roots: &[Value]) -> Result<Value, RuntimeError> {
    match next(session, roots) {
        HostPoll::Ready(value) => value,
        event => panic!("unexpected event {event:?}"),
    }
}
fn read(session: &mut HostSession, roots: &[Value], expected: &str) -> String {
    match next(session, roots) {
        HostPoll::Read { token, uri } => {
            assert_eq!(uri, expected);
            token
        }
        event => panic!("unexpected event {event:?}"),
    }
}
fn start(session: &mut HostSession, entry: usize, module: &Value) {
    session.start(entry, vec![module.clone()], 0, None).unwrap();
}

#[test]
fn first_local_selection_survives_failed_value_gc_and_retry_on_another_receiver() {
    let (mut session, module, entries) = setup(SOURCE, &["first", "second", "stable"]);
    let roots = std::slice::from_ref(&module);
    start(&mut session, entries[0], &module);
    let failed = read(&mut session, roots, "mem:local");
    session
        .reenter(&failed, entries[2], vec![module.clone()], 0, None)
        .unwrap();
    assert_eq!(ready(&mut session, roots), Ok(Value::Int(7.into())));
    assert_eq!(read(&mut session, roots, "mem:local"), failed);
    session
        .resume(&failed, Err(RuntimeError::new(ErrorKind::Host, "retry")), 0)
        .unwrap();
    assert_eq!(
        ready(&mut session, roots).unwrap_err().kind,
        ErrorKind::Host
    );
    start(&mut session, entries[1], &module);
    let retry = read(&mut session, roots, "mem:local");
    session
        .resume(&retry, Ok(Value::string("from local declaration")), 0)
        .unwrap();
    assert_eq!(
        ready(&mut session, roots),
        Ok(Value::string("from local declaration"))
    );
    session.close();
    assert_eq!(session.collect(&[]).slots, 0);
}

#[test]
fn public_selection_is_not_changed_after_failure_and_fresh_evaluator_has_own_choice() {
    let (mut session, module, entries) = setup(SOURCE, &["first", "second"]);
    let roots = std::slice::from_ref(&module);
    start(&mut session, entries[1], &module);
    let failed = read(&mut session, roots, "mem:public");
    session
        .resume(&failed, Err(RuntimeError::new(ErrorKind::Host, "retry")), 0)
        .unwrap();
    assert_eq!(
        ready(&mut session, roots).unwrap_err().kind,
        ErrorKind::Host
    );
    start(&mut session, entries[0], &module);
    // The first public specialization survives failure. No local Host read occurs.
    assert_eq!(
        ready(&mut session, roots).unwrap_err().kind,
        ErrorKind::MissingMember
    );
    start(&mut session, entries[1], &module);
    let retry = read(&mut session, roots, "mem:public");
    session
        .resume(&retry, Ok(Value::string("public")), 0)
        .unwrap();
    assert_eq!(ready(&mut session, roots), Ok(Value::string("public")));
    session.close();
    assert_eq!(session.collect(&[]).slots, 0);
    let (mut fresh, module, entries) = setup(SOURCE, &["first"]);
    start(&mut fresh, entries[0], &module);
    let token = read(&mut fresh, std::slice::from_ref(&module), "mem:local");
    fresh.resume(&token, Ok(Value::string("fresh")), 0).unwrap();
    assert_eq!(
        ready(&mut fresh, std::slice::from_ref(&module)),
        Ok(Value::string("fresh"))
    );
    fresh.close();
    assert_eq!(fresh.collect(&[]).slots, 0);
}

#[test]
fn nested_lexical_site_is_independent_from_public_first_outer_site() {
    let (mut session, module, entries) = setup(SOURCE, &["second", "nestedFirst", "nestedSecond"]);
    let roots = std::slice::from_ref(&module);
    start(&mut session, entries[0], &module);
    let public = read(&mut session, roots, "mem:public");
    session
        .resume(&public, Ok(Value::string("public")), 0)
        .unwrap();
    assert_eq!(ready(&mut session, roots), Ok(Value::string("public")));
    for &entry in &entries[1..] {
        start(&mut session, entry, &module);
        let local = read(&mut session, roots, "mem:local");
        session
            .resume(&local, Ok(Value::string("local")), 0)
            .unwrap();
        assert_eq!(ready(&mut session, roots), Ok(Value::string("local")));
    }
    session.close();
    assert_eq!(session.collect(&[]).slots, 0);
}

#[test]
fn pending_local_selection_cancel_or_close_releases_runtime_roots() {
    for close in [false, true] {
        let (mut session, module, entries) = setup(SOURCE, &["first"]);
        start(&mut session, entries[0], &module);
        let token = read(&mut session, std::slice::from_ref(&module), "mem:local");
        if close {
            session.close();
        } else {
            session.cancel();
        }
        assert_eq!(
            ready(&mut session, &[]).unwrap_err().kind,
            if close {
                ErrorKind::Closed
            } else {
                ErrorKind::Cancelled
            }
        );
        assert!(
            session
                .resume(&token, Ok(Value::string("late")), 0)
                .is_err()
        );
        assert_eq!(session.collect(&[]).slots, 0);
    }
}
