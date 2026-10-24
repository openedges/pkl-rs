//! Shared string-only source sessions; exported Java values are a separate boundary.
//! Usage: OUTPUT INPUT URI. The output directory must already exist and be empty.
use pkl_runtime::{
    modules::{ModuleKeyKind, ModuleRequestKind, SelectionPhase},
    probe::SourceProbe,
    value::PklString,
};
use std::{
    collections::HashMap,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

type CaptureResult<T> = Result<T, String>;

fn write(out: &Path, name: &str, value: impl AsRef<[u8]>) -> CaptureResult<()> {
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(out.join(name))
        .and_then(|mut file| file.write_all(value.as_ref()))
        .map_err(|error| format!("write {name}: {error}"))
}

fn units(out: &Path, name: &str, value: &[u16]) -> CaptureResult<()> {
    write(
        out,
        &format!("{name}.code-units.bin"),
        value
            .iter()
            .flat_map(|unit| unit.to_be_bytes())
            .collect::<Vec<_>>(),
    )?;
    write(
        out,
        &format!("{name}.bin"),
        PklString::from_utf16(value.to_vec()).to_java_utf8(),
    )
}

struct Capture {
    uri: String,
    source: String,
    support: HashMap<String, String>,
    keys: HashMap<u64, (String, String)>,
    resolved_keys: HashMap<u64, (u64, String, String)>,
    trace: fs::File,
    polls: usize,
    collections: usize,
    successes: usize,
    errors: usize,
}

impl Capture {
    fn owns_key(&self, key: &pkl_runtime::modules::ModuleKey) -> bool {
        key.kind == ModuleKeyKind::Host
            && self.keys.get(&key.id.0).is_some_and(|(uri, handle)| {
                uri == key.uri.as_ref() && handle == key.handle.as_ref()
            })
    }

    fn module(&mut self, probe: &mut SourceProbe, token: &str) -> CaptureResult<()> {
        let request = probe
            .module_request(token)
            .map_err(|e| format!("module request: {e:?}"))?;
        self.observe("host-module-request", &request)?;
        let key = match &request.kind {
            ModuleRequestKind::QueryCachePolicy { key }
            | ModuleRequestKind::QueryFragmentPaths { key }
            | ModuleRequestKind::ResolveImportUri { key, .. }
            | ModuleRequestKind::Resolve { key, .. }
            | ModuleRequestKind::ResolveKey { key, .. } => Some(key),
            ModuleRequestKind::Capabilities { key, .. }
            | ModuleRequestKind::HasElement { key, .. } => key.as_ref(),
            ModuleRequestKind::LoadSource { key, .. } => Some(&key.key),
            ModuleRequestKind::SelectKey { .. } | ModuleRequestKind::CheckImport { .. } => None,
        };
        if key.is_some_and(|key| key.kind == ModuleKeyKind::Host && !self.owns_key(key)) {
            return Err("unowned Host ModuleKey descriptor".into());
        }
        let completed = match &request.kind {
            ModuleRequestKind::SelectKey { id, uri, phase }
                if self.support.contains_key(uri.as_ref())
                    || (*phase == SelectionPhase::Primary
                        && uri.as_ref() == self.uri
                        && uri.starts_with("file:")) =>
            {
                let handle = format!("class-values-key:{}", id.0);
                if self
                    .keys
                    .insert(id.0, (uri.to_string(), handle.clone()))
                    .is_some()
                {
                    return Err("reused ModuleKey identity".into());
                }
                probe.complete_module_key(token, &id.0.to_string(), uri, "host", &handle, 0)
            }
            ModuleRequestKind::QueryCachePolicy { key } if self.owns_key(key) => {
                probe.complete_module_cache_policy(token, &key.id.0.to_string(), true, 0)
            }
            ModuleRequestKind::QueryFragmentPaths { key } if self.owns_key(key) => {
                probe.complete_module_fragment_paths(token, &key.id.0.to_string(), false, 0)
            }
            ModuleRequestKind::Capabilities { key: Some(key), .. } if self.owns_key(key) => probe
                .complete_module_capabilities(
                    token,
                    !self.support.contains_key(key.uri.as_ref()),
                    true,
                    false,
                    0,
                ),
            ModuleRequestKind::Resolve {
                key,
                resolved_key_id,
                ..
            }
            | ModuleRequestKind::ResolveKey {
                key,
                resolved_key_id,
                ..
            } if self.owns_key(key) && self.support.contains_key(key.uri.as_ref()) => {
                let handle = format!("class-values-resolved:{}", resolved_key_id.0);
                if self
                    .resolved_keys
                    .insert(
                        resolved_key_id.0,
                        (key.id.0, key.uri.to_string(), handle.clone()),
                    )
                    .is_some()
                {
                    return Err("reused ResolvedModuleKey identity".into());
                }
                probe.complete_module_resolved_key(
                    token,
                    &key.id.0.to_string(),
                    &resolved_key_id.0.to_string(),
                    &handle,
                    &key.uri,
                    &key.uri,
                    &key.uri,
                    0,
                )
            }
            ModuleRequestKind::LoadSource { key, resolved }
                if self.owns_key(&key.key) && self.support.contains_key(key.key.uri.as_ref()) =>
            {
                let expected = (
                    key.key.id.0,
                    key.key.uri.to_string(),
                    key.handle.to_string(),
                );
                if self.resolved_keys.get(&key.id.0) != Some(&expected)
                    || resolved != &key.module
                    || resolved.original_uri != key.key.uri
                    || resolved.resolved_uri != key.key.uri
                    || resolved.source_uri != key.key.uri
                {
                    return Err("unowned resolved support key".into());
                }
                let source = self
                    .support
                    .get(key.key.uri.as_ref())
                    .ok_or("missing support source")?;
                // Shared module loading replaces this fallback with the declared
                // name or its own original-key URI inference before compilation.
                let segment = key.key.uri.rsplit('/').next().unwrap_or("");
                let name = segment.rsplit_once('.').map_or(segment, |(name, _)| name);
                probe.complete_module_source(token, source, name, 0)
            }
            _ => {
                if !probe
                    .complete_default_module(token, 0)
                    .map_err(|e| format!("default module completion: {e:?}"))?
                {
                    return Err(format!("unconfigured module request: {request:?}"));
                }
                self.observe("host-module-completion", ("default", request.id))?;
                return Ok(());
            }
        };
        self.observe("host-module-completion", (&request, &completed))?;
        completed.map_err(|e| format!("module completion: {e:?}"))
    }

    fn observe(&mut self, kind: &str, value: impl std::fmt::Debug) -> CaptureResult<()> {
        writeln!(self.trace, "{kind}\t{value:?}").map_err(|error| error.to_string())
    }

    fn result(
        &mut self,
        out: &Path,
        call: &str,
        status: &str,
        value: &[u16],
        metadata: &[(&str, &str)],
    ) -> CaptureResult<()> {
        let success = status == "success";
        if success {
            self.successes += 1;
        } else {
            self.errors += 1;
        }
        write(out, "result.status.bin", format!("{status}\n"))?;
        units(out, "result.stdout", if success { value } else { &[] })?;
        units(out, "result.stderr", if success { &[] } else { value })?;
        for (name, value) in metadata {
            write(out, &format!("candidate.{name}.bin"), value)?;
        }
        println!("call-exit={call} status={status}");
        Ok(())
    }

    fn call(
        &mut self,
        probe: &mut SourceProbe,
        out: &Path,
        index: usize,
        id: &str,
        expression: &str,
    ) -> CaptureResult<()> {
        let call = format!("string-{index}-{id}");
        println!("call-enter={call}");
        self.observe("evaluate-module-source", (self.uri.clone(), expression))?;
        let started =
            probe.evaluate_module_source(&self.uri, Some(&self.source), expression, 0, None);
        if let Err(error) = started {
            self.observe("api-error", &error)?;
            return self.result(
                out,
                &call,
                "api-exception",
                &error.message.encode_utf16().collect::<Vec<_>>(),
                &[("api-error-kind", &format!("{:?}", error.kind))],
            );
        }
        loop {
            self.polls += 1;
            if self.polls > 2_000_000 {
                return Err("session polling bound exceeded".into());
            }
            let event = match probe.poll(0, 1) {
                Ok(event) => event,
                Err(error) => {
                    self.observe("poll-api-error", &error)?;
                    return self.result(
                        out,
                        &call,
                        "api-exception",
                        &error.message.encode_utf16().collect::<Vec<_>>(),
                        &[("api-error-kind", &format!("{:?}", error.kind))],
                    );
                }
            };
            probe.collect();
            self.collections += 1;
            match event.kind {
                "yield" => {}
                "module-request" => {
                    self.observe("module-request", &event)?;
                    self.module(probe, event.token.as_deref().ok_or("missing module token")?)?;
                }
                "value" | "error" => {
                    let success = event.kind == "value";
                    let status = if success {
                        "success"
                    } else if event.disposition == Some("language-error") {
                        "error"
                    } else {
                        "candidate-error"
                    };
                    self.observe("terminal", &event)?;
                    let value = if success {
                        event.value.ok_or("missing result payload")?
                    } else {
                        event.error.ok_or("missing error payload")?
                    };
                    return self.result(
                        out,
                        &call,
                        status,
                        &value,
                        &[
                            ("disposition", event.disposition.unwrap_or("")),
                            ("phase", event.phase.unwrap_or("")),
                            ("reason", event.reason.unwrap_or("")),
                            ("callback-origin", event.callback_origin.unwrap_or("")),
                        ],
                    );
                }
                _ => return Err(format!("unexpected event in nonlogging session: {event:?}")),
            }
        }
    }
}

fn parse_queries(text: &str, allow_mode_prefix: bool) -> CaptureResult<Vec<(&str, &str)>> {
    let mut rows = Vec::new();
    let mut ids = std::collections::BTreeSet::new();
    for line in text.lines() {
        let fields = line.split('\t').collect::<Vec<_>>();
        let (id, expression) = match fields.as_slice() {
            [id, expression] => (*id, *expression),
            ["string", id, expression] if allow_mode_prefix => (*id, *expression),
            _ => return Err("invalid query row".into()),
        };
        if id.is_empty()
            || !id
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
            || !ids.insert(id)
        {
            return Err("invalid or duplicate query identity".into());
        }
        rows.push((id, expression));
    }
    Ok(rows)
}

fn run(data_size_protocol: bool) -> CaptureResult<()> {
    let args = std::env::args().collect::<Vec<_>>();
    if args.len() != 4 {
        return Err(if data_size_protocol {
            "usage: data_size_fixture_probe OUTPUT INPUT URI"
        } else {
            "usage: string_session_probe OUTPUT INPUT URI"
        }
        .into());
    }
    let out = PathBuf::from(&args[1]);
    if fs::read_dir(&out)
        .map_err(|error| error.to_string())?
        .next()
        .is_some()
    {
        return Err("output directory must exist and be empty".into());
    }
    let input = PathBuf::from(&args[2]);
    let source = fs::read_to_string(input.join("module.pkl")).map_err(|e| e.to_string())?;
    let queries = fs::read_to_string(input.join("queries.tsv")).map_err(|e| e.to_string())?;
    let rows = parse_queries(&queries, !data_size_protocol)?;
    let trace = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(out.join("candidate-trace.tsv"))
        .map_err(|e| e.to_string())?;
    let mut capture = Capture {
        uri: args[3].clone(),
        source,
        support: HashMap::new(),
        keys: HashMap::new(),
        resolved_keys: HashMap::new(),
        trace,
        polls: 0,
        collections: 0,
        successes: 0,
        errors: 0,
    };
    println!("session-open mode=string rows={}", rows.len());
    let mut probe = SourceProbe::new_module_evaluator("compact", false, false, false)
        .map_err(|error| error.to_java_utf8())?;
    capture.observe(
        "constructor",
        ("module-evaluator", "compact", false, false, false),
    )?;
    for (index, (id, expression)) in rows.iter().enumerate() {
        let directory = out.join(format!("{index:03}-{id}"));
        fs::create_dir(&directory).map_err(|e| e.to_string())?;
        units(
            &directory,
            "expression",
            &expression.encode_utf16().collect::<Vec<_>>(),
        )?;
        capture.call(&mut probe, &directory, index, id, expression)?;
    }
    probe.close();
    probe.collect();
    capture.collections += 1;
    capture.observe("terminal-heap-slots", probe.heap_slots())?;
    capture.observe("terminal-context-depth", probe.context_depth())?;
    capture.observe("polls", capture.polls)?;
    capture.observe("collections", capture.collections)?;
    if probe.heap_slots() != 0 || probe.context_depth() != 0 {
        return Err("nonzero state after terminal close".into());
    }
    let suffix = if data_size_protocol {
        " datasize-rows=0 repeated-export-calls=0"
    } else {
        ""
    };
    println!(
        "session-closed mode=string rows={} public-calls={} success={} errors={}{suffix}",
        rows.len(),
        rows.len(),
        capture.successes,
        capture.errors
    );
    Ok(())
}

pub fn entrypoint(data_size_protocol: bool) {
    if let Err(error) = run(data_size_protocol) {
        let label = if data_size_protocol {
            "DataSize"
        } else {
            "String session"
        };
        eprintln!("{label} capture failed: {error}");
        std::process::exit(2);
    }
}

#[cfg(test)]
mod tests {
    use super::parse_queries;
    #[test]
    fn exact_string_mode_and_legacy_queries_preserve_expressions() {
        let source = "string\tfirst\t IntSeq(-1, 1).map((n) -> n) \nsecond\t\"λ\"\n";
        assert_eq!(
            parse_queries(source, true).unwrap(),
            vec![
                ("first", " IntSeq(-1, 1).map((n) -> n) "),
                ("second", "\"λ\"")
            ]
        );
        assert_eq!(
            parse_queries("legacy\t1\n", false).unwrap(),
            vec![("legacy", "1")]
        );
        assert!(parse_queries("string\tfirst\t1\n", false).is_err());
    }
    #[test]
    fn export_extra_columns_and_duplicate_identities_are_rejected() {
        for text in [
            "export\tfirst\t1\n",
            "other\tfirst\t1\n",
            "string\tfirst\t1\textra\n",
            "first\t1\nstring\tfirst\t2\n",
            "string\t\t1\n",
            "missing",
        ] {
            assert!(parse_queries(text, true).is_err(), "accepted {text:?}");
        }
    }
}
