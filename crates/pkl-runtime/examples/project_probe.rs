//! Wave 1 native CLI probe for M4 Track C (`project resolve`, `project package`,
//! `download-package`, `test`, `settings`, and project-aware `eval`).
//!
//! Uses direct `SourceSession` + `pkl-binary` evaluation for `PklProject`, `pkl:settings`,
//! and `pkl:test` (zero synthetic JSON wrapper hacks), session-owned `PackageResolver`,
//! `ProjectDependenciesResolver` (MVS Algorithm 1), `ProjectPackager`, and `HostPackageCache`.

#[path = "support/file_glob_host.rs"]
mod file_glob_host;
#[path = "support/module_host.rs"]
mod module_host;
#[path = "support/package_cache.rs"]
mod package_cache;
#[path = "support/package_transport.rs"]
mod package_transport;
#[path = "support/resource_host.rs"]
mod resource_host;

use package_cache::{HostPackageCache, default_module_cache_dir};
use package_transport::PackageTransportClient;
use pkl_runtime::evaluation::frontend::{
    DiagnosticProfile, LosslessModuleSourceInput, ModuleEvalOptions,
    evaluate_module_output_with_platform_and_hosts,
};
use pkl_runtime::evaluation::session::{SourceSession, SourceSessionOptions};
use pkl_runtime::host::HostPoll;
use pkl_runtime::logging::{LogDelivery, TraceMode};
use pkl_runtime::modules::{ModuleCompletion, ModuleHostError, ModuleRequest, ModuleRequestKind};
use pkl_runtime::packages::cycles::{
    filter_same_scheme_cycles, find_import_cycles, format_circular_project_dependencies_error,
};
use pkl_runtime::packages::http::PackageTransport;
use pkl_runtime::packages::mvs::ProjectDependenciesResolver;
use pkl_runtime::packages::packager::{PackagerHost, ProjectPackager, parse_imports_and_reads};
use pkl_runtime::packages::project::{
    CliSettingsInput, EffectiveSettings, EvaluatorColor, PKL_PROJECT_DEPS_FILENAME,
    PKL_PROJECT_FILENAME, PklProject, PklSettings, Project, ProjectDeps, ResolvedProjectDep,
    find_project_dir_with, resolve_effective_settings,
};
use pkl_runtime::packages::resolver::{
    PackageHost, PackageResolver, file_uri_from_path, with_package_host,
};
use pkl_runtime::packages::test_runner::{
    RENDER_EXAMPLES_DOC_EXPR, RENDER_EXAMPLES_PKL_BINARY_EXPR, TestError, TestResults,
    TestSectionName, TestSectionResults, decode_rendered_examples_map, evaluate_examples_rendered,
    evaluate_facts_decoded_with_source, render_spec_report, render_spec_summary,
};
use pkl_runtime::packages::zip::ZipArchive;
use pkl_runtime::packages::{PackageUri, metadata::write_escaped_json_string_pub};
use pkl_runtime::pkl_binary_decode::decode_lossless;
use pkl_runtime::source::SourceText;
use pkl_runtime::value::{PklString, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Clone, Debug, Default)]
struct CommonFlags {
    working_dir: Option<PathBuf>,
    project_dir: Option<PathBuf>,
    no_project: bool,
    omit_project_settings: bool,
    settings_uri: Option<String>,
    cache_dir: Option<PathBuf>,
    no_cache: bool,
    root_dir: Option<PathBuf>,
    properties: Vec<(String, String)>,
    has_properties: bool,
    env: Vec<(String, String)>,
    has_env: bool,
    output_format: Option<String>,
    allowed_modules: Option<Vec<String>>,
    allowed_resources: Option<Vec<String>>,
    http_proxy: Option<String>,
    http_no_proxy: Option<Vec<String>>,
    http_rewrites: Option<BTreeMap<String, String>>,
    ca_certificates: Option<PathBuf>,
    test_port: Option<u16>,
    test_mode: bool,
    overwrite: bool,
    no_transitive: bool,
    skip_publish_check: bool,
    output_path: Option<String>,
}

fn parse_key_value(raw: &str) -> Result<(String, String), String> {
    match raw.split_once('=') {
        Some((k, v)) => Ok((k.to_string(), v.to_string())),
        None => Ok((raw.to_string(), "true".to_string())),
    }
}

fn parse_args(args: &[String]) -> Result<(CommonFlags, Vec<String>), String> {
    let mut flags = CommonFlags::default();
    let mut positionals = Vec::new();
    let mut i = 0;
    while i < args.len() {
        let arg = &args[i];
        match arg.as_str() {
            "-w" | "--working-dir" => {
                i += 1;
                let v = args.get(i).ok_or("missing value for --working-dir")?;
                flags.working_dir = Some(PathBuf::from(v));
            }
            "--project-dir" => {
                i += 1;
                let v = args.get(i).ok_or("missing value for --project-dir")?;
                flags.project_dir = Some(PathBuf::from(v));
            }
            "--no-project" => flags.no_project = true,
            "--omit-project-settings" => flags.omit_project_settings = true,
            "--settings" => {
                i += 1;
                let v = args.get(i).ok_or("missing value for --settings")?;
                flags.settings_uri = Some(v.clone());
            }
            "--cache-dir" => {
                i += 1;
                let v = args.get(i).ok_or("missing value for --cache-dir")?;
                flags.cache_dir = Some(PathBuf::from(v));
            }
            "--no-cache" => flags.no_cache = true,
            "--root-dir" => {
                i += 1;
                let v = args.get(i).ok_or("missing value for --root-dir")?;
                flags.root_dir = Some(PathBuf::from(v));
            }
            "-p" | "--property" => {
                i += 1;
                let v = args.get(i).ok_or("missing value for --property")?;
                flags.has_properties = true;
                flags.properties.push(parse_key_value(v)?);
            }
            "-e" | "--env-var" => {
                i += 1;
                let v = args.get(i).ok_or("missing value for --env-var")?;
                flags.has_env = true;
                flags.env.push(parse_key_value(v)?);
            }
            "-f" | "--format" => {
                i += 1;
                let v = args.get(i).ok_or("missing value for --format")?;
                flags.output_format = Some(v.clone());
            }
            "--allowed-modules" => {
                i += 1;
                let v = args.get(i).ok_or("missing value for --allowed-modules")?;
                flags.allowed_modules = Some(v.split(',').map(str::to_string).collect());
            }
            "--allowed-resources" => {
                i += 1;
                let v = args.get(i).ok_or("missing value for --allowed-resources")?;
                flags.allowed_resources = Some(v.split(',').map(str::to_string).collect());
            }
            "--http-proxy" => {
                i += 1;
                let v = args.get(i).ok_or("missing value for --http-proxy")?;
                flags.http_proxy = Some(v.clone());
            }
            "--http-no-proxy" => {
                i += 1;
                let v = args.get(i).ok_or("missing value for --http-no-proxy")?;
                flags.http_no_proxy = Some(v.split(',').map(str::to_string).collect());
            }
            "--http-rewrite" => {
                i += 1;
                let v = args.get(i).ok_or("missing value for --http-rewrite")?;
                let (from, to) = v
                    .split_once('=')
                    .ok_or("invalid --http-rewrite (expected from=to)")?;
                flags
                    .http_rewrites
                    .get_or_insert_with(BTreeMap::new)
                    .insert(from.to_string(), to.to_string());
            }
            "--ca-certificates" => {
                i += 1;
                let v = args.get(i).ok_or("missing value for --ca-certificates")?;
                flags.ca_certificates = Some(PathBuf::from(v));
            }
            "--test-port" => {
                i += 1;
                let v = args.get(i).ok_or("missing value for --test-port")?;
                flags.test_port = Some(
                    v.parse::<u16>()
                        .map_err(|e| format!("invalid --test-port: {e}"))?,
                );
            }
            "--test-mode" => flags.test_mode = true,
            "--overwrite" => flags.overwrite = true,
            "--no-transitive" => flags.no_transitive = true,
            "--skip-publish-check" => flags.skip_publish_check = true,
            "--output-path" => {
                i += 1;
                let v = args.get(i).ok_or("missing value for --output-path")?;
                flags.output_path = Some(v.clone());
            }
            other if other.starts_with('-') => {
                return Err(format!("unknown flag: {other}"));
            }
            pos => positionals.push(pos.to_string()),
        }
        i += 1;
    }
    Ok((flags, positionals))
}

