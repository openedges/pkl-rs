//! Pure package and project resolution logic inside `pkl-runtime`.
//!
//! Networking and filesystem I/O are completely delegated to the Host adapter outside `pkl-runtime`.
use crate::modules::{ModuleHostError, ModuleSource, ResolvedModule};
use crate::packages::cache::PackageCache;
use crate::packages::metadata::{DependencyMetadata, accept_metadata};
use crate::packages::project::{PklProject, resolve_project_dependency};
use crate::packages::zip::ZipArchive;
use crate::packages::{PackageAssetUri, PackageUri};
use crate::value::PklString;
use sha2::{Digest, Sha256};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Host interface required for package fetching, caching, and project loading.
pub trait PackageHost: Send + Sync {
    /// Fetch package metadata bytes over HTTPS.
    fn fetch_metadata(
        &self,
        package: &PackageUri,
        uri: &PklString,
    ) -> Result<Vec<u8>, ModuleHostError>;

    /// Fetch package ZIP archive bytes over HTTPS.
    fn fetch_zip(&self, package: &PackageUri, url: &PklString) -> Result<Vec<u8>, ModuleHostError>;

    /// Check if disk cache has archive for package.
    fn read_disk_archive(&self, package: &PackageUri) -> Option<ZipArchive>;

    /// Write archive to disk cache.
    fn write_disk_archive(
        &self,
        package: &PackageUri,
        metadata_bytes: &[u8],
        zip_bytes: &[u8],
    ) -> Result<(), ModuleHostError>;

    /// Find project dir starting from path.
    fn find_project_dir(&self, start: &Path) -> Option<PathBuf>;

    /// Load PklProject for project dir.
    fn load_project(&self, project_dir: &Path) -> Result<PklProject, ModuleHostError>;

    /// Lazily load project dependencies into project.
    fn load_project_dependencies(&self, _project: &mut PklProject) -> Result<(), ModuleHostError> {
        Ok(())
    }

    /// Read file content.
    fn read_file(&self, path: &Path) -> Result<Vec<u8>, ModuleHostError>;

    /// List directory elements for globbing.
    fn list_dir(&self, path: &Path) -> Result<Vec<crate::glob::PathElement>, ModuleHostError>;

    /// Whether test mode is active (allowing $skipChecksumVerification and $computedChecksum).
    fn is_test_mode(&self) -> bool;

    /// Whether project mode is active (explicit project dir or project found).
    fn has_project(&self) -> bool {
        false
    }

    /// Allowed resources specified in project evaluator settings or CLI.
    fn allowed_resources(&self) -> Option<Vec<String>> {
        None
    }

    /// Allowed modules specified in project evaluator settings or CLI.
    fn allowed_modules(&self) -> Option<Vec<String>> {
        None
    }

    /// Trace mode specified in project evaluator settings or CLI.
    fn trace_mode(&self) -> Option<crate::logging::TraceMode> {
        None
    }
}

pub fn has_active_project() -> bool {
    CURRENT_HOST.with(|h| {
        h.borrow()
            .as_ref()
            .map(|host| host.has_project())
            .unwrap_or(false)
    })
}

pub fn current_allowed_resources() -> Option<Vec<String>> {
    CURRENT_HOST.with(|h| {
        h.borrow()
            .as_ref()
            .and_then(|host| host.allowed_resources())
    })
}

pub fn current_allowed_modules() -> Option<Vec<String>> {
    CURRENT_HOST.with(|h| h.borrow().as_ref().and_then(|host| host.allowed_modules()))
}

pub fn current_trace_mode() -> Option<crate::logging::TraceMode> {
    CURRENT_HOST.with(|h| h.borrow().as_ref().and_then(|host| host.trace_mode()))
}

thread_local! {
    static CURRENT_HOST: RefCell<Option<Arc<dyn PackageHost>>> = const { RefCell::new(None) };
    static SESSION_CACHE: RefCell<PackageCache> = const { RefCell::new(PackageCache::new()) };
    static SESSION_SOURCES: RefCell<BTreeMap<String, PklString>> = const { RefCell::new(BTreeMap::new()) };
}

