//! Host/GC continuation checks; Java compatibility is verified separately.
use pkl_runtime::{
    host::{HostPoll, HostSession},
    parser::parse_module,
    resolver::resolve_module,
    value::{ErrorKind, RuntimeError, Value},
    vm::{Instruction as I, compile_module},
};
use std::sync::Arc;

fn event(session: &mut HostSession) -> HostPoll {
    for _ in 0..200_000 {
        session.collect(&[]);
        match session.poll(0, 1) {
            HostPoll::Yielded => {}
            event => return event,
        }
    }
    panic!("reversal Host poll bound")
}
fn ready(session: &mut HostSession) -> Result<Value, RuntimeError> {
    match event(session) {
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
class Item { value: String }
outerValue = read("mem:outer")
items = List(new Item { value = read("mem:first") }, new Item { value = read("mem:last") })
reversed = items.reverse()
lastValue = reversed[0].value
firstValue = reversed[1].value
text = read("mem:text").reverse()
callback = Set((x) -> read("mem:set-first"), (x) -> read("mem:set-last")).reverse()[0].apply(0)
checked = items as List(reverse()[0].value == List(read("mem:expected"))[0])
"#;
    let mut program = (*compile_module(&resolve_module(&parse_module(source).unwrap()).unwrap())
        .unwrap())
    .clone();
    let entries = [
        "outerValue",
        "reversed",
        "lastValue",
        "firstValue",
        "text",
        "callback",
        "checked",
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
fn reversed_lazy_elements_survive_host_failure_retry_and_reentry() {
    let (mut session, module, entries) = setup();
    session
        .start(entries[1], vec![module.clone()], 0, None)
        .unwrap();
    ready(&mut session).unwrap(); // Neither nested Item property is forced.
    session
        .start(entries[0], vec![module.clone()], 0, None)
        .unwrap();
    let outer = read(&mut session, "mem:outer");
    session
        .reenter(&outer, entries[2], vec![module.clone()], 0, None)
        .unwrap();
    let last = read(&mut session, "mem:last");
    session
        .reenter(&last, entries[5], vec![module.clone()], 0, None)
        .unwrap();
    let callback = read(&mut session, "mem:set-last");
    session
        .resume(&callback, Ok(Value::string("callback")), 0)
        .unwrap();
    assert_eq!(ready(&mut session), Ok(Value::string("callback")));
    assert_eq!(read(&mut session, "mem:last"), last);
    session
        .resume(
            &last,
            Err(RuntimeError::new(ErrorKind::Host, "retry reversed element")),
            0,
        )
        .unwrap();
    assert_eq!(ready(&mut session).unwrap_err().kind, ErrorKind::Host);
    assert_eq!(read(&mut session, "mem:outer"), outer);
    session
        .reenter(&outer, entries[2], vec![module.clone()], 0, None)
        .unwrap();
    let last = read(&mut session, "mem:last");
    session.resume(&last, Ok(Value::string("last")), 0).unwrap();
    assert_eq!(ready(&mut session), Ok(Value::string("last")));
    assert_eq!(read(&mut session, "mem:outer"), outer);
    session
        .reenter(&outer, entries[3], vec![module], 0, None)
        .unwrap();
    let first = read(&mut session, "mem:first");
    session
        .resume(&first, Ok(Value::string("first")), 0)
        .unwrap();
    assert_eq!(ready(&mut session), Ok(Value::string("first")));
    assert_eq!(read(&mut session, "mem:outer"), outer);
    session.resume(&outer, Ok(Value::Null), 0).unwrap();
    assert_eq!(ready(&mut session), Ok(Value::Null));
    session.close();
    assert_eq!(session.collect(&[]).slots, 0);
}

#[test]
fn string_reverse_host_input_and_cancelled_element_read_keep_lifetimes_separate() {
    let (mut session, module, entries) = setup();
    session
        .start(entries[4], vec![module.clone()], 0, None)
        .unwrap();
    let text = read(&mut session, "mem:text");
    session.resume(&text, Ok(Value::string("a😀b")), 0).unwrap();
    assert_eq!(ready(&mut session), Ok(Value::string("b😀a")));
    session.start(entries[2], vec![module], 0, None).unwrap();
    let last = read(&mut session, "mem:last");
    session.cancel();
    assert_eq!(ready(&mut session).unwrap_err().kind, ErrorKind::Cancelled);
    assert_eq!(
        session.resume(&last, Ok(Value::Null), 0).unwrap_err().kind,
        ErrorKind::Closed
    );
    assert_eq!(session.collect(&[]).slots, 0);
}

#[test]
fn constraint_receiver_survives_nested_reentry_and_constructor_argument_retry() {
    let (mut session, module, entries) = setup();
    session
        .start(entries[6], vec![module.clone()], 0, None)
        .unwrap();
    let last = read(&mut session, "mem:last");
    session
        .reenter(&last, entries[4], vec![module.clone()], 0, None)
        .unwrap();
    let text = read(&mut session, "mem:text");
    session.resume(&text, Ok(Value::string("ab")), 0).unwrap();
    assert_eq!(ready(&mut session), Ok(Value::string("ba")));
    assert_eq!(read(&mut session, "mem:last"), last);
    session.resume(&last, Ok(Value::string("same")), 0).unwrap();
    let expected = read(&mut session, "mem:expected");
    session
        .resume(
            &expected,
            Err(RuntimeError::new(
                ErrorKind::Host,
                "retry constructor argument",
            )),
            0,
        )
        .unwrap();
    assert_eq!(ready(&mut session).unwrap_err().kind, ErrorKind::Host);
    session.start(entries[6], vec![module], 0, None).unwrap();
    // The reversed element was already evaluated, while the failed List
    // constructor argument must be retried with the original custom receiver.
    let expected = read(&mut session, "mem:expected");
    session
        .resume(&expected, Ok(Value::string("same")), 0)
        .unwrap();
    assert!(matches!(ready(&mut session), Ok(Value::Object(_))));
    session.close();
    assert_eq!(session.collect(&[]).slots, 0);
}
