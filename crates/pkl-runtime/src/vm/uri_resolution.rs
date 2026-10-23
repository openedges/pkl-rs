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
}
#[derive(Clone, Debug)]
pub(super) enum PreparationStep {
    Import {
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
            .program
            .units
            .iter()
            .filter_map(|unit| unit.identity.as_ref())
            .find(|identity| identity.original_uri == uri)
            .cloned()
            .unwrap_or(ModuleIdentity {
                original_uri: uri.clone(),
                resolved_uri: uri,
            });
        self.start_uri_resolution(Resolution {
            module,
            path,
            candidates: VecDeque::new(),
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
                PreparationStep::Import { uri, span, member } => {
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
                    }));
                }
                PreparationStep::Failure(diagnostic) => {
                    let source = declaration
                        .source
                        .clone()
                        .or_else(|| self.context.primary_source.clone())
                        .ok_or_else(|| invalid("AST diagnostic without source"))?;
                    let member =
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
                                        PklString::from(
                                            metadata.source.as_ref().map_or(
                                                self.program.module_name.as_ref(),
                                                |source| &source.module_name,
                                            ),
                                        )
                                        .concat(
                                            caller
                                                .diagnostic_member
                                                .as_ref()
                                                .unwrap_or(&metadata.member),
                                        )
                                    })
                            })
                            .unwrap_or_else(|| {
                                PklString::from(source.module_name.as_ref()).concat(
                                    &diagnostic
                                        .member
                                        .as_ref()
                                        .map_or_else(PklString::default, |suffix| {
                                            PklString::from("#").concat(suffix)
                                        }),
                                )
                            });
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
                    let (absolute, original) = crate::host::resources::prepare_module_uri(
                        &current.spec.authored,
                        module.as_ref().map(|m| m.original_uri.as_ref()),
                    )
                    .map_err(|mut error| {
                        error.frames.insert(0, current.origin.clone());
                        error.span = Some(current.spec.span);
                        error
                    })?;
                    current.spec.absolute_uri = absolute.clone().into();
                    current.spec.original_uri = original.clone().into();
                    let Some(path) =
                        crate::host::resources::import_triple_path(&current.spec.authored)?
                    else {
                        for code in &mut Arc::make_mut(&mut self.program).blocks {
                            for instruction in code {
                                if let Instruction::ImportModule(spec) = instruction
                                    && spec.unit == current.spec.unit
                                    && spec.span == current.spec.span
                                {
                                    spec.absolute_uri = absolute.clone().into();
                                    spec.original_uri = original.clone().into();
                                }
                            }
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
                        target: Target::Import { current, remaining },
                    });
                }
            }
        }
        self.state = State::Running;
        Ok(())
    }

    fn start_uri_resolution(&mut self, run: Resolution) -> Result<(), RuntimeError> {
        let request = ModuleRequestKind::Capabilities {
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
        let caps = result.map_err(|error| run.host_failure(error))?;
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
                for code in &mut Arc::make_mut(&mut self.program).blocks {
                    for instruction in code {
                        if let Instruction::ImportModule(spec) = instruction
                            && spec.unit == current.spec.unit
                            && spec.span == current.spec.span
                        {
                            spec.absolute_uri = absolute.clone().into();
                            spec.original_uri = original.clone().into();
                        }
                    }
                }
                self.next_import_preparation(Some(run.module), remaining)
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
            crate::resolver::AstPreparation::Import { uri, span, member } => {
                PreparationStep::Import {
                    uri: uri.clone(),
                    span: *span,
                    member: member.clone(),
                }
            }
        })
        .collect::<Vec<_>>();
    if let Some(error) = &resolved.preparation_failure {
        steps.push(PreparationStep::Failure(error.clone()));
    }
    steps
}
