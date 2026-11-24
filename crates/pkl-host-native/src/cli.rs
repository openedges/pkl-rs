//! Unified native Pkl 0.32.0 CLI implementation (`pkl_m4_cli`) integrating:
//! - Track A (`NativeHost`, `NativeFs`, `NativeModulePathResolver`, `NativeHttpClient`, `CaCertSource`)
//! - Track B (`ExternalReaderRegistry`, `ExternalReaderConfig`, `shlex`)
//! - Track C (`PackageResolver`, `ProjectDependenciesManager`, `ProjectDependenciesResolver`,
//!   `ProjectPackager`, `DiskPackageCache`, `NativePackageTransport`, `resolve_effective_settings`,
//!   `test_runner`)
//!
//! Supports `eval`, `project resolve`, `project package`, `download-package`,
//! `analyze imports`, `test`, and `settings`.

use crate::certs::CaCertSource;
use crate::driver::{NativeHost, NativeHostConfig, NativeStreamingOutputFilesOutcome};
use crate::fs::{path_to_file_uri, read_static_local_project_import};
use crate::http::{CancelFlag, NativeHttpClient};
use crate::package_host::{
    DiskPackageCache, NativePackageTransport, NativePackagerHost, default_module_cache_dir,
};
use pkl_runtime::evaluation::frontend::{DiagnosticProfile, ModuleEvalOptions, check_path_spec};
use pkl_runtime::evaluation::session::SourceSession;
use pkl_runtime::external_reader::ExternalReaderConfig;
use pkl_runtime::host::http::{HttpClientConfig, ProxyConfig};
use pkl_runtime::host::security::SecurityPolicy;
use pkl_runtime::logging::TraceMode;
use pkl_runtime::modules::ModuleHostError;
use pkl_runtime::packages::PackageUri;
use pkl_runtime::packages::cycles::{
    filter_same_scheme_cycles, find_import_cycles, format_circular_project_dependencies_error,
};
use pkl_runtime::packages::http::PackageTransport;
use pkl_runtime::packages::metadata::write_escaped_json_string_pub;
use pkl_runtime::packages::mvs::{ProjectDependenciesResolver, relativize_paths};
use pkl_runtime::packages::packager::{ProjectPackager, parse_imports_and_reads};
use pkl_runtime::packages::project::{
    CliSettingsInput, EffectiveSettings, EvaluatorColor, PKL_PROJECT_DEPS_FILENAME,
    PKL_PROJECT_FILENAME, PklSettings, Project, ProjectDependenciesManager, ProjectDeps,
    SETTINGS_ALLOWED_MODULES, SETTINGS_ALLOWED_RESOURCES, find_project_dir_with,
    resolve_effective_settings, try_parse_static_project,
};
use pkl_runtime::packages::resolver::{PackageResolver, file_uri_from_path};
use pkl_runtime::packages::test_runner::{
    RENDER_EXAMPLES_DOC_EXPR, TestError, TestFailure, TestResult, TestResults, TestSectionName,
    TestSectionResults, evaluate_examples_rendered, evaluate_facts_decoded_with_source,
    render_spec_report, render_spec_summary,
};
use pkl_runtime::pkl_binary_decode::{DecodedObject, DecodedValue, decode_lossless};
use pkl_runtime::source::SourceText;
use pkl_runtime::value::Value;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::io::{self, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

type HttpHeaderMap = BTreeMap<String, Vec<(String, String)>>;

const ANALYZE_IMPORTS_MODULE: &str = r#"import "pkl:analyze"

local importStrings = read*("prop:pkl.analyzeImports.**").toMap().values.toSet()

output {
  value = analyze.importGraph(importStrings)
  renderer {
    converters {
      [Map] = (it) -> it.toMapping()
      [Set] = (it) -> it.toListing()
    }
  }
}
"#;

/// Parsed CLI options shared across Pkl subcommands (`BaseOptions.kt` + subcommand options).
#[derive(Clone, Debug)]
pub struct ParsedCliOptions {
    pub allowed_modules: Option<Vec<String>>,
    pub allowed_resources: Option<Vec<String>>,
    pub root_dir: Option<PathBuf>,
    pub cache_dir: Option<PathBuf>,
    pub no_cache: bool,
    pub working_dir: Option<PathBuf>,
    pub properties: Vec<(String, String)>,
    pub has_properties: bool,
    pub env_vars: Vec<(String, String)>,
    pub has_env: bool,
    pub output_format: Option<String>,
    pub module_path: Option<Vec<PathBuf>>,
    pub settings_uri: Option<String>,
    pub timeout_ms: Option<u64>,
    pub ca_certificates: Vec<PathBuf>,
    pub http_proxy: Option<String>,
    pub http_no_proxy: Option<Vec<String>>,
    pub http_rewrites: Option<BTreeMap<String, String>>,
    pub http_headers: Option<HttpHeaderMap>,
    pub external_module_readers: Option<BTreeMap<String, ExternalReaderConfig>>,
    pub external_resource_readers: Option<BTreeMap<String, ExternalReaderConfig>>,
    pub color: Option<EvaluatorColor>,
    pub trace_mode: Option<String>,
    pub project_dir: Option<PathBuf>,
    pub no_project: bool,
    pub omit_project_settings: bool,
    pub test_mode: bool,
    pub test_port: Option<u16>,
    pub power_assertions: bool,
    // Subcommand-specific options
    pub output_path: Option<String>,
    pub multiple_file_output_path: Option<String>,
    pub expression: Option<String>,
    pub module_output_separator: String,
    pub skip_publish_check: bool,
    pub no_transitive: bool,
    pub overwrite: bool,
}

impl Default for ParsedCliOptions {
    fn default() -> Self {
        Self {
            allowed_modules: None,
            allowed_resources: None,
            root_dir: None,
            cache_dir: None,
            no_cache: false,
            working_dir: None,
            properties: Vec::new(),
            has_properties: false,
            env_vars: Vec::new(),
            has_env: false,
            output_format: None,
            module_path: None,
            settings_uri: None,
            timeout_ms: None,
            ca_certificates: Vec::new(),
            http_proxy: None,
            http_no_proxy: None,
            http_rewrites: None,
            http_headers: None,
            external_module_readers: None,
            external_resource_readers: None,
            color: None,
            trace_mode: None,
            project_dir: None,
            no_project: false,
            omit_project_settings: false,
            test_mode: false,
            test_port: None,
            power_assertions: true,
            output_path: None,
            multiple_file_output_path: None,
            expression: None,
            module_output_separator: "---".to_string(),
            skip_publish_check: false,
            no_transitive: false,
            overwrite: false,
        }
    }
}

/// Top-level CLI error distinguishing exit code and whether message should be printed.
#[derive(Debug)]
enum CliError {
    /// Print message to stderr (ensuring trailing `\n`) and exit with `code`.
    Message { text: String, exit_code: i32 },
    /// Already wrote output/errors; just exit with `exit_code`.
    Silent(i32),
}

impl CliError {
    fn err(msg: impl Into<String>) -> Self {
        Self::Message {
            text: msg.into(),
            exit_code: 1,
        }
    }
}

/// Entry point for `pkl_m4_cli` (takes arguments AFTER `argv[0]`).
pub fn run_cli(args: &[String]) -> i32 {
    if args.is_empty() {
        eprintln!(
            "Usage: pkl <eval|project resolve|project package|download-package|analyze imports|test|settings> [<options>] [<args>...]"
        );
        return 1;
    }

    let outcome = match args[0].as_str() {
        "--version" | "-v" => {
            println!("Pkl 0.32.0");
            Ok(0)
        }
        "--help" | "-h" => {
            println!(
                "Usage: pkl <eval|project resolve|project package|download-package|analyze imports|test|settings> [<options>] [<args>...]"
            );
            Ok(0)
        }
        "eval" => cmd_eval(&args[1..]),
        "project" => match args.get(1).map(String::as_str) {
            Some("resolve") => cmd_project_resolve(&args[2..]),
            Some("package") => cmd_project_package(&args[2..]),
            other => Err(CliError::err(format!(
                "Unknown `project` subcommand: {}",
                other.unwrap_or("<missing>")
            ))),
        },
        "download-package" => cmd_download_package(&args[1..]),
        "analyze" => match args.get(1).map(String::as_str) {
            Some("imports") => cmd_analyze_imports(&args[2..]),
            other => Err(CliError::err(format!(
                "Unknown `analyze` subcommand: {}",
                other.unwrap_or("<missing>")
            ))),
        },
        "test" => cmd_test(&args[1..]),
        "settings" => cmd_settings(&args[1..]),
        other => Err(CliError::err(format!("Unknown command: `{other}`."))),
    };

    match outcome {
        Ok(code) => code,
        Err(CliError::Silent(code)) => code,
        Err(CliError::Message { text, exit_code }) => {
            let mut stderr = io::stderr().lock();
            let _ = stderr.write_all(text.as_bytes());
            if !text.ends_with('\n') {
                let _ = stderr.write_all(b"\n");
            }
            let _ = stderr.flush();
            exit_code
        }
    }
}

// ============================================================================
// Argument Parsing (`BaseOptions.kt` + Subcommand Options)
// ============================================================================

fn parse_options(args: &[String]) -> Result<(ParsedCliOptions, Vec<String>), CliError> {
    let mut opts = ParsedCliOptions::default();
    let mut positionals = Vec::new();
    let mut i = 0usize;
    let mut end_of_flags = false;

    while i < args.len() {
        let raw = &args[i];
        if end_of_flags || raw == "-" || !raw.starts_with('-') {
            positionals.push(raw.clone());
            i += 1;
            continue;
        }
        if raw == "--" {
            end_of_flags = true;
            i += 1;
            continue;
        }

        // Split `--flag=value` (only for `--...` long flags).
        let (flag, inline_val) = if raw.starts_with("--") {
            match raw.split_once('=') {
                Some((k, v)) => (k, Some(v)),
                None => (raw.as_str(), None),
            }
        } else {
            (raw.as_str(), None)
        };

        let mut take_value = |flag_name: &str| -> Result<String, CliError> {
            if let Some(v) = inline_val {
                Ok(v.to_string())
            } else {
                i += 1;
                args.get(i)
                    .cloned()
                    .ok_or_else(|| CliError::err(format!("Option `{flag_name}` requires a value.")))
            }
        };

        match flag {
            "--allowed-modules" => {
                let val = take_value(flag)?;
                let list = opts.allowed_modules.get_or_insert_with(Vec::new);
                for pat in val.split(',') {
                    list.push(pat.to_string());
                }
            }
            "--allowed-resources" => {
                let val = take_value(flag)?;
                let list = opts.allowed_resources.get_or_insert_with(Vec::new);
                for pat in val.split(',') {
                    list.push(pat.to_string());
                }
            }
            "--root-dir" => {
                opts.root_dir = Some(PathBuf::from(take_value(flag)?));
            }
            "--cache-dir" => {
                opts.cache_dir = Some(PathBuf::from(take_value(flag)?));
            }
            "--no-cache" => {
                opts.no_cache = true;
            }
            "-w" | "--working-dir" => {
                opts.working_dir = Some(PathBuf::from(take_value(flag)?));
            }
            "-p" | "--property" => {
                let val = take_value(flag)?;
                opts.has_properties = true;
                opts.properties.push(parse_property_kv(&val));
            }
            "-e" | "--env-var" => {
                let val = take_value(flag)?;
                let (k, v) = val.split_once('=').ok_or_else(|| {
                    CliError::err(format!(
                        "Invalid environment variable `{val}` (expected `name=value`)."
                    ))
                })?;
                opts.has_env = true;
                opts.env_vars.push((k.to_string(), v.to_string()));
            }
            "-f" | "--format" => {
                opts.output_format = Some(take_value(flag)?);
            }
            "--module-path" => {
                let val = take_value(flag)?;
                let sep = if cfg!(windows) { ';' } else { ':' };
                let list = opts.module_path.get_or_insert_with(Vec::new);
                for entry in val.split(sep) {
                    if !entry.is_empty() {
                        list.push(PathBuf::from(entry));
                    }
                }
            }
            "--settings" => {
                opts.settings_uri = Some(take_value(flag)?);
            }
            "-t" | "--timeout" => {
                let val = take_value(flag)?;
                opts.timeout_ms = Some(parse_timeout_ms(&val)?);
            }
            "--ca-certificates" => {
                opts.ca_certificates.push(PathBuf::from(take_value(flag)?));
            }
            "--http-proxy" => {
                let val = take_value(flag)?;
                // Validate syntax eagerly matching `BaseOptions.kt:241-248`.
                pkl_runtime::host::http::ProxyAddress::parse(&val).map_err(CliError::err)?;
                opts.http_proxy = Some(val);
            }
            "--http-no-proxy" => {
                let val = take_value(flag)?;
                opts.http_no_proxy = Some(val.split(',').map(str::to_string).collect());
            }
            "--http-rewrite" => {
                let val = take_value(flag)?;
                let (from, to) = val
                    .split_once('=')
                    .ok_or_else(|| CliError::err("Rewrites must be in the form of <from>=<to>"))?;
                pkl_runtime::host::http::validate_rewrite_rule(from).map_err(CliError::err)?;
                pkl_runtime::host::http::validate_rewrite_rule(to).map_err(CliError::err)?;
                opts.http_rewrites
                    .get_or_insert_with(BTreeMap::new)
                    .insert(from.to_string(), to.to_string());
            }
            "--http-header" => {
                let val = take_value(flag)?;
                let (pattern, name, value) = parse_cli_http_header(&val)?;
                opts.http_headers
                    .get_or_insert_with(BTreeMap::new)
                    .entry(pattern)
                    .or_default()
                    .push((name, value));
            }
            "--external-module-reader" => {
                let val = take_value(flag)?;
                let (scheme, cfg) = ExternalReaderConfig::parse_cli_option(&val)
                    .map_err(|e| CliError::err(e.to_string()))?;
                opts.external_module_readers
                    .get_or_insert_with(BTreeMap::new)
                    .insert(scheme, cfg);
            }
            "--external-resource-reader" => {
                let val = take_value(flag)?;
                let (scheme, cfg) = ExternalReaderConfig::parse_cli_option(&val)
                    .map_err(|e| CliError::err(e.to_string()))?;
                opts.external_resource_readers
                    .get_or_insert_with(BTreeMap::new)
                    .insert(scheme, cfg);
            }
            "--color" => {
                let val = take_value(flag)?;
                let color = EvaluatorColor::parse(&val).ok_or_else(|| {
                    CliError::err(format!(
                        "Invalid value for `--color`: `{val}` (expected `never`, `auto`, or `always`)."
                    ))
                })?;
                opts.color = Some(color);
            }
            "--trace-mode" => {
                let val = take_value(flag)?;
                if val != "compact" && val != "pretty" {
                    return Err(CliError::err(format!(
                        "Invalid value for `--trace-mode`: `{val}` (expected `compact` or `pretty`)."
                    )));
                }
                opts.trace_mode = Some(val);
            }
            "--project-dir" => {
                opts.project_dir = Some(PathBuf::from(take_value(flag)?));
            }
            "--no-project" => {
                opts.no_project = true;
            }
            "--omit-project-settings" => {
                opts.omit_project_settings = true;
            }
            "--test-mode" => {
                opts.test_mode = true;
            }
            "--test-port" => {
                let val = take_value(flag)?;
                let port_i32 = val
                    .parse::<i32>()
                    .map_err(|e| CliError::err(format!("Invalid value for `--test-port`: {e}")))?;
                opts.test_port = if port_i32 < 0 {
                    None
                } else {
                    Some(u16::try_from(port_i32).map_err(|_| {
                        CliError::err(format!(
                            "Invalid port number `{port_i32}` for `--test-port`."
                        ))
                    })?)
                };
            }
            "--power-assertions" => {
                opts.power_assertions = true;
            }
            "--no-power-assertions" => {
                opts.power_assertions = false;
            }
            "-o" | "--output-path" => {
                opts.output_path = Some(take_value(flag)?);
            }
            "-m" | "--multiple-file-output-path" => {
                opts.multiple_file_output_path = Some(take_value(flag)?);
            }
            "-x" | "--expression" => {
                opts.expression = Some(take_value(flag)?);
            }
            "--module-output-separator" => {
                opts.module_output_separator = take_value(flag)?;
            }
            "--skip-publish-check" => {
                opts.skip_publish_check = true;
            }
            "--no-transitive" => {
                opts.no_transitive = true;
            }
            "--overwrite" => {
                opts.overwrite = true;
            }
            other => {
                return Err(CliError::err(format!("Unknown option: `{other}`.")));
            }
        }
        i += 1;
    }

    Ok((opts, positionals))
}

fn parse_property_kv(raw: &str) -> (String, String) {
    match raw.split_once('=') {
        Some((k, v)) => (k.to_string(), v.to_string()),
        None => (raw.to_string(), "true".to_string()),
    }
}

fn parse_timeout_ms(raw: &str) -> Result<u64, CliError> {
    if let Ok(secs) = raw.parse::<u64>() {
        return Ok(secs.saturating_mul(1000));
    }
    if let Ok(secs_f) = raw.parse::<f64>()
        && secs_f.is_finite()
        && secs_f >= 0.0
    {
        return Ok((secs_f * 1000.0).round() as u64);
    }
    Err(CliError::err(format!(
        "Invalid value for `--timeout`: `{raw}`."
    )))
}

fn parse_cli_http_header(spec: &str) -> Result<(String, String, String), CliError> {
    let Some((pattern, header_nv)) = spec.split_once('=') else {
        return Err(CliError::err(format!(
            "Invalid `--http-header` `{spec}` (expected `<url-pattern>=<header name>:<header value>`)."
        )));
    };
    pkl_runtime::host::http::HeaderGlobMatcher::compile(pattern).map_err(CliError::err)?;
    let Some((name, raw_val)) = header_nv.split_once(':') else {
        return Err(CliError::err(format!(
            "Header '{header_nv}' is not in 'name:value' format."
        )));
    };
    if name.is_empty() {
        return Err(CliError::err(format!(
            "Header '{header_nv}' is not in 'name:value' format."
        )));
    }
    let value = raw_val.trim_start_matches([' ', '\t']);
    pkl_runtime::host::http::validate_header_name(name).map_err(CliError::err)?;
    pkl_runtime::host::http::validate_header_value(value).map_err(CliError::err)?;
    Ok((pattern.to_string(), name.to_string(), value.to_string()))
}

// ============================================================================
// Path, URI, Settings, Project & Host Construction
// ============================================================================

fn normalize_lexical(path: &Path) -> PathBuf {
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

fn resolve_path(working_dir: &Path, p: &Path) -> PathBuf {
    let combined = if p.is_absolute() {
        p.to_path_buf()
    } else {
        working_dir.join(p)
    };
    normalize_lexical(&combined)
}

fn effective_working_dir(opts: &ParsedCliOptions) -> Result<PathBuf, CliError> {
    let cwd = std::env::current_dir()
        .map_err(|e| CliError::err(format!("Failed to determine working directory: {e}")))?;
    Ok(match &opts.working_dir {
        Some(w) => resolve_path(&cwd, w),
        None => normalize_lexical(&cwd),
    })
}

fn pkl_home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(|h| PathBuf::from(h).join(".pkl"))
}

fn is_uri_like(s: &str) -> bool {
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

#[derive(Clone, Debug)]
struct ResolvedSourceArg {
    uri: String,
    local_path: Option<PathBuf>,
    is_stdin: bool,
}

fn normalize_source_module(arg: &str, working_dir: &Path) -> Result<ResolvedSourceArg, CliError> {
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
            crate::fs::file_uri_to_path(&arg.into()).ok()
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

fn format_module_host_error(err: &ModuleHostError) -> String {
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
        ModuleHostError::Io(io) => io
            .message
            .as_ref()
            .map(|m| m.to_java_utf8())
            .unwrap_or_else(|| "I/O error.".to_string()),
        ModuleHostError::Denied(msg) | ModuleHostError::Unsupported(msg) => msg.to_java_utf8(),
        ModuleHostError::NotFound => "Module not found.".to_string(),
        ModuleHostError::NoLoader => "No module loader registered.".to_string(),
    }
}

struct LoadedSettings {
    settings: PklSettings,
    http_headers: Option<HttpHeaderMap>,
}

struct LoadedProject {
    project: Project,
    http_headers: Option<HttpHeaderMap>,
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

/// Detects local-project import cycles before evaluating `PklProject` (`Project.java:149-236`).
fn check_project_import_cycles(project_file: &Path) -> Result<(), CliError> {
    let norm = normalize_lexical(project_file);
    let root_uri =
        file_uri_from_path(&norm).map_err(|e| CliError::err(format_module_host_error(&e)))?;
    let mut graph: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut queue: Vec<PathBuf> = vec![norm];
    let mut visited: BTreeSet<PathBuf> = BTreeSet::new();

    while let Some(path) = queue.pop() {
        let canon = std::fs::canonicalize(&path).unwrap_or_else(|_| path.clone());
        if !visited.insert(canon) {
            continue;
        }
        let Ok(uri) = file_uri_from_path(&path) else {
            continue;
        };
        let Ok(src) = std::fs::read_to_string(&path) else {
            continue;
        };
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
            let target = if let Some(rest) = s.strip_prefix("file://") {
                normalize_lexical(Path::new(&pkl_runtime::packages::zip::percent_decode(rest)))
            } else if !is_uri_like(s) {
                normalize_lexical(&parent.join(s))
            } else {
                continue;
            };
            if target.is_file()
                && let Ok(target_uri) = file_uri_from_path(&target)
            {
                if !edges.contains(&target_uri) {
                    edges.push(target_uri);
                }
                queue.push(target);
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
fn load_user_settings(
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
            if let Some(rest) = uri.strip_prefix("file://") {
                normalize_lexical(Path::new(&pkl_runtime::packages::zip::percent_decode(rest)))
            } else if uri.starts_with("file:/") {
                normalize_lexical(Path::new(&pkl_runtime::packages::zip::percent_decode(
                    uri.strip_prefix("file:").unwrap_or(uri),
                )))
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
fn load_project_file(
    project_file: &Path,
    opts: &ParsedCliOptions,
    working_dir: &Path,
    host_env: &[(String, String)],
) -> Result<LoadedProject, CliError> {
    let norm = normalize_lexical(project_file);
    check_project_import_cycles(&norm)?;

    let uri = file_uri_from_path(&norm).map_err(|e| CliError::err(format_module_host_error(&e)))?;
    let text = std::fs::read_to_string(&norm).map_err(|e| {
        CliError::err(format!(
            "Cannot read PklProject file `{}`: {e}",
            norm.display()
        ))
    })?;

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

fn discover_optional_project(
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

fn load_project_dependencies_manager(
    project: &Project,
) -> Result<ProjectDependenciesManager, CliError> {
    let mut mgr = ProjectDependenciesManager::new(
        project.declared_dependencies.clone(),
        project.project_dir.clone(),
    );
    let deps_file = project.project_dir.join(PKL_PROJECT_DEPS_FILENAME);
    if deps_file.is_file() {
        let deps_uri = mgr.project_deps_file_uri();
        let text = std::fs::read_to_string(&deps_file).map_err(|e| {
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

fn build_effective_settings(
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

fn build_http_client_config(
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

fn build_ca_cert_source(opts: &ParsedCliOptions, working_dir: &Path) -> CaCertSource {
    if opts.ca_certificates.is_empty() {
        CaCertSource::bundled_default()
    } else {
        let resolved: Vec<PathBuf> = opts
            .ca_certificates
            .iter()
            .map(|p| resolve_path(working_dir, p))
            .collect();
        CaCertSource::from_files(resolved)
    }
}

fn build_native_host(
    opts: &ParsedCliOptions,
    working_dir: &Path,
    project: Option<&LoadedProject>,
    effective: &EffectiveSettings,
    effective_headers: Option<&HttpHeaderMap>,
    extra_properties: &[(String, String)],
) -> Result<NativeHost, CliError> {
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

    let ext_mod = effective.external_module_readers.clone();
    let ext_res = effective.external_resource_readers.clone();

    let project_dependencies = match project {
        Some(lp) => Some(load_project_dependencies_manager(&lp.project)?),
        None => None,
    };

    let trace_mode = match effective.trace_mode.as_str() {
        "pretty" => TraceMode::Pretty,
        _ => TraceMode::Compact,
    };

    NativeHost::new(NativeHostConfig {
        allowed_modules: Some(effective.allowed_modules.clone()),
        allowed_resources: Some(effective.allowed_resources.clone()),
        root_dir: effective.root_dir.clone(),
        module_paths: effective.module_path.clone(),
        http: http_cfg,
        ca_certs,
        properties,
        environment,
        timeout_ms: opts.timeout_ms,
        external_module_schemes: Vec::new(),
        external_resource_schemes: Vec::new(),
        external_module_readers: ext_mod,
        external_resource_readers: ext_res,
        module_cache_dir: effective.module_cache_dir.clone(),
        project_dependencies,
        test_mode: opts.test_mode,
        trace_mode,
        power_assertions: opts.power_assertions,
        editor_url_scheme: effective.editor_url_scheme.clone(),
        internal_pkl_binary_eval: false,
    })
    .map_err(CliError::err)
}

fn build_standalone_package_resolver(
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

fn read_stdin_utf8() -> Result<String, CliError> {
    let mut buf = String::new();
    io::stdin()
        .read_to_string(&mut buf)
        .map_err(|e| CliError::err(format!("Failed to read stdin: {e}")))?;
    Ok(buf)
}

// ============================================================================
// Subcommand 1: `eval`
// ============================================================================

fn infer_module_name_for_placeholder(uri: &str) -> String {
    let has_fragment = uri.starts_with("package:") || uri.starts_with("projectpackage:");
    let slice = if has_fragment {
        uri.rsplit('#').next().unwrap_or(uri)
    } else {
        uri.split(['?', '#']).next().unwrap_or(uri)
    };
    let leaf = slice.rsplit('/').next().unwrap_or(slice);
    let decoded = pkl_runtime::packages::zip::percent_decode(leaf);
    decoded.strip_suffix(".pkl").unwrap_or(&decoded).to_string()
}

fn resolve_output_template(
    working_dir: &Path,
    template: &str,
    src: &ResolvedSourceArg,
    output_format: Option<&str>,
) -> Result<PathBuf, CliError> {
    let module_name = infer_module_name_for_placeholder(&src.uri);
    let module_dir = src.local_path.as_ref().and_then(|p| {
        let parent = p.parent()?;
        let rel = relativize_paths(parent, working_dir);
        Some(if rel.is_empty() { ".".to_string() } else { rel })
    });

    let mut substituted = template.replace("%{moduleName}", &module_name);
    if let Some(fmt) = output_format {
        substituted = substituted.replace("%{outputFormat}", fmt);
    }
    if let Some(dir) = module_dir {
        substituted = substituted.replace("%{moduleDir}", &dir);
    }
    if substituted.contains("%{moduleDir}") {
        return Err(CliError::err(format!(
            "Cannot substitute output path placeholder `%{{moduleDir}}` because module `{}` does not have a file system path.",
            src.uri
        )));
    }
    Ok(resolve_path(working_dir, Path::new(&substituted)))
}

fn resolve_real_output_path(base: &Path, rel: &str) -> (PathBuf, PathBuf) {
    let mut resolved = base.to_path_buf();
    let mut real = base.to_path_buf();
    for comp in Path::new(rel).components() {
        resolved.push(comp);
        real.push(comp);
        if real.exists()
            && let Ok(canon) = std::fs::canonicalize(&real)
        {
            real = canon;
        }
    }
    (normalize_lexical(&real), normalize_lexical(&resolved))
}

fn cmd_eval(args: &[String]) -> Result<i32, CliError> {
    let (opts, positionals) = parse_options(args)?;
    if opts.multiple_file_output_path.is_some()
        && (opts.output_path.is_some() || opts.expression.is_some())
    {
        return Err(CliError::err(
            "Option `--multiple-file-output-path` is mutually exclusive with -o, --output-path and -x, --expression.",
        ));
    }
    if positionals.is_empty() {
        return Err(CliError::err(
            "Usage: pkl eval [<options>] <modules>...\n\nError: missing argument <modules>",
        ));
    }

    let working_dir = effective_working_dir(&opts)?;
    let host_env: Vec<(String, String)> = std::env::vars().collect();
    let user_settings = load_user_settings(&opts, &working_dir, &host_env)?;
    let project = discover_optional_project(&opts, &working_dir, &host_env)?;
    let (effective, effective_headers) = build_effective_settings(
        &opts,
        &working_dir,
        project.as_ref(),
        &user_settings,
        &host_env,
    );

    let mut modules: Vec<ResolvedSourceArg> = positionals
        .iter()
        .map(|arg| normalize_source_module(arg, &working_dir))
        .collect::<Result<_, _>>()?;
    modules.sort_by(|a, b| a.uri.cmp(&b.uri));

    let host = build_native_host(
        &opts,
        &working_dir,
        project.as_ref(),
        &effective,
        effective_headers.as_ref(),
        &[],
    )?;

    let eval_options = ModuleEvalOptions {
        properties: effective
            .external_properties
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect(),
        environment: effective
            .env
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect(),
        no_project: true,
        no_cache: true,
        settings: "pkl:settings".into(),
        diagnostic_profile: DiagnosticProfile::CliBundledSettings,
    };

    let result = run_eval_modes(&opts, &working_dir, &modules, &host, &eval_options);
    host.close();
    result
}

enum MultiFileError {
    Cli(String),
    Io(String),
    Eval,
}

fn run_eval_modes(
    opts: &ParsedCliOptions,
    working_dir: &Path,
    modules: &[ResolvedSourceArg],
    host: &NativeHost,
    eval_options: &ModuleEvalOptions,
) -> Result<i32, CliError> {
    let mut stdout = io::stdout().lock();
    let mut stderr = io::stderr().lock();

    // Mode 1: `-m` / `--multiple-file-output-path`
    if let Some(ref multi_template) = opts.multiple_file_output_path {
        let mut seen_uris = HashSet::new();
        let mut written_files: HashMap<PathBuf, (String, String)> = HashMap::new();
        #[cfg(windows)]
        const LINE_SEP: &str = "\r\n";
        #[cfg(not(windows))]
        const LINE_SEP: &str = "\n";

        for module in modules {
            if !seen_uris.insert(module.uri.clone()) {
                continue;
            }
            let output_dir = resolve_output_template(
                working_dir,
                multi_template,
                module,
                opts.output_format.as_deref(),
            )?;
            if output_dir.exists() && !output_dir.is_dir() {
                return Err(CliError::err(format!(
                    "Output path `{}` exists and is not a directory.",
                    output_dir.display()
                )));
            }
            let real_output_dir = if output_dir.exists() {
                std::fs::canonicalize(&output_dir).unwrap_or_else(|_| output_dir.clone())
            } else {
                output_dir.clone()
            };
            let stdin_text = if module.is_stdin {
                Some(read_stdin_utf8()?)
            } else {
                None
            };

            let outcome = host.eval_uri_output_files(
                &module.uri,
                stdin_text.as_deref(),
                eval_options,
                |path_spec, eval_bytes| {
                    if let Err(msg) = check_path_spec(path_spec, cfg!(windows)) {
                        return Err(MultiFileError::Cli(msg.to_java_utf8()));
                    }
                    let path_spec_str = path_spec.to_java_utf8();
                    let (real_path, resolved_path) =
                        resolve_real_output_path(&real_output_dir, &path_spec_str);
                    if !real_path.starts_with(&real_output_dir) {
                        return Err(MultiFileError::Cli(format!(
                            "Output file conflict: `output.files` entry `\"{path_spec_str}\"` in module `{}` resolves to file path `{}`, which is outside output directory `{}`.",
                            module.uri,
                            real_path.display(),
                            real_output_dir.display()
                        )));
                    }
                    if let Some((prev_spec, prev_uri)) = written_files.get(&real_path) {
                        return Err(MultiFileError::Cli(format!(
                            "Output file conflict: `output.files` entries `\"{prev_spec}\"` in module `{prev_uri}` and `\"{path_spec_str}\"` in module `{}` resolve to the same file path `{}`.",
                            module.uri,
                            real_path.display()
                        )));
                    }
                    if real_path.is_dir() {
                        return Err(MultiFileError::Cli(format!(
                            "Output file conflict: `output.files` entry `\"{path_spec_str}\"` in module `{}` resolves to file path `{}`, which is a directory.",
                            module.uri,
                            real_path.display()
                        )));
                    }
                    written_files.insert(real_path.clone(), (path_spec_str, module.uri.clone()));
                    if let Some(parent) = real_path.parent()
                        && !parent.as_os_str().is_empty()
                    {
                        std::fs::create_dir_all(parent)
                            .map_err(|e| MultiFileError::Io(e.to_string()))?;
                    }
                    let bytes = eval_bytes().map_err(|_| MultiFileError::Eval)?;
                    std::fs::write(&real_path, &bytes)
                        .map_err(|e| MultiFileError::Io(e.to_string()))?;
                    let relative = relativize_paths(&resolved_path, working_dir);
                    let line = format!("{relative}{LINE_SEP}");
                    stdout
                        .write_all(line.as_bytes())
                        .and_then(|()| stdout.flush())
                        .map_err(|e| MultiFileError::Io(e.to_string()))?;
                    Ok(())
                },
            );

            match outcome {
                NativeStreamingOutputFilesOutcome::Ok { stderr: eval_err } => {
                    let _ = stderr.write_all(&eval_err);
                }
                NativeStreamingOutputFilesOutcome::EvalFailure(failure) => {
                    let _ = stderr.write_all(&failure.stderr);
                    return Ok(failure.exit_code);
                }
                NativeStreamingOutputFilesOutcome::ConsumerError {
                    error,
                    stderr: eval_err,
                } => {
                    let _ = stderr.write_all(&eval_err);
                    match error {
                        MultiFileError::Cli(msg) => return Err(CliError::err(msg)),
                        MultiFileError::Io(msg) => return Err(CliError::err(msg)),
                        MultiFileError::Eval => return Ok(1),
                    }
                }
            }
        }
        return Ok(0);
    }

    // Mode 2: `-o` / `--output-path`
    if let Some(ref out_template) = opts.output_path {
        let mut seen_uris = HashSet::new();
        let mut written_non_empty: HashSet<PathBuf> = HashSet::new();
        for module in modules {
            if !seen_uris.insert(module.uri.clone()) {
                continue;
            }
            let output_file = resolve_output_template(
                working_dir,
                out_template,
                module,
                opts.output_format.as_deref(),
            )?;
            if output_file.is_dir() {
                return Err(CliError::err(format!(
                    "Output file `{}` is a directory. Did you mean `--multiple-file-output-path`?",
                    output_file.display()
                )));
            }
            let stdin_text = if module.is_stdin {
                Some(read_stdin_utf8()?)
            } else {
                None
            };
            let eval_res = host.eval_uri_output(
                &module.uri,
                stdin_text.as_deref(),
                opts.expression.as_deref(),
                eval_options,
            );
            let _ = stderr.write_all(&eval_res.stderr);
            if eval_res.exit_code != 0 {
                return Ok(eval_res.exit_code);
            }
            if let Some(parent) = output_file.parent()
                && !parent.as_os_str().is_empty()
            {
                std::fs::create_dir_all(parent)
                    .map_err(|e| CliError::err(format!("Failed to create parent dir: {e}")))?;
            }
            if !written_non_empty.contains(&output_file) {
                std::fs::write(&output_file, &eval_res.stdout).map_err(|e| {
                    CliError::err(format!("Failed to write `{}`: {e}", output_file.display()))
                })?;
                if !eval_res.stdout.is_empty() {
                    written_non_empty.insert(output_file);
                }
            } else if !eval_res.stdout.is_empty() {
                let mut file = std::fs::OpenOptions::new()
                    .append(true)
                    .open(&output_file)
                    .map_err(|e| {
                        CliError::err(format!("Failed to open `{}`: {e}", output_file.display()))
                    })?;
                file.write_all(format!("{}\n", opts.module_output_separator).as_bytes())
                    .and_then(|()| file.write_all(&eval_res.stdout))
                    .map_err(|e| {
                        CliError::err(format!("Failed to append `{}`: {e}", output_file.display()))
                    })?;
            }
        }
        return Ok(0);
    }

    // Mode 3: stdout (`CliEvaluator.writeOutput` console branch)
    let mut output_written = false;
    for module in modules {
        let stdin_text = if module.is_stdin {
            Some(read_stdin_utf8()?)
        } else {
            None
        };
        let eval_res = host.eval_uri_output(
            &module.uri,
            stdin_text.as_deref(),
            opts.expression.as_deref(),
            eval_options,
        );
        if eval_res.exit_code != 0 {
            let _ = stderr.write_all(&eval_res.stderr);
            return Ok(eval_res.exit_code);
        }
        if !eval_res.stdout.is_empty() {
            if output_written {
                let _ = stdout.write_all(format!("{}\n", opts.module_output_separator).as_bytes());
            }
            let _ = stdout.write_all(&eval_res.stdout);
            let _ = stdout.flush();
            output_written = true;
        }
        let _ = stderr.write_all(&eval_res.stderr);
    }
    Ok(0)
}

// ============================================================================
// Subcommand 2: `project resolve`
// ============================================================================

fn resolve_project_files(
    opts: &ParsedCliOptions,
    positionals: &[String],
    working_dir: &Path,
) -> Result<Vec<PathBuf>, CliError> {
    if !positionals.is_empty() {
        let mut files = Vec::with_capacity(positionals.len());
        for dir_arg in positionals {
            let dir = resolve_path(working_dir, Path::new(dir_arg));
            let file = dir.join(PKL_PROJECT_FILENAME);
            if !file.is_file() {
                return Err(CliError::err(format!(
                    "Directory {} does not contain a PklProject file.",
                    dir.display()
                )));
            }
            files.push(file);
        }
        return Ok(files);
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
        return Ok(vec![file]);
    }
    let root_dir = opts
        .root_dir
        .as_deref()
        .map(|r| resolve_path(working_dir, r));
    if let Some(found) = find_project_dir_with(working_dir, root_dir.as_deref(), |p| p.is_file()) {
        return Ok(vec![found.join(PKL_PROJECT_FILENAME)]);
    }
    Err(CliError::err(
        "No project visible to the working directory. Ensure there is a PklProject file in the workspace, or provide an explicit project directory as an argument.",
    ))
}

fn cmd_project_resolve(args: &[String]) -> Result<i32, CliError> {
    let (opts, positionals) = parse_options(args)?;
    let working_dir = effective_working_dir(&opts)?;
    let host_env: Vec<(String, String)> = std::env::vars().collect();
    let user_settings = load_user_settings(&opts, &working_dir, &host_env)?;
    let project_files = resolve_project_files(&opts, &positionals, &working_dir)?;

    for project_file in project_files {
        let loaded = load_project_file(&project_file, &opts, &working_dir, &host_env)?;
        let (effective, effective_headers) = build_effective_settings(
            &opts,
            &working_dir,
            Some(&loaded),
            &user_settings,
            &host_env,
        );
        let (resolver, http) = build_standalone_package_resolver(
            &opts,
            &working_dir,
            &effective,
            effective_headers.as_ref(),
        )?;
        let resolved = ProjectDependenciesResolver::new(&loaded.project, &resolver)
            .resolve()
            .map_err(|e| {
                resolver.close();
                http.close();
                CliError::err(format_module_host_error(&e))
            })?;
        // Upstream Pkl 0.32.0 `ProjectDependenciesResolver.java:71-75` writes override
        // warnings to `errWriter`, which `CliProjectResolver.kt:32,48-55` defaults to a
        // buffered `OutputStreamWriter(System.err)` and never flushes (`consoleWriter.flush()`
        // only), so `org.pkl.cli.Main project resolve` exits with 0 stderr bytes. Preserve
        // `resolved.warnings` in the library API while matching Pkl 0.32.0 CLI raw bytes here.
        let _ = &resolved.warnings;
        let deps_file = project_file
            .parent()
            .unwrap_or(&loaded.project.project_dir)
            .join(PKL_PROJECT_DEPS_FILENAME);
        let json = resolved.deps.write_json();
        std::fs::write(&deps_file, json.as_bytes()).map_err(|e| {
            resolver.close();
            http.close();
            CliError::err(format!("Failed to write to {}: {e}", deps_file.display()))
        })?;
        println!("{}", deps_file.display());
        resolver.close();
        http.close();
    }
    Ok(0)
}

// ============================================================================
// Subcommand 3: `project package`
// ============================================================================

fn cmd_project_package(args: &[String]) -> Result<i32, CliError> {
    let (opts, positionals) = parse_options(args)?;
    let working_dir = effective_working_dir(&opts)?;
    let host_env: Vec<(String, String)> = std::env::vars().collect();
    let user_settings = load_user_settings(&opts, &working_dir, &host_env)?;
    let project_files = resolve_project_files(&opts, &positionals, &working_dir)?;
    let output_pattern = opts
        .output_path
        .clone()
        .unwrap_or_else(|| ".out/%{name}@%{version}".to_string());

    let mut loaded_projects = Vec::with_capacity(project_files.len());
    for project_file in project_files {
        let loaded = load_project_file(&project_file, &opts, &working_dir, &host_env)?;
        let Some(ref pkg) = loaded.project.package else {
            return Err(CliError::err(format!(
                "No package was declared in project `{}`.\n\nAdd a `package` section to the PklProject file.",
                loaded.project.project_file_uri
            )));
        };

        if !pkg.api_tests.is_empty() {
            let (effective, effective_headers) = build_effective_settings(
                &opts,
                &working_dir,
                Some(&loaded),
                &user_settings,
                &host_env,
            );
            let test_host = build_native_host(
                &opts,
                &working_dir,
                Some(&loaded),
                &effective,
                effective_headers.as_ref(),
                &[],
            )?;
            let mut api_test_files: Vec<(String, PathBuf)> = pkg
                .api_tests
                .iter()
                .map(|t| {
                    let norm_path = normalize_lexical(&loaded.project.project_dir.join(t));
                    let raw_uri = file_uri_from_path(&norm_path)
                        .map_err(|e| CliError::err(format_module_host_error(&e)))?;
                    // Mirrors `CliProjectPackager.kt:41`: `project.projectDir.resolve(it).toUri()`
                    // passes unnormalized `./`/`../` segments to `URI.normalize()`, which in JDK 21
                    // drops the empty `//` authority (`file:///a/./b` -> `file:/a/b`).
                    let has_dot_segment = Path::new(t)
                        .components()
                        .any(|c| matches!(c, Component::CurDir | Component::ParentDir));
                    let uri = if has_dot_segment {
                        raw_uri
                            .strip_prefix("file:///")
                            .map(|rest| format!("file:/{rest}"))
                            .unwrap_or(raw_uri)
                    } else {
                        raw_uri
                    };
                    Ok((uri, norm_path))
                })
                .collect::<Result<_, CliError>>()?;
            api_test_files.sort_by(|a, b| a.0.cmp(&b.0));

            let mut all_results = Vec::new();
            let mut failed = false;
            for (idx, (test_uri, test_file)) in api_test_files.iter().enumerate() {
                let res = run_single_test_module(
                    test_file,
                    Some(test_uri.as_str()),
                    opts.overwrite,
                    &test_host,
                )?;
                if !res.logs.is_empty() {
                    let mut stderr = io::stderr().lock();
                    let _ = stderr.write_all(res.logs.as_bytes());
                    let _ = stderr.flush();
                }
                let report = render_spec_report(&res);
                print!("{report}");
                if !report.is_empty() && api_test_files.len() > 1 && idx + 1 != api_test_files.len()
                {
                    println!();
                }
                if res.failed() {
                    failed = true;
                }
                all_results.push(res);
            }
            test_host.close();
            println!();
            print!("{}", render_spec_summary(&all_results));
            if failed {
                return Err(CliError::err(format!(
                    "Failed to create package `{}`, because its API tests are failing.",
                    pkg.uri
                )));
            }
        }
        loaded_projects.push(loaded);
    }

    let projects: Vec<Project> = loaded_projects
        .iter()
        .map(|lp| lp.project.clone())
        .collect();
    let (effective, effective_headers) = build_effective_settings(
        &opts,
        &working_dir,
        loaded_projects.first(),
        &user_settings,
        &host_env,
    );
    let http_cfg = build_http_client_config(&opts, &effective, effective_headers.as_ref())?;
    let ca_certs = build_ca_cert_source(&opts, &working_dir);
    let http = Arc::new(NativeHttpClient::new(http_cfg, ca_certs, CancelFlag::new()));
    let policy = Arc::new(
        SecurityPolicy::new(
            Some(&effective.allowed_modules),
            Some(&effective.allowed_resources),
            None,
        )
        .map_err(CliError::err)?,
    );
    let packager_host = NativePackagerHost::new(http.clone(), policy);

    let result = ProjectPackager::new(
        &projects,
        working_dir,
        output_pattern,
        opts.skip_publish_check,
        &packager_host,
    )
    .create_packages();
    http.close();

    let (_, stdout_lines) = result.map_err(|e| CliError::err(format_module_host_error(&e)))?;
    for line in stdout_lines {
        println!("{line}");
    }
    Ok(0)
}

// ============================================================================
// Subcommand 4: `download-package` (`CliPackageDownloader.kt:24-63`)
// ============================================================================

fn cmd_download_package(args: &[String]) -> Result<i32, CliError> {
    let (opts, positionals) = parse_options(args)?;
    if positionals.is_empty() {
        return Err(CliError::err(
            "Usage: pkl download-package [<options>] <package>...\n\nError: missing argument <package>",
        ));
    }
    let working_dir = effective_working_dir(&opts)?;
    let host_env: Vec<(String, String)> = std::env::vars().collect();
    let user_settings = load_user_settings(&opts, &working_dir, &host_env)?;
    let (effective, effective_headers) =
        build_effective_settings(&opts, &working_dir, None, &user_settings, &host_env);

    // Exact upstream message (`CliPackageDownloader.kt:31-33`)!
    if effective.module_cache_dir.is_none() {
        return Err(CliError::err(
            "Cannot download packages because no cache directory is specified.",
        ));
    }

    let mut package_uris = Vec::with_capacity(positionals.len());
    for raw in &positionals {
        let pkg = PackageUri::parse(raw.as_str())
            .map_err(|e| CliError::err(e.reason().to_java_utf8()))?;
        package_uris.push(pkg);
    }

    let (resolver, http) = build_standalone_package_resolver(
        &opts,
        &working_dir,
        &effective,
        effective_headers.as_ref(),
    )?;

    let mut errors: Vec<(PackageUri, String)> = Vec::new();
    for pkg in package_uris {
        if let Err(err) = resolver.download_package(&pkg, pkg.sha256.as_ref(), opts.no_transitive) {
            errors.push((pkg, format_module_host_error(&err)));
        }
    }
    resolver.close();
    http.close();

    match errors.len() {
        0 => Ok(0),
        1 => Err(CliError::err(errors.remove(0).1)),
        _ => {
            let mut msg = String::from("Failed to download some packages.\n");
            for (uri, err) in errors {
                msg.push('\n');
                msg.push_str(&format!("Failed to download {uri} because:\n"));
                msg.push_str(&format!("{err}\n"));
            }
            Err(CliError::err(msg))
        }
    }
}

// ============================================================================
// Subcommand 5: `analyze imports` (`CliImportAnalyzer.kt:25-80`)
// ============================================================================

fn cmd_analyze_imports(args: &[String]) -> Result<i32, CliError> {
    let (opts, positionals) = parse_options(args)?;
    if positionals.is_empty() {
        return Err(CliError::err(
            "Usage: pkl analyze imports [<options>] <modules>...\n\nError: missing argument <modules>",
        ));
    }
    let working_dir = effective_working_dir(&opts)?;
    let host_env: Vec<(String, String)> = std::env::vars().collect();
    let user_settings = load_user_settings(&opts, &working_dir, &host_env)?;
    let project = discover_optional_project(&opts, &working_dir, &host_env)?;
    let (effective, effective_headers) = build_effective_settings(
        &opts,
        &working_dir,
        project.as_ref(),
        &user_settings,
        &host_env,
    );

    let mut modules: Vec<ResolvedSourceArg> = positionals
        .iter()
        .map(|arg| normalize_source_module(arg, &working_dir))
        .collect::<Result<_, _>>()?;
    modules.sort_by(|a, b| a.uri.cmp(&b.uri));

    let extra_props: Vec<(String, String)> = modules
        .iter()
        .enumerate()
        .map(|(idx, m)| (format!("pkl.analyzeImports.{idx}"), m.uri.clone()))
        .collect();

    let host = build_native_host(
        &opts,
        &working_dir,
        project.as_ref(),
        &effective,
        effective_headers.as_ref(),
        &extra_props,
    )?;

    let mut eval_props: Vec<(String, String)> = effective
        .external_properties
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    eval_props.extend(extra_props);

    let eval_options = ModuleEvalOptions {
        properties: eval_props,
        environment: effective
            .env
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect(),
        no_project: true,
        no_cache: true,
        settings: "pkl:settings".into(),
        diagnostic_profile: DiagnosticProfile::CliBundledSettings,
    };

    // Upstream `CliImportAnalyzer.kt:74` calls `evaluator.evaluateOutputText(sourceModule)`,
    // which evaluates `output.text` directly (no `Module.output.bytes` stack frame).
    let res = host.eval_uri_output(
        "repl:text",
        Some(ANALYZE_IMPORTS_MODULE),
        Some("output.text"),
        &eval_options,
    );
    host.close();

    if !res.stderr.is_empty() {
        let _ = io::stderr().write_all(&res.stderr);
    }
    if res.exit_code != 0 {
        return Ok(res.exit_code);
    }

    if let Some(ref out_path) = opts.output_path {
        let resolved = resolve_path(&working_dir, Path::new(out_path));
        if let Some(parent) = resolved.parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent)
                .map_err(|e| CliError::err(format!("Failed to create output directory: {e}")))?;
        }
        std::fs::write(&resolved, &res.stdout)
            .map_err(|e| CliError::err(format!("Failed to write `{}`: {e}", resolved.display())))?;
    } else {
        let mut stdout = io::stdout().lock();
        let _ = stdout.write_all(&res.stdout);
        let _ = stdout.flush();
    }
    Ok(0)
}

// ============================================================================
// Subcommand 6: `test` (`CliTestRunner.kt:31-141`)
// ============================================================================

const RENDER_FACTS_JSON_EXPR: &str =
    "if (facts == null) \"null\" else new JsonRenderer {}.renderValue(facts)";

const RENDER_FACTS_LENGTHS_JSON_EXPR: &str = "if (facts == null) \"null\" else new JsonRenderer {}.renderValue(facts.toMap().mapValues((_, g) -> g.length))";

const RENDER_EXAMPLES_JSON_EXPR: &str = r#"
let (r = new PcfRenderer { omitNullProperties = false })
new JsonRenderer {}.renderValue(
  if (examples == null) null
  else examples.toMap().mapValues((_, g) ->
    g.toList().map((v) ->
      let (s = r.renderValue(v))
      if (v is Typed || v is Dynamic || v is Listing || v is Mapping) "new " + s else s
    )
  )
)
"#;

const RENDER_EXAMPLES_LENGTHS_JSON_EXPR: &str = "if (examples == null) \"null\" else new JsonRenderer {}.renderValue(examples.toMap().mapValues((_, g) -> g.length))";

#[derive(Debug)]
enum MiniJson {
    Null,
    Bool(bool),
    Int(i64),
    Str(String),
    Array(Vec<MiniJson>),
    Object(Vec<(String, MiniJson)>),
}

struct MiniJsonParser<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> MiniJsonParser<'a> {
    fn new(input: &'a str) -> Self {
        Self {
            bytes: input.as_bytes(),
            pos: 0,
        }
    }

    fn skip_ws(&mut self) {
        while self.pos < self.bytes.len()
            && matches!(self.bytes[self.pos], b' ' | b'\t' | b'\r' | b'\n')
        {
            self.pos += 1;
        }
    }

    fn parse_value(&mut self) -> Result<MiniJson, String> {
        self.skip_ws();
        let Some(&b) = self.bytes.get(self.pos) else {
            return Err("Unexpected EOF in JSON".to_string());
        };
        match b {
            b'n' => {
                if self.bytes[self.pos..].starts_with(b"null") {
                    self.pos += 4;
                    Ok(MiniJson::Null)
                } else {
                    Err("Invalid JSON token".to_string())
                }
            }
            b't' => {
                if self.bytes[self.pos..].starts_with(b"true") {
                    self.pos += 4;
                    Ok(MiniJson::Bool(true))
                } else {
                    Err("Invalid JSON token".to_string())
                }
            }
            b'f' => {
                if self.bytes[self.pos..].starts_with(b"false") {
                    self.pos += 5;
                    Ok(MiniJson::Bool(false))
                } else {
                    Err("Invalid JSON token".to_string())
                }
            }
            b'"' => self.parse_string().map(MiniJson::Str),
            b'[' => self.parse_array(),
            b'{' => self.parse_object(),
            b'-' | b'0'..=b'9' => self.parse_int(),
            _ => Err(format!("Unexpected JSON byte 0x{b:02x}")),
        }
    }

    fn parse_int(&mut self) -> Result<MiniJson, String> {
        let start = self.pos;
        if self.bytes.get(self.pos) == Some(&b'-') {
            self.pos += 1;
        }
        while self.pos < self.bytes.len() && self.bytes[self.pos].is_ascii_digit() {
            self.pos += 1;
        }
        let s = std::str::from_utf8(&self.bytes[start..self.pos])
            .map_err(|e| format!("Invalid JSON number: {e}"))?;
        let n = s
            .parse::<i64>()
            .map_err(|e| format!("Invalid JSON integer `{s}`: {e}"))?;
        Ok(MiniJson::Int(n))
    }

    fn parse_hex4(&mut self) -> Result<u16, String> {
        if self.pos + 4 > self.bytes.len() {
            return Err("Truncated \\u escape in JSON".to_string());
        }
        let hex = std::str::from_utf8(&self.bytes[self.pos..self.pos + 4])
            .map_err(|_| "Invalid UTF-8 in \\u escape".to_string())?;
        self.pos += 4;
        u16::from_str_radix(hex, 16).map_err(|_| format!("Invalid hex escape `\\u{hex}`"))
    }

    fn parse_string(&mut self) -> Result<String, String> {
        if self.bytes.get(self.pos) != Some(&b'"') {
            return Err("Expected `\"`".to_string());
        }
        self.pos += 1;
        let mut out = String::new();
        while self.pos < self.bytes.len() {
            let b = self.bytes[self.pos];
            if b == b'"' {
                self.pos += 1;
                return Ok(out);
            }
            if b == b'\\' {
                self.pos += 1;
                let Some(&esc) = self.bytes.get(self.pos) else {
                    return Err("Truncated escape in JSON string".to_string());
                };
                self.pos += 1;
                match esc {
                    b'"' => out.push('"'),
                    b'\\' => out.push('\\'),
                    b'/' => out.push('/'),
                    b'b' => out.push('\u{0008}'),
                    b'f' => out.push('\u{000C}'),
                    b'n' => out.push('\n'),
                    b'r' => out.push('\r'),
                    b't' => out.push('\t'),
                    b'u' => {
                        let code = self.parse_hex4()?;
                        if (0xD800..=0xDBFF).contains(&code)
                            && self.bytes.get(self.pos..self.pos + 2) == Some(b"\\u")
                        {
                            self.pos += 2;
                            let low = self.parse_hex4()?;
                            if (0xDC00..=0xDFFF).contains(&low) {
                                let cp = 0x10000
                                    + (((u32::from(code) - 0xD800) << 10)
                                        | (u32::from(low) - 0xDC00));
                                if let Some(ch) = char::from_u32(cp) {
                                    out.push(ch);
                                } else {
                                    out.push('\u{FFFD}');
                                }
                            } else {
                                out.push('\u{FFFD}');
                            }
                        } else if let Some(ch) = char::from_u32(u32::from(code)) {
                            out.push(ch);
                        } else {
                            out.push('\u{FFFD}');
                        }
                    }
                    other => return Err(format!("Invalid JSON escape `\\{}`", other as char)),
                }
            } else {
                let rest = std::str::from_utf8(&self.bytes[self.pos..])
                    .map_err(|_| "Invalid UTF-8 in JSON string".to_string())?;
                let ch = rest.chars().next().ok_or("Unexpected EOF in JSON string")?;
                out.push(ch);
                self.pos += ch.len_utf8();
            }
        }
        Err("Unterminated JSON string".to_string())
    }

    fn parse_array(&mut self) -> Result<MiniJson, String> {
        self.pos += 1; // '['
        let mut items = Vec::new();
        self.skip_ws();
        if self.bytes.get(self.pos) == Some(&b']') {
            self.pos += 1;
            return Ok(MiniJson::Array(items));
        }
        loop {
            items.push(self.parse_value()?);
            self.skip_ws();
            match self.bytes.get(self.pos) {
                Some(&b',') => self.pos += 1,
                Some(&b']') => {
                    self.pos += 1;
                    return Ok(MiniJson::Array(items));
                }
                _ => return Err("Expected `,` or `]` in JSON array".to_string()),
            }
        }
    }

    fn parse_object(&mut self) -> Result<MiniJson, String> {
        self.pos += 1; // '{'
        let mut entries = Vec::new();
        self.skip_ws();
        if self.bytes.get(self.pos) == Some(&b'}') {
            self.pos += 1;
            return Ok(MiniJson::Object(entries));
        }
        loop {
            self.skip_ws();
            let key = self.parse_string()?;
            self.skip_ws();
            if self.bytes.get(self.pos) != Some(&b':') {
                return Err("Expected `:` in JSON object".to_string());
            }
            self.pos += 1;
            let val = self.parse_value()?;
            entries.push((key, val));
            self.skip_ws();
            match self.bytes.get(self.pos) {
                Some(&b',') => self.pos += 1,
                Some(&b'}') => {
                    self.pos += 1;
                    return Ok(MiniJson::Object(entries));
                }
                _ => return Err("Expected `,` or `}` in JSON object".to_string()),
            }
        }
    }
}

fn parse_mini_json(input: &str) -> Result<MiniJson, String> {
    let mut parser = MiniJsonParser::new(input);
    parser.parse_value()
}

fn mini_json_to_facts_decoded(val: &MiniJson) -> Option<DecodedValue> {
    match val {
        MiniJson::Null => None,
        MiniJson::Object(entries) => {
            let mut map = Vec::with_capacity(entries.len());
            for (k, v) in entries {
                let items = match v {
                    MiniJson::Array(arr) => arr
                        .iter()
                        .map(|elem| match elem {
                            MiniJson::Bool(b) => DecodedValue::Bool(*b),
                            _ => DecodedValue::Null,
                        })
                        .collect(),
                    _ => Vec::new(),
                };
                map.push((
                    DecodedValue::String(k.clone().into()),
                    DecodedValue::Listing(items),
                ));
            }
            Some(DecodedValue::Mapping(map))
        }
        _ => None,
    }
}

type ExampleGroups = Vec<(String, Vec<String>)>;

fn mini_json_to_examples_map(val: &MiniJson) -> ExampleGroups {
    let MiniJson::Object(entries) = val else {
        return Vec::new();
    };
    let mut out = Vec::with_capacity(entries.len());
    for (k, v) in entries {
        let items = match v {
            MiniJson::Array(arr) => arr
                .iter()
                .filter_map(|elem| match elem {
                    MiniJson::Str(s) => Some(s.clone()),
                    _ => None,
                })
                .collect(),
            _ => Vec::new(),
        };
        out.push((k.clone(), items));
    }
    out
}

fn mini_json_to_group_lengths(val: &MiniJson) -> Vec<(String, usize)> {
    let MiniJson::Object(entries) = val else {
        return Vec::new();
    };
    entries
        .iter()
        .map(|(k, v)| {
            let len = match v {
                MiniJson::Int(n) if *n >= 0 => *n as usize,
                _ => 0,
            };
            (k.clone(), len)
        })
        .collect()
}

fn pkl_escape_string_literal(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
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
    out
}

/// Removes trailing synthetic `repl:text` probe frames appended when evaluating
/// individual `facts[k][i]` / `examples[k][i]` test expressions (`TestRunner.java:124-129,223-228`).
fn strip_trailing_repl_frames(rendered: &str) -> String {
    let trimmed = rendered.trim_end_matches('\n');
    let mut blocks: Vec<&str> = trimmed.split("\n\n").collect();
    while blocks.len() > 1 {
        let last = blocks[blocks.len() - 1];
        if last
            .lines()
            .last()
            .is_some_and(|l| l.ends_with("(repl:text)"))
        {
            blocks.pop();
        } else {
            break;
        }
    }
    blocks.join("\n\n")
}

fn first_error_headline(rendered: &str) -> String {
    rendered
        .lines()
        .find(|line| !line.is_empty() && !line.starts_with("–– Pkl Error ––"))
        .unwrap_or(rendered)
        .to_string()
}

fn examples_differ(
    actual: &[(String, Vec<String>)],
    expected: Option<&[(String, Vec<String>)]>,
) -> bool {
    let Some(expected) = expected else {
        return true;
    };
    if actual.len() != expected.len() {
        return true;
    }
    for (name, act_vals) in actual {
        let Some((_, exp_vals)) = expected.iter().find(|(k, _)| k == name) else {
            return true;
        };
        if act_vals != exp_vals {
            return true;
        }
    }
    false
}

fn parse_module_header_info(text: &str, expected_parent_uri: &str) -> (bool, Option<String>) {
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

fn run_single_test_module(
    test_path: &Path,
    override_uri: Option<&str>,
    overwrite: bool,
    shared_host: &NativeHost,
) -> Result<TestResults, CliError> {
    let norm = normalize_lexical(test_path);
    let uri = match override_uri {
        Some(u) => u.to_string(),
        None => {
            file_uri_from_path(&norm).map_err(|e| CliError::err(format_module_host_error(&e)))?
        }
    };
    let inferred_name = norm
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("test")
        .to_string();

    let test_text = match std::fs::read_to_string(&norm) {
        Ok(t) => t,
        Err(err) => {
            let msg = format!("Cannot read `{}`: {err}", norm.display());
            return Ok(TestResults {
                module_name: inferred_name,
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
    };

    // Fast path: when the module directly amends/extends `"pkl:test"`, skip importing
    // `"pkl:reflect"` (saves ~5s per test module) and use the parsed module header name
    // (or file stem). Fall back to `"pkl:reflect"` only for indirect inheritance or
    // non-`pkl:test` modules.
    let (direct_test_module, declared_name) = parse_module_header_info(&test_text, "pkl:test");
    let effective_name = declared_name.unwrap_or_else(|| inferred_name.clone());

    let exprs = if direct_test_module {
        vec![
            "true".to_string(),
            format!("\"{}\"", pkl_escape_string_literal(&effective_name)),
            RENDER_FACTS_JSON_EXPR.to_string(),
            RENDER_EXAMPLES_JSON_EXPR.to_string(),
        ]
    } else {
        vec![
            "let (ref = import(\"pkl:reflect\")) ref.Class(module.getClass()).isSubclassOf(ref.Class(import(\"pkl:test\").getClass()))".to_string(),
            "(import(\"pkl:reflect\")).Module(module).name".to_string(),
            RENDER_FACTS_JSON_EXPR.to_string(),
            RENDER_EXAMPLES_JSON_EXPR.to_string(),
        ]
    };

    let (vals, mut logs) =
        match shared_host.evaluate_expressions_partial(&uri, &test_text, &inferred_name, &exprs) {
            Ok(v) => v,
            Err(err) => {
                return Ok(TestResults {
                    module_name: effective_name,
                    display_uri: uri,
                    facts: TestSectionResults::empty(TestSectionName::Facts),
                    examples: TestSectionResults::empty(TestSectionName::Examples),
                    logs: String::new(),
                    error: Some(TestError {
                        message: first_error_headline(&err),
                        rendered: err,
                    }),
                    generated_expected_pcf: None,
                    generated_actual_pcf: None,
                });
            }
        };

    let module_name = match &vals[1] {
        Ok(val) => SourceSession::result_string(val.clone())
            .map(|s| s.to_java_utf8())
            .unwrap_or(effective_name),
        Err(err) => {
            let cleaned = strip_trailing_repl_frames(err);
            return Ok(TestResults {
                module_name: effective_name,
                display_uri: uri,
                facts: TestSectionResults::empty(TestSectionName::Facts),
                examples: TestSectionResults::empty(TestSectionName::Examples),
                logs,
                error: Some(TestError {
                    message: first_error_headline(&cleaned),
                    rendered: cleaned,
                }),
                generated_expected_pcf: None,
                generated_actual_pcf: None,
            });
        }
    };

    let is_test = match &vals[0] {
        Ok(val) => value_is_true(val),
        Err(err) => {
            let cleaned = strip_trailing_repl_frames(err);
            return Ok(TestResults {
                module_name,
                display_uri: uri,
                facts: TestSectionResults::empty(TestSectionName::Facts),
                examples: TestSectionResults::empty(TestSectionName::Examples),
                logs,
                error: Some(TestError {
                    message: first_error_headline(&cleaned),
                    rendered: cleaned,
                }),
                generated_expected_pcf: None,
                generated_actual_pcf: None,
            });
        }
    };

    if !is_test {
        let msg = format!("Module `{uri}` does not amend `pkl:test`.");
        return Ok(TestResults {
            module_name,
            display_uri: uri,
            facts: TestSectionResults::empty(TestSectionName::Facts),
            examples: TestSectionResults::empty(TestSectionName::Examples),
            logs,
            error: Some(TestError {
                message: msg.clone(),
                rendered: msg,
            }),
            generated_expected_pcf: None,
            generated_actual_pcf: None,
        });
    }

    // 1. Evaluate `facts` (fast path: bulk JSON; fallback: per-fact error isolation).
    let facts = match &vals[2] {
        Ok(val) => {
            let json_str = SourceSession::result_string(val.clone())
                .map(|s| s.to_java_utf8())
                .map_err(|e| CliError::err(format!("Failed to read facts JSON: {e:?}")))?;
            let parsed = parse_mini_json(&json_str)
                .map_err(|e| CliError::err(format!("Failed to parse facts JSON: {e}")))?;
            let decoded = mini_json_to_facts_decoded(&parsed);
            evaluate_facts_decoded_with_source(decoded.as_ref(), &uri, Some(&test_text))
        }
        Err(_) => evaluate_facts_isolated(shared_host, &uri, &test_text, &module_name)?,
    };

    // 2. Prepare expected/actual files for `examples`.
    let file_name = norm
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("test.pkl");
    let expected_file = norm.with_file_name(format!("{file_name}-expected.pcf"));
    let actual_file = norm.with_file_name(format!("{file_name}-actual.pcf"));
    let _ = std::fs::remove_file(&actual_file);
    if overwrite {
        let _ = std::fs::remove_file(&expected_file);
    }

    let expected_uri = file_uri_from_path(&expected_file).unwrap_or_default();
    let actual_uri = file_uri_from_path(&actual_file).unwrap_or_default();

    let load_expected_groups = || -> Result<Option<ExampleGroups>, CliError> {
        if !expected_file.is_file() {
            return Ok(None);
        }
        let exp_text = std::fs::read_to_string(&expected_file).map_err(|e| {
            CliError::err(format!("Failed to read `{}`: {e}", expected_file.display()))
        })?;
        let exp_exprs = vec![RENDER_EXAMPLES_JSON_EXPR.to_string()];
        let (exp_vals, _) = shared_host
            .evaluate_expressions(&expected_uri, &exp_text, "expected", &exp_exprs)
            .map_err(CliError::err)?;
        let exp_json = SourceSession::result_string(exp_vals[0].clone())
            .map(|s| s.to_java_utf8())
            .map_err(|e| CliError::err(format!("Failed to read expected examples JSON: {e:?}")))?;
        let parsed = parse_mini_json(&exp_json)
            .map_err(|e| CliError::err(format!("Failed to parse expected examples JSON: {e}")))?;
        Ok(Some(mini_json_to_examples_map(&parsed)))
    };

    // 3. Evaluate `examples` (fast path: bulk JSON + lazy doc only on write/mismatch;
    //    fallback: per-example error isolation matching `TestRunner.java:220-229`).
    let (examples, gen_expected, gen_actual) = match &vals[3] {
        Ok(val) => {
            let ex_json = SourceSession::result_string(val.clone())
                .map(|s| s.to_java_utf8())
                .map_err(|e| CliError::err(format!("Failed to read examples JSON: {e:?}")))?;
            let parsed = parse_mini_json(&ex_json)
                .map_err(|e| CliError::err(format!("Failed to parse examples JSON: {e}")))?;
            let actual_examples = mini_json_to_examples_map(&parsed);
            let expected_groups = if !actual_examples.is_empty() {
                load_expected_groups()?
            } else {
                None
            };
            // Only evaluate `RENDER_EXAMPLES_DOC_EXPR` when writing `-expected.pcf` or `-actual.pcf`.
            let full_doc = if !actual_examples.is_empty()
                && examples_differ(&actual_examples, expected_groups.as_deref())
            {
                let doc_exprs = vec![RENDER_EXAMPLES_DOC_EXPR.to_string()];
                shared_host
                    .evaluate_expressions(&uri, &test_text, &module_name, &doc_exprs)
                    .ok()
                    .and_then(|(v, _)| SourceSession::result_string(v[0].clone()).ok())
                    .map(|s| s.to_java_utf8())
                    .filter(|s| !s.is_empty())
            } else {
                None
            };
            evaluate_examples_rendered(
                &actual_examples,
                expected_groups.as_deref(),
                &uri,
                &expected_uri,
                &actual_uri,
                full_doc,
            )
        }
        Err(_) => {
            let expected_groups = load_expected_groups()?;
            let (section, extra_logs) = evaluate_examples_isolated(
                shared_host,
                &uri,
                &test_text,
                &module_name,
                expected_groups.as_deref(),
                &expected_uri,
                &actual_uri,
            )?;
            if logs.is_empty() && !extra_logs.is_empty() {
                logs = extra_logs;
            }
            (section, None, None)
        }
    };

    if let Some(ref content) = gen_expected {
        std::fs::write(&expected_file, content).map_err(|e| {
            CliError::err(format!(
                "Failed to write `{}`: {e}",
                expected_file.display()
            ))
        })?;
    }
    if let Some(ref content) = gen_actual {
        std::fs::write(&actual_file, content).map_err(|e| {
            CliError::err(format!("Failed to write `{}`: {e}", actual_file.display()))
        })?;
    }

    Ok(TestResults {
        module_name,
        display_uri: uri,
        facts,
        examples,
        logs,
        error: None,
        generated_expected_pcf: gen_expected,
        generated_actual_pcf: gen_actual,
    })
}

fn evaluate_facts_isolated(
    shared_host: &NativeHost,
    uri: &str,
    test_text: &str,
    module_name: &str,
) -> Result<TestSectionResults, CliError> {
    let (len_vals, _) = shared_host
        .evaluate_expressions(
            uri,
            test_text,
            module_name,
            &[RENDER_FACTS_LENGTHS_JSON_EXPR.to_string()],
        )
        .map_err(CliError::err)?;
    let len_json = SourceSession::result_string(len_vals[0].clone())
        .map(|s| s.to_java_utf8())
        .map_err(|e| CliError::err(format!("Failed to read facts lengths JSON: {e:?}")))?;
    let parsed_lens = parse_mini_json(&len_json)
        .map_err(|e| CliError::err(format!("Failed to parse facts lengths JSON: {e}")))?;
    let groups = mini_json_to_group_lengths(&parsed_lens);
    if groups.is_empty() {
        return Ok(TestSectionResults::empty(TestSectionName::Facts));
    }

    let mut exprs = Vec::new();
    for (group, len) in &groups {
        let esc = pkl_escape_string_literal(group);
        for idx in 0..*len {
            exprs.push(format!("facts[\"{esc}\"][{idx}]"));
        }
    }
    let (vals, _) = shared_host
        .evaluate_expressions_partial(uri, test_text, module_name, &exprs)
        .map_err(CliError::err)?;

    let mut decoded_entries = Vec::with_capacity(groups.len());
    let mut group_errors: Vec<Vec<TestError>> = Vec::with_capacity(groups.len());
    let mut cursor = 0usize;
    for (group, len) in &groups {
        let mut bools = Vec::with_capacity(*len);
        let mut errs = Vec::new();
        for _ in 0..*len {
            match &vals[cursor] {
                Ok(val) => bools.push(DecodedValue::Bool(value_is_true(val))),
                Err(err) => {
                    bools.push(DecodedValue::Bool(true));
                    let cleaned = strip_trailing_repl_frames(err);
                    errs.push(TestError {
                        message: first_error_headline(&cleaned),
                        rendered: cleaned,
                    });
                }
            }
            cursor += 1;
        }
        decoded_entries.push((
            DecodedValue::String(group.clone().into()),
            DecodedValue::Listing(bools),
        ));
        group_errors.push(errs);
    }

    let decoded = DecodedValue::Mapping(decoded_entries);
    let mut section = evaluate_facts_decoded_with_source(Some(&decoded), uri, Some(test_text));
    for (res, errs) in section.results.iter_mut().zip(group_errors) {
        res.errors.extend(errs);
    }
    Ok(section)
}

fn evaluate_examples_isolated(
    shared_host: &NativeHost,
    uri: &str,
    test_text: &str,
    module_name: &str,
    expected_groups: Option<&[(String, Vec<String>)]>,
    expected_uri: &str,
    actual_uri: &str,
) -> Result<(TestSectionResults, String), CliError> {
    let (len_vals, _) = shared_host
        .evaluate_expressions(
            uri,
            test_text,
            module_name,
            &[RENDER_EXAMPLES_LENGTHS_JSON_EXPR.to_string()],
        )
        .map_err(CliError::err)?;
    let len_json = SourceSession::result_string(len_vals[0].clone())
        .map(|s| s.to_java_utf8())
        .map_err(|e| CliError::err(format!("Failed to read examples lengths JSON: {e:?}")))?;
    let parsed_lens = parse_mini_json(&len_json)
        .map_err(|e| CliError::err(format!("Failed to parse examples lengths JSON: {e}")))?;
    let groups = mini_json_to_group_lengths(&parsed_lens);
    if groups.is_empty() {
        return Ok((
            TestSectionResults::empty(TestSectionName::Examples),
            String::new(),
        ));
    }

    let mut exprs = Vec::new();
    for (group, len) in &groups {
        let esc = pkl_escape_string_literal(group);
        for idx in 0..*len {
            exprs.push(format!(
                "let (r = new PcfRenderer {{ omitNullProperties = false }}) let (v = examples[\"{esc}\"][{idx}]) let (s = r.renderValue(v)) if (v is Typed || v is Dynamic || v is Listing || v is Mapping) \"new \" + s else s"
            ));
        }
    }
    let (vals, logs) = shared_host
        .evaluate_expressions_partial(uri, test_text, module_name, &exprs)
        .map_err(CliError::err)?;

    let mut results = Vec::with_capacity(groups.len());
    let mut cursor = 0usize;
    for (group, len) in &groups {
        let slice = &vals[cursor..cursor + *len];
        cursor += *len;

        let mut failures = Vec::new();
        let mut errors = Vec::new();
        let mut total_asserts = 0usize;

        match expected_groups {
            Some(exp_list) => {
                let Some((_, exp_items)) = exp_list.iter().find(|(k, _)| k == group) else {
                    failures.push(TestFailure {
                        kind: "Example Property Mismatch".to_string(),
                        message: format!("Output file does not contain example `{group}` ({uri})"),
                    });
                    results.push(TestResult {
                        name: group.clone(),
                        total_asserts: 1,
                        failures,
                        errors,
                        is_example_written: false,
                    });
                    continue;
                };
                if exp_items.len() != *len {
                    failures.push(TestFailure {
                        kind: "Example Length Mismatch".to_string(),
                        message: format!(
                            "Expected example `{group}` to have {} elements, but got {} ({uri})",
                            exp_items.len(),
                            len
                        ),
                    });
                    results.push(TestResult {
                        name: group.clone(),
                        total_asserts: 1,
                        failures,
                        errors,
                        is_example_written: false,
                    });
                    continue;
                }
                for (idx, item) in slice.iter().enumerate() {
                    total_asserts += 1;
                    match item {
                        Ok(val) => {
                            let actual_pcf = SourceSession::result_string(val.clone())
                                .map(|s| s.to_java_utf8())
                                .unwrap_or_default();
                            if actual_pcf != exp_items[idx] {
                                failures.push(TestFailure {
                                    kind: "Example Failure".to_string(),
                                    message: format!(
                                        "#{idx}: ({uri})\n  Expected: ({expected_uri})\n  {}\n  Actual: ({actual_uri})\n  {}",
                                        exp_items[idx], actual_pcf
                                    ),
                                });
                            }
                        }
                        Err(err) => {
                            let cleaned = strip_trailing_repl_frames(err);
                            errors.push(TestError {
                                message: first_error_headline(&cleaned),
                                rendered: cleaned,
                            });
                        }
                    }
                }
            }
            None => {
                for item in slice {
                    total_asserts += 1;
                    if let Err(err) = item {
                        let cleaned = strip_trailing_repl_frames(err);
                        errors.push(TestError {
                            message: first_error_headline(&cleaned),
                            rendered: cleaned,
                        });
                    }
                }
            }
        }

        results.push(TestResult {
            name: group.clone(),
            total_asserts,
            failures,
            errors,
            is_example_written: false,
        });
    }

    Ok((
        TestSectionResults {
            name: TestSectionName::Examples,
            results,
        },
        logs,
    ))
}

fn cmd_test(args: &[String]) -> Result<i32, CliError> {
    let (opts, positionals) = parse_options(args)?;
    let working_dir = effective_working_dir(&opts)?;
    let host_env: Vec<(String, String)> = std::env::vars().collect();
    let user_settings = load_user_settings(&opts, &working_dir, &host_env)?;
    let project = discover_optional_project(&opts, &working_dir, &host_env)?;

    let mut sources: Vec<PathBuf> = if !positionals.is_empty() {
        let mut list: Vec<PathBuf> = positionals
            .iter()
            .map(|p| {
                if let Some(rest) = p.strip_prefix("file://") {
                    normalize_lexical(Path::new(&pkl_runtime::packages::zip::percent_decode(rest)))
                } else {
                    resolve_path(&working_dir, Path::new(p))
                }
            })
            .collect();
        list.sort();
        list
    } else if let Some(ref proj) = project {
        proj.project
            .tests
            .iter()
            .map(|t| normalize_lexical(&proj.project.project_dir.join(t)))
            .collect()
    } else {
        return Err(CliError::err(
            "Usage: pkl test [<options>] <modules>...\n\nError: missing argument <modules>",
        ));
    };
    sources.dedup();

    let (effective, effective_headers) = build_effective_settings(
        &opts,
        &working_dir,
        project.as_ref(),
        &user_settings,
        &host_env,
    );
    let shared_host = build_native_host(
        &opts,
        &working_dir,
        project.as_ref(),
        &effective,
        effective_headers.as_ref(),
        &[],
    )?;

    let mut all_results = Vec::new();
    let mut failed = false;
    let mut all_example_written = true;

    for (idx, src) in sources.iter().enumerate() {
        let res = run_single_test_module(src, None, opts.overwrite, &shared_host)?;
        if !res.logs.is_empty() {
            let mut stderr = io::stderr().lock();
            let _ = stderr.write_all(res.logs.as_bytes());
            let _ = stderr.flush();
        }
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
    shared_host.close();

    println!();
    print!("{}", render_spec_summary(&all_results));
    if failed {
        Err(CliError::Silent(if all_example_written { 10 } else { 1 }))
    } else {
        Ok(0)
    }
}

// ============================================================================
// Subcommand 7: `settings` (diagnostic JSON dump)
// ============================================================================

fn cmd_settings(args: &[String]) -> Result<i32, CliError> {
    let (opts, _) = parse_options(args)?;
    let working_dir = effective_working_dir(&opts)?;
    let host_env: Vec<(String, String)> = std::env::vars().collect();
    let user_settings = load_user_settings(&opts, &working_dir, &host_env)?;
    let project = discover_optional_project(&opts, &working_dir, &host_env)?;
    let (effective, _) = build_effective_settings(
        &opts,
        &working_dir,
        project.as_ref(),
        &user_settings,
        &host_env,
    );

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

#[cfg(test)]
mod static_project_parity_tests {
    use super::*;

    #[test]
    fn static_project_matches_native_host_evaluation() {
        let base = std::env::temp_dir().join(format!(
            "pkl_static_project_parity_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        let spec_dir = base.join("spec");
        let proj_dir = base.join("proj");
        std::fs::create_dir_all(&spec_dir).unwrap();
        std::fs::create_dir_all(&proj_dir).unwrap();

        let dep_src = r#"
amends "pkl:Project"

package {
  name = "example-lib"
  baseUri = "package://pkg.pkl-lang.org/github.com/example/example-lib/example-lib"
  version = "2.0.0-rc.1"
  packageZipUrl = "https://github.com/example/example-lib/releases/download/\(version)/example-lib@\(version).zip"
  description = "Core Library"
  authors {
    "Example Authors <info@example.com>"
  }
  sourceCode = "https://example.com/example-lib"
  sourceCodeUrlScheme = "\(sourceCode)/blob/\(version)%{path}#L%{line}-L%{endLine}"
  license = "MIT"
  exclude {
    "*.tmp"
  }
}
"#;
        let root_src = r#"
amends "pkl:Project"

dependencies {
  ["example-lib"] = import("../spec/PklProject")
  ["remote"] {
    uri = "package://example.com/foo@1.2.3"
    checksums {
      sha256 = "0123456789abcdef"
    }
  }
}

tests {
  "a.test.pkl"
  "b.test.pkl"
}
"#;
        let spec_file = spec_dir.join("PklProject");
        let root_file = proj_dir.join("PklProject");
        std::fs::write(&spec_file, dep_src).unwrap();
        std::fs::write(&root_file, root_src).unwrap();

        let fast_opts = ParsedCliOptions::default();
        let full_opts = ParsedCliOptions {
            timeout_ms: Some(30_000),
            ..ParsedCliOptions::default()
        };
        let fast = load_project_file(&root_file, &fast_opts, &proj_dir, &[]).unwrap();
        let full = load_project_file(&root_file, &full_opts, &proj_dir, &[]).unwrap();
        assert_eq!(fast.project, full.project);
        assert_eq!(fast.http_headers, full.http_headers);

        let _ = std::fs::remove_dir_all(&base);
    }
}
