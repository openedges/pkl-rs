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

pub type MonotonicClock = Arc<dyn Fn() -> u128 + Send + Sync>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlatformMetadata {
    pub os_name: PklString,
    pub os_version: PklString,
    pub cpu_architecture: PklString,
    pub java_version: PklString,
}

impl Default for PlatformMetadata {
    fn default() -> Self {
        Self {
            os_name: "macOS".into(),
            os_version: "27.2".into(),
            cpu_architecture: "aarch64".into(),
            java_version: "21.0.13".into(),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct EvaluationContext {
    pub primary_source: Option<Arc<RuntimeSource>>,
    pub trace_mode: TraceMode,
    pub log_delivery: LogDelivery,
    /// Independent of the logger sink; Host runs even for a no-op logger.
    pub frame_transformer: FrameTransformer,
    pub platform_metadata: Option<PlatformMetadata>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogLevel {
    Trace,
    Warn,
}

/// A complete raw or transformed frame shared by logging and diagnostics.
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

/// Stack frame policy, applied to raw authored locations at each public use.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FrameTransformer {
    #[default]
    Default,
    Identity,
    Host,
}

impl FrameTransformer {
    /// Pinned standard-library part of StackFrameTransformers.defaultTransformer.
    /// Package metadata and service-provider transformations remain Host capabilities.
    pub fn transform(self, mut frame: LogFrame) -> LogFrame {
        if self == Self::Default {
            let units: Vec<_> = frame.module_uri.encode_utf16().collect();
            if let Some(module) = units.strip_prefix(&[112, 107, 108, 58]) {
                let mut uri: Vec<_> = "https://github.com/apple/pkl/blob/0.32.0/stdlib/"
                    .encode_utf16()
                    .collect();
                uri.extend_from_slice(module);
                uri.extend(format!(".pkl#L{}", frame.start_line).encode_utf16());
                frame.module_uri = PklString::from_utf16(uri);
            }
        }
        frame
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FrameTransformRequest {
    pub id: u64,
    pub frame: Arc<LogFrame>,
}

/// The actual runtime producer of a failed callback path, without assigning a
/// foreign exception class or treating unrelated Host failures as callbacks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CallbackOrigin {
    LogTransformer,
    LogDelegate,
    DiagnosticTransformer,
    DiagnosticRenderer,
    ReflectionTransformer,
}
impl CallbackOrigin {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::LogTransformer => "log-transformer",
            Self::LogDelegate => "log-delegate",
            Self::DiagnosticTransformer => "diagnostic-transformer",
            Self::DiagnosticRenderer => "diagnostic-renderer",
            Self::ReflectionTransformer => "reflection-transformer",
        }
    }
}
