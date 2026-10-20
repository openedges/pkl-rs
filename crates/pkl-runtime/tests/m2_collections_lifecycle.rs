//! Collection continuation roots and Host lifecycle checks, separate from Java
//! language compatibility and resource-cache evidence.
use std::sync::Arc;

use pkl_runtime::host::{HostPoll, HostSession};
use pkl_runtime::parser::parse_module;
use pkl_runtime::resolver::resolve_module;
use pkl_runtime::value::{ErrorKind, RuntimeError, Value};
use pkl_runtime::vm::{Instruction as I, compile_module};

fn session() -> (HostSession, Value, usize, usize) {
    // Both iterable literals are temporary: only the iterator keeps their
    // receivers alive. Their targets are unpublished collection builders.
    // The parent captures each for binding in a lazy function, while the child
    // spread caches an object whose text captures a completed reader response.
    let parsed = parse_module(
        r#"
        parentResult = let (built = new Listing {
          for (value in new Listing {
            new Dynamic { text = "a" }
            read("mem:outer")
          }) { (suffix) -> value.text + suffix }
        }) built[0].apply("A") + built[1].apply("B")
        childResult = let (built = new Listing {
          ...new Listing {
            let (payload = read("mem:inner:first")) new Dynamic { text = payload }
            read("mem:inner:second")
          }
        }) new Dynamic { text = built[0].text + built[1] }
        "#,
    )
    .unwrap();
    let mut program = (*compile_module(&resolve_module(&parsed).unwrap()).unwrap()).clone();
    let module_entry = program.entry;
    let mut entries = Vec::new();
    for member in ["parentResult", "childResult"] {
        entries.push(program.blocks.len());
        program.blocks.push(vec![
            I::Input(0),
            I::GetMember(Arc::from(member)),
            I::Return,
        ]);
    }
    let mut session = HostSession::new(Arc::new(program));
    session.start(module_entry, vec![], 0, None).unwrap();
    let root = ready(&mut session, 0).unwrap();
    (session, root, entries[0], entries[1])
}

