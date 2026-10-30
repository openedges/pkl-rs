//! Native continuation and ownership checks; Java byte observations are separate
//! in m2_any_if_non_null.rs. Every suspension and cooperative yield collects.
use std::sync::Arc;

use pkl_runtime::{
    heap::LazyState,
    host::{HostPoll, HostSession},
    parser::parse_module,
    resolver::resolve_module,
    value::{ErrorKind, RuntimeError, Value},
    vm::{Instruction as I, Program, Vm, VmPoll, compile_module},
};

const SOURCE: &str = r#"
function capture(prefix) = (x) -> read("mem:callback").ifNonNull((suffix) -> prefix + ":" + x.value + ":" + suffix)
parent = read("mem:outer")
child = new Dynamic { value = "receiver"; unused = throw("unused") }.ifNonNull(capture("captured"))
inspect = ((Null(new Dynamic { value = "default" }).ifNonNull((x) -> throw("skipped"))) { extra = 7 }).value
"#;

fn program(source: &str) -> (Arc<Program>, [usize; 3]) {
    let mut program = (*compile_module(&resolve_module(&parse_module(source).unwrap()).unwrap())
        .unwrap())
    .clone();
    let entries = ["parent", "child", "inspect"].map(|name| {
        let block = program.blocks.len();
        program
            .blocks
            .push(vec![I::Input(0), I::GetMember(name.into()), I::Return]);
        block
    });
    (Arc::new(program), entries)
}

fn vm_event(vm: &mut Vm) -> VmPoll {
    for _ in 0..200_000 {
        match vm.poll(1) {
            VmPoll::Yielded => {
                vm.collect(&[]);
            }
            event => return event,
        }
    }
    panic!("Any.ifNonNull VM event budget")
}

fn vm_ready(vm: &mut Vm) -> Result<Value, RuntimeError> {
    match vm_event(vm) {
        VmPoll::Ready(result) => result,
        event => panic!("expected VM result, got {event:?}"),
    }
}

fn vm_read(vm: &mut Vm, uri: &str) -> u64 {
    match vm_event(vm) {
        VmPoll::Pending(read) => {
            assert_eq!(read.uri, uri);
            vm.collect(&[]);
            read.id
        }
        event => panic!("expected read {uri}, got {event:?}"),
    }
}

fn host_event(session: &mut HostSession, now: u64) -> HostPoll {
    for _ in 0..200_000 {
        match session.poll(now, 1) {
            HostPoll::Yielded => {
                session.collect(&[]);
            }
            event => return event,
        }
    }
    panic!("Any.ifNonNull Host event budget")
}

fn host_ready(session: &mut HostSession, now: u64) -> Result<Value, RuntimeError> {
    match host_event(session, now) {
        HostPoll::Ready(result) => result,
        event => panic!("expected Host result, got {event:?}"),
    }
}

fn host_read(session: &mut HostSession, now: u64, uri: &str) -> String {
    match host_event(session, now) {
        HostPoll::Read { token, uri: actual } => {
            assert_eq!(actual, uri);
            session.collect(&[]);
            token
        }
        event => panic!("expected read {uri}, got {event:?}"),
    }
}

fn setup(source: &str) -> (HostSession, Value, [usize; 3]) {
    let (program, entries) = program(source);
    let entry = program.entry;
    let mut session = HostSession::new(program);
    session.start(entry, vec![], 0, None).unwrap();
    let module = host_ready(&mut session, 0).unwrap();
    (session, module, entries)
}

#[test]
fn null_default_identity_and_lazy_payload_survive_eager_argument_suspension() {
    for (default, apply, expected) in [
        (
            r#"new Dynamic { payload = read("mem:default") }"#,
            "",
            "preserved",
        ),
        (
            r#"let (prefix = "captured:") () -> new Dynamic { payload = prefix + read("mem:default") }"#,
            ".apply()",
            "captured:preserved",
        ),
    ] {
        let source = format!(
            r#"
original = Null({default})
parent = original
child = original.ifNonNull(let (ignored = read("mem:argument")) (x) -> throw("skipped"))
inspect = ((child) {{ extra = 7 }}){apply}.payload
"#
        );
        let (program, [parent, child, inspect]) = program(&source);
        let mut vm = Vm::new(program.clone());
        vm.start(program.entry, vec![]).unwrap();
        let module = vm_ready(&mut vm).unwrap();
        vm.start(parent, vec![module.clone()]).unwrap();
        let original = vm_ready(&mut vm).unwrap();
        let Value::NullWithDefault(original_default) = original else {
            panic!("attached default")
        };
        vm.start(child, vec![module.clone()]).unwrap();
        let argument = vm_read(&mut vm, "mem:argument");
        vm.resume(argument, Ok(Value::string("checked"))).unwrap();
        let returned = vm_ready(&mut vm).unwrap();
        let Value::NullWithDefault(returned_default) = returned else {
            panic!("retained default")
        };
        assert_eq!(original_default, returned_default);
        vm.collect(std::slice::from_ref(&module));
        vm.start(inspect, vec![module]).unwrap();
        let payload = vm_read(&mut vm, "mem:default");
        vm.resume(payload, Ok(Value::string("preserved"))).unwrap();
        assert_eq!(vm_ready(&mut vm), Ok(Value::string(expected)));
        vm.close();
        assert_eq!(vm.heap_stats().slots, 0);
    }
}

