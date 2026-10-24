//! Ordered source type resolution and evaluator ownership. These heap/scheduler
//! contracts complement, and do not replace, raw pinned Java comparisons.
//! Retained module graphs include their one builtin Module parent; executable
//! nodes, transient-object deltas and terminal-zero assertions remain separate.
#[path = "support/constructor_baseline.rs"]
mod constructor_baseline;
#[path = "support/module_policy.rs"]
mod module_policy;
use constructor_baseline::compile_module_with_constructor_baseline;
use std::sync::Arc;
#[path = "support/eager_base.rs"]
mod eager_base;
use eager_base::Baseline;

use pkl_runtime::heap::LazyState;
use pkl_runtime::host::{HostPoll, HostSession};
use pkl_runtime::parser::parse_module;
use pkl_runtime::resolver::resolve_module;
use pkl_runtime::value::{ErrorKind, RuntimeError, Value};
use pkl_runtime::vm::{Instruction as I, Program, Vm, VmPoll};

fn program(source: &str) -> Arc<Program> {
    let parsed = parse_module(source).unwrap();
    let mut program =
        compile_module_with_constructor_baseline(&resolve_module(&parsed).unwrap()).unwrap();
    module_policy::register(Arc::make_mut(&mut program), source);
    program
}

fn ready(vm: &mut Vm) -> Result<Value, RuntimeError> {
    for _ in 0..50_000 {
        match vm.poll(1) {
            VmPoll::Ready(result) => return result,
            VmPoll::Yielded => {
                vm.collect(&[]);
            }
            VmPoll::Module(request) => vm
                .complete_module(
                    request.id,
                    pkl_runtime::modules::default_completion(&request)
                        .expect("default module policy"),
                )
                .unwrap(),
            event => panic!("unexpected VM event {event:?}"),
        }
    }
    panic!("type resolution did not settle")
}

fn member(vm: &mut Vm, module: &Value, name: &str) -> Result<Value, RuntimeError> {
    vm.start_member(module.as_object().unwrap(), name).unwrap();
    ready(vm)
}

const ORDERED_SOURCE: &str = r#"
typealias Poison = MissingInside
typealias Shadow<Poison> = List<Poison> | MissingGeneric
before = 1 is MissingFirst | Poison
after = 1 is Poison | MissingFirst
scalar = 7
temporary = let (item = new Dynamic { value = 42 }) item.value
"#;

#[test]
fn first_missing_type_leaves_later_declarations_unpublished_and_collectible() {
    let program = program(ORDERED_SOURCE);
    let mut vm = Vm::new(program.clone());
    vm.start(program.entry, vec![]).unwrap();
    let module = ready(&mut vm).unwrap();
    let initialized = Baseline::new(vm.collect(&[]));
    assert_eq!(
        initialized.added_values(vm.collect(std::slice::from_ref(&module)), 0),
        0
    );
    for _ in 0..2 {
        let error = member(&mut vm, &module, "before").unwrap_err();
        assert_eq!(error.kind, ErrorKind::MissingType);
        assert_eq!(error.message, "Cannot find type `MissingFirst`.");
        assert_eq!(
            vm.member_state(module.as_object().unwrap(), "Poison")
                .unwrap(),
            LazyState::Unevaluated
        );
        assert_eq!(
            initialized.added_values(vm.collect(std::slice::from_ref(&module)), 0),
            0
        );
    }
    let error = member(&mut vm, &module, "after").unwrap_err();
    assert_eq!(error.kind, ErrorKind::MissingType);
    assert_eq!(error.message, "Cannot find type `MissingInside`.");
    // Failure publishes exactly one alias singleton and its module environment.
    assert_eq!(
        initialized.added_values(vm.collect(std::slice::from_ref(&module)), 0),
        1
    );
    let alias = member(&mut vm, &module, "Poison").unwrap();
    assert!(matches!(alias, Value::TypeAlias(_)));
    assert_eq!(member(&mut vm, &module, "Poison"), Ok(alias.clone()));
    assert_eq!(
        member(&mut vm, &module, "temporary"),
        Ok(Value::Int(42.into()))
    );
    assert_eq!(member(&mut vm, &module, "scalar"), Ok(Value::Int(7.into())));
    // The temporary source object does not become a declaration root.
    assert_eq!(
        initialized.added_values(vm.collect(std::slice::from_ref(&module)), 1),
        1
    );
    assert_eq!(member(&mut vm, &module, "Poison"), Ok(alias));
    vm.close();
    assert_eq!(vm.heap_stats().live, 0);
    assert_eq!(vm.heap_stats().slots, 0);
}

