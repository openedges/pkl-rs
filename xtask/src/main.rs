//! Read-only checks for the pinned upstream compatibility references.

use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::Path;
use std::process::{Command, ExitCode};

type Result<T> = std::result::Result<T, String>;

const LOCK_KEYS: &[&str] = &[
    "schema",
    "baseline.pkl",
    "pkl.path",
    "pkl.url",
    "pkl.revision",
    "pkl.version",
    "pkl-pantry.path",
    "pkl-pantry.url",
    "pkl-pantry.revision",
    "pkl-pantry.pkl-version",
];

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    match args.as_slice() {
        [] => {
            print_help();
            ExitCode::SUCCESS
        }
        [arg] if arg == "help" || arg == "--help" || arg == "-h" => {
            print_help();
            ExitCode::SUCCESS
        }
        [arg] if arg == "verify" => {
            let root = Path::new(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .expect("xtask is a workspace member");
            match verify(root) {
                Ok(baseline) => {
                    println!("Reference pins verified for Pkl {baseline}.");
                    ExitCode::SUCCESS
                }
                Err(error) => {
                    eprintln!("Reference verification failed: {error}");
                    ExitCode::FAILURE
                }
            }
        }
        _ => {
            eprintln!("Unknown command. Run `cargo xtask --help`.");
            ExitCode::FAILURE
        }
    }
}

fn print_help() {
    println!(
        "Usage: cargo xtask <command>\n\nCommands:\n  verify  Check reference pins, metadata, and clean submodule checkouts"
    );
}

fn verify(root: &Path) -> Result<String> {
    let contents = read(&root.join("references.lock"))?;
    let lock = parse_lock(&contents)?;
    for name in ["pkl", "pkl-pantry"] {
        verify_reference(root, name, &lock)?;
    }

    let pkl_version = property(
        &read(&root.join("external/pkl/gradle.properties"))?,
        "version",
    )?;
    equal("Pkl upstream version", &pkl_version, &lock["pkl.version"])?;
    let pantry_version = pantry_pkl_version(&read(
        &root.join("external/pkl-pantry/gradle/libs.versions.toml"),
    )?)?;
    equal(
        "Pantry Pkl dependency",
        &pantry_version,
        &lock["pkl-pantry.pkl-version"],
    )?;
    Ok(lock["baseline.pkl"].clone())
}

fn parse_lock(contents: &str) -> Result<BTreeMap<String, String>> {
    let mut values = BTreeMap::new();
    for (index, line) in contents.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (key, value) = line
            .split_once('=')
            .ok_or_else(|| format!("references.lock:{}: expected key=value", index + 1))?;
        let (key, value) = (key.trim(), value.trim());
        if !LOCK_KEYS.contains(&key) {
            return Err(format!("references.lock: unknown key {key:?}"));
        }
        if value.is_empty() {
            return Err(format!("references.lock: empty value for {key}"));
        }
        if values.insert(key.to_owned(), value.to_owned()).is_some() {
            return Err(format!("references.lock: duplicate key {key}"));
        }
    }
    for key in LOCK_KEYS {
        if !values.contains_key(*key) {
            return Err(format!("references.lock: missing key {key}"));
        }
    }
    equal("references.lock schema", &values["schema"], "1")?;
    for name in ["pkl", "pkl-pantry"] {
        equal(
            &format!("{name} path"),
            &values[&format!("{name}.path")],
            &format!("external/{name}"),
        )?;
        let revision = &values[&format!("{name}.revision")];
        if revision.len() != 40
            || !revision
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(format!(
                "{name} revision must be a full lowercase Git SHA-1"
            ));
        }
    }
    for key in ["pkl.version", "pkl-pantry.pkl-version"] {
        equal(key, &values[key], &values["baseline.pkl"])?;
    }
    Ok(values)
}

