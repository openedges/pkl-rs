//! Generic constraint templates and call-target substitution boundaries.
use pkl_runtime::parser::parse_module;
use pkl_runtime::resolver::resolve_module;
use pkl_runtime::value::{ObjectId, RuntimeError, Value};
use pkl_runtime::vm::{Vm, VmPoll, compile_module};

fn finish(vm: &mut Vm) -> Result<Value, RuntimeError> {
    for _ in 0..1_000 {
        match vm.poll(100) {
            VmPoll::Ready(result) => return result,
            VmPoll::Yielded => {}
            VmPoll::Log(_) => panic!("unexpected logger request"),
            VmPoll::Module(_) => panic!("unexpected module request"),
            VmPoll::Pending(_) => panic!("unexpected Host request"),
        }
    }
    panic!("instruction budget exceeded")
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

fn const_failure(result: Result<Value, RuntimeError>, kind: &str, name: &str) {
    let verb = if kind == "method" {
        "call"
    } else {
        "reference"
    };
    assert_eq!(
        result.unwrap_err().message,
        format!("Cannot {verb} {kind} `{name}` from here because it is not `const`.")
    );
}

#[test]
fn recursive_generic_parameter_shadows_module_alias() {
    let (mut vm, module) = module(
        "typealias T=ShadowMissing\ntypealias A<T> =Int(this<0||(this-1)is A<T>)\nresult:A<String> =2",
    );
    assert_eq!(member(&mut vm, module, "result"), Ok(Value::Int(2)));
}

#[test]
fn recursive_generic_arguments_change_without_specializing_bytecode() {
    let (mut vm, module) =
        module("typealias A<T> =Int(this<0||(this-1)is A<List<T>>)\nresult:A<String> =2");
    assert_eq!(member(&mut vm, module, "result"), Ok(Value::Int(2)));
}

#[test]
fn recursive_type_variable_checks_use_the_current_argument() {
    let (mut vm, module) = module(
        "typealias A<T> =Int(this<0||((this-1)is A<String> && 0 is T))\none=0 is A<Int>\ntwo=1 is A<Int>\nthree=0 is A<String>",
    );
    assert_eq!(member(&mut vm, module, "one"), Ok(Value::Bool(true)));
    assert_eq!(member(&mut vm, module, "two"), Ok(Value::Bool(false)));
    assert_eq!(member(&mut vm, module, "three"), Ok(Value::Bool(false)));
}

#[test]
fn generic_let_root_shares_resolution_across_argument_types() {
    let (mut vm, module) = module(
        "local threshold=0\ntypealias A<T> =T(let(n=this)n<threshold)\nfirst:A<Int> =-1\nsecond:A<Int> =-2\nother:A<Float> =-3.0",
    );
    const_failure(member(&mut vm, module, "first"), "property", "threshold");
    assert_eq!(member(&mut vm, module, "second"), Ok(Value::Int(-2)));
    assert_eq!(member(&mut vm, module, "other"), Ok(Value::Float(-3.0)));
    assert_eq!(member(&mut vm, module, "first"), Ok(Value::Int(-1)));
}

#[test]
fn substitution_crosses_inline_and_lambda_but_not_let_call_targets() {
    for (constraint, expected) in [
        ("this is T", false),
        ("(n)->n is T", false),
        ("let(n=this)n is T", true),
    ] {
        let (mut vm, module) = module(&format!(
            "typealias A<T> =Int({constraint})\none=1 is A<Int>\ntwo=2 is A<String>"
        ));
        assert_eq!(member(&mut vm, module, "one"), Ok(Value::Bool(true)));
        assert_eq!(member(&mut vm, module, "two"), Ok(Value::Bool(expected)));
    }
}

#[test]
fn generic_collection_amendments_instantiate_actual_class_defaults() {
    // Each expression is captured against pinned Java and the Stage 17 binary
    // in stage18-review-default-java-01 / stage18-review-default-prior-01.
    for body in [
        "(new Mapping<String, T> {[\"k\"] {}})[\"k\"].p == 1",
        "(new Listing<T> {new {}})[0].p == 1",
        "(new Mapping<String, Mapping<String,T>> {[\"outer\"] { [\"inner\"] {} }})[\"outer\"][\"inner\"].p == 1",
    ] {
        let (mut vm, module) = module(&format!(
            "class C {{ p=1 }}\ntypealias A<T> = Int({body})\nresult: A<C> = 1"
        ));
        assert_eq!(member(&mut vm, module, "result"), Ok(Value::Int(1)));
    }
}

#[test]
fn substituted_reference_referents_validate_when_the_authored_use_is_reached() {
    for body in [
        "throw(\"value\") is ref.Reference<D,T>",
        "(x) -> throw(\"value\") is ref.Reference<D,T>",
    ] {
        let (mut vm, module) = module(&format!(
            "import \"pkl:ref\"\nclass D extends ref.Domain {{}}\ntypealias Positive=Int(this>0)\ntypealias A<T> = Int({body})\nresult=1 is A<Positive>\nplain=1 is A<Int>"
        ));
        assert_eq!(
            member(&mut vm, module, "result").unwrap_err().message,
            "`Reference` referent type argument may not include type constraints."
        );
        assert_eq!(
            member(&mut vm, module, "plain").unwrap_err().message,
            "value"
        );
    }
    let (mut vm, module) = module(
        "import \"pkl:ref\"\nclass D extends ref.Domain {}\ntypealias Positive=Int(this>0)\ntypealias A<T> = Int(this>0 || this is ref.Reference<D,T>)\nshort=1 is A<Positive>\nreached=0 is A<Positive>\ntypealias L<T> = Int(let(n=this)n is ref.Reference<D,T>)\nunbound=0 is L<Positive>",
    );
    assert_eq!(member(&mut vm, module, "short"), Ok(Value::Bool(true)));
    assert_eq!(
        member(&mut vm, module, "reached").unwrap_err().message,
        "`Reference` referent type argument may not include type constraints."
    );
    assert_eq!(member(&mut vm, module, "unbound"), Ok(Value::Bool(false)));
}