#[test]
fn failed_generic_shadow_preserves_only_its_own_published_alias() {
    let program = program(ORDERED_SOURCE);
    let mut vm = Vm::new(program.clone());
    vm.start(program.entry, vec![]).unwrap();
    let module = ready(&mut vm).unwrap();
    let initialized = Baseline::new(vm.collect(&[]));
    let error = member(&mut vm, &module, "Shadow").unwrap_err();
    assert_eq!(error.kind, ErrorKind::MissingType);
    assert_eq!(error.message, "Cannot find type `MissingGeneric`.");
    assert_eq!(
        vm.member_state(module.as_object().unwrap(), "Poison")
            .unwrap(),
        LazyState::Unevaluated
    );
    assert_eq!(
        initialized.added_values(vm.collect(std::slice::from_ref(&module)), 0),
        1
    );
    let shadow = member(&mut vm, &module, "Shadow").unwrap();
    assert!(matches!(shadow, Value::TypeAlias(_)));
    assert_eq!(member(&mut vm, &module, "Shadow"), Ok(shadow));
    let error = member(&mut vm, &module, "Poison").unwrap_err();
    assert_eq!(error.message, "Cannot find type `MissingInside`.");
    assert_eq!(
        initialized.added_values(vm.collect(std::slice::from_ref(&module)), 0),
        2
    );
    vm.close();
    assert_eq!(vm.heap_stats().slots, 0);
}

#[test]
fn shared_program_does_not_share_failure_publication_or_generic_bindings() {
    let program = program(ORDERED_SOURCE);
    let mut left = Vm::new(program.clone());
    let mut right = Vm::new(program.clone());
    left.start(program.entry, vec![]).unwrap();
    let left_module = ready(&mut left).unwrap();
    let left_initialized = Baseline::new(left.collect(&[]));
    right.start(program.entry, vec![]).unwrap();
    let right_module = ready(&mut right).unwrap();
    let right_initialized = Baseline::new(right.collect(&[]));
    assert_eq!(
        member(&mut left, &left_module, "Shadow")
            .unwrap_err()
            .message,
        "Cannot find type `MissingGeneric`."
    );
    assert_eq!(
        left_initialized.added_values(left.collect(std::slice::from_ref(&left_module)), 0),
        1
    );
    assert_eq!(
        right_initialized.added_values(right.collect(std::slice::from_ref(&right_module)), 0),
        0
    );
    assert_eq!(
        right
            .member_state(right_module.as_object().unwrap(), "Shadow")
            .unwrap(),
        LazyState::Unevaluated
    );
    let left_alias = member(&mut left, &left_module, "Shadow").unwrap();
    assert_eq!(
        right
            .start(program.entry, vec![left_alias])
            .unwrap_err()
            .kind,
        ErrorKind::StaleHandle
    );
    assert_eq!(
        member(&mut right, &right_module, "before")
            .unwrap_err()
            .message,
        "Cannot find type `MissingFirst`."
    );
    assert_eq!(
        right_initialized.added_values(right.collect(std::slice::from_ref(&right_module)), 0),
        0
    );
    left.close();
    right.close();
    assert_eq!(left.heap_stats().slots, 0);
    assert_eq!(right.heap_stats().slots, 0);
}

