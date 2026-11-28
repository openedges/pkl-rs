//! Full product Node.js N-API bindings backed by `pkl-runtime`, `pkl-host-native`,
//! `pkl`, and `pkl-formatter` with non-blocking `AsyncTask` execution, 64-bit monotonic
//! clock/timeouts, lossless UTF-16/BigInt/Uint8Array boundaries, external readers,
//! projects/packages, custom JS extensions, formatting, and static import analysis.

use crate::bridge::{
    BridgeCallbackResponseIn, BridgeRequestJs, BridgeResponse, CapturingHostLogger, JsBridge,
    JsFrameTransformerAdapter, JsModuleReaderAdapter, JsModuleReaderSpec, JsResourceReaderAdapter,
    JsResourceReaderSpec, JsSecurityAdapter, JsSecurityHooks,
};
use crate::error::{
    NativeError, NativeErrorKind, define_own_prop, js_pkl_string, parse_u64_bigint, to_js_outcome,
    utf16_to_pkl, utf16_to_string,
};
use crate::schema::module_schema_to_js;
use crate::values::decoded_value_to_js;
use napi::{
    Env, Result, ScopedTask,
    bindgen_prelude::{
        Array, AsyncTask, BigInt, BufferSlice, Function, Null, Object, Uint8Array, Utf16String,
    },
    threadsafe_function::UnknownReturnValue,
};
use napi_derive::napi;
use pkl::{Color, ModuleSource, Project, TraceMode};
use pkl_formatter::{GrammarVersion, format_str};
use pkl_host_native::{
    CaCertSource, CancelFlag, HostFrameTransformer, HostLogger, NativeHost, NativeHostConfig,
    NativeHostExtensions, ScopedEvalCancel, canonicalize_root_dir, default_module_cache_dir,
    file_uri_to_path, path_to_file_uri, write_file_nofollow,
};
use pkl_runtime::{
    diagnostic::TypeDiagnostic,
    evaluation::{frontend::DiagnosticProfile, session::SourceSession},
    external_reader::ExternalReaderConfig,
    host::{
        HostPoll, format_timeout_message,
        http::{HttpClientConfig, NoProxyRule, ProxyAddress, ProxyConfig},
        security::SecurityPolicy,
    },
    logging::FrameTransformer,
    modules::{
        ModuleCapabilities, ModuleCompletion, ModuleHostError, ModuleKeyKind, ModuleRequestKind,
        ModuleSource as RuntimeModuleSource, ModuleSourceSpec, ResolvedModule, ResolvedModuleKey,
        analyze::{AnalysisPoll, ImportGraphAnalysis},
    },
    packages::{
        project::{DEFAULT_EDITOR_URL_SCHEME, ProjectDependenciesManager},
        test_runner::{
            RENDER_EXAMPLES_DOC_EXPR, TestError, TestFailure, TestReporterKind, TestResult,
            TestResults, TestSectionName, TestSectionResults, decode_rendered_examples_map,
            evaluate_examples_rendered_detailed, extract_fact_spans, format_fact_failure,
            format_fact_failure_with_power_assertions, render_junit_report, render_minimal_report,
            render_spec_report_colored, render_summary_colored,
        },
    },
    pkl_binary_decode::{DecodedValue, decode_lossless},
    schema::{ModuleSchema, SchemaExtractor, SchemaStep},
    source::{SourceMap, SourceText},
    value::{ErrorKind, PklString, RuntimeError, Value},
};
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::{
    Arc, Condvar, Mutex, OnceLock,
    atomic::{AtomicBool, AtomicU64, Ordering},
};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

static PROCESS_CLOCK_EPOCH: OnceLock<Instant> = OnceLock::new();

fn process_clock_ms() -> u64 {
    let epoch = PROCESS_CLOCK_EPOCH.get_or_init(Instant::now);
    epoch.elapsed().as_millis().min(u64::MAX as u128) as u64
}

/// Converts a filesystem directory path to a `file:///.../` URI with trailing slash.
pub(crate) fn path_to_dir_uri(dir: &Path) -> PklString {
    let abs = if dir.is_absolute() {
        dir.to_path_buf()
    } else {
        std::env::current_dir()
            .map(|cwd| cwd.join(dir))
            .unwrap_or_else(|_| dir.to_path_buf())
    };
    let mut uri = path_to_file_uri(&abs).to_java_utf8();
    if !uri.ends_with('/') {
        uri.push('/');
    }
    PklString::from(uri)
}

/// Overflow-safe wall-clock timeout watchdog supporting full 64-bit unsigned millisecond budgets
/// (including values `> 2^32 - 1` up to `u64::MAX`).
pub(crate) struct SafeTimeoutWatchdog {
    state: Arc<(Mutex<bool>, Condvar)>,
    timed_out: Arc<AtomicBool>,
    deadline: Option<Instant>,
    handle: Option<JoinHandle<()>>,
}

impl SafeTimeoutWatchdog {
    pub(crate) fn start(
        timeout_ms: Option<u64>,
        call_cancel: Arc<AtomicBool>,
        http_cancel: Arc<CancelFlag>,
        host: Arc<NativeHost>,
        bridge: Option<Arc<JsBridge>>,
    ) -> Self {
        let state = Arc::new((Mutex::new(false), Condvar::new()));
        let timed_out = Arc::new(AtomicBool::new(false));
        let Some(ms) = timeout_ms else {
            return Self {
                state,
                timed_out,
                deadline: None,
                handle: None,
            };
        };
        // Use checked_add so huge u64 budgets (e.g. > 2^44 ms or u64::MAX) never panic on macOS/Linux.
        let Some(deadline) = Instant::now().checked_add(Duration::from_millis(ms)) else {
            return Self {
                state,
                timed_out,
                deadline: None,
                handle: None,
            };
        };
        // Avoid spawning a sleeping thread for far-future deadlines (> 100 years) while still
        // passing the exact 64-bit timeout_ms to SourceSession.
        if ms > 3_153_600_000_000 {
            return Self {
                state,
                timed_out,
                deadline: Some(deadline),
                handle: None,
            };
        }

        let state_bg = Arc::clone(&state);
        let timed_out_bg = Arc::clone(&timed_out);
        let handle = thread::Builder::new()
            .name("pkl-napi-timeout".into())
            .spawn(move || {
                let (lock, cvar) = &*state_bg;
                let mut done = lock.lock().unwrap_or_else(|e| e.into_inner());
                while !*done {
                    let now = Instant::now();
                    if now >= deadline {
                        timed_out_bg.store(true, Ordering::SeqCst);
                        call_cancel.store(true, Ordering::SeqCst);
                        http_cancel.cancel();
                        host.external_readers().cancel_all();
                        if let Some(ref b) = bridge {
                            b.wake_all();
                        }
                        return;
                    }
                    let wait = deadline.saturating_duration_since(now);
                    let (next, res) = cvar
                        .wait_timeout(done, wait)
                        .unwrap_or_else(|e| e.into_inner());
                    done = next;
                    if res.timed_out() && !*done {
                        timed_out_bg.store(true, Ordering::SeqCst);
                        call_cancel.store(true, Ordering::SeqCst);
                        http_cancel.cancel();
                        host.external_readers().cancel_all();
                        if let Some(ref b) = bridge {
                            b.wake_all();
                        }
                        return;
                    }
                }
            })
            .ok();

        Self {
            state,
            timed_out,
            deadline: Some(deadline),
            handle,
        }
    }

    pub(crate) fn deadline(&self) -> Option<Instant> {
        self.deadline
    }

    pub(crate) fn triggered(&self) -> bool {
        self.timed_out.load(Ordering::SeqCst)
    }

    pub(crate) fn stop(mut self) -> bool {
        self.finish_inner();
        self.triggered()
    }

