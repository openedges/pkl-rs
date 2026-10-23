//! Alias const barriers preserve lazy execution and declaration publication.
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
    panic!("alias check did not finish")
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
fn alias_const_barrier_runs_only_when_the_lexical_read_executes() {
    let (mut vm, module) = module(
        "local threshold = 0\ntypealias A = Int(this < 0 || this > threshold)\nidentity = A == A\nshort: A = -1\nforced: A = 2\nordinary = threshold",
    );
    assert_eq!(member(&mut vm, module, "identity"), Ok(Value::Bool(true)));
    assert_eq!(
        member(&mut vm, module, "short"),
        Ok(Value::Int((-1).into()))
    );
    let error = member(&mut vm, module, "forced").unwrap_err();
    assert_eq!(
        error.message,
        "Cannot reference property `threshold` from here because it is not `const`."
    );
    assert_eq!(member(&mut vm, module, "identity"), Ok(Value::Bool(true)));
    assert_eq!(
        member(&mut vm, module, "ordinary"),
        Ok(Value::Int(0.into()))
    );
}

#[test]
fn alias_const_barrier_distinguishes_methods_and_custom_input_properties() {
    let (mut vm, module) = module(
        "local function threshold() = 0\ntypealias A = Int(this > threshold())\ntypealias B = Dynamic(this.value == 42)\nforced: A = 2\nobject: B = new Dynamic { value = 42 }\nresult = object.value\nordinary = threshold()",
    );
    assert_eq!(
        member(&mut vm, module, "forced").unwrap_err().message,
        "Cannot call method `threshold` from here because it is not `const`."
    );
    assert_eq!(member(&mut vm, module, "result"), Ok(Value::Int(42.into())));
    assert_eq!(
        member(&mut vm, module, "ordinary"),
        Ok(Value::Int(0.into()))
    );
}

#[test]
fn alias_const_properties_and_methods_remain_usable_in_nested_constraints() {
    let (mut vm, module) = module(
        "local const threshold = 0\nlocal const function allowed(n) = n > threshold\ntypealias A = Int((value) -> allowed(value))\ntypealias B = A\nresult: B = 42",
    );
    assert_eq!(member(&mut vm, module, "result"), Ok(Value::Int(42.into())));
}
