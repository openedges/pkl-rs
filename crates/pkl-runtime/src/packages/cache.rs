//! Pure in-memory package cache, exact `IoUtils.encodePath` `package-2` disk layout,
//! and host cache persistence traits.
//!
//! Disk I/O is delegated to the Host adapter outside `pkl-runtime`.
use crate::modules::ModuleHostError;
use crate::packages::PackageUri;
use crate::packages::metadata::DependencyMetadata;
use crate::packages::zip::ZipArchive;
use std::collections::BTreeMap;
use std::path::PathBuf;

/// Cache format directory prefix matching `PackageResolvers.java:63`.
pub const CACHE_DIR_PREFIX: &str = "package-2";

/// Encodes reserved Windows filename characters (`0x00..=0x1f`, `<>:"\|?*`) as `(<2-hex>)`
/// and `(` as `((` on ALL operating systems, matching `IoUtils.encodePath` (`IoUtils.java:838-854`).
///
/// Forward slash `/` is preserved as a path separator.
pub fn encode_cache_path(path: &str) -> String {
    if path.is_empty() {
        return String::new();
    }
    let mut out = String::with_capacity(path.len());
    for ch in path.chars() {
        if is_reserved_windows_filename_char(ch) && ch != '/' {
            use std::fmt::Write;
            let byte = ch as u8;
            let _ = write!(&mut out, "({byte:02x})");
        } else if ch == '(' {
            out.push_str("((");
        } else {
            out.push(ch);
        }
    }
    out
}

fn is_reserved_windows_filename_char(ch: char) -> bool {
    matches!(
        ch,
        '\u{0000}'..='\u{001f}' | '<' | '>' | ':' | '"' | '\\' | '/' | '|' | '?' | '*'
    )
}

/// Computes the effective encoded path component for a `PackageUri` (stripping `::checksum`
/// only when `package.sha256.is_some()`), matching `PackageResolvers.java:449-457`.
pub fn effective_package_cache_path(package: &PackageUri) -> Option<String> {
    let uri_str = package.uri.as_str()?;
    let without_scheme = uri_str
        .strip_prefix("package://")
        .or_else(|| uri_str.strip_prefix("projectpackage://"))?;
    let (_, raw_path_and_query) = without_scheme.split_once('/')?;
    let raw_path = raw_path_and_query
        .split(['?', '#'])
        .next()
        .unwrap_or(raw_path_and_query);
    let stripped = if package.sha256.is_some()
        && let Some(idx) = raw_path.rfind("::")
    {
        &raw_path[..idx]
    } else {
        raw_path
    };
    Some(encode_cache_path(stripped))
}

/// Computes the relative cache directory path for a package within a cache root:
/// `package-2/<encode_cache_path(authority)>/<encode_cache_path(path)>`.
///
/// Rejects any path containing `..` segments that could escape the cache directory
/// (`PackageResolvers.java:459-471`).
pub fn get_cache_rel_path(package: &PackageUri) -> Option<PathBuf> {
    let uri_str = package.uri.as_str()?;
    let without_scheme = uri_str
        .strip_prefix("package://")
        .or_else(|| uri_str.strip_prefix("projectpackage://"))?;
    let (authority, raw_path_and_query) = without_scheme.split_once('/')?;
    if authority.is_empty() {
        return None;
    }
    let raw_path = raw_path_and_query
        .split(['?', '#'])
        .next()
        .unwrap_or(raw_path_and_query);
    let effective_path = if package.sha256.is_some()
        && let Some(idx) = raw_path.rfind("::")
    {
        &raw_path[..idx]
    } else {
        raw_path
    };

    let encoded_authority = encode_cache_path(authority);
    let encoded_path = encode_cache_path(effective_path);

    let mut rel = PathBuf::from(CACHE_DIR_PREFIX);
    if encoded_authority == ".." || encoded_authority == "." || encoded_authority.contains('/') {
        return None;
    }
    rel.push(encoded_authority);

    let mut depth: usize = 1; // authority segment
    for segment in encoded_path.split('/') {
        if segment.is_empty() || segment == "." {
            continue;
        }
        if segment == ".." {
            if depth == 0 {
                return None;
            }
            return None;
        }
        depth += 1;
        rel.push(segment);
    }
    if depth < 2 {
        return None;
    }
    Some(rel)
}

/// Extracts the encoded leaf name of a package for `<leaf>.json` / `<leaf>.zip` cache filenames.
pub fn package_leaf_name(package: &PackageUri) -> Option<String> {
    let effective = effective_package_cache_path(package)?;
    let leaf = effective.rsplit('/').next()?;
    if leaf.is_empty() || leaf == "." || leaf == ".." {
        return None;
    }
    Some(leaf.to_string())
}

/// Pure relative cache paths for a package's metadata JSON and ZIP files.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PackageCachePaths {
    pub dir: PathBuf,
    pub metadata_file: PathBuf,
    pub zip_file: PathBuf,
}

