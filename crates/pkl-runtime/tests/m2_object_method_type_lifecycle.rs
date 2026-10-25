//! Rust Host ownership checks, separate from synchronous Java byte authority.
use pkl_runtime::{
    host::{HostPoll, HostSession},
    parser::parse_module,
    resolver::resolve_module,
    value::{ErrorKind, RuntimeError, Value},
    vm::{Instruction as I, compile_module},
};
use std::sync::Arc;

fn event(session: &mut HostSession, roots: &[Value]) -> HostPoll {
    for _ in 0..200_000 {
        session.collect(roots);
        match session.poll(0, 1) {
            HostPoll::Yielded => (),
            event => return event,
        }
    }
    panic!("object method lifecycle exceeded continuation bound")
}
fn ready(session: &mut HostSession, roots: &[Value]) -> Result<Value, RuntimeError> {
    match event(session, roots) {
        HostPoll::Ready(value) => value,
        other => panic!("unexpected event {other:?}"),
    }
}
fn read(session: &mut HostSession, roots: &[Value]) -> String {
    match event(session, roots) {
        HostPoll::Read { token, uri } => {
            assert_eq!(&*uri, "mem:argument");
            token
        }
        other => panic!("signature resolved before its argument: {other:?}"),
    }
}
fn setup() -> (HostSession, Value, [usize; 3]) {
    let source = r#"
foo {
  local function f(x: NonExistingType): Int = x
  local function identity(x: Int): Int = x
  safe = 1
  call = f(read("mem:argument"))
  valid = identity(read("mem:argument"))
}
"#;
    let mut program = (*compile_module(&resolve_module(&parse_module(source).unwrap()).unwrap())
        .unwrap())
    .clone();
    let entries = ["call", "safe", "valid"].map(|name| {
        let block = program.blocks.len();
        program.blocks.push(vec![
            I::Input(0),
            I::GetMember("foo".into()),
            I::GetMember(name.into()),
            I::Return,
        ]);
        block
    });
    let entry = program.entry;
    let mut session = HostSession::new(Arc::new(program));
    session.start(entry, vec![], 0, None).unwrap();
    let module = ready(&mut session, &[]).unwrap();
    (session, module, entries)
}

#[test]
fn arguments_suspend_before_signature_and_failed_attempts_retry_after_gc() {
    let (mut session, module, entries) = setup();
    let roots = std::slice::from_ref(&module);
    session
        .start(entries[0], vec![module.clone()], 0, None)
        .unwrap();
    let first = read(&mut session, roots);
    // Reentry observes the same unforced object without touching its bad method.
    session
        .reenter(&first, entries[1], vec![module.clone()], 0, None)
        .unwrap();
    assert_eq!(ready(&mut session, roots), Ok(Value::Int(1.into())));
    assert_eq!(read(&mut session, roots), first);
    let failure = RuntimeError::new(ErrorKind::Host, "argument completion failed");
    session.resume(&first, Err(failure), 0).unwrap();
    assert_eq!(
        ready(&mut session, roots).unwrap_err().kind,
        ErrorKind::Host
    );

    let mut signature = None;
    for _ in 0..2 {
        session
            .start(entries[0], vec![module.clone()], 0, None)
            .unwrap();
        let token = read(&mut session, roots);
        for _ in 0..4 {
            assert_eq!(read(&mut session, roots), token);
        }
        session.resume(&token, Ok(Value::Int(7.into())), 0).unwrap();
        let error = ready(&mut session, roots).unwrap_err();
        assert_eq!(error.kind, ErrorKind::MissingType);
        if let Some(previous) = &signature {
            assert_eq!(&error, previous, "failed signature must be retryable");
        }
        signature = Some(error);
    }
    // Successful initialization and body invocation retain normal receiver/args.
    session
        .start(entries[2], vec![module.clone()], 0, None)
        .unwrap();
    let token = read(&mut session, roots);
    session.resume(&token, Ok(Value::Int(7.into())), 0).unwrap();
    assert_eq!(ready(&mut session, roots), Ok(Value::Int(7.into())));
    session.close();
    assert_eq!(session.collect(&[]).slots, 0);
}

#[test]
fn cancelling_argument_before_signature_rejects_late_value_and_releases_roots() {
    let (mut session, module, entries) = setup();
    let roots = std::slice::from_ref(&module);
    session
        .start(entries[0], vec![module.clone()], 0, None)
        .unwrap();
    let token = read(&mut session, roots);
    session.cancel();
    assert_eq!(
        ready(&mut session, roots).unwrap_err().kind,
        ErrorKind::Cancelled
    );
    assert_eq!(
        session
            .resume(&token, Ok(Value::Int(1.into())), 0)
            .unwrap_err()
            .kind,
        ErrorKind::Closed
    );
    assert_eq!(
        session
            .start(entries[0], vec![module.clone()], 0, None)
            .unwrap_err()
            .kind,
        ErrorKind::Closed
    );
    session.close();
    assert_eq!(session.collect(&[]).slots, 0);
}
