//! High-level CLI driver (`CliDocGenerator`, `CliDocGeneratorOptions`, and `pkldoc` argument parser)
//! matching Apple Pkl 0.32.0 `Main.kt`, `CliDocGenerator.kt`, `CliDocGeneratorOptions.kt`,
//! `BaseOptions.kt`, `ProjectOptions.kt`, and `CliCommand.kt`.

use crate::generator::DocGenerator;
use crate::migrator::{self, DocMigrator};
use crate::model::{
    DocGeneratorError, DocPackageInfo, DocsiteInfo, PackageDependency,
    metadata_annotation_to_decoded,
};
use crate::scope::{
    PKL_VERSION, STDLIB_SOURCE_CODE_HOMEPAGE, STDLIB_SOURCE_CODE_URL_SCHEME,
    stdlib_package_page_url,
};
use crate::util::{ParsedUri, file_uri_to_path_buf};
use pkl_host_native::{
    CaCertSource, NativeHost, NativeHostConfig, NativeHostExtensions, default_module_cache_dir,
    path_to_file_uri,
};
use pkl_runtime::diagnostic::mordant::{
    AnsiLevel, colorize_clikt_help, colorize_clikt_usage_error, detect_ansi_level,
};
use pkl_runtime::evaluation::frontend::DiagnosticProfile;
use pkl_runtime::evaluation::session::SourceSession;
use pkl_runtime::external_reader::ExternalReaderConfig;
use pkl_runtime::host::http::{
    HeaderGlobMatcher, HttpClientConfig, ProxyAddress, ProxyConfig, validate_header_name,
    validate_header_value, validate_rewrite_rule,
};
use pkl_runtime::logging::TraceMode;
use pkl_runtime::modules::{ModuleHostError, ModuleSourceSpec};
use pkl_runtime::packages::project::{
    CliSettingsInput, EffectiveSettings, EvaluatorColor, PKL_PROJECT_DEPS_FILENAME,
    PKL_PROJECT_FILENAME, PklSettings, Project, ProjectDependenciesManager, ProjectDeps,
    SETTINGS_ALLOWED_MODULES, SETTINGS_ALLOWED_RESOURCES, find_project_dir_with,
    resolve_effective_settings,
};
use pkl_runtime::packages::resolver::{PackageResolver, file_uri_from_path};
use pkl_runtime::packages::{PackageAssetUri, PackageUri};
use pkl_runtime::pkl_binary_decode::{DecodedObject, DecodedValue, decode_lossless};
use pkl_runtime::schema::{ModuleSchema, SchemaExtractor, SchemaStep};
use pkl_runtime::value::PklString;
use std::collections::{BTreeMap, HashSet};
use std::io::{self, IsTerminal, Write};
use std::path::{Component, Path, PathBuf};

type HttpHeaderMap = BTreeMap<String, Vec<(String, String)>>;

/// Exact `--help` text emitted by Apple Pkl 0.32.0 `org.pkl.doc.Main`.
pub const PKLDOC_HELP_TEXT: &str = concat!(
    "Usage: pkldoc [<options>] [<modules>]...\n",
    "\n",
    "  Generate HTML documentation from Pkl modules and packages.\n",
    "\n",
    "Options:\n",
    "  --allowed-modules=<pattern1,pattern2>\n",
    "                                URI patterns that determine which modules can be\n",
    "                                loaded and evaluated.\n",
    "  --allowed-resources=<pattern1,pattern2>\n",
    "                                URI patterns that determine which external\n",
    "                                resources can be read.\n",
    "  --root-dir=<path>             Restricts access to file-based modules and\n",
    "                                resources to those located under the root\n",
    "                                directory.\n",
    "  --cache-dir=<path>            The cache directory for storing packages.\n",
    "  -w, --working-dir=<path>      Base path that relative module paths are\n",
    "                                resolved against.\n",
    "  -p, --property=<name=value>   External property to set (repeatable).\n",
    "  --color=<when>                Whether to format messages in ANSI color.\n",
    "                                Possible values of <when> are 'never', 'auto',\n",
    "                                and 'always'.\n",
    "  --no-cache                    Disable caching of packages\n",
    "  -f, --format=<text>           Output format to generate. <json, jsonnet, pcf,\n",
    "                                properties, plist, textproto, xml, yaml,\n",
    "                                pkl-binary>\n",
    "  -e, --env-var=<name=value>    Environment variable to set (repeatable).\n",
    "  --module-path=<path1:path2>   Directories, ZIP archives, or JAR archives to\n",
    "                                search when resolving modulepath: URIs.\n",
    "  --settings=<value>            Pkl settings module to use.\n",
    "  -t, --timeout=<number>        Duration, in seconds, after which evaluation of\n",
    "                                a source module will be timed out.\n",
    "  --ca-certificates=<path>      Only trust CA certificates from the provided\n",
    "                                file(s).\n",
    "  --http-proxy=<address>        Proxy to use for HTTP(S) connections.\n",
    "  --http-no-proxy=<pattern1,pattern>\n",
    "                                Hostnames that should not be connected to via a\n",
    "                                proxy.\n",
    "  --http-rewrite=<from=to>      URL prefixes that should be rewritten.\n",
    "  --http-header=<url-pattern>=<header name>:<header value>\n",
    "                                HTTP header to add to the request.\n",
    "  --external-module-reader=<scheme>='<executable> [<arguments>]'>\n",
    "                                External reader registrations for module URI\n",
    "                                schemes\n",
    "  --external-resource-reader=<scheme>='<executable> [<arguments>]'>\n",
    "                                External reader registrations for resource URI\n",
    "                                schemes\n",
    "  --trace-mode=<style>          Specifies how calls to trace() are formatted.\n",
    "                                Possible values of <style> are 'compact' and\n",
    "                                'pretty'.\n",
    "  -o, --output-dir=<directory>  Directory where generated documentation is\n",
    "                                placed.\n",
    "  --no-symlinks                 Create copies of directories and files instead\n",
    "                                of symbolic links.\n",
    "  --migrate                     Migrate a pkl-doc site from version 0 to version\n",
    "                                1.\n",
    "  --project-dir=<path>          The project directory to use for this command.\n",
    "                                By default, searches up from the working\n",
    "                                directory for a PklProject file.\n",
    "  --omit-project-settings       Ignores evaluator settings set in the PklProject\n",
    "                                file.\n",
    "  --no-project                  Disables loading settings and dependencies from\n",
    "                                the PklProject file.\n",
    "  -v, --version                 Show the version and exit\n",
    "  -h, --help                    Show this message and exit\n",
    "\n",
    "Arguments:\n",
    "  <modules>  Module paths/uris, or package uris to generate documentation for\n",
    "\n",
    "For more information, visit\n",
    "https://pkl-lang.org/main/0.32.0/pkl-doc/index.html#cli\n",
);

