//! Exact port of `DocScope.kt` and `MarkdownParserContext` (`Markdown.kt:41-98`).

use crate::markdown::RefLinkResolution;
use crate::model::{DocPackage, DocPackageInfo};
use crate::util::{
    dir_to_file_uri_with_slash, encode_path, get_module_path, relativize_uri,
    replace_source_code_placeholders, resolve_uri, uri_encode, uri_encode_component,
};
use pkl_runtime::schema::{
    ClassId, ModuleSchema, PClass, PClassInfo, PClassMethod, PClassProperty, SourceLocation,
    TypeAlias, TypeAliasId, TypeAliasInfo,
};
use std::collections::{BTreeMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;

pub const PKL_VERSION: &str = "0.32.0";
pub const PACKAGE_INDEX_HOMEPAGE: &str = "https://pkl-lang.org/package-docs/";
pub const STDLIB_SOURCE_CODE_HOMEPAGE: &str = "https://github.com/apple/pkl/";
pub const STDLIB_SOURCE_CODE_URL_SCHEME: &str =
    "https://github.com/apple/pkl/blob/0.32.0/stdlib%{path}#L%{line}-L%{endLine}";

pub fn stdlib_package_page_url(version: &str) -> String {
    format!("{PACKAGE_INDEX_HOMEPAGE}pkl/{version}/")
}

/// Identity key mirroring `DocScope.equals` / `hashCode` (`DocScope.kt`) used for
/// `seenLinkTargets` deduplication and self-link suppression in `MarkdownParserContext`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ScopeKey {
    Site,
    Package(String),
    Module(String),
    Class(PClassInfo),
    TypeAlias(TypeAliasInfo),
    Method { owner: PClassInfo, name: String },
    Property { owner: PClassInfo, name: String },
    Parameter { name: String, parent: Box<ScopeKey> },
}

/// Shared site-wide resolution context (`SiteScope` in `DocScope.kt:127`).
pub struct SiteScope {
    pub doc_packages: Vec<DocPackage>,
    pub overview_imports: Vec<(String, String)>,
    pub imported_modules: BTreeMap<String, ModuleSchema>,
    pub output_dir: PathBuf,
    pub url: String,
    pub pkl_base_module: ModuleSchema,
    pub stdlib_package_index: Option<usize>,
    pub package_scopes: Vec<Arc<PackageScope>>,
}

impl SiteScope {
    pub fn new(
        doc_packages: Vec<DocPackage>,
        overview_imports: Vec<(String, String)>,
        imported_modules: BTreeMap<String, ModuleSchema>,
        output_dir: PathBuf,
    ) -> Arc<Self> {
        let base_uri = dir_to_file_uri_with_slash(&output_dir);
        let url = resolve_uri(&base_uri, "index.html");
        let pkl_base_module = imported_modules
            .get("pkl:base")
            .cloned()
            .expect("SiteScope requires `pkl:base` in imported_modules");
        let stdlib_package_index = doc_packages.iter().position(|p| p.name() == "pkl");

        let mut package_scopes = Vec::with_capacity(doc_packages.len());
        for pkg in &doc_packages {
            package_scopes.push(Arc::new(PackageScope::new(
                Arc::clone(&pkg.doc_package_info),
                pkg.doc_modules.iter().map(|m| m.schema.clone()).collect(),
                &url,
            )));
        }

        Arc::new(Self {
            doc_packages,
            overview_imports,
            imported_modules,
            output_dir,
            url,
            pkl_base_module,
            stdlib_package_index,
            package_scopes,
        })
    }

    pub fn stdlib_package_url(&self) -> Option<&str> {
        self.stdlib_package_index
            .map(|idx| self.package_scopes[idx].url.as_str())
    }

    pub fn package_scope(&self, pkg_index: usize) -> Arc<PackageScope> {
        Arc::clone(&self.package_scopes[pkg_index])
    }

    pub fn create_empty_package_scope(
        &self,
        name: String,
        version: String,
        source_code_url_scheme: Option<String>,
        source_code: Option<String>,
    ) -> Arc<PackageScope> {
        let info = Arc::new(DocPackageInfo {
            module_name_prefix: format!("{name}."),
            name,
            uri: None,
            version,
            import_uri: String::new(),
            authors: Some(Vec::new()),
            source_code,
            source_code_url_scheme,
            issue_tracker: None,
            dependencies: Vec::new(),
            overview: None,
            overview_imports: Vec::new(),
            annotations: Vec::new(),
            extra_attributes: Vec::new(),
        });
        Arc::new(PackageScope::new(info, Vec::new(), &self.url))
    }

