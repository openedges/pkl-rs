//! Read-only local-file glob effects for the native development adapters.
//! Matching, URI resolution, sorting and lazy value loads stay in the shared VM.
use pkl_runtime::{
    glob::{self, GlobCompletion, GlobReader, GlobRequest, GlobRequestKind, PathElement},
    modules::{ModuleHostError, ModuleKeyKind},
    value::PklString,
};

fn file_reader(reader: &GlobReader) -> bool {
    match reader {
        GlobReader::Module(key) => key.kind == ModuleKeyKind::StandardFile,
        GlobReader::Resource(reader) => reader.scheme == "file" && glob::is_default_reader(reader),
    }
}

// Capabilities need no policy/I/O. Element operations authorize the exact URI
// before converting its path or opening any filesystem object.
fn authorized<T>(
    uri: &PklString,
    resource: bool,
    operation: impl FnOnce() -> Result<T, ModuleHostError>,
) -> Result<T, ModuleHostError> {
    glob::check_default_resolve(uri, resource)?;
    operation()
}

fn list_elements(uri: &PklString) -> Result<Vec<PathElement>, ModuleHostError> {
    let path = super::module_host::file_path(uri)?;
    let entries = match std::fs::read_dir(path) {
        Ok(entries) => entries,
        Err(error)
            if matches!(
                error.kind(),
                std::io::ErrorKind::NotFound | std::io::ErrorKind::NotADirectory
            ) =>
        {
            return Ok(Vec::new());
        }
        Err(error) => return Err(ModuleHostError::Io(error.to_string().into())),
    };
    let mut result = Vec::new();
    for entry in entries {
        let entry = entry.map_err(iteration_error)?;
        let path = entry.path();
        // FileResolver skips symlink entries, but does not canonicalize or
        // reject a symlink used as the base directory itself.
        if path.is_symlink() {
            continue;
        }
        let name = entry.file_name().into_string().map_err(|_| {
            ModuleHostError::Unsupported(
                "Non-UTF-8 directory entry names are not supported by this development Host."
                    .into(),
            )
        })?;
        // Java Files.isDirectory follows links and returns false on I/O errors.
        // Keep its separate check after the symlink/name checks.
        result.push(PathElement {
            name: name.into(),
            is_directory: path.is_dir(),
        });
    }
    Ok(result)
}

// Java's DirectoryIteratorException is unchecked, unlike a directory-open
// IOException. Its internal-failure projection is not implemented by this Host;
// preserve that gap instead of exposing an ordinary catchable guest I/O error.
fn iteration_error(error: std::io::Error) -> ModuleHostError {
    ModuleHostError::Unsupported(format!(
        "Directory iteration failed; Java DirectoryIteratorException projection is unavailable: {error}"
    ).into())
}

fn is_package_reader(reader: &GlobReader) -> bool {
    match reader {
        GlobReader::Module(key) => {
            key.uri.starts_with("package:") || key.uri.starts_with("projectpackage:")
        }
        GlobReader::Resource(reader) => {
            reader.scheme == "package" || reader.scheme == "projectpackage"
        }
    }
}

