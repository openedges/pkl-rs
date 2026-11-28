//! Public Pkl evaluator and builder mirroring `org.pkl.core.{Evaluator, EvaluatorBuilder}`.

use crate::error::{PklError, Result, format_module_host_error};
use crate::frame::{
    HostFrameTransformerAdapter, StackFrame, StackFrameTransformer, StackFrameTransformers,
};
use crate::http::HttpClient;
use crate::logger::{HostLoggerAdapter, Logger, Loggers};
use crate::module_source::ModuleSource;
use crate::project::Project;
use crate::reader::{
    ModuleKeyFactory, ModuleKeyFactoryAdapter, ResourceReader, ResourceReaderAdapter,
};
use crate::security::{SecurityManager, SecurityManagerAdapter, StandardSecurityManager};
use crate::settings::{ExternalReaderSpec, OutputFormat, PklEvaluatorSettings, TraceMode};
use crate::value::{FromPklValue, PModule, PklBinaryDecoder, PklValue};
use pkl_host_native::{
    CaCertSource, CancelFlag, NativeHost, NativeHostConfig, NativeHostExtensions,
    default_module_cache_dir, file_uri_to_path, path_to_file_uri,
};
use pkl_runtime::diagnostic::TypeDiagnostic;
use pkl_runtime::evaluation::command::{
    COMMAND_SPEC_REFLECT_EXPR, CommandSpec, InvokedCommandLevel, decode_command_spec,
    extract_source_command_metadata, synthesize_command_eval_module,
};
use pkl_runtime::evaluation::frontend::DiagnosticProfile;
use pkl_runtime::evaluation::session::SourceSession;
use pkl_runtime::external_reader::ExternalReaderConfig;
use pkl_runtime::host::format_timeout_message;
use pkl_runtime::host::http::{HttpClientConfig, NoProxyRule, ProxyAddress};
use pkl_runtime::packages::project::{DEFAULT_EDITOR_URL_SCHEME, ProjectDependenciesManager};
use pkl_runtime::packages::test_runner::{
    RENDER_EXAMPLES_DOC_EXPR, RENDER_EXAMPLES_PKL_BINARY_EXPR, TestError, TestFailure, TestResult,
    TestResults, TestSectionName, TestSectionResults, decode_rendered_examples_map,
    evaluate_examples_rendered_detailed, extract_fact_spans, format_fact_failure,
    format_fact_failure_with_power_assertions,
};
use pkl_runtime::pkl_binary_decode::{DecodedValue, decode as decode_pkl_binary};
use pkl_runtime::schema::{ModuleSchema, SchemaExtractor, SchemaStep};
use pkl_runtime::source::SourceMap;
use pkl_runtime::value::{ErrorKind, PklString, RuntimeError, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Output of one entry in `Module.output.files` (`org.pkl.core.FileOutput`).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct FileOutput {
    bytes: Vec<u8>,
}

impl FileOutput {
    pub fn from_bytes(bytes: Vec<u8>) -> Self {
        Self { bytes }
    }

    /// Raw bytes of this file output (`FileOutput.getBytes()`).
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Consumes this file output and returns its raw bytes.
    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }

    /// UTF-8 text of this file output (`FileOutput.getText()`).
    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.bytes).into_owned()
    }
}

/// Result of executing a `pkl:Command` module (`output.bytes` and `output.files`).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct CommandEvalResult {
    pub output_bytes: Vec<u8>,
    pub output_files: Vec<(String, FileOutput)>,
}

/// Thread-safe handle allowing external cancellation of an in-flight evaluation.
#[derive(Clone)]
pub struct CancelHandle {
    flag: Arc<CancelFlag>,
    external_readers: Arc<pkl_runtime::external_reader::ExternalReaderRegistry>,
}

impl std::fmt::Debug for CancelHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CancelHandle")
            .field("cancelled", &self.is_cancelled())
            .finish()
    }
}

impl CancelHandle {
    pub fn cancel(&self) {
        self.flag.cancel();
        self.external_readers.cancel_all();
    }

    pub fn is_cancelled(&self) -> bool {
        self.flag.is_cancelled()
    }
}

/// Internal logger wrapper that forwards every event to `delegate` and captures raw
/// newline-delimited messages ONLY while `evaluate_test` is active.
struct TestCapturingLogger {
    delegate: Arc<dyn Logger>,
    capturing: AtomicBool,
    buffer: Mutex<String>,
}

impl TestCapturingLogger {
    fn new(delegate: Arc<dyn Logger>) -> Self {
        Self {
            delegate,
            capturing: AtomicBool::new(false),
            buffer: Mutex::new(String::new()),
        }
    }

    fn begin_capture(&self) {
        {
            let mut buf = self.buffer.lock().unwrap_or_else(|e| e.into_inner());
            buf.clear();
        }
        self.capturing.store(true, Ordering::SeqCst);
    }

    fn end_capture(&self) -> String {
        self.capturing.store(false, Ordering::SeqCst);
        let mut buf = self.buffer.lock().unwrap_or_else(|e| e.into_inner());
        std::mem::take(&mut *buf)
    }
}

impl Logger for TestCapturingLogger {
    fn trace(&self, message: &str, frame: &StackFrame) {
        if self.capturing.load(Ordering::SeqCst) {
            let mut buf = self.buffer.lock().unwrap_or_else(|e| e.into_inner());
            buf.push_str(message);
            buf.push('\n');
        }
        self.delegate.trace(message, frame);
    }

    fn warn(&self, message: &str, frame: &StackFrame) {
        if self.capturing.load(Ordering::SeqCst) {
            let mut buf = self.buffer.lock().unwrap_or_else(|e| e.into_inner());
            buf.push_str(message);
            buf.push('\n');
        }
        self.delegate.warn(message, frame);
    }
}

/// Builder for [`Evaluator`] mirroring `org.pkl.core.EvaluatorBuilder`.
pub struct EvaluatorBuilder {
    allowed_modules: Option<Vec<String>>,
    allowed_resources: Option<Vec<String>>,
    root_dir: Option<PathBuf>,
    module_paths: Vec<PathBuf>,
    http_config: HttpClientConfig,
    ca_certs: CaCertSource,
    http_client: Option<HttpClient>,
    properties: Vec<(String, String)>,
    environment: Option<Vec<(String, String)>>,
    timeout: Option<Duration>,
    external_module_readers: BTreeMap<String, ExternalReaderConfig>,
    external_resource_readers: BTreeMap<String, ExternalReaderConfig>,
    module_cache_dir: Option<Option<PathBuf>>,
    project_dependencies: Option<ProjectDependenciesManager>,
    output_format: Option<String>,
    test_mode: bool,
    trace_mode: TraceMode,
    power_assertions: bool,
    use_color: bool,
    editor_url_scheme: String,
    diagnostic_profile: DiagnosticProfile,
    logger: Arc<dyn Logger>,
    frame_transformer: Option<Arc<dyn StackFrameTransformer>>,
    security_manager: Option<Arc<dyn SecurityManager>>,
    custom_module_factories: Vec<Arc<dyn ModuleKeyFactory>>,
    custom_resource_readers: Vec<Arc<dyn ResourceReader>>,
}

impl EvaluatorBuilder {
    /// Creates a preconfigured builder with standard defaults (`EvaluatorBuilder.preconfigured()`):
    /// default module/resource allowlists, process environment variables, default `~/.pkl/cache`,
    /// stderr logger, and default stack-frame transformer.
    pub fn preconfigured() -> Self {
        Self {
            allowed_modules: None,
            allowed_resources: None,
            root_dir: None,
            module_paths: Vec::new(),
            http_config: HttpClientConfig::default(),
            ca_certs: CaCertSource::bundled_default(),
            http_client: None,
            properties: Vec::new(),
            environment: None,
            timeout: None,
            external_module_readers: BTreeMap::new(),
            external_resource_readers: BTreeMap::new(),
            module_cache_dir: None,
            project_dependencies: None,
            output_format: None,
            test_mode: false,
            trace_mode: TraceMode::Compact,
            power_assertions: false,
            use_color: false,
            editor_url_scheme: DEFAULT_EDITOR_URL_SCHEME.to_string(),
            diagnostic_profile: DiagnosticProfile::ApiIdentity,
            logger: Loggers::std_err(),
            frame_transformer: None,
            security_manager: None,
            custom_module_factories: Vec::new(),
            custom_resource_readers: Vec::new(),
        }
    }

