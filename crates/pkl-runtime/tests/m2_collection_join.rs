//! Development regressions; compatibility is checked with separate Java captures.
use pkl_runtime::{
    evaluation::frontend::{ModuleEvalOptions, ModuleSourceInput, evaluate_module_output},
    probe::SourceProbe,
    resources::ReadResult,
    value::PklString,
};

fn output(source: &str) -> Vec<u8> {
    let result = evaluate_module_output(
        ModuleSourceInput {
            text: source,
            uri: "file:///join.pkl",
            inferred_name: "join",
        },
        &ModuleEvalOptions::default(),
    );
    assert_eq!(
        result.exit_code,
        0,
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(result.stderr.is_empty());
    result.stdout
}

#[test]
fn strict_collection_join_preserves_order_and_java_value_stringification() {
    assert_eq!(output(r#"output { text = List().join("unused") }"#), b"");
    assert_eq!(
        output(
            r#"output { text = List("x", 2, true, null, "AQI=".base64DecodedBytes).join("|") }"#
        ),
        b"x|2|true|null|Bytes(1, 2)"
    );
    assert_eq!(
        output(r#"output { text = Set("b", "a", "b").join("/") }"#),
        b"b/a"
    );
    assert_eq!(
        output(r#"output { text = List("123456789".encodeToBytes("UTF-8")).join("") }"#),
        b"Bytes(49, 50, 51, 52, 53, 54, 55, 56, 57)"
    );
    assert_eq!(
        output(
            r#"
class Holder { x = 1; function toString(): String = throw("virtual override") }
output { text = List(new Holder {}).join(",") }
"#
        ),
        b"new Holder { x = 1 }"
    );
    assert_eq!(
        output(r#"output { text = List(List(new Dynamic { x = throw("unforced") })).join(",") }"#),
        b"List(new Dynamic {\n  x = ?\n})"
    );
}

#[test]
fn join_suspends_for_each_object_and_keeps_remaining_values_alive() {
    let mut probe = SourceProbe::new_expressions_text(
        r#"result = List(new Dynamic { x = read("join:first") }, new Dynamic { x = read("join:second") }).join("|")"#,
        "file:///join.pkl", "join", &["result".into()], "compact", false, false, true,
    ).unwrap();
    probe.start_path(0, 0, None).unwrap();
    let mut reads = Vec::new();
    let mut result = None;
    for _ in 0..100_000 {
        let event = probe.poll(0, 1).unwrap();
        probe.collect();
        match event.kind {
            "yield" => (),
            "read" => {
                reads.push(event.uri.unwrap());
                probe
                    .complete_read(
                        event.token.as_deref().unwrap(),
                        Ok(ReadResult::Text(PklString::from_utf16(vec![0xd800]))),
                        0,
                    )
                    .unwrap();
            }
            "value" => {
                result = event.value;
                break;
            }
            other => panic!("unexpected {other}: {event:?}"),
        }
    }
    assert_eq!(reads, ["join:first", "join:second"]);
    let one = "new Dynamic { x = \""
        .encode_utf16()
        .chain([0xd800])
        .chain("\" }".encode_utf16())
        .collect::<Vec<_>>();
    let expected = one
        .iter()
        .copied()
        .chain([b'|' as u16])
        .chain(one.iter().copied())
        .collect::<Vec<_>>();
    let owned = result.expect("join completion");
    assert_eq!(owned, expected);
    probe.close();
    assert_eq!(probe.collect().slots, 0);
    assert_eq!(owned, expected);
}
