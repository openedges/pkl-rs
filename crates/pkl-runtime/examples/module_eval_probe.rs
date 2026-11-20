//! Development-only original module-output operation, not a complete product CLI.
use pkl_runtime::evaluation::frontend::{
    ModuleSourceInput, StreamingOutputFilesOutcome, arguments::parse_module_eval_args,
    check_path_spec, evaluate_module_expression_with_platform_and_hosts,
    evaluate_module_output_with_glob_host, evaluate_module_output_with_platform_and_hosts,
    evaluate_output_files_streaming_with_hosts,
};
use pkl_runtime::logging::PlatformMetadata;
use pkl_runtime::modules::ModuleHostError;
use pkl_runtime::packages::PackageUri;
use pkl_runtime::packages::project::{
    EvaluatorSettings, PklProject, ResolvedProjectDep, find_matching_resolved, parse_deps_json,
};
use pkl_runtime::packages::zip::ZipArchive;
use pkl_runtime::value::PklString;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

#[path = "module_eval_probe/environment.rs"]
mod environment;
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

use package_cache::HostPackageCache;
use package_transport::PackageTransportClient;

#[derive(Debug)]
pub enum ProbeError {
    Unsupported(String),
    Internal(String),
}
impl ProbeError {
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::Unsupported(_) => 2,
            Self::Internal(_) => 4,
        }
    }
    pub fn message(&self) -> &str {
        match self {
            Self::Unsupported(message) | Self::Internal(message) => message,
        }
    }
}

fn file_uri(path: &Path) -> Result<String, ProbeError> {
    let path = path.to_str().ok_or_else(|| {
        ProbeError::Unsupported("module-eval adapter requires a UTF-8 source path".into())
    })?;
    pkl_runtime::modules::original_file_uri(path)
        .map_err(|error| ProbeError::Unsupported(format!("{error:?}")))
        .and_then(|uri| {
            uri.as_str().map(str::to_owned).ok_or_else(|| {
                ProbeError::Unsupported(
                    "The scalar module probe cannot represent an unpaired UTF-16 URI surrogate."
                        .into(),
                )
            })
        })
}

fn strip_wrapper_frames(stderr: &str, wrapper_uri: &str) -> String {
    let mut result = Vec::new();
    let mut current_frame_lines = Vec::new();

    for line in stderr.lines() {
        if line.trim_start().starts_with("at ") {
            if line.contains(wrapper_uri) || line.contains("at pkl.base#Module.output.") {
                break;
            } else if line.contains("at pkl.EvaluatorSettings#resolvePath")
                || line.contains("at pkl.Project#resolvedEvaluatorSettings")
            {
                current_frame_lines.clear();
            } else {
                result.append(&mut current_frame_lines);
                result.push(line.to_string());
            }
        } else if (line
            .trim_start()
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_digit())
            && line.contains('|'))
            || !current_frame_lines.is_empty()
        {
            current_frame_lines.push(line.to_string());
        } else {
            if line.is_empty() && result.last().is_some_and(|l| l.is_empty()) {
                // skip duplicate blank lines
            } else {
                result.push(line.to_string());
            }
        }
    }
    while result.last().is_some_and(|l| l.is_empty()) {
        result.pop();
    }
    result.join("\n")
}

struct NativePackageHost {
    transport: PackageTransportClient,
    cache: HostPackageCache,
    test_mode: bool,
    explicit_project_dir: Option<PathBuf>,
    no_project: bool,
    allowed_resources: std::sync::RwLock<Option<Vec<String>>>,
    allowed_modules: std::sync::RwLock<Option<Vec<String>>>,
    trace_mode: std::sync::RwLock<Option<pkl_runtime::logging::TraceMode>>,
    projects: std::sync::RwLock<
        std::collections::HashMap<PathBuf, (PklProject, Option<EvaluatorSettings>)>,
    >,
}