    /// Creates a minimal unconfigured builder (`EvaluatorBuilder.unconfigured()`):
    /// empty environment, no disk cache, no-op logger, and no implicit frame rewrites.
    pub fn unconfigured() -> Self {
        Self {
            allowed_modules: None,
            allowed_resources: None,
            root_dir: None,
            module_paths: Vec::new(),
            http_config: HttpClientConfig::default(),
            ca_certs: CaCertSource::bundled_default(),
            http_client: None,
            properties: Vec::new(),
            environment: Some(Vec::new()),
            timeout: None,
            external_module_readers: BTreeMap::new(),
            external_resource_readers: BTreeMap::new(),
            module_cache_dir: Some(None),
            project_dependencies: None,
            output_format: None,
            test_mode: false,
            trace_mode: TraceMode::Compact,
            power_assertions: false,
            use_color: false,
            editor_url_scheme: DEFAULT_EDITOR_URL_SCHEME.to_string(),
            diagnostic_profile: DiagnosticProfile::ApiIdentity,
            logger: Loggers::noop(),
            frame_transformer: Some(StackFrameTransformers::empty()),
            security_manager: None,
            custom_module_factories: Vec::new(),
            custom_resource_readers: Vec::new(),
        }
    }

    pub fn with_allowed_modules(mut self, patterns: Vec<String>) -> Self {
        self.allowed_modules = Some(patterns);
        self
    }

    pub fn with_allowed_resources(mut self, patterns: Vec<String>) -> Self {
        self.allowed_resources = Some(patterns);
        self
    }

    pub fn with_root_dir(mut self, root_dir: Option<PathBuf>) -> Self {
        self.root_dir = root_dir;
        self
    }

    pub fn with_security_manager(mut self, security_manager: Arc<dyn SecurityManager>) -> Self {
        self.security_manager = Some(security_manager);
        self
    }

    pub fn with_logger(mut self, logger: Arc<dyn Logger>) -> Self {
        self.logger = logger;
        self
    }

    pub fn with_stack_frame_transformer(
        mut self,
        transformer: Arc<dyn StackFrameTransformer>,
    ) -> Self {
        self.frame_transformer = Some(transformer);
        self
    }

    pub fn with_editor_url_scheme(mut self, scheme: impl Into<String>) -> Self {
        self.editor_url_scheme = scheme.into();
        self
    }

    #[allow(dead_code)]
    pub(crate) fn with_diagnostic_profile(mut self, profile: DiagnosticProfile) -> Self {
        self.diagnostic_profile = profile;
        self
    }

    pub fn with_external_properties(
        mut self,
        properties: impl IntoIterator<Item = (String, String)>,
    ) -> Self {
        self.properties = properties.into_iter().collect();
        self
    }

    pub fn add_external_property(
        mut self,
        key: impl Into<String>,
        value: impl Into<String>,
    ) -> Self {
        let k = key.into();
        let v = value.into();
        if let Some(entry) = self.properties.iter_mut().find(|(ek, _)| *ek == k) {
            entry.1 = v;
        } else {
            self.properties.push((k, v));
        }
        self
    }

    pub fn with_environment_variables(
        mut self,
        env: impl IntoIterator<Item = (String, String)>,
    ) -> Self {
        self.environment = Some(env.into_iter().collect());
        self
    }

    pub fn add_environment_variable(
        mut self,
        key: impl Into<String>,
        value: impl Into<String>,
    ) -> Self {
        let k = key.into();
        let v = value.into();
        let env = self
            .environment
            .get_or_insert_with(|| std::env::vars().collect());
        if let Some(entry) = env.iter_mut().find(|(ek, _)| *ek == k) {
            entry.1 = v;
        } else {
            env.push((k, v));
        }
        self
    }

    pub fn with_module_paths(mut self, module_paths: Vec<PathBuf>) -> Self {
        self.module_paths = module_paths;
        self
    }

    pub fn with_module_cache_dir(mut self, dir: Option<PathBuf>) -> Self {
        self.module_cache_dir = Some(dir);
        self
    }

    pub fn with_output_format(mut self, format: impl Into<String>) -> Self {
        self.output_format = Some(format.into());
        self
    }

    pub fn with_output_format_enum(mut self, format: OutputFormat) -> Self {
        self.output_format = Some(format.as_str().to_string());
        self
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    pub fn with_timeout_opt(mut self, timeout: Option<Duration>) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn with_color(mut self, use_color: bool) -> Self {
        self.use_color = use_color;
        self
    }

    pub fn with_trace_mode(mut self, trace_mode: TraceMode) -> Self {
        self.trace_mode = trace_mode;
        self
    }

    pub fn with_power_assertions(mut self, enabled: bool) -> Self {
        self.power_assertions = enabled;
        self
    }

    pub fn with_test_mode(mut self, enabled: bool) -> Self {
        self.test_mode = enabled;
        self
    }

    pub fn with_http_client(mut self, client: HttpClient) -> Self {
        self.http_client = Some(client);
        self
    }

    #[allow(dead_code)]
    pub(crate) fn with_http_config(mut self, config: HttpClientConfig) -> Self {
        self.http_config = config;
        self
    }

    #[allow(dead_code)]
    pub(crate) fn with_ca_certs(mut self, ca_certs: CaCertSource) -> Self {
        self.ca_certs = ca_certs;
        self
    }

    pub fn with_external_module_readers(
        mut self,
        readers: BTreeMap<String, ExternalReaderSpec>,
    ) -> Self {
        self.external_module_readers = readers;
        self
    }

    pub fn with_external_resource_readers(
        mut self,
        readers: BTreeMap<String, ExternalReaderSpec>,
    ) -> Self {
        self.external_resource_readers = readers;
        self
    }

    pub fn add_module_key_factory(mut self, factory: Arc<dyn ModuleKeyFactory>) -> Self {
        self.custom_module_factories.push(factory);
        self
    }

    pub fn add_resource_reader(mut self, reader: Arc<dyn ResourceReader>) -> Self {
        self.custom_resource_readers.push(reader);
        self
    }

    #[allow(dead_code)]
    pub(crate) fn with_project_dependencies(mut self, deps: ProjectDependenciesManager) -> Self {
        self.project_dependencies = Some(deps);
        self
    }

    /// Applies `PklEvaluatorSettings` (e.g. from `PklProject.evaluatorSettings`),
    /// mirroring `EvaluatorBuilder.applyFromSettings`.
    pub fn apply_from_settings(mut self, settings: &PklEvaluatorSettings) -> Result<Self> {
        if let Some(ref mods) = settings.allowed_modules {
            self.allowed_modules = Some(mods.clone());
        }
        if let Some(ref res) = settings.allowed_resources {
            self.allowed_resources = Some(res.clone());
        }
        if settings.external_properties_is_set {
            self.properties = settings.external_properties.clone();
        }
        if settings.env_is_set {
            self.environment = Some(settings.env.clone());
        }
        if let Some(ref paths) = settings.module_path {
            self.module_paths = paths.clone();
        }
        if let Some(ref root) = settings.root_dir {
            self.root_dir = Some(root.clone());
        }
        if settings.no_cache == Some(true) {
            self.module_cache_dir = Some(None);
        } else if let Some(ref cache_dir) = settings.module_cache_dir {
            self.module_cache_dir = Some(Some(cache_dir.clone()));
        }
        if let Some(nanos) = settings.timeout_nanos {
            self.timeout = Some(Duration::from_nanos(nanos));
        }
        if let Some(color) = settings.color {
            self.use_color = crate::settings::Color::from(color).has_color();
        }
        if let Some(ref mode) = settings.trace_mode {
            if mode == "pretty" {
                self.trace_mode = TraceMode::Pretty;
            } else if mode == "compact" {
                self.trace_mode = TraceMode::Compact;
            }
        }
        if let Some(ref http) = settings.http {
            if let Some(ref proxy) = http.proxy {
                if let Some(ref addr) = proxy.address {
                    self.http_config.proxy.address =
                        Some(ProxyAddress::parse(addr).map_err(PklError::configuration)?);
                }
                if let Some(ref no_proxy) = proxy.no_proxy {
                    let mut rules = Vec::with_capacity(no_proxy.len());
                    for raw in no_proxy {
                        if let Some(rule) =
                            NoProxyRule::parse(raw).map_err(PklError::configuration)?
                        {
                            rules.push(rule);
                        }
                    }
                    self.http_config.proxy.no_proxy = rules;
                }
            }
            if let Some(ref rewrites) = http.rewrites {
                self.http_config = self
                    .http_config
                    .with_rewrites(rewrites.clone())
                    .map_err(PklError::configuration)?;
            }
        }
        if let Some(ref ext_mods) = settings.external_module_readers {
            for (scheme, spec) in ext_mods {
                self.external_module_readers
                    .insert(scheme.clone(), spec.clone());
            }
        }
        if let Some(ref ext_res) = settings.external_resource_readers {
            for (scheme, spec) in ext_res {
                self.external_resource_readers
                    .insert(scheme.clone(), spec.clone());
            }
        }
        Ok(self)
    }

    /// Applies evaluator settings and project dependencies from a loaded [`Project`]
    /// (`EvaluatorBuilder.applyFromProject`).
    pub fn apply_from_project(mut self, project: &Project) -> Result<Self> {
        if let Some(settings) = project.evaluator_settings() {
            self = self.apply_from_settings(settings)?;
        }
        if let Some(deps) = project.dependencies_manager() {
            self.project_dependencies = Some(deps.clone());
        }
        Ok(self)
    }

    /// Constructs the [`Evaluator`].
    pub fn build(self) -> Result<Evaluator> {
        let timeout_ms = self.timeout.map(|d| {
            let ms = d.as_millis().min(u64::MAX as u128) as u64;
            if ms == 0 && !d.is_zero() { 1 } else { ms }
        });

        let needs_reader_security =
            !self.custom_module_factories.is_empty() || !self.custom_resource_readers.is_empty();
        let security_for_readers: Option<Arc<dyn SecurityManager>> = if needs_reader_security {
            Some(match &self.security_manager {
                Some(sec) => sec.clone(),
                None => Arc::new(StandardSecurityManager::new(
                    self.allowed_modules.as_deref(),
                    self.allowed_resources.as_deref(),
                    self.root_dir.clone(),
                )?),
            })
        } else {
            None
        };

        let test_logger = Arc::new(TestCapturingLogger::new(self.logger));
        let host_logger: Arc<dyn pkl_host_native::HostLogger> =
            Arc::new(HostLoggerAdapter(test_logger.clone()));

        let config = NativeHostConfig {
            allowed_modules: self.allowed_modules,
            allowed_resources: self.allowed_resources,
            root_dir: self.root_dir,
            module_paths: self.module_paths,
            http: self.http_config,
            ca_certs: self.ca_certs,
            properties: self.properties,
            environment: self
                .environment
                .unwrap_or_else(|| std::env::vars().collect()),
            timeout_ms,
            external_module_schemes: Vec::new(),
            external_resource_schemes: Vec::new(),
            external_module_readers: self.external_module_readers,
            external_resource_readers: self.external_resource_readers,
            module_cache_dir: self
                .module_cache_dir
                .unwrap_or_else(default_module_cache_dir),
            project_dependencies: self.project_dependencies,
            test_mode: self.test_mode,
            trace_mode: self.trace_mode,
            power_assertions: self.power_assertions,
            editor_url_scheme: self.editor_url_scheme.clone(),
            internal_pkl_binary_eval: false,
        };

        let mut ext = NativeHostExtensions::default()
            .with_color(self.use_color)
            .with_logger(host_logger);
        if let Some(fmt) = self.output_format {
            ext = ext.with_output_format(fmt);
        }
        if let Some(tf) = self.frame_transformer {
            ext = ext.with_frame_transformer(Arc::new(HostFrameTransformerAdapter(tf)), true);
        }
        if let Some(sec) = self.security_manager {
            ext = ext.with_custom_security(Arc::new(SecurityManagerAdapter(sec)));
        }
        if let Some(http) = self.http_client {
            ext = ext.with_http_client(http.inner());
        }
        if let Some(ref security_for_readers) = security_for_readers {
            for factory in self.custom_module_factories {
                ext = ext.with_custom_module_reader(Arc::new(ModuleKeyFactoryAdapter {
                    factory,
                    security: security_for_readers.clone(),
                }));
            }
            for reader in self.custom_resource_readers {
                ext = ext.with_custom_resource_reader(Arc::new(ResourceReaderAdapter {
                    reader,
                    security: security_for_readers.clone(),
                }));
            }
        }

        let host = NativeHost::with_extensions(config, ext).map_err(PklError::configuration)?;
        let session = host.new_public_session(self.diagnostic_profile);

        Ok(Evaluator {
            host,
            session,
            test_logger,
            power_assertions: self.power_assertions,
            use_color: self.use_color,
            editor_url_scheme: self.editor_url_scheme,
            test_mode: self.test_mode,
            diagnostic_profile: self.diagnostic_profile,
            active_deadline_ms: None,
            composite_eval_depth: 0,
            closed: false,
        })
    }
}

impl std::fmt::Debug for EvaluatorBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EvaluatorBuilder")
            .field("root_dir", &self.root_dir)
            .field("module_paths", &self.module_paths)
            .field("output_format", &self.output_format)
            .field("timeout", &self.timeout)
            .field("trace_mode", &self.trace_mode)
            .field("use_color", &self.use_color)
            .field("test_mode", &self.test_mode)
            .finish_non_exhaustive()
    }
}

