//! ClassNode publication and ordinary lazy-cache boundaries. Raw Java evidence
//! is recorded separately by the persistent evaluator comparison harness.
use pkl_runtime::heap::LazyState;
use pkl_runtime::parser::parse_module;
use pkl_runtime::resolver::resolve_module;
use pkl_runtime::value::{ErrorKind, ObjectId, RuntimeError, Value};
use pkl_runtime::vm::{Vm, VmPoll, compile_module};

fn finish(vm: &mut Vm) -> Result<Value, RuntimeError> {
    for _ in 0..1_000 {
        match vm.poll(100) {
            VmPoll::Ready(result) => return result,
            VmPoll::Yielded => {}
            VmPoll::Pending(_) => panic!("unexpected Host request"),
        }
    }
    panic!("class publication instruction budget exceeded")
}

fn module(source: &str) -> (Vm, ObjectId) {
    let parsed = parse_module(source).unwrap();
    let program = compile_module(&resolve_module(&parsed).unwrap()).unwrap();
    let mut vm = Vm::new(program.clone());
    vm.start(program.entry, vec![]).unwrap();
    let module = finish(&mut vm).unwrap().as_object().unwrap();
    (vm, module)
}

fn member(vm: &mut Vm, module: ObjectId, name: &str) -> Result<Value, RuntimeError> {
    vm.start_member(module, name)?;
    finish(vm)
}

#[test]
fn failed_class_keeps_prefix_and_wrapper_retries_without_reinitialization() {
    let (mut vm, module) = module(
        "open class Base { fixed p = 42 }\nclass Child extends Base { early = 10; p = 0; later = 20 }\ninvalid = Child\nearly = new Child {}.early\ninherited = new Child {}.p\nlater = new Child {}.later\nassigned = new Child { novel = 99; later = 7 }.novel\nfixedAssignment = new Child { p = 2 }.p",
    );
    assert_eq!(
        member(&mut vm, module, "invalid").unwrap_err().message,
        "Missing modifier `fixed`."
    );
    assert_eq!(
        vm.member_state(module, "invalid").unwrap(),
        LazyState::Unevaluated
    );
    assert_eq!(
        vm.member_state(module, "Child").unwrap(),
        LazyState::Unevaluated
    );
    let class = member(&mut vm, module, "invalid").unwrap();
    assert!(matches!(class, Value::Class(_)));
    assert_eq!(member(&mut vm, module, "Child"), Ok(class.clone()));
    assert_eq!(
        vm.member_state(module, "Child").unwrap(),
        LazyState::Completed(class)
    );
    assert_eq!(member(&mut vm, module, "early"), Ok(Value::Int(10)));
    assert_eq!(member(&mut vm, module, "inherited"), Ok(Value::Int(42)));
    assert_eq!(
        member(&mut vm, module, "later").unwrap_err().kind,
        ErrorKind::MissingMember
    );
    assert_eq!(member(&mut vm, module, "assigned"), Ok(Value::Int(99)));
    assert_eq!(
        member(&mut vm, module, "fixedAssignment")
            .unwrap_err()
            .message,
        "Cannot assign to fixed property `p`."
    );
}

#[test]
fn properties_publish_before_methods_and_failed_method_keeps_only_method_prefix() {
    let (mut vm, module) = module(
        "class C { function early(): Int = 5; function broken(x: Missing) = 0; p = 42; function later() = 6 }\nproperty = new C {}.p\nearly = new C {}.early()\nlater = new C {}.later()",
    );
    assert_eq!(
        member(&mut vm, module, "C").unwrap_err().message,
        "Cannot find type `Missing`."
    );
    assert!(matches!(member(&mut vm, module, "C"), Ok(Value::Class(_))));
    assert_eq!(member(&mut vm, module, "property"), Ok(Value::Int(42)));
    assert_eq!(member(&mut vm, module, "early"), Ok(Value::Int(5)));
    assert_eq!(
        member(&mut vm, module, "later").unwrap_err().kind,
        ErrorKind::MissingMember
    );

    let (mut vm, module) =
        self::module("class C { function early() = 5; p: Missing = 42 }\nearly = new C {}.early()");
    assert_eq!(
        member(&mut vm, module, "C").unwrap_err().message,
        "Cannot find type `Missing`."
    );
    assert_eq!(
        member(&mut vm, module, "early").unwrap_err().kind,
        ErrorKind::MissingMember
    );
}

#[test]
fn superclass_failures_leave_an_empty_singleton_before_parent_linking() {
    for (declarations, error) in [
        (
            "class C extends Missing { own = 1 }",
            "Cannot find type `Missing`.",
        ),
        (
            "class Base { inherited = 2 }\nclass C extends Base { own = 1 }",
            "Cannot extend non-open class `#Base`.",
        ),
        (
            "class C extends C { own = 1 }",
            "Class `#C` cannot extend itself.",
        ),
    ] {
        let source = format!(
            "{declarations}\nassigned = new C {{ novel = 42 }}.novel\nown = new C {{}}.own"
        );
        let (mut vm, module) = module(&source);
        assert_eq!(member(&mut vm, module, "C").unwrap_err().message, error);
        let class = member(&mut vm, module, "C").unwrap();
        assert!(matches!(class, Value::Class(_)));
        assert_eq!(member(&mut vm, module, "C"), Ok(class));
        assert_eq!(member(&mut vm, module, "assigned"), Ok(Value::Int(42)));
        assert_eq!(
            member(&mut vm, module, "own").unwrap_err().kind,
            ErrorKind::MissingMember
        );
    }
}

