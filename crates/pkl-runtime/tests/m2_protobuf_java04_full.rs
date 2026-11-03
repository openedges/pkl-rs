//! Java04 full-slot differential contracts for native evaluation.
//! Full result bytes, UTF-16 units, and ordered callback fields are compared.
//! All 18 internal/process reference slots are attempted as explicit nonpasses.
//! Those slots are not ignored; the separate strict test is ignored by default.
//! Default workspace checks the completed-reference development contract.
//! The named ignored strict gate remains an unmet full-completion requirement.
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
// Exact projected membership follows the independent Java04 full-slot reader.
// Java exception-class channels remain reference provenance, not native fields.
const RESULT_FIELDS: [&str; 8] = [
    "result.status.bin",
    "result.stdout.bin",
    "result.stdout.code-units.bin",
    "result.stderr.bin",
    "result.stderr.code-units.bin",
    "expression.bin",
    "expression.code-units.bin",
    "events.count.bin",
];
fn count(raw: Option<&Vec<u8>>) -> Option<usize> {
    let digits = raw?.strip_suffix(b"\n")?;
    if digits.is_empty()
        || (digits.len() > 1 && digits[0] == b'0')
        || !digits.iter().all(u8::is_ascii_digit)
    {
        return None;
    }
    let value = std::str::from_utf8(digits).ok()?.parse::<usize>().ok()?;
    (value <= 10_000).then_some(value)
}
fn projection_shape(actual: &Observation) -> Result<(), &'static str> {
    let names = actual
        .channels
        .keys()
        .map(String::as_str)
        .collect::<std::collections::BTreeSet<_>>();
    if names
        != RESULT_FIELDS
            .into_iter()
            .collect::<std::collections::BTreeSet<_>>()
    {
        return Err("native result channel membership differs");
    }
    if !matches!(
        actual.channels["result.status.bin"].as_slice(),
        b"success\n" | b"error\n" | b"unsupported-or-api-error\n"
    ) {
        return Err("missing/invalid native terminal");
    }
    if count(actual.channels.get("events.count.bin")) != Some(actual.events.len()) {
        return Err("native event count differs from ordered events");
    }
    for event in &actual.events {
        let mut required = [
            "kind.bin",
            "kind.code-units.bin",
            "message.bin",
            "message.code-units.bin",
            "frame.present.bin",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect::<std::collections::BTreeSet<_>>();
        match event.get("frame.present.bin").map(Vec::as_slice) {
            Some(b"0\n") => {}
            Some(b"1\n") => {
                if !matches!(
                    event.get("member.present.bin").map(Vec::as_slice),
                    Some(b"0\n" | b"1\n")
                ) {
                    return Err("native event member presence is invalid");
                }
                required.extend(["member.present.bin", "lines.count.bin"].map(str::to_owned));
                for field in [
                    "module-uri",
                    "member",
                    "start-line",
                    "start-column",
                    "end-line",
                    "end-column",
                ] {
                    required.extend([format!("{field}.bin"), format!("{field}.code-units.bin")]);
                }
                let lines = count(event.get("lines.count.bin"))
                    .ok_or("native event source line count is invalid")?;
                for index in 0..lines {
                    required.extend([
                        format!("line-{index:08}.bin"),
                        format!("line-{index:08}.code-units.bin"),
                    ]);
                }
            }
            _ => return Err("native event frame presence is invalid"),
        }
        if event
            .keys()
            .cloned()
            .collect::<std::collections::BTreeSet<_>>()
            != required
        {
            return Err("native event channel membership differs");
        }
    }
    Ok(())
}
fn record_projection_failure(actual: &mut Observation) {
    if let Err(error) = projection_shape(actual) {
        let prior = actual.harness_failure.take();
        actual.harness_failure = Some(match prior {
            Some(prior) => format!("{prior}; native projection structure: {error}"),
            None => format!("native projection structure: {error}"),
        });
    }
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
fn compare(expected: &Path, name: &str, actual: &[u8], row: &str, failures: &mut Vec<String>) {
    let reference = match fs::read(expected.join(name)) {
        Ok(reference) => reference,
        Err(error) => {
            failures.push(format!("{row}: absent expected field {name}: {error}"));
            return;
        }
    };
    if reference != actual {
        failures.push(format!(
            "{row}: {name}: full bytes differ (Java {}, Rust {})",
            reference.len(),
            actual.len()
        ));
    }
}
fn save_capture(root: &Path, case: &str, mode: &str, actual: &Observation) {
    let folder = root.join(case).join(mode);
    fs::create_dir_all(&folder).unwrap();
    for (name, value) in &actual.channels {
        fs::write(folder.join(name), value).unwrap();
    }
    for (index, event) in actual.events.iter().enumerate() {
        let event_folder = folder.join("events").join(format!("{index:08}"));
        fs::create_dir_all(&event_folder).unwrap();
        for (name, value) in event {
            fs::write(event_folder.join(name), value).unwrap();
        }
    }
    if let Some(failure) = &actual.harness_failure {
        fs::write(folder.join("harness-failure.txt"), failure).unwrap();
    }
}

#[derive(Clone, Copy)]
enum Gate {
    CompletedReference,
    Strict,
}
impl Gate {
    fn name(self) -> &'static str {
        match self {
            Self::CompletedReference => "completed-reference-experiment",
            Self::Strict => "strict",
        }
    }
}
#[derive(Clone, Copy)]
struct Totals {
    attempted: usize,
    terminals: usize,
    harness_failures: usize,
    renderer_matches: usize,
    pre_renderer_matches: usize,
    reference_nonpasses: usize,
}
impl Totals {
    fn completed_reference(self) -> bool {
        self.attempted == 266
            && self.terminals == 266
            && self.harness_failures == 0
            && self.renderer_matches == 240
            && self.pre_renderer_matches == 8
            && self.reference_nonpasses == 18
    }
    fn strict(self, raw_failures_empty: bool) -> bool {
        raw_failures_empty
            && self.attempted == 266
            && self.terminals == 266
            && self.harness_failures == 0
            && self.reference_nonpasses == 0
            && self.renderer_matches + self.pre_renderer_matches == 266
    }
}
fn compare_java04(gate: Gate) {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/m2-protobuf-java04-full");
    let roster = text(&root, "observations.tsv");
    assert_eq!(roster.lines().count(), 266);
    assert_eq!(text(&root, "cases.txt").lines().count(), 133);
    assert!(
        std::env::var_os("PKL_PROTOBUF_JAVA04_MODE").is_none_or(|value| value.is_empty()),
        "PKL_PROTOBUF_JAVA04_MODE no longer selects assertions; invoke a named gate"
    );
    let mode = gate.name();
    let capture = std::env::var_os("PKL_PROTOBUF_CAPTURE_DIR").map(PathBuf::from);
    assert!(
        !matches!(gate, Gate::Strict) || capture.is_some(),
        "the strict completion gate requires a fresh full capture directory"
    );
    if let Some(path) = &capture {
        // A future admitted runner selects a fresh target output; never overwrite.
        let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap();
        assert!(path.is_absolute() && path.starts_with(workspace.join("target/m2")));
        assert!(!path.components().any(|part| {
            let name = part.as_os_str().to_string_lossy();
            name == ".."
                || name == "stage65-b-native-regressions-evidence.tar.gz.partial"
                || name.to_ascii_lowercase().contains("stage86")
                || name.to_ascii_lowercase().contains("private8")
        }));
        assert!(!path.exists());
        fs::create_dir(path).unwrap();
    }
    let mut failures = Vec::new();
    let mut outcomes = String::new();
    let mut regular = 0usize;
    let mut pre_renderer = 0usize;
    let mut nonpasses = 0usize;
    let mut renderer_matches = 0usize;
    let mut pre_renderer_matches = 0usize;
    let mut native_terminal_count = 0usize;
    let mut harness_failure_count = 0usize;
    let mut seen = std::collections::BTreeSet::new();
    for line in roster.lines() {
        let row = line.split('\t').collect::<Vec<_>>();
        assert_eq!(row.len(), 5);
        let (case, mode, category, policy) = (row[0], row[2], row[3], row[4]);
        assert!(matches!(mode, "on" | "off") && seen.insert((case, mode)));
        let input = root.join(case);
        let expected = input.join(mode);
        assert_eq!(
            authored_line(&input, "uri.txt"),
            format!("file:///stage145-protobuf/{case}/main.pkl")
        );
        let mut actual = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            native(&input, mode == "on")
        }))
        .unwrap_or_else(|payload| {
            let message = payload
                .downcast_ref::<String>()
                .map(String::as_str)
                .or_else(|| payload.downcast_ref::<&str>().copied())
                .unwrap_or("non-string panic payload");
            Observation {
                channels: Channels::new(),
                events: Vec::new(),
                harness_failure: Some(format!(
                    "native panic; partial callback record unavailable: {message}"
                )),
            }
        });
        record_projection_failure(&mut actual);
        if let Some(folder) = &capture {
            save_capture(folder, case, mode, &actual);
        }
        let label = format!("{case}/{mode}");
        let before = failures.len();
        let native_terminal = matches!(
            actual.channels.get("result.status.bin").map(Vec::as_slice),
            Some(b"success\n" | b"error\n" | b"unsupported-or-api-error\n")
        );
        native_terminal_count += usize::from(native_terminal);
        harness_failure_count += usize::from(actual.harness_failure.is_some());
        if let Some(error) = &actual.harness_failure {
            failures.push(format!("{label}: HARNESS-NONPASS: {error}"));
        }
        match policy {
            "compare-regular" => regular += 1,
            "compare-pre-renderer" => pre_renderer += 1,
            "internal-reference-nonpass" | "no-complete-reference-nonpass" => {
                nonpasses += 1;
                failures.push(format!("{label}: REFERENCE-NONPASS: {category}; no regular renderer compatibility credit"));
            }
            other => panic!("unknown ledger policy {other}"),
        }
        if policy == "no-complete-reference-nonpass" {
            // Raw process channels are retained; absent result/effect channels
            // are not replaced with empty successful reference observations.
            let _ = bytes(&expected, "process.stdout.bin");
            let _ = bytes(&expected, "process.stderr.bin");
        } else {
            if !actual.channels.contains_key("result.status.bin") {
                failures.push(format!("{label}: no native terminal result"));
            }
            for (name, value) in &actual.channels {
                compare(&expected, name, value, &label, &mut failures);
            }
            // No Java exception-class string is fabricated for Rust. Both raw
            // class channels remain provenance, outside the result projection.
            let _ = bytes(&expected, "result.error-class.bin");
            let _ = bytes(&expected, "result.error-class.code-units.bin");
            let count = authored_line(&expected, "events.count.bin")
                .parse::<usize>()
                .unwrap();
            for (index, event) in actual.events.iter().enumerate() {
                if index >= count {
                    failures.push(format!("{label}: unexpected ordered event {index}"));
                    continue;
                }
                let expected_event = expected.join("events").join(format!("{index:08}"));
                for (name, value) in event {
                    compare(
                        &expected_event,
                        name,
                        value,
                        &format!("{label}/event-{index}"),
                        &mut failures,
                    );
                }
            }
        }
        let matched = failures.len() == before;
        let coverage = policy == "compare-regular" && matched;
        let pre_renderer_match = policy == "compare-pre-renderer" && matched;
        renderer_matches += usize::from(coverage);
        pre_renderer_matches += usize::from(pre_renderer_match);
        outcomes.push_str(&format!("{case}\t{mode}\t{category}\texact_projected_match={matched}\trenderer_coverage_pass={coverage}\tpre_renderer_reference_match={pre_renderer_match}\tnative_terminal={native_terminal}\tharness_failure={}\n", actual.harness_failure.is_some()));
        eprintln!(
            "protobuf-java04 {label} {category} exact_projected_match={matched} renderer_coverage_pass={coverage}"
        );
    }
    assert_eq!(
        (regular, pre_renderer, nonpasses, seen.len()),
        (240, 8, 18, 266)
    );
    let totals = Totals {
        attempted: seen.len(),
        terminals: native_terminal_count,
        harness_failures: harness_failure_count,
        renderer_matches,
        pre_renderer_matches,
        reference_nonpasses: nonpasses,
    };
    let strict_compatible = totals.strict(failures.is_empty());
    let experiment_contract_pass = totals.completed_reference();
    let summary = format!(
        "{{\n  \"mode\": \"{mode}\",\n  \"observations_attempted\": {},\n  \"native_terminal_count\": {native_terminal_count},\n  \"harness_failure_count\": {harness_failure_count},\n  \"regular_renderer_slots\": {regular},\n  \"regular_renderer_matches\": {renderer_matches},\n  \"pre_renderer_slots\": {pre_renderer},\n  \"pre_renderer_reference_matches\": {pre_renderer_matches},\n  \"reference_nonpasses\": {nonpasses},\n  \"strict_compatible\": {strict_compatible},\n  \"experiment_contract_pass\": {experiment_contract_pass},\n  \"java_exception_class_projected\": false,\n  \"full_compatibility_qualified\": false\n}}\n",
        seen.len(),
    );
    if let Some(folder) = &capture {
        fs::write(folder.join("native-outcomes.tsv"), outcomes).unwrap();
        fs::write(folder.join("strict-failures.txt"), failures.join("\n")).unwrap();
        fs::write(folder.join("contract.json"), &summary).unwrap();
    }
    eprintln!("protobuf-java04-contract {summary}");
    let accepted = match gate {
        Gate::CompletedReference => experiment_contract_pass,
        Gate::Strict => strict_compatible,
    };
    assert!(
        accepted,
        "{} gate failed; strict_compatible={strict_compatible}; {}",
        gate.name(),
        failures.join("\n")
    );
}

