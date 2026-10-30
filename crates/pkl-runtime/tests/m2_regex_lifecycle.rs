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

const SOURCE: &str = r#"
function make(pattern: String): Regex = Regex(pattern)
delayed = make(read("mem:pattern")).matchEntire(read("mem:input"))
stable = Regex("(a)").matchEntire("a")
bad = make("[")
good = make("(a)").matchEntire("a").groups[1].value
value = "unrelated"
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
fn regex_arguments_survive_gc_reentry_and_failed_host_response() {
    let (mut session, module, entries) = setup(SOURCE, &["delayed", "stable", "value"]);
    let roots = std::slice::from_ref(&module);
    start(&mut session, entries[0], &module);
    let pattern = read(&mut session, roots, "mem:pattern");
    session
        .reenter(&pattern, entries[1], vec![module.clone()], 0, None)
        .unwrap();
    let escaped_match = ready(&mut session, roots).unwrap();
    assert!(matches!(escaped_match, Value::Object(_)));
    assert_eq!(read(&mut session, roots, "mem:pattern"), pattern);
    session
        .resume(&pattern, Ok(Value::string("(a)")), 0)
        .unwrap();
    let input = read(&mut session, roots, "mem:input");
    session
        .reenter(&input, entries[2], vec![module.clone()], 0, None)
        .unwrap();
    assert_eq!(ready(&mut session, roots), Ok(Value::string("unrelated")));
    assert_eq!(read(&mut session, roots, "mem:input"), input);
    session
        .resume(
            &input,
            Err(RuntimeError::new(ErrorKind::Host, "retry input")),
            0,
        )
        .unwrap();
    assert_eq!(
        ready(&mut session, roots).unwrap_err().kind,
        ErrorKind::Host
    );
    start(&mut session, entries[0], &module);
    // Raw HostSession::resume does not populate the typed ResourceReader cache.
    // A failed enclosing property therefore evaluates its pattern read again.
    let retry_pattern = read(&mut session, roots, "mem:pattern");
    assert_ne!(retry_pattern, pattern);
    session
        .resume(&retry_pattern, Ok(Value::string("(a)")), 0)
        .unwrap();
    let retry_input = read(&mut session, roots, "mem:input");
    assert_ne!(retry_input, input);
    session
        .resume(&retry_input, Ok(Value::string("a")), 0)
        .unwrap();
    assert!(matches!(
        ready(&mut session, &[module.clone(), escaped_match]).unwrap(),
        Value::Object(_)
    ));
    session.close();
    assert_eq!(session.collect(&[]).slots, 0);
}

#[test]
fn failed_regex_compile_retries_at_the_same_function_site() {
    let (mut session, module, entries) = setup(SOURCE, &["bad", "good"]);
    let roots = std::slice::from_ref(&module);
    for _ in 0..2 {
        start(&mut session, entries[0], &module);
        assert_eq!(
            ready(&mut session, roots).unwrap_err().kind,
            ErrorKind::Type
        );
        start(&mut session, entries[1], &module);
        assert_eq!(ready(&mut session, roots), Ok(Value::string("a")));
    }
    session.close();
    assert_eq!(session.collect(&[]).slots, 0);
}

#[test]
fn pending_regex_input_cancel_and_close_release_compiled_state() {
    for close in [false, true] {
        let (mut session, module, entries) = setup(SOURCE, &["delayed"]);
        start(&mut session, entries[0], &module);
        let roots = std::slice::from_ref(&module);
        let pattern = read(&mut session, roots, "mem:pattern");
        session.resume(&pattern, Ok(Value::string("a")), 0).unwrap();
        let input = read(&mut session, roots, "mem:input");
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
                .resume(&input, Ok(Value::string("late")), 0)
                .is_err()
        );
        assert_eq!(session.collect(&[]).slots, 0);
    }
}
