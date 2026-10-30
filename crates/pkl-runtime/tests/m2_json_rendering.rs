//! Shared renderer development contracts; pinned Java raw acquisition is separate.
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
            uri: "file:///rendering.pkl",
            inferred_name: "rendering",
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
fn json_shared_traversal_keeps_order_and_omits_only_object_nulls() {
    assert_eq!(
        output(
            r#"
output {
 renderer = new JsonRenderer { indent = "" }
 value = new Dynamic {
  first = new Listing { 1; null; "x" }
  absent = null
  second = Map("a", null, "b", List(2, 3))
  empty = new Dynamic {}
 }
}
"#
        ),
        b"{\"first\":[1,null,\"x\"],\"second\":{\"b\":[2,3]},\"empty\":{}}\n"
    );
    assert_eq!(
        output("output { renderer = new JsonRenderer {}; value = List(1, new Dynamic { a = 2 }) }"),
        b"[\n  1,\n  {\n    \"a\": 2\n  }\n]\n"
    );
}

#[test]
fn directives_share_nested_renderer_continuations_and_do_not_force_bytes() {
    assert_eq!(
        output(
            r#"
class Tag extends ConvertProperty {
 render = (p, renderer) -> Pair(p.key, new RenderDirective {
  text = "= " + (renderer as ValueRenderer).renderValue(p.value) + " // tag"
  bytes = throw("unused bytes")
 })
}
class Holder { @Tag {} x: String = "value" }
output { value = new Holder {} }
"#
        ),
        b"x = \"value\" // tag\n"
    );
    assert_eq!(
        output(
            r#"output {
 renderer = new JsonRenderer {}
 value = new RenderDirective { text = ""; bytes = throw("unused") }
}"#
        ),
        b"\n"
    );
}

#[test]
fn property_conversion_keeps_original_path_and_null_omission_follows_conversion() {
    assert_eq!(
        output(
            r#"
open class Tag extends ConvertProperty {
 suffix: String = "A"
 render = (p, _) -> Pair(p.key + suffix, p.value + 1)
}

class SubTag extends Tag
class Holder { @SubTag {} x = 7; y = 8 }
output {
 value = new Holder {}
 renderer = new JsonRenderer {
  indent = ""
  convertPropertyTransformers { [Tag] { suffix = "T" } }
  converters { ["x"] = (v) -> v + 10; ["xT"] = (_) -> throw("renamed path"); ["y"] = (_) -> null }
 }
}
"#
        ),
        b"{\"xT\":18}\n"
    );
}

#[test]
fn json_property_runs_the_pinned_ordinary_annotation_body() {
    assert_eq!(
        output(
            r#"
import "pkl:json"
class Holder {
 @json.Property { name = "renamed" }
 original: Int = 1
 @json.Property { name = "omitted" }
 nullable: String? = null
}
output { value = new Holder {}; renderer = new JsonRenderer { indent = "" } }
"#
        ),
        b"{\"renamed\":1}\n"
    );
    assert_eq!(
        output(
            r#"
import "pkl:json"
class Holder { @json.Property { name = "renamed" } original: Int = 1 }
output { value = new Holder {} }
"#
        ),
        b"original = 1\n"
    );
}

#[test]
fn suspended_json_and_directive_keep_utf16_and_owned_output_through_collection_and_close() {
    let mut probe = SourceProbe::new_expressions_text(
        r#"result = new JsonRenderer { indent = "" }.renderValue(new Dynamic {
 a = read("life:value")
 b = new RenderDirective { text = read("life:directive"); bytes = throw("unused") }
})"#,
        "file:///rendering.pkl",
        "rendering",
        &["result".into()],
        "compact",
        false,
        false,
        true,
    )
    .unwrap();
    probe.start_path(0, 0, None).unwrap();
    let mut reads = Vec::new();
    let mut output = None;
    for _ in 0..100_000 {
        let event = probe.poll(0, 1).unwrap();
        probe.collect();
        match event.kind {
            "yield" => (),
            "read" => {
                let uri = event.uri.unwrap();
                let unit = if uri == "life:value" {
                    0xd800
                } else {
                    assert_eq!(uri, "life:directive");
                    0xdc00
                };
                reads.push(uri);
                probe
                    .complete_read(
                        event.token.as_deref().unwrap(),
                        Ok(ReadResult::Text(PklString::from_utf16(vec![unit]))),
                        0,
                    )
                    .unwrap();
            }
            "value" => {
                output = event.value;
                break;
            }
            other => panic!("unexpected {other}: {event:?}"),
        }
    }
    assert_eq!(reads, ["life:value", "life:directive"]);
    let mut expected = "{\"a\":\"".encode_utf16().collect::<Vec<_>>();
    expected.push(0xd800);
    expected.extend("\",\"b\":".encode_utf16());
    expected.extend([0xdc00, b'}' as u16]);
    let owned = output.expect("completed render");
    assert_eq!(owned, expected);
    probe.close();
    assert_eq!(probe.collect().slots, 0);
    assert_eq!(owned, expected);
}

