//! Class publication ownership and scheduler regressions. Heap counts and
//! injected instruction suspension below are Rust architecture assertions;
//! same-evaluator Java byte comparisons are recorded by the Stage 12 runner.
use std::sync::Arc;

use pkl_runtime::heap::{LazyState, Member};
use pkl_runtime::host::{HostPoll, HostSession};
use pkl_runtime::parser::parse_module;
use pkl_runtime::resolver::resolve_module;
use pkl_runtime::value::{ErrorKind, RuntimeError, Value};
use pkl_runtime::vm::{Instruction as I, Program, Vm, VmPoll, compile_module};

const SOURCE: &str = r#"
open class Base { fixed value: Int = 42 }
class Child extends Base {
  early: Int = captured.number
  value = 0
  late: Int = 99
}
const captured = new Dynamic { number = 10 }
wrapper = Child
instance = new Child { extra = 0 }
early = instance.early
late = instance.late
ordinary = let (temporary = new Dynamic { number = 1 }) read("mem:ordinary")
parent = read("mem:outer")
scalar = 7
"#;

fn program(source: &str) -> Program {
    let parsed = parse_module(source).unwrap();
    (*compile_module(&resolve_module(&parsed).unwrap()).unwrap()).clone()
}

fn ready(vm: &mut Vm) -> Result<Value, RuntimeError> {
    for _ in 0..50_000 {
        match vm.poll(1) {
            VmPoll::Ready(result) => return result,
            VmPoll::Yielded => {
                vm.collect(&[]);
            }
            result => panic!("unexpected VM event {result:?}"),
        }
    }
    panic!("evaluation failed to settle")
}

fn member(vm: &mut Vm, module: &Value, name: &str) -> Result<Value, RuntimeError> {
    vm.start_member(module.as_object().unwrap(), name).unwrap();
    ready(vm)
}

#[test]
fn partial_singleton_survives_failure_scalar_gc_and_wrapper_retry() {
    let mut program = program(SOURCE);
    let module_entry = program.entry;
    let scalar = program.blocks.len();
    program
        .blocks
        .push(vec![I::Constant(Value::Int(7)), I::Return]);
    let empty_template = program.objects.len();
    program.objects.push(vec![Member {
        entry: false,
        name: "early".into(),
        code: scalar,
        local: false,
        method: false,
        ty: None,
        type_span: None,
        constant: false,
        fixed: false,
    }]);
    let disposable = program.blocks.len();
    program.blocks.push(vec![
        I::Input(0),
        I::MakeInstance {
            template: empty_template,
        },
        I::Return,
    ]);
    let mut vm = Vm::new(Arc::new(program));
    vm.start(module_entry, vec![]).unwrap();
    let module = ready(&mut vm).unwrap();
    assert_eq!(
        member(&mut vm, &module, "wrapper").unwrap_err().kind,
        ErrorKind::Type
    );
    assert_eq!(
        vm.member_state(module.as_object().unwrap(), "wrapper")
            .unwrap(),
        LazyState::Unevaluated
    );
    // Exactly the captured module, Base and partial Child survive the failure.
    assert_eq!(vm.collect(&[]).live, 3);
    vm.start(scalar, vec![]).unwrap();
    assert_eq!(ready(&mut vm), Ok(Value::Int(7)));
    assert_eq!(vm.collect(&[]).live, 3);
    let child = member(&mut vm, &module, "wrapper").unwrap();
    assert!(matches!(child, Value::Class(_)));
    assert_eq!(member(&mut vm, &module, "Child"), Ok(child.clone()));
    assert_eq!(
        vm.member_state(module.as_object().unwrap(), "wrapper")
            .unwrap(),
        LazyState::Completed(child.clone())
    );
    vm.start(disposable, vec![child.clone()]).unwrap();
    let transient = ready(&mut vm).unwrap();
    assert_eq!(vm.collect(&[]).live, 4);
    vm.start(scalar, vec![]).unwrap();
    assert_eq!(ready(&mut vm), Ok(Value::Int(7)));
    let collected = vm.collect(&[]);
    assert_eq!(collected.live, 3);
    assert!(collected.reclaimed >= 1);
    assert_eq!(
        vm.start(disposable, vec![transient]).unwrap_err().kind,
        ErrorKind::StaleHandle
    );
    assert_eq!(member(&mut vm, &module, "early"), Ok(Value::Int(10)));
    // The cached instance and its captured Dynamic are now additionally rooted
    // through the class's captured module, rather than as temporary builders.
    assert_eq!(vm.collect(&[]).live, 5);
    assert_eq!(
        member(&mut vm, &module, "late").unwrap_err().kind,
        ErrorKind::MissingMember
    );
    assert_eq!(vm.collect(&[]).live, 5);
    assert_eq!(member(&mut vm, &module, "Child"), Ok(child));
    vm.close();
    assert_eq!(vm.heap_stats().live, 0);
    assert_eq!(vm.heap_stats().slots, 0);
}

