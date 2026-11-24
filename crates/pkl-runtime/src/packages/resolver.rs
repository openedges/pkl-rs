//! Pure package and project resolution logic inside `pkl-runtime`.
//!
//! Provides both:
//! - First-class, session-owned [`PackageResolver`] (zero `thread_local!`, Wasm-pure,
//!   supports disk-cache metadata+ZIP reads, corrupt-cache eviction, strict checksums,
//!   `download_package`, `close()`, and [`MetadataChecksumProvider`] for MVS resolution).
//! - Thin compatibility shims for M2 probes/tests (`with_package_host`, etc.) until Wave 2
//!   unifies `module_eval_probe.rs` onto the session-owned resolver.
use crate::modules::{ModuleHostError, ModuleSource, ResolvedModule};
use crate::packages::cache::{PackageCache, PackageCacheStore};
use crate::packages::http::{PackageTransport, RequestMetrics};
use crate::packages::metadata::{
    DependencyMetadata, accept_metadata, parse_metadata_bytes, sha256_hex,
};
use crate::packages::mvs::MetadataChecksumProvider;
use crate::packages::project::{PklProject, resolve_project_dependency};
use crate::packages::zip::ZipArchive;
use crate::packages::{CanonicalPackageUri, PackageAssetUri, PackageUri};
use crate::value::PklString;
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};

// ---------------------------------------------------------------------------
// First-class, session-owned, Wasm-pure PackageResolver (M4)
// ---------------------------------------------------------------------------

/// Session-owned package resolver matching `PackageResolvers.{InMemory,DiskCached}PackageResolver`
/// (`PackageResolvers.java:67-687`).
///
/// Uses zero `thread_local!` state, zero `unsafe`, and zero OS I/O (delegated via
/// [`PackageTransport`] and optional [`PackageCacheStore`]).
pub struct PackageResolver {
    transport: Arc<dyn PackageTransport>,
    cache_store: Option<Arc<dyn PackageCacheStore>>,
    test_mode: bool,
    metrics: RequestMetrics,
    memory_cache: RwLock<PackageCache>,
    closed: AtomicBool,
}

impl PackageResolver {
    /// Creates an in-memory-only resolver (`--no-cache` or `ProjectPackager` publish check).
    pub fn in_memory(transport: Arc<dyn PackageTransport>, test_mode: bool) -> Self {
        Self {
            transport,
            cache_store: None,
            test_mode,
            metrics: RequestMetrics::new(),
            memory_cache: RwLock::new(PackageCache::new()),
            closed: AtomicBool::new(false),
        }
    }

    /// Creates a disk-cached resolver backed by `cache_store`.
    pub fn disk_cached(
        transport: Arc<dyn PackageTransport>,
        cache_store: Arc<dyn PackageCacheStore>,
        test_mode: bool,
    ) -> Self {
        Self {
            transport,
            cache_store: Some(cache_store),
            test_mode,
            metrics: RequestMetrics::new(),
            memory_cache: RwLock::new(PackageCache::new()),
            closed: AtomicBool::new(false),
        }
    }

    pub fn metrics(&self) -> &RequestMetrics {
        &self.metrics
    }

    pub fn has_persistent_cache(&self) -> bool {
        self.cache_store.is_some()
    }

    pub fn is_closed(&self) -> bool {
        self.closed.load(Ordering::Acquire)
    }

    /// Closes this resolver and releases all in-memory archive caches (`PackageResolvers.java:677-686`).
    pub fn close(&self) {
        self.closed.store(true, Ordering::Release);
        self.memory_cache
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .clear();
    }

    fn ensure_open(&self) -> Result<(), ModuleHostError> {
        if self.is_closed() {
            return Err(ModuleHostError::Io(
                "Package resolver has been closed.".into(),
            ));
        }
        Ok(())
    }

    /// Loads `DependencyMetadata` (and optionally returns raw metadata bytes) obeying
    /// Pkl 0.32 disk-cache precedence, checksum checks, and corrupt-`.json` eviction.
    pub fn get_dependency_metadata(
        &self,
        package: &PackageUri,
        expected_checksum: Option<&PklString>,
    ) -> Result<DependencyMetadata, ModuleHostError> {
        let (meta, _) = self.load_metadata_and_bytes(package, expected_checksum, true)?;
        Ok(meta)
    }

