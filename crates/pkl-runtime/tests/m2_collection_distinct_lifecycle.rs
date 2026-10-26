//! Scheduling/ownership invariants; these are separate from Java raw compatibility.
use pkl_runtime::{
    host::{HostPoll, HostSession},
    parser::parse_module,
    resolver::resolve_module,
    value::{ErrorKind, RuntimeError, Value},
    vm::{Instruction as I, Program, compile_module},
};
use std::sync::Arc;

fn program(expression: &str) -> (Arc<Program>, usize, usize) {
    let source = format!(
        r#"
result = {expression}
side = List(7,7,8).distinct.length
"#
    );
    let mut program = (*compile_module(&resolve_module(&parse_module(&source).unwrap()).unwrap())
        .unwrap())
    .clone();
    let [result, side] = ["result", "side"].map(|name| {
        let block = program.blocks.len();
        program
            .blocks
            .push(vec![I::Input(0), I::GetMember(name.into()), I::Return]);
        block
    });
    (Arc::new(program), result, side)
}

fn event(session: &mut HostSession, roots: &[Value]) -> HostPoll {
    for _ in 0..200_000 {
        session.collect(roots);
        match session.poll(0, 1) {
            HostPoll::Yielded => {}
            event => return event,
        }
    }
    panic!("List transform Host poll bound")
}

fn ready(session: &mut HostSession, roots: &[Value]) -> Result<Value, RuntimeError> {
    match event(session, roots) {
        HostPoll::Ready(value) => value,
        event => panic!("unexpected {event:?}"),
    }
}

fn read(session: &mut HostSession, roots: &[Value], expected: &str) -> String {
    match event(session, roots) {
        HostPoll::Read { token, uri } => {
            assert_eq!(&*uri, expected);
            token
        }
        event => panic!("unexpected {event:?}"),
    }
}

const EXPRESSIONS: [&str; 2] = [
    r#"List(0,1,2,3,new Dynamic { n=read("mem:key") }).distinct.length"#,
    r#"(new Listing { 1; 1; read("mem:key") }).isDistinct"#,
];

#[test]
fn hash_and_shallow_force_suspend_reenter_fail_then_retry() {
    for (index, expression) in EXPRESSIONS.iter().enumerate() {
        let (program, result, side) = program(expression);
        let mut session = HostSession::new(program.clone());
        session.start(program.entry, vec![], 0, None).unwrap();
        let module = ready(&mut session, &[]).unwrap();
        let roots = [module.clone()];
        session
            .start(result, vec![module.clone()], 0, None)
            .unwrap();
        let token = read(&mut session, &roots, "mem:key");
        session
            .reenter(&token, side, vec![module.clone()], 0, None)
            .unwrap();
        assert_eq!(ready(&mut session, &roots), Ok(Value::Int(2.into())));
        assert_eq!(read(&mut session, &roots, "mem:key"), token);
        session
            .resume(
                &token,
                Err(RuntimeError::new(ErrorKind::Host, "retry hash")),
                0,
            )
            .unwrap();
        assert_eq!(
            ready(&mut session, &roots).unwrap_err().kind,
            ErrorKind::Host
        );
        session.start(result, vec![module], 0, None).unwrap();
        let token = read(&mut session, &roots, "mem:key");
        session
            .resume(&token, Ok(Value::Int(42.into())), 0)
            .unwrap();
        assert_eq!(
            ready(&mut session, &roots),
            Ok(if index == 0 {
                Value::Int(5.into())
            } else {
                Value::Bool(false)
            })
        );
        session.close();
        assert_eq!(session.collect(&[]).slots, 0);
    }
}

#[test]
fn cancel_and_close_release_visited_keys_and_pending_force() {
    for expression in EXPRESSIONS {
        for close in [false, true] {
            let (program, result, _) = program(expression);
            let mut session = HostSession::new(program.clone());
            session.start(program.entry, vec![], 0, None).unwrap();
            let module = ready(&mut session, &[]).unwrap();
            session
                .start(result, vec![module.clone()], 0, None)
                .unwrap();
            let token = read(&mut session, &[module], "mem:key");
            if close {
                session.close()
            } else {
                session.cancel()
            }
            assert_eq!(
                ready(&mut session, &[]).unwrap_err().kind,
                if close {
                    ErrorKind::Closed
                } else {
                    ErrorKind::Cancelled
                }
            );
            assert!(session.resume(&token, Ok(Value::Null), 0).is_err());
            assert_eq!(session.collect(&[]).slots, 0);
        }
    }
}

#[test]
fn successful_distinct_objects_survive_gc_and_repeated_evaluators_do_not_retain_keys() {
    // Distinct nested objects become reachable only through the result builder
    // after their construction frame returns; collect after every VM quantum.
    for _ in 0..24 {
        let (program, result, _) = program(
            r#"List(new Dynamic {n=1},new Dynamic {n=1},new Dynamic {n=2}).distinct.map((x)->x.n).toString()"#,
        );
        let mut session = HostSession::new(program.clone());
        session.start(program.entry, vec![], 0, None).unwrap();
        let module = ready(&mut session, &[]).unwrap();
        session
            .start(result, vec![module.clone()], 0, None)
            .unwrap();
        assert_eq!(
            ready(&mut session, &[module]),
            Ok(Value::string("List(1, 2)"))
        );
        session.close();
        assert_eq!(session.collect(&[]).slots, 0);
    }
}
