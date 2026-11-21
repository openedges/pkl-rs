//! Top-level documentation generator orchestrator matching Apple Pkl 0.32.0 `DocGenerator.kt`.

use crate::migrator::{self, DocMigrator};
use crate::model::{DocGeneratorError, DocPackage, DocPackageInfo, DocsiteInfo};
use crate::package_data::{PackageData, generate_package_data};
use crate::pages::HtmlGenerator;
use crate::runtime_data::{RuntimeDataGenerator, path_to_uri_display};
use crate::search_index::{PackageIndexEntry, SearchIndexGenerator};
use crate::util::{compare_versions_desc, encode_path, parse_semver_or_none};
use pkl_runtime::schema::ModuleSchema;
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

pub const CURRENT_DIRECTORY_NAME: &str = "current";

/// Entry point for the low-level Pkldoc API (`org.pkl.doc.DocGenerator`).
pub struct DocGenerator {
    pub docsite_info: DocsiteInfo,
    pub packages: Vec<(DocPackageInfo, Vec<ModuleSchema>)>,
    pub imported_modules: BTreeMap<String, ModuleSchema>,
    pub output_dir: PathBuf,
    pub is_test_mode: bool,
    pub no_symlinks: bool,
}

impl DocGenerator {
    pub fn new(
        docsite_info: DocsiteInfo,
        packages: Vec<(DocPackageInfo, Vec<ModuleSchema>)>,
        imported_modules: BTreeMap<String, ModuleSchema>,
        output_dir: PathBuf,
        is_test_mode: bool,
        no_symlinks: bool,
    ) -> Self {
        Self {
            docsite_info,
            packages,
            imported_modules,
            output_dir,
            is_test_mode,
            no_symlinks,
        }
    }

