//! Actual Java token/diagnostic coordinates, including interior surrogate EOF.
use super::*;
use crate::diagnostic::{PreparedDiagnostic, RenderFrame};
use crate::source::SourceMap;
use std::{fs, path::Path};

fn root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/m2-quoted-identifier-contract")
}
fn units(bytes: &[u8]) -> Vec<u16> {
    let (pairs, tail) = bytes.as_chunks::<2>();
    assert!(tail.is_empty());
    pairs.iter().map(|pair| u16::from_be_bytes(*pair)).collect()
}

#[test]
fn actual_java_quoted_tokens_and_terminator_coordinates() {
    let root = root();
    let index = fs::read_to_string(root.join("cases.tsv")).unwrap();
    assert_eq!(index.lines().count(), 38);
    for line in index.lines() {
        let (id, _) = line.split_once('\t').unwrap();
        let directory = root.join(id);
        let text = fs::read_to_string(directory.join("source.pkl")).unwrap();
        let mut sources = SourceMap::default();
        let source_id = sources.add(id, text.clone());
        let source = sources.get(source_id).unwrap();
        let mut lexer = Lexer {
            source: text.as_str().into(),
            cursor: 0,
            string_errors: Default::default(),
            skip_doc_comments: false,
        };
        let mut observed = Vec::new();
        let mut failure = None;
        let mut eof = false;
        for _ in 0..512 {
            match lexer.next_with_comments(false, true) {
                Ok(token) => {
                    if let Kind::QuotedName(name) = &token.kind {
                        observed.push((
                            source.utf16_span(token.span).unwrap(),
                            name.encode_utf16().collect::<Vec<_>>(),
                        ));
                    }
                    if token.kind == Kind::Eof {
                        eof = true;
                        break;
                    }
                }
                Err(error) => {
                    failure = Some(error);
                    break;
                }
            }
        }
        assert!(eof || failure.is_some(), "lexical bound: {id}");
        let tokens = fs::read_to_string(directory.join("lexer/tokens.tsv")).unwrap();
        let expected = tokens
            .lines()
            .filter_map(|line| {
                let columns = line.split('\t').collect::<Vec<_>>();
                if columns[0] != "IDENTIFIER" || !columns[7].starts_with("0060") {
                    return None;
                }
                let quoted = columns[7]
                    .as_bytes()
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .map(|unit| {
                        u16::from_str_radix(std::str::from_utf8(unit).unwrap(), 16).unwrap()
                    })
                    .collect::<Vec<_>>();
                assert!(quoted.len() >= 2 && quoted.last() == Some(&96));
                Some((
                    (
                        columns[1].parse::<usize>().unwrap(),
                        columns[2].parse::<usize>().unwrap(),
                    ),
                    quoted[1..quoted.len() - 1].to_vec(),
                ))
            })
            .collect::<Vec<_>>();
        assert_eq!(observed, expected, "quoted token: {id}");
        assert_eq!(
            fs::read(directory.join("lexer/status.bin")).unwrap(),
            if failure.is_some() {
                b"error\n".as_slice()
            } else {
                b"success\n".as_slice()
            },
            "lexer status: {id}"
        );
        if let Some(error) = failure {
            let (start, length) = source.utf16_span(error.span).unwrap();
            let expected = fs::read_to_string(directory.join("lexer/error.span.tsv"))
                .unwrap()
                .split_whitespace()
                .map(|v| v.parse::<usize>().unwrap())
                .collect::<Vec<_>>();
            assert_eq!(
                (
                    start + error.highlight_offset_utf16(),
                    error.highlight_length_utf16.unwrap_or(length)
                ),
                (expected[0], expected[1]),
                "Java UTF16 section: {id}"
            );
            assert_eq!(
                error.message.encode_utf16().collect::<Vec<_>>(),
                units(&fs::read(directory.join("lexer/error.message.code-units.bin")).unwrap()),
                "message units: {id}"
            );
            assert_eq!(
                error.message.to_java_utf8().as_bytes(),
                fs::read(directory.join("lexer/error.message.bin")).unwrap(),
                "message bytes: {id}"
            );
        }
    }
}

#[test]
fn origin_offset_precedes_compression_and_preserves_caller_coordinates_and_hint() {
    let directory = root().join("expression-unicode-eof");
    let text = fs::read_to_string(directory.join("source.pkl")).unwrap();
    let error = parse_expression(&text)
        .unwrap_err()
        .with_hint("retained hint");
    assert_eq!(error.highlight_offset_utf16(), 1);
    assert_eq!(error.hint().unwrap().as_str(), Some("retained hint"));
    let mut sources = SourceMap::default();
    let id = sources.add("origin", text);
    let source = sources.get(id).unwrap();
    let member: PklString = "caller#nested".into();
    let frame = || RenderFrame {
        source,
        uri: "file:///same.pkl".into(),
        member: &member,
        span: error.span,
        highlight_length_utf16: None,
    };
    // Identical physical anchors would otherwise be compressed together.
    // Only the origin's Java position differs; the callers stay unadjusted.
    let frames = [frame(), frame(), frame()];
    let mut rendered = PreparedDiagnostic::new_frontend(
        &error.message,
        &frames,
        error.hint(),
        error.highlight_offset_utf16(),
    );
    let mut columns = Vec::new();
    while let Some(frame) = rendered.next_frame().cloned() {
        columns.push((frame.start_column, frame.end_column));
        rendered.complete_frame(frame);
    }
    let expected = fs::read_to_string(directory.join("parser/error.span.tsv"))
        .unwrap()
        .split_whitespace()
        .map(|v| v.parse::<usize>().unwrap())
        .collect::<Vec<_>>();
    let origin = expected[0] + 1;
    let caller = source.position(error.span.start).unwrap().column_utf16;
    assert_eq!(columns, [(origin, origin), (caller, caller)]);
    assert!(rendered.render().to_java_utf8().contains("retained hint"));
}
