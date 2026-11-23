//! Declaration ModuleKey URI preparation. Triple-dot read resolution precedes
//! every ResourceManager lookup; import preparation precedes module evaluation.
use super::*;
use crate::modules::{
    ModuleCapabilities, ModuleHostError, ModuleIdentity, ModuleRequestKind, UnitId,
};
use std::collections::VecDeque;

#[derive(Clone)]
struct Import {
    spec: modules::ImportSpec,
    origin: RuntimeFrame,
    clause: bool,
    glob: bool,
    node_span: Span,
    prepared_uri: Option<PklString>,
}
#[derive(Clone, Debug)]
pub(super) enum PreparationStep {
    Import {
        clause: bool,
        glob: bool,
        node_span: Span,
        uri: PklString,
        span: Span,
        member: PklString,
    },
    Failure(crate::diagnostic::Diagnostic),
}

enum Preparation {
    Import(Import),
    Failure(RuntimeError),
}

enum Target {
    Read(ReadRequest),
    Import {
        current: Import,
        remaining: VecDeque<Preparation>,
    },
}
pub(super) struct Resolution {
    module: ModuleIdentity,
    path: PklString,
    candidates: VecDeque<PklString>,
    selected: Option<crate::modules::ModuleKey>,
    declaration_key: Option<crate::modules::ModuleKey>,
    wrapper_fragment_paths: bool,
    target: Target,
}
impl Resolution {
    fn failure(&self, mut error: RuntimeError) -> RuntimeError {
        if let Target::Import { current, .. } = &self.target {
            // AstBuilder.resolveImport reattaches its authored operand even
            // when IoUtils constructed the original exception without a node.
            error.unavailable_origin = false;
            error.insert_frame(0, current.origin.clone());
            error.span = Some(current.spec.span);
        }
        error
    }
    fn host_failure(&self, failure: ModuleHostError) -> RuntimeError {
        let error = match &self.target {
            Target::Import { current, .. } => failure.into_runtime(&current.spec.authored),
            Target::Read(request) => match failure {
                ModuleHostError::NotFound => crate::host::resources::missing(&request.uri),
                ModuleHostError::Io(cause) => {
                    let mut error = RuntimeError::host_language(
                        PklString::from("I/O error reading resource `")
                            .concat(&request.uri)
                            .concat(&"`.".into()),
                    );
                    error.hint = cause.message;
                    error
                }
                other => other.into_runtime(&request.uri),
            },
        };
        self.failure(error)
    }
}
impl Program {
    pub(super) fn update_import_uris<'a>(
        &mut self,
        unit: UnitId,
        prepared: impl Fn(Span) -> Option<&'a (PklString, PklString)>,
    ) {
        // Preparation can update earlier shell/body instructions. Find those
        // logical IDs read-only before requesting any segment's mutable COW
        // view; unrelated prepared code and other owners stay shared.
        let start = self
            .units
            .get(unit.0 as usize)
            .map_or(0, |info| info.blocks_start);
        let blocks: Vec<_> = self
            .blocks
            .iter_with_ids_from(start)
            .filter_map(|(id, code)| {
                if self
                    .metadata
                    .get(id)
                    .is_some_and(|meta| meta.unit.is_some() && meta.unit != Some(unit))
                {
                    return None;
                }
                code.iter()
                    .any(|instruction| {
                        let spec = match instruction {
                            Instruction::ImportModule(spec) => Some(spec),
                            Instruction::ImportGlob(glob) => glob.import.as_ref(),
                            _ => None,
                        };
                        spec.is_some_and(|spec| {
                            spec.unit == unit
                                && prepared(spec.span).is_some_and(|(absolute, original)| {
                                    spec.absolute_uri != *absolute || spec.original_uri != *original
                                })
                        })
                    })
                    .then_some(id)
            })
            .collect();
        for id in blocks {
            for instruction in self.blocks.get_mut(id).expect("selected import block") {
                let spec = match instruction {
                    Instruction::ImportModule(spec) => Some(spec),
                    Instruction::ImportGlob(glob) => glob.import.as_mut(),
                    _ => None,
                };
                if let Some(spec) = spec
                    && spec.unit == unit
                    && let Some((absolute, original)) = prepared(spec.span)
                {
                    spec.absolute_uri = absolute.clone();
                    spec.original_uri = original.clone();
                }
            }
        }
    }
}
impl Vm {
    pub(super) fn begin_read_resolution(
        &mut self,
        request: ReadRequest,
    ) -> Result<(), RuntimeError> {
        let Some(path) = crate::host::resources::read_triple_path(&request.uri)? else {
            return self.publish_read(request);
        };
        let uri = request.module_uri.clone().ok_or_else(|| {
            RuntimeError::new(
                ErrorKind::Unsupported,
                "Triple-dot reads require a declaration ModuleKey.",
            )
        })?;
        let module = self
            .frames
            .last()
            .and_then(|frame| self.program.metadata.get(frame.block))
            .and_then(|metadata| metadata.unit)
            .and_then(|id| self.program.units.get(id.0 as usize))
            .and_then(|unit| unit.identity.as_ref())
            .filter(|identity| identity.original_uri == uri)
            .cloned()
            .unwrap_or(ModuleIdentity {
                original_uri: uri.clone(),
                resolved_uri: uri,
            });
        self.start_uri_resolution(Resolution {
            module,
            path,
            candidates: VecDeque::new(),
            selected: None,
            declaration_key: None,
            wrapper_fragment_paths: false,
            target: Target::Read(request),
        })
    }

