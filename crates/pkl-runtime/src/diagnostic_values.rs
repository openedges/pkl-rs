//! Bounded, non-forcing value snapshots for diagnostics.
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
    })
}

fn function_name(id: ObjectId, heap: &Heap) -> Result<String, RuntimeError> {
    let object = heap.get(id)?;
    let arity = object
        .function
        .as_ref()
        .map(|function| function.arity)
        .or_else(|| {
            object
                .native_function
                .as_ref()
                .map(|function| match function.name.as_ref() {
                    "Reference" => 3,
                    "containsKey" | "getOrNull" => 1,
                    _ => 0,
                })
        })
        .ok_or_else(|| {
            RuntimeError::new(ErrorKind::InvalidBytecode, "function has no definition")
        })?;
    Ok(format!("Function{arity}"))
}

pub(crate) fn type_name(value: &Value, heap: &Heap, module: &str) -> Result<String, RuntimeError> {
    Ok(match value {
        Value::Null | Value::NullWithDefault(_) => "Null".into(),
        Value::Bool(_) => "Boolean".into(),
        Value::Int(_) => "Int".into(),
        Value::Float(_) => "Float".into(),
        Value::String(_) => "String".into(),
        Value::Bytes(_) => "Bytes".into(),
        Value::Function(id) => function_name(*id, heap)?,
        Value::Class(_) => "Class".into(),
        Value::TypeAlias(_) => "TypeAlias".into(),
        Value::Object(id) => {
            if heap.get(*id)?.reference.is_some() {
                "pkl.ref#Reference".into()
            } else if heap.get(*id)?.ref_module {
                "pkl.ref".into()
            } else if heap.get(*id)?.is_module {
                module.into()
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
                            module
                        };
                        name = format!("{defining_module}#{}", class.name);
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
) -> Result<String, RuntimeError> {
    let mut renderer = Renderer {
        heap,
        module,
        units: Vec::new(),
        limit: limit.max(3),
        stopped: false,
    };
    renderer.value(value, false)?;
    // Java's UTF-8 writer replaces an unmatched surrogate with '?'. The
    // upstream UTF-16 length limit can split a supplementary character.
    Ok(char::decode_utf16(renderer.units)
        .map(|unit| unit.unwrap_or('?'))
        .collect())
}

struct Renderer<'a> {
    heap: &'a Heap,
    module: &'a str,
    units: Vec<u16>,
    limit: usize,
    stopped: bool,
}

impl Renderer<'_> {
    fn append(&mut self, text: &str) {
        if self.stopped {
            return;
        }
        self.units.extend(text.encode_utf16());
        if self.units.len() >= self.limit {
            self.units.truncate(self.limit - 3);
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

    fn value(&mut self, value: &Value, implicit: bool) -> Result<(), RuntimeError> {
        if self.stopped {
            return Ok(());
        }
        match value {
            Value::Null | Value::NullWithDefault(_) => self.append("null"),
            Value::Bool(value) => self.append(if *value { "true" } else { "false" }),
            Value::Int(value) => self.append(&value.to_string()),
            Value::Float(value) => self.append(&crate::numeric::double_to_string(*value)),
            Value::String(value) => {
                let mut text = String::from("\"");
                for ch in value.chars() {
                    match ch {
                        '\n' => text.push_str("\\n"),
                        '\r' => text.push_str("\\r"),
                        '\t' => text.push_str("\\t"),
                        '\\' => text.push_str("\\\\"),
                        '"' => text.push_str("\\\""),
                        ch => text.push(ch),
                    }
                }
                text.push('"');
                self.append(&text);
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
                    let (divisor, unit) = if bytes.len() >= 1_000_000_000 {
                        (1_000_000_000, "gb")
                    } else if bytes.len() >= 1_000_000 {
                        (1_000_000, "mb")
                    } else if bytes.len() >= 1_000 {
                        (1_000, "kb")
                    } else {
                        (1, "b")
                    };
                    let count = bytes.len() as f64 / divisor as f64;
                    self.append(&if count.fract() == 0.0 {
                        format!("{count:.0}")
                    } else if (count * 10.0).fract() == 0.0 {
                        format!("{count:.1}")
                    } else {
                        format!("{count:.2}")
                    });
                    self.append(".");
                    self.append(unit);
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
                self.append(name);
            }
            Value::Class(id) => {
                if let Some(ty) = &self.heap.get(*id)?.builtin_type {
                    self.append(&match ty {
                        crate::vm::TypeCheck::Reference { .. } => "pkl.ref#Reference".into(),
                        crate::vm::TypeCheck::Function { parameters, .. } => parameters
                            .as_ref()
                            .map_or_else(|| "Function".into(), |p| format!("Function{}", p.len())),
                        _ => crate::vm::reference::type_name(ty, self.module),
                    });
                    return Ok(());
                }
                let name = class_name(*id, self.heap)?.ok_or_else(|| {
                    RuntimeError::new(ErrorKind::InvalidBytecode, "class has no definition")
                })?;
                self.append(&format!("{}#{name}", self.module));
            }
            Value::Object(id) => self.object(*id, implicit)?,
        }
        Ok(())
    }

    fn object(&mut self, id: ObjectId, implicit: bool) -> Result<(), RuntimeError> {
        if let Some(reference) = &self.heap.get(id)?.reference {
            self.append("Reference(");
            self.value(&reference.domain, false)?;
            self.append(", ");
            self.append(&crate::vm::reference::type_name(
                &reference.referent,
                self.module,
            ));
            self.append(", ");
            self.value(&reference.data, false)?;
            self.append(")");
            for access in &reference.path {
                if self.stopped {
                    break;
                }
                let Value::Object(access) = access else {
                    return Err(RuntimeError::new(
                        ErrorKind::InvalidBytecode,
                        "Reference path contains a non-object access",
                    ));
                };
                let component = |name: &str| -> Result<&Value, RuntimeError> {
                    let key = CacheKey {
                        entry: false,
                        owner: None,
                        name: name.into(),
                        method: false,
                    };
                    match self.heap.get(*access)?.cache.get(&key) {
                        Some(LazyState::Completed(value)) => Ok(value),
                        _ => Err(RuntimeError::new(
                            ErrorKind::InvalidBytecode,
                            "Reference path component is not cached",
                        )),
                    }
                };
                if let Value::String(property) = component("property")? {
                    self.append(".");
                    self.identifier(property);
                } else {
                    let key = component("key")?;
                    self.append("[");
                    self.value(key, false)?;
                    self.append("]");
                }
            }
            return Ok(());
        }
        let kind = self.heap.get(id)?.collection_kind;
        if matches!(kind, CollectionKind::List | CollectionKind::Map) {
            self.append(if kind == CollectionKind::Map {
                "Map("
            } else {
                "List("
            });
            for (index, member) in crate::vm::collections::ordered_members(self.heap, id)?
                .into_iter()
                .enumerate()
            {
                if self.stopped {
                    return Ok(());
                }
                if index > 0 {
                    self.append(", ");
                }
                if kind == CollectionKind::Map {
                    self.value(&member.key, false)?;
                    self.append(", ");
                }
                let key = CacheKey {
                    entry: true,
                    owner: None,
                    name: member.name,
                    method: false,
                };
                match self.heap.get(id)?.cache.get(&key) {
                    Some(LazyState::Completed(value)) => self.value(value, false)?,
                    _ => self.append("?"),
                }
            }
            self.append(")");
            return Ok(());
        }
        if !implicit {
            self.append("new ");
            self.append(&object_name(id, self.heap)?);
            self.append(" ");
        }
        self.append("{");
        let mut any = false;
        for member in crate::vm::collections::ordered_members(self.heap, id)? {
            if self.stopped {
                return Ok(());
            }
            if !any {
                self.append(" ");
                any = true;
            }
            let key = CacheKey {
                entry: member.kind != CollectionMemberKind::Property,
                owner: None,
                name: member.name.clone(),
                method: false,
            };
            let value = match self.heap.get(id)?.cache.get(&key) {
                Some(LazyState::Completed(value)) => Some(value),
                _ => None,
            };
            match member.kind {
                CollectionMemberKind::Property => self.identifier(&member.name),
                CollectionMemberKind::Entry => {
                    self.append("[");
                    self.value(&member.key, false)?;
                    self.append("]");
                }
                CollectionMemberKind::Element => {}
            }
            if member.kind != CollectionMemberKind::Element {
                let object_like = match value {
                    Some(Value::Object(value)) => {
                        !matches!(
                            self.heap.get(*value)?.collection_kind,
                            CollectionKind::List | CollectionKind::Map
                        ) && self.heap.get(*value)?.reference.is_none()
                    }
                    Some(Value::Function(_)) => true,
                    _ => false,
                };
                self.append(if object_like { " " } else { " = " });
            }
            match value {
                Some(value) => self.value(value, member.kind != CollectionMemberKind::Element)?,
                None => self.append("?"),
            }
            self.append("; ");
        }
        if any && !self.stopped {
            self.units.truncate(self.units.len() - 2);
            self.append(" ");
        }
        self.append("}");
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
        assert_eq!(
            render(
                &Value::string(format!("{}😀suffix", "a".repeat(75))),
                &heap,
                "test",
                80
            )
            .unwrap(),
            format!("\"{}?...", "a".repeat(75))
        );
    }

    fn member(name: &str, local: bool) -> Member {
        Member {
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
            LazyState::Completed(Value::Int(1)),
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
        assert!(text.ends_with("..."));
        assert!(text.starts_with("new Dynamic { self { self {"));
        assert_eq!(
            render(&value, &Heap::new(), "test", 80).unwrap_err().kind,
            ErrorKind::StaleHandle
        );
    }
}
