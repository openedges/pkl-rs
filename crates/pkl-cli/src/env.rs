//! Path/URI normalization, `settings.pkl` and `PklProject` discovery/loading,
//! effective settings resolution, ANSI color detection, and `NativeHost` construction.

use crate::options::{CliError, HttpHeaderMap, ParsedCliOptions};
use pkl_host_native::{
    CaCertSource, CancelFlag, DiskPackageCache, NativeFs, NativeHost, NativeHostConfig,
    NativeHostExtensions, NativeHttpClient, NativePackageTransport, canonicalize_root_dir,
    default_module_cache_dir, file_uri_to_path, path_to_file_uri, read_static_local_project_import,
};
use pkl_runtime::evaluation::session::SourceSession;
use pkl_runtime::host::http::{HttpClientConfig, ProxyConfig};
use pkl_runtime::host::security::SecurityPolicy;
use pkl_runtime::logging::TraceMode;
use pkl_runtime::modules::ModuleHostError;
use pkl_runtime::packages::cycles::{
    filter_same_scheme_cycles, find_import_cycles, format_circular_project_dependencies_error,
};
use pkl_runtime::packages::http::PackageTransport;
use pkl_runtime::packages::packager::parse_imports_and_reads;
use pkl_runtime::packages::project::{
    CliSettingsInput, EffectiveSettings, EvaluatorColor, PKL_PROJECT_DEPS_FILENAME,
    PKL_PROJECT_FILENAME, PklSettings, Project, ProjectDependenciesManager, ProjectDeps,
    SETTINGS_ALLOWED_MODULES, SETTINGS_ALLOWED_RESOURCES, find_project_dir_with,
    resolve_effective_settings, try_parse_static_project,
};
use pkl_runtime::packages::resolver::{PackageResolver, file_uri_from_path};
use pkl_runtime::pkl_binary_decode::{DecodedObject, DecodedValue, decode_lossless};
use pkl_runtime::source::SourceText;
use pkl_runtime::value::{PklString, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::io::{self, IsTerminal, Read};
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

pub struct LoadedSettings {
    pub settings: PklSettings,
    pub http_headers: Option<HttpHeaderMap>,
}

pub struct LoadedProject {
    pub project: Project,
    pub http_headers: Option<HttpHeaderMap>,
}

#[derive(Clone, Debug)]
pub struct ResolvedSourceArg {
    pub uri: String,
    pub local_path: Option<PathBuf>,
    pub is_stdin: bool,
}

pub fn normalize_lexical(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for comp in path.components() {
        match comp {
            Component::CurDir => {}
            Component::ParentDir => {
                if !out.pop() {
                    out.push(comp);
                }
            }
            other => out.push(other),
        }
    }
    out
}

pub fn resolve_path(working_dir: &Path, p: &Path) -> PathBuf {
    let combined = if p.is_absolute() {
        p.to_path_buf()
    } else {
        working_dir.join(p)
    };
    normalize_lexical(&combined)
}

pub fn effective_working_dir(opts: &ParsedCliOptions) -> Result<PathBuf, CliError> {
    let cwd = std::env::current_dir()
        .map_err(|e| CliError::err(format!("Failed to determine working directory: {e}")))?;
    Ok(match &opts.working_dir {
        Some(w) => resolve_path(&cwd, w),
        None => normalize_lexical(&cwd),
    })
}

pub fn pkl_home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(|h| PathBuf::from(h).join(".pkl"))
}

pub fn is_uri_like(s: &str) -> bool {
    pkl_host_native::is_uri_like(s)
}

pub fn normalize_source_module(
    arg: &str,
    working_dir: &Path,
) -> Result<ResolvedSourceArg, CliError> {
    if arg == "-" {
        return Ok(ResolvedSourceArg {
            uri: "repl:text".to_string(),
            local_path: None,
            is_stdin: true,
        });
    }
    if arg.starts_with('@') {
        return Ok(ResolvedSourceArg {
            uri: arg.to_string(),
            local_path: None,
            is_stdin: false,
        });
    }
    if is_uri_like(arg) {
        let local_path = if arg.starts_with("file:") {
            file_uri_to_path(&arg.into()).ok()
        } else {
            None
        };
        return Ok(ResolvedSourceArg {
            uri: arg.to_string(),
            local_path,
            is_stdin: false,
        });
    }
    let resolved = resolve_path(working_dir, Path::new(arg));
    let uri = path_to_file_uri(&resolved).to_java_utf8();
    Ok(ResolvedSourceArg {
        uri,
        local_path: Some(resolved),
        is_stdin: false,
    })
}

