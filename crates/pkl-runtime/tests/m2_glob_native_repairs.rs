//! Complete retained Java channels for native defects, with and without GC.
#[path = "../examples/support/glob_fixture.rs"]
mod glob_fixture;
#[path = "../examples/support/glob_fixture_host.rs"]
mod glob_fixture_host;
use glob_fixture::{Fixture, Provider};
use glob_fixture_host::FixtureHost;
use pkl_runtime::probe::{SourceLog, SourceProbe};
use pkl_runtime::value::PklString;
use std::collections::BTreeMap;
use std::path::Path;

fn text(actual: &mut BTreeMap<String, Vec<u8>>, name: &str, value: &PklString) {
    actual.insert(format!("{name}.bin"), value.to_java_utf8().into_bytes());
    actual.insert(
        format!("{name}.code-units.bin"),
        value.encode_utf16().flat_map(u16::to_be_bytes).collect(),
    );
}
fn log(actual: &mut BTreeMap<String, Vec<u8>>, index: usize, value: &SourceLog) {
    let prefix = format!("log-{index:03}");
    text(actual, &format!("{prefix}.level"), &value.level.into());
    text(
        actual,
        &format!("{prefix}.message"),
        &PklString::from_utf16(value.message.clone()),
    );
    text(
        actual,
        &format!("{prefix}.module-uri"),
        &PklString::from_utf16(value.module_uri.clone()),
    );
    text(
        actual,
        &format!("{prefix}.member"),
        &value
            .member_name
            .clone()
            .map(PklString::from_utf16)
            .unwrap_or_else(|| "null".into()),
    );
    text(
        actual,
        &format!("{prefix}.position"),
        &format!(
            "{}:{}-{}:{}",
            value.start_line, value.start_column, value.end_line, value.end_column
        )
        .into(),
    );
    for (line, value) in value.source_lines.iter().enumerate() {
        text(
            actual,
            &format!("{prefix}.source-{line}"),
            &PklString::from_utf16(value.clone()),
        );
    }
}
fn check_case(id: &str, collect_between_queries: bool) {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/m2-glob-native-repairs")
        .join(id);
    let fixture = Fixture::read(&directory.join("input")).unwrap();
    let queries = fixture.queries.clone();
    let uri = fixture.config["sourceUri"].clone();
    let contents = if fixture
        .config
        .get("primaryMode")
        .is_some_and(|v| v == "provider")
    {
        None
    } else {
        Some(fixture.source.clone())
    };
    let mut host = FixtureHost::new(Provider::new(fixture));
    let mut session = SourceProbe::new_module_evaluator("compact", true, false, false).unwrap();
    for (index, query) in queries.iter().enumerate() {
        let expected = directory.join(format!("{index:03}-{}", query.id));
        host.provider.phase = format!("query:{}", query.id);
        session
            .evaluate_module_source_text(
                uri.clone(),
                contents.clone(),
                query.expression.clone(),
                0,
                None,
            )
            .unwrap();
        let mut actual = BTreeMap::new();
        text(&mut actual, "expression", query.expression.original());
        let mut logs = 0;
        let mut terminal = false;
        for _ in 0..1_000_000 {
            let event = session.poll_text(0, 4096).unwrap();
            let token = event.token.as_deref();
            match event.kind {
                "yield" => {}
                "module-request" => {
                    let token = token.unwrap();
                    let request = session.module_request(token).unwrap();
                    let completion = host.module(&request).unwrap();
                    session.complete_module(token, completion, 0).unwrap();
                }
                "glob-request" => {
                    let token = token.unwrap();
                    let request = session.glob_request(token).unwrap();
                    let completion = host.glob(&request).unwrap();
                    session.complete_glob(token, completion, 0).unwrap();
                }
                "read" => {
                    let token = token.unwrap();
                    let uri = session.resolved_read_uri_text(token).unwrap();
                    let completion = host.read(&uri).unwrap();
                    session.complete_read(token, completion, 0).unwrap();
                }
                "log" => {
                    log(&mut actual, logs, event.log.as_ref().unwrap());
                    logs += 1;
                    session.acknowledge_log(token.unwrap(), None, 0).unwrap();
                }
                "value" | "error" => {
                    let (status, value, error) = if event.kind == "value" {
                        (
                            "success\n",
                            PklString::from_utf16(event.value.unwrap()),
                            PklString::default(),
                        )
                    } else {
                        (
                            if event.disposition == Some("unsupported") {
                                "unsupported\n"
                            } else {
                                "error\n"
                            },
                            PklString::default(),
                            PklString::from_utf16(event.error.unwrap()),
                        )
                    };
                    actual.insert("result.status.bin".into(), status.as_bytes().to_vec());
                    text(&mut actual, "result.stdout", &value);
                    text(&mut actual, "result.stderr", &error);
                    terminal = true;
                    break;
                }
                other => panic!("unexpected {id} effect {other}"),
            }
        }
        assert!(terminal, "bounded query must finish: {id}");
        actual.insert("logs.count.bin".into(), format!("{logs}\n").into_bytes());
        for (name, bytes) in actual {
            assert_eq!(
                bytes,
                std::fs::read(expected.join(&name)).unwrap(),
                "complete Java channel: {id}/{}/{name}, GC={collect_between_queries}",
                query.id
            );
        }
        if collect_between_queries {
            session.collect();
        }
    }
    session.close();
    println!("complete retained Java case {id}, GC={collect_between_queries}");
}
#[test]
fn retained_java_uri_security_generic_url_and_anonymous_glob_frames() {
    for id in [
        "pattern-13-import",
        "uri-hyphen-scheme-import",
        "capability-missing-reader-import",
        "failure-resolve-uri-security-read",
        "failure-resolve-uri-security-import",
        "cache-failed-member-retries-read",
        "cache-missing-value-cache-read",
        "cache-missing-value-cache-import",
        "cache-partial-success-retained-read-witness-v2",
    ] {
        check_case(id, false);
        check_case(id, true);
    }
}

