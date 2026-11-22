//! Top-level documentation generator orchestrator matching Apple Pkl 0.32.0 `DocGenerator.kt`.

use crate::migrator::{self, DocMigrator};
use crate::model::{DocGeneratorError, DocPackage, DocPackageInfo, DocsiteInfo};
use crate::package_data::PackageData;
use crate::pages::HtmlGenerator;
use crate::runtime_data::{RuntimeDataState, path_to_uri_display};
use crate::search_index::{PackageIndexEntry, SearchIndexGenerator};
use crate::util::{compare_versions_desc, encode_path, parse_semver_or_none};
use pkl_runtime::schema::ModuleSchema;
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};

pub const CURRENT_DIRECTORY_NAME: &str = "current";

pub(crate) enum FileBytes {
    Owned(Vec<u8>),
    Static(&'static [u8]),
}

impl FileBytes {
    fn as_slice(&self) -> &[u8] {
        match self {
            Self::Owned(v) => v.as_slice(),
            Self::Static(s) => s,
        }
    }
}

pub(crate) struct PendingFile {
    pub path: PathBuf,
    pub bytes: FileBytes,
}

pub(crate) fn write_files_parallel(files: &[PendingFile]) -> Result<(), DocGeneratorError> {
    if files.is_empty() {
        return Ok(());
    }
    // Deduplicate parent directories and skip ancestors of deeper directories so
    // `create_dir_all` is invoked only once per leaf directory.
    let mut dirs: Vec<&Path> = files
        .iter()
        .filter_map(|f| f.path.parent())
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    dirs.sort_by_key(|a| std::cmp::Reverse(a.as_os_str().len()));
    let mut covered: HashSet<&Path> = HashSet::new();
    for dir in dirs {
        if covered.contains(dir) {
            continue;
        }
        fs::create_dir_all(dir).map_err(|e| {
            DocGeneratorError::bug(format!("I/O error writing `{}`: {e}", dir.display()))
        })?;
        let mut cur = Some(dir);
        while let Some(d) = cur {
            if !covered.insert(d) {
                break;
            }
            cur = d.parent();
        }
    }

    if files.len() < 2 {
        for f in files {
            fs::write(&f.path, f.bytes.as_slice()).map_err(|e| {
                DocGeneratorError::bug(format!("I/O error writing `{}`: {e}", f.path.display()))
            })?;
        }
        return Ok(());
    }

    let next = AtomicUsize::new(0);
    let first_err: Mutex<Option<DocGeneratorError>> = Mutex::new(None);
    let workers = std::thread::available_parallelism()
        .map(|n| n.get().min(8).min(files.len()))
        .unwrap_or_else(|_| 8.min(files.len()));
    std::thread::scope(|scope| {
        for _ in 0..workers {
            scope.spawn(|| {
                loop {
                    let idx = next.fetch_add(1, AtomicOrdering::Relaxed);
                    if idx >= files.len() {
                        break;
                    }
                    let f = &files[idx];
                    if let Err(e) = fs::write(&f.path, f.bytes.as_slice()) {
                        let mut guard = first_err.lock().unwrap_or_else(|x| x.into_inner());
                        if guard.is_none() {
                            *guard = Some(DocGeneratorError::bug(format!(
                                "I/O error writing `{}`: {e}",
                                f.path.display()
                            )));
                        }
                        break;
                    }
                }
            });
        }
    });
    if let Some(err) = first_err.into_inner().unwrap_or_else(|x| x.into_inner()) {
        return Err(err);
    }
    Ok(())
}

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
        let output_dir_non_empty = fs::read_dir(&self.output_dir)
            .ok()
            .and_then(|mut it| it.next())
            .is_some();