    pub(super) fn publish_read(&mut self, request: ReadRequest) -> Result<(), RuntimeError> {
        if let Ok(uri) = crate::host::resources::resolved_uri(&request)
            && let Some(value) = self.resources.get(&crate::host::resources::cache_key(&uri))
        {
            self.stack.push(match value {
                Some(value) => value.clone(),
                None if request.nullable => Value::Null,
                None => return Err(crate::host::resources::missing(&request.uri)),
            });
            self.state = State::Running;
        } else {
            self.state = State::Waiting(PendingRequest::Read(request));
        }
        Ok(())
    }

    pub(super) fn prepare_module_imports(&mut self, unit: UnitId) -> Result<(), RuntimeError> {
        if self.program.units[unit.0 as usize].import_order.is_empty() {
            return Ok(());
        }
        let declaration = &self.program.units[unit.0 as usize];
        let mut remaining = VecDeque::new();

        for step in &declaration.import_order {
            match step {
                PreparationStep::Import {
                    uri,
                    span,
                    member,
                    clause,
                    glob,
                    node_span,
                } => {
                    let mut origin = RuntimeFrame {
                        anonymous: false,
                        source: declaration
                            .source
                            .clone()
                            .or_else(|| self.context.primary_source.clone()),
                        span: *span,
                        member: if member.is_empty() {
                            PklString::default()
                        } else {
                            PklString::from("#").concat(member)
                        },
                    };
                    if let Some(source) = &mut origin.source
                        && source.uri == "repl:text"
                        && source.module_name.is_empty()
                    {
                        // AstBuilder.resolveImport reports its lexical ModuleInfo
                        // scope, not the expression SimpleRootNode's empty name.
                        // Keep this provenance local to the preparation frame.
                        let lexical_module = source.uri.clone();
                        Arc::make_mut(source).module_name = lexical_module;
                        origin.member = if member.is_empty()
                            || matches!(member.encode_utf16().next(), Some(46 | 91))
                        {
                            member.clone()
                        } else {
                            PklString::from(".").concat(member)
                        };
                    }
                    if let Some(caller) = self.frames.len().checked_sub(2).map(|i| &self.frames[i])
                        && caller
                            .module_load
                            .as_ref()
                            .is_some_and(|run| run.initializing)
                    {
                        let metadata = &self.program.metadata[caller.block];
                        if let Some(source) = &mut origin.source {
                            Arc::make_mut(source).module_name = metadata
                                .source
                                .as_ref()
                                .map_or(&self.program.module_name, |s| &s.module_name)
                                .into();
                        }
                        origin.member = caller
                            .diagnostic_member
                            .clone()
                            .unwrap_or_else(|| metadata.member.clone());
                    }
                    remaining.push_back(Preparation::Import(Import {
                        spec: modules::ImportSpec {
                            authored: uri.clone(),
                            span: *span,
                            unit,
                            absolute_uri: uri.clone(),
                            original_uri: uri.clone(),
                        },
                        origin,
                        clause: *clause,
                        glob: *glob,
                        node_span: *node_span,
                        prepared_uri: None,
                    }));
                }
                PreparationStep::Failure(diagnostic) => {
                    let source = declaration
                        .source
                        .clone()
                        .or_else(|| self.context.primary_source.clone())
                        .ok_or_else(|| invalid("AST diagnostic without source"))?;
                    let declared_member = || {
                        PklString::from(
                            diagnostic
                                .module_name
                                .as_deref()
                                .unwrap_or(&source.module_name),
                        )
                        .concat(&diagnostic.member.as_ref().map_or_else(
                            PklString::default,
                            |suffix| {
                                if suffix.encode_utf16().next() == Some(46) {
                                    // Explicit AnnotationScope root boundary.
                                    suffix.as_ref().clone()
                                } else if source.uri == "repl:text" && source.module_name.is_empty()
                                {
                                    // Expression AST scopes descend from
                                    // the synthetic repl root. Real module
                                    // declarations retain their # boundary.
                                    if suffix.is_empty()
                                        || matches!(suffix.encode_utf16().next(), Some(46 | 91))
                                    {
                                        suffix.as_ref().clone()
                                    } else {
                                        PklString::from(".").concat(suffix)
                                    }
                                } else {
                                    PklString::from("#").concat(suffix)
                                }
                            },
                        ))
                    };
                    let initializing_caller = self.frames.len().checked_sub(2).and_then(|index| {
                        let caller = &self.frames[index];
                        caller
                            .module_load
                            .as_ref()
                            .filter(|run| run.initializing)
                            .map(|run| {
                                let metadata = &self.program.metadata[caller.block];
                                let member = PklString::from(
                                    metadata
                                        .source
                                        .as_ref()
                                        .map_or(&self.program.module_name, |source| {
                                            &source.module_name
                                        }),
                                )
                                .concat(
                                    caller
                                        .diagnostic_member
                                        .as_ref()
                                        .unwrap_or(&metadata.member),
                                );
                                (member, run.is_import_initializing())
                            })
                    });
                    let member = match initializing_caller {
                        // AstBuilder has not entered the child's call target.
                        // StackTraceGenerator keeps the exception's child
                        // source section but names the active importing root.
                        Some((member, true)) => member,
                        _ if diagnostic.member.is_some() || diagnostic.module_name.is_some() => {
                            declared_member()
                        }
                        Some((member, false)) => member,
                        None => declared_member(),
                    };
                    remaining.push_back(Preparation::Failure(modules::imported_diagnostic(
                        diagnostic.clone(),
                        source,
                        member,
                    )));
                }
            }
        }
        self.next_import_preparation(declaration.identity.clone(), remaining)
    }