fn effective_working_dir(flags: &CommonFlags) -> Result<PathBuf, String> {
    let cwd = std::env::current_dir().map_err(|e| format!("failed to read cwd: {e}"))?;
    Ok(match &flags.working_dir {
        Some(w) if w.is_absolute() => w.clone(),
        Some(w) => cwd.join(w),
        None => cwd,
    })
}

fn pkl_home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(|h| PathBuf::from(h).join(".pkl"))
}

fn eval_expressions_on_file(
    file_path: &Path,
    expressions: &[String],
    env_vars: &[(String, String)],
    properties: &[(String, String)],
) -> Result<Vec<Value>, String> {
    let abs = std::fs::canonicalize(file_path).unwrap_or_else(|_| file_path.to_path_buf());
    let text = std::fs::read_to_string(&abs)
        .map_err(|e| format!("Cannot read `{}`: {e}", abs.display()))?;
    let uri = file_uri_from_path(&abs).map_err(|e| format!("{e:?}"))?;
    let module_name = abs
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("module")
        .to_string();

    let mut session = SourceSession::compile_expressions_text(
        SourceText::from(text),
        PklString::from(uri.as_str()),
        PklString::from(module_name.as_str()),
        expressions,
        SourceSessionOptions {
            evaluation: pkl_runtime::evaluation::EvaluationOptions {
                power_assertions: true,
            },
            trace_mode: TraceMode::Compact,
            log_delivery: LogDelivery::Host,
            frame_transformer: pkl_runtime::logging::FrameTransformer::Default,
            platform_data: true,
        },
    )
    .map_err(|e| e.to_java_utf8())?;

    let mut results = Vec::with_capacity(expressions.len());
    let poll_budget = 5_000_000usize;
    for idx in 0..expressions.len() {
        session
            .start_path(idx, 0, None)
            .map_err(|e| session.format_error(e))?;
        let mut finished = None;
        for _ in 0..poll_budget {
            match session.poll(0, 1024) {
                HostPoll::Ready(res) => {
                    let val = res.map_err(|e| session.format_error(e))?;
                    finished = Some(val);
                    break;
                }
                HostPoll::Yielded => {}
                HostPoll::Module { token, request } => {
                    let completion = module_host::complete(&request);
                    session
                        .complete_module(&token, completion, 0)
                        .map_err(|e| session.format_error(e))?;
                }
                HostPoll::Read { token, .. } => {
                    let req = session
                        .read_request(&token)
                        .map_err(|e| session.format_error(e))?;
                    if let Some(result) = resource_host::complete(&req) {
                        session
                            .complete_read(&token, result, 0)
                            .map_err(|e| session.format_error(e))?;
                    } else {
                        session
                            .resume_default_read(&token, properties, env_vars, 0)
                            .map_err(|e| session.format_error(e))?;
                    }
                }
                HostPoll::Glob { token, request } => {
                    let completion = file_glob_host::complete(&request).unwrap_or_else(|| {
                        pkl_runtime::glob::configured_or_unsupported_completion(
                            &request, properties, env_vars,
                        )
                    });
                    session
                        .complete_glob(&token, completion, 0)
                        .map_err(|e| session.format_error(e))?;
                }
                HostPoll::Log { token, .. } => {
                    session
                        .acknowledge_log(&token, Ok(()), 0)
                        .map_err(|e| session.format_error(e))?;
                }
                HostPoll::TransformFrame { token, frame } => {
                    session
                        .complete_frame_transform(&token, Ok((*frame).clone()), 0)
                        .map_err(|e| session.format_error(e))?;
                }
            }
        }
        results.push(finished.ok_or("Evaluation budget exhausted")?);
    }
    Ok(results)
}

/// Detects local-project import cycles before evaluating `PklProject` (`Project.java:149-236`).
fn check_project_import_cycles(project_file: &Path) -> Result<(), String> {
    let abs = std::fs::canonicalize(project_file).unwrap_or_else(|_| project_file.to_path_buf());
    let root_uri = file_uri_from_path(&abs).map_err(|e| format!("{e:?}"))?;
    let mut graph: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut queue: Vec<PathBuf> = vec![abs.clone()];
    let mut visited: BTreeSet<PathBuf> = BTreeSet::new();

    while let Some(path) = queue.pop() {
        let canon = std::fs::canonicalize(&path).unwrap_or_else(|_| path.clone());
        if !visited.insert(canon.clone()) {
            continue;
        }
        let Ok(uri) = file_uri_from_path(&canon) else {
            continue;
        };
        let Ok(src) = std::fs::read_to_string(&canon) else {
            continue;
        };
        let Ok(entries) = parse_imports_and_reads(&SourceText::from(src)) else {
            continue;
        };
        let parent = canon.parent().unwrap_or(&canon);
        let mut edges = Vec::new();
        for entry in entries {
            if !entry.is_module || entry.is_glob {
                continue;
            }
            let s = entry.string_value.as_str().unwrap_or("");
            if s.starts_with("pkl:") || s.starts_with("package:") || s.starts_with('@') {
                continue;
            }
            let target = if let Some(rest) = s.strip_prefix("file://") {
                PathBuf::from(pkl_runtime::packages::zip::percent_decode(rest))
            } else if !s.contains("://") {
                parent.join(s)
            } else {
                continue;
            };
            if target.is_file() {
                let target_canon = std::fs::canonicalize(&target).unwrap_or(target);
                if let Ok(target_uri) = file_uri_from_path(&target_canon) {
                    if !edges.contains(&target_uri) {
                        edges.push(target_uri);
                    }
                    queue.push(target_canon);
                }
            }
        }
        graph.insert(uri, edges);
    }

    let cycles = filter_same_scheme_cycles(&root_uri, find_import_cycles(&graph));
    if let Some(msg) = format_circular_project_dependencies_error(&cycles) {
        return Err(msg);
    }
    Ok(())
}

