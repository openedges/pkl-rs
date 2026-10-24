//! Wrapper continuation roots and Host lifecycle, separate from Java resource semantics.
//! Retained module graphs include their one builtin Module parent; executable
//! nodes, transient-object deltas and terminal-zero assertions remain separate.
#[path = "support/constructor_baseline.rs"]
mod constructor_baseline;
use constructor_baseline::compile_module_with_constructor_baseline;
use std::sync::Arc;
#[path = "support/eager_base.rs"]
mod eager_base;
use eager_base::Baseline;

use pkl_runtime::host::{HostPoll, HostSession};
use pkl_runtime::parser::parse_module;
use pkl_runtime::resolver::resolve_module;
use pkl_runtime::value::{ErrorKind, RuntimeError, Value};
use pkl_runtime::vm::Instruction as I;

fn session() -> (HostSession, Value, usize, usize) {
    // Each method returns a wrapper that outlives its checking frame. Its
    // temporary class receiver supplies the predicate's lexical properties.
    // The nested List additionally has to retain the newly wrapped element.
    // Each explicit virtual method site adds one persistent executable owner;
    // its class target never roots the temporary Check instance.
    let parsed = parse_module(
        r#"
        class Check {
          text = "unset"
          uri = "unset"
          function listing(): Listing<String(read(uri) == text && this == text)> =
            new Listing { text }
          function mapping(): List<Mapping<String, String(read(uri) == text && this == text)>> =
            List(new Mapping { ["key"] = text })
        }
        outerResult = new Check { text = "outer"; uri = "mem:outer" }.listing()[0]
        innerResult = new Check { text = "inner"; uri = "mem:inner" }.mapping()[0]["key"]
        "#,
    )
    .unwrap();
    let mut program =
        (*compile_module_with_constructor_baseline(&resolve_module(&parsed).unwrap()).unwrap())
            .clone();
    let module_entry = program.entry;
    let mut entries = Vec::new();
    for member in ["outerResult", "innerResult"] {
        entries.push(program.blocks.len());
        program.blocks.push(vec![
            I::Input(0),
            I::GetMember(Arc::from(member)),
            I::Return,
        ]);
    }
    let mut session = HostSession::new(Arc::new(program));
    session.start(module_entry, vec![], 0, None).unwrap();
    let root = ready(&mut session, 0).unwrap();
    (session, root, entries[0], entries[1])
}

fn next_event(session: &mut HostSession, now: u64) -> HostPoll {
    loop {
        match session.poll(now, 2) {
            HostPoll::Yielded => {
                session.collect(&[]);
            }
            event => return event,
        }
    }
}

fn request(session: &mut HostSession, now: u64, expected_uri: &str) -> String {
    match next_event(session, now) {
        HostPoll::Read { token, uri } => {
            assert_eq!(&*uri, expected_uri);
            token
        }
        event => panic!("expected {expected_uri}, got {event:?}"),
    }
}

fn ready(session: &mut HostSession, now: u64) -> Result<Value, RuntimeError> {
    match next_event(session, now) {
        HostPoll::Ready(result) => result,
        event => panic!("expected completed context, got {event:?}"),
    }
}

