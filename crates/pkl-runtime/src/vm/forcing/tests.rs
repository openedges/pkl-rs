//! Ownership and scheduler assertions; raw compatibility is checked against Java.
use super::*;
use crate::host::{HostPoll, HostSession};
use crate::parser::parse_module;
use crate::resolver::resolve_module;

fn program(source: &str) -> Arc<Program> {
    compile_module(&resolve_module(&parse_module(source).unwrap()).unwrap()).unwrap()
}
fn event(vm: &mut Vm) -> VmPoll {
    for _ in 0..200_000 {
        match vm.poll(1) {
            VmPoll::Yielded => {
                vm.collect(&[]);
            }
            event => return event,
        }
    }
    panic!("VM did not settle")
}
fn ready(vm: &mut Vm) -> Result<Value, RuntimeError> {
    match event(vm) {
        VmPoll::Ready(result) => result,
        event => panic!("unexpected {event:?}"),
    }
}
fn read(vm: &mut Vm) -> u64 {
    match event(vm) {
        VmPoll::Pending(request) => request.id,
        event => panic!("unexpected {event:?}"),
    }
}
fn setup(source: &str) -> (Vm, Value) {
    let p = program(source);
    let entry = p.entry;
    let mut vm = Vm::new(p);
    vm.start(entry, vec![]).unwrap();
    let module = ready(&mut vm).unwrap();
    (vm, module)
}
fn member(vm: &mut Vm, object: &Value, name: &str) -> Result<Value, RuntimeError> {
    vm.start_member(object.as_object().unwrap(), name).unwrap();
    ready(vm)
}
fn callback_program(source: &str, names: &[&str]) -> (Arc<Program>, Vec<usize>) {
    let mut p = (*program(source)).clone();
    let entries = names
        .iter()
        .map(|name| {
            let entry = p.blocks.len();
            p.blocks.push(vec![
                Instruction::Input(0),
                Instruction::GetMember((*name).into()),
                Instruction::Return,
            ]);
            entry
        })
        .collect();
    (Arc::new(p), entries)
}

#[test]
fn reentrant_partial_string_and_callback_undefined_keep_outer_force_alive() {
    let (p, entries) = callback_program(
        r#"
class Required { missing: Int }
required = new Required {}
obj = new Dynamic { x = read("mem:outer"); y = 2 }
first = obj.toString()
second = obj.toString()
callback = obj.toString()
missing = required.missing
"#,
        &["callback", "missing"],
    );
    let entry = p.entry;
    let mut vm = Vm::new(p);
    vm.start(entry, vec![]).unwrap();
    let module = ready(&mut vm).unwrap();
    vm.start_member(module.as_object().unwrap(), "first")
        .unwrap();
    let request = read(&mut vm);
    vm.reenter(request, entries[1], vec![module.clone()])
        .unwrap();
    assert!(ready(&mut vm).unwrap_err().undefined_value);
    vm.finish_reentry();
    vm.reenter(request, entries[0], vec![module.clone()])
        .unwrap();
    assert_eq!(
        ready(&mut vm),
        Ok(Value::string("new Dynamic { x = ?; y = ? }"))
    );
    vm.finish_reentry();
    vm.collect(&[]);
    vm.resume(request, Ok(Value::string("read"))).unwrap();
    assert_eq!(
        ready(&mut vm),
        Ok(Value::string("new Dynamic { x = \"read\"; y = 2 }"))
    );
    assert_eq!(
        member(&mut vm, &module, "second"),
        Ok(Value::string("new Dynamic { x = \"read\"; y = 2 }"))
    );
    vm.close();
    assert_eq!(vm.heap_stats().slots, 0);
}

