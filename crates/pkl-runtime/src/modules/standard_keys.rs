//! Explicit non-I/O portion of the standard file/HTTP/package/generic-URL key profile.
//! Native development Hosts opt in; custom Hosts retain their own factories.
use super::*;

fn selected_kind(uri: &PklString) -> Result<Option<ModuleKeyKind>, ModuleHostError> {
    let source = SourceText::from(uri.clone());
    let Some((scheme, tail)) = source.entire().split_once(":") else {
        return Ok(None);
    };
    let Some(scheme) = scheme.as_str() else {
        return Ok(None);
    };
    if scheme.eq_ignore_ascii_case("file") {
        // IoUtils.validateFileUri checks the decoded scheme-specific part.
        let ssp = crate::host::resources::decoded_scheme_specific_part(uri).ok_or_else(|| {
            ModuleHostError::Unsupported("Invalid URI supplied to the standard key factory.".into())
        })?;
        if !ssp.starts_with("/") {
            return Err(ModuleHostError::UriSyntax {
                reason: "File URIs must have a path that starts with `/` (e.g. file:/path/to/my_module.pkl).\nTo resolve relative paths, remove the scheme prefix (remove \"file:\").".into(),
            });
        }
        return Ok(Some(ModuleKeyKind::StandardFile));
    }
    if scheme.eq_ignore_ascii_case("http") || scheme.eq_ignore_ascii_case("https") {
        return Ok(Some(ModuleKeyKind::StandardHttp));
    }
    if scheme.eq_ignore_ascii_case("package") || scheme.eq_ignore_ascii_case("projectpackage") {
        match crate::packages::PackageAssetUri::parse(uri) {
            Ok(_) => return Ok(Some(ModuleKeyKind::StandardGenericUrl)),
            Err(failure) => {
                return Err(match &failure.failure {
                    crate::packages::PackageUriFailure::Unsupported(reason) => {
                        ModuleHostError::Unsupported(reason.clone())
                    }
                    _ => ModuleHostError::UriSyntax {
                        reason: failure.reason(),
                    },
                });
            }
        }
    }
    // These have earlier built-in factories or provider semantics that this
    // bounded profile does not implement. Do not substitute GenericUrl for them.
    if ["pkl", "modulepath", "repl", "jrt"]
        .iter()
        .any(|known| scheme.eq_ignore_ascii_case(known))
    {
        return Ok(None);
    }
    // ModuleKeyFactories.GenericUrl is the last fallback. It accepts absolute
    // hierarchical URIs, or the opaque jar scheme (case-insensitive), before URL resolution.
    Ok(
        (tail.starts_with("/") || scheme.eq_ignore_ascii_case("jar"))
            .then_some(ModuleKeyKind::StandardGenericUrl),
    )
}

fn owned(key: &ModuleKey) -> bool {
    matches!(
        key.kind,
        ModuleKeyKind::StandardFile
            | ModuleKeyKind::StandardHttp
            | ModuleKeyKind::StandardGenericUrl
    )
}