#[test]
fn captured_callback_and_temporary_receiver_survive_reentry_failure_and_retry() {
    let (mut session, module, [parent, child, inspect]) = setup(SOURCE);
    session
        .start(parent, vec![module.clone()], 1, None)
        .unwrap();
    let outer = host_read(&mut session, 1, "mem:outer");
    session
        .reenter(&outer, child, vec![module.clone()], 2, None)
        .unwrap();
    let failed = host_read(&mut session, 2, "mem:callback");
    session
        .reenter(&failed, inspect, vec![module.clone()], 3, None)
        .unwrap();
    assert_eq!(host_ready(&mut session, 3), Ok(Value::string("default")));
    assert_eq!(host_read(&mut session, 3, "mem:callback"), failed);
    let error = RuntimeError::new(ErrorKind::Host, "callback failure\0\r\n🙂");
    session.resume(&failed, Err(error.clone()), 4).unwrap();
    let actual = host_ready(&mut session, 4).unwrap_err();
    assert_eq!((actual.kind, actual.message), (error.kind, error.message));
    assert_eq!(host_read(&mut session, 4, "mem:outer"), outer);
    session
        .reenter(&outer, child, vec![module.clone()], 5, None)
        .unwrap();
    let retry = host_read(&mut session, 5, "mem:callback");
    assert_ne!(failed, retry);
    assert_eq!(
        session
            .resume(&failed, Ok(Value::Null), 5)
            .unwrap_err()
            .kind,
        ErrorKind::Host
    );
    session
        .resume(&retry, Ok(Value::string("resumed")), 5)
        .unwrap();
    assert_eq!(
        host_ready(&mut session, 5),
        Ok(Value::string("captured:receiver:resumed"))
    );
    assert_eq!(host_read(&mut session, 5, "mem:outer"), outer);
    session.resume(&outer, Ok(Value::Null), 6).unwrap();
    assert_eq!(host_ready(&mut session, 6), Ok(Value::Null));
    session.start(child, vec![module], 7, None).unwrap();
    assert_eq!(
        host_ready(&mut session, 7),
        Ok(Value::string("captured:receiver:resumed"))
    );
    session.close();
    assert_eq!(session.heap_stats().slots, 0);
}

#[test]
fn function_class_and_alias_receivers_keep_their_executable_metadata() {
    for (receiver, result) in [
        (
            "let (prefix = \"closure\") (x) -> prefix + x",
            "x.apply(suffix)",
        ),
        ("C", "x.simpleName + suffix"),
        (
            "Alias",
            "if (x == Alias) suffix else throw(\"alias identity\")",
        ),
    ] {
        let source = format!(
            r#"
class C {{ value = 7 }}
typealias Alias = C
parent = read("mem:outer")
child = ({receiver}).ifNonNull((x) -> let (suffix = read("mem:callback")) {result})
inspect = 7
"#
        );
        let (mut session, module, [_, child, _]) = setup(&source);
        session.start(child, vec![module], 1, None).unwrap();
        let token = host_read(&mut session, 1, "mem:callback");
        session.resume(&token, Ok(Value::string("!")), 2).unwrap();
        let expected = if receiver.starts_with("let") {
            "closure!"
        } else if receiver == "C" {
            "C!"
        } else {
            "!"
        };
        assert_eq!(host_ready(&mut session, 2), Ok(Value::string(expected)));
        session.close();
        assert_eq!(session.heap_stats().slots, 0);
    }
}

