//! Ordered type resolution checks. Raw compatibility evidence comes from the
//! pinned Java oracle, independently of these runtime assertions.
use pkl_runtime::parser::parse_module;
use pkl_runtime::resolver::resolve_module;
use pkl_runtime::value::{ObjectId, RuntimeError, Value};
use pkl_runtime::vm::{Vm, VmPoll, compile_module};

fn finish(vm: &mut Vm) -> Result<Value, RuntimeError> {
    for _ in 0..1000 {
        match vm.poll(100) {
            VmPoll::Ready(result) => return result,
            VmPoll::Yielded => {}
            VmPoll::Log(_) => panic!("unexpected logger request"),
            VmPoll::Pending(_) => panic!("unexpected Host request"),
        }
    }
    panic!("type resolution exceeded instruction budget")
}

fn start(source: &str) -> Vm {
    let parsed = parse_module(source).unwrap();
    let program = compile_module(&resolve_module(&parsed).unwrap()).unwrap();
    let mut vm = Vm::new(program.clone());
    vm.start(program.entry, vec![]).unwrap();
    vm
}

fn module(source: &str) -> (Vm, ObjectId) {
    let mut vm = start(source);
    let module = finish(&mut vm).unwrap().as_object().unwrap();
    (vm, module)
}

fn member(vm: &mut Vm, module: ObjectId, name: &str) -> Result<Value, RuntimeError> {
    vm.start_member(module, name).unwrap();
    finish(vm)
}

#[test]
fn module_annotations_resolve_declarations_before_later_missing_names() {
    for (ty, expected) in [
        ("Missing | Broken", "Cannot find type `Missing`."),
        ("Broken | Missing", "Cannot find type `AliasMissing`."),
        ("Invalid | Missing", "Cannot find type `ClassMissing`."),
        ("Missing<Broken>", "Cannot find type `Missing`."),
    ] {
        let source = format!(
            "typealias Broken = AliasMissing\nclass Invalid {{ p: ClassMissing }}\nx: {ty} = 1"
        );
        let mut vm = start(&source);
        assert_eq!(finish(&mut vm).unwrap_err().message, expected, "{ty}");
    }
}

#[test]
fn alias_arguments_resolve_before_arity_but_after_the_base_declaration() {
    for (ty, expected) in [
        ("Pair<Missing, Broken>", "Cannot find type `Missing`."),
        ("Pair<Broken, Missing>", "Cannot find type `AliasMissing`."),
        ("Broken<Missing>", "Cannot find type `AliasMissing`."),
        (
            "Pair<Int, String>",
            "Expected 1 type argument(s) but got 2.",
        ),
    ] {
        let source = format!("typealias Broken = AliasMissing\ntypealias Pair<X> = X\nx: {ty} = 1");
        let mut vm = start(&source);
        assert_eq!(finish(&mut vm).unwrap_err().message, expected, "{ty}");
    }
}

#[test]
fn class_arity_resolves_the_class_but_skips_its_argument_types() {
    for (class_body, expected) in [
        ("", "Expected 0 type argument(s) but got 1."),
        ("p: ClassMissing", "Cannot find type `ClassMissing`."),
    ] {
        let source = format!(
            "class C {{ {class_body} }}\ntypealias Broken = AliasMissing\nx: C<Broken> = 1"
        );
        let mut vm = start(&source);
        assert_eq!(finish(&mut vm).unwrap_err().message, expected);
    }
}

#[test]
fn generic_parameters_shadow_bare_names_but_not_parameterized_base_names() {
    let (mut vm, module) = module(
        "typealias T<X> = X\ntypealias Bare<T> = List<T>\ntypealias Applied<T> = T<String>\nbare: Bare<Int> = List(3)\napplied: Applied<Int> = \"ok\"\nresult = bare[0]",
    );
    assert_eq!(member(&mut vm, module, "result"), Ok(Value::Int(3)));
    assert_eq!(member(&mut vm, module, "applied"), Ok(Value::string("ok")));
}

#[test]
fn failed_parameterized_shadow_uses_the_module_declaration() {
    let (mut vm, module) =
        module("typealias T = ShadowMissing\ntypealias A<T> = T<String>\nresult = A");
    assert_eq!(
        member(&mut vm, module, "result").unwrap_err().message,
        "Cannot find type `ShadowMissing`."
    );
    assert!(matches!(
        member(&mut vm, module, "T"),
        Ok(Value::TypeAlias(_))
    ));
}

#[test]
fn deferred_expression_types_resolve_in_authored_order() {
    for expression in [
        "1 is (Missing | Broken)",
        "1 as (Missing | Broken)",
        "let (x: Missing | Broken = 1) x",
        "(x: Missing | Broken) -> x",
        "let (x: (Missing) -> Broken = (v) -> v) x",
    ] {
        let source = format!("typealias Broken = AliasMissing\nresult = {expression}");
        let (mut vm, module) = module(&source);
        assert_eq!(
            member(&mut vm, module, "result").unwrap_err().message,
            "Cannot find type `Missing`.",
            "{expression}"
        );
    }
}

#[test]
fn unfinished_alias_checks_arguments_before_its_initialized_state() {
    let (mut vm, module) = module(
        "typealias A<T> = Missing\nfirst = A\nmissing = 1 is A<ArgumentMissing>\nwrong = 1 is A<Int, String>\nvalid = 1 is A<Int>",
    );
    assert_eq!(
        member(&mut vm, module, "first").unwrap_err().message,
        "Cannot find type `Missing`."
    );
    for _ in 0..2 {
        assert_eq!(
            member(&mut vm, module, "missing").unwrap_err().message,
            "Cannot find type `ArgumentMissing`."
        );
        for name in ["wrong", "valid"] {
            assert_eq!(
                member(&mut vm, module, name).unwrap_err().message,
                "Type alias definitions must not be cyclic."
            );
        }
    }
}

#[test]
fn parameterized_shadow_union_diagnostics_retain_the_authored_alternative() {
    let source = "typealias T<X> = X\ntypealias A<T> = T<String> | Int\np: A<Boolean> = false";
    let (mut vm, module) = module(source);
    let error = member(&mut vm, module, "p").unwrap_err();
    let pkl_runtime::diagnostic::TypeDiagnostic::Union { children, .. } =
        *error.type_diagnostic.unwrap()
    else {
        panic!("expected union diagnostic");
    };
    let labels: Vec<_> = children
        .iter()
        .map(|child| &source[child.type_span.start..child.type_span.end])
        .collect();
    assert_eq!(labels, ["T<String>", "Int"]);
}