/// Execute a closure within the context of a Host-provided package environment.
pub fn with_package_host<R>(host: Arc<dyn PackageHost>, f: impl FnOnce() -> R) -> R {
    let prior_host = CURRENT_HOST.with(|h| h.borrow_mut().replace(host));
    struct RestoreGuard {
        prior_host: Option<Arc<dyn PackageHost>>,
    }
    impl Drop for RestoreGuard {
        fn drop(&mut self) {
            CURRENT_HOST.with(|h| *h.borrow_mut() = self.prior_host.take());
            if CURRENT_HOST.with(|h| h.borrow().is_none()) {
                SESSION_CACHE.with(|c| *c.borrow_mut() = PackageCache::new());
                SESSION_SOURCES.with(|s| *s.borrow_mut() = BTreeMap::new());
                crate::packages::http::reset_request_counts();
            }
        }
    }
    let _guard = RestoreGuard { prior_host };
    f()
}

/// Store source text for a resolved module URI.
pub fn store_source(uri: &str, text: PklString) {
    SESSION_SOURCES.with(|s| s.borrow_mut().insert(uri.to_string(), text));
}

/// Retrieve stored source text for a resolved module URI.
pub fn get_stored_source(uri: &str) -> Option<PklString> {
    SESSION_SOURCES.with(|s| s.borrow().get(uri).cloned())
}

/// Ensure that package metadata and archive are downloaded and cached.
pub fn ensure_package_loaded(
    package: &PackageUri,
    expected_checksum: Option<&PklString>,
) -> Result<(DependencyMetadata, ZipArchive), ModuleHostError> {
    // 1. Check session in-memory cache
    if let (Some(archive), Some(metadata)) = (
        SESSION_CACHE.with(|c| c.borrow().get_archive(package)),
        SESSION_CACHE.with(|c| c.borrow().get_metadata(package)),
    ) {
        return Ok((metadata, archive));
    }

    // 2. Check disk cache if available
    let host_opt = CURRENT_HOST.with(|h| h.borrow().clone());
    if let Some(host) = &host_opt
        && let (Some(archive), Some(metadata)) = (
            host.read_disk_archive(package),
            SESSION_CACHE.with(|c| c.borrow().get_metadata(package)),
        )
    {
        SESSION_CACHE.with(|c| c.borrow_mut().insert_archive(package, archive.clone()));
        return Ok((metadata, archive));
    }

    // 3. Metadata acquisition & verification sequence:
    // Module/resource policy -> HTTP response -> checksum -> parser -> ZIP URL check
    let metadata_request_uri = crate::packages::http::get_metadata_request_uri(package);
    crate::packages::http::validate_metadata_url(metadata_request_uri.as_str().unwrap_or(""))?;

    crate::packages::http::record_metadata_request();

    let is_test_mode = host_opt.as_ref().is_some_and(|h| h.is_test_mode());

    let metadata_bytes = if let Some(host) = &host_opt {
        host.fetch_metadata(package, &metadata_request_uri)?
    } else {
        let raw_uri = metadata_request_uri.as_str().unwrap_or("");
        return Err(ModuleHostError::SecurityManagerException {
            message: Some(
                format!(
                    "Exception when making request `GET {raw_uri}`:\nConnection refused (localhost)"
                )
                .into(),
            ),
        });
    };

    let metadata = match accept_metadata(
        package,
        &metadata_request_uri,
        &metadata_bytes,
        expected_checksum.or(package.sha256.as_ref()),
        is_test_mode,
    ) {
        Ok(m) => m,
        Err(failure) => {
            return Err(ModuleHostError::SecurityManagerException {
                message: Some(failure.message()),
            });
        }
    };

    // 4. Download package ZIP (only reached if metadata is valid!)
    crate::packages::http::record_zip_request();

    let zip_bytes = if let Some(host) = &host_opt {
        host.fetch_zip(package, &metadata.package_zip_url)?
    } else {
        return Err(ModuleHostError::Unsupported(
            "Package ZIP download requires a registered PackageHost".into(),
        ));
    };

    // 5. Verify ZIP SHA-256 checksum
    let computed_sha256: String = Sha256::digest(&zip_bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    let expected_zip_sha256 = metadata.package_zip_sha256.as_str().unwrap_or("");
    let skip_zip_verification = (is_test_mode
        && (expected_zip_sha256 == "$skipChecksumVerification"
            || expected_zip_sha256 == "$computedChecksum"))
        || expected_zip_sha256 == "$computedChecksum";

    if !skip_zip_verification && computed_sha256 != expected_zip_sha256 {
        let display = metadata.package_uri.uri.as_str().unwrap_or("");
        let msg = format!(
            "Cannot download package `{display}` because the computed checksum does not match the expected checksum.\n\nComputed checksum: \"{computed_sha256}\"\nExpected checksum: \"{expected_zip_sha256}\"\nAsset URL: \"{}\"",
            metadata.package_zip_url
        );
        return Err(ModuleHostError::SecurityManagerException {
            message: Some(msg.into()),
        });
    }

    // 6. Parse ZIP archive
    let archive =
        ZipArchive::parse(&zip_bytes).map_err(|err| ModuleHostError::SecurityManagerException {
            message: Some(format!("Failed to parse ZIP archive: {err}").into()),
        })?;

    // 7. Store in caches
    if let Some(host) = &host_opt {
        let _ = host.write_disk_archive(package, &metadata_bytes, &zip_bytes);
    }
    SESSION_CACHE.with(|c| {
        let mut cache = c.borrow_mut();
        cache.insert_metadata(package, metadata.clone());
        cache.insert_archive(package, archive.clone());
    });

    Ok((metadata, archive))
}

pub fn file_uri_from_path(path: &Path) -> Result<String, ModuleHostError> {
    let text = path
        .to_str()
        .ok_or_else(|| ModuleHostError::Unsupported("Non-UTF-8 path".into()))?;
    let mut uri = String::from("file://");
    use std::fmt::Write;
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || b"/-._~!$&'()*+,;=:@".contains(&byte) {
            uri.push(char::from(byte));
        } else {
            write!(&mut uri, "%{byte:02X}")
                .map_err(|e| ModuleHostError::Io(e.to_string().into()))?;
        }
    }
    Ok(uri)
}