    fn finish_inner(&mut self) {
        let (lock, cvar) = &*self.state;
        {
            let mut done = lock.lock().unwrap_or_else(|e| e.into_inner());
            *done = true;
        }
        cvar.notify_all();
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for SafeTimeoutWatchdog {
    fn drop(&mut self) {
        self.finish_inner();
    }
}

#[napi(object)]
pub struct NativeKeyValue {
    pub key: Utf16String,
    pub value: Utf16String,
}

#[napi(object)]
pub struct NativeExternalReaderOption {
    pub scheme: Utf16String,
    pub executable: Utf16String,
    pub arguments: Option<Vec<Utf16String>>,
}

#[napi(object)]
pub struct NativeHttpHeaderEntry {
    pub url_pattern: Utf16String,
    pub name: Utf16String,
    pub value: Utf16String,
}

#[napi(object)]
pub struct NativeHttpOptions {
    pub ca_certificate_files: Option<Vec<Utf16String>>,
    pub ca_certificate_pems: Option<Vec<Uint8Array>>,
    pub proxy_address: Option<Utf16String>,
    pub no_proxy: Option<Vec<Utf16String>>,
    pub rewrites: Option<Vec<NativeKeyValue>>,
    pub headers: Option<Vec<NativeHttpHeaderEntry>>,
    pub test_port: Option<u32>,
}

#[napi(object)]
pub struct NativeModuleReaderSpecIn {
    pub index: u32,
    pub scheme: Utf16String,
    pub is_local: Option<bool>,
    pub is_globbable: Option<bool>,
    pub has_hierarchical_uris: Option<bool>,
    pub has_fragment_paths: Option<bool>,
    pub is_cached: Option<bool>,
    pub has_has_element: Option<bool>,
    pub has_list_elements: Option<bool>,
}

#[napi(object)]
pub struct NativeResourceReaderSpecIn {
    pub index: u32,
    pub scheme: Utf16String,
    pub is_globbable: Option<bool>,
    pub has_hierarchical_uris: Option<bool>,
    pub has_has_element: Option<bool>,
    pub has_list_elements: Option<bool>,
}

#[napi(object)]
#[derive(Default)]
pub struct NativeEvaluatorOptions {
    pub allowed_modules: Option<Vec<Utf16String>>,
    pub allowed_resources: Option<Vec<Utf16String>>,
    pub root_dir: Option<Utf16String>,
    pub working_dir: Option<Utf16String>,
    pub module_paths: Option<Vec<Utf16String>>,
    pub properties: Option<Vec<NativeKeyValue>>,
    pub env: Option<Vec<NativeKeyValue>>,
    pub output_format: Option<Utf16String>,
    pub trace_mode: Option<String>,
    pub power_assertions: Option<bool>,
    pub color: Option<String>,
    pub timeout_ms: Option<BigInt>,
    pub cache_dir: Option<Utf16String>,
    pub no_cache: Option<bool>,
    pub project_dir: Option<Utf16String>,
    pub test_mode: Option<bool>,
    pub http: Option<NativeHttpOptions>,
    pub external_module_readers: Option<Vec<NativeExternalReaderOption>>,
    pub external_resource_readers: Option<Vec<NativeExternalReaderOption>>,
    pub module_readers: Option<Vec<NativeModuleReaderSpecIn>>,
    pub resource_readers: Option<Vec<NativeResourceReaderSpecIn>>,
    pub has_logger_trace: Option<bool>,
    pub has_logger_warn: Option<bool>,
    pub has_custom_logger: Option<bool>,
    pub has_frame_transformer: Option<bool>,
    pub has_security_resolve_module: Option<bool>,
    pub has_security_import_module: Option<bool>,
    pub has_security_read_resource: Option<bool>,
    pub has_security_resolve_resource: Option<bool>,
}

pub(crate) struct BuiltNativeHost {
    pub(crate) host: Arc<NativeHost>,
    pub(crate) bridge: Option<Arc<JsBridge>>,
    pub(crate) logger: Arc<CapturingHostLogger>,
    pub(crate) frame_transformer: Option<Arc<JsFrameTransformerAdapter>>,
    pub(crate) default_timeout_ms: Option<u64>,
    pub(crate) use_color: bool,
    pub(crate) trace_mode: TraceMode,
    pub(crate) power_assertions: bool,
    pub(crate) test_mode: bool,
    pub(crate) editor_url_scheme: String,
    pub(crate) has_js_callbacks: bool,
}

fn load_project_from_dir_or_file(
    dir_or_file: &Path,
    env: Vec<(String, String)>,
    props: Vec<(String, String)>,
) -> std::result::Result<Project, NativeError> {
    let project_file = if dir_or_file.is_dir() {
        dir_or_file.join("PklProject")
    } else {
        dir_or_file.to_path_buf()
    };
    Project::load_from_path_with_env(&project_file, env, props).map_err(NativeError::from_pkl)
}

pub(crate) fn build_native_host(
    opts: NativeEvaluatorOptions,
    project: Option<&NativeProject>,
    bridge_cb: Option<Function<'_, BridgeRequestJs, UnknownReturnValue>>,
) -> std::result::Result<BuiltNativeHost, NativeError> {
    let mut allowed_modules: Option<Vec<String>> = None;
    let mut allowed_resources: Option<Vec<String>> = None;
    let mut root_dir: Option<PathBuf> = None;
    let mut module_paths: Vec<PathBuf> = Vec::new();
    let mut properties: Vec<(String, String)> = Vec::new();
    let mut environment: Vec<(String, String)> = std::env::vars().collect();
    let mut module_cache_dir: Option<PathBuf> = default_module_cache_dir();
    let mut project_dependencies: Option<ProjectDependenciesManager> = None;
    let mut timeout_ms: Option<u64> = None;
    let mut use_color = false;
    let mut trace_mode = TraceMode::Compact;
    let mut http_config = HttpClientConfig::default();
    let mut ca_certs = CaCertSource::bundled_default();
    let mut external_module_readers: BTreeMap<String, ExternalReaderConfig> = BTreeMap::new();
    let mut external_resource_readers: BTreeMap<String, ExternalReaderConfig> = BTreeMap::new();

    // Pre-parse caller security, timeout, env, and properties BEFORE optional `projectDir` loading
    // so `PklProject` evaluation honors the caller's policy and timeout instead of permissive defaults.
    let explicit_env: Option<Vec<(String, String)>> = match opts.env {
        Some(entries) => {
            let mut out = Vec::with_capacity(entries.len());
            for kv in entries {
                out.push((utf16_to_string(&kv.key)?, utf16_to_string(&kv.value)?));
            }
            Some(out)
        }
        None => None,
    };
    let explicit_props: Option<Vec<(String, String)>> = match opts.properties {
        Some(entries) => {
            let mut out = Vec::with_capacity(entries.len());
            for kv in entries {
                out.push((utf16_to_string(&kv.key)?, utf16_to_string(&kv.value)?));
            }
            Some(out)
        }
        None => None,
    };
    let explicit_allowed_modules: Option<Vec<String>> = match opts.allowed_modules {
        Some(mods) => {
            let mut parsed = Vec::with_capacity(mods.len());
            for m in mods {
                parsed.push(utf16_to_string(&m)?);
            }
            Some(parsed)
        }
        None => None,
    };
    let explicit_allowed_resources: Option<Vec<String>> = match opts.allowed_resources {
        Some(res) => {
            let mut parsed = Vec::with_capacity(res.len());
            for r in res {
                parsed.push(utf16_to_string(&r)?);
            }
            Some(parsed)
        }
        None => None,
    };
    let explicit_root_dir: Option<PathBuf> = match opts.root_dir {
        Some(root) => Some(PathBuf::from(utf16_to_string(&root)?)),
        None => None,
    };
    let explicit_timeout_ms: Option<u64> = match opts.timeout_ms {
        Some(ref t) => Some(parse_u64_bigint(t)?),
        None => None,
    };
    let security_hooks = JsSecurityHooks {
        has_resolve_module: opts.has_security_resolve_module.unwrap_or(false),
        has_import_module: opts.has_security_import_module.unwrap_or(false),
        has_read_resource: opts.has_security_read_resource.unwrap_or(false),
        has_resolve_resource: opts.has_security_resolve_resource.unwrap_or(false),
    };

    // 1. Apply project settings first (either explicit `project` handle or `projectDir`).
    let loaded_dir_project: Option<Project> = if project.is_none()
        && let Some(ref pdir) = opts.project_dir
    {
        if security_hooks.any() {
            return Err(NativeError::configuration(
                "`projectDir` cannot be combined with a custom `securityManager` during synchronous evaluator initialization; load the project explicitly with `Project.load(...)` or configure `allowedModules`/`allowedResources`/`rootDir` directly.",
            ));
        }
        let pdir_str = utf16_to_string(pdir)?;
        let dir_or_file = Path::new(&pdir_str);
        let project_file = if dir_or_file.is_dir() {
            dir_or_file.join("PklProject")
        } else {
            dir_or_file.to_path_buf()
        };
        let p_env = explicit_env
            .clone()
            .unwrap_or_else(|| std::env::vars().collect());
        let p_props = explicit_props.clone().unwrap_or_default();
        let proj_config = NativeHostConfig {
            allowed_modules: explicit_allowed_modules.clone().or_else(|| {
                Some(
                    pkl_runtime::host::security::DEFAULT_ALLOWED_MODULES
                        .iter()
                        .map(|s| (*s).to_string())
                        .collect(),
                )
            }),
            allowed_resources: explicit_allowed_resources.clone().or_else(|| {
                Some(
                    pkl_runtime::host::security::DEFAULT_ALLOWED_RESOURCES
                        .iter()
                        .map(|s| (*s).to_string())
                        .collect(),
                )
            }),
            root_dir: explicit_root_dir.clone(),
            timeout_ms: explicit_timeout_ms,
            properties: p_props,
            environment: p_env,
            internal_pkl_binary_eval: true,
            ..Default::default()
        };
        Some(
            Project::load_from_path_with_host_config(&project_file, proj_config, None)
                .map_err(NativeError::from_pkl)?,
        )
    } else {
        None
    };

    let active_project: Option<&Project> = project
        .map(|p| p.inner.as_ref())
        .or(loaded_dir_project.as_ref());

    if let Some(proj) = active_project {
        if let Some(settings) = proj.evaluator_settings() {
            if let Some(ref mods) = settings.allowed_modules {
                allowed_modules = Some(mods.clone());
            }
            if let Some(ref res) = settings.allowed_resources {
                allowed_resources = Some(res.clone());
            }
            if settings.external_properties_is_set {
                properties = settings.external_properties.clone();
            }
            if settings.env_is_set {
                environment = settings.env.clone();
            }
            if let Some(ref paths) = settings.module_path {
                module_paths = paths.clone();
            }
            if let Some(ref root) = settings.root_dir {
                root_dir = Some(root.clone());
            }
            if settings.no_cache == Some(true) {
                module_cache_dir = None;
            } else if let Some(ref cache_dir) = settings.module_cache_dir {
                module_cache_dir = Some(cache_dir.clone());
            }
            if let Some(nanos) = settings.timeout_nanos {
                timeout_ms = Some(
                    Duration::from_nanos(nanos)
                        .as_millis()
                        .min(u64::MAX as u128) as u64,
                );
            }
            if let Some(color) = settings.color {
                use_color = Color::from(color).has_color();
            }
            if let Some(ref mode) = settings.trace_mode {
                if mode == "pretty" {
                    trace_mode = TraceMode::Pretty;
                } else if mode == "compact" {
                    trace_mode = TraceMode::Compact;
                }
            }
            if let Some(ref http) = settings.http {
                if let Some(ref proxy) = http.proxy {
                    if let Some(ref addr) = proxy.address {
                        http_config.proxy.address =
                            Some(ProxyAddress::parse(addr).map_err(NativeError::configuration)?);
                    }
                    if let Some(ref no_proxy) = proxy.no_proxy {
                        let mut rules = Vec::with_capacity(no_proxy.len());
                        for raw in no_proxy {
                            if let Some(rule) =
                                NoProxyRule::parse(raw).map_err(NativeError::configuration)?
                            {
                                rules.push(rule);
                            }
                        }
                        http_config.proxy.no_proxy = rules;
                    }
                }
                if let Some(ref rewrites) = http.rewrites {
                    http_config = http_config
                        .with_rewrites(rewrites.clone())
                        .map_err(NativeError::configuration)?;
                }
            }
            if let Some(ref ext_mods) = settings.external_module_readers {
                for (scheme, spec) in ext_mods {
                    external_module_readers.insert(scheme.clone(), spec.clone());
                }
            }
            if let Some(ref ext_res) = settings.external_resource_readers {
                for (scheme, spec) in ext_res {
                    external_resource_readers.insert(scheme.clone(), spec.clone());
                }
            }
        }
        if let Some(deps) = proj.dependencies_manager() {
            project_dependencies = Some(deps.clone());
        }
    }

    // 2. Apply explicit JS options (overriding project defaults).
    if let Some(mods) = explicit_allowed_modules {
        allowed_modules = Some(mods);
    }
    if let Some(res) = explicit_allowed_resources {
        allowed_resources = Some(res);
    }
    if let Some(root) = explicit_root_dir {
        root_dir = Some(root);
    }
    if let Some(paths) = opts.module_paths {
        let mut parsed = Vec::with_capacity(paths.len());
        for p in paths {
            parsed.push(PathBuf::from(utf16_to_string(&p)?));
        }
        module_paths = parsed;
    }
    if let Some(props) = explicit_props {
        properties = props;
    }
    if let Some(env_vars) = explicit_env {
        environment = env_vars;
    }
    if opts.no_cache == Some(true) {
        module_cache_dir = None;
    } else if let Some(dir) = opts.cache_dir {
        module_cache_dir = Some(PathBuf::from(utf16_to_string(&dir)?));
    }
    if let Some(t) = explicit_timeout_ms {
        timeout_ms = Some(t);
    }
    if let Some(ref mode) = opts.trace_mode {
        trace_mode = match mode.as_str() {
            "compact" => TraceMode::Compact,
            "pretty" => TraceMode::Pretty,
            other => {
                return Err(NativeError::invalid_argument(format!(
                    "Invalid traceMode `{other}`. Expected `compact` or `pretty`."
                )));
            }
        };
    }
    if let Some(ref color_str) = opts.color {
        use_color = Color::parse(color_str)
            .map_err(NativeError::from_pkl)?
            .has_color();
    }
    let power_assertions = opts.power_assertions.unwrap_or(false);
    let test_mode = opts.test_mode.unwrap_or(false);
    let output_format = match opts.output_format {
        Some(ref f) => Some(utf16_to_string(f)?),
        None => None,
    };

    if let Some(http_opts) = opts.http {
        if http_opts.proxy_address.is_some() || http_opts.no_proxy.is_some() {
            let addr = match http_opts.proxy_address {
                Some(ref a) => Some(utf16_to_string(a)?),
                None => None,
            };
            let no_proxy: Vec<String> = match http_opts.no_proxy {
                Some(list) => {
                    let mut out = Vec::with_capacity(list.len());
                    for item in list {
                        out.push(utf16_to_string(&item)?);
                    }
                    out
                }
                None => Vec::new(),
            };
            http_config.proxy =
                ProxyConfig::new(addr.as_deref(), &no_proxy).map_err(NativeError::configuration)?;
        }
        if let Some(rewrites) = http_opts.rewrites {
            let mut map = BTreeMap::new();
            for kv in rewrites {
                map.insert(utf16_to_string(&kv.key)?, utf16_to_string(&kv.value)?);
            }
            http_config = http_config
                .with_rewrites(map)
                .map_err(NativeError::configuration)?;
        }
        if let Some(headers) = http_opts.headers {
            let mut grouped: BTreeMap<String, Vec<(String, String)>> = BTreeMap::new();
            for entry in headers {
                grouped
                    .entry(utf16_to_string(&entry.url_pattern)?)
                    .or_default()
                    .push((
                        utf16_to_string(&entry.name)?,
                        utf16_to_string(&entry.value)?,
                    ));
            }
            for (pattern, pairs) in grouped {
                http_config
                    .add_headers(&pattern, pairs)
                    .map_err(NativeError::configuration)?;
            }
        }
        if let Some(port) = http_opts.test_port {
            if port > u32::from(u16::MAX) {
                return Err(NativeError::invalid_argument(format!(
                    "Invalid testPort `{port}`."
                )));
            }
            http_config.test_port = Some(port as u16);
        }
        let mut cert_files = Vec::new();
        if let Some(files) = http_opts.ca_certificate_files {
            for f in files {
                cert_files.push(PathBuf::from(utf16_to_string(&f)?));
            }
        }
        let mut cert_pems = Vec::new();
        if let Some(pems) = http_opts.ca_certificate_pems {
            for p in pems {
                cert_pems.push(p.to_vec());
            }
        }
        if !cert_files.is_empty() || !cert_pems.is_empty() {
            ca_certs = CaCertSource {
                explicit_files: cert_files,
                cacerts_dir: None,
                extra_pem_bytes: cert_pems,
                disable_home_cacerts: true,
            };
        }
    }

    let working_dir_opt = match &opts.working_dir {
        Some(dir) => Some(utf16_to_string(dir)?),
        None => None,
    };

    if let Some(ext_mods) = opts.external_module_readers {
        for r in ext_mods {
            let scheme = utf16_to_string(&r.scheme)?;
            let executable = utf16_to_string(&r.executable)?;
            let mut arguments = Vec::new();
            if let Some(args) = r.arguments {
                for a in args {
                    arguments.push(utf16_to_string(&a)?);
                }
            }
            external_module_readers.insert(
                scheme,
                ExternalReaderConfig {
                    executable,
                    arguments,
                    working_dir: working_dir_opt.clone(),
                },
            );
        }
    }
    if let Some(ext_res) = opts.external_resource_readers {
        for r in ext_res {
            let scheme = utf16_to_string(&r.scheme)?;
            let executable = utf16_to_string(&r.executable)?;
            let mut arguments = Vec::new();
            if let Some(args) = r.arguments {
                for a in args {
                    arguments.push(utf16_to_string(&a)?);
                }
            }
            external_resource_readers.insert(
                scheme,
                ExternalReaderConfig {
                    executable,
                    arguments,
                    working_dir: working_dir_opt.clone(),
                },
            );
        }
    }

    // 3. Parse custom JS extension descriptors.
    let mut js_module_readers = Vec::new();
    let mut extra_module_schemes = Vec::new();
    if let Some(readers) = opts.module_readers {
        for r in readers {
            let scheme = utf16_to_string(&r.scheme)?;
            extra_module_schemes.push(scheme.clone());
            js_module_readers.push(JsModuleReaderSpec {
                index: r.index,
                scheme,
                is_local: r.is_local.unwrap_or(false),
                is_globbable: r.is_globbable.unwrap_or(false),
                has_hierarchical_uris: r.has_hierarchical_uris.unwrap_or(false),
                has_fragment_paths: r.has_fragment_paths.unwrap_or(false),
                is_cached: r.is_cached.unwrap_or(true),
                has_has_element: r.has_has_element.unwrap_or(false),
                has_list_elements: r.has_list_elements.unwrap_or(false),
            });
        }
    }

    let mut js_resource_readers = Vec::new();
    let mut extra_resource_schemes = Vec::new();
    if let Some(readers) = opts.resource_readers {
        for r in readers {
            let scheme = utf16_to_string(&r.scheme)?;
            extra_resource_schemes.push(scheme.clone());
            js_resource_readers.push(JsResourceReaderSpec {
                index: r.index,
                scheme,
                is_globbable: r.is_globbable.unwrap_or(false),
                has_hierarchical_uris: r.has_hierarchical_uris.unwrap_or(false),
                has_has_element: r.has_has_element.unwrap_or(false),
                has_list_elements: r.has_list_elements.unwrap_or(false),
            });
        }
    }

    let has_custom_logger = opts.has_custom_logger.unwrap_or(false);
    let has_logger_trace = opts.has_logger_trace.unwrap_or(false);
    let has_logger_warn = opts.has_logger_warn.unwrap_or(false);
    let has_frame_transformer = opts.has_frame_transformer.unwrap_or(false);

    let has_js_callbacks = has_custom_logger
        || has_frame_transformer
        || security_hooks.any()
        || !js_module_readers.is_empty()
        || !js_resource_readers.is_empty();

    // Build fallback SecurityPolicy BEFORE handing config to NativeHost so custom securityManager
    // can delegate omitted hooks to standard allowlists + rootDir + custom/external schemes.
    let fallback_policy = if security_hooks.any() {
        let normalized_root = match &root_dir {
            Some(dir) => {
                let real = canonicalize_root_dir(dir).map_err(|err| {
                    NativeError::configuration(format!(
                        "Failed to normalize rootDir `{}`: {err}",
                        dir.display()
                    ))
                })?;
                Some(real.to_string_lossy().into_owned())
            }
            None => None,
        };
        let mut all_mod_schemes: Vec<String> = external_module_readers.keys().cloned().collect();
        all_mod_schemes.extend(extra_module_schemes.iter().cloned());
        let mut all_res_schemes: Vec<String> = external_resource_readers.keys().cloned().collect();
        all_res_schemes.extend(extra_resource_schemes.iter().cloned());
        Some(
            SecurityPolicy::new(
                allowed_modules.as_deref(),
                allowed_resources.as_deref(),
                normalized_root,
            )
            .map_err(NativeError::configuration)?
            .with_external_module_schemes(&all_mod_schemes)
            .map_err(NativeError::configuration)?
            .with_external_resource_schemes(&all_res_schemes)
            .map_err(NativeError::configuration)?,
        )
    } else {
        None
    };

    // We pass `timeout_ms` to NativeHostConfig so HTTP and external reader timeouts clamp cleanly,
    // while our own `SafeTimeoutWatchdog` handles full 64-bit deadlines without overflow panics.
    let config = NativeHostConfig {
        allowed_modules,
        allowed_resources,
        root_dir,
        module_paths,
        http: http_config,
        ca_certs,
        properties,
        environment,
        timeout_ms,
        external_module_schemes: Vec::new(),
        external_resource_schemes: Vec::new(),
        external_module_readers,
        external_resource_readers,
        module_cache_dir,
        project_dependencies,
        test_mode,
        trace_mode,
        power_assertions,
        editor_url_scheme: DEFAULT_EDITOR_URL_SCHEME.to_string(),
        internal_pkl_binary_eval: false,
    };

    // Construct JsBridge if JS callbacks were supplied.
    let shared_bridge_cancel = pkl_host_native::CancelFlag::new();
    let bridge = if has_js_callbacks {
        let cb = bridge_cb.ok_or_else(|| {
            NativeError::invalid_argument(
                "Internal error: missing JS callback bridge function for custom extensions.",
            )
        })?;
        Some(
            JsBridge::new(cb, Arc::clone(&shared_bridge_cancel))
                .map_err(|e| NativeError::configuration(e.to_string()))?,
        )
    } else {
        None
    };

    let logger = CapturingHostLogger::new(
        if has_custom_logger {
            bridge.clone()
        } else {
            None
        },
        has_logger_trace,
        has_logger_warn,
        !has_custom_logger,
    );

    let frame_transformer: Option<Arc<JsFrameTransformerAdapter>> = if has_frame_transformer {
        bridge
            .as_ref()
            .map(|b| JsFrameTransformerAdapter::new(Arc::clone(b)))
    } else {
        None
    };

    let mut ext = NativeHostExtensions::default()
        .with_color(use_color)
        .with_logger(Arc::clone(&logger) as Arc<dyn HostLogger>)
        .with_cancel_flag(Arc::clone(&shared_bridge_cancel));
    ext.extra_allowed_module_schemes = extra_module_schemes;
    ext.extra_allowed_resource_schemes = extra_resource_schemes;

    if let Some(fmt) = output_format {
        ext = ext.with_output_format(fmt);
    }
    if let Some(ref tf) = frame_transformer {
        ext = ext.with_frame_transformer(Arc::clone(tf) as Arc<dyn HostFrameTransformer>, false);
    }
    if let (Some(b), Some(policy)) = (bridge.as_ref(), fallback_policy) {
        ext = ext.with_custom_security(JsSecurityAdapter::new(
            Arc::clone(b),
            security_hooks,
            policy,
        ));
    }
    if let Some(ref b) = bridge {
        for spec in js_module_readers {
            ext = ext.with_custom_module_reader(JsModuleReaderAdapter::new(Arc::clone(b), spec));
        }
        for spec in js_resource_readers {
            ext =
                ext.with_custom_resource_reader(JsResourceReaderAdapter::new(Arc::clone(b), spec));
        }
    }

    let host =
        Arc::new(NativeHost::with_extensions(config, ext).map_err(NativeError::configuration)?);

    Ok(BuiltNativeHost {
        host,
        bridge,
        logger,
        frame_transformer,
        default_timeout_ms: timeout_ms,
        use_color,
        trace_mode,
        power_assertions,
        test_mode,
        editor_url_scheme: DEFAULT_EDITOR_URL_SCHEME.to_string(),
        has_js_callbacks,
    })
}

// =============================================================================
// NativeProject
// =============================================================================

#[napi]
pub struct NativeProject {
    pub(crate) inner: Arc<Project>,
}

#[napi]
impl NativeProject {
    #[napi]
    pub fn load(
        dir_or_file: Utf16String,
        env_vars: Option<Vec<NativeKeyValue>>,
        properties: Option<Vec<NativeKeyValue>>,
    ) -> AsyncTask<LoadProjectTask> {
        let parsed = parse_project_load_args(dir_or_file, env_vars, properties);
        AsyncTask::new(LoadProjectTask { parsed })
    }

    #[napi]
    pub fn load_sync<'env>(
        env: &'env Env,
        dir_or_file: Utf16String,
        env_vars: Option<Vec<NativeKeyValue>>,
        properties: Option<Vec<NativeKeyValue>>,
    ) -> Result<Object<'env>> {
        let res = parse_project_load_args(dir_or_file, env_vars, properties).and_then(
            |(path, env_vec, prop_vec)| {
                load_project_from_dir_or_file(Path::new(&path), env_vec, prop_vec)
                    .map(|p| NativeProject { inner: Arc::new(p) })
            },
        );
        to_js_outcome(env, res)
    }