#[test]
fn nested_generic_metadata_preserves_authored_aliases_without_forcing_bodies() {
    let program = program(
        r#"
import "pkl:ref"
typealias T = MissingGlobal
typealias Identity<T> = T
typealias Nested<T> = List<Identity<T>>
class D extends ref.Domain { function renderReference(r): String = "ok" }
class A { p: (Nested<Int>) -> Nested<String> = read("mem:must-not-run") }
class B { p: (Nested<Int>) -> Nested<String> = read("mem:must-not-run") }
class C { p: (List<Int>) -> List<String> = read("mem:must-not-run") }
local d = new D {}
same = ref.Reference(d, A, null).p == ref.Reference(d, B, null).p
distinct = ref.Reference(d, A, null).p != ref.Reference(d, C, null).p
"#,
    );
    let mut vm = Vm::new(program.clone());
    vm.start(program.entry, vec![]).unwrap();
    let module = ready(&mut vm).unwrap();
    assert_eq!(member(&mut vm, &module, "same"), Ok(Value::Bool(true)));
    assert_eq!(member(&mut vm, &module, "distinct"), Ok(Value::Bool(true)));
    assert_eq!(
        vm.member_state(module.as_object().unwrap(), "T").unwrap(),
        LazyState::Unevaluated
    );
    vm.collect(&[]);
    assert_eq!(member(&mut vm, &module, "same"), Ok(Value::Bool(true)));
    vm.close();
    assert_eq!(vm.heap_stats().slots, 0);
}

const HOST_SOURCE: &str = r#"
typealias T = MissingGlobal
typealias Gate<T> = T(this is T && read(if (this is String) "mem:inner" else "mem:outer") == "ok")
typealias Nested<T> = List<Gate<T>>
outerResult = let (captured = new Dynamic { value = 42 })
  let (values: Nested<Int> = List(captured.value)) values[0]
innerResult = let (values: Nested<String> = List("inner")) values[0]
bad = let (values: Nested<String> = List(42)) values[0]
identity = Nested
"#;

