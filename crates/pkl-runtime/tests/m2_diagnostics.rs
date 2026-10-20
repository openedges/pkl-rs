use pkl_runtime::diagnostic::{RenderFrame, render_error};
use pkl_runtime::source::{SourceMap, Span};

#[test]
fn uncolored_error_layout_preserves_unicode_columns_and_hint_position() {
    let mut sources = SourceMap::default();
    let id = sources.add("module", "  result = \"😀\" + missing\n");
    let expression = sources.add("expression", "result");
    let source = sources.get(id).unwrap();
    let start = source.text().find("missing").unwrap();
    let frames = [
        RenderFrame {
            source,
            uri: "file:///module.pkl",
            member: "module#result",
            span: Span::new(start, start + 7),
            highlight_length_utf16: None,
        },
        RenderFrame {
            source: sources.get(expression).unwrap(),
            uri: "repl:text",
            member: "",
            span: Span::new(0, 6),
            highlight_length_utf16: None,
        },
    ];
    assert_eq!(
        render_error("Cannot find property `missing`.", &frames, Some("A hint.")),
        "–– Pkl Error ––\nCannot find property `missing`.\n\n1 | result = \"😀\" + missing\n                    ^^^^^^^\nat module#result (file:///module.pkl)\n\nA hint.\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
    );
}

#[test]
fn multiline_frames_underline_only_the_first_source_line() {
    let mut sources = SourceMap::default();
    let id = sources.add("module", "  result = if (\n    true) 1 else 2\n");
    assert_eq!(
        render_error(
            "failure",
            &[RenderFrame {
                source: sources.get(id).unwrap(),
                uri: "file:///module.pkl",
                member: "module#result",
                span: Span::new(11, 33),
                highlight_length_utf16: None,
            }],
            None,
        ),
        "–– Pkl Error ––\nfailure\n\n1 | result = if (\n             ^^^^\nat module#result (file:///module.pkl)\n"
    );
}

#[test]
fn errors_without_language_frames_keep_messages_and_hints() {
    assert_eq!(
        render_error("First line.\nValue: 42", &[], Some("A hint.")),
        "–– Pkl Error ––\nFirst line.\nValue: 42\n\nA hint.\n"
    );
}
