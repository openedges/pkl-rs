//! File ResourceReader for the development original-module adapter. The engine
//! owns URI resolution, cache identity, absence handling, and Resource values.
use pkl_runtime::{
    host::ReadFailure,
    resources::{ReadResult, Resource},
    vm::ReadRequest,
};

pub fn complete(request: &ReadRequest) -> Option<Result<ReadResult, ReadFailure>> {
    let req_uri = request.uri.as_str().unwrap_or("");

    // 1. Check allowed resources if configured
    if let Some(allowed) = pkl_runtime::packages::resolver::current_allowed_resources() {
        let units: Vec<u16> = req_uri.encode_utf16().collect();
        let is_allowed = allowed.iter().any(|pattern| {
            if let Ok(re) = pkl_runtime::java_regex::Utf16Regex::from_str(pattern, "") {
                re.looking_at(&units, pkl_runtime::java_regex::Limits::default())
                    .ok()
                    .flatten()
                    .is_some()
            } else {
                req_uri.starts_with(pattern)
            }
        });
        if !is_allowed {
            return Some(Err(ReadFailure::SecurityManagerException {
                message: Some(
                    format!(
                        "Refusing to read resource `{req_uri}` because it does not match any entry in the resource allowlist (`--allowed-resources`)."
                    )
                    .into(),
                ),
            }));
        }
    }

    // 2. Handle modulepath: scheme
    if req_uri.starts_with("modulepath:") {
        if request.nullable {
            return Some(Ok(ReadResult::Missing));
        } else {
            return Some(Err(ReadFailure::Io(
                format!("Cannot find resource `{req_uri}`.").into(),
            )));
        }
    }

    // 3. Handle @dep scheme
    if req_uri.starts_with('@') {
        let empty_base = pkl_runtime::value::PklString::from("");
        let base_uri = request.module_uri.as_ref().unwrap_or(&empty_base);
        let resolved =
            match pkl_runtime::packages::resolver::resolve_import_uri(base_uri, &request.uri) {
                Ok(uri) => uri,
                Err(pkl_runtime::modules::ModuleHostError::SecurityManagerException {
                    message,
                }) => {
                    return Some(Err(ReadFailure::SecurityManagerException { message }));
                }
                Err(pkl_runtime::modules::ModuleHostError::UriSyntax { reason }) => {
                    return Some(Err(ReadFailure::UriSyntax { reason }));
                }
                Err(pkl_runtime::modules::ModuleHostError::EvalError { message, hint }) => {
                    return Some(Err(ReadFailure::EvalError { message, hint }));
                }
                Err(err) => {
                    return Some(Err(ReadFailure::Io(format!("{err:?}").into())));
                }
            };
        if let Some(local_path) =
            pkl_runtime::packages::resolver::get_local_project_file_path(&resolved)
        {
            return Some(read_local_path(&local_path, request, req_uri));
        }
        return match pkl_runtime::packages::resolver::resolve_package_resource(&resolved) {
            Ok(bytes) => Some(Ok(ReadResult::Resource(Resource {
                uri: resolved,
                bytes: bytes.into(),
            }))),
            Err(pkl_runtime::modules::ModuleHostError::NotFound) => {
                if request.nullable {
                    Some(Ok(ReadResult::Missing))
                } else {
                    Some(Err(ReadFailure::Io(pkl_runtime::host::IoFailure::new(
                        None,
                    ))))
                }
            }
            Err(pkl_runtime::modules::ModuleHostError::Io(io)) => Some(Err(ReadFailure::Io(io))),
            Err(pkl_runtime::modules::ModuleHostError::UriSyntax { reason }) => {
                Some(Err(ReadFailure::UriSyntax { reason }))
            }
            Err(pkl_runtime::modules::ModuleHostError::EvalError { message, hint }) => {
                Some(Err(ReadFailure::EvalError { message, hint }))
            }
            Err(pkl_runtime::modules::ModuleHostError::SecurityManagerException { message }) => {
                Some(Err(ReadFailure::SecurityManagerException { message }))
            }
            Err(err) => Some(Err(ReadFailure::Io(format!("{err:?}").into()))),
        };
    }

    let uri = request.resolved_uri().ok()?;
    if uri.starts_with("package:") || uri.starts_with("projectpackage:") {
        if let Some(local_path) = pkl_runtime::packages::resolver::get_local_project_file_path(&uri)
        {
            return Some(read_local_path(&local_path, request, req_uri));
        }
        return match pkl_runtime::packages::resolver::resolve_package_resource(&uri) {
            Ok(bytes) => Some(Ok(ReadResult::Resource(Resource {
                uri,
                bytes: bytes.into(),
            }))),
            Err(pkl_runtime::modules::ModuleHostError::NotFound) => {
                if request.nullable {
                    Some(Ok(ReadResult::Missing))
                } else {
                    Some(Err(ReadFailure::Io(pkl_runtime::host::IoFailure::new(
                        None,
                    ))))
                }
            }
            Err(pkl_runtime::modules::ModuleHostError::Io(io)) => Some(Err(ReadFailure::Io(io))),
            Err(pkl_runtime::modules::ModuleHostError::UriSyntax { reason }) => {
                Some(Err(ReadFailure::UriSyntax { reason }))
            }
            Err(pkl_runtime::modules::ModuleHostError::EvalError { message, hint }) => {
                Some(Err(ReadFailure::EvalError { message, hint }))
            }
            Err(pkl_runtime::modules::ModuleHostError::SecurityManagerException { message }) => {
                Some(Err(ReadFailure::SecurityManagerException { message }))
            }
            Err(err) => Some(Err(ReadFailure::Io(format!("{err:?}").into()))),
        };
    }
    // Other schemes use the shared configured policy; the adapter does not
    // convert missing capabilities into successful absence.
    let path = super::module_host::file_path(&uri).ok()?;
    Some(match std::fs::read(&path) {
        Ok(bytes) => Ok(ReadResult::Resource(Resource {
            uri,
            bytes: bytes.into(),
        })),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(ReadResult::Missing),
        Err(_) if path.is_dir() => match std::fs::read_dir(&path) {
            Ok(entries) => {
                let mut list = Vec::new();
                for entry in entries.flatten() {
                    if !entry.path().is_symlink() {
                        list.push(entry.file_name().to_string_lossy().to_string());
                    }
                }
                list.sort_by(|a, b| {
                    a.to_lowercase()
                        .cmp(&b.to_lowercase())
                        .then_with(|| a.cmp(b))
                });
                let mut text = String::new();
                for name in list {
                    text.push_str(&name);
                    text.push('\n');
                }
                Ok(ReadResult::Resource(Resource {
                    uri,
                    bytes: text.into_bytes().into(),
                }))
            }
            Err(err) => Err(ReadFailure::Io(err.to_string().into())),
        },
        Err(error) => Err(ReadFailure::Io(error.to_string().into())),
    })
}

