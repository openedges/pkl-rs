//! Shared Host and tracing regressions; Java byte comparisons are recorded separately.
use std::sync::Arc;

use pkl_runtime::host::{HostPoll, HostSession};
use pkl_runtime::parser::parse_module;
use pkl_runtime::resolver::resolve_module;
use pkl_runtime::value::{ErrorKind, RuntimeError, Value};
use pkl_runtime::vm::{Instruction as I, compile_module};

const SOURCE: &str = r#"
import "pkl:ref"
class D extends ref.Domain {
  function renderReference(r): String = read("mem:render") + r.getData().text + r.getPath()[0].property
}
class Holder {
  value: String = read("mem:referent-body-must-not-run")
}
local domain = new D {}
local data = new Dynamic { text = "payload/" }
reference = ref.Reference(domain, Holder, data).value
parent = read("mem:outer")
rendered = "prefix/\(reference)"
inspect = reference.getData().text + reference.getPath()[0].property
checked: ref.Reference<D(read("mem:domain") == "yes"), String> = reference
checkedValue = checked.getData().text
"#;

fn setup(source: &str, members: &[&str]) -> (HostSession, Value, Vec<usize>) {
    let parsed = parse_module(source).unwrap();
    let mut program = (*compile_module(&resolve_module(&parsed).unwrap()).unwrap()).clone();
    let module_entry = program.entry;
    let entries = members
        .iter()
        .map(|member| {
            let entry = program.blocks.len();
            program.blocks.push(vec![
                I::Input(0),
                I::GetMember(Arc::from(*member)),
                I::Return,
            ]);
            entry
        })
        .collect();
    let mut session = HostSession::new(Arc::new(program));
    session.start(module_entry, vec![], 0, None).unwrap();
    let module = ready(&mut session, 0).unwrap();
    (session, module, entries)
}

fn event(session: &mut HostSession, now: u64) -> HostPoll {
    for _ in 0..50_000 {
        match session.poll(now, 1) {
            HostPoll::Yielded => {
                session.collect(&[]);
            }
            result => return result,
        }
    }
    panic!("reference evaluation did not reach a Host event");
}

fn ready(session: &mut HostSession, now: u64) -> Result<Value, RuntimeError> {
    match event(session, now) {
        HostPoll::Ready(result) => result,
        result => panic!("expected result, got {result:?}"),
    }
}

fn request(session: &mut HostSession, now: u64, uri: &str) -> String {
    match event(session, now) {
        HostPoll::Read { token, uri: actual } => {
            assert_eq!(&*actual, uri);
            token
        }
        result => panic!("expected {uri}, got {result:?}"),
    }
}

#[test]
fn suspended_rendering_preserves_reference_metadata_during_reentry_gc_and_retry() {
    let (mut session, module, entries) = setup(SOURCE, &["parent", "rendered", "inspect"]);
    let [parent, rendered, inspect] = entries[..] else {
        unreachable!()
    };
    session
        .start(parent, vec![module.clone()], 1, None)
        .unwrap();
    let outer = request(&mut session, 1, "mem:outer");
    session
        .reenter(&outer, rendered, vec![module.clone()], 2, None)
        .unwrap();
    let failed = request(&mut session, 2, "mem:render");
    session
        .reenter(&failed, inspect, vec![module.clone()], 3, None)
        .unwrap();
    assert_eq!(ready(&mut session, 3), Ok(Value::string("payload/value")));
    session.collect(&[]);
    let failure = RuntimeError::new(ErrorKind::Host, "render failed\0\r\n🙂");
    session.resume(&failed, Err(failure.clone()), 3).unwrap();
    let error = ready(&mut session, 3).unwrap_err();
    let interpolation = SOURCE.find(r"\(reference)").unwrap();
    assert!(error.frames.iter().any(
        |frame| frame.member.as_str().expect("well-formed frame label") == "#rendered"
            && frame.source.is_none()
            && frame.span
                == pkl_runtime::source::Span::new(
                    interpolation,
                    interpolation + r"\(reference)".len()
                )
    ));
    assert_eq!((error.kind, error.message), (failure.kind, failure.message));
    assert_eq!(session.context_depth(), 1);
    session
        .reenter(&outer, rendered, vec![module], 4, None)
        .unwrap();
    let retry = request(&mut session, 4, "mem:render");
    assert_ne!(retry, failed);
    assert_eq!(
        session
            .resume(&failed, Ok(Value::Null), 4)
            .unwrap_err()
            .kind,
        ErrorKind::Host
    );
    session
        .resume(&retry, Ok(Value::string("resolved/")), 4)
        .unwrap();
    assert_eq!(
        ready(&mut session, 4),
        Ok(Value::string("prefix/resolved/payload/value"))
    );
    session.resume(&outer, Ok(Value::Null), 5).unwrap();
    assert_eq!(ready(&mut session, 5), Ok(Value::Null));
    // Evaluator-owned class roots retain the captured module's completed cache
    // closure. Repeated GC must settle at this exact source-specific size.
    assert_eq!(session.collect(&[]).live, 10);
    assert_eq!(session.collect(&[]).live, 10);
    session.close();
    assert_eq!(session.heap_stats().live, 0);
    assert_eq!(session.heap_stats().slots, 0);
    assert!(session.drain_cancelled_reads().is_empty());
}

