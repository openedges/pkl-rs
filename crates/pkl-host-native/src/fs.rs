//! Secure native filesystem host operations matching Pkl 0.32.0:
//! - `SecurityManagers.Standard` (`rootDir`, `jar:file:` unwrap, UNC fast-reject, `resolveSecurePath`)
//! - `ModuleKeys.File` (pre-resolve + post-`toRealPath` allowlist/`rootDir` checks, `O_NOFOLLOW`)
//! - `ResourceReaders.FileResource` (leading `/` validation, `resolveSecurePath` IOException wrap, dir reads)
//! - `FileResolver` (glob `listElements` skips symlinks, `hasElement` follows symlinks)

use pkl_runtime::glob::PathElement;
use pkl_runtime::host::security::{
    SecurityPolicy, extract_file_uri_for_root_dir, normalize_lexical_path,
    validate_file_uri_has_leading_slash,
};
use pkl_runtime::host::{IoFailure, ReadFailure, quote_uri_path};
use pkl_runtime::modules::{ModuleHostError, ResolvedModule};
use pkl_runtime::value::PklString;
use std::fs::{self, File, OpenOptions};
use std::io::Read;
use std::path::{Path, PathBuf};
use unicode_normalization::UnicodeNormalization;

/// Normalizes a `rootDir` path matching `SecurityManagers.Standard.normalizePath`
/// (`toRealPath` if existing, else absolute + lexically normalized).
pub fn canonicalize_root_dir(path: &Path) -> std::io::Result<PathBuf> {
    if path.exists() {
        fs::canonicalize(path)
    } else if path.is_absolute() {
        Ok(PathBuf::from(normalize_lexical_path(
            &path.to_string_lossy(),
        )))
    } else {
        let abs = std::env::current_dir()?.join(path);
        Ok(PathBuf::from(normalize_lexical_path(
            &abs.to_string_lossy(),
        )))
    }
}

/// Converts a filesystem path into a canonical `file:///...` URI using Pkl's `IoUtils.toUri` quoting.
pub fn path_to_file_uri(path: &Path) -> PklString {
    let raw = path.to_string_lossy().replace('\\', "/");
    let with_slash = if raw.starts_with('/') {
        raw
    } else {
        format!("/{raw}")
    };
    PklString::from("file://").concat(&quote_uri_path(&with_slash.into()))
}

/// Converts a `file:` URI into a local filesystem `PathBuf`, applying NFC normalization
/// matching JDK 21's Unix path encoding.
pub fn file_uri_to_path(uri: &PklString) -> Result<PathBuf, PklString> {
    if let Some(s) = uri.as_str()
        && s.starts_with("file:///")
        && s.is_ascii()
        && !s.bytes().any(|b| matches!(b, b'%' | b'?' | b'#'))
    {
        return Ok(PathBuf::from(&s[7..]));
    }
    let info = extract_file_uri_for_root_dir(uri)
        .filter(|i| !i.from_jar)
        .ok_or_else(|| PklString::from(format!("Expected `file:` URI, got `{uri}`.")))?;
    if info.has_non_empty_authority {
        return Err(PklString::from(format!(
            "Unsupported remote authority in file URI `{uri}`."
        )));
    }
    if info.decoded_path.is_ascii() {
        return Ok(PathBuf::from(info.decoded_path));
    }
    let normalized = info.decoded_path.nfc().collect::<String>();
    Ok(PathBuf::from(normalized))
}

fn is_uri_scheme_prefix(s: &str) -> bool {
    let Some((scheme, _)) = s.split_once(':') else {
        return false;
    };
    let mut chars = scheme.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    first.is_ascii_alphabetic()
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
}

fn normalize_lexical_buf(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for comp in path.components() {
        match comp {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                if !out.pop() {
                    out.push(comp);
                }
            }
            other => out.push(other),
        }
    }
    out
}

#[cfg(windows)]
fn strip_windows_uri_leading_slash(path: PathBuf) -> PathBuf {
    let s = path.to_string_lossy();
    let b = s.as_bytes();
    if b.len() >= 3 && b[0] == b'/' && b[1].is_ascii_alphabetic() && b[2] == b':' {
        PathBuf::from(&s[1..])
    } else {
        path
    }
}

#[cfg(not(windows))]
fn strip_windows_uri_leading_slash(path: PathBuf) -> PathBuf {
    path
}

