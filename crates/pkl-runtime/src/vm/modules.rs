use super::*;
use crate::modules::{
    ModuleCompletion, ModuleIdentity, ModuleRequest, ModuleRequestKind, ModuleSource,
    ResolvedModule, UnitId,
};
use crate::value::RuntimeSource;

mod inheritance;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ModuleMode {
    Plain,
    Extends,
    Amends,
}

#[derive(Clone, Debug)]
pub(crate) struct ModuleUnit {
    pub id: UnitId,
    pub identity: Option<ModuleIdentity>,
    pub source: Option<Arc<RuntimeSource>>,
    pub name: Arc<str>,
    pub template: usize,
    pub entry: usize,
    pub initial_members: Vec<Member>,
    pub(super) import_order: Vec<uri_resolution::PreparationStep>,
    pub class_id: usize,
    pub mode: ModuleMode,
    pub modifiers: u32,
}

#[derive(Clone, Debug)]
pub struct ImportSpec {
    pub authored: Arc<str>,
    pub span: Span,
    pub unit: UnitId,
    pub absolute_uri: Arc<str>,
    pub original_uri: Arc<str>,
}
impl ImportSpec {
    pub(super) fn new(
        authored: Arc<str>,
        span: Span,
        unit: UnitId,
        identity: Option<&ModuleIdentity>,
    ) -> Result<Self, RuntimeError> {
        let (absolute, original) = crate::host::resources::prepare_module_uri(
            &authored,
            identity.map(|m| m.original_uri.as_ref()),
        )
        .map_err(|e| e.at(span))?;
        Ok(Self {
            authored,
            span,
            unit,
            absolute_uri: absolute.into(),
            original_uri: original.into(),
        })
    }
    pub(super) fn authored(authored: Arc<str>, span: Span, unit: UnitId) -> Self {
        Self {
            absolute_uri: authored.clone(),
            original_uri: authored.clone(),
            authored,
            span,
            unit,
        }
    }
}

impl Program {
    /// Register the primary ModuleKey separately from its display provenance.
    /// This prepares all import URI operands before any guest member is forced.
    pub fn register_primary_source(
        &mut self,
        source: Arc<RuntimeSource>,
        identity: ModuleIdentity,
    ) -> Result<(), RuntimeError> {
        let Some(unit) = self.units.first_mut() else {
            return Ok(());
        };
        unit.source = Some(source.clone());
        unit.name = source.module_name.clone();
        unit.identity = Some(identity.clone());
        self.module_name = source.module_name.clone();
        Ok(())
    }
}

#[derive(Clone)]
pub(super) enum CachedModule {
    Value(ObjectId),
    Failed(RuntimeError),
}

pub(super) struct ModuleRun {
    pub spec: ImportSpec,
    site: (ExecutableOwnerId, usize, usize),
    phase: ModulePhase,
    bundled: bool,
    pub resolved: Option<ResolvedModule>,
    pub receiver: Option<ObjectId>,
    pub initializing: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ModulePhase {
    ImportCheck,
    Resolve,
    Source,
    Initializer,
}

impl Vm {
    pub(super) fn current_module_name(&self) -> &str {
        self.frames
            .last()
            .and_then(|frame| self.program.metadata.get(frame.block))
            .and_then(|metadata| metadata.source.as_ref())
            .map_or(&self.program.module_name, |source| &source.module_name)
    }
    pub(super) fn module_cached(&mut self, cached: CachedModule) -> Result<Value, RuntimeError> {
        match cached {
            CachedModule::Value(id) => Ok(Value::Object(id)),
            CachedModule::Failed(error) => {
                // A cached exception is thrown by ModuleCache, whose boundary
                // has no guest call node, just like source initialization.
                if let Some(frame) = self.frames.last_mut() {
                    frame.callsite_hidden = true;
                }
                Err(error)
            }
        }
    }

