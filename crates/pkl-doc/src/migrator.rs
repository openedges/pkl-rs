//! Exact port of `DocMigrator.kt`: migrates a Pkldoc v1 website to v2.

use crate::assets::copy_resource;
use crate::json::{JsonValue, parse_json};
use crate::model::DocGeneratorError;
use crate::package_data::{ElementRef, ModuleRef, PackageData, PackageRef, TypeRef};
use crate::runtime_data::{RuntimeData, RuntimeDataLink};
use crate::util::{
    compare_versions_desc, encode_path, ensure_path_under_output_dir, join_confined_output_path,
    push_safe_inline_script_json_string, relativize_uri,
};
use std::collections::HashSet;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

pub const CURRENT_VERSION: i32 = 2;

const LEGACY_KNOWN_VERSIONS_PREFIX: &str = "runtimeData.links('known-versions','";
const LEGACY_KNOWN_USAGES_PREFIX: &str = "runtimeData.links('known-usages','";
const LEGACY_KNOWN_SUBTYPES_PREFIX: &str = "runtimeData.links('known-subtypes','";
const LEGACY_SUFFIX: &str = "');";

#[derive(Clone)]
enum AnyRef {
    Package(PackageRef),
    Module(ModuleRef),
    Type(TypeRef),
}

impl AnyRef {
    fn as_element_ref(&self) -> &dyn ElementRef {
        match self {
            Self::Package(r) => r,
            Self::Module(r) => r,
            Self::Type(r) => r,
        }
    }

    fn with_version(&self, version: &str) -> Self {
        match self {
            Self::Package(r) => {
                let mut c = r.clone();
                c.version = version.to_string();
                Self::Package(c)
            }
            Self::Module(r) => {
                let mut c = r.clone();
                c.version = version.to_string();
                Self::Module(c)
            }
            Self::Type(r) => {
                let mut c = r.clone();
                c.version = version.to_string();
                Self::Type(c)
            }
        }
    }
}

pub struct DocMigrator<'a> {
    output_dir: PathBuf,
    console_out: &'a mut dyn Write,
    migrated_current_packages: HashSet<String>,
}

impl<'a> DocMigrator<'a> {
    pub fn new(output_dir: PathBuf, console_out: &'a mut dyn Write) -> Self {
        Self {
            output_dir,
            console_out,
            migrated_current_packages: HashSet::new(),
        }
    }

    fn version_path(&self) -> PathBuf {
        self.output_dir.join(".pkldoc/VERSION")
    }

    pub fn docsite_version(&self) -> i32 {
        let vp = self.version_path();
        if !vp.is_file() {
            return 1;
        }
        pkl_host_native::read_path_to_string_bounded(&vp)
            .ok()
            .and_then(|s| s.trim().parse::<i32>().ok())
            .unwrap_or(1)
    }

    pub fn is_up_to_date(&self) -> bool {
        if !self.output_dir.join("index.html").exists() {
            return true;
        }
        self.docsite_version() == CURRENT_VERSION
    }

