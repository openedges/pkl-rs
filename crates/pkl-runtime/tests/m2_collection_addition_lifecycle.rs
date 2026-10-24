//! Host/GC continuation checks; Java string compatibility is tested separately.
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
    panic!("addition Host poll bound")
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
class Key { value: String }
outerValue = read("mem:outer")
left = Set(new Key { value = "seed" })
right = List(new Key { value = read("mem:first") }, new Key { value = read("mem:second") })
merged = (left + right).length
nested = (List(new Key { value = throw("unused") }) + List(new Key { value = read("mem:nested") }))[1].value
mapValue = (Map("key", new Key { value = throw("overwritten") }) + Map("key", new Key { value = read("mem:map-value") }))["key"].value
"#;
    let mut program = (*compile_module(&resolve_module(&parse_module(source).unwrap()).unwrap())
        .unwrap())
    .clone();
    let entries = [
        "outerValue",
        "left",
        "right",
        "merged",
        "nested",
        "mapValue",
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
fn map_overwrite_keeps_nested_value_lazy_and_resumes_after_collection() {
    let (mut session, module, entries) = setup();
    session.start(entries[5], vec![module], 0, None).unwrap();
    let token = read(&mut session, "mem:map-value");
    session
        .resume(&token, Ok(Value::string("right")), 0)
        .unwrap();
    assert_eq!(ready(&mut session), Ok(Value::string("right")));
    session.close();
    assert_eq!(session.collect(&[]).slots, 0);
}

#[test]
fn prebuilt_operands_hash_during_addition_and_survive_reentry_failure_retry() {
    let (mut session, module, entries) = setup();
    // Both operands complete before the addition begins. The List has not
    // forced its object elements, so these reads belong to Set concatenation.
    for entry in &entries[1..=2] {
        session
            .start(*entry, vec![module.clone()], 0, None)
            .unwrap();
        ready(&mut session).unwrap();
    }
    session
        .start(entries[0], vec![module.clone()], 0, None)
        .unwrap();
    let outer = read(&mut session, "mem:outer");
    session
        .reenter(&outer, entries[3], vec![module.clone()], 0, None)
        .unwrap();
    let first = read(&mut session, "mem:first");
    session
        .reenter(&first, entries[4], vec![module.clone()], 0, None)
        .unwrap();
    let nested = read(&mut session, "mem:nested");
    session
        .resume(&nested, Ok(Value::string("nested")), 0)
        .unwrap();
    assert_eq!(ready(&mut session), Ok(Value::string("nested")));
    assert_eq!(read(&mut session, "mem:first"), first);
    session
        .resume(
            &first,
            Err(RuntimeError::new(ErrorKind::Host, "retry addition hash")),
            0,
        )
        .unwrap();
    assert_eq!(ready(&mut session).unwrap_err().kind, ErrorKind::Host);
    assert_eq!(read(&mut session, "mem:outer"), outer);
    session
        .reenter(&outer, entries[3], vec![module], 0, None)
        .unwrap();
    for uri in ["mem:first", "mem:second"] {
        let token = read(&mut session, uri);
        session.resume(&token, Ok(Value::string(uri)), 0).unwrap();
    }
    assert_eq!(ready(&mut session), Ok(Value::Int(3.into())));
    assert_eq!(read(&mut session, "mem:outer"), outer);
    session.resume(&outer, Ok(Value::Null), 0).unwrap();
    assert_eq!(ready(&mut session), Ok(Value::Null));
    session.close();
    assert_eq!(session.collect(&[]).slots, 0);
}

#[test]
fn cancelling_mid_addition_discards_builder_and_rejects_stale_completion() {
    for close in [false, true] {
        let (mut session, module, entries) = setup();
        session.start(entries[3], vec![module], 0, None).unwrap();
        let first = read(&mut session, "mem:first");
        session
            .resume(&first, Ok(Value::string("first")), 0)
            .unwrap();
        let second = read(&mut session, "mem:second");
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
                .resume(&second, Ok(Value::Null), 0)
                .unwrap_err()
                .kind,
            ErrorKind::Closed
        );
        assert_eq!(session.collect(&[]).slots, 0);
    }
}
