//! Exact source/query transformer and Logger observations through the shared evaluator.
//! Java exceptions are reference provenance, never manufactured by this adapter.
//! Usage: DIRECTORY URI OPERATION TMODE TACTION TCALLBACK TAFTER LMODE LACTION LCALLBACK LAFTER.
//! DIRECTORY contains
//! input.pkl and expressions.txt; every observation file must be new.
use pkl_runtime::{
    modules::{ModuleKeyKind, ModuleRequestKind},
    probe::{SourceFrame, SourceLog, SourceProbe},
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
    transformer_mode: String,
    transformer_action: String,
    transformer_callback: String,
    transformer_after: usize,
    transformer_callback_used: bool,
    transformer_thrown: bool,
    logger_mode: String,
    logger_action: String,
    logger_callback: String,
    logger_after: usize,
    logger_callback_used: bool,
    logger_thrown: bool,
    replacement: Option<SourceFrame>,
    transforms: usize,
    logs: usize,
    warnings: usize,
    depth: usize,
    polls: usize,
    collections: usize,
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
                let handle = format!("frame-transformer-primary:{}", id.0);
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
        let started = if let Some(token) = callback_token {
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
                "transform-frame" => self.transform(
                    probe,
                    event.token.as_deref().ok_or("missing transformer token")?,
                    event.frame.ok_or("missing transformer frame")?,
                )?,
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

    fn frame(&self, prefix: &str, frame: &SourceFrame) -> CaptureResult<()> {
        units(&self.out, &format!("{prefix}.uri"), &frame.module_uri)?;
        units(
            &self.out,
            &format!("{prefix}.member"),
            frame.member_name.as_deref().unwrap_or(&[]),
        )?;
        write(
            &self.out,
            &format!("{prefix}.member.null.bin"),
            frame.member_name.is_none().to_string(),
        )?;
        write(
            &self.out,
            &format!("{prefix}.position.bin"),
            format!(
                "{}:{}:{}:{}",
                frame.start_line, frame.start_column, frame.end_line, frame.end_column
            ),
        )?;
        write(
            &self.out,
            &format!("{prefix}.source-count.bin"),
            frame.source_lines.len().to_string(),
        )?;
        for (index, line) in frame.source_lines.iter().enumerate() {
            units(&self.out, &format!("{prefix}.source-{index}"), line)?;
        }
        Ok(())
    }

    fn transform(
        &mut self,
        probe: &mut SourceProbe,
        token: &str,
        original: SourceFrame,
    ) -> CaptureResult<()> {
        let index = self.transforms;
        self.transforms += 1;
        println!("transform-enter={index} depth={}", self.depth);
        self.depth += 1;
        self.observe("transformer", (token, &original))?;
        self.frame(&format!("transform-{index}.input"), &original)?;
        if !self.transformer_callback_used
            && !self.transformer_callback.is_empty()
            && index >= self.transformer_after
        {
            self.transformer_callback_used = true;
            let callback = self.transformer_callback.clone();
            self.call(
                probe,
                &callback,
                &format!("transform-callback-{index}"),
                Some(token),
            )?;
        }
        let failure = self.transformer_action == "throw-once"
            && !self.transformer_thrown
            && index >= self.transformer_after;
        if failure {
            self.transformer_thrown = true;
            println!("transform-throw={index}");
            self.depth -= 1;
            probe
                .complete_frame_transform(
                    token,
                    None,
                    Some("stage55-transformer-failure".encode_utf16().collect()),
                    0,
                )
                .map_err(|error| format!("transformer rejection: {error:?}"))?;
            return Ok(());
        }
        let mut frame = match self.transformer_mode.as_str() {
            "identity" => original,
            "full" | "numbered" => self
                .replacement
                .clone()
                .ok_or("missing replacement frame")?,
            _ => return Err("unexpected callback for builtin transformer".into()),
        };
        if self.transformer_mode == "numbered" {
            frame.module_uri.extend(format!("#{index}").encode_utf16());
        }
        self.frame(&format!("transform-{index}.output"), &frame)?;
        println!("transform-exit={index}");
        self.depth -= 1;
        probe
            .complete_frame_transform(token, Some(frame), None, 0)
            .map_err(|error| format!("transformer completion: {error:?}"))?;
        Ok(())
    }

    fn log(&mut self, probe: &mut SourceProbe, token: &str, log: SourceLog) -> CaptureResult<()> {
        let index = self.logs;
        self.logs += 1;
        if log.level == "warn" {
            self.warnings += 1;
        }
        println!("log-enter={index} level={} depth={}", log.level, self.depth);
        self.depth += 1;
        self.observe("logger", (token, &log))?;
        let prefix = format!("log-{index}");
        write(&self.out, &format!("{prefix}.level.bin"), log.level)?;
        units(&self.out, &format!("{prefix}.message"), &log.message)?;
        write(
            &self.out,
            &format!("{prefix}.message.length.bin"),
            log.message.len().to_string(),
        )?;
        self.frame(
            &prefix,
            &SourceFrame {
                module_uri: log.module_uri,
                member_name: log.member_name,
                source_lines: log.source_lines,
                start_line: log.start_line,
                start_column: log.start_column,
                end_line: log.end_line,
                end_column: log.end_column,
            },
        )?;
        if !self.logger_callback_used
            && !self.logger_callback.is_empty()
            && index >= self.logger_after
        {
            self.logger_callback_used = true;
            let callback = self.logger_callback.clone();
            self.call(
                probe,
                &callback,
                &format!("log-callback-{index}"),
                Some(token),
            )?;
        }
        let failure =
            self.logger_action == "throw-once" && !self.logger_thrown && index >= self.logger_after;
        if failure {
            self.logger_thrown = true;
            println!("log-throw={index}");
        } else {
            println!("log-exit={index}");
        }
        self.depth -= 1;
        probe
            .acknowledge_log(
                token,
                failure.then(|| "stage55-logger-failure".encode_utf16().collect()),
                0,
            )
            .map_err(|error| format!("logger acknowledgement: {error:?}"))?;
        Ok(())
    }
}

fn read_units(directory: &Path, name: &str) -> CaptureResult<Vec<u16>> {
    let bytes =
        fs::read(directory.join(format!("{name}.u16be"))).map_err(|error| error.to_string())?;
    if !bytes.len().is_multiple_of(2) {
        return Err("odd replacement UTF-16 byte count".into());
    }
    Ok(bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u16::from_be_bytes([pair[0], pair[1]]))
        .collect())
}