/// Persistent Pkl evaluator with module caching across evaluation calls, timeout enforcement,
/// external cancellation, and idempotent `close()` + `Drop` cleanup (`org.pkl.core.Evaluator`).
pub struct Evaluator {
    host: NativeHost,
    session: SourceSession,
    test_logger: Arc<TestCapturingLogger>,
    power_assertions: bool,
    use_color: bool,
    editor_url_scheme: String,
    test_mode: bool,
    diagnostic_profile: DiagnosticProfile,
    active_deadline_ms: Option<u64>,
    composite_eval_depth: u32,
    closed: bool,
}

impl std::fmt::Debug for Evaluator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Evaluator")
            .field("closed", &self.is_closed())
            .field("use_color", &self.use_color)
            .field("test_mode", &self.test_mode)
            .finish_non_exhaustive()
    }
}

impl Evaluator {
    /// Creates a default preconfigured evaluator (`Evaluator.preconfigured()`).
    pub fn preconfigured() -> Result<Self> {
        EvaluatorBuilder::preconfigured().build()
    }

    /// Returns a [`CancelHandle`] that can abort in-flight evaluations from another thread.
    pub fn cancel_handle(&self) -> CancelHandle {
        CancelHandle {
            flag: self.host.cancel_flag(),
            external_readers: self.host.external_readers_arc(),
        }
    }

    pub fn is_closed(&self) -> bool {
        self.closed || self.session.is_closed()
    }

    /// Idempotently closes this evaluator and all owned native resources (external readers,
    /// HTTP client if owned, modulepath archives, and VM session).
    pub fn close(&mut self) {
        if !self.closed {
            self.closed = true;
            self.session.close();
            self.host.close();
        }
    }

    fn ensure_open(&self) -> Result<()> {
        if self.closed || self.session.is_closed() {
            return Err(PklError::closed("Evaluator has been closed."));
        }
        Ok(())
    }

    fn map_runtime_err(&self, err: RuntimeError) -> PklError {
        match err.kind {
            ErrorKind::Timeout => {
                PklError::timeout(format_timeout_message(self.host.timeout_ms().unwrap_or(0)))
            }
            ErrorKind::Closed => PklError::closed("Evaluator has been closed."),
            ErrorKind::Cancelled => PklError::cancelled("Evaluation was cancelled."),
            _ => {
                let is_security = err.is_security_error();
                let rendered = self.session.format_error(err);
                if is_security {
                    PklError::security(rendered)
                } else {
                    PklError::evaluation(rendered)
                }
            }
        }
    }

    fn reset_cancel_if_top_level(&self) {
        if !self.closed && self.composite_eval_depth == 0 {
            self.host.reset_cancel_flag_if_set();
        }
    }

    fn effective_clock_and_timeout(&self) -> Result<(u64, Option<u64>)> {
        if let Some(deadline_ms) = self.active_deadline_ms {
            let now = self.host.clock_ms();
            if now >= deadline_ms {
                return Err(PklError::timeout(format_timeout_message(
                    self.host.timeout_ms().unwrap_or(0),
                )));
            }
            return Ok((now, Some(deadline_ms.saturating_sub(now))));
        }
        let timeout_ms = self.host.timeout_ms();
        let now = if timeout_ms.is_some() {
            self.host.clock_ms()
        } else {
            self.session.host_session().last_clock()
        };
        Ok((now, timeout_ms))
    }

    fn run_eval_result<T>(
        &mut self,
        start: impl FnOnce(
            &mut SourceSession,
            u64,
            Option<u64>,
        ) -> std::result::Result<(), RuntimeError>,
        extract: impl FnOnce(Value) -> std::result::Result<T, RuntimeError>,
    ) -> Result<T> {
        self.ensure_open()?;
        let mut stderr = Vec::new();
        let (now, timeout_ms) = self.effective_clock_and_timeout()?;
        let res = (|| -> std::result::Result<T, RuntimeError> {
            start(&mut self.session, now, timeout_ms)?;
            let val = self.host.drive_public_session_with_timeout(
                &mut self.session,
                timeout_ms,
                &mut stderr,
            )?;
            extract(val)
        })();
        self.reset_cancel_if_top_level();
        res.map_err(|err| self.map_runtime_err(err))
    }

