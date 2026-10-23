//! Guest exception handlers are continuations in the current VM invocation.
//! They never create an evaluator, intercept Host protocol failures, or unwind
//! suspended callers. Successful inner lazy publications remain owned by their
//! receivers when an enclosing invocation subsequently throws.
use super::*;

pub(super) struct GuestCatch {
    pub stack: usize,
    pub or_null: bool,
}

impl Vm {
    pub(super) fn begin_guest_catch(&mut self, or_null: bool) -> Result<(), RuntimeError> {
        let Value::Function(function) = self.pop()? else {
            return Err(invalid("guest catch requires checked Function0"));
        };
        let frame = self
            .frames
            .last_mut()
            .ok_or_else(|| invalid("catch outside frame"))?;
        if frame.guest_catch.is_some() {
            return Err(invalid("overlapping guest exception handler"));
        }
        frame.guest_catch = Some(GuestCatch {
            stack: self.stack.len(),
            or_null,
        });
        self.invoke_function(function, Vec::new(), false)
    }

    pub(super) fn finish_guest_catch(&mut self) -> Result<(), RuntimeError> {
        let handler = self
            .frames
            .last_mut()
            .expect("catch frame")
            .guest_catch
            .take()
            .expect("catch continuation");
        if self.stack.len() != handler.stack + 1 {
            return Err(invalid("unbalanced guest catch return"));
        }
        self.pop()?;
        if handler.or_null {
            self.stack.push(Value::Null);
            Ok(())
        } else {
            // TestNodes throws this outside its try block. An enclosing handler
            // may catch it, but this retired handler cannot catch itself.
            Err(RuntimeError::new(
                ErrorKind::Type,
                "Expected an exception, but none was thrown.",
            ))
        }
    }

    pub(super) fn guest_catch_boundary(&self, error: &RuntimeError) -> Option<usize> {
        is_guest_exception(error)
            .then(|| {
                self.frames
                    .iter()
                    .rposition(|frame| frame.guest_catch.is_some())
            })
            .flatten()
    }

    pub(super) fn render_caught_exception(&self, error: &RuntimeError) -> PklString {
        let message = if let Some(diagnostic) = &error.type_diagnostic {
            let source = error
                .frames
                .first()
                .and_then(|frame| frame.source.as_ref())
                .or(self.context.primary_source.as_ref());
            let mut sources = crate::source::SourceMap::default();
            let id = sources.add("caught", source.map_or("", |source| source.text.as_ref()));
            diagnostic.render_without_power(sources.get(id).expect("caught source"))
        } else {
            error.message.clone()
        };
        // VmExceptionRenderer(null, false, false) omits stack frames AND hints.
        // TestNodes then applies Java String.lines(), skip(1), join(" "), strip().
        let rendered = crate::diagnostic::render_error_text(&message, &[], None);
        single_line(&rendered)
    }
}

fn single_line(rendered: &PklString) -> PklString {
    let units: Vec<_> = rendered.encode_utf16().collect();
    let mut lines = Vec::new();
    let mut start = 0;
    let mut index = 0;
    while index < units.len() {
        if matches!(units[index], 10 | 13) {
            lines.push(&units[start..index]);
            if units[index] == 13 && units.get(index + 1) == Some(&10) {
                index += 1;
            }
            start = index + 1;
        }
        index += 1;
    }
    if start < units.len() {
        lines.push(&units[start..]);
    }
    let mut output = Vec::new();
    for (index, line) in lines.into_iter().skip(1).enumerate() {
        if index != 0 {
            output.push(32);
        }
        output.extend_from_slice(line);
    }
    let whitespace = |unit: &u16| {
        char::from_u32(u32::from(*unit)).is_some_and(crate::diagnostic::java_whitespace)
    };
    let start = output.iter().take_while(|unit| whitespace(unit)).count();
    let end = output.len()
        - output[start..]
            .iter()
            .rev()
            .take_while(|unit| whitespace(unit))
            .count();
    PklString::from_utf16(output[start..end].to_vec())
}

/// Ordinary VmException provenance, shared by handlers and renderer unwinding.
pub(super) fn is_guest_exception(error: &RuntimeError) -> bool {
    use crate::diagnostic::FailureDisposition;
    if let Some(frontend) = &error.frontend
        && frontend.diagnostic.provenance.disposition != FailureDisposition::LanguageError
    {
        return false;
    }
    match error.kind {
        ErrorKind::Type
        | ErrorKind::MissingMember
        | ErrorKind::MissingType
        | ErrorKind::Arithmetic => true,
        ErrorKind::Cycle => !matches!(
            error.message.as_str(),
            Some("evaluation frame limit exceeded" | "experiment frame limit exceeded")
        ),
        ErrorKind::Host => error.host_language_exception,
        ErrorKind::InvalidBytecode
        | ErrorKind::StaleHandle
        | ErrorKind::Busy
        | ErrorKind::Cancelled
        | ErrorKind::Timeout
        | ErrorKind::Closed
        | ErrorKind::Unsupported => false,
    }
}
