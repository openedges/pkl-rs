//! Shared pure-Rust Wasm host state machine used by `WasmEvaluator`, `WasmReplSession`,
//! and `WasmImportAnalyzer`.
//!
//! Enforces `SecurityPolicy` (`allowedModules`, `allowedResources`, `rootDir`), handles
//! stdlib (`pkl:*`), `env:`, `prop:`, custom JS module/resource readers, VFS `file:`,
//! async `http:`/`https:`, pure-Wasm `package://` metadata + SHA-256 + `ZipArchive`
//! DEFLATE verification, custom security/logger/frameTransformer hooks, and explicit
//! `capability-unavailable` reporting.

use super::util::{
    StructuredError, get_prop, js_string_from_pkl, log_frame_to_js, rust_string_from_js, set_prop,
};
use js_sys::{Array, Object};
use pkl_runtime::{
    glob::{
        GlobCompletion, GlobHostError, GlobReader, GlobRequest, GlobRequestKind, PathElement,
        ResourceReader, configured_completion_with_policy,
        unsupported_completion as unsupported_glob_completion,
    },
    host::{
        IoFailure, ReadFailure, SecurityPolicy, resolve_module_key_uri, resolve_resource_read_uri,
        security::validate_file_uri_has_leading_slash, try_configured_scalar_read_with_policy,
    },
    logging::{FrameTransformer, LogFrame, LogLevel, PlatformMetadata, TraceMode},
    modules::{
        ModuleCapabilities, ModuleCompletion, ModuleHostError, ModuleKey, ModuleKeyKind,
        ModuleRequest, ModuleRequestKind, ModuleSource, ResolvedModule, ResolvedModuleKey,
        default_completion, standard_key_completion_with_policy,
        unsupported_completion as unsupported_module_completion,
    },
    packages::{
        PackageAssetUri, PackageUri, PackageUriError, PackageUriFailure,
        http::{get_metadata_request_uri, validate_metadata_url},
        metadata::{DependencyMetadata, accept_metadata, sha256_hex},
        resolver::{percent_encode_fragment, resolve_relative_path},
        zip::{ZipArchive, ZipReadError, percent_decode},
    },
    resources::{ReadResult, Resource},
    source::SourceText,
    value::PklString,
    vm::ReadRequest,
};
use std::collections::BTreeMap;
use wasm_bindgen::JsValue;

#[derive(Clone, Debug)]
pub struct CustomModuleReaderMeta {
    pub scheme: String,
    pub is_local: bool,
    pub is_globbable: bool,
    pub has_hierarchical_uris: bool,
    pub is_cached: bool,
    pub has_fragment_paths: bool,
}

#[derive(Clone, Debug)]
pub struct CustomResourceReaderMeta {
    pub scheme: String,
    pub is_globbable: bool,
    pub has_hierarchical_uris: bool,
}

#[derive(Clone, Debug, Default)]
pub struct WasmHostConfig {
    pub allowed_modules: Option<Vec<String>>,
    pub allowed_resources: Option<Vec<String>>,
    pub root_dir: Option<String>,
    pub properties: Vec<(String, String)>,
    pub environment: Vec<(String, String)>,
    pub output_format: Option<String>,
    pub trace_mode: TraceMode,
    pub power_assertions: bool,
    pub use_color: bool,
    pub test_mode: bool,
    pub has_vfs: bool,
    pub has_http_fetch: bool,
    pub has_package_transport: bool,
    pub has_custom_logger: bool,
    pub has_custom_frame_transformer: bool,
    pub has_custom_security_check_import: bool,
    pub has_custom_security_check_read: bool,
    pub has_custom_security_check_resolve: bool,
    pub has_custom_security_check_resolve_resource: bool,
    pub platform_metadata: Option<PlatformMetadata>,
    pub custom_module_readers: Vec<CustomModuleReaderMeta>,
    pub custom_resource_readers: Vec<CustomResourceReaderMeta>,
}

/// Pure-Wasm package metadata and ZIP archive cache with strict Pkl 0.32.0 SHA-256 verification.
#[derive(Default)]
pub struct WasmPackageCache {
    metadata: BTreeMap<String, (DependencyMetadata, Vec<u8>)>,
    archives: BTreeMap<String, ZipArchive>,
}

impl WasmPackageCache {
    pub fn cache_key(package: &PackageUri) -> String {
        package
            .to_external_package_uri()
            .without_checksums()
            .uri
            .to_java_utf8()
    }

    pub fn get_metadata(
        &self,
        package: &PackageUri,
        test_mode: bool,
    ) -> Result<Option<DependencyMetadata>, ModuleHostError> {
        let key = Self::cache_key(package);
        let Some((meta, raw_bytes)) = self.metadata.get(&key) else {
            return Ok(None);
        };
        if let Some(expected) = &package.sha256 {
            let request_uri = get_metadata_request_uri(package);
            accept_metadata(package, &request_uri, raw_bytes, Some(expected), test_mode).map_err(
                |failure| ModuleHostError::SecurityManagerException {
                    message: Some(failure.message()),
                },
            )?;
        }
        Ok(Some(meta.clone()))
    }

    pub fn accept_and_store_metadata(
        &mut self,
        package: &PackageUri,
        request_uri: &PklString,
        bytes: &[u8],
        test_mode: bool,
    ) -> Result<DependencyMetadata, ModuleHostError> {
        let meta = accept_metadata(
            package,
            request_uri,
            bytes,
            package.sha256.as_ref(),
            test_mode,
        )
        .map_err(|failure| ModuleHostError::SecurityManagerException {
            message: Some(failure.message()),
        })?;
        let key = Self::cache_key(package);
        self.metadata.insert(key, (meta.clone(), bytes.to_vec()));
        Ok(meta)
    }

    pub fn get_archive(&self, package: &PackageUri) -> Option<&ZipArchive> {
        let key = Self::cache_key(package);
        self.archives.get(&key)
    }

    pub fn verify_and_store_archive(
        &mut self,
        package: &PackageUri,
        metadata: &DependencyMetadata,
        zip_bytes: &[u8],
    ) -> Result<&ZipArchive, ModuleHostError> {
        let computed_sha256 = sha256_hex(zip_bytes);
        let expected_zip_sha256 = metadata.package_zip_sha256.as_str().unwrap_or("");
        if computed_sha256 != expected_zip_sha256 {
            let display = package.display_name();
            let msg = format!(
                "Cannot download package `{display}` because the computed checksum does not match the expected checksum.\n\nComputed checksum: \"{computed_sha256}\"\nExpected checksum: \"{expected_zip_sha256}\"\nAsset URL: \"{}\"",
                metadata.package_zip_url
            );
            return Err(ModuleHostError::SecurityManagerException {
                message: Some(msg.into()),
            });
        }
        let archive = ZipArchive::parse(zip_bytes).map_err(|err| {
            ModuleHostError::Io(IoFailure::with_class(
                "ZipException",
                format!("Failed to parse ZIP archive: {err}"),
            ))
        })?;
        let key = Self::cache_key(package);
        self.archives.insert(key.clone(), archive);
        Ok(self.archives.get(&key).expect("inserted archive"))
    }

    pub fn clear(&mut self) {
        self.metadata.clear();
        self.archives.clear();
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SuspendBackend {
    Vfs,
    Http,
    Custom(usize),
}

impl SuspendBackend {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Vfs => "vfs",
            Self::Http => "http",
            Self::Custom(_) => "custom",
        }
    }

    pub const fn reader_index(&self) -> Option<usize> {
        match self {
            Self::Custom(idx) => Some(*idx),
            _ => None,
        }
    }
}

/// High-level async Host suspension emitted to JS.
#[derive(Clone, Debug)]
pub enum WasmSuspendRequest {
    ReadModule {
        token: String,
        backend: SuspendBackend,
        scheme: PklString,
        uri: PklString,
    },
    ModuleHasElement {
        token: String,
        backend: SuspendBackend,
        uri: PklString,
    },
    ReadResource {
        token: String,
        backend: SuspendBackend,
        scheme: PklString,
        uri: PklString,
    },
    ResourceHasElement {
        token: String,
        backend: SuspendBackend,
        uri: PklString,
    },
    ListElements {
        token: String,
        is_resource: bool,
        backend: SuspendBackend,
        uri: PklString,
    },
    PackageMetadata {
        token: String,
        package_uri: PklString,
        request_url: PklString,
    },
    PackageArchive {
        token: String,
        package_uri: PklString,
        zip_url: PklString,
    },
    CheckImportModule {
        token: String,
        importing_uri: PklString,
        imported_uri: PklString,
    },
    CheckResolveModule {
        token: String,
        uri: PklString,
    },
    CheckReadResource {
        token: String,
        uri: PklString,
    },
    CheckResolveResource {
        token: String,
        uri: PklString,
    },
    Log {
        token: String,
        level: LogLevel,
        message: PklString,
        frame: LogFrame,
    },
    TransformFrame {
        token: String,
        frame: LogFrame,
    },
}

impl WasmSuspendRequest {
    pub fn token(&self) -> &str {
        match self {
            Self::ReadModule { token, .. }
            | Self::ModuleHasElement { token, .. }
            | Self::ReadResource { token, .. }
            | Self::ResourceHasElement { token, .. }
            | Self::ListElements { token, .. }
            | Self::PackageMetadata { token, .. }
            | Self::PackageArchive { token, .. }
            | Self::CheckImportModule { token, .. }
            | Self::CheckResolveModule { token, .. }
            | Self::CheckReadResource { token, .. }
            | Self::CheckResolveResource { token, .. }
            | Self::Log { token, .. }
            | Self::TransformFrame { token, .. } => token,
        }
    }

    pub fn to_js(&self) -> Result<JsValue, JsValue> {
        let obj = Object::new();
        match self {
            Self::ReadModule {
                token,
                backend,
                scheme,
                uri,
            } => {
                set_prop(&obj, "kind", JsValue::from_str("readModule"))?;
                set_prop(&obj, "token", JsValue::from_str(token))?;
                set_prop(&obj, "backend", JsValue::from_str(backend.as_str()))?;
                if let Some(idx) = backend.reader_index() {
                    set_prop(&obj, "readerIndex", JsValue::from_f64(idx as f64))?;
                }
                set_prop(&obj, "scheme", js_string_from_pkl(scheme).into())?;
                set_prop(&obj, "uri", js_string_from_pkl(uri).into())?;
            }
            Self::ModuleHasElement {
                token,
                backend,
                uri,
            } => {
                set_prop(&obj, "kind", JsValue::from_str("moduleHasElement"))?;
                set_prop(&obj, "token", JsValue::from_str(token))?;
                set_prop(&obj, "backend", JsValue::from_str(backend.as_str()))?;
                if let Some(idx) = backend.reader_index() {
                    set_prop(&obj, "readerIndex", JsValue::from_f64(idx as f64))?;
                }
                set_prop(&obj, "uri", js_string_from_pkl(uri).into())?;
            }
            Self::ReadResource {
                token,
                backend,
                scheme,
                uri,
            } => {
                set_prop(&obj, "kind", JsValue::from_str("readResource"))?;
                set_prop(&obj, "token", JsValue::from_str(token))?;
                set_prop(&obj, "backend", JsValue::from_str(backend.as_str()))?;
                if let Some(idx) = backend.reader_index() {
                    set_prop(&obj, "readerIndex", JsValue::from_f64(idx as f64))?;
                }
                set_prop(&obj, "scheme", js_string_from_pkl(scheme).into())?;
                set_prop(&obj, "uri", js_string_from_pkl(uri).into())?;
            }
            Self::ResourceHasElement {
                token,
                backend,
                uri,
            } => {
                set_prop(&obj, "kind", JsValue::from_str("resourceHasElement"))?;
                set_prop(&obj, "token", JsValue::from_str(token))?;
                set_prop(&obj, "backend", JsValue::from_str(backend.as_str()))?;
                if let Some(idx) = backend.reader_index() {
                    set_prop(&obj, "readerIndex", JsValue::from_f64(idx as f64))?;
                }
                set_prop(&obj, "uri", js_string_from_pkl(uri).into())?;
            }
            Self::ListElements {
                token,
                is_resource,
                backend,
                uri,
            } => {
                set_prop(
                    &obj,
                    "kind",
                    JsValue::from_str(if *is_resource {
                        "listResources"
                    } else {
                        "listModules"
                    }),
                )?;
                set_prop(&obj, "token", JsValue::from_str(token))?;
                set_prop(&obj, "backend", JsValue::from_str(backend.as_str()))?;
                if let Some(idx) = backend.reader_index() {
                    set_prop(&obj, "readerIndex", JsValue::from_f64(idx as f64))?;
                }
                set_prop(&obj, "uri", js_string_from_pkl(uri).into())?;
            }
            Self::PackageMetadata {
                token,
                package_uri,
                request_url,
            } => {
                set_prop(&obj, "kind", JsValue::from_str("packageMetadata"))?;
                set_prop(&obj, "token", JsValue::from_str(token))?;
                set_prop(&obj, "packageUri", js_string_from_pkl(package_uri).into())?;
                set_prop(&obj, "requestUrl", js_string_from_pkl(request_url).into())?;
            }
            Self::PackageArchive {
                token,
                package_uri,
                zip_url,
            } => {
                set_prop(&obj, "kind", JsValue::from_str("packageArchive"))?;
                set_prop(&obj, "token", JsValue::from_str(token))?;
                set_prop(&obj, "packageUri", js_string_from_pkl(package_uri).into())?;
                set_prop(&obj, "zipUrl", js_string_from_pkl(zip_url).into())?;
            }
            Self::CheckImportModule {
                token,
                importing_uri,
                imported_uri,
            } => {
                set_prop(&obj, "kind", JsValue::from_str("checkImportModule"))?;
                set_prop(&obj, "token", JsValue::from_str(token))?;
                set_prop(
                    &obj,
                    "importingUri",
                    js_string_from_pkl(importing_uri).into(),
                )?;
                set_prop(&obj, "importedUri", js_string_from_pkl(imported_uri).into())?;
            }
            Self::CheckResolveModule { token, uri } => {
                set_prop(&obj, "kind", JsValue::from_str("checkResolveModule"))?;
                set_prop(&obj, "token", JsValue::from_str(token))?;
                set_prop(&obj, "uri", js_string_from_pkl(uri).into())?;
            }
            Self::CheckReadResource { token, uri } => {
                set_prop(&obj, "kind", JsValue::from_str("checkReadResource"))?;
                set_prop(&obj, "token", JsValue::from_str(token))?;
                set_prop(&obj, "uri", js_string_from_pkl(uri).into())?;
            }
            Self::CheckResolveResource { token, uri } => {
                set_prop(&obj, "kind", JsValue::from_str("checkResolveResource"))?;
                set_prop(&obj, "token", JsValue::from_str(token))?;
                set_prop(&obj, "uri", js_string_from_pkl(uri).into())?;
            }
            Self::Log {
                token,
                level,
                message,
                frame,
            } => {
                set_prop(&obj, "kind", JsValue::from_str("log"))?;
                set_prop(&obj, "token", JsValue::from_str(token))?;
                set_prop(
                    &obj,
                    "level",
                    JsValue::from_str(match level {
                        LogLevel::Trace => "trace",
                        LogLevel::Warn => "warn",
                    }),
                )?;
                set_prop(&obj, "message", js_string_from_pkl(message).into())?;
                set_prop(&obj, "frame", log_frame_to_js(frame)?)?;
            }
            Self::TransformFrame { token, frame } => {
                set_prop(&obj, "kind", JsValue::from_str("transformFrame"))?;
                set_prop(&obj, "token", JsValue::from_str(token))?;
                set_prop(&obj, "frame", log_frame_to_js(frame)?)?;
            }
        }
        Ok(obj.into())
    }
}

/// What `WasmHostCore` produces after inspecting a `HostPoll` event or resuming a multi-step op.
pub enum WasmHostOutcome {
    CompleteModule(ModuleCompletion),
    CompleteRead(Result<ReadResult, ReadFailure>),
    CompleteGlob(GlobCompletion),
    AckLog,
    CompleteFrame(LogFrame),
    Suspend(WasmSuspendRequest),
    CapabilityUnavailable {
        capability: &'static str,
        message: String,
    },
}

