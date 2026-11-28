//! Pure Minimum Version Selection (MVS Algorithm 1) dependency resolver for `pkl project resolve`.
//!
//! Matches `org.pkl.core.project.ProjectDependenciesResolver` (`ProjectDependenciesResolver.java:47-150`).
use crate::modules::ModuleHostError;
use crate::packages::metadata::DependencyMetadata;
use crate::packages::project::{DeclaredDependenciesTree, Project, ProjectDepEntry, ProjectDeps};
use crate::packages::{CanonicalPackageUri, PackageUri};
use crate::value::PklString;
use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};

/// Metadata + computed SHA-256 provider for MVS dependency resolution.
///
/// Mirrors `PackageResolver.getDependencyMetadataAndComputeChecksum(PackageUri)`.
pub trait MetadataChecksumProvider {
    fn get_dependency_metadata_and_checksum(
        &self,
        package_uri: &PackageUri,
    ) -> Result<(DependencyMetadata, PklString), ModuleHostError>;
}

/// Output of `ProjectDependenciesResolver::resolve`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolveResult {
    pub deps: ProjectDeps,
    /// Warning lines emitted to stderr/logWriter (e.g. local dependency overridden to remote).
    pub warnings: Vec<String>,
}

/// MVS Algorithm 1 resolver (`ProjectDependenciesResolver.java:47-150`).
pub struct ProjectDependenciesResolver<'a, P: MetadataChecksumProvider + ?Sized> {
    project: &'a Project,
    provider: &'a P,
    resolved_dependencies: Vec<(CanonicalPackageUri, ProjectDepEntry)>,
    already_handled_dependencies: BTreeMap<PackageUri, PklString>,
    warnings: Vec<String>,
}

impl<'a, P: MetadataChecksumProvider + ?Sized> ProjectDependenciesResolver<'a, P> {
    pub fn new(project: &'a Project, provider: &'a P) -> Self {
        Self {
            project,
            provider,
            resolved_dependencies: Vec::new(),
            already_handled_dependencies: BTreeMap::new(),
            warnings: Vec::new(),
        }
    }

    pub fn resolve(mut self) -> Result<ResolveResult, ModuleHostError> {
        self.build_resolved_dependencies(&self.project.declared_dependencies.clone())?;
        // Warn if any direct local dependency was overridden by a higher-version remote dep
        // (`ProjectDependenciesResolver.java:65-76`).
        for (_, local_project) in &self.project.declared_dependencies.local_dependencies {
            if let Some(ref package_uri) = local_project.package_uri {
                let canonical = package_uri.to_canonical();
                if let Some((_, resolved_dep)) = self
                    .resolved_dependencies
                    .iter()
                    .find(|(k, _)| k == &canonical)
                    && !resolved_dep.is_local()
                {
                    self.warnings.push(format!(
                        "WARN: local dependency `{}` was overridden to remote dependency `{}`.",
                        package_uri.display_name(),
                        resolved_dep.package_uri().display_name()
                    ));
                }
            }
        }
        Ok(ResolveResult {
            deps: ProjectDeps::new(self.resolved_dependencies),
            warnings: self.warnings,
        })
    }

    fn build_resolved_dependencies(
        &mut self,
        declared: &DeclaredDependenciesTree,
    ) -> Result<(), ModuleHostError> {
        for dep in &declared.remote_dependencies {
            self.resolve_dependencies_of_package_uri(
                dep.package_uri.to_project_package_uri(),
                dep.sha256.as_ref(),
            )?;
        }
        for (_, local_decl) in &declared.local_dependencies {
            self.resolve_local_dependencies(local_decl)?;
        }
        Ok(())
    }

    fn resolve_dependencies_of_package_uri(
        &mut self,
        package_uri: PackageUri,
        expected_sha256: Option<&PklString>,
    ) -> Result<(), ModuleHostError> {
        if let Some(prev_sha256) = self.already_handled_dependencies.get(&package_uri) {
            if let Some(expected) = expected_sha256
                && prev_sha256 != expected
            {
                return Err(ModuleHostError::SecurityManagerException {
                    message: Some(
                        format!(
                            "Computed checksum did not match declared checksum for dependency `{}`.\n\nComputed: \"{prev_sha256}\"\nDeclared: \"{expected}\"",
                            package_uri.display_name()
                        )
                        .into(),
                    ),
                });
            }
            return Ok(());
        }
        let (metadata, computed_sha256) = self
            .provider
            .get_dependency_metadata_and_checksum(&package_uri)?;
        if let Some(expected) = expected_sha256
            && &computed_sha256 != expected
        {
            return Err(ModuleHostError::SecurityManagerException {
                message: Some(
                    format!(
                        "Computed checksum did not match declared checksum for dependency `{}`.\n\nComputed: \"{computed_sha256}\"\nDeclared: \"{expected}\"",
                        package_uri.display_name()
                    )
                    .into(),
                ),
            });
        }
        let dep_with_checksum = ProjectDepEntry::Remote {
            uri: package_uri.clone(),
            sha256: computed_sha256.clone(),
        };
        self.update_dependency(dep_with_checksum);
        self.already_handled_dependencies
            .insert(package_uri, computed_sha256);

        for transitive in &metadata.dependencies {
            self.resolve_dependencies_of_package_uri(
                transitive.package_uri.to_project_package_uri(),
                Some(&transitive.sha256),
            )?;
        }
        Ok(())
    }

