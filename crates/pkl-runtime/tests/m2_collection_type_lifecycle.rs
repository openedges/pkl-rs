//! Abstract Collection checks retain concrete values and executable metadata.
//! Native GC/Host contracts are separate from pinned Java byte comparisons.
//! Retained module graphs include their one builtin Module parent; executable
//! nodes, transient-object deltas and terminal-zero assertions remain separate.
use std::sync::Arc;

use pkl_runtime::heap::LazyState;
use pkl_runtime::host::{HostPoll, HostSession};
use pkl_runtime::parser::parse_module;
use pkl_runtime::resolver::resolve_module;
use pkl_runtime::value::{ErrorKind, RuntimeError, Value};
use pkl_runtime::vm::{Instruction as I, Program, Vm, VmPoll, compile_module};

fn program(source: &str) -> (Arc<Program>, [usize; 3]) {
    let mut program = (*compile_module(&resolve_module(&parse_module(source).unwrap()).unwrap())
        .unwrap())
    .clone();
    let entries = ["parent", "child", "inspect"].map(|name| {
        let block = program.blocks.len();
        program
            .blocks
            .push(vec![I::Input(0), I::GetMember(name.into()), I::Return]);
        block
    });
    (Arc::new(program), entries)
}

fn vm_event(vm: &mut Vm) -> VmPoll {
    for _ in 0..200_000 {
        match vm.poll(1) {
            VmPoll::Yielded => {
                vm.collect(&[]);
            }
            event => return event,
        }
    }
    panic!("Collection VM event budget exceeded")
}

