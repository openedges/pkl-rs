//! Method defaults and native lifetime checks; Java raw comparisons live in the corpus.
use pkl_runtime::heap::LazyState;
use pkl_runtime::parser::parse_module;
use pkl_runtime::resolver::resolve_module;
use pkl_runtime::value::{ErrorKind, RuntimeError, Value};
use pkl_runtime::vm::{Vm, VmPoll, compile_module};

fn event(vm: &mut Vm) -> VmPoll {
    for _ in 0..100_000 {
        match vm.poll(1) {
            VmPoll::Yielded => {
                vm.collect(&[]);
            }
            event => return event,
        }
    }
    panic!("method default did not settle")
}

fn ready(vm: &mut Vm) -> Result<Value, RuntimeError> {
    match event(vm) {
        VmPoll::Ready(result) => result,
        other => panic!("unexpected Host request: {other:?}"),
    }
}

fn module(source: &str) -> (Vm, Value) {
    let program = compile_module(&resolve_module(&parse_module(source).unwrap()).unwrap()).unwrap();
    let mut vm = Vm::new(program.clone());
    vm.start(program.entry, vec![]).unwrap();
    let module = ready(&mut vm).unwrap();
    (vm, module)
}

fn evaluate(source: &str) -> Result<Value, RuntimeError> {
    let (mut vm, module) = module(source);
    vm.start_member(module.as_object().unwrap(), "result")
        .unwrap();
    ready(&mut vm)
}

#[test]
fn method_defaults_follow_return_type_aliases_nullable_unions_and_body_paths() {
    for (source, expected) in [
        (
            "class C { x: Int = 7 }\nfunction make(): C = new {}\nresult = make().x",
            7,
        ),
        (
            "class C { x: Int = 7 }\ntypealias A = C\nfunction make(): A = new { x = 9 }\nresult = make().x",
            9,
        ),
        (
            "class C { x: Int = 7 }\nfunction make(): C? = new {}\nresult = make().x",
            7,
        ),
        (
            "class C { x: Int = 7 }\nfunction make(): *C | Int = new {}\nresult = make().x",
            7,
        ),
        (
            "class C { x: Int = 7 }\nfunction make(flag): C = if (flag) let (n = 9) new { x = n } else new {}\nresult = make(true).x + make(false).x",
            16,
        ),
        (
            "class C { x: Int = 7 }\nfunction make(): C(this.x == 9) = new { x = 9 }\nresult = make().x",
            9,
        ),
        (
            "class C { x: Int = 7 }\nfunction make(): Listing<C> = new { new {}; new { x = 9 } }\nresult = make()[0].x + make()[1].x",
            16,
        ),
        (
            "class C { x: Int = 7 }\ntypealias Rows<T> = Mapping<String, Listing<T>>\nfunction make(): Rows<C> = new { [\"a\"] = new { new { x = 11 } } }\nresult = make()[\"a\"][0].x",
            11,
        ),
    ] {
        assert_eq!(
            evaluate(source),
            Ok(Value::Int(expected.into())),
            "{source}"
        );
    }
}

#[test]
fn inherited_and_local_methods_keep_their_return_type_owner_and_captures() {
    for (source, expected) in [
        (
            "class C { x: Int = 7 }\nopen class F { seed = 9; function make(): C = new { x = seed } }\nclass G extends F { seed = 11 }\nresult = (new G {}).make().x",
            11,
        ),
        (
            "open class C { x: Int = 7 }\nclass D extends C { x = 8 }\nopen class F { function make(): C = new { x = 9 } }\nclass G extends F { function make(): D = new {} }\nresult = (new G {}).make().x",
            8,
        ),
        (
            "class C { x: Int = 7 }\nfunction create(n) = new { local function make(): C = let (k = n + 1) new { x = k }; value = make().x }\nresult = create(3).value + create(9).value",
            14,
        ),
        (
            "class C { x: Int = 7 }\nfunction make(n): Listing<C> = new { for (i in List(1, 2)) { new { x = n + i } } }\nresult = make(10)[0].x + make(20)[1].x",
            33,
        ),
        (
            "class C { x: Int = 7 }\nclass D { child: C = new { x = 12 } }\nfunction make(): D = new { child = if (true) let (v = 3) new { x = super.x + v } else new {} }\nresult = make().child.x",
            15,
        ),
        (
            "class C { x: Int = 7 }\none { two { local function make(n): C = let (k = n) new { x = k }; value = make(9) } }\nresult = one.two.value.x",
            9,
        ),
        (
            "class C { x: Int = 7 }\nopen class A { child: C = new { x = 99 } }\nclass B extends A { child = new {} }\nfunction make(): B = new {}\nresult = make().child.x",
            7,
        ),
        (
            "class C { x: Int = 7 }\nclass D { child: C(this.x == 9) = new { x = 7 } }\nfunction make(): D = new { child = new { x = 9 } }\nresult = make().child.x",
            9,
        ),
    ] {
        assert_eq!(
            evaluate(source),
            Ok(Value::Int(expected.into())),
            "{source}"
        );
    }
}

