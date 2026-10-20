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
