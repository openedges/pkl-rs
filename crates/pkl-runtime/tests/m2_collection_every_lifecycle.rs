//! Host scheduling and ownership tests; Java raw compatibility is a separate matrix.
use pkl_runtime::{
    host::{HostPoll, HostSession},
    parser::parse_module,
    resolver::resolve_module,
    value::{ErrorKind, RuntimeError, Value},
    vm::{Instruction as I, compile_module},
};
use std::sync::Arc;

fn event(session: &mut HostSession) -> HostPoll {
    event_rooted(session, &[])
}
fn event_rooted(session: &mut HostSession, roots: &[Value]) -> HostPoll {
    for _ in 0..200_000 {
        session.collect(roots);
        match session.poll(0, 1) {
            HostPoll::Yielded => {}
            event => return event,
        }
    }
    panic!("every Host poll bound")
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
fn resume(session: &mut HostSession, uri: &str, value: &str) {
    let token = read(session, uri);
    session.resume(&token, Ok(Value::string(value)), 0).unwrap();
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
const KINDS: [(&str, &str); 5] = [
    ("List(1, 2, 3)", "(v)"),
    ("Set(1, 2, 1, 3)", "(v)"),
    ("Map(\"a\", 1, \"b\", 2, \"c\", 3)", "(k, v)"),
    ("(new Listing { 1; 2; throw(\"later value\") })", "(v)"),
    (
        "(new Mapping { [\"a\"] = 1; [\"b\"] = 2; [\"c\"] = throw(\"later value\") })",
        "(k, v)",
    ),
];
#[test]
fn all_five_predicates_root_source_and_callback_through_nested_host_failure_retry() {
    for (collection, parameters) in KINDS {
        let source = format!(
            r#"
outerValue = read("mem:outer")
nested = (new Mapping {{ ["x"] = read("mem:nested") }}).every((k, v) -> k == "x" && v == "ok")
result = {collection}.every({parameters} -> read("mem:predicate/\(v)") == "true")
"#
        );
        let (mut session, module, entries) = setup(&source, &["outerValue", "result", "nested"]);
        session
            .start(entries[0], vec![module.clone()], 0, None)
            .unwrap();
        let outer = read(&mut session, "mem:outer");
        session
            .reenter(&outer, entries[1], vec![module.clone()], 0, None)
            .unwrap();
        let first = read(&mut session, "mem:predicate/1");
        session
            .reenter(&first, entries[2], vec![module.clone()], 0, None)
            .unwrap();
        resume(&mut session, "mem:nested", "ok");
        assert_eq!(ready(&mut session), Ok(Value::Bool(true)));
        assert_eq!(read(&mut session, "mem:predicate/1"), first);
        session
            .resume(
                &first,
                Err(RuntimeError::new(ErrorKind::Host, "predicate retry")),
                0,
            )
            .unwrap();
        assert_eq!(ready(&mut session).unwrap_err().kind, ErrorKind::Host);
        assert_eq!(read(&mut session, "mem:outer"), outer);
        session
            .reenter(&outer, entries[1], vec![module], 0, None)
            .unwrap();
        resume(&mut session, "mem:predicate/1", "true");
        resume(&mut session, "mem:predicate/2", "false");
        assert_eq!(ready(&mut session), Ok(Value::Bool(false)));
        assert_eq!(read(&mut session, "mem:outer"), outer);
        session.resume(&outer, Ok(Value::Null), 0).unwrap();
        assert_eq!(ready(&mut session), Ok(Value::Null));
        session.close();
        assert_eq!(session.collect(&[]).slots, 0);
    }
}
#[test]
fn lazy_member_reads_follow_parent_order_use_overrides_and_retry_only_failed_value() {
    for (collection, parameters) in [
        (
            r#"(new Listing { read("mem:overridden"); read("mem:second"); throw("later") }) { [0] = read("mem:first") }"#,
            "(v)",
        ),
        (
            r#"(new Mapping { ["a"] = read("mem:overridden"); ["b"] = read("mem:second"); ["c"] = throw("later") }) { ["a"] = read("mem:first") }"#,
            "(k, v)",
        ),
    ] {
        let source = format!(
            r#"values = {collection}
result = values.every({parameters} -> v == "true")"#
        );
        let (mut session, module, entries) = setup(&source, &["result"]);
        session
            .start(entries[0], vec![module.clone()], 0, None)
            .unwrap();
        resume(&mut session, "mem:first", "true");
        let failed = read(&mut session, "mem:second");
        session
            .resume(
                &failed,
                Err(RuntimeError::new(ErrorKind::Host, "member retry")),
                0,
            )
            .unwrap();
        match event_rooted(&mut session, std::slice::from_ref(&module)) {
            HostPoll::Ready(Err(error)) => assert_eq!(error.kind, ErrorKind::Host),
            event => panic!("unexpected {event:?}"),
        }
        session.collect(std::slice::from_ref(&module));
        session.start(entries[0], vec![module], 0, None).unwrap();
        let retried = read(&mut session, "mem:second");
        assert_ne!(failed, retried);
        session
            .resume(&retried, Ok(Value::string("false")), 0)
            .unwrap();
        assert_eq!(ready(&mut session), Ok(Value::Bool(false)));
        session.close();
        assert_eq!(session.collect(&[]).slots, 0);
    }
}
#[test]
fn pending_predicate_and_lazy_value_cursors_cancel_close_and_reject_stale_completion() {
    for close in [false, true] {
        for (collection, parameters) in KINDS {
            let source = format!(
                r#"result = {collection}.every({parameters} -> read("mem:pending") == "true")"#
            );
            let (mut session, module, entries) = setup(&source, &["result"]);
            session
                .start(entries[0], vec![module.clone()], 0, None)
                .unwrap();
            let token = read(&mut session, "mem:pending");
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
            assert!(
                session
                    .resume(&token, Ok(Value::string("true")), 0)
                    .is_err()
            );
            assert_eq!(
                session
                    .start(entries[0], vec![module], 0, None)
                    .unwrap_err()
                    .kind,
                ErrorKind::Closed
            );
            assert_eq!(session.collect(&[]).slots, 0);
        }
        for expression in [
            r#"(new Listing { read("mem:value") }).every((v) -> true)"#,
            r#"(new Mapping { ["a"] = read("mem:value") }).every((k, v) -> true)"#,
        ] {
            let (mut session, module, entries) =
                setup(&format!("result = {expression}"), &["result"]);
            session
                .start(entries[0], vec![module.clone()], 0, None)
                .unwrap();
            let token = read(&mut session, "mem:value");
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
            assert!(session.resume(&token, Ok(Value::string("ok")), 0).is_err());
            assert_eq!(
                session
                    .start(entries[0], vec![module], 0, None)
                    .unwrap_err()
                    .kind,
                ErrorKind::Closed
            );
            assert_eq!(session.collect(&[]).slots, 0);
        }
    }
}
