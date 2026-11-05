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
            Ok(_) => return Err(ModuleHostError::Unsupported(
                "Package URI validation succeeded, but package/project source resolution is not implemented.".into(),
            )),
            Err(failure) => {
                return Err(match &failure.failure {
                    crate::packages::PackageUriFailure::Unsupported(reason) => ModuleHostError::Unsupported(reason.clone()),
                    _ => ModuleHostError::UriSyntax { reason: failure.reason() },
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
            Some(ModuleCompletion::FragmentPaths(Ok(false)))
        }
        ModuleRequestKind::Resolve { key, .. } | ModuleRequestKind::ResolveKey { key, .. }
            if owned(key) =>
        {
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
