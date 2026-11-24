//! Pure `PklProject`, `PklSettings`, `ProjectDeps` lockfile, `SettingsPrecedence`,
//! project discovery, and `ProjectDependenciesManager` models.
//!
//! Filesystem reading and process execution are delegated to the Host adapter outside `pkl-runtime`.
use crate::modules::ModuleHostError;
use crate::packages::metadata::{
    Issue, JsonPrettyWriter, MetadataAnnotation, MetadataClassInfo, MetadataValue, Node,
    format_error, parse_json,
};
use crate::packages::{CanonicalPackageUri, PackageUri, PackageVersion};
use crate::pkl_binary_decode::{DecodedObject, DecodedValue};
use crate::value::PklString;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Current `PklProject.deps.json` schema version (`ProjectDeps.java:46`).
pub const PROJECT_DEPS_SCHEMA_VERSION: i32 = 1;

pub const PKL_PROJECT_FILENAME: &str = "PklProject";
pub const PKL_PROJECT_DEPS_FILENAME: &str = "PklProject.deps.json";

/// Default editor URL scheme (`PklSettings.Editor.SYSTEM`, `PklSettings.java:103`).
pub const DEFAULT_EDITOR_URL_SCHEME: &str = "%{url}, line %{line}";

/// Default CLI allowed module URI patterns (`SecurityManagers.defaultAllowedModules`).
pub const DEFAULT_ALLOWED_MODULES: &[&str] = &[
    "pkl:",
    "repl:",
    "file:",
    "modulepath:",
    "https:",
    "jar:file:",
    "jar:nested:",
    "package:",
    "projectpackage:",
];

/// Default CLI allowed resource URI patterns (`SecurityManagers.defaultAllowedResources`).
pub const DEFAULT_ALLOWED_RESOURCES: &[&str] = &[
    "env:",
    "prop:",
    "file:",
    "modulepath:",
    "package:",
    "projectpackage:",
    "https:",
];

/// Settings loader restricted allowlists (`PklSettings.java:38-42`).
pub const SETTINGS_ALLOWED_MODULES: &[&str] = &["pkl:", "file:"];
pub const SETTINGS_ALLOWED_RESOURCES: &[&str] = &["env:", "file:"];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedProjectDep {
    pub dep_type: String, // "local" or "remote"
    pub uri: PklString,   // "projectpackage://..."
    pub path: Option<String>,
    pub sha256: Option<PklString>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EvaluatorColor {
    Never,
    Auto,
    Always,
}

impl EvaluatorColor {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "never" => Some(Self::Never),
            "auto" => Some(Self::Auto),
            "always" => Some(Self::Always),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Never => "never",
            Self::Auto => "auto",
            Self::Always => "always",
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EvaluatorHttpProxy {
    pub address: Option<String>,
    pub no_proxy: Option<Vec<String>>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EvaluatorHttpSettings {
    pub proxy: Option<EvaluatorHttpProxy>,
    pub rewrites: Option<BTreeMap<String, String>>,
}

/// External reader configuration (`PklEvaluatorSettings.ExternalReader`), unified with
/// [`crate::external_reader::ExternalReaderConfig`].
pub type EvaluatorExternalReader = crate::external_reader::ExternalReaderConfig;

/// Complete representation of `pkl:EvaluatorSettings` (`PklEvaluatorSettings.java:41-55`).
///
/// Retains existing M2 fields (`external_properties`, `env`, etc.) while adding
/// explicit presence flags (`env_is_set`, `external_properties_is_set`) and the
/// remaining M4 settings (`color`, `module_path`, `timeout_nanos`, `http`,
/// `external_module_readers`, `external_resource_readers`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EvaluatorSettings {
    pub external_properties: Vec<(String, String)>,
    pub env: Vec<(String, String)>,
    pub allowed_modules: Option<Vec<String>>,
    pub allowed_resources: Option<Vec<String>>,
    pub no_cache: Option<bool>,
    pub module_cache_dir: Option<PathBuf>,
    pub root_dir: Option<PathBuf>,
    pub trace_mode: Option<String>,
    /// True when `env` was non-null in `PklProject` (distinguishes `env {}` from `env = null`).
    pub env_is_set: bool,
    /// True when `externalProperties` was non-null in `PklProject`.
    pub external_properties_is_set: bool,
    pub color: Option<EvaluatorColor>,
    pub module_path: Option<Vec<PathBuf>>,
    /// Timeout duration in nanoseconds (exact integer precision matching Pkl `Duration`).
    pub timeout_nanos: Option<u64>,
    pub http: Option<EvaluatorHttpSettings>,
    pub external_module_readers: Option<BTreeMap<String, EvaluatorExternalReader>>,
    pub external_resource_readers: Option<BTreeMap<String, EvaluatorExternalReader>>,
}

/// Parsed `pkl:settings` (`~/.pkl/settings.pkl` or `--settings <uri>`, `PklSettings.java:37-134`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PklSettings {
    pub editor_url_scheme: String,
    pub http: Option<EvaluatorHttpSettings>,
}

impl Default for PklSettings {
    fn default() -> Self {
        Self {
            editor_url_scheme: DEFAULT_EDITOR_URL_SCHEME.to_string(),
            http: None,
        }
    }
}

impl PklSettings {
    /// Extracts `PklSettings` from a `pkl-binary`-decoded `output.value`, matching
    /// `PklSettings.parseSettings` (`PklSettings.java:78-88`).
    pub fn from_decoded_value(
        value: &DecodedValue,
        location_uri: &str,
    ) -> Result<Self, ModuleHostError> {
        let DecodedValue::Object(obj) = value else {
            return Err(ModuleHostError::EvalError {
                message: format!("Invalid settings file `{location_uri}`.").into(),
                hint: None,
            });
        };
        let editor_scheme = obj
            .get_property("editor")
            .and_then(|v| match v {
                DecodedValue::Object(editor_obj) => editor_obj.get_property("urlScheme"),
                _ => None,
            })
            .and_then(|v| match v {
                DecodedValue::String(s) => s.as_str().map(str::to_string),
                _ => None,
            })
            .ok_or_else(|| ModuleHostError::EvalError {
                message: format!("Invalid settings file `{location_uri}`.").into(),
                hint: None,
            })?;
        let http = obj
            .get_property("http")
            .and_then(parse_decoded_http_settings);
        Ok(Self {
            editor_url_scheme: editor_scheme,
            http,
        })
    }
}

/// Complete `Project.Package` specification (`Package.java:27-42`, `Project.pkl:260-418`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PackageSpec {
    pub name: String,
    pub base_uri: String,
    pub version: PackageVersion,
    pub uri: PackageUri,
    pub package_zip_url: String,
    pub description: Option<String>,
    pub authors: Vec<String>,
    pub website: Option<String>,
    pub documentation: Option<String>,
    pub source_code: Option<String>,
    pub source_code_url_scheme: Option<String>,
    pub license: Option<String>,
    pub license_text: Option<String>,
    pub issue_tracker: Option<String>,
    pub api_tests: Vec<String>,
    pub exclude: Vec<String>,
}

/// Remote dependency entry declared in `PklProject`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeclaredRemoteDep {
    pub name: String,
    pub package_uri: PackageUri,
    /// Optional checksum explicitly declared in `PklProject` (`RemoteDependency.checksums`).
    pub sha256: Option<PklString>,
}

/// Recursive dependency declaration tree (`DeclaredDependencies.java:24-28`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeclaredDependenciesTree {
    pub project_file_uri: String,
    pub package_uri: Option<PackageUri>,
    /// Preserves declared order for deterministic lockfile serialization.
    pub remote_dependencies: Vec<DeclaredRemoteDep>,
    /// Local dependencies keyed by declared name (preserves declared order).
    pub local_dependencies: Vec<(String, DeclaredDependenciesTree)>,
}

/// Full `PklProject` representation matching `org.pkl.core.project.Project` (`Project.java:61-70`).
#[derive(Clone, Debug, PartialEq)]
pub struct Project {
    pub project_file_uri: String,
    pub project_base_uri: String,
    pub project_dir: PathBuf,
    pub package: Option<PackageSpec>,
    pub declared_dependencies: DeclaredDependenciesTree,
    pub evaluator_settings: Option<EvaluatorSettings>,
    pub resolved_evaluator_settings: Option<EvaluatorSettings>,
    pub tests: Vec<String>,
    pub local_project_dependencies: Vec<(String, Project)>,
    pub annotations: Vec<MetadataAnnotation>,
}

impl Project {
    /// Extracts a complete `Project` from a `pkl-binary`-decoded `output.value`,
    /// matching `Project.parseProject` (`Project.java:251-355`).
    pub fn from_decoded_value(value: &DecodedValue) -> Result<Self, ModuleHostError> {
        let DecodedValue::Object(obj) = value else {
            return Err(ModuleHostError::EvalError {
                message: "Expected `pkl.Project` object.".into(),
                hint: None,
            });
        };
        parse_decoded_project(obj)
    }

    /// Converts to the legacy M2 `PklProject` summary struct used by existing probes/tests.
    pub fn to_legacy_pkl_project(&self) -> PklProject {
        let mut declared = BTreeMap::new();
        for remote in &self.declared_dependencies.remote_dependencies {
            declared.insert(
                remote.name.clone(),
                DeclaredDep {
                    name: remote.name.clone(),
                    uri: remote.package_uri.uri.as_str().unwrap_or("").to_string(),
                    is_local: false,
                    local_package_uri: None,
                },
            );
        }
        for (name, local) in &self.declared_dependencies.local_dependencies {
            declared.insert(
                name.clone(),
                DeclaredDep {
                    name: name.clone(),
                    uri: local.project_file_uri.clone(),
                    is_local: true,
                    local_package_uri: local
                        .package_uri
                        .as_ref()
                        .and_then(|u| u.uri.as_str().map(str::to_string)),
                },
            );
        }
        PklProject {
            project_dir: self.project_dir.clone(),
            dependencies: BTreeMap::new(),
            declared_dependencies: declared,
            deps_loaded: false,
        }
    }
}

/// Legacy M2 declared-dependency summary struct retained for existing callers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeclaredDep {
    pub name: String,
    pub uri: String,
    pub is_local: bool,
    pub local_package_uri: Option<String>,
}

/// Legacy M2 `PklProject` struct retained for existing probes and M2 tests.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PklProject {
    pub project_dir: PathBuf,
    pub dependencies: BTreeMap<String, ResolvedProjectDep>,
    pub declared_dependencies: BTreeMap<String, DeclaredDep>,
    pub deps_loaded: bool,
}

/// Strictly typed `PklProject.deps.json` dependency entry (`ProjectDeps.java`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProjectDepEntry {
    Remote {
        uri: PackageUri,
        sha256: PklString,
    },
    Local {
        uri: PackageUri,
        /// Relative path using forward slashes (`/`) from the root project directory.
        path: String,
    },
}

impl ProjectDepEntry {
    pub fn package_uri(&self) -> &PackageUri {
        match self {
            Self::Remote { uri, .. } | Self::Local { uri, .. } => uri,
        }
    }

    pub fn version(&self) -> &PackageVersion {
        &self.package_uri().version
    }

    pub fn is_local(&self) -> bool {
        matches!(self, Self::Local { .. })
    }

    pub fn to_legacy_resolved(&self) -> ResolvedProjectDep {
        match self {
            Self::Remote { uri, sha256 } => ResolvedProjectDep {
                dep_type: "remote".to_string(),
                uri: uri.uri.clone(),
                path: None,
                sha256: Some(sha256.clone()),
            },
            Self::Local { uri, path } => ResolvedProjectDep {
                dep_type: "local".to_string(),
                uri: uri.uri.clone(),
                path: Some(path.clone()),
                sha256: None,
            },
        }
    }
}

/// Parsed or resolved `PklProject.deps.json` lockfile (`ProjectDeps.java:46-225`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectDeps {
    pub schema_version: i32,
    /// Ordered entries keyed by `CanonicalPackageUri` (preserves insertion order matching `EconomicMap`).
    pub entries: Vec<(CanonicalPackageUri, ProjectDepEntry)>,
}

impl Default for ProjectDeps {
    fn default() -> Self {
        Self {
            schema_version: PROJECT_DEPS_SCHEMA_VERSION,
            entries: Vec::new(),
        }
    }
}

impl ProjectDeps {
    pub fn new(entries: Vec<(CanonicalPackageUri, ProjectDepEntry)>) -> Self {
        Self {
            schema_version: PROJECT_DEPS_SCHEMA_VERSION,
            entries,
        }
    }

    pub fn get(&self, canonical: &CanonicalPackageUri) -> Option<&ProjectDepEntry> {
        self.entries
            .iter()
            .find(|(k, _)| k == canonical)
            .map(|(_, v)| v)
    }

