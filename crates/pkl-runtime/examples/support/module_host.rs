//! Filesystem capability for development examples. URI preparation, module
//! caches, parsing, and evaluation belong to the shared runtime.
use pkl_runtime::modules::{
    ModuleCompletion, ModuleHostError, ModuleRequest, ModuleRequestKind, ModuleSource,
    ResolvedModule,
};
use pkl_runtime::value::PklString;
use std::path::{Path, PathBuf};

pub fn complete(request: &ModuleRequest) -> ModuleCompletion {
    if let Some(completion) = pkl_runtime::modules::default_completion(request) {
        return completion;
    }
    // Selection/capability/policy are shared and perform no filesystem I/O.
    // In particular, an unsupported native file conversion is not NoLoader.
    if let Some(completion) = pkl_runtime::modules::standard_key_completion(request) {
        return completion;
    }
    match &request.kind {
        ModuleRequestKind::ResolveImportUri { .. } => {
            unreachable!("explicit default handles inherited URI resolution")
        }
        ModuleRequestKind::SelectKey { id, uri, .. } => {
            if uri.starts_with("package:") || uri.starts_with("projectpackage:") {
                ModuleCompletion::KeySelected(
                    match pkl_runtime::packages::PackageAssetUri::parse(uri.clone()) {
                        Ok(_) => Ok(pkl_runtime::modules::ModuleKey {
                            id: *id,
                            uri: uri.clone(),
                            kind: pkl_runtime::modules::ModuleKeyKind::StandardGenericUrl,
                            handle: "".into(),
                        }),
                        Err(failure) => Err(match &failure.failure {
                            pkl_runtime::packages::PackageUriFailure::Unsupported(reason) => {
                                ModuleHostError::Unsupported(reason.clone())
                            }
                            _ => ModuleHostError::UriSyntax {
                                reason: failure.reason(),
                            },
                        }),
                    },
                )
            } else {
                ModuleCompletion::KeySelected(
                    file_path(uri)
                        .map_err(|error| {
                            if uri.as_str().is_none() {
                                error
                            } else {
                                ModuleHostError::NoLoader
                            }
                        })
                        .map(|_| pkl_runtime::modules::ModuleKey {
                            id: *id,
                            uri: uri.clone(),
                            kind: pkl_runtime::modules::ModuleKeyKind::Host,
                            handle: "file".into(),
                        }),
                )
            }
        }
        ModuleRequestKind::QueryCachePolicy { .. } => ModuleCompletion::CachePolicy(Ok(true)),
        ModuleRequestKind::QueryFragmentPaths { key } => ModuleCompletion::FragmentPaths(Ok(key
            .uri
            .starts_with("package:")
            || key.uri.starts_with("projectpackage:"))),
        ModuleRequestKind::CheckImport { .. } => {
            unreachable!("default policy handles import admission")
        }
        ModuleRequestKind::Capabilities { module, key } => {
            if module.original_uri.starts_with("package:")
                || module.original_uri.starts_with("projectpackage:")
                || key.as_ref().is_some_and(|k| {
                    k.uri.starts_with("package:") || k.uri.starts_with("projectpackage:")
                })
            {
                ModuleCompletion::Capabilities(Ok(pkl_runtime::modules::ModuleCapabilities {
                    local: true,
                    hierarchical: true,
                    fragment_paths: true,
                }))
            } else {
                ModuleCompletion::Capabilities(file_path(&module.original_uri).map(|_| {
                    pkl_runtime::modules::ModuleCapabilities {
                        local: true,
                        hierarchical: true,
                        fragment_paths: false,
                    }
                }))
            }
        }
        ModuleRequestKind::HasElement {
            module,
            element_uri,
            ..
        } => ModuleCompletion::HasElement(
            pkl_runtime::glob::check_default_resolve(element_uri, false).and_then(|_| {
                if element_uri.starts_with("package:") || element_uri.starts_with("projectpackage:")
                {
                    pkl_runtime::packages::resolver::has_element(element_uri)
                } else {
                    let _ = file_path(&module.original_uri);
                    file_path(element_uri).map(|path| path.exists())
                }
            }),
        ),
        ModuleRequestKind::ResolveKey { key, .. } => {
            let res = if key.uri.starts_with("package:") || key.uri.starts_with("projectpackage:") {
                pkl_runtime::packages::resolver::resolve_package_module(&key.uri)
            } else {
                resolve(&key.uri, &key.uri)
            };
            match res {
                Ok(resolved) => ModuleCompletion::Resolved(Ok(resolved)),
                Err(err) => ModuleCompletion::KeyResolved(Err(err)),
            }
        }
        ModuleRequestKind::Resolve { key, .. } => {
            let res = if key.uri.starts_with("package:") || key.uri.starts_with("projectpackage:") {
                pkl_runtime::packages::resolver::resolve_package_module(&key.uri)
            } else {
                resolve(&key.uri, &key.uri)
            };
            ModuleCompletion::Resolved(res)
        }
        ModuleRequestKind::LoadSource { resolved, .. } => {
            if resolved.resolved_uri.starts_with("package:")
                || resolved.resolved_uri.starts_with("projectpackage:")
            {
                ModuleCompletion::Source(pkl_runtime::packages::resolver::load_source(resolved))
            } else {
                ModuleCompletion::Source(load(resolved))
            }
        }
    }
}