/// Strictly resolves a local `import(...)` target inside `try_parse_static_project`.
///
/// Returns `None` on triple-dot imports, backslashes, queries/fragments, non-empty
/// `file://` authorities, percent-encoding or `..` normalization mismatches on `file:`
/// URIs, or non-ASCII / special-character relative paths so the caller falls back to
/// full `NativeHost` evaluation with identical Pkl 0.32.0 semantics.
pub fn read_static_local_project_import(
    base_uri: &str,
    import_uri: &str,
) -> Option<(String, String)> {
    if import_uri.is_empty()
        || import_uri == "..."
        || import_uri.starts_with(".../")
        || import_uri.starts_with("pkl:")
        || import_uri.starts_with("package:")
        || import_uri.starts_with('@')
        || import_uri.contains('\\')
        || import_uri.contains('?')
        || import_uri.contains('#')
    {
        return None;
    }

    let (target, target_uri) = if import_uri.starts_with("file:") {
        let raw_path = strip_windows_uri_leading_slash(file_uri_to_path(&import_uri.into()).ok()?);
        let norm = normalize_lexical_buf(&raw_path);
        if !norm.is_file() {
            return None;
        }
        let canonical_uri = path_to_file_uri(&norm).to_java_utf8();
        if canonical_uri != import_uri {
            return None;
        }
        (norm, canonical_uri)
    } else if !is_uri_scheme_prefix(import_uri) {
        if import_uri.starts_with('/')
            || !import_uri.bytes().all(|b| {
                b > 0x20
                    && b < 0x7f
                    && !matches!(
                        b,
                        b'<' | b'>'
                            | b'"'
                            | b'{'
                            | b'}'
                            | b'|'
                            | b'\\'
                            | b'^'
                            | b'`'
                            | b'['
                            | b']'
                            | b'%'
                            | b'?'
                            | b'#'
                    )
            })
        {
            return None;
        }
        let base_path = strip_windows_uri_leading_slash(file_uri_to_path(&base_uri.into()).ok()?);
        let parent = base_path.parent()?;
        let norm = normalize_lexical_buf(&parent.join(import_uri));
        if !norm.is_file() {
            return None;
        }
        let canonical_uri = path_to_file_uri(&norm).to_java_utf8();
        (norm, canonical_uri)
    } else {
        return None;
    };

    let child_text = fs::read_to_string(&target).ok()?;
    Some((target_uri, child_text))
}

/// Resolves a path to its canonical form by canonicalizing its deepest existing ancestor
/// and appending any remaining lexically normalized components. This handles macOS
/// `/tmp -> /private/tmp` and `/var -> /private/var` cleanly even for non-existent targets.
fn resolve_canonical_or_ancestor(path: &Path) -> PathBuf {
    let lexical = PathBuf::from(normalize_lexical_path(&path.to_string_lossy()));
    if let Ok(real) = fs::canonicalize(&lexical) {
        return real;
    }
    let mut missing_tail: Vec<std::ffi::OsString> = Vec::new();
    let mut cur = lexical.as_path();
    while let Some(name) = cur.file_name() {
        missing_tail.push(name.to_os_string());
        let Some(parent) = cur.parent() else {
            break;
        };
        if let Ok(real_parent) = fs::canonicalize(parent) {
            let mut out = real_parent;
            for seg in missing_tail.into_iter().rev() {
                out.push(seg);
            }
            return out;
        }
        cur = parent;
    }
    lexical
}

