//! Interactive and piped `pkl repl` loop driving `pkl_runtime::repl::ReplSession`
//! against `pkl_host_native::NativeHost`.

use crate::ReplCliConfig;
use crate::terminal::{BufReadLineEditor, LineEditor, default_pkl_home_dir, is_dumb_terminal};
use pkl_host_native::{NativeHost, NativeHostExtensions};
use pkl_runtime::host::quote_uri_path;
use pkl_runtime::logging::{FrameTransformer, LogDelivery, LogLevel};
use pkl_runtime::repl::{
    HELP, ParsedReplCommand, ReplCommandKind, ReplPoll, ReplRequest, ReplResponse, ReplSession,
    ReplSessionOptions, WELCOME, examples, format_ambiguous_commands, format_prompt,
    format_unknown_command, highlight_backticks, match_commands,
};
use pkl_runtime::value::PklString;
use std::io::{self, BufRead, IsTerminal, Write};
use std::path::Path;

/// Runs `pkl repl` against real process `stdin`/`stdout`/`stderr`.
pub fn run_cli(config: ReplCliConfig) -> i32 {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let stderr = io::stderr();
    let is_tty = stdin.is_terminal() && stdout.is_terminal();
    let dumb = !is_tty || is_dumb_terminal();

    let mut editor = BufReadLineEditor::new(stdin.lock(), stdout.lock(), is_tty, dumb);
    if let Some(pkl_home) = default_pkl_home_dir() {
        editor = editor.with_history_file(pkl_home.join("repl-history"));
    }
    run_with_editor(config, &mut editor, stderr.lock())
}

/// Runs `pkl repl` against caller-supplied streams (used by tests and embedders).
pub fn run_io<R: BufRead, W: Write, E: Write>(
    config: ReplCliConfig,
    input: R,
    output: W,
    stderr: E,
    is_tty: bool,
) -> i32 {
    let dumb = !is_tty || is_dumb_terminal();
    let mut editor = BufReadLineEditor::new(input, output, is_tty, dumb);
    run_with_editor(config, &mut editor, stderr)
}

fn run_with_editor<L: LineEditor, E: Write>(
    config: ReplCliConfig,
    editor: &mut L,
    mut stderr: E,
) -> i32 {
    let trace_mode = config.host_config.trace_mode;
    let power_assertions = config.host_config.power_assertions;
    let mut ext = NativeHostExtensions::default().with_color(config.color);
    if let Some(ref fmt) = config.output_format {
        ext = ext.with_output_format(fmt.clone());
    }
    let host = match NativeHost::with_extensions(config.host_config, ext) {
        Ok(host) => host,
        Err(err) => {
            let _ = writeln!(stderr, "{err}");
            return 1;
        }
    };

    let working_dir_uri = path_to_dir_uri(&config.working_dir);
    let mut session = ReplSession::new(ReplSessionOptions {
        working_dir_uri,
        color: config.color,
        trace_mode,
        log_delivery: LogDelivery::Host,
        frame_transformer: FrameTransformer::Default,
        power_assertions,
        platform_data: true,
    });
    session.set_reusable_timeouts(true);
    session.set_platform_metadata(Some(pkl_host_native::detect_native_platform_metadata()));

    let mut runner = ReplRunner {
        host,
        session,
        stderr,
        color: config.color,
        continuation: false,
        quit: false,
        next_request_id: 0,
    };
    let exit_code = runner.run(editor);
    editor.save_history();
    runner.session.close();
    runner.host.close();
    exit_code
}

struct ReplRunner<E: Write> {
    host: NativeHost,
    session: ReplSession,
    stderr: E,
    color: bool,
    continuation: bool,
    quit: bool,
    next_request_id: usize,
}

