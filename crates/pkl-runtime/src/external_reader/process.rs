//! Native OS subprocess runner for Pkl 0.32.0 external module and resource readers.
//!
//! Strictly gated behind `#[cfg(not(target_arch = "wasm32"))]` with zero `unsafe`.
//! Mirrors `org.pkl.core.externalreader.ExternalReaderProcessImpl`:
//! - Lazy subprocess spawn on first `ensure_alive()` / `request()`.
//! - Stdio piping: stdin/stdout carry raw concatenated MessagePack `[code, map]`
//!   frames; stderr is inherited (`Stdio::inherit()`) and environment is inherited.
//! - Optional `working_dir` (when `None`, inherits the current process working dir).
//! - Debug logging to stderr when `PKL_DEBUG=1`.
//! - Deadlock-free background reader thread with immediate failure on unexpected
//!   child exit (EOF), malformed wire frame, cancellation, or request deadline.
//! - Idempotent `close()` & `Drop`: sends `CloseExternalProcess` (`0x32`), waits up
//!   to `close_timeout` (default **3 seconds**, matching `CLOSE_TIMEOUT`), then
//!   force-kills and reaps the child so zero orphan/zombie processes remain.
//! - Prompt `cancel()`: fails all pending requests immediately with `Cancelled`,
//!   force-kills and reaps the child without waiting for graceful `close_timeout`.

use super::codec;
use super::messages::{ExternalReaderConfig, ExternalReaderError, ExternalReaderMessage};
use super::transport::ExternalReaderTransport;
use std::collections::HashMap;
use std::ffi::OsStr;
use std::io::{BufReader, BufWriter};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::{
    Arc, Condvar, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

/// Default graceful-shutdown timeout matching `ExternalReaderProcessImpl.CLOSE_TIMEOUT`.
pub const DEFAULT_CLOSE_TIMEOUT: Duration = Duration::from_secs(3);

type PendingResult = Result<ExternalReaderMessage, ExternalReaderError>;

struct PendingSlot {
    state: Mutex<Option<PendingResult>>,
    ready: Condvar,
}

impl PendingSlot {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            state: Mutex::new(None),
            ready: Condvar::new(),
        })
    }

    fn complete(&self, result: PendingResult) {
        let mut guard = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if guard.is_none() {
            *guard = Some(result);
            self.ready.notify_all();
        }
    }

    fn wait(&self, deadline: Option<Instant>) -> PendingResult {
        let mut guard = self.state.lock().unwrap_or_else(|e| e.into_inner());
        loop {
            if let Some(res) = guard.take() {
                return res;
            }
            match deadline {
                None => {
                    guard = self.ready.wait(guard).unwrap_or_else(|e| e.into_inner());
                }
                Some(dl) => {
                    let now = Instant::now();
                    if now >= dl {
                        return Err(ExternalReaderError::Timeout);
                    }
                    let (next_guard, wait_res) = self
                        .ready
                        .wait_timeout(guard, dl.saturating_duration_since(now))
                        .unwrap_or_else(|e| e.into_inner());
                    guard = next_guard;
                    if let Some(res) = guard.take() {
                        return res;
                    }
                    if wait_res.timed_out() {
                        return Err(ExternalReaderError::Timeout);
                    }
                }
            }
        }
    }
}

struct SharedState {
    pending: Mutex<HashMap<i64, Arc<PendingSlot>>>,
    terminated: AtomicBool,
    terminal_error: Mutex<Option<ExternalReaderError>>,
    log_prefix: Option<String>,
}

impl SharedState {
    fn log(&self, msg: &str) {
        if let Some(prefix) = &self.log_prefix {
            eprintln!("{prefix}{msg}");
        }
    }

    fn fail_all(&self, error: ExternalReaderError) {
        self.terminated.store(true, Ordering::SeqCst);
        {
            let mut term = self
                .terminal_error
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            if term.is_none() {
                *term = Some(error.clone());
            }
        }
        let slots: Vec<Arc<PendingSlot>> = {
            let mut map = self.pending.lock().unwrap_or_else(|e| e.into_inner());
            map.drain().map(|(_, slot)| slot).collect()
        };
        for slot in slots {
            slot.complete(Err(error.clone()));
        }
    }
}

struct SpawnedProcess {
    child: Child,
    stdin: Arc<Mutex<Option<BufWriter<ChildStdin>>>>,
    rx_thread: Option<JoinHandle<()>>,
}