    #[napi]
    pub fn info<'env>(&self, env: &'env Env) -> Result<Object<'env>> {
        project_info_to_js(env, &self.inner)
    }
}

type ParsedProjectArgs =
    std::result::Result<(String, Vec<(String, String)>, Vec<(String, String)>), NativeError>;

fn parse_project_load_args(
    dir_or_file: Utf16String,
    env_vars: Option<Vec<NativeKeyValue>>,
    properties: Option<Vec<NativeKeyValue>>,
) -> ParsedProjectArgs {
    let path = utf16_to_string(&dir_or_file)?;
    let env_vec = match env_vars {
        Some(entries) => {
            let mut out = Vec::with_capacity(entries.len());
            for kv in entries {
                out.push((utf16_to_string(&kv.key)?, utf16_to_string(&kv.value)?));
            }
            out
        }
        None => std::env::vars().collect(),
    };
    let prop_vec = match properties {
        Some(entries) => {
            let mut out = Vec::with_capacity(entries.len());
            for kv in entries {
                out.push((utf16_to_string(&kv.key)?, utf16_to_string(&kv.value)?));
            }
            out
        }
        None => Vec::new(),
    };
    Ok((path, env_vec, prop_vec))
}

pub struct LoadProjectTask {
    parsed: ParsedProjectArgs,
}

impl<'task> ScopedTask<'task> for LoadProjectTask {
    type Output = std::result::Result<NativeProject, NativeError>;
    type JsValue = Object<'task>;

    fn compute(&mut self) -> Result<Self::Output> {
        let (path, env_vec, prop_vec) = match &self.parsed {
            Ok(v) => v.clone(),
            Err(e) => return Ok(Err(e.clone())),
        };
        Ok(
            load_project_from_dir_or_file(Path::new(&path), env_vec, prop_vec)
                .map(|p| NativeProject { inner: Arc::new(p) }),
        )
    }

    fn resolve(&mut self, env: &'task Env, output: Self::Output) -> Result<Self::JsValue> {
        to_js_outcome(env, output)
    }
}

fn project_info_to_js<'env>(env: &'env Env, proj: &Project) -> Result<Object<'env>> {
    let mut out = Object::new(env)?;
    out.set(
        "projectDir",
        proj.project_dir().to_string_lossy().into_owned(),
    )?;
    out.set("projectFileUri", proj.project_file_uri())?;
    out.set("projectBaseUri", proj.project_base_uri())?;
    out.set("tests", proj.tests().to_vec())?;

    match proj.package() {
        Some(pkg) => {
            let mut po = Object::new(env)?;
            po.set("name", pkg.name.as_str())?;
            po.set("baseUri", pkg.base_uri.as_str())?;
            po.set("version", pkg.version.to_string())?;
            po.set("uri", pkg.uri.uri.to_java_utf8())?;
            po.set("packageZipUrl", pkg.package_zip_url.as_str())?;
            match &pkg.description {
                Some(d) => po.set("description", d.as_str())?,
                None => po.set("description", Null)?,
            }
            po.set("authors", pkg.authors.clone())?;
            match &pkg.website {
                Some(w) => po.set("website", w.as_str())?,
                None => po.set("website", Null)?,
            }
            match &pkg.documentation {
                Some(d) => po.set("documentation", d.as_str())?,
                None => po.set("documentation", Null)?,
            }
            match &pkg.source_code {
                Some(s) => po.set("sourceCode", s.as_str())?,
                None => po.set("sourceCode", Null)?,
            }
            match &pkg.source_code_url_scheme {
                Some(s) => po.set("sourceCodeUrlScheme", s.as_str())?,
                None => po.set("sourceCodeUrlScheme", Null)?,
            }
            match &pkg.license {
                Some(l) => po.set("license", l.as_str())?,
                None => po.set("license", Null)?,
            }
            match &pkg.issue_tracker {
                Some(i) => po.set("issueTracker", i.as_str())?,
                None => po.set("issueTracker", Null)?,
            }
            po.set("apiTests", pkg.api_tests.clone())?;
            po.set("exclude", pkg.exclude.clone())?;
            out.set("package", po)?;
        }
        None => out.set("package", Null)?,
    }

    match proj.evaluator_settings() {
        Some(settings) => {
            let mut so = Object::new(env)?;
            match &settings.allowed_modules {
                Some(v) => so.set("allowedModules", v.clone())?,
                None => so.set("allowedModules", Null)?,
            }
            match &settings.allowed_resources {
                Some(v) => so.set("allowedResources", v.clone())?,
                None => so.set("allowedResources", Null)?,
            }
            match &settings.root_dir {
                Some(r) => so.set("rootDir", r.to_string_lossy().into_owned())?,
                None => so.set("rootDir", Null)?,
            }
            match &settings.module_cache_dir {
                Some(c) => so.set("moduleCacheDir", c.to_string_lossy().into_owned())?,
                None => so.set("moduleCacheDir", Null)?,
            }
            match settings.no_cache {
                Some(b) => so.set("noCache", b)?,
                None => so.set("noCache", Null)?,
            }
            match &settings.trace_mode {
                Some(t) => so.set("traceMode", t.as_str())?,
                None => so.set("traceMode", Null)?,
            }
            match settings.color {
                Some(c) => so.set("color", c.as_str())?,
                None => so.set("color", Null)?,
            }
            match settings.timeout_nanos {
                Some(ns) => so.set("timeoutMs", BigInt::from(ns / 1_000_000))?,
                None => so.set("timeoutMs", Null)?,
            }
            if settings.env_is_set {
                let mut eo = Object::new(env)?;
                for (k, v) in &settings.env {
                    let js_v = env.create_string(v.as_str())?;
                    define_own_prop(env, &mut eo, k.as_str(), &js_v)?;
                }
                so.set("env", eo)?;
            } else {
                so.set("env", Null)?;
            }
            if settings.external_properties_is_set {
                let mut po = Object::new(env)?;
                for (k, v) in &settings.external_properties {
                    let js_v = env.create_string(v.as_str())?;
                    define_own_prop(env, &mut po, k.as_str(), &js_v)?;
                }
                so.set("externalProperties", po)?;
            } else {
                so.set("externalProperties", Null)?;
            }
            match &settings.module_path {
                Some(mp) => so.set(
                    "modulePath",
                    mp.iter()
                        .map(|p| p.to_string_lossy().into_owned())
                        .collect::<Vec<_>>(),
                )?,
                None => so.set("modulePath", Null)?,
            }
            out.set("evaluatorSettings", so)?;
        }
        None => out.set("evaluatorSettings", Null)?,
    }

    Ok(out)
}

// =============================================================================
// NativeEvaluator
// =============================================================================

struct SessionState {
    session: SourceSession,
    columnar_buf: pkl_runtime::vm::columnar_value::ColumnarValueBuffer,
    closed: bool,
    pending_continue: bool,
}

pub(crate) type CallCancelPair = (Arc<AtomicBool>, Arc<CancelFlag>);
type InFlightCancelMap = Arc<Mutex<HashMap<u64, CallCancelPair>>>;
pub(crate) type ActiveCallCancelSlot = Arc<Mutex<Option<CallCancelPair>>>;

#[derive(Clone)]
struct EvaluatorShared {
    host: Arc<NativeHost>,
    state: Arc<Mutex<SessionState>>,
    pending_continue_cvar: Arc<Condvar>,
    abort_pending_continue: Arc<AtomicBool>,
    bridge: Option<Arc<JsBridge>>,
    logger: Arc<CapturingHostLogger>,
    frame_transformer: Option<Arc<JsFrameTransformerAdapter>>,
    closed_flag: Arc<AtomicBool>,
    cancel_generation: Arc<AtomicU64>,
    active_call_id: Arc<AtomicU64>,
    in_flight_cancels: InFlightCancelMap,
    default_timeout_ms: Option<u64>,
    use_color: bool,
    power_assertions: bool,
    test_mode: bool,
    editor_url_scheme: String,
    diagnostic_profile: DiagnosticProfile,
    has_js_callbacks: bool,
}

fn clamp_session_timeout(now_ms: u64, timeout_ms: Option<u64>) -> Option<u64> {
    timeout_ms.map(|ms| ms.min(u64::MAX.saturating_sub(now_ms)))
}

impl EvaluatorShared {
    fn map_runtime_err(
        &self,
        session: &SourceSession,
        err: RuntimeError,
        timeout_ms: Option<u64>,
    ) -> NativeError {
        match err.kind {
            ErrorKind::Timeout => {
                NativeError::timeout(format_timeout_message(timeout_ms.unwrap_or(0)))
            }
            ErrorKind::Closed => NativeError::closed("Evaluator has been closed."),
            ErrorKind::Cancelled => NativeError::cancelled("Evaluation was cancelled."),
            _ => {
                let is_security = err.is_security_error();
                let text = session.format_error_text(err);
                if is_security {
                    NativeError::security(text)
                } else {
                    NativeError::evaluation(text)
                }
            }
        }
    }

    fn check_drive_abort(
        &self,
        session: &mut SourceSession,
        call_cancel: &Arc<AtomicBool>,
        watchdog: &SafeTimeoutWatchdog,
        timeout_ms: Option<u64>,
    ) -> Option<RuntimeError> {
        if self.closed_flag.load(Ordering::SeqCst) {
            session.close();
            let _ = session.drain_cancelled_reads();
            let _ = session.drain_cancelled_modules();
            let _ = session.drain_cancelled_globs();
            return Some(RuntimeError::new(
                ErrorKind::Closed,
                "Evaluator has been closed.",
            ));
        }
        if watchdog.triggered() {
            session.abort_active_operation();
            let _ = session.drain_cancelled_reads();
            let _ = session.drain_cancelled_modules();
            let _ = session.drain_cancelled_globs();
            return Some(RuntimeError::new(
                ErrorKind::Timeout,
                format_timeout_message(timeout_ms.unwrap_or(0)),
            ));
        }
        if call_cancel.load(Ordering::SeqCst) {
            session.abort_active_operation();
            let _ = session.drain_cancelled_reads();
            let _ = session.drain_cancelled_modules();
            let _ = session.drain_cancelled_globs();
            return Some(RuntimeError::new(
                ErrorKind::Cancelled,
                "Evaluation was cancelled.",
            ));
        }
        None
    }

    fn drive_session(
        &self,
        session: &mut SourceSession,
        call_cancel: &Arc<AtomicBool>,
        watchdog: &SafeTimeoutWatchdog,
        timeout_ms: Option<u64>,
    ) -> std::result::Result<Value, RuntimeError> {
        let has_timeout = timeout_ms.is_some();
        let step_budget = if has_timeout { 4096 } else { 32_768 };
        let res = loop {
            if let Some(err) = self.check_drive_abort(session, call_cancel, watchdog, timeout_ms) {
                break Err(err);
            }
            match session.poll(self.host.clock_ms(), step_budget) {
                HostPoll::Ready(result) => break result,
                HostPoll::Yielded => {}
                HostPoll::Read { token, .. } => {
                    let req = session.read_request(&token)?;
                    let result = self.host.handle_read_request(&req);
                    if let Some(err) =
                        self.check_drive_abort(session, call_cancel, watchdog, timeout_ms)
                    {
                        break Err(err);
                    }
                    session.complete_read(&token, result, self.host.clock_ms())?;
                }
                HostPoll::Module { token, request } => {
                    let completion = self.host.handle_module_request(&request);
                    if let Some(err) =
                        self.check_drive_abort(session, call_cancel, watchdog, timeout_ms)
                    {
                        break Err(err);
                    }
                    session.complete_module(&token, completion, self.host.clock_ms())?;
                }
                HostPoll::Glob { token, request } => {
                    let completion = self.host.handle_glob_request(&request);
                    if let Some(err) =
                        self.check_drive_abort(session, call_cancel, watchdog, timeout_ms)
                    {
                        break Err(err);
                    }
                    session.complete_glob(&token, completion, self.host.clock_ms())?;
                }
                HostPoll::Log { token, event } => {
                    let log_res =
                        self.logger
                            .log_checked(event.level, &event.message, &event.frame);
                    if let Some(err) =
                        self.check_drive_abort(session, call_cancel, watchdog, timeout_ms)
                    {
                        break Err(err);
                    }
                    session.acknowledge_log(&token, log_res, self.host.clock_ms())?;
                }
                HostPoll::TransformFrame { token, frame } => {
                    let base = if self.test_mode {
                        (*frame).clone()
                    } else {
                        FrameTransformer::Default.transform((*frame).clone())
                    };
                    let transformed = match &self.frame_transformer {
                        Some(tf) => tf.transform_checked(base),
                        None => Ok(base),
                    };
                    if let Some(err) =
                        self.check_drive_abort(session, call_cancel, watchdog, timeout_ms)
                    {
                        break Err(err);
                    }
                    session.complete_frame_transform(&token, transformed, self.host.clock_ms())?;
                }
            }
        };
        match res {
            Ok(v) => Ok(v),
            Err(err) if watchdog.triggered() && err.kind != ErrorKind::Timeout => {
                session.abort_active_operation();
                let _ = session.drain_cancelled_reads();
                let _ = session.drain_cancelled_modules();
                let _ = session.drain_cancelled_globs();
                Err(RuntimeError::new(
                    ErrorKind::Timeout,
                    format_timeout_message(timeout_ms.unwrap_or(0)),
                ))
            }
            Err(err) => Err(err),
        }
    }

