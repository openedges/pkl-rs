//! Duration external methods retain the ordinary argument checks and call frames.
use super::*;
use crate::duration::{Duration, DurationUnit};

#[derive(Clone, Copy, Debug)]
pub enum Operation {
    FromNumber,
    Between,
    ToUnit,
}

pub(super) fn unit_type() -> (TypeCheck, Span) {
    let source = crate::stdlib_sources::base();
    let declaration = source.text.find("typealias DurationUnit =").unwrap();
    let start = declaration
        + source
            .text
            .entire()
            .range(declaration..)
            .find("\"")
            .unwrap();
    let end = start + source.text.entire().range(start..).find("\n").unwrap();
    let spans = DurationUnit::ALL.map(|unit| {
        let literal = format!("\"{}\"", unit.symbol());
        let offset = start
            + source
                .text
                .entire()
                .range(start..end)
                .find(&literal)
                .unwrap();
        Span::new(offset, offset + literal.len())
    });
    (
        TypeCheck::Union {
            string_literals: true,
            types: DurationUnit::ALL
                .map(|unit| TypeCheck::StringLiteral(unit.symbol().into()))
                .to_vec(),
            spans: spans.to_vec(),
            labels: spans.to_vec(),
            default: None,
        },
        Span::new(start, end),
    )
}

#[cfg_attr(target_arch = "wasm32", inline(never))]
pub(super) fn blocks(
    blocks: &mut Vec<Vec<Instruction>>,
    metadata: &mut Vec<BlockMetadata>,
) -> BTreeMap<(Arc<str>, Arc<str>), usize> {
    let source = crate::stdlib_sources::base();
    let mut result = BTreeMap::new();
    for (class, name, signature, operation, arity) in [
        (
            "Int",
            "toDuration",
            "external function toDuration(unit: DurationUnit): Duration",
            Operation::FromNumber,
            1,
        ),
        (
            "Float",
            "toDuration",
            "external function toDuration(unit: DurationUnit): Duration",
            Operation::FromNumber,
            1,
        ),
        (
            "Duration",
            "isBetween",
            "external function isBetween(start: Duration, inclusiveEnd: Duration): Boolean",
            Operation::Between,
            2,
        ),
        (
            "Duration",
            "toUnit",
            "external function toUnit(unit: DurationUnit): Duration",
            Operation::ToUnit,
            1,
        ),
    ] {
        let class_start = source.text.find(&format!("class {class} ")).unwrap();
        let start = class_start
            + source
                .text
                .entire()
                .range(class_start..)
                .find(signature)
                .unwrap();
        let mut code = Vec::new();
        let mut spans = Vec::new();
        for argument in 0..arity {
            code.push(Instruction::LoadLocal(arity - argument - 1));
            spans.push(None);
            let (ty, span) = if matches!(operation, Operation::Between) {
                let parameter = if argument == 0 {
                    "start: Duration"
                } else {
                    "inclusiveEnd: Duration"
                };
                let offset = start
                    + signature.find(parameter).unwrap()
                    + parameter.find("Duration").unwrap();
                (TypeCheck::Duration, Span::new(offset, offset + 8))
            } else {
                unit_type()
            };
            code.push(Instruction::CheckType(Box::new(ty)));
            spans.push(Some(span));
        }
        code.push(Instruction::Primitive(primitives::Operation::Duration(
            operation,
        )));
        spans.push(None);
        code.push(Instruction::Return);
        spans.push(None);
        let block = blocks.len();
        blocks.push(code);
        metadata.push(BlockMetadata {
            source: Some(source.clone()),
            member: format!("#{class}.{name}").into(),
            declaration: Some(Span::new(start, start + signature.len())),
            spans,
            ..Default::default()
        });
        result.insert((class.into(), name.into()), block);
    }
    result
}

impl Vm {
    #[cfg_attr(target_arch = "wasm32", inline(never))]
    pub(super) fn duration_operation(
        &mut self,
        receiver: Value,
        operation: Operation,
    ) -> Result<(), RuntimeError> {
        if matches!(operation, Operation::FromNumber) {
            let number = match receiver {
                Value::Int(value) => value.get() as f64,
                Value::Float(value) => value.get(),
                _ => return Err(invalid("toDuration receiver")),
            };
            let unit = checked_unit(self.pop()?)?;
            self.stack
                .push(Value::Duration(Duration::new(number, unit)));
            return Ok(());
        }
        let Value::Duration(duration) = receiver else {
            return Err(invalid("Duration method receiver"));
        };
        let result = match operation {
            Operation::Between => {
                let Value::Duration(end) = self.pop()? else {
                    return Err(invalid("checked Duration end"));
                };
                let Value::Duration(start) = self.pop()? else {
                    return Err(invalid("checked Duration start"));
                };
                Value::Bool(duration.compare(start).is_ge() && duration.compare(end).is_le())
            }
            Operation::ToUnit => Value::Duration(duration.to_unit(checked_unit(self.pop()?)?)),
            Operation::FromNumber => unreachable!(),
        };
        self.stack.push(result);
        Ok(())
    }