#[test]
fn undefined_success_survives_parent_failure_and_direct_reads_still_fail() {
    let (mut vm, module) = setup(
        r#"
class Required { missing: Int }
required = new Required {}
child = new Dynamic { x = if (read("mem:child") == "go") required.missing else 0 }
obj = new Dynamic { nested = List(child); bad = throw("later") }
first = obj.toString()
second = obj.toString()
childString = child.toString()
"#,
    );
    vm.start_member(module.as_object().unwrap(), "first")
        .unwrap();
    let request = read(&mut vm);
    vm.resume(request, Ok(Value::string("go"))).unwrap();
    assert_eq!(ready(&mut vm).unwrap_err().message, "later");
    let child = member(&mut vm, &module, "child").unwrap();
    let obj = member(&mut vm, &module, "obj").unwrap();
    assert!(vm.heap.get(child.as_object().unwrap()).unwrap().forced);
    assert!(!vm.heap.get(obj.as_object().unwrap()).unwrap().forced);
    assert_eq!(
        member(&mut vm, &module, "second").unwrap_err().message,
        "later"
    );
    assert_eq!(
        member(&mut vm, &module, "childString"),
        Ok(Value::string("new Dynamic { x = ? }"))
    );
    vm.start_member(child.as_object().unwrap(), "x").unwrap();
    let retry = read(&mut vm);
    vm.resume(retry, Ok(Value::string("go"))).unwrap();
    assert!(ready(&mut vm).unwrap_err().undefined_value);
    assert!(vm.heap.get(child.as_object().unwrap()).unwrap().forced);
    vm.close();
}

#[test]
fn nonterminal_cancel_resets_only_unfinished_forces_in_all_contexts() {
    let (p, entries) = callback_program(
        r#"
child = new Dynamic { done = 1 }
obj = new Dynamic { child = module.child; x = read("mem:outer") }
nested = new Dynamic { x = read("mem:inner") }
first = obj.toString()
callback = nested.toString()
"#,
        &["callback"],
    );
    let entry = p.entry;
    let mut vm = Vm::new(p);
    vm.start(entry, vec![]).unwrap();
    let module = ready(&mut vm).unwrap();
    vm.start_member(module.as_object().unwrap(), "first")
        .unwrap();
    let outer = read(&mut vm);
    vm.reenter(outer, entries[0], vec![module.clone()]).unwrap();
    let inner = read(&mut vm);
    vm.cancel();
    assert_eq!(ready(&mut vm).unwrap_err().kind, ErrorKind::Cancelled);
    vm.collect(std::slice::from_ref(&module));
    for name in ["obj", "nested"] {
        let object = member(&mut vm, &module, name).unwrap();
        assert!(!vm.heap.get(object.as_object().unwrap()).unwrap().forced);
    }
    let child = member(&mut vm, &module, "child").unwrap();
    assert!(vm.heap.get(child.as_object().unwrap()).unwrap().forced);
    for request in [outer, inner] {
        assert_eq!(
            vm.resume(request, Ok(Value::Null)).unwrap_err().kind,
            ErrorKind::Host
        );
    }
    vm.start_member(module.as_object().unwrap(), "first")
        .unwrap();
    let retry = read(&mut vm);
    vm.resume(retry, Ok(Value::Int(9.into()))).unwrap();
    assert!(ready(&mut vm).is_ok());
    vm.close();
    assert_eq!(vm.heap_stats().live, 0);
}

#[test]
fn terminal_host_actions_release_pending_force_and_render_hash_roots() {
    for source in [
        "obj=new Dynamic { x=read(\"mem:outer\") }; first=obj.toString(); nested=new Dynamic { x=read(\"mem:inner\") }; callback=nested.toString()",
        "key=new Dynamic { x=read(\"mem:key\") }; obj=new Mapping { [key]=1 }; first=obj.toString(); nested=new Dynamic { x=read(\"mem:inner\") }; callback=nested.toString()",
    ] {
        for terminal in [ErrorKind::Cancelled, ErrorKind::Closed, ErrorKind::Timeout] {
            let (p, entries) = callback_program(source, &["first", "callback"]);
            let entry = p.entry;
            let mut session = HostSession::new(p);
            session.start(entry, vec![], 0, None).unwrap();
            let module = loop {
                match session.poll(0, 1) {
                    HostPoll::Ready(result) => break result.unwrap(),
                    HostPoll::Yielded => {
                        session.collect(&[]);
                    }
                    event => panic!("{event:?}"),
                }
            };
            session
                .start(entries[0], vec![module.clone()], 1, Some(10))
                .unwrap();
            let outer = loop {
                match session.poll(1, 1) {
                    HostPoll::Read { token, .. } => break token,
                    HostPoll::Yielded => {
                        session.collect(&[]);
                    }
                    event => panic!("{event:?}"),
                }
            };
            session
                .reenter(&outer, entries[1], vec![module], 2, None)
                .unwrap();
            let inner = loop {
                match session.poll(2, 1) {
                    HostPoll::Read { token, .. } => break token,
                    HostPoll::Yielded => {
                        session.collect(&[]);
                    }
                    event => panic!("{event:?}"),
                }
            };
            assert!(session.collect(&[]).live > 0);
            match terminal {
                ErrorKind::Cancelled => session.cancel(),
                ErrorKind::Closed => session.close(),
                _ => {}
            }
            assert!(
                matches!(session.poll(12,1),HostPoll::Ready(Err(error)) if error.kind==terminal)
            );
            assert_eq!(session.heap_stats().slots, 0);
            assert_eq!(
                session.drain_cancelled_reads(),
                [outer.clone(), inner.clone()]
            );
            for token in [outer, inner] {
                assert_eq!(
                    session
                        .resume(&token, Ok(Value::Null), 12)
                        .unwrap_err()
                        .kind,
                    ErrorKind::Closed
                );
            }
        }
    }
}

