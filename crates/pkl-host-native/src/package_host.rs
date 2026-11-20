//! Native package transport, disk cache (`package-2`), and packager host adapters
//! backed directly by [`NativeHttpClient`] (zero `/usr/bin/curl`, zero `unsafe`).

use crate::http::NativeHttpClient;
use pkl_runtime::host::IoFailure;
use pkl_runtime::host::http::HttpError;
use pkl_runtime::host::security::SecurityPolicy;
use pkl_runtime::modules::ModuleHostError;
use pkl_runtime::packages::PackageUri;
use pkl_runtime::packages::cache::{PackageCachePaths, PackageCacheStore};
use pkl_runtime::packages::http::{PackageHttpError, PackageTransport};
use pkl_runtime::packages::packager::PackagerHost;
use pkl_runtime::packages::project::PKL_PROJECT_DEPS_FILENAME;
use pkl_runtime::packages::zip::{ZipArchive, ZipError};
use pkl_runtime::value::PklString;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_TMP_ID: AtomicU64 = AtomicU64::new(1);

/// Returns Pkl's default module cache directory (`$HOME/.pkl/cache` or `$USERPROFILE/.pkl/cache`).
pub fn default_module_cache_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(|home| PathBuf::from(home).join(".pkl").join("cache"))
}

/// Native [`PackageTransport`] backed directly by [`NativeHttpClient`] and [`SecurityPolicy`].
///
/// Mirrors `PackageResolvers.AbstractPackageResolver.openExternalUri` (`PackageResolvers.java:195-217`):
/// - Checks resource allowlist (`checkReadResource`) on initial URI and every redirect hop
/// - Uses `NativeHttpClient` (honoring `--ca-certificates`, `~/.pkl/cacerts`, bundled `PklCARoots.pem`,
///   `--http-proxy`, `--http-no-proxy`, `--http-rewrite`, `--http-header`, `--test-port`, `CancelFlag`)
/// - Maps non-200 responses to `badHttpStatusCode` (`PackageHttpError::BadStatusCode`)
/// - Preserves `HttpClientException` vs `SecurityManagerException` vs `ioErrorMakingHttpGet`
pub struct NativePackageTransport {
    http: Arc<NativeHttpClient>,
    policy: Arc<SecurityPolicy>,
}

impl NativePackageTransport {
    pub fn new(http: Arc<NativeHttpClient>, policy: Arc<SecurityPolicy>) -> Self {
        Self { http, policy }
    }

    fn check_resource_uri(&self, uri: &str) -> Result<(), PklString> {
        let pkl_uri = PklString::from(uri);
        self.policy.check_allowlist_only(&pkl_uri, true)
    }

    fn fetch_bytes(&self, url: &str) -> Result<Vec<u8>, PackageHttpError> {
        if !url.starts_with("http://") && !url.starts_with("https://") {
            return Err(PackageHttpError::Transport(
                format!("Expected HTTP(S) URL, but got: {url}").into(),
            ));
        }

        // Pre-rewrite resource allowlist check (`PackageResolvers.java:201`).
        if let Err(msg) = self.check_resource_uri(url) {
            return Err(PackageHttpError::Security(msg));
        }

        let (response, final_uri) = self
            .http
            .get_with_uri(url, |hop| self.check_resource_uri(hop))
            .map_err(|err| match err {
                HttpError::SecurityManagerException(msg) => PackageHttpError::Security(msg),
                HttpError::HttpClientException(msg) => PackageHttpError::Transport(msg),
                HttpError::Io(msg) | HttpError::Closed(msg) => PackageHttpError::Transport(
                    format!(
                        "Exception when making request `GET {url}`:\n{}",
                        msg.to_java_utf8()
                    )
                    .into(),
                ),
            })?;

        if response.status != 200 {
            return Err(PackageHttpError::BadStatusCode {
                status_code: response.status,
                url: PklString::from(final_uri),
            });
        }

        Ok(response.body)
    }
}