    fn execute_op(
        &self,
        op: EvalOp,
        call_timeout_override: Option<u64>,
        call_cancel: Arc<AtomicBool>,
        http_cancel: Arc<CancelFlag>,
        start_gen: u64,
        call_id: Option<u64>,
    ) -> std::result::Result<EvalOutput, NativeError> {
        struct InFlightGuard {
            map: InFlightCancelMap,
            id: Option<u64>,
        }
        impl Drop for InFlightGuard {
            fn drop(&mut self) {
                if let Some(id) = self.id {
                    let mut m = self.map.lock().unwrap_or_else(|e| e.into_inner());
                    m.remove(&id);
                }
            }
        }
        let _in_flight_guard = InFlightGuard {
            map: Arc::clone(&self.in_flight_cancels),
            id: call_id,
        };

        let is_continue = matches!(op, EvalOp::ContinueRunning { .. });

        if !is_continue {
            if self.closed_flag.load(Ordering::SeqCst) {
                return Err(NativeError::closed("Evaluator has been closed."));
            }
            if call_cancel.load(Ordering::SeqCst)
                || self.cancel_generation.load(Ordering::SeqCst) != start_gen
            {
                return Err(NativeError::cancelled("Evaluation was cancelled."));
            }
        }

        let mut guard = self.state.lock().unwrap_or_else(|e| e.into_inner());

        // If a fast try-poll left `pending_continue = true`, wait for its paired
        // `ContinueRunning` task to finish before starting an unrelated operation on
        // the shared `SourceSession`.
        if !is_continue {
            while guard.pending_continue
                && !guard.closed
                && !self.closed_flag.load(Ordering::SeqCst)
                && !call_cancel.load(Ordering::SeqCst)
                && self.cancel_generation.load(Ordering::SeqCst) == start_gen
            {
                if self.abort_pending_continue.swap(false, Ordering::SeqCst) {
                    guard.session.abort_active_operation();
                    let _ = guard.session.drain_cancelled_reads();
                    let _ = guard.session.drain_cancelled_modules();
                    let _ = guard.session.drain_cancelled_globs();
                    guard.pending_continue = false;
                    self.pending_continue_cvar.notify_all();
                    break;
                }
                let (next, _) = self
                    .pending_continue_cvar
                    .wait_timeout(guard, Duration::from_millis(20))
                    .unwrap_or_else(|e| e.into_inner());
                guard = next;
            }
        }

        if guard.closed || self.closed_flag.load(Ordering::SeqCst) || guard.session.is_closed() {
            if is_continue && guard.pending_continue {
                guard.pending_continue = false;
                self.abort_pending_continue.store(false, Ordering::SeqCst);
                self.pending_continue_cvar.notify_all();
            }
            if !guard.closed {
                guard.closed = true;
                guard.columnar_buf.clear_and_release();
                guard.session.close();
                self.host.close();
                self.pending_continue_cvar.notify_all();
            }
            return Err(NativeError::closed("Evaluator has been closed."));
        }
        if call_cancel.load(Ordering::SeqCst)
            || self.cancel_generation.load(Ordering::SeqCst) != start_gen
            || (is_continue && self.abort_pending_continue.swap(false, Ordering::SeqCst))
        {
            if is_continue && guard.pending_continue {
                guard.pending_continue = false;
                self.abort_pending_continue.store(false, Ordering::SeqCst);
                guard.session.abort_active_operation();
                let _ = guard.session.drain_cancelled_reads();
                let _ = guard.session.drain_cancelled_modules();
                let _ = guard.session.drain_cancelled_globs();
                self.pending_continue_cvar.notify_all();
            }
            return Err(NativeError::cancelled("Evaluation was cancelled."));
        }

        struct ActiveCallIdGuard {
            slot: Arc<AtomicU64>,
        }
        impl Drop for ActiveCallIdGuard {
            fn drop(&mut self) {
                self.slot.store(0, Ordering::SeqCst);
            }
        }
        self.active_call_id
            .store(call_id.unwrap_or(0), Ordering::SeqCst);
        let _active_call_id_guard = ActiveCallIdGuard {
            slot: Arc::clone(&self.active_call_id),
        };

        let effective_timeout = call_timeout_override.or(self.default_timeout_ms);
        let watchdog = SafeTimeoutWatchdog::start(
            effective_timeout,
            Arc::clone(&call_cancel),
            Arc::clone(&http_cancel),
            Arc::clone(&self.host),
            self.bridge.clone(),
        );
        let _scoped_http_cancel =
            ScopedEvalCancel::push_with_deadline(Arc::clone(&http_cancel), watchdog.deadline());
        let _bridge_guard = self
            .bridge
            .as_ref()
            .map(|b| b.enter_call(Arc::clone(&call_cancel), watchdog.deadline()));

        if is_continue {
            guard.pending_continue = false;
            self.abort_pending_continue.store(false, Ordering::SeqCst);
            self.pending_continue_cvar.notify_all();
        }
        let outcome = self.run_op_locked(
            &mut guard.session,
            op,
            &call_cancel,
            &watchdog,
            effective_timeout,
        );

        let timed_out = watchdog.stop();

        if self.closed_flag.load(Ordering::SeqCst) {
            if !guard.closed {
                guard.closed = true;
                guard.columnar_buf.clear_and_release();
                guard.session.close();
                self.host.close();
                self.pending_continue_cvar.notify_all();
            }
            return Err(NativeError::closed("Evaluator has been closed."));
        }
        if timed_out && outcome.is_err() {
            return Err(NativeError::timeout(format_timeout_message(
                effective_timeout.unwrap_or(0),
            )));
        }
        if call_cancel.load(Ordering::SeqCst) && outcome.is_err() {
            return Err(NativeError::cancelled("Evaluation was cancelled."));
        }
        outcome
    }