#[test]
fn inherited_hidden_owners_and_undefined_parent_fallback_use_receiver_cache() {
    let (mut vm, module) = setup(
        r#"
class Required { missing: Int }
open class Base { x=5 }
class Child extends Base { x=(new Required {}).missing }
open class Visible { x=1 }
class Hidden extends Visible { hidden x=2 }
child=new Child {}
cold=new Hidden {}
warm=new Hidden {}
coldString=cold.toString()
warmValue=warm.x
warmString=warm.toString()
childString=child.toString()
"#,
    );
    assert_eq!(
        member(&mut vm, &module, "childString"),
        Ok(Value::string("new Child { x = 5 }"))
    );
    assert_eq!(
        member(&mut vm, &module, "coldString"),
        Ok(Value::string("new Hidden { x = 1 }"))
    );
    assert_eq!(
        member(&mut vm, &module, "warmValue"),
        Ok(Value::Int(1.into()))
    );
    assert_eq!(
        member(&mut vm, &module, "warmString"),
        Ok(Value::string("new Hidden { x = 1 }"))
    );
    let (mut fresh, module) = setup(
        "open class Base { x=1 }; class Child extends Base { hidden x=2 }; obj=new Child {}; value=obj.x; result=obj.toString()",
    );
    assert_eq!(
        member(&mut fresh, &module, "value"),
        Ok(Value::Int(2.into()))
    );
    assert_eq!(
        member(&mut fresh, &module, "result"),
        Ok(Value::string("new Child { x = 2 }"))
    );
}

#[test]
fn builtin_integer_alias_values_have_stable_evaluator_local_identity_and_shadowing() {
    let p = program("result=List(Int8==Int8,Int8==UInt8,Int8.toString()).toString()\nalias=Int8\n");
    let mut values = Vec::new();
    for _ in 0..2 {
        let mut vm = Vm::new(p.clone());
        vm.start(p.entry, vec![]).unwrap();
        let module = ready(&mut vm).unwrap();
        assert_eq!(
            member(&mut vm, &module, "result"),
            Ok(Value::string("List(true, false, \"Int8\")"))
        );
        values.push(member(&mut vm, &module, "alias").unwrap());
        vm.close();
    }
    assert_ne!(values[0], values[1]);
    let (mut vm, module) = setup("Int8=17\nresult=Int8\n");
    assert_eq!(
        member(&mut vm, &module, "result"),
        Ok(Value::Int(17.into()))
    );
}

#[test]
fn deep_force_and_render_are_iterative_and_render_cycles_are_bounded() {
    std::thread::Builder::new()
        .stack_size(256 * 1024)
        .spawn(|| {
            let p = Arc::new(Program::new(vec![vec![
                Instruction::Input(0),
                Instruction::ToString,
                Instruction::Return,
            ]]));
            let mut vm = Vm::new(p);
            let mut value = Value::Int(1.into());
            for _ in 0..2000 {
                let mut object = Object::default();
                object.members.push(Member {
                    flags: 0,
                    entry: false,
                    name: "x".into(),
                    code: usize::MAX,
                    local: false,
                    method: false,
                    ty: None,
                    type_span: None,
                    constant: false,
                    fixed: false,
                });
                object.complete_cache(
                    CacheKey {
                        entry: false,
                        owner: None,
                        name: "x".into(),
                        method: false,
                    },
                    value,
                );
                value = Value::Object(vm.heap.allocate(object));
            }
            vm.start(0, vec![value]).unwrap();
            let mut yields = 0;
            let result = loop {
                match vm.poll(37) {
                    VmPoll::Ready(result) => break result,
                    VmPoll::Yielded => {
                        yields += 1;
                        if yields % 17 == 0 {
                            vm.collect(&[]);
                        }
                    }
                    event => panic!("{event:?}"),
                }
            };
            let Value::String(text) = result.unwrap() else {
                panic!("string")
            };
            assert!(text.as_str().unwrap().starts_with("new Dynamic { x { x {"));
            vm.close();
            assert_eq!(vm.heap_stats().slots, 0);
        })
        .unwrap()
        .join()
        .unwrap();
    let (mut vm, module) = setup("obj=new Dynamic { x=module.obj }\nresult=obj.toString()\n");
    assert_eq!(
        member(&mut vm, &module, "result").unwrap_err().kind,
        ErrorKind::Cycle
    );
    let obj = member(&mut vm, &module, "obj").unwrap();
    assert!(vm.heap.get(obj.as_object().unwrap()).unwrap().forced);
    vm.close();
}

