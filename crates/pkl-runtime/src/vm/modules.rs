use super::*;
use crate::modules::{
    ModuleCompletion, ModuleIdentity, ModuleKey, ModuleKeyId, ModuleKeyKind, ModuleRequest,
    ModuleRequestKind, ModuleSource, ResolvedKeyId, ResolvedModule, ResolvedModuleKey,
    SelectionPhase, UnitId,
};
use crate::source::SourceText;
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
    pub standard_library: bool,
    pub key: Option<ModuleKey>,
    pub delegate_key: Option<ModuleKey>,
    pub build: Option<ModuleBuild>,
    pub syntax: Option<Arc<crate::syntax::Module>>,
    pub source: Option<Arc<RuntimeSource>>,
    pub name: PklString,
    pub template: usize,
    pub entry: usize,
    pub initial_members: Vec<Member>,
    pub(super) import_order: Vec<uri_resolution::PreparationStep>,
    pub class_id: usize,
    pub mode: ModuleMode,
    pub modifiers: u32,
}

#[derive(Clone, Debug)]
pub(crate) struct ModuleBuild {
    pub module: crate::syntax::Module,
    pub options: CompileOptions,
    pub instrument: bool,
    pub prepared: BTreeMap<Span, (PklString, PklString)>,
}

impl ModuleUnit {
    pub(super) fn is_singleton(&self) -> bool {
        self.standard_library
            && self.identity.as_ref().is_some_and(|identity| {
                matches!(
                    crate::modules::stdlib_kind(&identity.original_uri),
                    Some(crate::modules::StdlibKind::Singleton(_))
                )
            })
    }
}

