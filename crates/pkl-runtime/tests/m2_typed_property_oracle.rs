//! All51 Typed property lookup contracts from actual pinned Java0.32.0.
//! Complete raw UTF8/UTF16 status/result/error/Logger comparison; Java cause/process fields remain provenance.
#[path = "support/module_keys.rs"]
mod module_keys;
use pkl_runtime::{modules::ModuleRequestKind, probe::SourceProbe, value::PklString};
use std::{
    fs,
    path::{Path, PathBuf},
};
fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/m2-typed-property-oracle")
}
fn units(raw: &[u8]) -> Vec<u16> {
    let (pairs, tail) = raw.as_chunks::<2>();
    assert!(tail.is_empty());
    pairs.iter().map(|p| u16::from_be_bytes(*p)).collect()
}
fn field(path: &Path, name: &str, actual: &[u16]) -> bool {
    units(&fs::read(path.join(format!("{name}.code-units.bin"))).unwrap()) == actual
        && fs::read(path.join(format!("{name}.bin"))).unwrap()
            == PklString::from_utf16(actual.to_vec())
                .to_java_utf8()
                .as_bytes()
}
fn capture(path: Option<&Path>, name: &str, value: &[u16]) {
    if let Some(path) = path {
        fs::write(
            path.join(format!("{name}.bin")),
            PklString::from_utf16(value.to_vec())
                .to_java_utf8()
                .as_bytes(),
        )
        .unwrap();
        fs::write(
            path.join(format!("{name}.code-units.bin")),
            value
                .iter()
                .flat_map(|x| x.to_be_bytes())
                .collect::<Vec<_>>(),
        )
        .unwrap();
    }
}
fn run(program: &str, count: usize, power_assertions: bool) {
    let input = root().join(program);
    let modules = fs::read_to_string(root().join("modules.tsv")).unwrap();
    let source = fs::read_to_string(input.join("module.pkl")).unwrap();
    let uri = fs::read_to_string(input.join("uri.txt")).unwrap();
    let queries = fs::read_to_string(input.join("queries.index.tsv")).unwrap();
    assert_eq!(
        queries.lines().count(),
        count,
        "exact admitted query inventory"
    );
    let capture_root = std::env::var_os("PKL_FRAME_CAPTURE_DIR").map(PathBuf::from);
    let mut failures = Vec::new();
    for repeat in 0..2 {
        let mut probe =
            SourceProbe::new_module_evaluator("compact", true, false, power_assertions).unwrap();
        for line in queries.lines() {
            let columns = line.split('\t').collect::<Vec<_>>();
            assert_eq!(columns.len(), 3);
            assert_eq!(columns[0], "string");
            let id = columns[1];
            let expected = input.join(format!("repeat-{repeat}")).join(columns[2]);
            let expression = fs::read_to_string(expected.join("expression.bin")).unwrap();
            assert!(field(
                &expected,
                "expression",
                &expression.encode_utf16().collect::<Vec<_>>()
            ));
            let actual_dir = capture_root.as_ref().map(|r| {
                r.join(program)
                    .join(format!("repeat-{repeat}"))
                    .join(columns[2])
            });
            if let Some(p) = &actual_dir {
                fs::create_dir_all(p).unwrap();
            }
            probe
                .evaluate_module_source(&uri, Some(&source), &expression, 0, None)
                .unwrap();
            let mut terminal = None;
            let mut logs = 0usize;
            let mut row_logs_match = true;
            for _ in 0..200_000 {
                probe.collect();
                let event = probe.poll(0, 1).unwrap();
                probe.collect();
                if event.kind == "yield" {
                    continue;
                }
                if event.kind == "read" {
                    assert_eq!(
                        event.uri.as_deref(),
                        Some("env:STAGE105_TYPED_LOOKUP_UNSET")
                    );
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
                    let request = probe.module_request(token).unwrap();
                    match request.kind {
                        ModuleRequestKind::Resolve { original_uri, .. } => {
                            assert!(
                                modules
                                    .lines()
                                    .any(|line| line.split('\t').next()
                                        == Some(original_uri.as_ref()))
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
                            assert!(
                                modules
                                    .lines()
                                    .any(|line| line.split('\t').next() == Some(key.uri.as_ref()))
                            );
                            probe
                                .complete_module_resolution(token, &key.uri, &key.uri, &key.uri, 0)
                                .unwrap();
                        }
                        ModuleRequestKind::LoadSource { resolved, .. } => {
                            let entry = modules
                                .lines()
                                .map(|line| line.split('\t').collect::<Vec<_>>())
                                .find(|row| row[0] == resolved.original_uri.as_ref())
                                .expect("pinned Java auxiliary module");
                            let text = fs::read_to_string(root().join(entry[1])).unwrap();
                            probe
                                .complete_module_source(token, &text, entry[2], 0)
                                .unwrap();
                        }
                        other => panic!("unexpected module boundary {other:?}"),
                    }
                    continue;
                }
                if event.kind == "log" {
                    let log = event.log.as_ref().unwrap();
                    let position = format!(
                        "{}:{}-{}:{}",
                        log.start_line, log.start_column, log.end_line, log.end_column
                    );
                    let fields = [
                        log.level.encode_utf16().collect::<Vec<_>>(),
                        log.message.clone(),
                        log.module_uri.clone(),
                        log.member_name
                            .clone()
                            .unwrap_or_else(|| "null".encode_utf16().collect()),
                        position.encode_utf16().collect(),
                    ];
                    for (name, actual) in ["level", "message", "module-uri", "member", "position"]
                        .iter()
                        .zip(&fields)
                    {
                        let name = format!("log-{logs:03}.{name}");
                        capture(actual_dir.as_deref(), &name, actual);
                        if !expected.join(format!("{name}.bin")).exists()
                            || !field(&expected, &name, actual)
                        {
                            row_logs_match = false;
                            failures.push(format!("{program}/{id}: Logger {name}"));
                        }
                    }
                    logs += 1;
                    probe
                        .acknowledge_log(event.token.as_deref().unwrap(), None, 0)
                        .unwrap();
                    continue;
                }
                assert!(
                    matches!(event.kind, "value" | "error"),
                    "unexpected pending event: {event:?}"
                );
                terminal = Some(event);
                break;
            }
            let event = terminal.expect("bounded Typed.hasProperty query");
            let actual_status = match event.kind {
                "value" => "success\n",
                "error" if event.disposition == Some("language-error") => "error\n",
                _ => "unsupported-or-api-error\n",
            };
            let stdout = event.value.as_deref().unwrap_or(&[]);
            let stderr = event.error.as_deref().unwrap_or(&[]);
            capture(actual_dir.as_deref(), "result.stdout", stdout);
            capture(actual_dir.as_deref(), "result.stderr", stderr);
            if let Some(p) = &actual_dir {
                fs::write(p.join("result.status.bin"), actual_status).unwrap();
                fs::write(p.join("logs.count.bin"), format!("{logs}\n")).unwrap();
            }
            let expected_logs = fs::read(expected.join("logs.count.bin")).unwrap();
            let equal = fs::read(expected.join("result.status.bin")).unwrap()
                == actual_status.as_bytes()
                && field(&expected, "result.stdout", stdout)
                && field(&expected, "result.stderr", stderr)
                && expected_logs == format!("{logs}\n").as_bytes()
                && row_logs_match;
            let required_deferred = program == "map-mapping-inventory"
                && matches!(id, "q0026" | "q0027" | "q0029" | "q0030" | "q0031");
            eprintln!(
                "typed-property-row program={program} repeat={repeat} id={id} required_deferred={required_deferred} exact_match={equal}"
            );
            if !equal {
                failures.push(format!(
                    "{program}/{id}/repeat-{repeat}: {event:?}; logs={logs}, expected-count-bytes={expected_logs:?}"
                ));
            }
        }
        probe.close();
        assert_eq!(probe.collect().slots, 0);
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn java_original_members() {
    run("original-members", 8, false);
}

#[test]
fn java_original_lazy() {
    run("original-lazy", 5, false);
}

#[test]
fn java_original_conversions() {
    run("original-conversions", 4, false);
}

#[test]
fn java_original_arguments() {
    run("original-arguments", 4, false);
}

#[test]
fn java_original_const_guard() {
    run("original-const-guard", 1, false);
}

#[test]
fn java_original_module_reference() {
    run("original-module-reference", 1, false);
}

#[test]
fn java_original_get_property_members() {
    run("original-get-property-members", 8, false);
}

#[test]
fn java_original_get_property_lazy() {
    run("original-get-property-lazy", 5, false);
}

#[test]
fn java_original_get_property_conversions() {
    run("original-get-property-conversions", 4, false);
}

#[test]
fn java_original_get_property_arguments() {
    run("original-get-property-arguments", 4, false);
}

#[test]
fn java_original_get_property_const_guard() {
    run("original-get-property-const-guard", 1, false);
}

#[test]
fn java_original_get_property_failed_class() {
    run("original-get-property-failed-class", 4, false);
}

#[test]
fn java_original_get_property_failed_superclass() {
    run("original-get-property-failed-superclass", 2, false);
}
