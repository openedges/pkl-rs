//! Full Java expression-string channels for authored standard-module controls.
//! Uses the actual native development module Host plus shared glob semantics.
#[path = "../examples/support/module_host.rs"]
mod module_host;
use pkl_runtime::{glob, probe::SourceProbe, value::PklString};
use std::{
    fs,
    path::{Path, PathBuf},
};

const CASES: [&str; 12] = [
    "opaque-file-expression",
    "opaque-file-glob-expression",
    "opaque-file-clause-unused",
    "uppercase-opaque-file",
    "unused-denied-clause",
    "used-denied-clause",
    "https-glob-clause",
    "http-glob-expression",
    "generic-url-glob",
    "unknown-opaque-expression",
    "missing-backslash",
    "missing-double-escape",
];
fn bytes(folder: &Path, name: &str) -> Vec<u8> {
    fs::read(folder.join(name))
        .unwrap_or_else(|error| panic!("{}: {error}", folder.join(name).display()))
}
fn text(folder: &Path, name: &str) -> String {
    String::from_utf8(bytes(folder, name)).unwrap()
}
fn same(folder: &Path, stem: &str, actual: &[u16]) -> bool {
    bytes(folder, &format!("{stem}.code-units.bin"))
        == actual
            .iter()
            .flat_map(|unit| unit.to_be_bytes())
            .collect::<Vec<_>>()
        && bytes(folder, &format!("{stem}.bin"))
            == PklString::from_utf16(actual.to_vec())
                .to_java_utf8()
                .as_bytes()
}

#[test]
fn java_standard_module_diagnostics_match_complete_raw_channels() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/m2-standard-module-diagnostics");
    assert_eq!(text(&root, "cases.txt").lines().collect::<Vec<_>>(), CASES);
    let mut failures = Vec::new();
    for case in CASES {
        let input = root.join(case);
        let source = text(&input, "module.pkl");
        let uri = text(&input, "uri.txt");
        let expression = text(&input, "expression.txt");
        assert_eq!(
            uri,
            format!("file:///stage137-module-diagnostics/{case}/main.pkl")
        );
        assert_eq!(text(&input, "module-name.txt"), "main");
        assert_eq!(expression, "output.text");
        for (mode, power) in [("on", true), ("off", false)] {
            let expected = input.join(mode);
            let status = bytes(&expected, "result.status.bin");
            assert!(
                status == b"success\n" || status == b"error\n",
                "{case}/{mode}: ordinary Java outcome required; preserve and inspect unexpected authority first"
            );
            let class = if status == b"success\n" {
                ""
            } else {
                "org.pkl.core.PklException"
            };
            assert!(
                same(
                    &expected,
                    "result.error-class",
                    &class.encode_utf16().collect::<Vec<_>>()
                ),
                "{case}/{mode}: Java class admission"
            );
            assert!(same(
                &expected,
                "expression",
                &expression.encode_utf16().collect::<Vec<_>>()
            ));
            let mut probe =
                SourceProbe::new_module_evaluator("compact", false, false, power).unwrap();
            probe
                .evaluate_module_source(&uri, Some(&source), &expression, 0, None)
                .unwrap();
            let mut terminal = None;
            for _ in 0..200_000 {
                let event = probe.poll_text(0, 1).unwrap();
                probe.collect();
                match event.kind {
                    "yield" => {}
                    "read" => {
                        assert_eq!(
                            event.uri.as_ref().and_then(PklString::as_str),
                            Some("prop:pkl.outputFormat")
                        );
                        probe
                            .complete_read_missing(event.token.as_deref().unwrap(), 0)
                            .unwrap();
                    }
                    "module-request" => {
                        let token = event.token.as_deref().unwrap();
                        let request = probe.module_request(token).unwrap();
                        probe
                            .complete_module(token, module_host::complete(&request), 0)
                            .unwrap();
                    }
                    "glob-request" => {
                        let token = event.token.as_deref().unwrap();
                        let request = probe.glob_request(token).unwrap();
                        let completion = glob::configured_completion(&request, &[], &[])
                            .expect("control unexpectedly requires glob I/O");
                        probe.complete_glob(token, completion, 0).unwrap();
                    }
                    "value" | "error" => {
                        terminal = Some(event);
                        break;
                    }
                    _ => panic!("unexpected Host effect: {event:?}"),
                }
            }
            let event = terminal.expect("bounded module control did not settle");
            let actual_status = match event.kind {
                "value" => b"success\n".as_slice(),
                "error" if event.disposition == Some("language-error") => b"error\n".as_slice(),
                _ => b"unsupported-or-api-error\n".as_slice(),
            };
            let equal = status == actual_status
                && same(
                    &expected,
                    "result.stdout",
                    event.value.as_deref().unwrap_or(&[]),
                )
                && same(
                    &expected,
                    "result.stderr",
                    event.error.as_deref().unwrap_or(&[]),
                );
            if !equal {
                failures.push(format!("{case}/{mode}: {event:?}"));
            }
            probe.close();
            assert_eq!(probe.collect().slots, 0);
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