impl PackageCachePaths {
    pub fn for_package(package: &PackageUri) -> Option<Self> {
        let dir = get_cache_rel_path(package)?;
        let leaf = package_leaf_name(package)?;
        Some(Self {
            metadata_file: dir.join(format!("{leaf}.json")),
            zip_file: dir.join(format!("{leaf}.zip")),
            dir,
        })
    }
}

/// Pure in-memory package cache storing metadata and decompressed archives.
#[derive(Clone, Default)]
pub struct PackageCache {
    metadata: BTreeMap<String, DependencyMetadata>,
    metadata_bytes: BTreeMap<String, Vec<u8>>,
    archives: BTreeMap<String, ZipArchive>,
}

impl PackageCache {
    pub const fn new() -> Self {
        Self {
            metadata: BTreeMap::new(),
            metadata_bytes: BTreeMap::new(),
            archives: BTreeMap::new(),
        }
    }

    fn cache_key(package: &PackageUri) -> Option<String> {
        // Key by full external package URI including any embedded `::sha256:...`
        // checksum (`PackageResolvers.java:69`) so differing checksums never collide
        // while `package:` and `projectpackage:` with identical checksums still share
        // cached entries.
        package
            .to_external_package_uri()
            .uri
            .as_str()
            .map(str::to_string)
    }

    pub fn get_metadata(&self, package: &PackageUri) -> Option<DependencyMetadata> {
        let key = Self::cache_key(package)?;
        self.metadata.get(&key).cloned()
    }

    pub fn insert_metadata(&mut self, package: &PackageUri, metadata: DependencyMetadata) {
        if let Some(key) = Self::cache_key(package) {
            self.metadata.insert(key, metadata);
        }
    }

    pub fn get_metadata_bytes(&self, package: &PackageUri) -> Option<Vec<u8>> {
        let key = Self::cache_key(package)?;
        self.metadata_bytes.get(&key).cloned()
    }

    pub fn insert_metadata_bytes(&mut self, package: &PackageUri, bytes: Vec<u8>) {
        if let Some(key) = Self::cache_key(package) {
            self.metadata_bytes.insert(key, bytes);
        }
    }

    pub fn remove_metadata(&mut self, package: &PackageUri) {
        if let Some(key) = Self::cache_key(package) {
            self.metadata.remove(&key);
            self.metadata_bytes.remove(&key);
        }
    }

    pub fn get_archive(&self, package: &PackageUri) -> Option<ZipArchive> {
        let key = Self::cache_key(package)?;
        self.archives.get(&key).cloned()
    }

    pub fn insert_archive(&mut self, package: &PackageUri, archive: ZipArchive) {
        if let Some(key) = Self::cache_key(package) {
            self.archives.insert(key, archive);
        }
    }

    pub fn clear(&mut self) {
        self.metadata.clear();
        self.metadata_bytes.clear();
        self.archives.clear();
    }
}

/// Result of reading a cached ZIP archive from persistent storage.
#[derive(Clone, Debug)]
pub enum CachedZipRead {
    Hit(ZipArchive),
    Miss,
}

/// Host-provided disk cache persistence trait.
///
/// Mirrors `DiskCachedPackageResolver` (`PackageResolvers.java:426-687`):
/// - Separate metadata `.json` and `.zip` cache operations
/// - Corrupt cached `.json` deletion (`delete_metadata`)
/// - Corrupt cached `.zip` propagates an error instead of returning `None`
pub trait PackageCacheStore: Send + Sync {
    /// Read raw metadata `.json` bytes from persistent cache if present.
    fn read_metadata_bytes(
        &self,
        _package: &PackageUri,
    ) -> Result<Option<Vec<u8>>, ModuleHostError> {
        Ok(None)
    }

    /// Persist metadata `.json` bytes atomically (e.g. via `<cache>/tmp` + atomic rename + `0444`).
    fn write_metadata_bytes(
        &self,
        _package: &PackageUri,
        _metadata_bytes: &[u8],
    ) -> Result<(), ModuleHostError> {
        Ok(())
    }

    /// Delete a corrupt cached metadata `.json` file (`PackageResolvers.java:544,553`).
    fn delete_metadata(&self, _package: &PackageUri) -> Result<(), ModuleHostError> {
        Ok(())
    }

    /// Read raw cached `.zip` bytes if present (allows caller to verify checksum after parsing).
    fn read_zip_bytes(&self, _package: &PackageUri) -> Result<Option<Vec<u8>>, ModuleHostError> {
        Ok(None)
    }

    /// Read and validate a cached `.zip` archive. Returns `Err` if the cached `.zip` exists
    /// but is corrupt/truncated/invalid, or `Ok(None)` on cache miss.
    fn read_zip_archive(
        &self,
        package: &PackageUri,
    ) -> Result<Option<ZipArchive>, ModuleHostError> {
        Ok(self.try_read_disk_archive(package))
    }

    /// Persist `.zip` bytes atomically (e.g. via `<cache>/tmp` + atomic rename + `0444`).
    fn write_zip_bytes(
        &self,
        _package: &PackageUri,
        _zip_bytes: &[u8],
    ) -> Result<(), ModuleHostError> {
        Ok(())
    }

