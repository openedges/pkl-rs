//! Nullable generic defaults retain their amendment parents across VM and Host transitions.
//! These native lifetime checks are separate from pinned Java byte comparisons.
//! Retained module graphs include their one builtin Module parent; executable
//! nodes, transient-object deltas and terminal-zero assertions remain separate.
use std::sync::Arc;

use pkl_runtime::heap::LazyState;
use pkl_runtime::host::{HostPoll, HostSession};
use pkl_runtime::parser::parse_module;
use pkl_runtime::resolver::resolve_module;
use pkl_runtime::value::{ErrorKind, RuntimeError, Value};
use pkl_runtime::vm::{Instruction as I, Program, Vm, VmPoll, compile_module};

const SOURCE: &str = r#"
class Outer {
  p = read("mem:outer")
  marker = 0
}
class Inner {
  p = read("mem:inner")
  marker = 0
}
typealias Pick<T> = T?
typealias Check<T> = Int((new Listing<Mapping<String,T>> {new {["key"] {marker = 9}}})[0]["key"].p == this)
outerResult: Check<Pick<Outer>> = 42
innerResult: Check<Pick<Inner>> = 7
"#;

fn program() -> (Arc<Program>, [usize; 2]) {
    let mut program = (*compile_module(&resolve_module(&parse_module(SOURCE).unwrap()).unwrap())
        .unwrap())
    .clone();
    let entries = ["outerResult", "innerResult"].map(|name| {
        let block = program.blocks.len();
        program
            .blocks
            .push(vec![I::Input(0), I::GetMember(name.into()), I::Return]);
        block
    });
    (Arc::new(program), entries)
}

fn vm_event(vm: &mut Vm) -> VmPoll {
    for _ in 0..100_000 {
        match vm.poll(1) {
            VmPoll::Yielded => {
                vm.collect(&[]);
            }
            event => return event,
        }
    }
    panic!("generic nullable default did not settle")
}

fn vm_ready(vm: &mut Vm) -> Result<Value, RuntimeError> {
    match vm_event(vm) {
        VmPoll::Ready(result) => result,
        event => panic!("expected VM result, got {event:?}"),
    }
}

fn vm_read(vm: &mut Vm, expected: &str) -> u64 {
    match vm_event(vm) {
        VmPoll::Pending(read) => {
            assert_eq!(&*read.uri, expected);
            vm.collect(&[]);
            read.id
        }
        event => panic!("expected {expected}, got {event:?}"),
    }
}

fn host_event(session: &mut HostSession, now: u64) -> HostPoll {
    for _ in 0..100_000 {
        match session.poll(now, 1) {
            HostPoll::Yielded => {
                session.collect(&[]);
            }
            event => return event,
        }
    }
    panic!("generic nullable Host default did not settle")
}

fn host_ready(session: &mut HostSession, now: u64) -> Result<Value, RuntimeError> {
    match host_event(session, now) {
        HostPoll::Ready(result) => result,
        event => panic!("expected Host result, got {event:?}"),
    }
}

fn host_read(session: &mut HostSession, now: u64, expected: &str) -> String {
    match host_event(session, now) {
        HostPoll::Read { token, uri } => {
            assert_eq!(&*uri, expected);
            session.collect(&[]);
            token
        }
        event => panic!("expected {expected}, got {event:?}"),
    }
}

fn setup() -> (HostSession, Value, [usize; 2]) {
    let (program, entries) = program();
    let entry = program.entry;
    let mut session = HostSession::new(program);
    session.start(entry, vec![], 0, None).unwrap();
    let module = host_ready(&mut session, 0).unwrap();
    (session, module, entries)
}

#[test]
fn nullable_defaults_keep_distinct_actual_classes_during_failed_child_reentry_and_retry() {
    let (mut session, module, [outer, inner]) = setup();
    session.start(outer, vec![module.clone()], 1, None).unwrap();
    let parent = host_read(&mut session, 1, "mem:outer");
    session
        .reenter(&parent, inner, vec![module.clone()], 2, None)
        .unwrap();
    let failed = host_read(&mut session, 2, "mem:inner");
    session
        .resume(
            &failed,
            Err(RuntimeError::new(ErrorKind::Host, "nullable retry\0\r\n🙂")),
            2,
        )
        .unwrap();
    let error = host_ready(&mut session, 2).unwrap_err();
    assert_eq!(error.kind, ErrorKind::Host);
    assert_eq!(error.message, "nullable retry\0\r\n🙂");
    assert_eq!(host_read(&mut session, 2, "mem:outer"), parent);
    session
        .reenter(&parent, inner, vec![module], 3, None)
        .unwrap();
    let child = host_read(&mut session, 3, "mem:inner");
    assert_ne!(child, failed);
    session.resume(&child, Ok(Value::Int(7)), 3).unwrap();
    assert_eq!(host_ready(&mut session, 3), Ok(Value::Int(7)));
    assert_eq!(host_read(&mut session, 3, "mem:outer"), parent);
    assert_eq!(
        session
            .resume(&failed, Ok(Value::Null), 3)
            .unwrap_err()
            .kind,
        ErrorKind::Host
    );
    session.resume(&parent, Ok(Value::Int(42)), 4).unwrap();
    assert_eq!(host_ready(&mut session, 4), Ok(Value::Int(42)));
    assert_eq!(session.context_depth(), 0);
    session.collect(&[]);
    session.close();
    assert_eq!(session.heap_stats().slots, 0);
}

