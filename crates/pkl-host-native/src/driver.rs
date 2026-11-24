//! Unified `NativeHost` evaluation & static import-analysis driver wiring:
//! - Configurable `SecurityPolicy` + `NativeFs` (`rootDir`, symlinks, `O_NOFOLLOW`, `jar:file:`, UNC)
//! - `NativeModulePathResolver` (`modulepath:`, `jar:file:` roundtrip, `...` triple-dot)
//! - `NativeHttpClient` (`http:`, `https:`, rewrites, headers, 20-hop redirect, proxy, CA certs)
//! - Track B `ExternalReaderRegistry` (`0x26`–`0x32` external module & resource reader subprocesses)
//! - Track C session-owned `PackageResolver` + `ProjectDependenciesManager` (`package:`, `projectpackage:`, `@dep/...`)
//! - `env:` & `prop:` scalar reads
//! - Real monotonic `Instant` clock, timeout watchdog, `CancelFlag` abort, and `close()`/`Drop` cleanup.

use crate::certs::CaCertSource;
use crate::fs::{NativeFs, canonicalize_root_dir, path_to_file_uri};
use crate::http::{CancelFlag, NativeHttpClient};
use crate::modulepath::{
    NativeModulePathResolver, read_generic_jar_resource, resolve_generic_jar_module,
};
use crate::package_host::{DiskPackageCache, NativePackageTransport};
use pkl_runtime::evaluation::frontend::{
    DiagnosticProfile, FrontendPhase, LosslessModuleSourceInput, ModuleEvalOptions,
    ModuleOutputResult, ModuleSourceInput,
};
use pkl_runtime::evaluation::session::{SourceSession, SourceSessionOptions};
use pkl_runtime::external_reader::{
    DEFAULT_CLOSE_TIMEOUT, ExternalReaderConfig, ExternalReaderRegistry,
    NativeExternalReaderProcess,
};
use pkl_runtime::glob::{
    GlobCompletion, GlobHostError, GlobReader, GlobRequest, GlobRequestKind, PathElement,
    ResourceReader, configured_completion_with_policy,
    unsupported_completion as unsupported_glob_completion,
};
use pkl_runtime::host::http::HttpClientConfig;
use pkl_runtime::host::security::SecurityPolicy;
use pkl_runtime::host::{
    HostPoll, HostSession, IoFailure, ReadFailure, format_timeout_message,
    try_configured_scalar_read_with_policy,
};
use pkl_runtime::logging::{FrameTransformer, LogDelivery, LogFrame, LogLevel, TraceMode};
use pkl_runtime::modules::analyze::{
    AnalysisFailure, AnalysisPoll, ImportGraph, ImportGraphAnalysis,
};
use pkl_runtime::modules::{
    ModuleCapabilities, ModuleCompletion, ModuleHostError, ModuleKey, ModuleKeyId, ModuleKeyKind,
    ModuleRequest, ModuleRequestKind, ModuleSource, ModuleSourceSpec, ResolvedModule,
    ResolvedModuleKey, default_completion, standard_key_completion_with_policy,
};
use pkl_runtime::packages::http::PackageTransport;
use pkl_runtime::packages::project::{
    DEFAULT_EDITOR_URL_SCHEME, ProjectDepEntry, ProjectDependenciesManager,
};
use pkl_runtime::packages::resolver::{
    PackageResolver, file_uri_from_path, percent_encode_fragment, resolve_relative_path,
};
use pkl_runtime::packages::{PackageAssetUri, PackageUriError, PackageUriFailure};
use pkl_runtime::resources::{ReadResult, Resource};
use pkl_runtime::source::SourceText;
use pkl_runtime::value::{ErrorKind, PklString, RuntimeError, Value};
use pkl_runtime::vm::ReadRequest;
use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

/// Configuration for constructing a [`NativeHost`].
#[derive(Clone, Debug)]
pub struct NativeHostConfig {
    pub allowed_modules: Option<Vec<String>>,
    pub allowed_resources: Option<Vec<String>>,
    pub root_dir: Option<PathBuf>,
    pub module_paths: Vec<PathBuf>,
    pub http: HttpClientConfig,
    pub ca_certs: CaCertSource,
    pub properties: Vec<(String, String)>,
    pub environment: Vec<(String, String)>,
    pub timeout_ms: Option<u64>,
    pub external_module_schemes: Vec<String>,
    pub external_resource_schemes: Vec<String>,
    pub external_module_readers: BTreeMap<String, ExternalReaderConfig>,
    pub external_resource_readers: BTreeMap<String, ExternalReaderConfig>,
    pub module_cache_dir: Option<PathBuf>,
    pub project_dependencies: Option<ProjectDependenciesManager>,
    pub test_mode: bool,
    pub trace_mode: TraceMode,
    pub power_assertions: bool,
    pub editor_url_scheme: String,
    /// When true (used exclusively when evaluating `settings.pkl` / `PklProject` into
    /// `pkl-binary`), `pkl:base#Module.output.renderer`'s internal `read?("prop:pkl.outputFormat")`
    /// resolves to `"pkl-binary"` without relaxing `SETTINGS_ALLOWED_RESOURCES` or user
    /// `--allowed-resources` for the evaluated module itself.
    pub internal_pkl_binary_eval: bool,
}

impl Default for NativeHostConfig {
    fn default() -> Self {
        Self {
            allowed_modules: None,
            allowed_resources: None,
            root_dir: None,
            module_paths: Vec::new(),
            http: HttpClientConfig::default(),
            ca_certs: CaCertSource::bundled_default(),
            properties: Vec::new(),
            environment: Vec::new(),
            timeout_ms: None,
            external_module_schemes: Vec::new(),
            external_resource_schemes: Vec::new(),
            external_module_readers: BTreeMap::new(),
            external_resource_readers: BTreeMap::new(),
            module_cache_dir: None,
            project_dependencies: None,
            test_mode: false,
            trace_mode: TraceMode::Compact,
            power_assertions: true,
            editor_url_scheme: DEFAULT_EDITOR_URL_SCHEME.to_string(),
            internal_pkl_binary_eval: false,
        }
    }
}

/// Custom log sink for `trace()` and `warn()` messages emitted during evaluation.
pub trait HostLogger: Send + Sync {
    fn log(&self, level: LogLevel, message: &PklString, frame: &LogFrame);
}

/// Custom stack-frame transformer applied to diagnostic and log stack frames.
pub trait HostFrameTransformer: Send + Sync {
    fn transform(&self, frame: LogFrame) -> LogFrame;
}

/// Custom security manager hook overriding default `SecurityPolicy` checks.
pub trait CustomSecurityChecker: Send + Sync {
    fn check_resolve_module(&self, uri: &PklString) -> Result<(), PklString>;
    fn check_import_module(
        &self,
        importing_module: &PklString,
        imported_module: &PklString,
    ) -> Result<(), PklString>;
    fn check_read_resource(&self, resource: &PklString) -> Result<(), PklString>;
    fn check_resolve_resource(&self, resource: &PklString) -> Result<(), PklString> {
        self.check_read_resource(resource)
    }
}

/// Custom in-process or remote (e.g. `pkl server` client) module key factory/reader.
pub trait CustomModuleReader: Send + Sync {
    /// Returns `true` if this reader claims the given URI during `SelectKey`.
    fn matches_uri(&self, uri: &PklString) -> bool;
    /// Whether URIs under this reader are hierarchical (support relative imports/reads).
    fn is_hierarchical(&self, uri: &PklString) -> bool;
    /// Whether modules under this reader are local (for import trust checks).
    fn is_local(&self, _uri: &PklString) -> bool {
        false
    }
    /// Whether this reader supports glob imports/reads (`import*`).
    fn is_globbable(&self, _uri: &PklString) -> bool {
        false
    }
    /// Whether resolved modules are cached by the evaluator (default `true`).
    fn is_cached(&self, _uri: &PklString) -> bool {
        true
    }
    /// Whether this reader stores asset paths in URI fragments (like `package:`).
    fn has_fragment_paths(&self, _uri: &PklString) -> bool {
        false
    }
    /// Resolves a module URI and loads its source text. Security checks have already run.
    fn resolve_module(
        &self,
        uri: &PklString,
    ) -> Result<(ResolvedModule, ModuleSource), ModuleHostError>;
    /// Checks whether a module element exists at `element_uri` (used by `...` and globbing).
    fn has_element(&self, _element_uri: &PklString) -> Result<bool, ModuleHostError> {
        Ok(false)
    }
    /// Lists child elements under `base_uri` when `is_globbable` returns `true`.
    fn list_elements(&self, _base_uri: &PklString) -> Result<Vec<PathElement>, ModuleHostError> {
        Err(ModuleHostError::Unsupported(
            "Module reader does not support listElements.".into(),
        ))
    }
    /// Optional cleanup hook invoked by [`NativeHost::close`].
    fn close(&self) {}
}

/// Custom in-process or remote (e.g. `pkl server` client) resource reader.
pub trait CustomResourceReader: Send + Sync {
    /// URI scheme without trailing `:` (matched case-insensitively).
    fn scheme(&self) -> &str;
    /// Whether URIs under this scheme are hierarchical.
    fn has_hierarchical_uris(&self) -> bool {
        false
    }
    /// Whether this reader supports glob reads (`read*`).
    fn is_globbable(&self) -> bool {
        false
    }
    /// Reads the resource at `uri`. Return `Ok(ReadResult::Missing)` if absent.
    fn read(&self, uri: &PklString) -> Result<ReadResult, ReadFailure>;
    /// Checks whether a resource element exists at `uri`.
    fn has_element(&self, uri: &PklString) -> Result<bool, ModuleHostError> {
        match self.read(uri) {
            Ok(ReadResult::Missing) => Ok(false),
            Ok(_) => Ok(true),
            Err(err) => Err(read_failure_to_module_err(err)),
        }
    }
    /// Lists child elements under `base_uri` when `is_globbable` returns `true`.
    fn list_elements(&self, _base_uri: &PklString) -> Result<Vec<PathElement>, ModuleHostError> {
        Err(ModuleHostError::Unsupported(
            "Resource reader does not support listElements.".into(),
        ))
    }
    /// Optional cleanup hook invoked by [`NativeHost::close`].
    fn close(&self) {}
}

/// Optional M5 extensions passed to [`NativeHost::with_extensions`].
///
/// Kept separate from [`NativeHostConfig`] so existing M4 callers remain source-compatible.
#[derive(Default)]
pub struct NativeHostExtensions {
    pub use_color: bool,
    pub output_format: Option<String>,
    pub logger: Option<Arc<dyn HostLogger>>,
    pub frame_transformer: Option<Arc<dyn HostFrameTransformer>>,
    pub skip_default_frame_transform: bool,
    pub custom_security: Option<Arc<dyn CustomSecurityChecker>>,
    pub custom_module_readers: Vec<Arc<dyn CustomModuleReader>>,
    pub custom_resource_readers: Vec<Arc<dyn CustomResourceReader>>,
    pub http_client: Option<Arc<NativeHttpClient>>,
    pub extra_allowed_module_schemes: Vec<String>,
    pub extra_allowed_resource_schemes: Vec<String>,
}

impl fmt::Debug for NativeHostExtensions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NativeHostExtensions")
            .field("use_color", &self.use_color)
            .field("output_format", &self.output_format)
            .field("has_logger", &self.logger.is_some())
            .field("has_frame_transformer", &self.frame_transformer.is_some())
            .field(
                "skip_default_frame_transform",
                &self.skip_default_frame_transform,
            )
            .field("has_custom_security", &self.custom_security.is_some())
            .field(
                "custom_module_readers_count",
                &self.custom_module_readers.len(),
            )
            .field(
                "custom_resource_readers_count",
                &self.custom_resource_readers.len(),
            )
            .field("has_shared_http_client", &self.http_client.is_some())
            .field(
                "extra_allowed_module_schemes",
                &self.extra_allowed_module_schemes,
            )
            .field(
                "extra_allowed_resource_schemes",
                &self.extra_allowed_resource_schemes,
            )
            .finish()
    }
}

impl NativeHostExtensions {
    pub fn with_color(mut self, use_color: bool) -> Self {
        self.use_color = use_color;
        self
    }

    pub fn with_output_format(mut self, output_format: impl Into<String>) -> Self {
        self.output_format = Some(output_format.into());
        self
    }

    pub fn with_logger(mut self, logger: Arc<dyn HostLogger>) -> Self {
        self.logger = Some(logger);
        self
    }

    pub fn with_frame_transformer(
        mut self,
        transformer: Arc<dyn HostFrameTransformer>,
        skip_default: bool,
    ) -> Self {
        self.frame_transformer = Some(transformer);
        self.skip_default_frame_transform = skip_default;
        self
    }

    pub fn with_custom_security(mut self, checker: Arc<dyn CustomSecurityChecker>) -> Self {
        self.custom_security = Some(checker);
        self
    }

    pub fn with_custom_module_reader(mut self, reader: Arc<dyn CustomModuleReader>) -> Self {
        self.custom_module_readers.push(reader);
        self
    }

    pub fn with_custom_resource_reader(mut self, reader: Arc<dyn CustomResourceReader>) -> Self {
        self.custom_resource_readers.push(reader);
        self
    }

    pub fn with_http_client(mut self, client: Arc<NativeHttpClient>) -> Self {
        self.http_client = Some(client);
        self
    }
}

/// Summary of cancelled/cleaned-up Host tokens observed on termination.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CleanupReport {
    pub cancelled_reads: Vec<String>,
    pub cancelled_modules: Vec<String>,
    pub cancelled_globs: Vec<String>,
    pub http_closed: bool,
    pub modulepath_closed: bool,
    pub external_readers_closed: bool,
}

/// Loaded primary module payload ready for `SourceSession` compilation.
#[derive(Clone, Debug)]
pub struct PrimaryModuleSource {
    pub original_uri: PklString,
    pub resolved_uri: PklString,
    pub text: SourceText,
    pub inferred_name: PklString,
}

/// Streaming multi-file output outcome matching `CliEvaluator.writeMultipleFileOutput`.
pub enum NativeStreamingOutputFilesOutcome<E> {
    Ok { stderr: Vec<u8> },
    EvalFailure(ModuleOutputResult),
    ConsumerError { error: E, stderr: Vec<u8> },
}

/// Cancellable wall-clock watchdog that aborts blocking HTTP / external-reader I/O
/// when `--timeout` elapses and joins immediately when evaluation completes early.
struct ActiveWatchdog {
    state: Arc<(Mutex<bool>, Condvar)>,
    timed_out: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

struct TimeoutWatchdog {
    active: Option<ActiveWatchdog>,
}

impl TimeoutWatchdog {
    fn none() -> Self {
        Self { active: None }
    }

    fn start(
        timeout_ms: Option<u64>,
        cancel_flag: Arc<CancelFlag>,
        external_readers: Arc<ExternalReaderRegistry>,
    ) -> Self {
        let Some(ms) = timeout_ms else {
            return Self { active: None };
        };
        let state = Arc::new((Mutex::new(false), Condvar::new()));
        let timed_out = Arc::new(AtomicBool::new(false));
        let state_bg = Arc::clone(&state);
        let timed_out_bg = Arc::clone(&timed_out);
        let handle = thread::Builder::new()
            .name("pkl-timeout-watchdog".into())
            .spawn(move || {
                let (lock, cvar) = &*state_bg;
                let guard = lock.lock().unwrap_or_else(|e| e.into_inner());
                let deadline = Instant::now() + Duration::from_millis(ms);
                let mut g = guard;
                while !*g {
                    let now = Instant::now();
                    if now >= deadline {
                        timed_out_bg.store(true, Ordering::SeqCst);
                        cancel_flag.cancel();
                        external_readers.cancel_all();
                        return;
                    }
                    let wait = deadline.saturating_duration_since(now);
                    let (next, res) = cvar
                        .wait_timeout(g, wait)
                        .unwrap_or_else(|e| e.into_inner());
                    g = next;
                    if res.timed_out() && !*g {
                        timed_out_bg.store(true, Ordering::SeqCst);
                        cancel_flag.cancel();
                        external_readers.cancel_all();
                        return;
                    }
                }
            })
            .ok();
        Self {
            active: Some(ActiveWatchdog {
                state,
                timed_out,
                handle,
            }),
        }
    }

