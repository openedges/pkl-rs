use std::sync::Arc;

use pkl_runtime::heap::{CacheKey, Heap, LazyState, Member, Object};
use pkl_runtime::parser::parse_module;
use pkl_runtime::resolver::resolve_module;
use pkl_runtime::value::{ErrorKind, RuntimeError, Value};
use pkl_runtime::vm::{CompileOptions, compile_module_with_options};
use pkl_runtime::vm::{Instruction as I, Program, Vm, VmPoll, compile_module};

fn done(vm: &mut Vm) -> Result<Value, RuntimeError> {
    for _ in 0..1000 {
        match vm.poll(97) {
            VmPoll::Ready(value) => return value,
            VmPoll::Yielded => {}
            VmPoll::Pending(_) => panic!("unexpected Host request"),
        }
    }
    panic!("bounded workload exhausted its instruction contract")
}

fn module(source: &str) -> (Vm, Value) {
    let program = compile_module(&resolve_module(&parse_module(source).unwrap()).unwrap()).unwrap();
    let mut vm = Vm::new(program.clone());
    vm.start(program.entry, vec![]).unwrap();
    let value = done(&mut vm).unwrap();
    (vm, value)
}

fn fixture(source: &str, expected: i64) {
    let (mut vm, object) = module(source);
    vm.start_member(object.as_object().unwrap(), "result")
        .unwrap();
    assert_eq!(done(&mut vm), Ok(Value::Int(expected)));
}

#[test]
fn amended_receiver_super_owner_and_local_definition_stay_distinct() {
    fixture(
        include_str!("../../../tests/fixtures/m1/runtime/amendment.pkl"),
        53,
    );
    fixture(
        include_str!("../../../tests/fixtures/m1/runtime/captured-receiver.pkl"),
        2,
    );
}

#[test]
fn lexical_captures_cross_object_boundaries() {
    fixture(
        include_str!("../../../tests/fixtures/m1/runtime/lexical-let.pkl"),
        15,
    );
}

#[test]
fn lazy_object_cycles_are_values_but_forcing_cycles_fail() {
    fixture(
        include_str!("../../../tests/fixtures/m1/runtime/object-cycle.pkl"),
        42,
    );
    let (mut vm, object) = module(include_str!(
        "../../../tests/fixtures/m1/runtime/forcing-cycle.pkl"
    ));
    let object = object.as_object().unwrap();
    vm.start_member(object, "result").unwrap();
    assert_eq!(done(&mut vm).unwrap_err().kind, ErrorKind::Cycle);
    for name in ["result", "left", "right"] {
        assert!(matches!(
            vm.member_state(object, name).unwrap(),
            LazyState::Failed(_)
        ));
    }
}

#[test]
fn first_error_stops_forcing_and_failed_state_does_not_reexecute() {
    fixture(
        include_str!("../../../tests/fixtures/m1/runtime/lazy-unused.pkl"),
        42,
    );
    let (mut vm, object) = module(include_str!(
        "../../../tests/fixtures/m1/runtime/first-error.pkl"
    ));
    let object = object.as_object().unwrap();
    vm.start_member(object, "result").unwrap();
    let error = done(&mut vm).unwrap_err();
    assert_eq!(error.message, "missing member missingFirst");
    assert_eq!(
        vm.member_state(object, "second").unwrap(),
        LazyState::Unevaluated
    );
    let count = vm.member_evaluations();
    vm.start_member(object, "result").unwrap();
    assert_eq!(done(&mut vm), Err(error));
    assert_eq!(vm.member_evaluations(), count);
}

#[test]
fn tracing_collects_cycles_reuses_slots_and_rejects_stale_or_foreign_handles() {
    let mut heap = Heap::new();
    let stale = heap.allocate(Object::default());
    let key = CacheKey {
        owner: None,
        name: Arc::from("self"),
    };
    heap.get_mut(stale)
        .unwrap()
        .cache
        .insert(key.clone(), LazyState::Completed(Value::Object(stale)));
    assert_eq!(heap.collect(&[Value::Object(stale)]).live, 1);
    assert_eq!(heap.collect(&[]).live, 0);
    let replacement = heap.allocate(Object::default());
    assert_ne!(stale, replacement);
    assert_eq!(heap.get(stale).unwrap_err().kind, ErrorKind::StaleHandle);
    assert_eq!(
        Heap::new().get(replacement).unwrap_err().kind,
        ErrorKind::StaleHandle
    );
    heap.clear();
    for _ in 0..10_000 {
        let first = heap.allocate(Object::default());
        let second = heap.allocate(Object::default());
        heap.get_mut(first)
            .unwrap()
            .cache
            .insert(key.clone(), LazyState::Completed(Value::Object(second)));
        heap.get_mut(second)
            .unwrap()
            .cache
            .insert(key.clone(), LazyState::Completed(Value::Object(first)));
        assert_eq!(heap.collect(&[]).live, 0);
    }
    assert_eq!(heap.stats().slots, 2);
    assert_eq!(heap.stats().reclaimed, 20_002);
}

