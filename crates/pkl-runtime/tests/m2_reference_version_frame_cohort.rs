//! Ordered public Java frame authority; Host completion supplies the pinned file modules.
#[path = "support/module_keys.rs"]
mod module_keys;
use pkl_runtime::{modules::ModuleRequestKind, probe::SourceProbe, value::PklString};
use std::{
    fs,
    path::{Path, PathBuf},
};
fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/m2-reference-version-frame-cohort")
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
fn run(program: &str) {
    let input = root().join(program);
    let source = fs::read_to_string(input.join("module.pkl")).unwrap();
    let uri = fs::read_to_string(input.join("uri.txt")).unwrap();
    let queries = fs::read_to_string(input.join("queries.tsv")).unwrap();
    let modules = fs::read_to_string(root().join("modules.tsv")).unwrap();
    let capture_root = std::env::var_os("PKL_FRAME_CAPTURE_DIR").map(PathBuf::from);
    let mut failures = Vec::new();
    for repeat in 0..2 {
        let mut probe = SourceProbe::new_module_evaluator("compact", true, false, false).unwrap();
        for (index, line) in queries.lines().enumerate() {
            let columns = line.split('\t').collect::<Vec<_>>();
            assert_eq!(columns.len(), 3);
            assert_eq!(columns[0], "string");
            let id = columns[1];
            let expression = columns[2];
            let expected = input.join(format!("{index:03}-{id}"));
            assert_eq!(
                fs::read(expected.join("expression.bin")).unwrap(),
                expression.as_bytes()
            );
            let actual_dir = capture_root.as_ref().map(|r| {
                r.join(program)
                    .join(format!("repeat-{repeat}"))
                    .join(format!("{index:03}-{id}"))
            });
            if let Some(p) = &actual_dir {
                fs::create_dir_all(p).unwrap();
            }
            probe
                .evaluate_module_source(&uri, Some(&source), expression, 0, None)
                .unwrap();
            let mut terminal = None;
            let mut logs = 0usize;
            for _ in 0..200_000 {
                probe.collect();
                let event = probe.poll(0, 1).unwrap();
                probe.collect();
                if event.kind == "yield" {
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
                            assert!(modules.lines().any(|line| {
                                line.split('\t')
                                    .next()
                                    .is_some_and(|value| original_uri == value)
                            }));
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
                        ModuleRequestKind::LoadSource { resolved, .. } => {
                            let entry = modules
                                .lines()
                                .map(|line| line.split('\t').collect::<Vec<_>>())
                                .find(|row| resolved.original_uri == row[0])
                                .expect("pinned sibling module");
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
                            failures.push(format!("{program}/{id}: Logger {name}"));
                        }
                    }
                    logs += 1;
                    probe
                        .acknowledge_log(event.token.as_deref().unwrap(), None, 0)
                        .unwrap();
                    continue;
                }
                terminal = Some(event);
                break;
            }
            let event = terminal.expect("bounded frame diagnostic query");
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
            let expected_logs: usize = fs::read_to_string(expected.join("logs.count.bin"))
                .unwrap()
                .trim_end_matches('\n')
                .parse()
                .unwrap();
            let equal = fs::read(expected.join("result.status.bin")).unwrap()
                == actual_status.as_bytes()
                && field(&expected, "result.stdout", stdout)
                && field(&expected, "result.stderr", stderr)
                && logs == expected_logs;
            eprintln!("frame-row program={program} repeat={repeat} id={id} exact_match={equal}");
            if !equal {
                failures.push(format!(
                    "{program}/{id}/repeat-{repeat}: {event:?}; logs={logs}/{expected_logs}"
                ));
            }
        }
        probe.close();
        assert_eq!(probe.collect().slots, 0);
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
#[test]
fn java_reference_members_and_retries() {
    run("reference-members");
}
#[test]
fn java_reference_declaration_contrast() {
    run("reference-declaration");
}
#[test]
fn java_version_nearest_caller_and_cached_sites() {
    run("version-sites");
}
