//! Module input specifications mirroring `org.pkl.core.ModuleSource`.

use pkl_host_native::path_to_file_uri;
use pkl_runtime::modules::ModuleSourceSpec;
use pkl_runtime::source::SourceText;
use pkl_runtime::value::PklString;
use std::path::Path;

/// A Pkl module source passed to [`crate::Evaluator`], mirroring `org.pkl.core.ModuleSource`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModuleSource {
    uri: String,
    contents: Option<String>,
}

impl ModuleSource {
    /// Creates a module source backed by a filesystem path (converted to an absolute `file://` URI).
    pub fn path(path: impl AsRef<Path>) -> Self {
        let path = path.as_ref();
        let abs = if path.is_absolute() {
            path.to_path_buf()
        } else {
            std::env::current_dir()
                .map(|cwd| cwd.join(path))
                .unwrap_or_else(|_| path.to_path_buf())
        };
        let normalized = normalize_lexical_path(&abs);
        let uri = path_to_file_uri(&normalized).to_java_utf8();
        Self {
            uri,
            contents: None,
        }
    }

    /// Creates a module source backed by an arbitrary URI (e.g. `file:///...`, `https://...`,
    /// `package://...`, `modulepath:/...`, `@dep/...`, `custom:/...`).
    pub fn uri(uri: impl Into<String>) -> Self {
        Self {
            uri: uri.into(),
            contents: None,
        }
    }

    /// Creates a module source from inline Pkl source text with the default `repl:text` URI.
    pub fn text(contents: impl Into<String>) -> Self {
        Self {
            uri: "repl:text".to_string(),
            contents: Some(contents.into()),
        }
    }

    /// Creates a module source from an explicit URI and inline Pkl source text.
    pub fn create(uri: impl Into<String>, contents: impl Into<String>) -> Self {
        Self {
            uri: uri.into(),
            contents: Some(contents.into()),
        }
    }

    /// Creates a module source backed by a `modulepath:` URI (ensuring leading `/`).
    pub fn module_path(path: impl AsRef<str>) -> Self {
        let p = path.as_ref();
        let uri = if p.starts_with("modulepath:") {
            p.to_string()
        } else if p.starts_with('/') {
            format!("modulepath:{p}")
        } else {
            format!("modulepath:/{p}")
        };
        Self {
            uri,
            contents: None,
        }
    }

    /// Returns the module's URI string.
    pub fn get_uri(&self) -> &str {
        &self.uri
    }

    /// Returns inline source contents if provided.
    pub fn contents(&self) -> Option<&str> {
        self.contents.as_deref()
    }

    pub(crate) fn to_runtime_spec(&self) -> ModuleSourceSpec {
        ModuleSourceSpec {
            uri: PklString::from(self.uri.as_str()),
            contents: self.contents.as_deref().map(SourceText::from),
        }
    }
}

fn normalize_lexical_path(path: &Path) -> std::path::PathBuf {
    use std::path::Component;
    let mut out = std::path::PathBuf::new();
    for comp in path.components() {
        match comp {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}