impl NativePackageHost {
    fn load_project_with_settings(
        &self,
        project_dir: &Path,
    ) -> Result<(PklProject, Option<EvaluatorSettings>), ModuleHostError> {
        if self.no_project {
            return Err(ModuleHostError::NotFound);
        }
        let canonical_dir = project_dir
            .canonicalize()
            .unwrap_or_else(|_| project_dir.to_path_buf());
        if let Some(cached) = self.projects.read().unwrap().get(&canonical_dir) {
            return Ok(cached.clone());
        }
        let pkl_project_file = project_dir.join("PklProject");

        if !pkl_project_file.is_file() {
            let res = (
                PklProject {
                    project_dir: project_dir.to_path_buf(),
                    declared_dependencies: BTreeMap::new(),
                    dependencies: BTreeMap::new(),
                    deps_loaded: true,
                },
                None,
            );
            self.projects
                .write()
                .unwrap()
                .insert(canonical_dir, res.clone());
            return Ok(res);
        }

        let pkl_text = std::fs::read_to_string(&pkl_project_file)
            .map_err(|err| ModuleHostError::Io(err.to_string().into()))?;

        let file_uri = format!("file://{}", pkl_project_file.display());
        let parsed_module = pkl_runtime::parser::parse_module(&pkl_text).map_err(|err| {
            ModuleHostError::SecurityManagerException {
                message: Some(err.message),
            }
        })?;

        let wrapper_text = format!(
            r#"import "pkl:Project"
import "{file_uri}" as proj

local v = proj.output.value
local isProj = v is Project
local isMod = v == proj
local rawSettings = if (isProj) v.evaluatorSettings else null
local s = if (isProj) v.resolvedEvaluatorSettings else null

output {{
  renderer = new JsonRenderer {{ indent = "" }}
  value = new Dynamic {{
    ["isProject"] = isProj
    ["isModule"] = isMod
    ["gotType"] = if (isMod) "" else v.getClass().simpleName
    ["declaredDependencies"] = if (!isProj) null else new Dynamic {{
      for (k, dep in v.dependencies) {{
        [k] = new Dynamic {{
          ["name"] = k
          ["uri"] = if (dep is Project) dep.projectFileUri else dep.uri
          ["isLocal"] = dep is Project
          ["localPackageUri"] = if (dep is Project) dep.package?.uri else null
        }}
      }}
    }}
    ["packageUri"] = if (isProj) v.package?.uri else null
    ["resolvedEvaluatorSettings"] = if (s == null) null else new Dynamic {{
      ["env"] = if (s.env == null) null else new Dynamic {{
        for (k, val in s.env) {{ [k] = val }}
      }}
      ["externalProperties"] = if (s.externalProperties == null) null else new Dynamic {{
        for (k, val in s.externalProperties) {{ [k] = val }}
      }}
      ["allowedModules"] = s.allowedModules
      ["allowedResources"] = s.allowedResources
      ["noCache"] = s.noCache
      ["modulePath"] = s.modulePath
      ["rootDir"] = s.rootDir
      ["moduleCacheDir"] = s.moduleCacheDir
      ["traceMode"] = s.traceMode
    }}
  }}
}}
"#
        );

        let wrapper_uri = "repl:project_validate";
        let wrapper_input = ModuleSourceInput {
            text: &wrapper_text,
            uri: wrapper_uri,
            inferred_name: "project_validate",
        };
        let wrapper_options = pkl_runtime::evaluation::frontend::ModuleEvalOptions {
            no_project: true,
            no_cache: true,
            settings: "pkl:settings".into(),
            ..Default::default()
        };
        let eval_res = evaluate_module_output_with_glob_host(
            wrapper_input,
            &wrapper_options,
            module_host::complete,
            resource_host::complete,
            file_glob_host::complete,
        );
        if eval_res.exit_code != 0 {
            let raw_err = String::from_utf8_lossy(&eval_res.stderr).to_string();
            let cleaned = strip_wrapper_frames(&raw_err, wrapper_uri);
            return Err(ModuleHostError::SecurityManagerException {
                message: Some(cleaned.into()),
            });
        }

        let json_text = String::from_utf8_lossy(&eval_res.stdout);
        let val_res = pkl_runtime::packages::project::parse_project_validation(&json_text)?;

        let is_project = val_res.is_project;

        if !is_project {
            let is_module = val_res.is_module;
            let got_type = if is_module {
                parsed_module
                    .declaration
                    .as_ref()
                    .and_then(|d| d.name.as_ref())
                    .map(|names| {
                        names
                            .iter()
                            .map(|n| n.as_str().unwrap_or(""))
                            .collect::<Vec<_>>()
                            .join(".")
                    })
                    .unwrap_or_else(|| "Module".to_string())
            } else {
                val_res.got_type
            };

            use pkl_runtime::diagnostic::{RenderFrame, render_error};
            use pkl_runtime::source::SourceMap;
            let mut sources = SourceMap::default();
            let source_id = sources.add(file_uri.clone(), pkl_text.clone());
            let source = sources.get(source_id).expect("source inserted");

            let msg = format!(
                "Expected `output.value` of module `{file_uri}` to be of type `pkl.Project`, but got type `{got_type}`."
            );
            if is_module {
                let span = parsed_module
                    .declaration
                    .as_ref()
                    .map(|d| d.span)
                    .unwrap_or(pkl_runtime::source::Span { start: 0, end: 0 });
                let member_name = PklString::from(got_type.clone());
                let frame = RenderFrame {
                    source,
                    uri: file_uri.into(),
                    member: &member_name,
                    span,
                    highlight_length_utf16: None,
                };
                let hint = "Try adding `amends \"pkl:Project\"` to the module header.";
                let rendered = render_error(&msg, &[frame], Some(hint));
                return Err(ModuleHostError::SecurityManagerException {
                    message: Some(rendered.into()),
                });
            } else {
                let output_val_span = parsed_module
                    .members
                    .iter()
                    .find(|m| m.name.as_str() == Some("output"))
                    .and_then(|m| match &m.expr.kind {
                        pkl_runtime::syntax::ExprKind::Object { members, .. } => {
                            members.iter().find_map(|om| match om {
                                pkl_runtime::syntax::ObjectMember::Property(pm)
                                    if pm.name.as_str() == Some("value") =>
                                {
                                    Some(pm.expr.span)
                                }
                                _ => None,
                            })
                        }
                        _ => None,
                    })
                    .unwrap_or(pkl_runtime::source::Span { start: 0, end: 0 });
                let member_name = PklString::from("value");
                let frame = RenderFrame {
                    source,
                    uri: file_uri.into(),
                    member: &member_name,
                    span: output_val_span,
                    highlight_length_utf16: None,
                };
                let rendered = render_error(&msg, &[frame], None);
                return Err(ModuleHostError::SecurityManagerException {
                    message: Some(rendered.into()),
                });
            }
        }

        let declared_dependencies = val_res.declared_dependencies;
        let mut evaluator_settings = None;
        if let Some(s) = val_res.resolved_evaluator_settings {
            if let Some(res) = &s.allowed_resources {
                *self.allowed_resources.write().unwrap() = Some(res.clone());
            }
            if let Some(mods) = &s.allowed_modules {
                *self.allowed_modules.write().unwrap() = Some(mods.clone());
            }
            if let Some(tm) = &s.trace_mode {
                match tm.as_str() {
                    "pretty" => {
                        *self.trace_mode.write().unwrap() =
                            Some(pkl_runtime::logging::TraceMode::Pretty);
                    }
                    _ => {
                        *self.trace_mode.write().unwrap() =
                            Some(pkl_runtime::logging::TraceMode::Compact);
                    }
                }
            }
            evaluator_settings = Some(s);
        }

        let project = PklProject {
            project_dir: project_dir.to_path_buf(),
            declared_dependencies,
            dependencies: BTreeMap::new(),
            deps_loaded: false,
        };

        self.projects
            .write()
            .unwrap()
            .insert(canonical_dir, (project.clone(), evaluator_settings.clone()));
        Ok((project, evaluator_settings))
    }
}

impl pkl_runtime::packages::resolver::PackageHost for NativePackageHost {
    fn fetch_metadata(
        &self,
        package: &PackageUri,
        uri: &PklString,
    ) -> Result<Vec<u8>, ModuleHostError> {
        use pkl_runtime::packages::http::PackageTransport;
        self.transport.fetch_metadata(package, uri)
    }

    fn fetch_zip(&self, package: &PackageUri, url: &PklString) -> Result<Vec<u8>, ModuleHostError> {
        use pkl_runtime::packages::http::PackageTransport;
        self.transport.fetch_zip(package, url)
    }