    pub fn resolve_import(&self, uri: &str) -> Option<&ModuleSchema> {
        self.imported_modules.get(uri)
    }

    pub fn resolve_module_name_to_doc_url(&self, name: &str) -> Option<String> {
        if let Some(rest) = name.strip_prefix("pkl.") {
            let package_page = match self.stdlib_package_url() {
                Some(local_url) => local_url.to_string(),
                None => stdlib_package_page_url(PKL_VERSION),
            };
            return Some(resolve_uri(&package_page, &format!("{rest}/")));
        }
        None
    }

    pub fn resolve_module_name_to_source_url(
        &self,
        name: &str,
        source_location: SourceLocation,
    ) -> Option<String> {
        if let Some(rest) = name.strip_prefix("pkl.") {
            let path = format!("/stdlib/{rest}.pkl");
            return Some(replace_source_code_placeholders(
                STDLIB_SOURCE_CODE_URL_SCHEME,
                &path,
                source_location.start_line,
                source_location.end_line,
            ));
        }
        None
    }

    fn pkl_base_scope(&self) -> ModuleScope {
        let base_url = self
            .resolve_module_name_to_doc_url("pkl.base")
            .expect("pkl.base doc url");
        ModuleScope::new(self.pkl_base_module.clone(), base_url, None)
    }

    pub fn as_doc_scope(self: &Arc<Self>) -> DocScope {
        DocScope::Site(Arc::clone(self))
    }
}

/// Package-level scope (`PackageScope` in `DocScope.kt:244`).
pub struct PackageScope {
    pub doc_package_info: Arc<DocPackageInfo>,
    pub url: String,
    pub per_package_data_url: String,
    pub per_package_version_data_url: String,
    pub module_scopes: BTreeMap<String, Arc<ModuleScope>>,
}

impl PackageScope {
    fn new(
        doc_package_info: Arc<DocPackageInfo>,
        modules: Vec<ModuleSchema>,
        site_url: &str,
    ) -> Self {
        let encoded_name = encode_path(&doc_package_info.name);
        let url = resolve_uri(
            site_url,
            &format!("./{encoded_name}/{}/index.html", doc_package_info.version),
        );
        let per_package_data_url =
            resolve_uri(site_url, &format!("./data/{encoded_name}/_/index.json"));
        let per_package_version_data_url = resolve_uri(
            site_url,
            &format!(
                "./data/{encoded_name}/{}/index.json",
                doc_package_info.version
            ),
        );

        let mut module_scopes = BTreeMap::new();
        for module in modules {
            let mod_rel = format!(
                "{}/index.html",
                uri_encode(&encode_path(&get_module_path(
                    &module.module_name,
                    &doc_package_info.module_name_prefix
                )))
            );
            let doc_url = resolve_uri(&url, &mod_rel);
            let mscope = Arc::new(ModuleScope {
                module: module.clone(),
                url: doc_url,
                parent_info: Some(Arc::clone(&doc_package_info)),
                parent_url: Some(url.clone()),
                parent_per_package_data_url: Some(per_package_data_url.clone()),
                parent_per_package_version_data_url: Some(per_package_version_data_url.clone()),
            });
            module_scopes.insert(module.module_name.clone(), mscope);
        }

        Self {
            doc_package_info,
            url,
            per_package_data_url,
            per_package_version_data_url,
            module_scopes,
        }
    }

    pub fn name(&self) -> &str {
        &self.doc_package_info.name
    }

    pub fn version(&self) -> &str {
        &self.doc_package_info.version
    }

    pub fn get_module(&self, name: &str) -> Arc<ModuleScope> {
        Arc::clone(
            self.module_scopes
                .get(name)
                .unwrap_or_else(|| panic!("ModuleScope not found for `{name}`")),
        )
    }

    pub fn url_for_version_relative_to(
        &self,
        site: &SiteScope,
        scope: &DocScope,
        version: &str,
    ) -> String {
        let my_version = resolve_uri(
            &site.url,
            &format!("./{}/{version}/index.html", encode_path(self.name())),
        );
        relativize_uri(&my_version, &scope.url())
    }

    pub fn resolve_module_name_to_doc_url(&self, site: &SiteScope, name: &str) -> Option<String> {
        if let Some(ms) = self.module_scopes.get(name) {
            return Some(ms.url.clone());
        }
        for dep in &self.doc_package_info.dependencies {
            if let Some(doc_url) = dep.get_module_doc_url(name) {
                return Some(resolve_uri(&site.url, &doc_url));
            }
        }
        site.resolve_module_name_to_doc_url(name)
    }

