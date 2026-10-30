//! Complete retained Java bytes plus general shared-engine ownership invariants.
//! Renamed/radix cases below are invariants, not newly acquired Java comparisons.
use pkl_runtime::{
    evaluation::{
        EvaluationOptions,
        session::{SourceSession, SourceSessionOptions},
    },
    host::HostPoll,
    logging::{FrameTransformer, LogDelivery, LogEvent, LogFrame, LogLevel},
    modules::{
        self, ModuleCapabilities, ModuleCompletion, ModuleKey, ModuleKeyKind, ModuleRequestKind,
        ModuleSource, ModuleSourceSpec, ResolvedModule, ResolvedModuleKey,
    },
    source::SourceText,
    value::PklString,
};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};

struct Host {
    primaries: Vec<PklString>,
    children: Vec<(PklString, SourceText)>,
}
#[derive(Default)]
struct Observation {
    result: Option<Result<PklString, PklString>>,
    factories: Vec<(PklString, bool)>,
    frames: Vec<(LogFrame, LogFrame)>,
    logs: Vec<Arc<LogEvent>>,
}
fn evaluator() -> SourceSession {
    SourceSession::module_evaluator(SourceSessionOptions {
        evaluation: EvaluationOptions {
            power_assertions: false,
        },
        frame_transformer: FrameTransformer::Host,
        log_delivery: LogDelivery::Host,
        ..SourceSessionOptions::default()
    })
}
fn evaluate(
    session: &mut SourceSession,
    host: &Host,
    uri: &PklString,
    source: &SourceText,
    query: &str,
    identity: bool,
) -> Observation {
    let mut observed = Observation::default();
    session
        .start_evaluation(
            ModuleSourceSpec {
                uri: uri.clone(),
                contents: Some(source.clone()),
            },
            query,
            0,
            None,
        )
        .unwrap();
    for _ in 0..50_000 {
        match session.poll(0, 64) {
            HostPoll::Ready(result) => {
                observed.result = Some(match result {
                    Ok(value) => Ok(SourceSession::result_string(value).unwrap()),
                    Err(error) => Err(session.format_error_text(error)),
                });
                return observed;
            }
            HostPoll::Yielded => (),
            HostPoll::Module { token, request } => {
                let completion = match &request.kind {
                    ModuleRequestKind::SelectKey { id, uri, .. } => {
                        let child = host.children.iter().any(|(key, _)| key == uri);
                        observed.factories.push((uri.clone(), child));
                        if child || host.primaries.contains(uri) {
                            ModuleCompletion::KeySelected(Ok(ModuleKey {
                                id: *id,
                                uri: uri.clone(),
                                kind: if child {
                                    ModuleKeyKind::Synthetic
                                } else {
                                    ModuleKeyKind::Host
                                },
                                handle: "owner-regression".into(),
                            }))
                        } else {
                            modules::default_completion(&request).expect("bundled key")
                        }
                    }
                    ModuleRequestKind::QueryCachePolicy { key }
                        if key.handle == "owner-regression" =>
                    {
                        ModuleCompletion::CachePolicy(Ok(key.kind != ModuleKeyKind::Synthetic))
                    }
                    ModuleRequestKind::Resolve {
                        key,
                        resolved_key_id,
                        ..
                    }
                    | ModuleRequestKind::ResolveKey {
                        key,
                        resolved_key_id,
                        ..
                    } if key.handle == "owner-regression" => {
                        ModuleCompletion::KeyResolved(Ok(ResolvedModuleKey {
                            id: *resolved_key_id,
                            key: key.clone(),
                            handle: key.handle.clone(),
                            module: ResolvedModule {
                                original_uri: key.uri.clone(),
                                resolved_uri: key.uri.clone(),
                                source_uri: key.uri.clone(),
                            },
                        }))
                    }
                    ModuleRequestKind::Capabilities { key: Some(key), .. }
                        if key.handle == "owner-regression" =>
                    {
                        ModuleCompletion::Capabilities(Ok(ModuleCapabilities {
                            local: true,
                            hierarchical: key.kind != ModuleKeyKind::Synthetic,
                            fragment_paths: false,
                        }))
                    }
                    ModuleRequestKind::LoadSource { key, .. }
                        if key.key.handle == "owner-regression" =>
                    {
                        let source = host
                            .children
                            .iter()
                            .find(|(uri, _)| uri == &key.key.uri)
                            .expect("registered child")
                            .1
                            .clone();
                        ModuleCompletion::Source(Ok(ModuleSource {
                            text: source,
                            inferred_name: PklString::default(),
                        }))
                    }
                    _ => modules::default_completion(&request)
                        .expect("configured default module effect"),
                };
                session.complete_module(&token, completion, 0).unwrap();
            }
            HostPoll::TransformFrame { token, frame } => {
                let transformed = if identity {
                    (*frame).clone()
                } else {
                    FrameTransformer::Default.transform((*frame).clone())
                };
                observed
                    .frames
                    .push(((*frame).clone(), transformed.clone()));
                session
                    .complete_frame_transform(&token, Ok(transformed), 0)
                    .unwrap();
            }
            HostPoll::Log { token, event } => {
                observed.logs.push(event);
                session.acknowledge_log(&token, Ok(()), 0).unwrap();
            }
            other => panic!("unexpected Host effect: {other:?}"),
        }
    }
    panic!("bounded regression did not settle")
}
fn units(path: &Path) -> Vec<u16> {
    let bytes = fs::read(path).unwrap();
    let (pairs, rest) = bytes.as_chunks::<2>();
    assert!(rest.is_empty());
    pairs.iter().map(|p| u16::from_be_bytes(*p)).collect()
}
fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/m2-imported-ast-owner-oracle")
}
fn fixture_source(path: &Path) -> SourceText {
    SourceText::from_utf16(units(&path.with_extension("code-units.bin")))
}
fn expect_text(path: &Path, name: &str, actual: &PklString) {
    assert_eq!(
        actual.encode_utf16().collect::<Vec<_>>(),
        units(&path.join(format!("{name}.code-units.bin"))),
        "complete UTF16 {name}"
    );
    assert_eq!(
        actual.to_java_utf8().as_bytes(),
        fs::read(path.join(format!("{name}.bin"))).unwrap(),
        "complete Java UTF8 {name}"
    );
}
fn integer(path: &Path) -> usize {
    let value = fs::read_to_string(path).unwrap();
    value.strip_suffix('\n').unwrap().parse().unwrap()
}
fn expect_frame(path: &Path, actual: &LogFrame) {
    expect_text(path, "module-uri", &actual.module_uri);
    let presence = fs::read(path.join("member-name.present.bin")).unwrap();
    assert_eq!(
        presence,
        if actual.member_name.is_some() {
            b"true\n".to_vec()
        } else {
            b"false\n".to_vec()
        }
    );
    if let Some(name) = &actual.member_name {
        expect_text(path, "member-name", name);
    }
    assert_eq!(
        format!(
            "{}\t{}\t{}\t{}\n",
            actual.start_line, actual.start_column, actual.end_line, actual.end_column
        )
        .as_bytes(),
        fs::read(path.join("positions.tsv")).unwrap()
    );
    assert_eq!(
        actual.source_lines.len(),
        integer(&path.join("source-line-count.bin"))
    );
    for (index, line) in actual.source_lines.iter().enumerate() {
        expect_text(path, &format!("source-line-{index:04}"), line);
    }
}
fn expect_observation(path: &Path, observation: Observation) {
    let status = fs::read(path.join("status.bin")).unwrap();
    match (status.as_slice(), observation.result.unwrap()) {
        (b"success\n", Ok(value)) => expect_text(path, "string", &value),
        (b"error\n", Err(error)) => {
            assert_eq!(fs::read(path.join("error.present.bin")).unwrap(), b"true\n");
            expect_text(path, "error", &error);
        }
        other => panic!("ordinary status/result mismatch: {other:?}"),
    }
    assert_eq!(
        observation.factories.len(),
        integer(&path.join("factory-count.bin"))
    );
    for (index, (uri, child)) in observation.factories.iter().enumerate() {
        expect_text(path, &format!("factory-{index:04}-uri"), uri);
        assert_eq!(
            format!("{child}\n").as_bytes(),
            fs::read(path.join(format!("factory-{index:04}-selected-child.bin"))).unwrap()
        );
    }
    assert_eq!(
        format!("{}\t{}\n", observation.frames.len(), observation.logs.len()).as_bytes(),
        fs::read(path.join("counts.tsv")).unwrap()
    );
    for (index, (input, output)) in observation.frames.iter().enumerate() {
        expect_frame(&path.join(format!("frame-{index:04}-input")), input);
        expect_frame(&path.join(format!("frame-{index:04}-output")), output);
    }
    for (index, event) in observation.logs.iter().enumerate() {
        let path = path.join(format!("logger-{index:04}"));
        expect_text(
            &path,
            "level",
            &match event.level {
                LogLevel::Trace => "trace".into(),
                LogLevel::Warn => "warn".into(),
            },
        );
        expect_text(&path, "message", &event.message);
        expect_frame(&path.join("frame"), &event.frame);
    }
}

