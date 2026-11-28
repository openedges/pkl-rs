//! Exact port of `PackageDataGenerator.kt`: `ElementRef`, `PackageRef`, `ModuleRef`, `TypeRef`,
//! `PackageData`, `DependencyData`, `ModuleData`, `ClassData`, `TypeAliasData`, `findTypesUsedBy`,
//! and compact JSON serialization/deserialization matching `kotlinx.serialization.json` 1.9.0.

use crate::json::{JsonValue, parse_json, push_json_string};
use crate::model::{
    DocGeneratorError, DocModule, DocPackage, DocPackageInfo, annotations_deprecation,
    annotations_is_unlisted,
};
use crate::util::{ParsedUri, encode_path, get_doc_comment_summary, relativize_uri};
use pkl_runtime::packages::PackageUri;
use pkl_runtime::schema::{
    ClassId, ModuleSchema, PClass, PType, SchemaStore, TypeAlias, TypeAliasId,
};
use std::fs;
use std::path::Path;

pub const MODULE_CLASS_NAME: &str = "ModuleClass";

pub type PackageId = String;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ModuleId {
    pub pkg: String,
    pub module: String,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TypeId {
    pub pkg: String,
    pub module: String,
    pub ty: String,
}

/// Common interface matching `ElementRef<T>` in `PackageDataGenerator.kt:51-107`.
pub trait ElementRef {
    fn pkg(&self) -> &str;
    fn pkg_uri(&self) -> Option<&str>;
    fn version(&self) -> &str;
    fn package_relative_html_path(&self) -> String;
    fn package_relative_data_path(&self) -> String;

    fn base_path(&self) -> String {
        if let Some(uri_str) = self.pkg_uri() {
            let parsed = ParsedUri::parse(uri_str);
            if parsed.scheme.as_deref() == Some("package")
                && let (Some(authority), Ok(pkg_uri)) =
                    (parsed.authority, PackageUri::parse(uri_str))
            {
                return encode_path(&format!("{authority}{}", pkg_uri.path_without_version));
            }
        }
        encode_path(self.pkg())
    }

    fn html_path(&self) -> String {
        format!(
            "{}/{}/{}",
            self.base_path(),
            encode_path(self.version()),
            self.package_relative_html_path()
        )
    }

    fn per_package_runtime_data_path(&self) -> String {
        format!(
            "data/{}/_/{}",
            self.base_path(),
            self.package_relative_data_path()
        )
    }

    fn per_package_version_runtime_data_path(&self) -> String {
        format!(
            "data/{}/{}/{}",
            self.base_path(),
            encode_path(self.version()),
            self.package_relative_data_path()
        )
    }

    fn legacy_versioned_runtime_data_path(&self) -> String {
        let data_path = self.package_relative_data_path();
        let stem = data_path.strip_suffix(".json").unwrap_or(&data_path);
        format!(
            "data/{}/{}/{stem}.js",
            self.base_path(),
            encode_path(self.version())
        )
    }

    fn page_url_relative_to(&self, other: &dyn ElementRef) -> String {
        relativize_uri(&self.html_path(), &other.html_path())
    }

    fn page_url_for_version(&self, version: &str) -> String {
        let path_to_base = relativize_uri(&self.base_path(), &self.html_path());
        format!(
            "{path_to_base}{}/{}",
            encode_path(version),
            self.package_relative_html_path()
        )
    }

    fn is_in_same_package_as(&self, other: &dyn ElementRef) -> bool {
        self.pkg() == other.pkg()
            && self.pkg_uri() == other.pkg_uri()
            && self.version() == other.version()
    }
}

/// Uniquely identifies a specific version of a package.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PackageRef {
    pub pkg: String,
    pub pkg_uri: Option<String>,
    pub version: String,
}

impl ElementRef for PackageRef {
    fn pkg(&self) -> &str {
        &self.pkg
    }
    fn pkg_uri(&self) -> Option<&str> {
        self.pkg_uri.as_deref()
    }
    fn version(&self) -> &str {
        &self.version
    }
    fn package_relative_html_path(&self) -> String {
        "index.html".to_string()
    }
    fn package_relative_data_path(&self) -> String {
        "index.json".to_string()
    }
}