    pub fn resolve_module_name_to_source_url(
        &self,
        site: &SiteScope,
        name: &str,
        source_location: SourceLocation,
    ) -> Option<String> {
        for dep in &self.doc_package_info.dependencies {
            if let Some(src_url) = dep.get_module_source_code_with_location(name, source_location) {
                return Some(src_url);
            }
        }
        site.resolve_module_name_to_source_url(name, source_location)
    }

    fn pkl_base_scope(&self, site: &SiteScope) -> ModuleScope {
        let base_url = self
            .resolve_module_name_to_doc_url(site, "pkl.base")
            .expect("pkl.base doc url");
        ModuleScope::new(site.pkl_base_module.clone(), base_url, None)
    }
}

/// Module-level scope (`ModuleScope` in `DocScope.kt:336`).
#[derive(Clone)]
pub struct ModuleScope {
    pub module: ModuleSchema,
    pub url: String,
    pub parent_info: Option<Arc<DocPackageInfo>>,
    pub parent_url: Option<String>,
    pub parent_per_package_data_url: Option<String>,
    pub parent_per_package_version_data_url: Option<String>,
}

impl ModuleScope {
    pub fn new(module: ModuleSchema, url: String, parent: Option<&PackageScope>) -> Self {
        Self {
            module,
            url,
            parent_info: parent.map(|p| Arc::clone(&p.doc_package_info)),
            parent_url: parent.map(|p| p.url.clone()),
            parent_per_package_data_url: parent.map(|p| p.per_package_data_url.clone()),
            parent_per_package_version_data_url: parent
                .map(|p| p.per_package_version_data_url.clone()),
        }
    }

    pub fn path(&self) -> String {
        let prefix = &self
            .parent_info
            .as_ref()
            .expect("ModuleScope::path requires parent")
            .module_name_prefix;
        uri_encode(&get_module_path(&self.module.module_name, prefix))
    }

    pub fn per_package_data_url(&self) -> String {
        let parent_url = self
            .parent_per_package_data_url
            .as_deref()
            .expect("ModuleScope::per_package_data_url requires parent");
        resolve_uri(parent_url, &format!("./{}/index.json", self.path()))
    }

    pub fn per_package_version_data_url(&self) -> String {
        let parent_url = self
            .parent_per_package_version_data_url
            .as_deref()
            .expect("ModuleScope::per_package_version_data_url requires parent");
        resolve_uri(parent_url, &format!("./{}/index.json", self.path()))
    }

    pub fn get_method(
        self: &Arc<Self>,
        parent_pkg: Option<Arc<PackageScope>>,
        site: Arc<SiteScope>,
        name: &str,
    ) -> Option<DocScope> {
        let mc_id = self.module.module_class_id;
        let method = self
            .module
            .store
            .all_methods(mc_id)
            .into_iter()
            .find(|(k, _)| k == name)
            .map(|(_, m)| m.clone())?;
        Some(DocScope::Method {
            site,
            method,
            parent: Box::new(DocScope::Module {
                site_pkg: parent_pkg,
                module: Arc::clone(self),
            }),
        })
    }

    pub fn get_property(
        self: &Arc<Self>,
        parent_pkg: Option<Arc<PackageScope>>,
        site: Arc<SiteScope>,
        name: &str,
    ) -> Option<DocScope> {
        let mc_id = self.module.module_class_id;
        if let Some((_, prop)) = self
            .module
            .store
            .all_properties(mc_id)
            .into_iter()
            .find(|(k, _)| k == name)
        {
            return Some(DocScope::Property {
                site,
                property: prop.clone(),
                parent: Box::new(DocScope::Module {
                    site_pkg: parent_pkg,
                    module: Arc::clone(self),
                }),
            });
        }
        if let Some((_, cid)) = self
            .module
            .all_classes()
            .into_iter()
            .find(|(k, _)| k == name)
        {
            return Some(DocScope::Class {
                site,
                schema: self.module.clone(),
                class_id: cid,
                parent_url: self.url.clone(),
                parent_module: Some((parent_pkg, Arc::clone(self))),
            });
        }
        if let Some((_, aid)) = self
            .module
            .all_type_aliases()
            .into_iter()
            .find(|(k, _)| k == name)
        {
            return Some(DocScope::TypeAlias {
                site,
                schema: self.module.clone(),
                alias_id: aid,
                parent_doc_url: self.url.clone(),
                parent_module: Some((parent_pkg, Arc::clone(self))),
            });
        }
        None
    }
}

