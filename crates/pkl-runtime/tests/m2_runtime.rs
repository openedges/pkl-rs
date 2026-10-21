//! Regression expectations are source-semantics checks, not a substitute for
//! the pinned Java raw-byte differential corpus.
use std::sync::Arc;

use pkl_runtime::heap::{Environment, Function, Heap, Object};
use pkl_runtime::parser::parse_module;
use pkl_runtime::resolver::resolve_module;
use pkl_runtime::value::{ErrorKind, RuntimeError, Value};
use pkl_runtime::vm::{Instruction as I, Program, Vm, VmPoll, compile_module};

fn done(vm: &mut Vm) -> Result<Value, RuntimeError> {
    for _ in 0..10_000 {
        match vm.poll(101) {
            VmPoll::Ready(value) => return value,
            VmPoll::Yielded => {}
            VmPoll::Pending(_) => panic!("unexpected Host request"),
        }
    }
    panic!("bounded regression exceeded instruction budget")
}

fn module(source: &str) -> (Vm, Value) {
    let parsed = parse_module(source).unwrap();
    let program = compile_module(&resolve_module(&parsed).unwrap()).unwrap();
    let mut vm = Vm::new(program.clone());
    vm.start(program.entry, vec![]).unwrap();
    let object = done(&mut vm).unwrap();
    (vm, object)
}

fn evaluate(source: &str) -> Result<Value, RuntimeError> {
    let (mut vm, object) = module(source);
    vm.start_member(object.as_object().unwrap(), "result")
        .unwrap();
    done(&mut vm)
}

#[test]
fn recursive_methods_and_argument_shadowing() {
    assert_eq!(
        evaluate(
            "n = 500\nfunction factorial(n: Int): Int = if (n <= 1) 1 else n * factorial(n - 1)\nresult = factorial(10)"
        ),
        Ok(Value::Int(3_628_800))
    );
    assert_eq!(
        evaluate("function choose(a, b, c) = let (b = a + c) b\nresult = choose(3, 90, 4)"),
        Ok(Value::Int(7))
    );
}

#[test]
fn closures_capture_parameters_lets_and_nested_object_scopes() {
    assert_eq!(
        evaluate(
            "function make(x) = (y) -> let (z = 2) (w) -> x + y + z + w\nresult = make(10).apply(20).apply(30)"
        ),
        Ok(Value::Int(62))
    );
    assert_eq!(
        evaluate(
            "base = new { offset = 5; local function make(x) = (y) -> offset + x + y; factory = make(2) }\nchild = (base) { offset = 20 }\nresult = child.factory.apply(3)"
        ),
        Ok(Value::Int(25))
    );
}

#[test]
fn property_and_method_namespaces_are_distinct() {
    assert_eq!(
        evaluate("f = 9\nfunction f(x) = x + 1\nresult = f + f(2)"),
        Ok(Value::Int(12))
    );
    assert_eq!(
        evaluate("function f() = 2\nresult = f").unwrap_err().kind,
        ErrorKind::MissingMember
    );
    assert_eq!(
        evaluate("f = (x) -> x\nresult = f(2)").unwrap_err().kind,
        ErrorKind::MissingMember
    );
    assert_eq!(
        evaluate(
            "base = new { local function apply(x) = x + 1; value = apply(2) }\nresult = base.value"
        ),
        Ok(Value::Int(3))
    );
}

#[test]
fn inherited_method_receiver_and_lexical_owner_are_preserved() {
    assert_eq!(
        evaluate(
            "base = new { local k = 10; x = 1; local function get() = k + x; value = get() }\nchild = (base) { local k = 100; x = 2; value = super.value + k }\nresult = child.value"
        ),
        Ok(Value::Int(112))
    );
}

#[test]
fn branches_and_short_circuits_skip_failing_lazy_members() {
    for source in [
        "result = if (true) 42 else missing",
        "result = if (false) missing else 42",
        "result = if (false && missing) 0 else 42",
        "result = if (true || missing) 42 else 0",
        "result = 42 ?? missing",
        "result = null ?? 42",
    ] {
        assert_eq!(evaluate(source), Ok(Value::Int(42)), "{source}");
    }
    for source in ["result = true && 42", "result = false || 42"] {
        assert_eq!(evaluate(source).unwrap_err().kind, ErrorKind::Type);
    }
}

#[test]
fn arity_checks_follow_eager_argument_evaluation() {
    let arity = evaluate("function f(x) = x\nresult = f(1, 2)").unwrap_err();
    assert_eq!(arity.kind, ErrorKind::Type);
    assert_eq!(arity.message, "Expected 1 function arguments but got 2.");
    assert_eq!(
        evaluate("function f(x) = x\nresult = f(1, missing)")
            .unwrap_err()
            .kind,
        ErrorKind::MissingMember
    );
    assert_eq!(
        evaluate("f = (x) -> x\nresult = f.apply()")
            .unwrap_err()
            .message,
        "Expected 1 function arguments but got 0."
    );
}

#[test]
fn primitive_checks_apply_to_properties_parameters_and_returns() {
    assert_eq!(
        evaluate(
            "function f(x: Int?): Int | String = if (x == null) \"empty\" else x\nresult: String = f(null)"
        ),
        Ok(Value::string("empty"))
    );
    for source in [
        "function f(x: Int) = missing\nresult = f(\"no\")",
        "function f(): Int = \"no\"\nresult = f()",
        "result: Int = \"no\"",
        "result: \"yes\" | \"maybe\" = \"no\"",
    ] {
        assert_eq!(evaluate(source).unwrap_err().kind, ErrorKind::Type);
    }
}

