//! Source-backed scheduling controls, separate from Java raw compatibility.
use pkl_runtime::{
    host::{HostPoll, HostSession},
    parser::parse_module,
    resolver::resolve_module,
    value::{ErrorKind, RuntimeError, Value},
    vm::{Instruction as I, Program, compile_module},
};
use std::sync::Arc;

fn program(source: &str) -> (Arc<Program>, usize, usize) {
    let mut p = (*compile_module(&resolve_module(&parse_module(source).unwrap()).unwrap())
        .unwrap())
    .clone();
    let [result, side] = ["result", "side"].map(|name| {
        let n = p.blocks.len();
        p.blocks
            .push(vec![I::Input(0), I::GetMember(name.into()), I::Return]);
        n
    });
    (Arc::new(p), result, side)
}
fn event(s: &mut HostSession, roots: &[Value]) -> HostPoll {
    for _ in 0..200_000 {
        s.collect(roots);
        match s.poll(0, 1) {
            HostPoll::Yielded => {}
            event => return event,
        }
    }
    panic!("contains poll bound")
}
fn ready(s: &mut HostSession, roots: &[Value]) -> Result<Value, RuntimeError> {
    match event(s, roots) {
        HostPoll::Ready(v) => v,
        event => panic!("unexpected {event:?}"),
    }
}
fn read(s: &mut HostSession, roots: &[Value], expected: &str) -> String {
    match event(s, roots) {
        HostPoll::Read { token, uri } => {
            assert_eq!(&*uri, expected);
            token
        }
        event => panic!("unexpected {event:?}"),
    }
}
const SOURCE: &str = r#"
stored = new Dynamic { n = read("mem:stored") }
query = new Dynamic { n = read("mem:query") }
tail = new Dynamic { n = read("mem:tail") }
result = List(stored, tail).contains(query)
side = List(1, 2).contains(2)
"#;

#[test]
fn stored_operand_precedes_query_and_pending_comparison_survives_reentry_retry_gc() {
    let (p, result, side) = program(SOURCE);
    let mut s = HostSession::new(p.clone());
    s.start(p.entry, vec![], 0, None).unwrap();
    let module = ready(&mut s, &[]).unwrap();
    let roots = [module.clone()];
    s.start(result, vec![module.clone()], 0, None).unwrap();
    let stored = read(&mut s, &roots, "mem:stored");
    s.reenter(&stored, side, vec![module.clone()], 0, None)
        .unwrap();
    assert_eq!(ready(&mut s, &roots), Ok(Value::Bool(true)));
    assert_eq!(read(&mut s, &roots, "mem:stored"), stored);
    s.resume(&stored, Ok(Value::Int(7.into())), 0).unwrap();
    let query = read(&mut s, &roots, "mem:query");
    s.resume(
        &query,
        Err(RuntimeError::new(ErrorKind::Host, "retry contains")),
        0,
    )
    .unwrap();
    assert_eq!(ready(&mut s, &roots).unwrap_err().kind, ErrorKind::Host);
    s.start(result, vec![module], 0, None).unwrap();
    let retried = read(&mut s, &roots, "mem:query");
    assert_ne!(query, retried);
    s.resume(&retried, Ok(Value::Int(7.into())), 0).unwrap();
    // First successful comparison terminates before tail's member is forced.
    assert_eq!(ready(&mut s, &roots), Ok(Value::Bool(true)));
    s.close();
    assert_eq!(s.collect(&[]).slots, 0);
}

#[test]
fn empty_and_identity_hit_do_not_force_query_or_later_member() {
    for (expression, expected) in [
        (
            r#"List().contains(new Dynamic { n=read("mem:unused") })"#,
            false,
        ),
        (
            r#"List(shared, new Dynamic { n=read("mem:tail") }).contains(shared)"#,
            true,
        ),
    ] {
        let source = format!(
            r#"shared = new Dynamic {{ n=read("mem:identity") }}
result = {expression}
side = true"#
        );
        let (p, result, _) = program(&source);
        let mut s = HostSession::new(p.clone());
        s.start(p.entry, vec![], 0, None).unwrap();
        let module = ready(&mut s, &[]).unwrap();
        s.start(result, vec![module.clone()], 0, None).unwrap();
        assert_eq!(ready(&mut s, &[module]), Ok(Value::Bool(expected)));
        s.close();
        assert_eq!(s.collect(&[]).slots, 0);
    }
}

#[test]
fn cancellation_at_either_operand_releases_cursor_and_comparison_roots() {
    for at_query in [false, true] {
        for close in [false, true] {
            let (p, result, _) = program(SOURCE);
            let mut s = HostSession::new(p.clone());
            s.start(p.entry, vec![], 0, None).unwrap();
            let module = ready(&mut s, &[]).unwrap();
            let roots = [module.clone()];
            s.start(result, vec![module], 0, None).unwrap();
            let mut token = read(&mut s, &roots, "mem:stored");
            if at_query {
                s.resume(&token, Ok(Value::Int(7.into())), 0).unwrap();
                token = read(&mut s, &roots, "mem:query");
            }
            if close {
                s.close()
            } else {
                s.cancel()
            }
            assert_eq!(
                ready(&mut s, &[]).unwrap_err().kind,
                if close {
                    ErrorKind::Closed
                } else {
                    ErrorKind::Cancelled
                }
            );
            assert!(s.resume(&token, Ok(Value::Null), 0).is_err());
            s.close();
            assert_eq!(s.collect(&[]).slots, 0);
        }
    }
}