/// Opens a verified real path without following symlinks (`O_NOFOLLOW` on Unix)
/// to defend against TOCTOU symlink swap attacks when `rootDir` is enforced.
pub fn open_file_nofollow(path: &Path) -> std::io::Result<File> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        // Exact OS-specific O_NOFOLLOW constants (zero `unsafe`, zero `libc` dependency).
        // NOTE: Linux kernel UAPI defines O_NOFOLLOW in OCTAL:
        // - asm-generic (x86/x86_64/riscv64/s390x/loongarch64): 00400000 = 0x20000
        // - arm / aarch64 / powerpc:                            00100000 = 0x8000
        //   (0x10000 on arm/aarch64 is O_DIRECT, which causes EINVAL on unaligned reads!)
        #[cfg(any(
            target_os = "macos",
            target_os = "ios",
            target_os = "freebsd",
            target_os = "openbsd",
            target_os = "netbsd",
            target_os = "dragonfly"
        ))]
        const O_NOFOLLOW: i32 = 0x0100;
        #[cfg(all(
            target_os = "linux",
            any(
                target_arch = "x86",
                target_arch = "x86_64",
                target_arch = "riscv64",
                target_arch = "s390x",
                target_arch = "loongarch64"
            )
        ))]
        const O_NOFOLLOW: i32 = 0o0400000;
        #[cfg(all(
            target_os = "linux",
            any(
                target_arch = "aarch64",
                target_arch = "arm",
                target_arch = "powerpc",
                target_arch = "powerpc64"
            )
        ))]
        const O_NOFOLLOW: i32 = 0o0100000;
        #[cfg(not(any(
            target_os = "macos",
            target_os = "ios",
            target_os = "freebsd",
            target_os = "openbsd",
            target_os = "netbsd",
            target_os = "dragonfly",
            all(
                target_os = "linux",
                any(
                    target_arch = "x86",
                    target_arch = "x86_64",
                    target_arch = "riscv64",
                    target_arch = "s390x",
                    target_arch = "loongarch64",
                    target_arch = "aarch64",
                    target_arch = "arm",
                    target_arch = "powerpc",
                    target_arch = "powerpc64"
                )
            )
        )))]
        const O_NOFOLLOW: i32 = 0;

        let mut opts = OpenOptions::new();
        opts.read(true);
        if O_NOFOLLOW != 0 {
            opts.custom_flags(O_NOFOLLOW);
        }
        opts.open(path)
    }
    #[cfg(not(unix))]
    {
        if fs::symlink_metadata(path)?.file_type().is_symlink() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "Refusing to follow symlink during secure file open",
            ));
        }
        OpenOptions::new().read(true).open(path)
    }
}

/// Secure native filesystem adapter bound to a `SecurityPolicy`.
#[derive(Clone, Debug)]
pub struct NativeFs {
    policy: SecurityPolicy,
}

impl NativeFs {
    pub fn new(policy: SecurityPolicy) -> Self {
        Self { policy }
    }

    pub fn policy(&self) -> &SecurityPolicy {
        &self.policy
    }

    /// Checks allowlist + real-path `rootDir` (including `jar:file:` inner archive path,
    /// UNC fast-reject, symlink resolution, and non-existent lexical/ancestor check).
    pub fn check_uri(&self, uri: &PklString, is_resource: bool) -> Result<(), PklString> {
        self.policy.check_allowlist_only(uri, is_resource)?;
        if self.policy.root_dir().is_none() {
            return Ok(());
        }
        let Some(info) = extract_file_uri_for_root_dir(uri) else {
            return Ok(());
        };
        if info.has_non_empty_authority {
            // UNC / remote host authority: reject immediately before any FS/network call.
            return Err(pkl_runtime::host::security::root_dir_denied_message(
                uri,
                is_resource,
            ));
        }
        let candidate = PathBuf::from(info.decoded_path.nfc().collect::<String>());
        let resolved = resolve_canonical_or_ancestor(&candidate);
        self.policy
            .check_real_path_under_root_dir(uri, &resolved.to_string_lossy(), is_resource)
    }

    pub fn check_resolve_module(&self, uri: &PklString) -> Result<(), ModuleHostError> {
        self.check_uri(uri, false)
            .map_err(|msg| ModuleHostError::SecurityManagerException { message: Some(msg) })
    }

    pub fn check_resolve_resource(&self, uri: &PklString) -> Result<(), ModuleHostError> {
        self.check_uri(uri, true)
            .map_err(|msg| ModuleHostError::SecurityManagerException { message: Some(msg) })
    }

    pub fn check_read_resource(&self, uri: &PklString) -> Result<(), ReadFailure> {
        self.check_uri(uri, true)
            .map_err(|msg| ReadFailure::SecurityManagerException { message: Some(msg) })
    }