    fn triggered(&self) -> bool {
        self.active
            .as_ref()
            .is_some_and(|w| w.timed_out.load(Ordering::SeqCst))
    }

    fn stop(mut self) -> bool {
        self.finish_inner();
        self.triggered()
    }

    fn finish_inner(&mut self) {
        let Some(active) = &mut self.active else {
            return;
        };
        let (lock, cvar) = &*active.state;
        {
            let mut done = lock.lock().unwrap_or_else(|e| e.into_inner());
            *done = true;
        }
        cvar.notify_all();
        if let Some(handle) = active.handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for TimeoutWatchdog {
    fn drop(&mut self) {
        self.finish_inner();
    }
}

/// Complete native Host environment for `pkl-runtime` evaluation & import analysis.
pub struct NativeHost {
    fs: NativeFs,
    modulepath: NativeModulePathResolver,
    http: Arc<NativeHttpClient>,
    owns_http_client: bool,
    external_readers: Arc<ExternalReaderRegistry>,
    package_resolver: Arc<PackageResolver>,
    project_deps: Option<ProjectDependenciesManager>,
    cancel_flag: Arc<CancelFlag>,
    clock_epoch: Instant,
    properties: Vec<(String, String)>,
    environment: Vec<(String, String)>,
    timeout_ms: Option<u64>,
    test_mode: bool,
    trace_mode: TraceMode,
    power_assertions: bool,
    editor_url_scheme: String,
    internal_pkl_binary_eval: bool,
    use_color: bool,
    logger: Option<Arc<dyn HostLogger>>,
    frame_transformer: Option<Arc<dyn HostFrameTransformer>>,
    skip_default_frame_transform: bool,
    custom_security: Option<Arc<dyn CustomSecurityChecker>>,
    custom_module_readers: Vec<Arc<dyn CustomModuleReader>>,
    custom_resource_readers: Vec<Arc<dyn CustomResourceReader>>,
    /// Loaded module bytes staged between `Resolve`/`ResolveKey` and `LoadSource`.
    staged_sources: Mutex<BTreeMap<PklString, Vec<u8>>>,
    /// Canonical filesystem paths staged between `Resolve`/`ResolveKey` and `LoadSource`
    /// so `file:` modules are only opened and read on cache miss (`LoadSource`), matching
    /// upstream `ModuleKeys.File.resolve` vs `ResolvedModuleKeys.File.loadSource`.
    resolved_file_paths: Mutex<Vec<(PklString, PathBuf)>>,
    /// Loaded custom module sources staged between `Resolve`/`ResolveKey` and `LoadSource`.
    staged_custom_sources: Mutex<BTreeMap<PklString, ModuleSource>>,
}

impl NativeHost {
    pub fn new(config: NativeHostConfig) -> Result<Self, String> {
        Self::with_extensions(config, NativeHostExtensions::default())
    }

    pub fn with_extensions(
        config: NativeHostConfig,
        ext: NativeHostExtensions,
    ) -> Result<Self, String> {
        let normalized_root = match &config.root_dir {
            Some(dir) => {
                let real = canonicalize_root_dir(dir).map_err(|err| {
                    format!("Failed to normalize rootDir `{}`: {err}", dir.display())
                })?;
                Some(real.to_string_lossy().into_owned())
            }
            None => None,
        };

        let mut ext_mod_schemes = config.external_module_schemes.clone();
        for scheme in config
            .external_module_readers
            .keys()
            .chain(ext.extra_allowed_module_schemes.iter())
        {
            if !ext_mod_schemes
                .iter()
                .any(|s| s.eq_ignore_ascii_case(scheme))
            {
                ext_mod_schemes.push(scheme.clone());
            }
        }
        let mut ext_res_schemes = config.external_resource_schemes.clone();
        for scheme in config
            .external_resource_readers
            .keys()
            .chain(ext.extra_allowed_resource_schemes.iter())
        {
            if !ext_res_schemes.iter().any(|s| s == scheme) {
                ext_res_schemes.push(scheme.clone());
            }
        }

        let policy = if ext.custom_security.is_some() {
            let allow_all = vec![".*".to_string()];
            SecurityPolicy::new(Some(&allow_all), Some(&allow_all), None)?
        } else {
            SecurityPolicy::new(
                config.allowed_modules.as_deref(),
                config.allowed_resources.as_deref(),
                normalized_root,
            )?
            .with_external_module_schemes(&ext_mod_schemes)?
            .with_external_resource_schemes(&ext_res_schemes)?
        };

        let mut http_cfg = config.http;
        if let Some(ms) = config.timeout_ms {
            http_cfg.connect_timeout_ms = http_cfg.connect_timeout_ms.min(ms);
            http_cfg.request_timeout_ms = http_cfg.request_timeout_ms.min(ms);
        }

        let cancel_flag = CancelFlag::new();
        let (http, owns_http_client) = match ext.http_client {
            Some(shared) => (shared, false),
            None => (
                Arc::new(NativeHttpClient::new(
                    http_cfg,
                    config.ca_certs,
                    cancel_flag.clone(),
                )),
                true,
            ),
        };

        let external_readers = if config.external_module_readers.is_empty()
            && config.external_resource_readers.is_empty()
        {
            static EMPTY_READERS: std::sync::OnceLock<Arc<ExternalReaderRegistry>> =
                std::sync::OnceLock::new();
            Arc::clone(EMPTY_READERS.get_or_init(|| Arc::new(ExternalReaderRegistry::new(0))))
        } else {
            let req_timeout = config.timeout_ms.map(Duration::from_millis);
            let mut registry = ExternalReaderRegistry::new(0);
            for (scheme, reader_cfg) in config.external_module_readers {
                registry.register_module_reader_with(scheme, reader_cfg, |cfg| {
                    Arc::new(NativeExternalReaderProcess::with_timeouts(
                        cfg.clone(),
                        DEFAULT_CLOSE_TIMEOUT,
                        req_timeout,
                    ))
                });
            }
            for (scheme, reader_cfg) in config.external_resource_readers {
                registry.register_resource_reader_with(scheme, reader_cfg, |cfg| {
                    Arc::new(NativeExternalReaderProcess::with_timeouts(
                        cfg.clone(),
                        DEFAULT_CLOSE_TIMEOUT,
                        req_timeout,
                    ))
                });
            }
            Arc::new(registry)
        };

        let pkg_transport: Arc<dyn PackageTransport> = Arc::new(NativePackageTransport::new(
            http.clone(),
            Arc::new(policy.clone()),
        ));
        let package_resolver = Arc::new(match config.module_cache_dir {
            Some(cache_dir) => PackageResolver::disk_cached(
                pkg_transport,
                Arc::new(DiskPackageCache::new(cache_dir)),
                config.test_mode,
            ),
            None => PackageResolver::in_memory(pkg_transport, config.test_mode),
        });

        let mut properties = config.properties;
        if let Some(output_format) = ext.output_format {
            if let Some(entry) = properties.iter_mut().find(|(k, _)| k == "pkl.outputFormat") {
                entry.1 = output_format;
            } else {
                properties.push(("pkl.outputFormat".to_string(), output_format));
            }
        }

        Ok(Self {
            fs: NativeFs::new(policy),
            modulepath: NativeModulePathResolver::new(config.module_paths),
            http,
            owns_http_client,
            external_readers,
            package_resolver,
            project_deps: config.project_dependencies,
            cancel_flag,
            clock_epoch: Instant::now(),
            properties,
            environment: config.environment,
            timeout_ms: config.timeout_ms,
            test_mode: config.test_mode,
            trace_mode: config.trace_mode,
            power_assertions: config.power_assertions,
            editor_url_scheme: config.editor_url_scheme,
            internal_pkl_binary_eval: config.internal_pkl_binary_eval,
            use_color: ext.use_color,
            logger: ext.logger,
            frame_transformer: ext.frame_transformer,
            skip_default_frame_transform: ext.skip_default_frame_transform,
            custom_security: ext.custom_security,
            custom_module_readers: ext.custom_module_readers,
            custom_resource_readers: ext.custom_resource_readers,
            staged_sources: Mutex::new(BTreeMap::new()),
            resolved_file_paths: Mutex::new(Vec::new()),
            staged_custom_sources: Mutex::new(BTreeMap::new()),
        })
    }

    pub fn policy(&self) -> &SecurityPolicy {
        self.fs.policy()
    }

    pub fn fs(&self) -> &NativeFs {
        &self.fs
    }

    pub fn http_client(&self) -> Arc<NativeHttpClient> {
        self.http.clone()
    }

    pub fn package_resolver(&self) -> Arc<PackageResolver> {
        self.package_resolver.clone()
    }

    pub fn external_readers(&self) -> &ExternalReaderRegistry {
        &self.external_readers
    }

    pub fn cancel_flag(&self) -> Arc<CancelFlag> {
        self.cancel_flag.clone()
    }

    pub fn clock_ms(&self) -> u64 {
        self.clock_epoch.elapsed().as_millis().min(u64::MAX as u128) as u64
    }

    pub fn use_color(&self) -> bool {
        self.use_color
    }

    pub fn timeout_ms(&self) -> Option<u64> {
        self.timeout_ms
    }

    pub fn project_dependencies(&self) -> Option<&ProjectDependenciesManager> {
        self.project_deps.as_ref()
    }

    pub fn cancel(&self) {
        self.cancel_flag.cancel();
        self.external_readers.cancel_all();
    }

    pub fn is_closed(&self) -> bool {
        if self.owns_http_client {
            self.http.is_closed() && self.modulepath.is_closed()
        } else {
            self.modulepath.is_closed() && self.cancel_flag.is_cancelled()
        }
    }

    /// Closes all owned native resources (HTTP client if owned, external & custom readers,
    /// package resolver, modulepath archive handles, staged buffers).
    pub fn close(&self) {
        self.cancel_flag.cancel();
        self.external_readers.close_all();
        for reader in &self.custom_module_readers {
            reader.close();
        }
        for reader in &self.custom_resource_readers {
            reader.close();
        }
        self.package_resolver.close();
        if self.owns_http_client {
            self.http.close();
        }
        self.modulepath.close();
        let mut staged = self
            .staged_sources
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        staged.clear();
        let mut resolved_paths = self
            .resolved_file_paths
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        resolved_paths.clear();
        let mut custom_staged = self
            .staged_custom_sources
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        custom_staged.clear();
    }

    fn check_resolve_module(&self, uri: &PklString) -> Result<(), ModuleHostError> {
        if let Some(ref checker) = self.custom_security {
            checker
                .check_resolve_module(uri)
                .map_err(|msg| ModuleHostError::SecurityManagerException { message: Some(msg) })
        } else {
            self.fs.policy().check_resolve_module(uri)
        }
    }

    fn check_import_module(
        &self,
        importer: &PklString,
        import_uri: &PklString,
    ) -> Result<(), ModuleHostError> {
        if let Some(ref checker) = self.custom_security {
            checker
                .check_import_module(importer, import_uri)
                .map_err(|msg| ModuleHostError::SecurityManagerException { message: Some(msg) })
        } else {
            self.fs.policy().check_import_module(importer, import_uri)
        }
    }

    fn check_read_resource(&self, uri: &PklString) -> Result<(), ReadFailure> {
        if let Some(ref checker) = self.custom_security {
            checker
                .check_read_resource(uri)
                .map_err(|msg| ReadFailure::SecurityManagerException { message: Some(msg) })
        } else {
            self.fs
                .check_uri(uri, true)
                .map_err(|msg| ReadFailure::SecurityManagerException { message: Some(msg) })
        }
    }

    fn check_resolve_resource_for_glob(&self, uri: &PklString) -> Result<(), ModuleHostError> {
        if let Some(ref checker) = self.custom_security {
            checker
                .check_resolve_resource(uri)
                .map_err(|msg| ModuleHostError::SecurityManagerException { message: Some(msg) })
        } else {
            self.fs
                .check_uri(uri, true)
                .map_err(|msg| ModuleHostError::SecurityManagerException { message: Some(msg) })
        }
    }

    fn emit_log_event(&self, event: &pkl_runtime::logging::LogEvent, stderr: &mut Vec<u8>) {
        if let Some(ref logger) = self.logger {
            logger.log(event.level, &event.message, &event.frame);
        } else {
            let level = match event.level {
                LogLevel::Trace => "TRACE",
                LogLevel::Warn => "WARN",
            };
            let uri = event.frame.module_uri.to_java_utf8();
            let message = event.message.to_java_utf8();
            let space = if message.ends_with('\n') { "" } else { " " };
            stderr.extend_from_slice(format!("pkl: {level}: {message}{space}({uri})\n").as_bytes());
        }
    }

    fn transform_log_frame(&self, frame: LogFrame) -> LogFrame {
        if self.skip_default_frame_transform {
            return match &self.frame_transformer {
                Some(custom) => custom.transform(frame),
                None => frame,
            };
        }
        let transformed = if self.test_mode {
            frame
        } else {
            let mut transformed = FrameTransformer::Default.transform(frame);
            let uri_str = transformed.module_uri.to_java_utf8();
            if uri_str.starts_with("file:") {
                let formatted = self
                    .editor_url_scheme
                    .replace("%{url}", &uri_str)
                    .replace("%{line}", &transformed.start_line.to_string());
                transformed.module_uri = PklString::from(formatted);
            } else if uri_str.len() >= 8
                && uri_str[..8].eq_ignore_ascii_case("package:")
                && let Ok(asset_uri) = PackageAssetUri::parse(transformed.module_uri.clone())
                && let Ok(meta) = self
                    .package_resolver
                    .get_dependency_metadata(&asset_uri.package, None)
                && let Some(ref scheme) = meta.source_code_url_scheme
            {
                // Mirrors `StackFrameTransformers.replacePackageUriWithSourceCodeUrl`
                // (`StackFrameTransformers.java:47-88`).
                let path = asset_uri.asset_path.to_java_utf8();
                let formatted = scheme
                    .to_java_utf8()
                    .replace("%{path}", &path)
                    .replace("%{url}", &uri_str)
                    .replace("%{line}", &transformed.start_line.to_string())
                    .replace("%{endLine}", &transformed.end_line.to_string())
                    .replace("%{column}", &transformed.start_column.to_string())
                    .replace("%{endColumn}", &transformed.end_column.to_string());
                transformed.module_uri = PklString::from(formatted);
            }
            transformed
        };
        match &self.frame_transformer {
            Some(custom) => custom.transform(transformed),
            None => transformed,
        }
    }

    fn format_timeout_output(&self, timeout_ms: u64) -> ModuleOutputResult {
        let message = format!("{}\n", format_timeout_message(timeout_ms));
        ModuleOutputResult {
            phase: FrontendPhase::ModuleOutput,
            disposition: None,
            reason: None,
            stdout: Vec::new(),
            stderr: message.into_bytes(),
            exit_code: 1,
        }
    }

    fn session_options(&self, diagnostic_profile: DiagnosticProfile) -> SourceSessionOptions {
        SourceSessionOptions {
            evaluation: pkl_runtime::evaluation::EvaluationOptions {
                power_assertions: self.power_assertions,
            },
            trace_mode: self.trace_mode,
            log_delivery: LogDelivery::Host,
            frame_transformer: if diagnostic_profile == DiagnosticProfile::CliBundledSettings
                || self.frame_transformer.is_some()
                || self.skip_default_frame_transform
            {
                FrameTransformer::Host
            } else {
                FrameTransformer::Default
            },
            platform_data: diagnostic_profile == DiagnosticProfile::CliBundledSettings,
        }
    }

    /// Creates a reusable public `SourceSession` (shared module cache across calls)
    /// configured with this host's evaluation, tracing, color, and frame-transformer settings.
    pub fn new_public_session(&self, diagnostic_profile: DiagnosticProfile) -> SourceSession {
        let pure_standard = self.custom_security.is_none()
            && self.custom_module_readers.is_empty()
            && self.external_readers.is_empty();
        SourceSession::module_evaluator(self.session_options(diagnostic_profile))
            .with_color(self.use_color)
            .with_pure_standard_host(pure_standard)
    }

    /// Normalizes a top-level [`ModuleSourceSpec`], expanding `@dep/...` URIs when
    /// project dependencies are configured and no inline contents are provided.
    pub fn resolve_module_source_spec(
        &self,
        spec: ModuleSourceSpec,
    ) -> Result<ModuleSourceSpec, ModuleHostError> {
        if spec.contents.is_none()
            && let Some(raw) = spec.uri.as_str()
            && raw.starts_with('@')
        {
            let resolved = self.resolve_top_level_dependency_uri(raw)?;
            return Ok(ModuleSourceSpec {
                uri: resolved,
                contents: None,
            });
        }
        Ok(spec)
    }

    /// Drives an already-started operation on a reusable `SourceSession` using this host's
    /// monotonic clock, custom/external/built-in handlers, and per-call timeout watchdog,
    /// without closing `session` or `self`.
    pub fn drive_public_session(
        &self,
        session: &mut SourceSession,
        stderr: &mut Vec<u8>,
    ) -> Result<Value, RuntimeError> {
        let watchdog = if self.timeout_ms.is_some() {
            let eval_cancel = CancelFlag::new();
            TimeoutWatchdog::start(self.timeout_ms, eval_cancel, self.external_readers.clone())
        } else {
            TimeoutWatchdog::none()
        };
        let now_ms = || self.clock_ms();
        let result = self.drive_source_session(session, &now_ms, &watchdog, stderr);
        let timed_out = watchdog.stop();
        match result {
            Ok(value) => Ok(value),
            Err(err) if timed_out && err.kind != ErrorKind::Timeout => Err(RuntimeError::new(
                ErrorKind::Timeout,
                format_timeout_message(self.timeout_ms.unwrap_or(0)),
            )),
            Err(err) => Err(err),
        }
    }

    /// Evaluates an expression against `source` on a persistent `SourceSession` and returns
    /// `pkl-binary` MessagePack bytes (shared by `pkl-server`, `crates/pkl`, and `pkl-doc`).
    pub fn evaluate_expression_pkl_binary_on(
        &self,
        session: &mut SourceSession,
        source: ModuleSourceSpec,
        expression: &str,
    ) -> Result<Vec<u8>, String> {
        let spec = match self.resolve_module_source_spec(source) {
            Ok(spec) => spec,
            Err(err) => {
                let res = self.render_primary_load_error("", err, &ModuleEvalOptions::default());
                return Err(String::from_utf8_lossy(&res.stderr)
                    .trim_end_matches('\n')
                    .to_string());
            }
        };
        let mut stderr = Vec::new();
        let outcome = (|| -> Result<Vec<u8>, RuntimeError> {
            session.start_evaluation_pkl_binary(
                spec,
                expression,
                self.clock_ms(),
                self.timeout_ms,
            )?;
            let val = self.drive_public_session(session, &mut stderr)?;
            SourceSession::result_bytes(val).map(|b| b.to_vec())
        })();
        match outcome {
            Ok(bytes) => Ok(bytes),
            Err(err) if err.kind == ErrorKind::Timeout => {
                Err(format_timeout_message(self.timeout_ms.unwrap_or(0)))
            }
            Err(err) => Err(session.format_error(err)),
        }
    }

    /// Convenience one-shot wrapper around [`Self::evaluate_expression_pkl_binary_on`].
    pub fn evaluate_expression_pkl_binary(
        &self,
        source: ModuleSourceSpec,
        expression: &str,
    ) -> Result<Vec<u8>, String> {
        let mut session = self.new_public_session(DiagnosticProfile::ApiIdentity);
        let result = self.evaluate_expression_pkl_binary_on(&mut session, source, expression);
        session.close();
        result
    }

    /// Evaluates `output.text` on a persistent `SourceSession`.
    pub fn evaluate_output_text_on(
        &self,
        session: &mut SourceSession,
        source: ModuleSourceSpec,
    ) -> Result<String, String> {
        let spec = self.resolve_module_source_spec(source).map_err(|err| {
            let res = self.render_primary_load_error("", err, &ModuleEvalOptions::default());
            String::from_utf8_lossy(&res.stderr)
                .trim_end_matches('\n')
                .to_string()
        })?;
        let mut stderr = Vec::new();
        let outcome = (|| -> Result<String, RuntimeError> {
            session.start_evaluation(spec, "output.text", self.clock_ms(), self.timeout_ms)?;
            let val = self.drive_public_session(session, &mut stderr)?;
            SourceSession::result_string(val).map(|s| s.to_java_utf8())
        })();
        match outcome {
            Ok(text) => Ok(text),
            Err(err) if err.kind == ErrorKind::Timeout => {
                Err(format_timeout_message(self.timeout_ms.unwrap_or(0)))
            }
            Err(err) => Err(session.format_error(err)),
        }
    }

    /// Evaluates `output.bytes` on a persistent `SourceSession`.
    pub fn evaluate_output_bytes_on(
        &self,
        session: &mut SourceSession,
        source: ModuleSourceSpec,
    ) -> Result<Vec<u8>, String> {
        let spec = self.resolve_module_source_spec(source).map_err(|err| {
            let res = self.render_primary_load_error("", err, &ModuleEvalOptions::default());
            String::from_utf8_lossy(&res.stderr)
                .trim_end_matches('\n')
                .to_string()
        })?;
        let mut stderr = Vec::new();
        let outcome = (|| -> Result<Vec<u8>, RuntimeError> {
            session.start_evaluation_output_bytes(spec, self.clock_ms(), self.timeout_ms)?;
            let val = self.drive_public_session(session, &mut stderr)?;
            SourceSession::result_bytes(val).map(|b| b.to_vec())
        })();
        match outcome {
            Ok(bytes) => Ok(bytes),
            Err(err) if err.kind == ErrorKind::Timeout => {
                Err(format_timeout_message(self.timeout_ms.unwrap_or(0)))
            }
            Err(err) => Err(session.format_error(err)),
        }
    }

    /// Evaluates an expression as a string (`ToString`) on a persistent `SourceSession`.
    pub fn evaluate_expression_string_on(
        &self,
        session: &mut SourceSession,
        source: ModuleSourceSpec,
        expression: &str,
    ) -> Result<String, String> {
        let spec = self.resolve_module_source_spec(source).map_err(|err| {
            let res = self.render_primary_load_error("", err, &ModuleEvalOptions::default());
            String::from_utf8_lossy(&res.stderr)
                .trim_end_matches('\n')
                .to_string()
        })?;
        let mut stderr = Vec::new();
        let outcome = (|| -> Result<String, RuntimeError> {
            session.start_evaluation(spec, expression, self.clock_ms(), self.timeout_ms)?;
            let val = self.drive_public_session(session, &mut stderr)?;
            SourceSession::result_string(val).map(|s| s.to_java_utf8())
        })();
        match outcome {
            Ok(text) => Ok(text),
            Err(err) if err.kind == ErrorKind::Timeout => {
                Err(format_timeout_message(self.timeout_ms.unwrap_or(0)))
            }
            Err(err) => Err(session.format_error(err)),
        }
    }

    /// Loads and returns the raw module `Value` on a persistent `SourceSession`
    /// (shared by `pkl-doc` / schema extraction and `crates/pkl`).
    pub fn load_module_on(
        &self,
        session: &mut SourceSession,
        source: ModuleSourceSpec,
    ) -> Result<Value, String> {
        let spec = self.resolve_module_source_spec(source).map_err(|err| {
            let res = self.render_primary_load_error("", err, &ModuleEvalOptions::default());
            String::from_utf8_lossy(&res.stderr)
                .trim_end_matches('\n')
                .to_string()
        })?;
        let mut stderr = Vec::new();
        let outcome = (|| -> Result<Value, RuntimeError> {
            session.start_load_module(spec, self.clock_ms(), self.timeout_ms)?;
            self.drive_public_session(session, &mut stderr)
        })();
        match outcome {
            Ok(val) => Ok(val),
            Err(err) if err.kind == ErrorKind::Timeout => {
                Err(format_timeout_message(self.timeout_ms.unwrap_or(0)))
            }
            Err(err) => Err(session.format_error(err)),
        }
    }

    /// Evaluates a `HostSession` entry point using real monotonic wall-clock `Instant`,
    /// enforcing `timeout_ms` and `CancelFlag`, and unconditionally cleaning up resources
    /// and draining cancelled Host tokens before returning.
    pub fn run_session(
        &self,
        session: &mut HostSession,
        entry: usize,
        inputs: Vec<Value>,
        step_budget: usize,
    ) -> (Result<Value, RuntimeError>, CleanupReport) {
        let start = Instant::now();
        let now_ms = || {
            if self.timeout_ms.is_some() {
                start.elapsed().as_millis().min(u64::MAX as u128) as u64
            } else {
                0
            }
        };
        let watchdog = TimeoutWatchdog::start(
            self.timeout_ms,
            self.cancel_flag.clone(),
            self.external_readers.clone(),
        );

        let outcome = (|| -> Result<Value, RuntimeError> {
            session.start(entry, inputs, now_ms(), self.timeout_ms)?;
            loop {
                if self.cancel_flag.is_cancelled() && !watchdog.triggered() {
                    session.cancel();
                }
                match session.poll(now_ms(), step_budget) {
                    HostPoll::Ready(result) => return result,
                    HostPoll::Yielded => continue,
                    HostPoll::Read { token, .. } => {
                        let req = session.read_request(&token)?;
                        let result = self.handle_read_request(&req);
                        let t = now_ms();
                        if self.cancel_flag.is_cancelled() && !watchdog.triggered() {
                            session.cancel();
                        }
                        session.complete_read(&token, result, t)?;
                    }
                    HostPoll::Module { token, request } => {
                        let completion = self.handle_module_request(&request);
                        let t = now_ms();
                        if self.cancel_flag.is_cancelled() && !watchdog.triggered() {
                            session.cancel();
                        }
                        session.complete_module(&token, completion, t)?;
                    }
                    HostPoll::Glob { token, request } => {
                        let completion = self.handle_glob_request(&request);
                        let t = now_ms();
                        if self.cancel_flag.is_cancelled() && !watchdog.triggered() {
                            session.cancel();
                        }
                        session.complete_glob(&token, completion, t)?;
                    }
                    HostPoll::Log { token, event } => {
                        if let Some(ref logger) = self.logger {
                            logger.log(event.level, &event.message, &event.frame);
                        }
                        session.acknowledge_log(&token, Ok(()), now_ms())?;
                    }
                    HostPoll::TransformFrame { token, frame } => {
                        let transformed = self.transform_log_frame((*frame).clone());
                        session.complete_frame_transform(&token, Ok(transformed), now_ms())?;
                    }
                }
            }
        })();

        let _ = watchdog.stop();
        self.external_readers.close_all();
        self.package_resolver.close();
        let report = CleanupReport {
            cancelled_reads: session.drain_cancelled_reads(),
            cancelled_modules: session.drain_cancelled_modules(),
            cancelled_globs: session.drain_cancelled_globs(),
            http_closed: {
                self.http.close();
                self.http.is_closed()
            },
            modulepath_closed: {
                self.modulepath.close();
                self.modulepath.is_closed()
            },
            external_readers_closed: true,
        };
        session.close();
        (outcome, report)
    }

    /// Evaluates a Pkl module to raw `ModuleOutputResult` (`stdout`/`stderr`/`exit_code`)
    /// using real monotonic `Instant` clock, enforcing `timeout_ms` and `CancelFlag`, and
    /// closing all native Host resources before returning (Wave 1 API preserved).
    pub fn evaluate_module_output(
        &self,
        input: ModuleSourceInput<'_>,
        options: &ModuleEvalOptions,
    ) -> (ModuleOutputResult, CleanupReport) {
        let sanitized = sanitized_eval_options(options);
        let (result, cancelled_reads, cancelled_modules, cancelled_globs) = self
            .run_compiled_module_output(
                LosslessModuleSourceInput::from(input),
                None,
                &sanitized,
                Instant::now(),
            );
        self.external_readers.close_all();
        self.package_resolver.close();
        let report = CleanupReport {
            cancelled_reads,
            cancelled_modules,
            cancelled_globs,
            http_closed: {
                self.http.close();
                self.http.is_closed()
            },
            modulepath_closed: {
                self.modulepath.close();
                self.modulepath.is_closed()
            },
            external_readers_closed: true,
        };
        (result, report)
    }

    /// Evaluates a top-level URI (or supplied stdin source) for CLI `eval` (single module
    /// or `-x`/`--expression`) WITHOUT closing the host so multi-module commands share
    /// caches and external reader processes until `self.close()`.
    pub fn eval_uri_output(
        &self,
        uri: &str,
        supplied_text: Option<&str>,
        expression: Option<&str>,
        options: &ModuleEvalOptions,
    ) -> ModuleOutputResult {
        let start = Instant::now();
        let sanitized = sanitized_eval_options(options);

        let primary = match supplied_text {
            Some(text) => PrimaryModuleSource {
                original_uri: PklString::from(uri),
                resolved_uri: PklString::from(uri),
                text: SourceText::from(text),
                inferred_name: infer_name_for_uri(uri),
            },
            None => match self.load_primary_module_with_timeout(uri, start, &sanitized) {
                Ok(src) => src,
                Err(failure) => return failure,
            },
        };

        let input = LosslessModuleSourceInput {
            text: primary.text,
            uri: primary.original_uri,
            inferred_name: primary.inferred_name,
        };
        let (result, _, _, _) =
            self.run_compiled_module_output(input, expression, &sanitized, start);
        result
    }

    /// Evaluates a top-level URI for CLI `eval -m` (`output.files` streaming), invoking
    /// `consumer(path_spec, eval_bytes)` lazily per entry without closing the host.
    pub fn eval_uri_output_files<E, F>(
        &self,
        uri: &str,
        supplied_text: Option<&str>,
        options: &ModuleEvalOptions,
        mut consumer: F,
    ) -> NativeStreamingOutputFilesOutcome<E>
    where
        F: FnMut(&PklString, &mut dyn FnMut() -> Result<Vec<u8>, ()>) -> Result<(), E>,
    {
        let start = Instant::now();
        let sanitized = sanitized_eval_options(options);

        let primary = match supplied_text {
            Some(text) => PrimaryModuleSource {
                original_uri: PklString::from(uri),
                resolved_uri: PklString::from(uri),
                text: SourceText::from(text),
                inferred_name: infer_name_for_uri(uri),
            },
            None => match self.load_primary_module_with_timeout(uri, start, &sanitized) {
                Ok(src) => src,
                Err(failure) => return NativeStreamingOutputFilesOutcome::EvalFailure(failure),
            },
        };

        let input = LosslessModuleSourceInput {
            text: primary.text,
            uri: primary.original_uri,
            inferred_name: primary.inferred_name,
        };

        let mut session = match SourceSession::compile_output_colored(
            input,
            &[],
            &sanitized,
            self.session_options(sanitized.diagnostic_profile),
            self.use_color,
        ) {
            Ok(s) => s,
            Err(error) => return NativeStreamingOutputFilesOutcome::EvalFailure(error.into()),
        };

        let remaining_ms = self.remaining_timeout_ms(start);
        if let Some(limit) = self.timeout_ms
            && remaining_ms == Some(0)
        {
            session.close();
            return NativeStreamingOutputFilesOutcome::EvalFailure(
                self.format_timeout_output(limit),
            );
        }

        let watchdog = TimeoutWatchdog::start(
            remaining_ms,
            self.cancel_flag.clone(),
            self.external_readers.clone(),
        );
        let mut stderr = Vec::new();
        let now_ms = || {
            if self.timeout_ms.is_some() {
                start.elapsed().as_millis().min(u64::MAX as u128) as u64
            } else {
                0
            }
        };

        let keys_result = (|| -> Result<Vec<PklString>, RuntimeError> {
            session.start_output_file_keys(now_ms(), self.timeout_ms)?;
            let val = self.drive_source_session(&mut session, &now_ms, &watchdog, &mut stderr)?;
            SourceSession::result_output_file_keys(val)
        })();

        let keys = match keys_result {
            Ok(keys) => keys,
            Err(err) => {
                let timed_out = watchdog.stop();
                let mut res = if timed_out || err.kind == ErrorKind::Timeout {
                    self.format_timeout_output(self.timeout_ms.unwrap_or(0))
                } else {
                    session.output_failure(err).into()
                };
                stderr.extend_from_slice(&res.stderr);
                res.stderr = stderr;
                session.close();
                return NativeStreamingOutputFilesOutcome::EvalFailure(res);
            }
        };

        let mut eval_failure: Option<RuntimeError> = None;
        let mut consumer_error: Option<E> = None;

        for key in &keys {
            let mut eval_bytes = || -> Result<Vec<u8>, ()> {
                match (|| -> Result<Vec<u8>, RuntimeError> {
                    session.start_output_file_bytes(key.clone(), now_ms(), self.timeout_ms)?;
                    let val =
                        self.drive_source_session(&mut session, &now_ms, &watchdog, &mut stderr)?;
                    SourceSession::result_bytes(val).map(|b| b.to_vec())
                })() {
                    Ok(bytes) => Ok(bytes),
                    Err(err) => {
                        eval_failure = Some(err);
                        Err(())
                    }
                }
            };
            if let Err(err) = consumer(key, &mut eval_bytes) {
                consumer_error = Some(err);
                break;
            }
        }

        let timed_out = watchdog.stop();
        if let Some(err) = eval_failure {
            let mut res = if timed_out || err.kind == ErrorKind::Timeout {
                self.format_timeout_output(self.timeout_ms.unwrap_or(0))
            } else {
                session.output_failure(err).into()
            };
            stderr.extend_from_slice(&res.stderr);
            res.stderr = stderr;
            session.close();
            return NativeStreamingOutputFilesOutcome::EvalFailure(res);
        }
        session.close();
        if let Some(error) = consumer_error {
            return NativeStreamingOutputFilesOutcome::ConsumerError { error, stderr };
        }
        NativeStreamingOutputFilesOutcome::Ok { stderr }
    }

    /// Evaluates one or more Pkl expressions against a loaded module using the full
    /// `NativeHost` (used by `PklProject`, `settings.pkl`, and `pkl:test` runners).
    /// Returns `(values, captured_logs)`.
    pub fn evaluate_expressions(
        &self,
        uri: &str,
        text: &str,
        inferred_name: &str,
        expressions: &[String],
    ) -> Result<(Vec<Value>, String), String> {
        let start = Instant::now();
        let now_ms = || {
            if self.timeout_ms.is_some() {
                start.elapsed().as_millis().min(u64::MAX as u128) as u64
            } else {
                0
            }
        };
        let watchdog = TimeoutWatchdog::start(
            self.timeout_ms,
            self.cancel_flag.clone(),
            self.external_readers.clone(),
        );

        let mut session = SourceSession::compile_expressions_text(
            SourceText::from(text),
            PklString::from(uri),
            PklString::from(inferred_name),
            expressions,
            self.session_options(DiagnosticProfile::CliBundledSettings),
        )
        .map_err(|e| e.to_java_utf8())?
        .with_color(self.use_color);

        let mut logs = Vec::new();
        let mut values = Vec::with_capacity(expressions.len());
        for idx in 0..expressions.len() {
            if let Err(err) = session.start_path(idx, now_ms(), self.timeout_ms) {
                let timed_out = watchdog.stop();
                let msg = if timed_out || err.kind == ErrorKind::Timeout {
                    format_timeout_message(self.timeout_ms.unwrap_or(0))
                } else {
                    session.format_error(err)
                };
                session.close();
                return Err(msg);
            }
            match self.drive_source_session(&mut session, &now_ms, &watchdog, &mut logs) {
                Ok(val) => values.push(val),
                Err(err) => {
                    let timed_out = watchdog.stop();
                    let msg = if timed_out || err.kind == ErrorKind::Timeout {
                        format_timeout_message(self.timeout_ms.unwrap_or(0))
                    } else {
                        session.format_error(err)
                    };
                    session.close();
                    return Err(msg);
                }
            }
        }
        let _ = watchdog.stop();
        session.close();
        let log_str = String::from_utf8_lossy(&logs).into_owned();
        Ok((values, log_str))
    }

    /// Evaluates one or more Pkl expressions against a single compiled `SourceSession`,
    /// isolating per-expression runtime errors instead of aborting subsequent paths.
    /// Compilation/syntax errors or timeouts abort and return `Err(message)`.
    /// Returns `(per_expression_results, captured_logs)`.
    pub fn evaluate_expressions_partial(
        &self,
        uri: &str,
        text: &str,
        inferred_name: &str,
        expressions: &[String],
    ) -> Result<(Vec<Result<Value, String>>, String), String> {
        let start = Instant::now();
        let now_ms = || {
            if self.timeout_ms.is_some() {
                start.elapsed().as_millis().min(u64::MAX as u128) as u64
            } else {
                0
            }
        };
        let watchdog = TimeoutWatchdog::start(
            self.timeout_ms,
            self.cancel_flag.clone(),
            self.external_readers.clone(),
        );

        let mut session = SourceSession::compile_expressions_text(
            SourceText::from(text),
            PklString::from(uri),
            PklString::from(inferred_name),
            expressions,
            self.session_options(DiagnosticProfile::CliBundledSettings),
        )
        .map_err(|e| e.to_java_utf8())?
        .with_color(self.use_color);

        let mut logs = Vec::new();
        let mut results = Vec::with_capacity(expressions.len());
        for idx in 0..expressions.len() {
            if let Err(err) = session.start_path(idx, now_ms(), self.timeout_ms) {
                if watchdog.triggered() {
                    let _ = watchdog.stop();
                    session.close();
                    return Err(format_timeout_message(self.timeout_ms.unwrap_or(0)));
                }
                results.push(Err(session.format_error(err)));
                continue;
            }
            match self.drive_source_session(&mut session, &now_ms, &watchdog, &mut logs) {
                Ok(val) => results.push(Ok(val)),
                Err(err) => {
                    if watchdog.triggered() || err.kind == pkl_runtime::value::ErrorKind::Timeout {
                        let _ = watchdog.stop();
                        session.close();
                        return Err(format_timeout_message(self.timeout_ms.unwrap_or(0)));
                    }
                    results.push(Err(session.format_error(err)));
                }
            }
        }
        let _ = watchdog.stop();
        session.close();
        let log_str = String::from_utf8_lossy(&logs).into_owned();
        Ok((results, log_str))
    }

    fn remaining_timeout_ms(&self, start: Instant) -> Option<u64> {
        self.timeout_ms.map(|limit| {
            let elapsed = start.elapsed().as_millis().min(u64::MAX as u128) as u64;
            limit.saturating_sub(elapsed)
        })
    }

    fn load_primary_module_with_timeout(
        &self,
        raw_uri: &str,
        start: Instant,
        options: &ModuleEvalOptions,
    ) -> Result<PrimaryModuleSource, ModuleOutputResult> {
        let watchdog = TimeoutWatchdog::start(
            self.remaining_timeout_ms(start),
            self.cancel_flag.clone(),
            self.external_readers.clone(),
        );

        let resolved_uri_pkl = if raw_uri.starts_with('@') {
            match self.resolve_top_level_dependency_uri(raw_uri) {
                Ok(u) => u,
                Err(host_err) => {
                    let timed_out = watchdog.stop();
                    if timed_out {
                        return Err(self.format_timeout_output(self.timeout_ms.unwrap_or(0)));
                    }
                    return Err(self.render_primary_load_error(raw_uri, host_err, options));
                }
            }
        } else {
            PklString::from(raw_uri)
        };

        let load_res = self.resolve_primary_module(&resolved_uri_pkl);
        let timed_out = watchdog.stop();
        if timed_out {
            return Err(self.format_timeout_output(self.timeout_ms.unwrap_or(0)));
        }
        match load_res {
            Ok(primary) => Ok(primary),
            Err(host_err) => Err(self.render_primary_load_error(
                &resolved_uri_pkl.to_java_utf8(),
                host_err,
                options,
            )),
        }
    }

    /// Resolves and loads a primary module URI via `SelectKey -> ResolveKey -> LoadSource`.
    pub fn resolve_primary_module(
        &self,
        uri: &PklString,
    ) -> Result<PrimaryModuleSource, ModuleHostError> {
        let key_id = ModuleKeyId(1);
        let select_req = ModuleRequest {
            id: 1,
            kind: ModuleRequestKind::SelectKey {
                id: key_id,
                uri: uri.clone(),
                phase: pkl_runtime::modules::SelectionPhase::Primary,
            },
        };
        let key = match self.handle_module_request(&select_req) {
            ModuleCompletion::KeySelected(Ok(k)) => k,
            ModuleCompletion::KeySelected(Err(err)) => return Err(err),
            _ => return Err(ModuleHostError::NoLoader),
        };

        let resolved_key_id = pkl_runtime::modules::ResolvedKeyId(1);
        let resolve_req = ModuleRequest {
            id: 2,
            kind: ModuleRequestKind::ResolveKey {
                key: key.clone(),
                resolved_key_id,
                phase: pkl_runtime::modules::ResolutionPhase::Primary,
            },
        };
        let (resolved_key, resolved) = match self.handle_module_request(&resolve_req) {
            ModuleCompletion::KeyResolved(Ok(rk)) => {
                let m = rk.module.clone();
                (rk, m)
            }
            ModuleCompletion::Resolved(Ok(r)) => {
                let rk = ResolvedModuleKey {
                    id: resolved_key_id,
                    key: key.clone(),
                    handle: key.handle.clone(),
                    module: r.clone(),
                };
                (rk, r)
            }
            ModuleCompletion::Resolved(Err(err)) | ModuleCompletion::KeyResolved(Err(err)) => {
                return Err(err);
            }
            _ => return Err(ModuleHostError::NoLoader),
        };

        let load_req = ModuleRequest {
            id: 3,
            kind: ModuleRequestKind::LoadSource {
                key: resolved_key,
                resolved: resolved.clone(),
            },
        };
        let source = match self.handle_module_request(&load_req) {
            ModuleCompletion::Source(Ok(s)) => s,
            ModuleCompletion::Source(Err(err)) => return Err(err),
            _ => return Err(ModuleHostError::NotFound),
        };

        let inferred_name = if source.inferred_name.is_empty() {
            infer_name_for_uri(&resolved.original_uri.to_java_utf8())
        } else {
            source.inferred_name
        };

        Ok(PrimaryModuleSource {
            original_uri: resolved.original_uri,
            resolved_uri: resolved.resolved_uri,
            text: source.text,
            inferred_name,
        })
    }

    /// Replays a primary-module `ModuleHostError` through `SourceSession::module_evaluator`
    /// with ZERO additional I/O so `pkl-runtime` renders the 100%-exact Java 0.32.0 error format.
    fn render_primary_load_error(
        &self,
        uri: &str,
        host_err: ModuleHostError,
        options: &ModuleEvalOptions,
    ) -> ModuleOutputResult {
        let mut session =
            SourceSession::module_evaluator(self.session_options(options.diagnostic_profile))
                .with_color(self.use_color);
        let spec = ModuleSourceSpec {
            uri: PklString::from(uri),
            contents: None,
        };
        if let Err(err) = session.start_evaluation(spec, "output.text", 0, None) {
            let res: ModuleOutputResult = session.output_failure(err).into();
            session.close();
            return res;
        }
        let mut injected = Some(host_err);
        let result = loop {
            match session.poll(0, 1024) {
                HostPoll::Ready(res) => break res,
                HostPoll::Yielded => {}
                HostPoll::Module { token, request } => {
                    let completion = match &request.kind {
                        ModuleRequestKind::SelectKey { id, uri, .. } => {
                            if matches!(
                                injected,
                                Some(
                                    ModuleHostError::NoLoader
                                        | ModuleHostError::UriSyntax { .. }
                                        | ModuleHostError::ExternalReaderProcess { .. }
                                )
                            ) {
                                ModuleCompletion::KeySelected(Err(injected
                                    .take()
                                    .unwrap_or(ModuleHostError::NoLoader)))
                            } else {
                                ModuleCompletion::KeySelected(Ok(ModuleKey {
                                    id: *id,
                                    uri: uri.clone(),
                                    kind: ModuleKeyKind::Host,
                                    handle: "primary-error-replay".into(),
                                }))
                            }
                        }
                        ModuleRequestKind::QueryCachePolicy { .. } => {
                            ModuleCompletion::CachePolicy(Ok(true))
                        }
                        ModuleRequestKind::Resolve { .. } => {
                            ModuleCompletion::Resolved(Err(injected
                                .take()
                                .unwrap_or(ModuleHostError::NoLoader)))
                        }
                        ModuleRequestKind::ResolveKey { .. } => {
                            ModuleCompletion::KeyResolved(Err(injected
                                .take()
                                .unwrap_or(ModuleHostError::NoLoader)))
                        }
                        ModuleRequestKind::LoadSource { .. } => {
                            ModuleCompletion::Source(Err(injected
                                .take()
                                .unwrap_or(ModuleHostError::NotFound)))
                        }
                        _ => ModuleCompletion::Resolved(Err(injected
                            .take()
                            .unwrap_or(ModuleHostError::NoLoader))),
                    };
                    if let Err(err) = session.complete_module(&token, completion, 0) {
                        break Err(err);
                    }
                }
                HostPoll::TransformFrame { token, frame } => {
                    let transformed = self.transform_log_frame((*frame).clone());
                    if let Err(err) = session.complete_frame_transform(&token, Ok(transformed), 0) {
                        break Err(err);
                    }
                }
                HostPoll::Log { token, .. } => {
                    if let Err(err) = session.acknowledge_log(&token, Ok(()), 0) {
                        break Err(err);
                    }
                }
                _ => break Err(RuntimeError::new(ErrorKind::Type, "unexpected host poll")),
            }
        };
        let output: ModuleOutputResult = match result {
            Ok(_) => ModuleOutputResult {
                phase: FrontendPhase::ModuleOutput,
                disposition: None,
                reason: None,
                stdout: Vec::new(),
                stderr: Vec::new(),
                exit_code: 0,
            },
            Err(err) => session.output_failure(err).into(),
        };
        session.close();
        output
    }

    fn run_compiled_module_output(
        &self,
        input: LosslessModuleSourceInput,
        expression: Option<&str>,
        options: &ModuleEvalOptions,
        start: Instant,
    ) -> (ModuleOutputResult, Vec<String>, Vec<String>, Vec<String>) {
        let session_opts = self.session_options(options.diagnostic_profile);
        let mut session = match expression {
            Some(expr) => match SourceSession::compile_output_expression_text_colored(
                input,
                expr,
                options,
                session_opts,
                self.use_color,
            ) {
                Ok(s) => s,
                Err(error) => {
                    let res: pkl_runtime::evaluation::frontend::ModuleFrontendResult = error.into();
                    return (res.into(), Vec::new(), Vec::new(), Vec::new());
                }
            },
            None => match SourceSession::compile_output_colored(
                input,
                &[],
                options,
                session_opts,
                self.use_color,
            ) {
                Ok(s) => s,
                Err(error) => return (error.into(), Vec::new(), Vec::new(), Vec::new()),
            },
        };

        let remaining_ms = self.remaining_timeout_ms(start);
        if let Some(limit) = self.timeout_ms
            && remaining_ms == Some(0)
        {
            session.close();
            return (
                self.format_timeout_output(limit),
                Vec::new(),
                Vec::new(),
                Vec::new(),
            );
        }

        let watchdog = TimeoutWatchdog::start(
            remaining_ms,
            self.cancel_flag.clone(),
            self.external_readers.clone(),
        );
        let now_ms = || {
            if self.timeout_ms.is_some() {
                start.elapsed().as_millis().min(u64::MAX as u128) as u64
            } else {
                0
            }
        };
        let mut stderr = Vec::new();

        let run = (|| -> Result<Vec<u8>, RuntimeError> {
            if expression.is_some() {
                session.start_path(0, now_ms(), self.timeout_ms)?;
                let val =
                    self.drive_source_session(&mut session, &now_ms, &watchdog, &mut stderr)?;
                SourceSession::result_string(val).map(|s| s.to_java_utf8().into_bytes())
            } else {
                session.start_output_bytes(now_ms(), self.timeout_ms)?;
                let val =
                    self.drive_source_session(&mut session, &now_ms, &watchdog, &mut stderr)?;
                SourceSession::result_bytes(val).map(|b| b.to_vec())
            }
        })();

        let timed_out = watchdog.stop();
        let mut result: ModuleOutputResult = match run {
            Ok(bytes) => ModuleOutputResult {
                phase: FrontendPhase::ModuleOutput,
                disposition: None,
                reason: None,
                stdout: bytes,
                stderr: Vec::new(),
                exit_code: 0,
            },
            Err(error) => {
                if timed_out || error.kind == ErrorKind::Timeout {
                    self.format_timeout_output(self.timeout_ms.unwrap_or(0))
                } else if expression.is_some() {
                    session.output_expression_failure(error).into()
                } else {
                    session.output_failure(error).into()
                }
            }
        };
        stderr.extend_from_slice(&result.stderr);
        result.stderr = stderr;

        let cancelled_reads = session.drain_cancelled_reads();
        let cancelled_modules = session.drain_cancelled_modules();
        let cancelled_globs = session.drain_cancelled_globs();
        session.close();
        (result, cancelled_reads, cancelled_modules, cancelled_globs)
    }

    fn drive_source_session(
        &self,
        session: &mut SourceSession,
        now_ms: &dyn Fn() -> u64,
        watchdog: &TimeoutWatchdog,
        stderr: &mut Vec<u8>,
    ) -> Result<Value, RuntimeError> {
        let has_timeout = self.timeout_ms.is_some();
        let step_budget = if has_timeout { 4096 } else { 1_048_576 };
        let base_clock = session.host_session().last_clock();
        let current_ms = || if has_timeout { now_ms() } else { base_clock };
        loop {
            if self.cancel_flag.is_cancelled() && !watchdog.triggered() {
                session.cancel();
            }
            match session.poll(current_ms(), step_budget) {
                HostPoll::Ready(result) => return result,
                HostPoll::Yielded => {}
                HostPoll::Read { token, .. } => {
                    let req = session.read_request(&token)?;
                    let result = self.handle_read_request(&req);
                    let t = current_ms();
                    if self.cancel_flag.is_cancelled() && !watchdog.triggered() {
                        session.cancel();
                    }
                    session.complete_read(&token, result, t)?;
                }
                HostPoll::Module { token, request } => {
                    let completion = self.handle_module_request(&request);
                    let t = current_ms();
                    if self.cancel_flag.is_cancelled() && !watchdog.triggered() {
                        session.cancel();
                    }
                    session.complete_module(&token, completion, t)?;
                }
                HostPoll::Glob { token, request } => {
                    let completion = self.handle_glob_request(&request);
                    let t = current_ms();
                    if self.cancel_flag.is_cancelled() && !watchdog.triggered() {
                        session.cancel();
                    }
                    session.complete_glob(&token, completion, t)?;
                }
                HostPoll::TransformFrame { token, frame } => {
                    let transformed = self.transform_log_frame((*frame).clone());
                    session.complete_frame_transform(&token, Ok(transformed), current_ms())?;
                }
                HostPoll::Log { token, event } => {
                    self.emit_log_event(&event, stderr);
                    session.acknowledge_log(&token, Ok(()), current_ms())?;
                }
            }
        }
    }

    /// Standalone native `Analyzer.importGraph` API backed by `ImportGraphAnalysis` + `NativeHost`.
    pub fn analyze_import_graph(
        &self,
        root_uris: impl IntoIterator<Item = PklString>,
    ) -> Result<ImportGraph, Box<AnalysisFailure>> {
        let mut analysis = ImportGraphAnalysis::new(root_uris);
        let result = loop {
            match analysis.poll() {
                AnalysisPoll::Ready(res) => break res,
                AnalysisPoll::Closed => {
                    break Err(Box::new(AnalysisFailure {
                        phase: "closed",
                        module_uri: PklString::default(),
                        site: None,
                        cause: pkl_runtime::modules::analyze::FailureCause::Unsupported(
                            "Analysis closed unexpectedly.".into(),
                        ),
                        traversal_order_qualified: true,
                    }));
                }
                AnalysisPoll::Request(request) => {
                    let completion = self.handle_module_request(&request);
                    if let Err(msg) = analysis.complete(request.id, completion) {
                        break Err(Box::new(AnalysisFailure {
                            phase: "host",
                            module_uri: PklString::default(),
                            site: None,
                            cause: pkl_runtime::modules::analyze::FailureCause::Unsupported(
                                msg.into(),
                            ),
                            traversal_order_qualified: true,
                        }));
                    }
                }
            }
        };
        analysis.close();
        result
    }

    // =========================================================================
    // Session-Owned Package & Project Dependency Resolution (Zero thread_local!)
    // =========================================================================

    fn resolve_top_level_dependency_uri(
        &self,
        authored: &str,
    ) -> Result<PklString, ModuleHostError> {
        let Some(ref mgr) = self.project_deps else {
            return Err(ModuleHostError::SecurityManagerException {
                message: Some(
                    "Cannot resolve dependency because there is no project found.\n\nIf you meant to import a path that starts with `@`, prefix the path with `./` (e.g. `import \"./@myPath\").\nIf you meant to import a dependency, ensure that this file is within a directory that contains a PklProject module."
                        .into(),
                ),
            });
        };
        let stripped = authored.strip_prefix('@').unwrap_or(authored);
        let (dep_name, asset_path) = match stripped.find('/') {
            Some(idx) => (&stripped[..idx], &stripped[idx..]),
            None => (stripped, "/"),
        };
        let deps = mgr.get_dependencies()?;
        let Some(entry) = deps.get(dep_name) else {
            return Err(ModuleHostError::SecurityManagerException {
                message: Some(
                    format!(
                        "Cannot find a dependency named `{dep_name}`, because it is not declared in the current project.\n\nTo fix this, add it to the `dependencies` section of your `PklProject` file, and resolve your dependencies."
                    )
                    .into(),
                ),
            });
        };
        let base = entry
            .package_uri()
            .to_project_package_uri()
            .without_checksums();
        let encoded = percent_encode_fragment(asset_path);
        Ok(format!("{}#{encoded}", base.uri).into())
    }

    fn resolve_package_or_dep_import_uri(
        &self,
        base_uri: &PklString,
        base_is_hierarchical: bool,
        authored_uri: &PklString,
    ) -> Result<PklString, ModuleHostError> {
        let base_str = base_uri.as_str().unwrap_or("");
        let authored_str = authored_uri.as_str().unwrap_or("");

        // 1. `@dep/...` dependency notation
        if let Some(stripped) = authored_str.strip_prefix('@') {
            let (dep_name, asset_path) = match stripped.find('/') {
                Some(idx) => (&stripped[..idx], &stripped[idx..]),
                None => (stripped, "/"),
            };

            // 1a. Importer is a `package:` or `projectpackage:` module
            if base_str.starts_with("package:") || base_str.starts_with("projectpackage:") {
                let asset_uri =
                    PackageAssetUri::parse(base_uri.clone()).map_err(map_package_uri_error)?;
                let encoded_asset = percent_encode_fragment(asset_path);

                if base_str.starts_with("projectpackage:")
                    && let Some(ref mgr) = self.project_deps
                {
                    // Local project dependency inside `PklProject.deps.json`
                    if let Some(local_deps) = mgr.get_local_package_dependencies(&asset_uri.package)
                    {
                        if let Some(dep_entry) = local_deps.get(dep_name) {
                            let target = dep_entry
                                .package_uri()
                                .to_project_package_uri()
                                .without_checksums();
                            return Ok(format!("{}#{encoded_asset}", target.uri).into());
                        }
                        let clean_pkg = asset_uri.package.display_name();
                        return Err(ModuleHostError::SecurityManagerException {
                            message: Some(
                                format!(
                                    "Cannot find dependency named `{dep_name}`, because it was not declared in package `{clean_pkg}`."
                                )
                                .into(),
                            ),
                        });
                    }

                    // Remote project dependency -> fetch metadata with lockfile checksum & remap via ProjectDependenciesManager
                    let resolved_dep = mgr.get_resolved_dependency(&asset_uri.package)?;
                    let expected_sha = match resolved_dep {
                        ProjectDepEntry::Remote { sha256, .. } => Some(sha256),
                        ProjectDepEntry::Local { .. } => None,
                    };
                    let meta = self
                        .package_resolver
                        .get_dependency_metadata(&asset_uri.package, expected_sha)?;
                    let mapped = mgr.resolve_package_dependencies(&asset_uri.package, &meta)?;
                    if let Some(dep_entry) = mapped.get(dep_name) {
                        let target = dep_entry
                            .package_uri()
                            .to_project_package_uri()
                            .without_checksums();
                        return Ok(format!("{}#{encoded_asset}", target.uri).into());
                    }
                    let clean_pkg = asset_uri.package.display_name();
                    return Err(ModuleHostError::SecurityManagerException {
                        message: Some(
                            format!(
                                "Cannot find dependency named `{dep_name}`, because it was not declared in package `{clean_pkg}`."
                            )
                            .into(),
                        ),
                    });
                }

                // External `package:` (or `projectpackage:` without project): metadata-only resolution
                let meta = self
                    .package_resolver
                    .get_dependency_metadata(&asset_uri.package, None)?;
                for dep in &meta.dependencies {
                    if dep.name == dep_name {
                        let target_uri = dep.package_uri.uri.as_str().unwrap_or("");
                        return Ok(format!("{target_uri}#{encoded_asset}").into());
                    }
                }
                let clean_pkg = asset_uri.package.display_name();
                return Err(ModuleHostError::SecurityManagerException {
                    message: Some(
                        format!(
                            "Cannot find dependency named `{dep_name}`, because it was not declared in package `{clean_pkg}`."
                        )
                        .into(),
                    ),
                });
            }

            // 1b. Importer is a local project file (or non-hierarchical module)
            if !base_is_hierarchical {
                return Err(ModuleHostError::SecurityManagerException {
                    message: Some(
                        format!(
                            "Cannot resolve dependency because project URI `{base_str}` does not have a hierarchical path."
                        )
                        .into(),
                    ),
                });
            }
            let Some(ref mgr) = self.project_deps else {
                return Err(ModuleHostError::SecurityManagerException {
                    message: Some(
                        "Cannot resolve dependency because there is no project found.\n\nIf you meant to import a path that starts with `@`, prefix the path with `./` (e.g. `import \"./@myPath\").\nIf you meant to import a dependency, ensure that this file is within a directory that contains a PklProject module."
                            .into(),
                    ),
                });
            };
            let (entry, resolved_asset) =
                mgr.resolve_dependency_notation(base_str, base_is_hierarchical, authored_str)?;
            let target = entry
                .package_uri()
                .to_project_package_uri()
                .without_checksums();
            let encoded = percent_encode_fragment(&resolved_asset);
            return Ok(format!("{}#{encoded}", target.uri).into());
        }

        // 2. Relative fragment path inside `package:` or `projectpackage:`
        if (base_str.starts_with("package:") || base_str.starts_with("projectpackage:"))
            && !has_uri_scheme(authored_str)
        {
            let asset_uri =
                PackageAssetUri::parse(base_uri.clone()).map_err(map_package_uri_error)?;
            let base_fragment = asset_uri.asset_path.as_str().unwrap_or("/");
            let new_fragment = resolve_relative_path(base_fragment, authored_str);
            let encoded_fragment = percent_encode_fragment(&new_fragment);
            let pkg_uri = asset_uri.package.uri.as_str().unwrap_or("");
            return Ok(format!("{pkg_uri}#{encoded_fragment}").into());
        }

        pkl_runtime::host::resolve_module_key_uri(authored_uri, base_uri)
            .map_err(|err| ModuleHostError::Unsupported(err.message))
    }

    fn resolve_projectpackage_entry(
        &self,
        asset_uri: &PackageAssetUri,
    ) -> Result<ProjectDepEntry, ModuleHostError> {
        let Some(ref mgr) = self.project_deps else {
            return Err(ModuleHostError::SecurityManagerException {
                message: Some(
                    format!(
                        "Did not find a resolved dependency for package `{}`.\n\nRun `pkl project resolve` to update resolved dependencies.",
                        asset_uri.package.uri
                    )
                    .into(),
                ),
            });
        };
        mgr.get_resolved_dependency(&asset_uri.package).cloned()
    }

    fn resolve_package_module_key(
        &self,
        uri: &PklString,
    ) -> Result<ResolvedModule, ModuleHostError> {
        self.check_resolve_module(uri)?;
        let raw_uri = uri.as_str().unwrap_or("");
        let asset_uri = PackageAssetUri::parse(uri.clone()).map_err(map_package_uri_error)?;

        if raw_uri.starts_with("projectpackage:") {
            let entry = self.resolve_projectpackage_entry(&asset_uri)?;
            match entry {
                ProjectDepEntry::Local { path, .. } => {
                    let mgr = self.project_deps.as_ref().expect("project_deps checked");
                    let file_path = mgr.resolve_local_asset_path(
                        &path,
                        asset_uri.asset_path.as_str().unwrap_or(""),
                    );
                    let bytes = read_local_module_bytes(&file_path)?;
                    // Mirror Java `LocalDependency.resolveAssetUri` (`Dependency.java:64`):
                    // `projectBaseUri.resolve(relativeUri)` produces single-slash `file:/abs/path`.
                    let full_uri = file_uri_from_path(&file_path)?;
                    let single_slash = full_uri
                        .strip_prefix("file:///")
                        .map(|rest| format!("file:/{rest}"))
                        .unwrap_or(full_uri);
                    let resolved_uri = PklString::from(single_slash);
                    self.stage_source(&resolved_uri, bytes.clone());
                    self.stage_source(uri, bytes);
                    return Ok(ResolvedModule {
                        original_uri: uri.clone(),
                        resolved_uri,
                        source_uri: uri.clone(),
                    });
                }
                ProjectDepEntry::Remote { sha256, .. } => {
                    let bytes =
                        self.package_resolver
                            .get_bytes(&asset_uri, false, Some(&sha256))?;
                    self.stage_source(uri, bytes);
                    return Ok(ResolvedModule {
                        original_uri: uri.clone(),
                        resolved_uri: uri.clone(),
                        source_uri: uri.clone(),
                    });
                }
            }
        }

        let bytes = self.package_resolver.get_bytes(&asset_uri, false, None)?;
        self.stage_source(uri, bytes);
        Ok(ResolvedModule {
            original_uri: uri.clone(),
            resolved_uri: uri.clone(),
            source_uri: uri.clone(),
        })
    }

    fn read_package_resource(&self, resolved_uri: &PklString) -> Result<ReadResult, ReadFailure> {
        self.check_read_resource(resolved_uri)?;

        let raw_uri = resolved_uri.as_str().unwrap_or("");
        let asset_uri = PackageAssetUri::parse(resolved_uri.clone())
            .map_err(|err| module_err_to_read_failure(map_package_uri_error(err)))?;

        let expected_sha = if raw_uri.starts_with("projectpackage:") {
            let entry = self
                .resolve_projectpackage_entry(&asset_uri)
                .map_err(module_err_to_read_failure)?;
            match entry {
                ProjectDepEntry::Local { path, .. } => {
                    let mgr = self.project_deps.as_ref().expect("project_deps checked");
                    let file_path = mgr.resolve_local_asset_path(
                        &path,
                        asset_uri.asset_path.as_str().unwrap_or(""),
                    );
                    return read_local_project_resource(&file_path, resolved_uri);
                }
                ProjectDepEntry::Remote { sha256, .. } => Some(sha256),
            }
        } else {
            None
        };

        match self
            .package_resolver
            .get_bytes(&asset_uri, true, expected_sha.as_ref())
        {
            Ok(bytes) => Ok(ReadResult::Resource(Resource {
                uri: resolved_uri.clone(),
                bytes: bytes.into(),
            })),
            Err(ModuleHostError::NotFound) => Ok(ReadResult::Missing),
            Err(ModuleHostError::Io(ref io))
                if io.message.as_ref().and_then(PklString::as_str) == Some("Is a directory") =>
            {
                let mut elements = self
                    .package_resolver
                    .list_elements(&asset_uri, expected_sha.as_ref())
                    .map_err(module_err_to_read_failure)?;
                elements.sort_by_key(|a| a.name.to_java_utf8());
                let mut listing = String::new();
                for elem in elements {
                    listing.push_str(&elem.name.to_java_utf8());
                    listing.push('\n');
                }
                Ok(ReadResult::Resource(Resource {
                    uri: resolved_uri.clone(),
                    bytes: listing.into_bytes().into(),
                }))
            }
            Err(err) => Err(module_err_to_read_failure(err)),
        }
    }

    fn package_has_element(
        &self,
        uri: &PklString,
        is_resource: bool,
    ) -> Result<bool, ModuleHostError> {
        if is_resource {
            self.check_resolve_resource_for_glob(uri)?;
        } else {
            self.check_resolve_module(uri)?;
        }
        let raw_uri = uri.as_str().unwrap_or("");
        let asset_uri = PackageAssetUri::parse(uri.clone()).map_err(map_package_uri_error)?;
        let expected_sha = if raw_uri.starts_with("projectpackage:") {
            match self.resolve_projectpackage_entry(&asset_uri)? {
                ProjectDepEntry::Local { path, .. } => {
                    let mgr = self.project_deps.as_ref().expect("project_deps checked");
                    let file_path = mgr.resolve_local_asset_path(
                        &path,
                        asset_uri.asset_path.as_str().unwrap_or(""),
                    );
                    return Ok(file_path.exists());
                }
                ProjectDepEntry::Remote { sha256, .. } => Some(sha256),
            }
        } else {
            None
        };
        self.package_resolver
            .has_element(&asset_uri, expected_sha.as_ref())
    }

    fn package_list_elements(
        &self,
        uri: &PklString,
        is_resource: bool,
    ) -> Result<Vec<PathElement>, ModuleHostError> {
        if is_resource {
            self.check_resolve_resource_for_glob(uri)?;
        } else {
            self.check_resolve_module(uri)?;
        }
        let raw_uri = uri.as_str().unwrap_or("");
        let asset_uri = PackageAssetUri::parse(uri.clone()).map_err(map_package_uri_error)?;
        let expected_sha = if raw_uri.starts_with("projectpackage:") {
            match self.resolve_projectpackage_entry(&asset_uri)? {
                ProjectDepEntry::Local { path, .. } => {
                    let mgr = self.project_deps.as_ref().expect("project_deps checked");
                    let dir_path = mgr.resolve_local_asset_path(
                        &path,
                        asset_uri.asset_path.as_str().unwrap_or(""),
                    );
                    return list_local_dir_elements(&dir_path);
                }
                ProjectDepEntry::Remote { sha256, .. } => Some(sha256),
            }
        } else {
            None
        };
        self.package_resolver
            .list_elements(&asset_uri, expected_sha.as_ref())
    }

    fn custom_module_reader_by_handle(
        &self,
        handle: &PklString,
    ) -> Option<&Arc<dyn CustomModuleReader>> {
        let idx = handle
            .as_str()
            .and_then(|s| s.strip_prefix("custom:module:"))
            .and_then(|n| n.parse::<usize>().ok())?;
        self.custom_module_readers.get(idx)
    }

    fn find_custom_module_reader_by_uri(
        &self,
        uri: &PklString,
    ) -> Option<(usize, &Arc<dyn CustomModuleReader>)> {
        self.custom_module_readers
            .iter()
            .enumerate()
            .find(|(_, reader)| reader.matches_uri(uri))
    }

    fn find_custom_resource_reader(
        &self,
        scheme: &str,
    ) -> Option<(usize, &Arc<dyn CustomResourceReader>)> {
        self.custom_resource_readers
            .iter()
            .enumerate()
            .rev()
            .find(|(_, reader)| reader.scheme().eq_ignore_ascii_case(scheme))
    }

    fn custom_resource_reader_by_handle(
        &self,
        handle: &PklString,
    ) -> Option<&Arc<dyn CustomResourceReader>> {
        let idx = handle
            .as_str()
            .and_then(|s| s.strip_prefix("custom:resource:"))
            .and_then(|n| n.parse::<usize>().ok())?;
        self.custom_resource_readers.get(idx)
    }

    fn stage_custom_source(&self, resolved: &ResolvedModule, source: ModuleSource) {
        let mut staged = self
            .staged_custom_sources
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        staged.insert(resolved.resolved_uri.clone(), source.clone());
        if resolved.original_uri != resolved.resolved_uri {
            staged.insert(resolved.original_uri.clone(), source);
        }
    }

    fn try_handle_custom_module_request(
        &self,
        request: &ModuleRequest,
    ) -> Option<ModuleCompletion> {
        if self.custom_module_readers.is_empty() {
            return None;
        }
        match &request.kind {
            ModuleRequestKind::SelectKey { id, uri, .. } => {
                let (idx, _) = self.find_custom_module_reader_by_uri(uri)?;
                Some(ModuleCompletion::KeySelected(Ok(ModuleKey {
                    id: *id,
                    uri: uri.clone(),
                    kind: ModuleKeyKind::Host,
                    handle: format!("custom:module:{idx}").into(),
                })))
            }
            ModuleRequestKind::Capabilities { key, module } => {
                let reader = key
                    .as_ref()
                    .and_then(|k| self.custom_module_reader_by_handle(&k.handle))
                    .or_else(|| {
                        let uri = key.as_ref().map(|k| &k.uri).unwrap_or(&module.original_uri);
                        self.find_custom_module_reader_by_uri(uri).map(|(_, r)| r)
                    })?;
                let uri = key.as_ref().map(|k| &k.uri).unwrap_or(&module.original_uri);
                Some(ModuleCompletion::Capabilities(Ok(ModuleCapabilities {
                    local: reader.is_local(uri),
                    hierarchical: reader.is_hierarchical(uri),
                    fragment_paths: reader.has_fragment_paths(uri),
                })))
            }
            ModuleRequestKind::QueryCachePolicy { key } => {
                let reader = self
                    .custom_module_reader_by_handle(&key.handle)
                    .or_else(|| {
                        self.find_custom_module_reader_by_uri(&key.uri)
                            .map(|(_, r)| r)
                    })?;
                Some(ModuleCompletion::CachePolicy(
                    Ok(reader.is_cached(&key.uri)),
                ))
            }
            ModuleRequestKind::Resolve { key, .. } => {
                let reader = self.custom_module_reader_by_handle(&key.handle)?;
                if let Err(err) = self.check_resolve_module(&key.uri) {
                    return Some(ModuleCompletion::Resolved(Err(err)));
                }
                match reader.resolve_module(&key.uri) {
                    Ok((resolved, source)) => {
                        self.stage_custom_source(&resolved, source);
                        Some(ModuleCompletion::Resolved(Ok(resolved)))
                    }
                    Err(err) => Some(ModuleCompletion::Resolved(Err(err))),
                }
            }
            ModuleRequestKind::ResolveKey {
                key,
                resolved_key_id,
                ..
            } => {
                let reader = self.custom_module_reader_by_handle(&key.handle)?;
                if let Err(err) = self.check_resolve_module(&key.uri) {
                    return Some(ModuleCompletion::KeyResolved(Err(err)));
                }
                match reader.resolve_module(&key.uri) {
                    Ok((resolved, source)) => {
                        self.stage_custom_source(&resolved, source);
                        Some(ModuleCompletion::KeyResolved(Ok(ResolvedModuleKey {
                            id: *resolved_key_id,
                            key: key.clone(),
                            handle: key.handle.clone(),
                            module: resolved,
                        })))
                    }
                    Err(err) => Some(ModuleCompletion::KeyResolved(Err(err))),
                }
            }
            ModuleRequestKind::LoadSource { key, resolved } => {
                if self.custom_module_reader_by_handle(&key.handle).is_none()
                    && self
                        .custom_module_reader_by_handle(&key.key.handle)
                        .is_none()
                {
                    return None;
                }
                let mut staged = self
                    .staged_custom_sources
                    .lock()
                    .unwrap_or_else(|e| e.into_inner());
                let from_resolved = staged.remove(&resolved.resolved_uri);
                let from_original = if resolved.original_uri != resolved.resolved_uri {
                    staged.remove(&resolved.original_uri)
                } else {
                    None
                };
                if let Some(src) = from_resolved.or(from_original) {
                    return Some(ModuleCompletion::Source(Ok(src)));
                }
                drop(staged);
                let reader = self
                    .custom_module_reader_by_handle(&key.handle)
                    .or_else(|| self.custom_module_reader_by_handle(&key.key.handle))?;
                match reader.resolve_module(&resolved.original_uri) {
                    Ok((_, src)) => Some(ModuleCompletion::Source(Ok(src))),
                    Err(err) => Some(ModuleCompletion::Source(Err(err))),
                }
            }
            ModuleRequestKind::HasElement {
                key,
                module,
                element_uri,
            } => {
                let reader = key
                    .as_ref()
                    .and_then(|k| self.custom_module_reader_by_handle(&k.handle))
                    .or_else(|| {
                        self.find_custom_module_reader_by_uri(element_uri)
                            .or_else(|| self.find_custom_module_reader_by_uri(&module.original_uri))
                            .map(|(_, r)| r)
                    })?;
                if let Err(err) = self.check_resolve_module(element_uri) {
                    return Some(ModuleCompletion::HasElement(Err(err)));
                }
                Some(ModuleCompletion::HasElement(
                    reader.has_element(element_uri),
                ))
            }
            ModuleRequestKind::ResolveImportUri { key, authored_uri } => {
                let reader = self.custom_module_reader_by_handle(&key.handle)?;
                if authored_uri.starts_with("@") {
                    return Some(ModuleCompletion::ImportUri(
                        self.resolve_package_or_dep_import_uri(
                            &key.uri,
                            reader.is_hierarchical(&key.uri),
                            authored_uri,
                        ),
                    ));
                }
                if !has_uri_scheme(authored_uri.as_str().unwrap_or("")) {
                    if !reader.is_hierarchical(&key.uri) {
                        return Some(ModuleCompletion::ImportUri(Err(
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
                    return Some(ModuleCompletion::ImportUri(
                        pkl_runtime::host::resolve_module_key_uri(authored_uri, &key.uri)
                            .map_err(|err| ModuleHostError::Unsupported(err.message)),
                    ));
                }
                None
            }
            _ => None,
        }
    }

    fn try_handle_custom_glob_request(&self, request: &GlobRequest) -> Option<GlobCompletion> {
        if self.custom_module_readers.is_empty() && self.custom_resource_readers.is_empty() {
            return None;
        }
        match &request.kind {
            GlobRequestKind::SelectResource { id, uri } => {
                let raw = uri.to_java_utf8();
                let (scheme, _) = raw.split_once(':')?;
                let (idx, reader) = self.find_custom_resource_reader(scheme)?;
                Some(GlobCompletion::Resource(Ok(ResourceReader {
                    id: *id,
                    scheme: reader.scheme().into(),
                    handle: format!("custom:resource:{idx}").into(),
                })))
            }
            GlobRequestKind::IsGlobbable { reader } => match reader {
                GlobReader::Module(key) => {
                    let r = self
                        .custom_module_reader_by_handle(&key.handle)
                        .or_else(|| {
                            self.find_custom_module_reader_by_uri(&key.uri)
                                .map(|(_, r)| r)
                        })?;
                    Some(GlobCompletion::Globbable(Ok(r.is_globbable(&key.uri))))
                }
                GlobReader::Resource(res) => {
                    let r = self
                        .custom_resource_reader_by_handle(&res.handle)
                        .or_else(|| {
                            self.find_custom_resource_reader(res.scheme.as_str().unwrap_or(""))
                                .map(|(_, r)| r)
                        })?;
                    Some(GlobCompletion::Globbable(Ok(r.is_globbable())))
                }
            },
            GlobRequestKind::IsHierarchical { reader } => match reader {
                GlobReader::Module(key) => {
                    let r = self
                        .custom_module_reader_by_handle(&key.handle)
                        .or_else(|| {
                            self.find_custom_module_reader_by_uri(&key.uri)
                                .map(|(_, r)| r)
                        })?;
                    Some(GlobCompletion::Hierarchical(
                        Ok(r.is_hierarchical(&key.uri)),
                    ))
                }
                GlobReader::Resource(res) => {
                    let r = self
                        .custom_resource_reader_by_handle(&res.handle)
                        .or_else(|| {
                            self.find_custom_resource_reader(res.scheme.as_str().unwrap_or(""))
                                .map(|(_, r)| r)
                        })?;
                    Some(GlobCompletion::Hierarchical(Ok(r.has_hierarchical_uris())))
                }
            },
            GlobRequestKind::FragmentPaths { reader } => {
                let (_, r) =
                    self.find_custom_resource_reader(reader.scheme.as_str().unwrap_or(""))?;
                let _ = r;
                Some(GlobCompletion::FragmentPaths(Ok(false)))
            }
            GlobRequestKind::HasElement { reader, uri } => {
                let r = self
                    .custom_resource_reader_by_handle(&reader.handle)
                    .or_else(|| {
                        let scheme = uri
                            .as_str()
                            .and_then(|s| s.split_once(':'))
                            .map(|(s, _)| s)
                            .unwrap_or(reader.scheme.as_str().unwrap_or(""));
                        self.find_custom_resource_reader(scheme).map(|(_, r)| r)
                    })?;
                if let Err(err) = self.check_resolve_resource_for_glob(uri) {
                    return Some(GlobCompletion::HasElement(Err(err.into())));
                }
                Some(GlobCompletion::HasElement(
                    r.has_element(uri).map_err(Into::into),
                ))
            }
            GlobRequestKind::ListElements { reader, uri } => match reader {
                GlobReader::Module(key) => {
                    let r = self
                        .custom_module_reader_by_handle(&key.handle)
                        .or_else(|| self.find_custom_module_reader_by_uri(uri).map(|(_, r)| r))?;
                    if let Err(err) = self.check_resolve_module(uri) {
                        return Some(GlobCompletion::Elements(Err(err.into())));
                    }
                    Some(GlobCompletion::Elements(
                        r.list_elements(uri).map_err(Into::into),
                    ))
                }
                GlobReader::Resource(res) => {
                    let r = self
                        .custom_resource_reader_by_handle(&res.handle)
                        .or_else(|| {
                            let scheme = uri
                                .as_str()
                                .and_then(|s| s.split_once(':'))
                                .map(|(s, _)| s)
                                .unwrap_or(res.scheme.as_str().unwrap_or(""));
                            self.find_custom_resource_reader(scheme).map(|(_, r)| r)
                        })?;
                    if let Err(err) = self.check_resolve_resource_for_glob(uri) {
                        return Some(GlobCompletion::Elements(Err(err.into())));
                    }
                    Some(GlobCompletion::Elements(
                        r.list_elements(uri).map_err(Into::into),
                    ))
                }
            },
        }
    }

    // =========================================================================
    // Unified Host Effect Dispatchers (`Read`, `Module`, `Glob`)
    // =========================================================================

    /// Handles a VM [`ReadRequest`], honoring any pre-resolved URI (such as triple-dot
    /// `read(".../...")` resolved against enclosing module hierarchy) before falling back
    /// to authored URI resolution in [`Self::handle_read`].
    pub fn handle_read_request(&self, req: &ReadRequest) -> Result<ReadResult, ReadFailure> {
        if let Some(resolved_uri) = &req.resolved_uri {
            if self.internal_pkl_binary_eval
                && resolved_uri.as_str() == Some("prop:pkl.outputFormat")
                && req.module_uri.as_ref().and_then(|u| u.as_str()) == Some("pkl:base")
            {
                return Ok(ReadResult::Text(PklString::from("pkl-binary")));
            }
            return self.handle_resolved_read(resolved_uri.clone());
        }
        self.handle_read(&req.uri, req.module_uri.as_ref())
    }

    /// Handles a resource read request across custom readers, external readers, `@dep`,
    /// `package:`, `projectpackage:`, `http:`, `https:`, `env:`, `prop:`, `file:`,
    /// `modulepath:`, `jar:file:`.
    pub fn handle_read(
        &self,
        authored_uri: &PklString,
        module_uri: Option<&PklString>,
    ) -> Result<ReadResult, ReadFailure> {
        let authored_raw = authored_uri.to_java_utf8();

        if self.internal_pkl_binary_eval
            && authored_raw == "prop:pkl.outputFormat"
            && module_uri.and_then(|u| u.as_str()) == Some("pkl:base")
        {
            return Ok(ReadResult::Text(PklString::from("pkl-binary")));
        }

        // 0. `@dep/...` dependency notation or relative read from hierarchical custom/external module
        let resolved_uri = if authored_raw.starts_with('@') {
            let empty_base = PklString::default();
            let base = module_uri.unwrap_or(&empty_base);
            let base_str = base.as_str().unwrap_or("");
            let hierarchical = self
                .find_custom_module_reader_by_uri(base)
                .map(|(_, r)| r.is_hierarchical(base))
                .unwrap_or_else(|| base_str.contains("://") || base_str.starts_with("file:/"));
            self.resolve_package_or_dep_import_uri(base, hierarchical, authored_uri)
                .map_err(module_err_to_read_failure)?
        } else if !has_uri_scheme(&authored_raw)
            && let Some(base) = module_uri
            && let Some((_, custom_mod)) = self.find_custom_module_reader_by_uri(base)
        {
            if !custom_mod.is_hierarchical(base) {
                return Err(ReadFailure::EvalError {
                    message: format!(
                        "Cannot resolve relative resource URI `{authored_raw}` because enclosing module `{}` does not have hierarchical URIs.",
                        base.to_java_utf8()
                    )
                    .into(),
                    hint: None,
                });
            }
            pkl_runtime::host::resolve_module_key_uri(authored_uri, base).map_err(|err| {
                ReadFailure::EvalError {
                    message: err.message.clone(),
                    hint: err.hint.clone(),
                }
            })?
        } else if !has_uri_scheme(&authored_raw)
            && let Some(base) = module_uri
            && let Some((base_scheme, _)) = base.as_str().and_then(|s| s.split_once(':'))
            && self
                .external_readers
                .module_schemes()
                .iter()
                .any(|s| s.eq_ignore_ascii_case(base_scheme))
        {
            let cap_req = ModuleRequest {
                id: 0,
                kind: ModuleRequestKind::Capabilities {
                    key: None,
                    module: pkl_runtime::modules::ModuleIdentity {
                        original_uri: base.clone(),
                        resolved_uri: base.clone(),
                    },
                },
            };
            let is_hierarchical = match self
                .external_readers
                .try_complete_module(&cap_req, |uri| self.check_resolve_module(uri))
            {
                Some(ModuleCompletion::Capabilities(Ok(caps))) => caps.hierarchical,
                Some(ModuleCompletion::Capabilities(Err(err))) => {
                    return Err(module_err_to_read_failure(err));
                }
                _ => false,
            };
            if !is_hierarchical {
                return Err(ReadFailure::EvalError {
                    message: format!(
                        "Cannot resolve relative resource URI `{authored_raw}` because enclosing module `{}` does not have hierarchical URIs.",
                        base.to_java_utf8()
                    )
                    .into(),
                    hint: None,
                });
            }
            pkl_runtime::host::resolve_module_key_uri(authored_uri, base).map_err(|err| {
                ReadFailure::EvalError {
                    message: err.message.clone(),
                    hint: err.hint.clone(),
                }
            })?
        } else {
            pkl_runtime::host::resolve_resource_read_uri(authored_uri, module_uri).map_err(
                |err| ReadFailure::EvalError {
                    message: err.message.clone(),
                    hint: err.hint.clone(),
                },
            )?
        };

        self.handle_resolved_read(resolved_uri)
    }

    fn handle_resolved_read(&self, resolved_uri: PklString) -> Result<ReadResult, ReadFailure> {
        let raw = resolved_uri.to_java_utf8();
        let scheme = raw.split_once(':').map(|(s, _)| s).unwrap_or("");

        // 0b. Custom resource readers FIRST (highest precedence: client/SPI readers can override built-ins).
        if let Some((_, custom_reader)) = self.find_custom_resource_reader(scheme) {
            self.check_read_resource(&resolved_uri)?;
            return custom_reader.read(&resolved_uri);
        }

        // 1. Registered external resource readers (last-registration precedence in `ResourceManager`)
        if let Some(ext_result) = self
            .external_readers
            .try_complete_read(&resolved_uri, |uri| self.check_read_resource(uri))
        {
            return ext_result;
        }

        // 2. `package:` / `projectpackage:`
        if scheme.eq_ignore_ascii_case("package") || scheme.eq_ignore_ascii_case("projectpackage") {
            return self.read_package_resource(&resolved_uri);
        }

        // 3. HTTP / HTTPS: defer SecurityManager check to HttpClient (post-rewrite + per redirect hop).
        if scheme.eq_ignore_ascii_case("http") || scheme.eq_ignore_ascii_case("https") {
            let bytes = self.http.fetch_resource(&raw, |hop_uri| {
                let hop_pkl: PklString = hop_uri.into();
                match self.check_read_resource(&hop_pkl) {
                    Ok(()) => Ok(()),
                    Err(ReadFailure::SecurityManagerException { message }) => Err(message
                        .unwrap_or_else(|| {
                            format!("Refusing to read resource `{hop_uri}`.").into()
                        })),
                    Err(other) => Err(format!("{other:?}").into()),
                }
            })?;
            return Ok(match bytes {
                Some(b) => ReadResult::Resource(Resource {
                    uri: resolved_uri,
                    bytes: b.into(),
                }),
                None => ReadResult::Missing,
            });
        }

        if self.custom_security.is_some() {
            self.check_read_resource(&resolved_uri)?;
        }

        // 4. Scalar `env:` / `prop:` + SecurityPolicy check BEFORE `noResourceReaderRegistered` (`ResourceManager.java:105-118`).
        if let Some(scalar) = try_configured_scalar_read_with_policy(
            &resolved_uri,
            &self.properties,
            &self.environment,
            self.fs.policy(),
        )? {
            return Ok(scalar);
        }

        // 5. `file:`
        if scheme.eq_ignore_ascii_case("file") {
            return Ok(match self.fs.read_file_resource(&resolved_uri)? {
                Some(bytes) => ReadResult::Resource(Resource {
                    uri: resolved_uri,
                    bytes: bytes.into(),
                }),
                None => ReadResult::Missing,
            });
        }

        // 6. `modulepath:`
        if scheme.eq_ignore_ascii_case("modulepath") {
            return Ok(
                match self.modulepath.read_resource(&resolved_uri, &self.fs)? {
                    Some(bytes) => ReadResult::Resource(Resource {
                        uri: resolved_uri,
                        bytes: bytes.into(),
                    }),
                    None => ReadResult::Missing,
                },
            );
        }

        // 7. `jar:file:` (`UrlResource`)
        if scheme.eq_ignore_ascii_case("jar") {
            return Ok(match read_generic_jar_resource(&resolved_uri, &self.fs)? {
                Some(bytes) => ReadResult::Resource(Resource {
                    uri: resolved_uri,
                    bytes: bytes.into(),
                }),
                None => ReadResult::Missing,
            });
        }

        // 8. Unregistered scheme (already passed SecurityPolicy above -> `noResourceReaderRegistered`).
        Err(ReadFailure::EvalError {
            message: format!("No resource reader is registered for scheme `{scheme}`.").into(),
            hint: None,
        })
    }

    /// Handles a typed `ModuleRequest` across custom readers, external readers, `pkl:`,
    /// `package:`, `projectpackage:`, `file:`, `modulepath:`, `jar:file:`, `http:`, `https:`.
    pub fn handle_module_request(&self, request: &ModuleRequest) -> ModuleCompletion {
        // 0. Custom in-process / client module readers FIRST.
        if let Some(completion) = self.try_handle_custom_module_request(request) {
            return completion;
        }

        // 1. Registered external module readers (`CliCommand.kt:264-275`).
        if let Some(completion) = self
            .external_readers
            .try_complete_module(request, |uri| self.check_resolve_module(uri))
        {
            return completion;
        }

        // 2. `@dep/...`, `package:`, `projectpackage:` import URI resolution (session-owned, zero thread_local!).
        if let ModuleRequestKind::ResolveImportUri { key, authored_uri } = &request.kind
            && (authored_uri.starts_with("@")
                || key.uri.starts_with("package:")
                || key.uri.starts_with("projectpackage:"))
        {
            let uri_str = key.uri.as_str().unwrap_or("");
            let is_hierarchical = key.kind == ModuleKeyKind::StandardFile
                || key.kind == ModuleKeyKind::StandardHttp
                || uri_str.contains("://")
                || uri_str.starts_with("file:/")
                || uri_str.starts_with("modulepath:");
            return ModuleCompletion::ImportUri(self.resolve_package_or_dep_import_uri(
                &key.uri,
                is_hierarchical,
                authored_uri,
            ));
        }

        // 3. Check `jar:file:` -> `modulepath:` roundtrip on SelectKey BEFORE standard_keys.
        if let ModuleRequestKind::SelectKey { id, uri, .. } = &request.kind
            && uri
                .to_java_utf8()
                .to_ascii_lowercase()
                .starts_with("jar:file:")
        {
            match self.modulepath.modulepath_uri_for_jar(uri) {
                Ok(Some(mp_uri)) => {
                    return ModuleCompletion::KeySelected(Ok(ModuleKey {
                        id: *id,
                        uri: mp_uri,
                        kind: ModuleKeyKind::Host,
                        handle: "builtin:modulepath".into(),
                    }));
                }
                Ok(None) => {}
                Err(err) => return ModuleCompletion::KeySelected(Err(err)),
            }
        }

        // 4. Import trust-level check via configurable SecurityPolicy or CustomSecurityChecker.
        if let ModuleRequestKind::CheckImport {
            importer,
            import_uri,
        } = &request.kind
        {
            return ModuleCompletion::ImportChecked(
                self.check_import_module(&importer.original_uri, import_uri),
            );
        }

        // 5. Intercept `package:` / `projectpackage:` Resolve/ResolveKey/LoadSource/HasElement
        //    BEFORE `standard_key_completion_with_policy` so we use our session-owned
        //    `PackageResolver` + `ProjectDependenciesManager` (zero `thread_local!`).
        match &request.kind {
            ModuleRequestKind::Resolve { key, .. }
                if key.uri.starts_with("package:") || key.uri.starts_with("projectpackage:") =>
            {
                return ModuleCompletion::Resolved(self.resolve_package_module_key(&key.uri));
            }
            ModuleRequestKind::ResolveKey {
                key,
                resolved_key_id,
                ..
            } if key.uri.starts_with("package:") || key.uri.starts_with("projectpackage:") => {
                return ModuleCompletion::KeyResolved(
                    self.resolve_package_module_key(&key.uri)
                        .map(|module| ResolvedModuleKey {
                            id: *resolved_key_id,
                            key: key.clone(),
                            handle: PklString::default(),
                            module,
                        }),
                );
            }
            ModuleRequestKind::LoadSource { resolved, .. }
                if resolved.original_uri.starts_with("package:")
                    || resolved.original_uri.starts_with("projectpackage:")
                    || resolved.resolved_uri.starts_with("package:")
                    || resolved.resolved_uri.starts_with("projectpackage:") =>
            {
                return ModuleCompletion::Source(self.load_module_source(resolved));
            }
            ModuleRequestKind::Capabilities { key, module }
                if {
                    let u = key.as_ref().map(|k| &k.uri).unwrap_or(&module.original_uri);
                    u.starts_with("package:") || u.starts_with("projectpackage:")
                } =>
            {
                // Mirrors `ModuleKeys.AbstractPackage` (`ModuleKeys.java:598-610`):
                // `isLocal() == true`, `hasHierarchicalUris() == true`, `hasFragmentPaths() == true`.
                return ModuleCompletion::Capabilities(Ok(ModuleCapabilities {
                    local: true,
                    hierarchical: true,
                    fragment_paths: true,
                }));
            }
            ModuleRequestKind::HasElement { element_uri, .. }
                if element_uri.starts_with("package:")
                    || element_uri.starts_with("projectpackage:") =>
            {
                return ModuleCompletion::HasElement(self.package_has_element(element_uri, false));
            }
            _ => {}
        }

        // 6. Synthetic inline/query modules (`ModuleKeys.Synthetic.resolve`, `ModuleKeys.java:250-254`)
        // must pass `SecurityManager.checkResolveModule(uri)` before `default_completion` resolves them.
        match &request.kind {
            ModuleRequestKind::Resolve { key, .. } | ModuleRequestKind::ResolveKey { key, .. }
                if key.kind == ModuleKeyKind::Synthetic =>
            {
                if let Err(err) = self.check_resolve_module(&key.uri) {
                    return ModuleCompletion::Resolved(Err(err));
                }
            }
            _ => {}
        }

        // 7. Builtin stdlib (`pkl:`) and default URI resolution.
        if let Some(completion) = default_completion(request) {
            return completion;
        }

        // 8. Policy-aware standard key selection & capabilities.
        if let Some(completion) = standard_key_completion_with_policy(request, self.fs.policy()) {
            return completion;
        }

        // 9. Native I/O phases.
        match &request.kind {
            ModuleRequestKind::SelectKey { .. } => {
                ModuleCompletion::KeySelected(Err(ModuleHostError::NoLoader))
            }
            ModuleRequestKind::Capabilities { module, key } => {
                let uri = key.as_ref().map(|k| &k.uri).unwrap_or(&module.original_uri);
                let lower = uri.to_java_utf8().to_ascii_lowercase();
                if lower.starts_with("jar:file:") {
                    ModuleCompletion::Capabilities(Ok(ModuleCapabilities {
                        local: true,
                        hierarchical: true,
                        fragment_paths: false,
                    }))
                } else {
                    ModuleCompletion::Capabilities(Ok(ModuleCapabilities {
                        local: false,
                        hierarchical: false,
                        fragment_paths: false,
                    }))
                }
            }
            ModuleRequestKind::HasElement {
                key,
                module,
                element_uri,
            } => {
                if self.custom_security.is_some()
                    && let Err(err) = self.check_resolve_module(element_uri)
                {
                    return ModuleCompletion::HasElement(Err(err));
                }
                let is_modulepath = key
                    .as_ref()
                    .is_some_and(|k| k.handle == "builtin:modulepath")
                    || module.original_uri.starts_with("modulepath:")
                    || element_uri.starts_with("modulepath:");
                let result = if is_modulepath {
                    self.modulepath.has_element(element_uri, &self.fs, false)
                } else if element_uri.starts_with("file:") {
                    self.fs.has_element(element_uri, false)
                } else {
                    Ok(false)
                };
                ModuleCompletion::HasElement(result)
            }
            ModuleRequestKind::Resolve { key, .. } => {
                ModuleCompletion::Resolved(self.resolve_module_key(key))
            }
            ModuleRequestKind::ResolveKey {
                key,
                resolved_key_id,
                ..
            } => ModuleCompletion::KeyResolved(self.resolve_module_key(key).map(|module| {
                ResolvedModuleKey {
                    id: *resolved_key_id,
                    key: key.clone(),
                    handle: PklString::default(),
                    module,
                }
            })),
            ModuleRequestKind::LoadSource { resolved, .. } => {
                ModuleCompletion::Source(self.load_module_source(resolved))
            }
            _ => pkl_runtime::modules::unsupported_completion(request),
        }
    }

    fn resolve_module_key(&self, key: &ModuleKey) -> Result<ResolvedModule, ModuleHostError> {
        // `modulepath:`
        if key.handle == "builtin:modulepath" || key.uri.starts_with("modulepath:") {
            if self.custom_security.is_some() {
                self.check_resolve_module(&key.uri)?;
            }
            let (resolved, bytes) = self.modulepath.resolve_module(&key.uri, &self.fs)?;
            self.stage_source(&resolved.resolved_uri, bytes);
            return Ok(resolved);
        }

        // `file:`
        if key.kind == ModuleKeyKind::StandardFile || key.uri.starts_with("file:") {
            if self.custom_security.is_some() {
                self.check_resolve_module(&key.uri)?;
            }
            let (resolved, real_path) = self.fs.resolve_file_module(&key.uri)?;
            if self.custom_security.is_some() && resolved.resolved_uri != key.uri {
                self.check_resolve_module(&resolved.resolved_uri)?;
            }
            let mut paths = self
                .resolved_file_paths
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            if let Some((_, existing)) = paths.iter_mut().find(|(k, _)| *k == resolved.resolved_uri)
            {
                *existing = real_path;
            } else {
                paths.push((resolved.resolved_uri.clone(), real_path));
            }
            return Ok(resolved);
        }

        // `http:` / `https:`
        if key.kind == ModuleKeyKind::StandardHttp
            || key.uri.starts_with("http:")
            || key.uri.starts_with("https:")
        {
            let raw = key.uri.to_java_utf8();
            let bytes = self.http.fetch_module(&raw, |hop_uri| {
                let hop_pkl: PklString = hop_uri.into();
                match self.check_resolve_module(&hop_pkl) {
                    Ok(()) => Ok(()),
                    Err(ModuleHostError::SecurityManagerException { message }) => Err(message
                        .unwrap_or_else(|| format!("Refusing to load module `{hop_uri}`.").into())),
                    Err(other) => Err(format!("{other:?}").into()),
                }
            })?;
            // `ModuleKeys.Http.resolve` keeps original `uri` (`ResolvedModuleKeys.virtual(this, uri, ...)`).
            let resolved = ResolvedModule {
                original_uri: key.uri.clone(),
                resolved_uri: key.uri.clone(),
                source_uri: key.uri.clone(),
            };
            self.stage_source(&resolved.resolved_uri, bytes);
            return Ok(resolved);
        }

        // `jar:file:` (`ModuleKeys.GenericUrl`)
        if key
            .uri
            .to_java_utf8()
            .to_ascii_lowercase()
            .starts_with("jar:file:")
        {
            if self.custom_security.is_some() {
                self.check_resolve_module(&key.uri)?;
            }
            let (resolved, bytes) = resolve_generic_jar_module(&key.uri, &self.fs)?;
            self.stage_source(&resolved.resolved_uri, bytes);
            return Ok(resolved);
        }

        Err(ModuleHostError::NoLoader)
    }

    fn stage_source(&self, resolved_uri: &PklString, bytes: Vec<u8>) {
        let mut staged = self
            .staged_sources
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        staged.insert(resolved_uri.clone(), bytes);
    }

    fn load_module_source(
        &self,
        resolved: &ResolvedModule,
    ) -> Result<ModuleSource, ModuleHostError> {
        if resolved.resolved_uri.starts_with("file:") {
            let staged_path = {
                let mut paths = self
                    .resolved_file_paths
                    .lock()
                    .unwrap_or_else(|e| e.into_inner());
                paths
                    .iter()
                    .position(|(k, _)| *k == resolved.resolved_uri)
                    .map(|idx| paths.swap_remove(idx).1)
            };
            if let Some(real_path) = staged_path {
                let bytes = self.fs.read_file_module_bytes(&real_path)?;
                let text = String::from_utf8(bytes)
                    .map_err(|err| ModuleHostError::Io(IoFailure::from(err.to_string())))?;
                return Ok(ModuleSource {
                    text: SourceText::from(text),
                    inferred_name: PklString::default(),
                });
            }
        }
        let bytes_opt = {
            let mut staged = self
                .staged_sources
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            let from_resolved = staged.remove(&resolved.resolved_uri);
            let from_original = if resolved.original_uri != resolved.resolved_uri {
                staged.remove(&resolved.original_uri)
            } else {
                None
            };
            from_resolved.or(from_original)
        };
        let bytes = match bytes_opt {
            Some(b) => b,
            None if resolved.resolved_uri.starts_with("file:") => {
                let real_path = self.fs.resolve_file_module(&resolved.resolved_uri)?.1;
                self.fs.read_file_module_bytes(&real_path)?
            }
            None => return Err(ModuleHostError::NotFound),
        };
        let text = String::from_utf8(bytes)
            .map_err(|err| ModuleHostError::Io(IoFailure::from(err.to_string())))?;
        let inferred_name = if resolved.original_uri.starts_with("package:")
            || resolved.original_uri.starts_with("projectpackage:")
        {
            infer_name_for_uri(&resolved.original_uri.to_java_utf8())
        } else {
            PklString::default()
        };
        Ok(ModuleSource {
            text: SourceText::from(text),
            inferred_name,
        })
    }

    /// Handles a typed `GlobRequest` across custom readers, external readers, `package:`,
    /// `projectpackage:`, `file:`, `env:`, `prop:`, `modulepath:`, `http:`, `https:`.
    pub fn handle_glob_request(&self, request: &GlobRequest) -> GlobCompletion {
        // 0. Custom in-process / client module and resource readers FIRST.
        if let Some(completion) = self.try_handle_custom_glob_request(request) {
            return completion;
        }

        // 1. Registered external module/resource readers.
        if let Some(completion) = self.external_readers.try_complete_glob(
            request,
            |uri| self.check_resolve_module(uri),
            |uri| self.check_resolve_resource_for_glob(uri),
        ) {
            return completion;
        }

        // 2. `package:` / `projectpackage:` BEFORE `configured_completion_with_policy`
        //    (otherwise `StandardGenericUrl` marks package modules as non-globbable!).
        match &request.kind {
            GlobRequestKind::SelectResource { id, uri }
                if uri.starts_with("package:") || uri.starts_with("projectpackage:") =>
            {
                let scheme = if uri.starts_with("package:") {
                    "package"
                } else {
                    "projectpackage"
                };
                return GlobCompletion::Resource(Ok(ResourceReader {
                    id: *id,
                    scheme: scheme.into(),
                    handle: format!("builtin:{scheme}").into(),
                }));
            }
            GlobRequestKind::IsGlobbable { reader } if is_package_glob_reader(reader) => {
                return GlobCompletion::Globbable(Ok(true));
            }
            GlobRequestKind::IsHierarchical { reader } if is_package_glob_reader(reader) => {
                return GlobCompletion::Hierarchical(Ok(true));
            }
            GlobRequestKind::FragmentPaths { reader }
                if reader.scheme == "package" || reader.scheme == "projectpackage" =>
            {
                return GlobCompletion::FragmentPaths(Ok(true));
            }
            GlobRequestKind::HasElement { reader, uri }
                if uri.starts_with("package:")
                    || uri.starts_with("projectpackage:")
                    || reader.scheme == "package"
                    || reader.scheme == "projectpackage" =>
            {
                return GlobCompletion::HasElement(
                    self.package_has_element(uri, true).map_err(Into::into),
                );
            }
            GlobRequestKind::ListElements { reader, uri }
                if uri.starts_with("package:")
                    || uri.starts_with("projectpackage:")
                    || is_package_glob_reader(reader) =>
            {
                let is_resource = matches!(reader, GlobReader::Resource(_));
                return GlobCompletion::Elements(
                    self.package_list_elements(uri, is_resource)
                        .map_err(Into::into),
                );
            }
            _ => {}
        }

        // 3. Builtin configured `env:`, `prop:`, and non-package capability metadata.
        if let Some(completion) = configured_completion_with_policy(
            request,
            &self.properties,
            &self.environment,
            self.fs.policy(),
        ) {
            return completion;
        }

        // 4. Native `file:` / `modulepath:` glob phases.
        match &request.kind {
            GlobRequestKind::IsGlobbable {
                reader: GlobReader::Module(key),
            } => {
                let globbable = key.kind == ModuleKeyKind::StandardFile
                    || key.uri.starts_with("file:")
                    || key.uri.starts_with("package:")
                    || key.uri.starts_with("projectpackage:");
                GlobCompletion::Globbable(Ok(globbable))
            }
            GlobRequestKind::IsHierarchical {
                reader: GlobReader::Module(key),
            } => {
                let hierarchical = key.kind == ModuleKeyKind::StandardFile
                    || key.handle == "builtin:modulepath"
                    || key.uri.starts_with("file:")
                    || key.uri.starts_with("modulepath:")
                    || key.uri.starts_with("http:")
                    || key.uri.starts_with("https:")
                    || key.uri.starts_with("package:")
                    || key.uri.starts_with("projectpackage:");
                GlobCompletion::Hierarchical(Ok(hierarchical))
            }
            GlobRequestKind::HasElement { uri, .. } => {
                if self.custom_security.is_some()
                    && let Err(err) = self.check_resolve_resource_for_glob(uri)
                {
                    return GlobCompletion::HasElement(Err(err.into()));
                }
                let result = if uri.starts_with("file:") {
                    self.fs.has_element(uri, true).map_err(Into::into)
                } else if uri.starts_with("modulepath:") {
                    self.modulepath
                        .has_element(uri, &self.fs, true)
                        .map_err(Into::into)
                } else {
                    Err(GlobHostError::Failure(ModuleHostError::Unsupported(
                        format!("Unsupported glob hasElement for `{uri}`.").into(),
                    )))
                };
                GlobCompletion::HasElement(result)
            }
            GlobRequestKind::ListElements { reader, uri } => {
                let is_resource = matches!(reader, GlobReader::Resource(_));
                if self.custom_security.is_some() {
                    let check = if is_resource {
                        self.check_resolve_resource_for_glob(uri)
                    } else {
                        self.check_resolve_module(uri)
                    };
                    if let Err(err) = check {
                        return GlobCompletion::Elements(Err(err.into()));
                    }
                }
                let result = if uri.starts_with("file:") {
                    self.fs.list_elements(uri, is_resource).map_err(Into::into)
                } else {
                    Err(GlobHostError::Failure(ModuleHostError::Unsupported(
                        format!("Unsupported glob listElements for `{uri}`.").into(),
                    )))
                };
                GlobCompletion::Elements(result)
            }
            _ => unsupported_glob_completion(request),
        }
    }
}

impl Drop for NativeHost {
    fn drop(&mut self) {
        self.close();
    }
}

/// Helper to check whether a `RuntimeError` is a timeout failure (`ErrorKind::Timeout`).
pub fn is_timeout_error(err: &RuntimeError) -> bool {
    err.kind == ErrorKind::Timeout
}

fn sanitized_eval_options(options: &ModuleEvalOptions) -> ModuleEvalOptions {
    ModuleEvalOptions {
        no_project: true,
        no_cache: true,
        settings: "pkl:settings".into(),
        ..options.clone()
    }
}

fn is_package_glob_reader(reader: &GlobReader) -> bool {
    match reader {
        GlobReader::Module(key) => {
            key.uri.starts_with("package:") || key.uri.starts_with("projectpackage:")
        }
        GlobReader::Resource(r) => r.scheme == "package" || r.scheme == "projectpackage",
    }
}

fn has_uri_scheme(s: &str) -> bool {
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

fn map_package_uri_error(failure: PackageUriError) -> ModuleHostError {
    match failure.failure {
        PackageUriFailure::Unsupported(reason) => ModuleHostError::Unsupported(reason),
        _ => ModuleHostError::UriSyntax {
            reason: failure.reason(),
        },
    }
}

fn read_failure_to_module_err(err: ReadFailure) -> ModuleHostError {
    match err {
        ReadFailure::SecurityManagerException { message } => {
            ModuleHostError::SecurityManagerException { message }
        }
        ReadFailure::HttpClientException { message } => {
            ModuleHostError::HttpClientException { message }
        }
        ReadFailure::ExternalReaderProcess { message } => {
            ModuleHostError::ExternalReaderProcess { message }
        }
        ReadFailure::UriSyntax { reason } => ModuleHostError::UriSyntax { reason },
        ReadFailure::EvalError { message, hint } => ModuleHostError::EvalError { message, hint },
        ReadFailure::Io(io) => ModuleHostError::Io(io),
    }
}

fn module_err_to_read_failure(err: ModuleHostError) -> ReadFailure {
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

fn read_local_module_bytes(path: &Path) -> Result<Vec<u8>, ModuleHostError> {
    match std::fs::read(path) {
        Ok(bytes) => Ok(bytes),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Err(ModuleHostError::NotFound),
        Err(err)
            if err.kind() == std::io::ErrorKind::IsADirectory
                || path.is_dir()
                || err.to_string().starts_with("Is a directory") =>
        {
            Err(ModuleHostError::Io("Is a directory".into()))
        }
        Err(err) => Err(ModuleHostError::Io(err.to_string().into())),
    }
}

fn read_local_project_resource(
    path: &Path,
    fallback_uri: &PklString,
) -> Result<ReadResult, ReadFailure> {
    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let resource_uri = if canonical.is_absolute() {
        path_to_file_uri(&canonical)
    } else {
        fallback_uri.clone()
    };
    if path.is_dir() {
        let entries = std::fs::read_dir(path).map_err(|e| ReadFailure::Io(e.to_string().into()))?;
        let mut names = Vec::new();
        for entry in entries.flatten() {
            if !entry.path().is_symlink() {
                names.push(entry.file_name().to_string_lossy().into_owned());
            }
        }
        names.sort_by(|a, b| {
            a.to_lowercase()
                .cmp(&b.to_lowercase())
                .then_with(|| a.cmp(b))
        });
        let mut out = String::new();
        for name in names {
            out.push_str(&name);
            out.push('\n');
        }
        return Ok(ReadResult::Resource(Resource {
            uri: resource_uri,
            bytes: out.into_bytes().into(),
        }));
    }
    match std::fs::read(path) {
        Ok(bytes) => Ok(ReadResult::Resource(Resource {
            uri: resource_uri,
            bytes: bytes.into(),
        })),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(ReadResult::Missing),
        Err(err) => Err(ReadFailure::Io(err.to_string().into())),
    }
}

fn list_local_dir_elements(dir_path: &Path) -> Result<Vec<PathElement>, ModuleHostError> {
    let entries = match std::fs::read_dir(dir_path) {
        Ok(e) => e,
        Err(err)
            if matches!(
                err.kind(),
                std::io::ErrorKind::NotFound | std::io::ErrorKind::NotADirectory
            ) =>
        {
            return Ok(Vec::new());
        }
        Err(err) => return Err(ModuleHostError::Io(err.to_string().into())),
    };
    let mut out = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|err| ModuleHostError::Io(err.to_string().into()))?;
        let path = entry.path();
        if path.is_symlink() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        out.push(PathElement {
            name: name.into(),
            is_directory: path.is_dir(),
        });
    }
    Ok(out)
}

fn infer_name_for_uri(uri: &str) -> PklString {
    let has_fragment_paths = uri.starts_with("package:") || uri.starts_with("projectpackage:");
    if has_fragment_paths {
        let slice = uri.rsplit('#').next().unwrap_or(uri);
        let leaf = slice.rsplit('/').next().unwrap_or(slice);
        let decoded = pkl_runtime::packages::zip::percent_decode(leaf);
        let stem = decoded.strip_suffix(".pkl").unwrap_or(&decoded);
        return if stem.is_empty() {
            PklString::from("module")
        } else {
            PklString::from(stem)
        };
    }

    // Mirror `IoUtils.doInferModuleName(URI)` (`IoUtils.java:289-297`):
    // opaque URIs (scheme-specific part does not start with `/`, e.g. `repl:text`,
    // `pkl:base`) drop any trailing `:version` segment and take the last `.`-segment.
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
    let decoded = pkl_runtime::packages::zip::percent_decode(leaf);
    let stem = decoded.strip_suffix(".pkl").unwrap_or(&decoded);
    if stem.is_empty() {
        PklString::from("module")
    } else {
        PklString::from(stem)
    }
}
