//! Whole Java diagnostics for AST lambda ordinals and unavailable read origins.
//! These supplied-source calls do not claim public evaluator policy parity.
use pkl_runtime::{probe::SourceProbe, value::PklString};
use std::path::PathBuf;

fn hex(text: &str) -> Vec<u8> {
    assert_eq!(text.len() % 2, 0);
    text.as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect()
}

fn units(text: &str) -> Vec<u16> {
    let bytes = hex(text);
    assert_eq!(bytes.len() % 2, 0);
    bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u16::from_be_bytes(*pair))
        .collect()
}

#[test]
fn java_logical_owner_and_capability_errors_preserve_every_channel_and_retry() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/m2-logical-capability");
    let cases = std::fs::read_to_string(root.join("cases.tsv")).unwrap();
    let expected = std::fs::read_to_string(root.join("expected.tsv")).unwrap();
    let rows = expected
        .lines()
        .map(|line| line.split('\t').collect::<Vec<_>>())
        .collect::<Vec<_>>();
    let mut compared = 0;
    for case in cases
        .lines()
        .map(|line| line.split('\t').collect::<Vec<_>>())
    {
        assert_eq!(case.len(), 7);
        let source = std::fs::read_to_string(root.join(case[1])).unwrap();
        let rows = rows
            .iter()
            .filter(|row| row[0] == case[0])
            .collect::<Vec<_>>();
        assert_eq!(rows.len(), 3);
        let expressions = rows.iter().map(|row| row[2].to_owned()).collect::<Vec<_>>();
        for _ in 0..4 {
            let mut probe = SourceProbe::new_expressions_text(
                &source,
                case[2],
                case[3],
                &expressions,
                "compact",
                false,
                false,
                true,
            )
            .unwrap();
            let mut capabilities = 0;
            for (index, row) in rows.iter().enumerate() {
                assert_eq!(row.len(), 10);
                assert_eq!(row[1].parse::<usize>().unwrap(), index);
                assert_eq!(row[8], "0");
                probe.start_path(index as u32, 0, None).unwrap();
                let mut settled = false;
                for _ in 0..100_000 {
                    let event = probe.poll(0, 1).unwrap();
                    probe.collect();
                    match event.kind {
                        "yield" => (),
                        "module-request" => {
                            assert_eq!(event.module.unwrap().kind, "capabilities");
                            capabilities += 1;
                            probe
                                .complete_module_capabilities(
                                    event.token.as_deref().unwrap(),
                                    case[4].parse().unwrap(),
                                    case[5].parse().unwrap(),
                                    case[6].parse().unwrap(),
                                    0,
                                )
                                .unwrap();
                        }
                        "value" | "error" => {
                            let (status, stdout, stderr) = if event.kind == "value" {
                                ("success", event.value.unwrap(), Vec::new())
                            } else {
                                ("error", Vec::new(), event.error.unwrap())
                            };
                            assert_eq!(status, row[3], "{}:{index}", case[0]);
                            assert_eq!(stdout, units(row[6]), "{}:{index} stdout", case[0]);
                            assert_eq!(stderr, units(row[7]), "{}:{index} stderr", case[0]);
                            assert_eq!(
                                PklString::from_utf16(stdout).to_java_utf8().as_bytes(),
                                hex(row[4])
                            );
                            assert_eq!(
                                PklString::from_utf16(stderr).to_java_utf8().as_bytes(),
                                hex(row[5])
                            );
                            assert_eq!(row[9], "0");
                            settled = true;
                            compared += 1;
                            break;
                        }
                        other => panic!("unexpected effect {other}; no reader call is permitted"),
                    }
                }
                assert!(settled, "{}:{index}", case[0]);
            }
            assert_eq!(
                capabilities,
                if case[0] == "coalesce-lambda-owner" {
                    0
                } else {
                    2
                }
            );
            probe.close();
            assert_eq!(probe.collect().slots, 0);
        }
    }
    assert_eq!(compared, 36);
}