/// Unified lexical scope enum (`DocScope` in `DocScope.kt`).
#[derive(Clone)]
pub enum DocScope {
    Site(Arc<SiteScope>),
    Package {
        site: Arc<SiteScope>,
        package: Arc<PackageScope>,
    },
    Module {
        site_pkg: Option<Arc<PackageScope>>,
        module: Arc<ModuleScope>,
    },
    Class {
        site: Arc<SiteScope>,
        schema: ModuleSchema,
        class_id: ClassId,
        parent_url: String,
        parent_module: Option<(Option<Arc<PackageScope>>, Arc<ModuleScope>)>,
    },
    TypeAlias {
        site: Arc<SiteScope>,
        schema: ModuleSchema,
        alias_id: TypeAliasId,
        parent_doc_url: String,
        parent_module: Option<(Option<Arc<PackageScope>>, Arc<ModuleScope>)>,
    },
    Method {
        site: Arc<SiteScope>,
        method: PClassMethod,
        parent: Box<DocScope>,
    },
    Property {
        site: Arc<SiteScope>,
        property: PClassProperty,
        parent: Box<DocScope>,
    },
    Parameter {
        site: Arc<SiteScope>,
        name: String,
        parent: Box<DocScope>,
    },
}

impl DocScope {
    pub fn is_page_scope(&self) -> bool {
        matches!(
            self,
            Self::Site(_) | Self::Package { .. } | Self::Module { .. } | Self::Class { .. }
        )
    }

    pub fn is_parameter_scope(&self) -> bool {
        matches!(self, Self::Parameter { .. })
    }

    pub fn key(&self) -> ScopeKey {
        match self {
            Self::Site(_) => ScopeKey::Site,
            Self::Package { package, .. } => ScopeKey::Package(package.name().to_string()),
            Self::Module { module, .. } => ScopeKey::Module(module.module.module_uri.clone()),
            Self::Class {
                schema, class_id, ..
            } => ScopeKey::Class(schema.store.class(*class_id).info.clone()),
            Self::TypeAlias {
                schema, alias_id, ..
            } => ScopeKey::TypeAlias(schema.store.type_alias(*alias_id).info.clone()),
            Self::Method { method, .. } => ScopeKey::Method {
                owner: method.owner_info.clone(),
                name: method.simple_name.clone(),
            },
            Self::Property { property, .. } => ScopeKey::Property {
                owner: property.owner_info.clone(),
                name: property.simple_name.clone(),
            },
            Self::Parameter { name, parent, .. } => ScopeKey::Parameter {
                name: name.clone(),
                parent: Box::new(parent.key()),
            },
        }
    }

    pub fn parent_key(&self) -> Option<ScopeKey> {
        match self {
            Self::Site(_) => None,
            Self::Package { .. } => Some(ScopeKey::Site),
            Self::Module { site_pkg, .. } => site_pkg
                .as_ref()
                .map(|p| ScopeKey::Package(p.name().to_string())),
            Self::Class { parent_module, .. } | Self::TypeAlias { parent_module, .. } => {
                parent_module
                    .as_ref()
                    .map(|(_, m)| ScopeKey::Module(m.module.module_uri.clone()))
            }
            Self::Method { parent, .. }
            | Self::Property { parent, .. }
            | Self::Parameter { parent, .. } => Some(parent.key()),
        }
    }

    pub fn url(&self) -> String {
        match self {
            Self::Site(site) => site.url.clone(),
            Self::Package { package, .. } => package.url.clone(),
            Self::Module { module, .. } => module.url.clone(),
            Self::Class {
                schema,
                class_id,
                parent_url,
                ..
            } => {
                let clazz = schema.store.class(*class_id);
                if clazz.is_module_class() {
                    parent_url.clone()
                } else {
                    resolve_uri(
                        parent_url,
                        &format!(
                            "{}.html",
                            uri_encode_component(&encode_path(clazz.simple_name()))
                        ),
                    )
                }
            }
            Self::TypeAlias {
                schema,
                alias_id,
                parent_doc_url,
                ..
            } => {
                let alias = schema.store.type_alias(*alias_id);
                resolve_uri(parent_doc_url, &format!("#{}", alias.simple_name()))
            }
            Self::Method { method, parent, .. } => {
                resolve_uri(&parent.url(), &format!("#{}()", method.simple_name))
            }
            Self::Property {
                property, parent, ..
            } => resolve_uri(&parent.url(), &format!("#{}", property.simple_name)),
            Self::Parameter { name, parent, .. } => {
                if matches!(parent.as_ref(), Self::Class { .. }) {
                    resolve_uri(&parent.url(), &format!("#{name}"))
                } else {
                    format!("{}.{name}", parent.url())
                }
            }
        }
    }