fn next_event(session: &mut HostSession, now: u64) -> HostPoll {
    loop {
        match session.poll(now, 3) {
            HostPoll::Yielded => {
                // Collect between instructions as well as at pending reads;
                // frames must trace partially constructed targets and cursors.
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

fn finish_inner(session: &mut HostSession, now: u64, prefix: &str) -> Value {
    let first = request(session, now, "mem:inner:first");
    assert!(session.collect(&[]).live > 0);
    session
        .resume(&first, Ok(Value::string(prefix)), now)
        .unwrap();
    let second = request(session, now, "mem:inner:second");
    assert!(session.collect(&[]).live > 0);
    session
        .resume(&second, Ok(Value::string("c")), now)
        .unwrap();
    ready(session, now).unwrap()
}

#[test]
fn reentered_spread_and_suspended_for_preserve_temporary_iterables_and_captures() {
    let (mut session, root, outer_entry, inner_entry) = session();
    session
        .start(outer_entry, vec![root.clone()], 1, None)
        .unwrap();
    let outer = request(&mut session, 1, "mem:outer");
    session
        .reenter(&outer, inner_entry, vec![root], 2, None)
        .unwrap();
    let inner_value = finish_inner(&mut session, 2, "b");
    assert_eq!(session.context_depth(), 1);
    assert_eq!(request(&mut session, 2, "mem:outer"), outer);
    // The child result must be retained explicitly until it is supplied as the
    // parent's reader completion. All other roots are internal continuations.
    session.collect(std::slice::from_ref(&inner_value));
    session.resume(&outer, Ok(inner_value), 3).unwrap();
    assert_eq!(ready(&mut session, 3), Ok(Value::string("aAbcB")));
    assert_eq!(session.context_depth(), 0);
    assert_eq!(session.collect(&[]).live, 0);
    assert!(session.drain_cancelled_reads().is_empty());
}

#[derive(Clone, Copy, Debug)]
enum Termination {
    Close,
    Cancel,
    AncestorTimeout,
    ChildTimeoutOnResume,
}

#[test]
fn terminal_reentry_releases_partial_builders_iterators_and_reader_tokens() {
    for termination in [
        Termination::Close,
        Termination::Cancel,
        Termination::AncestorTimeout,
        Termination::ChildTimeoutOnResume,
    ] {
        let (mut session, root, outer_entry, inner_entry) = session();
        let outer_timeout = matches!(termination, Termination::AncestorTimeout).then_some(9);
        session
            .start(outer_entry, vec![root.clone()], 1, outer_timeout)
            .unwrap();
        let outer = request(&mut session, 1, "mem:outer");
        let inner_timeout = matches!(termination, Termination::ChildTimeoutOnResume).then_some(5);
        session
            .reenter(&outer, inner_entry, vec![root.clone()], 2, inner_timeout)
            .unwrap();
        let completed = request(&mut session, 2, "mem:inner:first");
        session
            .resume(&completed, Ok(Value::string("retained")), 2)
            .unwrap();
        let inner = request(&mut session, 2, "mem:inner:second");
        assert_eq!(session.context_depth(), 2);
        assert!(session.collect(&[]).live > 0);
        assert_eq!(
            session.resume(&outer, Ok(Value::Null), 2).unwrap_err().kind,
            ErrorKind::Host
        );
        assert_eq!(request(&mut session, 2, "mem:inner:second"), inner);

        let (now, kind) = match termination {
            Termination::Close => {
                session.close();
                (3, ErrorKind::Closed)
            }
            Termination::Cancel => {
                session.cancel();
                (3, ErrorKind::Cancelled)
            }
            Termination::AncestorTimeout => (10, ErrorKind::Timeout),
            Termination::ChildTimeoutOnResume => {
                assert_eq!(
                    session
                        .resume(&inner, Ok(Value::string("late")), 7)
                        .unwrap_err()
                        .kind,
                    ErrorKind::Timeout
                );
                (7, ErrorKind::Timeout)
            }
        };
        let terminal = ready(&mut session, now).unwrap_err();
        assert_eq!(terminal.kind, kind, "{termination:?}");
        assert!(session.is_closed());
        assert_eq!(session.context_depth(), 0);
        assert_eq!(session.heap_stats().live, 0);
        assert_eq!(session.heap_stats().slots, 0);
        // The completed first child read is excluded; pending operations are
        // cancelled exactly once in ancestor order.
        assert_eq!(
            session.drain_cancelled_reads(),
            [outer.clone(), inner.clone()]
        );
        assert!(session.drain_cancelled_reads().is_empty());
        for token in [&outer, &inner, &completed] {
            for result in [
                Ok(Value::string("late")),
                Err(RuntimeError::new(ErrorKind::Host, "late failure")),
            ] {
                assert_eq!(
                    session.resume(token, result, now + 1).unwrap_err().kind,
                    ErrorKind::Closed
                );
            }
        }
        assert_eq!(
            session
                .start(outer_entry, vec![root], now + 1, None)
                .unwrap_err()
                .kind,
            ErrorKind::Closed
        );
        session.close();
        session.cancel();
        assert_eq!(ready(&mut session, now + 1).unwrap_err(), terminal);
        assert!(session.drain_cancelled_reads().is_empty());
    }
}

#[test]
fn failed_child_spread_retries_from_a_fresh_builder_while_parent_for_stays_pending() {
    let (mut session, root, outer_entry, inner_entry) = session();
    session
        .start(outer_entry, vec![root.clone()], 1, None)
        .unwrap();
    let outer = request(&mut session, 1, "mem:outer");
    session
        .reenter(&outer, inner_entry, vec![root.clone()], 2, None)
        .unwrap();
    let first = request(&mut session, 2, "mem:inner:first");
    session
        .resume(&first, Ok(Value::string("discarded")), 2)
        .unwrap();
    let failed = request(&mut session, 2, "mem:inner:second");
    let original_error = RuntimeError::new(ErrorKind::Host, "temporary spread failure\0\r\n🙂");
    session
        .resume(&failed, Err(original_error.clone()), 2)
        .unwrap();
    let error = ready(&mut session, 2).unwrap_err();
    assert_eq!(error.kind, original_error.kind);
    assert_eq!(error.message, original_error.message);
    assert_eq!(session.context_depth(), 1);
    assert_eq!(request(&mut session, 2, "mem:outer"), outer);
    session.collect(&[]);
    // Reentering the same property must repeat the earlier successful read:
    // neither the failed target nor its temporary iterable was published.
    session
        .reenter(&outer, inner_entry, vec![root], 3, None)
        .unwrap();
    for stale in [&first, &failed] {
        assert_eq!(
            session.resume(stale, Ok(Value::Null), 3).unwrap_err().kind,
            ErrorKind::Host
        );
    }
    let inner_value = finish_inner(&mut session, 3, "fresh");
    session.collect(std::slice::from_ref(&inner_value));
    session.resume(&outer, Ok(inner_value), 4).unwrap();
    assert_eq!(ready(&mut session, 4), Ok(Value::string("aAfreshcB")));
    assert!(!session.is_closed());
    assert_eq!(session.collect(&[]).live, 0);
    assert!(session.drain_cancelled_reads().is_empty());
}
