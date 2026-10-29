//! Source-authored Host/GC invariants for TODO and Undefined.
//! These are lifecycle tests, not Java-derived diagnostic byte expectations.
use pkl_runtime::{
    host::{HostPoll, HostSession},
    parser::parse_module,
    resolver::resolve_module,
    value::{ErrorKind, RuntimeError, Value},
    vm::{Instruction as I, Program, compile_module},
};
use std::sync::Arc;

fn event(session: &mut HostSession) -> HostPoll {
    for _ in 0..50_000 {
        session.collect(&[]);
        match session.poll(0, 64) {
            HostPoll::Yielded => {}
            event => return event,
        }
    }
    panic!("base failure lifecycle poll bound")
}

fn ready(session: &mut HostSession) -> Result<Value, RuntimeError> {
    match event(session) {
        HostPoll::Ready(result) => result,
        event => panic!("expected terminal invocation result, got {event:?}"),
    }
}

fn read(session: &mut HostSession, expected: &str) -> String {
    match event(session) {
        HostPoll::Read { token, uri } => {
            assert_eq!(uri.as_ref(), expected);
            session.collect(&[]);
            token
        }
        event => panic!("expected read {expected}, got {event:?}"),
    }
}

fn program(source: &str, names: &[&str]) -> (Arc<Program>, Vec<usize>) {
    let resolved = resolve_module(&parse_module(source).unwrap()).unwrap();
    let mut program = (*compile_module(&resolved).unwrap()).clone();
    // The appended blocks only enter source members, following the existing
    // inferred-new and Typed.hasProperty lifecycle test harnesses. The builtin,
    // strict read condition, default inference and toString are source-compiled.
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
    (Arc::new(program), entries)
}

fn boot(program: Arc<Program>) -> (HostSession, Value) {
    let entry = program.entry;
    let mut session = HostSession::new(program);
    session.start(entry, vec![], 0, None).unwrap();
    let module = ready(&mut session).unwrap();
    (session, module)
}

fn source(builtin: &str) -> String {
    r#"
outerValue = read("mem:outer")
payload = new {
  good = read("mem:cached")
  bad = if (good != "" && read("mem:gate")) BUILTIN() else good
}
rendered = payload.toString()
failed = payload.bad
saved = payload.good
fresh = if (read("mem:fresh")) BUILTIN() else payload.good
"#
    .replace("BUILTIN", builtin)
}

fn has_base_frame(error: &RuntimeError, builtin: &str) -> bool {
    let member = format!("#{builtin}");
    error.frames.iter().any(|frame| {
        frame.member.as_str() == Some(member.as_str())
            && frame
                .source
                .as_ref()
                .is_some_and(|source| source.uri.as_ref() == "pkl:base")
    })
}

fn authored_frame(error: &RuntimeError, source: &str, member: &str, expression: &str) {
    assert!(!error.frames.is_empty(), "{error:?}");
    assert!(
        error.frames.iter().any(|frame| {
            frame.source.is_none()
                && frame.member.as_str() == Some(member)
                && source.get(frame.span.start..frame.span.end) == Some(expression)
        }),
        "missing authored {member} frame for {expression:?}: {error:?}"
    );
}

fn builtin_frames(error: &RuntimeError, builtin: &str, caller: &str) {
    // Java keeps TODO's authored throw, but Undefined's external body has no
    // available source section. Its authored call site must still be visible.
    assert_eq!(
        has_base_frame(error, builtin),
        builtin == "TODO",
        "{error:?}"
    );
    authored_frame(error, &source(builtin), caller, &format!("{builtin}()"));
}

fn builtin_error(result: Result<Value, RuntimeError>, builtin: &str, caller: &str) -> RuntimeError {
    let error = result.unwrap_err();
    assert_eq!(error.kind, ErrorKind::Type);
    builtin_frames(&error, builtin, caller);
    error
}

fn finish_closed(session: &mut HostSession) {
    session.close();
    assert!(session.is_closed());
    assert_eq!(session.context_depth(), 0);
    assert_eq!(session.collect(&[]).slots, 0);
}