    pub fn url_relative_to(&self, other: &DocScope) -> String {
        relativize_uri(&self.url(), &other.url())
    }

    pub fn relative_site_url(&self, site: &SiteScope) -> String {
        relativize_uri(&site.url, &self.url())
    }

    pub fn relative_package_url(&self) -> Option<String> {
        let pkg_url = self.enclosing_package_url()?;
        Some(relativize_uri(&pkg_url, &self.url()))
    }

    fn enclosing_package_url(&self) -> Option<String> {
        match self {
            Self::Site(_) => None,
            Self::Package { package, .. } => Some(package.url.clone()),
            Self::Module { module, .. } => module.parent_url.clone(),
            Self::Class { parent_module, .. } | Self::TypeAlias { parent_module, .. } => {
                parent_module
                    .as_ref()
                    .and_then(|(_, m)| m.parent_url.clone())
            }
            Self::Method { parent, .. }
            | Self::Property { parent, .. }
            | Self::Parameter { parent, .. } => parent.enclosing_package_url(),
        }
    }

    pub fn per_package_data_url(&self) -> Option<String> {
        match self {
            Self::Package { package, .. } => Some(package.per_package_data_url.clone()),
            Self::Module { module, .. } => Some(module.per_package_data_url()),
            Self::Class {
                schema,
                class_id,
                parent_module,
                ..
            } => {
                let (_, m) = parent_module.as_ref()?;
                let clazz = schema.store.class(*class_id);
                Some(resolve_uri(
                    &m.per_package_data_url(),
                    &format!(
                        "{}.json",
                        uri_encode_component(&encode_path(clazz.simple_name()))
                    ),
                ))
            }
            _ => None,
        }
    }

    pub fn per_package_version_data_url(&self) -> Option<String> {
        match self {
            Self::Package { package, .. } => Some(package.per_package_version_data_url.clone()),
            Self::Module { module, .. } => Some(module.per_package_version_data_url()),
            Self::Class {
                schema,
                class_id,
                parent_module,
                ..
            } => {
                let (_, m) = parent_module.as_ref()?;
                let clazz = schema.store.class(*class_id);
                Some(resolve_uri(
                    &m.per_package_version_data_url(),
                    &format!(
                        "{}.json",
                        uri_encode_component(&encode_path(clazz.simple_name()))
                    ),
                ))
            }
            _ => None,
        }
    }

    pub fn resolve_module_name_to_doc_url(&self, site: &SiteScope, name: &str) -> Option<String> {
        match self {
            Self::Site(_) => site.resolve_module_name_to_doc_url(name),
            Self::Package { package, .. } => package.resolve_module_name_to_doc_url(site, name),
            Self::Module { site_pkg, module } => {
                if name == module.module.module_name {
                    Some(module.url.clone())
                } else {
                    site_pkg
                        .as_ref()
                        .and_then(|p| p.resolve_module_name_to_doc_url(site, name))
                        .or_else(|| site.resolve_module_name_to_doc_url(name))
                }
            }
            Self::Class { parent_module, .. } => {
                if let Some((pkg, m)) = parent_module {
                    DocScope::Module {
                        site_pkg: pkg.clone(),
                        module: Arc::clone(m),
                    }
                    .resolve_module_name_to_doc_url(site, name)
                } else {
                    site.resolve_module_name_to_doc_url(name)
                }
            }
            _ => None,
        }
    }

    pub fn resolve_module_name_to_relative_doc_url(
        &self,
        site: &SiteScope,
        name: &str,
    ) -> Option<String> {
        self.resolve_module_name_to_doc_url(site, name)
            .map(|target| relativize_uri(&target, &self.url()))
    }

    pub fn resolve_module_name_to_source_url(
        &self,
        site: &SiteScope,
        name: &str,
        source_location: SourceLocation,
    ) -> Option<String> {
        match self {
            Self::Site(_) => site.resolve_module_name_to_source_url(name, source_location),
            Self::Package { package, .. } => {
                package.resolve_module_name_to_source_url(site, name, source_location)
            }
            Self::Module { site_pkg, module } => {
                if name == module.module.module_name {
                    module
                        .parent_info
                        .as_ref()
                        .and_then(|info| info.source_code_url_scheme.as_deref())
                        .map(|scheme| {
                            replace_source_code_placeholders(
                                scheme,
                                &format!("/{}.pkl", module.path()),
                                source_location.start_line,
                                source_location.end_line,
                            )
                        })
                } else {
                    site_pkg
                        .as_ref()
                        .and_then(|p| {
                            p.resolve_module_name_to_source_url(site, name, source_location)
                        })
                        .or_else(|| site.resolve_module_name_to_source_url(name, source_location))
                }
            }
            Self::Class { parent_module, .. } => {
                if let Some((pkg, m)) = parent_module {
                    DocScope::Module {
                        site_pkg: pkg.clone(),
                        module: Arc::clone(m),
                    }
                    .resolve_module_name_to_source_url(
                        site,
                        name,
                        source_location,
                    )
                } else {
                    site.resolve_module_name_to_source_url(name, source_location)
                }
            }
            _ => None,
        }
    }