fn resource_program() -> Arc<Program> {
    let mut program = Program::new(vec![
        vec![
            I::MakeObject {
                template: 0,
                has_parent: false,
            },
            I::Return,
        ],
        vec![
            I::Constant(Value::string("memory:answer")),
            I::ReadResource,
            I::Return,
        ],
    ]);
    program.objects.push(vec![Member {
        name: Arc::from("answer"),
        code: 1,
        local: false,
    }]);
    Arc::new(program)
}

#[test]
fn pending_frames_are_gc_roots_and_memoize_completed_host_values() {
    let program = resource_program();
    let mut vm = Vm::new(program);
    vm.start(0, vec![]).unwrap();
    let object = done(&mut vm).unwrap().as_object().unwrap();
    vm.start_member(object, "answer").unwrap();
    assert_eq!(
        vm.member_state(object, "answer").unwrap(),
        LazyState::Evaluating
    );
    let VmPoll::Pending(request) = vm.poll(100) else {
        panic!("missing read")
    };
    assert_eq!(vm.collect(&[]).live, 1);
    assert!(vm.start(0, vec![]).is_err());
    let bytes = Value::Bytes(Arc::from([0, 255, 128]));
    vm.resume(request.id, Ok(bytes.clone())).unwrap();
    assert_eq!(done(&mut vm), Ok(bytes.clone()));
    assert!(vm.resume(request.id, Ok(Value::Null)).is_err());
    let instructions = vm.instructions_executed();
    vm.start_member(object, "answer").unwrap();
    assert_eq!(done(&mut vm), Ok(bytes));
    assert_eq!(vm.instructions_executed(), instructions);
    vm.close();
    assert_eq!(vm.heap_stats().live, 0);
}

#[test]
fn cancellation_is_bounded_at_instruction_safepoints_and_rejects_stale_resume() {
    let program = Arc::new(Program::new(vec![
        vec![I::Jump(0)],
        vec![I::Constant(Value::Int(42)), I::Return],
    ]));
    let mut vm = Vm::new(program);
    vm.start(0, vec![]).unwrap();
    assert_eq!(vm.poll(17), VmPoll::Yielded);
    assert_eq!(vm.instructions_executed(), 17);
    vm.cancel();
    assert_eq!(done(&mut vm).unwrap_err().kind, ErrorKind::Cancelled);
    vm.start(1, vec![]).unwrap();
    assert_eq!(done(&mut vm), Ok(Value::Int(42)));
    let mut waiting = Vm::new(resource_program());
    waiting.start(0, vec![]).unwrap();
    let object = done(&mut waiting).unwrap().as_object().unwrap();
    waiting.start_member(object, "answer").unwrap();
    let VmPoll::Pending(request) = waiting.poll(100) else {
        panic!("missing read")
    };
    waiting.cancel();
    assert!(waiting.resume(request.id, Ok(Value::Null)).is_err());
    assert_eq!(done(&mut waiting).unwrap_err().kind, ErrorKind::Cancelled);
    assert!(matches!(
        waiting.member_state(object, "answer").unwrap(),
        LazyState::Failed(_)
    ));
    assert_eq!(waiting.collect(&[]).live, 0);
    waiting.close();
    waiting.close();
    assert_eq!(
        waiting.start(0, vec![]).unwrap_err().kind,
        ErrorKind::Closed
    );
}

#[test]
fn compiled_program_reuse_never_reuses_evaluation_state() {
    let program = Arc::new(Program::new(vec![vec![
        I::Input(0),
        I::Constant(Value::Int(1)),
        I::Add,
        I::Return,
    ]]));
    let mut first = Vm::new(program.clone());
    let mut second = Vm::new(program);
    first.start(0, vec![Value::Int(4)]).unwrap();
    second.start(0, vec![Value::Int(90)]).unwrap();
    assert_eq!(done(&mut first), Ok(Value::Int(5)));
    assert_eq!(done(&mut second), Ok(Value::Int(91)));
    first.start(0, vec![Value::Int(i64::MAX)]).unwrap();
    assert_eq!(done(&mut first).unwrap_err().kind, ErrorKind::Arithmetic);
}