#[test]
fn java04_completed_reference_contract() {
    compare_java04(Gate::CompletedReference);
}

#[test]
#[ignore = "explicit strict completion gate; 18 reference nonpasses remain unmet"]
fn java04_strict_full_compatibility_gate() {
    compare_java04(Gate::Strict);
}

#[test]
fn reference_nonpasses_never_become_strict_completion() {
    let complete = Totals {
        attempted: 266,
        terminals: 266,
        harness_failures: 0,
        renderer_matches: 240,
        pre_renderer_matches: 8,
        reference_nonpasses: 18,
    };
    assert!(complete.completed_reference());
    // Even a mistaken empty failure vector cannot erase explicit nonpasses.
    assert!(!complete.strict(true));
}

#[test]
fn historical_twelve_mismatches_and_incomplete_acquisition_fail_contract() {
    // These counters describe the retained native02 outcome, not a new VM result.
    let historical = Totals {
        attempted: 266,
        terminals: 266,
        harness_failures: 0,
        renderer_matches: 228,
        pre_renderer_matches: 8,
        reference_nonpasses: 18,
    };
    assert!(!historical.completed_reference());
    assert!(!historical.strict(false));
    let otherwise_complete = Totals {
        renderer_matches: 240,
        ..historical
    };
    let missing_terminal = Totals {
        terminals: 265,
        ..otherwise_complete
    };
    let harness_failure = Totals {
        harness_failures: 1,
        ..otherwise_complete
    };
    assert!(!missing_terminal.completed_reference());
    assert!(!harness_failure.completed_reference());
}