fn value_is_true(value: &Value) -> bool {
    matches!(value, Value::Bool(true))
        || SourceSession::result_string(value.clone()).is_ok_and(|s| s == "true")
}

fn decode_base64(input: &str) -> Result<Vec<u8>, String> {
    let mut out = Vec::with_capacity(input.len() * 3 / 4);
    let mut buf: u32 = 0;
    let mut bits: u32 = 0;
    for byte in input.bytes() {
        let val = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            b'=' | b'\r' | b'\n' | b' ' | b'\t' => continue,
            other => return Err(format!("invalid base64 byte {other}")),
        };
        buf = (buf << 6) | u32::from(val);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push(((buf >> bits) & 0xff) as u8);
        }
    }
    Ok(out)
}

fn extract_binary_value(value: Value) -> Result<Vec<u8>, String> {
    if let Ok(bytes) = SourceSession::result_bytes(value.clone()) {
        return Ok(bytes.to_vec());
    }
    let b64 = SourceSession::result_string(value)
        .map_err(|e| format!("Failed to extract binary/base64 value: {e:?}"))?;
    decode_base64(b64.as_str().unwrap_or(""))
}

/// Loads a `PklProject` directly through `SourceSession` + `pkl-binary` (no JSON wrapper hack).
fn load_project_file(
    project_file: &Path,
    env_vars: &[(String, String)],
) -> Result<Project, String> {
    check_project_import_cycles(project_file)?;
    let abs = std::fs::canonicalize(project_file).unwrap_or_else(|_| project_file.to_path_buf());
    let uri = file_uri_from_path(&abs).map_err(|e| format!("{e:?}"))?;

    let check_exprs = vec![
        "output.value != null && let (ref = import(\"pkl:reflect\")) ref.Class(output.value.getClass()).isSubclassOf(ref.Class(import(\"pkl:Project\").getClass()))".to_string(),
        "if (output.value is Module) (import(\"pkl:reflect\")).Module(output.value).name else output.value.getClass().simpleName".to_string(),
    ];
    let pkl_bin_props = vec![("pkl.outputFormat".to_string(), "pkl-binary".to_string())];
    let check_vals = eval_expressions_on_file(&abs, &check_exprs, env_vars, &pkl_bin_props)?;
    let is_project = value_is_true(&check_vals[0]);
    if !is_project {
        let got_type = SourceSession::result_string(check_vals[1].clone())
            .map(|s| s.to_java_utf8())
            .unwrap_or_else(|_| "Any".to_string());
        return Err(format!(
            "Expected `output.value` of module `{uri}` to be of type `pkl.Project`, but got type `{got_type}`."
        ));
    }

    let bin_exprs = vec!["output.bytes".to_string()];
    let bin_vals = eval_expressions_on_file(&abs, &bin_exprs, env_vars, &pkl_bin_props)?;
    let bytes = extract_binary_value(bin_vals[0].clone())?;
    let decoded = decode_lossless(&bytes)
        .map_err(|e| format!("Failed to decode PklProject pkl-binary: {e:?}"))?;
    Project::from_decoded_value(&decoded).map_err(|e| format!("{e:?}"))
}

/// Loads `PklSettings` from `--settings <uri>` or `~/.pkl/settings.pkl` (`PklSettings.java:48-88`).
fn load_user_settings(
    settings_uri: Option<&str>,
    working_dir: &Path,
    env_vars: &[(String, String)],
) -> Result<PklSettings, String> {
    let target_file = match settings_uri {
        Some("pkl:settings") => return Ok(PklSettings::default()),
        Some(uri) => {
            if let Some(rest) = uri.strip_prefix("file://") {
                PathBuf::from(pkl_runtime::packages::zip::percent_decode(rest))
            } else if uri.contains("://") {
                return Err(format!("Unsupported settings URI `{uri}`."));
            } else {
                let p = PathBuf::from(uri);
                if p.is_absolute() {
                    p
                } else {
                    working_dir.join(p)
                }
            }
        }
        None => {
            let Some(home) = pkl_home_dir() else {
                return Ok(PklSettings::default());
            };
            let default_file = home.join("settings.pkl");
            if !default_file.is_file() {
                return Ok(PklSettings::default());
            }
            default_file
        }
    };

    let abs = std::fs::canonicalize(&target_file).unwrap_or_else(|_| target_file.clone());
    let uri = file_uri_from_path(&abs).map_err(|e| format!("{e:?}"))?;
    let check_exprs = vec![
        "output.value != null && let (ref = import(\"pkl:reflect\")) ref.Class(output.value.getClass()).isSubclassOf(ref.Class(import(\"pkl:settings\").getClass()))".to_string(),
        "if (output.value is Module) (import(\"pkl:reflect\")).Module(output.value).name else output.value.getClass().simpleName".to_string(),
    ];
    let pkl_bin_props = vec![("pkl.outputFormat".to_string(), "pkl-binary".to_string())];
    let check_vals = eval_expressions_on_file(&abs, &check_exprs, env_vars, &pkl_bin_props)?;
    if !value_is_true(&check_vals[0]) {
        let got_type = SourceSession::result_string(check_vals[1].clone())
            .map(|s| s.to_java_utf8())
            .unwrap_or_else(|_| "Any".to_string());
        return Err(format!(
            "Expected `output.value` of module `{uri}` to be of type `pkl.settings`, but got type `{got_type}`."
        ));
    }

    let bin_exprs = vec!["output.bytes".to_string()];
    let bin_vals = eval_expressions_on_file(&abs, &bin_exprs, env_vars, &pkl_bin_props)?;
    let bytes = extract_binary_value(bin_vals[0].clone())?;
    let decoded = decode_lossless(&bytes)
        .map_err(|e| format!("Failed to decode settings pkl-binary: {e:?}"))?;
    PklSettings::from_decoded_value(&decoded, &uri).map_err(|e| format!("{e:?}"))
}

