//! DataSize external methods retain the ordinary argument checks and call frames.
use super::*;
use crate::data_size::{DataSize, DataSizeUnit};

#[derive(Clone, Copy, Debug)]
pub enum Operation {
    FromNumber,
    Between,
    ToUnit,
    ToBinary,
    ToDecimal,
    IfNonNull,
}

pub(super) fn unit_type() -> (TypeCheck, Span) {
    let source = crate::stdlib_sources::base();
    let declaration = source.text.find("typealias DataSizeUnit =").unwrap();
    let start = declaration + source.text[declaration..].find('"').unwrap();
    let end = start + source.text[start..].find('\n').unwrap();
    let spans = DataSizeUnit::ALL.map(|unit| {
        let literal = format!("\"{}\"", unit.symbol());
        let offset = start + source.text[start..end].find(&literal).unwrap();
        Span::new(offset, offset + literal.len())
    });
    (
        TypeCheck::Union {
            string_literals: true,
            types: DataSizeUnit::ALL
                .map(|unit| TypeCheck::StringLiteral(unit.symbol().into()))
                .to_vec(),
            spans: spans.to_vec(),
            labels: spans.to_vec(),
            default: None,
        },
        Span::new(start, end),
    )
}

pub(super) fn blocks(
    blocks: &mut Vec<Vec<Instruction>>,
    metadata: &mut Vec<BlockMetadata>,
) -> BTreeMap<(Arc<str>, Arc<str>), usize> {
    let source = crate::stdlib_sources::base();
    let mut result = BTreeMap::new();
    for (class, name, signature, operation, arity) in [
        (
            "Int",
            "toDataSize",
            "external function toDataSize(unit: DataSizeUnit): DataSize",
            Operation::FromNumber,
            1,
        ),
        (
            "Float",
            "toDataSize",
            "external function toDataSize(unit: DataSizeUnit): DataSize",
            Operation::FromNumber,
            1,
        ),
        (
            "DataSize",
            "isBetween",
            "external function isBetween(start: DataSize, inclusiveEnd: DataSize)",
            Operation::Between,
            2,
        ),
        (
            "DataSize",
            "toUnit",
            "external function toUnit(unit: DataSizeUnit): DataSize",
            Operation::ToUnit,
            1,
        ),
        (
            "DataSize",
            "toBinaryUnit",
            "external function toBinaryUnit(): DataSize",
            Operation::ToBinary,
            0,
        ),
        (
            "DataSize",
            "toDecimalUnit",
            "external function toDecimalUnit(): DataSize",
            Operation::ToDecimal,
            0,
        ),
    ] {
        let class_start = source.text.find(&format!("class {class} ")).unwrap();
        let start = class_start + source.text[class_start..].find(signature).unwrap();
        let mut code = Vec::new();
        let mut spans = Vec::new();
        for argument in 0..arity {
            code.push(Instruction::LoadLocal(arity - argument - 1));
            spans.push(None);
            let (ty, span) = if matches!(operation, Operation::Between) {
                let parameter = if argument == 0 {
                    "start: DataSize"
                } else {
                    "inclusiveEnd: DataSize"
                };
                let offset = start
                    + signature.find(parameter).unwrap()
                    + parameter.find("DataSize").unwrap();
                (
                    classes::named_type("DataSize").unwrap(),
                    Span::new(offset, offset + 8),
                )
            } else {
                unit_type()
            };
            code.push(Instruction::CheckType(ty));
            spans.push(Some(span));
        }
        code.push(Instruction::Primitive(primitives::Operation::DataSize(
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
    let signature = "external function ifNonNull<Result>(transform: (NonNull) -> Result): Result?";
    let start = source.text.find(signature).unwrap();
    let type_start = start + signature.find("(NonNull) -> Result").unwrap();
    let block = blocks.len();
    blocks.push(vec![
        Instruction::LoadLocal(0),
        Instruction::CheckType(TypeCheck::Function {
            parameters: Some(vec![TypeCheck::NonNull]),
            result: Box::new(TypeCheck::Unknown),
            syntax: true,
        }),
        Instruction::Primitive(primitives::Operation::DataSize(Operation::IfNonNull)),
        Instruction::Return,
    ]);
    metadata.push(BlockMetadata {
        source: Some(source),
        member: "#Any.ifNonNull".into(),
        declaration: Some(Span::new(start, start + signature.len())),
        spans: vec![
            None,
            Some(Span::new(
                type_start,
                type_start + "(NonNull) -> Result".len(),
            )),
            None,
            None,
        ],
        ..Default::default()
    });
    result.insert(("DataSize".into(), "ifNonNull".into()), block);
    result
}

impl Vm {
    pub(super) fn data_size_operation(
        &mut self,
        receiver: Value,
        operation: Operation,
    ) -> Result<(), RuntimeError> {
        if matches!(operation, Operation::IfNonNull) {
            let Value::Function(function) = self.pop()? else {
                return Err(invalid("checked ifNonNull function"));
            };
            return self.invoke_function(function, vec![receiver], false);
        }
        if matches!(operation, Operation::FromNumber) {
            let number = match receiver {
                Value::Int(value) => value.get() as f64,
                Value::Float(value) => value.get(),
                _ => return Err(invalid("toDataSize receiver")),
            };
            let unit = checked_unit(self.pop()?)?;
            self.stack
                .push(Value::DataSize(DataSize::new(number, unit)));
            return Ok(());
        }
        let Value::DataSize(size) = receiver else {
            return Err(invalid("DataSize method receiver"));
        };
        let result = match operation {
            Operation::Between => {
                let Value::DataSize(end) = self.pop()? else {
                    return Err(invalid("checked DataSize end"));
                };
                let Value::DataSize(start) = self.pop()? else {
                    return Err(invalid("checked DataSize start"));
                };
                Value::Bool(size.compare(start).is_ge() && size.compare(end).is_le())
            }
            Operation::ToUnit => Value::DataSize(size.to_unit(checked_unit(self.pop()?)?)),
            Operation::ToBinary => Value::DataSize(size.to_binary_unit()),
            Operation::ToDecimal => Value::DataSize(size.to_decimal_unit()),
            Operation::FromNumber | Operation::IfNonNull => unreachable!(),
        };
        self.stack.push(result);
        Ok(())
    }

    pub(super) fn data_size_member_error(&self, name: &str, method: bool) -> RuntimeError {
        let properties = [
            "value",
            "unit",
            "isPositive",
            "isBinaryUnit",
            "isDecimalUnit",
        ]
        .map(str::to_owned);
        let methods = [
            ("isBetween", "isBetween(start, inclusiveEnd)", 2),
            ("toUnit", "toUnit(unit)", 1),
            ("toBinaryUnit", "toBinaryUnit()", 0),
            ("toDecimalUnit", "toDecimalUnit()", 0),
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
                format!("Cannot find method `{name}` in class `DataSize`.")
            } else {
                format!("Cannot find property `{name}` in object of type `DataSize`.")
            },
        );
        error.hint = crate::diagnostic::member_suggestions(name, arity, &properties, &methods)
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
                            "Available methods in class `DataSize`:"
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

fn checked_unit(value: Value) -> Result<DataSizeUnit, RuntimeError> {
    let Value::String(unit) = value else {
        return Err(invalid("checked DataSizeUnit"));
    };
    unit.as_str()
        .and_then(DataSizeUnit::parse)
        .ok_or_else(|| invalid("checked DataSizeUnit"))
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
    if let (Value::DataSize(left), Value::DataSize(right)) = (left, right) {
        let (x, y, unit) = left.common_values(*right);
        return Ok(Some(match instruction {
            Instruction::Add => Value::DataSize(DataSize::new(x + y, unit)),
            Instruction::Subtract => Value::DataSize(DataSize::new(x - y, unit)),
            Instruction::Divide => Value::Float((x / y).into()),
            Instruction::IntegerDivide => float_binary(instruction, x, y)?,
            Instruction::Less => Value::Bool(left.compare(*right).is_lt()),
            Instruction::LessEqual => Value::Bool(left.compare(*right).is_le()),
            Instruction::Greater => Value::Bool(left.compare(*right).is_gt()),
            Instruction::GreaterEqual => Value::Bool(left.compare(*right).is_ge()),
            _ => return Ok(None),
        }));
    }
    if let Value::DataSize(size) = left
        && let Some(number) = number(right)
    {
        let value = match instruction {
            Instruction::Multiply => size.value() * number,
            Instruction::Divide => size.value() / number,
            Instruction::Remainder => size.value() % number,
            Instruction::Power => float_power(size.value(), number),
            Instruction::IntegerDivide => {
                let Value::Int(value) = float_binary(instruction, size.value(), number)? else {
                    unreachable!()
                };
                value.get() as f64
            }
            _ => return Ok(None),
        };
        return Ok(Some(Value::DataSize(DataSize::new(value, size.unit()))));
    }
    if matches!(instruction, Instruction::Multiply)
        && let Value::DataSize(size) = right
        && let Some(number) = number(left)
    {
        // MultiplicationNode calls right.multiply(left), preserving this operand order.
        return Ok(Some(Value::DataSize(DataSize::new(
            size.value() * number,
            size.unit(),
        ))));
    }
    Ok(None)
}
