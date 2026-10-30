//! Host/GC protocol tests, separate from Java string compatibility rows.
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
    panic!("amendment Host poll bound")
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
            assert_eq!(uri, expected);
            token
        }
        event => panic!("unexpected {event:?}"),
    }
}
fn setup() -> (HostSession, Value, Vec<usize>) {
    let source = r#"
outerValue = read("mem:outer")
sequence = new Listing { read("mem:first"); read("mem:second") }
folded = sequence.fold(new Dynamic { value = "seed" }, (acc, item) ->
  let (suffix = read("mem:callback-\(item)")) new Dynamic { value = acc.value + suffix }).value
factory = (x) -> let (text = read("mem:factory")) new Dynamic { base = text; input = x }
amended = ((factory) { x -> extra = read("mem:amendment") }).apply(7).extra
generated = ((Dynamic) { for (key in List(read("mem:key"))) {
  [key] = new Dynamic { value = read("mem:value") }
} })["key"].value
"#;
    let mut program = (*compile_module(&resolve_module(&parse_module(source).unwrap()).unwrap())
        .unwrap())
    .clone();
    let entries = ["outerValue", "folded", "amended", "generated"]
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
fn listing_forces_all_values_before_callbacks_and_retries_under_host_reentry() {
    let (mut session, module, entries) = setup();
    session
        .start(entries[0], vec![module.clone()], 0, None)
        .unwrap();
    let outer = read(&mut session, "mem:outer");
    session
        .reenter(&outer, entries[1], vec![module.clone()], 0, None)
        .unwrap();
    let first = read(&mut session, "mem:first");
    session
        .resume(&first, Ok(Value::string("first")), 0)
        .unwrap();
    let second = read(&mut session, "mem:second");
    session
        .resume(
            &second,
            Err(RuntimeError::new(ErrorKind::Host, "retry second value")),
            0,
        )
        .unwrap();
    assert_eq!(ready(&mut session).unwrap_err().kind, ErrorKind::Host);
    assert_eq!(read(&mut session, "mem:outer"), outer);
    session
        .reenter(&outer, entries[1], vec![module.clone()], 0, None)
        .unwrap();
    let second = read(&mut session, "mem:second");
    session
        .resume(&second, Ok(Value::string("second")), 0)
        .unwrap();
    let callback = read(&mut session, "mem:callback-first");
    session
        .reenter(&callback, entries[2], vec![module.clone()], 0, None)
        .unwrap();
    let factory = read(&mut session, "mem:factory");
    session
        .resume(&factory, Ok(Value::string("base")), 0)
        .unwrap();
    let amendment = read(&mut session, "mem:amendment");
    session
        .resume(&amendment, Ok(Value::string("amended")), 0)
        .unwrap();
    assert_eq!(ready(&mut session), Ok(Value::string("amended")));
    assert_eq!(read(&mut session, "mem:callback-first"), callback);
    session
        .resume(&callback, Ok(Value::string("A")), 0)
        .unwrap();
    let callback = read(&mut session, "mem:callback-second");
    session
        .resume(&callback, Ok(Value::string("B")), 0)
        .unwrap();
    assert_eq!(ready(&mut session), Ok(Value::string("seedAB")));
    assert_eq!(read(&mut session, "mem:outer"), outer);
    session
        .reenter(&outer, entries[1], vec![module], 0, None)
        .unwrap();
    assert_eq!(ready(&mut session), Ok(Value::string("seedAB")));
    assert_eq!(read(&mut session, "mem:outer"), outer);
    session.resume(&outer, Ok(Value::Null), 0).unwrap();
    assert_eq!(ready(&mut session), Ok(Value::Null));
    session.close();
    assert_eq!(session.collect(&[]).slots, 0);
}

#[test]
fn class_parent_generator_roots_its_parent_key_and_lazy_member_across_host_reads() {
    let (mut session, module, entries) = setup();
    session.start(entries[3], vec![module], 0, None).unwrap();
    let key = read(&mut session, "mem:key");
    session.resume(&key, Ok(Value::string("key")), 0).unwrap();
    let value = read(&mut session, "mem:value");
    session
        .resume(&value, Ok(Value::string("member")), 0)
        .unwrap();
    assert_eq!(ready(&mut session), Ok(Value::string("member")));
    session.close();
    assert_eq!(session.collect(&[]).slots, 0);
}

#[test]
fn cancellation_during_shallow_force_or_callback_releases_all_continuations() {
    for (callback, close) in [(false, false), (false, true), (true, false), (true, true)] {
        let (mut session, module, entries) = setup();
        session.start(entries[1], vec![module], 0, None).unwrap();
        let first = read(&mut session, "mem:first");
        session
            .resume(&first, Ok(Value::string("first")), 0)
            .unwrap();
        let mut pending = read(&mut session, "mem:second");
        if callback {
            session
                .resume(&pending, Ok(Value::string("second")), 0)
                .unwrap();
            pending = read(&mut session, "mem:callback-first");
            session.resume(&pending, Ok(Value::string("A")), 0).unwrap();
            pending = read(&mut session, "mem:callback-second");
        }
        if close {
            session.close();
        } else {
            session.cancel();
        }
        assert_eq!(
            ready(&mut session).unwrap_err().kind,
            if close {
                ErrorKind::Closed
            } else {
                ErrorKind::Cancelled
            }
        );
        assert_eq!(
            session
                .resume(&pending, Ok(Value::Null), 0)
                .unwrap_err()
                .kind,
            ErrorKind::Closed
        );
        assert_eq!(session.collect(&[]).slots, 0);
    }
}