/// Multi-step continuation tracked inside `WasmHostCore` while an async JS callback is in flight.
#[derive(Clone, Debug)]
pub enum PendingContinuation {
    ModuleResolve {
        token: String,
        request: ModuleRequest,
        security_checked: bool,
    },
    ModuleHasElement {
        token: String,
        request: ModuleRequest,
        security_checked: bool,
    },
    ModuleCheckImport {
        token: String,
    },
    ModuleResolveDepImport {
        token: String,
        asset_uri: PackageAssetUri,
        dep_name: String,
        encoded_asset: String,
    },
    ReadResource {
        token: String,
        resolved_uri: PklString,
        security_checked: bool,
    },
    GlobHasElement {
        token: String,
        request: GlobRequest,
        security_checked: bool,
    },
    GlobListElements {
        token: String,
        request: GlobRequest,
        security_checked: bool,
    },
    Log {
        token: String,
    },
    TransformFrame {
        token: String,
    },
    PackageUrlCheck {
        next_suspend: Box<WasmSuspendRequest>,
        after_fetch: Box<PendingContinuation>,
    },
}

impl PendingContinuation {
    pub fn token(&self) -> &str {
        match self {
            Self::ModuleResolve { token, .. }
            | Self::ModuleHasElement { token, .. }
            | Self::ModuleCheckImport { token }
            | Self::ModuleResolveDepImport { token, .. }
            | Self::ReadResource { token, .. }
            | Self::GlobHasElement { token, .. }
            | Self::GlobListElements { token, .. }
            | Self::Log { token }
            | Self::TransformFrame { token } => token,
            Self::PackageUrlCheck { next_suspend, .. } => next_suspend.token(),
        }
    }
}

/// Pure-Rust Wasm Host state shared across evaluation, REPL, and import analysis.
pub struct WasmHostCore {
    pub config: WasmHostConfig,
    pub policy: SecurityPolicy,
    pub properties: Vec<(String, String)>,
    pub environment: Vec<(String, String)>,
    pub packages: WasmPackageCache,
    pub staged_sources: BTreeMap<PklString, ModuleSource>,
    pub pending: Option<PendingContinuation>,
    pub cancelled_tokens: Vec<String>,
    pub saw_security_denial: bool,
}

impl WasmHostCore {
    pub fn new(config: WasmHostConfig) -> Result<Self, StructuredError> {
        let mut policy = SecurityPolicy::new(
            config.allowed_modules.as_deref(),
            config.allowed_resources.as_deref(),
            config.root_dir.clone(),
        )
        .map_err(StructuredError::configuration)?;
        if !config.custom_module_readers.is_empty() {
            let schemes: Vec<String> = config
                .custom_module_readers
                .iter()
                .map(|r| r.scheme.clone())
                .collect();
            policy = policy
                .with_external_module_schemes(&schemes)
                .map_err(StructuredError::configuration)?;
        }
        if !config.custom_resource_readers.is_empty() {
            let schemes: Vec<String> = config
                .custom_resource_readers
                .iter()
                .map(|r| r.scheme.clone())
                .collect();
            policy = policy
                .with_external_resource_schemes(&schemes)
                .map_err(StructuredError::configuration)?;
        }

        let mut properties = config.properties.clone();
        if let Some(ref output_format) = config.output_format {
            if let Some(entry) = properties.iter_mut().find(|(k, _)| k == "pkl.outputFormat") {
                entry.1 = output_format.clone();
            } else {
                properties.push(("pkl.outputFormat".to_string(), output_format.clone()));
            }
        }
        let environment = config.environment.clone();

        Ok(Self {
            config,
            policy,
            properties,
            environment,
            packages: WasmPackageCache::default(),
            staged_sources: BTreeMap::new(),
            pending: None,
            cancelled_tokens: Vec::new(),
            saw_security_denial: false,
        })
    }

    pub fn reset_call_flags(&mut self) {
        self.saw_security_denial = false;
    }

    pub fn cancel_pending(&mut self) {
        if let Some(pending) = self.pending.take() {
            let tok = pending.token().to_string();
            if !self.cancelled_tokens.contains(&tok) {
                self.cancelled_tokens.push(tok);
            }
        }
    }

    pub fn drain_cancelled_tokens(&mut self) -> Vec<String> {
        std::mem::take(&mut self.cancelled_tokens)
    }

    pub fn close(&mut self) {
        self.cancel_pending();
        self.packages.clear();
        self.staged_sources.clear();
        self.properties = Vec::new();
        self.environment = Vec::new();
        self.config = WasmHostConfig::default();
        if let Ok(empty_policy) = SecurityPolicy::new(Some(&[]), Some(&[]), None) {
            self.policy = empty_policy;
        }
    }

    pub fn has_package_transport(&self) -> bool {
        self.config.has_package_transport || self.config.has_http_fetch
    }

    fn find_custom_module_reader(
        &self,
        uri: &PklString,
    ) -> Option<(usize, &CustomModuleReaderMeta)> {
        let raw = uri.to_java_utf8();
        let (scheme, _) = raw.split_once(':')?;
        self.config
            .custom_module_readers
            .iter()
            .enumerate()
            .find(|(_, r)| r.scheme.eq_ignore_ascii_case(scheme))
    }

    fn custom_module_reader_by_handle(
        &self,
        handle: &PklString,
    ) -> Option<(usize, &CustomModuleReaderMeta)> {
        let idx = handle
            .as_str()
            .and_then(|s| s.strip_prefix("custom:module:"))
            .and_then(|n| n.parse::<usize>().ok())?;
        self.config.custom_module_readers.get(idx).map(|r| (idx, r))
    }

    fn find_custom_resource_reader(
        &self,
        scheme: &str,
    ) -> Option<(usize, &CustomResourceReaderMeta)> {
        self.config
            .custom_resource_readers
            .iter()
            .enumerate()
            .rev()
            .find(|(_, r)| r.scheme.eq_ignore_ascii_case(scheme))
    }

    fn custom_resource_reader_by_handle(
        &self,
        handle: &PklString,
    ) -> Option<(usize, &CustomResourceReaderMeta)> {
        let idx = handle
            .as_str()
            .and_then(|s| s.strip_prefix("custom:resource:"))
            .and_then(|n| n.parse::<usize>().ok())?;
        self.config
            .custom_resource_readers
            .get(idx)
            .map(|r| (idx, r))
    }

    fn stage_source(&mut self, resolved: &ResolvedModule, source: ModuleSource) {
        self.staged_sources
            .insert(resolved.resolved_uri.clone(), source.clone());
        if resolved.original_uri != resolved.resolved_uri {
            self.staged_sources
                .insert(resolved.original_uri.clone(), source);
        }
    }

    fn record_module_err(&mut self, err: &ModuleHostError) {
        if matches!(err, ModuleHostError::SecurityManagerException { .. }) {
            self.saw_security_denial = true;
        }
    }

    fn record_read_err(&mut self, err: &ReadFailure) {
        if matches!(err, ReadFailure::SecurityManagerException { .. }) {
            self.saw_security_denial = true;
        }
    }

    fn check_policy_resolve_module(&mut self, uri: &PklString) -> Result<(), ModuleHostError> {
        let res = self.policy.check_resolve_module(uri);
        if let Err(ref err) = res {
            self.record_module_err(err);
        }
        res
    }

    fn check_policy_read_resource(&mut self, uri: &PklString) -> Result<(), ReadFailure> {
        let res = self.policy.check_read_resource(uri);
        if let Err(ref err) = res {
            self.record_read_err(err);
        }
        res
    }

    fn check_policy_resolve_resource(&mut self, uri: &PklString) -> Result<(), ModuleHostError> {
        let res = self.policy.check_resolve_resource(uri);
        if let Err(ref err) = res {
            self.record_module_err(err);
        }
        res
    }

    /// Validates an HTTP(S) package metadata/archive URL and enforces `allowedResources`
    /// or custom `securityManager.checkReadResource` before suspending to JS transport
    /// (matching upstream `PackageResolvers.java:195-207` and native `package_host.rs:78-92`).
    fn suspend_package_http(
        &mut self,
        url: &PklString,
        suspend: WasmSuspendRequest,
        after_fetch: PendingContinuation,
    ) -> Result<WasmHostOutcome, ModuleHostError> {
        let url_str = url.as_str().unwrap_or("");
        let lower = url_str.to_ascii_lowercase();
        if !lower.starts_with("http://") && !lower.starts_with("https://") {
            let err = ModuleHostError::Io(IoFailure::from(format!(
                "Expected HTTP(S) URL, but got: {url_str}"
            )));
            self.record_module_err(&err);
            return Err(err);
        }
        if self.config.has_custom_security_check_read {
            let check_req = WasmSuspendRequest::CheckReadResource {
                token: suspend.token().to_string(),
                uri: url.clone(),
            };
            self.pending = Some(PendingContinuation::PackageUrlCheck {
                next_suspend: Box::new(suspend),
                after_fetch: Box::new(after_fetch),
            });
            return Ok(WasmHostOutcome::Suspend(check_req));
        }
        if let Err(err) = self.check_policy_read_resource(url) {
            let mod_err = match err {
                ReadFailure::SecurityManagerException { message } => {
                    ModuleHostError::SecurityManagerException { message }
                }
                other => ModuleHostError::Io(IoFailure::from(format!("{other:?}"))),
            };
            self.record_module_err(&mod_err);
            return Err(mod_err);
        }
        self.pending = Some(after_fetch);
        Ok(WasmHostOutcome::Suspend(suspend))
    }

    // -----------------------------------------------------------------------
    // Entry points for HostPoll events
    // -----------------------------------------------------------------------

    pub fn handle_module(&mut self, token: String, request: ModuleRequest) -> WasmHostOutcome {
        match &request.kind {
            // 1. Custom module reader selection / capabilities / cache policy / import URI
            ModuleRequestKind::SelectKey { id, uri, .. } => {
                if let Some((idx, _)) = self.find_custom_module_reader(uri) {
                    return WasmHostOutcome::CompleteModule(ModuleCompletion::KeySelected(Ok(
                        ModuleKey {
                            id: *id,
                            uri: uri.clone(),
                            kind: ModuleKeyKind::Host,
                            handle: format!("custom:module:{idx}").into(),
                        },
                    )));
                }
                if let Some(comp) = default_completion(&request) {
                    return WasmHostOutcome::CompleteModule(comp);
                }
                if let Some(comp) = standard_key_completion_with_policy(&request, &self.policy) {
                    return WasmHostOutcome::CompleteModule(comp);
                }
                if self.staged_sources.contains_key(uri) {
                    return WasmHostOutcome::CompleteModule(ModuleCompletion::KeySelected(Ok(
                        ModuleKey {
                            id: *id,
                            uri: uri.clone(),
                            kind: ModuleKeyKind::Synthetic,
                            handle: "staged:inline".into(),
                        },
                    )));
                }
                WasmHostOutcome::CompleteModule(ModuleCompletion::KeySelected(Err(
                    ModuleHostError::NoLoader,
                )))
            }
            ModuleRequestKind::QueryCachePolicy { key } => {
                if self.staged_sources.contains_key(&key.uri) {
                    return WasmHostOutcome::CompleteModule(ModuleCompletion::CachePolicy(Ok(
                        false,
                    )));
                }
                if let Some((_, reader)) = self
                    .custom_module_reader_by_handle(&key.handle)
                    .or_else(|| self.find_custom_module_reader(&key.uri))
                {
                    return WasmHostOutcome::CompleteModule(ModuleCompletion::CachePolicy(Ok(
                        reader.is_cached,
                    )));
                }
                if let Some(comp) = default_completion(&request)
                    .or_else(|| standard_key_completion_with_policy(&request, &self.policy))
                {
                    return WasmHostOutcome::CompleteModule(comp);
                }
                WasmHostOutcome::CompleteModule(ModuleCompletion::CachePolicy(Ok(true)))
            }
            ModuleRequestKind::QueryFragmentPaths { key } => {
                if let Some((_, reader)) = self
                    .custom_module_reader_by_handle(&key.handle)
                    .or_else(|| self.find_custom_module_reader(&key.uri))
                {
                    return WasmHostOutcome::CompleteModule(ModuleCompletion::FragmentPaths(Ok(
                        reader.has_fragment_paths,
                    )));
                }
                if let Some(comp) = default_completion(&request)
                    .or_else(|| standard_key_completion_with_policy(&request, &self.policy))
                {
                    return WasmHostOutcome::CompleteModule(comp);
                }
                WasmHostOutcome::CompleteModule(ModuleCompletion::FragmentPaths(Ok(false)))
            }
            ModuleRequestKind::Capabilities { key, module } => {
                let uri = key.as_ref().map(|k| &k.uri).unwrap_or(&module.original_uri);
                if let Some((_, reader)) = key
                    .as_ref()
                    .and_then(|k| self.custom_module_reader_by_handle(&k.handle))
                    .or_else(|| self.find_custom_module_reader(uri))
                {
                    return WasmHostOutcome::CompleteModule(ModuleCompletion::Capabilities(Ok(
                        ModuleCapabilities {
                            local: reader.is_local,
                            hierarchical: reader.has_hierarchical_uris,
                            fragment_paths: reader.has_fragment_paths,
                        },
                    )));
                }
                if let Some(comp) = standard_key_completion_with_policy(&request, &self.policy) {
                    return WasmHostOutcome::CompleteModule(comp);
                }
                WasmHostOutcome::CompleteModule(ModuleCompletion::Capabilities(Ok(
                    ModuleCapabilities {
                        local: false,
                        hierarchical: false,
                        fragment_paths: false,
                    },
                )))
            }
            ModuleRequestKind::CheckImport {
                importer,
                import_uri,
            } => {
                if self.config.has_custom_security_check_import {
                    let req = WasmSuspendRequest::CheckImportModule {
                        token: token.clone(),
                        importing_uri: importer.original_uri.clone(),
                        imported_uri: import_uri.clone(),
                    };
                    self.pending = Some(PendingContinuation::ModuleCheckImport { token });
                    return WasmHostOutcome::Suspend(req);
                }
                let res = self
                    .policy
                    .check_import_module(&importer.resolved_uri, import_uri);
                if let Err(ref err) = res {
                    self.record_module_err(err);
                }
                WasmHostOutcome::CompleteModule(ModuleCompletion::ImportChecked(res))
            }
            ModuleRequestKind::ResolveImportUri { key, authored_uri } => {
                self.handle_resolve_import_uri(token, key.clone(), authored_uri.clone())
            }
            ModuleRequestKind::LoadSource { resolved, .. } => {
                let from_resolved = self.staged_sources.remove(&resolved.resolved_uri);
                let from_original = if resolved.original_uri != resolved.resolved_uri {
                    self.staged_sources.remove(&resolved.original_uri)
                } else {
                    None
                };
                if let Some(src) = from_resolved.or(from_original) {
                    return WasmHostOutcome::CompleteModule(ModuleCompletion::Source(Ok(src)));
                }
                WasmHostOutcome::CompleteModule(ModuleCompletion::Source(Err(
                    ModuleHostError::NotFound,
                )))
            }
            ModuleRequestKind::Resolve { .. } | ModuleRequestKind::ResolveKey { .. } => {
                self.continue_module_resolve(token, request, false)
            }
            ModuleRequestKind::HasElement { .. } => {
                self.continue_module_has_element(token, request, false)
            }
        }
    }