#[test]
fn interpolation_dispatches_typed_override_without_forcing_unrelated_members() {
    let (mut vm, module) = setup(
        r#"
class C {
  bad = throw("unforced")
  function toString(): String = "custom"
}
obj = new C {}
result = "value=\(obj)"
direct = obj.toString()
"#,
    );
    assert_eq!(
        member(&mut vm, &module, "result").unwrap(),
        Value::string("value=custom")
    );
    assert_eq!(
        member(&mut vm, &module, "direct").unwrap(),
        Value::string("custom")
    );
    let obj = member(&mut vm, &module, "obj").unwrap();
    assert!(!vm.heap.get(obj.as_object().unwrap()).unwrap().forced);
    assert_eq!(
        member(&mut vm, &obj, "bad").unwrap_err().message,
        "unforced"
    );
}

#[test]
fn inherited_object_entry_keys_compare_before_ancestor_reads_and_render_at_parent_position() {
    let (mut vm, module) = setup(
        r#"
k1 = new Dynamic { x = 1 }
k2 = new Dynamic { x = 1 }
k3 = new Dynamic { y = 0 }
base = new Mapping { [k1] = throw("overridden") }
obj = (base) { [k2] = "child"; [k3] = "collision" }
result = obj.toString()
"#,
    );
    assert_eq!(
        member(&mut vm, &module, "result").unwrap(),
        Value::string(
            "new Mapping { [new Dynamic { x = 1 }] = \"child\"; [new Dynamic { y = 0 }] = \"collision\" }"
        )
    );
}

#[test]
fn internal_declaration_bits_preserve_modifier_arithmetic() {
    // Pinned modifier arithmetic carries repeated CONST into IMPORT, CLASS,
    // then TYPE_ALIAS. Only the latter two hide a declaration from rendering.
    for (count, expected) in [(2, "new C { x = 1 }"), (4, "new C {}"), (8, "new C {}")] {
        let source = format!(
            "class C {{ {}x = 1 }}\nresult = new C {{}}.toString()",
            "const ".repeat(count)
        );
        let (mut vm, module) = setup(&source);
        assert_eq!(
            member(&mut vm, &module, "result").unwrap(),
            Value::string(expected)
        );
    }
}

#[test]
fn nested_callback_errors_keep_source_boundaries_and_do_not_unwind_waiting_contexts() {
    let (p, entries) = callback_program(
        r#"
obj = new Dynamic { x = read("mem:outer") }
obj2 = new Dynamic { x = read("mem:inner") }
first = obj.x
second = obj2.x
failure = throw("nested")
"#,
        &["first", "second", "failure"],
    );
    let mut p = (*p).clone();
    for (&entry, name) in entries.iter().zip(["first", "second", "failure"]) {
        p.metadata
            .resize_with(p.blocks.len(), BlockMetadata::default);
        p.metadata[entry] = BlockMetadata {
            source: Some(Arc::new(crate::value::RuntimeSource {
                text: name.into(),
                uri: "repl:text".into(),
                module_name: "".into(),
            })),
            spans: vec![None, Some(Span::new(0, name.len())), None],
            ..BlockMetadata::default()
        };
    }
    let entry = p.entry;
    let mut vm = Vm::new(Arc::new(p));
    vm.start(entry, vec![]).unwrap();
    let module = ready(&mut vm).unwrap();
    vm.start(entries[0], vec![module.clone()]).unwrap();
    let outer = read(&mut vm);
    vm.reenter(outer, entries[1], vec![module.clone()]).unwrap();
    let inner = read(&mut vm);
    vm.reenter(inner, entries[2], vec![module.clone()]).unwrap();
    let error = ready(&mut vm).unwrap_err();
    let names: Vec<_> = error
        .frames
        .iter()
        .map(|frame| {
            frame.source.as_ref().map_or_else(
                || frame.member.to_java_utf8(),
                |source| format!("{}:{}", source.uri, source.text),
            )
        })
        .collect();
    assert_eq!(
        names,
        [
            "#failure",
            "repl:text:failure",
            "#second",
            "repl:text:second",
            "#first",
            "repl:text:first"
        ]
    );
    vm.finish_reentry();
    vm.collect(std::slice::from_ref(&module));
    assert!(matches!(vm.poll(1), VmPoll::Pending(request) if request.id == inner));
    vm.resume(inner, Ok(Value::string("inner"))).unwrap();
    assert_eq!(ready(&mut vm), Ok(Value::string("inner")));
    vm.finish_reentry();
    assert!(matches!(vm.poll(1), VmPoll::Pending(request) if request.id == outer));
    vm.resume(outer, Ok(Value::string("outer"))).unwrap();
    assert_eq!(ready(&mut vm), Ok(Value::string("outer")));
    vm.start(entries[2], vec![module]).unwrap();
    let later = ready(&mut vm).unwrap_err();
    assert_eq!(later.frames.len(), 2);
    assert_eq!(later.frames[0].member.to_java_utf8(), "#failure");
    vm.close();
    assert_eq!(vm.heap_stats().slots, 0);
}