fn verify_reference(root: &Path, name: &str, lock: &BTreeMap<String, String>) -> Result<()> {
    let path = &lock[&format!("{name}.path")];
    let url = &lock[&format!("{name}.url")];
    let revision = &lock[&format!("{name}.revision")];
    let paths = git(
        root,
        &[
            "config",
            "--file",
            ".gitmodules",
            "--get-regexp",
            r"^submodule\..*\.path$",
        ],
    )?;
    let sections: Vec<&str> = paths
        .lines()
        .filter_map(|line| line.split_once(' '))
        .filter(|(_, value)| *value == path)
        .map(|(key, _)| key.trim_end_matches(".path"))
        .collect();
    if sections.len() != 1 {
        return Err(format!(".gitmodules must declare {path} exactly once"));
    }
    let declared_url = git(
        root,
        &[
            "config",
            "--file",
            ".gitmodules",
            "--get-all",
            &format!("{}.url", sections[0]),
        ],
    )?;
    equal(&format!("{name} .gitmodules URL"), declared_url.trim(), url)?;

    let index = git(root, &["ls-files", "--stage", "--", path])?;
    let expected = format!("160000 {revision} 0\t{path}\n");
    equal(&format!("{name} staged gitlink"), &index, &expected)?;

    let checkout = root.join(path);
    if !checkout.join(".git").exists() {
        return Err(format!(
            "{path} is not initialized; run `git submodule update --init --recursive`"
        ));
    }
    let head = git(&checkout, &["rev-parse", "HEAD"])?;
    equal(&format!("{name} checkout HEAD"), head.trim(), revision)?;
    let status = git(
        &checkout,
        &[
            "status",
            "--porcelain=v1",
            "--untracked-files=all",
            "--ignore-submodules=none",
        ],
    )?;
    if !status.is_empty() {
        return Err(format!("{path} has local changes:\n{status}"));
    }
    Ok(())
}

fn property(contents: &str, name: &str) -> Result<String> {
    let matches: Vec<&str> = contents
        .lines()
        .map(str::trim)
        .filter(|line| !line.starts_with('#') && !line.starts_with('!'))
        .filter_map(|line| line.split_once('='))
        .filter(|(key, _)| key.trim() == name)
        .map(|(_, value)| value.trim())
        .collect();
    match matches.as_slice() {
        [value] if !value.is_empty() => Ok((*value).to_owned()),
        _ => Err(format!("expected exactly one nonempty {name} property")),
    }
}

fn pantry_pkl_version(contents: &str) -> Result<String> {
    let mut in_versions = false;
    let mut versions = String::new();
    for line in contents.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_versions = line == "[versions]";
        } else if in_versions {
            versions.push_str(line);
            versions.push('\n');
        }
    }
    let quoted = property(&versions, "pkl")?;
    quoted
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| "Pantry [versions].pkl must be a quoted version".to_owned())
}

fn equal(label: &str, actual: &str, expected: &str) -> Result<()> {
    if actual == expected {
        Ok(())
    } else {
        Err(format!("{label}: expected {expected:?}, found {actual:?}"))
    }
}

fn read(path: &Path) -> Result<String> {
    fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))
}

