//! Equality and structural-key continuations use the shared Host/heap lifecycle.
//! Expected forcing gates come from pinned VmObject/VmListing/VmDynamic sources;
//! these Host callback checks are lifecycle tests, not Java byte comparisons.
use std::sync::Arc;

use pkl_runtime::host::{HostPoll, HostSession};
use pkl_runtime::parser::parse_module;
use pkl_runtime::resolver::resolve_module;
use pkl_runtime::value::{ErrorKind, RuntimeError, Value};
use pkl_runtime::vm::{Instruction as I, compile_module};

fn session(parent: &str, child: &str) -> (HostSession, Value, usize, usize) {
    let source = format!("parent = {parent}\nchild = {child}");
    session_source(&source)
}

fn session_source(source: &str) -> (HostSession, Value, usize, usize) {
    let parsed = parse_module(source).unwrap();
    let mut program = (*compile_module(&resolve_module(&parsed).unwrap()).unwrap()).clone();
    let module_entry = program.entry;
    let entries = ["parent", "child"].map(|member| {
        let entry = program.blocks.len();
        program.blocks.push(vec![
            I::Input(0),
            I::GetMember(Arc::from(member)),
            I::Return,
        ]);
        entry
    });
    let mut session = HostSession::new(Arc::new(program));
    session.start(module_entry, vec![], 0, None).unwrap();
    let root = ready(&mut session, 0).unwrap();
    (session, root, entries[0], entries[1])
}

fn event(session: &mut HostSession, now: u64) -> HostPoll {
    loop {
        match session.poll(now, 1) {
            HostPoll::Yielded => {
                session.collect(&[]);
            }
            event => return event,
        }
    }
}

fn ready(session: &mut HostSession, now: u64) -> Result<Value, RuntimeError> {
    match event(session, now) {
        HostPoll::Ready(value) => value,
        event => panic!("expected ready, got {event:?}"),
    }
}

fn request(session: &mut HostSession, now: u64, expected: &str) -> String {
    match event(session, now) {
        HostPoll::Read { token, uri } => {
            assert_eq!(&*uri, expected);
            token
        }
        event => panic!("expected read {expected}, got {event:?}"),
    }
}

const PARENT: &str = r#"new Dynamic { first = new Dynamic { text = "keep"; check = read("mem:left") }; last = 1 } == new Dynamic { first = new Dynamic { text = "keep"; check = read("mem:right") }; last = 1 }"#;
const INSERT: &str = r#"new Mapping { [new Dynamic { text = read("mem:key") }] = 42 }[new Dynamic { text = "key" }]"#;
const LOOKUP: &str = r#"new Mapping { [new Dynamic { text = "key" }] = 42 }[new Dynamic { text = read("mem:key") }]"#;
const HASH_TRANSITION: &str = r#"let (mapping = new Dynamic { [new Dynamic { text = read("mem:key") }] = 0; ["a"] = 1; ["b"] = 2; ["c"] = 3; ["d"] = 4 }) 42"#;

#[test]
fn temporary_operands_and_structural_keys_survive_reentry_failure_retry_and_gc() {
    for child in [INSERT, LOOKUP, HASH_TRANSITION] {
        let (mut session, root, parent, child_entry) = session(PARENT, child);
        session.start(parent, vec![root.clone()], 1, None).unwrap();
        let outer = request(&mut session, 1, "mem:left");
        let mut failed_token: Option<String> = None;
        for success in [false, true] {
            session
                .reenter(&outer, child_entry, vec![root.clone()], 2, None)
                .unwrap();
            let inner = request(&mut session, 2, "mem:key");
            assert!(session.collect(&[]).live > 0);
            if let Some(stale) = &failed_token {
                assert_ne!(&inner, stale);
                assert_eq!(
                    session.resume(stale, Ok(Value::Null), 2).unwrap_err().kind,
                    ErrorKind::Host
                );
            }
            let failure = RuntimeError::new(ErrorKind::Host, "structural key read\0\r\n🙂");
            session
                .resume(
                    &inner,
                    if success {
                        Ok(Value::string("key"))
                    } else {
                        Err(failure.clone())
                    },
                    2,
                )
                .unwrap();
            let result = ready(&mut session, 2);
            if success {
                assert_eq!(result, Ok(Value::Int(42)));
            } else {
                let error = result.unwrap_err();
                assert_eq!((error.kind, error.message), (failure.kind, failure.message));
                failed_token = Some(inner);
            }
            assert_eq!(session.context_depth(), 1);
            assert_eq!(request(&mut session, 2, "mem:left"), outer);
        }
        session
            .resume(&outer, Ok(Value::string("same")), 3)
            .unwrap();
        let right = request(&mut session, 3, "mem:right");
        session.collect(&[]);
        session
            .resume(&right, Ok(Value::string("same")), 4)
            .unwrap();
        assert_eq!(ready(&mut session, 4), Ok(Value::Bool(true)));
        assert_eq!(session.collect(&[]).live, 0);
        assert!(session.drain_cancelled_reads().is_empty());
    }
}

