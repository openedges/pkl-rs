//! Wasm-pure conversion & client-callback helpers for `pkl server`.
//!
//! Converts `ServerProject` wire trees into `DeclaredDependenciesTree` and provides
//! `ClientCallbackRegistry` reusing `ExternalModuleResolver` and
//! `ExternalResourceResolver` directly over any `ExternalReaderTransport`.

use super::messages::{ServerDependency, ServerProject};
use crate::external_reader::{
    ExternalModuleResolver, ExternalReaderTransport, ExternalResourceResolver, ModuleReaderSpec,
    ResourceReaderSpec,
};
use crate::packages::PackageUri;
use crate::packages::project::{DeclaredDependenciesTree, DeclaredRemoteDep};
use crate::value::PklString;
use std::sync::Arc;
use std::sync::atomic::AtomicI64;

impl ServerProject {
    /// Converts a decoded `CreateEvaluatorRequest.project` tree into the runtime's
    /// [`DeclaredDependenciesTree`], matching `Server.buildDeclaredDependencies`.
    pub fn to_declared_dependencies_tree(&self) -> Result<DeclaredDependenciesTree, String> {
        let mut remote_dependencies = Vec::new();
        let mut local_dependencies = Vec::new();
        for (name, dep) in &self.dependencies {
            match dep {
                ServerDependency::Remote {
                    package_uri,
                    checksums,
                } => {
                    let parsed = PackageUri::parse(PklString::from(package_uri.as_str()))
                        .map_err(|e| format!("Invalid package URI `{package_uri}`: {e:?}"))?;
                    remote_dependencies.push(DeclaredRemoteDep {
                        name: name.clone(),
                        package_uri: parsed,
                        sha256: checksums
                            .as_ref()
                            .map(|cs| PklString::from(cs.sha256.as_str())),
                    });
                }
                ServerDependency::Local(local_proj) => {
                    let child = local_proj.to_declared_dependencies_tree()?;
                    local_dependencies.push((name.clone(), child));
                }
            }
        }
        let package_uri = match &self.package_uri {
            Some(uri) => Some(
                PackageUri::parse(PklString::from(uri.as_str()))
                    .map_err(|e| format!("Invalid package URI `{uri}`: {e:?}"))?,
            ),
            None => None,
        };
        Ok(DeclaredDependenciesTree {
            project_file_uri: self.project_file_uri.clone(),
            package_uri,
            remote_dependencies,
            local_dependencies,
        })
    }
}

/// Wasm-pure client callback reader registry for one `pkl server` evaluator.
///
/// Mirrors `org.pkl.server.ClientModuleKeyFactory` + `Server.createResourceReaders`:
/// shares one [`ExternalModuleResolver`] across all `clientModuleReaders`, and holds
/// one [`ExternalResourceResolver`] per `clientResourceReaders` entry, all backed by
/// the same client `ExternalReaderTransport` and `evaluator_id`.
pub struct ClientCallbackRegistry {
    module_specs: Vec<ModuleReaderSpec>,
    module_resolver: Option<Arc<ExternalModuleResolver>>,
    resource_readers: Vec<(ResourceReaderSpec, Arc<ExternalResourceResolver>)>,
}

impl ClientCallbackRegistry {
    pub fn new(
        evaluator_id: i64,
        transport: Arc<dyn ExternalReaderTransport>,
        request_id_counter: Arc<AtomicI64>,
        module_specs: Vec<ModuleReaderSpec>,
        resource_specs: Vec<ResourceReaderSpec>,
    ) -> Self {
        let module_resolver = if module_specs.is_empty() {
            None
        } else {
            Some(Arc::new(ExternalModuleResolver::with_request_id_counter(
                Arc::clone(&transport),
                evaluator_id,
                Arc::clone(&request_id_counter),
            )))
        };
        let resource_readers = resource_specs
            .into_iter()
            .map(|spec| {
                let resolver = Arc::new(ExternalResourceResolver::with_request_id_counter(
                    Arc::clone(&transport),
                    evaluator_id,
                    Arc::clone(&request_id_counter),
                ));
                (spec, resolver)
            })
            .collect();
        Self {
            module_specs,
            module_resolver,
            resource_readers,
        }
    }

    pub fn module_specs(&self) -> &[ModuleReaderSpec] {
        &self.module_specs
    }

    pub fn module_resolver(&self) -> Option<&Arc<ExternalModuleResolver>> {
        self.module_resolver.as_ref()
    }

    pub fn resource_readers(&self) -> &[(ResourceReaderSpec, Arc<ExternalResourceResolver>)] {
        &self.resource_readers
    }

    /// Infers a Pkl module name for a client-provided module URI (hierarchical or opaque).
    pub fn infer_module_name(
        uri: &crate::value::PklString,
    ) -> Result<crate::value::PklString, crate::value::RuntimeError> {
        crate::host::resources::infer_module_name(uri, false)
    }

    pub fn find_module_spec(&self, scheme: &str) -> Option<&ModuleReaderSpec> {
        self.module_specs.iter().find(|spec| spec.scheme == scheme)
    }

    pub fn find_resource_reader(
        &self,
        scheme: &str,
    ) -> Option<(&ResourceReaderSpec, &Arc<ExternalResourceResolver>)> {
        self.resource_readers
            .iter()
            .rev()
            .find(|(spec, _)| spec.scheme.eq_ignore_ascii_case(scheme))
            .map(|(spec, resolver)| (spec, resolver))
    }
}
