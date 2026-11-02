//! Java 0.32 fixed-URI cross-module type diagnostic comparisons.
#[path = "support/module_keys.rs"]
mod module_keys;
use pkl_runtime::{modules::ModuleRequestKind, probe::SourceProbe, value::PklString};
use std::{
    fs,
    path::{Path, PathBuf},
};

const CASES: [&str; 8] = [
    "import-nullable",
    "import-union",
    "import-constrained",
    "import-generic",
    "forward-nullable",
    "unicode-columns",
    "same-span-distinct-source",
    "unicode-inline-columns",
];

fn read(dir: &Path, name: &str) -> Vec<u8> {
    fs::read(dir.join(name)).unwrap_or_else(|e| panic!("{}: {e}", dir.join(name).display()))
}
fn text(dir: &Path, name: &str) -> String {
    String::from_utf8(read(dir, name)).unwrap()
}
fn same(dir: &Path, stem: &str, units: &[u16]) -> bool {
    let expected_units: Vec<u8> = units.iter().flat_map(|unit| unit.to_be_bytes()).collect();
    read(dir, &format!("{stem}.code-units.bin")) == expected_units
        && read(dir, &format!("{stem}.bin"))
            == PklString::from_utf16(units.to_vec())
                .to_java_utf8()
                .as_bytes()
}

#[test]
fn java_cross_source_generic_diagnostics_match_all_channels() {
    let root =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/m2-cross-source");
    assert_eq!(text(&root, "cases.txt").lines().collect::<Vec<_>>(), CASES);
    let mut failures = Vec::new();
    for case in CASES {
        let input = root.join(case);
        let source = text(&input, "main.pkl");
        let uri = text(&input, "uri.txt");
        let expression = text(&input, "expression.txt");
        assert_eq!(uri, format!("file:///stage132-cross/{case}/main.pkl"));
        assert_eq!(expression, "output.text");
        let base = uri.strip_suffix("main.pkl").unwrap();
        for (mode, power) in [("on", true), ("off", false)] {
            let expected = input.join(mode);
            assert_eq!(
                read(&expected, "result.status.bin"),
                b"error\n",
                "{case}/{mode} must be an ordinary Java error"
            );
            assert_eq!(
                read(&expected, "result.error-class.bin"),
                b"org.pkl.core.PklException",
                "{case}/{mode} Java error class"
            );
            let java_error = read(&expected, "result.stderr.bin");
            assert!(
                java_error.starts_with("–– Pkl Error ––\nExpected value of type ".as_bytes()),
                "{case}/{mode}: Java did not reach a type mismatch"
            );
            let mut probe =
                SourceProbe::new_module_evaluator("compact", true, false, power).unwrap();
            probe
                .evaluate_module_source(&uri, Some(&source), &expression, 0, None)
                .unwrap();
            let mut result = None;
            for _ in 0..200_000 {
                probe.collect();
                let event = probe.poll(0, 1).unwrap();
                probe.collect();
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
                if event.kind == "module-request" {
                    let token = event.token.as_deref().unwrap();
                    if module_keys::complete_probe(&mut probe, token, 0).unwrap() {
                        continue;
                    }
                    match probe.module_request(token).unwrap().kind {
                        ModuleRequestKind::Resolve { original_uri, .. } => {
                            let u = original_uri.as_str().expect("fixture URI");
                            assert!(
                                u == format!("{base}defs.pkl")
                                    || (case == "forward-nullable"
                                        && u == format!("{base}forward.pkl"))
                            );
                            probe
                                .complete_module_resolution(
                                    token,
                                    &original_uri,
                                    &original_uri,
                                    &original_uri,
                                    0,
                                )
                                .unwrap();
                        }
                        ModuleRequestKind::ResolveKey { key, .. } => {
                            let u = key.uri.as_str().expect("fixture URI");
                            assert!(
                                u == format!("{base}defs.pkl")
                                    || (case == "forward-nullable"
                                        && u == format!("{base}forward.pkl"))
                            );
                            probe
                                .complete_module_resolution(token, &key.uri, &key.uri, &key.uri, 0)
                                .unwrap();
                        }
                        ModuleRequestKind::LoadSource { resolved, .. } => {
                            let u = resolved.original_uri.as_str().expect("fixture URI");
                            let filename = if u == format!("{base}defs.pkl") {
                                "defs.pkl"
                            } else if case == "forward-nullable"
                                && u == format!("{base}forward.pkl")
                            {
                                "forward.pkl"
                            } else {
                                panic!("unexpected module URI {u}")
                            };
                            let contents = text(&input, filename);
                            let inferred = filename.strip_suffix(".pkl").unwrap();
                            probe
                                .complete_module_source(token, contents, inferred, 0)
                                .unwrap();
                        }
                        other => panic!("unexpected module request: {other:?}"),
                    }
                    continue;
                }
                result = Some(event);
                break;
            }
            let event = result.expect("bounded cross-source evaluation");
            let status = match event.kind {
                "value" => b"success\n".as_slice(),
                "error" if event.disposition == Some("language-error") => b"error\n".as_slice(),
                _ => b"unsupported-or-api-error\n".as_slice(),
            };
            let stdout = event.value.as_deref().unwrap_or(&[]);
            let stderr = event.error.as_deref().unwrap_or(&[]);
            if read(&expected, "result.status.bin") != status
                || !same(&expected, "result.stdout", stdout)
                || !same(&expected, "result.stderr", stderr)
            {
                failures.push(format!("{case}/{mode}: {event:?}"));
            }
            probe.close();
            assert_eq!(probe.collect().slots, 0);
        }
    }
    assert!(
        failures.is_empty(),
        "cross-source mismatches:\n{}",
        failures.join("\n")
    );
}
