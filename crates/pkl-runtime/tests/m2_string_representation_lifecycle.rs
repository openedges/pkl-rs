//! Lossless language strings remain distinct before the explicit byte boundary.
//! Java compatibility is established separately by the recorded oracle comparisons.
use std::sync::Arc;

use pkl_runtime::host::{HostPoll, HostSession};
use pkl_runtime::parser::parse_module;
use pkl_runtime::resolver::resolve_module;
use pkl_runtime::value::{ErrorKind, PklString, RuntimeError, Value};
use pkl_runtime::vm::{Instruction as I, Program, compile_module};

fn units(value: Value) -> Vec<u16> {
    let Value::String(value) = value else {
        panic!("expected a String, got {value:?}");
    };
    value.encode_utf16().collect()
}

#[test]
fn escape_units_survive_values_keys_types_and_interpolation() {
    let source = r#"
high = "\u{d83d}"
low = "\u{de00}"
pair = "\u{d83d}\u{de00}"
reverse = "\u{de00}\u{d83d}"
replacement = "\u{fffd}"
joined = high + low
interpolated = "\(high)\(low)"
literal: "\u{d83d}" = high
keys = Map(high, 11, low, 22, replacement, 33, "?", 44)
lookup = keys["\u{d83d}"] + keys["\u{de00}"] + keys[replacement] + keys["?"]
distinct = Set(high, low, replacement, "?").length
pairKey = Map(pair, 42)["😀"]
pairDistinct = Set(pair, "😀", joined, interpolated).length
"#;
    for (name, expected) in [
        ("high", vec![0xd83d]),
        ("low", vec![0xde00]),
        ("pair", vec![0xd83d, 0xde00]),
        ("reverse", vec![0xde00, 0xd83d]),
        ("joined", vec![0xd83d, 0xde00]),
        ("interpolated", vec![0xd83d, 0xde00]),
        ("literal", vec![0xd83d]),
    ] {
        assert_eq!(
            units(pkl_runtime::probe::evaluate_member(source, name).unwrap()),
            expected,
            "{name}"
        );
    }
    for (name, expected) in [
        ("lookup", 110),
        ("distinct", 4),
        ("pairKey", 42),
        ("pairDistinct", 1),
    ] {
        assert_eq!(
            pkl_runtime::probe::evaluate_member(source, name),
            Ok(Value::Int(expected)),
            "{name}"
        );
    }
}

#[test]
fn exported_arrow_metadata_keeps_unmatched_literal_units_distinct() {
    let source = r#"
import "pkl:ref"
class D extends ref.Domain {}
d = new D {}
class High { p: ("\u{d83d}") -> Int }
class Low { p: ("\u{de00}") -> Int }
class Question { p: ("?") -> Int }
class Pair { p: ("\u{d83d}\u{de00}") -> Int }
class Scalar { p: ("😀") -> Int }
high = ref.Reference(d, High, null).p
low = ref.Reference(d, Low, null).p
question = ref.Reference(d, Question, null).p
pair = ref.Reference(d, Pair, null).p
scalar = ref.Reference(d, Scalar, null).p
distinct = high != low && high != question && low != question
keys = Map(high, 11, low, 22, question, 33)
count = keys.length
lookup = keys[low]
same = pair == scalar
sameKey = Map(pair, 42)[scalar]
"#;
    for name in ["distinct", "same"] {
        assert_eq!(
            pkl_runtime::probe::evaluate_member(source, name),
            Ok(Value::Bool(true)),
            "{name}"
        );
    }
    for (name, expected) in [("count", 3), ("lookup", 22), ("sameKey", 42)] {
        assert_eq!(
            pkl_runtime::probe::evaluate_member(source, name),
            Ok(Value::Int(expected)),
            "{name}"
        );
    }
}

fn program(source: &str) -> (Arc<Program>, [usize; 3]) {
    let mut program = (*compile_module(&resolve_module(&parse_module(source).unwrap()).unwrap())
        .unwrap())
    .clone();
    let entries = ["joined", "inspect", "applied"].map(|name| {
        let block = program.blocks.len();
        program
            .blocks
            .push(vec![I::Input(0), I::GetMember(name.into()), I::Return]);
        block
    });
    (Arc::new(program), entries)
}

fn event(session: &mut HostSession, module: &Value, now: u64) -> HostPoll {
    for _ in 0..200_000 {
        session.collect(std::slice::from_ref(module));
        match session.poll(now, 1) {
            HostPoll::Yielded => {}
            event => return event,
        }
    }
    panic!("lossless string Host work did not settle");
}

fn ready(session: &mut HostSession, module: &Value, now: u64) -> Result<Value, RuntimeError> {
    match event(session, module, now) {
        HostPoll::Ready(result) => result,
        event => panic!("expected Host result, got {event:?}"),
    }
}