    /// Strictly parses `PklProject.deps.json`, matching `ProjectDeps.parse` (`ProjectDeps.java:90-131`).
    pub fn parse(text: &str, deps_path_display: &str) -> Result<Self, ModuleHostError> {
        let malformed = |details: String| {
            ModuleHostError::EvalError {
            message: format!(
                "Cannot resolve dependency because file `{deps_path_display}` is malformed.\nRun `pkl project resolve` to re-create this file."
            )
            .into(),
            hint: Some(format!("Details: {details}").into()),
        }
        };

        let node = parse_json(&PklString::from(text)).map_err(|err| {
            let err_str = match &err {
                Issue::Java(s) | Issue::Unsupported(s) => s.as_str().unwrap_or(""),
            };
            malformed(err_str.to_string())
        })?;

        let Node::Object(root) = node else {
            return Err(malformed(
                "Expected object of type `object`, but got non-object.".to_string(),
            ));
        };

        // 1. Validate required `schemaVersion` == 1 (`ProjectDeps.java:92-95`)
        let schema_version = match root.required("schemaVersion") {
            Ok(Node::Integer(v)) => *v,
            Ok(other) => {
                let err = format_error(Some("schemaVersion"), "int", other);
                let msg = match err {
                    Issue::Java(s) | Issue::Unsupported(s) => s.as_str().unwrap_or("").to_string(),
                };
                return Err(malformed(msg));
            }
            Err(err) => {
                let msg = match err {
                    Issue::Java(s) | Issue::Unsupported(s) => s.as_str().unwrap_or("").to_string(),
                };
                return Err(malformed(msg));
            }
        };
        if schema_version != PROJECT_DEPS_SCHEMA_VERSION {
            return Err(ModuleHostError::SecurityManagerException {
                message: Some(
                    format!(
                        "Unsupported schemaVersion for `PklProject.deps.json`: {schema_version}.\n\nRun `pkl project resolve` to re-generate this file."
                    )
                    .into(),
                ),
            });
        }

        // 2. Validate required `resolvedDependencies` object (`ProjectDeps.java:96-113`)
        let deps_val = root.required("resolvedDependencies").map_err(|err| {
            let msg = match err {
                Issue::Java(s) | Issue::Unsupported(s) => s.as_str().unwrap_or("").to_string(),
            };
            malformed(msg)
        })?;
        let Node::Object(deps_obj) = deps_val else {
            let err = format_error(Some("resolvedDependencies"), "object", deps_val);
            let msg = match err {
                Issue::Java(s) | Issue::Unsupported(s) => s.as_str().unwrap_or("").to_string(),
            };
            return Err(malformed(msg));
        };

        let mut entries = Vec::with_capacity(deps_obj.entries.len());
        for (key_str, val) in &deps_obj.entries {
            let Node::Object(dep_obj) = val else {
                return Err(malformed(
                    "Failed to parse key `resolvedDependencies`: invalid object.".to_string(),
                ));
            };

            let dep_type = dep_obj.string("type").map_err(|err| {
                let msg = match err {
                    Issue::Java(s) | Issue::Unsupported(s) => s.as_str().unwrap_or("").to_string(),
                };
                malformed(format!(
                    "Failed to parse key `resolvedDependencies`: {msg}."
                ))
            })?;
            let dep_type_str = dep_type.as_str().unwrap_or("");
            if dep_type_str != "remote" && dep_type_str != "local" {
                return Err(malformed(format!(
                    "Failed to parse key `resolvedDependencies`: Invalid dependency type `{dep_type_str}`."
                )));
            }

            let uri_pkl = dep_obj.string("uri").map_err(|err| {
                let msg = match err {
                    Issue::Java(s) | Issue::Unsupported(s) => s.as_str().unwrap_or("").to_string(),
                };
                malformed(format!(
                    "Failed to parse key `resolvedDependencies`: {msg}."
                ))
            })?;
            let uri_str = uri_pkl.as_str().unwrap_or("");
            let package_uri = PackageUri::parse(uri_pkl.clone()).map_err(|failure| {
                let reason = failure.reason();
                let reason_str = reason.as_str().unwrap_or("");
                malformed(format!(
                    "Failed to parse key `resolvedDependencies`: Failed to parse key `uri`: {reason_str}: {uri_str}.."
                ))
            })?;
            if let Err(err_msg) = package_uri.check_has_no_checksum_component() {
                return Err(malformed(format!(
                    "Failed to parse key `resolvedDependencies`: Failed to parse key `uri`: {err_msg}."
                )));
            }

            let canonical = CanonicalPackageUri::parse(key_str.clone()).map_err(|failure| {
                let reason = failure.reason();
                let reason_str = reason.as_str().unwrap_or("");
                malformed(format!(
                    "Failed to parse key `resolvedDependencies`: {reason_str}: {key_str}."
                ))
            })?;

            let entry = if dep_type_str == "remote" {
                let checksums_obj = dep_obj.object_field("checksums").map_err(|err| {
                    let msg = match err {
                        Issue::Java(s) | Issue::Unsupported(s) => {
                            s.as_str().unwrap_or("").to_string()
                        }
                    };
                    malformed(format!(
                        "Failed to parse key `resolvedDependencies`: {msg}."
                    ))
                })?;
                let sha256 = checksums_obj.string("sha256").map_err(|err| {
                    let msg = match err {
                        Issue::Java(s) | Issue::Unsupported(s) => {
                            s.as_str().unwrap_or("").to_string()
                        }
                    };
                    malformed(format!(
                        "Failed to parse key `resolvedDependencies`: Failed to parse key `checksums`: {msg}."
                    ))
                })?;
                ProjectDepEntry::Remote {
                    uri: package_uri,
                    sha256,
                }
            } else {
                let path = dep_obj.string("path").map_err(|err| {
                    let msg = match err {
                        Issue::Java(s) | Issue::Unsupported(s) => {
                            s.as_str().unwrap_or("").to_string()
                        }
                    };
                    malformed(format!(
                        "Failed to parse key `resolvedDependencies`: {msg}."
                    ))
                })?;
                ProjectDepEntry::Local {
                    uri: package_uri,
                    path: path.as_str().unwrap_or("").to_string(),
                }
            };

            // Preserve last-write-wins semantics on duplicate canonical keys
            if let Some((_, slot)) = entries.iter_mut().find(|(k, _)| k == &canonical) {
                *slot = entry;
            } else {
                entries.push((canonical, entry));
            }
        }

        Ok(Self {
            schema_version,
            entries,
        })
    }

    /// Serializes `PklProject.deps.json` matching `ProjectDeps.writeTo` (`ProjectDeps.java:170-225`):
    /// 2-space indent, UTF-8, forward-slash local paths, and a **trailing `\n`**.
    pub fn write_json(&self) -> String {
        let mut w = JsonPrettyWriter::new("  ");
        w.begin_object();
        w.name("schemaVersion");
        w.i64_value(i64::from(self.schema_version));
        w.name("resolvedDependencies");
        w.begin_object();
        for (canonical, entry) in &self.entries {
            w.name(canonical.as_str().unwrap_or(""));
            w.begin_object();
            match entry {
                ProjectDepEntry::Local { uri, path } => {
                    w.name("type");
                    w.string_value("local");
                    w.name("uri");
                    w.string_value(uri.uri.as_str().unwrap_or(""));
                    w.name("path");
                    w.string_value(&path.replace('\\', "/"));
                }
                ProjectDepEntry::Remote { uri, sha256 } => {
                    w.name("type");
                    w.string_value("remote");
                    w.name("uri");
                    w.string_value(uri.uri.as_str().unwrap_or(""));
                    w.name("checksums");
                    w.begin_object();
                    w.name("sha256");
                    w.string_value(sha256.as_str().unwrap_or(""));
                    w.end_object();
                }
            }
            w.end_object();
        }
        w.end_object();
        w.end_object();
        w.trailing_newline();
        w.finish()
    }
}

/// CLI options input into Pkl 0.32 settings precedence (`CliCommand.kt:111-260`, `BaseOptions.kt:360-395`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CliSettingsInput {
    pub omit_project_settings: bool,
    pub external_properties: Option<Vec<(String, String)>>,
    pub output_format: Option<String>,
    pub env: Option<Vec<(String, String)>>,
    pub allowed_modules: Option<Vec<String>>,
    pub allowed_resources: Option<Vec<String>>,
    pub no_cache: bool,
    pub module_cache_dir: Option<PathBuf>,
    pub root_dir: Option<PathBuf>,
    pub module_path: Option<Vec<PathBuf>>,
    pub color: Option<EvaluatorColor>,
    pub trace_mode: Option<String>,
    pub timeout_nanos: Option<u64>,
    pub http_proxy_address: Option<String>,
    pub http_no_proxy: Option<Vec<String>>,
    pub http_rewrites: Option<BTreeMap<String, String>>,
    pub external_module_readers: Option<BTreeMap<String, EvaluatorExternalReader>>,
    pub external_resource_readers: Option<BTreeMap<String, EvaluatorExternalReader>>,
    /// If true, apply `project.resolved_evaluator_settings.timeout` (API `EvaluatorBuilder.applyFromProject`);
    /// if false, follow `CliCommand.kt:314` where CLI timeout is CLI-flag-only.
    pub apply_project_timeout: bool,
}

/// Resolved effective evaluator settings after applying full Pkl 0.32 precedence:
/// `CLI > PklProject.resolvedEvaluatorSettings > settings.pkl > defaults`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EffectiveSettings {
    pub external_properties: BTreeMap<String, String>,
    pub env: BTreeMap<String, String>,
    pub allowed_modules: Vec<String>,
    pub allowed_resources: Vec<String>,
    pub module_cache_dir: Option<PathBuf>,
    pub root_dir: Option<PathBuf>,
    pub module_path: Vec<PathBuf>,
    pub color: EvaluatorColor,
    pub trace_mode: String,
    pub timeout_nanos: Option<u64>,
    pub http_proxy_address: Option<String>,
    pub http_no_proxy: Vec<String>,
    pub http_rewrites: BTreeMap<String, String>,
    pub external_module_readers: BTreeMap<String, EvaluatorExternalReader>,
    pub external_resource_readers: BTreeMap<String, EvaluatorExternalReader>,
    pub editor_url_scheme: String,
}

/// Quotes a literal string as a Java/Pkl `\Q...\E` regex pattern (`Pattern.quote`).
pub fn regex_quote_scheme(scheme: &str) -> String {
    let mut out = String::from("\\Q");
    for part in scheme.split("\\E") {
        if out.len() > 2 {
            out.push_str("\\E\\\\E\\Q");
        }
        out.push_str(part);
    }
    out.push_str(":\\E");
    out
}

