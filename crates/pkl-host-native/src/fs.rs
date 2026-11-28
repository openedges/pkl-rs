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

/// Strips a Windows extended-length verbatim drive prefix (`\\?\C:\...`) when present,
/// returning the regular drive path (`C:\...`). Leaves UNC verbatim paths (`\\?\UNC\...`)
/// and non-Windows paths untouched.
pub fn strip_windows_verbatim_prefix(path: &Path) -> PathBuf {
    let raw = path.to_string_lossy();
    if let Some(rest) = raw.strip_prefix(r"\\?\") {
        let b = rest.as_bytes();
        if b.len() >= 3
            && b[0].is_ascii_alphabetic()
            && b[1] == b':'
            && (b[2] == b'\\' || b[2] == b'/')
        {
            return PathBuf::from(rest);
        }
    }
    path.to_path_buf()
}

/// Joins a slash-separated relative path (`rel`) onto `base` component-by-component,
/// avoiding Windows `\\?\` verbatim-path pitfalls where `/` and `..` inside a joined
/// string would otherwise be treated as literal filename characters.
pub fn join_relative_path_segments(base: &Path, rel: &str) -> PathBuf {
    let mut out = strip_windows_verbatim_prefix(base);
    for comp in Path::new(rel).components() {
        match comp {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => out.push(".."),
            std::path::Component::Normal(seg) => out.push(seg),
            std::path::Component::RootDir | std::path::Component::Prefix(_) => out.push(comp),
        }
    }
    out
}

/// Returns true if `meta` is a symlink or (on Windows) an NTFS directory junction /
/// mount-point reparse point whose link target can be read.
pub fn is_symlink_or_junction(path: &Path, meta: &fs::Metadata) -> bool {
    if meta.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if (meta.file_attributes() & 0x0400) != 0 && fs::read_link(path).is_ok() {
            return true;
        }
    }
    #[cfg(not(windows))]
    let _ = path;
    false
}

/// Normalizes a `rootDir` path matching `SecurityManagers.Standard.normalizePath`
/// (`toRealPath` if existing, else deepest existing ancestor canonicalized + normalized tail).
pub fn canonicalize_root_dir(path: &Path) -> std::io::Result<PathBuf> {
    if path.exists() {
        fs::canonicalize(path)
    } else if path.is_absolute() {
        Ok(resolve_canonical_or_ancestor(path))
    } else {
        let abs = std::env::current_dir()?.join(path);
        Ok(resolve_canonical_or_ancestor(&abs))
    }
}

/// Returns `true` iff `s` begins with an RFC 3986 URI scheme and is NOT a Windows
/// single-letter drive path (`C:\...`, `C:/...`, or `C:...` on Windows).
pub fn is_uri_like(s: &str) -> bool {
    let Some((scheme, rest)) = s.split_once(':') else {
        return false;
    };
    if scheme.len() == 1 && (rest.starts_with('\\') || rest.starts_with('/') || cfg!(windows)) {
        return false;
    }
    let mut chars = scheme.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    first.is_ascii_alphabetic()
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
}

/// Detects the current native host's OS, OS version, and CPU architecture once per process
/// for `pkl:platform` and `pkl:release` (`java_version = "native"`).
pub fn detect_native_platform_metadata() -> pkl_runtime::logging::PlatformMetadata {
    static CACHED: std::sync::OnceLock<pkl_runtime::logging::PlatformMetadata> =
        std::sync::OnceLock::new();
    CACHED
        .get_or_init(|| {
            let os_name = match std::env::consts::OS {
                "macos" => "macOS",
                "linux" => "Linux",
                "windows" => "Windows",
                "freebsd" => "FreeBSD",
                other => other,
            };
            let cpu_architecture = match std::env::consts::ARCH {
                "x86_64" => "amd64",
                "aarch64" => "aarch64",
                other => other,
            };
            let os_version = detect_native_os_version();
            pkl_runtime::logging::PlatformMetadata {
                os_name: os_name.into(),
                os_version: os_version.into(),
                cpu_architecture: cpu_architecture.into(),
                java_version: "native".into(),
            }
        })
        .clone()
}

fn detect_native_os_version() -> String {
    #[cfg(target_os = "macos")]
    {
        if let Ok(plist) = fs::read_to_string("/System/Library/CoreServices/SystemVersion.plist")
            && let Some(needle_idx) = plist.find("<key>ProductVersion</key>")
        {
            let rest = &plist[needle_idx + "<key>ProductVersion</key>".len()..];
            if let Some(s_open) = rest.find("<string>") {
                let after = &rest[s_open + "<string>".len()..];
                if let Some(s_close) = after.find("</string>") {
                    let ver = after[..s_close].trim();
                    if !ver.is_empty() {
                        return ver.to_string();
                    }
                }
            }
        }
        if let Ok(out) = std::process::Command::new("sw_vers")
            .arg("-productVersion")
            .output()
            && out.status.success()
        {
            let ver = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !ver.is_empty() {
                return ver;
            }
        }
        String::new()
    }
    #[cfg(target_os = "linux")]
    {
        if let Ok(rel) = fs::read_to_string("/proc/sys/kernel/osrelease") {
            let ver = rel.trim().to_string();
            if !ver.is_empty() {
                return ver;
            }
        }
        String::new()
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        String::new()
    }
}

