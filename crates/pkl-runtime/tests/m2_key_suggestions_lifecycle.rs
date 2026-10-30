//! Source-backed scheduling controls, separate from Java raw compatibility.
use pkl_runtime::{
    host::{HostPoll, HostSession},
    parser::parse_module,
    resolver::resolve_module,
    value::{ErrorKind, RuntimeError, Value},
    vm::{Instruction as I, Program, compile_module},
};
use std::sync::Arc;

fn program(source: &str) -> (Arc<Program>, usize, usize) {
    let mut p = (*compile_module(&resolve_module(&parse_module(source).unwrap()).unwrap())
        .unwrap())
    .clone();
    let [result, side] = ["result", "side"].map(|name| {
        let n = p.blocks.len();
        p.blocks
            .push(vec![I::Input(0), I::GetMember(name.into()), I::Return]);
        n
    });
    (Arc::new(p), result, side)
}
fn event(s: &mut HostSession, roots: &[Value]) -> HostPoll {
    for _ in 0..200_000 {
        s.collect(roots);
        match s.poll(0, 1) {
            HostPoll::Yielded => {}
            event => return event,
        }
    }
    panic!("key-suggestion poll bound")
}
fn ready(s: &mut HostSession, roots: &[Value]) -> Result<Value, RuntimeError> {
    match event(s, roots) {
        HostPoll::Ready(v) => v,
        event => panic!("unexpected {event:?}"),
    }
}
fn read(s: &mut HostSession, roots: &[Value], expected: &str) -> String {
    match event(s, roots) {
        HostPoll::Read { token, uri } => {
            assert_eq!(uri, expected);
            token
        }
        event => panic!("unexpected {event:?}"),
    }
}
// This read boundary is the runtime scheduling analogue of the separately
// captured Java read?/trace key. Host protocol behavior is an internal test,
// not a Java cancellation or reentry compatibility claim. A single object key
// avoids object-key equality during declaration insertion before hint traversal.
const SOURCE: &str = r#"
key = new Dynamic {
  first = read("mem:first")
  second = read("mem:second")
}
x = new Mapping {
  [key] = 1
  ["fooz"] = 3
}
result = x["foo"]
side = x["fooz"]
"#;
fn missing(error: RuntimeError) {
    assert_eq!(error.kind, ErrorKind::MissingMember);
    assert_eq!(error.message.to_java_utf8(), "Cannot find key `\"foo\"`.");
    assert_eq!(
        error.hint.as_ref().unwrap().to_java_utf8(),
        "Did you mean any of the following?\n\"fooz\""
    );
}
#[test]
fn hashing_keys_preserves_order_and_state_across_reentry_error_retry_and_gc() {
    let (p, result, side) = program(SOURCE);
    let mut s = HostSession::new(p.clone());
    s.start(p.entry, vec![], 0, None).unwrap();
    let module = ready(&mut s, &[]).unwrap();
    let roots = [module.clone()];
    s.start(result, vec![module.clone()], 0, None).unwrap();
    let first = read(&mut s, &roots, "mem:first");
    s.reenter(&first, side, vec![module.clone()], 0, None)
        .unwrap();
    assert_eq!(ready(&mut s, &roots), Ok(Value::Int(3.into())));
    assert_eq!(read(&mut s, &roots, "mem:first"), first);
    s.resume(&first, Ok(Value::Int(7.into())), 0).unwrap();
    let second = read(&mut s, &roots, "mem:second");
    s.resume(
        &second,
        Err(RuntimeError::new(ErrorKind::Host, "retry key hash")),
        0,
    )
    .unwrap();
    assert_eq!(ready(&mut s, &roots).unwrap_err().kind, ErrorKind::Host);
    s.start(result, vec![module], 0, None).unwrap();
    let retry = read(&mut s, &roots, "mem:second");
    assert_ne!(retry, second);
    s.resume(&retry, Ok(Value::Int(8.into())), 0).unwrap();
    missing(ready(&mut s, &roots).unwrap_err());
    s.close();
    assert_eq!(s.collect(&[]).slots, 0);
}
#[test]
fn found_entry_and_non_string_miss_do_not_start_hint_key_hashing() {
    for (expression, success) in [("x[\"fooz\"]", true), ("x[42]", false)] {
        let source = format!(
            r#"
k = new Dynamic {{ n = read("mem:unforced") }}
x = new Mapping {{ [k] = 1
["fooz"] = 2 }}
result = {expression}
side = true
"#
        );
        let (p, result, _) = program(&source);
        let mut s = HostSession::new(p.clone());
        s.start(p.entry, vec![], 0, None).unwrap();
        let module = ready(&mut s, &[]).unwrap();
        s.start(result, vec![module.clone()], 0, None).unwrap();
        let actual = ready(&mut s, &[module]);
        if success {
            assert_eq!(actual, Ok(Value::Int(2.into())));
        } else {
            let error = actual.unwrap_err();
            assert_eq!(error.kind, ErrorKind::MissingMember);
            assert_eq!(error.message.to_java_utf8(), "Cannot find key `42`.");
            assert!(error.hint.is_none());
        }
        s.close();
        assert_eq!(s.collect(&[]).slots, 0);
    }
}
#[test]
fn cancel_or_close_at_each_key_releases_hash_and_visited_roots() {
    for second in [false, true] {
        for close in [false, true] {
            let (p, result, _) = program(SOURCE);
            let mut s = HostSession::new(p.clone());
            s.start(p.entry, vec![], 0, None).unwrap();
            let module = ready(&mut s, &[]).unwrap();
            let roots = [module.clone()];
            s.start(result, vec![module], 0, None).unwrap();
            let mut token = read(&mut s, &roots, "mem:first");
            if second {
                s.resume(&token, Ok(Value::Int(7.into())), 0).unwrap();
                token = read(&mut s, &roots, "mem:second");
            }
            if close {
                s.close()
            } else {
                s.cancel()
            }
            assert_eq!(
                ready(&mut s, &[]).unwrap_err().kind,
                if close {
                    ErrorKind::Closed
                } else {
                    ErrorKind::Cancelled
                }
            );
            assert!(s.resume(&token, Ok(Value::Null), 0).is_err());
            s.close();
            assert_eq!(s.collect(&[]).slots, 0);
        }
    }
}
