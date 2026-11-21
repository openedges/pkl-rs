//! Per-evaluator native state for `pkl server` (`CreateEvaluatorRequest` -> `NativeHost`
//! + persistent `SourceSession::module_evaluator` + client reader/logger adapters).

use pkl_host_native::{
    CaCertSource, CustomModuleReader, CustomResourceReader, HostLogger, NativeHost,
    NativeHostConfig, NativeHostExtensions, file_uri_to_path,
};
use pkl_runtime::evaluation::frontend::DiagnosticProfile;
use pkl_runtime::evaluation::session::SourceSession;
use pkl_runtime::external_reader::{
    ExternalModuleResolver, ExternalReaderError, ExternalReaderTransport, ExternalResourceResolver,
    ModuleReaderSpec, ResourceReaderSpec,
};
use pkl_runtime::glob::PathElement;
use pkl_runtime::host::ReadFailure;
use pkl_runtime::host::http::{HttpClientConfig, ProxyConfig};
use pkl_runtime::logging::{LogFrame, LogLevel, TraceMode};
use pkl_runtime::modules::{ModuleHostError, ModuleSource, ModuleSourceSpec, ResolvedModule};
use pkl_runtime::packages::project::{
    PKL_PROJECT_DEPS_FILENAME, ProjectDependenciesManager, ProjectDeps,
};
use pkl_runtime::resources::{ReadResult, Resource};
use pkl_runtime::server::{
    ClientCallbackRegistry, ServerExternalReader, ServerHttp, ServerMessage, ServerProject,
};
use pkl_runtime::source::SourceText;
use pkl_runtime::value::PklString;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::{Arc, Mutex};

/// Outbound server message sink (serialized across threads by `Server`).
pub(crate) trait OutboundSink: Send + Sync {
    fn send(&self, msg: &ServerMessage) -> Result<(), ExternalReaderError>;
}

/// Distinguishes recoverable `CreateEvaluatorResponse.error` failures from fatal
/// `IllegalStateException` (`No security manager set.`) matching `Server.kt:190-235`.
pub(crate) enum CreateEvaluatorFailure {
    /// Recoverable `IllegalArgumentException` / `ProtocolException`: returned in
    /// `CreateEvaluatorResponse { evaluator_id: None, error: Some(msg) }`.
    Protocol(String),
    /// Fatal `IllegalStateException` (e.g. both `allowedModules` and `allowedResources`
    /// omitted/empty), matching `SecurityManagers.StandardBuilder.build()`.
    Fatal(String),
}

/// Configuration extracted from `ServerMessage::CreateEvaluatorRequest`.
pub(crate) struct CreateEvaluatorParams {
    pub allowed_modules: Option<Vec<String>>,
    pub allowed_resources: Option<Vec<String>>,
    pub client_module_readers: Option<Vec<ModuleReaderSpec>>,
    pub client_resource_readers: Option<Vec<ResourceReaderSpec>>,
    pub module_paths: Option<Vec<String>>,
    pub env: Option<BTreeMap<String, String>>,
    pub properties: Option<BTreeMap<String, String>>,
    pub timeout_seconds: Option<i64>,
    pub root_dir: Option<String>,
    pub cache_dir: Option<String>,
    pub output_format: Option<String>,
    pub project: Option<ServerProject>,
    pub http: Option<ServerHttp>,
    pub external_module_readers: Option<BTreeMap<String, ServerExternalReader>>,
    pub external_resource_readers: Option<BTreeMap<String, ServerExternalReader>>,
    pub trace_mode: Option<TraceMode>,
}

/// Persistent evaluator instance backing one `evaluatorId` in `pkl server`.
pub(crate) struct ServerEvaluator {
    evaluator_id: i64,
    host: Arc<NativeHost>,
    session: Mutex<Option<SourceSession>>,
    client_transport: Arc<dyn ExternalReaderTransport>,
    closed: AtomicBool,
}