/// Uniquely identifies a specific version of a module.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ModuleRef {
    pub pkg: String,
    pub pkg_uri: Option<String>,
    pub version: String,
    pub module: String,
}

impl ModuleRef {
    pub fn id(&self) -> ModuleId {
        ModuleId {
            pkg: self.pkg.clone(),
            module: self.module.clone(),
        }
    }

    pub fn full_name(&self) -> String {
        format!("{}.{}", self.pkg, self.module.replace('/', "."))
    }
}

impl ElementRef for ModuleRef {
    fn pkg(&self) -> &str {
        &self.pkg
    }
    fn pkg_uri(&self) -> Option<&str> {
        self.pkg_uri.as_deref()
    }
    fn version(&self) -> &str {
        &self.version
    }
    fn package_relative_html_path(&self) -> String {
        format!("{}/index.html", encode_path(&self.module))
    }
    fn package_relative_data_path(&self) -> String {
        encode_path(&format!("{}/index.json", self.module))
    }
}

/// Uniquely identifies a specific version of a class or type alias.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct TypeRef {
    pub pkg: String,
    pub pkg_uri: Option<String>,
    pub version: String,
    pub module: String,
    pub ty: String,
    pub is_type_alias: bool,
}

impl TypeRef {
    pub fn id(&self) -> TypeId {
        TypeId {
            pkg: self.pkg.clone(),
            module: self.module.clone(),
            ty: self.ty.clone(),
        }
    }

    pub fn is_module_class(&self) -> bool {
        self.ty == MODULE_CLASS_NAME
    }

    pub fn display_name(&self) -> &str {
        if self.is_module_class() {
            match self.module.rfind('/') {
                Some(idx) => &self.module[idx + 1..],
                None => &self.module,
            }
        } else {
            &self.ty
        }
    }
}

impl ElementRef for TypeRef {
    fn pkg(&self) -> &str {
        &self.pkg
    }
    fn pkg_uri(&self) -> Option<&str> {
        self.pkg_uri.as_deref()
    }
    fn version(&self) -> &str {
        &self.version
    }
    fn package_relative_html_path(&self) -> String {
        if self.is_type_alias {
            encode_path(&format!("{}/index.html#{}", self.module, self.ty))
        } else if self.is_module_class() {
            encode_path(&format!("{}/index.html", self.module))
        } else {
            encode_path(&format!("{}/{}.html", self.module, self.ty))
        }
    }
    fn package_relative_data_path(&self) -> String {
        debug_assert!(!self.is_type_alias, "typealiases don't have runtime data");
        if self.is_module_class() {
            encode_path(&format!("{}/index.json", self.module))
        } else {
            encode_path(&format!("{}/{}.json", self.module, self.ty))
        }
    }
}

