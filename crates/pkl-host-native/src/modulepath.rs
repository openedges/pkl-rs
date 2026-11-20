//! Full native `ModulePathResolver` and standalone `jar:file:` (`GenericUrl`) support
//! matching Pkl 0.32.0 `ModulePathResolver.java`, `ModuleKeyFactories.ModulePath`,
//! `ModuleKeys.ModulePath`, `ModuleKeys.GenericUrl`, and `ResourceReaders.ModulePath`.

use crate::fs::{NativeFs, path_to_file_uri};
use crate::zip_archive::ZipReader;
use pkl_runtime::host::modulepath::{
    format_jar_file_uri, is_zip_or_executable_jar_magic, parse_jar_file_uri, parse_modulepath_uri,
    should_index_modulepath_entry,
};
use pkl_runtime::host::{IoFailure, ReadFailure};
use pkl_runtime::modules::{ModuleHostError, ResolvedModule};
use pkl_runtime::value::PklString;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug)]
enum IndexedTarget {
    DirFile {
        real_path: PathBuf,
        resolved_uri: PklString,
    },
    ArchiveEntry {
        archive: Arc<ZipReader>,
        entry_path: String,
        resolved_uri: PklString,
    },
}

struct ResolverState {
    initialized: bool,
    closed: bool,
    entries: BTreeMap<String, IndexedTarget>,
    /// Canonical archive `file:///...` URI -> parsed archive (for `jar:file:` -> `modulepath:` roundtrip).
    mounted_jars: BTreeMap<String, Arc<ZipReader>>,
}

impl ResolverState {
    fn new() -> Self {
        Self {
            initialized: false,
            closed: false,
            entries: BTreeMap::new(),
            mounted_jars: BTreeMap::new(),
        }
    }
}

/// Lazily-indexed multi-entry native `ModulePathResolver` supporting directories,
/// `.jar`/`.zip`/ZIP-magic/executable-JAR archives, first-entry-wins precedence,
/// `.class` filtering, silent ignore of missing entries, `jar:file:` roundtrip,
/// `hasElement` (triple-dot `...`), and `close()`/`Drop` cleanup.
pub struct NativeModulePathResolver {
    roots: Vec<PathBuf>,
    state: Mutex<ResolverState>,
}

impl NativeModulePathResolver {
    pub fn new(roots: Vec<PathBuf>) -> Self {
        Self {
            roots,
            state: Mutex::new(ResolverState::new()),
        }
    }

    pub fn is_closed(&self) -> bool {
        let guard = self.state.lock().unwrap_or_else(|e| e.into_inner());
        guard.closed
    }

    /// Releases cached archive handles and marks the resolver closed (`ModulePathResolver.close`).
    pub fn close(&self) {
        let mut guard = self.state.lock().unwrap_or_else(|e| e.into_inner());
        guard.closed = true;
        guard.entries.clear();
        guard.mounted_jars.clear();
    }

