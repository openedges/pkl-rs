//! Development-only parsed-source Host session. Entries are fixed before execution;
//! this is not a general REPL. Module I/O is supplied through typed Host effects.
use super::frontend::{
    DiagnosticProfile, FailureDisposition, FrontendPhase, LosslessModuleSourceInput,
    ModuleEvalOptions, ModuleFrontendFailure, ModuleFrontendResult, prepare_module_text_colored,
    render_module_runtime_failure_text_colored,
};
use super::output_files::{
    OutputFileEntry, append_output_file_bytes_entry, append_output_file_keys_entry,
    append_output_files_entry, decode_output_file_keys, decode_output_files,
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
    pub platform_data: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum EntryKind {
    Path,
    OutputText,
    OutputBytes,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Call {
    Module,
    Path(usize),
    InitialPath(usize),
    OutputBytes,
    InitialOutputBytes,
    OutputFiles,
    InitialOutputFiles,
    OutputFileKeys,
    InitialOutputFileKeys,
    OutputFileBytes,
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
    entry_kinds: Vec<EntryKind>,
    output_entry: Option<usize>,
    output_files_entry: Option<usize>,
    output_file_keys_entry: Option<usize>,
    output_file_bytes_entry: Option<usize>,
    output_profile: DiagnosticProfile,
    module_entry: usize,
    module: Option<Value>,
    module_started: bool,
    setup_failure: Option<RuntimeError>,
    initialization_failed: bool,
    calls: Vec<Call>,
    last_completed_call: Option<Call>,
    last_expression: Option<usize>,
    use_color: bool,
    pure_standard_host: bool,
    query_repl_resolved_ok: bool,
    awaiting_query_resolve: bool,
    primary_source_cache: Option<Arc<RuntimeSource>>,
    primary_source_id: Option<crate::value::ObjectId>,
}

impl SourceSession {
    /// Configure an evaluator without parsing a module, selecting a key, or doing I/O.
    pub fn module_evaluator(options: SourceSessionOptions) -> Self {
        static EMPTY_REPL_SOURCE: std::sync::OnceLock<Source> = std::sync::OnceLock::new();
        let source = EMPTY_REPL_SOURCE
            .get_or_init(|| {
                let mut sources = SourceMap::default();
                let id = sources.add("repl:text", "");
                sources.get(id).unwrap().clone()
            })
            .clone();
        let mut host = HostSession::with_base_snapshot(EvaluationContext {
            primary_source: None,
            trace_mode: options.trace_mode,
            log_delivery: options.log_delivery,
            frame_transformer: options.frame_transformer,
            platform_metadata: options
                .platform_data
                .then(crate::logging::PlatformMetadata::default),
        });
        host.set_power_assertions(options.evaluation.power_assertions);
        host.enable_auto_gc();
        Self {
            host,
            public_lifecycle: true,
            public_queries: Vec::new(),
            source,
            uri: "repl:text".into(),
            name: "".into(),
            paths: Vec::new(),
            entries: Vec::new(),
            entry_kinds: Vec::new(),
            output_entry: None,
            output_files_entry: None,
            output_file_keys_entry: None,
            output_file_bytes_entry: None,
            output_profile: DiagnosticProfile::ApiIdentity,
            module_entry: 0,
            module: None,
            module_started: false,
            setup_failure: None,
            initialization_failed: false,
            calls: Vec::new(),
            last_completed_call: None,
            last_expression: None,
            use_color: false,
            pure_standard_host: false,
            query_repl_resolved_ok: false,
            awaiting_query_resolve: false,
            primary_source_cache: None,
            primary_source_id: None,
        }
    }

    /// Clears the cached elements on `facts[group_name]` for `module_uri` and starts collecting
    /// VM power-assertion observations for the next `facts[group][idx]` re-evaluation (`TestRunner.java:105-115`).
    pub fn begin_fact_trace(
        &mut self,
        module_uri: &PklString,
        group_name: &PklString,
        fact_span: crate::source::Span,
    ) -> bool {
        self.host
            .begin_fact_trace(module_uri, group_name, fact_span)
    }

    /// Stops fact power-assertion tracking and returns rendered observations anchored
    /// to byte spans in the test module's source.
    pub fn finish_fact_trace(&mut self) -> Vec<crate::diagnostic::PowerAssertionValue> {
        self.host.finish_fact_trace()
    }

    /// Marks whether the driving Host uses pure, immutable standard module resolution
    /// (no custom security manager, no custom/external module readers), allowing warm
    /// cached-module public operations to bypass redundant Host roundtrips.
    pub fn set_pure_standard_host(&mut self, enabled: bool) {
        self.pure_standard_host = enabled;
        self.host.set_pure_standard_host(enabled);
    }

    /// Builder-style helper for [`Self::set_pure_standard_host`].
    pub fn with_pure_standard_host(mut self, enabled: bool) -> Self {
        self.pure_standard_host = enabled;
        self.host.set_pure_standard_host(enabled);
        self
    }

    fn sync_primary_source(&mut self) {
        let id = self.host.public_primary_id();
        if id.is_some() && self.primary_source_id == id {
            return;
        }
        if let Some(source) = self.host.public_primary_source() {
            if self
                .primary_source_cache
                .as_ref()
                .is_some_and(|prev| Arc::ptr_eq(prev, &source))
            {
                self.primary_source_id = id;
                return;
            }
            let mut sources = SourceMap::default();
            let src_id = sources.add(source.uri.clone(), source.text.clone());
            self.source = sources.get(src_id).unwrap().clone();
            self.uri = source.uri.clone();
            self.name = source.module_name.clone();
            self.primary_source_cache = Some(source);
            self.primary_source_id = id;
        }
    }

    fn reset_provisional_source(&mut self, source: &crate::modules::ModuleSourceSpec) {
        if source.contents.is_none()
            && self
                .primary_source_cache
                .as_ref()
                .is_some_and(|cached| cached.uri == source.uri)
        {
            return;
        }
        let mut sources = SourceMap::default();
        let id = sources.add(
            source.uri.clone(),
            source.contents.clone().unwrap_or_else(|| "".into()),
        );
        self.source = sources.get(id).unwrap().clone();
        self.uri = source.uri.clone();
        self.name = PklString::default();
        self.primary_source_cache = None;
        self.primary_source_id = None;
    }

    /// Enables or disables ANSI color rendering for diagnostics emitted by this session.
    pub fn set_use_color(&mut self, use_color: bool) {
        self.use_color = use_color;
    }

    /// Builder-style helper to set ANSI color rendering on this session.
    pub fn with_color(mut self, use_color: bool) -> Self {
        self.use_color = use_color;
        self
    }

    /// Returns whether ANSI color rendering is enabled on this session.
    pub fn use_color(&self) -> bool {
        self.use_color
    }

    /// Returns `true` when `uri` is already warm-cached in this evaluator on a pure
    /// standard host and no evaluation is currently active.
    #[inline]
    pub fn has_warm_cached_module(&self, uri: &PklString) -> bool {
        self.pure_standard_host
            && self.public_lifecycle
            && self.calls.is_empty()
            && self.host.has_warm_cached_module(uri)
    }

    /// Attempts to pack an already-warm-cached module's `"module"` or `"output.value"`
    /// directly into a compact columnar buffer with zero VM execution or MessagePack
    /// encoding. Returns `false` when full evaluation/fallback is required.
    #[inline]
    pub fn try_pack_warm_expression_columnar(
        &mut self,
        uri: &PklString,
        expression: &str,
        out: &mut crate::vm::columnar_value::ColumnarValueBuffer,
    ) -> bool {
        self.pure_standard_host
            && self.public_lifecycle
            && self.calls.is_empty()
            && self.open().is_ok()
            && self
                .host
                .try_pack_warm_expression_columnar(uri, expression, out)
    }

    /// Read-only access to the underlying cooperative `HostSession`.
    pub fn host_session(&self) -> &HostSession {
        &self.host
    }

    /// Mutable access to the underlying cooperative `HostSession`.
    pub fn host_session_mut(&mut self) -> &mut HostSession {
        &mut self.host
    }

    #[allow(dead_code)]
    pub(crate) fn vm(&self) -> &crate::vm::Vm {
        self.host.vm()
    }

    #[allow(dead_code)]
    pub(crate) fn vm_mut(&mut self) -> &mut crate::vm::Vm {
        self.host.vm_mut()
    }

    /// Starts a cached-module evaluation that encodes the result of `expression`
    /// (`"module"`, `"output.text"`, `"output.value"`, `"output.bytes"`, or an arbitrary
    /// Pkl expression) into Pkl 0.32.0 `pkl-binary` (`Value::Bytes`).
    pub fn start_evaluation_pkl_binary(
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
        if self.calls.is_empty() {
            self.module = None;
            self.host.set_session_root(None);
            self.host.maybe_auto_gc_between_operations();
        }
        let warm_op = match expression.original().as_str() {
            Some("module") => Some(crate::vm::WarmPublicOp::PklBinaryModule),
            Some("output.text") => Some(crate::vm::WarmPublicOp::PklBinaryOutputText),
            Some("output.value") => Some(crate::vm::WarmPublicOp::PklBinaryOutputValue),
            Some("output.bytes") => Some(crate::vm::WarmPublicOp::PklBinaryOutputBytes),
            _ if self.query_repl_resolved_ok => Some(crate::vm::WarmPublicOp::Query {
                expression: &expression,
                raw_value: true,
                encode_pkl_binary: true,
            }),
            _ => None,
        };
        let mut started_warm = false;
        if self.pure_standard_host
            && self.calls.is_empty()
            && (source.contents.is_none() || source.uri != "repl:text")
            && let Some(op) = warm_op
        {
            started_warm =
                self.host
                    .try_start_warm_cached_public_op(&source.uri, op, now_ms, timeout_ms)?;
        }
        if !started_warm {
            let entry = self
                .host
                .append_public_pkl_binary_operation(source.clone(), expression.clone())?;
            self.host.start(entry, Vec::new(), now_ms, timeout_ms)?;
        }
        self.retain_active_query_text();
        if started_warm {
            self.sync_primary_source();
        } else {
            self.reset_provisional_source(&source);
        }
        let index = self.paths.len();
        self.paths.push(expression.clone());
        let call = match expression.original().as_str() {
            Some("output.text") => Call::OutputText,
            Some("output.bytes" | "module" | "output.value") => Call::OutputBytes,
            _ => Call::Path(index),
        };
        let query_slot = if started_warm {
            call.expression()
        } else {
            None
        };
        self.calls.push(call);
        self.public_queries.push(query_slot);
        self.last_expression = query_slot;
        Ok(())
    }

    /// Starts a cached-module evaluation returning `output.bytes` (`Value::Bytes`).
    pub fn start_evaluation_output_bytes(
        &mut self,
        source: crate::modules::ModuleSourceSpec,
        now_ms: u64,
        timeout_ms: Option<u64>,
    ) -> Result<(), RuntimeError> {
        self.open()?;
        if !self.public_lifecycle {
            return Err(error(
                "ModuleSource operations require the public lifecycle profile",
            ));
        }
        if self.calls.is_empty() {
            self.module = None;
            self.host.set_session_root(None);
            self.host.maybe_auto_gc_between_operations();
        }
        let mut started_warm = false;
        if self.pure_standard_host
            && self.calls.is_empty()
            && (source.contents.is_none() || source.uri != "repl:text")
        {
            started_warm = self.host.try_start_warm_cached_public_op(
                &source.uri,
                crate::vm::WarmPublicOp::OutputBytes,
                now_ms,
                timeout_ms,
            )?;
        }
        if !started_warm {
            let entry = self
                .host
                .append_public_output_bytes_operation(source.clone())?;
            self.host.start(entry, Vec::new(), now_ms, timeout_ms)?;
        }
        self.retain_active_query_text();
        if started_warm {
            self.sync_primary_source();
        } else {
            self.reset_provisional_source(&source);
        }
        self.calls.push(Call::OutputBytes);
        self.public_queries.push(None);
        self.last_expression = None;
        Ok(())
    }

    /// Starts loading a primary module (using cache) and returns the module `Value`.
    pub fn start_load_module(
        &mut self,
        source: crate::modules::ModuleSourceSpec,
        now_ms: u64,
        timeout_ms: Option<u64>,
    ) -> Result<(), RuntimeError> {
        self.open()?;
        if !self.public_lifecycle {
            return Err(error(
                "ModuleSource operations require the public lifecycle profile",
            ));
        }
        // `start_load_module` feeds `SchemaExtractor` / `pkldoc` / reflection consumers
        // that read `pkl:base` unit syntax & reflection annotations immutably via `session.vm()`.
        self.host.ensure_base_full_metadata()?;
        if self.calls.is_empty() {
            self.module = None;
            self.host.set_session_root(None);
            self.host.maybe_auto_gc_between_operations();
        }
        let mut started_warm = false;
        if self.pure_standard_host
            && self.calls.is_empty()
            && (source.contents.is_none() || source.uri != "repl:text")
        {
            started_warm = self.host.try_start_warm_cached_public_op(
                &source.uri,
                crate::vm::WarmPublicOp::LoadModule,
                now_ms,
                timeout_ms,
            )?;
        }
        if !started_warm {
            let entry = self
                .host
                .append_public_load_module_operation(source.clone())?;
            self.host.start(entry, Vec::new(), now_ms, timeout_ms)?;
        }
        self.retain_active_query_text();
        if started_warm {
            self.sync_primary_source();
        } else {
            self.reset_provisional_source(&source);
        }
        self.calls.push(Call::Module);
        self.public_queries.push(None);
        self.last_expression = None;
        Ok(())
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
        if self.calls.is_empty() {
            self.module = None;
            self.host.set_session_root(None);
            self.host.maybe_auto_gc_between_operations();
        }
        let is_output_text = expression.original() == "output.text";
        let mut started_warm = false;
        if self.pure_standard_host
            && self.calls.is_empty()
            && (source.contents.is_none() || source.uri != "repl:text")
            && (is_output_text || self.query_repl_resolved_ok)
        {
            let op = if is_output_text {
                crate::vm::WarmPublicOp::OutputText
            } else {
                crate::vm::WarmPublicOp::Query {
                    expression: &expression,
                    raw_value: false,
                    encode_pkl_binary: false,
                }
            };
            started_warm =
                self.host
                    .try_start_warm_cached_public_op(&source.uri, op, now_ms, timeout_ms)?;
        }
        if !started_warm {
            let entry = self
                .host
                .append_public_operation(source.clone(), expression.clone())?;
            self.host.start(entry, Vec::new(), now_ms, timeout_ms)?;
        }
        self.retain_active_query_text();
        if started_warm {
            self.sync_primary_source();
        } else {
            self.reset_provisional_source(&source);
        }
        let index = self.paths.len();
        self.paths.push(expression);
        let call = if is_output_text {
            Call::OutputText
        } else {
            Call::Path(index)
        };
        let query_slot = if started_warm {
            call.expression()
        } else {
            None
        };
        self.calls.push(call);
        self.public_queries.push(query_slot);
        self.last_expression = query_slot;
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
        Self::compile_output_text_colored(input, property_paths, module_options, options, false)
            .map_err(Into::into)
    }

    /// Prepare real module output with explicit ANSI color setting for frontend and runtime errors.
    pub fn compile_output_colored(
        input: impl Into<LosslessModuleSourceInput>,
        property_paths: &[String],
        module_options: &ModuleEvalOptions,
        options: SourceSessionOptions,
        use_color: bool,
    ) -> Result<Self, ModuleFrontendResult> {
        Self::compile_output_text_colored(input, property_paths, module_options, options, use_color)
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
        Self::compile_output_text_colored(input, property_paths, module_options, options, false)
    }

    pub fn compile_output_text_colored(
        input: impl Into<LosslessModuleSourceInput>,
        property_paths: &[String],
        module_options: &ModuleEvalOptions,
        options: SourceSessionOptions,
        use_color: bool,
    ) -> Result<Self, ModuleFrontendFailure> {
        let input = input.into();
        let prepared = prepare_module_text_colored(input.clone(), module_options, use_color)?;
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
        .map(|s| s.with_color(use_color))
        .map_err(|message| ModuleFrontendFailure {
            phase: FrontendPhase::Configuration,
            disposition: FailureDisposition::Unsupported,
            reason: Some("source-session-callback-expression"),
            text: crate::diagnostic::render_error_text_colored(&message, &[], None, use_color),
        })
    }

    /// Prepare a single CLI `-x`/`--expression` evaluation (`EvaluatorImpl.evaluateExpressionString`)
    /// on the shared output-session profile. `"output.text"` fast-paths without a synthetic REPL
    /// frame while all other expressions (including `"output.bytes"`) evaluate through `ToString`.
    pub fn compile_output_expression_text(
        input: impl Into<LosslessModuleSourceInput>,
        expression: impl Into<SourceText>,
        module_options: &ModuleEvalOptions,
        options: SourceSessionOptions,
    ) -> Result<Self, ModuleFrontendFailure> {
        Self::compile_output_expression_text_colored(
            input,
            expression,
            module_options,
            options,
            false,
        )
    }

    pub fn compile_output_expression_text_colored(
        input: impl Into<LosslessModuleSourceInput>,
        expression: impl Into<SourceText>,
        module_options: &ModuleEvalOptions,
        options: SourceSessionOptions,
        use_color: bool,
    ) -> Result<Self, ModuleFrontendFailure> {
        let input = input.into();
        let prepared = prepare_module_text_colored(input.clone(), module_options, use_color)?;
        Self::from_program_with_mode(
            prepared.program,
            prepared.source,
            input.uri,
            prepared.name,
            &[expression.into()],
            options,
            Some(module_options.diagnostic_profile),
            true,
            false,
        )
        .map(|s| s.with_color(use_color))
        .map_err(|message| ModuleFrontendFailure {
            phase: FrontendPhase::Configuration,
            disposition: FailureDisposition::Unsupported,
            reason: Some("source-session-output-expression"),
            text: crate::diagnostic::render_error_text_colored(&message, &[], None, use_color),
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn from_program(
        program: Arc<Program>,
        source: Source,
        uri: impl Into<PklString>,
        name: PklString,
        property_paths: &[SourceText],
        options: SourceSessionOptions,
        output_profile: Option<DiagnosticProfile>,
        parsed_expressions: bool,
    ) -> Result<Self, PklString> {
        Self::from_program_with_mode(
            program,
            source,
            uri,
            name,
            property_paths,
            options,
            output_profile,
            parsed_expressions,
            true,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn from_program_with_mode(
        mut program: Arc<Program>,
        source: Source,
        uri: impl Into<PklString>,
        name: PklString,
        property_paths: &[SourceText],
        options: SourceSessionOptions,
        output_profile: Option<DiagnosticProfile>,
        parsed_expressions: bool,
        fast_path_output_bytes: bool,
    ) -> Result<Self, PklString> {
        let uri = uri.into();

        let module_entry = program.entry;
        let program_mut = Arc::make_mut(&mut program);
        let needs_expression_owner = parsed_expressions
            && property_paths.iter().any(|expression| {
                expression.original() != "output.text"
                    && !(fast_path_output_bytes && expression.original() == "output.bytes")
            });
        let expression_owner = if needs_expression_owner {
            if let Some(syntax) = program_mut.units.get(0).and_then(|unit| {
                unit.syntax
                    .clone()
                    .or_else(|| unit.build.as_ref().map(|build| build.module.clone()))
            }) {
                Some(syntax)
            } else {
                Some(Arc::new(
                    crate::parser::parse_module_for_evaluation(source.text()).map_err(
                        |failure| {
                            failure
                                .minimum_version_error(&name)
                                .map_or(failure.diagnostic.message, |error| error.message)
                        },
                    )?,
                ))
            }
        } else {
            None
        };
        let mut entries = Vec::with_capacity(property_paths.len());
        let mut entry_kinds = Vec::with_capacity(property_paths.len());
        for expression in property_paths {
            // EvaluatorImpl.evaluateExpressionString / evaluateExpression fast-path
            // `output.text` and `output.bytes` before creating a REPL root.
            if expression.original() == "output.text" {
                entry_kinds.push(EntryKind::OutputText);
                entries
                    .push(append_output_entry(program_mut, "text").map_err(|error| error.message)?);
                continue;
            }
            if fast_path_output_bytes && expression.original() == "output.bytes" {
                entry_kinds.push(EntryKind::OutputBytes);
                entries.push(
                    append_output_entry(program_mut, "bytes").map_err(|error| error.message)?,
                );
                continue;
            }
            entry_kinds.push(EntryKind::Path);
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
                                if let Some(foreign) = &diagnostic.provenance.foreign_exception {
                                    error.foreign_exception = Some(foreign.clone());
                                }
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
                                vec![Instruction::Fail(Box::new(error))],
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
        let output_files_entry = output_profile
            .map(|_| append_output_files_entry(program_mut))
            .transpose()
            .map_err(|error| error.message)?;
        let output_file_keys_entry = output_profile
            .map(|_| append_output_file_keys_entry(program_mut))
            .transpose()
            .map_err(|error| error.message)?;
        let output_file_bytes_entry = output_profile
            .map(|_| append_output_file_bytes_entry(program_mut))
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
            platform_metadata: options
                .platform_data
                .then(crate::logging::PlatformMetadata::default),
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
        if output_profile.is_some() {
            host.enable_auto_gc();
        }
        Ok(Self {
            host,
            public_lifecycle: false,
            public_queries: Vec::new(),
            source,
            uri: uri.to_owned(),
            name,
            paths: property_paths.to_vec(),
            entries,
            entry_kinds,
            output_entry,
            output_files_entry,
            output_file_keys_entry,
            output_file_bytes_entry,
            output_profile: output_profile.unwrap_or(DiagnosticProfile::ApiIdentity),
            module_entry,
            module: None,
            module_started: false,
            setup_failure: None,
            initialization_failed: false,
            calls: Vec::new(),
            last_completed_call: None,
            last_expression: None,
            use_color: false,
            pure_standard_host: false,
            query_repl_resolved_ok: false,
            awaiting_query_resolve: false,
            primary_source_cache: None,
            primary_source_id: None,
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
        self.host.ensure_base_full_metadata()?;
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
        let (call, initial) = match self.entry_kinds[index] {
            EntryKind::OutputText => (Call::OutputText, Call::InitialOutputText),
            EntryKind::OutputBytes => (Call::OutputBytes, Call::InitialOutputBytes),
            EntryKind::Path => (Call::Path(index), Call::InitialPath(index)),
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

    /// Run module initialization and `Module.output.files` evaluation in one admitted
    /// operation, returning an encoded ordered `(path_spec, bytes)` payload decoded by
    /// `SourceSession::result_output_files`.
    pub fn start_output_files(
        &mut self,
        now_ms: u64,
        timeout_ms: Option<u64>,
    ) -> Result<(), RuntimeError> {
        self.open()?;
        let entry = self.output_files_entry()?;
        self.start_operation(
            entry,
            Call::OutputFiles,
            Call::InitialOutputFiles,
            now_ms,
            timeout_ms,
        )
    }

    /// Shallow-force `Module.output.files` entries and return ordered path-spec keys
    /// without evaluating `FileOutput.bytes` (matching `VmUtils.readFilesProperty`).
    pub fn start_output_file_keys(
        &mut self,
        now_ms: u64,
        timeout_ms: Option<u64>,
    ) -> Result<(), RuntimeError> {
        self.open()?;
        let entry = self.output_file_keys_entry()?;
        self.start_operation(
            entry,
            Call::OutputFileKeys,
            Call::InitialOutputFileKeys,
            now_ms,
            timeout_ms,
        )
    }

    /// Lazily evaluate `FileOutput.bytes` for one key after `start_output_file_keys`
    /// has initialized the module and shallow-forced `output.files`.
    pub fn start_output_file_bytes(
        &mut self,
        path_spec: PklString,
        now_ms: u64,
        timeout_ms: Option<u64>,
    ) -> Result<(), RuntimeError> {
        self.open()?;
        let entry = self.output_file_bytes_entry()?;
        if let Some(error) = &self.setup_failure {
            self.host.start_failed(error.clone(), now_ms, timeout_ms)?;
            self.calls.push(Call::OutputFileBytes);
            self.last_expression = None;
            return Ok(());
        }
        let module = self
            .module
            .clone()
            .ok_or_else(|| error("module setup has not completed"))?;
        self.host.start(
            entry,
            vec![module, Value::String(path_spec)],
            now_ms,
            timeout_ms,
        )?;
        self.calls.push(Call::OutputFileBytes);
        self.last_expression = None;
        Ok(())
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
        let call = match self.entry_kinds[index] {
            EntryKind::OutputText => Call::OutputText,
            EntryKind::OutputBytes => Call::OutputBytes,
            EntryKind::Path => Call::Path(index),
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

    pub fn reenter_output_files(
        &mut self,
        parent_token: &str,
        now_ms: u64,
        timeout_ms: Option<u64>,
    ) -> Result<(), RuntimeError> {
        self.open()?;
        let entry = self.output_files_entry()?;
        self.reenter_operation(parent_token, entry, Call::OutputFiles, now_ms, timeout_ms)
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
        self.last_completed_call = None;
        if self.public_lifecycle {
            self.last_expression = self.public_queries.last().copied().flatten();
        }
        let previous_depth = self.host.context_depth();
        let event = self.host.poll(now_ms, budget);
        if self.public_lifecycle {
            self.sync_primary_source();
            if matches!(&event, HostPoll::Module { request, .. } if matches!(request.kind,
                crate::modules::ModuleRequestKind::ResolveKey { phase: crate::modules::ResolutionPhase::Query, .. }))
            {
                self.awaiting_query_resolve = true;
                self.last_expression = self.calls.last().copied().and_then(Call::expression);
                if let Some(active) = self.public_queries.last_mut() {
                    *active = self.last_expression;
                }
            }
        }
        if let Some(result) = self.host.take_intermediate_result() {
            match result {
                Ok(module) => {
                    self.host.set_session_root(Some(module.clone()));
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
                self.last_completed_call = Some(call);
                if self.public_lifecycle {
                    self.last_expression = self.public_queries.pop().flatten();
                }
                match call {
                    Call::Module => {
                        self.last_expression = None;
                        match result {
                            Ok(module) => {
                                self.host.set_session_root(Some(module.clone()));
                                self.module = Some(module.clone());
                            }
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
                    | Call::OutputFiles
                    | Call::InitialOutputFiles
                    | Call::OutputFileKeys
                    | Call::InitialOutputFileKeys
                    | Call::OutputFileBytes
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
    pub fn set_platform_metadata(&mut self, metadata: Option<crate::logging::PlatformMetadata>) {
        self.host.set_platform_metadata(metadata);
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
        let query_ok = self.awaiting_query_resolve
            && matches!(
                &completion,
                ModuleCompletion::KeyResolved(Ok(_)) | ModuleCompletion::Resolved(Ok(_))
            );
        self.awaiting_query_resolve = false;
        let res = self.host.complete_module(token, completion, now_ms);
        if res.is_ok() && query_ok {
            self.query_repl_resolved_ok = true;
        }
        res
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
        super::runtime_error_text_with_profile_and_color(
            &self.source,
            &self.uri,
            &self.name,
            expression,
            error,
            None,
            DiagnosticProfile::ApiIdentity,
            self.use_color,
        )
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
    pub fn result_output_files(value: Value) -> Result<Vec<OutputFileEntry>, RuntimeError> {
        decode_output_files(value)
    }
    pub fn result_output_file_keys(value: Value) -> Result<Vec<PklString>, RuntimeError> {
        decode_output_file_keys(value)
    }
    pub(crate) fn last_call_was_output_files(&self) -> bool {
        matches!(
            self.last_completed_call,
            Some(Call::OutputFiles | Call::InitialOutputFiles)
        )
    }
    pub fn output_failure_text(&self, error: RuntimeError) -> ModuleFrontendFailure {
        render_module_runtime_failure_text_colored(
            &self.source,
            &self.uri,
            &self.name,
            self.output_profile,
            error,
            self.use_color,
        )
    }
    pub fn output_failure(&self, error: RuntimeError) -> ModuleFrontendResult {
        self.output_failure_text(error).into()
    }
    /// Render a CLI `-x`/`--expression` failure using the session's configured
    /// diagnostic profile while supplying the active expression text for `repl:text`.
    pub fn output_expression_failure(&self, error: RuntimeError) -> ModuleFrontendResult {
        let mut failure = self.output_failure_text(error.clone());
        if let Some(expression) = self
            .last_expression
            .and_then(|index| self.paths.get(index))
            .cloned()
        {
            failure.text = super::runtime_error_text_with_profile_and_color(
                &self.source,
                &self.uri,
                &self.name,
                expression,
                error,
                None,
                self.output_profile,
                self.use_color,
            );
        }
        failure.into()
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
    fn output_files_entry(&self) -> Result<usize, RuntimeError> {
        self.output_files_entry.ok_or_else(|| {
            RuntimeError::new(
                ErrorKind::Unsupported,
                "this source session was not compiled for module output",
            )
        })
    }
    fn output_file_keys_entry(&self) -> Result<usize, RuntimeError> {
        self.output_file_keys_entry.ok_or_else(|| {
            RuntimeError::new(
                ErrorKind::Unsupported,
                "this source session was not compiled for module output",
            )
        })
    }
    fn output_file_bytes_entry(&self) -> Result<usize, RuntimeError> {
        self.output_file_bytes_entry.ok_or_else(|| {
            RuntimeError::new(
                ErrorKind::Unsupported,
                "this source session was not compiled for module output",
            )
        })
    }
    fn release_module(&mut self) {
        self.module = None;
        self.host.set_session_root(None);
        self.calls.clear();
        self.public_queries.clear();
        self.setup_failure = None;
        self.last_completed_call = None;
        self.last_expression = None;
        self.awaiting_query_resolve = false;
        self.primary_source_cache = None;
        self.primary_source_id = None;
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