    fn next_import_preparation(
        &mut self,
        module: Option<ModuleIdentity>,
        mut remaining: VecDeque<Preparation>,
    ) -> Result<(), RuntimeError> {
        while let Some(step) = remaining.pop_front() {
            match step {
                Preparation::Failure(error) => return Err(error),
                Preparation::Import(mut current) => {
                    // AstBuilder.doVisitImport rejects glob triple dots before
                    // resolveImport or module factory selection, even unforced.
                    if current.glob && current.spec.authored.starts_with("...") {
                        let mut error = RuntimeError::new(
                            ErrorKind::Type,
                            "Cannot combine glob imports with triple-dot module URIs.",
                        );
                        let mut origin = current.origin.clone();
                        origin.span = current.node_span;
                        error.span = Some(current.node_span);
                        error.insert_frame(0, origin);
                        return Err(error);
                    }
                    // AstBuilder creates the operand URI before calling the
                    // declaration key's resolver, including for custom keys.
                    let (parsed_authored, _) =
                        crate::host::resources::prepare_module_uri(&current.spec.authored, None)
                            .map_err(|mut error| {
                                error.insert_frame(0, current.origin.clone());
                                error.span = Some(current.spec.span);
                                error
                            })?;
                    let is_dep_notation = parsed_authored.starts_with("@");
                    let unit_key = self.program.units[current.spec.unit.0 as usize].key.clone();
                    let key_to_resolve = if let Some(key) = unit_key {
                        if matches!(
                            key.kind,
                            crate::modules::ModuleKeyKind::Host
                                | crate::modules::ModuleKeyKind::StandardFile
                                | crate::modules::ModuleKeyKind::StandardHttp
                                | crate::modules::ModuleKeyKind::StandardGenericUrl
                        ) {
                            Some(key)
                        } else {
                            None
                        }
                    } else if is_dep_notation {
                        let uri = module
                            .as_ref()
                            .map(|m| m.original_uri.clone())
                            .unwrap_or_default();
                        let kind =
                            if uri.starts_with("package:") || uri.starts_with("projectpackage:") {
                                crate::modules::ModuleKeyKind::StandardGenericUrl
                            } else {
                                crate::modules::ModuleKeyKind::StandardFile
                            };
                        Some(crate::modules::ModuleKey {
                            id: crate::modules::ModuleKeyId(0),
                            uri,
                            kind,
                            handle: PklString::default(),
                        })
                    } else {
                        None
                    };
                    if current.prepared_uri.is_none()
                        && crate::host::resources::import_triple_path(&parsed_authored)?.is_none()
                        && let Some(key) = key_to_resolve
                    {
                        let key = key.clone();
                        let authored_uri = parsed_authored.clone();
                        self.frames.last_mut().unwrap().uri_resolution =
                            Some(Box::new(Resolution {
                                module: module
                                    .clone()
                                    .ok_or_else(|| invalid("missing declaration identity"))?,
                                path: PklString::default(),
                                candidates: VecDeque::new(),
                                selected: None,
                                declaration_key: None,
                                wrapper_fragment_paths: false,
                                target: Target::Import { current, remaining },
                            }));
                        return self.wait_module(ModuleRequestKind::ResolveImportUri {
                            key,
                            authored_uri,
                        });
                    }
                    let (absolute, original) = crate::host::resources::prepare_module_uri(
                        current
                            .prepared_uri
                            .as_ref()
                            .unwrap_or(&current.spec.authored),
                        if current.prepared_uri.is_some() {
                            None
                        } else {
                            module.as_ref().map(|m| &m.original_uri)
                        },
                    )
                    .map_err(|mut error| {
                        error.insert_frame(0, current.origin.clone());
                        error.span = Some(current.spec.span);
                        error
                    })?;
                    current.spec.absolute_uri = absolute.clone();
                    current.spec.original_uri = original.clone();
                    let Some(path) = crate::host::resources::import_triple_path(&parsed_authored)?
                    else {
                        let prepared = (absolute, original);
                        Arc::make_mut(&mut self.program)
                            .update_import_uris(current.spec.unit, |span| {
                                (span == current.spec.span).then_some(&prepared)
                            });
                        if current.clause
                            && self.program.units[current.spec.unit.0 as usize]
                                .build
                                .is_some()
                        {
                            return self.start_clause_selection(module, current, remaining);
                        }
                        continue;
                    };
                    let module = module.ok_or_else(|| {
                        RuntimeError::new(
                            ErrorKind::Unsupported,
                            "Triple-dot imports require a declaration ModuleKey.",
                        )
                    })?;
                    return self.start_uri_resolution(Resolution {
                        module,
                        path,
                        candidates: VecDeque::new(),
                        selected: None,
                        declaration_key: None,
                        wrapper_fragment_paths: false,
                        target: Target::Import { current, remaining },
                    });
                }
            }
        }
        self.finish_module_build()?;
        self.state = State::Running;
        Ok(())
    }

