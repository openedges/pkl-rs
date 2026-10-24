//! Resumable cache map publication uses the ordinary Host and tracing lifecycle.
//! The fifth cache insertion publishes before hashing; failure keeps that slot
//! but exposes only the rebuilt prefix (pinned EconomicMap Java gate/failure
//! observations in stage9-map-java-01/02). The distinct hidden Listing value and
//! subsequent whole-Mapping hash also have pinned Java observations in
//! target/m2/stage9-runtime-java-04. These tests exercise the Rust Host and GC
//! lifecycle; their callbacks do not constitute Java/Rust byte comparisons.

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

const SOURCE: &str = r#"
    key = new Listing { read("mem:key") }
    base = new Mapping { [key] = 0; ["a"] = 1; ["b"] = 2; ["c"] = 3 }
    mapping = (base) { ["d"] = let (value = read("mem:value")) new Listing { value } }
    parent = read("mem:outer")
    child = List(mapping[key], mapping["a"], mapping["b"], mapping["c"], mapping["d"])[4][0]
    inspect = let (index = new Mapping {
      [mapping] = 42; ["e"] = 1; ["f"] = 2; ["g"] = 3; ["h"] = 4
    }) index[mapping]
    compare = mapping == new Mapping {
      [key] = 0; ["a"] = 1; ["b"] = 2; ["c"] = 3; ["d"] = new Listing { "second" }
    }
"#;

fn session() -> (HostSession, Value, usize, usize, usize, usize) {
    session_source(SOURCE)
}

