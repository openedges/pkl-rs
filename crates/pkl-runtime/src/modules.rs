//! Typed module Host effects. Resolution and source loading have different
//! cache boundaries; neither is a resource read or an evaluation in another VM.
use std::sync::Arc;

use crate::{
    host::IoFailure,
    value::{ErrorKind, PklString, RuntimeError},
};

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

/// Capabilities of the declaration's original ModuleKey, not a resource reader.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ModuleCapabilities {
    pub local: bool,
    pub hierarchical: bool,
    pub fragment_paths: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ModuleRequestKind {
    /// Import-node trust admission. This precedes key/URI cache lookup and is
    /// distinct from the key's resolve/allowlist check.
    CheckImport {
        importer: ModuleIdentity,
        import_uri: Arc<str>,
    },
    Capabilities {
        module: ModuleIdentity,
    },
    HasElement {
        module: ModuleIdentity,
        element_uri: Arc<str>,
    },
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
    Io(IoFailure),
    /// An already constructed Host rejection; arbitrary text has no guest provenance.
    Denied(PklString),
    /// Typed `org.pkl.core.SecurityManagerException` from import admission,
    /// module resolution or ModuleKey.hasElement.
    /// The message is the Java exception message, not a formatted diagnostic.
    /// Source loading does not support this completion variant.
    SecurityManagerException {
        message: Option<PklString>,
    },
    Unsupported(PklString),
}

impl ModuleHostError {
    pub(crate) fn into_runtime(self, uri: &str) -> RuntimeError {
        match self {
            Self::NotFound => RuntimeError::host_language(format!("Cannot find module `{uri}`.")),
            Self::Io(cause) => cause.into_runtime("loading module", uri),
            Self::Denied(message) => RuntimeError::new(ErrorKind::Host, message),
            Self::SecurityManagerException { message } => RuntimeError::host_language(
                message.unwrap_or_else(|| "None (cause has no message)".into()),
            ),
            Self::Unsupported(message) => RuntimeError::new(ErrorKind::Unsupported, message),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ModuleCompletion {
    ImportChecked(Result<(), ModuleHostError>),
    Capabilities(Result<ModuleCapabilities, ModuleHostError>),
    HasElement(Result<bool, ModuleHostError>),
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
        ModuleRequestKind::CheckImport { .. } => ModuleCompletion::ImportChecked(Err(error)),
        ModuleRequestKind::Capabilities { .. } => ModuleCompletion::Capabilities(Err(error)),
        ModuleRequestKind::HasElement { .. } => ModuleCompletion::HasElement(Err(error)),
        ModuleRequestKind::Resolve { .. } => ModuleCompletion::Resolved(Err(error)),
        ModuleRequestKind::LoadSource { .. } => ModuleCompletion::Source(Err(error)),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum StdlibKind {
    Singleton(&'static str),
    Cached,
    Missing,
}

/// This classification follows ModuleCache's exact switch, independently of
/// the available source catalog and of which external bindings are implemented.
pub(crate) fn stdlib_kind(uri: &str) -> Option<StdlibKind> {
    uri.strip_prefix("pkl:")?;
    let name = crate::host::resources::decoded_scheme_specific_part(uri);
    let canonical = [
        "pkl:analyze",
        "pkl:base",
        "pkl:Benchmark",
        "pkl:Command",
        "pkl:jsonnet",
        "pkl:math",
        "pkl:pklbinary",
        "pkl:platform",
        "pkl:project",
        "pkl:ref",
        "pkl:reflect",
        "pkl:release",
        "pkl:semver",
        "pkl:settings",
        "pkl:test",
        "pkl:xml",
    ]
    .into_iter()
    .find(|candidate| name.as_ref().and_then(PklString::as_str) == Some(&candidate[4..]));
    Some(if let Some(canonical) = canonical {
        StdlibKind::Singleton(canonical)
    } else if include_str!("stdlib/modules.txt")
        .lines()
        .any(|candidate| candidate == uri)
    {
        StdlibKind::Cached
    } else {
        StdlibKind::Missing
    })
}

/// Explicit opt-in to the pinned default import trust policy and bundled key
/// resolution. This helper performs no I/O. A custom Host is never bypassed by
/// the VM; its caller chooses whether to delegate to this default profile.
pub fn default_completion(request: &ModuleRequest) -> Option<ModuleCompletion> {
    match &request.kind {
        ModuleRequestKind::CheckImport {
            importer,
            import_uri,
        } => {
            let result = (|| {
                let from = crate::host::resources::module_trust_level(&importer.resolved_uri)?;
                let to = crate::host::resources::module_trust_level(import_uri)?;
                if from < to {
                    return Err(ModuleHostError::SecurityManagerException { message: Some(format!(
                        "Refusing to import module `{import_uri}` because importing module `{}` has an insufficient trust level.",
                        importer.resolved_uri).into()) });
                }
                Ok(())
            })();
            Some(ModuleCompletion::ImportChecked(result))
        }
        ModuleRequestKind::Resolve {
            import_uri,
            original_uri,
            ..
        } if stdlib_kind(import_uri) == Some(StdlibKind::Cached) && import_uri == original_uri => {
            // The default allowlist includes the exact lowercase pkl:
            // scheme. StandardLibrary.resolve returns the same key.
            Some(ModuleCompletion::Resolved(Ok(ResolvedModule {
                original_uri: original_uri.clone(),
                resolved_uri: import_uri.clone(),
                source_uri: import_uri.clone(),
            })))
        }
        _ => None,
    }
}

/// Pure evaluator profile: explicit default policy, with other capabilities
/// still reported as unavailable rather than attempted by the runtime.
pub fn default_or_unsupported_completion(request: &ModuleRequest) -> ModuleCompletion {
    default_completion(request).unwrap_or_else(|| unsupported_completion(request))
}
