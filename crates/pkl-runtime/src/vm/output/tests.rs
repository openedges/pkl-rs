//! Structural graph assertions for the actual Module prototype, not Java heap counts.
use super::*;

fn compiled(source: &str) -> Arc<Program> {
    let parsed = crate::parser::parse_module(source).unwrap();
    compile_module(&crate::resolver::resolve_module(&parsed).unwrap()).unwrap()
}

fn ready(vm: &mut Vm) -> Result<Value, RuntimeError> {
    for _ in 0..10_000 {
        match vm.poll(1) {
            VmPoll::Ready(result) => return result,
            VmPoll::Yielded => {
                vm.collect(&[]);
            }
            other => panic!("unexpected prototype graph event: {other:?}"),
        }
    }
    panic!("prototype graph evaluation did not finish");
}

#[test]
fn captured_module_keeps_exactly_one_real_parent_until_terminal_release() {
    let program = compiled(
        "open class Base { fixed x: Int = 1 }\nclass Child extends Base { early: Int = 2; x = 3; late = 4 }\nscalar = 7\n",
    );
    let mut vm = Vm::new(program.clone());
    vm.start(program.entry, vec![]).unwrap();
    let module = ready(&mut vm).unwrap().as_object().unwrap();
    let parent = vm.heap.get(module).unwrap().parent.unwrap();
    assert_eq!(
        vm.heap
            .get(module)
            .unwrap()
            .class_definition
            .as_ref()
            .unwrap()
            .id,
        reference::MODULE_ID
    );
    let builtin = vm.heap.get(parent).unwrap();
    assert_eq!(builtin.class_definition.as_ref().unwrap().id, MODULE);
    assert!(
        builtin
            .builtin_type
            .as_ref()
            .unwrap()
            .equivalent(&builtins::ty(MODULE))
    );
    assert!(builtin.parent.is_none());
    assert!(builtin.cache.is_empty());
    assert!(
        builtin
            .members
            .iter()
            .any(|member| member.name.as_ref() == "output")
    );
    assert_eq!(
        vm.member_state(module, "output").unwrap(),
        LazyState::Unevaluated
    );

    vm.start_member(module, "Child").unwrap();
    assert_eq!(ready(&mut vm).unwrap_err().kind, ErrorKind::Type);
    assert_eq!(vm.class_singletons.len(), 2);
    let classes = vm.class_singletons.values().copied().collect::<Vec<_>>();
    let base = classes
        .iter()
        .copied()
        .find(|id| {
            vm.heap
                .get(*id)
                .unwrap()
                .class_definition
                .as_ref()
                .unwrap()
                .name
                .as_ref()
                == "Base"
        })
        .unwrap();
    let child = classes
        .iter()
        .copied()
        .find(|id| {
            vm.heap
                .get(*id)
                .unwrap()
                .class_definition
                .as_ref()
                .unwrap()
                .name
                .as_ref()
                == "Child"
        })
        .unwrap();
    assert!(vm.heap.get(base).unwrap().parent.is_none());
    assert_eq!(vm.heap.get(child).unwrap().parent, Some(base));
    for class in [base, child] {
        let scope = vm
            .heap
            .get(class)
            .unwrap()
            .environment
            .scopes
            .last()
            .unwrap();
        assert_eq!((scope.receiver, scope.owner), (module, module));
    }
    assert_eq!(
        [module, parent, base, child]
            .into_iter()
            .collect::<BTreeSet<_>>()
            .len(),
        4
    );
    assert_eq!(vm.collect(&[]).live, 4);
    vm.start_member(module, "scalar").unwrap();
    assert_eq!(ready(&mut vm), Ok(Value::Int(7)));
    assert_eq!(vm.collect(&[]).live, 4);
    assert!(vm.heap.get(parent).unwrap().cache.is_empty());
    vm.close();
    assert_eq!(vm.heap_stats().live, 0);
    assert_eq!(vm.heap_stats().slots, 0);
    assert!(vm.class_singletons.is_empty());
    for id in [module, parent, base, child] {
        assert_eq!(vm.heap.get(id).unwrap_err().kind, ErrorKind::StaleHandle);
    }
}

#[test]
fn unrooted_module_and_builtin_parent_are_reclaimed_together() {
    let program = compiled("scalar = 7\n");
    let mut vm = Vm::new(program.clone());
    vm.start(program.entry, vec![]).unwrap();
    let module = ready(&mut vm).unwrap().as_object().unwrap();
    let parent = vm.heap.get(module).unwrap().parent.unwrap();
    assert_eq!(vm.collect(&[]).live, 2);
    vm.start_member(module, "scalar").unwrap();
    assert_eq!(ready(&mut vm), Ok(Value::Int(7)));
    assert!(vm.class_singletons.is_empty());
    assert_eq!(vm.collect(&[]).live, 0);
    for id in [module, parent] {
        assert_eq!(vm.heap.get(id).unwrap_err().kind, ErrorKind::StaleHandle);
    }
    vm.start(program.entry, vec![]).unwrap();
    let replacement = ready(&mut vm).unwrap().as_object().unwrap();
    let replacement_parent = vm.heap.get(replacement).unwrap().parent.unwrap();
    assert_ne!(replacement, module);
    assert_ne!(replacement_parent, parent);
    assert_eq!(vm.collect(&[]).live, 2);
    vm.close();
    assert_eq!(vm.heap_stats().live, 0);
    assert_eq!(vm.heap_stats().slots, 0);
}
