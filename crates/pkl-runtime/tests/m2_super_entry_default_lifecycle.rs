//! Authored Host ownership/order assertions, separate from Java raw compatibility.
//! Host scheduling assertions exercise the shared VM; Java SPI scheduling parity is not claimed.
use pkl_runtime::{
    host::{HostPoll, HostSession},
    parser::parse_module,
    resolver::resolve_module,
    value::{ErrorKind, RuntimeError, Value},
    vm::{Instruction as I, compile_module},
};
use std::sync::Arc;

fn event(session: &mut HostSession) -> HostPoll {
    for _ in 0..200_000 {
        session.collect(&[]);
        match session.poll(0, 1) {
            HostPoll::Yielded => {}
            event => return event,
        }
    }
    panic!("super entry Host poll bound")
}
fn ready(session: &mut HostSession) -> Result<Value, RuntimeError> {
    match event(session) {
        HostPoll::Ready(value) => value,
        event => panic!("unexpected {event:?}"),
    }
}
fn read(session: &mut HostSession, expected: &str) -> String {
    match event(session) {
        HostPoll::Read { token, uri } => {
            assert_eq!(&*uri, expected);
            token
        }
        event => panic!("unexpected {event:?}"),
    }
}
fn reply(session: &mut HostSession, uri: &str, value: Value) {
    let token = read(session, uri);
    session.resume(&token, Ok(value), 0).unwrap();
}
fn setup(source: &str, names: &[&str]) -> (HostSession, Value, Vec<usize>) {
    let mut program = (*compile_module(&resolve_module(&parse_module(source).unwrap()).unwrap())
        .unwrap())
    .clone();
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
    let entry = program.entry;
    let mut session = HostSession::new(Arc::new(program));
    session.start(entry, vec![], 0, None).unwrap();
    let module = ready(&mut session).unwrap();
    (session, module, entries)
}
const SOURCE: &str = r#"
outerValue = read("mem:outer")
parent = new Mapping {
  default = (key) -> throw("lexical parent default must not run")
  ["hit"] = read("mem:parent")
}
child = (parent) {
  default = let (gate = read("mem:default")) (key) -> gate + read("mem:callback-\(key)")
  ["hit"] = super["hit"]
  ["miss"] = super[read("mem:key")]
}
result = child["miss"]
hit = child["hit"]
ordinaryMissing = child["ordinary"]
"#;

#[test]
fn default_property_and_callback_suspend_with_key_receiver_and_outer_roots() {
    let (mut session, module, entries) = setup(SOURCE, &["outerValue", "result", "hit"]);
    session
        .start(entries[0], vec![module.clone()], 0, None)
        .unwrap();
    let outer = read(&mut session, "mem:outer");
    session
        .reenter(&outer, entries[1], vec![module.clone()], 0, None)
        .unwrap();
    let key = read(&mut session, "mem:key");
    // A present inherited entry must not touch either default function.
    session
        .reenter(&key, entries[2], vec![module.clone()], 0, None)
        .unwrap();
    reply(&mut session, "mem:parent", Value::string("inherited"));
    assert_eq!(ready(&mut session), Ok(Value::string("inherited")));
    assert_eq!(read(&mut session, "mem:key"), key);
    session
        .resume(&key, Ok(Value::string("missing")), 0)
        .unwrap();
    let default = read(&mut session, "mem:default");
    for _ in 0..4 {
        session.collect(&[]);
        assert_eq!(read(&mut session, "mem:default"), default);
    }
    session
        .resume(&default, Ok(Value::string("prefix-")), 0)
        .unwrap();
    reply(&mut session, "mem:callback-missing", Value::string("value"));
    assert_eq!(ready(&mut session), Ok(Value::string("prefix-value")));
    assert_eq!(read(&mut session, "mem:outer"), outer);
    // Successful member caching must not invoke the fallback a second time.
    session
        .reenter(&outer, entries[1], vec![module], 0, None)
        .unwrap();
    assert_eq!(ready(&mut session), Ok(Value::string("prefix-value")));
    assert_eq!(read(&mut session, "mem:outer"), outer);
    session.resume(&outer, Ok(Value::Null), 0).unwrap();
    assert_eq!(ready(&mut session), Ok(Value::Null));
    session.close();
    assert_eq!(session.collect(&[]).slots, 0);
}