#[test]
fn recursive_types_reuse_published_class_without_bypassing_ordinary_cycles() {
    let (mut vm, module) = module(
        "class A { A = 99; b: B? = null; self: A? = null }\nclass B { a: A? = null }\nresult = new A {}.self == null\ncycle = cycle",
    );
    assert_eq!(member(&mut vm, module, "result"), Ok(Value::Bool(true)));
    let class = member(&mut vm, module, "A").unwrap();
    assert_eq!(member(&mut vm, module, "A"), Ok(class));
    for _ in 0..2 {
        assert_eq!(
            member(&mut vm, module, "cycle").unwrap_err().kind,
            ErrorKind::Cycle
        );
        assert_eq!(
            vm.member_state(module, "cycle").unwrap(),
            LazyState::Unevaluated
        );
    }
}

#[test]
fn ordinary_failure_after_completed_class_does_not_reset_class_or_cache_error() {
    let (mut vm, module) = module(
        "class C { p = throw(\"body\") }\ninstance = new C {}\nfailed = instance.p\nordinary = throw(\"ordinary\")",
    );
    let class = member(&mut vm, module, "C").unwrap();
    for _ in 0..2 {
        let before = vm.member_evaluations();
        assert!(member(&mut vm, module, "failed").is_err());
        assert!(vm.member_evaluations() > before);
        assert_eq!(
            vm.member_state(module, "failed").unwrap(),
            LazyState::Unevaluated
        );
        assert!(member(&mut vm, module, "ordinary").is_err());
        assert_eq!(
            vm.member_state(module, "ordinary").unwrap(),
            LazyState::Unevaluated
        );
        assert_eq!(member(&mut vm, module, "C"), Ok(class.clone()));
    }
}

#[test]
fn indirect_superclass_backedges_preserve_class_identity() {
    let (mut vm, module) =
        module("open class A extends B\nopen class B extends A\nisA = A == A\nisB = B == B");
    assert_eq!(member(&mut vm, module, "isA"), Ok(Value::Bool(true)));
    assert_eq!(member(&mut vm, module, "isB"), Ok(Value::Bool(true)));
    assert_eq!(member(&mut vm, module, "isA"), Ok(Value::Bool(true)));
    // This covers publication identity only. Java hierarchy traversal on this
    // cyclic graph is separately retained as a timed-out oracle observation.
}

#[test]
fn late_superclass_metadata_remains_distinct_from_prototype_override() {
    let (mut vm, module) = module(
        "open class A { b: B? = null; fixed p = 42 }\nclass B extends A { p = 2 }\ntrigger = A == A\nvalue = new B {}.p\nassignment = new B { p = 3 }.p",
    );
    assert_eq!(member(&mut vm, module, "trigger"), Ok(Value::Bool(true)));
    assert_eq!(member(&mut vm, module, "value"), Ok(Value::Int(2)));
    assert_eq!(
        member(&mut vm, module, "assignment").unwrap_err().message,
        "Cannot assign to fixed property `p`."
    );
}

#[test]
fn missing_members_list_only_published_definitions_and_inherited_methods() {
    let (mut vm, module) = module(
        "module publication\nclass C { early = 1; function before(): Int = 7; function broken(): Missing = 8; function after(): Int = 9 }\nmissingProperty = new C {}.unpublished\nmissingMethod = new C {}.after()",
    );
    assert!(member(&mut vm, module, "C").is_err());
    let property = member(&mut vm, module, "missingProperty").unwrap_err();
    assert_eq!(
        property.message,
        "Cannot find property `unpublished` in object of type `publication#C`."
    );
    assert_eq!(
        property.hint.as_deref(),
        Some("Available properties:\nearly")
    );
    let method = member(&mut vm, module, "missingMethod").unwrap_err();
    assert_eq!(
        method.message,
        "Cannot find method `after` in class `publication#C`."
    );
    assert_eq!(
        method.hint.as_deref(),
        Some(
            "Available methods in class `publication#C`:\nbefore\ngetClass\ngetProperty\ngetPropertyOrNull\nhasProperty\nifNonNull\ntoDynamic\ntoMap\ntoString"
        )
    );
}

#[test]
fn missing_method_candidates_include_published_property_and_method_signatures() {
    let (mut vm, module) = module(
        "class C { later = 1; function before(value) = value; function broken(): Missing = 8 }\npropertyCandidate = new C {}.after()\nmethodCandidate = new C {}.befor(1)",
    );
    assert!(member(&mut vm, module, "C").is_err());
    assert_eq!(
        member(&mut vm, module, "propertyCandidate")
            .unwrap_err()
            .hint
            .as_deref(),
        Some("Did you mean any of the following?\nlater")
    );
    assert_eq!(
        member(&mut vm, module, "methodCandidate")
            .unwrap_err()
            .hint
            .as_deref(),
        Some("Did you mean any of the following?\nbefore(value)")
    );
}
