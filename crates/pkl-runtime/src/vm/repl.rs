//! Wasm-pure stateful live-heap REPL VM hooks mirroring Pkl 0.32.0 `ReplServer.java`.
//!
//! All REPL state lives directly on the evaluator's `Heap` and `Program` arena:
//! - Non-local property or method redefinition allocates a child `repl` module whose
//!   parent is the previous `repl` module, preserving late binding and inherited type checks.
//! - Lazy values are rendered non-forcingly via `diagnostic_values::render_multiline` and
//!   syntax-highlighted via `diagnostic::highlight::write_to`.
//! - `:force` runs `force_value_frame` (`allow_undefined = false, recurse = true`) cooperatively
//!   through `HostSession::poll`, mutating heap caches identically to `VmValue.force(v, false)`.

use super::*;
use crate::diagnostic::ansi::AnsiStringBuilder;
use crate::heap::CacheSlot;
use crate::modules::{ModuleIdentity, ModuleSourceSpec, UnitId};
use crate::source::SourceText;
use crate::value::RuntimeSource;

/// Synthetic expression preamble mirroring `ReplServer.java:66`.
pub(crate) const REPL_EXPR_PREAMBLE: &str = "`THE REPL TEXT EXPR` = ";

/// Compiled single REPL expression ready to execute via `HostSession::start`.
pub(crate) struct CompiledReplExpr {
    pub entry: usize,
    pub source: Arc<RuntimeSource>,
}

/// Compiled single REPL member declaration (`import`, `class`, `typealias`, `function`, property).
pub(crate) struct CompiledReplMember {
    pub init_entry: usize,
    pub eval_entry: Option<usize>,
    pub member: Member,
    pub source: Arc<RuntimeSource>,
}

type ReplCacheSnapshot = (
    crate::heap::FastMap<CacheKey, LazyState>,
    Vec<CacheSlot>,
    crate::economic_map::EconomicMapIndex,
);

impl Vm {
    /// Appends a one-time bootstrap block loading `pkl:base` so `Module` and builtin
    /// class prototypes exist on the heap before creating the initial `repl` module.
    pub(crate) fn repl_bootstrap_entry(&mut self) -> Result<usize, RuntimeError> {
        let program = Arc::make_mut(&mut self.program);
        program.seal_code_segment();
        let entry = program.alloc_block(
            vec![Instruction::LoadBaseModule, Instruction::Return],
            BlockMetadata::default(),
        )?;
        program.seal_code_segment();
        Ok(entry)
    }

    /// Appends a trivial `[Instruction::Return]` wrapper block used to host a cooperative
    /// `force_value_frame` continuation under `HostSession`.
    pub(crate) fn repl_return_entry(&mut self) -> Result<usize, RuntimeError> {
        let program = Arc::make_mut(&mut self.program);
        program.seal_code_segment();
        let entry = program.alloc_block(vec![Instruction::Return], BlockMetadata::default())?;
        program.seal_code_segment();
        Ok(entry)
    }

    /// Pushes a deep-forcing continuation (`allow_undefined = false, recurse = true`)
    /// onto the active wrapper frame started by `HostSession::start(return_entry, ...)`.
    pub(crate) fn repl_begin_force(&mut self, value: Value) -> Result<(), RuntimeError> {
        self.force_value_frame(value)
    }

    /// Appends a `LoadPrimary` entry for `:load <uri>`, resolving and loading the target
    /// module cooperatively through the Host boundary.
    pub(crate) fn repl_load_module_entry(&mut self, uri: PklString) -> Result<usize, RuntimeError> {
        crate::host::resources::validate_module_key_uri(&uri)?;
        let program = Arc::make_mut(&mut self.program);
        program.seal_code_segment();
        let spec = Arc::new(ModuleSourceSpec {
            uri,
            contents: None,
        });
        let entry = program.alloc_block(
            vec![Instruction::LoadPrimary(spec), Instruction::Return],
            BlockMetadata::default(),
        )?;
        program.seal_code_segment();
        self.public_operation_entries.insert(entry);
        Ok(entry)
    }

