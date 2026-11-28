//! Shared glob continuation and lazy mapping members.
use super::*;
use crate::glob::{
    GlobCompletion, GlobHostError, GlobReader, GlobRequest, GlobRequestKind, ResourceReaderId,
};
use crate::heap::{CollectionKind, CollectionMember, CollectionMemberKind};
use crate::host::resources::glob as uri;
use crate::modules::{
    ModuleCompletion, ModuleHostError, ModuleIdentity, ModuleRequestKind, UnitId,
};

#[derive(Clone, Debug)]
pub struct Spec {
    pub import: Option<modules::ImportSpec>,
    pub unit: UnitId,
    pub member_block: usize,
}

#[derive(Clone, Debug)]
pub(crate) struct Payload {
    spec: Spec,
    entries: BTreeMap<PklString, PklString>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    ResolvePattern,
    SelectModule,
    SelectResource,
    Globbable,
    Hierarchical,
    FragmentPaths,
    TraversalFragmentPaths,
    ResolveBase,
    Traverse,
    HasElement,
    ListElements,
}

pub(super) struct Run {
    spec: Spec,
    authored: PklString,
    owner: ExecutableOwnerId,
    site: (usize, usize),
    phase: Phase,
    declaration: Option<crate::modules::ModuleKey>,
    module_uri: Option<PklString>,
    pattern_uri: PklString,
    target: Option<GlobReader>,
    base_path: PklString,
    parts: Vec<PklString>,
    traversal: Option<super::glob_traversal::Traversal>,
}

impl Run {
    pub(super) fn roots(&self, roots: &mut Vec<Value>) {
        roots.push(self.owner.root());
    }

    fn failure(&self, failure: GlobHostError) -> RuntimeError {
        match failure {
            GlobHostError::ReaderNotFound => RuntimeError::host_language(
                PklString::from("No resource reader is registered for scheme `")
                    .concat(&uri::scheme(&self.pattern_uri).unwrap_or_else(|| "null".into()))
                    .concat(&"`.".into()),
            ),
            GlobHostError::Failure(ModuleHostError::Io(cause)) => {
                let operand = if self.spec.import.is_some() {
                    &self.pattern_uri
                } else {
                    &self.authored
                };
                cause.into_runtime("resolving glob pattern", operand)
            }
            GlobHostError::Failure(other) => other.into_runtime(&self.pattern_uri),
        }
    }
}

pub(super) fn invalid_pattern(pattern: &PklString, hint: PklString) -> RuntimeError {
    let mut error = RuntimeError::new(
        ErrorKind::Type,
        PklString::from("Invalid glob pattern `")
            .concat(pattern)
            .concat(&"`.".into()),
    );
    error.hint = Some(hint);
    error
}

impl Vm {
    /// Explicit oracle test-mode profile. The production limit remains 16384.
    /// Profile changes require an idle evaluator and never alter other limits.
    pub fn set_glob_test_mode(&mut self, enabled: bool) -> Result<(), RuntimeError> {
        if matches!(self.state, State::Closed) {
            return Err(RuntimeError::new(ErrorKind::Closed, "evaluator is closed"));
        }
        if !self.frames.is_empty()
            || !self.suspended.is_empty()
            || matches!(self.state, State::Running | State::Waiting(_))
        {
            return Err(RuntimeError::new(
                ErrorKind::Busy,
                "evaluation is already active",
            ));
        }
        self.glob_list_threshold = if enabled { 512 } else { 16384 };
        Ok(())
    }