#[test]
fn repeated_nonterminal_cancel_reclaims_nullable_defaults_without_losing_type_roots() {
    let (program, _) = program();
    let mut vm = Vm::new(program.clone());
    vm.start(program.entry, vec![]).unwrap();
    let module = vm_ready(&mut vm).unwrap();
    let object = module.as_object().unwrap();
    let mut retained = None;
    for _ in 0..24 {
        vm.start_member(object, "outerResult").unwrap();
        let token = vm_read(&mut vm, "mem:outer");
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
        let stats = vm.collect(std::slice::from_ref(&module));
        let live = (stats.live - stats.executable_live, stats.executable_live);
        // Only the module, its Module parent, two classes and two aliases survive.
        // Nullable wrappers, amended defaults, and intermediate collections are temporary.
        assert_eq!(live.0, 6);
        assert!(live.1 > 0);
        if let Some(expected) = retained {
            assert_eq!(live, expected);
        } else {
            retained = Some(live);
        }
    }
    vm.start_member(object, "outerResult").unwrap();
    let token = vm_read(&mut vm, "mem:outer");
    vm.resume(token, Ok(Value::Int(42))).unwrap();
    assert_eq!(vm_ready(&mut vm), Ok(Value::Int(42)));
    vm.close();
    assert_eq!(vm.heap_stats().slots, 0);
}

#[test]
fn nullable_default_parents_and_reads_are_evaluator_local() {
    let (program, _) = program();
    let mut left = Vm::new(program.clone());
    let mut right = Vm::new(program.clone());
    left.start(program.entry, vec![]).unwrap();
    let left_module = vm_ready(&mut left).unwrap();
    right.start(program.entry, vec![]).unwrap();
    let right_module = vm_ready(&mut right).unwrap();
    left.start_member(left_module.as_object().unwrap(), "outerResult")
        .unwrap();
    let left_token = vm_read(&mut left, "mem:outer");
    assert_eq!(
        right
            .start_member(left_module.as_object().unwrap(), "outerResult")
            .unwrap_err()
            .kind,
        ErrorKind::StaleHandle
    );
    right
        .start_member(right_module.as_object().unwrap(), "innerResult")
        .unwrap();
    let right_token = vm_read(&mut right, "mem:inner");
    right.resume(right_token, Ok(Value::Int(7))).unwrap();
    assert_eq!(vm_ready(&mut right), Ok(Value::Int(7)));
    right.close();
    assert_eq!(right.heap_stats().slots, 0);
    left.collect(std::slice::from_ref(&left_module));
    left.resume(left_token, Ok(Value::Int(42))).unwrap();
    assert_eq!(vm_ready(&mut left), Ok(Value::Int(42)));
    left.close();
    assert_eq!(left.heap_stats().slots, 0);
}

#[test]
fn cached_closure_keeps_nullable_collection_default_after_its_creator_returns() {
    let source = r#"
class C {
  p = read("mem:captured")
  marker = 0
}
class Holder { item: Mapping<String,C>? }
function make() = let (holder = new Holder {})
  let (captured = holder.item) (unused) -> ((captured) {["key"] {marker = 9}})["key"].p
saved = make()
result = saved.apply(0)
"#;
    let program = compile_module(&resolve_module(&parse_module(source).unwrap()).unwrap()).unwrap();
    let mut vm = Vm::new(program.clone());
    vm.start(program.entry, vec![]).unwrap();
    let module = vm_ready(&mut vm).unwrap();
    let object = module.as_object().unwrap();
    vm.start_member(object, "saved").unwrap();
    let saved = vm_ready(&mut vm).unwrap();
    assert!(matches!(saved, Value::Function(_)));
    vm.collect(std::slice::from_ref(&module));
    vm.start_member(object, "result").unwrap();
    let failed = vm_read(&mut vm, "mem:captured");
    vm.resume(
        failed,
        Err(RuntimeError::new(ErrorKind::Host, "captured default")),
    )
    .unwrap();
    assert_eq!(vm_ready(&mut vm).unwrap_err().kind, ErrorKind::Host);
    vm.collect(std::slice::from_ref(&module));
    vm.start_member(object, "saved").unwrap();
    assert_eq!(vm_ready(&mut vm), Ok(saved));
    vm.start_member(object, "result").unwrap();
    let token = vm_read(&mut vm, "mem:captured");
    assert_ne!(token, failed);
    vm.resume(token, Ok(Value::Int(42))).unwrap();
    assert_eq!(vm_ready(&mut vm), Ok(Value::Int(42)));
    vm.close();
    assert_eq!(vm.heap_stats().slots, 0);
}