fn read(session: &mut HostSession, module: &Value, now: u64, expected: &str) -> String {
    match event(session, module, now) {
        HostPoll::Read { token, uri } => {
            assert_eq!(&*uri, expected);
            token
        }
        event => panic!("expected Host read, got {event:?}"),
    }
}

#[test]
fn captured_units_survive_host_failure_reentry_collection_and_fresh_evaluators() {
    let source = r#"
high = "\u{d83d}"
low = "\u{de00}"
joined = "\(high)\(read("mem:tail"))"
keys = Map(high, 11, low, 22, "\u{fffd}", 33, "?", 44)
inspect = keys[high] + keys[low] + keys["\u{fffd}"] + keys["?"]
closure = let (captured = high) () -> "\(captured)\(read("mem:closure"))"
applied = closure.apply()
"#;
    let (program, [joined, inspect, applied]) = program(source);
    for repetition in 0..2 {
        let mut session = HostSession::new(program.clone());
        session.start(program.entry, vec![], 0, None).unwrap();
        let module = ready(&mut session, &Value::Null, 0).unwrap();
        session
            .start(joined, vec![module.clone()], 1, None)
            .unwrap();
        let outer = read(&mut session, &module, 1, "mem:tail");
        session
            .reenter(&outer, inspect, vec![module.clone()], 2, None)
            .unwrap();
        assert_eq!(ready(&mut session, &module, 2), Ok(Value::Int(110)));
        assert_eq!(read(&mut session, &module, 2, "mem:tail"), outer);

        session
            .reenter(&outer, applied, vec![module.clone()], 3, None)
            .unwrap();
        let failed = read(&mut session, &module, 3, "mem:closure");
        session
            .resume(
                &failed,
                Err(RuntimeError::new(ErrorKind::Host, "string retry")),
                3,
            )
            .unwrap();
        let error = ready(&mut session, &module, 3).unwrap_err();
        assert_eq!(
            (error.kind, error.message.as_str()),
            (ErrorKind::Host, "string retry")
        );
        assert_eq!(session.context_depth(), 1);

        session
            .reenter(&outer, applied, vec![module.clone()], 4, None)
            .unwrap();
        let retry = read(&mut session, &module, 4, "mem:closure");
        assert_ne!(failed, retry);
        let tail = PklString::from_utf16([0xde00]);
        session
            .resume(&retry, Ok(Value::string(tail.clone())), 4)
            .unwrap();
        assert_eq!(
            units(ready(&mut session, &module, 4).unwrap()),
            [0xd83d, 0xde00]
        );
        assert_eq!(read(&mut session, &module, 4, "mem:tail"), outer);
        session.resume(&outer, Ok(Value::string(tail)), 5).unwrap();
        assert_eq!(
            units(ready(&mut session, &module, 5).unwrap()),
            [0xd83d, 0xde00]
        );
        session
            .start(inspect, vec![module.clone()], 6, None)
            .unwrap();
        assert_eq!(ready(&mut session, &module, 6), Ok(Value::Int(110)));
        session.close();
        assert_eq!(
            session.heap_stats().slots,
            0,
            "fresh evaluator {repetition}"
        );
    }
}

#[test]
fn terminal_events_release_suspended_string_captures_and_reject_late_tails() {
    let source = r#"
high = "\u{d83d}"
joined = "\(high)\(read("mem:tail"))"
inspect = 42
closure = let (captured = high) () -> "\(captured)\(read("mem:closure"))"
applied = closure.apply()
"#;
    let (program, [joined, _, applied]) = program(source);
    for kind in [ErrorKind::Cancelled, ErrorKind::Closed, ErrorKind::Timeout] {
        let mut session = HostSession::new(program.clone());
        session.start(program.entry, vec![], 0, None).unwrap();
        let module = ready(&mut session, &Value::Null, 0).unwrap();
        session
            .start(
                joined,
                vec![module.clone()],
                1,
                (kind == ErrorKind::Timeout).then_some(5),
            )
            .unwrap();
        let outer = read(&mut session, &module, 1, "mem:tail");
        session
            .reenter(&outer, applied, vec![module.clone()], 2, None)
            .unwrap();
        let inner = read(&mut session, &module, 2, "mem:closure");
        match kind {
            ErrorKind::Cancelled => session.cancel(),
            ErrorKind::Closed => session.close(),
            ErrorKind::Timeout => {}
            _ => unreachable!(),
        }
        let now = if kind == ErrorKind::Timeout { 6 } else { 3 };
        assert_eq!(ready(&mut session, &module, now).unwrap_err().kind, kind);
        assert_eq!(session.heap_stats().slots, 0);
        assert_eq!(session.context_depth(), 0);
        assert_eq!(
            session.drain_cancelled_reads(),
            [outer.clone(), inner.clone()]
        );
        for token in [outer, inner] {
            assert_eq!(
                session
                    .resume(
                        &token,
                        Ok(Value::string(PklString::from_utf16([0xde00]))),
                        now
                    )
                    .unwrap_err()
                    .kind,
                ErrorKind::Closed
            );
        }
    }
}
