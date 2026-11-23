//! Metadata ownership tests; these establish stack bounds, not Pkl compatibility.
use super::*;

#[test]
fn shared_abstract_collection_metadata_traversals_and_release_are_stack_safe() {
    std::thread::Builder::new()
        .stack_size(256 * 1024)
        .spawn(|| {
            let mut left = TypeCheck::Int.shared();
            let TypeCheck::Shared(node) = &left else {
                unreachable!()
            };
            let released = Arc::downgrade(node.0.as_ref().unwrap());
            let mut right = TypeCheck::Int.shared();
            let span = Span::new(0, 0);
            for _ in 0..20_000 {
                left = TypeCheck::CollectionBase {
                    arguments: vec![(left, span)],
                }
                .shared();
                right = TypeCheck::CollectionBase {
                    arguments: vec![(right, span)],
                }
                .shared();
            }
            // Abstract Collection type-node identity is deliberately distinct
            // from exported structural metadata equality.
            assert!(!left.equivalent(&right));
            assert!(!left.noop());
            assert!(!left.contains_parameter());
            assert!(!left.has_deferred_reference_validation());
            assert!(!typecheck::needs_custom_this(&left));
            left.validate_reference_arguments(span, &[], &BTreeMap::new())
                .unwrap();
            let left_metadata = reference::normalize(&left, &"m2-collection".into());
            let right_metadata = reference::normalize(&right, &"m2-collection".into());
            assert_eq!(
                reference::metadata_key(&left_metadata),
                reference::metadata_key(&right_metadata)
            );
            assert!(reference::is_subtype(
                &left_metadata,
                &right_metadata,
                &Heap::new()
            ));
            assert!(reference::is_subtype(
                &right_metadata,
                &left_metadata,
                &Heap::new()
            ));
            assert_eq!(
                reference::type_name(&left_metadata, &"".into()),
                format!("{}Int{}", "Collection<".repeat(20_000), ">".repeat(20_000))
            );
            assert_eq!(
                type_name_value(&left, &"".into()).to_java_utf8(),
                "Collection"
            );
            drop(left);
            drop(right);
            assert!(released.upgrade().is_none());
            drop(left_metadata);
            drop(right_metadata);
        })
        .unwrap()
        .join()
        .unwrap();
}

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
            environment.type_arguments.insert((7, "T".into()), left);
            let clone = environment.clone();
            let substituted = TypeCheck::Nullable(Box::new(TypeCheck::Parameter {
                block: 7,
                name: "T".into(),
            }))
            .substituted(&clone)
            .unwrap();
            assert!(substituted.accepts(&Value::Null, &Heap::new()));
            assert_eq!(
                type_name_value(&substituted, &"".into()).to_java_utf8(),
                "Mapping?"
            );
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
            assert!(ty.accepts(&Value::Int(1.into()), &Heap::new()));
            assert!(!ty.accepts(&Value::Bool(false), &Heap::new()));
            assert_eq!(
                type_name_value(&ty, &"".into()).to_java_utf8(),
                format!("Int{}", "?".repeat(20_000))
            );
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
        let mut ty = TypeCheck::StringLiteral("leaf".into()).shared();
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
        // Defaults share the evaluator's builtin class/prototype graph. Its
        // retained roots are separate from the unpublished default chain.
        vm.canonical_builtin_class(classes::BuiltinClass::Listing);
        let canonical_live = vm.collect(&[]).live;
        vm.start(0, vec![]).unwrap();
        let target = vm.heap.allocate(Object::default());
        let mut unpublished = BTreeSet::new();
        vm.frames.last_mut().unwrap().generic_default =
            Some(Box::new(GenericDefaultRun::new(target, ty, None)));
        for index in 0..stop {
            assert!(matches!(vm.poll(1), VmPoll::Yielded));
            // One shared empty Listing and the target accompany exactly one
            // allocated child per descent step; unwind only links children.
            let expected = canonical_live + 2 + (index + 1).min(DEPTH);
            assert_eq!(vm.collect(&[]).live, expected);
            let run = vm.frames.last().unwrap().generic_default.as_ref().unwrap();
            unpublished.extend(run.tasks.iter().filter_map(|task| match task {
                DefaultTask::Collection(id) => Some(*id),
                _ => None,
            }));
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
        assert_eq!(vm.collect(&[]).live, canonical_live);
        assert!(vm.heap.get(target).is_err());
        assert!(unpublished.into_iter().all(|id| vm.heap.get(id).is_err()));
        assert!(vm.empty_objects[1].is_some_and(|id| vm.heap.get(id).is_err()));
        vm.close();
        assert_eq!(vm.heap_stats().slots, 0);
    }
}

