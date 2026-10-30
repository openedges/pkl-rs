//! Host lifecycle assertions; Java syntax/value/diagnostic authority is in m2_string_is_regex.
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

const SOURCE: &str = r##"
pending = read("mem:pattern").isRegex
stable = #"\X"#.isRegex
"##;

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
    panic!("bounded Regex continuation");
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
fn syntax_property_preserves_host_failure_retry_reentry_and_gc() {
    let (mut session, module, entries) = setup(SOURCE, &["pending", "stable"]);
    let roots = std::slice::from_ref(&module);
    start(&mut session, entries[0], &module);
    let token = read(&mut session, roots, "mem:pattern");
    session
        .reenter(&token, entries[1], vec![module.clone()], 0, None)
        .unwrap();
    assert_eq!(ready(&mut session, roots), Ok(Value::Bool(true)));
    assert_eq!(read(&mut session, roots, "mem:pattern"), token);
    session
        .resume(
            &token,
            Err(RuntimeError::new(ErrorKind::Host, "retry pattern")),
            0,
        )
        .unwrap();
    assert_eq!(
        ready(&mut session, roots).unwrap_err().kind,
        ErrorKind::Host
    );
    start(&mut session, entries[0], &module);
    let retry = read(&mut session, roots, "mem:pattern");
    assert_ne!(retry, token);
    session
        .resume(&retry, Ok(Value::string("(abc")), 0)
        .unwrap();
    assert_eq!(ready(&mut session, roots), Ok(Value::Bool(false)));
    start(&mut session, entries[0], &module);
    assert_eq!(ready(&mut session, roots), Ok(Value::Bool(false)));
    session.close();
    assert_eq!(session.collect(&[]).slots, 0);
}

#[test]
fn syntax_property_pending_cancel_and_close_release_ownership() {
    for close in [false, true] {
        let (mut session, module, entries) = setup(SOURCE, &["pending"]);
        start(&mut session, entries[0], &module);
        let token = read(&mut session, std::slice::from_ref(&module), "mem:pattern");
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
        assert!(session.resume(&token, Ok(Value::string("abc")), 0).is_err());
        assert_eq!(session.collect(&[]).slots, 0);
    }
}