#[test]
fn class_free_evaluations_still_release_unrooted_modules() {
    let program = Arc::new(program("scalar = 7"));
    let mut vm = Vm::new(program.clone());
    vm.start(program.entry, vec![]).unwrap();
    let module = ready(&mut vm).unwrap();
    assert_eq!(member(&mut vm, &module, "scalar"), Ok(Value::Int(7)));
    assert_eq!(vm.collect(&[]).live, 0);
}

#[test]
fn shared_program_has_evaluator_local_singletons_and_rejects_foreign_handles() {
    let program = Arc::new(program("class Item { value = 42 }"));
    let mut left = Vm::new(program.clone());
    let mut right = Vm::new(program.clone());
    left.start(program.entry, vec![]).unwrap();
    let left_module = ready(&mut left).unwrap();
    right.start(program.entry, vec![]).unwrap();
    let right_module = ready(&mut right).unwrap();
    let left_class = member(&mut left, &left_module, "Item").unwrap();
    let right_class = member(&mut right, &right_module, "Item").unwrap();
    assert_ne!(left_class, right_class);
    assert_eq!(
        right
            .start(program.entry, vec![left_class.clone()])
            .unwrap_err()
            .kind,
        ErrorKind::StaleHandle
    );
    left.start(program.entry, vec![]).unwrap();
    let second_module = ready(&mut left).unwrap();
    // This is the development Program/template identity contract; it does not
    // claim Java module-loading semantics for repeated entry execution.
    assert_eq!(member(&mut left, &second_module, "Item"), Ok(left_class));
    left.close();
    right.close();
    assert_eq!(left.heap_stats().slots, 0);
    assert_eq!(right.heap_stats().slots, 0);
}

fn host_setup() -> (HostSession, Value, [usize; 4]) {
    let mut program = program(SOURCE);
    let module_entry = program.entry;
    let entries = ["wrapper", "Child", "ordinary", "parent"].map(|name| {
        let entry = program.blocks.len();
        program
            .blocks
            .push(vec![I::Input(0), I::GetMember(name.into()), I::Return]);
        entry
    });
    let mut session = HostSession::new(Arc::new(program));
    session.start(module_entry, vec![], 0, None).unwrap();
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
    panic!("Host evaluation failed to settle")
}

fn host_ready(session: &mut HostSession, now: u64) -> Result<Value, RuntimeError> {
    match host_event(session, now) {
        HostPoll::Ready(result) => result,
        event => panic!("expected ready, got {event:?}"),
    }
}

fn request(session: &mut HostSession, now: u64, expected_uri: &str) -> String {
    match host_event(session, now) {
        HostPoll::Read { token, uri } => {
            assert_eq!(&*uri, expected_uri);
            token
        }
        event => panic!("expected read, got {event:?}"),
    }
}

