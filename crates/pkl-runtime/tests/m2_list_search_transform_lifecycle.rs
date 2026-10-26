//! Scheduling/ownership invariants; these are separate from Java raw compatibility.
use pkl_runtime::{
    heap::LazyState,
    host::{HostPoll, HostSession},
    parser::parse_module,
    resolver::resolve_module,
    value::{ErrorKind, RuntimeError, Value},
    vm::{Instruction as I, Program, Vm, VmPoll, compile_module},
};
use std::sync::Arc;

fn program(expression: &str) -> (Arc<Program>, usize, usize) {
    let source = format!(
        r#"
result = {expression}
side = List(7).flatMap((x) -> List(x,x+1)).filter((x) -> x > 7).find((x) -> true)
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

const EXPRESSIONS: [&str; 3] = [
    r#"List(1,2,3).find((x) -> read("mem:callback/\(x)") == "yes")"#,
    r#"List(1,2,3).filter((x) -> read("mem:callback/\(x)") == "yes").toString()"#,
    r#"List(1,2,3).flatMap((x) -> List(read("mem:callback/\(x)"),x)).toString()"#,
];

#[test]
fn callbacks_reenter_then_fail_and_retry_without_losing_cursor_or_builder_roots() {
    for (index, expression) in EXPRESSIONS.iter().enumerate() {
        let (program, result, side) = program(expression);
        let mut session = HostSession::new(program.clone());
        session.start(program.entry, vec![], 0, None).unwrap();
        let module = ready(&mut session, &[]).unwrap();
        let roots = [module.clone()];
        session
            .start(result, vec![module.clone()], 0, None)
            .unwrap();
        let token = read(&mut session, &roots, "mem:callback/1");
        session
            .reenter(&token, side, vec![module.clone()], 0, None)
            .unwrap();
        assert_eq!(ready(&mut session, &roots), Ok(Value::Int(8.into())));
        assert_eq!(read(&mut session, &roots, "mem:callback/1"), token);
        session
            .resume(
                &token,
                Err(RuntimeError::new(ErrorKind::Host, "retry callback")),
                0,
            )
            .unwrap();
        assert_eq!(
            ready(&mut session, &roots).unwrap_err().kind,
            ErrorKind::Host
        );
        session.start(result, vec![module], 0, None).unwrap();
        let count: i64 = if index == 0 { 2 } else { 3 };
        for x in 1..=count {
            let token = read(&mut session, &roots, &format!("mem:callback/{x}"));
            let value = match index {
                0 => Value::string(if x == 2 { "yes" } else { "no" }),
                1 => Value::string(if x == 2 { "no" } else { "yes" }),
                _ => Value::Int((x * 10).into()),
            };
            session.resume(&token, Ok(value), 0).unwrap();
        }
        let expected = match index {
            0 => Value::Int(2.into()),
            1 => Value::string("List(1, 3)"),
            _ => Value::string("List(10, 1, 20, 2, 30, 3)"),
        };
        assert_eq!(ready(&mut session, &roots), Ok(expected));
        session.close();
        assert_eq!(session.collect(&[]).slots, 0);
    }
}

#[test]
fn cancellation_and_close_drop_each_callback_cursor_and_reject_stale_tokens() {
    for expression in EXPRESSIONS {
        for close in [false, true] {
            let (program, result, _) = program(expression);
            let mut session = HostSession::new(program.clone());
            session.start(program.entry, vec![], 0, None).unwrap();
            let module = ready(&mut session, &[]).unwrap();
            session
                .start(result, vec![module.clone()], 0, None)
                .unwrap();
            let token = read(&mut session, &[module], "mem:callback/1");
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
            assert!(session.resume(&token, Ok(Value::string("yes")), 0).is_err());
            assert_eq!(session.collect(&[]).slots, 0);
        }
    }
}

fn vm_event(vm: &mut Vm, roots: &[Value]) -> VmPoll {
    for _ in 0..200_000 {
        vm.collect(roots);
        match vm.poll(1) {
            VmPoll::Yielded => {}
            event => return event,
        }
    }
    panic!("List transform VM poll bound")
}

#[test]
fn flat_map_inner_result_survives_every_instruction_gc_and_cancelled_builder_is_reclaimable() {
    let (compiled, result, _) = program(
        r#"List(1,2).flatMap((x) ->
        List(new Dynamic { n=x }, new Dynamic { n=x+10 })).map((x) -> x.n).toString()"#,
    );
    let mut vm = Vm::new(compiled.clone());
    vm.start(compiled.entry, vec![]).unwrap();
    let module = match vm_event(&mut vm, &[]) {
        VmPoll::Ready(Ok(v)) => v,
        other => panic!("{other:?}"),
    };
    vm.start(result, vec![module.clone()]).unwrap();
    assert!(matches!(vm_event(&mut vm, &[module]), VmPoll::Ready(Ok(v))
        if v == Value::string("List(1, 11, 2, 12)")));
    vm.close();
    assert_eq!(vm.collect(&[]).slots, 0);

    // Newly allocated object elements must survive after their callback frame
    // returns. Cancellation happens at the next callback, after one inner drain.
    let expression = r#"List(1,2).flatMap((x) -> if (x == 1)
        List(new Dynamic { n=x }, new Dynamic { n=x+10 })
        else List(read("mem:second"))).map((x) -> x.n).toString()"#;
    let (program, result, _) = program(expression);
    let mut vm = Vm::new(program.clone());
    vm.start(program.entry, vec![]).unwrap();
    let module = match vm_event(&mut vm, &[]) {
        VmPoll::Ready(Ok(v)) => v,
        other => panic!("{other:?}"),
    };
    let roots = [module.clone()];

    // First observe the complete interval from starting the member until the
    // second callback suspends. This includes construction, callback return,
    // every inner drain step and the next outer call. No runtime internals or
    // private instruction names determine where cancellation is permitted.
    vm.start(result, vec![module.clone()]).unwrap();
    let mut yielded_prefixes = 0;
    let pending = loop {
        vm.collect(&roots);
        match vm.poll(1) {
            VmPoll::Yielded => {
                yielded_prefixes += 1;
                assert!(yielded_prefixes < 2_000, "bounded inner-drain interval");
            }
            VmPoll::Pending(request) => {
                assert_eq!(&*request.uri, "mem:second");
                break request.id;
            }
            other => panic!("unexpected before second callback: {other:?}"),
        }
    };
    assert!(yielded_prefixes > 0);
    vm.cancel();
    assert!(matches!(vm_event(&mut vm, &roots), VmPoll::Ready(Err(e))
        if e.kind == ErrorKind::Cancelled));
    assert!(vm.resume(pending, Ok(Value::Null)).is_err());

    // Exercise every yielded prefix, including those while the inner result
    // cursor owns the only reference to newly allocated objects. Warm caches
    // may reach the final pending read earlier; cancel that boundary as well.
    for prefix in 0..=yielded_prefixes {
        vm.start(result, vec![module.clone()]).unwrap();
        let mut pending = None;
        for _ in 0..prefix {
            vm.collect(&roots);
            match vm.poll(1) {
                VmPoll::Yielded => {}
                VmPoll::Pending(request) => {
                    assert_eq!(&*request.uri, "mem:second");
                    pending = Some(request.id);
                    break;
                }
                other => panic!("unexpected prefix {prefix}: {other:?}"),
            }
        }
        vm.cancel();
        assert!(matches!(vm_event(&mut vm, &roots), VmPoll::Ready(Err(e))
            if e.kind == ErrorKind::Cancelled));
        if let Some(token) = pending {
            assert!(vm.resume(token, Ok(Value::Null)).is_err());
        }
        assert_eq!(
            vm.member_state(module.as_object().unwrap(), "result")
                .unwrap(),
            LazyState::Unevaluated
        );
        vm.collect(&roots);
    }
    let mut plateau = None;
    for iteration in 0..24 {
        vm.start(result, vec![module.clone()]).unwrap();
        let token = match vm_event(&mut vm, &roots) {
            VmPoll::Pending(request) => {
                assert_eq!(&*request.uri, "mem:second");
                request.id
            }
            other => panic!("{other:?}"),
        };
        vm.cancel();
        assert!(
            matches!(vm_event(&mut vm, &roots), VmPoll::Ready(Err(e)) if e.kind==ErrorKind::Cancelled)
        );
        assert!(vm.resume(token, Ok(Value::Null)).is_err());
        assert_eq!(
            vm.member_state(module.as_object().unwrap(), "result")
                .unwrap(),
            LazyState::Unevaluated
        );
        let stats = vm.collect(&roots);
        let retained = (stats.live, stats.executable_live, stats.slots);
        if iteration == 3 {
            plateau = Some(retained);
        }
        if iteration >= 3 {
            assert_eq!(Some(retained), plateau);
        }
    }
    vm.close();
    assert_eq!(vm.collect(&[]).slots, 0);
}
