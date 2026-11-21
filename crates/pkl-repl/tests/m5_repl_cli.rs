#![forbid(unsafe_code)]

use pkl_host_native::NativeHostConfig;
use pkl_repl::{ReplCliConfig, run_repl_io};
use pkl_runtime::logging::TraceMode;
use std::io::Cursor;
use std::path::PathBuf;

fn run_pipe(config: ReplCliConfig, lines: &[&str]) -> (i32, String, String) {
    let input_text = lines.concat();
    let input = Cursor::new(input_text.into_bytes());
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let code = run_repl_io(config, input, &mut stdout, &mut stderr, false);
    (
        code,
        String::from_utf8(stdout).expect("utf8 stdout"),
        String::from_utf8(stderr).expect("utf8 stderr"),
    )
}

fn default_config(color: bool) -> ReplCliConfig {
    ReplCliConfig {
        host_config: NativeHostConfig::default(),
        working_dir: PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/m5/repl"),
        output_format: None,
        color,
    }
}

#[test]
fn pipe_command_prefixes_unknown_and_help() {
    let (code, out, err) = run_pipe(default_config(false), &[":h\n", ":bogus\n", ":q\n"]);
    assert_eq!(code, 0);
    assert!(err.is_empty(), "stderr: {err}");
    assert!(out.starts_with("Welcome to Pkl 0.32.0.\nType an expression to have it evaluated.\nType \x1b[1m:help\x1b[22m or \x1b[1m:examples\x1b[22m for more information.\n\npkl0> "));
    assert!(out.contains("Multi-line programs can be copy-pasted into the REPL.\n\npkl0> Unknown command: \x1b[1mbogus\x1b[22m\npkl0> "));
}

#[test]
fn pipe_error_preamble_and_recovery() {
    let (code, out, err) = run_pipe(default_config(false), &["foo\n", "1 + 2\n", ":quit\n"]);
    assert_eq!(code, 0);
    assert!(err.is_empty(), "stderr: {err}");
    assert_eq!(
        out,
        "Welcome to Pkl 0.32.0.\nType an expression to have it evaluated.\nType \x1b[1m:help\x1b[22m or \x1b[1m:examples\x1b[22m for more information.\n\npkl0> –– Pkl Error ––\nCannot find property \x1b[1mfoo\x1b[22m.\n\n1 | foo\n    ^^^\nat  (repl:pkl0)\n\npkl1> 3\npkl2> "
    );
}

#[test]
fn pipe_props_env_and_pretty_trace_stderr() {
    let mut cfg = default_config(false);
    cfg.host_config.properties = vec![("who".into(), "repl".into())];
    cfg.host_config.environment = vec![("GREET".into(), "hello".into())];
    cfg.host_config.trace_mode = TraceMode::Pretty;
    let (code, out, err) = run_pipe(
        cfg,
        &[
            "read(\"prop:who\")\n",
            "read(\"env:GREET\")\n",
            "trace(new Dynamic { a = 1; b = 2 })\n",
            ":quit\n",
        ],
    );
    assert_eq!(code, 0);
    assert_eq!(
        err,
        "pkl: TRACE: new Dynamic { a = 1; b = 2 } =\n  new Dynamic {\n    a = 1\n    b = 2\n  }\n(repl:pkl2)\n"
    );
    assert_eq!(
        out,
        "Welcome to Pkl 0.32.0.\nType an expression to have it evaluated.\nType \x1b[1m:help\x1b[22m or \x1b[1m:examples\x1b[22m for more information.\n\npkl0> \"repl\"\npkl1> \"hello\"\npkl2> new Dynamic {\n  a = 1\n  b = 2\n}\npkl3> "
    );
}

#[test]
fn pipe_color_always_highlights_values_and_errors_without_faint_prompt() {
    let (code, out, err) = run_pipe(
        default_config(true),
        &["5.ms\n", "\"hello\"\n", "foo\n", ":quit\n"],
    );
    assert_eq!(code, 0);
    assert!(err.is_empty(), "stderr: {err}");
    assert_eq!(
        out,
        "Welcome to Pkl 0.32.0.\nType an expression to have it evaluated.\nType \x1b[1m:help\x1b[22m or \x1b[1m:examples\x1b[22m for more information.\n\npkl0> \x1b[32m5\x1b[0m.ms\npkl1> \x1b[33m\"hello\"\x1b[0m\npkl2> \x1b[31m–– Pkl Error ––\x1b[0m\n\x1b[1;31mCannot find property \x1b[1mfoo\x1b[22m.\x1b[0m\n\n\x1b[33m\x1b[0m\x1b[2m1 | \x1b[0mfoo\n\x1b[33m\x1b[0m    \x1b[31m^^^\x1b[0m\n\x1b[33m\x1b[0m\x1b[2mat  (repl:pkl2\x1b[0m\x1b[2m)\n\x1b[0m\n\npkl3> "
    );
}

#[test]
fn pipe_load_clear_reset_and_eof() {
    let (code, out, err) = run_pipe(
        default_config(false),
        &[
            ":load load_sample.pkl\n",
            ":clear\n",
            "loadedGreeting\n",
            "doubleIt(loadedCount)\n",
            ":reset\n",
            "loadedGreeting\n",
        ],
    );
    assert_eq!(code, 0);
    assert!(err.is_empty(), "stderr: {err}");
    assert_eq!(
        out,
        "Welcome to Pkl 0.32.0.\nType an expression to have it evaluated.\nType \x1b[1m:help\x1b[22m or \x1b[1m:examples\x1b[22m for more information.\n\npkl0> pkl1> pkl1> \"hello from file\"\npkl2> 198\npkl3> pkl0> –– Pkl Error ––\nCannot find property \x1b[1mloadedGreeting\x1b[22m.\n\n1 | loadedGreeting\n    ^^^^^^^^^^^^^^\nat  (repl:pkl0)\n\npkl1> "
    );
}

#[test]
fn pipe_lazy_force_and_unclosed_brace_recovery() {
    let (code, out, err) = run_pipe(
        default_config(false),
        &[
            "obj = new Dynamic { a = 1; b = a + 2 }\n",
            "obj\n",
            ":force obj\n",
            "broken = new {\n",
            "1 + 2\n",
            ":quit\n",
        ],
    );
    assert_eq!(code, 0);
    assert!(err.is_empty(), "stderr: {err}");
    assert_eq!(
        out,
        "Welcome to Pkl 0.32.0.\nType an expression to have it evaluated.\nType \x1b[1m:help\x1b[22m or \x1b[1m:examples\x1b[22m for more information.\n\npkl0> pkl1> new Dynamic {\n  a = ?\n  b = ?\n}\npkl2> new Dynamic {\n  a = 1\n  b = 3\n}\npkl3> –– Pkl Error ––\nMissing \x1b[1m}\x1b[22m delimiter.\n\n1 | broken = new {\n    ^\nat repl:pkl3 (repl:pkl3)\n\npkl4> 3\npkl5> "
    );
}
