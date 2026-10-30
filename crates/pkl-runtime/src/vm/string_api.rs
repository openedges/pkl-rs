//! Pinned String scalar APIs and resumable predicate slicing.
use super::*;
#[path = "string_glob.rs"]
mod string_glob;
#[path = "string_lower.rs"]
mod string_lower;
#[path = "string_parse.rs"]
mod string_parse;
#[path = "string_predicate.rs"]
mod string_predicate;

#[derive(Clone, Copy, Debug)]
pub enum Operation {
    Capitalize { lower: bool },
    GetOrNull,
    Take { last: bool },
    Drop,
    Trim { start: bool, end: bool, jdk: bool },
    Lowercase,
    Pad { end: bool },
    ReplaceRange,
    Parse { kind: ParseKind, nullable: bool },
    PredicateBegin { last: bool },
    PredicateNext { last: bool, drop: bool },
    PredicateAccept { last: bool, drop: bool },
}
#[derive(Clone, Copy, Debug)]
pub enum ParseKind {
    Int,
    Float,
    Boolean,
}
#[derive(Clone, Copy)]
enum Parameter {
    Int,
    String,
    Char,
    Predicate,
}
type Specification = (
    &'static str,
    Operation,
    &'static [(&'static str, Parameter)],
);

fn char_type(
    blocks: &mut Vec<Vec<Instruction>>,
    metadata: &mut Vec<BlockMetadata>,
    member: &str,
) -> (TypeCheck, Span) {
    let source = crate::stdlib_sources::base();
    let start = source
        .text
        .find("typealias Char = String(length == 1)")
        .unwrap()
        + "typealias Char = ".len();
    let base = Span::new(start, start + 6);
    let body = Span::new(start + 7, start + 18);
    let length = Span::new(start + 7, start + 13);
    let block = blocks.len();
    blocks.push(vec![
        Instruction::LoadCustomThis,
        Instruction::GetMember("length".into()),
        Instruction::Observe(length),
        Instruction::Constant(Value::Int(1.into())),
        Instruction::Equal,
        Instruction::Observe(body),
        Instruction::Return,
    ]);
    metadata.push(BlockMetadata {
        source: Some(source),
        member: member.into(),
        inline_constraint: true,
        spans: vec![
            None,
            Some(length),
            Some(length),
            None,
            Some(body),
            Some(body),
            None,
        ],
        ..Default::default()
    });
    (
        TypeCheck::Constrained {
            base: Box::new(TypeCheck::String),
            base_span: base,
            constraints: vec![ConstraintCode {
                block,
                arguments: BTreeMap::new(),
                argument_sources: BTreeMap::new(),
                span: body,
                module_environment: false,
                source: "length == 1".into(),
            }],
        },
        base,
    )
}
pub(super) fn blocks(
    blocks: &mut Vec<Vec<Instruction>>,
    metadata: &mut Vec<BlockMetadata>,
) -> BTreeMap<(Arc<str>, Arc<str>), usize> {
    use Parameter::{Char, Int, Predicate, String as Text};
    let source = crate::stdlib_sources::base();
    let class_start = source
        .text
        .find("external class String extends Any {")
        .unwrap();
    let mut result = BTreeMap::new();
    let specifications: &[Specification] = &[
        ("capitalize", Operation::Capitalize { lower: false }, &[]),
        ("decapitalize", Operation::Capitalize { lower: true }, &[]),
        ("getOrNull", Operation::GetOrNull, &[("index", Int)]),
        ("take", Operation::Take { last: false }, &[("n", Int)]),
        ("takeLast", Operation::Take { last: true }, &[("n", Int)]),
        ("drop", Operation::Drop, &[("n", Int)]),
        (
            "trim",
            Operation::Trim {
                start: true,
                end: true,
                jdk: true,
            },
            &[],
        ),
        (
            "trimStart",
            Operation::Trim {
                start: true,
                end: false,
                jdk: false,
            },
            &[],
        ),
        (
            "trimEnd",
            Operation::Trim {
                start: false,
                end: true,
                jdk: false,
            },
            &[],
        ),
        ("toLowerCase", Operation::Lowercase, &[]),
        (
            "padStart",
            Operation::Pad { end: false },
            &[("width", Int), ("char", Char)],
        ),
        (
            "padEnd",
            Operation::Pad { end: true },
            &[("width", Int), ("char", Char)],
        ),
        (
            "replaceRange",
            Operation::ReplaceRange,
            &[("start", Int), ("exclusiveEnd", Int), ("replacement", Text)],
        ),
        (
            "toInt",
            Operation::Parse {
                kind: ParseKind::Int,
                nullable: false,
            },
            &[],
        ),
        (
            "toIntOrNull",
            Operation::Parse {
                kind: ParseKind::Int,
                nullable: true,
            },
            &[],
        ),
        (
            "toFloat",
            Operation::Parse {
                kind: ParseKind::Float,
                nullable: false,
            },
            &[],
        ),
        (
            "toFloatOrNull",
            Operation::Parse {
                kind: ParseKind::Float,
                nullable: true,
            },
            &[],
        ),
        (
            "toBoolean",
            Operation::Parse {
                kind: ParseKind::Boolean,
                nullable: false,
            },
            &[],
        ),
        (
            "toBooleanOrNull",
            Operation::Parse {
                kind: ParseKind::Boolean,
                nullable: true,
            },
            &[],
        ),
        (
            "takeWhile",
            Operation::PredicateBegin { last: false },
            &[("predicate", Predicate)],
        ),
        (
            "takeLastWhile",
            Operation::PredicateBegin { last: true },
            &[("predicate", Predicate)],
        ),
        (
            "dropWhile",
            Operation::PredicateBegin { last: false },
            &[("predicate", Predicate)],
        ),
        (
            "dropLastWhile",
            Operation::PredicateBegin { last: true },
            &[("predicate", Predicate)],
        ),
    ];
    for &(name, operation, parameters) in specifications {
        let anchor = format!("external function {name}(");
        let start = class_start
            + source
                .text
                .entire()
                .range(class_start..)
                .find(&anchor)
                .expect("pinned String API");
        let end = start + source.text.entire().range(start..).find("\n").unwrap();
        let declaration = &source.text.entire().range(start..end);
        let member = format!("#String.{name}");
        let mut code = Vec::new();
        let mut spans = Vec::new();
        for (index, &(parameter, kind)) in parameters.iter().enumerate() {
            let anchor = format!("{parameter}: ");
            let at = start + declaration.find(&anchor).unwrap() + anchor.len();
            let (ty, span) = match kind {
                Int => (TypeCheck::Int, Span::new(at, at + 3)),
                Text => (TypeCheck::String, Span::new(at, at + 6)),
                Char => char_type(blocks, metadata, &member),
                Predicate => (
                    TypeCheck::Function {
                        parameters: Some(vec![TypeCheck::String]),
                        result: Box::new(TypeCheck::Boolean),
                        syntax: true,
                    },
                    Span::new(at, at + "(String) -> Boolean".len()),
                ),
            };
            code.extend([
                Instruction::LoadLocal(parameters.len() - index - 1),
                Instruction::CheckType(ty),
            ]);
            spans.extend([None, Some(span)]);
        }
        code.push(Instruction::Primitive(super::Operation::StringApi(
            operation,
        )));
        spans.push(None);
        if let Operation::PredicateBegin { last } = operation {
            let drop = name.starts_with("drop");
            code.extend([
                Instruction::Primitive(super::Operation::StringApi(Operation::PredicateNext {
                    last,
                    drop,
                })),
                Instruction::Call {
                    arity: 1,
                    method: false,
                },
                Instruction::Primitive(super::Operation::StringApi(Operation::PredicateAccept {
                    last,
                    drop,
                })),
            ]);
            spans.extend([None, None, None]);
        }
        code.push(Instruction::Return);
        spans.push(None);
        let block = blocks.len();
        blocks.push(code);
        metadata.push(BlockMetadata {
            source: Some(source.clone()),
            member: member.into(),
            declaration: Some(Span::new(start, end)),
            spans,
            ..Default::default()
        });
        result.insert(("String".into(), name.into()), block);
    }
    result
}
fn empty() -> PklString {
    PklString::from("").source_literal()
}
fn first_case(text: &PklString, lower: bool) -> PklString {
    let units = text.encode_utf16().collect::<Vec<_>>();
    let Some(first) = char::decode_utf16(units.iter().copied()).next() else {
        return empty();
    };
    let (point, width) = match first {
        Ok(point) => (u32::from(point), point.len_utf16()),
        Err(unit) => (u32::from(unit.unpaired_surrogate()), 1),
    };
    let mut result = Vec::with_capacity(units.len() + 1);
    if lower && point == 0x130 {
        // ROOT String lowercase of an isolated I-with-dot expands. An isolated
        // Sigma has no preceding cased character, so it is never final Sigma.
        result.extend([0x69, 0x307]);
    } else {
        let mapped = java_regex::utf16::character_record(point).map_or(point, |record| {
            if lower { record.lower } else { record.title }
        });
        if let Some(point) = char::from_u32(mapped) {
            result.extend_from_slice(point.encode_utf16(&mut [0; 2]));
        } else {
            result.push(mapped as u16);
        }
    }
    result.extend_from_slice(&units[width..]);
    // Java concatenates even when the mapped character is unchanged.
    PklString::from_utf16(result)
}
fn pkl_whitespace(point: u32) -> bool {
    matches!(point, 0x09..=0x0d | 0x20 | 0x85 | 0xa0 | 0x1680 | 0x2000..=0x200a | 0x2028 | 0x2029 | 0x202f | 0x205f | 0x3000)
}
fn trimmed(text: &PklString, start: bool, end: bool, jdk: bool) -> PklString {
    let points = char::decode_utf16(text.encode_utf16())
        .map(|point| point.map_or_else(|unit| u32::from(unit.unpaired_surrogate()), u32::from))
        .collect::<Vec<_>>();
    // StringLatin1.strip returns the canonical literal for an empty/all-space
    // string. Pkl trimStart/trimEnd instead use substring and keep a full view.
    if jdk && points.is_empty() {
        return empty();
    }
    let whitespace = |point: u32| {
        if jdk {
            java_regex::utf16::character_record(point)
                .is_some_and(|record| record.predicate_bits & (1 << 23) != 0)
        } else {
            pkl_whitespace(point)
        }
    };
    let mut left = 0;
    let mut right = points.len();
    if start {
        while left < right && whitespace(points[left]) {
            left += 1;
        }
    }
    if end {
        while right > left && whitespace(points[right - 1]) {
            right -= 1;
        }
    }
    text.substring_code_points(left as i64, right as i64)
        .expect("trim codepoint bounds")
}