#[test]
fn suspended_parent_constraint_keeps_reentry_eager_until_it_finishes() {
    // Java's shouldEagerTypecheck is dynamic evaluator state. Reentry through
    // a Host callback stays within the suspended predicate's dynamic extent.
    let parsed = parse_module(
        r#"
        parentResult: Int(read("mem:outer") == "ok") = 1
        checked: Listing<Int> = new Listing { 1; "bad" }
        childResult = checked.length
        "#,
    )
    .unwrap();
    let mut program =
        (*compile_module_with_constructor_baseline(&resolve_module(&parsed).unwrap()).unwrap())
            .clone();
    let module_entry = program.entry;
    let mut entries = Vec::new();
    for member in ["parentResult", "childResult"] {
        entries.push(program.blocks.len());
        program.blocks.push(vec![
            I::Input(0),
            I::GetMember(Arc::from(member)),
            I::Return,
        ]);
    }
    let mut session = HostSession::new(Arc::new(program));
    session.start(module_entry, vec![], 0, None).unwrap();
    let root = ready(&mut session, 0).unwrap();
    let initialized = Baseline::new(session.collect(&[]));
    let stale_root = root.clone();
    session
        .start(entries[0], vec![root.clone()], 1, None)
        .unwrap();
    let outer = request(&mut session, 1, "mem:outer");
    session
        .reenter(&outer, entries[1], vec![root.clone()], 2, None)
        .unwrap();
    assert_eq!(ready(&mut session, 2).unwrap_err().kind, ErrorKind::Type);
    assert_eq!(session.context_depth(), 1);
    assert_eq!(request(&mut session, 2, "mem:outer"), outer);
    session.collect(std::slice::from_ref(&root));
    session.resume(&outer, Ok(Value::string("ok")), 3).unwrap();
    assert_eq!(ready(&mut session, 3), Ok(Value::Int(1.into())));
    // After the parent constraint exits, the failed child property's retry
    // creates a lazy wrapper and length does not force the invalid element.
    session.start(entries[1], vec![root], 4, None).unwrap();
    assert_eq!(ready(&mut session, 4), Ok(Value::Int(2.into())));
    assert_eq!(initialized.added_values(session.collect(&[]), 3), -2);
    assert_eq!(
        session
            .start(module_entry, vec![stale_root], 10, None)
            .unwrap_err()
            .kind,
        ErrorKind::StaleHandle
    );
    session.close();
    assert_eq!(session.heap_stats().slots, 0);
    assert!(session.drain_cancelled_reads().is_empty());
}

#[test]
fn parameter_and_let_bindings_publish_checked_wrappers_before_the_body_runs() {
    for (index, source) in [
        r#"function consume(prefix, values: Listing<String(read("mem:parameter") == "ok")>) = prefix + values[0]
            result = consume("prefix:", new Listing { "value" })"#,
        r#"function consume(prefix, values: List<Mapping<String, String(read("mem:parameter") == "ok")>>) = prefix + values[0]["key"]
            result = consume("prefix:", List(new Mapping { ["key"] = "value" }))"#,
        r#"result = let (values: Listing<String(read("mem:parameter") == "ok")> = new Listing { "value" }) "prefix:" + values[0]"#,
        r#"f = (prefix, values: Listing<String(read("mem:parameter") == "ok")>) -> prefix + values[0]
            result = f.apply("prefix:", new Listing { "value" })"#,
    ].into_iter().enumerate() {
        let parsed = parse_module(source).unwrap();
        let mut program = (*compile_module_with_constructor_baseline(&resolve_module(&parsed).unwrap()).unwrap()).clone();
        let module_entry = program.entry;
        let result_entry = program.blocks.len();
        program.blocks.push(vec![
            I::Input(0),
            I::GetMember(Arc::from("result")),
            I::Return,
        ]);
        let mut session = HostSession::new(Arc::new(program));
        session.start(module_entry, vec![], 0, None).unwrap();
        let root = ready(&mut session, 0).unwrap();
        let initialized = Baseline::new(session.collect(&[]));
        let stale_root = root.clone();
        session.start(result_entry, vec![root], 1, None).unwrap();
        let token = request(&mut session, 1, "mem:parameter");
        assert!(initialized.added_values(session.collect(&[]), [4, 5, 4, 4][index]) > 0);
        session.resume(&token, Ok(Value::string("ok")), 2).unwrap();
        assert_eq!(
            ready(&mut session, 2),
            Ok(Value::string("prefix:value")),
            "{source}"
        );
        assert_eq!(initialized.added_values(session.collect(&[]), [4, 5, 4, 4][index]), -2);
    assert_eq!(session.start(module_entry, vec![stale_root], 10, None).unwrap_err().kind, ErrorKind::StaleHandle);
    session.close();
    assert_eq!(session.heap_stats().slots, 0);
    }
}

