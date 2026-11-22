//! Exact port of `RuntimeData.kt` and `RuntimeDataGenerator.kt`.

use crate::json::{JsonValue, parse_json, push_json_string};
use crate::model::DocGeneratorError;
use crate::package_data::{
    ClassData, ElementRef, ModuleData, PackageData, PackageId, PackageRef, TypeId, TypeRef,
};
use crate::util::compare_versions_desc;
use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct RuntimeDataLink {
    pub text: String,
    pub href: Option<String>,
}

/// Exact port of `RuntimeData` (`RuntimeData.kt:33`).
/// Order of links in each vector matches Kotlin's `LinkedHashSet` iteration order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RuntimeData {
    pub known_versions: Vec<RuntimeDataLink>,
    pub known_usages: Vec<RuntimeDataLink>,
    pub known_subtypes: Vec<RuntimeDataLink>,
}

impl RuntimeData {
    pub fn read_or_empty(path: &Path) -> Result<Self, DocGeneratorError> {
        let content = match fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(e) => {
                return Err(DocGeneratorError::bug(format!(
                    "I/O error reading `{}`: {e}",
                    path.display()
                )));
            }
        };
        let value = parse_json(&content).map_err(|e| {
            DocGeneratorError::bug(format!("Error deserializing `{}`: {e}", path.display()))
        })?;
        Self::from_json(&value).map_err(|e| {
            DocGeneratorError::bug(format!("Error deserializing `{}`: {e}", path.display()))
        })
    }

    pub fn from_json(v: &JsonValue) -> Result<Self, DocGeneratorError> {
        let known_versions = parse_link_set(v.get("knownVersions"))?;
        let known_usages = parse_link_set(v.get("knownUsages"))?;
        let known_subtypes = parse_link_set(v.get("knownSubtypes"))?;
        Ok(Self {
            known_versions,
            known_usages,
            known_subtypes,
        })
    }

    /// Formats matching `kotlinx.serialization.json` with `prettyPrint = true`,
    /// `explicitNulls = false`, `encodeDefaults = false`, `prettyPrintIndent = "  "`,
    /// and NO trailing newline.
    pub fn to_pretty_json(&self) -> String {
        let mut sections: Vec<(&str, &[RuntimeDataLink])> = Vec::new();
        if !self.known_versions.is_empty() {
            sections.push(("knownVersions", &self.known_versions));
        }
        if !self.known_usages.is_empty() {
            sections.push(("knownUsages", &self.known_usages));
        }
        if !self.known_subtypes.is_empty() {
            sections.push(("knownSubtypes", &self.known_subtypes));
        }
        if sections.is_empty() {
            return "{}".to_string();
        }
        let mut out = String::from("{\n");
        for (sec_idx, (name, links)) in sections.iter().enumerate() {
            if sec_idx > 0 {
                out.push_str(",\n");
            }
            out.push_str("  \"");
            out.push_str(name);
            out.push_str("\": [\n");
            for (link_idx, link) in links.iter().enumerate() {
                if link_idx > 0 {
                    out.push_str(",\n");
                }
                out.push_str("    {\n");
                out.push_str("      \"text\": ");
                push_json_string(&mut out, &link.text);
                if let Some(ref href) = link.href {
                    out.push_str(",\n      \"href\": ");
                    push_json_string(&mut out, href);
                }
                out.push_str("\n    }");
            }
            out.push_str("\n  ]");
        }
        out.push_str("\n}");
        out
    }

    pub fn write_to(&self, path: &Path) -> Result<(), DocGeneratorError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                DocGeneratorError::bug(format!("I/O error writing `{}`: {e}", path.display()))
            })?;
        }
        fs::write(path, self.to_pretty_json().as_bytes()).map_err(|e| {
            DocGeneratorError::bug(format!("I/O error writing `{}`: {e}", path.display()))
        })
    }

    pub fn per_package(&self) -> Self {
        Self {
            known_versions: self.known_versions.clone(),
            known_usages: Vec::new(),
            known_subtypes: Vec::new(),
        }
    }

    pub fn per_package_version(&self) -> Self {
        Self {
            known_versions: Vec::new(),
            known_usages: self.known_usages.clone(),
            known_subtypes: self.known_subtypes.clone(),
        }
    }

    pub fn add_known_versions<T, F>(
        &self,
        my_ref: &T,
        versions: Option<&[String]>,
        comparator: &F,
    ) -> Self
    where
        T: ElementRef,
        F: Fn(&str, &str) -> Ordering,
    {
        let Some(versions) = versions else {
            return self.clone();
        };
        let mut effective: Vec<String> = Vec::new();
        let mut seen: HashSet<String> = HashSet::new();
        for link in &self.known_versions {
            if seen.insert(link.text.clone()) {
                effective.push(link.text.clone());
            }
        }
        for v in versions {
            if seen.insert(v.clone()) {
                effective.push(v.clone());
            }
        }
        effective.sort_by(|a, b| comparator(a, b));
        let mut new_links: Vec<RuntimeDataLink> = Vec::with_capacity(effective.len());
        let mut seen_links: HashSet<RuntimeDataLink> = HashSet::new();
        for version in effective {
            let link = RuntimeDataLink {
                href: Some(my_ref.page_url_for_version(&version)),
                text: version,
            };
            if seen_links.insert(link.clone()) {
                new_links.push(link);
            }
        }
        if new_links == self.known_versions {
            return self.clone();
        }
        let mut copy = self.clone();
        copy.known_versions = new_links;
        copy
    }

    pub fn add_known_usages<T, TextFn, F>(
        &self,
        my_ref: &T,
        refs: Option<&[T]>,
        text_fn: TextFn,
        comparator: &F,
    ) -> Self
    where
        T: ElementRef,
        TextFn: Fn(&T) -> String,
        F: Fn(&str, &str) -> Ordering,
    {
        let Some(refs) = refs else {
            return self.clone();
        };
        let mut combined = self.known_usages.clone();
        let mut seen: HashSet<RuntimeDataLink> = combined.iter().cloned().collect();
        for r in refs {
            let link = RuntimeDataLink {
                text: text_fn(r),
                href: Some(r.page_url_relative_to(my_ref)),
            };
            if seen.insert(link.clone()) {
                combined.push(link);
            }
        }
        let mut deduped = distinct_by_comparator(&combined, comparator);
        deduped.sort_by(|a, b| a.text.cmp(&b.text));
        if deduped == self.known_usages {
            return self.clone();
        }
        let mut copy = self.clone();
        copy.known_usages = deduped;
        copy
    }

    pub fn add_known_subtypes<F>(
        &self,
        my_ref: &TypeRef,
        subtypes: Option<&[TypeRef]>,
        comparator: &F,
    ) -> Self
    where
        F: Fn(&str, &str) -> Ordering,
    {
        let Some(subtypes) = subtypes else {
            return self.clone();
        };
        let mut combined = self.known_subtypes.clone();
        let mut seen: HashSet<RuntimeDataLink> = combined.iter().cloned().collect();
        for r in subtypes {
            let link = RuntimeDataLink {
                text: r.display_name().to_string(),
                href: Some(r.page_url_relative_to(my_ref)),
            };
            if seen.insert(link.clone()) {
                combined.push(link);
            }
        }
        let mut deduped = distinct_by_comparator(&combined, comparator);
        deduped.sort_by(|a, b| a.text.cmp(&b.text));
        if deduped == self.known_subtypes {
            return self.clone();
        }
        let mut copy = self.clone();
        copy.known_subtypes = deduped;
        copy
    }
}