fn resolve(
    import_uri: impl Into<PklString>,
    original_uri: impl Into<PklString>,
) -> Result<ResolvedModule, ModuleHostError> {
    let import_uri = import_uri.into();
    let original_uri = original_uri.into();
    let path = file_path(&import_uri)?.canonicalize().map_err(io_error)?;
    let uri = file_uri(&path)?;
    Ok(ResolvedModule {
        original_uri: original_uri.clone(),
        resolved_uri: uri.into(),
        // VmUtils.loadSource retains the original ModuleKey URI for source
        // diagnostics even when resolution canonicalizes a symlink alias.
        source_uri: original_uri,
    })
}

fn load(module: &ResolvedModule) -> Result<ModuleSource, ModuleHostError> {
    let path = file_path(&module.resolved_uri)?;
    let text = std::fs::read_to_string(&path).map_err(io_error)?;
    // The original key also determines the inferred declaration name, while
    // only the resolved identity selects the file to read.
    let original_path = file_path(&module.original_uri)?;
    let name = original_path
        .file_stem()
        .and_then(|value| value.to_str())
        .ok_or_else(|| {
            ModuleHostError::Unsupported(
                "Non-UTF-8 module filenames are not supported by this development Host.".into(),
            )
        })?;
    Ok(ModuleSource {
        text: text.into(),
        inferred_name: name.into(),
    })
}

pub(super) fn io_error(error: std::io::Error) -> ModuleHostError {
    if error.kind() == std::io::ErrorKind::NotFound {
        ModuleHostError::NotFound
    } else if error.kind() == std::io::ErrorKind::IsADirectory
        || error.to_string().starts_with("Is a directory")
    {
        ModuleHostError::Io("Is a directory".into())
    } else {
        ModuleHostError::Io(error.to_string().into())
    }
}