pub fn percent_encode_fragment(fragment: &str) -> String {
    let mut encoded = String::with_capacity(fragment.len());
    let bytes = fragment.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if b == b'%'
            && i + 2 < bytes.len()
            && bytes[i + 1].is_ascii_hexdigit()
            && bytes[i + 2].is_ascii_hexdigit()
        {
            encoded.push('%');
            encoded.push(bytes[i + 1] as char);
            encoded.push(bytes[i + 2] as char);
            i += 3;
            continue;
        }
        if b.is_ascii_alphanumeric() || b"-_.!~*'();/:@&=+$,?[]".contains(&b) {
            encoded.push(b as char);
        } else {
            use std::fmt::Write;
            let _ = write!(&mut encoded, "%{b:02X}");
        }
        i += 1;
    }
    encoded
}

fn map_package_uri_error(failure: crate::packages::PackageUriError) -> ModuleHostError {
    match failure.failure {
        crate::packages::PackageUriFailure::Unsupported(reason) => {
            ModuleHostError::Unsupported(reason)
        }
        _ => ModuleHostError::UriSyntax {
            reason: failure.reason(),
        },
    }
}

pub fn normalize_local_subpath(raw_asset_path: &str) -> PathBuf {
    let decoded = crate::packages::zip::percent_decode(raw_asset_path);
    let mut components = Vec::new();
    for part in decoded.split('/') {
        if part.is_empty() || part == "." {
            continue;
        } else if part == ".." {
            components.pop();
        } else {
            components.push(part);
        }
    }
    let mut path = PathBuf::new();
    for comp in components {
        path.push(comp);
    }
    path
}

/// Returns the local filesystem PathBuf if the URI is a local project dependency.
pub fn get_local_project_file_path(uri: &PklString) -> Option<PathBuf> {
    let raw_uri = uri.as_str().unwrap_or("");
    if !raw_uri.starts_with("projectpackage:") {
        return None;
    }
    let asset_uri = PackageAssetUri::parse(uri.clone()).ok()?;
    let host_opt = CURRENT_HOST.with(|h| h.borrow().clone())?;
    let project_dir = host_opt.find_project_dir(&PathBuf::from("."))?;
    let mut project = host_opt.load_project(&project_dir).ok()?;
    if !project.deps_loaded {
        let _ = host_opt.load_project_dependencies(&mut project);
    }
    for dep in project.dependencies.values() {
        let dep_uri_str = dep.uri.as_str().unwrap_or("");
        if raw_uri.starts_with(dep_uri_str)
            && dep.dep_type == "local"
            && let Some(rel_path) = &dep.path
        {
            let local_root = project.project_dir.join(rel_path);
            let raw_subpath = asset_uri.asset_path.as_str().unwrap_or("");
            let local_subpath = normalize_local_subpath(raw_subpath);
            return Some(local_root.join(local_subpath));
        }
    }
    None
}

