//! Directory classpath capability for the JVM snippet development adapter.
//! The shared VM retains module identity, source caching and diagnostics.
use pkl_runtime::modules::{
    ModuleCompletion, ModuleHostError, ModuleRequest, ModuleRequestKind, ModuleSource,
    ResolvedModule,
};
use std::path::{Component, Path, PathBuf};

pub struct DirectoryClassPath {
    root: PathBuf,
}

impl DirectoryClassPath {
    pub fn new(root: &Path) -> Result<Self, ModuleHostError> {
        let root = std::path::absolute(root).map_err(super::module_host::io_error)?;
        if !root.is_dir() {
            return Err(ModuleHostError::Unsupported(
                "The JVM snippet Host requires an existing directory classpath.".into(),
            ));
        }
        Ok(Self { root })
    }

    pub fn complete(&self, request: &ModuleRequest) -> ModuleCompletion {
        match &request.kind {
            ModuleRequestKind::SelectKey { id, uri, .. } if uri.starts_with("modulepath:") => {
                ModuleCompletion::KeySelected(Self::resource_path(uri).map(|_| {
                    pkl_runtime::modules::ModuleKey {
                        id: *id,
                        uri: uri.clone(),
                        kind: pkl_runtime::modules::ModuleKeyKind::Host,
                        handle: "classpath".into(),
                    }
                }))
            }
            ModuleRequestKind::Resolve { key, .. } | ModuleRequestKind::ResolveKey { key, .. }
                if key.uri.starts_with("modulepath:") =>
            {
                ModuleCompletion::Resolved(self.resolve(&key.uri, &key.uri))
            }
            ModuleRequestKind::LoadSource { resolved, .. }
                if resolved.original_uri.starts_with("modulepath:") =>
            {
                ModuleCompletion::Source(self.load(resolved))
            }
            _ => super::module_host::complete(request),
        }
    }

    fn resource_path(uri: &str) -> Result<PathBuf, ModuleHostError> {
        let unsupported = || {
            ModuleHostError::Unsupported(
                "The directory classpath Host requires a hierarchical modulepath URI without authority, query, fragment or dot segments.".into(),
            )
        };
        let tail = uri.strip_prefix("modulepath:").ok_or_else(unsupported)?;
        if !tail.starts_with('/') || tail.starts_with("//") || tail.contains(['?', '#']) {
            return Err(unsupported());
        }
        let decoded = super::module_host::file_path(&format!("file:{tail}"))?;
        let path = decoded.strip_prefix("/").map_err(|_| unsupported())?;
        if path.as_os_str().is_empty()
            || path
                .components()
                .any(|part| !matches!(part, Component::Normal(_)))
            || tail.split('/').any(|part| matches!(part, "." | ".."))
        {
            return Err(unsupported());
        }
        Ok(path.to_owned())
    }

    fn resolve(
        &self,
        import_uri: &str,
        original_uri: &str,
    ) -> Result<ResolvedModule, ModuleHostError> {
        let relative = Self::resource_path(import_uri)?;
        let path = self.root.join(relative);
        // ClassLoader.getResource returns a URL retaining a symlink's spelling.
        // Do not reuse the file ModuleKey's canonicalize() operation here.
        let metadata = std::fs::metadata(&path).map_err(super::module_host::io_error)?;
        if !metadata.is_file() {
            return Err(ModuleHostError::Unsupported(
                "The directory classpath Host requires a regular resource file.".into(),
            ));
        }
        let uri = super::module_host::file_uri(&path)?;
        let uri = format!("file:{}", uri.strip_prefix("file://").expect("file URI"));
        Ok(ResolvedModule {
            original_uri: original_uri.into(),
            resolved_uri: uri.into(),
            source_uri: original_uri.into(),
        })
    }

    fn load(&self, module: &ResolvedModule) -> Result<ModuleSource, ModuleHostError> {
        let path = super::module_host::file_path(&module.resolved_uri)?;
        let text = std::fs::read_to_string(path).map_err(super::module_host::io_error)?;
        let original = Self::resource_path(&module.original_uri)?;
        let name = original
            .file_stem()
            .and_then(|value| value.to_str())
            .ok_or_else(|| {
                ModuleHostError::Unsupported(
                    "The directory classpath Host requires a UTF-8 module name.".into(),
                )
            })?;
        Ok(ModuleSource {
            text: text.into(),
            inferred_name: name.into(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsupported_classpath_forms_cannot_escape_the_configured_root() {
        for uri in [
            "modulepath:child.pkl",
            "modulepath://server/child.pkl",
            "modulepath:/../child.pkl",
            "modulepath:/%2e%2e/child.pkl",
            "modulepath:/child.pkl?q",
            "modulepath:/child.pkl#f",
        ] {
            assert!(
                matches!(
                    DirectoryClassPath::resource_path(uri),
                    Err(ModuleHostError::Unsupported(_))
                ),
                "{uri}"
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn classpath_symlink_identity_and_original_source_are_retained() {
        let root = std::env::temp_dir().join(format!("pkl-classpath-host-{}", std::process::id()));
        std::fs::create_dir(&root).unwrap();
        std::fs::write(root.join("child.pkl"), "value = 7\n").unwrap();
        std::os::unix::fs::symlink("child.pkl", root.join("alias.pkl")).unwrap();
        let host = DirectoryClassPath::new(&root).unwrap();
        let child = host
            .resolve("modulepath:/child.pkl", "modulepath:/child.pkl")
            .unwrap();
        let alias = host
            .resolve("modulepath:/alias.pkl", "modulepath:/alias.pkl")
            .unwrap();
        assert_ne!(child.resolved_uri, alias.resolved_uri);
        assert!(alias.resolved_uri.ends_with("/alias.pkl"));
        assert!(alias.resolved_uri.starts_with("file:/"));
        assert!(!alias.resolved_uri.starts_with("file://"));
        assert_eq!(&*alias.source_uri, "modulepath:/alias.pkl");
        let source = host.load(&alias).unwrap();
        assert_eq!(&*source.inferred_name, "alias");
        assert_eq!(&*source.text, "value = 7\n");
        assert_eq!(
            host.resolve("modulepath:/missing.pkl", "modulepath:/missing.pkl"),
            Err(ModuleHostError::NotFound)
        );
        std::fs::remove_dir_all(root).unwrap();
    }
}