    fn handle_resolve_import_uri(
        &mut self,
        token: String,
        key: ModuleKey,
        authored_uri: PklString,
    ) -> WasmHostOutcome {
        let base_str = key.uri.as_str().unwrap_or("");
        let authored_str = authored_uri.as_str().unwrap_or("");

        // Custom module reader relative-URI guard
        if let Some((_, reader)) = self.custom_module_reader_by_handle(&key.handle)
            && !authored_str.starts_with('@')
            && !has_uri_scheme(authored_str)
            && !reader.has_hierarchical_uris
        {
            return WasmHostOutcome::CompleteModule(ModuleCompletion::ImportUri(Err(
                ModuleHostError::EvalError {
                    message: format!(
                        "Cannot resolve relative module URI `{authored_uri}` because enclosing module `{}` does not have hierarchical URIs.",
                        key.uri
                    )
                    .into(),
                    hint: None,
                },
            )));
        }

        // `@dep/...` dependency notation
        if let Some(stripped) = authored_str.strip_prefix('@') {
            let (dep_name, asset_path) = match stripped.find('/') {
                Some(idx) => (&stripped[..idx], &stripped[idx..]),
                None => (stripped, "/"),
            };
            if base_str.starts_with("package:") || base_str.starts_with("projectpackage:") {
                let asset_uri = match PackageAssetUri::parse(key.uri.clone()) {
                    Ok(u) => u,
                    Err(e) => {
                        return WasmHostOutcome::CompleteModule(ModuleCompletion::ImportUri(Err(
                            map_package_uri_error(e),
                        )));
                    }
                };
                let encoded_asset = percent_encode_fragment(asset_path);
                return self.resolve_package_dep_import(
                    token,
                    asset_uri,
                    dep_name.to_string(),
                    encoded_asset,
                );
            }
            // No PklProject in pure Wasm -> match exact Pkl 0.32.0 message
            let err = ModuleHostError::SecurityManagerException {
                message: Some(
                    "Cannot resolve dependency because there is no project found.\n\nIf you meant to import a path that starts with `@`, prefix the path with `./` (e.g. `import \"./@myPath\").\nIf you meant to import a dependency, ensure that this file is within a directory that contains a PklProject module."
                        .into(),
                ),
            };
            self.record_module_err(&err);
            return WasmHostOutcome::CompleteModule(ModuleCompletion::ImportUri(Err(err)));
        }

        // Relative fragment path inside `package:` or `projectpackage:`
        if (base_str.starts_with("package:") || base_str.starts_with("projectpackage:"))
            && !has_uri_scheme(authored_str)
        {
            let asset_uri = match PackageAssetUri::parse(key.uri.clone()) {
                Ok(u) => u,
                Err(e) => {
                    return WasmHostOutcome::CompleteModule(ModuleCompletion::ImportUri(Err(
                        map_package_uri_error(e),
                    )));
                }
            };
            let base_fragment = asset_uri.asset_path.as_str().unwrap_or("/");
            let new_fragment = resolve_relative_path(base_fragment, authored_str);
            let encoded_fragment = percent_encode_fragment(&new_fragment);
            let pkg_uri = asset_uri.package.uri.as_str().unwrap_or("");
            return WasmHostOutcome::CompleteModule(ModuleCompletion::ImportUri(Ok(format!(
                "{pkg_uri}#{encoded_fragment}"
            )
            .into())));
        }

        WasmHostOutcome::CompleteModule(ModuleCompletion::ImportUri(
            resolve_module_key_uri(&authored_uri, &key.uri)
                .map_err(|err| ModuleHostError::Unsupported(err.message)),
        ))
    }

    fn resolve_package_dep_import(
        &mut self,
        token: String,
        asset_uri: PackageAssetUri,
        dep_name: String,
        encoded_asset: String,
    ) -> WasmHostOutcome {
        match self
            .packages
            .get_metadata(&asset_uri.package, self.config.test_mode)
        {
            Ok(Some(meta)) => {
                for dep in &meta.dependencies {
                    if dep.name == dep_name.as_str() {
                        let target_uri = dep.package_uri.uri.as_str().unwrap_or("");
                        return WasmHostOutcome::CompleteModule(ModuleCompletion::ImportUri(Ok(
                            format!("{target_uri}#{encoded_asset}").into(),
                        )));
                    }
                }
                let clean_pkg = asset_uri.package.display_name();
                let err = ModuleHostError::SecurityManagerException {
                    message: Some(
                        format!(
                            "Cannot find dependency named `{dep_name}`, because it was not declared in package `{clean_pkg}`."
                        )
                        .into(),
                    ),
                };
                self.record_module_err(&err);
                WasmHostOutcome::CompleteModule(ModuleCompletion::ImportUri(Err(err)))
            }
            Ok(None) => {
                if !self.has_package_transport() {
                    return WasmHostOutcome::CapabilityUnavailable {
                        capability: "nativeHttp",
                        message: format!(
                            "Package fetch for `{}` requires `packageTransport` or `httpFetch` in Wasm.",
                            asset_uri.package.uri
                        ),
                    };
                }
                let request_url = get_metadata_request_uri(&asset_uri.package);
                if let Err(err) = validate_metadata_url(request_url.as_str().unwrap_or("")) {
                    self.record_module_err(&err);
                    return WasmHostOutcome::CompleteModule(ModuleCompletion::ImportUri(Err(err)));
                }
                let suspend = WasmSuspendRequest::PackageMetadata {
                    token: token.clone(),
                    package_uri: asset_uri.package.uri.clone(),
                    request_url: request_url.clone(),
                };
                match self.suspend_package_http(
                    &request_url,
                    suspend,
                    PendingContinuation::ModuleResolveDepImport {
                        token,
                        asset_uri,
                        dep_name,
                        encoded_asset,
                    },
                ) {
                    Ok(outcome) => outcome,
                    Err(err) => {
                        WasmHostOutcome::CompleteModule(ModuleCompletion::ImportUri(Err(err)))
                    }
                }
            }
            Err(err) => {
                self.record_module_err(&err);
                WasmHostOutcome::CompleteModule(ModuleCompletion::ImportUri(Err(err)))
            }
        }
    }

    fn finish_module_resolve_err(
        &mut self,
        request: &ModuleRequest,
        err: ModuleHostError,
    ) -> WasmHostOutcome {
        self.record_module_err(&err);
        if matches!(request.kind, ModuleRequestKind::ResolveKey { .. }) {
            WasmHostOutcome::CompleteModule(ModuleCompletion::KeyResolved(Err(err)))
        } else {
            WasmHostOutcome::CompleteModule(ModuleCompletion::Resolved(Err(err)))
        }
    }

    fn finish_module_resolve_ok(
        &mut self,
        request: &ModuleRequest,
        resolved: ResolvedModule,
        source: Option<ModuleSource>,
    ) -> WasmHostOutcome {
        if let Some(src) = source {
            self.stage_source(&resolved, src);
        }
        match &request.kind {
            ModuleRequestKind::ResolveKey {
                key,
                resolved_key_id,
                ..
            } => WasmHostOutcome::CompleteModule(ModuleCompletion::KeyResolved(Ok(
                ResolvedModuleKey {
                    id: *resolved_key_id,
                    key: key.clone(),
                    handle: key.handle.clone(),
                    module: resolved,
                },
            ))),
            _ => WasmHostOutcome::CompleteModule(ModuleCompletion::Resolved(Ok(resolved))),
        }
    }

    fn continue_module_resolve(
        &mut self,
        token: String,
        request: ModuleRequest,
        security_checked: bool,
    ) -> WasmHostOutcome {
        let key = match &request.kind {
            ModuleRequestKind::Resolve { key, .. } | ModuleRequestKind::ResolveKey { key, .. } => {
                key.clone()
            }
            _ => {
                return WasmHostOutcome::CompleteModule(unsupported_module_completion(&request));
            }
        };

        // Security check BEFORE any I/O, stdlib resolution, or capability check
        // (mirrors `ModuleKeys.StandardLibrary.resolve(SecurityManager)`).
        if !security_checked {
            if self.config.has_custom_security_check_resolve {
                let suspend = WasmSuspendRequest::CheckResolveModule {
                    token: token.clone(),
                    uri: key.uri.clone(),
                };
                self.pending = Some(PendingContinuation::ModuleResolve {
                    token,
                    request,
                    security_checked: true,
                });
                return WasmHostOutcome::Suspend(suspend);
            }
            if let Err(err) = self.check_policy_resolve_module(&key.uri) {
                return self.finish_module_resolve_err(&request, err);
            }
        }

        // Staged inline source (e.g. `ReplSession.load(ModuleSource.text(...))`), served
        // after security checks without mutating any caller-supplied VFS.
        if self.staged_sources.contains_key(&key.uri) {
            let resolved = ResolvedModule {
                original_uri: key.uri.clone(),
                resolved_uri: key.uri.clone(),
                source_uri: key.uri.clone(),
            };
            return self.finish_module_resolve_ok(&request, resolved, None);
        }

        // Builtin Pkl 0.32.0 standard library (`pkl:*`), supporting both VM evaluation (`Resolve`)
        // and static import graph analysis (`ResolveKey` + subsequent `LoadSource`).
        if key.kind == ModuleKeyKind::StandardLibrary {
            let Some(stub_src) = stdlib_analysis_source(&key.uri) else {
                return self.finish_module_resolve_err(&request, ModuleHostError::NotFound);
            };
            let resolved = ResolvedModule {
                original_uri: key.uri.clone(),
                resolved_uri: key.uri.clone(),
                source_uri: key.uri.clone(),
            };
            let staged = if matches!(request.kind, ModuleRequestKind::ResolveKey { .. }) {
                Some(ModuleSource {
                    text: SourceText::from(stub_src),
                    inferred_name: PklString::default(),
                })
            } else {
                None
            };
            return self.finish_module_resolve_ok(&request, resolved, staged);
        }

        // Synthetic inline / query module (`repl:text`)
        if key.kind == ModuleKeyKind::Synthetic {
            let resolved = ResolvedModule {
                original_uri: key.uri.clone(),
                resolved_uri: key.uri.clone(),
                source_uri: key.uri.clone(),
            };
            return self.finish_module_resolve_ok(&request, resolved, None);
        }

        // Custom module reader
        if let Some((idx, reader)) = self.custom_module_reader_by_handle(&key.handle) {
            let scheme: PklString = reader.scheme.as_str().into();
            let suspend = WasmSuspendRequest::ReadModule {
                token: token.clone(),
                backend: SuspendBackend::Custom(idx),
                scheme,
                uri: key.uri.clone(),
            };
            self.pending = Some(PendingContinuation::ModuleResolve {
                token,
                request,
                security_checked: true,
            });
            return WasmHostOutcome::Suspend(suspend);
        }

        let raw_uri = key.uri.to_java_utf8();
        let lower = raw_uri.to_ascii_lowercase();

        // `package:` / `projectpackage:`
        if lower.starts_with("package:") || lower.starts_with("projectpackage:") {
            if lower.starts_with("projectpackage:") {
                return WasmHostOutcome::CapabilityUnavailable {
                    capability: "projectResolution",
                    message: format!(
                        "Project package URI `{raw_uri}` requires `PklProject` resolution, which is unavailable in Wasm."
                    ),
                };
            }
            let asset_uri = match PackageAssetUri::parse(key.uri.clone()) {
                Ok(u) => u,
                Err(e) => {
                    return self.finish_module_resolve_err(&request, map_package_uri_error(e));
                }
            };
            return self.continue_package_module_resolve(token, request, asset_uri);
        }

        // `file:`
        if key.kind == ModuleKeyKind::StandardFile || lower.starts_with("file:") {
            if !self.config.has_vfs {
                return WasmHostOutcome::CapabilityUnavailable {
                    capability: "nativeFs",
                    message: format!(
                        "Cannot load `{raw_uri}`: native filesystem access is unavailable in Wasm (attach a `VirtualFileSystem` via `options.vfs`)."
                    ),
                };
            }
            let suspend = WasmSuspendRequest::ReadModule {
                token: token.clone(),
                backend: SuspendBackend::Vfs,
                scheme: "file".into(),
                uri: key.uri.clone(),
            };
            self.pending = Some(PendingContinuation::ModuleResolve {
                token,
                request,
                security_checked: true,
            });
            return WasmHostOutcome::Suspend(suspend);
        }

        // `http:` / `https:`
        if key.kind == ModuleKeyKind::StandardHttp
            || lower.starts_with("http:")
            || lower.starts_with("https:")
        {
            if !self.config.has_http_fetch {
                return WasmHostOutcome::CapabilityUnavailable {
                    capability: "nativeHttp",
                    message: format!(
                        "Cannot load `{raw_uri}`: native HTTP access is unavailable in Wasm (provide `options.httpFetch`)."
                    ),
                };
            }
            let scheme = if lower.starts_with("https:") {
                "https"
            } else {
                "http"
            };
            let suspend = WasmSuspendRequest::ReadModule {
                token: token.clone(),
                backend: SuspendBackend::Http,
                scheme: scheme.into(),
                uri: key.uri.clone(),
            };
            self.pending = Some(PendingContinuation::ModuleResolve {
                token,
                request,
                security_checked: true,
            });
            return WasmHostOutcome::Suspend(suspend);
        }

        // `modulepath:` / `jar:`
        if key.handle == "builtin:modulepath"
            || lower.starts_with("modulepath:")
            || lower.starts_with("jar:")
        {
            return WasmHostOutcome::CapabilityUnavailable {
                capability: "modulePathJar",
                message: format!(
                    "Cannot load `{raw_uri}`: `modulepath:` / JAR archives are unavailable in Wasm."
                ),
            };
        }

        self.finish_module_resolve_err(&request, ModuleHostError::NoLoader)
    }

    fn continue_package_module_resolve(
        &mut self,
        token: String,
        request: ModuleRequest,
        asset_uri: PackageAssetUri,
    ) -> WasmHostOutcome {
        let meta = match self
            .packages
            .get_metadata(&asset_uri.package, self.config.test_mode)
        {
            Ok(Some(meta)) => meta,
            Ok(None) => {
                if !self.has_package_transport() {
                    return WasmHostOutcome::CapabilityUnavailable {
                        capability: "nativeHttp",
                        message: format!(
                            "Package fetch for `{}` requires `packageTransport` or `httpFetch` in Wasm.",
                            asset_uri.package.uri
                        ),
                    };
                }
                let request_url = get_metadata_request_uri(&asset_uri.package);
                if let Err(err) = validate_metadata_url(request_url.as_str().unwrap_or("")) {
                    return self.finish_module_resolve_err(&request, err);
                }
                let suspend = WasmSuspendRequest::PackageMetadata {
                    token: token.clone(),
                    package_uri: asset_uri.package.uri.clone(),
                    request_url: request_url.clone(),
                };
                return match self.suspend_package_http(
                    &request_url,
                    suspend,
                    PendingContinuation::ModuleResolve {
                        token,
                        request: request.clone(),
                        security_checked: true,
                    },
                ) {
                    Ok(outcome) => outcome,
                    Err(err) => self.finish_module_resolve_err(&request, err),
                };
            }
            Err(err) => return self.finish_module_resolve_err(&request, err),
        };

        let Some(archive) = self.packages.get_archive(&asset_uri.package) else {
            if !self.has_package_transport() {
                return WasmHostOutcome::CapabilityUnavailable {
                    capability: "nativeHttp",
                    message: format!(
                        "Package archive fetch for `{}` requires `packageTransport` or `httpFetch` in Wasm.",
                        asset_uri.package.uri
                    ),
                };
            }
            let zip_url = meta.package_zip_url.clone();
            let suspend = WasmSuspendRequest::PackageArchive {
                token: token.clone(),
                package_uri: asset_uri.package.uri.clone(),
                zip_url: zip_url.clone(),
            };
            return match self.suspend_package_http(
                &zip_url,
                suspend,
                PendingContinuation::ModuleResolve {
                    token,
                    request: request.clone(),
                    security_checked: true,
                },
            ) {
                Ok(outcome) => outcome,
                Err(err) => self.finish_module_resolve_err(&request, err),
            };
        };

        let asset_path = asset_uri.asset_path.as_str().unwrap_or("");
        match archive.read_bytes(asset_path, false) {
            Ok(bytes) => match String::from_utf8(bytes) {
                Ok(text) => {
                    let uri = asset_uri.uri.clone();
                    let inferred_name = infer_name_for_uri(&uri.to_java_utf8());
                    let resolved = ResolvedModule {
                        original_uri: uri.clone(),
                        resolved_uri: uri.clone(),
                        source_uri: uri,
                    };
                    let source = ModuleSource {
                        text: SourceText::from(text),
                        inferred_name,
                    };
                    self.finish_module_resolve_ok(&request, resolved, Some(source))
                }
                Err(err) => self.finish_module_resolve_err(
                    &request,
                    ModuleHostError::Io(IoFailure::from(err.to_string())),
                ),
            },
            Err(ZipReadError::IsADirectory) => self.finish_module_resolve_err(
                &request,
                ModuleHostError::Io(IoFailure::from("Is a directory")),
            ),
            Err(ZipReadError::NotFound) => {
                self.finish_module_resolve_err(&request, ModuleHostError::NotFound)
            }
        }
    }

