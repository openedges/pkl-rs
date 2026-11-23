use pkl_doc::package_data::{PackageData, PackageRef};
use pkl_doc::{DocGenerator, PKLDOC_HELP_TEXT, run_cli_with_io_and_ansi_level};
use pkl_runtime::diagnostic::mordant::AnsiLevel;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(1);

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(prefix: &str) -> Self {
        let id = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("pkl-doc-test-{prefix}-{}-{id}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("create temp test dir");
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn run(args: &[&str]) -> (i32, String, String) {
    run_with_level(args, AnsiLevel::None)
}

fn run_with_level(args: &[&str], level: AnsiLevel) -> (i32, String, String) {
    let owned: Vec<String> = args.iter().map(|s| (*s).to_string()).collect();
    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = run_cli_with_io_and_ansi_level(&owned, &mut out, &mut err, level);
    (
        code,
        String::from_utf8(out).expect("utf8 stdout"),
        String::from_utf8(err).expect("utf8 stderr"),
    )
}

fn copy_tree_raw(src: &Path, dst: &Path) {
    fs::create_dir_all(dst).expect("mkdir dst");
    let mut entries: Vec<PathBuf> = fs::read_dir(src)
        .expect("read_dir")
        .map(|e| e.expect("dir entry").path())
        .collect();
    entries.sort();
    for entry in entries {
        let name = entry.file_name().expect("file name");
        let target = dst.join(name);
        let meta = fs::symlink_metadata(&entry).expect("lstat");
        if meta.file_type().is_symlink() {
            let link_target = fs::read_link(&entry).expect("read_link");
            #[cfg(unix)]
            std::os::unix::fs::symlink(&link_target, &target).expect("symlink");
        } else if meta.is_dir() {
            copy_tree_raw(&entry, &target);
        } else if meta.is_file() {
            fs::copy(&entry, &target).expect("copy file");
        }
    }
}

fn collect_tree_raw(root: &Path) -> BTreeMap<String, Vec<u8>> {
    fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
        let mut entries: Vec<PathBuf> = fs::read_dir(dir)
            .expect("read_dir")
            .map(|e| e.expect("dir entry").path())
            .collect();
        entries.sort();
        for entry in entries {
            let rel = entry
                .strip_prefix(root)
                .expect("strip_prefix")
                .to_string_lossy()
                .replace('\\', "/");
            let meta = fs::symlink_metadata(&entry).expect("lstat");
            if meta.file_type().is_symlink() {
                let target = fs::read_link(&entry).expect("read_link");
                out.insert(
                    format!("symlink/{rel}"),
                    target.to_string_lossy().into_owned().into_bytes(),
                );
            } else if meta.is_dir() {
                walk(root, &entry, out);
            } else if meta.is_file() {
                out.insert(format!("file/{rel}"), fs::read(&entry).expect("read file"));
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(root, root, &mut out);
    out
}

#[test]
fn version_outputs_honest_native_banner() {
    for flag in ["--version", "-v"] {
        let (code, stdout, stderr) = run(&[flag]);
        assert_eq!(code, 0);
        assert!(stderr.is_empty(), "unexpected stderr: {stderr:?}");
        assert!(
            stdout.starts_with("pkldoc-rs 0.32.0 (") && stdout.ends_with(", native)\n"),
            "unexpected version banner: {stdout:?}"
        );
    }
}

#[test]
fn help_matches_pinned_text_verbatim() {
    for flag in ["--help", "-h"] {
        let (code, stdout, stderr) = run(&[flag]);
        assert_eq!(code, 0);
        assert!(stderr.is_empty());
        assert_eq!(stdout, PKLDOC_HELP_TEXT);
    }
}

#[test]
fn cli_validation_errors_match_upstream_messages() {
    let tmp = TempDir::new("validation");
    let out_dir = tmp.path().join("out");
    let out_str = out_dir.to_str().unwrap();

    // Missing --output-dir
    let (code, _, stderr) = run(&["foo.pkl"]);
    assert_eq!(code, 1);
    assert!(stderr.contains("missing option --output-dir"), "{stderr}");

    // Package URI with fragment
    let (code, stdout, stderr) = run(&[
        "--output-dir",
        out_str,
        "--test-mode",
        "package://localhost:0/birds@0.5.0#/Bird.pkl",
    ]);
    assert_eq!(code, 1);
    assert!(stdout.is_empty());
    assert_eq!(
        stderr,
        "Cannot generate documentation for just one module within a package\n"
    );

    // Missing doc-package-info.pkl
    let alpha = tmp.path().join("Alpha.pkl");
    fs::write(&alpha, "module com.example.Alpha\n").unwrap();
    let (code, stdout, stderr) = run(&[
        "--output-dir",
        out_str,
        "--test-mode",
        alpha.to_str().unwrap(),
    ]);
    assert_eq!(code, 1);
    assert!(stdout.is_empty());
    assert_eq!(
        stderr,
        "`sourceModules` must contain at least one module named `doc-package-info.pkl`, or an argument must be a package URI.\n"
    );

    // Multiple docsite-info.pkl
    let d1 = tmp.path().join("d1/docsite-info.pkl");
    let d2 = tmp.path().join("d2/docsite-info.pkl");
    fs::create_dir_all(d1.parent().unwrap()).unwrap();
    fs::create_dir_all(d2.parent().unwrap()).unwrap();
    fs::write(&d1, "amends \"pkl:DocsiteInfo\"\n").unwrap();
    fs::write(&d2, "amends \"pkl:DocsiteInfo\"\n").unwrap();
    let (code, stdout, stderr) = run(&[
        "--output-dir",
        out_str,
        "--test-mode",
        d2.to_str().unwrap(),
        d1.to_str().unwrap(),
    ]);
    assert_eq!(code, 1);
    assert!(stdout.is_empty());
    let lines: Vec<&str> = stderr.lines().collect();
    assert_eq!(lines.len(), 3, "{stderr}");
    assert_eq!(
        lines[0],
        "`sourceModules` contains multiple modules named `docsite-info.pkl`:"
    );
    assert!(lines[1].ends_with("/d1/docsite-info.pkl"), "{stderr}");
    assert!(lines[2].ends_with("/d2/docsite-info.pkl"), "{stderr}");

    // Legacy v1 docsite without --migrate
    fs::create_dir_all(&out_dir).unwrap();
    fs::write(out_dir.join("index.html"), "<html></html>").unwrap();
    let (code, stdout, stderr) = run(&[
        "--output-dir",
        out_str,
        "--test-mode",
        alpha.to_str().unwrap(),
    ]);
    assert_eq!(code, 1);
    assert!(stdout.is_empty());
    assert_eq!(
        stderr,
        "pkldoc website model is too old (found: 1, required: 2). Run `pkldoc --migrate` to migrate the website.\n"
    );
}

#[test]
fn migrate_v1_matches_upstream_doc_migrator_output_byte_for_byte() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let input_dir = manifest_dir
        .join("../../external/pkl/pkl-doc/src/test/files/DocMigratorTest/input/version-1");
    let expected_dir =
        manifest_dir.join("../../external/pkl/pkl-doc/src/test/files/DocMigratorTest/output");

    let tmp = TempDir::new("migrate-v1");
    copy_tree_raw(&input_dir, tmp.path());

    let (code, stdout, stderr) = run(&["--migrate", "--output-dir", tmp.path().to_str().unwrap()]);
    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(stderr.is_empty(), "stderr: {stderr}");
    assert!(
        stdout.ends_with("Finished migration, migrated 11 packages.\n"),
        "unexpected stdout: {stdout:?}"
    );

    let expected_tree = collect_tree_raw(&expected_dir);
    let actual_tree = collect_tree_raw(tmp.path());
    assert_eq!(actual_tree.len(), expected_tree.len());
    for (k, exp) in &expected_tree {
        let act = actual_tree
            .get(k)
            .unwrap_or_else(|| panic!("missing migrated path: {k}"));
        assert_eq!(act, exp, "byte mismatch for migrated path {k}");
    }
}

#[test]
fn determine_current_packages_prefers_highest_stable_semver() {
    let make_pkg = |name: &str, ver: &str| PackageData {
        r#ref: PackageRef {
            pkg: name.to_string(),
            pkg_uri: None,
            version: ver.to_string(),
        },
        summary: None,
        deprecation: None,
        source_code: None,
        source_code_url_scheme: None,
        dependencies: Vec::new(),
        modules: Vec::new(),
    };
    let pkgs = vec![
        make_pkg("pkg.b", "0.5.0"),
        make_pkg("pkg.b", "0.7.0"),
        make_pkg("pkg.b", "0.6.0"),
        make_pkg("pkg.b", "1.0.0-SNAPSHOT"),
        make_pkg("pkg.a", "2.0.0-alpha.1"),
        make_pkg("pkg.a", "1.2.3"),
    ];
    let current = DocGenerator::determine_current_packages(&pkgs);
    assert_eq!(current.len(), 2);
    assert_eq!(current[0].r#ref.pkg, "pkg.a");
    assert_eq!(current[0].r#ref.version, "1.2.3");
    assert_eq!(current[1].r#ref.pkg, "pkg.b");
    assert_eq!(current[1].r#ref.version, "0.7.0");
}

#[test]
fn single_package_test_matches_upstream_output_byte_for_byte() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let input_dir =
        manifest_dir.join("../../external/pkl/pkl-doc/src/test/files/SinglePackageTest/input");
    let expected_dir =
        manifest_dir.join("../../external/pkl/pkl-doc/src/test/files/SinglePackageTest/output");

    let tmp = TempDir::new("single-package");
    let out_dir = tmp.path().join("out");
    let cache_dir = tmp.path().join("cache");

    let mut inputs = Vec::new();
    fn collect_files(dir: &Path, out: &mut Vec<PathBuf>) {
        let mut entries: Vec<PathBuf> = fs::read_dir(dir)
            .expect("read_dir")
            .map(|e| e.expect("entry").path())
            .collect();
        entries.sort();
        for p in entries {
            if p.is_dir() {
                collect_files(&p, out);
            } else if p.is_file() {
                out.push(p);
            }
        }
    }
    collect_files(&input_dir, &mut inputs);

    let mut args = vec![
        "--output-dir".to_string(),
        out_dir.to_string_lossy().into_owned(),
        "--cache-dir".to_string(),
        cache_dir.to_string_lossy().into_owned(),
        "--no-symlinks".to_string(),
        "--test-mode".to_string(),
    ];
    for p in inputs {
        args.push(p.to_string_lossy().into_owned());
    }
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let code = run_cli_with_io_and_ansi_level(&args, &mut stdout, &mut stderr, AnsiLevel::None);
    assert_eq!(code, 0, "stderr: {}", String::from_utf8_lossy(&stderr));
    assert!(stderr.is_empty());

    let expected_tree = collect_tree_raw(&expected_dir);
    let actual_tree = collect_tree_raw(&out_dir);
    assert_eq!(actual_tree.len(), expected_tree.len());
    let branded_icons = [
        (
            "file/images/apple-touch-icon.png",
            "assets/images/apple-touch-icon.png",
        ),
        ("file/images/favicon.svg", "assets/images/favicon.svg"),
        (
            "file/images/favicon-16x16.png",
            "assets/images/favicon-16x16.png",
        ),
        (
            "file/images/favicon-32x32.png",
            "assets/images/favicon-32x32.png",
        ),
    ];
    for (k, exp) in &expected_tree {
        let act = actual_tree
            .get(k)
            .unwrap_or_else(|| panic!("missing generated path: {k}"));
        if let Some((_, asset_rel)) = branded_icons.iter().find(|(key, _)| *key == k.as_str()) {
            let branded = fs::read(manifest_dir.join(asset_rel)).expect("read branded icon");
            assert_eq!(act, &branded, "branded icon mismatch for {k}");
            assert_ne!(act, exp, "branded icon {k} must not match Apple Pkl logo");
        } else {
            assert_eq!(act, exp, "byte mismatch for SinglePackageTest path {k}");
        }
    }
}

#[test]
fn stdlib_doc_package_info_generates_symlink_and_pages() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let tmp = TempDir::new("stdlib-pkg");
    let pkg_dir = tmp.path().join("stdlib_pkg");
    let out_dir = tmp.path().join("out");
    let cache_dir = tmp.path().join("cache");
    fs::create_dir_all(&pkg_dir).unwrap();
    fs::copy(
        manifest_dir.join("../../external/pkl/stdlib/doc-package-info.pkl"),
        pkg_dir.join("doc-package-info.pkl"),
    )
    .unwrap();
    fs::copy(
        manifest_dir.join("../../tests/fixtures/m5/pkldoc/stdlib_sample/sample.pkl"),
        pkg_dir.join("sample.pkl"),
    )
    .unwrap();

    let (code, _stdout, stderr) = run(&[
        "--output-dir",
        out_dir.to_str().unwrap(),
        "--cache-dir",
        cache_dir.to_str().unwrap(),
        "--test-mode",
        pkg_dir.join("doc-package-info.pkl").to_str().unwrap(),
        pkg_dir.join("sample.pkl").to_str().unwrap(),
    ]);
    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(stderr.is_empty());
    let link = out_dir.join("pkl/current");
    assert_eq!(
        fs::read_link(&link).expect("pkl/current symlink"),
        Path::new("0.32.0")
    );
    assert!(out_dir.join("pkl/0.32.0/sample/index.html").is_file());
    assert_eq!(
        fs::read_to_string(out_dir.join(".pkldoc/VERSION")).unwrap(),
        "2"
    );
}

#[test]
fn mordant_help_usage_error_and_eval_color_parity() {
    let (code, tc_help, tc_err) = run_with_level(&["--help"], AnsiLevel::TrueColor);
    assert_eq!(code, 0);
    assert!(tc_err.is_empty());
    assert!(
        tc_help.starts_with(
            "\x1b[38;2;229;192;123mUsage:\x1b[39m pkldoc \x1b[2m[<options>]\x1b[22m \x1b[2m[<modules>]...\x1b[22m\n"
        ),
        "{tc_help:?}"
    );
    assert!(
        tc_help.contains("resolving \x1b[1mmodulepath:\x1b[22m URIs."),
        "{tc_help:?}"
    );

    let (code, _, a256_err) = run_with_level(&["foo.pkl"], AnsiLevel::Ansi256);
    assert_eq!(code, 1);
    assert_eq!(
        a256_err,
        "\x1b[38;5;186mUsage:\x1b[39m pkldoc \x1b[2m[<options>]\x1b[22m \x1b[2m[<modules>]...\x1b[22m\n\n\x1b[38;5;174mError:\x1b[39m missing option \x1b[38;5;111m--output-dir\x1b[39m\n"
    );

    // Subprocess env detection on `pkldoc` binary
    let bin = env!("CARGO_BIN_EXE_pkldoc");
    let out = Command::new(bin)
        .args(["--help"])
        .stdin(Stdio::null())
        .env_remove("NO_COLOR")
        .env_remove("FORCE_COLOR")
        .env_remove("CI")
        .env_remove("GITHUB_ACTIONS")
        .env("TERM", "xterm-256color")
        .env("COLORTERM", "truecolor")
        .output()
        .expect("spawn pkldoc");
    assert_eq!(out.status.code(), Some(0));
    assert!(
        String::from_utf8(out.stdout)
            .unwrap()
            .starts_with("\x1b[38;2;229;192;123mUsage:\x1b[39m pkldoc")
    );

    // Pkl evaluation error under `--color=always` emits ANSI diagnostics.
    let tmp = TempDir::new("color-err");
    let bad = tmp.path().join("doc-package-info.pkl");
    let alpha = tmp.path().join("Alpha.pkl");
    fs::write(&bad, "amends \"pkl:DocPackageInfo\"\nname = 1 + \"x\"\n").unwrap();
    fs::write(&alpha, "module com.example.Alpha\n").unwrap();
    let out_dir = tmp.path().join("out");
    let (code, _, stderr) = run(&[
        "--output-dir",
        out_dir.to_str().unwrap(),
        "--color=always",
        "--test-mode",
        bad.to_str().unwrap(),
        alpha.to_str().unwrap(),
    ]);
    assert_eq!(code, 1);
    assert!(
        stderr.contains("\x1b[31m–– Pkl Error ––\x1b[0m\n"),
        "{stderr:?}"
    );
    assert!(
        stderr.contains("Left operand : 1\nRight operand: \"x\"\n"),
        "{stderr:?}"
    );
}