fn build_effective_settings(
    flags: &CommonFlags,
    project: Option<&Project>,
    user_settings: &PklSettings,
    host_env: &[(String, String)],
) -> EffectiveSettings {
    let cli = CliSettingsInput {
        omit_project_settings: flags.omit_project_settings,
        external_properties: flags.has_properties.then(|| flags.properties.clone()),
        output_format: flags.output_format.clone(),
        env: flags.has_env.then(|| flags.env.clone()),
        allowed_modules: flags.allowed_modules.clone(),
        allowed_resources: flags.allowed_resources.clone(),
        no_cache: flags.no_cache,
        module_cache_dir: flags.cache_dir.clone(),
        root_dir: flags.root_dir.clone(),
        module_path: None,
        color: None,
        trace_mode: None,
        timeout_nanos: None,
        http_proxy_address: flags.http_proxy.clone(),
        http_no_proxy: flags.http_no_proxy.clone(),
        http_rewrites: flags.http_rewrites.clone(),
        external_module_readers: None,
        external_resource_readers: None,
        apply_project_timeout: false,
    };
    resolve_effective_settings(
        &cli,
        project.and_then(|p| p.resolved_evaluator_settings.as_ref()),
        user_settings,
        host_env,
        default_module_cache_dir(),
    )
}

fn build_package_resolver(
    flags: &CommonFlags,
    effective_cache_dir: Option<PathBuf>,
) -> PackageResolver {
    let transport: Arc<dyn PackageTransport> = Arc::new(PackageTransportClient::new(
        flags.ca_certificates.clone(),
        flags.test_port,
    ));
    if let Some(dir) = effective_cache_dir {
        let store = Arc::new(HostPackageCache::new(Some(dir), false));
        PackageResolver::disk_cached(transport, store, flags.test_mode)
    } else {
        PackageResolver::in_memory(transport, flags.test_mode)
    }
}

struct FsPackagerHost {
    transport: PackageTransportClient,
}

impl PackagerHost for FsPackagerHost {
    fn list_project_files(
        &self,
        project_dir: &Path,
    ) -> Result<Vec<(String, Vec<u8>)>, ModuleHostError> {
        let mut files = Vec::new();
        walk_regular_files(project_dir, project_dir, &mut files)?;
        files.sort_by(|(a, _), (b, _)| a.cmp(b));
        Ok(files)
    }

    fn read_project_deps_json(
        &self,
        project_dir: &Path,
    ) -> Result<Option<String>, ModuleHostError> {
        let path = project_dir.join(PKL_PROJECT_DEPS_FILENAME);
        if !path.is_file() {
            return Ok(None);
        }
        let text = std::fs::read_to_string(&path)
            .map_err(|e| ModuleHostError::Io(format!("Failed to read deps file: {e}").into()))?;
        Ok(Some(text))
    }

    fn write_artifact(&self, path: &Path, bytes: &[u8]) -> Result<(), ModuleHostError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                ModuleHostError::Io(format!("Failed to create output dir: {e}").into())
            })?;
        }
        std::fs::write(path, bytes)
            .map_err(|e| ModuleHostError::Io(format!("Failed to write artifact: {e}").into()))
    }

    fn transport(&self) -> Option<&dyn PackageTransport> {
        Some(&self.transport)
    }
}

fn walk_regular_files(
    root: &Path,
    dir: &Path,
    out: &mut Vec<(String, Vec<u8>)>,
) -> Result<(), ModuleHostError> {
    let entries = std::fs::read_dir(dir)
        .map_err(|e| ModuleHostError::Io(format!("Failed to read dir: {e}").into()))?;
    for entry in entries {
        let entry = entry
            .map_err(|e| ModuleHostError::Io(format!("Failed to read dir entry: {e}").into()))?;
        let path = entry.path();
        let meta = std::fs::symlink_metadata(&path)
            .map_err(|e| ModuleHostError::Io(format!("Failed to stat path: {e}").into()))?;
        if meta.is_dir() {
            walk_regular_files(root, &path, out)?;
        } else if meta.is_file() {
            let rel = path
                .strip_prefix(root)
                .map_err(|e| ModuleHostError::Io(format!("Failed to strip prefix: {e}").into()))?;
            let rel_str = rel.to_string_lossy().replace('\\', "/");
            let bytes = std::fs::read(&path)
                .map_err(|e| ModuleHostError::Io(format!("Failed to read file: {e}").into()))?;
            out.push((rel_str, bytes));
        }
    }
    Ok(())
}

/// Probe host adapter backing `with_package_host` for `eval` and `test` subcommands.
struct ProbePackageHost {
    transport: PackageTransportClient,
    cache: HostPackageCache,
    project: Option<PklProject>,
    test_mode: bool,
    allowed_modules: Option<Vec<String>>,
    allowed_resources: Option<Vec<String>>,
    trace_mode: Option<TraceMode>,
}

impl PackageHost for ProbePackageHost {
    fn fetch_metadata(
        &self,
        package: &PackageUri,
        uri: &PklString,
    ) -> Result<Vec<u8>, ModuleHostError> {
        self.transport.fetch_metadata(package, uri)
    }

    fn fetch_zip(&self, package: &PackageUri, url: &PklString) -> Result<Vec<u8>, ModuleHostError> {
        self.transport.fetch_zip(package, url)
    }

    fn read_disk_archive(&self, package: &PackageUri) -> Option<ZipArchive> {
        self.cache.try_read_disk_archive(package)
    }

    fn read_disk_archive_checked(
        &self,
        package: &PackageUri,
    ) -> Result<Option<ZipArchive>, ModuleHostError> {
        self.cache.read_zip_archive(package)
    }

    fn read_disk_metadata(&self, package: &PackageUri) -> Result<Option<Vec<u8>>, ModuleHostError> {
        self.cache.read_metadata_bytes(package)
    }

    fn delete_disk_metadata(&self, package: &PackageUri) -> Result<(), ModuleHostError> {
        self.cache.delete_metadata(package)
    }

    fn write_disk_archive(
        &self,
        package: &PackageUri,
        metadata_bytes: &[u8],
        zip_bytes: &[u8],
    ) -> Result<(), ModuleHostError> {
        self.cache
            .try_write_disk_archive(package, metadata_bytes, zip_bytes)
    }

    fn find_project_dir(&self, _start: &Path) -> Option<PathBuf> {
        self.project.as_ref().map(|p| p.project_dir.clone())
    }

    fn load_project(&self, project_dir: &Path) -> Result<PklProject, ModuleHostError> {
        if let Some(ref p) = self.project
            && p.project_dir == project_dir
        {
            return Ok(p.clone());
        }
        Ok(PklProject {
            project_dir: project_dir.to_path_buf(),
            ..Default::default()
        })
    }

