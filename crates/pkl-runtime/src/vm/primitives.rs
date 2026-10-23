//! Primitive lookup shares ordinary call frames, argument checks and UTF-16 values.
use super::*;

#[derive(Clone, Copy, Debug)]
pub enum Operation {
    Between,
    Uppercase,
    Contains,
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
            "isBetween",
            "external function isBetween(start: Number, inclusiveEnd: Number): Boolean",
            Operation::Between,
            2,
        ),
        (
            "Float",
            "isBetween",
            "external function isBetween(start: Number, inclusiveEnd: Number): Boolean",
            Operation::Between,
            2,
        ),
        (
            "String",
            "toUpperCase",
            "external function toUpperCase(): String",
            Operation::Uppercase,
            0,
        ),
        (
            "String",
            "contains",
            "external function contains(pattern: String | Regex): Boolean",
            Operation::Contains,
            1,
        ),
    ] {
        let class_start = source
            .text
            .find(&format!("class {class} "))
            .expect("primitive class source");
        let start = class_start
            + source.text[class_start..]
                .find(signature)
                .expect("primitive method source");
        let mut code = Vec::new();
        let mut spans = Vec::new();
        for argument in 0..arity {
            code.push(Instruction::LoadLocal(arity - argument - 1));
            spans.push(None);
            if matches!(operation, Operation::Between) {
                let parameter = if argument == 0 {
                    "start: Number"
                } else {
                    "inclusiveEnd: Number"
                };
                let offset =
                    start + signature.find(parameter).unwrap() + parameter.find("Number").unwrap();
                code.push(Instruction::CheckType(TypeCheck::Number));
                spans.push(Some(Span::new(offset, offset + 6)));
            }
        }
        code.push(Instruction::Primitive(operation));
        spans.push(if matches!(operation, Operation::Contains) {
            let offset = start + signature.find("String | Regex").unwrap();
            Some(Span::new(offset, offset + 14))
        } else {
            None
        });
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
    pub(super) fn primitive_member(
        &mut self,
        receiver: &Value,
        name: &str,
        method: bool,
    ) -> Result<bool, RuntimeError> {
        let class = match receiver {
            Value::Int(_) => "Int",
            Value::Float(_) => "Float",
            Value::String(_) => "String",
            _ => return Ok(false),
        };
        if method {
            let Some(&block) = self
                .program
                .primitive_methods
                .get(&(class.into(), name.into()))
            else {
                return Ok(false);
            };
            let arity = match name {
                "isBetween" => 2,
                "contains" => 1,
                _ => 0,
            };
            let id = self.heap.allocate(Object {
                function: Some(Function {
                    block,
                    arity,
                    method: true,
                    captured_this: Some(receiver.clone()),
                }),
                ..Default::default()
            });
            self.stack.push(Value::Function(id));
            return Ok(true);
        }
        let property = match receiver {
            Value::Int(value) => match name {
                "isPositive" => Some(*value >= 0),
                "isFinite" => Some(true),
                "isInfinite" | "isNaN" => Some(false),
                "isEven" => Some(value & 1 == 0),
                "isOdd" => Some(value & 1 != 0),
                "isNonZero" => Some(*value != 0),
                _ => None,
            },
            Value::Float(value) => match name {
                "isPositive" => Some(*value >= 0.0),
                "isFinite" => Some(value.is_finite()),
                "isInfinite" => Some(value.is_infinite()),
                "isNaN" => Some(value.is_nan()),
                "isNonZero" => Some(*value != 0.0),
                _ => None,
            },
            _ => None,
        };
        if let Some(value) = property {
            self.stack.push(Value::Bool(value));
            Ok(true)
        } else {
            Ok(false)
        }
    }
    pub(super) fn primitive(&mut self, operation: Operation) -> Result<(), RuntimeError> {
        let receiver = self
            .frames
            .last()
            .and_then(|f| f.environment.this_value.clone())
            .ok_or_else(|| invalid("primitive receiver"))?;
        match operation {
            Operation::Between => {
                let end = self.pop()?;
                let start = self.pop()?;
                let ordered = |a: &Value, b: &Value, lower: bool| match (a, b) {
                    (Value::Int(a), Value::Int(b)) => {
                        if lower {
                            a >= b
                        } else {
                            a <= b
                        }
                    }
                    _ => {
                        let number = |v: &Value| match v {
                            Value::Int(n) => *n as f64,
                            Value::Float(n) => *n,
                            _ => unreachable!("checked Number"),
                        };
                        let (a, b) = (number(a), number(b));
                        if lower { a >= b } else { a <= b }
                    }
                };
                self.stack.push(Value::Bool(
                    ordered(&receiver, &start, true) && ordered(&receiver, &end, false),
                ));
            }
            Operation::Uppercase => {
                let Value::String(value) = receiver else {
                    return Err(invalid("String uppercase receiver"));
                };
                self.stack
                    .push(Value::String(crate::string_case::uppercase(&value)));
            }
            Operation::Contains => {
                let pattern = self.pop()?;
                let Value::String(text) = receiver else {
                    return Err(invalid("String contains receiver"));
                };
                let Value::String(pattern) = pattern else {
                    let name = crate::diagnostic_values::type_name(
                        &pattern,
                        &self.heap,
                        &self.program.module_name,
                    )?;
                    let rendered = crate::diagnostic_values::render(
                        &pattern,
                        &self.heap,
                        &self.program.module_name,
                        80,
                    )?;
                    return Err(RuntimeError::new(
                        ErrorKind::Type,
                        PklString::from(format!(
                            "Expected value of type `String | Regex`, but got type `{name}`.\nValue: "
                        )).concat(&rendered),
                    ));
                };
                let text = text.encode_utf16().collect::<Vec<_>>();
                let pattern = pattern.encode_utf16().collect::<Vec<_>>();
                self.stack.push(Value::Bool(
                    pattern.is_empty() || text.windows(pattern.len()).any(|part| part == pattern),
                ));
            }
        }
        Ok(())
    }
}
