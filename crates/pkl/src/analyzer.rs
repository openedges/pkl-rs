//! Static module import-graph analysis mirroring `org.pkl.core.{Analyzer, ImportGraph}`.

use crate::error::{PklError, Result, format_analysis_failure};
use crate::frame::{HostFrameTransformerAdapter, StackFrameTransformer};
use crate::http::HttpClient;
use crate::project::Project;
use crate::reader::{ModuleKeyFactory, ModuleKeyFactoryAdapter};
use crate::security::{SecurityManager, SecurityManagerAdapter, StandardSecurityManager};
use crate::settings::TraceMode;
use pkl_host_native::{NativeHost, NativeHostConfig, NativeHostExtensions};
use pkl_runtime::packages::project::ProjectDependenciesManager;
use pkl_runtime::value::PklString;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// A single imported module URI in an [`ImportGraph`] (`org.pkl.core.ImportGraph.Import`).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Import {
    pub uri: String,
}

/// A single node in an [`ImportGraph`] in canonical Java URI-component sort order.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ImportNode {
    /// The module's in-language URI.
    pub uri: String,
    /// The module's resolved URI.
    pub resolved_uri: String,
    /// Direct imports declared by this module.
    pub imports: Vec<String>,
}

/// Static import graph returned by [`Analyzer::import_graph`] (`org.pkl.core.ImportGraph`).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ImportGraph {
    /// Map from each module URI in the graph to the set of URIs it directly imports.
    pub imports: BTreeMap<String, BTreeSet<Import>>,
    /// Map from each module's in-language URI to its resolved URI.
    pub resolved_imports: BTreeMap<String, String>,
    /// Nodes in canonical Java URI-component sort order as produced by `pkl-runtime`.
    pub nodes: Vec<ImportNode>,
}

impl ImportGraph {
    pub(crate) fn from_runtime(raw: pkl_runtime::modules::analyze::ImportGraph) -> Self {
        let mut imports = BTreeMap::new();
        let mut resolved_imports = BTreeMap::new();
        let mut nodes = Vec::with_capacity(raw.nodes.len());
        for node in raw.nodes {
            let uri = node.uri.to_java_utf8();
            let resolved_uri = node.resolved_uri.to_java_utf8();
            let import_uris: Vec<String> =
                node.imports.iter().map(PklString::to_java_utf8).collect();
            let targets: BTreeSet<Import> = import_uris
                .iter()
                .map(|imp| Import { uri: imp.clone() })
                .collect();
            imports.insert(uri.clone(), targets);
            resolved_imports.insert(uri.clone(), resolved_uri.clone());
            nodes.push(ImportNode {
                uri,
                resolved_uri,
                imports: import_uris,
            });
        }
        Self {
            imports,
            resolved_imports,
            nodes,
        }
    }
}

/// Builder for [`Analyzer`] (`org.pkl.core.Analyzer`).
#[derive(Default)]
pub struct AnalyzerBuilder {
    allowed_modules: Option<Vec<String>>,
    allowed_resources: Option<Vec<String>>,
    root_dir: Option<PathBuf>,
    module_paths: Vec<PathBuf>,
    module_cache_dir: Option<PathBuf>,
    project_dependencies: Option<ProjectDependenciesManager>,
    security_manager: Option<Arc<dyn SecurityManager>>,
    frame_transformer: Option<Arc<dyn StackFrameTransformer>>,
    custom_module_factories: Vec<Arc<dyn ModuleKeyFactory>>,
    http_client: Option<HttpClient>,
    trace_mode: TraceMode,
    use_color: bool,
}

impl AnalyzerBuilder {
    pub fn preconfigured() -> Self {
        Self {
            module_cache_dir: pkl_host_native::default_module_cache_dir(),
            ..Default::default()
        }
    }

    pub fn with_allowed_modules(mut self, patterns: Vec<String>) -> Self {
        self.allowed_modules = Some(patterns);
        self
    }

    pub fn with_allowed_resources(mut self, patterns: Vec<String>) -> Self {
        self.allowed_resources = Some(patterns);
        self
    }

    pub fn with_root_dir(mut self, root_dir: Option<PathBuf>) -> Self {
        self.root_dir = root_dir;
        self
    }

    pub fn with_module_paths(mut self, module_paths: Vec<PathBuf>) -> Self {
        self.module_paths = module_paths;
        self
    }

    pub fn with_module_cache_dir(mut self, dir: Option<PathBuf>) -> Self {
        self.module_cache_dir = dir;
        self
    }

    pub fn with_project(mut self, project: &Project) -> Self {
        if let Some(settings) = project.evaluator_settings() {
            if let Some(ref allowed) = settings.allowed_modules {
                self.allowed_modules = Some(allowed.clone());
            }
            if let Some(ref allowed) = settings.allowed_resources {
                self.allowed_resources = Some(allowed.clone());
            }
            if let Some(ref root) = settings.root_dir {
                self.root_dir = Some(PathBuf::from(root));
            }
            if let Some(ref paths) = settings.module_path {
                self.module_paths = paths.iter().map(PathBuf::from).collect();
            }
            if settings.no_cache == Some(true) {
                self.module_cache_dir = None;
            } else if let Some(ref dir) = settings.module_cache_dir {
                self.module_cache_dir = Some(PathBuf::from(dir));
            }
            if let Some(color) = settings.color {
                self.use_color = crate::settings::Color::from(color).has_color();
            }
            if let Some(ref mode) = settings.trace_mode {
                if mode == "pretty" {
                    self.trace_mode = TraceMode::Pretty;
                } else if mode == "compact" {
                    self.trace_mode = TraceMode::Compact;
                }
            }
        }
        self.project_dependencies = project.dependencies_manager().cloned();
        self
    }