#[test]
fn scalar_operators_preserve_types_and_overflow() {
    for (source, expected) in [
        ("result = 2 ** 10", Value::Int(1024)),
        ("result = -7 ~/ 2", Value::Int(-3)),
        ("result = -7 % 2", Value::Int(-1)),
        ("result = 7.5 ~/ 2", Value::Int(3)),
        ("result = 2 ** -1", Value::Float(0.5)),
        ("result = 2 == 2.0", Value::Bool(true)),
        ("result = 2 != \"2\"", Value::Bool(true)),
        ("result = !false && 2 >= 2 && 3 > 2", Value::Bool(true)),
    ] {
        assert_eq!(evaluate(source), Ok(expected), "{source}");
    }
    for source in ["result = 2 ** 63", "result = 1 ~/ 0"] {
        assert_eq!(evaluate(source).unwrap_err().kind, ErrorKind::Arithmetic);
    }
}

#[test]
fn gc_traces_closure_cycles_and_invalidates_reclaimed_function_handles() {
    let mut heap = Heap::new();
    let parent = heap.allocate(Object::default());
    let closure = heap.allocate(Object {
        function: Some(Function {
            block: 0,
            arity: 0,
            method: false,
            capture_custom_this: false,
        }),
        environment: Environment {
            locals: vec![Value::Object(parent)],
            scopes: vec![],
            ..Environment::default()
        },
        ..Object::default()
    });
    heap.get_mut(parent).unwrap().environment.locals = vec![Value::Function(closure)];
    assert_eq!(heap.collect(&[Value::Function(closure)]).live, 2);
    assert_eq!(heap.collect(&[]).live, 0);
    assert_eq!(heap.get(closure).unwrap_err().kind, ErrorKind::StaleHandle);
}

#[test]
fn suspended_closure_keeps_captures_and_resumes_the_same_call_frame() {
    let (mut vm, object) = module(
        "function make(x) = (suffix) -> let (resource = read(\"mem:test\")) x + resource + suffix\nf = make(\"a\")\nresult = f.apply(\"z\")",
    );
    vm.start_member(object.as_object().unwrap(), "result")
        .unwrap();
    let request = match vm.poll(1000) {
        VmPoll::Pending(request) => request,
        result => panic!("expected suspension, got {result:?}"),
    };
    assert_eq!(request.uri.as_ref(), "mem:test");
    let stats = vm.collect(&[]);
    assert!(stats.live >= 2);
    vm.resume(request.id, Ok(Value::string("body"))).unwrap();
    assert_eq!(done(&mut vm), Ok(Value::string("abodyz")));
}

#[test]
fn compiled_code_rejects_function_handles_and_other_evaluators_reject_inputs() {
    let (mut first, object) = module("result = (x) -> x");
    first
        .start_member(object.as_object().unwrap(), "result")
        .unwrap();
    let closure = done(&mut first).unwrap();
    let mut vm = Vm::new(Arc::new(Program::new(vec![vec![
        I::Constant(closure.clone()),
        I::Return,
    ]])));
    assert_eq!(
        vm.start(0, vec![]).unwrap_err().kind,
        ErrorKind::InvalidBytecode
    );
    let mut vm = Vm::new(Arc::new(Program::new(vec![vec![I::Input(0), I::Return]])));
    assert_eq!(
        vm.start(0, vec![closure]).unwrap_err().kind,
        ErrorKind::StaleHandle
    );
}

#[test]
fn nested_host_contexts_share_closure_captures_and_survive_collection() {
    use pkl_runtime::host::{HostPoll, HostSession};
    let parsed = parse_module(
        "data = new { prefix = \"a\" }\nfunction make(x) = (suffix) -> let (resource = read(\"mem:test\")) x.prefix + resource + suffix\nf = make(data)\nresult = f.apply(\"z\")\nnested = f.apply(\"q\")",
    ).unwrap();
    let mut program = (*compile_module(&resolve_module(&parsed).unwrap()).unwrap()).clone();
    let result_entry = program.blocks.len();
    program.blocks.push(vec![
        I::Input(0),
        I::GetMember(Arc::from("result")),
        I::Return,
    ]);
    let nested_entry = program.blocks.len();
    program.blocks.push(vec![
        I::Input(0),
        I::GetMember(Arc::from("nested")),
        I::Return,
    ]);
    let entry = program.entry;
    let mut session = HostSession::new(Arc::new(program));
    session.start(entry, vec![], 0, None).unwrap();
    let object = match session.poll(0, 1000) {
        HostPoll::Ready(Ok(value)) => value,
        result => panic!("expected module object, got {result:?}"),
    };
    session
        .start(result_entry, vec![object.clone()], 0, None)
        .unwrap();
    let outer = match session.poll(0, 1000) {
        HostPoll::Read { token, .. } => token,
        result => panic!("expected outer read, got {result:?}"),
    };
    session
        .reenter(&outer, nested_entry, vec![object], 0, None)
        .unwrap();
    let inner = match session.poll(0, 1000) {
        HostPoll::Read { token, .. } => token,
        result => panic!("expected inner read, got {result:?}"),
    };
    assert_eq!(session.context_depth(), 2);
    assert!(session.collect(&[]).live >= 3);
    session
        .resume(&inner, Ok(Value::string("inner")), 0)
        .unwrap();
    assert_eq!(
        session.poll(0, 1000),
        HostPoll::Ready(Ok(Value::string("ainnerq")))
    );
    assert_eq!(session.context_depth(), 1);
    session.collect(&[]);
    session
        .resume(&outer, Ok(Value::string("outer")), 0)
        .unwrap();
    assert_eq!(
        session.poll(0, 1000),
        HostPoll::Ready(Ok(Value::string("aouterz")))
    );
    session.close();
    assert_eq!(session.heap_stats().live, 0);
}

