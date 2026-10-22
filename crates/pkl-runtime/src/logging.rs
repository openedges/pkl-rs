//! Shared logger data. Messages and locations retain Java UTF-16 code units.
use crate::value::{PklString, RuntimeSource};
use std::sync::Arc;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TraceMode {
    #[default]
    Compact,
    Pretty,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LogDelivery {
    #[default]
    Noop,
    Host,
}

#[derive(Clone, Debug, Default)]
pub struct EvaluationContext {
    pub primary_source: Option<Arc<RuntimeSource>>,
    pub trace_mode: TraceMode,
    pub log_delivery: LogDelivery,
    /// A custom frame transformer runs even when the logger sink is a no-op.
    pub host_frame_transformer: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogLevel {
    Trace,
    Warn,
}

/// An operand source section, independent of diagnostic stack-frame rendering.
/// Lines and columns are one-based; end columns include the final UTF-16 unit.
/// Source lines retain their contents without physical line terminators.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogFrame {
    pub module_uri: PklString,
    pub member_name: Option<PklString>,
    pub source_lines: Arc<[PklString]>,
    pub start_line: usize,
    pub start_column: usize,
    pub end_line: usize,
    pub end_column: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogEvent {
    pub level: LogLevel,
    pub message: PklString,
    pub frame: LogFrame,
}

/// A Host callback failure, not a language undefined-value exception.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoggerFailure {
    pub message: PklString,
}
impl LoggerFailure {
    pub fn new(message: impl Into<PklString>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogRequest {
    pub id: u64,
    pub event: Arc<LogEvent>,
}
