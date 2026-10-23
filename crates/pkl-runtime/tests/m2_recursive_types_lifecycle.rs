//! Recursive source constraints use the existing Host scheduler and traced heap.
//! Retained module graphs include their one builtin Module parent; executable
//! nodes, transient-object deltas and terminal-zero assertions remain separate.
use std::sync::Arc;
#[path = "support/eager_base.rs"]
mod eager_base;
use eager_base::Baseline;

use pkl_runtime::heap::LazyState;
use pkl_runtime::host::{HostPoll, HostSession};
use pkl_runtime::parser::parse_module;
use pkl_runtime::resolver::resolve_module;
use pkl_runtime::value::{ErrorKind, RuntimeError, Value};
use pkl_runtime::vm::{Instruction as I, Program, Vm, VmPoll, compile_module};

const SOURCE: &str = r#"
typealias Countdown = Int(this == 0 || (read("mem:step") == "ok" && (this - 1) is Countdown))
outerResult = let (captured = new Dynamic { value = 42 }) let (n: Countdown = 3) captured.value
innerResult = let (n: Countdown = 2) 7
identity = Countdown
"#;

fn program() -> Arc<Program> {
    compile_module(&resolve_module(&parse_module(SOURCE).unwrap()).unwrap()).unwrap()
}

fn vm_event(vm: &mut Vm) -> VmPoll {
    for _ in 0..50_000 {
        match vm.poll(1) {
            VmPoll::Yielded => {
                vm.collect(&[]);
            }
            event => return event,
        }
    }
    panic!("recursive constraint did not settle")
}

fn vm_ready(vm: &mut Vm) -> Result<Value, RuntimeError> {
    match vm_event(vm) {
        VmPoll::Ready(value) => value,
        event => panic!("expected result, got {event:?}"),
    }
}

fn vm_read(vm: &mut Vm) -> u64 {
    match vm_event(vm) {
        VmPoll::Pending(read) => {
            assert_eq!(&*read.uri, "mem:step");
            read.id
        }
        event => panic!("expected read, got {event:?}"),
    }
}

fn host_event(session: &mut HostSession, now: u64) -> HostPoll {
    for _ in 0..50_000 {
        match session.poll(now, 1) {
            HostPoll::Yielded => {
                session.collect(&[]);
            }
            event => return event,
        }
    }
    panic!("recursive Host constraint did not settle")
}

fn host_ready(session: &mut HostSession, now: u64) -> Result<Value, RuntimeError> {
    match host_event(session, now) {
        HostPoll::Ready(value) => value,
        event => panic!("expected result, got {event:?}"),
    }
}

fn host_read(session: &mut HostSession, now: u64) -> String {
    match host_event(session, now) {
        HostPoll::Read { token, uri } => {
            assert_eq!(&*uri, "mem:step");
            token
        }
        event => panic!("expected read, got {event:?}"),
    }
}

fn host_setup() -> (HostSession, Value, [usize; 2]) {
    let mut program = (*program()).clone();
    let entry = program.entry;
    // Adapter entries only select source members; recursive checks and reads
    // are compiled from the Pkl source above.
    let entries = ["outerResult", "innerResult"].map(|name| {
        let block = program.blocks.len();
        program
            .blocks
            .push(vec![I::Input(0), I::GetMember(name.into()), I::Return]);
        block
    });
    let mut session = HostSession::new(Arc::new(program));
    session.start(entry, vec![], 0, None).unwrap();
    let module = host_ready(&mut session, 0).unwrap();
    (session, module, entries)
}