/// Mirrors `RuntimeData.distinctByComparator` (`RuntimeData.kt:138-150`).
pub fn distinct_by_comparator<F>(links: &[RuntimeDataLink], comparator: &F) -> Vec<RuntimeDataLink>
where
    F: Fn(&str, &str) -> Ordering,
{
    let mut order: Vec<String> = Vec::new();
    let mut highest: BTreeMap<String, RuntimeDataLink> = BTreeMap::new();
    for link in links {
        match highest.get(&link.text) {
            None => {
                order.push(link.text.clone());
                highest.insert(link.text.clone(), link.clone());
            }
            Some(prev) => {
                if compare_link_segments(prev, link, comparator) == Ordering::Greater {
                    highest.insert(link.text.clone(), link.clone());
                }
            }
        }
    }
    order
        .into_iter()
        .filter_map(|k| highest.remove(&k))
        .collect()
}

fn compare_link_segments<F>(o1: &RuntimeDataLink, o2: &RuntimeDataLink, comparator: &F) -> Ordering
where
    F: Fn(&str, &str) -> Ordering,
{
    let h1 = o1.href.as_deref().unwrap_or("");
    let h2 = o2.href.as_deref().unwrap_or("");
    for (p1, p2) in h1.split('/').zip(h2.split('/')) {
        if p1 == p2 {
            continue;
        }
        // Only compare segments that are parseable semantic versions, mirroring
        // Kotlin's `try { comparator.compare(path1, path2) } catch (_: Throwable) { continue }`.
        if crate::util::parse_semver_or_none(p1).is_none()
            || crate::util::parse_semver_or_none(p2).is_none()
        {
            continue;
        }
        let ord = comparator(p1, p2);
        if ord != Ordering::Equal {
            return ord;
        }
    }
    Ordering::Equal
}

