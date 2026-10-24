//! Full-byte imported diagnostics use the unchanged pinned Java fixture origins.
//! URI strings deliberately retain those observations' actual source identities.
#[path = "support/module_keys.rs"]
mod module_keys;
use std::path::{Path, PathBuf};

use pkl_runtime::{
    evaluation::session::{SourceSession, SourceSessionOptions},
    host::HostPoll,
    logging::LogDelivery,
    modules::{ModuleCompletion, ModuleHostError, ModuleRequestKind, ModuleSource, ResolvedModule},
    value::{RuntimeError, Value},
};

#[derive(Clone, Copy, PartialEq)]
enum Scenario {
    Parse,
    Init,
    Source,
    OuterFailure,
    InnerFailure,
    EmptyPublication,
}

struct Fixture {
    id: &'static str,
    origin: &'static str,
    uri_origin: &'static str,
    scenario: Scenario,
}

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/m2-module-loading")
}

fn bytes(path: impl AsRef<Path>) -> Vec<u8> {
    std::fs::read(path).unwrap()
}

fn text(path: impl AsRef<Path>) -> String {
    String::from_utf8(bytes(path)).unwrap()
}

fn compare_result(session: &SourceSession, result: Result<Value, RuntimeError>, expected: &Path) {
    let (status, stdout, stderr) = match result {
        Ok(Value::String(value)) => (
            b"success\n".as_slice(),
            value.to_java_utf8().into_bytes(),
            vec![],
        ),
        Ok(value) => panic!("expression string result: {value:?}"),
        Err(error) => (
            b"error\n".as_slice(),
            vec![],
            session.format_error(error).into_bytes(),
        ),
    };
    for (channel, actual) in [("status", status), ("stdout", &stdout), ("stderr", &stderr)] {
        let path = expected.with_extension(format!("{channel}.bin"));
        let expected = bytes(&path);
        assert_eq!(
            actual,
            expected,
            "{}\nactual:\n{}",
            path.display(),
            String::from_utf8_lossy(actual)
        );
    }
}

fn drive(
    session: &mut SourceSession,
    fixture: &Fixture,
    loads: &mut usize,
    resolves: &mut usize,
    callback_done: &mut bool,
) -> Result<Value, RuntimeError> {
    for _ in 0..10_000 {
        match session.poll(0, 16) {
            HostPoll::Ready(result) => return result,
            HostPoll::Module { token, request } => {
                let completion = match request.kind {
                    ModuleRequestKind::ResolveKey { .. } => {
                        panic!("public lifecycle request in supplied-source test")
                    }
                    ModuleRequestKind::SelectKey { .. }
                    | ModuleRequestKind::QueryCachePolicy { .. }
                    | ModuleRequestKind::QueryFragmentPaths { .. }
                    | ModuleRequestKind::ResolveImportUri { .. } => module_keys::complete(&request),
                    ModuleRequestKind::CheckImport { .. } => {
                        module_keys::defaults(&request).expect("default import policy")
                    }
                    ModuleRequestKind::Capabilities { .. }
                    | ModuleRequestKind::HasElement { .. } => {
                        panic!("unexpected ModuleKey preparation in this fixture")
                    }
                    ModuleRequestKind::Resolve { original_uri, .. } => {
                        *resolves += 1;
                        ModuleCompletion::Resolved(Ok(ResolvedModule {
                            source_uri: original_uri.clone(),
                            resolved_uri: original_uri.clone(),
                            original_uri,
                        }))
                    }
                    ModuleRequestKind::LoadSource { .. } => {
                        *loads += 1;
                        let load = *loads;
                        if load == 1
                            && matches!(
                                fixture.scenario,
                                Scenario::OuterFailure | Scenario::InnerFailure
                            )
                        {
                            *callback_done = true;
                            session.reenter_path(&token, 0, 0, None).unwrap();
                            let result = drive(session, fixture, loads, resolves, callback_done);
                            compare_result(session, result, &origin(fixture).join("inner-0"));
                        }
                        let failure = match fixture.scenario {
                            Scenario::Source => Some("source-first"),
                            Scenario::OuterFailure if load == 1 => Some("outer-source-after-inner"),
                            Scenario::InnerFailure if load == 2 => Some("source-first"),
                            _ => None,
                        };
                        ModuleCompletion::Source(match failure {
                            Some(message) => Err(ModuleHostError::Io(message.into())),
                            None => {
                                let name =
                                    if matches!(fixture.scenario, Scenario::Parse | Scenario::Init)
                                    {
                                        "bad.pkl"
                                    } else {
                                        "source.pkl"
                                    };
                                Ok(ModuleSource {
                                    text: text(
                                        fixtures().join("graphs").join(fixture.id).join(name),
                                    )
                                    .into(),
                                    inferred_name: if name == "bad.pkl" { "bad" } else { "a" }
                                        .into(),
                                })
                            }
                        })
                    }
                };
                session.complete_module(&token, completion, 0).unwrap();
            }
            HostPoll::Log { token, .. } => {
                if fixture.scenario == Scenario::EmptyPublication && !*callback_done {
                    *callback_done = true;
                    session.reenter_path(&token, 0, 0, None).unwrap();
                    let result = drive(session, fixture, loads, resolves, callback_done);
                    compare_result(session, result, &origin(fixture).join("inner-0"));
                }
                session.acknowledge_log(&token, Ok(()), 0).unwrap();
            }
            HostPoll::Read { .. } => panic!("unexpected resource effect"),
            HostPoll::TransformFrame { .. } => panic!("unexpected default transformer callback"),
            HostPoll::Yielded => {}
        }
        session.collect(&[]);
    }
    panic!("diagnostic fixture did not settle");
}