    fn continue_module_has_element(
        &mut self,
        token: String,
        request: ModuleRequest,
        security_checked: bool,
    ) -> WasmHostOutcome {
        let ModuleRequestKind::HasElement {
            key,
            module,
            element_uri,
        } = &request.kind
        else {
            return WasmHostOutcome::CompleteModule(unsupported_module_completion(&request));
        };
        let element_uri = element_uri.clone();

        if !security_checked {
            if self.config.has_custom_security_check_resolve {
                let suspend = WasmSuspendRequest::CheckResolveModule {
                    token: token.clone(),
                    uri: element_uri,
                };
                self.pending = Some(PendingContinuation::ModuleHasElement {
                    token,
                    request,
                    security_checked: true,
                });
                return WasmHostOutcome::Suspend(suspend);
            }
            if let Err(err) = self.check_policy_resolve_module(&element_uri) {
                return WasmHostOutcome::CompleteModule(ModuleCompletion::HasElement(Err(err)));
            }
        }

        if let Some((idx, _)) = key
            .as_ref()
            .and_then(|k| self.custom_module_reader_by_handle(&k.handle))
            .or_else(|| self.find_custom_module_reader(&element_uri))
            .or_else(|| self.find_custom_module_reader(&module.original_uri))
        {
            let suspend = WasmSuspendRequest::ModuleHasElement {
                token: token.clone(),
                backend: SuspendBackend::Custom(idx),
                uri: element_uri,
            };
            self.pending = Some(PendingContinuation::ModuleHasElement {
                token,
                request,
                security_checked: true,
            });
            return WasmHostOutcome::Suspend(suspend);
        }

        let lower = element_uri.to_java_utf8().to_ascii_lowercase();
        if lower.starts_with("package:") || lower.starts_with("projectpackage:") {
            let asset_uri = match PackageAssetUri::parse(element_uri) {
                Ok(u) => u,
                Err(e) => {
                    return WasmHostOutcome::CompleteModule(ModuleCompletion::HasElement(Err(
                        map_package_uri_error(e),
                    )));
                }
            };
            return self.continue_package_has_element_module(token, request, asset_uri);
        }

        if lower.starts_with("file:") {
            if !self.config.has_vfs {
                return WasmHostOutcome::CapabilityUnavailable {
                    capability: "nativeFs",
                    message: format!(
                        "Cannot probe `{element_uri}`: native filesystem access is unavailable in Wasm (attach a `VirtualFileSystem` via `options.vfs`)."
                    ),
                };
            }
            let suspend = WasmSuspendRequest::ModuleHasElement {
                token: token.clone(),
                backend: SuspendBackend::Vfs,
                uri: element_uri,
            };
            self.pending = Some(PendingContinuation::ModuleHasElement {
                token,
                request,
                security_checked: true,
            });
            return WasmHostOutcome::Suspend(suspend);
        }

        if lower.starts_with("modulepath:") || lower.starts_with("jar:") {
            return WasmHostOutcome::CapabilityUnavailable {
                capability: "modulePathJar",
                message: format!(
                    "Cannot probe `{element_uri}`: `modulepath:` / JAR archives are unavailable in Wasm."
                ),
            };
        }

        WasmHostOutcome::CompleteModule(ModuleCompletion::HasElement(Ok(false)))
    }

    fn continue_package_has_element_module(
        &mut self,
        token: String,
        request: ModuleRequest,
        asset_uri: PackageAssetUri,
    ) -> WasmHostOutcome {
        let meta = match self
            .packages
            .get_metadata(&asset_uri.package, self.config.test_mode)
        {
            Ok(Some(m)) => m,
            Ok(None) => {
                if !self.has_package_transport() {
                    return WasmHostOutcome::CapabilityUnavailable {
                        capability: "nativeHttp",
                        message: format!(
                            "Package fetch for `{}` requires `packageTransport` or `httpFetch` in Wasm.",
                            asset_uri.package.uri
                        ),
                    };
                }
                let request_url = get_metadata_request_uri(&asset_uri.package);
                if let Err(err) = validate_metadata_url(request_url.as_str().unwrap_or("")) {
                    self.record_module_err(&err);
                    return WasmHostOutcome::CompleteModule(ModuleCompletion::HasElement(Err(err)));
                }
                let suspend = WasmSuspendRequest::PackageMetadata {
                    token: token.clone(),
                    package_uri: asset_uri.package.uri.clone(),
                    request_url: request_url.clone(),
                };
                return match self.suspend_package_http(
                    &request_url,
                    suspend,
                    PendingContinuation::ModuleHasElement {
                        token,
                        request,
                        security_checked: true,
                    },
                ) {
                    Ok(outcome) => outcome,
                    Err(err) => {
                        WasmHostOutcome::CompleteModule(ModuleCompletion::HasElement(Err(err)))
                    }
                };
            }
            Err(err) => {
                self.record_module_err(&err);
                return WasmHostOutcome::CompleteModule(ModuleCompletion::HasElement(Err(err)));
            }
        };

        let Some(archive) = self.packages.get_archive(&asset_uri.package) else {
            if !self.has_package_transport() {
                return WasmHostOutcome::CapabilityUnavailable {
                    capability: "nativeHttp",
                    message: format!(
                        "Package archive fetch for `{}` requires `packageTransport` or `httpFetch` in Wasm.",
                        asset_uri.package.uri
                    ),
                };
            }
            let zip_url = meta.package_zip_url.clone();
            let suspend = WasmSuspendRequest::PackageArchive {
                token: token.clone(),
                package_uri: asset_uri.package.uri.clone(),
                zip_url: zip_url.clone(),
            };
            return match self.suspend_package_http(
                &zip_url,
                suspend,
                PendingContinuation::ModuleHasElement {
                    token,
                    request,
                    security_checked: true,
                },
            ) {
                Ok(outcome) => outcome,
                Err(err) => WasmHostOutcome::CompleteModule(ModuleCompletion::HasElement(Err(err))),
            };
        };

        let exists = archive.has_element(asset_uri.asset_path.as_str().unwrap_or(""));
        WasmHostOutcome::CompleteModule(ModuleCompletion::HasElement(Ok(exists)))
    }

    // -----------------------------------------------------------------------
    // Resource reads
    // -----------------------------------------------------------------------

    pub fn handle_read(&mut self, token: String, req: &ReadRequest) -> WasmHostOutcome {
        if let Some(resolved_uri) = &req.resolved_uri {
            return self.continue_read_resource(token, resolved_uri.clone(), false);
        }
        let authored_raw = req.uri.to_java_utf8();

        let resolved_uri = if !has_uri_scheme(&authored_raw)
            && let Some(base) = req.module_uri.as_ref()
            && let Some((_, custom_mod)) = self.find_custom_module_reader(base)
        {
            if !custom_mod.has_hierarchical_uris {
                return WasmHostOutcome::CompleteRead(Err(ReadFailure::EvalError {
                    message: format!(
                        "Cannot resolve relative resource URI `{authored_raw}` because enclosing module `{}` does not have hierarchical URIs.",
                        base.to_java_utf8()
                    )
                    .into(),
                    hint: None,
                }));
            }
            match resolve_module_key_uri(&req.uri, base) {
                Ok(u) => u,
                Err(err) => {
                    return WasmHostOutcome::CompleteRead(Err(ReadFailure::EvalError {
                        message: err.message.clone(),
                        hint: err.hint.clone(),
                    }));
                }
            }
        } else if !has_uri_scheme(&authored_raw)
            && let Some(base) = req.module_uri.as_ref()
            && (base.starts_with("package:") || base.starts_with("projectpackage:"))
        {
            match PackageAssetUri::parse(base.clone()) {
                Ok(asset_uri) => {
                    let base_fragment = asset_uri.asset_path.as_str().unwrap_or("/");
                    let new_fragment = resolve_relative_path(base_fragment, &authored_raw);
                    let encoded = percent_encode_fragment(&new_fragment);
                    let pkg = asset_uri.package.uri.as_str().unwrap_or("");
                    PklString::from(format!("{pkg}#{encoded}"))
                }
                Err(err) => {
                    return WasmHostOutcome::CompleteRead(Err(module_err_to_read_failure(
                        map_package_uri_error(err),
                    )));
                }
            }
        } else {
            match resolve_resource_read_uri(&req.uri, req.module_uri.as_ref()) {
                Ok(u) => u,
                Err(err) => {
                    return WasmHostOutcome::CompleteRead(Err(ReadFailure::EvalError {
                        message: err.message.clone(),
                        hint: err.hint.clone(),
                    }));
                }
            }
        };

        self.continue_read_resource(token, resolved_uri, false)
    }

    fn continue_read_resource(
        &mut self,
        token: String,
        resolved_uri: PklString,
        security_checked: bool,
    ) -> WasmHostOutcome {
        if !security_checked {
            if self.config.has_custom_security_check_read {
                let suspend = WasmSuspendRequest::CheckReadResource {
                    token: token.clone(),
                    uri: resolved_uri.clone(),
                };
                self.pending = Some(PendingContinuation::ReadResource {
                    token,
                    resolved_uri,
                    security_checked: true,
                });
                return WasmHostOutcome::Suspend(suspend);
            }
            if let Err(err) = self.check_policy_read_resource(&resolved_uri) {
                return WasmHostOutcome::CompleteRead(Err(err));
            }
        }

        let raw = resolved_uri.to_java_utf8();
        let scheme = raw.split_once(':').map(|(s, _)| s).unwrap_or("");

        // 1. Custom resource reader FIRST
        if let Some((idx, reader)) = self.find_custom_resource_reader(scheme) {
            let scheme_pkl: PklString = reader.scheme.as_str().into();
            let suspend = WasmSuspendRequest::ReadResource {
                token: token.clone(),
                backend: SuspendBackend::Custom(idx),
                scheme: scheme_pkl,
                uri: resolved_uri.clone(),
            };
            self.pending = Some(PendingContinuation::ReadResource {
                token,
                resolved_uri,
                security_checked: true,
            });
            return WasmHostOutcome::Suspend(suspend);
        }

        // 2. `package:` / `projectpackage:`
        if scheme.eq_ignore_ascii_case("package") || scheme.eq_ignore_ascii_case("projectpackage") {
            if scheme.eq_ignore_ascii_case("projectpackage") {
                return WasmHostOutcome::CapabilityUnavailable {
                    capability: "projectResolution",
                    message: format!(
                        "Project package URI `{raw}` requires `PklProject` resolution, which is unavailable in Wasm."
                    ),
                };
            }
            let asset_uri = match PackageAssetUri::parse(resolved_uri.clone()) {
                Ok(u) => u,
                Err(e) => {
                    return WasmHostOutcome::CompleteRead(Err(module_err_to_read_failure(
                        map_package_uri_error(e),
                    )));
                }
            };
            return self.continue_package_read_resource(token, resolved_uri, asset_uri);
        }

        // 3. `http:` / `https:`
        if scheme.eq_ignore_ascii_case("http") || scheme.eq_ignore_ascii_case("https") {
            if !self.config.has_http_fetch {
                return WasmHostOutcome::CapabilityUnavailable {
                    capability: "nativeHttp",
                    message: format!(
                        "Cannot read resource `{raw}`: native HTTP access is unavailable in Wasm (provide `options.httpFetch`)."
                    ),
                };
            }
            let suspend = WasmSuspendRequest::ReadResource {
                token: token.clone(),
                backend: SuspendBackend::Http,
                scheme: scheme.to_ascii_lowercase().into(),
                uri: resolved_uri.clone(),
            };
            self.pending = Some(PendingContinuation::ReadResource {
                token,
                resolved_uri,
                security_checked: true,
            });
            return WasmHostOutcome::Suspend(suspend);
        }

        // 4. `env:` / `prop:` scalar reads
        if scheme.eq_ignore_ascii_case("env") || scheme.eq_ignore_ascii_case("prop") {
            let allow_all = SecurityPolicy::new(Some(&[".*".into()]), Some(&[".*".into()]), None)
                .expect("valid wildcard policy");
            let effective_policy = if self.config.has_custom_security_check_read {
                &allow_all
            } else {
                &self.policy
            };
            match try_configured_scalar_read_with_policy(
                &resolved_uri,
                &self.properties,
                &self.environment,
                effective_policy,
            ) {
                Ok(Some(result)) => return WasmHostOutcome::CompleteRead(Ok(result)),
                Ok(None) => {}
                Err(err) => {
                    self.record_read_err(&err);
                    return WasmHostOutcome::CompleteRead(Err(err));
                }
            }
        }

        // 5. `file:`
        if scheme.eq_ignore_ascii_case("file") {
            if let Err(reason) = validate_file_uri_has_leading_slash(&resolved_uri) {
                return WasmHostOutcome::CompleteRead(Err(ReadFailure::UriSyntax { reason }));
            }
            if !self.config.has_vfs {
                return WasmHostOutcome::CapabilityUnavailable {
                    capability: "nativeFs",
                    message: format!(
                        "Cannot read resource `{raw}`: native filesystem access is unavailable in Wasm (attach a `VirtualFileSystem` via `options.vfs`)."
                    ),
                };
            }
            let suspend = WasmSuspendRequest::ReadResource {
                token: token.clone(),
                backend: SuspendBackend::Vfs,
                scheme: "file".into(),
                uri: resolved_uri.clone(),
            };
            self.pending = Some(PendingContinuation::ReadResource {
                token,
                resolved_uri,
                security_checked: true,
            });
            return WasmHostOutcome::Suspend(suspend);
        }

        // 6. `modulepath:` / `jar:`
        if scheme.eq_ignore_ascii_case("modulepath") || scheme.eq_ignore_ascii_case("jar") {
            return WasmHostOutcome::CapabilityUnavailable {
                capability: "modulePathJar",
                message: format!(
                    "Cannot read resource `{raw}`: `modulepath:` / JAR archives are unavailable in Wasm."
                ),
            };
        }

        WasmHostOutcome::CompleteRead(Err(ReadFailure::EvalError {
            message: format!("No resource reader is registered for scheme `{scheme}`.").into(),
            hint: None,
        }))
    }

    fn continue_package_read_resource(
        &mut self,
        token: String,
        resolved_uri: PklString,
        asset_uri: PackageAssetUri,
    ) -> WasmHostOutcome {
        let meta = match self
            .packages
            .get_metadata(&asset_uri.package, self.config.test_mode)
        {
            Ok(Some(m)) => m,
            Ok(None) => {
                if !self.has_package_transport() {
                    return WasmHostOutcome::CapabilityUnavailable {
                        capability: "nativeHttp",
                        message: format!(
                            "Package fetch for `{}` requires `packageTransport` or `httpFetch` in Wasm.",
                            asset_uri.package.uri
                        ),
                    };
                }
                let request_url = get_metadata_request_uri(&asset_uri.package);
                if let Err(err) = validate_metadata_url(request_url.as_str().unwrap_or("")) {
                    self.record_module_err(&err);
                    return WasmHostOutcome::CompleteRead(Err(module_err_to_read_failure(err)));
                }
                let suspend = WasmSuspendRequest::PackageMetadata {
                    token: token.clone(),
                    package_uri: asset_uri.package.uri.clone(),
                    request_url: request_url.clone(),
                };
                return match self.suspend_package_http(
                    &request_url,
                    suspend,
                    PendingContinuation::ReadResource {
                        token,
                        resolved_uri,
                        security_checked: true,
                    },
                ) {
                    Ok(outcome) => outcome,
                    Err(err) => WasmHostOutcome::CompleteRead(Err(module_err_to_read_failure(err))),
                };
            }
            Err(err) => {
                self.record_module_err(&err);
                return WasmHostOutcome::CompleteRead(Err(module_err_to_read_failure(err)));
            }
        };

        let Some(archive) = self.packages.get_archive(&asset_uri.package) else {
            if !self.has_package_transport() {
                return WasmHostOutcome::CapabilityUnavailable {
                    capability: "nativeHttp",
                    message: format!(
                        "Package archive fetch for `{}` requires `packageTransport` or `httpFetch` in Wasm.",
                        asset_uri.package.uri
                    ),
                };
            }
            let zip_url = meta.package_zip_url.clone();
            let suspend = WasmSuspendRequest::PackageArchive {
                token: token.clone(),
                package_uri: asset_uri.package.uri.clone(),
                zip_url: zip_url.clone(),
            };
            return match self.suspend_package_http(
                &zip_url,
                suspend,
                PendingContinuation::ReadResource {
                    token,
                    resolved_uri,
                    security_checked: true,
                },
            ) {
                Ok(outcome) => outcome,
                Err(err) => WasmHostOutcome::CompleteRead(Err(module_err_to_read_failure(err))),
            };
        };

        match archive.read_bytes(asset_uri.asset_path.as_str().unwrap_or(""), true) {
            Ok(bytes) => WasmHostOutcome::CompleteRead(Ok(ReadResult::Resource(Resource {
                uri: resolved_uri,
                bytes: bytes.into(),
            }))),
            Err(ZipReadError::NotFound) => WasmHostOutcome::CompleteRead(Ok(ReadResult::Missing)),
            Err(ZipReadError::IsADirectory) => WasmHostOutcome::CompleteRead(Err(ReadFailure::Io(
                IoFailure::from("Is a directory"),
            ))),
        }
    }