/// Honest native-flavor version string identifying `pkldoc-rs` (`pkldoc-rs 0.32.0 (<os>, native)`).
pub fn pkldoc_version_info() -> String {
    let os_name = detect_os_name();
    let os_version = detect_os_version();
    let os_part = if os_version.is_empty() {
        os_name
    } else {
        format!("{os_name} {os_version}")
    };
    format!("pkldoc-rs {PKL_VERSION} ({os_part}, native)")
}

fn detect_os_name() -> String {
    match std::env::consts::OS {
        "macos" => "macOS".to_string(),
        "linux" => "Linux".to_string(),
        "windows" => "Windows".to_string(),
        "freebsd" => "FreeBSD".to_string(),
        other => other.to_string(),
    }
}

fn detect_os_version() -> String {
    #[cfg(target_os = "macos")]
    {
        if let Ok(plist) =
            std::fs::read_to_string("/System/Library/CoreServices/SystemVersion.plist")
            && let Some(ver) = extract_plist_string(&plist, "ProductVersion")
        {
            return ver;
        }
        if let Ok(out) = std::process::Command::new("/usr/bin/sw_vers")
            .arg("-productVersion")
            .output()
            && out.status.success()
        {
            let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !s.is_empty() {
                return s;
            }
        }
        String::new()
    }
    #[cfg(target_os = "linux")]
    {
        if let Ok(rel) = std::fs::read_to_string("/proc/sys/kernel/osrelease") {
            let s = rel.trim().to_string();
            if !s.is_empty() {
                return s;
            }
        }
        String::new()
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        String::new()
    }
}

#[cfg(target_os = "macos")]
fn extract_plist_string(plist: &str, key: &str) -> Option<String> {
    let needle = format!("<key>{key}</key>");
    let start = plist.find(&needle)? + needle.len();
    let rest = &plist[start..];
    let s_open = rest.find("<string>")? + "<string>".len();
    let s_close = rest[s_open..].find("</string>")?;
    let val = rest[s_open..s_open + s_close].trim();
    if val.is_empty() {
        None
    } else {
        Some(val.to_string())
    }
}

/// Configuration options for [`CliDocGenerator`] (`CliDocGeneratorOptions.kt` + `CliBaseOptions.kt`).
#[derive(Clone, Debug, Default)]
pub struct CliDocGeneratorOptions {
    pub source_modules: Vec<String>,
    pub output_dir: PathBuf,
    pub is_test_mode: bool,
    pub no_symlinks: bool,
    pub migrate: bool,
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
    pub test_port: Option<u16>,
}

enum ParsedCliAction {
    Help,
    Version,
    Run(Box<CliDocGeneratorOptions>),
}

/// Entry point for the `pkldoc` standalone CLI binary (`args` excludes `argv[0]`).
pub fn run_cli(args: &[String]) -> i32 {
    let mut stdout = io::stdout().lock();
    let mut stderr = io::stderr().lock();
    run_cli_with_io(args, &mut stdout, &mut stderr)
}

/// Testable entry point capturing `stdout` and `stderr` using ambient Mordant ANSI detection.
pub fn run_cli_with_io(args: &[String], stdout: &mut dyn Write, stderr: &mut dyn Write) -> i32 {
    run_cli_with_io_and_ansi_level(args, stdout, stderr, detect_ansi_level())
}

/// Testable entry point capturing `stdout` and `stderr` with an explicit Mordant ANSI color level.
pub fn run_cli_with_io_and_ansi_level(
    args: &[String],
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
    level: AnsiLevel,
) -> i32 {
    let action = match parse_cli_args(args) {
        Ok(action) => action,
        Err(err) => {
            let msg = colorize_clikt_usage_error(err.message(), level);
            write_error_line(stderr, &msg);
            return 1;
        }
    };

    match action {
        ParsedCliAction::Help => {
            let help = colorize_clikt_help(PKLDOC_HELP_TEXT, level);
            let _ = stdout.write_all(help.as_bytes());
            let _ = stdout.flush();
            0
        }
        ParsedCliAction::Version => {
            let _ = writeln!(stdout, "{}", pkldoc_version_info());
            let _ = stdout.flush();
            0
        }
        ParsedCliAction::Run(options) => {
            let generator = CliDocGenerator::new(*options);
            match generator.run(stdout) {
                Ok(()) => {
                    let _ = stdout.flush();
                    0
                }
                Err(DocGeneratorError::User(msg)) => {
                    write_error_line(stderr, &msg);
                    1
                }
                Err(DocGeneratorError::Bug(msg)) => {
                    write_error_line(
                        stderr,
                        &format!(
                            "An unexpected error has occurred. Would you mind filing a bug report?\n\n{msg}"
                        ),
                    );
                    1
                }
            }
        }
    }
}

