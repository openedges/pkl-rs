//! Host/GC and cached-publication regression cases, independent of raw Java
//! fixture generation. The separate Stage104 oracle captures govern compatibility.
use pkl_runtime::{
    host::{HostPoll, HostSession},
    parser::parse_module,
    resolver::resolve_module,
    value::{ErrorKind, ObjectId, RuntimeError, Value},
    vm::{Instruction as I, Vm, VmPoll, compile_module},
};
use std::sync::Arc;

fn event(session: &mut HostSession) -> HostPoll {
    for _ in 0..200_000 {
        session.collect(&[]);
        match session.poll(0, 1) {
            HostPoll::Yielded => {}
            event => return event,
        }
    }
    panic!("Typed.hasProperty Host poll bound")
}
fn ready(session: &mut HostSession) -> Result<Value, RuntimeError> {
    match event(session) {
        HostPoll::Ready(value) => value,
        event => panic!("unexpected {event:?}"),
    }
}
fn read(session: &mut HostSession, expected: &str) -> String {
    match event(session) {
        HostPoll::Read { token, uri } => {
            assert_eq!(uri, expected);
            token
        }
        event => panic!("unexpected {event:?}"),
    }
}
fn host_setup(source: &str, names: &[&str]) -> (HostSession, Value, Vec<usize>) {
    let mut program = (*compile_module(&resolve_module(&parse_module(source).unwrap()).unwrap())
        .unwrap())
    .clone();
    let entries = names
        .iter()
        .map(|name| {
            let entry = program.blocks.len();
            program
                .blocks
                .push(vec![I::Input(0), I::GetMember((*name).into()), I::Return]);
            entry
        })
        .collect();
    let entry = program.entry;
    let mut session = HostSession::new(Arc::new(program));
    session.start(entry, vec![], 0, None).unwrap();
    let module = ready(&mut session).unwrap();
    (session, module, entries)
}

#[test]
fn metadata_lookup_survives_argument_suspension_nested_reentry_gc_and_later_value_read() {
    let source = r#"
open class Parent {
  inherited: String
  hidden concealed = read("mem:hidden-must-remain-lazy")
  local scratch = read("mem:local-must-remain-lazy")
  function method() = read("mem:method-must-remain-lazy")
}
class Data extends Parent { value: String }
hidden subject = (new Dynamic {
  value = read("mem:value")
  extra = read("mem:extra-must-remain-lazy")
}).toTyped(Data)
outerRead = read("mem:outer")
lookup = subject.hasProperty(read("mem:name"))
metadata = subject.hasProperty("inherited") && subject.hasProperty("concealed")
  && !subject.hasProperty("scratch") && !subject.hasProperty("method")
  && !subject.hasProperty("extra") && !subject.hasProperty("default")
value = subject.value
"#;
    let (mut session, module, entries) =
        host_setup(source, &["outerRead", "lookup", "metadata", "value"]);
    session
        .start(entries[0], vec![module.clone()], 0, None)
        .unwrap();
    let outer = read(&mut session, "mem:outer");
    session
        .reenter(&outer, entries[1], vec![module.clone()], 0, None)
        .unwrap();
    let name = read(&mut session, "mem:name");
    session
        .reenter(&name, entries[2], vec![module.clone()], 0, None)
        .unwrap();
    // Any forced property, method, delegate entry or implicit Dynamic default
    // would produce a different Host event instead of this immediate Boolean.
    assert_eq!(ready(&mut session), Ok(Value::Bool(true)));
    assert_eq!(read(&mut session, "mem:name"), name);
    session
        .resume(&name, Ok(Value::string("value")), 0)
        .unwrap();
    assert_eq!(ready(&mut session), Ok(Value::Bool(true)));
    assert_eq!(read(&mut session, "mem:outer"), outer);
    session
        .reenter(&outer, entries[3], vec![module], 0, None)
        .unwrap();
    let value = read(&mut session, "mem:value");
    session
        .resume(&value, Ok(Value::string("later")), 0)
        .unwrap();
    assert_eq!(ready(&mut session), Ok(Value::string("later")));
    assert_eq!(read(&mut session, "mem:outer"), outer);
    session.resume(&outer, Ok(Value::Null), 0).unwrap();
    assert_eq!(ready(&mut session), Ok(Value::Null));
    session.close();
    assert_eq!(session.collect(&[]).slots, 0);
}