#[test]
fn nullable_default_handles_cross_vm_inputs_and_host_completions_with_heap_validation() {
    let source = "class C { p = 42 }\nnullable: C?\n";
    let mut program = (*compile_module(&resolve_module(&parse_module(source).unwrap()).unwrap())
        .unwrap())
    .clone();
    let module_entry = program.entry;
    let amend = program.blocks.len();
    program.blocks.push(vec![
        I::Input(0),
        I::AmendEmpty,
        I::GetMember("p".into()),
        I::Return,
    ]);
    let read = program.blocks.len();
    program.blocks.push(vec![
        I::Constant(Value::string("mem:wrapper")),
        I::ReadResource,
        I::AmendEmpty,
        I::GetMember("p".into()),
        I::Return,
    ]);
    let identity = program.blocks.len();
    program.blocks.push(vec![I::Input(0), I::Return]);
    let transient = program.blocks.len();
    program.blocks.push(vec![
        I::Constant(Value::Int(1)),
        I::WrapNullableDefault { defined: true },
        I::Return,
    ]);
    let program = Arc::new(program);
    let mut left = Vm::new(program.clone());
    let mut right = Vm::new(program);
    left.start(module_entry, vec![]).unwrap();
    let module = vm_ready(&mut left).unwrap();
    left.start_member(module.as_object().unwrap(), "nullable")
        .unwrap();
    let nullable = vm_ready(&mut left).unwrap();
    assert!(matches!(nullable, Value::NullWithDefault(_)));
    assert_eq!(nullable, Value::Null);
    assert_eq!(
        right
            .start(identity, vec![nullable.clone()])
            .unwrap_err()
            .kind,
        ErrorKind::StaleHandle
    );
    right.start(module_entry, vec![]).unwrap();
    let right_module = vm_ready(&mut right).unwrap();
    right
        .start_member(right_module.as_object().unwrap(), "nullable")
        .unwrap();
    let right_nullable = vm_ready(&mut right).unwrap();
    left.start(amend, vec![nullable.clone()]).unwrap();
    assert_eq!(vm_ready(&mut left), Ok(Value::Int(42)));
    left.start(read, vec![]).unwrap();
    let token = vm_read(&mut left, "mem:wrapper");
    left.collect(&[module, nullable.clone()]);
    assert_eq!(
        left.resume(token, Ok(right_nullable)).unwrap_err().kind,
        ErrorKind::StaleHandle
    );
    left.resume(token, Ok(nullable.clone())).unwrap();
    assert_eq!(vm_ready(&mut left), Ok(Value::Int(42)));
    // A separate temporary wrapper is not retained by a module member cache.
    left.start(transient, vec![]).unwrap();
    let stale = vm_ready(&mut left).unwrap();
    assert!(matches!(stale, Value::NullWithDefault(_)));
    // Clear the last call's input/result roots before checking the stale wrapper.
    left.start(identity, vec![Value::Null]).unwrap();
    assert_eq!(vm_ready(&mut left), Ok(Value::Null));
    left.collect(&[]);
    assert_eq!(
        left.start(identity, vec![stale.clone()]).unwrap_err().kind,
        ErrorKind::StaleHandle
    );
    left.start(read, vec![]).unwrap();
    let token = vm_read(&mut left, "mem:wrapper");
    assert_eq!(
        left.resume(token, Ok(stale)).unwrap_err().kind,
        ErrorKind::StaleHandle
    );
    left.close();
    right.close();
    assert_eq!(left.heap_stats().slots, 0);
    assert_eq!(right.heap_stats().slots, 0);
}

#[test]
fn terminal_events_release_nullable_default_graphs_parent_and_child_reads() {
    for kind in [ErrorKind::Cancelled, ErrorKind::Closed, ErrorKind::Timeout] {
        let (mut session, module, [outer, inner]) = setup();
        session
            .start(
                outer,
                vec![module.clone()],
                1,
                (kind == ErrorKind::Timeout).then_some(5),
            )
            .unwrap();
        let parent = host_read(&mut session, 1, "mem:outer");
        session
            .reenter(&parent, inner, vec![module], 2, None)
            .unwrap();
        let child = host_read(&mut session, 2, "mem:inner");
        assert_eq!(session.context_depth(), 2);
        match kind {
            ErrorKind::Cancelled => session.cancel(),
            ErrorKind::Closed => session.close(),
            ErrorKind::Timeout => {}
            _ => unreachable!(),
        }
        let now = if kind == ErrorKind::Timeout { 6 } else { 3 };
        assert_eq!(host_ready(&mut session, now).unwrap_err().kind, kind);
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