fn origin(fixture: &Fixture) -> PathBuf {
    fixtures()
        .join("origins")
        .join(fixture.origin)
        .join("cases")
        .join(fixture.id)
        .join("0/calls")
}

#[test]
fn imported_parse_init_source_and_callback_errors_match_complete_java_frames_and_cache() {
    let cases = [
        Fixture {
            id: "parse-failure-cache",
            origin: "stage41-loader-oracle-02",
            uri_origin: "stage41-loader-oracle-02",
            scenario: Scenario::Parse,
        },
        Fixture {
            id: "init-failure-cache",
            origin: "stage41-loader-oracle-02",
            uri_origin: "stage41-loader-oracle-02",
            scenario: Scenario::Init,
        },
        Fixture {
            id: "spi-source-failure-cache",
            origin: "stage41-loader-oracle-01",
            uri_origin: "stage41-loader-oracle-01",
            scenario: Scenario::Source,
        },
        Fixture {
            id: "source-reentry-outer-failure",
            origin: "stage41-module-api-oracle-04",
            uri_origin: "stage41-loader-oracle-03",
            scenario: Scenario::OuterFailure,
        },
        Fixture {
            id: "source-reentry-inner-failure",
            origin: "stage41-module-api-oracle-04",
            uri_origin: "stage41-loader-oracle-03",
            scenario: Scenario::InnerFailure,
        },
        Fixture {
            id: "init-empty-publication",
            origin: "stage41-module-api-oracle-04",
            uri_origin: "stage41-loader-oracle-03",
            scenario: Scenario::EmptyPublication,
        },
    ];
    for fixture in cases {
        let source = text(
            fixtures()
                .join("graphs")
                .join(fixture.id)
                .join("primary.pkl"),
        );
        let uri = format!(
            "file:///Users/cody.ij.hwang/Projects/pkl-rs.stage41/target/m2/{}/workspace/{}/main.pkl",
            fixture.uri_origin, fixture.id
        );
        let paths: Vec<_> = (0..3)
            .map(|index| text(origin(&fixture).join(format!("outer-{index}.expression.bin"))))
            .collect();
        let mut session = SourceSession::compile(
            &source,
            &uri,
            "main",
            &paths,
            SourceSessionOptions {
                log_delivery: LogDelivery::Host,
                ..Default::default()
            },
        )
        .unwrap();
        let (mut loads, mut resolves, mut callback_done) = (0, 0, false);
        for index in 0..3 {
            session.start_path(index, 0, None).unwrap();
            let result = drive(
                &mut session,
                &fixture,
                &mut loads,
                &mut resolves,
                &mut callback_done,
            );
            compare_result(
                &session,
                result,
                &origin(&fixture).join(format!("outer-{index}")),
            );
            session.collect(&[]);
        }
        // Even a callback's successful member value and an outer module failure
        // remain independently cached; none of the later calls reloads source.
        let expected = if matches!(
            fixture.scenario,
            Scenario::OuterFailure | Scenario::InnerFailure
        ) {
            2
        } else {
            1
        };
        assert_eq!((resolves, loads), (expected, expected), "{}", fixture.id);
        session.close();
        assert_eq!(session.heap_stats().live, 0);
    }
}

