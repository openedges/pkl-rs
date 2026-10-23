//! Collection bytecode and lifecycle regressions. Compatibility evidence is the
//! separate pinned Java raw-byte comparison corpus.
use pkl_runtime::parser::parse_module;
use pkl_runtime::resolver::resolve_module;
use pkl_runtime::value::{ErrorKind, RuntimeError, Value};
use pkl_runtime::vm::{Vm, VmPoll, compile_module};

fn done(vm: &mut Vm) -> Result<Value, RuntimeError> {
    loop {
        match vm.poll(101) {
            VmPoll::Ready(value) => return value,
            VmPoll::Yielded => {}
            VmPoll::Log(_) => panic!("unexpected logger request"),
            VmPoll::Module(_) => panic!("unexpected module request"),
            VmPoll::Pending(read) => panic!("unexpected read {read:?}"),
        }
    }
}
fn module(source: &str) -> (Vm, Value) {
    let ast = parse_module(source).unwrap();
    let program = compile_module(&resolve_module(&ast).unwrap()).unwrap();
    let mut vm = Vm::new(program.clone());
    vm.start(program.entry, vec![]).unwrap();
    let value = done(&mut vm).unwrap();
    (vm, value)
}
fn evaluate(source: &str) -> Result<Value, RuntimeError> {
    let (mut vm, root) = module(source);
    vm.start_member(root.as_object().unwrap(), "result")
        .unwrap();
    done(&mut vm)
}
#[test]
fn collection_namespaces_lazy_bodies_and_boxed_scalar_keys() {
    assert_eq!(
        evaluate(
            "o = new Dynamic { x = 1; [\"x\"] = 2; missing; 4 }; result = o.x + o[\"x\"] + o[1]"
        ),
        Ok(Value::Int(7.into()))
    );
    assert_eq!(
        evaluate(
            "o = new Mapping { [1] = 10; [1.0] = 20; [0.0] = 3; [-0.0] = 4 }; result = o[1] + o[1.0] + o[0.0] + o[-0.0]"
        ),
        Ok(Value::Int(37.into()))
    );
    assert_eq!(
        evaluate("o = new Listing { missing; 4 }; result = o.length + o[1]"),
        Ok(Value::Int(6.into()))
    );
}
#[test]
fn nested_generators_capture_each_iteration_and_late_receiver() {
    assert_eq!(
        evaluate(
            "o = new Listing { for (i, x in List(10,20)) { for (y in List(1,2)) { (z) -> i + x + y + z } } }; result = o[0].apply(100) + o[3].apply(100)"
        ),
        Ok(Value::Int(234.into()))
    );
    assert_eq!(
        evaluate(
            "base = new Dynamic { x = 2; for (v in List(3,4)) { x + v } }; child = (base) { x = 20 }; result = child[0] + child[1]"
        ),
        Ok(Value::Int(47.into()))
    );
    assert_eq!(
        evaluate(
            "o = new Mapping { for (k,v in new Mapping { [\"a\"] = 1; [\"b\"] = 2 }) { [k] = v * 10 } }; result = o[\"a\"] + o[\"b\"]"
        ),
        Ok(Value::Int(30.into()))
    );
}
#[test]
fn amendments_preserve_entry_identity_and_spread_values() {
    assert_eq!(
        evaluate(
            "base = new Listing { new { x = 1 }; 20 }; child = (base) { [0] { x = 9 }; 30 }; result = child[0].x + child[1] + child[2]"
        ),
        Ok(Value::Int(59.into()))
    );
    assert_eq!(
        evaluate(
            "base = new Listing { ...List(1,2) }; child = (base) { 3 }; result = child[0] + child[2]"
        ),
        Ok(Value::Int(4.into()))
    );
    assert_eq!(
        evaluate(
            "base = new Mapping { [2]=2; [1]=1 }; child = (base) { [2]=20; [3]=3 }; o = new Listing { for (k,v in child) { v } }; result = o[0]*100+o[1]*10+o[2]"
        ),
        Ok(Value::Int(2013.into()))
    );
}
#[test]
fn generator_builders_and_iterators_survive_host_suspension_and_gc() {
    let (mut vm, root) = module(
        "source = new Listing { read(\"mem:first\"); read(\"mem:second\") }; generated = new Mapping { for (i,v in source) { [v] = new { index = i; value = v } } }; result = generated[\"a\"].index + generated[\"b\"].index",
    );
    vm.start_member(root.as_object().unwrap(), "result")
        .unwrap();
    for (uri, value) in [("mem:first", "a"), ("mem:second", "b")] {
        let request = loop {
            match vm.poll(10) {
                VmPoll::Pending(request) => break request,
                VmPoll::Yielded => {}
                other => panic!("unexpected {other:?}"),
            }
        };
        assert_eq!(&*request.uri, uri);
        assert!(vm.collect(std::slice::from_ref(&root)).live > 0);
        vm.resume(request.id, Ok(Value::string(value))).unwrap();
    }
    assert_eq!(done(&mut vm), Ok(Value::Int(1.into())));
    vm.collect(std::slice::from_ref(&root));
    vm.start_member(root.as_object().unwrap(), "result")
        .unwrap();
    assert_eq!(done(&mut vm), Ok(Value::Int(1.into())));
}
#[test]
fn failed_generator_construction_retries_without_publishing_partial_object() {
    let (mut vm, root) =
        module("result = new Listing { 1; when (read(\"mem:condition\") == \"ok\") { 2 } }.length");
    for success in [false, true] {
        vm.start_member(root.as_object().unwrap(), "result")
            .unwrap();
        let request = loop {
            match vm.poll(100) {
                VmPoll::Pending(request) => break request,
                VmPoll::Yielded => {}
                event => panic!("unexpected {event:?}"),
            }
        };
        vm.collect(std::slice::from_ref(&root));
        vm.resume(
            request.id,
            if success {
                Ok(Value::string("ok"))
            } else {
                Err(RuntimeError::new(ErrorKind::Host, "retry"))
            },
        )
        .unwrap();
        if success {
            assert_eq!(done(&mut vm), Ok(Value::Int(2.into())));
        } else {
            assert_eq!(done(&mut vm).unwrap_err().kind, ErrorKind::Host);
        }
    }
}