fn write_error_line(stderr: &mut dyn Write, msg: &str) {
    let _ = stderr.write_all(msg.as_bytes());
    if !msg.ends_with('\n') {
        let _ = stderr.write_all(b"\n");
    }
    let _ = stderr.flush();
}

fn parse_cli_args(args: &[String]) -> Result<ParsedCliAction, DocGeneratorError> {
    let mut opts = CliDocGeneratorOptions {
        // Mirrors `BaseOptions.kt:173` (`.default(Color.AUTO)`), so CLI `--color`
        // defaults to `auto` and takes precedence over `PklProject.evaluatorSettings.color`.
        color: Some(EvaluatorColor::Auto),
        ..CliDocGeneratorOptions::default()
    };
    let mut output_dir: Option<PathBuf> = None;
    let mut i = 0usize;
    let mut end_of_flags = false;

    while i < args.len() {
        let raw = &args[i];
        if end_of_flags || !raw.starts_with('-') || raw == "-" {
            opts.source_modules.push(raw.clone());
            i += 1;
            continue;
        }
        if raw == "--" {
            end_of_flags = true;
            i += 1;
            continue;
        }

        let (flag, inline_val) = if raw.starts_with("--") {
            match raw.split_once('=') {
                Some((k, v)) => (k, Some(v)),
                None => (raw.as_str(), None),
            }
        } else {
            (raw.as_str(), None)
        };

        let mut take_value = |flag_name: &str| -> Result<String, DocGeneratorError> {
            if let Some(v) = inline_val {
                Ok(v.to_string())
            } else {
                i += 1;
                args.get(i).cloned().ok_or_else(|| {
                    DocGeneratorError::user(format!("Error: option {flag_name} requires a value"))
                })
            }
        };

        match flag {
            "-h" | "--help" => return Ok(ParsedCliAction::Help),
            "-v" | "--version" => return Ok(ParsedCliAction::Version),
            "-o" | "--output-dir" => {
                output_dir = Some(PathBuf::from(take_value(flag)?));
            }
            "--no-symlinks" => {
                opts.no_symlinks = true;
            }
            "--test-mode" => {
                opts.is_test_mode = true;
            }
            "--migrate" => {
                opts.migrate = true;
            }
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
                opts.properties.push(match val.split_once('=') {
                    Some((k, v)) => (k.to_string(), v.to_string()),
                    None => (val, "true".to_string()),
                });
            }
            "-e" | "--env-var" => {
                let val = take_value(flag)?;
                let (k, v) = val.split_once('=').ok_or_else(|| {
                    DocGeneratorError::user(format!("Error: invalid value for {flag}: {val}"))
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
                ProxyAddress::parse(&val).map_err(DocGeneratorError::user)?;
                opts.http_proxy = Some(val);
            }
            "--http-no-proxy" => {
                let val = take_value(flag)?;
                opts.http_no_proxy = Some(val.split(',').map(str::to_string).collect());
            }
            "--http-rewrite" => {
                let val = take_value(flag)?;
                let (from, to) = val.split_once('=').ok_or_else(|| {
                    DocGeneratorError::user("Rewrites must be in the form of <from>=<to>")
                })?;
                validate_rewrite_rule(from).map_err(DocGeneratorError::user)?;
                validate_rewrite_rule(to).map_err(DocGeneratorError::user)?;
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
                    .map_err(|e| DocGeneratorError::user(e.to_string()))?;
                opts.external_module_readers
                    .get_or_insert_with(BTreeMap::new)
                    .insert(scheme, cfg);
            }
            "--external-resource-reader" => {
                let val = take_value(flag)?;
                let (scheme, cfg) = ExternalReaderConfig::parse_cli_option(&val)
                    .map_err(|e| DocGeneratorError::user(e.to_string()))?;
                opts.external_resource_readers
                    .get_or_insert_with(BTreeMap::new)
                    .insert(scheme, cfg);
            }
            "--color" => {
                let val = take_value(flag)?;
                let color = EvaluatorColor::parse(&val).ok_or_else(|| {
                    DocGeneratorError::user(format!(
                        "Invalid value for `--color`: `{val}` (expected `never`, `auto`, or `always`)."
                    ))
                })?;
                opts.color = Some(color);
            }
            "--trace-mode" => {
                let val = take_value(flag)?;
                if val != "compact" && val != "pretty" {
                    return Err(DocGeneratorError::user(format!(
                        "Invalid value for `--trace-mode`: `{val}` (expected `compact` or `pretty`)."
                    )));
                }
                opts.trace_mode = Some(val);
            }
            "--project-dir" => {
                opts.project_dir = Some(PathBuf::from(take_value(flag)?));
            }
            "--omit-project-settings" => {
                opts.omit_project_settings = true;
            }
            "--no-project" => {
                opts.no_project = true;
            }
            "--test-port" => {
                let val = take_value(flag)?;
                let port_i32 = val.parse::<i32>().map_err(|e| {
                    DocGeneratorError::user(format!("Invalid value for `--test-port`: {e}"))
                })?;
                opts.test_port = if port_i32 < 0 {
                    None
                } else {
                    Some(u16::try_from(port_i32).map_err(|_| {
                        DocGeneratorError::user(format!(
                            "Invalid port number `{port_i32}` for `--test-port`."
                        ))
                    })?)
                };
            }
            other => {
                return Err(DocGeneratorError::user(format!(
                    "Usage: pkldoc [<options>] [<modules>]...\n\nError: no such option {other}"
                )));
            }
        }
        i += 1;
    }

    opts.output_dir = output_dir.ok_or_else(|| {
        DocGeneratorError::user(
            "Usage: pkldoc [<options>] [<modules>]...\n\nError: missing option --output-dir",
        )
    })?;

    Ok(ParsedCliAction::Run(Box::new(opts)))
}

fn parse_timeout_ms(raw: &str) -> Result<u64, DocGeneratorError> {
    if let Ok(secs) = raw.parse::<u64>() {
        return Ok(secs.saturating_mul(1000));
    }
    Err(DocGeneratorError::user(format!(
        "Invalid value for `--timeout`: `{raw}`."
    )))
}

fn parse_cli_http_header(spec: &str) -> Result<(String, String, String), DocGeneratorError> {
    let Some((pattern, header_nv)) = spec.split_once('=') else {
        return Err(DocGeneratorError::user(format!(
            "Invalid `--http-header` `{spec}` (expected `<url-pattern>=<header name>:<header value>`)."
        )));
    };
    HeaderGlobMatcher::compile(pattern).map_err(DocGeneratorError::user)?;
    let Some((name, raw_val)) = header_nv.split_once(':') else {
        return Err(DocGeneratorError::user(format!(
            "Header '{header_nv}' is not in 'name:value' format."
        )));
    };
    if name.is_empty() {
        return Err(DocGeneratorError::user(format!(
            "Header '{header_nv}' is not in 'name:value' format."
        )));
    }
    let value = raw_val.trim_start_matches([' ', '\t']);
    validate_header_name(name).map_err(DocGeneratorError::user)?;
    validate_header_value(value).map_err(DocGeneratorError::user)?;
    Ok((pattern.to_string(), name.to_string(), value.to_string()))
}

/// High-level Pkldoc API matching `org.pkl.doc.CliDocGenerator`.
pub struct CliDocGenerator {
    options: CliDocGeneratorOptions,
}

impl CliDocGenerator {
    pub fn new(options: CliDocGeneratorOptions) -> Self {
        Self { options }
    }

    pub fn run(&self, console_out: &mut dyn Write) -> Result<(), DocGeneratorError> {
        let working_dir = effective_working_dir(&self.options)?;
        let normalized_output_dir = resolve_path(&working_dir, &self.options.output_dir);

        // Migration path (`CliDocGenerator.kt:148-156`)
        if self.options.migrate {
            let mut migrator = DocMigrator::new(normalized_output_dir, console_out);
            return migrator.run();
        }
        {
            let migrator = DocMigrator::new(normalized_output_dir.clone(), console_out);
            if !migrator.is_up_to_date() {
                return Err(DocGeneratorError::user(format!(
                    "pkldoc website model is too old (found: {}, required: {}). Run `pkldoc --migrate` to migrate the website.",
                    migrator.docsite_version(),
                    migrator::CURRENT_VERSION
                )));
            }
        }

        // Normalize and sort source module URIs (`CliBaseOptions.kt:188-196`).
        let mut normalized_source_modules: Vec<String> =
            Vec::with_capacity(self.options.source_modules.len());
        for raw in &self.options.source_modules {
            normalized_source_modules.push(normalize_source_module_uri(raw, &working_dir)?);
        }
        normalized_source_modules.sort();

        // Partition source modules (`CliDocGenerator.kt:157-187`).
        let mut docsite_info_uris: Vec<String> = Vec::new();
        let mut package_info_uris: Vec<String> = Vec::new();
        let mut regular_module_uris: Vec<String> = Vec::new();
        let mut package_uris: Vec<PackageUri> = Vec::new();

        for module_uri in &normalized_source_modules {
            let parsed = ParsedUri::parse(module_uri);
            let decoded_path_lower = parsed.decoded_path().to_ascii_lowercase();
            if decoded_path_lower.ends_with("/docsite-info.pkl") {
                docsite_info_uris.push(module_uri.clone());
            } else if decoded_path_lower.ends_with("/doc-package-info.pkl") {
                package_info_uris.push(module_uri.clone());
            } else if parsed.scheme.as_deref() == Some("package") {
                if parsed.raw_fragment.is_some() {
                    return Err(DocGeneratorError::user(
                        "Cannot generate documentation for just one module within a package",
                    ));
                }
                let pkg_uri = PackageUri::parse(module_uri.as_str())
                    .map_err(|e| DocGeneratorError::user(e.reason().to_string()))?;
                package_uris.push(pkg_uri);
            } else {
                regular_module_uris.push(module_uri.clone());
            }
        }

        if docsite_info_uris.len() > 1 {
            return Err(DocGeneratorError::user(format!(
                "`sourceModules` contains multiple modules named `docsite-info.pkl`:\n{}",
                docsite_info_uris.join("\n")
            )));
        }

        if package_info_uris.is_empty() && package_uris.is_empty() {
            return Err(DocGeneratorError::user(
                "`sourceModules` must contain at least one module named `doc-package-info.pkl`, or an argument must be a package URI.",
            ));
        }

        if regular_module_uris.is_empty() && package_uris.is_empty() {
            return Err(DocGeneratorError::user(
                "`sourceModules` must contain at least one module to generate documentation for.",
            ));
        }

        // Build evaluator host (honoring settings, project, cache dir, security, HTTP, etc.).
        let host_env: Vec<(String, String)> = std::env::vars().collect();
        let user_settings = load_user_settings(&self.options, &working_dir, &host_env)?;
        let project = discover_optional_project(&self.options, &working_dir, &host_env)?;
        let (effective, effective_headers) = build_effective_settings(
            &self.options,
            &working_dir,
            project.as_ref(),
            &user_settings,
            &host_env,
        );
        let host = build_native_host(
            &self.options,
            &working_dir,
            project.as_ref(),
            &effective,
            effective_headers.as_ref(),
        )?;

        let result = self.evaluate_and_generate(
            &host,
            &package_uris,
            &docsite_info_uris,
            &package_info_uris,
            &regular_module_uris,
            &normalized_output_dir,
            console_out,
        );
        host.close();
        result
    }

    #[allow(clippy::too_many_arguments)]
    fn evaluate_and_generate(
        &self,
        host: &NativeHost,
        package_uris: &[PackageUri],
        docsite_info_uris: &[String],
        package_info_uris: &[String],
        regular_module_uris: &[String],
        normalized_output_dir: &Path,
        console_out: &mut dyn Write,
    ) -> Result<(), DocGeneratorError> {
        let package_resolver = host.package_resolver();
        let mut session = host.new_public_session(DiagnosticProfile::CliBundledSettings);

        let mut imported_modules: BTreeMap<String, ModuleSchema> = BTreeMap::new();
        let mut packages_out: Vec<(DocPackageInfo, Vec<ModuleSchema>)> = Vec::new();
        let mut package_info_entries: Vec<(DocPackageInfo, PathBuf, Vec<ModuleSchema>)> =
            Vec::new();

        // Always resolve `pkl:base` first (`CliDocGenerator.kt:271-273`) so its immutable
        // schema cache is warm before extracting user/package module schemas.
        let pkl_base_schema = evaluate_schema(host, &mut session, "pkl:base")?;
        imported_modules.insert("pkl:base".to_string(), pkl_base_schema);

        // 1. Evaluate `package://` inputs (`CliDocGenerator.kt:227-238`).
        for pkg_uri in package_uris {
            let doc_pkg_info = package_uri_to_doc_package_info(
                pkg_uri,
                &package_resolver,
                self.options.is_test_mode,
            )?;
            let pkl_modules = gather_all_package_modules(pkg_uri, &package_resolver)?;
            let mut schemas = Vec::new();
            let mut seen_uris = HashSet::new();
            for asset_uri in pkl_modules {
                let uri_str = asset_uri.uri.to_java_utf8();
                let schema = evaluate_schema(host, &mut session, &uri_str)?;
                if schema.is_amend {
                    continue;
                }
                collect_imported_modules(
                    host,
                    &mut session,
                    &schema.imports,
                    &mut imported_modules,
                )?;
                if seen_uris.insert(schema.module_uri.clone()) {
                    schemas.push(schema);
                }
            }
            packages_out.push((doc_pkg_info, schemas));
        }

        // 2. Evaluate optional `docsite-info.pkl` (`CliDocGenerator.kt:240-249`).
        let docsite_info = if let Some(uri) = docsite_info_uris.first() {
            let obj = evaluate_module_object(host, &mut session, uri)?;
            let info = DocsiteInfo::from_decoded(&obj)?;
            collect_imported_modules(
                host,
                &mut session,
                &info.overview_imports,
                &mut imported_modules,
            )?;
            info
        } else {
            DocsiteInfo::default()
        };

        // 3. Evaluate each `doc-package-info.pkl` (`CliDocGenerator.kt:251-258`).
        for uri in package_info_uris {
            let obj = evaluate_module_object(host, &mut session, uri)?;
            let doc_pkg_info = DocPackageInfo::from_decoded(&obj)?;
            collect_imported_modules(
                host,
                &mut session,
                &doc_pkg_info.overview_imports,
                &mut imported_modules,
            )?;
            let parent_dir = file_uri_to_path_buf(uri)
                .and_then(|p| p.parent().map(Path::to_path_buf))
                .ok_or_else(|| {
                    DocGeneratorError::user(format!("Expected file URI for `{uri}`."))
                })?;
            package_info_entries.push((doc_pkg_info, parent_dir, Vec::new()));
        }

        // 4. Evaluate regular file modules and associate each with its `doc-package-info.pkl` (`CliDocGenerator.kt:260-269`).
        for uri in regular_module_uris {
            let mod_path = file_uri_to_path_buf(uri).ok_or_else(|| {
                DocGeneratorError::user(format!(
                    "Could not find a doc-package-info.pkl for module {uri}"
                ))
            })?;
            let entry = package_info_entries
                .iter_mut()
                .find(|(_, pkg_dir, _)| mod_path.starts_with(pkg_dir))
                .ok_or_else(|| {
                    DocGeneratorError::user(format!(
                        "Could not find a doc-package-info.pkl for module {uri}"
                    ))
                })?;
            let schema = evaluate_schema(host, &mut session, uri)?;
            collect_imported_modules(host, &mut session, &schema.imports, &mut imported_modules)?;
            if !entry.2.iter().any(|s| s.module_uri == schema.module_uri) {
                entry.2.push(schema);
            }
        }

        session.close();

        for (info, _, schemas) in package_info_entries {
            packages_out.push((info, schemas));
        }

        let generator = DocGenerator::new(
            docsite_info,
            packages_out,
            imported_modules,
            normalized_output_dir.to_path_buf(),
            self.options.is_test_mode,
            self.options.no_symlinks,
        );
        generator.run(console_out)
    }
}

fn stdlib_dependency(is_test_mode: bool) -> PackageDependency {
    if is_test_mode {
        PackageDependency {
            name: "pkl".to_string(),
            uri: None,
            version: "0.24.0".to_string(),
            source_code: Some("https://github.com/apple/pkl/blob/dev/stdlib/".to_string()),
            source_code_url_scheme: Some(
                "https://github.com/apple/pkl/blob/0.24.0/stdlib%{path}#L%{line}-L%{endLine}"
                    .to_string(),
            ),
            documentation: Some(
                "https://pages.github.com/apple/pkl/stdlib/pkl/0.24.0/".to_string(),
            ),
        }
    } else {
        PackageDependency {
            name: "pkl".to_string(),
            uri: None,
            version: PKL_VERSION.to_string(),
            source_code: Some(STDLIB_SOURCE_CODE_HOMEPAGE.to_string()),
            source_code_url_scheme: Some(STDLIB_SOURCE_CODE_URL_SCHEME.to_string()),
            documentation: Some(stdlib_package_page_url(PKL_VERSION)),
        }
    }
}

fn package_uri_to_doc_package_info(
    package_uri: &PackageUri,
    resolver: &PackageResolver,
    is_test_mode: bool,
) -> Result<DocPackageInfo, DocGeneratorError> {
    let (metadata, checksum) = resolver
        .get_dependency_metadata_and_compute_checksum(package_uri, None)
        .map_err(|e| {
            DocGeneratorError::user(format!(
                "Failed to package metadata for {package_uri}: {}",
                format_module_host_error(&e)
            ))
        })?;

    let mut dependencies = Vec::with_capacity(metadata.dependencies.len() + 1);
    for dep in &metadata.dependencies {
        let dep_meta = resolver
            .get_dependency_metadata(&dep.package_uri, Some(&dep.sha256))
            .map_err(|e| {
                DocGeneratorError::user(format!(
                    "Failed to fetch dependency metadata for {}: {}",
                    dep.package_uri,
                    format_module_host_error(&e)
                ))
            })?;
        dependencies.push(PackageDependency {
            name: dep_meta.name.to_java_utf8(),
            uri: Some(dep.package_uri.uri.to_java_utf8()),
            version: dep_meta.version.to_string(),
            source_code: dep_meta.source_code.map(|s| s.to_java_utf8()),
            source_code_url_scheme: dep_meta.source_code_url_scheme.map(|s| s.to_java_utf8()),
            documentation: dep_meta.documentation.map(|s| s.to_java_utf8()),
        });
    }
    dependencies.push(stdlib_dependency(is_test_mode));

    let uri_str = package_uri.uri.to_java_utf8();
    let parsed = ParsedUri::parse(&uri_str);
    let authority = parsed.authority.unwrap_or_default();
    let path_without_version = package_uri.path_without_version.to_java_utf8();
    let name = format!("{authority}{path_without_version}");
    let import_uri = package_uri
        .to_package_asset_uri("/")
        .map(|a| a.uri.to_java_utf8())
        .unwrap_or_else(|_| format!("{uri_str}#/"));

    Ok(DocPackageInfo {
        name,
        module_name_prefix: format!("{}.", metadata.name.to_java_utf8()),
        version: metadata.version.to_string(),
        import_uri,
        uri: Some(uri_str),
        authors: metadata
            .authors
            .map(|list| list.into_iter().map(|s| s.to_java_utf8()).collect()),
        issue_tracker: metadata.issue_tracker.map(|s| s.to_java_utf8()),
        dependencies,
        overview: metadata.description.map(|s| s.to_java_utf8()),
        overview_imports: Vec::new(),
        extra_attributes: vec![("Checksum".to_string(), checksum.to_java_utf8())],
        source_code: metadata.source_code.map(|s| s.to_java_utf8()),
        source_code_url_scheme: metadata.source_code_url_scheme.map(|s| s.to_java_utf8()),
        annotations: metadata
            .annotations
            .iter()
            .map(metadata_annotation_to_decoded)
            .collect(),
    })
}

fn gather_all_package_modules(
    package_uri: &PackageUri,
    resolver: &PackageResolver,
) -> Result<Vec<PackageAssetUri>, DocGeneratorError> {
    let (_, archive) = resolver.get_archive(package_uri, None).map_err(|e| {
        DocGeneratorError::user(format!(
            "Failed to list elements for {package_uri}: {}",
            format_module_host_error(&e)
        ))
    })?;

    fn recurse(
        package_uri: &PackageUri,
        dir_path: &str,
        archive: &pkl_runtime::packages::zip::ZipArchive,
        out: &mut Vec<PackageAssetUri>,
    ) -> Result<(), DocGeneratorError> {
        let Some(children) = archive.directories.get(dir_path) else {
            return Ok(());
        };
        // Match JDK 21 `jdk.nio.zipfs.ZipFileSystem.buildNodeTree()`, which inserts entries
        // from the ZIP central directory in order by prepending to `parent.child` (LIFO),
        // causing `Files.newDirectoryStream` / `PackageResolver.listElements` to yield
        // directory entries in reverse central-directory insertion order.
        for name in children.iter().rev() {
            let child_path = if dir_path == "/" {
                format!("/{name}")
            } else {
                format!("{dir_path}/{name}")
            };
            if archive.directories.contains_key(&child_path) {
                recurse(package_uri, &child_path, archive, out)?;
            } else if name.ends_with(".pkl") {
                let child_uri = package_uri
                    .to_package_asset_uri(child_path)
                    .map_err(|e| DocGeneratorError::user(e.reason().to_string()))?;
                out.push(child_uri);
            }
        }
        Ok(())
    }

    let mut out = Vec::new();
    recurse(package_uri, "/", &archive, &mut out)?;
    Ok(out)
}

fn module_uri_spec(uri: &str) -> ModuleSourceSpec {
    ModuleSourceSpec {
        uri: PklString::from(uri),
        contents: None,
    }
}

fn evaluate_schema(
    host: &NativeHost,
    session: &mut SourceSession,
    uri: &str,
) -> Result<ModuleSchema, DocGeneratorError> {
    let module_val = host
        .load_module_on(session, module_uri_spec(uri))
        .map_err(DocGeneratorError::user)?;
    let mut extractor = SchemaExtractor::new(module_val)
        .map_err(|err| DocGeneratorError::user(session.format_error(err)))?;
    let mut last_result = None;
    loop {
        match extractor
            .step(session, last_result, host.clock_ms(), host.timeout_ms())
            .map_err(|err| DocGeneratorError::user(session.format_error(err)))?
        {
            SchemaStep::RunPending => {
                let mut stderr = Vec::new();
                let val = host
                    .drive_public_session(session, &mut stderr)
                    .map_err(|err| DocGeneratorError::user(session.format_error(err)))?;
                if !stderr.is_empty() {
                    let _ = io::stderr().write_all(&stderr);
                }
                last_result = Some(val);
            }
            SchemaStep::Done(schema) => return Ok(schema),
        }
    }
}

fn evaluate_module_object(
    host: &NativeHost,
    session: &mut SourceSession,
    uri: &str,
) -> Result<DecodedObject, DocGeneratorError> {
    let bytes = host
        .evaluate_expression_pkl_binary_on(session, module_uri_spec(uri), "module")
        .map_err(DocGeneratorError::user)?;
    let decoded = decode_lossless(&bytes)
        .map_err(|e| DocGeneratorError::bug(format!("Failed to decode module `{uri}`: {e:?}")))?;
    match decoded {
        DecodedValue::Object(obj) => Ok(obj),
        other => Err(DocGeneratorError::bug(format!(
            "Expected module object when evaluating `{uri}`, got `{other:?}`"
        ))),
    }
}

fn collect_imported_modules(
    host: &NativeHost,
    session: &mut SourceSession,
    imports: &[(String, String)],
    imported_modules: &mut BTreeMap<String, ModuleSchema>,
) -> Result<(), DocGeneratorError> {
    for (_, uri) in imports {
        if !imported_modules.contains_key(uri) {
            let schema = evaluate_schema(host, session, uri)?;
            imported_modules.insert(uri.clone(), schema);
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Path, URI, Settings, Project & NativeHost construction (mirroring `CliCommand.kt`)
// ---------------------------------------------------------------------------

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

fn effective_working_dir(opts: &CliDocGeneratorOptions) -> Result<PathBuf, DocGeneratorError> {
    let cwd = std::env::current_dir().map_err(|e| {
        DocGeneratorError::user(format!("Failed to determine working directory: {e}"))
    })?;
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

fn normalize_source_module_uri(arg: &str, working_dir: &Path) -> Result<String, DocGeneratorError> {
    if arg == "-" {
        return Ok("repl:text".to_string());
    }
    if arg.starts_with('@') || is_uri_like(arg) {
        return Ok(arg.to_string());
    }
    let resolved = resolve_path(working_dir, Path::new(arg));
    Ok(path_to_file_uri(&resolved).to_java_utf8())
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

fn load_user_settings(
    opts: &CliDocGeneratorOptions,
    working_dir: &Path,
    host_env: &[(String, String)],
) -> Result<LoadedSettings, DocGeneratorError> {
    let target_file = match opts.settings_uri.as_deref() {
        Some("pkl:settings") => {
            return Ok(LoadedSettings {
                settings: PklSettings::default(),
                http_headers: None,
            });
        }
        Some(uri) => {
            if let Some(path) = file_uri_to_path_buf(uri) {
                normalize_lexical(&path)
            } else if is_uri_like(uri) {
                return Err(DocGeneratorError::user(format!(
                    "Unsupported settings URI `{uri}`."
                )));
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
        .map_err(|e| DocGeneratorError::user(format_module_host_error(&e)))?;
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
        environment: host_env.to_vec(),
        timeout_ms: opts.timeout_ms,
        internal_pkl_binary_eval: true,
        ..Default::default()
    })
    .map_err(DocGeneratorError::user)?;
    let bytes = host
        .evaluate_expression_pkl_binary(module_uri_spec(uri.as_str()), "module")
        .map_err(DocGeneratorError::user)?;
    host.close();
    let decoded = decode_lossless(&bytes)
        .map_err(|e| DocGeneratorError::user(format!("Failed to decode settings: {e:?}")))?;
    let settings = PklSettings::from_decoded_value(&decoded, &uri)
        .map_err(|e| DocGeneratorError::user(format_module_host_error(&e)))?;
    let http_headers = match &decoded {
        DecodedValue::Object(obj) => extract_decoded_http_headers(obj),
        _ => None,
    };
    Ok(LoadedSettings {
        settings,
        http_headers,
    })
}

fn load_project_file(
    project_file: &Path,
    opts: &CliDocGeneratorOptions,
    working_dir: &Path,
    host_env: &[(String, String)],
) -> Result<LoadedProject, DocGeneratorError> {
    let norm = normalize_lexical(project_file);
    let uri = file_uri_from_path(&norm)
        .map_err(|e| DocGeneratorError::user(format_module_host_error(&e)))?;
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
        environment: project_env,
        timeout_ms: opts.timeout_ms,
        internal_pkl_binary_eval: true,
        ..Default::default()
    })
    .map_err(DocGeneratorError::user)?;

    let bytes = host
        .evaluate_expression_pkl_binary(module_uri_spec(uri.as_str()), "module")
        .map_err(DocGeneratorError::user)?;
    host.close();
    let decoded = decode_lossless(&bytes)
        .map_err(|e| DocGeneratorError::user(format!("Failed to decode PklProject: {e:?}")))?;
    let project = Project::from_decoded_value(&decoded)
        .map_err(|e| DocGeneratorError::user(format_module_host_error(&e)))?;
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
    opts: &CliDocGeneratorOptions,
    working_dir: &Path,
    host_env: &[(String, String)],
) -> Result<Option<LoadedProject>, DocGeneratorError> {
    if opts.no_project {
        return Ok(None);
    }
    if let Some(ref dir) = opts.project_dir {
        let resolved = resolve_path(working_dir, dir);
        let file = resolved.join(PKL_PROJECT_FILENAME);
        if !file.is_file() {
            return Err(DocGeneratorError::user(format!(
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
) -> Result<ProjectDependenciesManager, DocGeneratorError> {
    let mut mgr = ProjectDependenciesManager::new(
        project.declared_dependencies.clone(),
        project.project_dir.clone(),
    );
    let deps_file = project.project_dir.join(PKL_PROJECT_DEPS_FILENAME);
    if deps_file.is_file() {
        let deps_uri = mgr.project_deps_file_uri();
        let text = std::fs::read_to_string(&deps_file).map_err(|e| {
            DocGeneratorError::user(format!(
                "Encountered an error when attempting to load `PklProject.deps.json` at `{deps_uri}`.\n{e}\n\nTry running `pkl project resolve` within the project directory to create a new set of dependencies."
            ))
        })?;
        let parsed = ProjectDeps::parse(&text, &deps_uri)
            .map_err(|e| DocGeneratorError::user(format_module_host_error(&e)))?;
        mgr.initialize_with_deps(parsed)
            .map_err(|e| DocGeneratorError::user(format_module_host_error(&e)))?;
    }
    Ok(mgr)
}

fn build_effective_settings(
    opts: &CliDocGeneratorOptions,
    working_dir: &Path,
    project: Option<&LoadedProject>,
    user_settings: &LoadedSettings,
    host_env: &[(String, String)],
) -> (EffectiveSettings, Option<HttpHeaderMap>) {
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
        external_module_readers: opts.external_module_readers.clone(),
        external_resource_readers: opts.external_resource_readers.clone(),
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
    format!("Pkl/{PKL_VERSION} ({}; native)", detect_os_name())
}

fn build_native_host(
    opts: &CliDocGeneratorOptions,
    working_dir: &Path,
    project: Option<&LoadedProject>,
    effective: &EffectiveSettings,
    effective_headers: Option<&HttpHeaderMap>,
) -> Result<NativeHost, DocGeneratorError> {
    let proxy = ProxyConfig::new(
        effective.http_proxy_address.as_deref(),
        &effective.http_no_proxy,
    )
    .map_err(DocGeneratorError::user)?;

    let mut http_cfg = HttpClientConfig {
        user_agent: default_native_user_agent(),
        ..HttpClientConfig::default()
    }
    .with_proxy(proxy)
    .with_test_port(opts.test_port);

    if !effective.http_rewrites.is_empty() {
        http_cfg = http_cfg
            .with_rewrites(effective.http_rewrites.clone())
            .map_err(DocGeneratorError::user)?;
    }
    if let Some(headers) = effective_headers {
        for (pattern, pairs) in headers {
            http_cfg
                .add_headers(pattern, pairs.clone())
                .map_err(DocGeneratorError::user)?;
        }
    }

    let ca_certs = if opts.ca_certificates.is_empty() {
        CaCertSource::bundled_default()
    } else {
        let resolved: Vec<PathBuf> = opts
            .ca_certificates
            .iter()
            .map(|p| resolve_path(working_dir, p))
            .collect();
        CaCertSource::from_files(resolved)
    };

    let properties: Vec<(String, String)> = effective
        .external_properties
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
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
    let use_color = match effective.color {
        EvaluatorColor::Never => false,
        EvaluatorColor::Always => true,
        EvaluatorColor::Auto => io::stdin().is_terminal() && io::stdout().is_terminal(),
    };

    let ext = NativeHostExtensions::default().with_color(use_color);

    NativeHost::with_extensions(
        NativeHostConfig {
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
            external_module_readers: effective.external_module_readers.clone(),
            external_resource_readers: effective.external_resource_readers.clone(),
            module_cache_dir: effective.module_cache_dir.clone(),
            project_dependencies,
            test_mode: false,
            trace_mode,
            power_assertions: false,
            editor_url_scheme: effective.editor_url_scheme.clone(),
            internal_pkl_binary_eval: false,
        },
        ext,
    )
    .map_err(DocGeneratorError::user)
}