    fn read_disk_archive(&self, package: &PackageUri) -> Option<ZipArchive> {
        self.cache.try_read_disk_archive(package)
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

    fn find_project_dir(&self, start: &Path) -> Option<PathBuf> {
        if self.no_project {
            return None;
        }
        if let Some(explicit) = &self.explicit_project_dir {
            return Some(explicit.clone());
        }
        let mut current = if start.is_file() {
            start.parent()?.to_path_buf()
        } else {
            start.to_path_buf()
        };
        loop {
            if current.join("PklProject").is_file() {
                return Some(current);
            }
            if !current.pop() {
                break;
            }
        }
        None
    }

    fn load_project(&self, project_dir: &Path) -> Result<PklProject, ModuleHostError> {
        let (project, _) = self.load_project_with_settings(project_dir)?;
        Ok(project)
    }

    fn load_project_dependencies(&self, project: &mut PklProject) -> Result<(), ModuleHostError> {
        if project.deps_loaded {
            return Ok(());
        }
        let deps_file = project.project_dir.join("PklProject.deps.json");
        let deps_display = format!("file://{}", deps_file.display());
        if !deps_file.is_file() {
            if !project.declared_dependencies.is_empty() {
                let path_str = deps_file.display().to_string();
                let msg = format!(
                    "Encountered an error when attempting to load `PklProject.deps.json` at `{deps_display}`.\nNoSuchFileException: {path_str}"
                );
                return Err(ModuleHostError::EvalError {
                    message: msg.into(),
                    hint: Some("Try running `pkl project resolve` within the project directory to create a new set of dependencies.".into()),
                });
            }
            project.deps_loaded = true;
            return Ok(());
        }

        let deps_text = std::fs::read_to_string(&deps_file)
            .map_err(|err| ModuleHostError::Io(err.to_string().into()))?;
        let mut json_resolved: BTreeMap<String, ResolvedProjectDep> = BTreeMap::new();
        parse_deps_json(&deps_text, &mut json_resolved, &deps_display)?;

        for (name, decl) in &project.declared_dependencies {
            let lookup_uri = decl.local_package_uri.as_deref().unwrap_or(&decl.uri);
            if let Some(dep) = find_matching_resolved(lookup_uri, name, &json_resolved) {
                if decl.is_local {
                    if let (Some(dec_ver), Some(res_ver)) = (
                        decl.local_package_uri
                            .as_deref()
                            .and_then(pkl_runtime::packages::project::extract_version_from_uri),
                        dep.uri
                            .as_str()
                            .and_then(pkl_runtime::packages::project::extract_version_from_uri),
                    ) && dec_ver != res_ver
                    {
                        let declared_full = decl.local_package_uri.as_deref().unwrap_or(&decl.uri);
                        let clean_res_uri = dep
                            .uri
                            .as_str()
                            .unwrap_or("")
                            .replace("projectpackage:", "package:");
                        let msg = format!(
                            "Project `file://{}/PklProject` declares a dependency on a local package whose version doesn't match what is declared in `PklProject.deps.json`.\n\nDeclared: `{declared_full}`\nResolved: `{clean_res_uri}`\n\nRun `pkl project resolve` to update resolved dependencies.",
                            project.project_dir.display()
                        );
                        return Err(ModuleHostError::SecurityManagerException {
                            message: Some(msg.into()),
                        });
                    }
                } else if let (Some(dec_ver), Some(res_ver)) = (
                    pkl_runtime::packages::project::extract_version_from_uri(&decl.uri),
                    dep.uri
                        .as_str()
                        .and_then(pkl_runtime::packages::project::extract_version_from_uri),
                ) && pkl_runtime::packages::project::compare_dep_versions(
                    dec_ver, res_ver,
                ) == std::cmp::Ordering::Greater
                {
                    let clean_res_uri = dep
                        .uri
                        .as_str()
                        .unwrap_or("")
                        .replace("projectpackage:", "package:");
                    let project_file_uri =
                        format!("file://{}/PklProject", project.project_dir.display());
                    let decl_uri = &decl.uri;
                    let msg = format!(
                        "Project `{project_file_uri}` declares a dependency on a package that is newer than what is declared in `PklProject.deps.json`.\n\nDeclared: `{decl_uri}`\nResolved: `{clean_res_uri}`\n\nRun `pkl project resolve` to update resolved dependencies."
                    );
                    return Err(ModuleHostError::SecurityManagerException {
                        message: Some(msg.into()),
                    });
                }
                project.dependencies.insert(name.clone(), dep);
            } else {
                let display_uri = if decl.is_local {
                    decl.local_package_uri.as_deref().unwrap_or(&decl.uri)
                } else {
                    &decl.uri
                };
                let msg = format!(
                    "Did not find a resolved dependency for package `{display_uri}`.\n\nRun `pkl project resolve` to update resolved dependencies."
                );
                return Err(ModuleHostError::SecurityManagerException {
                    message: Some(msg.into()),
                });
            }
        }

        for (key, dep) in &json_resolved {
            project
                .dependencies
                .entry(key.clone())
                .or_insert_with(|| dep.clone());
        }
        project.deps_loaded = true;
        Ok(())
    }

    fn read_file(&self, path: &Path) -> Result<Vec<u8>, ModuleHostError> {
        if path.is_dir() {
            let entries = self.list_dir(path)?;
            let mut text = String::new();
            for entry in entries {
                text.push_str(entry.name.as_str().unwrap_or(""));
                text.push('\n');
            }
            return Ok(text.into_bytes());
        }
        std::fs::read(path).map_err(|err| ModuleHostError::Io(err.to_string().into()))
    }

    fn list_dir(
        &self,
        path: &Path,
    ) -> Result<Vec<pkl_runtime::glob::PathElement>, ModuleHostError> {
        let entries =
            std::fs::read_dir(path).map_err(|err| ModuleHostError::Io(err.to_string().into()))?;
        let mut result = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|err| ModuleHostError::Io(err.to_string().into()))?;
            let file_type = entry
                .file_type()
                .map_err(|err| ModuleHostError::Io(err.to_string().into()))?;
            let name = entry.file_name().to_string_lossy().to_string();
            result.push(pkl_runtime::glob::PathElement {
                name: name.into(),
                is_directory: file_type.is_dir(),
            });
        }
        result.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(result)
    }

    fn is_test_mode(&self) -> bool {
        self.test_mode
    }

    fn has_project(&self) -> bool {
        !self.no_project
            && (self.explicit_project_dir.is_some()
                || self.find_project_dir(&PathBuf::from(".")).is_some())
    }

    fn allowed_resources(&self) -> Option<Vec<String>> {
        self.allowed_resources.read().unwrap().clone()
    }

    fn allowed_modules(&self) -> Option<Vec<String>> {
        self.allowed_modules.read().unwrap().clone()
    }

    fn trace_mode(&self) -> Option<pkl_runtime::logging::TraceMode> {
        *self.trace_mode.read().unwrap()
    }
}