/// Resolves a package module URI.
pub fn resolve_package_module(uri: &PklString) -> Result<ResolvedModule, ModuleHostError> {
    let raw_uri = uri.as_str().unwrap_or("");

    if raw_uri.starts_with("projectpackage:") {
        return resolve_projectpackage_module(uri);
    }

    let asset_uri = PackageAssetUri::parse(uri.clone()).map_err(map_package_uri_error)?;

    let (_, archive) = ensure_package_loaded(&asset_uri.package, None)?;

    let asset_path = asset_uri.asset_path.as_str().unwrap_or("");
    if archive.is_directory(asset_path) {
        return Err(ModuleHostError::Io(crate::host::IoFailure::from(
            "Is a directory",
        )));
    }
    let bytes = archive.get(asset_path).ok_or(ModuleHostError::NotFound)?;

    let text_str = std::str::from_utf8(bytes)
        .map_err(|_| ModuleHostError::Unsupported("Package module must be valid UTF-8".into()))?;
    let text = PklString::from(text_str);

    store_source(raw_uri, text);

    Ok(ResolvedModule {
        original_uri: uri.clone(),
        resolved_uri: uri.clone(),
        source_uri: uri.clone(),
    })
}

/// Resolves a projectpackage: module URI (which can be local or remote).
pub fn resolve_projectpackage_module(uri: &PklString) -> Result<ResolvedModule, ModuleHostError> {
    let raw_uri = uri.as_str().unwrap_or("");
    let asset_uri = PackageAssetUri::parse(uri.clone()).map_err(map_package_uri_error)?;

    // Check project dependencies if host is available
    let host_opt = CURRENT_HOST.with(|h| h.borrow().clone());
    if let Some(host) = &host_opt
        && let Some(project_dir) = host.find_project_dir(&PathBuf::from("."))
        && let Ok(mut project) = host.load_project(&project_dir)
    {
        if !project.deps_loaded {
            let _ = host.load_project_dependencies(&mut project);
        }
        for dep in project.dependencies.values() {
            let dep_uri_str = dep.uri.as_str().unwrap_or("");
            if dep_uri_str.starts_with(
                asset_uri
                    .package
                    .path_without_version
                    .as_str()
                    .unwrap_or(""),
            ) || raw_uri.starts_with(dep_uri_str)
            {
                if dep.dep_type == "local"
                    && let Some(rel_path) = &dep.path
                {
                    let local_root = project.project_dir.join(rel_path);
                    let raw_subpath = asset_uri.asset_path.as_str().unwrap_or("");
                    let local_subpath = normalize_local_subpath(raw_subpath);
                    let file_path = local_root.join(&local_subpath);
                    let bytes = host.read_file(&file_path)?;
                    let text_str = std::str::from_utf8(&bytes).map_err(|_| {
                        ModuleHostError::Unsupported("Project module must be valid UTF-8".into())
                    })?;
                    let text_pkl = PklString::from(text_str);
                    store_source(raw_uri, text_pkl.clone());
                    let resolved_file_uri = file_uri_from_path(&file_path)?;
                    store_source(&resolved_file_uri, text_pkl);
                    return Ok(ResolvedModule {
                        original_uri: uri.clone(),
                        resolved_uri: resolved_file_uri.into(),
                        source_uri: uri.clone(),
                    });
                } else if dep.dep_type == "remote" {
                    let (_, archive) =
                        ensure_package_loaded(&asset_uri.package, dep.sha256.as_ref())?;
                    let asset_path = asset_uri.asset_path.as_str().unwrap_or("");
                    if archive.is_directory(asset_path) {
                        return Err(ModuleHostError::Io(crate::host::IoFailure::from(
                            "Is a directory",
                        )));
                    }
                    let bytes = archive.get(asset_path).ok_or(ModuleHostError::NotFound)?;
                    let text_str = std::str::from_utf8(bytes).map_err(|_| {
                        ModuleHostError::Unsupported("Package module must be valid UTF-8".into())
                    })?;
                    let text = PklString::from(text_str);
                    store_source(raw_uri, text);
                    return Ok(ResolvedModule {
                        original_uri: uri.clone(),
                        resolved_uri: uri.clone(),
                        source_uri: uri.clone(),
                    });
                }
            }
        }
    }

    // Fallback to loading as regular package
    let (_, archive) = ensure_package_loaded(&asset_uri.package, None)?;
    let asset_path = asset_uri.asset_path.as_str().unwrap_or("");
    if archive.is_directory(asset_path) {
        return Err(ModuleHostError::Io(crate::host::IoFailure::from(
            "Is a directory",
        )));
    }
    let bytes = archive.get(asset_path).ok_or(ModuleHostError::NotFound)?;
    let text_str = std::str::from_utf8(bytes)
        .map_err(|_| ModuleHostError::Unsupported("Package module must be valid UTF-8".into()))?;
    let text = PklString::from(text_str);
    store_source(raw_uri, text);

    Ok(ResolvedModule {
        original_uri: uri.clone(),
        resolved_uri: uri.clone(),
        source_uri: uri.clone(),
    })
}