    fn prepare_spec(
        &self,
        uri: &PklString,
        contents: Option<&PklString>,
    ) -> std::result::Result<ModuleSourceSpec, NativeError> {
        let raw = ModuleSourceSpec {
            uri: uri.clone(),
            contents: contents.cloned().map(SourceText::from),
        };
        self.host.resolve_module_source_spec(raw).map_err(|err| {
            let msg = format_module_host_error_for_napi(&err);
            if matches!(err, ModuleHostError::SecurityManagerException { .. }) {
                NativeError::security(msg)
            } else {
                NativeError::evaluation(msg)
            }
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn eval_pkl_binary_locked(
        &self,
        session: &mut SourceSession,
        uri: &PklString,
        contents: Option<&PklString>,
        expression: &str,
        call_cancel: &Arc<AtomicBool>,
        watchdog: &SafeTimeoutWatchdog,
        timeout_ms: Option<u64>,
    ) -> std::result::Result<Vec<u8>, NativeError> {
        let spec = self.prepare_spec(uri, contents)?;
        let now = self.host.clock_ms();
        session
            .start_evaluation_pkl_binary(
                spec,
                expression,
                now,
                clamp_session_timeout(now, timeout_ms),
            )
            .map_err(|e| self.map_runtime_err(session, e, timeout_ms))?;
        let val = self
            .drive_session(session, call_cancel, watchdog, timeout_ms)
            .map_err(|e| self.map_runtime_err(session, e, timeout_ms))?;
        SourceSession::result_bytes(val)
            .map(|b| b.to_vec())
            .map_err(|e| self.map_runtime_err(session, e, timeout_ms))
    }

    #[allow(clippy::too_many_arguments)]
    fn eval_string_locked(
        &self,
        session: &mut SourceSession,
        uri: &PklString,
        contents: Option<&PklString>,
        expression: &str,
        call_cancel: &Arc<AtomicBool>,
        watchdog: &SafeTimeoutWatchdog,
        timeout_ms: Option<u64>,
    ) -> std::result::Result<PklString, NativeError> {
        let spec = self.prepare_spec(uri, contents)?;
        let now = self.host.clock_ms();
        session
            .start_evaluation(
                spec,
                expression,
                now,
                clamp_session_timeout(now, timeout_ms),
            )
            .map_err(|e| self.map_runtime_err(session, e, timeout_ms))?;
        let val = self
            .drive_session(session, call_cancel, watchdog, timeout_ms)
            .map_err(|e| self.map_runtime_err(session, e, timeout_ms))?;
        SourceSession::result_string(val).map_err(|e| self.map_runtime_err(session, e, timeout_ms))
    }

    fn run_op_locked(
        &self,
        session: &mut SourceSession,
        op: EvalOp,
        call_cancel: &Arc<AtomicBool>,
        watchdog: &SafeTimeoutWatchdog,
        timeout_ms: Option<u64>,
    ) -> std::result::Result<EvalOutput, NativeError> {
        match op {
            EvalOp::ContinueRunning { binary } => {
                let val = self
                    .drive_session(session, call_cancel, watchdog, timeout_ms)
                    .map_err(|e| self.map_runtime_err(session, e, timeout_ms))?;
                if binary {
                    let bytes = SourceSession::result_bytes(val)
                        .map(|b| b.to_vec())
                        .map_err(|e| self.map_runtime_err(session, e, timeout_ms))?;
                    Ok(EvalOutput::Bytes(bytes))
                } else {
                    let text = SourceSession::result_string(val)
                        .map_err(|e| self.map_runtime_err(session, e, timeout_ms))?;
                    Ok(EvalOutput::Text(text))
                }
            }
            EvalOp::OutputText { uri, contents } => {
                let text = self.eval_string_locked(
                    session,
                    &uri,
                    contents.as_ref(),
                    "output.text",
                    call_cancel,
                    watchdog,
                    timeout_ms,
                )?;
                Ok(EvalOutput::Text(text))
            }
            EvalOp::OutputBytes { uri, contents } => {
                let spec = self.prepare_spec(&uri, contents.as_ref())?;
                let now = self.host.clock_ms();
                session
                    .start_evaluation_output_bytes(
                        spec,
                        now,
                        clamp_session_timeout(now, timeout_ms),
                    )
                    .map_err(|e| self.map_runtime_err(session, e, timeout_ms))?;
                let val = self
                    .drive_session(session, call_cancel, watchdog, timeout_ms)
                    .map_err(|e| self.map_runtime_err(session, e, timeout_ms))?;
                let bytes = SourceSession::result_bytes(val)
                    .map(|b| b.to_vec())
                    .map_err(|e| self.map_runtime_err(session, e, timeout_ms))?;
                Ok(EvalOutput::Bytes(bytes))
            }
            EvalOp::OutputFiles { uri, contents } => {
                const EXPR: &str = "let (f = output.files) if (f == null) Map() else f.toMap().mapValues((_, v) -> v.bytes)";
                let bytes = self.eval_pkl_binary_locked(
                    session,
                    &uri,
                    contents.as_ref(),
                    EXPR,
                    call_cancel,
                    watchdog,
                    timeout_ms,
                )?;
                let mut decoded = decode_lossless(&bytes).map_err(|err| {
                    NativeError::decode(format!("Failed to decode output.files: {err}"))
                })?;
                let mut files = Vec::new();
                if let DecodedValue::Map(entries) | DecodedValue::Mapping(entries) = &mut decoded {
                    for (k, mut v) in entries.drain(..) {
                        let key = match &k {
                            DecodedValue::String(s) => s.clone(),
                            other => other.to_java_key_string(),
                        };
                        let raw_bytes = match &mut v {
                            DecodedValue::Bytes(b) => std::mem::take(b),
                            DecodedValue::String(s) => s.to_java_utf8().into_bytes(),
                            other => {
                                return Err(NativeError::decode(format!(
                                    "Unexpected FileOutput.bytes value for `{}`: {other:?}",
                                    key.to_java_utf8()
                                )));
                            }
                        };
                        files.push((key, raw_bytes));
                    }
                }
                Ok(EvalOutput::Files(files))
            }
            EvalOp::OutputValue { uri, contents } => {
                let bytes = self.eval_pkl_binary_locked(
                    session,
                    &uri,
                    contents.as_ref(),
                    "output.value",
                    call_cancel,
                    watchdog,
                    timeout_ms,
                )?;
                let decoded = decode_lossless(&bytes).map_err(|err| {
                    NativeError::decode(format!("Failed to decode output.value: {err}"))
                })?;
                Ok(EvalOutput::Value(decoded))
            }
            EvalOp::Expression {
                uri,
                contents,
                expression,
            } => {
                let expr_str = expression.to_java_utf8();
                let text = self.eval_string_locked(
                    session,
                    &uri,
                    contents.as_ref(),
                    &expr_str,
                    call_cancel,
                    watchdog,
                    timeout_ms,
                )?;
                Ok(EvalOutput::Text(text))
            }
            EvalOp::ExpressionValue {
                uri,
                contents,
                expression,
            } => {
                let expr_str = expression.to_java_utf8();
                let bytes = self.eval_pkl_binary_locked(
                    session,
                    &uri,
                    contents.as_ref(),
                    &expr_str,
                    call_cancel,
                    watchdog,
                    timeout_ms,
                )?;
                let decoded = decode_lossless(&bytes).map_err(|err| {
                    NativeError::decode(format!("Failed to decode expression value: {err}"))
                })?;
                Ok(EvalOutput::Value(decoded))
            }
            EvalOp::ExpressionPklBinary {
                uri,
                contents,
                expression,
            } => {
                let expr_str = expression.to_java_utf8();
                let bytes = self.eval_pkl_binary_locked(
                    session,
                    &uri,
                    contents.as_ref(),
                    &expr_str,
                    call_cancel,
                    watchdog,
                    timeout_ms,
                )?;
                Ok(EvalOutput::Bytes(bytes))
            }
            EvalOp::Schema { uri, contents } => {
                let spec = self.prepare_spec(&uri, contents.as_ref())?;
                let now = self.host.clock_ms();
                session
                    .start_load_module(spec, now, clamp_session_timeout(now, timeout_ms))
                    .map_err(|e| self.map_runtime_err(session, e, timeout_ms))?;
                let module_val = self
                    .drive_session(session, call_cancel, watchdog, timeout_ms)
                    .map_err(|e| self.map_runtime_err(session, e, timeout_ms))?;
                let mut extractor = SchemaExtractor::new(module_val)
                    .map_err(|e| self.map_runtime_err(session, e, timeout_ms))?;
                let mut last_result = None;
                loop {
                    let t = self.host.clock_ms();
                    match extractor
                        .step(
                            session,
                            last_result,
                            t,
                            clamp_session_timeout(t, timeout_ms),
                        )
                        .map_err(|e| self.map_runtime_err(session, e, timeout_ms))?
                    {
                        SchemaStep::RunPending => {
                            last_result = Some(
                                self.drive_session(session, call_cancel, watchdog, timeout_ms)
                                    .map_err(|e| self.map_runtime_err(session, e, timeout_ms))?,
                            );
                        }
                        SchemaStep::Done(schema) => {
                            return Ok(EvalOutput::Schema(Box::new(schema)));
                        }
                    }
                }
            }
            EvalOp::Test {
                uri,
                contents,
                overwrite,
                reporter,
            } => {
                self.logger.begin_capture();
                let res = self.run_test_locked(
                    session,
                    &uri,
                    contents.as_ref(),
                    overwrite,
                    call_cancel,
                    watchdog,
                    timeout_ms,
                );
                let logs = self.logger.end_capture();
                let mut results = res?;
                results.logs = logs;
                let rendered = match reporter {
                    TestReporterMode::Spec => {
                        let mut out = render_spec_report_colored(&results, self.use_color);
                        out.push('\n');
                        out.push_str(&render_summary_colored(
                            std::slice::from_ref(&results),
                            self.use_color,
                        ));
                        out
                    }
                    TestReporterMode::Minimal => {
                        let mut out = render_minimal_report(&results, self.use_color);
                        out.push('\n');
                        out.push_str(&render_summary_colored(
                            std::slice::from_ref(&results),
                            self.use_color,
                        ));
                        out
                    }
                    TestReporterMode::JUnit => render_junit_report(&results),
                };
                Ok(EvalOutput::Test { results, rendered })
            }
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

    #[allow(clippy::too_many_arguments)]
    fn run_test_locked(
        &self,
        session: &mut SourceSession,
        uri: &PklString,
        contents: Option<&PklString>,
        overwrite: bool,
        call_cancel: &Arc<AtomicBool>,
        watchdog: &SafeTimeoutWatchdog,
        timeout_ms: Option<u64>,
    ) -> std::result::Result<TestResults, NativeError> {
        let uri_str = uri.to_java_utf8();
        let check_expr = r#"
let (reflect = import("pkl:reflect"))
let (m = reflect.Module(module))
let (isTest = reflect.Class(module.getClass()).isSubclassOf(reflect.Class(import("pkl:test").getClass())))
new Mapping {
  ["moduleName"] = m.name
  ["moduleUri"] = m.uri
  ["isTest"] = isTest
}
"#;
        let check_bytes = self.eval_pkl_binary_locked(
            session,
            uri,
            contents,
            check_expr,
            call_cancel,
            watchdog,
            timeout_ms,
        )?;
        let check_decoded = decode_lossless(&check_bytes)
            .map_err(|err| NativeError::decode(format!("Failed to decode test check: {err}")))?;
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

        let module_source_text: Option<String> = if let Some(inline) = contents {
            Some(inline.to_java_utf8())
        } else if module_uri.starts_with("file:") {
            let mod_uri_pkl = PklString::from(module_uri.as_str());
            self.host
                .fs()
                .resolve_file_module(&mod_uri_pkl)
                .ok()
                .and_then(|(resolved, real_path)| {
                    if resolved.resolved_uri != mod_uri_pkl
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

        let facts = self.run_test_facts(
            session,
            uri,
            contents,
            &module_uri,
            module_source_text.as_deref(),
            call_cancel,
            watchdog,
            timeout_ms,
        )?;
        let (examples, gen_exp, gen_act) = self.run_test_examples(
            session,
            uri,
            contents,
            &module_uri,
            module_source_text.as_deref(),
            overwrite,
            call_cancel,
            watchdog,
            timeout_ms,
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

    #[allow(clippy::too_many_arguments)]
    fn run_test_facts(
        &self,
        session: &mut SourceSession,
        uri: &PklString,
        contents: Option<&PklString>,
        module_uri: &str,
        module_source_text: Option<&str>,
        call_cancel: &Arc<AtomicBool>,
        watchdog: &SafeTimeoutWatchdog,
        timeout_ms: Option<u64>,
    ) -> std::result::Result<TestSectionResults, NativeError> {
        let shape_expr = "if (facts == null) null else facts.toMap().mapValues((_, g) -> g.length)";
        let shape_bytes = self.eval_pkl_binary_locked(
            session,
            uri,
            contents,
            shape_expr,
            call_cancel,
            watchdog,
            timeout_ms,
        )?;
        let shape_decoded = decode_lossless(&shape_bytes)
            .map_err(|err| NativeError::decode(format!("Failed to decode facts shape: {err}")))?;
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
                match self.eval_pkl_binary_locked(
                    session,
                    uri,
                    contents,
                    &expr,
                    call_cancel,
                    watchdog,
                    timeout_ms,
                ) {
                    Ok(bytes) => {
                        let decoded = decode_lossless(&bytes).map_err(|err| {
                            NativeError::decode(format!("Failed to decode fact value: {err}"))
                        })?;
                        if matches!(decoded, DecodedValue::Bool(false)) {
                            let failure = self.build_fact_failure(
                                session,
                                uri,
                                contents,
                                module_uri,
                                group_locs.and_then(|v| v.get(idx)),
                                idx,
                                call_cancel,
                                watchdog,
                                timeout_ms,
                            )?;
                            failures.push(failure);
                        }
                    }
                    Err(err) => {
                        if matches!(
                            err.kind,
                            NativeErrorKind::Cancelled
                                | NativeErrorKind::Timeout
                                | NativeErrorKind::Closed
                        ) {
                            return Err(err);
                        }
                        let rendered = err.message.to_java_utf8();
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

    #[allow(clippy::too_many_arguments)]
    fn build_fact_failure(
        &self,
        session: &mut SourceSession,
        uri: &PklString,
        contents: Option<&PklString>,
        module_uri: &str,
        loc_info: Option<&(String, usize, pkl_runtime::source::Span)>,
        idx: usize,
        call_cancel: &Arc<AtomicBool>,
        watchdog: &SafeTimeoutWatchdog,
        timeout_ms: Option<u64>,
    ) -> std::result::Result<TestFailure, NativeError> {
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
            let probe_expr = format!("(null as Any({expr_text}))");
            if let Ok(spec) = self.prepare_spec(uri, contents) {
                let now = self.host.clock_ms();
                if session
                    .start_evaluation_pkl_binary(
                        spec,
                        &probe_expr,
                        now,
                        clamp_session_timeout(now, timeout_ms),
                    )
                    .is_ok()
                {
                    let res = self.drive_session(session, call_cancel, watchdog, timeout_ms);
                    if let Err(err) = res {
                        if matches!(
                            err.kind,
                            ErrorKind::Timeout | ErrorKind::Cancelled | ErrorKind::Closed
                        ) {
                            return Err(self.map_runtime_err(session, err, timeout_ms));
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

    #[allow(clippy::too_many_arguments)]
    fn run_test_examples(
        &self,
        session: &mut SourceSession,
        uri: &PklString,
        contents: Option<&PklString>,
        module_uri: &str,
        module_source_text: Option<&str>,
        overwrite: bool,
        call_cancel: &Arc<AtomicBool>,
        watchdog: &SafeTimeoutWatchdog,
        timeout_ms: Option<u64>,
    ) -> std::result::Result<(TestSectionResults, Option<String>, Option<String>), NativeError>
    {
        let is_null_bytes = self.eval_pkl_binary_locked(
            session,
            uri,
            contents,
            "examples == null",
            call_cancel,
            watchdog,
            timeout_ms,
        )?;
        if matches!(
            decode_lossless(&is_null_bytes),
            Ok(DecodedValue::Bool(true))
        ) {
            return Ok((
                TestSectionResults::empty(TestSectionName::Examples),
                None,
                None,
            ));
        }

        if !module_uri.starts_with("file:") {
            return Err(NativeError::evaluation(format!(
                "–– Pkl Error ––\nCannot evaluate examples for non-file-based test module `{module_uri}`.\n"
            )));
        }

        let module_uri_pkl = PklString::from(module_uri);
        self.host
            .check_resolve_module(&module_uri_pkl)
            .map_err(|err| NativeError::security(format_module_host_error_for_napi(&err)))?;

        let module_path = file_uri_to_path(&module_uri_pkl).map_err(|err| {
            NativeError::evaluation(format!("Invalid file URI `{module_uri}`: {err}"))
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
            .map_err(|err| NativeError::security(format_module_host_error_for_napi(&err)))?;
        self.host
            .check_resolve_module(&actual_uri_pkl)
            .map_err(|err| NativeError::security(format_module_host_error_for_napi(&err)))?;
        let canon_expected = pkl_host_native::try_resolve_canonical_or_ancestor(&expected_path)
            .map_err(|()| {
                NativeError::security(
                    pkl_runtime::host::security::root_dir_denied_message(&expected_uri_pkl, false)
                        .to_java_utf8(),
                )
            })?;
        let canon_expected_uri = path_to_file_uri(&canon_expected);
        if canon_expected_uri != expected_uri_pkl {
            self.host
                .check_resolve_module(&canon_expected_uri)
                .map_err(|err| NativeError::security(format_module_host_error_for_napi(&err)))?;
        }
        let canon_actual = pkl_host_native::try_resolve_canonical_or_ancestor(&actual_path)
            .map_err(|()| {
                NativeError::security(
                    pkl_runtime::host::security::root_dir_denied_message(&actual_uri_pkl, false)
                        .to_java_utf8(),
                )
            })?;
        let canon_actual_uri = path_to_file_uri(&canon_actual);
        if canon_actual_uri != actual_uri_pkl {
            self.host
                .check_resolve_module(&canon_actual_uri)
                .map_err(|err| NativeError::security(format_module_host_error_for_napi(&err)))?;
        }

        match std::fs::remove_file(&actual_path) {
            Ok(()) => {}
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => {
                return Err(NativeError::io(format!(
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
                    return Err(NativeError::io(format!(
                        "I/O error writing test output file `{}`: {err}",
                        expected_path.display()
                    )));
                }
            }
        }

        let actual_map_expr = r#"
let (r = new PcfRenderer { omitNullProperties = false })
examples.toMap().mapValues((_, g) ->
  g.toList().map((v) ->
    let (s = r.renderValue(v))
    if (v is Typed || v is Dynamic || v is Listing || v is Mapping) "new " + s else s
  )
)
"#;
        let actual_bytes = self.eval_pkl_binary_locked(
            session,
            uri,
            contents,
            actual_map_expr,
            call_cancel,
            watchdog,
            timeout_ms,
        )?;
        let actual_decoded = decode_lossless(&actual_bytes)
            .map_err(|err| NativeError::decode(format!("Failed to decode examples: {err}")))?;
        let actual_groups = decode_rendered_examples_map(Some(&actual_decoded));

        let full_doc = self
            .eval_string_locked(
                session,
                uri,
                contents,
                RENDER_EXAMPLES_DOC_EXPR,
                call_cancel,
                watchdog,
                timeout_ms,
            )?
            .to_java_utf8();
        let expected_uri = path_to_file_uri(&expected_path).to_java_utf8();
        let actual_uri = path_to_file_uri(&actual_path).to_java_utf8();

        let (expected_groups, expected_source) = match std::fs::symlink_metadata(&expected_path) {
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => (None, None),
            Err(err) => {
                return Err(NativeError::io(format!(
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
                                    NativeError::security(format_module_host_error_for_napi(&err))
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
                                return Err(NativeError::security(
                                    format_module_host_error_for_napi(&err),
                                ));
                            }
                            Err(err) => {
                                return Err(NativeError::io(format!(
                                    "I/O error reading test output file `{}`: {}",
                                    expected_path.display(),
                                    format_module_host_error_for_napi(&err)
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
                        return Err(NativeError::security(format_module_host_error_for_napi(
                            &err,
                        )));
                    }
                    Err(err) => {
                        return Err(NativeError::io(format!(
                            "I/O error reading test output file `{}`: {}",
                            expected_path.display(),
                            format_module_host_error_for_napi(&err)
                        )));
                    }
                };
                if let Some(raw_bytes) = raw_bytes_opt {
                    let exp_text = String::from_utf8(raw_bytes).map_err(|err| {
                        NativeError::io(format!(
                            "I/O error reading test output file `{}`: {err}",
                            expected_path.display()
                        ))
                    })?;
                    let exp_uri_pkl = PklString::from(expected_uri.as_str());
                    let exp_text_pkl = PklString::from(exp_text.as_str());
                    let mut exp_session = self.host.new_public_session(
                        pkl_runtime::evaluation::frontend::DiagnosticProfile::ApiIdentity,
                    );
                    let exp_bytes_res = self.eval_pkl_binary_locked(
                        &mut exp_session,
                        &exp_uri_pkl,
                        Some(&exp_text_pkl),
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
                        call_cancel,
                        watchdog,
                        timeout_ms,
                    );
                    exp_session.close();
                    let exp_bytes = exp_bytes_res?;
                    let exp_decoded = decode_lossless(&exp_bytes).map_err(|err| {
                        NativeError::decode(format!("Failed to decode expected examples: {err}"))
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
            write_file_nofollow(&expected_path, content.as_bytes()).map_err(|err| {
                NativeError::io(format!(
                    "I/O error writing test output file `{}`: {err}",
                    expected_path.display()
                ))
            })?;
        }
        if let Some(ref content) = gen_actual {
            write_file_nofollow(&actual_path, content.as_bytes()).map_err(|err| {
                NativeError::io(format!(
                    "I/O error writing test output file `{}`: {err}",
                    actual_path.display()
                ))
            })?;
        }

        Ok((section, gen_expected, gen_actual))
    }
}

#[derive(Clone, Copy)]
enum TestReporterMode {
    Spec,
    Minimal,
    JUnit,
}

impl TestReporterMode {
    fn parse(raw: Option<&str>) -> std::result::Result<Self, NativeError> {
        match raw.unwrap_or("spec") {
            "spec" => Ok(Self::Spec),
            "minimal" => Ok(Self::Minimal),
            "junit" => Ok(Self::JUnit),
            other => Err(NativeError::invalid_argument(format!(
                "Invalid test reporter `{other}`. Expected `spec`, `minimal`, or `junit`."
            ))),
        }
    }
}

enum EvalOp {
    ContinueRunning {
        binary: bool,
    },
    OutputText {
        uri: PklString,
        contents: Option<PklString>,
    },
    OutputBytes {
        uri: PklString,
        contents: Option<PklString>,
    },
    OutputFiles {
        uri: PklString,
        contents: Option<PklString>,
    },
    OutputValue {
        uri: PklString,
        contents: Option<PklString>,
    },
    Expression {
        uri: PklString,
        contents: Option<PklString>,
        expression: PklString,
    },
    ExpressionValue {
        uri: PklString,
        contents: Option<PklString>,
        expression: PklString,
    },
    ExpressionPklBinary {
        uri: PklString,
        contents: Option<PklString>,
        expression: PklString,
    },
    Schema {
        uri: PklString,
        contents: Option<PklString>,
    },
    Test {
        uri: PklString,
        contents: Option<PklString>,
        overwrite: bool,
        reporter: TestReporterMode,
    },
}

pub enum EvalOutput {
    Text(PklString),
    Bytes(Vec<u8>),
    Files(Vec<(PklString, Vec<u8>)>),
    Value(DecodedValue),
    Schema(Box<ModuleSchema>),
    Test {
        results: TestResults,
        rendered: String,
    },
}

fn eval_output_to_js<'env>(
    env: &'env Env,
    res: std::result::Result<EvalOutput, NativeError>,
) -> Result<Object<'env>> {
    match res {
        Ok(EvalOutput::Text(text)) => to_js_outcome(env, Ok(js_pkl_string(env, &text)?)),
        Ok(EvalOutput::Bytes(bytes)) => to_js_outcome(env, Ok(Uint8Array::new(bytes))),
        Ok(EvalOutput::Files(files)) => {
            let mut pairs = Vec::with_capacity(files.len());
            for (k, bytes) in files {
                let mut entry = Object::new(env)?;
                entry.set("path", js_pkl_string(env, &k)?)?;
                entry.set("bytes", Uint8Array::new(bytes))?;
                pairs.push(entry);
            }
            to_js_outcome(env, Ok(Array::from_vec(env, pairs)?))
        }
        Ok(EvalOutput::Value(decoded)) => {
            let js_val = decoded_value_to_js(env, &decoded)?;
            to_js_outcome(env, Ok(js_val))
        }
        Ok(EvalOutput::Schema(schema)) => {
            let js_schema = module_schema_to_js(env, &schema)?;
            to_js_outcome(env, Ok(js_schema))
        }
        Ok(EvalOutput::Test { results, rendered }) => {
            let js_test = test_results_to_js(env, &results, &rendered)?;
            to_js_outcome(env, Ok(js_test))
        }
        Err(err) => to_js_outcome::<Null>(env, Err(err)),
    }
}

fn test_results_to_js<'env>(
    env: &'env Env,
    results: &TestResults,
    rendered: &str,
) -> Result<Object<'env>> {
    let mut out = Object::new(env)?;
    out.set("moduleName", results.module_name.as_str())?;
    out.set("displayUri", results.display_uri.as_str())?;
    out.set("failed", results.failed())?;
    out.set("totalTests", results.total_tests() as u32)?;
    out.set("totalFailures", results.total_failures() as u32)?;
    out.set("totalAsserts", results.total_asserts() as u32)?;
    out.set("totalAssertsFailed", results.total_asserts_failed() as u32)?;
    out.set("facts", test_section_to_js(env, &results.facts)?)?;
    out.set("examples", test_section_to_js(env, &results.examples)?)?;
    out.set("logs", results.logs.as_str())?;
    match &results.error {
        Some(err) => {
            let mut eo = Object::new(env)?;
            eo.set("message", err.message.as_str())?;
            eo.set("rendered", err.rendered.as_str())?;
            out.set("error", eo)?;
        }
        None => out.set("error", Null)?,
    }
    out.set("rendered", rendered)?;
    Ok(out)
}

fn test_section_to_js<'env>(env: &'env Env, section: &TestSectionResults) -> Result<Object<'env>> {
    let mut out = Object::new(env)?;
    out.set("name", section.name.as_str())?;
    out.set("failed", section.failed())?;
    out.set("totalTests", section.total_tests() as u32)?;
    out.set("totalFailures", section.total_failures() as u32)?;
    out.set("totalAsserts", section.total_asserts() as u32)?;
    let mut items = Vec::with_capacity(section.results.len());
    for r in &section.results {
        let mut ro = Object::new(env)?;
        ro.set("name", r.name.as_str())?;
        ro.set("totalAsserts", r.total_asserts as u32)?;
        ro.set("failed", r.is_failure())?;
        ro.set("isExampleWritten", r.is_example_written)?;
        let mut failures = Vec::with_capacity(r.failures.len());
        for f in &r.failures {
            let mut fo = Object::new(env)?;
            fo.set("kind", f.kind.as_str())?;
            fo.set("message", f.message.as_str())?;
            failures.push(fo);
        }
        ro.set("failures", Array::from_vec(env, failures)?)?;
        let mut errors = Vec::with_capacity(r.errors.len());
        for e in &r.errors {
            let mut eo = Object::new(env)?;
            eo.set("message", e.message.as_str())?;
            eo.set("rendered", e.rendered.as_str())?;
            errors.push(eo);
        }
        ro.set("errors", Array::from_vec(env, errors)?)?;
        items.push(ro);
    }
    out.set("results", Array::from_vec(env, items)?)?;
    Ok(out)
}

pub struct EvalAsyncTask {
    shared: EvaluatorShared,
    op: Option<std::result::Result<(EvalOp, Option<u64>), NativeError>>,
    call_cancel: Arc<AtomicBool>,
    http_cancel: Arc<CancelFlag>,
    start_gen: u64,
    call_id: Option<u64>,
    is_continue: bool,
}

impl<'task> ScopedTask<'task> for EvalAsyncTask {
    type Output = std::result::Result<EvalOutput, NativeError>;
    type JsValue = Object<'task>;

    fn compute(&mut self) -> Result<Self::Output> {
        let Some(parsed) = self.op.take() else {
            return Ok(Err(NativeError::evaluation(
                "Evaluation task already consumed.",
            )));
        };
        let (op, timeout_override) = match parsed {
            Ok(v) => v,
            Err(e) => {
                if let Some(id) = self.call_id {
                    let mut map = self
                        .shared
                        .in_flight_cancels
                        .lock()
                        .unwrap_or_else(|e| e.into_inner());
                    map.remove(&id);
                }
                if self.is_continue {
                    let mut guard = self.shared.state.lock().unwrap_or_else(|e| e.into_inner());
                    if guard.pending_continue {
                        guard.session.abort_active_operation();
                        let _ = guard.session.drain_cancelled_reads();
                        let _ = guard.session.drain_cancelled_modules();
                        let _ = guard.session.drain_cancelled_globs();
                        guard.pending_continue = false;
                        self.shared
                            .abort_pending_continue
                            .store(false, Ordering::SeqCst);
                        self.shared.pending_continue_cvar.notify_all();
                    }
                }
                return Ok(Err(e));
            }
        };
        Ok(self.shared.execute_op(
            op,
            timeout_override,
            Arc::clone(&self.call_cancel),
            Arc::clone(&self.http_cancel),
            self.start_gen,
            self.call_id,
        ))
    }

    fn resolve(&mut self, env: &'task Env, output: Self::Output) -> Result<Self::JsValue> {
        eval_output_to_js(env, output)
    }
}

pub struct EvalCloseTask {
    host: Arc<NativeHost>,
    state: Arc<Mutex<SessionState>>,
    pending_continue_cvar: Arc<Condvar>,
}

impl<'task> ScopedTask<'task> for EvalCloseTask {
    type Output = ();
    type JsValue = ();

    fn compute(&mut self) -> Result<Self::Output> {
        let mut guard = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if !guard.closed {
            guard.closed = true;
            guard.pending_continue = false;
            guard.columnar_buf.clear_and_release();
            guard.session.close();
            self.host.close();
            self.pending_continue_cvar.notify_all();
        }
        Ok(())
    }

    fn resolve(&mut self, _env: &'task Env, _output: Self::Output) -> Result<Self::JsValue> {
        Ok(())
    }
}

#[napi]
pub struct NativeEvaluator {
    shared: EvaluatorShared,
}

#[napi]
impl NativeEvaluator {
    #[napi]
    pub fn create<'env>(
        env: &'env Env,
        options: Option<NativeEvaluatorOptions>,
        project: Option<&NativeProject>,
        bridge_cb: Option<Function<'env, BridgeRequestJs, UnknownReturnValue>>,
    ) -> Result<Object<'env>> {
        let opts = options.unwrap_or_default();
        let built = match build_native_host(opts, project, bridge_cb) {
            Ok(b) => b,
            Err(err) => return to_js_outcome::<Null>(env, Err(err)),
        };
        let diagnostic_profile = DiagnosticProfile::ApiIdentity;
        let session = built.host.new_public_session(diagnostic_profile);
        let shared = EvaluatorShared {
            host: built.host,
            state: Arc::new(Mutex::new(SessionState {
                session,
                columnar_buf: pkl_runtime::vm::columnar_value::ColumnarValueBuffer::new(),
                closed: false,
                pending_continue: false,
            })),
            pending_continue_cvar: Arc::new(Condvar::new()),
            abort_pending_continue: Arc::new(AtomicBool::new(false)),
            bridge: built.bridge,
            logger: built.logger,
            frame_transformer: built.frame_transformer,
            closed_flag: Arc::new(AtomicBool::new(false)),
            cancel_generation: Arc::new(AtomicU64::new(0)),
            active_call_id: Arc::new(AtomicU64::new(0)),
            in_flight_cancels: Arc::new(Mutex::new(HashMap::new())),
            default_timeout_ms: built.default_timeout_ms,
            use_color: built.use_color,
            power_assertions: built.power_assertions,
            test_mode: built.test_mode,
            editor_url_scheme: built.editor_url_scheme,
            diagnostic_profile,
            has_js_callbacks: built.has_js_callbacks,
        };
        to_js_outcome(env, Ok(Self { shared }))
    }

    #[napi(getter)]
    pub fn is_closed(&self) -> bool {
        self.shared.closed_flag.load(Ordering::SeqCst)
    }

    #[napi]
    pub fn complete_bridge_callback(
        &self,
        id: BigInt,
        response: BridgeCallbackResponseIn,
    ) -> Result<()> {
        if let Some(ref bridge) = self.shared.bridge
            && let Ok(req_id) = parse_u64_bigint(&id)
        {
            bridge.complete(req_id, BridgeResponse::from(response));
        }
        Ok(())
    }

    #[napi]
    pub fn cancel(&self) {
        self.shared.cancel_generation.fetch_add(1, Ordering::SeqCst);
        let flags: Vec<(Arc<AtomicBool>, Arc<CancelFlag>)> = {
            let map = self
                .shared
                .in_flight_cancels
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            map.values().cloned().collect()
        };
        self.shared.pending_continue_cvar.notify_all();
        if !flags.is_empty() {
            for (flag, http_cancel) in flags {
                flag.store(true, Ordering::SeqCst);
                http_cancel.cancel();
            }
            self.shared.host.external_readers().cancel_all();
            if let Some(ref bridge) = self.shared.bridge {
                bridge.wake_all();
            }
        }
    }

    #[napi]
    pub fn abort_pending_fast(&self) {
        self.shared
            .abort_pending_continue
            .store(true, Ordering::SeqCst);
        self.shared.pending_continue_cvar.notify_all();
        if let Ok(mut guard) = self.shared.state.try_lock()
            && guard.pending_continue
        {
            guard.session.abort_active_operation();
            let _ = guard.session.drain_cancelled_reads();
            let _ = guard.session.drain_cancelled_modules();
            let _ = guard.session.drain_cancelled_globs();
            guard.pending_continue = false;
            self.shared
                .abort_pending_continue
                .store(false, Ordering::SeqCst);
            self.shared.pending_continue_cvar.notify_all();
        }
    }

    #[napi]
    pub fn cancel_call(&self, call_id: BigInt) {
        if let Ok(id) = parse_u64_bigint(&call_id) {
            let entry = {
                let map = self
                    .shared
                    .in_flight_cancels
                    .lock()
                    .unwrap_or_else(|e| e.into_inner());
                map.get(&id).cloned()
            };
            if let Some((flag, http_cancel)) = entry {
                flag.store(true, Ordering::SeqCst);
                http_cancel.cancel();
                if self.shared.active_call_id.load(Ordering::SeqCst) == id {
                    self.shared.host.external_readers().cancel_all();
                    if let Some(ref bridge) = self.shared.bridge {
                        bridge.wake_all();
                    }
                }
                self.shared.pending_continue_cvar.notify_all();
            }
        }
    }

    /// Synchronously closes if idle (returns `true`); returns `false` if an async task is
    /// still unwinding so the caller may await `closeAsync()`.
    #[napi]
    pub fn close_now(&self) -> bool {
        self.shared.closed_flag.store(true, Ordering::SeqCst);
        self.cancel();
        if let Some(ref bridge) = self.shared.bridge {
            bridge.abort_all();
        }
        self.shared.pending_continue_cvar.notify_all();
        if let Ok(mut guard) = self.shared.state.try_lock() {
            if !guard.closed {
                guard.closed = true;
                guard.pending_continue = false;
                guard.columnar_buf.clear_and_release();
                guard.session.close();
                self.shared.host.close();
            }
            return true;
        }
        // Only force-cancel the host (including immediate external reader termination)
        // when a worker still holds `state`; idle close must deliver graceful `0x32`.
        self.shared.host.cancel();
        false
    }

    #[napi]
    pub fn close_async(&self) -> AsyncTask<EvalCloseTask> {
        self.shared.closed_flag.store(true, Ordering::SeqCst);
        self.cancel();
        if let Some(ref bridge) = self.shared.bridge {
            bridge.abort_all();
        }
        self.shared.pending_continue_cvar.notify_all();
        if self.shared.state.try_lock().is_err() {
            self.shared.host.cancel();
        }
        AsyncTask::new(EvalCloseTask {
            host: Arc::clone(&self.shared.host),
            state: Arc::clone(&self.shared.state),
            pending_continue_cvar: Arc::clone(&self.shared.pending_continue_cvar),
        })
    }

    fn spawn_task(
        &self,
        op: std::result::Result<(EvalOp, Option<u64>), NativeError>,
        call_id: Option<BigInt>,
    ) -> AsyncTask<EvalAsyncTask> {
        self.spawn_task_inner(op, call_id, false)
    }

    fn spawn_task_inner(
        &self,
        op: std::result::Result<(EvalOp, Option<u64>), NativeError>,
        call_id: Option<BigInt>,
        is_continue: bool,
    ) -> AsyncTask<EvalAsyncTask> {
        let start_gen = self.shared.cancel_generation.load(Ordering::SeqCst);
        let call_cancel = Arc::new(AtomicBool::new(false));
        let http_cancel = CancelFlag::new();
        let parsed_id = call_id.as_ref().and_then(|bi| parse_u64_bigint(bi).ok());
        if let Some(id) = parsed_id {
            let mut map = self
                .shared
                .in_flight_cancels
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            map.insert(id, (Arc::clone(&call_cancel), Arc::clone(&http_cancel)));
        }
        AsyncTask::new(EvalAsyncTask {
            shared: self.shared.clone(),
            op: Some(op),
            call_cancel,
            http_cancel,
            start_gen,
            call_id: parsed_id,
            is_continue,
        })
    }

    fn run_sync<'env>(
        &self,
        env: &'env Env,
        op: std::result::Result<(EvalOp, Option<u64>), NativeError>,
    ) -> Result<Object<'env>> {
        if self.shared.has_js_callbacks {
            return to_js_outcome::<Null>(
                env,
                Err(NativeError::invalid_argument(
                    "Synchronous evaluation methods cannot be used with custom JS callbacks; use the async method instead.",
                )),
            );
        }
        let start_gen = self.shared.cancel_generation.load(Ordering::SeqCst);
        let call_cancel = Arc::new(AtomicBool::new(false));
        let http_cancel = CancelFlag::new();
        let outcome = op.and_then(|(op, timeout)| {
            self.shared
                .execute_op(op, timeout, call_cancel, http_cancel, start_gen, None)
        });
        eval_output_to_js(env, outcome)
    }

    /// Bounded synchronous try-poll fast path for warm-cached pure-standard-host calls
    /// with no custom JS hooks/readers or timeouts. Only enters when `uri` is already
    /// warm-cached in the session; runs a single bounded VM poll (`32_768` steps). If it
    /// completes immediately (`HostPoll::Ready`), returns the JS outcome directly without
    /// scheduling a libuv worker thread. If it yields or requests Host I/O, leaves the
    /// started operation intact (never cancelling the session) and returns a sentinel
    /// object `{ __continueAsync: true }` so JS immediately continues on `AsyncTask`.
    fn try_poll_cached_fast<'env>(
        &self,
        env: &'env Env,
        uri: &Utf16String,
        contents: Option<&Utf16String>,
        expression: SourceText,
        binary: bool,
    ) -> Result<Option<Object<'env>>> {
        if self.shared.has_js_callbacks
            || self.shared.default_timeout_ms.is_some()
            || self.shared.closed_flag.load(Ordering::SeqCst)
        {
            return Ok(None);
        }
        let Ok(mut guard) = self.shared.state.try_lock() else {
            return Ok(None);
        };
        if guard.closed || guard.pending_continue || guard.session.is_closed() {
            return Ok(None);
        }
        let uri_pkl = utf16_to_pkl(uri);
        let contents_pkl = contents.map(utf16_to_pkl);
        if contents_pkl.is_some() && uri_pkl == "repl:text" {
            return Ok(None);
        }
        let spec = match self.shared.prepare_spec(&uri_pkl, contents_pkl.as_ref()) {
            Ok(spec) => spec,
            Err(_) => return Ok(None),
        };
        if !guard.session.has_warm_cached_module(&spec.uri) {
            return Ok(None);
        }
        let now = self.shared.host.clock_ms();
        let started = if binary {
            guard
                .session
                .start_evaluation_pkl_binary(spec, expression, now, None)
        } else {
            guard.session.start_evaluation(spec, expression, now, None)
        };
        if let Err(err) = started {
            let mapped = self.shared.map_runtime_err(&guard.session, err, None);
            return Ok(Some(eval_output_to_js(env, Err(mapped))?));
        }
        match guard.session.poll(now, 32_768) {
            HostPoll::Ready(result) => {
                if binary {
                    match result.and_then(SourceSession::result_bytes) {
                        Ok(bytes) => {
                            let slice: &[u8] = &bytes;
                            let copied = BufferSlice::copy_from(env, slice)?;
                            return Ok(Some(to_js_outcome(env, Ok(copied))?));
                        }
                        Err(err) => {
                            let mapped = self.shared.map_runtime_err(&guard.session, err, None);
                            return Ok(Some(eval_output_to_js(env, Err(mapped))?));
                        }
                    }
                }
                let outcome = result
                    .and_then(|val| SourceSession::result_string(val).map(EvalOutput::Text))
                    .map_err(|err| self.shared.map_runtime_err(&guard.session, err, None));
                Ok(Some(eval_output_to_js(env, outcome)?))
            }
            HostPoll::Yielded
            | HostPoll::Module { .. }
            | HostPoll::Read { .. }
            | HostPoll::Glob { .. }
            | HostPoll::Log { .. }
            | HostPoll::TransformFrame { .. } => {
                guard.pending_continue = true;
                let mut marker = Object::new(env)?;
                marker.set("__continueAsync", true)?;
                Ok(Some(marker))
            }
        }
    }

    #[napi]
    pub fn continue_pending_fast(
        &self,
        binary: bool,
        timeout_ms: Option<BigInt>,
        call_id: Option<BigInt>,
    ) -> AsyncTask<EvalAsyncTask> {
        let parsed =
            parse_opt_timeout(&timeout_ms).map(|t| (EvalOp::ContinueRunning { binary }, t));
        self.spawn_task_inner(parsed, call_id, true)
    }

    #[napi]
    pub fn try_evaluate_output_text_fast<'env>(
        &self,
        env: &'env Env,
        uri: Utf16String,
        contents: Option<Utf16String>,
    ) -> Result<Option<Object<'env>>> {
        self.try_poll_cached_fast(
            env,
            &uri,
            contents.as_ref(),
            SourceText::from("output.text"),
            false,
        )
    }

    #[napi]
    pub fn try_evaluate_expression_fast<'env>(
        &self,
        env: &'env Env,
        uri: Utf16String,
        contents: Option<Utf16String>,
        expression: Utf16String,
    ) -> Result<Option<Object<'env>>> {
        let expr_src = SourceText::from(utf16_to_pkl(&expression));
        self.try_poll_cached_fast(env, &uri, contents.as_ref(), expr_src, false)
    }

    #[napi]
    pub fn try_evaluate_expression_pkl_binary_fast<'env>(
        &self,
        env: &'env Env,
        uri: Utf16String,
        contents: Option<Utf16String>,
        expression: Utf16String,
    ) -> Result<Option<Object<'env>>> {
        let expr_src = SourceText::from(utf16_to_pkl(&expression));
        self.try_poll_cached_fast(env, &uri, contents.as_ref(), expr_src, true)
    }

    #[napi]
    pub fn try_evaluate_expression_columnar_fast<'env>(
        &self,
        env: &'env Env,
        uri: Utf16String,
        contents: Option<Utf16String>,
        expression: Utf16String,
    ) -> Result<Option<BufferSlice<'env>>> {
        if self.shared.has_js_callbacks
            || self.shared.default_timeout_ms.is_some()
            || self.shared.closed_flag.load(Ordering::SeqCst)
        {
            return Ok(None);
        }
        let Ok(mut guard) = self.shared.state.try_lock() else {
            return Ok(None);
        };
        if guard.closed || guard.pending_continue || guard.session.is_closed() {
            return Ok(None);
        }
        let uri_pkl = utf16_to_pkl(&uri);
        let contents_pkl = contents.as_ref().map(utf16_to_pkl);
        if contents_pkl.is_some() && uri_pkl == "repl:text" {
            return Ok(None);
        }
        let spec = match self.shared.prepare_spec(&uri_pkl, contents_pkl.as_ref()) {
            Ok(spec) => spec,
            Err(_) => return Ok(None),
        };
        let expr_pkl = utf16_to_pkl(&expression);
        let Some(expr_str) = expr_pkl.as_str() else {
            return Ok(None);
        };
        let SessionState {
            session,
            columnar_buf,
            ..
        } = &mut *guard;
        if !session.try_pack_warm_expression_columnar(&spec.uri, expr_str, columnar_buf) {
            return Ok(None);
        }
        let copied = BufferSlice::copy_from(env, columnar_buf.as_bytes())?;
        columnar_buf.shrink_if_oversized();
        Ok(Some(copied))
    }

    #[napi]
    pub fn evaluate_output_text(
        &self,
        uri: Utf16String,
        contents: Option<Utf16String>,
        timeout_ms: Option<BigInt>,
        call_id: Option<BigInt>,
    ) -> AsyncTask<EvalAsyncTask> {
        let parsed = parse_opt_timeout(&timeout_ms).map(|t| {
            (
                EvalOp::OutputText {
                    uri: utf16_to_pkl(&uri),
                    contents: contents.as_ref().map(utf16_to_pkl),
                },
                t,
            )
        });
        self.spawn_task(parsed, call_id)
    }

    #[napi]
    pub fn evaluate_output_text_sync<'env>(
        &self,
        env: &'env Env,
        uri: Utf16String,
        contents: Option<Utf16String>,
        timeout_ms: Option<BigInt>,
    ) -> Result<Object<'env>> {
        let parsed = parse_opt_timeout(&timeout_ms).map(|t| {
            (
                EvalOp::OutputText {
                    uri: utf16_to_pkl(&uri),
                    contents: contents.as_ref().map(utf16_to_pkl),
                },
                t,
            )
        });
        self.run_sync(env, parsed)
    }

    #[napi]
    pub fn evaluate_output_bytes(
        &self,
        uri: Utf16String,
        contents: Option<Utf16String>,
        timeout_ms: Option<BigInt>,
        call_id: Option<BigInt>,
    ) -> AsyncTask<EvalAsyncTask> {
        let parsed = parse_opt_timeout(&timeout_ms).map(|t| {
            (
                EvalOp::OutputBytes {
                    uri: utf16_to_pkl(&uri),
                    contents: contents.as_ref().map(utf16_to_pkl),
                },
                t,
            )
        });
        self.spawn_task(parsed, call_id)
    }

    #[napi]
    pub fn evaluate_output_bytes_sync<'env>(
        &self,
        env: &'env Env,
        uri: Utf16String,
        contents: Option<Utf16String>,
        timeout_ms: Option<BigInt>,
    ) -> Result<Object<'env>> {
        let parsed = parse_opt_timeout(&timeout_ms).map(|t| {
            (
                EvalOp::OutputBytes {
                    uri: utf16_to_pkl(&uri),
                    contents: contents.as_ref().map(utf16_to_pkl),
                },
                t,
            )
        });
        self.run_sync(env, parsed)
    }

    #[napi]
    pub fn evaluate_output_files(
        &self,
        uri: Utf16String,
        contents: Option<Utf16String>,
        timeout_ms: Option<BigInt>,
        call_id: Option<BigInt>,
    ) -> AsyncTask<EvalAsyncTask> {
        let parsed = parse_opt_timeout(&timeout_ms).map(|t| {
            (
                EvalOp::OutputFiles {
                    uri: utf16_to_pkl(&uri),
                    contents: contents.as_ref().map(utf16_to_pkl),
                },
                t,
            )
        });
        self.spawn_task(parsed, call_id)
    }

    #[napi]
    pub fn evaluate_output_files_sync<'env>(
        &self,
        env: &'env Env,
        uri: Utf16String,
        contents: Option<Utf16String>,
        timeout_ms: Option<BigInt>,
    ) -> Result<Object<'env>> {
        let parsed = parse_opt_timeout(&timeout_ms).map(|t| {
            (
                EvalOp::OutputFiles {
                    uri: utf16_to_pkl(&uri),
                    contents: contents.as_ref().map(utf16_to_pkl),
                },
                t,
            )
        });
        self.run_sync(env, parsed)
    }

    #[napi]
    pub fn evaluate_output_value(
        &self,
        uri: Utf16String,
        contents: Option<Utf16String>,
        timeout_ms: Option<BigInt>,
        call_id: Option<BigInt>,
    ) -> AsyncTask<EvalAsyncTask> {
        let parsed = parse_opt_timeout(&timeout_ms).map(|t| {
            (
                EvalOp::OutputValue {
                    uri: utf16_to_pkl(&uri),
                    contents: contents.as_ref().map(utf16_to_pkl),
                },
                t,
            )
        });
        self.spawn_task(parsed, call_id)
    }

    #[napi]
    pub fn evaluate_output_value_sync<'env>(
        &self,
        env: &'env Env,
        uri: Utf16String,
        contents: Option<Utf16String>,
        timeout_ms: Option<BigInt>,
    ) -> Result<Object<'env>> {
        let parsed = parse_opt_timeout(&timeout_ms).map(|t| {
            (
                EvalOp::OutputValue {
                    uri: utf16_to_pkl(&uri),
                    contents: contents.as_ref().map(utf16_to_pkl),
                },
                t,
            )
        });
        self.run_sync(env, parsed)
    }

    #[napi]
    pub fn evaluate_expression(
        &self,
        uri: Utf16String,
        contents: Option<Utf16String>,
        expression: Utf16String,
        timeout_ms: Option<BigInt>,
        call_id: Option<BigInt>,
    ) -> AsyncTask<EvalAsyncTask> {
        let parsed = parse_opt_timeout(&timeout_ms).map(|t| {
            (
                EvalOp::Expression {
                    uri: utf16_to_pkl(&uri),
                    contents: contents.as_ref().map(utf16_to_pkl),
                    expression: utf16_to_pkl(&expression),
                },
                t,
            )
        });
        self.spawn_task(parsed, call_id)
    }

    #[napi]
    pub fn evaluate_expression_sync<'env>(
        &self,
        env: &'env Env,
        uri: Utf16String,
        contents: Option<Utf16String>,
        expression: Utf16String,
        timeout_ms: Option<BigInt>,
    ) -> Result<Object<'env>> {
        let parsed = parse_opt_timeout(&timeout_ms).map(|t| {
            (
                EvalOp::Expression {
                    uri: utf16_to_pkl(&uri),
                    contents: contents.as_ref().map(utf16_to_pkl),
                    expression: utf16_to_pkl(&expression),
                },
                t,
            )
        });
        self.run_sync(env, parsed)
    }