/// Native OS child-process implementation of [`ExternalReaderTransport`].
pub struct NativeExternalReaderProcess {
    config: ExternalReaderConfig,
    close_timeout: Duration,
    request_timeout: Option<Duration>,
    env_overrides: Vec<(String, String)>,
    closed: AtomicBool,
    cancelled: AtomicBool,
    shared: Arc<SharedState>,
    process: Mutex<Option<SpawnedProcess>>,
}

impl NativeExternalReaderProcess {
    /// Construct a lazily-spawned native external reader process handle.
    ///
    /// Allocates zero OS process resources until the first `ensure_alive()` or
    /// `request()` call (`ExternalReaderProcess.java:24-30`).
    pub fn new(config: ExternalReaderConfig) -> Self {
        Self::with_timeouts(config, DEFAULT_CLOSE_TIMEOUT, None)
    }

    /// Construct with explicit close and per-request timeouts (useful for tests
    /// and evaluator-level timeout enforcement).
    pub fn with_timeouts(
        config: ExternalReaderConfig,
        close_timeout: Duration,
        request_timeout: Option<Duration>,
    ) -> Self {
        let log_prefix = if std::env::var_os("PKL_DEBUG").as_deref() == Some(OsStr::new("1")) {
            Some(format!(
                "[pkl-core][external-process][{}] ",
                config.executable
            ))
        } else {
            None
        };
        Self {
            config,
            close_timeout,
            request_timeout,
            env_overrides: Vec::new(),
            closed: AtomicBool::new(false),
            cancelled: AtomicBool::new(false),
            shared: Arc::new(SharedState {
                pending: Mutex::new(HashMap::new()),
                terminated: AtomicBool::new(false),
                terminal_error: Mutex::new(None),
                log_prefix,
            }),
            process: Mutex::new(None),
        }
    }

    /// Override a specific child environment variable (for example `PATH` in
    /// tests without mutating the parent process environment).
    pub fn with_env_override(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env_overrides.push((key.into(), value.into()));
        self
    }

    pub fn config(&self) -> &ExternalReaderConfig {
        &self.config
    }

    /// Returns the OS PID if the child process has been spawned and not yet cleared.
    pub fn pid(&self) -> Option<u32> {
        self.process
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_ref()
            .map(|p| p.child.id())
    }

    /// Returns `true` if the child process was ever spawned.
    pub fn was_spawned(&self) -> bool {
        self.process
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .is_some()
    }

    /// Ensures the child process is spawned and alive, returning a clone of its
    /// serialized stdin writer.
    fn get_stdin(&self) -> Result<Arc<Mutex<Option<BufWriter<ChildStdin>>>>, ExternalReaderError> {
        let mut guard = self.process.lock().unwrap_or_else(|e| e.into_inner());
        if self.cancelled.load(Ordering::SeqCst) {
            return Err(ExternalReaderError::Cancelled);
        }
        if self.closed.load(Ordering::SeqCst) {
            return Err(ExternalReaderError::Closed);
        }

        if let Some(spawned) = guard.as_mut() {
            let exited = matches!(spawned.child.try_wait(), Ok(Some(_)))
                || self.shared.terminated.load(Ordering::SeqCst);
            if exited {
                if let Some(err) = self
                    .shared
                    .terminal_error
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .clone()
                {
                    return Err(err);
                }
                return Err(ExternalReaderError::already_terminated());
            }
            return Ok(Arc::clone(&spawned.stdin));
        }

        // Lazy spawn: relies on OS PATH resolution, absolute/relative paths,
        // inherited environment, and inherited stderr (`ExternalReaderProcessImpl.java:101-118`).
        let mut cmd = Command::new(&self.config.executable);
        cmd.args(&self.config.arguments);
        if let Some(dir) = &self.config.working_dir {
            cmd.current_dir(dir);
        }
        for (key, value) in &self.env_overrides {
            cmd.env(key, value);
        }
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());

        let mut child = cmd.spawn().map_err(|io_err| {
            ExternalReaderError::Process(format!(
                "java.io.IOException: Cannot run program \"{}\": {}",
                self.config.executable, io_err
            ))
        })?;

