//! Logical custom-this slot lifetimes; separate from Java raw-byte compatibility evidence.
use std::sync::Arc;

use pkl_runtime::heap::LazyState;
use pkl_runtime::host::{HostPoll, HostSession};
use pkl_runtime::parser::parse_module;
use pkl_runtime::resolver::resolve_module;
use pkl_runtime::value::{ErrorKind, RuntimeError, Value};
use pkl_runtime::vm::{Instruction as I, Program, Vm, VmPoll, compile_module};

const SOURCE: &str = r#"
typealias Outer = Dynamic(
  (new Dynamic { marker = 41; callback = (ignored) -> marker }) is Dynamic(true)
  && read("mem:outer") == "ok" && this.marker == 41
  && this.callback.apply(null) == 41)
typealias Inner = Dynamic(
  (new Dynamic { marker = 77 }) is Dynamic(true)
  && read("mem:inner") == "ok" && this.marker == 77)
outerResult = let (checked: Outer = new Dynamic { marker = 1 }) checked.marker
innerResult = let (checked: Inner = new Dynamic { marker = 2 }) checked.marker
identity = Outer
snapshot = let (checked: Dynamic(
  let (f = (ignored) -> (new Dynamic { marker = 66 }) is Dynamic(true) && this.marker == 3)
    (new Dynamic { marker = 99 }) is Dynamic(true)
    && read("mem:snapshot") == "ok"
    && this.marker == 99 && f.apply(null) && this.marker == 99)
  = new Dynamic { marker = 3 }) checked.marker
letRoot = let (checked: Dynamic(
  let (original = this) (new Dynamic { marker = 88 }) is Dynamic(true)
    && this.marker == 88 && original.marker == 4,
  this.marker == 4) = new Dynamic { marker = 4 }) checked.marker
"#;

fn program() -> Program {
    let parsed = parse_module(SOURCE).unwrap();
    (*compile_module(&resolve_module(&parsed).unwrap()).unwrap()).clone()
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
    panic!("custom-this VM evaluation did not settle")
}

fn vm_ready(vm: &mut Vm) -> Result<Value, RuntimeError> {
    match vm_event(vm) {
        VmPoll::Ready(result) => result,
        event => panic!("expected completed VM context, got {event:?}"),
    }
}

fn vm_read(vm: &mut Vm, uri: &str) -> u64 {
    match vm_event(vm) {
        VmPoll::Pending(read) => {
            assert_eq!(&*read.uri, uri);
            vm.collect(&[]);
            read.id
        }
        event => panic!("expected {uri}, got {event:?}"),
    }
}

fn event(session: &mut HostSession, now: u64) -> HostPoll {
    for _ in 0..50_000 {
        match session.poll(now, 1) {
            HostPoll::Yielded => {
                session.collect(&[]);
            }
            event => return event,
        }
    }
    panic!("custom-this Host evaluation did not settle")
}

fn ready(session: &mut HostSession, now: u64) -> Result<Value, RuntimeError> {
    match event(session, now) {
        HostPoll::Ready(result) => result,
        event => panic!("expected completed Host context, got {event:?}"),
    }
}

fn read(session: &mut HostSession, now: u64, expected: &str) -> String {
    match event(session, now) {
        HostPoll::Read { token, uri } => {
            assert_eq!(&*uri, expected);
            session.collect(&[]);
            token
        }
        event => panic!("expected {expected}, got {event:?}"),
    }
}

fn setup() -> (HostSession, Value, [usize; 4]) {
    let mut program = program();
    let module_entry = program.entry;
    // Adapter blocks select real source members; constraints and Host reads are source compiled.
    let entries = ["outerResult", "innerResult", "snapshot", "letRoot"].map(|name| {
        let block = program.blocks.len();
        program
            .blocks
            .push(vec![I::Input(0), I::GetMember(name.into()), I::Return]);
        block
    });
    let mut session = HostSession::new(Arc::new(program));
    session.start(module_entry, vec![], 0, None).unwrap();
    let module = ready(&mut session, 0).unwrap();
    (session, module, entries)
}

