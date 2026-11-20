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
    let info = extract_file_uri_for_root_dir(uri)
        .filter(|i| !i.from_jar)
        .ok_or_else(|| PklString::from(format!("Expected `file:` URI, got `{uri}`.")))?;
    if info.has_non_empty_authority {
        return Err(PklString::from(format!(
            "Unsupported remote authority in file URI `{uri}`."
        )));
    }
    let normalized = info.decoded_path.nfc().collect::<String>();
    Ok(PathBuf::from(normalized))
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
            any(target_arch = "x86", target_arch = "x86_64", target_arch = "riscv64")
        ))]
        const O_NOFOLLOW: i32 = 0x20000;
        #[cfg(all(target_os = "linux", any(target_arch = "aarch64", target_arch = "arm")))]
        const O_NOFOLLOW: i32 = 0x10000;
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
                    target_arch = "aarch64",
                    target_arch = "arm"
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

        let resolved_uri = path_to_file_uri(&real_path);

        // Post-resolve check catches symlinks pointing to disallowed or outside-rootDir paths.
        if let Err(msg) = self.policy.check_allowlist_only(&resolved_uri, false) {
            return Err(ModuleHostError::SecurityManagerException { message: Some(msg) });
        }
        if let Err(msg) =
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
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)
            .map_err(|err| ModuleHostError::Io(IoFailure::from(err.to_string())))?;
        Ok(bytes)
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