#[test]
fn imported_ast_failure_matches_complete_retained_java_bytes_and_callback_frames() {
    let root = fixture_root();
    let primary = fixture_source(&root.join("module.pkl"));
    let recovery = fixture_source(&root.join("recovery.pkl"));
    let uri: PklString = fs::read_to_string(root.join("uri.txt")).unwrap().into();
    let recovery_uri: PklString = fs::read_to_string(root.join("recovery-uri.txt"))
        .unwrap()
        .into();
    let registry = fs::read_to_string(root.join("modules.tsv")).unwrap();
    let mut children: Vec<(PklString, SourceText)> = Vec::new();
    for row in registry.strip_suffix('\n').unwrap().split('\n') {
        let (uri, path) = row.split_once('\t').unwrap();
        children.push((uri.into(), fixture_source(&root.join(path))));
    }
    let host = Host {
        primaries: vec![uri.clone(), recovery_uri.clone()],
        children,
    };
    for identity in [false, true] {
        let mut session = evaluator();
        let profile = if identity { "identity" } else { "default" };
        for round in 0..2 {
            let expected = root
                .join("observations")
                .join(profile)
                .join(format!("round-{round}"));
            expect_observation(
                &expected.join("q0000"),
                evaluate(&mut session, &host, &uri, &primary, "result", identity),
            );
            expect_observation(
                &expected.join("q0001"),
                evaluate(
                    &mut session,
                    &host,
                    &recovery_uri,
                    &recovery,
                    "stable",
                    identity,
                ),
            );
            session.collect(&[]);
        }
        session.close();
        assert!(session.is_closed());
        assert_eq!(session.heap_stats().slots, 0);
    }
    // Eight retained ordinary observations; this is not a JVM exception-class/cause adapter.
}

