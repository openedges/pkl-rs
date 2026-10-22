//! Shared frontend preparation and single-source module-output evaluation.
//!
//! The frontend-only adapter stops explicitly after preparation. The output
//! adapter evaluates the actual `output.bytes` property through the shared VM.
pub use crate::diagnostic::FailureDisposition;
pub mod arguments;

use crate::diagnostic::{Diagnostic, DiagnosticOrigin, RenderFrame, render_error};
use crate::source::SourceMap;
use crate::value::PklString;
use std::sync::Arc;

/// Compiled source and provenance shared by actual module operations.
pub struct PreparedModule {
    pub program: Arc<crate::vm::Program>,
    pub source: crate::source::Source,
    pub name: String,
}

#[derive(Clone, Copy, Debug)]
pub struct ModuleSourceInput<'a> {
    pub text: &'a str,
    pub uri: &'a str,
    pub inferred_name: &'a str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiagnosticProfile {
    ApiIdentity,
    CliBundledSettings,
}

#[derive(Clone, Debug)]
pub struct ModuleEvalOptions {
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

#[derive(Clone, Debug)]
pub struct ModuleFrontendResult {
    pub phase: FrontendPhase,
    pub disposition: FailureDisposition,
    pub reason: Option<&'static str>,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub exit_code: i32,
}

fn unsupported(phase: FrontendPhase, reason: &'static str, message: &str) -> ModuleFrontendResult {
    ModuleFrontendResult {
        phase,
        disposition: FailureDisposition::Unsupported,
        reason: Some(reason),
        stdout: Vec::new(),
        stderr: render_error(message, &[], None).into_bytes(),
        exit_code: 2,
    }
}

/// Preserve structured failure meaning independently of its plain-text rendering.
/// Ordered option maps belong to the evaluation request; the frontend prefix
/// does not resolve resources or evaluate declarations that consume those maps.
pub fn evaluate_module_frontend(
    input: ModuleSourceInput<'_>,
    options: &ModuleEvalOptions,
) -> ModuleFrontendResult {
    if let Err(error) = resolve_input(input, options) {
        return error;
    }
    unsupported(
        FrontendPhase::ModuleOutput,
        "module-output-evaluation",
        "Module output evaluation is not implemented by this frontend adapter.",
    )
}

fn resolve_input(
    input: ModuleSourceInput<'_>,
    options: &ModuleEvalOptions,
) -> Result<(crate::resolver::ResolvedModule, String), ModuleFrontendResult> {
    if !options.no_cache || !options.no_project || options.settings != "pkl:settings" {
        return Err(unsupported(
            FrontendPhase::Configuration,
            "module-evaluation-configuration",
            "This module-evaluation adapter requires --no-cache --no-project --settings pkl:settings.",
        ));
    }
    let module = match crate::parser::parse_module(input.text) {
        Ok(module) => module,
        Err(error) => return Err(render_failure(input, options.diagnostic_profile, error)),
    };
    let declared_name = module
        .declaration
        .as_ref()
        .and_then(|header| header.name.as_ref())
        .map(|parts| parts.join("."));
    let resolved_input = ModuleSourceInput {
        inferred_name: declared_name.as_deref().unwrap_or(input.inferred_name),
        ..input
    };
    let resolved = crate::resolver::resolve_module(&module)
        .map_err(|error| render_failure(resolved_input, options.diagnostic_profile, error))?;
    Ok((resolved, resolved_input.inferred_name.to_owned()))
}

/// Prepare actual execution without discarding structured frontend failures.
pub fn prepare_module(
    input: ModuleSourceInput<'_>,
    options: &ModuleEvalOptions,
) -> Result<PreparedModule, ModuleFrontendResult> {
    let (resolved, name) = resolve_input(input, options)?;
    let mut sources = SourceMap::default();
    let id = sources.add(input.uri, input.text);
    let source = sources.get(id).expect("source inserted").clone();
    let mut program =
        crate::vm::compile_module_with_power_assertions(&resolved).map_err(|error| {
            let mut result = render_module_runtime_failure(
                &source,
                input.uri,
                &name,
                options.diagnostic_profile,
                error,
            );
            result.phase = FrontendPhase::Compilation;
            result
        })?;
    Arc::make_mut(&mut program).module_name = name.clone().into();
    Ok(PreparedModule {
        program,
        source,
        name,
    })
}

/// Render an actual module-operation failure with no synthetic expression frame.
pub fn render_module_runtime_failure(
    source: &crate::source::Source,
    uri: &str,
    name: &str,
    profile: DiagnosticProfile,
    error: crate::value::RuntimeError,
) -> ModuleFrontendResult {
    use crate::value::ErrorKind;
    let (disposition, exit_code, reason) = match error.kind {
        ErrorKind::Unsupported => (
            FailureDisposition::Unsupported,
            2,
            Some("runtime-capability"),
        ),
        ErrorKind::InvalidBytecode | ErrorKind::StaleHandle => (
            FailureDisposition::InternalFailure,
            4,
            Some("runtime-invariant"),
        ),
        _ => (FailureDisposition::LanguageError, 1, None),
    };
    ModuleFrontendResult {
        phase: FrontendPhase::ModuleOutput,
        disposition,
        reason,
        stdout: Vec::new(),
        stderr: super::runtime_error_with_profile(source, uri, name, "", error, None, profile)
            .into_bytes(),
        exit_code,
    }
}

fn render_failure(
    input: ModuleSourceInput<'_>,
    profile: DiagnosticProfile,
    error: Diagnostic,
) -> ModuleFrontendResult {
    let phase = match error.provenance.origin {
        DiagnosticOrigin::Lexer => FrontendPhase::Lexing,
        DiagnosticOrigin::Parser => FrontendPhase::Parsing,
        DiagnosticOrigin::DeferredAstValidation => FrontendPhase::AstValidation,
        DiagnosticOrigin::Resolver => FrontendPhase::Resolution,
        DiagnosticOrigin::Unclassified => FrontendPhase::Unclassified,
    };
    let mut sources = SourceMap::default();
    let id = sources.add(input.uri, input.text);
    let source = sources.get(id).expect("source inserted");
    let name = error.module_name.as_deref().unwrap_or(input.inferred_name);
    let member = error.member.as_ref().map_or_else(
        || PklString::from(name),
        |suffix| PklString::from(format!("{name}#")).concat(suffix),
    );
    // Pinned bundled settings use `%{url}, line %{line}` for file URLs.
    // Transform the logical frame before rendering; never edit rendered bytes.
    let display_uri = match profile {
        DiagnosticProfile::CliBundledSettings if input.uri.starts_with("file:") => {
            format!(
                "{}, line {}",
                input.uri,
                source
                    .position(error.span.start)
                    .map_or(1, |position| position.line)
            )
        }
        _ => input.uri.to_owned(),
    };
    ModuleFrontendResult {
        phase,
        disposition: error.provenance.disposition,
        reason: error.provenance.reason,
        stdout: Vec::new(),
        stderr: render_error(
            &error.message,
            &[RenderFrame {
                source,
                uri: &display_uri,
                member: &member,
                span: error.span,
                highlight_length_utf16: error.highlight_length_utf16,
            }],
            error.hint(),
        )
        .into_bytes(),
        // Development-probe outcomes, not the complete product CLI exit contract.
        exit_code: match error.provenance.disposition {
            FailureDisposition::LanguageError => 1,
            FailureDisposition::Unsupported => 2,
            FailureDisposition::LimitExceeded => 3,
            FailureDisposition::InternalFailure => 4,
        },
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
    input: ModuleSourceInput<'_>,
    options: &ModuleEvalOptions,
) -> ModuleOutputResult {
    evaluate_module_output_with_budget(input, options, 1_048_576)
}

fn evaluate_module_output_with_budget(
    input: ModuleSourceInput<'_>,
    options: &ModuleEvalOptions,
    poll_budget: usize,
) -> ModuleOutputResult {
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
            log_delivery: crate::logging::LogDelivery::Host,
            ..Default::default()
        },
    ) {
        Ok(session) => session,
        Err(error) => return error.into(),
    };
    let mut stderr = Vec::new();
    let mut budget_exhausted = false;
    let run = (|| {
        session.start_output_bytes(0, None)?;
        for _ in 0..poll_budget {
            match session.poll(0, 1024) {
                HostPoll::Ready(result) => return result.and_then(SourceSession::result_bytes),
                HostPoll::Yielded => {}
                HostPoll::Read { token, .. } => session.resume_default_read(
                    &token,
                    &options.properties,
                    &options.environment,
                    0,
                )?,
                HostPoll::Log { token, event } => {
                    let level = match event.level {
                        crate::logging::LogLevel::Trace => "TRACE",
                        crate::logging::LogLevel::Warn => "WARN",
                    };
                    let mut uri = event.frame.module_uri.to_java_utf8();
                    if options.diagnostic_profile == DiagnosticProfile::CliBundledSettings
                        && uri.starts_with("file:")
                    {
                        uri = format!("{uri}, line {}", event.frame.start_line);
                    }
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
