use pkl_runtime::{
    host::{HostPoll, HostSession},
    value::{ErrorKind, RuntimeError, Value},
    vm::{Instruction as I, Program},
};
use std::sync::Arc;

fn reader() -> HostSession {
    HostSession::new(Arc::new(Program::new(vec![vec![
        I::Constant(Value::Int(40)),
        I::Input(0),
        I::ReadResource,
        I::Add,
        I::Return,
    ]])))
}
fn start(session: &mut HostSession, now: u64, timeout: u64) {
    session
        .start(0, vec![Value::string("test:/resource")], now, Some(timeout))
        .unwrap();
}
fn request(session: &mut HostSession, now: u64) -> String {
    match session.poll(now, 100) {
        HostPoll::Read { token, uri } => {
            assert_eq!(&*uri, "test:/resource");
            token
        }
        event => panic!("{event:?}"),
    }
}
fn error_kind(event: HostPoll) -> ErrorKind {
    match event {
        HostPoll::Ready(Err(error)) => error.kind,
        event => panic!("{event:?}"),
    }
}

#[test]
fn suspended_stack_survives_and_completions_are_exactly_once() {
    let mut session = reader();
    start(&mut session, 10, 100);
    let token = request(&mut session, 11);
    assert_eq!(request(&mut session, 12), token);
    assert_eq!(
        session.start(0, vec![], 12, None).unwrap_err().kind,
        ErrorKind::Busy
    );
    session.resume(&token, Ok(Value::Int(2)), 13).unwrap();
    assert_eq!(
        session
            .resume(&token, Ok(Value::Int(5)), 13)
            .unwrap_err()
            .kind,
        ErrorKind::Host
    );
    assert_eq!(session.poll(13, 100), HostPoll::Ready(Ok(Value::Int(42))));
    start(&mut session, 14, 100);
    let next = request(&mut session, 14);
    assert_ne!(token, next);
    assert_eq!(
        session
            .resume(&token, Ok(Value::Int(2)), 14)
            .unwrap_err()
            .kind,
        ErrorKind::Host
    );
    session.resume(&next, Ok(Value::Int(3)), 14).unwrap();
    assert_eq!(session.poll(14, 100), HostPoll::Ready(Ok(Value::Int(43))));
}

#[test]
fn request_identity_cannot_cross_evaluators_or_accept_noncanonical_tokens() {
    let mut a = reader();
    let mut b = reader();
    start(&mut a, 0, 100);
    start(&mut b, 0, 100);
    let at = request(&mut a, 0);
    let bt = request(&mut b, 0);
    for token in [at, format!("0{bt}"), "invalid".into()] {
        assert_eq!(
            b.resume(&token, Ok(Value::Int(2)), 1).unwrap_err().kind,
            ErrorKind::Host
        );
    }
    b.resume(&bt, Ok(Value::Int(2)), 1).unwrap();
    assert_eq!(b.poll(1, 100), HostPoll::Ready(Ok(Value::Int(42))));
}

#[test]
fn timeout_wins_at_deadline_even_while_waiting_and_closes_permanently() {
    let mut session = reader();
    start(&mut session, 0, 10);
    let token = request(&mut session, 1);
    assert_eq!(
        session
            .resume(&token, Ok(Value::Int(2)), 10)
            .unwrap_err()
            .kind,
        ErrorKind::Timeout
    );
    assert!(session.is_closed());
    assert_eq!(error_kind(session.poll(10, 100)), ErrorKind::Timeout);
    assert_eq!(
        session.start(0, vec![], 11, None).unwrap_err().kind,
        ErrorKind::Closed
    );
    assert_eq!(
        session
            .resume(&token, Ok(Value::Int(2)), 11)
            .unwrap_err()
            .kind,
        ErrorKind::Closed
    );
    session.close();
    assert_eq!(error_kind(session.poll(11, 100)), ErrorKind::Timeout);
}

#[test]
fn cancellation_close_and_host_failures_have_distinct_lifetimes() {
    for cancel in [true, false] {
        let mut session = reader();
        start(&mut session, 0, 100);
        let token = request(&mut session, 0);
        if cancel {
            session.cancel();
        } else {
            session.close();
        }
        assert_eq!(
            error_kind(session.poll(1, 100)),
            if cancel {
                ErrorKind::Cancelled
            } else {
                ErrorKind::Closed
            }
        );
        assert_eq!(
            session
                .resume(&token, Ok(Value::Int(2)), 1)
                .unwrap_err()
                .kind,
            ErrorKind::Closed
        );
        assert_eq!(session.heap_stats().live, 0);
    }
    let mut session = reader();
    start(&mut session, 0, 100);
    let token = request(&mut session, 0);
    let failure = RuntimeError::new(ErrorKind::Host, "host\0failure\r\n🙂");
    session.resume(&token, Err(failure.clone()), 1).unwrap();
    assert_eq!(session.poll(1, 100), HostPoll::Ready(Err(failure)));
    start(&mut session, 2, 100);
    assert!(!session.is_closed());
}