    fn load_project_dependencies(&self, project: &mut PklProject) -> Result<(), ModuleHostError> {
        if project.deps_loaded {
            return Ok(());
        }
        let deps_file = project.project_dir.join(PKL_PROJECT_DEPS_FILENAME);
        let deps_uri = file_uri_from_path(&deps_file)?;
        let text = std::fs::read_to_string(&deps_file).map_err(|_| ModuleHostError::EvalError {
            message: format!(
                "Encountered an error when attempting to load `PklProject.deps.json` at `{deps_uri}`.\n\nTry running `pkl project resolve` within the project directory to create a new set of dependencies."
            )
            .into(),
            hint: None,
        })?;
        let parsed = ProjectDeps::parse(&text, &deps_uri)?;
        let mut by_canonical: BTreeMap<String, ResolvedProjectDep> = BTreeMap::new();
        for (canon, entry) in parsed.entries {
            if let Some(k) = canon.as_str() {
                by_canonical.insert(k.to_string(), entry.to_legacy_resolved());
            }
        }
        // Populate both canonical keys and declared dependency names for lookup
        let mut combined = by_canonical.clone();
        for (decl_name, decl) in &project.declared_dependencies {
            let target_uri = decl
                .local_package_uri
                .as_deref()
                .unwrap_or(decl.uri.as_str());
            if let Some(matched) = pkl_runtime::packages::project::find_matching_resolved(
                target_uri,
                decl_name,
                &by_canonical,
            ) {
                combined.insert(decl_name.clone(), matched);
            }
        }
        project.dependencies = combined;
        project.deps_loaded = true;
        Ok(())
    }

    fn read_file(&self, path: &Path) -> Result<Vec<u8>, ModuleHostError> {
        std::fs::read(path).map_err(|e| {
            ModuleHostError::Io(format!("Failed to read {}: {e}", path.display()).into())
        })
    }

    fn list_dir(
        &self,
        path: &Path,
    ) -> Result<Vec<pkl_runtime::glob::PathElement>, ModuleHostError> {
        let mut elems = Vec::new();
        let entries = std::fs::read_dir(path).map_err(|e| {
            ModuleHostError::Io(format!("Failed to list {}: {e}", path.display()).into())
        })?;
        for entry in entries.flatten() {
            let is_directory = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
            elems.push(pkl_runtime::glob::PathElement {
                name: entry.file_name().to_string_lossy().into_owned().into(),
                is_directory,
            });
        }
        Ok(elems)
    }

    fn is_test_mode(&self) -> bool {
        self.test_mode
    }

    fn has_project(&self) -> bool {
        self.project.is_some()
    }

    fn allowed_resources(&self) -> Option<Vec<String>> {
        self.allowed_resources.clone()
    }

    fn allowed_modules(&self) -> Option<Vec<String>> {
        self.allowed_modules.clone()
    }

    fn trace_mode(&self) -> Option<TraceMode> {
        self.trace_mode
    }
}

fn run_single_test_module(
    test_path: &Path,
    overwrite: bool,
    env_vars: &[(String, String)],
    properties: &[(String, String)],
) -> Result<TestResults, String> {
    let abs = std::fs::canonicalize(test_path).unwrap_or_else(|_| test_path.to_path_buf());
    let uri = file_uri_from_path(&abs).map_err(|e| format!("{e:?}"))?;
    let module_name = abs
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("test")
        .to_string();

    let exprs = vec![
        "let (ref = import(\"pkl:reflect\")) ref.Class(module.getClass()).isSubclassOf(ref.Class(import(\"pkl:test\").getClass()))".to_string(),
        "(import(\"pkl:reflect\")).Module(module).name".to_string(),
        "output.renderer.renderValue(facts).base64".to_string(),
        RENDER_EXAMPLES_PKL_BINARY_EXPR.to_string(),
        RENDER_EXAMPLES_DOC_EXPR.to_string(),
    ];
    let mut test_props: Vec<(String, String)> = properties
        .iter()
        .filter(|(k, _)| k != "pkl.outputFormat")
        .cloned()
        .collect();
    test_props.push(("pkl.outputFormat".to_string(), "pkl-binary".to_string()));
    let vals = match eval_expressions_on_file(&abs, &exprs, env_vars, &test_props) {
        Ok(v) => v,
        Err(err) => {
            return Ok(TestResults {
                module_name,
                display_uri: uri,
                facts: TestSectionResults::empty(TestSectionName::Facts),
                examples: TestSectionResults::empty(TestSectionName::Examples),
                logs: String::new(),
                error: Some(TestError {
                    message: err.clone(),
                    rendered: err,
                }),
                generated_expected_pcf: None,
                generated_actual_pcf: None,
            });
        }
    };

    let module_name = SourceSession::result_string(vals[1].clone())
        .map(|s| s.to_java_utf8())
        .unwrap_or(module_name);

    if !value_is_true(&vals[0]) {
        let msg = format!("Module `{uri}` does not amend `pkl:test`.");
        return Ok(TestResults {
            module_name,
            display_uri: uri,
            facts: TestSectionResults::empty(TestSectionName::Facts),
            examples: TestSectionResults::empty(TestSectionName::Examples),
            logs: String::new(),
            error: Some(TestError {
                message: msg.clone(),
                rendered: msg,
            }),
            generated_expected_pcf: None,
            generated_actual_pcf: None,
        });
    }

    let facts_bytes = extract_binary_value(vals[2].clone())?;
    let facts_decoded = decode_lossless(&facts_bytes)
        .map_err(|e| format!("Failed to decode facts pkl-binary: {e:?}"))?;
    let test_source = std::fs::read_to_string(&abs).ok();
    let facts =
        evaluate_facts_decoded_with_source(Some(&facts_decoded), &uri, test_source.as_deref());

    let examples_bytes = extract_binary_value(vals[3].clone())?;
    let examples_decoded = decode_lossless(&examples_bytes)
        .map_err(|e| format!("Failed to decode examples pkl-binary: {e:?}"))?;
    let actual_examples = decode_rendered_examples_map(Some(&examples_decoded));
    let full_doc = SourceSession::result_string(vals[4].clone())
        .map(|s| s.to_java_utf8())
        .unwrap_or_default();

    let file_name = abs
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("test.pkl");
    let expected_file = abs.with_file_name(format!("{file_name}-expected.pcf"));
    let actual_file = abs.with_file_name(format!("{file_name}-actual.pcf"));
    let _ = std::fs::remove_file(&actual_file);
    if overwrite {
        let _ = std::fs::remove_file(&expected_file);
    }

    let expected_uri = file_uri_from_path(&expected_file).unwrap_or_default();
    let actual_uri = file_uri_from_path(&actual_file).unwrap_or_default();

    let expected_groups = if !actual_examples.is_empty() && expected_file.is_file() {
        let exp_exprs = vec![RENDER_EXAMPLES_PKL_BINARY_EXPR.to_string()];
        let exp_vals = eval_expressions_on_file(&expected_file, &exp_exprs, env_vars, &test_props)?;
        let exp_bytes = extract_binary_value(exp_vals[0].clone())?;
        let exp_decoded = decode_lossless(&exp_bytes)
            .map_err(|e| format!("Failed to decode expected examples: {e:?}"))?;
        Some(decode_rendered_examples_map(Some(&exp_decoded)))
    } else {
        None
    };

    let (examples, gen_expected, gen_actual) = evaluate_examples_rendered(
        &actual_examples,
        expected_groups.as_deref(),
        &uri,
        &expected_uri,
        &actual_uri,
        (!full_doc.is_empty()).then_some(full_doc),
    );

    if let Some(ref content) = gen_expected {
        std::fs::write(&expected_file, content)
            .map_err(|e| format!("Failed to write {}: {e}", expected_file.display()))?;
    }
    if let Some(ref content) = gen_actual {
        std::fs::write(&actual_file, content)
            .map_err(|e| format!("Failed to write {}: {e}", actual_file.display()))?;
    }

    Ok(TestResults {
        module_name,
        display_uri: uri,
        facts,
        examples,
        logs: String::new(),
        error: None,
        generated_expected_pcf: gen_expected,
        generated_actual_pcf: gen_actual,
    })
}