fn read_local_path(
    path: &std::path::Path,
    request: &ReadRequest,
    req_uri: &str,
) -> Result<ReadResult, ReadFailure> {
    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let file_uri_str = format!("file:{}", canonical.to_string_lossy());
    let file_uri = pkl_runtime::value::PklString::from(file_uri_str);
    if path.is_file() {
        match std::fs::read(path) {
            Ok(bytes) => Ok(ReadResult::Resource(Resource {
                uri: file_uri,
                bytes: bytes.into(),
            })),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                if request.nullable {
                    Ok(ReadResult::Missing)
                } else {
                    Err(ReadFailure::Io(
                        format!("Cannot find resource `{req_uri}`.").into(),
                    ))
                }
            }
            Err(err) => Err(ReadFailure::Io(err.to_string().into())),
        }
    } else if path.is_dir() {
        match std::fs::read_dir(path) {
            Ok(entries) => {
                let mut list = Vec::new();
                for entry in entries.flatten() {
                    if !entry.path().is_symlink() {
                        list.push(entry.file_name().to_string_lossy().to_string());
                    }
                }
                list.sort_by(|a, b| {
                    a.to_lowercase()
                        .cmp(&b.to_lowercase())
                        .then_with(|| a.cmp(b))
                });
                let mut text = String::new();
                for name in list {
                    text.push_str(&name);
                    text.push('\n');
                }
                Ok(ReadResult::Resource(Resource {
                    uri: file_uri,
                    bytes: text.into_bytes().into(),
                }))
            }
            Err(err) => Err(ReadFailure::Io(err.to_string().into())),
        }
    } else if request.nullable {
        Ok(ReadResult::Missing)
    } else {
        Err(ReadFailure::Io(
            format!("Cannot find resource `{req_uri}`.").into(),
        ))
    }
}