    /// Creates an empty `repl` module whose parent is `parent` (or `pkl:base#Module`
    /// prototype when `None`), mirroring `ReplServer.createEmptyReplModule`.
    pub(crate) fn repl_create_empty_module(
        &mut self,
        parent: Option<ObjectId>,
        working_dir_uri: &PklString,
    ) -> Result<ObjectId, RuntimeError> {
        let parent_obj = match parent {
            Some(id) => id,
            None => self.base_module_prototype()?,
        };
        self.alloc_repl_module_from(
            Some(parent_obj),
            crate::heap::MemberList::default(),
            crate::heap::ThinBTreeMap::default(),
            crate::heap::ThinBTreeMap::default(),
            crate::heap::ThinBTreeSet::default(),
            Environment::default(),
            None,
            working_dir_uri,
        )
    }

    /// Adopts a loaded module's declared properties, methods, members, and parent into
    /// a new `repl` module (`ReplServer.java:413-418`).
    pub(crate) fn repl_adopt_loaded_module(
        &mut self,
        loaded: ObjectId,
        working_dir_uri: &PklString,
    ) -> Result<ObjectId, RuntimeError> {
        let src = self.heap.get(loaded)?.clone();
        let parent_obj = match src.parent {
            Some(p) => Some(p),
            None => Some(self.base_module_prototype()?),
        };
        self.alloc_repl_module_from(
            parent_obj,
            src.members,
            src.member_annotations,
            src.member_deprecations,
            src.published_module_members,
            src.environment,
            None,
            working_dir_uri,
        )
    }

    /// Creates a non-mutating snapshot of `current` for `ReplRequest::Completion`
    /// (`ReplServer.java:437-444`) so speculative evaluation never mutates live state.
    pub(crate) fn repl_snapshot_module(
        &mut self,
        current: ObjectId,
        working_dir_uri: &PklString,
    ) -> Result<ObjectId, RuntimeError> {
        let src = self.heap.get(current)?.clone();
        let cache_snapshot = (src.cache, src.cache_slots, src.cache_index);
        self.alloc_repl_module_from(
            src.parent,
            src.members,
            src.member_annotations,
            src.member_deprecations,
            src.published_module_members,
            src.environment,
            Some(cache_snapshot),
            working_dir_uri,
        )
    }

    fn base_module_prototype(&mut self) -> Result<ObjectId, RuntimeError> {
        let class = self.canonical_builtin_class(classes::BuiltinClass::Ordinary(output::MODULE));
        Ok(self.heap.get_class(class)?.prototype)
    }

    #[allow(clippy::too_many_arguments)]
    fn alloc_repl_module_from(
        &mut self,
        parent: Option<ObjectId>,
        members: crate::heap::MemberList,
        member_annotations: crate::heap::ThinBTreeMap<usize, Vec<Value>>,
        member_deprecations: crate::heap::ThinBTreeMap<usize, PklString>,
        published_module_members: crate::heap::ThinBTreeSet<usize>,
        environment: Environment,
        cache_snapshot: Option<ReplCacheSnapshot>,
        working_dir_uri: &PklString,
    ) -> Result<ObjectId, RuntimeError> {
        let program = Arc::make_mut(&mut self.program);
        program.align_metadata()?;
        let unit_id = UnitId(
            program
                .units
                .next_id()
                .try_into()
                .map_err(|_| invalid("module unit limit"))?,
        );
        let class_id = program.next_declaration;
        program.next_declaration = class_id
            .checked_add(1)
            .ok_or_else(|| invalid("declaration identity limit"))?;
        let template = program.objects.try_push(Vec::new())?;
        let identity = ModuleIdentity {
            original_uri: working_dir_uri.clone(),
            resolved_uri: working_dir_uri.clone(),
        };
        let source = Arc::new(RuntimeSource {
            text: SourceText::from(""),
            uri: "repl:repl".into(),
            module_name: "repl".into(),
        });
        program.units.try_push(modules::ModuleUnit {
            id: unit_id,
            identity: Some(identity),
            standard_library: false,
            singleton_cache_key: None,
            key: None,
            delegate_key: None,
            build: None,
            syntax: None,
            source: Some(source),
            name: "repl".into(),
            template,
            entry: usize::MAX,
            blocks_start: 0,
            initial_members: Vec::new(),
            import_order: Vec::new(),
            class_id,
            mode: modules::ModuleMode::Plain,
            modifiers: crate::syntax::ModifierKind::Open.flag(),
        })?;

        let mut object = Object {
            is_module: true,
            module_unit: Some(unit_id),
            class_definition: Some(Box::new(Class {
                module: None,
                origin_uri: Some("repl:repl".into()),
                id: class_id,
                name: "repl".into(),
                open: true,
                abstract_: false,
            })),
            class_superclass_initialized: true,
            parent,
            members,
            member_annotations,
            member_deprecations,
            published_module_members,
            environment,
            ..Object::default()
        };
        if let Some((cache, cache_slots, cache_index)) = cache_snapshot {
            object.cache = cache;
            object.cache_slots = cache_slots;
            object.cache_index = cache_index;
        }
        let obj_id = self.heap.allocate(object);
        let class_obj = self.heap.allocate_class(obj_id);
        let superclass = match parent {
            Some(parent_id) => self.heap.object_class(parent_id)?,
            None => {
                Some(self.canonical_builtin_class(classes::BuiltinClass::Ordinary(output::MODULE)))
            }
        };
        self.heap.link_superclass(class_obj, superclass)?;
        // Restore exact `parent` object pointer in case `parent` was an amending module
        // whose own members live on the amendment rather than on `superclass.prototype`.
        self.heap.get_mut(obj_id)?.parent = parent;
        self.declaration_roots_dirty = true;
        Ok(obj_id)
    }

