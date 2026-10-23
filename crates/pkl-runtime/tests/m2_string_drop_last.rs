//! Shared UTF-16 and suspension contracts; full Java diagnostics are compared separately.
use pkl_runtime::{
    probe::{SourceEvent, SourceProbe, evaluate_member},
    resources::ReadResult,
    value::{PklString, Value},
};

#[test]
fn drop_last_counts_surrogate_pairs_and_preserves_unmatched_units() {
    for (count, units) in [
        (0, vec![97, 0xd800, 0xdc00, 0xdbff, 98]),
        (1, vec![97, 0xd800, 0xdc00, 0xdbff]),
        (2, vec![97, 0xd800, 0xdc00]),
        (3, vec![97]),
        (4, vec![]),
        (i64::MAX, vec![]),
    ] {
        let source = format!(r#"result = "a\u{{10000}}\u{{dbff}}b".dropLast({count})"#);
        assert_eq!(
            evaluate_member(&source, "result"),
            Ok(Value::String(PklString::from_utf16(units)))
        );
    }
}

fn next(probe: &mut SourceProbe) -> SourceEvent {
    for _ in 0..100_000 {
        let event = probe.poll(0, 1).unwrap();
        probe.collect();
        if event.kind != "yield" {
            return event;
        }
    }
    panic!("unfinished String.dropLast evaluation");
}

#[test]
fn pending_count_keeps_the_lossless_receiver_and_owned_result() {
    let expressions = vec!["result".to_owned()];
    let mut probe = SourceProbe::new_expressions_text(
        r#"result = read("mem:receiver").dropLast(if (read("mem:count") == "one") 1 else 0)"#,
        "repl:text",
        "main",
        &expressions,
        "compact",
        false,
        false,
        true,
    )
    .unwrap();
    probe.start_path(0, 0, None).unwrap();
    for (uri, value) in [
        (
            "mem:receiver",
            PklString::from_utf16(vec![97, 0xd800, 0xdc00, 0xdbff, 98]),
        ),
        ("mem:count", "one".into()),
    ] {
        let event = next(&mut probe);
        assert_eq!(event.kind, "read");
        let token = event.token.unwrap();
        assert_eq!(probe.resolved_read_uri(&token).unwrap(), uri);
        probe
            .complete_read(&token, Ok(ReadResult::Text(value)), 0)
            .unwrap();
    }
    let event = next(&mut probe);
    assert_eq!(event.kind, "value");
    let result = event.value.unwrap();
    probe.close();
    assert_eq!(probe.collect().slots, 0);
    assert_eq!(result, [97, 0xd800, 0xdc00, 0xdbff]);
}