/// Persisted data for a package (`package-data.json`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PackageData {
    pub r#ref: PackageRef,
    pub summary: Option<String>,
    pub deprecation: Option<String>,
    pub source_code: Option<String>,
    pub source_code_url_scheme: Option<String>,
    pub dependencies: Vec<DependencyData>,
    pub modules: Vec<ModuleData>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DependencyData {
    pub r#ref: PackageRef,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModuleData {
    pub r#ref: ModuleRef,
    pub summary: Option<String>,
    pub deprecation: Option<String>,
    pub supermodules: Vec<ModuleRef>,
    pub module_class: Option<ClassData>,
    pub classes: Vec<ClassData>,
    pub type_aliases: Vec<TypeAliasData>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClassData {
    pub r#ref: TypeRef,
    pub superclasses: Vec<TypeRef>,
    pub used_types: Vec<TypeRef>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TypeAliasData {
    pub r#ref: TypeRef,
    pub used_types: Vec<TypeRef>,
}

impl PackageData {
    pub fn from_doc_package(pkg: &DocPackage) -> Self {
        let info = &pkg.doc_package_info;
        let dependencies = info
            .dependencies
            .iter()
            .map(|dep| DependencyData {
                r#ref: PackageRef {
                    pkg: dep.name.clone(),
                    pkg_uri: dep.uri.clone(),
                    version: dep.version.clone(),
                },
            })
            .collect();
        let modules = pkg
            .doc_modules
            .iter()
            .filter(|m| !m.is_unlisted)
            .map(|m| ModuleData::from_doc_module(pkg, m))
            .collect();
        Self {
            r#ref: PackageRef {
                pkg: info.name.clone(),
                pkg_uri: info.uri.clone(),
                version: info.version.clone(),
            },
            summary: get_doc_comment_summary(info.overview.as_deref()),
            deprecation: annotations_deprecation(&info.annotations),
            source_code: info.source_code.clone(),
            source_code_url_scheme: info.source_code_url_scheme.clone(),
            dependencies,
            modules,
        }
    }

    pub fn read(path: &Path) -> Result<Self, DocGeneratorError> {
        let json_str = pkl_host_native::read_path_to_string_bounded(path).map_err(|e| {
            DocGeneratorError::bug(format!("I/O error reading `{}`: {e}", path.display()))
        })?;
        let value = parse_json(&json_str).map_err(|e| {
            DocGeneratorError::bug(format!("Error deserializing `{}`: {e}", path.display()))
        })?;
        Self::from_json(&value).map_err(|e| {
            DocGeneratorError::bug(format!("Error deserializing `{}`: {e}", path.display()))
        })
    }

    pub fn write(&self, path: &Path) -> Result<(), DocGeneratorError> {
        let json_str = self.to_json_string();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                DocGeneratorError::bug(format!("I/O error writing `{}`: {e}", path.display()))
            })?;
        }
        pkl_host_native::write_file_nofollow(path, json_str.as_bytes()).map_err(|e| {
            DocGeneratorError::bug(format!("I/O error writing `{}`: {e}", path.display()))
        })
    }

    pub fn to_json_string(&self) -> String {
        let mut out = String::new();
        self.write_json(&mut out);
        out
    }

    fn write_json(&self, out: &mut String) {
        out.push('{');
        out.push_str("\"ref\":");
        self.r#ref.write_json(out);
        if let Some(ref summary) = self.summary {
            out.push_str(",\"summary\":");
            push_json_string(out, summary);
        }
        if let Some(ref deprecation) = self.deprecation {
            out.push_str(",\"deprecation\":");
            push_json_string(out, deprecation);
        }
        // `sourceCode` and `sourceCodeUrlScheme` have NO default value in Kotlin -> always emitted!
        out.push_str(",\"sourceCode\":");
        match &self.source_code {
            Some(s) => push_json_string(out, s),
            None => out.push_str("null"),
        }
        out.push_str(",\"sourceCodeUrlScheme\":");
        match &self.source_code_url_scheme {
            Some(s) => push_json_string(out, s),
            None => out.push_str("null"),
        }
        if !self.dependencies.is_empty() {
            out.push_str(",\"dependencies\":[");
            for (idx, dep) in self.dependencies.iter().enumerate() {
                if idx > 0 {
                    out.push(',');
                }
                dep.write_json(out);
            }
            out.push(']');
        }
        if !self.modules.is_empty() {
            out.push_str(",\"modules\":[");
            for (idx, m) in self.modules.iter().enumerate() {
                if idx > 0 {
                    out.push(',');
                }
                m.write_json(out);
            }
            out.push(']');
        }
        out.push('}');
    }

    fn from_json(v: &JsonValue) -> Result<Self, DocGeneratorError> {
        let r#ref = PackageRef::from_json(req_field(v, "ref")?)?;
        let summary = opt_string_field(v, "summary")?;
        let deprecation = opt_string_field(v, "deprecation")?;
        let source_code = opt_string_field(v, "sourceCode")?;
        let source_code_url_scheme = opt_string_field(v, "sourceCodeUrlScheme")?;
        let dependencies = match v.get("dependencies") {
            None | Some(JsonValue::Null) => Vec::new(),
            Some(JsonValue::Array(items)) => items
                .iter()
                .map(DependencyData::from_json)
                .collect::<Result<Vec<_>, _>>()?,
            Some(_) => return Err(DocGeneratorError::bug("Expected array for `dependencies`")),
        };
        let modules = match v.get("modules") {
            None | Some(JsonValue::Null) => Vec::new(),
            Some(JsonValue::Array(items)) => items
                .iter()
                .map(ModuleData::from_json)
                .collect::<Result<Vec<_>, _>>()?,
            Some(_) => return Err(DocGeneratorError::bug("Expected array for `modules`")),
        };
        Ok(Self {
            r#ref,
            summary,
            deprecation,
            source_code,
            source_code_url_scheme,
            dependencies,
            modules,
        })
    }
}