    /// Resolves a doc link like `someImport.SomeClass` or `SomeClass.someMethod()` (`DocScope.kt:89`).
    pub fn resolve_doc_link(&self, site: &Arc<SiteScope>, text: &str) -> Option<DocScope> {
        let mut parts = text.split('.');
        let first = parts.next()?;
        let mut curr = if let Some(method_name) = first.strip_suffix("()") {
            self.resolve_method(site, method_name)?
        } else {
            self.resolve_variable(site, first)?
        };
        for part in parts {
            curr = if let Some(method_name) = part.strip_suffix("()") {
                curr.get_method(site, method_name)?
            } else {
                curr.get_property(site, part)?
            };
        }
        Some(curr)
    }

    pub fn get_method(&self, site: &Arc<SiteScope>, name: &str) -> Option<DocScope> {
        match self {
            Self::Site(_) | Self::Package { .. } => None,
            Self::Module { site_pkg, module } => {
                module.get_method(site_pkg.clone(), Arc::clone(site), name)
            }
            Self::Class {
                schema, class_id, ..
            } => {
                let method = schema
                    .store
                    .all_methods(*class_id)
                    .into_iter()
                    .find(|(k, _)| k == name)
                    .map(|(_, m)| m.clone())?;
                Some(Self::Method {
                    site: Arc::clone(site),
                    method,
                    parent: Box::new(self.clone()),
                })
            }
            Self::TypeAlias { parent_module, .. } => {
                let (pkg, m) = parent_module.as_ref()?;
                m.get_method(pkg.clone(), Arc::clone(site), name)
            }
            Self::Method { .. } | Self::Property { .. } | Self::Parameter { .. } => None,
        }
    }

    pub fn get_property(&self, site: &Arc<SiteScope>, name: &str) -> Option<DocScope> {
        match self {
            Self::Site(_) | Self::Package { .. } => None,
            Self::Module { site_pkg, module } => {
                module.get_property(site_pkg.clone(), Arc::clone(site), name)
            }
            Self::Class {
                schema, class_id, ..
            } => {
                let prop = schema
                    .store
                    .all_properties(*class_id)
                    .into_iter()
                    .find(|(k, _)| k == name)
                    .map(|(_, p)| p.clone())?;
                Some(Self::Property {
                    site: Arc::clone(site),
                    property: prop,
                    parent: Box::new(self.clone()),
                })
            }
            Self::TypeAlias { parent_module, .. } => {
                let (pkg, m) = parent_module.as_ref()?;
                m.get_property(pkg.clone(), Arc::clone(site), name)
            }
            Self::Method { .. } | Self::Property { .. } | Self::Parameter { .. } => None,
        }
    }

    pub fn resolve_method(&self, site: &Arc<SiteScope>, name: &str) -> Option<DocScope> {
        match self {
            Self::Site(s) => {
                let base = Arc::new(s.pkl_base_scope());
                base.get_method(None, Arc::clone(site), name)
            }
            Self::Package { package, .. } => {
                let base = Arc::new(package.pkl_base_scope(site));
                base.get_method(None, Arc::clone(site), name)
            }
            Self::Module { site_pkg, module } => {
                if let Some(m) = module.module.module_class().method(name) {
                    return Some(Self::Method {
                        site: Arc::clone(site),
                        method: m.clone(),
                        parent: Box::new(self.clone()),
                    });
                }
                if let Some(pkg) = site_pkg {
                    let base = Arc::new(pkg.pkl_base_scope(site));
                    if let Some(found) = base.get_method(None, Arc::clone(site), name) {
                        return Some(found);
                    }
                }
                self.get_method(site, name)
            }
            Self::Class {
                schema,
                class_id,
                parent_module,
                ..
            } => {
                let clazz = schema.store.class(*class_id);
                if let Some(m) = clazz.method(name) {
                    return Some(Self::Method {
                        site: Arc::clone(site),
                        method: m.clone(),
                        parent: Box::new(self.clone()),
                    });
                }
                if let Some((pkg, m)) = parent_module {
                    let parent = Self::Module {
                        site_pkg: pkg.clone(),
                        module: Arc::clone(m),
                    };
                    if let Some(found) = parent.resolve_method(site, name) {
                        return Some(found);
                    }
                }
                self.get_method(site, name)
            }
            Self::TypeAlias { parent_module, .. } => {
                let (pkg, m) = parent_module.as_ref()?;
                Self::Module {
                    site_pkg: pkg.clone(),
                    module: Arc::clone(m),
                }
                .resolve_method(site, name)
            }
            Self::Method { parent, .. }
            | Self::Property { parent, .. }
            | Self::Parameter { parent, .. } => parent.resolve_method(site, name),
        }
    }

