//! Full actual Java parser/API channels. Process/cause classes stay provenance.
#[path = "support/module_keys.rs"]
mod module_keys;
use pkl_runtime::{
    modules::*,
    parser::{parse_expression, parse_module},
    probe::{SourceFrame, SourceProbe},
    source::SourceMap,
    value::PklString,
};
use std::{
    fs,
    path::{Path, PathBuf},
};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/m2-quoted-identifier-contract")
}

type FrameSnapshot = (
    Vec<u16>,
    Option<Vec<u16>>,
    Vec<Vec<u16>>,
    u32,
    u32,
    u32,
    u32,
);
fn frame_snapshot(frame: &SourceFrame) -> FrameSnapshot {
    (
        frame.module_uri.clone(),
        frame.member_name.clone(),
        frame.source_lines.clone(),
        frame.start_line,
        frame.start_column,
        frame.end_line,
        frame.end_column,
    )
}

fn assert_java_frame(directory: &Path, actual: &SourceFrame) {
    field(directory, "module-uri", &actual.module_uri);
    let member_present = fs::read(directory.join("member-name.present.bin")).unwrap();
    assert_eq!(
        member_present,
        if actual.member_name.is_some() {
            b"true\n".as_slice()
        } else {
            b"false\n".as_slice()
        }
    );
    if let Some(member) = &actual.member_name {
        field(directory, "member-name", member);
    }
    assert_eq!(
        fs::read(directory.join("positions.tsv")).unwrap(),
        format!(
            "{}\t{}\t{}\t{}\n",
            actual.start_line, actual.start_column, actual.end_line, actual.end_column
        )
        .as_bytes()
    );
    assert_eq!(
        fs::read(directory.join("source-line-count.bin")).unwrap(),
        format!("{}\n", actual.source_lines.len()).as_bytes()
    );
    for (index, line) in actual.source_lines.iter().enumerate() {
        field(directory, &format!("source-line-{index:04}"), line);
    }
}