    #[allow(dead_code)]
    pub(crate) fn with_project_dependencies(mut self, deps: ProjectDependenciesManager) -> Self {
        self.project_dependencies = Some(deps);
        self
    }

    pub fn with_security_manager(mut self, security_manager: Arc<dyn SecurityManager>) -> Self {
        self.security_manager = Some(security_manager);
        self
    }

    pub fn with_stack_frame_transformer(
        mut self,
        transformer: Arc<dyn StackFrameTransformer>,
    ) -> Self {
        self.frame_transformer = Some(transformer);
        self
    }

    pub fn add_module_key_factory(mut self, factory: Arc<dyn ModuleKeyFactory>) -> Self {
        self.custom_module_factories.push(factory);
        self
    }

    pub fn with_http_client(mut self, client: HttpClient) -> Self {
        self.http_client = Some(client);
        self
    }

    pub fn with_trace_mode(mut self, trace_mode: TraceMode) -> Self {
        self.trace_mode = trace_mode;
        self
    }

    pub fn with_color(mut self, use_color: bool) -> Self {
        self.use_color = use_color;
        self
    }

    pub fn build(self) -> Result<Analyzer> {
        let security_for_readers: Option<Arc<dyn SecurityManager>> =
            if !self.custom_module_factories.is_empty() {
                Some(match &self.security_manager {
                    Some(sec) => sec.clone(),
                    None => Arc::new(StandardSecurityManager::new(
                        self.allowed_modules.as_deref(),
                        self.allowed_resources.as_deref(),
                        self.root_dir.clone(),
                    )?),
                })
            } else {
                None
            };
        let config = NativeHostConfig {
            allowed_modules: self.allowed_modules,
            allowed_resources: self.allowed_resources,
            root_dir: self.root_dir,
            module_paths: self.module_paths,
            module_cache_dir: self.module_cache_dir,
            project_dependencies: self.project_dependencies,
            trace_mode: self.trace_mode,
            ..Default::default()
        };
        let mut ext = NativeHostExtensions::default().with_color(self.use_color);
        if let Some(sec) = self.security_manager {
            ext = ext.with_custom_security(Arc::new(SecurityManagerAdapter(sec)));
        }
        if let Some(tf) = self.frame_transformer {
            ext = ext.with_frame_transformer(Arc::new(HostFrameTransformerAdapter(tf)), true);
        }
        if let Some(http) = self.http_client {
            ext = ext.with_http_client(http.inner());
        }
        if let Some(ref security) = security_for_readers {
            for factory in self.custom_module_factories {
                ext = ext.with_custom_module_reader(Arc::new(ModuleKeyFactoryAdapter {
                    factory,
                    security: security.clone(),
                }));
            }
        }
        let host = NativeHost::with_extensions(config, ext).map_err(PklError::configuration)?;
        Ok(Analyzer {
            host,
            closed: AtomicBool::new(false),
        })
    }
}

impl fmt::Debug for AnalyzerBuilder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AnalyzerBuilder")
            .field("root_dir", &self.root_dir)
            .field("module_paths", &self.module_paths)
            .field("module_cache_dir", &self.module_cache_dir)
            .field("trace_mode", &self.trace_mode)
            .field("use_color", &self.use_color)
            .finish_non_exhaustive()
    }
}

/// Static Pkl module import analyzer (`org.pkl.core.Analyzer`).
pub struct Analyzer {
    host: NativeHost,
    closed: AtomicBool,
}

impl fmt::Debug for Analyzer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Analyzer")
            .field("closed", &self.closed.load(Ordering::Acquire))
            .finish_non_exhaustive()
    }
}

impl Analyzer {
    pub fn builder() -> AnalyzerBuilder {
        AnalyzerBuilder::preconfigured()
    }

    /// Analyzes the static import graph reachable from `uris` (`Analyzer.importGraph`).
    pub fn import_graph<I, S>(&self, uris: I) -> Result<ImportGraph>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        if self.closed.load(Ordering::Acquire) {
            return Err(PklError::closed("Analyzer has been closed."));
        }
        let root_uris: Vec<PklString> = uris
            .into_iter()
            .map(|s| PklString::from(s.into()))
            .collect();
        self.host
            .analyze_import_graph(root_uris)
            .map(ImportGraph::from_runtime)
            .map_err(|failure| {
                let msg = format_analysis_failure(&failure);
                if matches!(
                    failure.cause,
                    pkl_runtime::modules::analyze::FailureCause::Host(
                        pkl_runtime::modules::ModuleHostError::SecurityManagerException { .. }
                    )
                ) {
                    PklError::security(msg)
                } else {
                    PklError::evaluation(msg)
                }
            })
    }

    pub fn close(&self) {
        if !self.closed.swap(true, Ordering::AcqRel) {
            self.host.close();
        }
    }
}

impl Drop for Analyzer {
    fn drop(&mut self) {
        self.close();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn analyzer_returns_closed_error_after_close() {
        let analyzer = Analyzer::builder().build().expect("build analyzer");
        analyzer.close();
        let err = analyzer
            .import_graph(["pkl:base"])
            .expect_err("closed analyzer must fail");
        assert_eq!(err.kind(), crate::PklErrorKind::Closed);
    }
}