    pub(super) fn begin_glob(
        &mut self,
        spec: Spec,
        authored: PklString,
    ) -> Result<(), RuntimeError> {
        let owner = self.current_executable()?;
        let frame = self.frames.last().ok_or_else(|| invalid("glob frame"))?;
        let site = (frame.block, frame.pc - 1);
        if let Some(mapping) = self.heap.glob_mapping(owner, site, &authored)? {
            self.stack.push(Value::Object(mapping));
            return Ok(());
        }
        if authored.starts_with("...") {
            return Err(RuntimeError::new(
                ErrorKind::Type,
                "Cannot combine glob imports with triple-dot module URIs.",
            ));
        }
        let unit = self.program.units.get(spec.unit.0 as usize);
        let declaration = unit.and_then(|unit| unit.key.clone());
        let module_uri = unit
            .and_then(|unit| unit.identity.as_ref())
            .map(|identity| identity.original_uri.clone());
        let pattern_uri: PklString = if let Some(import) = &spec.import {
            import.absolute_uri.clone()
        } else {
            uri::parse_authored(&authored)?
        };
        let phase = if spec.import.is_some() {
            Phase::SelectModule
        } else {
            Phase::ResolvePattern
        };
        self.frames.last_mut().unwrap().glob = Some(Box::new(Run {
            spec,
            authored,
            owner,
            site,
            phase,
            declaration,
            module_uri,
            pattern_uri,
            target: None,
            base_path: PklString::default(),
            parts: Vec::new(),
            traversal: None,
        }));
        self.step_glob()
    }

    pub(super) fn wait_glob(&mut self, kind: GlobRequestKind) -> Result<(), RuntimeError> {
        if self.callback_closing {
            return Err(RuntimeError::new(ErrorKind::Closed, "evaluator is closing"));
        }
        let id = self.next_request;
        self.next_request = id
            .checked_add(1)
            .ok_or_else(|| invalid("Host request identity exhausted"))?;
        self.state = State::Waiting(PendingRequest::Glob(GlobRequest { id, kind }));
        Ok(())
    }