    /// Determines the latest non-prerelease version per package (`DocGenerator.kt:97-110`).
    pub fn determine_current_packages(packages: &[PackageData]) -> Vec<PackageData> {
        let mut filtered: Vec<PackageData> = packages
            .iter()
            .filter(|p| {
                parse_semver_or_none(&p.r#ref.version).is_none_or(|v| v.pre_release.is_none())
            })
            .cloned()
            .collect();
        filtered.sort_by(|a, b| {
            a.r#ref
                .pkg
                .cmp(&b.r#ref.pkg)
                .then_with(|| compare_versions_desc(&a.r#ref.version, &b.r#ref.version))
        });
        let mut seen = HashSet::new();
        filtered.retain(|p| seen.insert(p.r#ref.pkg.clone()));
        filtered
    }

    /// Runs the documentation generator writing progress output to `console_out`.
    pub fn run(&self, console_out: &mut dyn Write) -> Result<(), DocGeneratorError> {
        {
            let migrator = DocMigrator::new(self.output_dir.clone(), console_out);
            if !migrator.is_up_to_date() {
                return Err(DocGeneratorError::user(format!(
                    "Docsite is not up to date. Expected: {}. Found: {}. Use DocMigrator to migrate the site.",
                    migrator::CURRENT_VERSION,
                    migrator.docsite_version()
                )));
            }
        }

        let doc_packages: Vec<DocPackage> = self
            .packages
            .iter()
            .map(|(info, schemas)| DocPackage::new(info.clone(), schemas.clone()))
            .filter(|pkg| !pkg.is_unlisted)
            .collect();

        let mut newly_generated_packages: Vec<PackageData> = doc_packages
            .iter()
            .map(PackageData::from_doc_package)
            .collect();
        newly_generated_packages.sort_by(|a, b| a.r#ref.pkg.cmp(&b.r#ref.pkg));

        let current_search_index = {
            let mut sig = SearchIndexGenerator::new(&self.output_dir, console_out);
            sig.get_current_search_index()?
        };

        let _ = writeln!(console_out, "Loaded current search index");

        let existing_current_packages =
            self.get_current_packages(&current_search_index, console_out)?;

        let _ = writeln!(console_out, "Fetched latest packages");

        let mut all_candidates =
            Vec::with_capacity(newly_generated_packages.len() + existing_current_packages.len());
        all_candidates.extend(newly_generated_packages.iter().cloned());
        all_candidates.extend(existing_current_packages.iter().cloned());
        let current_packages = Self::determine_current_packages(&all_candidates);

        let is_single_package_site =
            self.docsite_info.overview.is_none() && current_packages.len() == 1;

        let html_generator = HtmlGenerator::new(
            &self.docsite_info,
            doc_packages.clone(),
            self.imported_modules.clone(),
            &self.output_dir,
            self.is_test_mode,
            is_single_package_site,
        );

        for (pkg_idx, doc_package) in doc_packages.iter().enumerate() {
            let pkg_dir = self.output_dir.join(encode_path(&format!(
                "{}/{}",
                doc_package.name(),
                doc_package.version()
            )));
            delete_path_recursively(&pkg_dir)?;

            {
                let mut sig = SearchIndexGenerator::new(&self.output_dir, console_out);
                sig.generate(doc_package)?;
            }
            html_generator.generate_package(pkg_idx, console_out)?;
            generate_package_data(&self.output_dir, doc_package)?;
        }

        let _ = writeln!(console_out, "Generated HTML for packages");

        self.create_current_directories(&current_packages, &existing_current_packages)?;

        {
            let mut sig = SearchIndexGenerator::new(&self.output_dir, console_out);
            sig.generate_site_index(&current_packages)?;
        }
        html_generator.generate_site(&current_packages, console_out)?;
        {
            let mut rdg = RuntimeDataGenerator::new(&self.output_dir, console_out);
            rdg.generate(&newly_generated_packages)?;
        }

        let _ = writeln!(console_out, "Wrote package runtime data files");

        {
            let migrator = DocMigrator::new(self.output_dir.clone(), console_out);
            migrator.update_docsite_version()?;
        }

        Ok(())
    }

    fn try_load_package_data(
        &self,
        entry: &PackageIndexEntry,
        console_out: &mut dyn Write,
    ) -> Result<Option<PackageData>, DocGeneratorError> {
        let mut package_data_file = self
            .output_dir
            .join(&entry.package_entry.url)
            .with_file_name("package-data.json");
        if !package_data_file.exists() {
            // search-index.js in Pkl 0.29 and below did not encode path.
            package_data_file = self
                .output_dir
                .join(encode_path(&entry.package_entry.url))
                .with_file_name("package-data.json");
            if !package_data_file.exists() {
                let _ = writeln!(
                    console_out,
                    "[Warn] likely corrupted search index; missing {}. This entry will be removed in the newly generated index.",
                    package_data_file.display()
                );
                return Ok(None);
            }
        }
        let _ = write!(
            console_out,
            "Loading package data for {}\r",
            path_to_uri_display(&package_data_file)
        );
        PackageData::read(&package_data_file).map(Some)
    }

    fn get_current_packages(
        &self,
        site_search_index: &[PackageIndexEntry],
        console_out: &mut dyn Write,
    ) -> Result<Vec<PackageData>, DocGeneratorError> {
        let mut result = Vec::new();
        for entry in site_search_index {
            if let Some(pkg_data) = self.try_load_package_data(entry, console_out)? {
                result.push(pkg_data);
            }
        }
        Ok(result)
    }

    fn create_current_directories(
        &self,
        current_packages: &[PackageData],
        existing_current_packages: &[PackageData],
    ) -> Result<(), DocGeneratorError> {
        for package_data in current_packages {
            if existing_current_packages.contains(package_data) {
                continue;
            }
            let base_path = self.output_dir.join(encode_path(&package_data.r#ref.pkg));
            let src = base_path.join(&package_data.r#ref.version);
            let dst = base_path.join(CURRENT_DIRECTORY_NAME);

            if self.no_symlinks {
                delete_path_recursively(&dst)?;
                copy_dir_recursively(&src, &dst)?;
            } else if !dst.exists() || !is_same_path(&dst, &src) {
                delete_path_recursively(&dst)?;
                create_directory_symlink(Path::new(&package_data.r#ref.version), &dst)?;
            }
        }
        Ok(())
    }
}

fn is_same_path(a: &Path, b: &Path) -> bool {
    match (fs::canonicalize(a), fs::canonicalize(b)) {
        (Ok(ca), Ok(cb)) => ca == cb,
        _ => false,
    }
}

pub(crate) fn delete_path_recursively(path: &Path) -> Result<(), DocGeneratorError> {
    let meta = match fs::symlink_metadata(path) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => {
            return Err(DocGeneratorError::bug(format!(
                "I/O error generating documentation: {e}"
            )));
        }
    };
    if meta.file_type().is_symlink() || meta.is_file() {
        fs::remove_file(path)
    } else {
        fs::remove_dir_all(path)
    }
    .map_err(|e| DocGeneratorError::bug(format!("I/O error generating documentation: {e}")))
}

fn copy_dir_recursively(src: &Path, dst: &Path) -> Result<(), DocGeneratorError> {
    fs::create_dir_all(dst)
        .map_err(|e| DocGeneratorError::bug(format!("I/O error generating documentation: {e}")))?;
    let mut entries: Vec<PathBuf> = fs::read_dir(src)
        .map_err(|e| DocGeneratorError::bug(format!("I/O error generating documentation: {e}")))?
        .filter_map(|e| e.ok().map(|ent| ent.path()))
        .collect();
    entries.sort();
    for entry_path in entries {
        let Some(name) = entry_path.file_name() else {
            continue;
        };
        let target = dst.join(name);
        let meta = fs::symlink_metadata(&entry_path).map_err(|e| {
            DocGeneratorError::bug(format!("I/O error generating documentation: {e}"))
        })?;
        if meta.is_dir() {
            copy_dir_recursively(&entry_path, &target)?;
        } else if meta.is_file() {
            fs::copy(&entry_path, &target).map_err(|e| {
                DocGeneratorError::bug(format!("I/O error generating documentation: {e}"))
            })?;
        }
    }
    Ok(())
}

fn create_directory_symlink(target: &Path, link: &Path) -> Result<(), DocGeneratorError> {
    if let Some(parent) = link.parent() {
        fs::create_dir_all(parent).map_err(|e| {
            DocGeneratorError::bug(format!("I/O error generating documentation: {e}"))
        })?;
    }
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(target, link)
            .map_err(|e| DocGeneratorError::bug(format!("I/O error generating documentation: {e}")))
    }
    #[cfg(windows)]
    {
        std::os::windows::fs::symlink_dir(target, link)
            .map_err(|e| DocGeneratorError::bug(format!("I/O error generating documentation: {e}")))
    }
}
