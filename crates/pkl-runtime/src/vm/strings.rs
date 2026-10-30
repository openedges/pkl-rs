//! Shared String builtins. Java-compatible values retain UTF-16 code units.
use super::*;
use crate::diagnostic::{PowerAssertionValue, TypeDiagnostic};

pub(super) fn repeat_block(
    blocks: &mut Vec<Vec<Instruction>>,
    metadata: &mut Vec<BlockMetadata>,
) -> usize {
    let source = crate::stdlib_sources::base();
    let declaration = "external function repeat(count: UInt): String";
    let start = source
        .text
        .find(declaration)
        .expect("pinned String.repeat declaration");
    let block = blocks.len();
    blocks.push(vec![
        Instruction::LoadLocal(0),
        Instruction::StringRepeat,
        Instruction::Return,
    ]);
    metadata.push(BlockMetadata {
        source: Some(source),
        member: "#String.repeat".into(),
        declaration: Some(Span::new(start, start + declaration.len())),
        // External body errors have no guest callsite; parameter checks below
        // supply the authored UInt alias location explicitly.
        spans: vec![None; 3],
        ..BlockMetadata::default()
    });
    block
}

impl Vm {
    pub(super) fn subscript_string(
        &self,
        text: &PklString,
        key: Value,
        key_span: Span,
    ) -> Result<Value, RuntimeError> {
        let Value::Int(index) = key else {
            let mut message = crate::pkl_string::PklStringBuilder::default();
            message.push_str("Operator `[]` is not defined for operand types `String` and `");
            message.push_value(&crate::diagnostic_values::type_name(
                &key,
                &self.heap,
                &self.program.module_name,
            )?);
            message.push_str("`.\nLeft operand : ");
            message.push_value(&crate::diagnostic_values::render_string(
                &Value::String(text.clone()),
                &self.heap,
                &self.program.module_name,
                80,
            )?);
            message.push_str("\nRight operand: ");
            message.push_value(&crate::diagnostic_values::render_string(
                &key,
                &self.heap,
                &self.program.module_name,
                80,
            )?);
            return Err(RuntimeError::new(ErrorKind::Type, message.finish()));
        };
        if let Some(point) = usize::try_from(index.get())
            .ok()
            .and_then(|i| text.code_point_at(i))
        {
            return Ok(Value::String(point));
        }
        let last = text.code_point_len() as i64 - 1;
        let mut message = crate::pkl_string::PklStringBuilder::default();
        message.push_str(&format!(
            "Character index `{}` is out of range `0`..`{}`.\nString: ",
            diagnostic_integer(*index),
            diagnostic_integer(last)
        ));
        message.push_value(&crate::diagnostic_values::render_string(
            &Value::String(text.clone()),
            &self.heap,
            &self.program.module_name,
            80,
        )?);
        Err(RuntimeError::new(ErrorKind::Type, message.finish()).at(key_span))
    }

    pub(super) fn string_repeat_method(&mut self, receiver: Value) {
        let id = self.heap.allocate(Object {
            function: Some(Function {
                block: self.program.string_repeat_block,
                arity: 1,
                method: true,
                captured_this: Some(receiver),
            }),
            ..Object::default()
        });
        self.stack.push(Value::Function(id));
    }

