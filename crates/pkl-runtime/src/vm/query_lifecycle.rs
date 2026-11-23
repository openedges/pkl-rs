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
                    .is_some_and(|source| source.uri == "repl:text"))
        );
        if self.units.contains(&0)
            || program
                .module_template
                .is_some_and(|template| self.objects.contains(&template))
        {
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
        program
            .declaration_sources
            .retain(|block, _| !self.blocks.contains(block));
        program
            .extra_declaration_sources
            .retain(|(range, _)| range.end <= self.blocks.start || range.start >= self.blocks.end);
        program
            .reflection_annotations
            .retain(|_, block| !self.blocks.contains(block));
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
/// First matching arena for each half-open ID range, rebuilt per collection.
/// A boundary sweep preserves linear `.position()` behavior even for overlapping
/// ranges. It indexes arena boundaries, not every instruction/recipe/object ID.
#[derive(Default)]
struct ArenaRangeIndex {
    boundaries: Vec<(usize, Option<usize>)>,
}
impl ArenaRangeIndex {
    fn new<'a>(ranges: impl Iterator<Item = &'a Range<usize>>) -> Self {
        let mut changes = Vec::new();
        for (arena, range) in ranges.enumerate() {
            if !range.is_empty() {
                changes.push((range.start, arena, true));
                changes.push((range.end, arena, false));
            }
        }
        changes.sort_unstable_by_key(|change| change.0);
        let mut active = BTreeSet::new();
        let mut boundaries = Vec::new();
        for group in changes.chunk_by(|left, right| left.0 == right.0) {
            for &(_, arena, start) in group {
                if start {
                    active.insert(arena);
                } else {
                    active.remove(&arena);
                }
            }
            boundaries.push((group[0].0, active.first().copied()));
        }
        Self { boundaries }
    }

    fn get(&self, id: usize) -> Option<usize> {
        let after = self.boundaries.partition_point(|(start, _)| *start <= id);
        after
            .checked_sub(1)
            .and_then(|index| self.boundaries[index].1)
    }
}

impl Vm {
    pub(super) fn declaration_root_counts(&self) -> (usize, usize, usize, usize, usize, usize) {
        (
            self.code_arenas.len(),
            self.resolved_declarations.len(),
            self.type_parent_defaults.len(),
            self.executable_roots.len(),
            self.class_singletons.len(),
            self.alias_singletons.len(),
        )
    }

    #[cfg_attr(target_arch = "wasm32", inline(never))]
    pub(super) fn declaration_code_roots(&mut self, roots: &mut Vec<Value>) {
        let small = self.code_arenas.len() <= 8;
        let recipes = if small || self.resolved_declarations.is_empty() {
            ArenaRangeIndex::default()
        } else {
            ArenaRangeIndex::new(self.code_arenas.iter().map(|arena| &arena.recipes))
        };
        let blocks = if small
            || (self.type_parent_defaults.is_empty() && self.executable_roots.is_empty())
        {
            ArenaRangeIndex::default()
        } else {
            ArenaRangeIndex::new(self.code_arenas.iter().map(|arena| &arena.blocks))
        };
        let objects = if small || self.class_singletons.is_empty() {
            ArenaRangeIndex::default()
        } else {
            ArenaRangeIndex::new(self.code_arenas.iter().map(|arena| &arena.objects))
        };
        let mut aliases = BTreeMap::new();
        let small_aliases = self.code_arenas.len() <= 16;
        if !small_aliases && !self.alias_singletons.is_empty() {
            for (index, arena) in self.code_arenas.iter().enumerate() {
                for alias in &arena.aliases {
                    aliases.entry(*alias).or_insert(index);
                }
            }
        }
        let mut owned = vec![Vec::new(); self.code_arenas.len()];
        let mut unowned = std::mem::take(&mut self.unowned_declaration_roots);
        unowned.clear();
        let mut retain = |index: Option<usize>, value: Value| {
            if let Some(index) = index {
                owned[index].push(value);
            } else {
                unowned.push(value);
            }
        };
        for ((recipe, owner), value) in &self.resolved_declarations {
            let index = if small {
                self.code_arenas
                    .iter()
                    .position(|arena| arena.recipes.contains(recipe))
            } else {
                recipes.get(*recipe)
            };
            retain(index, Value::Object(*owner));
            if let type_resolution::DeclarationValue::Value(value) = value {
                retain(index, value.clone());
            }
        }
        for ((block, _), value) in &self.type_parent_defaults {
            let index = if small {
                self.code_arenas
                    .iter()
                    .position(|arena| arena.blocks.contains(block))
            } else {
                blocks.get(*block)
            };
            retain(index, value.clone());
        }
        for (block, owner) in self.executable_roots.iter() {
            let index = if small {
                self.code_arenas
                    .iter()
                    .position(|arena| arena.blocks.contains(&block))
            } else {
                blocks.get(block)
            };
            retain(index, owner.root());
        }
        for (template, id) in &self.class_singletons {
            let index = if small {
                self.code_arenas
                    .iter()
                    .position(|arena| arena.objects.contains(template))
            } else {
                objects.get(*template)
            };
            retain(index, Value::Class(*id));
        }
        for (declaration, id) in &self.alias_singletons {
            let index = if small_aliases {
                self.code_arenas
                    .iter()
                    .position(|arena| arena.aliases.contains(declaration))
            } else {
                aliases.get(declaration).copied()
            };
            retain(index, Value::TypeAlias(*id));
        }
        let mut by_owner: Vec<(ExecutableOwnerId, ObjectId, Vec<Value>)> =
            Vec::with_capacity(self.code_arenas.len());
        for (arena, roots) in self.code_arenas.iter().zip(owned) {
            let owner_key = arena.owner.root().heap_id().expect("executable owner ID");
            if let Some((_, _, existing)) =
                by_owner.iter_mut().find(|(_, key, _)| *key == owner_key)
            {
                existing.extend(roots);
            } else {
                by_owner.push((arena.owner, owner_key, roots));
            }
        }
        for (owner, _, roots) in by_owner {
            self.heap.set_code_roots(owner, roots);
        }
        roots.extend(unowned.iter().cloned());
        self.unowned_declaration_roots = unowned;
        self.declaration_roots_dirty = false;
        self.declaration_roots_lens = self.declaration_root_counts();
    }