fn git(directory: &Path, args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .current_dir(directory)
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .env("GIT_OPTIONAL_LOCKS", "0")
        .args(args)
        .output()
        .map_err(|error| format!("cannot run git in {}: {error}", directory.display()))?;
    if !output.status.success() {
        return Err(format!(
            "git {} in {} failed: {}",
            args.join(" "),
            directory.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    String::from_utf8(output.stdout).map_err(|error| format!("git output is not UTF-8: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    const LOCK: &str = include_str!("../../references.lock");
    static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

    struct Fixture(PathBuf);

    impl Fixture {
        fn new() -> Self {
            let path = env::temp_dir().join(format!(
                "pkl-rs-xtask-{}-{}",
                std::process::id(),
                NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            let fixture = Self(path);
            fixture.run(&["init", "--quiet"]);
            let mut lock = parse_lock(LOCK).unwrap();
            let mut modules = String::new();
            for name in ["pkl", "pkl-pantry"] {
                let relative = format!("external/{name}");
                let checkout = fixture.0.join(&relative);
                fs::create_dir_all(&checkout).unwrap();
                git(&checkout, &["init", "--quiet"]).unwrap();
                if name == "pkl" {
                    fs::write(checkout.join("gradle.properties"), "version=0.32.0\n").unwrap();
                } else {
                    fs::create_dir(checkout.join("gradle")).unwrap();
                    fs::write(
                        checkout.join("gradle/libs.versions.toml"),
                        "[versions]\npkl = \"0.32.0\"\n[plugins]\npkl = {}\n",
                    )
                    .unwrap();
                }
                fs::write(checkout.join(".gitignore"), "/build/\n").unwrap();
                git(&checkout, &["add", "."]).unwrap();
                git(
                    &checkout,
                    &[
                        "-c",
                        "user.name=Fixture",
                        "-c",
                        "user.email=fixture@example.invalid",
                        "-c",
                        "commit.gpgsign=false",
                        "-c",
                        "core.hooksPath=/dev/null",
                        "commit",
                        "--quiet",
                        "-m",
                        "fixture",
                    ],
                )
                .unwrap();
                let revision = git(&checkout, &["rev-parse", "HEAD"]).unwrap();
                let revision = revision.trim();
                lock.insert(format!("{name}.revision"), revision.to_owned());
                fixture.run(&[
                    "update-index",
                    "--add",
                    "--cacheinfo",
                    &format!("160000,{revision},{relative}"),
                ]);
                modules.push_str(&format!(
                    "[submodule \"{name}\"]\n\tpath = {relative}\n\turl = {}\n",
                    lock[&format!("{name}.url")]
                ));
            }
            fs::write(fixture.0.join(".gitmodules"), modules).unwrap();
            let contents = lock
                .into_iter()
                .map(|(key, value)| format!("{key}={value}\n"))
                .collect::<String>();
            fs::write(fixture.0.join("references.lock"), contents).unwrap();
            fixture
        }

        fn run(&self, args: &[&str]) -> String {
            git(&self.0, args).unwrap()
        }

        fn error(&self) -> String {
            verify(&self.0).unwrap_err()
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn lock_rejects_missing_duplicate_unknown_and_invalid_fields() {
        assert_eq!(parse_lock(LOCK).unwrap()["baseline.pkl"], "0.32.0");
        for (contents, expected) in [
            (LOCK.replace("schema=1\n", ""), "missing key schema"),
            (format!("{LOCK}\nschema=1\n"), "duplicate key schema"),
            (format!("{LOCK}\nunknown=x\n"), "unknown key"),
            (LOCK.replace("schema=1", "schema="), "empty value"),
            (LOCK.replace("schema=1", "schema=2"), "schema: expected"),
            (LOCK.replace("schema=1", "schema"), "expected key=value"),
            (
                LOCK.replace("pkl.version=0.32.0", "pkl.version=0.31.0"),
                "pkl.version: expected",
            ),
            (
                LOCK.replace("pkl.path=external/pkl", "pkl.path=../pkl"),
                "pkl path: expected",
            ),
            (
                LOCK.replace("e51fb2f91774265666f94980ec630db794a4db2d", "e51fb2f"),
                "full lowercase Git SHA-1",
            ),
        ] {
            let error = parse_lock(&contents).unwrap_err();
            assert!(error.contains(expected), "{error}");
        }
    }

    #[test]
    fn verifies_staged_submodules_before_initial_parent_commit() {
        let fixture = Fixture::new();
        assert_eq!(verify(&fixture.0).unwrap(), "0.32.0");
        let build = fixture.0.join("external/pkl/build");
        fs::create_dir(&build).unwrap();
        fs::write(build.join("output.txt"), "ignored output").unwrap();
        assert!(verify(&fixture.0).is_ok());
    }

    #[test]
    fn rejects_dirty_checkout_and_uninitialized_submodule() {
        let fixture = Fixture::new();
        let untracked = fixture.0.join("external/pkl/untracked.txt");
        fs::write(&untracked, "local change").unwrap();
        assert!(fixture.error().contains("has local changes"));
        fs::remove_file(untracked).unwrap();
        fs::rename(
            fixture.0.join("external/pkl/.git"),
            fixture.0.join("uninitialized-pkl-git"),
        )
        .unwrap();
        assert!(fixture.error().contains("is not initialized"));
    }

    #[test]
    fn rejects_wrong_declared_url_and_missing_gitlink() {
        let fixture = Fixture::new();
        fixture.run(&[
            "config",
            "--file",
            ".gitmodules",
            "submodule.pkl.url",
            "https://example.invalid/pkl.git",
        ]);
        assert!(fixture.error().contains(".gitmodules URL"));
        fixture.run(&[
            "config",
            "--file",
            ".gitmodules",
            "submodule.pkl.url",
            "https://github.com/apple/pkl.git",
        ]);
        fixture.run(&["update-index", "--force-remove", "external/pkl"]);
        assert!(fixture.error().contains("staged gitlink"));
    }

    #[test]
    fn rejects_checkout_revision_mismatch() {
        let fixture = Fixture::new();
        let checkout = fixture.0.join("external/pkl");
        git(
            &checkout,
            &[
                "-c",
                "user.name=Fixture",
                "-c",
                "user.email=fixture@example.invalid",
                "-c",
                "commit.gpgsign=false",
                "-c",
                "core.hooksPath=/dev/null",
                "commit",
                "--quiet",
                "--allow-empty",
                "-m",
                "wrong revision",
            ],
        )
        .unwrap();
        assert!(fixture.error().contains("checkout HEAD"));
    }

    #[test]
    fn version_metadata_rejects_absent_and_duplicate_values() {
        assert!(property("# version=0.32.0\n", "version").is_err());
        assert!(property("version=0.32.0\nversion=0.33.0", "version").is_err());
        assert!(pantry_pkl_version("[plugins]\npkl = \"0.32.0\"").is_err());
        assert!(pantry_pkl_version("[versions]\npkl = 0.32.0").is_err());
        assert_eq!(
            pantry_pkl_version("[versions]\npkl = \"0.32.0\"\n[plugins]\npkl = {}").unwrap(),
            "0.32.0"
        );
    }
}