    #[napi]
    pub fn evaluate_expression_value(
        &self,
        uri: Utf16String,
        contents: Option<Utf16String>,
        expression: Utf16String,
        timeout_ms: Option<BigInt>,
        call_id: Option<BigInt>,
    ) -> AsyncTask<EvalAsyncTask> {
        let parsed = parse_opt_timeout(&timeout_ms).map(|t| {
            (
                EvalOp::ExpressionValue {
                    uri: utf16_to_pkl(&uri),
                    contents: contents.as_ref().map(utf16_to_pkl),
                    expression: utf16_to_pkl(&expression),
                },
                t,
            )
        });
        self.spawn_task(parsed, call_id)
    }

    #[napi]
    pub fn evaluate_expression_value_sync<'env>(
        &self,
        env: &'env Env,
        uri: Utf16String,
        contents: Option<Utf16String>,
        expression: Utf16String,
        timeout_ms: Option<BigInt>,
    ) -> Result<Object<'env>> {
        let parsed = parse_opt_timeout(&timeout_ms).map(|t| {
            (
                EvalOp::ExpressionValue {
                    uri: utf16_to_pkl(&uri),
                    contents: contents.as_ref().map(utf16_to_pkl),
                    expression: utf16_to_pkl(&expression),
                },
                t,
            )
        });
        self.run_sync(env, parsed)
    }

    #[napi]
    pub fn evaluate_expression_pkl_binary(
        &self,
        uri: Utf16String,
        contents: Option<Utf16String>,
        expression: Utf16String,
        timeout_ms: Option<BigInt>,
        call_id: Option<BigInt>,
    ) -> AsyncTask<EvalAsyncTask> {
        let parsed = parse_opt_timeout(&timeout_ms).map(|t| {
            (
                EvalOp::ExpressionPklBinary {
                    uri: utf16_to_pkl(&uri),
                    contents: contents.as_ref().map(utf16_to_pkl),
                    expression: utf16_to_pkl(&expression),
                },
                t,
            )
        });
        self.spawn_task(parsed, call_id)
    }

    #[napi]
    pub fn evaluate_expression_pkl_binary_sync<'env>(
        &self,
        env: &'env Env,
        uri: Utf16String,
        contents: Option<Utf16String>,
        expression: Utf16String,
        timeout_ms: Option<BigInt>,
    ) -> Result<Object<'env>> {
        let parsed = parse_opt_timeout(&timeout_ms).map(|t| {
            (
                EvalOp::ExpressionPklBinary {
                    uri: utf16_to_pkl(&uri),
                    contents: contents.as_ref().map(utf16_to_pkl),
                    expression: utf16_to_pkl(&expression),
                },
                t,
            )
        });
        self.run_sync(env, parsed)
    }

    #[napi]
    pub fn evaluate_schema(
        &self,
        uri: Utf16String,
        contents: Option<Utf16String>,
        timeout_ms: Option<BigInt>,
        call_id: Option<BigInt>,
    ) -> AsyncTask<EvalAsyncTask> {
        let parsed = parse_opt_timeout(&timeout_ms).map(|t| {
            (
                EvalOp::Schema {
                    uri: utf16_to_pkl(&uri),
                    contents: contents.as_ref().map(utf16_to_pkl),
                },
                t,
            )
        });
        self.spawn_task(parsed, call_id)
    }

    #[napi]
    pub fn evaluate_schema_sync<'env>(
        &self,
        env: &'env Env,
        uri: Utf16String,
        contents: Option<Utf16String>,
        timeout_ms: Option<BigInt>,
    ) -> Result<Object<'env>> {
        let parsed = parse_opt_timeout(&timeout_ms).map(|t| {
            (
                EvalOp::Schema {
                    uri: utf16_to_pkl(&uri),
                    contents: contents.as_ref().map(utf16_to_pkl),
                },
                t,
            )
        });
        self.run_sync(env, parsed)
    }

    #[napi]
    pub fn evaluate_test(
        &self,
        uri: Utf16String,
        contents: Option<Utf16String>,
        overwrite: Option<bool>,
        reporter: Option<String>,
        timeout_ms: Option<BigInt>,
        call_id: Option<BigInt>,
    ) -> AsyncTask<EvalAsyncTask> {
        let parsed = parse_opt_timeout(&timeout_ms).and_then(|t| {
            let rep = TestReporterMode::parse(reporter.as_deref())?;
            Ok((
                EvalOp::Test {
                    uri: utf16_to_pkl(&uri),
                    contents: contents.as_ref().map(utf16_to_pkl),
                    overwrite: overwrite.unwrap_or(false),
                    reporter: rep,
                },
                t,
            ))
        });
        self.spawn_task(parsed, call_id)
    }

    #[napi]
    pub fn evaluate_test_sync<'env>(
        &self,
        env: &'env Env,
        uri: Utf16String,
        contents: Option<Utf16String>,
        overwrite: Option<bool>,
        reporter: Option<String>,
        timeout_ms: Option<BigInt>,
    ) -> Result<Object<'env>> {
        let parsed = parse_opt_timeout(&timeout_ms).and_then(|t| {
            let rep = TestReporterMode::parse(reporter.as_deref())?;
            Ok((
                EvalOp::Test {
                    uri: utf16_to_pkl(&uri),
                    contents: contents.as_ref().map(utf16_to_pkl),
                    overwrite: overwrite.unwrap_or(false),
                    reporter: rep,
                },
                t,
            ))
        });
        self.run_sync(env, parsed)
    }
}