#[test]
fn varargs_metadata_retains_substituted_elements_without_value_checks() {
    // VarArgs is invariant metadata. Public checks reject later; retaining the
    // element here is necessary for aliases, Reference exports, and cache keys.
    let ty = TypeCheck::VarArgs {
        element: Box::new(TypeCheck::Parameter {
            block: 7,
            name: "T".into(),
        }),
        span: Span::new(3, 17),
    };
    let mut environment = Environment::default();
    environment
        .type_arguments
        .insert((7, "T".into()), TypeCheck::Int);
    assert!(ty.contains_parameter());
    let resolved = ty.substituted(&environment).unwrap();
    assert!(!resolved.contains_parameter());
    let normalized = reference::normalize(&resolved, &"module".into());
    assert!(normalized.equivalent(&resolved));
    assert!(!normalized.equivalent(&TypeCheck::VarArgs {
        element: Box::new(TypeCheck::String),
        span: Span::new(0, 0)
    }));
    assert_ne!(
        reference::metadata_key(&resolved),
        reference::metadata_key(&TypeCheck::VarArgs {
            element: Box::new(TypeCheck::String),
            span: Span::new(0, 0)
        })
    );
    assert_eq!(
        reference::type_name(&normalized, &"module".into()),
        "VarArgs<Int>"
    );
    assert!(!resolved.noop());
}

#[test]
fn normalized_generic_metadata_substitutes_each_lexical_invocation() {
    let parameter = TypeCheck::Parameter {
        block: 41,
        name: "T".into(),
    };
    let original = TypeCheck::Nullable(Box::new(parameter));
    let normalized = reference::normalize(&original, &"shared-control".into());
    for actual in [TypeCheck::Int, TypeCheck::String] {
        let mut environment = Environment::default();
        environment
            .type_arguments
            .insert((41, "T".into()), actual.clone());
        let substituted = normalized.substituted(&environment).unwrap();
        let expected = reference::normalize(
            &TypeCheck::Nullable(Box::new(actual)),
            &"shared-control".into(),
        );
        assert!(!substituted.contains_parameter());
        let normalized_result = reference::normalize(&substituted, &"shared-control".into());
        assert_eq!(
            reference::metadata_key(&normalized_result),
            reference::metadata_key(&expected)
        );
    }
    assert!(
        normalized.contains_parameter(),
        "reusable lexical template stays unbound"
    );
}

#[test]
fn shared_constraint_actuals_follow_each_lexical_invocation() {
    let constraint = ConstraintCode {
        section: None,
        block: 42,
        arguments: BTreeMap::from([(
            "T".into(),
            TypeCheck::Parameter {
                block: 41,
                name: "T".into(),
            },
        )]),
        argument_sources: BTreeMap::new(),
        span: Span::new(0, 4),
        module_environment: false,
        source: "this".into(),
    };
    let template = TypeCheck::Constrained {
        base: Box::new(TypeCheck::Int),
        base_span: Span::new(0, 0),
        constraints: vec![constraint],
    }
    .shared();
    for actual in [TypeCheck::Int, TypeCheck::String] {
        let mut environment = Environment::default();
        environment
            .type_arguments
            .insert((41, "T".into()), actual.clone());
        let result = template.substituted(&environment).unwrap();
        let TypeCheck::Constrained { constraints, .. } = result.resolved() else {
            panic!("constraint remains executable")
        };
        assert!(constraints[0].arguments[&PklString::from("T")].equivalent(&actual));
    }
}