    pub(super) fn resolved_import_uri(
        &mut self,
        result: Result<PklString, ModuleHostError>,
    ) -> Result<(), RuntimeError> {
        let run = self
            .frames
            .last_mut()
            .and_then(|frame| frame.uri_resolution.take())
            .ok_or_else(|| invalid("missing declaration URI continuation"))?;
        let uri = result.map_err(|failure| run.host_failure(failure))?;
        let Target::Import {
            mut current,
            mut remaining,
        } = run.target
        else {
            return Err(invalid("wrong declaration URI target"));
        };
        current.prepared_uri = Some(uri);
        remaining.push_front(Preparation::Import(current));
        self.next_import_preparation(Some(run.module), remaining)
    }
    fn start_clause_selection(
        &mut self,
        module: Option<ModuleIdentity>,
        current: Import,
        remaining: VecDeque<Preparation>,
    ) -> Result<(), RuntimeError> {
        let uri = current.spec.original_uri.clone();
        let module =
            module.ok_or_else(|| invalid("import clause requires a declaration ModuleKey"))?;
        self.frames.last_mut().unwrap().uri_resolution = Some(Box::new(Resolution {
            module,
            path: PklString::default(),
            candidates: VecDeque::new(),
            selected: None,
            declaration_key: None,
            wrapper_fragment_paths: false,
            target: Target::Import { current, remaining },
        }));
        self.wait_module(ModuleRequestKind::SelectKey {
            id: crate::modules::ModuleKeyId(modules::next_key_id()?),
            uri,
            phase: crate::modules::SelectionPhase::ClausePreparation,
        })
    }
    pub(super) fn selected_clause_key(
        &mut self,
        result: Result<crate::modules::ModuleKey, ModuleHostError>,
    ) -> Result<(), RuntimeError> {
        let mut run = self
            .frames
            .last_mut()
            .and_then(|frame| frame.uri_resolution.take())
            .ok_or_else(|| invalid("missing clause key continuation"))?;
        let key = result.map_err(|failure| match &run.target {
            Target::Import { current, .. } => {
                // ModuleResolver.resolve(uri) has no optional ImportNode here.
                failure.into_factory_runtime(&current.spec.absolute_uri)
            }
            _ => invalid("clause key completion without a clause"),
        })?;
        let Target::Import { current, .. } = &run.target else {
            return Err(invalid("missing import clause"));
        };
        let unit = &self.program.units[current.spec.unit.0 as usize];
        let alias = unit
            .build
            .as_ref()
            .and_then(|build| {
                build
                    .module
                    .members
                    .iter()
                    .find_map(|member| match &member.expr.kind {
                        crate::syntax::ExprKind::Import {
                            uri_span,
                            clause_alias: Some(alias),
                            ..
                        } if *uri_span == current.spec.span => Some(alias.clone()),
                        _ => None,
                    })
            })
            .ok_or_else(|| invalid("missing import clause metadata"))?;
        run.selected = Some(key.clone());
        if let Some(alias) = alias {
            return self.finish_clause_name(run, alias);
        }
        if !crate::host::resources::module_name_needs_fragment_paths(&key.uri) {
            let name = crate::host::resources::infer_module_name(&key.uri, false)?;
            return self.finish_clause_name(run, name);
        }
        self.frames.last_mut().unwrap().uri_resolution = Some(run);
        self.wait_module(ModuleRequestKind::QueryFragmentPaths { key })
    }
    pub(super) fn resolved_name_fragment_paths(
        &mut self,
        result: Result<bool, ModuleHostError>,
    ) -> Result<(), RuntimeError> {
        let run = self
            .frames
            .last_mut()
            .and_then(|frame| frame.uri_resolution.take())
            .ok_or_else(|| invalid("missing name inference continuation"))?;
        let fragment_paths = result.map_err(|failure| run.host_failure(failure))?;
        let key = run
            .selected
            .as_ref()
            .ok_or_else(|| invalid("missing inferred-name key"))?;
        let name = crate::host::resources::infer_module_name(&key.uri, fragment_paths)?;
        self.finish_clause_name(run, name)
    }
    fn finish_clause_name(
        &mut self,
        run: Box<Resolution>,
        name: PklString,
    ) -> Result<(), RuntimeError> {
        let Target::Import { current, remaining } = run.target else {
            return Err(invalid("missing name inference clause"));
        };
        let unit = &mut Arc::make_mut(&mut self.program).units[current.spec.unit.0 as usize];
        let build = unit
            .build
            .as_mut()
            .ok_or_else(|| invalid("missing module build continuation"))?;
        let member_idx = build
            .module
            .members
            .iter()
            .position(|member| {
                matches!(&member.expr.kind,
                    crate::syntax::ExprKind::Import { uri_span, clause_alias: Some(_), .. } if *uri_span == current.spec.span)
            })
            .ok_or_else(|| invalid("missing import member"))?;
        if build.module.members[member_idx].name != name {
            Arc::make_mut(&mut build.module).members[member_idx].name = name;
            build.name_changed = true;
            build.resolved = None;
        }
        build.prepared.insert(
            current.spec.span,
            (
                current.spec.absolute_uri.clone(),
                current.spec.original_uri.clone(),
            ),
        );
        self.next_import_preparation(Some(run.module), remaining)
    }
    fn finish_module_build(&mut self) -> Result<(), RuntimeError> {
        let Some(unit) = self
            .frames
            .last()
            .and_then(|frame| self.program.metadata.get(frame.block))
            .and_then(|m| m.unit)
        else {
            return Ok(());
        };
        let declaration = self.program.units[unit.0 as usize].clone();
        let Some(build) = declaration.build else {
            return Ok(());
        };
        let resolved_owned;
        let resolved: &crate::resolver::ResolvedModule = if !build.name_changed
            && let Some(cached) = build.resolved.as_ref()
        {
            cached
        } else {
            resolved_owned =
                crate::resolver::resolve_module_for_evaluation_arc(Arc::clone(&build.module));
            &resolved_owned
        };
        let cached_syntax = Some(Arc::clone(&resolved.module));
        let code_owner = self
            .frames
            .last()
            .and_then(|frame| frame.environment.code_owner);
        let pure_standard_host = self.pure_standard_host;
        let program = Arc::make_mut(&mut self.program);
        program.align_metadata()?;
        let blocks_start = program.blocks.next_id();
        let objects_start = program.objects.next_id();
        let recipes_start = program.type_recipes.next_id();
        let units_start = program.units.next_id();
        let declaration_base = program.next_declaration;
        let empty_uri = PklString::default();
        let (has_identity, original_uri, resolved_uri) = match &declaration.identity {
            Some(id) => (true, &id.original_uri, &id.resolved_uri),
            None => (false, &empty_uri, &empty_uri),
        };
        let is_base_module =
            has_identity && (original_uri == "pkl:base" || resolved_uri == "pkl:base");
        let is_primary_root = !is_base_module && unit == UnitId(0);
        let prepared_list: Vec<(Span, (PklString, PklString))> = build
            .prepared
            .iter()
            .map(|(span, uris)| (*span, uris.clone()))
            .collect();
        let cacheable_source = if !build.name_changed
            && resolved.preparation_failure.is_none()
            && let Some(src) = declaration.source.as_ref()
            && src.text.len_bytes() <= 256 * 1024
            && !program
                .type_recipes
                .iter_with_ids()
                .any(|(_, r)| r.unit == unit)
        {
            Some((src, modules::source_text_hash(&src.text)))
        } else {
            None
        };
        let cache_key = cacheable_source.map(|(src, text_hash)| modules::CompiledUnitCacheKey {
            hash: text_hash,
            text: &src.text,
            source_uri: &src.uri,
            module_name: &src.module_name,
            has_identity,
            original_uri,
            resolved_uri,
            power_assertions: build.instrument,
            fuse_constant_add: build.options.fuse_constant_add,
            pure_standard_host,
            is_primary_root,
            is_replacement: true,
            prepared: &prepared_list,
            blocks_start,
            objects_start,
            recipes_start,
            unit_id: unit.0,
            declaration_base,
            base_mixin_declaration: program.base_mixin_declaration,
        });
        let arena = code_owner.map(|_| query_lifecycle::CodeArena::begin(program));
        if let Some(key) = &cache_key
            && let Some(mut unit_clone) = modules::try_replay_compiled_unit(program, key)?
        {
            unit_clone.key = declaration.key;
            unit_clone.delegate_key = declaration.delegate_key;
            unit_clone.standard_library = declaration.standard_library;
            let entry = unit_clone.entry;
            program.units[unit.0 as usize] = unit_clone;
            if let (Some(arena), Some(owner)) = (arena, code_owner) {
                self.code_arenas.push(arena.finish(program, owner, false));
                self.declaration_roots_dirty = true;
            }
            let frame = self.frames.last_mut().unwrap();
            frame.block = entry;
            frame.pc = if matches!(
                program.blocks[entry].first(),
                Some(Instruction::LoadBaseModule)
            ) {
                2
            } else {
                0
            };
            return Ok(());
        }
        let checkpoint = modules::ArenaCheckpoint::new(program);
        program.blocks.seal();
        program.metadata.seal();
        program.objects.seal();
        program.type_recipes.seal();
        let before_decl_len = program.declaration_sources.len();
        let before_refl_len = program.reflection_annotations.len();
        let before_alias_len = program.alias_definitions.len();
        let before_constr_len = program.constraint_origins.len();
        let before_base_mixin = program.base_mixin_declaration;
        let result = if resolved.preparation_failure.is_some() {
            let mut shell = resolved.clone();
            let shell_mod = Arc::make_mut(&mut shell.module);
            shell_mod.members.clear();
            shell_mod.deferred_diagnostics.clear();
            if let Some(decl) = &mut shell_mod.declaration {
                decl.annotations.clear();
                decl.parent = None;
                decl.modifiers.clear();
            }
            shell.preparation.clear();
            shell.preparation_failure = None;
            compile_unit_body_at_with_syntax(
                Some(unit),
                program,
                &shell,
                None,
                true,
                build.options,
                build.instrument,
                declaration.source.clone(),
                declaration.identity.clone(),
            )
            .inspect(|_| {
                program.units[unit.0 as usize].import_order =
                    module_import_order_filtered(resolved, true)
            })
        } else {
            compile_unit_body_at_with_syntax(
                Some(unit),
                program,
                resolved,
                cached_syntax,
                true,
                build.options,
                build.instrument,
                declaration.source.clone(),
                declaration.identity.clone(),
            )
        };
        if let Err(error) = result {
            checkpoint.rollback(program);
            return Err(error);
        }
        program.update_import_uris(unit, |span| build.prepared.get(&span));
        if let Some(key) = &cache_key
            && program.units[unit.0 as usize].build.is_none()
            && program.units.next_id() == units_start
            && program.base_mixin_declaration == before_base_mixin
        {
            modules::store_compiled_unit(
                program,
                key,
                before_decl_len,
                before_refl_len,
                before_alias_len,
                before_constr_len,
                program.units[unit.0 as usize].clone(),
            );
        }
        if let (Some(arena), Some(owner)) = (arena, code_owner) {
            self.code_arenas.push(arena.finish(program, owner, false));
            self.declaration_roots_dirty = true;
        }
        program.units[unit.0 as usize].key = declaration.key;
        program.units[unit.0 as usize].delegate_key = declaration.delegate_key;
        program.units[unit.0 as usize].standard_library = declaration.standard_library;
        let entry = program.units[unit.0 as usize].entry;
        let frame = self.frames.last_mut().unwrap();
        frame.block = entry;
        frame.pc = if matches!(
            program.blocks[entry].first(),
            Some(Instruction::LoadBaseModule)
        ) {
            2
        } else {
            0
        };
        Ok(())
    }

