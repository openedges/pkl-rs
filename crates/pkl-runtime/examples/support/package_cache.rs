//! Native Host disk cache under `<cache_dir>/package-2/` or `~/.pkl/cache/package-2/`.
//!
//! Matches `PackageResolvers.DiskCachedPackageResolver` (`PackageResolvers.java:426-584`):
//! - `IoUtils.encodePath` encoding on all OSes (e.g. `localhost:0` -> `localhost(3a)0`)
//! - Atomic write via `<cache_dir>/tmp/` + `rename` + `0444` POSIX permissions
//! - No non-upstream `.json.sha256` sidecar files
//! - Corrupt cached `.json` deletion and strict corrupt `.zip` error propagation
use pkl_runtime::modules::ModuleHostError;
use pkl_runtime::packages::PackageUri;
use pkl_runtime::packages::cache::{PackageCachePaths, PackageCacheStore, encode_cache_path};
use pkl_runtime::packages::zip::ZipArchive;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static TMP_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug)]
pub struct HostPackageCache {
    pub cache_dir: Option<PathBuf>,
    pub no_cache: bool,
}

impl HostPackageCache {
    pub fn new(cache_dir: Option<PathBuf>, no_cache: bool) -> Self {
        let effective_cache_dir = if no_cache {
            None
        } else if let Some(dir) = cache_dir {
            Some(dir)
        } else {
            default_module_cache_dir()
        };

        Self {
            cache_dir: effective_cache_dir,
            no_cache,
        }
    }

    pub fn metadata_path(&self, package: &PackageUri) -> Option<PathBuf> {
        if self.no_cache {
            return None;
        }
        let cache_dir = self.cache_dir.as_ref()?;
        let paths = PackageCachePaths::for_package(package)?;
        Some(cache_dir.join(paths.metadata_file))
    }

    pub fn zip_path(&self, package: &PackageUri) -> Option<PathBuf> {
        if self.no_cache {
            return None;
        }
        let cache_dir = self.cache_dir.as_ref()?;
        let paths = PackageCachePaths::for_package(package)?;
        Some(cache_dir.join(paths.zip_file))
    }

    fn write_atomic_readonly(
        &self,
        package: &PackageUri,
        target_path: &Path,
        suffix: &str,
        bytes: &[u8],
    ) -> Result<(), ModuleHostError> {
        let Some(cache_dir) = self.cache_dir.as_ref() else {
            return Ok(());
        };
        let tmp_dir = cache_dir.join("tmp");
        std::fs::create_dir_all(&tmp_dir).map_err(|e| {
            ModuleHostError::Io(format!("Failed to create cache tmp dir: {e}").into())
        })?;
        if let Some(parent) = target_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                ModuleHostError::Io(format!("Failed to create package cache dir: {e}").into())
            })?;
        }
        let prefix = encode_cache_path(
            &package
                .display_name()
                .as_str()
                .unwrap_or("pkg")
                .replace('/', "-"),
        );
        let seq = TMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let pid = std::process::id();
        let tmp_path = tmp_dir.join(format!("{prefix}-{pid}-{seq}{suffix}"));

        let write_res = std::fs::write(&tmp_path, bytes);
        if let Err(err) = write_res {
            let _ = std::fs::remove_file(&tmp_path);
            return Err(ModuleHostError::Io(
                format!("Failed to write temp cache file: {err}").into(),
            ));
        }

        // If target already exists as read-only (e.g. concurrent write), remove first on platforms
        // that block overwriting read-only files.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&tmp_path, std::fs::Permissions::from_mode(0o444));
        }

        if let Err(err) = std::fs::rename(&tmp_path, target_path) {
            let _ = std::fs::remove_file(&tmp_path);
            return Err(ModuleHostError::Io(
                format!("Failed to move temp cache file into cache: {err}").into(),
            ));
        }
        Ok(())
    }

    pub fn read_metadata_bytes(
        &self,
        package: &PackageUri,
    ) -> Result<Option<Vec<u8>>, ModuleHostError> {
        let Some(path) = self.metadata_path(package) else {
            return Ok(None);
        };
        if !path.is_file() {
            return Ok(None);
        }
        let bytes = std::fs::read(&path).map_err(|e| {
            ModuleHostError::Io(format!("Failed to read cached metadata: {e}").into())
        })?;
        Ok(Some(bytes))
    }

    pub fn delete_metadata(&self, package: &PackageUri) -> Result<(), ModuleHostError> {
        let Some(path) = self.metadata_path(package) else {
            return Ok(());
        };
        if path.exists() {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644));
            }
            let _ = std::fs::remove_file(&path);
        }
        Ok(())
    }

    pub fn read_zip_archive(
        &self,
        package: &PackageUri,
    ) -> Result<Option<ZipArchive>, ModuleHostError> {
        let Some(zip_file) = self.zip_path(package) else {
            return Ok(None);
        };
        if !zip_file.is_file() {
            return Ok(None);
        }
        let bytes = std::fs::read(&zip_file)
            .map_err(|e| ModuleHostError::Io(format!("Failed to read cached ZIP: {e}").into()))?;
        let archive = ZipArchive::parse(&bytes).map_err(|err| {
            ModuleHostError::Io(format!("Failed to read cached ZIP archive: {err}").into())
        })?;
        Ok(Some(archive))
    }

    pub fn try_read_disk_archive(&self, package: &PackageUri) -> Option<ZipArchive> {
        self.read_zip_archive(package).ok().flatten()
    }

    pub fn try_write_disk_archive(
        &self,
        package: &PackageUri,
        metadata_bytes: &[u8],
        zip_bytes: &[u8],
    ) -> Result<(), ModuleHostError> {
        if self.no_cache {
            return Ok(());
        }
        if let Some(json_file) = self.metadata_path(package) {
            self.write_atomic_readonly(package, &json_file, ".json", metadata_bytes)?;
        }
        if let Some(zip_file) = self.zip_path(package) {
            self.write_atomic_readonly(package, &zip_file, ".zip", zip_bytes)?;
        }
        Ok(())
    }
}

