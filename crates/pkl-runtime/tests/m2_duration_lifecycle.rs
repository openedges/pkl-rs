//! Ownership and cancellation checks complement the independent Java cohort.
use pkl_runtime::{
    duration::{Duration, DurationUnit},
    heap::LazyState,
    parser::parse_module,
    resolver::resolve_module,
    value::{ErrorKind, Value},
    vm::{Vm, VmPoll, compile_module},
};

fn event(vm: &mut Vm, retained: &[Value]) -> VmPoll {
    for _ in 0..200_000 {
        vm.collect(retained);
        let event = vm.poll(1);
        let mut roots = retained.to_vec();
        if let VmPoll::Ready(Ok(value)) = &event {
            roots.push(value.clone());
        }
        vm.collect(&roots);
        if !matches!(event, VmPoll::Yielded) {
            return event;
        }
    }
    panic!("Duration lifecycle poll budget")
}

fn read(vm: &mut Vm, uri: &str, retained: &[Value]) -> u64 {
    let VmPoll::Pending(request) = event(vm, retained) else {
        panic!("expected Host read {uri}")
    };
    assert_eq!(request.uri, uri);
    request.id
}

#[test]
fn converted_duration_callbacks_survive_cancellation_and_keep_published_lazy_values() {
    let source = r#"
initial = read("duration:first")
unused = throw("must remain lazy")
result = initial.ifNonNull((x: Duration) -> x.toUnit(read("duration:unit")).ifNonNull((d) -> (d + 30.s).isoString))
"#;
    let program = compile_module(&resolve_module(&parse_module(source).unwrap()).unwrap()).unwrap();
    let mut vm = Vm::new(program.clone());
    vm.start(program.entry, vec![]).unwrap();
    let VmPoll::Ready(Ok(module)) = event(&mut vm, &[]) else {
        panic!("module initialization")
    };
    let retained_module = [module.clone()];
    let module = module.as_object().unwrap();
    vm.start_member(module, "result").unwrap();
    let initial = read(&mut vm, "duration:first", &retained_module);
    vm.resume(
        initial,
        Ok(Value::Duration(Duration::new(90.0, DurationUnit::Seconds))),
    )
    .unwrap();
    let mut unit = read(&mut vm, "duration:unit", &retained_module);
    let mut retained = None;
    for _ in 0..12 {
        vm.cancel();
        let VmPoll::Ready(Err(error)) = event(&mut vm, &retained_module) else {
            panic!("cancelled evaluation")
        };
        assert_eq!(error.kind, ErrorKind::Cancelled);
        assert_eq!(
            vm.member_state(module, "result").unwrap(),
            LazyState::Unevaluated
        );
        assert_eq!(
            vm.member_state(module, "unused").unwrap(),
            LazyState::Unevaluated
        );
        assert_eq!(
            vm.resume(unit, Ok(Value::string("min"))).unwrap_err().kind,
            ErrorKind::Host
        );
        let stats = vm.collect(&[Value::Object(module)]);
        let current = (stats.live, stats.executable_live);
        if let Some(previous) = retained {
            assert_eq!(current, previous);
        }
        retained = Some(current);
        vm.start_member(module, "result").unwrap();
        // The completed initial read remains published after cancellation.
        unit = read(&mut vm, "duration:unit", &retained_module);
    }
    vm.resume(unit, Ok(Value::string("min"))).unwrap();
    assert_eq!(
        event(&mut vm, &retained_module),
        VmPoll::Ready(Ok(Value::string("PT2M")))
    );
    vm.start_member(module, "result").unwrap();
    assert_eq!(
        event(&mut vm, &retained_module),
        VmPoll::Ready(Ok(Value::string("PT2M")))
    );
    vm.close();
    assert_eq!(vm.heap_stats().slots, 0);
}
