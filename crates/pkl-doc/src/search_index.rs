//! Exact port of `SearchIndexGenerator.kt`.

use crate::json::{JsonValue, parse_json, push_json_string};
use crate::model::{
    DocGeneratorError, DocModule, DocPackage, annotations_also_known_as, annotations_is_unlisted,
    is_base_annotation,
};
use crate::package_data::{ElementRef, ModuleData, PackageData};
use crate::runtime_data::path_to_uri_display;
use crate::util::{encode_path, join_confined_output_path};

/// Serializes `s` as a JSON string literal safe inside `searchData='[...]';`
/// (escapes `'` -> `\u0027`, U+2028 -> `\u2028`, U+2029 -> `\u2029` so untrusted
/// symbols/annotations cannot break out of the JS single-quoted string literal).
fn push_search_index_json_string(out: &mut String, s: &str) {
    if !s.contains(['\'', '\u{2028}', '\u{2029}']) {
        push_json_string(out, s);
        return;
    }
    let mut tmp = String::with_capacity(s.len() + 8);
    push_json_string(&mut tmp, s);
    for ch in tmp.chars() {
        match ch {
            '\'' => out.push_str("\\u0027"),
            '\u{2028}' => out.push_str("\\u2028"),
            '\u{2029}' => out.push_str("\\u2029"),
            other => out.push(other),
        }
    }
}
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
        push_search_index_json_string(out, &self.name);
        out.push_str(",\"kind\":");
        out.push_str(&(self.kind as i32).to_string());
        out.push_str(",\"url\":");
        push_search_index_json_string(out, &self.url);
        if let Some(ref sig) = self.sig {
            out.push_str(",\"sig\":");
            push_search_index_json_string(out, sig);
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
                push_search_index_json_string(out, item);
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
        let text =
            pkl_host_native::read_path_to_string_bounded(&self.search_index_file).map_err(|e| {
                DocGeneratorError::bug(format!(
                    "I/O error reading `{}`: {e}",
                    self.search_index_file.display()
                ))
            })?;
        let Some(json_str) = text
            .strip_prefix(PREFIX)
            .and_then(|rest| rest.strip_suffix(POSTFIX))
        else {
            let _ = writeln!(
                self.console_out,
                "[error] Incorrect existing search-index.js; either doesnt start with prefix '{PREFIX}', or end with postfix '{POSTFIX}'"
            );
            return Ok(Vec::new());
        };
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
        let entries = build_site_entries(current_packages);
        self.write_entries(&self.search_index_file.clone(), &entries)
    }

    pub(crate) fn collect_site_index_write(
        output_dir: &Path,
        current_packages: &[PackageData],
        writes: &mut Vec<crate::generator::PendingFile>,
        logs: &mut Vec<String>,
    ) {
        let path = output_dir.join("search-index.js");
        let entries = build_site_entries(current_packages);
        let bytes = format_entries_js(&entries);
        logs.push(format!("Wrote file {}\r", path_to_uri_display(&path)));
        writes.push(crate::generator::PendingFile {
            path,
            bytes: crate::generator::FileBytes::Owned(bytes),
        });
    }

    pub(crate) fn collect_package_write(
        output_dir: &Path,
        doc_package: &DocPackage,
        writes: &mut Vec<crate::generator::PendingFile>,
        logs: &mut Vec<String>,
    ) -> Result<(), DocGeneratorError> {
        let rel = format!(
            "{}/{}/search-index.js",
            encode_path(doc_package.name()),
            doc_package.version()
        );
        let path = join_confined_output_path(output_dir, &rel)?;
        let entries = build_package_entries(doc_package);
        let bytes = format_entries_js(&entries);
        logs.push(format!("Wrote file {}\r", path_to_uri_display(&path)));
        writes.push(crate::generator::PendingFile {
            path,
            bytes: crate::generator::FileBytes::Owned(bytes),
        });
        Ok(())
    }

    pub fn generate(&mut self, doc_package: &DocPackage) -> Result<(), DocGeneratorError> {
        let rel = format!(
            "{}/{}/search-index.js",
            encode_path(doc_package.name()),
            doc_package.version()
        );
        let path = join_confined_output_path(self.output_dir, &rel)?;
        let entries = build_package_entries(doc_package);
        self.write_entries(&path, &entries)
    }

    fn write_entries(
        &mut self,
        path: &Path,
        entries: &[SearchIndexEntry],
    ) -> Result<(), DocGeneratorError> {
        let checked = crate::util::ensure_path_under_output_dir(self.output_dir, path)?;
        if let Some(parent) = checked.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                DocGeneratorError::bug(format!("I/O error writing `{}`: {e}", checked.display()))
            })?;
        }
        let checked = crate::util::ensure_path_under_output_dir(self.output_dir, &checked)?;
        let bytes = format_entries_js(entries);
        pkl_host_native::write_file_nofollow(&checked, &bytes).map_err(|e| {
            DocGeneratorError::bug(format!("I/O error writing `{}`: {e}", checked.display()))
        })?;
        let _ = write!(
            self.console_out,
            "Wrote file {}\r",
            path_to_uri_display(&checked)
        );
        Ok(())
    }
}