        let child_stdin = child
            .stdin
            .take()
            .ok_or_else(|| ExternalReaderError::Process("Failed to capture child stdin.".into()))?;
        let child_stdout = child.stdout.take().ok_or_else(|| {
            ExternalReaderError::Process("Failed to capture child stdout.".into())
        })?;

        let stdin = Arc::new(Mutex::new(Some(BufWriter::new(child_stdin))));
        let shared_rx = Arc::clone(&self.shared);
        let thread_name = format!("pkl-ext-reader-rx({})", self.config.executable);
        let rx_thread = thread::Builder::new()
            .name(thread_name)
            .spawn(move || run_rx_loop(child_stdout, shared_rx))
            .map_err(|io_err| {
                let _ = child.kill();
                let _ = child.wait();
                ExternalReaderError::Process(format!(
                    "Failed to spawn external reader rx thread: {io_err}"
                ))
            })?;

        *guard = Some(SpawnedProcess {
            child,
            stdin: Arc::clone(&stdin),
            rx_thread: Some(rx_thread),
        });
        Ok(stdin)
    }

    fn shutdown_process(&self, grace: Duration) {
        let mut guard = self.process.lock().unwrap_or_else(|e| e.into_inner());
        let Some(spawned) = guard.as_mut() else {
            return;
        };

        let alive = matches!(spawned.child.try_wait(), Ok(None));
        if alive {
            // Send graceful `CloseExternalProcess` (`0x32`) and close child stdin.
            {
                let mut stdin_guard = spawned.stdin.lock().unwrap_or_else(|e| e.into_inner());
                if let Some(mut writer) = stdin_guard.take() {
                    self.shared.log("Sending message: CloseExternalProcess[]");
                    let _ = codec::write_message(
                        &mut writer,
                        &ExternalReaderMessage::CloseExternalProcess,
                    );
                }
            }

            let deadline = Instant::now() + grace;
            let mut poll_sleep = Duration::from_millis(2);
            loop {
                match spawned.child.try_wait() {
                    Ok(Some(_)) => break,
                    Ok(None) => {
                        if self.cancelled.load(Ordering::SeqCst) {
                            break;
                        }
                        let now = Instant::now();
                        if now >= deadline {
                            break;
                        }
                        let remaining = deadline.saturating_duration_since(now);
                        thread::sleep(poll_sleep.min(remaining));
                        poll_sleep = (poll_sleep * 2).min(Duration::from_millis(25));
                    }
                    Err(_) => break,
                }
            }
        } else {
            // Drop stdin pipe if still held.
            let mut stdin_guard = spawned.stdin.lock().unwrap_or_else(|e| e.into_inner());
            let _ = stdin_guard.take();
        }

        // Unconditionally force-kill if still alive and reap child (no zombies/orphans).
        if matches!(spawned.child.try_wait(), Ok(None)) {
            let _ = spawned.child.kill();
        }
        let _ = spawned.child.wait();

        if let Some(handle) = spawned.rx_thread.take() {
            let _ = handle.join();
        }
    }

    /// Force-kill and reap the child process immediately without sending
    /// `CloseExternalProcess` or waiting for `close_timeout`.
    fn terminate_immediately(&self) {
        let mut guard = self.process.lock().unwrap_or_else(|e| e.into_inner());
        let Some(spawned) = guard.as_mut() else {
            return;
        };

        // Kill first so any blocked stdin writer or stdout reader unblocks immediately.
        if matches!(spawned.child.try_wait(), Ok(None)) {
            let _ = spawned.child.kill();
        }
        let _ = spawned.child.wait();

        {
            let mut stdin_guard = spawned.stdin.lock().unwrap_or_else(|e| e.into_inner());
            let _ = stdin_guard.take();
        }

        if let Some(handle) = spawned.rx_thread.take() {
            let _ = handle.join();
        }
    }
}

