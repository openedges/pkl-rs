//! All150 Typed.hasProperty contracts from actual pinned Pkl0.32.0 Java.
//! Preserve original syntax errors and separate corrected class/const errors.
//! Compare complete status, UTF8/UTF16 results/errors and ordered Logger fields;
//! raw Java exception/class/cause/process metadata remains provenance.
#[path = "support/module_keys.rs"]
mod module_keys;
use pkl_runtime::{probe::SourceProbe, value::PklString};
use std::{
    fs,
    path::{Path, PathBuf},
};
fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/m2-typed-hasproperty-oracle")
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
                if event.kind == "module-request" {
                    let token = event.token.as_deref().unwrap();
                    assert!(module_keys::complete_probe(&mut probe, token, 0).unwrap());
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
                "typed-hasproperty-row program={program} repeat={repeat} id={id} required_deferred={required_deferred} exact_match={equal}"
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
fn java_original_typed_members() {
    run("original-typed-members", 17, false);
}

#[test]
fn java_original_conversions() {
    run("original-conversions", 19, false);
}

#[test]
fn java_original_module_members() {
    run("original-module-members", 14, false);
}

#[test]
fn java_original_unicode_names() {
    run("original-unicode-names", 13, false);
}

#[test]
fn java_original_call_diagnostics() {
    run("original-call-diagnostics", 12, false);
}

#[test]
fn java_original_no_force() {
    run("original-no-force", 20, false);
}

#[test]
fn java_original_recursive_types() {
    run("original-recursive-types", 5, false);
}

#[test]
fn java_original_forward_publication() {
    run("original-forward-publication", 5, false);
}

#[test]
fn java_original_recursive_initializer() {
    run("original-recursive-initializer", 4, false);
}

#[test]
fn java_original_failed_property_publication() {
    run("original-failed-property-publication", 8, false);
}

#[test]
fn java_original_failed_method_publication() {
    run("original-failed-method-publication", 8, false);
}

#[test]
fn java_original_failed_superclass_publication() {
    run("original-failed-superclass-publication", 4, false);
}

#[test]
fn java_original_incomplete_superclass() {
    run("original-incomplete-superclass", 3, false);
}

#[test]
fn java_original_module_method_init() {
    run("original-module-method-init", 3, false);
}

#[test]
fn java_original_module_version_init() {
    run("original-module-version-init", 3, false);
}

#[test]
fn java_original_class_method_init() {
    run("original-class-method-init", 3, false);
}

#[test]
fn java_trace_corrected_module_method_init() {
    run("trace-corrected-module-method-init", 3, false);
}

#[test]
fn java_trace_corrected_module_version_init() {
    run("trace-corrected-module-version-init", 3, false);
}

#[test]
fn java_trace_corrected_class_method_init() {
    run("trace-corrected-class-method-init", 3, false);
}
