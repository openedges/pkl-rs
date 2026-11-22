//! Actual pinned Java lexer observations; parser/API checks remain separate.
use super::*;
use crate::source::SourceMap;
use std::{fs, path::Path};

fn units(bytes: &[u8]) -> Vec<u16> {
    let (pairs, tail) = bytes.as_chunks::<2>();
    assert!(tail.is_empty());
    pairs.iter().map(|pair| u16::from_be_bytes(*pair)).collect()
}

#[test]
fn java_shebang_tokens_and_raw_prefix_failures_preserve_utf16() {
    let root =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/m2-shebang-contract");
    let index = fs::read_to_string(root.join("cases.tsv")).unwrap();
    let mut compared = 0;
    for line in index.lines() {
        let (id, _) = line.split_once('\t').unwrap();
        // The Rust string lexer coalesces complete interpolated strings;
        // nested SHEBANG rejection is covered by the actual parser/API tests.
        if id.ends_with("reject-interpolation") || id.ends_with("expression-interpolation") {
            continue;
        }
        compared += 1;
        let directory = root.join(id);
        let source = fs::read_to_string(directory.join("source.pkl")).unwrap();
        let mut map = SourceMap::default();
        let source_id = map.add(id, source.clone());
        let mapped = map.get(source_id).unwrap();
        let mut lexer = Lexer {
            source: source.as_str().into(),
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
                    if token.kind == Kind::Shebang {
                        observed.push((
                            mapped.utf16_span(token.span).unwrap(),
                            source[token.span.start..token.span.end]
                                .encode_utf16()
                                .collect::<Vec<_>>(),
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
        assert!(eof || failure.is_some(), "lexer bound: {id}");
        let tokens = fs::read_to_string(directory.join("lexer/tokens.tsv")).unwrap();
        let expected = tokens
            .lines()
            .filter(|line| line.starts_with("SHEBANG\t"))
            .map(|line| {
                let columns = line.split('\t').collect::<Vec<_>>();
                let text = columns[7]
                    .as_bytes()
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .map(|unit| {
                        u16::from_str_radix(std::str::from_utf8(unit).unwrap(), 16).unwrap()
                    })
                    .collect::<Vec<_>>();
                (
                    (
                        columns[1].parse::<usize>().unwrap(),
                        columns[2].parse::<usize>().unwrap(),
                    ),
                    text,
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(observed, expected, "SHEBANG tokens: {id}");
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
            let expected_span = fs::read_to_string(directory.join("lexer/error.span.tsv")).unwrap();
            let span = expected_span
                .split_whitespace()
                .map(|s| s.parse::<usize>().unwrap())
                .collect::<Vec<_>>();
            let (start, length) = mapped.utf16_span(error.span).unwrap();
            assert_eq!(
                (start, error.highlight_length_utf16.unwrap_or(length)),
                (span[0], span[1]),
                "lexer span: {id}"
            );
            assert_eq!(
                error.message.encode_utf16().collect::<Vec<_>>(),
                units(&fs::read(directory.join("lexer/error.message.code-units.bin")).unwrap()),
                "lexer message units: {id}"
            );
            assert_eq!(
                error.message.to_java_utf8().as_bytes(),
                fs::read(directory.join("lexer/error.message.bin")).unwrap(),
                "lexer message bytes: {id}"
            );
        }
    }
    assert_eq!(compared, 60);
}