#[test]
fn partial_class_identity_crosses_contexts_while_ordinary_failure_retries_and_reclaims_builder() {
    let (mut session, module, [wrapper, class, ordinary, parent]) = host_setup();
    session
        .start(wrapper, vec![module.clone()], 0, None)
        .unwrap();
    assert_eq!(
        host_ready(&mut session, 0).unwrap_err().kind,
        ErrorKind::Type
    );
    assert_eq!(session.collect(&[]).live, 3);
    session.start(class, vec![module.clone()], 0, None).unwrap();
    let partial = host_ready(&mut session, 0).unwrap();
    session
        .start(parent, vec![module.clone()], 1, None)
        .unwrap();
    let outer = request(&mut session, 1, "mem:outer");
    session
        .reenter(&outer, ordinary, vec![module.clone()], 2, None)
        .unwrap();
    let failed = request(&mut session, 2, "mem:ordinary");
    assert_eq!(session.collect(&[]).live, 4);
    session
        .reenter(&failed, class, vec![module.clone()], 2, None)
        .unwrap();
    assert_eq!(host_ready(&mut session, 2), Ok(partial.clone()));
    let failure = RuntimeError::new(ErrorKind::Host, "retry ordinary lazy property");
    session.resume(&failed, Err(failure.clone()), 2).unwrap();
    let error = host_ready(&mut session, 2).unwrap_err();
    assert_eq!((error.kind, error.message), (failure.kind, failure.message));
    assert_eq!(session.collect(&[]).live, 3);
    session
        .reenter(&outer, ordinary, vec![module.clone()], 3, None)
        .unwrap();
    let retry = request(&mut session, 3, "mem:ordinary");
    assert_ne!(failed, retry);
    assert_eq!(
        session
            .resume(&failed, Ok(Value::Null), 3)
            .unwrap_err()
            .kind,
        ErrorKind::Host
    );
    session.resume(&retry, Ok(Value::Int(42)), 3).unwrap();
    assert_eq!(host_ready(&mut session, 3), Ok(Value::Int(42)));
    assert_eq!(session.collect(&[]).live, 3);
    session
        .reenter(&outer, class, vec![module], 4, None)
        .unwrap();
    assert_eq!(host_ready(&mut session, 4), Ok(partial));
    session.resume(&outer, Ok(Value::Null), 4).unwrap();
    assert_eq!(host_ready(&mut session, 4), Ok(Value::Null));
    assert_eq!(session.collect(&[]).live, 3);
    assert!(session.drain_cancelled_reads().is_empty());
    session.close();
    assert_eq!(session.heap_stats().slots, 0);
}

#[test]
fn terminal_events_release_partial_singletons_and_nested_pending_reads() {
    for termination in [ErrorKind::Cancelled, ErrorKind::Closed, ErrorKind::Timeout] {
        let (mut session, module, [wrapper, _, ordinary, parent]) = host_setup();
        session
            .start(wrapper, vec![module.clone()], 0, None)
            .unwrap();
        assert_eq!(
            host_ready(&mut session, 0).unwrap_err().kind,
            ErrorKind::Type
        );
        session
            .start(
                parent,
                vec![module.clone()],
                1,
                (termination == ErrorKind::Timeout).then_some(5),
            )
            .unwrap();
        let outer = request(&mut session, 1, "mem:outer");
        session
            .reenter(&outer, ordinary, vec![module], 2, None)
            .unwrap();
        let inner = request(&mut session, 2, "mem:ordinary");
        assert_eq!(session.collect(&[]).live, 4);
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
        assert!(session.drain_cancelled_reads().is_empty());
    }
}

#[test]
fn injected_initialization_read_exposes_prefix_without_advancing_parent_cursor() {
    let mut program = program(SOURCE);
    let module_entry = program.entry;
    inject_initialization_read(&mut program);
    let [wrapper, class, early, late] = ["wrapper", "Child", "early", "late"].map(|name| {
        let entry = program.blocks.len();
        program
            .blocks
            .push(vec![I::Input(0), I::GetMember(name.into()), I::Return]);
        entry
    });
    let mut session = HostSession::new(Arc::new(program));
    session.start(module_entry, vec![], 0, None).unwrap();
    let module = host_ready(&mut session, 0).unwrap();
    session
        .start(wrapper, vec![module.clone()], 1, None)
        .unwrap();
    let initialization = request(&mut session, 1, "mem:initialization");
    assert_eq!(session.collect(&[]).live, 3);
    session
        .reenter(&initialization, class, vec![module.clone()], 2, None)
        .unwrap();
    let partial = host_ready(&mut session, 2).unwrap();
    session
        .reenter(&initialization, early, vec![module.clone()], 2, None)
        .unwrap();
    assert_eq!(host_ready(&mut session, 2), Ok(Value::Int(10)));
    session
        .reenter(&initialization, late, vec![module.clone()], 2, None)
        .unwrap();
    assert_eq!(
        host_ready(&mut session, 2).unwrap_err().kind,
        ErrorKind::MissingMember
    );
    assert_eq!(session.collect(&[]).live, 5);
    session.resume(&initialization, Ok(Value::Null), 3).unwrap();
    assert_eq!(
        host_ready(&mut session, 3).unwrap_err().kind,
        ErrorKind::Type
    );
    assert_eq!(session.collect(&[]).live, 5);
    session.start(class, vec![module], 4, None).unwrap();
    assert_eq!(host_ready(&mut session, 4), Ok(partial));
    session.close();
    assert_eq!(session.heap_stats().slots, 0);
}

