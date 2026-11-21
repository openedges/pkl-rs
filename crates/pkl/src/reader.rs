//! Custom module key & resource reader SPI mirroring
//! `org.pkl.core.module.{ModuleKey, ModuleKeyFactory, ResolvedModuleKey, PathElement}` and
//! `org.pkl.core.resource.{Resource, ResourceReader}`.

use crate::error::{PklError, PklErrorKind, Result};
use crate::security::SecurityManager;
use pkl_host_native::{CustomModuleReader, CustomResourceReader};
use pkl_runtime::glob::PathElement as RuntimePathElement;
use pkl_runtime::host::{IoFailure, ReadFailure};
use pkl_runtime::modules::{ModuleHostError, ModuleSource as RuntimeModuleSource, ResolvedModule};
use pkl_runtime::resources::{ReadResult, Resource as RuntimeResource};
use pkl_runtime::source::SourceText;
use pkl_runtime::value::PklString;
use std::sync::Arc;

/// Directory/file listing entry returned by globbable module and resource readers
/// (`org.pkl.core.module.PathElement`).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct PathElement {
    pub name: String,
    pub is_directory: bool,
}

impl PathElement {
    pub fn new(name: impl Into<String>, is_directory: bool) -> Self {
        Self {
            name: name.into(),
            is_directory,
        }
    }
}

/// Resolved module source returned by [`ModuleKey::resolve`] (`org.pkl.core.module.ResolvedModuleKey`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedModuleKey {
    pub original_uri: String,
    pub resolved_uri: String,
    pub source_text: String,
    pub inferred_name: Option<String>,
}

impl ResolvedModuleKey {
    /// Creates a virtual/in-memory resolved module key (`ResolvedModuleKeys.virtual`).
    pub fn virtual_module(uri: impl Into<String>, source_text: impl Into<String>) -> Self {
        let u = uri.into();
        Self {
            original_uri: u.clone(),
            resolved_uri: u,
            source_text: source_text.into(),
            inferred_name: None,
        }
    }

    pub fn with_resolved_uri(mut self, resolved_uri: impl Into<String>) -> Self {
        self.resolved_uri = resolved_uri.into();
        self
    }

    pub fn with_inferred_name(mut self, inferred_name: impl Into<String>) -> Self {
        self.inferred_name = Some(inferred_name.into());
        self
    }
}

/// Custom module key SPI (`org.pkl.core.module.ModuleKey`).
pub trait ModuleKey: Send + Sync {
    fn uri(&self) -> &str;

    fn has_hierarchical_uris(&self) -> bool {
        false
    }

    fn is_local(&self) -> bool {
        false
    }

    fn is_globbable(&self) -> bool {
        false
    }

    fn is_cached(&self) -> bool {
        true
    }

    fn has_fragment_paths(&self) -> bool {
        false
    }

    fn resolve(&self, security_manager: &dyn SecurityManager) -> Result<ResolvedModuleKey>;

    fn has_element(
        &self,
        _security_manager: &dyn SecurityManager,
        _element_uri: &str,
    ) -> Result<bool> {
        Ok(false)
    }

    fn list_elements(
        &self,
        _security_manager: &dyn SecurityManager,
        _base_uri: &str,
    ) -> Result<Vec<PathElement>> {
        Err(PklError::evaluation(
            "Module key does not support listElements.",
        ))
    }
}

/// Factory creating [`ModuleKey`] instances for custom URI schemes
/// (`org.pkl.core.module.ModuleKeyFactory`).
pub trait ModuleKeyFactory: Send + Sync {
    fn create(&self, uri: &str) -> Option<Box<dyn ModuleKey>>;

    fn close(&self) {}
}

struct FnModuleKeyFactory<F>(F);

impl<F> ModuleKeyFactory for FnModuleKeyFactory<F>
where
    F: Fn(&str) -> Option<Box<dyn ModuleKey>> + Send + Sync,
{
    fn create(&self, uri: &str) -> Option<Box<dyn ModuleKey>> {
        (self.0)(uri)
    }
}

/// Helper constructors mirroring `org.pkl.core.module.ModuleKeyFactories`.
pub struct ModuleKeyFactories;

