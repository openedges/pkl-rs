//! Development-only parsed-source Host session. Entries are fixed before execution;
//! this is not a general REPL. Module I/O is supplied through typed Host effects.
use super::frontend::{
    DiagnosticProfile, FailureDisposition, FrontendPhase, LosslessModuleSourceInput,
    ModuleEvalOptions, ModuleFrontendFailure, ModuleFrontendResult, prepare_module_text,
    render_module_runtime_failure_text,
};
use super::{EvaluationOptions, compile_program_text, runtime_error_text};
use crate::{
    heap::HeapStats,
    host::{HostPoll, HostSession},
    logging::{EvaluationContext, LogDelivery, LoggerFailure, TraceMode},
    modules::{ModuleCompletion, ModuleIdentity},
    source::{Source, SourceMap, SourceText, Span},
    value::{ErrorKind, PklString, RuntimeError, RuntimeSource, Value},
    vm::{BlockMetadata, Instruction, Program},
};
use std::sync::Arc;

#[derive(Clone, Copy, Debug, Default)]
pub struct SourceSessionOptions {
    pub evaluation: EvaluationOptions,
    pub trace_mode: TraceMode,
    pub log_delivery: LogDelivery,
    pub frame_transformer: crate::logging::FrameTransformer,
}

#[derive(Clone, Copy)]
enum Call {
    Module,
    Path(usize),
    InitialPath(usize),
    OutputBytes,
    InitialOutputBytes,
    OutputText,
    InitialOutputText,
}

impl Call {
    fn expression(self) -> Option<usize> {
        match self {
            Self::Path(index) | Self::InitialPath(index) => Some(index),
            _ => None,
        }
    }
}

/// Owns one actual compiled source, module receiver, heap and Host context stack.
/// Raw heap handles stay inside the runtime or native caller's explicit roots.
pub struct SourceSession {
    host: HostSession,
    public_lifecycle: bool,
    public_queries: Vec<Option<usize>>,
    source: Source,
    uri: PklString,
    name: PklString,
    paths: Vec<SourceText>,
    entries: Vec<usize>,
    output_text_entries: Vec<bool>,
    output_entry: Option<usize>,
    output_profile: DiagnosticProfile,
    module_entry: usize,
    module: Option<Value>,
    module_started: bool,
    setup_failure: Option<RuntimeError>,
    initialization_failed: bool,
    calls: Vec<Call>,
    last_expression: Option<usize>,
}

impl SourceSession {
    /// Configure an evaluator without parsing a module, selecting a key, or doing I/O.
    pub fn module_evaluator(options: SourceSessionOptions) -> Self {
        let mut sources = SourceMap::default();
        let id = sources.add("repl:text", "");
        let source = sources.get(id).unwrap().clone();
        let program = Arc::new(Program::new(vec![vec![Instruction::Return]]));
        let mut host = HostSession::with_context(
            program,
            EvaluationContext {
                primary_source: None,
                trace_mode: options.trace_mode,
                log_delivery: options.log_delivery,
                frame_transformer: options.frame_transformer,
            },
        );
        host.set_power_assertions(options.evaluation.power_assertions);
        Self {
            host,
            public_lifecycle: true,
            public_queries: Vec::new(),
            source,
            uri: "repl:text".into(),
            name: "".into(),
            paths: Vec::new(),
            entries: Vec::new(),
            output_text_entries: Vec::new(),
            output_entry: None,
            output_profile: DiagnosticProfile::ApiIdentity,
            module_entry: 0,
            module: None,
            module_started: false,
            setup_failure: None,
            initialization_failed: false,
            calls: Vec::new(),
            last_expression: None,
        }
    }

    pub fn start_evaluation(
        &mut self,
        source: crate::modules::ModuleSourceSpec,
        expression: impl Into<SourceText>,
        now_ms: u64,
        timeout_ms: Option<u64>,
    ) -> Result<(), RuntimeError> {
        let expression = expression.into();
        self.open()?;
        if !self.public_lifecycle {
            return Err(error(
                "ModuleSource operations require the public lifecycle profile",
            ));
        }
        let entry = self
            .host
            .append_public_operation(source.clone(), expression.clone())?;
        self.host.start(entry, Vec::new(), now_ms, timeout_ms)?;
        self.retain_active_query_text();
        let mut sources = SourceMap::default();
        let id = sources.add(
            source.uri.clone(),
            source.contents.clone().unwrap_or_else(|| "".into()),
        );
        self.source = sources.get(id).unwrap().clone();
        self.uri = source.uri.clone();
        self.name = PklString::default();
        let index = self.paths.len();
        self.paths.push(expression.clone());
        self.calls.push(if expression.original() == "output.text" {
            Call::OutputText
        } else {
            Call::Path(index)
        });
        self.public_queries.push(None);
        self.last_expression = None;
        Ok(())
    }