#[test]
fn recursive_constraints_reenter_retry_and_keep_suspended_values_alive() {
    let (mut session, module, [outer, inner]) = host_setup();
    let initialized = Baseline::new(session.collect(&[]));
    session.start(outer, vec![module.clone()], 1, None).unwrap();
    let parent = host_read(&mut session, 1);
    session
        .reenter(&parent, inner, vec![module.clone()], 2, None)
        .unwrap();
    let failed = host_read(&mut session, 2);
    session
        .resume(
            &failed,
            Err(RuntimeError::new(ErrorKind::Host, "recursive retry\0🙂")),
            2,
        )
        .unwrap();
    assert_eq!(
        host_ready(&mut session, 2).unwrap_err().message,
        "recursive retry\0🙂"
    );
    assert_eq!(host_read(&mut session, 2), parent);
    session
        .reenter(&parent, inner, vec![module], 3, None)
        .unwrap();
    for _ in 0..2 {
        let child = host_read(&mut session, 3);
        assert_ne!(child, failed);
        session.collect(&[]);
        session.resume(&child, Ok(Value::string("ok")), 3).unwrap();
    }
    assert_eq!(host_ready(&mut session, 3), Ok(Value::Int(7.into())));
    assert_eq!(host_read(&mut session, 3), parent);
    assert_eq!(
        session
            .resume(&failed, Ok(Value::Null), 3)
            .unwrap_err()
            .kind,
        ErrorKind::Host
    );
    let mut request = parent;
    for step in 0..3 {
        session.collect(&[]);
        session
            .resume(&request, Ok(Value::string("ok")), 4)
            .unwrap();
        if step < 2 {
            request = host_read(&mut session, 4);
        }
    }
    assert_eq!(host_ready(&mut session, 4), Ok(Value::Int(42.into())));
    assert_eq!(initialized.added_values(session.collect(&[]), 30), 1);
    session.close();
    assert_eq!(session.heap_stats().slots, 0);
}

#[test]
fn cancelling_recursive_check_reclaims_temporaries_and_preserves_retry_identity() {
    let program = program();
    let mut vm = Vm::new(program.clone());
    vm.start(program.entry, vec![]).unwrap();
    let module = vm_ready(&mut vm).unwrap();
    let initialized = Baseline::new(vm.collect(&[]));
    let object = module.as_object().unwrap();
    vm.start_member(object, "identity").unwrap();
    let alias = vm_ready(&mut vm).unwrap();
    vm.start_member(object, "outerResult").unwrap();
    let old = vm_read(&mut vm);
    assert!(initialized.added_values(vm.collect(&[]), 5) > 0);
    vm.cancel();
    assert_eq!(vm_ready(&mut vm).unwrap_err().kind, ErrorKind::Cancelled);
    assert!(!vm.is_closed());
    assert_eq!(
        vm.member_state(object, "outerResult").unwrap(),
        LazyState::Unevaluated
    );
    assert_eq!(initialized.added_values(vm.collect(&[]), 5), 1);
    vm.start_member(object, "identity").unwrap();
    assert_eq!(vm_ready(&mut vm), Ok(alias));
    vm.start_member(object, "outerResult").unwrap();
    for _ in 0..3 {
        let request = vm_read(&mut vm);
        assert_ne!(request, old);
        assert_eq!(
            vm.resume(old, Ok(Value::Null)).unwrap_err().kind,
            ErrorKind::Host
        );
        vm.resume(request, Ok(Value::string("ok"))).unwrap();
    }
    assert_eq!(vm_ready(&mut vm), Ok(Value::Int(42.into())));
    assert_eq!(initialized.added_values(vm.collect(&[]), 17), 1);
    vm.close();
    assert_eq!(vm.heap_stats().slots, 0);
}

#[test]
fn terminal_actions_release_recursive_parent_and_child_reads() {
    for termination in [ErrorKind::Cancelled, ErrorKind::Closed, ErrorKind::Timeout] {
        let (mut session, module, [outer, inner]) = host_setup();
        let initialized = Baseline::new(session.collect(&[]));
        session
            .start(
                outer,
                vec![module.clone()],
                1,
                (termination == ErrorKind::Timeout).then_some(5),
            )
            .unwrap();
        let parent = host_read(&mut session, 1);
        session
            .reenter(&parent, inner, vec![module], 2, None)
            .unwrap();
        let child = host_read(&mut session, 2);
        assert_eq!(session.context_depth(), 2);
        assert!(initialized.added_values(session.collect(&[]), 10) > 0);
        match termination {
            ErrorKind::Cancelled => session.cancel(),
            ErrorKind::Closed => session.close(),
            ErrorKind::Timeout => {}
            _ => unreachable!(),
        }
        let now = if termination == ErrorKind::Timeout {
            6
        } else {
            3
        };
        assert_eq!(host_ready(&mut session, now).unwrap_err().kind, termination);
        assert!(session.is_closed());
        assert_eq!(session.heap_stats().slots, 0);
        assert_eq!(session.context_depth(), 0);
        assert_eq!(
            session.drain_cancelled_reads(),
            [parent.clone(), child.clone()]
        );
        for token in [parent, child] {
            assert_eq!(
                session
                    .resume(&token, Ok(Value::Null), now)
                    .unwrap_err()
                    .kind,
                ErrorKind::Closed
            );
        }
        session.close();
        assert!(session.drain_cancelled_reads().is_empty());
    }
}