    fn evaluate_string_warm_or_cold(
        &mut self,
        source: &ModuleSource,
        expression: &str,
    ) -> Result<String> {
        self.ensure_open()?;
        let uri = source.get_uri();
        if source.contents().is_none() && !uri.starts_with('@') {
            let (now, timeout_ms) = self.effective_clock_and_timeout()?;
            match self
                .session
                .try_start_warm_evaluation_str(uri, expression, now, timeout_ms)
            {
                Ok(true) => {
                    let mut stderr = Vec::new();
                    let res = self
                        .host
                        .drive_public_session_with_timeout(
                            &mut self.session,
                            timeout_ms,
                            &mut stderr,
                        )
                        .and_then(|val| {
                            SourceSession::result_string(val).map(|s| s.to_java_utf8())
                        });
                    self.reset_cancel_if_top_level();
                    return res.map_err(|err| self.map_runtime_err(err));
                }
                Ok(false) => {}
                Err(err) => return Err(self.map_runtime_err(err)),
            }
        }
        let spec = self.prepare_spec(source)?;
        self.run_eval_result(
            |session, now, timeout| session.start_evaluation(spec, expression, now, timeout),
            |val| SourceSession::result_string(val).map(|s| s.to_java_utf8()),
        )
    }

    fn prepare_spec(
        &self,
        source: &ModuleSource,
    ) -> Result<pkl_runtime::modules::ModuleSourceSpec> {
        self.host
            .resolve_module_source_spec(source.to_runtime_spec())
            .map_err(|err| {
                let msg = format_module_host_error(&err);
                if matches!(
                    err,
                    pkl_runtime::modules::ModuleHostError::SecurityManagerException { .. }
                ) {
                    PklError::security(msg)
                } else {
                    PklError::evaluation(msg)
                }
            })
    }

    /// Evaluates a module and decodes it as a [`PModule`] (`Evaluator.evaluate`).
    pub fn evaluate(&mut self, source: &ModuleSource) -> Result<PModule> {
        let bytes = self.evaluate_expression_pkl_binary(source, "module")?;
        PklBinaryDecoder::decode(&bytes)?.into_module()
    }

    /// Evaluates a module and decodes it into a typed Rust value `T` implementing [`FromPklValue`].
    pub fn evaluate_typed<T: FromPklValue>(&mut self, source: &ModuleSource) -> Result<T> {
        let bytes = self.evaluate_expression_pkl_binary(source, "module")?;
        let val = PklBinaryDecoder::decode(&bytes)?;
        T::from_pkl_value(&val)
    }

    /// Evaluates a module's `output.text` (`Evaluator.evaluateOutputText`).
    pub fn evaluate_output_text(&mut self, source: &ModuleSource) -> Result<String> {
        self.evaluate_string_warm_or_cold(source, "output.text")
    }

    /// Evaluates a module's `output.bytes` (`Evaluator.evaluateOutputBytes`).
    pub fn evaluate_output_bytes(&mut self, source: &ModuleSource) -> Result<Vec<u8>> {
        let spec = self.prepare_spec(source)?;
        self.run_eval_result(
            |session, now, timeout| session.start_evaluation_output_bytes(spec, now, timeout),
            |val| SourceSession::result_bytes(val).map(|b| b.to_vec()),
        )
    }

    /// Evaluates a module's `output.value` decoded into [`PklValue`] (`Evaluator.evaluateOutputValue`).
    pub fn evaluate_output_value(&mut self, source: &ModuleSource) -> Result<PklValue> {
        let bytes = self.evaluate_expression_pkl_binary(source, "output.value")?;
        PklBinaryDecoder::decode(&bytes)
    }

    /// Evaluates a module's `output.files` (`Evaluator.evaluateOutputFiles`), returning entries
    /// keyed by relative path specification.
    pub fn evaluate_output_files(
        &mut self,
        source: &ModuleSource,
    ) -> Result<BTreeMap<String, FileOutput>> {
        let ordered = self.evaluate_output_files_ordered(source)?;
        Ok(ordered.into_iter().collect())
    }

    /// Evaluates a module's `output.files` preserving Pkl `Mapping` declaration order.
    pub fn evaluate_output_files_ordered(
        &mut self,
        source: &ModuleSource,
    ) -> Result<Vec<(String, FileOutput)>> {
        const EXPR: &str = "let (f = output.files) if (f == null) Map() else f.toMap().mapValues((_, v) -> v.bytes)";
        let bytes = self.evaluate_expression_pkl_binary(source, EXPR)?;
        let mut decoded = decode_pkl_binary(&bytes)
            .map_err(|err| PklError::decode(format!("Failed to decode output.files: {err}")))?;
        let mut out = Vec::new();
        if let DecodedValue::Map(entries) | DecodedValue::Mapping(entries) = &mut decoded {
            for (k, mut v) in entries.drain(..) {
                let key = match &k {
                    DecodedValue::String(s) => s.to_java_utf8(),
                    other => other.to_java_key_string().to_java_utf8(),
                };
                let raw_bytes = match &mut v {
                    DecodedValue::Bytes(b) => std::mem::take(b),
                    DecodedValue::String(s) => s.to_java_utf8().into_bytes(),
                    other => {
                        return Err(PklError::decode(format!(
                            "Unexpected FileOutput.bytes value for `{key}`: {other:?}"
                        )));
                    }
                };
                out.push((key, FileOutput::from_bytes(raw_bytes)));
            }
        }
        Ok(out)
    }

    /// Evaluates an arbitrary Pkl expression against `source` and decodes it into [`PklValue`]
    /// (`Evaluator.evaluateExpressionRaw`).
    pub fn evaluate_expression(
        &mut self,
        source: &ModuleSource,
        expression: &str,
    ) -> Result<PklValue> {
        let bytes = self.evaluate_expression_pkl_binary(source, expression)?;
        PklBinaryDecoder::decode(&bytes)
    }

    /// Evaluates an arbitrary Pkl expression against `source` and returns its string representation
    /// (`Evaluator.evaluateExpressionString`).
    pub fn evaluate_expression_string(
        &mut self,
        source: &ModuleSource,
        expression: &str,
    ) -> Result<String> {
        self.evaluate_string_warm_or_cold(source, expression)
    }

    /// Evaluates an arbitrary Pkl expression against `source` and returns raw `pkl-binary`
    /// MessagePack bytes (`Evaluator.evaluateExpressionPklBinary`).
    pub fn evaluate_expression_pkl_binary(
        &mut self,
        source: &ModuleSource,
        expression: &str,
    ) -> Result<Vec<u8>> {
        let spec = self.prepare_spec(source)?;
        self.run_eval_result(
            |session, now, timeout| {
                session.start_evaluation_pkl_binary(spec, expression, now, timeout)
            },
            |val| SourceSession::result_bytes(val).map(|b| b.to_vec()),
        )
    }

    /// Extracts the [`ModuleSchema`] of `source` (`Evaluator.evaluateSchema`).
    pub fn evaluate_schema(&mut self, source: &ModuleSource) -> Result<ModuleSchema> {
        self.ensure_open()?;
        let prev_deadline = self.active_deadline_ms;
        if self.active_deadline_ms.is_none() {
            self.active_deadline_ms = self
                .host
                .timeout_ms()
                .map(|ms| self.host.clock_ms().saturating_add(ms));
        }
        self.composite_eval_depth += 1;
        let outcome = self.evaluate_schema_inner(source);
        self.composite_eval_depth -= 1;
        self.active_deadline_ms = prev_deadline;
        self.reset_cancel_if_top_level();
        outcome
    }

    fn evaluate_schema_inner(&mut self, source: &ModuleSource) -> Result<ModuleSchema> {
        let spec = self.prepare_spec(source)?;
        let module_val = self.run_eval_result(
            |session, now, timeout| session.start_load_module(spec, now, timeout),
            Ok,
        )?;
        let mut stderr = Vec::new();
        let mut extractor =
            SchemaExtractor::new(module_val).map_err(|err| self.map_runtime_err(err))?;
        let mut last_result = None;
        loop {
            let (now, rem_timeout) = self.effective_clock_and_timeout()?;
            match extractor
                .step(&mut self.session, last_result, now, rem_timeout)
                .map_err(|err| self.map_runtime_err(err))?
            {
                SchemaStep::RunPending => {
                    let (_, drive_timeout) = match self.effective_clock_and_timeout() {
                        Ok(pair) => pair,
                        Err(err) => {
                            self.session.abort_active_operation();
                            return Err(err);
                        }
                    };
                    last_result = Some(
                        self.host
                            .drive_public_session_with_timeout(
                                &mut self.session,
                                drive_timeout,
                                &mut stderr,
                            )
                            .map_err(|err| self.map_runtime_err(err))?,
                    );
                }
                SchemaStep::Done(schema) => return Ok(schema),
            }
        }
    }