#[test]
fn missing_defaults_remain_lazy_and_first_errors_precede_amendment_members() {
    for ty in ["Int", "Any", "String", "C | Int", "A", "Abstract"] {
        let prefix = "class C {}\nabstract class Abstract {}\ntypealias A = unknown\n";
        let unused = format!("{prefix}function make(): {ty} = new {{}}\nresult = 7");
        assert_eq!(evaluate(&unused), Ok(Value::Int(7.into())), "{ty}");
        let used = format!(
            "{prefix}function make(): {ty} = new {{ x = throw(\"body\") }}\nresult = make()"
        );
        let error = evaluate(&used).unwrap_err();
        assert_eq!(error.message, "Cannot tell which parent to amend.", "{ty}");
        let span = error.span.unwrap();
        assert_eq!(&used[span.start..span.end], "new", "{ty}");
        assert_eq!(
            error.hint.as_ref().and_then(|text| text.as_str()),
            Some(
                "Please specify a parent explicitly, for example `new Person { ... }` instead of `new { ... }`."
            )
        );
    }
    for body in [
        "if (throw(\"earlier\")) new {} else new {}",
        "let (n = throw(\"earlier\")) new {}",
    ] {
        let source = format!("function make(): Int = {body}\nresult = make()");
        assert_eq!(evaluate(&source).unwrap_err().message, "earlier");
    }
    assert_eq!(
        evaluate("function make(): Any = if (false) new {} else 7\nresult = make()"),
        Ok(Value::Int(7.into()))
    );
    let source = "class C {}\nfunction make(): C = if (new {}) new {} else new {}\nresult = make()";
    let error = evaluate(source).unwrap_err();
    assert_eq!(error.kind, ErrorKind::Type);
    assert_eq!(
        error.message,
        "Expected value of type `Boolean`, but got type `#C`.\nValue: new C {}"
    );
    let source = "class C { child: Int }\nfunction make(): C = new { child = let (n = 1) new {} }\nresult = make().child";
    let error = evaluate(source).unwrap_err();
    assert_eq!(error.message, "Cannot tell which parent to amend.");
    assert_eq!(error.frames[0].member.as_str(), Some("#make.child"));
}

#[test]
fn suspended_nested_default_keeps_synthesized_declaration_origin() {
    let source = r#"
open class A { items: Listing<Listing<Any>> }
class B extends A { items = new { new { new {} } } }
result = let (held = (new B {}).items) let (unused = read("mem:resume")) held[0][0]
"#;
    let (mut vm, module) = module(source);
    vm.start_member(module.as_object().unwrap(), "result")
        .unwrap();
    let VmPoll::Pending(read) = event(&mut vm) else {
        panic!("expected Host suspension")
    };
    assert_eq!(read.uri, "mem:resume");
    vm.collect(std::slice::from_ref(&module));
    vm.resume(read.id, Ok(Value::Null)).unwrap();
    let error = ready(&mut vm).unwrap_err();
    assert_eq!(
        error.message,
        "Tried to read property `default` but its value is undefined."
    );
    assert_eq!(error.frames[0].member.as_str(), Some("#A.items.default"));
    let span = error.frames[0].span;
    assert_eq!(&source[span.start..span.end], "Listing<Listing<Any>>");
    vm.close();
    assert_eq!(vm.heap_stats().slots, 0);
}

#[test]
fn only_literal_unknown_inherits_the_dynamic_collection_element_default() {
    for ty in ["unknown", "Any", "U"] {
        let source = format!(
            "typealias U = unknown\nfunction make(): Listing<{ty}> = new {{ new {{}} }}\nresult = make()[0] is Dynamic"
        );
        if ty == "unknown" {
            assert_eq!(evaluate(&source), Ok(Value::Bool(true)));
        } else {
            let error = evaluate(&source).unwrap_err();
            assert_eq!(
                error.message,
                "Tried to read property `default` but its value is undefined."
            );
            assert_eq!(error.frames[0].member.as_str(), Some("#make.default"));
            assert_eq!(error.frames[1].member.as_str(), Some("#make[#1]"));
            let span = error.frames[0].span;
            assert_eq!(
                &source[span.start..span.end],
                format!("function make(): Listing<{ty}>")
            );
            let span = error.frames[1].span;
            assert_eq!(&source[span.start..span.end], "new");
        }
    }
    assert_eq!(
        evaluate(
            "typealias Rows<T> = Listing<T>\nfunction make(): Rows<unknown> = new { new {} }\nresult = make()[0] is Dynamic"
        ),
        Ok(Value::Bool(true))
    );
}

#[test]
fn method_defaults_and_captures_survive_host_failure_gc_and_nonterminal_cancel() {
    let source = r#"
class C { value: Int = read("mem:default") }
class Factory {
  offset = 10
  function make(n): Listing<C> = new { new { value = super.value + n + offset } }
}
unused = (new Factory {}).make(99)
result = (new Factory {}).make(2)[0].value
"#;
    let (mut vm, module) = module(source);
    let object = module.as_object().unwrap();
    for cancel in [false, true] {
        vm.start_member(object, "result").unwrap();
        let VmPoll::Pending(read) = event(&mut vm) else {
            panic!("expected default Host read")
        };
        assert_eq!(read.uri, "mem:default");
        vm.collect(std::slice::from_ref(&module));
        if cancel {
            vm.cancel();
            assert_eq!(ready(&mut vm).unwrap_err().kind, ErrorKind::Cancelled);
        } else {
            vm.resume(
                read.id,
                Err(RuntimeError::new(ErrorKind::Host, "retry default")),
            )
            .unwrap();
            assert_eq!(ready(&mut vm).unwrap_err().kind, ErrorKind::Host);
        }
        assert_eq!(
            vm.member_state(object, "result").unwrap(),
            LazyState::Unevaluated
        );
        assert_eq!(
            vm.resume(read.id, Ok(Value::Null)).unwrap_err().kind,
            ErrorKind::Host
        );
        vm.collect(std::slice::from_ref(&module));
    }
    vm.start_member(object, "result").unwrap();
    let VmPoll::Pending(read) = event(&mut vm) else {
        panic!("expected retry Host read")
    };
    vm.resume(read.id, Ok(Value::Int(30.into()))).unwrap();
    assert_eq!(ready(&mut vm), Ok(Value::Int(42.into())));
    vm.close();
    assert_eq!(vm.heap_stats().slots, 0);
}
