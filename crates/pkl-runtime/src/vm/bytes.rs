//! Bytes literal argument checks, including the pinned UInt8 diagnostic source.
use super::*;

impl Vm {
    pub(super) fn check_byte(&self) -> Result<(), RuntimeError> {
        let value = self.stack.last().ok_or_else(|| invalid("Bytes argument"))?;
        if matches!(value, Value::Int(value) if (0..=255).contains(&value.get())) {
            return Ok(());
        }
        let source = crate::stdlib_sources::base();
        let declaration = source
            .text
            .find("typealias UInt8 = Int(isBetween(0, 255))")
            .unwrap();
        let int_start = declaration + "typealias UInt8 = ".len();
        let (mut error, span) = if matches!(value, Value::Int(_)) {
            let span = Span::new(int_start + 4, int_start + 4 + "isBetween(0, 255)".len());
            let mut error = RuntimeError::new(ErrorKind::Type, "Type constraint violated.");
            error.type_diagnostic = Some(Box::new(crate::diagnostic::TypeDiagnostic::Constraint {
                span,
                value: crate::diagnostic_values::render_string(
                    value,
                    &self.heap,
                    &self.program.module_name,
                    80,
                )?,
                body_span: span,
                // IntMaskSlotTypeNode reports its builtin range predicate even
                // when general source power assertions are disabled.
                observations: Some(vec![crate::diagnostic::PowerAssertionValue {
                    span,
                    value: "false".into(),
                    executions: 1,
                    hidden: false,
                }]),
            }));
            (error, span)
        } else {
            (
                type_error(
                    &TypeCheck::Int,
                    value,
                    &self.program.module_name,
                    &self.heap,
                )?,
                Span::new(int_start, int_start + 3),
            )
        };
        let frame = self
            .frames
            .last()
            .ok_or_else(|| invalid("Bytes argument frame"))?;
        let metadata = self
            .program
            .metadata
            .get(frame.block)
            .ok_or_else(|| invalid("Bytes argument metadata"))?;
        let module_name = metadata
            .source
            .as_ref()
            .map_or(&self.program.module_name, |source| &source.module_name);
        error.span = Some(span);
        error.frames.push(RuntimeFrame {
            span,
            member: frame
                .diagnostic_member
                .clone()
                .unwrap_or_else(|| metadata.member.clone()),
            source: Some(Arc::new(crate::value::RuntimeSource {
                module_name: module_name.clone(),
                ..source.as_ref().clone()
            })),
        });
        // The instruction's authored argument span is appended by ordinary
        // unwinding, matching BytesLiteralNode's inserted argument frame.
        Err(error)
    }
}