    pub fn resolve_variable(&self, site: &Arc<SiteScope>, name: &str) -> Option<DocScope> {
        match self {
            Self::Site(s) => {
                if let Some((_, uri)) = s.overview_imports.iter().find(|(k, _)| k == name)
                    && let Some(mod_schema) = s.resolve_import(uri)
                    && let Some(doc_url) = s.resolve_module_name_to_doc_url(&mod_schema.module_name)
                {
                    return Some(Self::Module {
                        site_pkg: None,
                        module: Arc::new(ModuleScope::new(mod_schema.clone(), doc_url, None)),
                    });
                }
                let base = Arc::new(s.pkl_base_scope());
                base.get_property(None, Arc::clone(site), name)
            }
            Self::Package { package, .. } => {
                if let Some(uri) = package.doc_package_info.get_overview_import(name)
                    && let Some(mod_schema) = site.resolve_import(uri)
                    && let Some(doc_url) =
                        package.resolve_module_name_to_doc_url(site, &mod_schema.module_name)
                {
                    return Some(Self::Module {
                        site_pkg: None,
                        module: Arc::new(ModuleScope::new(mod_schema.clone(), doc_url, None)),
                    });
                }
                let base = Arc::new(package.pkl_base_scope(site));
                base.get_property(None, Arc::clone(site), name)
            }
            Self::Module { site_pkg, module } => {
                if name == "module" {
                    return Some(self.clone());
                }
                if let Some((_, uri)) = module.module.imports.iter().find(|(k, _)| k == name)
                    && let Some(mod_schema) = site.resolve_import(uri)
                {
                    let doc_url =
                        self.resolve_module_name_to_doc_url(site, &mod_schema.module_name);
                    if let Some(url) = doc_url {
                        return Some(Self::Module {
                            site_pkg: None,
                            module: Arc::new(ModuleScope::new(mod_schema.clone(), url, None)),
                        });
                    }
                    // Upstream `ModuleScope.kt:390-393` returns `null` from the `let` block,
                    // falling through to properties / classes / pkl.base!
                }
                if let Some(prop) = module.module.module_class().property(name) {
                    return Some(Self::Property {
                        site: Arc::clone(site),
                        property: prop.clone(),
                        parent: Box::new(self.clone()),
                    });
                }
                if let Some((_, cid)) = module
                    .module
                    .all_classes()
                    .into_iter()
                    .find(|(k, _)| k == name)
                {
                    return Some(Self::Class {
                        site: Arc::clone(site),
                        schema: module.module.clone(),
                        class_id: cid,
                        parent_url: module.url.clone(),
                        parent_module: Some((site_pkg.clone(), Arc::clone(module))),
                    });
                }
                if let Some((_, aid)) = module
                    .module
                    .all_type_aliases()
                    .into_iter()
                    .find(|(k, _)| k == name)
                {
                    return Some(Self::TypeAlias {
                        site: Arc::clone(site),
                        schema: module.module.clone(),
                        alias_id: aid,
                        parent_doc_url: module.url.clone(),
                        parent_module: Some((site_pkg.clone(), Arc::clone(module))),
                    });
                }
                if let Some(pkg) = site_pkg {
                    let base = Arc::new(pkg.pkl_base_scope(site));
                    if let Some(found) = base.get_property(None, Arc::clone(site), name) {
                        return Some(found);
                    }
                }
                self.get_property(site, name)
            }
            Self::Class {
                schema,
                class_id,
                parent_module,
                ..
            } => {
                let clazz = schema.store.class(*class_id);
                if clazz.type_parameters.iter().any(|tp| tp.name == name) {
                    return Some(Self::Parameter {
                        site: Arc::clone(site),
                        name: name.to_string(),
                        parent: Box::new(self.clone()),
                    });
                }
                if let Some(prop) = clazz.property(name) {
                    return Some(Self::Property {
                        site: Arc::clone(site),
                        property: prop.clone(),
                        parent: Box::new(self.clone()),
                    });
                }
                if let Some((pkg, m)) = parent_module {
                    let parent = Self::Module {
                        site_pkg: pkg.clone(),
                        module: Arc::clone(m),
                    };
                    if let Some(found) = parent.resolve_variable(site, name) {
                        return Some(found);
                    }
                }
                if let Some((_, prop)) = schema
                    .store
                    .all_properties(*class_id)
                    .into_iter()
                    .find(|(k, _)| k == name)
                {
                    return Some(Self::Property {
                        site: Arc::clone(site),
                        property: prop.clone(),
                        parent: Box::new(self.clone()),
                    });
                }
                None
            }
            Self::TypeAlias { parent_module, .. } => {
                let (pkg, m) = parent_module.as_ref()?;
                Self::Module {
                    site_pkg: pkg.clone(),
                    module: Arc::clone(m),
                }
                .resolve_variable(site, name)
            }
            Self::Method { method, parent, .. } => {
                if method.type_parameters.iter().any(|tp| tp.name == name)
                    || method.parameters.iter().any(|(p, _)| p == name)
                {
                    return Some(Self::Parameter {
                        site: Arc::clone(site),
                        name: name.to_string(),
                        parent: Box::new(self.clone()),
                    });
                }
                parent.resolve_variable(site, name)
            }
            Self::Property { parent, .. } | Self::Parameter { parent, .. } => {
                parent.resolve_variable(site, name)
            }
        }
    }

