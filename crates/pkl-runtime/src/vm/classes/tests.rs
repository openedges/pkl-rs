use super::*;

fn settle(vm: &mut Vm) -> Result<Value, RuntimeError> {
    for _ in 0..10_000 {
        match vm.poll(10) {
            VmPoll::Ready(value) => return value,
            VmPoll::Yielded => {
                vm.collect(&[]);
            }
            event => panic!("unexpected Class test event: {event:?}"),
        }
    }
    panic!("Class test did not settle")
}

fn module(source: &str) -> (Vm, ObjectId) {
    let parsed = crate::parser::parse_module(source).unwrap();
    let program = compile_module(&crate::resolver::resolve_module(&parsed).unwrap()).unwrap();
    let mut vm = Vm::new(program.clone());
    vm.start(program.entry, vec![]).unwrap();
    let module = settle(&mut vm).unwrap().as_object().unwrap();
    (vm, module)
}

fn member(vm: &mut Vm, module: ObjectId, name: &str) -> Result<Value, RuntimeError> {
    vm.start_member(module, name).unwrap();
    settle(vm)
}

#[test]
fn class_values_are_distinct_from_prototypes_and_follow_real_superclasses() {
    let (mut vm, module) = module(
        "open class Base extends Typed { x = 1 }\nclass Child extends Base\nu = new Child {}\na = u.getClass() == Child\nb = Class.getClass().simpleName\nc = module.getClass().simpleName\nd = u is Typed && u is Object && u is Any",
    );
    let Value::Class(child) = member(&mut vm, module, "Child").unwrap() else {
        panic!("Class")
    };
    let Value::Class(base) = member(&mut vm, module, "Base").unwrap() else {
        panic!("Class")
    };
    let prototype = vm.heap.get_class(child).unwrap().prototype;
    assert_ne!(child, prototype);
    assert!(vm.heap.get(child).is_err());
    assert_eq!(vm.heap.object_class(prototype).unwrap(), Some(child));
    assert_eq!(vm.heap.get_class(child).unwrap().superclass, Some(base));
    let typed = vm.canonical_builtin_class(BuiltinClass::Typed);
    assert_eq!(vm.heap.get_class(base).unwrap().superclass, Some(typed));
    assert_eq!(
        member(&mut vm, module, "u").unwrap(),
        Value::Object(prototype)
    );
    assert_eq!(member(&mut vm, module, "a").unwrap(), Value::Bool(true));
    assert_eq!(
        member(&mut vm, module, "b").unwrap(),
        Value::string("Class")
    );
    assert_eq!(
        member(&mut vm, module, "c").unwrap(),
        Value::string("ModuleClass")
    );
    assert_eq!(member(&mut vm, module, "d").unwrap(), Value::Bool(true));
    vm.close();
    assert!(vm.heap.get_class(child).is_err());
    assert_eq!(vm.heap_stats().live, 0);
}

#[test]
fn class_member_lookup_does_not_read_or_force_instance_members() {
    let (mut vm, module) = module(
        "class Person { foo = throw(\"instance only\"); function bar() = 2 }\na = Person.foo\nb = Person.bar()\nc = Person.simpleName\nd = Person.getClass() == Class",
    );
    assert_eq!(
        member(&mut vm, module, "a").unwrap_err().message,
        "Cannot find property `foo` in object of type `Class`."
    );
    assert_eq!(
        member(&mut vm, module, "b").unwrap_err().message,
        "Cannot find method `bar` in class `Class`."
    );
    assert_eq!(
        member(&mut vm, module, "c").unwrap(),
        Value::string("Person")
    );
    assert_eq!(member(&mut vm, module, "d").unwrap(), Value::Bool(true));
}

#[test]
fn virtual_get_class_override_does_not_change_intrinsic_class_identity() {
    let (mut vm, module) = module(
        "class User { function getClass(): Class = String }\nu = new User {}\nexplicit = u.getClass() == String\nnominal = u is User && !(u is String)",
    );
    assert_eq!(
        member(&mut vm, module, "explicit").unwrap(),
        Value::Bool(true)
    );
    assert_eq!(
        member(&mut vm, module, "nominal").unwrap(),
        Value::Bool(true)
    );
    let user = member(&mut vm, module, "User").unwrap();
    let value = member(&mut vm, module, "u").unwrap();
    assert_eq!(vm.reference_get_class(&value).unwrap(), user);
}

#[test]
fn retained_class_traces_its_prototype_and_superclass_without_leaking_cycles() {
    let mut heap = Heap::new();
    let base_prototype = heap.allocate(Object::default());
    let base = heap.allocate_class(base_prototype);
    heap.link_superclass(base, None).unwrap();
    let prototype = heap.allocate(Object::default());
    let class = heap.allocate_class(prototype);
    heap.link_superclass(class, Some(base)).unwrap();
    assert_eq!(heap.collect(&[Value::Class(class)]).live, 4);
    assert_eq!(heap.get_class(class).unwrap().prototype, prototype);
    assert_eq!(heap.get(prototype).unwrap().parent, Some(base_prototype));
    assert_eq!(heap.collect(&[]).live, 0);
    assert!(heap.get_class(class).is_err());
    assert!(heap.get(prototype).is_err());
}
