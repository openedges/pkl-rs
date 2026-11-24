//! Configurable Wasm-pure SecurityManager policy matching Pkl 0.32.0 `SecurityManagers.java`.
//!
//! All regex allowlist matching (`Pattern.matcher(uri).lookingAt()`), trust-level
//! enforcement, `jar:file:` inner-URI unwrapping, UNC/authority rejection, and lexical
//! `rootDir` containment live here with zero OS I/O. Native adapters combine this with
//! canonical real-path (`toRealPath` / `O_NOFOLLOW`) verification.

use crate::host::errors::{IoFailure, ReadFailure};
use crate::host::resources::{decoded_scheme_specific_part, module_trust_level};
use crate::java_regex::{Limits, Utf16Regex};
use crate::modules::ModuleHostError;
use crate::value::PklString;
use std::sync::{Arc, OnceLock};

/// Default module URI allowlist (`SecurityManagers.defaultAllowedModules`).
pub const DEFAULT_ALLOWED_MODULES: &[&str] = &[
    "pkl:",
    "repl:",
    "file:",
    "modulepath:",
    "https:",
    "jar:file:",
    "jar:nested:",
    "package:",
    "projectpackage:",
];

/// Default resource URI allowlist (`SecurityManagers.defaultAllowedResources`).
pub const DEFAULT_ALLOWED_RESOURCES: &[&str] = &[
    "env:",
    "prop:",
    "file:",
    "modulepath:",
    "package:",
    "projectpackage:",
    "https:",
];

#[derive(Clone, Debug)]
struct CompiledPattern {
    raw: String,
    literal_prefix: Option<String>,
    regex: Arc<Utf16Regex>,
}

fn is_plain_literal_pattern(pattern: &str) -> bool {
    !pattern.bytes().any(|b| {
        matches!(
            b,
            b'\\'
                | b'.'
                | b'^'
                | b'$'
                | b'*'
                | b'+'
                | b'?'
                | b'('
                | b')'
                | b'['
                | b']'
                | b'{'
                | b'}'
                | b'|'
        )
    })
}

impl CompiledPattern {
    fn compile(pattern: &str) -> Result<Self, String> {
        let units: Vec<u16> = pattern.encode_utf16().collect();
        let regex = crate::regex::compile_utf16(&units, "").map_err(|err| {
            format!(
                "Invalid security allowlist regex `{pattern}`: {}",
                err.message
            )
        })?;
        let literal_prefix = is_plain_literal_pattern(pattern).then(|| pattern.to_string());
        Ok(Self {
            raw: pattern.to_string(),
            literal_prefix,
            regex: Arc::new(regex),
        })
    }

    fn looking_at(&self, input: &PklString) -> bool {
        if let Some(prefix) = &self.literal_prefix
            && let Some(text) = input.as_str()
        {
            return text.starts_with(prefix.as_str());
        }
        let units: Vec<u16> = input.encode_utf16().collect();
        self.regex
            .looking_at(&units, Limits::default())
            .is_ok_and(|matched| matched.is_some())
    }
}

fn default_module_patterns() -> Arc<[CompiledPattern]> {
    static CACHE: OnceLock<Arc<[CompiledPattern]>> = OnceLock::new();
    CACHE
        .get_or_init(|| {
            DEFAULT_ALLOWED_MODULES
                .iter()
                .map(|p| CompiledPattern::compile(p).expect("builtin module pattern"))
                .collect::<Vec<_>>()
                .into()
        })
        .clone()
}

fn default_resource_patterns() -> Arc<[CompiledPattern]> {
    static CACHE: OnceLock<Arc<[CompiledPattern]>> = OnceLock::new();
    CACHE
        .get_or_init(|| {
            DEFAULT_ALLOWED_RESOURCES
                .iter()
                .map(|p| CompiledPattern::compile(p).expect("builtin resource pattern"))
                .collect::<Vec<_>>()
                .into()
        })
        .clone()
}

/// Escapes regex metacharacters in a literal string (equivalent to Java `Pattern.quote`).
pub fn escape_regex_literal(literal: &str) -> String {
    let mut out = String::with_capacity(literal.len() + 4);
    for ch in literal.chars() {
        if matches!(
            ch,
            '\\' | '.' | '^' | '$' | '*' | '+' | '?' | '(' | ')' | '[' | ']' | '{' | '}' | '|'
        ) {
            out.push('\\');
        }
        out.push(ch);
    }
    out
}