    pub(super) fn duration_member_error(
        &self,
        name: impl Into<PklString>,
        method: bool,
    ) -> RuntimeError {
        let name = name.into();
        let properties = ["value", "unit", "isPositive", "isoString"].map(str::to_owned);
        let methods = [
            ("isBetween", "isBetween(start, inclusiveEnd)", 2),
            ("toUnit", "toUnit(unit)", 1),
            ("getClass", "getClass()", 0),
            ("toString", "toString()", 0),
            ("ifNonNull", "ifNonNull(transform)", 1),
        ]
        .map(|(name, signature, arity)| (name.to_owned(), signature.to_owned(), arity));
        let arity = if method {
            self.frames
                .last()
                .and_then(|frame| {
                    self.program.metadata.get(frame.block).and_then(|metadata| {
                        metadata.method_arities.get(&frame.pc.saturating_sub(1))
                    })
                })
                .copied()
                .unwrap_or(0) as isize
        } else {
            -1
        };
        let mut error = RuntimeError::new(
            ErrorKind::MissingMember,
            if method {
                PklString::from("Cannot find method `")
                    .concat(&name)
                    .concat(&"` in class `Duration`.".into())
            } else {
                PklString::from("Cannot find property `")
                    .concat(&name)
                    .concat(&"` in object of type `Duration`.".into())
            },
        );
        error.hint = crate::diagnostic::member_suggestions(&name, arity, &properties, &methods)
            .or_else(|| {
                let mut names = if method {
                    methods
                        .iter()
                        .map(|(name, _, _)| name.as_str())
                        .collect::<Vec<_>>()
                } else {
                    properties.iter().map(String::as_str).collect()
                };
                names.sort();
                Some(
                    format!(
                        "{}\n{}",
                        if method {
                            "Available methods in class `Duration`:"
                        } else {
                            "Available properties:"
                        },
                        names.join("\n")
                    )
                    .into(),
                )
            });
        error
    }
}

fn checked_unit(value: Value) -> Result<DurationUnit, RuntimeError> {
    let Value::String(unit) = value else {
        return Err(invalid("checked DurationUnit"));
    };
    unit.as_str()
        .and_then(DurationUnit::parse)
        .ok_or_else(|| invalid("checked DurationUnit"))
}

pub(super) fn binary(
    instruction: &Instruction,
    left: &Value,
    right: &Value,
) -> Result<Option<Value>, RuntimeError> {
    let number = |value: &Value| match value {
        Value::Int(value) => Some(value.get() as f64),
        Value::Float(value) => Some(value.get()),
        _ => None,
    };
    if let (Value::Duration(left), Value::Duration(right)) = (left, right) {
        let (x, y, unit) = left.common_values(*right);
        return Ok(Some(match instruction {
            Instruction::Add => Value::Duration(Duration::new(x + y, unit)),
            Instruction::Subtract => Value::Duration(Duration::new(x - y, unit)),
            Instruction::Divide => Value::Float((x / y).into()),
            Instruction::IntegerDivide => float_binary(instruction, x, y)?,
            Instruction::Less => Value::Bool(left.compare(*right).is_lt()),
            Instruction::LessEqual => Value::Bool(left.compare(*right).is_le()),
            Instruction::Greater => Value::Bool(left.compare(*right).is_gt()),
            Instruction::GreaterEqual => Value::Bool(left.compare(*right).is_ge()),
            _ => return Ok(None),
        }));
    }
    if let Value::Duration(duration) = left
        && let Some(number) = number(right)
    {
        let value = match instruction {
            Instruction::Multiply => duration.value() * number,
            Instruction::Divide => duration.value() / number,
            Instruction::Remainder => duration.value() % number,
            Instruction::Power => float_power(duration.value(), number),
            Instruction::IntegerDivide => {
                let Value::Int(value) = float_binary(instruction, duration.value(), number)? else {
                    unreachable!()
                };
                value.get() as f64
            }
            _ => return Ok(None),
        };
        return Ok(Some(Value::Duration(Duration::new(value, duration.unit()))));
    }
    if matches!(instruction, Instruction::Multiply)
        && let Value::Duration(duration) = right
        && let Some(number) = number(left)
    {
        // MultiplicationNode calls right.multiply(left), preserving this operand order.
        return Ok(Some(Value::Duration(Duration::new(
            duration.value() * number,
            duration.unit(),
        ))));
    }
    Ok(None)
}
