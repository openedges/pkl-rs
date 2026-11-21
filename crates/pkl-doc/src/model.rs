//! Core domain model for `pkl-doc`: `DocsiteInfo`, `DocPackageInfo`, `PackageDependency`,
//! `DocPackage`, `DocModule`, error types, and annotation extraction helpers.

use crate::package_data::{ModuleRef, TypeRef};
use crate::util::{
    ParsedUri, encode_path, get_module_path, parse_semver_or_none,
    replace_source_code_placeholders, resolve_uri, uri_encode,
};
use pkl_runtime::packages::metadata::{MetadataAnnotation, MetadataValue};
use pkl_runtime::packages::{PackageUri, PackageVersion};
use pkl_runtime::pkl_binary_decode::{
    DecodedClassInfo, DecodedObject, DecodedObjectMember, DecodedValue, ObjectMemberKind,
};
use pkl_runtime::schema::{ModuleSchema, PClass, SourceLocation};
use pkl_runtime::value::PklString;
use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

/// Error returned by `DocGenerator` / `DocMigrator` / `CliDocGenerator`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DocGeneratorError {
    /// Expected user/CLI error (`org.pkl.doc.DocGeneratorException` / `CliException`).
    User(String),
    /// Internal bug (`org.pkl.doc.DocGeneratorBugException` / `CliBugException`).
    Bug(String),
}

impl DocGeneratorError {
    pub fn user(message: impl Into<String>) -> Self {
        Self::User(message.into())
    }

    pub fn bug(message: impl Into<String>) -> Self {
        Self::Bug(message.into())
    }

    pub fn message(&self) -> &str {
        match self {
            Self::User(msg) | Self::Bug(msg) => msg,
        }
    }
}

impl fmt::Display for DocGeneratorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.message())
    }
}

impl std::error::Error for DocGeneratorError {}

/// API equivalent of standard library module `pkl.DocsiteInfo`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DocsiteInfo {
    pub title: Option<String>,
    pub overview: Option<String>,
    /// Preserves insertion order matching Pkl `Mapping`/`Map`.
    pub overview_imports: Vec<(String, String)>,
}

impl DocsiteInfo {
    pub fn get_overview_import(&self, alias: &str) -> Option<&str> {
        self.overview_imports
            .iter()
            .find(|(k, _)| k == alias)
            .map(|(_, v)| v.as_str())
    }

    pub fn from_decoded(module: &DecodedObject) -> Result<Self, DocGeneratorError> {
        let title = get_opt_string(module, "title")?;
        let overview = get_opt_string(module, "overview")?;
        let overview_imports = get_string_map(module, "overviewImports")?;
        Ok(Self {
            title,
            overview,
            overview_imports,
        })
    }
}

/// Information about a depended-on package (`DocPackageInfo.PackageDependency`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PackageDependency {
    pub name: String,
    pub uri: Option<String>,
    pub version: String,
    pub source_code: Option<String>,
    pub source_code_url_scheme: Option<String>,
    pub documentation: Option<String>,
}

impl PackageDependency {
    pub fn prefix(&self) -> String {
        format!("{}.", self.name)
    }

    /// Matches `DocPackageInfo.PackageDependency.qualifiedName`.
    pub fn qualified_name(&self) -> String {
        if let Some(uri_str) = &self.uri
            && let Ok(pkg_uri) = PackageUri::parse(uri_str.as_str())
        {
            let parsed = ParsedUri::parse(uri_str);
            if let Some(authority) = parsed.authority {
                return format!("{authority}{}", pkg_uri.path_without_version);
            }
        }
        self.name.clone()
    }

    /// Returns an absolute URI, or a URI relative to the current site (`DocPackageInfo.kt:209`).
    pub fn get_module_doc_url(&self, module_name: &str) -> Option<String> {
        let prefix = self.prefix();
        if !module_name.starts_with(&prefix) {
            return None;
        }
        let module_path = encode_path(&module_name[prefix.len()..].replace('.', "/"));
        match &self.documentation {
            None => Some(format!(
                "{}/{}/{}/index.html",
                encode_path(&self.name),
                self.version,
                module_path
            )),
            Some(doc_url) => Some(resolve_uri(doc_url, &format!("{module_path}/index.html"))),
        }
    }

