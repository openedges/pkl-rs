//! Evaluator settings, output formats, color modes, and `pkl:settings` data types
//! mirroring `org.pkl.core.{OutputFormat, evaluatorSettings.*}` and `org.pkl.core.settings.PklSettings`.

use crate::error::{PklError, Result, format_module_host_error};
pub use pkl_runtime::logging::TraceMode;
pub use pkl_runtime::packages::project::{
    DEFAULT_EDITOR_URL_SCHEME, EvaluatorColor, EvaluatorExternalReader as ExternalReaderSpec,
    EvaluatorHttpProxy as ProxySettings, EvaluatorHttpSettings as HttpSettings,
    EvaluatorSettings as PklEvaluatorSettings, PklSettings as RuntimePklSettings,
};
use std::fmt;
use std::io::IsTerminal;
use std::path::Path;
use std::str::FromStr;

/// Built-in Pkl output formats (`org.pkl.core.OutputFormat`).
#[non_exhaustive]
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

impl FromStr for OutputFormat {
    type Err = PklError;

    fn from_str(s: &str) -> Result<Self> {
        Self::parse(s).ok_or_else(|| {
            PklError::invalid_argument(format!(
                "Invalid output format `{s}`. Expected one of: json, jsonnet, pcf, plist, properties, textproto, xml, yaml, pkl-binary."
            ))
        })
    }
}

/// ANSI color mode mirroring `org.pkl.core.evaluatorSettings.Color`.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Color {
    Never,
    #[default]
    Auto,
    Always,
}

impl Color {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Never => "never",
            Self::Auto => "auto",
            Self::Always => "always",
        }
    }

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

impl fmt::Display for Color {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Color {
    type Err = PklError;

    fn from_str(s: &str) -> Result<Self> {
        Self::parse(s)
    }
}

impl From<EvaluatorColor> for Color {
    fn from(c: EvaluatorColor) -> Self {
        match c {
            EvaluatorColor::Never => Self::Never,
            EvaluatorColor::Auto => Self::Auto,
            EvaluatorColor::Always => Self::Always,
            _ => Self::Auto,
        }
    }
}

/// Editor link scheme (`org.pkl.core.settings.PklSettings.Editor`).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
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
            .with_allowed_modules(
                pkl_runtime::packages::project::SETTINGS_ALLOWED_MODULES
                    .iter()
                    .map(|s| (*s).to_string())
                    .collect(),
            )
            .with_allowed_resources(
                pkl_runtime::packages::project::SETTINGS_ALLOWED_RESOURCES
                    .iter()
                    .map(|s| (*s).to_string())
                    .collect(),
            )
            .with_module_cache_dir(None)
            .build()?;
        let bytes = evaluator.evaluate_expression_pkl_binary(source, "output.value")?;
        let decoded = pkl_runtime::pkl_binary_decode::decode_lossless(&bytes)
            .map_err(|err| PklError::decode(format!("Failed to decode settings: {err}")))?;
        let parsed = RuntimePklSettings::from_decoded_value(&decoded, source.get_uri())
            .map_err(|err| PklError::configuration(format_module_host_error(&err)))?;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_format_and_color_display_and_from_str() {
        for fmt in [
            OutputFormat::Json,
            OutputFormat::Jsonnet,
            OutputFormat::Pcf,
            OutputFormat::Plist,
            OutputFormat::Properties,
            OutputFormat::TextProto,
            OutputFormat::Xml,
            OutputFormat::Yaml,
            OutputFormat::PklBinary,
        ] {
            let parsed: OutputFormat = fmt.to_string().parse().unwrap();
            assert_eq!(parsed, fmt);
        }
        assert!("unknown".parse::<OutputFormat>().is_err());

        for color in [Color::Never, Color::Auto, Color::Always] {
            let parsed: Color = color.to_string().parse().unwrap();
            assert_eq!(parsed, color);
        }
        assert_eq!("ALWAYS".parse::<Color>().unwrap(), Color::Always);
        assert!("invalid".parse::<Color>().is_err());
    }

    #[test]
    fn pkl_settings_load_blocks_http_import() {
        let src = crate::ModuleSource::text(
            "amends \"pkl:settings\"\nimport \"http://127.0.0.1:9/evil.pkl\" as evil\n",
        );
        let err = PklSettings::load(&src).expect_err("settings.pkl must not allow http: imports");
        assert_eq!(err.kind(), crate::PklErrorKind::Security, "err={err:?}");
    }
}
