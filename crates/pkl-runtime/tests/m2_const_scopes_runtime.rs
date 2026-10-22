//! Const method barriers and lambda lookup depth follow distinct lexical rules.
use pkl_runtime::parser::parse_module;
use pkl_runtime::resolver::resolve_module;
use pkl_runtime::value::{RuntimeError, Value};
use pkl_runtime::vm::{Vm, VmPoll, compile_module};

fn finish(vm: &mut Vm) -> Result<Value, RuntimeError> {
    for _ in 0..1_000 {
        match vm.poll(100) {
            VmPoll::Ready(result) => return result,
            VmPoll::Yielded => {}
            VmPoll::Log(_) => panic!("unexpected logger request"),
            VmPoll::Pending(_) => panic!("unexpected Host request"),
        }
    }
    panic!("const scope evaluation did not finish")
}

fn result(source: &str) -> Result<Value, RuntimeError> {
    let module = parse_module(source).unwrap();
    let program = compile_module(&resolve_module(&module).unwrap()).unwrap();
    let mut vm = Vm::new(program.clone());
    vm.start(program.entry, vec![]).unwrap();
    let module = finish(&mut vm).unwrap().as_object().unwrap();
    vm.start_member(module, "result").unwrap();
    finish(&mut vm)
}

#[test]
fn const_methods_allow_lexical_reads_and_signature_constraints() {
    for body in ["x", "this.y", "other()", "let (n = x) n"] {
        let source = format!(
            "local x = 7\ny = 7\nfunction other() = 7\nconst function f() = {body}\nresult = f()"
        );
        assert_eq!(result(&source), Ok(Value::Int(7)), "{body}");
    }
    assert_eq!(
        result(
            "local limit = 7\nconst function f(n: Int(this < limit)): Int(this < limit) = n\nresult = f(2)"
        ),
        Ok(Value::Int(2))
    );
}

#[test]
fn lambdas_do_not_add_method_const_barriers() {
    for body in [
        "(() -> x).apply()",
        "new Dynamic { z = (() -> x).apply() }.z",
        "(() -> new Dynamic { z = x }.z).apply()",
    ] {
        let source = format!("local x = 7\nconst function f() = {body}\nresult = f()");
        assert_eq!(
            result(&source).unwrap_err().message,
            "Cannot reference property `x` from here because it is not `const`.",
            "{body}"
        );
    }
    assert_eq!(
        result(
            "local x = 7\nconst function f() = new Dynamic { local function g() = x; z = g() }.z\nresult = f()"
        ),
        Ok(Value::Int(7))
    );
}

#[test]
fn method_scope_does_not_relax_module_or_following_property_boundaries() {
    for source in [
        "x = 7\nconst function f() = module.x\nresult = f()",
        "local x = 7\nconst function f() = x\nconst result = x",
        "local x = 7\nclass C { const function f() = x }\nresult = new C {}.f()",
    ] {
        assert_eq!(
            result(source).unwrap_err().message,
            "Cannot reference property `x` from here because it is not `const`."
        );
    }
}

#[test]
fn outer_uses_the_immediate_lexical_const_scope() {
    for body in ["(n) -> outer.threshold", "(n) -> outer"] {
        let suffix = if body.ends_with("threshold") {
            ""
        } else {
            ".threshold"
        };
        assert_eq!(
            result(&format!(
                "threshold = 0\nconst function f() = {body}\nresult = f().apply(0){suffix}"
            )),
            Ok(Value::Int(0))
        );
    }
    assert_eq!(
        result("threshold = 0\nconst f = (n) -> outer.threshold\nresult = f.apply(0)")
            .unwrap_err()
            .message,
        "Cannot reference property `threshold` from here because it is not `const`."
    );
    let module = parse_module("threshold = 0\nconst f = (n) -> outer").unwrap();
    assert_eq!(
        resolve_module(&module).unwrap_err().message,
        "Cannot reference `outer` from here because it's not `const`."
    );
}