/// Policy-aware standard key profile supporting `file:`, `modulepath:`, `http:`, `https:`,
/// `jar:`, `package:`, and `projectpackage:`.
///
/// Returns `None` when actual platform I/O (filesystem, modulepath archive/dir lookup,
/// HTTP transport, or generic URL I/O) is needed.
pub fn standard_key_completion_with_policy(
    request: &ModuleRequest,
    policy: &crate::host::security::SecurityPolicy,
) -> Option<ModuleCompletion> {
    match &request.kind {
        ModuleRequestKind::SelectKey { id, uri, .. } => {
            let source = SourceText::from(uri.clone());
            if let Some((scheme, _)) = source.entire().split_once(":")
                && scheme
                    .as_str()
                    .is_some_and(|s| s.eq_ignore_ascii_case("modulepath"))
            {
                return match crate::host::modulepath::parse_modulepath_uri(uri) {
                    Ok(_) => Some(ModuleCompletion::KeySelected(Ok(ModuleKey {
                        id: *id,
                        uri: uri.clone(),
                        kind: ModuleKeyKind::Host,
                        handle: "builtin:modulepath".into(),
                    }))),
                    Err(reason) => Some(ModuleCompletion::KeySelected(Err(
                        ModuleHostError::UriSyntax { reason },
                    ))),
                };
            }
            match selected_kind(uri) {
                Ok(Some(kind)) => Some(ModuleCompletion::KeySelected(Ok(ModuleKey {
                    id: *id,
                    uri: uri.clone(),
                    kind,
                    handle: PklString::default(),
                }))),
                Ok(None) => None,
                Err(error) => Some(ModuleCompletion::KeySelected(Err(error))),
            }
        }
        ModuleRequestKind::QueryCachePolicy { key }
            if owned(key) || key.handle == "builtin:modulepath" =>
        {
            Some(ModuleCompletion::CachePolicy(Ok(true)))
        }
        ModuleRequestKind::QueryFragmentPaths { key }
            if owned(key) || key.handle == "builtin:modulepath" =>
        {
            let has_fragment =
                key.uri.starts_with("package:") || key.uri.starts_with("projectpackage:");
            Some(ModuleCompletion::FragmentPaths(Ok(has_fragment)))
        }
        ModuleRequestKind::Capabilities { module, key } => {
            let uri = key.as_ref().map(|k| &k.uri).unwrap_or(&module.original_uri);
            if uri.starts_with("package:") || uri.starts_with("projectpackage:") {
                return Some(ModuleCompletion::Capabilities(Ok(
                    crate::modules::ModuleCapabilities {
                        local: true,
                        hierarchical: true,
                        fragment_paths: true,
                    },
                )));
            }
            if key
                .as_ref()
                .is_some_and(|k| k.handle == "builtin:modulepath")
                || uri.starts_with("modulepath:")
            {
                return Some(ModuleCompletion::Capabilities(Ok(
                    crate::modules::ModuleCapabilities {
                        local: true,
                        hierarchical: true,
                        fragment_paths: false,
                    },
                )));
            }
            if key
                .as_ref()
                .is_some_and(|k| k.kind == ModuleKeyKind::StandardFile)
                || uri.starts_with("file:")
            {
                return Some(ModuleCompletion::Capabilities(Ok(
                    crate::modules::ModuleCapabilities {
                        local: true,
                        hierarchical: true,
                        fragment_paths: false,
                    },
                )));
            }
            if key
                .as_ref()
                .is_some_and(|k| k.kind == ModuleKeyKind::StandardHttp)
                || uri.starts_with("http:")
                || uri.starts_with("https:")
            {
                return Some(ModuleCompletion::Capabilities(Ok(
                    crate::modules::ModuleCapabilities {
                        local: false,
                        hierarchical: true,
                        fragment_paths: false,
                    },
                )));
            }
            None
        }
        ModuleRequestKind::Resolve { key, .. } | ModuleRequestKind::ResolveKey { key, .. }
            if owned(key) || key.handle == "builtin:modulepath" =>
        {
            let is_resolve_key = matches!(request.kind, ModuleRequestKind::ResolveKey { .. });
            let err_completion = |error: ModuleHostError| {
                if is_resolve_key {
                    ModuleCompletion::KeyResolved(Err(error))
                } else {
                    ModuleCompletion::Resolved(Err(error))
                }
            };
            if key.uri.starts_with("package:") || key.uri.starts_with("projectpackage:") {
                if let Err(error) = policy.check_resolve_module(&key.uri) {
                    return Some(err_completion(error));
                }
                let result = crate::packages::resolver::resolve_package_module(&key.uri);
                if let ModuleRequestKind::ResolveKey {
                    resolved_key_id, ..
                } = &request.kind
                {
                    return Some(ModuleCompletion::KeyResolved(result.map(|module| {
                        ResolvedModuleKey {
                            id: *resolved_key_id,
                            key: key.clone(),
                            handle: PklString::default(),
                            module,
                        }
                    })));
                }
                return Some(ModuleCompletion::Resolved(result));
            }
            // StandardHttp checks security AFTER rewrites and on every redirect hop inside HttpClient.
            // For local `file:` / `jar:file:` delegated to a native Host, enforce allowlist here
            // and let the native Host enforce canonical real-path `rootDir` (handling OS symlinks
            // like macOS `/var -> /private/var` accurately).
            if key.kind != ModuleKeyKind::StandardHttp
                && let Err(msg) = policy.check_allowlist_only(&key.uri, false)
            {
                return Some(err_completion(ModuleHostError::SecurityManagerException {
                    message: Some(msg),
                }));
            }
            // Delegate actual I/O (file, modulepath, http, jar:file:) to platform Host.
            None
        }
        _ => standard_key_completion(request),
    }
}