    /// Runs a `pkl:test` module (`Evaluator.evaluateTest`), evaluating `facts` (with optional
    /// power-assertion diagrams) and `examples` (with `*-expected.pcf` / `*-actual.pcf` I/O
    /// and `--overwrite` support).
    pub fn evaluate_test(&mut self, source: &ModuleSource, overwrite: bool) -> Result<TestResults> {
        self.ensure_open()?;
        struct CaptureGuard {
            logger: Arc<TestCapturingLogger>,
            finished: bool,
        }
        impl CaptureGuard {
            fn finish(mut self) -> String {
                self.finished = true;
                self.logger.end_capture()
            }
        }
        impl Drop for CaptureGuard {
            fn drop(&mut self) {
                if !self.finished {
                    let _ = self.logger.end_capture();
                }
            }
        }
        self.test_logger.begin_capture();
        let guard = CaptureGuard {
            logger: self.test_logger.clone(),
            finished: false,
        };
        let prev_deadline = self.active_deadline_ms;
        if self.active_deadline_ms.is_none() {
            self.active_deadline_ms = self
                .host
                .timeout_ms()
                .map(|ms| self.host.clock_ms().saturating_add(ms));
        }
        self.composite_eval_depth += 1;
        let outcome = self.evaluate_test_inner(source, overwrite);
        self.composite_eval_depth -= 1;
        self.active_deadline_ms = prev_deadline;
        self.reset_cancel_if_top_level();
        let logs = guard.finish();
        match outcome {
            Ok(mut results) => {
                results.logs = logs;
                Ok(results)
            }
            Err(err) => Err(err),
        }
    }

    fn format_test_location(&self, uri: &str, line: Option<usize>) -> String {
        if self.test_mode || self.diagnostic_profile != DiagnosticProfile::CliBundledSettings {
            return match line {
                Some(l) if uri.starts_with("file:") => format!("{uri}, line {l}"),
                _ => uri.to_string(),
            };
        }
        if uri.starts_with("file:") {
            match line {
                Some(l) => self
                    .editor_url_scheme
                    .replace("%{url}", uri)
                    .replace("%{line}", &l.to_string()),
                None => uri.to_string(),
            }
        } else {
            uri.to_string()
        }
    }

    fn evaluate_test_inner(
        &mut self,
        source: &ModuleSource,
        overwrite: bool,
    ) -> Result<TestResults> {
        let uri_str = source.get_uri().to_string();
        // 1. Load module name + check whether it amends `pkl:test`.
        //    Syntax/load errors propagate as `Err(PklError)` (matching `CliTestRunner.kt:114`).
        let check_expr = r#"
let (reflect = import("pkl:reflect"))
let (testMod = import("pkl:test"))
let (m = reflect.Module(module))
let (isTest = reflect.Class(module.getClass()).isSubclassOf(reflect.Class(testMod.getClass())))
new Mapping {
  ["moduleName"] = m.name
  ["moduleUri"] = m.uri
  ["isTest"] = isTest
}
"#;
        let check_bytes = self.evaluate_expression_pkl_binary(source, check_expr)?;
        let check_decoded = decode_pkl_binary(&check_bytes)
            .map_err(|err| PklError::decode(format!("Failed to decode test check: {err}")))?;
        let (module_name, module_uri, is_test) = match &check_decoded {
            DecodedValue::Mapping(entries) | DecodedValue::Map(entries) => {
                let get_s = |k: &str| {
                    entries
                        .iter()
                        .find(|(ek, _)| ek.to_java_key_string().as_str() == Some(k))
                        .and_then(|(_, v)| match v {
                            DecodedValue::String(s) => Some(s.to_java_utf8()),
                            _ => None,
                        })
                };
                let get_b = |k: &str| {
                    entries
                        .iter()
                        .find(|(ek, _)| ek.to_java_key_string().as_str() == Some(k))
                        .and_then(|(_, v)| match v {
                            DecodedValue::Bool(b) => Some(*b),
                            _ => None,
                        })
                };
                (
                    get_s("moduleName").unwrap_or_else(|| "test".to_string()),
                    get_s("moduleUri").unwrap_or_else(|| uri_str.clone()),
                    get_b("isTest").unwrap_or(false),
                )
            }
            _ => ("test".to_string(), uri_str.clone(), false),
        };

        let display_uri = self.format_test_location(&module_uri, None);

        if !is_test {
            // Mirrors `TestRunner.java:71-77` (`VmUtils.checkAmends(testModule, TestModule.getModule().getVmClass())`):
            // frameless `VmEvalException` with uncolored `Value: <moduleName>` program value.
            let err_msg = pkl_runtime::diagnostic::render_error_text_with_trailer_colored(
                &"Expected value of type `pkl.test`, but got type `Class`.".into(),
                &format!("Value: {module_name}").into(),
                &[],
                None,
                self.use_color,
            )
            .to_java_utf8();
            let short_msg = extract_short_error_message(&err_msg);
            return Ok(TestResults {
                module_name,
                display_uri,
                facts: TestSectionResults::empty(TestSectionName::Facts),
                examples: TestSectionResults::empty(TestSectionName::Examples),
                logs: String::new(),
                error: Some(TestError {
                    message: short_msg,
                    rendered: err_msg,
                }),
                generated_expected_pcf: None,
                generated_actual_pcf: None,
            });
        }

        // Load module text for source spans / line numbers (best-effort for file: or inline text).
        let module_source_text: Option<String> = if let Some(inline) = source.contents() {
            Some(inline.to_string())
        } else if module_uri.starts_with("file:") {
            let uri_pkl = PklString::from(module_uri.as_str());
            self.host
                .fs()
                .resolve_file_module(&uri_pkl)
                .ok()
                .and_then(|(resolved, real_path)| {
                    if resolved.resolved_uri != uri_pkl
                        && self
                            .host
                            .check_resolve_module(&resolved.resolved_uri)
                            .is_err()
                    {
                        return None;
                    }
                    self.host.fs().read_file_module_bytes(&real_path).ok()
                })
                .and_then(|bytes| String::from_utf8(bytes).ok())
        } else {
            None
        };

        // 2. Run `facts` section (element-by-element so errors in one fact don't abort others).
        let facts = self.run_test_facts(source, &module_uri, module_source_text.as_deref())?;

        // 3. Run `examples` section.
        let (examples, gen_exp, gen_act) = self.run_test_examples(
            source,
            &module_uri,
            module_source_text.as_deref(),
            overwrite,
        )?;

        Ok(TestResults {
            module_name,
            display_uri,
            facts,
            examples,
            logs: String::new(),
            error: None,
            generated_expected_pcf: gen_exp,
            generated_actual_pcf: gen_act,
        })
    }

    fn run_test_facts(
        &mut self,
        source: &ModuleSource,
        module_uri: &str,
        module_source_text: Option<&str>,
    ) -> Result<TestSectionResults> {
        let shape_expr = "if (facts == null) null else facts.toMap().mapValues((_, g) -> g.length)";
        let shape_bytes = self.evaluate_expression_pkl_binary(source, shape_expr)?;
        let shape_decoded = decode_pkl_binary(&shape_bytes)
            .map_err(|err| PklError::decode(format!("Failed to decode facts shape: {err}")))?;
        let groups = match &shape_decoded {
            DecodedValue::Null => return Ok(TestSectionResults::empty(TestSectionName::Facts)),
            DecodedValue::Map(entries) | DecodedValue::Mapping(entries) => entries,
            _ => return Ok(TestSectionResults::empty(TestSectionName::Facts)),
        };

        let fact_spans = module_source_text
            .map(extract_fact_spans)
            .unwrap_or_default();
        let mut results = Vec::with_capacity(groups.len());

        for (group_key, len_val) in groups {
            let group_name = group_key.to_java_key_string().to_java_utf8();
            let count = match len_val {
                DecodedValue::Int(n) if *n >= 0 => *n as usize,
                _ => 0,
            };
            let group_locs = fact_spans.get(&group_name);
            let mut total_asserts = 0usize;
            let mut failures = Vec::new();
            let mut errors = Vec::new();

            for idx in 0..count {
                let expr = format!("facts[{}][{idx}]", pkl_escape_str(&group_name));
                total_asserts += 1;
                match self.evaluate_expression_pkl_binary(source, &expr) {
                    Ok(bytes) => {
                        let decoded = decode_pkl_binary(&bytes).map_err(|err| {
                            PklError::decode(format!("Failed to decode fact value: {err}"))
                        })?;
                        if matches!(decoded, DecodedValue::Bool(false)) {
                            let failure = self.build_fact_failure(
                                source,
                                module_uri,
                                group_locs.and_then(|v| v.get(idx)),
                                idx,
                            )?;
                            failures.push(failure);
                        }
                    }
                    Err(err)
                        if matches!(
                            err.kind(),
                            crate::PklErrorKind::Timeout
                                | crate::PklErrorKind::Cancelled
                                | crate::PklErrorKind::Closed
                        ) =>
                    {
                        return Err(err);
                    }
                    Err(err) => {
                        let rendered = err.message().to_string();
                        let short = extract_short_error_message(&rendered);
                        errors.push(TestError {
                            message: short,
                            rendered,
                        });
                    }
                }
            }

            results.push(TestResult {
                name: group_name,
                total_asserts,
                failures,
                errors,
                is_example_written: false,
            });
        }

        Ok(TestSectionResults {
            name: TestSectionName::Facts,
            results,
        })
    }