    fn ensure_indexed<'a>(
        &self,
        guard: &'a mut ResolverState,
    ) -> Result<&'a ResolverState, String> {
        if guard.closed {
            return Err("ModulePathResolver has already been closed.".to_string());
        }
        if !guard.initialized {
            for root in &self.roots {
                if !root.exists() {
                    // Missing modulepath entries are silently ignored (`ModulePathResolver.java:74-86`).
                    continue;
                }
                let Ok(real_root) = fs::canonicalize(root) else {
                    continue;
                };
                if real_root.is_dir() {
                    index_directory(&real_root, &real_root, &mut guard.entries)?;
                } else if real_root.is_file() && is_archive_candidate(&real_root) {
                    let bytes = fs::read(&real_root).map_err(|err| {
                        format!(
                            "Failed to read modulepath archive `{}`: {err}",
                            real_root.display()
                        )
                    })?;
                    let archive = Arc::new(ZipReader::parse(&bytes).map_err(|err| {
                        format!(
                            "Failed to parse modulepath archive `{}`: {err}",
                            real_root.display()
                        )
                    })?);
                    let archive_file_uri = path_to_file_uri(&real_root).to_java_utf8();
                    guard
                        .mounted_jars
                        .entry(archive_file_uri.clone())
                        .or_insert_with(|| archive.clone());
                    for (entry_path, _) in archive.entries() {
                        if !should_index_modulepath_entry(entry_path) {
                            continue;
                        }
                        let resolved_uri = format_jar_file_uri(&archive_file_uri, entry_path);
                        guard
                            .entries
                            .entry(entry_path.to_string())
                            .or_insert_with(|| IndexedTarget::ArchiveEntry {
                                archive: archive.clone(),
                                entry_path: entry_path.to_string(),
                                resolved_uri,
                            });
                    }
                }
            }
            guard.initialized = true;
        }
        Ok(&*guard)
    }

    /// Maps a `jar:file:` URI back to `modulepath:/...` if its archive is mounted in this resolver
    /// (`ModuleKeyFactories.ModulePath.create` roundtrip).
    pub fn modulepath_uri_for_jar(
        &self,
        jar_uri: &PklString,
    ) -> Result<Option<PklString>, ModuleHostError> {
        let Some(parsed) = parse_jar_file_uri(jar_uri) else {
            return Ok(None);
        };
        let mut guard = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let state = self
            .ensure_indexed(&mut guard)
            .map_err(|msg| ModuleHostError::Io(IoFailure::from(msg)))?;

        // Check both original archive URI and canonicalized archive URI.
        let mut matched = state.mounted_jars.contains_key(&parsed.archive_file_uri);
        if !matched && let Ok(real_archive) = fs::canonicalize(&parsed.archive_path) {
            let real_uri = path_to_file_uri(&real_archive).to_java_utf8();
            matched = state.mounted_jars.contains_key(&real_uri);
        }
        if matched {
            let encoded =
                pkl_runtime::host::quote_uri_path(&format!("/{}", parsed.entry_path).into());
            Ok(Some(PklString::from("modulepath:").concat(&encoded)))
        } else {
            Ok(None)
        }
    }

    /// `ModuleKeys.ModulePath.hasElement` / `ResourceReaders.ModulePath.hasElement`.
    pub fn has_element(
        &self,
        uri: &PklString,
        fs_host: &NativeFs,
        is_resource: bool,
    ) -> Result<bool, ModuleHostError> {
        if is_resource {
            fs_host.check_resolve_resource(uri)?;
        } else {
            fs_host.check_resolve_module(uri)?;
        }
        let rel =
            parse_modulepath_uri(uri).map_err(|reason| ModuleHostError::UriSyntax { reason })?;
        let mut guard = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let state = self
            .ensure_indexed(&mut guard)
            .map_err(|msg| ModuleHostError::Io(IoFailure::from(msg)))?;
        if state.entries.contains_key(&rel) {
            return Ok(true);
        }
        let prefix = format!("{rel}/");
        Ok(state.entries.keys().any(|k| k.starts_with(&prefix)))
    }

    /// Resolves a `modulepath:/...` module URI and returns `(ResolvedModule, source_bytes)`.
    pub fn resolve_module(
        &self,
        uri: &PklString,
        fs_host: &NativeFs,
    ) -> Result<(ResolvedModule, Vec<u8>), ModuleHostError> {
        fs_host.check_resolve_module(uri)?;
        let rel =
            parse_modulepath_uri(uri).map_err(|reason| ModuleHostError::UriSyntax { reason })?;
        let mut guard = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let state = self
            .ensure_indexed(&mut guard)
            .map_err(|msg| ModuleHostError::Io(IoFailure::from(msg)))?;
        let target = state
            .entries
            .get(&rel)
            .cloned()
            .ok_or(ModuleHostError::NotFound)?;
        drop(guard);

        match target {
            IndexedTarget::DirFile {
                real_path,
                resolved_uri,
            } => {
                let bytes = fs::read(&real_path).map_err(|err| {
                    if err.kind() == std::io::ErrorKind::NotFound {
                        ModuleHostError::NotFound
                    } else {
                        ModuleHostError::Io(IoFailure::from(err.to_string()))
                    }
                })?;
                Ok((
                    ResolvedModule {
                        original_uri: uri.clone(),
                        resolved_uri,
                        source_uri: uri.clone(),
                    },
                    bytes,
                ))
            }
            IndexedTarget::ArchiveEntry {
                archive,
                entry_path,
                resolved_uri,
            } => {
                let bytes = archive
                    .get(&entry_path)
                    .ok_or(ModuleHostError::NotFound)?
                    .to_vec();
                Ok((
                    ResolvedModule {
                        original_uri: uri.clone(),
                        resolved_uri,
                        source_uri: uri.clone(),
                    },
                    bytes,
                ))
            }
        }
    }

    /// Reads a `modulepath:/...` resource (`ResourceReaders.ModulePath.read`).
    pub fn read_resource(
        &self,
        uri: &PklString,
        fs_host: &NativeFs,
    ) -> Result<Option<Vec<u8>>, ReadFailure> {
        fs_host.check_read_resource(uri)?;
        let rel = parse_modulepath_uri(uri).map_err(|reason| ReadFailure::UriSyntax { reason })?;
        let mut guard = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let state = self
            .ensure_indexed(&mut guard)
            .map_err(|msg| ReadFailure::Io(IoFailure::from(msg)))?;
        let Some(target) = state.entries.get(&rel).cloned() else {
            return Ok(None);
        };
        drop(guard);

        match target {
            IndexedTarget::DirFile { real_path, .. } => match fs::read(&real_path) {
                Ok(bytes) => Ok(Some(bytes)),
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
                Err(err) => Err(ReadFailure::Io(IoFailure::from(err.to_string()))),
            },
            IndexedTarget::ArchiveEntry {
                archive,
                entry_path,
                ..
            } => Ok(archive.get(&entry_path).map(<[u8]>::to_vec)),
        }
    }
}