#[test]
fn nested_check_slot_mutation_survives_gc_and_separate_host_reentry() {
    let (mut session, module, [outer, inner, _, _]) = setup();
    session.start(outer, vec![module.clone()], 1, None).unwrap();
    let parent = read(&mut session, 1, "mem:outer");
    session
        .reenter(&parent, inner, vec![module], 2, None)
        .unwrap();
    let child = read(&mut session, 2, "mem:inner");
    session.resume(&child, Ok(Value::string("ok")), 3).unwrap();
    // Successful checks return their original input, even though their slot changed.
    assert_eq!(ready(&mut session, 3), Ok(Value::Int(2)));
    assert_eq!(session.context_depth(), 1);
    assert_eq!(read(&mut session, 3, "mem:outer"), parent);
    session.resume(&parent, Ok(Value::string("ok")), 4).unwrap();
    assert_eq!(ready(&mut session, 4), Ok(Value::Int(1)));
    assert_eq!(session.context_depth(), 0);
    session.close();
    assert_eq!(session.heap_stats().slots, 0);
}

#[test]
fn lambda_receiver_is_a_creation_snapshot_and_let_root_mutation_stays_local() {
    let (mut session, module, [_, _, snapshot, let_root]) = setup();
    session
        .start(snapshot, vec![module.clone()], 1, None)
        .unwrap();
    let token = read(&mut session, 1, "mem:snapshot");
    session.resume(&token, Ok(Value::string("ok")), 2).unwrap();
    assert_eq!(ready(&mut session, 2), Ok(Value::Int(3)));
    session.start(let_root, vec![module], 3, None).unwrap();
    assert_eq!(ready(&mut session, 3), Ok(Value::Int(4)));
    session.close();
    assert_eq!(session.heap_stats().slots, 0);
}

#[test]
fn failed_child_retries_with_fresh_slots_and_rejects_old_read_tokens() {
    let (mut session, module, [outer, inner, _, _]) = setup();
    session.start(outer, vec![module.clone()], 1, None).unwrap();
    let parent = read(&mut session, 1, "mem:outer");
    let mut old: Option<String> = None;
    for (now, response) in [
        (
            2,
            Err(RuntimeError::new(ErrorKind::Host, "slot retry\0\r\n🙂")),
        ),
        (3, Ok(Value::string("wrong"))),
        (4, Ok(Value::string("ok"))),
    ] {
        session
            .reenter(&parent, inner, vec![module.clone()], now, None)
            .unwrap();
        let child = read(&mut session, now, "mem:inner");
        if let Some(previous) = &old {
            assert_ne!(&child, previous);
            assert_eq!(
                session
                    .resume(previous, Ok(Value::Null), now)
                    .unwrap_err()
                    .kind,
                ErrorKind::Host
            );
        }
        session.resume(&child, response, now).unwrap();
        let result = ready(&mut session, now);
        match now {
            2 => {
                let error = result.unwrap_err();
                assert_eq!(error.kind, ErrorKind::Host);
                assert_eq!(error.message, "slot retry\0\r\n🙂");
            }
            3 => assert_eq!(result.unwrap_err().kind, ErrorKind::Type),
            4 => assert_eq!(result, Ok(Value::Int(2))),
            _ => unreachable!(),
        }
        assert_eq!(read(&mut session, now, "mem:outer"), parent);
        old = Some(child);
    }
    session.resume(&parent, Ok(Value::string("ok")), 5).unwrap();
    assert_eq!(ready(&mut session, 5), Ok(Value::Int(1)));
    session.close();
    assert_eq!(session.heap_stats().slots, 0);
}