/// Normalizes a slash-separated path lexically (resolving `.` and `..`) without FS I/O.
///
/// Also canonicalizes Windows verbatim (`\\?\C:\...`), URI-decoded (`/C:/...`), and
/// drive-letter (`C:\...`) forms into a single consistent `<DRIVE>:/...` form so `rootDir`
/// confinement matches identically across `fs::canonicalize`, `file:///C:/...`, and OS paths.
pub fn normalize_lexical_path(path: &str) -> String {
    let mut normalized_seps = path.replace('\\', "/");
    if let Some(rest) = normalized_seps.strip_prefix("//?/") {
        let b = rest.as_bytes();
        if b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':' {
            normalized_seps = rest.to_string();
        }
    }
    if let Some(rest) = normalized_seps.strip_prefix('/') {
        let b = rest.as_bytes();
        if b.len() >= 2
            && b[0].is_ascii_alphabetic()
            && b[1] == b':'
            && (b.len() == 2 || b[2] == b'/')
        {
            normalized_seps = rest.to_string();
        }
    }

    let bytes = normalized_seps.as_bytes();
    if bytes.len() >= 2
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && (bytes.len() == 2 || bytes[2] == b'/')
    {
        let drive = (bytes[0] as char).to_ascii_uppercase();
        let rest = &normalized_seps[2..];
        let mut stack: Vec<&str> = Vec::new();
        for seg in rest.split('/') {
            match seg {
                "" | "." => {}
                ".." => {
                    stack.pop();
                }
                other => stack.push(other),
            }
        }
        return if stack.is_empty() {
            format!("{drive}:/")
        } else {
            format!("{drive}:/{}", stack.join("/"))
        };
    }

    let is_abs = normalized_seps.starts_with('/');
    let mut stack: Vec<&str> = Vec::new();
    for seg in normalized_seps.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                if is_abs || stack.last().is_some_and(|&top| top != "..") {
                    stack.pop();
                } else {
                    stack.push("..");
                }
            }
            other => stack.push(other),
        }
    }
    if is_abs {
        if stack.is_empty() {
            "/".to_string()
        } else {
            format!("/{}", stack.join("/"))
        }
    } else if stack.is_empty() {
        ".".to_string()
    } else {
        stack.join("/")
    }
}

/// Returns true iff `candidate` is equal to `root` or strictly nested beneath `root`
/// along a path-component boundary (so `/tmp/root2` is NOT under `/tmp/root`).
pub fn is_path_under_root(candidate: &str, root: &str) -> bool {
    let candidate_norm = normalize_lexical_path(candidate);
    let root_norm = normalize_lexical_path(root);
    if root_norm == "/" {
        return candidate_norm.starts_with('/');
    }
    if root_norm.len() == 3 && root_norm.as_bytes()[1] == b':' && root_norm.ends_with('/') {
        return candidate_norm.starts_with(&root_norm);
    }
    if candidate_norm == root_norm {
        return true;
    }
    candidate_norm
        .strip_prefix(&root_norm)
        .is_some_and(|rest| rest.starts_with('/'))
}

/// Configurable security policy mirroring `org.pkl.core.SecurityManagers.Standard`.
#[derive(Clone, Debug)]
pub struct SecurityPolicy {
    allowed_modules: Arc<[CompiledPattern]>,
    allowed_resources: Arc<[CompiledPattern]>,
    root_dir: Option<String>,
    custom_allowed_modules: bool,
    custom_allowed_resources: bool,
}

impl Default for SecurityPolicy {
    fn default() -> Self {
        Self::default_standard()
    }
}

impl SecurityPolicy {
    /// Standard default policy (`SecurityManagers.defaultManager`).
    pub fn default_standard() -> Self {
        Self {
            allowed_modules: default_module_patterns(),
            allowed_resources: default_resource_patterns(),
            root_dir: None,
            custom_allowed_modules: false,
            custom_allowed_resources: false,
        }
    }