impl<E: Write> ReplRunner<E> {
    fn run<L: LineEditor>(&mut self, editor: &mut L) -> i32 {
        if self.println_highlighted(editor, WELCOME).is_err()
            || self.println_highlighted(editor, "").is_err()
        {
            return 1;
        }

        let mut input_buffer = String::new();
        let faint_prompt = editor.supports_faint_prompt(self.color);

        while !self.quit {
            let prompt = format_prompt(self.next_request_id, self.continuation, faint_prompt);

            let line = match editor.read_line(&prompt) {
                Ok(Some(line)) => line,
                Ok(None) => {
                    // Clean EOF (`^D` or closed pipe): mirrors `Repl.kt:113-115` (`":quit"`).
                    self.quit = true;
                    break;
                }
                Err(err) => {
                    let _ = writeln!(self.stderr, "I/O error reading REPL input: {err}");
                    return 1;
                }
            };

            let input = line.trim();
            if input.is_empty() {
                continue;
            }

            if self.continuation {
                input_buffer = format!("{input_buffer}\n{input}").trim().to_string();
                self.continuation = false;
            } else {
                input_buffer = input.to_string();
            }

            let status = if input_buffer.starts_with(':') {
                self.execute_command(editor, &input_buffer)
            } else {
                self.evaluate(editor, &input_buffer)
            };
            if let Err(code) = status {
                return code;
            }
        }
        0
    }

    fn execute_command<L: LineEditor>(&mut self, editor: &mut L, input: &str) -> Result<(), i32> {
        let candidates = match_commands(input);
        match candidates.as_slice() {
            [] => {
                let msg = format_unknown_command(input);
                self.println_highlighted(editor, &msg).map_err(|_| 1)
            }
            [single] => self.do_execute_command(editor, single),
            many => {
                let msg = format_ambiguous_commands(many);
                self.println_highlighted(editor, &msg).map_err(|_| 1)
            }
        }
    }

    fn do_execute_command<L: LineEditor>(
        &mut self,
        editor: &mut L,
        cmd: &ParsedReplCommand,
    ) -> Result<(), i32> {
        match cmd.kind {
            ReplCommandKind::Clear => editor.clear_screen().map_err(|_| 1),
            ReplCommandKind::Examples => {
                let text = examples(cfg!(target_os = "macos"));
                self.println_highlighted(editor, &text).map_err(|_| 1)
            }
            ReplCommandKind::Force => {
                let id = self.take_request_id();
                self.handle_eval_request(
                    editor,
                    ReplRequest::Eval {
                        id,
                        text: cmd.arg.clone(),
                        eval_definitions: false,
                        force_results: true,
                    },
                )
            }
            ReplCommandKind::Help => self.println_highlighted(editor, HELP).map_err(|_| 1),
            ReplCommandKind::Load => {
                let id = self.take_request_id();
                let uri = arg_to_uri(&cmd.arg);
                self.handle_eval_request(editor, ReplRequest::Load { id, uri })
            }
            ReplCommandKind::Quit => {
                self.quit = true;
                Ok(())
            }
            ReplCommandKind::Reset => {
                let id = self.take_request_id();
                let responses = self.drive_request(ReplRequest::Reset { id });
                let mut had_error = false;
                for response in responses {
                    match response {
                        ReplResponse::EvalError { message } => {
                            had_error = true;
                            self.println_highlighted(editor, &message).map_err(|_| 1)?;
                        }
                        ReplResponse::InternalError { message } => {
                            let _ = writeln!(self.stderr, "Internal REPL error: {message}");
                            return Err(1);
                        }
                        ReplResponse::InvalidRequest { message } => {
                            let _ = writeln!(self.stderr, "Invalid REPL request: {message}");
                            return Err(1);
                        }
                        _ => {}
                    }
                }
                if !had_error {
                    editor.clear_screen().map_err(|_| 1)?;
                    self.next_request_id = 0;
                }
                Ok(())
            }
        }
    }

    fn evaluate<L: LineEditor>(&mut self, editor: &mut L, input: &str) -> Result<(), i32> {
        let id = self.take_request_id();
        self.handle_eval_request(
            editor,
            ReplRequest::Eval {
                id,
                text: input.to_string(),
                eval_definitions: false,
                force_results: false,
            },
        )
    }