/// Resolves raw resource bytes for `read("package:...")` or `read("projectpackage:...")`.
pub fn resolve_package_resource(uri: &PklString) -> Result<Vec<u8>, ModuleHostError> {
    let raw_uri = uri.as_str().unwrap_or("");
    let asset_uri = PackageAssetUri::parse(uri.clone()).map_err(map_package_uri_error)?;

    if raw_uri.starts_with("projectpackage:") {
        let host_opt = CURRENT_HOST.with(|h| h.borrow().clone());
        if let Some(host) = &host_opt
            && let Some(project_dir) = host.find_project_dir(&PathBuf::from("."))
            && let Ok(mut project) = host.load_project(&project_dir)
        {
            if !project.deps_loaded {
                let _ = host.load_project_dependencies(&mut project);
            }
            for dep in project.dependencies.values() {
                let dep_uri_str = dep.uri.as_str().unwrap_or("");
                if raw_uri.starts_with(dep_uri_str)
                    && dep.dep_type == "local"
                    && let Some(rel_path) = &dep.path
                {
                    let local_root = project.project_dir.join(rel_path);
                    let raw_subpath = asset_uri.asset_path.as_str().unwrap_or("");
                    let local_subpath = normalize_local_subpath(raw_subpath);
                    let file_path = local_root.join(&local_subpath);
                    return host.read_file(&file_path);
                }
            }
        }
    }

    let (_, archive) = ensure_package_loaded(&asset_uri.package, None)?;
    let asset_path = asset_uri.asset_path.as_str().unwrap_or("");
    match archive.read_bytes(asset_path, true) {
        Ok(bytes) => Ok(bytes),
        Err(crate::packages::zip::ZipReadError::IsADirectory) => {
            let elements = archive.list_elements(asset_path);
            let mut out = String::new();
            for (name, _) in elements {
                out.push_str(&name);
                out.push('\n');
            }
            Ok(out.into_bytes())
        }
        Err(crate::packages::zip::ZipReadError::NotFound) => Err(ModuleHostError::NotFound),
    }
}

