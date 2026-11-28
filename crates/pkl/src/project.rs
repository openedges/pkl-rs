//! PklProject loading and discovery mirroring `org.pkl.core.project.Project`.

use crate::error::{PklError, Result, format_module_host_error};
use crate::settings::PklEvaluatorSettings;
use pkl_host_native::{NativeHost, NativeHostConfig, NativeHostExtensions, path_to_file_uri};
use pkl_runtime::evaluation::frontend::DiagnosticProfile;
use pkl_runtime::evaluation::session::SourceSession;
use pkl_runtime::modules::ModuleSourceSpec;
use pkl_runtime::packages::project::{
    PackageSpec, Project as RuntimeProject, ProjectDependenciesManager, ProjectDeps,
    find_project_dir_with,
};
use pkl_runtime::pkl_binary_decode::decode_lossless as decode_pkl_binary;
use pkl_runtime::value::{ErrorKind, PklString};
use std::path::{Path, PathBuf};

/// Loaded `PklProject` descriptor (`org.pkl.core.project.Project`).
#[derive(Clone, Debug)]
pub struct Project {
    inner: RuntimeProject,
    dependencies_manager: Option<ProjectDependenciesManager>,
}

impl Project {
    /// Loads a `PklProject` file from `project_file` (mirroring `Project.loadFromPath`).
    pub fn load_from_path(project_file: impl AsRef<Path>) -> Result<Self> {
        Self::load_from_path_with_env(project_file, std::env::vars().collect(), Vec::new())
    }

    /// Loads a `PklProject` file with explicit environment variables and external properties.
    pub fn load_from_path_with_env(
        project_file: impl AsRef<Path>,
        environment: Vec<(String, String)>,
        properties: Vec<(String, String)>,
    ) -> Result<Self> {
        let config = NativeHostConfig {
            allowed_modules: Some(
                pkl_runtime::host::security::DEFAULT_ALLOWED_MODULES
                    .iter()
                    .map(|s| (*s).to_string())
                    .collect(),
            ),
            allowed_resources: Some(
                pkl_runtime::host::security::DEFAULT_ALLOWED_RESOURCES
                    .iter()
                    .map(|s| (*s).to_string())
                    .collect(),
            ),
            properties,
            environment,
            internal_pkl_binary_eval: true,
            ..Default::default()
        };
        Self::load_from_path_with_host_config(project_file, config, None)
    }