#[test]
fn completed_left_forcing_survives_a_failed_right_and_is_reused_on_retry() {
    // Use named operands so successful left-member caches outlive the failed
    // comparison. A failed right force must not publish a comparison result.
    let source = r#"left = new Dynamic { text = read("mem:left") }
        right = new Dynamic { text = read("mem:right") }
        parent = left == right"#;
    let parsed = parse_module(source).unwrap();
    let mut program = (*compile_module(&resolve_module(&parsed).unwrap()).unwrap()).clone();
    let module_entry = program.entry;
    let parent = program.blocks.len();
    program.blocks.push(vec![
        I::Input(0),
        I::GetMember(Arc::from("parent")),
        I::Return,
    ]);
    let mut session = HostSession::new(Arc::new(program));
    session.start(module_entry, vec![], 0, None).unwrap();
    let root = ready(&mut session, 0).unwrap();
    session.start(parent, vec![root.clone()], 1, None).unwrap();
    let left = request(&mut session, 1, "mem:left");
    session.resume(&left, Ok(Value::string("same")), 1).unwrap();
    let right = request(&mut session, 1, "mem:right");
    session
        .resume(
            &right,
            Err(RuntimeError::new(ErrorKind::Host, "retry right")),
            1,
        )
        .unwrap();
    assert_eq!(ready(&mut session, 1).unwrap_err().kind, ErrorKind::Host);
    session.collect(std::slice::from_ref(&root));
    session.start(parent, vec![root], 2, None).unwrap();
    let retried = request(&mut session, 2, "mem:right");
    assert_ne!(retried, right);
    session
        .resume(&retried, Ok(Value::string("same")), 2)
        .unwrap();
    assert_eq!(ready(&mut session, 2), Ok(Value::Bool(true)));
    assert_eq!(session.collect(&[]).live, 0);
}

#[test]
fn key_comparison_forces_wrappers_with_temporary_declaration_receivers() {
    let source = r#"
        class Check {
          text = "key"
          uri = "unset"
          function values(): Listing<String(read(uri) == "ok")> = new Listing { text }
        }
        parent = new Check { uri = "mem:outer" }.values() == new Listing { "key" }
        child = new Mapping {
          [new Check { uri = "mem:key" }.values()] = 42
        }[new Listing { "key" }]
    "#;
    let (mut session, root, parent, child) = session_source(source);
    session.start(parent, vec![root.clone()], 1, None).unwrap();
    let outer = request(&mut session, 1, "mem:outer");
    session.reenter(&outer, child, vec![root], 2, None).unwrap();
    let inner = request(&mut session, 2, "mem:key");
    session.collect(&[]);
    session.resume(&inner, Ok(Value::string("ok")), 2).unwrap();
    assert_eq!(ready(&mut session, 2), Ok(Value::Int(42)));
    assert_eq!(request(&mut session, 2, "mem:outer"), outer);
    session.collect(&[]);
    session.resume(&outer, Ok(Value::string("ok")), 3).unwrap();
    assert_eq!(ready(&mut session, 3), Ok(Value::Bool(true)));
    assert_eq!(session.collect(&[]).live, 0);
}

#[test]
fn first_structural_key_and_identity_lookup_leave_lazy_members_unforced() {
    let source = r#"
        key = new Dynamic { text = read("mem:unreached") }
        parent = new Dynamic { [key] = 42 }[key]
        child = 0
    "#;
    let (mut session, root, parent, _) = session_source(source);
    session.start(parent, vec![root], 1, None).unwrap();
    assert_eq!(ready(&mut session, 1), Ok(Value::Int(42)));
    assert_eq!(session.collect(&[]).live, 0);
}