    /// Constructs a policy with optional custom allowlist patterns and optional normalized `root_dir`.
    pub fn new(
        allowed_modules: Option<&[String]>,
        allowed_resources: Option<&[String]>,
        root_dir: Option<String>,
    ) -> Result<Self, String> {
        let (mods, custom_mods) = match allowed_modules {
            Some(patterns) => (
                patterns
                    .iter()
                    .map(|p| CompiledPattern::compile(p))
                    .collect::<Result<Vec<_>, _>>()?
                    .into(),
                true,
            ),
            None => (default_module_patterns(), false),
        };
        let (res, custom_res) = match allowed_resources {
            Some(patterns) => (
                patterns
                    .iter()
                    .map(|p| CompiledPattern::compile(p))
                    .collect::<Result<Vec<_>, _>>()?
                    .into(),
                true,
            ),
            None => (default_resource_patterns(), false),
        };
        Ok(Self {
            allowed_modules: mods,
            allowed_resources: res,
            root_dir: root_dir.map(|dir| normalize_lexical_path(&dir)),
            custom_allowed_modules: custom_mods,
            custom_allowed_resources: custom_res,
        })
    }

    /// Sets or clears the normalized `rootDir`.
    pub fn with_root_dir(mut self, root_dir: Option<String>) -> Self {
        self.root_dir = root_dir.map(|dir| normalize_lexical_path(&dir));
        self
    }

    /// Appends external module reader schemes (`Pattern.quote("<scheme>:")`) when
    /// `allowed_modules` was NOT explicitly overridden (`CliCommand.kt:116-128`).
    pub fn with_external_module_schemes(mut self, schemes: &[String]) -> Result<Self, String> {
        if !self.custom_allowed_modules && !schemes.is_empty() {
            let mut mods = self.allowed_modules.to_vec();
            for scheme in schemes {
                let pattern = format!("{}:", escape_regex_literal(scheme));
                if !mods.iter().any(|p| p.raw == pattern) {
                    mods.push(CompiledPattern::compile(&pattern)?);
                }
            }
            self.allowed_modules = mods.into();
        }
        Ok(self)
    }

    /// Appends external resource reader schemes (`Pattern.quote("<scheme>:")`) when
    /// `allowed_resources` was NOT explicitly overridden (`CliCommand.kt:116-128`).
    pub fn with_external_resource_schemes(mut self, schemes: &[String]) -> Result<Self, String> {
        if !self.custom_allowed_resources && !schemes.is_empty() {
            let mut res = self.allowed_resources.to_vec();
            for scheme in schemes {
                let pattern = format!("{}:", escape_regex_literal(scheme));
                if !res.iter().any(|p| p.raw == pattern) {
                    res.push(CompiledPattern::compile(&pattern)?);
                }
            }
            self.allowed_resources = res.into();
        }
        Ok(self)
    }

    pub fn root_dir(&self) -> Option<&str> {
        self.root_dir.as_deref()
    }

    pub fn allowed_module_patterns(&self) -> Vec<&str> {
        self.allowed_modules
            .iter()
            .map(|p| p.raw.as_str())
            .collect()
    }

    pub fn allowed_resource_patterns(&self) -> Vec<&str> {
        self.allowed_resources
            .iter()
            .map(|p| p.raw.as_str())
            .collect()
    }

    /// Checks module import trust levels (`SecurityManagers.Standard.checkImportModule`).
    pub fn check_import_module(
        &self,
        importing_module: &PklString,
        imported_module: &PklString,
    ) -> Result<(), ModuleHostError> {
        let importing_level = module_trust_level(importing_module)?;
        let imported_level = module_trust_level(imported_module)?;
        if importing_level < imported_level {
            return Err(ModuleHostError::SecurityManagerException {
                message: Some(
                    PklString::from("Refusing to import module `")
                        .concat(imported_module)
                        .concat(&"` because importing module `".into())
                        .concat(importing_module)
                        .concat(&"` has an insufficient trust level.".into()),
                ),
            });
        }
        Ok(())
    }

