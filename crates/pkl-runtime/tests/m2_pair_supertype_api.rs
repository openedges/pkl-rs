//! Actual Java public API rows; raw callback frames and both forwarding profiles.
#[path = "support/module_keys.rs"]
mod module_keys;
use pkl_runtime::{
    logging::{FrameTransformer, LogFrame},
    probe::{SourceFrame, SourceProbe},
    value::PklString,
};
use std::{fs, path::Path};

fn units(bytes: &[u8]) -> Vec<u16> {
    let (pairs, tail) = bytes.as_chunks::<2>();
    assert!(tail.is_empty());
    pairs.iter().map(|pair| u16::from_be_bytes(*pair)).collect()
}

fn field(directory: &Path, name: &str, actual: &[u16]) -> bool {
    let expected_units =
        units(&fs::read(directory.join(format!("{name}.code-units.bin"))).unwrap());
    let expected_bytes = fs::read(directory.join(format!("{name}.bin"))).unwrap();
    let actual_bytes = PklString::from_utf16(actual.to_vec())
        .to_java_utf8()
        .into_bytes();
    let equal = expected_units == actual && expected_bytes == actual_bytes;
    if !equal {
        eprintln!(
            "{} {name}: UTF16 expected={expected_units:?} actual={actual:?}; UTF8 expected={expected_bytes:?} actual={actual_bytes:?}",
            directory.display()
        );
    }
    equal
}

fn frame(directory: &Path, actual: &SourceFrame) -> bool {
    let mut equal = field(directory, "module-uri", &actual.module_uri);
    let expected_member = fs::read(directory.join("member-name.present.bin")).unwrap();
    equal &= expected_member
        == if actual.member_name.is_some() {
            b"true\n".as_slice()
        } else {
            b"false\n".as_slice()
        };
    if let Some(member) = &actual.member_name
        && expected_member == b"true\n"
    {
        equal &= field(directory, "member-name", member);
    }
    equal &= fs::read(directory.join("positions.tsv")).unwrap()
        == format!(
            "{}\t{}\t{}\t{}\n",
            actual.start_line, actual.start_column, actual.end_line, actual.end_column
        )
        .as_bytes();
    let count = fs::read_to_string(directory.join("source-line-count.bin"))
        .unwrap()
        .strip_suffix('\n')
        .unwrap()
        .parse::<usize>()
        .unwrap();
    equal &= actual.source_lines.len() == count;
    for (index, line) in actual.source_lines.iter().enumerate() {
        if index < count {
            equal &= field(directory, &format!("source-line-{index:04}"), line);
        }
    }
    if !equal {
        eprintln!(
            "{} complete frame mismatch: actual={actual:?}",
            directory.display()
        );
    }
    equal
}