#[test]
fn source_forcing_precedes_target_generation_and_follows_child_declarations() {
    let error = evaluate("source = new Listing { 1; throw(\"source\") }; generated = new Mapping { for (v in source) { [throw(\"target\")] = v } }; result = generated.length").unwrap_err();
    assert_eq!(error.message, "source");
    let error = evaluate("base = new Mapping { [\"a\"] = throw(\"parent\"); [\"b\"] = 1 }; source = (base) { [\"b\"] = throw(\"child\") }; generated = new Listing { for (v in source) { v } }; result = generated.length").unwrap_err();
    assert_eq!(error.message, "child");
    assert_eq!(
        evaluate(
            "source = new Listing { new { missing = throw(\"nested\") } }; generated = new Listing { for (v in source) { 42 } }; result = generated[0]"
        ),
        Ok(Value::Int(42.into()))
    );
}

#[test]
fn specialized_literals_and_generated_bodies_preserve_distinct_collision_rules() {
    assert_eq!(
        evaluate("o = new Dynamic { [0] = 1; 2 }; result = o[0]"),
        Ok(Value::Int(2.into()))
    );
    assert!(
        evaluate("o = new Dynamic { [0 + 0] = 1; 2 }; result = o[0]")
            .unwrap_err()
            .message
            .contains("out of range")
    );
    assert!(
        evaluate("o = new Dynamic { [0] = 1; when (true) { 2 } }; result = o[0]")
            .unwrap_err()
            .message
            .contains("Duplicate")
    );
}

#[test]
fn default_members_are_excluded_from_iteration_and_empty_entry_amendments() {
    assert_eq!(
        evaluate(
            "source = new Listing { default = throw(\"unused\"); 40; 2 }; o = new Listing { ...source }; result = o[0] + o[1]"
        ),
        Ok(Value::Int(42.into()))
    );
    assert_eq!(
        evaluate("o = new Mapping { [\"a\"] { x = 42 } }; result = o[\"a\"].x"),
        Ok(Value::Int(42.into()))
    );
    assert_eq!(
        evaluate("o = new Listing { for (v in List(1,2)) { ...?null; v } }; result = o.length"),
        Ok(Value::Int(2.into()))
    );
}

#[test]
fn typed_objects_and_modules_are_not_dynamic_or_iterable() {
    assert_eq!(
        evaluate("class C {}; result = (new C {}) is Dynamic"),
        Ok(Value::Bool(false))
    );
    assert_eq!(
        evaluate("result = module is Dynamic"),
        Ok(Value::Bool(false))
    );
    let error =
        evaluate("class C { x=1 }; o = new Listing { for (v in new C {}) { v } }; result=o.length")
            .unwrap_err();
    assert!(error.message.starts_with("Cannot iterate"), "{error:?}");
}

#[test]
fn implicit_collection_members_base_constants_and_guarded_types_resolve_correctly() {
    assert_eq!(
        evaluate("o = new Listing { length }; result = o[0]"),
        Ok(Value::Int(1.into()))
    );
    assert_eq!(
        evaluate(
            "o = new Listing { when(false) { for(n:MissingType in List(1)) {n} }; 42 }; result=o[0]"
        ),
        Ok(Value::Int(42.into()))
    );
    assert_eq!(
        evaluate("NaN=1; Infinity=2; result=NaN+Infinity"),
        Ok(Value::Int(3.into()))
    );
    assert_eq!(evaluate("result=let(NaN=3) NaN"), Ok(Value::Int(3.into())));
    assert_eq!(
        evaluate("o = new Mapping { [NaN] = 42 }; result = o[NaN]"),
        Ok(Value::Int(42.into()))
    );
}
