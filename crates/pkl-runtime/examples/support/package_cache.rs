//! Real Host-backed disk caching under `<cache_dir>/package-2/` or `~/.pkl/cache/package-2/`.
use pkl_runtime::modules::ModuleHostError;
use pkl_runtime::packages::PackageUri;
use pkl_runtime::packages::cache::{get_cache_rel_path, package_leaf_name};
use pkl_runtime::packages::zip::ZipArchive;
use sha2::{Digest, Sha256};
use std::path::PathBuf;

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
        } else if let Ok(dir) = std::env::var("PKL_CACHE_DIR") {
            Some(PathBuf::from(dir))
        } else {
            std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".pkl").join("cache"))
        };

        Self {
            cache_dir: effective_cache_dir,
            no_cache,
        }
    }

    pub fn try_read_disk_archive(&self, package: &PackageUri) -> Option<ZipArchive> {
        if self.no_cache {
            return None;
        }
        let cache_dir = self.cache_dir.as_ref()?;
        let rel_path = get_cache_rel_path(package)?;
        let leaf = package_leaf_name(package)?;
        let zip_file = cache_dir.join(rel_path).join(format!("{leaf}.zip"));
        if zip_file.is_file() {
            let bytes = std::fs::read(&zip_file).ok()?;
            ZipArchive::parse(&bytes).ok()
        } else {
            None
        }
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
        let Some(cache_dir) = self.cache_dir.as_ref() else {
            return Ok(());
        };
        let Some(rel_path) = get_cache_rel_path(package) else {
            return Ok(());
        };
        let Some(leaf) = package_leaf_name(package) else {
            return Ok(());
        };

        let target_dir = cache_dir.join(rel_path);
        let _ = std::fs::create_dir_all(&target_dir);

        let json_file = target_dir.join(format!("{leaf}.json"));
        let zip_file = target_dir.join(format!("{leaf}.zip"));
        let sha_file = target_dir.join(format!("{leaf}.json.sha256"));

        let _ = std::fs::write(json_file, metadata_bytes);
        let _ = std::fs::write(zip_file, zip_bytes);

        let meta_hash: String = Sha256::digest(metadata_bytes)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        let _ = std::fs::write(sha_file, meta_hash.as_bytes());

        Ok(())
    }
}
