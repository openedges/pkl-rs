//! CLI engine tests mirroring `CliFormatterTest.kt` and `CliFormatterCommand.kt`.

use pkl_formatter::GrammarVersion;
use pkl_formatter::cli::{EXIT_ERROR, EXIT_SUCCESS, EXIT_VIOLATION, FormatCliOptions, run_cli};
use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(prefix: &str) -> Self {
        static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let id = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "pkl_formatter_cli_test_{prefix}_{}_{}_{}",
            std::process::id(),
            nanos,
            id
        ));
        fs::create_dir_all(&path).expect("create temp dir");
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

fn run(opts: &FormatCliOptions, stdin_data: &str) -> (i32, String, String) {
    let mut stdin = Cursor::new(stdin_data.as_bytes());
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let code = run_cli(opts, &mut stdin, &mut stdout, &mut stderr);
    (
        code,
        String::from_utf8(stdout).expect("utf8 stdout"),
        String::from_utf8(stderr).expect("utf8 stderr"),
    )
}

#[test]
fn no_double_newline_when_writing_to_stdout_and_exits_11_on_violation() {
    let dir = TempDir::new("no_double_nl");
    let file = dir.path().join("foo.pkl");
    fs::write(&file, "foo = 1").unwrap();

    let opts = FormatCliOptions {
        paths: vec![file.clone()],
        ..Default::default()
    };
    let (code, stdout, stderr) = run(&opts, "");
    assert_eq!(code, EXIT_VIOLATION);
    assert_eq!(stdout, "foo = 1\n");
    assert_eq!(stderr, "");
    // Original file untouched
    assert_eq!(fs::read_to_string(&file).unwrap(), "foo = 1");
}

#[test]
fn already_formatted_file_exits_0() {
    let dir = TempDir::new("clean");
    let file = dir.path().join("clean.pkl");
    fs::write(&file, "foo = 1\n").unwrap();

    let opts = FormatCliOptions {
        paths: vec![file],
        ..Default::default()
    };
    let (code, stdout, stderr) = run(&opts, "");
    assert_eq!(code, EXIT_SUCCESS);
    assert_eq!(stdout, "foo = 1\n");
    assert_eq!(stderr, "");
}

#[test]
fn parse_errors_report_exact_stderr_and_exit_1() {
    let dir = TempDir::new("parse_err");
    let file = dir.path().join("foo.pkl");
    fs::write(&file, "foo = \"/foo/\\${BAR}/baz\"").unwrap();

    let opts = FormatCliOptions {
        paths: vec![file.clone()],
        ..Default::default()
    };
    let (code, stdout, stderr) = run(&opts, "");
    assert_eq!(code, EXIT_ERROR);
    assert_eq!(stdout, "");
    let expected = format!(
        "Could not format `{}`: org.pkl.parser.ParserError: Invalid character escape sequence `\\$`.\n\nValid character escape sequences are:  \\n  \\r  \\t  \\\"  \\\\ \\<newline>\nAn error occurred during formatting.\n",
        file.display()
    );
    assert_eq!(stderr, expected);
}

#[test]
fn write_mode_overwrites_prints_changed_paths_and_exits_0() {
    let dir = TempDir::new("write");
    let dirty = dir.path().join("dirty.pkl");
    let clean = dir.path().join("clean.pkl");
    fs::write(&dirty, "a=1").unwrap();
    fs::write(&clean, "b = 2\n").unwrap();

    let opts = FormatCliOptions {
        paths: vec![dirty.clone(), clean.clone()],
        write: true,
        ..Default::default()
    };
    let (code, stdout, stderr) = run(&opts, "");
    assert_eq!(code, EXIT_SUCCESS);
    assert_eq!(stdout, format!("{}\n", dirty.display()));
    assert_eq!(stderr, "");
    assert_eq!(fs::read_to_string(&dirty).unwrap(), "a = 1\n");
    assert_eq!(fs::read_to_string(&clean).unwrap(), "b = 2\n");
}

