//! Lazy Listing/Mapping visitors with resumable Java HashSet/equality work.
use super::*;
use crate::heap::{CollectionKind, CollectionMember, CollectionMemberKind};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operation {
    Any,
    Contains,
    Distinct,
    DistinctBy,
    IsDistinctBy,
    Fold,
}
impl Operation {
    fn callback(self) -> bool {
        matches!(
            self,
            Self::Any | Self::DistinctBy | Self::IsDistinctBy | Self::Fold
        )
    }
    fn shallow(self) -> bool {
        matches!(
            self,
            Self::Distinct | Self::DistinctBy | Self::IsDistinctBy | Self::Fold
        )
    }
    fn builder(self) -> bool {
        matches!(self, Self::Distinct | Self::DistinctBy)
    }
}

#[cfg_attr(target_arch = "wasm32", inline(never))]
pub(super) fn blocks(
    blocks: &mut Vec<Vec<Instruction>>,
    metadata: &mut Vec<BlockMetadata>,
) -> BTreeMap<(Arc<str>, Arc<str>), usize> {
    let source = crate::stdlib_sources::base();
    let mut result = BTreeMap::new();
    for (class, name, declaration, function_type, operation) in [
        (
            "Mapping",
            "fold",
            "external function fold<Result>(initial: Result, operator: (Result, Key, Value) -> Result): Result",
            "(Result, Key, Value) -> Result",
            Operation::Fold,
        ),
        (
            "Listing",
            "any",
            "external function any(predicate: (Element) -> Boolean): Boolean",
            "(Element) -> Boolean",
            Operation::Any,
        ),
        (
            "Mapping",
            "any",
            "external function any(predicate: (Key, Value) -> Boolean): Boolean",
            "(Key, Value) -> Boolean",
            Operation::Any,
        ),
        (
            "Listing",
            "contains",
            "external function contains(element: Element): Boolean",
            "",
            Operation::Contains,
        ),
        (
            "Mapping",
            "containsValue",
            "external function containsValue(value: Any): Boolean",
            "",
            Operation::Contains,
        ),
        (
            "Listing",
            "distinct",
            "external distinct: Listing<Element>",
            "",
            Operation::Distinct,
        ),
        (
            "Listing",
            "distinctBy",
            "external function distinctBy(selector: (Element) -> Any): Listing<Element>",
            "(Element) -> Any",
            Operation::DistinctBy,
        ),
        (
            "Listing",
            "isDistinctBy",
            "external function isDistinctBy(selector: (Element) -> Any): Boolean",
            "(Element) -> Any",
            Operation::IsDistinctBy,
        ),
    ] {
        let class_start = source
            .text
            .find(&format!("class {class}<"))
            .expect("pinned lazy class");
        let start = class_start
            + source
                .text
                .entire()
                .range(class_start..)
                .find(declaration)
                .expect("pinned lazy member");
        let arity = if operation == Operation::Fold {
            3
        } else if class == "Mapping" {
            2
        } else {
            1
        };
        let mut code = Vec::new();
        let mut spans = Vec::new();
        if operation == Operation::Fold {
            code.push(Instruction::LoadLocal(1));
            spans.push(None);
        }
        if operation != Operation::Distinct {
            code.push(Instruction::LoadLocal(0));
            spans.push(None);
        }
        if operation.callback() {
            let parameter = start + declaration.find(function_type).unwrap();
            code.push(Instruction::CheckType(Box::new(TypeCheck::Function {
                parameters: Some(vec![TypeCheck::Any; arity]),
                result: Box::new(TypeCheck::Any),
                syntax: true,
            })));
            spans.push(Some(Span::new(parameter, parameter + function_type.len())));
        }
        code.push(Instruction::BeginLazyCollection(operation));
        let next = code.len();
        code.push(Instruction::LazyCollectionNext { end: 0 });
        if operation.callback() {
            code.push(Instruction::Call {
                arity,
                method: false,
            });
            code.push(Instruction::LazyCollectionAccept { next, end: 0 });
        }
        let end = code.len();
        code[next] = Instruction::LazyCollectionNext { end };
        if operation.callback() {
            code[end - 1] = Instruction::LazyCollectionAccept { next, end };
        }
        code.extend([Instruction::EndLazyCollection, Instruction::Return]);
        spans.resize(code.len(), None);
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

#[derive(Default)]
struct Seen {
    values: Vec<Value>,
    index: collection_transforms::InsertionHashIndex,
}
impl Seen {
    fn insert(&mut self, value: Value, hash: Option<i32>) {
        self.index.insert(hash, self.values.len());
        self.values.push(value);
    }
}

pub(super) struct Run {
    operation: Operation,
    source: ObjectId,
    mapping: bool,
    members: Vec<collections::RenderMember>,
    next: usize,
    properties: Seen,
    entries: Seen,
    member_hash: Option<Option<i32>>,
    awaiting: Option<CollectionMember>,
    current: Option<(Value, Value)>,
    argument: Option<Value>,
    accumulator: Option<Value>,
    selected: Option<Value>,
    selected_hash: Option<Option<i32>>,
    projected: Seen,
    shallow: bool,
    result: bool,
}
impl Run {
    pub(super) fn roots(&self, roots: &mut Vec<Value>) {
        roots.push(Value::Object(self.source));
        roots.extend(self.members.iter().map(|item| item.descriptor.key.clone()));
        roots.extend(
            self.properties
                .values
                .iter()
                .chain(&self.entries.values)
                .cloned(),
        );
        roots.extend(self.projected.values.iter().cloned());
        roots.extend(self.argument.iter().chain(&self.selected).cloned());
        roots.extend(self.accumulator.iter().cloned());
        if let Some((key, value)) = &self.current {
            roots.extend([key.clone(), value.clone()]);
        }
    }
}

impl Vm {
    fn lazy_run(&self) -> &Run {
        self.frames
            .last()
            .unwrap()
            .lazy_collection_run
            .as_ref()
            .unwrap()
    }
    fn lazy_run_mut(&mut self) -> &mut Run {
        self.frames
            .last_mut()
            .unwrap()
            .lazy_collection_run
            .as_mut()
            .unwrap()
    }
    #[cfg_attr(target_arch = "wasm32", inline(never))]
    pub(super) fn begin_lazy_collection(
        &mut self,
        operation: Operation,
    ) -> Result<(), RuntimeError> {
        let argument = if operation == Operation::Distinct {
            None
        } else {
            Some(self.pop()?)
        };
        let accumulator = if operation == Operation::Fold {
            Some(self.pop()?)
        } else {
            None
        };
        let source = self.conversion_receiver()?;
        let mapping = self.heap.get(source)?.collection_kind == CollectionKind::Mapping;
        let run = Run {
            operation,
            source,
            mapping,
            members: collections::render_members(&self.heap, source)?,
            next: 0,
            properties: Seen::default(),
            entries: Seen::default(),
            member_hash: None,
            awaiting: None,
            current: None,
            argument,
            accumulator,
            selected: None,
            selected_hash: None,
            projected: Seen::default(),
            shallow: operation.shallow(),
            result: operation == Operation::IsDistinctBy,
        };
        self.frames.last_mut().unwrap().lazy_collection_run = Some(Box::new(run));
        if operation.builder() {
            // VmObjectBuilder.toListing uses the base prototype, not the source's
            // default, parent, or element type wrapper. Preserve value identities.
            let class = self.canonical_builtin_class(classes::BuiltinClass::Listing);
            let parent = self.heap.get_class(class)?.prototype;
            let builder = self.heap.allocate(Object {
                parent: Some(parent),
                collection_kind: CollectionKind::Listing,
                ..Default::default()
            });
            self.frames.last_mut().unwrap().builders.push(builder);
        }
        if operation.shallow() {
            self.force_shallow_frame(Value::Object(source))?;
        }
        Ok(())
    }

    #[cfg_attr(target_arch = "wasm32", inline(never))]
    pub(super) fn lazy_collection_next(&mut self, end: usize) -> Result<(), RuntimeError> {
        if self.resume_visitor_lookup()? {
            return Ok(());
        }
        if self.lazy_run().shallow {
            self.pop()?;
            self.lazy_run_mut().shallow = false;
        }
        if let Some(member) = self.lazy_run_mut().awaiting.take() {
            let value = self.pop()?;
            self.lazy_run_mut().current = Some((member.key, value));
        }
        // Keep the current input while equality/hash continuations rewind this
        // instruction. Never read the next member before the current result.
        if self.lazy_run().current.is_some() {
            return self.lazy_collection_input(end);
        }
        let Some(item) = self.lazy_run().members.get(self.lazy_run().next).cloned() else {
            return self.jump(end);
        };
        let source = self.lazy_run().source;
        let member = item.descriptor;
        let property = member.kind == CollectionMemberKind::Property;
        let hash = if let Some(hash) = self.lazy_run().member_hash {
            hash
        } else {
            if self.prepare_keys(vec![member.key.clone()])? {
                return Ok(());
            }
            let hash = self.java_hash(&member.key)?;
            self.lazy_run_mut().member_hash = Some(hash);
            hash
        };
        let run = self.lazy_run();
        let seen = if property {
            &run.properties
        } else {
            &run.entries
        };
        let candidates = seen.index.candidates(hash, &seen.values);
        if self.search_key(&member.key, candidates.clone())? {
            return Ok(());
        }
        let duplicate = candidates
            .iter()
            .any(|old| equality::same_key(&self.heap, &member.key, old));
        let run = self.lazy_run_mut();
        if !duplicate {
            let seen = if property {
                &mut run.properties
            } else {
                &mut run.entries
            };
            seen.insert(member.key.clone(), hash);
        }
        run.next += 1;
        run.member_hash = None;
        // Java's visited.add precedes its hidden/external-member filter.
        if duplicate || item.skip {
            self.frames.last_mut().unwrap().pc -= 1;
            return Ok(());
        }
        if !property {
            let mode = if self.lazy_run().operation.shallow() {
                mapping_ops::LookupMode::ForcedVisitor
            } else {
                mapping_ops::LookupMode::LazyVisitor
            };
            self.lazy_run_mut().awaiting = Some(member.clone());
            self.frames.last_mut().unwrap().prepared_hashes = None;
            return self.mapping_lookup(source, member.key, mode);
        }
        let cache = CacheKey {
            owner: None,
            name: member.name.clone(),
            method: false,
            entry: false,
        };
        if let Some(value) = self.cached_slot(source, &cache)? {
            self.lazy_run_mut().current = Some((member.key, value));
            return self.lazy_collection_input(end);
        }
        if self.lazy_run().operation.shallow() {
            return Err(RuntimeError::new(
                ErrorKind::Unsupported,
                "The pinned forced-iteration missing-cache internal-error protocol is not implemented.",
            ));
        }
        self.lazy_run_mut().awaiting = Some(member.clone());
        self.frames.last_mut().unwrap().pc -= 1;
        self.force_key_mode(
            source,
            source,
            member.name,
            false,
            true,
            false,
            member.kind != CollectionMemberKind::Property,
            false,
        )
    }

    fn lazy_collection_input(&mut self, end: usize) -> Result<(), RuntimeError> {
        let run = self.lazy_run();
        let (key, value) = run.current.clone().expect("current lazy input");
        if run.operation.callback() {
            let callback = run.argument.clone().expect("lazy callback");
            let mapping = run.mapping;
            let accumulator = run.accumulator.clone();
            self.stack.push(callback);
            if let Some(accumulator) = accumulator {
                self.stack.push(accumulator);
            }
            if mapping {
                self.stack.push(key);
            }
            self.stack.push(value);
            return Ok(());
        }
        if run.operation == Operation::Contains {
            let query = run.argument.clone().expect("contains query");
            // VmListing/Mapping contains uses query.equals(stored), without a
            // preceding hash of either value. Identity/empty cases stay lazy.
            if self.search_key(&query, vec![value.clone()])? {
                return Ok(());
            }
            if equality::same_key(&self.heap, &query, &value) {
                self.lazy_run_mut().result = true;
                return self.jump(end);
            }
            self.lazy_run_mut().current = None;
            self.frames.last_mut().unwrap().pc -= 1;
            return Ok(());
        }
        if run.selected.is_none() {
            self.lazy_run_mut().selected = Some(value);
        }
        let next = self.frames.last().unwrap().pc - 1;
        self.lazy_collection_select(next, end)
    }

    #[cfg_attr(target_arch = "wasm32", inline(never))]
    pub(super) fn lazy_collection_accept(
        &mut self,
        next: usize,
        end: usize,
    ) -> Result<(), RuntimeError> {
        if self.lazy_run().operation == Operation::Fold {
            let value = self.pop()?;
            let run = self.lazy_run_mut();
            run.accumulator = Some(value);
            run.current = None;
            return self.jump(next);
        }
        if self.lazy_run().operation == Operation::Any {
            let value = self.pop()?;
            let Value::Bool(accepted) = value else {
                return Err(type_error(
                    &TypeCheck::Boolean,
                    &value,
                    &self.program.module_name,
                    &self.heap,
                )?);
            };
            self.lazy_run_mut().current = None;
            if accepted {
                self.lazy_run_mut().result = true;
                return self.jump(end);
            }
            return self.jump(next);
        }
        if self.lazy_run().selected.is_none() {
            let value = self.pop()?;
            self.lazy_run_mut().selected = Some(value);
        }
        self.lazy_collection_select(next, end)
    }

    fn lazy_collection_select(&mut self, next: usize, end: usize) -> Result<(), RuntimeError> {
        let key = self.lazy_run().selected.clone().expect("projection result");
        let hash = if let Some(hash) = self.lazy_run().selected_hash {
            hash
        } else {
            if self.prepare_keys(vec![key.clone()])? {
                return Ok(());
            }
            let hash = self.java_hash(&key)?;
            self.lazy_run_mut().selected_hash = Some(hash);
            hash
        };
        let run = self.lazy_run();
        let candidates = run.projected.index.candidates(hash, &run.projected.values);
        if self.search_key(&key, candidates.clone())? {
            return Ok(());
        }
        let duplicate = candidates
            .iter()
            .any(|old| equality::same_key(&self.heap, &key, old));
        let run = self.lazy_run_mut();
        if duplicate && run.operation == Operation::IsDistinctBy {
            run.result = false;
            return self.jump(end);
        }
        let append = !duplicate && run.operation.builder();
        if !duplicate {
            run.projected.insert(key, hash);
        }
        let (_, original) = run.current.take().expect("projected input");
        run.selected = None;
        run.selected_hash = None;
        if append {
            self.stack.push(original);
            self.append_strict_element()?;
        }
        self.jump(next)
    }

    #[cfg_attr(target_arch = "wasm32", inline(never))]
    pub(super) fn end_lazy_collection(&mut self) -> Result<(), RuntimeError> {
        let frame = self.frames.last_mut().unwrap();
        let run = frame
            .lazy_collection_run
            .take()
            .ok_or_else(|| invalid("lazy collection result"))?;
        let value = if run.operation == Operation::Fold {
            run.accumulator
                .ok_or_else(|| invalid("Mapping.fold accumulator"))?
        } else if run.operation.builder() {
            Value::Object(
                frame
                    .builders
                    .pop()
                    .ok_or_else(|| invalid("Listing builder"))?,
            )
        } else {
            Value::Bool(run.result)
        };
        self.stack.push(value);
        Ok(())
    }
}