pub(super) fn file_path(uri: impl Into<PklString>) -> Result<PathBuf, ModuleHostError> {
    let uri = uri.into();
    let uri = uri.as_str().ok_or_else(|| ModuleHostError::Unsupported(
        "This development filesystem Host cannot represent an unpaired UTF-16 surrogate in a URI.".into(),
    ))?;
    let unsupported = || {
        ModuleHostError::Unsupported(
            "This development module Host supports absolute local file URIs only.".into(),
        )
    };
    let (scheme, tail) = uri.split_once(':').ok_or_else(unsupported)?;
    if !scheme.eq_ignore_ascii_case("file") || tail.contains(['?', '#']) {
        return Err(unsupported());
    }
    let path = if let Some(authority_path) = tail.strip_prefix("//") {
        if !authority_path.starts_with('/') {
            return Err(unsupported());
        }
        authority_path
    } else {
        tail
    };
    if !path.starts_with('/') {
        return Err(unsupported());
    }
    // IoUtils.pathOf calls URI.toASCIIString before Path.of. Normalize only
    // the entire literal string before quoting, without decoding existing
    // escapes. A combining mark can therefore change an adjacent hex letter;
    // malformed escapes after that transformation remain unsupported here.
    let path = java_ascii_file_path(path);
    // UnixUriUtils uses its byte-path branch only for this exact spelling.
    // Other accepted scheme spellings take File(URI), which first decodes text.
    #[cfg(unix)]
    let raw_unix_path = uri.starts_with("file:///");
    #[cfg(not(unix))]
    let raw_unix_path = false;
    let path = if raw_unix_path && path.len() > 1 && path.ends_with('/') {
        &path[..path.len() - 1]
    } else {
        path.as_ref()
    };
    let mut decoded = Vec::with_capacity(path.len());
    let mut index = 0;
    while index < path.len() {
        let byte = path.as_bytes()[index];
        if byte == b'%' {
            let pair = path.get(index + 1..index + 3).ok_or_else(unsupported)?;
            decoded.push(u8::from_str_radix(pair, 16).map_err(|_| unsupported())?);
            index += 3;
        } else {
            decoded.push(byte);
            index += 1;
        }
    }
    if decoded.contains(&0) {
        return Err(unsupported());
    }
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt;
        // Both Java Unix path constructors collapse repeated separators. Only
        // File(URI)'s text route removes every trailing decoded separator.
        let mut previous_slash = false;
        decoded.retain(|byte| {
            let slash = *byte == b'/';
            let retain = !slash || !previous_slash;
            previous_slash = slash;
            retain
        });
        if raw_unix_path {
            return Ok(PathBuf::from(std::ffi::OsString::from_vec(decoded)));
        }
        while decoded.len() > 1 && decoded.last() == Some(&b'/') {
            decoded.pop();
        }
        let text = pkl_runtime::resources::decode_utf8(&decoded);
        Ok(PathBuf::from(
            text.as_str().expect("UTF-8 decoder emits scalars"),
        ))
    }
    #[cfg(not(unix))]
    {
        // Windows provider/native-encoding behavior is not qualified by the
        // Unix oracle; preserve the previous strict scalar-only boundary.
        let text = String::from_utf8(decoded).map_err(|_| unsupported())?;
        Ok(PathBuf::from(text))
    }
}

// The pinned unicode-normalization dependency uses Unicode 15.0 tables,
// matching JDK 21. Raw Java/native boundary comparisons qualify behavior.
fn java_ascii_file_path(path: &str) -> std::borrow::Cow<'_, str> {
    if path.is_ascii() {
        return std::borrow::Cow::Borrowed(path);
    }
    use std::fmt::Write;
    use unicode_normalization::UnicodeNormalization;
    let normalized: String = path.nfc().collect();
    let mut ascii = String::with_capacity(normalized.len());
    for byte in normalized.bytes() {
        if byte.is_ascii() {
            ascii.push(char::from(byte));
        } else {
            write!(&mut ascii, "%{byte:02X}").expect("String write");
        }
    }
    std::borrow::Cow::Owned(ascii)
}