fn session_source(source: &str) -> (HostSession, Value, usize, usize, usize, usize) {
    let parsed = parse_module(source).unwrap();
    let mut program =
        (*compile_module_with_constructor_baseline(&resolve_module(&parsed).unwrap()).unwrap())
            .clone();
    let module_entry = program.entry;
    let [outer, child, inspect, compare] =
        ["parent", "child", "inspect", "compare"].map(|member| {
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
    (session, root, outer, child, inspect, compare)
}

#[test]
fn zero_hash_cached_lookup_returns_to_eager_collection_typecheck() {
    let source = r#"
        mapping = new Mapping {
          [new Listing { 0 }] = 0; [new Listing { 1 }] = 1
          [new Listing { 2 }] = 2; [new Listing { 3 }] = 3
          [new Listing { 4 }] = read("mem:value")
        }
        parent = read("mem:outer")
        child = List(mapping[new Listing { 0 }], mapping[new Listing { 1 }],
          mapping[new Listing { 2 }], mapping[new Listing { 3 }],
          mapping[new Listing { 4 }], mapping is Mapping<Listing, Int>)[5]
    "#;
    let (mut session, root, parent, child, _, _) = session_source(source);
    let initialized = Baseline::new(session.collect(&[]));
    let stale_module = root.clone();
    session.start(parent, vec![root.clone()], 1, None).unwrap();
    let outer = request(&mut session, 1, "mem:outer");
    session.reenter(&outer, child, vec![root], 2, None).unwrap();
    let value = request(&mut session, 2, "mem:value");
    session.collect(&[]);
    session.resume(&value, Ok(Value::Int(4.into())), 2).unwrap();
    assert_eq!(ready(&mut session, 2), Ok(Value::Bool(true)));
    session.resume(&outer, Ok(Value::Null), 3).unwrap();
    assert_eq!(ready(&mut session, 3), Ok(Value::Null));
    // User values are reclaimed; the private base and exact executable graph remain.
    assert_eq!(initialized.added_values(session.collect(&[]), 0), -2);
    assert_eq!(
        session
            .start(parent, vec![stale_module], 10, None)
            .unwrap_err()
            .kind,
        ErrorKind::StaleHandle
    );
    session.close();
    assert_eq!(session.heap_stats().slots, 0);
}

#[test]
fn inherited_mapping_length_retries_key_count_without_forcing_entry_values() {
    // VmMapping.getLength visits ancestor keys first and uses HashSet.add's
    // query hash/equality, while leaving entry bodies unevaluated.
    let source = r#"
        firstKey = new Listing { read("mem:firstKey") }
        lastKey = new Listing { read("mem:lastKey") }
        base = new Mapping { [firstKey] = throw("unforced ancestor value") }
        mapping = (base) { [lastKey] = throw("unforced child value") }
        parent = read("mem:outer")
        child = mapping.length
        inspect = mapping.isEmpty
    "#;
    let (mut session, root, parent, child, inspect, _) = session_source(source);
    let initialized = Baseline::new(session.collect(&[]));
    let stale_module = root.clone();
    session.start(parent, vec![root.clone()], 1, None).unwrap();
    let outer = request(&mut session, 1, "mem:outer");
    session
        .reenter(&outer, child, vec![root.clone()], 2, None)
        .unwrap();
    let first = request(&mut session, 2, "mem:firstKey");
    session.resume(&first, Ok(Value::Int(0.into())), 2).unwrap();
    let failed_last = request(&mut session, 2, "mem:lastKey");
    session.collect(&[]);
    let failure = RuntimeError::new(ErrorKind::Host, "retry Mapping key count\0\r\n🙂");
    session
        .resume(&failed_last, Err(failure.clone()), 2)
        .unwrap();
    let result = ready(&mut session, 2).unwrap_err();
    assert_eq!(
        (result.kind, result.message),
        (failure.kind, failure.message)
    );
    assert_eq!(session.context_depth(), 1);

    session
        .reenter(&outer, child, vec![root.clone()], 3, None)
        .unwrap();
    // The first key's completed value survives. Its zero hash is recomputed
    // without another Host read, and the failed child key acquires a new token.
    let last = request(&mut session, 3, "mem:lastKey");
    assert_ne!(last, failed_last);
    assert_eq!(
        session
            .resume(&failed_last, Ok(Value::Int(0.into())), 3)
            .unwrap_err()
            .kind,
        ErrorKind::Host
    );
    session.resume(&last, Ok(Value::Int(0.into())), 3).unwrap();
    assert_eq!(ready(&mut session, 3), Ok(Value::Int(1.into())));
    session.collect(&[]);
    session
        .reenter(&outer, inspect, vec![root], 3, None)
        .unwrap();
    assert_eq!(ready(&mut session, 3), Ok(Value::Bool(false)));
    session.resume(&outer, Ok(Value::Null), 4).unwrap();
    assert_eq!(ready(&mut session, 4), Ok(Value::Null));
    // User values are reclaimed; the private base and exact executable graph remain.
    assert_eq!(initialized.added_values(session.collect(&[]), 0), -2);
    assert_eq!(
        session
            .start(parent, vec![stale_module], 10, None)
            .unwrap_err()
            .kind,
        ErrorKind::StaleHandle
    );
    assert!(session.drain_cancelled_reads().is_empty());
    session.close();
    assert_eq!(session.heap_stats().slots, 0);
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
    panic!("evaluation did not reach a Host event within the step budget");
}

fn ready(session: &mut HostSession, now: u64) -> Result<Value, RuntimeError> {
    match event(session, now) {
        HostPoll::Ready(result) => result,
        result => panic!("expected result, got {result:?}"),
    }
}

fn request(session: &mut HostSession, now: u64, expected: &str) -> String {
    match event(session, now) {
        HostPoll::Read { token, uri } => {
            assert_eq!(&*uri, expected);
            token
        }
        result => panic!("expected {expected}, got {result:?}"),
    }
}

#[test]
fn fifth_cache_insertion_suspends_after_value_and_retries_hidden_slots_after_failure() {
    for key_value in [0, 7] {
        cache_retry(key_value);
    }
}

fn cache_retry(key_value: i64) {
    let (mut session, root, outer_entry, child, inspect, compare) = session();
    let initialized = Baseline::new(session.collect(&[]));
    let stale_module = root.clone();
    session
        .start(outer_entry, vec![root.clone()], 1, None)
        .unwrap();
    let outer = request(&mut session, 1, "mem:outer");
    session
        .reenter(&outer, child, vec![root.clone()], 2, None)
        .unwrap();
    // Parent and child declaration maps each have <= 4 entries. Only warming
    // the effective fifth cache entry triggers hashing of the Listing key.
    let value = request(&mut session, 2, "mem:value");
    session
        .resume(&value, Ok(Value::string("first")), 2)
        .unwrap();
    let failed_key = request(&mut session, 2, "mem:key");
    assert!(session.collect(&[]).live > 0);
    let failure = RuntimeError::new(ErrorKind::Host, "cache hash retry\0\r\n🙂");
    session
        .resume(&failed_key, Err(failure.clone()), 2)
        .unwrap();
    let result = ready(&mut session, 2).unwrap_err();
    assert_eq!(
        (result.kind, result.message),
        (failure.kind, failure.message)
    );
    assert_eq!(session.context_depth(), 1);
    assert_eq!(request(&mut session, 2, "mem:outer"), outer);

    session
        .reenter(&outer, child, vec![root.clone()], 3, None)
        .unwrap();
    // The failed rebuild already published a hash array. A fresh lookup hashes
    // its query first, and then cannot find any of the hidden cached slots.
    let retried_key = request(&mut session, 3, "mem:key");
    assert_ne!(retried_key, failed_key);
    assert_eq!(
        session
            .resume(&failed_key, Ok(Value::Null), 3)
            .unwrap_err()
            .kind,
        ErrorKind::Host
    );
    session
        .resume(&retried_key, Ok(Value::Int(key_value.into())), 3)
        .unwrap();
    let retried_value = request(&mut session, 3, "mem:value");
    assert_ne!(retried_value, value);
    session.collect(&[]);
    session
        .resume(&retried_value, Ok(Value::string("second")), 3)
        .unwrap();
    assert_eq!(ready(&mut session, 3), Ok(Value::string("second")));
    assert_eq!(session.context_depth(), 1);
    assert_eq!(request(&mut session, 3, "mem:outer"), outer);
    // The original hidden fifth value is a distinct Listing object reachable
    // only through its old cache cursor slot. Hashing the Mapping must still
    // traverse that value after collections and duplicate publication.
    session.collect(&[]);
    session
        .reenter(&outer, inspect, vec![root.clone()], 3, None)
        .unwrap();
    assert_eq!(ready(&mut session, 3), Ok(Value::Int(42.into())));
    session.collect(&[]);
    session
        .reenter(&outer, compare, vec![root], 3, None)
        .unwrap();
    // Java compares the original cursor value "first" as well as the later
    // duplicate "second". Comparing only the latest lookup value gives true.
    assert_eq!(ready(&mut session, 3), Ok(Value::Bool(false)));
    session
        .resume(&outer, Ok(Value::string("outer")), 4)
        .unwrap();
    assert_eq!(ready(&mut session, 4), Ok(Value::string("outer")));
    // User values are reclaimed; the private base and exact executable graph remain.
    assert_eq!(initialized.added_values(session.collect(&[]), 2), -2);
    assert_eq!(
        session
            .start(outer_entry, vec![stale_module], 10, None)
            .unwrap_err()
            .kind,
        ErrorKind::StaleHandle
    );
    assert!(session.drain_cancelled_reads().is_empty());
    session.close();
    assert_eq!(session.heap_stats().slots, 0);
}

#[test]
fn terminal_transitions_release_pending_cache_rebuild_and_all_host_reads() {
    for termination in ["close", "cancel", "ancestor timeout", "child timeout"] {
        let (mut session, root, outer_entry, child, _, _) = session();
        session
            .start(
                outer_entry,
                vec![root.clone()],
                1,
                (termination == "ancestor timeout").then_some(5),
            )
            .unwrap();
        let outer = request(&mut session, 1, "mem:outer");
        session
            .reenter(
                &outer,
                child,
                vec![root.clone()],
                2,
                (termination == "child timeout").then_some(3),
            )
            .unwrap();
        let value = request(&mut session, 2, "mem:value");
        session
            .resume(&value, Ok(Value::string("published")), 2)
            .unwrap();
        let key = request(&mut session, 2, "mem:key");
        assert!(session.collect(&[]).live > 0);
        let expected = match termination {
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
                    session
                        .resume(&key, Ok(Value::Int(7.into())), 6)
                        .unwrap_err()
                        .kind,
                    ErrorKind::Timeout
                );
                ErrorKind::Timeout
            }
            _ => ErrorKind::Timeout,
        };
        assert_eq!(ready(&mut session, 6).unwrap_err().kind, expected);
        assert_eq!(session.context_depth(), 0);
        assert_eq!(session.heap_stats().live, 0);
        assert_eq!(session.heap_stats().slots, 0);
        assert_eq!(
            session.drain_cancelled_reads(),
            [outer.clone(), key.clone()]
        );
        for token in [&outer, &key, &value] {
            assert_eq!(
                session.resume(token, Ok(Value::Null), 7).unwrap_err().kind,
                ErrorKind::Closed
            );
        }
        assert_eq!(
            session.start(child, vec![root], 7, None).unwrap_err().kind,
            ErrorKind::Closed
        );
    }
}
