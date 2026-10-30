//! Native ownership/continuation tests; Java byte evidence lives in m2_bytes_values.
use pkl_runtime::{
    host::{HostPoll, HostSession},
    parser::parse_module,
    resolver::resolve_module,
    value::{ErrorKind, RuntimeError, Value},
    vm::{Instruction as I, Program, Vm, VmPoll, compile_module},
};
use std::sync::Arc;

fn ready(vm: &mut Vm) -> Value {
    ready_roots(vm, &[])
}
fn ready_roots(vm: &mut Vm, roots: &[Value]) -> Value {
    for _ in 0..200_000 {
        match vm.poll(1) {
            VmPoll::Yielded => {
                vm.collect(roots);
            }
            VmPoll::Ready(result) => return result.unwrap(),
            event => panic!("unexpected {event:?}"),
        }
    }
    panic!("Bytes VM poll bound")
}
fn event(session: &mut HostSession) -> HostPoll {
    for _ in 0..200_000 {
        match session.poll(0, 1) {
            HostPoll::Yielded => {
                session.collect(&[]);
            }
            event => return event,
        }
    }
    panic!("Bytes Host poll bound")
}
fn host_ready(session: &mut HostSession) -> Result<Value, RuntimeError> {
    match event(session) {
        HostPoll::Ready(result) => result,
        event => panic!("unexpected {event:?}"),
    }
}
fn read(session: &mut HostSession, expected: &str) -> String {
    match event(session) {
        HostPoll::Read { token, uri } => {
            assert_eq!(uri, expected);
            session.collect(&[]);
            token
        }
        event => panic!("unexpected {event:?}"),
    }
}
fn method(name: &str) -> Vec<I> {
    vec![
        I::Input(0),
        I::GetMethod(name.into()),
        I::Call {
            arity: 0,
            method: true,
        },
        I::Return,
    ]
}

#[test]
fn bytes_backlinks_trace_external_values_and_keep_evaluators_isolated() {
    let bytes = Value::Bytes([0, 127, 128, 255].into());
    let program = Arc::new(Program::new(vec![
        method("toList"),
        vec![I::Constant(Value::Null), I::Return],
        method("toBytes"),
    ]));
    let mut first = Vm::new(program.clone());
    let mut second = Vm::new(program);
    first.start(0, vec![bytes.clone()]).unwrap();
    let original = ready(&mut first);
    second.start(0, vec![bytes.clone()]).unwrap();
    let independent = ready(&mut second);
    assert_ne!(original.heap_id(), independent.heap_id());
    first.start(1, vec![]).unwrap();
    ready_roots(&mut first, std::slice::from_ref(&bytes));
    first.collect(std::slice::from_ref(&bytes));
    first.start(0, vec![bytes.clone()]).unwrap();
    assert_eq!(ready(&mut first).heap_id(), original.heap_id());
    first.start(2, vec![original.clone()]).unwrap();
    let converted = ready(&mut first);
    first.start(1, vec![]).unwrap();
    ready_roots(&mut first, std::slice::from_ref(&converted));
    first.collect(std::slice::from_ref(&converted));
    first.start(0, vec![converted]).unwrap();
    assert_eq!(ready(&mut first).heap_id(), original.heap_id());
    first.close();
    second.close();
    assert_eq!(first.heap_stats().slots, 0);
    assert_eq!(second.heap_stats().slots, 0);
}

#[test]
fn constant_bytes_cache_survives_collection_and_terminal_release() {
    let bytes = Value::Bytes([255].into());
    let program = Arc::new(Program::new(vec![
        vec![
            I::Constant(bytes),
            I::GetMethod("toList".into()),
            I::Call {
                arity: 0,
                method: true,
            },
            I::Return,
        ],
        vec![I::Constant(Value::Null), I::Return],
    ]));
    let mut vm = Vm::new(program);
    vm.start(0, vec![]).unwrap();
    let first = ready(&mut vm);
    vm.start(1, vec![]).unwrap();
    ready(&mut vm);
    vm.collect(&[]);
    vm.start(0, vec![]).unwrap();
    assert_eq!(ready(&mut vm).heap_id(), first.heap_id());
    vm.close();
    assert_eq!(vm.collect(&[]).slots, 0);
}