#[test]
fn missing_dynamic_properties_report_inherited_names_without_forcing_values() {
    let (mut vm, module) = setup(
        r#"
base = new Dynamic { inherited = read("mem:must-remain-lazy") }
obj = (base) { local secret = 1; authored = 2 }
result = obj.absent
"#,
    );
    let error = member(&mut vm, &module, "result").unwrap_err();
    assert_eq!(
        error.message,
        "Cannot find property `absent` in object of type `Dynamic`."
    );
    assert_eq!(
        error.hint.as_ref().and_then(|text| text.as_str()),
        Some("Available properties:\nauthored\ndefault\ninherited")
    );
    let obj = member(&mut vm, &module, "obj").unwrap();
    assert!(!vm.heap.get(obj.as_object().unwrap()).unwrap().forced);
}

#[test]
fn failed_output_shallow_force_clears_reentrant_deep_mark_but_keeps_child_cache() {
    let (p, entries) = callback_program(
        r#"
obj = new Dynamic { x = read("life:outer"); nested = new Dynamic { y = 7 } }
callback = obj.toString()
output { value = obj }
"#,
        &["callback"],
    );
    let entry = p.entry;
    let mut vm = Vm::new(p);
    vm.start(entry, vec![]).unwrap();
    let module = ready(&mut vm).unwrap();
    let object = member(&mut vm, &module, "obj").unwrap();
    let output = member(&mut vm, &module, "output").unwrap();
    vm.start_member(output.as_object().unwrap(), "bytes")
        .unwrap();
    let format = read(&mut vm);
    vm.resume(format, Ok(Value::Null)).unwrap();
    let outer = read(&mut vm);
    vm.reenter(outer, entries[0], vec![module.clone()]).unwrap();
    let inner = read(&mut vm);
    vm.resume(inner, Ok(Value::string("inner"))).unwrap();
    assert!(matches!(ready(&mut vm), Ok(Value::String(_))));
    vm.finish_reentry();
    let object_id = object.as_object().unwrap();
    assert!(vm.heap.get(object_id).unwrap().forced);
    vm.resume(
        outer,
        Err(RuntimeError::new(ErrorKind::Unsupported, "reader failure")),
    )
    .unwrap();
    assert_eq!(ready(&mut vm).unwrap_err().message, "reader failure");
    assert!(!vm.heap.get(object_id).unwrap().forced);
    let child = member(&mut vm, &object, "nested")
        .unwrap()
        .as_object()
        .unwrap();
    assert!(vm.heap.get(child).unwrap().forced);
    assert_eq!(member(&mut vm, &object, "x"), Ok(Value::string("inner")));
    // Retry consumes successful callback caches without issuing a new read.
    assert_eq!(
        member(&mut vm, &output, "bytes"),
        Ok(Value::Bytes(
            Arc::<[u8]>::from(b"x = \"inner\"\nnested {\n  y = 7\n}\n".as_slice()).into()
        ))
    );
    assert!(!vm.heap.get(object_id).unwrap().forced);
    vm.close();
    assert_eq!(vm.heap_stats().slots, 0);
}
