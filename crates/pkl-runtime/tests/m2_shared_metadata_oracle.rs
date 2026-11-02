//! Complete Java generic Reference metadata observations.
#[path = "support/module_keys.rs"]
mod module_keys;
use pkl_runtime::{probe::SourceProbe, value::PklString};
use std::{fs, path::Path};

#[test]
fn generic_reference_metadata_matches_java() {
    let root =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/m2-shared-metadata");
    let cases = fs::read_to_string(root.join("cases.txt")).unwrap();
    assert_eq!(cases.lines().count(), 4);
    for case in cases.lines() {
        let input = root.join(case);
        let source = fs::read_to_string(input.join("module.pkl")).unwrap();
        let uri = fs::read_to_string(input.join("uri.txt")).unwrap();
        assert_eq!(uri, format!("file:///stage132-shared/{case}.pkl"));
        assert_eq!(
            fs::read_to_string(input.join("expression.txt")).unwrap(),
            "output.text"
        );
        for (mode, power) in [("on", true), ("off", false)] {
            let expected = input.join(mode);
            let mut probe =
                SourceProbe::new_module_evaluator("compact", true, false, power).unwrap();
            probe
                .evaluate_module_source(&uri, Some(&source), "output.text", 0, None)
                .unwrap();
            let mut terminal = None;
            for _ in 0..200_000 {
                let event = probe.poll(0, 16).unwrap();
                if event.kind == "yield" {
                    continue;
                }
                if event.kind == "read" {
                    assert_eq!(event.uri.as_deref(), Some("prop:pkl.outputFormat"));
                    probe
                        .complete_read_missing(event.token.as_deref().unwrap(), 0)
                        .unwrap();
                    continue;
                }
                if event.kind == "module-request"
                    && module_keys::complete_probe(&mut probe, event.token.as_deref().unwrap(), 0)
                        .unwrap()
                {
                    continue;
                }
                terminal = Some(event);
                break;
            }
            let event = terminal.expect("bounded generic Reference evaluation");
            let (status, stdout, stderr) = match event.kind {
                "value" => (b"success\n".as_slice(), event.value.unwrap(), Vec::new()),
                "error" => (b"error\n".as_slice(), Vec::new(), event.error.unwrap()),
                other => panic!("unexpected completion {other}"),
            };
            assert_eq!(
                fs::read(expected.join("result.status.bin")).unwrap(),
                status,
                "{case}/{mode}"
            );
            for (stem, actual) in [("result.stdout", stdout), ("result.stderr", stderr)] {
                assert_eq!(
                    fs::read(expected.join(format!("{stem}.code-units.bin"))).unwrap(),
                    actual
                        .iter()
                        .flat_map(|v| v.to_be_bytes())
                        .collect::<Vec<_>>(),
                    "{case}/{mode}/{stem}"
                );
                assert_eq!(
                    fs::read(expected.join(format!("{stem}.bin"))).unwrap(),
                    PklString::from_utf16(actual).to_java_utf8().as_bytes(),
                    "{case}/{mode}/{stem}"
                );
            }
            probe.close();
            assert_eq!(probe.collect().slots, 0);
        }
    }
}
