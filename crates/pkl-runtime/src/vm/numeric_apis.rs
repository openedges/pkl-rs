//! Missing Int/Float routes use ordinary calls and declared parameter checks.
use super::*;
#[path = "numeric_fixed.rs"]
mod numeric_fixed;

#[derive(Clone, Copy, Debug)]
pub enum Operation {
    IntIdentity,
    ToFloat,
    Fixed,
    Shl,
    Shr,
    Ushr,
    And,
    Or,
    Xor,
    ToChar,
}

pub(super) fn blocks(
    blocks: &mut Vec<Vec<Instruction>>,
    metadata: &mut Vec<BlockMetadata>,
) -> BTreeMap<(Arc<str>, Arc<str>), usize> {
    let source = crate::stdlib_sources::base();
    let mut result = BTreeMap::new();
    for (class, name, declaration, operation, parameter) in [
        (
            "Int",
            "toInt",
            "external function toInt(): Int",
            Operation::IntIdentity,
            None,
        ),
        (
            "Int",
            "toFloat",
            "external function toFloat(): Float",
            Operation::ToFloat,
            None,
        ),
        (
            "Float",
            "toFloat",
            "external function toFloat(): Float",
            Operation::ToFloat,
            None,
        ),
        (
            "Int",
            "toFixed",
            "external function toFixed(fractionDigits: Int(this.isBetween(0, 20))): String",
            Operation::Fixed,
            Some("fractionDigits: "),
        ),
        (
            "Float",
            "toFixed",
            "external function toFixed(fractionDigits: Int(this.isBetween(0, 20))): String",
            Operation::Fixed,
            Some("fractionDigits: "),
        ),
        (
            "Int",
            "shl",
            "external function shl(n: Int): Int",
            Operation::Shl,
            Some("n: "),
        ),
        (
            "Int",
            "shr",
            "external function shr(n: Int): Int",
            Operation::Shr,
            Some("n: "),
        ),
        (
            "Int",
            "ushr",
            "external function ushr(n: Int): Int",
            Operation::Ushr,
            Some("n: "),
        ),
        (
            "Int",
            "and",
            "external function and(n: Int): Int",
            Operation::And,
            Some("n: "),
        ),
        (
            "Int",
            "or",
            "external function or(n: Int): Int",
            Operation::Or,
            Some("n: "),
        ),
        (
            "Int",
            "xor",
            "external function xor(n: Int): Int",
            Operation::Xor,
            Some("n: "),
        ),
        (
            "Int",
            "toChar",
            "external function toChar(): Char",
            Operation::ToChar,
            None,
        ),
    ] {
        let class_start = source
            .text
            .find(&format!("class {class} "))
            .expect("pinned class");
        let start = class_start
            + source
                .text
                .entire()
                .range(class_start..)
                .find(declaration)
                .expect("pinned numeric API");
        let member: PklString = format!("#{class}.{name}").into();
        let mut code = Vec::new();
        let mut spans = Vec::new();
        if let Some(parameter) = parameter {
            let int_start = start + declaration.find(parameter).unwrap() + parameter.len();
            let int_span = Span::new(int_start, int_start + 3);
            let ty = if matches!(operation, Operation::Fixed) {
                let predicate = "this.isBetween(0, 20)";
                let body_start = int_start + "Int(".len();
                let body = Span::new(body_start, body_start + predicate.len());
                let this_span = Span::new(body_start, body_start + 4);
                let constraint = blocks.len();
                blocks.push(vec![
                    Instruction::LoadCustomThis,
                    Instruction::Observe(this_span),
                    Instruction::GetMethod("isBetween".into()),
                    Instruction::Constant(Value::Int(0.into())),
                    Instruction::Constant(Value::Int(20.into())),
                    Instruction::Call {
                        arity: 2,
                        method: true,
                    },
                    Instruction::Observe(body),
                    Instruction::Return,
                ]);
                metadata.push(BlockMetadata {
                    source: Some(source.clone()),
                    member: member.clone(),
                    inline_constraint: true,
                    spans: vec![
                        Some(this_span),
                        Some(this_span),
                        Some(body),
                        None,
                        None,
                        Some(body),
                        Some(body),
                        None,
                    ],
                    ..Default::default()
                });
                TypeCheck::Constrained {
                    base: Box::new(TypeCheck::Int),
                    base_span: int_span,
                    constraints: vec![ConstraintCode {
                        block: constraint,
                        arguments: BTreeMap::new(),
                        argument_sources: BTreeMap::new(),
                        span: body,
                        module_environment: false,
                        source: predicate.into(),
                    }],
                }
            } else {
                TypeCheck::Int
            };
            code.extend([Instruction::LoadLocal(0), Instruction::CheckType(ty)]);
            spans.extend([None, Some(int_span)]);
        }
        code.extend([
            Instruction::Primitive(super::Operation::NumericApi(operation)),
            Instruction::Return,
        ]);
        spans.extend([None, None]);
        let block = blocks.len();
        blocks.push(code);
        metadata.push(BlockMetadata {
            source: Some(source.clone()),
            member,
            declaration: Some(Span::new(start, start + declaration.len())),
            spans,
            ..Default::default()
        });
        result.insert((class.into(), name.into()), block);
    }
    result
}