    pub fn reenter_evaluation(
        &mut self,
        token: &str,
        source: crate::modules::ModuleSourceSpec,
        expression: impl Into<SourceText>,
        now: u64,
        timeout: Option<u64>,
    ) -> Result<(), RuntimeError> {
        let expression = expression.into();
        if !self.public_lifecycle {
            return Err(error(
                "ModuleSource operations require the public lifecycle profile",
            ));
        }
        self.host
            .reenter_public_operation(token, source, expression.clone(), now, timeout)?;
        self.retain_active_query_text();
        let index = self.paths.len();
        self.paths.push(expression.clone());
        self.calls.push(if expression.original() == "output.text" {
            Call::OutputText
        } else {
            Call::Path(index)
        });
        self.public_queries.push(None);
        self.last_expression = None;
        Ok(())
    }
    fn retain_active_query_text(&mut self) {
        // A completed diagnostic remains available until the next admitted
        // operation. Suspended calls keep their own source text for reentry.
        let retained = self
            .calls
            .iter()
            .filter_map(|call| call.expression())
            .max()
            .map_or(0, |index| index + 1);
        self.paths.truncate(retained);
    }
    /// Legacy constructor encoding diagnostic text at the Java UTF-8 boundary.
    pub fn compile(
        text: &str,
        uri: &str,
        module_name: &str,
        property_paths: &[String],
        options: SourceSessionOptions,
    ) -> Result<Self, String> {
        Self::compile_text(text, uri, module_name, property_paths, options)
            .map_err(|text| text.to_java_utf8())
    }

    pub fn compile_text(
        text: impl Into<SourceText>,
        uri: impl Into<PklString>,
        module_name: impl Into<PklString>,
        property_paths: &[String],
        options: SourceSessionOptions,
    ) -> Result<Self, PklString> {
        let text = text.into();
        let uri = uri.into();
        let module_name = module_name.into();
        let mut sources = SourceMap::default();
        let id = sources.add(uri.clone(), text.clone());
        let source = sources.get(id).expect("inserted source").clone();
        let (program, name) = compile_program_text(
            &source,
            &uri,
            &module_name,
            property_paths.first().map_or("", String::as_str),
            options.evaluation,
        )?;
        Self::from_program(
            program,
            source,
            uri,
            name,
            &property_paths
                .iter()
                .map(SourceText::from)
                .collect::<Vec<_>>(),
            options,
            None,
            false,
        )
    }

    /// Compile fixed ordinary expressions in synthetic `repl:text` scopes.
    /// The actual module remains their receiver; this is not a mutable REPL.
    pub fn compile_expressions(
        text: &str,
        uri: &str,
        module_name: &str,
        expressions: &[String],
        options: SourceSessionOptions,
    ) -> Result<Self, String> {
        Self::compile_expressions_text(text, uri, module_name, expressions, options)
            .map_err(|text| text.to_java_utf8())
    }

    pub fn compile_expressions_text(
        text: impl Into<SourceText>,
        uri: impl Into<PklString>,
        module_name: impl Into<PklString>,
        expressions: &[String],
        options: SourceSessionOptions,
    ) -> Result<Self, PklString> {
        let text = text.into();
        let uri = uri.into();
        let module_name = module_name.into();
        let mut sources = SourceMap::default();
        let id = sources.add(uri.clone(), text.clone());
        let source = sources.get(id).expect("inserted source").clone();
        let (program, name) =
            compile_program_text(&source, &uri, &module_name, "", options.evaluation)?;
        Self::from_program(
            program,
            source,
            uri,
            name,
            &expressions.iter().map(SourceText::from).collect::<Vec<_>>(),
            options,
            None,
            true,
        )
    }

