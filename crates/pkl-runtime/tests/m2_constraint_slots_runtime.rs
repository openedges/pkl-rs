//! Logical-frame regressions. Expected receiver observations come from the pinned
//! Java Stage 16 acquisition; complete diagnostics are compared by the M2 harness.
use pkl_runtime::diagnostic::TypeDiagnostic;
use pkl_runtime::parser::parse_module;
use pkl_runtime::resolver::resolve_module;
use pkl_runtime::value::{RuntimeError, Value};
use pkl_runtime::vm::{Vm, VmPoll, compile_module, compile_module_with_power_assertions};

fn finish(vm: &mut Vm) -> Result<Value, RuntimeError> {
    for _ in 0..100_000 {
        let poll = vm.poll(1);
        vm.collect(&[]);
        match poll {
            VmPoll::Ready(result) => return result,
            VmPoll::Yielded => {}
            VmPoll::Log(_) => panic!("unexpected logger request"),
            VmPoll::Pending(_) => panic!("unexpected Host request"),
        }
    }
    panic!("constraint instruction budget exceeded")
}

fn evaluate(source: &str, power: bool) -> Result<Value, RuntimeError> {
    let parsed = parse_module(source).unwrap();
    let resolved = resolve_module(&parsed).unwrap();
    let program = if power {
        compile_module_with_power_assertions(&resolved)
    } else {
        compile_module(&resolved)
    }
    .unwrap();
    let mut vm = Vm::new(program.clone());
    vm.set_power_assertions(power);
    vm.start(program.entry, vec![]).unwrap();
    let module = finish(&mut vm).unwrap().as_object().unwrap();
    vm.start_member(module, "result").unwrap();
    finish(&mut vm)
}

fn rejection(source: &str, power: bool, expected: &str) {
    let error = evaluate(source, power).unwrap_err();
    let TypeDiagnostic::Constraint {
        value,
        observations,
        ..
    } = error
        .type_diagnostic
        .as_deref()
        .expect("constraint diagnostic")
    else {
        panic!("unexpected diagnostic: {error:?}")
    };
    assert_eq!(value, expected);
    assert_eq!(observations.is_some(), power);
}

#[test]
fn recursive_inline_checks_leave_the_final_auxiliary_receiver() {
    rejection(
        "typealias A = Int(this < 0 || (this > 0 && (this - 1) is A))\nresult: A = 2",
        false,
        "0",
    );
    for constraint in [
        "this < 0 || ((this - 1) is A && this > 0)",
        "this < 0 || ((this - 1) as A) < this",
    ] {
        rejection(
            &format!("typealias A = Int({constraint})\nresult: A = 2"),
            false,
            "-1",
        );
    }
}

#[test]
fn constrained_entry_initializes_once_and_returns_the_original_checked_value() {
    for constraint in [
        "(this - 1) is Int(true)",
        "(this - 1) is Int(true), this == 1",
        "(this - 1) is Int(true), (x) -> x == 1",
    ] {
        assert_eq!(
            evaluate(
                &format!("typealias A = Int({constraint})\nresult: A = 2"),
                false
            ),
            Ok(Value::Int(2)),
        );
    }
    rejection(
        "typealias A = Int((this - 1) is Int(true), this == 2)\nresult: A = 2",
        false,
        "1",
    );
}

#[test]
fn boolean_power_replay_starts_with_the_mutated_slot() {
    let source = "typealias A = Int(((this - 1) is Int(true)) && this == 2)\nresult: A = 2";
    rejection(source, false, "1");
    rejection(source, true, "0");
}

#[test]
fn let_roots_isolate_mutations_in_each_replay_from_the_checking_frame() {
    let source =
        "typealias A = Int(let (n = this) ((n - 1) is Int(true)) && this == 2)\nresult: A = 2";
    rejection(source, false, "2");
    rejection(source, true, "2");
}

#[test]
fn function_predicate_failures_keep_the_argument_captured_before_the_call() {
    for power in [false, true] {
        rejection(
            "typealias A = Int((this - 1) is Int(true), (x) -> x == 2)\nresult: A = 2",
            power,
            "1",
        );
        rejection(
            "typealias A = Int((x) -> ((x - 1) is Int(true)) && x == 1)\nresult: A = 2",
            power,
            "2",
        );
    }
}

#[test]
fn aliases_and_union_alternatives_keep_the_callers_slot_identity() {
    for source in [
        "typealias B = Int(true)\ntypealias A = Int((this - 1) is B, this == 1)\nresult: A = 2",
        "typealias A = Int(((this - 1) is (Int(false) | Int(this == 1))) && this == 1)\nresult: A = 2",
        "typealias A = Int(((this - 1) is Int(false) || this == 1))\nresult: A = 2",
    ] {
        assert_eq!(evaluate(source, false), Ok(Value::Int(2)));
    }
}

#[test]
fn lambda_receiver_is_a_creation_snapshot_and_real_calls_get_their_own_slot() {
    for source in [
        "typealias A = Int(let (f = (ignored) -> this) ((this - 1) is Int(true)) && f.apply(null) == 2)\nresult: A = 2",
        "typealias A = Int(let (f = (ignored) -> ((this - 1) is Int(true))) f.apply(null) && this == 2)\nresult: A = 2",
        "const function check(x: Int): Boolean = (x - 1) is Int(true)\ntypealias A = Int(check(this) && this == 2)\nresult: A = 2",
        "const holder = new { checked = 1 is Int(true) }\ntypealias A = Int(holder.checked && this == 2)\nresult: A = 2",
    ] {
        assert_eq!(evaluate(source, false), Ok(Value::Int(2)));
    }
}