/// Pure Pkl 0.32 settings precedence resolver (`CliCommand.kt:111-260`, `EvaluatorBuilder.java:489-571`).
pub fn resolve_effective_settings(
    cli: &CliSettingsInput,
    project_resolved_settings: Option<&EvaluatorSettings>,
    user_settings: &PklSettings,
    host_env: &[(String, String)],
    default_cache_dir: Option<PathBuf>,
) -> EffectiveSettings {
    let project_settings = if cli.omit_project_settings {
        None
    } else {
        project_resolved_settings
    };

    // 1. External readers: CLI replaces entire map per kind (`CliCommand.kt:195-201`)
    let external_module_readers = cli
        .external_module_readers
        .clone()
        .or_else(|| project_settings.and_then(|s| s.external_module_readers.clone()))
        .unwrap_or_default();
    let external_resource_readers = cli
        .external_resource_readers
        .clone()
        .or_else(|| project_settings.and_then(|s| s.external_resource_readers.clone()))
        .unwrap_or_default();

    // 2. Allowed modules / resources: CLI > project > defaults + auto-appended external reader schemes
    //    (ONLY when falling back to defaults! `CliCommand.kt:116-128`, `CliCommandTest.kt:66-95`)
    let allowed_modules = if let Some(ref explicit) = cli.allowed_modules {
        explicit.clone()
    } else if let Some(explicit) = project_settings.and_then(|s| s.allowed_modules.as_ref()) {
        explicit.clone()
    } else {
        let mut defaults: Vec<String> = DEFAULT_ALLOWED_MODULES
            .iter()
            .map(|s| (*s).to_string())
            .collect();
        for scheme in external_module_readers.keys() {
            defaults.push(regex_quote_scheme(scheme));
        }
        defaults
    };

    let allowed_resources = if let Some(ref explicit) = cli.allowed_resources {
        explicit.clone()
    } else if let Some(explicit) = project_settings.and_then(|s| s.allowed_resources.as_ref()) {
        explicit.clone()
    } else {
        let mut defaults: Vec<String> = DEFAULT_ALLOWED_RESOURCES
            .iter()
            .map(|s| (*s).to_string())
            .collect();
        for scheme in external_resource_readers.keys() {
            defaults.push(regex_quote_scheme(scheme));
        }
        defaults
    };

    // 3. External properties: CLI `-p` REPLACES project `externalProperties` (no merge, `CliCommand.kt:139`).
    //    `-f`/`--format` injects `pkl.outputFormat` afterwards (`VmContext.java:91-94`).
    let mut external_properties = BTreeMap::new();
    if let Some(ref cli_props) = cli.external_properties {
        for (k, v) in cli_props {
            external_properties.insert(k.clone(), v.clone());
        }
    } else if let Some(proj) = project_settings
        && (proj.external_properties_is_set || !proj.external_properties.is_empty())
    {
        for (k, v) in &proj.external_properties {
            external_properties.insert(k.clone(), v.clone());
        }
    }
    if let Some(ref format) = cli.output_format {
        external_properties.insert("pkl.outputFormat".to_string(), format.clone());
    }

    // 4. Environment variables: CLI `-e` > project `env` (even if empty `{}`!) > host env (`CliCommand.kt:142`).
    let mut env = BTreeMap::new();
    if let Some(ref cli_env) = cli.env {
        for (k, v) in cli_env {
            env.insert(k.clone(), v.clone());
        }
    } else if let Some(proj) = project_settings
        && (proj.env_is_set || !proj.env.is_empty())
    {
        for (k, v) in &proj.env {
            env.insert(k.clone(), v.clone());
        }
    } else {
        for (k, v) in host_env {
            env.insert(k.clone(), v.clone());
        }
    }

    // 5. Module cache dir / noCache (`CliCommand.kt:145-153`):
    //    CLI `--no-cache` -> None
    //    else CLI `--cache-dir` -> Some(dir) (overrides project `noCache = true`!)
    //    else project `noCache == Some(true)` -> None
    //    else project `moduleCacheDir` -> Some(dir)
    //    else default (`~/.pkl/cache`)
    let module_cache_dir = if cli.no_cache {
        None
    } else if let Some(ref dir) = cli.module_cache_dir {
        Some(dir.clone())
    } else if let Some(proj) = project_settings {
        if proj.no_cache == Some(true) {
            None
        } else {
            proj.module_cache_dir.clone().or(default_cache_dir)
        }
    } else {
        default_cache_dir
    };

    // 6. Root dir, module path, color, traceMode, timeout
    let root_dir = cli
        .root_dir
        .clone()
        .or_else(|| project_settings.and_then(|s| s.root_dir.clone()));

    let module_path = cli
        .module_path
        .clone()
        .or_else(|| project_settings.and_then(|s| s.module_path.clone()))
        .unwrap_or_default();

    let color = cli
        .color
        .or_else(|| project_settings.and_then(|s| s.color))
        .unwrap_or(EvaluatorColor::Auto);

    let trace_mode = cli
        .trace_mode
        .clone()
        .or_else(|| project_settings.and_then(|s| s.trace_mode.clone()))
        .unwrap_or_else(|| "compact".to_string());

    let timeout_nanos = cli.timeout_nanos.or_else(|| {
        if cli.apply_project_timeout {
            project_settings.and_then(|s| s.timeout_nanos)
        } else {
            None
        }
    });

    // 7. HTTP settings: per-field 3-tier precedence `CLI > PklProject > settings.pkl > defaults`
    //    (`CliCommand.kt:177-193`)
    let proj_http = project_settings.and_then(|s| s.http.as_ref());
    let user_http = user_settings.http.as_ref();

    let http_proxy_address = cli
        .http_proxy_address
        .clone()
        .or_else(|| {
            proj_http
                .and_then(|h| h.proxy.as_ref())
                .and_then(|p| p.address.clone())
        })
        .or_else(|| {
            user_http
                .and_then(|h| h.proxy.as_ref())
                .and_then(|p| p.address.clone())
        });

    let http_no_proxy = cli
        .http_no_proxy
        .clone()
        .or_else(|| {
            proj_http
                .and_then(|h| h.proxy.as_ref())
                .and_then(|p| p.no_proxy.clone())
        })
        .or_else(|| {
            user_http
                .and_then(|h| h.proxy.as_ref())
                .and_then(|p| p.no_proxy.clone())
        })
        .unwrap_or_default();

    let http_rewrites = cli
        .http_rewrites
        .clone()
        .or_else(|| proj_http.and_then(|h| h.rewrites.clone()))
        .or_else(|| user_http.and_then(|h| h.rewrites.clone()))
        .unwrap_or_default();

    EffectiveSettings {
        external_properties,
        env,
        allowed_modules,
        allowed_resources,
        module_cache_dir,
        root_dir,
        module_path,
        color,
        trace_mode,
        timeout_nanos,
        http_proxy_address,
        http_no_proxy,
        http_rewrites,
        external_module_readers,
        external_resource_readers,
        editor_url_scheme: user_settings.editor_url_scheme.clone(),
    }
}

/// Pure project directory discovery walking up from `working_dir` and stopping at `root_dir`,
/// matching `CliBaseOptions.kt:164-185`.
pub fn find_project_dir_with<F>(
    working_dir: &Path,
    root_dir: Option<&Path>,
    mut pkl_project_exists: F,
) -> Option<PathBuf>
where
    F: FnMut(&Path) -> bool,
{
    let mut current = working_dir.to_path_buf();
    loop {
        if let Some(root) = root_dir
            && !current.starts_with(root)
        {
            return None;
        }
        let candidate = current.join(PKL_PROJECT_FILENAME);
        if pkl_project_exists(&candidate) {
            return Some(current);
        }
        let parent = current.parent()?.to_path_buf();
        if parent == current {
            return None;
        }
        if let Some(root) = root_dir
            && !parent.starts_with(root)
        {
            return None;
        }
        current = parent;
    }
}

/// Session-owned project dependency manager matching `ProjectDependenciesManager.java:43-270`.
///
/// Manages:
/// - `has_uri(uri)` project scope check (`cannotResolveDependencyNoProject`)
/// - Root project direct dependencies + arbitrary-depth nested local project dependency tables
/// - Transitive remote package dependency remapping with exact version-out-of-date checks
/// - Local package path resolution against `project_base_uri`
#[derive(Clone, Debug)]
pub struct ProjectDependenciesManager {
    pub declared: DeclaredDependenciesTree,
    pub project_base_uri: String,
    pub project_dir: PathBuf,
    project_deps: Option<ProjectDeps>,
    my_dependencies: Option<BTreeMap<String, ProjectDepEntry>>,
    /// Maps `projectpackage://...` of every nested local project to its resolved dependency map.
    local_package_dependencies: BTreeMap<PackageUri, BTreeMap<String, ProjectDepEntry>>,
}

impl ProjectDependenciesManager {
    pub fn new(declared: DeclaredDependenciesTree, project_dir: PathBuf) -> Self {
        let project_base_uri = resolve_parent_uri(&declared.project_file_uri);
        Self {
            declared,
            project_base_uri,
            project_dir,
            project_deps: None,
            my_dependencies: None,
            local_package_dependencies: BTreeMap::new(),
        }
    }

    pub fn with_deps(
        declared: DeclaredDependenciesTree,
        project_dir: PathBuf,
        deps: ProjectDeps,
    ) -> Result<Self, ModuleHostError> {
        let mut mgr = Self::new(declared, project_dir);
        mgr.initialize_with_deps(deps)?;
        Ok(mgr)
    }

    pub fn project_deps_file_uri(&self) -> String {
        format!("{}{PKL_PROJECT_DEPS_FILENAME}", self.project_base_uri)
    }

    /// Checks whether `uri` belongs to this project's base URI directory tree
    /// (`ProjectDependenciesManager.java:80-84`).
    pub fn has_uri(&self, uri: &str) -> bool {
        let Some((base_scheme, base_auth, base_path)) =
            parse_hierarchical_uri(&self.project_base_uri)
        else {
            return false;
        };
        let Some((uri_scheme, uri_auth, uri_path)) = parse_hierarchical_uri(uri) else {
            return false;
        };
        base_scheme == uri_scheme && base_auth == uri_auth && uri_path.starts_with(base_path)
    }

    /// Initializes dependency tables from an already-parsed `ProjectDeps`.
    pub fn initialize_with_deps(&mut self, deps: ProjectDeps) -> Result<(), ModuleHostError> {
        let root_map = build_resolved_deps_for_project(&self.declared, &deps)?;
        let mut local_maps = BTreeMap::new();
        for (_, local_decl) in &self.declared.local_dependencies {
            collect_local_project_deps(local_decl, &deps, &mut local_maps)?;
        }
        self.project_deps = Some(deps);
        self.my_dependencies = Some(root_map);
        self.local_package_dependencies = local_maps;
        Ok(())
    }

    pub fn project_deps(&self) -> Option<&ProjectDeps> {
        self.project_deps.as_ref()
    }

    pub fn get_dependencies(&self) -> Result<&BTreeMap<String, ProjectDepEntry>, ModuleHostError> {
        self.my_dependencies.as_ref().ok_or_else(|| {
            ModuleHostError::EvalError {
                message: format!(
                    "Encountered an error when attempting to load `PklProject.deps.json` at `{}`.\n\nTry running `pkl project resolve` within the project directory to create a new set of dependencies.",
                    self.project_deps_file_uri()
                )
                .into(),
                hint: None,
            }
        })
    }

    pub fn is_local_package(&self, package_uri: &PackageUri) -> bool {
        let key = package_uri.to_project_package_uri().without_checksums();
        self.local_package_dependencies.contains_key(&key)
    }

    pub fn get_local_package_dependencies(
        &self,
        package_uri: &PackageUri,
    ) -> Option<&BTreeMap<String, ProjectDepEntry>> {
        let key = package_uri.to_project_package_uri().without_checksums();
        self.local_package_dependencies.get(&key)
    }

    pub fn get_resolved_dependency(
        &self,
        package_uri: &PackageUri,
    ) -> Result<&ProjectDepEntry, ModuleHostError> {
        let deps = self.project_deps.as_ref().ok_or_else(|| {
            ModuleHostError::EvalError {
                message: format!(
                    "Encountered an error when attempting to load `PklProject.deps.json` at `{}`.\n\nTry running `pkl project resolve` within the project directory to create a new set of dependencies.",
                    self.project_deps_file_uri()
                )
                .into(),
                hint: None,
            }
        })?;
        let canonical = package_uri.to_canonical();
        deps.get(&canonical).ok_or_else(|| {
            ModuleHostError::SecurityManagerException {
                message: Some(
                    format!(
                        "Did not find a resolved dependency for package `{}`.\n\nRun `pkl project resolve` to update resolved dependencies.",
                        package_uri.uri
                    )
                    .into(),
                ),
            }
        })
    }

    /// Resolves `@dep/asset` authored inside a project file (enforces `hasHierarchicalUris` and `hasUri`).
    pub fn resolve_dependency_notation(
        &self,
        importer_uri: &str,
        importer_has_hierarchical_uris: bool,
        authored_uri: &str,
    ) -> Result<(ProjectDepEntry, String), ModuleHostError> {
        if !importer_has_hierarchical_uris {
            return Err(ModuleHostError::SecurityManagerException {
                message: Some(
                    format!(
                        "Cannot resolve dependency because project URI `{}` does not have a hierarchical path.",
                        importer_uri
                    )
                    .into(),
                ),
            });
        }
        if !self.has_uri(importer_uri) {
            return Err(ModuleHostError::SecurityManagerException {
                message: Some(
                    "Cannot resolve dependency because there is no project found.\n\nIf you meant to import a path that starts with `@`, prefix the path with `./` (e.g. `import \"./@myPath\").\nIf you meant to import a dependency, ensure that this file is within a directory that contains a PklProject module."
                        .into(),
                ),
            });
        }

        let stripped = authored_uri.strip_prefix('@').unwrap_or(authored_uri);
        let (dep_name, asset_path) = match stripped.find('/') {
            Some(idx) => (&stripped[..idx], &stripped[idx..]),
            None => (stripped, "/"),
        };

        let deps = self.get_dependencies()?;
        let Some(entry) = deps.get(dep_name) else {
            return Err(ModuleHostError::SecurityManagerException {
                message: Some(
                    format!(
                        "Cannot find a dependency named `{dep_name}`, because it is not declared in the current project.\n\nTo fix this, add it to the `dependencies` section of your `PklProject` file, and resolve your dependencies."
                    )
                    .into(),
                ),
            });
        };
        Ok((entry.clone(), asset_path.to_string()))
    }

    /// Resolves transitive dependencies of a remote `projectpackage:` against `PklProject.deps.json`
    /// (`ProjectDependenciesManager.getResolvedDependenciesForPackage`, L191-218).
    pub fn resolve_package_dependencies(
        &self,
        package_uri: &PackageUri,
        metadata: &crate::packages::metadata::DependencyMetadata,
    ) -> Result<BTreeMap<String, ProjectDepEntry>, ModuleHostError> {
        let deps = self.project_deps.as_ref().ok_or_else(|| {
            ModuleHostError::EvalError {
                message: format!(
                    "Encountered an error when attempting to load `PklProject.deps.json` at `{}`.\n\nTry running `pkl project resolve` within the project directory to create a new set of dependencies.",
                    self.project_deps_file_uri()
                )
                .into(),
                hint: None,
            }
        })?;
        let mut resolved = BTreeMap::new();
        for dep in &metadata.dependencies {
            let canonical = dep.package_uri.to_canonical();
            let Some(resolved_dep) = deps.get(&canonical) else {
                return Err(ModuleHostError::SecurityManagerException {
                    message: Some(
                        format!(
                            "Did not find a resolved dependency for package `{}`.\n\nRun `pkl project resolve` to update resolved dependencies.",
                            dep.package_uri.display_name()
                        )
                        .into(),
                    ),
                });
            };
            if resolved_dep.version() < &dep.package_uri.version {
                return Err(ModuleHostError::SecurityManagerException {
                    message: Some(
                        format!(
                            "Transitive dependency declared by package `{}` is newer than what is declared in `PklProject.deps.json`.\n\nDeclared: `{}`\nResolved: `{}`\n\nRun `pkl project resolve` to update resolved dependencies.",
                            package_uri.display_name(),
                            dep.package_uri.display_name(),
                            resolved_dep.package_uri().display_name()
                        )
                        .into(),
                    ),
                });
            }
            resolved.insert(
                dep.name.as_str().unwrap_or("").to_string(),
                resolved_dep.clone(),
            );
        }
        Ok(resolved)
    }