    /// Checks whether `module` directly declares a member with `(name, method)`.
    pub(crate) fn repl_module_has_member(
        &self,
        module: ObjectId,
        name: &PklString,
        method: bool,
    ) -> Result<bool, RuntimeError> {
        let obj = self.heap.get(module)?;
        Ok(obj
            .members
            .iter()
            .any(|m| !m.entry && m.method == method && &m.name == name))
    }

    /// Builds `local ... = Undefined()` stubs for all local members on `repl_module`,
    /// mirroring `ReplServer.buildSyntheticModuleText` (`ReplServer.java:219-226`).
    pub(crate) fn repl_build_local_stubs(
        &self,
        module: ObjectId,
        exclude: Option<(&PklString, bool)>,
    ) -> Result<String, RuntimeError> {
        let obj = self.heap.get(module)?;
        let mut out = String::new();
        let mut seen = BTreeSet::new();
        for member in &obj.members {
            if !member.local || member.entry {
                continue;
            }
            if exclude == Some((&member.name, member.method)) {
                continue;
            }
            if !seen.insert((member.name.clone(), member.method)) {
                continue;
            }
            let ident = format_repl_ident(&member.name.to_java_utf8());
            if member.method {
                let arity = self
                    .program
                    .metadata
                    .get(member.code)
                    .and_then(|md| md.method_signature.as_ref().map(|(_, a)| *a))
                    .unwrap_or(0);
                let params: Vec<String> = (0..arity).map(|i| format!("_p{i}")).collect();
                let const_kw = if member.constant { "const " } else { "" };
                out.push_str(&format!(
                    "local {const_kw}function {ident}({}) = Undefined()\n\n",
                    params.join(", ")
                ));
            } else if member.constant {
                out.push_str(&format!("local const {ident} = Undefined()\n\n"));
            } else {
                out.push_str(&format!("local {ident} = Undefined()\n\n"));
            }
        }
        Ok(out)
    }

    /// Compiles a single REPL expression item against `repl_module`'s local scope.
    pub(crate) fn repl_compile_expr(
        &mut self,
        repl_module: ObjectId,
        node_text: &str,
        request_uri: &PklString,
        working_dir_uri: &PklString,
    ) -> Result<CompiledReplExpr, RuntimeError> {
        let local_stubs = self.repl_build_local_stubs(repl_module, None)?;
        let mut synthetic = String::with_capacity(
            REPL_EXPR_PREAMBLE.len() + node_text.len() + 1 + local_stubs.len(),
        );
        synthetic.push_str(REPL_EXPR_PREAMBLE);
        synthetic.push_str(node_text);
        synthetic.push('\n');
        synthetic.push_str(&local_stubs);

        let source = Arc::new(RuntimeSource {
            text: SourceText::from(synthetic.as_str()),
            uri: request_uri.clone(),
            module_name: PklString::default(),
        });
        let identity = ModuleIdentity {
            original_uri: working_dir_uri.clone(),
            resolved_uri: working_dir_uri.clone(),
        };

        let parsed =
            crate::parser::parse_module_for_evaluation(synthetic.as_str()).map_err(|failure| {
                modules::imported_diagnostic(failure.diagnostic, source.clone(), "".into())
            })?;
        let resolved = crate::resolver::resolve_module_for_evaluation(&parsed);

        let program = Arc::make_mut(&mut self.program);
        let checkpoint = modules::ArenaCheckpoint::new(program);
        let res = compile_repl_expr_into(
            program,
            &resolved,
            source.clone(),
            identity,
            self.power_assertions,
        );
        match res {
            Ok(entry) => {
                program.seal_code_segment();
                Ok(CompiledReplExpr { entry, source })
            }
            Err(err) => {
                checkpoint.rollback(program);
                Err(err)
            }
        }
    }

