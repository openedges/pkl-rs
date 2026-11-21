//! Integration tests for the product `pkl` CLI binary (`crates/pkl-cli`).

use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

fn pkl_bin() -> &'static str {
    env!("CARGO_BIN_EXE_pkl")
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root")
        .to_path_buf()
}

fn make_temp_dir(label: &str) -> PathBuf {
    let n = TEMP_COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir =
        std::env::temp_dir().join(format!("pkl-cli-test-{}-{}-{n}", label, std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

fn run_pkl(args: &[&str]) -> Output {
    Command::new(pkl_bin())
        .args(args)
        .current_dir(workspace_root())
        .stdin(Stdio::null())
        .output()
        .expect("spawn pkl")
}

fn run_pkl_with_stdin(args: &[&str], stdin_bytes: &[u8]) -> Output {
    use std::io::Write;
    let mut child = Command::new(pkl_bin())
        .args(args)
        .current_dir(workspace_root())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn pkl");
    child
        .stdin
        .as_mut()
        .expect("stdin")
        .write_all(stdin_bytes)
        .expect("write stdin");
    child.wait_with_output().expect("wait pkl")
}

#[test]
fn version_flags_emit_honest_native_banner() {
    for flag in ["--version", "-v"] {
        let out = run_pkl(&[flag]);
        assert_eq!(out.status.code(), Some(0), "flag {flag}");
        assert!(out.stderr.is_empty(), "stderr not empty for {flag}");
        let stdout = String::from_utf8(out.stdout).expect("utf8");
        assert!(
            stdout.starts_with("Pkl 0.32.0 (") && stdout.ends_with(", native)\n"),
            "unexpected version banner for {flag}: {stdout:?}"
        );
    }
}

#[test]
fn root_and_subcommand_help_and_unknown_subcommand() {
    for args in [
        &[][..],
        &["--help"],
        &["-h"],
        &["eval", "--help"],
        &["test", "--help"],
        &["repl", "--help"],
        &["server", "--help"],
        &["format", "--help"],
        &["run", "--help"],
        &["shell-completion", "--help"],
        &["project", "--help"],
        &["project"],
        &["project", "resolve", "--help"],
        &["project", "package", "--help"],
        &["download-package", "--help"],
        &["analyze", "--help"],
        &["analyze"],
        &["analyze", "imports", "--help"],
    ] {
        let out = run_pkl(args);
        assert_eq!(out.status.code(), Some(0), "help failed for {args:?}");
        assert!(out.stderr.is_empty(), "non-empty stderr for {args:?}");
        let stdout = String::from_utf8(out.stdout).expect("utf8");
        assert!(
            stdout.starts_with("Usage: pkl"),
            "missing Usage header for {args:?}: {stdout:?}"
        );
        assert!(
            stdout.ends_with("https://pkl-lang.org/main/0.32.0/pkl-cli/index.html#usage\n"),
            "missing epilog for {args:?}"
        );
    }

    let bogus = run_pkl(&["bogus-cmd"]);
    assert_eq!(bogus.status.code(), Some(1));
    assert!(bogus.stdout.is_empty());
    let stderr = String::from_utf8(bogus.stderr).expect("utf8");
    assert_eq!(
        stderr,
        "Usage: pkl [<options>] <command> [<args>]...\n\nError: no such subcommand bogus-cmd. (Possible subcommands: eval, repl, server, test, project, download-package, analyze, format, run, shell-completion)\n"
    );
}

#[test]
fn shell_completion_valid_and_invalid() {
    let bash = run_pkl(&["shell-completion", "bash"]);
    assert_eq!(bash.status.code(), Some(0));
    assert!(bash.stderr.is_empty());
    assert!(bash.stdout.starts_with(b"#!/usr/bin/env bash\n"));

    let zsh = run_pkl(&["shell-completion", "zsh"]);
    assert_eq!(zsh.status.code(), Some(0));
    assert!(zsh.stderr.is_empty());
    assert!(zsh.stdout.starts_with(b"#!/usr/bin/env zsh\n"));

    let fish = run_pkl(&["shell-completion", "fish"]);
    assert_eq!(fish.status.code(), Some(0));
    assert!(fish.stderr.is_empty());
    assert!(fish.stdout.starts_with(b"# Command completion for pkl\n"));

    let bad = run_pkl(&["shell-completion", "powershell"]);
    assert_eq!(bad.status.code(), Some(1));
    assert!(bad.stdout.is_empty());
    assert_eq!(
        String::from_utf8(bad.stderr).unwrap(),
        "Usage: pkl shell-completion [<options>] <shell>\n\nError: invalid value for <shell>: invalid choice: powershell. (choose from bash, zsh, fish)\n"
    );
}

#[test]
fn eval_color_power_assertions_trace_separator_and_stdlib() {
    let root = workspace_root();
    let color_err = root.join("tests/fixtures/m5/cli/eval_color_err.pkl");
    let trace_file = root.join("tests/fixtures/m5/cli/eval_trace.pkl");
    let multi_a = root.join("tests/fixtures/m5/cli/eval_multi_a.pkl");
    let multi_b = root.join("tests/fixtures/m5/cli/eval_multi_b.pkl");
    let doc_pkg = root.join("external/pkl/stdlib/doc-package-info.pkl");

    // --color=always includes ANSI escape codes
    let out = run_pkl(&[
        "eval",
        "--no-project",
        "--color=always",
        color_err.to_str().unwrap(),
    ]);
    assert_eq!(out.status.code(), Some(1));
    assert!(out.stdout.is_empty());
    let stderr = String::from_utf8(out.stderr).unwrap();
    assert!(
        stderr.contains("\x1b[31m–– Pkl Error ––\x1b[0m"),
        "{stderr:?}"
    );

    // --power-assertions on vs off
    let on = run_pkl(&[
        "eval",
        "--no-project",
        "--color=never",
        "--power-assertions",
        color_err.to_str().unwrap(),
    ]);
    assert_eq!(on.status.code(), Some(1));
    let on_err = String::from_utf8(on.stderr).unwrap();
    assert!(on_err.contains("    this == 10\n    │    │\n    3    false\n"));

    let off = run_pkl(&[
        "eval",
        "--no-project",
        "--color=never",
        "--no-power-assertions",
        color_err.to_str().unwrap(),
    ]);
    assert_eq!(off.status.code(), Some(1));
    let off_err = String::from_utf8(off.stderr).unwrap();
    assert!(!off_err.contains("│"));

    // --trace-mode=pretty
    let tr = run_pkl(&[
        "eval",
        "--no-project",
        "--color=never",
        "--trace-mode=pretty",
        trace_file.to_str().unwrap(),
    ]);
    assert_eq!(tr.status.code(), Some(0));
    let tr_err = String::from_utf8(tr.stderr).unwrap();
    assert!(
        tr_err.contains("pkl: TRACE: data =\n  new Dynamic {"),
        "{tr_err:?}"
    );

    // --module-output-separator + -o
    let tmp = make_temp_dir("eval-sep");
    let out_file = tmp.join("combined.pcf");
    let sep = run_pkl(&[
        "eval",
        "--no-project",
        "--color=never",
        "--module-output-separator=//SEP//",
        "-o",
        out_file.to_str().unwrap(),
        multi_a.to_str().unwrap(),
        multi_b.to_str().unwrap(),
    ]);
    assert_eq!(sep.status.code(), Some(0));
    assert!(sep.stdout.is_empty());
    assert_eq!(
        std::fs::read_to_string(&out_file).unwrap(),
        "first = \"one\"\n//SEP//\nsecond = \"two\"\n"
    );
    let _ = std::fs::remove_dir_all(&tmp);

    // stdlib/doc-package-info.pkl
    let doc = run_pkl(&[
        "eval",
        "--no-project",
        "--color=never",
        doc_pkg.to_str().unwrap(),
    ]);
    assert_eq!(doc.status.code(), Some(0));
    let doc_out = String::from_utf8(doc.stdout).unwrap();
    assert!(doc_out.contains("name = \"pkl\"\nversion = \"0.32.0\"\n"));
}

#[test]
fn test_reporters_junit_overwrite_and_no_modules() {
    let root = workspace_root();
    let pass = root.join("tests/fixtures/m5/cli/test_pass.pkl");

    let no_mod = run_pkl(&["test", "--no-project"]);
    assert_eq!(no_mod.status.code(), Some(1));
    assert_eq!(
        String::from_utf8(no_mod.stderr).unwrap(),
        "Usage: pkl test [<options>] <modules>...\n\nError: missing argument <modules>\n"
    );

    let spec = run_pkl(&[
        "test",
        "--no-project",
        "--color=never",
        "--test-reporter=spec",
        pass.to_str().unwrap(),
    ]);
    assert_eq!(spec.status.code(), Some(0));
    assert_eq!(
        String::from_utf8(spec.stdout).unwrap(),
        "module test_pass\n  facts\n    ✔ math holds\n  examples\n    ✔ greeting\n\n100.0% tests pass [2 passed], 100.0% asserts pass [3 passed]\n"
    );

    let minimal = run_pkl(&[
        "test",
        "--no-project",
        "--color=never",
        "--test-reporter=minimal",
        pass.to_str().unwrap(),
    ]);
    assert_eq!(minimal.status.code(), Some(0));
    assert_eq!(
        String::from_utf8(minimal.stdout).unwrap(),
        "\n100.0% tests pass [2 passed], 100.0% asserts pass [3 passed]\n"
    );

    // --overwrite exit 10 + file generation
    let tmp = make_temp_dir("test-overwrite");
    let dst = tmp.join("test_overwrite.pkl");
    std::fs::copy(root.join("tests/fixtures/m5/cli/test_overwrite.pkl"), &dst).unwrap();
    let ov = run_pkl(&[
        "test",
        "--no-project",
        "--color=never",
        "--overwrite",
        dst.to_str().unwrap(),
    ]);
    assert_eq!(ov.status.code(), Some(10));
    assert_eq!(
        String::from_utf8(ov.stdout).unwrap(),
        "module test_overwrite\n  examples\n    ✍️ generated example\n\n1 examples written\n"
    );
    assert!(tmp.join("test_overwrite.pkl-expected.pcf").is_file());
    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn run_command_help_execution_and_errors() {
    let root = workspace_root();
    let basic = root.join("tests/fixtures/m5/cli/cmd_basic.pkl");
    let err_cmd = root.join("tests/fixtures/m5/cli/cmd_error.pkl");

    let help = run_pkl(&["run", "--no-project", basic.to_str().unwrap(), "--help"]);
    assert_eq!(help.status.code(), Some(0));
    assert!(help.stderr.is_empty());
    let help_text = String::from_utf8(help.stdout).unwrap();
    assert!(help_text.contains("-n, --name=<text>"), "{help_text}");
    assert!(help_text.contains("--mode=<fast, safe>"), "{help_text}");
    assert!(
        help_text.contains("--dry-run / --no-dry-run"),
        "{help_text}"
    );

    let exec = run_pkl(&[
        "run",
        "--no-project",
        "--color=never",
        basic.to_str().unwrap(),
        "-n",
        "Alice",
        "--count=3",
        "--mode=safe",
        "--dry-run",
        "-vv",
        "--delay=12.s",
        "--tag=a",
        "--tag=b",
        "--label=k1=v1",
        "--label=k2=v2",
        "pos1",
        "pos2",
    ]);
    assert_eq!(
        exec.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&exec.stderr)
    );
    assert_eq!(
        String::from_utf8(exec.stdout).unwrap(),
        "result {\n  greeting = \"hello, Alice\"\n  count = 3\n  mode = \"safe\"\n  dryRun = true\n  verbose = 2\n  delay = 12.s\n  tags = List(\"a\", \"b\")\n  labels = Map(\"k1\", \"v1\", \"k2\", \"v2\")\n  inputs {\n    \"pos1\"\n    \"pos2\"\n  }\n}\n"
    );

    let missing = run_pkl(&[
        "run",
        "--no-project",
        "--color=never",
        err_cmd.to_str().unwrap(),
    ]);
    assert_eq!(missing.status.code(), Some(1));
    assert!(
        String::from_utf8(missing.stderr)
            .unwrap()
            .contains("Error: missing option --requiredFlag\n")
    );

    let bad_conv = run_pkl(&[
        "run",
        "--no-project",
        "--color=never",
        err_cmd.to_str().unwrap(),
        "--requiredFlag=ok",
        "--checked=bad",
    ]);
    assert_eq!(bad_conv.status.code(), Some(1));
    let conv_err = String::from_utf8(bad_conv.stderr).unwrap();
    assert!(
        conv_err.contains(
            "Error: invalid value for --checked: \n–– Pkl Error ––\ninvalid custom value"
        ),
        "{conv_err:?}"
    );
}

#[test]
fn format_and_analyze_imports_cli() {
    let out = run_pkl_with_stdin(&["format", "-"], b"x=1\n");
    assert_eq!(out.status.code(), Some(11));
    assert_eq!(out.stdout, b"x = 1\n");

    let ok = run_pkl_with_stdin(&["format", "-"], b"x = 1\n");
    assert_eq!(ok.status.code(), Some(0));
    assert_eq!(ok.stdout, b"x = 1\n");

    let err = run_pkl_with_stdin(&["format", "-w", "-"], b"a=1\n");
    assert_eq!(err.status.code(), Some(1));
    assert_eq!(err.stderr, b"Cannot write to stdin\n");

    let root = workspace_root();
    let tmp = make_temp_dir("analyze-imports");
    let out_file = tmp.join("imports.pcf");
    let main_pkl = root.join("tests/fixtures/m5/api/project_sample/main.pkl");
    let an = run_pkl(&[
        "analyze",
        "imports",
        "--no-project",
        "-o",
        out_file.to_str().unwrap(),
        main_pkl.to_str().unwrap(),
    ]);
    assert_eq!(
        an.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&an.stderr)
    );
    let content = std::fs::read_to_string(&out_file).unwrap();
    assert!(content.contains("imports {") && content.contains("resolvedImports {"));
    let _ = std::fs::remove_dir_all(&tmp);
}
