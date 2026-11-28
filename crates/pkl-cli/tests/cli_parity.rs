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
    run_pkl_with_env(args, &[("NO_COLOR", "1")])
}

fn run_pkl_with_env(args: &[&str], env: &[(&str, &str)]) -> Output {
    let mut cmd = Command::new(pkl_bin());
    cmd.args(args)
        .current_dir(workspace_root())
        .stdin(Stdio::null())
        .env_remove("NO_COLOR")
        .env_remove("FORCE_COLOR")
        .env_remove("TERM")
        .env_remove("COLORTERM")
        .env_remove("TERM_PROGRAM")
        .env_remove("TERM_PROGRAM_VERSION")
        .env_remove("WT_SESSION")
        .env_remove("ANSICON")
        .env_remove("ConEmuANSI")
        .env_remove("CI")
        .env_remove("GITHUB_ACTIONS");
    for (k, v) in env {
        cmd.env(k, v);
    }
    cmd.output().expect("spawn pkl")
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
            stdout.starts_with("pkl-rs 0.32.0 (") && stdout.ends_with(", native)\n"),
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

#[test]
fn ansi_color_parity_across_cli_and_diagnostics() {
    let root = workspace_root();

    // 1. Clikt/Mordant help & usage error colors across TrueColor / Ansi256 / Ansi16 / None.
    let tc_help = run_pkl_with_env(
        &["--help"],
        &[("TERM", "xterm-256color"), ("COLORTERM", "truecolor")],
    );
    assert_eq!(tc_help.status.code(), Some(0));
    let tc_stdout = String::from_utf8(tc_help.stdout).unwrap();
    assert!(tc_stdout.starts_with("\x1b[38;2;229;192;123mUsage:\x1b[39m pkl \x1b[2m[<options>]\x1b[22m \x1b[2m<command> [<args>]...\x1b[22m\n"));
    assert!(
        tc_stdout
            .contains("\x1b[38;2;97;175;239m-v\x1b[39m, \x1b[38;2;97;175;239m--version\x1b[39m")
    );

    let a256_fmt = run_pkl_with_env(&["format", "--help"], &[("TERM", "xterm-256color")]);
    assert_eq!(a256_fmt.status.code(), Some(0));
    let a256_stdout = String::from_utf8(a256_fmt.stdout).unwrap();
    assert!(a256_stdout.contains("\x1b[38;5;186mUsage:\x1b[39m"));
    assert!(a256_stdout.contains("\x1b[1m2\x1b[22m: 0.30+ \x1b[1m(default)\x1b[22m"));

    let bad_flag = run_pkl_with_env(
        &["--bad-flag"],
        &[("TERM", "xterm-256color"), ("COLORTERM", "truecolor")],
    );
    assert_eq!(bad_flag.status.code(), Some(1));
    assert_eq!(
        String::from_utf8(bad_flag.stderr).unwrap(),
        "\x1b[38;2;229;192;123mUsage:\x1b[39m pkl \x1b[2m[<options>]\x1b[22m \x1b[2m<command> [<args>]...\x1b[22m\n\n\x1b[38;2;224;108;117mError:\x1b[39m no such option \x1b[38;2;97;175;239m--bad-flag\x1b[39m. (Possible options: \x1b[38;2;97;175;239m-v\x1b[39m, \x1b[38;2;97;175;239m--version\x1b[39m, \x1b[38;2;97;175;239m-h\x1b[39m, \x1b[38;2;97;175;239m--help\x1b[39m)\n"
    );

    let run_help = run_pkl_with_env(
        &[
            "run",
            "--no-project",
            root.join("tests/fixtures/m5/cli/cmd_basic.pkl")
                .to_str()
                .unwrap(),
            "--help",
        ],
        &[("TERM", "xterm")],
    );
    assert_eq!(run_help.status.code(), Some(0));
    let run_help_stdout = String::from_utf8(run_help.stdout).unwrap();
    assert!(
        run_help_stdout.contains("\x1b[36m--mode\x1b[39m=\x1b[33;2m<fast, safe>\x1b[39;22m"),
        "{run_help_stdout:?}"
    );

    let tmp = make_temp_dir("ansi-color-parity");

    // 2. Uncolored ProgramValue trailer vs colored VmTypeMismatchException.
    let op_err = tmp.join("op_err.pkl");
    std::fs::write(&op_err, "a = 1 + \"x\"\n").unwrap();
    let op_out = run_pkl(&[
        "eval",
        "--no-project",
        "--color=always",
        op_err.to_str().unwrap(),
    ]);
    assert_eq!(op_out.status.code(), Some(1));
    let op_stderr = String::from_utf8(op_out.stderr).unwrap();
    assert!(
        op_stderr.contains(
            "\x1b[1;31mOperator `+` is not defined for operand types `Int` and `String`.\x1b[0m\nLeft operand : 1\nRight operand: \"x\"\n"
        ),
        "{op_stderr:?}"
    );

    // 3. Union type mismatch hint + uncolored `|` operator token in SyntaxHighlighter.
    let union_err = tmp.join("union_err.pkl");
    std::fs::write(&union_err, "x: Int(this > 5) | String(length > 3) = 2\n").unwrap();
    let union_out = run_pkl(&[
        "eval",
        "--no-project",
        "--color=always",
        union_err.to_str().unwrap(),
    ]);
    assert_eq!(union_out.status.code(), Some(1));
    let union_stderr = String::from_utf8(union_out.stderr).unwrap();
    assert!(
        union_stderr.contains(
            "x: Int(\x1b[34mthis\x1b[0m \x1b[0m>\x1b[0m \x1b[32m5\x1b[0m) | String(length \x1b[0m>\x1b[0m \x1b[32m3\x1b[0m) = \x1b[32m2\x1b[0m\n"
        ),
        "bare union `|` must stay uncolored: {union_stderr:?}"
    );
    assert!(
        union_stderr.contains("\n\x1b[33m* Value is not of type `Int(this > 5)` because:\n"),
        "union hint must not have spurious yellow reset: {union_stderr:?}"
    );
    assert!(
        !union_stderr.contains("\x1b[33m\x1b[0m* Value is not of type"),
        "union hint must not have spurious yellow reset: {union_stderr:?}"
    );

    // 4. VmWrappedEvalException colored inner body in `pkl analyze imports --color=always`.
    let syn_err = tmp.join("syn_err.pkl");
    std::fs::write(&syn_err, "a = (\n").unwrap();
    let an_out = run_pkl(&[
        "analyze",
        "imports",
        "--no-project",
        "--color=always",
        syn_err.to_str().unwrap(),
    ]);
    assert_eq!(an_out.status.code(), Some(1));
    let an_stderr = String::from_utf8(an_out.stderr).unwrap();
    assert!(
        an_stderr.contains(">\t\x1b[1;31mUnexpected end of file.\x1b[0m\n"),
        "{an_stderr:?}"
    );
    assert!(an_stderr.contains(">\t\x1b[0m\x1b[0m\n"), "{an_stderr:?}");
    assert!(an_stderr.ends_with("\x1b[0m\n"), "{an_stderr:?}");

    // 5. `pkl test --color=always` failing fact power assertions with local variables + non-test module.
    let fail_fact = tmp.join("fail_fact.pkl");
    std::fs::write(
        &fail_fact,
        "amends \"pkl:test\"\nlocal x = 1\nlocal y = 2\nfacts {\n  [\"math\"] {\n    x + y == 5\n  }\n}\n",
    )
    .unwrap();
    let tf_out = run_pkl(&[
        "test",
        "--no-project",
        "--color=always",
        fail_fact.to_str().unwrap(),
    ]);
    assert_eq!(tf_out.status.code(), Some(1));
    let tf_stdout = String::from_utf8(tf_out.stdout).unwrap();
    assert!(
        tf_stdout
            .contains("\x1b[2m│\x1b[0m \x1b[2m│\x1b[0m \x1b[2m│\x1b[0m \x1b[36mfalse\x1b[0m\n"),
        "{tf_stdout:?}"
    );
    assert!(
        tf_stdout.contains("\x1b[2m│\x1b[0m \x1b[2m│\x1b[0m \x1b[32m2\x1b[0m\n"),
        "{tf_stdout:?}"
    );
    assert!(
        tf_stdout.contains("\x1b[2m│\x1b[0m \x1b[32m3\x1b[0m\n"),
        "{tf_stdout:?}"
    );
    assert!(
        tf_stdout.contains("       \x1b[32m1\x1b[0m\n"),
        "{tf_stdout:?}"
    );

    let not_test = tmp.join("not_test.pkl");
    std::fs::write(&not_test, "a = 1\n").unwrap();
    let nt_out = run_pkl(&[
        "test",
        "--no-project",
        "--color=always",
        not_test.to_str().unwrap(),
    ]);
    assert_eq!(nt_out.status.code(), Some(1));
    let nt_stdout = String::from_utf8(nt_out.stdout).unwrap();
    assert!(
        nt_stdout.contains("\x1b[1;31mExpected value of type `pkl.test`, but got type `Class`.\x1b[0m\n  Value: not_test\n"),
        "{nt_stdout:?}"
    );

    // 6. CLI `--color` defaults to `auto`, overriding `PklProject.evaluatorSettings.color = "always"` over a pipe.
    let proj_dir = tmp.join("proj_always");
    std::fs::create_dir_all(&proj_dir).unwrap();
    std::fs::write(
        proj_dir.join("PklProject"),
        "amends \"pkl:Project\"\nevaluatorSettings {\n  color = \"always\"\n}\n",
    )
    .unwrap();
    let proj_mod = proj_dir.join("err.pkl");
    std::fs::write(&proj_mod, "x = 1 + \"a\"\n").unwrap();
    let proj_out = run_pkl(&[
        "eval",
        "--project-dir",
        proj_dir.to_str().unwrap(),
        proj_mod.to_str().unwrap(),
    ]);
    assert_eq!(proj_out.status.code(), Some(1));
    let proj_stderr = String::from_utf8(proj_out.stderr).unwrap();
    assert!(
        !proj_stderr.contains('\x1b'),
        "CLI default --color=auto on pipe must override PklProject color=always: {proj_stderr:?}"
    );

    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn static_project_fast_path_matches_pkl_0_32_0_on_invalid_manifests() {
    let tmp = make_temp_dir("static-project-parity");

    let run_with_project = |name: &str, project_src: &str| {
        let proj = tmp.join(name);
        std::fs::create_dir_all(&proj).unwrap();
        std::fs::write(proj.join("PklProject"), project_src).unwrap();
        let main_pkl = proj.join("main.pkl");
        std::fs::write(&main_pkl, "x = 1\n").unwrap();
        run_pkl(&[
            "eval",
            "--color=never",
            "--project-dir",
            proj.to_str().unwrap(),
            main_pkl.to_str().unwrap(),
        ])
    };

    // 1. Strict SemVer validation on package.version and RemoteDependency.uri
    for (tag, src) in [
        (
            "semver-pkg-leading-zero",
            "amends \"pkl:Project\"\npackage {\n  name = \"a\"\n  baseUri = \"package://example.com/a\"\n  version = \"01.2.3\"\n  packageZipUrl = \"https://example.com/a.zip\"\n}\n",
        ),
        (
            "semver-pkg-pre-zero",
            "amends \"pkl:Project\"\npackage {\n  name = \"a\"\n  baseUri = \"package://example.com/a\"\n  version = \"1.2.3-01\"\n  packageZipUrl = \"https://example.com/a.zip\"\n}\n",
        ),
        (
            "semver-dep-leading-zero",
            "amends \"pkl:Project\"\ndependencies {\n  [\"a\"] { uri = \"package://example.com/a@01.2.3\" }\n}\n",
        ),
        // 2. RemoteDependency.uri with #fragment or ?query
        (
            "dep-uri-fragment",
            "amends \"pkl:Project\"\ndependencies {\n  [\"a\"] { uri = \"package://example.com/a@1.2.3#/foo.pkl\" }\n}\n",
        ),
        (
            "dep-uri-query",
            "amends \"pkl:Project\"\ndependencies {\n  [\"a\"] { uri = \"package://example.com/a@1.2.3?q=1\" }\n}\n",
        ),
        // 3. Parenthesized new { ... }
        (
            "paren-new-tests",
            "amends \"pkl:Project\"\ntests = (new { \"a.pkl\" })\n",
        ),
        // 4. EmailAddress with vertical tab or Unicode line separator
        (
            "email-vtab",
            "amends \"pkl:Project\"\npackage {\n  name = \"a\"\n  baseUri = \"package://example.com/a\"\n  version = \"1.0.0\"\n  packageZipUrl = \"https://example.com/a.zip\"\n  authors { \"a@b\\u{0b}c\" }\n}\n",
        ),
        (
            "email-linesep",
            "amends \"pkl:Project\"\npackage {\n  name = \"a\"\n  baseUri = \"package://example.com/a\"\n  version = \"1.0.0\"\n  packageZipUrl = \"https://example.com/a.zip\"\n  authors { \"a\\u{2028}b@c.com\" }\n}\n",
        ),
        // 6. Malformed Java URIs in package fields
        (
            "uri-bad-zip-escape",
            "amends \"pkl:Project\"\npackage {\n  name = \"a\"\n  baseUri = \"package://example.com/a\"\n  version = \"1.0.0\"\n  packageZipUrl = \"https://example.com/%ZZ.zip\"\n}\n",
        ),
        (
            "uri-bad-website-bracket",
            "amends \"pkl:Project\"\npackage {\n  name = \"a\"\n  baseUri = \"package://example.com/a\"\n  version = \"1.0.0\"\n  packageZipUrl = \"https://example.com/a.zip\"\n  website = \"https://example.com/a[0]\"\n}\n",
        ),
        (
            "uri-bad-doc-double-frag",
            "amends \"pkl:Project\"\npackage {\n  name = \"a\"\n  baseUri = \"package://example.com/a\"\n  version = \"1.0.0\"\n  packageZipUrl = \"https://example.com/a.zip\"\n  documentation = \"https://example.com/a#b#c\"\n}\n",
        ),
    ] {
        let out = run_with_project(tag, src);
        assert_eq!(
            out.status.code(),
            Some(1),
            "case `{tag}` unexpectedly succeeded: stdout={}",
            String::from_utf8_lossy(&out.stdout)
        );
    }

    // 5. Local project import edge cases (percent-encoded PklProject and triple-dot)
    let dep_dir = tmp.join("dep_real");
    std::fs::create_dir_all(&dep_dir).unwrap();
    std::fs::write(
        dep_dir.join("PklProject"),
        "amends \"pkl:Project\"\npackage {\n  name = \"dep\"\n  baseUri = \"package://example.com/dep\"\n  version = \"1.0.0\"\n  packageZipUrl = \"https://example.com/dep.zip\"\n}\n",
    )
    .unwrap();
    let encoded_dep_uri = format!(
        "{}/Pkl%50roject",
        pkl_host_native::path_to_file_uri(&dep_dir)
            .to_java_utf8()
            .trim_end_matches('/')
    );
    let pct_out = run_with_project(
        "dep-percent-encoded",
        &format!(
            "amends \"pkl:Project\"\ndependencies {{\n  [\"dep\"] = import(\"{encoded_dep_uri}\")\n}}\n"
        ),
    );
    assert_eq!(
        pct_out.status.code(),
        Some(1),
        "percent-encoded Pkl%50roject import must fail: stdout={}",
        String::from_utf8_lossy(&pct_out.stdout)
    );

    // Windows NTFS strips trailing dots from directory names (cannot create a `...` directory).
    #[cfg(not(windows))]
    {
        let triple_proj = tmp.join("triple_proj");
        std::fs::create_dir_all(triple_proj.join("...")).unwrap();
        std::fs::write(
            triple_proj.join("...").join("PklProject"),
            "amends \"pkl:Project\"\npackage {\n  name = \"t\"\n  baseUri = \"package://example.com/t\"\n  version = \"1.0.0\"\n  packageZipUrl = \"https://example.com/t.zip\"\n}\n",
        )
        .unwrap();
        std::fs::write(
            triple_proj.join("PklProject"),
            "amends \"pkl:Project\"\ndependencies {\n  [\"t\"] = import(\".../PklProject\")\n}\n",
        )
        .unwrap();
        let triple_main = triple_proj.join("main.pkl");
        std::fs::write(&triple_main, "x = 1\n").unwrap();
        let triple_out = run_pkl(&[
            "eval",
            "--color=never",
            "--project-dir",
            triple_proj.to_str().unwrap(),
            triple_main.to_str().unwrap(),
        ]);
        assert_eq!(
            triple_out.status.code(),
            Some(1),
            "triple-dot .../PklProject import must fail: stdout={}",
            String::from_utf8_lossy(&triple_out.stdout)
        );
    }

    // 8. Non-Project PklProject single-evaluation error reporting (direct & non-direct & function & null)
    for (tag, src, expected_type) in [
        (
            "non-project-direct-int",
            "amends \"pkl:Project\"\noutput { value = 42 }\n",
            "Int",
        ),
        (
            "non-project-direct-fn",
            "amends \"pkl:Project\"\noutput { value = (x) -> x }\n",
            "Function1",
        ),
        (
            "non-project-direct-null",
            "amends \"pkl:Project\"\noutput { value = null }\n",
            "Null",
        ),
        ("non-project-plain-module", "x = 1\n", "PklProject"),
    ] {
        let out = run_with_project(tag, src);
        assert_eq!(out.status.code(), Some(1), "case `{tag}` must fail");
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(
            stderr.contains(&format!(
                "to be of type `pkl.Project`, but got type `{expected_type}`."
            )),
            "case `{tag}` unexpected stderr: {stderr}"
        );
    }

    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn wave1_cli_security_regressions_dangling_symlink_junit_traversal_and_utf8_flag() {
    let tmp = make_temp_dir("wave1-cli-sec");

    // 1. Non-ASCII short flag `-é` must return clean CLI usage error, never panic on `&raw[..2]`.
    let utf8_out = run_pkl(&["eval", "-é"]);
    assert_eq!(utf8_out.status.code(), Some(1));
    let utf8_stderr = String::from_utf8_lossy(&utf8_out.stderr);
    assert!(
        utf8_stderr.contains("no such option") || utf8_stderr.contains("Error"),
        "unexpected stderr for `-é`: {utf8_stderr}"
    );

    // 2. `pkl eval -m <dir>` must reject dangling symlink pointing outside output directory.
    #[cfg(unix)]
    {
        let out_dir = tmp.join("multi_out");
        let outside = tmp.join("outside_dir");
        std::fs::create_dir_all(&out_dir).unwrap();
        std::fs::create_dir_all(&outside).unwrap();
        let target_outside = outside.join("pwned.txt");
        std::os::unix::fs::symlink(&target_outside, out_dir.join("escape.txt")).unwrap();

        let mod_path = tmp.join("multi.pkl");
        std::fs::write(
            &mod_path,
            "output {\n  files {\n    [\"escape.txt\"] { text = \"pwned\" }\n  }\n}\n",
        )
        .unwrap();
        let m_out = run_pkl(&[
            "eval",
            "--no-project",
            "-m",
            out_dir.to_str().unwrap(),
            mod_path.to_str().unwrap(),
        ]);
        assert_eq!(
            m_out.status.code(),
            Some(1),
            "dangling symlink in -m output dir must fail"
        );
        assert!(
            !target_outside.exists(),
            "dangling symlink escape must never create file outside output dir"
        );
    }

    // 3. `pkl test --junit-reports <dir>` must reject module name path traversal.
    let junit_dir = tmp.join("junit");
    std::fs::create_dir_all(&junit_dir).unwrap();
    let bad_test = tmp.join("bad_junit.pkl");
    std::fs::write(
        &bad_test,
        "module `../escaped`\namends \"pkl:test\"\nfacts {\n  [\"ok\"] { true }\n}\n",
    )
    .unwrap();
    let j_out = run_pkl(&[
        "test",
        "--no-project",
        "--junit-reports",
        junit_dir.to_str().unwrap(),
        bad_test.to_str().unwrap(),
    ]);
    assert_eq!(
        j_out.status.code(),
        Some(1),
        "JUnit module-name traversal must fail"
    );
    assert!(
        !tmp.join("escaped.xml").exists(),
        "JUnit report must never escape junit-dir"
    );

    // 4. `pkl eval -o %{moduleName}.out` must reject percent-encoded `/` in moduleName.
    let tpl_out = run_pkl(&[
        "eval",
        "--no-project",
        "-o",
        &format!("{}/%{{moduleName}}.out", tmp.display()),
        "https://example.com/a%2F..%2Fescape.pkl",
    ]);
    assert_eq!(tpl_out.status.code(), Some(1));
    assert!(!tmp.join("escape.out").exists());

    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn round3_cli_security_dangling_test_symlink_double_placeholder_and_shell_completion() {
    let tmp = make_temp_dir("round3-cli-sec");

    // 1. P0-6: `pkl test` must refuse to write through a dangling `-actual.pcf` / `-expected.pcf` symlink.
    #[cfg(unix)]
    {
        let test_dir = tmp.join("test_sym");
        let outside = tmp.join("outside_test");
        std::fs::create_dir_all(&test_dir).unwrap();
        std::fs::create_dir_all(&outside).unwrap();
        let test_file = test_dir.join("sym_test.pkl");
        std::fs::write(
            &test_file,
            "amends \"pkl:test\"\nexamples {\n  [\"ex\"] {\n    123\n  }\n}\n",
        )
        .unwrap();
        let target_outside = outside.join("pwned_expected.pcf");
        let expected_link = test_dir.join("sym_test.pkl-expected.pcf");
        std::os::unix::fs::symlink(&target_outside, &expected_link).unwrap();

        // 1a. Without --overwrite, reading a dangling symlink expected.pcf fails cleanly.
        let out_no_ov = run_pkl(&["test", "--no-project", test_file.to_str().unwrap()]);
        assert_eq!(
            out_no_ov.status.code(),
            Some(1),
            "pkl test without --overwrite must fail on dangling symlink expected.pcf"
        );
        assert!(
            !target_outside.exists(),
            "pkl test must never create files outside test dir via dangling symlink"
        );

        // 1b. With --overwrite, unlinks the dangling symlink safely and writes a regular file (never following symlink).
        let out = run_pkl(&[
            "test",
            "--no-project",
            "--overwrite",
            test_file.to_str().unwrap(),
        ]);
        assert_eq!(
            out.status.code(),
            Some(10),
            "pkl test --overwrite should safely unlink dangling symlink and write new regular expected.pcf"
        );
        assert!(
            !target_outside.exists(),
            "pkl test --overwrite must never create files outside test dir via dangling symlink"
        );
        assert!(
            !std::fs::symlink_metadata(&expected_link)
                .expect("expected.pcf exists")
                .file_type()
                .is_symlink(),
            "overwritten expected.pcf must be a regular file, not a symlink"
        );
    }

    // 2. P0-4: `pkl eval -o` expands repeated placeholders without path escape.
    let mod_file = tmp.join("sample.pkl");
    std::fs::write(&mod_file, "x = 1\n").unwrap();
    let dbl_out = run_pkl(&[
        "eval",
        "--no-project",
        "-o",
        &format!("{}/%{{moduleName}}-%{{moduleName}}.pcf", tmp.display()),
        mod_file.to_str().unwrap(),
    ]);
    assert_eq!(
        dbl_out.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&dbl_out.stderr)
    );
    assert!(tmp.join("sample-sample.pcf").is_file());

    // 3. P0-5: `pkl run` shell completion escapes quotes & shell metacharacters safely.
    let cmd_file = tmp.join("evil_cmd.pkl");
    std::fs::write(
        &cmd_file,
        r#"amends "pkl:Command"
class Options {
  /// Evil 'desc' $(id) `uname`
  @Flag {
    shortName = "x"
    completionCandidates = List("ok", "a'$(id)`b")
  }
  mode: String = "ok"
}
command {
  name = "safe'$(id)"
  description = "Desc $(whoami) `uname`"
}
options: Options
output { text = options.mode }
"#,
    )
    .unwrap();
    for shell in ["bash", "zsh", "fish"] {
        let comp = run_pkl(&[
            "run",
            "--no-project",
            cmd_file.to_str().unwrap(),
            &format!("--generate-completion={shell}"),
        ]);
        if comp.status.code() == Some(0) {
            let script = String::from_utf8(comp.stdout).unwrap();
            assert!(
                !script.contains("$(id)`b\""),
                "unescaped command substitution in {shell} completion:\n{script}"
            );
        }
    }

    let _ = std::fs::remove_dir_all(&tmp);
}