#[derive(Clone, Debug)]
pub struct ImportSpec {
    pub authored: PklString,
    pub span: Span,
    pub unit: UnitId,
    pub absolute_uri: PklString,
    pub original_uri: PklString,
}
impl ImportSpec {
    pub(super) fn new(
        authored: PklString,
        span: Span,
        unit: UnitId,
        identity: Option<&ModuleIdentity>,
    ) -> Result<Self, RuntimeError> {
        let (absolute, original) = crate::host::resources::prepare_module_uri(
            &authored,
            identity.map(|m| &m.original_uri),
        )
        .map_err(|e| e.at(span))?;
        Ok(Self {
            authored,
            span,
            unit,
            absolute_uri: absolute,
            original_uri: original,
        })
    }
    pub(super) fn authored(authored: PklString, span: Span, unit: UnitId) -> Self {
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
        let Some(unit) = self.units.get_mut(0) else {
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

/// Module URI equality shares the Java URI.equals rules already used by resources.
#[derive(Default)]
pub(super) struct ModuleCache(BTreeMap<crate::host::resources::CacheKey, CachedModule>);
impl ModuleCache {
    pub fn get(&self, uri: impl Into<PklString>) -> Option<&CachedModule> {
        self.0.get(&crate::host::resources::cache_key(&uri.into()))
    }
    pub fn insert(&mut self, uri: PklString, value: CachedModule) {
        self.0
            .insert(crate::host::resources::cache_key(&uri), value);
    }
    pub fn clear(&mut self) {
        self.0.clear();
    }
    pub fn values(&self) -> impl Iterator<Item = &CachedModule> {
        self.0.values()
    }
    pub fn values_mut(&mut self) -> impl Iterator<Item = &mut CachedModule> {
        self.0.values_mut()
    }
}

static NEXT_KEY: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
pub(super) fn next_key_id() -> Result<u64, RuntimeError> {
    NEXT_KEY
        .fetch_update(
            std::sync::atomic::Ordering::Relaxed,
            std::sync::atomic::Ordering::Relaxed,
            |n| n.checked_add(1),
        )
        .map_err(|_| invalid("ModuleKey identity exhausted"))
}

pub(super) fn next_failure_id() -> u64 {
    next_key_id().expect("cached exception identity exhausted")
}

type ParsedModule =
    Result<crate::syntax::Module, Box<(crate::parser::ModuleParseFailure, Option<PklString>)>>;

pub(super) struct ModuleRun {
    pub spec: ImportSpec,
    site: Option<(ExecutableOwnerId, usize, usize)>,
    purpose: ModulePurpose,
    supplied: Option<SourceText>,
    internal_key: bool,
    phase: ModulePhase,
    key: Option<ModuleKey>,
    delegate_key: Option<ModuleKey>,
    resolved_key: Option<ResolvedModuleKey>,
    cached: bool,
    pub resolved: Option<ResolvedModule>,
    pub receiver: Option<ObjectId>,
    pub initializing: bool,
    parsed: Option<ParsedModule>,
    source: Option<Arc<RuntimeSource>>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ModulePurpose {
    Import,
    Primary,
    Query,
}

impl ModuleRun {
    pub(super) fn is_import_initializing(&self) -> bool {
        self.initializing && self.purpose == ModulePurpose::Import
    }

    pub(super) fn is_singleton(&self) -> bool {
        self.key.as_ref().is_some_and(|key| {
            key.kind == ModuleKeyKind::StandardLibrary
                && matches!(
                    crate::modules::stdlib_kind(&key.uri),
                    Some(crate::modules::StdlibKind::Singleton(_))
                )
        })
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ModulePhase {
    ImportCheck,
    SelectKey,
    CachePolicy,
    Resolve,
    Source,
    Name,
    Initializer,
}

impl Vm {
    pub(crate) fn append_public_operation(
        &mut self,
        source: crate::modules::ModuleSourceSpec,
        expression: SourceText,
    ) -> Result<usize, RuntimeError> {
        crate::host::resources::validate_module_key_uri(&source.uri)?;
        let program = Arc::make_mut(&mut self.program);
        program.seal_code_segment();
        let mut code = vec![Instruction::LoadPrimary(Arc::new(source))];
        if expression.original() == "output.text" {
            code.extend([
                Instruction::GetMember("output".into()),
                Instruction::CheckModuleOutput,
                Instruction::GetMember("text".into()),
            ]);
        } else {
            code.push(Instruction::EvaluateQuery(expression));
        }
        code.push(Instruction::Return);
        let entry = program.alloc_block(code, BlockMetadata::default())?;
        program.seal_code_segment();
        self.public_operation_entries.insert(entry);
        Ok(entry)
    }
    fn fresh_module_run(
        &self,
        uri: PklString,
        purpose: ModulePurpose,
        supplied: Option<SourceText>,
    ) -> ModuleRun {
        ModuleRun {
            spec: ImportSpec {
                authored: uri.clone(),
                absolute_uri: uri.clone(),
                original_uri: uri,
                span: Span::default(),
                unit: UnitId(u32::MAX),
            },
            site: None,
            purpose,
            supplied,
            internal_key: false,
            phase: ModulePhase::SelectKey,
            key: None,
            delegate_key: None,
            resolved_key: None,
            cached: true,
            resolved: None,
            receiver: None,
            initializing: false,
            parsed: None,
            source: None,
        }
    }
    pub(super) fn begin_primary(
        &mut self,
        source: Arc<crate::modules::ModuleSourceSpec>,
    ) -> Result<(), RuntimeError> {
        self.public_primary = None;
        let (_, normalized) = crate::host::resources::prepare_module_uri(&source.uri, None)?;
        let mut run = self.fresh_module_run(
            source.uri.clone(),
            ModulePurpose::Primary,
            source.contents.clone(),
        );
        run.spec.original_uri = normalized.clone();
        self.frames.last_mut().unwrap().module_load = Some(run);
        let id = ModuleKeyId(next_key_id()?);
        if source.uri == "repl:text" && source.contents.is_some() {
            self.frames
                .last_mut()
                .unwrap()
                .module_load
                .as_mut()
                .unwrap()
                .internal_key = true;
            return self.selected_module_key(Ok(ModuleKey {
                id,
                uri: source.uri.clone(),
                kind: ModuleKeyKind::Synthetic,
                handle: "".into(),
            }));
        }
        self.wait_module(ModuleRequestKind::SelectKey {
            id,
            uri: normalized,
            phase: SelectionPhase::Primary,
        })
    }
    pub(super) fn begin_query(&mut self, expression: SourceText) -> Result<(), RuntimeError> {
        let receiver = self.pop()?.as_object()?;
        let mut run =
            self.fresh_module_run("repl:text".into(), ModulePurpose::Query, Some(expression));
        run.receiver = Some(receiver);
        run.phase = ModulePhase::Resolve;
        run.internal_key = true;
        let key = ModuleKey {
            id: ModuleKeyId(next_key_id()?),
            uri: "repl:text".into(),
            kind: ModuleKeyKind::Synthetic,
            handle: "".into(),
        };
        run.key = Some(key.clone());
        self.frames.last_mut().unwrap().module_load = Some(run);
        self.wait_module(ModuleRequestKind::ResolveKey {
            key,
            resolved_key_id: ResolvedKeyId(next_key_id()?),
            phase: crate::modules::ResolutionPhase::Query,
        })
    }
    fn compile_public_query(&mut self) -> Result<(), RuntimeError> {
        let run = self
            .frames
            .last_mut()
            .unwrap()
            .module_load
            .take()
            .ok_or_else(|| invalid("missing query continuation"))?;
        let receiver = run
            .receiver
            .ok_or_else(|| invalid("missing query receiver"))?;
        let unit = self
            .heap
            .get(receiver)?
            .module_unit
            .ok_or_else(|| invalid("query receiver has no module unit"))?;
        let declaration = self.program.units[unit.0 as usize].clone();
        let source = declaration
            .source
            .ok_or_else(|| invalid("query module has no source"))?;
        let module = declaration
            .syntax
            .map_or_else(
                || crate::parser::parse_module_for_evaluation(&source.text),
                |syntax| Ok((*syntax).clone()),
            )
            .map_err(|failure| {
                self.module_parse_error(*failure, source.clone(), Some("".into()))
            })?;
        let expression = run
            .supplied
            .ok_or_else(|| invalid("missing query source"))?;
        let program = Arc::make_mut(&mut self.program);
        let arena = query_lifecycle::CodeArena::begin(program);
        let entry = session_expressions::compile_session_expression_for_unit(
            program,
            &module,
            &expression,
            CompileOptions {
                fuse_constant_add: true,
            },
            self.power_assertions,
            unit,
        )
        .map_err(|failure| match failure {
            SessionExpressionError::Frontend(error) => imported_diagnostic(
                *error,
                Arc::new(RuntimeSource {
                    text: expression.clone(),
                    uri: "repl:text".into(),
                    module_name: "".into(),
                }),
                "repl:text".into(),
            ),
            SessionExpressionError::Runtime(error) => *error,
        })?;
        self.push_module_initializer(entry, None);
        // The synthetic query owns its executable state for this operation.
        // Frames and escaped environments trace this owner; the evaluator's
        // permanent static-root table must not retain a completed query site.
        let owner = self.heap.allocate_executable();
        self.code_arenas
            .push(arena.finish(Arc::make_mut(&mut self.program), owner, true));
        self.frames.last_mut().unwrap().environment.executable_owner = Some(owner);
        self.frames.last_mut().unwrap().environment.code_owner = Some(owner);
        self.frames
            .last_mut()
            .unwrap()
            .environment
            .scopes
            .push(Scope {
                receiver,
                owner: receiver,
            });
        self.state = State::Running;
        Ok(())
    }
    pub(crate) fn public_primary_source(&self) -> Option<Arc<RuntimeSource>> {
        self.public_primary
            .and_then(|id| self.heap.get(id).ok())
            .and_then(|object| object.module_unit)
            .and_then(|unit| self.program.units[unit.0 as usize].source.clone())
    }
    pub(super) fn current_module_name(&self) -> &PklString {
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
        self.begin_import_node(spec, true)
    }
    pub(super) fn begin_glob_member_import(
        &mut self,
        spec: ImportSpec,
    ) -> Result<(), RuntimeError> {
        self.begin_import_node(spec, false)
    }
    fn begin_import_node(
        &mut self,
        spec: ImportSpec,
        cache_node: bool,
    ) -> Result<(), RuntimeError> {
        let owner = self.current_executable()?;
        let frame = self.frames.last().expect("import frame");
        let site = (frame.block, frame.pc - 1);
        if cache_node && let Some(receiver) = self.heap.imported_module(owner, site)? {
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
            site: cache_node.then_some((owner, site.0, site.1)),
            purpose: ModulePurpose::Import,
            supplied: None,
            internal_key: false,
            phase: ModulePhase::ImportCheck,
            key: None,
            delegate_key: None,
            resolved_key: None,
            cached: true,
            spec,
            resolved: None,
            receiver: None,
            initializing: false,
            parsed: None,
            source: None,
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
        let uri = run.spec.original_uri.clone();
        self.frames
            .last_mut()
            .unwrap()
            .module_load
            .as_mut()
            .unwrap()
            .phase = ModulePhase::SelectKey;
        self.wait_module(ModuleRequestKind::SelectKey {
            id: ModuleKeyId(next_key_id()?),
            uri,
            phase: SelectionPhase::RuntimeImport,
        })
    }

    fn selected_module_key(
        &mut self,
        result: Result<ModuleKey, crate::modules::ModuleHostError>,
    ) -> Result<(), RuntimeError> {
        let run = self
            .frames
            .last_mut()
            .and_then(|frame| frame.module_load.as_mut())
            .ok_or_else(|| invalid("missing key selection continuation"))?;
        let key = result.map_err(|error| error.into_factory_runtime(&run.spec.absolute_uri))?;
        let key =
            if run.purpose == ModulePurpose::Primary && run.supplied.is_some() && !run.internal_key
            {
                run.delegate_key = Some(key.clone());
                run.internal_key = true;
                ModuleKey {
                    id: ModuleKeyId(next_key_id()?),
                    uri: key.uri,
                    kind: ModuleKeyKind::SuppliedText,
                    handle: "".into(),
                }
            } else {
                key
            };
        run.spec.original_uri = key.uri.clone();
        run.key = Some(key.clone());
        if key.kind == ModuleKeyKind::StandardLibrary {
            match crate::modules::stdlib_kind(&key.uri) {
                Some(crate::modules::StdlibKind::Missing) => {
                    return Err(self.stdlib_source(&key.uri).unwrap_err());
                }
                Some(crate::modules::StdlibKind::Singleton(canonical)) => {
                    if let Some(cached) = self.modules_stdlib.get(canonical).cloned() {
                        let value = self.module_cached(cached)?;
                        return self.finish_import(value);
                    }
                    return self.initialize_bundled(ResolvedModule {
                        original_uri: canonical.into(),
                        resolved_uri: canonical.into(),
                        source_uri: canonical.into(),
                    });
                }
                Some(crate::modules::StdlibKind::Cached) => {}
                None => return Err(invalid("StandardLibrary key requires the pkl scheme")),
            }
        }
        let internal_key = run.internal_key;
        self.frames
            .last_mut()
            .unwrap()
            .module_load
            .as_mut()
            .unwrap()
            .phase = ModulePhase::CachePolicy;
        if internal_key {
            return self.queried_module_cache_policy(Ok(key.kind == ModuleKeyKind::SuppliedText));
        }
        self.wait_module(ModuleRequestKind::QueryCachePolicy { key })
    }

    fn queried_module_cache_policy(
        &mut self,
        result: Result<bool, crate::modules::ModuleHostError>,
    ) -> Result<(), RuntimeError> {
        let run = self
            .frames
            .last_mut()
            .and_then(|frame| frame.module_load.as_mut())
            .ok_or_else(|| invalid("missing cache policy continuation"))?;
        run.cached = result.map_err(|error| error.into_runtime(&run.spec.original_uri))?;
        let spec = run.spec.clone();
        let key = run
            .key
            .clone()
            .ok_or_else(|| invalid("missing selected ModuleKey"))?;
        if run.cached
            && let Some(cached) = self.modules_original.get(&key.uri).cloned()
        {
            let value = self.module_cached(cached)?;
            return self.finish_import(value);
        }
        let run = self
            .frames
            .last_mut()
            .unwrap()
            .module_load
            .as_mut()
            .unwrap();
        run.phase = ModulePhase::Resolve;
        if run.internal_key && key.kind == ModuleKeyKind::SuppliedText {
            return self.resolved_module(Ok(ResolvedModuleKey {
                id: ResolvedKeyId(next_key_id()?),
                key: key.clone(),
                handle: "".into(),
                module: ResolvedModule {
                    original_uri: key.uri.clone(),
                    resolved_uri: key.uri.clone(),
                    source_uri: key.uri.clone(),
                },
            }));
        }
        let resolved_key_id = ResolvedKeyId(next_key_id()?);
        if run.purpose == ModulePurpose::Primary {
            return self.wait_module(ModuleRequestKind::ResolveKey {
                key,
                resolved_key_id,
                phase: crate::modules::ResolutionPhase::Primary,
            });
        }
        let importer = self.program.units[spec.unit.0 as usize]
            .identity
            .clone()
            .ok_or_else(|| invalid("import identity disappeared"))?;
        self.wait_module(ModuleRequestKind::Resolve {
            key,
            resolved_key_id,
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
        if let Some(site) = run.site {
            self.heap
                .publish_imported_module(site.0, (site.1, site.2), receiver)?;
        }
        if run.purpose == ModulePurpose::Primary {
            self.public_primary = Some(receiver);
        }
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
                    ModuleRequestKind::SelectKey { .. },
                    ModuleCompletion::KeySelected(_)
                ) | (
                    ModuleRequestKind::QueryCachePolicy { .. },
                    ModuleCompletion::CachePolicy(_)
                ) | (
                    ModuleRequestKind::QueryFragmentPaths { .. },
                    ModuleCompletion::FragmentPaths(_)
                ) | (
                    ModuleRequestKind::ResolveImportUri { .. },
                    ModuleCompletion::ImportUri(_)
                ) | (
                    ModuleRequestKind::Capabilities { .. },
                    ModuleCompletion::Capabilities(_)
                ) | (
                    ModuleRequestKind::HasElement { .. },
                    ModuleCompletion::HasElement(_)
                ) | (
                    ModuleRequestKind::Resolve { .. } | ModuleRequestKind::ResolveKey { .. },
                    ModuleCompletion::Resolved(_) | ModuleCompletion::KeyResolved(_)
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
            ModuleCompletion::ImportChecked(Err(crate::modules::ModuleHostError::NotFound
                | crate::modules::ModuleHostError::NoLoader
                | crate::modules::ModuleHostError::UriSyntax { .. }
                | crate::modules::ModuleHostError::ExternalReaderProcess { .. }
                | crate::modules::ModuleHostError::Io(_)))
        ) {
            return Err(invalid(
                "Import admission does not accept module lookup or I/O failures.",
            ));
        }
        use crate::modules::ModuleHostError as Failure;
        if matches!(
            &completion,
            ModuleCompletion::KeySelected(Err(
                Failure::NotFound | Failure::SecurityManagerException { .. }
            )) | ModuleCompletion::FragmentPaths(Err(Failure::NotFound
                | Failure::NoLoader
                | Failure::Io(_)
                | Failure::UriSyntax { .. }
                | Failure::ExternalReaderProcess { .. }
                | Failure::SecurityManagerException { .. }))
                | ModuleCompletion::CachePolicy(Err(Failure::NotFound
                    | Failure::NoLoader
                    | Failure::Io(_)
                    | Failure::UriSyntax { .. }
                    | Failure::ExternalReaderProcess { .. }
                    | Failure::SecurityManagerException { .. }))
                | ModuleCompletion::Resolved(Err(Failure::NoLoader))
                | ModuleCompletion::KeyResolved(Err(Failure::NoLoader))
                | ModuleCompletion::Source(Err(Failure::NoLoader
                    | Failure::UriSyntax { .. }
                    | Failure::ExternalReaderProcess { .. }))
                | ModuleCompletion::Capabilities(Err(Failure::NoLoader))
                | ModuleCompletion::HasElement(Err(Failure::NoLoader))
                | ModuleCompletion::ImportUri(Err(Failure::NoLoader | Failure::NotFound))
        ) {
            return Err(invalid(
                "Module Host failure does not belong to this phase.",
            ));
        }
        match (&request.kind, &completion) {
            (ModuleRequestKind::ResolveImportUri { .. }, ModuleCompletion::ImportUri(Ok(uri))) => {
                crate::host::resources::validate_module_key_uri(uri)?;
            }
            (ModuleRequestKind::SelectKey { id, .. }, ModuleCompletion::KeySelected(Ok(key))) => {
                if key.id != *id {
                    return Err(stale_module());
                }
                crate::host::resources::validate_module_key_uri(&key.uri)?;
                if key.kind == ModuleKeyKind::StandardLibrary
                    && crate::modules::stdlib_kind(&key.uri).is_none()
                {
                    return Err(invalid("StandardLibrary key requires the pkl scheme"));
                }
            }
            (
                ModuleRequestKind::Resolve {
                    key,
                    resolved_key_id,
                    ..
                }
                | ModuleRequestKind::ResolveKey {
                    key,
                    resolved_key_id,
                    ..
                },
                ModuleCompletion::KeyResolved(Ok(resolved)),
            ) => {
                if resolved.id != *resolved_key_id || resolved.key != *key {
                    return Err(stale_module());
                }
                validate_resolution(key, &resolved.module)?;
            }
            (
                ModuleRequestKind::Resolve { key, .. } | ModuleRequestKind::ResolveKey { key, .. },
                ModuleCompletion::Resolved(Ok(resolved)),
            ) => validate_resolution(key, resolved)?,
            _ => {}
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
        if self.frames.last().is_some_and(|frame| frame.glob.is_some()) {
            let result = self.complete_glob_module(completion);
            if let Err(error) = result {
                self.fail(error);
            }
            return Ok(());
        }
        let result = match completion {
            ModuleCompletion::ImportChecked(result) => self.checked_import(result),
            ModuleCompletion::KeySelected(result) => {
                if matches!(
                    &request.kind,
                    ModuleRequestKind::SelectKey {
                        phase: SelectionPhase::ClausePreparation,
                        ..
                    }
                ) {
                    self.selected_clause_key(result)
                } else {
                    self.selected_module_key(result)
                }
            }
            ModuleCompletion::FragmentPaths(result) => {
                if self
                    .frames
                    .last()
                    .and_then(|frame| frame.module_load.as_ref())
                    .is_some_and(|run| run.phase == ModulePhase::Name)
                {
                    self.resolved_module_name(result)
                } else {
                    self.resolved_name_fragment_paths(result)
                }
            }
            ModuleCompletion::ImportUri(result) => self.resolved_import_uri(result),
            ModuleCompletion::CachePolicy(result) => self.queried_module_cache_policy(result),
            ModuleCompletion::Capabilities(result) => self.resolved_capabilities(result),
            ModuleCompletion::HasElement(result) => self.resolved_element(result),
            ModuleCompletion::Resolved(result) => {
                let (ModuleRequestKind::Resolve {
                    key,
                    resolved_key_id,
                    ..
                }
                | ModuleRequestKind::ResolveKey {
                    key,
                    resolved_key_id,
                    ..
                }) = &request.kind
                else {
                    unreachable!()
                };
                let key = key.clone();
                let id = *resolved_key_id;
                self.resolved_module(result.map(|module| ResolvedModuleKey {
                    id,
                    key,
                    module,
                    handle: "".into(),
                }))
            }
            ModuleCompletion::KeyResolved(result) => self.resolved_module(result),
            ModuleCompletion::Source(result) => self.loaded_module(result),
        };
        if let Err(error) = result {
            self.fail(error);
        }
        Ok(())
    }

    fn resolved_module(
        &mut self,
        result: Result<ResolvedModuleKey, crate::modules::ModuleHostError>,
    ) -> Result<(), RuntimeError> {
        let run = self
            .frames
            .last_mut()
            .and_then(|frame| frame.module_load.as_mut())
            .ok_or_else(|| invalid("missing import continuation"))?;
        let resolved_key = result.map_err(|error| error.into_runtime(&run.spec.original_uri))?;
        let resolved = resolved_key.module.clone();
        if resolved.original_uri != run.spec.original_uri {
            return Err(invalid("module resolver changed original key"));
        }
        if run.purpose == ModulePurpose::Query {
            return self.compile_public_query();
        }
        if run.cached
            && let Some(cached) = self.modules_resolved.get(&resolved.resolved_uri).cloned()
        {
            let value = self.module_cached(cached)?;
            // A resolved-key hit does not seed the original alias cache.
            return self.finish_import(value);
        }
        if resolved_key.key.kind == ModuleKeyKind::StandardLibrary {
            return self.initialize_bundled(resolved);
        }
        // VmTyped exists before source I/O but is not cache-visible until that
        // source succeeds. A callback can independently load the same key.
        let receiver = self.heap.allocate(Object::default());
        run.receiver = Some(receiver);
        run.resolved = Some(resolved.clone());
        run.resolved_key = Some(resolved_key.clone());
        run.phase = ModulePhase::Source;
        if let Some(text) = run.supplied.clone() {
            return self.loaded_module(Ok(ModuleSource {
                text,
                inferred_name: "".into(),
            }));
        }
        self.wait_module(ModuleRequestKind::LoadSource {
            key: resolved_key,
            resolved,
        })
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
        let input = result.map_err(|error| error.into_runtime(&resolved.source_uri))?;
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
        });
        // An ImportNode's guest call frame supplies the first visible member
        // for its parser exception. Primary loading has no such guest frame.
        let frame = self.frames.last().expect("import continuation");
        let metadata = &self.program.metadata[frame.block];
        let import_member = (frame.module_load.as_ref().expect("module load").purpose
            == ModulePurpose::Import)
            .then(|| {
                PklString::from(
                    metadata
                        .source
                        .as_ref()
                        .map_or(&self.program.module_name, |source| &source.module_name),
                )
                .concat(frame.diagnostic_member.as_ref().unwrap_or(&metadata.member))
            });
        // VmLanguage infers the selected key's module name even when parsing
        // fails. Keep the original diagnostic while its name getter suspends.
        let parsed = crate::parser::parse_module_for_evaluation(&input.text)
            .map_err(|failure| Box::new((*failure, import_member)));
        if let Some(name) = parsed
            .as_ref()
            .ok()
            .and_then(|parsed| parsed.declaration.as_ref())
            .and_then(|d| d.name.as_ref())
        {
            Arc::make_mut(&mut source).module_name = PklString::join(name, ".");
            return self.finish_loaded_parse(parsed, source, receiver);
        }
        let run = self
            .frames
            .last_mut()
            .unwrap()
            .module_load
            .as_mut()
            .unwrap();
        let key = run
            .key
            .clone()
            .ok_or_else(|| invalid("missing loaded ModuleKey"))?;
        if !crate::host::resources::module_name_needs_fragment_paths(&key.uri) {
            Arc::make_mut(&mut source).module_name =
                crate::host::resources::infer_module_name(&key.uri, false)?;
            return self.finish_loaded_parse(parsed, source, receiver);
        }
        run.parsed = Some(parsed);
        run.source = Some(source);
        run.phase = ModulePhase::Name;
        self.wait_module(ModuleRequestKind::QueryFragmentPaths { key })
    }

    fn resolved_module_name(
        &mut self,
        result: Result<bool, crate::modules::ModuleHostError>,
    ) -> Result<(), RuntimeError> {
        let run = self
            .frames
            .last_mut()
            .and_then(|frame| frame.module_load.as_mut())
            .ok_or_else(|| invalid("missing module name continuation"))?;
        let key = run
            .key
            .as_ref()
            .ok_or_else(|| invalid("missing name ModuleKey"))?;
        let fragments = result.map_err(|error| error.into_runtime(&key.uri))?;
        let name = crate::host::resources::infer_module_name(&key.uri, fragments)?;
        let mut source = run
            .source
            .take()
            .ok_or_else(|| invalid("missing module name source"))?;
        Arc::make_mut(&mut source).module_name = name;
        let parsed = run
            .parsed
            .take()
            .ok_or_else(|| invalid("missing module name AST"))?;
        let receiver = run
            .receiver
            .ok_or_else(|| invalid("missing module receiver"))?;
        self.finish_loaded_parse(parsed, source, receiver)
    }

    fn finish_loaded_parse(
        &mut self,
        parsed: ParsedModule,
        source: Arc<RuntimeSource>,
        receiver: ObjectId,
    ) -> Result<(), RuntimeError> {
        match parsed {
            Ok(parsed) => self.compile_loaded_module(parsed, source, receiver),
            Err(error) => {
                let (failure, member) = *error;
                Err(self.module_parse_error(failure, source, member))
            }
        }
    }

    fn module_parse_error(
        &self,
        failure: crate::parser::ModuleParseFailure,
        source: Arc<RuntimeSource>,
        member: Option<PklString>,
    ) -> RuntimeError {
        if let Some(mut error) = failure.minimum_version_error(&source.module_name) {
            if error.kind == ErrorKind::Type
                && let Some(caller) =
                    self.frames.iter().rev().find(|frame| {
                        frame.module_load.as_ref().is_some_and(|run| {
                            run.initializing && run.purpose == ModulePurpose::Import
                        }) && matches!(
                            self.program.blocks[frame.block].get(frame.pc.saturating_sub(1)),
                            Some(Instruction::ImportModule(_))
                        )
                    })
            {
                // Initialization hides this ImportNode's ordinary callsite.
                // The checker explicitly supplies it, with no child parser
                // location; outer authored frames unwind on the existing path.
                let metadata = &self.program.metadata[caller.block];
                if let Some(span) = metadata.spans[caller.pc.saturating_sub(1)] {
                    error.frames.push(RuntimeFrame {
                        source: metadata.source.clone(),
                        span,
                        member: caller
                            .diagnostic_member
                            .clone()
                            .unwrap_or_else(|| metadata.member.clone()),
                        anonymous: false,
                    });
                }
            }
            return error;
        }
        let member = member.unwrap_or_else(|| source.module_name.clone());
        imported_diagnostic(failure.diagnostic, source, member)
    }

    fn compile_loaded_module(
        &mut self,
        parsed: crate::syntax::Module,
        source: Arc<RuntimeSource>,
        receiver: ObjectId,
    ) -> Result<(), RuntimeError> {
        let run = self
            .frames
            .last_mut()
            .and_then(|frame| frame.module_load.as_mut())
            .ok_or_else(|| invalid("missing module compilation continuation"))?;
        run.phase = ModulePhase::Initializer;
        let resolved = run
            .resolved
            .clone()
            .ok_or_else(|| invalid("missing resolved module"))?;
        let resolved_ast = crate::resolver::resolve_module_for_evaluation(&parsed);
        // The evaluator owns one append-only code arena. Clone only while an
        // external prepared-program snapshot shares it; subsequent modules
        // append in place. Failed compilation retracts only its uncommitted tail.
        let program = Arc::make_mut(&mut self.program);
        let checkpoint = ArenaCheckpoint::new(program);
        let arena = query_lifecycle::CodeArena::begin(program);
        let compiled = compile_unit_into(program, &resolved_ast, CompileOptions { fuse_constant_add: true }, self.power_assertions,
            Some(source.clone()), Some(ModuleIdentity { original_uri: resolved.original_uri, resolved_uri: resolved.resolved_uri }))
            .and_then(|unit| {
                if program.blocks.iter_from(checkpoint.blocks).flatten().any(|instruction|
                    matches!(instruction, Instruction::Constant(value) if value.heap_id().is_some())) {
                    Err(invalid("object constants cannot be stored in compiled code"))
                } else { Ok(unit) }
            });
        let unit = match compiled {
            Ok(unit) => unit,
            Err(mut error) => {
                checkpoint.rollback(program);
                if let Some(span) = error.span {
                    error.insert_frame(
                        0,
                        RuntimeFrame {
                            anonymous: false,
                            span,
                            source: Some(source),
                            member: PklString::default(),
                        },
                    );
                }
                return Err(error);
            }
        };
        program.units[unit.0 as usize].key = self
            .frames
            .last()
            .and_then(|frame| frame.module_load.as_ref())
            .and_then(|run| run.key.clone());
        program.units[unit.0 as usize].delegate_key = self
            .frames
            .last()
            .and_then(|frame| frame.module_load.as_ref())
            .and_then(|run| run.delegate_key.clone());
        let entry = program.units[unit.0 as usize].entry;
        let owner = self.heap.allocate_executable();
        self.code_arenas.push(arena.finish(program, owner, false));
        self.heap.get_mut(receiver)?.environment.code_owner = Some(owner);
        self.state = State::Running;
        self.push_module_initializer(entry, Some(receiver));
        self.frames.last_mut().unwrap().environment.code_owner = Some(owner);
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
            warning: None,
            method_call_site: None,
            pcf_render: None,
            collection_join: None,
            collection_transform: None,
            set_order: None,
            set_sequence: None,
            list_ops: None,
            string_transform: None,
            int_seq_run: None,
            lazy_collection_run: None,
            mapping_lookup: None,
            mapping_keys: None,
            reflection: None,
            conversion_run: None,
            collection_addition: None,
            function_owner: None,
            amendment_arguments: None,
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
            glob: None,
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
        let receiver = run
            .receiver
            .ok_or_else(|| invalid("missing initialized module"))?;
        if let Some(site) = run.site {
            self.heap
                .publish_imported_module(site.0, (site.1, site.2), receiver)?;
        }
        if run.purpose == ModulePurpose::Primary {
            self.public_primary = Some(receiver);
        }
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
        anonymous: false,
        span: diagnostic.span,
        source: Some(source.clone()),
        member: diagnostic
            .member
            .clone()
            .map_or_else(PklString::default, |suffix| {
                if suffix.encode_utf16().next() == Some(46) {
                    suffix.as_ref().clone()
                } else {
                    PklString::from("#").concat(&suffix)
                }
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
    units: CodeStorage<ModuleUnit>,
    module_name: PklString,
    module_template: Option<usize>,
    entry: usize,
    initial_members: Vec<Member>,
    declaration: usize,
    type_recipes: usize,
    aliases: BTreeSet<usize>,
    base_mixin_declaration: Option<usize>,
}
impl ArenaCheckpoint {
    pub(super) fn new(program: &mut Program) -> Self {
        program.seal_code_segment();
        Self {
            blocks: program.blocks.next_id(),
            metadata: program.metadata.next_id(),
            objects: program.objects.next_id(),
            units: program.units.clone(),
            module_name: program.module_name.clone(),
            module_template: program.module_template,
            entry: program.entry,
            initial_members: program.initial_module_members.clone(),
            declaration: program.next_declaration,
            type_recipes: program.type_recipes.next_id(),
            aliases: program.alias_definitions.keys().copied().collect(),
            base_mixin_declaration: program.base_mixin_declaration,
        }
    }
    pub(super) fn rollback(self, program: &mut Program) {
        program.blocks.rollback(self.blocks);
        program.metadata.rollback(self.metadata);
        program.objects.rollback(self.objects);
        program.units = self.units;
        program.module_name = self.module_name;
        program.module_template = self.module_template;
        program.entry = self.entry;
        program.initial_module_members = self.initial_members;
        program.next_declaration = self.declaration;
        program.type_recipes.rollback(self.type_recipes);
        program.base_mixin_declaration = self.base_mixin_declaration;
        program
            .alias_definitions
            .retain(|id, _| self.aliases.contains(id));
        program
            .constraint_origins
            .retain(|block, _| *block < self.blocks);
        program
            .declaration_sources
            .retain(|block, _| *block < self.blocks);
        program
            .reflection_annotations
            .retain(|_, block| *block < self.blocks);
    }
}

fn validate_resolution(key: &ModuleKey, resolved: &ResolvedModule) -> Result<(), RuntimeError> {
    if resolved.original_uri != key.uri {
        return Err(invalid("module resolver changed original key"));
    }
    crate::host::resources::validate_module_key_uri(&resolved.resolved_uri)?;
    if key.kind == ModuleKeyKind::StandardLibrary
        && (resolved.resolved_uri != key.uri || resolved.source_uri != key.uri)
    {
        return Err(invalid(
            "Bundled module resolution must retain its pinned key and source identity.",
        ));
    }
    Ok(())
}
