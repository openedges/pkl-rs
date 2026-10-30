//! Parameterized recursive checks keep type arguments and roots across Host transitions.
//! Native lifecycle guarantees are separate from pinned Java byte comparisons.
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
local threshold = 0
typealias Shared<T> = Int(let (n = this) read("mem:shared") == "ok" && n is T && n > threshold)
typealias Inline<T> = Int(read("mem:inline") == "ok" && this is T && this > threshold)
sharedInt = 2 is Shared<Int>
sharedString = 3 is Shared<String>
inlineInt = 2 is Inline<Int>
inlineString = 3 is Inline<String>
typealias Countdown<T> = Int(this < 0 || (read("mem:step") == "ok" && this is T && (this - 1) is Countdown<T>))
typealias Grow<T> = Int(this < 0 || (read("mem:grow") == "ok" && (this - 1) is Grow<List<T>>))
outerResult = let (captured = new Dynamic { value = 42 }) let (n: Countdown<Int> = 2) captured.value
innerResult = let (n: Countdown<Int> = 1) 7
wrongArgument = 2 is Countdown<String>
changingArguments = let (n: Grow<Int> = 2) 5
identity = Countdown
check = (n) -> n is Countdown<Int>
"#;

fn program() -> (Arc<Program>, [usize; 3]) {
    let mut program = (*compile_module(&resolve_module(&parse_module(SOURCE).unwrap()).unwrap())
        .unwrap())
    .clone();
    // Adapter blocks select source members; all checks and reads come from Pkl.
    let entries = ["outerResult", "innerResult", "wrongArgument"].map(|name| {
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
    panic!("generic recursive check did not settle")
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
            assert_eq!(read.uri, expected);
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
    panic!("generic Host check did not settle")
}

fn host_ready(session: &mut HostSession, now: u64) -> Result<Value, RuntimeError> {
    match host_event(session, now) {
        HostPoll::Ready(result) => result,
        event => panic!("expected Host result, got {event:?}"),
    }
}

fn host_read(session: &mut HostSession, now: u64) -> String {
    match host_event(session, now) {
        HostPoll::Read { token, uri } => {
            assert_eq!(uri, "mem:step");
            session.collect(&[]);
            token
        }
        event => panic!("expected generic Host read, got {event:?}"),
    }
}

fn setup() -> (HostSession, Value, [usize; 3]) {
    let (program, entries) = program();
    let entry = program.entry;
    let mut session = HostSession::new(program);
    session.start(entry, vec![], 0, None).unwrap();
    let module = host_ready(&mut session, 0).unwrap();
    (session, module, entries)
}

#[test]
fn recursive_generic_reentry_preserves_arguments_and_suspended_capture_on_failure_retry() {
    let (mut session, module, [outer, inner, wrong]) = setup();
    session.start(outer, vec![module.clone()], 1, None).unwrap();
    let parent = host_read(&mut session, 1);
    session
        .reenter(&parent, wrong, vec![module.clone()], 2, None)
        .unwrap();
    let child = host_read(&mut session, 2);
    session.resume(&child, Ok(Value::string("ok")), 2).unwrap();
    assert_eq!(host_ready(&mut session, 2), Ok(Value::Bool(false)));
    assert_eq!(host_read(&mut session, 2), parent);
    session
        .reenter(&parent, inner, vec![module.clone()], 3, None)
        .unwrap();
    let failed = host_read(&mut session, 3);
    session
        .resume(
            &failed,
            Err(RuntimeError::new(ErrorKind::Host, "generic retry\0\r\n🙂")),
            3,
        )
        .unwrap();
    let error = host_ready(&mut session, 3).unwrap_err();
    assert_eq!(error.kind, ErrorKind::Host);
    assert_eq!(error.message, "generic retry\0\r\n🙂");
    assert_eq!(host_read(&mut session, 3), parent);
    session
        .reenter(&parent, inner, vec![module], 4, None)
        .unwrap();
    for _ in 0..2 {
        let token = host_read(&mut session, 4);
        assert_ne!(token, failed);
        session.resume(&token, Ok(Value::string("ok")), 4).unwrap();
    }
    assert_eq!(host_ready(&mut session, 4), Ok(Value::Int(7.into())));
    assert_eq!(host_read(&mut session, 4), parent);
    assert_eq!(
        session
            .resume(&failed, Ok(Value::Null), 4)
            .unwrap_err()
            .kind,
        ErrorKind::Host
    );
    let mut token = parent;
    for index in 0..3 {
        session.resume(&token, Ok(Value::string("ok")), 5).unwrap();
        if index < 2 {
            token = host_read(&mut session, 5);
        }
    }
    assert_eq!(host_ready(&mut session, 5), Ok(Value::Int(42.into())));
    assert_eq!(session.context_depth(), 0);
    session.close();
    assert_eq!(session.heap_stats().slots, 0);
}

#[test]
fn changing_recursive_arguments_remain_finite_and_repeated_cancel_reclaims_activations() {
    let (program, _) = program();
    let mut vm = Vm::new(program.clone());
    vm.start(program.entry, vec![]).unwrap();
    let module = vm_ready(&mut vm).unwrap();
    let initialized = vm.collect(&[]);
    let initialized_values = initialized.live - initialized.executable_live;
    let object = module.as_object().unwrap();
    let mut retained = None;
    for _ in 0..24 {
        vm.start_member(object, "changingArguments").unwrap();
        let first = vm_read(&mut vm, "mem:grow");
        vm.resume(first, Ok(Value::string("ok"))).unwrap();
        let second = vm_read(&mut vm, "mem:grow");
        vm.cancel();
        assert_eq!(vm_ready(&mut vm).unwrap_err().kind, ErrorKind::Cancelled);
        assert!(!vm.is_closed());
        assert_eq!(
            vm.member_state(object, "changingArguments").unwrap(),
            LazyState::Unevaluated
        );
        assert_eq!(
            vm.resume(second, Ok(Value::Null)).unwrap_err().kind,
            ErrorKind::Host
        );
        let stats = vm.collect(std::slice::from_ref(&module));
        // Separate ordinary values from persistent executable/type-use nodes.
        // Only the module, its Module parent and initialized Grow alias survive a canceled call.
        assert_eq!(stats.live - stats.executable_live, initialized_values + 1);
        assert!(stats.executable_live > 0);
        if let Some(expected) = retained {
            assert_eq!(stats.executable_live, expected);
        } else {
            retained = Some(stats.executable_live);
        }
    }
    vm.start_member(object, "changingArguments").unwrap();
    for _ in 0..3 {
        let token = vm_read(&mut vm, "mem:grow");
        vm.resume(token, Ok(Value::string("ok"))).unwrap();
    }
    assert_eq!(vm_ready(&mut vm), Ok(Value::Int(5.into())));
    vm.close();
    assert_eq!(vm.heap_stats().slots, 0);
}

#[test]
fn nonterminal_cancel_preserves_alias_identity_and_retries_deep_generic_prefix() {
    let (program, _) = program();
    let mut vm = Vm::new(program.clone());
    vm.start(program.entry, vec![]).unwrap();
    let module = vm_ready(&mut vm).unwrap();
    let object = module.as_object().unwrap();
    vm.start_member(object, "identity").unwrap();
    let identity = vm_ready(&mut vm).unwrap();
    vm.start_member(object, "outerResult").unwrap();
    let first = vm_read(&mut vm, "mem:step");
    vm.resume(first, Ok(Value::string("ok"))).unwrap();
    let stale = vm_read(&mut vm, "mem:step");
    vm.cancel();
    assert_eq!(vm_ready(&mut vm).unwrap_err().kind, ErrorKind::Cancelled);
    assert_eq!(
        vm.member_state(object, "outerResult").unwrap(),
        LazyState::Unevaluated
    );
    vm.collect(std::slice::from_ref(&module));
    vm.start_member(object, "identity").unwrap();
    assert_eq!(vm_ready(&mut vm), Ok(identity));
    vm.start_member(object, "outerResult").unwrap();
    for _ in 0..3 {
        let token = vm_read(&mut vm, "mem:step");
        assert_ne!(token, stale);
        assert_eq!(
            vm.resume(stale, Ok(Value::Null)).unwrap_err().kind,
            ErrorKind::Host
        );
        vm.resume(token, Ok(Value::string("ok"))).unwrap();
    }
    assert_eq!(vm_ready(&mut vm), Ok(Value::Int(42.into())));
    vm.close();
    assert_eq!(vm.heap_stats().slots, 0);
}

#[test]
fn shared_compilation_keeps_generic_state_and_pending_reads_evaluator_local() {
    let (program, _) = program();
    let mut left = Vm::new(program.clone());
    let mut right = Vm::new(program.clone());
    left.start(program.entry, vec![]).unwrap();
    let left_module = vm_ready(&mut left).unwrap();
    right.start(program.entry, vec![]).unwrap();
    let right_module = vm_ready(&mut right).unwrap();
    left.start_member(left_module.as_object().unwrap(), "outerResult")
        .unwrap();
    let left_token = vm_read(&mut left, "mem:step");
    assert_eq!(
        right
            .start_member(left_module.as_object().unwrap(), "outerResult")
            .unwrap_err()
            .kind,
        ErrorKind::StaleHandle
    );
    right
        .start_member(right_module.as_object().unwrap(), "wrongArgument")
        .unwrap();
    let right_token = vm_read(&mut right, "mem:step");
    right.resume(right_token, Ok(Value::string("ok"))).unwrap();
    assert_eq!(vm_ready(&mut right), Ok(Value::Bool(false)));
    right.close();
    assert_eq!(right.heap_stats().slots, 0);
    let mut token = left_token;
    for index in 0..3 {
        left.resume(token, Ok(Value::string("ok"))).unwrap();
        if index < 2 {
            token = vm_read(&mut left, "mem:step");
        }
    }
    assert_eq!(vm_ready(&mut left), Ok(Value::Int(42.into())));
    left.close();
    assert_eq!(left.heap_stats().slots, 0);
}

#[test]
fn terminal_events_release_generic_graphs_parent_and_child_reads() {
    for kind in [ErrorKind::Cancelled, ErrorKind::Closed, ErrorKind::Timeout] {
        let (mut session, module, [outer, inner, _]) = setup();
        session
            .start(
                outer,
                vec![module.clone()],
                1,
                (kind == ErrorKind::Timeout).then_some(5),
            )
            .unwrap();
        let parent = host_read(&mut session, 1);
        session
            .reenter(&parent, inner, vec![module], 2, None)
            .unwrap();
        let child = host_read(&mut session, 2);
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

#[test]
fn generic_let_target_shares_published_resolution_while_inline_arguments_remain_concrete() {
    let (program, _) = program();
    let mut vm = Vm::new(program.clone());
    vm.start(program.entry, vec![]).unwrap();
    let module = vm_ready(&mut vm).unwrap();
    let object = module.as_object().unwrap();
    // Pinned Java controls observe an unbound T in the shared let call target.
    // The first failed threshold lookup publishes resolution for other arguments.
    for (member, uri, expected) in [
        ("sharedInt", "mem:shared", None),
        ("sharedString", "mem:shared", Some(true)),
        ("sharedInt", "mem:shared", Some(true)),
        ("inlineString", "mem:inline", Some(false)),
        ("inlineInt", "mem:inline", None),
        ("inlineInt", "mem:inline", Some(true)),
    ] {
        vm.start_member(object, member).unwrap();
        let token = vm_read(&mut vm, uri);
        vm.resume(token, Ok(Value::string("ok"))).unwrap();
        let result = vm_ready(&mut vm);
        if let Some(value) = expected {
            assert_eq!(result, Ok(Value::Bool(value)));
        } else {
            let error = result.unwrap_err();
            assert_eq!(error.kind, ErrorKind::Type);
            assert_eq!(
                error.message,
                "Cannot reference property `threshold` from here because it is not `const`."
            );
            assert_eq!(
                vm.member_state(object, member).unwrap(),
                LazyState::Unevaluated
            );
        }
    }
    vm.close();
    assert_eq!(vm.heap_stats().slots, 0);
}

const DEFAULT_SOURCE: &str = r#"
class DefaultValue {
  p = read("mem:default")
  marker = 0
}
typealias ViaDefault<T> = Int((new Mapping<String,T> { ["key"] { marker = 7 } })["key"].p == this)
defaultOuter: ViaDefault<DefaultValue> = 42
defaultInner: ViaDefault<DefaultValue> = 7
"#;

fn default_setup() -> (HostSession, Value, [usize; 2]) {
    let mut program =
        (*compile_module(&resolve_module(&parse_module(DEFAULT_SOURCE).unwrap()).unwrap())
            .unwrap())
        .clone();
    let entry = program.entry;
    let entries = ["defaultOuter", "defaultInner"].map(|name| {
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

fn default_read(session: &mut HostSession, now: u64) -> String {
    match host_event(session, now) {
        HostPoll::Read { token, uri } => {
            assert_eq!(uri, "mem:default");
            session.collect(&[]);
            token
        }
        event => panic!("expected generic default member read, got {event:?}"),
    }
}

#[test]
fn generic_default_objects_keep_class_parents_and_receivers_across_host_retry_and_reentry() {
    let (mut session, module, [outer, inner]) = default_setup();
    // Both annotated members have resolved ViaDefault and DefaultValue.
    let initialized = session.collect(&[]);
    let initialized_values = initialized.live - initialized.executable_live;
    session.start(outer, vec![module.clone()], 1, None).unwrap();
    let parent = default_read(&mut session, 1);
    session
        .reenter(&parent, inner, vec![module.clone()], 2, None)
        .unwrap();
    let failed = default_read(&mut session, 2);
    session
        .resume(
            &failed,
            Err(RuntimeError::new(
                ErrorKind::Host,
                "generic default retry\0🙂",
            )),
            2,
        )
        .unwrap();
    let error = host_ready(&mut session, 2).unwrap_err();
    assert_eq!(error.kind, ErrorKind::Host);
    assert_eq!(error.message, "generic default retry\0🙂");
    assert_eq!(default_read(&mut session, 2), parent);
    session
        .reenter(&parent, inner, vec![module], 3, None)
        .unwrap();
    let child = default_read(&mut session, 3);
    session.resume(&child, Ok(Value::Int(7.into())), 3).unwrap();
    assert_eq!(host_ready(&mut session, 3), Ok(Value::Int(7.into())));
    assert_eq!(default_read(&mut session, 3), parent);
    assert_eq!(
        session
            .resume(&failed, Ok(Value::Null), 3)
            .unwrap_err()
            .kind,
        ErrorKind::Host
    );
    session
        .resume(&parent, Ok(Value::Int(42.into())), 4)
        .unwrap();
    assert_eq!(host_ready(&mut session, 4), Ok(Value::Int(42.into())));
    // The temporary amended defaults and Mapping objects have no persistent value cache.
    let stats = session.collect(&[]);
    assert_eq!(stats.live - stats.executable_live, initialized_values);
    session.close();
    assert_eq!(session.heap_stats().slots, 0);
}

#[test]
fn terminal_events_release_generic_default_objects_and_all_pending_reads() {
    for kind in [ErrorKind::Cancelled, ErrorKind::Closed, ErrorKind::Timeout] {
        let (mut session, module, [outer, inner]) = default_setup();
        session
            .start(
                outer,
                vec![module.clone()],
                1,
                (kind == ErrorKind::Timeout).then_some(5),
            )
            .unwrap();
        let parent = default_read(&mut session, 1);
        session
            .reenter(&parent, inner, vec![module], 2, None)
            .unwrap();
        let child = default_read(&mut session, 2);
        assert_eq!(session.context_depth(), 2);
        match kind {
            ErrorKind::Cancelled => session.cancel(),
            ErrorKind::Closed => session.close(),
            ErrorKind::Timeout => {}
            _ => unreachable!(),
        }
        let now = if kind == ErrorKind::Timeout { 6 } else { 3 };
        assert_eq!(host_ready(&mut session, now).unwrap_err().kind, kind);
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
    }
}