    pub(super) fn begin_import(&mut self, spec: ImportSpec) -> Result<(), RuntimeError> {
        let owner = self.current_executable()?;
        let frame = self.frames.last().expect("import frame");
        let site = (frame.block, frame.pc - 1);
        if let Some(receiver) = self.heap.imported_module(owner, site)? {
            self.stack.push(Value::Object(receiver));
            return Ok(());
        }
        let importer = self
            .program
            .units
            .get(spec.unit.0 as usize)
            .and_then(|unit| unit.identity.clone())
            .ok_or_else(|| {
                RuntimeError::new(
                    ErrorKind::Unsupported,
                    "Module imports require a registered declaration ModuleKey.",
                )
            })?;
        let request = ModuleRequestKind::CheckImport {
            importer,
            import_uri: spec.absolute_uri.clone(),
        };
        self.frames.last_mut().expect("import frame").module_load = Some(ModuleRun {
            site: (owner, site.0, site.1),
            phase: ModulePhase::ImportCheck,
            bundled: crate::modules::stdlib_kind(&spec.absolute_uri).is_some(),
            spec,
            resolved: None,
            receiver: None,
            initializing: false,
        });
        self.wait_module(request)
    }

    fn checked_import(
        &mut self,
        result: Result<(), crate::modules::ModuleHostError>,
    ) -> Result<(), RuntimeError> {
        let run = self
            .frames
            .last()
            .and_then(|frame| frame.module_load.as_ref())
            .ok_or_else(|| invalid("missing import admission continuation"))?;
        if run.phase != ModulePhase::ImportCheck {
            return Err(invalid("unexpected import admission phase"));
        }
        result.map_err(|error| error.into_runtime(&run.spec.absolute_uri))?;
        let spec = run.spec.clone();
        match crate::modules::stdlib_kind(&spec.absolute_uri) {
            Some(crate::modules::StdlibKind::Missing) => {
                return Err(self.stdlib_source(&spec.absolute_uri).unwrap_err());
            }
            Some(crate::modules::StdlibKind::Singleton(canonical)) => {
                if let Some(cached) = self.modules_original.get(canonical).cloned() {
                    let value = self.module_cached(cached)?;
                    return self.finish_import(value);
                }
                return self.initialize_bundled(ResolvedModule {
                    original_uri: canonical.into(),
                    resolved_uri: canonical.into(),
                    source_uri: canonical.into(),
                });
            }
            _ => {}
        }
        if let Some(cached) = self.modules_original.get(&spec.original_uri).cloned() {
            let value = self.module_cached(cached)?;
            return self.finish_import(value);
        }
        let importer = self.program.units[spec.unit.0 as usize]
            .identity
            .clone()
            .ok_or_else(|| invalid("import identity disappeared"))?;
        self.frames
            .last_mut()
            .unwrap()
            .module_load
            .as_mut()
            .unwrap()
            .phase = ModulePhase::Resolve;
        self.wait_module(ModuleRequestKind::Resolve {
            importer,
            import_uri: spec.absolute_uri,
            original_uri: spec.original_uri,
        })
    }

    fn finish_import(&mut self, value: Value) -> Result<(), RuntimeError> {
        let receiver = value.as_object()?;
        let run = self
            .frames
            .last_mut()
            .and_then(|frame| frame.module_load.take())
            .ok_or_else(|| invalid("missing import result continuation"))?;
        self.heap
            .publish_imported_module(run.site.0, (run.site.1, run.site.2), receiver)?;
        self.stack.push(value);
        self.state = State::Running;
        Ok(())
    }

    fn initialize_bundled(&mut self, resolved: ResolvedModule) -> Result<(), RuntimeError> {
        let receiver = self.heap.allocate(Object::default());
        let run = self
            .frames
            .last_mut()
            .and_then(|frame| frame.module_load.as_mut())
            .ok_or_else(|| invalid("missing bundled import continuation"))?;
        run.resolved = Some(resolved.clone());
        run.receiver = Some(receiver);
        run.initializing = true;
        run.phase = ModulePhase::Initializer;
        let entry = self.stdlib_entry(&resolved.original_uri)?;
        self.state = State::Running;
        self.push_module_initializer(entry, Some(receiver));
        Ok(())
    }

