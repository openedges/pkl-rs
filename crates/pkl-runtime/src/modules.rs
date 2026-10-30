//! Typed module Host effects. Resolution and source loading have different
//! cache boundaries; neither is a resource read or an evaluation in another VM.
use crate::source::SourceText;
use std::sync::Arc;

use crate::{
    host::IoFailure,
    value::{ErrorKind, PklString, RuntimeError},
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct UnitId(pub u32);

/// Evaluator-owned identities. They identify key objects, never URI cache entries.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ModuleKeyId(pub u64);
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ResolvedKeyId(pub u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModuleKeyKind {
    StandardLibrary,
    Host,
    SuppliedText,
    Synthetic,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModuleKey {
    pub id: ModuleKeyId,
    pub uri: PklString,
    pub kind: ModuleKeyKind,
    /// The Host owns the object behind this handle. It is not a source or a cache.
    pub handle: PklString,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedModuleKey {
    pub id: ResolvedKeyId,
    pub key: ModuleKey,
    pub handle: PklString,
    pub module: ResolvedModule,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectionPhase {
    RuntimeImport,
    ClausePreparation,
    Primary,
}

/// A public evaluation input. Construction performs no factory lookup or I/O.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModuleSourceSpec {
    pub uri: PklString,
    pub contents: Option<SourceText>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResolutionPhase {
    Primary,
    Query,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModuleIdentity {
    pub original_uri: PklString,
    pub resolved_uri: PklString,
}

/// Construct the original file ModuleKey from an absolute slash-separated path.
/// This follows `IoUtils.toUri` path quoting: Unicode remains Unicode, while
/// reserved path characters are escaped. It performs no I/O, normalization, or
/// canonicalization. A Host's canonical resolved key is a separate identity.
pub fn original_file_uri(path: &str) -> Result<PklString, ModuleHostError> {
    if !path.starts_with('/') {
        return Err(ModuleHostError::Unsupported(
            "Original file URI construction requires an absolute slash-separated path.".into(),
        ));
    }
    Ok(PklString::from("file://").concat(&crate::host::resources::quote_path(&path.into())))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedModule {
    pub original_uri: PklString,
    pub resolved_uri: PklString,
    /// Diagnostic source URI; independent from both cache keys.
    pub source_uri: PklString,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModuleSource {
    pub text: SourceText,
    pub inferred_name: PklString,
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
        import_uri: PklString,
    },
    SelectKey {
        id: ModuleKeyId,
        uri: PklString,
        phase: SelectionPhase,
    },
    QueryCachePolicy {
        key: ModuleKey,
    },
    QueryFragmentPaths {
        key: ModuleKey,
    },
    ResolveImportUri {
        key: ModuleKey,
        authored_uri: PklString,
    },
    Capabilities {
        key: Option<ModuleKey>,
        module: ModuleIdentity,
    },
    HasElement {
        key: Option<ModuleKey>,
        module: ModuleIdentity,
        element_uri: PklString,
    },
    Resolve {
        key: ModuleKey,
        resolved_key_id: ResolvedKeyId,
        importer: ModuleIdentity,
        import_uri: PklString,
        original_uri: PklString,
    },
    ResolveKey {
        key: ModuleKey,
        resolved_key_id: ResolvedKeyId,
        phase: ResolutionPhase,
    },
    LoadSource {
        key: ResolvedModuleKey,
        resolved: ResolvedModule,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ModuleHostError {
    NotFound,
    NoLoader,
    UriSyntax {
        reason: PklString,
    },
    ExternalReaderProcess {
        message: Option<PklString>,
    },
    /// Raw I/O exception message; shared diagnostics add the IOException type.
    Io(IoFailure),
    /// An already constructed Host rejection; arbitrary text has no guest provenance.
    Denied(PklString),
    /// Typed `org.pkl.core.SecurityManagerException` from import admission,
    /// module resolution, ModuleKey.resolveUri or ModuleKey.hasElement.
    /// The message is the Java exception message, not a formatted diagnostic.
    /// Source loading does not support this completion variant.
    SecurityManagerException {
        message: Option<PklString>,
    },
    Unsupported(PklString),
}

impl ModuleHostError {
    pub(crate) fn into_factory_runtime(self, uri: impl Into<PklString>) -> RuntimeError {
        let uri = uri.into();
        match self {
            // The pinned ModuleResolver passes no message argument for this template.
            Self::Io(cause) => cause.into_runtime("loading module", "{0}"),
            other => other.into_runtime(&uri),
        }
    }
    pub(crate) fn into_runtime(self, uri: impl Into<PklString>) -> RuntimeError {
        let uri = uri.into();
        match self {
            Self::NotFound => RuntimeError::host_language(
                PklString::from("Cannot find module `")
                    .concat(&uri)
                    .concat(&"`.".into()),
            ),
            Self::NoLoader => RuntimeError::host_language(
                PklString::from("No module loader is registered for module `")
                    .concat(&uri)
                    .concat(&"`.".into()),
            ),
            Self::UriSyntax { reason } => {
                let mut error = RuntimeError::host_language(
                    PklString::from("Module URI `")
                        .concat(&uri)
                        .concat(&"` has invalid syntax.".into()),
                );
                error.hint = Some(reason);
                error
            }
            Self::ExternalReaderProcess { message } => {
                let headline =
                    PklString::from("Failed to communicate with external reader process.");
                let message = match message {
                    Some(message) if message != headline => headline
                        .concat(&"\nExternalReaderProcessException: ".into())
                        .concat(&message),
                    _ => headline,
                };
                RuntimeError::host_language(message)
            }
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
    KeySelected(Result<ModuleKey, ModuleHostError>),
    CachePolicy(Result<bool, ModuleHostError>),
    FragmentPaths(Result<bool, ModuleHostError>),
    ImportUri(Result<PklString, ModuleHostError>),
    Capabilities(Result<ModuleCapabilities, ModuleHostError>),
    HasElement(Result<bool, ModuleHostError>),
    Resolved(Result<ResolvedModule, ModuleHostError>),
    KeyResolved(Result<ResolvedModuleKey, ModuleHostError>),
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
        ModuleRequestKind::SelectKey { .. } => ModuleCompletion::KeySelected(Err(error)),
        ModuleRequestKind::QueryCachePolicy { .. } => ModuleCompletion::CachePolicy(Err(error)),
        ModuleRequestKind::QueryFragmentPaths { .. } => ModuleCompletion::FragmentPaths(Err(error)),
        ModuleRequestKind::ResolveImportUri { .. } => ModuleCompletion::ImportUri(Err(error)),
        ModuleRequestKind::Capabilities { .. } => ModuleCompletion::Capabilities(Err(error)),
        ModuleRequestKind::HasElement { .. } => ModuleCompletion::HasElement(Err(error)),
        ModuleRequestKind::Resolve { .. } | ModuleRequestKind::ResolveKey { .. } => {
            ModuleCompletion::Resolved(Err(error))
        }
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
pub(crate) fn stdlib_kind(uri: &PklString) -> Option<StdlibKind> {
    if !uri.starts_with("pkl:") {
        return None;
    }
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
        .any(|candidate| uri == candidate)
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
        ModuleRequestKind::SelectKey { id, uri, .. } if uri.starts_with("pkl:") => {
            Some(ModuleCompletion::KeySelected(Ok(ModuleKey {
                id: *id,
                uri: uri.clone(),
                kind: ModuleKeyKind::StandardLibrary,
                handle: PklString::default(),
            })))
        }
        ModuleRequestKind::QueryCachePolicy { key }
            if key.kind == ModuleKeyKind::StandardLibrary =>
        {
            Some(ModuleCompletion::CachePolicy(Ok(true)))
        }
        ModuleRequestKind::ResolveImportUri { key, authored_uri } => {
            Some(ModuleCompletion::ImportUri(
                crate::host::resources::resolve_module_key_uri(authored_uri, &key.uri)
                    .map_err(|error| ModuleHostError::Unsupported(error.message)),
            ))
        }
        ModuleRequestKind::QueryFragmentPaths { .. } => {
            Some(ModuleCompletion::FragmentPaths(Ok(false)))
        }
        ModuleRequestKind::CheckImport {
            importer,
            import_uri,
        } => {
            let result = (|| {
                let from = crate::host::resources::module_trust_level(&importer.resolved_uri)?;
                let to = crate::host::resources::module_trust_level(import_uri)?;
                if from < to {
                    return Err(ModuleHostError::SecurityManagerException {
                        message: Some(
                            PklString::from("Refusing to import module `")
                                .concat(import_uri)
                                .concat(&"` because importing module `".into())
                                .concat(&importer.resolved_uri)
                                .concat(&"` has an insufficient trust level.".into()),
                        ),
                    });
                }
                Ok(())
            })();
            Some(ModuleCompletion::ImportChecked(result))
        }
        ModuleRequestKind::Resolve { key, .. } | ModuleRequestKind::ResolveKey { key, .. }
            if key.kind == ModuleKeyKind::StandardLibrary
                && stdlib_kind(&key.uri) == Some(StdlibKind::Cached) =>
        {
            Some(ModuleCompletion::Resolved(Ok(ResolvedModule {
                original_uri: key.uri.clone(),
                resolved_uri: key.uri.clone(),
                source_uri: key.uri.clone(),
            })))
        }
        ModuleRequestKind::ResolveKey { key, .. } if key.kind == ModuleKeyKind::Synthetic => {
            Some(ModuleCompletion::Resolved(Ok(ResolvedModule {
                original_uri: key.uri.clone(),
                resolved_uri: key.uri.clone(),
                source_uri: key.uri.clone(),
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