/// Converts a filesystem path into a canonical `file:///...` URI using Pkl's `IoUtils.toUri` quoting.
///
/// Strips Windows extended-length verbatim drive prefixes (`\\?\C:\...`) produced by
/// `std::fs::canonicalize` so canonical Windows paths yield `file:///C:/...` instead of
/// `file:/%3F/C:/...`.
pub fn path_to_file_uri(path: &Path) -> PklString {
    let lossy = path.to_string_lossy();
    let is_win_prefix = cfg!(windows)
        || lossy.starts_with(r"\\?\")
        || lossy.starts_with(r"\\.\")
        || lossy.starts_with(r"\\")
        || {
            let b = lossy.as_bytes();
            b.len() >= 3
                && b[0].is_ascii_alphabetic()
                && b[1] == b':'
                && (b[2] == b'\\' || b[2] == b'/')
        };
    let mut raw = if is_win_prefix {
        lossy.replace('\\', "/")
    } else {
        lossy.into_owned()
    };
    if let Some(rest) = raw.strip_prefix("//?/") {
        let b = rest.as_bytes();
        if b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':' {
            raw = rest.to_string();
        }
    }
    let with_slash = if raw.starts_with('/') {
        raw
    } else {
        format!("/{raw}")
    };
    PklString::from("file://").concat(&quote_uri_path(&with_slash.into()))
}

/// Converts a `file:` URI into a local filesystem `PathBuf`, applying NFC normalization
/// matching JDK 21's Unix path encoding and stripping Windows URI leading slashes (`/C:/...`).
pub fn file_uri_to_path(uri: &PklString) -> Result<PathBuf, PklString> {
    if let Some(s) = uri.as_str()
        && s.starts_with("file:///")
        && s.is_ascii()
        && !s.bytes().any(|b| matches!(b, b'%' | b'?' | b'#' | 0))
    {
        let raw_path = &s[7..];
        if is_windows_unc_or_drive_relative(raw_path) {
            return Err(PklString::from(format!(
                "Unsupported remote authority or drive-relative path in file URI `{uri}`."
            )));
        }
        return Ok(strip_windows_uri_leading_slash(PathBuf::from(raw_path)));
    }
    let info = extract_file_uri_for_root_dir(uri)
        .filter(|i| !i.from_jar)
        .ok_or_else(|| PklString::from(format!("Expected `file:` URI, got `{uri}`.")))?;
    if info.has_non_empty_authority || is_windows_unc_or_drive_relative(&info.decoded_path) {
        return Err(PklString::from(format!(
            "Unsupported remote authority in file URI `{uri}`."
        )));
    }
    if info.decoded_path.contains('\0') {
        return Err(PklString::from(format!(
            "Invalid NUL byte in file URI `{uri}`."
        )));
    }
    if info.decoded_path.is_ascii() {
        return Ok(strip_windows_uri_leading_slash(PathBuf::from(
            info.decoded_path,
        )));
    }
    let normalized = info.decoded_path.nfc().collect::<String>();
    Ok(strip_windows_uri_leading_slash(PathBuf::from(normalized)))
}

#[cfg(windows)]
fn is_windows_unc_or_drive_relative(decoded_path: &str) -> bool {
    // Reject UNC / Win32 device namespace paths (`//server`, `\\server`, `/\server`, `\/server`,
    // `\\.\pipe\...`, `\\?\UNC\...`) while allowing canonical verbatim drive roots (`//?/C:/...`).
    if decoded_path.starts_with("//")
        || decoded_path.starts_with("\\\\")
        || decoded_path.starts_with("/\\")
        || decoded_path.starts_with("\\/")
    {
        let rest = &decoded_path[2..];
        let is_verbatim_drive = rest
            .strip_prefix("?/")
            .or_else(|| rest.strip_prefix("?\\"))
            .is_some_and(|r| {
                let b = r.as_bytes();
                b.len() >= 3
                    && b[0].is_ascii_alphabetic()
                    && b[1] == b':'
                    && (b[2] == b'/' || b[2] == b'\\')
            });
        if !is_verbatim_drive {
            return true;
        }
    }
    let b = decoded_path.as_bytes();
    // Reject `/C:` (bare drive) and `/C:relative` (drive-relative with leading slash).
    if b.len() >= 3
        && b[0] == b'/'
        && b[1].is_ascii_alphabetic()
        && b[2] == b':'
        && (b.len() == 3 || (b[3] != b'/' && b[3] != b'\\'))
    {
        return true;
    }
    // Reject `C:` (bare drive) and `C:relative` (opaque drive-relative without leading slash).
    if b.len() >= 2
        && b[0].is_ascii_alphabetic()
        && b[1] == b':'
        && (b.len() == 2 || (b[2] != b'/' && b[2] != b'\\'))
    {
        return true;
    }
    false
}

#[cfg(not(windows))]
fn is_windows_unc_or_drive_relative(_decoded_path: &str) -> bool {
    false
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
pub(crate) fn strip_windows_uri_leading_slash(path: PathBuf) -> PathBuf {
    let s = path.to_string_lossy();
    let b = s.as_bytes();
    if b.len() >= 3
        && b[0] == b'/'
        && b[1].is_ascii_alphabetic()
        && b[2] == b':'
        && (b.len() == 3 || b[3] == b'/' || b[3] == b'\\')
    {
        PathBuf::from(&s[1..])
    } else {
        path
    }
}

#[cfg(not(windows))]
pub(crate) fn strip_windows_uri_leading_slash(path: PathBuf) -> PathBuf {
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
        || import_uri.starts_with("...")
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
        let base_path = normalize_lexical_buf(&strip_windows_uri_leading_slash(
            file_uri_to_path(&base_uri.into()).ok()?,
        ));
        // Fall back if `base_uri` uses a different encoding than `path_to_file_uri`
        // (e.g. percent-encoded non-ASCII ancestor dirs from `file_uri_from_path`).
        if path_to_file_uri(&base_path).to_java_utf8() != base_uri {
            return None;
        }
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

/// Resolves a path to its canonical form **without** collapsing `..` across symlinks first.
///
/// Mirrors OS `realpath` / Java `Path.toRealPath()` semantics even for non-existent tails
/// and dangling symlinks:
/// 1. Tries `fs::canonicalize(path)` on the raw path directly so the OS resolves symlinks
///    before `..`.
/// 2. Fast path (when no `..` component is present): pops trailing missing normal segments
///    right-to-left until an existing ancestor canonicalizes in 1 syscall, verifying the
///    first missing segment is not a dangling symlink via `symlink_metadata`.
/// 3. Fallback: walks components left-to-right, resolving every existing prefix and following
///    dangling symlinks via `read_link` (bounded to 40 hops) before applying any `..`.
pub fn resolve_canonical_or_ancestor(path: &Path) -> PathBuf {
    try_resolve_canonical_or_ancestor(path)
        .unwrap_or_else(|_| PathBuf::from(normalize_lexical_path(&path.to_string_lossy())))
}

/// Like [`resolve_canonical_or_ancestor`], but returns `Err(())` (fail-closed) when
/// dangling/chained symlink expansion exceeds `MAX_SYMLINK_HOPS` (40).
#[allow(clippy::result_unit_err)]
pub fn try_resolve_canonical_or_ancestor(path: &Path) -> Result<PathBuf, ()> {
    if let Ok(real) = fs::canonicalize(path) {
        return Ok(real);
    }
    // Fast right-to-left ancestor resolution when there are no `..` components: avoids
    // O(depth) `realpath` syscalls for not-yet-created output paths under existing dirs.
    let has_parent_dir = path
        .components()
        .any(|c| matches!(c, std::path::Component::ParentDir));
    if !has_parent_dir {
        let mut missing_tail: Vec<&std::ffi::OsStr> = Vec::new();
        let mut cur = path;
        while let Some(name) = cur.file_name() {
            let Some(parent) = cur.parent() else {
                break;
            };
            if parent.as_os_str().is_empty() {
                break;
            }
            missing_tail.push(name);
            if let Ok(real_parent) = fs::canonicalize(parent) {
                let first_missing = missing_tail[missing_tail.len() - 1];
                let probe = real_parent.join(first_missing);
                match fs::symlink_metadata(&probe) {
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                        let mut out = real_parent;
                        for seg in missing_tail.into_iter().rev() {
                            out.push(seg);
                        }
                        return Ok(out);
                    }
                    _ => break, // Dangling symlink or race -> full symlink-aware walk.
                }
            }
            cur = parent;
        }
    }
    resolve_components_symlink_aware(path, 0)
}

fn resolve_components_symlink_aware(path: &Path, depth: usize) -> Result<PathBuf, ()> {
    const MAX_SYMLINK_HOPS: usize = 40;
    let mut cur = PathBuf::new();
    let mut pending: Vec<std::path::Component<'_>> = path.components().collect();
    pending.reverse();
    let mut owned_hops: usize = depth;

    while let Some(comp) = pending.pop() {
        match comp {
            std::path::Component::Prefix(p) => cur.push(p.as_os_str()),
            std::path::Component::RootDir => cur.push(comp.as_os_str()),
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                if let Ok(real) = fs::canonicalize(&cur) {
                    cur = real;
                }
                if !cur.pop() {
                    if !cur.has_root() {
                        cur.push(comp.as_os_str());
                    }
                } else if let Ok(real) = fs::canonicalize(&cur) {
                    cur = real;
                }
            }
            std::path::Component::Normal(seg) => {
                cur.push(seg);
                if let Ok(real) = fs::canonicalize(&cur) {
                    cur = real;
                } else if let Ok(meta) = fs::symlink_metadata(&cur)
                    && is_symlink_or_junction(&cur, &meta)
                {
                    if owned_hops >= MAX_SYMLINK_HOPS {
                        return Err(());
                    }
                    if let Ok(target) = fs::read_link(&cur) {
                        owned_hops += 1;
                        let base = cur.parent().unwrap_or(Path::new("/")).to_path_buf();
                        let next = if target.is_absolute() {
                            target
                        } else {
                            base.join(target)
                        };
                        cur = resolve_components_symlink_aware(&next, owned_hops)?;
                    } else {
                        return Err(());
                    }
                }
            }
        }
    }

    if let Ok(real) = fs::canonicalize(&cur) {
        return Ok(real);
    }
    if cur.as_os_str().is_empty() {
        Ok(PathBuf::from(normalize_lexical_path(
            &path.to_string_lossy(),
        )))
    } else {
        Ok(cur)
    }
}

// Exact OS-specific O_NOFOLLOW constants (zero `unsafe`, zero `libc` dependency).
// NOTE: Linux kernel UAPI defines O_NOFOLLOW in OCTAL:
// - asm-generic (x86/x86_64/riscv64/s390x/loongarch64): 00400000 = 0x20000
// - arm / aarch64 / powerpc:                            00100000 = 0x8000
//   (0x10000 on arm/aarch64 is O_DIRECT, which causes EINVAL on unaligned reads!)
#[cfg(all(
    unix,
    any(
        target_os = "macos",
        target_os = "ios",
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "netbsd",
        target_os = "dragonfly"
    )
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
#[cfg(all(
    unix,
    not(any(
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
    ))
))]
const O_NOFOLLOW: i32 = 0;

