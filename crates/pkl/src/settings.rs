//! Evaluator settings, output formats, color modes, and `pkl:settings` data types
//! mirroring `org.pkl.core.{OutputFormat, evaluatorSettings.*}` and `org.pkl.core.settings.PklSettings`.

use crate::error::{PklError, Result};
pub use pkl_runtime::logging::TraceMode;
pub use pkl_runtime::packages::project::{
    DEFAULT_EDITOR_URL_SCHEME, EvaluatorColor, EvaluatorExternalReader as ExternalReaderSpec,
    EvaluatorHttpProxy as ProxySettings, EvaluatorHttpSettings as HttpSettings,
    EvaluatorSettings as PklEvaluatorSettings, PklSettings as RuntimePklSettings,
};
use std::fmt;
use std::io::IsTerminal;
use std::path::Path;

/// Built-in Pkl output formats (`org.pkl.core.OutputFormat`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum OutputFormat {
    Json,
    Jsonnet,
    Pcf,
    Plist,
    Properties,
    TextProto,
    Xml,
    Yaml,
    PklBinary,
}

impl OutputFormat {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Json => "json",
            Self::Jsonnet => "jsonnet",
            Self::Pcf => "pcf",
            Self::Plist => "plist",
            Self::Properties => "properties",
            Self::TextProto => "textproto",
            Self::Xml => "xml",
            Self::Yaml => "yaml",
            Self::PklBinary => "pkl-binary",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "json" => Some(Self::Json),
            "jsonnet" => Some(Self::Jsonnet),
            "pcf" => Some(Self::Pcf),
            "plist" => Some(Self::Plist),
            "properties" => Some(Self::Properties),
            "textproto" => Some(Self::TextProto),
            "xml" => Some(Self::Xml),
            "yaml" => Some(Self::Yaml),
            "pkl-binary" => Some(Self::PklBinary),
            _ => None,
        }
    }
}

impl fmt::Display for OutputFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// ANSI color mode mirroring `org.pkl.core.evaluatorSettings.Color`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Color {
    Never,
    #[default]
    Auto,
    Always,
}

impl Color {
    /// Resolves whether ANSI colors should be enabled (`Color.hasColor()`).
    ///
    /// Mirrors Apple Pkl 0.32.0 `Color.kt`: `Auto` checks interactive console
    /// (`stdin.is_terminal() && stdout.is_terminal()`, matching `System.console() != null`).
    pub fn has_color(self) -> bool {
        match self {
            Self::Never => false,
            Self::Always => true,
            Self::Auto => std::io::stdin().is_terminal() && std::io::stdout().is_terminal(),
        }
    }

    pub fn parse(s: &str) -> Result<Self> {
        match s.to_ascii_lowercase().as_str() {
            "never" => Ok(Self::Never),
            "auto" => Ok(Self::Auto),
            "always" => Ok(Self::Always),
            other => Err(PklError::invalid_argument(format!(
                "Invalid color `{other}`. Expected one of: never, auto, always."
            ))),
        }
    }
}

impl From<EvaluatorColor> for Color {
    fn from(c: EvaluatorColor) -> Self {
        match c {
            EvaluatorColor::Never => Self::Never,
            EvaluatorColor::Auto => Self::Auto,
            EvaluatorColor::Always => Self::Always,
        }
    }
}

/// Editor link scheme (`org.pkl.core.settings.PklSettings.Editor`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Editor {
    pub url_scheme: String,
}

impl Editor {
    pub fn system() -> Self {
        Self {
            url_scheme: DEFAULT_EDITOR_URL_SCHEME.to_string(),
        }
    }

    pub fn new(url_scheme: impl Into<String>) -> Self {
        Self {
            url_scheme: url_scheme.into(),
        }
    }
}

impl Default for Editor {
    fn default() -> Self {
        Self::system()
    }
}

/// User settings loaded from `settings.pkl` (`org.pkl.core.settings.PklSettings`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PklSettings {
    pub editor: Editor,
    pub http: Option<HttpSettings>,
}

impl PklSettings {
    /// Loads `PklSettings` from a `settings.pkl` module source (e.g. `pkl:settings` or file path).
    pub fn load(source: &crate::ModuleSource) -> Result<Self> {
        let mut evaluator = crate::EvaluatorBuilder::preconfigured()
            .with_module_cache_dir(None)
            .build()?;
        let bytes = evaluator.evaluate_expression_pkl_binary(source, "output.value")?;
        let decoded = pkl_runtime::pkl_binary_decode::decode_lossless(&bytes)
            .map_err(|err| PklError::decode(format!("Failed to decode settings: {err}")))?;
        let parsed = RuntimePklSettings::from_decoded_value(&decoded, source.get_uri())
            .map_err(|err| PklError::configuration(format!("{err:?}")))?;
        Ok(Self {
            editor: Editor::new(parsed.editor_url_scheme),
            http: parsed.http,
        })
    }

    /// Loads `PklSettings` from the default `~/.pkl/settings.pkl` if present, otherwise `pkl:settings`.
    pub fn load_from_pkl_home_dir() -> Result<Self> {
        if let Some(home) = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")) {
            let path = Path::new(&home).join(".pkl").join("settings.pkl");
            if path.is_file() {
                return Self::load(&crate::ModuleSource::path(&path));
            }
        }
        Ok(Self::default())
    }
}