impl ModuleKeyFactories {
    /// Creates a [`ModuleKeyFactory`] from a closure.
    pub fn from_fn(
        f: impl Fn(&str) -> Option<Box<dyn ModuleKey>> + Send + Sync + 'static,
    ) -> Arc<dyn ModuleKeyFactory> {
        Arc::new(FnModuleKeyFactory(f))
    }
}

/// Binary resource payload returned by [`ResourceReader::read`] (`org.pkl.core.resource.Resource`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Resource {
    pub uri: String,
    pub bytes: Vec<u8>,
}

impl Resource {
    pub fn new(uri: impl Into<String>, bytes: impl Into<Vec<u8>>) -> Self {
        Self {
            uri: uri.into(),
            bytes: bytes.into(),
        }
    }

    pub fn from_text(uri: impl Into<String>, text: impl AsRef<str>) -> Self {
        Self {
            uri: uri.into(),
            bytes: text.as_ref().as_bytes().to_vec(),
        }
    }
}

/// Return value of [`ResourceReader::read`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ResourceValue {
    /// Binary/text resource (`pkl:base#Resource` with `uri`, `bytes`, `text`, `base64`).
    Resource(Resource),
    /// Scalar string value (like `env:` / `prop:`).
    Text(String),
}

/// Custom resource reader SPI (`org.pkl.core.resource.ResourceReader`).
pub trait ResourceReader: Send + Sync {
    /// URI scheme without trailing `:` (matched case-insensitively).
    fn uri_scheme(&self) -> &str;

    fn has_hierarchical_uris(&self) -> bool {
        false
    }

    fn is_globbable(&self) -> bool {
        false
    }

    /// Reads the resource at `uri`. Return `Ok(None)` when the resource does not exist.
    fn read(&self, uri: &str) -> Result<Option<ResourceValue>>;

    fn has_element(
        &self,
        _security_manager: &dyn SecurityManager,
        element_uri: &str,
    ) -> Result<bool> {
        Ok(self.read(element_uri)?.is_some())
    }

    fn list_elements(
        &self,
        _security_manager: &dyn SecurityManager,
        _base_uri: &str,
    ) -> Result<Vec<PathElement>> {
        Err(PklError::evaluation(
            "Resource reader does not support listElements.",
        ))
    }

    fn close(&self) {}
}

/// Adapter bridging public [`ModuleKeyFactory`] to `pkl_host_native::CustomModuleReader`.
pub(crate) struct ModuleKeyFactoryAdapter {
    pub(crate) factory: Arc<dyn ModuleKeyFactory>,
    pub(crate) security: Arc<dyn SecurityManager>,
}

impl CustomModuleReader for ModuleKeyFactoryAdapter {
    fn matches_uri(&self, uri: &PklString) -> bool {
        self.factory.create(&uri.to_java_utf8()).is_some()
    }

    fn is_hierarchical(&self, uri: &PklString) -> bool {
        self.factory
            .create(&uri.to_java_utf8())
            .is_some_and(|k| k.has_hierarchical_uris())
    }

    fn is_local(&self, uri: &PklString) -> bool {
        self.factory
            .create(&uri.to_java_utf8())
            .is_some_and(|k| k.is_local())
    }

    fn is_globbable(&self, uri: &PklString) -> bool {
        self.factory
            .create(&uri.to_java_utf8())
            .is_some_and(|k| k.is_globbable())
    }

    fn is_cached(&self, uri: &PklString) -> bool {
        self.factory
            .create(&uri.to_java_utf8())
            .is_none_or(|k| k.is_cached())
    }

    fn has_fragment_paths(&self, uri: &PklString) -> bool {
        self.factory
            .create(&uri.to_java_utf8())
            .is_some_and(|k| k.has_fragment_paths())
    }

    fn resolve_module(
        &self,
        uri: &PklString,
    ) -> std::result::Result<(ResolvedModule, RuntimeModuleSource), ModuleHostError> {
        let key = self
            .factory
            .create(&uri.to_java_utf8())
            .ok_or(ModuleHostError::NoLoader)?;
        let resolved = key
            .resolve(self.security.as_ref())
            .map_err(pkl_err_to_module_host)?;
        let module = ResolvedModule {
            original_uri: PklString::from(resolved.original_uri),
            resolved_uri: PklString::from(resolved.resolved_uri),
            source_uri: uri.clone(),
        };
        let source = RuntimeModuleSource {
            text: SourceText::from(resolved.source_text),
            inferred_name: resolved
                .inferred_name
                .map(PklString::from)
                .unwrap_or_default(),
        };
        Ok((module, source))
    }

