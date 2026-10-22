//! Parsed-source logger/reader comparison adapter. Actions drive actual pending
//! Host events; all language evaluation, error formatting and UTF-16 records are
//! shared by native, Wasm and N-API through SourceProbe.
use pkl_runtime::{
    probe::{SourceEvent, SourceLog, SourceProbe},
    value::{PklString, Value},
};
use std::{
    fs,
    path::{Path, PathBuf},
};

fn write(out: &Path, name: &str, bytes: impl AsRef<[u8]>) {
    fs::write(out.join(name), bytes).expect("write trace capture");
}
fn text(units: Vec<u16>) -> String {
    PklString::from_utf16(units).to_java_utf8()
}
fn utf16(out: &Path, name: &str, units: &[u16]) {
    write(
        out,
        name,
        units
            .iter()
            .flat_map(|n| n.to_be_bytes())
            .collect::<Vec<_>>(),
    );
}
fn save_log(out: &Path, id: usize, log: SourceLog) {
    let prefix = format!("log-{id}");
    utf16(
        out,
        &format!("{prefix}.message.code-units.bin"),
        &log.message,
    );
    write(
        out,
        &format!("{prefix}.message.length.bin"),
        log.message.len().to_string(),
    );
    write(out, &format!("{prefix}.level.bin"), log.level);
    utf16(
        out,
        &format!("{prefix}.uri.code-units.bin"),
        &log.module_uri,
    );
    write(
        out,
        &format!("{prefix}.member.null.bin"),
        if log.member_name.is_none() {
            "true"
        } else {
            "false"
        },
    );
    utf16(
        out,
        &format!("{prefix}.member.code-units.bin"),
        log.member_name.as_deref().unwrap_or(&[]),
    );
    write(out, &format!("{prefix}.message.bin"), text(log.message));
    write(out, &format!("{prefix}.uri.bin"), text(log.module_uri));
    write(
        out,
        &format!("{prefix}.member.bin"),
        log.member_name.map_or_else(|| "null".into(), text),
    );
    write(
        out,
        &format!("{prefix}.position.bin"),
        format!(
            "{}:{}:{}:{}",
            log.start_line, log.start_column, log.end_line, log.end_column
        ),
    );
    for (index, line) in log.source_lines.into_iter().enumerate() {
        utf16(
            out,
            &format!("{prefix}.source-{index}.code-units.bin"),
            &line,
        );
        write(out, &format!("{prefix}.source-{index}.bin"), text(line));
    }
}
fn save_result(out: &Path, prefix: &str, event: SourceEvent) {
    let (status, stdout, stderr) = match event.kind {
        "value" => (
            "success\n",
            text(event.value.expect("value")).into_bytes(),
            Vec::new(),
        ),
        "bytes" => ("success\n", event.bytes.expect("bytes"), Vec::new()),
        "error" => (
            "error\n",
            Vec::new(),
            event
                .error_bytes
                .unwrap_or_else(|| text(event.error.expect("error")).into_bytes()),
        ),
        kind => panic!("expected expression completion, got {kind}"),
    };
    for (channel, bytes) in [
        ("status", status.as_bytes()),
        ("stdout", stdout.as_slice()),
        ("stderr", stderr.as_slice()),
        ("error-class", b""),
    ] {
        write(out, &format!("{prefix}.{channel}.bin"), bytes);
    }
    println!(
        "result={prefix} status={}",
        status.strip_suffix('\n').unwrap()
    );
}
struct Driver {
    probe: SourceProbe,
    out: PathBuf,
    action: String,
    logger_path: Option<u32>,
    reader_path: Option<u32>,
    post_close_path: Option<u32>,
    logger_depth: usize,
    reader_depth: usize,
    logs: usize,
    reads: usize,
    collections: usize,
    budget: u32,
    output: bool,
}
impl Driver {
    fn poll(&mut self) -> SourceEvent {
        for _ in 0..100_000 {
            let event = self
                .probe
                .poll(0, self.budget)
                .expect("poll source session");
            self.probe.collect();
            self.collections += 1;
            if event.kind != "yield" {
                return event;
            }
        }
        panic!("source logger probe instruction budget exceeded");
    }
    fn capture(&mut self, index: u32, prefix: &str, parent: Option<&str>) {
        let start = if let Some(token) = parent {
            self.probe.reenter_path(token, index, 0, None)
        } else if self.output {
            self.probe.start_output_bytes(0, None)
        } else {
            self.probe.start_path(index, 0, None)
        };
        let event = match start {
            Ok(()) => self.evaluate(),
            Err(error) => SourceEvent {
                kind: "error",
                token: None,
                uri: None,
                value: None,
                bytes: None,
                error: Some(error.message.encode_utf16().collect()),
                error_bytes: None,
                log: None,
            },
        };
        save_result(&self.out, prefix, event);
    }
    fn evaluate(&mut self) -> SourceEvent {
        loop {
            let event = self.poll();
            match event.kind {
                "module" | "value" | "bytes" | "error" => return event,
                "read" => {
                    let token = event.token.expect("read token");
                    let uri = event.uri.expect("read URI");
                    if self.output && !uri.starts_with("life:") {
                        self.probe
                            .respond_configured(&token, 0)
                            .expect("configured resource completion");
                        continue;
                    }
                    let id = self.reads;
                    self.reads += 1;
                    println!("read-enter={id} uri={uri}");
                    self.reader_depth += 1;
                    if self.reader_depth == 1
                        && let Some(index) = self.reader_path
                    {
                        self.capture(index, &format!("read-callback-{id}"), Some(&token));
                    }
                    self.reader_depth -= 1;
                    println!("read-exit={id}");
                    self.probe
                        .respond(&token, Value::string("resource"), 0)
                        .expect("respond read");
                }
                "log" => {
                    let token = event.token.expect("log token");
                    let id = self.logs;
                    self.logs += 1;
                    println!("log-enter={id} depth={}", self.logger_depth);
                    save_log(&self.out, id, event.log.expect("log payload"));
                    self.logger_depth += 1;
                    if self.logger_depth == 1
                        && let Some(index) = self.logger_path
                    {
                        self.capture(index, &format!("log-callback-{id}"), Some(&token));
                    }
                    if self.action.starts_with("close") {
                        self.probe
                            .close_from_callback(&token)
                            .expect("callback close");
                    }
                    if self.action == "close-reenter"
                        && let Some(index) = self.post_close_path
                    {
                        self.capture(index, &format!("post-close-callback-{id}"), Some(&token));
                    }
                    let failure = (self.action == "throw")
                        .then(|| "stage37-logger-failure".encode_utf16().collect());
                    self.probe
                        .acknowledge_log(&token, failure, 0)
                        .expect("acknowledge log");
                    self.logger_depth -= 1;
                    if self.action != "throw" {
                        println!("log-exit={id}");
                    }
                }
                kind => panic!("unexpected source event {kind}"),
            }
        }
    }
}
fn main() {
    let args: Vec<_> = std::env::args().collect();
    assert!(
        matches!(args.len(), 4 | 5 | 7),
        "case-directory compact|pretty|COMPACT|PRETTY capture|noop|throw|close|close-reenter [budget [uri module-name]]"
    );
    let out = PathBuf::from(&args[1]);
    let operation = match fs::read_to_string(out.join("operation.txt")) {
        Ok(value) => value,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => "expression".into(),
        Err(error) => panic!("operation input: {error}"),
    };
    assert!(
        matches!(operation.as_str(), "expression" | "output-bytes"),
        "unsupported operation"
    );
    let output = operation == "output-bytes";
    let source = fs::read_to_string(out.join("input.pkl")).expect("source");
    let expressions: Vec<String> = fs::read_to_string(out.join("expressions.txt"))
        .expect("expressions")
        .lines()
        .map(str::to_owned)
        .collect();
    let logger = fs::read_to_string(out.join("logger-callback.txt")).expect("logger callback");
    let reader = fs::read_to_string(out.join("reader-callback.txt")).expect("reader callback");
    let budget = args
        .get(4)
        .map_or(32, |s| s.parse::<u32>().expect("budget"));
    assert!(budget > 0, "budget must be positive");
    let mut paths = if output {
        Vec::new()
    } else {
        expressions.clone()
    };
    let mut add = |path: &str| {
        if path.is_empty() {
            None
        } else {
            let index = paths.len() as u32;
            paths.push(path.to_owned());
            Some(index)
        }
    };
    let logger_path = add(&logger);
    let reader_path = add(&reader);
    let post_close_path = (args[3] == "close-reenter").then(|| add("stable").unwrap());
    let uri = args
        .get(5)
        .map_or("file:///stage37-trace.pkl", String::as_str);
    let name = args.get(6).map_or("stage37-trace", String::as_str);
    let prepared = if output {
        use pkl_runtime::evaluation::frontend::{
            DiagnosticProfile, ModuleEvalOptions, ModuleSourceInput,
        };
        SourceProbe::new_output(
            ModuleSourceInput {
                text: &source,
                uri,
                inferred_name: name,
            },
            &paths,
            &ModuleEvalOptions {
                diagnostic_profile: DiagnosticProfile::ApiIdentity,
                ..Default::default()
            },
            SourceProbe::options(&args[2], args[3] != "noop", false, false).expect("trace options"),
        )
        .map_err(|failure| {
            String::from_utf8(failure.stderr).expect("shared diagnostic renderer emits UTF-8")
        })
    } else {
        SourceProbe::new(
            &source,
            uri,
            name,
            &paths,
            &args[2],
            args[3] != "noop",
            false,
            false,
        )
    };
    let probe = match prepared {
        Ok(probe) => probe,
        Err(error) => {
            for index in 0..expressions.len() {
                save_result(
                    &out,
                    &format!("outer-{index}"),
                    SourceEvent {
                        kind: "error",
                        token: None,
                        uri: None,
                        value: None,
                        bytes: None,
                        error: Some(error.encode_utf16().collect()),
                        error_bytes: None,
                        log: None,
                    },
                );
            }
            write(&out,"architecture.json",b"{\"collections\":0,\"logs\":0,\"reads\":0,\"terminal_slots\":null,\"setup_failed\":true,\"session_created\":false}\n");
            println!("logs=0 reads=0");
            return;
        }
    };
    let mut driver = Driver {
        probe,
        out,
        action: args[3].clone(),
        logger_path,
        reader_path,
        post_close_path,
        logger_depth: 0,
        reader_depth: 0,
        logs: 0,
        reads: 0,
        collections: 0,
        budget,
        output,
    };
    for index in 0..expressions.len() {
        driver.capture(index as u32, &format!("outer-{index}"), None);
    }
    driver.probe.close();
    assert_eq!(driver.probe.heap_slots(), 0);
    write(
        &driver.out,
        "architecture.json",
        format!(
            "{{\"collections\":{},\"logs\":{},\"reads\":{},\"terminal_slots\":0,\"setup_failed\":{}}}\n",
            driver.collections,
            driver.logs,
            driver.reads,
            driver.probe.setup_failed()
        ),
    );
    println!("logs={} reads={}", driver.logs, driver.reads);
}