    pub(super) fn repeat_string(&mut self) -> Result<(), RuntimeError> {
        let value = self.pop()?;
        let count = match value {
            Value::Int(count) if count >= 0 => count,
            value => {
                let source = crate::stdlib_sources::base();
                let alias = source
                    .text
                    .find("typealias UInt = Int(isPositive)")
                    .expect("pinned UInt alias");
                let int_start = alias + "typealias UInt = ".len();
                let (mut error, span) = if let Value::Int(count) = value {
                    let span = Span::new(int_start + 4, int_start + 4 + "isPositive".len());
                    let mut error = RuntimeError::new(ErrorKind::Type, "Type constraint violated.");
                    // Stdlib constraints retain their instrumentation even when
                    // user-module power assertions are disabled.
                    error.type_diagnostic = Some(Box::new(TypeDiagnostic::Constraint {
                        span,
                        body_span: span,
                        value: count.to_string().into(),
                        observations: Some(vec![PowerAssertionValue {
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
                            &value,
                            &self.program.module_name,
                            &self.heap,
                        )?,
                        Span::new(int_start, int_start + 3),
                    )
                };
                error.span = Some(span);
                error.frames.push(RuntimeFrame {
                    anonymous: false,
                    source: Some(source),
                    span,
                    member: "#String.repeat".into(),
                });
                return Err(error);
            }
        };
        let Some(Value::String(text)) = self
            .frames
            .last()
            .and_then(|frame| frame.environment.this_value.as_ref())
        else {
            return Err(invalid("String.repeat receiver"));
        };
        let length = i64::try_from(text.encode_utf16().count())
            .map_err(|_| invalid("String length exceeds Int"))?;
        let total = length
            .checked_mul(*count)
            .ok_or_else(|| numeric_error(crate::numeric::NumericError::IntegerOverflow))?;
        if total > i32::MAX as i64 {
            let digits = total.to_string();
            let mut grouped = String::with_capacity(digits.len() + digits.len() / 3);
            for (index, digit) in digits.chars().enumerate() {
                if index != 0 && (digits.len() - index).is_multiple_of(3) {
                    grouped.push(',');
                }
                grouped.push(digit);
            }
            return Err(RuntimeError::new(
                ErrorKind::Arithmetic,
                format!("Int value `{grouped}` is too large (only Int32 supported here)."),
            ));
        }
        // The checked length precedes Java's long-to-int count narrowing. For an
        // empty receiver this can expose a negative Java implementation count.
        let narrowed = count.get() as i32;
        if narrowed < 0 {
            return Err(RuntimeError::new(
                ErrorKind::Unsupported,
                "String.repeat encountered the pinned Java negative narrowed-count bug.",
            ));
        }
        if total == 0 {
            self.stack.push(Value::string(""));
        } else if count == 1 {
            self.stack.push(Value::String(text.clone()));
        } else {
            let mut units = Vec::new();
            units.try_reserve_exact(total as usize).map_err(|_| {
                RuntimeError::new(ErrorKind::Host, "String.repeat allocation failed.")
            })?;
            units.extend(text.encode_utf16());
            let source_length = units.len();
            while units.len() < total as usize {
                let copy = units.len().min(total as usize - units.len());
                debug_assert!(copy.is_multiple_of(source_length));
                units.extend_from_within(..copy);
            }
            self.stack.push(Value::String(PklString::from_utf16(units)));
        }
        Ok(())
    }
}

// Java MessageFormat formats numeric error arguments with grouping separators.
pub(super) fn diagnostic_integer(value: i64) -> String {
    let digits = value.unsigned_abs().to_string();
    let mut output = String::with_capacity(digits.len() + digits.len() / 3 + 1);
    if value < 0 {
        output.push('-');
    }
    for (index, digit) in digits.chars().enumerate() {
        if index != 0 && (digits.len() - index).is_multiple_of(3) {
            output.push(',');
        }
        output.push(digit);
    }
    output
}

pub(super) fn encode_block(
    blocks: &mut Vec<Vec<Instruction>>,
    metadata: &mut Vec<BlockMetadata>,
) -> usize {
    let source = crate::stdlib_sources::base();
    let declaration = "external function encodeToBytes(charset: Charset): Bytes";
    let start = source
        .text
        .find(declaration)
        .expect("String.encodeToBytes declaration");
    let (charset_type, charset_span) = bytes::charset_type();
    let block = blocks.len();
    blocks.push(vec![
        Instruction::LoadLocal(0),
        Instruction::CheckType(charset_type),
        Instruction::StringEncode,
        Instruction::Return,
    ]);
    metadata.push(BlockMetadata {
        source: Some(source),
        member: "#String.encodeToBytes".into(),
        declaration: Some(Span::new(start, start + declaration.len())),
        spans: vec![None, Some(charset_span), None, None],
        ..Default::default()
    });
    block
}
impl Vm {
    pub(super) fn string_encode_method(&mut self, receiver: Value) {
        let id = self.heap.allocate(Object {
            function: Some(Function {
                block: self.program.string_encode_block,
                arity: 1,
                method: true,
                captured_this: Some(receiver),
            }),
            ..Default::default()
        });
        self.stack.push(Value::Function(id));
    }
    pub(super) fn encode_string(&mut self) -> Result<(), RuntimeError> {
        let Value::String(charset) = self.pop()? else {
            return Err(invalid("String.encodeToBytes charset"));
        };
        let Some(Value::String(text)) = self
            .frames
            .last()
            .and_then(|f| f.environment.this_value.as_ref())
        else {
            return Err(invalid("String.encodeToBytes receiver"));
        };
        let bytes = match charset.as_str() {
            Some("UTF-8") => text.to_java_utf8().into_bytes(),
            Some("UTF-16") => {
                let mut out = Vec::new();
                if !text.is_empty() {
                    out.extend([0xfe, 0xff]);
                }
                for value in char::decode_utf16(text.encode_utf16()) {
                    let mut units = [0; 2];
                    for u in value
                        .unwrap_or(char::REPLACEMENT_CHARACTER)
                        .encode_utf16(&mut units)
                    {
                        out.extend(u.to_be_bytes());
                    }
                }
                out
            }
            Some("ISO-8859-1") => char::decode_utf16(text.encode_utf16())
                .map(|value| {
                    value
                        .ok()
                        .filter(|v| (*v as u32) <= 255)
                        .map_or(b'?', |v| v as u8)
                })
                .collect(),
            _ => return Err(invalid("checked Charset value")),
        };
        self.stack.push(Value::Bytes(bytes.into()));
        Ok(())
    }
}
