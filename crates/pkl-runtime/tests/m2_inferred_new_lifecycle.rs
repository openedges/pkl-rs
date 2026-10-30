//! Authored Host/GC controls, separate from Java text compatibility observations.
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
            other => return other,
        }
    }
    panic!("inferred parent Host poll bound")
}
fn ready(session: &mut HostSession) -> Result<Value, RuntimeError> {
    match event(session) {
        HostPoll::Ready(value) => value,
        other => panic!("unexpected {other:?}"),
    }
}
fn read(session: &mut HostSession, expected: &str) -> String {
    match event(session) {
        HostPoll::Read { token, uri } => {
            assert_eq!(uri, expected);
            token
        }
        other => panic!("unexpected {other:?}"),
    }
}
fn setup() -> (HostSession, Value, Vec<usize>) {
    let source = r#"
outerValue = read("mem:outer")
class Person { name: String = read("mem:person") }
typealias MaybePerson = Person?
baseMapping = new Mapping {
  default = (key) -> let (seed = read("mem:default-\(key)")) new Dynamic {
    name = seed
    origin = key
  }
  ["existing"] = new Dynamic { name = "inherited" }
}
assigned = (baseMapping) {
  ["existing"] = if (true) let (unused = 1) new { suffix = read("mem:suffix") }
    else throw("unselected")
}
assignedResult = assigned["existing"].name + assigned["existing"].suffix
bareResult = ((baseMapping) { ["existing"] { extra = 1 } })["existing"].name
missingResult = ((baseMapping) { ["missing"] { extra = 1 } })["missing"].name
compositeResult = (new MaybePerson {}).name
function implicit() = new { name = read("mem:implicit") }
implicitResult = implicit().name
function typed(): Person = if (read("mem:gate")) new { name = read("mem:body") } else new {}
typedResult = typed().name
"#;
    let resolved = resolve_module(&parse_module(source).unwrap()).unwrap();
    let mut program = (*compile_module(&resolved).unwrap()).clone();
    let entries = [
        "outerValue",
        "assignedResult",
        "bareResult",
        "missingResult",
        "compositeResult",
        "implicitResult",
        "typedResult",
    ]
    .into_iter()
    .map(|name| {
        let entry = program.blocks.len();
        program
            .blocks
            .push(vec![I::Input(0), I::GetMember(name.into()), I::Return]);
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
fn assigned_entry_default_retries_and_preserves_nested_parent_calls_across_gc() {
    let (mut session, module, entries) = setup();
    session
        .start(entries[0], vec![module.clone()], 0, None)
        .unwrap();
    let outer = read(&mut session, "mem:outer");
    session
        .reenter(&outer, entries[1], vec![module.clone()], 0, None)
        .unwrap();
    let default = read(&mut session, "mem:default-existing");
    session
        .resume(
            &default,
            Err(RuntimeError::new(ErrorKind::Host, "retry default")),
            0,
        )
        .unwrap();
    assert_eq!(ready(&mut session).unwrap_err().kind, ErrorKind::Host);
    assert_eq!(read(&mut session, "mem:outer"), outer);
    session
        .reenter(&outer, entries[1], vec![module.clone()], 0, None)
        .unwrap();
    let default = read(&mut session, "mem:default-existing");

    // A bare amendment reads the inherited entry and never calls default.
    session
        .reenter(&default, entries[2], vec![module.clone()], 0, None)
        .unwrap();
    assert_eq!(ready(&mut session), Ok(Value::string("inherited")));
    assert_eq!(read(&mut session, "mem:default-existing"), default);
    session
        .reenter(&default, entries[4], vec![module.clone()], 0, None)
        .unwrap();
    let person = read(&mut session, "mem:person");
    session
        .resume(&person, Ok(Value::string("person")), 0)
        .unwrap();
    assert_eq!(ready(&mut session), Ok(Value::string("person")));
    assert_eq!(read(&mut session, "mem:default-existing"), default);
    session
        .resume(&default, Ok(Value::string("new")), 0)
        .unwrap();
    let suffix = read(&mut session, "mem:suffix");
    session
        .reenter(&suffix, entries[5], vec![module.clone()], 0, None)
        .unwrap();
    let implicit = read(&mut session, "mem:implicit");
    session
        .resume(&implicit, Ok(Value::string("method")), 0)
        .unwrap();
    assert_eq!(ready(&mut session), Ok(Value::string("method")));
    assert_eq!(read(&mut session, "mem:suffix"), suffix);
    session
        .resume(&suffix, Ok(Value::string("-suffix")), 0)
        .unwrap();
    assert_eq!(ready(&mut session), Ok(Value::string("new-suffix")));
    assert_eq!(read(&mut session, "mem:outer"), outer);
    session
        .reenter(&outer, entries[1], vec![module], 0, None)
        .unwrap();
    assert_eq!(ready(&mut session), Ok(Value::string("new-suffix")));
    assert_eq!(read(&mut session, "mem:outer"), outer);
    session.resume(&outer, Ok(Value::Null), 0).unwrap();
    assert_eq!(ready(&mut session), Ok(Value::Null));
    session.close();
    assert_eq!(session.collect(&[]).slots, 0);
}

#[test]
fn absent_bare_entry_uses_default_and_cancel_releases_parent_selection() {
    for (entry, complete_default, close) in [
        (3, false, false),
        (3, false, true),
        (1, true, false),
        (1, true, true),
    ] {
        let (mut session, module, entries) = setup();
        session
            .start(entries[entry], vec![module], 0, None)
            .unwrap();
        let mut pending = read(
            &mut session,
            if entry == 3 {
                "mem:default-missing"
            } else {
                "mem:default-existing"
            },
        );
        if complete_default {
            session
                .resume(&pending, Ok(Value::string("value")), 0)
                .unwrap();
            pending = read(&mut session, "mem:suffix");
        }
        if close {
            session.close();
        } else {
            session.cancel();
        }
        assert_eq!(
            ready(&mut session).unwrap_err().kind,
            if close {
                ErrorKind::Closed
            } else {
                ErrorKind::Cancelled
            }
        );
        assert_eq!(
            session
                .resume(&pending, Ok(Value::Null), 0)
                .unwrap_err()
                .kind,
            ErrorKind::Closed
        );
        assert_eq!(session.collect(&[]).slots, 0);
    }
}

#[test]
fn typed_method_parent_survives_body_failure_and_retry_in_a_suspended_call() {
    for _ in 0..2 {
        let (mut session, module, entries) = setup();
        session
            .start(entries[0], vec![module.clone()], 0, None)
            .unwrap();
        let outer = read(&mut session, "mem:outer");
        for phase in 0..3 {
            session
                .reenter(&outer, entries[6], vec![module.clone()], 0, None)
                .unwrap();
            let gate = read(&mut session, "mem:gate");
            if phase == 0 {
                session
                    .resume(
                        &gate,
                        Err(RuntimeError::new(ErrorKind::Host, "before parent")),
                        0,
                    )
                    .unwrap();
                assert_eq!(ready(&mut session).unwrap_err().kind, ErrorKind::Host);
            } else {
                session.resume(&gate, Ok(Value::Bool(true)), 0).unwrap();
                let body = read(&mut session, "mem:body");
                if phase == 1 {
                    session
                        .resume(
                            &body,
                            Err(RuntimeError::new(ErrorKind::Host, "after parent")),
                            0,
                        )
                        .unwrap();
                    assert_eq!(ready(&mut session).unwrap_err().kind, ErrorKind::Host);
                } else {
                    session.resume(&body, Ok(Value::string("body")), 0).unwrap();
                    assert_eq!(ready(&mut session), Ok(Value::string("body")));
                }
            }
            assert_eq!(read(&mut session, "mem:outer"), outer);
        }
        session.close();
        assert_eq!(session.collect(&[]).slots, 0);
    }
}