    fn start_uri_resolution(&mut self, mut run: Resolution) -> Result<(), RuntimeError> {
        let unit = match &run.target {
            Target::Import { current, .. } => Some(current.spec.unit),
            Target::Read(_) => self
                .frames
                .last()
                .and_then(|frame| self.program.metadata.get(frame.block))
                .and_then(|metadata| metadata.unit),
        };
        if let Some(unit) = unit.and_then(|id| self.program.units.get(id.0 as usize)) {
            run.wrapper_fragment_paths = unit.delegate_key.is_some();
            run.declaration_key = unit.delegate_key.clone().or_else(|| unit.key.clone());
        }
        let request = ModuleRequestKind::Capabilities {
            key: run.declaration_key.clone(),
            module: run.module.clone(),
        };
        self.frames
            .last_mut()
            .expect("URI preparation frame")
            .uri_resolution = Some(Box::new(run));
        self.wait_module(request)
    }

    pub(super) fn resolved_capabilities(
        &mut self,
        result: Result<ModuleCapabilities, ModuleHostError>,
    ) -> Result<(), RuntimeError> {
        let mut run = self
            .frames
            .last_mut()
            .and_then(|frame| frame.uri_resolution.take())
            .ok_or_else(|| invalid("missing URI capabilities continuation"))?;
        let mut caps = result.map_err(|error| run.host_failure(error))?;
        // CachedModuleKey delegates locality/hierarchy and hasElement, but
        // inherits ModuleKey.hasFragmentPaths=false from its own class.
        if run.wrapper_fragment_paths {
            caps.fragment_paths = false;
        }
        run.candidates =
            crate::host::resources::triple_candidates(&run.module.original_uri, &run.path, caps)
                .map_err(|error| run.failure(error))?
                .into();
        self.next_uri_candidate(run)
    }
    fn next_uri_candidate(&mut self, mut run: Box<Resolution>) -> Result<(), RuntimeError> {
        let Some(uri) = run.candidates.pop_front() else {
            return Err(run.host_failure(ModuleHostError::NotFound));
        };
        let request = ModuleRequestKind::HasElement {
            key: run.declaration_key.clone(),
            module: run.module.clone(),
            element_uri: uri,
        };
        self.frames.last_mut().unwrap().uri_resolution = Some(run);
        self.wait_module(request)
    }
    pub(super) fn resolved_element(
        &mut self,
        result: Result<bool, ModuleHostError>,
    ) -> Result<(), RuntimeError> {
        let State::Waiting(PendingRequest::Module(request)) = &self.state else {
            return Err(invalid("missing element request"));
        };
        let ModuleRequestKind::HasElement { element_uri, .. } = &request.kind else {
            return Err(invalid("wrong URI preparation effect"));
        };
        let uri = element_uri.clone();
        let run = self
            .frames
            .last_mut()
            .and_then(|frame| frame.uri_resolution.take())
            .ok_or_else(|| invalid("missing element continuation"))?;
        if !result.map_err(|error| run.host_failure(error))? {
            return self.next_uri_candidate(run);
        }
        match run.target {
            Target::Read(mut request) => {
                request.resolved_uri = Some(uri);
                self.publish_read(request)
            }
            Target::Import { current, remaining } => {
                let (absolute, original) = crate::host::resources::prepare_module_uri(&uri, None)?;
                let prepared = (absolute.clone(), original.clone());
                Arc::make_mut(&mut self.program).update_import_uris(current.spec.unit, |span| {
                    (span == current.spec.span).then_some(&prepared)
                });
                if current.clause
                    && self.program.units[current.spec.unit.0 as usize]
                        .build
                        .is_some()
                {
                    let mut current = current;
                    current.spec.absolute_uri = absolute;
                    current.spec.original_uri = original;
                    self.start_clause_selection(Some(run.module), current, remaining)
                } else {
                    self.next_import_preparation(Some(run.module), remaining)
                }
            }
        }
    }
}