#[test]
fn failed_cast_of_cached_source_retries_and_successful_wrapper_cache_is_shared() {
    let parsed = parse_module(
        r#"
        source = new Listing { read("mem:value") }
        checked: Listing<String(read("mem:check") == "ok")> = source
        prime = source[0]
        result = checked[0]
        again = checked[0]
        "#,
    )
    .unwrap();
    let mut program =
        (*compile_module_with_constructor_baseline(&resolve_module(&parsed).unwrap()).unwrap())
            .clone();
    let module_entry = program.entry;
    let mut entries = Vec::new();
    for member in ["prime", "result", "again"] {
        entries.push(program.blocks.len());
        program.blocks.push(vec![
            I::Input(0),
            I::GetMember(Arc::from(member)),
            I::Return,
        ]);
    }
    let mut session = HostSession::new(Arc::new(program));
    session.start(module_entry, vec![], 0, None).unwrap();
    let root = ready(&mut session, 0).unwrap();
    let initialized = Baseline::new(session.collect(&[]));
    let stale_root = root.clone();
    session
        .start(entries[0], vec![root.clone()], 1, None)
        .unwrap();
    let value = request(&mut session, 1, "mem:value");
    session
        .resume(&value, Ok(Value::string("cached")), 1)
        .unwrap();
    assert_eq!(ready(&mut session, 1), Ok(Value::string("cached")));
    session
        .start(entries[1], vec![root.clone()], 2, None)
        .unwrap();
    // Empty wrappers may reuse the parent's cached source value, but they must
    // run their own cast and publish a cache only after it succeeds.
    let failed = request(&mut session, 2, "mem:check");
    let original = RuntimeError::new(ErrorKind::Host, "temporary cast failure");
    session.resume(&failed, Err(original.clone()), 2).unwrap();
    let error = ready(&mut session, 2).unwrap_err();
    assert_eq!(
        (error.kind, error.message),
        (original.kind, original.message)
    );
    session
        .start(entries[1], vec![root.clone()], 3, None)
        .unwrap();
    let retried = request(&mut session, 3, "mem:check");
    assert_ne!(retried, failed);
    session.collect(std::slice::from_ref(&root));
    session
        .resume(&retried, Ok(Value::string("ok")), 3)
        .unwrap();
    assert_eq!(ready(&mut session, 3), Ok(Value::string("cached")));
    session.start(entries[2], vec![root], 4, None).unwrap();
    assert_eq!(ready(&mut session, 4), Ok(Value::string("cached")));
    assert_eq!(initialized.added_values(session.collect(&[]), 4), -2);
    assert_eq!(
        session
            .start(module_entry, vec![stale_root], 10, None)
            .unwrap_err()
            .kind,
        ErrorKind::StaleHandle
    );
    session.close();
    assert_eq!(session.heap_stats().slots, 0);
    assert!(session.drain_cancelled_reads().is_empty());
}

#[test]
fn nested_wrappers_keep_temporary_check_receivers_across_gc_and_reentry() {
    let (mut session, root, outer_entry, inner_entry) = session();
    let initialized = Baseline::new(session.collect(&[]));
    session
        .start(outer_entry, vec![root.clone()], 1, None)
        .unwrap();
    let outer = request(&mut session, 1, "mem:outer");
    assert!(initialized.added_values(session.collect(&[]), 5) > 0);
    session
        .reenter(&outer, inner_entry, vec![root], 2, None)
        .unwrap();
    let inner = request(&mut session, 2, "mem:inner");
    assert!(initialized.added_values(session.collect(&[]), 11) > 0);
    session
        .resume(&inner, Ok(Value::string("inner")), 3)
        .unwrap();
    assert_eq!(ready(&mut session, 3), Ok(Value::string("inner")));
    assert_eq!(session.context_depth(), 1);
    assert_eq!(request(&mut session, 3, "mem:outer"), outer);
    session.collect(&[]);
    session
        .resume(&outer, Ok(Value::string("outer")), 4)
        .unwrap();
    assert_eq!(ready(&mut session, 4), Ok(Value::string("outer")));
    assert_eq!(session.context_depth(), 0);
    // Only Check's descriptor/prototype, its captured module and builtin Module parent survive. Temporary receivers,
    // Listings, Mappings and wrappers must all have been reclaimed.
    assert_eq!(initialized.added_values(session.collect(&[]), 11), 2);
    assert_eq!(initialized.added_values(session.collect(&[]), 11), 2);
    session.close();
    assert_eq!(session.heap_stats().live, 0);
    assert_eq!(session.heap_stats().slots, 0);
    assert!(session.drain_cancelled_reads().is_empty());
}

