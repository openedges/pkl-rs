//! Typed module Host effects. Resolution and source loading have different
//! cache boundaries; neither is a resource read or an evaluation in another VM.
use std::sync::Arc;

use crate::value::{ErrorKind, RuntimeError};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct UnitId(pub u32);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModuleIdentity {
    pub original_uri: Arc<str>,
    pub resolved_uri: Arc<str>,
}

/// Construct the original file ModuleKey from an absolute slash-separated path.
/// This follows `IoUtils.toUri` path quoting: Unicode remains Unicode, while
/// reserved path characters are escaped. It performs no I/O, normalization, or
/// canonicalization. A Host's canonical resolved key is a separate identity.
pub fn original_file_uri(path: &str) -> Result<String, ModuleHostError> {
    if !path.starts_with('/') {
        return Err(ModuleHostError::Unsupported(
            "Original file URI construction requires an absolute slash-separated path.".into(),
        ));
    }
    Ok(format!(
        "file://{}",
        crate::host::resources::quote_path(path)
    ))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedModule {
    pub original_uri: Arc<str>,
    pub resolved_uri: Arc<str>,
    /// Diagnostic source URI; independent from both cache keys.
    pub source_uri: Arc<str>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModuleSource {
    pub text: Arc<str>,
    pub inferred_name: Arc<str>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModuleRequest {
    pub id: u64,
    pub kind: ModuleRequestKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ModuleRequestKind {
    Resolve {
        importer: ModuleIdentity,
        import_uri: Arc<str>,
        original_uri: Arc<str>,
    },
    LoadSource {
        resolved: ResolvedModule,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ModuleHostError {
    NotFound,
    /// Raw I/O exception message; shared diagnostics add the IOException type.
    Io(Arc<str>),
    Denied(Arc<str>),
    Unsupported(Arc<str>),
}

impl ModuleHostError {
    pub(crate) fn into_runtime(self, uri: &str) -> RuntimeError {
        match self {
            Self::NotFound => {
                RuntimeError::new(ErrorKind::Host, format!("Cannot find module `{uri}`."))
            }
            Self::Io(cause) => RuntimeError::new(
                ErrorKind::Host,
                format!("I/O error loading module `{uri}`.\nIOException: {cause}"),
            ),
            Self::Denied(message) => RuntimeError::new(ErrorKind::Host, message.as_ref()),
            Self::Unsupported(message) => {
                RuntimeError::new(ErrorKind::Unsupported, message.as_ref())
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ModuleCompletion {
    Resolved(Result<ResolvedModule, ModuleHostError>),
    Source(Result<ModuleSource, ModuleHostError>),
}

/// Original frontend failure and source are retained until the public rendering
/// boundary, including through module-cache replay and Host callback unwinding.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModuleDiagnostic {
    pub diagnostic: crate::diagnostic::Diagnostic,
    pub source: Arc<crate::value::RuntimeSource>,
    /// The active importing guest root owns this frame's name even though the
    /// parser's source section belongs to the imported module.
    pub member: crate::value::PklString,
}

/// Explicit absence of module I/O in a pure evaluator configuration.
pub fn unsupported_completion(request: &ModuleRequest) -> ModuleCompletion {
    let error =
        ModuleHostError::Unsupported("No module resolver/source reader is configured.".into());
    match request.kind {
        ModuleRequestKind::Resolve { .. } => ModuleCompletion::Resolved(Err(error)),
        ModuleRequestKind::LoadSource { .. } => ModuleCompletion::Source(Err(error)),
    }
}
