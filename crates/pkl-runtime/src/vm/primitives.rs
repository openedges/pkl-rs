//! Primitive lookup shares ordinary call frames, argument checks and UTF-16 values.
use super::*;
#[path = "function_apply.rs"]
mod function_apply;
#[path = "remaining_apis.rs"]
mod remaining_apis;
#[path = "string_api.rs"]
mod string_api;

#[derive(Clone, Copy, Debug)]
pub enum Operation {
    StringApi(string_api::Operation),
    FunctionApplyToList,
    Remaining(remaining_apis::Operation),
    DataSize(data_size::Operation),
    Duration(duration::Operation),
    Bytes(bytes::Operation),
    IntSeq(int_seq::Operation),
    Regex(regex::Operation),
    StringTransform(string_transforms::Operation),
    CollectionEndpoint {
        last: bool,
        nullable: bool,
        single: bool,
    },
    IfNonNull,
    Between,
    NumberRound {
        truncate: bool,
    },
    Uppercase,
    ReverseString,
    CheckStringPattern,
    StringIndex {
        last: bool,
        nullable: bool,
    },
    Contains,
    Matches,
    StartsWith,
    EndsWith,
    DropLast,
    Substring {
        nullable: bool,
    },
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
            "Int",
            "round",
            "external function round(): Int",
            Operation::NumberRound { truncate: false },
            0,
        ),
        (
            "Int",
            "truncate",
            "external function truncate(): Int",
            Operation::NumberRound { truncate: true },
            0,
        ),
        (
            "Float",
            "round",
            "external function round(): Float",
            Operation::NumberRound { truncate: false },
            0,
        ),
        (
            "Float",
            "truncate",
            "external function truncate(): Float",
            Operation::NumberRound { truncate: true },
            0,
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
        (
            "String",
            "substring",
            "external function substring(start: Int, exclusiveEnd: Int): String",
            Operation::Substring { nullable: false },
            2,
        ),
        (
            "String",
            "substringOrNull",
            "external function substringOrNull(start: Int, exclusiveEnd: Int): String?",
            Operation::Substring { nullable: true },
            2,
        ),
        (
            "String",
            "matches",
            "external function matches(regex: Regex): Boolean",
            Operation::Matches,
            1,
        ),
        (
            "String",
            "startsWith",
            "external function startsWith(pattern: String | Regex): Boolean",
            Operation::StartsWith,
            1,
        ),
        (
            "String",
            "endsWith",
            "external function endsWith(pattern: String | Regex): Boolean",
            Operation::EndsWith,
            1,
        ),
        (
            "String",
            "indexOf",
            "external function indexOf(pattern: String | Regex): Int",
            Operation::StringIndex {
                last: false,
                nullable: false,
            },
            1,
        ),
        (
            "String",
            "indexOfOrNull",
            "external function indexOfOrNull(pattern: String | Regex): Int?",
            Operation::StringIndex {
                last: false,
                nullable: true,
            },
            1,
        ),
        (
            "String",
            "lastIndexOf",
            "external function lastIndexOf(pattern: String | Regex): Int",
            Operation::StringIndex {
                last: true,
                nullable: false,
            },
            1,
        ),
        (
            "String",
            "lastIndexOfOrNull",
            "external function lastIndexOfOrNull(pattern: String | Regex): Int?",
            Operation::StringIndex {
                last: true,
                nullable: true,
            },
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
            } else if matches!(
                operation,
                Operation::Contains
                    | Operation::StartsWith
                    | Operation::EndsWith
                    | Operation::StringIndex { .. }
            ) {
                let offset = start + signature.find("String | Regex").unwrap();
                code.push(Instruction::Primitive(Operation::CheckStringPattern));
                spans.push(Some(Span::new(offset, offset + 14)));
            } else if matches!(operation, Operation::Matches) {
                let offset = start + signature.find(": Regex").unwrap() + 2;
                code.push(Instruction::CheckType(TypeCheck::Class {
                    module: None,
                    id: classes::REGEX,
                    name: "Regex".into(),
                }));
                spans.push(Some(Span::new(offset, offset + 5)));
            } else if matches!(operation, Operation::Substring { .. }) {
                let parameter = if argument == 0 {
                    "start: Int"
                } else {
                    "exclusiveEnd: Int"
                };
                let offset =
                    start + signature.find(parameter).unwrap() + parameter.find("Int").unwrap();
                code.push(Instruction::CheckType(TypeCheck::Int));
                spans.push(Some(Span::new(offset, offset + 3)));
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
    result.extend(function_apply::blocks(blocks, metadata));
    result.extend(string_api::blocks(blocks, metadata));
    result.extend(remaining_apis::blocks(blocks, metadata));
    result.extend(data_size::blocks(blocks, metadata));
    result.extend(duration::blocks(blocks, metadata));
    result.extend(bytes::blocks(blocks, metadata));
    result.extend(int_seq::blocks(blocks, metadata));
    result.extend(lazy_collections::blocks(blocks, metadata));
    result.extend(regex::blocks(blocks, metadata));
    result.extend(collection_transforms::endpoint_blocks(blocks, metadata));
    result.extend(any::blocks(blocks, metadata));
    result.extend(string_transforms::blocks(blocks, metadata));
    result
}

fn string_pattern_type_error(
    pattern: &Value,
    heap: &Heap,
    module: &str,
) -> Result<RuntimeError, RuntimeError> {
    let name = crate::diagnostic_values::type_name(pattern, heap, module)?;
    let rendered = crate::diagnostic_values::render(pattern, heap, module, 80)?;
    let actual = if pattern.is_null() {
        "`null`".to_owned()
    } else {
        format!("type `{name}`")
    };
    Ok(RuntimeError::new(
        ErrorKind::Type,
        PklString::from(format!(
            "Expected value of type `String | Regex`, but got {actual}.\nValue: "
        ))
        .concat(&rendered),
    ))
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
            Value::Regex(_) => "Regex",
            Value::Int(_) => "Int",
            Value::Float(_) => "Float",
            Value::Bool(_) => "Boolean",
            Value::Null | Value::NullWithDefault(_) => "Null",
            _ => return None,
        };
        Some(self.builtin_member_error(class, name, method))
    }

    pub(super) fn builtin_member_error(
        &self,
        class: &str,
        name: &str,
        method: bool,
    ) -> RuntimeError {
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
        error
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
        if method && name == "every" && self.collection_every_method(receiver.clone())? {
            return Ok(true);
        }
        if !method {
            if let Value::Regex(regex) = receiver {
                let value = match name {
                    "pattern" => Some(Value::String(regex.pattern().clone())),
                    "groupCount" => Some(Value::Int((regex.group_count() as i64).into())),
                    _ => None,
                };
                if let Some(value) = value {
                    self.stack.push(value);
                    return Ok(true);
                }
            }
            let numeric = match receiver {
                Value::Int(value) => match name {
                    "sign" => Some(Value::Int(value.get().signum().into())),
                    "abs" => Some(Value::Int(
                        value
                            .get()
                            .checked_abs()
                            .ok_or_else(|| {
                                RuntimeError::new(ErrorKind::Arithmetic, "Integer overflow.")
                            })?
                            .into(),
                    )),
                    "ceil" | "floor" => Some(Value::Int(value.get().into())),
                    _ => None,
                },
                Value::Float(value) => {
                    let number = value.get();
                    let result = match name {
                        // StrictMath.signum retains both zero signs and NaN.
                        "sign" if number == 0.0 || number.is_nan() => Some(number),
                        "sign" => Some(number.signum()),
                        "abs" => Some(number.abs()),
                        "ceil" => Some(number.ceil()),
                        "floor" => Some(number.floor()),
                        _ => None,
                    };
                    result.map(|number| Value::Float(number.into()))
                }
                _ => None,
            };
            if let Some(value) = numeric {
                self.stack.push(value);
                return Ok(true);
            }
            if let Value::String(text) = receiver {
                if self.string_extra_property(text, name)? {
                    return Ok(true);
                }
                if matches!(name, "chars" | "codePoints") {
                    if text.is_empty() {
                        let empty = self.empty_strict_collection(crate::heap::CollectionKind::List);
                        self.stack.push(Value::Object(empty));
                        return Ok(true);
                    }
                    let builder = self.heap.allocate(Object {
                        collection_kind: crate::heap::CollectionKind::List,
                        ..Default::default()
                    });
                    self.frames
                        .last_mut()
                        .expect("String property frame")
                        .builders
                        .push(builder);
                    // Java String.codePoints combines valid pairs and preserves
                    // unpaired UTF-16 units. Iterate once, without replacement.
                    for point in char::decode_utf16(text.encode_utf16()) {
                        let value = if name == "chars" {
                            Value::String(match point {
                                Ok(point) => point.to_string().into(),
                                Err(point) => {
                                    PklString::from_utf16(vec![point.unpaired_surrogate()])
                                }
                            })
                        } else {
                            Value::Int(
                                match point {
                                    Ok(point) => i64::from(u32::from(point)),
                                    Err(point) => i64::from(point.unpaired_surrogate()),
                                }
                                .into(),
                            )
                        };
                        self.stack.push(value);
                        self.append_strict_element()?;
                    }
                    self.frames
                        .last_mut()
                        .expect("String property frame")
                        .builders
                        .pop();
                    self.stack.push(Value::Object(builder));
                    return Ok(true);
                }
                let value = match name {
                    "length" => Some(Value::Int((text.code_point_len() as i64).into())),
                    "lastIndex" => Some(Value::Int((text.code_point_len() as i64 - 1).into())),
                    "isEmpty" => Some(Value::Bool(text.is_empty())),
                    "isNotEmpty" => Some(Value::Bool(!text.is_empty())),
                    "isBlank" => Some(Value::Bool(text.is_blank())),
                    "isNotBlank" => Some(Value::Bool(!text.is_blank())),
                    "isRegex" => Some(Value::Bool(
                        crate::pkl_regex::PklRegex::is_valid_pattern(text)
                            .map_err(|error| regex::backend_error(error, text))?,
                    )),
                    "isBase64" => Some(Value::Bool(resources::decode_base64(text).is_ok())),
                    "base64" => Some(Value::String(resources::base64(
                        text.to_java_utf8().as_bytes(),
                    ))),
                    "base64Decoded" | "base64DecodedBytes" => {
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
                                PklString::join(
                                    &[message.into(), "\nString: ".into(), rendered],
                                    "",
                                ),
                            )
                        })?;
                        Some(if name == "base64Decoded" {
                            Value::String(crate::resources::decode_utf8(&bytes))
                        } else {
                            Value::Bytes(bytes.into())
                        })
                    }
                    _ => None,
                };
                if let Some(value) = value {
                    self.stack.push(value);
                    return Ok(true);
                }
            }
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
        let class = match receiver {
            Value::Bool(_) => "Boolean",
            Value::Int(_) => "Int",
            Value::Float(_) => "Float",
            Value::DataSize(_) => "DataSize",
            Value::Duration(_) => "Duration",
            Value::Bytes(_) => "Bytes",
            Value::IntSeq(_) => "IntSeq",
            Value::String(_) => "String",
            Value::Regex(_) => "Regex",
            Value::Function(_) => "Function",
            Value::Object(_) if self.is_list(receiver) => "List",
            Value::Object(_) if self.is_set(receiver) => "Set",
            Value::Object(id)
                if self.heap.get(*id)?.collection_kind == crate::heap::CollectionKind::Listing =>
            {
                "Listing"
            }
            Value::Object(id)
                if self.heap.get(*id)?.collection_kind == crate::heap::CollectionKind::Mapping =>
            {
                "Mapping"
            }
            _ => return Ok(false),
        };
        if method
            && matches!(class, "List" | "Set" | "Listing")
            && (matches!(
                name,
                "first" | "firstOrNull" | "last" | "lastOrNull" | "single" | "singleOrNull"
            ) || (class == "Listing" && name == "distinct")
                || (class == "Set" && name == "restOrNull"))
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
            if class != "List" {
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
            && ((matches!(class, "List" | "Set")
                && matches!(
                    name,
                    "first" | "firstOrNull" | "last" | "lastOrNull" | "single" | "singleOrNull"
                ))
                || (class == "Set" && name == "restOrNull"))
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
                "replaceRange" if class == "String" => 3,
                "padStart" | "padEnd" if class == "String" => 2,
                "takeWhile" | "takeLastWhile" | "dropWhile" | "dropLastWhile"
                    if class == "String" =>
                {
                    1
                }
                "isBetween" | "fold" | "foldIndexed" | "splitLimit" | "replaceAll"
                | "replaceFirst" | "replaceLast" | "replaceAllMapped" | "replaceFirstMapped"
                | "replaceLastMapped" | "substring" | "substringOrNull" => 2,
                "step" | "map" | "findMatchesIn" | "matchEntire" | "split" | "toRadixString"
                | "applyToList" | "take" | "takeLast" | "drop" => 1,
                "contains" | "matches" | "startsWith" | "endsWith" | "indexOf"
                | "indexOfOrNull" | "lastIndexOf" | "lastIndexOfOrNull" | "dropLast" | "xor"
                | "implies" | "toDataSize" | "toDuration" | "toUnit" | "getOrNull"
                | "decodeToString" | "any" | "containsValue" | "distinctBy" | "isDistinctBy" => 1,
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
            Operation::StringApi(operation) => self.string_api_operation(receiver, operation)?,
            Operation::FunctionApplyToList => self.function_apply_to_list(receiver)?,
            Operation::Remaining(operation) => self.remaining_api(receiver, operation)?,
            Operation::DataSize(operation) => self.data_size_operation(receiver, operation)?,
            Operation::Duration(operation) => self.duration_operation(receiver, operation)?,
            Operation::Bytes(operation) => self.bytes_operation(receiver, operation)?,
            Operation::IntSeq(operation) => self.int_seq_operation(receiver, operation)?,
            Operation::Regex(operation) => self.regex_operation(receiver, operation)?,
            Operation::StringTransform(operation) => self.string_transform(receiver, operation)?,
            Operation::CollectionEndpoint {
                last,
                nullable,
                single,
            } => self.collection_endpoint(receiver, last, nullable, single)?,
            Operation::IfNonNull => self.if_non_null(receiver)?,
            Operation::NumberRound { truncate } => {
                let result = match receiver {
                    Value::Int(value) => Value::Int(value.get().into()),
                    Value::Float(value) => {
                        let number = value.get();
                        let rounded = if truncate {
                            // VmSafeMath.truncate chooses ceil only for negative values.
                            if number < 0.0 {
                                number.ceil()
                            } else {
                                number.floor()
                            }
                        } else {
                            number.round_ties_even()
                        };
                        Value::Float(rounded.into())
                    }
                    _ => return Err(invalid("Number rounding receiver")),
                };
                self.stack.push(result);
            }
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
            Operation::Substring { nullable } => {
                let Value::String(text) = receiver else {
                    return Err(invalid("String.substring receiver"));
                };
                let Value::Int(exclusive_end) = self.pop()? else {
                    return Err(invalid("checked String.substring end"));
                };
                let Value::Int(start) = self.pop()? else {
                    return Err(invalid("checked String.substring start"));
                };
                let value = match text.substring_code_points(start.get(), exclusive_end.get()) {
                    Ok(value) => Value::String(value),
                    Err(_) if nullable => Value::Null,
                    Err((index, minimum)) => {
                        let mut message = crate::pkl_string::PklStringBuilder::default();
                        message.push_str(&format!(
                            "Character index `{}` is out of range `{}`..`{}`.\nString: ",
                            strings::diagnostic_integer(index),
                            strings::diagnostic_integer(minimum),
                            strings::diagnostic_integer(text.code_point_len() as i64),
                        ));
                        message.push_value(&crate::diagnostic_values::render_string(
                            &Value::String(text),
                            &self.heap,
                            &self.program.module_name,
                            80,
                        )?);
                        return Err(RuntimeError::new(ErrorKind::Type, message.finish()));
                    }
                };
                self.stack.push(value);
            }
            Operation::CheckStringPattern => {
                let pattern = self
                    .stack
                    .last()
                    .ok_or_else(|| invalid("String pattern argument"))?;
                if !matches!(pattern, Value::String(_) | Value::Regex(_)) {
                    return Err(string_pattern_type_error(
                        pattern,
                        &self.heap,
                        &self.program.module_name,
                    )?);
                }
            }
            Operation::StringIndex { last, nullable } => {
                let pattern = self.pop()?;
                let Value::String(text) = receiver else {
                    return Err(invalid("String index receiver"));
                };
                let position = match &pattern {
                    Value::Regex(regex_pattern) => regex_pattern
                        .find_index(&text, last)
                        .map_err(|error| regex::backend_error(error, regex_pattern.pattern()))?,
                    Value::String(literal) => {
                        let units = text.encode_utf16().collect::<Vec<_>>();
                        let literal = literal.encode_utf16().collect::<Vec<_>>();
                        let unit_index = if literal.is_empty() {
                            Some(if last { units.len() } else { 0 })
                        } else if last {
                            units
                                .windows(literal.len())
                                .rposition(|part| part == literal)
                        } else {
                            units
                                .windows(literal.len())
                                .position(|part| part == literal)
                        };
                        unit_index.map(|index| {
                            PklString::from_utf16(units[..index].to_vec()).code_point_len()
                        })
                    }
                    _ => return Err(invalid("unchecked String index pattern")),
                };
                match position {
                    Some(position) => self.stack.push(Value::Int(
                        i64::try_from(position)
                            .map_err(|_| invalid("String index exceeds Int range"))?
                            .into(),
                    )),
                    None if nullable => self.stack.push(Value::Null),
                    None => {
                        let kind = if matches!(pattern, Value::Regex(_)) {
                            "regex"
                        } else {
                            "literal"
                        };
                        let text = crate::diagnostic_values::render(
                            &Value::String(text),
                            &self.heap,
                            &self.program.module_name,
                            80,
                        )?;
                        let pattern = crate::diagnostic_values::render(
                            &pattern,
                            &self.heap,
                            &self.program.module_name,
                            80,
                        )?;
                        return Err(RuntimeError::new(ErrorKind::Type, PklString::join(&[
                            format!("String does not contain a match for {kind} pattern.\nString : ").into(),
                            text, "\nPattern: ".into(), pattern,
                        ], "")));
                    }
                }
            }
            Operation::Uppercase => {
                let Value::String(value) = receiver else {
                    return Err(invalid("String uppercase receiver"));
                };
                self.stack
                    .push(Value::String(crate::string_case::uppercase(&value)));
            }
            Operation::Contains
            | Operation::Matches
            | Operation::StartsWith
            | Operation::EndsWith => {
                let pattern = self.pop()?;
                let Value::String(text) = receiver else {
                    return Err(invalid("String predicate receiver"));
                };
                let result = match pattern {
                    Value::Regex(pattern) => {
                        use crate::pkl_regex::StringPredicate;
                        let predicate = match operation {
                            Operation::Contains => StringPredicate::Contains,
                            Operation::Matches => StringPredicate::Matches,
                            Operation::StartsWith => StringPredicate::StartsWith,
                            Operation::EndsWith => StringPredicate::EndsWith,
                            _ => unreachable!(),
                        };
                        pattern
                            .string_predicate(&text, predicate)
                            .map_err(|error| regex::backend_error(error, pattern.pattern()))?
                    }
                    Value::String(pattern) if !matches!(operation, Operation::Matches) => {
                        let text = text.encode_utf16().collect::<Vec<_>>();
                        let pattern = pattern.encode_utf16().collect::<Vec<_>>();
                        match operation {
                            Operation::Contains => {
                                pattern.is_empty()
                                    || text.windows(pattern.len()).any(|part| part == pattern)
                            }
                            Operation::StartsWith => text.starts_with(&pattern),
                            Operation::EndsWith => text.ends_with(&pattern),
                            _ => unreachable!(),
                        }
                    }
                    _ => {
                        // The Regex-only matches argument is checked by CheckType.
                        if matches!(operation, Operation::Matches) {
                            return Err(invalid("unchecked String.matches argument"));
                        }
                        return Err(invalid("unchecked String pattern argument"));
                    }
                };
                self.stack.push(Value::Bool(result));
            }
        }
        Ok(())
    }
}
