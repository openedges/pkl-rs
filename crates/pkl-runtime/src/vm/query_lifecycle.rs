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
    pub(super) fn begin(program: &Program) -> Start {
        Start {
            blocks: program.blocks.len(),
            objects: program.objects.len(),
            units: program.units.len(),
            recipes: program.type_recipes.len(),
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
        for block in self.blocks.clone() {
            program.blocks[block] = Vec::new();
            program.metadata[block] = BlockMetadata::default();
        }
        for object in self.objects.clone() {
            program.objects[object] = Vec::new();
        }
        for recipe in self.recipes.clone() {
            program.type_recipes[recipe] = type_resolution::TypeRecipe {
                unit: crate::modules::UnitId(u32::MAX),
                path: Vec::new(),
                names: Vec::new(),
                span: Span::default(),
                argument_spans: Vec::new(),
                builtin: None,
                reference: None,
            };
        }
        for unit in self.units.clone() {
            let unit = &mut program.units[unit];
            unit.identity = None;
            unit.key = None;
            unit.delegate_key = None;
            unit.build = None;
            unit.syntax = None;
            unit.source = None;
            unit.name = "".into();
            unit.template = usize::MAX;
            unit.entry = usize::MAX;
            unit.initial_members.clear();
            unit.import_order.clear();
        }
        program
            .alias_definitions
            .retain(|id, _| !self.aliases.contains(id));
        program
            .constraint_origins
            .retain(|block, _| !self.blocks.contains(block));
        // Loaded modules can be appended while a query is running. Preserve
        // their numeric indices; non-tail retired ranges keep empty slots.
        if self.blocks.end == program.blocks.len() {
            program.blocks.truncate(self.blocks.start);
            program.metadata.truncate(self.blocks.start);
        }
        if self.objects.end == program.objects.len() {
            program.objects.truncate(self.objects.start);
        }
        if self.recipes.end == program.type_recipes.len() {
            program.type_recipes.truncate(self.recipes.start);
        }
        if self.units.end == program.units.len() {
            program.units.truncate(self.units.start);
        }
    }
}
impl Start {
    pub(super) fn finish(
        self,
        program: &Program,
        owner: ExecutableOwnerId,
        query: bool,
    ) -> CodeArena {
        CodeArena {
            owner,
            query,
            blocks: self.blocks..program.blocks.len(),
            objects: self.objects..program.objects.len(),
            units: self.units..program.units.len(),
            recipes: self.recipes..program.type_recipes.len(),
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
            program.blocks[entry] = Vec::new();
            program.metadata[entry] = BlockMetadata::default();
            if entry + 1 == program.blocks.len() {
                program.blocks.pop();
                program.metadata.pop();
            }
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
            // Payload reclamation is independent of arena slot reclamation.
            // Non-tail tombstones are still retained; record that open memory
            // limitation explicitly rather than counting only live payload.
            eprintln!(
                "uncached iteration={iteration} slots={:?} capacity={:?}",
                counts(&vm),
                (
                    vm.program.blocks.capacity(),
                    vm.program.objects.capacity(),
                    vm.program.units.capacity(),
                    vm.program.type_recipes.capacity(),
                )
            );
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
        let call = program.blocks.len();
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
        let call = program.blocks.len();
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
