//! Source-derived native controls. Java raw oracle comparisons are separate.
use pkl_runtime::{
    evaluation::{
        frontend::{ModuleEvalOptions, ModuleSourceInput, evaluate_module_output},
        session::{SourceSession, SourceSessionOptions},
    },
    host::HostPoll,
    modules::{ModuleRequestKind, default_completion},
    resources::ReadResult,
    value::{ErrorKind, PklString, Value},
};
fn output(source: &str, options: &ModuleEvalOptions) -> Vec<u8> {
    let result = evaluate_module_output(
        ModuleSourceInput {
            text: source,
            uri: "file:///stage141-xml/main.pkl",
            inferred_name: "main",
        },
        options,
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
fn document_layout_and_default_format_share_authored_renderer() {
    let source = r#"import "pkl:xml"
output { renderer=new xml.Renderer {}; value=new Dynamic { a=1; b=List("x",null,"y"); absent=null } }
"#;
    assert_eq!(
        output(source, &Default::default()),
        b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<root>\n  <a>1</a>\n  <b>xy</b>\n</root>\n"
    );
    let options = ModuleEvalOptions {
        properties: vec![("pkl.outputFormat".into(), "xml".into())],
        ..Default::default()
    };
    assert_eq!(
        output("a=1", &options),
        b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<root>\n  <a>1</a>\n</root>\n"
    );
    assert_eq!(
        output(
            r#"import "pkl:xml"
output {text=new xml.Renderer { converters {[Dynamic]=(_)->"converted"} }.renderValue(List(new Dynamic { boom=throw("premature force") }))}
"#,
            &Default::default()
        ),
        b"converted"
    );
}
#[test]
fn explicit_elements_inline_once_and_directives_are_structural() {
    let source = r#"import "pkl:xml"
output { renderer=new xml.Renderer {}; value=(xml.Element("r")) {
  attributes { ["a"]="<&" }
  xml.Inline(List("x"))
  xml.CData("a]]>b")
  (xml.Element("child")) { isBlockFormat=false; "z" }
} }
"#;
    assert_eq!(output(source,&Default::default()),b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<r a=\"&lt;&amp;\">x<![CDATA[a]]]]><![CDATA[>b]]><child>z</child></r>\n");
    assert_eq!(
        output(
            r#"import "pkl:xml"
output {text=new xml.Renderer {}.renderValue(new Dynamic { p=xml.Inline(List("x")) })}
"#,
            &Default::default()
        ),
        b"x"
    );
}
#[test]
fn property_annotation_uses_original_converter_path_and_canonical_class_identity() {
    let source = r#"import "pkl:xml"
class Holder {
 @xml.Property { name="renamed" } original: Int=7
 hidden ignored=throw("hidden")
}
class Comment { text="ordinary" }
output { renderer=new xml.Renderer { converters { ["original"]=(v)->v+1; ["renamed"]=(_)->throw("wrong path") } }; value=new Holder {} }
"#;
    assert_eq!(
        output(source, &Default::default()),
        b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<root>\n  <renamed>8</renamed>\n</root>\n"
    );
    assert_eq!(
        output(
            r#"import "pkl:xml"
class Comment { text="ordinary" }
output {text=new xml.Renderer {}.renderValue(new Comment {})}
"#,
            &Default::default()
        ),
        b"<text>ordinary</text>"
    );
}
fn session(source: &str) -> SourceSession {
    SourceSession::compile(
        source,
        "file:///stage141-xml/main.pkl",
        "main",
        &["result".to_owned()],
        SourceSessionOptions::default(),
    )
    .unwrap()
}
fn module(s: &mut SourceSession, token: &str, request: &pkl_runtime::modules::ModuleRequest) {
    assert!(!matches!(
        request.kind,
        ModuleRequestKind::LoadSource { .. }
    ));
    s.complete_module(
        token,
        default_completion(request).expect("bundled module"),
        0,
    )
    .unwrap();
}
#[test]
fn renderer_metadata_attribute_and_content_roots_survive_suspension() {
    let source = r#"import "pkl:xml"
local renderer=new xml.Renderer {
 converters { [Int]=(v)->(xml.Element(read("xml:name"))) {
   attributes { ["a"]=read("xml:attribute") }
   (xml.Comment(read("xml:comment"))) { isBlockFormat=false }
   xml.CData(read("xml:cdata"))
   new RenderDirective {text=read("xml:raw");bytes=throw("unused")}
 } }
}
result=renderer.renderValue(7)
"#;
    let mut s = session(source);
    s.start_path(0, 0, None).unwrap();
    let mut reads = Vec::new();
    let mut result = None;
    let mut active = false;
    for _ in 0..100_000 {
        match s.poll(0, if active { 16 } else { 10_000 }) {
            HostPoll::Yielded => {
                if active {
                    s.collect(&[]);
                }
            }
            HostPoll::Module { token, request } => module(&mut s, &token, &request),
            HostPoll::Read { token, uri } => {
                s.collect(&[]);
                reads.push(uri.clone());
                let text = match uri.as_str() {
                    Some("xml:name") => "r".into(),
                    Some("xml:attribute") => "&".into(),
                    Some("xml:comment") => "c".into(),
                    Some("xml:cdata") => PklString::from_utf16(vec![0xd800]),
                    Some("xml:raw") => PklString::from_utf16(vec![0xdc00]),
                    _ => panic!("{uri:?}"),
                };
                s.complete_read(&token, Ok(ReadResult::Text(text)), 0)
                    .unwrap();
                s.collect(&[]);
                active = true;
            }
            HostPoll::Ready(value) => {
                result = Some(value.unwrap());
                break;
            }
            other => panic!("{other:?}"),
        }
    }
    assert_eq!(
        reads,
        [
            "xml:name",
            "xml:attribute",
            "xml:comment",
            "xml:cdata",
            "xml:raw"
        ]
        .map(PklString::from)
    );
    let mut expected = "<r a=\"&amp;\"><!--c--><![CDATA["
        .encode_utf16()
        .collect::<Vec<_>>();
    expected.push(0xd800);
    expected.extend("]]>".encode_utf16());
    expected.push(0xdc00);
    expected.extend("</r>".encode_utf16());
    let Value::String(text) = result.expect("settled") else {
        panic!("expression string")
    };
    assert_eq!(text.encode_utf16().collect::<Vec<_>>(), expected);
    s.close();
    assert_eq!(s.collect(&[]).slots, 0);
    assert_eq!(text.encode_utf16().collect::<Vec<_>>(), expected);
}
#[test]
fn element_force_order_does_not_convert_direct_children() {
    let source = r#"import "pkl:xml"
result=new xml.Renderer { converters { [Int]=(_)->throw("must not convert explicit child") } }.renderValue((xml.Element("r")) {
 ignored=read("xml:ignored")
 7
})
"#;
    let mut s = session(source);
    s.start_path(0, 0, None).unwrap();
    let mut reads = 0;
    let mut result = None;
    for _ in 0..100_000 {
        match s.poll(0, 10_000) {
            HostPoll::Yielded => (),
            HostPoll::Module { token, request } => module(&mut s, &token, &request),
            HostPoll::Read { token, uri } => {
                assert_eq!(uri, "xml:ignored");
                reads += 1;
                s.collect(&[]);
                s.complete_read(&token, Ok(ReadResult::Text("forced".into())), 0)
                    .unwrap();
            }
            HostPoll::Ready(value) => {
                result = Some(value.unwrap());
                break;
            }
            event => panic!("{event:?}"),
        }
    }
    assert_eq!(reads, 1);
    assert_eq!(result, Some(Value::string("<r>7</r>")));
    s.close();
    assert_eq!(s.collect(&[]).slots, 0);
}
#[test]
fn cancel_pending_element_attribute_discards_render_continuation() {
    let mut s = session(
        r#"import "pkl:xml"
result=new xml.Renderer {}.renderValue((xml.Element("r")) {attributes {["a"]=read("xml:pending")}; "x"})
"#,
    );
    s.start_path(0, 0, None).unwrap();
    let mut pending = None;
    for _ in 0..100_000 {
        match s.poll(0, 10_000) {
            HostPoll::Yielded => (),
            HostPoll::Module { token, request } => module(&mut s, &token, &request),
            HostPoll::Read { token, uri } => {
                assert_eq!(uri, "xml:pending");
                s.collect(&[]);
                pending = Some(token);
                break;
            }
            event => panic!("{event:?}"),
        }
    }
    let token = pending.expect("attribute suspension");
    s.cancel();
    s.collect(&[]);
    let HostPoll::Ready(Err(error)) = s.poll(0, 10_000) else {
        panic!("cancelled")
    };
    assert_eq!(error.kind, ErrorKind::Cancelled);
    assert!(
        s.complete_read(&token, Ok(ReadResult::Text("late".into())), 0)
            .is_err()
    );
    s.close();
    assert_eq!(s.collect(&[]).slots, 0);
}
