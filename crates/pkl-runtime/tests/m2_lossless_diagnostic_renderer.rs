use pkl_runtime::diagnostic::{RenderFrame, render_error_text};
use pkl_runtime::source::{SourceMap, Span};
use pkl_runtime::value::PklString;

#[test]
fn canonical_message_hint_and_frame_preserve_distinct_utf16_units() {
    let payload = PklString::from_utf16(vec![0xd800, 0xdc00, 0xd800, 0x3f, 0xfffd, 0]);
    let mut sources = SourceMap::default();
    let id = sources.add("file:///test.pkl", "value");
    let frames = [RenderFrame {
        source: sources.get(id).unwrap(),
        uri: "file:///test.pkl",
        member: &payload,
        span: Span::new(0, 5),
        highlight_length_utf16: None,
    }];
    let actual = render_error_text(&payload, &frames, Some(&payload));
    let mut expected: Vec<u16> = "–– Pkl Error ––\n".encode_utf16().collect();
    expected.extend(payload.encode_utf16());
    expected.extend("\n\n1 | value\n    ^^^^^\nat ".encode_utf16());
    expected.extend(payload.encode_utf16());
    expected.extend(" (file:///test.pkl)\n\n".encode_utf16());
    expected.extend(payload.encode_utf16());
    expected.push(10);
    assert_eq!(actual.encode_utf16().collect::<Vec<_>>(), expected);
    assert_eq!(
        actual.to_java_utf8(),
        "–– Pkl Error ––\n𐀀??�\0\n\n1 | value\n    ^^^^^\nat 𐀀??�\0 (file:///test.pkl)\n\n𐀀??�\0\n"
    );
}
