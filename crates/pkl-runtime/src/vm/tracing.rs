//! Trace evaluation uses the shared forcing and renderer continuations.
use super::*;
use crate::logging::{LogEvent, LogFrame, LogLevel, TraceMode};
use crate::value::RuntimeSource;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Rendering,
    Waiting,
    Acknowledged,
}

pub(super) struct TraceRun {
    source: Arc<RuntimeSource>,
    operand: Span,
    phase: Phase,
}
impl TraceRun {
    pub(super) fn acknowledge(&mut self) -> Result<(), RuntimeError> {
        if self.phase != Phase::Waiting {
            return Err(invalid("trace logger is not waiting"));
        }
        self.phase = Phase::Acknowledged;
        Ok(())
    }
    pub(super) fn acknowledged(&self) -> bool {
        self.phase == Phase::Acknowledged
    }
}

impl Vm {
    #[cfg_attr(target_arch = "wasm32", inline(never))]
    pub(super) fn begin_trace(&mut self, operand: Span) -> Result<(), RuntimeError> {
        if self.callback_closing {
            return Err(RuntimeError::new(ErrorKind::Closed, "evaluator is closing"));
        }
        let frame = self
            .frames
            .last()
            .ok_or_else(|| invalid("trace outside frame"))?;
        let source = self
            .program
            .metadata
            .get(frame.block)
            .and_then(|metadata| metadata.source.clone())
            .or_else(|| self.context.primary_source.clone())
            .ok_or_else(|| invalid("trace requires authored source context"))?;
        let root = self.pop()?;
        self.begin_forcing(
            root,
            forcing::FinishAction::Trace(TraceRun {
                source,
                operand,
                phase: Phase::Rendering,
            }),
        )
    }

    /// Returns true only when no external callback is needed.
    pub(super) fn emit_trace(
        &mut self,
        trace: &mut TraceRun,
        rendered: PklString,
    ) -> Result<bool, RuntimeError> {
        if trace.phase != Phase::Rendering {
            return Err(invalid("trace logger emitted twice"));
        }
        let lhs = trace
            .source
            .text
            .get(trace.operand.start..trace.operand.end)
            .ok_or_else(|| invalid("trace operand source span"))?;
        let message = message(lhs, &rendered, self.context.trace_mode);
        let frame = source_frame(&trace.source, trace.operand)?;
        // BufferedLogger records before frame transformation or the delegate callback.
        self.log_buffer.extend(message.encode_utf16());
        self.log_buffer.push(u16::from(b'\n'));
        let event = Arc::new(LogEvent {
            level: LogLevel::Trace,
            message,
            frame,
        });
        if self.begin_log_delivery(event)? {
            return Ok(true);
        }
        trace.phase = Phase::Waiting;
        Ok(false)
    }
}

fn message(lhs: impl Into<PklString>, rhs: &PklString, mode: TraceMode) -> PklString {
    let left: Vec<u16> = lhs.into().encode_utf16().collect();
    let right: Vec<u16> = rhs.encode_utf16().collect();
    let mut result = Vec::with_capacity(left.len() + right.len() + 3);
    let newline = u16::from(b'\n');
    match mode {
        TraceMode::Compact => {
            result.extend(left);
            result.extend(" = ".encode_utf16());
            result.extend(right);
        }
        TraceMode::Pretty => {
            if left.contains(&newline) {
                result.push(newline);
                indent(&left, &mut result);
                result.extend("\n=".encode_utf16());
            } else {
                result.extend(left);
                result.extend(" =".encode_utf16());
            }
            if right.contains(&newline) {
                result.push(newline);
                indent(&right, &mut result);
            } else {
                result.push(u16::from(b' '));
                result.extend(right);
            }
            result.push(newline);
        }
    }
    PklString::from_utf16(result)
}

fn indent(units: &[u16], output: &mut Vec<u16>) {
    // Java String.split("\n") discards trailing empty fields, preserving CR.
    let mut lines: Vec<_> = units.split(|unit| *unit == u16::from(b'\n')).collect();
    while lines.last().is_some_and(|line| line.is_empty()) {
        lines.pop();
    }
    for (index, line) in lines.into_iter().enumerate() {
        if index > 0 {
            output.push(u16::from(b'\n'));
        }
        output.extend("  ".encode_utf16());
        output.extend_from_slice(line);
    }
}

pub(super) fn source_frame(source: &RuntimeSource, span: Span) -> Result<LogFrame, RuntimeError> {
    if span.start >= span.end || !source.text.entire().is_boundary(span.end) {
        return Err(invalid("trace operand source span"));
    }
    let index = source.text.index();
    let start = index
        .position(span.start)
        .ok_or_else(|| invalid("trace source start"))?;
    let (last_byte, last_char) = source
        .text
        .entire()
        .range(..span.end)
        .characters()
        .next_back()
        .ok_or_else(|| invalid("trace source end"))?;
    let end = index
        .position(last_byte)
        .ok_or_else(|| invalid("trace source end"))?;
    let mut lines = Vec::with_capacity(end.line.saturating_sub(start.line) + 1);
    for line_no in start.line..=end.line {
        let slice = index
            .line_slice(line_no)
            .ok_or_else(|| invalid("trace source lines"))?;
        lines.push(slice.to_pkl_string());
    }
    Ok(LogFrame {
        module_uri: source.uri.clone(),
        member_name: None,
        source_lines: lines.into(),
        start_line: start.line,
        start_column: start.column_utf16,
        end_line: end.line,
        end_column: end.column_utf16 + last_char.len_utf16() - 1,
    })
}

#[cfg(test)]
mod tests;