fn build_site_entries(current_packages: &[PackageData]) -> Vec<SearchIndexEntry> {
    let search_index = SearchIndexGenerator::build_search_index(current_packages);
    let mut entries = Vec::new();
    for pkg_idx in search_index {
        entries.push(pkg_idx.package_entry);
        entries.extend(pkg_idx.module_entries);
    }
    entries
}

fn build_package_entries(doc_package: &DocPackage) -> Vec<SearchIndexEntry> {
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
    entries
}

fn format_entries_js(entries: &[SearchIndexEntry]) -> Vec<u8> {
    let mut out = String::with_capacity(PREFIX.len() + entries.len() * 64 + POSTFIX.len() + 2);
    out.push_str(PREFIX);
    out.push('[');
    for (idx, entry) in entries.iter().enumerate() {
        if idx > 0 {
            out.push(',');
        }
        entry.write_json(&mut out);
    }
    out.push(']');
    out.push_str(POSTFIX);
    out.into_bytes()
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
        _ => {}
    }
}

#[cfg(test)]
mod security_tests {
    use super::*;

    #[test]
    fn search_index_escapes_single_quotes_and_handles_short_corrupt_index() {
        let entry = SearchIndexEntry {
            name: "foo';alert(1);//".to_string(),
            kind: Kind::Property,
            url: "a/index.html#x".to_string(),
            sig: Some(": String".to_string()),
            par_id: Some(0),
            deprecated: None,
            aka: None,
        };
        let bytes = format_entries_js(&[entry]);
        let js = String::from_utf8(bytes).unwrap();
        assert!(js.starts_with(PREFIX) && js.ends_with(POSTFIX));
        let inner = js
            .strip_prefix(PREFIX)
            .and_then(|s| s.strip_suffix(POSTFIX))
            .unwrap();
        assert!(
            !inner.contains('\''),
            "unescaped single quote in searchData: {js}"
        );
        assert!(inner.contains("\\u0027"));

        // Short overlapping `"searchData=';\n"` must not panic when reading existing index.
        let dir = std::env::temp_dir().join(format!("pkl-doc-si-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("search-index.js"), "searchData=';\n").unwrap();
        let mut out = Vec::new();
        let mut generator = SearchIndexGenerator::new(&dir, &mut out);
        let parsed = generator.get_current_search_index().unwrap();
        assert!(parsed.is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn json_and_markdown_enforce_recursion_depth_limits() {
        let deep_json = format!("{}{}", "[".repeat(256), "]".repeat(256));
        assert!(parse_json(&deep_json).is_err());

        let deep_md = format!("{}hello", "> ".repeat(256));
        let rendered = crate::markdown::render_markdown_trimmed(&deep_md, |label| {
            crate::markdown::RefLinkResolution {
                destination: String::new(),
                label: label.to_string(),
                code_only_if_short: true,
            }
        });
        assert!(rendered.is_some());
    }
}