    /// Equivalent to `SecurityManagers.Standard.resolveSecurePath(uri, isResource)`.
    /// Returns `Ok(Some(real_path))` when `rootDir` is configured for a local `file:` URI.
    pub fn resolve_secure_path(
        &self,
        uri: &PklString,
        is_resource: bool,
    ) -> Result<Option<PathBuf>, Result<PklString, std::io::Error>> {
        if self.policy.root_dir().is_none() {
            return Ok(None);
        }
        let Some(info) = extract_file_uri_for_root_dir(uri) else {
            return Ok(None);
        };
        if info.from_jar || info.has_non_empty_authority {
            return Ok(None);
        }
        let path = PathBuf::from(info.decoded_path.nfc().collect::<String>());
        let real = fs::canonicalize(&path).map_err(Err)?;
        if let Err(msg) =
            self.policy
                .check_real_path_under_root_dir(uri, &real.to_string_lossy(), is_resource)
        {
            return Err(Ok(msg));
        }
        Ok(Some(real))
    }

    /// Resolves a `file:` module URI (`ModuleKeys.File.resolve`), enforcing:
    /// 1. Pre-resolve `check_resolve_module(uri)`
    /// 2. Windows backslash rejection as `NotFound`
    /// 3. `toRealPath` / `resolveSecurePath`
    /// 4. Post-resolve `check_resolve_module(resolved_uri)` (symlink target check!)
    pub fn resolve_file_module(
        &self,
        uri: &PklString,
    ) -> Result<(ResolvedModule, PathBuf), ModuleHostError> {
        self.check_resolve_module(uri)?;

        if cfg!(windows) && uri.to_java_utf8().contains('\\') {
            return Err(ModuleHostError::NotFound);
        }

        let path = file_uri_to_path(uri)
            .map_err(|msg| ModuleHostError::SecurityManagerException { message: Some(msg) })?;

        let real_path = match fs::canonicalize(&path) {
            Ok(real) => real,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                return Err(ModuleHostError::NotFound);
            }
            Err(err) => return Err(ModuleHostError::Io(IoFailure::from(err.to_string()))),
        };

        let same_canonical_uri = cfg!(unix)
            && real_path == path
            && uri.as_str().is_some_and(|raw| {
                raw.strip_prefix("file://")
                    .is_some_and(|rest| rest == real_path.to_str().unwrap_or(""))
            });
        let resolved_uri = if same_canonical_uri {
            uri.clone()
        } else {
            path_to_file_uri(&real_path)
        };

        // Post-resolve check catches symlinks pointing to disallowed or outside-rootDir paths.
        if !same_canonical_uri
            && let Err(msg) = self.policy.check_allowlist_only(&resolved_uri, false)
        {
            return Err(ModuleHostError::SecurityManagerException { message: Some(msg) });
        }
        if self.policy.root_dir().is_some()
            && let Err(msg) =
                self.policy
                    .check_real_path_under_root_dir(uri, &real_path.to_string_lossy(), false)
        {
            return Err(ModuleHostError::SecurityManagerException { message: Some(msg) });
        }

