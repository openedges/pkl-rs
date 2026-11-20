//! Wasm-pure `modulepath:` and `jar:file:` URI parsing, validation, and in-memory index
//! matching Pkl 0.32.0 `ModulePathResolver.java`, `ModuleKeys.ModulePath`, and
//! `ResourceReaders.ModulePath`.

use crate::host::security::{normalize_lexical_path, percent_decode_utf8};
use crate::value::PklString;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};

/// Parsed `jar:file:///path/to/archive.jar!/entry/path` components.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JarFileUri {
    /// Original inner `file:` URI of the archive (e.g. `file:///path/to/archive.jar`).
    pub archive_file_uri: String,
    /// Decoded filesystem path of the archive (e.g. `/path/to/archive.jar`).
    pub archive_path: String,
    /// Normalized entry path inside the archive without leading `/` (e.g. `foo/bar.pkl`).
    pub entry_path: String,
}

/// Validates a `modulepath:` URI and returns its normalized relative entry path
/// (without leading `/`).
///
/// Rejects opaque URIs missing a leading `/` after `modulepath:` with Pkl 0.32.0's
/// `invalidModuleUriMissingSlash` message.
pub fn parse_modulepath_uri(uri: &PklString) -> Result<String, PklString> {
    let raw = uri.to_java_utf8();
    let Some(rest) = strip_scheme_ascii(&raw, "modulepath:") else {
        return Err(PklString::from(format!(
            "Expected `modulepath:` URI, got `{raw}`."
        )));
    };
    let without_frag = rest.split('#').next().unwrap_or(rest);
    let without_query = without_frag.split('?').next().unwrap_or(without_frag);
    if !without_query.starts_with('/') {
        return Err(PklString::from("Module URI `")
            .concat(uri)
            .concat(&"` is missing a `/` after `modulepath:`.".into()));
    }
    let decoded = percent_decode_utf8(without_query);
    let normalized = normalize_lexical_path(&decoded);
    Ok(normalized.trim_start_matches('/').to_string())
}

/// Parses a `jar:file:...!/...` URI into archive file URI, decoded archive path, and entry path.
pub fn parse_jar_file_uri(uri: &PklString) -> Option<JarFileUri> {
    let raw = uri.to_java_utf8();
    let rest = strip_scheme_ascii(&raw, "jar:")?;
    let (archive_uri, entry_raw) = rest.split_once("!/")?;
    let after_file = strip_scheme_ascii(archive_uri, "file:")?;
    let raw_archive_path = if let Some(after_slashes) = after_file.strip_prefix("//") {
        let idx = after_slashes.find('/')?;
        if idx > 0 {
            // Non-empty authority on file URI
            return None;
        }
        &after_slashes[idx..]
    } else {
        after_file
    };
    let entry_no_frag = entry_raw.split('#').next().unwrap_or(entry_raw);
    let entry_no_query = entry_no_frag.split('?').next().unwrap_or(entry_no_frag);
    let decoded_entry = percent_decode_utf8(entry_no_query);
    let normalized_entry = normalize_lexical_path(&format!("/{decoded_entry}"));
    Some(JarFileUri {
        archive_file_uri: archive_uri.to_string(),
        archive_path: percent_decode_utf8(raw_archive_path),
        entry_path: normalized_entry.trim_start_matches('/').to_string(),
    })
}

/// Formats a canonical `jar:<archive_file_uri>!/<entry_path>` URI matching JDK `ZipPath.toUri()`.
pub fn format_jar_file_uri(archive_file_uri: &str, entry_path: &str) -> PklString {
    let clean = entry_path.trim_start_matches('/');
    let encoded = crate::host::resources::quote_path(&clean.into());
    PklString::from(format!("jar:{archive_file_uri}!/")).concat(&encoded)
}

/// Exact 39-byte executable JAR shell preamble recognized by `ModulePathResolver.java:185-186`.
pub const EXECUTABLE_JAR_HEADER: &[u8] = b"#!/bin/sh\n      exec java  -jar $0 \"$@\"";