impl PackageTransport for NativePackageTransport {
    fn fetch_metadata(
        &self,
        _package: &PackageUri,
        request_uri: &PklString,
    ) -> Result<Vec<u8>, ModuleHostError> {
        let raw = request_uri.as_str().unwrap_or("");
        self.fetch_bytes(raw)
            .map_err(PackageHttpError::into_host_error)
    }

    fn fetch_zip(
        &self,
        _package: &PackageUri,
        zip_url: &PklString,
    ) -> Result<Vec<u8>, ModuleHostError> {
        let raw = zip_url.as_str().unwrap_or("");
        self.fetch_bytes(raw)
            .map_err(PackageHttpError::into_host_error)
    }

    fn fetch_metadata_with_status(
        &self,
        _package: &PackageUri,
        request_uri: &PklString,
    ) -> Result<Vec<u8>, PackageHttpError> {
        let raw = request_uri.as_str().unwrap_or("");
        self.fetch_bytes(raw)
    }
}

/// Persistent disk package cache implementing [`PackageCacheStore`] using Pkl 0.32.0's
/// `package-2` encoded directory layout, atomic `<cache_dir>/tmp` writes with `0444`
/// read-only permissions, corrupt `.json` eviction, and strict checked ZIP parsing.
#[derive(Clone, Debug)]
pub struct DiskPackageCache {
    cache_dir: PathBuf,
}

impl DiskPackageCache {
    pub fn new(cache_dir: PathBuf) -> Self {
        Self { cache_dir }
    }

    pub fn cache_dir(&self) -> &Path {
        &self.cache_dir
    }

    fn resolve_paths(&self, package: &PackageUri) -> Option<(PathBuf, PathBuf, PathBuf)> {
        let rel = PackageCachePaths::for_package(package)?;
        Some((
            self.cache_dir.join(rel.dir),
            self.cache_dir.join(rel.metadata_file),
            self.cache_dir.join(rel.zip_file),
        ))
    }

    fn write_atomic_readonly(
        &self,
        target_path: &Path,
        bytes: &[u8],
    ) -> Result<(), ModuleHostError> {
        if target_path.exists() {
            return Ok(());
        }
        let Some(parent) = target_path.parent() else {
            return Err(ModuleHostError::Io(
                format!("Invalid cache file path `{}`.", target_path.display()).into(),
            ));
        };
        fs::create_dir_all(parent).map_err(|err| {
            ModuleHostError::Io(
                format!(
                    "Failed to create package cache directory `{}`: {err}",
                    parent.display()
                )
                .into(),
            )
        })?;

        let tmp_dir = self.cache_dir.join("tmp");
        fs::create_dir_all(&tmp_dir).map_err(|err| {
            ModuleHostError::Io(
                format!(
                    "Failed to create package cache tmp directory `{}`: {err}",
                    tmp_dir.display()
                )
                .into(),
            )
        })?;

        let seq = NEXT_TMP_ID.fetch_add(1, Ordering::Relaxed);
        let pid = std::process::id();
        let leaf = target_path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("pkg");
        let tmp_path = tmp_dir.join(format!("{leaf}.{pid}.{seq}.tmp"));

        {
            let mut file = fs::File::create(&tmp_path).map_err(|err| {
                ModuleHostError::Io(
                    format!(
                        "Failed to create cache temp file `{}`: {err}",
                        tmp_path.display()
                    )
                    .into(),
                )
            })?;
            file.write_all(bytes).map_err(|err| {
                let _ = fs::remove_file(&tmp_path);
                ModuleHostError::Io(
                    format!(
                        "Failed to write cache temp file `{}`: {err}",
                        tmp_path.display()
                    )
                    .into(),
                )
            })?;
            file.flush().map_err(|err| {
                let _ = fs::remove_file(&tmp_path);
                ModuleHostError::Io(
                    format!(
                        "Failed to flush cache temp file `{}`: {err}",
                        tmp_path.display()
                    )
                    .into(),
                )
            })?;
        }

        // Set read-only permissions (0444) matching `DiskCachedPackageResolver.java:636-641`.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&tmp_path, fs::Permissions::from_mode(0o444));
        }
        #[cfg(not(unix))]
        {
            if let Ok(mut perms) = fs::metadata(&tmp_path).map(|m| m.permissions()) {
                perms.set_readonly(true);
                let _ = fs::set_permissions(&tmp_path, perms);
            }
        }

