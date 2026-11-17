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
            pkl_runtime::vm::VmPoll::Glob(_) => {
                panic!("unexpected glob request in non-glob regression")
            }
            VmPoll::Ready(value) => return value,
            VmPoll::Yielded => {}
            VmPoll::Log(_) | VmPoll::TransformFrame(_) => panic!("unexpected logger request"),
            VmPoll::Module(_) => panic!("unexpected module request"),
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
    assert_eq!(done(&mut vm), Ok(Value::Int(expected.into())));
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
            LazyState::Unevaluated
        ));
    }
}

#[test]
fn first_error_stops_forcing_and_failed_attempt_is_retryable() {
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
    assert_eq!(error.message, "Cannot find property `missingFirst`.");
    assert_eq!(
        vm.member_state(object, "second").unwrap(),
        LazyState::Unevaluated
    );
    let count = vm.member_evaluations();
    vm.start_member(object, "result").unwrap();
    assert_eq!(done(&mut vm), Err(error));
    assert!(vm.member_evaluations() > count);
}

#[test]
fn tracing_collects_cycles_reuses_slots_and_rejects_stale_or_foreign_handles() {
    let mut heap = Heap::new();
    let stale = heap.allocate(Object::default());
    let key = CacheKey {
        entry: false,
        owner: None,
        name: "self".into(),
        method: false,
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
        flags: 0,
        entry: false,
        name: "answer".into(),
        code: 1,
        local: false,
        method: false,
        ty: None,
        type_span: None,
        constant: false,
        fixed: false,
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
    let bytes = Value::Bytes(Arc::<[u8]>::from([0, 255, 128]).into());
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
        vec![I::Constant(Value::Int(42.into())), I::Return],
    ]));
    let mut vm = Vm::new(program);
    vm.start(0, vec![]).unwrap();
    assert_eq!(vm.poll(17), VmPoll::Yielded);
    assert_eq!(vm.instructions_executed(), 17);
    vm.cancel();
    assert_eq!(done(&mut vm).unwrap_err().kind, ErrorKind::Cancelled);
    vm.start(1, vec![]).unwrap();
    assert_eq!(done(&mut vm), Ok(Value::Int(42.into())));
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
        LazyState::Unevaluated
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
        I::Constant(Value::Int(1.into())),
        I::Add,
        I::Return,
    ]]));
    let mut first = Vm::new(program.clone());
    let mut second = Vm::new(program);
    first.start(0, vec![Value::Int(4.into())]).unwrap();
    second.start(0, vec![Value::Int(90.into())]).unwrap();
    assert_eq!(done(&mut first), Ok(Value::Int(5.into())));
    assert_eq!(done(&mut second), Ok(Value::Int(91.into())));
    first.start(0, vec![Value::Int(i64::MAX.into())]).unwrap();
    assert_eq!(done(&mut first).unwrap_err().kind, ErrorKind::Arithmetic);
}

#[test]
fn malformed_bytecode_is_an_error_and_cannot_leak_a_running_thunk() {
    let mut vm = Vm::new(Arc::new(Program::new(vec![
        vec![I::Add],
        vec![I::Constant(Value::Int(1.into())), I::Return],
    ])));
    vm.start(0, vec![]).unwrap();
    assert_eq!(done(&mut vm).unwrap_err().kind, ErrorKind::InvalidBytecode);
    vm.start(1, vec![]).unwrap();
    assert_eq!(done(&mut vm), Ok(Value::Int(1.into())));
    let mut program = Program::new(vec![
        vec![
            I::MakeObject {
                template: 0,
                has_parent: false,
            },
            I::Return,
        ],
        vec![
            I::Constant(Value::Int(1.into())),
            I::Constant(Value::Int(2.into())),
            I::Return,
        ],
    ]);
    program.objects.push(vec![Member {
        flags: 0,
        entry: false,
        name: "bad".into(),
        code: 1,
        local: false,
        method: false,
        ty: None,
        type_span: None,
        constant: false,
        fixed: false,
    }]);
    let mut vm = Vm::new(Arc::new(program));
    vm.start(0, vec![]).unwrap();
    let object = done(&mut vm).unwrap().as_object().unwrap();
    vm.start_member(object, "bad").unwrap();
    assert_eq!(done(&mut vm).unwrap_err().kind, ErrorKind::InvalidBytecode);
    assert!(matches!(
        vm.member_state(object, "bad").unwrap(),
        LazyState::Unevaluated
    ));
}