fn resolve_project_dirs(
    flags: &CommonFlags,
    positionals: &[String],
    working_dir: &Path,
) -> Result<Vec<PathBuf>, String> {
    if !positionals.is_empty() {
        return Ok(positionals
            .iter()
            .map(|p| {
                let pb = PathBuf::from(p);
                if pb.is_absolute() {
                    pb
                } else {
                    working_dir.join(pb)
                }
            })
            .collect());
    }
    if let Some(ref dir) = flags.project_dir {
        let pb = if dir.is_absolute() {
            dir.clone()
        } else {
            working_dir.join(dir)
        };
        return Ok(vec![pb]);
    }
    if let Some(found) =
        find_project_dir_with(working_dir, flags.root_dir.as_deref(), |p| p.is_file())
    {
        return Ok(vec![found]);
    }
    Err(format!(
        "Could not find `{PKL_PROJECT_FILENAME}` in `{}` or any parent directory.",
        working_dir.display()
    ))
}

fn cmd_project_resolve(args: &[String]) -> Result<i32, String> {
    let (flags, positionals) = parse_args(args)?;
    let working_dir = effective_working_dir(&flags)?;
    let host_env: Vec<(String, String)> = std::env::vars().collect();
    let user_settings = load_user_settings(flags.settings_uri.as_deref(), &working_dir, &host_env)?;
    let dirs = resolve_project_dirs(&flags, &positionals, &working_dir)?;

    for dir in dirs {
        let project_file = dir.join(PKL_PROJECT_FILENAME);
        let project = load_project_file(&project_file, &host_env)?;
        let effective = build_effective_settings(&flags, Some(&project), &user_settings, &host_env);
        let resolver = build_package_resolver(&flags, effective.module_cache_dir.clone());
        let resolved = ProjectDependenciesResolver::new(&project, &resolver)
            .resolve()
            .map_err(|e| format!("{e:?}"))?;
        for warn in &resolved.warnings {
            eprintln!("{warn}");
        }
        let deps_file = project.project_dir.join(PKL_PROJECT_DEPS_FILENAME);
        let json = resolved.deps.write_json();
        std::fs::write(&deps_file, json.as_bytes())
            .map_err(|e| format!("Failed to write {}: {e}", deps_file.display()))?;
        println!("{}", deps_file.display());
        resolver.close();
    }
    Ok(0)
}

fn cmd_project_package(args: &[String]) -> Result<i32, String> {
    let (flags, positionals) = parse_args(args)?;
    let working_dir = effective_working_dir(&flags)?;
    let host_env: Vec<(String, String)> = std::env::vars().collect();
    let user_settings = load_user_settings(flags.settings_uri.as_deref(), &working_dir, &host_env)?;
    let dirs = resolve_project_dirs(&flags, &positionals, &working_dir)?;
    let output_pattern = flags
        .output_path
        .clone()
        .unwrap_or_else(|| ".out/%{name}@%{version}".to_string());

    let mut projects = Vec::with_capacity(dirs.len());
    for dir in dirs {
        let project_file = dir.join(PKL_PROJECT_FILENAME);
        let project = load_project_file(&project_file, &host_env)?;
        let Some(ref pkg) = project.package else {
            return Err(format!(
                "No package was declared in project `{}`.\n\nAdd a `package` section to the PklProject file.",
                project.project_file_uri
            ));
        };
        if !pkg.api_tests.is_empty() {
            let effective =
                build_effective_settings(&flags, Some(&project), &user_settings, &host_env);
            let env_pairs: Vec<(String, String)> = effective.env.into_iter().collect();
            let prop_pairs: Vec<(String, String)> =
                effective.external_properties.into_iter().collect();
            let explicit_allowed_modules = flags.allowed_modules.clone().or_else(|| {
                if flags.omit_project_settings {
                    None
                } else {
                    project
                        .resolved_evaluator_settings
                        .as_ref()
                        .and_then(|s| s.allowed_modules.clone())
                }
            });
            let explicit_allowed_resources = flags.allowed_resources.clone().or_else(|| {
                if flags.omit_project_settings {
                    None
                } else {
                    project
                        .resolved_evaluator_settings
                        .as_ref()
                        .and_then(|s| s.allowed_resources.clone())
                }
            });
            let host = Arc::new(ProbePackageHost {
                transport: PackageTransportClient::new(
                    flags.ca_certificates.clone(),
                    flags.test_port,
                ),
                cache: HostPackageCache::new(
                    effective.module_cache_dir.clone(),
                    effective.module_cache_dir.is_none(),
                ),
                project: Some(project.to_legacy_pkl_project()),
                test_mode: flags.test_mode,
                allowed_modules: explicit_allowed_modules,
                allowed_resources: explicit_allowed_resources,
                trace_mode: None,
            });
            let mut all_results = Vec::new();
            let mut failed = false;
            with_package_host(host, || {
                for api_test in &pkg.api_tests {
                    let test_file = project.project_dir.join(api_test);
                    let res = run_single_test_module(
                        &test_file,
                        flags.overwrite,
                        &env_pairs,
                        &prop_pairs,
                    )?;
                    print!("{}", render_spec_report(&res));
                    if res.failed() {
                        failed = true;
                    }
                    all_results.push(res);
                }
                Ok::<(), String>(())
            })?;
            println!();
            print!("{}", render_spec_summary(&all_results));
            if failed {
                return Err(format!(
                    "Failed to create package `{}`, because its API tests are failing.",
                    pkg.uri
                ));
            }
        }
        projects.push(project);
    }

    let packager_host = FsPackagerHost {
        transport: PackageTransportClient::new(flags.ca_certificates.clone(), flags.test_port),
    };
    let (_, stdout_lines) = ProjectPackager::new(
        &projects,
        working_dir,
        output_pattern,
        flags.skip_publish_check,
        &packager_host,
    )
    .create_packages()
    .map_err(|e| match e {
        ModuleHostError::EvalError { message, hint } => match hint {
            Some(h) => format!("{}\n\n{}", message.to_java_utf8(), h.to_java_utf8()),
            None => message.to_java_utf8(),
        },
        ModuleHostError::SecurityManagerException {
            message: Some(message),
        } => message.to_java_utf8(),
        other => format!("{other:?}"),
    })?;

    for line in stdout_lines {
        println!("{line}");
    }
    Ok(0)
}