impl Vm {
    pub(super) fn numeric_api_operation(
        &mut self,
        receiver: Value,
        operation: Operation,
    ) -> Result<(), RuntimeError> {
        let result = match operation {
            Operation::IntIdentity => receiver,
            Operation::ToFloat => match receiver {
                Value::Int(value) => Value::Float((value.get() as f64).into()),
                Value::Float(_) => receiver,
                _ => return Err(invalid("numeric toFloat receiver")),
            },
            Operation::Fixed => {
                let Value::Int(scale) = self.pop()? else {
                    return Err(invalid("checked toFixed scale"));
                };
                let scale = scale.get();
                if !(0..=20).contains(&scale) {
                    return Err(invalid("unchecked toFixed constraint"));
                }
                let text = match receiver {
                    Value::Int(value) => numeric_fixed::integer(value.get(), scale as usize),
                    Value::Float(value) => numeric_fixed::floating(value.get(), scale as usize),
                    _ => return Err(invalid("numeric toFixed receiver")),
                };
                Value::String(text.into())
            }
            Operation::ToChar => {
                let Value::Int(value) = receiver else {
                    return Err(invalid("Int.toChar receiver"));
                };
                let value = value.get();
                if !(0..=0x10ffff).contains(&value) {
                    return Err(RuntimeError::new(
                        ErrorKind::Arithmetic,
                        format!(
                            "Decimal `{}` is not a valid Unicode code point.",
                            strings::diagnostic_integer(value)
                        ),
                    ));
                }
                let units = if value < 0x10000 {
                    vec![value as u16]
                } else {
                    let supplementary = value as u32 - 0x10000;
                    vec![
                        0xd800 | (supplementary >> 10) as u16,
                        0xdc00 | (supplementary & 0x3ff) as u16,
                    ]
                };
                Value::String(PklString::from_utf16(units))
            }
            Operation::Shl
            | Operation::Shr
            | Operation::Ushr
            | Operation::And
            | Operation::Or
            | Operation::Xor => {
                let Value::Int(value) = receiver else {
                    return Err(invalid("integer bitwise receiver"));
                };
                let Value::Int(argument) = self.pop()? else {
                    return Err(invalid("checked bitwise argument"));
                };
                let (value, argument) = (value.get(), argument.get());
                let shift = (argument & 63) as u32;
                Value::Int(
                    match operation {
                        Operation::Shl => value.wrapping_shl(shift),
                        Operation::Shr => value.wrapping_shr(shift),
                        Operation::Ushr => (value as u64).wrapping_shr(shift) as i64,
                        Operation::And => value & argument,
                        Operation::Or => value | argument,
                        Operation::Xor => value ^ argument,
                        _ => unreachable!(),
                    }
                    .into(),
                )
            }
        };
        self.stack.push(result);
        Ok(())
    }
}
