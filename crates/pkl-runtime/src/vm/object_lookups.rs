//! Lazy property/entry lookup for the object forms returned by conversions.
use super::*;
use crate::heap::{CollectionKind, CollectionMemberKind};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lookup {
    HasProperty,
    Property,
    PropertyOrNull,
    EntryOrNull,
    EntryOrDefault,
}

pub(super) fn blocks(
    blocks: &mut Vec<Vec<Instruction>>,
    metadata: &mut Vec<BlockMetadata>,
) -> BTreeMap<(Arc<str>, Arc<str>), (usize, usize)> {
    let source = crate::stdlib_sources::base();
    let mut result = BTreeMap::new();
    for (class, name, operation) in [
        ("Dynamic", "hasProperty", Lookup::HasProperty),
        ("Dynamic", "getProperty", Lookup::Property),
        ("Dynamic", "getPropertyOrNull", Lookup::PropertyOrNull),
        ("Listing", "getOrNull", Lookup::EntryOrNull),
        ("Listing", "getOrDefault", Lookup::EntryOrDefault),
        ("Mapping", "getOrNull", Lookup::EntryOrNull),
        ("Mapping", "getOrDefault", Lookup::EntryOrDefault),
    ] {
        let anchor = if class == "Dynamic" {
            "class Dynamic extends".into()
        } else {
            format!("class {class}<")
        };
        let class_start = source.text.find(&anchor).unwrap();
        let class_end = class_start + source.text[class_start..].find("\n}").unwrap();
        let start = class_start
            + source.text[class_start..class_end]
                .find(&format!("external function {name}("))
                .unwrap();
        let end = start + source.text[start..].find('\n').unwrap();
        let mut code = Vec::new();
        let mut spans = Vec::new();
        let ty = match class {
            "Dynamic" => Some(("String", TypeCheck::String)),
            "Listing" => Some(("Int", TypeCheck::Int)),
            _ => None,
        };
        if let Some((text, ty)) = ty {
            let position = start + source.text[start..end].find(&format!(": {text}")).unwrap() + 2;
            code.extend([
                Instruction::LoadLocal(0),
                Instruction::CheckType(ty),
                Instruction::Pop,
            ]);
            spans.extend([None, Some(Span::new(position, position + text.len())), None]);
        }
        code.push(Instruction::ObjectLookup(operation));
        if operation == Lookup::EntryOrDefault {
            code.extend([
                Instruction::LoadLocal(0),
                Instruction::Call {
                    arity: 1,
                    method: false,
                },
            ]);
        }
        code.push(Instruction::Return);
        spans.resize(code.len(), None);
        let block = blocks.len();
        blocks.push(code);
        metadata.push(BlockMetadata {
            source: Some(source.clone()),
            member: format!("#{class}.{name}").into(),
            declaration: Some(Span::new(start, end)),
            spans,
            ..Default::default()
        });
        result.insert((class.into(), name.into()), (block, 1));
    }
    result
}

pub(super) fn default_blocks(
    blocks: &mut Vec<Vec<Instruction>>,
    metadata: &mut Vec<BlockMetadata>,
) -> [usize; 3] {
    ["Dynamic", "Listing", "Mapping"].map(|class| {
        let index = blocks.len();
        blocks.push(vec![
            Instruction::ObjectDefaultProperty,
            Instruction::Return,
        ]);
        metadata.push(BlockMetadata {
            source: Some(crate::stdlib_sources::base()),
            member: format!("#{class}.default").into(),
            spans: vec![None; 2],
            ..Default::default()
        });
        index
    })
}