    /// Legacy M2 helper retained for compatibility with existing adapters.
    fn try_read_disk_archive(&self, _package: &PackageUri) -> Option<ZipArchive> {
        None
    }

    /// Legacy M2 helper retained for compatibility with existing adapters.
    fn try_write_disk_archive(
        &self,
        package: &PackageUri,
        metadata_bytes: &[u8],
        zip_bytes: &[u8],
    ) -> Result<(), ModuleHostError> {
        self.write_metadata_bytes(package, metadata_bytes)?;
        self.write_zip_bytes(package, zip_bytes)
    }
}

/// Pure in-memory `PackageCacheStore` for Wasm and unit tests.
#[derive(Default)]
pub struct InMemoryPackageCacheStore {
    inner: std::sync::RwLock<InMemoryStoreData>,
}

#[derive(Default)]
struct InMemoryStoreData {
    metadata: BTreeMap<PathBuf, Vec<u8>>,
    zips: BTreeMap<PathBuf, Vec<u8>>,
}

impl InMemoryPackageCacheStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn seed_raw_metadata(&self, package: &PackageUri, bytes: Vec<u8>) {
        if let Some(paths) = PackageCachePaths::for_package(package) {
            self.inner
                .write()
                .unwrap_or_else(|e| e.into_inner())
                .metadata
                .insert(paths.metadata_file, bytes);
        }
    }

    pub fn seed_raw_zip(&self, package: &PackageUri, bytes: Vec<u8>) {
        if let Some(paths) = PackageCachePaths::for_package(package) {
            self.inner
                .write()
                .unwrap_or_else(|e| e.into_inner())
                .zips
                .insert(paths.zip_file, bytes);
        }
    }

    pub fn has_metadata(&self, package: &PackageUri) -> bool {
        PackageCachePaths::for_package(package).is_some_and(|paths| {
            self.inner
                .read()
                .unwrap_or_else(|e| e.into_inner())
                .metadata
                .contains_key(&paths.metadata_file)
        })
    }

    pub fn has_zip(&self, package: &PackageUri) -> bool {
        PackageCachePaths::for_package(package).is_some_and(|paths| {
            self.inner
                .read()
                .unwrap_or_else(|e| e.into_inner())
                .zips
                .contains_key(&paths.zip_file)
        })
    }
}

impl PackageCacheStore for InMemoryPackageCacheStore {
    fn read_metadata_bytes(
        &self,
        package: &PackageUri,
    ) -> Result<Option<Vec<u8>>, ModuleHostError> {
        let Some(paths) = PackageCachePaths::for_package(package) else {
            return Ok(None);
        };
        Ok(self
            .inner
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .metadata
            .get(&paths.metadata_file)
            .cloned())
    }

    fn write_metadata_bytes(
        &self,
        package: &PackageUri,
        metadata_bytes: &[u8],
    ) -> Result<(), ModuleHostError> {
        if let Some(paths) = PackageCachePaths::for_package(package) {
            self.inner
                .write()
                .unwrap_or_else(|e| e.into_inner())
                .metadata
                .insert(paths.metadata_file, metadata_bytes.to_vec());
        }
        Ok(())
    }

    fn delete_metadata(&self, package: &PackageUri) -> Result<(), ModuleHostError> {
        if let Some(paths) = PackageCachePaths::for_package(package) {
            self.inner
                .write()
                .unwrap_or_else(|e| e.into_inner())
                .metadata
                .remove(&paths.metadata_file);
        }
        Ok(())
    }

    fn read_zip_bytes(&self, package: &PackageUri) -> Result<Option<Vec<u8>>, ModuleHostError> {
        let Some(paths) = PackageCachePaths::for_package(package) else {
            return Ok(None);
        };
        Ok(self
            .inner
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .zips
            .get(&paths.zip_file)
            .cloned())
    }

    fn read_zip_archive(
        &self,
        package: &PackageUri,
    ) -> Result<Option<ZipArchive>, ModuleHostError> {
        let Some(bytes) = self.read_zip_bytes(package)? else {
            return Ok(None);
        };
        let archive = ZipArchive::parse(&bytes).map_err(|err| {
            ModuleHostError::Io(format!("Failed to read cached ZIP archive: {err}").into())
        })?;
        Ok(Some(archive))
    }

    fn write_zip_bytes(
        &self,
        package: &PackageUri,
        zip_bytes: &[u8],
    ) -> Result<(), ModuleHostError> {
        if let Some(paths) = PackageCachePaths::for_package(package) {
            self.inner
                .write()
                .unwrap_or_else(|e| e.into_inner())
                .zips
                .insert(paths.zip_file, zip_bytes.to_vec());
        }
        Ok(())
    }

    fn try_read_disk_archive(&self, package: &PackageUri) -> Option<ZipArchive> {
        self.read_zip_archive(package).ok().flatten()
    }
}