    /// Loads `DependencyMetadata` together with the SHA-256 hex digest of its raw JSON bytes,
    /// matching `PackageResolvers.getDependencyMetadataAndComputeChecksum`.
    pub fn get_dependency_metadata_and_compute_checksum(
        &self,
        package: &PackageUri,
        expected_checksum: Option<&PklString>,
    ) -> Result<(DependencyMetadata, PklString), ModuleHostError> {
        let (meta, raw_bytes) = self.load_metadata_and_bytes(package, expected_checksum, true)?;
        let digest = PklString::from(sha256_hex(&raw_bytes).as_str());
        Ok((meta, digest))
    }

    fn load_metadata_and_bytes(
        &self,
        package: &PackageUri,
        expected_checksum: Option<&PklString>,
        write_disk_cache: bool,
    ) -> Result<(DependencyMetadata, Vec<u8>), ModuleHostError> {
        self.ensure_open()?;
        let request_uri = crate::packages::http::get_metadata_request_uri(package);
        let effective_expected = expected_checksum.or(package.sha256.as_ref());
        {
            let guard = self.memory_cache.read().unwrap_or_else(|e| e.into_inner());
            if let (Some(meta), Some(bytes)) = (
                guard.get_metadata(package),
                guard.get_metadata_bytes(package),
            ) {
                if let Some(expected) = effective_expected {
                    accept_metadata(
                        package,
                        &request_uri,
                        &bytes,
                        Some(expected),
                        self.test_mode,
                    )
                    .map_err(|failure| {
                        ModuleHostError::SecurityManagerException {
                            message: Some(failure.message()),
                        }
                    })?;
                }
                return Ok((meta, bytes));
            }
        }

        // 1. Check persistent disk cache first (`PackageResolvers.java:512-556`)
        if let Some(store) = &self.cache_store
            && let Some(cached_bytes) = store.read_metadata_bytes(package)?
        {
            match parse_metadata_bytes(package, &request_uri, &cached_bytes) {
                Ok(meta) => {
                    let mut guard = self.memory_cache.write().unwrap_or_else(|e| e.into_inner());
                    guard.insert_metadata(package, meta.clone());
                    guard.insert_metadata_bytes(package, cached_bytes.clone());
                    return Ok((meta, cached_bytes));
                }
                Err(failure) => {
                    // Delete corrupt cached `.json` file matching `PackageResolvers.java:544,553`
                    let _ = store.delete_metadata(package);
                    return Err(ModuleHostError::SecurityManagerException {
                        message: Some(failure.message()),
                    });
                }
            }
        }

        // 2. Cache miss -> fetch over HTTPS & verify metadata checksum BEFORE caching/parsing
        crate::packages::http::validate_metadata_url(request_uri.as_str().unwrap_or(""))?;
        self.metrics.record_metadata();
        let metadata_bytes = self.transport.fetch_metadata(package, &request_uri)?;

        let meta = accept_metadata(
            package,
            &request_uri,
            &metadata_bytes,
            effective_expected,
            self.test_mode,
        )
        .map_err(|failure| ModuleHostError::SecurityManagerException {
            message: Some(failure.message()),
        })?;

        if write_disk_cache && let Some(store) = &self.cache_store {
            store.write_metadata_bytes(package, &metadata_bytes)?;
        }
        {
            let mut guard = self.memory_cache.write().unwrap_or_else(|e| e.into_inner());
            guard.insert_metadata(package, meta.clone());
            guard.insert_metadata_bytes(package, metadata_bytes.clone());
        }
        Ok((meta, metadata_bytes))
    }