    #[allow(dead_code)]
    pub fn get_module_source_code(&self, module_name: &str) -> Option<String> {
        let prefix = self.prefix();
        if !module_name.starts_with(&prefix) {
            return None;
        }
        let module_path = module_name[prefix.len()..].replace('.', "/");
        self.source_code
            .as_deref()
            .map(|base| resolve_uri(base, &format!("{module_path}.pkl")))
    }

    pub fn get_module_source_code_with_location(
        &self,
        module_name: &str,
        source_location: SourceLocation,
    ) -> Option<String> {
        let prefix = self.prefix();
        if !module_name.starts_with(&prefix) {
            return None;
        }
        let module_path = module_name[prefix.len()..].replace('.', "/");
        let path = format!("/{module_path}.pkl");
        self.source_code_url_scheme.as_deref().map(|scheme| {
            replace_source_code_placeholders(
                scheme,
                &path,
                source_location.start_line,
                source_location.end_line,
            )
        })
    }
}

/// API equivalent of standard library module `pkl.DocPackageInfo`.
#[derive(Clone, Debug, PartialEq)]
pub struct DocPackageInfo {
    pub name: String,
    pub module_name_prefix: String,
    pub uri: Option<String>,
    pub version: String,
    pub import_uri: String,
    pub authors: Option<Vec<String>>,
    pub source_code: Option<String>,
    pub source_code_url_scheme: Option<String>,
    pub issue_tracker: Option<String>,
    pub dependencies: Vec<PackageDependency>,
    pub overview: Option<String>,
    pub overview_imports: Vec<(String, String)>,
    pub annotations: Vec<DecodedObject>,
    /// Preserves insertion order for rendering `<dt>/<dd>` rows.
    pub extra_attributes: Vec<(String, String)>,
}

impl DocPackageInfo {
    pub fn get_overview_import(&self, alias: &str) -> Option<&str> {
        self.overview_imports
            .iter()
            .find(|(k, _)| k == alias)
            .map(|(_, v)| v.as_str())
    }

    pub fn from_decoded(module: &DecodedObject) -> Result<Self, DocGeneratorError> {
        let name = get_req_string(module, "name")?;
        let version = get_req_string(module, "version")?;
        let import_uri = get_req_string(module, "importUri")?;
        let uri = get_opt_string(module, "uri")?;
        let authors = Some(get_string_list(module, "authors")?);
        let source_code = get_opt_string(module, "sourceCode")?;
        let source_code_url_scheme = get_opt_string(module, "sourceCodeUrlScheme")?;
        let issue_tracker = get_opt_string(module, "issueTracker")?;
        let mut dependencies = Vec::new();
        for dep_obj in get_object_list(module, "dependencies")? {
            dependencies.push(PackageDependency {
                name: get_req_string(dep_obj, "name")?,
                uri: None, // dependencies declared in a doc-package-info file do not have URIs
                version: get_req_string(dep_obj, "version")?,
                source_code: get_opt_string(dep_obj, "sourceCode")?,
                source_code_url_scheme: get_opt_string(dep_obj, "sourceCodeUrlScheme")?,
                documentation: get_opt_string(dep_obj, "documentation")?,
            });
        }
        let overview = Some(get_req_string(module, "overview")?);
        let overview_imports = get_string_map(module, "overviewImports")?;
        let annotations = get_object_list(module, "annotations")?
            .into_iter()
            .cloned()
            .collect();
        let extra_attributes = get_string_map(module, "extraAttributes")?;
        let module_name_prefix = format!("{name}.");
        Ok(Self {
            name,
            module_name_prefix,
            uri,
            version,
            import_uri,
            authors,
            source_code,
            source_code_url_scheme,
            issue_tracker,
            dependencies,
            overview,
            overview_imports,
            annotations,
            extra_attributes,
        })
    }

    pub fn get_module_ref(&self, module_name: &str) -> Option<ModuleRef> {
        if module_name.starts_with(&self.module_name_prefix) {
            return Some(ModuleRef {
                pkg: self.name.clone(),
                pkg_uri: self.uri.clone(),
                version: self.version.clone(),
                module: get_module_path(module_name, &self.module_name_prefix),
            });
        }
        for dependency in &self.dependencies {
            let prefix = dependency.prefix();
            if module_name.starts_with(&prefix) {
                return Some(ModuleRef {
                    pkg: dependency.name.clone(),
                    pkg_uri: dependency.uri.clone(),
                    version: dependency.version.clone(),
                    module: get_module_path(module_name, &prefix),
                });
            }
        }
        None
    }

