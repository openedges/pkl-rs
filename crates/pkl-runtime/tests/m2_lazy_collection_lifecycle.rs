//! Internal Host continuation/GC checks; separate from Java public byte comparisons.
use pkl_runtime::{
    host::{HostPoll, HostSession},
    parser::parse_module,
    resolver::resolve_module,
    value::{ErrorKind, RuntimeError, Value},
    vm::{Instruction as I, compile_module},
};
use std::sync::Arc;
fn event(session: &mut HostSession, roots: &[Value]) -> HostPoll {
    for _ in 0..200_000 {
        session.collect(roots);
        let result = session.poll(0, 1);
        session.collect(roots);
        if !matches!(result, HostPoll::Yielded) {
            return result;
        }
    }
    panic!("lazy collection Host poll bound")
}
fn ready(session: &mut HostSession, roots: &[Value]) -> Result<Value, RuntimeError> {
    match event(session, roots) {
        HostPoll::Ready(result) => result,
        other => panic!("unexpected {other:?}"),
    }
}
fn read(session: &mut HostSession, module: &Value, uri: &str) -> String {
    match event(session, std::slice::from_ref(module)) {
        HostPoll::Read { token, uri: actual } => {
            assert_eq!(&*actual, uri);
            token
        }
        other => panic!("expected {uri}: {other:?}"),
    }
}
fn resume(session: &mut HostSession, module: &Value, uri: &str, value: Value) {
    let token = read(session, module, uri);
    session.resume(&token, Ok(value), 0).unwrap();
}
fn setup(source: &str, names: &[&str]) -> (HostSession, Value, Vec<usize>) {
    let mut program = (*compile_module(&resolve_module(&parse_module(source).unwrap()).unwrap())
        .unwrap())
    .clone();
    let entries = names
        .iter()
        .map(|name| {
            let block = program.blocks.len();
            program
                .blocks
                .push(vec![I::Input(0), I::GetMember((*name).into()), I::Return]);
            block
        })
        .collect();
    let entry = program.entry;
    let mut session = HostSession::new(Arc::new(program));
    session.start(entry, vec![], 0, None).unwrap();
    let module = ready(&mut session, &[]).unwrap();
    (session, module, entries)
}
#[test]
fn endpoint_selects_only_one_value_and_preserves_nested_read_failure_retry() {
    for name in [
        "first",
        "firstOrNull",
        "last",
        "lastOrNull",
        "single",
        "singleOrNull",
    ] {
        let body = if name.starts_with("first") {
            r#"read("mem:selected"); throw("unselected")"#
        } else if name.starts_with("last") {
            r#"throw("unselected"); read("mem:selected")"#
        } else {
            r#"read("mem:selected")"#
        };
        let source = format!(
            r#"
parent = read("mem:outer")
values = new Listing {{ {body} }}
result = values.{name}
nested = read("mem:nested")
"#
        );
        let (mut session, module, entries) = setup(&source, &["parent", "result", "nested"]);
        session
            .start(entries[0], vec![module.clone()], 0, None)
            .unwrap();
        let outer = read(&mut session, &module, "mem:outer");
        session
            .reenter(&outer, entries[1], vec![module.clone()], 0, None)
            .unwrap();
        let selected = read(&mut session, &module, "mem:selected");
        session
            .reenter(&selected, entries[2], vec![module.clone()], 0, None)
            .unwrap();
        resume(&mut session, &module, "mem:nested", Value::string("nested"));
        assert_eq!(
            ready(&mut session, std::slice::from_ref(&module)),
            Ok(Value::string("nested"))
        );
        assert_eq!(read(&mut session, &module, "mem:selected"), selected);
        session
            .resume(
                &selected,
                Err(RuntimeError::new(ErrorKind::Host, "retry selected")),
                0,
            )
            .unwrap();
        assert_eq!(
            ready(&mut session, std::slice::from_ref(&module))
                .unwrap_err()
                .kind,
            ErrorKind::Host
        );
        assert_eq!(read(&mut session, &module, "mem:outer"), outer);
        session
            .reenter(&outer, entries[1], vec![module.clone()], 0, None)
            .unwrap();
        let retry = read(&mut session, &module, "mem:selected");
        assert_ne!(retry, selected);
        session.resume(&retry, Ok(Value::string("ok")), 0).unwrap();
        assert_eq!(
            ready(&mut session, std::slice::from_ref(&module)),
            Ok(Value::string("ok"))
        );
        assert_eq!(read(&mut session, &module, "mem:outer"), outer);
        session.resume(&outer, Ok(Value::Null), 0).unwrap();
        assert_eq!(
            ready(&mut session, std::slice::from_ref(&module)),
            Ok(Value::Null)
        );
        session.close();
        assert_eq!(session.collect(&[]).slots, 0);
    }
}
#[test]
fn mapping_fold_forces_values_before_callbacks_and_roots_accumulator_through_reentry() {
    let source = r#"
parent = read("mem:outer")
base = new Mapping { ["a"] = read("mem:old"); ["b"] = read("mem:b") }
values = (base) { ["a"] = read("mem:a"); ["c"] = read("mem:c") }
result = values.fold(List(), (acc, key, value) -> if (read("mem:callback/\(key)") == "ok") acc.add(value) else throw("callback failed")).toBytes()
nested = read("mem:nested")
"#;
    let (mut session, module, entries) = setup(source, &["parent", "result", "nested"]);
    session
        .start(entries[0], vec![module.clone()], 0, None)
        .unwrap();
    let outer = read(&mut session, &module, "mem:outer");
    session
        .reenter(&outer, entries[1], vec![module.clone()], 0, None)
        .unwrap();
    for (uri, value) in [("mem:a", 1), ("mem:c", 3), ("mem:b", 2)] {
        resume(&mut session, &module, uri, Value::Int(value.into()));
    }
    resume(&mut session, &module, "mem:callback/a", Value::string("ok"));
    let callback = read(&mut session, &module, "mem:callback/b");
    session
        .reenter(&callback, entries[2], vec![module.clone()], 0, None)
        .unwrap();
    resume(&mut session, &module, "mem:nested", Value::string("nested"));
    assert_eq!(
        ready(&mut session, std::slice::from_ref(&module)),
        Ok(Value::string("nested"))
    );
    assert_eq!(read(&mut session, &module, "mem:callback/b"), callback);
    session
        .resume(
            &callback,
            Err(RuntimeError::new(ErrorKind::Host, "retry fold")),
            0,
        )
        .unwrap();
    assert_eq!(
        ready(&mut session, std::slice::from_ref(&module))
            .unwrap_err()
            .kind,
        ErrorKind::Host
    );
    assert_eq!(read(&mut session, &module, "mem:outer"), outer);
    session
        .reenter(&outer, entries[1], vec![module.clone()], 0, None)
        .unwrap();
    // Shallow receiver values remain cached; the new fold restarts its accumulator.
    for key in ["a", "b", "c"] {
        resume(
            &mut session,
            &module,
            &format!("mem:callback/{key}"),
            Value::string("ok"),
        );
    }
    assert_eq!(
        ready(&mut session, std::slice::from_ref(&module)),
        Ok(Value::Bytes([1, 2, 3].into()))
    );
    assert_eq!(read(&mut session, &module, "mem:outer"), outer);
    session.resume(&outer, Ok(Value::Null), 0).unwrap();
    assert_eq!(
        ready(&mut session, std::slice::from_ref(&module)),
        Ok(Value::Null)
    );
    session.close();
    assert_eq!(session.collect(&[]).slots, 0);
}
#[test]
fn endpoint_and_fold_value_key_callback_boundaries_cancel_close_without_leaks() {
    for close in [false, true] {
        for source in [
            r#"result = (new Listing { read("mem:pending") }).single"#,
            r#"result = (new Mapping { ["x"] = read("mem:pending") }).fold(0, (a, k, v) -> a)"#,
            r#"key = new Dynamic { value = read("mem:pending") }
result = (new Mapping { [key] = 1 }).fold(0, (a, k, v) -> a + v)"#,
            r#"result = (new Mapping { ["x"] = 1 }).fold(0, (a, k, v) -> read("mem:pending"))"#,
        ] {
            let (mut session, module, entries) = setup(source, &["result"]);
            session
                .start(entries[0], vec![module.clone()], 0, None)
                .unwrap();
            let pending = read(&mut session, &module, "mem:pending");
            if close {
                session.close();
            } else {
                session.cancel();
            }
            assert_eq!(
                ready(&mut session, &[]).unwrap_err().kind,
                if close {
                    ErrorKind::Closed
                } else {
                    ErrorKind::Cancelled
                }
            );
            assert!(session.resume(&pending, Ok(Value::Null), 0).is_err());
            assert_eq!(
                session
                    .start(entries[0], vec![module], 0, None)
                    .unwrap_err()
                    .kind,
                ErrorKind::Closed
            );
            assert_eq!(session.collect(&[]).slots, 0);
        }
    }
}