impl Vm {
    pub(super) fn object_default_type(
        &mut self,
        receiver: ObjectId,
    ) -> Result<Option<(TypeCheck, Span, PklString, ObjectId)>, RuntimeError> {
        let object = self.heap.get(receiver)?;
        if object.is_module
            || object.reference.is_some()
            || object.pair.is_some()
            || collections::typed_class(&self.heap, receiver)?.is_some()
        {
            return Ok(None);
        }
        let (index, class, builtin, text) = match object.collection_kind {
            CollectionKind::Dynamic => (
                0,
                "Dynamic",
                classes::BuiltinClass::Dynamic,
                "(unknown) -> Any",
            ),
            CollectionKind::Listing => (
                1,
                "Listing",
                classes::BuiltinClass::Listing,
                "(Int) -> Element",
            ),
            CollectionKind::Mapping => (
                2,
                "Mapping",
                classes::BuiltinClass::Mapping,
                "(Key) -> Value",
            ),
            _ => return Ok(None),
        };
        let source = crate::stdlib_sources::base();
        let marker = format!("hidden default: {text}");
        let start = source.text.find(&marker).unwrap() + "hidden default: ".len();
        let span = Span::new(start, start + text.len());
        let ty = TypeCheck::Function {
            parameters: Some(vec![TypeCheck::Any]),
            result: Box::new(TypeCheck::Any),
            syntax: false,
        };
        let class_id = self.canonical_builtin_class(builtin);
        let owner = self.heap.get_class(class_id)?.prototype;
        if !self
            .heap
            .get(owner)?
            .members
            .iter()
            .any(|m| m.name.as_ref() == "default" && !m.entry)
        {
            self.heap.get_mut(owner)?.members.push(Member {
                flags: crate::syntax::ModifierKind::Hidden.flag(),
                name: "default".into(),
                code: self.program.object_default_property_blocks[index],
                local: false,
                method: false,
                entry: false,
                ty: Some(ty.clone()),
                type_span: Some(span),
                constant: false,
                fixed: false,
            });
        }
        Ok(Some((ty, span, format!("#{class}.default").into(), owner)))
    }

    fn lookup_result(&mut self, value: Value) {
        self.stack.push(value);
        let frame = self.frames.last_mut().unwrap();
        frame.pc = self.program.blocks[frame.block].len() - 1;
    }
    pub(super) fn object_lookup(&mut self, operation: Lookup) -> Result<(), RuntimeError> {
        let receiver = self.conversion_receiver()?;
        let key = self.conversion_argument(0)?;
        if self.heap.get(receiver)?.collection_kind == CollectionKind::Mapping
            && matches!(operation, Lookup::EntryOrNull | Lookup::EntryOrDefault)
        {
            return self.mapping_lookup(
                receiver,
                key,
                if operation == Lookup::EntryOrNull {
                    mapping_ops::LookupMode::Null
                } else {
                    mapping_ops::LookupMode::Default
                },
            );
        }
        if matches!(
            operation,
            Lookup::HasProperty | Lookup::Property | Lookup::PropertyOrNull
        ) {
            let Value::String(text) = &key else {
                return Err(invalid("checked property name"));
            };
            let name = collection_conversions::property_identifier(text);
            if operation == Lookup::HasProperty {
                // Java checks the String-keyed value cache before Identifier-keyed
                // declarations. A previously forced string entry therefore counts.
                for slot in &self.heap.get(receiver)?.cache_slots {
                    if slot.key.entry
                        && slot.key.owner.is_none()
                        && !slot.key.method
                        && collections::cache_slot_entry_key(&self.heap, receiver, slot)?.as_ref()
                            == Some(&key)
                    {
                        self.lookup_result(Value::Bool(true));
                        return Ok(());
                    }
                }
            }
            let mut owner = Some(receiver);
            let mut found = name.as_ref() == "default";
            while let Some(id) = owner {
                let object = self.heap.get(id)?;
                found |= object
                    .members
                    .iter()
                    .any(|m| !m.local && !m.entry && !m.method && m.name == name);
                owner = object.parent;
            }
            if operation == Lookup::HasProperty {
                self.lookup_result(Value::Bool(found));
            } else if !found && operation == Lookup::PropertyOrNull {
                self.lookup_result(Value::Null);
            } else if !found {
                return Err(self.missing_lookup_property(receiver, text)?);
            } else {
                self.force(receiver, receiver, name, false, true, false)?;
            }
            return Ok(());
        }
        if self.heap.get(receiver)?.members.len() > 4 && self.prepare_keys(vec![key.clone()])? {
            return Ok(());
        }
        let candidates = self.declaration_candidates(receiver, &key, true)?;
        if self.search_key(&key, candidates.clone())? {
            return Ok(());
        }
        let listing = self.heap.get(receiver)?.collection_kind == CollectionKind::Listing;
        let in_range = !listing
            || matches!(&key, Value::Int(index) if index.get() >= 0 && (index.get() as usize) < self.heap.get(receiver)?.length);
        let member = if in_range {
            collections::ordered_members(&self.heap, receiver)?
                .into_iter()
                .rev()
                .find(|member| {
                    member.kind != CollectionMemberKind::Property
                        && candidates.iter().any(|candidate| {
                            equality::identical_key(&self.heap, candidate, &member.key)
                        })
                        && equality::same_key(&self.heap, &key, &member.key)
                })
        } else {
            None
        };
        if let Some(member) = member {
            let frame = self.frames.last_mut().unwrap();
            frame.pc = self.program.blocks[frame.block].len() - 1;
            self.force_key_probe(receiver, receiver, member.name, true, key)?;
        } else if operation == Lookup::EntryOrNull {
            self.lookup_result(Value::Null);
        } else {
            self.force(receiver, receiver, "default".into(), false, true, false)?;
        }
        Ok(())
    }

