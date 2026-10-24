//! Shared ownership and cooperative execution checks, separate from Java rows.
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
    panic!("IntSeq Host poll bound")
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
fn setup(source: &str, names: &[&str]) -> (HostSession, Value, Vec<usize>) {
    let mut program = (*compile_module(&resolve_module(&parse_module(source).unwrap()).unwrap())
        .unwrap())
    .clone();
    let entries = names
        .iter()
        .map(|name| {
            let index = program.blocks.len();
            program
                .blocks
                .push(vec![I::Input(0), I::GetMember((*name).into()), I::Return]);
            index
        })
        .collect();
    let entry = program.entry;
    let mut session = HostSession::new(Arc::new(program));
    session.start(entry, vec![], 0, None).unwrap();
    let module = ready(&mut session).unwrap();
    (session, module, entries)
}
const SOURCE: &str = r#"
parent = read("mem:outer")
mapped = IntSeq(1,3).map((n) -> if (read("mem:map/\(n)") == "ok") List(n) else throw("map failed")).map((row) -> row[0]).toBytes()
folded = IntSeq(1,3).fold(List(), (acc,n) -> if (read("mem:fold/\(n)") == "ok") acc.add(n) else throw("fold failed")).toBytes()
listFolded = List(1,2,3).fold(List(), (acc,n) -> if (read("mem:listfold/\(n)") == "ok") acc.add(n) else throw("list fold failed")).toBytes()
generatedObject = new Dynamic { for (n in IntSeq(1,3)) { [read("mem:key/\(n)")] = n } }
generated = generatedObject["one"] + generatedObject["two"] + generatedObject["three"]
nested = IntSeq(1,2).map((n) -> IntSeq(1,n).fold(0, (acc,m) -> acc+m)).toBytes()
"#;
#[test]
fn callbacks_and_accumulators_survive_gc_reentry_failure_and_retry() {
    for (method, prefix) in [(1, "map"), (2, "fold"), (3, "listfold")] {
        let (mut session, module, entries) = setup(
            SOURCE,
            &["parent", "mapped", "folded", "listFolded", "nested"],
        );
        session
            .start(entries[0], vec![module.clone()], 0, None)
            .unwrap();
        let outer = read(&mut session, "mem:outer");
        session
            .reenter(&outer, entries[method], vec![module.clone()], 0, None)
            .unwrap();
        let first = read(&mut session, &format!("mem:{prefix}/1"));
        session.resume(&first, Ok(Value::string("ok")), 0).unwrap();
        let second = read(&mut session, &format!("mem:{prefix}/2"));
        session
            .reenter(&second, entries[4], vec![module.clone()], 0, None)
            .unwrap();
        assert_eq!(ready(&mut session), Ok(Value::Bytes([1, 3].into())));
        assert_eq!(read(&mut session, &format!("mem:{prefix}/2")), second);
        session
            .resume(
                &second,
                Err(RuntimeError::new(ErrorKind::Host, "retry sequence")),
                0,
            )
            .unwrap();
        assert_eq!(ready(&mut session).unwrap_err().kind, ErrorKind::Host);
        assert_eq!(read(&mut session, "mem:outer"), outer);
        session
            .reenter(&outer, entries[method], vec![module], 0, None)
            .unwrap();
        for n in 1..=3 {
            let token = read(&mut session, &format!("mem:{prefix}/{n}"));
            assert_ne!(token, first);
            session.resume(&token, Ok(Value::string("ok")), 0).unwrap();
        }
        assert_eq!(ready(&mut session), Ok(Value::Bytes([1, 2, 3].into())));
        assert_eq!(read(&mut session, "mem:outer"), outer);
        session.resume(&outer, Ok(Value::Null), 0).unwrap();
        assert_eq!(ready(&mut session), Ok(Value::Null));
        session.close();
        assert_eq!(session.collect(&[]).slots, 0);
    }
}
#[test]
fn generator_cursor_and_binding_scopes_survive_host_requests() {
    let (mut session, module, entries) = setup(SOURCE, &["generated"]);
    session.start(entries[0], vec![module], 0, None).unwrap();
    for (n, key) in [(1, "one"), (2, "two"), (3, "three")] {
        let token = read(&mut session, &format!("mem:key/{n}"));
        session.resume(&token, Ok(Value::string(key)), 0).unwrap();
    }
    assert_eq!(ready(&mut session), Ok(Value::Int(6.into())));
    session.close();
    assert_eq!(session.collect(&[]).slots, 0);
}
#[test]
fn pending_sequence_callbacks_cancel_close_and_release_all_state() {
    for close in [false, true] {
        for member in ["mapped", "folded", "listFolded"] {
            let (mut session, module, entries) = setup(SOURCE, &[member]);
            session.start(entries[0], vec![module], 0, None).unwrap();
            let prefix = match member {
                "mapped" => "map",
                "folded" => "fold",
                _ => "listfold",
            };
            let first = read(&mut session, &format!("mem:{prefix}/1"));
            session.resume(&first, Ok(Value::string("ok")), 0).unwrap();
            let pending = read(&mut session, &format!("mem:{prefix}/2"));
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
            assert_eq!(session.heap_stats().slots, 0);
            assert_eq!(
                session
                    .resume(&pending, Ok(Value::Null), 0)
                    .unwrap_err()
                    .kind,
                ErrorKind::Closed
            );
        }
    }
}
#[test]
fn huge_sequences_yield_before_materialization_and_remain_cancellable() {
    for expression in [
        "IntSeq(0,1000000000000).toList()",
        "IntSeq(0,1000000000000).toListing()",
        "IntSeq(0,1000000000000).map((n) -> n)",
        "IntSeq(0,1000000000000).fold(0,(acc,n) -> acc+n)",
        "new Listing { for (n in IntSeq(0,1000000000000)) { n } }",
        "new Listing { ...IntSeq(0,1000000000000) }",
    ] {
        let (mut session, module, entries) = setup(&format!("result = {expression}"), &["result"]);
        session.start(entries[0], vec![module], 0, None).unwrap();
        for _ in 0..500 {
            assert!(
                matches!(session.poll(0, 1), HostPoll::Yielded),
                "{expression}"
            );
            session.collect(&[]);
        }
        assert!(session.heap_stats().live < 1000, "{expression}");
        session.cancel();
        assert_eq!(ready(&mut session).unwrap_err().kind, ErrorKind::Cancelled);
        assert_eq!(session.heap_stats().slots, 0);
    }
}
