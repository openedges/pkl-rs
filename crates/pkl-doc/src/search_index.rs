//! Exact port of `SearchIndexGenerator.kt`.

use crate::json::{JsonValue, parse_json, push_json_string};
use crate::model::{
    DocGeneratorError, DocModule, DocPackage, annotations_also_known_as, annotations_is_unlisted,
    is_base_annotation,
};
use crate::package_data::{ElementRef, ModuleData, PackageData};
use crate::runtime_data::path_to_uri_display;
use crate::util::encode_path;
use pkl_runtime::pkl_binary_decode::DecodedObject;
use pkl_runtime::schema::{PClass, PClassMethod, PClassProperty, PType, SchemaStore, TypeAlias};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

const PREFIX: &str = "searchData='";
const POSTFIX: &str = "';\n";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum Kind {
    Package = 0,
    Module = 1,
    TypeAlias = 2,
    Class = 3,
    Method = 4,
    Property = 5,
}

impl Kind {
    pub fn from_i32(v: i32) -> Result<Self, DocGeneratorError> {
        match v {
            0 => Ok(Self::Package),
            1 => Ok(Self::Module),
            2 => Ok(Self::TypeAlias),
            3 => Ok(Self::Class),
            4 => Ok(Self::Method),
            5 => Ok(Self::Property),
            other => Err(DocGeneratorError::bug(format!(
                "Unknown Kind value: {other}"
            ))),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SearchIndexEntry {
    pub name: String,
    pub kind: Kind,
    pub url: String,
    pub sig: Option<String>,
    pub par_id: Option<i32>,
    pub deprecated: Option<bool>,
    pub aka: Option<Vec<String>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PackageIndexEntry {
    pub package_entry: SearchIndexEntry,
    pub module_entries: Vec<SearchIndexEntry>,
}

impl SearchIndexEntry {
    fn with_annotations(mut self, annotations: &[DecodedObject]) -> Self {
        let deprecated = annotations
            .iter()
            .any(|a| is_base_annotation(a, "Deprecated"))
            .then_some(true);
        let aka = annotations_also_known_as(annotations);
        self.deprecated = deprecated;
        self.aka = aka;
        self
    }

    fn write_json(&self, out: &mut String) {
        out.push_str("{\"name\":");
        push_json_string(out, &self.name);
        out.push_str(",\"kind\":");
        out.push_str(&(self.kind as i32).to_string());
        out.push_str(",\"url\":");
        push_json_string(out, &self.url);
        if let Some(ref sig) = self.sig {
            out.push_str(",\"sig\":");
            push_json_string(out, sig);
        }
        if let Some(par_id) = self.par_id {
            out.push_str(",\"parId\":");
            out.push_str(&par_id.to_string());
        }
        if let Some(dep) = self.deprecated {
            out.push_str(",\"deprecated\":");
            out.push_str(if dep { "true" } else { "false" });
        }
        if let Some(ref aka) = self.aka {
            out.push_str(",\"aka\":[");
            for (idx, item) in aka.iter().enumerate() {
                if idx > 0 {
                    out.push(',');
                }
                push_json_string(out, item);
            }
            out.push(']');
        }
        out.push('}');
    }

    fn from_json(v: &JsonValue) -> Result<Self, DocGeneratorError> {
        let name = match v.get("name") {
            Some(JsonValue::String(s)) => s.clone(),
            _ => {
                return Err(DocGeneratorError::bug(
                    "Missing string `name` in search index",
                ));
            }
        };
        let kind_int = match v.get("kind") {
            Some(JsonValue::Number(n)) => n
                .parse::<i32>()
                .map_err(|e| DocGeneratorError::bug(format!("Invalid `kind`: {e}")))?,
            _ => return Err(DocGeneratorError::bug("Missing int `kind` in search index")),
        };
        let kind = Kind::from_i32(kind_int)?;
        let url = match v.get("url") {
            Some(JsonValue::String(s)) => s.clone(),
            _ => {
                return Err(DocGeneratorError::bug(
                    "Missing string `url` in search index",
                ));
            }
        };
        let sig = match v.get("sig") {
            None | Some(JsonValue::Null) => None,
            Some(JsonValue::String(s)) => Some(s.clone()),
            _ => return Err(DocGeneratorError::bug("Invalid `sig` in search index")),
        };
        let par_id = match v.get("parId") {
            None | Some(JsonValue::Null) => None,
            Some(JsonValue::Number(n)) => Some(
                n.parse::<i32>()
                    .map_err(|e| DocGeneratorError::bug(format!("Invalid `parId`: {e}")))?,
            ),
            _ => return Err(DocGeneratorError::bug("Invalid `parId` in search index")),
        };
        let deprecated = match v.get("deprecated") {
            None | Some(JsonValue::Null) => None,
            Some(JsonValue::Bool(b)) => Some(*b),
            _ => {
                return Err(DocGeneratorError::bug(
                    "Invalid `deprecated` in search index",
                ));
            }
        };
        let aka = match v.get("aka") {
            None | Some(JsonValue::Null) => None,
            Some(JsonValue::Array(items)) => {
                let mut out = Vec::with_capacity(items.len());
                for item in items {
                    match item {
                        JsonValue::String(s) => out.push(s.clone()),
                        _ => return Err(DocGeneratorError::bug("Invalid `aka` item")),
                    }
                }
                Some(out)
            }
            _ => return Err(DocGeneratorError::bug("Invalid `aka` in search index")),
        };
        Ok(Self {
            name,
            kind,
            url,
            sig,
            par_id,
            deprecated,
            aka,
        })
    }
}

pub struct SearchIndexGenerator<'a> {
    output_dir: &'a Path,
    search_index_file: PathBuf,
    console_out: &'a mut dyn Write,
}

impl<'a> SearchIndexGenerator<'a> {
    pub fn new(output_dir: &'a Path, console_out: &'a mut dyn Write) -> Self {
        Self {
            output_dir,
            search_index_file: output_dir.join("search-index.js"),
            console_out,
        }
    }

    pub fn get_current_search_index(
        &mut self,
    ) -> Result<Vec<PackageIndexEntry>, DocGeneratorError> {
        if !self.search_index_file.is_file() {
            return Ok(Vec::new());
        }
        let text = fs::read_to_string(&self.search_index_file).map_err(|e| {
            DocGeneratorError::bug(format!(
                "I/O error reading `{}`: {e}",
                self.search_index_file.display()
            ))
        })?;
        if !(text.starts_with(PREFIX) && text.ends_with(POSTFIX)) {
            let _ = writeln!(
                self.console_out,
                "[error] Incorrect existing search-index.js; either doesnt start with prefix '{PREFIX}', or end with postfix '{POSTFIX}'"
            );
            return Ok(Vec::new());
        }
        let json_str = &text[PREFIX.len()..text.len() - POSTFIX.len()];
        let val = parse_json(json_str)?;
        let items = val
            .as_array()
            .ok_or_else(|| DocGeneratorError::bug("Expected array in search-index.js"))?;
        let entries: Vec<SearchIndexEntry> = items
            .iter()
            .map(SearchIndexEntry::from_json)
            .collect::<Result<Vec<_>, _>>()?;

        let mut result = Vec::new();
        let mut i = 0;
        while i < entries.len() {
            let package_entry = entries[i].clone();
            i += 1;
            let mut module_entries = Vec::new();
            while i < entries.len() && entries[i].kind == Kind::Module {
                module_entries.push(entries[i].clone());
                i += 1;
            }
            result.push(PackageIndexEntry {
                package_entry,
                module_entries,
            });
        }
        Ok(result)
    }

    pub fn build_search_index(packages: &[PackageData]) -> Vec<PackageIndexEntry> {
        packages
            .iter()
            .map(|pkg| {
                let pkg_path = format!("{}/current", encode_path(&pkg.r#ref.pkg));
                let package_entry = package_data_to_entry(pkg);
                let module_entries = pkg
                    .modules
                    .iter()
                    .map(|m| module_data_to_entry(m, &pkg_path))
                    .collect();
                PackageIndexEntry {
                    package_entry,
                    module_entries,
                }
            })
            .collect()
    }

    pub fn generate_site_index(
        &mut self,
        current_packages: &[PackageData],
    ) -> Result<(), DocGeneratorError> {
        let search_index = Self::build_search_index(current_packages);
        let mut entries = Vec::new();
        for pkg_idx in search_index {
            entries.push(pkg_idx.package_entry);
            entries.extend(pkg_idx.module_entries);
        }
        self.write_entries(&self.search_index_file.clone(), &entries)
    }

    pub fn generate(&mut self, doc_package: &DocPackage) -> Result<(), DocGeneratorError> {
        let path = self.output_dir.join(format!(
            "{}/{}/search-index.js",
            encode_path(doc_package.name()),
            doc_package.version()
        ));
        let mut entries = Vec::new();
        let mut next_id: i32 = 0;

        for doc_module in &doc_package.doc_modules {
            if doc_module.is_unlisted {
                continue;
            }
            let module = &doc_module.schema;
            let store = &module.store;
            let module_id = next_id;
            next_id += 1;
            entries.push(doc_module_to_entry(doc_module));
            let module_base_path = &doc_module.path;
            let module_index_path = format!("{module_base_path}/index.html");

            let mc = module.module_class();
            for (_, property) in &mc.properties {
                if annotations_is_unlisted(&property.annotations) {
                    continue;
                }
                next_id += 1;
                entries.push(property_to_entry(
                    store,
                    property,
                    module_id,
                    &module_index_path,
                ));
            }
            for (_, method) in &mc.methods {
                if annotations_is_unlisted(&method.annotations) {
                    continue;
                }
                next_id += 1;
                entries.push(method_to_entry(
                    store,
                    method,
                    module_id,
                    &module_index_path,
                ));
            }
            for (_, cid) in &module.classes {
                let clazz = store.class(*cid);
                if annotations_is_unlisted(&clazz.annotations) {
                    continue;
                }
                let class_id = next_id;
                next_id += 1;
                entries.push(class_to_entry(clazz, module_id, module_base_path));
                // Exact upstream quirk (`SearchIndexGenerator.kt:286`): unencoded `clazz.simpleName`.
                let class_base_path = format!("{}/{}.html", doc_module.path, clazz.simple_name());

                for (_, property) in &clazz.properties {
                    if annotations_is_unlisted(&property.annotations) {
                        continue;
                    }
                    next_id += 1;
                    entries.push(property_to_entry(
                        store,
                        property,
                        class_id,
                        &class_base_path,
                    ));
                }
                for (_, method) in &clazz.methods {
                    if annotations_is_unlisted(&method.annotations) {
                        continue;
                    }
                    next_id += 1;
                    entries.push(method_to_entry(store, method, class_id, &class_base_path));
                }
            }
            // Exact upstream behavior (`SearchIndexGenerator.kt:303`): does NOT filter `isUnlisted`.
            for (_, aid) in &module.type_aliases {
                let alias = store.type_alias(*aid);
                next_id += 1;
                entries.push(type_alias_to_entry(alias, module_id, &module_index_path));
            }
        }

        self.write_entries(&path, &entries)
    }

    fn write_entries(
        &mut self,
        path: &Path,
        entries: &[SearchIndexEntry],
    ) -> Result<(), DocGeneratorError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                DocGeneratorError::bug(format!("I/O error writing `{}`: {e}", path.display()))
            })?;
        }
        let mut out = String::from(PREFIX);
        out.push('[');
        for (idx, entry) in entries.iter().enumerate() {
            if idx > 0 {
                out.push(',');
            }
            entry.write_json(&mut out);
        }
        out.push(']');
        out.push_str(POSTFIX);
        fs::write(path, out.as_bytes()).map_err(|e| {
            DocGeneratorError::bug(format!("I/O error writing `{}`: {e}", path.display()))
        })?;
        let _ = write!(
            self.console_out,
            "Wrote file {}\r",
            path_to_uri_display(path)
        );
        Ok(())
    }
}

fn package_data_to_entry(pkg: &PackageData) -> SearchIndexEntry {
    let pkg_path = format!("{}/current", encode_path(&pkg.r#ref.pkg));
    SearchIndexEntry {
        name: pkg.r#ref.pkg.clone(),
        kind: Kind::Package,
        url: format!("{pkg_path}/{}", pkg.r#ref.package_relative_html_path()),
        sig: None,
        par_id: None,
        deprecated: pkg.deprecation.as_ref().map(|_| true),
        aka: None,
    }
}

fn module_data_to_entry(module: &ModuleData, base_path: &str) -> SearchIndexEntry {
    SearchIndexEntry {
        name: module.r#ref.full_name(),
        kind: Kind::Module,
        url: format!("{base_path}/{}", module.r#ref.package_relative_html_path()),
        sig: None,
        par_id: None,
        deprecated: module.deprecation.as_ref().map(|_| true),
        aka: None,
    }
}

fn doc_module_to_entry(doc_module: &DocModule) -> SearchIndexEntry {
    let mc = doc_module.schema.module_class();
    SearchIndexEntry {
        name: doc_module.schema.module_name.clone(),
        kind: Kind::Module,
        url: format!("{}/index.html", doc_module.path),
        sig: None,
        par_id: None,
        deprecated: None,
        aka: None,
    }
    .with_annotations(&mc.annotations)
}

fn property_to_entry(
    store: &SchemaStore,
    property: &PClassProperty,
    parent_id: i32,
    base_path: &str,
) -> SearchIndexEntry {
    SearchIndexEntry {
        name: property.simple_name.clone(),
        kind: Kind::Property,
        url: format!("{base_path}#{}", property.simple_name),
        sig: Some(render_property_signature(store, property)),
        par_id: Some(parent_id),
        deprecated: None,
        aka: None,
    }
    .with_annotations(&property.annotations)
}

fn method_to_entry(
    store: &SchemaStore,
    method: &PClassMethod,
    parent_id: i32,
    base_path: &str,
) -> SearchIndexEntry {
    SearchIndexEntry {
        name: method.simple_name.clone(),
        kind: Kind::Method,
        url: format!("{base_path}#{}()", encode_path(&method.simple_name)),
        sig: Some(render_method_signature(store, method)),
        par_id: Some(parent_id),
        deprecated: None,
        aka: None,
    }
    .with_annotations(&method.annotations)
}

fn class_to_entry(clazz: &PClass, parent_id: i32, base_path: &str) -> SearchIndexEntry {
    SearchIndexEntry {
        name: clazz.simple_name().to_string(),
        kind: Kind::Class,
        url: format!("{base_path}/{}.html", encode_path(clazz.simple_name())),
        sig: None,
        par_id: Some(parent_id),
        deprecated: None,
        aka: None,
    }
    .with_annotations(&clazz.annotations)
}

fn type_alias_to_entry(alias: &TypeAlias, parent_id: i32, base_path: &str) -> SearchIndexEntry {
    SearchIndexEntry {
        name: alias.simple_name().to_string(),
        kind: Kind::TypeAlias,
        url: format!("{base_path}#{}", encode_path(alias.simple_name())),
        sig: None,
        par_id: Some(parent_id),
        deprecated: None,
        aka: None,
    }
    .with_annotations(&alias.annotations)
}

fn render_method_signature(store: &SchemaStore, method: &PClassMethod) -> String {
    let mut out = String::new();
    out.push('(');
    for (idx, (name, _)) in method.parameters.iter().enumerate() {
        if idx > 0 {
            out.push_str(", ");
        }
        out.push_str(name);
    }
    out.push_str("): ");
    append_type(store, &method.return_type, &mut out);
    out
}

fn render_property_signature(store: &SchemaStore, property: &PClassProperty) -> String {
    let mut out = String::from(": ");
    append_type(store, &property.ty, &mut out);
    out
}

fn append_type(store: &SchemaStore, ty: &PType, out: &mut String) {
    match ty {
        PType::Unknown => out.push_str("unknown"),
        PType::Nothing => out.push_str("nothing"),
        PType::Module => out.push_str("module"),
        // Exact upstream quirk (`SearchIndexGenerator.kt:347`): appends `\"lit\"` so JSON encodes `\\\"lit\\\"`.
        PType::StringLiteral(lit) => {
            out.push_str("\\\"");
            out.push_str(lit);
            out.push_str("\\\"");
        }
        PType::Class {
            class_id,
            type_arguments,
            ..
        } => {
            let pclass = store.class(*class_id);
            let name = if pclass.is_module_class() {
                match pclass.module_name().rfind('.') {
                    Some(idx) => &pclass.module_name()[idx + 1..],
                    None => pclass.module_name(),
                }
            } else {
                pclass.simple_name()
            };
            out.push_str(name);
            if !type_arguments.is_empty() {
                out.push('<');
                for (idx, arg) in type_arguments.iter().enumerate() {
                    if idx > 0 {
                        out.push_str(", ");
                    }
                    append_type(store, arg, out);
                }
                out.push('>');
            }
        }
        PType::Constrained { base_type, .. } => append_type(store, base_type, out),
        PType::Union(elems) => {
            for (idx, elem) in elems.iter().enumerate() {
                if idx > 0 {
                    out.push('|');
                }
                append_type(store, elem, out);
            }
        }
        PType::Nullable(inner) => {
            append_type(store, inner, out);
            out.push('?');
        }
        PType::Function {
            parameter_types,
            return_type,
        } => {
            if parameter_types.len() == 1 {
                append_type(store, &parameter_types[0], out);
            } else {
                out.push('(');
                for (idx, pt) in parameter_types.iter().enumerate() {
                    if idx > 0 {
                        out.push_str(", ");
                    }
                    append_type(store, pt, out);
                }
                out.push(')');
            }
            out.push_str(" -> ");
            append_type(store, return_type, out);
        }
        PType::Alias { alias_id, .. } => {
            out.push_str(store.type_alias(*alias_id).simple_name());
        }
        PType::TypeVariable(tp) => {
            out.push_str(&tp.name);
        }
    }
}