    pub fn get_type_ref(
        &self,
        module_name: &str,
        simple_name: &str,
        is_type_alias: bool,
    ) -> Option<TypeRef> {
        if module_name.starts_with(&self.module_name_prefix) {
            return Some(TypeRef {
                pkg: self.name.clone(),
                pkg_uri: self.uri.clone(),
                version: self.version.clone(),
                module: get_module_path(module_name, &self.module_name_prefix),
                ty: simple_name.to_string(),
                is_type_alias,
            });
        }
        for dependency in &self.dependencies {
            let prefix = dependency.prefix();
            if module_name.starts_with(&prefix) {
                return Some(TypeRef {
                    pkg: dependency.name.clone(),
                    pkg_uri: dependency.uri.clone(),
                    version: dependency.version.clone(),
                    module: get_module_path(module_name, &prefix),
                    ty: simple_name.to_string(),
                    is_type_alias,
                });
            }
        }
        None
    }

    pub fn get_module_import_uri(&self, module_name: &str) -> String {
        if self.import_uri == "pkl:/" {
            format!("pkl:{}", &module_name[4..])
        } else {
            let path = format!(
                "{}.pkl",
                uri_encode(&get_module_path(module_name, &self.module_name_prefix))
            );
            format!("{}{}", self.import_uri, path)
        }
    }

    pub fn get_module_source_code(&self, module_name: &str) -> Option<String> {
        let path = format!(
            "/{}.pkl",
            uri_encode(&get_module_path(module_name, &self.module_name_prefix))
        );
        self.source_code_url_scheme.as_deref().map(|scheme| {
            let replaced = scheme.replace("%{path}", &path);
            match replaced.find('#') {
                Some(idx) => replaced[..idx].to_string(),
                None => replaced,
            }
        })
    }
}

/// Package prepared for HTML/data/search-index generation (`DocGenerator.kt:279`).
#[derive(Clone, Debug)]
pub struct DocPackage {
    pub doc_package_info: Arc<DocPackageInfo>,
    pub modules: Vec<ModuleSchema>,
    pub doc_modules: Vec<DocModule>,
    pub min_pkl_version: Option<PackageVersion>,
    pub deprecation: Option<String>,
    pub is_unlisted: bool,
    pub has_listed_module: bool,
}

impl DocPackage {
    pub fn new(doc_package_info: DocPackageInfo, modules: Vec<ModuleSchema>) -> Self {
        let info = Arc::new(doc_package_info);
        let mut example_modules_by_subject: BTreeMap<String, Vec<ModuleSchema>> = BTreeMap::new();
        for module in &modules {
            if let Some(ann) = module.annotations.iter().find(|a| is_doc_example_ann(a)) {
                for subject in doc_example_subjects(ann) {
                    example_modules_by_subject
                        .entry(subject)
                        .or_default()
                        .push(module.clone());
                }
            }
        }

        let mut doc_modules = Vec::new();
        for module in &modules {
            if module.annotations.iter().any(is_doc_example_ann) {
                continue;
            }
            let import_uri = info.get_module_import_uri(&module.module_name);
            let source_url = info.get_module_source_code(&module.module_name);
            let examples = example_modules_by_subject
                .get(&module.module_name)
                .cloned()
                .unwrap_or_default();
            doc_modules.push(DocModule::new(
                Arc::clone(&info),
                module.clone(),
                info.version.clone(),
                import_uri,
                source_url,
                examples,
            ));
        }

        let min_pkl_version = doc_modules
            .iter()
            .filter_map(|m| m.min_pkl_version.clone())
            .max();
        let deprecation = annotations_deprecation(&info.annotations);
        let is_unlisted = annotations_is_unlisted(&info.annotations);
        let has_listed_module = doc_modules.iter().any(|m| !m.is_unlisted);

        Self {
            doc_package_info: info,
            modules,
            doc_modules,
            min_pkl_version,
            deprecation,
            is_unlisted,
            has_listed_module,
        }
    }

    pub fn name(&self) -> &str {
        &self.doc_package_info.name
    }