    /// Loads and verifies the package ZIP archive (never skips ZIP SHA-256 verification).
    pub fn get_archive(
        &self,
        package: &PackageUri,
        expected_checksum: Option<&PklString>,
    ) -> Result<(DependencyMetadata, ZipArchive), ModuleHostError> {
        self.ensure_open()?;
        let metadata = self.get_dependency_metadata(package, expected_checksum)?;

        if let Some(archive) = self
            .memory_cache
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .get_archive(package)
        {
            return Ok((metadata, archive));
        }

        // 1. Check persistent disk cache (corrupt cached ZIP propagates error!)
        if let Some(store) = &self.cache_store
            && let Some(archive) = store.read_zip_archive(package)?
        {
            self.memory_cache
                .write()
                .unwrap_or_else(|e| e.into_inner())
                .insert_archive(package, archive.clone());
            return Ok((metadata, archive));
        }

        // 2. Download ZIP over HTTPS & strictly verify SHA-256 (`PackageResolvers.java:181-194`)
        self.metrics.record_zip();
        let zip_bytes = self
            .transport
            .fetch_zip(package, &metadata.package_zip_url)?;

        verify_zip_checksum(package, &metadata, &zip_bytes)?;

        // 3. Strictly parse ZIP (CRC-32, decompressed length, EOCD/CD bounds)
        let archive = ZipArchive::parse(&zip_bytes).map_err(|err| {
            ModuleHostError::Io(format!("Failed to parse ZIP archive: {err}").into())
        })?;

        if let Some(store) = &self.cache_store {
            store.write_zip_bytes(package, &zip_bytes)?;
        }
        self.memory_cache
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .insert_archive(package, archive.clone());

        Ok((metadata, archive))
    }

    /// Reads an asset file or directory listing inside a package.
    pub fn get_bytes(
        &self,
        asset_uri: &PackageAssetUri,
        allow_directories: bool,
        expected_checksum: Option<&PklString>,
    ) -> Result<Vec<u8>, ModuleHostError> {
        let (_, archive) = self.get_archive(&asset_uri.package, expected_checksum)?;
        let asset_path = asset_uri.asset_path.as_str().unwrap_or("");
        match archive.read_bytes(asset_path, allow_directories) {
            Ok(bytes) => Ok(bytes),
            Err(crate::packages::zip::ZipReadError::IsADirectory) => Err(ModuleHostError::Io(
                crate::host::IoFailure::from("Is a directory"),
            )),
            Err(crate::packages::zip::ZipReadError::NotFound) => Err(ModuleHostError::NotFound),
        }
    }

    pub fn has_element(
        &self,
        asset_uri: &PackageAssetUri,
        expected_checksum: Option<&PklString>,
    ) -> Result<bool, ModuleHostError> {
        let (_, archive) = self.get_archive(&asset_uri.package, expected_checksum)?;
        Ok(archive.has_element(asset_uri.asset_path.as_str().unwrap_or("")))
    }

    pub fn list_elements(
        &self,
        asset_uri: &PackageAssetUri,
        expected_checksum: Option<&PklString>,
    ) -> Result<Vec<crate::glob::PathElement>, ModuleHostError> {
        let (_, archive) = self.get_archive(&asset_uri.package, expected_checksum)?;
        let entries = archive.list_elements(asset_uri.asset_path.as_str().unwrap_or(""));
        Ok(entries
            .into_iter()
            .map(|(name, is_directory)| crate::glob::PathElement {
                name: name.into(),
                is_directory,
            })
            .collect())
    }

    /// Downloads a package (and optionally its transitive dependencies) into the persistent cache
    /// (`PackageResolvers.java:610-620`, `CliPackageDownloader.kt:24-63`).
    pub fn download_package(
        &self,
        package: &PackageUri,
        expected_checksum: Option<&PklString>,
        no_transitive: bool,
    ) -> Result<Vec<PackageUri>, ModuleHostError> {
        self.ensure_open()?;
        if self.cache_store.is_none() {
            return Err(ModuleHostError::EvalError {
                message: "Cannot download packages because caching is disabled.".into(),
                hint: None,
            });
        }
        let mut visited = BTreeSet::new();
        let mut downloaded = Vec::new();
        self.download_recursive(
            package,
            expected_checksum,
            no_transitive,
            &mut visited,
            &mut downloaded,
        )?;
        Ok(downloaded)
    }

