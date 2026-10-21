//! Metadata ownership tests; these establish stack bounds, not Pkl compatibility.
use super::*;

#[test]
fn shared_argument_dags_clone_compare_substitute_and_release_on_small_stack() {
    std::thread::Builder::new()
        .stack_size(256 * 1024)
        .spawn(|| {
            let leaf = TypeCheck::Int.shared();
            let TypeCheck::Shared(node) = &leaf else {
                unreachable!()
            };
            let released = Arc::downgrade(node.0.as_ref().unwrap());
            let mut left = leaf;
            let mut right = TypeCheck::Int.shared();
            let span = Span::new(0, 0);
            for _ in 0..20_000 {
                left = TypeCheck::ParameterizedCollection {
                    kind: crate::heap::CollectionKind::Mapping,
                    arguments: vec![(left.clone(), span), (left, span)],
                }
                .shared();
                right = TypeCheck::ParameterizedCollection {
                    kind: crate::heap::CollectionKind::Mapping,
                    arguments: vec![(right.clone(), span), (right, span)],
                }
                .shared();
            }
            assert!(left.equivalent(&right));
            assert!(!left.noop());
            assert!(!typecheck::needs_custom_this(&left));
            assert!(!left.contains_parameter());
            assert!(!left.has_deferred_reference_validation());
            left.validate_reference_arguments(span, &[], &BTreeMap::new())
                .unwrap();
            let mut environment = Environment::default();
            environment.type_arguments.insert((7, Arc::from("T")), left);
            let clone = environment.clone();
            let substituted = TypeCheck::Nullable(Box::new(TypeCheck::Parameter {
                block: 7,
                name: Arc::from("T"),
            }))
            .substituted(&clone)
            .unwrap();
            assert!(substituted.accepts(&Value::Null, &Heap::new()));
            assert_eq!(type_name(&substituted, ""), "Mapping?");
            assert!(format!("{clone:?}").len() < 512);
            drop(environment);
            drop(clone);
            assert!(released.upgrade().is_some());
            drop(substituted);
            drop(right);
            assert!(released.upgrade().is_none());
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn shared_nullable_traversals_and_concurrent_last_release_are_stack_safe() {
    std::thread::Builder::new()
        .stack_size(256 * 1024)
        .spawn(|| {
            let mut ty = TypeCheck::Int.shared();
            let TypeCheck::Shared(node) = &ty else {
                unreachable!()
            };
            let released = Arc::downgrade(node.0.as_ref().unwrap());
            for _ in 0..20_000 {
                ty = TypeCheck::Nullable(Box::new(ty)).shared();
            }
            assert!(ty.accepts(&Value::Int(1), &Heap::new()));
            assert!(!ty.accepts(&Value::Bool(false), &Heap::new()));
            assert_eq!(type_name(&ty, ""), format!("Int{}", "?".repeat(20_000)));
            let other = ty.clone();
            let barrier = Arc::new(std::sync::Barrier::new(2));
            let barrier2 = barrier.clone();
            let thread = std::thread::Builder::new()
                .stack_size(256 * 1024)
                .spawn(move || {
                    barrier2.wait();
                    drop(other);
                })
                .unwrap();
            barrier.wait();
            drop(ty);
            thread.join().unwrap();
            assert!(released.upgrade().is_none());
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn default_descent_and_unwind_yield_trace_unpublished_objects_and_cancel() {
    const DEPTH: usize = 512;
    for stop in [1, DEPTH / 2, DEPTH + 1, DEPTH + DEPTH / 2] {
        let mut ty = TypeCheck::StringLiteral(Arc::from("leaf")).shared();
        for _ in 0..DEPTH {
            ty = TypeCheck::ParameterizedCollection {
                kind: crate::heap::CollectionKind::Listing,
                arguments: vec![(ty, Span::new(0, 0))],
            }
            .shared();
        }
        let program = Arc::new(Program::new(vec![vec![
            Instruction::Constant(Value::Null),
            Instruction::Return,
        ]]));
        let mut vm = Vm::new(program);
        vm.start(0, vec![]).unwrap();
        let target = vm.heap.allocate(Object::default());
        vm.frames.last_mut().unwrap().generic_default = Some(GenericDefaultRun::new(target, ty));
        for index in 0..stop {
            assert!(matches!(vm.poll(1), VmPoll::Yielded));
            // Descent allocates exactly one object per step; unwind publishes
            // each child only into its still-unpublished parent.
            let expected = 1 + (index + 1).min(DEPTH);
            assert_eq!(vm.collect(&[]).live, expected);
            assert!(vm.heap.get(target).unwrap().collection_default.is_none());
        }
        let run = vm.frames.last().unwrap().generic_default.as_ref().unwrap();
        if stop <= DEPTH {
            assert!(matches!(run.tasks.last(), Some(DefaultTask::Visit(_))));
        } else {
            assert!(matches!(run.tasks.last(), Some(DefaultTask::Collection(_))));
        }
        vm.cancel();
        assert!(
            matches!(vm.poll(1), VmPoll::Ready(Err(error)) if error.kind == ErrorKind::Cancelled)
        );
        assert_eq!(vm.collect(&[]).live, 0);
        vm.close();
        assert_eq!(vm.heap_stats().slots, 0);
    }
}
