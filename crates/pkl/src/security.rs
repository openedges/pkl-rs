//! SecurityManager SPI and standard policies mirroring `org.pkl.core.{SecurityManager, SecurityManagers}`.

use crate::error::{PklError, Result};
use pkl_host_native::CustomSecurityChecker;
use pkl_runtime::host::security::{
    DEFAULT_ALLOWED_MODULES, DEFAULT_ALLOWED_RESOURCES, SecurityPolicy,
};
use pkl_runtime::value::PklString;
use std::path::PathBuf;
use std::sync::Arc;

/// SecurityManager SPI controlling module resolution, imports, and resource reads
/// (`org.pkl.core.SecurityManager`).
pub trait SecurityManager: Send + Sync {
    /// Checks whether a module at `uri` may be resolved/loaded.
    fn check_resolve_module(&self, uri: &str) -> Result<()>;

    /// Checks whether `importing_module` may import `imported_module`.
    fn check_import_module(&self, importing_module: &str, imported_module: &str) -> Result<()>;

    /// Checks whether a resource at `resource_uri` may be read.
    fn check_read_resource(&self, resource_uri: &str) -> Result<()>;

    /// Checks whether a resource at `resource_uri` may be resolved (e.g. during globbing).
    fn check_resolve_resource(&self, resource_uri: &str) -> Result<()> {
        self.check_read_resource(resource_uri)
    }
}

/// Standard allowlist + `rootDir` + trust-level [`SecurityManager`] (`SecurityManagers.standard`).
#[derive(Clone, Debug)]
pub struct StandardSecurityManager {
    policy: SecurityPolicy,
}

impl StandardSecurityManager {
    pub fn new(
        allowed_modules: Option<&[String]>,
        allowed_resources: Option<&[String]>,
        root_dir: Option<PathBuf>,
    ) -> Result<Self> {
        let root_str = match root_dir {
            Some(dir) => {
                let real = pkl_host_native::canonicalize_root_dir(&dir).map_err(|err| {
                    PklError::configuration(format!(
                        "Failed to normalize rootDir `{}`: {err}",
                        dir.display()
                    ))
                })?;
                Some(real.to_string_lossy().into_owned())
            }
            None => None,
        };
        let policy = SecurityPolicy::new(allowed_modules, allowed_resources, root_str)
            .map_err(PklError::configuration)?;
        Ok(Self { policy })
    }

    pub fn policy(&self) -> &SecurityPolicy {
        &self.policy
    }
}

impl SecurityManager for StandardSecurityManager {
    fn check_resolve_module(&self, uri: &str) -> Result<()> {
        self.policy
            .check_resolve_module(&PklString::from(uri))
            .map_err(|err| PklError::security(format_module_host_security(err)))
    }

    fn check_import_module(&self, importing_module: &str, imported_module: &str) -> Result<()> {
        self.policy
            .check_import_module(
                &PklString::from(importing_module),
                &PklString::from(imported_module),
            )
            .map_err(|err| PklError::security(format_module_host_security(err)))
    }

    fn check_read_resource(&self, resource_uri: &str) -> Result<()> {
        self.policy
            .check_read_resource(&PklString::from(resource_uri))
            .map_err(|err| PklError::security(format!("{err:?}")))
    }
}

fn format_module_host_security(err: pkl_runtime::modules::ModuleHostError) -> String {
    match err {
        pkl_runtime::modules::ModuleHostError::SecurityManagerException { message } => {
            message.map(|m| m.to_java_utf8()).unwrap_or_default()
        }
        other => format!("{other:?}"),
    }
}

/// Factory helpers mirroring `org.pkl.core.SecurityManagers`.
pub struct SecurityManagers;

impl SecurityManagers {
    /// Default allowed module URI patterns (`SecurityManagers.defaultAllowedModules`).
    pub fn default_allowed_modules() -> Vec<String> {
        DEFAULT_ALLOWED_MODULES
            .iter()
            .map(|s| (*s).to_string())
            .collect()
    }

    /// Default allowed resource URI patterns (`SecurityManagers.defaultAllowedResources`).
    pub fn default_allowed_resources() -> Vec<String> {
        DEFAULT_ALLOWED_RESOURCES
            .iter()
            .map(|s| (*s).to_string())
            .collect()
    }

    /// Default standard security manager (`SecurityManagers.defaultManager`).
    pub fn default_manager() -> Arc<dyn SecurityManager> {
        Arc::new(
            StandardSecurityManager::new(None, None, None)
                .expect("default security policy compiles"),
        )
    }

    /// Standard security manager with explicit allowlists, trust-level checking, and optional `rootDir`.
    pub fn standard(
        allowed_modules: Vec<String>,
        allowed_resources: Vec<String>,
        root_dir: Option<PathBuf>,
    ) -> Result<Arc<dyn SecurityManager>> {
        Ok(Arc::new(StandardSecurityManager::new(
            Some(&allowed_modules),
            Some(&allowed_resources),
            root_dir,
        )?))
    }
}

/// Adapter bridging public [`SecurityManager`] to `pkl_host_native::CustomSecurityChecker`.
pub(crate) struct SecurityManagerAdapter(pub(crate) Arc<dyn SecurityManager>);

impl CustomSecurityChecker for SecurityManagerAdapter {
    fn check_resolve_module(&self, uri: &PklString) -> std::result::Result<(), PklString> {
        self.0
            .check_resolve_module(&uri.to_java_utf8())
            .map_err(|err| PklString::from(err.message()))
    }

    fn check_import_module(
        &self,
        importing_module: &PklString,
        imported_module: &PklString,
    ) -> std::result::Result<(), PklString> {
        self.0
            .check_import_module(
                &importing_module.to_java_utf8(),
                &imported_module.to_java_utf8(),
            )
            .map_err(|err| PklString::from(err.message()))
    }

    fn check_read_resource(&self, resource: &PklString) -> std::result::Result<(), PklString> {
        self.0
            .check_read_resource(&resource.to_java_utf8())
            .map_err(|err| PklString::from(err.message()))
    }

    fn check_resolve_resource(&self, resource: &PklString) -> std::result::Result<(), PklString> {
        self.0
            .check_resolve_resource(&resource.to_java_utf8())
            .map_err(|err| PklString::from(err.message()))
    }
}