    /// Compiles a single REPL member declaration (`import`, `class`, `typealias`, `function`,
    /// or property) against `repl_module`'s local scope.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn repl_compile_member(
        &mut self,
        repl_module: ObjectId,
        node_text: &str,
        member_name: &PklString,
        is_method: bool,
        is_property: bool,
        eval_definitions: bool,
        request_uri: &PklString,
        working_dir_uri: &PklString,
    ) -> Result<CompiledReplMember, RuntimeError> {
        let local_stubs =
            self.repl_build_local_stubs(repl_module, Some((member_name, is_method)))?;
        let mut synthetic = String::with_capacity(node_text.len() + 1 + local_stubs.len());
        synthetic.push_str(node_text);
        synthetic.push('\n');
        synthetic.push_str(&local_stubs);

        let source = Arc::new(RuntimeSource {
            text: SourceText::from(synthetic.as_str()),
            uri: request_uri.clone(),
            module_name: "repl".into(),
        });
        let identity = ModuleIdentity {
            original_uri: working_dir_uri.clone(),
            resolved_uri: working_dir_uri.clone(),
        };

        let parsed =
            crate::parser::parse_module_for_evaluation(synthetic.as_str()).map_err(|failure| {
                modules::imported_diagnostic(failure.diagnostic, source.clone(), "repl".into())
            })?;
        let resolved = crate::resolver::resolve_module_for_evaluation(&parsed);

        let program = Arc::make_mut(&mut self.program);
        let checkpoint = modules::ArenaCheckpoint::new(program);
        let res = compile_repl_member_into(
            program,
            &resolved,
            is_property && eval_definitions,
            source.clone(),
            identity,
            self.power_assertions,
        );
        match res {
            Ok(compiled) => {
                program.seal_code_segment();
                Ok(compiled)
            }
            Err(err) => {
                checkpoint.rollback(program);
                Err(err)
            }
        }
    }

    /// Publishes a compiled member onto `repl_module`, running override checks for
    /// non-local properties/methods and evicting stale cache entries on local/static replacement.
    pub(crate) fn repl_publish_member(
        &mut self,
        repl_module: ObjectId,
        member: Member,
        is_static_decl: bool,
    ) -> Result<(), RuntimeError> {
        if !is_static_decl
            && !member.local
            && let Some(parent) = self.heap.get(repl_module)?.parent
        {
            self.validate_class_override(parent, &member)?;
        }
        let obj = self.heap.get_mut(repl_module)?;
        if is_static_decl || member.local {
            obj.members
                .retain(|m| !(!m.entry && m.method == member.method && m.name == member.name));
            let key_local = CacheKey {
                entry: false,
                owner: Some(repl_module),
                name: member.name.clone(),
                method: member.method,
            };
            let key_nonlocal = CacheKey {
                entry: false,
                owner: None,
                name: member.name.clone(),
                method: member.method,
            };
            for key in [&key_local, &key_nonlocal] {
                obj.cache.remove(key);
                obj.cache_slots.retain(|s| &s.key != key);
            }
        }
        obj.published_module_members.insert(member.code);
        obj.members.push(member);
        self.declaration_roots_dirty = true;
        Ok(())
    }

    /// Renders a REPL result value non-forcingly (`VmValueRenderer.multiLine(Integer.MAX_VALUE)`)
    /// and applies Pkl 0.32.0 `SyntaxHighlighter` ANSI coloring when `use_color` is `true`.
    pub(crate) fn repl_render_value(
        &self,
        value: &Value,
        _module: ObjectId,
        use_color: bool,
    ) -> Result<String, RuntimeError> {
        let raw = crate::diagnostic_values::render_multiline(value, &self.heap, "repl")?;
        let mut sb = AnsiStringBuilder::new(use_color);
        crate::diagnostic::highlight::write_to(&mut sb, &raw);
        Ok(sb.finish_pkl().to_java_utf8())
    }

    /// Collects identifier property names and method signatures (`foo()` / `foo(`) for
    /// `ReplRequest::Completion`, mirroring `ReplServer.handleCompletion` + `collectMembers`.
    pub(crate) fn repl_collect_completions(
        &mut self,
        target: Option<&Value>,
        repl_module: ObjectId,
    ) -> Result<BTreeSet<String>, RuntimeError> {
        let mut members = BTreeSet::new();
        match target {
            None => {
                // Empty/whitespace expression: `BaseModule.getModule()` + `replState.module`.
                collect_base_module_top_level(&mut members);
                self.collect_object_members(&mut members, repl_module)?;
            }
            Some(value) => {
                let composite = match value {
                    Value::Object(id) => *id,
                    other => {
                        let cls = match classes::runtime_class(other, &self.heap)? {
                            classes::RuntimeClass::Declared(id) => id,
                            classes::RuntimeClass::Builtin(b) => self.canonical_builtin_class(b),
                        };
                        self.heap.get_class(cls)?.prototype
                    }
                };
                self.collect_object_members(&mut members, composite)?;
            }
        }
        Ok(members)
    }

    fn collect_object_members(
        &mut self,
        out: &mut BTreeSet<String>,
        composite: ObjectId,
    ) -> Result<(), RuntimeError> {
        // 1. Properties: walk `composite` and its `parent` chain (`VmObject.iterateMembers` skips local).
        let mut cur = Some(composite);
        let mut visited_objs = BTreeSet::new();
        while let Some(id) = cur {
            if !visited_objs.insert(id) {
                break;
            }
            let obj = self.heap.get(id)?;
            if obj.builtin_type.is_some() {
                if let Some(cls_def) = &obj.class_definition {
                    collect_builtin_class_members(
                        out,
                        cls_def.origin_uri.as_ref().map(|u| u.to_java_utf8()),
                        &cls_def.name.to_java_utf8(),
                        true,
                        false,
                    );
                }
            } else {
                for m in &obj.members {
                    if !m.entry && !m.local && !m.method {
                        out.insert(m.name.to_java_utf8());
                    }
                }
            }
            cur = obj.parent;
        }

        // Also include properties declared on the object's class superclass chain
        // (for builtin types such as Dynamic -> default, String -> length, Module -> output).
        let start_class = match self.heap.object_class(composite)? {
            Some(cls) => Some(cls),
            None => {
                let obj = self.heap.get(composite)?;
                let builtin = if obj.pair.is_some() {
                    Some(classes::BuiltinClass::Pair)
                } else if obj.reference.is_some() {
                    Some(classes::BuiltinClass::Reference)
                } else {
                    classes::BuiltinClass::from_type(&TypeCheck::Collection(obj.collection_kind))
                };
                builtin.map(|b| self.canonical_builtin_class(b))
            }
        };

        // 2. Methods (and builtin class properties): walk class -> superclass chain.
        let mut cur_cls = start_class;
        let mut visited_cls = BTreeSet::new();
        while let Some(cls_id) = cur_cls {
            if !visited_cls.insert(cls_id) {
                break;
            }
            let record = self.heap.get_class(cls_id)?.clone();
            let proto = self.heap.get(record.prototype)?;
            if proto.builtin_type.is_some() {
                if let Some(cls_def) = &proto.class_definition {
                    collect_builtin_class_members(
                        out,
                        cls_def.origin_uri.as_ref().map(|u| u.to_java_utf8()),
                        &cls_def.name.to_java_utf8(),
                        true,
                        true,
                    );
                }
            } else {
                for m in &proto.members {
                    if m.entry || m.local || !m.method {
                        continue;
                    }
                    let arity = self
                        .program
                        .metadata
                        .get(m.code)
                        .and_then(|md| md.method_signature.as_ref().map(|(_, a)| *a))
                        .unwrap_or(0);
                    let suffix = if arity == 0 { "()" } else { "(" };
                    out.insert(format!("{}{}", m.name.to_java_utf8(), suffix));
                }
            }
            cur_cls = record.superclass;
        }
        Ok(())
    }
}

