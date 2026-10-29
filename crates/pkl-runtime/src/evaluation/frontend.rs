//! Shared frontend preparation and single-source module-output evaluation.
//!
//! The frontend-only adapter stops explicitly after preparation. The output
//! adapter evaluates the actual `output.bytes` property through the shared VM.
pub use crate::diagnostic::FailureDisposition;
pub mod arguments;

use crate::diagnostic::{Diagnostic, DiagnosticOrigin, RenderFrame, render_error_text};
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
                FailureDisposition::LanguageError => 1,
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
    input: ModuleSourceInput<'_>,
    options: &ModuleEvalOptions,
) -> ModuleFrontendResult {
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
    input: ModuleSourceInput<'_>,
    options: &ModuleEvalOptions,
    defer_ast: bool,
) -> Result<(crate::resolver::ResolvedModule, String), ModuleFrontendFailure> {
    if !options.no_cache || !options.no_project || options.settings != "pkl:settings" {
        return Err(unsupported(
            FrontendPhase::Configuration,
            "module-evaluation-configuration",
            "This module-evaluation adapter requires --no-cache --no-project --settings pkl:settings.",
        ));
    }
    let parsed = if defer_ast {
        crate::parser::parse_module_for_evaluation(input.text)
    } else {
        crate::parser::parse_module(input.text)
    };
    let module = match parsed {
        Ok(module) => module,
        Err(error) => {
            return Err(render_failure_text(
                input,
                options.diagnostic_profile,
                error,
            ));
        }
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
    let resolved = if defer_ast {
        crate::resolver::resolve_module_for_evaluation(&module)
    } else {
        crate::resolver::resolve_module(&module).map_err(|error| {
            render_failure_text(resolved_input, options.diagnostic_profile, error)
        })?
    };
    Ok((resolved, resolved_input.inferred_name.to_owned()))
}

/// Prepare actual execution without discarding structured frontend failures.
pub fn prepare_module(
    input: ModuleSourceInput<'_>,
    options: &ModuleEvalOptions,
) -> Result<PreparedModule, ModuleFrontendResult> {
    prepare_module_text(input, options).map_err(Into::into)
}

/// Prepare execution with lossless diagnostic text and structured failure meaning.
pub fn prepare_module_text(
    input: ModuleSourceInput<'_>,
    options: &ModuleEvalOptions,
) -> Result<PreparedModule, ModuleFrontendFailure> {
    let (resolved, name) = resolve_input(input, options, true)?;
    let mut sources = SourceMap::default();
    let id = sources.add(input.uri, input.text);
    let source = sources.get(id).expect("source inserted").clone();
    let mut program =
        crate::vm::compile_module_with_power_assertions(&resolved).map_err(|error| {
            let mut result = render_module_runtime_failure_text(
                &source,
                input.uri,
                &name,
                options.diagnostic_profile,
                error,
            );
            result.phase = FrontendPhase::Compilation;
            result
        })?;
    Arc::make_mut(&mut program)
        .register_primary_source(
            Arc::new(crate::value::RuntimeSource {
                text: input.text.into(),
                uri: input.uri.into(),
                module_name: name.clone().into(),
            }),
            crate::modules::ModuleIdentity {
                original_uri: input.uri.into(),
                resolved_uri: input.uri.into(),
            },
        )
        .map_err(|error| {
            render_module_runtime_failure_text(
                &source,
                input.uri,
                &name,
                options.diagnostic_profile,
                error,
            )
        })?;
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
    render_module_runtime_failure_text(source, uri, name, profile, error).into()
}

/// Retain diagnostic code units and metadata for an actual module-operation failure.
pub fn render_module_runtime_failure_text(
    source: &crate::source::Source,
    uri: &str,
    name: &str,
    profile: DiagnosticProfile,
    error: crate::value::RuntimeError,
) -> ModuleFrontendFailure {
    use crate::value::ErrorKind;
    if let Some(frontend) = &error.frontend {
        let disposition = frontend.diagnostic.provenance.disposition;
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
            text: super::runtime_error_text_with_profile(
                source, uri, name, "", error, None, profile,
            ),
        };
    }
    let (disposition, reason) = match error.kind {
        ErrorKind::Unsupported => (FailureDisposition::Unsupported, Some("runtime-capability")),
        ErrorKind::InvalidBytecode | ErrorKind::StaleHandle => (
            FailureDisposition::InternalFailure,
            Some("runtime-invariant"),
        ),
        _ => (FailureDisposition::LanguageError, None),
    };
    ModuleFrontendFailure {
        phase: FrontendPhase::ModuleOutput,
        disposition,
        reason,
        text: super::runtime_error_text_with_profile(source, uri, name, "", error, None, profile),
    }
}

