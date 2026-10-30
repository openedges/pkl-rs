//! Shared VM regressions; pinned Java raw-byte comparisons live in the M2 harness.
#[path = "support/module_policy.rs"]
mod module_policy;
use pkl_runtime::parser::parse_module;
use pkl_runtime::resolver::resolve_module;
use pkl_runtime::value::{ObjectId, RuntimeError, Value};
use pkl_runtime::vm::{Vm, VmPoll, compile_module};

fn finish(vm: &mut Vm) -> Result<Value, RuntimeError> {
    for _ in 0..1_000 {
        match vm.poll(100) {
            pkl_runtime::vm::VmPoll::Glob(_) => {
                panic!("unexpected glob request in non-glob regression")
            }
            VmPoll::Ready(result) => return result,
            VmPoll::Yielded => {}
            VmPoll::Log(_) | VmPoll::TransformFrame(_) => panic!("unexpected logger request"),
            VmPoll::Module(request) => vm
                .complete_module(
                    request.id,
                    pkl_runtime::modules::default_completion(&request)
                        .expect("default module policy"),
                )
                .unwrap(),
            VmPoll::Pending(_) => panic!("unexpected Host request"),
        }
    }
    panic!("constraint instruction budget exceeded")
}

fn module(source: &str) -> (Vm, ObjectId) {
    let parsed = parse_module(source).unwrap();
    let mut program = compile_module(&resolve_module(&parsed).unwrap()).unwrap();
    module_policy::register(std::sync::Arc::make_mut(&mut program), source);
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
fn class_arguments_are_erased_without_publishing_their_aliases() {
    let (mut vm, module) = module(
        "class C {}\ntypealias Broken = Missing\nvalue: Class<Broken> = C\nmissing = C is Class<Missing>\ncast = (C as Class<Missing>) == C\nidentity = value == C\nfirst = Broken\nsecond = Broken == Broken",
    );
    for name in ["missing", "cast", "identity"] {
        assert_eq!(member(&mut vm, module, name), Ok(Value::Bool(true)));
    }
    assert_eq!(
        member(&mut vm, module, "first").unwrap_err().message,
        "Cannot find type `Missing`."
    );
    assert_eq!(member(&mut vm, module, "second"), Ok(Value::Bool(true)));
}

#[test]
fn class_argument_arity_precedes_child_resolution_and_shadowing_is_preserved() {
    let (mut vm, module) = module("class C {}\nwrong = C is Class<Missing, AlsoMissing>");
    assert_eq!(
        member(&mut vm, module, "wrong").unwrap_err().message,
        "Expected 1 type argument(s) but got 2."
    );
    let (mut vm, module) = self::module("typealias Class<T> = T\nshadow: Class<Int> = 42");
    assert_eq!(member(&mut vm, module, "shadow"), Ok(Value::Int(42.into())));
}

#[test]
fn class_erasure_is_retained_in_reference_metadata() {
    let (mut vm, module) = module(
        "import \"pkl:ref\"\nclass D extends ref.Domain {}\nlocal d = new D {}\nclass C { p: Class<Missing> }\nresult = ref.Reference(d, C, null).p is ref.Reference<D, Class>",
    );
    assert_eq!(member(&mut vm, module, "result"), Ok(Value::Bool(true)));
}

#[test]
fn recursive_constraints_compile_as_deferred_blocks_and_check_finite_values() {
    let (mut vm, module) = module(
        "typealias A = Int(this < 0 || (this - 1) is A)\nunused = 42\nidentity = A == A\nshort: A = -1\nfinite: A = 4\nnegative = \"text\" is A",
    );
    assert_eq!(member(&mut vm, module, "unused"), Ok(Value::Int(42.into())));
    assert_eq!(member(&mut vm, module, "identity"), Ok(Value::Bool(true)));
    assert_eq!(
        member(&mut vm, module, "short"),
        Ok(Value::Int((-1).into()))
    );
    assert_eq!(member(&mut vm, module, "finite"), Ok(Value::Int(4.into())));
    assert_eq!(member(&mut vm, module, "negative"), Ok(Value::Bool(false)));
}

#[test]
fn mutually_recursive_constraints_and_let_roots_use_the_same_vm_scheduler() {
    let (mut vm, module) = module(
        "typealias A = Int(let (n = this) n < 0 || (n - 1) is B)\ntypealias B = Int(this < 0 || (this - 1) is A)\nvalue: A = 4\nother: B = 3",
    );
    assert_eq!(member(&mut vm, module, "value"), Ok(Value::Int(4.into())));
    assert_eq!(member(&mut vm, module, "other"), Ok(Value::Int(3.into())));
}

#[test]
fn recursive_constraint_failure_retries_without_poisoning_the_declaration() {
    let (mut vm, module) = module(
        "typealias A = Int(this > 0 && (this - 1) is A)\nfailed = 2 is A\nidentity = A == A\nwrong: A = 2",
    );
    assert_eq!(member(&mut vm, module, "failed"), Ok(Value::Bool(false)));
    let error = member(&mut vm, module, "wrong").unwrap_err();
    assert_eq!(member(&mut vm, module, "wrong").unwrap_err(), error);
    assert_eq!(member(&mut vm, module, "identity"), Ok(Value::Bool(true)));
}

#[test]
fn recursively_expanding_generic_constraints_can_be_cancelled_after_bounded_progress() {
    let (mut vm, module) = module(
        "typealias A<T> = Int(this < 0 || this is A<List<T>>)\nunused = 42\nidentity = A == A\nshort: A<Int> = -1\nunsupported: A<Int> = 1",
    );
    assert_eq!(member(&mut vm, module, "unused"), Ok(Value::Int(42.into())));
    assert_eq!(member(&mut vm, module, "identity"), Ok(Value::Bool(true)));
    assert_eq!(
        member(&mut vm, module, "short"),
        Ok(Value::Int((-1).into()))
    );
    // Preserve the original nonterminating input. Generic expansion now runs;
    // bounded progress and cancellation are the contract of this regression.
    // Unbounded by-value argument growth is a separately recorded stack limit,
    // not a compatibility pass or an implicit successful evaluation.
    vm.start_member(module, "unsupported").unwrap();
    for _ in 0..3 {
        assert!(matches!(vm.poll(100), VmPoll::Yielded));
    }
    vm.cancel();
    assert_eq!(member(&mut vm, module, "unused"), Ok(Value::Int(42.into())));
    assert_eq!(member(&mut vm, module, "identity"), Ok(Value::Bool(true)));
    assert_eq!(
        member(&mut vm, module, "short"),
        Ok(Value::Int((-1).into()))
    );
}

#[test]
fn recursive_constraint_type_names_keep_the_alias_declaration_scope() {
    let (mut vm, module) = module(
        "typealias A = Int(this < 0 || (this - 1) is A)\ntypealias B = A\nclass Holder { local A = 42; value: B = 2 }\nresult = new Holder {}.value",
    );
    assert_eq!(member(&mut vm, module, "result"), Ok(Value::Int(2.into())));
}

#[test]
fn recursive_constraint_blocks_preserve_power_assertion_observations_and_retry() {
    use pkl_runtime::diagnostic::TypeDiagnostic;
    use pkl_runtime::vm::compile_module_with_power_assertions;
    let parsed = parse_module(
        "typealias A = Int(let (n = this) n < 0 || ((n - 1) is A && n < 2))\nresult: A = 2",
    )
    .unwrap();
    let program = compile_module_with_power_assertions(&resolve_module(&parsed).unwrap()).unwrap();
    let mut vm = Vm::new(program.clone());
    vm.set_power_assertions(true);
    vm.start(program.entry, vec![]).unwrap();
    let module = finish(&mut vm).unwrap().as_object().unwrap();
    let error = member(&mut vm, module, "result").unwrap_err();
    let Some(diagnostic) = &error.type_diagnostic else {
        panic!("missing constraint diagnostic")
    };
    let TypeDiagnostic::Constraint {
        observations: Some(observations),
        ..
    } = &**diagnostic
    else {
        panic!("missing recursive observations")
    };
    assert!(
        observations
            .iter()
            .any(|observation| observation.value == "false")
    );
    assert!(
        observations
            .iter()
            .any(|observation| observation.value == "2")
    );
    assert_eq!(member(&mut vm, module, "result").unwrap_err(), error);
}
