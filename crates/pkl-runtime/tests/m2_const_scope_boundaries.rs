//! Let and generator scopes contribute const barriers only inside their bodies.
use pkl_runtime::parser::parse_module;
use pkl_runtime::resolver::resolve_module;
use pkl_runtime::value::{RuntimeError, Value};
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
            VmPoll::Module(_) => panic!("unexpected module request"),
            VmPoll::Pending(_) => panic!("unexpected Host request"),
        }
    }
    panic!("const boundary evaluation did not finish")
}

fn result(body: &str) -> Result<Value, RuntimeError> {
    let source = format!("local threshold = 2\nvalue = 2\nfunction f() = 2\n{body}");
    let module = parse_module(&source).unwrap();
    let program = compile_module(&resolve_module(&module).unwrap()).unwrap();
    let mut vm = Vm::new(program.clone());
    vm.start(program.entry, vec![]).unwrap();
    let module = finish(&mut vm).unwrap().as_object().unwrap();
    vm.start_member(module, "result").unwrap();
    finish(&mut vm)
}

fn non_const(body: &str, name: &str) {
    assert_eq!(
        result(body).unwrap_err().message,
        format!("Cannot reference property `{name}` from here because it is not `const`."),
        "{body}"
    );
}

#[test]
fn let_barriers_begin_after_initializers_and_annotations() {
    for body in [
        "let (n = 0) threshold",
        "let (_ = 0) threshold",
        "let (n = 0) f()",
        "let (n = 0) this.value",
        "let (n = 0) let (m = 0) (() -> threshold).apply()",
    ] {
        assert_eq!(
            result(&format!("const result = {body}")),
            Ok(Value::Int(2.into()))
        );
    }
    for body in [
        "let (n = threshold) n",
        "let (n: Int(this < threshold) = 1) n",
        "let (n = 0) (() -> threshold).apply()",
        "(() -> let (n = 0) threshold).apply()",
    ] {
        non_const(&format!("const result = {body}"), "threshold");
    }
    non_const("const result = let (n = 0) module.value", "value");
}

#[test]
fn for_bodies_add_a_barrier_without_removing_inherited_const_level() {
    for body in [
        "for (n in List(0)) { threshold }",
        "for (n in List(0)) { f() }",
        "for (n in List(0)) { let (m = 0) threshold }",
        "for (n in List(0)) { when (threshold > n) { threshold } }",
        "for (n in List(0)) { for (m in List(threshold)) { m } }",
    ] {
        assert_eq!(
            result(&format!(
                "const xs = new Listing {{ {body} }}\nresult = xs[0]"
            )),
            Ok(Value::Int(2.into())),
            "{body}"
        );
    }
    for body in [
        "threshold",
        "(() -> threshold).apply()",
        "for (n in List(0)) { (() -> threshold).apply() }",
    ] {
        non_const(
            &format!("const xs = new Listing {{ {body} }}\nresult = xs[0]"),
            "threshold",
        );
    }
    non_const(
        "const xs = new Listing { for (n in List(0)) { module.value } }\nresult = xs[0]",
        "value",
    );
}

#[test]
fn eager_generator_inputs_keep_their_own_lookup_depth() {
    for body in [
        "...List(threshold)",
        "for (n in List(threshold)) { n }",
        "when (threshold > 0) { 2 }",
    ] {
        assert_eq!(
            result(&format!(
                "const xs = new Listing {{ {body} }}\nresult = xs[0]"
            )),
            Ok(Value::Int(2.into()))
        );
    }
    for body in [
        "...List((() -> threshold).apply())",
        "for (n in List((() -> threshold).apply())) { n }",
        "when ((() -> threshold > 0).apply()) { 2 }",
    ] {
        non_const(
            &format!("const xs = new Listing {{ {body} }}\nresult = xs[0]"),
            "threshold",
        );
    }
    non_const(
        "const xs = new Mapping { [threshold] = threshold }\nresult = xs[2]",
        "threshold",
    );
}

#[test]
fn finished_body_barriers_do_not_leak_into_siblings_or_module_lookups() {
    non_const(
        "const good = let (n = 0) threshold\nconst result = threshold",
        "threshold",
    );
    non_const(
        "const xs = new Listing { for (n in List(0)) { threshold }; threshold }\nresult = xs[1]",
        "threshold",
    );
    assert_eq!(
        result(
            "class C { local threshold = 3; const x = let (n = 0) threshold }\nresult = new C {}.x"
        ),
        Ok(Value::Int(3.into()))
    );
    non_const(
        "class C { xs = new Listing { for (n in List(0)) { value } } }\nresult = new C {}.xs[0]",
        "value",
    );
}