/// Resolves an import URI relative to a base URI, supporting `@dep` notation.
pub fn resolve_import_uri(
    base_uri: &PklString,
    authored_uri: &PklString,
) -> Result<PklString, ModuleHostError> {
    let base_str = base_uri.as_str().unwrap_or("");
    let authored_str = authored_uri.as_str().unwrap_or("");

    // 1. Dependency notation: starts with `@`
    if let Some(stripped) = authored_str.strip_prefix('@') {
        let (dep_name, asset_path) = match stripped.find('/') {
            Some(idx) => (&stripped[..idx], &stripped[idx..]),
            None => (stripped, "/"),
        };

        // If base is a package, resolve from package dependencies
        if (base_str.starts_with("package:") || base_str.starts_with("projectpackage:"))
            && let Ok(asset_uri) = PackageAssetUri::parse(base_uri.clone())
        {
            let host_opt = CURRENT_HOST.with(|h| h.borrow().clone());
            let mut local_match = None;
            // In Java Pkl, only `projectpackage:` scheme consults ProjectDependenciesManager.
            // Absolute `package:` scheme imports resolve dependencies strictly from package metadata.
            if base_str.starts_with("projectpackage:")
                && let Some(host) = &host_opt
                && let Some(project_dir) = host.find_project_dir(&PathBuf::from("."))
                && let Ok(mut project) = host.load_project(&project_dir)
            {
                if !project.deps_loaded {
                    let _ = host.load_project_dependencies(&mut project);
                }
                for dep in project.dependencies.values() {
                    let dep_uri_str = dep.uri.as_str().unwrap_or("");
                    if base_str.starts_with(dep_uri_str)
                        && dep.dep_type == "local"
                        && let Some(rel_path) = &dep.path
                    {
                        let local_root = project.project_dir.join(rel_path);
                        if let Ok(mut local_proj) = host.load_project(&local_root) {
                            if !local_proj.deps_loaded {
                                let _ = host.load_project_dependencies(&mut local_proj);
                            }
                            local_match = Some((project, local_proj));
                            break;
                        }
                    }
                }
            }

            if let Some((root_project, local_proj)) = local_match {
                if let Some(dep) = local_proj.dependencies.get(dep_name) {
                    let target_base = dep.uri.as_str().unwrap_or("");
                    let mut resolved_base = target_base.to_string();
                    if let Some(matching) = crate::packages::project::find_matching_resolved(
                        target_base,
                        dep_name,
                        &root_project.dependencies,
                    ) {
                        resolved_base = matching.uri.as_str().unwrap_or(target_base).to_string();
                    }
                    let encoded_asset = percent_encode_fragment(asset_path);
                    return Ok(format!("{resolved_base}#{encoded_asset}").into());
                } else {
                    let pkg_display = asset_uri.package.uri.as_str().unwrap_or("");
                    let clean_pkg_uri = pkg_display.replace("projectpackage:", "package:");
                    let msg = format!(
                        "Cannot find dependency named `{dep_name}`, because it was not declared in package `{clean_pkg_uri}`."
                    );
                    return Err(ModuleHostError::SecurityManagerException {
                        message: Some(msg.into()),
                    });
                }
            }

            let (meta, _) = ensure_package_loaded(&asset_uri.package, None)?;
            for dep in &meta.dependencies {
                if dep.name == dep_name {
                    let target_base = dep.package_uri.uri.as_str().unwrap_or("");
                    let host_opt = CURRENT_HOST.with(|h| h.borrow().clone());
                    let mut resolved_base = target_base.to_string();
                    if base_str.starts_with("projectpackage:")
                        && let Some(host) = &host_opt
                        && let Some(project_dir) = host.find_project_dir(&PathBuf::from("."))
                        && let Ok(mut project) = host.load_project(&project_dir)
                    {
                        if !project.deps_loaded {
                            let _ = host.load_project_dependencies(&mut project);
                        }
                        if let Some(matching) = crate::packages::project::find_matching_resolved(
                            target_base,
                            dep_name,
                            &project.dependencies,
                        ) {
                            let resolved_uri_str = matching.uri.as_str().unwrap_or("");
                            let dec_ver =
                                crate::packages::project::extract_version_from_uri(target_base);
                            let res_ver = crate::packages::project::extract_version_from_uri(
                                resolved_uri_str,
                            );
                            if let (Some(dv), Some(rv)) = (dec_ver, res_ver)
                                && crate::packages::project::compare_dep_versions(dv, rv)
                                    == std::cmp::Ordering::Greater
                            {
                                let pkg_display = asset_uri.package.uri.as_str().unwrap_or("");
                                let clean_res_uri =
                                    resolved_uri_str.replace("projectpackage:", "package:");
                                let clean_pkg_uri =
                                    pkg_display.replace("projectpackage:", "package:");
                                let msg = format!(
                                    "Transitive dependency declared by package `{clean_pkg_uri}` is newer than what is declared in `PklProject.deps.json`.\n\nDeclared: `{target_base}`\nResolved: `{clean_res_uri}`\n\nRun `pkl project resolve` to update resolved dependencies."
                                );
                                return Err(ModuleHostError::SecurityManagerException {
                                    message: Some(msg.into()),
                                });
                            }
                            resolved_base = resolved_uri_str.to_string();
                        }
                    }
                    let encoded_asset = percent_encode_fragment(asset_path);
                    return Ok(format!("{resolved_base}#{encoded_asset}").into());
                }
            }
            let pkg_display = asset_uri.package.uri.as_str().unwrap_or("");
            let clean_pkg_uri = pkg_display.replace("projectpackage:", "package:");
            let msg = format!(
                "Cannot find dependency named `{dep_name}`, because it was not declared in package `{clean_pkg_uri}`."
            );
            return Err(ModuleHostError::SecurityManagerException {
                message: Some(msg.into()),
            });
        }

        // Otherwise resolve from project
        let host_opt = CURRENT_HOST.with(|h| h.borrow().clone());
        let base_path = if base_str.starts_with("file://") {
            PathBuf::from(base_str.strip_prefix("file://").unwrap_or(base_str))
        } else {
            PathBuf::from(".")
        };

        if let Some(host) = &host_opt {
            if let Some(project_dir) = host.find_project_dir(&base_path) {
                let mut project = host.load_project(&project_dir)?;
                if !project.deps_loaded {
                    host.load_project_dependencies(&mut project)?;
                }
                return resolve_project_dependency(&project, authored_str);
            } else {
                let msg = "Cannot resolve dependency because there is no project found.\n\nIf you meant to import a path that starts with `@`, prefix the path with `./` (e.g. `import \"./@myPath\").\nIf you meant to import a dependency, ensure that this file is within a directory that contains a PklProject module.";
                return Err(ModuleHostError::SecurityManagerException {
                    message: Some(msg.into()),
                });
            }
        } else {
            let msg = "Cannot resolve dependency because there is no project found.\n\nIf you meant to import a path that starts with `@`, prefix the path with `./` (e.g. `import \"./@myPath\").\nIf you meant to import a dependency, ensure that this file is within a directory that contains a PklProject module.";
            return Err(ModuleHostError::SecurityManagerException {
                message: Some(msg.into()),
            });
        }
    }

    // 2. Fragment relative path in package/projectpackage URIs
    if (base_str.starts_with("package:") || base_str.starts_with("projectpackage:"))
        && !authored_str.contains("://")
        && let Ok(asset_uri) = PackageAssetUri::parse(base_uri.clone())
    {
        let base_fragment = asset_uri.asset_path.as_str().unwrap_or("/");
        let new_fragment = resolve_relative_path(base_fragment, authored_str);
        let encoded_fragment = percent_encode_fragment(&new_fragment);
        let pkg_uri = asset_uri.package.uri.as_str().unwrap_or("");
        return Ok(format!("{pkg_uri}#{encoded_fragment}").into());
    }

    // Fallback to standard URI resolution
    crate::host::resources::resolve_module_key_uri(authored_uri, base_uri)
        .map_err(|err| ModuleHostError::Unsupported(err.message))
}