    /// Checks whether `uri` matches the allowlist patterns (without checking `rootDir`).
    pub fn check_allowlist_only(
        &self,
        uri: &PklString,
        is_resource: bool,
    ) -> Result<(), PklString> {
        let patterns = if is_resource {
            &self.allowed_resources
        } else {
            &self.allowed_modules
        };
        if patterns.iter().any(|pattern| pattern.looking_at(uri)) {
            return Ok(());
        }
        Err(allowlist_denied_message(uri, is_resource))
    }

    /// Checks lexical `rootDir` containment for `file:` and `jar:file:` URIs without FS I/O.
    ///
    /// - Rejects `file://host/share` (non-empty authority / UNC) immediately before any FS/network call.
    /// - Unwraps `jar:` URIs and validates the inner archive `file:` path against `rootDir`.
    /// - Percent-decodes and lexically normalizes non-existent paths to prevent leaking
    ///   file existence outside `rootDir`.
    pub fn check_root_dir_lexical(
        &self,
        uri: &PklString,
        is_resource: bool,
    ) -> Result<(), PklString> {
        let Some(root_dir) = &self.root_dir else {
            return Ok(());
        };
        let Some(file_info) = extract_file_uri_for_root_dir(uri) else {
            return Ok(());
        };
        if file_info.has_non_empty_authority {
            // UNC / remote host authority: reject immediately if lexical path is not under root_dir
            // (matches `SecurityManagers.java:238-240` avoiding ~20s network/SMB probe).
            return Err(root_dir_denied_message(uri, is_resource));
        }
        if !is_path_under_root(&file_info.decoded_path, root_dir) {
            return Err(root_dir_denied_message(uri, is_resource));
        }
        Ok(())
    }

    /// Checks a canonicalized / real filesystem path against `rootDir`, emitting the exact
    /// upstream `modulePastRootDir` / `resourcePastRootDir` message using `original_uri`.
    pub fn check_real_path_under_root_dir(
        &self,
        original_uri: &PklString,
        real_path: &str,
        is_resource: bool,
    ) -> Result<(), PklString> {
        let Some(root_dir) = &self.root_dir else {
            return Ok(());
        };
        if !is_path_under_root(real_path, root_dir) {
            return Err(root_dir_denied_message(original_uri, is_resource));
        }
        Ok(())
    }

    /// Full pure-core module resolution check (`checkResolveModule`): allowlist + lexical `rootDir`.
    pub fn check_resolve_module(&self, uri: &PklString) -> Result<(), ModuleHostError> {
        self.check_allowlist_only(uri, false)
            .and_then(|()| self.check_root_dir_lexical(uri, false))
            .map_err(|msg| ModuleHostError::SecurityManagerException { message: Some(msg) })
    }

    /// Full pure-core resource resolution check (`checkResolveResource`, used by globbing).
    pub fn check_resolve_resource(&self, uri: &PklString) -> Result<(), ModuleHostError> {
        self.check_allowlist_only(uri, true)
            .and_then(|()| self.check_root_dir_lexical(uri, true))
            .map_err(|msg| ModuleHostError::SecurityManagerException { message: Some(msg) })
    }

    /// Full pure-core resource read check (`checkReadResource`).
    pub fn check_read_resource(&self, uri: &PklString) -> Result<(), ReadFailure> {
        self.check_allowlist_only(uri, true)
            .and_then(|()| self.check_root_dir_lexical(uri, true))
            .map_err(|msg| ReadFailure::SecurityManagerException { message: Some(msg) })
    }

    /// Wraps a `resolveSecurePath` `SecurityManagerException` in `IOException`,
    /// matching `ResourceReaders.FileResource.read` (`ResourceReaders.java:275-276`).
    pub fn wrap_file_resource_security_error(message: PklString) -> ReadFailure {
        ReadFailure::Io(IoFailure::from(
            PklString::from("org.pkl.core.SecurityManagerException: ").concat(&message),
        ))
    }
}

pub fn allowlist_denied_message(uri: &PklString, is_resource: bool) -> PklString {
    if is_resource {
        PklString::from("Refusing to read resource `")
            .concat(uri)
            .concat(&"` because it does not match any entry in the resource allowlist (`--allowed-resources`).".into())
    } else {
        PklString::from("Refusing to load module `")
            .concat(uri)
            .concat(
            &"` because it does not match any entry in the module allowlist (`--allowed-modules`)."
                .into(),
        )
    }
}