    fn handle_eval_request<L: LineEditor>(
        &mut self,
        editor: &mut L,
        request: ReplRequest,
    ) -> Result<(), i32> {
        let responses = self.drive_request(request);
        for response in responses {
            match response {
                ReplResponse::EvalSuccess { result } => {
                    self.println_highlighted(editor, &result).map_err(|_| 1)?;
                }
                ReplResponse::EvalError { message } => {
                    self.println_highlighted(editor, &message).map_err(|_| 1)?;
                }
                ReplResponse::IncompleteInput => {
                    self.next_request_id = self.next_request_id.saturating_sub(1);
                    self.continuation = true;
                }
                ReplResponse::InternalError { message } => {
                    let _ = writeln!(self.stderr, "Internal REPL error: {message}");
                    return Err(1);
                }
                ReplResponse::InvalidRequest { message } => {
                    let _ = writeln!(self.stderr, "Invalid REPL request: {message}");
                    return Err(1);
                }
                ReplResponse::Completion { .. } => {}
            }
        }
        Ok(())
    }

    fn drive_request(&mut self, request: ReplRequest) -> Vec<ReplResponse> {
        let now_ms = self.host.clock_ms();
        let timeout_ms = self.host.timeout_ms();
        let timeout_guard = self.host.start_eval_timeout_guard(timeout_ms);
        let timeout_response = |ms: Option<u64>| {
            vec![ReplResponse::EvalError {
                message: format!(
                    "–– Pkl Error ––\n{}\n",
                    pkl_runtime::host::format_timeout_message(ms.unwrap_or(0))
                ),
            }]
        };
        let map_host_err = |session: &mut ReplSession,
                            err: pkl_runtime::value::RuntimeError,
                            timed_out: bool,
                            ms: Option<u64>| {
            session.abort_active_operation();
            if timed_out || err.kind == pkl_runtime::value::ErrorKind::Timeout {
                timeout_response(ms)
            } else {
                vec![ReplResponse::InternalError {
                    message: err.message.to_java_utf8(),
                }]
            }
        };
        if let Err(err) = self.session.start_request(request, now_ms, timeout_ms) {
            let timed_out = timeout_guard.stop();
            return map_host_err(&mut self.session, err, timed_out, timeout_ms);
        }
        let step_budget = if timeout_ms.is_some() { 4096 } else { 32_768 };
        loop {
            if timeout_guard.triggered() {
                self.session.abort_active_operation();
                let _ = timeout_guard.stop();
                return timeout_response(timeout_ms);
            }
            let t = self.host.clock_ms();
            match self.session.poll(t, step_budget) {
                ReplPoll::Ready(responses) => {
                    let _ = timeout_guard.stop();
                    return responses;
                }
                ReplPoll::Yielded => {}
                ReplPoll::Read { token, .. } => match self.session.read_request(&token) {
                    Ok(req) => {
                        let result = self.host.handle_read_request(&req);
                        if timeout_guard.triggered() {
                            self.session.abort_active_operation();
                            let _ = timeout_guard.stop();
                            return timeout_response(timeout_ms);
                        }
                        if let Err(err) =
                            self.session
                                .complete_read(&token, result, self.host.clock_ms())
                        {
                            let timed_out = timeout_guard.stop();
                            return map_host_err(&mut self.session, err, timed_out, timeout_ms);
                        }
                    }
                    Err(err) => {
                        let timed_out = timeout_guard.stop();
                        return map_host_err(&mut self.session, err, timed_out, timeout_ms);
                    }
                },
                ReplPoll::Module { token, request } => {
                    let completion = self.host.handle_module_request(&request);
                    if timeout_guard.triggered() {
                        self.session.abort_active_operation();
                        let _ = timeout_guard.stop();
                        return timeout_response(timeout_ms);
                    }
                    if let Err(err) =
                        self.session
                            .complete_module(&token, completion, self.host.clock_ms())
                    {
                        let timed_out = timeout_guard.stop();
                        return map_host_err(&mut self.session, err, timed_out, timeout_ms);
                    }
                }
                ReplPoll::Glob { token, request } => {
                    let completion = self.host.handle_glob_request(&request);
                    if timeout_guard.triggered() {
                        self.session.abort_active_operation();
                        let _ = timeout_guard.stop();
                        return timeout_response(timeout_ms);
                    }
                    if let Err(err) =
                        self.session
                            .complete_glob(&token, completion, self.host.clock_ms())
                    {
                        let timed_out = timeout_guard.stop();
                        return map_host_err(&mut self.session, err, timed_out, timeout_ms);
                    }
                }
                ReplPoll::Log { token, event } => {
                    let level = match event.level {
                        LogLevel::Trace => "TRACE",
                        LogLevel::Warn => "WARN",
                    };
                    let uri = event.frame.module_uri.to_java_utf8();
                    let message = event.message.to_java_utf8();
                    let space = if message.ends_with('\n') { "" } else { " " };
                    let _ = writeln!(self.stderr, "pkl: {level}: {message}{space}({uri})");
                    let _ = self.stderr.flush();
                    if timeout_guard.triggered() {
                        self.session.abort_active_operation();
                        let _ = timeout_guard.stop();
                        return timeout_response(timeout_ms);
                    }
                    if let Err(err) =
                        self.session
                            .acknowledge_log(&token, Ok(()), self.host.clock_ms())
                    {
                        let timed_out = timeout_guard.stop();
                        return map_host_err(&mut self.session, err, timed_out, timeout_ms);
                    }
                }
                ReplPoll::TransformFrame { token, frame } => {
                    let transformed = FrameTransformer::Default.transform((*frame).clone());
                    if timeout_guard.triggered() {
                        self.session.abort_active_operation();
                        let _ = timeout_guard.stop();
                        return timeout_response(timeout_ms);
                    }
                    if let Err(err) = self.session.complete_frame_transform(
                        &token,
                        Ok(transformed),
                        self.host.clock_ms(),
                    ) {
                        let timed_out = timeout_guard.stop();
                        return map_host_err(&mut self.session, err, timed_out, timeout_ms);
                    }
                }
            }
        }
    }

