//! Filesystem capability for development examples. URI preparation, module
//! caches, parsing, and evaluation belong to the shared runtime.
use pkl_runtime::modules::{
    ModuleCompletion, ModuleHostError, ModuleRequest, ModuleRequestKind, ModuleSource,
    ResolvedModule,
};
use std::path::{Path, PathBuf};

pub fn complete(request: &ModuleRequest) -> ModuleCompletion {
    match &request.kind {
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
    fn unsupported_uri_forms_do_not_turn_into_missing_files() {
        for uri in [
            "https://example.test/a.pkl",
            "file://server/a.pkl",
            "file:relative.pkl",
            "file:/a.pkl?x",
            "file:/a.pkl#x",
            "file:/bad%00.pkl",
        ] {
            assert!(matches!(
                file_path(uri),
                Err(ModuleHostError::Unsupported(_))
            ));
        }
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
                    ModuleRequestKind::Resolve { .. } => "resolve",
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
        assert_eq!(phases, ["resolve", "source"]);
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
                    ModuleRequestKind::Resolve { .. } => "resolve",
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
        assert_eq!(phases, ["resolve", "source", "resolve"]);
    }
}
