//! String split/replacement uses the shared checked UTF-16 matcher. A mapped
//! replacement advances only after its ordinary guest call has returned.
use super::*;
use crate::pkl_regex::{PklRegex, RegexCursor, RegexMatchData};

#[derive(Clone, Copy, Debug)]
pub enum Mode {
    All,
    First,
    Last,
}
#[derive(Clone, Copy, Debug)]
pub enum Operation {
    Split { limited: bool },
    Replace(Mode),
    BeginMapped(Mode),
    NextMapped { end: usize },
    AppendMapped { next: usize },
    EndMapped,
}

pub(super) struct Run {
    cursor: RegexCursor,
    callback: Value,
    mode: Mode,
    visited: bool,
    current: Option<RegexMatchData>,
    current_object: Option<Value>,
    output: Vec<u16>,
    copied: usize,
}
impl Run {
    pub(super) fn roots(&self, roots: &mut Vec<Value>) {
        roots.push(self.callback.clone());
        roots.extend(self.current_object.iter().cloned());
    }
}
fn op(operation: Operation) -> Instruction {
    Instruction::Primitive(primitives::Operation::StringTransform(operation))
}

pub(super) fn blocks(
    blocks: &mut Vec<Vec<Instruction>>,
    metadata: &mut Vec<BlockMetadata>,
) -> BTreeMap<(Arc<str>, Arc<str>), usize> {
    let source = crate::stdlib_sources::base();
    let mut result = BTreeMap::new();
    for (name, operation) in [
        ("split", Operation::Split { limited: false }),
        ("splitLimit", Operation::Split { limited: true }),
        ("replaceAll", Operation::Replace(Mode::All)),
        ("replaceFirst", Operation::Replace(Mode::First)),
        ("replaceLast", Operation::Replace(Mode::Last)),
        ("replaceAllMapped", Operation::BeginMapped(Mode::All)),
        ("replaceFirstMapped", Operation::BeginMapped(Mode::First)),
        ("replaceLastMapped", Operation::BeginMapped(Mode::Last)),
    ] {
        let start = source
            .text
            .find(&format!("external function {name}("))
            .expect("String declaration");
        let closing = start + source.text[start..].find("): ").expect("method result");
        let end = closing + source.text[closing..].find('\n').expect("declaration end");
        let declaration = &source.text[start..end];
        let pattern = start + declaration.find("String | Regex").expect("pattern type");
        let arity = if matches!(operation, Operation::Split { limited: false }) {
            1
        } else {
            2
        };
        let mut code = vec![
            Instruction::LoadLocal(arity - 1),
            Instruction::Primitive(primitives::Operation::CheckStringPattern),
        ];
        let mut spans = vec![
            None,
            Some(Span::new(pattern, pattern + "String | Regex".len())),
        ];
        if arity == 2 {
            code.push(Instruction::LoadLocal(0));
            spans.push(None);
            let (ty, span) = if matches!(operation, Operation::Split { .. }) {
                let offset = start + declaration.find("Int(this > 0)").expect("limit type");
                let predicate = Span::new(offset + 4, offset + "Int(this > 0".len());
                let constraint_block = blocks.len();
                blocks.push(vec![
                    Instruction::LoadCustomThis,
                    Instruction::Observe(Span::new(predicate.start, predicate.start + 4)),
                    Instruction::Constant(Value::Int(0.into())),
                    Instruction::ObserveNativeLiteral {
                        span: Span::new(predicate.end - 1, predicate.end),
                        rendered: "0".into(),
                    },
                    Instruction::Greater,
                    Instruction::Observe(predicate),
                    Instruction::Return,
                ]);
                metadata.push(BlockMetadata {
                    source: Some(source.clone()),
                    member: format!("#String.{name}").into(),
                    declaration: Some(predicate),
                    spans: vec![None; 7],
                    inline_constraint: true,
                    ..Default::default()
                });
                (
                    TypeCheck::Constrained {
                        base: Box::new(TypeCheck::Int),
                        base_span: Span::new(offset, offset + 3),
                        constraints: vec![ConstraintCode {
                            block: constraint_block,
                            arguments: BTreeMap::new(),
                            argument_sources: BTreeMap::new(),
                            span: predicate,
                            module_environment: false,
                            source: "this > 0".into(),
                        }],
                    },
                    Span::new(offset, offset + "Int(this > 0)".len()),
                )
            } else if matches!(operation, Operation::BeginMapped(_)) {
                let offset = start
                    + declaration
                        .find("(RegexMatch) -> String")
                        .expect("mapper type");
                (
                    TypeCheck::Function {
                        parameters: Some(vec![builtins::ty(builtins::REGEX_MATCH)]),
                        result: Box::new(TypeCheck::String),
                        syntax: true,
                    },
                    Span::new(offset, offset + "(RegexMatch) -> String".len()),
                )
            } else {
                let offset = start
                    + declaration
                        .find("replacement: String")
                        .expect("replacement type")
                    + "replacement: ".len();
                (TypeCheck::String, Span::new(offset, offset + 6))
            };
            code.push(Instruction::CheckType(ty));
            spans.push(Some(span));
        }
        code.push(op(operation));
        spans.push(None);
        if matches!(operation, Operation::BeginMapped(_)) {
            let next = code.len();
            code.extend([
                op(Operation::NextMapped { end: next + 3 }),
                Instruction::Call {
                    arity: 1,
                    method: false,
                },
                op(Operation::AppendMapped { next }),
                op(Operation::EndMapped),
            ]);
            spans.extend([None; 4]);
        }
        code.push(Instruction::Return);
        spans.push(None);
        let block = blocks.len();
        blocks.push(code);
        metadata.push(BlockMetadata {
            source: Some(source.clone()),
            member: format!("#String.{name}").into(),
            declaration: Some(Span::new(start, end)),
            spans,
            ..Default::default()
        });
        result.insert(("String".into(), name.into()), block);
    }
    result
}