pub fn root_dir_denied_message(uri: &PklString, is_resource: bool) -> PklString {
    if is_resource {
        PklString::from("Refusing to read resource `")
            .concat(uri)
            .concat(&"` because it is not within the root directory (`--root-dir`).".into())
    } else {
        PklString::from("Refusing to load module `")
            .concat(uri)
            .concat(&"` because it is not within the root directory (`--root-dir`).".into())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileUriRootInfo {
    pub has_non_empty_authority: bool,
    pub decoded_path: String,
    pub from_jar: bool,
}

/// Extracts `file:` path information from `file:` or `jar:file:` URIs for `rootDir` enforcement.
/// Returns `None` for non-file schemes (e.g., `https:`, `modulepath:`, `pkl:`, `jar:nested:`).
pub fn extract_file_uri_for_root_dir(uri: &PklString) -> Option<FileUriRootInfo> {
    let raw = uri.to_java_utf8();
    let (candidate, from_jar) = if let Some(rest) = strip_scheme_ascii(&raw, "jar:") {
        // Strip `!/entry` suffix if present so we inspect the archive's `file:` URI.
        let archive_part = match rest.find("!/") {
            Some(idx) => &rest[..idx],
            None => rest.split('#').next().unwrap_or(rest),
        };
        (archive_part, true)
    } else {
        (raw.as_str(), false)
    };

    let after_file = strip_scheme_ascii(candidate, "file:")?;
    let without_frag = after_file.split('#').next().unwrap_or(after_file);
    let without_query = without_frag.split('?').next().unwrap_or(without_frag);

    let (has_non_empty_authority, raw_path) = if let Some(rest) = without_query.strip_prefix("//") {
        let (auth, path) = match rest.find('/') {
            Some(idx) => (&rest[..idx], &rest[idx..]),
            None => (rest, ""),
        };
        (!auth.is_empty(), path)
    } else {
        (false, without_query)
    };

    let decoded_path = percent_decode_utf8(raw_path);
    Some(FileUriRootInfo {
        has_non_empty_authority,
        decoded_path,
        from_jar,
    })
}

fn strip_scheme_ascii<'a>(s: &'a str, scheme_with_colon: &str) -> Option<&'a str> {
    let prefix = s.get(..scheme_with_colon.len())?;
    if prefix.eq_ignore_ascii_case(scheme_with_colon) {
        Some(&s[scheme_with_colon.len()..])
    } else {
        None
    }
}

