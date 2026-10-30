//! Shared Host lifecycle regressions, not Java resource-cache compatibility evidence.
use std::sync::Arc;

use pkl_runtime::host::{HostPoll, HostSession};
use pkl_runtime::parser::parse_module;
use pkl_runtime::resolver::resolve_module;
use pkl_runtime::value::{ErrorKind, RuntimeError, Value};
use pkl_runtime::vm::{Instruction as I, compile_module};

fn request(session: &mut HostSession, now: u64, expected_uri: &str) -> String {
    match session.poll(now, 1_000) {
        HostPoll::Read { token, uri } => {
            assert_eq!(uri, expected_uri);
            token
        }
        event => panic!("expected pending constraint read, got {event:?}"),
    }
}

fn terminal_error(session: &mut HostSession, now: u64) -> RuntimeError {
    match session.poll(now, 1_000) {
        HostPoll::Ready(Err(error)) => error,
        event => panic!("expected terminal error, got {event:?}"),
    }
}

#[derive(Clone, Copy, Debug)]
enum Termination {
    Close,
    AncestorTimeout,
    PredicateTimeoutOnResume,
}

#[test]
fn pending_constraint_contexts_abort_together_and_reject_late_completions() {
    for termination in [
        Termination::Close,
        Termination::AncestorTimeout,
        Termination::PredicateTimeoutOnResume,
    ] {
        // The suspended parent checks an object receiver. The reentered child
        // checks a scalar through a function that captures another object.
        let parsed = parse_module(
            "class C { value = 5 }\n\
             local function predicate(captured) = (value) -> read(\"mem:inner\") == captured.text && value == 7\n\
             checked: C(read(\"mem:outer\") == \"ok\" && this.value == 5) = new C {}\n\
             nested: Int(predicate(new Dynamic { text = \"ok\" })) = 7",
        )
        .unwrap();
        let mut program = (*compile_module(&resolve_module(&parsed).unwrap()).unwrap()).clone();
        let root_entry = program.entry;
        let mut entries = Vec::new();
        for member in ["checked", "nested"] {
            entries.push(program.blocks.len());
            program
                .blocks
                .push(vec![I::Input(0), I::GetMember(member.into()), I::Return]);
        }
        let [outer_entry, inner_entry] = entries[..] else {
            unreachable!()
        };
        let mut session = HostSession::new(Arc::new(program));
        session.start(root_entry, vec![], 0, None).unwrap();
        let module = initialized_module(&mut session);
        let outer_timeout = match termination {
            Termination::AncestorTimeout => Some(9),
            _ => None,
        };
        session
            .start(outer_entry, vec![module.clone()], 1, outer_timeout)
            .unwrap();
        let outer = request(&mut session, 1, "mem:outer");
        let inner_timeout = match termination {
            Termination::PredicateTimeoutOnResume => Some(5),
            _ => None,
        };
        session
            .reenter(&outer, inner_entry, vec![module.clone()], 2, inner_timeout)
            .unwrap();
        let inner = request(&mut session, 2, "mem:inner");
        assert_ne!(outer, inner);
        assert_eq!(session.context_depth(), 2);
        assert!(session.collect(&[]).live > 0);
        // A parent callback cannot bypass the still-active child predicate.
        assert_eq!(
            session
                .resume(&outer, Ok(Value::string("ok")), 2)
                .unwrap_err()
                .kind,
            ErrorKind::Host
        );
        assert_eq!(request(&mut session, 2, "mem:inner"), inner);

        let (now, expected_kind) = match termination {
            Termination::Close => {
                session.close();
                (3, ErrorKind::Closed)
            }
            Termination::AncestorTimeout => (10, ErrorKind::Timeout),
            Termination::PredicateTimeoutOnResume => {
                assert_eq!(
                    session
                        .resume(&inner, Ok(Value::string("ok")), 7)
                        .unwrap_err()
                        .kind,
                    ErrorKind::Timeout
                );
                (7, ErrorKind::Timeout)
            }
        };
        let terminal = terminal_error(&mut session, now);
        assert_eq!(terminal.kind, expected_kind, "{termination:?}");
        assert!(session.is_closed());
        assert_eq!(session.context_depth(), 0);
        assert_eq!(session.heap_stats().live, 0);
        assert_eq!(session.heap_stats().slots, 0);
        assert_eq!(
            session.drain_cancelled_reads(),
            [outer.clone(), inner.clone()]
        );
        assert!(session.drain_cancelled_reads().is_empty());

        for token in [&outer, &inner] {
            for completion in [
                Ok(Value::string("ok")),
                Err(RuntimeError::new(ErrorKind::Host, "late reader failure")),
            ] {
                assert_eq!(
                    session.resume(token, completion, now + 1).unwrap_err().kind,
                    ErrorKind::Closed
                );
            }
        }
        assert_eq!(
            session
                .start(outer_entry, vec![module.clone()], now + 1, None)
                .unwrap_err()
                .kind,
            ErrorKind::Closed
        );
        assert_eq!(
            session
                .reenter(&outer, inner_entry, vec![module], now + 1, None)
                .unwrap_err()
                .kind,
            ErrorKind::Closed
        );
        // Explicit close is idempotent and cannot replace an earlier timeout.
        session.close();
        assert_eq!(terminal_error(&mut session, now + 1), terminal);
        assert!(session.drain_cancelled_reads().is_empty());
    }
}

// Eager base initialization is resumable and remains under the same VM budget.
fn initialized_module(session: &mut HostSession) -> Value {
    for _ in 0..100_000 {
        match session.poll(0, 1) {
            HostPoll::Yielded => {
                session.collect(&[]);
            }
            HostPoll::Ready(Ok(module)) => return module,
            event => panic!("module construction unexpectedly suspended: {event:?}"),
        }
    }
    panic!("module initialization did not finish");
}