    pub fn update_docsite_version(&self) -> Result<(), DocGeneratorError> {
        let vp = join_confined_output_path(&self.output_dir, ".pkldoc/VERSION")?;
        if let Some(parent) = vp.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                DocGeneratorError::bug(format!("I/O error writing `{}`: {e}", vp.display()))
            })?;
        }
        let checked = ensure_path_under_output_dir(&self.output_dir, &vp)?;
        pkl_host_native::write_file_nofollow(&checked, CURRENT_VERSION.to_string().as_bytes())
            .map_err(|e| {
                DocGeneratorError::bug(format!("I/O error writing `{}`: {e}", vp.display()))
            })
    }

    pub fn run(&mut self) -> Result<(), DocGeneratorError> {
        if self.is_up_to_date() {
            let _ = writeln!(
                self.console_out,
                "Generated pkldoc is already version {CURRENT_VERSION}; there's nothing to do."
            );
            return Ok(());
        }

        let mut package_datas = Vec::new();
        collect_package_data_files(&self.output_dir, &mut package_datas)?;

        // Match upstream `DocMigrator.kt:149` (`val count = AtomicInteger(1)`).
        let mut count = 1usize;
        for path in package_datas {
            let pkg_data = PackageData::read(&path)?;
            let is_current_version = path
                .parent()
                .and_then(|p| p.file_name())
                .and_then(|n| n.to_str())
                == Some("current");
            if !is_current_version {
                self.migrate_runtime_data(&pkg_data)?;
            }
            self.migrate_html(&pkg_data, is_current_version)?;
            if !is_current_version {
                self.delete_legacy_runtime_data(&pkg_data)?;
            }
            count += 1;
            let _ = write!(
                self.console_out,
                "Migrated {count} packages ({}@{})\r",
                pkg_data.r#ref.pkg, pkg_data.r#ref.version
            );
            let _ = self.console_out.flush();
        }

        copy_resource("scripts/pkldoc.js", &self.output_dir)
            .map_err(|e| DocGeneratorError::user(e.to_string()))?;
        self.update_docsite_version()?;
        let _ = writeln!(
            self.console_out,
            "Finished migration, migrated {count} packages."
        );
        Ok(())
    }

    fn migrate_html(
        &self,
        package_data: &PackageData,
        is_current_version: bool,
    ) -> Result<(), DocGeneratorError> {
        for r in package_data_refs(package_data) {
            let effective = if is_current_version {
                r.with_version("current")
            } else {
                r.clone()
            };
            let html_path = join_confined_output_path(
                &self.output_dir,
                &effective.as_element_ref().html_path(),
            )?;
            do_migrate_html(r.as_element_ref(), &html_path)?;
        }
        Ok(())
    }

    fn get_latest_package_data(&self, pkg: &str) -> Result<PackageData, DocGeneratorError> {
        let pkg_dir = join_confined_output_path(&self.output_dir, &encode_path(pkg))?;
        let current_pkg = ensure_path_under_output_dir(
            &self.output_dir,
            &pkg_dir.join("current/package-data.json"),
        )?;
        if current_pkg.is_file() {
            return PackageData::read(&current_pkg);
        }
        let mut versions: Vec<String> = fs::read_dir(&pkg_dir)
            .map_err(|e| {
                DocGeneratorError::bug(format!("I/O error listing `{}`: {e}", pkg_dir.display()))
            })?
            .filter_map(|entry| {
                entry
                    .ok()
                    .and_then(|e| e.file_name().to_str().map(str::to_string))
            })
            .collect();
        versions.sort_by(|a, b| compare_versions_desc(a, b));
        let latest = versions.first().ok_or_else(|| {
            DocGeneratorError::bug(format!("No versions found under `{}`", pkg_dir.display()))
        })?;
        let latest_path = ensure_path_under_output_dir(
            &self.output_dir,
            &pkg_dir.join(latest).join("package-data.json"),
        )?;
        PackageData::read(&latest_path)
    }

    fn migrate_runtime_data(
        &mut self,
        package_data: &PackageData,
    ) -> Result<(), DocGeneratorError> {
        if !self
            .migrated_current_packages
            .contains(&package_data.r#ref.pkg)
        {
            let current_pkg_data = self.get_latest_package_data(&package_data.r#ref.pkg)?;
            for r in package_data_refs(&current_pkg_data) {
                self.do_migrate_per_package_data(r.as_element_ref())?;
            }
            self.migrated_current_packages
                .insert(package_data.r#ref.pkg.clone());
        }
        for r in package_data_refs(package_data) {
            self.do_migrate_per_package_version_data(r.as_element_ref())?;
        }
        Ok(())
    }

    fn load_legacy_runtime_data(
        &self,
        r: &dyn ElementRef,
    ) -> Result<Option<RuntimeData>, DocGeneratorError> {
        let legacy_path =
            join_confined_output_path(&self.output_dir, &r.legacy_versioned_runtime_data_path())?;
        if !legacy_path.exists() {
            return Ok(None);
        }
        let my_version_href = r.page_url_for_version(r.version());
        parse_legacy_runtime_data(&legacy_path, &my_version_href).map(Some)
    }

    fn do_migrate_per_package_data(&self, r: &dyn ElementRef) -> Result<(), DocGeneratorError> {
        let Some(data) = self.load_legacy_runtime_data(r)? else {
            return Ok(());
        };
        let target =
            join_confined_output_path(&self.output_dir, &r.per_package_runtime_data_path())?;
        data.per_package().write_to(&self.output_dir, &target)
    }

    fn do_migrate_per_package_version_data(
        &self,
        r: &dyn ElementRef,
    ) -> Result<(), DocGeneratorError> {
        let Some(data) = self.load_legacy_runtime_data(r)? else {
            return Ok(());
        };
        let target = join_confined_output_path(
            &self.output_dir,
            &r.per_package_version_runtime_data_path(),
        )?;
        data.per_package_version()
            .write_to(&self.output_dir, &target)
    }

    fn delete_legacy_runtime_data(
        &self,
        package_data: &PackageData,
    ) -> Result<(), DocGeneratorError> {
        for r in package_data_refs(package_data) {
            let legacy_path = join_confined_output_path(
                &self.output_dir,
                &r.as_element_ref().legacy_versioned_runtime_data_path(),
            )?;
            if legacy_path.exists() {
                fs::remove_file(&legacy_path).map_err(|e| {
                    DocGeneratorError::bug(format!(
                        "I/O error deleting `{}`: {e}",
                        legacy_path.display()
                    ))
                })?;
            }
        }
        Ok(())
    }
}

/// Exact port of `DocMigrator.parseLegacyRuntimeData` (`DocMigrator.kt:66-101`).
pub fn parse_legacy_runtime_data(
    path: &Path,
    my_version_href: &str,
) -> Result<RuntimeData, DocGeneratorError> {
    let content = pkl_host_native::read_path_to_string_bounded(path).map_err(|e| {
        DocGeneratorError::bug(format!("I/O error reading `{}`: {e}", path.display()))
    })?;
    let mut data = RuntimeData::default();
    for line in content.lines() {
        if line.starts_with(LEGACY_KNOWN_VERSIONS_PREFIX) {
            let links = read_legacy_line(line, LEGACY_KNOWN_VERSIONS_PREFIX, LEGACY_SUFFIX)?;
            data.known_versions = links
                .into_iter()
                .map(|mut link| {
                    if link.href.is_none() {
                        link.href = Some(my_version_href.to_string());
                    }
                    link
                })
                .collect();
        } else if line.starts_with(LEGACY_KNOWN_USAGES_PREFIX) {
            data.known_usages = read_legacy_line(line, LEGACY_KNOWN_USAGES_PREFIX, LEGACY_SUFFIX)?;
        } else if line.starts_with(LEGACY_KNOWN_SUBTYPES_PREFIX) {
            data.known_subtypes =
                read_legacy_line(line, LEGACY_KNOWN_SUBTYPES_PREFIX, LEGACY_SUFFIX)?;
        } else {
            return Err(DocGeneratorError::bug(format!(
                "Unexpected runtimeData line: {line}"
            )));
        }
    }
    Ok(data)
}

fn read_legacy_line(
    line: &str,
    prefix: &str,
    suffix: &str,
) -> Result<Vec<RuntimeDataLink>, DocGeneratorError> {
    let Some(json_str) = line
        .strip_prefix(prefix)
        .and_then(|rest| rest.strip_suffix(suffix))
    else {
        return Err(DocGeneratorError::bug(format!(
            "Malformed legacy runtimeData line: {line}"
        )));
    };
    let val = parse_json(json_str)?;
    let arr = val
        .as_array()
        .ok_or_else(|| DocGeneratorError::bug("Expected JSON array in legacy runtimeData"))?;
    let mut out = Vec::with_capacity(arr.len());
    let mut seen: HashSet<RuntimeDataLink> = HashSet::new();
    for item in arr {
        let text = match item.get("text") {
            Some(JsonValue::String(s)) => s.clone(),
            _ => {
                return Err(DocGeneratorError::bug(
                    "Expected string `text` in legacy RuntimeDataLink",
                ));
            }
        };
        let href = match item.get("href") {
            None | Some(JsonValue::Null) => None,
            Some(JsonValue::String(s)) => Some(s.clone()),
            _ => {
                return Err(DocGeneratorError::bug(
                    "Expected string/null `href` in legacy RuntimeDataLink",
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

fn do_migrate_html(r: &dyn ElementRef, path: &Path) -> Result<(), DocGeneratorError> {
    let content = pkl_host_native::read_path_to_string_bounded(path).map_err(|e| {
        DocGeneratorError::bug(format!("I/O error reading `{}`: {e}", path.display()))
    })?;
    let lines: Vec<&str> = content.lines().collect();
    let legacy_data_path = r.legacy_versioned_runtime_data_path();
    let mut new_html = String::with_capacity(content.len() + 512);

    for (idx, &line) in lines.iter().enumerate() {
        if line.contains(&legacy_data_path) {
            continue;
        }
        new_html.push_str(line);
        new_html.push('\n');

        if line.contains("scripts/pkldoc.js")
            && let Some(next_line) = lines.get(idx + 1)
            && !next_line.contains("<script type=\"module\"")
        {
            let html_path = r.html_path();
            let package_version_url =
                relativize_uri(&r.per_package_runtime_data_path(), &html_path);
            let per_package_version_url =
                relativize_uri(&r.per_package_version_runtime_data_path(), &html_path);
            let mut pkg_url_js = String::new();
            push_safe_inline_script_json_string(&mut pkg_url_js, &package_version_url);
            let mut per_pkg_ver_url_js = String::new();
            push_safe_inline_script_json_string(&mut per_pkg_ver_url_js, &per_package_version_url);
            let mut version_json = String::new();
            push_safe_inline_script_json_string(&mut version_json, r.version());

            new_html.push_str("    <script type=\"module\">\n");
            new_html.push_str(&format!(
                "      import perPackageData from {pkg_url_js} with {{ type: \"json\" }}\n"
            ));
            new_html.push_str(&format!(
                "      import perPackageVersionData from {per_pkg_ver_url_js} with {{ type: \"json\" }}\n"
            ));
            new_html.push('\n');
            new_html.push_str(&format!(
                "      runtimeData.knownVersions(perPackageData.knownVersions, {version_json});\n"
            ));
            new_html.push_str(
                "      runtimeData.knownUsagesOrSubtypes(\"known-subtypes\", perPackageVersionData.knownSubtypes);\n",
            );
            new_html.push_str(
                "      runtimeData.knownUsagesOrSubtypes(\"known-usages\", perPackageVersionData.knownUsages);\n",
            );
            new_html.push_str("    </script>\n");
        }
    }

    pkl_host_native::write_file_nofollow(path, new_html.as_bytes())
        .map_err(|e| DocGeneratorError::bug(format!("I/O error writing `{}`: {e}", path.display())))
}

fn package_data_refs(pkg: &PackageData) -> Vec<AnyRef> {
    let mut refs = Vec::new();
    refs.push(AnyRef::Package(pkg.r#ref.clone()));
    for module in &pkg.modules {
        refs.push(AnyRef::Module(module.r#ref.clone()));
        for clazz in &module.classes {
            refs.push(AnyRef::Type(clazz.r#ref.clone()));
        }
    }
    refs
}

/// Non-symlink-following recursive walk matching `Files.walk(outputDir)` for `package-data.json`.
pub fn collect_package_data_files(
    dir: &Path,
    out: &mut Vec<PathBuf>,
) -> Result<(), DocGeneratorError> {
    if !dir.exists() {
        return Ok(());
    }
    let mut entries: Vec<PathBuf> = fs::read_dir(dir)
        .map_err(|e| DocGeneratorError::bug(format!("I/O error listing `{}`: {e}", dir.display())))?
        .filter_map(|e| e.ok().map(|ent| ent.path()))
        .collect();
    entries.sort();
    for path in entries {
        let meta = fs::symlink_metadata(&path).map_err(|e| {
            DocGeneratorError::bug(format!("I/O error stat `{}`: {e}", path.display()))
        })?;
        if pkl_host_native::is_symlink_or_junction(&path, &meta) {
            continue;
        }
        if meta.is_dir() {
            collect_package_data_files(&path, out)?;
        } else if meta.is_file()
            && path.file_name().and_then(|n| n.to_str()) == Some("package-data.json")
        {
            out.push(path);
        }
    }
    Ok(())
}

#[cfg(test)]
mod security_tests {
    use super::*;

    #[test]
    fn read_legacy_line_handles_overlapping_prefix_suffix_without_panic() {
        let short = "runtimeData.links('known-versions','');";
        // Valid empty string inside quotes fails JSON parse cleanly (no slice panic).
        assert!(read_legacy_line(short, LEGACY_KNOWN_VERSIONS_PREFIX, LEGACY_SUFFIX).is_err());
        // Even shorter overlapping prefix+suffix (`prefix = "a'"`, `suffix = "'a"`, line = `"a'a"`).
        assert!(read_legacy_line("a'a", "a'", "'a").is_err());
    }
}
