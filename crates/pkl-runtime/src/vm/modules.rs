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
            identity.map(|key| key.original_uri.as_ref()),
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
        for block in &mut self.blocks {
            for instruction in block {
                if let Instruction::ImportModule(spec) = instruction
                    && spec.unit == UnitId(0)
                {
                    *spec = ImportSpec::new(
                        spec.authored.clone(),
                        spec.span,
                        spec.unit,
                        Some(&identity),
                    )?;
                }
            }
        }
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
    pub resolved: Option<ResolvedModule>,
    pub receiver: Option<ObjectId>,
    pub initializing: bool,
}

impl Vm {
    pub(super) fn current_module_name(&self) -> &str {
        self.frames
            .last()
            .and_then(|frame| self.program.metadata.get(frame.block))
            .and_then(|metadata| metadata.source.as_ref())
            .map_or(&self.program.module_name, |source| &source.module_name)
    }
    fn module_cached(&self, cached: CachedModule) -> Result<Value, RuntimeError> {
        match cached {
            CachedModule::Value(id) => Ok(Value::Object(id)),
            CachedModule::Failed(mut error) => {
                error.cached_module_frames = true;
                Err(error)
            }
        }
    }

    pub(super) fn begin_import(&mut self, spec: ImportSpec) -> Result<(), RuntimeError> {
        if let Some(cached) = self.modules_original.get(&spec.original_uri).cloned() {
            let value = self.module_cached(cached)?;
            self.stack.push(value);
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
        let request = ModuleRequestKind::Resolve {
            importer,
            import_uri: spec.absolute_uri.clone(),
            original_uri: spec.original_uri.clone(),
        };
        self.frames.last_mut().expect("import frame").module_load = Some(ModuleRun {
            spec,
            resolved: None,
            receiver: None,
            initializing: false,
        });
        self.wait_module(request)
    }

    fn wait_module(&mut self, kind: ModuleRequestKind) -> Result<(), RuntimeError> {
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
        let result = match completion {
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
            self.frames.last_mut().unwrap().module_load = None;
            self.stack.push(value);
            self.state = State::Running;
            return Ok(());
        }
        // VmTyped exists before source I/O but is not cache-visible until that
        // source succeeds. A callback can independently load the same key.
        let receiver = self.heap.allocate(Object::default());
        run.receiver = Some(receiver);
        run.resolved = Some(resolved.clone());
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
        let parsed = crate::parser::parse_module(&input.text)
            .map_err(|error| imported_diagnostic(error, source.clone(), member.clone()))?;
        if let Some(name) = parsed.declaration.as_ref().and_then(|d| d.name.as_ref()) {
            Arc::make_mut(&mut source).module_name = name.join(".").into();
        }
        let resolved_ast = crate::resolver::resolve_module(&parsed)
            .map_err(|error| imported_diagnostic(error, source.clone(), member))?;
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
        self.frames.push(Frame {
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
            function_owner: None,
            amendment_end: None,
            prepared_hashes: None,
            key_search: None,
            return_to_type: false,
            diagnostic_member: None,
            callsite_hidden: false,
            builders: Vec::new(),
            module_load: None,
            module_receiver: Some(receiver),
        });
        Ok(())
    }

    pub(super) fn step_module(&mut self) -> Result<(), RuntimeError> {
        let run = self
            .frames
            .last_mut()
            .and_then(|frame| frame.module_load.take())
            .ok_or_else(|| invalid("missing module continuation"))?;
        if !run.initializing {
            return Err(invalid(
                "module continuation resumed before Host completion",
            ));
        }
        // The initializer already returned the actual receiver onto our stack.
        if self.stack.last().and_then(Value::heap_id) != run.receiver {
            return Err(invalid("module initializer result mismatch"));
        }
        Ok(())
    }
}

fn stale_module() -> RuntimeError {
    RuntimeError::new(ErrorKind::Host, "stale or unexpected Host completion")
}

fn imported_diagnostic(
    diagnostic: crate::diagnostic::Diagnostic,
    source: Arc<RuntimeSource>,
    member: PklString,
) -> RuntimeError {
    let mut error = RuntimeError::new(ErrorKind::Type, diagnostic.message.clone());
    error.span = Some(diagnostic.span);
    error.hint = diagnostic.hint().map(str::to_owned);
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
