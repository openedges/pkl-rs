//! Set API queries from pinned Pkl 0.32.0 Java.
//! Compare full status, UTF8/UTF16 results and ordered Logger fields; Java
//! exception/class/cause metadata is retained in fixtures, not compared here.
#[path = "support/module_keys.rs"]
mod module_keys;
use pkl_runtime::{logging::PlatformMetadata, probe::SourceProbe, value::PklString};
use std::{
    fs,
    path::{Path, PathBuf},
};
fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/m2-set-apis-oracle")
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
    let queries = fs::read_to_string(input.join("comparable.tsv")).unwrap();
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
            assert_eq!(columns.len(), 4);
            assert_eq!(columns[0], "string");
            let id = columns[1];
            let expression = columns[2];
            let expected = input.join(format!("repeat-{repeat}")).join(columns[3]);
            assert_eq!(
                fs::read(expected.join("expression.bin")).unwrap(),
                expression.as_bytes()
            );
            let actual_dir = capture_root.as_ref().map(|r| {
                r.join(program)
                    .join(format!("repeat-{repeat}"))
                    .join(columns[3])
            });
            if let Some(p) = &actual_dir {
                fs::create_dir_all(p).unwrap();
            }
            probe
                .evaluate_module_source(&uri, Some(&source), expression, 0, None)
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
            let event = terminal.expect("bounded Set API query");
            let actual_status = match event.kind {
                "value" => "success\n",
                "error" if event.disposition == Some("oracle-error") => "oracle-error\n",
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
            eprintln!("set-api-row program={program} repeat={repeat} id={id} exact_match={equal}");
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
fn java_set_discovery() {
    run("set-discovery", 52, false);
}

#[test]
fn java_set_predicates() {
    run("set-predicates", 115, false);
}

#[test]
fn java_set_slicing() {
    run("set-slicing", 166, false);
}

#[test]
fn java_set_flatmap() {
    run("set-flatmap", 25, false);
}

#[test]
fn java_set_fold_group() {
    run("set-fold-group", 28, false);
}

#[test]
fn java_set_extrema() {
    run("set-extrema", 104, false);
}

#[test]
fn java_set_sorting() {
    run("set-sorting", 44, false);
}

#[test]
fn java_set_algebra() {
    run("set-algebra", 18, false);
}

#[test]
fn java_set_identity() {
    run("set-identity", 17, false);
}

#[test]
fn java_set_contracts() {
    run("set-contracts", 52, false);
}

#[test]
fn java_set_add_hash_order() {
    run("set-add-hash-order", 12, false);
}

#[test]
fn java_set_recursive_slice_table() {
    run("set-recursive-slice-table", 17, false);
}

#[test]
fn java_set_add_recursive_inner() {
    run("set-add-recursive-inner", 3, false);
}

#[test]
fn java_set_flatmap_listing_order() {
    run("set-flatmap-listing-order", 4, false);
}

#[test]
fn java_set_flatten_map_entry() {
    run("set-flatten-map-entry", 7, false);
}

#[test]
fn java_set_flatten_entry_children() {
    run("set-flatten-entry-children", 9, false);
}

// Required unresolved upstream protocol; an ignored test never satisfies full M2.
#[test]
fn java_foreign_entry_internal_protocol_remains_required() {
    let cases = [
        (
            "set-flatmap",
            "021-q0021",
            "(Set(Map(1,2)).flatten()).toString()",
        ),
        (
            "set-flatten-map-entry",
            "007-q0007",
            "(Set(Map(1, 2)).flatten().first.toString()).toString()",
        ),
        (
            "set-flatten-entry-children",
            "006-q0006",
            "(let (s = Set(Map(List(1), List(2))).flatten()) s.toList().first == s.first).toString()",
        ),
    ];
    for (program, id, expression) in cases {
        let input = root().join(program);
        let source = fs::read_to_string(input.join("module.pkl")).unwrap();
        let uri = fs::read_to_string(input.join("uri.txt")).unwrap();
        let expected = input.join("repeat-0").join(id);
        let mut probe = SourceProbe::new_module_evaluator("compact", true, false, false).unwrap();
        probe.set_platform_metadata(Some(PlatformMetadata {
            os_name: "macOS".into(),
            os_version: "27.0".into(),
            cpu_architecture: "aarch64".into(),
            java_version: "21.0.12.1".into(),
        }));
        probe
            .evaluate_module_source(&uri, Some(&source), expression, 0, None)
            .unwrap();
        let mut terminal = None;
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
            terminal = Some(event);
            break;
        }
        let event = terminal.expect("terminal event");
        let actual_status = match event.kind {
            "value" => "success\n",
            "error" if event.disposition == Some("oracle-error") => "oracle-error\n",
            "error" if event.disposition == Some("language-error") => "error\n",
            _ => "unsupported-or-api-error\n",
        };
        let stdout = event.value.as_deref().unwrap_or(&[]);
        let stderr = event.error.as_deref().unwrap_or(&[]);
        let expected_status = fs::read(expected.join("result.status.bin")).unwrap();
        assert_eq!(
            expected_status,
            actual_status.as_bytes(),
            "{id} status mismatch"
        );
        assert!(
            field(&expected, "result.stdout", stdout),
            "{id} stdout mismatch"
        );
        let expected_stderr = fs::read(expected.join("result.stderr.bin")).unwrap();
        let actual_stderr_bytes = pkl_runtime::value::PklString::from_utf16(stderr.to_vec())
            .to_java_utf8()
            .into_bytes();
        if expected_stderr != actual_stderr_bytes {
            eprintln!(
                "EXPECTED STDERR FOR {id}:\n{}",
                String::from_utf8_lossy(&expected_stderr)
            );
            eprintln!(
                "ACTUAL STDERR FOR {id}:\n{}",
                String::from_utf8_lossy(&actual_stderr_bytes)
            );
        }
        assert_eq!(expected_stderr, actual_stderr_bytes, "{id} stderr mismatch");
        probe.close();
        assert_eq!(probe.collect().slots, 0);
    }
}