#[test]
fn malformed_bytecode_is_an_error_and_cannot_leak_a_running_thunk() {
    let mut vm = Vm::new(Arc::new(Program::new(vec![
        vec![I::Add],
        vec![I::Constant(Value::Int(1)), I::Return],
    ])));
    vm.start(0, vec![]).unwrap();
    assert_eq!(done(&mut vm).unwrap_err().kind, ErrorKind::InvalidBytecode);
    vm.start(1, vec![]).unwrap();
    assert_eq!(done(&mut vm), Ok(Value::Int(1)));
    let mut program = Program::new(vec![
        vec![
            I::MakeObject {
                template: 0,
                has_parent: false,
            },
            I::Return,
        ],
        vec![
            I::Constant(Value::Int(1)),
            I::Constant(Value::Int(2)),
            I::Return,
        ],
    ]);
    program.objects.push(vec![Member {
        name: Arc::from("bad"),
        code: 1,
        local: false,
    }]);
    let mut vm = Vm::new(Arc::new(program));
    vm.start(0, vec![]).unwrap();
    let object = done(&mut vm).unwrap().as_object().unwrap();
    vm.start_member(object, "bad").unwrap();
    assert_eq!(done(&mut vm).unwrap_err().kind, ErrorKind::InvalidBytecode);
    assert!(matches!(
        vm.member_state(object, "bad").unwrap(),
        LazyState::Failed(_)
    ));
}

#[test]
fn mixed_numeric_execution_uses_ieee_division_and_checked_integer_arithmetic() {
    let (mut vm, object) = module("result = 1 / 2 + 1\n");
    vm.start_member(object.as_object().unwrap(), "result")
        .unwrap();
    assert_eq!(done(&mut vm), Ok(Value::Float(1.5)));
}

#[test]
fn super_reads_preserve_receiver_but_do_not_memoize_parent_body() {
    let mut program = Program::new(vec![
        vec![
            I::MakeObject {
                template: 0,
                has_parent: false,
            },
            I::MakeObject {
                template: 1,
                has_parent: true,
            },
            I::Return,
        ],
        vec![
            I::Constant(Value::string("memory:value")),
            I::ReadResource,
            I::Return,
        ],
        vec![
            I::GetSuperMember(Arc::from("answer")),
            I::GetSuperMember(Arc::from("answer")),
            I::Add,
            I::Return,
        ],
    ]);
    program.objects.push(vec![Member {
        name: Arc::from("answer"),
        code: 1,
        local: false,
    }]);
    program.objects.push(vec![Member {
        name: Arc::from("result"),
        code: 2,
        local: false,
    }]);
    let mut vm = Vm::new(Arc::new(program));
    vm.start(0, vec![]).unwrap();
    let child = done(&mut vm).unwrap().as_object().unwrap();
    vm.start_member(child, "result").unwrap();
    for value in [20, 22] {
        let VmPoll::Pending(request) = vm.poll(100) else {
            panic!("expected uncached super read")
        };
        vm.resume(request.id, Ok(Value::Int(value))).unwrap();
    }
    assert_eq!(done(&mut vm), Ok(Value::Int(42)));
    assert_eq!(
        vm.member_state(child, "answer").unwrap(),
        LazyState::Unevaluated
    );
    assert_eq!(vm.member_evaluations(), 3);
}

#[test]
fn compiler_fusion_preserves_checked_overflow_types_and_first_error() {
    let sources = [
        "result = 9223372036854775806 + 1\n",
        include_str!("../../../tests/fixtures/m1/runtime/overflow-first.pkl"),
        "result = missingFirst + 1 + missingLater\n",
        "result = \"bad\" + 1\n",
        include_str!("../../../tests/fixtures/m1/runtime/mixed-numeric.pkl"),
    ];
    let mut results = Vec::new();
    for source in sources {
        let resolved = resolve_module(&parse_module(source).unwrap()).unwrap();
        let mut variants = Vec::new();
        for fuse_constant_add in [false, true] {
            let program =
                compile_module_with_options(&resolved, CompileOptions { fuse_constant_add })
                    .unwrap();
            assert_eq!(
                program
                    .blocks
                    .iter()
                    .flatten()
                    .any(|instruction| matches!(instruction, I::AddIntConstant(_))),
                fuse_constant_add
            );
            let mut vm = Vm::new(program.clone());
            vm.start(program.entry, vec![]).unwrap();
            let object = done(&mut vm).unwrap().as_object().unwrap();
            vm.start_member(object, "result").unwrap();
            variants.push(done(&mut vm));
        }
        assert_eq!(variants[0], variants[1], "fusion changed {source}");
        results.push(variants.remove(0));
    }
    assert_eq!(results[0], Ok(Value::Int(i64::MAX)));
    assert_eq!(results[1].as_ref().unwrap_err().kind, ErrorKind::Arithmetic);
    assert_eq!(
        results[2].as_ref().unwrap_err().message,
        "missing member missingFirst"
    );
    assert_eq!(results[3].as_ref().unwrap_err().kind, ErrorKind::Type);
    assert_eq!(results[4], Ok(Value::Float(1.5)));
}
