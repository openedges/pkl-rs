//! Pure in-memory package cache and disk-cache path computation.
//!
//! Disk I/O is delegated to the Host adapter outside `pkl-runtime`.
use crate::modules::ModuleHostError;
use crate::packages::PackageUri;
use crate::packages::metadata::DependencyMetadata;
use crate::packages::zip::ZipArchive;
use std::collections::BTreeMap;
use std::path::PathBuf;

/// Pure in-memory package cache storing metadata and decompressed archives.
#[derive(Clone, Default)]
pub struct PackageCache {
    metadata: BTreeMap<String, DependencyMetadata>,
    archives: BTreeMap<String, ZipArchive>,
}

impl PackageCache {
    pub const fn new() -> Self {
        Self {
            metadata: BTreeMap::new(),
            archives: BTreeMap::new(),
        }
    }

    pub fn get_metadata(&self, package: &PackageUri) -> Option<DependencyMetadata> {
        let key = package.uri.as_str()?;
        self.metadata.get(key).cloned()
    }

    pub fn insert_metadata(&mut self, package: &PackageUri, metadata: DependencyMetadata) {
        if let Some(key) = package.uri.as_str() {
            self.metadata.insert(key.to_string(), metadata);
        }
    }

    pub fn get_archive(&self, package: &PackageUri) -> Option<ZipArchive> {
        let key = package.uri.as_str()?;
        self.archives.get(key).cloned()
    }

    pub fn insert_archive(&mut self, package: &PackageUri, archive: ZipArchive) {
        if let Some(key) = package.uri.as_str() {
            self.archives.insert(key.to_string(), archive);
        }
    }
}

/// Computes the relative cache path for a package within a cache root (`package-2/<authority>/<path>`).
pub fn get_cache_rel_path(package: &PackageUri) -> Option<PathBuf> {
    let uri_str = package.uri.as_str()?;
    let without_scheme = uri_str
        .strip_prefix("package://")
        .or_else(|| uri_str.strip_prefix("projectpackage://"))?;
    let (authority, path) = without_scheme.split_once('/')?;
    let effective_path = path.split("::").next().unwrap_or(path);
    let mut rel = PathBuf::from("package-2");
    rel.push(authority);
    rel.push(effective_path);
    Some(rel)
}

/// Extracts the leaf name of a package for cache filenames.
pub fn package_leaf_name(package: &PackageUri) -> Option<String> {
    let raw = package.uri.as_str()?;
    let path = raw.split("::").next().unwrap_or(raw);
    let leaf = path.rsplit('/').next()?;
    Some(leaf.to_string())
}

/// Host-provided disk cache persistence trait.
pub trait PackageCacheStore: Send + Sync {
    fn try_read_disk_archive(&self, package: &PackageUri) -> Option<ZipArchive>;
    fn try_write_disk_archive(
        &self,
        package: &PackageUri,
        metadata_bytes: &[u8],
        zip_bytes: &[u8],
    ) -> Result<(), ModuleHostError>;
}