#[test]
fn failed_wrapper_predicate_retries_without_publishing_a_successful_member() {
    let (mut session, root, outer_entry, inner_entry) = session();
    let initialized = Baseline::new(session.collect(&[]));
    session
        .start(outer_entry, vec![root.clone()], 1, None)
        .unwrap();
    let outer = request(&mut session, 1, "mem:outer");
    session
        .reenter(&outer, inner_entry, vec![root.clone()], 2, None)
        .unwrap();
    let failed = request(&mut session, 2, "mem:inner");
    let original = RuntimeError::new(ErrorKind::Host, "wrapper read failure\0\r\n🙂");
    session.resume(&failed, Err(original.clone()), 2).unwrap();
    let error = ready(&mut session, 2).unwrap_err();
    assert_eq!(
        (error.kind, error.message),
        (original.kind, original.message)
    );
    assert_eq!(request(&mut session, 2, "mem:outer"), outer);
    session.collect(&[]);
    session
        .reenter(&outer, inner_entry, vec![root], 3, None)
        .unwrap();
    let retried = request(&mut session, 3, "mem:inner");
    assert_ne!(retried, failed);
    assert_eq!(
        session
            .resume(&failed, Ok(Value::Null), 3)
            .unwrap_err()
            .kind,
        ErrorKind::Host
    );
    session
        .resume(&retried, Ok(Value::string("inner")), 3)
        .unwrap();
    assert_eq!(ready(&mut session, 3), Ok(Value::string("inner")));
    session
        .resume(&outer, Ok(Value::string("outer")), 4)
        .unwrap();
    assert_eq!(ready(&mut session, 4), Ok(Value::string("outer")));
    // Only Check's descriptor/prototype, its captured module and builtin Module parent survive. Temporary receivers,
    // Listings, Mappings and wrappers must all have been reclaimed.
    assert_eq!(initialized.added_values(session.collect(&[]), 11), 2);
    assert_eq!(initialized.added_values(session.collect(&[]), 11), 2);
    session.close();
    assert_eq!(session.heap_stats().live, 0);
    assert_eq!(session.heap_stats().slots, 0);
}

#[derive(Clone, Copy, Debug)]
enum Termination {
    Close,
    Cancel,
    AncestorTimeout,
    ChildTimeoutOnResume,
}

#[test]
fn terminal_reentry_releases_wrapper_checks_and_cancels_both_pending_reads() {
    for termination in [
        Termination::Close,
        Termination::Cancel,
        Termination::AncestorTimeout,
        Termination::ChildTimeoutOnResume,
    ] {
        let (mut session, root, outer_entry, inner_entry) = session();
        let initialized = Baseline::new(session.collect(&[]));
        let deadline = matches!(termination, Termination::AncestorTimeout).then_some(9);
        session
            .start(outer_entry, vec![root.clone()], 1, deadline)
            .unwrap();
        let outer = request(&mut session, 1, "mem:outer");
        let child_deadline = matches!(termination, Termination::ChildTimeoutOnResume).then_some(5);
        session
            .reenter(&outer, inner_entry, vec![root.clone()], 2, child_deadline)
            .unwrap();
        let inner = request(&mut session, 2, "mem:inner");
        assert_eq!(session.context_depth(), 2);
        assert!(initialized.added_values(session.collect(&[]), 11) > 0);
        let (now, kind) = match termination {
            Termination::Close => {
                session.close();
                (3, ErrorKind::Closed)
            }
            Termination::Cancel => {
                session.cancel();
                (3, ErrorKind::Cancelled)
            }
            Termination::AncestorTimeout => (10, ErrorKind::Timeout),
            Termination::ChildTimeoutOnResume => {
                assert_eq!(
                    session
                        .resume(&inner, Ok(Value::string("late")), 7)
                        .unwrap_err()
                        .kind,
                    ErrorKind::Timeout
                );
                (7, ErrorKind::Timeout)
            }
        };
        let error = ready(&mut session, now).unwrap_err();
        assert_eq!(error.kind, kind, "{termination:?}");
        assert!(session.is_closed());
        assert_eq!(session.context_depth(), 0);
        assert_eq!(session.heap_stats().live, 0);
        assert_eq!(session.heap_stats().slots, 0);
        assert_eq!(
            session.drain_cancelled_reads(),
            [outer.clone(), inner.clone()]
        );
        for token in [&outer, &inner] {
            assert_eq!(
                session
                    .resume(token, Ok(Value::Null), now + 1)
                    .unwrap_err()
                    .kind,
                ErrorKind::Closed
            );
        }
        assert_eq!(
            session
                .start(outer_entry, vec![root], now + 1, None)
                .unwrap_err()
                .kind,
            ErrorKind::Closed
        );
        session.close();
        session.cancel();
        assert_eq!(ready(&mut session, now + 1).unwrap_err(), error);
        assert!(session.drain_cancelled_reads().is_empty());
    }
}
