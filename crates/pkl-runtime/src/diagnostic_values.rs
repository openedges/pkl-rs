//! Non-forcing value snapshots for diagnostics and selected collection strings.
use crate::heap::{CacheKey, CollectionKind, CollectionMemberKind, Heap, LazyState};
use crate::value::{ErrorKind, ObjectId, RuntimeError, Value};
use std::collections::BTreeSet;

fn ancestry(id: ObjectId, heap: &Heap) -> Result<Vec<ObjectId>, RuntimeError> {
    let mut result = Vec::new();
    let mut seen = BTreeSet::new();
    let mut next = Some(id);
    while let Some(id) = next {
        if !seen.insert(id) {
            return Err(RuntimeError::new(ErrorKind::Cycle, "cyclic parent chain"));
        }
        result.push(id);
        next = heap.get(id)?.parent;
    }
    Ok(result)
}

fn class_name(id: ObjectId, heap: &Heap) -> Result<Option<String>, RuntimeError> {
    for id in ancestry(id, heap)? {
        if let Some(class) = &heap.get(id)?.class_definition {
            return Ok(Some(class.name.to_string()));
        }
    }
    Ok(None)
}

fn object_name(id: ObjectId, heap: &Heap) -> Result<String, RuntimeError> {
    if heap.get(id)?.is_module {
        return Ok("ModuleClass".into());
    }
    Ok(match heap.get(id)?.collection_kind {
        CollectionKind::Dynamic => class_name(id, heap)?.unwrap_or_else(|| "Dynamic".into()),
        CollectionKind::Listing => "Listing".into(),
        CollectionKind::Mapping => "Mapping".into(),
        CollectionKind::List => "List".into(),
        CollectionKind::Map => "Map".into(),
        CollectionKind::Set => "Set".into(),
    })
}

fn function_name(id: ObjectId, heap: &Heap) -> Result<String, RuntimeError> {
    Ok(format!("Function{}", function_arity(id, heap)?))
}

pub(crate) fn function_arity(id: ObjectId, heap: &Heap) -> Result<usize, RuntimeError> {
    let object = heap.get(id)?;
    object
        .function
        .as_ref()
        .map(|function| function.arity)
        .or_else(|| {
            object
                .native_function
                .as_ref()
                .map(|function| match function.name.as_ref() {
                    "Reference" => 3,
                    "contains" | "containsKey" | "getOrNull" => 1,
                    _ => 0,
                })
        })
        .ok_or_else(|| RuntimeError::new(ErrorKind::InvalidBytecode, "function has no definition"))
}

pub(crate) fn type_name(value: &Value, heap: &Heap, module: &str) -> Result<String, RuntimeError> {
    Ok(match value {
        Value::Null | Value::NullWithDefault(_) => "Null".into(),
        Value::Bool(_) => "Boolean".into(),
        Value::Int(_) => "Int".into(),
        Value::Float(_) => "Float".into(),
        Value::Duration(_) => "Duration".into(),
        Value::DataSize(_) => "DataSize".into(),
        Value::String(_) => "String".into(),
        Value::Bytes(_) => "Bytes".into(),
        Value::Function(id) => function_name(*id, heap)?,
        Value::Class(_) => "Class".into(),
        Value::TypeAlias(_) => "TypeAlias".into(),
        Value::Object(id) => {
            if heap.get(*id)?.pair.is_some() {
                "Pair".into()
            } else if heap.get(*id)?.reference.is_some() {
                "pkl.ref#Reference".into()
            } else if heap.get(*id)?.ref_module {
                "pkl.ref".into()
            } else if heap.get(*id)?.is_module {
                heap.get(*id)?
                    .class_definition
                    .as_ref()
                    .map_or(module, |class| class.name.as_ref())
                    .into()
            } else if heap.get(*id)?.collection_kind == CollectionKind::Dynamic {
                let mut name = "Dynamic".to_owned();
                for owner in ancestry(*id, heap)? {
                    if let Some(class) = &heap.get(owner)?.class_definition {
                        let defining_module = if matches!(
                            class.id,
                            crate::vm::reference::DOMAIN_ID | crate::vm::reference::ACCESS_ID
                        ) {
                            "pkl.ref"
                        } else {
                            class.module.as_deref().unwrap_or(module)
                        };
                        name = if crate::vm::builtins::name(class.id).is_some() {
                            class.name.to_string()
                        } else {
                            format!("{defining_module}#{}", class.name)
                        };
                        break;
                    }
                }
                name
            } else {
                object_name(*id, heap)?
            }
        }
    })
}

