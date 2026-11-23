//! Shared CLI option model and parser matching Apple Pkl 0.32.0 (`BaseOptions.kt`,
//! `ProjectOptions.kt`, `TestOptions.kt`, and per-subcommand Clikt flags).

use pkl_runtime::external_reader::ExternalReaderConfig;
use pkl_runtime::packages::project::EvaluatorColor;
use pkl_runtime::packages::test_runner::TestReporterKind;
use std::collections::BTreeMap;
use std::path::PathBuf;

pub type HttpHeaderMap = BTreeMap<String, Vec<(String, String)>>;

/// Top-level CLI error distinguishing exit code and whether stderr message should be printed.
#[derive(Debug)]
pub enum CliError {
    /// Print message to stderr (ensuring trailing `\n`) and exit with `exit_code`.
    Message { text: String, exit_code: i32 },
    /// Already wrote output/errors; exit silently with `exit_code`.
    Silent(i32),
}

impl CliError {
    pub fn err(msg: impl Into<String>) -> Self {
        Self::Message {
            text: msg.into(),
            exit_code: 1,
        }
    }

    pub fn usage(usage_line: &str, err_msg: &str) -> Self {
        let plain = format!("{usage_line}\n\nError: {err_msg}\n");
        let level = pkl_runtime::diagnostic::mordant::detect_ansi_level();
        Self::Message {
            text: pkl_runtime::diagnostic::mordant::colorize_clikt_usage_error(&plain, level),
            exit_code: 1,
        }
    }
}

/// Identifies which Pkl CLI subcommand is parsing options so only valid flags are accepted
/// and subcommand defaults (`power_assertions`) match Apple Pkl 0.32.0.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SubcommandKind {
    Eval,
    Test,
    Repl,
    ProjectResolve,
    ProjectPackage,
    DownloadPackage,
    AnalyzeImports,
    Run,
}

impl SubcommandKind {
    pub const fn usage_line(self) -> &'static str {
        match self {
            Self::Eval => "Usage: pkl eval [<options>] <modules>...",
            Self::Test => "Usage: pkl test [<options>] [<modules>]...",
            Self::Repl => "Usage: pkl repl [<options>]",
            Self::ProjectResolve => "Usage: pkl project resolve [<options>] [<dir>]...",
            Self::ProjectPackage => "Usage: pkl project package [<options>] [<dir>]...",
            Self::DownloadPackage => "Usage: pkl download-package [<options>] <package>...",
            Self::AnalyzeImports => "Usage: pkl analyze imports [<options>] <modules>...",
            Self::Run => "Usage: pkl run [<options>] [<module>] [<args>]...",
        }
    }

    pub const fn allows_project_options(self) -> bool {
        matches!(
            self,
            Self::Eval
                | Self::Test
                | Self::Repl
                | Self::DownloadPackage
                | Self::AnalyzeImports
                | Self::Run
        )
    }

    pub const fn allows_test_options(self) -> bool {
        matches!(self, Self::Test | Self::ProjectPackage)
    }

    pub const fn allows_power_assertions_flag(self) -> bool {
        matches!(self, Self::Eval | Self::Test)
    }

    pub const fn default_power_assertions(self) -> bool {
        matches!(self, Self::Eval | Self::Test)
    }
}

/// Parsed CLI options shared across Pkl subcommands (`BaseOptions.kt` + subcommand options).
#[derive(Clone, Debug)]
pub struct ParsedCliOptions {
    pub show_help: bool,
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
    // Eval / Analyze / Project Package options
    pub output_path: Option<String>,
    pub multiple_file_output_path: Option<String>,
    pub expression: Option<String>,
    pub module_output_separator: String,
    pub skip_publish_check: bool,
    pub no_transitive: bool,
    // Test options (`TestOptions.kt`)
    pub junit_reports: Option<PathBuf>,
    pub junit_aggregate_reports: bool,
    pub junit_aggregate_suite_name: String,
    pub overwrite: bool,
    pub test_reporter: TestReporterKind,
}

impl ParsedCliOptions {
    pub fn for_subcommand(kind: SubcommandKind) -> Self {
        Self {
            show_help: false,
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
            // Mirrors `BaseOptions.kt:173` (`.default(Color.AUTO)`), so CLI `--color`
            // always defaults to `auto` and overrides `PklProject.evaluatorSettings.color`.
            color: Some(EvaluatorColor::Auto),
            trace_mode: None,
            project_dir: None,
            no_project: false,
            omit_project_settings: false,
            test_mode: false,
            test_port: None,
            power_assertions: kind.default_power_assertions(),
            output_path: None,
            multiple_file_output_path: None,
            expression: None,
            module_output_separator: "---".to_string(),
            skip_publish_check: false,
            no_transitive: false,
            junit_reports: None,
            junit_aggregate_reports: false,
            junit_aggregate_suite_name: "pkl-tests".to_string(),
            overwrite: false,
            test_reporter: TestReporterKind::Spec,
        }
    }
}