#[test]
fn diff_name_only_prints_only_violated_files_and_exits_11() {
    let dir = TempDir::new("diff_name");
    let dirty = dir.path().join("dirty.pkl");
    let clean = dir.path().join("clean.pkl");
    fs::write(&dirty, "a=1\n").unwrap();
    fs::write(&clean, "b = 2\n").unwrap();

    let opts = FormatCliOptions {
        paths: vec![dirty.clone(), clean],
        diff_name_only: true,
        ..Default::default()
    };
    let (code, stdout, stderr) = run(&opts, "");
    assert_eq!(code, EXIT_VIOLATION);
    assert_eq!(stdout, format!("{}\n", dirty.display()));
    assert_eq!(stderr, "");
    assert_eq!(fs::read_to_string(&dirty).unwrap(), "a=1\n");
}

#[test]
fn silent_flag_matches_upstream_suppression_semantics() {
    let dir = TempDir::new("silent");
    let dirty = dir.path().join("dirty.pkl");
    fs::write(&dirty, "a=1").unwrap();

    // silent + diff_name_only suppresses path output, still exits 11
    let opts = FormatCliOptions {
        paths: vec![dirty.clone()],
        diff_name_only: true,
        silent: true,
        ..Default::default()
    };
    let (code, stdout, stderr) = run(&opts, "");
    assert_eq!(code, EXIT_VIOLATION);
    assert_eq!(stdout, "");
    assert_eq!(stderr, "");

    // silent + write suppresses path output, still overwrites and exits 0
    let opts = FormatCliOptions {
        paths: vec![dirty.clone()],
        write: true,
        silent: true,
        ..Default::default()
    };
    let (code, stdout, stderr) = run(&opts, "");
    assert_eq!(code, EXIT_SUCCESS);
    assert_eq!(stdout, "");
    assert_eq!(stderr, "");
    assert_eq!(fs::read_to_string(&dirty).unwrap(), "a = 1\n");

    // silent + parse error suppresses trailing "An error occurred during formatting."
    let bad = dir.path().join("bad.pkl");
    fs::write(&bad, "a = ").unwrap();
    let opts = FormatCliOptions {
        paths: vec![bad],
        silent: true,
        ..Default::default()
    };
    let (code, _, stderr) = run(&opts, "");
    assert_eq!(code, EXIT_ERROR);
    assert!(stderr.contains("Could not format"));
    assert!(!stderr.contains("An error occurred during formatting."));
}

#[test]
fn stdin_formatting_and_write_abort() {
    let opts = FormatCliOptions {
        paths: vec![PathBuf::from("-")],
        ..Default::default()
    };
    let (code, stdout, stderr) = run(&opts, "foo=1");
    assert_eq!(code, EXIT_VIOLATION);
    assert_eq!(stdout, "foo = 1\n");
    assert_eq!(stderr, "");

    let (code, stdout, stderr) = run(&opts, "foo = 1\n");
    assert_eq!(code, EXIT_SUCCESS);
    assert_eq!(stdout, "foo = 1\n");
    assert_eq!(stderr, "");

    // -w - aborts immediately with "Cannot write to stdin\n"
    let write_stdin = FormatCliOptions {
        paths: vec![PathBuf::from("-")],
        write: true,
        ..Default::default()
    };
    let (code, stdout, stderr) = run(&write_stdin, "foo = 1\n");
    assert_eq!(code, EXIT_ERROR);
    assert_eq!(stdout, "");
    assert_eq!(stderr, "Cannot write to stdin\n");
}

#[test]
fn grammar_version_option_via_cli() {
    let input = "x = fun(\"loooooooooooooooooooongString\", \"loooooooooooooooongString\", \"looooooooooooooooongString\", \"notTooLong\")\n";
    let opts_v1 = FormatCliOptions {
        paths: vec![PathBuf::from("-")],
        grammar_version: GrammarVersion::V1,
        ..Default::default()
    };
    let opts_v2 = FormatCliOptions {
        paths: vec![PathBuf::from("-")],
        grammar_version: GrammarVersion::V2,
        ..Default::default()
    };
    let (c1, out1, _) = run(&opts_v1, input);
    let (c2, out2, _) = run(&opts_v2, input);
    assert_eq!(c1, EXIT_VIOLATION);
    assert_eq!(c2, EXIT_VIOLATION);
    assert!(out1.contains("\"notTooLong\"\n  )"));
    assert!(out2.contains("\"notTooLong\",\n  )"));
}