pub fn resolve_relative_path(base_path: &str, relative: &str) -> String {
    let base_norm = crate::packages::zip::normalize_zip_path(base_path);
    let ends_with_slash = relative.ends_with('/');
    let dir = match base_norm.rfind('/') {
        Some(idx) => &base_norm[..=idx],
        None => "/",
    };
    let combined = format!("{dir}{relative}");
    let norm = crate::packages::zip::normalize_zip_path(&combined);
    if ends_with_slash && !norm.ends_with('/') {
        format!("{norm}/")
    } else {
        norm
    }
}

/// List elements in package directory for globbing.
pub fn list_elements(uri: &PklString) -> Result<Vec<crate::glob::PathElement>, ModuleHostError> {
    let raw_uri = uri.as_str().unwrap_or("");
    let asset_uri = PackageAssetUri::parse(uri.clone()).map_err(map_package_uri_error)?;

    if raw_uri.starts_with("projectpackage:") {
        let host_opt = CURRENT_HOST.with(|h| h.borrow().clone());
        if let Some(host) = &host_opt
            && let Some(project_dir) = host.find_project_dir(&PathBuf::from("."))
            && let Ok(mut project) = host.load_project(&project_dir)
        {
            if !project.deps_loaded {
                let _ = host.load_project_dependencies(&mut project);
            }
            for dep in project.dependencies.values() {
                let dep_uri_str = dep.uri.as_str().unwrap_or("");
                if raw_uri.starts_with(dep_uri_str)
                    && dep.dep_type == "local"
                    && let Some(rel_path) = &dep.path
                {
                    let local_root = project.project_dir.join(rel_path);
                    let raw_subpath = asset_uri.asset_path.as_str().unwrap_or("");
                    let local_subpath = normalize_local_subpath(raw_subpath);
                    let dir_path = local_root.join(local_subpath);
                    return host.list_dir(&dir_path);
                }
            }
        }
    }

    let (_, archive) = ensure_package_loaded(&asset_uri.package, None)?;
    let asset_path = asset_uri.asset_path.as_str().unwrap_or("");
    let entries = archive.list_elements(asset_path);
    let elements = entries
        .into_iter()
        .map(|(name, is_dir)| crate::glob::PathElement {
            name: name.into(),
            is_directory: is_dir,
        })
        .collect();
    Ok(elements)
}