fn finish(vm: &mut Vm) -> Result<Value, RuntimeError> {
    for _ in 0..200_000 {
        vm.collect(&[]);
        match vm.poll(1) {
            VmPoll::Yielded => {}
            VmPoll::Ready(value) => return value,
            event => panic!("unexpected {event:?}"),
        }
    }
    panic!("Typed.hasProperty publication poll bound")
}
fn module(source: &str) -> (Vm, ObjectId) {
    let program = compile_module(&resolve_module(&parse_module(source).unwrap()).unwrap()).unwrap();
    let entry = program.entry;
    let mut vm = Vm::new(program);
    vm.start(entry, vec![]).unwrap();
    let module = finish(&mut vm).unwrap().as_object().unwrap();
    (vm, module)
}
fn member(vm: &mut Vm, module: ObjectId, name: &str) -> Result<Value, RuntimeError> {
    vm.start_member(module, name)?;
    finish(vm)
}

#[test]
fn cached_failed_class_uses_initialization_guard_but_unlinked_parent_has_no_builtin() {
    let (mut vm, owner) = module(
        "class C { before = 1; broken: Missing; after = 2; local scratch = 3; function method() = 0 }\nunknownName = new C {}.hasProperty(\"absent\")\nlocalName = new C {}.hasProperty(\"scratch\")\nmethodName = new C {}.hasProperty(\"method\")\ndefaultName = new C {}.hasProperty(\"default\")",
    );
    assert_eq!(
        member(&mut vm, owner, "C").unwrap_err().message,
        "Cannot find type `Missing`."
    );
    for name in ["unknownName", "localName", "methodName", "defaultName"] {
        assert_eq!(member(&mut vm, owner, name), Ok(Value::Bool(true)));
    }
    vm.close();
    assert_eq!(vm.collect(&[]).slots, 0);
    let (mut vm, owner) =
        module("class C extends Missing { own = 1 }\nlookup = new C {}.hasProperty(\"own\")");
    assert_eq!(
        member(&mut vm, owner, "C").unwrap_err().message,
        "Cannot find type `Missing`."
    );
    assert_eq!(
        member(&mut vm, owner, "lookup").unwrap_err().kind,
        ErrorKind::MissingMember
    );
    vm.close();
    assert_eq!(vm.collect(&[]).slots, 0);
}

#[test]
fn nullable_typed_lookup_keeps_missing_names_lazy_and_resumes_selected_value() {
    let source = r#"
class Data {
  value: String
  hidden concealed = 42
  local scratch = read("mem:local-must-remain-lazy")
  function method() = read("mem:method-must-remain-lazy")
}
hidden subject = (new Dynamic { value = read("mem:value") }).toTyped(Data)
metadata = subject.getPropertyOrNull("absent") == null
  && subject.getPropertyOrNull("default") == null
  && subject.getPropertyOrNull("scratch") == null
  && subject.getPropertyOrNull("method") == null
  && subject.getPropertyOrNull("concealed") == 42
lookup = subject.getPropertyOrNull(read("mem:name"))
"#;
    let (mut session, module, entries) = host_setup(source, &["metadata", "lookup"]);
    session
        .start(entries[0], vec![module.clone()], 0, None)
        .unwrap();
    assert_eq!(ready(&mut session), Ok(Value::Bool(true)));
    session
        .start(entries[1], vec![module.clone()], 0, None)
        .unwrap();
    let name = read(&mut session, "mem:name");
    session
        .resume(&name, Ok(Value::string("value")), 0)
        .unwrap();
    let value = read(&mut session, "mem:value");
    session
        .resume(&value, Ok(Value::string("delivered")), 0)
        .unwrap();
    assert_eq!(ready(&mut session), Ok(Value::string("delivered")));
    session.start(entries[1], vec![module], 0, None).unwrap();
    assert_eq!(ready(&mut session), Ok(Value::string("delivered")));
    session.close();
    assert_eq!(session.collect(&[]).slots, 0);
}
