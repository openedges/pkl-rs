//! Stack frames and stack-frame transformers mirroring
//! `org.pkl.core.{StackFrame, StackFrameTransformer, StackFrameTransformers}`.

use pkl_host_native::HostFrameTransformer;
use pkl_runtime::logging::{FrameTransformer, LogFrame};
use pkl_runtime::value::PklString;
use std::sync::Arc;

/// A single Pkl stack frame (`org.pkl.core.StackFrame`).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct StackFrame {
    module_uri: String,
    member_name: Option<String>,
    source_lines: Vec<String>,
    start_line: usize,
    start_column: usize,
    end_line: usize,
    end_column: usize,
}

impl StackFrame {
    pub fn new(
        module_uri: impl Into<String>,
        member_name: Option<String>,
        source_lines: Vec<String>,
        start_line: usize,
        start_column: usize,
        end_line: usize,
        end_column: usize,
    ) -> Self {
        Self {
            module_uri: module_uri.into(),
            member_name,
            source_lines,
            start_line,
            start_column,
            end_line,
            end_column,
        }
    }

    pub fn module_uri(&self) -> &str {
        &self.module_uri
    }

    pub fn member_name(&self) -> Option<&str> {
        self.member_name.as_deref()
    }

    pub fn source_lines(&self) -> &[String] {
        &self.source_lines
    }

    pub fn start_line(&self) -> usize {
        self.start_line
    }

    pub fn start_column(&self) -> usize {
        self.start_column
    }

    pub fn end_line(&self) -> usize {
        self.end_line
    }

    pub fn end_column(&self) -> usize {
        self.end_column
    }

    /// Returns a copy of this frame with a new `module_uri` (`StackFrame.withModuleUri`).
    pub fn with_module_uri(&self, module_uri: impl Into<String>) -> Self {
        let mut copy = self.clone();
        copy.module_uri = module_uri.into();
        copy
    }

    /// Returns a copy of this frame with a new `member_name`.
    pub fn with_member_name(&self, member_name: Option<String>) -> Self {
        let mut copy = self.clone();
        copy.member_name = member_name;
        copy
    }

    pub(crate) fn from_log_frame(frame: &LogFrame) -> Self {
        Self {
            module_uri: frame.module_uri.to_java_utf8(),
            member_name: frame.member_name.as_ref().map(PklString::to_java_utf8),
            source_lines: frame
                .source_lines
                .iter()
                .map(PklString::to_java_utf8)
                .collect(),
            start_line: frame.start_line,
            start_column: frame.start_column,
            end_line: frame.end_line,
            end_column: frame.end_column,
        }
    }

    pub(crate) fn into_log_frame(self) -> LogFrame {
        LogFrame {
            module_uri: PklString::from(self.module_uri),
            member_name: self.member_name.map(PklString::from),
            source_lines: self
                .source_lines
                .into_iter()
                .map(PklString::from)
                .collect::<Vec<_>>()
                .into(),
            start_line: self.start_line,
            start_column: self.start_column,
            end_line: self.end_line,
            end_column: self.end_column,
        }
    }
}

/// Transforms stack frames before rendering diagnostics or delivering log events
/// (`org.pkl.core.StackFrameTransformer`).
pub trait StackFrameTransformer: Send + Sync {
    fn transform(&self, frame: StackFrame) -> StackFrame;

    /// Composes this transformer with `after`, applying `self` first and then `after`.
    fn and_then(self, after: Arc<dyn StackFrameTransformer>) -> Arc<dyn StackFrameTransformer>
    where
        Self: Sized + 'static,
    {
        Arc::new(ChainedTransformer {
            first: Arc::new(self),
            second: after,
        })
    }
}

struct ChainedTransformer {
    first: Arc<dyn StackFrameTransformer>,
    second: Arc<dyn StackFrameTransformer>,
}

impl StackFrameTransformer for ChainedTransformer {
    fn transform(&self, frame: StackFrame) -> StackFrame {
        self.second.transform(self.first.transform(frame))
    }
}

struct FnTransformer<F>(F);

impl<F> StackFrameTransformer for FnTransformer<F>
where
    F: Fn(StackFrame) -> StackFrame + Send + Sync,
{
    fn transform(&self, frame: StackFrame) -> StackFrame {
        (self.0)(frame)
    }
}

struct EmptyTransformer;

impl StackFrameTransformer for EmptyTransformer {
    fn transform(&self, frame: StackFrame) -> StackFrame {
        frame
    }
}

struct DefaultTransformer;

impl StackFrameTransformer for DefaultTransformer {
    fn transform(&self, frame: StackFrame) -> StackFrame {
        let log = FrameTransformer::Default.transform(frame.into_log_frame());
        StackFrame::from_log_frame(&log)
    }
}

/// Built-in stack-frame transformers mirroring `org.pkl.core.StackFrameTransformers`.
#[derive(Clone, Copy, Debug, Default)]
pub struct StackFrameTransformers;

impl StackFrameTransformers {
    /// Identity transformer leaving frames untouched (`StackFrameTransformers.empty`).
    pub fn empty() -> Arc<dyn StackFrameTransformer> {
        Arc::new(EmptyTransformer)
    }

    /// Default transformer replacing `pkl:` stdlib URIs with GitHub 0.32.0 links
    /// (`StackFrameTransformers.defaultTransformer`).
    pub fn default_transformer() -> Arc<dyn StackFrameTransformer> {
        Arc::new(DefaultTransformer)
    }

    /// Creates a transformer from a closure.
    pub fn from_fn(
        f: impl Fn(StackFrame) -> StackFrame + Send + Sync + 'static,
    ) -> Arc<dyn StackFrameTransformer> {
        Arc::new(FnTransformer(f))
    }

    /// Chains two boxed transformers (`first` then `second`).
    pub fn chain(
        first: Arc<dyn StackFrameTransformer>,
        second: Arc<dyn StackFrameTransformer>,
    ) -> Arc<dyn StackFrameTransformer> {
        Arc::new(ChainedTransformer { first, second })
    }
}

pub(crate) struct HostFrameTransformerAdapter(pub(crate) Arc<dyn StackFrameTransformer>);

impl HostFrameTransformer for HostFrameTransformerAdapter {
    fn transform(&self, frame: LogFrame) -> LogFrame {
        let sf = StackFrame::from_log_frame(&frame);
        self.0.transform(sf).into_log_frame()
    }
}