    fn has_element(&self, element_uri: &PklString) -> std::result::Result<bool, ModuleHostError> {
        let uri_str = element_uri.to_java_utf8();
        let Some(key) = self.factory.create(&uri_str) else {
            return Ok(false);
        };
        key.has_element(self.security.as_ref(), &uri_str)
            .map_err(pkl_err_to_module_host)
    }

    fn list_elements(
        &self,
        base_uri: &PklString,
    ) -> std::result::Result<Vec<RuntimePathElement>, ModuleHostError> {
        let uri_str = base_uri.to_java_utf8();
        let key = self
            .factory
            .create(&uri_str)
            .ok_or(ModuleHostError::NoLoader)?;
        let elems = key
            .list_elements(self.security.as_ref(), &uri_str)
            .map_err(pkl_err_to_module_host)?;
        Ok(elems
            .into_iter()
            .map(|e| RuntimePathElement {
                name: PklString::from(e.name),
                is_directory: e.is_directory,
            })
            .collect())
    }

    fn close(&self) {
        self.factory.close();
    }
}

/// Adapter bridging public [`ResourceReader`] to `pkl_host_native::CustomResourceReader`.
pub(crate) struct ResourceReaderAdapter {
    pub(crate) reader: Arc<dyn ResourceReader>,
    pub(crate) security: Arc<dyn SecurityManager>,
}

impl CustomResourceReader for ResourceReaderAdapter {
    fn scheme(&self) -> &str {
        self.reader.uri_scheme()
    }

    fn has_hierarchical_uris(&self) -> bool {
        self.reader.has_hierarchical_uris()
    }

    fn is_globbable(&self) -> bool {
        self.reader.is_globbable()
    }

    fn read(&self, uri: &PklString) -> std::result::Result<ReadResult, ReadFailure> {
        let uri_str = uri.to_java_utf8();
        match self.reader.read(&uri_str) {
            Ok(None) => Ok(ReadResult::Missing),
            Ok(Some(ResourceValue::Text(s))) => Ok(ReadResult::Text(PklString::from(s))),
            Ok(Some(ResourceValue::Resource(res))) => Ok(ReadResult::Resource(RuntimeResource {
                uri: PklString::from(res.uri),
                bytes: res.bytes.into(),
            })),
            Err(err) => Err(pkl_err_to_read_failure(err)),
        }
    }

    fn has_element(&self, uri: &PklString) -> std::result::Result<bool, ModuleHostError> {
        self.reader
            .has_element(self.security.as_ref(), &uri.to_java_utf8())
            .map_err(pkl_err_to_module_host)
    }

    fn list_elements(
        &self,
        base_uri: &PklString,
    ) -> std::result::Result<Vec<RuntimePathElement>, ModuleHostError> {
        let elems = self
            .reader
            .list_elements(self.security.as_ref(), &base_uri.to_java_utf8())
            .map_err(pkl_err_to_module_host)?;
        Ok(elems
            .into_iter()
            .map(|e| RuntimePathElement {
                name: PklString::from(e.name),
                is_directory: e.is_directory,
            })
            .collect())
    }

    fn close(&self) {
        self.reader.close();
    }
}

fn pkl_err_to_module_host(err: PklError) -> ModuleHostError {
    match err.kind() {
        PklErrorKind::Security => ModuleHostError::SecurityManagerException {
            message: Some(PklString::from(err.message())),
        },
        PklErrorKind::Io => {
            if err.message().contains("not found") || err.message().contains("No such file") {
                ModuleHostError::NotFound
            } else {
                ModuleHostError::Io(IoFailure::from(err.message()))
            }
        }
        _ => ModuleHostError::EvalError {
            message: PklString::from(err.message()),
            hint: None,
        },
    }
}

fn pkl_err_to_read_failure(err: PklError) -> ReadFailure {
    match err.kind() {
        PklErrorKind::Security => ReadFailure::SecurityManagerException {
            message: Some(PklString::from(err.message())),
        },
        PklErrorKind::Io => ReadFailure::Io(IoFailure::from(err.message())),
        _ => ReadFailure::EvalError {
            message: PklString::from(err.message()),
            hint: None,
        },
    }
}