    fn missing_lookup_property(
        &self,
        receiver: ObjectId,
        name: &PklString,
    ) -> Result<RuntimeError, RuntimeError> {
        let identifier = collection_conversions::property_identifier(name);
        let mut error = self.missing_member(receiver, &identifier, false)?;
        error.message = PklString::join(
            &[
                "Cannot find property `".into(),
                name.clone(),
                "` in object of type `Dynamic`.".into(),
            ],
            "",
        );
        if error
            .hint
            .as_ref()
            .is_some_and(|hint| hint.starts_with("Available properties:\n"))
        {
            let mut names = BTreeSet::from([PklString::from("default")]);
            let mut owner = Some(receiver);
            while let Some(id) = owner {
                let object = self.heap.get(id)?;
                for member in object
                    .members
                    .iter()
                    .filter(|m| !m.local && !m.entry && !m.method)
                {
                    let text = object
                        .collection_members
                        .iter()
                        .find(|m| m.kind == CollectionMemberKind::Property && m.name == member.name)
                        .and_then(|m| {
                            if let Value::String(text) = &m.key {
                                Some(text.clone())
                            } else {
                                None
                            }
                        })
                        .unwrap_or_else(|| PklString::from(member.name.clone()));
                    names.insert(text);
                }
                owner = object.parent;
            }
            error.hint = Some(
                PklString::from("Available properties:\n").concat(&PklString::join(
                    &names.into_iter().collect::<Vec<_>>(),
                    "\n",
                )),
            );
        }
        Ok(error)
    }

    /// Builtin object defaults are closures, just like the source declarations.
    /// Authored overrides have already won ordinary member lookup before this path.
    pub(super) fn object_default_member(
        &mut self,
        receiver: ObjectId,
    ) -> Result<bool, RuntimeError> {
        let object = self.heap.get(receiver)?;
        if object.is_module
            || object.reference.is_some()
            || object.pair.is_some()
            || collections::typed_class(&self.heap, receiver)?.is_some()
            || !matches!(
                object.collection_kind,
                CollectionKind::Dynamic | CollectionKind::Listing | CollectionKind::Mapping
            )
        {
            return Ok(false);
        }
        // ListingOrMappingTypeNode creates an undefined *property* when
        // the element type has no default, not a function that throws later.
        // Reading default must therefore fail before the implicit call node.
        let mut owner = Some(receiver);
        while let Some(id) = owner {
            let object = self.heap.get(id)?;
            if let Some(value) = &object.collection_default {
                if value.is_none() {
                    let mut error = RuntimeError::new(
                        ErrorKind::MissingMember,
                        "Tried to read property `default` but its value is undefined.",
                    )
                    .undefined();
                    error
                        .frames
                        .extend(object.collection_default_origin.clone());
                    return Err(error);
                }
                break;
            }
            owner = object.parent;
        }
        let id = self.heap.allocate(Object {
            function: Some(Function {
                block: self.program.object_default_block,
                arity: 1,
                method: false,
                captured_this: Some(Value::Object(receiver)),
            }),
            ..Default::default()
        });
        let value = Value::Function(id);
        self.stack.push(value.clone());
        self.complete_cache(
            receiver,
            CacheKey {
                entry: false,
                owner: None,
                name: "default".into(),
                method: false,
            },
            value,
        )?;
        Ok(true)
    }
    pub(super) fn object_default(&mut self) -> Result<(), RuntimeError> {
        let receiver = self.conversion_receiver()?;
        let mut owner = Some(receiver);
        while let Some(id) = owner {
            let object = self.heap.get(id)?;
            if let Some(value) = &object.collection_default {
                if let Some(value) = value {
                    self.stack.push(value.clone());
                    return Ok(());
                }
                let mut error = RuntimeError::new(
                    ErrorKind::MissingMember,
                    "Tried to read property `default` but its value is undefined.",
                )
                .undefined();
                error
                    .frames
                    .extend(object.collection_default_origin.clone());
                return Err(error);
            }
            owner = object.parent;
        }
        let result = self.empty_object(CollectionKind::Dynamic);
        self.stack.push(Value::Object(result));
        Ok(())
    }
}
