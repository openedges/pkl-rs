//! Reentrant predicate cursors use the ordinary Host lifecycle and heap roots.
use std::sync::Arc;
#[path = "support/eager_base.rs"]
mod eager_base;
use eager_base::Baseline;

use pkl_runtime::host::{HostPoll, HostSession};
use pkl_runtime::parser::parse_module;
use pkl_runtime::resolver::resolve_module;
use pkl_runtime::value::{ErrorKind, RuntimeError, Value};
use pkl_runtime::vm::{Instruction as I, compile_module};

fn session() -> (HostSession, Value, usize, usize) {
    let parsed = parse_module(
        r#"
        parent = let (source = new Listing { new {text="keep"}; "select" })
          let (selected = (source) {
            [[this is String && read("mem:outer") == this]] = 42
          }) "\(selected[0].text)\(selected[1])"
        child = let (source = new Listing { "child" })
          let (selected = (source) {
            [[(source) { [[read("mem:inner") == this]] = 1 }[0] == 1 && this == "child"]] = 7
          }) selected[0]
        "#,
    )
    .unwrap();
    let mut program = (*compile_module(&resolve_module(&parsed).unwrap()).unwrap()).clone();
    let module_entry = program.entry;
    let entries = ["parent", "child"].map(|member| {
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

fn event(session: &mut HostSession, now: u64) -> HostPoll {
    loop {
        match session.poll(now, 1) {
            HostPoll::Yielded => {
                session.collect(&[]);
            }
            event => return event,
        }
    }
}

fn ready(session: &mut HostSession, now: u64) -> Result<Value, RuntimeError> {
    match event(session, now) {
        HostPoll::Ready(value) => value,
        event => panic!("expected ready, got {event:?}"),
    }
}

fn request(session: &mut HostSession, now: u64, expected: &str) -> String {
    match event(session, now) {
        HostPoll::Read { token, uri } => {
            assert_eq!(&*uri, expected);
            token
        }
        event => panic!("expected read, got {event:?}"),
    }
}

#[test]
fn nested_predicate_reentry_and_retry_preserve_parent_cursor_and_custom_this() {
    let (mut session, root, parent, child) = session();
    let initialized = Baseline::new(session.collect(&[]));
    let stale_root = root.clone();
    session.start(parent, vec![root.clone()], 1, None).unwrap();
    let outer = request(&mut session, 1, "mem:outer");
    for success in [false, true] {
        session
            .reenter(&outer, child, vec![root.clone()], 2, None)
            .unwrap();
        let inner = request(&mut session, 2, "mem:inner");
        session.collect(&[]);
        session
            .resume(
                &inner,
                if success {
                    Ok(Value::string("child"))
                } else {
                    Err(RuntimeError::new(ErrorKind::Host, "retry predicate"))
                },
                2,
            )
            .unwrap();
        let result = ready(&mut session, 2);
        if success {
            assert_eq!(result, Ok(Value::Int(7.into())));
        } else {
            assert_eq!(result.unwrap_err().kind, ErrorKind::Host);
        }
        assert_eq!(session.context_depth(), 1);
        assert_eq!(request(&mut session, 2, "mem:outer"), outer);
    }
    session
        .resume(&outer, Ok(Value::string("select")), 3)
        .unwrap();
    assert_eq!(ready(&mut session, 3), Ok(Value::string("keep42")));
    // Only evaluator-owned base values and the exact lexical descriptors survive.
    assert_eq!(initialized.added_values(session.collect(&[]), 2), -2);
    assert_eq!(
        session
            .start(parent, vec![stale_root], 10, None)
            .unwrap_err()
            .kind,
        ErrorKind::StaleHandle
    );
    session.close();
    assert_eq!(session.heap_stats().slots, 0);
    assert!(session.drain_cancelled_reads().is_empty());
}

#[test]
fn terminal_predicate_reentry_cancels_reads_and_releases_all_cursor_roots() {
    for termination in ["close", "cancel", "ancestor timeout", "child timeout"] {
        let (mut session, root, parent, child) = session();
        session
            .start(
                parent,
                vec![root.clone()],
                1,
                (termination == "ancestor timeout").then_some(5),
            )
            .unwrap();
        let outer = request(&mut session, 1, "mem:outer");
        session
            .reenter(
                &outer,
                child,
                vec![root],
                2,
                (termination == "child timeout").then_some(3),
            )
            .unwrap();
        let inner = request(&mut session, 2, "mem:inner");
        assert!(session.collect(&[]).live > 0);
        let kind = match termination {
            "close" => {
                session.close();
                ErrorKind::Closed
            }
            "cancel" => {
                session.cancel();
                ErrorKind::Cancelled
            }
            "child timeout" => {
                assert_eq!(
                    session.resume(&inner, Ok(Value::Null), 6).unwrap_err().kind,
                    ErrorKind::Timeout
                );
                ErrorKind::Timeout
            }
            _ => ErrorKind::Timeout,
        };
        assert_eq!(ready(&mut session, 6).unwrap_err().kind, kind);
        assert_eq!(session.context_depth(), 0);
        assert_eq!(session.heap_stats().live, 0);
        assert_eq!(session.heap_stats().slots, 0);
        assert_eq!(
            session.drain_cancelled_reads(),
            [outer.clone(), inner.clone()]
        );
        assert!(session.drain_cancelled_reads().is_empty());
        for stale in [&outer, &inner] {
            assert_eq!(
                session.resume(stale, Ok(Value::Null), 7).unwrap_err().kind,
                ErrorKind::Closed
            );
        }
    }
}