#[test]
fn terminal_events_release_both_mutated_slots_and_pending_reads() {
    for kind in [ErrorKind::Cancelled, ErrorKind::Closed, ErrorKind::Timeout] {
        let (mut session, module, [outer, inner, _, _]) = setup();
        session
            .start(
                outer,
                vec![module.clone()],
                1,
                (kind == ErrorKind::Timeout).then_some(5),
            )
            .unwrap();
        let parent = read(&mut session, 1, "mem:outer");
        session
            .reenter(&parent, inner, vec![module], 2, None)
            .unwrap();
        let child = read(&mut session, 2, "mem:inner");
        assert_eq!(session.context_depth(), 2);
        match kind {
            ErrorKind::Cancelled => session.cancel(),
            ErrorKind::Closed => session.close(),
            ErrorKind::Timeout => {}
            _ => unreachable!(),
        }
        let now = if kind == ErrorKind::Timeout { 6 } else { 3 };
        assert_eq!(ready(&mut session, now).unwrap_err().kind, kind);
        assert!(session.is_closed());
        assert_eq!(session.context_depth(), 0);
        assert_eq!(session.heap_stats().live, 0);
        assert_eq!(session.heap_stats().slots, 0);
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

#[test]
fn cancelling_reclaims_slot_capture_cycles_and_retry_keeps_alias_identity() {
    let mut program = program();
    let entry = program.entry;
    let scalar = program.blocks.len();
    program
        .blocks
        .push(vec![I::Constant(Value::Int(0)), I::Return]);
    let mut vm = Vm::new(Arc::new(program));
    vm.start(entry, vec![]).unwrap();
    let module = vm_ready(&mut vm).unwrap();
    let object = module.as_object().unwrap();
    vm.start_member(object, "identity").unwrap();
    let alias = vm_ready(&mut vm).unwrap();
    let baseline = vm.collect(&[]).live;
    let mut previous = None;
    for _ in 0..8 {
        vm.start_member(object, "outerResult").unwrap();
        let token = vm_read(&mut vm, "mem:outer");
        assert!(vm.collect(&[]).live > baseline);
        if let Some(old) = previous {
            assert_ne!(old, token);
            assert_eq!(
                vm.resume(old, Ok(Value::Null)).unwrap_err().kind,
                ErrorKind::Host
            );
        }
        vm.cancel();
        assert_eq!(vm_ready(&mut vm).unwrap_err().kind, ErrorKind::Cancelled);
        assert!(!vm.is_closed());
        assert_eq!(
            vm.member_state(object, "outerResult").unwrap(),
            LazyState::Unevaluated
        );
        assert_eq!(
            vm.resume(token, Ok(Value::Null)).unwrap_err().kind,
            ErrorKind::Host
        );
        vm.start(scalar, vec![]).unwrap();
        assert_eq!(vm_ready(&mut vm), Ok(Value::Int(0)));
        let collected = vm.collect(&[]);
        assert_eq!(collected.live, baseline);
        previous = Some(token);
    }
    vm.start_member(object, "identity").unwrap();
    assert_eq!(vm_ready(&mut vm), Ok(alias));
    vm.start_member(object, "outerResult").unwrap();
    let token = vm_read(&mut vm, "mem:outer");
    vm.resume(token, Ok(Value::string("ok"))).unwrap();
    assert_eq!(vm_ready(&mut vm), Ok(Value::Int(1)));
    vm.start(scalar, vec![]).unwrap();
    assert_eq!(vm_ready(&mut vm), Ok(Value::Int(0)));
    assert_eq!(vm.collect(&[]).live, baseline);
    vm.close();
    assert_eq!(vm.heap_stats().slots, 0);
}

#[test]
fn shared_immutable_program_keeps_mutated_slots_and_read_ids_per_vm() {
    let program = Arc::new(program());
    let mut left = Vm::new(program.clone());
    let mut right = Vm::new(program.clone());
    left.start(program.entry, vec![]).unwrap();
    let left_module = vm_ready(&mut left).unwrap();
    right.start(program.entry, vec![]).unwrap();
    let right_module = vm_ready(&mut right).unwrap();
    assert_eq!(
        right
            .start(program.entry, vec![left_module.clone()])
            .unwrap_err()
            .kind,
        ErrorKind::StaleHandle
    );
    left.start_member(left_module.as_object().unwrap(), "outerResult")
        .unwrap();
    let outer = vm_read(&mut left, "mem:outer");
    right
        .start_member(right_module.as_object().unwrap(), "innerResult")
        .unwrap();
    let inner = vm_read(&mut right, "mem:inner");
    left.collect(&[]);
    right.collect(&[]);
    right.resume(inner, Ok(Value::string("ok"))).unwrap();
    assert_eq!(vm_ready(&mut right), Ok(Value::Int(2)));
    right.close();
    assert_eq!(right.heap_stats().slots, 0);
    left.resume(outer, Ok(Value::string("ok"))).unwrap();
    assert_eq!(vm_ready(&mut left), Ok(Value::Int(1)));
    left.close();
    assert_eq!(left.heap_stats().slots, 0);
}