pub(super) fn file_uri(path: &Path) -> Result<String, ModuleHostError> {
    let text = path.to_str().ok_or_else(|| {
        ModuleHostError::Unsupported(
            "Non-UTF-8 module paths are not supported by this development Host.".into(),
        )
    })?;
    let mut uri = String::from("file://");
    use std::fmt::Write;
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || b"/-._~!$&'()*+,;=:@".contains(&byte) {
            uri.push(char::from(byte));
        } else {
            write!(&mut uri, "%{byte:02X}").expect("String write");
        }
    }
    Ok(uri)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Workspace(PathBuf);
    impl Workspace {
        fn new() -> Self {
            static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let id = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let path =
                std::env::temp_dir().join(format!("pkl-module-host-{}-{id}", std::process::id()));
            std::fs::create_dir(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for Workspace {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.0).unwrap();
        }
    }

    #[test]
    fn file_uri_roundtrip_preserves_unicode_and_reserved_filename_bytes() {
        let path = Path::new("/tmp/日本語 #?%.pkl");
        let uri = file_uri(path).unwrap();
        assert_eq!(
            uri,
            "file:///tmp/%E6%97%A5%E6%9C%AC%E8%AA%9E%20%23%3F%25.pkl"
        );
        assert_eq!(file_path(&uri).unwrap(), path);
    }

    #[test]
    fn literal_unicode_is_nfc_before_escaping_but_existing_octets_are_not_normalized() {
        // Source-derived Java URI.toASCIIString expectations; actual Java and
        // native qualification is pending, including normalization table parity.
        for (uri, expected_path) in [
            ("file:/tmp/e\u{301}.pkl", "/tmp/é.pkl"),
            ("file:/tmp/é.pkl", "/tmp/é.pkl"),
            ("file:/tmp/%65%CC%81.pkl", "/tmp/e\u{301}.pkl"),
            (
                "file:/tmp/e\u{301}-%65%CC%81-日本語.pkl",
                "/tmp/é-e\u{301}-日本語.pkl",
            ),
        ] {
            assert_eq!(file_path(uri).unwrap(), Path::new(expected_path));
        }
    }

    #[test]
    fn unsupported_uri_forms_do_not_turn_into_missing_files() {
        for unit in [0xd800, 0xdfff] {
            let uri = PklString::from("file:/").concat(&PklString::from_utf16(vec![unit]));
            assert!(matches!(
                file_path(&uri),
                Err(ModuleHostError::Unsupported(_))
            ));
        }
        for uri in [
            "https://example.test/a.pkl",
            "file://server/a.pkl",
            "file:relative.pkl",
            "file:/a.pkl?x",
            "file:/a.pkl#x",
            "file:/bad%00.pkl",
            "file:/bad%.pkl",
            "file:/bad%GG.pkl",
        ] {
            assert!(matches!(
                file_path(uri),
                Err(ModuleHostError::Unsupported(_))
            ));
        }
    }

    #[cfg(unix)]
    #[test]
    fn unix_file_uri_spelling_selects_text_or_lossless_bytes() {
        use std::os::unix::ffi::OsStrExt;
        // The first control has a closed Java path-text observation. Remaining
        // byte identities are source-derived and await the paired Java oracle.
        for uri in ["file:/tmp/stage147/%FF", "FILE:///tmp/stage147/%FF"] {
            assert_eq!(
                file_path(uri).unwrap().as_os_str().as_bytes(),
                b"/tmp/stage147/\xef\xbf\xbd"
            );
        }
        assert_eq!(
            file_path("file:///tmp/stage147/%FF")
                .unwrap()
                .as_os_str()
                .as_bytes(),
            b"/tmp/stage147/\xff"
        );
        assert_ne!(
            file_path("file:///tmp/stage147/%FF").unwrap(),
            file_path("file:/tmp/stage147/%FF").unwrap()
        );
        for (suffix, expected) in [
            ("%ED%A0%80", "\u{fffd}"),
            ("%E2%82", "\u{fffd}"),
            ("%C3%28", "\u{fffd}("),
            ("%F0%80%80%80", "\u{fffd}\u{fffd}\u{fffd}\u{fffd}"),
        ] {
            assert_eq!(
                file_path(format!("file:/tmp/{suffix}")).unwrap(),
                Path::new(&format!("/tmp/{expected}"))
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn raw_uri_trailing_escape_is_not_text_path_normalization() {
        use std::os::unix::ffi::OsStrExt;
        for (uri, bytes) in [
            ("file:///tmp//name%2F", b"/tmp/name/".as_slice()),
            ("file:/tmp//name%2F", b"/tmp/name".as_slice()),
            ("file:///tmp/name//", b"/tmp/name/".as_slice()),
            ("file:/tmp/name//", b"/tmp/name".as_slice()),
        ] {
            assert_eq!(file_path(uri).unwrap().as_os_str().as_bytes(), bytes);
        }
        // Whole-URI NFC can change the last ASCII hex letter before reparsing.
        // That Java error projection is still unsupported, never a guessed path.
        assert!(matches!(
            file_path("file:/tmp/%4A\u{30c}"),
            Err(ModuleHostError::Unsupported(_))
        ));
        assert!(matches!(
            file_path("file:///tmp/%00"),
            Err(ModuleHostError::Unsupported(_))
        ));
    }

    #[test]
    fn native_file_host_resolves_loads_and_evaluates_real_module_output() {
        use pkl_runtime::evaluation::frontend::{
            ModuleEvalOptions, ModuleSourceInput, evaluate_module_output_with_host,
        };
        let workspace = Workspace::new();
        let child = workspace.0.join("child %.pkl");
        std::fs::write(&child, "value=7\n").unwrap();
        let main_uri = file_uri(&workspace.0.join("main.pkl")).unwrap();
        let mut phases = Vec::new();
        let output = evaluate_module_output_with_host(
            ModuleSourceInput {
                text: "child=import(\"child %.pkl\")\noutput { text=child.value.toString() }\n",
                uri: &main_uri,
                inferred_name: "main",
            },
            &ModuleEvalOptions::default(),
            |request| {
                phases.push(match request.kind {
                    ModuleRequestKind::SelectKey { .. } => "select-key",
                    ModuleRequestKind::QueryCachePolicy { .. } => "query-cache-policy",
                    ModuleRequestKind::QueryFragmentPaths { .. } => "query-fragment-paths",
                    ModuleRequestKind::CheckImport { .. } => "check-import",
                    ModuleRequestKind::Capabilities { .. } => "capabilities",
                    ModuleRequestKind::HasElement { .. } => "has-element",
                    ModuleRequestKind::Resolve { .. } => "resolve",
                    ModuleRequestKind::ResolveKey { .. } => "resolve-key",
                    ModuleRequestKind::ResolveImportUri { .. } => "resolve-import-uri",
                    ModuleRequestKind::LoadSource { .. } => "source",
                });
                complete(request)
            },
        );
        assert_eq!(
            output.exit_code,
            0,
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(output.stdout, b"7");
        assert!(output.stderr.is_empty());
        assert_eq!(
            phases,
            [
                "check-import",
                "select-key",
                "query-cache-policy",
                "resolve",
                "source",
                "query-fragment-paths"
            ]
        );
    }

    #[cfg(unix)]
    #[test]
    fn symlink_aliases_share_a_resolved_module_without_second_source_load() {
        use pkl_runtime::evaluation::frontend::{
            ModuleEvalOptions, ModuleSourceInput, evaluate_module_output_with_host,
        };
        let workspace = Workspace::new();
        std::fs::write(workspace.0.join("child.pkl"), "value=7\n").unwrap();
        std::os::unix::fs::symlink("child.pkl", workspace.0.join("alias.pkl")).unwrap();
        let main_uri = file_uri(&workspace.0.join("main.pkl")).unwrap();
        let mut phases = Vec::new();
        let output = evaluate_module_output_with_host(
            ModuleSourceInput {
                text: "a=import(\"child.pkl\")\nb=import(\"alias.pkl\")\noutput { text=(a==b).toString() }\n",
                uri: &main_uri,
                inferred_name: "main",
            },
            &ModuleEvalOptions::default(),
            |request| {
                phases.push(match request.kind {
                    ModuleRequestKind::SelectKey { .. } => "select-key",
                    ModuleRequestKind::QueryCachePolicy { .. } => "query-cache-policy",
                    ModuleRequestKind::QueryFragmentPaths { .. } => "query-fragment-paths",
                    ModuleRequestKind::CheckImport { .. } => "check-import",
                    ModuleRequestKind::Capabilities { .. } => "capabilities",
                    ModuleRequestKind::HasElement { .. } => "has-element",
                    ModuleRequestKind::Resolve { .. } => "resolve",
                    ModuleRequestKind::ResolveKey { .. } => "resolve-key",
                    ModuleRequestKind::ResolveImportUri { .. } => "resolve-import-uri",
                    ModuleRequestKind::LoadSource { .. } => "source",
                });
                complete(request)
            },
        );
        assert_eq!(
            output.exit_code,
            0,
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(output.stdout, b"true");
        assert_eq!(
            phases,
            [
                "check-import",
                "select-key",
                "query-cache-policy",
                "resolve",
                "source",
                "query-fragment-paths",
                "check-import",
                "select-key",
                "query-cache-policy",
                "resolve"
            ]
        );
    }
}