fn detect_os_name() -> String {
    #[cfg(target_os = "macos")]
    {
        "macOS".to_string()
    }
    #[cfg(target_os = "linux")]
    {
        "Linux".to_string()
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        std::env::consts::OS.to_string()
    }
}

fn detect_os_version() -> String {
    #[cfg(target_os = "macos")]
    {
        if let Ok(output) = std::process::Command::new("/usr/bin/sw_vers")
            .arg("-productVersion")
            .output()
            && output.status.success()
        {
            let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !version.is_empty() {
                return version;
            }
        }
    }
    #[cfg(target_os = "linux")]
    {
        if let Ok(output) = std::process::Command::new("uname").arg("-r").output()
            && output.status.success()
        {
            let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !version.is_empty() {
                return version;
            }
        }
    }
    "unknown".to_string()
}

fn detect_java_version() -> String {
    if let Ok(version) = std::env::var("PKL_ORACLE_JAVA_VERSION") {
        let trimmed = version.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }
    let java_bin = std::env::var("PKL_ORACLE_JAVA").unwrap_or_else(|_| "java".to_string());
    if let Ok(output) = std::process::Command::new(java_bin)
        .args(["-XshowSettings:properties", "-version"])
        .output()
    {
        let text = String::from_utf8_lossy(&output.stderr);
        for line in text.lines() {
            if let Some(rest) = line.trim().strip_prefix("java.version = ") {
                let version = rest.trim();
                if !version.is_empty() {
                    return version.to_string();
                }
            }
        }
    }
    "unknown".to_string()
}

fn detect_platform_metadata() -> PlatformMetadata {
    PlatformMetadata {
        os_name: detect_os_name().into(),
        os_version: detect_os_version().into(),
        cpu_architecture: std::env::consts::ARCH.into(),
        java_version: detect_java_version().into(),
    }
}

fn normalize_path(path: &Path) -> PathBuf {
    let mut components = Vec::new();
    for comp in path.components() {
        match comp {
            Component::CurDir => {}
            Component::ParentDir => match components.last() {
                Some(Component::Normal(_)) => {
                    components.pop();
                }
                Some(Component::RootDir | Component::Prefix(_)) => {}
                _ => components.push(comp),
            },
            c => components.push(c),
        }
    }
    components.into_iter().collect()
}

/// Relative path from `base` to `path`, matching `IoUtils.relativize` / `Path.relativize`.
fn relativize(path: &Path, base: &Path) -> PathBuf {
    let path_comps: Vec<_> = path.components().collect();
    let base_comps: Vec<_> = base.components().collect();
    #[cfg(windows)]
    {
        if let (Some(Component::Prefix(a)), Some(Component::Prefix(b))) =
            (path_comps.first(), base_comps.first())
            && a != b
        {
            return path.to_path_buf();
        }
    }
    let common = path_comps
        .iter()
        .zip(&base_comps)
        .take_while(|(a, b)| a == b)
        .count();
    let mut result = PathBuf::new();
    for _ in common..base_comps.len() {
        result.push("..");
    }
    for comp in &path_comps[common..] {
        result.push(comp.as_os_str());
    }
    result
}

/// Component-wise symlink-aware path resolution matching `CliEvaluator.resolveRealPath`.
fn resolve_real_path(base: &Path, path_spec: &str) -> (PathBuf, PathBuf) {
    let mut resolved = base.to_path_buf();
    let mut real = base.to_path_buf();
    for comp in Path::new(path_spec).components() {
        match comp {
            Component::RootDir | Component::Prefix(_) => continue,
            c => {
                resolved.push(c.as_os_str());
                real.push(c.as_os_str());
                if real.exists()
                    && let Ok(canon) = std::fs::canonicalize(&real)
                {
                    real = canon;
                }
            }
        }
    }
    (normalize_path(&real), normalize_path(&resolved))
}

fn resolve_output_path(
    working_dir: &Path,
    template: &str,
    module_path: &Path,
    inferred_name: &str,
    output_format: Option<&str>,
) -> PathBuf {
    let module_dir = module_path
        .parent()
        .map(|parent| {
            let rel = relativize(parent, working_dir);
            if rel.as_os_str().is_empty() {
                ".".to_owned()
            } else {
                rel.to_string_lossy().into_owned()
            }
        })
        .unwrap_or_else(|| ".".to_owned());
    let substituted = template
        .replace("%{moduleName}", inferred_name)
        .replace(
            "%{outputFormat}",
            output_format.unwrap_or("%{outputFormat}"),
        )
        .replace("%{moduleDir}", &module_dir);
    normalize_path(&working_dir.join(substituted))
}

struct ResolvedSourceModule {
    path: PathBuf,
    uri: String,
    inferred_name: String,
}

enum MultiFileConsumerError {
    Cli(String),
    Io(String),
    Eval,
}

fn write_stderr_bytes(
    stderr: &mut dyn Write,
    bytes: &[u8],
    test_mode: bool,
) -> Result<(), ProbeError> {
    if bytes.is_empty() {
        return Ok(());
    }
    let adjusted;
    let out = if test_mode {
        adjusted = adjust_test_mode_stderr(bytes);
        adjusted.as_slice()
    } else {
        bytes
    };
    stderr
        .write_all(out)
        .map_err(|error| ProbeError::Internal(error.to_string()))
}

fn canonical_or_lexical_path(path: &Path) -> PathBuf {
    if path.exists() {
        path.canonicalize().unwrap_or_else(|_| {
            let abs = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
            normalize_path(&abs)
        })
    } else {
        let abs = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
        normalize_path(&abs)
    }
}

fn is_under_root_dir(uri_str: &str, root_dir: &Path) -> bool {
    let path_str = uri_str
        .strip_prefix("file://")
        .or_else(|| uri_str.strip_prefix("file:"))
        .unwrap_or(uri_str);
    let target_path = PathBuf::from(path_str);
    let norm = canonical_or_lexical_path(&target_path);
    norm.starts_with(root_dir)
}