/// Returns true if the leading bytes of a file indicate a ZIP/JAR archive or executable JAR
/// (`ModulePathResolver.java:176-187`).
pub fn is_zip_or_executable_jar_magic(header: &[u8]) -> bool {
    if header.len() >= 4
        && (header.starts_with(&[0x50, 0x4B, 0x03, 0x04])
            || header.starts_with(&[0x50, 0x4B, 0x05, 0x06]))
    {
        return true;
    }
    if header.starts_with(EXECUTABLE_JAR_HEADER) {
        return true;
    }
    // Also recognize shell-prefixed executable JARs where ZIP header appears within the sampled prefix.
    if header.starts_with(b"#!/bin/sh") && header.windows(4).any(|w| w == [0x50, 0x4B, 0x03, 0x04])
    {
        return true;
    }
    false
}

/// Returns true if a relative entry path should be indexed in `ModulePathResolver`.
/// Mirrors `ModulePathResolver.java`: skips `.class` files and empty paths.
pub fn should_index_modulepath_entry(rel_path: &str) -> bool {
    let trimmed = rel_path.trim_start_matches('/');
    !trimmed.is_empty() && !trimmed.ends_with('/') && !trimmed.ends_with(".class")
}

/// Resolved entry inside a modulepath root.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedModulePathEntry {
    /// Normalized relative path without leading `/`.
    pub relative_path: String,
    /// Canonical resolved URI (`file:///...` or `jar:file:///...!/...`).
    pub resolved_uri: PklString,
    /// Raw content bytes.
    pub bytes: Vec<u8>,
}

/// Wasm-pure in-memory `ModulePathResolver` supporting ordered roots (directories/archives),
/// first-entry-wins precedence, `.class` filtering, `has_element`, `jar:file:` roundtrip,
/// and `close()` lifecycle semantics.
#[derive(Debug)]
pub struct InMemoryModulePath {
    entries: BTreeMap<String, ResolvedModulePathEntry>,
    jar_mounts: BTreeMap<String, Vec<String>>,
    closed: AtomicBool,
}

impl Default for InMemoryModulePath {
    fn default() -> Self {
        Self::new()
    }
}

impl InMemoryModulePath {
    pub fn new() -> Self {
        Self {
            entries: BTreeMap::new(),
            jar_mounts: BTreeMap::new(),
            closed: AtomicBool::new(false),
        }
    }

    /// Adds a directory-style root entry (`file:///...`); first root providing a relative path wins.
    pub fn add_dir_entry(
        &mut self,
        relative_path: &str,
        resolved_file_uri: impl Into<PklString>,
        bytes: Vec<u8>,
    ) {
        let rel = normalize_lexical_path(&format!("/{relative_path}"))
            .trim_start_matches('/')
            .to_string();
        if !should_index_modulepath_entry(&rel) {
            return;
        }
        self.entries
            .entry(rel.clone())
            .or_insert_with(|| ResolvedModulePathEntry {
                relative_path: rel,
                resolved_uri: resolved_file_uri.into(),
                bytes,
            });
    }

    /// Adds an archive-style root entry (`jar:file:///...!/...`); first root providing a path wins.
    pub fn add_jar_entry(&mut self, archive_file_uri: &str, relative_path: &str, bytes: Vec<u8>) {
        let rel = normalize_lexical_path(&format!("/{relative_path}"))
            .trim_start_matches('/')
            .to_string();
        if !should_index_modulepath_entry(&rel) {
            return;
        }
        let resolved_uri = format_jar_file_uri(archive_file_uri, &rel);
        self.jar_mounts
            .entry(archive_file_uri.to_string())
            .or_default()
            .push(rel.clone());
        self.entries
            .entry(rel.clone())
            .or_insert_with(|| ResolvedModulePathEntry {
                relative_path: rel,
                resolved_uri,
                bytes,
            });
    }

    pub fn is_closed(&self) -> bool {
        self.closed.load(Ordering::Acquire)
    }

    pub fn close(&self) {
        self.closed.store(true, Ordering::Release);
    }