    fn take_request_id(&mut self) -> String {
        let id = format!("pkl{}", self.next_request_id);
        self.next_request_id += 1;
        id
    }

    fn println_highlighted<L: LineEditor>(&self, editor: &mut L, text: &str) -> io::Result<()> {
        let highlighted = highlight_backticks(text);
        editor.write_out(&highlighted)?;
        editor.write_out("\n")?;
        editor.flush_out()
    }
}

/// Converts a filesystem working directory path to a `file:///.../` URI with trailing slash.
fn path_to_dir_uri(dir: &Path) -> PklString {
    let abs = if dir.is_absolute() {
        dir.to_path_buf()
    } else {
        std::env::current_dir()
            .map(|cwd| cwd.join(dir))
            .unwrap_or_else(|_| dir.to_path_buf())
    };
    let mut uri = pkl_host_native::path_to_file_uri(&abs).to_java_utf8();
    if !uri.ends_with('/') {
        uri.push('/');
    }
    PklString::from(uri)
}

/// Converts a `:load <arg>` argument into a URI string matching `IoUtils.toUri(String)`.
fn arg_to_uri(raw: &str) -> String {
    let trimmed = raw.trim();
    if pkl_host_native::is_uri_like(trimmed) {
        return trimmed.to_string();
    }
    let path = Path::new(trimmed);
    let b = trimmed.as_bytes();
    let is_windows_drive_abs = b.len() >= 3
        && b[0].is_ascii_alphabetic()
        && b[1] == b':'
        && (b[2] == b'\\' || b[2] == b'/');
    if path.is_absolute() || is_windows_drive_abs {
        return pkl_host_native::path_to_file_uri(path).to_java_utf8();
    }
    if cfg!(windows) && b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':' {
        let abs = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
        return pkl_host_native::path_to_file_uri(&abs).to_java_utf8();
    }
    let slash_path = if cfg!(windows) {
        trimmed.replace('\\', "/")
    } else {
        trimmed.to_string()
    };
    quote_uri_path(&PklString::from(slash_path.as_str())).to_java_utf8()
}