fn compile_repl_expr_into(
    program: &mut Program,
    resolved: &crate::resolver::ResolvedModule,
    source: Arc<RuntimeSource>,
    identity: ModuleIdentity,
    instrument: bool,
) -> Result<usize, RuntimeError> {
    program.align_metadata()?;
    let unit = UnitId(
        program
            .units
            .next_id()
            .try_into()
            .map_err(|_| invalid("module unit limit"))?,
    );
    let declaration_base = program.next_declaration;
    program.next_declaration = declaration_base
        .checked_add(resolved.module.source.len_bytes() + 2)
        .ok_or_else(|| invalid("declaration identity limit"))?;

    let mut compiler = new_repl_compiler(
        resolved,
        program,
        unit,
        declaration_base,
        source.clone(),
        identity.clone(),
        PklString::default(),
        instrument,
    );

    let mut code = Code::default();
    code.push(Instruction::PrepareModuleImports(unit));
    let expr_span = if resolved.preparation_failure.is_none() {
        let member = resolved
            .module
            .members
            .first()
            .ok_or_else(|| invalid("missing synthesized REPL expression"))?;
        compiler.expression(&member.expr, &mut code)?;
        Some(member.expr.span)
    } else {
        None
    };
    code.push(Instruction::Return);

    let entry = compiler.block(code, expr_span)?;
    compiler.program.metadata[entry].module_receiver_input = Some(0);
    compiler.program.metadata[entry].member = PklString::default();
    compiler
        .program
        .constraint_origins
        .extend(compiler.constraint_origins);

    let template = compiler.program.objects.try_push(Vec::new())?;
    let class_id = declaration_base + resolved.module.source.len_bytes() + 1;
    compiler.program.units.try_push(modules::ModuleUnit {
        id: unit,
        identity: Some(identity),
        standard_library: false,
        singleton_cache_key: None,
        key: None,
        delegate_key: None,
        build: None,
        syntax: None,
        source: Some(source),
        name: PklString::default(),
        template,
        entry: usize::MAX,
        blocks_start: 0,
        initial_members: Vec::new(),
        import_order: uri_resolution::module_import_order(resolved),
        class_id,
        mode: modules::ModuleMode::Plain,
        modifiers: 0,
    })?;
    Ok(entry)
}

