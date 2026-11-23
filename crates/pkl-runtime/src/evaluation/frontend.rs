//! Shared frontend preparation and single-source module-output evaluation.
//!
//! The frontend-only adapter stops explicitly after preparation. The output
//! adapter evaluates the actual `output.bytes` property through the shared VM.
pub use super::output_files::{
    ModuleOutputFilesResult, OutputFileEntry, StreamingOutputFilesOutcome, check_path_spec,
    evaluate_output_files, evaluate_output_files_streaming_with_hosts,
    evaluate_output_files_with_glob_host, evaluate_output_files_with_host,
    evaluate_output_files_with_hosts, evaluate_output_files_with_platform_and_hosts,
};
pub use crate::diagnostic::FailureDisposition;
pub mod arguments;

use crate::diagnostic::{Diagnostic, DiagnosticOrigin, RenderFrame, render_error_text};
use crate::source::{SourceMap, SourceText};
use crate::value::PklString;
use std::sync::Arc;

/// Compiled source and provenance shared by actual module operations.
pub struct PreparedModule {
    pub program: Arc<crate::vm::Program>,
    pub source: crate::source::Source,
    pub name: PklString,
}

#[derive(Clone, Copy, Debug)]
pub struct ModuleSourceInput<'a> {
    pub text: &'a str,
    pub uri: &'a str,
    pub inferred_name: &'a str,
}

/// Lossless source boundary used by native and JavaScript adapters. The scalar
/// ModuleSourceInput remains an ergonomic wrapper for existing UTF-8 callers;
/// both enter the same parser, resolver and VM implementation.
#[derive(Clone, Debug)]
pub struct LosslessModuleSourceInput {
    pub text: SourceText,
    pub uri: PklString,
    pub inferred_name: PklString,
}