fn parse_link_set(value: Option<&JsonValue>) -> Result<Vec<RuntimeDataLink>, DocGeneratorError> {
    let Some(v) = value else {
        return Ok(Vec::new());
    };
    if matches!(v, JsonValue::Null) {
        return Ok(Vec::new());
    }
    let Some(items) = v.as_array() else {
        return Err(DocGeneratorError::bug(
            "Expected array for runtime data links",
        ));
    };
    let mut out = Vec::with_capacity(items.len());
    let mut seen: HashSet<RuntimeDataLink> = HashSet::new();
    for item in items {
        let text = match item.get("text") {
            Some(JsonValue::String(s)) => s.clone(),
            _ => {
                return Err(DocGeneratorError::bug(
                    "Expected string `text` in RuntimeDataLink",
                ));
            }
        };
        let href = match item.get("href") {
            None | Some(JsonValue::Null) => None,
            Some(JsonValue::String(s)) => Some(s.clone()),
            Some(_) => {
                return Err(DocGeneratorError::bug(
                    "Expected string or null `href` in RuntimeDataLink",
                ));
            }
        };
        let link = RuntimeDataLink { text, href };
        if seen.insert(link.clone()) {
            out.push(link);
        }
    }
    Ok(out)
}

/// Exact port of `RuntimeDataGenerator` (`RuntimeDataGenerator.kt`).
pub struct RuntimeDataGenerator<'a> {
    output_dir: &'a Path,
    console_out: &'a mut dyn Write,
    state: RuntimeDataState,
    pub written_files: HashSet<PathBuf>,
}

pub(crate) struct RuntimeDataState {
    package_versions: Vec<(PackageId, Vec<PackageRef>)>,
    class_versions: BTreeMap<TypeId, Vec<String>>,
    package_usages: Vec<(PackageRef, Vec<PackageRef>)>,
    type_usages: Vec<(TypeRef, Vec<TypeRef>)>,
    subtypes: Vec<(TypeRef, Vec<TypeRef>)>,
}

impl RuntimeDataState {
    pub(crate) fn collect(packages: &[PackageData]) -> Self {
        let mut state = Self {
            package_versions: Vec::new(),
            class_versions: BTreeMap::new(),
            package_usages: Vec::new(),
            type_usages: Vec::new(),
            subtypes: Vec::new(),
        };
        for pkg in packages {
            push_map_set(
                &mut state.package_versions,
                &pkg.r#ref.pkg,
                pkg.r#ref.clone(),
            );
            for dep in &pkg.dependencies {
                if dep.r#ref.pkg == "pkl" {
                    continue;
                }
                push_map_set(&mut state.package_usages, &dep.r#ref, pkg.r#ref.clone());
            }
            for module in &pkg.modules {
                for clazz in effective_classes(module) {
                    state.collect_class_data(clazz);
                }
            }
        }
        state
    }

