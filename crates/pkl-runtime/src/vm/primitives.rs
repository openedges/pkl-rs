//! Primitive lookup shares ordinary call frames, argument checks and UTF-16 values.
use super::*;

#[derive(Clone, Copy, Debug)]
pub enum Operation {
    DataSize(data_size::Operation),
    Duration(duration::Operation),
    Bytes(bytes::Operation),
    IntSeq(int_seq::Operation),
    CollectionEndpoint { last: bool, nullable: bool },
    IfNonNull,
    Between,
    Uppercase,
    ReverseString,
    Contains,
    DropLast,
    BooleanXor,
    BooleanImplies,
}

pub(super) fn blocks(
    blocks: &mut Vec<Vec<Instruction>>,
    metadata: &mut Vec<BlockMetadata>,
) -> BTreeMap<(Arc<str>, Arc<str>), usize> {
    let source = crate::stdlib_sources::base();
    let mut result = BTreeMap::new();
    for (class, name, signature, operation, arity) in [
        (
            "Boolean",
            "xor",
            "external function xor(other: Boolean): Boolean",
            Operation::BooleanXor,
            1,
        ),
        (
            "Boolean",
            "implies",
            "external function implies(other: Boolean): Boolean",
            Operation::BooleanImplies,
            1,
        ),
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
            "reverse",
            "external function reverse(): String",
            Operation::ReverseString,
            0,
        ),
        (
            "String",
            "contains",
            "external function contains(pattern: String | Regex): Boolean",
            Operation::Contains,
            1,
        ),
        (
            "String",
            "dropLast",
            "external function dropLast(n: Int): String",
            Operation::DropLast,
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
            } else if matches!(operation, Operation::DropLast) {
                let offset = start + signature.find("Int").unwrap();
                code.push(Instruction::CheckType(TypeCheck::Int));
                spans.push(Some(Span::new(offset, offset + 3)));
            } else if matches!(operation, Operation::BooleanXor | Operation::BooleanImplies) {
                let offset = start + signature.find("Boolean").unwrap();
                code.push(Instruction::CheckType(TypeCheck::Boolean));
                spans.push(Some(Span::new(offset, offset + 7)));
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
    result.extend(data_size::blocks(blocks, metadata));
    result.extend(duration::blocks(blocks, metadata));
    result.extend(bytes::blocks(blocks, metadata));
    result.extend(int_seq::blocks(blocks, metadata));
    result.extend(collection_transforms::endpoint_blocks(blocks, metadata));
    result.extend(any::blocks(blocks, metadata));
    result
}

impl Vm {
    pub(super) fn scalar_member_error(
        &self,
        receiver: &Value,
        name: &str,
        method: bool,
    ) -> Option<RuntimeError> {
        let class = match receiver {
            Value::String(_) => "String",
            Value::Int(_) => "Int",
            Value::Float(_) => "Float",
            Value::Bool(_) => "Boolean",
            Value::Null | Value::NullWithDefault(_) => "Null",
            _ => return None,
        };
        let (properties, methods) = crate::diagnostic_names::builtin_members(class);
        let arity = if method {
            self.frames
                .last()
                .and_then(|frame| {
                    self.program
                        .metadata
                        .get(frame.block)?
                        .method_arities
                        .get(&frame.pc.saturating_sub(1))
                })
                .copied()
                .unwrap_or(0) as isize
        } else {
            -1
        };
        let mut error = RuntimeError::new(
            ErrorKind::MissingMember,
            if method {
                format!("Cannot find method `{name}` in class `{class}`.")
            } else {
                format!("Cannot find property `{name}` in object of type `{class}`.")
            },
        );
        error.hint = crate::diagnostic::member_suggestions(name, arity, &properties, &methods)
            .or_else(|| {
                let mut names = if method {
                    methods.into_iter().map(|method| method.0).collect()
                } else {
                    properties
                };
                names.sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));
                names.dedup();
                Some(
                    if method {
                        format!(
                            "Available methods in class `{class}`:\n{}",
                            names.join("\n")
                        )
                    } else {
                        format!(
                            "Available properties:\n{}",
                            if names.is_empty() {
                                "(none)".into()
                            } else {
                                names.join("\n")
                            }
                        )
                    }
                    .into(),
                )
            });
        Some(error)
    }

    pub(super) fn function_member_error(
        &self,
        receiver: &Value,
        name: &str,
        method: bool,
    ) -> Result<RuntimeError, RuntimeError> {
        let Value::Function(id) = receiver else {
            return Err(invalid("function member receiver"));
        };
        let arity = crate::diagnostic_values::function_arity(*id, &self.heap)?;
        let class = format!("Function{arity}");
        let mut methods = [
            ("getClass", "getClass()", 0),
            ("toString", "toString()", 0),
            ("ifNonNull", "ifNonNull(transform)", 1),
            ("applyToList", "applyToList(argumentList)", 1),
        ]
        .map(|(name, signature, count)| (name.to_owned(), signature.to_owned(), count))
        .to_vec();
        methods.push((
            "apply".into(),
            format!(
                "apply({})",
                (1..=arity)
                    .map(|n| format!("p{n}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            arity,
        ));
        let requested_arity = if method {
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
                format!("Cannot find method `{name}` in class `{class}`.")
            } else {
                format!("Cannot find property `{name}` in object of type `{class}`.")
            },
        );
        error.hint = crate::diagnostic::member_suggestions(name, requested_arity, &[], &methods)
            .or_else(|| {
                Some(if method {
                    let mut names = methods.into_iter().map(|m| m.0).collect::<Vec<_>>();
                    names.sort();
                    format!(
                        "Available methods in class `{class}`:\n{}",
                        names.join("\n")
                    )
                    .into()
                } else {
                    "Available properties:\n(none)".into()
                })
            });
        Ok(error)
    }

    pub(super) fn primitive_member(
        &mut self,
        receiver: &Value,
        name: &str,
        method: bool,
    ) -> Result<bool, RuntimeError> {
        if !method {
            if let Some(unit) = crate::data_size::DataSizeUnit::parse(name) {
                let number = match receiver {
                    Value::Int(value) => Some(value.get() as f64),
                    Value::Float(value) => Some(value.get()),
                    _ => None,
                };
                if let Some(value) = number {
                    self.stack
                        .push(Value::DataSize(crate::data_size::DataSize::new(
                            value, unit,
                        )));
                    return Ok(true);
                }
            }
            if let Value::DataSize(size) = receiver {
                let value = size.value();
                let result = match name {
                    "value" if value.is_finite() && value.fract() == 0.0 => {
                        Value::Int((value as i64).into())
                    }
                    "value" => Value::Float(value.into()),
                    "unit" => Value::String(size.unit().symbol_value()),
                    "isPositive" => Value::Bool(value >= 0.0),
                    "isBinaryUnit" => Value::Bool(size.unit().is_binary()),
                    "isDecimalUnit" => Value::Bool(size.unit().is_decimal()),
                    _ => return Ok(false),
                };
                self.stack.push(result);
                return Ok(true);
            }
            if let Some(unit) = crate::duration::DurationUnit::parse(name) {
                let number = match receiver {
                    Value::Int(value) => Some(value.get() as f64),
                    Value::Float(value) => Some(value.get()),
                    _ => None,
                };
                if let Some(value) = number {
                    self.stack
                        .push(Value::Duration(crate::duration::Duration::new(value, unit)));
                    return Ok(true);
                }
            }
            if let Value::Duration(duration) = receiver {
                let value = duration.value();
                let result = match name {
                    "value" => {
                        if value.is_finite() && value.fract() == 0.0 {
                            Value::Int((value as i64).into())
                        } else {
                            Value::Float(value.into())
                        }
                    }
                    "isoString" => Value::String(
                        duration
                            .iso_string()
                            .ok_or_else(|| {
                                RuntimeError::new(
                                    ErrorKind::Arithmetic,
                                    format!(
                                        "Cannot convert duration `{duration}` to ISO 8601 duration."
                                    ),
                                )
                            })?
                            .into(),
                    ),
                    "unit" => Value::String(duration.unit().symbol_value()),
                    "isPositive" => Value::Bool(value >= 0.0),
                    _ => return Ok(false),
                };
                self.stack.push(result);
                return Ok(true);
            }
        }
        if !method && let Value::IntSeq(sequence) = receiver {
            return Ok(self.int_seq_property(sequence, name));
        }
        if !method && let Value::Bytes(bytes) = receiver {
            return self.bytes_property(bytes, name);
        }
        if !method
            && name == "base64DecodedBytes"
            && let Value::String(text) = receiver
        {
            let bytes = resources::decode_base64(text).map_err(|message| {
                let rendered = crate::diagnostic_values::render_string(
                    receiver,
                    &self.heap,
                    &self.program.module_name,
                    80,
                )
                .expect("rendering a String does not read the heap");
                RuntimeError::new(
                    ErrorKind::Arithmetic,
                    PklString::join(&[message.into(), "\nString: ".into(), rendered], ""),
                )
            })?;
            self.stack.push(Value::Bytes(bytes.into()));
            return Ok(true);
        }
        let class = match receiver {
            Value::Bool(_) => "Boolean",
            Value::Int(_) => "Int",
            Value::Float(_) => "Float",
            Value::DataSize(_) => "DataSize",
            Value::Duration(_) => "Duration",
            Value::Bytes(_) => "Bytes",
            Value::IntSeq(_) => "IntSeq",
            Value::String(_) => "String",
            Value::Object(_) if self.is_list(receiver) => "List",
            Value::Object(_) if self.is_set(receiver) => "Set",
            Value::Object(id)
                if self.heap.get(*id)?.collection_kind == crate::heap::CollectionKind::Listing =>
            {
                "Listing"
            }
            _ => return Ok(false),
        };
        if method
            && matches!(class, "List" | "Set")
            && matches!(name, "first" | "firstOrNull" | "last" | "lastOrNull")
        {
            let mut error = RuntimeError::new(
                ErrorKind::MissingMember,
                format!("Cannot find method `{name}` in class `{class}`."),
            );
            let arity = self
                .frames
                .last()
                .and_then(|frame| {
                    self.program
                        .metadata
                        .get(frame.block)?
                        .method_arities
                        .get(&frame.pc.saturating_sub(1))
                })
                .copied()
                .unwrap_or(0) as isize;
            if class == "Set" {
                let (properties, methods) = crate::diagnostic_names::collection_members(class);
                error.hint =
                    crate::diagnostic::member_suggestions(name, arity, &properties, &methods);
            } else {
                error.hint = crate::diagnostic::member_suggestions(
                    name,
                    arity,
                    &crate::diagnostic_names::LIST_PROPERTIES
                        .iter()
                        .map(|name| (*name).to_owned())
                        .collect::<Vec<_>>(),
                    &crate::diagnostic_names::LIST_METHODS
                        .iter()
                        .map(|(name, signature, arity)| {
                            ((*name).to_owned(), (*signature).to_owned(), *arity)
                        })
                        .collect::<Vec<_>>(),
                );
            }
            return Err(error);
        }
        if !method
            && matches!(class, "List" | "Set")
            && matches!(name, "first" | "firstOrNull" | "last" | "lastOrNull")
        {
            let block = self.program.primitive_methods[&(class.into(), name.into())];
            let id = self.heap.allocate(Object {
                function: Some(Function {
                    block,
                    arity: 0,
                    method: true,
                    captured_this: Some(receiver.clone()),
                }),
                ..Default::default()
            });
            self.invoke_function(id, Vec::new(), false)?;
            return Ok(true);
        }
        if method {
            if matches!(receiver, Value::DataSize(_) | Value::Duration(_))
                && matches!(name, "getClass" | "toString")
            {
                let function = self.native_reference_method(
                    receiver.clone(),
                    if name == "toString" {
                        "scalarToString"
                    } else {
                        name
                    },
                );
                self.stack.push(function);
                return Ok(true);
            }
            let Some(&block) = self
                .program
                .primitive_methods
                .get(&(class.into(), name.into()))
            else {
                return Ok(false);
            };
            let arity = match name {
                "isBetween" | "fold" => 2,
                "step" | "map" => 1,
                "contains" | "dropLast" | "xor" | "implies" | "toDataSize" | "toDuration"
                | "toUnit" | "getOrNull" | "decodeToString" => 1,
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
                "isEven" => Some(value.get() & 1 == 0),
                "isOdd" => Some(value.get() & 1 != 0),
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
            Operation::DataSize(operation) => self.data_size_operation(receiver, operation)?,
            Operation::Duration(operation) => self.duration_operation(receiver, operation)?,
            Operation::Bytes(operation) => self.bytes_operation(receiver, operation)?,
            Operation::IntSeq(operation) => self.int_seq_operation(receiver, operation)?,
            Operation::CollectionEndpoint { last, nullable } => {
                self.collection_endpoint(receiver, last, nullable)?
            }
            Operation::IfNonNull => self.if_non_null(receiver)?,
            Operation::ReverseString => {
                let Value::String(text) = receiver else {
                    return Err(invalid("String.reverse receiver"));
                };
                self.stack.push(Value::String(text.reversed()));
            }
            Operation::BooleanXor | Operation::BooleanImplies => {
                let Value::Bool(left) = receiver else {
                    return Err(invalid("Boolean method receiver"));
                };
                let Value::Bool(right) = self.pop()? else {
                    return Err(invalid("checked Boolean method argument"));
                };
                self.stack.push(Value::Bool(match operation {
                    Operation::BooleanXor => left ^ right,
                    _ => !left || right,
                }));
            }
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
                            Value::Int(n) => n.get() as f64,
                            Value::Float(n) => **n,
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
            Operation::DropLast => {
                let Value::String(text) = receiver else {
                    return Err(invalid("String.dropLast receiver"));
                };
                let Value::Int(count) = self.pop()? else {
                    return Err(invalid("checked String.dropLast count"));
                };
                if count < 0 {
                    return Err(RuntimeError::new(
                        ErrorKind::Type,
                        format!(
                            "Expected a positive number, but got `{}`.",
                            strings::diagnostic_integer(count.get())
                        ),
                    ));
                }
                self.stack.push(Value::String(
                    text.drop_last_code_points(count.get() as u64),
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