fn render_failure_text(
    input: ModuleSourceInput<'_>,
    profile: DiagnosticProfile,
    error: Diagnostic,
) -> ModuleFrontendFailure {
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
    ModuleFrontendFailure {
        phase,
        disposition: error.provenance.disposition,
        reason: error.provenance.reason,
        text: render_error_text(
            &error.message,
            &[RenderFrame {
                source,
                uri: &display_uri,
                member: &member,
                span: error.span,
                highlight_length_utf16: error.highlight_length_utf16,
            }],
            error.hint(),
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
    input: ModuleSourceInput<'_>,
    options: &ModuleEvalOptions,
) -> ModuleOutputResult {
    evaluate_module_output_with_host(
        input,
        options,
        crate::modules::default_or_unsupported_completion,
    )
}

/// Evaluate actual output with module resolution/source I/O supplied by the Host.
/// Parsing, code installation, identity, caching and initialization stay in the VM.
pub fn evaluate_module_output_with_host(
    input: ModuleSourceInput<'_>,
    options: &ModuleEvalOptions,
    module_host: impl FnMut(&crate::modules::ModuleRequest) -> crate::modules::ModuleCompletion,
) -> ModuleOutputResult {
    evaluate_module_output_with_hosts(input, options, module_host, |_| None)
}

/// Evaluate with typed external resource reads in addition to module I/O.
/// Returning None delegates to the shared configured property/environment policy.
pub fn evaluate_module_output_with_hosts(
    input: ModuleSourceInput<'_>,
    options: &ModuleEvalOptions,
    module_host: impl FnMut(&crate::modules::ModuleRequest) -> crate::modules::ModuleCompletion,
    resource_host: impl FnMut(
        &crate::vm::ReadRequest,
    )
        -> Option<Result<crate::resources::ReadResult, crate::host::ReadFailure>>,
) -> ModuleOutputResult {
    evaluate_module_output_with_budget_and_hosts(
        input,
        options,
        1_048_576,
        module_host,
        resource_host,
    )
}

#[cfg(test)]
fn evaluate_module_output_with_budget(
    input: ModuleSourceInput<'_>,
    options: &ModuleEvalOptions,
    poll_budget: usize,
) -> ModuleOutputResult {
    evaluate_module_output_with_budget_and_hosts(
        input,
        options,
        poll_budget,
        crate::modules::default_or_unsupported_completion,
        |_| None,
    )
}

fn evaluate_module_output_with_budget_and_hosts(
    input: ModuleSourceInput<'_>,
    options: &ModuleEvalOptions,
    poll_budget: usize,
    mut module_host: impl FnMut(&crate::modules::ModuleRequest) -> crate::modules::ModuleCompletion,
    mut resource_host: impl FnMut(
        &crate::vm::ReadRequest,
    ) -> Option<
        Result<crate::resources::ReadResult, crate::host::ReadFailure>,
    >,
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
            // CLI settings transform reflection locations as well as log and
            // error frames. Keep the API default independent of editor settings.
            frame_transformer: if options.diagnostic_profile
                == DiagnosticProfile::CliBundledSettings
            {
                crate::logging::FrameTransformer::Host
            } else {
                crate::logging::FrameTransformer::Default
            },
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
                HostPoll::Module { token, request } => {
                    session.complete_module(&token, module_host(&request), 0)?
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