impl Drop for NativeModulePathResolver {
    fn drop(&mut self) {
        self.close();
    }
}

/// Resolves a standalone `jar:file:///path/to/archive.jar!/entry/path` module URI
/// (`ModuleKeys.GenericUrl`), enforcing allowlist + `rootDir` against the archive's `file:` path.
pub fn resolve_generic_jar_module(
    uri: &PklString,
    fs_host: &NativeFs,
) -> Result<(ResolvedModule, Vec<u8>), ModuleHostError> {
    fs_host.check_resolve_module(uri)?;
    let parsed = parse_jar_file_uri(uri).ok_or_else(|| ModuleHostError::UriSyntax {
        reason: format!("Invalid `jar:file:` URI `{uri}`.").into(),
    })?;
    let archive_path = PathBuf::from(&parsed.archive_path);
    let real_archive = match fs::canonicalize(&archive_path) {
        Ok(real) => real,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return Err(ModuleHostError::NotFound);
        }
        Err(err) => return Err(ModuleHostError::Io(IoFailure::from(err.to_string()))),
    };
    if let Err(msg) =
        fs_host
            .policy()
            .check_real_path_under_root_dir(uri, &real_archive.to_string_lossy(), false)
    {
        return Err(ModuleHostError::SecurityManagerException { message: Some(msg) });
    }
    let archive_bytes = fs::read(&real_archive).map_err(|err| {
        if err.kind() == std::io::ErrorKind::NotFound {
            ModuleHostError::NotFound
        } else {
            ModuleHostError::Io(IoFailure::from(err.to_string()))
        }
    })?;
    let zip = ZipReader::parse(&archive_bytes)
        .map_err(|err| ModuleHostError::Io(IoFailure::from(err)))?;
    let entry_bytes = zip
        .get(&parsed.entry_path)
        .ok_or(ModuleHostError::NotFound)?
        .to_vec();
    let canonical_uri = format_jar_file_uri(
        &path_to_file_uri(&real_archive).to_java_utf8(),
        &parsed.entry_path,
    );
    Ok((
        ResolvedModule {
            original_uri: uri.clone(),
            resolved_uri: canonical_uri,
            source_uri: uri.clone(),
        },
        entry_bytes,
    ))
}