    /// Computes the local filesystem path for a `Local` dependency asset (`Dependency.java:55-69`).
    pub fn resolve_local_asset_path(&self, local_rel_path: &str, asset_path: &str) -> PathBuf {
        let dep_root = normalize_path_lexical(&self.project_dir.join(local_rel_path));
        let sub = crate::packages::resolver::normalize_local_subpath(asset_path);
        dep_root.join(sub)
    }
}

fn check_project_dependency_out_of_date(
    project_file_uri: &str,
    declared_pkg: &PackageUri,
    resolved_dep: &ProjectDepEntry,
) -> Result<(), ModuleHostError> {
    if resolved_dep.is_local() && declared_pkg.version != *resolved_dep.version() {
        return Err(ModuleHostError::SecurityManagerException {
            message: Some(
                format!(
                    "Project `{project_file_uri}` declares a dependency on a local package whose version doesn't match what is declared in `PklProject.deps.json`.\n\nDeclared: `{}`\nResolved: `{}`\n\nRun `pkl project resolve` to update resolved dependencies.",
                    declared_pkg.display_name(),
                    resolved_dep.package_uri().display_name()
                )
                .into(),
            ),
        });
    }
    if resolved_dep.version() < &declared_pkg.version {
        return Err(ModuleHostError::SecurityManagerException {
            message: Some(
                format!(
                    "Project `{project_file_uri}` declares a dependency on a package that is newer than what is declared in `PklProject.deps.json`.\n\nDeclared: `{}`\nResolved: `{}`\n\nRun `pkl project resolve` to update resolved dependencies.",
                    declared_pkg.display_name(),
                    resolved_dep.package_uri().display_name()
                )
                .into(),
            ),
        });
    }
    Ok(())
}

fn build_resolved_deps_for_project(
    declared: &DeclaredDependenciesTree,
    resolved_deps: &ProjectDeps,
) -> Result<BTreeMap<String, ProjectDepEntry>, ModuleHostError> {
    let mut out = BTreeMap::new();
    for (name, local_decl) in &declared.local_dependencies {
        let Some(pkg_uri) = &local_decl.package_uri else {
            continue;
        };
        let canonical = pkg_uri.to_canonical();
        let Some(resolved) = resolved_deps.get(&canonical) else {
            return Err(ModuleHostError::SecurityManagerException {
                message: Some(
                    format!(
                        "Did not find a resolved dependency for package `{}`.\n\nRun `pkl project resolve` to update resolved dependencies.",
                        pkg_uri.uri
                    )
                    .into(),
                ),
            });
        };
        check_project_dependency_out_of_date(&declared.project_file_uri, pkg_uri, resolved)?;
        out.insert(name.clone(), resolved.clone());
    }
    for remote in &declared.remote_dependencies {
        let canonical = remote.package_uri.to_canonical();
        let Some(resolved) = resolved_deps.get(&canonical) else {
            return Err(ModuleHostError::SecurityManagerException {
                message: Some(
                    format!(
                        "Did not find a resolved dependency for package `{}`.\n\nRun `pkl project resolve` to update resolved dependencies.",
                        remote.package_uri.uri
                    )
                    .into(),
                ),
            });
        };
        check_project_dependency_out_of_date(
            &declared.project_file_uri,
            &remote.package_uri,
            resolved,
        )?;
        out.insert(remote.name.clone(), resolved.clone());
    }
    Ok(out)
}

