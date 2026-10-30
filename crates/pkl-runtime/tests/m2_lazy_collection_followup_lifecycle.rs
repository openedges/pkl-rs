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
            assert_eq!(actual, uri);
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
fn lazy_any_and_contains_preserve_key_value_callback_and_operand_order() {
    let source = r#"
key = new Dynamic { value = read("mem:key") }
values = new Mapping { [key] = read("mem:value"); ["tail"] = throw("tail") }
result = values.any((k, v) -> read("mem:predicate") == v)
query = new Dynamic { value = read("mem:query") }
stored = new Dynamic { value = read("mem:stored") }
items = new Listing { module.stored; throw("tail") }
contains = items.contains(query)
parent = read("mem:parent")
nested = read("mem:nested")
"#;
    let (mut session, module, entries) = setup(source, &["parent", "result", "contains", "nested"]);
    session
        .start(entries[0], vec![module.clone()], 0, None)
        .unwrap();
    let parent = read(&mut session, &module, "mem:parent");
    session
        .reenter(&parent, entries[1], vec![module.clone()], 0, None)
        .unwrap();
    resume(&mut session, &module, "mem:key", Value::Int(9.into()));
    resume(&mut session, &module, "mem:value", Value::Int(1.into()));
    let predicate = read(&mut session, &module, "mem:predicate");
    session
        .reenter(&predicate, entries[3], vec![module.clone()], 0, None)
        .unwrap();
    resume(&mut session, &module, "mem:nested", Value::string("nested"));
    assert_eq!(
        ready(&mut session, std::slice::from_ref(&module)),
        Ok(Value::string("nested"))
    );
    assert_eq!(read(&mut session, &module, "mem:predicate"), predicate);
    session
        .resume(&predicate, Ok(Value::Int(1.into())), 0)
        .unwrap();
    assert_eq!(
        ready(&mut session, std::slice::from_ref(&module)),
        Ok(Value::Bool(true))
    );
    assert_eq!(read(&mut session, &module, "mem:parent"), parent);
    session
        .reenter(&parent, entries[2], vec![module.clone()], 0, None)
        .unwrap();
    // Unlike strict List.contains, Listing uses query.equals(stored).
    resume(&mut session, &module, "mem:query", Value::Int(7.into()));
    resume(&mut session, &module, "mem:stored", Value::Int(7.into()));
    assert_eq!(
        ready(&mut session, std::slice::from_ref(&module)),
        Ok(Value::Bool(true))
    );
    assert_eq!(read(&mut session, &module, "mem:parent"), parent);
    session.resume(&parent, Ok(Value::Null), 0).unwrap();
    assert_eq!(
        ready(&mut session, std::slice::from_ref(&module)),
        Ok(Value::Null)
    );
    session.close();
    assert_eq!(session.collect(&[]).slots, 0);
}

#[test]
fn indexed_fold_forces_all_values_then_restarts_accumulator_after_callback_failure() {
    let source = r#"
base = new Listing { read("mem:old"); read("mem:b") }
values = (base) { [0] = read("mem:a"); read("mem:c") }
result = values.foldIndexed(List(), (index, acc, value) -> if (read("mem:callback/\(index)") == "ok") acc.add(value) else throw("callback failed")).toBytes()
parent = read("mem:parent")
nested = read("mem:nested")
"#;
    let (mut session, module, entries) = setup(source, &["parent", "result", "nested"]);
    session
        .start(entries[0], vec![module.clone()], 0, None)
        .unwrap();
    let parent = read(&mut session, &module, "mem:parent");
    session
        .reenter(&parent, entries[1], vec![module.clone()], 0, None)
        .unwrap();
    for (uri, value) in [("mem:a", 1), ("mem:c", 3), ("mem:b", 2)] {
        resume(&mut session, &module, uri, Value::Int(value.into()));
    }
    resume(&mut session, &module, "mem:callback/0", Value::string("ok"));
    let callback = read(&mut session, &module, "mem:callback/1");
    session
        .reenter(&callback, entries[2], vec![module.clone()], 0, None)
        .unwrap();
    resume(&mut session, &module, "mem:nested", Value::string("nested"));
    assert_eq!(
        ready(&mut session, std::slice::from_ref(&module)),
        Ok(Value::string("nested"))
    );
    session
        .resume(
            &callback,
            Err(RuntimeError::new(ErrorKind::Host, "retry indexed fold")),
            0,
        )
        .unwrap();
    assert_eq!(
        ready(&mut session, std::slice::from_ref(&module))
            .unwrap_err()
            .kind,
        ErrorKind::Host
    );
    assert_eq!(read(&mut session, &module, "mem:parent"), parent);
    session
        .reenter(&parent, entries[1], vec![module.clone()], 0, None)
        .unwrap();
    // Cached receiver values are not read again; callback traversal starts over.
    for index in 0..3 {
        resume(
            &mut session,
            &module,
            &format!("mem:callback/{index}"),
            Value::string("ok"),
        );
    }
    assert_eq!(
        ready(&mut session, std::slice::from_ref(&module)),
        Ok(Value::Bytes([1, 2, 3].into()))
    );
    session.resume(&parent, Ok(Value::Null), 0).unwrap();
    assert_eq!(
        ready(&mut session, std::slice::from_ref(&module)),
        Ok(Value::Null)
    );
    session.close();
    assert_eq!(session.collect(&[]).slots, 0);
}

