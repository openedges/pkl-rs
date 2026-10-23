//! Predicate continuations and lazy replacements. Compatibility claims require
//! the separate pinned Java raw-byte comparison corpus.
use pkl_runtime::parser::parse_module;
use pkl_runtime::resolver::resolve_module;
use pkl_runtime::value::{ErrorKind, RuntimeError, Value};
use pkl_runtime::vm::{Vm, VmPoll, compile_module};

fn done(vm: &mut Vm) -> Result<Value, RuntimeError> {
    loop {
        match vm.poll(31) {
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
    let root = done(&mut vm).unwrap();
    (vm, root)
}

fn evaluate(source: &str) -> Result<Value, RuntimeError> {
    let (mut vm, root) = module(source);
    vm.start_member(root.as_object().unwrap(), "result")
        .unwrap();
    done(&mut vm)
}

#[test]
fn selects_only_parent_entries_and_keeps_replacements_lazy() {
    assert_eq!(
        evaluate(
            "p = new Dynamic { local unused = throw(\"local\"); default = throw(\"default\"); ordinary = throw(\"property\"); [\"x\"] = 1; 2 }; q=(p) { [[this == 1]] = 40; 3 }; result=q[\"x\"]+q[0]+q[1]"
        ),
        Ok(Value::Int(45))
    );
    assert_eq!(
        evaluate(
            "p=new Listing {1;2}; q=(p){3; [[this==3]]=throw(\"new member\"); [[this==1]]=throw(\"lazy\")}; result=q[1]+q[2]+q.length"
        ),
        Ok(Value::Int(8))
    );
    assert_eq!(
        evaluate("result=new Listing { [[throw(\"empty\")]]=9 }.length"),
        Ok(Value::Int(0))
    );
}

#[test]
fn predicates_interleave_child_first_forcing_and_condition_execution() {
    let error = evaluate("p=new Listing {1;throw(\"later source\")};q=(p){[[throw(\"condition\")]]=9};result=q.length").unwrap_err();
    assert_eq!(error.message, "condition");
    let error = evaluate("p=new Mapping {[\"a\"]=throw(\"ancestor\");[\"b\"]=1};c=(p){[\"b\"]=throw(\"child\")};q=(c){[[false]]=9};result=q.length").unwrap_err();
    assert_eq!(error.message, "child");
    let error = evaluate(
        "p=new Listing {throw(\"forced despite false\")};q=(p){[[false]]=9};result=q.length",
    )
    .unwrap_err();
    assert_eq!(error.message, "forced despite false");
    assert_eq!(
        evaluate("p:Listing<Int> =new Listing {1;\"bad\"};q=(p){[[false]]=9};result=q.length"),
        Ok(Value::Int(2))
    );
}

#[test]
fn predicate_custom_this_closures_lexical_captures_and_nested_contexts() {
    assert_eq!(
        evaluate(
            "limit=1;p=new Listing {1;2};q=(p){for(n in List(1,2)){when(n==1){[[let(v=this) (() -> this==v && this==limit).apply()]]=n+40}}};result=q[0]+q[1]"
        ),
        Ok(Value::Int(43))
    );
    assert_eq!(
        evaluate(
            "p=new Listing {1;2};q=(p){[[(p){[[this==2]]=4}[1]==4 && this==1]]=9};result=q[0]+q[1]"
        ),
        Ok(Value::Int(11))
    );
    assert_eq!(
        evaluate(
            "p=new Mapping {[\"a\"] {x=1};[\"b\"] {x=2}};q=(p){[[x==1]] {x=40}};result=q[\"a\"].x+q[\"b\"].x"
        ),
        Ok(Value::Int(42))
    );
    assert_eq!(
        evaluate("p=new Dynamic {x=1;2};q=(p){x=40;[[true]]=this.x};result=q[0]"),
        Ok(Value::Int(40))
    );
}

#[test]
fn duplicates_are_reported_at_selection_and_conditions_require_boolean() {
    let error =
        evaluate("p=new Listing {1;2};q=(p){[0]=3;[[true]]=throw(\"lazy\")};result=q.length")
            .unwrap_err();
    assert_eq!(error.message, "Duplicate definition of member `0`.");
    let error =
        evaluate("p=new Listing {1};q=(p){[[false]]=9;[[1]]=9};result=q.length").unwrap_err();
    assert_eq!(error.kind, ErrorKind::Type);
    assert!(error.message.contains("Boolean"), "{error:?}");
    let error = evaluate("class C {};q=(new C {}) {[[false]]=9};result=q").unwrap_err();
    assert!(
        error.message.contains("cannot have a predicate member"),
        "{error:?}"
    );
    let error = evaluate("class C {};q=new C {[[false]]=9};result=q").unwrap_err();
    assert!(
        error.message.contains("cannot have a predicate member"),
        "{error:?}"
    );
    assert_eq!(
        evaluate("class C {x=42};q=new C {when(false){[[throw(\"unreachable\")]]=9}};result=q.x"),
        Ok(Value::Int(42))
    );
    assert_eq!(
        evaluate("q=(null){[[throw(\"empty\")]]=9; x=42};result=q.x"),
        Ok(Value::Int(42))
    );
}

#[test]
fn predicate_owner_caches_are_reused_by_other_derived_receivers() {
    assert_eq!(
        evaluate("p=new Dynamic {x=1;x};c=(p){x=2};q=(c){[[false]]=9};result=let(unused=q) p[0]"),
        Ok(Value::Int(2))
    );
}

#[test]
fn suspended_source_and_condition_keep_nested_predicate_state_live() {
    let (mut vm, root) = module(
        "p=new Listing {read(\"mem:source\")};q=(p){[[read(\"mem:condition\")==this && (p){[[read(\"mem:nested\")==this]]=9}[0]==9 && this==\"yes\"]]=42};result=q[0]",
    );
    vm.start_member(root.as_object().unwrap(), "result")
        .unwrap();
    for uri in ["mem:source", "mem:condition", "mem:nested"] {
        let request = loop {
            match vm.poll(7) {
                VmPoll::Pending(request) => break request,
                VmPoll::Yielded => {}
                event => panic!("unexpected {event:?}"),
            }
        };
        assert_eq!(&*request.uri, uri);
        vm.collect(std::slice::from_ref(&root));
        vm.resume(request.id, Ok(Value::string("yes"))).unwrap();
    }
    assert_eq!(done(&mut vm), Ok(Value::Int(42)));
}

#[test]
fn failed_predicate_retries_using_completed_source_cache() {
    let (mut vm, root) = module(
        "p=new Listing {read(\"mem:source\")};q=(p){[[read(\"mem:condition\")==this]]=42};result=q[0]",
    );
    for success in [false, true] {
        vm.start_member(root.as_object().unwrap(), "result")
            .unwrap();
        loop {
            match vm.poll(11) {
                VmPoll::Log(_) => panic!("unexpected logger request"),
                VmPoll::Module(_) => panic!("unexpected module request"),
                VmPoll::Pending(request) => {
                    vm.collect(std::slice::from_ref(&root));
                    if request.uri.as_ref() == "mem:source" {
                        assert!(
                            !success,
                            "completed source cache must survive failed condition"
                        );
                        vm.resume(request.id, Ok(Value::string("yes"))).unwrap();
                    } else {
                        vm.resume(
                            request.id,
                            if success {
                                Ok(Value::string("yes"))
                            } else {
                                Err(RuntimeError::new(ErrorKind::Host, "retry"))
                            },
                        )
                        .unwrap();
                    }
                }
                VmPoll::Yielded => {}
                VmPoll::Ready(value) => {
                    if success {
                        assert_eq!(value, Ok(Value::Int(42)));
                    } else {
                        assert_eq!(value.unwrap_err().kind, ErrorKind::Host);
                    }
                    break;
                }
            }
        }
    }
}