        Ok((
            ResolvedModule {
                original_uri: uri.clone(),
                resolved_uri,
                source_uri: uri.clone(),
            },
            real_path,
        ))
    }

    /// Reads module source bytes using `O_NOFOLLOW` when `rootDir` is set.
    pub fn read_file_module_bytes(&self, real_path: &Path) -> Result<Vec<u8>, ModuleHostError> {
        let mut file = if self.policy.root_dir().is_some() {
            open_file_nofollow(real_path)
        } else {
            File::open(real_path)
        }
        .map_err(|err| {
            if err.kind() == std::io::ErrorKind::NotFound {
                ModuleHostError::NotFound
            } else {
                ModuleHostError::Io(IoFailure::from(err.to_string()))
            }
        })?;
        // Fast path for typical small Pkl modules (< 8 KiB): avoids `fstat` + `lseek`
        // syscalls that `File::read_to_end` issues before reading.
        let mut stack_buf = [0u8; 8192];
        let mut total = 0usize;
        loop {
            match file.read(&mut stack_buf[total..]) {
                Ok(0) => return Ok(stack_buf[..total].to_vec()),
                Ok(n) => {
                    total += n;
                    if total == stack_buf.len() {
                        let mut bytes = Vec::with_capacity(16384);
                        bytes.extend_from_slice(&stack_buf);
                        file.read_to_end(&mut bytes)
                            .map_err(|err| ModuleHostError::Io(IoFailure::from(err.to_string())))?;
                        return Ok(bytes);
                    }
                }
                Err(err) if err.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(err) => return Err(ModuleHostError::Io(IoFailure::from(err.to_string()))),
            }
        }
    }

    /// Reads a `file:` resource (`ResourceReaders.FileResource.read`), returning `Ok(None)`
    /// for missing files (`read?` -> `null`, `read` -> `cannotFindResource`).
    pub fn read_file_resource(&self, uri: &PklString) -> Result<Option<Vec<u8>>, ReadFailure> {
        // 1. ResourceManager.read upfront security check (direct SecurityManagerException)
        self.check_read_resource(uri)?;

        // 2. FileResource validates leading `/`
        if let Err(reason) = validate_file_uri_has_leading_slash(uri) {
            return Err(ReadFailure::UriSyntax { reason });
        }

        // 3. resolveSecurePath (wraps race SecurityManagerException in IOException, per ResourceReaders.java:275)
        let target_path = match self.resolve_secure_path(uri, true) {
            Ok(Some(real)) => real,
            Ok(None) => file_uri_to_path(uri)
                .map_err(|msg| ReadFailure::SecurityManagerException { message: Some(msg) })?,
            Err(Ok(sec_msg)) => {
                return Err(SecurityPolicy::wrap_file_resource_security_error(sec_msg));
            }
            Err(Err(io_err)) if io_err.kind() == std::io::ErrorKind::NotFound => {
                return Ok(None);
            }
            Err(Err(io_err)) => return Err(ReadFailure::Io(IoFailure::from(io_err.to_string()))),
        };

        let meta = match fs::metadata(&target_path) {
            Ok(m) => m,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(err) => return Err(ReadFailure::Io(IoFailure::from(err.to_string()))),
        };

        if meta.is_dir() {
            // JDK FileURLConnection lists directory entries (including symlinks), sorted.
            let mut names = Vec::new();
            let dir = fs::read_dir(&target_path)
                .map_err(|err| ReadFailure::Io(IoFailure::from(err.to_string())))?;
            for entry in dir {
                let entry =
                    entry.map_err(|err| ReadFailure::Io(IoFailure::from(err.to_string())))?;
                names.push(entry.file_name().to_string_lossy().into_owned());
            }
            names.sort();
            let mut body = names.join("\n");
            if !body.is_empty() {
                body.push('\n');
            }
            return Ok(Some(body.into_bytes()));
        }

        let mut file = match if self.policy.root_dir().is_some() {
            open_file_nofollow(&target_path)
        } else {
            File::open(&target_path)
        } {
            Ok(f) => f,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(err) => return Err(ReadFailure::Io(IoFailure::from(err.to_string()))),
        };
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)
            .map_err(|err| ReadFailure::Io(IoFailure::from(err.to_string())))?;
        Ok(Some(bytes))
    }

    /// `FileResolver.hasElement`: checks security (including `rootDir` symlink check) first,
    /// then follows symlinks to test existence.
    pub fn has_element(&self, uri: &PklString, is_resource: bool) -> Result<bool, ModuleHostError> {
        if is_resource {
            self.check_resolve_resource(uri)?;
        } else {
            self.check_resolve_module(uri)?;
        }
        let path = file_uri_to_path(uri)
            .map_err(|msg| ModuleHostError::SecurityManagerException { message: Some(msg) })?;
        Ok(path.exists())
    }

    /// `FileResolver.listElements`: checks security (including `rootDir` symlink check on base dir),
    /// follows symlink base dir, skips symlink entries, returns empty list for missing/non-dir.
    pub fn list_elements(
        &self,
        uri: &PklString,
        is_resource: bool,
    ) -> Result<Vec<PathElement>, ModuleHostError> {
        if is_resource {
            self.check_resolve_resource(uri)?;
        } else {
            self.check_resolve_module(uri)?;
        }
        let path = file_uri_to_path(uri)
            .map_err(|msg| ModuleHostError::SecurityManagerException { message: Some(msg) })?;
        let Ok(meta) = fs::metadata(&path) else {
            return Ok(Vec::new());
        };
        if !meta.is_dir() {
            return Ok(Vec::new());
        }
        let read_dir = match fs::read_dir(&path) {
            Ok(rd) => rd,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(err) => return Err(ModuleHostError::Io(IoFailure::from(err.to_string()))),
        };
        let mut elements = Vec::new();
        for entry in read_dir {
            let entry =
                entry.map_err(|err| ModuleHostError::Io(IoFailure::from(err.to_string())))?;
            let file_type = entry
                .file_type()
                .map_err(|err| ModuleHostError::Io(IoFailure::from(err.to_string())))?;
            // FileResolver.listElements explicitly skips symlinks.
            if file_type.is_symlink() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().into_owned();
            elements.push(PathElement {
                name: name.into(),
                is_directory: file_type.is_dir(),
            });
        }
        Ok(elements)
    }
}