    pub(super) fn step_glob(&mut self) -> Result<(), RuntimeError> {
        let mut run = self
            .frames
            .last_mut()
            .and_then(|frame| frame.glob.take())
            .ok_or_else(|| invalid("missing glob continuation"))?;
        enum Effect {
            Glob(GlobRequestKind),
            Module(ModuleRequestKind),
            None,
        }
        let effect = match run.phase {
            Phase::ResolvePattern | Phase::ResolveBase => {
                let text = if run.phase == Phase::ResolvePattern {
                    run.pattern_uri.clone()
                } else {
                    uri::parse_base(&run.base_path)?
                };
                let dep_key = if let Some(key) = &run.declaration {
                    Some(key.clone())
                } else if text.starts_with("@") {
                    let uri = run.module_uri.clone().unwrap_or_default();
                    let kind = if uri.starts_with("package:") || uri.starts_with("projectpackage:")
                    {
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
                if let Some(key) = dep_key {
                    Effect::Module(ModuleRequestKind::ResolveImportUri {
                        key,
                        authored_uri: text,
                    })
                } else {
                    let resolved = if let Some(base) = &run.module_uri {
                        crate::host::resources::resolve_module_key_uri(&text, base)?
                    } else {
                        text.clone()
                    };
                    Self::glob_resolved_uri(&mut run, resolved, self.glob_list_threshold)?;
                    Effect::None
                }
            }
            Phase::SelectModule => Effect::Module(ModuleRequestKind::SelectKey {
                id: crate::modules::ModuleKeyId(modules::next_key_id()?),
                uri: run.pattern_uri.clone(),
                phase: crate::modules::SelectionPhase::RuntimeImport,
            }),
            Phase::SelectResource => Effect::Glob(GlobRequestKind::SelectResource {
                id: ResourceReaderId(modules::next_key_id()?),
                uri: run.pattern_uri.clone(),
            }),
            Phase::Globbable => Effect::Glob(GlobRequestKind::IsGlobbable {
                reader: run.target.clone().unwrap(),
            }),
            Phase::Hierarchical => Effect::Glob(GlobRequestKind::IsHierarchical {
                reader: run.target.clone().unwrap(),
            }),
            Phase::FragmentPaths | Phase::TraversalFragmentPaths => {
                match run.target.clone().unwrap() {
                    GlobReader::Module(key) => {
                        Effect::Module(ModuleRequestKind::QueryFragmentPaths { key })
                    }
                    GlobReader::Resource(reader) => {
                        Effect::Glob(GlobRequestKind::FragmentPaths { reader })
                    }
                }
            }
            Phase::Traverse => match run
                .traversal
                .as_mut()
                .ok_or_else(|| invalid("missing glob traversal"))?
                .poll()?
            {
                super::glob_traversal::Poll::Continue => Effect::None,
                super::glob_traversal::Poll::FragmentPaths => {
                    run.phase = Phase::TraversalFragmentPaths;
                    Effect::None
                }
                super::glob_traversal::Poll::HasElement(uri) => {
                    run.phase = Phase::HasElement;
                    match run.target.clone().unwrap() {
                        GlobReader::Module(key) => Effect::Module(ModuleRequestKind::HasElement {
                            module: ModuleIdentity {
                                original_uri: key.uri.clone(),
                                resolved_uri: key.uri.clone(),
                            },
                            key: Some(key),
                            element_uri: uri,
                        }),
                        GlobReader::Resource(reader) => {
                            Effect::Glob(GlobRequestKind::HasElement { reader, uri })
                        }
                    }
                }
                super::glob_traversal::Poll::ListElements(uri) => {
                    run.phase = Phase::ListElements;
                    Effect::Glob(GlobRequestKind::ListElements {
                        reader: run.target.clone().unwrap(),
                        uri,
                    })
                }
                super::glob_traversal::Poll::Done(elements) => {
                    let mapping = self.publish_glob_mapping(&run, elements)?;
                    self.heap
                        .publish_glob_mapping(run.owner, run.site, run.authored, mapping)?;
                    self.stack.push(Value::Object(mapping));
                    self.state = State::Running;
                    return Ok(());
                }
            },
            Phase::HasElement | Phase::ListElements => {
                return Err(invalid("glob stepped while awaiting an element result"));
            }
        };
        self.frames.last_mut().unwrap().glob = Some(run);
        match effect {
            Effect::Glob(request) => self.wait_glob(request),
            Effect::Module(request) => self.wait_module(request),
            Effect::None => {
                self.state = State::Running;
                Ok(())
            }
        }
    }

    fn glob_resolved_uri(
        run: &mut Run,
        resolved: PklString,
        threshold: u32,
    ) -> Result<(), RuntimeError> {
        if run.phase == Phase::ResolvePattern {
            run.pattern_uri = resolved;
            run.phase = Phase::SelectResource;
        } else {
            run.traversal = Some(super::glob_traversal::Traversal::hierarchical(
                run.authored.clone(),
                resolved,
                run.base_path.clone(),
                std::mem::take(&mut run.parts),
                None,
                threshold,
            )?);
            run.phase = Phase::Traverse;
        }
        Ok(())
    }

    fn glob_fragment_paths(
        run: &mut Run,
        fragments: bool,
        threshold: u32,
    ) -> Result<(), RuntimeError> {
        if run.phase == Phase::TraversalFragmentPaths {
            run.traversal
                .as_mut()
                .ok_or_else(|| invalid("glob URI continuation"))?
                .complete_fragments(fragments)?;
            run.phase = Phase::Traverse;
            return Ok(());
        }
        let (base, parts) = uri::split(&run.authored, fragments)?;
        run.base_path = base;
        run.parts = parts;
        if run.parts.is_empty() {
            let resolved = run.authored.clone();
            run.traversal = Some(super::glob_traversal::Traversal::hierarchical(
                run.authored.clone(),
                resolved,
                run.base_path.clone(),
                Vec::new(),
                run.module_uri.clone(),
                threshold,
            )?);
            run.phase = Phase::Traverse;
        } else {
            run.phase = Phase::ResolveBase;
        }
        Ok(())
    }

    pub(super) fn complete_glob_module(
        &mut self,
        completion: ModuleCompletion,
    ) -> Result<(), RuntimeError> {
        let mut run = self
            .frames
            .last_mut()
            .and_then(|frame| frame.glob.take())
            .ok_or_else(|| invalid("missing glob module continuation"))?;
        match completion {
            ModuleCompletion::KeySelected(result) if run.phase == Phase::SelectModule => {
                run.target = Some(GlobReader::Module(
                    result.map_err(|failure| run.failure(failure.into()))?,
                ));
                run.phase = Phase::Globbable;
            }
            ModuleCompletion::FragmentPaths(result)
                if matches!(
                    run.phase,
                    Phase::FragmentPaths | Phase::TraversalFragmentPaths
                ) =>
            {
                let value = result.map_err(|failure| run.failure(failure.into()))?;
                Self::glob_fragment_paths(&mut run, value, self.glob_list_threshold)?;
            }
            ModuleCompletion::ImportUri(result)
                if matches!(run.phase, Phase::ResolvePattern | Phase::ResolveBase) =>
            {
                let value = result.map_err(|failure| run.failure(failure.into()))?;
                Self::glob_resolved_uri(&mut run, value, self.glob_list_threshold)?;
            }
            ModuleCompletion::HasElement(result) if run.phase == Phase::HasElement => {
                let value = result.map_err(|failure| run.failure(failure.into()))?;
                run.traversal.as_mut().unwrap().complete_has(value)?;
                run.phase = Phase::Traverse;
            }
            _ => return Err(invalid("unexpected glob module completion")),
        }
        self.frames.last_mut().unwrap().glob = Some(run);
        self.state = State::Running;
        Ok(())
    }

    pub fn complete_glob(
        &mut self,
        id: u64,
        completion: GlobCompletion,
    ) -> Result<(), RuntimeError> {
        let State::Waiting(PendingRequest::Glob(request)) = &self.state else {
            return Err(RuntimeError::new(ErrorKind::Host, "stale glob completion"));
        };
        if request.id != id
            || !matches!(
                (&request.kind, &completion),
                (
                    GlobRequestKind::SelectResource { .. },
                    GlobCompletion::Resource(_)
                ) | (
                    GlobRequestKind::IsGlobbable { .. },
                    GlobCompletion::Globbable(_)
                ) | (
                    GlobRequestKind::IsHierarchical { .. },
                    GlobCompletion::Hierarchical(_)
                ) | (
                    GlobRequestKind::FragmentPaths { .. },
                    GlobCompletion::FragmentPaths(_)
                ) | (
                    GlobRequestKind::HasElement { .. },
                    GlobCompletion::HasElement(_)
                ) | (
                    GlobRequestKind::ListElements { .. },
                    GlobCompletion::Elements(_)
                )
            )
        {
            return Err(RuntimeError::new(ErrorKind::Host, "stale glob completion"));
        }
        if let (GlobRequestKind::SelectResource { id, uri }, GlobCompletion::Resource(Ok(reader))) =
            (&request.kind, &completion)
            && (reader.id != *id || Some(reader.scheme.clone()) != uri::scheme(uri))
        {
            return Err(RuntimeError::new(
                ErrorKind::Host,
                "glob reader identity mismatch",
            ));
        }
        // Phase-specific failure admission is performed before taking the run.
        let failure = match &completion {
            GlobCompletion::Resource(Err(error))
            | GlobCompletion::Globbable(Err(error))
            | GlobCompletion::Hierarchical(Err(error))
            | GlobCompletion::FragmentPaths(Err(error))
            | GlobCompletion::HasElement(Err(error))
            | GlobCompletion::Elements(Err(error)) => Some(error),
            _ => None,
        };
        if let Some(failure) = failure {
            let valid = match failure {
                GlobHostError::ReaderNotFound => {
                    matches!(request.kind, GlobRequestKind::SelectResource { .. })
                }
                GlobHostError::Failure(
                    ModuleHostError::Unsupported(_) | ModuleHostError::Denied(_),
                ) => true,
                GlobHostError::Failure(
                    ModuleHostError::Io(_) | ModuleHostError::ExternalReaderProcess { .. },
                ) => matches!(
                    request.kind,
                    GlobRequestKind::IsGlobbable { .. }
                        | GlobRequestKind::IsHierarchical { .. }
                        | GlobRequestKind::HasElement { .. }
                        | GlobRequestKind::ListElements { .. }
                ),
                GlobHostError::Failure(ModuleHostError::SecurityManagerException { .. }) => {
                    matches!(
                        request.kind,
                        GlobRequestKind::HasElement { .. } | GlobRequestKind::ListElements { .. }
                    )
                }
                _ => false,
            };
            if !valid {
                return Err(RuntimeError::new(
                    ErrorKind::Host,
                    "glob failure does not belong to this phase",
                ));
            }
        }
        let result = self.accept_glob_completion(completion);
        if let Err(error) = result {
            self.fail(error);
        }
        Ok(())
    }

    fn accept_glob_completion(&mut self, completion: GlobCompletion) -> Result<(), RuntimeError> {
        let mut run = self
            .frames
            .last_mut()
            .and_then(|frame| frame.glob.take())
            .ok_or_else(|| invalid("missing glob completion continuation"))?;
        match completion {
            GlobCompletion::Resource(result) => {
                run.target = Some(GlobReader::Resource(
                    result.map_err(|failure| run.failure(failure))?,
                ));
                run.phase = Phase::Globbable;
            }
            GlobCompletion::Globbable(result) => {
                if !result.map_err(|failure| run.failure(failure))? {
                    let pattern_uri = if run.spec.import.is_some() {
                        run.pattern_uri.clone()
                    } else {
                        uri::parse_authored(&run.authored)?
                    };
                    let scheme = uri::scheme(&pattern_uri).unwrap_or_else(|| "null".into());
                    return Err(RuntimeError::new(
                        ErrorKind::Type,
                        PklString::from("Cannot expand glob pattern `")
                            .concat(&pattern_uri)
                            .concat(&"` because scheme `".into())
                            .concat(&scheme)
                            .concat(&"` is not globbable.".into()),
                    ));
                }
                run.phase = Phase::Hierarchical;
            }
            GlobCompletion::Hierarchical(result) => {
                if result.map_err(|failure| run.failure(failure))? {
                    if uri::absolute_pattern(&run.authored) {
                        run.phase = Phase::FragmentPaths;
                    } else {
                        // The split operation does not consult target traits
                        // for a relative pattern. Resolve the base first.
                        Self::glob_fragment_paths(&mut run, false, self.glob_list_threshold)?;
                    }
                } else {
                    let list_uri = if uri::absolute_pattern(&run.authored) {
                        run.authored.clone()
                    } else {
                        run.module_uri
                            .as_ref()
                            .and_then(uri::scheme)
                            .unwrap_or_default()
                            .concat(&":dummy".into())
                    };
                    run.traversal = Some(super::glob_traversal::Traversal::opaque(
                        run.authored.clone(),
                        list_uri,
                    )?);
                    run.phase = Phase::Traverse;
                }
            }
            GlobCompletion::FragmentPaths(result) => {
                let fragments = result.map_err(|failure| run.failure(failure))?;
                Self::glob_fragment_paths(&mut run, fragments, self.glob_list_threshold)?;
            }
            GlobCompletion::HasElement(result) => {
                let present = result.map_err(|failure| run.failure(failure))?;
                run.traversal.as_mut().unwrap().complete_has(present)?;
                run.phase = Phase::Traverse;
            }
            GlobCompletion::Elements(result) => {
                let elements = result.map_err(|failure| run.failure(failure))?;
                run.traversal.as_mut().unwrap().complete_list(elements)?;
                run.phase = Phase::Traverse;
            }
        }
        self.frames.last_mut().unwrap().glob = Some(run);
        self.state = State::Running;
        Ok(())
    }

    fn publish_glob_mapping(
        &mut self,
        run: &Run,
        elements: Vec<super::glob_traversal::Element>,
    ) -> Result<ObjectId, RuntimeError> {
        let class = self.canonical_builtin_class(classes::BuiltinClass::Mapping);
        let parent = self.heap.get_class(class)?.prototype;
        // VmObjectBuilder.toMapping uses an empty lexical frame. Only code
        // ownership survives here; caller locals/this/scopes must not leak into
        // a cached glob mapping or prolong an unrelated activation's lifetime.
        let environment = Environment {
            executable_owner: Some(run.owner),
            code_owner: self.frames.last().unwrap().environment.code_owner,
            ..Environment::default()
        };
        let mut object = Object {
            parent: Some(parent),
            collection_kind: CollectionKind::Mapping,
            environment,
            ..Object::default()
        };
        let mut entries = BTreeMap::new();
        for (index, element) in elements.into_iter().enumerate() {
            let name: PklString = format!("\0glob:{index}").into();
            // Java ResolvedGlobElement keeps the raw Mapping key separately
            // and normalizes the URI used by lazy reads and imports.
            entries.insert(name.clone(), uri::normalize(&element.uri)?);
            object.collection_members.push(CollectionMember {
                key: Value::String(element.path),
                name: name.clone(),
                kind: CollectionMemberKind::Entry,
            });
            object.members.push(Member {
                flags: 0,
                entry: true,
                name,
                code: run.spec.member_block,
                local: false,
                method: false,
                ty: None,
                type_span: None,
                constant: false,
                fixed: false,
            });
        }
        object.glob = Some(Arc::new(Payload {
            spec: run.spec.clone(),
            entries,
        }));
        Ok(self.heap.allocate(object))
    }

    #[cfg_attr(target_arch = "wasm32", inline(never))]
    pub(super) fn load_glob_member(&mut self) -> Result<(), RuntimeError> {
        let frame = self
            .frames
            .last()
            .ok_or_else(|| invalid("glob member frame"))?;
        let force = frame
            .force
            .as_ref()
            .ok_or_else(|| invalid("glob member owner"))?;
        let payload = self
            .heap
            .get(force.owner)?
            .glob
            .clone()
            .ok_or_else(|| invalid("missing glob mapping payload"))?;
        let uri = payload
            .entries
            .get(&force.name)
            .cloned()
            .ok_or_else(|| invalid("missing glob mapping entry"))?;
        if payload.spec.import.is_some() {
            let span = self.program.metadata[frame.block].spans[0]
                .ok_or_else(|| invalid("glob member source span"))?;
            self.begin_glob_member_import(modules::ImportSpec {
                authored: uri.clone(),
                span,
                unit: payload.spec.unit,
                absolute_uri: uri.clone(),
                original_uri: uri,
            })
        } else {
            let id = self.next_request;
            self.next_request = id
                .checked_add(1)
                .ok_or_else(|| invalid("Host request identity exhausted"))?;
            self.publish_read(ReadRequest {
                id,
                uri: uri.clone(),
                module_uri: None,
                resolved_uri: Some(uri::normalize(&uri)?),
                nullable: false,
            })
        }
    }
}

impl Compiler<'_> {
    pub(super) fn glob_spec(
        &mut self,
        expression: &crate::syntax::Expr,
        import: Option<modules::ImportSpec>,
    ) -> Result<Spec, RuntimeError> {
        let member_block = self.program.alloc_block(
            vec![Instruction::GlobMember, Instruction::Return],
            BlockMetadata {
                unit: Some(self.unit),
                source: self.source.clone(),
                member: PklString::default(),
                anonymous_root: true,
                declaration: Some(expression.span),
                spans: vec![Some(expression.span), Some(expression.span)],
                ..Default::default()
            },
        )?;
        Ok(Spec {
            import,
            unit: self.unit,
            member_block,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glob::ResourceReader;
    use crate::modules::ModuleKeyKind;

    #[test]
    fn primary_module_read_glob_dependency_requests_import_uri_resolution() {
        let source = "x = read*(\"@birds/catalog/*.pkl\")\n";
        let parsed = crate::parser::parse_module(source).unwrap();
        let program = compile_module(&crate::resolver::resolve_module(&parsed).unwrap()).unwrap();
        let mut vm = Vm::with_context(
            program.clone(),
            EvaluationContext {
                primary_source: Some(Arc::new(crate::value::RuntimeSource {
                    text: parsed.source.clone(),
                    uri: "file:///project/main.pkl".into(),
                    module_name: "main".into(),
                })),
                ..Default::default()
            },
        );
        vm.start(program.entry, vec![]).unwrap();
        let module = loop {
            match vm.poll(100) {
                VmPoll::Ready(result) => break result.unwrap().as_object().unwrap(),
                VmPoll::Yielded => {}
                other => panic!("unexpected poll initializing module: {other:?}"),
            }
        };
        vm.start_member(module, "x").unwrap();
        let mut resolve_calls = Vec::new();
        let mut list_uris = Vec::new();
        let value = loop {
            match vm.poll(100) {
                VmPoll::Ready(result) => break result.unwrap(),
                VmPoll::Yielded => {}
                VmPoll::Module(req) => match req.kind {
                    ModuleRequestKind::ResolveImportUri { key, authored_uri } => {
                        resolve_calls.push((key.uri.clone(), key.kind, authored_uri.clone()));
                        let resolved = if authored_uri == "@birds/catalog/*.pkl" {
                            "package://localhost:0/birds@0.5.0#/catalog/*.pkl"
                        } else if authored_uri == "@birds/catalog/" {
                            "package://localhost:0/birds@0.5.0#/catalog/"
                        } else {
                            panic!("unexpected authored_uri: {authored_uri:?}");
                        };
                        vm.complete_module(
                            req.id,
                            ModuleCompletion::ImportUri(Ok(resolved.into())),
                        )
                        .unwrap();
                    }
                    other => panic!("unexpected module request: {other:?}"),
                },
                VmPoll::Glob(req) => match req.kind {
                    GlobRequestKind::SelectResource { id, uri } => {
                        assert_eq!(uri, "package://localhost:0/birds@0.5.0#/catalog/*.pkl");
                        vm.complete_glob(
                            req.id,
                            GlobCompletion::Resource(Ok(ResourceReader {
                                id,
                                scheme: "package".into(),
                                handle: PklString::default(),
                            })),
                        )
                        .unwrap();
                    }
                    GlobRequestKind::IsGlobbable { .. } => {
                        vm.complete_glob(req.id, GlobCompletion::Globbable(Ok(true)))
                            .unwrap();
                    }
                    GlobRequestKind::IsHierarchical { .. } => {
                        vm.complete_glob(req.id, GlobCompletion::Hierarchical(Ok(true)))
                            .unwrap();
                    }
                    GlobRequestKind::FragmentPaths { .. } => {
                        vm.complete_glob(req.id, GlobCompletion::FragmentPaths(Ok(true)))
                            .unwrap();
                    }
                    GlobRequestKind::ListElements { uri, .. } => {
                        list_uris.push(uri);
                        vm.complete_glob(req.id, GlobCompletion::Elements(Ok(Vec::new())))
                            .unwrap();
                    }
                    other => panic!("unexpected glob request: {other:?}"),
                },
                other => panic!("unexpected poll event: {other:?}"),
            }
        };
        assert!(matches!(value, Value::Object(_)));
        assert_eq!(
            resolve_calls,
            vec![
                (
                    "file:///project/main.pkl".into(),
                    ModuleKeyKind::StandardFile,
                    "@birds/catalog/*.pkl".into(),
                ),
                (
                    "file:///project/main.pkl".into(),
                    ModuleKeyKind::StandardFile,
                    "@birds/catalog/".into(),
                ),
            ]
        );
        assert_eq!(
            list_uris,
            vec![PklString::from(
                "package://localhost:0/birds@0.5.0#/catalog/"
            )]
        );
    }
}