#[test]
fn selector_hash_keeps_original_values_partial_builder_and_projected_keys_rooted() {
    let source = r#"
values = new Listing { 1; 2; 3 }
result = values.distinctBy((value) -> new Dynamic { n = read("mem:key/\(value)") }).toList().toBytes()
parent = read("mem:parent")
nested = read("mem:nested")
"#;
    let (mut session, module, entries) = setup(source, &["parent", "result", "nested"]);
    session
        .start(entries[0], vec![module.clone()], 0, None)
        .unwrap();
    let parent = read(&mut session, &module, "mem:parent");
    session
        .reenter(&parent, entries[1], vec![module.clone()], 0, None)
        .unwrap();
    resume(&mut session, &module, "mem:key/1", Value::Int(0.into()));
    let key = read(&mut session, &module, "mem:key/2");
    session
        .reenter(&key, entries[2], vec![module.clone()], 0, None)
        .unwrap();
    resume(&mut session, &module, "mem:nested", Value::string("nested"));
    assert_eq!(
        ready(&mut session, std::slice::from_ref(&module)),
        Ok(Value::string("nested"))
    );
    assert_eq!(read(&mut session, &module, "mem:key/2"), key);
    session.resume(&key, Ok(Value::Int(0.into())), 0).unwrap();
    resume(&mut session, &module, "mem:key/3", Value::Int(1.into()));
    assert_eq!(
        ready(&mut session, std::slice::from_ref(&module)),
        Ok(Value::Bytes([1, 3].into()))
    );
    session.resume(&parent, Ok(Value::Null), 0).unwrap();
    assert_eq!(
        ready(&mut session, std::slice::from_ref(&module)),
        Ok(Value::Null)
    );
    session.close();
    assert_eq!(session.collect(&[]).slots, 0);
}

#[test]
fn lazy_visitor_cancel_close_covers_key_value_predicate_equality_projection_and_join() {
    for close in [false, true] {
        for source in [
            r#"key = new Dynamic { x = read("mem:pending") }
result = (new Mapping { [key] = 1 }).any((k, v) -> true)"#,
            r#"result = (new Listing { read("mem:pending") }).any((v) -> true)"#,
            r#"result = (new Listing { 1 }).any((v) -> read("mem:pending"))"#,
            r#"query = new Dynamic { x = read("mem:pending") }
items = new Listing { new Dynamic { x = 1 } }
result = items.contains(query)"#,
            r#"query = new Dynamic { x = 1 }
items = new Listing { new Dynamic { x = read("mem:pending") } }
result = items.contains(query)"#,
            r#"result = (new Listing { 1 }).distinctBy((v) -> new Dynamic { x = read("mem:pending") })"#,
            r#"result = (new Listing { 1 }).isDistinctBy((v) -> new Dynamic { x = read("mem:pending") })"#,
            r#"result = (new Listing { new Dynamic { x = read("mem:pending") } }).distinct"#,
            r#"result = (new Listing { 1 }).foldIndexed(0, (i, a, v) -> read("mem:pending"))"#,
            r#"result = (new Listing { new Dynamic { x = read("mem:pending") } }).join(",")"#,
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