fn cmd_download_package(args: &[String]) -> Result<i32, String> {
    let (flags, positionals) = parse_args(args)?;
    if positionals.is_empty() {
        return Err("download-package requires at least one package URI".to_string());
    }
    let working_dir = effective_working_dir(&flags)?;
    let host_env: Vec<(String, String)> = std::env::vars().collect();
    let user_settings = load_user_settings(flags.settings_uri.as_deref(), &working_dir, &host_env)?;
    let effective = build_effective_settings(&flags, None, &user_settings, &host_env);
    let resolver = build_package_resolver(&flags, effective.module_cache_dir.clone());

    for raw in &positionals {
        let pkg = PackageUri::parse(raw.as_str()).map_err(|e| e.reason().to_java_utf8())?;
        let downloaded = resolver
            .download_package(&pkg, pkg.sha256.as_ref(), flags.no_transitive)
            .map_err(|e| format!("{e:?}"))?;
        for item in downloaded {
            println!("{item}");
        }
    }
    resolver.close();
    Ok(0)
}

fn cmd_test(args: &[String]) -> Result<i32, String> {
    let (flags, positionals) = parse_args(args)?;
    let working_dir = effective_working_dir(&flags)?;
    let host_env: Vec<(String, String)> = std::env::vars().collect();
    let user_settings = load_user_settings(flags.settings_uri.as_deref(), &working_dir, &host_env)?;

    let project = if flags.no_project {
        None
    } else if let Some(ref dir) = flags.project_dir {
        let pb = if dir.is_absolute() {
            dir.clone()
        } else {
            working_dir.join(dir)
        };
        Some(load_project_file(
            &pb.join(PKL_PROJECT_FILENAME),
            &host_env,
        )?)
    } else if let Some(found) =
        find_project_dir_with(&working_dir, flags.root_dir.as_deref(), |p| p.is_file())
    {
        Some(load_project_file(
            &found.join(PKL_PROJECT_FILENAME),
            &host_env,
        )?)
    } else {
        None
    };

    let sources: Vec<PathBuf> = if !positionals.is_empty() {
        positionals
            .iter()
            .map(|p| {
                let pb = PathBuf::from(p);
                if pb.is_absolute() {
                    pb
                } else {
                    working_dir.join(pb)
                }
            })
            .collect()
    } else if let Some(ref proj) = project {
        proj.tests
            .iter()
            .map(|t| proj.project_dir.join(t))
            .collect()
    } else {
        return Err("Usage: project_probe test [<options>] <modules>...\n\nError: missing argument <modules>".to_string());
    };

    let effective = build_effective_settings(&flags, project.as_ref(), &user_settings, &host_env);
    let env_pairs: Vec<(String, String)> = effective.env.into_iter().collect();
    let prop_pairs: Vec<(String, String)> = effective.external_properties.into_iter().collect();
    let explicit_allowed_modules = flags.allowed_modules.clone().or_else(|| {
        if flags.omit_project_settings {
            None
        } else {
            project
                .as_ref()
                .and_then(|p| p.resolved_evaluator_settings.as_ref())
                .and_then(|s| s.allowed_modules.clone())
        }
    });
    let explicit_allowed_resources = flags.allowed_resources.clone().or_else(|| {
        if flags.omit_project_settings {
            None
        } else {
            project
                .as_ref()
                .and_then(|p| p.resolved_evaluator_settings.as_ref())
                .and_then(|s| s.allowed_resources.clone())
        }
    });
    let host = Arc::new(ProbePackageHost {
        transport: PackageTransportClient::new(flags.ca_certificates.clone(), flags.test_port),
        cache: HostPackageCache::new(
            effective.module_cache_dir.clone(),
            effective.module_cache_dir.is_none(),
        ),
        project: project.as_ref().map(Project::to_legacy_pkl_project),
        test_mode: flags.test_mode,
        allowed_modules: explicit_allowed_modules,
        allowed_resources: explicit_allowed_resources,
        trace_mode: None,
    });

    let mut all_results = Vec::new();
    let mut failed = false;
    let mut all_example_written = true;

    with_package_host(host, || {
        for (idx, src) in sources.iter().enumerate() {
            let res = run_single_test_module(src, flags.overwrite, &env_pairs, &prop_pairs)?;
            let report = render_spec_report(&res);
            print!("{report}");
            if !report.is_empty() && sources.len() > 1 && idx + 1 != sources.len() {
                println!();
            }
            if res.failed() {
                failed = true;
                all_example_written = all_example_written && res.is_example_written_failure();
            }
            all_results.push(res);
        }
        Ok::<(), String>(())
    })?;

    println!();
    print!("{}", render_spec_summary(&all_results));
    if failed {
        Ok(if all_example_written { 10 } else { 1 })
    } else {
        Ok(0)
    }
}

fn cmd_settings(args: &[String]) -> Result<i32, String> {
    let (flags, _) = parse_args(args)?;
    let working_dir = effective_working_dir(&flags)?;
    let host_env: Vec<(String, String)> = std::env::vars().collect();
    let user_settings = load_user_settings(flags.settings_uri.as_deref(), &working_dir, &host_env)?;

    let project = if flags.no_project {
        None
    } else if let Some(ref dir) = flags.project_dir {
        let pb = if dir.is_absolute() {
            dir.clone()
        } else {
            working_dir.join(dir)
        };
        Some(load_project_file(
            &pb.join(PKL_PROJECT_FILENAME),
            &host_env,
        )?)
    } else if let Some(found) =
        find_project_dir_with(&working_dir, flags.root_dir.as_deref(), |p| p.is_file())
    {
        Some(load_project_file(
            &found.join(PKL_PROJECT_FILENAME),
            &host_env,
        )?)
    } else {
        None
    };

    let effective = build_effective_settings(&flags, project.as_ref(), &user_settings, &host_env);
    let mut json = String::from("{\n");
    json.push_str("  \"editorUrlScheme\": ");
    write_escaped_json_string_pub(&mut json, &effective.editor_url_scheme);
    json.push_str(",\n  \"color\": ");
    write_escaped_json_string_pub(&mut json, effective.color.as_str());
    json.push_str(",\n  \"traceMode\": ");
    write_escaped_json_string_pub(&mut json, &effective.trace_mode);
    json.push_str(",\n  \"moduleCacheDir\": ");
    match &effective.module_cache_dir {
        Some(p) => write_escaped_json_string_pub(&mut json, &p.to_string_lossy()),
        None => json.push_str("null"),
    }
    json.push_str(",\n  \"rootDir\": ");
    match &effective.root_dir {
        Some(p) => write_escaped_json_string_pub(&mut json, &p.to_string_lossy()),
        None => json.push_str("null"),
    }
    json.push_str(",\n  \"httpProxyAddress\": ");
    match &effective.http_proxy_address {
        Some(p) => write_escaped_json_string_pub(&mut json, p),
        None => json.push_str("null"),
    }
    json.push_str("\n}");
    println!("{json}");
    Ok(0)
}