    // -----------------------------------------------------------------------
    // Glob requests (`import*` / `read*`)
    // -----------------------------------------------------------------------

    pub fn handle_glob(&mut self, token: String, request: GlobRequest) -> WasmHostOutcome {
        match &request.kind {
            GlobRequestKind::SelectResource { id, uri } => {
                let raw = uri.to_java_utf8();
                if let Some((scheme, _)) = raw.split_once(':')
                    && let Some((idx, reader)) = self.find_custom_resource_reader(scheme)
                {
                    return WasmHostOutcome::CompleteGlob(GlobCompletion::Resource(Ok(
                        ResourceReader {
                            id: *id,
                            scheme: reader.scheme.as_str().into(),
                            handle: format!("custom:resource:{idx}").into(),
                        },
                    )));
                }
                if uri_scheme_eq(uri, "package") || uri_scheme_eq(uri, "projectpackage") {
                    let scheme = if uri_scheme_eq(uri, "package") {
                        "package"
                    } else {
                        "projectpackage"
                    };
                    return WasmHostOutcome::CompleteGlob(GlobCompletion::Resource(Ok(
                        ResourceReader {
                            id: *id,
                            scheme: scheme.into(),
                            handle: format!("builtin:{scheme}").into(),
                        },
                    )));
                }
                let allow_all =
                    SecurityPolicy::new(Some(&[".*".into()]), Some(&[".*".into()]), None)
                        .expect("valid wildcard policy");
                let effective_policy = if self.config.has_custom_security_check_read
                    || self.config.has_custom_security_check_resolve_resource
                {
                    &allow_all
                } else {
                    &self.policy
                };
                if let Some(comp) = configured_completion_with_policy(
                    &request,
                    &self.properties,
                    &self.environment,
                    effective_policy,
                ) {
                    return WasmHostOutcome::CompleteGlob(comp);
                }
                WasmHostOutcome::CompleteGlob(GlobCompletion::Resource(Err(
                    GlobHostError::ReaderNotFound,
                )))
            }
            GlobRequestKind::IsGlobbable { reader } => match reader {
                GlobReader::Module(key) => {
                    if let Some((_, r)) = self
                        .custom_module_reader_by_handle(&key.handle)
                        .or_else(|| self.find_custom_module_reader(&key.uri))
                    {
                        return WasmHostOutcome::CompleteGlob(GlobCompletion::Globbable(Ok(
                            r.is_globbable
                        )));
                    }
                    if is_package_glob_reader(reader) {
                        return WasmHostOutcome::CompleteGlob(GlobCompletion::Globbable(Ok(true)));
                    }
                    if let Some(comp) = configured_completion_with_policy(
                        &request,
                        &self.properties,
                        &self.environment,
                        &self.policy,
                    ) {
                        return WasmHostOutcome::CompleteGlob(comp);
                    }
                    let globbable =
                        key.kind == ModuleKeyKind::StandardFile || uri_scheme_eq(&key.uri, "file");
                    WasmHostOutcome::CompleteGlob(GlobCompletion::Globbable(Ok(globbable)))
                }
                GlobReader::Resource(res) => {
                    if let Some((_, r)) = self
                        .custom_resource_reader_by_handle(&res.handle)
                        .or_else(|| {
                            self.find_custom_resource_reader(res.scheme.as_str().unwrap_or(""))
                        })
                    {
                        return WasmHostOutcome::CompleteGlob(GlobCompletion::Globbable(Ok(
                            r.is_globbable
                        )));
                    }
                    if is_package_glob_reader(reader) {
                        return WasmHostOutcome::CompleteGlob(GlobCompletion::Globbable(Ok(true)));
                    }
                    if let Some(comp) = configured_completion_with_policy(
                        &request,
                        &self.properties,
                        &self.environment,
                        &self.policy,
                    ) {
                        return WasmHostOutcome::CompleteGlob(comp);
                    }
                    WasmHostOutcome::CompleteGlob(GlobCompletion::Globbable(Ok(false)))
                }
            },
            GlobRequestKind::IsHierarchical { reader } => match reader {
                GlobReader::Module(key) => {
                    if let Some((_, r)) = self
                        .custom_module_reader_by_handle(&key.handle)
                        .or_else(|| self.find_custom_module_reader(&key.uri))
                    {
                        return WasmHostOutcome::CompleteGlob(GlobCompletion::Hierarchical(Ok(
                            r.has_hierarchical_uris
                        )));
                    }
                    if is_package_glob_reader(reader) {
                        return WasmHostOutcome::CompleteGlob(GlobCompletion::Hierarchical(Ok(
                            true,
                        )));
                    }
                    let hierarchical = key.kind == ModuleKeyKind::StandardFile
                        || key.handle == "builtin:modulepath"
                        || uri_scheme_eq(&key.uri, "file")
                        || uri_scheme_eq(&key.uri, "modulepath")
                        || uri_scheme_eq(&key.uri, "http")
                        || uri_scheme_eq(&key.uri, "https");
                    WasmHostOutcome::CompleteGlob(GlobCompletion::Hierarchical(Ok(hierarchical)))
                }
                GlobReader::Resource(res) => {
                    if let Some((_, r)) = self
                        .custom_resource_reader_by_handle(&res.handle)
                        .or_else(|| {
                            self.find_custom_resource_reader(res.scheme.as_str().unwrap_or(""))
                        })
                    {
                        return WasmHostOutcome::CompleteGlob(GlobCompletion::Hierarchical(Ok(
                            r.has_hierarchical_uris
                        )));
                    }
                    if is_package_glob_reader(reader) {
                        return WasmHostOutcome::CompleteGlob(GlobCompletion::Hierarchical(Ok(
                            true,
                        )));
                    }
                    if let Some(comp) = configured_completion_with_policy(
                        &request,
                        &self.properties,
                        &self.environment,
                        &self.policy,
                    ) {
                        return WasmHostOutcome::CompleteGlob(comp);
                    }
                    WasmHostOutcome::CompleteGlob(GlobCompletion::Hierarchical(Ok(false)))
                }
            },
            GlobRequestKind::FragmentPaths { reader } => {
                if self
                    .custom_resource_reader_by_handle(&reader.handle)
                    .is_some()
                {
                    return WasmHostOutcome::CompleteGlob(GlobCompletion::FragmentPaths(Ok(false)));
                }
                if reader.scheme == "package" || reader.scheme == "projectpackage" {
                    return WasmHostOutcome::CompleteGlob(GlobCompletion::FragmentPaths(Ok(true)));
                }
                if let Some(comp) = configured_completion_with_policy(
                    &request,
                    &self.properties,
                    &self.environment,
                    &self.policy,
                ) {
                    return WasmHostOutcome::CompleteGlob(comp);
                }
                WasmHostOutcome::CompleteGlob(GlobCompletion::FragmentPaths(Ok(false)))
            }
            GlobRequestKind::HasElement { .. } => {
                self.continue_glob_has_element(token, request, false)
            }
            GlobRequestKind::ListElements { .. } => {
                self.continue_glob_list_elements(token, request, false)
            }
        }
    }

    fn continue_glob_has_element(
        &mut self,
        token: String,
        request: GlobRequest,
        security_checked: bool,
    ) -> WasmHostOutcome {
        let GlobRequestKind::HasElement { reader, uri } = &request.kind else {
            return WasmHostOutcome::CompleteGlob(unsupported_glob_completion(&request));
        };
        let uri = uri.clone();

        if !security_checked {
            if self.config.has_custom_security_check_resolve_resource {
                let suspend = WasmSuspendRequest::CheckResolveResource {
                    token: token.clone(),
                    uri,
                };
                self.pending = Some(PendingContinuation::GlobHasElement {
                    token,
                    request,
                    security_checked: true,
                });
                return WasmHostOutcome::Suspend(suspend);
            }
            if self.config.has_custom_security_check_read {
                let suspend = WasmSuspendRequest::CheckReadResource {
                    token: token.clone(),
                    uri,
                };
                self.pending = Some(PendingContinuation::GlobHasElement {
                    token,
                    request,
                    security_checked: true,
                });
                return WasmHostOutcome::Suspend(suspend);
            }
            if let Err(err) = self.check_policy_resolve_resource(&uri) {
                return WasmHostOutcome::CompleteGlob(GlobCompletion::HasElement(Err(err.into())));
            }
        }

        if let Some((idx, _)) = self
            .custom_resource_reader_by_handle(&reader.handle)
            .or_else(|| self.find_custom_resource_reader(reader.scheme.as_str().unwrap_or("")))
        {
            let suspend = WasmSuspendRequest::ResourceHasElement {
                token: token.clone(),
                backend: SuspendBackend::Custom(idx),
                uri,
            };
            self.pending = Some(PendingContinuation::GlobHasElement {
                token,
                request,
                security_checked: true,
            });
            return WasmHostOutcome::Suspend(suspend);
        }

        if uri_scheme_eq(&uri, "projectpackage") {
            return WasmHostOutcome::CapabilityUnavailable {
                capability: "projectResolution",
                message: format!(
                    "Project package URI `{uri}` requires `PklProject` resolution, which is unavailable in Wasm."
                ),
            };
        }

        if uri_scheme_eq(&uri, "package") {
            let asset_uri = match PackageAssetUri::parse(uri) {
                Ok(u) => u,
                Err(e) => {
                    return WasmHostOutcome::CompleteGlob(GlobCompletion::HasElement(Err(
                        map_package_uri_error(e).into(),
                    )));
                }
            };
            return self.continue_package_glob_has_element(token, request, asset_uri);
        }

        if uri_scheme_eq(&uri, "file") {
            if !self.config.has_vfs {
                return WasmHostOutcome::CapabilityUnavailable {
                    capability: "nativeFs",
                    message: format!(
                        "Cannot probe `{uri}`: native filesystem access is unavailable in Wasm (attach a `VirtualFileSystem` via `options.vfs`)."
                    ),
                };
            }
            let suspend = WasmSuspendRequest::ResourceHasElement {
                token: token.clone(),
                backend: SuspendBackend::Vfs,
                uri,
            };
            self.pending = Some(PendingContinuation::GlobHasElement {
                token,
                request,
                security_checked: true,
            });
            return WasmHostOutcome::Suspend(suspend);
        }

        if uri_scheme_eq(&uri, "modulepath") || uri_scheme_eq(&uri, "jar") {
            return WasmHostOutcome::CapabilityUnavailable {
                capability: "modulePathJar",
                message: format!(
                    "Cannot probe `{uri}`: `modulepath:` / JAR archives are unavailable in Wasm."
                ),
            };
        }

        WasmHostOutcome::CompleteGlob(unsupported_glob_completion(&request))
    }

    fn continue_package_glob_has_element(
        &mut self,
        token: String,
        request: GlobRequest,
        asset_uri: PackageAssetUri,
    ) -> WasmHostOutcome {
        let meta = match self
            .packages
            .get_metadata(&asset_uri.package, self.config.test_mode)
        {
            Ok(Some(m)) => m,
            Ok(None) => {
                if !self.has_package_transport() {
                    return WasmHostOutcome::CapabilityUnavailable {
                        capability: "nativeHttp",
                        message: format!(
                            "Package fetch for `{}` requires `packageTransport` or `httpFetch` in Wasm.",
                            asset_uri.package.uri
                        ),
                    };
                }
                let request_url = get_metadata_request_uri(&asset_uri.package);
                if let Err(err) = validate_metadata_url(request_url.as_str().unwrap_or("")) {
                    self.record_module_err(&err);
                    return WasmHostOutcome::CompleteGlob(GlobCompletion::HasElement(Err(
                        err.into()
                    )));
                }
                let suspend = WasmSuspendRequest::PackageMetadata {
                    token: token.clone(),
                    package_uri: asset_uri.package.uri.clone(),
                    request_url: request_url.clone(),
                };
                return match self.suspend_package_http(
                    &request_url,
                    suspend,
                    PendingContinuation::GlobHasElement {
                        token,
                        request,
                        security_checked: true,
                    },
                ) {
                    Ok(outcome) => outcome,
                    Err(err) => {
                        WasmHostOutcome::CompleteGlob(GlobCompletion::HasElement(Err(err.into())))
                    }
                };
            }
            Err(err) => {
                self.record_module_err(&err);
                return WasmHostOutcome::CompleteGlob(GlobCompletion::HasElement(Err(err.into())));
            }
        };

        let Some(archive) = self.packages.get_archive(&asset_uri.package) else {
            if !self.has_package_transport() {
                return WasmHostOutcome::CapabilityUnavailable {
                    capability: "nativeHttp",
                    message: format!(
                        "Package archive fetch for `{}` requires `packageTransport` or `httpFetch` in Wasm.",
                        asset_uri.package.uri
                    ),
                };
            }
            let zip_url = meta.package_zip_url.clone();
            let suspend = WasmSuspendRequest::PackageArchive {
                token: token.clone(),
                package_uri: asset_uri.package.uri.clone(),
                zip_url: zip_url.clone(),
            };
            return match self.suspend_package_http(
                &zip_url,
                suspend,
                PendingContinuation::GlobHasElement {
                    token,
                    request,
                    security_checked: true,
                },
            ) {
                Ok(outcome) => outcome,
                Err(err) => {
                    WasmHostOutcome::CompleteGlob(GlobCompletion::HasElement(Err(err.into())))
                }
            };
        };

        let exists = archive.has_element(asset_uri.asset_path.as_str().unwrap_or(""));
        WasmHostOutcome::CompleteGlob(GlobCompletion::HasElement(Ok(exists)))
    }