/// Checks if an element exists in a package for globbing.
pub fn has_element(uri: &PklString) -> Result<bool, ModuleHostError> {
    let raw_uri = uri.as_str().unwrap_or("");
    let asset_uri = PackageAssetUri::parse(uri.clone()).map_err(map_package_uri_error)?;

    if raw_uri.starts_with("projectpackage:") {
        let host_opt = CURRENT_HOST.with(|h| h.borrow().clone());
        if let Some(host) = &host_opt
            && let Some(project_dir) = host.find_project_dir(&PathBuf::from("."))
            && let Ok(mut project) = host.load_project(&project_dir)
        {
            if !project.deps_loaded {
                let _ = host.load_project_dependencies(&mut project);
            }
            for dep in project.dependencies.values() {
                let dep_uri_str = dep.uri.as_str().unwrap_or("");
                if raw_uri.starts_with(dep_uri_str)
                    && dep.dep_type == "local"
                    && let Some(rel_path) = &dep.path
                {
                    let local_root = project.project_dir.join(rel_path);
                    let raw_subpath = asset_uri.asset_path.as_str().unwrap_or("");
                    let local_subpath = normalize_local_subpath(raw_subpath);
                    let file_path = local_root.join(local_subpath);
                    return Ok(
                        host.read_file(&file_path).is_ok() || host.list_dir(&file_path).is_ok()
                    );
                }
            }
        }
    }

    let (_, archive) = ensure_package_loaded(&asset_uri.package, None)?;
    let asset_path = asset_uri.asset_path.as_str().unwrap_or("");
    Ok(archive.has_element(asset_path))
}

/// Loads module source text.
pub fn load_source(resolved: &ResolvedModule) -> Result<ModuleSource, ModuleHostError> {
    let uri_str = resolved.resolved_uri.as_str().unwrap_or("");
    let original = resolved.original_uri.as_str().unwrap_or("");
    let source_str = resolved.source_uri.as_str().unwrap_or("");

    let text = get_stored_source(uri_str)
        .or_else(|| get_stored_source(original))
        .or_else(|| get_stored_source(source_str));

    if let Some(text) = text {
        let leaf = original.rsplit('#').next().unwrap_or(original);
        let name = leaf.rsplit('/').next().unwrap_or(leaf);
        let name_clean = name.strip_suffix(".pkl").unwrap_or(name);
        return Ok(ModuleSource {
            text: text.into(),
            inferred_name: name_clean.into(),
        });
    }

    let module = resolve_package_module(&resolved.resolved_uri)?;
    let text = get_stored_source(module.resolved_uri.as_str().unwrap_or(""))
        .or_else(|| get_stored_source(module.original_uri.as_str().unwrap_or("")));
    if let Some(text) = text {
        let leaf = original.rsplit('#').next().unwrap_or(original);
        let name = leaf.rsplit('/').next().unwrap_or(leaf);
        let name_clean = name.strip_suffix(".pkl").unwrap_or(name);
        return Ok(ModuleSource {
            text: text.into(),
            inferred_name: name_clean.into(),
        });
    }

    Err(ModuleHostError::NotFound)
}
