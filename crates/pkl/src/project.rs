//! PklProject loading and discovery mirroring `org.pkl.core.project.Project`.

use crate::error::{PklError, Result};
use crate::settings::PklEvaluatorSettings;
use pkl_host_native::{NativeHost, NativeHostConfig, path_to_file_uri};
use pkl_runtime::evaluation::frontend::DiagnosticProfile;
use pkl_runtime::modules::ModuleSourceSpec;
use pkl_runtime::packages::project::{
    PackageSpec, Project as RuntimeProject, ProjectDependenciesManager, ProjectDeps,
    find_project_dir_with,
};
use pkl_runtime::pkl_binary_decode::decode_lossless as decode_pkl_binary;
use pkl_runtime::value::PklString;
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
        let project_path = project_file.as_ref();
        let abs = if project_path.is_absolute() {
            project_path.to_path_buf()
        } else {
            std::env::current_dir()?.join(project_path)
        };
        let canonical = std::fs::canonicalize(&abs).unwrap_or(abs);
        let project_uri = path_to_file_uri(&canonical).to_java_utf8();

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
        let host = NativeHost::new(config).map_err(PklError::configuration)?;
        let mut session = host.new_public_session(DiagnosticProfile::CliBundledSettings);
        let spec = ModuleSourceSpec {
            uri: PklString::from(project_uri.as_str()),
            contents: None,
        };
        let bytes = host
            .evaluate_expression_pkl_binary_on(&mut session, spec, "output.value")
            .map_err(PklError::evaluation)?;
        session.close();
        host.close();

        let decoded = decode_pkl_binary(&bytes)
            .map_err(|err| PklError::decode(format!("Failed to decode PklProject: {err}")))?;
        let mut runtime_project = RuntimeProject::from_decoded_value(&decoded)
            .map_err(|err| PklError::evaluation(format!("Failed to parse PklProject: {err:?}")))?;
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
            let raw = std::fs::read_to_string(&deps_json_path)?;
            let deps_uri = mgr.project_deps_file_uri();
            let deps = ProjectDeps::parse(&raw, &deps_uri).map_err(|err| {
                PklError::configuration(format!(
                    "Failed to parse `{}`: {err:?}",
                    deps_json_path.display()
                ))
            })?;
            mgr.initialize_with_deps(deps).map_err(|err| {
                PklError::configuration(format!(
                    "Failed to initialize project dependencies for `{}`: {err:?}",
                    deps_json_path.display()
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

    pub fn dependencies_manager(&self) -> Option<&ProjectDependenciesManager> {
        self.dependencies_manager.as_ref()
    }

    pub fn runtime_project(&self) -> &RuntimeProject {
        &self.inner
    }
}
