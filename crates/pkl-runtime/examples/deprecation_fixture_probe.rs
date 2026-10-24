//! Exact source/query Logger observations through the shared public evaluator.
//! Java exceptions are reference provenance, never manufactured by this adapter.
//! Usage: DIRECTORY URI ACTION CALLBACK CALLBACK_AFTER. DIRECTORY contains
//! input.pkl and expressions.txt; every observation file must be new.
use pkl_runtime::{
    modules::{ModuleKeyKind, ModuleRequestKind},
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
    action: String,
    callback: String,
    callback_after: usize,
    callback_used: bool,
    logs: usize,
    warnings: usize,
    depth: usize,
    polls: usize,
    collections: usize,
    closed: bool,
    primary_keys: HashMap<u64, String>,
    trace: fs::File,
}

impl Capture {
    fn module(&mut self, probe: &mut SourceProbe, token: &str) -> CaptureResult<()> {
        let request = probe
            .module_request(token)
            .map_err(|error| format!("module request: {error:?}"))?;
        let completed = match &request.kind {
            ModuleRequestKind::SelectKey { id, uri, .. } if uri.as_ref() == self.uri => {
                let handle = format!("deprecation-primary:{}", id.0);
                if self.primary_keys.insert(id.0, handle.clone()).is_some() {
                    return Err("reused primary ModuleKey identity".into());
                }
                probe.complete_module_key(token, &id.0.to_string(), uri, "host", &handle, 0)
            }
            ModuleRequestKind::QueryCachePolicy { key }
            | ModuleRequestKind::QueryFragmentPaths { key }
                if key.uri.as_ref() == self.uri && key.kind == ModuleKeyKind::Host =>
            {
                if self.primary_keys.get(&key.id.0).map(String::as_str) != Some(key.handle.as_ref())
                {
                    return Err("unowned primary ModuleKey descriptor".into());
                }
                if matches!(&request.kind, ModuleRequestKind::QueryCachePolicy { .. }) {
                    probe.complete_module_cache_policy(token, &key.id.0.to_string(), true, 0)
                } else {
                    probe.complete_module_fragment_paths(token, &key.id.0.to_string(), false, 0)
                }
            }
            _ => {
                if !probe
                    .complete_default_module(token, 0)
                    .map_err(|error| format!("module completion: {error:?}"))?
                {
                    return Err(format!("unconfigured module request: {request:?}"));
                }
                return Ok(());
            }
        };
        completed.map_err(|error| format!("primary ModuleKey completion: {error:?}"))
    }

    fn observe(&mut self, kind: &str, value: impl std::fmt::Debug) -> CaptureResult<()> {
        writeln!(self.trace, "{kind}\t{value:?}").map_err(|error| error.to_string())
    }

    fn api_error(&mut self, prefix: &str, error: RuntimeError) -> CaptureResult<()> {
        self.observe("api-error", &error)?;
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
        let warning = self.warnings;
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
        let mut failed = false;
        if log.level == "warn" {
            if self.depth == 1
                && !self.callback_used
                && !self.callback.is_empty()
                && warning >= self.callback_after
            {
                self.callback_used = true;
                let callback = self.callback.clone();
                self.call(
                    probe,
                    &callback,
                    &format!("log-callback-{index}"),
                    Some(token),
                )?;
            }
            failed = self.action == "throw" || (self.action == "throw-once" && warning == 0);
            if self.action.starts_with("close") {
                self.observe("close-from-callback", token)?;
                probe
                    .close_from_callback(token)
                    .map_err(|error| format!("callback close: {error:?}"))?;
                self.closed = true;
            }
            if self.action == "close-reenter" {
                self.call(
                    probe,
                    "stable",
                    &format!("post-close-callback-{index}"),
                    None,
                )?;
            }
        }
        self.depth -= 1;
        // Closing from a callback rejects new evaluations but still permits
        // this issued continuation to complete. The token must be acknowledged.
        let failure = failed.then(|| "stage54-logger-failure".encode_utf16().collect());
        self.observe("acknowledge-log", (token, &failure))?;
        probe
            .acknowledge_log(token, failure, 0)
            .map_err(|error| format!("logger acknowledgement: {error:?}"))?;
        if !failed {
            println!("log-exit={index}");
        }
        Ok(())
    }
}

fn run() -> CaptureResult<()> {
    let args = std::env::args().collect::<Vec<_>>();
    if args.len() != 6 {
        return Err("DIRECTORY URI ACTION CALLBACK CALLBACK_AFTER".into());
    }
    if ![
        "capture",
        "noop",
        "throw",
        "throw-once",
        "close",
        "close-reenter",
    ]
    .contains(&args[3].as_str())
    {
        return Err("unknown logger action".into());
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
    let mut capture = Capture {
        out,
        source,
        uri: args[2].clone(),
        action: args[3].clone(),
        callback: args[4].clone(),
        callback_after: args[5].parse().map_err(|_| "invalid callback threshold")?,
        callback_used: false,
        logs: 0,
        warnings: 0,
        depth: 0,
        polls: 0,
        collections: 0,
        closed: false,
        primary_keys: HashMap::new(),
        trace,
    };
    let mut probe =
        SourceProbe::new_module_evaluator("compact", capture.action != "noop", false, false)
            .map_err(|error| error.to_java_utf8())?;
    capture.observe(
        "constructor",
        (
            "module-evaluator",
            "compact",
            capture.action != "noop",
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
        eprintln!("deprecation capture failed: {error}");
        std::process::exit(2);
    }
}