    pub(super) fn wait_module(&mut self, kind: ModuleRequestKind) -> Result<(), RuntimeError> {
        let id = self.next_request;
        self.next_request = id
            .checked_add(1)
            .ok_or_else(|| invalid("Host request identity exhausted"))?;
        self.state = State::Waiting(PendingRequest::Module(ModuleRequest { id, kind }));
        Ok(())
    }

    pub fn complete_module(
        &mut self,
        id: u64,
        completion: ModuleCompletion,
    ) -> Result<(), RuntimeError> {
        let State::Waiting(PendingRequest::Module(request)) = &self.state else {
            return Err(stale_module());
        };
        if request.id != id
            || !matches!(
                (&request.kind, &completion),
                (
                    ModuleRequestKind::CheckImport { .. },
                    ModuleCompletion::ImportChecked(_)
                ) | (
                    ModuleRequestKind::Capabilities { .. },
                    ModuleCompletion::Capabilities(_)
                ) | (
                    ModuleRequestKind::HasElement { .. },
                    ModuleCompletion::HasElement(_)
                ) | (
                    ModuleRequestKind::Resolve { .. },
                    ModuleCompletion::Resolved(_)
                ) | (
                    ModuleRequestKind::LoadSource { .. },
                    ModuleCompletion::Source(_)
                )
            )
        {
            return Err(stale_module());
        }
        if matches!(
            &completion,
            ModuleCompletion::ImportChecked(Err(
                crate::modules::ModuleHostError::NotFound | crate::modules::ModuleHostError::Io(_)
            ))
        ) {
            return Err(invalid(
                "Import admission does not accept module lookup or I/O failures.",
            ));
        }
        if let (
            ModuleRequestKind::Resolve {
                import_uri,
                original_uri,
                ..
            },
            ModuleCompletion::Resolved(Ok(resolved)),
        ) = (&request.kind, &completion)
            && crate::modules::stdlib_kind(import_uri) == Some(crate::modules::StdlibKind::Cached)
            && (resolved.original_uri != *original_uri
                || resolved.resolved_uri != *import_uri
                || resolved.source_uri != *import_uri)
        {
            return Err(invalid(
                "Bundled module resolution must retain its pinned key and source identity.",
            ));
        }
        if matches!(
            &completion,
            ModuleCompletion::Source(Err(
                crate::modules::ModuleHostError::SecurityManagerException { .. }
            ))
        ) {
            return Err(RuntimeError::new(
                ErrorKind::Host,
                "SecurityManagerException is only supported for module resolution.",
            ));
        }
        if matches!(
            &completion,
            ModuleCompletion::Capabilities(Err(
                crate::modules::ModuleHostError::SecurityManagerException { .. }
            ))
        ) {
            return Err(RuntimeError::new(
                ErrorKind::Host,
                "SecurityManagerException is not supported for ModuleKey capability getters.",
            ));
        }
        let result = match completion {
            ModuleCompletion::ImportChecked(result) => self.checked_import(result),
            ModuleCompletion::Capabilities(result) => self.resolved_capabilities(result),
            ModuleCompletion::HasElement(result) => self.resolved_element(result),
            ModuleCompletion::Resolved(result) => self.resolved_module(result),
            ModuleCompletion::Source(result) => self.loaded_module(result),
        };
        if let Err(error) = result {
            self.fail(error);
        }
        Ok(())
    }