impl Drop for NativeEvaluator {
    fn drop(&mut self) {
        // Never block on `self.shared.state.lock()` on the V8 main thread: a libuv worker holding
        // `state` may be waiting on a JS callback or unwinding. Signal abort and try_lock only;
        // if a worker still holds `state`, it checks `closed_flag` on exit and closes resources.
        self.shared.closed_flag.store(true, Ordering::SeqCst);
        self.cancel();
        if let Some(ref bridge) = self.shared.bridge {
            bridge.abort_all();
        }
        self.shared.pending_continue_cvar.notify_all();
        if let Ok(mut guard) = self.shared.state.try_lock() {
            if !guard.closed {
                guard.closed = true;
                guard.pending_continue = false;
                guard.columnar_buf.clear_and_release();
                guard.session.close();
                self.shared.host.close();
            }
        } else {
            self.shared.host.cancel();
        }
    }
}

fn parse_opt_timeout(timeout: &Option<BigInt>) -> std::result::Result<Option<u64>, NativeError> {
    match timeout {
        Some(bi) => Ok(Some(parse_u64_bigint(bi)?)),
        None => Ok(None),
    }
}

// =============================================================================
// Static Import Analysis, Formatter, Path Helper, 64-bit Monotonic Clock
// =============================================================================

#[napi]
pub struct NativeAnalyzer {
    host: Arc<NativeHost>,
    bridge: Option<Arc<JsBridge>>,
    timeout_ms: Option<u64>,
    cancelled: Arc<AtomicBool>,
    active_cancel: ActiveCallCancelSlot,
}