    pub fn compile_expressions_source(
        text: impl Into<SourceText>,
        uri: impl Into<PklString>,
        module_name: impl Into<PklString>,
        expressions: &[SourceText],
        options: SourceSessionOptions,
    ) -> Result<Self, PklString> {
        let text = text.into();
        let uri = uri.into();
        let module_name = module_name.into();
        let mut sources = SourceMap::default();
        let id = sources.add(uri.clone(), text);
        let source = sources.get(id).expect("inserted source").clone();
        let (program, name) =
            compile_program_text(&source, &uri, &module_name, "", options.evaluation)?;
        Self::from_program(program, source, uri, name, expressions, options, None, true)
    }

    /// Prepare real module output and optional callback property paths on one
    /// Program, receiver and Host. Output does not use expression-string ToString.
    pub fn compile_output(
        input: impl Into<LosslessModuleSourceInput>,
        property_paths: &[String],
        module_options: &ModuleEvalOptions,
        options: SourceSessionOptions,
    ) -> Result<Self, ModuleFrontendResult> {
        Self::compile_output_text(input, property_paths, module_options, options)
            .map_err(Into::into)
    }

    /// Canonical output preparation preserves diagnostic text until its caller
    /// chooses a byte-oriented result or an API UTF-16 transport.
    pub fn compile_output_text(
        input: impl Into<LosslessModuleSourceInput>,
        property_paths: &[String],
        module_options: &ModuleEvalOptions,
        options: SourceSessionOptions,
    ) -> Result<Self, ModuleFrontendFailure> {
        let input = input.into();
        let prepared = prepare_module_text(input.clone(), module_options)?;
        Self::from_program(
            prepared.program,
            prepared.source,
            input.uri,
            prepared.name,
            &property_paths
                .iter()
                .map(SourceText::from)
                .collect::<Vec<_>>(),
            options,
            Some(module_options.diagnostic_profile),
            false,
        )
        .map_err(|message| ModuleFrontendFailure {
            phase: FrontendPhase::Configuration,
            disposition: FailureDisposition::Unsupported,
            reason: Some("source-session-callback-expression"),
            text: crate::diagnostic::render_error_text(&message, &[], None),
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn from_program(
        mut program: Arc<Program>,
        source: Source,
        uri: impl Into<PklString>,
        name: PklString,
        property_paths: &[SourceText],
        options: SourceSessionOptions,
        output_profile: Option<DiagnosticProfile>,
        parsed_expressions: bool,
    ) -> Result<Self, PklString> {
        let uri = uri.into();

        let module_entry = program.entry;
        let program_mut = Arc::make_mut(&mut program);
        let expression_owner = if parsed_expressions {
            Some(
                crate::parser::parse_module_for_evaluation(source.text()).map_err(|failure| {
                    failure
                        .minimum_version_error(&name)
                        .map_or(failure.diagnostic.message, |error| error.message)
                })?,
            )
        } else {
            None
        };
        let mut entries = Vec::with_capacity(property_paths.len());
        let mut output_text_entries = Vec::with_capacity(property_paths.len());
        for expression in property_paths {
            // EvaluatorImpl.evaluateExpressionString routes this exact API
            // expression to evaluateOutputText before creating a REPL root.
            let output_text = expression.original() == "output.text";
            output_text_entries.push(output_text);
            if output_text {
                entries
                    .push(append_output_entry(program_mut, "text").map_err(|error| error.message)?);
                continue;
            }
            if parsed_expressions {
                let entry = match crate::vm::compile_session_expression(
                    program_mut,
                    expression_owner.as_ref().expect("parsed source owner"),
                    expression,
                    crate::vm::CompileOptions {
                        fuse_constant_add: true,
                    },
                    options.evaluation.power_assertions,
                ) {
                    Ok(entry) => entry,
                    Err(failure) => {
                        // Java initializes the actual module before parsing this
                        // call's expression. A rejected fixed entry must not
                        // prevent earlier calls or move ahead of setup failure.
                        let source = Arc::new(RuntimeSource {
                            text: expression.clone(),
                            uri: "repl:text".into(),
                            module_name: "".into(),
                        });
                        let mut error = match failure {
                            crate::vm::SessionExpressionError::Frontend(diagnostic) => {
                                let mut error =
                                    RuntimeError::new(ErrorKind::Type, diagnostic.message.clone());
                                error.span = Some(diagnostic.span);
                                error.hint = diagnostic.hint().cloned();
                                error.frontend = Some(Arc::new(crate::modules::ModuleDiagnostic {
                                    diagnostic: *diagnostic,
                                    source: source.clone(),
                                    member: "repl:text".into(),
                                }));
                                error
                            }
                            crate::vm::SessionExpressionError::Runtime(error) => *error,
                        };
                        if error.frames.is_empty() {
                            error.frames.push(crate::value::RuntimeFrame {
                                anonymous: false,
                                source: Some(source.clone()),
                                span: error.span.unwrap_or(Span::new(0, expression.len_bytes())),
                                member: "repl:text".into(),
                            });
                        }
                        program_mut
                            .alloc_block(
                                vec![Instruction::Fail(error)],
                                BlockMetadata {
                                    source: Some(source),
                                    spans: vec![None],
                                    module_receiver_input: Some(0),
                                    ..Default::default()
                                },
                            )
                            .map_err(|error| error.message)?
                    }
                };
                entries.push(entry);
                continue;
            }
            let mut block = Vec::new();
            let mut spans = Vec::new();
            let mut prefix_end = 0;
            for (index, member) in expression.entire().split(".").enumerate() {
                if member.is_empty()
                    || !member.characters().all(|(_, ch)| {
                        ch.scalar()
                            .is_some_and(|ch| ch.is_alphanumeric() || ch == '_')
                    })
                {
                    return Err(
                        "the M2 source session currently accepts property paths only".into(),
                    );
                }
                prefix_end += usize::from(prefix_end > 0) + member.len_bytes();
                block.push(if index == 0 {
                    Instruction::GetImplicitProperty(member.to_pkl_string())
                } else {
                    Instruction::GetMember(member.into())
                });
                spans.push(Some(Span::new(0, prefix_end)));
            }
            // Java's expression-string API invokes ToString on the returned
            // value with an unavailable callsite. The shared opcode owns forcing
            // and virtual dispatch; adapters do not render compound values.
            block.push(Instruction::ToString);
            spans.push(None);
            block.push(Instruction::Return);
            spans.push(None);
            let metadata = BlockMetadata {
                source: Some(Arc::new(RuntimeSource {
                    text: expression.clone(),
                    uri: "repl:text".into(),
                    module_name: "".into(),
                })),
                spans,
                module_receiver_input: Some(0),
                ..Default::default()
            };
            entries.push(
                program_mut
                    .alloc_block(block, metadata)
                    .map_err(|error| error.message)?,
            );
        }
        let output_entry = output_profile
            .map(|_| append_output_entry(program_mut, "bytes"))
            .transpose()
            .map_err(|error| error.message)?;
        let text = source.text();
        let context = EvaluationContext {
            primary_source: Some(Arc::new(RuntimeSource {
                text: text.into(),
                uri: uri.clone(),
                module_name: name.clone(),
            })),
            trace_mode: options.trace_mode,
            log_delivery: options.log_delivery,
            frame_transformer: options.frame_transformer,
        };
        Arc::make_mut(&mut program)
            .register_primary_source(
                context
                    .primary_source
                    .as_ref()
                    .expect("primary source")
                    .clone(),
                ModuleIdentity {
                    original_uri: uri.clone(),
                    resolved_uri: uri.clone(),
                },
            )
            .map_err(|failure| runtime_error_text(&source, &uri, &name, "", failure, None))?;
        let mut host = HostSession::with_context(program, context);
        host.set_power_assertions(options.evaluation.power_assertions);
        Ok(Self {
            host,
            public_lifecycle: false,
            public_queries: Vec::new(),
            source,
            uri: uri.to_owned(),
            name,
            paths: property_paths.to_vec(),
            entries,
            output_text_entries,
            output_entry,
            output_profile: output_profile.unwrap_or(DiagnosticProfile::ApiIdentity),
            module_entry,
            module: None,
            module_started: false,
            setup_failure: None,
            initialization_failed: false,
            calls: Vec::new(),
            last_expression: None,
        })
    }

    pub fn start_module(
        &mut self,
        now_ms: u64,
        timeout_ms: Option<u64>,
    ) -> Result<(), RuntimeError> {
        self.open()?;
        if self.public_lifecycle {
            return Err(error(
                "Public ModuleSource evaluation requires a source and expression",
            ));
        }
        if self.module_started {
            return Err(error("module setup has already started"));
        }
        self.host
            .start(self.module_entry, Vec::new(), now_ms, timeout_ms)?;
        self.module_started = true;
        self.calls.push(Call::Module);
        self.last_expression = None;
        Ok(())
    }

    pub fn start_path(
        &mut self,
        index: usize,
        now_ms: u64,
        timeout_ms: Option<u64>,
    ) -> Result<(), RuntimeError> {
        self.open()?;
        let entry = self.entry(index)?;
        let (call, initial) = if self.output_text_entries[index] {
            (Call::OutputText, Call::InitialOutputText)
        } else {
            (Call::Path(index), Call::InitialPath(index))
        };
        self.start_operation(entry, call, initial, now_ms, timeout_ms)
    }

    /// Run module initialization and the output continuation in one admitted
    /// evaluation. A callback close may finish this operation but not start one.
    pub fn start_output_bytes(
        &mut self,
        now_ms: u64,
        timeout_ms: Option<u64>,
    ) -> Result<(), RuntimeError> {
        self.open()?;
        let entry = self.output_entry()?;
        self.start_operation(
            entry,
            Call::OutputBytes,
            Call::InitialOutputBytes,
            now_ms,
            timeout_ms,
        )
    }

    fn start_operation(
        &mut self,
        entry: usize,
        call: Call,
        initial_call: Call,
        now_ms: u64,
        timeout_ms: Option<u64>,
    ) -> Result<(), RuntimeError> {
        self.open()?;
        if !self.module_started {
            self.host
                .start_then(self.module_entry, Vec::new(), entry, now_ms, timeout_ms)?;
            self.module_started = true;
            self.calls.push(initial_call);
            self.last_expression = None;
            return Ok(());
        }
        if let Some(error) = &self.setup_failure {
            self.host.start_failed(error.clone(), now_ms, timeout_ms)?;
            self.calls.push(call);
            self.last_expression = call.expression();
            return Ok(());
        }
        let module = self
            .module
            .clone()
            .ok_or_else(|| error("module setup has not completed"))?;
        self.host.start(entry, vec![module], now_ms, timeout_ms)?;
        self.calls.push(call);
        self.last_expression = call.expression();
        Ok(())
    }

    pub fn reenter_path(
        &mut self,
        parent_token: &str,
        index: usize,
        now_ms: u64,
        timeout_ms: Option<u64>,
    ) -> Result<(), RuntimeError> {
        self.open()?;
        let entry = self.entry(index)?;
        let call = if self.output_text_entries[index] {
            Call::OutputText
        } else {
            Call::Path(index)
        };
        self.reenter_operation(parent_token, entry, call, now_ms, timeout_ms)
    }

    pub fn reenter_output_bytes(
        &mut self,
        parent_token: &str,
        now_ms: u64,
        timeout_ms: Option<u64>,
    ) -> Result<(), RuntimeError> {
        self.open()?;
        let entry = self.output_entry()?;
        self.reenter_operation(parent_token, entry, Call::OutputBytes, now_ms, timeout_ms)
    }

    fn reenter_operation(
        &mut self,
        parent_token: &str,
        entry: usize,
        call: Call,
        now_ms: u64,
        timeout_ms: Option<u64>,
    ) -> Result<(), RuntimeError> {
        self.open()?;
        // This is the VM's actual bootstrap receiver, never a second allocation
        // or a fabricated completed module. Publication semantics are bounded by
        // the shared VM's primary-module query.
        let module = self
            .module
            .clone()
            .or_else(|| self.host.primary_module())
            .ok_or_else(|| error("same-module callback receiver is not yet published"))?;
        self.host
            .reenter(parent_token, entry, vec![module], now_ms, timeout_ms)?;
        self.calls.push(call);
        self.last_expression = call.expression();
        Ok(())
    }

    pub fn poll(&mut self, now_ms: u64, budget: usize) -> HostPoll {
        if self.public_lifecycle {
            self.last_expression = self.public_queries.last().copied().flatten();
        }
        let previous_depth = self.host.context_depth();
        let event = self.host.poll(now_ms, budget);
        if self.public_lifecycle {
            if let Some(source) = self.host.public_primary_source() {
                let mut sources = SourceMap::default();
                let id = sources.add(source.uri.clone(), source.text.clone());
                self.source = sources.get(id).unwrap().clone();
                self.uri = source.uri.clone();
                self.name = source.module_name.clone();
            }
            if matches!(&event, HostPoll::Module { request, .. } if matches!(request.kind,
                crate::modules::ModuleRequestKind::ResolveKey { phase: crate::modules::ResolutionPhase::Query, .. }))
            {
                self.last_expression = self.calls.last().copied().and_then(Call::expression);
                if let Some(active) = self.public_queries.last_mut() {
                    *active = self.last_expression;
                }
            }
        }
        if let Some(result) = self.host.take_intermediate_result() {
            match result {
                Ok(module) => {
                    self.module = Some(module);
                    self.last_expression = self.calls.last().and_then(|call| match call {
                        Call::InitialPath(index) => Some(*index),
                        _ => None,
                    });
                }
                Err(error) => {
                    self.initialization_failed = true;
                    self.last_expression = None;
                    if !self.host.is_closed() {
                        self.setup_failure = Some(error);
                    }
                }
            }
        }
        if let HostPoll::Ready(result) = &event {
            let completed = self.host.context_depth() < previous_depth;
            if let Some(call) = completed.then(|| self.calls.pop()).flatten() {
                if self.public_lifecycle {
                    self.last_expression = self.public_queries.pop().flatten();
                }
                match call {
                    Call::Module => {
                        self.last_expression = None;
                        match result {
                            Ok(module) => self.module = Some(module.clone()),
                            Err(error) => {
                                if !matches!(
                                    error.kind,
                                    ErrorKind::Timeout | ErrorKind::Cancelled | ErrorKind::Closed
                                ) {
                                    self.initialization_failed = true;
                                    if !self.host.is_closed() {
                                        self.setup_failure = Some(error.clone());
                                    }
                                }
                            }
                        }
                    }
                    Call::Path(index) => {
                        if !self.public_lifecycle {
                            self.last_expression = Some(index);
                        }
                    }
                    Call::OutputBytes
                    | Call::InitialOutputBytes
                    | Call::OutputText
                    | Call::InitialOutputText => self.last_expression = None,
                    Call::InitialPath(index) => {
                        self.last_expression =
                            (self.module.is_some() && !self.initialization_failed).then_some(index);
                    }
                }
            }
            if self.host.is_closed() {
                self.calls.clear();
                self.module = None;
                self.setup_failure = None;
            }
        }
        event
    }

    pub fn resume(
        &mut self,
        token: &str,
        completion: Result<Value, RuntimeError>,
        now_ms: u64,
    ) -> Result<(), RuntimeError> {
        self.host.resume(token, completion, now_ms)
    }
    /// Use the shared URI policy and the actual pending request, rather than
    /// rebuilding a resource request from the Host transport's display URI.
    pub fn resume_default_read(
        &mut self,
        token: &str,
        properties: &[(String, String)],
        environment: &[(String, String)],
        now_ms: u64,
    ) -> Result<(), RuntimeError> {
        self.host
            .resume_default_read(token, properties, environment, now_ms)
    }

    pub fn complete_read(
        &mut self,
        token: &str,
        result: Result<crate::resources::ReadResult, crate::host::ReadFailure>,
        now_ms: u64,
    ) -> Result<(), RuntimeError> {
        self.host.complete_read(token, result, now_ms)
    }

    pub fn module_request(
        &self,
        token: &str,
    ) -> Result<crate::modules::ModuleRequest, RuntimeError> {
        self.host.module_request(token)
    }
    pub fn read_request(&self, token: &str) -> Result<crate::vm::ReadRequest, RuntimeError> {
        self.host.read_request(token)
    }
    pub fn reject_read(
        &mut self,
        token: &str,
        failure: crate::host::ReadFailure,
        now_ms: u64,
    ) -> Result<(), RuntimeError> {
        self.host.reject_read(token, failure, now_ms)
    }
    pub fn acknowledge_log(
        &mut self,
        token: &str,
        completion: Result<(), LoggerFailure>,
        now_ms: u64,
    ) -> Result<(), RuntimeError> {
        self.host.acknowledge_log(token, completion, now_ms)
    }
    pub fn complete_frame_transform(
        &mut self,
        token: &str,
        completion: Result<crate::logging::LogFrame, LoggerFailure>,
        now_ms: u64,
    ) -> Result<(), RuntimeError> {
        self.host
            .complete_frame_transform(token, completion, now_ms)
    }
    pub fn set_frame_transformer(
        &mut self,
        transformer: crate::logging::FrameTransformer,
    ) -> Result<(), RuntimeError> {
        self.host.set_frame_transformer(transformer)
    }
    pub fn close_from_callback(&mut self, token: &str) -> Result<(), RuntimeError> {
        self.host.close_from_callback(token)
    }
    pub fn cancel(&mut self) {
        self.host.cancel();
        self.release_module();
    }
    pub fn close(&mut self) {
        self.host.close();
        self.release_module();
    }
    pub fn is_closed(&self) -> bool {
        self.host.is_closed()
    }
    /// A recorded initialization failure survives cleanup of the cached error.
    /// Property errors and Host-wide cancellation/timeouts do not change it:
    /// termination before initialization completes is not an initialization result.
    pub fn setup_failed(&self) -> bool {
        self.initialization_failed
    }
    pub fn context_depth(&self) -> usize {
        self.host.context_depth()
    }
    pub fn drain_cancelled_reads(&mut self) -> Vec<String> {
        self.host.drain_cancelled_reads()
    }
    pub fn set_glob_test_mode(&mut self, enabled: bool) -> Result<(), RuntimeError> {
        self.host.set_glob_test_mode(enabled)
    }
    pub fn glob_request(&self, token: &str) -> Result<crate::glob::GlobRequest, RuntimeError> {
        self.host.glob_request(token)
    }
    pub fn complete_glob(
        &mut self,
        token: &str,
        completion: crate::glob::GlobCompletion,
        now_ms: u64,
    ) -> Result<(), RuntimeError> {
        self.host.complete_glob(token, completion, now_ms)
    }
    pub fn complete_default_glob(
        &mut self,
        token: &str,
        properties: &[(String, String)],
        environment: &[(String, String)],
        now_ms: u64,
    ) -> Result<bool, RuntimeError> {
        self.host
            .complete_default_glob(token, properties, environment, now_ms)
    }
    pub fn drain_cancelled_globs(&mut self) -> Vec<String> {
        self.host.drain_cancelled_globs()
    }
    pub fn complete_module(
        &mut self,
        token: &str,
        completion: ModuleCompletion,
        now_ms: u64,
    ) -> Result<(), RuntimeError> {
        self.host.complete_module(token, completion, now_ms)
    }

    pub fn complete_default_module(
        &mut self,
        token: &str,
        now_ms: u64,
    ) -> Result<bool, RuntimeError> {
        self.host.complete_default_module(token, now_ms)
    }
    pub fn drain_cancelled_modules(&mut self) -> Vec<String> {
        self.host.drain_cancelled_modules()
    }
    pub fn heap_stats(&self) -> HeapStats {
        self.host.heap_stats()
    }

    /// The retained module and a just-returned result are roots even after the
    /// Host has restored a suspended parent context.
    pub fn collect(&mut self, external_roots: &[Value]) -> HeapStats {
        let mut roots = external_roots.to_vec();
        if let Some(module) = &self.module {
            roots.push(module.clone());
        }
        self.host.collect(&roots)
    }
    /// Canonical diagnostic text. No UTF-8 conversion occurs before API units.
    pub fn format_error_text(&self, error: RuntimeError) -> PklString {
        let expression = self
            .last_expression
            .and_then(|index| self.paths.get(index))
            .cloned()
            .unwrap_or_else(|| "".into());
        runtime_error_text(&self.source, &self.uri, &self.name, expression, error, None)
    }
    /// Legacy byte-oriented diagnostic convenience wrapper.
    pub fn format_error(&self, error: RuntimeError) -> String {
        self.format_error_text(error).to_java_utf8()
    }
    pub fn result_string(value: Value) -> Result<PklString, RuntimeError> {
        match value {
            Value::String(value) => Ok(value),
            _ => Err(error("source expression did not return a string")),
        }
    }
    pub fn result_bytes(value: Value) -> Result<Arc<[u8]>, RuntimeError> {
        match value {
            Value::Bytes(bytes) => Ok(bytes.into_data()),
            _ => Err(RuntimeError::new(
                ErrorKind::InvalidBytecode,
                "module output operation did not return bytes",
            )),
        }
    }
    pub fn output_failure_text(&self, error: RuntimeError) -> ModuleFrontendFailure {
        render_module_runtime_failure_text(
            &self.source,
            &self.uri,
            &self.name,
            self.output_profile,
            error,
        )
    }
    pub fn output_failure(&self, error: RuntimeError) -> ModuleFrontendResult {
        self.output_failure_text(error).into()
    }
    /// A callback property path still belongs to the expression-string API,
    /// even when its suspended parent is an output operation.
    pub fn output_session_error_bytes(&self, error: RuntimeError) -> Vec<u8> {
        self.output_session_failure(error).stderr
    }
    pub fn output_session_failure_text(&self, error: RuntimeError) -> ModuleFrontendFailure {
        let mut failure = self.output_failure_text(error.clone());
        if self.last_expression.is_some() {
            failure.text = self.format_error_text(error);
        }
        failure
    }
    pub fn output_session_failure(&self, error: RuntimeError) -> ModuleFrontendResult {
        self.output_session_failure_text(error).into()
    }
    fn open(&self) -> Result<(), RuntimeError> {
        if self.host.is_closed() {
            Err(RuntimeError::new(ErrorKind::Closed, "evaluator is closed"))
        } else {
            Ok(())
        }
    }
    fn entry(&self, index: usize) -> Result<usize, RuntimeError> {
        self.entries
            .get(index)
            .copied()
            .ok_or_else(|| error("source property-path index is out of range"))
    }
    fn output_entry(&self) -> Result<usize, RuntimeError> {
        self.output_entry.ok_or_else(|| {
            RuntimeError::new(
                ErrorKind::Unsupported,
                "this source session was not compiled for module output",
            )
        })
    }
    fn release_module(&mut self) {
        self.module = None;
        self.calls.clear();
        self.public_queries.clear();
        self.setup_failure = None;
        self.last_expression = None;
        if self.public_lifecycle {
            self.paths.clear();
        }
    }
}

fn error(message: &str) -> RuntimeError {
    RuntimeError::new(ErrorKind::Host, message)
}

/// Real ModuleOutput operation, with no synthetic expression or ToString root.
pub(super) fn append_output_entry(
    program: &mut Program,
    member: &str,
) -> Result<usize, RuntimeError> {
    program.alloc_block(
        vec![
            Instruction::Input(0),
            Instruction::GetMember("output".into()),
            Instruction::CheckModuleOutput,
            Instruction::GetMember(member.into()),
            Instruction::Return,
        ],
        BlockMetadata {
            module_receiver_input: Some(0),
            ..Default::default()
        },
    )
}

#[cfg(test)]
mod public_query_storage_tests {
    use super::*;
    use crate::modules::{ModuleKey, ModuleKeyKind, ModuleRequestKind, ModuleSourceSpec};
    fn source() -> ModuleSourceSpec {
        ModuleSourceSpec {
            uri: "memo:/main.pkl".into(),
            contents: Some("stable=7".into()),
        }
    }
    fn event(session: &mut SourceSession) -> HostPoll {
        for _ in 0..100_000 {
            let event = session.poll(0, 1);
            session.collect(&[]);
            match event {
                HostPoll::Yielded => {}
                HostPoll::Module { token, request } => {
                    let completion =
                        if let ModuleRequestKind::SelectKey { id, uri, .. } = &request.kind {
                            ModuleCompletion::KeySelected(Ok(ModuleKey {
                                id: *id,
                                uri: uri.clone(),
                                kind: ModuleKeyKind::Host,
                                handle: "fixture".into(),
                            }))
                        } else {
                            crate::modules::default_completion(&request).unwrap()
                        };
                    session.complete_module(&token, completion, 0).unwrap();
                }
                event => return event,
            }
        }
        panic!("event poll bound")
    }
    #[test]
    fn public_expression_text_tracks_only_active_calls_and_the_current_diagnostic() {
        let mut session = SourceSession::module_evaluator(SourceSessionOptions::default());
        for _ in 0..8 {
            session
                .start_evaluation(source(), "throw(\"ordinary failure\")", 0, None)
                .unwrap();
            let HostPoll::Ready(Err(error)) = event(&mut session) else {
                panic!("query failure")
            };
            assert!(session.format_error(error).contains("ordinary failure"));
            assert_eq!(session.paths.len(), 1);
        }
        session
            .start_evaluation(source(), "read(\"prop:pause\")", 0, None)
            .unwrap();
        let HostPoll::Read { token, .. } = event(&mut session) else {
            panic!("parent callback")
        };
        for _ in 0..8 {
            session
                .reenter_evaluation(&token, source(), "throw(\"child failure\")", 0, None)
                .unwrap();
            let HostPoll::Ready(Err(error)) = event(&mut session) else {
                panic!("child failure")
            };
            assert!(session.format_error(error).contains("child failure"));
            assert_eq!(session.paths.len(), 2);
        }
        session
            .resume(&token, Ok(Value::string("parent")), 0)
            .unwrap();
        assert!(
            matches!(event(&mut session), HostPoll::Ready(Ok(value)) if value == Value::string("parent"))
        );
        session.close();
        assert!(session.paths.is_empty());
    }
}
