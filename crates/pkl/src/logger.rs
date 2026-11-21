//! Logging SPI mirroring `org.pkl.core.{Logger, Loggers, BufferedLogger}`.

use crate::frame::StackFrame;
use pkl_host_native::HostLogger;
use pkl_runtime::logging::{LogFrame, LogLevel};
use pkl_runtime::value::PklString;
use std::io::Write;
use std::sync::{Arc, Mutex};

/// Logger SPI receiving `trace()` and `warn()` messages during evaluation (`org.pkl.core.Logger`).
pub trait Logger: Send + Sync {
    fn trace(&self, message: &str, frame: &StackFrame);
    fn warn(&self, message: &str, frame: &StackFrame);
}

struct NoopLogger;

impl Logger for NoopLogger {
    fn trace(&self, _message: &str, _frame: &StackFrame) {}
    fn warn(&self, _message: &str, _frame: &StackFrame) {}
}

struct StdErrLogger;

impl Logger for StdErrLogger {
    fn trace(&self, message: &str, frame: &StackFrame) {
        let formatted = Loggers::format_log_message("TRACE", message, frame);
        let mut stderr = std::io::stderr().lock();
        let _ = stderr.write_all(formatted.as_bytes());
        let _ = stderr.flush();
    }

    fn warn(&self, message: &str, frame: &StackFrame) {
        let formatted = Loggers::format_log_message("WARN", message, frame);
        let mut stderr = std::io::stderr().lock();
        let _ = stderr.write_all(formatted.as_bytes());
        let _ = stderr.flush();
    }
}

/// Captures raw log messages (newline-delimited) while optionally forwarding to a delegate
/// logger (`org.pkl.core.BufferedLogger`, used by `pkl test` JUnit `<system-err>`).
pub struct BufferedLogger {
    delegate: Arc<dyn Logger>,
    buffer: Mutex<String>,
}

impl BufferedLogger {
    pub fn new(delegate: Arc<dyn Logger>) -> Self {
        Self {
            delegate,
            buffer: Mutex::new(String::new()),
        }
    }

    /// Returns all captured log messages (`BufferedLogger.getLogs()`).
    pub fn logs(&self) -> String {
        self.buffer
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    /// Clears captured logs without touching the delegate.
    pub fn clear(&self) {
        self.buffer
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clear();
    }
}

impl Logger for BufferedLogger {
    fn trace(&self, message: &str, frame: &StackFrame) {
        {
            let mut buf = self.buffer.lock().unwrap_or_else(|e| e.into_inner());
            buf.push_str(message);
            buf.push('\n');
        }
        self.delegate.trace(message, frame);
    }

    fn warn(&self, message: &str, frame: &StackFrame) {
        {
            let mut buf = self.buffer.lock().unwrap_or_else(|e| e.into_inner());
            buf.push_str(message);
            buf.push('\n');
        }
        self.delegate.warn(message, frame);
    }
}

/// Factory helpers mirroring `org.pkl.core.Loggers`.
pub struct Loggers;

impl Loggers {
    pub fn noop() -> Arc<dyn Logger> {
        Arc::new(NoopLogger)
    }

    pub fn std_err() -> Arc<dyn Logger> {
        Arc::new(StdErrLogger)
    }

    pub fn buffered(delegate: Arc<dyn Logger>) -> Arc<BufferedLogger> {
        Arc::new(BufferedLogger::new(delegate))
    }

    /// Formats a standard Pkl log line (`pkl: TRACE: <msg> (<uri>)\n`).
    pub fn format_log_message(level: &str, message: &str, frame: &StackFrame) -> String {
        let space = if message.ends_with('\n') { "" } else { " " };
        format!("pkl: {level}: {message}{space}({})\n", frame.module_uri())
    }
}

pub(crate) struct HostLoggerAdapter(pub(crate) Arc<dyn Logger>);

impl HostLogger for HostLoggerAdapter {
    fn log(&self, level: LogLevel, message: &PklString, frame: &LogFrame) {
        let sf = StackFrame::from_log_frame(frame);
        let msg = message.to_java_utf8();
        match level {
            LogLevel::Trace => self.0.trace(&msg, &sf),
            LogLevel::Warn => self.0.warn(&msg, &sf),
        }
    }
}