fn compile_repl_member_into(
    program: &mut Program,
    resolved: &crate::resolver::ResolvedModule,
    emit_eval_entry: bool,
    source: Arc<RuntimeSource>,
    identity: ModuleIdentity,
    instrument: bool,
) -> Result<CompiledReplMember, RuntimeError> {
    program.align_metadata()?;
    let unit = UnitId(
        program
            .units
            .next_id()
            .try_into()
            .map_err(|_| invalid("module unit limit"))?,
    );
    let declaration_base = program.next_declaration;
    program.next_declaration = declaration_base
        .checked_add(resolved.module.source.len_bytes() + 2)
        .ok_or_else(|| invalid("declaration identity limit"))?;

    let mut compiler = new_repl_compiler(
        resolved,
        program,
        unit,
        declaration_base,
        source.clone(),
        identity.clone(),
        PklString::default(),
        instrument,
    );

    if resolved.preparation_failure.is_some() {
        let mut init_code = Code::default();
        init_code.push(Instruction::PrepareModuleImports(unit));
        init_code.push(Instruction::Return);
        let init_entry = compiler.block(init_code, None)?;
        compiler.program.metadata[init_entry].module_receiver_input = Some(0);
        let template = compiler.program.objects.try_push(Vec::new())?;
        let class_id = declaration_base + resolved.module.source.len_bytes() + 1;
        compiler.program.units.try_push(modules::ModuleUnit {
            id: unit,
            identity: Some(identity),
            standard_library: false,
            singleton_cache_key: None,
            key: None,
            delegate_key: None,
            build: None,
            syntax: None,
            source: Some(source.clone()),
            name: "repl".into(),
            template,
            entry: usize::MAX,
            blocks_start: 0,
            initial_members: Vec::new(),
            import_order: uri_resolution::module_import_order(resolved),
            class_id,
            mode: modules::ModuleMode::Plain,
            modifiers: 0,
        })?;
        // Placeholder member (never published because init_entry fails on PrepareModuleImports).
        return Ok(CompiledReplMember {
            init_entry,
            eval_entry: None,
            member: Member {
                flags: 0,
                entry: false,
                name: PklString::default(),
                code: init_entry,
                local: false,
                method: false,
                ty: None,
                type_span: None,
                constant: false,
                fixed: false,
            },
            source,
        });
    }

    let target_member = resolved
        .module
        .members
        .first()
        .ok_or_else(|| invalid("missing synthesized REPL member"))?;
    let single = std::slice::from_ref(target_member);
    compiler.type_scopes.push(extract_type_scope(single));
    let template = compiler.object_template(single, false, false)?;
    let compiled = compiler.program.objects[template][0].clone();

    let mut init_code = Code::default();
    init_code.push(Instruction::Input(0));
    init_code.push(Instruction::PrepareModuleImports(unit));
    let is_class_or_alias = matches!(
        target_member.expr.kind,
        crate::syntax::ExprKind::Class { .. } | crate::syntax::ExprKind::TypeAlias { .. }
    );
    let annotated = !target_member.annotations.is_empty() && !is_class_or_alias;
    if annotated {
        compiler.member_annotations(target_member, &mut init_code, false)?;
    }
    if !is_class_or_alias {
        compiler.initialize_member_types(single, true, &mut init_code)?;
    }
    if annotated {
        if compiled.method {
            init_code.push(Instruction::InitializeDeprecation);
            init_code.push(Instruction::StoreDeprecation);
        }
        init_code.push(Instruction::PublishMemberAnnotations {
            code: compiled.code,
        });
    }
    init_code.push(Instruction::Return);
    compiler.type_scopes.pop();

    let init_entry = compiler.block(init_code, Some(target_member.span))?;
    compiler.program.metadata[init_entry].module_receiver_input = Some(0);

    let eval_entry = if emit_eval_entry {
        let mut eval_code = Code::default();
        eval_code.push(Instruction::GetProperty {
            name: compiled.name.clone(),
            owner_depth: 0,
            local: compiled.local,
        });
        eval_code.push(Instruction::Return);
        let entry = compiler.block(eval_code, Some(target_member.span))?;
        compiler.program.metadata[entry].module_receiver_input = Some(0);
        Some(entry)
    } else {
        None
    };

    compiler
        .program
        .constraint_origins
        .extend(compiler.constraint_origins);

    let class_id = declaration_base + resolved.module.source.len_bytes() + 1;
    compiler.program.units.try_push(modules::ModuleUnit {
        id: unit,
        identity: Some(identity),
        standard_library: false,
        singleton_cache_key: None,
        key: None,
        delegate_key: None,
        build: None,
        syntax: Some(Arc::clone(&resolved.module)),
        source: Some(source.clone()),
        name: "repl".into(),
        template,
        entry: usize::MAX,
        blocks_start: 0,
        initial_members: Vec::new(),
        import_order: uri_resolution::module_import_order(resolved),
        class_id,
        mode: modules::ModuleMode::Plain,
        modifiers: 0,
    })?;

    Ok(CompiledReplMember {
        init_entry,
        eval_entry,
        member: compiled,
        source,
    })
}