impl From<ModuleSourceInput<'_>> for LosslessModuleSourceInput {
    fn from(input: ModuleSourceInput<'_>) -> Self {
        Self {
            text: input.text.into(),
            uri: input.uri.into(),
            inferred_name: input.inferred_name.into(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiagnosticProfile {
    ApiIdentity,
    CliBundledSettings,
}

#[derive(Clone, Debug)]
pub struct ModuleEvalOptions {
    /// Whether Host-managed package caching is disabled. The bounded file/pkl
    /// Host does not persist package sources; other Host caching is not implied.
    pub no_cache: bool,
    pub no_project: bool,
    pub settings: String,
    pub properties: Vec<(String, String)>,
    pub environment: Vec<(String, String)>,
    pub diagnostic_profile: DiagnosticProfile,
}

impl Default for ModuleEvalOptions {
    fn default() -> Self {
        Self {
            no_cache: true,
            no_project: true,
            settings: "pkl:settings".into(),
            properties: Vec::new(),
            environment: Vec::new(),
            diagnostic_profile: DiagnosticProfile::CliBundledSettings,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FrontendPhase {
    Unclassified,
    Configuration,
    Lexing,
    Parsing,
    AstValidation,
    Resolution,
    Compilation,
    ModuleOutput,
}

impl FrontendPhase {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Unclassified => "unclassified",
            Self::Configuration => "configuration",
            Self::Lexing => "lexing",
            Self::Parsing => "parsing",
            Self::AstValidation => "ast-validation",
            Self::Resolution => "resolution",
            Self::Compilation => "compilation",
            Self::ModuleOutput => "module-output",
        }
    }
}

/// A rendered failure whose message retains all Java UTF-16 code units.
/// Byte-oriented adapters consume this value at their output boundary.
#[derive(Clone, Debug)]
pub struct ModuleFrontendFailure {
    pub phase: FrontendPhase,
    pub disposition: FailureDisposition,
    pub reason: Option<&'static str>,
    pub text: PklString,
}

/// Legacy byte-oriented frontend observation.
#[derive(Clone, Debug)]
pub struct ModuleFrontendResult {
    pub phase: FrontendPhase,
    pub disposition: FailureDisposition,
    pub reason: Option<&'static str>,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub exit_code: i32,
}

impl From<ModuleFrontendFailure> for ModuleFrontendResult {
    fn from(failure: ModuleFrontendFailure) -> Self {
        Self {
            phase: failure.phase,
            disposition: failure.disposition,
            reason: failure.reason,
            stdout: Vec::new(),
            stderr: failure.text.to_java_utf8().into_bytes(),
            // Development-probe outcomes, not the complete product CLI exit contract.
            exit_code: match failure.disposition {
                FailureDisposition::LanguageError | FailureDisposition::OracleError => 1,
                FailureDisposition::Unsupported => 2,
                FailureDisposition::LimitExceeded => 3,
                FailureDisposition::InternalFailure => 4,
            },
        }
    }
}

fn unsupported(phase: FrontendPhase, reason: &'static str, message: &str) -> ModuleFrontendFailure {
    ModuleFrontendFailure {
        phase,
        disposition: FailureDisposition::Unsupported,
        reason: Some(reason),
        text: render_error_text(&message.into(), &[], None),
    }
}

/// Preserve structured failure meaning independently of its plain-text rendering.
/// Ordered option maps belong to the evaluation request; the frontend prefix
/// does not resolve resources or evaluate declarations that consume those maps.
pub fn evaluate_module_frontend(
    input: impl Into<LosslessModuleSourceInput>,
    options: &ModuleEvalOptions,
) -> ModuleFrontendResult {
    let input = input.into();
    if let Err(error) = resolve_input(input, options, false) {
        return error.into();
    }
    unsupported(
        FrontendPhase::ModuleOutput,
        "module-output-evaluation",
        "Module output evaluation is not implemented by this frontend adapter.",
    )
    .into()
}

fn resolve_input(
    input: impl Into<LosslessModuleSourceInput>,
    options: &ModuleEvalOptions,
    defer_ast: bool,
) -> Result<(crate::resolver::ResolvedModule, PklString), ModuleFrontendFailure> {
    resolve_input_colored(input, options, defer_ast, false)
}

fn resolve_input_colored(
    input: impl Into<LosslessModuleSourceInput>,
    options: &ModuleEvalOptions,
    defer_ast: bool,
    use_color: bool,
) -> Result<(crate::resolver::ResolvedModule, PklString), ModuleFrontendFailure> {
    let input = input.into();
    // Java's moduleCacheDir controls package: caching (EvaluatorBuilder), while
    // File ModuleKey resolves the file directly. Preserve the original enabled
    // cache option for the bounded file/pkl profile; do not append --no-cache.
    // Remote/package cache behavior remains a Host responsibility and is not
    // admitted here for a cache-enabled non-file primary source.
    let file_or_bundled = input.uri.starts_with("file:") || input.uri.starts_with("pkl:");
    let has_project_allowed = options.no_project || crate::packages::resolver::has_active_project();
    if !has_project_allowed
        || options.settings != "pkl:settings"
        || (!options.no_cache && !file_or_bundled)
    {
        return Err(unsupported(
            FrontendPhase::Configuration,
            "module-evaluation-configuration",
            "This module-evaluation adapter requires --no-project or --project-dir and --settings pkl:settings; cache-enabled primary sources are limited to file: and pkl:.",
        ));
    }
    let module = match crate::parser::parse_module_for_evaluation(&input.text) {
        Ok(module) => module,
        Err(failure) => {
            return Err(render_module_parse_failure_text_colored(
                input,
                options.diagnostic_profile,
                *failure,
                use_color,
            ));
        }
    };
    if !defer_ast && let Some(error) = module.deferred_diagnostics.first() {
        return Err(render_failure_text_colored(
            input,
            options.diagnostic_profile,
            error.clone(),
            use_color,
        ));
    }
    let declared_name = module
        .declaration
        .as_ref()
        .and_then(|header| header.name.as_ref())
        .map(|parts| PklString::join(parts, "."));
    let resolved_input = LosslessModuleSourceInput {
        inferred_name: declared_name.unwrap_or_else(|| input.inferred_name.clone()),
        ..input.clone()
    };
    let resolved = if defer_ast {
        crate::resolver::resolve_module_for_evaluation_arc(Arc::new(module))
    } else {
        crate::resolver::resolve_module_owned(module).map_err(|error| {
            render_failure_text_colored(
                resolved_input.clone(),
                options.diagnostic_profile,
                error,
                use_color,
            )
        })?
    };
    Ok((resolved, resolved_input.inferred_name.to_owned()))
}

struct CachedPreparedModule {
    hash: u64,
    text: SourceText,
    uri: PklString,
    inferred_name: PklString,
    program: Arc<crate::vm::Program>,
    source: crate::source::Source,
    name: PklString,
}

fn prepared_module_cache() -> &'static std::sync::Mutex<Vec<CachedPreparedModule>> {
    static CACHE: std::sync::OnceLock<std::sync::Mutex<Vec<CachedPreparedModule>>> =
        std::sync::OnceLock::new();
    CACHE.get_or_init(|| std::sync::Mutex::new(Vec::with_capacity(32)))
}

/// Prepare actual execution without discarding structured frontend failures.
pub fn prepare_module(
    input: impl Into<LosslessModuleSourceInput>,
    options: &ModuleEvalOptions,
) -> Result<PreparedModule, ModuleFrontendResult> {
    let input = input.into();
    prepare_module_text(input, options).map_err(Into::into)
}

/// Prepare execution with lossless diagnostic text and structured failure meaning.
pub fn prepare_module_text(
    input: impl Into<LosslessModuleSourceInput>,
    options: &ModuleEvalOptions,
) -> Result<PreparedModule, ModuleFrontendFailure> {
    prepare_module_text_colored(input, options, false)
}

/// Prepare execution with optional ANSI-colored frontend diagnostics.
pub fn prepare_module_text_colored(
    input: impl Into<LosslessModuleSourceInput>,
    options: &ModuleEvalOptions,
    use_color: bool,
) -> Result<PreparedModule, ModuleFrontendFailure> {
    let input = input.into();
    let file_or_bundled = input.uri.starts_with("file:") || input.uri.starts_with("pkl:");
    let has_project_allowed = options.no_project || crate::packages::resolver::has_active_project();
    if !has_project_allowed
        || options.settings != "pkl:settings"
        || (!options.no_cache && !file_or_bundled)
    {
        return Err(unsupported(
            FrontendPhase::Configuration,
            "module-evaluation-configuration",
            "This module-evaluation adapter requires --no-project or --project-dir and --settings pkl:settings; cache-enabled primary sources are limited to file: and pkl:.",
        ));
    }
    let cacheable = input.text.len_bytes() <= 256 * 1024;
    let hash = if cacheable {
        crate::vm::modules::source_text_hash(&input.text)
    } else {
        0
    };
    if cacheable
        && let Ok(mut cache) = prepared_module_cache().lock()
        && let Some(pos) = cache.iter().rposition(|entry| {
            entry.hash == hash
                && entry.uri == input.uri
                && entry.inferred_name == input.inferred_name
                && entry.text == input.text
        })
    {
        let last = cache.len() - 1;
        if pos != last {
            let entry = cache.remove(pos);
            cache.push(entry);
        }
        let hit = &cache[last];
        return Ok(PreparedModule {
            program: Arc::clone(&hit.program),
            source: hit.source.clone(),
            name: hit.name.clone(),
        });
    }
    let (resolved, name) = resolve_input_colored(input.clone(), options, true, use_color)?;
    let mut sources = SourceMap::default();
    let id = sources.add(input.uri.clone(), input.text.clone());
    let source = sources.get(id).expect("source inserted").clone();
    let mut program =
        crate::vm::compile_module_with_power_assertions(&resolved).map_err(|error| {
            let mut result = render_module_runtime_failure_text_colored(
                &source,
                &input.uri,
                &name,
                options.diagnostic_profile,
                error,
                use_color,
            );
            result.phase = FrontendPhase::Compilation;
            result
        })?;
    Arc::make_mut(&mut program)
        .register_primary_source(
            Arc::new(crate::value::RuntimeSource {
                text: input.text.clone(),
                uri: input.uri.clone(),
                module_name: name.clone(),
            }),
            crate::modules::ModuleIdentity {
                original_uri: input.uri.clone(),
                resolved_uri: input.uri.clone(),
            },
        )
        .map_err(|error| {
            render_module_runtime_failure_text_colored(
                &source,
                &input.uri,
                &name,
                options.diagnostic_profile,
                error,
                use_color,
            )
        })?;
    if cacheable && let Ok(mut cache) = prepared_module_cache().lock() {
        if cache.len() >= 32 {
            cache.remove(0);
        }
        cache.push(CachedPreparedModule {
            hash,
            text: input.text,
            uri: input.uri,
            inferred_name: input.inferred_name,
            program: Arc::clone(&program),
            source: source.clone(),
            name: name.clone(),
        });
    }
    Ok(PreparedModule {
        program,
        source,
        name,
    })
}

/// Render an actual module-operation failure with no synthetic expression frame.
pub fn render_module_runtime_failure(
    source: &crate::source::Source,
    uri: impl Into<PklString>,
    name: impl Into<PklString>,
    profile: DiagnosticProfile,
    error: crate::value::RuntimeError,
) -> ModuleFrontendResult {
    let uri = uri.into();
    let name = name.into();

    render_module_runtime_failure_text(source, uri, name, profile, error).into()
}

/// Retain diagnostic code units and metadata for an actual module-operation failure.
pub fn render_module_runtime_failure_text(
    source: &crate::source::Source,
    uri: impl Into<PklString>,
    name: impl Into<PklString>,
    profile: DiagnosticProfile,
    error: crate::value::RuntimeError,
) -> ModuleFrontendFailure {
    render_module_runtime_failure_text_colored(source, uri, name, profile, error, false)
}

/// Retain diagnostic code units with optional ANSI color codes for a module-operation failure.
pub fn render_module_runtime_failure_text_colored(
    source: &crate::source::Source,
    uri: impl Into<PklString>,
    name: impl Into<PklString>,
    profile: DiagnosticProfile,
    error: crate::value::RuntimeError,
    use_color: bool,
) -> ModuleFrontendFailure {
    let uri = uri.into();
    let name = name.into();

    use crate::value::ErrorKind;
    if let Some(frontend) = &error.frontend {
        let mut disposition = frontend.diagnostic.provenance.disposition;
        if error.foreign_exception.is_some() {
            disposition = FailureDisposition::OracleError;
        }
        let phase = match frontend.diagnostic.provenance.origin {
            DiagnosticOrigin::Lexer => FrontendPhase::Lexing,
            DiagnosticOrigin::Parser => FrontendPhase::Parsing,
            DiagnosticOrigin::DeferredAstValidation => FrontendPhase::AstValidation,
            DiagnosticOrigin::Resolver => FrontendPhase::Resolution,
            DiagnosticOrigin::Unclassified => FrontendPhase::Unclassified,
        };
        return ModuleFrontendFailure {
            phase,
            disposition,
            reason: frontend.diagnostic.provenance.reason,
            text: super::runtime_error_text_with_profile_and_color(
                source, &uri, &name, "", error, None, profile, use_color,
            ),
        };
    }
    let (disposition, reason) = if error.foreign_exception.is_some() {
        (FailureDisposition::OracleError, None)
    } else {
        match error.kind {
            ErrorKind::Unsupported => (FailureDisposition::Unsupported, Some("runtime-capability")),
            ErrorKind::InvalidBytecode | ErrorKind::StaleHandle => (
                FailureDisposition::InternalFailure,
                Some("runtime-invariant"),
            ),
            _ => (FailureDisposition::LanguageError, None),
        }
    };
    ModuleFrontendFailure {
        phase: FrontendPhase::ModuleOutput,
        disposition,
        reason,
        text: super::runtime_error_text_with_profile_and_color(
            source, &uri, &name, "", error, None, profile, use_color,
        ),
    }
}

fn render_module_parse_failure_text_colored(
    input: LosslessModuleSourceInput,
    profile: DiagnosticProfile,
    failure: crate::parser::ModuleParseFailure,
    use_color: bool,
) -> ModuleFrontendFailure {
    if let Some(error) = failure.minimum_version_error(&input.inferred_name) {
        let mut sources = SourceMap::default();
        let id = sources.add(input.uri.clone(), input.text);
        let source = sources.get(id).expect("source inserted");
        return render_module_runtime_failure_text_colored(
            source,
            input.uri,
            input.inferred_name,
            profile,
            error,
            use_color,
        );
    }
    render_failure_text_colored(input, profile, failure.diagnostic, use_color)
}

fn render_failure_text_colored(
    input: impl Into<LosslessModuleSourceInput>,
    profile: DiagnosticProfile,
    error: Diagnostic,
    use_color: bool,
) -> ModuleFrontendFailure {
    let input = input.into();
    let phase = match error.provenance.origin {
        DiagnosticOrigin::Lexer => FrontendPhase::Lexing,
        DiagnosticOrigin::Parser => FrontendPhase::Parsing,
        DiagnosticOrigin::DeferredAstValidation => FrontendPhase::AstValidation,
        DiagnosticOrigin::Resolver => FrontendPhase::Resolution,
        DiagnosticOrigin::Unclassified => FrontendPhase::Unclassified,
    };
    let mut sources = SourceMap::default();
    let id = sources.add(input.uri.clone(), input.text.clone());
    let source = sources.get(id).expect("source inserted");
    let name = error.module_name.as_deref().unwrap_or(&input.inferred_name);
    let member = error.member.as_ref().map_or_else(
        || PklString::from(name),
        |suffix| {
            let separator = if suffix.encode_utf16().next() == Some(46) {
                ""
            } else {
                "#"
            };
            name.concat(&separator.into()).concat(suffix)
        },
    );
    // Pinned bundled settings use `%{url}, line %{line}` for file URLs.
    // Transform the logical frame before rendering; never edit rendered bytes.
    let display_uri = match profile {
        DiagnosticProfile::CliBundledSettings if input.uri.starts_with("file:") => {
            input.uri.concat(
                &format!(
                    ", line {}",
                    source
                        .position(error.span.start)
                        .map_or(1, |position| position.line)
                )
                .into(),
            )
        }
        _ => input.uri.clone(),
    };
    ModuleFrontendFailure {
        phase,
        disposition: error.provenance.disposition,
        reason: error.provenance.reason,
        text: crate::diagnostic::render_frontend_error_text_colored(
            &error,
            &[RenderFrame {
                source,
                uri: display_uri,
                member: &member,
                span: error.span,
                highlight_length_utf16: error.highlight_length_utf16,
            }],
            use_color,
        ),
    }
}

/// Actual output-operation result. Success is distinct from every failure disposition.
#[derive(Clone, Debug)]
pub struct ModuleOutputResult {
    pub phase: FrontendPhase,
    pub disposition: Option<FailureDisposition>,
    pub reason: Option<&'static str>,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub exit_code: i32,
}
impl From<ModuleFrontendResult> for ModuleOutputResult {
    fn from(value: ModuleFrontendResult) -> Self {
        Self {
            phase: value.phase,
            disposition: Some(value.disposition),
            reason: value.reason,
            stdout: value.stdout,
            stderr: value.stderr,
            exit_code: value.exit_code,
        }
    }
}

/// Single-source development output evaluator. Resource effects use the shared
/// configured policy; unavailable module/resource capabilities remain explicit.
pub fn evaluate_module_output(
    input: impl Into<LosslessModuleSourceInput>,
    options: &ModuleEvalOptions,
) -> ModuleOutputResult {
    let input = input.into();
    evaluate_module_output_with_host(
        input,
        options,
        crate::modules::default_or_unsupported_completion,
    )
}

/// Evaluate actual output with module resolution/source I/O supplied by the Host.
/// Parsing, code installation, identity, caching and initialization stay in the VM.
pub fn evaluate_module_output_with_host(
    input: impl Into<LosslessModuleSourceInput>,
    options: &ModuleEvalOptions,
    module_host: impl FnMut(&crate::modules::ModuleRequest) -> crate::modules::ModuleCompletion,
) -> ModuleOutputResult {
    let input = input.into();
    evaluate_module_output_with_hosts(input, options, module_host, |_| None)
}

/// Evaluate with typed external resource reads in addition to module I/O.
/// Returning None delegates to the shared configured property/environment policy.
pub fn evaluate_module_output_with_hosts(
    input: impl Into<LosslessModuleSourceInput>,
    options: &ModuleEvalOptions,
    module_host: impl FnMut(&crate::modules::ModuleRequest) -> crate::modules::ModuleCompletion,
    resource_host: impl FnMut(
        &crate::vm::ReadRequest,
    )
        -> Option<Result<crate::resources::ReadResult, crate::host::ReadFailure>>,
) -> ModuleOutputResult {
    let input = input.into();
    evaluate_module_output_with_glob_host(input, options, module_host, resource_host, |_| None)
}

/// Evaluate with explicit typed glob enumeration in addition to module and read
/// effects. `None` delegates to configured env/prop readers or typed Unsupported.
pub fn evaluate_module_output_with_glob_host(
    input: impl Into<LosslessModuleSourceInput>,
    options: &ModuleEvalOptions,
    module_host: impl FnMut(&crate::modules::ModuleRequest) -> crate::modules::ModuleCompletion,
    resource_host: impl FnMut(
        &crate::vm::ReadRequest,
    )
        -> Option<Result<crate::resources::ReadResult, crate::host::ReadFailure>>,
    glob_host: impl FnMut(&crate::glob::GlobRequest) -> Option<crate::glob::GlobCompletion>,
) -> ModuleOutputResult {
    evaluate_module_output_with_platform_and_hosts(
        input,
        options,
        None,
        module_host,
        resource_host,
        glob_host,
    )
}

/// Evaluate `Module.output.bytes` with explicit optional Host `PlatformMetadata`.
pub fn evaluate_module_output_with_platform_and_hosts(
    input: impl Into<LosslessModuleSourceInput>,
    options: &ModuleEvalOptions,
    platform_metadata: Option<crate::logging::PlatformMetadata>,
    module_host: impl FnMut(&crate::modules::ModuleRequest) -> crate::modules::ModuleCompletion,
    resource_host: impl FnMut(
        &crate::vm::ReadRequest,
    )
        -> Option<Result<crate::resources::ReadResult, crate::host::ReadFailure>>,
    glob_host: impl FnMut(&crate::glob::GlobRequest) -> Option<crate::glob::GlobCompletion>,
) -> ModuleOutputResult {
    let input = input.into();
    evaluate_module_output_with_budget_and_hosts(
        input,
        options,
        platform_metadata,
        1_048_576,
        module_host,
        resource_host,
        glob_host,
    )
}

/// Evaluate a CLI `-x`/`--expression` string (`EvaluatorImpl.evaluateExpressionString`)
/// with optional Host `PlatformMetadata` and Host callbacks.
pub fn evaluate_module_expression_with_platform_and_hosts(
    input: impl Into<LosslessModuleSourceInput>,
    expression: impl Into<SourceText>,
    options: &ModuleEvalOptions,
    platform_metadata: Option<crate::logging::PlatformMetadata>,
    mut module_host: impl FnMut(&crate::modules::ModuleRequest) -> crate::modules::ModuleCompletion,
    mut resource_host: impl FnMut(
        &crate::vm::ReadRequest,
    ) -> Option<
        Result<crate::resources::ReadResult, crate::host::ReadFailure>,
    >,
    mut glob_host: impl FnMut(&crate::glob::GlobRequest) -> Option<crate::glob::GlobCompletion>,
) -> ModuleOutputResult {
    use super::session::{SourceSession, SourceSessionOptions};
    use crate::host::HostPoll;
    let input = input.into();
    let mut session = match SourceSession::compile_output_expression_text(
        input,
        expression,
        options,
        SourceSessionOptions {
            evaluation: super::EvaluationOptions {
                power_assertions: true,
            },
            trace_mode: crate::packages::resolver::current_trace_mode()
                .unwrap_or(crate::logging::TraceMode::Compact),
            log_delivery: crate::logging::LogDelivery::Host,
            frame_transformer: if options.diagnostic_profile
                == DiagnosticProfile::CliBundledSettings
            {
                crate::logging::FrameTransformer::Host
            } else {
                crate::logging::FrameTransformer::Default
            },
            platform_data: options.diagnostic_profile == DiagnosticProfile::CliBundledSettings,
        },
    ) {
        Ok(session) => session,
        Err(error) => {
            let error: ModuleFrontendResult = error.into();
            return error.into();
        }
    };
    if let Some(metadata) = platform_metadata {
        session.set_platform_metadata(Some(metadata));
    }
    let mut stderr = Vec::new();
    let mut budget_exhausted = false;
    let run = (|| {
        session.start_path(0, 0, None)?;
        for _ in 0..1_048_576 {
            match session.poll(0, 1024) {
                HostPoll::Ready(result) => return result.and_then(SourceSession::result_string),
                HostPoll::Yielded => {}
                HostPoll::Module { token, request } => {
                    session.complete_module(&token, module_host(&request), 0)?
                }
                HostPoll::Glob { token, request } => {
                    let completion = glob_host(&request).unwrap_or_else(|| {
                        crate::glob::configured_or_unsupported_completion(
                            &request,
                            &options.properties,
                            &options.environment,
                        )
                    });
                    session.complete_glob(&token, completion, 0)?;
                }
                HostPoll::Read { token, .. } => {
                    let request = session.read_request(&token)?;
                    if let Some(result) = resource_host(&request) {
                        session.complete_read(&token, result, 0)?;
                    } else {
                        session.resume_default_read(
                            &token,
                            &options.properties,
                            &options.environment,
                            0,
                        )?;
                    }
                }
                HostPoll::TransformFrame { token, frame } => {
                    let mut frame =
                        crate::logging::FrameTransformer::Default.transform((*frame).clone());
                    if frame.module_uri.to_java_utf8().starts_with("file:") {
                        frame.module_uri = frame
                            .module_uri
                            .concat(&format!(", line {}", frame.start_line).into());
                    }
                    session.complete_frame_transform(&token, Ok(frame), 0)?;
                }
                HostPoll::Log { token, event } => {
                    let level = match event.level {
                        crate::logging::LogLevel::Trace => "TRACE",
                        crate::logging::LogLevel::Warn => "WARN",
                    };
                    let uri = event.frame.module_uri.to_java_utf8();
                    let message = event.message.to_java_utf8();
                    let space = if message.ends_with('\n') { "" } else { " " };
                    stderr.extend_from_slice(
                        format!("pkl: {level}: {message}{space}({uri})\n").as_bytes(),
                    );
                    session.acknowledge_log(&token, Ok(()), 0)?;
                }
            }
        }
        budget_exhausted = true;
        session.cancel();
        Err(crate::value::RuntimeError::new(
            crate::value::ErrorKind::Unsupported,
            "Module output instruction budget exceeded.",
        ))
    })();
    let mut result = match run {
        Ok(text) => ModuleOutputResult {
            phase: FrontendPhase::ModuleOutput,
            disposition: None,
            reason: None,
            stdout: text.to_java_utf8().into_bytes(),
            stderr: Vec::new(),
            exit_code: 0,
        },
        Err(error) => session.output_expression_failure(error).into(),
    };
    if budget_exhausted {
        result.disposition = Some(FailureDisposition::LimitExceeded);
        result.reason = Some("instruction-budget");
        result.exit_code = 3;
    }
    stderr.extend_from_slice(&result.stderr);
    result.stderr = stderr;
    session.close();
    result
}

#[cfg(test)]
fn evaluate_module_output_with_budget(
    input: impl Into<LosslessModuleSourceInput>,
    options: &ModuleEvalOptions,
    poll_budget: usize,
) -> ModuleOutputResult {
    let input = input.into();
    evaluate_module_output_with_budget_and_hosts(
        input,
        options,
        None,
        poll_budget,
        crate::modules::default_or_unsupported_completion,
        |_| None,
        |_| None,
    )
}

fn evaluate_module_output_with_budget_and_hosts(
    input: impl Into<LosslessModuleSourceInput>,
    options: &ModuleEvalOptions,
    platform_metadata: Option<crate::logging::PlatformMetadata>,
    poll_budget: usize,
    mut module_host: impl FnMut(&crate::modules::ModuleRequest) -> crate::modules::ModuleCompletion,
    mut resource_host: impl FnMut(
        &crate::vm::ReadRequest,
    ) -> Option<
        Result<crate::resources::ReadResult, crate::host::ReadFailure>,
    >,
    mut glob_host: impl FnMut(&crate::glob::GlobRequest) -> Option<crate::glob::GlobCompletion>,
) -> ModuleOutputResult {
    let input = input.into();
    use super::session::{SourceSession, SourceSessionOptions};
    use crate::host::HostPoll;
    let mut session = match SourceSession::compile_output(
        input,
        &[],
        options,
        SourceSessionOptions {
            evaluation: super::EvaluationOptions {
                power_assertions: true,
            },
            trace_mode: crate::packages::resolver::current_trace_mode()
                .unwrap_or(crate::logging::TraceMode::Compact),
            log_delivery: crate::logging::LogDelivery::Host,
            // CLI settings transform reflection locations as well as log and
            // error frames. Keep the API default independent of editor settings.
            frame_transformer: if options.diagnostic_profile
                == DiagnosticProfile::CliBundledSettings
            {
                crate::logging::FrameTransformer::Host
            } else {
                crate::logging::FrameTransformer::Default
            },
            platform_data: options.diagnostic_profile == DiagnosticProfile::CliBundledSettings,
        },
    ) {
        Ok(session) => session,
        Err(error) => return error.into(),
    };
    if let Some(metadata) = platform_metadata {
        session.set_platform_metadata(Some(metadata));
    }
    let mut stderr = Vec::new();
    let mut budget_exhausted = false;
    let run = (|| {
        session.start_output_bytes(0, None)?;
        for _ in 0..poll_budget {
            match session.poll(0, 1024) {
                HostPoll::Ready(result) => return result.and_then(SourceSession::result_bytes),
                HostPoll::Yielded => {}
                HostPoll::Module { token, request } => {
                    session.complete_module(&token, module_host(&request), 0)?
                }
                HostPoll::Glob { token, request } => {
                    let completion = glob_host(&request).unwrap_or_else(|| {
                        crate::glob::configured_or_unsupported_completion(
                            &request,
                            &options.properties,
                            &options.environment,
                        )
                    });
                    session.complete_glob(&token, completion, 0)?;
                }
                HostPoll::Read { token, .. } => {
                    let request = session.read_request(&token)?;
                    if let Some(result) = resource_host(&request) {
                        session.complete_read(&token, result, 0)?;
                    } else {
                        session.resume_default_read(
                            &token,
                            &options.properties,
                            &options.environment,
                            0,
                        )?;
                    }
                }
                HostPoll::TransformFrame { token, frame } => {
                    let mut frame =
                        crate::logging::FrameTransformer::Default.transform((*frame).clone());
                    // settings.System.urlScheme is "%{url}, line %{line}".
                    // Apply it through the same Host continuation used by the
                    // public evaluator, before reflection caches its result.
                    if frame.module_uri.to_java_utf8().starts_with("file:") {
                        frame.module_uri = frame
                            .module_uri
                            .concat(&format!(", line {}", frame.start_line).into());
                    }
                    session.complete_frame_transform(&token, Ok(frame), 0)?;
                }
                HostPoll::Log { token, event } => {
                    let level = match event.level {
                        crate::logging::LogLevel::Trace => "TRACE",
                        crate::logging::LogLevel::Warn => "WARN",
                    };
                    let uri = event.frame.module_uri.to_java_utf8();
                    let message = event.message.to_java_utf8();
                    let space = if message.ends_with('\n') { "" } else { " " };
                    stderr.extend_from_slice(
                        format!("pkl: {level}: {message}{space}({uri})\n").as_bytes(),
                    );
                    session.acknowledge_log(&token, Ok(()), 0)?;
                }
            }
        }
        budget_exhausted = true;
        session.cancel();
        Err(crate::value::RuntimeError::new(
            crate::value::ErrorKind::Unsupported,
            "Module output instruction budget exceeded.",
        ))
    })();
    let mut result = match run {
        Ok(bytes) => ModuleOutputResult {
            phase: FrontendPhase::ModuleOutput,
            disposition: None,
            reason: None,
            stdout: bytes.to_vec(),
            stderr: Vec::new(),
            exit_code: 0,
        },
        Err(error) => session.output_failure(error).into(),
    };
    if budget_exhausted {
        result.disposition = Some(FailureDisposition::LimitExceeded);
        result.reason = Some("instruction-budget");
        result.exit_code = 3;
    }
    stderr.extend_from_slice(&result.stderr);
    result.stderr = stderr;
    session.close();
    result
}

#[cfg(test)]
mod output_budget_tests {
    use super::*;

    #[test]
    fn file_output_keeps_the_package_cache_switch_without_changing_raw_output() {
        let input = ModuleSourceInput {
            text: "output { text = \"plain file\" }",
            uri: "file:///cache-policy.pkl",
            inferred_name: "cachePolicy",
        };
        let uncached = evaluate_module_output(input, &ModuleEvalOptions::default());
        let cached = evaluate_module_output(
            input,
            &ModuleEvalOptions {
                no_cache: false,
                ..Default::default()
            },
        );
        assert_eq!(uncached.exit_code, 0);
        assert_eq!(cached.exit_code, uncached.exit_code);
        assert_eq!(cached.stdout, uncached.stdout);
        assert_eq!(cached.stderr, uncached.stderr);
    }

    #[test]
    fn enabled_cache_does_not_admit_project_or_remote_primary_profiles() {
        let input = || ModuleSourceInput {
            text: "x = 1",
            uri: "file:///cache-policy.pkl",
            inferred_name: "cachePolicy",
        };
        assert!(
            resolve_input(
                input(),
                &ModuleEvalOptions {
                    no_cache: false,
                    no_project: false,
                    ..Default::default()
                },
                false
            )
            .is_err()
        );
        assert!(
            resolve_input(
                ModuleSourceInput {
                    uri: "https://example.test/cache-policy.pkl",
                    ..input()
                },
                &ModuleEvalOptions {
                    no_cache: false,
                    ..Default::default()
                },
                false
            )
            .is_err()
        );
    }

    #[test]
    fn reflection_locations_use_cli_editor_settings_without_changing_api_defaults() {
        let input = ModuleSourceInput {
            text: "\nmodule example\noutput { text = import(\"pkl:reflect\").Module(module).location.displayUri }",
            uri: "file:///reflection-location.pkl",
            inferred_name: "example",
        };
        for (profile, expected) in [
            (
                DiagnosticProfile::CliBundledSettings,
                "file:///reflection-location.pkl, line 2",
            ),
            (
                DiagnosticProfile::ApiIdentity,
                "file:///reflection-location.pkl",
            ),
        ] {
            let result = evaluate_module_output(
                input,
                &ModuleEvalOptions {
                    diagnostic_profile: profile,
                    ..Default::default()
                },
            );
            assert_eq!(result.exit_code, 0, "{:?}", result.stderr);
            assert_eq!(result.stdout, expected.as_bytes());
            assert!(result.stderr.is_empty());
        }
    }

    #[test]
    fn cli_trace_uses_the_transformed_file_location_exactly_once() {
        let result = evaluate_module_output(
            ModuleSourceInput {
                text: "output { text = trace(42).toString() }",
                uri: "file:///cli-policy.pkl",
                inferred_name: "example",
            },
            &ModuleEvalOptions::default(),
        );
        assert_eq!(result.exit_code, 0, "{:?}", result.stderr);
        assert_eq!(result.stdout, b"42");
        assert_eq!(
            result.stderr,
            b"pkl: TRACE: 42 = 42 (file:///cli-policy.pkl, line 1)\n"
        );
    }

    #[test]
    fn reflection_module_of_skips_an_object_amendment_without_its_own_module_info() {
        let result = evaluate_module_output(
            ModuleSourceInput {
                text: r#"import "pkl:reflect"
marker = false
hidden amended = (module) { marker = true }
output { text = (reflect.Module(module) == reflect.moduleOf(amended)).toString() }
"#,
                uri: "file:///reflection-module.pkl",
                inferred_name: "example",
            },
            &ModuleEvalOptions::default(),
        );
        assert_eq!(result.exit_code, 0, "{:?}", result.stderr);
        assert_eq!(result.stdout, b"true");
        assert!(result.stderr.is_empty());
    }

    #[test]
    fn interrupted_output_is_a_limit_failure_and_never_partial_success() {
        let result = evaluate_module_output_with_budget(
            ModuleSourceInput {
                text: "result = 42",
                uri: "file:///output.pkl",
                inferred_name: "output",
            },
            &ModuleEvalOptions::default(),
            0,
        );
        assert_eq!(result.phase, FrontendPhase::ModuleOutput);
        assert_eq!(result.disposition, Some(FailureDisposition::LimitExceeded));
        assert_eq!(result.reason, Some("instruction-budget"));
        assert_eq!(result.exit_code, 3);
        assert!(result.stdout.is_empty());
        assert!(!result.stderr.is_empty());
    }
}