/// Explicit standard factory/capability profile with no I/O. Callers can use
/// `default_completion` for bundled keys and import trust first. `None` means
/// this helper cannot complete the operation, not a successful resolution.
///
/// File resolution is delegated after its allowlist check. HTTP resolution is
/// left unsupported because request-construction errors precede its security
/// callback; the GenericUrl security check is safe before any URL I/O.
pub fn standard_key_completion(request: &ModuleRequest) -> Option<ModuleCompletion> {
    match &request.kind {
        ModuleRequestKind::SelectKey { id, uri, .. } => match selected_kind(uri) {
            Ok(Some(kind)) => Some(ModuleCompletion::KeySelected(Ok(ModuleKey {
                id: *id,
                uri: uri.clone(),
                kind,
                handle: PklString::default(),
            }))),
            Ok(None) => None,
            Err(error) => Some(ModuleCompletion::KeySelected(Err(error))),
        },
        ModuleRequestKind::QueryCachePolicy { key } if owned(key) => {
            Some(ModuleCompletion::CachePolicy(Ok(true)))
        }
        ModuleRequestKind::QueryFragmentPaths { key } if owned(key) => {
            let has_fragment =
                key.uri.starts_with("package:") || key.uri.starts_with("projectpackage:");
            Some(ModuleCompletion::FragmentPaths(Ok(has_fragment)))
        }
        ModuleRequestKind::Capabilities { module, key } => {
            if module.original_uri.starts_with("package:")
                || module.original_uri.starts_with("projectpackage:")
                || key.as_ref().is_some_and(|k| {
                    k.uri.starts_with("package:") || k.uri.starts_with("projectpackage:")
                })
            {
                return Some(ModuleCompletion::Capabilities(Ok(
                    crate::modules::ModuleCapabilities {
                        local: true,
                        hierarchical: true,
                        fragment_paths: true,
                    },
                )));
            }
            None
        }
        ModuleRequestKind::ResolveImportUri { key, authored_uri } => {
            if key.uri.starts_with("package:")
                || key.uri.starts_with("projectpackage:")
                || authored_uri.starts_with("@")
            {
                let result = crate::packages::resolver::resolve_import_uri(&key.uri, authored_uri);
                return Some(ModuleCompletion::ImportUri(result));
            }
            None
        }
        ModuleRequestKind::LoadSource { resolved, .. } => {
            if resolved.resolved_uri.starts_with("package:")
                || resolved.resolved_uri.starts_with("projectpackage:")
                || resolved.original_uri.starts_with("package:")
                || resolved.original_uri.starts_with("projectpackage:")
                || resolved.source_uri.starts_with("package:")
                || resolved.source_uri.starts_with("projectpackage:")
                || crate::packages::resolver::get_stored_source(
                    resolved.resolved_uri.as_str().unwrap_or(""),
                )
                .is_some()
                || crate::packages::resolver::get_stored_source(
                    resolved.original_uri.as_str().unwrap_or(""),
                )
                .is_some()
                || crate::packages::resolver::get_stored_source(
                    resolved.source_uri.as_str().unwrap_or(""),
                )
                .is_some()
            {
                let result = crate::packages::resolver::load_source(resolved);
                return Some(ModuleCompletion::Source(result));
            }
            None
        }
        ModuleRequestKind::HasElement { element_uri, .. } => {
            if element_uri.starts_with("package:") || element_uri.starts_with("projectpackage:") {
                let result = crate::packages::resolver::has_element(element_uri);
                return Some(ModuleCompletion::HasElement(result));
            }
            None
        }
        ModuleRequestKind::Resolve { key, .. } | ModuleRequestKind::ResolveKey { key, .. }
            if owned(key) =>
        {
            if key.uri.starts_with("package:") || key.uri.starts_with("projectpackage:") {
                let result = crate::packages::resolver::resolve_package_module(&key.uri);
                return Some(ModuleCompletion::Resolved(result));
            }
            if key.kind != ModuleKeyKind::StandardHttp
                && let Err(error) = crate::glob::check_default_resolve(&key.uri, false)
            {
                return Some(ModuleCompletion::Resolved(Err(error)));
            }
            if key.kind == ModuleKeyKind::StandardFile {
                None
            } else {
                Some(ModuleCompletion::Resolved(Err(
                    ModuleHostError::Unsupported(
                        "The standard module key profile does not implement URL source I/O.".into(),
                    ),
                )))
            }
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolution_hint_decodes_only_the_path_once() {
        for (uri, hint) in [
            ("file:/missing%5Cchild.pkl", true),
            ("file:/missing%255Cchild.pkl", false),
            ("file:/missing.pkl?query=%5C", false),
            ("file:/missing.pkl#%5C", false),
            ("other:opaque%5Cpath", false),
        ] {
            assert_eq!(
                ModuleHostError::NotFound
                    .into_resolution_runtime(uri)
                    .hint
                    .is_some(),
                hint
            );
            assert!(
                ModuleHostError::NotFound
                    .into_factory_runtime(uri)
                    .hint
                    .is_none()
            );
            assert!(ModuleHostError::NotFound.into_runtime(uri).hint.is_none());
        }
    }
}
