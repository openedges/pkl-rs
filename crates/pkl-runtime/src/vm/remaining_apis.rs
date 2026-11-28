//! Four concrete original-corpus first failures; no generic numeric rewrite.
use super::*;
use crate::heap::CollectionKind;

#[derive(Clone, Copy, Debug)]
pub enum Operation {
    FloatToInt,
    IntToRadixString,
    ListGetOrNull,
    SetRestOrNull,
    SetRest,
}

#[cfg_attr(target_arch = "wasm32", inline(never))]
pub(super) fn blocks(
    blocks: &mut Vec<Vec<Instruction>>,
    metadata: &mut Vec<BlockMetadata>,
) -> BTreeMap<(Arc<str>, Arc<str>), usize> {
    let source = crate::stdlib_sources::base();
    let mut result = BTreeMap::new();
    for (class, name, declaration, operation, parameter) in [
        (
            "Float",
            "toInt",
            "external function toInt(): Int",
            Operation::FloatToInt,
            None,
        ),
        (
            "Int",
            "toRadixString",
            "external function toRadixString(radix: Int(this.isBetween(2, 36))): String",
            Operation::IntToRadixString,
            Some("radix: "),
        ),
        (
            "List",
            "getOrNull",
            "external function getOrNull(index: Int): Element?",
            Operation::ListGetOrNull,
            Some("index: "),
        ),
        (
            "Set",
            "rest",
            "external rest: Set<Element>",
            Operation::SetRest,
            None,
        ),
        (
            "Set",
            "restOrNull",
            "external restOrNull: Set<Element>?",
            Operation::SetRestOrNull,
            None,
        ),
    ] {
        let anchor = if matches!(class, "List" | "Set") {
            format!("class {class}<")
        } else {
            format!("class {class} ")
        };
        let class_start = source.text.find(&anchor).expect("pinned class");
        let start = class_start
            + source
                .text
                .entire()
                .range(class_start..)
                .find(declaration)
                .expect("pinned API");
        let mut code = Vec::new();
        let mut spans = Vec::new();
        if let Some(parameter) = parameter {
            let int_start = start + declaration.find(parameter).unwrap() + parameter.len();
            let int_span = Span::new(int_start, int_start + 3);
            let ty = if matches!(operation, Operation::IntToRadixString) {
                // Use the ordinary resumable type-constraint path, including
                // declaration frames and configured power-assertion tracking.
                let predicate = "this.isBetween(2, 36)";
                let body_start = int_start + "Int(".len();
                let body = Span::new(body_start, body_start + predicate.len());
                let this_span = Span::new(body_start, body_start + 4);
                let constraint = blocks.len();
                blocks.push(vec![
                    Instruction::LoadCustomThis,
                    Instruction::Observe(this_span),
                    Instruction::GetMethod("isBetween".into()),
                    Instruction::Constant(Value::Int(2.into())),
                    Instruction::Constant(Value::Int(36.into())),
                    Instruction::Call {
                        arity: 2,
                        method: true,
                    },
                    Instruction::Observe(body),
                    Instruction::Return,
                ]);
                metadata.push(BlockMetadata {
                    source: Some(source.clone()),
                    member: "#Int.toRadixString".into(),
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
                        section: None,
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
            code.extend([
                Instruction::LoadLocal(0),
                Instruction::CheckType(Box::new(ty)),
            ]);
            spans.extend([None, Some(int_span)]);
        }
        code.extend([
            Instruction::Primitive(super::Operation::Remaining(operation)),
            Instruction::Return,
        ]);
        spans.extend([None, None]);
        let block = blocks.len();
        blocks.push(code);
        metadata.push(BlockMetadata {
            source: Some(source.clone()),
            member: format!("#{class}.{name}").into(),
            declaration: Some(Span::new(start, start + declaration.len())),
            spans,
            ..Default::default()
        });
        result.insert((class.into(), name.into()), block);
    }
    result
}

impl Vm {
    #[cfg_attr(target_arch = "wasm32", inline(never))]
    pub(super) fn remaining_api(
        &mut self,
        receiver: Value,
        operation: Operation,
    ) -> Result<(), RuntimeError> {
        match operation {
            Operation::FloatToInt => {
                let Value::Float(value) = receiver else {
                    return Err(invalid("Float.toInt receiver"));
                };
                let value = value.get();
                if !value.is_finite()
                    || !(-9223372036854775808.0..9223372036854775808.0).contains(&value)
                {
                    let displayed = crate::numeric::double_to_string(value);
                    return Err(RuntimeError::new(
                        ErrorKind::Arithmetic,
                        if value.is_finite() {
                            format!(
                                "Cannot convert Float `{displayed}` to Int because it is too large."
                            )
                        } else {
                            format!("Cannot convert non-finite Float `{displayed}` to Int.")
                        },
                    ));
                }
                // Rust truncates toward zero; the explicit finite/range check
                // above prevents its saturating-cast behavior from leaking.
                self.stack.push(Value::Int((value as i64).into()));
            }
            Operation::IntToRadixString => {
                let Value::Int(value) = receiver else {
                    return Err(invalid("Int.toRadixString receiver"));
                };
                let Value::Int(radix) = self.pop()? else {
                    return Err(invalid("checked radix type"));
                };
                let radix = radix.get();
                if !(2..=36).contains(&radix) {
                    return Err(invalid("unchecked radix constraint"));
                }
                let mut magnitude = value.get().unsigned_abs();
                let mut digits = Vec::new();
                loop {
                    let digit = (magnitude % radix as u64) as u8;
                    digits.push(if digit < 10 {
                        b'0' + digit
                    } else {
                        b'a' + digit - 10
                    });
                    magnitude /= radix as u64;
                    if magnitude == 0 {
                        break;
                    }
                }
                if value.get() < 0 {
                    digits.push(b'-');
                }
                digits.reverse();
                self.stack.push(Value::String(
                    String::from_utf8(digits)
                        .expect("ASCII radix digits")
                        .into(),
                ));
            }
            Operation::ListGetOrNull => {
                let Value::Int(index) = self.pop()? else {
                    return Err(invalid("checked List index"));
                };
                let index = index.get();
                let id = receiver.as_object()?;
                let obj = self.heap.get(id)?;
                let total_len = obj.length;
                let value = if index < 0 || index as u64 >= total_len as u64 {
                    Value::Null
                } else if let Some(elems) = &obj.list_elements {
                    elems.get(index as usize).cloned().unwrap_or(Value::Null)
                } else {
                    let idx = index as usize;
                    let member = if idx < obj.collection_members.len() {
                        &obj.collection_members[idx]
                    } else if !obj.collection_members.is_empty() {
                        let base_len = obj
                            .repeat_base_len
                            .unwrap_or(obj.collection_members.len())
                            .max(1);
                        &obj.collection_members[idx % base_len]
                    } else {
                        return Err(invalid("empty collection members with positive length"));
                    };
                    collections::cached_render_value(&self.heap, id, member)?
                        .ok_or_else(|| invalid("uncached strict List element"))?
                };
                // Return the stored Value itself, preserving heap identity.
                self.stack.push(value);
            }
            Operation::SetRestOrNull | Operation::SetRest => {
                let source = receiver.as_object()?;
                let members = collections::ordered_members(&self.heap, source)?;
                if members.is_empty() {
                    if matches!(operation, Operation::SetRest) {
                        let rendered = crate::diagnostic_values::render_string(
                            &receiver,
                            &self.heap,
                            &self.program.module_name,
                            80,
                        )?;
                        return Err(RuntimeError::with_program_values(
                            ErrorKind::Type,
                            "Expected a non-empty collection.",
                            &[("Collection", rendered)],
                        ));
                    }
                    self.stack.push(Value::Null);
                    return Ok(());
                }
                let first = collections::cached_render_value(&self.heap, source, &members[0])?
                    .ok_or_else(|| invalid("uncached strict Set first element"))?;
                self.begin_set_rest(source, first);
            }
        }
        Ok(())
    }

    pub(in crate::vm) fn prepare_set_rest_result(
        &mut self,
        source: ObjectId,
        removed: Option<(usize, Option<i32>)>,
    ) -> Result<(ObjectId, Vec<Value>), RuntimeError> {
        let members = collections::ordered_members(&self.heap, source)?;
        // VmSet.create selects EMPTY solely from the visible element vector,
        // even when a changed key hash left an entry in the persistent table.
        if members.len() == 1 {
            let empty = self.empty_strict_collection(CollectionKind::Set);
            return Ok((empty, Vec::new()));
        }
        let old = self.heap.get(source)?;
        let index = match removed {
            Some((slot, hash)) => old.map_index().without_matched(hash, slot),
            None => old.map_index().clone(),
        };
        let entries = old
            .set_entries
            .iter()
            .enumerate()
            .filter(|(slot, _)| removed.is_none_or(|(removed, _)| *slot != removed))
            .map(|(_, value)| value.clone())
            .collect();
        // The ordered vector always drops its first entry, including the
        // upstream edge case where a recomputed key hash cannot find that key.
        let values = members
            .iter()
            .skip(1)
            .map(|member| {
                collections::cached_render_value(&self.heap, source, member)?
                    .ok_or_else(|| invalid("uncached strict Set element"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let target = self.heap.allocate(Object {
            collection_kind: CollectionKind::Set,
            map_index: Some(Box::new(index)),
            set_entries: entries,
            ..Object::default()
        });
        self.frames
            .last_mut()
            .ok_or_else(|| invalid("Set.restOrNull frame"))?
            .builders
            .push(target);
        Ok((target, values))
    }
}