    fn download_recursive(
        &self,
        package: &PackageUri,
        expected_checksum: Option<&PklString>,
        no_transitive: bool,
        visited: &mut BTreeSet<PackageUri>,
        downloaded: &mut Vec<PackageUri>,
    ) -> Result<(), ModuleHostError> {
        let key = package.to_external_package_uri().without_checksums();
        if !visited.insert(key) {
            return Ok(());
        }
        let (metadata, _) = self.get_archive(package, expected_checksum)?;
        downloaded.push(package.to_external_package_uri().without_checksums());
        if !no_transitive {
            for dep in &metadata.dependencies {
                let dep_expected = if dep.sha256.is_empty() {
                    None
                } else {
                    Some(&dep.sha256)
                };
                self.download_recursive(
                    &dep.package_uri,
                    dep_expected,
                    false,
                    visited,
                    downloaded,
                )?;
            }
        }
        Ok(())
    }
}

impl MetadataChecksumProvider for PackageResolver {
    fn get_dependency_metadata_and_checksum(
        &self,
        package_uri: &PackageUri,
    ) -> Result<(DependencyMetadata, PklString), ModuleHostError> {
        // Matches `PackageResolvers.java:102-107, 623-632`: read disk cache if present,
        // otherwise fetch over HTTPS without writing `.json` back into disk cache.
        let (meta, raw_bytes) = self.load_metadata_and_bytes(package_uri, None, false)?;
        let digest = PklString::from(sha256_hex(&raw_bytes).as_str());
        Ok((meta, digest))
    }
}