    /// Creates a `MarkdownParserContext`-equivalent link resolver closure for one Markdown parse
    /// (`Markdown.kt:41-98`). Note that `seen_link_targets` resets on every `parse()` call
    /// (matching `InlineParserFactory.create` being invoked per `markdownParser.parse(...)`).
    pub fn make_markdown_ref_resolver<'a>(
        &'a self,
        site: &'a Arc<SiteScope>,
        page_scope: &'a DocScope,
    ) -> impl FnMut(&str) -> RefLinkResolution + 'a {
        const KEYWORDS: &[&str] = &["null", "true", "false", "this", "unknown", "nothing"];
        let doc_key = self.key();
        let mut seen_link_targets: HashSet<ScopeKey> = HashSet::new();
        move |label: &str| {
            if KEYWORDS.contains(&label) {
                return RefLinkResolution {
                    destination: label.to_string(),
                    label: label.to_string(),
                    code_only_if_short: true,
                };
            }
            let dest_scope = self.resolve_doc_link(site, label);
            let destination = dest_scope.as_ref().map_or_else(
                || "not_found".to_string(),
                |d| d.url_relative_to(page_scope),
            );
            let code_only_if_short = match &dest_scope {
                None => false, // command 2: dangling link
                Some(dest) => {
                    let dest_key = dest.key();
                    if dest_key == doc_key
                        || (dest.is_parameter_scope()
                            && dest.parent_key().as_ref() == Some(&doc_key))
                    {
                        true // command 1: don't link to ourselves or our own method/type parameters
                    } else {
                        !seen_link_targets.insert(dest_key) // command 1 if already seen, else 2
                    }
                }
            };
            RefLinkResolution {
                destination,
                label: label.to_string(),
                code_only_if_short,
            }
        }
    }
}

/// Convenience helper for constructing a `ClassScope` for a class declared in a `ModuleScope`.
pub fn make_class_scope(
    site: &Arc<SiteScope>,
    pkg: &Arc<PackageScope>,
    module: &Arc<ModuleScope>,
    class: &PClass,
) -> DocScope {
    DocScope::Class {
        site: Arc::clone(site),
        schema: module.module.clone(),
        class_id: class.id,
        parent_url: module.url.clone(),
        parent_module: Some((Some(Arc::clone(pkg)), Arc::clone(module))),
    }
}

/// Convenience helper for constructing a `TypeAliasScope` declared in a `ModuleScope`.
pub fn make_type_alias_scope(
    site: &Arc<SiteScope>,
    pkg: &Arc<PackageScope>,
    module: &Arc<ModuleScope>,
    alias: &TypeAlias,
) -> DocScope {
    DocScope::TypeAlias {
        site: Arc::clone(site),
        schema: module.module.clone(),
        alias_id: alias.id,
        parent_doc_url: module.url.clone(),
        parent_module: Some((Some(Arc::clone(pkg)), Arc::clone(module))),
    }
}