    fn continue_glob_list_elements(
        &mut self,
        token: String,
        request: GlobRequest,
        security_checked: bool,
    ) -> WasmHostOutcome {
        let GlobRequestKind::ListElements { reader, uri } = &request.kind else {
            return WasmHostOutcome::CompleteGlob(unsupported_glob_completion(&request));
        };
        let is_resource = matches!(reader, GlobReader::Resource(_));
        let uri = uri.clone();

        if !security_checked {
            if is_resource && self.config.has_custom_security_check_resolve_resource {
                let suspend = WasmSuspendRequest::CheckResolveResource {
                    token: token.clone(),
                    uri,
                };
                self.pending = Some(PendingContinuation::GlobListElements {
                    token,
                    request,
                    security_checked: true,
                });
                return WasmHostOutcome::Suspend(suspend);
            }
            if is_resource && self.config.has_custom_security_check_read {
                let suspend = WasmSuspendRequest::CheckReadResource {
                    token: token.clone(),
                    uri,
                };
                self.pending = Some(PendingContinuation::GlobListElements {
                    token,
                    request,
                    security_checked: true,
                });
                return WasmHostOutcome::Suspend(suspend);
            }
            if !is_resource && self.config.has_custom_security_check_resolve {
                let suspend = WasmSuspendRequest::CheckResolveModule {
                    token: token.clone(),
                    uri,
                };
                self.pending = Some(PendingContinuation::GlobListElements {
                    token,
                    request,
                    security_checked: true,
                });
                return WasmHostOutcome::Suspend(suspend);
            }
            let check = if is_resource {
                self.check_policy_resolve_resource(&uri)
            } else {
                self.check_policy_resolve_module(&uri)
            };
            if let Err(err) = check {
                return WasmHostOutcome::CompleteGlob(GlobCompletion::Elements(Err(err.into())));
            }
        }

        // Custom module / resource readers
        match reader {
            GlobReader::Module(key) => {
                if let Some((idx, _)) = self
                    .custom_module_reader_by_handle(&key.handle)
                    .or_else(|| self.find_custom_module_reader(&uri))
                {
                    let suspend = WasmSuspendRequest::ListElements {
                        token: token.clone(),
                        is_resource: false,
                        backend: SuspendBackend::Custom(idx),
                        uri,
                    };
                    self.pending = Some(PendingContinuation::GlobListElements {
                        token,
                        request,
                        security_checked: true,
                    });
                    return WasmHostOutcome::Suspend(suspend);
                }
            }
            GlobReader::Resource(res) => {
                if let Some((idx, _)) = self
                    .custom_resource_reader_by_handle(&res.handle)
                    .or_else(|| self.find_custom_resource_reader(res.scheme.as_str().unwrap_or("")))
                {
                    let suspend = WasmSuspendRequest::ListElements {
                        token: token.clone(),
                        is_resource: true,
                        backend: SuspendBackend::Custom(idx),
                        uri,
                    };
                    self.pending = Some(PendingContinuation::GlobListElements {
                        token,
                        request,
                        security_checked: true,
                    });
                    return WasmHostOutcome::Suspend(suspend);
                }
            }
        }

        // `projectpackage:` is unavailable in Wasm
        if uri_scheme_eq(&uri, "projectpackage") {
            return WasmHostOutcome::CapabilityUnavailable {
                capability: "projectResolution",
                message: format!(
                    "Project package URI `{uri}` requires `PklProject` resolution, which is unavailable in Wasm."
                ),
            };
        }

        // `package:`
        if uri_scheme_eq(&uri, "package") || is_package_glob_reader(reader) {
            let asset_uri = match PackageAssetUri::parse(uri) {
                Ok(u) => u,
                Err(e) => {
                    return WasmHostOutcome::CompleteGlob(GlobCompletion::Elements(Err(
                        map_package_uri_error(e).into(),
                    )));
                }
            };
            return self.continue_package_glob_list_elements(token, request, asset_uri);
        }

        // Builtin `env:` / `prop:`
        let allow_all = SecurityPolicy::new(Some(&[".*".into()]), Some(&[".*".into()]), None)
            .expect("valid wildcard policy");
        let effective_policy = if self.config.has_custom_security_check_read
            || self.config.has_custom_security_check_resolve_resource
        {
            &allow_all
        } else {
            &self.policy
        };
        if let Some(comp) = configured_completion_with_policy(
            &request,
            &self.properties,
            &self.environment,
            effective_policy,
        ) {
            return WasmHostOutcome::CompleteGlob(comp);
        }

        // `file:` via VFS
        if uri_scheme_eq(&uri, "file") {
            if !self.config.has_vfs {
                return WasmHostOutcome::CapabilityUnavailable {
                    capability: "nativeFs",
                    message: format!(
                        "Cannot list `{uri}`: native filesystem access is unavailable in Wasm (attach a `VirtualFileSystem` via `options.vfs`)."
                    ),
                };
            }
            let suspend = WasmSuspendRequest::ListElements {
                token: token.clone(),
                is_resource,
                backend: SuspendBackend::Vfs,
                uri,
            };
            self.pending = Some(PendingContinuation::GlobListElements {
                token,
                request,
                security_checked: true,
            });
            return WasmHostOutcome::Suspend(suspend);
        }

        WasmHostOutcome::CompleteGlob(unsupported_glob_completion(&request))
    }

    fn continue_package_glob_list_elements(
        &mut self,
        token: String,
        request: GlobRequest,
        asset_uri: PackageAssetUri,
    ) -> WasmHostOutcome {
        let meta = match self
            .packages
            .get_metadata(&asset_uri.package, self.config.test_mode)
        {
            Ok(Some(m)) => m,
            Ok(None) => {
                if !self.has_package_transport() {
                    return WasmHostOutcome::CapabilityUnavailable {
                        capability: "nativeHttp",
                        message: format!(
                            "Package fetch for `{}` requires `packageTransport` or `httpFetch` in Wasm.",
                            asset_uri.package.uri
                        ),
                    };
                }
                let request_url = get_metadata_request_uri(&asset_uri.package);
                if let Err(err) = validate_metadata_url(request_url.as_str().unwrap_or("")) {
                    self.record_module_err(&err);
                    return WasmHostOutcome::CompleteGlob(GlobCompletion::Elements(
                        Err(err.into()),
                    ));
                }
                let suspend = WasmSuspendRequest::PackageMetadata {
                    token: token.clone(),
                    package_uri: asset_uri.package.uri.clone(),
                    request_url: request_url.clone(),
                };
                return match self.suspend_package_http(
                    &request_url,
                    suspend,
                    PendingContinuation::GlobListElements {
                        token,
                        request,
                        security_checked: true,
                    },
                ) {
                    Ok(outcome) => outcome,
                    Err(err) => {
                        WasmHostOutcome::CompleteGlob(GlobCompletion::Elements(Err(err.into())))
                    }
                };
            }
            Err(err) => {
                self.record_module_err(&err);
                return WasmHostOutcome::CompleteGlob(GlobCompletion::Elements(Err(err.into())));
            }
        };

        let Some(archive) = self.packages.get_archive(&asset_uri.package) else {
            if !self.has_package_transport() {
                return WasmHostOutcome::CapabilityUnavailable {
                    capability: "nativeHttp",
                    message: format!(
                        "Package archive fetch for `{}` requires `packageTransport` or `httpFetch` in Wasm.",
                        asset_uri.package.uri
                    ),
                };
            }
            let zip_url = meta.package_zip_url.clone();
            let suspend = WasmSuspendRequest::PackageArchive {
                token: token.clone(),
                package_uri: asset_uri.package.uri.clone(),
                zip_url: zip_url.clone(),
            };
            return match self.suspend_package_http(
                &zip_url,
                suspend,
                PendingContinuation::GlobListElements {
                    token,
                    request,
                    security_checked: true,
                },
            ) {
                Ok(outcome) => outcome,
                Err(err) => {
                    WasmHostOutcome::CompleteGlob(GlobCompletion::Elements(Err(err.into())))
                }
            };
        };

        let elements = archive
            .list_elements(asset_uri.asset_path.as_str().unwrap_or(""))
            .into_iter()
            .map(|(name, is_directory)| PathElement {
                name: name.into(),
                is_directory,
            })
            .collect();
        WasmHostOutcome::CompleteGlob(GlobCompletion::Elements(Ok(elements)))
    }

    // -----------------------------------------------------------------------
    // Logging and frame transformation
    // -----------------------------------------------------------------------

    pub fn handle_log(
        &mut self,
        token: String,
        level: LogLevel,
        message: PklString,
        frame: LogFrame,
        force_suspend: bool,
    ) -> WasmHostOutcome {
        if self.config.has_custom_logger || force_suspend {
            let suspend = WasmSuspendRequest::Log {
                token: token.clone(),
                level,
                message,
                frame,
            };
            self.pending = Some(PendingContinuation::Log { token });
            return WasmHostOutcome::Suspend(suspend);
        }
        WasmHostOutcome::AckLog
    }

    pub fn handle_transform_frame(&mut self, token: String, frame: LogFrame) -> WasmHostOutcome {
        let base_frame = if self.config.test_mode {
            frame
        } else {
            FrameTransformer::Default.transform(frame)
        };
        if self.config.has_custom_frame_transformer {
            let suspend = WasmSuspendRequest::TransformFrame {
                token: token.clone(),
                frame: base_frame,
            };
            self.pending = Some(PendingContinuation::TransformFrame { token });
            return WasmHostOutcome::Suspend(suspend);
        }
        WasmHostOutcome::CompleteFrame(base_frame)
    }

    // -----------------------------------------------------------------------
    // Resuming pending multi-step continuations from JS responses
    // -----------------------------------------------------------------------

    fn take_pending_for(&mut self, token: &str) -> Result<PendingContinuation, StructuredError> {
        let Some(pending) = self.pending.take() else {
            return Err(StructuredError::invalid_argument(
                "no pending Wasm host operation",
            ));
        };
        if pending.token() != token {
            self.pending = Some(pending);
            return Err(StructuredError::invalid_argument(
                "stale or mismatched Wasm host operation token",
            ));
        }
        Ok(pending)
    }

    pub fn complete_security_check(
        &mut self,
        token: &str,
        error_message: Option<PklString>,
    ) -> Result<WasmHostOutcome, StructuredError> {
        let pending = self.take_pending_for(token)?;
        match pending {
            PendingContinuation::ModuleCheckImport { .. } => {
                if let Some(msg) = error_message {
                    self.saw_security_denial = true;
                    return Ok(WasmHostOutcome::CompleteModule(
                        ModuleCompletion::ImportChecked(Err(
                            ModuleHostError::SecurityManagerException { message: Some(msg) },
                        )),
                    ));
                }
                Ok(WasmHostOutcome::CompleteModule(
                    ModuleCompletion::ImportChecked(Ok(())),
                ))
            }
            PendingContinuation::ModuleResolve { token, request, .. } => {
                if let Some(msg) = error_message {
                    return Ok(self.finish_module_resolve_err(
                        &request,
                        ModuleHostError::SecurityManagerException { message: Some(msg) },
                    ));
                }
                Ok(self.continue_module_resolve(token, request, true))
            }
            PendingContinuation::ModuleHasElement { token, request, .. } => {
                if let Some(msg) = error_message {
                    self.saw_security_denial = true;
                    return Ok(WasmHostOutcome::CompleteModule(
                        ModuleCompletion::HasElement(Err(
                            ModuleHostError::SecurityManagerException { message: Some(msg) },
                        )),
                    ));
                }
                Ok(self.continue_module_has_element(token, request, true))
            }
            PendingContinuation::ReadResource {
                token,
                resolved_uri,
                ..
            } => {
                if let Some(msg) = error_message {
                    self.saw_security_denial = true;
                    return Ok(WasmHostOutcome::CompleteRead(Err(
                        ReadFailure::SecurityManagerException { message: Some(msg) },
                    )));
                }
                Ok(self.continue_read_resource(token, resolved_uri, true))
            }
            PendingContinuation::GlobHasElement { token, request, .. } => {
                if let Some(msg) = error_message {
                    self.saw_security_denial = true;
                    return Ok(WasmHostOutcome::CompleteGlob(GlobCompletion::HasElement(
                        Err(
                            ModuleHostError::SecurityManagerException { message: Some(msg) }.into(),
                        ),
                    )));
                }
                Ok(self.continue_glob_has_element(token, request, true))
            }
            PendingContinuation::GlobListElements { token, request, .. } => {
                if let Some(msg) = error_message {
                    self.saw_security_denial = true;
                    return Ok(WasmHostOutcome::CompleteGlob(GlobCompletion::Elements(
                        Err(
                            ModuleHostError::SecurityManagerException { message: Some(msg) }.into(),
                        ),
                    )));
                }
                Ok(self.continue_glob_list_elements(token, request, true))
            }
            PendingContinuation::PackageUrlCheck {
                next_suspend,
                after_fetch,
            } => {
                if let Some(msg) = error_message {
                    self.saw_security_denial = true;
                    let mod_err = ModuleHostError::SecurityManagerException {
                        message: Some(msg.clone()),
                    };
                    return Ok(match *after_fetch {
                        PendingContinuation::ModuleResolveDepImport { .. } => {
                            WasmHostOutcome::CompleteModule(ModuleCompletion::ImportUri(Err(
                                mod_err,
                            )))
                        }
                        PendingContinuation::ModuleResolve { request, .. } => {
                            self.finish_module_resolve_err(&request, mod_err)
                        }
                        PendingContinuation::ModuleHasElement { .. } => {
                            WasmHostOutcome::CompleteModule(ModuleCompletion::HasElement(Err(
                                mod_err,
                            )))
                        }
                        PendingContinuation::ReadResource { .. } => WasmHostOutcome::CompleteRead(
                            Err(ReadFailure::SecurityManagerException { message: Some(msg) }),
                        ),
                        PendingContinuation::GlobHasElement { .. } => {
                            WasmHostOutcome::CompleteGlob(GlobCompletion::HasElement(Err(
                                mod_err.into()
                            )))
                        }
                        PendingContinuation::GlobListElements { .. } => {
                            WasmHostOutcome::CompleteGlob(GlobCompletion::Elements(Err(
                                mod_err.into()
                            )))
                        }
                        _ => WasmHostOutcome::CompleteRead(Err(
                            ReadFailure::SecurityManagerException { message: Some(msg) },
                        )),
                    });
                }
                self.pending = Some(*after_fetch);
                Ok(WasmHostOutcome::Suspend(*next_suspend))
            }
            other => {
                self.pending = Some(other);
                Err(StructuredError::invalid_argument(
                    "pending operation is not a security check",
                ))
            }
        }
    }

    pub fn complete_read_module(
        &mut self,
        token: &str,
        result: Result<(PklString, SourceText), ModuleHostError>,
    ) -> Result<WasmHostOutcome, StructuredError> {
        let pending = self.take_pending_for(token)?;
        let PendingContinuation::ModuleResolve { request, .. } = pending else {
            self.pending = Some(pending);
            return Err(StructuredError::invalid_argument(
                "pending operation is not a module read",
            ));
        };
        let key = match &request.kind {
            ModuleRequestKind::Resolve { key, .. } | ModuleRequestKind::ResolveKey { key, .. } => {
                key.clone()
            }
            _ => {
                return Err(StructuredError::invalid_argument(
                    "pending module request is not Resolve/ResolveKey",
                ));
            }
        };
        match result {
            Ok((resolved_uri, text)) => {
                let inferred_name = infer_name_for_uri(&key.uri.to_java_utf8());
                let resolved = ResolvedModule {
                    original_uri: key.uri.clone(),
                    resolved_uri,
                    source_uri: key.uri,
                };
                let source = ModuleSource {
                    text,
                    inferred_name,
                };
                Ok(self.finish_module_resolve_ok(&request, resolved, Some(source)))
            }
            Err(err) => Ok(self.finish_module_resolve_err(&request, err)),
        }
    }

    pub fn complete_module_has_element(
        &mut self,
        token: &str,
        result: Result<bool, ModuleHostError>,
    ) -> Result<WasmHostOutcome, StructuredError> {
        let pending = self.take_pending_for(token)?;
        let PendingContinuation::ModuleHasElement { .. } = pending else {
            self.pending = Some(pending);
            return Err(StructuredError::invalid_argument(
                "pending operation is not moduleHasElement",
            ));
        };
        if let Err(ref err) = result {
            self.record_module_err(err);
        }
        Ok(WasmHostOutcome::CompleteModule(
            ModuleCompletion::HasElement(result),
        ))
    }

    pub fn complete_read_resource(
        &mut self,
        token: &str,
        result: Result<Option<Vec<u8>>, ReadFailure>,
    ) -> Result<WasmHostOutcome, StructuredError> {
        let pending = self.take_pending_for(token)?;
        let PendingContinuation::ReadResource { resolved_uri, .. } = pending else {
            self.pending = Some(pending);
            return Err(StructuredError::invalid_argument(
                "pending operation is not readResource",
            ));
        };
        match result {
            Ok(Some(bytes)) => Ok(WasmHostOutcome::CompleteRead(Ok(ReadResult::Resource(
                Resource {
                    uri: resolved_uri,
                    bytes: bytes.into(),
                },
            )))),
            Ok(None) => Ok(WasmHostOutcome::CompleteRead(Ok(ReadResult::Missing))),
            Err(err) => {
                self.record_read_err(&err);
                Ok(WasmHostOutcome::CompleteRead(Err(err)))
            }
        }
    }