fn run_rx_loop(stdout: std::process::ChildStdout, shared: Arc<SharedState>) {
    let mut reader = BufReader::new(stdout);
    loop {
        match codec::read_message(&mut reader) {
            Ok(Some(msg)) => {
                shared.log(&format!("Received message: {msg:?}"));
                if msg.is_one_way() {
                    shared.fail_all(ExternalReaderError::Protocol(format!(
                        "Unexpected incoming one-way message: {msg:?}"
                    )));
                    return;
                }
                if msg.is_request() {
                    shared.fail_all(ExternalReaderError::Protocol(format!(
                        "Unexpected incoming request message: {msg:?}"
                    )));
                    return;
                }
                if let Some(req_id) = msg.request_id() {
                    let slot = {
                        let mut map = shared.pending.lock().unwrap_or_else(|e| e.into_inner());
                        map.remove(&req_id)
                    };
                    match slot {
                        Some(slot) => slot.complete(Ok(msg)),
                        None => {
                            shared.fail_all(ExternalReaderError::unknown_request_id(
                                msg.java_simple_name(),
                                req_id,
                            ));
                            return;
                        }
                    }
                }
            }
            Ok(None) => {
                // Child closed stdout / exited. Fail any in-flight requests immediately.
                shared.fail_all(ExternalReaderError::already_terminated());
                return;
            }
            Err(err) => {
                shared.fail_all(err);
                return;
            }
        }
    }
}

impl ExternalReaderTransport for NativeExternalReaderProcess {
    fn ensure_alive(&self) -> Result<(), ExternalReaderError> {
        let _ = self.get_stdin()?;
        Ok(())
    }

    fn request(
        &self,
        message: ExternalReaderMessage,
    ) -> Result<ExternalReaderMessage, ExternalReaderError> {
        let request_id = message.request_id().ok_or_else(|| {
            ExternalReaderError::Protocol("Expected request message with requestId".into())
        })?;
        let stdin = self.get_stdin()?;

        // Register pending slot BEFORE writing to stdin to avoid response races.
        let slot = PendingSlot::new();
        {
            let mut map = self
                .shared
                .pending
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            if self.cancelled.load(Ordering::SeqCst) {
                return Err(ExternalReaderError::Cancelled);
            }
            if self.closed.load(Ordering::SeqCst) {
                return Err(ExternalReaderError::Closed);
            }
            if self.shared.terminated.load(Ordering::SeqCst) {
                let err = self
                    .shared
                    .terminal_error
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .clone()
                    .unwrap_or_else(ExternalReaderError::already_terminated);
                return Err(err);
            }
            if map.contains_key(&request_id) {
                return Err(ExternalReaderError::Protocol(format!(
                    "Duplicate in-flight request ID `{request_id}`."
                )));
            }
            map.insert(request_id, Arc::clone(&slot));
        }

        // Write and flush MessagePack frame under the stdin serializer lock.
        let write_result = {
            let mut writer_guard = stdin.lock().unwrap_or_else(|e| e.into_inner());
            match writer_guard.as_mut() {
                Some(writer) => {
                    self.shared.log(&format!("Sending message: {message:?}"));
                    codec::write_message(writer, &message)
                }
                None => {
                    if self.cancelled.load(Ordering::SeqCst) {
                        Err(ExternalReaderError::Cancelled)
                    } else {
                        Err(ExternalReaderError::Closed)
                    }
                }
            }
        };

        if let Err(err) = write_result {
            self.shared
                .pending
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .remove(&request_id);
            if self.cancelled.load(Ordering::SeqCst) {
                return Err(ExternalReaderError::Cancelled);
            }
            return Err(err);
        }

        let deadline = self.request_timeout.map(|t| Instant::now() + t);
        let result = slot.wait(deadline);
        if matches!(result, Err(ExternalReaderError::Timeout)) {
            self.shared
                .pending
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .remove(&request_id);
        }
        result
    }

    fn send_one_way(&self, message: ExternalReaderMessage) -> Result<(), ExternalReaderError> {
        let stdin = self.get_stdin()?;
        let mut writer_guard = stdin.lock().unwrap_or_else(|e| e.into_inner());
        match writer_guard.as_mut() {
            Some(writer) => {
                self.shared.log(&format!("Sending message: {message:?}"));
                codec::write_message(writer, &message)
            }
            None => Err(ExternalReaderError::Closed),
        }
    }

    fn close(&self) {
        if self
            .closed
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
        {
            self.shared.fail_all(ExternalReaderError::Closed);
            self.shutdown_process(self.close_timeout);
        }
    }

    fn is_closed(&self) -> bool {
        self.closed.load(Ordering::SeqCst)
    }

    fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
        self.shared.fail_all(ExternalReaderError::Cancelled);
        if self
            .closed
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
        {
            self.terminate_immediately();
        }
    }
}

impl Drop for NativeExternalReaderProcess {
    fn drop(&mut self) {
        self.close();
    }
}