impl ServerEvaluator {
    pub(crate) fn create(
        evaluator_id: i64,
        params: CreateEvaluatorParams,
        outbound: Arc<dyn OutboundSink>,
        client_transport: Arc<dyn ExternalReaderTransport>,
        request_id_counter: Arc<AtomicI64>,
    ) -> Result<Self, CreateEvaluatorFailure> {
        // Match `Server.kt:194-205` order: validate HTTP config BEFORE SecurityManager
        // (verified by `AbstractServerTest` `http rewrites -- invalid rule`, where both
        // `allowedModules` and `allowedResources` are null yet invalid rewrite returns
        // `CreateEvaluatorResponse.error` instead of `No security manager set.`).
        let (http_cfg, ca_certs) = build_http_config(params.http)?;

        // Match `Server.kt:206-216` + `SecurityManagers.StandardBuilder.build()`:
        // omitted allowlists default to EMPTY `[]` (NOT `defaultAllowed*`); if both are
        // empty, throw `No security manager set.`.
        let allowed_modules = params.allowed_modules.unwrap_or_default();
        let allowed_resources = params.allowed_resources.unwrap_or_default();
        if allowed_modules.is_empty() && allowed_resources.is_empty() {
            return Err(CreateEvaluatorFailure::Fatal(
                "No security manager set.".to_string(),
            ));
        }

        let project_dependencies = match params.project {
            Some(proj) => Some(build_project_dependencies(&proj)?),
            None => None,
        };

        let timeout_ms = params
            .timeout_seconds
            .map(|secs| u64::try_from(secs.max(0)).unwrap_or(0).saturating_mul(1000));

        let external_module_readers = params
            .external_module_readers
            .unwrap_or_default()
            .into_iter()
            .map(|(scheme, spec)| (scheme, spec.to_external_reader_config()))
            .collect();
        let external_resource_readers = params
            .external_resource_readers
            .unwrap_or_default()
            .into_iter()
            .map(|(scheme, spec)| (scheme, spec.to_external_reader_config()))
            .collect();

        let host_config = NativeHostConfig {
            allowed_modules: Some(allowed_modules),
            allowed_resources: Some(allowed_resources),
            root_dir: params.root_dir.map(PathBuf::from),
            module_paths: params
                .module_paths
                .unwrap_or_default()
                .into_iter()
                .map(PathBuf::from)
                .collect(),
            http: http_cfg,
            ca_certs,
            properties: params.properties.unwrap_or_default().into_iter().collect(),
            environment: params.env.unwrap_or_default().into_iter().collect(),
            timeout_ms,
            external_module_schemes: Vec::new(),
            external_resource_schemes: Vec::new(),
            external_module_readers,
            external_resource_readers,
            // Match `Server.kt:223`: `moduleCacheDir = message.cacheDir` (`None` disables disk cache).
            module_cache_dir: params.cache_dir.map(PathBuf::from),
            project_dependencies,
            test_mode: false,
            trace_mode: params.trace_mode.unwrap_or(TraceMode::Compact),
            power_assertions: false,
            ..NativeHostConfig::default()
        };

        let client_registry = ClientCallbackRegistry::new(
            evaluator_id,
            Arc::clone(&client_transport),
            request_id_counter,
            params.client_module_readers.unwrap_or_default(),
            params.client_resource_readers.unwrap_or_default(),
        );

        let mut ext = NativeHostExtensions::default()
            .with_color(false)
            .with_logger(Arc::new(ServerClientLogger {
                evaluator_id,
                outbound,
            }));
        if let Some(output_format) = params.output_format {
            ext = ext.with_output_format(output_format);
        }

        if let Some(resolver) = client_registry.module_resolver().cloned() {
            for spec in client_registry.module_specs() {
                ext = ext.with_custom_module_reader(Arc::new(ClientModuleReaderAdapter {
                    spec: spec.clone(),
                    resolver: Arc::clone(&resolver),
                }));
            }
        }
        for (spec, resolver) in client_registry.resource_readers() {
            ext = ext.with_custom_resource_reader(Arc::new(ClientResourceReaderAdapter {
                spec: spec.clone(),
                resolver: Arc::clone(resolver),
            }));
        }

        let host = Arc::new(
            NativeHost::with_extensions(host_config, ext)
                .map_err(CreateEvaluatorFailure::Protocol)?,
        );
        let session = host.new_public_session(DiagnosticProfile::ApiIdentity);

        Ok(Self {
            evaluator_id,
            host,
            session: Mutex::new(Some(session)),
            client_transport,
            closed: AtomicBool::new(false),
        })
    }

    pub(crate) fn evaluator_id(&self) -> i64 {
        self.evaluator_id
    }

    pub(crate) fn evaluate(
        &self,
        module_uri: String,
        module_text: Option<String>,
        expr: Option<String>,
    ) -> Result<Vec<u8>, String> {
        if self.closed.load(Ordering::SeqCst) {
            return Err("Evaluator has already been closed.".to_string());
        }
        let mut guard = self.session.lock().unwrap_or_else(|e| e.into_inner());
        let Some(session) = guard.as_mut() else {
            return Err("Evaluator has already been closed.".to_string());
        };
        let source = ModuleSourceSpec {
            uri: PklString::from(module_uri),
            contents: module_text.map(SourceText::from),
        };
        let expression = expr.as_deref().unwrap_or("module");
        self.host
            .evaluate_expression_pkl_binary_on(session, source, expression)
    }