fn pattern(value: Value, unicode_case: bool) -> Result<PklRegex, RuntimeError> {
    match value {
        Value::Regex(regex) => Ok(regex),
        Value::String(text) => Ok(PklRegex::literal(text, unicode_case)),
        _ => Err(invalid("checked String pattern")),
    }
}
fn replacement_error(regex: &PklString, replacement: &PklString, message: String) -> RuntimeError {
    RuntimeError::new(
        ErrorKind::Type,
        PklString::join(
            &[
                "Error replacing matches for regex `".into(),
                regex.clone(),
                "` with `".into(),
                replacement.clone(),
                "`: `".into(),
                message.into(),
                "`".into(),
            ],
            "",
        ),
    )
}
fn literal_replace(input: &[u16], pattern: &[u16], replacement: &[u16], mode: Mode) -> Vec<u16> {
    let mut result = Vec::new();
    if pattern.is_empty() {
        match mode {
            Mode::First => {
                result.extend_from_slice(replacement);
                result.extend_from_slice(input);
            }
            Mode::Last => {
                result.extend_from_slice(input);
                result.extend_from_slice(replacement);
            }
            Mode::All => {
                result.extend_from_slice(replacement);
                for unit in input {
                    result.push(*unit);
                    result.extend_from_slice(replacement);
                }
            }
        }
        return result;
    }
    if matches!(mode, Mode::Last) {
        if let Some(start) = input
            .windows(pattern.len())
            .rposition(|window| window == pattern)
        {
            result.extend_from_slice(&input[..start]);
            result.extend_from_slice(replacement);
            result.extend_from_slice(&input[start + pattern.len()..]);
            return result;
        }
        return input.to_vec();
    }
    let mut copied = 0;
    while let Some(relative) = input[copied..]
        .windows(pattern.len())
        .position(|window| window == pattern)
    {
        let start = copied + relative;
        result.extend_from_slice(&input[copied..start]);
        result.extend_from_slice(replacement);
        copied = start + pattern.len();
        if matches!(mode, Mode::First) {
            break;
        }
    }
    result.extend_from_slice(&input[copied..]);
    result
}