#[test]
fn imported_eof_origin_is_adjusted_once_and_nested_callers_keep_their_coordinates_on_retry() {
    // The former two-owner-frame assertion failed and is preserved in validation01.
    // Actual Java supplies complete input/output callback frames and public error
    // bytes for this identical supplied-module + expression-query API boundary.
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/m2-quoted-identifier-imported-owner");
    let inputs = root.join("inputs");
    let owner = fs::read_to_string(inputs.join("module.pkl")).unwrap();
    let owner_uri = fs::read_to_string(inputs.join("uri.txt")).unwrap();
    let child = fs::read_to_string(inputs.join("child.pkl")).unwrap();
    let child_uri = fs::read_to_string(inputs.join("child-uri.txt")).unwrap();
    assert_eq!(
        owner.encode_utf16().collect::<Vec<_>>(),
        units(&fs::read(inputs.join("module.code-units.bin")).unwrap())
    );
    assert_eq!(
        child.encode_utf16().collect::<Vec<_>>(),
        units(&fs::read(inputs.join("child.code-units.bin")).unwrap())
    );
    let child_key_uri = child_uri.replacen("file:///", "file:/", 1);
    let owner_key_uri = owner_uri.replacen("file:///", "file:/", 1);
    assert_eq!(
        fs::read(inputs.join("profiles.tsv")).unwrap(),
        b"default\nidentity\n"
    );
    for profile in ["default", "identity"] {
        let mut previous = None;
        let mut probe = SourceProbe::new_module_evaluator("compact", false, true, false).unwrap();
        for query in fs::read_to_string(inputs.join("queries.tsv"))
            .unwrap()
            .lines()
        {
            let (query_id, expression) = query.split_once('\t').unwrap();
            let expected = root.join("observations").join(profile).join(query_id);
            assert_eq!(fs::read(expected.join("status.bin")).unwrap(), b"error\n");
            field(
                &expected,
                "expression",
                &expression.encode_utf16().collect::<Vec<_>>(),
            );
            let expected_count = fs::read_to_string(expected.join("counts.tsv"))
                .unwrap()
                .split_whitespace()
                .next()
                .unwrap()
                .parse::<usize>()
                .unwrap();
            probe
                .evaluate_module_source(&owner_uri, Some(&owner), expression, 0, None)
                .unwrap();
            let mut frames = Vec::new();
            let mut tokens = std::collections::HashSet::new();
            let mut terminal = None;
            for _ in 0..1_000_000_u32.div_ceil(64) {
                let event = probe.poll(0, 64).unwrap();
                probe.collect();
                match event.kind {
                    "yield" => continue,
                    "module-request" => {
                        let token = event.token.as_deref().unwrap();
                        let request = probe.module_request(token).unwrap();
                        let completion = module_keys::defaults(&request).unwrap_or_else(|| {
                            match &request.kind {
                                ModuleRequestKind::ResolveKey {
                                    key,
                                    resolved_key_id,
                                    ..
                                }
                                | ModuleRequestKind::Resolve {
                                    key,
                                    resolved_key_id,
                                    ..
                                } => {
                                    let source_uri = if key.uri == child_uri
                                        || key.uri == child_key_uri
                                    {
                                        child_uri.as_str()
                                    } else {
                                        assert!(key.uri == owner_uri || key.uri == owner_key_uri);
                                        owner_uri.as_str()
                                    };
                                    ModuleCompletion::KeyResolved(Ok(ResolvedModuleKey {
                                        id: *resolved_key_id,
                                        key: key.clone(),
                                        handle: "resolved".into(),
                                        module: ResolvedModule {
                                            original_uri: key.uri.clone(),
                                            // The Java child synthetic key explicitly resolves to
                                            // the authored source URI; selected key identity stays normalized.
                                            resolved_uri: if source_uri == child_uri {
                                                source_uri.into()
                                            } else {
                                                key.uri.clone()
                                            },
                                            source_uri: source_uri.into(),
                                        },
                                    }))
                                }
                                ModuleRequestKind::LoadSource { resolved, .. } => {
                                    assert_eq!(resolved.source_uri, child_uri.as_str());
                                    ModuleCompletion::Source(Ok(ModuleSource {
                                        text: child.clone().into(),
                                        inferred_name: "ignoredWrongOwner".into(),
                                    }))
                                }
                                ModuleRequestKind::Capabilities { key, .. } => {
                                    // Java's explicit child synthetic key is local and nonhierarchical;
                                    // the supplied primary retains the normal file factory profile.
                                    let child_key = key.as_ref().is_some_and(|key| {
                                        key.uri == child_uri || key.uri == child_key_uri
                                    });
                                    ModuleCompletion::Capabilities(Ok(ModuleCapabilities {
                                        local: true,
                                        hierarchical: !child_key,
                                        fragment_paths: false,
                                    }))
                                }
                                _ => panic!("unexpected import Host effect: {request:?}"),
                            }
                        });
                        probe.complete_module(token, completion, 0).unwrap();
                    }
                    "transform-frame" => {
                        assert!(
                            frames.len() < expected_count,
                            "extra callback for {profile}/{query_id}"
                        );
                        let token = event.token.as_deref().unwrap();
                        assert!(
                            tokens.insert(token.to_owned()),
                            "each compressed frame is transformed once"
                        );
                        let frame = event.frame.unwrap();
                        assert_java_frame(
                            &expected.join(format!("frame-{:04}-input", frames.len())),
                            &frame,
                        );
                        // Both actual Java forwarding profiles leave this finite file/repl
                        // domain unchanged. Check the full output before forwarding the
                        // observed frame; do not inject expected frames into the evaluator.
                        assert_java_frame(
                            &expected.join(format!("frame-{:04}-output", frames.len())),
                            &frame,
                        );
                        frames.push(frame_snapshot(&frame));
                        probe
                            .complete_frame_transform(token, Some(frame), None, 0)
                            .unwrap();
                    }
                    "read" => {
                        assert_eq!(event.uri.as_deref(), Some("prop:pkl.outputFormat"));
                        probe
                            .complete_read_missing(event.token.as_deref().unwrap(), 0)
                            .unwrap();
                    }
                    "error" => {
                        terminal = Some(event);
                        break;
                    }
                    _ => panic!("unexpected imported diagnostic event: {event:?}"),
                }
            }
            let event = terminal.expect("bounded imported diagnostic");
            assert_eq!(event.disposition, Some("language-error"));
            assert_eq!(frames.len(), expected_count);
            let error = event.error.unwrap();
            field(&expected, "error", &error);
            let snapshot = (frames, error);
            if let Some(previous) = &previous {
                assert_eq!(&snapshot, previous);
            } else {
                previous = Some(snapshot);
            }
        }
        probe.close();
        assert_eq!(probe.collect().slots, 0);
    }
}
fn units(bytes: &[u8]) -> Vec<u16> {
    let (pairs, tail) = bytes.as_chunks::<2>();
    assert!(tail.is_empty());
    pairs.iter().map(|pair| u16::from_be_bytes(*pair)).collect()
}
fn field(directory: &Path, name: &str, actual: &[u16]) {
    assert_eq!(
        units(&fs::read(directory.join(format!("{name}.code-units.bin"))).unwrap()),
        actual,
        "{} {name}: UTF16",
        directory.display()
    );
    assert_eq!(
        fs::read(directory.join(format!("{name}.bin"))).unwrap(),
        PklString::from_utf16(actual.to_vec())
            .to_java_utf8()
            .as_bytes(),
        "{} {name}: UTF8",
        directory.display()
    );
}