fn cmd_eval(args: &[String]) -> Result<i32, String> {
    let (flags, positionals) = parse_args(args)?;
    let target = positionals
        .first()
        .ok_or("eval requires a source module argument")?;
    let working_dir = effective_working_dir(&flags)?;
    let host_env: Vec<(String, String)> = std::env::vars().collect();
    let user_settings = load_user_settings(flags.settings_uri.as_deref(), &working_dir, &host_env)?;

    let project = if flags.no_project {
        None
    } else if let Some(ref dir) = flags.project_dir {
        let pb = if dir.is_absolute() {
            dir.clone()
        } else {
            working_dir.join(dir)
        };
        Some(load_project_file(
            &pb.join(PKL_PROJECT_FILENAME),
            &host_env,
        )?)
    } else if let Some(found) =
        find_project_dir_with(&working_dir, flags.root_dir.as_deref(), |p| p.is_file())
    {
        Some(load_project_file(
            &found.join(PKL_PROJECT_FILENAME),
            &host_env,
        )?)
    } else {
        None
    };

    let effective = build_effective_settings(&flags, project.as_ref(), &user_settings, &host_env);
    let env_pairs: Vec<(String, String)> = effective.env.clone().into_iter().collect();
    let prop_pairs: Vec<(String, String)> =
        effective.external_properties.clone().into_iter().collect();
    let trace_mode = match effective.trace_mode.as_str() {
        "pretty" => TraceMode::Pretty,
        _ => TraceMode::Compact,
    };

    let explicit_allowed_modules = flags.allowed_modules.clone().or_else(|| {
        if flags.omit_project_settings {
            None
        } else {
            project
                .as_ref()
                .and_then(|p| p.resolved_evaluator_settings.as_ref())
                .and_then(|s| s.allowed_modules.clone())
        }
    });
    let explicit_allowed_resources = flags.allowed_resources.clone().or_else(|| {
        if flags.omit_project_settings {
            None
        } else {
            project
                .as_ref()
                .and_then(|p| p.resolved_evaluator_settings.as_ref())
                .and_then(|s| s.allowed_resources.clone())
        }
    });
    let host = Arc::new(ProbePackageHost {
        transport: PackageTransportClient::new(flags.ca_certificates.clone(), flags.test_port),
        cache: HostPackageCache::new(
            effective.module_cache_dir.clone(),
            effective.module_cache_dir.is_none(),
        ),
        project: project.as_ref().map(Project::to_legacy_pkl_project),
        test_mode: flags.test_mode,
        allowed_modules: explicit_allowed_modules,
        allowed_resources: explicit_allowed_resources,
        trace_mode: Some(trace_mode),
    });

    let abs_target = if target.starts_with("package:") || target.starts_with("projectpackage:") {
        None
    } else {
        let pb = PathBuf::from(target);
        let full = if pb.is_absolute() {
            pb
        } else {
            working_dir.join(pb)
        };
        Some(std::fs::canonicalize(&full).unwrap_or(full))
    };

    let (uri, source_text, module_name) = if let Some(ref abs) = abs_target {
        let text = std::fs::read_to_string(abs)
            .map_err(|e| format!("Cannot read `{}`: {e}", abs.display()))?;
        let uri = file_uri_from_path(abs).map_err(|e| format!("{e:?}"))?;
        let name = abs
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("module")
            .to_string();
        (uri, text, name)
    } else {
        let resolved = with_package_host(host.clone(), || {
            pkl_runtime::packages::resolver::resolve_package_module(&PklString::from(
                target.as_str(),
            ))
            .and_then(|m| pkl_runtime::packages::resolver::load_source(&m))
        })
        .map_err(|e| format!("{e:?}"))?;
        (
            target.clone(),
            resolved.text.as_str().unwrap_or("").to_string(),
            resolved.inferred_name.to_java_utf8(),
        )
    };

    let options = ModuleEvalOptions {
        properties: prop_pairs.clone(),
        environment: env_pairs.clone(),
        no_project: flags.no_project,
        diagnostic_profile: DiagnosticProfile::CliBundledSettings,
        ..Default::default()
    };

    let result = with_package_host(host, || {
        evaluate_module_output_with_platform_and_hosts(
            LosslessModuleSourceInput {
                text: SourceText::from(source_text),
                uri: PklString::from(uri.as_str()),
                inferred_name: PklString::from(module_name.as_str()),
            },
            &options,
            None,
            |request: &ModuleRequest| -> ModuleCompletion {
                if let ModuleRequestKind::CheckImport { .. } = &request.kind {
                    return module_host::complete(request);
                }
                module_host::complete(request)
            },
            resource_host::complete,
            file_glob_host::complete,
        )
    });

    if !result.stderr.is_empty() {
        let _ = std::io::Write::write_all(&mut std::io::stderr(), &result.stderr);
    }
    if result.exit_code == 0 {
        let _ = std::io::Write::write_all(&mut std::io::stdout(), &result.stdout);
        Ok(0)
    } else {
        Ok(result.exit_code)
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!(
            "Usage: project_probe <project resolve|project package|download-package|test|settings|eval> [options] [args...]"
        );
        std::process::exit(2);
    }

    let outcome = match args[0].as_str() {
        "project" => match args.get(1).map(String::as_str) {
            Some("resolve") => cmd_project_resolve(&args[2..]),
            Some("package") => cmd_project_package(&args[2..]),
            other => Err(format!("unknown project subcommand: {other:?}")),
        },
        "download-package" => cmd_download_package(&args[1..]),
        "test" => cmd_test(&args[1..]),
        "settings" => cmd_settings(&args[1..]),
        "eval" => cmd_eval(&args[1..]),
        other => Err(format!("unknown subcommand: {other}")),
    };

    match outcome {
        Ok(code) => std::process::exit(code),
        Err(err) => {
            eprintln!("{err}");
            std::process::exit(1);
        }
    }
}

#[allow(dead_code)]
fn _use_color_enum(_: EvaluatorColor) {}