/// Default Pkl module cache directory (`~/.pkl/cache`), matching `IoUtils.getDefaultModuleCacheDir()`.
pub fn default_module_cache_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(|home| PathBuf::from(home).join(".pkl").join("cache"))
}

impl PackageCacheStore for HostPackageCache {
    fn read_metadata_bytes(
        &self,
        package: &PackageUri,
    ) -> Result<Option<Vec<u8>>, ModuleHostError> {
        HostPackageCache::read_metadata_bytes(self, package)
    }

    fn write_metadata_bytes(
        &self,
        package: &PackageUri,
        metadata_bytes: &[u8],
    ) -> Result<(), ModuleHostError> {
        if self.no_cache {
            return Ok(());
        }
        if let Some(json_file) = self.metadata_path(package) {
            self.write_atomic_readonly(package, &json_file, ".json", metadata_bytes)?;
        }
        Ok(())
    }

    fn delete_metadata(&self, package: &PackageUri) -> Result<(), ModuleHostError> {
        HostPackageCache::delete_metadata(self, package)
    }

    fn read_zip_archive(
        &self,
        package: &PackageUri,
    ) -> Result<Option<ZipArchive>, ModuleHostError> {
        HostPackageCache::read_zip_archive(self, package)
    }

    fn write_zip_bytes(
        &self,
        package: &PackageUri,
        zip_bytes: &[u8],
    ) -> Result<(), ModuleHostError> {
        if self.no_cache {
            return Ok(());
        }
        if let Some(zip_file) = self.zip_path(package) {
            self.write_atomic_readonly(package, &zip_file, ".zip", zip_bytes)?;
        }
        Ok(())
    }

    fn try_read_disk_archive(&self, package: &PackageUri) -> Option<ZipArchive> {
        HostPackageCache::try_read_disk_archive(self, package)
    }

    fn try_write_disk_archive(
        &self,
        package: &PackageUri,
        metadata_bytes: &[u8],
        zip_bytes: &[u8],
    ) -> Result<(), ModuleHostError> {
        HostPackageCache::try_write_disk_archive(self, package, metadata_bytes, zip_bytes)
    }
}
