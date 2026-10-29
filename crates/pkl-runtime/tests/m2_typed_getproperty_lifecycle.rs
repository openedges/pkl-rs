//! Required Typed property lookup Host/GC lifecycle; Stage105 raw Java fixtures govern compatibility.
use pkl_runtime::{
    host::{HostPoll, HostSession},
    parser::parse_module,
    resolver::resolve_module,
    value::{ErrorKind, RuntimeError, Value},
    vm::{Instruction as I, compile_module},
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
            assert_eq!(&*uri, expected);
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
fn selected_typed_value_resumes_nested_host_gc_and_missing_lookup_does_not_poison_it() {
    let source = r#"module stage105.lifecycle
class Data {
  value: String
  hidden concealed = 42
  local scratch = throw("local must stay lazy")
  function method() = throw("method must stay lazy")
  unselected = read("mem:must-remain-lazy")
}
hidden subject = (new Dynamic { value = read("mem:value") }).toTyped(Data)
metadata = subject.hasProperty("value") && subject.hasProperty("concealed") && !subject.hasProperty("scratch")
lookup = subject.getProperty(read("mem:name"))
missing = subject.getProperty("absent")
cached = subject.getProperty("value")
"#;
    let (mut session, module, entries) =
        host_setup(source, &["lookup", "metadata", "missing", "cached"]);
    session
        .start(entries[0], vec![module.clone()], 0, None)
        .unwrap();
    let name = read(&mut session, "mem:name");
    session
        .reenter(&name, entries[1], vec![module.clone()], 0, None)
        .unwrap();
    assert_eq!(ready(&mut session), Ok(Value::Bool(true)));
    assert_eq!(read(&mut session, "mem:name"), name);
    session
        .resume(&name, Ok(Value::string("value")), 0)
        .unwrap();
    let value = read(&mut session, "mem:value");
    session
        .reenter(&value, entries[1], vec![module.clone()], 0, None)
        .unwrap();
    assert_eq!(ready(&mut session), Ok(Value::Bool(true)));
    assert_eq!(read(&mut session, "mem:value"), value);
    let raw = pkl_runtime::value::PklString::from_utf16(vec![0xd800, 0x20, 0xdc00]);
    session
        .resume(&value, Ok(Value::String(raw.clone())), 0)
        .unwrap();
    assert_eq!(ready(&mut session), Ok(Value::String(raw.clone())));
    session
        .start(entries[2], vec![module.clone()], 0, None)
        .unwrap();
    assert_eq!(
        ready(&mut session).unwrap_err().kind,
        ErrorKind::MissingMember
    );
    session.start(entries[3], vec![module], 0, None).unwrap();
    assert_eq!(ready(&mut session), Ok(Value::String(raw)));
    session.close();
    assert_eq!(session.collect(&[]).slots, 0);
}