    pub fn version(&self) -> &str {
        &self.doc_package_info.version
    }

    pub fn uri(&self) -> Option<&str> {
        self.doc_package_info.uri.as_deref()
    }

    pub fn overview(&self) -> Option<&str> {
        self.doc_package_info.overview.as_deref()
    }
}

/// Module prepared for HTML/data/search-index generation (`DocGenerator.kt:335`).
#[derive(Clone, Debug)]
pub struct DocModule {
    pub parent_info: Arc<DocPackageInfo>,
    pub schema: ModuleSchema,
    pub version: String,
    pub import_uri: String,
    pub source_url: Option<String>,
    pub examples: Vec<ModuleSchema>,
    pub path: String,
    pub min_pkl_version: Option<PackageVersion>,
    pub deprecation: Option<String>,
    pub is_unlisted: bool,
}

impl DocModule {
    pub fn new(
        parent_info: Arc<DocPackageInfo>,
        schema: ModuleSchema,
        version: String,
        import_uri: String,
        source_url: Option<String>,
        examples: Vec<ModuleSchema>,
    ) -> Self {
        // Exact upstream quirk (`DocGenerator.kt:347`):
        // `name.pathEncoded.substring(parent.docPackageInfo.moduleNamePrefix.length).replace('.', '/')`
        let encoded_name = encode_path(&schema.module_name);
        let prefix_len = parent_info.module_name_prefix.len();
        let path = if encoded_name.len() >= prefix_len {
            encoded_name[prefix_len..].replace('.', "/")
        } else {
            encoded_name.replace('.', "/")
        };
        let min_pkl_version = module_info_min_pkl_version(&schema.annotations);
        let deprecation = annotations_deprecation(&schema.annotations);
        let is_unlisted = annotations_is_unlisted(&schema.annotations);
        Self {
            parent_info,
            schema,
            version,
            import_uri,
            source_url,
            examples,
            path,
            min_pkl_version,
            deprecation,
            is_unlisted,
        }
    }

    pub fn name(&self) -> &str {
        &self.schema.module_name
    }

    pub fn overview(&self) -> Option<&str> {
        self.schema.doc_comment.as_deref()
    }
}

// ---------------------------------------------------------------------------
// Annotation & schema helpers matching `Util.kt`
// ---------------------------------------------------------------------------

pub fn is_base_annotation(ann: &DecodedObject, simple_name: &str) -> bool {
    ann.module_uri.as_str() == Some("pkl:base") && ann.class_info.simple_name == simple_name
}

pub fn annotations_is_unlisted(annotations: &[DecodedObject]) -> bool {
    annotations
        .iter()
        .any(|ann| is_base_annotation(ann, "Unlisted"))
}

pub fn annotations_deprecation(annotations: &[DecodedObject]) -> Option<String> {
    let ann = annotations
        .iter()
        .find(|ann| is_base_annotation(ann, "Deprecated"))?;
    match ann.get_property("message") {
        Some(DecodedValue::String(s)) => Some(s.to_string()),
        _ => None,
    }
}

pub fn annotations_also_known_as(annotations: &[DecodedObject]) -> Option<Vec<String>> {
    let ann = annotations
        .iter()
        .find(|ann| is_base_annotation(ann, "AlsoKnownAs"))?;
    match ann.get_property("names") {
        Some(DecodedValue::List(items) | DecodedValue::Listing(items)) => {
            let mut out = Vec::with_capacity(items.len());
            for item in items {
                if let DecodedValue::String(s) = item {
                    out.push(s.to_string());
                }
            }
            Some(out)
        }
        _ => None,
    }
}

pub fn is_doc_example_ann(ann: &DecodedObject) -> bool {
    is_base_annotation(ann, "DocExample")
}

