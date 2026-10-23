//! Fixed declarations share the ordinary Host scheduler and tracing lifecycle.
//! Java byte compatibility for selected sources is recorded separately.
//! Retained module graphs include their builtin Module parent and source-backed
//! stdlib annotations; transient objects and terminal-zero assertions stay separate.
#[path = "support/eager_base.rs"]
mod eager_base;
use eager_base::Baseline;
#[path = "support/module_policy.rs"]
mod module_policy;
use std::sync::Arc;

use pkl_runtime::host::{HostPoll, HostSession};
use pkl_runtime::parser::parse_module;
use pkl_runtime::resolver::resolve_module;
use pkl_runtime::value::{ErrorKind, RuntimeError, Value};
use pkl_runtime::vm::{Instruction as I, compile_module};

const SOURCE: &str = r#"
import "pkl:ref"
class D extends ref.Domain {
  function renderReference(r): String = read("mem:render") + r.getPath()[0].property
}
open class Base {
  fixed payload: String = read("mem:fixed") + suffix
  suffix: String = "base"
  hidden fixed secret: String = read("mem:referent-body-must-not-run")
}
class Child extends Base { suffix = "child" }
local item = new Child {}
local amended = (item) { suffix = "amended" }
local domain = new D {}
reference = ref.Reference(domain, Child, new Dynamic { text = "kept" }).secret
parent = read("mem:outer")
value = amended.payload
rendered = "\(reference)"
inspect = reference is ref.Reference<D, String> && reference.getPath()[0].property == "secret" && reference.getData().text == "kept"
"#;

fn setup() -> (HostSession, Value, [usize; 4]) {
    let parsed = parse_module(SOURCE).unwrap();
    let mut program = (*compile_module(&resolve_module(&parsed).unwrap()).unwrap()).clone();
    module_policy::register(&mut program, SOURCE);
    let module_entry = program.entry;
    let entries = ["parent", "value", "rendered", "inspect"].map(|member| {
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
    let module = ready(&mut session, 0).unwrap();
    (session, module, entries)
}

fn event(session: &mut HostSession, now: u64) -> HostPoll {
    for _ in 0..50_000 {
        match session.poll(now, 1) {
            HostPoll::Yielded => {
                session.collect(&[]);
            }
            HostPoll::Module { token, request } => session
                .complete_module(
                    &token,
                    pkl_runtime::modules::default_completion(&request)
                        .expect("default module policy"),
                    now,
                )
                .unwrap(),
            result => return result,
        }
    }
    panic!("fixed declaration evaluation did not reach a Host event");
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
fn inherited_fixed_values_keep_the_amended_receiver_across_gc_reentry_and_retry() {
    let (mut session, module, [parent, value, _, inspect]) = setup();
    let initialized = Baseline::new(session.collect(&[]));
    session
        .start(parent, vec![module.clone()], 1, None)
        .unwrap();
    let outer = request(&mut session, 1, "mem:outer");
    session
        .reenter(&outer, value, vec![module.clone()], 2, None)
        .unwrap();
    let failed = request(&mut session, 2, "mem:fixed");
    session
        .reenter(&failed, inspect, vec![module.clone()], 3, None)
        .unwrap();
    assert_eq!(ready(&mut session, 3), Ok(Value::Bool(true)));
    session.collect(&[]);
    let failure = RuntimeError::new(ErrorKind::Host, "fixed value failure\0\r\n🙂");
    session.resume(&failed, Err(failure.clone()), 3).unwrap();
    let error = ready(&mut session, 3).unwrap_err();
    assert_eq!((error.kind, error.message), (failure.kind, failure.message));
    assert_eq!(session.context_depth(), 1);
    session
        .reenter(&outer, value, vec![module.clone()], 4, None)
        .unwrap();
    let retry = request(&mut session, 4, "mem:fixed");
    assert_ne!(retry, failed);
    assert_eq!(
        session
            .resume(&failed, Ok(Value::Null), 4)
            .unwrap_err()
            .kind,
        ErrorKind::Host
    );
    session
        .resume(&retry, Ok(Value::string("prefix/")), 4)
        .unwrap();
    assert_eq!(ready(&mut session, 4), Ok(Value::string("prefix/amended")));
    session
        .reenter(&outer, value, vec![module], 5, None)
        .unwrap();
    assert_eq!(ready(&mut session, 5), Ok(Value::string("prefix/amended")));
    session.resume(&outer, Ok(Value::Null), 6).unwrap();
    assert_eq!(ready(&mut session, 6), Ok(Value::Null));
    // Exact source-specific class, annotation and completed Reference/cache
    // closure above the initialized base. Access-list temporaries are reclaimed;
    // shared annotation descriptors are already present in the base baseline.
    // The lazy Domain class first forces its authored ref import here. The
    // VM roots that static site's executable; the module caches separately
    // retain module receivers.
    assert_eq!(initialized.added_values(session.collect(&[]), 1), 12);
    assert_eq!(initialized.added_values(session.collect(&[]), 1), 12);
    session.close();
    assert_eq!(session.heap_stats().live, 0);
    assert_eq!(session.heap_stats().slots, 0);
    assert!(session.drain_cancelled_reads().is_empty());
}

#[test]
fn inherited_hidden_fixed_reference_metadata_stays_deferred_during_render_retry() {
    let (mut session, module, [parent, _, rendered, inspect]) = setup();
    let initialized = Baseline::new(session.collect(&[]));
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
    assert_eq!(ready(&mut session, 3), Ok(Value::Bool(true)));
    session.collect(&[]);
    let failure = RuntimeError::new(ErrorKind::Host, "fixed metadata render failure");
    session.resume(&failed, Err(failure.clone()), 3).unwrap();
    let error = ready(&mut session, 3).unwrap_err();
    assert_eq!((error.kind, error.message), (failure.kind, failure.message));
    session
        .reenter(&outer, rendered, vec![module], 4, None)
        .unwrap();
    let retry = request(&mut session, 4, "mem:render");
    session.collect(&[]);
    session
        .resume(&retry, Ok(Value::string("path/")), 4)
        .unwrap();
    assert_eq!(ready(&mut session, 4), Ok(Value::string("path/secret")));
    session.resume(&outer, Ok(Value::Null), 5).unwrap();
    assert_eq!(ready(&mut session, 5), Ok(Value::Null));
    // Exact source-specific class, annotation and completed Reference/cache
    // closure above the initialized base. Access-list temporaries are reclaimed;
    // shared annotation descriptors are already present in the base baseline.
    // The lazy ref import site and renderReference parameter-slot descriptor
    // each add one executable above the setup baseline.
    assert_eq!(initialized.added_values(session.collect(&[]), 2), 12);
    assert_eq!(initialized.added_values(session.collect(&[]), 2), 12);
    session.close();
    assert_eq!(session.heap_stats().live, 0);
    assert_eq!(session.heap_stats().slots, 0);
}

#[test]
fn pending_inherited_fixed_values_release_all_contexts_on_terminal_events() {
    for termination in [ErrorKind::Cancelled, ErrorKind::Closed, ErrorKind::Timeout] {
        let (mut session, module, [parent, value, _, _]) = setup();
        session
            .start(
                parent,
                vec![module.clone()],
                1,
                (termination == ErrorKind::Timeout).then_some(5),
            )
            .unwrap();
        let outer = request(&mut session, 1, "mem:outer");
        session
            .reenter(&outer, value, vec![module], 2, None)
            .unwrap();
        let inner = request(&mut session, 2, "mem:fixed");
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