#[allow(clippy::too_many_arguments)]
fn new_repl_compiler<'a>(
    resolved: &'a crate::resolver::ResolvedModule,
    program: &'a mut Program,
    unit: UnitId,
    declaration_base: usize,
    source: Arc<RuntimeSource>,
    identity: ModuleIdentity,
    member: PklString,
    instrument: bool,
) -> Compiler<'a> {
    Compiler {
        resolved,
        type_lookup: build_type_lookup(resolved),
        program,
        unit,
        declaration_base,
        source: Some(source),
        identity: Some(identity),
        options: CompileOptions {
            fuse_constant_add: true,
        },
        member,
        super_method_allowed: false,
        module_annotation: false,
        let_members: Vec::new(),
        type_scopes: Vec::new(),
        runtime_type_owner: true,
        runtime_type_fallback: None,
        modifiers: Vec::new(),
        constraint_module: false,
        alias_frames: Vec::new(),
        constraint_origins: BTreeMap::new(),
        active_constraints: BTreeMap::new(),
        shared_constraint_roots: BTreeMap::new(),
        active_type_parameters: BTreeMap::new(),
        constructor_parameters_bound: false,
        parameter_spans: BTreeMap::new(),
        parameter_label_spans: BTreeMap::new(),
        argument_spans: BTreeMap::new(),
        function_counts: BTreeMap::new(),
        entry_counts: BTreeMap::new(),
        function_names: BTreeMap::new(),
        declared_recipes: BTreeMap::new(),
        module_recipes: BTreeMap::new(),
        instrument,
    }
}

