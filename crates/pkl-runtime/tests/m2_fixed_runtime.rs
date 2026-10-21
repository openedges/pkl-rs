//! Fixed declaration invariants. Java byte comparisons live in the M2 corpus.
use pkl_runtime::parser::parse_module;
use pkl_runtime::resolver::resolve_module;
use pkl_runtime::value::{RuntimeError, Value};
use pkl_runtime::vm::{Vm, VmPoll, compile_module};

fn evaluate(source: &str) -> Result<Value, RuntimeError> {
    let parsed = parse_module(source).unwrap();
    let program = compile_module(&resolve_module(&parsed).unwrap()).unwrap();
    let mut vm = Vm::new(program.clone());
    vm.start(program.entry, vec![]).unwrap();
    let module = finish(&mut vm)?.as_object()?;
    vm.start_member(module, "result")?;
    finish(&mut vm)
}

fn finish(vm: &mut Vm) -> Result<Value, RuntimeError> {
    for _ in 0..1_000 {
        match vm.poll(100) {
            VmPoll::Ready(value) => return value,
            VmPoll::Yielded => {}
            VmPoll::Pending(_) => panic!("unexpected Host request"),
        }
    }
    panic!("fixed regression exceeded its instruction budget")
}

#[test]
fn fixed_declarations_remain_virtual_and_keep_bodies_lazy() {
    assert_eq!(
        evaluate(
            "open class A { n = 1; fixed p = n }\nclass B extends A { fixed p = super.p + 1 }\nresult = new B { n = 40 }.p"
        ),
        Ok(Value::Int(41))
    );
    assert_eq!(
        evaluate("class A { fixed p: Int = throw(\"unused\") }\nresult = A == A"),
        Ok(Value::Bool(true))
    );
    assert_eq!(
        evaluate("open class A { fixed p = 1 }\nclass B extends A { p = 2 }\nresult = 42"),
        Ok(Value::Int(42))
    );
}

#[test]
fn fixed_overrides_use_immediate_superclass_and_preserve_const_rules() {
    let error = evaluate("open class A { fixed p = 1 }\nopen class B extends A\nclass C extends B { p = 2 }\nresult = C").unwrap_err();
    assert_eq!(error.message, "Missing modifier `fixed`.");
    assert_eq!(
        error.hint.as_deref(),
        Some(
            "Property `p` must be declared fixed, because it overrides a fixed property on parent class `#B`."
        )
    );
    assert_eq!(
        evaluate(
            "open class A { const p = 1 }\nclass B extends A { const p = 2 }\nresult = new B {}.p"
        ),
        Ok(Value::Int(2))
    );
    assert_eq!(
        evaluate("open class A { fixed p = 1 }\nclass B extends A { const p = 2 }\nresult = B")
            .unwrap_err()
            .message,
        "Missing modifier `fixed`."
    );
    assert_eq!(
        evaluate("open class A { const p = 1 }\nclass B extends A { fixed p = 2 }\nresult = B")
            .unwrap_err()
            .message,
        "Modifier `fixed` cannot be applied to property `p`."
    );
}

#[test]
fn class_checks_each_property_type_before_its_modifiers_in_source_order() {
    assert_eq!(
        evaluate("open class A { fixed p = 1 }\nclass B extends A { p: Missing = 2 }\nresult = B")
            .unwrap_err()
            .message,
        "Cannot find type `Missing`."
    );
    assert_eq!(
        evaluate(
            "open class A { fixed p = 1 }\nclass B extends A { p = 2; q: Missing = 3 }\nresult = B"
        )
        .unwrap_err()
        .message,
        "Missing modifier `fixed`."
    );
    assert_eq!(
        evaluate(
            "open class A { fixed p = 1 }\nclass B extends A { q: Missing = 3; p = 2 }\nresult = B"
        )
        .unwrap_err()
        .message,
        "Cannot find type `Missing`."
    );
}

#[test]
fn fixed_assignment_and_amendment_fail_during_construction_before_bodies() {
    assert_eq!(evaluate("class A { hidden fixed p = throw(\"base\"); q = 42 }\nresult = new A { p = throw(\"replacement\") }.q").unwrap_err().message, "Cannot assign to fixed property `p`.");
    assert_eq!(
        evaluate("class A { fixed p = new { n = 1 }; q = 42 }\nresult = new A { p { n = 2 } }.q")
            .unwrap_err()
            .message,
        "Cannot amend fixed property `p`."
    );
    assert_eq!(
        evaluate(
            "class A { fixed p = new { n = 1 } }\nlocal a = new A {}\nresult = (a.p) { n = 2 }.n"
        ),
        Ok(Value::Int(2))
    );
    assert_eq!(
        evaluate("class A { fixed p = 1; q = p }\nresult = new A { local p = 2 }.q"),
        Ok(Value::Int(1))
    );
}

#[test]
fn typed_constructor_reports_first_invalid_authored_member() {
    assert_eq!(
        evaluate("class A { fixed p = 1 }\nresult = new A { missing = 2; p = 3 }.p")
            .unwrap_err()
            .message,
        "Cannot find property `missing` in object of type `#A`."
    );
    assert_eq!(
        evaluate("class A { fixed p = 1 }\nresult = new A { p = 3; missing = 2 }.p")
            .unwrap_err()
            .message,
        "Cannot assign to fixed property `p`."
    );
}