impl DependencyData {
    fn write_json(&self, out: &mut String) {
        out.push_str("{\"ref\":");
        self.r#ref.write_json(out);
        out.push('}');
    }

    fn from_json(v: &JsonValue) -> Result<Self, DocGeneratorError> {
        Ok(Self {
            r#ref: PackageRef::from_json(req_field(v, "ref")?)?,
        })
    }
}

impl ModuleData {
    fn from_doc_module(pkg: &DocPackage, module: &DocModule) -> Self {
        let info = &pkg.doc_package_info;
        // Exact upstream behavior (`PackageDataGenerator.kt:327-330`):
        // maps each supermodule step using `pkg.docPackageInfo.getModuleRef(module.name)` (the child module name).
        let mut supermodules = Vec::new();
        let mut cur = module.schema.supermodule.as_deref();
        while let Some(sup) = cur {
            if let Some(mref) = info.get_module_ref(module.name()) {
                supermodules.push(mref);
            }
            cur = sup.supermodule.as_deref();
        }

        let module_class = if module.schema.is_amend {
            None
        } else {
            Some(ClassData::from_class(
                pkg,
                module,
                &module.schema,
                module.schema.module_class_id,
            ))
        };

        let classes = module
            .schema
            .classes
            .iter()
            .filter_map(|(_, cid)| {
                let c = module.schema.store.class(*cid);
                if annotations_is_unlisted(&c.annotations) {
                    None
                } else {
                    Some(ClassData::from_class(pkg, module, &module.schema, *cid))
                }
            })
            .collect();

        let type_aliases = module
            .schema
            .type_aliases
            .iter()
            .filter_map(|(_, aid)| {
                let a = module.schema.store.type_alias(*aid);
                if annotations_is_unlisted(&a.annotations) {
                    None
                } else {
                    Some(TypeAliasData::from_alias(pkg, module, &module.schema, *aid))
                }
            })
            .collect();

        Self {
            r#ref: ModuleRef {
                pkg: info.name.clone(),
                pkg_uri: info.uri.clone(),
                version: info.version.clone(),
                module: module.path.clone(),
            },
            summary: get_doc_comment_summary(module.overview()),
            deprecation: annotations_deprecation(&module.schema.annotations),
            supermodules,
            module_class,
            classes,
            type_aliases,
        }
    }

    fn write_json(&self, out: &mut String) {
        out.push('{');
        out.push_str("\"ref\":");
        self.r#ref.write_json(out);
        if let Some(ref summary) = self.summary {
            out.push_str(",\"summary\":");
            push_json_string(out, summary);
        }
        if let Some(ref deprecation) = self.deprecation {
            out.push_str(",\"deprecation\":");
            push_json_string(out, deprecation);
        }
        if !self.supermodules.is_empty() {
            out.push_str(",\"supermodules\":[");
            for (idx, sm) in self.supermodules.iter().enumerate() {
                if idx > 0 {
                    out.push(',');
                }
                sm.write_json(out);
            }
            out.push(']');
        }
        if let Some(ref mc) = self.module_class {
            out.push_str(",\"moduleClass\":");
            mc.write_json(out);
        }
        if !self.classes.is_empty() {
            out.push_str(",\"classes\":[");
            for (idx, c) in self.classes.iter().enumerate() {
                if idx > 0 {
                    out.push(',');
                }
                c.write_json(out);
            }
            out.push(']');
        }
        if !self.type_aliases.is_empty() {
            out.push_str(",\"typeAliases\":[");
            for (idx, a) in self.type_aliases.iter().enumerate() {
                if idx > 0 {
                    out.push(',');
                }
                a.write_json(out);
            }
            out.push(']');
        }
        out.push('}');
    }

    fn from_json(v: &JsonValue) -> Result<Self, DocGeneratorError> {
        let r#ref = ModuleRef::from_json(req_field(v, "ref")?)?;
        let summary = opt_string_field(v, "summary")?;
        let deprecation = opt_string_field(v, "deprecation")?;
        let supermodules = match v.get("supermodules") {
            None | Some(JsonValue::Null) => Vec::new(),
            Some(JsonValue::Array(items)) => items
                .iter()
                .map(ModuleRef::from_json)
                .collect::<Result<Vec<_>, _>>()?,
            Some(_) => return Err(DocGeneratorError::bug("Expected array for `supermodules`")),
        };
        let module_class = match v.get("moduleClass") {
            None | Some(JsonValue::Null) => None,
            Some(obj) => Some(ClassData::from_json(obj)?),
        };
        let classes = match v.get("classes") {
            None | Some(JsonValue::Null) => Vec::new(),
            Some(JsonValue::Array(items)) => items
                .iter()
                .map(ClassData::from_json)
                .collect::<Result<Vec<_>, _>>()?,
            Some(_) => return Err(DocGeneratorError::bug("Expected array for `classes`")),
        };
        let type_aliases = match v.get("typeAliases") {
            None | Some(JsonValue::Null) => Vec::new(),
            Some(JsonValue::Array(items)) => items
                .iter()
                .map(TypeAliasData::from_json)
                .collect::<Result<Vec<_>, _>>()?,
            Some(_) => return Err(DocGeneratorError::bug("Expected array for `typeAliases`")),
        };
        Ok(Self {
            r#ref,
            summary,
            deprecation,
            supermodules,
            module_class,
            classes,
            type_aliases,
        })
    }
}