#[test]
fn nested_force_failure_retries_keep_successful_dependencies_and_detached_errors() {
    for builtin in ["TODO", "Undefined"] {
        let (program, entries) = program(
            &source(builtin),
            &["outerValue", "rendered", "failed", "saved"],
        );
        let (mut session, module) = boot(program);
        session
            .start(entries[0], vec![module.clone()], 0, None)
            .unwrap();
        let outer = read(&mut session, "mem:outer");
        session
            .reenter(&outer, entries[1], vec![module.clone()], 0, None)
            .unwrap();
        let cached = read(&mut session, "mem:cached");
        session
            .resume(&cached, Ok(Value::string("stable")), 0)
            .unwrap();
        let gate = read(&mut session, "mem:gate");

        // A third context reads the successfully published dependency while
        // the object's force and failed property's read remain suspended.
        session
            .reenter(&gate, entries[3], vec![module.clone()], 0, None)
            .unwrap();
        assert_eq!(ready(&mut session), Ok(Value::string("stable")));
        assert_eq!(read(&mut session, "mem:gate"), gate);
        session.resume(&gate, Ok(Value::Bool(true)), 0).unwrap();
        let rendered = ready(&mut session);
        if builtin == "Undefined" {
            // toString permits undefined members; it must not swallow TODO.
            assert!(matches!(rendered, Ok(Value::String(_))), "{rendered:?}");
        } else {
            builtin_error(rendered, builtin, "#payload.bad");
        }
        assert_eq!(read(&mut session, "mem:outer"), outer);

        session
            .reenter(&outer, entries[2], vec![module.clone()], 0, None)
            .unwrap();
        // The good member stays cached; the failed bad member runs again.
        let retry = read(&mut session, "mem:gate");
        assert_ne!(retry, gate);
        assert_eq!(
            session
                .resume(&gate, Ok(Value::Bool(false)), 0)
                .unwrap_err()
                .kind,
            ErrorKind::Host
        );
        assert_eq!(read(&mut session, "mem:gate"), retry);
        session.resume(&retry, Ok(Value::Bool(true)), 0).unwrap();
        let detached = builtin_error(ready(&mut session), builtin, "#payload.bad");
        let snapshot = detached.clone();
        assert_eq!(read(&mut session, "mem:outer"), outer);

        session
            .reenter(&outer, entries[3], vec![module.clone()], 0, None)
            .unwrap();
        assert_eq!(ready(&mut session), Ok(Value::string("stable")));
        assert_eq!(read(&mut session, "mem:outer"), outer);
        session
            .reenter(&outer, entries[2], vec![module.clone()], 0, None)
            .unwrap();
        let recovered = read(&mut session, "mem:gate");
        session
            .resume(&recovered, Ok(Value::Bool(false)), 0)
            .unwrap();
        assert_eq!(ready(&mut session), Ok(Value::string("stable")));
        assert_eq!(read(&mut session, "mem:outer"), outer);
        session.resume(&outer, Ok(Value::Null), 0).unwrap();
        assert_eq!(ready(&mut session), Ok(Value::Null));
        assert_eq!(session.context_depth(), 0);
        finish_closed(&mut session);
        drop(session);
        assert_eq!(detached, snapshot);
        builtin_frames(&detached, builtin, "#payload.bad");
    }
}

#[test]
fn inferred_parent_conversion_precedes_allow_undefined_force_and_retry_recovers() {
    for builtin in ["TODO", "Undefined"] {
        let source = r#"
class Parent {
  value = if (read("mem:prototype")) BUILTIN() else new Dynamic { marker = 7 }
}
amended = new Parent { value = new { child = 9 } }
rendered = amended.toString()
recovered = amended.value.marker + amended.value.child
"#
        .replace("BUILTIN", builtin);
        let (program, entries) = program(&source, &["rendered", "recovered"]);
        let (mut session, module) = boot(program);
        session
            .start(entries[0], vec![module.clone()], 0, None)
            .unwrap();
        let prototype = read(&mut session, "mem:prototype");
        session
            .resume(&prototype, Ok(Value::Bool(true)), 0)
            .unwrap();
        let error = ready(&mut session).unwrap_err();
        assert_eq!(error.kind, ErrorKind::Type);
        // Undefined becomes an inferred-parent error before object.toString
        // can swallow it; TODO remains the original authored base failure.
        assert_eq!(has_base_frame(&error, builtin), builtin == "TODO");
        assert!(!error.frames.is_empty());
        if builtin == "Undefined" {
            // Replacing an unavailable external origin must preserve the new
            // authored origin at the inferred-parent boundary.
            authored_frame(&error, &source, "#amended.value", "new");
        } else {
            authored_frame(&error, &source, "#Parent.value", "TODO()");
        }
        assert!(!session.is_closed());
        session.start(entries[1], vec![module], 0, None).unwrap();
        let retry = read(&mut session, "mem:prototype");
        assert_ne!(retry, prototype);
        session.resume(&retry, Ok(Value::Bool(false)), 0).unwrap();
        assert_eq!(ready(&mut session), Ok(Value::Int(16.into())));
        finish_closed(&mut session);
    }
}