/// Resolves whether ANSI color output is active (`Color.hasColor()`).
///
/// Mirrors `org.pkl.core.evaluatorSettings.Color`: `Auto` checks interactive console
/// (`System.console() != null` -> `stdin.is_terminal() && stdout.is_terminal()`).
pub fn resolve_use_color(color: EvaluatorColor) -> bool {
    match color {
        EvaluatorColor::Never => false,
        EvaluatorColor::Always => true,
        EvaluatorColor::Auto => io::stdin().is_terminal() && io::stdout().is_terminal(),
        _ => io::stdin().is_terminal() && io::stdout().is_terminal(),
    }
}

pub fn format_module_host_error(err: &ModuleHostError) -> String {
    match err {
        ModuleHostError::EvalError { message, hint } => match hint {
            Some(h) => format!("{}\n\n{}", message.to_java_utf8(), h.to_java_utf8()),
            None => message.to_java_utf8(),
        },
        ModuleHostError::SecurityManagerException { message }
        | ModuleHostError::HttpClientException { message }
        | ModuleHostError::ExternalReaderProcess { message } => message
            .as_ref()
            .map(|m| m.to_java_utf8())
            .unwrap_or_else(|| "Security or transport error.".to_string()),
        ModuleHostError::UriSyntax { reason } => reason.to_java_utf8(),
        ModuleHostError::Io(io_err) => io_err
            .message
            .as_ref()
            .map(|m| m.to_java_utf8())
            .unwrap_or_else(|| "I/O error.".to_string()),
        ModuleHostError::Denied(msg) | ModuleHostError::Unsupported(msg) => msg.to_java_utf8(),
        ModuleHostError::NotFound => "Module not found.".to_string(),
        ModuleHostError::NoLoader => "No module loader registered.".to_string(),
    }
}

pub fn parse_module_header_info(text: &str, expected_parent_uri: &str) -> (bool, Option<String>) {
    let src = SourceText::from(text);
    let Ok(parsed) = pkl_runtime::parser::parse_module(src.entire()) else {
        return (false, None);
    };
    let Some(decl) = parsed.declaration else {
        return (false, None);
    };
    let direct_parent = decl
        .parent
        .as_ref()
        .is_some_and(|p| p.uri.as_str() == Some(expected_parent_uri));
    let declared_name = decl.name.map(|parts| {
        parts
            .iter()
            .map(|p| p.to_java_utf8())
            .collect::<Vec<_>>()
            .join(".")
    });
    (direct_parent, declared_name)
}

fn extract_decoded_http_headers(obj: &DecodedObject) -> Option<HttpHeaderMap> {
    let http_val = obj.get_property("http")?;
    let DecodedValue::Object(http_obj) = http_val else {
        return None;
    };
    let headers_val = http_obj.get_property("headers")?;
    let entries = match headers_val {
        DecodedValue::Map(e) | DecodedValue::Mapping(e) => e,
        _ => return None,
    };
    let mut map: HttpHeaderMap = BTreeMap::new();
    for (pat_val, header_map_val) in entries {
        let DecodedValue::String(pat_pkl) = pat_val else {
            continue;
        };
        let Some(pat) = pat_pkl.as_str() else {
            continue;
        };
        let header_entries = match header_map_val {
            DecodedValue::Map(e) | DecodedValue::Mapping(e) => e,
            _ => continue,
        };
        let list = map.entry(pat.to_string()).or_default();
        for (name_val, val_val) in header_entries {
            let DecodedValue::String(name_pkl) = name_val else {
                continue;
            };
            let Some(name) = name_pkl.as_str() else {
                continue;
            };
            match val_val {
                DecodedValue::String(v) => {
                    if let Some(vs) = v.as_str() {
                        list.push((name.to_string(), vs.to_string()));
                    }
                }
                DecodedValue::List(items) | DecodedValue::Listing(items) => {
                    for item in items {
                        if let DecodedValue::String(v) = item
                            && let Some(vs) = v.as_str()
                        {
                            list.push((name.to_string(), vs.to_string()));
                        }
                    }
                }
                _ => {}
            }
        }
    }
    Some(map)
}