impl ClassData {
    fn from_class(
        pkg: &DocPackage,
        module: &DocModule,
        schema: &ModuleSchema,
        cid: ClassId,
    ) -> Self {
        let info = &pkg.doc_package_info;
        let clazz = schema.store.class(cid);
        let mut superclasses = Vec::new();
        let mut cur = clazz.superclass;
        while let Some(sup_id) = cur {
            let sup = schema.store.class(sup_id);
            if let Some(tref) =
                info.get_type_ref(&sup.info.module_name, &sup.info.simple_name, false)
            {
                superclasses.push(tref);
            }
            cur = sup.superclass;
        }
        let used_types = find_types_used_by_class(&schema.store, clazz, info);
        Self {
            r#ref: TypeRef {
                pkg: info.name.clone(),
                pkg_uri: info.uri.clone(),
                version: info.version.clone(),
                module: module.path.clone(),
                ty: clazz.info.simple_name.clone(),
                is_type_alias: false,
            },
            superclasses,
            used_types,
        }
    }

    fn write_json(&self, out: &mut String) {
        out.push('{');
        out.push_str("\"ref\":");
        self.r#ref.write_json(out);
        if !self.superclasses.is_empty() {
            out.push_str(",\"superclasses\":[");
            for (idx, sc) in self.superclasses.iter().enumerate() {
                if idx > 0 {
                    out.push(',');
                }
                sc.write_json(out);
            }
            out.push(']');
        }
        if !self.used_types.is_empty() {
            out.push_str(",\"usedTypes\":[");
            for (idx, ut) in self.used_types.iter().enumerate() {
                if idx > 0 {
                    out.push(',');
                }
                ut.write_json(out);
            }
            out.push(']');
        }
        out.push('}');
    }

    fn from_json(v: &JsonValue) -> Result<Self, DocGeneratorError> {
        let r#ref = TypeRef::from_json(req_field(v, "ref")?)?;
        let superclasses = match v.get("superclasses") {
            None | Some(JsonValue::Null) => Vec::new(),
            Some(JsonValue::Array(items)) => items
                .iter()
                .map(TypeRef::from_json)
                .collect::<Result<Vec<_>, _>>()?,
            Some(_) => return Err(DocGeneratorError::bug("Expected array for `superclasses`")),
        };
        let used_types = match v.get("usedTypes") {
            None | Some(JsonValue::Null) => Vec::new(),
            Some(JsonValue::Array(items)) => items
                .iter()
                .map(TypeRef::from_json)
                .collect::<Result<Vec<_>, _>>()?,
            Some(_) => return Err(DocGeneratorError::bug("Expected array for `usedTypes`")),
        };
        Ok(Self {
            r#ref,
            superclasses,
            used_types,
        })
    }
}