fn is_pattern_allowed(patterns: &[String], uri_str: &str) -> bool {
    patterns.iter().any(|pat| {
        if let Ok(re) = java_regex::Regex::new(pat) {
            re.looking_at(uri_str).is_some()
        } else {
            uri_str.starts_with(pat)
        }
    })
}

fn run(arguments: &[String]) -> Result<i32, ProbeError> {
    let working_dir =
        std::env::current_dir().map_err(|error| ProbeError::Internal(error.to_string()))?;
    let mut stdout = std::io::stdout();
    let mut stderr = std::io::stderr();
    run_with_io(arguments, &working_dir, &mut stdout, &mut stderr)
}

pub fn run_with_io(
    arguments: &[String],
    working_dir: &Path,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> Result<i32, ProbeError> {
    // Extract probe-specific arguments before parse_module_eval_args
    let mut filtered_args = Vec::new();
    let mut ca_certificates = None;
    let mut test_port = None;
    let mut test_mode = false;
    let mut cache_dir = None;
    let mut cli_trace_mode = None;
    let mut end_of_options = false;

    let mut iter = arguments.iter();
    while let Some(arg) = iter.next() {
        if end_of_options {
            filtered_args.push(arg.clone());
            continue;
        }
        match arg.as_str() {
            "--" => {
                end_of_options = true;
                filtered_args.push(arg.clone());
            }
            "--ca-certificates" => {
                if let Some(ca) = iter.next() {
                    ca_certificates = Some(PathBuf::from(ca));
                }
            }
            "--test-port" => {
                if let Some(port_str) = iter.next() {
                    test_port = port_str.parse::<u16>().ok();
                }
            }
            "--test-mode" => {
                test_mode = true;
            }
            "--cache-dir" => {
                if let Some(dir) = iter.next() {
                    cache_dir = Some(PathBuf::from(dir));
                }
            }
            "--trace-mode" => {
                if let Some(mode_str) = iter.next() {
                    cli_trace_mode = match mode_str.as_str() {
                        "compact" => Some(pkl_runtime::logging::TraceMode::Compact),
                        "pretty" => Some(pkl_runtime::logging::TraceMode::Pretty),
                        _ => None,
                    };
                }
            }
            _ => {
                filtered_args.push(arg.clone());
            }
        }
    }

    let invocation = parse_module_eval_args(&filtered_args).map_err(ProbeError::Unsupported)?;
    if let Err(message) = invocation.validate_cli() {
        writeln!(stderr, "{message}").map_err(|error| ProbeError::Internal(error.to_string()))?;
        return Ok(1);
    }

    let working_dir = normalize_path(working_dir);

    let mut modules = Vec::with_capacity(invocation.source_paths.len());
    for source_path in &invocation.source_paths {
        let path = normalize_path(&working_dir.join(source_path));
        let uri = file_uri(&path)?;
        let inferred_name = path
            .file_stem()
            .and_then(|value| value.to_str())
            .ok_or_else(|| {
                ProbeError::Unsupported("module-eval adapter requires a UTF-8 module name".into())
            })?
            .to_owned();
        modules.push(ResolvedSourceModule {
            path,
            uri,
            inferred_name,
        });
    }
    // Upstream `CliBaseOptions.normalizedSourceModules`: sort modules by URI
    // so CLI output is independent of source module argument order.
    modules.sort_by(|a, b| a.uri.cmp(&b.uri));

    let primary_path = &modules[0].path;
    let explicit_project_dir = if let Some(dir) = &invocation.project_dir {
        Some(normalize_path(&working_dir.join(dir)))
    } else if invocation.options.no_project {
        None
    } else {
        let mut cur = primary_path.clone();
        let mut found = None;
        while cur.pop() {
            if cur.join("PklProject").is_file() {
                found = Some(cur);
                break;
            }
        }
        found
    };

    let cli_env = invocation.options.environment.clone();
    let cli_props = invocation.options.properties.clone();
    let output_format = invocation.output_format.clone();
    let output_path = invocation.output_path.clone();
    let multiple_file_output_path = invocation.multiple_file_output_path.clone();
    let expression = invocation.expression.clone();
    let module_output_separator = invocation.module_output_separator.clone();

    let transport = PackageTransportClient::new(ca_certificates, test_port);
    let cache = HostPackageCache::new(cache_dir, invocation.options.no_cache);
    let host = Arc::new(NativePackageHost {
        transport,
        cache,
        test_mode,
        explicit_project_dir: explicit_project_dir.clone(),
        no_project: invocation.options.no_project,
        allowed_resources: std::sync::RwLock::new(None),
        allowed_modules: std::sync::RwLock::new(None),
        trace_mode: std::sync::RwLock::new(None),
        projects: std::sync::RwLock::new(std::collections::HashMap::new()),
    });

    let (_project_opt, settings_opt) = if let Some(pdir) = &explicit_project_dir {
        match host.load_project_with_settings(pdir) {
            Ok((proj, settings)) => (Some(proj), settings),
            Err(ModuleHostError::SecurityManagerException { message: Some(msg) }) => {
                let clean_msg = msg.as_str().unwrap_or("").trim_end_matches('\n');
                let err_out = if clean_msg.starts_with("–– Pkl Error ––") {
                    format!("{clean_msg}\n")
                } else {
                    format!("–– Pkl Error ––\n{clean_msg}\n")
                };
                write_stderr_bytes(stderr, err_out.as_bytes(), test_mode)?;
                return Ok(1);
            }
            Err(err) => {
                let err_out = format!("–– Pkl Error ––\n{err:?}\n");
                write_stderr_bytes(stderr, err_out.as_bytes(), test_mode)?;
                return Ok(1);
            }
        }
    } else {
        (None, None)
    };

    let mut options = invocation.options;
    if explicit_project_dir.is_none() {
        options.no_project = true;
    }

    // Apply precedence rules:
    // 1. Properties: settings overridden by CLI `-p`, overridden by `-f`/`--format` (`VmContext.java:91-94`)
    let mut effective_props = settings_opt
        .as_ref()
        .map(|s| s.external_properties.clone())
        .unwrap_or_default();
    for (k, v) in cli_props {
        if let Some(pos) = effective_props.iter().position(|(ck, _)| ck == &k) {
            effective_props[pos] = (k, v);
        } else {
            effective_props.push((k, v));
        }
    }
    if let Some(format) = &output_format {
        effective_props.retain(|(k, _)| k != "pkl.outputFormat");
        effective_props.push(("pkl.outputFormat".into(), format.clone()));
    }
    options.properties = effective_props;

    // 2. Environment: CLI `--env-var` > settings.env > system env
    if !cli_env.is_empty() {
        options.environment = cli_env;
    } else if let Some(settings) = &settings_opt
        && !settings.env.is_empty()
    {
        options.environment = settings.env.clone();
    } else {
        options.environment = environment::inherited().map_err(ProbeError::Unsupported)?;
    }

    // 3. Trace mode: CLI > settings > Compact
    let effective_trace_mode = cli_trace_mode.unwrap_or({
        if let Some(settings) = &settings_opt
            && let Some(tm) = &settings.trace_mode
        {
            match tm.as_str() {
                "pretty" => pkl_runtime::logging::TraceMode::Pretty,
                _ => pkl_runtime::logging::TraceMode::Compact,
            }
        } else {
            pkl_runtime::logging::TraceMode::Compact
        }
    });
    *host.trace_mode.write().unwrap() = Some(effective_trace_mode);

    // 4. Cache: CLI no_cache || settings no_cache
    if let Some(nc) = settings_opt.as_ref().and_then(|s| s.no_cache) {
        options.no_cache = options.no_cache || nc;
    }

    let effective_root = settings_opt
        .as_ref()
        .and_then(|s| s.root_dir.as_ref())
        .map(|p| canonical_or_lexical_path(p));
    let platform_metadata = detect_platform_metadata();

    let make_mod_host = {
        let root_for_mod = effective_root.clone();
        move || {
            let root_for_mod = root_for_mod.clone();
            move |request: &pkl_runtime::modules::ModuleRequest| -> pkl_runtime::modules::ModuleCompletion {
                match &request.kind {
                    pkl_runtime::modules::ModuleRequestKind::ResolveKey { key, .. } => {
                        let authored = key.uri.as_str().unwrap_or("");
                        if let Some(root_dir) = &root_for_mod
                            && authored.starts_with("file:")
                            && !is_under_root_dir(authored, root_dir)
                        {
                            return pkl_runtime::modules::ModuleCompletion::KeyResolved(Err(
                                pkl_runtime::modules::ModuleHostError::SecurityManagerException {
                                    message: Some(format!("Refusing to load module `{authored}` because it is not within the root directory (`--root-dir`).").into()),
                                },
                            ));
                        }
                        if let Some(allowed) =
                            pkl_runtime::packages::resolver::current_allowed_modules()
                            && !is_pattern_allowed(&allowed, authored)
                        {
                            return pkl_runtime::modules::ModuleCompletion::KeyResolved(Err(
                                pkl_runtime::modules::ModuleHostError::SecurityManagerException {
                                    message: Some(format!("Refusing to load module `{authored}` because it does not match any entry in the module allowlist (`--allowed-modules`).").into()),
                                },
                            ));
                        }
                    }
                    pkl_runtime::modules::ModuleRequestKind::Resolve { key, .. } => {
                        let authored = key.uri.as_str().unwrap_or("");
                        if let Some(root_dir) = &root_for_mod
                            && authored.starts_with("file:")
                            && !is_under_root_dir(authored, root_dir)
                        {
                            return pkl_runtime::modules::ModuleCompletion::Resolved(Err(
                                pkl_runtime::modules::ModuleHostError::SecurityManagerException {
                                    message: Some(format!("Refusing to load module `{authored}` because it is not within the root directory (`--root-dir`).").into()),
                                },
                            ));
                        }
                        if let Some(allowed) =
                            pkl_runtime::packages::resolver::current_allowed_modules()
                            && !is_pattern_allowed(&allowed, authored)
                        {
                            return pkl_runtime::modules::ModuleCompletion::Resolved(Err(
                                pkl_runtime::modules::ModuleHostError::SecurityManagerException {
                                    message: Some(format!("Refusing to load module `{authored}` because it does not match any entry in the module allowlist (`--allowed-modules`).").into()),
                                },
                            ));
                        }
                    }
                    pkl_runtime::modules::ModuleRequestKind::SelectKey { id, uri, .. }
                        if uri.starts_with("modulepath:") =>
                    {
                        return pkl_runtime::modules::ModuleCompletion::KeySelected(Ok(
                            pkl_runtime::modules::ModuleKey {
                                id: *id,
                                uri: uri.clone(),
                                kind: pkl_runtime::modules::ModuleKeyKind::StandardGenericUrl,
                                handle: "".into(),
                            },
                        ));
                    }
                    _ => {}
                }
                module_host::complete(request)
            }
        }
    };

    let make_res_host = {
        let root_for_res = effective_root.clone();
        move || {
            let root_for_res = root_for_res.clone();
            move |request: &pkl_runtime::vm::ReadRequest| -> Option<
                Result<pkl_runtime::resources::ReadResult, pkl_runtime::host::ReadFailure>,
            > {
                let req_uri = request.uri.as_str().unwrap_or("");
                if let Some(allowed) = pkl_runtime::packages::resolver::current_allowed_resources()
                    && !is_pattern_allowed(&allowed, req_uri)
                {
                    return Some(Err(
                        pkl_runtime::host::ReadFailure::SecurityManagerException {
                            message: Some(format!("Refusing to read resource `{req_uri}` because it does not match any entry in the resource allowlist (`--allowed-resources`).").into()),
                        },
                    ));
                }
                if let Some(root_dir) = &root_for_res
                    && req_uri.starts_with("file:")
                    && !is_under_root_dir(req_uri, root_dir)
                {
                    return Some(Err(
                        pkl_runtime::host::ReadFailure::SecurityManagerException {
                            message: Some(format!("Refusing to read resource `{req_uri}` because it is not within the root directory (`--root-dir`).").into()),
                        },
                    ));
                }
                resource_host::complete(request)
            }
        }
    };

    pkl_runtime::packages::resolver::with_package_host(host, move || {
        // Mode 1: `-m` / `--multiple-file-output-path` (`CliEvaluator.writeMultipleFileOutput`)
        if let Some(multi_template) = &multiple_file_output_path {
            let mut seen_uris = HashSet::new();
            let mut written_files: HashMap<PathBuf, (String, String)> = HashMap::new();
            #[cfg(windows)]
            const LINE_SEP: &str = "\r\n";
            #[cfg(not(windows))]
            const LINE_SEP: &str = "\n";

            for module in &modules {
                if !seen_uris.insert(module.uri.clone()) {
                    continue;
                }
                let output_dir = resolve_output_path(
                    &working_dir,
                    multi_template,
                    &module.path,
                    &module.inferred_name,
                    output_format.as_deref(),
                );
                if output_dir.exists() && !output_dir.is_dir() {
                    let message = format!(
                        "Output path `{}` exists and is not a directory.\n",
                        output_dir.display()
                    );
                    write_stderr_bytes(stderr, message.as_bytes(), test_mode)?;
                    return Ok(1);
                }
                let text = std::fs::read_to_string(&module.path)
                    .map_err(|error| ProbeError::Internal(error.to_string()))?;
                let real_output_dir = if output_dir.exists() {
                    std::fs::canonicalize(&output_dir).unwrap_or_else(|_| output_dir.clone())
                } else {
                    output_dir.clone()
                };

                let outcome = evaluate_output_files_streaming_with_hosts(
                    ModuleSourceInput {
                        text: &text,
                        uri: &module.uri,
                        inferred_name: &module.inferred_name,
                    },
                    &options,
                    Some(platform_metadata.clone()),
                    make_mod_host(),
                    make_res_host(),
                    file_glob_host::complete,
                    |path_spec, eval_bytes| {
                        if let Err(message) = check_path_spec(path_spec, cfg!(windows)) {
                            return Err(MultiFileConsumerError::Cli(message.to_java_utf8()));
                        }
                        let path_spec_str = path_spec.to_java_utf8();
                        let (real_path, resolved_path) =
                            resolve_real_path(&real_output_dir, &path_spec_str);
                        if !real_path.starts_with(&real_output_dir) {
                            return Err(MultiFileConsumerError::Cli(format!(
                                "Output file conflict: `output.files` entry `\"{path_spec_str}\"` in module `{}` resolves to file path `{}`, which is outside output directory `{}`.",
                                module.uri,
                                real_path.display(),
                                real_output_dir.display()
                            )));
                        }
                        if let Some((prev_spec, prev_uri)) = written_files.get(&real_path) {
                            return Err(MultiFileConsumerError::Cli(format!(
                                "Output file conflict: `output.files` entries `\"{prev_spec}\"` in module `{prev_uri}` and `\"{path_spec_str}\"` in module `{}` resolve to the same file path `{}`.",
                                module.uri,
                                real_path.display()
                            )));
                        }
                        if real_path.is_dir() {
                            return Err(MultiFileConsumerError::Cli(format!(
                                "Output file conflict: `output.files` entry `\"{path_spec_str}\"` in module `{}` resolves to file path `{}`, which is a directory.",
                                module.uri,
                                real_path.display()
                            )));
                        }
                        written_files
                            .insert(real_path.clone(), (path_spec_str, module.uri.clone()));
                        if let Some(parent) = real_path.parent()
                            && !parent.as_os_str().is_empty()
                        {
                            std::fs::create_dir_all(parent)
                                .map_err(|error| MultiFileConsumerError::Io(error.to_string()))?;
                        }
                        let bytes = eval_bytes().map_err(|_| MultiFileConsumerError::Eval)?;
                        std::fs::write(&real_path, &bytes)
                            .map_err(|error| MultiFileConsumerError::Io(error.to_string()))?;
                        let relative = relativize(&resolved_path, &working_dir);
                        let line = format!("{}{LINE_SEP}", relative.display());
                        stdout
                            .write_all(line.as_bytes())
                            .and_then(|()| stdout.flush())
                            .map_err(|error| MultiFileConsumerError::Io(error.to_string()))?;
                        Ok(())
                    },
                );

                match outcome {
                    StreamingOutputFilesOutcome::Ok { stderr: eval_err } => {
                        write_stderr_bytes(stderr, &eval_err, test_mode)?;
                    }
                    StreamingOutputFilesOutcome::EvalFailure(failure) => {
                        write_stderr_bytes(stderr, &failure.stderr, test_mode)?;
                        return Ok(failure.exit_code);
                    }
                    StreamingOutputFilesOutcome::ConsumerError {
                        error,
                        stderr: eval_err,
                    } => {
                        write_stderr_bytes(stderr, &eval_err, test_mode)?;
                        match error {
                            MultiFileConsumerError::Cli(message) => {
                                let line = format!("{message}\n");
                                write_stderr_bytes(stderr, line.as_bytes(), test_mode)?;
                                return Ok(1);
                            }
                            MultiFileConsumerError::Io(message) => {
                                return Err(ProbeError::Internal(message));
                            }
                            MultiFileConsumerError::Eval => {
                                return Ok(1);
                            }
                        }
                    }
                }
            }
            return Ok(0);
        }

        // Mode 2: `-o` / `--output-path` (`CliEvaluator.writeOutput` file branch)
        if let Some(out_template) = &output_path {
            let mut seen_uris = HashSet::new();
            let mut written_non_empty: HashSet<PathBuf> = HashSet::new();
            for module in &modules {
                if !seen_uris.insert(module.uri.clone()) {
                    continue;
                }
                let output_file = resolve_output_path(
                    &working_dir,
                    out_template,
                    &module.path,
                    &module.inferred_name,
                    output_format.as_deref(),
                );
                if output_file.is_dir() {
                    let message = format!(
                        "Output file `{}` is a directory. Did you mean `--multiple-file-output-path`?\n",
                        output_file.display()
                    );
                    write_stderr_bytes(stderr, message.as_bytes(), test_mode)?;
                    return Ok(1);
                }
                let text = std::fs::read_to_string(&module.path)
                    .map_err(|error| ProbeError::Internal(error.to_string()))?;
                let input = ModuleSourceInput {
                    text: &text,
                    uri: &module.uri,
                    inferred_name: &module.inferred_name,
                };
                let eval_res = if let Some(expr) = &expression {
                    evaluate_module_expression_with_platform_and_hosts(
                        input,
                        expr.as_str(),
                        &options,
                        Some(platform_metadata.clone()),
                        make_mod_host(),
                        make_res_host(),
                        file_glob_host::complete,
                    )
                } else {
                    evaluate_module_output_with_platform_and_hosts(
                        input,
                        &options,
                        Some(platform_metadata.clone()),
                        make_mod_host(),
                        make_res_host(),
                        file_glob_host::complete,
                    )
                };
                write_stderr_bytes(stderr, &eval_res.stderr, test_mode)?;
                if eval_res.exit_code != 0 {
                    return Ok(eval_res.exit_code);
                }
                if let Some(parent) = output_file.parent()
                    && !parent.as_os_str().is_empty()
                {
                    std::fs::create_dir_all(parent)
                        .map_err(|error| ProbeError::Internal(error.to_string()))?;
                }
                if !written_non_empty.contains(&output_file) {
                    std::fs::write(&output_file, &eval_res.stdout)
                        .map_err(|error| ProbeError::Internal(error.to_string()))?;
                    if !eval_res.stdout.is_empty() {
                        written_non_empty.insert(output_file);
                    }
                } else if !eval_res.stdout.is_empty() {
                    let mut file = std::fs::OpenOptions::new()
                        .append(true)
                        .open(&output_file)
                        .map_err(|error| ProbeError::Internal(error.to_string()))?;
                    file.write_all(format!("{module_output_separator}\n").as_bytes())
                        .and_then(|()| file.write_all(&eval_res.stdout))
                        .map_err(|error| ProbeError::Internal(error.to_string()))?;
                }
            }
            return Ok(0);
        }

        // Mode 3: stdout (`CliEvaluator.writeOutput` console branch)
        let mut output_written = false;
        for module in &modules {
            let text = std::fs::read_to_string(&module.path)
                .map_err(|error| ProbeError::Internal(error.to_string()))?;
            let input = ModuleSourceInput {
                text: &text,
                uri: &module.uri,
                inferred_name: &module.inferred_name,
            };
            let eval_res = if let Some(expr) = &expression {
                evaluate_module_expression_with_platform_and_hosts(
                    input,
                    expr.as_str(),
                    &options,
                    Some(platform_metadata.clone()),
                    make_mod_host(),
                    make_res_host(),
                    file_glob_host::complete,
                )
            } else {
                evaluate_module_output_with_platform_and_hosts(
                    input,
                    &options,
                    Some(platform_metadata.clone()),
                    make_mod_host(),
                    make_res_host(),
                    file_glob_host::complete,
                )
            };
            if eval_res.exit_code != 0 {
                write_stderr_bytes(stderr, &eval_res.stderr, test_mode)?;
                return Ok(eval_res.exit_code);
            }
            if !eval_res.stdout.is_empty() {
                if output_written {
                    stdout
                        .write_all(format!("{module_output_separator}\n").as_bytes())
                        .map_err(|error| ProbeError::Internal(error.to_string()))?;
                }
                stdout
                    .write_all(&eval_res.stdout)
                    .and_then(|()| stdout.flush())
                    .map_err(|error| ProbeError::Internal(error.to_string()))?;
                output_written = true;
            }
            write_stderr_bytes(stderr, &eval_res.stderr, test_mode)?;
        }
        Ok(0)
    })
}

fn adjust_test_mode_stderr(stderr: &[u8]) -> Vec<u8> {
    if let Ok(s) = std::str::from_utf8(stderr) {
        let mut out = String::with_capacity(s.len());
        for line in s.lines() {
            let mut modified_line = line.to_string();
            if let (Some(open), Some(close)) = (modified_line.rfind('('), modified_line.rfind(')'))
            {
                let inside = &modified_line[open + 1..close];
                if inside.starts_with("file:///") {
                    if let Some(comma) = inside.rfind(", line ") {
                        let uri = &inside[..comma];
                        modified_line = format!("{}({})", &modified_line[..open], uri);
                    }
                } else if inside.starts_with("https://github.com/apple/pkl/blob/0.32.0/stdlib/")
                    && let Some(pkl_end) = inside.find(".pkl#L")
                {
                    let mod_name =
                        &inside["https://github.com/apple/pkl/blob/0.32.0/stdlib/".len()..pkl_end];
                    modified_line = format!("{}(pkl:{})", &modified_line[..open], mod_name);
                }
            }
            out.push_str(&modified_line);
            out.push('\n');
        }
        out.into_bytes()
    } else {
        stderr.to_vec()
    }
}

pub fn main() {
    let arguments = std::env::args_os()
        .skip(1)
        .map(|argument| {
            argument.into_string().map_err(|_| {
                ProbeError::Unsupported("module-eval adapter requires UTF-8 arguments".into())
            })
        })
        .collect::<Result<Vec<_>, _>>();
    let result = arguments.and_then(|arguments| run(&arguments));
    match result {
        Ok(code) => std::process::exit(code),
        Err(error) => {
            let category = match error {
                ProbeError::Unsupported(_) => "unsupported",
                ProbeError::Internal(_) => "internal failure",
            };
            let _ = writeln!(
                std::io::stderr(),
                "module-eval adapter {category}: {}",
                error.message()
            );
            std::process::exit(error.exit_code());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn original_file_uri_preserves_unicode_and_quotes_reserved_path_characters() {
        assert_eq!(
            file_uri(Path::new("/tmp/日本語 #?%.pkl")).unwrap(),
            "file:///tmp/日本語%20%23%3F%25.pkl"
        );
        assert_eq!(
            file_uri(Path::new("/tmp/child%20%25.pkl")).unwrap(),
            "file:///tmp/child%2520%2525.pkl"
        );
    }

    #[test]
    fn nonexistent_input_is_an_internal_acquisition_failure_not_unsupported() {
        let path = std::env::temp_dir()
            .join(format!("pkl-stage39-absent-{}", std::process::id()))
            .join("missing.pkl");
        let args = vec![
            "eval".into(),
            "--no-cache".into(),
            "--no-project".into(),
            path.to_string_lossy().into_owned(),
        ];
        assert!(matches!(run(&args), Err(ProbeError::Internal(_))));
        let error = run(&["not-eval".into()]).unwrap_err();
        assert!(matches!(error, ProbeError::Unsupported(_)));
        assert_eq!(error.exit_code(), 2);
        assert_eq!(ProbeError::Internal("I/O".into()).exit_code(), 4);
    }
}
