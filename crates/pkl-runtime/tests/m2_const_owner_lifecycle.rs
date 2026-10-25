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
open class Base {
  marker: Int = 0
  const payload: String = read("mem:const")
}
class Child extends Base {}
a = new Base { marker = 1 }
b = (a) { marker = 2 }
c = new Child { marker = 3 }
first = a.payload
second = b.payload
third = c.payload
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
    panic!("bounded const owner continuation");
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
            assert_eq!(&*uri, expected);
            token
        }
        event => panic!("unexpected event {event:?}"),
    }
}
fn start(session: &mut HostSession, entry: usize, module: &Value) {
    session.start(entry, vec![module.clone()], 0, None).unwrap();
}

#[test]
fn const_owner_host_failure_retries_then_all_receivers_reuse_success() {
    let (mut session, module, entries) = setup(SOURCE, &["first", "second", "third", "stable"]);
    let roots = std::slice::from_ref(&module);
    start(&mut session, entries[0], &module);
    let failed = read(&mut session, roots, "mem:const");
    session
        .reenter(&failed, entries[3], vec![module.clone()], 0, None)
        .unwrap();
    assert_eq!(ready(&mut session, roots), Ok(Value::Int(7.into())));
    assert_eq!(read(&mut session, roots, "mem:const"), failed);
    session
        .resume(
            &failed,
            Err(RuntimeError::new(ErrorKind::Host, "owner retry")),
            0,
        )
        .unwrap();
    assert_eq!(
        ready(&mut session, roots).unwrap_err().kind,
        ErrorKind::Host
    );
    start(&mut session, entries[1], &module);
    let retry = read(&mut session, roots, "mem:const");
    assert_ne!(retry, failed);
    session
        .resume(&retry, Ok(Value::string("owner result")), 0)
        .unwrap();
    assert_eq!(
        ready(&mut session, roots),
        Ok(Value::string("owner result"))
    );
    for &entry in &[entries[0], entries[2], entries[1]] {
        start(&mut session, entry, &module);
        // A duplicate Host read is a failure, not a new supplied response.
        assert_eq!(
            ready(&mut session, roots),
            Ok(Value::string("owner result"))
        );
    }
    session.close();
    assert_eq!(session.collect(&[]).slots, 0);
}

#[test]
fn const_type_failure_does_not_publish_owner_or_receiver_cache() {
    let source = r#"
class C {
  marker: Int = 0
  const value: String(read("mem:type") == "pass") = read("mem:body")
}
a = new C { marker = 1 }
b = (a) { marker = 2 }
first = a.value
second = b.value
"#;
    let (mut session, module, entries) = setup(source, &["first", "second"]);
    let roots = std::slice::from_ref(&module);
    start(&mut session, entries[0], &module);
    let body = read(&mut session, roots, "mem:body");
    session
        .resume(&body, Ok(Value::string("payload")), 0)
        .unwrap();
    let check = read(&mut session, roots, "mem:type");
    session
        .resume(&check, Ok(Value::string("fail")), 0)
        .unwrap();
    assert_eq!(
        ready(&mut session, roots).unwrap_err().kind,
        ErrorKind::Type
    );
    start(&mut session, entries[1], &module);
    let retry_body = read(&mut session, roots, "mem:body");
    assert_ne!(body, retry_body);
    session
        .resume(&retry_body, Ok(Value::string("payload")), 0)
        .unwrap();
    let retry_check = read(&mut session, roots, "mem:type");
    assert_ne!(check, retry_check);
    session
        .resume(&retry_check, Ok(Value::string("pass")), 0)
        .unwrap();
    assert_eq!(ready(&mut session, roots), Ok(Value::string("payload")));
    start(&mut session, entries[0], &module);
    assert_eq!(ready(&mut session, roots), Ok(Value::string("payload")));
    session.close();
    assert_eq!(session.collect(&[]).slots, 0);
}

#[test]
fn pending_const_owner_cancel_close_release_both_owner_and_receiver() {
    for close in [false, true] {
        let (mut session, module, entries) = setup(SOURCE, &["second"]);
        start(&mut session, entries[0], &module);
        let pending = read(&mut session, std::slice::from_ref(&module), "mem:const");
        if close {
            session.close();
        } else {
            session.cancel();
        }
        let error = ready(&mut session, &[]).unwrap_err();
        assert_eq!(
            error.kind,
            if close {
                ErrorKind::Closed
            } else {
                ErrorKind::Cancelled
            }
        );
        assert!(
            session
                .resume(&pending, Ok(Value::string("late")), 0)
                .is_err()
        );
        assert_eq!(session.collect(&[]).slots, 0);
    }
}

#[test]
fn two_reentrant_temporary_receivers_remain_rooted_until_owner_publication() {
    let source = r#"
class C {
  marker: Int = 0
  const value: String = read("mem:shared-owner")
}
first = (new C { marker = 1 }).value
second = (new C { marker = 2 }).value
third = (new C { marker = 3 }).value
"#;
    let (mut session, module, entries) = setup(source, &["first", "second", "third"]);
    let roots = std::slice::from_ref(&module);
    start(&mut session, entries[0], &module);
    let outer = read(&mut session, roots, "mem:shared-owner");
    session
        .reenter(&outer, entries[1], vec![module.clone()], 0, None)
        .unwrap();
    let inner = read(&mut session, roots, "mem:shared-owner");
    assert_ne!(outer, inner);
    // This asserts the runtime's supported Host reentry/ownership contract,
    // not equivalence to Java's synchronous callback scheduling. Equal Host
    // completions deliberately avoid imposing a concurrent value-winner policy.
    session
        .resume(&inner, Ok(Value::string("shared")), 0)
        .unwrap();
    assert_eq!(ready(&mut session, roots), Ok(Value::string("shared")));
    assert_eq!(read(&mut session, roots, "mem:shared-owner"), outer);
    session
        .resume(&outer, Ok(Value::string("shared")), 0)
        .unwrap();
    assert_eq!(ready(&mut session, roots), Ok(Value::string("shared")));
    start(&mut session, entries[2], &module);
    assert_eq!(ready(&mut session, roots), Ok(Value::string("shared")));
    session.close();
    assert_eq!(session.collect(&[]).slots, 0);
}