    /// Cancels any in-flight evaluation and releases host/session/external-reader resources.
    pub(crate) fn close(&self) {
        if self
            .closed
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
        {
            self.client_transport.cancel();
            self.host.cancel_flag().cancel();
            // Send graceful `0x32 CloseExternalProcess` (and fail any in-flight reader calls)
            // before locking `session` so blocked reader evaluations unwind cleanly without
            // force-killing external readers prematurely via `NativeHost::cancel()`.
            self.host.external_readers().close_all();
            if let Ok(mut guard) = self.session.lock()
                && let Some(mut session) = guard.take()
            {
                session.close();
            }
            self.host.close();
        }
    }
}

impl Drop for ServerEvaluator {
    fn drop(&mut self) {
        self.close();
    }
}

fn build_http_config(
    http: Option<ServerHttp>,
) -> Result<(HttpClientConfig, CaCertSource), CreateEvaluatorFailure> {
    let mut cfg = HttpClientConfig::default();
    let mut ca_certs = CaCertSource::bundled_only();
    let Some(http) = http else {
        return Ok((cfg, ca_certs));
    };
    if let Some(proxy) = http.proxy {
        let no_proxy = proxy.no_proxy.unwrap_or_default();
        cfg.proxy = ProxyConfig::new(proxy.address.as_deref(), &no_proxy)
            .map_err(CreateEvaluatorFailure::Protocol)?;
    }
    if let Some(pem_bytes) = http.ca_certificates {
        ca_certs = CaCertSource {
            extra_pem_bytes: vec![pem_bytes],
            disable_home_cacerts: true,
            ..CaCertSource::default()
        };
    }
    if let Some(rewrites) = http.rewrites {
        cfg = cfg
            .with_rewrites(rewrites)
            .map_err(CreateEvaluatorFailure::Protocol)?;
    }
    if let Some(headers) = http.headers {
        for (pattern, header_map) in headers {
            let mut pairs = Vec::new();
            for (name, values) in header_map {
                for val in values {
                    pairs.push((name.clone(), val));
                }
            }
            cfg.add_headers(&pattern, pairs)
                .map_err(CreateEvaluatorFailure::Protocol)?;
        }
    }
    Ok((cfg, ca_certs))
}

fn build_project_dependencies(
    project: &ServerProject,
) -> Result<ProjectDependenciesManager, CreateEvaluatorFailure> {
    let tree = project
        .to_declared_dependencies_tree()
        .map_err(CreateEvaluatorFailure::Protocol)?;
    let project_file_path = file_uri_to_path(&PklString::from(project.project_file_uri.as_str()))
        .map_err(|err| {
        CreateEvaluatorFailure::Protocol(format!(
            "Invalid project file URI `{}`: {err:?}",
            project.project_file_uri
        ))
    })?;
    let project_dir = project_file_path
        .parent()
        .map(std::path::Path::to_path_buf)
        .unwrap_or(project_file_path);

    let mut mgr = ProjectDependenciesManager::new(tree, project_dir.clone());
    let deps_file = project_dir.join(PKL_PROJECT_DEPS_FILENAME);
    if deps_file.is_file() {
        let deps_uri = mgr.project_deps_file_uri();
        let text = std::fs::read_to_string(&deps_file).map_err(|e| {
            CreateEvaluatorFailure::Protocol(format!(
                "Encountered an error when attempting to load `PklProject.deps.json` at `{deps_uri}`.\n{e}"
            ))
        })?;
        let parsed = ProjectDeps::parse(&text, &deps_uri)
            .map_err(|e| CreateEvaluatorFailure::Protocol(format!("{e:?}")))?;
        mgr.initialize_with_deps(parsed)
            .map_err(|e| CreateEvaluatorFailure::Protocol(format!("{e:?}")))?;
    }
    Ok(mgr)
}

// ============================================================================
// Client Logger & Client Module/Resource Reader Adapters
// ============================================================================

struct ServerClientLogger {
    evaluator_id: i64,
    outbound: Arc<dyn OutboundSink>,
}

impl HostLogger for ServerClientLogger {
    fn log(&self, level: LogLevel, message: &PklString, frame: &LogFrame) {
        let level_code = match level {
            LogLevel::Trace => 0,
            LogLevel::Warn => 1,
        };
        let _ = self.outbound.send(&ServerMessage::LogMessage {
            evaluator_id: self.evaluator_id,
            level: level_code,
            message: message.to_java_utf8(),
            frame_uri: frame.module_uri.to_java_utf8(),
        });
    }
}