        match fs::rename(&tmp_path, target_path) {
            Ok(()) => Ok(()),
            Err(err) => {
                let _ = remove_possibly_readonly(&tmp_path);
                // Another concurrent writer may have raced and won; accept if target now exists.
                if target_path.is_file() {
                    Ok(())
                } else {
                    Err(ModuleHostError::Io(
                        format!(
                            "Failed to move cache file `{}` to `{}`: {err}",
                            tmp_path.display(),
                            target_path.display()
                        )
                        .into(),
                    ))
                }
            }
        }
    }
}

fn remove_possibly_readonly(path: &Path) -> std::io::Result<()> {
    if let Err(err) = fs::remove_file(path) {
        if err.kind() == std::io::ErrorKind::NotFound {
            return Ok(());
        }
        // Make writable and retry (cached files are 0444).
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o644));
        }
        #[cfg(not(unix))]
        {
            if let Ok(mut perms) = fs::metadata(path).map(|m| m.permissions()) {
                perms.set_readonly(false);
                let _ = fs::set_permissions(path, perms);
            }
        }
        match fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e),
        }
    } else {
        Ok(())
    }
}

impl PackageCacheStore for DiskPackageCache {
    fn read_metadata_bytes(
        &self,
        package: &PackageUri,
    ) -> Result<Option<Vec<u8>>, ModuleHostError> {
        let Some((_, meta_path, _)) = self.resolve_paths(package) else {
            return Ok(None);
        };
        match fs::read(&meta_path) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(err) => Err(ModuleHostError::Io(
                format!(
                    "Failed to read cached package metadata `{}`: {err}",
                    meta_path.display()
                )
                .into(),
            )),
        }
    }

    fn write_metadata_bytes(
        &self,
        package: &PackageUri,
        metadata_bytes: &[u8],
    ) -> Result<(), ModuleHostError> {
        let Some((_, meta_path, _)) = self.resolve_paths(package) else {
            return Ok(());
        };
        self.write_atomic_readonly(&meta_path, metadata_bytes)
    }

    fn delete_metadata(&self, package: &PackageUri) -> Result<(), ModuleHostError> {
        let Some((_, meta_path, _)) = self.resolve_paths(package) else {
            return Ok(());
        };
        remove_possibly_readonly(&meta_path).map_err(|err| {
            ModuleHostError::Io(
                format!(
                    "Failed to delete corrupt cached metadata `{}`: {err}",
                    meta_path.display()
                )
                .into(),
            )
        })
    }

    fn read_zip_archive(
        &self,
        package: &PackageUri,
    ) -> Result<Option<ZipArchive>, ModuleHostError> {
        let Some((_, _, zip_path)) = self.resolve_paths(package) else {
            return Ok(None);
        };
        let bytes = match fs::read(&zip_path) {
            Ok(b) => b,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(err) => {
                return Err(ModuleHostError::Io(
                    format!(
                        "Failed to read cached ZIP archive `{}`: {err}",
                        zip_path.display()
                    )
                    .into(),
                ));
            }
        };
        let archive = ZipArchive::parse(&bytes).map_err(|err| {
            let reason = match err {
                ZipError::UnexpectedEof | ZipError::InvalidSignature => {
                    "zip END header not found".to_string()
                }
                other => other.to_string(),
            };
            ModuleHostError::Io(IoFailure::with_class("ZipException", reason))
        })?;
        Ok(Some(archive))
    }

    fn write_zip_bytes(
        &self,
        package: &PackageUri,
        zip_bytes: &[u8],
    ) -> Result<(), ModuleHostError> {
        let Some((_, _, zip_path)) = self.resolve_paths(package) else {
            return Ok(());
        };
        self.write_atomic_readonly(&zip_path, zip_bytes)
    }

    fn try_read_disk_archive(&self, package: &PackageUri) -> Option<ZipArchive> {
        self.read_zip_archive(package).ok().flatten()
    }
}