fn verify_zip_checksum(
    package: &PackageUri,
    metadata: &DependencyMetadata,
    zip_bytes: &[u8],
) -> Result<(), ModuleHostError> {
    let computed_sha256 = sha256_hex(zip_bytes);
    let expected_zip_sha256 = metadata.package_zip_sha256.as_str().unwrap_or("");
    if computed_sha256 != expected_zip_sha256 {
        let display = package.display_name();
        let msg = format!(
            "Cannot download package `{display}` because the computed checksum does not match the expected checksum.\n\nComputed checksum: \"{computed_sha256}\"\nExpected checksum: \"{expected_zip_sha256}\"\nAsset URL: \"{}\"",
            metadata.package_zip_url
        );
        return Err(ModuleHostError::SecurityManagerException {
            message: Some(msg.into()),
        });
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Backwards-compatible M2 thread-local shim (upgraded with M4 bug fixes)
// ---------------------------------------------------------------------------

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

    /// Checked disk-cache ZIP read: returns `Err` on corrupt/truncated ZIP instead of swallowing.
    fn read_disk_archive_checked(
        &self,
        package: &PackageUri,
    ) -> Result<Option<ZipArchive>, ModuleHostError> {
        Ok(self.read_disk_archive(package))
    }

    /// Read cached metadata `.json` bytes from disk cache if present.
    fn read_disk_metadata(
        &self,
        _package: &PackageUri,
    ) -> Result<Option<Vec<u8>>, ModuleHostError> {
        Ok(None)
    }

    /// Delete corrupt cached metadata `.json` from disk cache.
    fn delete_disk_metadata(&self, _package: &PackageUri) -> Result<(), ModuleHostError> {
        Ok(())
    }

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

    /// Whether test mode is active (allowing `$skipChecksumVerification` on metadata).
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

/// Ensure that package metadata is loaded (without downloading the ZIP archive).
pub fn ensure_package_metadata_loaded(
    package: &PackageUri,
    expected_checksum: Option<&PklString>,
) -> Result<(DependencyMetadata, Option<Vec<u8>>), ModuleHostError> {
    if let Some(metadata) = SESSION_CACHE.with(|c| c.borrow().get_metadata(package)) {
        let bytes = SESSION_CACHE.with(|c| c.borrow().get_metadata_bytes(package));
        return Ok((metadata, bytes));
    }

    let host_opt = CURRENT_HOST.with(|h| h.borrow().clone());
    let metadata_request_uri = crate::packages::http::get_metadata_request_uri(package);

    // 1. Check disk cache for metadata JSON first
    if let Some(host) = &host_opt
        && let Some(cached_bytes) = host.read_disk_metadata(package)?
    {
        match parse_metadata_bytes(package, &metadata_request_uri, &cached_bytes) {
            Ok(meta) => {
                SESSION_CACHE.with(|c| {
                    let mut cache = c.borrow_mut();
                    cache.insert_metadata(package, meta.clone());
                    cache.insert_metadata_bytes(package, cached_bytes.clone());
                });
                return Ok((meta, Some(cached_bytes)));
            }
            Err(failure) => {
                let _ = host.delete_disk_metadata(package);
                return Err(ModuleHostError::SecurityManagerException {
                    message: Some(failure.message()),
                });
            }
        }
    }

    // 2. Fetch over HTTPS
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

    let metadata = accept_metadata(
        package,
        &metadata_request_uri,
        &metadata_bytes,
        expected_checksum.or(package.sha256.as_ref()),
        is_test_mode,
    )
    .map_err(|failure| ModuleHostError::SecurityManagerException {
        message: Some(failure.message()),
    })?;

    SESSION_CACHE.with(|c| {
        let mut cache = c.borrow_mut();
        cache.insert_metadata(package, metadata.clone());
        cache.insert_metadata_bytes(package, metadata_bytes.clone());
    });

    Ok((metadata, Some(metadata_bytes)))
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

    // 2. Ensure metadata is loaded (checks disk cache before network!)
    let (metadata, metadata_bytes_opt) =
        ensure_package_metadata_loaded(package, expected_checksum)?;

    // 3. Check disk cache for ZIP archive (propagating corrupt ZIP errors!)
    let host_opt = CURRENT_HOST.with(|h| h.borrow().clone());
    if let Some(host) = &host_opt
        && let Some(archive) = host.read_disk_archive_checked(package)?
    {
        SESSION_CACHE.with(|c| c.borrow_mut().insert_archive(package, archive.clone()));
        return Ok((metadata, archive));
    }

    // 4. Download package ZIP (only reached if metadata is valid!)
    crate::packages::http::record_zip_request();

    let zip_bytes = if let Some(host) = &host_opt {
        host.fetch_zip(package, &metadata.package_zip_url)?
    } else {
        return Err(ModuleHostError::Unsupported(
            "Package ZIP download requires a registered PackageHost".into(),
        ));
    };

    // 5. Verify ZIP SHA-256 checksum strictly (`PackageResolvers.java:181-194`)
    verify_zip_checksum(package, &metadata, &zip_bytes)?;

    // 6. Strictly parse ZIP archive (CRC-32, length, EOCD/CD bounds)
    let archive = ZipArchive::parse(&zip_bytes)
        .map_err(|err| ModuleHostError::Io(format!("Failed to parse ZIP archive: {err}").into()))?;

    // 7. Store in caches
    if let Some(host) = &host_opt
        && let Some(ref metadata_bytes) = metadata_bytes_opt
    {
        let _ = host.write_disk_archive(package, metadata_bytes, &zip_bytes);
    }
    SESSION_CACHE.with(|c| {
        let mut cache = c.borrow_mut();
        cache.insert_metadata(package, metadata.clone());
        cache.insert_archive(package, archive.clone());
    });

    Ok((metadata, archive))
}

pub fn file_uri_from_path(path: &Path) -> Result<String, ModuleHostError> {
    let raw = path
        .to_str()
        .ok_or_else(|| ModuleHostError::Unsupported("Non-UTF-8 path".into()))?;
    let mut normalized = if raw.contains('\\') {
        raw.replace('\\', "/")
    } else {
        raw.to_string()
    };
    if let Some(rest) = normalized.strip_prefix("//?/") {
        let b = rest.as_bytes();
        if b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':' {
            normalized = rest.to_string();
        }
    }
    if !normalized.starts_with('/') {
        normalized.insert(0, '/');
    }
    let mut uri = String::from("file://");
    use std::fmt::Write;
    for byte in normalized.bytes() {
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
    crate::packages::zip::percent_encode_fragment(fragment)
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

/// Finds the resolved project dependency entry for a `projectpackage:` URI using
/// exact `CanonicalPackageUri` matching (preserving major-version separation).
fn lookup_project_dep_by_canonical<'a>(
    project: &'a PklProject,
    package: &PackageUri,
) -> Option<&'a crate::packages::project::ResolvedProjectDep> {
    let canonical: CanonicalPackageUri = package.to_canonical();
    if let Some(key) = canonical.as_str()
        && let Some(dep) = project.dependencies.get(key)
    {
        return Some(dep);
    }
    for dep in project.dependencies.values() {
        if let Ok(dep_pkg) = PackageUri::parse(dep.uri.clone())
            && dep_pkg.to_canonical() == canonical
        {
            return Some(dep);
        }
    }
    None
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
    let dep = lookup_project_dep_by_canonical(&project, &asset_uri.package)?;
    if dep.dep_type == "local"
        && let Some(rel_path) = &dep.path
    {
        let local_root = project.project_dir.join(rel_path);
        let raw_subpath = asset_uri.asset_path.as_str().unwrap_or("");
        let local_subpath = normalize_local_subpath(raw_subpath);
        return Some(local_root.join(local_subpath));
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

/// Resolves a `projectpackage:` module URI (local or remote with mandatory lockfile lookup & checksum).
pub fn resolve_projectpackage_module(uri: &PklString) -> Result<ResolvedModule, ModuleHostError> {
    let raw_uri = uri.as_str().unwrap_or("");
    let asset_uri = PackageAssetUri::parse(uri.clone()).map_err(map_package_uri_error)?;

    let host_opt = CURRENT_HOST.with(|h| h.borrow().clone());
    if let Some(host) = &host_opt
        && let Some(project_dir) = host.find_project_dir(&PathBuf::from("."))
        && let Ok(mut project) = host.load_project(&project_dir)
    {
        if !project.deps_loaded {
            host.load_project_dependencies(&mut project)?;
        }
        let Some(dep) = lookup_project_dep_by_canonical(&project, &asset_uri.package) else {
            return Err(ModuleHostError::SecurityManagerException {
                message: Some(
                    format!(
                        "Did not find a resolved dependency for package `{}`.\n\nRun `pkl project resolve` to update resolved dependencies.",
                        asset_uri.package.uri
                    )
                    .into(),
                ),
            });
        };
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
        } else {
            let (_, archive) = ensure_package_loaded(&asset_uri.package, dep.sha256.as_ref())?;
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

    Err(ModuleHostError::SecurityManagerException {
        message: Some(
            format!(
                "Did not find a resolved dependency for package `{}`.\n\nRun `pkl project resolve` to update resolved dependencies.",
                asset_uri.package.uri
            )
            .into(),
        ),
    })
}

/// Helper for `projectpackage:` resource/glob/has_element: resolves local path OR mandatory remote checksum.
enum ProjectPackageTarget {
    LocalPath(PathBuf),
    Remote { expected_sha256: Option<PklString> },
}

fn resolve_projectpackage_target(
    asset_uri: &PackageAssetUri,
) -> Result<ProjectPackageTarget, ModuleHostError> {
    let host_opt = CURRENT_HOST.with(|h| h.borrow().clone());
    if let Some(host) = &host_opt
        && let Some(project_dir) = host.find_project_dir(&PathBuf::from("."))
        && let Ok(mut project) = host.load_project(&project_dir)
    {
        if !project.deps_loaded {
            host.load_project_dependencies(&mut project)?;
        }
        if let Some(dep) = lookup_project_dep_by_canonical(&project, &asset_uri.package) {
            if dep.dep_type == "local"
                && let Some(rel_path) = &dep.path
            {
                let local_root = project.project_dir.join(rel_path);
                let raw_subpath = asset_uri.asset_path.as_str().unwrap_or("");
                let local_subpath = normalize_local_subpath(raw_subpath);
                return Ok(ProjectPackageTarget::LocalPath(
                    local_root.join(local_subpath),
                ));
            }
            return Ok(ProjectPackageTarget::Remote {
                expected_sha256: dep.sha256.clone(),
            });
        }
    }
    Err(ModuleHostError::SecurityManagerException {
        message: Some(
            format!(
                "Did not find a resolved dependency for package `{}`.\n\nRun `pkl project resolve` to update resolved dependencies.",
                asset_uri.package.uri
            )
            .into(),
        ),
    })
}

/// Resolves raw resource bytes for `read("package:...")` or `read("projectpackage:...")`
/// (enforcing lockfile SHA-256 on remote `projectpackage:` resources).
pub fn resolve_package_resource(uri: &PklString) -> Result<Vec<u8>, ModuleHostError> {
    let raw_uri = uri.as_str().unwrap_or("");
    let asset_uri = PackageAssetUri::parse(uri.clone()).map_err(map_package_uri_error)?;

    let expected_checksum = if raw_uri.starts_with("projectpackage:") {
        match resolve_projectpackage_target(&asset_uri)? {
            ProjectPackageTarget::LocalPath(file_path) => {
                let host = CURRENT_HOST
                    .with(|h| h.borrow().clone())
                    .ok_or(ModuleHostError::NotFound)?;
                return host.read_file(&file_path);
            }
            ProjectPackageTarget::Remote { expected_sha256 } => expected_sha256,
        }
    } else {
        None
    };

    let (_, archive) = ensure_package_loaded(&asset_uri.package, expected_checksum.as_ref())?;
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
            let mut remote_expected_sha: Option<PklString> = None;

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
                if let Some(dep) = lookup_project_dep_by_canonical(&project, &asset_uri.package) {
                    if dep.dep_type == "local"
                        && let Some(rel_path) = &dep.path
                    {
                        let local_root = project.project_dir.join(rel_path);
                        if let Ok(mut local_proj) = host.load_project(&local_root) {
                            if !local_proj.deps_loaded {
                                let _ = host.load_project_dependencies(&mut local_proj);
                            }
                            local_match = Some((project, local_proj));
                        }
                    } else {
                        remote_expected_sha = dep.sha256.clone();
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
                    let clean_pkg_uri = asset_uri.package.display_name();
                    let msg = format!(
                        "Cannot find dependency named `{dep_name}`, because it was not declared in package `{clean_pkg_uri}`."
                    );
                    return Err(ModuleHostError::SecurityManagerException {
                        message: Some(msg.into()),
                    });
                }
            }

            // Metadata-only fetch (never requires downloading the ZIP just to rewrite `@dep`)
            let (meta, _) =
                ensure_package_metadata_loaded(&asset_uri.package, remote_expected_sha.as_ref())?;
            for dep in &meta.dependencies {
                if dep.name == dep_name {
                    let target_base = dep.package_uri.uri.as_str().unwrap_or("");
                    let mut resolved_base = target_base.to_string();
                    if base_str.starts_with("projectpackage:")
                        && let Some(host) = &host_opt
                        && let Some(project_dir) = host.find_project_dir(&PathBuf::from("."))
                        && let Ok(mut project) = host.load_project(&project_dir)
                    {
                        if !project.deps_loaded {
                            let _ = host.load_project_dependencies(&mut project);
                        }
                        let Some(matching) = crate::packages::project::find_matching_resolved(
                            target_base,
                            dep_name,
                            &project.dependencies,
                        ) else {
                            return Err(ModuleHostError::SecurityManagerException {
                                message: Some(
                                    format!(
                                        "Did not find a resolved dependency for package `{target_base}`.\n\nRun `pkl project resolve` to update resolved dependencies."
                                    )
                                    .into(),
                                ),
                            });
                        };
                        let resolved_uri_str = matching.uri.as_str().unwrap_or("");
                        let dec_ver =
                            crate::packages::project::extract_version_from_uri(target_base);
                        let res_ver =
                            crate::packages::project::extract_version_from_uri(resolved_uri_str);
                        if let (Some(dv), Some(rv)) = (dec_ver, res_ver)
                            && crate::packages::project::compare_dep_versions(dv, rv)
                                == std::cmp::Ordering::Greater
                        {
                            let clean_res_uri =
                                resolved_uri_str.replace("projectpackage:", "package:");
                            let clean_pkg_uri = asset_uri.package.display_name();
                            let msg = format!(
                                "Transitive dependency declared by package `{clean_pkg_uri}` is newer than what is declared in `PklProject.deps.json`.\n\nDeclared: `{target_base}`\nResolved: `{clean_res_uri}`\n\nRun `pkl project resolve` to update resolved dependencies."
                            );
                            return Err(ModuleHostError::SecurityManagerException {
                                message: Some(msg.into()),
                            });
                        }
                        resolved_base = resolved_uri_str.to_string();
                    }
                    let encoded_asset = percent_encode_fragment(asset_path);
                    return Ok(format!("{resolved_base}#{encoded_asset}").into());
                }
            }
            let clean_pkg_uri = asset_uri.package.display_name();
            let msg = format!(
                "Cannot find dependency named `{dep_name}`, because it was not declared in package `{clean_pkg_uri}`."
            );
            return Err(ModuleHostError::SecurityManagerException {
                message: Some(msg.into()),
            });
        }

        // Guard opaque/non-hierarchical URIs (`repl:text`, etc.) per `IoUtils.java:406-414`
        if !base_str.contains("://") && !base_str.starts_with("file:/") {
            let msg = format!(
                "Cannot resolve dependency because project URI `{base_str}` does not have a hierarchical path."
            );
            return Err(ModuleHostError::SecurityManagerException {
                message: Some(msg.into()),
            });
        }

        // Otherwise resolve from project
        let host_opt = CURRENT_HOST.with(|h| h.borrow().clone());
        let base_path = if base_str.starts_with("file://") {
            PathBuf::from(crate::packages::zip::percent_decode(
                base_str.strip_prefix("file://").unwrap_or(base_str),
            ))
        } else {
            PathBuf::from(".")
        };

        if let Some(host) = &host_opt
            && let Some(project_dir) = host.find_project_dir(&base_path)
        {
            let mut project = host.load_project(&project_dir)?;
            if !project.deps_loaded {
                host.load_project_dependencies(&mut project)?;
            }
            return resolve_project_dependency(&project, authored_str);
        }
        let msg = "Cannot resolve dependency because there is no project found.\n\nIf you meant to import a path that starts with `@`, prefix the path with `./` (e.g. `import \"./@myPath\").\nIf you meant to import a dependency, ensure that this file is within a directory that contains a PklProject module.";
        return Err(ModuleHostError::SecurityManagerException {
            message: Some(msg.into()),
        });
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

/// List elements in package directory for globbing (enforces lockfile SHA-256 for `projectpackage:`).
pub fn list_elements(uri: &PklString) -> Result<Vec<crate::glob::PathElement>, ModuleHostError> {
    let raw_uri = uri.as_str().unwrap_or("");
    let asset_uri = PackageAssetUri::parse(uri.clone()).map_err(map_package_uri_error)?;

    let expected_checksum = if raw_uri.starts_with("projectpackage:") {
        match resolve_projectpackage_target(&asset_uri)? {
            ProjectPackageTarget::LocalPath(dir_path) => {
                let host = CURRENT_HOST
                    .with(|h| h.borrow().clone())
                    .ok_or(ModuleHostError::NotFound)?;
                return host.list_dir(&dir_path);
            }
            ProjectPackageTarget::Remote { expected_sha256 } => expected_sha256,
        }
    } else {
        None
    };

    let (_, archive) = ensure_package_loaded(&asset_uri.package, expected_checksum.as_ref())?;
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

/// Checks if an element exists in a package for globbing (enforces lockfile SHA-256 for `projectpackage:`).
pub fn has_element(uri: &PklString) -> Result<bool, ModuleHostError> {
    let raw_uri = uri.as_str().unwrap_or("");
    let asset_uri = PackageAssetUri::parse(uri.clone()).map_err(map_package_uri_error)?;

    let expected_checksum = if raw_uri.starts_with("projectpackage:") {
        match resolve_projectpackage_target(&asset_uri)? {
            ProjectPackageTarget::LocalPath(file_path) => {
                let host = CURRENT_HOST
                    .with(|h| h.borrow().clone())
                    .ok_or(ModuleHostError::NotFound)?;
                return Ok(host.read_file(&file_path).is_ok() || host.list_dir(&file_path).is_ok());
            }
            ProjectPackageTarget::Remote { expected_sha256 } => expected_sha256,
        }
    } else {
        None
    };

    let (_, archive) = ensure_package_loaded(&asset_uri.package, expected_checksum.as_ref())?;
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