fn compare_row(
    probe: &mut SourceProbe,
    expected: &Path,
    uri: &str,
    source: &str,
    operation: &str,
    expression: &str,
    profile: &str,
) -> Result<bool, String> {
    let mut equal = field(
        expected,
        "expression",
        &expression.encode_utf16().collect::<Vec<_>>(),
    );
    equal &= field(
        expected,
        "operation",
        &operation.encode_utf16().collect::<Vec<_>>(),
    );
    let query = match operation {
        "expression" => expression,
        "output-text" => "output.text",
        _ => return Err("unknown operation".into()),
    };
    let counts = fs::read_to_string(expected.join("counts.tsv"))
        .unwrap()
        .split_whitespace()
        .map(|value| value.parse::<usize>().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(counts.len(), 2);
    probe
        .evaluate_module_source(uri, Some(source), query, 0, None)
        .map_err(|e| format!("evaluate: {e:?}"))?;
    let mut frames = 0;
    let mut logs = 0;
    let mut terminal = None;
    for _ in 0..2_000_000_u32.div_ceil(64) {
        let event = probe.poll(0, 64).map_err(|e| format!("poll: {e:?}"))?;
        probe.collect();
        match event.kind {
            "yield" => continue,
            "module-request" => {
                let token = event.token.as_deref().ok_or("module token")?;
                if !module_keys::complete_probe(probe, token, 0)
                    .map_err(|e| format!("module: {e:?}"))?
                {
                    return Err(format!("unhandled module request: {event:?}"));
                }
            }
            "read" => {
                if event.uri.as_deref() != Some("prop:pkl.outputFormat") {
                    return Err(format!("unhandled read: {event:?}"));
                }
                probe
                    .complete_read_missing(event.token.as_deref().ok_or("read token")?, 0)
                    .map_err(|e| format!("read: {e:?}"))?;
            }
            "transform-frame" => {
                let input = event.frame.ok_or("transform input")?;
                let policy = match profile {
                    "default" => FrameTransformer::Default,
                    "identity" => FrameTransformer::Identity,
                    _ => return Err("profile".into()),
                };
                // Forward an actual frame through the shared policy, never an expected fixture frame.
                let output =
                    SourceFrame::try_from(&policy.transform(LogFrame::from(input.clone())))
                        .map_err(|e| format!("transform: {e:?}"))?;
                if frames < counts[0] {
                    equal &= frame(&expected.join(format!("frame-{frames:04}-input")), &input);
                    equal &= frame(&expected.join(format!("frame-{frames:04}-output")), &output);
                } else {
                    equal = false;
                    eprintln!("{} extra actual frame: {input:?}", expected.display());
                }
                frames += 1;
                probe
                    .complete_frame_transform(
                        event.token.as_deref().ok_or("transform token")?,
                        Some(output),
                        None,
                        0,
                    )
                    .map_err(|e| format!("complete transform: {e:?}"))?;
            }
            "log" => {
                let log = event.log.ok_or("log event")?;
                if logs < counts[1] {
                    let directory = expected.join(format!("logger-{logs:04}"));
                    equal &= field(
                        &directory,
                        "level",
                        &log.level.encode_utf16().collect::<Vec<_>>(),
                    );
                    equal &= field(&directory, "message", &log.message);
                    equal &= frame(
                        &directory.join("frame"),
                        &SourceFrame {
                            module_uri: log.module_uri,
                            member_name: log.member_name,
                            source_lines: log.source_lines,
                            start_line: log.start_line,
                            start_column: log.start_column,
                            end_line: log.end_line,
                            end_column: log.end_column,
                        },
                    );
                } else {
                    equal = false;
                    eprintln!("{} extra Logger event", expected.display());
                }
                logs += 1;
                probe
                    .acknowledge_log(event.token.as_deref().ok_or("log token")?, None, 0)
                    .map_err(|e| format!("complete log: {e:?}"))?;
            }
            "value" | "error" => {
                terminal = Some(event);
                break;
            }
            _ => return Err(format!("unhandled event: {event:?}")),
        }
    }
    let event = terminal.ok_or("bounded query did not terminate")?;
    equal &= frames == counts[0] && logs == counts[1];
    if frames != counts[0] || logs != counts[1] {
        eprintln!(
            "{} counts expected={counts:?} actual=[{frames}, {logs}]",
            expected.display()
        );
    }
    let status = fs::read(expected.join("status.bin")).unwrap();
    let actual_status = match event.kind {
        "value" if event.error.is_none() && event.value.is_some() => b"success\n".as_slice(),
        "error"
            if event.disposition == Some("language-error")
                && event.value.is_none()
                && event.error.is_some() =>
        {
            b"error\n".as_slice()
        }
        _ => b"unsupported-or-api-error\n".as_slice(),
    };
    equal &= status == actual_status;
    if status == b"success\n" {
        equal &= field(expected, "string", event.value.as_deref().unwrap_or(&[]));
    } else {
        assert_eq!(status, b"error\n");
        equal &= field(expected, "error", event.error.as_deref().unwrap_or(&[]));
    }
    if !equal {
        eprintln!("{} required API mismatch: {event:?}", expected.display());
    }
    Ok(equal)
}

fn compare_programs(
    family: &str,
    pair: Option<bool>,
    program_count: usize,
    expected_rows: usize,
) -> bool {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures")
        .join(family);
    let inputs = root.join("inputs");
    assert_eq!(
        fs::read(inputs.join("profiles.tsv")).unwrap(),
        b"default\nidentity\n"
    );
    let programs = fs::read_to_string(inputs.join("programs.tsv")).unwrap();
    assert_eq!(programs.lines().count(), program_count);
    let mut passed = 0;
    let mut mismatches = Vec::new();
    let mut aborted = Vec::new();
    for program in programs
        .lines()
        .filter(|program| pair.is_none_or(|pair| program.starts_with("pair-") == pair))
    {
        let source_dir = inputs.join(program);
        let source = fs::read_to_string(source_dir.join("module.pkl")).unwrap();
        assert_eq!(
            source.encode_utf16().collect::<Vec<_>>(),
            units(&fs::read(source_dir.join("module.code-units.bin")).unwrap())
        );
        let uri = fs::read_to_string(source_dir.join("uri.txt")).unwrap();
        let queries = fs::read_to_string(source_dir.join("queries.tsv")).unwrap();
        assert_eq!(
            queries.lines().count(),
            match program {
                "pair-members" => 22,
                "pair-object-boundary" => 13,
                "any-arity-boundary" => 101,
                _ => 1,
            }
        );
        for profile in ["default", "identity"] {
            let mut probe =
                SourceProbe::new_module_evaluator("compact", true, true, false).unwrap();
            let mut unusable = None;
            for round in 0..2 {
                for query in queries.lines() {
                    let columns = query.split('\t').collect::<Vec<_>>();
                    assert_eq!(columns.len(), 3);
                    let id = format!("{program}/{profile}/round-{round}/{}", columns[0]);
                    let directory = root.join("observations").join(&id);
                    if let Some(reason) = &unusable {
                        aborted.push((
                            id,
                            format!("same evaluator unavailable after prior aborted row: {reason}"),
                        ));
                        continue;
                    }
                    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        compare_row(
                            &mut probe, &directory, &uri, &source, columns[1], columns[2], profile,
                        )
                    }));
                    match result {
                        Ok(Ok(true)) => {
                            passed += 1;
                            eprintln!("required-api-row id={id} exact_match=true");
                        }
                        Ok(Ok(false)) => {
                            eprintln!("required-api-row id={id} exact_match=false");
                            mismatches.push(id);
                        }
                        Ok(Err(error)) => {
                            eprintln!("required-api-row id={id} aborted={error}");
                            aborted.push((id, error.clone()));
                            unusable = Some(error);
                        }
                        Err(error) => {
                            let message = format!("comparison panicked: {error:?}");
                            aborted.push((id, message.clone()));
                            unusable = Some(message);
                        }
                    }
                }
            }
            probe.close();
            assert_eq!(probe.collect().slots, 0);
        }
    }
    eprintln!(
        "required-api-inventory total={expected_rows} passed={passed} mismatched={} aborted={}",
        mismatches.len(),
        aborted.len()
    );
    assert_eq!(passed + mismatches.len() + aborted.len(), expected_rows);
    let equal = mismatches.is_empty() && aborted.is_empty() && passed == expected_rows;
    if !equal {
        eprintln!("required failures: mismatched={mismatches:?}; aborted={aborted:?}");
    }
    equal
}

#[test]
fn actual_java_pair_public_channels_frames_and_same_session_retries() {
    let original = compare_programs("m2-pair-supertype-api", Some(true), 13, 92);
    let lookup = compare_programs("m2-pair-lookup-api", None, 1, 52);
    assert!(
        original && lookup,
        "required original and lookup Pair API inventories"
    );
}
#[test]
fn actual_java_supertype_context_public_channels_frames_and_same_session_retries() {
    assert!(compare_programs(
        "m2-pair-supertype-api",
        Some(false),
        13,
        44
    ));
}

#[test]
fn actual_java_stdlib_parameterized_supertype_channels_frames_and_same_session_retries() {
    assert!(compare_programs(
        "m2-parameterized-supertype-api",
        None,
        8,
        32
    ));
}

#[test]
fn actual_java_any_native_arity_channels_frames_logger_and_same_session_retries() {
    assert!(compare_programs("m2-any-native-arity-api", None, 1, 404));
}