#[test]
fn imported_unforced_overflows_keep_active_property_method_and_class_owners() {
    let primary_uri: PklString = "file:///owner-regression/parent.pkl".into();
    let child_uri: PklString = "file:///owner-regression/child.pkl".into();
    let recovery_uri: PklString = "file:///owner-regression/recovery.pkl".into();
    let recovery: SourceText = "module recovery.clean\nstable = \"recovered\"\n".into();
    let cases = [
        (
            "module owner.alpha\nprobe = import(\"child.pkl\").value\n",
            "probe",
            "owner.alpha#probe",
        ),
        (
            "module owner.beta\nfunction load() = import(\"child.pkl\").value\n",
            "load()",
            "owner.beta#load",
        ),
        (
            "module owner.gamma\nclass Holder { load = import(\"child.pkl\").value }\nroot = new Holder {}\n",
            "root.load",
            "owner.gamma#Holder.load",
        ),
    ];
    for (parent, query, owner) in cases {
        for number in [
            "99999999999999999999999999999999999999",
            "0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF",
            "-99999999999999999999999999999999999999",
        ] {
            let child: SourceText =
                format!("module leaf.unrelated\nvalue = \"unreachable\"\nunforced = {number}\n")
                    .into();
            let primary: SourceText = parent.into();
            let host = Host {
                primaries: vec![primary_uri.clone(), recovery_uri.clone()],
                children: vec![(child_uri.clone(), child)],
            };
            for identity in [false, true] {
                let mut session = evaluator();
                for _ in 0..2 {
                    let failed =
                        evaluate(&mut session, &host, &primary_uri, &primary, query, identity);
                    assert!(failed.result.unwrap().is_err());
                    let (input, output) = failed
                        .frames
                        .first()
                        .expect("imported AST diagnostic frame");
                    assert_eq!(input.member_name, Some(owner.into()));
                    assert_eq!(output.member_name, Some(owner.into()));
                    assert_eq!(input.module_uri, child_uri);
                    assert_eq!(output.module_uri, child_uri);
                    assert_eq!(
                        input.source_lines.as_ref(),
                        &[PklString::from(format!("unforced = {number}"))]
                    );
                    let recovered = evaluate(
                        &mut session,
                        &host,
                        &recovery_uri,
                        &recovery,
                        "stable",
                        identity,
                    );
                    assert_eq!(recovered.result.unwrap(), Ok("recovered".into()));
                    session.collect(&[]);
                }
                session.close();
                assert_eq!(session.heap_stats().slots, 0);
            }
        }
    }
    // These renamed/radix scenarios verify an upstream ownership invariant.
    // Their expected complete Java payloads have not been newly acquired.
}

#[test]
fn direct_primary_ast_owner_and_imported_parser_version_locations_stay_distinct() {
    let primary_uri: PklString = "file:///owner-controls/primary.pkl".into();
    let child_uri: PklString = "file:///owner-controls/child.pkl".into();
    let mut direct = evaluator();
    let source: SourceText = "module leaf.direct\nvalue = \"unused\"\nunforced = 99999999999999999999999999999999999999\n".into();
    let host = Host {
        primaries: vec![primary_uri.clone()],
        children: vec![],
    };
    let error = evaluate(&mut direct, &host, &primary_uri, &source, "value", true);
    assert!(error.result.unwrap().is_err());
    assert_eq!(
        error.frames.first().unwrap().0.member_name,
        Some("leaf.direct#unforced".into())
    );
    direct.close();
    let parent: SourceText = "module owner.control\nprobe = import(\"child.pkl\").value\n".into();
    for (child, source_uri) in [
        ("module leaf.syntax\nvalue = (\n", &child_uri),
        (
            "@ModuleInfo { minPklVersion = \"99.9.9\" }\nmodule leaf.version\nvalue = (\n",
            &primary_uri,
        ),
    ] {
        let host = Host {
            primaries: vec![primary_uri.clone()],
            children: vec![(child_uri.clone(), child.into())],
        };
        let mut session = evaluator();
        let error = evaluate(&mut session, &host, &primary_uri, &parent, "probe", true);
        assert!(error.result.unwrap().is_err());
        let frame = &error.frames.first().unwrap().0;
        assert_eq!(frame.member_name, Some("owner.control#probe".into()));
        assert_eq!(&frame.module_uri, source_uri);
        session.close();
    }
}