/// Opens a verified real path without following symlinks (`O_NOFOLLOW` on Unix)
/// to defend against TOCTOU symlink swap attacks when `rootDir` is enforced.
pub fn open_file_nofollow(path: &Path) -> std::io::Result<File> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        let mut opts = OpenOptions::new();
        opts.read(true);
        if O_NOFOLLOW != 0 {
            opts.custom_flags(O_NOFOLLOW);
        }
        opts.open(path)
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::{MetadataExt, OpenOptionsExt};
        const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;
        let file = OpenOptions::new()
            .read(true)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
            .open(path)?;
        if file.metadata()?.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
            && fs::read_link(path).is_ok()
        {
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "Refusing to follow symlink during secure file open",
            ));
        }
        Ok(file)
    }
    #[cfg(not(any(unix, windows)))]
    {
        let meta = fs::symlink_metadata(path)?;
        if is_symlink_or_junction(path, &meta) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "Refusing to follow symlink during secure file open",
            ));
        }
        OpenOptions::new().read(true).open(path)
    }
}

/// Writes bytes to a verified real path without following symlinks (`O_NOFOLLOW` on Unix)
/// to defend against dangling-symlink and TOCTOU symlink-swap output writes.
pub fn write_file_nofollow(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        let mut opts = OpenOptions::new();
        opts.write(true).create(true).truncate(true);
        if O_NOFOLLOW != 0 {
            opts.custom_flags(O_NOFOLLOW);
        }
        let mut file = opts.open(path)?;
        file.write_all(bytes)
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::{MetadataExt, OpenOptionsExt};
        const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
            .open(path)?;
        if file.metadata()?.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
            && fs::read_link(path).is_ok()
        {
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "Refusing to follow symlink during secure file write",
            ));
        }
        file.set_len(0)?;
        file.write_all(bytes)
    }
    #[cfg(not(any(unix, windows)))]
    {
        if let Ok(meta) = fs::symlink_metadata(path)
            && is_symlink_or_junction(path, &meta)
        {
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "Refusing to follow symlink during secure file write",
            ));
        }
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(path)?;
        file.write_all(bytes)
    }
}

