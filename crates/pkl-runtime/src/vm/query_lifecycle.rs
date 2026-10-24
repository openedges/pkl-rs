//! Dynamic module/query code lives as long as its active or captured environments.
//! Cached declarations are edges of a traced owner, never permanent VM roots.
use super::*;
use std::ops::Range;

pub(super) struct CodeArena {
    owner: ExecutableOwnerId,
    pub(super) query: bool,
    blocks: Range<usize>,
    objects: Range<usize>,
    units: Range<usize>,
    recipes: Range<usize>,
    aliases: BTreeSet<usize>,
}
pub(super) struct Start {
    blocks: usize,
    objects: usize,
    units: usize,
    recipes: usize,
    aliases: BTreeSet<usize>,
}
impl CodeArena {
    pub(super) fn begin(program: &mut Program) -> Start {
        program.seal_code_segment();
        Start {
            blocks: program.blocks.next_id(),
            objects: program.objects.next_id(),
            units: program.units.next_id(),
            recipes: program.type_recipes.next_id(),
            aliases: program.alias_definitions.keys().copied().collect(),
        }
    }
    fn retire(self, program: &mut Program) {
        debug_assert!(
            !self.query
                || self.units.clone().all(|unit| program.units[unit]
                    .source
                    .as_ref()
                    .is_some_and(|source| source.uri.as_ref() == "repl:text"))
        );
        if self.units.contains(&0) {
            program.initial_module_members = Vec::new();
            program.module_name = "".into();
            program.module_template = None;
            program.entry = usize::MAX;
        }
        program.blocks.retire(self.blocks.clone());
        program.metadata.retire(self.blocks.clone());
        program.objects.retire(self.objects);
        program.type_recipes.retire(self.recipes);
        program.units.retire(self.units);
        program
            .alias_definitions
            .retain(|id, _| !self.aliases.contains(id));
        program
            .constraint_origins
            .retain(|block, _| !self.blocks.contains(block));
    }
}
impl Start {
    pub(super) fn finish(
        self,
        program: &mut Program,
        owner: ExecutableOwnerId,
        query: bool,
    ) -> CodeArena {
        program.seal_code_segment();
        CodeArena {
            owner,
            query,
            blocks: self.blocks..program.blocks.next_id(),
            objects: self.objects..program.objects.next_id(),
            units: self.units..program.units.next_id(),
            recipes: self.recipes..program.type_recipes.next_id(),
            aliases: program
                .alias_definitions
                .keys()
                .filter(|id| !self.aliases.contains(id))
                .copied()
                .collect(),
        }
    }
}
impl Vm {
    pub(super) fn declaration_code_roots(&mut self, roots: &mut Vec<Value>) {
        let mut owned = vec![Vec::new(); self.code_arenas.len()];
        let mut retain = |index: Option<usize>, value: Value| {
            if let Some(index) = index {
                owned[index].push(value);
            } else {
                roots.push(value);
            }
        };
        for ((recipe, owner), value) in &self.resolved_declarations {
            let index = self
                .code_arenas
                .iter()
                .position(|arena| arena.recipes.contains(recipe));
            retain(index, Value::Object(*owner));
            if let type_resolution::DeclarationValue::Value(value) = value {
                retain(index, value.clone());
            }
        }
        for (block, owner) in &self.executable_roots {
            retain(
                self.code_arenas
                    .iter()
                    .position(|arena| arena.blocks.contains(block)),
                owner.root(),
            );
        }
        for (template, id) in &self.class_singletons {
            retain(
                self.code_arenas
                    .iter()
                    .position(|arena| arena.objects.contains(template)),
                Value::Class(*id),
            );
        }
        for (declaration, id) in &self.alias_singletons {
            retain(
                self.code_arenas
                    .iter()
                    .position(|arena| arena.aliases.contains(declaration)),
                Value::TypeAlias(*id),
            );
        }
        let mut by_owner: Vec<(ExecutableOwnerId, Vec<Value>)> = Vec::new();
        for (arena, roots) in self.code_arenas.iter().zip(owned) {
            if let Some((_, values)) = by_owner.iter_mut().find(|(owner, _)| *owner == arena.owner)
            {
                values.extend(roots);
            } else {
                by_owner.push((arena.owner, roots));
            }
        }
        for (owner, roots) in by_owner {
            self.heap.set_code_roots(owner, roots);
        }
    }