#[cfg(test)]
mod tests {
    use super::{open_file_nofollow, path_to_file_uri, read_static_local_project_import};
    use std::fs;
    use std::io::Read;

    #[test]
    fn open_file_nofollow_reads_regular_file_and_rejects_symlink() {
        let dir = std::env::temp_dir().join(format!(
            "pkl-host-nofollow-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        let real = dir.join("payload.pkl");
        fs::write(&real, b"hello = 42\n").unwrap();

        let mut f = open_file_nofollow(&real).expect("open_file_nofollow must read regular file");
        let mut buf = Vec::new();
        f.read_to_end(&mut buf)
            .expect("buffered read on O_NOFOLLOW handle must succeed (never O_DIRECT EINVAL)");
        assert_eq!(buf, b"hello = 42\n");

        #[cfg(unix)]
        {
            let link = dir.join("link.pkl");
            std::os::unix::fs::symlink(&real, &link).unwrap();
            let err = open_file_nofollow(&link)
                .expect_err("open_file_nofollow must reject symlinks on Unix");
            assert!(
                err.raw_os_error().is_some(),
                "expected OS error (ELOOP) on symlink, got {err:?}"
            );
        }

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn static_local_project_import_rejects_authority_percent_triple_dot_and_backslash() {
        let dir = std::env::temp_dir().join(format!(
            "pkl-static-import-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        let root = dir.join("root");
        let dep = dir.join("dep");
        let triple = root.join("...");
        fs::create_dir_all(&root).unwrap();
        fs::create_dir_all(&dep).unwrap();
        fs::create_dir_all(&triple).unwrap();

        let dep_file = dep.join("PklProject");
        fs::write(&dep_file, "amends \"pkl:Project\"\n").unwrap();
        fs::write(triple.join("PklProject"), "amends \"pkl:Project\"\n").unwrap();

        let base_uri = path_to_file_uri(&root.join("PklProject")).to_java_utf8();
        let dep_uri = path_to_file_uri(&dep_file).to_java_utf8();

        // Valid relative and canonical `file:///` imports succeed.
        let (got_rel_uri, _) =
            read_static_local_project_import(&base_uri, "../dep/PklProject").expect("relative ok");
        assert_eq!(got_rel_uri, dep_uri);
        let (got_abs_uri, _) =
            read_static_local_project_import(&base_uri, &dep_uri).expect("canonical file uri ok");
        assert_eq!(got_abs_uri, dep_uri);

        // Percent-encoded file URI must fall back (must not rewrite Pkl%50roject -> PklProject).
        let encoded_uri = dep_uri.replace("/PklProject", "/Pkl%50roject");
        assert!(read_static_local_project_import(&base_uri, &encoded_uri).is_none());

        // Non-empty authority file:// URIs must fall back.
        assert!(
            read_static_local_project_import(&base_uri, "file://localhost/PklProject").is_none()
        );
        assert!(read_static_local_project_import(&base_uri, "file://../dep/PklProject").is_none());

        // Triple-dot, backslash, absolute POSIX path, query/fragment, and percent-encoded relative must fall back.
        assert!(read_static_local_project_import(&base_uri, ".../PklProject").is_none());
        assert!(read_static_local_project_import(&base_uri, "..\\dep\\PklProject").is_none());
        assert!(read_static_local_project_import(&base_uri, "/etc/PklProject").is_none());
        assert!(read_static_local_project_import(&base_uri, "../dep/Pkl%50roject").is_none());
        assert!(read_static_local_project_import(&base_uri, &format!("{dep_uri}?q=1")).is_none());
        assert!(read_static_local_project_import(&base_uri, &format!("{dep_uri}#frag")).is_none());

        let _ = fs::remove_dir_all(&dir);
    }
}
