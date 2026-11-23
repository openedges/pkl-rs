//! List range/replacement and index-search operations in ordinary resumable frames.
use super::*;

#[derive(Clone, Copy, Debug)]
pub enum Kind {
    Sublist,
    SublistOrNull,
    Rest,
    RestOrNull,
    Replace,
    ReplaceOrNull,
    ReplaceRange,
    ReplaceRangeOrNull,
    FindIndex,
    FindIndexOrNull,
    FindLastIndex,
    FindLastIndexOrNull,
    IndexOf,
    IndexOfOrNull,
    LastIndexOf,
    LastIndexOfOrNull,
}
impl Kind {
    fn name(self) -> &'static str {
        match self {
            Self::Sublist => "sublist",
            Self::SublistOrNull => "sublistOrNull",
            Self::Rest => "rest",
            Self::RestOrNull => "restOrNull",
            Self::Replace => "replace",
            Self::ReplaceOrNull => "replaceOrNull",
            Self::ReplaceRange => "replaceRange",
            Self::ReplaceRangeOrNull => "replaceRangeOrNull",
            Self::FindIndex => "findIndex",
            Self::FindIndexOrNull => "findIndexOrNull",
            Self::FindLastIndex => "findLastIndex",
            Self::FindLastIndexOrNull => "findLastIndexOrNull",
            Self::IndexOf => "indexOf",
            Self::IndexOfOrNull => "indexOfOrNull",
            Self::LastIndexOf => "lastIndexOf",
            Self::LastIndexOfOrNull => "lastIndexOfOrNull",
        }
    }
    fn nullable(self) -> bool {
        matches!(
            self,
            Self::SublistOrNull
                | Self::RestOrNull
                | Self::ReplaceOrNull
                | Self::ReplaceRangeOrNull
                | Self::FindIndexOrNull
                | Self::FindLastIndexOrNull
                | Self::IndexOfOrNull
                | Self::LastIndexOfOrNull
        )
    }
    fn predicate(self) -> bool {
        matches!(
            self,
            Self::FindIndex
                | Self::FindIndexOrNull
                | Self::FindLastIndex
                | Self::FindLastIndexOrNull
        )
    }
    fn search(self) -> bool {
        self.predicate()
            || matches!(
                self,
                Self::IndexOf | Self::IndexOfOrNull | Self::LastIndexOf | Self::LastIndexOfOrNull
            )
    }
    fn last(self) -> bool {
        matches!(
            self,
            Self::FindLastIndex
                | Self::FindLastIndexOrNull
                | Self::LastIndexOf
                | Self::LastIndexOfOrNull
        )
    }
    fn arity(self) -> usize {
        match self {
            Self::Rest | Self::RestOrNull => 0,
            Self::Sublist | Self::SublistOrNull | Self::Replace | Self::ReplaceOrNull => 2,
            Self::ReplaceRange | Self::ReplaceRangeOrNull => 3,
            _ => 1,
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub enum Operation {
    Start(Kind),
    Step,
}

#[cfg_attr(target_arch = "wasm32", inline(never))]
pub(super) fn blocks(
    blocks: &mut Vec<Vec<Instruction>>,
    metadata: &mut Vec<BlockMetadata>,
) -> BTreeMap<(Arc<str>, Arc<str>), usize> {
    use Kind::*;
    let source = crate::stdlib_sources::base();
    let class = source.text.find("external class List<").unwrap();
    let mut result = BTreeMap::new();
    for kind in [
        Sublist,
        SublistOrNull,
        Rest,
        RestOrNull,
        Replace,
        ReplaceOrNull,
        ReplaceRange,
        ReplaceRangeOrNull,
        FindIndex,
        FindIndexOrNull,
        FindLastIndex,
        FindLastIndexOrNull,
        IndexOf,
        IndexOfOrNull,
        LastIndexOf,
        LastIndexOfOrNull,
    ] {
        let name = kind.name();
        let needle = if kind.arity() == 0 {
            format!("external {name}:")
        } else {
            format!("external function {name}")
        };
        let start = class + source.text.entire().range(class..).find(&needle).unwrap();
        let end = if kind.arity() == 0 {
            start + source.text.entire().range(start..).find("\n").unwrap()
        } else {
            let open = start + source.text.entire().range(start..).find("(").unwrap();
            let mut depth = 0;
            let mut close = open;
            for (i, c) in source
                .text
                .entire()
                .range(open..)
                .as_str()
                .expect("pinned builtin source is scalar")
                .char_indices()
            {
                if c == '(' {
                    depth += 1;
                } else if c == ')' {
                    depth -= 1;
                    if depth == 0 {
                        close = open + i;
                        break;
                    }
                }
            }
            close + source.text.entire().range(close..).find("\n").unwrap()
        };
        let parameters: Vec<(&str, Option<TypeCheck>)> = if kind.predicate() {
            vec![(
                "predicate: ",
                Some(TypeCheck::Function {
                    parameters: Some(vec![TypeCheck::Any]),
                    result: Box::new(TypeCheck::Any),
                    syntax: true,
                }),
            )]
        } else {
            match kind {
                Rest | RestOrNull => vec![],
                Sublist | SublistOrNull => vec![
                    ("start: ", Some(TypeCheck::Int)),
                    ("exclusiveEnd: ", Some(TypeCheck::Int)),
                ],
                Replace | ReplaceOrNull => {
                    vec![("index: ", Some(TypeCheck::Int)), ("replacement: ", None)]
                }
                ReplaceRange | ReplaceRangeOrNull => vec![
                    ("start: ", Some(TypeCheck::Int)),
                    ("exclusiveEnd: ", Some(TypeCheck::Int)),
                    (
                        "replacement: ",
                        Some(TypeCheck::CollectionBase { arguments: vec![] }),
                    ),
                ],
                _ => vec![("element: ", None)],
            }
        };
        let mut code = Vec::new();
        let mut spans = Vec::new();
        for (index, (label, ty)) in parameters.iter().enumerate() {
            code.push(Instruction::LoadLocal(parameters.len() - index - 1));
            spans.push(None);
            if let Some(ty) = ty {
                let from = start
                    + source.text.entire().range(start..end).find(label).unwrap()
                    + label.len();
                let to = if kind.predicate() {
                    from + "(Element) -> Boolean".len()
                } else if matches!(ty, TypeCheck::Int) {
                    from + 3
                } else {
                    from + "Collection<Other>".len()
                };
                code.push(Instruction::CheckType(ty.clone()));
                spans.push(Some(Span::new(from, to)));
            }
        }
        code.push(Instruction::Primitive(primitives::Operation::ListOps(
            Operation::Start(kind),
        )));
        spans.push(None);
        let step = code.len();
        code.extend([
            Instruction::Primitive(primitives::Operation::ListOps(Operation::Step)),
            Instruction::Jump(step),
            Instruction::Return,
        ]);
        spans.extend([None, None, None]);
        let block = blocks.len();
        blocks.push(code);
        metadata.push(BlockMetadata {
            source: Some(source.clone()),
            member: format!("#List.{name}").into(),
            declaration: Some(Span::new(start, end)),
            spans,
            ..Default::default()
        });
        result.insert(("List".into(), name.into()), block);
    }
    result
}

#[derive(Clone)]
pub(super) struct Run {
    kind: Kind,
    source: ObjectId,
    values: Vec<Value>,
    argument: Value,
    builder: Option<ObjectId>,
    next: usize,
    index: usize,
    awaiting: bool,
    step: usize,
}
impl Run {
    pub(super) fn roots(&self, roots: &mut Vec<Value>) {
        roots.push(Value::Object(self.source));
        roots.extend(self.values.iter().cloned());
        roots.push(self.argument.clone());
        roots.extend(self.builder.map(Value::Object));
    }
}
fn values(vm: &Vm, source: ObjectId) -> Result<Vec<Value>, RuntimeError> {
    let object = vm.heap.get(source)?;
    if let Some(elems) = &object.list_elements {
        return Ok(elems.clone());
    }
    collections::ordered_members(&vm.heap, source)?
        .iter()
        .map(|m| {
            collections::cached_render_value(&vm.heap, source, m)?
                .ok_or_else(|| invalid("strict List value"))
        })
        .collect()
}
fn index_error(
    vm: &Vm,
    receiver: &Value,
    index: i64,
    min: i64,
    max: i64,
) -> Result<RuntimeError, RuntimeError> {
    let rendered =
        crate::diagnostic_values::render_string(receiver, &vm.heap, &vm.program.module_name, 80)?;
    Ok(RuntimeError::with_program_values(
        ErrorKind::Arithmetic,
        format!(
            "Element index `{}` is out of range `{}`..`{}`.",
            strings::diagnostic_integer(index),
            strings::diagnostic_integer(min),
            strings::diagnostic_integer(max)
        ),
        &[("Collection", rendered)],
    ))
}
impl Vm {
    fn list_ops_return(&mut self, step: usize, value: Value) -> Result<(), RuntimeError> {
        self.stack.push(value);
        self.jump(step + 2)
    }
    pub(super) fn list_ops(
        &mut self,
        receiver: Value,
        operation: Operation,
    ) -> Result<(), RuntimeError> {
        use Kind::*;
        if let Operation::Start(kind) = operation {
            let mut arguments = Vec::with_capacity(kind.arity());
            for _ in 0..kind.arity() {
                arguments.push(self.pop()?);
            }
            arguments.reverse();
            let source = receiver.as_object()?;
            let input = values(self, source)?;
            let len = input.len() as i64;
            let step = self.frames.last().unwrap().pc;
            let integer = |index: usize| -> Result<i64, RuntimeError> {
                match arguments[index] {
                    Value::Int(value) => Ok(value.get()),
                    _ => Err(invalid("checked List index")),
                }
            };
            let mut selected = Vec::new();
            if matches!(kind, Rest | RestOrNull) {
                if input.is_empty() {
                    if kind.nullable() {
                        return self.list_ops_return(step, Value::Null);
                    }
                    return Err(RuntimeError::with_program_values(
                        ErrorKind::Type,
                        "Expected a non-empty collection.",
                        &[("Collection", "List()".into())],
                    ));
                }
                selected.extend_from_slice(&input[1..]);
            } else if matches!(kind, Replace | ReplaceOrNull) {
                let index = integer(0)?;
                if index < 0 || index >= len {
                    if kind.nullable() {
                        return self.list_ops_return(step, Value::Null);
                    }
                    return Err(index_error(self, &receiver, index, 0, len - 1)?);
                }
                selected = input;
                selected[index as usize] = arguments[1].clone();
            } else if matches!(
                kind,
                Sublist | SublistOrNull | ReplaceRange | ReplaceRangeOrNull
            ) {
                let start = integer(0)?;
                let end = integer(1)?;
                if start < 0 || start > len {
                    if kind.nullable() {
                        return self.list_ops_return(step, Value::Null);
                    }
                    return Err(index_error(self, &receiver, start, 0, len)?);
                }
                if end < start || end > len {
                    if kind.nullable() {
                        return self.list_ops_return(step, Value::Null);
                    }
                    return Err(index_error(self, &receiver, end, start, len)?);
                }
                if matches!(kind, ReplaceRange | ReplaceRangeOrNull) {
                    selected.extend_from_slice(&input[..start as usize]);
                    selected.extend(values(self, arguments[2].as_object()?)?);
                    selected.extend_from_slice(&input[end as usize..]);
                } else {
                    selected.extend_from_slice(&input[start as usize..end as usize]);
                }
            } else {
                selected = input;
            }
            if !kind.search() {
                let built = self.build_strict_list(selected);
                return self.list_ops_return(step, Value::Object(built));
            }
            let builder = None;
            self.frames.last_mut().unwrap().list_ops = Some(Box::new(Run {
                kind,
                source,
                values: selected,
                argument: arguments.first().cloned().unwrap_or(Value::Null),
                builder,
                next: 0,
                index: 0,
                awaiting: false,
                step,
            }));
            return Ok(());
        }
        let frame = self.frames.len() - 1;
        let mut run = *self.frames[frame]
            .list_ops
            .take()
            .ok_or_else(|| invalid("List operation cursor"))?;
        if run.awaiting {
            let value = self.pop()?;
            let Value::Bool(matched) = value else {
                return Err(type_error(
                    &TypeCheck::Boolean,
                    &value,
                    &self.program.module_name,
                    &self.heap,
                )?);
            };
            run.awaiting = false;
            if matched {
                return self.list_ops_return(run.step, Value::Int((run.index as i64).into()));
            }
        }
        if run.next == run.values.len() {
            let value = if let Some(builder) = run.builder {
                Value::Object(self.finish_strict_collection(builder)?)
            } else if run.kind.nullable() {
                Value::Null
            } else {
                let headline = if run.kind.predicate() {
                    "Cannot find matching element in collection."
                } else {
                    "Cannot find element in collection."
                };
                let mut program_values = Vec::with_capacity(2);
                if !run.kind.predicate() {
                    program_values.push((
                        "Element",
                        crate::diagnostic_values::render_string(
                            &run.argument,
                            &self.heap,
                            &self.program.module_name,
                            80,
                        )?,
                    ));
                }
                program_values.push((
                    "Collection",
                    crate::diagnostic_values::render_string(
                        &Value::Object(run.source),
                        &self.heap,
                        &self.program.module_name,
                        80,
                    )?,
                ));
                return Err(RuntimeError::with_program_values(
                    ErrorKind::Type,
                    headline,
                    &program_values,
                ));
            };
            return self.list_ops_return(run.step, value);
        }
        let index = if run.kind.last() {
            run.values.len() - run.next - 1
        } else {
            run.next
        };
        let value = run.values[index].clone();
        if let Some(builder) = run.builder {
            run.next += 1;
            self.frames[frame].list_ops = Some(Box::new(run));
            self.frames[frame].builders.push(builder);
            self.stack.push(value);
            let result = self.append_strict_element();
            self.frames[frame].builders.pop();
            return result;
        }
        if run.kind.predicate() {
            let Value::Function(callback) = run.argument else {
                return Err(invalid("checked List predicate"));
            };
            run.index = index;
            run.next += 1;
            run.awaiting = true;
            self.frames[frame].list_ops = Some(Box::new(run));
            return self.invoke_function(callback, vec![value], false);
        }
        // Paguro UnmodList.indexOf/lastIndexOf use Objects.equals(stored, query).
        // This is deliberately boxed equality without a hash request.
        let query = run.argument.clone();
        self.frames[frame].list_ops = Some(Box::new(run));
        if self.search_key(&value, vec![query.clone()])? {
            return Ok(());
        }
        run = *self.frames[frame].list_ops.take().unwrap();
        if equality::same_key(&self.heap, &value, &query) {
            return self.list_ops_return(run.step, Value::Int((index as i64).into()));
        }
        run.next += 1;
        self.frames[frame].list_ops = Some(Box::new(run));
        Ok(())
    }
}
pub(super) fn method_arity(name: &str) -> Option<usize> {
    Some(match name {
        "sublist" | "sublistOrNull" | "replace" | "replaceOrNull" => 2,
        "replaceRange" | "replaceRangeOrNull" => 3,
        "findIndex"
        | "findIndexOrNull"
        | "findLastIndex"
        | "findLastIndexOrNull"
        | "indexOf"
        | "indexOfOrNull"
        | "lastIndexOf"
        | "lastIndexOfOrNull" => 1,
        _ => return None,
    })
}