#[test]
fn interpolation_raw_strings_multiline_and_utf16_string_order() {
    assert_eq!(
        evaluate(r#"result = "prefix \(1 + 2) \(true) \(null) \(1.5)""#),
        Ok(Value::string("prefix 3 true null 1.5"))
    );
    assert_eq!(
        evaluate("result = #\"literal \\n \\#(2 + 3)\"#"),
        Ok(Value::string("literal \\n 5"))
    );
    assert_eq!(
        evaluate("result = \"\"\"\n  first\n  second\n  \"\"\""),
        Ok(Value::string("first\nsecond"))
    );
    // Java compares UTF-16 units: a supplementary high surrogate precedes U+E000.
    assert_eq!(
        evaluate("result = \"🙂\" < \"\u{e000}\""),
        Ok(Value::Bool(true))
    );
}

#[test]
fn nullable_calls_skip_arguments_and_outer_scope_follows_lambda_owner() {
    assert_eq!(
        evaluate("result = null?.missing(missingArgument) ?? 42"),
        Ok(Value::Int(42))
    );
    assert_eq!(evaluate("result = null?.missing ?? 42"), Ok(Value::Int(42)));
    assert_eq!(
        evaluate(
            "x = 2\nobj = new { x = 3; f = () -> outer.x; result = f.apply() + outer.x + module.x }\nresult = obj.result"
        ),
        Ok(Value::Int(7))
    );
    assert_eq!(
        evaluate("result = (3 as Number) is Int"),
        Ok(Value::Bool(true))
    );
    assert_eq!(evaluate("result = null is Int?"), Ok(Value::Bool(true)));
    assert_eq!(evaluate("result = 2!!"), Ok(Value::Int(2)));
    let error = evaluate("result = null!!").unwrap_err();
    assert_eq!(error.message, "Expected a non-null value, but got `null`.");
    assert_eq!(
        evaluate("result = throw(\"message\")").unwrap_err().message,
        "message"
    );
}

#[test]
fn type_errors_keep_declared_type_value_and_caller_frames() {
    let error = evaluate("value: Int = \"wrong\"\nresult = value").unwrap_err();
    assert_eq!(
        error.message,
        "Expected value of type `Int`, but got type `String`.\nValue: \"wrong\""
    );
    assert_eq!(error.frames.len(), 3);
    assert_eq!(error.frames[0].span, pkl_runtime::source::Span::new(7, 10));
    assert_eq!(error.frames[1].span, pkl_runtime::source::Span::new(13, 20));
    assert_eq!(error.frames[2].span, pkl_runtime::source::Span::new(30, 35));
}

#[test]
fn invalid_operator_diagnostics_keep_operand_types_and_values() {
    assert_eq!(
        evaluate("result = !1").unwrap_err().message,
        "Operator `!` is not defined for operand type `Int`.\nOperand: 1"
    );
    assert_eq!(
        evaluate("result = 1 + true").unwrap_err().message,
        "Operator `+` is not defined for operand types `Int` and `Boolean`.\nLeft operand : 1\nRight operand: true"
    );
}

#[test]
fn logical_operator_errors_preserve_operator_and_skip_unneeded_operands() {
    for (source, operator) in [
        ("result = 1 && missing", "&&"),
        ("result = 1 || missing", "||"),
    ] {
        assert_eq!(
            evaluate(source).unwrap_err().message,
            format!(
                "Operator `{operator}` is not defined for left operand type `Int`.\nLeft operand: 1"
            )
        );
    }
    assert_eq!(
        evaluate("result = true && 1").unwrap_err().message,
        "Operator `&&` is not defined for operand types `Boolean` and `Int`.\nLeft operand : true\nRight operand: 1"
    );
    assert_eq!(
        evaluate("result = false || 1").unwrap_err().message,
        "Operator `||` is not defined for operand types `Boolean` and `Int`.\nLeft operand : false\nRight operand: 1"
    );
}

#[test]
fn power_and_truncating_division_follow_java_special_value_rules() {
    for source in [
        "result = 1.0 ** (1.0 / 0.0)",
        "result = (-1.0) ** (1.0 / 0.0)",
        "result = 1.0 ** (0.0 / 0.0)",
    ] {
        assert!(matches!(evaluate(source), Ok(Value::Float(value)) if value.is_nan()));
    }
    assert_eq!(
        evaluate("result = (0.0 / 0.0) ** 0.0"),
        Ok(Value::Float(1.0))
    );
    assert_eq!(
        evaluate("result = 1.0 ~/ 0.0").unwrap_err().message,
        "Cannot convert Float `1.0` to Int because it is too large."
    );
    assert_eq!(
        evaluate("result = (1.0 / 0.0) ~/ 2.0").unwrap_err().message,
        "Cannot convert non-finite Float `Infinity` to Int."
    );
}

#[test]
fn class_methods_preserve_subclass_receiver_and_super_owner() {
    assert_eq!(
        evaluate(
            "open class A { x: Int = 1; function f() = x + 2 }\nclass B extends A { x = 3; function f() = super.f() + x }\nb = new B {}\nresult = b.f()"
        ),
        Ok(Value::Int(8))
    );
    assert_eq!(
        evaluate(
            "class C { x: Int = 2; function make(y) = (z) -> x + y + z }\nc = new C { x = 5 }\nresult = c.make(2).apply(3)"
        ),
        Ok(Value::Int(10))
    );
}

#[test]
fn class_type_checks_follow_ancestry_and_inherited_property_annotations() {
    assert_eq!(
        evaluate("open class A {}\nclass B extends A {}\nx = new B {}\nresult = x is A"),
        Ok(Value::Bool(true))
    );
    assert_eq!(
        evaluate("class C { x: Int = 2 }\nc = new C { x = \"wrong\" }\nresult = c.x")
            .unwrap_err()
            .kind,
        ErrorKind::Type
    );
    assert_eq!(evaluate("open class A { x: Int = 2 }\nclass B extends A { x = \"wrong\" }\nb = new B {}\nresult = b.x").unwrap_err().kind, ErrorKind::Type);
}

#[test]
fn type_aliases_resolve_lexically_and_contextual_new_uses_declared_class() {
    assert_eq!(
        evaluate("typealias Num = Int\nresult: Num = 42"),
        Ok(Value::Int(42))
    );
    assert_eq!(
        evaluate("class C { x = 42 }\ntypealias Alias = C\nc: Alias = new {}\nresult = c.x"),
        Ok(Value::Int(42))
    );
    for (source, kind, message) in [
        (
            "local Int = 5\nresult: Int = 1",
            ErrorKind::Type,
            "Expected `Int` to be a type, but it is not.",
        ),
        (
            "typealias A = B\ntypealias B = A\nresult: A = 1",
            ErrorKind::Cycle,
            "Type alias definitions must not be cyclic.",
        ),
    ] {
        let parsed = parse_module(source).unwrap();
        let program = compile_module(&resolve_module(&parsed).unwrap()).unwrap();
        let mut vm = Vm::new(program.clone());
        vm.start(program.entry, vec![]).unwrap();
        let error = done(&mut vm).unwrap_err();
        assert_eq!(error.kind, kind);
        assert_eq!(error.message, message);
    }
}

#[test]
fn undefined_typed_properties_are_lazy_and_supported_defaults_are_values() {
    assert_eq!(
        evaluate("class C { missing: Int }\nresult = 42"),
        Ok(Value::Int(42))
    );
    assert_eq!(
        evaluate("class C { x: Int }\nc = new C { x = 42 }\nresult = c.x"),
        Ok(Value::Int(42))
    );
    assert_eq!(
        evaluate("class C { x = 42 }\nc: C\nresult = c.x"),
        Ok(Value::Int(42))
    );
    assert_eq!(evaluate("x: Int?\nresult = x"), Ok(Value::Null));
    assert_eq!(
        evaluate("x: \"default\"\nresult = x"),
        Ok(Value::string("default"))
    );
    assert_eq!(
        evaluate("class C { missing: Int }\nc = new C {}\nresult = c.missing")
            .unwrap_err()
            .kind,
        ErrorKind::MissingMember
    );
}

#[test]
fn generic_alias_arguments_substitute_at_use_site_and_defaults_survive_expansion() {
    assert_eq!(
        evaluate("typealias Optional<T> = T?\nvalue: Optional<Int>\nresult = value"),
        Ok(Value::Null)
    );
    assert_eq!(
        evaluate("typealias Either<A, B> = A | B\nresult: Either<Int, String> = \"ok\""),
        Ok(Value::string("ok"))
    );
    assert_eq!(
        evaluate(
            "typealias Identity<T> = T\nclass C { x = 42 }\nc: Identity<C> = new Identity<C> {}\nresult = c.x"
        ),
        Ok(Value::Int(42))
    );
    assert_eq!(
        evaluate("typealias Identity<T> = T\nresult: Identity<Identity<Int>> = 42"),
        Ok(Value::Int(42))
    );
    assert_eq!(
        evaluate("typealias Choice<T> = *\"default\" | T\nvalue: Choice<Int>\nresult = value"),
        Ok(Value::string("default"))
    );
    assert_eq!(
        evaluate(
            "class C { x = 42 }\nclass D { C = 2; value: C = new C {} }\nd = new D {}\nresult = d.value.x"
        ),
        Ok(Value::Int(42))
    );
    assert_eq!(
        evaluate("function f(Int) = let (x: Int = 42) x\nresult = f(\"not a type\")"),
        Ok(Value::Int(42))
    );
}

#[test]
fn unused_invalid_type_declarations_do_not_force_their_definitions() {
    for source in [
        "typealias Unused = Missing\nresult = 42",
        "typealias A = B\ntypealias B = A\nresult = 42",
        "class A {}\nclass B extends A {}\nresult = 42",
        "class B extends Missing {}\nresult = 42",
    ] {
        assert_eq!(evaluate(source), Ok(Value::Int(42)));
    }
}

#[test]
fn const_barriers_are_checked_on_reads_and_keep_unused_class_members_lazy() {
    assert_eq!(
        evaluate("x = 1\nclass C { value = x }\nresult = 42"),
        Ok(Value::Int(42))
    );
    let error =
        evaluate("x = 1\nclass C { value = x }\nc = new C {}\nresult = c.value").unwrap_err();
    assert!(
        error
            .message
            .starts_with("Cannot reference property `x` from here because it is not `const`.")
    );
    assert_eq!(
        evaluate("const x = 2\nclass C { value = x }\nc = new C {}\nresult = c.value"),
        Ok(Value::Int(2))
    );
    assert_eq!(
        evaluate(
            "const function f(x) = x + 1\nclass C { function g() = f(2) }\nc = new C {}\nresult = c.g()"
        ),
        Ok(Value::Int(3))
    );
    let error = evaluate("class C { const x = 1 }\nc = new C { x = 2 }\nresult = c.x").unwrap_err();
    assert_eq!(error.message, "Cannot assign to const property `x`.");
}

#[test]
fn class_handles_and_prototype_captures_are_traced_and_reject_foreign_heaps() {
    let parsed = parse_module("class C { value = 42 }\nresult = C").unwrap();
    let mut program = (*compile_module(&resolve_module(&parsed).unwrap()).unwrap()).clone();
    let entry = program.entry;
    let instantiate = program.blocks.len();
    program
        .blocks
        .push(vec![I::Input(0), I::DefaultClassInstance, I::Return]);
    let program = Arc::new(program);
    let mut vm = Vm::new(program.clone());
    vm.start(entry, vec![]).unwrap();
    let module = done(&mut vm).unwrap();
    vm.start_member(module.as_object().unwrap(), "result")
        .unwrap();
    let class = done(&mut vm).unwrap();
    assert!(matches!(class, Value::Class(_)));
    assert_eq!(vm.collect(std::slice::from_ref(&class)).live, 2);
    vm.start(instantiate, vec![class.clone()]).unwrap();
    let instance = done(&mut vm).unwrap();
    vm.start_member(instance.as_object().unwrap(), "value")
        .unwrap();
    assert_eq!(done(&mut vm), Ok(Value::Int(42)));
    let mut other = Vm::new(program);
    assert_eq!(
        other.start(instantiate, vec![class]).unwrap_err().kind,
        ErrorKind::StaleHandle
    );
}

#[test]
fn inherited_and_alias_type_failures_retain_definition_and_value_sources() {
    let source = "open class A { x: Int = 2 }\nclass B extends A { x = \"wrong\" }\nb = new B {}\nresult = b.x";
    let error = evaluate(source).unwrap_err();
    assert_eq!(
        error
            .frames
            .iter()
            .map(|frame| frame.member.as_ref())
            .collect::<Vec<_>>(),
        ["#A.x", "#B.x", "#result"]
    );
    assert_eq!(
        &source[error.frames[0].span.start..error.frames[0].span.end],
        "Int"
    );
    assert_eq!(
        &source[error.frames[1].span.start..error.frames[1].span.end],
        "\"wrong\""
    );
    let source = "typealias Num = Int\nvalue: Num = \"wrong\"\nresult = value";
    let error = evaluate(source).unwrap_err();
    assert_eq!(
        &source[error.frames[0].span.start..error.frames[0].span.end],
        "Int"
    );
    assert_eq!(
        evaluate("result = let (x: Int = \"wrong\") 42")
            .unwrap_err()
            .frames
            .len(),
        1
    );
}

#[test]
fn class_type_initialization_is_eager_for_declarations_and_lazy_for_expressions() {
    use pkl_runtime::evaluation::scalar_expression;
    let prefix = "class A {}\nclass B extends A {}\n";
    for body in [
        "unused: B = 1\nresult = 42",
        "function f(x: B) = x\nresult = 42",
        "result = 1 is B",
        "result = throw(\"value evaluated first\") as B",
    ] {
        let error = scalar_expression(&format!("{prefix}{body}"), "repl:types", "types", "result")
            .unwrap_err();
        assert!(
            error.contains("Cannot extend non-open class `types#A`."),
            "{error}"
        );
    }
    for body in [
        "result = if (false) 1 is B else true",
        "result = if (false) 1 as B else true",
    ] {
        assert_eq!(
            scalar_expression(&format!("{prefix}{body}"), "repl:types", "types", "result"),
            Ok("true".into())
        );
    }
    assert_eq!(
        evaluate("class A { b: B? }\nclass B { a: A? }\na = new A {}\nresult = a.b"),
        Ok(Value::Null)
    );
    assert_eq!(
        evaluate("result = if (false) 1 is Missing else true"),
        Ok(Value::Bool(true))
    );
    assert_eq!(
        evaluate("result = if (false) let (x: Missing = 1) x else 42"),
        Ok(Value::Int(42))
    );
}

#[test]
fn super_property_skips_its_annotation_until_the_overriding_value_is_checked() {
    let source = "open class A { x: Int = \"bad\" }\nclass B extends A { x = super.x + \"!\" }\nb = new B {}\nresult = b.x";
    let error = evaluate(source).unwrap_err();
    assert!(error.message.ends_with("Value: \"bad!\""), "{error}");
    assert_eq!(error.frames[0].member.as_ref(), "#A.x");
    assert_eq!(error.frames[1].member.as_ref(), "#B.x");
}

#[test]
fn unknown_typed_object_properties_report_headers_and_visible_property_candidates() {
    let source = "module sample\nopen class A { x = 1 }\nclass B extends A { z = 2; local extraPrivate = 3; function extraMethod() = 4 }\nb = new B { extra = 5 }\nresult = b.x";
    let error = evaluate(source).unwrap_err();
    assert_eq!(
        error.message,
        "Cannot find property `extra` in object of type `sample#B`."
    );
    assert_eq!(error.hint.as_deref(), Some("Available properties:\nx\nz"));
    assert_eq!(
        &source[error.frames[0].span.start..error.frames[0].span.end],
        "extra"
    );
    assert_eq!(error.frames[0].member.as_ref(), "#b");

    let error = evaluate("class C { answer = 42 }\nc = new C { answr = 2 }\nresult = c.answer")
        .unwrap_err();
    assert_eq!(
        error.hint.as_deref(),
        Some("Did you mean any of the following?\nanswer")
    );
}

#[test]
fn abstract_members_fail_lazily_before_defaults_and_keep_qualified_identity() {
    for declaration in [
        "abstract value: Int?",
        "abstract value: \"default\"",
        "abstract function value(): Int",
    ] {
        let prefix = format!(
            "module example.named\nabstract class Base {{ {declaration} }}\nclass Child extends Base {{}}\n"
        );
        assert_eq!(
            evaluate(&format!("{prefix}result = 42")),
            Ok(Value::Int(42))
        );
        let call = if declaration.contains("function") {
            "()"
        } else {
            ""
        };
        let error =
            evaluate(&format!("{prefix}result = (new Child {{}}).value{call}")).unwrap_err();
        let kind = if call.is_empty() {
            "property"
        } else {
            "method"
        };
        assert_eq!(
            error.message,
            format!("Cannot invoke abstract {kind} `example.named#Base.value`.")
        );
        assert_eq!(error.frames[0].member.as_ref(), "#Base.value");
    }
    let error = evaluate("module example.named\nclass C {}\nresult: C = 1").unwrap_err();
    assert!(
        error
            .message
            .starts_with("Expected value of type `example.named#C`,")
    );
}

#[test]
fn repeated_modifier_flags_preserve_upstream_addition_semantics() {
    let error = evaluate("const const x = 42\nclass C { value = x }\nresult = (new C {}).value")
        .unwrap_err();
    assert_eq!(
        error.message,
        "Cannot reference property `x` from here because it is not `const`."
    );
    assert_eq!(
        evaluate("open open class A { x = 42 }\nresult = (new A {}).x"),
        Ok(Value::Int(42))
    );
    let error =
        evaluate("open open class A { x = 42 }\nclass B extends A {}\nresult = (new B {}).x")
            .unwrap_err();
    assert!(
        error.message.starts_with("Cannot extend non-open class"),
        "{error}"
    );
}

#[test]
fn pipeline_preserves_precedence_capture_and_left_to_right_failure_order() {
    assert_eq!(
        evaluate("result = 1 + 2 |> (x) -> x * 3"),
        Ok(Value::Int(9))
    );
    assert_eq!(
        evaluate("result = let (add = 4) 2 |> (x) -> x + add"),
        Ok(Value::Int(6))
    );
    assert_eq!(
        evaluate("result = 1 |> ((x) -> x + 2) |> ((x) -> x * 3)"),
        Ok(Value::Int(9))
    );
    assert_eq!(
        evaluate("result = 42 ?? missing |> missingToo"),
        Ok(Value::Int(42))
    );
    assert_eq!(
        evaluate("result = throw(\"left\") |> throw(\"right\")")
            .unwrap_err()
            .message,
        "left"
    );
    assert!(
        evaluate("result = 1 |> 2")
            .unwrap_err()
            .message
            .starts_with("Operator `|>`")
    );
    assert!(
        evaluate("result = 1 |> ((x, y) -> x)")
            .unwrap_err()
            .message
            .starts_with("Expected 2 function arguments but got 1")
    );
}

#[test]
fn constraints_execute_in_order_with_custom_receivers_aliases_and_recoverable_unions() {
    for source in [
        "result: Int(this > 0) = 42",
        "result: Int(this > 0, this < 100) = 42",
        "result: Int((x) -> x > 0) = 42",
        "result: Int(false) | Int = 42",
        "typealias Positive = Int(this > 0)\nresult: Positive = 42",
        "typealias Same<T> = T(this is T)\nresult: Same<Int> = 42",
        "class C { threshold = 100; value: Int(this < threshold) = 42 }\nresult = (new C {}).value",
        "result = let (x: Int(this > 0) = 42) x",
        "function f(x: Int(this > 0)): Int(this < 100) = x\nresult = f(42)",
    ] {
        assert_eq!(evaluate(source), Ok(Value::Int(42)), "{source}");
    }
    assert_eq!(
        evaluate("result = 42 is Int(false)"),
        Ok(Value::Bool(false))
    );
    assert!(evaluate("result = 42 is Int(1)").is_err());
    assert!(evaluate("result: Int(1) | Int = 42").is_err());
    let error = evaluate("result: Int(false, throw(\"later\")) = 42").unwrap_err();
    assert!(error.type_diagnostic.is_some());
    assert_ne!(error.message, "later");
}

#[test]
fn constraint_receivers_preserve_lambda_outer_and_declaration_owner() {
    for source in [
        "value: Int((ignored) -> ((q) -> this == 5 && outer == 5).apply(0)) = 5\nresult = value",
        "marker=42\nvalue: Int((new Dynamic {marker=7; answer=outer.marker==42 && this.marker==7}).answer)=5\nresult=value",
        "value: Int((ignored) -> (new Dynamic {marker=7; answer=outer==5 && this.marker==7}).answer)=5\nresult=value",
        "function check(other:Int,value:Int(this>other)):Int(this>other)=value\nresult=check(3,5)",
        "x=3\nresult=let(x:Int(this>x)=5) x",
        "class C {value=5}\nv:C((ignored)->value==5)=new C{}\nresult=v.value",
        "value=99\nclass C{value=5}\nv:C(value==99 && this.value==5)=new C{}\nresult=v.value",
    ] {
        assert_eq!(evaluate(source), Ok(Value::Int(5)), "{source}");
    }
    for source in [
        "open class Base{local limit=4; value:Int(this>limit)=5}\nclass C extends Base{value=6}\nresult=(new C{}).value",
        "class C{local limit=4; value:Int(this>limit)=5}\nresult=(new C{value=6}).value",
    ] {
        assert_eq!(evaluate(source), Ok(Value::Int(6)), "{source}");
    }
    assert_eq!(evaluate("typealias Identity<T> = T\nclass C{limit=4;value:Identity<Int(this>limit)> = 5}\nresult=(new C{}).value").unwrap_err().kind, ErrorKind::MissingMember);
    assert_eq!(
        evaluate(
            "typealias Identity<T> = T\nfunction f(limit:Int,value:Identity<Int(this>limit)>)=value\nresult=f(4,5)"
        ),
        Ok(Value::Int(5))
    );
}

#[test]
fn constraint_host_suspension_cancellation_failure_and_retry_preserve_lazy_state() {
    let (mut vm, object) = module(
        "data=new {prefix=\"ok\"}\nfunction predicate(captured)=(value)->read(\"mem:predicate\")==captured.prefix && value==5\nresult:Int(predicate(data))=5",
    );
    let object_id = object.as_object().unwrap();
    vm.start_member(object_id, "result").unwrap();
    let request = match vm.poll(1000) {
        VmPoll::Pending(request) => request,
        other => panic!("{other:?}"),
    };
    assert!(vm.collect(&[]).live >= 3);
    vm.cancel();
    assert_eq!(done(&mut vm).unwrap_err().kind, ErrorKind::Cancelled);
    assert!(vm.resume(request.id, Ok(Value::string("ok"))).is_err());
    vm.start_member(object_id, "result").unwrap();
    let failed = match vm.poll(1000) {
        VmPoll::Pending(request) => request,
        other => panic!("{other:?}"),
    };
    vm.resume(
        failed.id,
        Err(RuntimeError::new(ErrorKind::Host, "reader failed")),
    )
    .unwrap();
    assert_eq!(done(&mut vm).unwrap_err().kind, ErrorKind::Host);
    vm.collect(std::slice::from_ref(&object));
    vm.start_member(object_id, "result").unwrap();
    let retry = match vm.poll(1000) {
        VmPoll::Pending(request) => request,
        other => panic!("{other:?}"),
    };
    vm.collect(&[]);
    vm.resume(retry.id, Ok(Value::string("ok"))).unwrap();
    assert_eq!(done(&mut vm), Ok(Value::Int(5)));
    // A completed check is cached; forcing again cannot issue another read.
    vm.start_member(object_id, "result").unwrap();
    assert_eq!(done(&mut vm), Ok(Value::Int(5)));
    assert_eq!(
        evaluate("result:Int(false,read(\"mem:must-not-read\")==\"ok\")=5")
            .unwrap_err()
            .kind,
        ErrorKind::Type
    );
}

#[test]
fn constraint_host_reentry_roots_both_continuations() {
    use pkl_runtime::host::{HostPoll, HostSession};
    let parsed=parse_module("class C {value=5}\nresult:C(read(\"mem:outer\")==\"ok\" && this.value==5)=new C{}\nnested:Int((v)->read(\"mem:inner\")==\"ok\" && v==7)=7").unwrap();
    let mut program = (*compile_module(&resolve_module(&parsed).unwrap()).unwrap()).clone();
    let root_entry = program.entry;
    let outer_entry = program.blocks.len();
    program.blocks.push(vec![
        I::Input(0),
        I::GetMember(Arc::from("result")),
        I::GetMember(Arc::from("value")),
        I::Return,
    ]);
    let inner_entry = program.blocks.len();
    program.blocks.push(vec![
        I::Input(0),
        I::GetMember(Arc::from("nested")),
        I::Return,
    ]);
    let mut session = HostSession::new(Arc::new(program));
    session.start(root_entry, vec![], 0, None).unwrap();
    let object = match session.poll(0, 1000) {
        HostPoll::Ready(Ok(value)) => value,
        other => panic!("{other:?}"),
    };
    session
        .start(outer_entry, vec![object.clone()], 0, None)
        .unwrap();
    let outer = match session.poll(0, 1000) {
        HostPoll::Read { token, uri } => {
            assert_eq!(&*uri, "mem:outer");
            token
        }
        other => panic!("{other:?}"),
    };
    session
        .reenter(&outer, inner_entry, vec![object], 0, None)
        .unwrap();
    let inner = match session.poll(0, 1000) {
        HostPoll::Read { token, uri } => {
            assert_eq!(&*uri, "mem:inner");
            token
        }
        other => panic!("{other:?}"),
    };
    assert_eq!(session.context_depth(), 2);
    // Empty typed construction may return the class prototype. Successful
    // reads after collection prove retention without assuming an allocation.
    session.collect(&[]);
    session.resume(&inner, Ok(Value::string("ok")), 0).unwrap();
    assert_eq!(session.poll(0, 1000), HostPoll::Ready(Ok(Value::Int(7))));
    session.collect(&[]);
    session.resume(&outer, Ok(Value::string("ok")), 0).unwrap();
    assert_eq!(session.poll(0, 1000), HostPoll::Ready(Ok(Value::Int(5))));
    session.close();
    assert_eq!(session.heap_stats().live, 0);
}

#[test]
fn power_constraint_replay_resumes_and_gc_traces_observed_values() {
    use pkl_runtime::vm::compile_module_with_power_assertions;
    let parsed=parse_module("class C{value=5}\nresult:C(let(seen=this) read(\"mem:check\")==\"ok\" && seen.value>10)=new C{}").unwrap();
    let program = compile_module_with_power_assertions(&resolve_module(&parsed).unwrap()).unwrap();
    let mut vm = Vm::new(program.clone());
    vm.set_power_assertions(true);
    vm.start(program.entry, vec![]).unwrap();
    let object = done(&mut vm).unwrap();
    vm.start_member(object.as_object().unwrap(), "result")
        .unwrap();
    for _ in 0..2 {
        let request = match vm.poll(1000) {
            VmPoll::Pending(request) => request,
            other => panic!("{other:?}"),
        };
        assert_eq!(&*request.uri, "mem:check");
        // The retained receiver may be its class prototype; the observations
        // below must still render that receiver after each collection.
        vm.collect(&[]);
        vm.resume(request.id, Ok(Value::string("ok"))).unwrap();
    }
    let error = done(&mut vm).unwrap_err();
    let Some(diagnostic) = error.type_diagnostic else {
        panic!("missing diagnostic")
    };
    let pkl_runtime::diagnostic::TypeDiagnostic::Constraint {
        observations: Some(observations),
        ..
    } = *diagnostic
    else {
        panic!("missing trace")
    };
    assert!(
        observations
            .iter()
            .any(|item| item.value == "new C { value = 5 }")
    );
    assert!(observations.iter().any(|item| item.value == "false"));
}

#[test]
fn enabled_union_replays_only_after_all_alternatives_fail_and_type_tests_do_not_replay() {
    use pkl_runtime::vm::compile_module_with_power_assertions;
    for test in [false, true] {
        let source = if test {
            "result=5 is (Int(read(\"mem:a\")==\"pass\") | Int(read(\"mem:b\")==\"pass\"))"
        } else {
            "result:Int(read(\"mem:a\")==\"pass\") | Int(read(\"mem:b\")==\"pass\")=5"
        };
        let parsed = parse_module(source).unwrap();
        let program =
            compile_module_with_power_assertions(&resolve_module(&parsed).unwrap()).unwrap();
        let mut vm = Vm::new(program.clone());
        vm.set_power_assertions(true);
        vm.start(program.entry, vec![]).unwrap();
        let object = done(&mut vm).unwrap();
        vm.start_member(object.as_object().unwrap(), "result")
            .unwrap();
        let mut requests = Vec::new();
        let outcome = loop {
            match vm.poll(1000) {
                VmPoll::Yielded => {}
                VmPoll::Ready(result) => break result,
                VmPoll::Pending(request) => {
                    requests.push(request.uri.to_string());
                    vm.collect(&[]);
                    vm.resume(request.id, Ok(Value::string("fail"))).unwrap();
                }
            }
        };
        if test {
            assert_eq!(requests, ["mem:a", "mem:b"]);
            assert_eq!(outcome, Ok(Value::Bool(false)));
        } else {
            assert_eq!(
                requests,
                ["mem:a", "mem:b", "mem:a", "mem:a", "mem:b", "mem:b"]
            );
            assert_eq!(outcome.unwrap_err().kind, ErrorKind::Type);
        }
    }
}

#[test]
fn inherited_constraint_fatal_errors_keep_declaration_and_override_value_frames() {
    let source =
        "open class A{value:Int(1 ~/ 0 > 0)=5}\nclass B extends A{value=6}\nresult=(new B{}).value";
    let error = evaluate(source).unwrap_err();
    assert_eq!(error.kind, ErrorKind::Arithmetic);
    assert_eq!(error.frames[0].member.as_ref(), "#A.value");
    assert_eq!(
        &source[error.frames[0].span.start..error.frames[0].span.end],
        "1 ~/ 0"
    );
    assert_eq!(error.frames[1].member.as_ref(), "#B.value");
    assert_eq!(
        &source[error.frames[1].span.start..error.frames[1].span.end],
        "6"
    );
    assert_eq!(error.frames[2].member.as_ref(), "#result");
    assert_eq!(
        evaluate("result=1 |> new Dynamic{}").unwrap_err().message,
        "Operator `|>` is not defined for operand types `Int` and `Dynamic`.\nLeft operand : 1\nRight operand: new Dynamic {}"
    );
    let error = evaluate("class C{x=missing}\nresult=1 |> new C{}").unwrap_err();
    assert_eq!(
        error.message,
        "Operator `|>` is not defined for operand types `Int` and `#C`.\nLeft operand : 1\nRight operand: new C { x = ? }"
    );
}

#[test]
fn inherited_invalid_constraint_results_preserve_fatal_validation_frames() {
    for power in [false, true] {
        for (constraint, authored) in [
            ("42", Some("42")),
            ("null", Some("null")),
            ("(x) -> 42", None),
        ] {
            let source = format!(
                "open class A {{ value: Int({constraint}) = 5 }}\nclass B extends A {{ value = 6 }}\nresult = (new B {{}}).value"
            );
            let parsed = parse_module(&source).unwrap();
            let resolved = resolve_module(&parsed).unwrap();
            let program = if power {
                pkl_runtime::vm::compile_module_with_power_assertions(&resolved)
            } else {
                compile_module(&resolved)
            }
            .unwrap();
            let mut vm = Vm::new(program.clone());
            vm.set_power_assertions(power);
            vm.start(program.entry, vec![]).unwrap();
            let object = done(&mut vm).unwrap();
            vm.start_member(object.as_object().unwrap(), "result")
                .unwrap();
            let error = done(&mut vm).unwrap_err();
            assert_eq!(error.kind, ErrorKind::Type);
            assert!(
                error.type_diagnostic.is_none(),
                "fatal errors must not become recoverable mismatches"
            );
            let actual: Vec<_> = error
                .frames
                .iter()
                .map(|frame| {
                    (
                        frame.member.as_ref(),
                        &source[frame.span.start..frame.span.end],
                    )
                })
                .collect();
            let mut expected = Vec::new();
            if let Some(expression) = authored {
                expected.push(("#A.value", expression));
            }
            expected.extend([("#B.value", "6"), ("#result", "(new B {}).value")]);
            assert_eq!(actual, expected, "constraint={constraint}, power={power}");
            // Failed validation must not publish a cached property value.
            vm.start_member(object.as_object().unwrap(), "result")
                .unwrap();
            assert_eq!(done(&mut vm), Err(error));
        }
    }
}