fn collect_local_project_deps(
    local_decl: &DeclaredDependenciesTree,
    resolved_deps: &ProjectDeps,
    out: &mut BTreeMap<PackageUri, BTreeMap<String, ProjectDepEntry>>,
) -> Result<(), ModuleHostError> {
    let Some(pkg_uri) = &local_decl.package_uri else {
        return Ok(());
    };
    let key = pkg_uri.to_project_package_uri().without_checksums();
    if out.contains_key(&key) {
        return Ok(());
    }
    let deps_map = build_resolved_deps_for_project(local_decl, resolved_deps)?;
    out.insert(key, deps_map);
    for (_, child) in &local_decl.local_dependencies {
        collect_local_project_deps(child, resolved_deps, out)?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// DecodedValue -> Project / EvaluatorSettings / Annotations extraction
// ---------------------------------------------------------------------------

fn parse_decoded_project(obj: &DecodedObject) -> Result<Project, ModuleHostError> {
    let project_file_uri = get_required_string(obj, "projectFileUri")?;
    let project_base_uri = resolve_parent_uri(&project_file_uri);
    let project_dir = file_uri_to_dir_path(&project_base_uri);

    let package = match obj.get_property("package") {
        Some(DecodedValue::Object(pkg_obj)) => Some(parse_decoded_package(pkg_obj)?),
        Some(DecodedValue::Null) | None => None,
        Some(_) => {
            return Err(ModuleHostError::EvalError {
                message: "Expected `package` property to be a `Package` object or null.".into(),
                hint: None,
            });
        }
    };

    let declared_dependencies = parse_decoded_declared_dependencies(
        obj,
        &project_file_uri,
        package.as_ref().map(|p| p.uri.clone()),
    )?;

    let evaluator_settings = obj.get_property("evaluatorSettings").and_then(|v| match v {
        DecodedValue::Object(s) => Some(parse_decoded_evaluator_settings(s)),
        _ => None,
    });

    let resolved_evaluator_settings =
        obj.get_property("resolvedEvaluatorSettings")
            .and_then(|v| match v {
                DecodedValue::Object(s) => Some(parse_decoded_evaluator_settings(s)),
                _ => None,
            });

    let tests = obj
        .get_property("tests")
        .map(extract_string_sequence)
        .unwrap_or_default();

    let mut local_project_dependencies = Vec::new();
    if let Some(deps_val) = obj.get_property("dependencies") {
        for (dep_name, dep_val) in extract_key_value_pairs(deps_val) {
            if let DecodedValue::Object(dep_obj) = dep_val
                && is_decoded_project(dep_obj)
            {
                local_project_dependencies.push((dep_name, parse_decoded_project(dep_obj)?));
            }
        }
    }

    let annotations = match obj.get_property("annotations") {
        Some(DecodedValue::List(items)) | Some(DecodedValue::Listing(items)) => {
            let mut out = Vec::with_capacity(items.len());
            for item in items {
                if let DecodedValue::Object(ann_obj) = item {
                    out.push(decoded_object_to_annotation(ann_obj)?);
                }
            }
            out
        }
        _ => Vec::new(),
    };

    Ok(Project {
        project_file_uri,
        project_base_uri,
        project_dir,
        package,
        declared_dependencies,
        evaluator_settings,
        resolved_evaluator_settings,
        tests,
        local_project_dependencies,
        annotations,
    })
}

fn is_decoded_project(obj: &DecodedObject) -> bool {
    obj.get_property("projectFileUri").is_some()
        || (obj.class_info.module_uri == "pkl:Project" && obj.class_info.is_module)
}

fn parse_decoded_declared_dependencies(
    obj: &DecodedObject,
    project_file_uri: &str,
    my_package_uri: Option<PackageUri>,
) -> Result<DeclaredDependenciesTree, ModuleHostError> {
    let mut remote_dependencies = Vec::new();
    let mut local_dependencies = Vec::new();

    if let Some(deps_val) = obj.get_property("dependencies") {
        for (dep_name, dep_val) in extract_key_value_pairs(deps_val) {
            let DecodedValue::Object(dep_obj) = dep_val else {
                continue;
            };
            if is_decoded_project(dep_obj) {
                let local_file_uri = get_required_string(dep_obj, "projectFileUri")?;
                let local_pkg_uri = dep_obj
                    .get_property("package")
                    .and_then(|v| match v {
                        DecodedValue::Object(p) => p.get_property("uri"),
                        _ => None,
                    })
                    .and_then(|v| match v {
                        DecodedValue::String(s) => s.as_str(),
                        _ => None,
                    })
                    .map(PackageUri::parse)
                    .transpose()
                    .map_err(|e| ModuleHostError::UriSyntax { reason: e.reason() })?;
                let child_tree =
                    parse_decoded_declared_dependencies(dep_obj, &local_file_uri, local_pkg_uri)?;
                local_dependencies.push((dep_name, child_tree));
            } else {
                let uri_str = get_required_string(dep_obj, "uri")?;
                let package_uri = PackageUri::parse(uri_str.as_str())
                    .map_err(|e| ModuleHostError::UriSyntax { reason: e.reason() })?;
                if let Err(err) = package_uri.check_has_no_checksum_component() {
                    return Err(ModuleHostError::EvalError {
                        message: err,
                        hint: None,
                    });
                }
                let sha256 = dep_obj
                    .get_property("checksums")
                    .and_then(|v| match v {
                        DecodedValue::Object(c) => c.get_property("sha256"),
                        _ => None,
                    })
                    .and_then(|v| match v {
                        DecodedValue::String(s) => Some(s.clone()),
                        _ => None,
                    });
                remote_dependencies.push(DeclaredRemoteDep {
                    name: dep_name,
                    package_uri,
                    sha256,
                });
            }
        }
    }

    Ok(DeclaredDependenciesTree {
        project_file_uri: project_file_uri.to_string(),
        package_uri: my_package_uri,
        remote_dependencies,
        local_dependencies,
    })
}

fn parse_decoded_package(obj: &DecodedObject) -> Result<PackageSpec, ModuleHostError> {
    let name = get_required_string(obj, "name")?;
    let base_uri = get_required_string(obj, "baseUri")?;
    let version_str = get_required_string(obj, "version")?;
    let version = PackageVersion::parse(version_str.as_str())
        .map_err(|e| ModuleHostError::UriSyntax { reason: e.reason() })?;
    let uri_str = get_required_string(obj, "uri")?;
    let uri = PackageUri::parse(uri_str.as_str())
        .map_err(|e| ModuleHostError::UriSyntax { reason: e.reason() })?;
    let package_zip_url = get_required_string(obj, "packageZipUrl")?;
    crate::host::resources::package_components(&PklString::from(package_zip_url.as_str()))
        .map_err(|e| ModuleHostError::UriSyntax { reason: e.reason() })?;
    let description = get_optional_string(obj, "description");
    let authors = obj
        .get_property("authors")
        .map(extract_string_sequence)
        .unwrap_or_default();
    let website = get_optional_string(obj, "website");
    if let Some(ref u) = website {
        crate::host::resources::package_components(&PklString::from(u.as_str()))
            .map_err(|e| ModuleHostError::UriSyntax { reason: e.reason() })?;
    }
    let documentation = get_optional_string(obj, "documentation");
    if let Some(ref u) = documentation {
        crate::host::resources::package_components(&PklString::from(u.as_str()))
            .map_err(|e| ModuleHostError::UriSyntax { reason: e.reason() })?;
    }
    let source_code = get_optional_string(obj, "sourceCode");
    if let Some(ref u) = source_code {
        crate::host::resources::package_components(&PklString::from(u.as_str()))
            .map_err(|e| ModuleHostError::UriSyntax { reason: e.reason() })?;
    }
    let source_code_url_scheme = get_optional_string(obj, "sourceCodeUrlScheme");
    let license = get_optional_string(obj, "license");
    let license_text = get_optional_string(obj, "licenseText");
    let issue_tracker = get_optional_string(obj, "issueTracker");
    if let Some(ref u) = issue_tracker {
        crate::host::resources::package_components(&PklString::from(u.as_str()))
            .map_err(|e| ModuleHostError::UriSyntax { reason: e.reason() })?;
    }
    let api_tests = obj
        .get_property("apiTests")
        .map(extract_string_sequence)
        .unwrap_or_default();
    let exclude = obj
        .get_property("exclude")
        .map(extract_string_sequence)
        .unwrap_or_default();

    Ok(PackageSpec {
        name,
        base_uri,
        version,
        uri,
        package_zip_url,
        description,
        authors,
        website,
        documentation,
        source_code,
        source_code_url_scheme,
        license,
        license_text,
        issue_tracker,
        api_tests,
        exclude,
    })
}

fn parse_decoded_evaluator_settings(obj: &DecodedObject) -> EvaluatorSettings {
    let mut s = EvaluatorSettings::default();
    if let Some(val) = obj.get_property("externalProperties")
        && !matches!(val, DecodedValue::Null)
    {
        s.external_properties_is_set = true;
        s.external_properties = extract_string_pairs(val);
    }
    if let Some(val) = obj.get_property("env")
        && !matches!(val, DecodedValue::Null)
    {
        s.env_is_set = true;
        s.env = extract_string_pairs(val);
    }
    if let Some(val) = obj.get_property("allowedModules")
        && !matches!(val, DecodedValue::Null)
    {
        s.allowed_modules = Some(extract_string_sequence(val));
    }
    if let Some(val) = obj.get_property("allowedResources")
        && !matches!(val, DecodedValue::Null)
    {
        s.allowed_resources = Some(extract_string_sequence(val));
    }
    if let Some(DecodedValue::Bool(b)) = obj.get_property("noCache") {
        s.no_cache = Some(*b);
    }
    if let Some(path) = get_optional_string(obj, "moduleCacheDir") {
        s.module_cache_dir = Some(strip_file_scheme_to_path(&path));
    }
    if let Some(path) = get_optional_string(obj, "rootDir") {
        s.root_dir = Some(strip_file_scheme_to_path(&path));
    }
    if let Some(trace) = get_optional_string(obj, "traceMode") {
        s.trace_mode = Some(trace);
    }
    if let Some(color_str) = get_optional_string(obj, "color") {
        s.color = EvaluatorColor::parse(&color_str);
    }
    if let Some(val) = obj.get_property("modulePath")
        && !matches!(val, DecodedValue::Null)
    {
        s.module_path = Some(
            extract_string_sequence(val)
                .into_iter()
                .map(|p| strip_file_scheme_to_path(&p))
                .collect(),
        );
    }
    if let Some(DecodedValue::Duration(d)) = obj.get_property("timeout") {
        let nanos = d
            .to_unit(crate::duration::DurationUnit::Nanoseconds)
            .value();
        if nanos.is_finite() && nanos >= 0.0 {
            s.timeout_nanos = Some(nanos as u64);
        }
    }
    if let Some(val) = obj.get_property("http") {
        s.http = parse_decoded_http_settings(val);
    }
    if let Some(val) = obj.get_property("externalModuleReaders")
        && !matches!(val, DecodedValue::Null)
    {
        s.external_module_readers = Some(parse_decoded_external_readers(val));
    }
    if let Some(val) = obj.get_property("externalResourceReaders")
        && !matches!(val, DecodedValue::Null)
    {
        s.external_resource_readers = Some(parse_decoded_external_readers(val));
    }
    s
}

fn parse_decoded_http_settings(val: &DecodedValue) -> Option<EvaluatorHttpSettings> {
    let DecodedValue::Object(obj) = val else {
        return None;
    };
    let proxy = obj.get_property("proxy").and_then(|pv| {
        let DecodedValue::Object(pobj) = pv else {
            return None;
        };
        let address = get_optional_string(pobj, "address");
        let no_proxy = pobj.get_property("noProxy").and_then(|nv| {
            if matches!(nv, DecodedValue::Null) {
                None
            } else {
                Some(extract_string_sequence(nv))
            }
        });
        Some(EvaluatorHttpProxy { address, no_proxy })
    });
    let rewrites = obj.get_property("rewrites").and_then(|rv| {
        if matches!(rv, DecodedValue::Null) {
            None
        } else {
            let mut map = BTreeMap::new();
            for (k, v) in extract_string_pairs(rv) {
                map.insert(k, v);
            }
            Some(map)
        }
    });
    Some(EvaluatorHttpSettings { proxy, rewrites })
}

fn parse_decoded_external_readers(val: &DecodedValue) -> BTreeMap<String, EvaluatorExternalReader> {
    let mut map = BTreeMap::new();
    for (scheme, reader_val) in extract_key_value_pairs(val) {
        if let DecodedValue::Object(robj) = reader_val {
            let executable = get_optional_string(robj, "executable").unwrap_or_default();
            let arguments = robj
                .get_property("arguments")
                .map(extract_string_sequence)
                .unwrap_or_default();
            let working_dir = get_optional_string(robj, "workingDir");
            map.insert(
                scheme,
                EvaluatorExternalReader {
                    executable,
                    arguments,
                    working_dir,
                },
            );
        }
    }
    map
}

fn decoded_object_to_annotation(
    obj: &DecodedObject,
) -> Result<MetadataAnnotation, ModuleHostError> {
    let mut properties = Vec::new();
    for (k, v) in obj.properties() {
        properties.push((k, decoded_value_to_metadata_value(v)?));
    }
    Ok(MetadataAnnotation {
        class_info: MetadataClassInfo {
            module_name: obj.class_info.module_name.clone(),
            class_name: obj.class_info.simple_name.clone(),
            module_uri: obj.class_info.module_uri.clone(),
        },
        properties,
    })
}

fn decoded_value_to_metadata_value(val: &DecodedValue) -> Result<MetadataValue, ModuleHostError> {
    Ok(match val {
        DecodedValue::Null => MetadataValue::Null,
        DecodedValue::Bool(b) => MetadataValue::Bool(*b),
        DecodedValue::Int(i) => MetadataValue::Int(*i),
        DecodedValue::Float(f) => MetadataValue::Float(*f),
        DecodedValue::String(s) => MetadataValue::String(s.clone()),
        DecodedValue::Object(obj) => MetadataValue::PObject(decoded_object_to_annotation(obj)?),
        DecodedValue::List(items) | DecodedValue::Listing(items) => {
            let mut out = Vec::with_capacity(items.len());
            for item in items {
                out.push(decoded_value_to_metadata_value(item)?);
            }
            MetadataValue::List(out)
        }
        DecodedValue::Set(items) => {
            let mut out = Vec::with_capacity(items.len());
            for item in items {
                out.push(decoded_value_to_metadata_value(item)?);
            }
            MetadataValue::Set(out)
        }
        DecodedValue::Map(entries) | DecodedValue::Mapping(entries) => {
            let mut out = Vec::with_capacity(entries.len());
            for (k, v) in entries {
                out.push((
                    decoded_value_to_metadata_value(k)?,
                    decoded_value_to_metadata_value(v)?,
                ));
            }
            MetadataValue::Map(out)
        }
        DecodedValue::Regex(pat) => MetadataValue::Pattern(pat.clone()),
        DecodedValue::DataSize(ds) => MetadataValue::DataSize {
            value: ds.value(),
            unit: PklString::from(ds.unit().symbol()),
        },
        DecodedValue::Duration(d) => MetadataValue::Duration {
            value: d.value(),
            unit: PklString::from(d.unit().symbol()),
        },
        DecodedValue::Pair(a, b) => MetadataValue::Pair(
            Box::new(decoded_value_to_metadata_value(a)?),
            Box::new(decoded_value_to_metadata_value(b)?),
        ),
        other => {
            return Err(ModuleHostError::EvalError {
                message: format!(
                    "Error serializing annotation for PklProject:\n:  cannot render value with unexpected type: {other:?}"
                )
                .into(),
                hint: None,
            });
        }
    })
}

fn get_optional_string(obj: &DecodedObject, field: &str) -> Option<String> {
    match obj.get_property(field) {
        Some(DecodedValue::String(s)) => s.as_str().map(str::to_string),
        _ => None,
    }
}

fn get_required_string(obj: &DecodedObject, field: &str) -> Result<String, ModuleHostError> {
    get_optional_string(obj, field).ok_or_else(|| ModuleHostError::EvalError {
        message: format!("Missing required string property `{field}`.").into(),
        hint: None,
    })
}

fn extract_string_sequence(val: &DecodedValue) -> Vec<String> {
    match val {
        DecodedValue::List(items) | DecodedValue::Listing(items) | DecodedValue::Set(items) => {
            items
                .iter()
                .filter_map(|v| match v {
                    DecodedValue::String(s) => s.as_str().map(str::to_string),
                    _ => None,
                })
                .collect()
        }
        _ => Vec::new(),
    }
}

fn extract_key_value_pairs(val: &DecodedValue) -> Vec<(String, &DecodedValue)> {
    match val {
        DecodedValue::Map(entries) | DecodedValue::Mapping(entries) => entries
            .iter()
            .filter_map(|(k, v)| match k {
                DecodedValue::String(s) => s.as_str().map(|ks| (ks.to_string(), v)),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

fn extract_string_pairs(val: &DecodedValue) -> Vec<(String, String)> {
    extract_key_value_pairs(val)
        .into_iter()
        .filter_map(|(k, v)| match v {
            DecodedValue::String(s) => s.as_str().map(|vs| (k, vs.to_string())),
            _ => None,
        })
        .collect()
}

fn strip_file_scheme_to_path(raw: &str) -> PathBuf {
    let path_part = raw
        .strip_prefix("file://")
        .or_else(|| raw.strip_prefix("file:"))
        .unwrap_or(raw);
    PathBuf::from(crate::packages::zip::percent_decode(path_part))
}

fn resolve_parent_uri(file_uri: &str) -> String {
    match file_uri.rfind('/') {
        Some(idx) => file_uri[..=idx].to_string(),
        None => file_uri.to_string(),
    }
}

fn file_uri_to_dir_path(base_uri: &str) -> PathBuf {
    let raw = base_uri
        .strip_prefix("file://")
        .or_else(|| base_uri.strip_prefix("file:"))
        .unwrap_or(base_uri);
    let trimmed = if raw.len() > 1 {
        raw.trim_end_matches('/')
    } else {
        raw
    };
    PathBuf::from(crate::packages::zip::percent_decode(trimmed))
}

fn parse_hierarchical_uri(uri: &str) -> Option<(&str, &str, &str)> {
    let (scheme, rest) = uri.split_once(':')?;
    if scheme.is_empty() {
        return None;
    }
    let without_frag = rest.split_once('#').map_or(rest, |(before, _)| before);
    let without_query = without_frag
        .split_once('?')
        .map_or(without_frag, |(before, _)| before);
    if let Some(after_slashes) = without_query.strip_prefix("//") {
        let (auth, path) = split_authority_and_path(after_slashes);
        Some((scheme, auth, path))
    } else if without_query.starts_with('/') {
        // Single-slash hierarchical URI such as `file:/path/to/mod.pkl` (`ProjectDependenciesManager.java:80-84`).
        Some((scheme, "", without_query))
    } else {
        None
    }
}

fn split_authority_and_path(rest: &str) -> (&str, &str) {
    match rest.find('/') {
        Some(idx) => (&rest[..idx], &rest[idx..]),
        None => (rest, ""),
    }
}

fn normalize_path_lexical(path: &Path) -> PathBuf {
    use std::path::Component;
    let mut out = PathBuf::new();
    for comp in path.components() {
        match comp {
            Component::CurDir => {}
            Component::ParentDir => {
                if !out.pop() {
                    out.push(comp);
                }
            }
            other => out.push(other),
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Backwards-compatible M2 helper functions (upgraded to spec-exact semantics)
// ---------------------------------------------------------------------------

/// Spec-exact SemVer comparison (matching `Version.java:188-206`).
pub fn compare_dep_versions(v1: &str, v2: &str) -> std::cmp::Ordering {
    match (PackageVersion::parse(v1), PackageVersion::parse(v2)) {
        (Ok(a), Ok(b)) => a.cmp(&b),
        _ => v1.cmp(v2),
    }
}

pub fn extract_version_from_uri(uri: &str) -> Option<&str> {
    let without_frag = uri.split('#').next().unwrap_or(uri);
    let without_checksum = without_frag.split("::").next().unwrap_or(without_frag);
    without_checksum.rsplit('@').next()
}

/// Parse `PklProject.deps.json` content into resolved dependencies (legacy BTreeMap target).
///
/// Enforces full Pkl 0.32 `ProjectDeps.parse` validation (`schemaVersion == 1`,
/// canonical keys, non-checksummed package URIs, required fields).
pub fn parse_deps_json(
    text: &str,
    resolved: &mut BTreeMap<String, ResolvedProjectDep>,
    deps_path_display: &str,
) -> Result<(), ModuleHostError> {
    let parsed = ProjectDeps::parse(text, deps_path_display)?;
    for (canonical, entry) in parsed.entries {
        if let Some(key) = canonical.as_str() {
            resolved.insert(key.to_string(), entry.to_legacy_resolved());
        }
    }
    Ok(())
}

/// Matches a declared dependency URI/name against the resolved dependencies table using
/// `CanonicalPackageUri` (respects major version separation!).
pub fn find_matching_resolved(
    uri: &str,
    name: &str,
    resolved: &BTreeMap<String, ResolvedProjectDep>,
) -> Option<ResolvedProjectDep> {
    // 1. Try canonical package URI match first (exact host + path + major version!)
    if let Ok(pkg) = PackageUri::parse(uri) {
        let canonical = pkg.to_canonical();
        if let Some(key) = canonical.as_str()
            && let Some(dep) = resolved.get(key)
        {
            return Some(dep.clone());
        }
        // Also support tables keyed by dependency name when values hold matching canonical URI
        for dep in resolved.values() {
            if let Ok(dep_pkg) = PackageUri::parse(dep.uri.clone())
                && dep_pkg.to_canonical() == canonical
            {
                return Some(dep.clone());
            }
        }
    }
    // 2. Try exact name or exact URI key (for unit tests keying by declared name)
    if let Some(dep) = resolved.get(name) {
        return Some(dep.clone());
    }
    if let Some(dep) = resolved.get(uri) {
        return Some(dep.clone());
    }
    None
}

/// Resolves an `@dep` import authored in a project module (legacy M2 helper upgraded to SemVer).
pub fn resolve_project_dependency(
    project: &PklProject,
    authored_uri: &str,
) -> Result<PklString, ModuleHostError> {
    let stripped = authored_uri.strip_prefix('@').unwrap_or(authored_uri);
    let (dep_name, asset_path) = match stripped.find('/') {
        Some(idx) => (&stripped[..idx], &stripped[idx..]),
        None => (stripped, "/"),
    };

    let declared = project.declared_dependencies.get(dep_name);

    if let Some(dep) = project.dependencies.get(dep_name) {
        let base_uri = dep.uri.as_str().unwrap_or("");
        if let Some(decl) = declared {
            let project_file_uri = format!("file://{}/PklProject", project.project_dir.display());
            let resolved_uri_clean = base_uri.replace("projectpackage:", "package:");
            if decl.is_local {
                if let Some(local_uri) = &decl.local_package_uri {
                    let dec_ver = extract_version_from_uri(local_uri);
                    let res_ver = extract_version_from_uri(base_uri);
                    if let (Some(dv), Some(rv)) = (dec_ver, res_ver)
                        && compare_dep_versions(dv, rv) != std::cmp::Ordering::Equal
                    {
                        let msg = format!(
                            "Project `{project_file_uri}` declares a dependency on a local package whose version doesn't match what is declared in `PklProject.deps.json`.\n\nDeclared: `{local_uri}`\nResolved: `{resolved_uri_clean}`\n\nRun `pkl project resolve` to update resolved dependencies."
                        );
                        return Err(ModuleHostError::SecurityManagerException {
                            message: Some(msg.into()),
                        });
                    }
                }
            } else {
                let dec_ver = extract_version_from_uri(&decl.uri);
                let res_ver = extract_version_from_uri(base_uri);
                if let (Some(dv), Some(rv)) = (dec_ver, res_ver)
                    && compare_dep_versions(dv, rv) == std::cmp::Ordering::Greater
                {
                    let decl_uri = &decl.uri;
                    let msg = format!(
                        "Project `{project_file_uri}` declares a dependency on a package that is newer than what is declared in `PklProject.deps.json`.\n\nDeclared: `{decl_uri}`\nResolved: `{resolved_uri_clean}`\n\nRun `pkl project resolve` to update resolved dependencies."
                    );
                    return Err(ModuleHostError::SecurityManagerException {
                        message: Some(msg.into()),
                    });
                }
            }
        }
        let formatted = format!("{base_uri}#{asset_path}");
        Ok(formatted.into())
    } else if let Some(decl) = declared {
        let declared_uri = &decl.uri;
        let msg = format!(
            "Did not find a resolved dependency for package `{declared_uri}`.\n\nRun `pkl project resolve` to update resolved dependencies."
        );
        Err(ModuleHostError::SecurityManagerException {
            message: Some(msg.into()),
        })
    } else {
        let msg = format!(
            "Cannot find a dependency named `{dep_name}`, because it is not declared in the current project.\n\nTo fix this, add it to the `dependencies` section of your `PklProject` file, and resolve your dependencies."
        );
        Err(ModuleHostError::SecurityManagerException {
            message: Some(msg.into()),
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectValidationResult {
    pub is_project: bool,
    pub is_module: bool,
    pub got_type: String,
    pub package_uri: Option<String>,
    pub declared_dependencies: BTreeMap<String, DeclaredDep>,
    pub resolved_evaluator_settings: Option<EvaluatorSettings>,
}

/// Legacy JSON validation parser retained for `module_eval_probe.rs` until Wave 2 unification.
pub fn parse_project_validation(
    json_text: &str,
) -> Result<ProjectValidationResult, ModuleHostError> {
    let node = parse_json(&PklString::from(json_text)).map_err(|err| {
        ModuleHostError::SecurityManagerException {
            message: Some(format!("Failed to parse project JSON: {err:?}").into()),
        }
    })?;

    let Node::Object(root) = node else {
        return Err(ModuleHostError::SecurityManagerException {
            message: Some("Expected JSON object from project validation".into()),
        });
    };

    let is_project = match root.optional("isProject") {
        Some(Node::Boolean(b)) => *b,
        _ => false,
    };
    let is_module = match root.optional("isModule") {
        Some(Node::Boolean(b)) => *b,
        _ => false,
    };
    let got_type = match root.optional("gotType") {
        Some(Node::String(s)) => s.as_str().unwrap_or("Module").to_string(),
        _ => "Module".to_string(),
    };
    let package_uri = match root.optional("packageUri") {
        Some(Node::String(s)) => s.as_str().map(|v| v.to_string()),
        _ => None,
    };

    let mut declared_dependencies = BTreeMap::new();
    if let Some(Node::Object(deps_obj)) = root.optional("declaredDependencies") {
        for (name_str, dep_node) in &deps_obj.entries {
            if let Node::Object(dep_fields) = dep_node {
                let dep_name = name_str.as_str().unwrap_or("").to_string();
                let dep_uri = match dep_fields.optional("uri") {
                    Some(Node::String(s)) => s.as_str().unwrap_or("").to_string(),
                    _ => String::new(),
                };
                let is_local = match dep_fields.optional("isLocal") {
                    Some(Node::Boolean(b)) => *b,
                    _ => false,
                };
                let local_pkg_uri = match dep_fields.optional("localPackageUri") {
                    Some(Node::String(s)) => s.as_str().map(|v| v.to_string()),
                    _ => None,
                };
                declared_dependencies.insert(
                    dep_name.clone(),
                    DeclaredDep {
                        name: dep_name,
                        uri: dep_uri,
                        is_local,
                        local_package_uri: local_pkg_uri,
                    },
                );
            }
        }
    }

    let mut resolved_evaluator_settings = None;
    if let Some(Node::Object(s_obj)) = root.optional("resolvedEvaluatorSettings") {
        let mut s = EvaluatorSettings::default();
        if let Some(Node::Object(env_obj)) = s_obj.optional("env") {
            s.env_is_set = true;
            for (k, v) in &env_obj.entries {
                if let Node::String(vs) = v
                    && let (Some(ks), Some(vss)) = (k.as_str(), vs.as_str())
                {
                    s.env.push((ks.to_string(), vss.to_string()));
                }
            }
        }
        if let Some(Node::Object(props_obj)) = s_obj.optional("externalProperties") {
            s.external_properties_is_set = true;
            for (k, v) in &props_obj.entries {
                if let Node::String(vs) = v
                    && let (Some(ks), Some(vss)) = (k.as_str(), vs.as_str())
                {
                    s.external_properties
                        .push((ks.to_string(), vss.to_string()));
                }
            }
        }
        if let Some(Node::Array(arr)) = s_obj.optional("allowedModules") {
            let mut list = Vec::new();
            for item in arr {
                if let Node::String(str_val) = item
                    && let Some(sv) = str_val.as_str()
                {
                    list.push(sv.to_string());
                }
            }
            s.allowed_modules = Some(list);
        }
        if let Some(Node::Array(arr)) = s_obj.optional("allowedResources") {
            let mut list = Vec::new();
            for item in arr {
                if let Node::String(str_val) = item
                    && let Some(sv) = str_val.as_str()
                {
                    list.push(sv.to_string());
                }
            }
            s.allowed_resources = Some(list);
        }
        if let Some(Node::Boolean(b)) = s_obj.optional("noCache") {
            s.no_cache = Some(*b);
        }
        if let Some(Node::String(str_val)) = s_obj.optional("rootDir")
            && let Some(sv) = str_val.as_str()
        {
            s.root_dir = Some(strip_file_scheme_to_path(sv));
        }
        if let Some(Node::String(str_val)) = s_obj.optional("moduleCacheDir")
            && let Some(sv) = str_val.as_str()
        {
            s.module_cache_dir = Some(strip_file_scheme_to_path(sv));
        }
        if let Some(Node::String(str_val)) = s_obj.optional("traceMode")
            && let Some(sv) = str_val.as_str()
        {
            s.trace_mode = Some(sv.to_string());
        }
        resolved_evaluator_settings = Some(s);
    }

    Ok(ProjectValidationResult {
        is_project,
        is_module,
        got_type,
        package_uri,
        declared_dependencies,
        resolved_evaluator_settings,
    })
}

// ---------------------------------------------------------------------------
// Fast static AST loader for common `amends "pkl:Project"` manifests.
// Returns `None` on any dynamic/non-trivial feature, constraint failure, or
// unknown field so the caller falls back to full `NativeHost` evaluation with
// identical Pkl diagnostics.
// ---------------------------------------------------------------------------

const MAX_STATIC_PROJECT_DEPTH: usize = 16;
const MAX_STATIC_PROJECT_VISITS: usize = 64;

/// Attempts to load a purely static `amends "pkl:Project"` manifest directly
/// from its parsed AST, avoiding compiling `pkl:Project`, `pkl:EvaluatorSettings`,
/// `pkl:platform`, `pkl:reflect`, and `pkl:semver` on every cold CLI invocation.
pub fn try_parse_static_project<F>(
    project_file_uri: &str,
    text: &str,
    read_local_project: &mut F,
) -> Option<Project>
where
    F: FnMut(&str, &str) -> Option<(String, String)>,
{
    let mut active = std::collections::HashSet::new();
    let mut memo = std::collections::HashMap::new();
    let mut visits = 0usize;
    try_parse_static_project_inner(
        project_file_uri,
        text,
        read_local_project,
        0,
        &mut active,
        &mut memo,
        &mut visits,
    )
}

fn try_parse_static_project_inner<F>(
    project_file_uri: &str,
    text: &str,
    read_local_project: &mut F,
    depth: usize,
    active: &mut std::collections::HashSet<String>,
    memo: &mut std::collections::HashMap<String, Project>,
    visits: &mut usize,
) -> Option<Project>
where
    F: FnMut(&str, &str) -> Option<(String, String)>,
{
    if depth > MAX_STATIC_PROJECT_DEPTH || !active.insert(project_file_uri.to_string()) {
        return None;
    }
    if let Some(cached) = memo.get(project_file_uri) {
        active.remove(project_file_uri);
        return Some(cached.clone());
    }
    *visits += 1;
    if *visits > MAX_STATIC_PROJECT_VISITS {
        active.remove(project_file_uri);
        return None;
    }

    let parsed = parse_static_project_body(
        project_file_uri,
        text,
        read_local_project,
        depth,
        active,
        memo,
        visits,
    );
    active.remove(project_file_uri);
    if let Some(ref project) = parsed {
        memo.insert(project_file_uri.to_string(), project.clone());
    }
    parsed
}

fn parse_static_project_body<F>(
    project_file_uri: &str,
    text: &str,
    read_local_project: &mut F,
    depth: usize,
    active: &mut std::collections::HashSet<String>,
    memo: &mut std::collections::HashMap<String, Project>,
    visits: &mut usize,
) -> Option<Project>
where
    F: FnMut(&str, &str) -> Option<(String, String)>,
{
    use crate::syntax::{ExprKind, ModuleParentKind, ObjectMember};
    use std::collections::HashSet;

    let src = crate::source::SourceText::from(text);
    let module = crate::parser::parse_module(src.entire()).ok()?;
    if !module.deferred_diagnostics.is_empty() {
        return None;
    }
    let decl = module.declaration.as_ref()?;
    if !decl.annotations.is_empty() || !decl.modifiers.is_empty() || decl.name.is_some() {
        return None;
    }
    let parent = decl.parent.as_ref()?;
    if parent.kind != ModuleParentKind::Amends || parent.uri != "pkl:Project" {
        return None;
    }

    let project_base_uri = resolve_parent_uri(project_file_uri);
    let project_dir = file_uri_to_dir_path(&project_base_uri);

    let mut seen_top = HashSet::new();
    let mut package: Option<PackageSpec> = None;
    let mut tests: Vec<String> = Vec::new();
    let mut remote_dependencies: Vec<DeclaredRemoteDep> = Vec::new();
    let mut local_dependencies: Vec<(String, DeclaredDependenciesTree)> = Vec::new();
    let mut local_project_dependencies: Vec<(String, Project)> = Vec::new();

    for member in &module.members {
        if member.local
            || !member.annotations.is_empty()
            || !member.modifiers.is_empty()
            || member.ty.is_some()
        {
            return None;
        }
        let name = member.name.as_str()?;
        if !seen_top.insert(name) {
            return None;
        }
        match name {
            "dependencies" => {
                let dep_members = extract_static_object_body(&member.expr, Some(member), false)?;
                let mut seen_deps = HashSet::new();
                for om in dep_members {
                    let ObjectMember::Entry { key, value, .. } = om else {
                        return None;
                    };
                    let dep_name = static_string_expr(key)?;
                    if dep_name.contains('/') || !seen_deps.insert(dep_name.clone()) {
                        return None;
                    }
                    if let Some(entry_members) = extract_static_object_body(value, None, true) {
                        let remote = parse_static_remote_dep(&dep_name, entry_members)?;
                        remote_dependencies.push(remote);
                    } else if let ExprKind::Import {
                        uri: import_uri,
                        glob: false,
                        ..
                    } = &value.unparenthesized().kind
                    {
                        let import_str = import_uri.as_str()?;
                        let (child_uri, child_text) =
                            read_local_project(project_file_uri, import_str)?;
                        if !same_uri_scheme(project_file_uri, &child_uri)
                            || !child_uri.ends_with("/PklProject")
                            || child_uri == project_file_uri
                        {
                            return None;
                        }
                        let child = try_parse_static_project_inner(
                            &child_uri,
                            &child_text,
                            read_local_project,
                            depth + 1,
                            active,
                            memo,
                            visits,
                        )?;
                        child.package.as_ref()?;
                        local_dependencies
                            .push((dep_name.clone(), child.declared_dependencies.clone()));
                        local_project_dependencies.push((dep_name, child));
                    } else {
                        return None;
                    }
                }
            }
            "package" => {
                let pkg_members = extract_static_object_body(&member.expr, Some(member), false)?;
                package = Some(parse_static_package(pkg_members)?);
            }
            "tests" => {
                let items = parse_static_string_listing(&member.expr, Some(member))?;
                if !all_distinct(&items) || !items.iter().all(|s| is_valid_static_uri(s)) {
                    return None;
                }
                tests = items;
            }
            _ => return None,
        }
    }

    let declared_dependencies = DeclaredDependenciesTree {
        project_file_uri: project_file_uri.to_string(),
        package_uri: package.as_ref().map(|p| p.uri.clone()),
        remote_dependencies,
        local_dependencies,
    };

    Some(Project {
        project_file_uri: project_file_uri.to_string(),
        project_base_uri,
        project_dir,
        package,
        declared_dependencies,
        evaluator_settings: Some(EvaluatorSettings::default()),
        resolved_evaluator_settings: Some(EvaluatorSettings::default()),
        tests,
        local_project_dependencies,
        annotations: Vec::new(),
    })
}

fn static_string_expr(expr: &crate::syntax::Expr) -> Option<String> {
    match &expr.unparenthesized().kind {
        crate::syntax::ExprKind::String(s) => s.as_str().map(str::to_string),
        _ => None,
    }
}

fn same_uri_scheme(a: &str, b: &str) -> bool {
    let Some(ia) = a.find(':') else {
        return false;
    };
    let Some(ib) = b.find(':') else {
        return false;
    };
    a[..ia] == b[..ib]
}

fn all_distinct(items: &[String]) -> bool {
    let mut seen = std::collections::HashSet::with_capacity(items.len());
    items.iter().all(|s| seen.insert(s.as_str()))
}

/// Strictly validates a SemVer 2.0.0 string matching `pkl:semver`'s `versionRegex`
/// (`external/pkl/stdlib/semver.pkl:227`) and non-negative 64-bit integer components.
fn is_strict_semver(s: &str) -> bool {
    let (core_and_pre, build) = match s.split_once('+') {
        Some((head, b)) => (head, Some(b)),
        None => (s, None),
    };
    let (core, pre) = match core_and_pre.split_once('-') {
        Some((c, p)) => (c, Some(p)),
        None => (core_and_pre, None),
    };

    let mut parts = core.split('.');
    let (Some(major), Some(minor), Some(patch), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return false;
    };
    for num in [major, minor, patch] {
        if num.is_empty()
            || !num.bytes().all(|b| b.is_ascii_digit())
            || (num.len() > 1 && num.starts_with('0'))
            || num.parse::<i64>().is_err()
        {
            return false;
        }
    }

    if let Some(pre) = pre {
        if pre.is_empty() {
            return false;
        }
        for ident in pre.split('.') {
            if ident.is_empty()
                || !ident
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-')
            {
                return false;
            }
            if ident.bytes().all(|b| b.is_ascii_digit())
                && ident.len() > 1
                && ident.starts_with('0')
            {
                return false;
            }
        }
    }

    if let Some(build) = build {
        if build.is_empty() {
            return false;
        }
        for ident in build.split('.') {
            if ident.is_empty()
                || !ident
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-')
            {
                return false;
            }
        }
    }

    true
}

/// Validates RFC 2396 / `java.net.URI` syntax via `ResourceUri::parse`, conservatively
/// rejecting opaque URIs and empty strings so non-standard cases fall back to `NativeHost`.
fn is_valid_static_uri(s: &str) -> bool {
    !s.is_empty()
        && crate::host::resources::package_components(&PklString::from(s))
            .is_ok_and(|components| !components.opaque)
}

fn is_java_dot(c: char) -> bool {
    !matches!(c, '\n' | '\r' | '\u{0085}' | '\u{2028}' | '\u{2029}')
}

fn is_java_non_whitespace(c: char) -> bool {
    !matches!(c, ' ' | '\t' | '\n' | '\u{000B}' | '\u{000C}' | '\r')
}

/// Conservative match for `Project.pkl:269`:
/// `typealias EmailAddress = String(matches(Regex(#".+@\S+|.+<\S+@\S+>"#)))`.
fn is_valid_email_address(s: &str) -> bool {
    if s.is_empty() || !s.chars().all(is_java_dot) {
        return false;
    }
    if s.match_indices('@').any(|(at, _)| {
        at > 0 && at + 1 < s.len() && s[at + 1..].chars().all(is_java_non_whitespace)
    }) {
        return true;
    }
    if let Some(before_gt) = s.strip_suffix('>') {
        for (open, _) in before_gt.match_indices('<') {
            if open > 0 {
                let inner = &before_gt[open + 1..];
                if inner.chars().all(is_java_non_whitespace)
                    && inner
                        .match_indices('@')
                        .any(|(at, _)| at > 0 && at + 1 < inner.len())
                {
                    return true;
                }
            }
        }
    }
    false
}

fn extract_static_object_body<'a>(
    expr: &'a crate::syntax::Expr,
    member_amendment: Option<&crate::syntax::Member>,
    allow_entry_amendment: bool,
) -> Option<&'a [crate::syntax::ObjectMember]> {
    use crate::syntax::ExprKind;
    // Do NOT strip parentheses: `(new { ... })` is rejected by Pkl 0.32.0
    // with `Cannot tell which parent to amend.`.
    let ExprKind::Object {
        parent,
        parameters,
        members,
    } = &expr.kind
    else {
        return None;
    };
    if !parameters.is_empty() {
        return None;
    }
    match parent {
        None => Some(members),
        Some(parent_expr) => {
            if let Some(member) = member_amendment
                && member.is_amendment()
                && parent_expr.span == member.name_span
            {
                return Some(members);
            }
            if allow_entry_amendment && matches!(parent_expr.kind, ExprKind::EntryParent) {
                return Some(members);
            }
            None
        }
    }
}

fn parse_static_string_listing(
    expr: &crate::syntax::Expr,
    member_amendment: Option<&crate::syntax::Member>,
) -> Option<Vec<String>> {
    use crate::syntax::ObjectMember;
    let members = extract_static_object_body(expr, member_amendment, false)?;
    let mut out = Vec::with_capacity(members.len());
    for om in members {
        let ObjectMember::Element { value, .. } = om else {
            return None;
        };
        out.push(static_string_expr(value)?);
    }
    Some(out)
}

fn parse_static_remote_dep(
    dep_name: &str,
    members: &[crate::syntax::ObjectMember],
) -> Option<DeclaredRemoteDep> {
    use crate::syntax::ObjectMember;
    use std::collections::HashSet;

    let mut seen = HashSet::new();
    let mut uri_str: Option<String> = None;
    let mut sha256: Option<PklString> = None;

    for om in members {
        let ObjectMember::Property(prop) = om else {
            return None;
        };
        if prop.local
            || !prop.annotations.is_empty()
            || !prop.modifiers.is_empty()
            || prop.ty.is_some()
        {
            return None;
        }
        let field = prop.name.as_str()?;
        if !seen.insert(field) {
            return None;
        }
        match field {
            "uri" => {
                uri_str = Some(static_string_expr(&prop.expr)?);
            }
            "checksums" => {
                let cs_members = extract_static_object_body(&prop.expr, Some(prop), false)?;
                let mut cs_seen = HashSet::new();
                for cs_om in cs_members {
                    let ObjectMember::Property(cs_prop) = cs_om else {
                        return None;
                    };
                    if cs_prop.local
                        || !cs_prop.annotations.is_empty()
                        || !cs_prop.modifiers.is_empty()
                        || cs_prop.ty.is_some()
                    {
                        return None;
                    }
                    let cs_field = cs_prop.name.as_str()?;
                    if cs_field != "sha256" || !cs_seen.insert(cs_field) {
                        return None;
                    }
                    sha256 = Some(PklString::from(static_string_expr(&cs_prop.expr)?));
                }
                sha256.as_ref()?;
            }
            _ => return None,
        }
    }

    let uri_str = uri_str?;
    if !uri_str.starts_with("package://")
        || uri_str.contains('#')
        || uri_str.contains('?')
        || !is_valid_static_uri(&uri_str)
    {
        return None;
    }
    let (_, version_suffix) = uri_str.rsplit_once('@')?;
    if !is_strict_semver(version_suffix) {
        return None;
    }
    let package_uri = PackageUri::parse(uri_str.as_str()).ok()?;
    package_uri.check_has_no_checksum_component().ok()?;

    Some(DeclaredRemoteDep {
        name: dep_name.to_string(),
        package_uri,
        sha256,
    })
}

fn resolve_package_string_expr(
    expr: &crate::syntax::Expr,
    known: &std::collections::HashMap<&str, String>,
) -> Option<String> {
    use crate::syntax::ExprKind;
    match &expr.unparenthesized().kind {
        ExprKind::String(s) => s.as_str().map(str::to_string),
        ExprKind::InterpolatedString(parts) => {
            let mut out = String::new();
            for part in parts {
                match &part.unparenthesized().kind {
                    ExprKind::String(s) => out.push_str(s.as_str()?),
                    ExprKind::Name(var) => {
                        let key = var.as_str()?;
                        out.push_str(known.get(key)?);
                    }
                    _ => return None,
                }
            }
            Some(out)
        }
        _ => None,
    }
}

fn parse_static_package(members: &[crate::syntax::ObjectMember]) -> Option<PackageSpec> {
    use crate::syntax::{Expr, ObjectMember};
    use std::collections::{HashMap, HashSet};

    let mut seen = HashSet::new();
    let mut raw_strings: HashMap<&str, &Expr> = HashMap::new();
    let mut authors: Vec<String> = Vec::new();
    let mut api_tests: Vec<String> = Vec::new();
    let mut exclude: Vec<String> = vec![
        "PklProject".to_string(),
        "PklProject.deps.json".to_string(),
        ".**".to_string(),
    ];

    for om in members {
        let ObjectMember::Property(prop) = om else {
            return None;
        };
        if prop.local
            || !prop.annotations.is_empty()
            || !prop.modifiers.is_empty()
            || prop.ty.is_some()
        {
            return None;
        }
        let field = prop.name.as_str()?;
        if !seen.insert(field) {
            return None;
        }
        match field {
            "name"
            | "baseUri"
            | "version"
            | "packageZipUrl"
            | "description"
            | "website"
            | "documentation"
            | "sourceCode"
            | "sourceCodeUrlScheme"
            | "license"
            | "licenseText"
            | "issueTracker" => {
                raw_strings.insert(field, &prop.expr);
            }
            "authors" => {
                let items = parse_static_string_listing(&prop.expr, Some(prop))?;
                if !items.iter().all(|s| is_valid_email_address(s)) {
                    return None;
                }
                authors = items;
            }
            "apiTests" => {
                let items = parse_static_string_listing(&prop.expr, Some(prop))?;
                if !all_distinct(&items) {
                    return None;
                }
                api_tests = items;
            }
            "exclude" => {
                let items = parse_static_string_listing(&prop.expr, Some(prop))?;
                if prop.is_amendment() {
                    exclude.extend(items);
                } else {
                    exclude = items;
                }
            }
            _ => return None,
        }
    }

    // Resolve plain string literals first, then iteratively resolve interpolated fields
    // (including derived `uri` once `baseUri` and `version` are known).
    let mut resolved: HashMap<&str, String> = HashMap::with_capacity(raw_strings.len() + 1);
    for (&field, &expr) in &raw_strings {
        if let Some(s) = static_string_expr(expr) {
            resolved.insert(field, s);
        }
    }
    loop {
        if !resolved.contains_key("uri")
            && let (Some(base_uri), Some(version)) =
                (resolved.get("baseUri"), resolved.get("version"))
        {
            resolved.insert("uri", format!("{base_uri}@{version}"));
        }
        let mut progressed = false;
        for (&field, &expr) in &raw_strings {
            if !resolved.contains_key(field)
                && let Some(s) = resolve_package_string_expr(expr, &resolved)
            {
                resolved.insert(field, s);
                progressed = true;
            }
        }
        if !progressed {
            break;
        }
    }
    if raw_strings
        .keys()
        .any(|field| !resolved.contains_key(field))
    {
        return None;
    }

    let name = resolved.remove("name")?;
    let base_uri = resolved.remove("baseUri")?;
    let version_str = resolved.remove("version")?;
    let package_zip_url = resolved.remove("packageZipUrl")?;

    if !base_uri.starts_with("package://")
        || base_uri.contains('?')
        || base_uri.contains('#')
        || !is_valid_static_uri(&base_uri)
    {
        return None;
    }
    if !package_zip_url.starts_with("https://") || !is_valid_static_uri(&package_zip_url) {
        return None;
    }
    if !is_strict_semver(&version_str) {
        return None;
    }
    let version = PackageVersion::parse(version_str.as_str()).ok()?;
    let uri_str = format!("{base_uri}@{version_str}");
    let uri = PackageUri::parse(uri_str.as_str()).ok()?;
    uri.check_has_no_checksum_component().ok()?;

    let documentation = resolved.remove("documentation");
    if let Some(ref doc) = documentation
        && (doc.ends_with('/') || !is_valid_static_uri(doc))
    {
        return None;
    }
    let website = resolved.remove("website");
    if let Some(ref url) = website
        && !is_valid_static_uri(url)
    {
        return None;
    }
    let source_code = resolved.remove("sourceCode");
    if let Some(ref url) = source_code
        && !is_valid_static_uri(url)
    {
        return None;
    }
    let issue_tracker = resolved.remove("issueTracker");
    if let Some(ref url) = issue_tracker
        && !is_valid_static_uri(url)
    {
        return None;
    }

    Some(PackageSpec {
        name,
        base_uri,
        version,
        uri,
        package_zip_url,
        description: resolved.remove("description"),
        authors,
        website,
        documentation,
        source_code,
        source_code_url_scheme: resolved.remove("sourceCodeUrlScheme"),
        license: resolved.remove("license"),
        license_text: resolved.remove("licenseText"),
        issue_tracker,
        api_tests,
        exclude,
    })
}

#[cfg(test)]
mod static_project_tests {
    use super::*;

    #[test]
    fn static_project_parses_remote_local_and_package_with_interpolation() {
        let dep_src = r#"
amends "pkl:Project"

package {
  name = "example-lib"
  baseUri = "package://pkg.pkl-lang.org/github.com/example/example-lib/example-lib"
  version = "2.0.0-rc.1"
  packageZipUrl = "https://github.com/example/example-lib/releases/download/\(version)/example-lib@\(version).zip"
  description = "Core Library"
  authors {
    "Example Authors <info@example.com>"
  }
  license = "MIT"
}
"#;
        let root_src = r#"
amends "pkl:Project"

dependencies {
  ["example-lib"] = import("../spec/PklProject")
  ["remote"] {
    uri = "package://example.com/foo@1.2.3"
  }
}
"#;
        let project = try_parse_static_project(
            "file:///workspace/proj/PklProject",
            root_src,
            &mut |_base, import_uri| {
                assert_eq!(import_uri, "../spec/PklProject");
                Some((
                    "file:///workspace/spec/PklProject".to_string(),
                    dep_src.to_string(),
                ))
            },
        )
        .expect("static project should parse");

        assert_eq!(
            project.project_file_uri,
            "file:///workspace/proj/PklProject"
        );
        assert_eq!(project.project_base_uri, "file:///workspace/proj/");
        assert_eq!(project.declared_dependencies.local_dependencies.len(), 1);
        assert_eq!(project.declared_dependencies.remote_dependencies.len(), 1);
        assert_eq!(project.local_project_dependencies.len(), 1);
        let (_, child) = &project.local_project_dependencies[0];
        let pkg = child.package.as_ref().unwrap();
        assert_eq!(pkg.name, "example-lib");
        assert_eq!(
            pkg.package_zip_url,
            "https://github.com/example/example-lib/releases/download/2.0.0-rc.1/example-lib@2.0.0-rc.1.zip"
        );
        assert_eq!(
            pkg.exclude,
            vec!["PklProject", "PklProject.deps.json", ".**"]
        );
    }

    #[test]
    fn static_project_falls_back_on_evaluator_settings_or_dynamic_expr() {
        let with_settings = r#"
amends "pkl:Project"
evaluatorSettings {
  noCache = true
}
"#;
        assert!(
            try_parse_static_project(
                "file:///workspace/PklProject",
                with_settings,
                &mut |_, _| None
            )
            .is_none()
        );

        let invalid_version = r#"
amends "pkl:Project"
package {
  name = "bad"
  baseUri = "package://example.com/bad"
  version = "not-a-semver"
  packageZipUrl = "https://example.com/bad.zip"
}
"#;
        assert!(
            try_parse_static_project(
                "file:///workspace/PklProject",
                invalid_version,
                &mut |_, _| None
            )
            .is_none()
        );
    }

    #[test]
    fn static_project_enforces_strict_semver_and_rejects_query_or_fragment_dep_uris() {
        for bad_ver in [
            "01.2.3",
            "1.02.3",
            "1.2.03",
            "1.2.3-01",
            "1.2.3-a_b",
            "1.2.3+a_b",
        ] {
            let pkg_src = format!(
                r#"amends "pkl:Project"
package {{
  name = "a"
  baseUri = "package://example.com/a"
  version = "{bad_ver}"
  packageZipUrl = "https://example.com/a.zip"
}}"#
            );
            assert!(
                try_parse_static_project("file:///workspace/PklProject", &pkg_src, &mut |_, _| {
                    None
                })
                .is_none(),
                "package version `{bad_ver}` must fall back"
            );

            let dep_src = format!(
                r#"amends "pkl:Project"
dependencies {{
  ["a"] {{
    uri = "package://example.com/a@{bad_ver}"
  }}
}}"#
            );
            assert!(
                try_parse_static_project("file:///workspace/PklProject", &dep_src, &mut |_, _| {
                    None
                })
                .is_none(),
                "remote dep version `{bad_ver}` must fall back"
            );
        }

        for bad_dep_uri in [
            "package://example.com/a@1.2.3#/foo.pkl",
            "package://example.com/a@1.2.3?q=1",
            "package://example.com/a@1.2.3?q=@1.2.3",
            "package://example.com/a@1.2.3#foo@1.2.3",
        ] {
            let dep_src = format!(
                r#"amends "pkl:Project"
dependencies {{
  ["a"] {{
    uri = "{bad_dep_uri}"
  }}
}}"#
            );
            assert!(
                try_parse_static_project("file:///workspace/PklProject", &dep_src, &mut |_, _| {
                    None
                })
                .is_none(),
                "remote dep uri `{bad_dep_uri}` must fall back"
            );
        }

        let valid_semver_src = r#"amends "pkl:Project"
package {
  name = "ok"
  baseUri = "package://example.com/ok"
  version = "1.2.3-0a.0+01.build-2"
  packageZipUrl = "https://example.com/ok.zip"
}
dependencies {
  ["dep"] {
    uri = "package://example.com/dep@0.1.0-alpha.1+001"
  }
}"#;
        assert!(
            try_parse_static_project(
                "file:///workspace/PklProject",
                valid_semver_src,
                &mut |_, _| None
            )
            .is_some()
        );
    }

    #[test]
    fn static_project_rejects_parenthesized_objects_bad_emails_and_bad_uris() {
        for bad_paren in [
            "amends \"pkl:Project\"\ntests = (new { \"a.pkl\" })\n",
            "amends \"pkl:Project\"\ndependencies = (new { [\"a\"] { uri = \"package://example.com/a@1.0.0\" } })\n",
            "amends \"pkl:Project\"\ndependencies { [\"a\"] = (new { uri = \"package://example.com/a@1.0.0\" }) }\n",
            "amends \"pkl:Project\"\ndependencies { [\"a\"] { uri = \"package://example.com/a@1.0.0\"; checksums = (new { sha256 = \"abc\" }) } }\n",
            "amends \"pkl:Project\"\npackage = (new { name = \"a\"; baseUri = \"package://example.com/a\"; version = \"1.0.0\"; packageZipUrl = \"https://example.com/a.zip\" })\n",
            "amends \"pkl:Project\"\npackage { name = \"a\"; baseUri = \"package://example.com/a\"; version = \"1.0.0\"; packageZipUrl = \"https://example.com/a.zip\"; authors = (new { \"a@b.com\" }) }\n",
        ] {
            assert!(
                try_parse_static_project("file:///workspace/PklProject", bad_paren, &mut |_, _| {
                    None
                })
                .is_none(),
                "parenthesized object must fall back:\n{bad_paren}"
            );
        }

        for bad_email in [
            "a@b\\u{0b}c",
            "Name <a@b\\u{0b}c.com>",
            "a\\u{2028}b@c.com",
            "a\\u{0085}@b.com",
            "Name\\u{2029} <a@b.com>",
        ] {
            let src = format!(
                "amends \"pkl:Project\"\npackage {{\n  name = \"a\"\n  baseUri = \"package://example.com/a\"\n  version = \"1.0.0\"\n  packageZipUrl = \"https://example.com/a.zip\"\n  authors {{ \"{bad_email}\" }}\n}}\n"
            );
            assert!(
                try_parse_static_project("file:///workspace/PklProject", &src, &mut |_, _| None)
                    .is_none(),
                "invalid email `{bad_email}` must fall back"
            );
        }
        assert!(is_valid_email_address("a@b c@d"));
        assert!(is_valid_email_address("Name <a@b.com>"));
        assert!(!is_valid_email_address("a@b\u{0b}c"));
        assert!(!is_valid_email_address("Name <a@b\u{0b}c.com>"));
        assert!(!is_valid_email_address("a\u{2028}b@c.com"));
        assert!(!is_valid_email_address("a\u{0085}@b.com"));
        assert!(!is_valid_email_address("Name\u{2029} <a@b.com>"));

        for bad_uri_field in [
            r#"packageZipUrl = "https://example.com/%ZZ.zip""#,
            r#"packageZipUrl = "https://example.com/a[0].zip""#,
            r#"packageZipUrl = "https://example.com/a#b#c""#,
            r#"packageZipUrl = "https:foo://bar""#,
            r#"packageZipUrl = "https://example.com/a.zip"; documentation = "https://example.com/%ZZ""#,
            r#"packageZipUrl = "https://example.com/a.zip"; documentation = "https://example.com/a[0]""#,
            r#"packageZipUrl = "https://example.com/a.zip"; website = "https://example.com/%ZZ""#,
            r#"packageZipUrl = "https://example.com/a.zip"; sourceCode = "https://example.com/a[0]""#,
            r#"packageZipUrl = "https://example.com/a.zip"; issueTracker = "https://example.com/a#b#c""#,
        ] {
            let src = format!(
                "amends \"pkl:Project\"\npackage {{\n  name = \"a\"\n  baseUri = \"package://example.com/a\"\n  version = \"1.0.0\"\n  {bad_uri_field}\n}}\n"
            );
            assert!(
                try_parse_static_project("file:///workspace/PklProject", &src, &mut |_, _| None)
                    .is_none(),
                "invalid URI field `{bad_uri_field}` must fall back"
            );
        }

        let bad_test_uri = "amends \"pkl:Project\"\ntests { \"a b.pkl\" }\n";
        assert!(
            try_parse_static_project("file:///workspace/PklProject", bad_test_uri, &mut |_, _| {
                None
            })
            .is_none()
        );
    }

    #[test]
    fn static_project_memoizes_diamond_dag_and_bounds_cyclic_graphs() {
        let leaf_src = r#"amends "pkl:Project"
package {
  name = "leaf"
  baseUri = "package://example.com/leaf"
  version = "1.0.0"
  packageZipUrl = "https://example.com/leaf.zip"
}"#;
        let node_src = |idx: usize, next: usize| {
            format!(
                r#"amends "pkl:Project"
package {{
  name = "n{idx}"
  baseUri = "package://example.com/n{idx}"
  version = "1.0.0"
  packageZipUrl = "https://example.com/n{idx}.zip"
}}
dependencies {{
  ["a"] = import("../l{next}a/PklProject")
  ["b"] = import("../l{next}b/PklProject")
}}"#
            )
        };

        // 1. Depth-10 diamond ladder: 2^10 = 1024 paths, but memoization keeps reads linear.
        let mut reads = 0usize;
        let root_src = node_src(0, 1);
        let parsed = try_parse_static_project(
            "file:///workspace/root/PklProject",
            &root_src,
            &mut |_base, import_uri| {
                reads += 1;
                let dir = import_uri
                    .strip_prefix("../")?
                    .strip_suffix("/PklProject")?;
                let level: usize = dir[1..dir.len() - 1].parse().ok()?;
                let uri = format!("file:///workspace/{dir}/PklProject");
                let src = if level >= 10 {
                    leaf_src.to_string()
                } else {
                    node_src(level, level + 1)
                };
                Some((uri, src))
            },
        );
        assert!(parsed.is_some(), "diamond DAG should parse via memoization");
        assert!(
            reads <= 40,
            "expected linear reads on depth-10 diamond DAG, got {reads}"
        );

        // 2. Direct 2-node cycle is caught immediately by active set.
        let cyc_a = r#"amends "pkl:Project"
package { name = "a"; baseUri = "package://example.com/a"; version = "1.0.0"; packageZipUrl = "https://example.com/a.zip" }
dependencies { ["b"] = import("../b/PklProject") }"#;
        let cyc_b = r#"amends "pkl:Project"
package { name = "b"; baseUri = "package://example.com/b"; version = "1.0.0"; packageZipUrl = "https://example.com/b.zip" }
dependencies { ["a"] = import("../a/PklProject") }"#;
        let mut cyc_reads = 0usize;
        assert!(
            try_parse_static_project(
                "file:///workspace/a/PklProject",
                cyc_a,
                &mut |_base, import_uri| {
                    cyc_reads += 1;
                    if import_uri == "../b/PklProject" {
                        Some(("file:///workspace/b/PklProject".into(), cyc_b.into()))
                    } else {
                        Some(("file:///workspace/a/PklProject".into(), cyc_a.into()))
                    }
                }
            )
            .is_none()
        );
        assert_eq!(cyc_reads, 2);

        // 3. Branching symlink-aliased cycle with fresh non-canonical URIs is capped by visit budget.
        let branch_src = r#"amends "pkl:Project"
package { name = "s"; baseUri = "package://example.com/s"; version = "1.0.0"; packageZipUrl = "https://example.com/s.zip" }
dependencies {
  ["x"] = import("x/PklProject")
  ["y"] = import("y/PklProject")
}"#;
        let mut sym_reads = 0usize;
        assert!(
            try_parse_static_project(
                "file:///workspace/sym/PklProject",
                branch_src,
                &mut |base, import_uri| {
                    sym_reads += 1;
                    let prefix = base.strip_suffix("/PklProject")?;
                    Some((format!("{prefix}/{import_uri}"), branch_src.to_string()))
                }
            )
            .is_none()
        );
        assert!(
            sym_reads <= MAX_STATIC_PROJECT_VISITS,
            "branching symlink cycle must stay within visit budget, got {sym_reads}"
        );
    }
}