#[test]
fn same_length_raw_diagnostic_mutation_is_rejected() {
    let expected = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/m2-protobuf-java04-full/mapping-type-float/off");
    let name = "result.stderr.code-units.bin";
    let mut mutant = bytes(&expected, name);
    assert!(!mutant.is_empty());
    let mut failures = Vec::new();
    compare(
        &expected,
        name,
        &mutant,
        "gate-negative-control",
        &mut failures,
    );
    assert!(failures.is_empty());
    mutant[0] ^= 1;
    compare(
        &expected,
        name,
        &mutant,
        "gate-negative-control",
        &mut failures,
    );
    assert_eq!(failures.len(), 1, "same length is not raw equality");
}

#[test]
fn missing_result_or_callback_channel_is_harness_failure() {
    // Use unchanged Java result bytes and both real callback shapes, including
    // a source frame. Java error-class fields are deliberately not projected.
    let expected = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/m2-protobuf-java04-full/constraint-read-io/off");
    let channels = RESULT_FIELDS
        .into_iter()
        .map(|name| (name.to_owned(), bytes(&expected, name)))
        .collect();
    let mut events = Vec::new();
    for index in 0..2 {
        let directory = expected.join("events").join(format!("{index:08}"));
        events.push(
            fs::read_dir(&directory)
                .unwrap()
                .map(|entry| {
                    let name = entry.unwrap().file_name().into_string().unwrap();
                    let value = bytes(&directory, &name);
                    (name, value)
                })
                .collect(),
        );
    }
    let mut actual = Observation {
        channels,
        events,
        harness_failure: None,
    };
    assert_eq!(projection_shape(&actual), Ok(()));
    let original_result = actual.channels.remove("result.stderr.bin").unwrap();
    record_projection_failure(&mut actual);
    assert!(
        actual.harness_failure.is_some(),
        "omission cannot become a raw match"
    );
    actual
        .channels
        .insert("result.stderr.bin".into(), original_result);
    actual.harness_failure = None;
    actual.events[0].remove("message.code-units.bin").unwrap();
    record_projection_failure(&mut actual);
    assert!(
        actual.harness_failure.is_some(),
        "callback omission is a harness failure"
    );
    let rejected = Totals {
        attempted: 266,
        terminals: 266,
        harness_failures: usize::from(actual.harness_failure.is_some()),
        renderer_matches: 240,
        pre_renderer_matches: 8,
        reference_nonpasses: 18,
    };
    assert!(!rejected.completed_reference());
}