#[test]
fn cancellation_and_close_abort_retry_and_parent_tokens_without_retaining_errors() {
    for builtin in ["TODO", "Undefined"] {
        for cancel in [false, true] {
            let (program, entries) = program(&source(builtin), &["outerValue", "failed"]);
            let (mut session, module) = boot(program);
            session
                .start(entries[0], vec![module.clone()], 0, None)
                .unwrap();
            let outer = read(&mut session, "mem:outer");
            session
                .reenter(&outer, entries[1], vec![module.clone()], 0, None)
                .unwrap();
            let cached = read(&mut session, "mem:cached");
            session
                .resume(&cached, Ok(Value::string("stable")), 0)
                .unwrap();
            let gate = read(&mut session, "mem:gate");
            session.resume(&gate, Ok(Value::Bool(true)), 0).unwrap();
            let detached = builtin_error(ready(&mut session), builtin, "#payload.bad");
            let snapshot = detached.clone();
            assert_eq!(read(&mut session, "mem:outer"), outer);
            session
                .reenter(&outer, entries[1], vec![module.clone()], 0, None)
                .unwrap();
            let retry = read(&mut session, "mem:gate");
            assert_eq!(session.context_depth(), 2);
            if cancel {
                session.cancel();
            } else {
                session.close();
            }
            assert_eq!(
                ready(&mut session).unwrap_err().kind,
                if cancel {
                    ErrorKind::Cancelled
                } else {
                    ErrorKind::Closed
                }
            );
            assert_eq!(
                session.drain_cancelled_reads(),
                vec![outer.clone(), retry.clone()]
            );
            for token in [&outer, &retry, &gate] {
                assert_eq!(
                    session
                        .resume(token, Ok(Value::Bool(false)), 0)
                        .unwrap_err()
                        .kind,
                    ErrorKind::Closed
                );
            }
            assert_eq!(
                session
                    .start(entries[1], vec![module], 0, None)
                    .unwrap_err()
                    .kind,
                ErrorKind::Closed
            );
            finish_closed(&mut session);
            assert!(session.drain_cancelled_reads().is_empty());
            drop(session);
            assert_eq!(detached, snapshot);
            builtin_frames(&detached, builtin, "#payload.bad");
        }
    }
}

#[test]
fn shared_compiled_program_keeps_failure_retries_tokens_and_caches_evaluator_local() {
    for builtin in ["TODO", "Undefined"] {
        let (program, entries) = program(&source(builtin), &["failed", "saved", "fresh"]);
        let (mut left, left_module) = boot(program.clone());
        let (mut right, right_module) = boot(program);
        left.start(entries[0], vec![left_module.clone()], 0, None)
            .unwrap();
        right
            .start(entries[0], vec![right_module.clone()], 0, None)
            .unwrap();
        let left_cached = read(&mut left, "mem:cached");
        let right_cached = read(&mut right, "mem:cached");
        left.resume(&left_cached, Ok(Value::string("left")), 0)
            .unwrap();
        right
            .resume(&right_cached, Ok(Value::string("right")), 0)
            .unwrap();
        let left_gate = read(&mut left, "mem:gate");
        let right_gate = read(&mut right, "mem:gate");
        assert_ne!(left_gate, right_gate);
        assert_eq!(
            right
                .resume(&left_gate, Ok(Value::Bool(true)), 0)
                .unwrap_err()
                .kind,
            ErrorKind::Host
        );
        assert_eq!(read(&mut right, "mem:gate"), right_gate);
        left.resume(&left_gate, Ok(Value::Bool(true)), 0).unwrap();
        let detached = builtin_error(ready(&mut left), builtin, "#payload.bad");
        right
            .resume(&right_gate, Ok(Value::Bool(false)), 0)
            .unwrap();
        assert_eq!(ready(&mut right), Ok(Value::string("right")));
        left.start(entries[0], vec![left_module], 0, None).unwrap();
        let left_retry = read(&mut left, "mem:gate");
        left.resume(&left_retry, Ok(Value::Bool(false)), 0).unwrap();
        assert_eq!(ready(&mut left), Ok(Value::string("left")));
        finish_closed(&mut left);

        // Closing one evaluator and retaining its failure must not destroy the
        // second evaluator's source-backed builtin executable or member cache.
        right
            .start(entries[1], vec![right_module.clone()], 0, None)
            .unwrap();
        assert_eq!(ready(&mut right), Ok(Value::string("right")));
        right
            .start(entries[2], vec![right_module], 0, None)
            .unwrap();
        let fresh = read(&mut right, "mem:fresh");
        right.resume(&fresh, Ok(Value::Bool(true)), 0).unwrap();
        builtin_error(ready(&mut right), builtin, "#fresh");
        builtin_frames(&detached, builtin, "#payload.bad");
        finish_closed(&mut right);
    }
}