    fn resolved_module(
        &mut self,
        result: Result<ResolvedModule, crate::modules::ModuleHostError>,
    ) -> Result<(), RuntimeError> {
        let run = self
            .frames
            .last_mut()
            .and_then(|frame| frame.module_load.as_mut())
            .ok_or_else(|| invalid("missing import continuation"))?;
        let resolved = result.map_err(|error| error.into_runtime(&run.spec.original_uri))?;
        if resolved.original_uri != run.spec.original_uri {
            return Err(invalid("module resolver changed original key"));
        }
        if let Some(cached) = self.modules_resolved.get(&resolved.resolved_uri).cloned() {
            let value = self.module_cached(cached)?;
            // A resolved-key hit does not seed the original alias cache.
            return self.finish_import(value);
        }
        if run.bundled {
            return self.initialize_bundled(resolved);
        }
        // VmTyped exists before source I/O but is not cache-visible until that
        // source succeeds. A callback can independently load the same key.
        let receiver = self.heap.allocate(Object::default());
        run.receiver = Some(receiver);
        run.resolved = Some(resolved.clone());
        run.phase = ModulePhase::Source;
        self.wait_module(ModuleRequestKind::LoadSource { resolved })
    }

    fn loaded_module(
        &mut self,
        result: Result<ModuleSource, crate::modules::ModuleHostError>,
    ) -> Result<(), RuntimeError> {
        let run = self
            .frames
            .last_mut()
            .and_then(|frame| frame.module_load.as_mut())
            .ok_or_else(|| invalid("missing module source continuation"))?;
        let resolved = run
            .resolved
            .clone()
            .ok_or_else(|| invalid("missing resolved key"))?;
        // From this boundary onward failure is cached, including source errors.
        run.initializing = true;
        run.phase = ModulePhase::Initializer;
        let input = result.map_err(|error| error.into_runtime(&resolved.original_uri))?;
        let receiver = run
            .receiver
            .ok_or_else(|| invalid("missing empty module"))?;
        self.modules_original
            .insert(resolved.original_uri.clone(), CachedModule::Value(receiver));
        self.modules_resolved
            .insert(resolved.resolved_uri.clone(), CachedModule::Value(receiver));
        let mut source = Arc::new(RuntimeSource {
            text: input.text.clone(),
            uri: resolved.source_uri.clone(),
            module_name: input.inferred_name,
            line_anchors: false,
        });
        let frame = self.frames.last().expect("import continuation");
        let metadata = &self.program.metadata[frame.block];
        let member = PklString::from(
            metadata
                .source
                .as_ref()
                .map_or(self.program.module_name.as_ref(), |source| {
                    source.module_name.as_ref()
                }),
        )
        .concat(frame.diagnostic_member.as_ref().unwrap_or(&metadata.member));
        let parsed = crate::parser::parse_module_for_evaluation(&input.text)
            .map_err(|error| imported_diagnostic(error, source.clone(), member.clone()))?;
        if let Some(name) = parsed.declaration.as_ref().and_then(|d| d.name.as_ref()) {
            Arc::make_mut(&mut source).module_name = name.join(".").into();
        }
        let resolved_ast = crate::resolver::resolve_module_for_evaluation(&parsed);
        // The evaluator owns one append-only code arena. Clone only while an
        // external prepared-program snapshot shares it; subsequent modules
        // append in place. Failed compilation retracts only its uncommitted tail.
        let program = Arc::make_mut(&mut self.program);
        let checkpoint = ArenaCheckpoint::new(program);
        let compiled = compile_unit_into(program, &resolved_ast, CompileOptions { fuse_constant_add: true }, self.power_assertions,
            Some(source.clone()), Some(ModuleIdentity { original_uri: resolved.original_uri, resolved_uri: resolved.resolved_uri }))
            .and_then(|unit| {
                if program.blocks[checkpoint.blocks..].iter().flatten().any(|instruction|
                    matches!(instruction, Instruction::Constant(value) if value.heap_id().is_some())) {
                    Err(invalid("object constants cannot be stored in compiled code"))
                } else { Ok(unit) }
            });
        let unit = match compiled {
            Ok(unit) => unit,
            Err(mut error) => {
                checkpoint.rollback(program);
                if let Some(span) = error.span {
                    error.frames.insert(
                        0,
                        RuntimeFrame {
                            span,
                            source: Some(source),
                            member: PklString::default(),
                        },
                    );
                }
                return Err(error);
            }
        };
        let entry = program.units[unit.0 as usize].entry;
        self.state = State::Running;
        self.push_module_initializer(entry, Some(receiver));
        Ok(())
    }

