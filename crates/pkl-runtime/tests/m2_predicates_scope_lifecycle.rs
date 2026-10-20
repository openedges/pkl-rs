//! Suspended predicate values and lazy replacement captures are heap roots.
use std::sync::Arc;

use pkl_runtime::host::{HostPoll, HostSession};
use pkl_runtime::parser::parse_module;
use pkl_runtime::resolver::resolve_module;
use pkl_runtime::value::{ErrorKind, RuntimeError, Value};
use pkl_runtime::vm::{Instruction as I, compile_module};

fn session() -> (HostSession, Value, usize, usize) {
    // Each unpublished target and temporary parent survives through its cursor.
    // Replacement closures capture the target this and the completed for binding.
    let parsed = parse_module(
        r#"
        outerResult = let (built = (new Listing {
          new Dynamic { text = "a" }
          new Dynamic { text = "b" }
        }) {
          for (prefix in List("outer:")) {
            [[let (copy = this)
              read("mem:outer:" + text) == "select" &&
              (() -> this.text == copy.text).apply()]] =
                (suffix) -> prefix + suffix + this[1].text
          }
        }) built[0].apply("A")
        innerResult = let (built = (new Mapping {
          ["key"] = new Dynamic { text = "c" }
          ["other"] = new Dynamic { text = "d" }
        }) {
          [[let (copy = this)
            read("mem:inner:" + text) == "select" &&
            (() -> this.text == copy.text).apply()]] =
              () -> this["other"].text
        }) built["key"].apply()
        "#,
    )
    .unwrap();
    let mut program = (*compile_module(&resolve_module(&parsed).unwrap()).unwrap()).clone();
    let module_entry = program.entry;
    let entries = ["outerResult", "innerResult"].map(|member| {
        let entry = program.blocks.len();
        program.blocks.push(vec![
            I::Input(0),
            I::GetMember(Arc::from(member)),
            I::Return,
        ]);
        entry
    });
    let mut session = HostSession::new(Arc::new(program));
    session.start(module_entry, vec![], 0, None).unwrap();
    let root = ready(&mut session, 0).unwrap();
    (session, root, entries[0], entries[1])
}

fn next_event(session: &mut HostSession, now: u64) -> HostPoll {
    loop {
        match session.poll(now, 2) {
            HostPoll::Yielded => {
                session.collect(&[]);
            }
            event => return event,
        }
    }
}

fn request(session: &mut HostSession, now: u64, expected_uri: &str) -> String {
    match next_event(session, now) {
        HostPoll::Read { token, uri } => {
            assert_eq!(&*uri, expected_uri);
            token
        }
        event => panic!("expected {expected_uri}, got {event:?}"),
    }
}

fn ready(session: &mut HostSession, now: u64) -> Result<Value, RuntimeError> {
    match next_event(session, now) {
        HostPoll::Ready(result) => result,
        event => panic!("expected completed context, got {event:?}"),
    }
}

fn finish_inner(session: &mut HostSession, now: u64) -> Value {
    let first = request(session, now, "mem:inner:c");
    assert!(session.collect(&[]).live > 0);
    session
        .resume(&first, Ok(Value::string("select")), now)
        .unwrap();
    let second = request(session, now, "mem:inner:d");
    assert!(session.collect(&[]).live > 0);
    session
        .resume(&second, Ok(Value::string("skip")), now)
        .unwrap();
    ready(session, now).unwrap()
}

#[test]
fn suspended_predicates_preserve_for_let_and_replacement_this_captures() {
    let (mut session, root, outer_entry, inner_entry) = session();
    session
        .start(outer_entry, vec![root.clone()], 1, None)
        .unwrap();
    let outer = request(&mut session, 1, "mem:outer:a");
    session
        .reenter(&outer, inner_entry, vec![root], 2, None)
        .unwrap();
    assert_eq!(finish_inner(&mut session, 2), Value::string("d"));
    assert_eq!(session.context_depth(), 1);
    assert_eq!(request(&mut session, 2, "mem:outer:a"), outer);
    session.collect(&[]);
    session
        .resume(&outer, Ok(Value::string("select")), 3)
        .unwrap();
    let second = request(&mut session, 3, "mem:outer:b");
    session
        .resume(&second, Ok(Value::string("skip")), 3)
        .unwrap();
    assert_eq!(ready(&mut session, 3), Ok(Value::string("outer:Ab")));
    assert_eq!(session.context_depth(), 0);
    assert_eq!(session.collect(&[]).live, 0);
    assert!(session.drain_cancelled_reads().is_empty());
}

#[test]
fn failed_predicate_retries_from_a_fresh_target_with_fresh_captured_values() {
    let (mut session, root, outer_entry, inner_entry) = session();
    session
        .start(outer_entry, vec![root.clone()], 1, None)
        .unwrap();
    let outer = request(&mut session, 1, "mem:outer:a");
    session
        .reenter(&outer, inner_entry, vec![root.clone()], 2, None)
        .unwrap();
    let first = request(&mut session, 2, "mem:inner:c");
    session
        .resume(&first, Ok(Value::string("select")), 2)
        .unwrap();
    let failed = request(&mut session, 2, "mem:inner:d");
    let original = RuntimeError::new(ErrorKind::Host, "temporary predicate failure\0\r\n🙂");
    session.resume(&failed, Err(original.clone()), 2).unwrap();
    let error = ready(&mut session, 2).unwrap_err();
    assert_eq!(
        (error.kind, error.message),
        (original.kind, original.message)
    );
    assert_eq!(session.context_depth(), 1);
    assert_eq!(request(&mut session, 2, "mem:outer:a"), outer);
    session.collect(&[]);
    session
        .reenter(&outer, inner_entry, vec![root], 3, None)
        .unwrap();
    for stale in [&first, &failed] {
        assert_eq!(
            session.resume(stale, Ok(Value::Null), 3).unwrap_err().kind,
            ErrorKind::Host
        );
    }
    assert_eq!(finish_inner(&mut session, 3), Value::string("d"));
    session
        .resume(&outer, Ok(Value::string("select")), 4)
        .unwrap();
    let second = request(&mut session, 4, "mem:outer:b");
    session
        .resume(&second, Ok(Value::string("skip")), 4)
        .unwrap();
    assert_eq!(ready(&mut session, 4), Ok(Value::string("outer:Ab")));
    assert!(!session.is_closed());
    assert_eq!(session.collect(&[]).live, 0);
    assert!(session.drain_cancelled_reads().is_empty());
}