fn format_repl_ident(name: &str) -> String {
    if crate::parser::is_regular_identifier(name) {
        name.to_string()
    } else {
        format!("`{name}`")
    }
}

fn collect_base_module_top_level(out: &mut BTreeSet<String>) {
    let base = crate::stdlib_sources::base();
    let Ok(parsed) = crate::parser::parse_module(base.text.entire()) else {
        return;
    };
    for member in &parsed.members {
        if member.local {
            continue;
        }
        match &member.expr.kind {
            crate::syntax::ExprKind::Function {
                parameters,
                method: true,
                ..
            } => {
                let suffix = if parameters.is_empty() { "()" } else { "(" };
                out.insert(format!("{}{}", member.name.to_java_utf8(), suffix));
            }
            crate::syntax::ExprKind::Import { .. } => {}
            _ => {
                out.insert(member.name.to_java_utf8());
            }
        }
    }
}

fn collect_builtin_class_members(
    out: &mut BTreeSet<String>,
    origin_uri: Option<String>,
    class_name: &str,
    include_properties: bool,
    include_methods: bool,
) {
    let uri = origin_uri.as_deref().unwrap_or("pkl:base");
    let source = match uri {
        "pkl:base" => crate::stdlib_sources::base(),
        "pkl:reflect" => crate::stdlib_sources::reflect(),
        "pkl:test" => crate::stdlib_sources::test(),
        "pkl:json" => crate::stdlib_sources::json(),
        "pkl:yaml" => crate::stdlib_sources::yaml(),
        "pkl:xml" => crate::stdlib_sources::xml(),
        "pkl:jsonnet" => crate::stdlib_sources::jsonnet(),
        "pkl:protobuf" => crate::stdlib_sources::protobuf(),
        "pkl:platform" => crate::stdlib_sources::platform(),
        "pkl:release" => crate::stdlib_sources::release(),
        "pkl:semver" => crate::stdlib_sources::semver(),
        "pkl:settings" => crate::stdlib_sources::settings(),
        "pkl:shell" => crate::stdlib_sources::shell(),
        "pkl:math" => crate::stdlib_sources::math(),
        "pkl:benchmark" => crate::stdlib_sources::benchmark(),
        "pkl:analyze" => crate::stdlib_sources::analyze(),
        "pkl:Command" => crate::stdlib_sources::command(),
        "pkl:Project" => crate::stdlib_sources::project(),
        "pkl:EvaluatorSettings" => crate::stdlib_sources::evaluator_settings(),
        "pkl:PklBinary" => crate::stdlib_sources::pklbinary(),
        "pkl:ref" => crate::stdlib_sources::reference(),
        "pkl:DocPackageInfo" => crate::stdlib_sources::doc_package_info(),
        "pkl:DocsiteInfo" => crate::stdlib_sources::docsite_info(),
        _ => crate::stdlib_sources::base(),
    };
    let Ok(parsed) = crate::parser::parse_module(source.text.entire()) else {
        return;
    };
    let Some(class_member) = parsed.members.iter().find(|m| {
        m.name.to_java_utf8() == class_name
            && matches!(m.expr.kind, crate::syntax::ExprKind::Class { .. })
    }) else {
        return;
    };
    let crate::syntax::ExprKind::Class { members, .. } = &class_member.expr.kind else {
        return;
    };
    for m in members {
        if m.local {
            continue;
        }
        match &m.expr.kind {
            crate::syntax::ExprKind::Function {
                parameters,
                method: true,
                ..
            } => {
                if include_methods {
                    let suffix = if parameters.is_empty() { "()" } else { "(" };
                    out.insert(format!("{}{}", m.name.to_java_utf8(), suffix));
                }
            }
            crate::syntax::ExprKind::Class { .. }
            | crate::syntax::ExprKind::TypeAlias { .. }
            | crate::syntax::ExprKind::Import { .. } => {}
            _ => {
                if include_properties {
                    out.insert(m.name.to_java_utf8());
                }
            }
        }
    }
}