#[test]
fn domain_constraint_continuations_keep_reference_and_custom_this_through_gc() {
    let (mut session, module, entries) = setup(SOURCE, &["parent", "checkedValue", "inspect"]);
    let [parent, checked, inspect] = entries[..] else {
        unreachable!()
    };
    session
        .start(parent, vec![module.clone()], 1, None)
        .unwrap();
    let outer = request(&mut session, 1, "mem:outer");
    session
        .reenter(&outer, checked, vec![module.clone()], 2, None)
        .unwrap();
    let constraint = request(&mut session, 2, "mem:domain");
    session
        .reenter(&constraint, inspect, vec![module], 3, None)
        .unwrap();
    assert_eq!(ready(&mut session, 3), Ok(Value::string("payload/value")));
    session.collect(&[]);
    session
        .resume(&constraint, Ok(Value::string("yes")), 3)
        .unwrap();
    assert_eq!(ready(&mut session, 3), Ok(Value::string("payload/")));
    session.resume(&outer, Ok(Value::Null), 4).unwrap();
    assert_eq!(ready(&mut session, 4), Ok(Value::Null));
    // Evaluator-owned class roots retain the captured module's completed cache
    // closure. Repeated GC must settle at this exact source-specific size.
    assert_eq!(retained_values(session.collect(&[]), 4), 9);
    assert_eq!(retained_values(session.collect(&[]), 4), 9);
    session.close();
    assert_eq!(session.heap_stats().live, 0);
    assert_eq!(session.heap_stats().slots, 0);
}

#[test]
fn pending_reference_rendering_closes_all_contexts_on_cancel_close_and_deadline() {
    for termination in [ErrorKind::Cancelled, ErrorKind::Closed, ErrorKind::Timeout] {
        let (mut session, module, entries) = setup(SOURCE, &["parent", "rendered"]);
        session
            .start(
                entries[0],
                vec![module.clone()],
                1,
                (termination == ErrorKind::Timeout).then_some(5),
            )
            .unwrap();
        let outer = request(&mut session, 1, "mem:outer");
        session
            .reenter(&outer, entries[1], vec![module], 2, None)
            .unwrap();
        let inner = request(&mut session, 2, "mem:render");
        session.collect(&[]);
        match termination {
            ErrorKind::Cancelled => session.cancel(),
            ErrorKind::Closed => session.close(),
            ErrorKind::Timeout => {}
            _ => unreachable!(),
        }
        let now = if termination == ErrorKind::Timeout {
            6
        } else {
            3
        };
        assert_eq!(ready(&mut session, now).unwrap_err().kind, termination);
        assert!(session.is_closed());
        assert_eq!(session.context_depth(), 0);
        assert_eq!(session.heap_stats().live, 0);
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
        assert!(session.drain_cancelled_reads().is_empty());
    }
}

#[test]
fn equality_retains_pending_domain_and_data_comparisons_and_retries_only_failed_reads() {
    let source = r#"
        import "pkl:ref"
        class D extends ref.Domain {
          tag: String
          function renderReference(r): String = throw("equality must not render")
        }
        class Holder { value: String = throw("equality must not force referent") }
        left = ref.Reference(new D { tag = read("mem:domain") }, Holder,
          new Dynamic { text = read("mem:data") }).value
        right = ref.Reference(new D { tag = "same" }, Holder,
          new Dynamic { text = "same" }).value
        parent = read("mem:outer")
        compare = left == right
        inspect = left.getPath()[0].property
    "#;
    let (mut session, module, entries) = setup(source, &["parent", "compare", "inspect"]);
    let [parent, compare, inspect] = entries[..] else {
        unreachable!()
    };
    session
        .start(parent, vec![module.clone()], 1, None)
        .unwrap();
    let outer = request(&mut session, 1, "mem:outer");
    session
        .reenter(&outer, compare, vec![module.clone()], 2, None)
        .unwrap();
    let domain = request(&mut session, 2, "mem:domain");
    session
        .resume(&domain, Ok(Value::string("same")), 2)
        .unwrap();
    let data = request(&mut session, 2, "mem:data");
    session
        .reenter(&data, inspect, vec![module.clone()], 3, None)
        .unwrap();
    assert_eq!(ready(&mut session, 3), Ok(Value::string("value")));
    let failure = RuntimeError::new(ErrorKind::Host, "reference equality data failure");
    session.resume(&data, Err(failure.clone()), 3).unwrap();
    let error = ready(&mut session, 3).unwrap_err();
    assert_eq!((error.kind, error.message), (failure.kind, failure.message));
    session
        .reenter(&outer, compare, vec![module], 4, None)
        .unwrap();
    let retry = request(&mut session, 4, "mem:data");
    assert_ne!(retry, data);
    session
        .resume(&retry, Ok(Value::string("same")), 4)
        .unwrap();
    assert_eq!(ready(&mut session, 4), Ok(Value::Bool(true)));
    session.resume(&outer, Ok(Value::Null), 5).unwrap();
    assert_eq!(ready(&mut session, 5), Ok(Value::Null));
    // Evaluator-owned class roots retain the captured module's completed cache
    // closure. Repeated GC must settle at this exact source-specific size.
    assert_eq!(session.collect(&[]).live, 14);
    assert_eq!(session.collect(&[]).live, 14);
    session.close();
    assert_eq!(session.heap_stats().live, 0);
    assert_eq!(session.heap_stats().slots, 0);
}

// Persistent executable/type nodes have their own exact bound. The returned
// count preserves the original object and custom-this-slot assertions.
#[track_caller]
fn retained_values(stats: pkl_runtime::heap::HeapStats, expected_executables: usize) -> usize {
    assert_eq!(
        stats.executable_live, expected_executables,
        "persistent executable graph"
    );
    stats.live - stats.executable_live
}