#[test]
fn anonymous_diagnostic_root_keeps_source_identity_with_and_without_detached_source() {
    use pkl_runtime::evaluation::runtime_error_text;
    use pkl_runtime::source::{SourceMap, SourceText, Span};
    use pkl_runtime::value::{ErrorKind, RuntimeError, RuntimeFrame, RuntimeSource};
    use std::sync::Arc;
    let body = include_str!(
        "fixtures/m2-glob-native-repairs/cache-missing-value-cache-read/input/module.pkl"
    );
    let expected = include_bytes!(
        "fixtures/m2-glob-native-repairs/cache-missing-value-cache-read/001-q01/result.stderr.code-units.bin"
    );
    let query = "entries[\"gdata:/a/../b\"].toString()";
    let uri = PklString::from("gmain:/tree/cache-missing-value-cache-read.pkl");
    let name = PklString::from_utf16(vec![0x6d, 0xd800, 0x6e]);
    let mut sources = SourceMap::default();
    let id = sources.add(uri.clone(), body);
    let source = sources.get(id).unwrap();
    let retained = Arc::new(RuntimeSource {
        text: SourceText::from(body),
        uri: uri.clone(),
        module_name: name.clone(),
    });
    for detached in [false, true] {
        let mut error = RuntimeError::new(ErrorKind::Host, "Cannot find resource `gdata:/b`.");
        error.frames.push(RuntimeFrame {
            anonymous: true,
            span: Span::new(body.find("read*").unwrap(), body.find('\n').unwrap()),
            member: PklString::default(),
            source: detached.then(|| retained.clone()),
        });
        let rendered = runtime_error_text(
            source,
            &uri,
            &name,
            query,
            error.clone(),
            Some(query.find(".toString").unwrap()),
        );
        assert_eq!(
            rendered
                .encode_utf16()
                .flat_map(u16::to_be_bytes)
                .collect::<Vec<_>>(),
            expected,
            "full actual Java anonymous frame, detached={detached}"
        );
        assert_eq!(
            retained.module_name, name,
            "diagnostic policy must not change module identity"
        );
        assert_eq!(retained.uri, uri);
        assert_eq!(retained.text.original(), body);
        error.frames[0].anonymous = false;
        assert_ne!(
            runtime_error_text(
                source,
                &uri,
                &name,
                query,
                error,
                Some(query.find(".toString").unwrap())
            ),
            rendered,
            "ordinary module root keeps its authored name"
        );
    }
}