fn replacement(out: &Path) -> CaptureResult<SourceFrame> {
    let directory = out.join("replacement");
    let text =
        |name: &str| fs::read_to_string(directory.join(name)).map_err(|error| error.to_string());
    let positions = text("position.txt")?
        .split(':')
        .map(|value| value.parse::<u32>().map_err(|error| error.to_string()))
        .collect::<Result<Vec<_>, _>>()?;
    let [start_line, start_column, end_line, end_column]: [u32; 4] = positions
        .try_into()
        .map_err(|_| "expected four frame positions")?;
    let count = text("source-count.txt")?
        .parse::<usize>()
        .map_err(|error| error.to_string())?;
    let member_name = match text("member-null.txt")?.as_str() {
        "true" => None,
        "false" => Some(read_units(&directory, "member")?),
        _ => return Err("invalid nullable member marker".into()),
    };
    Ok(SourceFrame {
        module_uri: read_units(&directory, "uri")?,
        member_name,
        source_lines: (0..count)
            .map(|index| read_units(&directory, &format!("source-{index}")))
            .collect::<Result<_, _>>()?,
        start_line,
        start_column,
        end_line,
        end_column,
    })
}

fn run() -> CaptureResult<()> {
    let args = std::env::args().collect::<Vec<_>>();
    if args.len() != 12 {
        return Err(
            "DIRECTORY URI OPERATION TMODE TACTION TCALLBACK TAFTER LMODE LACTION LCALLBACK LAFTER"
                .into(),
        );
    }
    if args[3] != "expression"
        || ![
            "default",
            "builtin-identity",
            "identity",
            "full",
            "numbered",
        ]
        .contains(&args[4].as_str())
        || !["none", "throw-once"].contains(&args[5].as_str())
        || !["capture", "noop"].contains(&args[8].as_str())
        || !["none", "throw-once"].contains(&args[9].as_str())
    {
        return Err("unsupported acquisition profile".into());
    }
    let out = PathBuf::from(&args[1]);
    let source = fs::read_to_string(out.join("input.pkl")).map_err(|error| error.to_string())?;
    let expressions =
        fs::read_to_string(out.join("expressions.txt")).map_err(|error| error.to_string())?;
    let replacement = if ["full", "numbered"].contains(&args[4].as_str()) {
        Some(replacement(&out)?)
    } else {
        None
    };
    let trace = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(out.join("candidate-trace.tsv"))
        .map_err(|error| error.to_string())?;
    let mut capture = Capture {
        out,
        source,
        uri: args[2].clone(),
        transformer_mode: args[4].clone(),
        transformer_action: args[5].clone(),
        transformer_callback: args[6].clone(),
        transformer_after: args[7]
            .parse()
            .map_err(|_| "invalid transformer threshold")?,
        transformer_callback_used: false,
        transformer_thrown: false,
        logger_mode: args[8].clone(),
        logger_action: args[9].clone(),
        logger_callback: args[10].clone(),
        logger_after: args[11].parse().map_err(|_| "invalid logger threshold")?,
        logger_callback_used: false,
        logger_thrown: false,
        replacement,
        transforms: 0,
        logs: 0,
        warnings: 0,
        depth: 0,
        polls: 0,
        collections: 0,
        primary_keys: HashMap::new(),
        trace,
    };
    let mut probe =
        SourceProbe::new_module_evaluator("compact", capture.logger_mode != "noop", false, false)
            .map_err(|error| error.to_java_utf8())?;
    let policy = match capture.transformer_mode.as_str() {
        "default" => "default",
        "builtin-identity" => "identity",
        _ => "host",
    };
    probe
        .set_frame_transformer(policy)
        .map_err(|error| format!("transformer policy: {error:?}"))?;
    capture.observe(
        "constructor",
        (
            "module-evaluator",
            "compact",
            capture.logger_mode.clone(),
            capture.transformer_mode.clone(),
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
    println!(
        "closed transforms={} logs={} warnings={}",
        capture.transforms, capture.logs, capture.warnings
    );
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("frame transformer capture failed: {error}");
        std::process::exit(2);
    }
}
