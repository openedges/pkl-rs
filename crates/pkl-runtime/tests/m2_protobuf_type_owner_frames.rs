//! Protobuf type-operation root controls from two closed Java acquisitions.
//! Eighteen existing and eight supplemental ordinary observations compare full
//! result bytes, UTF-16 units, and ordered callbacks. Java exception classes are
//! retained reference provenance; supplemental Java internal errors are excluded.
use pkl_runtime::{
    modules::{ModuleCompletion, ModuleKey, ModuleKeyKind, ModuleRequestKind, default_completion},
    probe::{SourceLog, SourceProbe},
    value::PklString,
};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

type Channels = BTreeMap<String, Vec<u8>>;
struct Observation {
    channels: Channels,
    events: Vec<Channels>,
    harness_failure: Option<String>,
}
fn bytes(path: &Path, name: &str) -> Vec<u8> {
    fs::read(path.join(name))
        .unwrap_or_else(|error| panic!("{}: {error}", path.join(name).display()))
}
fn text(path: &Path, name: &str) -> String {
    String::from_utf8(bytes(path, name)).expect("authored fixture UTF-8")
}
fn authored_line(path: &Path, name: &str) -> String {
    let value = text(path, name);
    let line = value.strip_suffix('\n').expect("one authored terminal LF");
    assert!(!line.contains(['\n', '\r']));
    line.to_owned()
}
fn registry(path: &Path, name: &str, width: usize) -> Vec<Vec<String>> {
    let source = text(path, name);
    if source.is_empty() {
        return Vec::new();
    }
    let mut keys = std::collections::BTreeSet::new();
    source
        .strip_suffix('\n')
        .expect("registry terminal LF")
        .split('\n')
        .map(|line| {
            let row = line.split('\t').map(str::to_owned).collect::<Vec<_>>();
            assert_eq!(row.len(), width);
            assert!(keys.insert(row[0].clone()), "duplicate fixture key");
            let file = Path::new(row.last().unwrap());
            assert_eq!(file.components().count(), 1, "finite fixture filename");
            assert!(file.file_name().is_some());
            row
        })
        .collect()
}
fn field(channels: &mut Channels, name: &str, units: &[u16]) {
    channels.insert(
        format!("{name}.bin"),
        PklString::from_utf16(units.to_vec())
            .to_java_utf8()
            .as_bytes()
            .to_vec(),
    );
    channels.insert(
        format!("{name}.code-units.bin"),
        units.iter().flat_map(|unit| unit.to_be_bytes()).collect(),
    );
}
fn scalar(channels: &mut Channels, name: &str, value: &str) {
    field(channels, name, &value.encode_utf16().collect::<Vec<_>>());
}
fn callback(kind: &str, message: &[u16], frame: Option<&SourceLog>) -> Channels {
    let mut result = Channels::new();
    scalar(&mut result, "kind", kind);
    field(&mut result, "message", message);
    result.insert(
        "frame.present.bin".into(),
        if frame.is_some() {
            b"1\n".to_vec()
        } else {
            b"0\n".to_vec()
        },
    );
    if let Some(log) = frame {
        field(&mut result, "module-uri", &log.module_uri);
        result.insert(
            "member.present.bin".into(),
            if log.member_name.is_some() {
                b"1\n".to_vec()
            } else {
                b"0\n".to_vec()
            },
        );
        field(
            &mut result,
            "member",
            log.member_name.as_deref().unwrap_or(&[]),
        );
        for (name, value) in [
            ("start-line", log.start_line),
            ("start-column", log.start_column),
            ("end-line", log.end_line),
            ("end-column", log.end_column),
        ] {
            scalar(&mut result, name, &value.to_string());
        }
        result.insert(
            "lines.count.bin".into(),
            format!("{}\n", log.source_lines.len()).into_bytes(),
        );
        for (index, line) in log.source_lines.iter().enumerate() {
            field(&mut result, &format!("line-{index:08}"), line);
        }
    }
    result
}
fn native(input: &Path, power: bool) -> Observation {
    let source = text(input, "module.pkl");
    let uri = authored_line(input, "uri.txt");
    let expression = authored_line(input, "expression.txt");
    let modules = registry(input, "modules.tsv", 2);
    let resources = registry(input, "resources.tsv", 3);
    let properties = registry(input, "properties.tsv", 2);
    let mut result = Observation {
        channels: Channels::new(),
        events: Vec::new(),
        harness_failure: None,
    };
    let mut probe = match SourceProbe::new_module_evaluator("compact", true, false, power) {
        Ok(probe) => probe,
        Err(error) => {
            result.harness_failure = Some(format!("probe initialization: {error}"));
            return result;
        }
    };
    let operation = (|| -> Result<(), String> {
        probe
            .evaluate_module_source(
                uri.as_str(),
                Some(source.as_str()),
                expression.as_str(),
                0,
                None,
            )
            .map_err(|error| format!("start: {error:?}"))?;
        for _ in 0..200_000 {
            let event = probe
                .poll(0, 10_000)
                .map_err(|error| format!("poll: {error:?}"))?;
            if event.kind == "yield" {
                continue;
            }
            if event.kind == "read" {
                let token = event.token.as_deref().ok_or("read token absent")?;
                let resource = event.uri.as_deref().ok_or("read URI absent")?;
                if let Some(key) = resource.strip_prefix("prop:") {
                    if let Some(row) = properties.iter().find(|row| row[0] == key) {
                        probe
                            .complete_read_text(token, text(input, &row[1]).into(), 0)
                            .map_err(|error| format!("property completion: {error:?}"))?;
                    } else {
                        probe
                            .complete_read_missing(token, 0)
                            .map_err(|error| format!("missing property: {error:?}"))?;
                    }
                } else if resource.starts_with("proto:") {
                    result.events.push(callback(
                        "resource",
                        &resource.encode_utf16().collect::<Vec<_>>(),
                        None,
                    ));
                    if let Some(row) = resources.iter().find(|row| row[0] == resource) {
                        let payload = text(input, &row[2]);
                        match row[1].as_str() {
                            "text" => probe.complete_read_text(token, payload.into(), 0),
                            "io" => probe.reject_read_io(token, Some(payload.into()), 0),
                            "utf16" => {
                                let units = payload
                                    .split(' ')
                                    .map(|unit| {
                                        if unit.len() != 4
                                            || !unit.bytes().all(|byte| {
                                                byte.is_ascii_digit()
                                                    || (b'a'..=b'f').contains(&byte)
                                            })
                                        {
                                            return Err("invalid authored UTF-16 unit".to_owned());
                                        }
                                        u16::from_str_radix(unit, 16)
                                            .map_err(|error| error.to_string())
                                    })
                                    .collect::<Result<Vec<_>, _>>()?;
                                probe.complete_read_text(token, PklString::from_utf16(units), 0)
                            }
                            other => return Err(format!("unmodeled resource kind {other}")),
                        }
                        .map_err(|error| format!("resource completion: {error:?}"))?;
                    } else {
                        probe
                            .complete_read_missing(token, 0)
                            .map_err(|error| format!("missing resource: {error:?}"))?;
                    }
                } else if resource.starts_with("env:") {
                    probe
                        .complete_read_missing(token, 0)
                        .map_err(|error| format!("empty environment: {error:?}"))?;
                } else {
                    return Err(format!("unmodeled resource URI {resource}"));
                }
                continue;
            }
            if event.kind == "module-request" {
                let token = event.token.as_deref().ok_or("module token absent")?;
                let request = probe
                    .module_request(token)
                    .map_err(|error| format!("module request: {error:?}"))?;
                // Java's authored factory logs each matching selection, even if
                // a later shared module cache suppresses source loading.
                if let ModuleRequestKind::SelectKey { id, uri, .. } = &request.kind
                    && modules.iter().any(|row| uri == row[0].as_str())
                {
                    result.events.push(callback(
                        "module",
                        &uri.encode_utf16().collect::<Vec<_>>(),
                        None,
                    ));
                    probe
                        .complete_module(
                            token,
                            ModuleCompletion::KeySelected(Ok(ModuleKey {
                                id: *id,
                                uri: uri.clone(),
                                kind: ModuleKeyKind::Synthetic,
                                handle: "java04-fixture".into(),
                            })),
                            0,
                        )
                        .map_err(|error| format!("fixture key: {error:?}"))?;
                    continue;
                }
                if let Some(completion) = default_completion(&request) {
                    probe
                        .complete_module(token, completion, 0)
                        .map_err(|error| format!("default module completion: {error:?}"))?;
                    continue;
                }
                match &request.kind {
                    ModuleRequestKind::SelectKey {
                        id, uri: requested, ..
                    } if requested == uri.as_str() => {
                        // The evaluator has the exact primary source supplied.
                        probe.complete_module(
                            token,
                            ModuleCompletion::KeySelected(Ok(ModuleKey {
                                id: *id,
                                uri: requested.clone(),
                                kind: ModuleKeyKind::Host,
                                handle: "supplied-primary".into(),
                            })),
                            0,
                        )
                    }
                    ModuleRequestKind::QueryCachePolicy { .. } => {
                        probe.complete_module(token, ModuleCompletion::CachePolicy(Ok(true)), 0)
                    }
                    ModuleRequestKind::Resolve { original_uri, .. }
                        if modules.iter().any(|row| original_uri == row[0].as_str()) =>
                    {
                        probe.complete_module_resolution(
                            token,
                            original_uri.clone(),
                            original_uri.clone(),
                            original_uri.clone(),
                            0,
                        )
                    }
                    ModuleRequestKind::LoadSource { resolved, .. } => {
                        let row = modules
                            .iter()
                            .find(|row| resolved.original_uri == row[0].as_str())
                            .ok_or_else(|| {
                                format!("unmodeled source load {:?}", resolved.original_uri)
                            })?;
                        let name = row[0]
                            .rsplit('/')
                            .next()
                            .and_then(|name| name.strip_suffix(".pkl"))
                            .ok_or("fixture module name absent")?;
                        probe.complete_module_source(token, text(input, &row[1]).as_str(), name, 0)
                    }
                    other => return Err(format!("unmodeled module boundary {other:?}")),
                }
                .map_err(|error| format!("module completion: {error:?}"))?;
                continue;
            }
            if event.kind == "log" {
                let log = event.log.as_ref().ok_or("log record absent")?;
                result
                    .events
                    .push(callback(log.level, &log.message, Some(log)));
                probe
                    .acknowledge_log(event.token.as_deref().ok_or("log token absent")?, None, 0)
                    .map_err(|error| format!("log acknowledgment: {error:?}"))?;
                continue;
            }
            let status = match event.kind {
                "value" => "success\n",
                "error" if event.disposition == Some("language-error") => "error\n",
                "error" => "unsupported-or-api-error\n",
                other => return Err(format!("unmodeled event {other}: {event:?}")),
            };
            result
                .channels
                .insert("result.status.bin".into(), status.as_bytes().to_vec());
            field(
                &mut result.channels,
                "result.stdout",
                event.value.as_deref().unwrap_or(&[]),
            );
            field(
                &mut result.channels,
                "result.stderr",
                event.error.as_deref().unwrap_or(&[]),
            );
            scalar(&mut result.channels, "expression", &expression);
            return Ok(());
        }
        Err("poll budget exhausted before terminal result".into())
    })();
    if let Err(error) = operation {
        result.harness_failure = Some(error);
    }
    probe.close();
    let retained = probe.collect().slots;
    if retained != 0 {
        result.harness_failure = Some(format!(
            "{}; {retained} retained slots after close",
            result
                .harness_failure
                .as_deref()
                .unwrap_or("terminal observed")
        ));
    }
    result.channels.insert(
        "events.count.bin".into(),
        format!("{}\n", result.events.len()).into_bytes(),
    );
    result
}
fn check_type_owner(id: &str, power: bool) {
    let input = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/m2-protobuf-java04-full")
        .join(id);
    check_type_owner_at(id, input, power);
}
fn check_type_owner_at(id: &str, input: PathBuf, power: bool) {
    let expected = input.join(if power { "on" } else { "off" });
    let actual = native(&input, power);
    assert!(
        actual.harness_failure.is_none(),
        "{id}: {:?}",
        actual.harness_failure
    );
    let fields = [
        "result.status.bin",
        "result.stdout.bin",
        "result.stdout.code-units.bin",
        "result.stderr.bin",
        "result.stderr.code-units.bin",
        "expression.bin",
        "expression.code-units.bin",
        "events.count.bin",
    ];
    assert_eq!(
        actual.channels.len(),
        fields.len(),
        "{id}: complete native channels"
    );
    for field in fields {
        assert_eq!(
            actual.channels.get(field).expect("required native channel"),
            &bytes(&expected, field),
            "{id}: {field}: full raw bytes"
        );
    }
    assert_eq!(
        actual.events.len(),
        text(&expected, "events.count.bin")
            .strip_suffix('\n')
            .expect("event count LF")
            .parse::<usize>()
            .unwrap()
    );
    for (index, event) in actual.events.iter().enumerate() {
        let directory = expected.join("events").join(format!("{index:08}"));
        let expected_fields = fs::read_dir(&directory)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(
            event
                .keys()
                .cloned()
                .collect::<std::collections::BTreeSet<_>>(),
            expected_fields,
            "{id}: event{index}: exact field membership"
        );
        for (field, value) in event {
            assert_eq!(
                value,
                &bytes(&directory, field),
                "{id}: event{index}/{field}: full raw bytes"
            );
        }
    }
    // Java classes remain reference provenance, never an invented Rust channel.
    for field in [
        "result.error-class.bin",
        "result.error-class.code-units.bin",
    ] {
        let _ = bytes(&expected, field);
    }
}
macro_rules! owner_control {
    ($name:ident, $case:literal, $power:literal) => {
        #[test]
        fn $name() {
            check_type_owner($case, $power);
        }
    };
}
owner_control!(mapping_type_float_on, "mapping-type-float", true);
owner_control!(mapping_type_float_off, "mapping-type-float", false);
owner_control!(mapping_type_union_on, "mapping-type-union", true);
owner_control!(mapping_type_union_off, "mapping-type-union", false);
owner_control!(mapping_type_class_on, "mapping-type-class", true);
owner_control!(mapping_type_class_off, "mapping-type-class", false);
owner_control!(imported_key_diagnostic_on, "imported-key-diagnostic", true);
owner_control!(
    imported_key_diagnostic_off,
    "imported-key-diagnostic",
    false
);
owner_control!(constraint_throw_on, "constraint-throw", true);
owner_control!(constraint_throw_off, "constraint-throw", false);
owner_control!(constraint_read_io_on, "constraint-read-io", true);
owner_control!(constraint_read_io_off, "constraint-read-io", false);
owner_control!(mapping_type_int32_on, "mapping-type-int32", true);
owner_control!(mapping_type_int32_off, "mapping-type-int32", false);
owner_control!(mapping_type_boolean_on, "mapping-type-boolean", true);
owner_control!(mapping_type_boolean_off, "mapping-type-boolean", false);
owner_control!(mapping_type_alias_on, "mapping-type-alias", true);
owner_control!(mapping_type_alias_off, "mapping-type-alias", false);

macro_rules! supplemental_control {
    ($name:ident, $case:literal, $power:literal) => {
        #[test]
        fn $name() {
            let input = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../tests/fixtures/m2-protobuf-type-owner-java")
                .join($case);
            check_type_owner_at($case, input, $power);
        }
    };
}
supplemental_control!(
    mapping_float_render_value_on,
    "mapping-float-render-value",
    true
);
supplemental_control!(
    mapping_float_render_value_off,
    "mapping-float-render-value",
    false
);
supplemental_control!(
    constraint_throw_render_value_on,
    "constraint-throw-render-value",
    true
);
supplemental_control!(
    constraint_throw_render_value_off,
    "constraint-throw-render-value",
    false
);
supplemental_control!(
    ordinary_constraint_throw_on,
    "ordinary-constraint-throw",
    true
);
supplemental_control!(
    ordinary_constraint_throw_off,
    "ordinary-constraint-throw",
    false
);
supplemental_control!(
    protobuf_constraint_inline_lambda_on,
    "protobuf-constraint-inline-lambda",
    true
);
supplemental_control!(
    protobuf_constraint_inline_lambda_off,
    "protobuf-constraint-inline-lambda",
    false
);
