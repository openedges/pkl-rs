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
    prepared_uri: Option<Arc<str>>,
}
#[derive(Clone, Debug)]
pub(super) enum PreparationStep {
    Import {
        clause: bool,
        uri: String,
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
    path: String,
    candidates: VecDeque<String>,
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
            error.frames.insert(0, current.origin.clone());
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
                    let mut error = RuntimeError::host_language(format!(
                        "I/O error reading resource `{}`.",
                        request.uri
                    ));
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
    fn update_import_uris<'a>(
        &mut self,
        unit: UnitId,
        prepared: impl Fn(Span) -> Option<&'a (Arc<str>, Arc<str>)>,
    ) {
        // Preparation can update earlier shell/body instructions. Find those
        // logical IDs read-only before requesting any segment's mutable COW
        // view; unrelated prepared code and other owners stay shared.
        let blocks: Vec<_> = self
            .blocks
            .iter_with_ids()
            .filter_map(|(id, code)| {
                code.iter()
                    .any(|instruction| {
                        matches!(instruction, Instruction::ImportModule(spec)
                            if spec.unit == unit
                                && prepared(spec.span).is_some_and(|(absolute, original)|
                                    spec.absolute_uri != *absolute || spec.original_uri != *original))
                    })
                    .then_some(id)
            })
            .collect();
        for id in blocks {
            for instruction in self.blocks.get_mut(id).expect("selected import block") {
                if let Instruction::ImportModule(spec) = instruction
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

    fn publish_read(&mut self, request: ReadRequest) -> Result<(), RuntimeError> {
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
                } => {
                    let mut origin = RuntimeFrame {
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
                                .map_or(self.program.module_name.as_ref(), |s| &s.module_name)
                                .into();
                        }
                        origin.member = caller
                            .diagnostic_member
                            .clone()
                            .unwrap_or_else(|| metadata.member.clone());
                    }
                    remaining.push_back(Preparation::Import(Import {
                        spec: modules::ImportSpec {
                            authored: uri.as_str().into(),
                            span: *span,
                            unit,
                            absolute_uri: uri.as_str().into(),
                            original_uri: uri.as_str().into(),
                        },
                        origin,
                        clause: *clause,
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
                        .concat(
                            &diagnostic
                                .member
                                .as_ref()
                                .map_or_else(PklString::default, |suffix| {
                                    PklString::from("#").concat(suffix)
                                }),
                        )
                    };
                    let member =
                        if diagnostic.member.is_some() || diagnostic.module_name.is_some() {
                            // Delayed AST validation already knows its lexical
                            // declaration owner; the module initializer is only
                            // executing preparation on that declaration's behalf.
                            declared_member()
                        } else {
                            self.frames
                                .len()
                                .checked_sub(2)
                                .and_then(|index| {
                                    let caller = &self.frames[index];
                                    caller
                                        .module_load
                                        .as_ref()
                                        .filter(|run| run.initializing)
                                        .map(|_| {
                                            let metadata = &self.program.metadata[caller.block];
                                            PklString::from(metadata.source.as_ref().map_or(
                                                self.program.module_name.as_ref(),
                                                |source| &source.module_name,
                                            ))
                                            .concat(
                                                caller
                                                    .diagnostic_member
                                                    .as_ref()
                                                    .unwrap_or(&metadata.member),
                                            )
                                        })
                                })
                                .unwrap_or_else(declared_member)
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
                    // AstBuilder creates the operand URI before calling the
                    // declaration key's resolver, including for custom keys.
                    let (parsed_authored, _) =
                        crate::host::resources::prepare_module_uri(&current.spec.authored, None)
                            .map_err(|mut error| {
                                error.frames.insert(0, current.origin.clone());
                                error.span = Some(current.spec.span);
                                error
                            })?;
                    if current.prepared_uri.is_none()
                        && crate::host::resources::import_triple_path(&parsed_authored)?.is_none()
                        && let Some(key) = self.program.units[current.spec.unit.0 as usize]
                            .key
                            .as_ref()
                        && key.kind == crate::modules::ModuleKeyKind::Host
                    {
                        let key = key.clone();
                        let authored_uri = parsed_authored.clone().into();
                        self.frames.last_mut().unwrap().uri_resolution = Some(Resolution {
                            module: module
                                .clone()
                                .ok_or_else(|| invalid("missing declaration identity"))?,
                            path: String::new(),
                            candidates: VecDeque::new(),
                            selected: None,
                            declaration_key: None,
                            wrapper_fragment_paths: false,
                            target: Target::Import { current, remaining },
                        });
                        return self.wait_module(ModuleRequestKind::ResolveImportUri {
                            key,
                            authored_uri,
                        });
                    }
                    let (absolute, original) = crate::host::resources::prepare_module_uri(
                        current
                            .prepared_uri
                            .as_deref()
                            .unwrap_or(&current.spec.authored),
                        if current.prepared_uri.is_some() {
                            None
                        } else {
                            module.as_ref().map(|m| m.original_uri.as_ref())
                        },
                    )
                    .map_err(|mut error| {
                        error.frames.insert(0, current.origin.clone());
                        error.span = Some(current.spec.span);
                        error
                    })?;
                    current.spec.absolute_uri = absolute.clone().into();
                    current.spec.original_uri = original.clone().into();
                    let Some(path) = crate::host::resources::import_triple_path(&parsed_authored)?
                    else {
                        let prepared = (absolute.into(), original.into());
                        Arc::make_mut(&mut self.program)
                            .update_import_uris(current.spec.unit, |span| {
                                (span == current.spec.span).then_some(&prepared)
                            });
                        if current.clause {
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
        result: Result<Arc<str>, ModuleHostError>,
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
        self.frames.last_mut().unwrap().uri_resolution = Some(Resolution {
            module,
            path: String::new(),
            candidates: VecDeque::new(),
            selected: None,
            declaration_key: None,
            wrapper_fragment_paths: false,
            target: Target::Import { current, remaining },
        });
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
    fn finish_clause_name(&mut self, run: Resolution, name: String) -> Result<(), RuntimeError> {
        let Target::Import { current, remaining } = run.target else {
            return Err(invalid("missing name inference clause"));
        };
        let unit = &mut Arc::make_mut(&mut self.program).units[current.spec.unit.0 as usize];
        let build = unit
            .build
            .as_mut()
            .ok_or_else(|| invalid("missing module build continuation"))?;
        let member = build.module.members.iter_mut().find(|member| matches!(&member.expr.kind,
            crate::syntax::ExprKind::Import { uri_span, clause_alias: Some(_), .. } if *uri_span == current.spec.span))
            .ok_or_else(|| invalid("missing import member"))?;
        member.name = name;
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
        let mut resolved = crate::resolver::resolve_module_for_evaluation(&build.module);
        resolved.preparation.retain(|step| {
            !matches!(
                step,
                crate::resolver::AstPreparation::Import { clause: true, .. }
            )
        });
        let code_owner = self
            .frames
            .last()
            .and_then(|frame| frame.environment.code_owner);
        let program = Arc::make_mut(&mut self.program);
        let checkpoint = modules::ArenaCheckpoint::new(program);
        let arena = code_owner.map(|_| query_lifecycle::CodeArena::begin(program));
        let result = if resolved.preparation_failure.is_some() {
            let mut shell = resolved.clone();
            shell.module.members.clear();
            shell.module.deferred_diagnostics.clear();
            if let Some(decl) = &mut shell.module.declaration {
                decl.annotations.clear();
                decl.parent = None;
                decl.modifiers.clear();
            }
            shell.preparation.clear();
            shell.preparation_failure = None;
            compile_unit_body_at(
                Some(unit),
                program,
                &shell,
                build.options,
                build.instrument,
                declaration.source.clone(),
                declaration.identity.clone(),
            )
            .inspect(|_| {
                program.units[unit.0 as usize].import_order = module_import_order(&resolved)
            })
        } else {
            compile_unit_body_at(
                Some(unit),
                program,
                &resolved,
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
        if let (Some(arena), Some(owner)) = (arena, code_owner) {
            self.code_arenas.push(arena.finish(program, owner, false));
        }
        program.update_import_uris(unit, |span| build.prepared.get(&span));
        program.units[unit.0 as usize].key = declaration.key;
        program.units[unit.0 as usize].delegate_key = declaration.delegate_key;
        program.units[unit.0 as usize].standard_library = declaration.standard_library;
        let entry = program.units[unit.0 as usize].entry;
        let frame = self.frames.last_mut().unwrap();
        frame.block = entry;
        frame.pc = if unit.0 == 0 { 2 } else { 0 };
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
            .uri_resolution = Some(run);
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
    fn next_uri_candidate(&mut self, mut run: Resolution) -> Result<(), RuntimeError> {
        let Some(uri) = run.candidates.pop_front() else {
            return Err(run.host_failure(ModuleHostError::NotFound));
        };
        let request = ModuleRequestKind::HasElement {
            key: run.declaration_key.clone(),
            module: run.module.clone(),
            element_uri: uri.into(),
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
                let prepared = (absolute.clone().into(), original.clone().into());
                Arc::make_mut(&mut self.program).update_import_uris(current.spec.unit, |span| {
                    (span == current.spec.span).then_some(&prepared)
                });
                if current.clause {
                    let mut current = current;
                    current.spec.absolute_uri = absolute.into();
                    current.spec.original_uri = original.into();
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
    let mut steps = resolved
        .preparation
        .iter()
        .map(|step| match step {
            crate::resolver::AstPreparation::Import {
                uri,
                span,
                member,
                clause,
            } => PreparationStep::Import {
                clause: *clause,
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
