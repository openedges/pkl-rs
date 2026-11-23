//! `pkl eval`, `pkl project resolve`, `pkl project package`, `pkl download-package`,
//! and `pkl analyze imports` subcommands.

use crate::env::{
    ResolvedSourceArg, build_ca_cert_source, build_effective_settings, build_http_client_config,
    build_native_host, build_standalone_package_resolver, discover_optional_project,
    effective_working_dir, format_module_host_error, load_project_file, load_user_settings,
    normalize_lexical, normalize_source_module, read_stdin_utf8, resolve_path, resolve_use_color,
};
use crate::help::{
    HELP_ANALYZE_IMPORTS, HELP_DOWNLOAD_PACKAGE, HELP_EVAL, HELP_PROJECT_PACKAGE,
    HELP_PROJECT_RESOLVE,
};
use crate::options::{CliError, ParsedCliOptions, SubcommandKind, parse_options};
use crate::test_cmd::run_project_api_tests;
use pkl_host_native::{
    CancelFlag, NativeHost, NativeHttpClient, NativePackagerHost, NativeStreamingOutputFilesOutcome,
};
use pkl_runtime::evaluation::frontend::{DiagnosticProfile, ModuleEvalOptions, check_path_spec};
use pkl_runtime::host::security::SecurityPolicy;
use pkl_runtime::packages::PackageUri;
use pkl_runtime::packages::mvs::{ProjectDependenciesResolver, relativize_paths};
use pkl_runtime::packages::packager::ProjectPackager;
use pkl_runtime::packages::project::{
    PKL_PROJECT_DEPS_FILENAME, PKL_PROJECT_FILENAME, Project, find_project_dir_with,
};
use pkl_runtime::packages::resolver::file_uri_from_path;
use std::collections::{HashMap, HashSet};
use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

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

// ============================================================================
// Subcommand: `eval`
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

pub fn cmd_eval(args: &[String]) -> Result<i32, CliError> {
    let (opts, positionals) = parse_options(SubcommandKind::Eval, args)?;
    if opts.show_help {
        crate::write_help(HELP_EVAL);
        return Ok(0);
    }
    if opts.multiple_file_output_path.is_some()
        && (opts.output_path.is_some() || opts.expression.is_some())
    {
        return Err(CliError::usage(
            SubcommandKind::Eval.usage_line(),
            "Option is mutually exclusive with -o, --output-path and -x, --expression.",
        ));
    }
    if positionals.is_empty() {
        return Err(CliError::usage(
            SubcommandKind::Eval.usage_line(),
            "missing argument <modules>",
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

    let use_color = resolve_use_color(effective.color);
    let host = build_native_host(
        &opts,
        &working_dir,
        project.as_ref(),
        &effective,
        effective_headers.as_ref(),
        &[],
        use_color,
    )?;

    let eval_options = ModuleEvalOptions {
        properties: Vec::new(),
        environment: Vec::new(),
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
                    write_cli_error_stderr(&mut stderr, &failure.stderr);
                    return Ok(failure.exit_code);
                }
                NativeStreamingOutputFilesOutcome::ConsumerError {
                    error,
                    stderr: eval_err,
                } => {
                    write_cli_error_stderr(&mut stderr, &eval_err);
                    match error {
                        MultiFileError::Cli(msg) | MultiFileError::Io(msg) => {
                            return Err(CliError::err(msg));
                        }
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
            if eval_res.exit_code != 0 {
                write_cli_error_stderr(&mut stderr, &eval_res.stderr);
                return Ok(eval_res.exit_code);
            }
            let _ = stderr.write_all(&eval_res.stderr);
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

    // Mode 3: stdout
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
            write_cli_error_stderr(&mut stderr, &eval_res.stderr);
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

/// Matches `CliMain.kt`'s `printError`: always terminates non-empty error stderr with `\n`
/// (critical for ANSI-colored errors whose rendered string ends with `\x1b[0m`).
fn write_cli_error_stderr(stderr: &mut impl Write, bytes: &[u8]) {
    if bytes.is_empty() {
        return;
    }
    let _ = stderr.write_all(bytes);
    if !bytes.ends_with(b"\n") {
        let _ = stderr.write_all(b"\n");
    }
    let _ = stderr.flush();
}

// ============================================================================
// Subcommand: `project resolve`
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

pub fn cmd_project_resolve(args: &[String]) -> Result<i32, CliError> {
    let (opts, positionals) = parse_options(SubcommandKind::ProjectResolve, args)?;
    if opts.show_help {
        crate::write_help(HELP_PROJECT_RESOLVE);
        return Ok(0);
    }

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
// Subcommand: `project package`
// ============================================================================

pub fn cmd_project_package(args: &[String]) -> Result<i32, CliError> {
    let (opts, positionals) = parse_options(SubcommandKind::ProjectPackage, args)?;
    if opts.show_help {
        crate::write_help(HELP_PROJECT_PACKAGE);
        return Ok(0);
    }

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
            let mut api_test_files: Vec<(String, PathBuf)> = pkg
                .api_tests
                .iter()
                .map(|t| {
                    let norm_path = normalize_lexical(&loaded.project.project_dir.join(t));
                    let raw_uri = file_uri_from_path(&norm_path)
                        .map_err(|e| CliError::err(format_module_host_error(&e)))?;
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

            let passed = run_project_api_tests(
                &opts,
                &working_dir,
                &loaded,
                &user_settings,
                &host_env,
                &api_test_files,
            )?;
            if !passed {
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
// Subcommand: `download-package`
// ============================================================================

pub fn cmd_download_package(args: &[String]) -> Result<i32, CliError> {
    let (opts, positionals) = parse_options(SubcommandKind::DownloadPackage, args)?;
    if opts.show_help {
        crate::write_help(HELP_DOWNLOAD_PACKAGE);
        return Ok(0);
    }
    if positionals.is_empty() {
        return Err(CliError::usage(
            SubcommandKind::DownloadPackage.usage_line(),
            "missing argument <package>",
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
// Subcommand: `analyze imports`
// ============================================================================

pub fn cmd_analyze_imports(args: &[String]) -> Result<i32, CliError> {
    let (opts, positionals) = parse_options(SubcommandKind::AnalyzeImports, args)?;
    if opts.show_help {
        crate::write_help(HELP_ANALYZE_IMPORTS);
        return Ok(0);
    }
    if positionals.is_empty() {
        return Err(CliError::usage(
            SubcommandKind::AnalyzeImports.usage_line(),
            "missing argument <modules>",
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

    let use_color = resolve_use_color(effective.color);
    let host = build_native_host(
        &opts,
        &working_dir,
        project.as_ref(),
        &effective,
        effective_headers.as_ref(),
        &extra_props,
        use_color,
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

    let res = host.eval_uri_output(
        "repl:text",
        Some(ANALYZE_IMPORTS_MODULE),
        Some("output.text"),
        &eval_options,
    );
    host.close();

    if res.exit_code != 0 {
        let mut stderr = io::stderr().lock();
        write_cli_error_stderr(&mut stderr, &res.stderr);
        return Ok(res.exit_code);
    }
    if !res.stderr.is_empty() {
        let _ = io::stderr().write_all(&res.stderr);
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