impl TypeAliasData {
    fn from_alias(
        pkg: &DocPackage,
        module: &DocModule,
        schema: &ModuleSchema,
        aid: TypeAliasId,
    ) -> Self {
        let info = &pkg.doc_package_info;
        let alias = schema.store.type_alias(aid);
        let used_types = find_types_used_by_alias(&schema.store, alias, info);
        Self {
            r#ref: TypeRef {
                pkg: info.name.clone(),
                pkg_uri: info.uri.clone(),
                version: info.version.clone(),
                module: module.path.clone(),
                ty: alias.info.simple_name.clone(),
                is_type_alias: true,
            },
            used_types,
        }
    }

    fn write_json(&self, out: &mut String) {
        out.push('{');
        out.push_str("\"ref\":");
        self.r#ref.write_json(out);
        if !self.used_types.is_empty() {
            out.push_str(",\"usedTypes\":[");
            for (idx, ut) in self.used_types.iter().enumerate() {
                if idx > 0 {
                    out.push(',');
                }
                ut.write_json(out);
            }
            out.push(']');
        }
        out.push('}');
    }

    fn from_json(v: &JsonValue) -> Result<Self, DocGeneratorError> {
        let r#ref = TypeRef::from_json(req_field(v, "ref")?)?;
        let used_types = match v.get("usedTypes") {
            None | Some(JsonValue::Null) => Vec::new(),
            Some(JsonValue::Array(items)) => items
                .iter()
                .map(TypeRef::from_json)
                .collect::<Result<Vec<_>, _>>()?,
            Some(_) => return Err(DocGeneratorError::bug("Expected array for `usedTypes`")),
        };
        Ok(Self { r#ref, used_types })
    }
}

impl PackageRef {
    fn write_json(&self, out: &mut String) {
        out.push_str("{\"pkg\":");
        push_json_string(out, &self.pkg);
        out.push_str(",\"pkgUri\":");
        match &self.pkg_uri {
            Some(u) => push_json_string(out, u),
            None => out.push_str("null"),
        }
        out.push_str(",\"version\":");
        push_json_string(out, &self.version);
        out.push('}');
    }

    fn from_json(v: &JsonValue) -> Result<Self, DocGeneratorError> {
        Ok(Self {
            pkg: req_string_field(v, "pkg")?,
            pkg_uri: opt_string_field(v, "pkgUri")?,
            version: req_string_field(v, "version")?,
        })
    }
}

impl ModuleRef {
    fn write_json(&self, out: &mut String) {
        out.push_str("{\"pkg\":");
        push_json_string(out, &self.pkg);
        out.push_str(",\"pkgUri\":");
        match &self.pkg_uri {
            Some(u) => push_json_string(out, u),
            None => out.push_str("null"),
        }
        out.push_str(",\"version\":");
        push_json_string(out, &self.version);
        out.push_str(",\"module\":");
        push_json_string(out, &self.module);
        out.push('}');
    }

    fn from_json(v: &JsonValue) -> Result<Self, DocGeneratorError> {
        Ok(Self {
            pkg: req_string_field(v, "pkg")?,
            pkg_uri: opt_string_field(v, "pkgUri")?,
            version: req_string_field(v, "version")?,
            module: req_string_field(v, "module")?,
        })
    }
}

impl TypeRef {
    fn write_json(&self, out: &mut String) {
        out.push_str("{\"pkg\":");
        push_json_string(out, &self.pkg);
        out.push_str(",\"pkgUri\":");
        match &self.pkg_uri {
            Some(u) => push_json_string(out, u),
            None => out.push_str("null"),
        }
        out.push_str(",\"version\":");
        push_json_string(out, &self.version);
        out.push_str(",\"module\":");
        push_json_string(out, &self.module);
        out.push_str(",\"type\":");
        push_json_string(out, &self.ty);
        if self.is_type_alias {
            out.push_str(",\"isTypeAlias\":true");
        }
        out.push('}');
    }

    fn from_json(v: &JsonValue) -> Result<Self, DocGeneratorError> {
        let is_type_alias = match v.get("isTypeAlias") {
            None | Some(JsonValue::Null) => false,
            Some(JsonValue::Bool(b)) => *b,
            Some(_) => return Err(DocGeneratorError::bug("Expected bool for `isTypeAlias`")),
        };
        Ok(Self {
            pkg: req_string_field(v, "pkg")?,
            pkg_uri: opt_string_field(v, "pkgUri")?,
            version: req_string_field(v, "version")?,
            module: req_string_field(v, "module")?,
            ty: req_string_field(v, "type")?,
            is_type_alias,
        })
    }
}

// ---------------------------------------------------------------------------
// `findTypesUsedBy` (`PackageDataGenerator.kt:399-487`)
// ---------------------------------------------------------------------------

#[derive(Clone, Copy)]
enum EnclosingType<'a> {
    Class(&'a PClass),
    Alias(&'a TypeAlias),
}

impl<'a> EnclosingType<'a> {
    fn module_name(&self) -> &'a str {
        match self {
            Self::Class(c) => &c.info.module_name,
            Self::Alias(a) => &a.info.module_name,
        }
    }

    fn simple_name(&self) -> &'a str {
        match self {
            Self::Class(c) => &c.info.simple_name,
            Self::Alias(a) => &a.info.simple_name,
        }
    }

    fn is_same_class(&self, other: &PClass) -> bool {
        match self {
            Self::Class(c) => c.id == other.id || c.info == other.info,
            Self::Alias(_) => false,
        }
    }

    fn is_same_alias(&self, other: &TypeAlias) -> bool {
        match self {
            Self::Alias(a) => a.id == other.id || a.info == other.info,
            Self::Class(_) => false,
        }
    }
}