    pub(super) fn push_module_initializer(&mut self, entry: usize, receiver: Option<ObjectId>) {
        self.frames.push(Frame {
            guest_catch: None,
            block: entry,
            pc: 0,
            base: self.stack.len(),
            environment: Environment::default(),
            force: None,
            lambda: false,
            type_run: None,
            type_resolution: None,
            generic_default: None,
            equality_run: None,
            deep_force: None,
            pcf_render: None,
            collection_join: None,
            function_owner: None,
            amendment_end: None,
            prepared_hashes: None,
            key_search: None,
            return_to_type: false,
            diagnostic_member: None,
            callsite_hidden: false,
            builders: Vec::new(),
            generator_scopes: Vec::new(),
            module_load: None,
            uri_resolution: None,
            module_receiver: receiver,
        });
    }

    pub(super) fn step_module(&mut self) -> Result<(), RuntimeError> {
        let run = self
            .frames
            .last_mut()
            .and_then(|frame| frame.module_load.take())
            .ok_or_else(|| invalid("missing module continuation"))?;
        if !run.initializing || run.phase != ModulePhase::Initializer {
            return Err(invalid(
                "module continuation resumed before Host completion",
            ));
        }
        // The initializer already returned the actual receiver onto our stack.
        if self.stack.last().and_then(Value::heap_id) != run.receiver {
            return Err(invalid("module initializer result mismatch"));
        }
        self.heap.publish_imported_module(
            run.site.0,
            (run.site.1, run.site.2),
            run.receiver
                .ok_or_else(|| invalid("missing initialized module"))?,
        )?;
        Ok(())
    }
}

fn stale_module() -> RuntimeError {
    RuntimeError::new(ErrorKind::Host, "stale or unexpected Host completion")
}

pub(super) fn imported_diagnostic(
    diagnostic: crate::diagnostic::Diagnostic,
    source: Arc<RuntimeSource>,
    member: PklString,
) -> RuntimeError {
    let mut error = RuntimeError::new(ErrorKind::Type, diagnostic.message.clone());
    error.span = Some(diagnostic.span);
    error.hint = diagnostic.hint().cloned();
    error.frames.push(RuntimeFrame {
        span: diagnostic.span,
        source: Some(source.clone()),
        member: diagnostic
            .member
            .clone()
            .map_or_else(PklString::default, |suffix| {
                PklString::from("#").concat(&suffix)
            }),
    });
    error.frontend = Some(Arc::new(crate::modules::ModuleDiagnostic {
        diagnostic,
        source,
        member,
    }));
    error
}

/// Compiler transactions never mutate an already published instruction or
/// declaration. Constraint origins are keyed by their newly appended blocks.
pub(super) struct ArenaCheckpoint {
    blocks: usize,
    metadata: usize,
    objects: usize,
    units: usize,
    declaration: usize,
    type_recipes: usize,
    aliases: BTreeSet<usize>,
}
impl ArenaCheckpoint {
    pub(super) fn new(program: &Program) -> Self {
        Self {
            blocks: program.blocks.len(),
            metadata: program.metadata.len(),
            objects: program.objects.len(),
            units: program.units.len(),
            declaration: program.next_declaration,
            type_recipes: program.type_recipes.len(),
            aliases: program.alias_definitions.keys().copied().collect(),
        }
    }
    pub(super) fn rollback(self, program: &mut Program) {
        program.blocks.truncate(self.blocks);
        program.metadata.truncate(self.metadata);
        program.objects.truncate(self.objects);
        program.units.truncate(self.units);
        program.next_declaration = self.declaration;
        program.type_recipes.truncate(self.type_recipes);
        program
            .alias_definitions
            .retain(|id, _| self.aliases.contains(id));
        program
            .constraint_origins
            .retain(|block, _| *block < self.blocks);
    }
}