    fn build_fact_failure(
        &mut self,
        source: &ModuleSource,
        module_uri: &str,
        loc_info: Option<&(String, usize, pkl_runtime::source::Span)>,
        idx: usize,
    ) -> Result<TestFailure> {
        let Some((expr_text, line, _span)) = loc_info else {
            let loc = self.format_test_location(module_uri, None);
            return Ok(format_fact_failure(
                &format!("fact #{idx} == false"),
                &loc,
                self.use_color,
            ));
        };
        let location = self.format_test_location(module_uri, Some(*line));

        if self.power_assertions {
            // Re-evaluate via a constrained type check so the VM's real power-assertion tracker
            // records all sub-expression observations for this failing fact.
            let probe_expr = format!("(null as Any({expr_text}))");
            if let Ok(spec) = self.prepare_spec(source) {
                let (now, rem_timeout) = self.effective_clock_and_timeout()?;
                let mut stderr = Vec::new();
                if self
                    .session
                    .start_evaluation_pkl_binary(spec, &probe_expr, now, rem_timeout)
                    .is_ok()
                {
                    let res = self.host.drive_public_session_with_timeout(
                        &mut self.session,
                        rem_timeout,
                        &mut stderr,
                    );
                    if let Err(err) = res {
                        if matches!(
                            err.kind,
                            ErrorKind::Timeout | ErrorKind::Cancelled | ErrorKind::Closed
                        ) {
                            if err.kind == ErrorKind::Cancelled && !self.closed {
                                self.host.cancel_flag().reset();
                            }
                            return Err(self.map_runtime_err(err));
                        }
                        if let Some(ref td) = err.type_diagnostic
                            && let Some((body_span, observations)) =
                                extract_constraint_observations(td)
                        {
                            let mut sm = SourceMap::default();
                            let sid = sm.add("repl:text", probe_expr.as_str());
                            if let Some(src) = sm.get(sid) {
                                return Ok(format_fact_failure_with_power_assertions(
                                    src,
                                    body_span,
                                    observations,
                                    &location,
                                    self.use_color,
                                ));
                            }
                        }
                    }
                }
            }
        }

        Ok(format_fact_failure(expr_text, &location, self.use_color))
    }

    fn run_test_examples(
        &mut self,
        source: &ModuleSource,
        module_uri: &str,
        module_source_text: Option<&str>,
        overwrite: bool,
    ) -> Result<(TestSectionResults, Option<String>, Option<String>)> {
        // Check if `examples == null`
        let is_null_bytes = self.evaluate_expression_pkl_binary(source, "examples == null")?;
        let is_null = matches!(
            decode_pkl_binary(&is_null_bytes),
            Ok(DecodedValue::Bool(true))
        );
        if is_null {
            return Ok((
                TestSectionResults::empty(TestSectionName::Examples),
                None,
                None,
            ));
        }

        if !module_uri.starts_with("file:") {
            return Err(PklError::evaluation(format!(
                "–– Pkl Error ––\nCannot evaluate examples for non-file-based test module `{module_uri}`.\n"
            )));
        }

        let module_uri_pkl = PklString::from(module_uri);
        self.host
            .check_resolve_module(&module_uri_pkl)
            .map_err(|err| PklError::security(format_module_host_error(&err)))?;

        let module_path = file_uri_to_path(&module_uri_pkl).map_err(|err| {
            PklError::evaluation(format!("Invalid file URI `{module_uri}`: {err}"))
        })?;
        let file_name = module_path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let expected_path = module_path.with_file_name(format!("{file_name}-expected.pcf"));
        let actual_path = module_path.with_file_name(format!("{file_name}-actual.pcf"));
        let expected_uri_pkl = path_to_file_uri(&expected_path);
        let actual_uri_pkl = path_to_file_uri(&actual_path);
        self.host
            .check_resolve_module(&expected_uri_pkl)
            .map_err(|err| PklError::security(format_module_host_error(&err)))?;
        self.host
            .check_resolve_module(&actual_uri_pkl)
            .map_err(|err| PklError::security(format_module_host_error(&err)))?;
        let canon_expected = pkl_host_native::try_resolve_canonical_or_ancestor(&expected_path)
            .map_err(|()| {
                PklError::security(
                    pkl_runtime::host::security::root_dir_denied_message(&expected_uri_pkl, false)
                        .to_java_utf8(),
                )
            })?;
        let canon_expected_uri = path_to_file_uri(&canon_expected);
        if canon_expected_uri != expected_uri_pkl {
            self.host
                .check_resolve_module(&canon_expected_uri)
                .map_err(|err| PklError::security(format_module_host_error(&err)))?;
        }
        let canon_actual = pkl_host_native::try_resolve_canonical_or_ancestor(&actual_path)
            .map_err(|()| {
                PklError::security(
                    pkl_runtime::host::security::root_dir_denied_message(&actual_uri_pkl, false)
                        .to_java_utf8(),
                )
            })?;
        let canon_actual_uri = path_to_file_uri(&canon_actual);
        if canon_actual_uri != actual_uri_pkl {
            self.host
                .check_resolve_module(&canon_actual_uri)
                .map_err(|err| PklError::security(format_module_host_error(&err)))?;
        }

        match std::fs::remove_file(&actual_path) {
            Ok(()) => {}
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => {
                return Err(PklError::io(format!(
                    "I/O error writing test output file `{}`: {err}",
                    actual_path.display()
                )));
            }
        }
        if overwrite {
            match std::fs::remove_file(&expected_path) {
                Ok(()) => {}
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
                Err(err) => {
                    return Err(PklError::io(format!(
                        "I/O error writing test output file `{}`: {err}",
                        expected_path.display()
                    )));
                }
            }
        }

        // Evaluate actual examples rendered to PCF comparison strings + full PCF document
        let rendered_expr = RENDER_EXAMPLES_PKL_BINARY_EXPR;
        // Note: RENDER_EXAMPLES_PKL_BINARY_EXPR uses `output.renderer` with pkl-binary.
        // Using direct `evaluate_expression_pkl_binary` with a cleaner pure expression avoids
        // requiring `prop:pkl.outputFormat = "pkl-binary"`!
        let _ = rendered_expr;
        let actual_map_expr = r#"
let (r = new PcfRenderer { omitNullProperties = false })
examples.toMap().mapValues((_, g) ->
  g.toList().map((v) ->
    let (s = r.renderValue(v))
    if (v is Typed || v is Dynamic || v is Listing || v is Mapping) "new " + s else s
  )
)
"#;
        let actual_bytes = self.evaluate_expression_pkl_binary(source, actual_map_expr)?;
        let actual_decoded = decode_pkl_binary(&actual_bytes)
            .map_err(|err| PklError::decode(format!("Failed to decode examples: {err}")))?;
        let actual_groups = decode_rendered_examples_map(Some(&actual_decoded));

        let full_doc = self.evaluate_expression_string(source, RENDER_EXAMPLES_DOC_EXPR)?;
        let expected_uri = path_to_file_uri(&expected_path).to_java_utf8();
        let actual_uri = path_to_file_uri(&actual_path).to_java_utf8();

        let (expected_groups, expected_source) = match std::fs::symlink_metadata(&expected_path) {
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => (None, None),
            Err(err) => {
                return Err(PklError::io(format!(
                    "I/O error reading test output file `{}`: {err}",
                    expected_path.display()
                )));
            }
            Ok(meta) if meta.is_dir() => (None, None),
            Ok(_) => {
                let raw_bytes_opt = match self.host.fs().resolve_file_module(&expected_uri_pkl) {
                    Ok((resolved, real_path)) => {
                        if resolved.resolved_uri != expected_uri_pkl {
                            self.host
                                .check_resolve_module(&resolved.resolved_uri)
                                .map_err(|err| {
                                    PklError::security(format_module_host_error(&err))
                                })?;
                        }
                        match self.host.fs().read_file_module_bytes(&real_path) {
                            Ok(b) => Some(b),
                            Err(pkl_runtime::modules::ModuleHostError::NotFound) => None,
                            Err(
                                err @ pkl_runtime::modules::ModuleHostError::SecurityManagerException {
                                    ..
                                },
                            ) => {
                                return Err(PklError::security(format_module_host_error(&err)));
                            }
                            Err(err) => {
                                return Err(PklError::io(format!(
                                    "I/O error reading test output file `{}`: {}",
                                    expected_path.display(),
                                    format_module_host_error(&err)
                                )));
                            }
                        }
                    }
                    Err(pkl_runtime::modules::ModuleHostError::NotFound) => None,
                    Err(
                        err @ pkl_runtime::modules::ModuleHostError::SecurityManagerException {
                            ..
                        },
                    ) => {
                        return Err(PklError::security(format_module_host_error(&err)));
                    }
                    Err(err) => {
                        return Err(PklError::io(format!(
                            "I/O error reading test output file `{}`: {}",
                            expected_path.display(),
                            format_module_host_error(&err)
                        )));
                    }
                };
                if let Some(raw_bytes) = raw_bytes_opt {
                    let exp_text = String::from_utf8(raw_bytes).map_err(|err| {
                        PklError::io(format!(
                            "I/O error reading test output file `{}`: {err}",
                            expected_path.display()
                        ))
                    })?;
                    // Evaluate on a fresh short-lived session sharing `self.host` so repeated
                    // `evaluate_test` calls on the same `Evaluator` never read stale cached
                    // `*-expected.pcf` contents while preserving `allowedModules` (no `repl:` needed).
                    let exp_spec = pkl_runtime::modules::ModuleSourceSpec {
                        uri: expected_uri_pkl.clone(),
                        contents: Some(exp_text.as_str().into()),
                    };
                    let (now, rem_timeout) = self.effective_clock_and_timeout()?;
                    let mut exp_session =
                        self.host.new_public_session(DiagnosticProfile::ApiIdentity);
                    let mut exp_stderr = Vec::new();
                    let exp_eval =
                        (|| -> std::result::Result<Vec<u8>, pkl_runtime::value::RuntimeError> {
                            exp_session.start_evaluation_pkl_binary(
                                exp_spec,
                                r#"
let (r = new PcfRenderer { omitNullProperties = false })
let (ex = examples)
if (ex == null) throw("Invalid output file structure")
else ex.toMap().mapValues((_, g) ->
  g.toList().map((v) ->
    let (s = r.renderValue(v))
    if (v is Typed || v is Dynamic || v is Listing || v is Mapping) "new " + s else s
  )
)
"#,
                                now,
                                rem_timeout,
                            )?;
                            let val = self.host.drive_public_session_with_timeout(
                                &mut exp_session,
                                rem_timeout,
                                &mut exp_stderr,
                            )?;
                            SourceSession::result_bytes(val).map(|b| b.to_vec())
                        })();
                    let exp_bytes = match exp_eval {
                        Ok(b) => {
                            exp_session.close();
                            b
                        }
                        Err(err) => {
                            let mapped = match err.kind {
                                ErrorKind::Timeout => PklError::timeout(format_timeout_message(
                                    self.host.timeout_ms().unwrap_or(0),
                                )),
                                ErrorKind::Cancelled => {
                                    PklError::cancelled(err.message.to_java_utf8())
                                }
                                ErrorKind::Closed => PklError::closed(err.message.to_java_utf8()),
                                _ => {
                                    let is_security = err.is_security_error();
                                    let rendered = exp_session.format_error(err);
                                    if is_security {
                                        PklError::security(rendered)
                                    } else {
                                        PklError::evaluation(rendered)
                                    }
                                }
                            };
                            exp_session.close();
                            return Err(mapped);
                        }
                    };
                    let exp_decoded = decode_pkl_binary(&exp_bytes).map_err(|err| {
                        PklError::decode(format!("Failed to decode expected examples: {err}"))
                    })?;
                    (
                        Some(decode_rendered_examples_map(Some(&exp_decoded))),
                        Some(exp_text),
                    )
                } else {
                    (None, None)
                }
            }
        };

        let (section, gen_expected, gen_actual) = evaluate_examples_rendered_detailed(
            &actual_groups,
            expected_groups.as_deref(),
            module_uri,
            &expected_uri,
            &actual_uri,
            Some(full_doc),
            module_source_text,
            expected_source.as_deref(),
            self.use_color,
            |u, l| self.format_test_location(u, l),
        );

        if let Some(ref content) = gen_expected {
            pkl_host_native::write_file_nofollow(&expected_path, content.as_bytes()).map_err(
                |err| {
                    PklError::io(format!(
                        "I/O error writing test output file `{}`: {err}",
                        expected_path.display()
                    ))
                },
            )?;
        }
        if let Some(ref content) = gen_actual {
            pkl_host_native::write_file_nofollow(&actual_path, content.as_bytes()).map_err(
                |err| {
                    PklError::io(format!(
                        "I/O error writing test output file `{}`: {err}",
                        actual_path.display()
                    ))
                },
            )?;
        }

        Ok((section, gen_expected, gen_actual))
    }