fn vm_ready(vm: &mut Vm) -> Result<Value, RuntimeError> {
    match vm_event(vm) {
        VmPoll::Ready(result) => result,
        event => panic!("expected VM result, got {event:?}"),
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

fn host_event(session: &mut HostSession, now: u64) -> HostPoll {
    for _ in 0..200_000 {
        match session.poll(now, 1) {
            HostPoll::Yielded => {
                session.collect(&[]);
            }
            event => return event,
        }
    }
    panic!("Collection Host event budget exceeded")
}

fn host_ready(session: &mut HostSession, now: u64) -> Result<Value, RuntimeError> {
    match host_event(session, now) {
        HostPoll::Ready(result) => result,
        event => panic!("expected Host result, got {event:?}"),
    }
}

fn host_read(session: &mut HostSession, now: u64, uri: &str) -> String {
    match host_event(session, now) {
        HostPoll::Read { token, uri: actual } => {
            assert_eq!(&*actual, uri);
            session.collect(&[]);
            token
        }
        event => panic!("expected {uri}, got {event:?}"),
    }
}

fn setup(source: &str) -> (HostSession, Value, [usize; 3]) {
    let (program, entries) = program(source);
    let entry = program.entry;
    let mut session = HostSession::new(program);
    session.start(entry, vec![], 0, None).unwrap();
    let module = host_ready(&mut session, 0).unwrap();
    (session, module, entries)
}

fn source(constructor: &str) -> String {
    format!(
        r#"
typealias Items<T> = Collection<Listing<T>>
parent = read("mem:outer")
source = {constructor}(new Listing {{ 7 }})
checked: Items<Int(read("mem:check") == "ok")> = source
child = checked.length
inspect = checked[0][0]
"#
    )
}

#[test]
fn list_checks_retain_lazy_nested_wrappers_while_set_checks_are_eager() {
    for constructor in ["List", "Set"] {
        let (mut session, module, [_, child, inspect]) = setup(&source(constructor));
        session.start(child, vec![module.clone()], 1, None).unwrap();
        if constructor == "Set" {
            let token = host_read(&mut session, 1, "mem:check");
            session.resume(&token, Ok(Value::string("ok")), 1).unwrap();
        }
        assert_eq!(host_ready(&mut session, 1), Ok(Value::Int(1)));
        if constructor == "List" {
            session.start(inspect, vec![module], 2, None).unwrap();
            let token = host_read(&mut session, 2, "mem:check");
            session.resume(&token, Ok(Value::string("ok")), 2).unwrap();
            assert_eq!(host_ready(&mut session, 2), Ok(Value::Int(7)));
        }
        session.close();
        assert_eq!(session.heap_stats().slots, 0);
    }
}

#[test]
fn suspended_constraint_makes_reentrant_collection_list_checks_eager_until_exit() {
    let source = r#"
parent: Int(read("mem:outer") == "ok") = 1
checked: Collection<Listing<Int>> = List(new Listing { "bad" })
child = checked.length
inspect = child
"#;
    let (mut session, module, [parent, child, _]) = setup(source);
    session
        .start(parent, vec![module.clone()], 1, None)
        .unwrap();
    let outer = host_read(&mut session, 1, "mem:outer");
    session
        .reenter(&outer, child, vec![module.clone()], 2, None)
        .unwrap();
    assert_eq!(
        host_ready(&mut session, 2).unwrap_err().kind,
        ErrorKind::Type
    );
    assert_eq!(session.context_depth(), 1);
    assert_eq!(host_read(&mut session, 2, "mem:outer"), outer);
    session.resume(&outer, Ok(Value::string("ok")), 3).unwrap();
    assert_eq!(host_ready(&mut session, 3), Ok(Value::Int(1)));
    session.start(child, vec![module], 4, None).unwrap();
    assert_eq!(host_ready(&mut session, 4), Ok(Value::Int(1)));
    session.close();
    assert_eq!(session.heap_stats().slots, 0);
}

#[test]
fn retained_collection_cast_keeps_temporary_receiver_and_lexical_captures() {
    let source = r#"
class Check {
  uri = "mem:check"
  text = "captured"
  function cast(): Collection<Listing<String(read(uri) == text && this == text)>> = List(new Listing { text })
}
parent = read("mem:outer")
checked = new Check {}.cast()
child = checked.length
inspect = checked[0][0]
"#;
    let (mut session, module, [parent, child, inspect]) = setup(source);
    session.start(child, vec![module.clone()], 1, None).unwrap();
    assert_eq!(host_ready(&mut session, 1), Ok(Value::Int(1)));
    session.collect(std::slice::from_ref(&module));
    session
        .start(parent, vec![module.clone()], 2, None)
        .unwrap();
    let outer = host_read(&mut session, 2, "mem:outer");
    session
        .reenter(&outer, inspect, vec![module.clone()], 3, None)
        .unwrap();
    let inner = host_read(&mut session, 3, "mem:check");
    session.collect(std::slice::from_ref(&module));
    session
        .resume(&inner, Ok(Value::string("captured")), 3)
        .unwrap();
    assert_eq!(host_ready(&mut session, 3), Ok(Value::string("captured")));
    assert_eq!(host_read(&mut session, 3, "mem:outer"), outer);
    session.resume(&outer, Ok(Value::Null), 4).unwrap();
    assert_eq!(host_ready(&mut session, 4), Ok(Value::Null));
    session.start(inspect, vec![module], 5, None).unwrap();
    assert_eq!(host_ready(&mut session, 5), Ok(Value::string("captured")));
    session.close();
    assert_eq!(session.heap_stats().live, 0);
    assert_eq!(session.heap_stats().slots, 0);
}

#[test]
fn failed_eager_check_retries_cached_set_during_parent_reentry() {
    let (mut session, module, [parent, child, _]) = setup(&source("Set"));
    session
        .start(parent, vec![module.clone()], 1, None)
        .unwrap();
    let outer = host_read(&mut session, 1, "mem:outer");
    session
        .reenter(&outer, child, vec![module.clone()], 2, None)
        .unwrap();
    let failed = host_read(&mut session, 2, "mem:check");
    let error = RuntimeError::new(ErrorKind::Host, "Collection retry\0\r\n🙂");
    session.resume(&failed, Err(error.clone()), 2).unwrap();
    let actual = host_ready(&mut session, 2).unwrap_err();
    assert_eq!((actual.kind, actual.message), (error.kind, error.message));
    assert_eq!(host_read(&mut session, 2, "mem:outer"), outer);
    session
        .reenter(&outer, child, vec![module.clone()], 3, None)
        .unwrap();
    let retry = host_read(&mut session, 3, "mem:check");
    assert_ne!(retry, failed);
    assert_eq!(
        session
            .resume(&failed, Ok(Value::Null), 3)
            .unwrap_err()
            .kind,
        ErrorKind::Host
    );
    session.resume(&retry, Ok(Value::string("ok")), 3).unwrap();
    assert_eq!(host_ready(&mut session, 3), Ok(Value::Int(1)));
    session
        .reenter(&outer, child, vec![module], 4, None)
        .unwrap();
    assert_eq!(host_ready(&mut session, 4), Ok(Value::Int(1)));
    session.resume(&outer, Ok(Value::Null), 4).unwrap();
    assert_eq!(host_ready(&mut session, 4), Ok(Value::Null));
    session.close();
    assert_eq!(session.heap_stats().slots, 0);
}

#[test]
fn repeated_nonterminal_cancel_reclaims_active_collection_type_checks() {
    for constructor in ["List", "Set"] {
        let (program, [_, child, inspect]) = program(&source(constructor));
        let mut vm = Vm::new(program.clone());
        vm.start(program.entry, vec![]).unwrap();
        let module = vm_ready(&mut vm).unwrap();
        let member = if constructor == "List" {
            "inspect"
        } else {
            "child"
        };
        let entry = if constructor == "List" {
            inspect
        } else {
            child
        };
        let mut retained = None;
        for _ in 0..24 {
            vm.start(entry, vec![module.clone()]).unwrap();
            let token = vm_read(&mut vm, "mem:check");
            vm.cancel();
            assert_eq!(vm_ready(&mut vm).unwrap_err().kind, ErrorKind::Cancelled);
            assert!(!vm.is_closed());
            assert_eq!(
                vm.member_state(module.as_object().unwrap(), member)
                    .unwrap(),
                LazyState::Unevaluated
            );
            assert_eq!(
                vm.resume(token, Ok(Value::Null)).unwrap_err().kind,
                ErrorKind::Host
            );
            let stats = vm.collect(std::slice::from_ref(&module));
            let live = (stats.live - stats.executable_live, stats.executable_live);
            assert_eq!(live, (if constructor == "List" { 8 } else { 5 }, 7));
            if let Some(expected) = retained {
                assert_eq!(live, expected);
            } else {
                retained = Some(live);
            }
        }
        println!("Collection {constructor} cancellation plateau: {retained:?}");
        vm.start(entry, vec![module]).unwrap();
        let token = vm_read(&mut vm, "mem:check");
        vm.resume(token, Ok(Value::string("ok"))).unwrap();
        assert_eq!(
            vm_ready(&mut vm),
            Ok(Value::Int(if constructor == "List" { 7 } else { 1 }))
        );
        vm.close();
        assert_eq!(vm.heap_stats().live, 0);
        assert_eq!(vm.heap_stats().slots, 0);
    }
}

#[test]
fn collection_wrappers_and_pending_actual_arguments_are_evaluator_local() {
    let (program, [_, child, inspect]) = program(&source("List"));
    let mut left = Vm::new(program.clone());
    let mut right = Vm::new(program.clone());
    left.start(program.entry, vec![]).unwrap();
    let left_module = vm_ready(&mut left).unwrap();
    right.start(program.entry, vec![]).unwrap();
    let right_module = vm_ready(&mut right).unwrap();
    left.start_member(left_module.as_object().unwrap(), "checked")
        .unwrap();
    let wrapper = vm_ready(&mut left).unwrap();
    assert_eq!(
        right.start(child, vec![wrapper.clone()]).unwrap_err().kind,
        ErrorKind::StaleHandle
    );
    left.start(inspect, vec![left_module.clone()]).unwrap();
    let left_read = vm_read(&mut left, "mem:check");
    right.start(inspect, vec![right_module]).unwrap();
    let right_read = vm_read(&mut right, "mem:check");
    right.resume(right_read, Ok(Value::string("ok"))).unwrap();
    assert_eq!(vm_ready(&mut right), Ok(Value::Int(7)));
    right.close();
    left.collect(std::slice::from_ref(&left_module));
    left.resume(left_read, Ok(Value::string("ok"))).unwrap();
    assert_eq!(vm_ready(&mut left), Ok(Value::Int(7)));
    left.close();
    assert_eq!(left.heap_stats().slots, 0);
    assert_eq!(right.heap_stats().slots, 0);
    assert_eq!(
        left.start(child, vec![wrapper]).unwrap_err().kind,
        ErrorKind::Closed
    );
}

#[test]
fn terminal_events_release_collection_wrappers_and_all_pending_reads() {
    for constructor in ["List", "Set"] {
        for kind in [ErrorKind::Cancelled, ErrorKind::Closed, ErrorKind::Timeout] {
            let (mut session, module, [parent, child, inspect]) = setup(&source(constructor));
            session
                .start(
                    parent,
                    vec![module.clone()],
                    1,
                    (kind == ErrorKind::Timeout).then_some(5),
                )
                .unwrap();
            let outer = host_read(&mut session, 1, "mem:outer");
            let entry = if constructor == "List" {
                inspect
            } else {
                child
            };
            session
                .reenter(&outer, entry, vec![module], 2, None)
                .unwrap();
            let inner = host_read(&mut session, 2, "mem:check");
            match kind {
                ErrorKind::Cancelled => session.cancel(),
                ErrorKind::Closed => session.close(),
                ErrorKind::Timeout => {}
                _ => unreachable!(),
            }
            let now = if kind == ErrorKind::Timeout { 6 } else { 3 };
            assert_eq!(host_ready(&mut session, now).unwrap_err().kind, kind);
            assert!(session.is_closed());
            assert_eq!(session.context_depth(), 0);
            assert_eq!(session.heap_stats().live, 0);
            assert_eq!(session.heap_stats().slots, 0);
            assert_eq!(
                session.drain_cancelled_reads(),
                [outer.clone(), inner.clone()]
            );
            for token in [outer, inner] {
                assert_eq!(
                    session
                        .resume(&token, Ok(Value::Null), now)
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
}