#[test]
fn import_declaration_and_inline_failures_keep_exact_phase_specific_java_frames() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/m2-module-import-frames/cases");
    for phase in ["resolve", "source", "parse", "init"] {
        for syntax in ["declaration", "inline"] {
            let fixture = root.join(format!("{phase}-{syntax}"));
            for repeat in 0..2 {
                let source = text(fixture.join("main.pkl"));
                let imported = text(fixture.join("source.pkl"));
                let uri = text(fixture.join("module-uri.bin"));
                let mut session = SourceSession::compile(
                    &source,
                    &uri,
                    "main",
                    &["result".into()],
                    SourceSessionOptions::default(),
                )
                .unwrap();
                session.start_path(0, 0, None).unwrap();
                let (mut resolves, mut loads) = (0, 0);
                let result = loop {
                    match session.poll(0, 16) {
                        HostPoll::Ready(result) => break result,
                        HostPoll::Module { token, request } => {
                            let completion = match request.kind {
                                ModuleRequestKind::ResolveKey { .. } => {
                                    panic!("public lifecycle request in supplied-source test")
                                }
                                ModuleRequestKind::SelectKey { .. }
                                | ModuleRequestKind::QueryCachePolicy { .. }
                                | ModuleRequestKind::QueryFragmentPaths { .. }
                                | ModuleRequestKind::ResolveImportUri { .. } => {
                                    module_keys::complete(&request)
                                }
                                ModuleRequestKind::CheckImport { .. } => {
                                    module_keys::defaults(&request).expect("default import policy")
                                }
                                ModuleRequestKind::Capabilities { .. }
                                | ModuleRequestKind::HasElement { .. } => {
                                    panic!("unexpected ModuleKey preparation in this fixture")
                                }
                                ModuleRequestKind::Resolve { original_uri, .. } => {
                                    resolves += 1;
                                    ModuleCompletion::Resolved(if phase == "resolve" {
                                        Err(ModuleHostError::NotFound)
                                    } else {
                                        Ok(ResolvedModule {
                                            source_uri: original_uri.clone(),
                                            resolved_uri: original_uri.clone(),
                                            original_uri,
                                        })
                                    })
                                }
                                ModuleRequestKind::LoadSource { .. } => {
                                    loads += 1;
                                    ModuleCompletion::Source(if phase == "source" {
                                        Err(ModuleHostError::Io("source-first".into()))
                                    } else {
                                        Ok(ModuleSource {
                                            text: imported.as_str().into(),
                                            inferred_name: "child".into(),
                                        })
                                    })
                                }
                            };
                            session.complete_module(&token, completion, 0).unwrap();
                        }
                        HostPoll::TransformFrame { .. } => {
                            panic!("unexpected default transformer callback")
                        }
                        HostPoll::Yielded => {}
                        other => panic!("unexpected effect: {other:?}"),
                    }
                    session.collect(&[]);
                };
                let error = result.unwrap_err();
                let rendered = session.format_error(error.clone());
                let expected = fixture.join(repeat.to_string()).join("outer-0");
                compare_result(&session, Err(error), &expected);
                let units: Vec<_> = rendered.encode_utf16().flat_map(u16::to_be_bytes).collect();
                assert_eq!(
                    units,
                    bytes(expected.with_extension("stderr.code-units.bin"))
                );
                assert_eq!((resolves, loads), (1, usize::from(phase != "resolve")));
                session.close();
                assert_eq!(session.heap_stats().live, 0);
            }
        }
    }
}
