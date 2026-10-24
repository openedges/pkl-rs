//! Exact source/query Logger observations through the shared public evaluator.
//! Java exceptions are reference provenance, never manufactured by this adapter.
//! Usage: DIRECTORY URI LOGGER_MODE LOGGER_ACTION CALLBACK CALLBACK_AFTER.
//! DIRECTORY contains input.pkl, expressions.txt, modules.tsv and support files.
//! Java SPI callbacks remain separate authority, not synthesized Host events.
use pkl_runtime::{
    modules::{ModuleKeyKind, ModuleRequestKind, SelectionPhase},
    probe::{SourceLog, SourceProbe},
    value::{PklString, RuntimeError},
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
    out: PathBuf,
    uri: String,
    source: String,
    mode: String,
    action: String,
    throw_used: bool,
    close_used: bool,
    support: HashMap<String, String>,
    callback: String,
    callback_after: usize,
    callback_used: bool,
    logs: usize,
    warnings: usize,
    depth: usize,
    polls: usize,
    collections: usize,
    closed: bool,
    keys: HashMap<u64, (String, String)>,
    resolved_keys: HashMap<u64, (u64, String, String)>,
    trace: fs::File,
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

    fn api_error(&mut self, prefix: &str, error: RuntimeError) -> CaptureResult<()> {
        self.observe("api-error", &error)?;
        write(
            &self.out,
            &format!("{prefix}.callback-origin.bin"),
            error.callback_origin().map_or("", |origin| origin.as_str()),
        )?;
        write(
            &self.out,
            &format!("{prefix}.api-error-kind.bin"),
            format!("{:?}", error.kind),
        )?;
        units(&self.out, &format!("{prefix}.stdout"), &[])?;
        units(
            &self.out,
            &format!("{prefix}.stderr"),
            &error.message.encode_utf16().collect::<Vec<_>>(),
        )?;
        write(
            &self.out,
            &format!("{prefix}.status.bin"),
            "api-exception\n",
        )?;
        println!("call-exit={prefix} status=api-exception");
        Ok(())
    }

    fn call(
        &mut self,
        probe: &mut SourceProbe,
        expression: &str,
        prefix: &str,
        callback_token: Option<&str>,
    ) -> CaptureResult<()> {
        println!("call-enter={prefix}");
        self.observe("call", (prefix, expression, callback_token))?;
        let started = if let Some(token) = callback_token.filter(|_| !self.closed) {
            probe.reenter_module_source(token, &self.uri, Some(&self.source), expression, 0, None)
        } else {
            probe.evaluate_module_source(&self.uri, Some(&self.source), expression, 0, None)
        };
        if let Err(error) = started {
            return self.api_error(prefix, error);
        }
        loop {
            self.polls += 1;
            if self.polls > 2_000_000 {
                return Err("session polling bound exceeded".into());
            }
            let event = match probe.poll(0, 1) {
                Ok(event) => event,
                Err(error) => return self.api_error(prefix, error),
            };
            probe.collect();
            self.collections += 1;
            match event.kind {
                "yield" => {}
                "module-request" => {
                    self.observe("module-request", &event)?;
                    self.module(probe, event.token.as_deref().ok_or("missing module token")?)?;
                }
                "log" => self.log(
                    probe,
                    event.token.as_deref().ok_or("missing logger token")?,
                    event.log.ok_or("missing logger payload")?,
                )?,
                "value" | "error" => {
                    let is_value = event.kind == "value";
                    let status = if is_value {
                        "success"
                    } else if event.disposition == Some("language-error") {
                        "error"
                    } else {
                        "candidate-error"
                    };
                    self.observe("terminal", &event)?;
                    let value = if is_value {
                        event.value.ok_or("missing result payload")?
                    } else {
                        event.error.ok_or("missing error payload")?
                    };
                    units(
                        &self.out,
                        &format!("{prefix}.stdout"),
                        if is_value { &value } else { &[] },
                    )?;
                    units(
                        &self.out,
                        &format!("{prefix}.stderr"),
                        if is_value { &[] } else { &value },
                    )?;
                    write(
                        &self.out,
                        &format!("{prefix}.status.bin"),
                        format!("{status}\n"),
                    )?;
                    for (field, value) in [
                        ("disposition", event.disposition),
                        ("phase", event.phase),
                        ("reason", event.reason),
                        ("callback-origin", event.callback_origin),
                    ] {
                        write(
                            &self.out,
                            &format!("{prefix}.{field}.bin"),
                            value.unwrap_or(""),
                        )?;
                    }
                    println!("call-exit={prefix} status={status}");
                    return Ok(());
                }
                _ => return Err(format!("unexpected event: {event:?}")),
            }
        }
    }

    fn log(&mut self, probe: &mut SourceProbe, token: &str, log: SourceLog) -> CaptureResult<()> {
        let index = self.logs;
        self.logs += 1;
        if log.level == "warn" {
            self.warnings += 1;
        }
        let prefix = format!("log-{index}");
        println!("log-enter={index} level={} depth={}", log.level, self.depth);
        self.observe("logger", (token, &log))?;
        write(&self.out, &format!("{prefix}.level.bin"), log.level)?;
        units(&self.out, &format!("{prefix}.message"), &log.message)?;
        write(
            &self.out,
            &format!("{prefix}.message.length.bin"),
            log.message.len().to_string(),
        )?;
        units(&self.out, &format!("{prefix}.uri"), &log.module_uri)?;
        units(
            &self.out,
            &format!("{prefix}.member"),
            log.member_name.as_deref().unwrap_or(&[]),
        )?;
        write(
            &self.out,
            &format!("{prefix}.member.null.bin"),
            log.member_name.is_none().to_string(),
        )?;
        write(
            &self.out,
            &format!("{prefix}.position.bin"),
            format!(
                "{}:{}:{}:{}",
                log.start_line, log.start_column, log.end_line, log.end_column
            ),
        )?;
        write(
            &self.out,
            &format!("{prefix}.source-count.bin"),
            log.source_lines.len().to_string(),
        )?;
        for (line, value) in log.source_lines.iter().enumerate() {
            units(&self.out, &format!("{prefix}.source-{line}"), value)?;
        }
        self.depth += 1;
        if !self.callback_used && !self.callback.is_empty() && index >= self.callback_after {
            self.callback_used = true;
            let callback = self.callback.clone();
            self.call(
                probe,
                &callback,
                &format!("log-callback-{index}"),
                Some(token),
            )?;
        }
        // The nested call can consume the action before this callback resumes.
        let failed =
            index >= self.callback_after && self.action == "throw-once" && !self.throw_used;
        if failed {
            self.throw_used = true;
            println!("log-throw={index}");
        }
        if index >= self.callback_after && self.action == "close" && !self.close_used {
            self.close_used = true;
            probe
                .close_from_callback(token)
                .map_err(|e| format!("callback close: {e:?}"))?;
            self.closed = true;
            println!("logger-close={index}");
        }
        self.depth -= 1;
        let failure = failed.then(|| "stage56-logger-failure".encode_utf16().collect());
        self.observe("acknowledge-log", (token, &failure))?;
        probe
            .acknowledge_log(token, failure, 0)
            .map_err(|e| format!("logger acknowledgement: {e:?}"))?;
        if !failed {
            println!("log-exit={index}");
        }
        Ok(())
    }
}