/// Percent-decodes UTF-8 bytes from a URI path component (lossy only on invalid UTF-8 sequences).
pub fn percent_decode_utf8(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let (Some(hi), Some(lo)) = (hex_val(bytes[i + 1]), hex_val(bytes[i + 2]))
        {
            out.push((hi << 4) | lo);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

/// Helper checking `file:` leading-slash requirement on decoded scheme-specific part.
pub fn validate_file_uri_has_leading_slash(uri: &PklString) -> Result<(), PklString> {
    let Some(ssp) = decoded_scheme_specific_part(uri) else {
        return Err("File URIs must have a path that starts with `/` (e.g. file:/path/to/my_module.pkl).\nTo resolve relative paths, remove the scheme prefix (remove \"file:\").".into());
    };
    if ssp.starts_with("/") {
        Ok(())
    } else {
        Err("File URIs must have a path that starts with `/` (e.g. file:/path/to/my_module.pkl).\nTo resolve relative paths, remove the scheme prefix (remove \"file:\").".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_policy_matches_pkl_0_32_0() {
        let policy = SecurityPolicy::default_standard();
        assert!(
            policy
                .check_resolve_module(&"file:///tmp/a.pkl".into())
                .is_ok()
        );
        assert!(
            policy
                .check_resolve_module(&"https://example.com/a.pkl".into())
                .is_ok()
        );
        assert!(
            policy
                .check_resolve_module(&"modulepath:/a/b.pkl".into())
                .is_ok()
        );
        assert!(
            policy
                .check_resolve_module(&"jar:file:///tmp/a.jar!/a.pkl".into())
                .is_ok()
        );
        assert!(
            policy
                .check_resolve_module(&"jar:nested:/a.jar!/b.jar!/a.pkl".into())
                .is_ok()
        );
        assert!(
            policy
                .check_resolve_module(&"http://example.com/a.pkl".into())
                .is_err()
        );

        assert!(policy.check_read_resource(&"env:HOME".into()).is_ok());
        assert!(policy.check_read_resource(&"prop:os.name".into()).is_ok());
        assert!(
            policy
                .check_read_resource(&"https://example.com/a.txt".into())
                .is_ok()
        );
        assert!(
            policy
                .check_read_resource(&"http://example.com/a.txt".into())
                .is_err()
        );
    }

    #[test]
    fn root_dir_handles_jar_unc_traversal_and_component_boundary() {
        let policy = SecurityPolicy::default_standard().with_root_dir(Some("/tmp/root".into()));
        assert!(
            policy
                .check_resolve_module(&"file:///tmp/root/a.pkl".into())
                .is_ok()
        );
        assert!(
            policy
                .check_resolve_module(&"file:///tmp/root2/a.pkl".into())
                .is_err()
        );
        assert!(
            policy
                .check_resolve_module(&"file:///tmp/root/../secret.pkl".into())
                .is_err()
        );
        assert!(
            policy
                .check_resolve_module(&"file:///tmp/root/%2e%2e/secret.pkl".into())
                .is_err()
        );
        assert!(
            policy
                .check_resolve_module(&"file://127.0.0.1/tmp/root/a.pkl".into())
                .is_err()
        );
        assert!(
            policy
                .check_resolve_module(&"jar:file:///tmp/root/a.jar!/m.pkl".into())
                .is_ok()
        );
        assert!(
            policy
                .check_resolve_module(&"jar:file:///tmp/other/a.jar!/m.pkl".into())
                .is_err()
        );
        // Non-file schemes unaffected by rootDir
        assert!(
            policy
                .check_resolve_module(&"https://example.com/a.pkl".into())
                .is_ok()
        );
    }

    #[test]
    fn root_dir_handles_windows_verbatim_and_drive_uri_paths() {
        let root = r"\\?\C:\Users\runneradmin\AppData\Local\Temp\proj";
        assert_eq!(
            normalize_lexical_path(root),
            "C:/Users/runneradmin/AppData/Local/Temp/proj"
        );
        assert_eq!(
            normalize_lexical_path("/c:/Users/runneradmin/AppData/Local/Temp/proj/main.pkl"),
            "C:/Users/runneradmin/AppData/Local/Temp/proj/main.pkl"
        );
        assert_eq!(normalize_lexical_path(r"C:\a\..\..\b"), "C:/b");
        assert!(is_path_under_root(
            "/C:/Users/runneradmin/AppData/Local/Temp/proj/main.pkl",
            root
        ));
        assert!(is_path_under_root(
            r"\\?\C:\Users\runneradmin\AppData\Local\Temp\proj\sub\lib.pkl",
            r"C:\Users\runneradmin\AppData\Local\Temp\proj"
        ));
        assert!(!is_path_under_root(
            "/C:/Users/runneradmin/AppData/Local/Temp/proj2/secret.pkl",
            root
        ));
        assert!(!is_path_under_root(
            "/C:/Users/runneradmin/AppData/Local/Temp/proj/../secret.pkl",
            root
        ));
        assert!(is_path_under_root("C:/any/file.pkl", r"\\?\C:\"));
        assert!(!is_path_under_root("D:/any/file.pkl", r"\\?\C:\"));
    }

    #[test]
    fn external_schemes_append_only_to_default_allowlist() {
        let default_ext = SecurityPolicy::default_standard()
            .with_external_module_schemes(&["custom+ext".into()])
            .unwrap();
        assert!(
            default_ext
                .check_resolve_module(&"custom+ext:/a.pkl".into())
                .is_ok()
        );

        let custom = SecurityPolicy::new(Some(&["file:".into()]), None, None)
            .unwrap()
            .with_external_module_schemes(&["custom+ext".into()])
            .unwrap();
        assert!(
            custom
                .check_resolve_module(&"custom+ext:/a.pkl".into())
                .is_err()
        );
    }
}
