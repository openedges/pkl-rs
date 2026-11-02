//! Typed glob reader effects. Readers enumerate names, never evaluated values.
//! The VM owns traversal, ordering, pattern matching and lazy value loading.
use crate::source::SourceText;

use crate::{
    modules::{ModuleHostError, ModuleKey},
    value::PklString,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ResourceReaderId(pub u64);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResourceReader {
    pub id: ResourceReaderId,
    pub scheme: PklString,
    pub handle: PklString,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GlobReader {
    Module(ModuleKey),
    Resource(ResourceReader),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PathElement {
    pub name: PklString,
    pub is_directory: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GlobRequest {
    pub id: u64,
    pub kind: GlobRequestKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GlobRequestKind {
    SelectResource {
        id: ResourceReaderId,
        uri: PklString,
    },
    IsGlobbable {
        reader: GlobReader,
    },
    IsHierarchical {
        reader: GlobReader,
    },
    FragmentPaths {
        reader: ResourceReader,
    },
    HasElement {
        reader: ResourceReader,
        uri: PklString,
    },
    ListElements {
        reader: GlobReader,
        uri: PklString,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GlobHostError {
    ReaderNotFound,
    Failure(ModuleHostError),
}

impl From<ModuleHostError> for GlobHostError {
    fn from(failure: ModuleHostError) -> Self {
        Self::Failure(failure)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GlobCompletion {
    Resource(Result<ResourceReader, GlobHostError>),
    Globbable(Result<bool, GlobHostError>),
    Hierarchical(Result<bool, GlobHostError>),
    FragmentPaths(Result<bool, GlobHostError>),
    HasElement(Result<bool, GlobHostError>),
    Elements(Result<Vec<PathElement>, GlobHostError>),
}

/// Build a failure with exactly the requested completion shape. Admission and
/// phase validation still belong to `Vm::complete_glob`.
pub fn failure_completion(request: &GlobRequest, failure: GlobHostError) -> GlobCompletion {
    match request.kind {
        GlobRequestKind::SelectResource { .. } => GlobCompletion::Resource(Err(failure)),
        GlobRequestKind::IsGlobbable { .. } => GlobCompletion::Globbable(Err(failure)),
        GlobRequestKind::IsHierarchical { .. } => GlobCompletion::Hierarchical(Err(failure)),
        GlobRequestKind::FragmentPaths { .. } => GlobCompletion::FragmentPaths(Err(failure)),
        GlobRequestKind::HasElement { .. } => GlobCompletion::HasElement(Err(failure)),
        GlobRequestKind::ListElements { .. } => GlobCompletion::Elements(Err(failure)),
    }
}

pub fn unsupported_completion(request: &GlobRequest) -> GlobCompletion {
    failure_completion(
        request,
        ModuleHostError::Unsupported(
            "No glob reader is configured for this Host operation.".into(),
        )
        .into(),
    )
}

/// Fixed standard-policy profile, with no filesystem or process access. Custom
/// Hosts apply their own policy at each typed resolve/list/existence operation.
pub fn check_default_resolve(
    uri: impl Into<PklString>,
    resource: bool,
) -> Result<(), ModuleHostError> {
    let uri = uri.into();
    let text = SourceText::from(uri.clone());
    let scheme = text
        .entire()
        .find(":")
        .and_then(|end| text.entire().range(..end).as_str());
    let allowed = if resource {
        matches!(
            scheme,
            Some("env" | "prop" | "file" | "modulepath" | "package" | "projectpackage" | "https")
        )
    } else {
        matches!(
            scheme,
            Some("repl" | "file" | "modulepath" | "package" | "projectpackage" | "https" | "pkl")
        ) || uri.starts_with("jar:file:")
            || uri.starts_with("jar:nested:")
    };
    if allowed {
        Ok(())
    } else {
        let (verb, noun, option) = if resource {
            ("read", "resource", "resources")
        } else {
            ("load", "module", "modules")
        };
        Err(ModuleHostError::SecurityManagerException { message: Some(PklString::from(format!("Refusing to {verb} {noun} `")).concat(&uri)
            .concat(&format!("` because it does not match any entry in the {noun} allowlist (`--allowed-{option}`).").into())) })
    }
}

/// Explicit builtin identity. A custom reader for the same scheme is not
/// silently replaced by the configured reader.
pub fn is_default_reader(reader: &ResourceReader) -> bool {
    matches!(
        reader.scheme.as_str(),
        Some(
            "env"
                | "prop"
                | "file"
                | "http"
                | "https"
                | "modulepath"
                | "package"
                | "projectpackage"
        )
    ) && reader.handle == PklString::from("builtin:").concat(&reader.scheme)
}

/// Return `None` for unavailable I/O, preserving the opportunity for an external
/// Host to supply it. Capabilities are queried separately and never eagerly.
pub fn configured_completion(
    request: &GlobRequest,
    properties: &[(String, String)],
    environment: &[(String, String)],
) -> Option<GlobCompletion> {
    match &request.kind {
        GlobRequestKind::SelectResource { id, uri } => {
            let text = SourceText::from(uri.clone());
            let scheme = text
                .entire()
                .find(":")
                .and_then(|end| text.entire().range(..end).as_str());
            Some(GlobCompletion::Resource(
                if matches!(
                    scheme,
                    Some(
                        "env"
                            | "prop"
                            | "file"
                            | "http"
                            | "https"
                            | "modulepath"
                            | "package"
                            | "projectpackage"
                    )
                ) {
                    Ok(ResourceReader {
                        id: *id,
                        scheme: scheme.expect("configured ASCII scheme").into(),
                        handle: PklString::from("builtin:")
                            .concat(&scheme.expect("configured ASCII scheme").into()),
                    })
                } else {
                    Err(GlobHostError::ReaderNotFound)
                },
            ))
        }
        GlobRequestKind::IsGlobbable {
            reader: GlobReader::Resource(reader),
        } if is_default_reader(reader) && reader.scheme != "modulepath" => {
            Some(GlobCompletion::Globbable(Ok(!matches!(
                reader.scheme.as_str(),
                Some("http" | "https")
            ))))
        }
        GlobRequestKind::IsHierarchical {
            reader: GlobReader::Resource(reader),
        } if is_default_reader(reader) => Some(GlobCompletion::Hierarchical(Ok(!matches!(
            reader.scheme.as_str(),
            Some("env" | "prop")
        )))),
        GlobRequestKind::FragmentPaths { reader } if is_default_reader(reader) => {
            Some(GlobCompletion::FragmentPaths(Ok(matches!(
                reader.scheme.as_str(),
                Some("package" | "projectpackage")
            ))))
        }
        GlobRequestKind::IsGlobbable {
            reader: GlobReader::Module(key),
        } if key.kind == crate::modules::ModuleKeyKind::StandardLibrary
            || matches!(
                key.kind,
                crate::modules::ModuleKeyKind::StandardHttp
                    | crate::modules::ModuleKeyKind::StandardGenericUrl
            )
            || (key.handle == "builtin:url" && {
                let text = SourceText::from(key.uri.clone());
                text.entire()
                    .find(":")
                    .and_then(|end| text.entire().range(..end).as_str())
                    .is_some_and(|scheme| {
                        scheme.eq_ignore_ascii_case("http") || scheme.eq_ignore_ascii_case("https")
                    })
            }) =>
        {
            Some(GlobCompletion::Globbable(Ok(false)))
        }
        GlobRequestKind::ListElements {
            reader: GlobReader::Resource(reader),
            uri,
        } if is_default_reader(reader)
            && matches!(reader.scheme.as_str(), Some("env" | "prop")) =>
        {
            Some(GlobCompletion::Elements(
                check_default_resolve(uri, true)
                    .map(|()| {
                        let entries = if reader.scheme == "prop" {
                            properties
                        } else {
                            environment
                        };
                        let mut names = std::collections::BTreeSet::new();
                        entries
                            .iter()
                            .filter(|&(name, _)| names.insert(name.as_str()))
                            .map(|(name, _)| PathElement {
                                name: name.as_str().into(),
                                is_directory: false,
                            })
                            .collect()
                    })
                    .map_err(Into::into),
            ))
        }
        _ => None,
    }
}

pub fn configured_or_unsupported_completion(
    request: &GlobRequest,
    properties: &[(String, String)],
    environment: &[(String, String)],
) -> GlobCompletion {
    configured_completion(request, properties, environment)
        .unwrap_or_else(|| unsupported_completion(request))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reader(scheme: &str) -> ResourceReader {
        ResourceReader {
            id: ResourceReaderId(u64::MAX),
            scheme: scheme.into(),
            handle: format!("builtin:{scheme}").into(),
        }
    }

    #[test]
    fn configured_opaque_enumeration_has_unique_names_without_reading_values() {
        let request = GlobRequest {
            id: 7,
            kind: GlobRequestKind::ListElements {
                reader: GlobReader::Resource(reader("prop")),
                uri: "prop:ignored/search/path".into(),
            },
        };
        let properties = vec![
            ("日本語".into(), "first".into()),
            ("z".into(), "value".into()),
            ("日本語".into(), "last".into()),
        ];
        let Some(GlobCompletion::Elements(Ok(elements))) =
            configured_completion(&request, &properties, &[])
        else {
            panic!("configured property reader")
        };
        assert_eq!(
            elements,
            vec![
                PathElement {
                    name: "日本語".into(),
                    is_directory: false
                },
                PathElement {
                    name: "z".into(),
                    is_directory: false
                }
            ]
        );
        let request = GlobRequest {
            id: 8,
            kind: GlobRequestKind::ListElements {
                reader: GlobReader::Resource(reader("env")),
                uri: "env:another/path".into(),
            },
        };
        assert_eq!(
            configured_completion(&request, &[], &[("ONLY_CONFIGURED".into(), "value".into())]),
            Some(GlobCompletion::Elements(Ok(vec![PathElement {
                name: "ONLY_CONFIGURED".into(),
                is_directory: false
            }])))
        );
    }

    #[test]
    fn builtin_http_reader_selection_and_capability_need_no_io_or_allowlist_check() {
        for scheme in ["http", "https"] {
            let request = GlobRequest {
                id: 9,
                kind: GlobRequestKind::SelectResource {
                    id: ResourceReaderId(u64::MAX),
                    uri: format!("{scheme}://example.invalid/**").into(),
                },
            };
            assert_eq!(
                configured_completion(&request, &[], &[]),
                Some(GlobCompletion::Resource(Ok(reader(scheme))))
            );
            let request = GlobRequest {
                id: 10,
                kind: GlobRequestKind::IsGlobbable {
                    reader: GlobReader::Resource(reader(scheme)),
                },
            };
            assert_eq!(
                configured_completion(&request, &[], &[]),
                Some(GlobCompletion::Globbable(Ok(false)))
            );
        }
    }

    #[test]
    fn unavailable_file_io_and_custom_readers_never_become_empty_successes() {
        let request = GlobRequest {
            id: 11,
            kind: GlobRequestKind::ListElements {
                reader: GlobReader::Resource(reader("file")),
                uri: "file:/missing/".into(),
            },
        };
        assert!(configured_completion(&request, &[], &[]).is_none());
        assert!(matches!(
            configured_or_unsupported_completion(&request, &[], &[]),
            GlobCompletion::Elements(Err(GlobHostError::Failure(ModuleHostError::Unsupported(_))))
        ));
        let mut custom = reader("prop");
        custom.handle = "custom:property-reader".into();
        let request = GlobRequest {
            id: 12,
            kind: GlobRequestKind::ListElements {
                reader: GlobReader::Resource(custom),
                uri: "prop:*".into(),
            },
        };
        assert!(configured_completion(&request, &[], &[]).is_none());
        let request = GlobRequest {
            id: 13,
            kind: GlobRequestKind::SelectResource {
                id: ResourceReaderId(2),
                uri: "PROP:*".into(),
            },
        };
        assert_eq!(
            configured_completion(&request, &[], &[]),
            Some(GlobCompletion::Resource(Err(GlobHostError::ReaderNotFound)))
        );
    }

    #[test]
    fn fixed_resolve_profile_distinguishes_module_and_resource_policy() {
        assert_eq!(check_default_resolve("file:/tmp/", false), Ok(()));
        assert_eq!(check_default_resolve("prop:anything", true), Ok(()));
        assert!(check_default_resolve("jar:file:/tmp/archive.jar!/", false).is_ok());
        assert!(check_default_resolve("jar:https://example.invalid/a.jar!/", false).is_err());
        assert!(
            matches!(check_default_resolve("http://example.invalid/", false), Err(ModuleHostError::SecurityManagerException { message: Some(message) }) if message == "Refusing to load module `http://example.invalid/` because it does not match any entry in the module allowlist (`--allowed-modules`).")
        );
    }
}