    /// Parses a `pkl:Command` module specification (`Evaluator.evaluateCommand` step 1).
    pub fn evaluate_command_spec(
        &mut self,
        source: &ModuleSource,
        reserved_flags: &BTreeSet<String>,
        reserved_shorts: &BTreeSet<String>,
    ) -> Result<CommandSpec> {
        let bytes = self.evaluate_expression_pkl_binary(source, COMMAND_SPEC_REFLECT_EXPR)?;
        let decoded = decode_pkl_binary(&bytes)
            .map_err(|err| PklError::decode(format!("Failed to decode CommandSpec: {err}")))?;
        let source_text = if let Some(inline) = source.contents() {
            Some(inline.to_string())
        } else if source.get_uri().starts_with("file:") {
            let uri_pkl = PklString::from(source.get_uri());
            self.host
                .fs()
                .resolve_file_module(&uri_pkl)
                .ok()
                .and_then(|(resolved, real_path)| {
                    if resolved.resolved_uri != uri_pkl
                        && self
                            .host
                            .check_resolve_module(&resolved.resolved_uri)
                            .is_err()
                    {
                        return None;
                    }
                    self.host.fs().read_file_module_bytes(&real_path).ok()
                })
                .and_then(|bytes| String::from_utf8(bytes).ok())
        } else {
            None
        };
        let meta = source_text
            .as_deref()
            .map(extract_source_command_metadata)
            .unwrap_or_default();
        decode_command_spec(&decoded, &meta, reserved_flags, reserved_shorts)
            .map_err(PklError::evaluation)
    }

    /// Evaluates an invoked `pkl:Command` chain (`output.bytes` and `output.files`).
    pub fn evaluate_command_execution(
        &mut self,
        root_module_uri: &str,
        levels: &[InvokedCommandLevel],
        working_dir_uri: &str,
    ) -> Result<CommandEvalResult> {
        self.ensure_open()?;
        let prev_deadline = self.active_deadline_ms;
        if self.active_deadline_ms.is_none() {
            self.active_deadline_ms = self
                .host
                .timeout_ms()
                .map(|ms| self.host.clock_ms().saturating_add(ms));
        }
        self.composite_eval_depth += 1;
        let outcome = (|| -> Result<CommandEvalResult> {
            let synth_pkl =
                synthesize_command_eval_module(root_module_uri, levels, working_dir_uri);
            let synth_source = ModuleSource::text(synth_pkl);
            let output_bytes = self.evaluate_output_bytes(&synth_source)?;
            let output_files = self.evaluate_output_files_ordered(&synth_source)?;
            Ok(CommandEvalResult {
                output_bytes,
                output_files,
            })
        })();
        self.composite_eval_depth -= 1;
        self.active_deadline_ms = prev_deadline;
        self.reset_cancel_if_top_level();
        outcome
    }
}

impl Drop for Evaluator {
    fn drop(&mut self) {
        self.close();
    }
}

fn extract_constraint_observations(
    td: &TypeDiagnostic,
) -> Option<(
    pkl_runtime::source::Span,
    &[pkl_runtime::diagnostic::PowerAssertionValue],
)> {
    match td {
        TypeDiagnostic::AtSource { diagnostic, .. } => extract_constraint_observations(diagnostic),
        TypeDiagnostic::Constraint {
            body_span,
            observations: Some(obs),
            ..
        } => Some((*body_span, obs.as_slice())),
        _ => None,
    }
}

fn extract_short_error_message(rendered: &str) -> String {
    let stripped = pkl_runtime::diagnostic::ansi::strip_ansi_codes(rendered);
    let body = stripped
        .strip_prefix("–– Pkl Error ––\n")
        .unwrap_or(&stripped);
    body.split("\n\n")
        .next()
        .unwrap_or(body)
        .trim_end_matches('\n')
        .to_string()
}