    fn collect_class_data(&mut self, clazz: &ClassData) {
        let versions = self.class_versions.entry(clazz.r#ref.id()).or_default();
        if !versions.contains(&clazz.r#ref.version) {
            versions.push(clazz.r#ref.version.clone());
        }
        for superclass in &clazz.superclasses {
            if superclass.is_in_same_package_as(&clazz.r#ref) {
                push_map_set(&mut self.subtypes, superclass, clazz.r#ref.clone());
            }
        }
        for ty in &clazz.used_types {
            if ty.is_in_same_package_as(&clazz.r#ref) {
                push_map_set(&mut self.type_usages, ty, clazz.r#ref.clone());
            }
        }
    }

    pub(crate) fn collect_writes(
        &self,
        output_dir: &Path,
        packages: &[PackageData],
        data_dir_existed: bool,
        writes: &mut Vec<crate::generator::PendingFile>,
        logs: &mut Vec<String>,
        written_files: &mut HashSet<PathBuf>,
    ) -> Result<(), DocGeneratorError> {
        let mut in_memory: HashMap<PathBuf, RuntimeData> = HashMap::new();
        let mut path_slot: HashMap<PathBuf, usize> = HashMap::new();

        for pkg in packages {
            let pkg_ref = &pkg.r#ref;
            let path = output_dir.join(pkg_ref.per_package_version_runtime_data_path());
            let existing = read_existing(&path, data_dir_existed, &in_memory)?;
            let usages = find_map_set(&self.package_usages, pkg_ref);
            let data = existing
                .add_known_usages(pkg_ref, usages, |r| r.pkg.clone(), &compare_versions_desc)
                .per_package_version();
            push_runtime_write(
                path,
                data,
                &mut in_memory,
                &mut path_slot,
                writes,
                logs,
                written_files,
            );
            for module in &pkg.modules {
                for clazz in effective_classes(module) {
                    let class_ref = &clazz.r#ref;
                    let path = output_dir.join(class_ref.per_package_version_runtime_data_path());
                    let existing = read_existing(&path, data_dir_existed, &in_memory)?;
                    let new_subtypes = find_map_set(&self.subtypes, class_ref);
                    let new_usages = find_map_set(&self.type_usages, class_ref);
                    let data = existing
                        .add_known_subtypes(class_ref, new_subtypes, &compare_versions_desc)
                        .add_known_usages(
                            class_ref,
                            new_usages,
                            |r| r.display_name().to_string(),
                            &compare_versions_desc,
                        )
                        .per_package_version();
                    push_runtime_write(
                        path,
                        data,
                        &mut in_memory,
                        &mut path_slot,
                        writes,
                        logs,
                        written_files,
                    );
                }
            }
        }

        let mut seen_pkgs: HashSet<&str> = HashSet::new();
        for pkg in packages {
            if !seen_pkgs.insert(pkg.r#ref.pkg.as_str()) {
                continue;
            }
            let pkg_ref = &pkg.r#ref;
            let path = output_dir.join(pkg_ref.per_package_runtime_data_path());
            let existing = read_existing(&path, data_dir_existed, &in_memory)?;
            let new_versions: Option<Vec<String>> =
                find_map_set(&self.package_versions, &pkg_ref.pkg).map(|refs| {
                    let mut out = Vec::new();
                    for r in refs {
                        if !out.contains(&r.version) {
                            out.push(r.version.clone());
                        }
                    }
                    out
                });
            let data = existing
                .add_known_versions(pkg_ref, new_versions.as_deref(), &compare_versions_desc)
                .per_package();
            push_runtime_write(
                path,
                data,
                &mut in_memory,
                &mut path_slot,
                writes,
                logs,
                written_files,
            );
            for module in &pkg.modules {
                for clazz in effective_classes(module) {
                    let class_ref = &clazz.r#ref;
                    let path = output_dir.join(class_ref.per_package_runtime_data_path());
                    let existing = read_existing(&path, data_dir_existed, &in_memory)?;
                    let new_versions = self
                        .class_versions
                        .get(&class_ref.id())
                        .map(|v| v.as_slice());
                    let data = existing
                        .add_known_versions(class_ref, new_versions, &compare_versions_desc)
                        .per_package();
                    push_runtime_write(
                        path,
                        data,
                        &mut in_memory,
                        &mut path_slot,
                        writes,
                        logs,
                        written_files,
                    );
                }
            }
        }
        Ok(())
    }

    pub(crate) fn update_known_usages(
        &self,
        output_dir: &Path,
        packages: &[PackageData],
        console_out: &mut dyn Write,
    ) -> Result<(), DocGeneratorError> {
        let newly_written: HashSet<&PackageRef> = packages.iter().map(|p| &p.r#ref).collect();
        for (pkg_ref, usages) in &self.package_usages {
            if newly_written.contains(pkg_ref) {
                continue;
            }
            let path = output_dir.join(pkg_ref.per_package_version_runtime_data_path());
            if !path.is_file() {
                continue;
            }
            let existing = RuntimeData::read_or_empty(&path)?;
            let updated = existing.add_known_usages(
                pkg_ref,
                Some(usages),
                |r| r.pkg.clone(),
                &compare_versions_desc,
            );
            if updated != existing {
                updated.write_to(&path)?;
                let _ = write!(console_out, "Wrote file {}\r", path_to_uri_display(&path));
            }
        }
        Ok(())
    }
}

fn read_existing(
    path: &Path,
    data_dir_existed: bool,
    in_memory: &HashMap<PathBuf, RuntimeData>,
) -> Result<RuntimeData, DocGeneratorError> {
    if let Some(prev) = in_memory.get(path) {
        Ok(prev.clone())
    } else if data_dir_existed {
        RuntimeData::read_or_empty(path)
    } else {
        Ok(RuntimeData::default())
    }
}

fn push_runtime_write(
    path: PathBuf,
    data: RuntimeData,
    in_memory: &mut HashMap<PathBuf, RuntimeData>,
    path_slot: &mut HashMap<PathBuf, usize>,
    writes: &mut Vec<crate::generator::PendingFile>,
    logs: &mut Vec<String>,
    written_files: &mut HashSet<PathBuf>,
) {
    let bytes = data.to_pretty_json().into_bytes();
    logs.push(format!("Wrote file {}\r", path_to_uri_display(&path)));
    in_memory.insert(path.clone(), data);
    written_files.insert(path.clone());
    if let Some(&slot) = path_slot.get(&path) {
        writes[slot].bytes = crate::generator::FileBytes::Owned(bytes);
    } else {
        let slot = writes.len();
        path_slot.insert(path.clone(), slot);
        writes.push(crate::generator::PendingFile {
            path,
            bytes: crate::generator::FileBytes::Owned(bytes),
        });
    }
}

impl<'a> RuntimeDataGenerator<'a> {
    pub fn new(output_dir: &'a Path, console_out: &'a mut dyn Write) -> Self {
        Self {
            output_dir,
            console_out,
            state: RuntimeDataState {
                package_versions: Vec::new(),
                class_versions: BTreeMap::new(),
                package_usages: Vec::new(),
                type_usages: Vec::new(),
                subtypes: Vec::new(),
            },
            written_files: HashSet::new(),
        }
    }

    pub fn generate(&mut self, packages: &[PackageData]) -> Result<(), DocGeneratorError> {
        self.state = RuntimeDataState::collect(packages);
        let data_dir_existed = self.output_dir.join("data").exists();
        let mut writes = Vec::new();
        let mut logs = Vec::new();
        self.state.collect_writes(
            self.output_dir,
            packages,
            data_dir_existed,
            &mut writes,
            &mut logs,
            &mut self.written_files,
        )?;
        crate::generator::write_files_parallel(&writes)?;
        for line in &logs {
            let _ = self.console_out.write_all(line.as_bytes());
        }
        if data_dir_existed {
            self.state
                .update_known_usages(self.output_dir, packages, self.console_out)?;
        }
        Ok(())
    }
}

fn effective_classes(module: &ModuleData) -> Vec<&ClassData> {
    let mut out =
        Vec::with_capacity(module.classes.len() + usize::from(module.module_class.is_some()));
    if let Some(ref mc) = module.module_class {
        out.push(mc);
    }
    out.extend(module.classes.iter());
    out
}

fn push_map_set<K: PartialEq + Clone, V: PartialEq>(map: &mut Vec<(K, Vec<V>)>, key: &K, value: V) {
    if let Some((_, list)) = map.iter_mut().find(|(k, _)| k == key) {
        if !list.contains(&value) {
            list.push(value);
        }
    } else {
        map.push((key.clone(), vec![value]));
    }
}

fn find_map_set<'a, K: PartialEq, V>(map: &'a [(K, Vec<V>)], key: &K) -> Option<&'a [V]> {
    map.iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.as_slice())
}

pub fn path_to_uri_display(path: &Path) -> String {
    pkl_host_native::path_to_file_uri(path).to_java_utf8()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_to_uri_display_matches_java_path_to_uri() {
        assert_eq!(
            path_to_uri_display(Path::new(
                "/out/data/com.package1/1.2.3/Module Containing Spaces/index.json"
            )),
            "file:///out/data/com.package1/1.2.3/Module%20Containing%20Spaces/index.json"
        );
        assert_eq!(
            path_to_uri_display(Path::new(
                "/out/data/com.package2/4.5.6/Module3/Class Two {}.json"
            )),
            "file:///out/data/com.package2/4.5.6/Module3/Class%20Two%20%7B%7D.json"
        );
        assert_eq!(
            path_to_uri_display(Path::new("/out/data/localhost(3a)0/birds/0.5.0/index.json")),
            "file:///out/data/localhost(3a)0/birds/0.5.0/index.json"
        );
    }
}