/// The resolver records construction effects at the actual validation phase.
pub(super) fn module_import_order(
    resolved: &crate::resolver::ResolvedModule,
) -> Vec<PreparationStep> {
    module_import_order_filtered(resolved, false)
}

pub(super) fn module_import_order_filtered(
    resolved: &crate::resolver::ResolvedModule,
    skip_import_clauses: bool,
) -> Vec<PreparationStep> {
    let mut steps = resolved
        .preparation
        .iter()
        .filter(|step| {
            !skip_import_clauses
                || !matches!(
                    step,
                    crate::resolver::AstPreparation::Import { clause: true, .. }
                )
        })
        .map(|step| match step {
            crate::resolver::AstPreparation::Import {
                uri,
                span,
                member,
                clause,
                glob,
                node_span,
            } => PreparationStep::Import {
                clause: *clause,
                glob: *glob,
                node_span: *node_span,
                uri: uri.clone(),
                span: *span,
                member: member.clone(),
            },
        })
        .collect::<Vec<_>>();
    if let Some(error) = &resolved.preparation_failure {
        steps.push(PreparationStep::Failure(error.clone()));
    }
    steps
}

impl Vm {
    pub(super) fn module_relative_path_to(&mut self) -> Result<(), RuntimeError> {
        let other = self.pop()?.as_object()?;
        let receiver = self.conversion_receiver()?;
        let error = |message: PklString| {
            let mut error = RuntimeError::new(ErrorKind::Type, message);
            // External method bodies have an unavailable source section.
            error.unavailable_origin = true;
            error
        };
        if self.heap.get(receiver)?.module_unit.is_none() {
            return Err(error(
                "Expected a module as the receiver, but got an object that amends a module.".into(),
            ));
        }
        if self.heap.get(other)?.module_unit.is_none() {
            return Err(error(
                "Expected a module as argument, but got an object that amends a module.".into(),
            ));
        }
        let key_uri = |id| -> Result<PklString, RuntimeError> {
            let unit = self.heap.get(id)?.module_unit.expect("actual module owner");
            let unit = self
                .program
                .units
                .get(unit.0 as usize)
                .ok_or_else(|| invalid("module path unit"))?;
            unit.key
                .as_ref()
                .map(|key| key.uri.clone())
                .or_else(|| {
                    unit.identity
                        .as_ref()
                        .map(|identity| identity.original_uri.clone())
                })
                .ok_or_else(|| {
                    invalid("Module.relativePathTo requires original ModuleKey URI metadata")
                })
        };
        let receiver_uri = key_uri(receiver)?;
        let other_uri = key_uri(other)?;
        let Some(elements) =
            crate::host::resources::module_relative_path_elements(&receiver_uri, &other_uri)?
        else {
            return Err(error(
                PklString::from("No descendent path exists between modules `")
                    .concat(&receiver_uri)
                    .concat(&"` and `".into())
                    .concat(&other_uri)
                    .concat(&"`.".into()),
            ));
        };
        if elements.is_empty() {
            let empty = self.empty_strict_collection(crate::heap::CollectionKind::List);
            self.stack.push(Value::Object(empty));
            return Ok(());
        }
        // VmList.create(elements, elements.length - 1) makes a fresh List when
        // the split array is nonempty, even when the resulting List is empty.
        let builder = self.heap.allocate(Object {
            collection_kind: crate::heap::CollectionKind::List,
            ..Default::default()
        });
        self.frames
            .last_mut()
            .expect("module path frame")
            .builders
            .push(builder);
        for element in elements.iter().take(elements.len() - 1) {
            self.stack.push(Value::string(element.clone()));
            self.append_strict_element()?;
        }
        self.frames
            .last_mut()
            .expect("module path frame")
            .builders
            .pop();
        self.stack.push(Value::Object(builder));
        Ok(())
    }
}