#[napi]
impl NativeAnalyzer {
    #[napi]
    pub fn create<'env>(
        env: &'env Env,
        options: Option<NativeEvaluatorOptions>,
        project: Option<&NativeProject>,
        bridge_cb: Option<Function<'env, BridgeRequestJs, UnknownReturnValue>>,
    ) -> Result<Object<'env>> {
        let opts = options.unwrap_or_default();
        let built = match build_native_host(opts, project, bridge_cb) {
            Ok(b) => b,
            Err(err) => return to_js_outcome::<Null>(env, Err(err)),
        };
        to_js_outcome(
            env,
            Ok(Self {
                host: built.host,
                bridge: built.bridge,
                timeout_ms: built.default_timeout_ms,
                cancelled: Arc::new(AtomicBool::new(false)),
                active_cancel: Arc::new(Mutex::new(None)),
            }),
        )
    }

    #[napi]
    pub fn complete_bridge_callback(
        &self,
        id: BigInt,
        response: BridgeCallbackResponseIn,
    ) -> Result<()> {
        if let Some(ref bridge) = self.bridge
            && let Ok(req_id) = parse_u64_bigint(&id)
        {
            bridge.complete(req_id, BridgeResponse::from(response));
        }
        Ok(())
    }

    #[napi]
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
        if let Some((c, http_c)) = self
            .active_cancel
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
        {
            c.store(true, Ordering::SeqCst);
            http_c.cancel();
            self.host.external_readers().cancel_all();
        }
        if let Some(ref bridge) = self.bridge {
            bridge.wake_all();
        }
    }

    #[napi]
    pub fn close(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
        let had_active = {
            let slot = self
                .active_cancel
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clone();
            if let Some((c, http_c)) = slot {
                c.store(true, Ordering::SeqCst);
                http_c.cancel();
                true
            } else {
                false
            }
        };
        if let Some(ref bridge) = self.bridge {
            bridge.abort_all();
        }
        if had_active {
            // Do not block the V8 main thread closing host resources while an async
            // analysis worker is still unwinding; signal cancellation and let the worker's
            // cleanup block call `self.host.close()`.
            self.host.cancel();
        } else {
            self.host.close();
        }
    }

    #[napi]
    pub fn import_graph(&self, uris: Vec<Utf16String>) -> AsyncTask<AnalyzeImportGraphTask> {
        let roots: Vec<PklString> = uris.iter().map(utf16_to_pkl).collect();
        AsyncTask::new(AnalyzeImportGraphTask {
            host: Arc::clone(&self.host),
            bridge: self.bridge.clone(),
            timeout_ms: self.timeout_ms,
            cancelled: Arc::clone(&self.cancelled),
            active_cancel: Arc::clone(&self.active_cancel),
            roots,
        })
    }
}

impl Drop for NativeAnalyzer {
    fn drop(&mut self) {
        self.close();
    }
}

pub struct AnalyzeImportGraphTask {
    host: Arc<NativeHost>,
    bridge: Option<Arc<JsBridge>>,
    timeout_ms: Option<u64>,
    cancelled: Arc<AtomicBool>,
    active_cancel: ActiveCallCancelSlot,
    roots: Vec<PklString>,
}

pub struct AnalyzedGraphData {
    imports: Vec<(PklString, Vec<PklString>)>,
    resolved_imports: Vec<(PklString, PklString)>,
}

fn stdlib_analysis_source(uri: &str) -> Option<&'static str> {
    match uri {
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

fn format_module_host_error_for_napi(err: &ModuleHostError) -> String {
    match err {
        ModuleHostError::NotFound => "Module not found.".to_string(),
        ModuleHostError::NoLoader => "No module loader is registered for this URI.".to_string(),
        ModuleHostError::UriSyntax { reason } => {
            format!("Invalid module URI syntax: {}", reason.to_java_utf8())
        }
        ModuleHostError::ExternalReaderProcess { message } => match message {
            Some(msg) => format!(
                "Failed to communicate with external reader process: {}",
                msg.to_java_utf8()
            ),
            None => "Failed to communicate with external reader process.".to_string(),
        },
        ModuleHostError::Io(io) => match &io.message {
            Some(msg) => msg.to_java_utf8(),
            None => "I/O error loading module.".to_string(),
        },
        ModuleHostError::Denied(msg) | ModuleHostError::Unsupported(msg) => msg.to_java_utf8(),
        ModuleHostError::SecurityManagerException { message } => message
            .as_ref()
            .map(PklString::to_java_utf8)
            .unwrap_or_else(|| "Module access denied by security manager.".to_string()),
        ModuleHostError::HttpClientException { message } => message
            .as_ref()
            .map(PklString::to_java_utf8)
            .unwrap_or_else(|| "HTTP client error loading module.".to_string()),
        ModuleHostError::EvalError { message, hint } => match hint {
            Some(hint) => format!("{}\n{}", message.to_java_utf8(), hint.to_java_utf8()),
            None => message.to_java_utf8(),
        },
    }
}

fn format_analysis_failure_for_napi(
    failure: &pkl_runtime::modules::analyze::AnalysisFailure,
) -> NativeError {
    use pkl_runtime::modules::analyze::{FailureCause, UriFailure};
    let uri = failure.module_uri.to_java_utf8();
    match &failure.cause {
        FailureCause::Host(err) => {
            let msg = format!(
                "Failed to analyze module `{uri}`: {}",
                format_module_host_error_for_napi(err)
            );
            if matches!(err, ModuleHostError::SecurityManagerException { .. }) {
                NativeError::security(msg)
            } else {
                NativeError::evaluation(msg)
            }
        }
        FailureCause::Syntax(diag) => NativeError::evaluation(format!(
            "Syntax error while analyzing module `{uri}`: {}",
            diag.message.to_java_utf8()
        )),
        FailureCause::Uri { input, reason } => {
            let input_str = input.to_java_utf8();
            let detail = match reason {
                UriFailure::Relative => "URI must be absolute.".to_string(),
                UriFailure::Syntax { reason, .. } => reason.to_java_utf8(),
                UriFailure::Unsupported(msg) => msg.to_java_utf8(),
            };
            NativeError::evaluation(format!(
                "Invalid URI `{input_str}` while analyzing `{uri}`: {detail}"
            ))
        }
        FailureCause::Unsupported(msg) => NativeError::evaluation(format!(
            "Unsupported import construct while analyzing `{uri}`: {}",
            msg.to_java_utf8()
        )),
    }
}

fn analyze_import_graph_with_stdlib(
    host: &NativeHost,
    roots: Vec<PklString>,
    call_cancel: &Arc<AtomicBool>,
    watchdog: &SafeTimeoutWatchdog,
    timeout_ms: Option<u64>,
) -> std::result::Result<AnalyzedGraphData, NativeError> {
    let mut analysis = ImportGraphAnalysis::new(roots);
    let outcome = loop {
        if watchdog.triggered() {
            break Err(NativeError::timeout(format_timeout_message(
                timeout_ms.unwrap_or(0),
            )));
        }
        if call_cancel.load(Ordering::SeqCst) {
            break Err(NativeError::cancelled(
                "Import graph analysis was cancelled.",
            ));
        }
        match analysis.poll() {
            AnalysisPoll::Ready(Ok(graph)) => {
                let mut imports = Vec::with_capacity(graph.nodes.len());
                let mut resolved_imports = Vec::with_capacity(graph.nodes.len());
                for node in graph.nodes {
                    imports.push((node.uri.clone(), node.imports));
                    resolved_imports.push((node.uri, node.resolved_uri));
                }
                break Ok(AnalyzedGraphData {
                    imports,
                    resolved_imports,
                });
            }
            AnalysisPoll::Ready(Err(failure)) => {
                break Err(format_analysis_failure_for_napi(&failure));
            }
            AnalysisPoll::Closed => {
                break Err(NativeError::closed("Analysis closed unexpectedly."));
            }
            AnalysisPoll::Request(request) => {
                let completion = match &request.kind {
                    ModuleRequestKind::ResolveKey {
                        key,
                        resolved_key_id,
                        ..
                    } if key.kind == ModuleKeyKind::StandardLibrary => {
                        if let Err(err) = host.check_resolve_module(&key.uri) {
                            ModuleCompletion::KeyResolved(Err(err))
                        } else {
                            let uri_str = key.uri.to_java_utf8();
                            if stdlib_analysis_source(&uri_str).is_some() {
                                ModuleCompletion::KeyResolved(Ok(ResolvedModuleKey {
                                    id: *resolved_key_id,
                                    key: key.clone(),
                                    handle: PklString::default(),
                                    module: ResolvedModule {
                                        original_uri: key.uri.clone(),
                                        resolved_uri: key.uri.clone(),
                                        source_uri: key.uri.clone(),
                                    },
                                }))
                            } else {
                                ModuleCompletion::KeyResolved(Err(ModuleHostError::NotFound))
                            }
                        }
                    }
                    ModuleRequestKind::LoadSource { key, resolved }
                        if key.key.kind == ModuleKeyKind::StandardLibrary =>
                    {
                        let uri_str = resolved.resolved_uri.to_java_utf8();
                        match stdlib_analysis_source(&uri_str) {
                            Some(src) => ModuleCompletion::Source(Ok(RuntimeModuleSource {
                                text: SourceText::from(src),
                                inferred_name: PklString::default(),
                            })),
                            None => ModuleCompletion::Source(Err(ModuleHostError::NotFound)),
                        }
                    }
                    ModuleRequestKind::Capabilities { key: Some(key), .. }
                        if key.kind == ModuleKeyKind::StandardLibrary =>
                    {
                        ModuleCompletion::Capabilities(Ok(ModuleCapabilities {
                            local: true,
                            hierarchical: false,
                            fragment_paths: false,
                        }))
                    }
                    _ => host.handle_module_request(&request),
                };
                if let Err(msg) = analysis.complete(request.id, completion) {
                    break Err(NativeError::evaluation(msg));
                }
            }
        }
    };
    analysis.close();
    outcome
}

impl<'task> ScopedTask<'task> for AnalyzeImportGraphTask {
    type Output = std::result::Result<AnalyzedGraphData, NativeError>;
    type JsValue = Object<'task>;

    fn compute(&mut self) -> Result<Self::Output> {
        if self.cancelled.load(Ordering::SeqCst) {
            self.host.close();
            if let Some(ref bridge) = self.bridge {
                bridge.abort_all();
            }
            return Ok(Err(NativeError::cancelled(
                "Import graph analysis was cancelled.",
            )));
        }
        let call_cancel = Arc::new(AtomicBool::new(false));
        let http_cancel = CancelFlag::new();
        {
            let mut active = self.active_cancel.lock().unwrap_or_else(|e| e.into_inner());
            *active = Some((Arc::clone(&call_cancel), Arc::clone(&http_cancel)));
        }
        if self.cancelled.load(Ordering::SeqCst) {
            call_cancel.store(true, Ordering::SeqCst);
            http_cancel.cancel();
        }
        let watchdog = SafeTimeoutWatchdog::start(
            self.timeout_ms,
            Arc::clone(&call_cancel),
            Arc::clone(&http_cancel),
            Arc::clone(&self.host),
            self.bridge.clone(),
        );
        let _scoped_http_cancel =
            ScopedEvalCancel::push_with_deadline(Arc::clone(&http_cancel), watchdog.deadline());
        let _bridge_guard = self
            .bridge
            .as_ref()
            .map(|b| b.enter_call(Arc::clone(&call_cancel), watchdog.deadline()));

        let res = analyze_import_graph_with_stdlib(
            &self.host,
            self.roots.clone(),
            &call_cancel,
            &watchdog,
            self.timeout_ms,
        );
        let timed_out = watchdog.stop();
        {
            let mut active = self.active_cancel.lock().unwrap_or_else(|e| e.into_inner());
            *active = None;
        }
        self.host.close();
        if let Some(ref bridge) = self.bridge {
            bridge.abort_all();
        }

        if timed_out && res.is_err() {
            return Ok(Err(NativeError::timeout(format_timeout_message(
                self.timeout_ms.unwrap_or(0),
            ))));
        }
        if (call_cancel.load(Ordering::SeqCst) || self.cancelled.load(Ordering::SeqCst))
            && res.is_err()
        {
            return Ok(Err(NativeError::cancelled(
                "Import graph analysis was cancelled.",
            )));
        }

        Ok(res)
    }

    fn resolve(&mut self, env: &'task Env, output: Self::Output) -> Result<Self::JsValue> {
        match output {
            Ok(data) => {
                let mut imports_obj = Object::new(env)?;
                for (uri, targets) in data.imports {
                    let mut arr = Vec::with_capacity(targets.len());
                    for t in targets {
                        arr.push(js_pkl_string(env, &t)?);
                    }
                    let js_arr = Array::from_vec(env, arr)?;
                    define_own_prop(env, &mut imports_obj, &uri.to_java_utf8(), &js_arr)?;
                }
                let mut resolved_obj = Object::new(env)?;
                for (uri, resolved) in data.resolved_imports {
                    let js_resolved = js_pkl_string(env, &resolved)?;
                    define_own_prop(env, &mut resolved_obj, &uri.to_java_utf8(), &js_resolved)?;
                }
                let mut graph_obj = Object::new(env)?;
                graph_obj.set("imports", imports_obj)?;
                graph_obj.set("resolvedImports", resolved_obj)?;
                to_js_outcome(env, Ok(graph_obj))
            }
            Err(err) => to_js_outcome::<Null>(env, Err(err)),
        }
    }
}

#[napi]
pub fn product_format<'env>(
    env: &'env Env,
    source: Utf16String,
    grammar_version: Option<u32>,
) -> Result<Object<'env>> {
    let version = match grammar_version.unwrap_or(2) {
        1 => GrammarVersion::V1,
        2 => GrammarVersion::V2,
        other => {
            return to_js_outcome::<Null>(
                env,
                Err(NativeError::invalid_argument(format!(
                    "Invalid grammarVersion `{other}`. Expected 1 or 2."
                ))),
            );
        }
    };
    let text = match utf16_to_string(&source) {
        Ok(s) => s,
        Err(err) => return to_js_outcome::<Null>(env, Err(err)),
    };
    match format_str(&text, version) {
        Ok(formatted) => to_js_outcome(env, Ok(formatted)),
        Err(err) => to_js_outcome::<Null>(env, Err(NativeError::evaluation(err.to_string()))),
    }
}

#[napi]
pub fn product_path_to_file_uri<'env>(env: &'env Env, path: Utf16String) -> Result<Object<'env>> {
    let path_str = match utf16_to_string(&path) {
        Ok(s) => s,
        Err(err) => return to_js_outcome::<Null>(env, Err(err)),
    };
    let src = ModuleSource::path(path_str);
    to_js_outcome(env, Ok(src.get_uri().to_string()))
}

#[napi]
pub fn product_monotonic_clock_ms() -> BigInt {
    BigInt::from(process_clock_ms())
}

#[napi]
pub fn product_pkl_version() -> String {
    pkl::PKL_VERSION.to_string()
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

#[allow(dead_code)]
fn _keep_test_reporter_import(_: TestReporterKind) {}