fn push_unique_type_ref(out: &mut Vec<TypeRef>, tref: TypeRef) {
    if !out.contains(&tref) {
        out.push(tref);
    }
}

fn find_types_used_by_class(
    store: &SchemaStore,
    clazz: &PClass,
    enclosing_package: &DocPackageInfo,
) -> Vec<TypeRef> {
    let mut result = Vec::new();
    let enclosing = EnclosingType::Class(clazz);
    if let Some(PType::Class { type_arguments, .. }) = &clazz.supertype {
        for arg in type_arguments {
            find_types_used_by(store, arg, enclosing, enclosing_package, &mut result);
        }
    }
    for (_, prop) in &clazz.properties {
        find_types_used_by(store, &prop.ty, enclosing, enclosing_package, &mut result);
    }
    for (_, method) in &clazz.methods {
        for (_, param_ty) in &method.parameters {
            find_types_used_by(store, param_ty, enclosing, enclosing_package, &mut result);
        }
        find_types_used_by(
            store,
            &method.return_type,
            enclosing,
            enclosing_package,
            &mut result,
        );
    }
    result
}

fn find_types_used_by_alias(
    store: &SchemaStore,
    alias: &TypeAlias,
    enclosing_package: &DocPackageInfo,
) -> Vec<TypeRef> {
    let mut result = Vec::new();
    find_types_used_by(
        store,
        &alias.aliased_type,
        EnclosingType::Alias(alias),
        enclosing_package,
        &mut result,
    );
    result
}