    fn ensure_open(&self) -> Result<(), PklString> {
        if self.is_closed() {
            Err("ModulePathResolver is closed.".into())
        } else {
            Ok(())
        }
    }

    /// Converts a mounted `jar:file:` URI back into its canonical `modulepath:/...` URI
    /// (`ModuleKeyFactories.ModulePath.create` roundtrip).
    pub fn modulepath_uri_for_jar(
        &self,
        jar_uri: &PklString,
    ) -> Result<Option<PklString>, PklString> {
        self.ensure_open()?;
        let Some(parsed) = parse_jar_file_uri(jar_uri) else {
            return Ok(None);
        };
        if self.jar_mounts.contains_key(&parsed.archive_file_uri) {
            let encoded =
                crate::host::resources::quote_path(&format!("/{}", parsed.entry_path).into());
            Ok(Some(PklString::from("modulepath:").concat(&encoded)))
        } else {
            Ok(None)
        }
    }

    /// Checks whether `modulepath:/...` exists in the indexed roots (for triple-dot `...`).
    pub fn has_element(&self, uri: &PklString) -> Result<bool, PklString> {
        self.ensure_open()?;
        let rel = parse_modulepath_uri(uri)?;
        if self.entries.contains_key(&rel) {
            return Ok(true);
        }
        let prefix = format!("{rel}/");
        Ok(self.entries.keys().any(|k| k.starts_with(&prefix)))
    }

    /// Resolves a `modulepath:/...` URI to its winning entry.
    pub fn resolve(&self, uri: &PklString) -> Result<Option<&ResolvedModulePathEntry>, PklString> {
        self.ensure_open()?;
        let rel = parse_modulepath_uri(uri)?;
        Ok(self.entries.get(&rel))
    }
}

fn strip_scheme_ascii<'a>(s: &'a str, scheme_with_colon: &str) -> Option<&'a str> {
    let prefix = s.get(..scheme_with_colon.len())?;
    if prefix.eq_ignore_ascii_case(scheme_with_colon) {
        Some(&s[scheme_with_colon.len()..])
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_modulepath_uri_without_leading_slash() {
        let err = parse_modulepath_uri(&"modulepath:foo/bar.pkl".into()).unwrap_err();
        assert_eq!(
            err,
            "Module URI `modulepath:foo/bar.pkl` is missing a `/` after `modulepath:`."
        );
        assert_eq!(
            parse_modulepath_uri(&"modulepath:/foo/bar.pkl".into()).unwrap(),
            "foo/bar.pkl"
        );
    }

    #[test]
    fn first_root_wins_skips_class_files_and_supports_jar_roundtrip() {
        let mut mp = InMemoryModulePath::new();
        mp.add_dir_entry("a/b.pkl", "file:///dir1/a/b.pkl", b"first".to_vec());
        mp.add_jar_entry("file:///libs/lib.jar", "a/b.pkl", b"second".to_vec());
        mp.add_jar_entry("file:///libs/lib.jar", "a/Only.class", b"class".to_vec());
        mp.add_jar_entry("file:///libs/lib.jar", "a/c.pkl", b"from-jar".to_vec());

        let hit = mp.resolve(&"modulepath:/a/b.pkl".into()).unwrap().unwrap();
        assert_eq!(hit.resolved_uri, "file:///dir1/a/b.pkl");
        assert_eq!(hit.bytes, b"first");
        assert!(
            mp.resolve(&"modulepath:/a/Only.class".into())
                .unwrap()
                .is_none()
        );

        let jar_hit = mp.resolve(&"modulepath:/a/c.pkl".into()).unwrap().unwrap();
        assert_eq!(jar_hit.resolved_uri, "jar:file:///libs/lib.jar!/a/c.pkl");
        assert_eq!(
            mp.modulepath_uri_for_jar(&jar_hit.resolved_uri)
                .unwrap()
                .unwrap(),
            "modulepath:/a/c.pkl"
        );

        mp.close();
        assert!(mp.resolve(&"modulepath:/a/b.pkl".into()).is_err());
    }
}
