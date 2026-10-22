//! Shared frontend prefix of module-output evaluation.
//!
//! Successful preparation is explicitly unsupported at the module-output phase;
//! it does not stand in for `evaluateOutputBytes` or invoke value `toString`.
pub use crate::diagnostic::FailureDisposition;
pub mod arguments;

use crate::diagnostic::{Diagnostic, DiagnosticOrigin, RenderFrame, render_error};
use crate::source::SourceMap;
use crate::value::PklString;

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
    if !options.no_cache || !options.no_project || options.settings != "pkl:settings" {
        return unsupported(
            FrontendPhase::Configuration,
            "module-evaluation-configuration",
            "This module-evaluation adapter requires --no-cache --no-project --settings pkl:settings.",
        );
    }
    let module = match crate::parser::parse_module(input.text) {
        Ok(module) => module,
        Err(error) => return render_failure(input, options.diagnostic_profile, error),
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
    if let Err(error) = crate::resolver::resolve_module(&module) {
        return render_failure(resolved_input, options.diagnostic_profile, error);
    }
    unsupported(
        FrontendPhase::ModuleOutput,
        "module-output-evaluation",
        "Module output evaluation is not implemented by this frontend adapter.",
    )
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