#[test]
fn cooperative_loop_observes_clock_and_budget_contracts() {
    let mut session = HostSession::new(Arc::new(Program::new(vec![vec![I::Jump(0)]])));
    session.start(0, vec![], 10, Some(5)).unwrap();
    assert_eq!(session.poll(11, 100), HostPoll::Yielded);
    assert_eq!(error_kind(session.poll(10, 100)), ErrorKind::Host);
    assert_eq!(error_kind(session.poll(12, 0)), ErrorKind::Host);
    assert_eq!(session.poll(14, 100), HostPoll::Yielded);
    assert_eq!(error_kind(session.poll(15, 100)), ErrorKind::Timeout);
}

#[test]
fn close_reclaims_heap_while_read_is_pending() {
    let mut program = Program::new(vec![vec![
        I::MakeObject {
            template: 0,
            has_parent: false,
        },
        I::Constant(Value::string("test:/resource")),
        I::ReadResource,
        I::Return,
    ]]);
    program.objects.push(vec![]);
    let mut session = HostSession::new(Arc::new(program));
    session.start(0, vec![], 0, None).unwrap();
    request(&mut session, 0);
    assert_eq!(session.heap_stats().live, 1);
    session.close();
    assert_eq!(session.heap_stats().live, 0);
    assert_eq!(session.heap_stats().slots, 0);
}

#[test]
fn adapters_use_real_frontend_and_vm_and_reject_dangling_objects() {
    use pkl_runtime::probe::evaluate_member;
    assert_eq!(
        evaluate_member("answer = 40 + 2", "answer"),
        Ok(Value::Int(42))
    );
    assert!(evaluate_member("answer = new { value = 1 }", "answer").is_err());
    assert!(evaluate_member("answer = 1 +", "answer").is_err());
}

fn shared_session() -> (HostSession, Value) {
    use pkl_runtime::heap::Member;
    let mut program = Program::new(vec![
        vec![
            I::MakeObject {
                template: 0,
                has_parent: false,
            },
            I::Return,
        ],
        vec![I::Input(0), I::GetMember(Arc::from("outer")), I::Return],
        vec![I::Input(0), I::GetMember(Arc::from("shared")), I::Return],
        vec![
            I::Constant(Value::string("test:/resource")),
            I::ReadResource,
            I::GetProperty {
                name: Arc::from("shared"),
                owner_depth: 0,
                local: false,
            },
            I::Add,
            I::Return,
        ],
        vec![
            I::Constant(Value::string("test:/resource")),
            I::ReadResource,
            I::Return,
        ],
    ]);
    program.objects.push(vec![
        Member {
            flags: 0,
            entry: false,
            name: Arc::from("outer"),
            code: 3,
            local: false,
            method: false,
            ty: None,
            type_span: None,
            constant: false,
            fixed: false,
        },
        Member {
            flags: 0,
            entry: false,
            name: Arc::from("shared"),
            code: 4,
            local: false,
            method: false,
            ty: None,
            type_span: None,
            constant: false,
            fixed: false,
        },
    ]);
    let mut session = HostSession::new(Arc::new(program));
    session.start(0, vec![], 0, None).unwrap();
    let HostPoll::Ready(Ok(value)) = session.poll(0, 100) else {
        panic!("expected module")
    };
    (session, value)
}

#[test]
fn nested_contexts_share_member_cache_preserve_parent_and_reject_out_of_order_completion() {
    let (mut session, object) = shared_session();
    session
        .start(1, vec![object.clone()], 0, Some(100))
        .unwrap();
    let outer = request(&mut session, 0);
    session
        .reenter(&outer, 2, vec![object.clone()], 1, None)
        .unwrap();
    let inner = request(&mut session, 1);
    assert_ne!(outer, inner);
    assert_eq!(session.context_depth(), 2);
    assert_eq!(
        session
            .resume(&outer, Ok(Value::Int(500)), 1)
            .unwrap_err()
            .kind,
        ErrorKind::Host
    );
    assert_eq!(
        session
            .reenter(&outer, 0, vec![], 1, None)
            .unwrap_err()
            .kind,
        ErrorKind::Host
    );
    session.resume(&inner, Ok(Value::Int(2)), 2).unwrap();
    // Even after resume, the inner continuation must finish before the parent can resume.
    assert_eq!(
        session
            .resume(&outer, Ok(Value::Int(500)), 2)
            .unwrap_err()
            .kind,
        ErrorKind::Host
    );
    assert_eq!(session.poll(2, 100), HostPoll::Ready(Ok(Value::Int(2))));
    assert_eq!(session.context_depth(), 1);
    assert_eq!(request(&mut session, 2), outer);
    assert_eq!(
        session
            .resume(&inner, Ok(Value::Int(3)), 2)
            .unwrap_err()
            .kind,
        ErrorKind::Host
    );
    session.resume(&outer, Ok(Value::Int(40)), 2).unwrap();
    // No additional shared-member read: the inner completion populated this same object.
    assert_eq!(session.poll(2, 100), HostPoll::Ready(Ok(Value::Int(42))));
    session.start(2, vec![object], 3, None).unwrap();
    assert_eq!(session.poll(3, 100), HostPoll::Ready(Ok(Value::Int(2))));
    assert!(session.drain_cancelled_reads().is_empty());
}

