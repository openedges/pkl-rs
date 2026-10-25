//! Rust Host scheduling/ownership controls; Java public bytes are tested separately.
#[path = "support/module_policy.rs"]
mod module_policy;
use pkl_runtime::{
    host::{HostPoll, HostSession},
    parser::parse_module,
    resolver::resolve_module,
    value::{ErrorKind, RuntimeError, Value},
    vm::{Instruction as I, compile_module},
};
use std::sync::Arc;

const SOURCE: &str = r#"
import "pkl:ref" as ref
import "pkl:base" as base
class D extends ref.Domain {
  function renderReference(r): String = "reference"
}
local d = new D {}
local typealias Optional<T> = base.Mapping<String, T?>
class Holder {
  value: Optional<Int> = read("mem:referent-body-must-not-run")
  constrained: Mapping<String, Int>(read("mem:constraint-must-not-run") == "yes") = read("mem:constrained-body-must-not-run")
}
r = ref.Reference(d, Holder, null)
projected = r.value[read("mem:key")].getPath().length
inspect = r.value["probe"].getPath().length
constrained = r.constrained["probe"].getPath().length
stable = 7
"#;

fn setup(source: &str, names: &[&str]) -> (HostSession, Value, Vec<usize>) {
    let mut program = (*compile_module(&resolve_module(&parse_module(source).unwrap()).unwrap())
        .unwrap())
    .clone();
    module_policy::register(&mut program, source);
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
            HostPoll::Module { token, request } => {
                let completion = pkl_runtime::modules::default_completion(&request)
                    .expect("explicit module policy");
                session.complete_module(&token, completion, 0).unwrap();
            }
            event => return event,
        }
    }
    panic!("bounded Reference metadata continuation");
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
fn declared_metadata_remains_live_while_key_read_suspends_reenters_and_retries() {
    let (mut session, module, entries) = setup(SOURCE, &["projected", "inspect", "constrained"]);
    let roots = std::slice::from_ref(&module);
    start(&mut session, entries[0], &module);
    let failed = read(&mut session, roots, "mem:key");
    session
        .reenter(&failed, entries[1], vec![module.clone()], 0, None)
        .unwrap();
    assert_eq!(ready(&mut session, roots), Ok(Value::Int(2.into())));
    assert_eq!(read(&mut session, roots, "mem:key"), failed);
    session
        .resume(
            &failed,
            Err(RuntimeError::new(ErrorKind::Host, "retry key")),
            0,
        )
        .unwrap();
    assert_eq!(
        ready(&mut session, roots).unwrap_err().kind,
        ErrorKind::Host
    );
    start(&mut session, entries[0], &module);
    let retry = read(&mut session, roots, "mem:key");
    session.resume(&retry, Ok(Value::string("k")), 0).unwrap();
    assert_eq!(ready(&mut session, roots), Ok(Value::Int(2.into())));
    start(&mut session, entries[2], &module);
    // Reference metadata erases constraints without evaluating the constraint
    // or the referent property; an extra Host request fails ready().
    assert_eq!(ready(&mut session, roots), Ok(Value::Int(2.into())));
    session.close();
    assert_eq!(session.collect(&[]).slots, 0);
}

#[test]
fn pending_reference_key_cancel_and_close_release_metadata_and_reject_late_response() {
    for close in [false, true] {
        let (mut session, module, entries) = setup(SOURCE, &["projected"]);
        start(&mut session, entries[0], &module);
        let token = read(&mut session, std::slice::from_ref(&module), "mem:key");
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