    pub fn complete_glob_has_element(
        &mut self,
        token: &str,
        result: Result<bool, ModuleHostError>,
    ) -> Result<WasmHostOutcome, StructuredError> {
        let pending = self.take_pending_for(token)?;
        let PendingContinuation::GlobHasElement { .. } = pending else {
            self.pending = Some(pending);
            return Err(StructuredError::invalid_argument(
                "pending operation is not resourceHasElement",
            ));
        };
        if let Err(ref err) = result {
            self.record_module_err(err);
        }
        Ok(WasmHostOutcome::CompleteGlob(GlobCompletion::HasElement(
            result.map_err(Into::into),
        )))
    }

    pub fn complete_list_elements(
        &mut self,
        token: &str,
        result: Result<Vec<PathElement>, ModuleHostError>,
    ) -> Result<WasmHostOutcome, StructuredError> {
        let pending = self.take_pending_for(token)?;
        let PendingContinuation::GlobListElements { .. } = pending else {
            self.pending = Some(pending);
            return Err(StructuredError::invalid_argument(
                "pending operation is not listElements",
            ));
        };
        if let Err(ref err) = result {
            self.record_module_err(err);
        }
        Ok(WasmHostOutcome::CompleteGlob(GlobCompletion::Elements(
            result.map_err(Into::into),
        )))
    }

    pub fn complete_package_metadata(
        &mut self,
        token: &str,
        result: Result<Vec<u8>, ModuleHostError>,
    ) -> Result<WasmHostOutcome, StructuredError> {
        let pending = self.take_pending_for(token)?;
        match pending {
            PendingContinuation::ModuleResolveDepImport {
                token,
                asset_uri,
                dep_name,
                encoded_asset,
            } => {
                let bytes = match result {
                    Ok(b) => b,
                    Err(err) => {
                        self.record_module_err(&err);
                        return Ok(WasmHostOutcome::CompleteModule(
                            ModuleCompletion::ImportUri(Err(err)),
                        ));
                    }
                };
                let request_url = get_metadata_request_uri(&asset_uri.package);
                if let Err(err) = self.packages.accept_and_store_metadata(
                    &asset_uri.package,
                    &request_url,
                    &bytes,
                    self.config.test_mode,
                ) {
                    self.record_module_err(&err);
                    return Ok(WasmHostOutcome::CompleteModule(
                        ModuleCompletion::ImportUri(Err(err)),
                    ));
                }
                Ok(self.resolve_package_dep_import(token, asset_uri, dep_name, encoded_asset))
            }
            PendingContinuation::ModuleResolve { token, request, .. } => {
                let uri = match &request.kind {
                    ModuleRequestKind::Resolve { key, .. }
                    | ModuleRequestKind::ResolveKey { key, .. } => key.uri.clone(),
                    _ => unreachable!(),
                };
                let asset_uri = PackageAssetUri::parse(uri)
                    .map_err(|e| StructuredError::evaluation(e.reason()))?;
                let bytes = match result {
                    Ok(b) => b,
                    Err(err) => return Ok(self.finish_module_resolve_err(&request, err)),
                };
                let request_url = get_metadata_request_uri(&asset_uri.package);
                if let Err(err) = self.packages.accept_and_store_metadata(
                    &asset_uri.package,
                    &request_url,
                    &bytes,
                    self.config.test_mode,
                ) {
                    return Ok(self.finish_module_resolve_err(&request, err));
                }
                Ok(self.continue_package_module_resolve(token, request, asset_uri))
            }
            PendingContinuation::ModuleHasElement { token, request, .. } => {
                let ModuleRequestKind::HasElement { element_uri, .. } = &request.kind else {
                    unreachable!()
                };
                let asset_uri = PackageAssetUri::parse(element_uri.clone())
                    .map_err(|e| StructuredError::evaluation(e.reason()))?;
                let bytes = match result {
                    Ok(b) => b,
                    Err(err) => {
                        self.record_module_err(&err);
                        return Ok(WasmHostOutcome::CompleteModule(
                            ModuleCompletion::HasElement(Err(err)),
                        ));
                    }
                };
                let request_url = get_metadata_request_uri(&asset_uri.package);
                if let Err(err) = self.packages.accept_and_store_metadata(
                    &asset_uri.package,
                    &request_url,
                    &bytes,
                    self.config.test_mode,
                ) {
                    self.record_module_err(&err);
                    return Ok(WasmHostOutcome::CompleteModule(
                        ModuleCompletion::HasElement(Err(err)),
                    ));
                }
                Ok(self.continue_package_has_element_module(token, request, asset_uri))
            }
            PendingContinuation::ReadResource {
                token,
                resolved_uri,
                ..
            } => {
                let asset_uri = PackageAssetUri::parse(resolved_uri.clone())
                    .map_err(|e| StructuredError::evaluation(e.reason()))?;
                let bytes = match result {
                    Ok(b) => b,
                    Err(err) => {
                        self.record_module_err(&err);
                        return Ok(WasmHostOutcome::CompleteRead(Err(
                            module_err_to_read_failure(err),
                        )));
                    }
                };
                let request_url = get_metadata_request_uri(&asset_uri.package);
                if let Err(err) = self.packages.accept_and_store_metadata(
                    &asset_uri.package,
                    &request_url,
                    &bytes,
                    self.config.test_mode,
                ) {
                    self.record_module_err(&err);
                    return Ok(WasmHostOutcome::CompleteRead(Err(
                        module_err_to_read_failure(err),
                    )));
                }
                Ok(self.continue_package_read_resource(token, resolved_uri, asset_uri))
            }
            PendingContinuation::GlobHasElement { token, request, .. } => {
                let GlobRequestKind::HasElement { uri, .. } = &request.kind else {
                    unreachable!()
                };
                let asset_uri = PackageAssetUri::parse(uri.clone())
                    .map_err(|e| StructuredError::evaluation(e.reason()))?;
                let bytes = match result {
                    Ok(b) => b,
                    Err(err) => {
                        self.record_module_err(&err);
                        return Ok(WasmHostOutcome::CompleteGlob(GlobCompletion::HasElement(
                            Err(err.into()),
                        )));
                    }
                };
                let request_url = get_metadata_request_uri(&asset_uri.package);
                if let Err(err) = self.packages.accept_and_store_metadata(
                    &asset_uri.package,
                    &request_url,
                    &bytes,
                    self.config.test_mode,
                ) {
                    self.record_module_err(&err);
                    return Ok(WasmHostOutcome::CompleteGlob(GlobCompletion::HasElement(
                        Err(err.into()),
                    )));
                }
                Ok(self.continue_package_glob_has_element(token, request, asset_uri))
            }
            PendingContinuation::GlobListElements { token, request, .. } => {
                let GlobRequestKind::ListElements { uri, .. } = &request.kind else {
                    unreachable!()
                };
                let asset_uri = PackageAssetUri::parse(uri.clone())
                    .map_err(|e| StructuredError::evaluation(e.reason()))?;
                let bytes = match result {
                    Ok(b) => b,
                    Err(err) => {
                        self.record_module_err(&err);
                        return Ok(WasmHostOutcome::CompleteGlob(GlobCompletion::Elements(
                            Err(err.into()),
                        )));
                    }
                };
                let request_url = get_metadata_request_uri(&asset_uri.package);
                if let Err(err) = self.packages.accept_and_store_metadata(
                    &asset_uri.package,
                    &request_url,
                    &bytes,
                    self.config.test_mode,
                ) {
                    self.record_module_err(&err);
                    return Ok(WasmHostOutcome::CompleteGlob(GlobCompletion::Elements(
                        Err(err.into()),
                    )));
                }
                Ok(self.continue_package_glob_list_elements(token, request, asset_uri))
            }
            other => {
                self.pending = Some(other);
                Err(StructuredError::invalid_argument(
                    "pending operation is not a packageMetadata fetch",
                ))
            }
        }
    }

    pub fn complete_package_archive(
        &mut self,
        token: &str,
        result: Result<Vec<u8>, ModuleHostError>,
    ) -> Result<WasmHostOutcome, StructuredError> {
        let pending = self.take_pending_for(token)?;
        match pending {
            PendingContinuation::ModuleResolve { token, request, .. } => {
                let uri = match &request.kind {
                    ModuleRequestKind::Resolve { key, .. }
                    | ModuleRequestKind::ResolveKey { key, .. } => key.uri.clone(),
                    _ => unreachable!(),
                };
                let asset_uri = PackageAssetUri::parse(uri)
                    .map_err(|e| StructuredError::evaluation(e.reason()))?;
                let zip_bytes = match result {
                    Ok(b) => b,
                    Err(err) => return Ok(self.finish_module_resolve_err(&request, err)),
                };
                let meta = match self
                    .packages
                    .get_metadata(&asset_uri.package, self.config.test_mode)
                {
                    Ok(Some(m)) => m,
                    Ok(None) => {
                        return Ok(self.finish_module_resolve_err(
                            &request,
                            ModuleHostError::Io("package metadata missing from cache".into()),
                        ));
                    }
                    Err(err) => return Ok(self.finish_module_resolve_err(&request, err)),
                };
                if let Err(err) =
                    self.packages
                        .verify_and_store_archive(&asset_uri.package, &meta, &zip_bytes)
                {
                    return Ok(self.finish_module_resolve_err(&request, err));
                }
                Ok(self.continue_package_module_resolve(token, request, asset_uri))
            }
            PendingContinuation::ModuleHasElement { token, request, .. } => {
                let ModuleRequestKind::HasElement { element_uri, .. } = &request.kind else {
                    unreachable!()
                };
                let asset_uri = PackageAssetUri::parse(element_uri.clone())
                    .map_err(|e| StructuredError::evaluation(e.reason()))?;
                let zip_bytes = match result {
                    Ok(b) => b,
                    Err(err) => {
                        self.record_module_err(&err);
                        return Ok(WasmHostOutcome::CompleteModule(
                            ModuleCompletion::HasElement(Err(err)),
                        ));
                    }
                };
                let meta = match self
                    .packages
                    .get_metadata(&asset_uri.package, self.config.test_mode)
                {
                    Ok(Some(m)) => m,
                    Ok(None) => {
                        return Ok(WasmHostOutcome::CompleteModule(
                            ModuleCompletion::HasElement(Err(ModuleHostError::Io(
                                "package metadata missing from cache".into(),
                            ))),
                        ));
                    }
                    Err(err) => {
                        self.record_module_err(&err);
                        return Ok(WasmHostOutcome::CompleteModule(
                            ModuleCompletion::HasElement(Err(err)),
                        ));
                    }
                };
                if let Err(err) =
                    self.packages
                        .verify_and_store_archive(&asset_uri.package, &meta, &zip_bytes)
                {
                    self.record_module_err(&err);
                    return Ok(WasmHostOutcome::CompleteModule(
                        ModuleCompletion::HasElement(Err(err)),
                    ));
                }
                Ok(self.continue_package_has_element_module(token, request, asset_uri))
            }
            PendingContinuation::ReadResource {
                token,
                resolved_uri,
                ..
            } => {
                let asset_uri = PackageAssetUri::parse(resolved_uri.clone())
                    .map_err(|e| StructuredError::evaluation(e.reason()))?;
                let zip_bytes = match result {
                    Ok(b) => b,
                    Err(err) => {
                        self.record_module_err(&err);
                        return Ok(WasmHostOutcome::CompleteRead(Err(
                            module_err_to_read_failure(err),
                        )));
                    }
                };
                let meta = match self
                    .packages
                    .get_metadata(&asset_uri.package, self.config.test_mode)
                {
                    Ok(Some(m)) => m,
                    Ok(None) => {
                        return Ok(WasmHostOutcome::CompleteRead(Err(ReadFailure::Io(
                            "package metadata missing from cache".into(),
                        ))));
                    }
                    Err(err) => {
                        self.record_module_err(&err);
                        return Ok(WasmHostOutcome::CompleteRead(Err(
                            module_err_to_read_failure(err),
                        )));
                    }
                };
                if let Err(err) =
                    self.packages
                        .verify_and_store_archive(&asset_uri.package, &meta, &zip_bytes)
                {
                    self.record_module_err(&err);
                    return Ok(WasmHostOutcome::CompleteRead(Err(
                        module_err_to_read_failure(err),
                    )));
                }
                Ok(self.continue_package_read_resource(token, resolved_uri, asset_uri))
            }
            PendingContinuation::GlobHasElement { token, request, .. } => {
                let GlobRequestKind::HasElement { uri, .. } = &request.kind else {
                    unreachable!()
                };
                let asset_uri = PackageAssetUri::parse(uri.clone())
                    .map_err(|e| StructuredError::evaluation(e.reason()))?;
                let zip_bytes = match result {
                    Ok(b) => b,
                    Err(err) => {
                        self.record_module_err(&err);
                        return Ok(WasmHostOutcome::CompleteGlob(GlobCompletion::HasElement(
                            Err(err.into()),
                        )));
                    }
                };
                let meta = match self
                    .packages
                    .get_metadata(&asset_uri.package, self.config.test_mode)
                {
                    Ok(Some(m)) => m,
                    Ok(None) => {
                        return Ok(WasmHostOutcome::CompleteGlob(GlobCompletion::HasElement(
                            Err(
                                ModuleHostError::Io("package metadata missing from cache".into())
                                    .into(),
                            ),
                        )));
                    }
                    Err(err) => {
                        self.record_module_err(&err);
                        return Ok(WasmHostOutcome::CompleteGlob(GlobCompletion::HasElement(
                            Err(err.into()),
                        )));
                    }
                };
                if let Err(err) =
                    self.packages
                        .verify_and_store_archive(&asset_uri.package, &meta, &zip_bytes)
                {
                    self.record_module_err(&err);
                    return Ok(WasmHostOutcome::CompleteGlob(GlobCompletion::HasElement(
                        Err(err.into()),
                    )));
                }
                Ok(self.continue_package_glob_has_element(token, request, asset_uri))
            }
            PendingContinuation::GlobListElements { token, request, .. } => {
                let GlobRequestKind::ListElements { uri, .. } = &request.kind else {
                    unreachable!()
                };
                let asset_uri = PackageAssetUri::parse(uri.clone())
                    .map_err(|e| StructuredError::evaluation(e.reason()))?;
                let zip_bytes = match result {
                    Ok(b) => b,
                    Err(err) => {
                        self.record_module_err(&err);
                        return Ok(WasmHostOutcome::CompleteGlob(GlobCompletion::Elements(
                            Err(err.into()),
                        )));
                    }
                };
                let meta = match self
                    .packages
                    .get_metadata(&asset_uri.package, self.config.test_mode)
                {
                    Ok(Some(m)) => m,
                    Ok(None) => {
                        return Ok(WasmHostOutcome::CompleteGlob(GlobCompletion::Elements(
                            Err(
                                ModuleHostError::Io("package metadata missing from cache".into())
                                    .into(),
                            ),
                        )));
                    }
                    Err(err) => {
                        self.record_module_err(&err);
                        return Ok(WasmHostOutcome::CompleteGlob(GlobCompletion::Elements(
                            Err(err.into()),
                        )));
                    }
                };
                if let Err(err) =
                    self.packages
                        .verify_and_store_archive(&asset_uri.package, &meta, &zip_bytes)
                {
                    self.record_module_err(&err);
                    return Ok(WasmHostOutcome::CompleteGlob(GlobCompletion::Elements(
                        Err(err.into()),
                    )));
                }
                Ok(self.continue_package_glob_list_elements(token, request, asset_uri))
            }
            other => {
                self.pending = Some(other);
                Err(StructuredError::invalid_argument(
                    "pending operation is not a packageArchive fetch",
                ))
            }
        }
    }

    pub fn take_pending_log(&mut self, token: &str) -> Result<(), StructuredError> {
        let pending = self.take_pending_for(token)?;
        if matches!(pending, PendingContinuation::Log { .. }) {
            Ok(())
        } else {
            self.pending = Some(pending);
            Err(StructuredError::invalid_argument(
                "pending operation is not a log event",
            ))
        }
    }

