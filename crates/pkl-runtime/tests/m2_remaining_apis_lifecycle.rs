//! Shared VM lifecycle checks, distinct from the raw Java oracle regressions.
use pkl_runtime::{
    host::{HostPoll, HostSession},
    parser::parse_module,
    resolver::resolve_module,
    value::{ErrorKind, RuntimeError, Value},
    vm::{Instruction as I, compile_module},
};
use std::sync::Arc;

fn event(session: &mut HostSession, roots: &[Value], now: u64) -> HostPoll {
    for _ in 0..200_000 {
        session.collect(roots);
        let result = session.poll(now, 1);
        let mut live = roots.to_vec();
        if let HostPoll::Ready(Ok(value)) = &result {
            live.push(value.clone());
        }
        session.collect(&live);
        if !matches!(result, HostPoll::Yielded) {
            return result;
        }
    }
    panic!("remaining API lifecycle step bound")
}
fn ready(session: &mut HostSession, roots: &[Value]) -> Result<Value, RuntimeError> {
    match event(session, roots, 0) {
        HostPoll::Ready(result) => result,
        other => panic!("unexpected {other:?}"),
    }
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
fn member(session: &mut HostSession, module: &Value, entry: usize) -> Value {
    session.start(entry, vec![module.clone()], 0, None).unwrap();
    ready(session, std::slice::from_ref(module)).unwrap()
}

#[test]
fn list_get_or_null_preserves_identity_and_leaves_nested_values_lazy() {
    let (mut session, module, entries) = setup(
        r#"
hidden object = new Dynamic { value = read("mem:value") }
hidden values = List(object)
original = object
selected = values.getOrNull(0)
negative = values.getOrNull(-9223372036854775808)
large = values.getOrNull(9223372036854775807)
"#,
        &["original", "selected", "negative", "large"],
    );
    let original = member(&mut session, &module, entries[0]);
    let selected = member(&mut session, &module, entries[1]);
    assert_eq!(selected, original, "retained heap identity");
    assert_eq!(member(&mut session, &module, entries[2]), Value::Null);
    assert_eq!(member(&mut session, &module, entries[3]), Value::Null);
    session.collect(std::slice::from_ref(&selected));
    // The selected value survives after dropping its producer module/list.
    assert!(session.heap_stats().live > 0);
    session.close();
    assert_eq!(session.collect(&[]).slots, 0);
}

#[test]
fn set_rest_preserves_elements_and_weak_canonical_empty_identity() {
    let (mut session, module, entries) = setup(
        r#"
hidden retained = List(-31)
hidden original = Map(31, "first", retained, "second", 0, "third").keys
first = retained
rest = original.restOrNull
selected = rest.first
empty = Set()
removedSingleton = Set(retained).restOrNull
emptyRest = empty.restOrNull
"#,
        &[
            "first",
            "rest",
            "selected",
            "empty",
            "removedSingleton",
            "emptyRest",
        ],
    );
    let original = member(&mut session, &module, entries[0]);
    let rest = member(&mut session, &module, entries[1]);
    let selected = member(&mut session, &module, entries[2]);
    assert_eq!(selected, original, "Map.keys retained value identity");
    let empty = member(&mut session, &module, entries[3]);
    assert_eq!(member(&mut session, &module, entries[4]), empty);
    assert_eq!(member(&mut session, &module, entries[5]), Value::Null);
    session.collect(&[rest, selected]);
    session.close();
    assert_eq!(session.collect(&[]).slots, 0);
}

#[test]
fn set_rest_tasks_release_roots_on_cancel_timeout_and_close() {
    for terminal in ["cancel", "timeout", "close"] {
        let mut completed = false;
        // Exercise every yielded boundary, including both hash/equality passes
        // and each shared cache publication, without depending on task counts.
        for pause_after in 0..256 {
            let (mut session, module, entries) = setup(
                r#"
hidden source = Set(List(-31), List(1), List(2))
warm = source
result = source.restOrNull
"#,
                &["warm", "result"],
            );
            member(&mut session, &module, entries[0]);
            session.start(entries[1], vec![module], 0, Some(5)).unwrap();
            for _ in 0..pause_after {
                session.collect(&[]);
                match session.poll(0, 1) {
                    HostPoll::Yielded => (),
                    HostPoll::Ready(Ok(_)) => {
                        completed = true;
                        break;
                    }
                    other => panic!("unexpected Set rest event: {other:?}"),
                }
            }
            if completed {
                session.close();
                assert!(pause_after > 2);
                break;
            }
            session.collect(&[]);
            let expected = match terminal {
                "cancel" => {
                    session.cancel();
                    ErrorKind::Cancelled
                }
                "close" => {
                    session.close();
                    ErrorKind::Closed
                }
                _ => ErrorKind::Timeout,
            };
            let HostPoll::Ready(Err(error)) = event(&mut session, &[], 6) else {
                panic!("expected terminal {terminal} at boundary {pause_after}")
            };
            assert_eq!(error.kind, expected);
            assert_eq!(session.context_depth(), 0);
            assert_eq!(session.heap_stats().slots, 0);
            assert!(session.drain_cancelled_reads().is_empty());
        }
        assert!(
            completed,
            "Set rest did not complete within the boundary limit"
        );
    }
}

fn read(session: &mut HostSession, module: &Value, uri: &str) -> String {
    match event(session, std::slice::from_ref(module), 0) {
        HostPoll::Read { token, uri: actual } => {
            assert_eq!(&*actual, uri);
            token
        }
        other => panic!("expected {uri}: {other:?}"),
    }
}

const APPLY_SOURCE: &str = r#"
hidden object = new Dynamic { unused = throw("not forced") }
hidden box = new Dynamic {
  marker = "go"
  worker = (x: Dynamic) -> if (read("mem:gate") == this.marker) x else throw("gate")
}
original = object
result = box.worker.applyToList(List(object))
parent = read("mem:parent")
nested = read("mem:nested")
"#;

#[test]
fn function_apply_retains_capture_and_arguments_across_failure_reentry_and_gc() {
    let (mut session, module, entries) =
        setup(APPLY_SOURCE, &["original", "parent", "result", "nested"]);
    let original = member(&mut session, &module, entries[0]);
    session
        .start(entries[1], vec![module.clone()], 0, None)
        .unwrap();
    let parent = read(&mut session, &module, "mem:parent");
    session
        .reenter(&parent, entries[2], vec![module.clone()], 0, None)
        .unwrap();
    let failed = read(&mut session, &module, "mem:gate");
    let failure = RuntimeError::new(ErrorKind::Host, "apply retry\0\r\n🙂");
    session.resume(&failed, Err(failure.clone()), 0).unwrap();
    let actual = ready(&mut session, std::slice::from_ref(&module)).unwrap_err();
    assert_eq!(
        (actual.kind, actual.message),
        (failure.kind, failure.message)
    );
    assert_eq!(session.context_depth(), 1);
    session
        .reenter(&parent, entries[2], vec![module.clone()], 0, None)
        .unwrap();
    let gate = read(&mut session, &module, "mem:gate");
    assert_ne!(gate, failed);
    assert_eq!(
        session
            .resume(&failed, Ok(Value::Null), 0)
            .unwrap_err()
            .kind,
        ErrorKind::Host
    );
    session
        .reenter(&gate, entries[3], vec![module.clone()], 0, None)
        .unwrap();
    let nested = read(&mut session, &module, "mem:nested");
    session
        .resume(&nested, Ok(Value::Int(7.into())), 0)
        .unwrap();
    assert_eq!(
        ready(&mut session, std::slice::from_ref(&module)),
        Ok(Value::Int(7.into()))
    );
    assert_eq!(read(&mut session, &module, "mem:gate"), gate);
    session.resume(&gate, Ok(Value::string("go")), 0).unwrap();
    let result = ready(&mut session, std::slice::from_ref(&module)).unwrap();
    assert_eq!(result, original, "function argument heap identity");
    session.resume(&parent, Ok(Value::Null), 0).unwrap();
    assert_eq!(ready(&mut session, &[result]), Ok(Value::Null));
    session.close();
    assert_eq!(session.collect(&[]).slots, 0);
}

#[test]
fn function_apply_pending_body_is_released_by_all_terminal_transitions() {
    for terminal in ["cancel", "timeout", "close"] {
        let (mut session, module, entries) = setup(APPLY_SOURCE, &["parent", "result"]);
        session
            .start(entries[0], vec![module.clone()], 0, Some(5))
            .unwrap();
        let parent = read(&mut session, &module, "mem:parent");
        session
            .reenter(&parent, entries[1], vec![module.clone()], 0, None)
            .unwrap();
        let gate = read(&mut session, &module, "mem:gate");
        let expected = match terminal {
            "cancel" => {
                session.cancel();
                ErrorKind::Cancelled
            }
            "close" => {
                session.close();
                ErrorKind::Closed
            }
            _ => ErrorKind::Timeout,
        };
        let HostPoll::Ready(Err(error)) = event(&mut session, &[], 6) else {
            panic!("expected terminal {terminal}")
        };
        assert_eq!(error.kind, expected);
        assert_eq!(session.context_depth(), 0);
        assert_eq!(session.heap_stats().slots, 0);
        assert_eq!(session.drain_cancelled_reads(), [parent, gate]);
    }
}