const SOURCE: &str = r#"
original = List(0, 127, 128, 255)
encoded = original.toBytes()
roundtrip = encoded.toList()
parent = read("mem:outer")
child = List(0, read("mem:bytes").length, 255).toBytes().ifNonNull((bytes) -> let (list = bytes.toList()) if (read("mem:callback") == "ok") list else bytes.toList()).toBytes()
inspect = encoded.hex
"#;
fn setup() -> (HostSession, Value, [usize; 6]) {
    let mut program = (*compile_module(&resolve_module(&parse_module(SOURCE).unwrap()).unwrap())
        .unwrap())
    .clone();
    let entries = [
        "original",
        "encoded",
        "roundtrip",
        "parent",
        "child",
        "inspect",
    ]
    .map(|name| {
        let index = program.blocks.len();
        program
            .blocks
            .push(vec![I::Input(0), I::GetMember(name.into()), I::Return]);
        index
    });
    let entry = program.entry;
    let mut session = HostSession::new(Arc::new(program));
    session.start(entry, vec![], 0, None).unwrap();
    let module = host_ready(&mut session).unwrap();
    (session, module, entries)
}

#[test]
fn original_list_backlink_and_heap_members_survive_collection() {
    let (mut session, module, [original, encoded, roundtrip, ..]) = setup();
    session
        .start(original, vec![module.clone()], 0, None)
        .unwrap();
    let list = host_ready(&mut session).unwrap();
    session
        .start(encoded, vec![module.clone()], 0, None)
        .unwrap();
    let bytes = host_ready(&mut session).unwrap();
    assert_eq!(bytes, Value::Bytes([0, 127, 128, 255].into()));
    session.collect(std::slice::from_ref(&module));
    session.start(roundtrip, vec![module], 0, None).unwrap();
    assert_eq!(host_ready(&mut session).unwrap().heap_id(), list.heap_id());
    session.close();
    assert_eq!(session.heap_stats().slots, 0);
}

#[test]
fn bytes_survive_host_reentry_failure_retry_and_pending_callback() {
    let (mut session, module, [_, _, _, parent, child, inspect]) = setup();
    session
        .start(parent, vec![module.clone()], 0, None)
        .unwrap();
    let outer = read(&mut session, "mem:outer");
    session
        .reenter(&outer, child, vec![module.clone()], 0, None)
        .unwrap();
    let input = read(&mut session, "mem:bytes");
    session.resume(&input, Ok(Value::string("ab")), 0).unwrap();
    let callback = read(&mut session, "mem:callback");
    session
        .reenter(&callback, inspect, vec![module.clone()], 0, None)
        .unwrap();
    assert_eq!(host_ready(&mut session), Ok(Value::string("007f80ff")));
    assert_eq!(read(&mut session, "mem:callback"), callback);
    session
        .resume(
            &callback,
            Err(RuntimeError::new(ErrorKind::Host, "retry Bytes")),
            0,
        )
        .unwrap();
    assert_eq!(host_ready(&mut session).unwrap_err().kind, ErrorKind::Host);
    assert_eq!(read(&mut session, "mem:outer"), outer);
    session
        .reenter(&outer, child, vec![module], 0, None)
        .unwrap();
    let retry = read(&mut session, "mem:bytes");
    assert_ne!(retry, input);
    session.resume(&retry, Ok(Value::string("abc")), 0).unwrap();
    let callback = read(&mut session, "mem:callback");
    session
        .resume(&callback, Ok(Value::string("ok")), 0)
        .unwrap();
    assert_eq!(
        host_ready(&mut session),
        Ok(Value::Bytes([0, 3, 255].into()))
    );
    assert_eq!(read(&mut session, "mem:outer"), outer);
    session.resume(&outer, Ok(Value::Null), 0).unwrap();
    assert_eq!(host_ready(&mut session), Ok(Value::Null));
    session.close();
    assert_eq!(session.heap_stats().slots, 0);
}

#[test]
fn pending_bytes_callbacks_cancel_and_release_without_cross_evaluator_effects() {
    for close in [false, true] {
        let (mut first, module, [_, _, _, _, child, _]) = setup();
        first.start(child, vec![module], 0, None).unwrap();
        let input = read(&mut first, "mem:bytes");
        first.resume(&input, Ok(Value::string("abc")), 0).unwrap();
        let pending = read(&mut first, "mem:callback");
        if close {
            first.close();
        } else {
            first.cancel();
        }
        let expected = if close {
            ErrorKind::Closed
        } else {
            ErrorKind::Cancelled
        };
        assert_eq!(host_ready(&mut first).unwrap_err().kind, expected);
        assert_eq!(first.heap_stats().slots, 0);
        assert_eq!(
            first.resume(&pending, Ok(Value::Null), 0).unwrap_err().kind,
            ErrorKind::Closed
        );
        let (mut second, module, [_, _, _, _, _, inspect]) = setup();
        second.start(inspect, vec![module], 0, None).unwrap();
        assert_eq!(host_ready(&mut second), Ok(Value::string("007f80ff")));
        second.close();
        assert_eq!(second.heap_stats().slots, 0);
    }
}
