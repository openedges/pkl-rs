//! Full actual Java parser/API channels. Process/cause classes stay provenance.
#[path = "support/module_keys.rs"]
mod module_keys;
use pkl_runtime::{
    parser::{parse_expression, parse_module},
    probe::SourceProbe,
    source::SourceMap,
    value::PklString,
};
use std::{
    fs,
    path::{Path, PathBuf},
};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/m2-shebang-contract")
}
fn units(bytes: &[u8]) -> Vec<u16> {
    let (pairs, tail) = bytes.as_chunks::<2>();
    assert!(tail.is_empty());
    pairs.iter().map(|pair| u16::from_be_bytes(*pair)).collect()
}
fn field(directory: &Path, name: &str, actual: &[u16]) {
    assert_eq!(
        units(&fs::read(directory.join(format!("{name}.code-units.bin"))).unwrap()),
        actual,
        "{} {name}: UTF16",
        directory.display()
    );
    assert_eq!(
        fs::read(directory.join(format!("{name}.bin"))).unwrap(),
        PklString::from_utf16(actual.to_vec())
            .to_java_utf8()
            .as_bytes(),
        "{} {name}: UTF8",
        directory.display()
    );
}

#[test]
fn actual_java_module_and_expression_parser_entry_contracts() {
    let root = root();
    let index = fs::read_to_string(root.join("cases.tsv")).unwrap();
    assert_eq!(index.lines().count(), 62);
    for line in index.lines() {
        let (id, entry) = line.split_once('\t').unwrap();
        let directory = root.join(id);
        let text = fs::read_to_string(directory.join("source.pkl")).unwrap();
        assert_eq!(
            text.encode_utf16().collect::<Vec<_>>(),
            units(&fs::read(directory.join("source.code-units.bin")).unwrap())
        );
        let result = if entry == "module" {
            parse_module(&text).map(|_| ())
        } else {
            assert_eq!(entry, "expression");
            parse_expression(&text).map(|_| ())
        };
        assert_eq!(
            fs::read(directory.join("parser/status.bin")).unwrap(),
            if result.is_ok() {
                b"success\n".as_slice()
            } else {
                b"error\n".as_slice()
            },
            "parser status: {id}"
        );
        if let Err(error) = result {
            field(
                &directory.join("parser"),
                "error.message",
                &error.message.encode_utf16().collect::<Vec<_>>(),
            );
            let mut map = SourceMap::default();
            let source = map.add(id, text);
            let (start, length) = map.get(source).unwrap().utf16_span(error.span).unwrap();
            let span = fs::read_to_string(directory.join("parser/error.span.tsv"))
                .unwrap()
                .split_whitespace()
                .map(|v| v.parse::<usize>().unwrap())
                .collect::<Vec<_>>();
            assert_eq!(
                (start, error.highlight_length_utf16.unwrap_or(length)),
                (span[0], span[1]),
                "parser effective UTF16 span: {id}"
            );
        }
    }
}

#[test]
fn actual_java_public_module_output_and_expression_string_channels() {
    let root = root();
    let index = fs::read_to_string(root.join("cases.tsv")).unwrap();
    assert_eq!(index.lines().count(), 62);
    for line in index.lines() {
        let (id, entry) = line.split_once('\t').unwrap();
        let directory = root.join(id);
        let text = fs::read_to_string(directory.join("source.pkl")).unwrap();
        let uri = fs::read_to_string(directory.join("uri.txt")).unwrap();
        let mut probe = SourceProbe::new_module_evaluator("compact", true, false, false).unwrap();
        // Java evaluateExpressionString special-cases output.text by calling
        // evaluateOutputText directly. Only module rows use that operation;
        // expression rows retain the original expression and an empty module.
        let (module, expression) = if entry == "module" {
            (text.as_str(), "output.text")
        } else {
            assert_eq!(entry, "expression");
            ("", text.as_str())
        };
        probe
            .evaluate_module_source(&uri, Some(module), expression, 0, None)
            .unwrap();
        let mut terminal = None;
        for _ in 0..1_000_000_u32.div_ceil(64) {
            probe.collect();
            let event = probe.poll(0, 64).unwrap();
            probe.collect();
            match event.kind {
                "yield" => continue,
                "module-request" => {
                    assert!(
                        module_keys::complete_probe(&mut probe, event.token.as_deref().unwrap(), 0)
                            .unwrap()
                    );
                    continue;
                }
                "read" => {
                    assert_eq!(event.uri.as_deref(), Some("prop:pkl.outputFormat"));
                    probe
                        .complete_read_missing(event.token.as_deref().unwrap(), 0)
                        .unwrap();
                    continue;
                }
                "value" | "error" => {
                    terminal = Some(event);
                    break;
                }
                _ => panic!("unexpected Host effect for {id}: {event:?}"),
            }
        }
        let event = terminal.unwrap_or_else(|| panic!("bounded shebang API row: {id}"));
        let expected = directory.join("evaluator");
        if event.kind == "value" {
            assert_eq!(fs::read(expected.join("status.bin")).unwrap(), b"success\n");
            assert!(event.error.is_none());
            field(&expected, "string", event.value.as_deref().unwrap());
        } else {
            assert_eq!(event.disposition, Some("language-error"), "{id}: {event:?}");
            assert_eq!(fs::read(expected.join("status.bin")).unwrap(), b"error\n");
            assert!(event.value.is_none());
            field(&expected, "error", event.error.as_deref().unwrap());
        }
        probe.close();
        assert_eq!(
            probe.collect().slots,
            0,
            "closed evaluator owns no retained heap slots: {id}"
        );
    }
}