    pub fn take_pending_frame_transform(&mut self, token: &str) -> Result<(), StructuredError> {
        let pending = self.take_pending_for(token)?;
        if matches!(pending, PendingContinuation::TransformFrame { .. }) {
            Ok(())
        } else {
            self.pending = Some(pending);
            Err(StructuredError::invalid_argument(
                "pending operation is not a transformFrame event",
            ))
        }
    }
}

/// Parses JS configuration object into `WasmHostConfig`.
pub fn parse_host_config_js(value: &JsValue) -> Result<WasmHostConfig, StructuredError> {
    if value.is_null() || value.is_undefined() {
        return Ok(WasmHostConfig::default());
    }
    if !value.is_object() {
        return Err(StructuredError::configuration(
            "evaluator options must be an object",
        ));
    }

    let opt_str_list = |key: &str| -> Result<Option<Vec<String>>, StructuredError> {
        let val = get_prop(value, key)?;
        if val.is_null() || val.is_undefined() {
            return Ok(None);
        }
        if !Array::is_array(&val) {
            return Err(StructuredError::configuration(format!(
                "`{key}` must be an array of strings"
            )));
        }
        let mut out = Vec::new();
        for item in Array::from(&val).iter() {
            out.push(rust_string_from_js(&item)?);
        }
        Ok(Some(out))
    };

    let opt_str = |key: &str| -> Result<Option<String>, StructuredError> {
        let val = get_prop(value, key)?;
        if val.is_null() || val.is_undefined() {
            return Ok(None);
        }
        rust_string_from_js(&val).map(Some)
    };

    let opt_bool = |key: &str, default: bool| -> Result<bool, StructuredError> {
        let val = get_prop(value, key)?;
        if val.is_null() || val.is_undefined() {
            return Ok(default);
        }
        val.as_bool()
            .ok_or_else(|| StructuredError::configuration(format!("`{key}` must be a boolean")))
    };

    let opt_pairs = |key: &str| -> Result<Vec<(String, String)>, StructuredError> {
        let val = get_prop(value, key)?;
        if val.is_null() || val.is_undefined() {
            return Ok(Vec::new());
        }
        if !Array::is_array(&val) {
            return Err(StructuredError::configuration(format!(
                "`{key}` must be an array of [key, value] pairs"
            )));
        }
        let mut out = Vec::new();
        for item in Array::from(&val).iter() {
            if !Array::is_array(&item) {
                return Err(StructuredError::configuration(format!(
                    "`{key}` entry must be a [key, value] pair"
                )));
            }
            let pair = Array::from(&item);
            if pair.length() != 2 {
                return Err(StructuredError::configuration(format!(
                    "`{key}` entry must have length 2"
                )));
            }
            out.push((
                rust_string_from_js(&pair.get(0))?,
                rust_string_from_js(&pair.get(1))?,
            ));
        }
        Ok(out)
    };

    let trace_mode = match opt_str("traceMode")?.as_deref() {
        None | Some("compact") => TraceMode::Compact,
        Some("pretty") => TraceMode::Pretty,
        Some(other) => {
            return Err(StructuredError::configuration(format!(
                "invalid traceMode `{other}` (expected \"compact\" or \"pretty\")"
            )));
        }
    };

    let platform_val = get_prop(value, "platform")?;
    let platform_metadata = if platform_val.is_null() || platform_val.is_undefined() {
        None
    } else if platform_val.is_object() {
        let mut meta = PlatformMetadata {
            os_name: "unknown".into(),
            os_version: "".into(),
            cpu_architecture: "wasm32".into(),
            java_version: "native".into(),
        };
        let os = get_prop(&platform_val, "os")?;
        if !os.is_null() && !os.is_undefined() {
            meta.os_name = rust_string_from_js(&os)?.into();
        }
        let os_version = get_prop(&platform_val, "osVersion")?;
        if !os_version.is_null() && !os_version.is_undefined() {
            meta.os_version = rust_string_from_js(&os_version)?.into();
        }
        let arch = get_prop(&platform_val, "arch")?;
        if !arch.is_null() && !arch.is_undefined() {
            meta.cpu_architecture = rust_string_from_js(&arch)?.into();
        }
        Some(meta)
    } else {
        return Err(StructuredError::configuration(
            "`platform` must be an object",
        ));
    };

    let mut custom_module_readers = Vec::new();
    let mr_val = get_prop(value, "moduleReaders")?;
    if !mr_val.is_null() && !mr_val.is_undefined() {
        if !Array::is_array(&mr_val) {
            return Err(StructuredError::configuration(
                "`moduleReaders` must be an array",
            ));
        }
        for item in Array::from(&mr_val).iter() {
            let scheme = rust_string_from_js(&get_prop(&item, "scheme")?)?
                .trim_end_matches(':')
                .to_string();
            let flag = |k: &str, default: bool| -> Result<bool, StructuredError> {
                let v = get_prop(&item, k)?;
                if v.is_null() || v.is_undefined() {
                    Ok(default)
                } else {
                    v.as_bool().ok_or_else(|| {
                        StructuredError::configuration(format!("ModuleReader.{k} must be boolean"))
                    })
                }
            };
            custom_module_readers.push(CustomModuleReaderMeta {
                scheme,
                is_local: flag("isLocal", false)?,
                is_globbable: flag("isGlobbable", false)?,
                has_hierarchical_uris: flag("hasHierarchicalUris", false)?,
                is_cached: flag("isCached", true)?,
                has_fragment_paths: flag("hasFragmentPaths", false)?,
            });
        }
    }

    let mut custom_resource_readers = Vec::new();
    let rr_val = get_prop(value, "resourceReaders")?;
    if !rr_val.is_null() && !rr_val.is_undefined() {
        if !Array::is_array(&rr_val) {
            return Err(StructuredError::configuration(
                "`resourceReaders` must be an array",
            ));
        }
        for item in Array::from(&rr_val).iter() {
            let scheme = rust_string_from_js(&get_prop(&item, "scheme")?)?
                .trim_end_matches(':')
                .to_string();
            let flag = |k: &str, default: bool| -> Result<bool, StructuredError> {
                let v = get_prop(&item, k)?;
                if v.is_null() || v.is_undefined() {
                    Ok(default)
                } else {
                    v.as_bool().ok_or_else(|| {
                        StructuredError::configuration(format!(
                            "ResourceReader.{k} must be boolean"
                        ))
                    })
                }
            };
            custom_resource_readers.push(CustomResourceReaderMeta {
                scheme,
                is_globbable: flag("isGlobbable", false)?,
                has_hierarchical_uris: flag("hasHierarchicalUris", false)?,
            });
        }
    }

    Ok(WasmHostConfig {
        allowed_modules: opt_str_list("allowedModules")?,
        allowed_resources: opt_str_list("allowedResources")?,
        root_dir: opt_str("rootDir")?,
        properties: opt_pairs("properties")?,
        environment: opt_pairs("env")?,
        output_format: opt_str("outputFormat")?,
        trace_mode,
        power_assertions: opt_bool("powerAssertions", false)?,
        use_color: opt_bool("color", false)?,
        test_mode: opt_bool("testMode", false)?,
        has_vfs: opt_bool("hasVfs", false)?,
        has_http_fetch: opt_bool("hasHttpFetch", false)?,
        has_package_transport: opt_bool("hasPackageTransport", false)?,
        has_custom_logger: opt_bool("hasCustomLogger", false)?,
        has_custom_frame_transformer: opt_bool("hasCustomFrameTransformer", false)?,
        has_custom_security_check_import: opt_bool("hasCustomSecurityCheckImport", false)?,
        has_custom_security_check_read: opt_bool("hasCustomSecurityCheckRead", false)?,
        has_custom_security_check_resolve: opt_bool("hasCustomSecurityCheckResolve", false)?,
        has_custom_security_check_resolve_resource: opt_bool(
            "hasCustomSecurityCheckResolveResource",
            false,
        )?,
        platform_metadata,
        custom_module_readers,
        custom_resource_readers,
    })
}

pub fn parse_module_host_error(kind: &str, message: Option<PklString>) -> ModuleHostError {
    match kind {
        "not-found" => ModuleHostError::NotFound,
        "no-loader" => ModuleHostError::NoLoader,
        "security" => ModuleHostError::SecurityManagerException { message },
        "http" => ModuleHostError::HttpClientException { message },
        "uri-syntax" => ModuleHostError::UriSyntax {
            reason: message.unwrap_or_else(|| "Invalid module URI syntax.".into()),
        },
        "eval" => ModuleHostError::EvalError {
            message: message.unwrap_or_else(|| "Module evaluation error.".into()),
            hint: None,
        },
        "unsupported" => {
            ModuleHostError::Unsupported(message.unwrap_or_else(|| "Unsupported operation.".into()))
        }
        _ => ModuleHostError::Io(IoFailure::new(message)),
    }
}

pub fn parse_read_failure(kind: &str, message: Option<PklString>) -> ReadFailure {
    match kind {
        "security" => ReadFailure::SecurityManagerException { message },
        "http" => ReadFailure::HttpClientException { message },
        "uri-syntax" => ReadFailure::UriSyntax {
            reason: message.unwrap_or_else(|| "Invalid resource URI syntax.".into()),
        },
        "eval" => ReadFailure::EvalError {
            message: message.unwrap_or_else(|| "Resource read error.".into()),
            hint: None,
        },
        _ => ReadFailure::Io(IoFailure::new(message)),
    }
}

pub fn parse_path_elements_js(value: &JsValue) -> Result<Vec<PathElement>, StructuredError> {
    if !Array::is_array(value) {
        return Err(StructuredError::invalid_argument(
            "expected array of PathElement",
        ));
    }
    let mut out = Vec::new();
    for item in Array::from(value).iter() {
        let name = super::util::pkl_string_from_js(&get_prop(&item, "name")?)?;
        let is_directory = get_prop(&item, "isDirectory")?.as_bool().ok_or_else(|| {
            StructuredError::invalid_argument("PathElement.isDirectory must be boolean")
        })?;
        out.push(PathElement { name, is_directory });
    }
    Ok(out)
}

fn uri_scheme_eq(uri: &PklString, expected: &str) -> bool {
    let raw = uri.to_java_utf8();
    raw.split_once(':')
        .is_some_and(|(scheme, _)| scheme.eq_ignore_ascii_case(expected))
}

pub fn is_package_glob_reader(reader: &GlobReader) -> bool {
    match reader {
        GlobReader::Module(key) => {
            uri_scheme_eq(&key.uri, "package") || uri_scheme_eq(&key.uri, "projectpackage")
        }
        GlobReader::Resource(r) => r.scheme.as_str().is_some_and(|s| {
            s.eq_ignore_ascii_case("package") || s.eq_ignore_ascii_case("projectpackage")
        }),
    }
}

pub fn has_uri_scheme(s: &str) -> bool {
    let Some((scheme, _)) = s.split_once(':') else {
        return false;
    };
    let mut chars = scheme.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    first.is_ascii_alphabetic()
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
}

pub fn map_package_uri_error(failure: PackageUriError) -> ModuleHostError {
    match failure.failure {
        PackageUriFailure::Unsupported(reason) => ModuleHostError::Unsupported(reason),
        _ => ModuleHostError::UriSyntax {
            reason: failure.reason(),
        },
    }
}

pub fn module_err_to_read_failure(err: ModuleHostError) -> ReadFailure {
    match err {
        ModuleHostError::SecurityManagerException { message } => {
            ReadFailure::SecurityManagerException { message }
        }
        ModuleHostError::HttpClientException { message } => {
            ReadFailure::HttpClientException { message }
        }
        ModuleHostError::ExternalReaderProcess { message } => {
            ReadFailure::ExternalReaderProcess { message }
        }
        ModuleHostError::UriSyntax { reason } => ReadFailure::UriSyntax { reason },
        ModuleHostError::EvalError { message, hint } => ReadFailure::EvalError { message, hint },
        ModuleHostError::Io(io) => ReadFailure::Io(io),
        ModuleHostError::NotFound => ReadFailure::Io(IoFailure::new(None)),
        other => ReadFailure::Io(IoFailure::from(format!("{other:?}"))),
    }
}

/// Static import declarations for all 23 Pkl 0.32.0 standard library modules (`crates/pkl-runtime/src/stdlib/*.pkl`),
/// mirroring `ModuleKeys.StandardLibrary.loadSource()` for `ImportGraphAnalysis` (`pkl:*`).
fn stdlib_analysis_source(uri: &PklString) -> Option<&'static str> {
    match uri.as_str()? {
        "pkl:analyze" => Some("import \"pkl:reflect\"\n"),
        "pkl:base" => Some(
            "import \"pkl:json\"\n\
             import \"pkl:jsonnet\"\n\
             import \"pkl:math\"\n\
             import \"pkl:pklbinary\"\n\
             import \"pkl:protobuf\"\n\
             import \"pkl:xml\"\n\
             import \"pkl:yaml\"\n",
        ),
        "pkl:Benchmark" => Some("import \"pkl:platform\" as _platform\n"),
        "pkl:Command" => Some("import \"pkl:Command\"\nimport \"pkl:reflect\"\n"),
        "pkl:DocPackageInfo" => Some(
            "import \"pkl:DocPackageInfo\"\n\
             import \"pkl:Project\"\n\
             import \"pkl:reflect\"\n\
             import \"pkl:semver\"\n",
        ),
        "pkl:DocsiteInfo" => Some("import \"pkl:reflect\"\n"),
        "pkl:EvaluatorSettings" => Some(
            "import \"pkl:EvaluatorSettings\"\n\
             import \"pkl:platform\"\n\
             import \"pkl:Project\"\n",
        ),
        "pkl:Project" | "pkl:project" => Some(
            "import \"pkl:EvaluatorSettings\" as EvaluatorSettingsModule\n\
             import \"pkl:Project\"\n\
             import \"pkl:reflect\"\n\
             import \"pkl:semver\"\n",
        ),
        "pkl:protobuf" => Some("import \"pkl:reflect\"\n"),
        "pkl:reflect" => Some("import \"pkl:base\"\n"),
        "pkl:release" => Some("import \"pkl:semver\"\n"),
        "pkl:settings" => Some("import \"pkl:EvaluatorSettings\"\n"),
        "pkl:json" | "pkl:jsonnet" | "pkl:math" | "pkl:pklbinary" | "pkl:platform" | "pkl:ref"
        | "pkl:semver" | "pkl:shell" | "pkl:test" | "pkl:xml" | "pkl:yaml" => Some(""),
        _ => None,
    }
}

/// Mirrors `IoUtils.doInferModuleName(URI)` (`crates/pkl-host-native/src/driver.rs:3457`).
pub fn infer_name_for_uri(uri: &str) -> PklString {
    let has_fragment_paths = uri.starts_with("package:") || uri.starts_with("projectpackage:");
    if has_fragment_paths {
        let slice = uri.rsplit('#').next().unwrap_or(uri);
        let leaf = slice.rsplit('/').next().unwrap_or(slice);
        let decoded = percent_decode(leaf);
        let stem = decoded.strip_suffix(".pkl").unwrap_or(&decoded);
        return if stem.is_empty() {
            PklString::from("module")
        } else {
            PklString::from(stem)
        };
    }

    if let Some((_scheme, ssp)) = uri.split_once(':')
        && !ssp.starts_with('/')
    {
        let ssp_no_frag = ssp.split('#').next().unwrap_or(ssp);
        let without_ver = ssp_no_frag
            .rsplit_once(':')
            .map_or(ssp_no_frag, |(head, _)| head);
        let last_seg = without_ver.rsplit('.').next().unwrap_or(without_ver);
        return if last_seg.is_empty() {
            PklString::from("module")
        } else {
            PklString::from(last_seg)
        };
    }

    let slice = uri.split(['?', '#']).next().unwrap_or(uri);
    let leaf = slice.rsplit('/').next().unwrap_or(slice);
    let decoded = percent_decode(leaf);
    let stem = decoded.strip_suffix(".pkl").unwrap_or(&decoded);
    if stem.is_empty() {
        PklString::from("module")
    } else {
        PklString::from(stem)
    }
}