#[test]
fn failed_default_or_callback_can_retry_without_losing_suspended_caller() {
    for fail_default in [true, false] {
        let (mut session, module, entries) = setup(SOURCE, &["outerValue", "result"]);
        session
            .start(entries[0], vec![module.clone()], 0, None)
            .unwrap();
        let outer = read(&mut session, "mem:outer");
        session
            .reenter(&outer, entries[1], vec![module.clone()], 0, None)
            .unwrap();
        reply(&mut session, "mem:key", Value::string("missing"));
        let pending = if fail_default {
            read(&mut session, "mem:default")
        } else {
            reply(&mut session, "mem:default", Value::string("prefix-"));
            read(&mut session, "mem:callback-missing")
        };
        session
            .resume(
                &pending,
                Err(RuntimeError::new(ErrorKind::Host, "retry fallback")),
                0,
            )
            .unwrap();
        assert_eq!(ready(&mut session).unwrap_err().kind, ErrorKind::Host);
        assert_eq!(read(&mut session, "mem:outer"), outer);
        session
            .reenter(&outer, entries[1], vec![module], 0, None)
            .unwrap();
        reply(&mut session, "mem:key", Value::string("missing"));
        if fail_default {
            reply(&mut session, "mem:default", Value::string("prefix-"));
        }
        reply(
            &mut session,
            "mem:callback-missing",
            Value::string("retried"),
        );
        assert_eq!(ready(&mut session), Ok(Value::string("prefix-retried")));
        assert_eq!(read(&mut session, "mem:outer"), outer);
        session.close();
        assert_eq!(session.collect(&[]).slots, 0);
    }
}

#[test]
fn cancellation_at_key_default_and_callback_releases_continuations() {
    for stage in 0..3 {
        for close in [false, true] {
            let (mut session, module, entries) = setup(SOURCE, &["result"]);
            session.start(entries[0], vec![module], 0, None).unwrap();
            let pending = if stage == 0 {
                read(&mut session, "mem:key")
            } else {
                reply(&mut session, "mem:key", Value::string("missing"));
                if stage == 1 {
                    read(&mut session, "mem:default")
                } else {
                    reply(&mut session, "mem:default", Value::string("prefix-"));
                    read(&mut session, "mem:callback-missing")
                }
            };
            if close {
                session.close();
            } else {
                session.cancel();
            }
            assert_eq!(
                ready(&mut session).unwrap_err().kind,
                if close {
                    ErrorKind::Closed
                } else {
                    ErrorKind::Cancelled
                }
            );
            assert_eq!(
                session
                    .resume(&pending, Ok(Value::Null), 0)
                    .unwrap_err()
                    .kind,
                ErrorKind::Closed
            );
            session.close();
            assert_eq!(session.collect(&[]).slots, 0);
        }
    }
}

#[test]
fn object_key_hash_suspends_before_default_and_retries_after_host_failure() {
    let source = r#"
outerValue = read("mem:outer")
parent = new Mapping { ["hit"] = "inherited" }
child = (parent) {
  default = let (gate = read("mem:default")) (key) -> gate + key.value
  ["miss"] = super[new Dynamic { value = read("mem:key-hash") }]
}
result = child["miss"]
hit = child["hit"]
"#;
    let (mut session, module, entries) = setup(source, &["outerValue", "result", "hit"]);
    session
        .start(entries[0], vec![module.clone()], 0, None)
        .unwrap();
    let outer = read(&mut session, "mem:outer");
    session
        .reenter(&outer, entries[1], vec![module.clone()], 0, None)
        .unwrap();
    // The ephemeral object key must be hashed before even reading default.
    // Polling collects on every step; the lookup owns the key while suspended.
    let failed_hash = read(&mut session, "mem:key-hash");
    session
        .reenter(&failed_hash, entries[2], vec![module.clone()], 0, None)
        .unwrap();
    assert_eq!(ready(&mut session), Ok(Value::string("inherited")));
    assert_eq!(session.context_depth(), 2);
    assert_eq!(read(&mut session, "mem:key-hash"), failed_hash);
    session
        .resume(
            &failed_hash,
            Err(RuntimeError::new(ErrorKind::Host, "retry object key hash")),
            0,
        )
        .unwrap();
    assert_eq!(ready(&mut session).unwrap_err().kind, ErrorKind::Host);
    assert_eq!(session.context_depth(), 1);
    assert_eq!(read(&mut session, "mem:outer"), outer);
    session
        .reenter(&outer, entries[1], vec![module.clone()], 0, None)
        .unwrap();
    let retry_hash = read(&mut session, "mem:key-hash");
    for _ in 0..4 {
        session.collect(&[]);
        assert_eq!(read(&mut session, "mem:key-hash"), retry_hash);
    }
    session
        .resume(&retry_hash, Ok(Value::string("key")), 0)
        .unwrap();
    // Resuming the terminal declaration probe must not report a cyclic owner.
    // The completed hash also caches key.value, which default reads below.
    let default = read(&mut session, "mem:default");
    session
        .reenter(&default, entries[2], vec![module], 0, None)
        .unwrap();
    assert_eq!(ready(&mut session), Ok(Value::string("inherited")));
    assert_eq!(read(&mut session, "mem:default"), default);
    session
        .resume(&default, Ok(Value::string("fallback-")), 0)
        .unwrap();
    assert_eq!(ready(&mut session), Ok(Value::string("fallback-key")));
    assert_eq!(session.context_depth(), 1);
    assert_eq!(read(&mut session, "mem:outer"), outer);
    session.resume(&outer, Ok(Value::Null), 0).unwrap();
    assert_eq!(ready(&mut session), Ok(Value::Null));
    session.close();
    assert_eq!(session.collect(&[]).slots, 0);
    assert!(session.drain_cancelled_reads().is_empty());
}
