//! Three Java-first Stage83 programs preserve first-read/retry evaluator boundaries.
#[path = "support/module_keys.rs"]
mod module_keys;

use pkl_runtime::probe::SourceProbe;
use pkl_runtime::value::PklString;
use std::fs;
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/m2-duplicate-class-owner-edge-oracle")
}

fn matches_text(row: &Path, name: &str, actual: &[u16]) -> bool {
    let raw = fs::read(row.join(format!("{name}.code-units.bin"))).unwrap();
    let (pairs, tail) = raw.as_chunks::<2>();
    assert!(tail.is_empty());
    let expected = pairs
        .iter()
        .map(|pair| u16::from_be_bytes(*pair))
        .collect::<Vec<_>>();
    expected == actual
        && fs::read(row.join(format!("{name}.bin"))).unwrap()
            == PklString::from_utf16(actual.to_vec())
                .to_java_utf8()
                .as_bytes()
}

fn run(program: &str) {
    let input = root().join(program);
    let uri = fs::read_to_string(input.join("uri.txt")).unwrap();
    let source = fs::read_to_string(input.join("module.pkl")).unwrap();
    let queries = fs::read_to_string(input.join("queries.tsv")).unwrap();
    for repeat in 0..2 {
        let mut probe = SourceProbe::new_module_evaluator("compact", false, false, false).unwrap();
        for (index, line) in queries.lines().enumerate() {
            let columns = line.split('\t').collect::<Vec<_>>();
            assert_eq!(columns.len(), 3);
            assert_eq!(columns[0], "string");
            let id = columns[1];
            let expression = columns[2];
            let expected = input
                .join(format!("repeat-{repeat}"))
                .join(format!("{index:03}-{id}"));
            assert_eq!(
                fs::read(expected.join("expression.bin")).unwrap(),
                expression.as_bytes()
            );
            probe
                .evaluate_module_source(&uri, Some(&source), expression, 0, None)
                .unwrap();
            let mut logs = 0usize;
            let mut terminal = None;
            for _ in 0..200_000 {
                probe.collect();
                let event = probe.poll(0, 16).unwrap();
                probe.collect();
                if event.kind == "yield" {
                    continue;
                }
                if event.kind == "module-request" {
                    assert!(
                        module_keys::complete_probe(&mut probe, event.token.as_deref().unwrap(), 0)
                            .unwrap(),
                        "{program}/{id} unexpected Host request"
                    );
                    continue;
                }
                if event.kind == "log" {
                    logs += 1;
                    probe
                        .acknowledge_log(event.token.as_deref().unwrap(), None, 0)
                        .unwrap();
                    continue;
                }
                terminal = Some(event);
                break;
            }
            let terminal = terminal.expect("bounded Stage83 edge poll");
            assert_eq!(
                fs::read_to_string(expected.join("logs.count.bin")).unwrap(),
                format!("{logs}\n"),
                "{program}/{id} repeat {repeat} logger count"
            );
            assert_eq!(
                fs::read(expected.join("result.status.bin")).unwrap(),
                b"error\n",
                "{program}/{id} Java status"
            );
            assert_eq!(
                terminal.kind, "error",
                "{program}/{id} repeat {repeat} status"
            );
            assert_eq!(terminal.disposition, Some("language-error"));
            assert!(
                matches_text(
                    &expected,
                    "result.stdout",
                    terminal.value.as_deref().unwrap_or(&[])
                ),
                "{program}/{id} repeat {repeat} stdout"
            );
            assert!(
                matches_text(
                    &expected,
                    "result.stderr",
                    terminal.error.as_deref().unwrap_or(&[])
                ),
                "{program}/{id} repeat {repeat} stderr"
            );
        }
        probe.close();
        assert_eq!(probe.collect().slots, 0);
    }
}

#[test]
fn java_three_duplicate_owner_programs_match_six_ordered_rows() {
    for program in ["quoted-class", "nested-body", "body-before-duplicate"] {
        run(program);
    }
}