impl Vm {
    pub(super) fn string_extra_property(
        &mut self,
        text: &PklString,
        name: &str,
    ) -> Result<bool, RuntimeError> {
        if name == "isGlobPattern" {
            self.stack.push(Value::Bool(string_glob::valid(text)?));
            return Ok(true);
        }
        if matches!(name, "md5" | "sha1" | "sha256" | "sha256Int") {
            return self.bytes_property(&text.to_java_utf8().as_bytes().to_vec().into(), name);
        }
        Ok(false)
    }
    pub(super) fn string_api_operation(
        &mut self,
        receiver: Value,
        operation: Operation,
    ) -> Result<(), RuntimeError> {
        let Value::String(text) = receiver else {
            return Err(invalid("String API receiver"));
        };
        let value = match operation {
            Operation::PredicateBegin { last } => return self.string_predicate_begin(&text, last),
            Operation::PredicateNext { last, drop } => {
                return self.string_predicate_next(text, last, drop);
            }
            Operation::PredicateAccept { last, drop } => {
                return self.string_predicate_accept(text, last, drop);
            }
            Operation::Lowercase => Value::String(string_lower::lowercase(&text)),
            Operation::Parse { kind, nullable } => {
                let parsed = match kind {
                    ParseKind::Int => string_parse::integer(&text).map(|n| Value::Int(n.into())),
                    ParseKind::Float => string_parse::float(&text).map(|n| Value::Float(n.into())),
                    ParseKind::Boolean => string_parse::boolean(&text).map(Value::Bool),
                };
                match parsed {
                    Some(value) => value,
                    None if nullable => Value::Null,
                    None => {
                        let kind = match kind {
                            ParseKind::Int => "Int",
                            ParseKind::Float => "Float",
                            ParseKind::Boolean => "Boolean",
                        };
                        let mut message = crate::pkl_string::PklStringBuilder::default();
                        message.push_str(&format!("Cannot parse string as `{kind}`.\nString: "));
                        message.push_value(&crate::diagnostic_values::render_string(
                            &Value::String(text),
                            &self.heap,
                            &self.program.module_name,
                            80,
                        )?);
                        return Err(RuntimeError::new(ErrorKind::Type, message.finish()));
                    }
                }
            }
            Operation::Pad { end } => {
                let Value::String(padding) = self.pop()? else {
                    return Err(invalid("checked padding Char"));
                };
                let Value::Int(width) = self.pop()? else {
                    return Err(invalid("checked padding width"));
                };
                let width = width.get();
                let length = text.code_point_len() as i64;
                if length >= width {
                    Value::String(text)
                } else {
                    if width > i64::from(i32::MAX) {
                        return Err(RuntimeError::new(
                            ErrorKind::Arithmetic,
                            format!(
                                "Int value `{}` is too large (only Int32 supported here).",
                                strings::diagnostic_integer(width)
                            ),
                        ));
                    }
                    let mut units = Vec::new();
                    let pad = padding.encode_utf16().collect::<Vec<_>>();
                    if end {
                        units.extend(text.encode_utf16());
                    }
                    for _ in length..width {
                        units.extend_from_slice(&pad);
                    }
                    if !end {
                        units.extend(text.encode_utf16());
                    }
                    Value::String(PklString::from_utf16(units))
                }
            }
            Operation::ReplaceRange => {
                let Value::String(replacement) = self.pop()? else {
                    return Err(invalid("checked replacement"));
                };
                let Value::Int(end) = self.pop()? else {
                    return Err(invalid("checked range end"));
                };
                let Value::Int(start) = self.pop()? else {
                    return Err(invalid("checked range start"));
                };
                if let Err((index, minimum)) = text.substring_code_points(start.get(), end.get()) {
                    let mut message = crate::pkl_string::PklStringBuilder::default();
                    message.push_str(&format!(
                        "Character index `{}` is out of range `{}`..`{}`.\nString: ",
                        strings::diagnostic_integer(index),
                        strings::diagnostic_integer(minimum),
                        strings::diagnostic_integer(text.code_point_len() as i64)
                    ));
                    message.push_value(&crate::diagnostic_values::render_string(
                        &Value::String(text),
                        &self.heap,
                        &self.program.module_name,
                        80,
                    )?);
                    return Err(RuntimeError::new(ErrorKind::Type, message.finish()));
                }
                let prefix = text
                    .substring_code_points(0, start.get())
                    .expect("checked range prefix");
                let suffix = text
                    .substring_code_points(end.get(), text.code_point_len() as i64)
                    .expect("checked range suffix");
                Value::String(PklString::from_utf16(
                    prefix
                        .encode_utf16()
                        .chain(replacement.encode_utf16())
                        .chain(suffix.encode_utf16())
                        .collect::<Vec<_>>(),
                ))
            }
            Operation::Capitalize { lower } => Value::String(first_case(&text, lower)),
            Operation::Trim { start, end, jdk } => Value::String(trimmed(&text, start, end, jdk)),
            Operation::GetOrNull | Operation::Take { .. } | Operation::Drop => {
                let Value::Int(count) = self.pop()? else {
                    return Err(invalid("checked String count"));
                };
                let count = count.get();
                let length = text.code_point_len() as i64;
                if matches!(operation, Operation::GetOrNull) {
                    if count < 0 || count >= length {
                        Value::Null
                    } else {
                        Value::String(
                            text.substring_code_points(count, count + 1)
                                .expect("String index bounds"),
                        )
                    }
                } else {
                    if count < 0 {
                        return Err(RuntimeError::new(
                            ErrorKind::Type,
                            format!(
                                "Expected a positive number, but got `{}`.",
                                strings::diagnostic_integer(count)
                            ),
                        ));
                    }
                    match operation {
                        Operation::Take { .. } if count > length => Value::String(text),
                        Operation::Drop if count > length => Value::String(empty()),
                        Operation::Take { last: false } => Value::String(
                            text.substring_code_points(0, count).expect("take bounds"),
                        ),
                        Operation::Take { last: true } => Value::String(
                            text.substring_code_points(length - count, length)
                                .expect("takeLast bounds"),
                        ),
                        Operation::Drop => Value::String(
                            text.substring_code_points(count, length)
                                .expect("drop bounds"),
                        ),
                        _ => return Err(invalid("String slice operation")),
                    }
                }
            }
        };
        self.stack.push(value);
        Ok(())
    }
}