pub fn doc_example_subjects(ann: &DecodedObject) -> Vec<String> {
    match ann.get_property("subjects") {
        Some(DecodedValue::List(items) | DecodedValue::Listing(items)) => items
            .iter()
            .filter_map(|item| match item {
                DecodedValue::String(s) => Some(s.to_string()),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

pub fn module_info_min_pkl_version(annotations: &[DecodedObject]) -> Option<PackageVersion> {
    let ann = annotations
        .iter()
        .find(|ann| is_base_annotation(ann, "ModuleInfo"))?;
    match ann.get_property("minPklVersion") {
        Some(DecodedValue::String(s)) => parse_semver_or_none(&s.to_string()),
        _ => None,
    }
}

pub fn create_deprecated_annotation(message: &str) -> DecodedObject {
    DecodedObject {
        class_name: PklString::from("Deprecated"),
        module_uri: PklString::from("pkl:base"),
        class_info: DecodedClassInfo::from_wire(
            &PklString::from("Deprecated"),
            &PklString::from("pkl:base"),
        ),
        members: vec![
            DecodedObjectMember {
                kind: ObjectMemberKind::Property,
                key: DecodedValue::String(PklString::from("message")),
                value: DecodedValue::String(PklString::from(message)),
            },
            DecodedObjectMember {
                kind: ObjectMemberKind::Property,
                key: DecodedValue::String(PklString::from("replaceWith")),
                value: DecodedValue::Null,
            },
        ],
    }
}

pub fn schema_has_listed_class(schema: &ModuleSchema) -> bool {
    schema
        .all_classes()
        .iter()
        .any(|(_, id)| !annotations_is_unlisted(&schema.store.class(*id).annotations))
}

pub fn schema_has_listed_type_alias(schema: &ModuleSchema) -> bool {
    schema
        .all_type_aliases()
        .iter()
        .any(|(_, id)| !annotations_is_unlisted(&schema.store.type_alias(*id).annotations))
}

pub fn class_has_listed_property(schema: &ModuleSchema, class: &PClass) -> bool {
    schema
        .store
        .all_properties(class.id)
        .iter()
        .any(|(_, p)| !annotations_is_unlisted(&p.annotations))
}

pub fn class_has_listed_method(schema: &ModuleSchema, class: &PClass) -> bool {
    schema
        .store
        .all_methods(class.id)
        .iter()
        .any(|(_, m)| !annotations_is_unlisted(&m.annotations))
}

/// Converts a `DependencyMetadata` JSON annotation (`MetadataAnnotation`) into a `DecodedObject`
/// matching how `org.pkl.core.packages.DependencyMetadata` produces `List<PObject>`.
pub fn metadata_annotation_to_decoded(ann: &MetadataAnnotation) -> DecodedObject {
    let class_info =
        DecodedClassInfo::from_wire(&ann.class_info.class_name, &ann.class_info.module_uri);
    let members = ann
        .properties
        .iter()
        .map(|(k, v)| DecodedObjectMember {
            kind: ObjectMemberKind::Property,
            key: DecodedValue::String(k.clone()),
            value: metadata_value_to_decoded(v),
        })
        .collect();
    DecodedObject {
        class_name: ann.class_info.class_name.clone(),
        module_uri: ann.class_info.module_uri.clone(),
        class_info,
        members,
    }
}

fn metadata_value_to_decoded(value: &MetadataValue) -> DecodedValue {
    match value {
        MetadataValue::Null => DecodedValue::Null,
        MetadataValue::Bool(b) => DecodedValue::Bool(*b),
        MetadataValue::Int(i) => DecodedValue::Int(*i),
        MetadataValue::Float(f) => DecodedValue::Float(*f),
        MetadataValue::String(s) => DecodedValue::String(s.clone()),
        MetadataValue::List(items) => {
            DecodedValue::List(items.iter().map(metadata_value_to_decoded).collect())
        }
        MetadataValue::Set(items) => {
            DecodedValue::Set(items.iter().map(metadata_value_to_decoded).collect())
        }
        MetadataValue::Map(entries) => DecodedValue::Map(
            entries
                .iter()
                .map(|(k, v)| (metadata_value_to_decoded(k), metadata_value_to_decoded(v)))
                .collect(),
        ),
        MetadataValue::PObject(obj) => DecodedValue::Object(metadata_annotation_to_decoded(obj)),
        MetadataValue::Pattern(p) => DecodedValue::Regex(p.clone()),
        MetadataValue::Pair(a, b) => DecodedValue::Pair(
            Box::new(metadata_value_to_decoded(a)),
            Box::new(metadata_value_to_decoded(b)),
        ),
        MetadataValue::DataSize { value, unit } => {
            let parsed_unit = unit
                .as_str()
                .and_then(pkl_runtime::data_size::DataSizeUnit::parse)
                .unwrap_or(pkl_runtime::data_size::DataSizeUnit::Bytes);
            DecodedValue::DataSize(pkl_runtime::data_size::DataSize::new(*value, parsed_unit))
        }
        MetadataValue::Duration { value, unit } => {
            let parsed_unit = unit
                .as_str()
                .and_then(pkl_runtime::duration::DurationUnit::parse)
                .unwrap_or(pkl_runtime::duration::DurationUnit::Seconds);
            DecodedValue::Duration(pkl_runtime::duration::Duration::new(*value, parsed_unit))
        }
    }
}

// ---------------------------------------------------------------------------
// DecodedObject extraction helpers
// ---------------------------------------------------------------------------

fn get_opt_string(obj: &DecodedObject, field: &str) -> Result<Option<String>, DocGeneratorError> {
    match obj.get_property(field) {
        None | Some(DecodedValue::Null) => Ok(None),
        Some(DecodedValue::String(s)) => Ok(Some(s.to_string())),
        Some(other) => Err(DocGeneratorError::bug(format!(
            "Expected String or null for property `{field}`, got `{other:?}`"
        ))),
    }
}

fn get_req_string(obj: &DecodedObject, field: &str) -> Result<String, DocGeneratorError> {
    match obj.get_property(field) {
        Some(DecodedValue::String(s)) => Ok(s.to_string()),
        other => Err(DocGeneratorError::bug(format!(
            "Expected String for property `{field}`, got `{other:?}`"
        ))),
    }
}

fn get_string_list(obj: &DecodedObject, field: &str) -> Result<Vec<String>, DocGeneratorError> {
    match obj.get_property(field) {
        None | Some(DecodedValue::Null) => Ok(Vec::new()),
        Some(DecodedValue::List(items) | DecodedValue::Listing(items)) => {
            let mut out = Vec::with_capacity(items.len());
            for item in items {
                match item {
                    DecodedValue::String(s) => out.push(s.to_string()),
                    other => {
                        return Err(DocGeneratorError::bug(format!(
                            "Expected String element in `{field}`, got `{other:?}`"
                        )));
                    }
                }
            }
            Ok(out)
        }
        Some(other) => Err(DocGeneratorError::bug(format!(
            "Expected List/Listing for property `{field}`, got `{other:?}`"
        ))),
    }
}

fn get_object_list<'a>(
    obj: &'a DecodedObject,
    field: &str,
) -> Result<Vec<&'a DecodedObject>, DocGeneratorError> {
    match obj.get_property(field) {
        None | Some(DecodedValue::Null) => Ok(Vec::new()),
        Some(DecodedValue::List(items) | DecodedValue::Listing(items)) => {
            let mut out = Vec::with_capacity(items.len());
            for item in items {
                match item {
                    DecodedValue::Object(o) => out.push(o),
                    other => {
                        return Err(DocGeneratorError::bug(format!(
                            "Expected Object element in `{field}`, got `{other:?}`"
                        )));
                    }
                }
            }
            Ok(out)
        }
        Some(other) => Err(DocGeneratorError::bug(format!(
            "Expected List/Listing for property `{field}`, got `{other:?}`"
        ))),
    }
}

fn get_string_map(
    obj: &DecodedObject,
    field: &str,
) -> Result<Vec<(String, String)>, DocGeneratorError> {
    match obj.get_property(field) {
        None | Some(DecodedValue::Null) => Ok(Vec::new()),
        Some(DecodedValue::Map(entries) | DecodedValue::Mapping(entries)) => {
            let mut out = Vec::with_capacity(entries.len());
            for (k, v) in entries {
                let ks = match k {
                    DecodedValue::String(s) => s.to_string(),
                    other => {
                        return Err(DocGeneratorError::bug(format!(
                            "Expected String key in `{field}`, got `{other:?}`"
                        )));
                    }
                };
                let vs = match v {
                    DecodedValue::String(s) => s.to_string(),
                    other => {
                        return Err(DocGeneratorError::bug(format!(
                            "Expected String value in `{field}`, got `{other:?}`"
                        )));
                    }
                };
                out.push((ks, vs));
            }
            Ok(out)
        }
        Some(other) => Err(DocGeneratorError::bug(format!(
            "Expected Map/Mapping for property `{field}`, got `{other:?}`"
        ))),
    }
}