struct ClientModuleReaderAdapter {
    spec: ModuleReaderSpec,
    resolver: Arc<ExternalModuleResolver>,
}

impl CustomModuleReader for ClientModuleReaderAdapter {
    fn matches_uri(&self, uri: &PklString) -> bool {
        let Some(raw) = uri.as_str() else {
            return false;
        };
        let Some((scheme, _)) = raw.split_once(':') else {
            return false;
        };
        // Exact match to `ClientModuleKeyFactory.kt:35`: `uri.scheme in schemes`.
        !scheme.is_empty() && !scheme.contains(['/', '?', '#']) && scheme == self.spec.scheme
    }

    fn is_hierarchical(&self, _uri: &PklString) -> bool {
        self.spec.has_hierarchical_uris
    }

    fn is_local(&self, _uri: &PklString) -> bool {
        self.spec.is_local
    }

    fn is_globbable(&self, _uri: &PklString) -> bool {
        self.spec.is_globbable
    }

    fn is_cached(&self, _uri: &PklString) -> bool {
        true
    }

    fn resolve_module(
        &self,
        uri: &PklString,
    ) -> Result<(ResolvedModule, ModuleSource), ModuleHostError> {
        let uri_str = uri.as_str().ok_or_else(|| {
            ModuleHostError::Unsupported("Client module reader URI must be valid UTF-8.".into())
        })?;
        let text = self
            .resolver
            .resolve_module(uri_str)
            .map_err(ExternalReaderError::into_module_host_error)?;
        let inferred_name = ClientCallbackRegistry::infer_module_name(uri)
            .map_err(|e| ModuleHostError::Unsupported(e.message))?;
        let resolved = ResolvedModule {
            original_uri: uri.clone(),
            resolved_uri: uri.clone(),
            source_uri: uri.clone(),
        };
        let source = ModuleSource {
            text: SourceText::from(text),
            inferred_name,
        };
        Ok((resolved, source))
    }

    fn has_element(&self, element_uri: &PklString) -> Result<bool, ModuleHostError> {
        let uri_str = element_uri.as_str().ok_or_else(|| {
            ModuleHostError::Unsupported("Client module reader URI must be valid UTF-8.".into())
        })?;
        Ok(self.resolver.has_element(uri_str))
    }

    fn list_elements(&self, base_uri: &PklString) -> Result<Vec<PathElement>, ModuleHostError> {
        let uri_str = base_uri.as_str().ok_or_else(|| {
            ModuleHostError::Unsupported("Client module reader URI must be valid UTF-8.".into())
        })?;
        let elements = self
            .resolver
            .list_elements(uri_str)
            .map_err(ExternalReaderError::into_module_host_error)?;
        Ok(elements.into_iter().map(PathElement::from).collect())
    }
}

struct ClientResourceReaderAdapter {
    spec: ResourceReaderSpec,
    resolver: Arc<ExternalResourceResolver>,
}

impl CustomResourceReader for ClientResourceReaderAdapter {
    fn scheme(&self) -> &str {
        &self.spec.scheme
    }

    fn has_hierarchical_uris(&self) -> bool {
        self.spec.has_hierarchical_uris
    }

    fn is_globbable(&self) -> bool {
        self.spec.is_globbable
    }

    fn read(&self, uri: &PklString) -> Result<ReadResult, ReadFailure> {
        let uri_str = uri.as_str().ok_or_else(|| ReadFailure::EvalError {
            message: "Client resource reader URI must be valid UTF-8.".into(),
            hint: None,
        })?;
        match self
            .resolver
            .read(uri_str)
            .map_err(ExternalReaderError::into_read_failure)?
        {
            Some(bytes) => Ok(ReadResult::Resource(Resource {
                uri: uri.clone(),
                bytes: bytes.into(),
            })),
            None => Ok(ReadResult::Missing),
        }
    }

    fn has_element(&self, uri: &PklString) -> Result<bool, ModuleHostError> {
        let uri_str = uri.as_str().ok_or_else(|| {
            ModuleHostError::Unsupported("Client resource reader URI must be valid UTF-8.".into())
        })?;
        Ok(self.resolver.has_element(uri_str))
    }

    fn list_elements(&self, base_uri: &PklString) -> Result<Vec<PathElement>, ModuleHostError> {
        let uri_str = base_uri.as_str().ok_or_else(|| {
            ModuleHostError::Unsupported("Client resource reader URI must be valid UTF-8.".into())
        })?;
        let elements = self
            .resolver
            .list_elements(uri_str)
            .map_err(ExternalReaderError::into_module_host_error)?;
        Ok(elements.into_iter().map(PathElement::from).collect())
    }
}