    pub(super) fn retire_unreachable_code(&mut self) {
        for index in (0..self.code_arenas.len()).rev() {
            if self.heap.executable_is_live(self.code_arenas[index].owner) {
                continue;
            }
            let arena = self.code_arenas.remove(index);
            self.resolved_declarations
                .retain(|(recipe, _), _| !arena.recipes.contains(recipe));
            self.executable_roots
                .retain(|block, _| !arena.blocks.contains(block));
            self.class_singletons
                .retain(|template, _| !arena.objects.contains(template));
            self.alias_singletons
                .retain(|declaration, _| !arena.aliases.contains(declaration));
            arena.retire(Arc::make_mut(&mut self.program));
        }
        let active: BTreeSet<_> = self
            .frames
            .iter()
            .chain(self.suspended.iter().flat_map(|context| &context.frames))
            .map(|frame| frame.block)
            .collect();
        let finished: Vec<_> = self
            .public_operation_entries
            .iter()
            .filter(|entry| !active.contains(entry))
            .copied()
            .collect();
        for entry in finished.into_iter().rev() {
            self.public_operation_entries.remove(&entry);
            let program = Arc::make_mut(&mut self.program);
            program.blocks.retire(entry..entry + 1);
            program.metadata.retire(entry..entry + 1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::{
        ModuleCompletion, ModuleHostError, ModuleKey, ModuleKeyKind, ModuleRequest,
        ModuleRequestKind, ModuleSource, ModuleSourceSpec, ResolutionPhase, ResolvedModule,
    };

    fn complete(request: &ModuleRequest) -> ModuleCompletion {
        match &request.kind {
            ModuleRequestKind::SelectKey { id, uri, .. } if uri.starts_with("memo:") => {
                ModuleCompletion::KeySelected(Ok(ModuleKey {
                    id: *id,
                    uri: uri.clone(),
                    kind: ModuleKeyKind::Host,
                    handle: "test".into(),
                }))
            }
            ModuleRequestKind::QueryCachePolicy { .. } => ModuleCompletion::CachePolicy(Ok(false)),
            ModuleRequestKind::Resolve { key, .. } => {
                ModuleCompletion::Resolved(Ok(ResolvedModule {
                    original_uri: key.uri.clone(),
                    resolved_uri: key.uri.clone(),
                    source_uri: key.uri.clone(),
                }))
            }
            ModuleRequestKind::LoadSource { key, .. } => {
                ModuleCompletion::Source(Ok(ModuleSource {
                    text: if key.key.uri.as_ref() == "memo:/broken.pkl" {
                        "p: Missing=1\ntag=0"
                    } else if key.key.uri.as_ref() == "memo:/rich.pkl" {
                        "import \"memo:/dependency.pkl\" as dependency\n@Deprecated { message=\"stage50\" }\nclass C { value: Int(this > 0) = dependency.tag }\ntypealias A<T> = T\ntypealias Checked = Int(this > 7)\nx: A<dependency.D> = new dependency.D {}\ny:C=new C{}\ntag=x.value+y.value-9"
                    } else if key.key.uri.as_ref() == "memo:/dependency.pkl" {
                        "class D { value: Int=9 }\ntag=9"
                    } else {
                        "class C { value=9 }\ntypealias A=C\nx: A=new C {}\ntag=x.value"
                    }
                    .into(),
                    inferred_name: "ignored".into(),
                }))
            }
            _ => crate::modules::default_completion(request).unwrap_or(
                ModuleCompletion::KeySelected(Err(ModuleHostError::NoLoader)),
            ),
        }
    }
    fn settle_result(
        vm: &mut Vm,
        external: &[Value],
        raw_result: bool,
    ) -> Result<Value, RuntimeError> {
        for _ in 0..100_000 {
            match vm.poll(1) {
                VmPoll::Yielded => {}
                VmPoll::Module(request) => {
                    let query = matches!(
                        request.kind,
                        ModuleRequestKind::ResolveKey {
                            phase: ResolutionPhase::Query,
                            ..
                        }
                    );
                    vm.complete_module(request.id, complete(&request)).unwrap();
                    if query && raw_result {
                        // Exercise the underlying VM's escaped-value roots;
                        // the public expression-string API intentionally stringifies.
                        let block = vm.frames.last().unwrap().block;
                        let program = Arc::make_mut(&mut vm.program);
                        let at = program.blocks[block]
                            .iter()
                            .rposition(|instruction| matches!(instruction, Instruction::ToString))
                            .unwrap();
                        program.blocks[block].remove(at);
                        program.metadata[block].spans.remove(at);
                    }
                }
                VmPoll::Ready(value) => {
                    vm.collect(external);
                    return value;
                }
                event => panic!("unexpected query event {event:?}"),
            }
            vm.collect(external);
        }
        panic!("query poll bound")
    }
    fn settle(vm: &mut Vm, external: &[Value], raw_result: bool) -> Value {
        settle_result(vm, external, raw_result).unwrap()
    }
    fn start(vm: &mut Vm, expression: &str) {
        let entry = vm
            .append_public_operation(
                ModuleSourceSpec {
                    uri: "memo:/main.pkl".into(),
                    contents: Some("stable=7".into()),
                },
                expression.into(),
            )
            .unwrap();
        vm.start(entry, Vec::new()).unwrap();
    }
    fn counts(vm: &Vm) -> (usize, usize, usize, usize) {
        (
            vm.program.blocks.len(),
            vm.program.objects.len(),
            vm.program.units.len(),
            vm.program.type_recipes.len(),
        )
    }
    fn query_count(vm: &Vm) -> usize {
        vm.code_arenas.iter().filter(|arena| arena.query).count()
    }
    #[derive(Debug, PartialEq, Eq)]
    struct ResidentStats {
        outer: [(usize, usize, usize); 5],
        nested_capacity: [usize; 5],
        source_bytes: usize,
        heap_live: usize,
        executable_live: usize,
        aliases: usize,
        owners: usize,
        declaration_caches: [usize; 5],
    }
    fn resident_stats(vm: &Vm) -> ResidentStats {
        let p = &vm.program;
        let mut sources = BTreeMap::new();
        for source in p
            .units
            .iter()
            .filter_map(|unit| unit.source.as_ref())
            .chain(
                p.metadata
                    .iter()
                    .filter_map(|metadata| metadata.source.as_ref()),
            )
        {
            sources.insert(Arc::as_ptr(source), source.text.len());
        }
        let heap = vm.heap.stats();
        ResidentStats {
            outer: [
                (
                    p.blocks.len(),
                    p.blocks.capacity(),
                    p.blocks.resident_segments(),
                ),
                (
                    p.metadata.len(),
                    p.metadata.capacity(),
                    p.metadata.resident_segments(),
                ),
                (
                    p.objects.len(),
                    p.objects.capacity(),
                    p.objects.resident_segments(),
                ),
                (
                    p.units.len(),
                    p.units.capacity(),
                    p.units.resident_segments(),
                ),
                (
                    p.type_recipes.len(),
                    p.type_recipes.capacity(),
                    p.type_recipes.resident_segments(),
                ),
            ],
            nested_capacity: [
                p.blocks.iter().map(Vec::capacity).sum(),
                p.metadata
                    .iter()
                    .map(|m| m.spans.capacity() + m.let_scopes.capacity() + m.type_casts.capacity())
                    .sum(),
                p.objects.iter().map(Vec::capacity).sum(),
                p.units
                    .iter()
                    .map(|u| u.initial_members.capacity() + u.import_order.capacity())
                    .sum(),
                p.type_recipes
                    .iter()
                    .map(|r| r.path.capacity() + r.names.capacity() + r.argument_spans.capacity())
                    .sum(),
            ],
            source_bytes: sources.values().sum(),
            heap_live: heap.live,
            executable_live: heap.executable_live,
            aliases: p.alias_definitions.len(),
            owners: vm.code_arenas.len(),
            declaration_caches: [
                p.constraint_origins.len(),
                vm.class_singletons.len(),
                vm.alias_singletons.len(),
                vm.resolved_declarations.len(),
                vm.executable_roots.len(),
            ],
        }
    }

    fn import_preparation_keeps_unrelated_snapshot_segments_shared(triple: bool) {
        use crate::modules::{ModuleCapabilities, SelectionPhase};
        let mut vm = Vm::new(Arc::new(Program::new(Vec::new())));
        start(&mut vm, "import(\"memo:/rich.pkl\").tag");
        assert_eq!(settle(&mut vm, &[], false), Value::string("9"));
        let snapshot = vm.program.clone();
        assert!(snapshot.blocks.resident_segments() > 1);
        let holder = "memo:/holder/a/holder.pkl";
        let source = if triple {
            "import \".../clause.pkl\" as clause\ntag=clause.tag+import(\".../expression.pkl\").tag"
        } else {
            "import \"./clause.pkl\" as clause\ntag=clause.tag+import(\"./expression.pkl\").tag"
        };
        let expected = |name: &str| {
            if triple {
                format!("memo:/{name}.pkl")
            } else {
                format!("memo:/mapped/{name}.pkl")
            }
        };
        start(&mut vm, &format!("import(\"{holder}\").tag"));
        let mut clause_selections = 0;
        let mut elements = 0;
        let mut expression_updates = 0;
        let mut finished = false;
        for _ in 0..100_000 {
            match vm.poll(1) {
                VmPoll::Yielded => {}
                VmPoll::Module(request) => {
                    let mut expression = false;
                    let completion = match &request.kind {
                        ModuleRequestKind::LoadSource { key, .. }
                            if key.key.uri.as_ref() == holder =>
                        {
                            ModuleCompletion::Source(Ok(ModuleSource {
                                text: source.into(),
                                inferred_name: "ignored".into(),
                            }))
                        }
                        ModuleRequestKind::ResolveImportUri { key, authored_uri }
                            if key.uri.as_ref() == holder =>
                        {
                            assert!(!triple);
                            expression = authored_uri.ends_with("expression.pkl");
                            ModuleCompletion::ImportUri(Ok(expected(if expression {
                                "expression"
                            } else {
                                "clause"
                            })
                            .into()))
                        }
                        ModuleRequestKind::Capabilities { .. } => {
                            assert!(triple);
                            ModuleCompletion::Capabilities(Ok(ModuleCapabilities {
                                local: true,
                                hierarchical: true,
                                fragment_paths: false,
                            }))
                        }
                        ModuleRequestKind::HasElement { element_uri, .. } => {
                            assert!(triple);
                            elements += 1;
                            let exists = element_uri.as_ref() == expected("clause")
                                || element_uri.as_ref() == expected("expression");
                            expression = exists && element_uri.ends_with("expression.pkl");
                            ModuleCompletion::HasElement(Ok(exists))
                        }
                        ModuleRequestKind::SelectKey {
                            phase: SelectionPhase::ClausePreparation,
                            uri,
                            ..
                        } => {
                            clause_selections += 1;
                            assert_eq!(uri.as_ref(), expected("clause"));
                            complete(&request)
                        }
                        _ => complete(&request),
                    };
                    let old_expression = expression.then(|| {
                        let old = vm.program.clone();
                        let block = old
                            .blocks
                            .iter_with_ids()
                            .find_map(|(id, code)| {
                                code.iter().any(|instruction| {
                                    matches!(instruction, Instruction::ImportModule(spec)
                                        if spec.authored.ends_with("expression.pkl")
                                            && old.units[spec.unit.0 as usize].identity.as_ref()
                                                .is_some_and(|identity| identity.original_uri.as_ref() == holder))
                                }).then_some(id)
                            })
                            .expect("unprepared expression instruction");
                        // Append and complete a different owner's query after
                        // the target body. Preparation must still find and
                        // mutate the older body, not just a recent suffix.
                        let child = vm
                            .append_public_operation(
                                ModuleSourceSpec {
                                    uri: "memo:/main.pkl".into(),
                                    contents: Some("stable=7".into()),
                                },
                                "stable".into(),
                            )
                            .unwrap();
                        vm.reenter(request.id, child, Vec::new()).unwrap();
                        assert_eq!(settle(&mut vm, &[], false), Value::string("7"));
                        vm.finish_reentry();
                        (old, block)
                    });
                    vm.complete_module(request.id, completion).unwrap();
                    if let Some((old, block)) = old_expression {
                        expression_updates += 1;
                        assert!(!std::ptr::eq(&old.blocks[block], &vm.program.blocks[block]));
                        assert!(old.blocks[block].iter().any(|instruction| {
                            matches!(instruction, Instruction::ImportModule(spec)
                                if spec.authored.ends_with("expression.pkl")
                                    && spec.original_uri.as_ref() != expected("expression"))
                        }));
                    }
                }
                VmPoll::Ready(result) => {
                    assert_eq!(result.unwrap(), Value::string("18"));
                    finished = true;
                }
                event => panic!("unexpected preparation event {event:?}"),
            }
            vm.collect(&[]);
            for (id, old) in snapshot.blocks.iter_with_ids() {
                assert!(
                    std::ptr::eq(old, vm.program.blocks.get(id).expect("unrelated code")),
                    "URI preparation cloned unrelated block {id}"
                );
            }
            if finished {
                break;
            }
        }
        assert!(finished, "preparation poll bound");
        assert_eq!(clause_selections, 1);
        assert_eq!(elements, if triple { 4 } else { 0 });
        assert_eq!(expression_updates, 1);
        let unit = vm
            .program
            .units
            .iter()
            .find(|unit| {
                unit.identity
                    .as_ref()
                    .is_some_and(|identity| identity.original_uri.as_ref() == holder)
            })
            .unwrap();
        assert!(unit.build.is_none(), "clause body must have been built");
        for name in ["clause", "expression"] {
            let specs: Vec<_> = vm
                .program
                .blocks
                .iter()
                .flatten()
                .filter_map(|instruction| match instruction {
                    Instruction::ImportModule(spec)
                        if spec.unit == unit.id
                            && spec.authored.ends_with(&format!("{name}.pkl")) =>
                    {
                        Some(spec)
                    }
                    _ => None,
                })
                .collect();
            assert!(!specs.is_empty());
            assert!(specs.iter().all(|spec| {
                spec.absolute_uri.as_ref() == expected(name)
                    && spec.original_uri.as_ref() == expected(name)
            }));
        }
    }

    #[test]
    fn clause_and_expression_preparation_preserve_unrelated_cow_segments() {
        import_preparation_keeps_unrelated_snapshot_segments_shared(false);
    }

    #[test]
    fn triple_clause_and_expression_preparation_preserve_unrelated_cow_segments() {
        import_preparation_keeps_unrelated_snapshot_segments_shared(true);
    }

    #[test]
    fn retiring_dynamic_code_preserves_metadata_added_for_an_older_raw_block() {
        let mut program = Program::new(Vec::new());
        let raw = program.blocks.next_id();
        // Raw development adapters may omit provenance. Filling that earlier
        // gap must not merge its storage with a later query's ownership.
        program.blocks.push(vec![
            Instruction::Constant(Value::Int(17.into())),
            Instruction::Return,
        ]);
        assert!(program.metadata.get(raw).is_none());
        let mut vm = Vm::new(Arc::new(program));
        for _ in 0..3 {
            start(&mut vm, "import(\"memo:/rich.pkl\").tag");
            assert_eq!(settle(&mut vm, &[], false), Value::string("9"));
            assert_eq!(query_count(&vm), 0);
            assert!(vm.program.metadata.get(raw).is_some());
            vm.start(raw, Vec::new()).unwrap();
            assert_eq!(settle(&mut vm, &[], false), Value::Int(17.into()));
        }
    }

    #[test]
    fn twenty_to_two_hundred_uncached_modules_reclaim_all_segment_and_nested_storage() {
        let mut vm = Vm::new(Arc::new(Program::new(Vec::new())));
        let mut baseline = None;
        let mut previous_address = 0;
        for iteration in 1..=200 {
            start(&mut vm, "import(\"memo:/rich.pkl\").tag");
            assert_eq!(settle(&mut vm, &[], false), Value::string("9"));
            // Clauses produce an initial shell and a replacement body under
            // one owner. Each iteration also initializes a dependency module.
            assert!(vm.code_arenas.iter().any(|arena| {
                vm.code_arenas
                    .iter()
                    .filter(|other| other.owner == arena.owner)
                    .count()
                    > 1
            }));
            let current = resident_stats(&vm);
            if iteration == 20 {
                eprintln!("uncached N=20 {current:?}");
                baseline = Some(current);
            } else if iteration > 20 {
                assert_eq!(Some(&current), baseline.as_ref(), "iteration {iteration}");
                if iteration == 200 {
                    eprintln!("uncached N=200 {current:?}");
                }
            }
            assert!(vm.program.blocks.next_id() > previous_address);
            previous_address = vm.program.blocks.next_id();
        }
        vm.close();
        assert_eq!(vm.heap.stats().slots, 0);
    }

    #[test]
    fn escaped_class_alias_and_constraint_closure_keep_old_segments_until_the_last_root_is_gone() {
        let mut vm = Vm::new(Arc::new(Program::new(Vec::new())));
        let mut roots = Vec::new();
        let mut ids = Vec::new();
        let mut owners = Vec::new();
        for expression in [
            "import(\"memo:/rich.pkl\").C",
            "import(\"memo:/rich.pkl\").Checked",
            "(x: Int(this > 0))->import(\"memo:/rich.pkl\").tag + x",
        ] {
            start(&mut vm, expression);
            let value = settle(&mut vm, &roots, true);
            let id = match value {
                Value::Class(id) | Value::TypeAlias(id) | Value::Function(id) => id,
                _ => panic!("escaped declaration"),
            };
            owners.push(vm.heap.get(id).unwrap().environment.code_owner.unwrap());
            ids.push(id);
            roots.push(value);
        }
        let alias = vm.heap.get(ids[1]).unwrap();
        let alias_environment = alias.environment.clone();
        let alias_body = vm.program.alias_definitions[&alias.type_alias_declaration.unwrap()]
            .body
            .clone();
        let old_class_identity = vm
            .heap
            .get(ids[0])
            .unwrap()
            .class_definition
            .as_ref()
            .unwrap()
            .id;
        let program = Arc::make_mut(&mut vm.program);
        program.seal_code_segment();
        let empty = program.objects.try_push(Vec::new()).unwrap();
        let instantiate = program
            .alloc_block(
                vec![
                    Instruction::Input(0),
                    Instruction::MakeInstance { template: empty },
                    Instruction::GetMember("value".into()),
                    Instruction::Return,
                ],
                BlockMetadata::default(),
            )
            .unwrap();
        let call = program
            .alloc_block(
                vec![
                    Instruction::Input(0),
                    Instruction::Input(1),
                    Instruction::Call {
                        arity: 1,
                        method: false,
                    },
                    Instruction::Return,
                ],
                BlockMetadata::default(),
            )
            .unwrap();
        // The helper embeds an old alias's constraint code, so its segment has
        // the same traced owner as the alias rather than a permanent root.
        let arena = CodeArena::begin(program);
        let check_alias = program
            .alloc_block(
                vec![
                    Instruction::Input(0),
                    Instruction::CheckType(alias_body),
                    Instruction::Return,
                ],
                BlockMetadata::default(),
            )
            .unwrap();
        vm.code_arenas.push(arena.finish(program, owners[1], false));

        vm.start(call, vec![roots[2].clone(), Value::Int(1.into())])
            .unwrap();
        assert_eq!(settle(&mut vm, &roots, false), Value::Int(10.into()));
        for _ in 0..40 {
            start(&mut vm, "import(\"memo:/rich.pkl\").tag");
            assert_eq!(settle(&mut vm, &roots, false), Value::string("9"));
            for owner in &owners {
                assert!(vm.heap.executable_is_live(*owner));
            }
        }
        start(&mut vm, "import(\"memo:/rich.pkl\").C");
        let Value::Class(new_class) = settle(&mut vm, &roots, true) else {
            panic!("new class")
        };
        assert_ne!(
            vm.heap
                .get(new_class)
                .unwrap()
                .class_definition
                .as_ref()
                .unwrap()
                .id,
            old_class_identity
        );
        vm.start(instantiate, vec![roots[0].clone()]).unwrap();
        assert_eq!(settle(&mut vm, &roots, false), Value::Int(9.into()));
        for value in [9, 1, 10] {
            vm.start(check_alias, vec![Value::Int(value.into())])
                .unwrap();
            vm.frames.last_mut().unwrap().environment = alias_environment.clone();
            let result = settle_result(&mut vm, &roots, false);
            if value == 1 {
                assert!(result.is_err());
            } else {
                assert_eq!(result.unwrap(), Value::Int(value.into()));
            }
        }
        let requests = vm.next_request;
        vm.start(call, vec![roots[2].clone(), Value::Int(2.into())])
            .unwrap();
        assert_eq!(settle(&mut vm, &roots, false), Value::Int(11.into()));
        assert_eq!(
            vm.next_request, requests,
            "old closure lost its successful import site"
        );
        roots.clear();
        start(&mut vm, "stable");
        settle(&mut vm, &[], false);
        for id in ids {
            assert!(vm.heap.get(id).is_err());
        }
        for owner in owners {
            assert!(!vm.heap.executable_is_live(owner));
            assert!(vm.code_arenas.iter().all(|arena| arena.owner != owner));
        }
        assert!(vm.program.blocks.get(check_alias).is_none());
        assert!(
            vm.start(check_alias, Vec::new()).is_err(),
            "retired code must not alias a later address"
        );
    }

    #[test]
    fn failed_replacement_compile_restores_unit_recipes_aliases_and_existing_code() {
        let initial =
            crate::parser::parse_module("module kept\nclass Keep { value=7 }\nstable=7\n").unwrap();
        let initial = crate::resolver::resolve_module(&initial).unwrap();
        let prepared = compile_module(&initial).unwrap();
        let mut program = (*prepared).clone();
        let replacement = crate::parser::parse_module("module changed\nclass Added { value=9 }\ntypealias AddedAlias = Int(this > 0)\nf=(x:module)->x\nbad=missing\n").unwrap();
        let mut replacement = crate::resolver::resolve_module_for_evaluation(&replacement);
        let bad = replacement.module.members.last().unwrap().expr.span;
        replacement
            .names
            .insert(bad, crate::resolver::Binding::Lexical { depth: 0, slot: 1 });
        let original_unit = format!("{:?}", program.units[0]);
        let original = (
            program.blocks.next_id(),
            program.objects.next_id(),
            program.type_recipes.next_id(),
            program.alias_definitions.len(),
            program.constraint_origins.len(),
            program.module_name.clone(),
            program.entry,
        );
        let checkpoint = modules::ArenaCheckpoint::new(&mut program);
        let error = compile_unit_body_at(
            Some(crate::modules::UnitId(0)),
            &mut program,
            &replacement,
            CompileOptions::default(),
            false,
            None,
            None,
        )
        .unwrap_err();
        assert_eq!(error.kind, ErrorKind::InvalidBytecode);
        assert!(program.blocks.next_id() > original.0);
        assert!(program.objects.next_id() > original.1);
        assert!(program.type_recipes.next_id() > original.2);
        assert!(program.alias_definitions.len() > original.3);
        checkpoint.rollback(&mut program);
        assert_eq!(
            (
                program.blocks.next_id(),
                program.objects.next_id(),
                program.type_recipes.next_id(),
                program.alias_definitions.len(),
                program.constraint_origins.len(),
                program.module_name.clone(),
                program.entry
            ),
            original
        );
        assert_eq!(format!("{:?}", program.units[0]), original_unit);
        assert_eq!(program.units.len(), prepared.units.len());
        assert_eq!(program.blocks.len(), prepared.blocks.len());
        let entry = program.entry;
        let mut vm = Vm::new(Arc::new(program));
        vm.start(entry, Vec::new()).unwrap();
        let module = settle(&mut vm, &[], false);
        vm.start_member(module.as_object().unwrap(), "stable")
            .unwrap();
        assert_eq!(settle(&mut vm, &[module], false), Value::Int(7.into()));
    }
    #[test]
    fn overwritten_uncached_modules_release_class_alias_and_source_code_graphs() {
        let mut vm = Vm::new(Arc::new(Program::new(Vec::new())));
        let mut baseline = None;
        for iteration in 0..12 {
            start(&mut vm, "import(\"memo:/child.pkl\").tag");
            assert_eq!(settle(&mut vm, &[], false), Value::string("9"));
            let stats = vm.heap.stats();
            let retained = (
                stats.live,
                stats.executable_live,
                vm.program.blocks.iter().map(Vec::len).sum::<usize>(),
                vm.program.objects.iter().map(Vec::len).sum::<usize>(),
                vm.program
                    .units
                    .iter()
                    .filter(|unit| unit.source.is_some())
                    .count(),
                vm.program
                    .type_recipes
                    .iter()
                    .filter(|recipe| recipe.unit.0 != u32::MAX)
                    .count(),
                vm.program.alias_definitions.len(),
                vm.code_arenas.len(),
            );
            if let Some(expected) = baseline {
                assert_eq!(retained, expected);
            } else {
                baseline = Some(retained);
            }
            if iteration == 0 || iteration == 11 {
                eprintln!("uncached iteration={iteration} {:?}", resident_stats(&vm));
            }
        }
        vm.close();
        assert_eq!(vm.heap.stats().slots, 0);
    }
    #[test]
    fn ordinary_completed_queries_reclaim_code_sources_and_type_recipes() {
        let mut vm = Vm::new(Arc::new(Program::new(Vec::new())));
        start(&mut vm, "stable");
        assert_eq!(settle(&mut vm, &[], false), Value::string("7"));
        let initial = counts(&vm);
        for _ in 0..16 {
            start(&mut vm, "((x: Int)->x + stable).apply(2)");
            assert_eq!(settle(&mut vm, &[], false), Value::string("9"));
            assert_eq!(query_count(&vm), 0);
            assert!(vm.public_operation_entries.is_empty());
            assert_eq!(counts(&vm), initial);
        }
    }
    #[test]
    fn repeated_failed_queries_release_code_but_retained_errors_keep_their_sources() {
        let mut vm = Vm::new(Arc::new(Program::new(Vec::new())));
        start(&mut vm, "stable");
        settle(&mut vm, &[], false);
        let initial = counts(&vm);
        let mut failures = Vec::new();
        for _ in 0..8 {
            for expression in ["(", "throw(\"failed query\")", "new Missing {}"] {
                start(&mut vm, expression);
                let error = settle_result(&mut vm, &[], false).unwrap_err();
                assert!(
                    error.frames.iter().any(|frame| frame
                        .source
                        .as_ref()
                        .is_some_and(|source| source.text.as_ref() == expression)),
                    "{error:?}"
                );
                failures.push((expression, error));
                assert_eq!(query_count(&vm), 0);
                assert_eq!(counts(&vm), initial);
            }
        }
        start(&mut vm, "stable");
        settle(&mut vm, &[], false);
        for (expression, error) in failures {
            assert!(error.frames.iter().any(|frame| {
                frame
                    .source
                    .as_ref()
                    .is_some_and(|source| source.text.as_ref() == expression)
            }));
        }
    }
    #[test]
    fn cached_module_error_keeps_its_source_after_code_retirement() {
        let mut vm = Vm::new(Arc::new(Program::new(Vec::new())));
        let expression = "import(\"memo:/broken.pkl\").tag";
        start(&mut vm, expression);
        let first = settle_result(&mut vm, &[], false).unwrap_err();
        assert_eq!(query_count(&vm), 0);
        assert!(first.frames.iter().any(|frame| {
            frame
                .source
                .as_ref()
                .is_some_and(|source| source.text.as_ref() == "p: Missing=1\ntag=0")
        }));
        assert!(!vm.program.units.iter().any(|unit| {
            unit.source
                .as_ref()
                .is_some_and(|source| source.uri.as_ref() == "memo:/broken.pkl")
        }));
        start(&mut vm, "import(\"memo:/broken.pkl\").tag + 1");
        let second = loop {
            match vm.poll(1) {
                VmPoll::Yielded => {}
                VmPoll::Module(request) => {
                    let completion =
                        if matches!(request.kind, ModuleRequestKind::QueryCachePolicy { .. }) {
                            ModuleCompletion::CachePolicy(Ok(true))
                        } else {
                            complete(&request)
                        };
                    vm.complete_module(request.id, completion).unwrap();
                }
                VmPoll::Ready(result) => break result.unwrap_err(),
                event => panic!("unexpected cached error event {event:?}"),
            }
            vm.collect(&[]);
        };
        vm.collect(&[]);
        assert_eq!(first.frames, second.frames);
        assert_eq!(query_count(&vm), 0);
        assert!(second.frames.iter().any(|frame| {
            frame
                .source
                .as_ref()
                .is_some_and(|source| source.text.as_ref() == "p: Missing=1\ntag=0")
        }));
    }
    #[test]
    fn reentrant_uncached_imports_plateau_while_the_parent_remains_suspended() {
        let mut vm = Vm::new(Arc::new(Program::new(Vec::new())));
        start(&mut vm, "read(\"prop:pause\") + stable.toString()");
        let pending = loop {
            match vm.poll(1) {
                VmPoll::Yielded => {}
                VmPoll::Module(request) => {
                    vm.complete_module(request.id, complete(&request)).unwrap()
                }
                VmPoll::Pending(request) => break request.id,
                event => panic!("unexpected parent event {event:?}"),
            }
            vm.collect(&[]);
        };
        let parent = vm.code_arenas.iter().find(|arena| arena.query).unwrap();
        let parent_owner = parent.owner;
        let parent_blocks = parent.blocks.clone();
        let mut baseline = None;
        for iteration in 1..=200 {
            let child = vm
                .append_public_operation(
                    ModuleSourceSpec {
                        uri: "memo:/main.pkl".into(),
                        contents: Some("stable=7".into()),
                    },
                    "import(\"memo:/rich.pkl\").tag".into(),
                )
                .unwrap();
            vm.reenter(pending, child, Vec::new()).unwrap();
            assert_eq!(settle(&mut vm, &[], false), Value::string("9"));
            vm.finish_reentry();
            vm.collect(&[]);
            assert_eq!(query_count(&vm), 1);
            assert!(vm.heap.executable_is_live(parent_owner));
            assert!(
                parent_blocks
                    .clone()
                    .all(|id| vm.program.blocks.get(id).is_some())
            );
            let current = resident_stats(&vm);
            if iteration == 20 {
                eprintln!("reentrant N=20 {current:?}");
                baseline = Some(current);
            } else if iteration > 20 {
                assert_eq!(Some(&current), baseline.as_ref(), "iteration {iteration}");
                if iteration == 200 {
                    eprintln!("reentrant N=200 {current:?}");
                }
            }
        }
        vm.resume(pending, Ok(Value::string("parent/"))).unwrap();
        assert_eq!(settle(&mut vm, &[], false), Value::string("parent/7"));
        assert_eq!(query_count(&vm), 0);
        assert!(!vm.heap.executable_is_live(parent_owner));
        assert!(
            parent_blocks
                .clone()
                .all(|id| vm.program.blocks.get(id).is_none())
        );
    }

    #[test]
    fn nested_public_queries_keep_the_suspended_parents_code_until_resume() {
        let mut vm = Vm::new(Arc::new(Program::new(Vec::new())));
        start(&mut vm, "stable");
        settle(&mut vm, &[], false);
        let initial = counts(&vm);
        for _ in 0..8 {
            start(&mut vm, "read(\"prop:pause\") + stable.toString()");
            let pending = loop {
                match vm.poll(1) {
                    VmPoll::Yielded => {}
                    VmPoll::Module(request) => {
                        vm.complete_module(request.id, complete(&request)).unwrap()
                    }
                    VmPoll::Pending(request) => break request.id,
                    event => panic!("unexpected parent event {event:?}"),
                }
                vm.collect(&[]);
            };
            let parent = vm
                .code_arenas
                .iter()
                .find(|arena| arena.query)
                .unwrap()
                .owner;
            let child = vm
                .append_public_operation(
                    ModuleSourceSpec {
                        uri: "memo:/main.pkl".into(),
                        contents: Some("stable=7".into()),
                    },
                    "stable".into(),
                )
                .unwrap();
            vm.reenter(pending, child, Vec::new()).unwrap();
            assert_eq!(settle(&mut vm, &[], false), Value::string("7"));
            assert_eq!(query_count(&vm), 1);
            assert!(vm.heap.executable_is_live(parent));
            vm.finish_reentry();
            vm.collect(&[]);
            vm.resume(pending, Ok(Value::string("parent/"))).unwrap();
            assert_eq!(settle(&mut vm, &[], false), Value::string("parent/7"));
            assert_eq!(query_count(&vm), 0);
            assert_eq!(counts(&vm), initial);
        }
    }
    #[test]
    fn escaped_object_keeps_lazy_property_and_constraint_code_until_collection() {
        let mut vm = Vm::new(Arc::new(Program::new(Vec::new())));
        start(
            &mut vm,
            "new Dynamic { local checked: Int(this > 7) = 8; result=checked }",
        );
        let object = settle(&mut vm, &[], true);
        start(&mut vm, "stable");
        settle(&mut vm, std::slice::from_ref(&object), false);
        vm.start_member(object.as_object().unwrap(), "result")
            .unwrap();
        assert_eq!(
            settle(&mut vm, std::slice::from_ref(&object), false),
            Value::Int(8.into())
        );
        assert_eq!(query_count(&vm), 1);
        start(&mut vm, "stable");
        settle(&mut vm, &[], false);
        assert_eq!(query_count(&vm), 0);
    }
    #[test]
    fn escaped_function_type_constraints_keep_query_code_through_gc_and_failure() {
        let mut vm = Vm::new(Arc::new(Program::new(Vec::new())));
        start(&mut vm, "(x: Int(this > 7))->x");
        let closure = settle(&mut vm, &[], true);
        start(&mut vm, "stable");
        settle(&mut vm, std::slice::from_ref(&closure), false);
        let program = Arc::make_mut(&mut vm.program);
        let call = program.blocks.next_id();
        program.blocks.push(vec![
            Instruction::Input(0),
            Instruction::Input(1),
            Instruction::Call {
                arity: 1,
                method: false,
            },
            Instruction::Return,
        ]);
        program.metadata.push(BlockMetadata::default());
        for n in [9, 1, 10] {
            vm.start(call, vec![closure.clone(), Value::Int(n.into())])
                .unwrap();
            let result = settle_result(&mut vm, std::slice::from_ref(&closure), false);
            if n == 1 {
                assert!(result.is_err());
            } else {
                assert_eq!(result.unwrap(), Value::Int(n.into()));
            }
            assert_eq!(query_count(&vm), 1);
        }
        start(&mut vm, "stable");
        settle(&mut vm, &[], false);
        assert_eq!(query_count(&vm), 0);
    }
    #[test]
    fn parameterized_currying_and_lazy_closures_retain_and_release_query_code() {
        let mut vm = Vm::new(Arc::new(Program::new(Vec::new())));
        start(&mut vm, "stable");
        settle(&mut vm, &[], false);
        let program = Arc::make_mut(&mut vm.program);
        let call = program.blocks.next_id();
        program.blocks.push(vec![
            Instruction::Input(0),
            Instruction::Input(1),
            Instruction::Call {
                arity: 1,
                method: false,
            },
            Instruction::Return,
        ]);
        program.metadata.push(BlockMetadata::default());
        let initial = resident_stats(&vm);
        for iteration in 0..40 {
            start(
                &mut vm,
                "(((first) -> (second) -> new Dynamic { parent = first + second }) { first: Int(this > 0) -> captured = (last) -> first + last }).apply(7)",
            );
            let curried = settle(&mut vm, &[], true);
            start(&mut vm, "stable");
            settle(&mut vm, std::slice::from_ref(&curried), false);
            assert_eq!(query_count(&vm), 1);
            vm.start(call, vec![curried, Value::Int(20.into())])
                .unwrap();
            let object = settle(&mut vm, &[], true);
            vm.start_member(object.as_object().unwrap(), "parent")
                .unwrap();
            assert_eq!(
                settle(&mut vm, std::slice::from_ref(&object), false),
                Value::Int(27.into())
            );
            vm.start_member(object.as_object().unwrap(), "captured")
                .unwrap();
            let closure = settle(&mut vm, &[], true);
            start(&mut vm, "stable");
            settle(&mut vm, std::slice::from_ref(&closure), false);
            assert_eq!(query_count(&vm), 1);
            vm.start(call, vec![closure, Value::Int(30.into())])
                .unwrap();
            assert_eq!(settle(&mut vm, &[], false), Value::Int(37.into()));
            start(&mut vm, "stable");
            settle(&mut vm, &[], false);
            assert_eq!(query_count(&vm), 0);
            if iteration == 3 || iteration == 39 {
                assert_eq!(resident_stats(&vm), initial);
            }
        }
    }
    #[test]
    fn escaped_query_closure_keeps_its_code_and_site_cache_until_its_last_root_is_gone() {
        let mut vm = Vm::new(Arc::new(Program::new(Vec::new())));
        start(&mut vm, "(x)->import(\"memo:/child.pkl\").tag + x");
        let closure = settle(&mut vm, &[], true);
        assert!(matches!(closure, Value::Function(_)));
        let blocks = vm
            .code_arenas
            .iter()
            .find(|arena| arena.query)
            .unwrap()
            .blocks
            .clone();
        start(&mut vm, "stable");
        assert_eq!(
            settle(&mut vm, std::slice::from_ref(&closure), false),
            Value::string("7")
        );
        assert_eq!(query_count(&vm), 1);
        let program = Arc::make_mut(&mut vm.program);
        let call = program.blocks.next_id();
        program.blocks.push(vec![
            Instruction::Input(0),
            Instruction::Input(1),
            Instruction::Call {
                arity: 1,
                method: false,
            },
            Instruction::Return,
        ]);
        program.metadata.push(BlockMetadata::default());
        for n in [1, 2] {
            let requests_before = vm.next_request;
            vm.start(call, vec![closure.clone(), Value::Int(n.into())])
                .unwrap();
            assert_eq!(
                settle(&mut vm, std::slice::from_ref(&closure), false),
                Value::Int((n + 9).into())
            );
            if n == 2 {
                assert_eq!(
                    vm.next_request, requests_before,
                    "escaped closure lost its successful import site"
                );
            }
        }
        start(&mut vm, "stable");
        assert_eq!(settle(&mut vm, &[], false), Value::string("7"));
        assert_eq!(query_count(&vm), 0);
        for block in blocks {
            assert!(vm.program.blocks.get(block).is_none_or(Vec::is_empty));
        }
        vm.close();
        assert_eq!(vm.heap.stats().slots, 0);
    }
}