#[test]
fn actual_java_quoted_module_and_expression_parser_contracts() {
    compare_parser_contracts(&root(), 38);
}

fn compare_parser_contracts(root: &Path, count: usize) {
    let index = fs::read_to_string(root.join("cases.tsv")).unwrap();
    assert_eq!(index.lines().count(), count);
    for line in index.lines() {
        let (id, entry) = line.split_once('\t').unwrap();
        let directory = root.join(id);
        let text = fs::read_to_string(directory.join("source.pkl")).unwrap();
        assert_eq!(
            text.encode_utf16().collect::<Vec<_>>(),
            units(&fs::read(directory.join("source.code-units.bin")).unwrap())
        );
        let result = if entry == "module" {
            parse_module(&text).map(|_| ())
        } else {
            assert_eq!(entry, "expression");
            parse_expression(&text).map(|_| ())
        };
        assert_eq!(
            fs::read(directory.join("parser/status.bin")).unwrap(),
            if result.is_ok() {
                b"success\n".as_slice()
            } else {
                b"error\n".as_slice()
            },
            "parser status: {id}"
        );
        if let Err(error) = result {
            field(
                &directory.join("parser"),
                "error.message",
                &error.message.encode_utf16().collect::<Vec<_>>(),
            );
            let mut map = SourceMap::default();
            let source = map.add(id, text);
            let (start, length) = map.get(source).unwrap().utf16_span(error.span).unwrap();
            let span = fs::read_to_string(directory.join("parser/error.span.tsv"))
                .unwrap()
                .split_whitespace()
                .map(|v| v.parse::<usize>().unwrap())
                .collect::<Vec<_>>();
            assert_eq!(
                (
                    start + error.highlight_offset_utf16(),
                    error.highlight_length_utf16.unwrap_or(length)
                ),
                (span[0], span[1]),
                "parser effective UTF16 span: {id}"
            );
        }
    }
}

#[test]
fn actual_java_quoted_public_module_and_expression_channels() {
    compare_public_channels(&root(), 38);
}

fn compare_public_channels(root: &Path, count: usize) {
    let index = fs::read_to_string(root.join("cases.tsv")).unwrap();
    assert_eq!(index.lines().count(), count);
    let mut passed = 0;
    let mut mismatched = Vec::new();
    let mut aborted = Vec::new();
    for line in index.lines() {
        let (id, entry) = line.split_once('\t').unwrap();
        let directory = root.join(id);
        // Only the test runner catches assertions. Every required row is attempted;
        // an aborted row is a required failure and is never counted as a pass.
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            compare_public_row(&directory, id, entry)
        })) {
            Ok(true) => {
                passed += 1;
                eprintln!("required-api-row id={id} exact_match=true");
            }
            Ok(false) => {
                mismatched.push(id.to_owned());
                eprintln!("required-api-row id={id} exact_match=false");
            }
            Err(_) => {
                aborted.push(id.to_owned());
                eprintln!("required-api-row id={id} comparison_aborted=true");
            }
        }
    }
    eprintln!(
        "required-api-inventory total={count} passed={passed} mismatched={} aborted={}",
        mismatched.len(),
        aborted.len()
    );
    assert_eq!(passed + mismatched.len() + aborted.len(), count);
    assert!(
        mismatched.is_empty() && aborted.is_empty(),
        "required API failures: mismatched={mismatched:?}; aborted={aborted:?}"
    );
    assert_eq!(passed, count);
}