pub fn value_is_true(value: &Value) -> bool {
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
            other => return Err(format!("Invalid base64 byte {other}")),
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

fn build_project_cycle_fs_guard(
    opts: &ParsedCliOptions,
    working_dir: &Path,
) -> Result<Option<NativeFs>, CliError> {
    if opts.allowed_modules.is_none() && opts.root_dir.is_none() {
        return Ok(None);
    }
    let normalized_root = match opts.root_dir.as_deref() {
        Some(dir) => {
            let resolved = resolve_path(working_dir, dir);
            let canon = canonicalize_root_dir(&resolved).map_err(|e| {
                CliError::err(format!(
                    "Failed to normalize rootDir `{}`: {e}",
                    resolved.display()
                ))
            })?;
            Some(canon.to_string_lossy().into_owned())
        }
        None => None,
    };
    let policy = SecurityPolicy::new(opts.allowed_modules.as_deref(), None, normalized_root)
        .map_err(CliError::err)?;
    Ok(Some(NativeFs::new(policy)))
}

fn check_project_import_cycles(
    project_file: &Path,
    fs_guard: Option<&NativeFs>,
) -> Result<(), CliError> {
    let norm = normalize_lexical(project_file);
    let root_uri =
        file_uri_from_path(&norm).map_err(|e| CliError::err(format_module_host_error(&e)))?;
    let mut graph: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut queue: Vec<PathBuf> = vec![norm];
    let mut visited: BTreeSet<PathBuf> = BTreeSet::new();

    while let Some(path) = queue.pop() {
        let Ok(uri) = file_uri_from_path(&path) else {
            continue;
        };
        let (canon, src) = if let Some(fs) = fs_guard {
            let Ok((_, real_path)) = fs.resolve_file_module(&PklString::from(uri.as_str())) else {
                continue;
            };
            if !visited.insert(real_path.clone()) {
                continue;
            }
            let Ok(bytes) = fs.read_file_module_bytes(&real_path) else {
                continue;
            };
            let Ok(src) = String::from_utf8(bytes) else {
                continue;
            };
            (real_path, src)
        } else {
            let canon = std::fs::canonicalize(&path).unwrap_or_else(|_| path.clone());
            if !visited.insert(canon.clone()) {
                continue;
            }
            let Ok(src) = std::fs::read_to_string(&path) else {
                continue;
            };
            (canon, src)
        };
        let _ = canon;
        let Ok(entries) = parse_imports_and_reads(&SourceText::from(src)) else {
            continue;
        };
        let parent = path.parent().unwrap_or(&path);
        let mut edges = Vec::new();
        for entry in entries {
            if !entry.is_module || entry.is_glob {
                continue;
            }
            let s = entry.string_value.as_str().unwrap_or("");
            if s.starts_with("pkl:") || s.starts_with("package:") || s.starts_with('@') {
                continue;
            }
            let target = if s.starts_with("file:") {
                let Ok(p) = file_uri_to_path(&PklString::from(s)) else {
                    continue;
                };
                normalize_lexical(&p)
            } else if !is_uri_like(s) {
                normalize_lexical(&parent.join(s))
            } else {
                continue;
            };
            if let Ok(target_uri) = file_uri_from_path(&target) {
                let allowed = match fs_guard {
                    Some(fs) => fs
                        .resolve_file_module(&PklString::from(target_uri.as_str()))
                        .is_ok(),
                    None => target.is_file(),
                };
                if allowed {
                    if !edges.contains(&target_uri) {
                        edges.push(target_uri);
                    }
                    queue.push(target);
                }
            }
        }
        graph.insert(uri, edges);
    }

    let cycles = filter_same_scheme_cycles(&root_uri, find_import_cycles(&graph));
    if let Some(msg) = format_circular_project_dependencies_error(&cycles) {
        return Err(CliError::err(format!("–– Pkl Error ––\n{msg}")));
    }
    Ok(())
}

/// Loads `PklSettings` from `--settings <uri>` or `~/.pkl/settings.pkl` (`PklSettings.java:48-88`).
pub fn load_user_settings(
    opts: &ParsedCliOptions,
    working_dir: &Path,
    host_env: &[(String, String)],
) -> Result<LoadedSettings, CliError> {
    let target_file = match opts.settings_uri.as_deref() {
        Some("pkl:settings") => {
            return Ok(LoadedSettings {
                settings: PklSettings::default(),
                http_headers: None,
            });
        }
        Some(uri) => {
            if uri.starts_with("file:") {
                let path = file_uri_to_path(&PklString::from(uri))
                    .map_err(|e| CliError::err(e.to_java_utf8()))?;
                normalize_lexical(&path)
            } else if is_uri_like(uri) {
                return Err(CliError::err(format!("Unsupported settings URI `{uri}`.")));
            } else {
                resolve_path(working_dir, Path::new(uri))
            }
        }
        None => {
            let Some(home) = pkl_home_dir() else {
                return Ok(LoadedSettings {
                    settings: PklSettings::default(),
                    http_headers: None,
                });
            };
            let default_file = home.join("settings.pkl");
            if !default_file.is_file() {
                return Ok(LoadedSettings {
                    settings: PklSettings::default(),
                    http_headers: None,
                });
            }
            normalize_lexical(&default_file)
        }
    };

    let uri = file_uri_from_path(&target_file)
        .map_err(|e| CliError::err(format_module_host_error(&e)))?;
    let text = std::fs::read_to_string(&target_file).map_err(|e| {
        CliError::err(format!(
            "Cannot find settings file `{}`: {e}",
            target_file.display()
        ))
    })?;
    let inferred_name = target_file
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("settings");

    let host = NativeHost::new(NativeHostConfig {
        allowed_modules: Some(
            SETTINGS_ALLOWED_MODULES
                .iter()
                .map(|s| (*s).to_string())
                .collect(),
        ),
        allowed_resources: Some(
            SETTINGS_ALLOWED_RESOURCES
                .iter()
                .map(|s| (*s).to_string())
                .collect(),
        ),
        properties: Vec::new(),
        environment: host_env.to_vec(),
        timeout_ms: opts.timeout_ms,
        test_mode: opts.test_mode,
        internal_pkl_binary_eval: true,
        ..Default::default()
    })
    .map_err(CliError::err)?;

    let (direct_settings, _) = parse_module_header_info(&text, "pkl:settings");
    let check_exprs = if direct_settings {
        vec![
            "output.value == module || (output.value != null && let (ref = import(\"pkl:reflect\")) ref.Class(output.value.getClass()).isSubclassOf(ref.Class(import(\"pkl:settings\").getClass())))".to_string(),
            "if (output.value == module) \"pkl.settings\" else if (output.value is Module) (import(\"pkl:reflect\")).Module(output.value).name else output.value.getClass().simpleName".to_string(),
            "output.bytes".to_string(),
        ]
    } else {
        vec![
            "output.value != null && let (ref = import(\"pkl:reflect\")) ref.Class(output.value.getClass()).isSubclassOf(ref.Class(import(\"pkl:settings\").getClass()))".to_string(),
            "if (output.value is Module) (import(\"pkl:reflect\")).Module(output.value).name else output.value.getClass().simpleName".to_string(),
            "output.bytes".to_string(),
        ]
    };
    let (vals, _) = host
        .evaluate_expressions(&uri, &text, inferred_name, &check_exprs)
        .map_err(CliError::err)?;
    host.close();

    if !value_is_true(&vals[0]) {
        let got_type = SourceSession::result_string(vals[1].clone())
            .map(|s| s.to_java_utf8())
            .unwrap_or_else(|_| "Any".to_string());
        return Err(CliError::err(format!(
            "Expected `output.value` of module `{uri}` to be of type `pkl.settings`, but got type `{got_type}`."
        )));
    }

    let bytes = extract_binary_value(vals[2].clone()).map_err(CliError::err)?;
    let decoded = decode_lossless(&bytes)
        .map_err(|e| CliError::err(format!("Failed to decode settings pkl-binary: {e:?}")))?;
    let settings = PklSettings::from_decoded_value(&decoded, &uri)
        .map_err(|e| CliError::err(format_module_host_error(&e)))?;
    let http_headers = match &decoded {
        DecodedValue::Object(obj) => extract_decoded_http_headers(obj),
        _ => None,
    };

    Ok(LoadedSettings {
        settings,
        http_headers,
    })
}

/// Loads a `PklProject` directly through `NativeHost` + `pkl-binary` (`CliCommand.kt:89-109`).
pub fn load_project_file(
    project_file: &Path,
    opts: &ParsedCliOptions,
    working_dir: &Path,
    host_env: &[(String, String)],
) -> Result<LoadedProject, CliError> {
    let norm = normalize_lexical(project_file);
    let fs_guard = build_project_cycle_fs_guard(opts, working_dir)?;
    check_project_import_cycles(&norm, fs_guard.as_ref())?;

    let uri = file_uri_from_path(&norm).map_err(|e| CliError::err(format_module_host_error(&e)))?;
    let text = if let Some(ref fs) = fs_guard {
        let (_, real_path) = fs
            .resolve_file_module(&PklString::from(uri.as_str()))
            .map_err(|e| CliError::err(format_module_host_error(&e)))?;
        let bytes = fs
            .read_file_module_bytes(&real_path)
            .map_err(|e| CliError::err(format_module_host_error(&e)))?;
        String::from_utf8(bytes).map_err(|e| {
            CliError::err(format!(
                "Cannot read PklProject file `{}`: {e}",
                norm.display()
            ))
        })?
    } else {
        std::fs::read_to_string(&norm).map_err(|e| {
            CliError::err(format!(
                "Cannot read PklProject file `{}`: {e}",
                norm.display()
            ))
        })?
    };

    if opts.allowed_modules.is_none()
        && opts.allowed_resources.is_none()
        && opts.root_dir.is_none()
        && opts.timeout_ms.is_none()
        && let Some(project) =
            try_parse_static_project(&uri, &text, &mut read_static_local_project_import)
    {
        return Ok(LoadedProject {
            project,
            http_headers: None,
        });
    }

    let project_env = if opts.has_env {
        opts.env_vars.clone()
    } else {
        host_env.to_vec()
    };
    let root_dir = opts
        .root_dir
        .as_deref()
        .map(|r| resolve_path(working_dir, r));

    let host = NativeHost::new(NativeHostConfig {
        allowed_modules: opts.allowed_modules.clone(),
        allowed_resources: opts.allowed_resources.clone(),
        root_dir,
        properties: Vec::new(),
        environment: project_env,
        timeout_ms: opts.timeout_ms,
        test_mode: opts.test_mode,
        power_assertions: opts.power_assertions,
        internal_pkl_binary_eval: true,
        ..Default::default()
    })
    .map_err(CliError::err)?;

    let (direct_project, _) = parse_module_header_info(&text, "pkl:Project");
    let check_or_type_expr = if direct_project {
        "if (output.value == module || (output.value != null && let (ref = import(\"pkl:reflect\")) ref.Class(output.value.getClass()).isSubclassOf(ref.Class(import(\"pkl:Project\").getClass())))) \"\" else if (output.value is Module) (import(\"pkl:reflect\")).Module(output.value).name else output.value.getClass().simpleName"
    } else {
        "if (output.value != null && let (ref = import(\"pkl:reflect\")) ref.Class(output.value.getClass()).isSubclassOf(ref.Class(import(\"pkl:Project\").getClass()))) \"\" else if (output.value is Module) (import(\"pkl:reflect\")).Module(output.value).name else output.value.getClass().simpleName"
    };
    let exprs = [check_or_type_expr.to_string(), "output.bytes".to_string()];
    let eval_result = host.evaluate_expressions_partial(&uri, &text, "PklProject", &exprs);
    host.close();
    let (mut results, _) = eval_result.map_err(CliError::err)?;
    let mut iter = results.drain(..);
    let check_val = iter
        .next()
        .ok_or_else(|| CliError::err("Missing PklProject type-check result."))?
        .map_err(CliError::err)?;
    let got_type = SourceSession::result_string(check_val)
        .map(|s| s.to_java_utf8())
        .unwrap_or_else(|_| "Any".to_string());
    if !got_type.is_empty() {
        return Err(CliError::err(format!(
            "Expected `output.value` of module `{uri}` to be of type `pkl.Project`, but got type `{got_type}`."
        )));
    }
    let bytes_val = iter
        .next()
        .ok_or_else(|| CliError::err("Missing PklProject output.bytes result."))?
        .map_err(CliError::err)?;
    let bytes = extract_binary_value(bytes_val).map_err(CliError::err)?;
    let decoded = decode_lossless(&bytes)
        .map_err(|e| CliError::err(format!("Failed to decode PklProject pkl-binary: {e:?}")))?;
    let project = Project::from_decoded_value(&decoded)
        .map_err(|e| CliError::err(format_module_host_error(&e)))?;
    let http_headers = match &decoded {
        DecodedValue::Object(obj) => {
            obj.get_property("resolvedEvaluatorSettings")
                .and_then(|v| match v {
                    DecodedValue::Object(s) => extract_decoded_http_headers(s),
                    _ => None,
                })
        }
        _ => None,
    };

    Ok(LoadedProject {
        project,
        http_headers,
    })
}

pub fn discover_optional_project(
    opts: &ParsedCliOptions,
    working_dir: &Path,
    host_env: &[(String, String)],
) -> Result<Option<LoadedProject>, CliError> {
    if opts.no_project {
        return Ok(None);
    }
    if let Some(ref dir) = opts.project_dir {
        let resolved = resolve_path(working_dir, dir);
        let file = resolved.join(PKL_PROJECT_FILENAME);
        if !file.is_file() {
            return Err(CliError::err(format!(
                "Directory {} does not contain a PklProject file.",
                resolved.display()
            )));
        }
        return Ok(Some(load_project_file(&file, opts, working_dir, host_env)?));
    }
    let root_dir = opts
        .root_dir
        .as_deref()
        .map(|r| resolve_path(working_dir, r));
    if let Some(found) = find_project_dir_with(working_dir, root_dir.as_deref(), |p| p.is_file()) {
        let file = found.join(PKL_PROJECT_FILENAME);
        return Ok(Some(load_project_file(&file, opts, working_dir, host_env)?));
    }
    Ok(None)
}

pub fn load_project_dependencies_manager(
    project: &Project,
) -> Result<ProjectDependenciesManager, CliError> {
    let mut mgr = ProjectDependenciesManager::new(
        project.declared_dependencies.clone(),
        project.project_dir.clone(),
    );
    let deps_file = project.project_dir.join(PKL_PROJECT_DEPS_FILENAME);
    if deps_file.is_file() {
        let deps_uri = mgr.project_deps_file_uri();
        let text = pkl_host_native::read_path_to_string_bounded(&deps_file).map_err(|e| {
            CliError::err(format!(
                "Encountered an error when attempting to load `PklProject.deps.json` at `{deps_uri}`.\n{e}\n\nTry running `pkl project resolve` within the project directory to create a new set of dependencies."
            ))
        })?;
        let parsed = ProjectDeps::parse(&text, &deps_uri)
            .map_err(|e| CliError::err(format_module_host_error(&e)))?;
        mgr.initialize_with_deps(parsed)
            .map_err(|e| CliError::err(format_module_host_error(&e)))?;
    }
    Ok(mgr)
}

pub fn build_effective_settings(
    opts: &ParsedCliOptions,
    working_dir: &Path,
    project: Option<&LoadedProject>,
    user_settings: &LoadedSettings,
    host_env: &[(String, String)],
) -> (EffectiveSettings, Option<HttpHeaderMap>) {
    let cli_ext_mod = opts.external_module_readers.clone();
    let cli_ext_res = opts.external_resource_readers.clone();
    let cli_module_path = opts
        .module_path
        .as_ref()
        .map(|paths| paths.iter().map(|p| resolve_path(working_dir, p)).collect());

    let cli = CliSettingsInput {
        omit_project_settings: opts.omit_project_settings,
        external_properties: opts.has_properties.then(|| opts.properties.clone()),
        output_format: opts.output_format.clone(),
        env: opts.has_env.then(|| opts.env_vars.clone()),
        allowed_modules: opts.allowed_modules.clone(),
        allowed_resources: opts.allowed_resources.clone(),
        no_cache: opts.no_cache,
        module_cache_dir: opts
            .cache_dir
            .as_deref()
            .map(|p| resolve_path(working_dir, p)),
        root_dir: opts
            .root_dir
            .as_deref()
            .map(|p| resolve_path(working_dir, p)),
        module_path: cli_module_path,
        color: opts.color,
        trace_mode: opts.trace_mode.clone(),
        timeout_nanos: opts.timeout_ms.map(|ms| ms.saturating_mul(1_000_000)),
        http_proxy_address: opts.http_proxy.clone(),
        http_no_proxy: opts.http_no_proxy.clone(),
        http_rewrites: opts.http_rewrites.clone(),
        external_module_readers: cli_ext_mod,
        external_resource_readers: cli_ext_res,
        apply_project_timeout: false,
    };

    let effective = resolve_effective_settings(
        &cli,
        project.and_then(|p| p.project.resolved_evaluator_settings.as_ref()),
        &user_settings.settings,
        host_env,
        default_module_cache_dir(),
    );

    let effective_headers = opts
        .http_headers
        .clone()
        .or_else(|| {
            if opts.omit_project_settings {
                None
            } else {
                project.and_then(|p| p.http_headers.clone())
            }
        })
        .or_else(|| user_settings.http_headers.clone());

    (effective, effective_headers)
}

fn default_native_user_agent() -> String {
    let os = if cfg!(target_os = "macos") {
        "macOS"
    } else if cfg!(target_os = "linux") {
        "Linux"
    } else if cfg!(target_os = "windows") {
        "Windows"
    } else {
        std::env::consts::OS
    };
    format!("pkl-rs/0.32.0 ({os}; native)")
}

pub fn build_http_client_config(
    opts: &ParsedCliOptions,
    effective: &EffectiveSettings,
    effective_headers: Option<&HttpHeaderMap>,
) -> Result<HttpClientConfig, CliError> {
    let proxy = ProxyConfig::new(
        effective.http_proxy_address.as_deref(),
        &effective.http_no_proxy,
    )
    .map_err(CliError::err)?;

    let mut cfg = HttpClientConfig {
        user_agent: default_native_user_agent(),
        ..HttpClientConfig::default()
    }
    .with_proxy(proxy)
    .with_test_port(opts.test_port);

    if !effective.http_rewrites.is_empty() {
        cfg = cfg
            .with_rewrites(effective.http_rewrites.clone())
            .map_err(CliError::err)?;
    }
    if let Some(headers) = effective_headers {
        for (pattern, pairs) in headers {
            cfg.add_headers(pattern, pairs.clone())
                .map_err(CliError::err)?;
        }
    }
    Ok(cfg)
}

pub fn build_ca_cert_source(opts: &ParsedCliOptions, working_dir: &Path) -> CaCertSource {
    if !opts.ca_certificates.is_empty() {
        let resolved: Vec<PathBuf> = opts
            .ca_certificates
            .iter()
            .map(|p| resolve_path(working_dir, p))
            .collect();
        return CaCertSource::from_files(resolved);
    }
    // `CaCertSource::bundled_default()` lazily probes `~/.pkl/cacerts/` inside
    // `build_tls_config()` only when an actual HTTPS request occurs.
    CaCertSource::bundled_default()
}

pub fn build_native_host_config(
    opts: &ParsedCliOptions,
    working_dir: &Path,
    project: Option<&LoadedProject>,
    effective: &EffectiveSettings,
    effective_headers: Option<&HttpHeaderMap>,
    extra_properties: &[(String, String)],
) -> Result<NativeHostConfig, CliError> {
    let http_cfg = build_http_client_config(opts, effective, effective_headers)?;
    let ca_certs = build_ca_cert_source(opts, working_dir);

    let mut properties: Vec<(String, String)> = effective
        .external_properties
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    for (k, v) in extra_properties {
        properties.retain(|(ek, _)| ek != k);
        properties.push((k.clone(), v.clone()));
    }
    let environment: Vec<(String, String)> = effective
        .env
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();

    let project_dependencies = match project {
        Some(lp) => Some(load_project_dependencies_manager(&lp.project)?),
        None => None,
    };

    let trace_mode = match effective.trace_mode.as_str() {
        "pretty" => TraceMode::Pretty,
        _ => TraceMode::Compact,
    };

    let default_mods = effective.external_module_readers.is_empty()
        && effective.allowed_modules.iter().map(String::as_str).eq(
            pkl_runtime::host::security::DEFAULT_ALLOWED_MODULES
                .iter()
                .copied(),
        );
    let default_res = effective.external_resource_readers.is_empty()
        && effective.allowed_resources.iter().map(String::as_str).eq(
            pkl_runtime::host::security::DEFAULT_ALLOWED_RESOURCES
                .iter()
                .copied(),
        );

    Ok(NativeHostConfig {
        allowed_modules: (!default_mods).then(|| effective.allowed_modules.clone()),
        allowed_resources: (!default_res).then(|| effective.allowed_resources.clone()),
        root_dir: effective.root_dir.clone(),
        module_paths: effective.module_path.clone(),
        http: http_cfg,
        ca_certs,
        properties,
        environment,
        timeout_ms: opts.timeout_ms,
        external_module_schemes: Vec::new(),
        external_resource_schemes: Vec::new(),
        external_module_readers: effective.external_module_readers.clone(),
        external_resource_readers: effective.external_resource_readers.clone(),
        module_cache_dir: effective.module_cache_dir.clone(),
        project_dependencies,
        test_mode: opts.test_mode,
        trace_mode,
        power_assertions: opts.power_assertions,
        editor_url_scheme: effective.editor_url_scheme.clone(),
        internal_pkl_binary_eval: false,
    })
}

pub fn build_native_host(
    opts: &ParsedCliOptions,
    working_dir: &Path,
    project: Option<&LoadedProject>,
    effective: &EffectiveSettings,
    effective_headers: Option<&HttpHeaderMap>,
    extra_properties: &[(String, String)],
    use_color: bool,
) -> Result<NativeHost, CliError> {
    let config = build_native_host_config(
        opts,
        working_dir,
        project,
        effective,
        effective_headers,
        extra_properties,
    )?;
    let mut ext = NativeHostExtensions::default().with_color(use_color);
    if let Some(ref fmt) = opts.output_format {
        ext = ext.with_output_format(fmt.clone());
    }
    NativeHost::with_extensions(config, ext).map_err(CliError::err)
}

pub fn build_standalone_package_resolver(
    opts: &ParsedCliOptions,
    working_dir: &Path,
    effective: &EffectiveSettings,
    effective_headers: Option<&HttpHeaderMap>,
) -> Result<(PackageResolver, Arc<NativeHttpClient>), CliError> {
    let http_cfg = build_http_client_config(opts, effective, effective_headers)?;
    let ca_certs = build_ca_cert_source(opts, working_dir);
    let cancel_flag = CancelFlag::new();
    let http = Arc::new(NativeHttpClient::new(http_cfg, ca_certs, cancel_flag));
    let policy = Arc::new(
        SecurityPolicy::new(
            Some(&effective.allowed_modules),
            Some(&effective.allowed_resources),
            None,
        )
        .map_err(CliError::err)?,
    );
    let transport: Arc<dyn PackageTransport> =
        Arc::new(NativePackageTransport::new(http.clone(), policy));
    let resolver = match &effective.module_cache_dir {
        Some(dir) => PackageResolver::disk_cached(
            transport,
            Arc::new(DiskPackageCache::new(dir.clone())),
            opts.test_mode,
        ),
        None => PackageResolver::in_memory(transport, opts.test_mode),
    };
    Ok((resolver, http))
}

pub fn read_stdin_utf8() -> Result<String, CliError> {
    let mut buf = String::new();
    io::stdin()
        .read_to_string(&mut buf)
        .map_err(|e| CliError::err(format!("Failed to read stdin: {e}")))?;
    Ok(buf)
}