#[test]
fn cancellation_reclaims_temporary_receivers_and_retries_without_stale_tokens() {
    let (program, [_, child, _]) = program(SOURCE);
    let mut vm = Vm::new(program.clone());
    vm.start(program.entry, vec![]).unwrap();
    let module = vm_ready(&mut vm).unwrap();
    let mut retained = None;
    for _ in 0..24 {
        vm.start(child, vec![module.clone()]).unwrap();
        let token = vm_read(&mut vm, "mem:callback");
        vm.cancel();
        assert_eq!(vm_ready(&mut vm).unwrap_err().kind, ErrorKind::Cancelled);
        assert_eq!(
            vm.member_state(module.as_object().unwrap(), "child")
                .unwrap(),
            LazyState::Unevaluated
        );
        assert_eq!(
            vm.resume(token, Ok(Value::Null)).unwrap_err().kind,
            ErrorKind::Host
        );
        let stats = vm.collect(std::slice::from_ref(&module));
        let current = (stats.live, stats.executable_live);
        if let Some(previous) = retained {
            assert_eq!(current, previous);
        }
        retained = Some(current);
    }
    vm.start(child, vec![module]).unwrap();
    let token = vm_read(&mut vm, "mem:callback");
    vm.resume(token, Ok(Value::string("retry"))).unwrap();
    assert_eq!(
        vm_ready(&mut vm),
        Ok(Value::string("captured:receiver:retry"))
    );
    vm.close();
    assert_eq!(vm.heap_stats().slots, 0);
}

#[test]
fn terminal_nested_calls_release_all_roots_and_keep_evaluators_isolated() {
    for kind in [ErrorKind::Cancelled, ErrorKind::Closed, ErrorKind::Timeout] {
        let (mut session, module, [parent, child, _]) = setup(SOURCE);
        let (mut other, other_module, [_, other_child, _]) = setup(SOURCE);
        other
            .start(other_child, vec![other_module], 1, None)
            .unwrap();
        let independent = host_read(&mut other, 1, "mem:callback");
        session
            .start(
                parent,
                vec![module.clone()],
                1,
                (kind == ErrorKind::Timeout).then_some(5),
            )
            .unwrap();
        let outer = host_read(&mut session, 1, "mem:outer");
        session
            .reenter(&outer, child, vec![module], 2, None)
            .unwrap();
        let inner = host_read(&mut session, 2, "mem:callback");
        assert_eq!(
            session
                .resume(&independent, Ok(Value::Null), 2)
                .unwrap_err()
                .kind,
            ErrorKind::Host
        );
        match kind {
            ErrorKind::Cancelled => session.cancel(),
            ErrorKind::Closed => session.close(),
            ErrorKind::Timeout => {}
            _ => unreachable!(),
        }
        let now = if kind == ErrorKind::Timeout { 6 } else { 3 };
        assert_eq!(host_ready(&mut session, now).unwrap_err().kind, kind);
        assert_eq!(session.context_depth(), 0);
        assert_eq!(session.heap_stats().slots, 0);
        assert_eq!(
            session.drain_cancelled_reads(),
            [outer.clone(), inner.clone()]
        );
        for token in [outer, inner] {
            assert_eq!(
                session
                    .resume(&token, Ok(Value::Null), now)
                    .unwrap_err()
                    .kind,
                ErrorKind::Closed
            );
        }
        other
            .resume(&independent, Ok(Value::string("independent")), now)
            .unwrap();
        assert_eq!(
            host_ready(&mut other, now),
            Ok(Value::string("captured:receiver:independent"))
        );
        other.close();
        assert_eq!(other.heap_stats().slots, 0);
    }
}

#[test]
fn escaped_transformer_result_retains_the_temporary_receiver_and_capture() {
    let source = r#"
function transform(prefix) = (x) -> (suffix) -> prefix + x.value + suffix
parent = read("mem:outer")
child = new Dynamic { value = "receiver" }.ifNonNull(transform("captured:"))
inspect = child.apply(read("mem:callback"))
"#;
    let (mut session, module, [_, child, inspect]) = setup(source);
    session.start(child, vec![module.clone()], 1, None).unwrap();
    let closure = host_ready(&mut session, 1).unwrap();
    assert!(matches!(closure, Value::Function(_)));
    session.collect(&[module.clone(), closure]);
    session.start(inspect, vec![module], 2, None).unwrap();
    let token = host_read(&mut session, 2, "mem:callback");
    session
        .resume(&token, Ok(Value::string(":escaped")), 3)
        .unwrap();
    assert_eq!(
        host_ready(&mut session, 3),
        Ok(Value::string("captured:receiver:escaped"))
    );
    session.close();
    assert_eq!(session.heap_stats().slots, 0);
}