/// Appends bytes to a verified real path without following symlinks (`O_NOFOLLOW` on Unix).
pub fn append_file_nofollow(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        let mut opts = OpenOptions::new();
        opts.write(true).append(true);
        if O_NOFOLLOW != 0 {
            opts.custom_flags(O_NOFOLLOW);
        }
        let mut file = opts.open(path)?;
        file.write_all(bytes)
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::{MetadataExt, OpenOptionsExt};
        const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;
        let mut file = OpenOptions::new()
            .write(true)
            .append(true)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
            .open(path)?;
        if file.metadata()?.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
            && fs::read_link(path).is_ok()
        {
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "Refusing to follow symlink during secure file append",
            ));
        }
        file.write_all(bytes)
    }
    #[cfg(not(any(unix, windows)))]
    {
        let meta = fs::symlink_metadata(path)?;
        if is_symlink_or_junction(path, &meta) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "Refusing to follow symlink during secure file append",
            ));
        }
        let mut file = OpenOptions::new().write(true).append(true).open(path)?;
        file.write_all(bytes)
    }
}

const MAX_LOCAL_FILE_READ_BYTES: usize = 256 * 1024 * 1024;
const MAX_CACHED_CANONICAL_PARENTS: usize = 64;

/// Reads an open file up to the 256 MiB local file safety cap.
pub fn read_file_bounded(file: &mut File) -> std::io::Result<Vec<u8>> {
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
                    let mut chunk = [0u8; 65536];
                    loop {
                        match file.read(&mut chunk) {
                            Ok(0) => return Ok(bytes),
                            Ok(m) => {
                                if bytes.len().saturating_add(m) > MAX_LOCAL_FILE_READ_BYTES {
                                    return Err(std::io::Error::new(
                                        std::io::ErrorKind::InvalidData,
                                        format!(
                                            "File exceeds maximum allowed read size of {MAX_LOCAL_FILE_READ_BYTES} bytes"
                                        ),
                                    ));
                                }
                                bytes.try_reserve(m).map_err(|_| {
                                    std::io::Error::new(
                                        std::io::ErrorKind::OutOfMemory,
                                        "Failed to allocate buffer while reading file",
                                    )
                                })?;
                                bytes.extend_from_slice(&chunk[..m]);
                            }
                            Err(err) if err.kind() == std::io::ErrorKind::Interrupted => continue,
                            Err(err) => return Err(err),
                        }
                    }
                }
            }
            Err(err) if err.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(err) => return Err(err),
        }
    }
}