#[test]
fn nested_failure_retries_and_failed_callback_attempt_preserves_parent_lazy_marker() {
    let (mut session, object) = shared_session();
    session.start(1, vec![object.clone()], 0, None).unwrap();
    let outer = request(&mut session, 0);
    // Reader callbacks may reevaluate a suspended member. Failed callback
    // attempts must not remove the older parent attempt's evaluating marker.
    for _ in 0..2 {
        session
            .reenter(&outer, 1, vec![object.clone()], 0, None)
            .unwrap();
        let nested = request(&mut session, 0);
        let error = RuntimeError::new(ErrorKind::Host, "callback attempt failed");
        session.resume(&nested, Err(error.clone()), 0).unwrap();
        assert_eq!(session.poll(0, 100), HostPoll::Ready(Err(error)));
        assert_eq!(request(&mut session, 0), outer);
    }
    session
        .reenter(&outer, 2, vec![object.clone()], 0, None)
        .unwrap();
    let failed = request(&mut session, 0);
    let error = RuntimeError::new(ErrorKind::Host, "nested transient\0error\r\n🙂");
    session.resume(&failed, Err(error.clone()), 0).unwrap();
    assert_eq!(session.poll(0, 100), HostPoll::Ready(Err(error)));
    assert_eq!(request(&mut session, 0), outer);
    session.reenter(&outer, 2, vec![object], 0, None).unwrap();
    let retry = request(&mut session, 0);
    assert_ne!(failed, retry);
    session.resume(&retry, Ok(Value::Int(2)), 0).unwrap();
    assert_eq!(session.poll(0, 100), HostPoll::Ready(Ok(Value::Int(2))));
    session.resume(&outer, Ok(Value::Int(40)), 0).unwrap();
    assert_eq!(session.poll(0, 100), HostPoll::Ready(Ok(Value::Int(42))));
}

#[test]
fn suspended_parent_roots_survive_collection_in_child() {
    let (mut session, object) = shared_session();
    session.start(1, vec![object], 0, None).unwrap();
    let outer = request(&mut session, 0);
    // The child has no object input: only the suspended parent keeps its object alive.
    session.reenter(&outer, 4, vec![], 0, None).unwrap();
    let inner = request(&mut session, 0);
    assert_eq!(session.collect(&[]).live, 1);
    session.resume(&inner, Ok(Value::Int(99)), 0).unwrap();
    assert_eq!(session.poll(0, 100), HostPoll::Ready(Ok(Value::Int(99))));
    session.resume(&outer, Ok(Value::Int(40)), 0).unwrap();
    let shared = request(&mut session, 0);
    session.resume(&shared, Ok(Value::Int(2)), 0).unwrap();
    assert_eq!(session.poll(0, 100), HostPoll::Ready(Ok(Value::Int(42))));
    assert_eq!(session.collect(&[]).live, 0);
}

#[test]
fn rejected_reentry_preserves_parent_and_its_original_deadline() {
    let mut session = reader();
    start(&mut session, 0, 10);
    let outer = request(&mut session, 0);
    assert_eq!(
        session
            .reenter(&outer, usize::MAX, vec![], 1, Some(100))
            .unwrap_err()
            .kind,
        ErrorKind::InvalidBytecode
    );
    assert_eq!(session.context_depth(), 1);
    assert_eq!(request(&mut session, 1), outer);
    let (_foreign_session, foreign_object) = shared_session();
    assert_eq!(
        session
            .reenter(&outer, 0, vec![foreign_object], 1, None)
            .unwrap_err()
            .kind,
        ErrorKind::StaleHandle
    );
    assert_eq!(session.context_depth(), 1);
    assert_eq!(request(&mut session, 1), outer);
    assert_eq!(
        session
            .reenter("foreign:token", 0, vec![], 2, None)
            .unwrap_err()
            .kind,
        ErrorKind::Host
    );
    assert_eq!(request(&mut session, 2), outer);
    assert_eq!(error_kind(session.poll(10, 100)), ErrorKind::Timeout);
    assert_eq!(session.drain_cancelled_reads(), vec![outer]);
}

