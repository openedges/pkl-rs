//! Pre-bootstrapped `pkl:base` VM snapshot for zero-overhead fresh evaluators.
//!
//! Compiles and executes `pkl:base` once per process in an isolated `Vm`, runs a
//! GC pass to compact the live object graph and pre-compute edge caches, and
//! freezes all `CodeStorage` prefixes into shared `Arc` slices. Each fresh
//! `SourceSession::module_evaluator` clones the ~250-slot heap with a fresh
//! `heap_id` (~25 µs) instead of re-compiling and re-bootstrapping `pkl:base`
//! (~1,350 µs).

use super::*;
use std::sync::OnceLock;

pub(super) struct BaseSnapshot {
    program: Arc<Program>,
    heap: crate::heap::FrozenBaseHeap,
    modules_stdlib: modules::ModuleCache,
    modules_original: modules::ModuleCache,
    modules_resolved: modules::ModuleCache,
    class_singletons: BTreeMap<usize, ObjectId>,
    builtin_classes: BTreeMap<classes::BuiltinClass, ObjectId>,
    alias_singletons: BTreeMap<usize, ObjectId>,
    empty_collections: [Option<ObjectId>; 3],
    empty_objects: [Option<ObjectId>; 3],
    resolved_declarations: BTreeMap<(usize, ObjectId), type_resolution::DeclarationValue>,
    type_parent_defaults: BTreeMap<(usize, usize), Value>,
    executable_roots: ExecutableRoots,
    unowned_declaration_roots: Vec<Value>,
    declaration_roots_dirty: bool,
    declaration_roots_lens: (usize, usize, usize, usize, usize, usize),
}

fn build_base_snapshot() -> BaseSnapshot {
    let mut program = (*Program::session_shared()).clone();
    program.blocks[0] = vec![Instruction::LoadBaseModule, Instruction::Return];
    let mut vm = Vm::with_context(Arc::new(program), EvaluationContext::default());
    vm.start(0, Vec::new())
        .expect("pkl:base snapshot start succeeds");
    loop {
        match vm.poll(1_000_000) {
            VmPoll::Ready(Ok(_)) => break,
            VmPoll::Yielded => continue,
            other => panic!("pkl:base snapshot bootstrap unexpected poll: {other:?}"),
        }
    }
    vm.collect(&[]);
    debug_assert!(vm.code_arenas.is_empty());
    debug_assert!(vm.public_operation_entries.is_empty());
    debug_assert!(vm.query_cache.is_empty());
    let program_mut = Arc::make_mut(&mut vm.program);
    program_mut.blocks[0] = vec![Instruction::Return];
    program_mut.blocks.freeze_prepared_prefix();
    program_mut.metadata.freeze_prepared_prefix();
    program_mut.objects.freeze_prepared_prefix();
    program_mut.type_recipes.freeze_prepared_prefix();
    program_mut.units.freeze_prepared_prefix();
    program_mut.builtin_block_range = 0..program_mut.blocks.next_id();
    program_mut.base_declaration_sources = Some(Arc::new(std::mem::take(
        &mut program_mut.declaration_sources,
    )));
    program_mut.base_reflection_annotations = Some(Arc::new(std::mem::take(
        &mut program_mut.reflection_annotations,
    )));
    program_mut.base_alias_definitions =
        Some(Arc::new(std::mem::take(&mut program_mut.alias_definitions)));
    program_mut.module_name = PklString::default();
    program_mut.module_template = None;
    program_mut.entry = 0;
    program_mut.initial_module_members.clear();
    vm.executable_roots.freeze_base();
    BaseSnapshot {
        program: vm.program,
        heap: vm.heap.freeze_base(),
        modules_stdlib: vm.modules_stdlib,
        modules_original: vm.modules_original,
        modules_resolved: vm.modules_resolved,
        class_singletons: vm.class_singletons,
        builtin_classes: vm.builtin_classes,
        alias_singletons: vm.alias_singletons,
        empty_collections: vm.empty_collections,
        empty_objects: vm.empty_objects,
        resolved_declarations: vm.resolved_declarations,
        type_parent_defaults: vm.type_parent_defaults,
        executable_roots: vm.executable_roots,
        unowned_declaration_roots: vm.unowned_declaration_roots,
        declaration_roots_dirty: vm.declaration_roots_dirty,
        declaration_roots_lens: vm.declaration_roots_lens,
    }
}

pub(super) fn base_snapshot() -> &'static BaseSnapshot {
    static SNAPSHOT: OnceLock<BaseSnapshot> = OnceLock::new();
    SNAPSHOT.get_or_init(build_base_snapshot)
}

impl Vm {
    /// Construct a fresh evaluator VM sharing the frozen `pkl:base` heap prefix
    /// via copy-on-write, with a fresh heap ID for all evaluator-local objects.
    pub(crate) fn from_base_snapshot(context: EvaluationContext) -> Self {
        let snap = base_snapshot();
        let heap = Heap::from_frozen_base(&snap.heap);
        let platform_metadata = context.platform_metadata.clone();
        Self {
            program: snap.program.clone(),
            context,
            callback_closing: false,
            log_buffer: Vec::new(),
            heap,
            frames: Vec::new(),
            frame_pool: Vec::new(),
            stack: identity::OperandStack::default(),
            inputs: Vec::new(),
            state: State::Idle,
            suspended: Vec::new(),
            next_request: 1,
            instructions: 0,
            member_evaluations: 0,
            code_validation: Ok(()),
            power_assertions: false,
            pure_standard_host: false,
            inline_module_depth: 0,
            active_type_trackers: 0,
            fact_tracker: None,
            public_primary: None,
            modules_original: snap.modules_original.clone(),
            modules_stdlib: snap.modules_stdlib.clone(),
            modules_resolved: snap.modules_resolved.clone(),
            resources: BTreeMap::new(),
            glob_list_threshold: 16384,
            class_singletons: snap.class_singletons.clone(),
            builtin_classes: snap.builtin_classes.clone(),
            alias_singletons: snap.alias_singletons.clone(),
            empty_collections: snap.empty_collections,
            empty_objects: snap.empty_objects,
            resolved_declarations: snap.resolved_declarations.clone(),
            type_parent_defaults: snap.type_parent_defaults.clone(),
            executable_roots: snap.executable_roots.clone(),
            code_arenas: Vec::new(),
            declaration_roots_dirty: snap.declaration_roots_dirty,
            declaration_roots_lens: snap.declaration_roots_lens,
            unowned_declaration_roots: snap.unowned_declaration_roots.clone(),
            public_operation_entries: BTreeSet::new(),
            query_cache: Vec::new(),
            auto_gc: false,
            gc_threshold: 8192,
            last_gc_arenas: 0,
            gc_epoch: 1,
            session_root: None,
            uncached_module_uris: std::collections::VecDeque::new(),
            cached_module_uris: BTreeSet::new(),
            last_warm_module: None,
            fast_int_cache: FastIntCache::default(),
            fast_int_captured: Vec::new(),
            fast_int_vals: Vec::new(),
            fast_int_locals: Vec::new(),
            fast_int_calls: Vec::new(),
            scratch_utf8: String::with_capacity(256),
            platform_metadata,
            monotonic_clock: None,
        }
    }
}