fn pkl_escape_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evaluate_test_runs_passing_and_failing_pkl_test_modules() {
        let mut evaluator = EvaluatorBuilder::preconfigured()
            .with_module_cache_dir(None)
            .build()
            .expect("build evaluator");

        let passing = ModuleSource::text(
            "amends \"pkl:test\"\n\nfacts {\n  [\"math\"] {\n    1 + 1 == 2\n    2 * 3 == 6\n  }\n}\n",
        );
        let res = evaluator
            .evaluate_test(&passing, false)
            .expect("evaluate passing pkl:test");
        assert!(!res.failed(), "unexpected failures: {res:?}");
        assert_eq!(res.total_tests(), 1);
        assert_eq!(res.total_asserts(), 2);
        assert_eq!(res.total_failures(), 0);

        let failing = ModuleSource::text(
            "amends \"pkl:test\"\n\nfacts {\n  [\"broken\"] {\n    1 == 2\n  }\n}\n",
        );
        let res = evaluator
            .evaluate_test(&failing, false)
            .expect("evaluate failing pkl:test");
        assert!(res.failed());
        assert_eq!(res.total_tests(), 1);
        assert_eq!(res.total_failures(), 1);

        let non_test = ModuleSource::text("x = 1\n");
        let res = evaluator
            .evaluate_test(&non_test, false)
            .expect("evaluate non-test module");
        let err = res
            .error
            .as_ref()
            .expect("non-test module must report test error");
        assert_eq!(
            err.rendered,
            "–– Pkl Error ––\nExpected value of type `pkl.test`, but got type `Class`.\nValue: text\n"
        );
        assert_eq!(
            err.message,
            "Expected value of type `pkl.test`, but got type `Class`.\nValue: text"
        );

        evaluator.close();
    }

    #[test]
    fn round2_vm_dos_and_correctness_regressions() {
        let mut evaluator = EvaluatorBuilder::preconfigured()
            .with_module_cache_dir(None)
            .build()
            .expect("build evaluator");

        // 1. Exponential typealias DAG hits node/depth guard cleanly instead of OOM/hang.
        let mut dag = String::from("typealias T0 = Int | String\n");
        for i in 1..=22 {
            dag.push_str(&format!("typealias T{i} = T{} | T{}\n", i - 1, i - 1));
        }
        dag.push_str("x: T22 = 1\n");
        assert!(
            evaluator
                .evaluate_output_text(&ModuleSource::text(dag))
                .is_err(),
            "exponential typealias DAG must fail cleanly"
        );

        // 2. List.repeat preserves virtual length, materializes normal repeats accurately,
        //    and rejects oversized materializations explicitly instead of truncating.
        let rep_ok = evaluator
            .evaluate_expression(
                &ModuleSource::text(
                    "a = List(1, 2).repeat(3)\nb = List(0).repeat(118866785).length\n",
                ),
                "Pair(a, b)",
            )
            .expect("evaluate List.repeat");
        assert_eq!(
            rep_ok,
            crate::PklValue::Pair(
                Box::new(crate::PklValue::List(vec![
                    crate::PklValue::Int(1),
                    crate::PklValue::Int(2),
                    crate::PklValue::Int(1),
                    crate::PklValue::Int(2),
                    crate::PklValue::Int(1),
                    crate::PklValue::Int(2),
                ])),
                Box::new(crate::PklValue::Int(118_866_785)),
            )
        );
        assert!(
            evaluator
                .evaluate_expression_pkl_binary(
                    &ModuleSource::text("huge = List(1).repeat(3000000)\n"),
                    "huge",
                )
                .is_err(),
            "oversized List.repeat materialization must fail explicitly"
        );

        // 3. YAML nesting depth > 256 fails cleanly without stack overflow.
        let deep_yaml = format!(
            "import \"pkl:yaml\"\nres = new yaml.Parser {{}}.parse({:?})\n",
            format!("{}{}", "[".repeat(300), "]".repeat(300))
        );
        assert!(
            evaluator
                .evaluate_output_text(&ModuleSource::text(deep_yaml))
                .is_err(),
            "YAML nesting depth > 256 must fail cleanly"
        );

        // 4. Flat YAML Dynamic mapping with 1500 keys stays linear (no O(N^2) blowup).
        let mut flat_yaml = String::new();
        for i in 0..1500 {
            flat_yaml.push_str(&format!("k{i}: {i}\n"));
        }
        let yaml_src = format!(
            "import \"pkl:yaml\"\nd = new yaml.Parser {{}}.parse({flat_yaml:?})\nlast = d.k1499\n"
        );
        let last = evaluator
            .evaluate_expression(&ModuleSource::text(yaml_src), "last")
            .expect("flat YAML dynamic mapping parses");
        assert_eq!(last, crate::PklValue::Int(1499));

        evaluator.close();
    }

    #[test]
    fn round3_evaluator_reusable_timeout_typed_security_and_module_path() {
        // 1. P2-10: ModuleSource::module_path normalizes bare `modulepath:` prefix.
        assert_eq!(
            ModuleSource::module_path("modulepath:foo/bar.pkl").get_uri(),
            "modulepath:/foo/bar.pkl"
        );
        assert_eq!(
            ModuleSource::module_path("modulepath:/foo/bar.pkl").get_uri(),
            "modulepath:/foo/bar.pkl"
        );

        // 2. P0-7 & P1-14: Timeout aborts only the active evaluation so the evaluator stays reusable,
        //    and user throw("Refusing to ...") stays Evaluation (not Security).
        let mut evaluator = EvaluatorBuilder::preconfigured()
            .with_module_cache_dir(None)
            .with_timeout(Duration::from_millis(25))
            .build()
            .expect("build evaluator");

        let slow = ModuleSource::text(
            "function fib(n: Int): Int = if (n <= 1) n else fib(n - 1) + fib(n - 2)\nx = fib(32)\n",
        );
        let timeout_err = evaluator
            .evaluate_expression(&slow, "x")
            .expect_err("slow fib must time out");
        assert_eq!(timeout_err.kind(), crate::PklErrorKind::Timeout);
        assert!(!evaluator.is_closed());

        // Subsequent evaluation on the same evaluator succeeds!
        let ok = evaluator
            .evaluate_expression(&ModuleSource::text("x = 20 + 22\n"), "x")
            .expect("evaluator must remain reusable after timeout");
        assert_eq!(ok, crate::PklValue::Int(42));

        // User throw with "Refusing to " text must remain Evaluation, not Security.
        let throw_err = evaluator
            .evaluate_output_text(&ModuleSource::text(
                "x = throw(\"Refusing to eat broccoli\")\n",
            ))
            .expect_err("throw must error");
        assert_eq!(throw_err.kind(), crate::PklErrorKind::Evaluation);

        evaluator.close();
    }

    #[test]
    fn round5_evaluate_test_expected_pcf_uses_allowed_modules_and_refreshes_cache() {
        let dir = std::env::temp_dir().join(format!(
            "pkl-r5-eval-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).expect("create temp test dir");
        let test_path = dir.join("sample.pkl");
        let expected_path = dir.join("sample.pkl-expected.pcf");
        std::fs::write(
            &test_path,
            "amends \"pkl:test\"\n\nexamples {\n  [\"e\"] {\n    1\n  }\n}\n",
        )
        .expect("write test module");
        std::fs::write(&expected_path, "examples {\n  [\"e\"] {\n    1\n  }\n}\n")
            .expect("write expected pcf");

        // Intentionally omit `file:` from allowed_resources: `*-expected.pcf` must be gated by
        // `allowed_modules` (matching upstream `TestRunner.java:371-379`).
        let mut evaluator = EvaluatorBuilder::preconfigured()
            .with_module_cache_dir(None)
            .with_allowed_modules(vec!["pkl:".into(), "file:".into(), "repl:".into()])
            .with_allowed_resources(vec!["prop:".into()])
            .build()
            .expect("build evaluator");

        let src = ModuleSource::path(&test_path);
        let first = evaluator
            .evaluate_test(&src, false)
            .expect("first evaluate_test");
        assert!(!first.failed(), "expected first run to pass: {first:?}");

        // Modify `*-expected.pcf` on disk and re-run on the SAME evaluator: must see updated file.
        std::fs::write(&expected_path, "examples {\n  [\"e\"] {\n    2\n  }\n}\n")
            .expect("update expected pcf");
        let second = evaluator
            .evaluate_test(&src, false)
            .expect("second evaluate_test");
        assert!(
            second.failed(),
            "expected second run to observe modified *-expected.pcf, got: {second:?}"
        );

        evaluator.close();
        let _ = std::fs::remove_dir_all(&dir);
    }
}