#[test]
fn directory_walk_filters_pkl_and_pklproject_and_skips_nested_symlinks() {
    let dir = TempDir::new("walk");
    let sub = dir.path().join("sub");
    fs::create_dir_all(&sub).unwrap();

    let a_pkl = dir.path().join("a.pkl");
    let dot_pkl = dir.path().join(".pkl");
    let proj = sub.join("PklProject");
    let deps = sub.join("PklProject.deps.json");
    let txt = sub.join("notes.txt");

    fs::write(&a_pkl, "a=1").unwrap();
    fs::write(&dot_pkl, "d=2").unwrap();
    fs::write(&proj, "amends \"pkl:Project\"\npackage { name=\"x\" }").unwrap();
    fs::write(&deps, "{}").unwrap();
    fs::write(&txt, "not pkl {{{").unwrap();

    #[cfg(unix)]
    let outside = {
        let outside = TempDir::new("walk_symlink_target");
        let outside_pkl = outside.path().join("ignored.pkl");
        fs::write(&outside_pkl, "ignored=1").unwrap();
        std::os::unix::fs::symlink(outside.path(), dir.path().join("linked_dir")).unwrap();
        outside
    };

    let opts = FormatCliOptions {
        paths: vec![dir.path().to_path_buf()],
        write: true,
        ..Default::default()
    };
    let (code, stdout, stderr) = run(&opts, "");
    assert_eq!(code, EXIT_SUCCESS);
    assert_eq!(stderr, "");

    let mut reported: Vec<&str> = stdout.lines().collect();
    reported.sort();
    let mut expected = vec![
        a_pkl.to_string_lossy().into_owned(),
        dot_pkl.to_string_lossy().into_owned(),
        proj.to_string_lossy().into_owned(),
    ];
    expected.sort();
    assert_eq!(reported, expected);

    // Non-pkl files untouched
    assert_eq!(fs::read_to_string(&deps).unwrap(), "{}");
    assert_eq!(fs::read_to_string(&txt).unwrap(), "not pkl {{{");

    // Nested symlinked directory was NOT descended into
    #[cfg(unix)]
    assert_eq!(
        fs::read_to_string(outside.path().join("ignored.pkl")).unwrap(),
        "ignored=1"
    );
}

#[test]
fn deduplicates_paths_handles_missing_file_and_error_dominates_violation() {
    let dir = TempDir::new("status");
    let dirty = dir.path().join("dirty.pkl");
    let missing = dir.path().join("missing.pkl");
    fs::write(&dirty, "a=1").unwrap();

    // Duplicate paths only printed once
    let dup_opts = FormatCliOptions {
        paths: vec![dirty.clone(), dirty.clone()],
        diff_name_only: true,
        ..Default::default()
    };
    let (code, stdout, _) = run(&dup_opts, "");
    assert_eq!(code, EXIT_VIOLATION);
    assert_eq!(stdout.lines().count(), 1);

    // Error dominates violation; all paths still processed
    let mixed_opts = FormatCliOptions {
        paths: vec![dirty.clone(), missing.clone()],
        diff_name_only: true,
        ..Default::default()
    };
    let (code, stdout, stderr) = run(&mixed_opts, "");
    assert_eq!(code, EXIT_ERROR);
    assert_eq!(stdout, format!("{}\n", dirty.display()));
    assert_eq!(
        stderr,
        format!(
            "IO error while reading `{}`: {}\nAn error occurred during formatting.\n",
            missing.display(),
            missing.display()
        )
    );

    // Empty paths -> exit 0
    let (code, stdout, stderr) = run(&FormatCliOptions::default(), "");
    assert_eq!(code, EXIT_SUCCESS);
    assert_eq!(stdout, "");
    assert_eq!(stderr, "");
}