#[test]
fn mixed_numeric_execution_uses_ieee_division_and_checked_integer_arithmetic() {
    let (mut vm, object) = module("result = 1 / 2 + 1\n");
    vm.start_member(object.as_object().unwrap(), "result")
        .unwrap();
    assert_eq!(done(&mut vm), Ok(Value::Float(1.5.into())));
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
            I::GetSuperMember("answer".into()),
            I::GetSuperMember("answer".into()),
            I::Add,
            I::Return,
        ],
    ]);
    program.objects.push(vec![Member {
        flags: 0,
        entry: false,
        name: "answer".into(),
        code: 1,
        local: false,
        method: false,
        ty: None,
        type_span: None,
        constant: false,
        fixed: false,
    }]);
    program.objects.push(vec![Member {
        flags: 0,
        entry: false,
        name: "result".into(),
        code: 2,
        local: false,
        method: false,
        ty: None,
        type_span: None,
        constant: false,
        fixed: false,
    }]);
    let mut vm = Vm::new(Arc::new(program));
    vm.start(0, vec![]).unwrap();
    let child = done(&mut vm).unwrap().as_object().unwrap();
    vm.start_member(child, "result").unwrap();
    for value in [20, 22] {
        let VmPoll::Pending(request) = vm.poll(100) else {
            panic!("expected uncached super read")
        };
        vm.resume(request.id, Ok(Value::Int(value.into()))).unwrap();
    }
    assert_eq!(done(&mut vm), Ok(Value::Int(42.into())));
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
    assert_eq!(results[0], Ok(Value::Int(i64::MAX.into())));
    assert_eq!(results[1].as_ref().unwrap_err().kind, ErrorKind::Arithmetic);
    assert_eq!(
        results[2].as_ref().unwrap_err().message,
        "Cannot find property `missingFirst`."
    );
    assert_eq!(results[3].as_ref().unwrap_err().kind, ErrorKind::Type);
    assert_eq!(results[4], Ok(Value::Float(1.5.into())));
}

#[test]
fn transient_failures_retry_in_order_while_successful_dependencies_stay_cached() {
    let mut program = Program::new(vec![
        vec![
            I::MakeObject {
                template: 0,
                has_parent: false,
            },
            I::Return,
        ],
        vec![
            I::Constant(Value::string("memory:first")),
            I::ReadResource,
            I::Return,
        ],
        vec![
            I::Constant(Value::string("memory:second")),
            I::ReadResource,
            I::Return,
        ],
        vec![
            I::GetProperty {
                name: "first".into(),
                owner_depth: 0,
                local: false,
            },
            I::GetProperty {
                name: "second".into(),
                owner_depth: 0,
                local: false,
            },
            I::Add,
            I::Return,
        ],
    ]);
    program.objects.push(
        [("first", 1), ("second", 2), ("result", 3)]
            .map(|(name, code)| Member {
                flags: 0,
                entry: false,
                name: name.into(),
                code,
                local: false,
                method: false,
                ty: None,
                type_span: None,
                constant: false,
                fixed: false,
            })
            .to_vec(),
    );
    let mut vm = Vm::new(Arc::new(program));
    vm.start(0, vec![]).unwrap();
    let object = done(&mut vm).unwrap().as_object().unwrap();
    let read = |vm: &mut Vm, uri: &str| {
        let VmPoll::Pending(request) = vm.poll(100) else {
            panic!("expected read")
        };
        assert_eq!(request.uri, uri);
        request.id
    };
    let failure = RuntimeError::new(ErrorKind::Host, "transient\0failure\r\n🙂");
    vm.start_member(object, "result").unwrap();
    let first = read(&mut vm, "memory:first");
    vm.resume(first, Err(failure.clone())).unwrap();
    assert_eq!(done(&mut vm), Err(failure.clone()));
    for name in ["first", "second", "result"] {
        assert_eq!(
            vm.member_state(object, name).unwrap(),
            LazyState::Unevaluated
        );
    }
    vm.start_member(object, "result").unwrap();
    let retry = read(&mut vm, "memory:first");
    assert_ne!(first, retry);
    vm.resume(retry, Ok(Value::Int(40.into()))).unwrap();
    let second = read(&mut vm, "memory:second");
    vm.resume(second, Err(failure.clone())).unwrap();
    assert_eq!(done(&mut vm), Err(failure));
    assert_eq!(
        vm.member_state(object, "first").unwrap(),
        LazyState::Completed(Value::Int(40.into()))
    );
    vm.start_member(object, "result").unwrap();
    let retry = read(&mut vm, "memory:second");
    vm.resume(retry, Ok(Value::Int(2.into()))).unwrap();
    assert_eq!(done(&mut vm), Ok(Value::Int(42.into())));
    let evaluations = vm.member_evaluations();
    vm.start_member(object, "result").unwrap();
    assert_eq!(done(&mut vm), Ok(Value::Int(42.into())));
    assert_eq!(vm.member_evaluations(), evaluations);
}

#[test]
fn power_assertion_compilation_excludes_module_keyword_observation() {
    let source = "x = (it) -> it != module\n";
    let parsed = parse_module(source).unwrap();
    let resolved = resolve_module(&parsed).unwrap();
    let program = pkl_runtime::vm::compile_module_with_power_assertions(&resolved).unwrap();
    let module_offset = source.find("module").unwrap();
    let module_len = "module".len();
    for i in 0..program.blocks.next_id() {
        if let Some(instructions) = program.blocks.get(i) {
            for inst in instructions {
                if let I::Observe(span) = inst {
                    let is_module_span =
                        span.start == module_offset && (span.end - span.start) == module_len;
                    assert!(
                        !is_module_span,
                        "Instruction::Observe must not be emitted for `module` keyword (GetModuleNode in Java)"
                    );
                }
            }
        }
    }
}