/// Opens and reads `path` with the 256 MiB local file safety cap.
pub fn read_path_bounded(path: &Path) -> std::io::Result<Vec<u8>> {
    let mut file = File::open(path)?;
    read_file_bounded(&mut file)
}

/// Opens and reads `path` as UTF-8 with the 256 MiB local file safety cap.
pub fn read_path_to_string_bounded(path: &Path) -> std::io::Result<String> {
    let bytes = read_path_bounded(path)?;
    String::from_utf8(bytes)
        .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err))
}

/// Opens and reads `path` without following symlinks (`O_NOFOLLOW`) up to the 256 MiB cap.
pub fn read_path_nofollow_bounded(path: &Path) -> std::io::Result<Vec<u8>> {
    let mut file = open_file_nofollow(path)?;
    read_file_bounded(&mut file)
}

/// Secure native filesystem adapter bound to a `SecurityPolicy`.
#[derive(Clone, Debug)]
pub struct NativeFs {
    policy: SecurityPolicy,
    /// When `true` (e.g., `rootDir` configured alongside a custom `SecurityManager`),
    /// preserve `O_NOFOLLOW` TOCTOU defense on module/resource opens even when `policy`
    /// delegates allowlist/root checks to the custom security checker.
    enforce_nofollow: bool,
    /// Per-evaluator cache of verified canonical parent directories (used strictly when
    /// `policy.root_dir().is_none()` so sibling imports avoid redundant `realpath` syscalls).
    canonical_parents: std::sync::Arc<std::sync::Mutex<Vec<PathBuf>>>,
}

impl NativeFs {
    pub fn new(policy: SecurityPolicy) -> Self {
        let enforce_nofollow = policy.root_dir().is_some();
        Self {
            policy,
            enforce_nofollow,
            canonical_parents: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
        }
    }

    pub fn with_enforce_nofollow(mut self, enforce: bool) -> Self {
        self.enforce_nofollow = enforce || self.policy.root_dir().is_some();
        self
    }

    pub fn policy(&self) -> &SecurityPolicy {
        &self.policy
    }

    /// Checks allowlist + real-path `rootDir` (including `jar:file:` inner archive path,
    /// UNC fast-reject, symlink resolution, and non-existent lexical/ancestor check).
    pub fn check_uri(&self, uri: &PklString, is_resource: bool) -> Result<(), PklString> {
        self.check_uri_and_resolve(uri, is_resource).map(|_| ())
    }