    #[doc(hidden)]
    pub fn load_from_path_with_host_config(
        project_file: impl AsRef<Path>,
        mut config: NativeHostConfig,
        extensions: Option<NativeHostExtensions>,
    ) -> Result<Self> {
        let project_path = project_file.as_ref();
        let abs = if project_path.is_absolute() {
            project_path.to_path_buf()
        } else {
            std::env::current_dir()?.join(project_path)
        };
        let canonical = std::fs::canonicalize(&abs).unwrap_or(abs);
        let project_uri = path_to_file_uri(&canonical).to_java_utf8();

        config.internal_pkl_binary_eval = true;
        let timeout_ms = config.timeout_ms;
        let host = match extensions {
            Some(ext) => NativeHost::with_extensions(config, ext),
            None => NativeHost::new(config),
        }
        .map_err(PklError::configuration)?;
        let mut session = host.new_public_session(DiagnosticProfile::CliBundledSettings);
        let spec = ModuleSourceSpec {
            uri: PklString::from(project_uri.as_str()),
            contents: None,
        };
        let resolved_spec = match host.resolve_module_source_spec(spec) {
            Ok(s) => s,
            Err(err) => {
                let mapped = match &err {
                    pkl_runtime::modules::ModuleHostError::SecurityManagerException { .. } => {
                        PklError::security(format_module_host_error(&err))
                    }
                    pkl_runtime::modules::ModuleHostError::Io(_)
                    | pkl_runtime::modules::ModuleHostError::NotFound => {
                        PklError::io(format_module_host_error(&err))
                    }
                    _ => PklError::evaluation(format_module_host_error(&err)),
                };
                session.close();
                host.close();
                return Err(mapped);
            }
        };
        let eval_res = (|| -> std::result::Result<Vec<u8>, pkl_runtime::value::RuntimeError> {
            let now = if timeout_ms.is_some() {
                host.clock_ms()
            } else {
                session.host_session().last_clock()
            };
            let mut stderr = Vec::new();
            session.start_evaluation_pkl_binary(resolved_spec, "output.value", now, timeout_ms)?;
            let val = host.drive_public_session(&mut session, &mut stderr)?;
            SourceSession::result_bytes(val).map(|b| b.to_vec())
        })();
        let bytes = match eval_res {
            Ok(b) => b,
            Err(err) => {
                let mapped = match err.kind {
                    ErrorKind::Timeout => PklError::timeout(
                        pkl_runtime::host::format_timeout_message(timeout_ms.unwrap_or(0)),
                    ),
                    ErrorKind::Cancelled => PklError::cancelled(err.message.to_java_utf8()),
                    ErrorKind::Closed => PklError::closed(err.message.to_java_utf8()),
                    _ => {
                        let is_security = err.is_security_error();
                        let rendered = session.format_error(err);
                        if is_security {
                            PklError::security(rendered)
                        } else {
                            PklError::evaluation(rendered)
                        }
                    }
                };
                session.close();
                host.close();
                return Err(mapped);
            }
        };
        session.close();
        host.close();

        let decoded = decode_pkl_binary(&bytes)
            .map_err(|err| PklError::decode(format!("Failed to decode PklProject: {err}")))?;
        let mut runtime_project = RuntimeProject::from_decoded_value(&decoded).map_err(|err| {
            PklError::evaluation(format!(
                "Failed to parse PklProject: {}",
                format_module_host_error(&err)
            ))
        })?;
        if let Some(dir) = canonical.parent() {
            runtime_project.project_dir = dir.to_path_buf();
            runtime_project.project_file_uri = project_uri;
            let mut base_uri = path_to_file_uri(dir).to_java_utf8();
            if !base_uri.ends_with('/') {
                base_uri.push('/');
            }
            runtime_project.project_base_uri = base_uri;
        }

        let deps_json_path = runtime_project.project_dir.join("PklProject.deps.json");
        let mut mgr = ProjectDependenciesManager::new(
            runtime_project.declared_dependencies.clone(),
            runtime_project.project_dir.clone(),
        );
        if deps_json_path.is_file() {
            let raw_bytes = pkl_host_native::read_path_nofollow_bounded(&deps_json_path)?;
            let raw = String::from_utf8(raw_bytes).map_err(|err| {
                PklError::io(format!(
                    "I/O error reading `{}`: {err}",
                    deps_json_path.display()
                ))
            })?;
            let deps_uri = mgr.project_deps_file_uri();
            let deps = ProjectDeps::parse(&raw, &deps_uri).map_err(|err| {
                PklError::configuration(format!(
                    "Failed to parse `{}`: {}",
                    deps_json_path.display(),
                    format_module_host_error(&err)
                ))
            })?;
            mgr.initialize_with_deps(deps).map_err(|err| {
                PklError::configuration(format!(
                    "Failed to initialize project dependencies for `{}`: {}",
                    deps_json_path.display(),
                    format_module_host_error(&err)
                ))
            })?;
        }
        let dependencies_manager = Some(mgr);

        Ok(Self {
            inner: runtime_project,
            dependencies_manager,
        })
    }

    /// Walks up from `dir` (bounded by `root_dir` if provided) looking for a `PklProject` file.
    pub fn discover_from_dir(dir: impl AsRef<Path>, root_dir: Option<&Path>) -> Option<PathBuf> {
        find_project_dir_with(dir.as_ref(), root_dir, |p| p.is_file()).map(|d| d.join("PklProject"))
    }

    pub fn project_dir(&self) -> &Path {
        &self.inner.project_dir
    }

    pub fn project_file_uri(&self) -> &str {
        &self.inner.project_file_uri
    }

    pub fn project_base_uri(&self) -> &str {
        &self.inner.project_base_uri
    }

    pub fn package(&self) -> Option<&PackageSpec> {
        self.inner.package.as_ref()
    }

    pub fn evaluator_settings(&self) -> Option<&PklEvaluatorSettings> {
        self.inner
            .resolved_evaluator_settings
            .as_ref()
            .or(self.inner.evaluator_settings.as_ref())
    }

    pub fn tests(&self) -> &[String] {
        &self.inner.tests
    }

    #[doc(hidden)]
    pub fn dependencies_manager(&self) -> Option<&ProjectDependenciesManager> {
        self.dependencies_manager.as_ref()
    }

    #[allow(dead_code)]
    pub(crate) fn runtime_project(&self) -> &RuntimeProject {
        &self.inner
    }
}