#[test]
fn pending_directive_render_releases_all_state_on_cancel_close_and_deadline() {
    use pkl_runtime::{
        evaluation::session::{SourceSession, SourceSessionOptions},
        host::HostPoll,
        value::{ErrorKind, Value},
    };
    for reason in [ErrorKind::Cancelled, ErrorKind::Closed, ErrorKind::Timeout] {
        let mut session = SourceSession::compile_output(
            ModuleSourceInput {
                text: r#"output {
 renderer = new JsonRenderer {}
 value = new RenderDirective { text = read("life:pending"); bytes = throw("unused") }
}"#,
                uri: "file:///rendering.pkl",
                inferred_name: "rendering",
            },
            &[],
            &ModuleEvalOptions::default(),
            SourceSessionOptions::default(),
        )
        .unwrap();
        session.start_output_bytes(0, Some(10)).unwrap();
        let mut pending = None;
        for _ in 0..100_000 {
            let event = session.poll(0, 1);
            session.collect(&[]);
            match event {
                HostPoll::Yielded => (),
                HostPoll::Read { token, uri } => {
                    assert_eq!(uri, "life:pending");
                    pending = Some(token);
                    break;
                }
                other => panic!("unexpected pending renderer event: {other:?}"),
            }
        }
        let token = pending.expect("directive must suspend at its actual text read");
        assert!(session.heap_stats().live > 0);
        match reason {
            ErrorKind::Cancelled => session.cancel(),
            ErrorKind::Closed => session.close(),
            ErrorKind::Timeout => (),
            _ => unreachable!(),
        }
        let now = if reason == ErrorKind::Timeout { 11 } else { 1 };
        let HostPoll::Ready(Err(error)) = session.poll(now, 1) else {
            panic!("terminal renderer error");
        };
        assert_eq!(error.kind, reason);
        assert!(
            session
                .resume(&token, Ok(Value::string("late")), now)
                .is_err()
        );
        session.collect(&[]);
        assert_eq!(session.heap_stats().slots, 0);
        assert_eq!(session.context_depth(), 0);
        assert_eq!(
            session.start_output_bytes(now, None).unwrap_err().kind,
            ErrorKind::Closed
        );
    }
}

#[test]
fn document_hint_keeps_both_objects_across_host_suspension_and_gc() {
    use pkl_runtime::{
        evaluation::session::{SourceSession, SourceSessionOptions},
        host::{HostPoll, IoFailure, ReadFailure},
        value::{ErrorKind, RuntimeError},
    };
    for typed in [true, false] {
        let mut session = SourceSession::compile_output(
            ModuleSourceInput {
                text: r#"output {
 renderer = new JsonRenderer {
  converters { [Dynamic] = (_) -> new Dynamic { pending = read("hint:pending") } }
 }
 value = new Dynamic { old = 7 }
}"#,
                uri: "file:///rendering.pkl",
                inferred_name: "rendering",
            },
            &[],
            &ModuleEvalOptions::default(),
            SourceSessionOptions::default(),
        )
        .unwrap();
        session.start_output_bytes(0, None).unwrap();
        let mut error = None;
        for _ in 0..100_000 {
            let event = session.poll(0, 1);
            session.collect(&[]);
            match event {
                HostPoll::Yielded => (),
                HostPoll::Read { token, uri } => {
                    assert_eq!(uri, "hint:pending");
                    if typed {
                        session
                            .reject_read(
                                &token,
                                ReadFailure::Io(IoFailure::new(Some("reader failed".into()))),
                                0,
                            )
                            .unwrap();
                    } else {
                        session
                            .resume(
                                &token,
                                Err(RuntimeError::new(ErrorKind::Host, "raw Host rejection")),
                                0,
                            )
                            .unwrap();
                    }
                }
                HostPoll::Ready(Err(value)) => {
                    error = Some(value);
                    break;
                }
                other => panic!("unexpected rendering result: {other:?}"),
            }
        }
        assert_eq!(error.expect("failed rendering").hint, typed.then(||
            "This value was converted during rendering. Previous: new Dynamic { old = ? }. After: new Dynamic { pending = ? }.".into()
        ));
        session.close();
        session.collect(&[]);
        assert_eq!(session.heap_stats().slots, 0);
    }
}

#[test]
fn bytes_error_template_hint_takes_precedence_over_document_conversion_hint() {
    use pkl_runtime::{
        evaluation::session::{SourceSession, SourceSessionOptions},
        host::HostPoll,
    };
    for (body, hint) in [
        (
            "value",
            "Consider adding a converter to `output.converters`.",
        ),
        (
            "\"AQ==\".base64DecodedBytes",
            "Consider adding a converter to `output.converters`.",
        ),
    ] {
        let source = format!(
            r#"output {{ renderer = new JsonRenderer {{ converters {{ [Bytes] = (value) -> {body} }} }}; value = "AQ==".base64DecodedBytes }}"#
        );
        let mut session = SourceSession::compile_output(
            ModuleSourceInput {
                text: &source,
                uri: "file:///rendering.pkl",
                inferred_name: "rendering",
            },
            &[],
            &ModuleEvalOptions::default(),
            SourceSessionOptions::default(),
        )
        .unwrap();
        session.start_output_bytes(0, None).unwrap();
        let mut error = None;
        for _ in 0..100_000 {
            let event = session.poll(0, 1);
            session.collect(&[]);
            match event {
                HostPoll::Yielded => (),
                HostPoll::Ready(Err(value)) => {
                    error = Some(value);
                    break;
                }
                other => panic!("unexpected Bytes rendering result: {other:?}"),
            }
        }
        assert_eq!(
            error.expect("Bytes rendering error").hint,
            Some(hint.into())
        );
        session.close();
        session.collect(&[]);
        assert_eq!(session.heap_stats().slots, 0);
    }
}
