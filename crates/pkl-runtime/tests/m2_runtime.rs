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
        }),
        environment: Environment {
            locals: vec![Value::Object(parent)],
            scopes: vec![],
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
