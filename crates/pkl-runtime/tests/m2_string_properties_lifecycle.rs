//! Host ownership and evaluation order; Java byte comparisons are separate.
use pkl_runtime::{
    host::{HostPoll, HostSession},
    parser::parse_module,
    resolver::resolve_module,
    value::{ErrorKind, PklString, RuntimeError, Value},
    vm::{Instruction as I, compile_module},
};
use std::sync::Arc;

fn event(session: &mut HostSession) -> HostPoll {
    event_with_roots(session, &[])
}

fn event_with_roots(session: &mut HostSession, roots: &[Value]) -> HostPoll {
    for _ in 0..200_000 {
        session.collect(roots);
        match session.poll(0, 1) {
            HostPoll::Yielded => {}
            event => return event,
        }
    }
    panic!("blankness Host poll bound")
}

fn ready(session: &mut HostSession) -> Result<Value, RuntimeError> {
    ready_with_roots(session, &[])
}

fn ready_with_roots(session: &mut HostSession, roots: &[Value]) -> Result<Value, RuntimeError> {
    match event_with_roots(session, roots) {
        HostPoll::Ready(value) => value,
        event => panic!("unexpected {event:?}"),
    }
}

fn read(session: &mut HostSession, expected: &str) -> String {
    match event(session) {
        HostPoll::Read { token, uri } => {
            assert_eq!(&*uri, expected);
            token
        }
        event => panic!("unexpected {event:?}"),
    }
}

fn setup() -> (HostSession, Value, Vec<usize>) {
    let source = r#"
text = read("mem:text")
blank = text.isBlank
nonblank = text.isNotBlank
checked = text as String(isBlank && isNotEmpty && lastIndex == length - 1)
other = read("mem:other").isNotBlank
missingMethod = text.isBlank(read("mem:argument-must-not-run"))
indexed = read("mem:indices") as String(length == 4 && lastIndex == 3 && !isEmpty)
"#;
    let mut program = (*compile_module(&resolve_module(&parse_module(source).unwrap()).unwrap())
        .unwrap())
    .clone();
    let entries = [
        "blank",
        "nonblank",
        "checked",
        "other",
        "missingMethod",
        "indexed",
    ]
    .into_iter()
    .map(|name| {
        let entry = program.blocks.len();
        program
            .blocks
            .push(vec![I::Input(0), I::GetMember(name.into()), I::Return]);
        entry
    })
    .collect();
    let entry = program.entry;
    let mut session = HostSession::new(Arc::new(program));
    session.start(entry, vec![], 0, None).unwrap();
    let module = ready(&mut session).unwrap();
    (session, module, entries)
}

#[test]
fn receiver_retry_reentry_and_implicit_constraint_preserve_string_units() {
    let (mut session, module, entries) = setup();
    session
        .start(entries[0], vec![module.clone()], 0, None)
        .unwrap();
    let text = read(&mut session, "mem:text");
    session
        .reenter(&text, entries[3], vec![module.clone()], 0, None)
        .unwrap();
    let other = read(&mut session, "mem:other");
    session
        .resume(
            &other,
            Ok(Value::String(PklString::from_utf16(vec![0xd800]))),
            0,
        )
        .unwrap();
    assert_eq!(ready(&mut session), Ok(Value::Bool(true)));
    assert_eq!(read(&mut session, "mem:text"), text);
    session
        .resume(
            &text,
            Err(RuntimeError::new(ErrorKind::Host, "retry blank receiver")),
            0,
        )
        .unwrap();
    // A failed Host completion has already unwound the VM activation. The
    // caller still owns this module handle for retry and must trace it.
    assert_eq!(
        ready_with_roots(&mut session, std::slice::from_ref(&module))
            .unwrap_err()
            .kind,
        ErrorKind::Host
    );

    session
        .start(entries[0], vec![module.clone()], 0, None)
        .unwrap();
    let text = read(&mut session, "mem:text");
    let whitespace = Value::string("\u{85}\u{a0}\u{2007}\u{202f}");
    session.resume(&text, Ok(whitespace.clone()), 0).unwrap();
    assert_eq!(ready(&mut session), Ok(Value::Bool(true)));
    session
        .start(entries[1], vec![module.clone()], 0, None)
        .unwrap();
    assert_eq!(ready(&mut session), Ok(Value::Bool(false)));
    session
        .start(entries[2], vec![module.clone()], 0, None)
        .unwrap();
    assert_eq!(ready(&mut session), Ok(whitespace));
    session
        .start(entries[4], vec![module.clone()], 0, None)
        .unwrap();
    // Method lookup fails before its argument can issue a Host request.
    assert_eq!(
        ready(&mut session).unwrap_err().kind,
        ErrorKind::MissingMember
    );
    session.start(entries[5], vec![module], 0, None).unwrap();
    let indexed = read(&mut session, "mem:indices");
    let units = Value::String(PklString::from_utf16(vec![
        0xd800, 0xdc00, 0xd800, 0x41, 0xdc00,
    ]));
    session.resume(&indexed, Ok(units.clone()), 0).unwrap();
    assert_eq!(ready(&mut session), Ok(units));
    session.close();
    assert_eq!(session.collect(&[]).slots, 0);
}

#[test]
fn cancelled_receiver_read_rejects_late_completion_and_releases_roots() {
    let (mut session, module, entries) = setup();
    session.start(entries[0], vec![module], 0, None).unwrap();
    let text = read(&mut session, "mem:text");
    session.cancel();
    assert_eq!(ready(&mut session).unwrap_err().kind, ErrorKind::Cancelled);
    assert_eq!(
        session
            .resume(&text, Ok(Value::string(" ")), 0)
            .unwrap_err()
            .kind,
        ErrorKind::Closed
    );
    assert_eq!(session.collect(&[]).slots, 0);
}