#[test]
fn structural_lookup_compares_query_before_stored_key_across_host_suspension() {
    let (mut session, root, parent, _) = session(
        r#"new Dynamic { [new Dynamic { text = read("mem:stored") }] = 42 }[new Dynamic { text = read("mem:query") }]"#,
        "0",
    );
    session.start(parent, vec![root], 1, None).unwrap();
    for expected in ["mem:query", "mem:stored"] {
        let token = request(&mut session, 1, expected);
        session.collect(&[]);
        session
            .resume(&token, Ok(Value::string("same")), 1)
            .unwrap();
    }
    assert_eq!(ready(&mut session, 1), Ok(Value::Int(42)));
    assert_eq!(session.collect(&[]).live, 0);
}

#[test]
fn inherited_key_comparison_follows_child_value_force_and_keeps_nested_keys_alive() {
    // VmObject.force reads the child value, then queries its cache with the
    // ancestor key before deciding whether the ancestor body is overridden.
    let source = r#"
        ancestor = new Dynamic {
          [new Dynamic { text = read("mem:ancestorKey") }] = throw("overridden body")
        }
        derived = (ancestor) {
          [new Dynamic { text = read("mem:childKey") }] = read("mem:childValue")
        }
        parent = derived == new Dynamic { [new Dynamic { text = "same" }] = "value" }
        child = 0
    "#;
    let (mut session, root, parent, _) = session_source(source);
    session.start(parent, vec![root], 1, None).unwrap();
    for (uri, value) in [
        ("mem:childValue", "value"),
        ("mem:ancestorKey", "same"),
        ("mem:childKey", "same"),
    ] {
        let token = request(&mut session, 1, uri);
        session.collect(&[]);
        session.resume(&token, Ok(Value::string(value)), 1).unwrap();
    }
    assert_eq!(ready(&mut session, 1), Ok(Value::Bool(true)));
    assert_eq!(session.collect(&[]).live, 0);
}

#[test]
fn object_deep_force_visits_nested_members_before_comparing_an_earlier_difference() {
    let (mut session, root, parent, _) = session(
        r#"new Dynamic { first=0; nested=new Dynamic { text=read("mem:nested") } } == new Dynamic { first=1; nested=new Dynamic { text="same" } }"#,
        "0",
    );
    session.start(parent, vec![root], 1, None).unwrap();
    let nested = request(&mut session, 1, "mem:nested");
    session.collect(&[]);
    session
        .resume(&nested, Ok(Value::string("same")), 2)
        .unwrap();
    assert_eq!(ready(&mut session, 2), Ok(Value::Bool(false)));
    assert_eq!(session.collect(&[]).live, 0);
}

#[test]
fn terminal_reentry_releases_comparison_and_key_state_and_cancels_reads() {
    for termination in ["close", "cancel", "ancestor timeout", "child timeout"] {
        let (mut session, root, parent, child) = session(PARENT, INSERT);
        session
            .start(
                parent,
                vec![root.clone()],
                1,
                (termination == "ancestor timeout").then_some(5),
            )
            .unwrap();
        let outer = request(&mut session, 1, "mem:left");
        session
            .reenter(
                &outer,
                child,
                vec![root.clone()],
                2,
                (termination == "child timeout").then_some(3),
            )
            .unwrap();
        let inner = request(&mut session, 2, "mem:key");
        assert!(session.collect(&[]).live > 0);
        let kind = match termination {
            "close" => {
                session.close();
                ErrorKind::Closed
            }
            "cancel" => {
                session.cancel();
                ErrorKind::Cancelled
            }
            "child timeout" => {
                assert_eq!(
                    session.resume(&inner, Ok(Value::Null), 6).unwrap_err().kind,
                    ErrorKind::Timeout
                );
                ErrorKind::Timeout
            }
            _ => ErrorKind::Timeout,
        };
        let error = ready(&mut session, 6).unwrap_err();
        assert_eq!(error.kind, kind, "{termination}");
        assert_eq!(session.context_depth(), 0);
        assert_eq!(session.heap_stats().live, 0);
        assert_eq!(session.heap_stats().slots, 0);
        assert_eq!(
            session.drain_cancelled_reads(),
            [outer.clone(), inner.clone()]
        );
        for stale in [&outer, &inner] {
            assert_eq!(
                session.resume(stale, Ok(Value::Null), 7).unwrap_err().kind,
                ErrorKind::Closed
            );
        }
        assert_eq!(
            session.start(parent, vec![root], 7, None).unwrap_err().kind,
            ErrorKind::Closed
        );
        session.close();
        session.cancel();
        assert_eq!(ready(&mut session, 7).unwrap_err(), error);
        assert!(session.drain_cancelled_reads().is_empty());
    }
}
