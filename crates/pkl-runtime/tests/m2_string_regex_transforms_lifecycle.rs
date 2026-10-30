//! Regex Host scheduling and ownership controls; full Java bytes are separate.
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
    panic!("bounded mapped String continuation");
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

const SOURCE: &str = r#"
all = "aba".replaceAllMapped(Regex("(a)"), (m) ->
  if (read("mem:match-\(m.start)") == "continue") m.groups[1].value else throw("stop"))
first = "aba".replaceFirstMapped(Regex("(a)"), (m) ->
  if (read("mem:match-\(m.start)") == "continue") m.groups[1].value else throw("stop"))
last = "aba".replaceLastMapped(Regex("(a)"), (m) ->
  if (read("mem:match-\(m.start)") == "continue") m.groups[1].value else throw("stop"))
stable = Regex("(a)").matchEntire("a")
value = "unrelated"
"#;

#[test]
fn mapped_match_and_cursor_survive_gc_reentry_and_failed_second_callback() {
    let (mut session, module, entries) = setup(SOURCE, &["all", "stable", "value"]);
    let roots = std::slice::from_ref(&module);
    start(&mut session, entries[0], &module);
    let first = read(&mut session, roots, "mem:match-0");
    session
        .reenter(&first, entries[1], vec![module.clone()], 0, None)
        .unwrap();
    let escaped = ready(&mut session, roots).unwrap();
    assert!(matches!(escaped, Value::Object(_)));
    let held = [module.clone(), escaped];
    assert_eq!(read(&mut session, &held, "mem:match-0"), first);
    session
        .resume(&first, Ok(Value::string("continue")), 0)
        .unwrap();
    let second = read(&mut session, &held, "mem:match-2");
    session
        .reenter(&second, entries[2], vec![module.clone()], 0, None)
        .unwrap();
    assert_eq!(ready(&mut session, &held), Ok(Value::string("unrelated")));
    assert_eq!(read(&mut session, &held, "mem:match-2"), second);
    session
        .resume(
            &second,
            Err(RuntimeError::new(ErrorKind::Host, "retry second mapper")),
            0,
        )
        .unwrap();
    assert_eq!(
        ready(&mut session, &held).unwrap_err().kind,
        ErrorKind::Host
    );
    start(&mut session, entries[0], &module);
    let retry = read(&mut session, &held, "mem:match-0");
    assert_ne!(retry, first);
    session
        .resume(&retry, Ok(Value::string("continue")), 0)
        .unwrap();
    let retry_second = read(&mut session, &held, "mem:match-2");
    assert_ne!(retry_second, second);
    session
        .resume(&retry_second, Ok(Value::string("continue")), 0)
        .unwrap();
    // Public value authority is the same identity mapper observation; this test
    // adds Rust Host scheduling/ownership, not Java SPI scheduling parity.
    assert_eq!(ready(&mut session, &held), Ok(Value::string("aba")));
    session.close();
    assert_eq!(session.collect(&[]).slots, 0);
}

#[test]
fn mapped_first_and_last_invoke_only_the_selected_callback() {
    for (name, uri) in [("first", "mem:match-0"), ("last", "mem:match-2")] {
        let (mut session, module, entries) = setup(SOURCE, &[name]);
        start(&mut session, entries[0], &module);
        let roots = std::slice::from_ref(&module);
        let token = read(&mut session, roots, uri);
        session
            .resume(&token, Ok(Value::string("continue")), 0)
            .unwrap();
        assert_eq!(ready(&mut session, roots), Ok(Value::string("aba")));
        session.close();
        assert_eq!(session.collect(&[]).slots, 0);
    }
}

#[test]
fn pending_mapped_callback_cancel_or_close_releases_cursor_and_match() {
    for close in [false, true] {
        let (mut session, module, entries) = setup(SOURCE, &["all"]);
        start(&mut session, entries[0], &module);
        let roots = std::slice::from_ref(&module);
        let token = read(&mut session, roots, "mem:match-0");
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