/// Parses subcommand arguments into `(ParsedCliOptions, positionals)`.
///
/// For `SubcommandKind::Run`, mirrors Clikt's `treatUnknownOptionsAsArgs = true` and
/// deferred `-h`/`--help` handling so command-defined options pass through into `positionals`.
pub fn parse_options(
    kind: SubcommandKind,
    args: &[String],
) -> Result<(ParsedCliOptions, Vec<String>), CliError> {
    let mut opts = ParsedCliOptions::for_subcommand(kind);
    let mut positionals = Vec::new();
    let mut i = 0usize;
    let mut end_of_flags = false;
    let usage = kind.usage_line();

    while i < args.len() {
        let raw = &args[i];
        if end_of_flags || raw == "-" || !raw.starts_with('-') {
            positionals.push(raw.clone());
            i += 1;
            continue;
        }
        if raw == "--" {
            if kind == SubcommandKind::Run && !positionals.is_empty() {
                // Preserve `--` for the downstream pkl:Command parser if module already seen.
                positionals.push(raw.clone());
                end_of_flags = true;
                i += 1;
                continue;
            }
            end_of_flags = true;
            i += 1;
            continue;
        }

        // Split `--flag=value` (for `--...`) or attached short value (e.g. `-pfoo=bar` for known short value flags).
        let (flag, inline_val): (&str, Option<&str>) = if raw.starts_with("--") {
            match raw.split_once('=') {
                Some((k, v)) => (k, Some(v)),
                None => (raw.as_str(), None),
            }
        } else if raw.len() > 2 {
            let short = &raw[..2];
            if is_known_short_value_flag(kind, short) {
                let rest = &raw[2..];
                let rest = rest.strip_prefix('=').unwrap_or(rest);
                (short, Some(rest))
            } else {
                (raw.as_str(), None)
            }
        } else {
            (raw.as_str(), None)
        };

        // Check `-h` / `--help`.
        if flag == "-h" || flag == "--help" {
            opts.show_help = true;
            if kind != SubcommandKind::Run {
                return Ok((opts, positionals));
            }
            i += 1;
            continue;
        }

        let mut take_value = |flag_name: &str| -> Result<String, CliError> {
            if let Some(v) = inline_val {
                Ok(v.to_string())
            } else {
                i += 1;
                args.get(i).cloned().ok_or_else(|| {
                    CliError::usage(usage, &format!("option {flag_name} requires a value"))
                })
            }
        };

        let mut handled = true;
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
                if inline_val.is_some() {
                    return Err(CliError::usage(
                        usage,
                        &format!("option {flag} does not take a value"),
                    ));
                }
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
                    CliError::usage(usage, &format!("invalid value for {flag}: {val}"))
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
                opts.timeout_ms = Some(parse_timeout_ms(usage, &val)?);
            }
            "--ca-certificates" => {
                opts.ca_certificates.push(PathBuf::from(take_value(flag)?));
            }
            "--http-proxy" => {
                let val = take_value(flag)?;
                pkl_runtime::host::http::ProxyAddress::parse(&val).map_err(CliError::err)?;
                opts.http_proxy = Some(val);
            }
            "--http-no-proxy" => {
                let val = take_value(flag)?;
                opts.http_no_proxy = Some(val.split(',').map(str::to_string).collect());
            }
            "--http-rewrite" => {
                let val = take_value(flag)?;
                let (from, to) = val.split_once('=').ok_or_else(|| {
                    CliError::usage(
                        usage,
                        "invalid value for --http-rewrite: Rewrites must be in the form of <from>=<to>",
                    )
                })?;
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
                let lower = val.to_ascii_lowercase();
                let color = EvaluatorColor::parse(&lower).ok_or_else(|| {
                    CliError::usage(
                        usage,
                        &format!(
                            "invalid value for --color: invalid choice: {val}. (choose from never, auto, always)"
                        ),
                    )
                })?;
                opts.color = Some(color);
            }
            "--trace-mode" => {
                let val = take_value(flag)?;
                let lower = val.to_ascii_lowercase();
                if lower != "compact" && lower != "pretty" {
                    return Err(CliError::usage(
                        usage,
                        &format!(
                            "invalid value for --trace-mode: invalid choice: {val}. (choose from compact, pretty)"
                        ),
                    ));
                }
                opts.trace_mode = Some(lower);
            }
            "--test-port" => {
                let val = take_value(flag)?;
                let port_i32 = val.parse::<i32>().map_err(|_| {
                    CliError::usage(
                        usage,
                        &format!("invalid value for --test-port: {val} is not a valid integer"),
                    )
                })?;
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
            "--project-dir" if kind.allows_project_options() => {
                opts.project_dir = Some(PathBuf::from(take_value(flag)?));
            }
            "--no-project" if kind.allows_project_options() => {
                opts.no_project = true;
            }
            "--omit-project-settings" if kind.allows_project_options() => {
                opts.omit_project_settings = true;
            }
            "--test-mode" if kind == SubcommandKind::Eval => {
                opts.test_mode = true;
            }
            "--power-assertions" if kind.allows_power_assertions_flag() => {
                opts.power_assertions = true;
            }
            "--no-power-assertions" if kind.allows_power_assertions_flag() => {
                opts.power_assertions = false;
            }
            "-o" | "--output-path"
                if matches!(kind, SubcommandKind::Eval | SubcommandKind::AnalyzeImports) =>
            {
                opts.output_path = Some(take_value(flag)?);
            }
            "--output-path" if kind == SubcommandKind::ProjectPackage => {
                opts.output_path = Some(take_value(flag)?);
            }
            "-m" | "--multiple-file-output-path" if kind == SubcommandKind::Eval => {
                opts.multiple_file_output_path = Some(take_value(flag)?);
            }
            "-x" | "--expression" if kind == SubcommandKind::Eval => {
                opts.expression = Some(take_value(flag)?);
            }
            "--module-output-separator" if kind == SubcommandKind::Eval => {
                opts.module_output_separator = take_value(flag)?;
            }
            "--skip-publish-check" if kind == SubcommandKind::ProjectPackage => {
                opts.skip_publish_check = true;
            }
            "--no-transitive" if kind == SubcommandKind::DownloadPackage => {
                opts.no_transitive = true;
            }
            "--junit-reports" if kind.allows_test_options() => {
                opts.junit_reports = Some(PathBuf::from(take_value(flag)?));
            }
            "--junit-aggregate-reports" if kind.allows_test_options() => {
                opts.junit_aggregate_reports = true;
            }
            "--junit-aggregate-suite-name" if kind.allows_test_options() => {
                opts.junit_aggregate_suite_name = take_value(flag)?;
            }
            "--overwrite" if kind.allows_test_options() => {
                opts.overwrite = true;
            }
            "--test-reporter" if kind.allows_test_options() => {
                let val = take_value(flag)?;
                opts.test_reporter = match val.to_ascii_lowercase().as_str() {
                    "spec" => TestReporterKind::Spec,
                    "minimal" => TestReporterKind::Minimal,
                    _ => {
                        return Err(CliError::usage(
                            usage,
                            &format!(
                                "invalid value for --test-reporter: invalid choice: {val}. (choose from spec, minimal)"
                            ),
                        ));
                    }
                };
            }
            _ => {
                handled = false;
            }
        }

        if !handled {
            if kind == SubcommandKind::Run {
                // `RunCommand.kt:37`: `override val treatUnknownOptionsAsArgs = true`
                positionals.push(raw.clone());
            } else {
                return Err(CliError::usage(usage, &format!("no such option {flag}")));
            }
        }
        i += 1;
    }

    Ok((opts, positionals))
}

fn is_known_short_value_flag(kind: SubcommandKind, short: &str) -> bool {
    match short {
        "-w" | "-p" | "-e" | "-f" | "-t" => true,
        "-o" => matches!(kind, SubcommandKind::Eval | SubcommandKind::AnalyzeImports),
        "-m" | "-x" => kind == SubcommandKind::Eval,
        _ => false,
    }
}

fn parse_property_kv(raw: &str) -> (String, String) {
    match raw.split_once('=') {
        Some((k, v)) => (k.to_string(), v.to_string()),
        None => (raw.to_string(), "true".to_string()),
    }
}

fn parse_timeout_ms(usage: &str, raw: &str) -> Result<u64, CliError> {
    if let Ok(secs) = raw.parse::<u64>() {
        return Ok(secs.saturating_mul(1000));
    }
    if let Ok(secs_f) = raw.parse::<f64>()
        && secs_f.is_finite()
        && secs_f >= 0.0
    {
        return Ok((secs_f * 1000.0).round() as u64);
    }
    Err(CliError::usage(
        usage,
        &format!("invalid value for --timeout: {raw} is not a valid integer"),
    ))
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