#[test]
fn terminal_lifecycle_releases_all_contexts_and_drains_pending_reads_once() {
    for terminal in [ErrorKind::Cancelled, ErrorKind::Closed, ErrorKind::Timeout] {
        let (mut session, object) = shared_session();
        session.start(1, vec![object], 0, Some(10)).unwrap();
        let outer = request(&mut session, 0);
        session.reenter(&outer, 4, vec![], 1, Some(100)).unwrap();
        let inner = request(&mut session, 1);
        session.reenter(&inner, 4, vec![], 2, None).unwrap();
        let deepest = request(&mut session, 2);
        match terminal {
            ErrorKind::Cancelled => session.cancel(),
            ErrorKind::Closed => session.close(),
            ErrorKind::Timeout => assert_eq!(error_kind(session.poll(10, 100)), ErrorKind::Timeout),
            _ => unreachable!(),
        }
        assert_eq!(error_kind(session.poll(10, 100)), terminal);
        assert_eq!(session.context_depth(), 0);
        assert_eq!(session.heap_stats().live, 0);
        assert_eq!(session.heap_stats().slots, 0);
        let tokens = vec![outer, inner, deepest];
        assert_eq!(session.drain_cancelled_reads(), tokens);
        for token in tokens {
            assert_eq!(
                session
                    .resume(&token, Ok(Value::Int(1)), 10)
                    .unwrap_err()
                    .kind,
                ErrorKind::Closed
            );
        }
        session.close();
        session.cancel();
        assert!(session.drain_cancelled_reads().is_empty());
    }
}

#[test]
fn child_deadline_and_expired_reentry_close_the_whole_evaluator() {
    for expire_before_entry in [false, true] {
        let mut session = reader();
        start(&mut session, 0, 100);
        let outer = request(&mut session, 0);
        if expire_before_entry {
            assert_eq!(
                session
                    .reenter(&outer, 0, vec![], 100, None)
                    .unwrap_err()
                    .kind,
                ErrorKind::Timeout
            );
        } else {
            session.reenter(&outer, 0, vec![], 1, Some(0)).unwrap();
            assert_eq!(error_kind(session.poll(1, 100)), ErrorKind::Timeout);
        }
        assert!(session.is_closed());
        // Child did not issue a read, so only the suspended parent requires aborting.
        assert_eq!(session.drain_cancelled_reads(), vec![outer]);
        assert_eq!(session.context_depth(), 0);
    }
}

#[test]
fn bounded_context_stack_rejects_overflow_without_losing_any_parent() {
    let mut session = reader();
    start(&mut session, 0, 100);
    let mut tokens = vec![request(&mut session, 0)];
    for _ in 1..64 {
        session
            .reenter(
                tokens.last().unwrap(),
                0,
                vec![Value::string("test:/resource")],
                0,
                None,
            )
            .unwrap();
        let token = request(&mut session, 0);
        assert!(!tokens.contains(&token));
        tokens.push(token);
    }
    assert_eq!(session.context_depth(), 64);
    assert_eq!(
        session
            .reenter(tokens.last().unwrap(), 0, vec![], 0, None)
            .unwrap_err()
            .kind,
        ErrorKind::Busy
    );
    assert_eq!(request(&mut session, 0), *tokens.last().unwrap());
    for token in tokens.into_iter().rev() {
        session.resume(&token, Ok(Value::Int(2)), 0).unwrap();
        assert_eq!(session.poll(0, 100), HostPoll::Ready(Ok(Value::Int(42))));
    }
    assert_eq!(session.context_depth(), 0);
    assert!(session.drain_cancelled_reads().is_empty());
}

#[test]
fn close_after_child_error_drains_only_the_still_pending_parent() {
    let (mut session, object) = shared_session();
    session.start(1, vec![object], 0, None).unwrap();
    let outer = request(&mut session, 0);
    session.reenter(&outer, 4, vec![], 0, None).unwrap();
    let inner = request(&mut session, 0);
    let error = RuntimeError::new(ErrorKind::Host, "child failure");
    session.resume(&inner, Err(error.clone()), 0).unwrap();
    assert_eq!(session.poll(0, 100), HostPoll::Ready(Err(error)));
    assert_eq!(session.context_depth(), 1);
    assert_eq!(request(&mut session, 0), outer);
    session.close();
    assert_eq!(session.context_depth(), 0);
    assert_eq!(session.heap_stats().live, 0);
    assert_eq!(session.heap_stats().slots, 0);
    assert_eq!(session.drain_cancelled_reads(), vec![outer]);
    assert!(session.drain_cancelled_reads().is_empty());
}
