//! Resumable List/Set folds, extrema, and the pinned eight-element-block merge sort.
use super::*;
use crate::heap::CollectionKind;

#[derive(Clone, Copy, Debug)]
pub enum Kind {
    Fold,
    FoldBack,
    FoldIndexed,
    Reduce,
    ReduceOrNull,
    Min,
    MinOrNull,
    Max,
    MaxOrNull,
    MinBy,
    MinByOrNull,
    MaxBy,
    MaxByOrNull,
    MinWith,
    MinWithOrNull,
    MaxWith,
    MaxWithOrNull,
    Sort,
    SortBy,
    SortWith,
}
impl Kind {
    fn name(self) -> &'static str {
        match self {
            Self::Fold => "fold",
            Self::FoldBack => "foldBack",
            Self::FoldIndexed => "foldIndexed",
            Self::Reduce => "reduce",
            Self::ReduceOrNull => "reduceOrNull",
            Self::Min => "min",
            Self::MinOrNull => "minOrNull",
            Self::Max => "max",
            Self::MaxOrNull => "maxOrNull",
            Self::MinBy => "minBy",
            Self::MinByOrNull => "minByOrNull",
            Self::MaxBy => "maxBy",
            Self::MaxByOrNull => "maxByOrNull",
            Self::MinWith => "minWith",
            Self::MinWithOrNull => "minWithOrNull",
            Self::MaxWith => "maxWith",
            Self::MaxWithOrNull => "maxWithOrNull",
            Self::Sort => "sort",
            Self::SortBy => "sortBy",
            Self::SortWith => "sortWith",
        }
    }
    fn fold(self) -> bool {
        matches!(
            self,
            Self::Fold | Self::FoldBack | Self::FoldIndexed | Self::Reduce | Self::ReduceOrNull
        )
    }
    fn sort(self) -> bool {
        matches!(self, Self::Sort | Self::SortBy | Self::SortWith)
    }
    fn selector(self) -> bool {
        matches!(
            self,
            Self::MinBy | Self::MinByOrNull | Self::MaxBy | Self::MaxByOrNull | Self::SortBy
        )
    }
    fn comparator(self) -> bool {
        matches!(
            self,
            Self::MinWith
                | Self::MinWithOrNull
                | Self::MaxWith
                | Self::MaxWithOrNull
                | Self::SortWith
        )
    }
    fn maximum(self) -> bool {
        matches!(
            self,
            Self::Max | Self::MaxOrNull | Self::MaxBy | Self::MaxByOrNull
        )
    }
    fn nullable(self) -> bool {
        matches!(
            self,
            Self::ReduceOrNull
                | Self::MinOrNull
                | Self::MaxOrNull
                | Self::MinByOrNull
                | Self::MaxByOrNull
                | Self::MinWithOrNull
                | Self::MaxWithOrNull
        )
    }
    pub(super) fn arity(self) -> usize {
        if matches!(self, Self::Fold | Self::FoldBack | Self::FoldIndexed) {
            2
        } else {
            usize::from(self.fold() || self.selector() || self.comparator())
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub enum Operation {
    Start(Kind),
    Step,
}

pub(super) fn blocks(
    blocks: &mut Vec<Vec<Instruction>>,
    metadata: &mut Vec<BlockMetadata>,
) -> BTreeMap<(Arc<str>, Arc<str>), usize> {
    use Kind::*;
    let source = crate::stdlib_sources::base();
    let mut result = BTreeMap::new();
    for class_name in ["Set", "List"] {
        let class = source
            .text
            .find(&format!("external class {class_name}<"))
            .unwrap();
        for kind in [
            Fold,
            FoldBack,
            FoldIndexed,
            Reduce,
            ReduceOrNull,
            Min,
            MinOrNull,
            Max,
            MaxOrNull,
            MinBy,
            MinByOrNull,
            MaxBy,
            MaxByOrNull,
            MinWith,
            MinWithOrNull,
            MaxWith,
            MaxWithOrNull,
            Sort,
            SortBy,
            SortWith,
        ] {
            let property = matches!(kind, Min | MinOrNull | Max | MaxOrNull);
            let name = kind.name();
            let needle = if property {
                format!("external {name}:")
            } else {
                format!("external function {name}")
            };
            let start = class + source.text[class..].find(&needle).expect("pinned Set API");
            let end = if property {
                start + source.text[start..].find('\n').unwrap()
            } else {
                let open = start + source.text[start..].find('(').unwrap();
                let mut depth = 0;
                let mut closing = open;
                for (i, c) in source.text[open..].char_indices() {
                    if c == '(' {
                        depth += 1;
                    }
                    if c == ')' {
                        depth -= 1;
                        if depth == 0 {
                            closing = open + i;
                            break;
                        }
                    }
                }
                closing + source.text[closing..].find('\n').unwrap()
            };
            let mut code = vec![];
            let mut spans = vec![];
            if kind.arity() == 2 {
                code.push(Instruction::LoadLocal(1));
                spans.push(None);
            }
            if kind.arity() != 0 {
                let label = if kind.fold() {
                    "operator: "
                } else if kind.selector() {
                    "selector: "
                } else {
                    "comparator: "
                };
                let parameter = start + source.text[start..end].find(label).unwrap() + label.len();
                let arrow = parameter + source.text[parameter..end].find(" -> ").unwrap();
                let type_end = arrow + 4 + source.text[arrow + 4..end].find([')', '\n']).unwrap();
                let type_end =
                    arrow + 4 + source.text[arrow + 4..type_end].trim_end_matches(',').len();
                code.extend([
                    Instruction::LoadLocal(0),
                    Instruction::CheckType(TypeCheck::Function {
                        parameters: Some(vec![
                            TypeCheck::Any;
                            if matches!(kind, FoldIndexed) {
                                3
                            } else if kind.fold() || kind.comparator() {
                                2
                            } else {
                                1
                            }
                        ]),
                        result: Box::new(TypeCheck::Any),
                        syntax: true,
                    }),
                ]);
                spans.extend([None, Some(Span::new(parameter, type_end))]);
            }
            code.push(Instruction::Primitive(primitives::Operation::SetOrder(
                Operation::Start(kind),
            )));
            spans.push(None);
            let step = code.len();
            code.extend([
                Instruction::Primitive(primitives::Operation::SetOrder(Operation::Step)),
                Instruction::Jump(step),
                Instruction::Return,
            ]);
            spans.extend([None, None, None]);
            let block = blocks.len();
            blocks.push(code);
            metadata.push(BlockMetadata {
                source: Some(source.clone()),
                member: format!("#{class_name}.{name}").into(),
                declaration: Some(Span::new(start, end)),
                spans,
                ..Default::default()
            });
            result.insert((class_name.into(), name.into()), block);
        }
    }
    result
}

#[derive(Clone, Copy)]
enum Stage {
    Next,
    FoldResult,
    InitialKey,
    CandidateKey,
    ComparatorResult,
    SortLeft,
    SortRight,
    SortResult,
    Emit,
}
#[derive(Clone)]
pub(super) struct Run {
    kind: Kind,
    source: ObjectId,
    list: bool,
    values: Vec<Value>,
    callback: Option<ObjectId>,
    cursor: usize,
    accumulator: Value,
    key: Value,
    current: Value,
    left: Value,
    right: Value,
    stage: Stage,
    sort: Option<SortRun>,
    builder: Option<ObjectId>,
}
impl Run {
    pub(super) fn roots(&self, roots: &mut Vec<Value>) {
        roots.push(Value::Object(self.source));
        roots.extend(self.values.iter().cloned());
        roots.extend(self.callback.map(Value::Function));
        roots.extend([
            self.accumulator.clone(),
            self.key.clone(),
            self.current.clone(),
            self.left.clone(),
            self.right.clone(),
        ]);
        roots.extend(self.builder.map(Value::Object));
        if let Some(sort) = &self.sort {
            roots.extend(sort.temp.iter().cloned());
        }
    }
}

#[derive(Clone, Copy)]
enum SortPhase {
    Insertion {
        block: usize,
        i: usize,
        j: usize,
    },
    MergeStart {
        stride: usize,
        start: usize,
    },
    Merge {
        stride: usize,
        start: usize,
        mid: usize,
        end: usize,
        i: usize,
        j: usize,
        k: usize,
    },
    Done,
}
#[derive(Clone, Copy)]
enum SortAction {
    Insert {
        block: usize,
        i: usize,
        j: usize,
    },
    Shortcut {
        stride: usize,
        start: usize,
        mid: usize,
        end: usize,
    },
    Choose {
        stride: usize,
        start: usize,
        mid: usize,
        end: usize,
        i: usize,
        j: usize,
        k: usize,
    },
}
#[derive(Clone)]
struct SortRun {
    phase: SortPhase,
    temp: Vec<Value>,
    pending: Option<SortAction>,
}
impl SortRun {
    // One cursor movement or comparison per VM step; selectors are not cached.
    fn next(&mut self, values: &mut [Value]) -> Option<(Value, Value)> {
        let length = values.len();
        match self.phase {
            SortPhase::Insertion { block, i, j } => {
                let end = (block + 8).min(length);
                if block >= length {
                    self.phase = SortPhase::MergeStart {
                        stride: 8,
                        start: 0,
                    };
                } else if i >= end {
                    self.phase = SortPhase::Insertion {
                        block: block + 8,
                        i: block + 8,
                        j: block + 8,
                    };
                } else if j <= block {
                    self.phase = SortPhase::Insertion {
                        block,
                        i: i + 1,
                        j: i + 1,
                    };
                } else {
                    self.pending = Some(SortAction::Insert { block, i, j });
                    return Some((values[j].clone(), values[j - 1].clone()));
                }
            }
            SortPhase::MergeStart { stride, start } => {
                if stride >= length {
                    self.phase = SortPhase::Done;
                } else if start >= length - stride {
                    self.phase = SortPhase::MergeStart {
                        stride: stride * 2,
                        start: 0,
                    };
                } else {
                    let mid = start + stride;
                    let end = (start + stride * 2).min(length);
                    self.pending = Some(SortAction::Shortcut {
                        stride,
                        start,
                        mid,
                        end,
                    });
                    return Some((values[mid - 1].clone(), values[mid].clone()));
                }
            }
            SortPhase::Merge {
                stride,
                start,
                mid,
                end,
                mut i,
                mut j,
                k,
            } => {
                if k >= end {
                    self.phase = SortPhase::MergeStart {
                        stride,
                        start: start + stride * 2,
                    };
                } else if i >= mid {
                    values[k] = self.temp[j].clone();
                    j += 1;
                    self.phase = SortPhase::Merge {
                        stride,
                        start,
                        mid,
                        end,
                        i,
                        j,
                        k: k + 1,
                    };
                } else if j >= end {
                    values[k] = self.temp[i].clone();
                    i += 1;
                    self.phase = SortPhase::Merge {
                        stride,
                        start,
                        mid,
                        end,
                        i,
                        j,
                        k: k + 1,
                    };
                } else {
                    self.pending = Some(SortAction::Choose {
                        stride,
                        start,
                        mid,
                        end,
                        i,
                        j,
                        k,
                    });
                    return Some((self.temp[j].clone(), self.temp[i].clone()));
                }
            }
            SortPhase::Done => {}
        }
        None
    }
    fn accept(&mut self, values: &mut [Value], less: bool) {
        match self.pending.take().expect("pending sort comparison") {
            SortAction::Insert { block, i, j } => {
                if less {
                    values.swap(j, j - 1);
                    self.phase = SortPhase::Insertion { block, i, j: j - 1 };
                } else {
                    self.phase = SortPhase::Insertion {
                        block,
                        i: i + 1,
                        j: i + 1,
                    };
                }
            }
            SortAction::Shortcut {
                stride,
                start,
                mid,
                end,
            } => {
                if less {
                    self.phase = SortPhase::MergeStart {
                        stride,
                        start: start + stride * 2,
                    };
                } else {
                    self.temp[start..end].clone_from_slice(&values[start..end]);
                    self.phase = SortPhase::Merge {
                        stride,
                        start,
                        mid,
                        end,
                        i: start,
                        j: mid,
                        k: start,
                    };
                }
            }
            SortAction::Choose {
                stride,
                start,
                mid,
                end,
                mut i,
                mut j,
                k,
            } => {
                if less {
                    values[k] = self.temp[j].clone();
                    j += 1;
                } else {
                    values[k] = self.temp[i].clone();
                    i += 1;
                }
                self.phase = SortPhase::Merge {
                    stride,
                    start,
                    mid,
                    end,
                    i,
                    j,
                    k: k + 1,
                };
            }
        }
    }
}

impl Vm {
    pub(super) fn set_order(
        &mut self,
        receiver: Value,
        operation: Operation,
    ) -> Result<(), RuntimeError> {
        if let Operation::Start(kind) = operation {
            let callback = if kind.arity() != 0 {
                let Value::Function(id) = self.pop()? else {
                    return Err(invalid("checked Set callback"));
                };
                Some(id)
            } else {
                None
            };
            let initial = if kind.arity() == 2 {
                Some(self.pop()?)
            } else {
                None
            };
            let source = receiver.as_object()?;
            let list = self.is_list(&receiver);
            let mut values = collections::ordered_members(&self.heap, source)?
                .iter()
                .map(|m| {
                    collections::cached_render_value(&self.heap, source, m)?
                        .ok_or_else(|| invalid("strict Set value"))
                })
                .collect::<Result<Vec<_>, _>>()?;
            if matches!(kind, Kind::FoldBack) {
                values.reverse();
            }
            if values.is_empty() && !kind.sort() && initial.is_none() {
                if !kind.nullable() {
                    return Err(RuntimeError::new(
                        ErrorKind::Type,
                        if list {
                            "Expected a non-empty collection.\nCollection: List()"
                        } else {
                            "Expected a non-empty collection.\nCollection: Set()"
                        },
                    ));
                }
                self.stack.push(Value::Null);
                let end = self.program.blocks[self.frames.last().unwrap().block].len() - 1;
                return self.jump(end);
            }
            let (accumulator, cursor) = if let Some(initial) = initial {
                (initial, 0)
            } else if kind.sort() {
                (Value::Null, 0)
            } else {
                (values[0].clone(), 1)
            };
            let sort = kind.sort().then(|| SortRun {
                phase: SortPhase::Insertion {
                    block: 0,
                    i: 0,
                    j: 0,
                },
                temp: vec![Value::Null; values.len()],
                pending: None,
            });
            let stage = if kind.selector() && !kind.sort() {
                Stage::InitialKey
            } else {
                Stage::Next
            };
            let run = Run {
                kind,
                source,
                list,
                values,
                callback,
                cursor,
                accumulator,
                key: Value::Null,
                current: Value::Null,
                left: Value::Null,
                right: Value::Null,
                stage,
                sort,
                builder: None,
            };
            let initial_arg = run.accumulator.clone();
            self.frames.last_mut().unwrap().set_order = Some(Box::new(run));
            if matches!(stage, Stage::InitialKey) {
                self.invoke_function(callback.unwrap(), vec![initial_arg], false)?;
            }
            return Ok(());
        }
        let frame_index = self.frames.len() - 1;
        let mut run = *self.frames[frame_index]
            .set_order
            .take()
            .ok_or_else(|| invalid("Set order cursor"))?;
        let mut call = None;
        let mut done = None;
        match run.stage {
            Stage::FoldResult => {
                run.accumulator = self.pop()?;
                run.stage = Stage::Next;
            }
            Stage::InitialKey => {
                run.key = self.pop()?;
                run.stage = Stage::Next;
            }
            Stage::CandidateKey => {
                let key = self.pop()?;
                if order(
                    &run.kind,
                    &key,
                    &run.key,
                    &self.heap,
                    &self.program.module_name,
                )? {
                    run.accumulator = run.current.clone();
                    run.key = key;
                }
                run.stage = Stage::Next;
            }
            Stage::ComparatorResult => {
                let value = self.pop()?;
                let accepted = if run.list {
                    let Value::Bool(accepted) = value else {
                        return Err(type_error(
                            &TypeCheck::Boolean,
                            &value,
                            &self.program.module_name,
                            &self.heap,
                        )?);
                    };
                    accepted
                } else {
                    comparator(value, &self.heap, &self.program.module_name)?
                };
                if accepted {
                    run.accumulator = run.current.clone();
                }
                run.stage = Stage::Next;
            }
            Stage::SortLeft => {
                run.key = self.pop()?;
                run.stage = Stage::SortRight;
                call = Some(vec![run.right.clone()]);
            }
            Stage::SortRight => {
                let right = self.pop()?;
                let less = order(
                    &Kind::Sort,
                    &run.key,
                    &right,
                    &self.heap,
                    &self.program.module_name,
                )?;
                run.sort.as_mut().unwrap().accept(&mut run.values, less);
                run.stage = Stage::Next;
            }
            Stage::SortResult => {
                let less = comparator(self.pop()?, &self.heap, &self.program.module_name)?;
                run.sort.as_mut().unwrap().accept(&mut run.values, less);
                run.stage = Stage::Next;
            }
            Stage::Emit => {
                if run.cursor == run.values.len() {
                    let builder = run.builder.unwrap();
                    done = Some(Value::Object(self.finish_strict_collection(builder)?));
                } else {
                    let value = run.values[run.cursor].clone();
                    run.cursor += 1;
                    let builder = run.builder.unwrap();
                    self.frames[frame_index].set_order = Some(Box::new(run));
                    self.frames[frame_index].builders.push(builder);
                    self.stack.push(value);
                    let result = self.append_strict_element();
                    self.frames[frame_index].builders.pop();
                    return result;
                }
            }
            Stage::Next => {
                if run.kind.sort() {
                    let sort = run.sort.as_mut().unwrap();
                    if let Some((left, right)) = sort.next(&mut run.values) {
                        if run.kind.selector() {
                            run.left = left.clone();
                            run.right = right;
                            run.stage = Stage::SortLeft;
                            call = Some(vec![left]);
                        } else if run.kind.comparator() {
                            run.stage = Stage::SortResult;
                            call = Some(vec![left, right]);
                        } else {
                            let less = order(
                                &Kind::Sort,
                                &left,
                                &right,
                                &self.heap,
                                &self.program.module_name,
                            )?;
                            sort.accept(&mut run.values, less);
                        }
                    } else if matches!(sort.phase, SortPhase::Done) {
                        run.builder = Some(self.heap.allocate(Object {
                            collection_kind: CollectionKind::List,
                            ..Default::default()
                        }));
                        run.cursor = 0;
                        run.stage = Stage::Emit;
                    }
                } else if run.cursor == run.values.len() {
                    done = Some(run.accumulator.clone());
                } else {
                    run.current = run.values[run.cursor].clone();
                    let index = run.cursor;
                    run.cursor += 1;
                    if run.kind.fold() {
                        run.stage = Stage::FoldResult;
                        call = Some(if matches!(run.kind, Kind::FoldIndexed) {
                            vec![
                                Value::Int((index as i64).into()),
                                run.accumulator.clone(),
                                run.current.clone(),
                            ]
                        } else if matches!(run.kind, Kind::FoldBack) {
                            vec![run.current.clone(), run.accumulator.clone()]
                        } else {
                            vec![run.accumulator.clone(), run.current.clone()]
                        });
                    } else if run.kind.selector() {
                        run.stage = Stage::CandidateKey;
                        call = Some(vec![run.current.clone()]);
                    } else if run.kind.comparator() {
                        run.stage = Stage::ComparatorResult;
                        call = Some(
                            if matches!(run.kind, Kind::MaxWithOrNull)
                                || (run.list && matches!(run.kind, Kind::MaxWith))
                            {
                                vec![run.accumulator.clone(), run.current.clone()]
                            } else {
                                vec![run.current.clone(), run.accumulator.clone()]
                            },
                        );
                    } else if order(
                        &run.kind,
                        &run.current,
                        &run.accumulator,
                        &self.heap,
                        &self.program.module_name,
                    )? {
                        run.accumulator = run.current.clone();
                    }
                }
            }
        }
        if let Some(value) = done {
            self.stack.push(value);
            let end = self.program.blocks[self.frames[frame_index].block].len() - 1;
            self.jump(end)?;
        } else {
            let callback = run.callback;
            self.frames[frame_index].set_order = Some(Box::new(run));
            if let Some(arguments) = call {
                self.invoke_function(callback.unwrap(), arguments, false)?;
            }
        }
        Ok(())
    }
}
fn order(
    kind: &Kind,
    left: &Value,
    right: &Value,
    heap: &Heap,
    module: &str,
) -> Result<bool, RuntimeError> {
    let Value::Bool(result) = binary(
        if kind.maximum() {
            &Instruction::Greater
        } else {
            &Instruction::Less
        },
        left.clone(),
        right.clone(),
        heap,
        module,
    )?
    else {
        return Err(invalid("comparison result"));
    };
    Ok(result)
}
fn comparator(value: Value, heap: &Heap, module: &str) -> Result<bool, RuntimeError> {
    match value {
        Value::Bool(value) => Ok(value),
        Value::Int(value) => Ok(value.get() < 0),
        value => {
            // Java uses the two-class typeMismatch overload, not a union type.
            let message = if value.is_null() {
                "Expected value of type `Boolean` or `Int`, but got `null`.".into()
            } else {
                PklString::join(
                    &[
                        "Expected value of type `Boolean` or `Int`, but got type `".into(),
                        crate::diagnostic_values::type_name(&value, heap, module)?.into(),
                        "`.\nValue: ".into(),
                        crate::diagnostic_values::render_string(&value, heap, module, 80)?,
                    ],
                    "",
                )
            };
            Err(RuntimeError::new(ErrorKind::Type, message))
        }
    }
}
pub(super) fn method_arity(name: &str) -> Option<usize> {
    Some(match name {
        "fold" | "foldBack" | "foldIndexed" => 2,
        "reduce" | "reduceOrNull" | "minBy" | "minByOrNull" | "maxBy" | "maxByOrNull"
        | "minWith" | "minWithOrNull" | "maxWith" | "maxWithOrNull" | "sortBy" | "sortWith" => 1,
        "sort" => 0,
        _ => return None,
    })
}