fn public_field_matches(directory: &Path, name: &str, actual: Option<&[u16]>) -> bool {
    let expected_units =
        units(&fs::read(directory.join(format!("{name}.code-units.bin"))).unwrap());
    let expected_bytes = fs::read(directory.join(format!("{name}.bin"))).unwrap();
    let actual_bytes = actual.map(|value| {
        PklString::from_utf16(value.to_vec())
            .to_java_utf8()
            .as_bytes()
            .to_vec()
    });
    let units_match = actual == Some(expected_units.as_slice());
    let bytes_match = actual_bytes.as_deref() == Some(expected_bytes.as_slice());
    // Preserve complete raw channels for every mismatching row; neither channel
    // comparison short-circuits the other, and later rows still run.
    if !units_match || !bytes_match {
        eprintln!(
            "{} {name}: UTF16 expected={expected_units:?} actual={actual:?}; UTF8 expected={expected_bytes:?} actual={actual_bytes:?}",
            directory.display()
        );
    }
    units_match && bytes_match
}

fn compare_public_row(directory: &Path, id: &str, entry: &str) -> bool {
    let text = fs::read_to_string(directory.join("source.pkl")).unwrap();
    let uri = fs::read_to_string(directory.join("uri.txt")).unwrap();
    let mut probe = SourceProbe::new_module_evaluator("compact", true, false, false).unwrap();
    // Java evaluateExpressionString special-cases output.text by calling
    // evaluateOutputText directly. Only module rows use that operation;
    // expression rows retain the original expression and an empty module.
    let (module, expression) = if entry == "module" {
        (text.as_str(), "output.text")
    } else {
        assert_eq!(entry, "expression");
        ("", text.as_str())
    };
    probe
        .evaluate_module_source(&uri, Some(module), expression, 0, None)
        .unwrap();
    let mut terminal = None;
    for _ in 0..1_000_000_u32.div_ceil(64) {
        probe.collect();
        let event = probe.poll(0, 64).unwrap();
        probe.collect();
        match event.kind {
            "yield" => continue,
            "module-request" => {
                assert!(
                    module_keys::complete_probe(&mut probe, event.token.as_deref().unwrap(), 0)
                        .unwrap()
                );
                continue;
            }
            "read" => {
                assert_eq!(event.uri.as_deref(), Some("prop:pkl.outputFormat"));
                probe
                    .complete_read_missing(event.token.as_deref().unwrap(), 0)
                    .unwrap();
                continue;
            }
            "value" | "error" => {
                terminal = Some(event);
                break;
            }
            _ => panic!("unexpected Host effect for {id}: {event:?}"),
        }
    }
    let event = terminal.unwrap_or_else(|| panic!("bounded quoted-identifier API row: {id}"));
    probe.close();
    assert_eq!(
        probe.collect().slots,
        0,
        "closed evaluator owns no retained heap slots: {id}"
    );
    let expected = directory.join("evaluator");
    let expected_status = fs::read(expected.join("status.bin")).unwrap();
    let actual_status = match event.kind {
        "value" if event.error.is_none() => b"success\n".as_slice(),
        "error" if event.disposition == Some("language-error") && event.value.is_none() => {
            b"error\n".as_slice()
        }
        _ => b"unsupported-or-api-error\n".as_slice(),
    };
    let status_match = expected_status == actual_status;
    if !status_match {
        eprintln!(
            "{id}: status expected={expected_status:?} actual={actual_status:?}; event={event:?}"
        );
    }
    let payload_match = match expected_status.as_slice() {
        b"success\n" => public_field_matches(&expected, "string", event.value.as_deref()),
        b"error\n" => public_field_matches(&expected, "error", event.error.as_deref()),
        _ => panic!("unadmitted Java status: {id}"),
    };
    status_match && payload_match
}

#[test]
fn actual_java_property_definition_parser_boundaries() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/m2-quoted-identifier-property-boundaries");
    compare_parser_contracts(&root, 13);
}

#[test]
fn actual_java_property_definition_public_channels() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/m2-quoted-identifier-property-boundaries");
    // All acquired Java API outcomes are required, including the five parsed
    // programs' runtime paths. A mismatch remains a required failure.
    compare_public_channels(&root, 13);
}