/// Native [`PackagerHost`] for `pkl project package`, using [`NativePackageTransport`]
/// for optional publish checks (zero `/usr/bin/curl`).
pub struct NativePackagerHost {
    transport: NativePackageTransport,
}

impl NativePackagerHost {
    pub fn new(http: Arc<NativeHttpClient>, policy: Arc<SecurityPolicy>) -> Self {
        Self {
            transport: NativePackageTransport::new(http, policy),
        }
    }
}

impl PackagerHost for NativePackagerHost {
    fn list_project_files(
        &self,
        project_dir: &Path,
    ) -> Result<Vec<(String, Vec<u8>)>, ModuleHostError> {
        let mut files = Vec::new();
        walk_regular_files(project_dir, project_dir, &mut files)?;
        files.sort_by(|(a, _), (b, _)| a.cmp(b));
        Ok(files)
    }

    fn read_project_deps_json(
        &self,
        project_dir: &Path,
    ) -> Result<Option<String>, ModuleHostError> {
        let path = project_dir.join(PKL_PROJECT_DEPS_FILENAME);
        if !path.is_file() {
            return Ok(None);
        }
        let text = fs::read_to_string(&path).map_err(|err| {
            ModuleHostError::Io(
                format!("Failed to read deps file `{}`: {err}", path.display()).into(),
            )
        })?;
        Ok(Some(text))
    }

    fn write_artifact(&self, path: &Path, bytes: &[u8]) -> Result<(), ModuleHostError> {
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            fs::create_dir_all(parent).map_err(|err| {
                ModuleHostError::Io(
                    format!(
                        "Failed to create output directory `{}`: {err}",
                        parent.display()
                    )
                    .into(),
                )
            })?;
        }
        fs::write(path, bytes).map_err(|err| {
            ModuleHostError::Io(
                format!("Failed to write artifact `{}`: {err}", path.display()).into(),
            )
        })
    }

    fn transport(&self) -> Option<&dyn PackageTransport> {
        Some(&self.transport)
    }
}

fn walk_regular_files(
    root: &Path,
    dir: &Path,
    out: &mut Vec<(String, Vec<u8>)>,
) -> Result<(), ModuleHostError> {
    let entries = fs::read_dir(dir).map_err(|err| {
        ModuleHostError::Io(format!("Failed to read directory `{}`: {err}", dir.display()).into())
    })?;
    for entry in entries {
        let entry = entry.map_err(|err| {
            ModuleHostError::Io(format!("Failed to read directory entry: {err}").into())
        })?;
        let path = entry.path();
        let meta = fs::symlink_metadata(&path).map_err(|err| {
            ModuleHostError::Io(format!("Failed to stat `{}`: {err}", path.display()).into())
        })?;
        if meta.is_dir() {
            walk_regular_files(root, &path, out)?;
        } else if meta.is_file() {
            let rel = path.strip_prefix(root).map_err(|err| {
                ModuleHostError::Io(format!("Failed to strip prefix: {err}").into())
            })?;
            let rel_str = rel.to_string_lossy().replace('\\', "/");
            let bytes = fs::read(&path).map_err(|err| {
                ModuleHostError::Io(format!("Failed to read `{}`: {err}", path.display()).into())
            })?;
            out.push((rel_str, bytes));
        }
    }
    Ok(())
}