impl Vm {
    #[inline(never)]
    pub(super) fn string_transform(
        &mut self,
        receiver: Value,
        operation: Operation,
    ) -> Result<(), RuntimeError> {
        let Value::String(text) = receiver else {
            return Err(invalid("String transform receiver"));
        };
        match operation {
            Operation::Split { limited } => {
                // Java narrows the already constrained Int to i32, including wraparound.
                let limit = if limited {
                    let Value::Int(limit) = self.pop()? else {
                        return Err(invalid("checked split limit"));
                    };
                    limit.get() as i32
                } else {
                    0
                };
                let regex = pattern(self.pop()?, false)?;
                let mut cursor = regex.cursor(&text);
                let mut strings = Vec::new();
                let mut copied = 0;
                let mut count = 0;
                while let Some(matched) = cursor
                    .find()
                    .map_err(|error| regex::backend_error(error, regex.pattern()))?
                {
                    if limit <= 0 || count < limit - 1 {
                        if copied == 0 && matched.start() == 0 && matched.end() == 0 {
                            continue;
                        }
                        strings.push(PklString::from_utf16(
                            cursor.input()[copied..matched.start()].to_vec(),
                        ));
                        copied = matched.end();
                        count += 1;
                    } else if count == limit - 1 {
                        strings.push(PklString::from_utf16(cursor.input()[copied..].to_vec()));
                        copied = matched.end();
                        count += 1;
                    }
                    // Pattern.split continues the matcher after reaching a positive limit.
                }
                if copied == 0 {
                    strings = vec![text];
                } else {
                    if limit <= 0 || count < limit {
                        strings.push(PklString::from_utf16(cursor.input()[copied..].to_vec()));
                    }
                    if limit == 0 {
                        while strings.last().is_some_and(PklString::is_empty) {
                            strings.pop();
                        }
                    }
                }
                let length = strings.len();
                self.stack.extend(strings.into_iter().map(Value::String));
                self.collection_instruction(Instruction::MakeList { length })?;
            }
            Operation::Replace(mode) => {
                let Value::String(replacement) = self.pop()? else {
                    return Err(invalid("checked replacement"));
                };
                let source = self.pop()?;
                let value = if let Value::String(literal) = source {
                    literal_replace(
                        &text.encode_utf16().collect::<Vec<_>>(),
                        &literal.encode_utf16().collect::<Vec<_>>(),
                        &replacement.encode_utf16().collect::<Vec<_>>(),
                        mode,
                    )
                } else {
                    let regex = pattern(source, false)?;
                    let mut cursor = regex.cursor(&text);
                    let mut output = Vec::new();
                    let mut copied = 0;
                    loop {
                        let matched = match mode {
                            Mode::Last => cursor.last_region(),
                            _ => cursor.find(),
                        }
                        .map_err(|error| regex::backend_error(error, regex.pattern()))?;
                        let Some(matched) = matched else {
                            break;
                        };
                        let expanded =
                            cursor.expand(&matched, &replacement).map_err(|message| {
                                replacement_error(regex.pattern(), &replacement, message)
                            })?;
                        output.extend_from_slice(&cursor.input()[copied..matched.start()]);
                        output.extend(expanded);
                        copied = matched.end();
                        if !matches!(mode, Mode::All) {
                            break;
                        }
                    }
                    output.extend_from_slice(&cursor.input()[copied..]);
                    output
                };
                self.stack.push(Value::String(PklString::from_utf16(value)));
            }
            Operation::BeginMapped(mode) => {
                let callback = self.pop()?;
                let regex = pattern(self.pop()?, true)?;
                self.frames.last_mut().unwrap().string_transform = Some(Box::new(Run {
                    cursor: regex.cursor(&text),
                    callback,
                    mode,
                    visited: false,
                    current: None,
                    current_object: None,
                    output: Vec::new(),
                    copied: 0,
                }));
            }
            Operation::NextMapped { end } => {
                let next = {
                    let run = self
                        .frames
                        .last_mut()
                        .unwrap()
                        .string_transform
                        .as_mut()
                        .ok_or_else(|| invalid("missing mapped cursor"))?;
                    if run.visited && !matches!(run.mode, Mode::All) {
                        None
                    } else {
                        let matched = match run.mode {
                            Mode::Last => run.cursor.last_region(),
                            _ => run.cursor.find(),
                        }
                        .map_err(|error| regex::backend_error(error, run.cursor.pattern()))?;
                        run.visited = true;
                        matched.map(|matched| (matched, run.callback.clone()))
                    }
                };
                if let Some((matched, callback)) = next {
                    let object = self.regex_match(matched.clone());
                    let run = self
                        .frames
                        .last_mut()
                        .unwrap()
                        .string_transform
                        .as_mut()
                        .unwrap();
                    run.current = Some(matched);
                    run.current_object = Some(object.clone());
                    self.stack.push(callback);
                    self.stack.push(object);
                } else {
                    self.jump(end)?;
                }
            }
            Operation::AppendMapped { next } => {
                let value = self.pop()?;
                let Value::String(replacement) = value else {
                    // ApplyVmFunction1Node.executeString has no authored type-node frame.
                    return Err(type_error(
                        &TypeCheck::String,
                        &value,
                        &self.program.module_name,
                        &self.heap,
                    )?);
                };
                let run = self
                    .frames
                    .last_mut()
                    .unwrap()
                    .string_transform
                    .as_mut()
                    .ok_or_else(|| invalid("missing mapped cursor"))?;
                let matched = run
                    .current
                    .take()
                    .ok_or_else(|| invalid("missing mapped match"))?;
                run.output
                    .extend_from_slice(&run.cursor.input()[run.copied..matched.start()]);
                for unit in replacement.encode_utf16() {
                    // Pinned first/last concatenate quoteReplacement's output directly;
                    // all uses appendReplacement, which consumes these quoting slashes.
                    if !matches!(run.mode, Mode::All) && matches!(unit, 0x24 | 0x5c) {
                        run.output.push(0x5c);
                    }
                    run.output.push(unit);
                }
                run.copied = matched.end();
                run.current_object = None;
                self.jump(next)?;
            }
            Operation::EndMapped => {
                let mut run = self
                    .frames
                    .last_mut()
                    .unwrap()
                    .string_transform
                    .take()
                    .ok_or_else(|| invalid("missing mapped cursor"))?;
                run.output
                    .extend_from_slice(&run.cursor.input()[run.copied..]);
                self.stack
                    .push(Value::String(PklString::from_utf16(run.output)));
            }
        }
        Ok(())
    }
}