/// Reads a standalone `jar:file:///path/to/archive.jar!/entry/path` resource URI
/// (`ResourceReaders.UrlResource`), enforcing allowlist + `rootDir` against the archive's `file:` path.
pub fn read_generic_jar_resource(
    uri: &PklString,
    fs_host: &NativeFs,
) -> Result<Option<Vec<u8>>, ReadFailure> {
    fs_host.check_read_resource(uri)?;
    let parsed = parse_jar_file_uri(uri).ok_or_else(|| ReadFailure::UriSyntax {
        reason: format!("Invalid `jar:file:` URI `{uri}`.").into(),
    })?;
    let archive_path = PathBuf::from(&parsed.archive_path);
    let real_archive = match fs::canonicalize(&archive_path) {
        Ok(real) => real,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(ReadFailure::Io(IoFailure::from(err.to_string()))),
    };
    if let Err(msg) =
        fs_host
            .policy()
            .check_real_path_under_root_dir(uri, &real_archive.to_string_lossy(), true)
    {
        return Err(ReadFailure::SecurityManagerException { message: Some(msg) });
    }
    let archive_bytes = match fs::read(&real_archive) {
        Ok(b) => b,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(ReadFailure::Io(IoFailure::from(err.to_string()))),
    };
    let zip =
        ZipReader::parse(&archive_bytes).map_err(|err| ReadFailure::Io(IoFailure::from(err)))?;
    Ok(zip.get(&parsed.entry_path).map(<[u8]>::to_vec))
}

fn is_archive_candidate(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }
    if let Some(ext) = path.extension().and_then(|e| e.to_str())
        && (ext.eq_ignore_ascii_case("jar") || ext.eq_ignore_ascii_case("zip"))
    {
        return true;
    }
    let Ok(mut file) = fs::File::open(path) else {
        return false;
    };
    // Exact `ModulePathResolver.isJarOrZipFile` (`ModulePathResolver.java:176-187`):
    // read up to 39 bytes and check ZIP local header OR exact 39-byte executable-JAR header.
    let mut header = [0u8; 39];
    let mut filled = 0usize;
    while filled < header.len() {
        match std::io::Read::read(&mut file, &mut header[filled..]) {
            Ok(0) => break,
            Ok(n) => filled += n,
            Err(_) => return false,
        }
    }
    let slice = &header[..filled];
    if slice.len() >= 4 && &slice[..4] == b"PK\x03\x04" {
        return true;
    }
    if slice == b"#!/bin/sh\n      exec java  -jar $0 \"$@\"" {
        return true;
    }
    is_zip_or_executable_jar_magic(slice)
}

fn index_directory(
    root: &Path,
    dir: &Path,
    entries: &mut BTreeMap<String, IndexedTarget>,
) -> Result<(), String> {
    let mut dir_entries: Vec<PathBuf> = fs::read_dir(dir)
        .map_err(|err| {
            format!(
                "Failed to read modulepath directory `{}`: {err}",
                dir.display()
            )
        })?
        .filter_map(|e| e.ok().map(|entry| entry.path()))
        .collect();
    dir_entries.sort();

    for path in dir_entries {
        // Use `symlink_metadata` (do NOT follow symlinks, matching `Files.find` without
        // `FOLLOW_LINKS` in `ModulePathResolver.java:146`) to avoid symlink directory cycles.
        let Ok(meta) = fs::symlink_metadata(&path) else {
            continue;
        };
        if meta.is_symlink() {
            continue;
        }
        if meta.is_dir() {
            index_directory(root, &path, entries)?;
        } else if meta.is_file() {
            let Ok(rel) = path.strip_prefix(root) else {
                continue;
            };
            let rel_str = rel.to_string_lossy().replace('\\', "/");
            if !should_index_modulepath_entry(&rel_str) {
                continue;
            }
            let Ok(real_path) = fs::canonicalize(&path) else {
                continue;
            };
            let resolved_uri = path_to_file_uri(&real_path);
            entries
                .entry(rel_str)
                .or_insert_with(|| IndexedTarget::DirFile {
                    real_path,
                    resolved_uri,
                });
        }
    }
    Ok(())
}