pub fn complete(request: &GlobRequest) -> Option<GlobCompletion> {
    match &request.kind {
        GlobRequestKind::SelectResource { id, uri }
            if uri.starts_with("package:") || uri.starts_with("projectpackage:") =>
        {
            let scheme = if uri.starts_with("package:") {
                "package"
            } else {
                "projectpackage"
            };
            Some(GlobCompletion::Resource(Ok(
                pkl_runtime::glob::ResourceReader {
                    id: *id,
                    scheme: scheme.into(),
                    handle: format!("builtin:{scheme}").into(),
                },
            )))
        }
        GlobRequestKind::IsGlobbable { reader } if is_package_reader(reader) => {
            Some(GlobCompletion::Globbable(Ok(true)))
        }
        GlobRequestKind::IsHierarchical { reader } if is_package_reader(reader) => {
            Some(GlobCompletion::Hierarchical(Ok(true)))
        }
        GlobRequestKind::FragmentPaths { reader }
            if reader.scheme == "package" || reader.scheme == "projectpackage" =>
        {
            Some(GlobCompletion::FragmentPaths(Ok(true)))
        }
        GlobRequestKind::HasElement { uri, reader }
            if uri.starts_with("package:")
                || uri.starts_with("projectpackage:")
                || reader.scheme == "package"
                || reader.scheme == "projectpackage" =>
        {
            Some(GlobCompletion::HasElement(
                pkl_runtime::packages::resolver::has_element(uri).map_err(Into::into),
            ))
        }
        GlobRequestKind::ListElements { uri, reader }
            if uri.starts_with("package:")
                || uri.starts_with("projectpackage:")
                || is_package_reader(reader) =>
        {
            Some(GlobCompletion::Elements(
                pkl_runtime::packages::resolver::list_elements(uri).map_err(Into::into),
            ))
        }
        GlobRequestKind::IsGlobbable {
            reader: GlobReader::Module(key),
        } if key.kind == ModuleKeyKind::StandardFile => Some(GlobCompletion::Globbable(Ok(true))),
        GlobRequestKind::IsHierarchical {
            reader: GlobReader::Module(key),
        } if key.kind == ModuleKeyKind::StandardFile => {
            Some(GlobCompletion::Hierarchical(Ok(true)))
        }
        GlobRequestKind::HasElement { reader, uri }
            if reader.scheme == "file" && glob::is_default_reader(reader) =>
        {
            Some(GlobCompletion::HasElement(
                authorized(uri, true, || {
                    // Files.exists follows symlinks and maps inaccessible/missing
                    // paths to false; Path::exists has the same bounded behavior.
                    Ok(super::module_host::file_path(uri)?.exists())
                })
                .map_err(Into::into),
            ))
        }
        GlobRequestKind::ListElements { reader, uri } if file_reader(reader) => {
            let resource = matches!(reader, GlobReader::Resource(_));
            Some(GlobCompletion::Elements(
                authorized(uri, resource, || list_elements(uri)).map_err(Into::into),
            ))
        }
        // Preserve configured resource metadata/properties/environment and
        // unavailable/custom readers. Never replace another file reader.
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pkl_runtime::{
        glob::{GlobHostError, ResourceReader, ResourceReaderId},
        modules::{ModuleKey, ModuleKeyId},
    };
    use std::{
        cell::Cell,
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
    };
    struct Workspace(PathBuf);
    impl Workspace {
        fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "pkl-file-glob-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir(&path).unwrap();
            Self(path)
        }
        fn uri(&self, name: &str) -> PklString {
            super::super::module_host::file_uri(&self.0.join(name))
                .unwrap()
                .into()
        }
    }
    impl Drop for Workspace {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.0).unwrap();
        }
    }
    fn resource() -> ResourceReader {
        ResourceReader {
            id: ResourceReaderId(7),
            scheme: "file".into(),
            handle: "builtin:file".into(),
        }
    }
    fn listing(uri: PklString, resource_reader: bool) -> Vec<PathElement> {
        let reader = if resource_reader {
            GlobReader::Resource(resource())
        } else {
            GlobReader::Module(ModuleKey {
                id: ModuleKeyId(8),
                uri: uri.clone(),
                kind: ModuleKeyKind::StandardFile,
                handle: "".into(),
            })
        };
        let request = GlobRequest {
            id: 1,
            kind: GlobRequestKind::ListElements { reader, uri },
        };
        let Some(GlobCompletion::Elements(Ok(elements))) = complete(&request) else {
            panic!("file listing")
        };
        elements
    }
    #[test]
    fn missing_and_non_directory_listings_are_empty_for_module_and_resource() {
        let workspace = Workspace::new();
        std::fs::write(workspace.0.join("regular"), b"x").unwrap();
        for resource in [false, true] {
            assert!(listing(workspace.uri("missing"), resource).is_empty());
            assert!(listing(workspace.uri("regular"), resource).is_empty());
        }
    }
    #[test]
    fn entry_names_preserve_utf16_units_and_literal_uri_punctuation() {
        let workspace = Workspace::new();
        let names = ["日本語.pkl", "𐐀%.pkl", "space #.pkl"];
        for name in names {
            std::fs::write(workspace.0.join(name), b"x").unwrap();
        }
        for resource in [false, true] {
            let elements = listing(workspace.uri(""), resource);
            assert_eq!(elements.len(), names.len());
            for name in names {
                assert!(elements.iter().any(|element| !element.is_directory
                    && element.name.encode_utf16().eq(name.encode_utf16())));
            }
        }
    }
    #[cfg(unix)]
    #[test]
    fn symlink_entries_are_skipped_but_a_symlink_base_is_followed() {
        use std::os::unix::fs::symlink;
        let workspace = Workspace::new();
        let real = workspace.0.join("real");
        std::fs::create_dir(&real).unwrap();
        std::fs::write(real.join("file"), b"x").unwrap();
        std::fs::create_dir(real.join("directory")).unwrap();
        symlink("file", real.join("file-link")).unwrap();
        symlink("directory", real.join("directory-link")).unwrap();
        symlink("missing", real.join("dangling-link")).unwrap();
        symlink("real", workspace.0.join("base-link")).unwrap();
        for resource in [false, true] {
            let elements = listing(workspace.uri("base-link"), resource);
            assert_eq!(elements.len(), 2);
            assert!(
                elements
                    .iter()
                    .any(|entry| entry.name == "file" && !entry.is_directory)
            );
            assert!(
                elements
                    .iter()
                    .any(|entry| entry.name == "directory" && entry.is_directory)
            );
        }
    }
    #[test]
    fn authorization_rejects_before_the_io_operation_runs() {
        let called = Cell::new(false);
        let denied = authorized(&"http://blocked/".into(), true, || {
            called.set(true);
            Ok(())
        });
        assert!(matches!(
            denied,
            Err(ModuleHostError::SecurityManagerException { .. })
        ));
        assert!(!called.get());
        let allowed = authorized(&"file:/not-opened".into(), true, || {
            called.set(true);
            Ok(())
        });
        assert!(allowed.is_ok());
        assert!(called.get());
    }
    #[test]
    fn custom_file_reader_is_not_overridden() {
        let mut reader = resource();
        reader.handle = "custom:file".into();
        let request = GlobRequest {
            id: 1,
            kind: GlobRequestKind::ListElements {
                reader: GlobReader::Resource(reader),
                uri: "file:/not-opened".into(),
            },
        };
        assert_eq!(complete(&request), None);
    }
    #[test]
    fn unpaired_utf16_file_uri_remains_explicitly_unsupported() {
        let uri = PklString::from("file:/").concat(&PklString::from_utf16(vec![0xd800]));
        let request = GlobRequest {
            id: 1,
            kind: GlobRequestKind::ListElements {
                reader: GlobReader::Resource(resource()),
                uri,
            },
        };
        assert!(matches!(
            complete(&request),
            Some(GlobCompletion::Elements(Err(GlobHostError::Failure(
                ModuleHostError::Unsupported(_)
            ))))
        ));
    }
    #[test]
    fn resource_has_element_observes_real_files_and_missing_paths() {
        let workspace = Workspace::new();
        std::fs::write(workspace.0.join("present"), b"x").unwrap();
        std::fs::create_dir(workspace.0.join("directory")).unwrap();
        for (name, expected) in [("present", true), ("directory", true), ("missing", false)] {
            let request = GlobRequest {
                id: 1,
                kind: GlobRequestKind::HasElement {
                    reader: resource(),
                    uri: workspace.uri(name),
                },
            };
            assert_eq!(
                complete(&request),
                Some(GlobCompletion::HasElement(Ok(expected)))
            );
        }
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink("present", workspace.0.join("link")).unwrap();
            std::os::unix::fs::symlink("missing", workspace.0.join("dangling")).unwrap();
            for (name, expected) in [("link", true), ("dangling", false)] {
                let request = GlobRequest {
                    id: 1,
                    kind: GlobRequestKind::HasElement {
                        reader: resource(),
                        uri: workspace.uri(name),
                    },
                };
                assert_eq!(
                    complete(&request),
                    Some(GlobCompletion::HasElement(Ok(expected)))
                );
            }
        }
    }
    #[test]
    fn custom_file_module_key_is_delegated_for_capabilities_and_listing() {
        let key = ModuleKey {
            id: ModuleKeyId(9),
            uri: "file:/not-opened".into(),
            kind: ModuleKeyKind::Host,
            handle: "custom:file".into(),
        };
        let reader = GlobReader::Module(key);
        for kind in [
            GlobRequestKind::IsGlobbable {
                reader: reader.clone(),
            },
            GlobRequestKind::IsHierarchical {
                reader: reader.clone(),
            },
            GlobRequestKind::ListElements {
                reader,
                uri: "file:/not-opened".into(),
            },
        ] {
            assert_eq!(complete(&GlobRequest { id: 1, kind }), None);
        }
    }
    #[test]
    fn module_denial_precedes_file_conversion_for_listing_and_has_element() {
        use pkl_runtime::modules::{
            ModuleCompletion, ModuleIdentity, ModuleRequest, ModuleRequestKind,
        };
        let denied = PklString::from("http:/not-a-file");
        let key = ModuleKey {
            id: ModuleKeyId(9),
            uri: denied.clone(),
            kind: ModuleKeyKind::StandardFile,
            handle: "".into(),
        };
        let request = GlobRequest {
            id: 1,
            kind: GlobRequestKind::ListElements {
                reader: GlobReader::Module(key),
                uri: denied.clone(),
            },
        };
        assert!(matches!(
            complete(&request),
            Some(GlobCompletion::Elements(Err(GlobHostError::Failure(
                ModuleHostError::SecurityManagerException { .. }
            ))))
        ));
        // An invalid original file URI would be Unsupported if conversion ran
        // before authorizing the denied element URI.
        let module = ModuleIdentity {
            original_uri: "invalid:original".into(),
            resolved_uri: "invalid:resolved".into(),
        };
        let request = ModuleRequest {
            id: 1,
            kind: ModuleRequestKind::HasElement {
                key: None,
                module,
                element_uri: denied,
            },
        };
        assert!(matches!(
            super::super::module_host::complete(&request),
            ModuleCompletion::HasElement(Err(ModuleHostError::SecurityManagerException { .. }))
        ));
    }
    #[test]
    fn directory_iteration_failure_is_unsupported_instead_of_guest_io() {
        let failure = iteration_error(std::io::Error::other("synthetic iterator failure"));
        assert!(matches!(failure, ModuleHostError::Unsupported(_)));
    }
    #[test]
    fn package_reader_and_select_resource_glob_support() {
        let package_uri: PklString = "package://example.com/birds@0.5.0#/catalog/".into();
        let select_req = GlobRequest {
            id: 1,
            kind: GlobRequestKind::SelectResource {
                id: ResourceReaderId(42),
                uri: package_uri.clone(),
            },
        };
        let Some(GlobCompletion::Resource(Ok(reader))) = complete(&select_req) else {
            panic!("SelectResource failed");
        };
        assert_eq!(reader.scheme, "package");
        assert_eq!(reader.handle, "builtin:package");

        let glob_req = GlobRequest {
            id: 2,
            kind: GlobRequestKind::IsGlobbable {
                reader: GlobReader::Resource(reader.clone()),
            },
        };
        assert_eq!(
            complete(&glob_req),
            Some(GlobCompletion::Globbable(Ok(true)))
        );

        let hier_req = GlobRequest {
            id: 3,
            kind: GlobRequestKind::IsHierarchical {
                reader: GlobReader::Resource(reader.clone()),
            },
        };
        assert_eq!(
            complete(&hier_req),
            Some(GlobCompletion::Hierarchical(Ok(true)))
        );

        let frag_req = GlobRequest {
            id: 4,
            kind: GlobRequestKind::FragmentPaths {
                reader: reader.clone(),
            },
        };
        assert_eq!(
            complete(&frag_req),
            Some(GlobCompletion::FragmentPaths(Ok(true)))
        );
    }
}