#[test]
fn development_vm_cancel_unwinds_temporary_work_but_keeps_published_class_identity() {
    let program = Arc::new(program(SOURCE));
    let mut vm = Vm::new(program.clone());
    vm.start(program.entry, vec![]).unwrap();
    let module = ready(&mut vm).unwrap();
    assert_eq!(
        member(&mut vm, &module, "wrapper").unwrap_err().kind,
        ErrorKind::Type
    );
    let partial = member(&mut vm, &module, "Child").unwrap();
    vm.start_member(module.as_object().unwrap(), "ordinary")
        .unwrap();
    let pending = loop {
        match vm.poll(1) {
            VmPoll::Pending(read) => break read.id,
            VmPoll::Yielded => {
                vm.collect(&[]);
            }
            event => panic!("expected ordinary read, got {event:?}"),
        }
    };
    assert_eq!(vm.collect(&[]).live, 4);
    // This development API is nonterminal, unlike HostSession::cancel.
    vm.cancel();
    assert_eq!(ready(&mut vm).unwrap_err().kind, ErrorKind::Cancelled);
    assert!(!vm.is_closed());
    assert_eq!(vm.collect(&[]).live, 3);
    assert_eq!(
        vm.resume(pending, Ok(Value::Null)).unwrap_err().kind,
        ErrorKind::Host
    );
    assert_eq!(
        vm.member_state(module.as_object().unwrap(), "ordinary")
            .unwrap(),
        LazyState::Unevaluated
    );
    assert_eq!(member(&mut vm, &module, "Child"), Ok(partial));
    vm.close();
    assert_eq!(vm.heap_stats().live, 0);
    assert_eq!(vm.heap_stats().slots, 0);
}

fn inject_initialization_read(program: &mut Program) {
    let class_block = program
        .blocks
        .iter_mut()
        .find(|block| {
            block.iter().any(|instruction| {
                matches!(instruction,
            I::BeginClass { class, .. } if &*class.name == "Child")
            })
        })
        .unwrap();
    let insertion = class_block
        .iter()
        .position(|instruction| matches!(instruction, I::PublishClassMember { index: 0, .. }))
        .unwrap()
        + 1;
    for instruction in class_block.iter_mut() {
        if let I::BeginClass { end, .. } = instruction {
            *end += 3;
        }
    }
    // No supported language type-initialization path currently performs this
    // read. This deliberately injected scheduler test is not Java parity.
    class_block.splice(
        insertion..insertion,
        [
            I::Constant(Value::string("mem:initialization")),
            I::ReadResource,
            I::Pop,
        ],
    );
}

#[test]
fn development_cancel_during_injected_initialization_preserves_only_published_prefix() {
    let mut program = program(SOURCE);
    inject_initialization_read(&mut program);
    let entry = program.entry;
    let mut vm = Vm::new(Arc::new(program));
    vm.start(entry, vec![]).unwrap();
    let module = ready(&mut vm).unwrap();
    vm.start_member(module.as_object().unwrap(), "wrapper")
        .unwrap();
    loop {
        match vm.poll(1) {
            VmPoll::Pending(read) => {
                assert_eq!(&*read.uri, "mem:initialization");
                break;
            }
            VmPoll::Yielded => {
                vm.collect(&[]);
            }
            event => panic!("expected injected initialization read, got {event:?}"),
        }
    }
    assert_eq!(vm.collect(&[]).live, 3);
    // Scheduler-only cancellation observation; no Java cancellation parity claim.
    vm.cancel();
    assert_eq!(ready(&mut vm).unwrap_err().kind, ErrorKind::Cancelled);
    assert_eq!(vm.collect(&[]).live, 3);
    assert_eq!(
        vm.member_state(module.as_object().unwrap(), "wrapper")
            .unwrap(),
        LazyState::Unevaluated
    );
    let partial = member(&mut vm, &module, "wrapper").unwrap();
    assert_eq!(member(&mut vm, &module, "Child"), Ok(partial));
    assert_eq!(member(&mut vm, &module, "early"), Ok(Value::Int(10)));
    assert_eq!(
        member(&mut vm, &module, "late").unwrap_err().kind,
        ErrorKind::MissingMember
    );
    assert_eq!(vm.collect(&[]).live, 5);
    vm.close();
    assert_eq!(vm.heap_stats().live, 0);
    assert_eq!(vm.heap_stats().slots, 0);
}