    /// Same as [`Self::check_uri`], returning the verified canonicalized local filesystem
    /// path when `rootDir` is enforced for a `file:` URI so callers never re-resolve a raw
    /// uncanonicalized path across symlinks.
    pub(crate) fn check_uri_and_resolve(
        &self,
        uri: &PklString,
        is_resource: bool,
    ) -> Result<Option<PathBuf>, PklString> {
        self.policy.check_allowlist_only(uri, is_resource)?;
        if self.policy.root_dir().is_none() {
            return Ok(None);
        }
        let Some(info) = extract_file_uri_for_root_dir(uri) else {
            return Ok(None);
        };
        if info.has_non_empty_authority || is_windows_unc_or_drive_relative(&info.decoded_path) {
            // UNC / remote host authority or Windows drive-relative path: reject immediately
            // before any FS/network call.
            return Err(pkl_runtime::host::security::root_dir_denied_message(
                uri,
                is_resource,
            ));
        }
        if info.decoded_path.contains('\0') {
            return Err(pkl_runtime::host::security::root_dir_denied_message(
                uri,
                is_resource,
            ));
        }
        let candidate = strip_windows_uri_leading_slash(PathBuf::from(
            info.decoded_path.nfc().collect::<String>(),
        ));
        let Ok(resolved) = try_resolve_canonical_or_ancestor(&candidate) else {
            return Err(pkl_runtime::host::security::root_dir_denied_message(
                uri,
                is_resource,
            ));
        };
        self.policy.check_real_path_under_root_dir(
            uri,
            &resolved.to_string_lossy(),
            is_resource,
        )?;
        Ok((!info.from_jar).then_some(resolved))
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
        if is_windows_unc_or_drive_relative(&info.decoded_path) || info.decoded_path.contains('\0')
        {
            return Err(Ok(pkl_runtime::host::security::root_dir_denied_message(
                uri,
                is_resource,
            )));
        }
        let path = strip_windows_uri_leading_slash(PathBuf::from(
            info.decoded_path.nfc().collect::<String>(),
        ));
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

        if cfg!(windows) && path.to_string_lossy().contains('\\') {
            return Err(ModuleHostError::NotFound);
        }

        let allow_canonical_parent_fast_path = cfg!(target_os = "linux")
            && self.policy.root_dir().is_none()
            && path.to_str().is_some_and(|s| !s.contains("//"))
            && !path.components().any(|c| {
                matches!(
                    c,
                    std::path::Component::ParentDir | std::path::Component::CurDir
                )
            });

        let real_path = if allow_canonical_parent_fast_path
            && let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
            && path.file_name().is_some()
            && self
                .canonical_parents
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .iter()
                .any(|p| p.as_os_str() == parent.as_os_str())
        {
            match fs::symlink_metadata(&path) {
                Ok(meta) if !is_symlink_or_junction(&path, &meta) => path.clone(),
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                    return Err(ModuleHostError::NotFound);
                }
                _ => match fs::canonicalize(&path) {
                    Ok(real) => real,
                    Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                        return Err(ModuleHostError::NotFound);
                    }
                    Err(err) => return Err(ModuleHostError::Io(IoFailure::from(err.to_string()))),
                },
            }
        } else {
            let real = match fs::canonicalize(&path) {
                Ok(real) => real,
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                    return Err(ModuleHostError::NotFound);
                }
                Err(err) => return Err(ModuleHostError::Io(IoFailure::from(err.to_string()))),
            };
            if allow_canonical_parent_fast_path
                && real.as_os_str() == path.as_os_str()
                && let Some(parent) = path.parent()
                && !parent.as_os_str().is_empty()
            {
                let mut parents = self
                    .canonical_parents
                    .lock()
                    .unwrap_or_else(|e| e.into_inner());
                if parents.len() < MAX_CACHED_CANONICAL_PARENTS
                    && !parents.iter().any(|p| p.as_os_str() == parent.as_os_str())
                {
                    parents.push(parent.to_path_buf());
                }
            }
            real
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
        let mut file = if self.enforce_nofollow || self.policy.root_dir().is_some() {
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
        read_file_bounded(&mut file)
            .map_err(|err| ModuleHostError::Io(IoFailure::from(err.to_string())))
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

        let nofollow = self.enforce_nofollow || self.policy.root_dir().is_some();
        self.read_file_resource_at_path(&target_path, uri, nofollow)
    }

    /// Reads a `file:` resource directly from a pre-canonicalized path verified by
    /// `CustomSecurityChecker`, always using `symlink_metadata` + `O_NOFOLLOW` to prevent
    /// TOCTOU symlink swaps between the security check and the filesystem open.
    pub fn read_file_resource_from_verified_path(
        &self,
        target_path: &Path,
        uri: &PklString,
    ) -> Result<Option<Vec<u8>>, ReadFailure> {
        if let Err(reason) = validate_file_uri_has_leading_slash(uri) {
            return Err(ReadFailure::UriSyntax { reason });
        }
        self.read_file_resource_at_path(target_path, uri, true)
    }

    fn read_file_resource_at_path(
        &self,
        target_path: &Path,
        uri: &PklString,
        nofollow: bool,
    ) -> Result<Option<Vec<u8>>, ReadFailure> {
        let meta = if nofollow {
            // `target_path` was already canonicalized; use `symlink_metadata` so a TOCTOU swap
            // to a symlink is rejected before `read_dir` or `open`.
            let m = match fs::symlink_metadata(target_path) {
                Ok(m) => m,
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
                Err(err) => return Err(ReadFailure::Io(IoFailure::from(err.to_string()))),
            };
            if is_symlink_or_junction(target_path, &m) {
                return Err(SecurityPolicy::wrap_file_resource_security_error(
                    pkl_runtime::host::security::root_dir_denied_message(uri, true),
                ));
            }
            m
        } else {
            match fs::metadata(target_path) {
                Ok(m) => m,
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
                Err(err) => return Err(ReadFailure::Io(IoFailure::from(err.to_string()))),
            }
        };

        if meta.is_dir() {
            // JDK FileURLConnection lists directory entries (including symlinks), sorted.
            let mut names = Vec::new();
            let dir = fs::read_dir(target_path)
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

        let mut file = match if nofollow {
            open_file_nofollow(target_path)
        } else {
            File::open(target_path)
        } {
            Ok(f) => f,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(err) => return Err(ReadFailure::Io(IoFailure::from(err.to_string()))),
        };
        let bytes = read_file_bounded(&mut file)
            .map_err(|err| ReadFailure::Io(IoFailure::from(err.to_string())))?;
        Ok(Some(bytes))
    }

    /// `FileResolver.hasElement`: checks security (including `rootDir` symlink check) first,
    /// then tests existence on the verified canonical path (when `rootDir` is set).
    pub fn has_element(&self, uri: &PklString, is_resource: bool) -> Result<bool, ModuleHostError> {
        let verified = self
            .check_uri_and_resolve(uri, is_resource)
            .map_err(|msg| ModuleHostError::SecurityManagerException { message: Some(msg) })?;
        let path = match verified {
            Some(real) => real,
            None => file_uri_to_path(uri)
                .map_err(|msg| ModuleHostError::SecurityManagerException { message: Some(msg) })?,
        };
        Ok(path.exists())
    }

    /// `FileResolver.listElements`: checks security (including `rootDir` symlink check on base dir),
    /// skips symlink entries, returns empty list for missing/non-dir.
    pub fn list_elements(
        &self,
        uri: &PklString,
        is_resource: bool,
    ) -> Result<Vec<PathElement>, ModuleHostError> {
        let verified = self
            .check_uri_and_resolve(uri, is_resource)
            .map_err(|msg| ModuleHostError::SecurityManagerException { message: Some(msg) })?;
        let path = match verified {
            Some(real) => real,
            None => file_uri_to_path(uri)
                .map_err(|msg| ModuleHostError::SecurityManagerException { message: Some(msg) })?,
        };
        let nofollow = self.enforce_nofollow || self.policy.root_dir().is_some();
        self.list_elements_at_path(&path, uri, is_resource, nofollow)
    }

    /// Lists directory elements directly from a pre-canonicalized path verified by
    /// `CustomSecurityChecker`, rejecting symlinks/junctions via `symlink_metadata`.
    pub fn list_elements_from_verified_path(
        &self,
        path: &Path,
        uri: &PklString,
        is_resource: bool,
    ) -> Result<Vec<PathElement>, ModuleHostError> {
        self.list_elements_at_path(path, uri, is_resource, true)
    }

    fn list_elements_at_path(
        &self,
        path: &Path,
        uri: &PklString,
        is_resource: bool,
        nofollow: bool,
    ) -> Result<Vec<PathElement>, ModuleHostError> {
        let meta = if nofollow {
            let Ok(m) = fs::symlink_metadata(path) else {
                return Ok(Vec::new());
            };
            if is_symlink_or_junction(path, &m) {
                return Err(ModuleHostError::SecurityManagerException {
                    message: Some(pkl_runtime::host::security::root_dir_denied_message(
                        uri,
                        is_resource,
                    )),
                });
            }
            m
        } else {
            let Ok(m) = fs::metadata(path) else {
                return Ok(Vec::new());
            };
            m
        };
        if !meta.is_dir() {
            return Ok(Vec::new());
        }
        let read_dir = match fs::read_dir(path) {
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
            // FileResolver.listElements explicitly skips symlinks (and Windows NTFS junctions).
            if file_type.is_symlink() {
                continue;
            }
            #[cfg(windows)]
            {
                if let Ok(entry_meta) = fs::symlink_metadata(entry.path())
                    && is_symlink_or_junction(&entry.path(), &entry_meta)
                {
                    continue;
                }
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
        let triple_foo = root.join("...foo");
        let non_ascii_root = dir.join("테스트").join("proj");
        let non_ascii_dep = dir.join("테스트").join("dep");
        fs::create_dir_all(&root).unwrap();
        fs::create_dir_all(&dep).unwrap();
        fs::create_dir_all(&triple_foo).unwrap();
        fs::create_dir_all(&non_ascii_root).unwrap();
        fs::create_dir_all(&non_ascii_dep).unwrap();

        let dep_file = dep.join("PklProject");
        fs::write(&dep_file, "amends \"pkl:Project\"\n").unwrap();
        #[cfg(not(windows))]
        {
            let triple = root.join("...");
            fs::create_dir_all(&triple).unwrap();
            fs::write(triple.join("PklProject"), "amends \"pkl:Project\"\n").unwrap();
        }
        fs::write(triple_foo.join("PklProject"), "amends \"pkl:Project\"\n").unwrap();
        fs::write(non_ascii_dep.join("PklProject"), "amends \"pkl:Project\"\n").unwrap();

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

        // Triple-dot (`.../` and `...<non-slash>`), backslash, absolute POSIX path, query/fragment,
        // percent-encoded relative, and non-ASCII percent-encoded base URI must fall back.
        assert!(read_static_local_project_import(&base_uri, ".../PklProject").is_none());
        assert!(read_static_local_project_import(&base_uri, "...foo/PklProject").is_none());
        assert!(read_static_local_project_import(&base_uri, "..\\dep\\PklProject").is_none());
        assert!(read_static_local_project_import(&base_uri, "/etc/PklProject").is_none());
        assert!(read_static_local_project_import(&base_uri, "../dep/Pkl%50roject").is_none());
        assert!(read_static_local_project_import(&base_uri, &format!("{dep_uri}?q=1")).is_none());
        assert!(read_static_local_project_import(&base_uri, &format!("{dep_uri}#frag")).is_none());

        let non_ascii_base_uri =
            pkl_runtime::packages::resolver::file_uri_from_path(&non_ascii_root.join("PklProject"))
                .unwrap();
        assert!(
            read_static_local_project_import(&non_ascii_base_uri, "../dep/PklProject").is_none()
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn path_to_file_uri_strips_windows_verbatim_drive_prefix() {
        assert_eq!(
            path_to_file_uri(std::path::Path::new(
                r"\\?\C:\Users\runneradmin\AppData\Local\Temp\proj\PklProject"
            ))
            .to_java_utf8(),
            "file:///C:/Users/runneradmin/AppData/Local/Temp/proj/PklProject"
        );
        assert_eq!(
            path_to_file_uri(std::path::Path::new(r"C:\Users\runneradmin\main.pkl")).to_java_utf8(),
            "file:///C:/Users/runneradmin/main.pkl"
        );
        assert_eq!(
            path_to_file_uri(std::path::Path::new("/tmp/proj/PklProject")).to_java_utf8(),
            "file:///tmp/proj/PklProject"
        );
    }

    #[cfg(unix)]
    #[test]
    fn root_dir_symlink_before_parent_dir_cannot_escape() {
        use super::{NativeFs, canonicalize_root_dir, write_file_nofollow};
        use pkl_runtime::host::security::SecurityPolicy;

        let dir = std::env::temp_dir().join(format!(
            "pkl-host-symlink-dotdot-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        let root = dir.join("root");
        let sub = root.join("sub");
        let outside = dir.join("outside");
        let outside_nested = outside.join("deep");
        fs::create_dir_all(&sub).unwrap();
        fs::create_dir_all(&outside_nested).unwrap();
        let secret = outside.join("secret.pkl");
        fs::write(&secret, "secret = \"top\"\n").unwrap();

        // `root/sub/link -> outside/deep`; `root/sub/link/../secret.pkl` resolves to
        // `outside/secret.pkl` (NOT `root/sub/secret.pkl`!).
        let link = sub.join("link");
        std::os::unix::fs::symlink(&outside_nested, &link).unwrap();

        let canon_root = canonicalize_root_dir(&root).unwrap();
        let policy = SecurityPolicy::default_standard()
            .with_root_dir(Some(canon_root.to_string_lossy().into_owned()));
        let native_fs = NativeFs::new(policy);

        let escape_uri = path_to_file_uri(&link.join("..").join("secret.pkl"));
        assert!(
            native_fs.check_uri(&escape_uri, false).is_err(),
            "symlink-before-`..` escape must be rejected by rootDir"
        );
        assert!(
            native_fs.has_element(&escape_uri, true).is_err(),
            "has_element must reject symlink-before-`..` escape"
        );

        // Dangling symlink pointing outside rootDir must also be rejected.
        let dangling = sub.join("dangling");
        std::os::unix::fs::symlink(outside.join("nonexistent.pkl"), &dangling).unwrap();
        let dangling_uri = path_to_file_uri(&dangling);
        assert!(
            native_fs.check_uri(&dangling_uri, false).is_err(),
            "dangling symlink outside rootDir must be rejected"
        );
        assert!(
            write_file_nofollow(&dangling, b"evil").is_err(),
            "write_file_nofollow must refuse to follow dangling symlink"
        );
        assert!(!outside.join("nonexistent.pkl").exists());

        let _ = fs::remove_dir_all(&dir);
    }
}