pub(crate) fn render(
    value: &Value,
    heap: &Heap,
    module: &str,
    limit: usize,
) -> Result<crate::value::PklString, RuntimeError> {
    render_string(value, heap, module, limit)
}

pub(crate) fn render_string(
    value: &Value,
    heap: &Heap,
    module: &str,
    limit: usize,
) -> Result<crate::value::PklString, RuntimeError> {
    render_with_layout(value, heap, module, limit, false)
}

/// Collection.toString uses a multiline layout, whereas diagnostics and Map
/// use the single-line layout. Neither layout forces uncached object members.
pub(crate) fn render_multiline(
    value: &Value,
    heap: &Heap,
    module: &str,
) -> Result<crate::value::PklString, RuntimeError> {
    render_with_layout(value, heap, module, i32::MAX as usize, true)
}

fn render_with_layout(
    value: &Value,
    heap: &Heap,
    module: &str,
    limit: usize,
    multiline: bool,
) -> Result<crate::value::PklString, RuntimeError> {
    let mut run = RenderRun::new(value.clone(), limit, multiline);
    while !run.step(heap, module)? {}
    Ok(run.finish())
}

enum RenderTask {
    Value(Value, bool, usize),
    Text(crate::value::PklString),
    Identifier(crate::value::PklString),
    Members {
        id: ObjectId,
        members: Vec<crate::vm::collections::RenderMember>,
        index: usize,
        depth: usize,
        strict: bool,
        hash_ready: bool,
        seen: Vec<(bool, Value, Option<i32>)>,
        emitted: usize,
        key_hash: Option<Option<i32>>,
        compare_index: usize,
        duplicate: bool,
    },
    Exit(ObjectId),
}
/// Shared iterative value formatting. VM callers service key hash/equality
/// requests; synchronous diagnostic snapshots currently cannot suspend for them.
pub(crate) struct RenderRun {
    tasks: Vec<RenderTask>,
    units: Vec<u16>,
    limit: usize,
    stopped: bool,
    multiline: bool,
    active: BTreeSet<ObjectId>,
}
impl RenderRun {
    pub(crate) fn new(value: Value, limit: usize, multiline: bool) -> Self {
        Self {
            tasks: vec![RenderTask::Value(value, false, 0)],
            units: Vec::new(),
            limit: limit.max(3),
            stopped: false,
            multiline,
            active: BTreeSet::new(),
        }
    }
    pub(crate) fn finish(self) -> crate::value::PklString {
        crate::value::PklString::from_utf16(self.units)
    }
    pub(crate) fn roots(&self, roots: &mut Vec<Value>) {
        for task in &self.tasks {
            match task {
                RenderTask::Value(value, ..) => roots.push(value.clone()),
                RenderTask::Members { id, members, .. } => {
                    roots.push(Value::Object(*id));
                    roots.extend(members.iter().map(|member| member.descriptor.key.clone()));
                }
                RenderTask::Exit(id) => roots.push(Value::Object(*id)),
                _ => {}
            }
        }
    }
    /// Runtime rendering services Java HashSet key hashing at this exact visit.
    /// Read-only diagnostic snapshots leave this request to the caller.
    pub(crate) fn take_key_to_prepare(&mut self) -> Option<Value> {
        if self.stopped {
            return None;
        }
        if let Some(RenderTask::Members {
            members,
            index,
            strict: false,
            hash_ready,
            ..
        }) = self.tasks.last_mut()
            && !*hash_ready
            && let Some(member) = members.get(*index)
        {
            *hash_ready = true;
            return Some(member.descriptor.key.clone());
        }
        None
    }
    pub(crate) fn record_key_hash(&mut self, hash: Option<i32>) {
        if let Some(RenderTask::Members { key_hash, .. }) = self.tasks.last_mut() {
            *key_hash = Some(hash);
        }
    }
    pub(crate) fn take_key_comparison(&mut self, heap: &Heap) -> Option<(Value, Value)> {
        let Some(RenderTask::Members {
            members,
            index,
            strict: false,
            key_hash: Some(hash),
            compare_index,
            duplicate,
            seen,
            ..
        }) = self.tasks.last_mut()
        else {
            return None;
        };
        if *duplicate {
            return None;
        }
        let member = &members.get(*index)?.descriptor;
        let entry = member.kind != CollectionMemberKind::Property;
        while let Some((old_entry, old_key, old_hash)) = seen.get(*compare_index) {
            *compare_index += 1;
            if *old_entry != entry {
                continue;
            }
            if crate::vm::equality::same_key(heap, &member.key, old_key) {
                *duplicate = true;
                return None;
            }
            // Some upstream key hashes contain JVM identity components. An
            // unknown numeric hash cannot rule out a collision; equality still
            // decides membership after both keys have completed hash forcing.
            if hash.is_none() || old_hash.is_none() || hash == old_hash {
                return Some((member.key.clone(), old_key.clone()));
            }
        }
        None
    }
    pub(crate) fn record_key_comparison(&mut self, equal: bool) {
        if equal && let Some(RenderTask::Members { duplicate, .. }) = self.tasks.last_mut() {
            *duplicate = true;
        }
    }
    pub(crate) fn step(&mut self, heap: &Heap, module: &str) -> Result<bool, RuntimeError> {
        if self.stopped {
            return Ok(true);
        }
        let Some(task) = self.tasks.pop() else {
            return Ok(true);
        };
        let mut renderer = Renderer {
            heap,
            module,
            state: self,
        };
        match task {
            RenderTask::Value(value, implicit, depth) => renderer.value(&value, implicit, depth)?,
            RenderTask::Text(text) => renderer.append_units(text.encode_utf16()),
            RenderTask::Identifier(text) => {
                if let Some(text) = text.as_str() {
                    renderer.identifier(text);
                } else {
                    renderer.append("`");
                    renderer.append_units(text.encode_utf16());
                    renderer.append("`");
                }
            }
            RenderTask::Members {
                id,
                members,
                index,
                depth,
                strict,
                seen,
                emitted,
                key_hash,
                duplicate,
                ..
            } => renderer.member(
                id, members, index, depth, strict, seen, emitted, key_hash, duplicate,
            )?,
            RenderTask::Exit(id) => {
                renderer.active.remove(&id);
            }
        }
        Ok(renderer.stopped || renderer.tasks.is_empty())
    }
}
struct Renderer<'a> {
    heap: &'a Heap,
    module: &'a str,
    state: &'a mut RenderRun,
}
impl std::ops::Deref for Renderer<'_> {
    type Target = RenderRun;
    fn deref(&self) -> &Self::Target {
        self.state
    }
}
impl std::ops::DerefMut for Renderer<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.state
    }
}
impl Renderer<'_> {
    fn append(&mut self, text: &str) {
        self.append_units(text.encode_utf16());
    }

    fn append_units(&mut self, units: impl IntoIterator<Item = u16>) {
        if self.stopped {
            return;
        }
        self.units.extend(units);
        if self.units.len() >= self.limit {
            let end = self.limit - 3;
            self.units.truncate(end);
            self.units.extend("...".encode_utf16());
            self.stopped = true;
        }
    }

    fn identifier(&mut self, name: &str) {
        if crate::parser::is_regular_identifier(name) {
            self.append(name);
        } else {
            self.append("`");
            self.append(name);
            self.append("`");
        }
    }

    fn value(&mut self, value: &Value, implicit: bool, depth: usize) -> Result<(), RuntimeError> {
        if self.stopped {
            return Ok(());
        }
        match value {
            Value::Null | Value::NullWithDefault(_) => self.append("null"),
            Value::Bool(value) => self.append(if *value { "true" } else { "false" }),
            Value::Int(value) => self.append(&value.to_string()),
            Value::Float(value) => self.append(&crate::numeric::double_to_string(**value)),
            Value::Duration(value) => self.append(&value.to_string()),
            Value::DataSize(value) => self.append(&value.to_string()),
            Value::String(value) => {
                self.append_units(value.quoted().encode_utf16());
            }
            Value::Bytes(bytes) => {
                self.append("Bytes(");
                for (index, value) in bytes.iter().take(8).enumerate() {
                    if index > 0 {
                        self.append(", ");
                    }
                    self.append(&value.to_string());
                }
                if bytes.len() > 8 {
                    self.append(", ... <total size: ");
                    let size = bytes.size();
                    let count = size.value();
                    self.append(&if count.fract() == 0.0 {
                        format!("{count:.0}")
                    } else if (count * 10.0).fract() == 0.0 {
                        format!("{count:.1}")
                    } else {
                        format!("{count:.2}")
                    });
                    self.append(".");
                    self.append(size.unit().symbol());
                    self.append(">");
                }
                self.append(")");
            }
            Value::Function(id) => {
                let name = function_name(*id, self.heap)?;
                if !implicit {
                    self.append("new ");
                    self.append(&name);
                    self.append(" ");
                }
                self.append("{}");
            }
            Value::TypeAlias(id) => {
                let name = self.heap.get(*id)?.type_alias.as_ref().ok_or_else(|| {
                    RuntimeError::new(ErrorKind::InvalidBytecode, "type alias has no definition")
                })?;
                // VmTypeAlias.toString uses the simple name for pkl.base.
                self.append(name.strip_prefix("pkl.base#").unwrap_or(name));
            }
            Value::Class(id) => {
                self.append(&self.heap.class_display_name(*id)?);
            }
            Value::Object(id) => self.object(*id, implicit, depth)?,
        }
        Ok(())
    }

    fn schedule(&mut self, tasks: impl IntoIterator<Item = RenderTask>) {
        let tasks: Vec<_> = tasks.into_iter().collect();
        self.tasks.extend(tasks.into_iter().rev());
    }
    fn object(&mut self, id: ObjectId, implicit: bool, depth: usize) -> Result<(), RuntimeError> {
        if self.limit >= i32::MAX as usize && !self.active.insert(id) {
            return Err(RuntimeError::new(
                ErrorKind::Cycle,
                "cyclic value rendering",
            ));
        }
        self.tasks.push(RenderTask::Exit(id));
        if let Some(pair) = &self.heap.get(id)?.pair {
            self.schedule([
                RenderTask::Text("Pair(".into()),
                RenderTask::Value(pair[0].clone(), false, depth),
                RenderTask::Text(", ".into()),
                RenderTask::Value(pair[1].clone(), false, depth),
                RenderTask::Text(")".into()),
            ]);
            return Ok(());
        }
        if let Some(reference) = &self.heap.get(id)?.reference {
            let mut tasks = vec![
                RenderTask::Text("Reference(".into()),
                RenderTask::Value(reference.domain.clone(), false, depth),
                RenderTask::Text(", ".into()),
                RenderTask::Text(crate::vm::reference::type_name_value(
                    &reference.referent,
                    self.module,
                )),
                RenderTask::Text(", ".into()),
                RenderTask::Value(reference.data.clone(), false, depth),
                RenderTask::Text(")".into()),
            ];
            for access in &reference.path {
                let Value::Object(access) = access else {
                    return Err(RuntimeError::new(
                        ErrorKind::InvalidBytecode,
                        "Reference path contains a non-object access",
                    ));
                };
                let component = |name: &str| -> Result<Value, RuntimeError> {
                    let key = CacheKey {
                        entry: false,
                        owner: None,
                        name: name.into(),
                        method: false,
                    };
                    match self.heap.get(*access)?.cache.get(&key) {
                        Some(LazyState::Completed(value)) => Ok(value.clone()),
                        _ => Err(RuntimeError::new(
                            ErrorKind::InvalidBytecode,
                            "Reference path component is not cached",
                        )),
                    }
                };
                if let Value::String(property) = component("property")? {
                    tasks.push(RenderTask::Text(".".into()));
                    tasks.push(RenderTask::Identifier(property));
                } else {
                    tasks.push(RenderTask::Text("[".into()));
                    tasks.push(RenderTask::Value(component("key")?, false, depth));
                    tasks.push(RenderTask::Text("]".into()));
                }
            }
            self.schedule(tasks);
            return Ok(());
        }
        let kind = self.heap.get(id)?.collection_kind;
        let strict = matches!(
            kind,
            CollectionKind::List | CollectionKind::Map | CollectionKind::Set
        );
        let members = if strict {
            self.append(match kind {
                CollectionKind::Map => "Map(",
                CollectionKind::Set => "Set(",
                _ => "List(",
            });
            crate::vm::collections::ordered_members(self.heap, id)?
                .into_iter()
                .map(|descriptor| crate::vm::collections::RenderMember {
                    descriptor,
                    skip: false,
                    code: None,
                })
                .collect()
        } else {
            if !implicit {
                self.append("new ");
                self.append(&object_name(id, self.heap)?);
                self.append(" ");
            }
            self.append("{");
            crate::vm::collections::render_members(self.heap, id)?
        };
        self.tasks.push(RenderTask::Members {
            id,
            members,
            index: 0,
            depth,
            strict,
            hash_ready: false,
            seen: Vec::new(),
            emitted: 0,
            key_hash: None,
            compare_index: 0,
            duplicate: false,
        });
        Ok(())
    }
    #[allow(clippy::too_many_arguments)]
    fn member(
        &mut self,
        id: ObjectId,
        members: Vec<crate::vm::collections::RenderMember>,
        index: usize,
        depth: usize,
        strict: bool,
        mut seen: Vec<(bool, Value, Option<i32>)>,
        emitted: usize,
        key_hash: Option<Option<i32>>,
        duplicate: bool,
    ) -> Result<(), RuntimeError> {
        let Some(render_member) = members.get(index).cloned() else {
            if strict {
                self.append(")");
            } else {
                if emitted > 0 {
                    if self.multiline {
                        self.append("\n");
                        self.append(&"  ".repeat(depth));
                    } else {
                        self.append(" ");
                    }
                }
                self.append("}");
            }
            return Ok(());
        };
        let member = render_member.descriptor;
        let entry = member.kind != CollectionMemberKind::Property;
        let already_seen = !strict
            && if key_hash.is_some() {
                duplicate
            } else {
                seen.iter().any(|(old_entry, key, _)| {
                    *old_entry == entry
                        && crate::vm::equality::same_key(self.heap, key, &member.key)
                })
            };
        if !strict && !already_seen {
            seen.push((entry, member.key.clone(), key_hash.flatten()));
        }
        let skip = already_seen || render_member.skip;
        self.tasks.push(RenderTask::Members {
            id,
            members,
            index: index + 1,
            depth,
            strict,
            hash_ready: false,
            seen,
            emitted: emitted + usize::from(!skip),
            key_hash: None,
            compare_index: 0,
            duplicate: false,
        });
        if skip {
            return Ok(());
        }
        let value = crate::vm::collections::cached_render_value(self.heap, id, &member)?;
        let mut tasks = Vec::new();
        if strict {
            if emitted > 0 {
                self.append(", ");
            }
            if self.heap.get(id)?.collection_kind == CollectionKind::Map {
                tasks.push(RenderTask::Value(member.key, false, depth));
                tasks.push(RenderTask::Text(", ".into()));
            }
        } else {
            if self.multiline {
                self.append("\n");
                self.append(&"  ".repeat(depth + 1));
            } else {
                self.append(if emitted == 0 { " " } else { "; " });
            }
            match member.kind {
                CollectionMemberKind::Property => {
                    tasks.push(RenderTask::Identifier(member.name.clone().into()))
                }
                CollectionMemberKind::Entry => {
                    tasks.push(RenderTask::Text("[".into()));
                    tasks.push(RenderTask::Value(member.key, false, depth + 1));
                    tasks.push(RenderTask::Text("]".into()));
                }
                CollectionMemberKind::Element => {}
            }
            if member.kind != CollectionMemberKind::Element {
                let object_like = match &value {
                    Some(Value::Object(id)) => {
                        !matches!(
                            self.heap.get(*id)?.collection_kind,
                            CollectionKind::List | CollectionKind::Map | CollectionKind::Set
                        ) && self.heap.get(*id)?.reference.is_none()
                    }
                    Some(Value::Function(_)) => true,
                    _ => false,
                };
                tasks.push(RenderTask::Text(
                    if object_like { " " } else { " = " }.into(),
                ));
            }
        }
        tasks.push(match value {
            Some(value) => RenderTask::Value(
                value,
                !strict && member.kind != CollectionMemberKind::Element,
                if strict { depth } else { depth + 1 },
            ),
            None => RenderTask::Text("?".into()),
        });
        self.schedule(tasks);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::heap::{Class, Member, Object};

    #[test]
    fn strings_use_pkl_escapes_and_utf16_length_boundaries() {
        let heap = Heap::new();
        assert_eq!(
            render(&Value::string("\0\u{1}\t\n\r\"\\"), &heap, "test", 80).unwrap(),
            "\"\0\u{1}\\t\\n\\r\\\"\\\\\""
        );
        assert_eq!(
            render(&Value::string("x".repeat(77)), &heap, "test", 80).unwrap(),
            format!("\"{}\"", "x".repeat(77))
        );
        assert_eq!(
            render(&Value::string("x".repeat(78)), &heap, "test", 80).unwrap(),
            format!("\"{}...", "x".repeat(76))
        );
        let truncated = render(
            &Value::string(format!("{}😀suffix", "a".repeat(75))),
            &heap,
            "test",
            80,
        )
        .unwrap();
        let mut expected: Vec<_> = format!("\"{}", "a".repeat(75)).encode_utf16().collect();
        expected.push(0xd83d);
        expected.extend("...".encode_utf16());
        // Truncation is measured in UTF-16 units. Encoding the detached result
        // at the byte boundary retains the previous Java UTF-8 output.
        assert_eq!(truncated.encode_utf16().collect::<Vec<_>>(), expected);
        assert_eq!(
            truncated.to_java_utf8(),
            format!("\"{}?...", "a".repeat(75))
        );
    }

    fn member(name: &str, local: bool) -> Member {
        Member {
            flags: 0,
            entry: false,
            name: name.into(),
            code: usize::MAX, // Rendering must not execute any code.
            local,
            method: false,
            ty: None,
            type_span: None,
            constant: false,
            fixed: false,
        }
    }

    #[test]
    fn inherited_order_uses_receiver_cache_without_forcing_members() {
        let mut heap = Heap::new();
        let base = heap.allocate(Object {
            members: vec![
                member("z", false),
                member("private", true),
                member("a", false),
            ],
            ..Object::default()
        });
        let class = heap.allocate(Object {
            class_definition: Some(Class {
                module: None,
                id: 1,
                name: "Child".into(),
                open: false,
                abstract_: false,
            }),
            parent: Some(base),
            members: vec![member("m", false)],
            ..Object::default()
        });
        let object = heap.allocate(Object {
            parent: Some(class),
            members: vec![member("a", false)],
            ..Object::default()
        });
        let value = Value::Object(object);
        assert_eq!(type_name(&value, &heap, "test").unwrap(), "test#Child");
        assert_eq!(
            render(&value, &heap, "test", 80).unwrap(),
            "new Child { z = ?; a = ?; m = ? }"
        );
        heap.get_mut(object).unwrap().cache.insert(
            CacheKey {
                entry: false,
                owner: None,
                name: "a".into(),
                method: false,
            },
            LazyState::Completed(Value::Int(1.into())),
        );
        let before = heap.get(object).unwrap().cache.clone();
        assert_eq!(
            render(&value, &heap, "test", 80).unwrap(),
            "new Child { z = ?; a = 1; m = ? }"
        );
        assert_eq!(heap.get(object).unwrap().cache, before);
    }

    #[test]
    fn cached_cycles_stop_at_the_same_length_limit_and_stale_handles_fail() {
        let mut heap = Heap::new();
        let object = heap.allocate(Object {
            members: vec![member("self", false)],
            ..Object::default()
        });
        heap.get_mut(object).unwrap().cache.insert(
            CacheKey {
                entry: false,
                owner: None,
                name: "self".into(),
                method: false,
            },
            LazyState::Completed(Value::Object(object)),
        );
        let value = Value::Object(object);
        let text = render(&value, &heap, "test", 80).unwrap();
        assert_eq!(text.encode_utf16().count(), 80);
        assert!(text.as_str().is_some_and(|text| text.ends_with("...")));
        assert!(text.starts_with("new Dynamic { self { self {"));
        assert_eq!(
            render(&value, &Heap::new(), "test", 80).unwrap_err().kind,
            ErrorKind::StaleHandle
        );
    }
}