    fn resolve_local_dependencies(
        &mut self,
        declared: &DeclaredDependenciesTree,
    ) -> Result<(), ModuleHostError> {
        let Some(ref package_uri) = declared.package_uri else {
            return Err(ModuleHostError::EvalError {
                message: format!(
                    "No package was declared in project `{}`.\n\nAdd a `package` section to the PklProject file.",
                    declared.project_file_uri
                )
                .into(),
                hint: None,
            });
        };
        let local_project_dir = project_dir_from_file_uri(&declared.project_file_uri);
        let rel_path = relativize_paths(&local_project_dir, &self.project.project_dir);
        let local_dep = ProjectDepEntry::Local {
            uri: package_uri.to_project_package_uri(),
            path: rel_path,
        };
        self.update_dependency(local_dep);
        self.build_resolved_dependencies(declared)?;
        Ok(())
    }

    fn update_dependency(&mut self, candidate: ProjectDepEntry) {
        let canonical = candidate.package_uri().to_canonical();
        if let Some((_, current)) = self
            .resolved_dependencies
            .iter_mut()
            .find(|(k, _)| k == &canonical)
        {
            if current.version() < candidate.version() {
                *current = candidate;
            }
        } else {
            self.resolved_dependencies.push((canonical, candidate));
        }
    }
}

fn project_dir_from_file_uri(file_uri: &str) -> PathBuf {
    let raw = file_uri
        .strip_prefix("file://")
        .or_else(|| file_uri.strip_prefix("file:"))
        .unwrap_or(file_uri);
    let decoded = crate::packages::zip::percent_decode(raw);
    let trimmed = {
        let b = decoded.as_bytes();
        if cfg!(windows)
            && b.len() >= 3
            && b[0] == b'/'
            && b[1].is_ascii_alphabetic()
            && b[2] == b':'
            && (b.len() == 3 || b[3] == b'/' || b[3] == b'\\')
        {
            &decoded[1..]
        } else {
            decoded.as_str()
        }
    };
    let path = PathBuf::from(trimmed);
    path.parent().map_or(path.clone(), Path::to_path_buf)
}

/// Computes forward-slash relative path from `base` to `target`, matching `IoUtils.relativize(target, base)`.
pub fn relativize_paths(target: &Path, base: &Path) -> String {
    let target_norm = normalize_components(target);
    let base_norm = normalize_components(base);

    let mut common = 0;
    while common < target_norm.len()
        && common < base_norm.len()
        && target_norm[common] == base_norm[common]
    {
        common += 1;
    }

    let mut parts: Vec<String> = Vec::new();
    for _ in common..base_norm.len() {
        parts.push("..".to_string());
    }
    for comp in &target_norm[common..] {
        parts.push(comp.clone());
    }
    if parts.is_empty() {
        ".".to_string()
    } else {
        parts.join("/")
    }
}

fn normalize_components(path: &Path) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut has_prefix = false;
    for comp in path.components() {
        match comp {
            Component::CurDir => {}
            Component::ParentDir => {
                let min_len = usize::from(has_prefix);
                if out.len() > min_len && out.last().is_some_and(|s| s != ".." && !s.is_empty()) {
                    out.pop();
                } else if !out.last().is_some_and(|s| s.is_empty()) {
                    out.push("..".to_string());
                }
            }
            Component::RootDir => {
                if has_prefix {
                    out.truncate(1);
                } else {
                    out.clear();
                }
                out.push(String::new());
            }
            Component::Prefix(p) => {
                out.clear();
                out.push(p.as_os_str().to_string_lossy().into_owned());
                has_prefix = true;
            }
            Component::Normal(s) => {
                out.push(s.to_string_lossy().into_owned());
            }
        }
    }
    out
}