fn host_setup() -> (HostSession, Value, [usize; 4]) {
    let mut program = (*program(HOST_SOURCE)).clone();
    let entry = program.entry;
    // Adapter entries select source members; all type checks and reads below
    // come from the parsed source rather than injected semantic instructions.
    let entries = ["outerResult", "innerResult", "bad", "identity"].map(|name| {
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

fn host_event(session: &mut HostSession, now: u64) -> HostPoll {
    for _ in 0..50_000 {
        match session.poll(now, 1) {
            HostPoll::Yielded => {
                session.collect(&[]);
            }
            event => return event,
        }
    }
    panic!("nested generic evaluation did not settle")
}

fn host_ready(session: &mut HostSession, now: u64) -> Result<Value, RuntimeError> {
    match host_event(session, now) {
        HostPoll::Ready(result) => result,
        event => panic!("expected ready, got {event:?}"),
    }
}

fn host_read(session: &mut HostSession, now: u64, uri: &str) -> String {
    match host_event(session, now) {
        HostPoll::Read { token, uri: actual } => {
            assert_eq!(&*actual, uri);
            token
        }
        event => panic!("expected {uri}, got {event:?}"),
    }
}

#[test]
fn nested_generic_bindings_survive_host_reentry_failures_and_gc() {
    let (mut session, module, [outer, inner, bad, identity]) = host_setup();
    let initialized = Baseline::new(session.collect(&[]));
    session
        .start(identity, vec![module.clone()], 0, None)
        .unwrap();
    let alias = host_ready(&mut session, 0).unwrap();
    session.start(outer, vec![module.clone()], 1, None).unwrap();
    let parent = host_read(&mut session, 1, "mem:outer");
    let suspended = initialized.added_values(session.collect(&[]), 8);
    for (now, response) in [
        (
            2,
            Err(RuntimeError::new(ErrorKind::Host, "nested retry\0\r\n🙂")),
        ),
        (3, Ok(Value::string("wrong"))),
        (4, Ok(Value::string("ok"))),
    ] {
        session
            .reenter(&parent, inner, vec![module.clone()], now, None)
            .unwrap();
        let child = host_read(&mut session, now, "mem:inner");
        session
            .reenter(&child, bad, vec![module.clone()], now, None)
            .unwrap();
        assert_eq!(
            host_ready(&mut session, now).unwrap_err().kind,
            ErrorKind::Type
        );
        assert_eq!(host_read(&mut session, now, "mem:inner"), child);
        session.collect(&[]);
        session.resume(&child, response, now).unwrap();
        let result = host_ready(&mut session, now);
        match now {
            2 => assert_eq!(result.unwrap_err().message, "nested retry\0\r\n🙂"),
            3 => assert_eq!(result.unwrap_err().kind, ErrorKind::Type),
            4 => assert_eq!(result, Ok(Value::string("inner"))),
            _ => unreachable!(),
        }
        assert_eq!(
            session
                .resume(&child, Ok(Value::Null), now)
                .unwrap_err()
                .kind,
            ErrorKind::Host
        );
        assert_eq!(host_read(&mut session, now, "mem:outer"), parent);
    }
    session
        .reenter(&parent, identity, vec![module], 5, None)
        .unwrap();
    assert_eq!(host_ready(&mut session, 5), Ok(alias));
    session.resume(&parent, Ok(Value::string("ok")), 5).unwrap();
    assert_eq!(host_ready(&mut session, 5), Ok(Value::Int(42.into())));
    assert!(initialized.added_values(session.collect(&[]), 23) < suspended);
    session.close();
    assert_eq!(session.heap_stats().slots, 0);
}

#[test]
fn terminal_actions_release_nested_generic_captures_and_both_reads() {
    for termination in [ErrorKind::Cancelled, ErrorKind::Closed, ErrorKind::Timeout] {
        let (mut session, module, [outer, inner, _, _]) = host_setup();
        let initialized = Baseline::new(session.collect(&[]));
        session
            .start(
                outer,
                vec![module.clone()],
                1,
                (termination == ErrorKind::Timeout).then_some(5),
            )
            .unwrap();
        let parent = host_read(&mut session, 1, "mem:outer");
        session
            .reenter(&parent, inner, vec![module], 2, None)
            .unwrap();
        let child = host_read(&mut session, 2, "mem:inner");
        assert_eq!(session.context_depth(), 2);
        assert!(initialized.added_values(session.collect(&[]), 16) > 0);
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
fn nonterminal_cancel_keeps_alias_roots_and_retries_nested_bindings() {
    let program = program(HOST_SOURCE);
    let mut vm = Vm::new(program.clone());
    vm.start(program.entry, vec![]).unwrap();
    let module = ready(&mut vm).unwrap();
    let initialized = Baseline::new(vm.collect(&[]));
    let alias = member(&mut vm, &module, "identity").unwrap();
    let mut cancelled_request = None;
    for attempt in 0..2 {
        vm.start_member(module.as_object().unwrap(), "outerResult")
            .unwrap();
        let pending = loop {
            match vm.poll(1) {
                VmPoll::Pending(read) => {
                    assert_eq!(&*read.uri, "mem:outer");
                    break read.id;
                }
                VmPoll::Yielded => {
                    vm.collect(&[]);
                }
                event => panic!("expected generic constraint read, got {event:?}"),
            }
        };
        if attempt == 0 {
            assert!(initialized.added_values(vm.collect(&[]), 8) > 1);
            vm.cancel();
            assert_eq!(ready(&mut vm).unwrap_err().kind, ErrorKind::Cancelled);
            assert!(!vm.is_closed());
            assert_eq!(
                vm.member_state(module.as_object().unwrap(), "outerResult")
                    .unwrap(),
                LazyState::Unevaluated
            );
            assert_eq!(member(&mut vm, &module, "identity"), Ok(alias.clone()));
            // Nested, Gate, their declaration module and its Module parent survive cancellation;
            // the captured object and collection check temporaries do not.
            assert_eq!(initialized.added_values(vm.collect(&[]), 8), 2);
            cancelled_request = Some(pending);
        } else {
            assert_ne!(Some(pending), cancelled_request);
            assert_eq!(
                vm.resume(cancelled_request.unwrap(), Ok(Value::Null))
                    .unwrap_err()
                    .kind,
                ErrorKind::Host
            );
            vm.resume(pending, Ok(Value::string("ok"))).unwrap();
            assert_eq!(ready(&mut vm), Ok(Value::Int(42.into())));
        }
    }
    assert_eq!(member(&mut vm, &module, "identity"), Ok(alias));
    assert_eq!(initialized.added_values(vm.collect(&[]), 8), 2);
    vm.close();
    assert_eq!(vm.heap_stats().slots, 0);
}