fn run() -> CaptureResult<()> {
    let args = std::env::args().collect::<Vec<_>>();
    if args.len() != 7 {
        return Err("DIRECTORY URI LOGGER_MODE LOGGER_ACTION CALLBACK CALLBACK_AFTER".into());
    }
    if !["capture", "noop"].contains(&args[3].as_str())
        || !["none", "throw-once", "close"].contains(&args[4].as_str())
    {
        return Err("unknown logger mode/action".into());
    }
    if args[3] == "noop" && (args[4] != "none" || !args[5].is_empty()) {
        return Err("noop logger cannot invoke actions".into());
    }
    let out = PathBuf::from(&args[1]);
    let source = fs::read_to_string(out.join("input.pkl")).map_err(|error| error.to_string())?;
    let expressions =
        fs::read_to_string(out.join("expressions.txt")).map_err(|error| error.to_string())?;
    let trace = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(out.join("candidate-trace.tsv"))
        .map_err(|error| error.to_string())?;
    let mut support = HashMap::new();
    let mappings = fs::read_to_string(out.join("modules.tsv")).map_err(|e| e.to_string())?;
    for line in mappings.lines() {
        let (uri, name) = line.split_once('\t').ok_or("invalid support mapping")?;
        if uri == args[2]
            || name.contains('/')
            || name.contains('\\')
            || !name.starts_with("support-")
            || !name.ends_with(".pkl")
        {
            return Err("invalid support source path/URI".into());
        }
        let text = fs::read_to_string(out.join(name)).map_err(|e| e.to_string())?;
        if support.insert(uri.to_owned(), text).is_some() {
            return Err("duplicate support URI".into());
        }
    }
    let mut capture = Capture {
        out,
        source,
        uri: args[2].clone(),
        mode: args[3].clone(),
        action: args[4].clone(),
        throw_used: false,
        close_used: false,
        support,
        callback: args[5].clone(),
        callback_after: args[6].parse().map_err(|_| "invalid callback threshold")?,
        callback_used: false,
        logs: 0,
        warnings: 0,
        depth: 0,
        polls: 0,
        collections: 0,
        closed: false,
        keys: HashMap::new(),
        resolved_keys: HashMap::new(),
        trace,
    };
    let mut probe =
        SourceProbe::new_module_evaluator("compact", capture.mode != "noop", false, false)
            .map_err(|error| error.to_java_utf8())?;
    capture.observe(
        "constructor",
        (
            "module-evaluator",
            "compact",
            capture.mode != "noop",
            false,
            false,
        ),
    )?;
    for (index, expression) in expressions.lines().enumerate() {
        capture.call(&mut probe, expression, &format!("outer-{index}"), None)?;
    }
    probe.close();
    probe.collect();
    capture.collections += 1;
    capture.observe("terminal-heap-slots", probe.heap_slots())?;
    capture.observe("terminal-context-depth", probe.context_depth())?;
    capture.observe("polls", capture.polls)?;
    capture.observe("collections", capture.collections)?;
    println!("closed logs={} warnings={}", capture.logs, capture.warnings);
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("Class capture failed: {error}");
        std::process::exit(2);
    }
}