    #[cfg_attr(target_arch = "wasm32", inline(never))]
    pub(super) fn retire_unreachable_code(&mut self) {
        let mut retired_arena_blocks: Vec<Range<usize>> = Vec::new();
        for index in (0..self.code_arenas.len()).rev() {
            if self.heap.executable_is_live(self.code_arenas[index].owner) {
                continue;
            }
            self.declaration_roots_dirty = true;
            let arena = self.code_arenas.remove(index);
            self.resolved_declarations
                .retain(|(recipe, _), _| !arena.recipes.contains(recipe));
            self.type_parent_defaults
                .retain(|(block, _), _| !arena.blocks.contains(block));
            if !arena.blocks.is_empty() {
                retired_arena_blocks.push(arena.blocks.clone());
            }
            self.class_singletons
                .retain(|template, _| !arena.objects.contains(template));
            self.alias_singletons
                .retain(|declaration, _| !arena.aliases.contains(declaration));
            arena.retire(Arc::make_mut(&mut self.program));
        }
        if !retired_arena_blocks.is_empty() {
            self.executable_roots.retain(|block, _| {
                !retired_arena_blocks
                    .iter()
                    .any(|range| range.contains(block))
            });
            self.resolved_instruction_types
                .retain(|(block, _, owner, _), _| {
                    !retired_arena_blocks
                        .iter()
                        .any(|range| range.contains(block))
                        && self.heap.get(*owner).is_ok()
                });
            self.resolved_property_types.retain(|(block, owner), _| {
                !retired_arena_blocks
                    .iter()
                    .any(|range| range.contains(block))
                    && self.heap.get(*owner).is_ok()
            });
        } else {
            self.executable_roots.compact();
            if !self.resolved_instruction_types.is_empty() {
                self.resolved_instruction_types
                    .retain(|(_, _, owner, _), _| self.heap.get(*owner).is_ok());
            }
            if !self.resolved_property_types.is_empty() {
                self.resolved_property_types
                    .retain(|(_, owner), _| self.heap.get(*owner).is_ok());
            }
        }
        if !self.query_cache.is_empty() {
            self.query_cache
                .retain(|cached| self.heap.executable_is_live(cached.owner));
        }
        let mut retired_public_entries: Vec<usize> = Vec::new();
        if !self.public_operation_entries.is_empty() {
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
                if !self.fast_int_cache.is_empty() {
                    retired_public_entries.push(entry);
                }
                let program = Arc::make_mut(&mut self.program);
                program.blocks.retire(entry..entry + 1);
                program.metadata.retire(entry..entry + 1);
                program.declaration_sources.remove(&entry);
                program
                    .reflection_annotations
                    .retain(|_, block| *block != entry);
            }
        }
        if !retired_arena_blocks.is_empty() || !retired_public_entries.is_empty() {
            self.fast_int_cache.retain(|block_id| {
                !retired_arena_blocks
                    .iter()
                    .any(|range| range.contains(&block_id))
                    && !retired_public_entries.contains(&block_id)
            });
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

    #[test]
    fn cyclic_ephemeral_modules_keep_executable_roots_and_fast_int_cache_bounded() {
        let mut session = crate::evaluation::session::SourceSession::module_evaluator(
            crate::evaluation::session::SourceSessionOptions::default(),
        );
        for step in 0..200usize {
            let pad = format!("seg_{step}_abcdefghij");
            let src = format!(
                "class Node {{\n  id: Int\n  payload: String\n  next: Node\n}}\n\
                 local a {{ tag = \"a-{step}\"; payload = \"{pad}\"; peer = b }}\n\
                 local b {{ tag = \"b-{step}\"; payload = \"{pad}\"; peer = a }}\n\
                 local n0: Node = new Node {{ id = {i0}; payload = \"{pad}\"; next = n1 }}\n\
                 local n1: Node = new Node {{ id = {i1}; payload = \"{pad}\"; next = n0 }}\n\
                 local f = (k) -> if (k <= 0) n0.id else g.apply(k - 1) + 1\n\
                 local g = (k) -> if (k <= 0) n1.id else f.apply(k - 1) + 2\n\
                 summary = \"\\(a.peer.peer.tag)|\\(n0.next.next.id)|\\(f.apply(6))\"\n",
                i0 = step,
                i1 = step + 1,
            );
            let spec = ModuleSourceSpec {
                uri: "repl:text".into(),
                contents: Some(src.into()),
            };
            session.start_evaluation(spec, "summary", 0, None).unwrap();
            loop {
                match session.poll(0, 100_000) {
                    crate::host::HostPoll::Yielded => {}
                    crate::host::HostPoll::Ready(Ok(_)) => break,
                    crate::host::HostPoll::Module { token, request } => {
                        let comp = crate::modules::default_completion(&request).unwrap();
                        session.complete_module(&token, comp, 0).unwrap();
                    }
                    other => panic!("unexpected {other:?}"),
                }
            }
        }
        let vm = session.vm_mut();
        assert!(
            vm.executable_roots.local_capacity() <= 512,
            "executable_roots local table must stay O(live), got cap={}",
            vm.executable_roots.local_capacity()
        );
        assert!(
            vm.fast_int_cache.capacity() <= 512,
            "fast_int_cache must stay O(live), got cap={}",
            vm.fast_int_cache.capacity()
        );
    }

    #[test]
    fn arena_range_index_keeps_first_membership_across_overlaps_gaps_and_empty_ranges() {
        let ranges = [10..20, 0..30, 15..35, 30..30, usize::MAX - 2..usize::MAX];
        let indexed = ArenaRangeIndex::new(ranges.iter());
        for id in [
            0,
            9,
            10,
            14,
            15,
            19,
            20,
            24,
            25,
            29,
            30,
            34,
            35,
            usize::MAX - 3,
            usize::MAX - 2,
            usize::MAX - 1,
            usize::MAX,
        ] {
            assert_eq!(
                indexed.get(id),
                ranges.iter().position(|range| range.contains(&id)),
                "first arena for {id}"
            );
        }
        assert_eq!(ArenaRangeIndex::new(std::iter::empty()).get(0), None);
    }

    #[test]
    fn declaration_roots_keep_repeated_owner_order_and_refresh_materialized_aliases() {
        let mut vm = Vm::new(Arc::new(Program::new(Vec::new())));
        let stale = vm.heap.allocate_executable();
        let stale_id = stale.root().heap_id().unwrap();
        vm.collect(&[]);
        assert!(!vm.heap.executable_is_live(stale));
        let first = vm.heap.allocate_executable();
        let first_id = first.root().heap_id().unwrap();
        assert_eq!(first_id.heap, stale_id.heap);
        assert_eq!(first_id.index, stale_id.index);
        assert_ne!(first_id.generation, stale_id.generation);
        assert_ne!(first, stale);
        let second = vm.heap.allocate_executable();
        assert_ne!(first, second);
        assert!(vm.code_arenas.is_empty());
        vm.code_arenas = vec![
            CodeArena {
                owner: first,
                query: false,
                blocks: 10..20,
                objects: 10..20,
                units: 0..0,
                recipes: 10..20,
                aliases: BTreeSet::from([100, 101]),
            },
            CodeArena {
                owner: second,
                query: false,
                blocks: 20..30,
                objects: 20..30,
                units: 0..0,
                recipes: 20..30,
                aliases: BTreeSet::from([200]),
            },
            CodeArena {
                owner: first,
                query: false,
                blocks: 30..40,
                objects: 30..40,
                units: 0..0,
                recipes: 30..40,
                aliases: BTreeSet::from([300, 301]),
            },
        ];
        let declarations: Vec<_> = (0..4)
            .map(|_| vm.heap.allocate(Object::default()))
            .collect();
        let executables: Vec<_> = (0..4).map(|_| vm.heap.allocate_executable()).collect();
        let classes: Vec<_> = (0..4)
            .map(|_| {
                let prototype = vm.heap.allocate(Object::default());
                vm.heap.allocate_class(prototype)
            })
            .collect();
        let aliases: Vec<_> = (0..6)
            .map(|_| vm.heap.allocate(Object::default()))
            .collect();
        for (index, recipe) in [11, 21, 31, 99].into_iter().enumerate() {
            vm.resolved_declarations.insert(
                (recipe, declarations[index]),
                type_resolution::DeclarationValue::Value(Value::Int(
                    (111 + index as i64 * 100).into(),
                )),
            );
        }
        for (index, block) in [12, 22, 32, 98].into_iter().enumerate() {
            vm.type_parent_defaults
                .insert((block, 0), Value::Int((112 + index as i64 * 100).into()));
        }
        for (index, block) in [13, 23, 33, 97].into_iter().enumerate() {
            vm.executable_roots.insert(block, executables[index]);
        }
        for (index, template) in [14, 24, 34, 96].into_iter().enumerate() {
            vm.class_singletons.insert(template, classes[index]);
        }
        for (index, alias) in [100, 101, 200, 300, 999].into_iter().enumerate() {
            vm.alias_singletons.insert(alias, aliases[index]);
        }
        let mut first_expected = vec![
            Value::Object(declarations[0]),
            Value::Int(111.into()),
            Value::Int(112.into()),
            executables[0].root(),
            Value::Class(classes[0]),
            Value::TypeAlias(aliases[0]),
            Value::TypeAlias(aliases[1]),
            Value::Object(declarations[2]),
            Value::Int(311.into()),
            Value::Int(312.into()),
            executables[2].root(),
            Value::Class(classes[2]),
            Value::TypeAlias(aliases[3]),
        ];
        let second_expected = vec![
            Value::Object(declarations[1]),
            Value::Int(211.into()),
            Value::Int(212.into()),
            executables[1].root(),
            Value::Class(classes[1]),
            Value::TypeAlias(aliases[2]),
        ];
        let unowned_expected = vec![
            Value::Int((-1).into()),
            Value::Object(declarations[3]),
            Value::Int(411.into()),
            Value::Int(412.into()),
            executables[3].root(),
            Value::Class(classes[3]),
            Value::TypeAlias(aliases[4]),
        ];
        let mut unowned = vec![Value::Int((-1).into())];
        vm.declaration_code_roots(&mut unowned);
        assert_eq!(unowned, unowned_expected);
        assert_eq!(
            vm.heap.code_roots_for_test(first),
            first_expected.as_slice()
        );
        assert_eq!(
            vm.heap.code_roots_for_test(second),
            second_expected.as_slice()
        );

        // A declaration in the third arena materializes after the first GC-root
        // enumeration. Rebuilding must replace the shared owner's complete edge
        // vector, retaining earlier arena order and adding exactly one new edge.
        vm.alias_singletons.insert(301, aliases[5]);
        first_expected.push(Value::TypeAlias(aliases[5]));
        let mut refreshed = vec![Value::Int((-1).into())];
        vm.declaration_code_roots(&mut refreshed);
        assert_eq!(refreshed, unowned_expected);
        assert_eq!(
            vm.heap.code_roots_for_test(first),
            first_expected.as_slice()
        );
        assert_eq!(
            vm.heap.code_roots_for_test(second),
            second_expected.as_slice()
        );

        // Observe the actual published executable edges through tracing. With
        // only the first owner rooted, its aliases survive and the other owner's
        // alias is reclaimed; unowned values survive only via explicit roots.
        refreshed.push(first.root());
        vm.heap.collect(&refreshed);
        for index in [0, 1, 3, 4, 5] {
            assert!(vm.heap.get(aliases[index]).is_ok());
        }
        assert!(vm.heap.get(aliases[2]).is_err());
        assert!(vm.heap.executable_is_live(first));
        assert!(!vm.heap.executable_is_live(second));
    }

    #[test]
    fn empty_declaration_maps_clear_previously_published_owner_edges() {
        // Like the repeated-owner test, this constructs publication dataflow
        // over an empty Program; it does not claim compiler/VM retirement coverage.
        let mut vm = Vm::new(Arc::new(Program::new(Vec::new())));
        let owner = vm.heap.allocate_executable();
        let alias = vm.heap.allocate(Object::default());
        vm.code_arenas.push(CodeArena {
            owner,
            query: false,
            blocks: 0..0,
            objects: 0..0,
            units: 0..0,
            recipes: 0..0,
            aliases: BTreeSet::from([100]),
        });
        vm.alias_singletons.insert(100, alias);
        let mut roots = Vec::new();
        vm.declaration_code_roots(&mut roots);
        assert!(roots.is_empty());
        assert_eq!(
            vm.heap.code_roots_for_test(owner),
            &[Value::TypeAlias(alias)]
        );
        vm.heap.collect(&[owner.root()]);
        assert!(vm.heap.get(alias).is_ok());

        vm.alias_singletons.clear();
        assert!(vm.resolved_declarations.is_empty());
        assert!(vm.type_parent_defaults.is_empty());
        assert!(vm.executable_roots.is_empty());
        assert!(vm.class_singletons.is_empty());
        vm.declaration_code_roots(&mut roots);
        assert!(roots.is_empty());
        assert!(vm.heap.code_roots_for_test(owner).is_empty());
        vm.heap.collect(&[owner.root()]);
        assert!(vm.heap.executable_is_live(owner));
        assert!(vm.heap.get(alias).is_err());
    }

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
                    text: if key.key.uri == "memo:/broken.pkl" {
                        "p: Missing=1\ntag=0"
                    } else if key.key.uri == "memo:/rich.pkl" {
                        "import \"memo:/dependency.pkl\" as dependency\n@Deprecated { message=\"stage50\" }\nclass C { value: Int(this > 0) = dependency.tag }\ntypealias A<T> = T\ntypealias Checked = Int(this > 7)\nx: A<dependency.D> = new dependency.D {}\ny:C=new C{}\ntag=x.value+y.value-9"
                    } else if key.key.uri == "memo:/dependency.pkl" {
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
        settle_result_bounded(vm, external, raw_result, 100_000)
    }
    fn settle_result_bounded(
        vm: &mut Vm,
        external: &[Value],
        raw_result: bool,
        poll_limit: usize,
    ) -> Result<Value, RuntimeError> {
        for _ in 0..poll_limit {
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
    fn settle_reflection(vm: &mut Vm, external: &[Value], raw_result: bool) -> Value {
        // The pinned eager reflect bootstrap measured 237_534 instructions.
        // Only these reflection workloads get a 400_000-step bound; poll(1)
        // and collection after every instruction remain unchanged.
        settle_result_bounded(vm, external, raw_result, 400_000).unwrap()
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

    #[test]
    fn explicit_constructor_sites_share_only_neutral_canonical_parents_across_body_retry() {
        use crate::heap::CollectionKind;

        let source = r#"
            d0 = new Dynamic {}
            d1 = new Dynamic {}
            l0 = new Listing {}
            l1 = new Listing {}
            m0 = new Mapping {}
            m1 = new Mapping {}
            result = new Dynamic { for (value in List(read("mem:body"))) { value } }
        "#;
        let resolved =
            crate::resolver::resolve_module(&crate::parser::parse_module(source).unwrap()).unwrap();
        let program = compile_module(&resolved).unwrap();
        let mut vm = Vm::new(program.clone());
        vm.start(program.entry, Vec::new()).unwrap();
        let module = settle(&mut vm, &[], false);
        let roots = std::slice::from_ref(&module);
        let mut sites = BTreeSet::new();
        let mut parents = BTreeSet::new();
        for (index, kind, names) in [
            (0, CollectionKind::Dynamic, ["d0", "d1"]),
            (1, CollectionKind::Listing, ["l0", "l1"]),
            (2, CollectionKind::Mapping, ["m0", "m1"]),
        ] {
            let baseline = vm.collect(roots);
            let existing_parent = vm.empty_objects[index].filter(|id| vm.heap.get(*id).is_ok());
            if let Some(parent) = existing_parent {
                assert_eq!(kind, CollectionKind::Mapping);
                assert!(
                    vm.builtin_classes.values().any(|class| {
                        let prototype = vm.heap.get_class(*class).unwrap().prototype;
                        vm.heap
                            .get(prototype)
                            .unwrap()
                            .cache
                            .iter()
                            .any(|(key, state)| {
                                if key.name != "converters" {
                                    return false;
                                }
                                let LazyState::Completed(Value::Object(id)) = state else {
                                    return false;
                                };
                                let object = vm.heap.get(*id).unwrap();
                                object.parent == Some(parent)
                                    && object.collection_cast.as_ref().is_some_and(|cast| {
                                        cast.member == "#BaseValueRenderer.converters"
                                    })
                            })
                    }),
                    "existing canonical Mapping must be owned by the builtin renderer's converters"
                );
            }
            for name in names {
                vm.start_member(module.as_object().unwrap(), name).unwrap();
                let value = settle(&mut vm, roots, false);
                let id = value.as_object().unwrap();
                assert_eq!(vm.empty_objects[index], Some(id));
                if let Some(existing) = existing_parent {
                    assert_eq!(id, existing);
                }
                let parent = vm.heap.get(id).unwrap();
                assert_eq!(parent.collection_kind, kind);
                assert!(parent.members.is_empty());
                assert!(parent.environment.locals.is_empty());
                assert!(parent.environment.scopes.is_empty());
                assert!(parent.environment.custom_this.is_none());
                assert!(parent.environment.executable_owner.is_none());
                assert!(parent.environment.code_owner.is_none());
                assert!(parent.parent.is_some());
                let new_sites: Vec<_> = vm
                    .type_parent_defaults
                    .iter()
                    .filter(|(site, _)| !sites.contains(*site))
                    .map(|(site, parent)| (*site, parent.clone()))
                    .collect();
                assert_eq!(new_sites.len(), 1);
                let (site, cached) = &new_sites[0];
                assert!(matches!(
                    vm.program.blocks[site.0][site.1],
                    Instruction::GenericConstructorParent { cache: true, .. }
                ));
                assert_eq!(cached, &value);
                sites.insert(*site);
                parents.insert(id);
                let retained = vm.collect(roots);
                assert_eq!(
                    retained.live,
                    baseline.live + usize::from(existing_parent.is_none()),
                    "canonical parent allocation for {kind:?} at {name}"
                );
                assert_eq!(retained.executable_live, baseline.executable_live);
            }
        }
        assert_eq!(sites.len(), 6);
        assert_eq!(parents.len(), 3);
        let baseline = vm.collect(roots);
        let dynamic = vm.empty_objects[0].unwrap();
        let mut retry_site = None;
        for cancel in [false, true, false] {
            vm.start_member(module.as_object().unwrap(), "result")
                .unwrap();
            let request = loop {
                vm.collect(roots);
                match vm.poll(1) {
                    VmPoll::Yielded => {}
                    VmPoll::Pending(request) => break request,
                    other => panic!("expected constructor body read: {other:?}"),
                }
            };
            assert_eq!(request.uri, "mem:body");
            let cached: Vec<_> = vm
                .type_parent_defaults
                .iter()
                .filter(|(site, _)| !sites.contains(*site))
                .map(|(site, value)| (*site, value.clone()))
                .collect();
            assert_eq!(cached.len(), 1);
            assert_eq!(cached[0].1, Value::Object(dynamic));
            if let Some(site) = retry_site {
                assert_eq!(cached[0].0, site);
            } else {
                retry_site = Some(cached[0].0);
            }
            let builders: Vec<_> = vm
                .frames
                .iter()
                .flat_map(|f| &f.builders)
                .copied()
                .collect();
            assert_eq!(builders.len(), 1);
            assert_ne!(builders[0], dynamic);
            assert_eq!(vm.heap.get(builders[0]).unwrap().parent, Some(dynamic));
            if cancel {
                vm.cancel();
            } else {
                vm.resume(
                    request.id,
                    Err(RuntimeError::new(ErrorKind::Host, "retry constructor body")),
                )
                .unwrap();
            }
            let failure = settle_result(&mut vm, roots, false).unwrap_err();
            assert_eq!(
                failure.kind,
                if cancel {
                    ErrorKind::Cancelled
                } else {
                    ErrorKind::Host
                }
            );
            assert!(vm.heap.get(builders[0]).is_err());
            assert!(vm.heap.get(dynamic).is_ok());
            assert_eq!(vm.collect(roots).live, baseline.live);
        }
        vm.close();
        assert!(vm.type_parent_defaults.is_empty());
        assert_eq!(vm.heap_stats().slots, 0);
        for id in parents {
            assert!(vm.heap.get(id).is_err());
        }
    }

    #[test]
    fn explicit_constructor_parent_cache_retires_with_its_code_owner() {
        for (expression, parameterized) in [
            ("new Dynamic { value=7 }", false),
            ("new Listing { 7 }", false),
            ("new Mapping<String, Int> { [\"value\"]=7 }", true),
        ] {
            let mut vm = Vm::new(Arc::new(Program::new(Vec::new())));
            start(&mut vm, expression);
            let escaped = settle(&mut vm, &[], true);
            let site = *vm
                .type_parent_defaults
                .keys()
                .find(|(block, pc)| {
                    matches!(
                        vm.program.blocks[*block][*pc],
                        Instruction::GenericConstructorParent { cache: true, .. }
                    )
                })
                .expect("explicit constructor cache");
            let parent = vm.type_parent_defaults[&site].clone();
            let result = vm.heap.get(escaped.as_object().unwrap()).unwrap();
            let authored = if parameterized {
                // The explicit generic type check wraps the authored object.
                assert!(result.collection_cast.is_some());
                let cached = vm.heap.get(parent.as_object().unwrap()).unwrap();
                assert_ne!(Some(parent.as_object().unwrap()), vm.empty_objects[2]);
                assert_eq!(cached.parent, vm.empty_objects[2]);
                assert!(cached.collection_default.is_some());
                vm.heap.get(result.parent.unwrap()).unwrap()
            } else {
                assert!(result.collection_cast.is_none());
                result
            };
            assert_eq!(authored.parent, Some(parent.as_object().unwrap()));
            start(&mut vm, "stable");
            settle(&mut vm, std::slice::from_ref(&escaped), false);
            assert_eq!(vm.type_parent_defaults.get(&site), Some(&parent));
            start(&mut vm, "stable");
            settle(&mut vm, &[], false);
            assert!(!vm.type_parent_defaults.contains_key(&site));
            assert!(vm.program.blocks.get(site.0).is_none());
            // A parameterized Mapping has a query-owned specialized parent,
            // distinct from the empty Mapping held by the builtin renderer.
            vm.collect(&[]);
            assert!(vm.heap.get(parent.as_object().unwrap()).is_err());
            vm.close();
            assert!(vm.type_parent_defaults.is_empty());
            assert_eq!(vm.heap_stats().slots, 0);
        }
    }

    #[test]
    fn inferred_method_parent_cache_follows_query_code_reachability() {
        let mut vm = Vm::new(Arc::new(Program::new(Vec::new())));
        start(
            &mut vm,
            "(new Dynamic { local function make(): Dynamic = new { marker=7 }; result=make() }).result",
        );
        let escaped = settle(&mut vm, &[], true);
        let site = *vm
            .type_parent_defaults
            .keys()
            .find(|(block, pc)| {
                matches!(
                    vm.program.blocks[*block][*pc],
                    Instruction::ResolvedInferredParent { cache: true, .. }
                )
            })
            .expect("inferred parent cache");
        let parent = vm.type_parent_defaults[&site].clone();
        start(&mut vm, "stable");
        settle(&mut vm, std::slice::from_ref(&escaped), false);
        assert_eq!(vm.type_parent_defaults.get(&site), Some(&parent));
        assert!(vm.program.blocks.get(site.0).is_some());
        start(&mut vm, "stable");
        settle(&mut vm, &[], false);
        assert!(!vm.type_parent_defaults.contains_key(&site));
        assert!(vm.program.blocks.get(site.0).is_none());
        vm.close();
        assert!(vm.type_parent_defaults.is_empty());
        assert_eq!(vm.collect(&[]).slots, 0);
        let isolated = Vm::new(Arc::new(Program::new(Vec::new())));
        assert!(isolated.type_parent_defaults.is_empty());
    }

    #[test]
    fn inferred_method_parent_cache_is_published_before_a_failed_body() {
        let source =
            "function make(): Dynamic = new { value=read(\"mem:body\") }\nresult=make().value";
        let resolved =
            crate::resolver::resolve_module(&crate::parser::parse_module(source).unwrap()).unwrap();
        let program = compile_module(&resolved).unwrap();
        let mut vm = Vm::new(program.clone());
        vm.start(program.entry, Vec::new()).unwrap();
        let module = settle(&mut vm, &[], false);
        let mut first = None;
        for succeeds in [false, true] {
            vm.start_member(module.as_object().unwrap(), "result")
                .unwrap();
            let request = loop {
                vm.collect(std::slice::from_ref(&module));
                match vm.poll(1) {
                    VmPoll::Yielded => {}
                    VmPoll::Pending(request) => break request,
                    other => panic!("expected body read: {other:?}"),
                }
            };
            assert_eq!(request.uri, "mem:body");
            let cached = vm
                .type_parent_defaults
                .iter()
                .find(|((block, pc), _)| {
                    matches!(
                        vm.program.blocks[*block][*pc],
                        Instruction::ResolvedInferredParent { cache: true, .. }
                    )
                })
                .map(|(key, value)| (*key, value.clone()))
                .expect("parent acquired before body read");
            if let Some(first) = &first {
                assert_eq!(&cached, first);
            } else {
                first = Some(cached);
            }
            vm.resume(
                request.id,
                if succeeds {
                    Ok(Value::string("ready"))
                } else {
                    Err(RuntimeError::new(ErrorKind::Host, "failed body"))
                },
            )
            .unwrap();
            let value = settle_result(&mut vm, std::slice::from_ref(&module), false);
            if succeeds {
                assert_eq!(value, Ok(Value::string("ready")));
            } else {
                assert_eq!(value.unwrap_err().kind, ErrorKind::Host);
            }
        }
        vm.close();
        assert!(vm.type_parent_defaults.is_empty());
        assert_eq!(vm.collect(&[]).slots, 0);
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
        declaration_caches: [usize; 8],
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
            sources.insert(Arc::as_ptr(source), source.text.len_bytes());
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
                p.declaration_sources.len(),
                p.reflection_annotations.len(),
                p.constraint_origins.len(),
                vm.class_singletons.len(),
                vm.alias_singletons.len(),
                vm.resolved_declarations.len(),
                vm.type_parent_defaults.len(),
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
                        ModuleRequestKind::LoadSource { key, .. } if key.key.uri == holder => {
                            ModuleCompletion::Source(Ok(ModuleSource {
                                text: source.into(),
                                inferred_name: "ignored".into(),
                            }))
                        }
                        ModuleRequestKind::ResolveImportUri { key, authored_uri }
                            if key.uri == holder =>
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
                            let exists = element_uri == &PklString::from(expected("clause"))
                                || element_uri == &PklString::from(expected("expression"));
                            expression = exists && element_uri.ends_with("expression.pkl");
                            ModuleCompletion::HasElement(Ok(exists))
                        }
                        ModuleRequestKind::SelectKey {
                            phase: SelectionPhase::ClausePreparation,
                            uri,
                            ..
                        } => {
                            clause_selections += 1;
                            assert_eq!(uri, &PklString::from(expected("clause")));
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
                                                .is_some_and(|identity| identity.original_uri == holder))
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
                                    && spec.original_uri != expected("expression"))
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
                    .is_some_and(|identity| identity.original_uri == holder)
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
                spec.absolute_uri == expected(name) && spec.original_uri == expected(name)
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
            let declaration = vm.heap.declaration_object(&value).unwrap();
            owners.push(
                vm.heap
                    .get(declaration)
                    .unwrap()
                    .environment
                    .code_owner
                    .unwrap(),
            );
            ids.push(id);
            roots.push(value);
        }
        let alias = vm.heap.get(ids[1]).unwrap();
        let alias_environment = alias.environment.clone();
        let alias_body = vm.program.alias_definitions[&alias.type_alias_declaration.unwrap()]
            .body
            .clone();
        let old_class_identity = vm.heap.class_definition(ids[0]).unwrap().id;
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
            vm.heap.class_definition(new_class).unwrap().id,
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
        let original_declarations = program.declaration_sources.clone();
        let original_reflection_annotations = program.reflection_annotations.clone();
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
        assert_eq!(program.declaration_sources, original_declarations);
        assert_eq!(
            program.reflection_annotations,
            original_reflection_annotations
        );
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
                        .is_some_and(|source| source.text.original() == expression)),
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
                    .is_some_and(|source| source.text.original() == expression)
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
                .is_some_and(|source| source.text.original() == "p: Missing=1\ntag=0")
        }));
        assert!(!vm.program.units.iter().any(|unit| {
            unit.source
                .as_ref()
                .is_some_and(|source| source.uri == "memo:/broken.pkl")
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
                .is_some_and(|source| source.text.original() == "p: Missing=1\ntag=0")
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
    fn escaped_property_mirror_retains_uncached_declaration_code_until_its_last_root_is_gone() {
        let mut vm = Vm::new(Arc::new(Program::new(Vec::new())));
        start(
            &mut vm,
            "import(\"pkl:reflect\").Class(import(\"memo:/rich.pkl\").C).properties[\"value\"]",
        );
        let mirror = settle_reflection(&mut vm, &[], true);
        let id = mirror.as_object().unwrap();
        let owner = match &vm.heap.get(id).unwrap().mirror.as_ref().unwrap().data {
            reflection::Data::Member(declaration) => declaration.owner,
            _ => panic!("property mirror"),
        };
        let executable = vm.heap.get(owner).unwrap().environment.code_owner.unwrap();
        // Uncached keys bypass lookups but still publish both URI aliases,
        // matching ModuleCache.doLoad. This query drops only the query result;
        // the last loaded module remains an ordinary evaluator cache root.
        start(&mut vm, "stable");
        settle_reflection(&mut vm, std::slice::from_ref(&mirror), false);
        assert!(vm.heap.executable_is_live(executable));
        vm.start_member(id, "defaultValue").unwrap();
        assert_eq!(
            settle_reflection(&mut vm, std::slice::from_ref(&mirror), false),
            Value::Int(9.into())
        );
        // Check the legitimate module-cache roots before replacing them.
        // A fresh import site bypasses lookups for this uncached key.
        let lexical_module = vm
            .declaring_module(&vm.heap.get(owner).unwrap().environment, false)
            .unwrap();
        let original_alias = matches!(vm.modules_original.get("memo:/rich.pkl"), Some(modules::CachedModule::Value(module)) if *module == lexical_module);
        let resolved_alias = matches!(vm.modules_resolved.get("memo:/rich.pkl"), Some(modules::CachedModule::Value(module)) if *module == lexical_module);
        assert!(original_alias);
        assert!(resolved_alias);
        start(&mut vm, "import(\"memo:/rich.pkl\").tag");
        assert_eq!(
            settle_reflection(&mut vm, std::slice::from_ref(&mirror), false),
            Value::string("9")
        );
        assert!(vm.modules_original.values().chain(vm.modules_resolved.values()).all(|value| !matches!(value, modules::CachedModule::Value(module) if *module == lexical_module)));
        // With both aliases replaced, only the escaped mirror keeps its old
        // declaration, dependency and constraint code usable across collection.
        assert!(vm.heap.get(id).is_ok());
        assert!(vm.heap.executable_is_live(executable));
        start(&mut vm, "stable");
        settle_reflection(&mut vm, &[], false);
        assert!(vm.heap.get(id).is_err());
        assert!(!vm.heap.executable_is_live(executable));
        vm.close();
        assert_eq!(vm.heap.stats().slots, 0);
    }

    #[test]
    fn uncached_reflection_defaults_release_owner_resolution_caches_between_queries() {
        let mut vm = Vm::new(Arc::new(Program::new(Vec::new())));
        let mut baseline = None;
        for iteration in 1..=24 {
            start(
                &mut vm,
                "import(\"pkl:reflect\").Class(import(\"memo:/rich.pkl\").C).properties[\"value\"].defaultValue",
            );
            assert_eq!(settle_reflection(&mut vm, &[], false), Value::string("9"));
            // Retire the reflected declaration's two last-loaded URI aliases
            // using a fresh uncached load, rather than mistaking them for leaks.
            start(&mut vm, "import(\"memo:/rich.pkl\").tag");
            assert_eq!(settle_reflection(&mut vm, &[], false), Value::string("9"));
            start(&mut vm, "stable");
            assert_eq!(settle_reflection(&mut vm, &[], false), Value::string("7"));
            let current = resident_stats(&vm);
            if iteration == 4 {
                baseline = Some(current);
            } else if iteration > 4 {
                assert_eq!(Some(&current), baseline.as_ref(), "iteration {iteration}");
            }
        }
        vm.close();
        assert_eq!(vm.heap.stats().slots, 0);
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
