//! Filesystem capability for development examples. URI preparation, module
//! caches, parsing, and evaluation belong to the shared runtime.
use pkl_runtime::modules::{
    ModuleCompletion, ModuleHostError, ModuleRequest, ModuleRequestKind, ModuleSource,
    ResolvedModule,
};
use std::path::{Path, PathBuf};

pub fn complete(request: &ModuleRequest) -> ModuleCompletion {
    if let Some(completion) = pkl_runtime::modules::default_completion(request) {
        return completion;
    }
    match &request.kind {
        ModuleRequestKind::CheckImport { .. } => {
            unreachable!("default policy handles import admission")
        }
        ModuleRequestKind::Capabilities { module } => {
            ModuleCompletion::Capabilities(file_path(&module.original_uri).map(|_| {
                pkl_runtime::modules::ModuleCapabilities {
                    local: true,
                    hierarchical: true,
                    fragment_paths: false,
                }
            }))
        }
        ModuleRequestKind::HasElement {
            module,
            element_uri,
        } => ModuleCompletion::HasElement(
            file_path(&module.original_uri)
                .and_then(|_| file_path(element_uri))
                .map(|path| path.exists()),
        ),
        ModuleRequestKind::Resolve {
            import_uri,
            original_uri,
            ..
        } => ModuleCompletion::Resolved(resolve(import_uri, original_uri)),
        ModuleRequestKind::LoadSource { resolved } => ModuleCompletion::Source(load(resolved)),
    }
}

fn resolve(import_uri: &str, original_uri: &str) -> Result<ResolvedModule, ModuleHostError> {
    let path = file_path(import_uri)?.canonicalize().map_err(io_error)?;
    let uri = file_uri(&path)?;
    Ok(ResolvedModule {
        original_uri: original_uri.into(),
        resolved_uri: uri.into(),
        // VmUtils.loadSource retains the original ModuleKey URI for source
        // diagnostics even when resolution canonicalizes a symlink alias.
        source_uri: original_uri.into(),
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
    } else {
        ModuleHostError::Io(error.to_string().into())
    }
}

pub(super) fn file_path(uri: &str) -> Result<PathBuf, ModuleHostError> {
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
    let text = String::from_utf8(decoded).map_err(|_| unsupported())?;
    if text.contains('\0') {
        return Err(unsupported());
    }
    Ok(PathBuf::from(text))
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