        if output_dir_non_empty {
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

        let pkg_data_in_doc_order: Vec<PackageData> = doc_packages
            .iter()
            .map(PackageData::from_doc_package)
            .collect();
        let mut newly_generated_packages = pkg_data_in_doc_order.clone();
        newly_generated_packages.sort_by(|a, b| a.r#ref.pkg.cmp(&b.r#ref.pkg));

        let current_search_index = if output_dir_non_empty {
            let mut sig = SearchIndexGenerator::new(&self.output_dir, console_out);
            sig.get_current_search_index()?
        } else {
            Vec::new()
        };

        let _ = writeln!(console_out, "Loaded current search index");

        let existing_current_packages = if current_search_index.is_empty() {
            Vec::new()
        } else {
            self.get_current_packages(&current_search_index, console_out)?
        };

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
            doc_packages,
            self.imported_modules.clone(),
            &self.output_dir,
            self.is_test_mode,
            is_single_package_site,
        );

        let data_dir_existed = output_dir_non_empty && self.output_dir.join("data").exists();
        if output_dir_non_empty {
            for doc_package in html_generator.doc_packages() {
                let pkg_dir = self.output_dir.join(encode_path(&format!(
                    "{}/{}",
                    doc_package.name(),
                    doc_package.version()
                )));
                delete_path_recursively(&pkg_dir)?;
            }
        }

        let mut pkg_writes: Vec<PendingFile> = Vec::with_capacity(64);
        let mut pkg_logs: Vec<String> = Vec::new();
        for (pkg_idx, doc_package) in html_generator.doc_packages().iter().enumerate() {
            SearchIndexGenerator::collect_package_write(
                &self.output_dir,
                doc_package,
                &mut pkg_writes,
                &mut pkg_logs,
            );
            html_generator.collect_package_writes(pkg_idx, &mut pkg_writes, &mut pkg_logs)?;
            let pkg_data_path = self
                .output_dir
                .join(encode_path(doc_package.name()))
                .join(doc_package.version())
                .join("package-data.json");
            pkg_writes.push(PendingFile {
                path: pkg_data_path,
                bytes: FileBytes::Owned(
                    pkg_data_in_doc_order[pkg_idx].to_json_string().into_bytes(),
                ),
            });
        }

        let mut site_logs: Vec<String> = Vec::new();
        let mut runtime_logs: Vec<String> = Vec::new();
        let rdg_state = RuntimeDataState::collect(&newly_generated_packages);
        let mut rdg_written = HashSet::new();

        if self.no_symlinks {
            // `--no-symlinks` copies `<pkg>/<ver>` into `<pkg>/current`, so flush package
            // files before copying, then flush all remaining files in a second parallel batch.
            write_files_parallel(&pkg_writes)?;
            for line in &pkg_logs {
                let _ = console_out.write_all(line.as_bytes());
            }
            let _ = writeln!(console_out, "Generated HTML for packages");
            self.create_current_directories(&current_packages, &existing_current_packages)?;

            let mut rest_writes: Vec<PendingFile> = Vec::with_capacity(48);
            SearchIndexGenerator::collect_site_index_write(
                &self.output_dir,
                &current_packages,
                &mut rest_writes,
                &mut site_logs,
            );
            html_generator.collect_site_page_write(
                &current_packages,
                &mut rest_writes,
                &mut site_logs,
            )?;
            for &(rel, bytes) in crate::assets::STATIC_RESOURCES {
                rest_writes.push(PendingFile {
                    path: self.output_dir.join(rel),
                    bytes: FileBytes::Static(bytes),
                });
            }
            rdg_state.collect_writes(
                &self.output_dir,
                &newly_generated_packages,
                data_dir_existed,
                &mut rest_writes,
                &mut runtime_logs,
                &mut rdg_written,
            )?;
            rest_writes.push(PendingFile {
                path: self.output_dir.join(".pkldoc/VERSION"),
                bytes: FileBytes::Owned(migrator::CURRENT_VERSION.to_string().into_bytes()),
            });
            write_files_parallel(&rest_writes)?;
            for line in &site_logs {
                let _ = console_out.write_all(line.as_bytes());
            }
            for line in &runtime_logs {
                let _ = console_out.write_all(line.as_bytes());
            }
            if data_dir_existed {
                rdg_state.update_known_usages(
                    &self.output_dir,
                    &newly_generated_packages,
                    console_out,
                )?;
            }
            let _ = writeln!(console_out, "Wrote package runtime data files");
        } else {
            // All package, site, static asset, runtime data, and version files are disjoint:
            // flush everything in one parallel batch and emit console progress in exact order.
            let mut writes = pkg_writes;
            SearchIndexGenerator::collect_site_index_write(
                &self.output_dir,
                &current_packages,
                &mut writes,
                &mut site_logs,
            );
            html_generator.collect_site_page_write(
                &current_packages,
                &mut writes,
                &mut site_logs,
            )?;
            for &(rel, bytes) in crate::assets::STATIC_RESOURCES {
                writes.push(PendingFile {
                    path: self.output_dir.join(rel),
                    bytes: FileBytes::Static(bytes),
                });
            }
            rdg_state.collect_writes(
                &self.output_dir,
                &newly_generated_packages,
                data_dir_existed,
                &mut writes,
                &mut runtime_logs,
                &mut rdg_written,
            )?;
            writes.push(PendingFile {
                path: self.output_dir.join(".pkldoc/VERSION"),
                bytes: FileBytes::Owned(migrator::CURRENT_VERSION.to_string().into_bytes()),
            });
            write_files_parallel(&writes)?;
            self.create_current_directories(&current_packages, &existing_current_packages)?;

            for line in &pkg_logs {
                let _ = console_out.write_all(line.as_bytes());
            }
            let _ = writeln!(console_out, "Generated HTML for packages");
            for line in &site_logs {
                let _ = console_out.write_all(line.as_bytes());
            }
            for line in &runtime_logs {
                let _ = console_out.write_all(line.as_bytes());
            }
            if data_dir_existed {
                rdg_state.update_known_usages(
                    &self.output_dir,
                    &newly_generated_packages,
                    console_out,
                )?;
            }
            let _ = writeln!(console_out, "Wrote package runtime data files");
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_files_parallel_creates_nested_dirs_and_writes_all_payloads() {
        let dir = std::env::temp_dir().join(format!("pkl-doc-par-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let files = vec![
            PendingFile {
                path: dir.join("index.html"),
                bytes: FileBytes::Owned(b"root".to_vec()),
            },
            PendingFile {
                path: dir.join("a/b/c/leaf.html"),
                bytes: FileBytes::Owned(b"leaf".to_vec()),
            },
            PendingFile {
                path: dir.join("a/b/mid.html"),
                bytes: FileBytes::Owned(b"mid".to_vec()),
            },
            PendingFile {
                path: dir.join("assets/static.txt"),
                bytes: FileBytes::Static(b"static-bytes"),
            },
        ];
        write_files_parallel(&files).unwrap();
        assert_eq!(fs::read(dir.join("index.html")).unwrap(), b"root");
        assert_eq!(fs::read(dir.join("a/b/c/leaf.html")).unwrap(), b"leaf");
        assert_eq!(fs::read(dir.join("a/b/mid.html")).unwrap(), b"mid");
        assert_eq!(
            fs::read(dir.join("assets/static.txt")).unwrap(),
            b"static-bytes"
        );
        let _ = fs::remove_dir_all(&dir);
    }
}
