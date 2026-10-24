//! Identity/ownership checks; public Java string observations are separate.
use super::*;

fn settle(vm: &mut Vm, roots: &[Value]) -> Value {
    for _ in 0..100_000 {
        vm.collect(roots);
        match vm.poll(1) {
            VmPoll::Yielded => {}
            VmPoll::Ready(value) => return value.unwrap(),
            event => panic!("unexpected identity event: {event:?}"),
        }
    }
    panic!("amendment identity poll bound")
}

#[test]
fn zero_member_amendments_preserve_parent_and_class_empty_identity() {
    // EmptyObjectLiteralNode and GeneratorObjectLiteralNode return the input
    // object or the class's canonical empty/prototype. This tests handles,
    // which evaluateExpressionString cannot expose.
    let source = r#"
class Box { value = 1 }
box = new Box { value = 2 }
boxEmpty = (box) {}
boxZero = (box) { when (false) { value = throw("unused") } }
classEmpty = (Box) {}
classZero = (Box) { for (x in List()) { [x] = x } }
dynamicEmpty = (Dynamic) {}
dynamicZero = (Dynamic) { when (false) { value = 2 } }
dynamicAgain = (dynamicEmpty) { ...new Dynamic {} }
listingEmpty = (Listing) {}
listingZero = (Listing) { for (x in List()) { x } }
listingAgain = (listingEmpty) { when (false) { 1 } }
mappingEmpty = (Mapping) {}
mappingZero = (Mapping) { when (false) { ["key"] = 1 } }
mappingAgain = (mappingEmpty) { ...new Mapping {} }
"#;
    let parsed = crate::parser::parse_module(source).unwrap();
    let program = compile_module(&crate::resolver::resolve_module(&parsed).unwrap()).unwrap();
    let mut vm = Vm::new(program.clone());
    vm.start(program.entry, vec![]).unwrap();
    let module = settle(&mut vm, &[]);
    let id = module.as_object().unwrap();
    let mut values = BTreeMap::new();
    for name in [
        "box",
        "boxEmpty",
        "boxZero",
        "classEmpty",
        "classZero",
        "dynamicEmpty",
        "dynamicZero",
        "dynamicAgain",
        "listingEmpty",
        "listingZero",
        "listingAgain",
        "mappingEmpty",
        "mappingZero",
        "mappingAgain",
    ] {
        vm.start_member(id, name).unwrap();
        values.insert(name, settle(&mut vm, std::slice::from_ref(&module)));
    }
    for names in [
        vec!["box", "boxEmpty", "boxZero"],
        vec!["classEmpty", "classZero"],
        vec!["dynamicEmpty", "dynamicZero", "dynamicAgain"],
        vec!["listingEmpty", "listingZero", "listingAgain"],
        vec!["mappingEmpty", "mappingZero", "mappingAgain"],
    ] {
        for name in &names[1..] {
            assert_eq!(values[names[0]], values[name]);
        }
    }
    for name in ["dynamicEmpty", "listingEmpty", "mappingEmpty"] {
        let empty = vm.heap.get(values[name].as_object().unwrap()).unwrap();
        assert!(empty.members.is_empty());
        assert!(empty.environment.scopes.is_empty());
        let prototype = vm.heap.get(empty.parent.unwrap()).unwrap();
        assert!(prototype.class_definition.is_some());
        assert_ne!(values[name].as_object().unwrap(), empty.parent.unwrap());
    }
    vm.close();
    assert_eq!(vm.collect(&[]).slots, 0);
}

#[test]
fn empty_object_cache_is_weak_and_rejects_collected_generations() {
    let parsed = crate::parser::parse_module("").unwrap();
    let program = compile_module(&crate::resolver::resolve_module(&parsed).unwrap()).unwrap();
    let mut vm = Vm::new(program);
    for kind in [
        CollectionKind::Dynamic,
        CollectionKind::Listing,
        CollectionKind::Mapping,
    ] {
        let first = vm.empty_object(kind);
        vm.collect(&[Value::Object(first)]);
        assert_eq!(first, vm.empty_object(kind));
        vm.collect(&[]);
        assert!(vm.heap.get(first).is_err());
        let second = vm.empty_object(kind);
        assert_ne!(first, second);
        assert!(vm.heap.get(second).unwrap().environment.scopes.is_empty());
    }
    vm.close();
    assert!(vm.empty_objects.iter().all(Option::is_none));
    assert_eq!(vm.collect(&[]).slots, 0);
}