fn find_types_used_by(
    store: &SchemaStore,
    ty: &PType,
    enclosing: EnclosingType<'_>,
    enclosing_package: &DocPackageInfo,
    result: &mut Vec<TypeRef>,
) {
    match ty {
        PType::Class {
            class_id,
            type_arguments,
            ..
        } => {
            let target = store.class(*class_id);
            let is_stdlib = target.info.module_name.starts_with("pkl.");
            if !is_stdlib
                && !annotations_is_unlisted(&target.annotations)
                && !enclosing.is_same_class(target)
                && let Some(tref) = enclosing_package.get_type_ref(
                    &target.info.module_name,
                    &target.info.simple_name,
                    false,
                )
            {
                push_unique_type_ref(result, tref);
            }
            for arg in type_arguments {
                find_types_used_by(store, arg, enclosing, enclosing_package, result);
            }
        }
        PType::Alias { alias_id, .. } => {
            let target = store.type_alias(*alias_id);
            let is_stdlib = target.info.module_name.starts_with("pkl.");
            if !is_stdlib
                && !annotations_is_unlisted(&target.annotations)
                && !enclosing.is_same_alias(target)
                && let Some(tref) = enclosing_package.get_type_ref(
                    &target.info.module_name,
                    &target.info.simple_name,
                    true,
                )
            {
                push_unique_type_ref(result, tref);
            }
        }
        PType::Constrained { base_type, .. } => {
            find_types_used_by(store, base_type, enclosing, enclosing_package, result);
        }
        PType::Function {
            parameter_types,
            return_type,
        } => {
            for pt in parameter_types {
                find_types_used_by(store, pt, enclosing, enclosing_package, result);
            }
            find_types_used_by(store, return_type, enclosing, enclosing_package, result);
        }
        PType::Module => {
            if enclosing.simple_name() != MODULE_CLASS_NAME {
                let mod_name = enclosing.module_name();
                let mod_path = mod_name
                    .get(enclosing_package.module_name_prefix.len()..)
                    .unwrap_or(mod_name)
                    .replace('.', "/");
                push_unique_type_ref(
                    result,
                    TypeRef {
                        pkg: enclosing_package.name.clone(),
                        pkg_uri: enclosing_package.uri.clone(),
                        version: enclosing_package.version.clone(),
                        module: mod_path,
                        ty: MODULE_CLASS_NAME.to_string(),
                        is_type_alias: false,
                    },
                );
            }
        }
        PType::Nullable(inner) => {
            find_types_used_by(store, inner, enclosing, enclosing_package, result);
        }
        PType::Union(elements) => {
            for elem in elements {
                find_types_used_by(store, elem, enclosing, enclosing_package, result);
            }
        }
        PType::Nothing | PType::StringLiteral(_) | PType::TypeVariable(_) | PType::Unknown => {}
        _ => {}
    }
}

pub fn generate_package_data(output_dir: &Path, pkg: &DocPackage) -> Result<(), DocGeneratorError> {
    let path = output_dir
        .join(encode_path(pkg.name()))
        .join(pkg.version())
        .join("package-data.json");
    PackageData::from_doc_package(pkg).write(&path)
}

// ---------------------------------------------------------------------------
// JSON field extraction helpers
// ---------------------------------------------------------------------------

fn req_field<'a>(v: &'a JsonValue, name: &str) -> Result<&'a JsonValue, DocGeneratorError> {
    v.get(name)
        .ok_or_else(|| DocGeneratorError::bug(format!("Missing required JSON field `{name}`")))
}

fn req_string_field(v: &JsonValue, name: &str) -> Result<String, DocGeneratorError> {
    match v.get(name) {
        Some(JsonValue::String(s)) => Ok(s.clone()),
        _ => Err(DocGeneratorError::bug(format!(
            "Expected string JSON field `{name}`"
        ))),
    }
}

fn opt_string_field(v: &JsonValue, name: &str) -> Result<Option<String>, DocGeneratorError> {
    match v.get(name) {
        None | Some(JsonValue::Null) => Ok(None),
        Some(JsonValue::String(s)) => Ok(Some(s.clone())),
        _ => Err(DocGeneratorError::bug(format!(
            "Expected optional string JSON field `{name}`"
        ))),
    }
}
