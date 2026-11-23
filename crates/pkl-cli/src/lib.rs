//! Product `pkl` CLI library for Apple Pkl 0.32.0.
//!
//! Dispatches all Apple Pkl 0.32.0 subcommands (`eval`, `repl`, `server`, `test`,
//! `project resolve`, `project package`, `download-package`, `analyze imports`,
//! `format`, `run`, `shell-completion`) with byte-exact help, errors, and honest
//! native-flavor `--version` output.

mod env;
mod eval_cmd;
mod format_cmd;
mod help;
mod options;
mod repl_cmd;
mod run_cmd;
mod test_cmd;

use crate::help::{
    COMPLETION_BASH, COMPLETION_FISH, COMPLETION_ZSH, HELP_ANALYZE, HELP_PROJECT, HELP_ROOT,
    HELP_SERVER, HELP_SHELL_COMPLETION,
};
use crate::options::CliError;
use std::io::{self, Write};

const ROOT_USAGE: &str = "Usage: pkl [<options>] <command> [<args>]...";
const ROOT_SUBCOMMANDS: &str =
    "eval, repl, server, test, project, download-package, analyze, format, run, shell-completion";

/// Main entry point for the `pkl` product CLI binary (arguments after `argv[0]`).
pub fn run_cli(args: &[String]) -> i32 {
    let outcome = dispatch(args);
    match outcome {
        Ok(code) => code,
        Err(CliError::Silent(code)) => code,
        Err(CliError::Message { text, exit_code }) => {
            let mut stderr = io::stderr().lock();
            let _ = stderr.write_all(text.as_bytes());
            if !text.ends_with('\n') {
                let _ = stderr.write_all(b"\n");
            }
            let _ = stderr.flush();
            exit_code
        }
    }
}

fn dispatch(args: &[String]) -> Result<i32, CliError> {
    let Some(first) = args.first().map(String::as_str) else {
        write_help(HELP_ROOT);
        return Ok(0);
    };

    match first {
        "-v" | "--version" => {
            let banner = pkl::Release::current().version_info();
            write_stdout(&format!("{banner}\n"));
            Ok(0)
        }
        "-h" | "--help" => {
            write_help(HELP_ROOT);
            Ok(0)
        }
        "eval" => eval_cmd::cmd_eval(&args[1..]),
        "repl" => repl_cmd::cmd_repl(&args[1..]),
        "server" => cmd_server(&args[1..]),
        "test" => test_cmd::cmd_test(&args[1..]),
        "project" => cmd_project(&args[1..]),
        "download-package" => eval_cmd::cmd_download_package(&args[1..]),
        "analyze" => cmd_analyze(&args[1..]),
        "format" => format_cmd::cmd_format(&args[1..]),
        "run" => run_cmd::cmd_run(&args[1..]),
        "shell-completion" => cmd_shell_completion(&args[1..]),
        other if other.starts_with('-') => Err(CliError::usage(
            ROOT_USAGE,
            &format!("no such option {other}. (Possible options: -v, --version, -h, --help)"),
        )),
        other => Err(CliError::usage(
            ROOT_USAGE,
            &format!("no such subcommand {other}. (Possible subcommands: {ROOT_SUBCOMMANDS})"),
        )),
    }
}

fn cmd_server(args: &[String]) -> Result<i32, CliError> {
    const USAGE: &str = "Usage: pkl server [<options>]";
    if args.iter().any(|a| a == "-h" || a == "--help") {
        write_help(HELP_SERVER);
        return Ok(0);
    }
    if let Some(arg) = args.first() {
        return if arg.starts_with('-') {
            Err(CliError::usage(USAGE, &format!("no such option {arg}")))
        } else {
            Err(CliError::usage(
                USAGE,
                &format!("got unexpected extra argument ({arg})"),
            ))
        };
    }
    Ok(pkl_server::run_cli_server())
}

fn cmd_project(args: &[String]) -> Result<i32, CliError> {
    const USAGE: &str = "Usage: pkl project [<options>] <command> [<args>]...";
    let Some(first) = args.first().map(String::as_str) else {
        write_help(HELP_PROJECT);
        return Ok(0);
    };
    match first {
        "-h" | "--help" => {
            write_help(HELP_PROJECT);
            Ok(0)
        }
        "resolve" => eval_cmd::cmd_project_resolve(&args[1..]),
        "package" => eval_cmd::cmd_project_package(&args[1..]),
        other if other.starts_with('-') => {
            Err(CliError::usage(USAGE, &format!("no such option {other}")))
        }
        other => Err(CliError::usage(
            USAGE,
            &format!("no such subcommand {other}. (Possible subcommands: resolve, package)"),
        )),
    }
}

fn cmd_analyze(args: &[String]) -> Result<i32, CliError> {
    const USAGE: &str = "Usage: pkl analyze [<options>] <command> [<args>]...";
    let Some(first) = args.first().map(String::as_str) else {
        write_help(HELP_ANALYZE);
        return Ok(0);
    };
    match first {
        "-h" | "--help" => {
            write_help(HELP_ANALYZE);
            Ok(0)
        }
        "imports" => eval_cmd::cmd_analyze_imports(&args[1..]),
        other if other.starts_with('-') => {
            Err(CliError::usage(USAGE, &format!("no such option {other}")))
        }
        other => Err(CliError::usage(
            USAGE,
            &format!("no such subcommand {other}. (Possible subcommands: imports)"),
        )),
    }
}

fn cmd_shell_completion(args: &[String]) -> Result<i32, CliError> {
    const USAGE: &str = "Usage: pkl shell-completion [<options>] <shell>";
    let mut positionals = Vec::new();
    let mut end_of_flags = false;
    for arg in args {
        if !end_of_flags && arg == "--" {
            end_of_flags = true;
            continue;
        }
        if !end_of_flags && (arg == "-h" || arg == "--help") {
            write_help(HELP_SHELL_COMPLETION);
            return Ok(0);
        }
        if !end_of_flags && arg.starts_with('-') && arg != "-" {
            return Err(CliError::usage(USAGE, &format!("no such option {arg}")));
        }
        positionals.push(arg.as_str());
    }
    let Some(&shell) = positionals.first() else {
        return Err(CliError::usage(USAGE, "missing argument <shell>"));
    };
    if positionals.len() > 1 {
        let extra = &positionals[1..];
        let msg = if extra.len() == 1 {
            format!("got unexpected extra argument ({})", extra[0])
        } else {
            format!("got unexpected extra arguments ({})", extra.join(" "))
        };
        return Err(CliError::usage(USAGE, &msg));
    }
    match shell {
        "bash" => {
            write_stdout(COMPLETION_BASH);
            Ok(0)
        }
        "zsh" => {
            write_stdout(COMPLETION_ZSH);
            Ok(0)
        }
        "fish" => {
            write_stdout(COMPLETION_FISH);
            Ok(0)
        }
        other => Err(CliError::usage(
            USAGE,
            &format!(
                "invalid value for <shell>: invalid choice: {other}. (choose from bash, zsh, fish)"
            ),
        )),
    }
}

pub(crate) fn write_help(plain_help: &str) {
    let level = pkl_runtime::diagnostic::mordant::detect_ansi_level();
    let rendered = pkl_runtime::diagnostic::mordant::colorize_clikt_help(plain_help, level);
    write_stdout(&rendered);
}

fn write_stdout(text: &str) {
    let mut stdout = io::stdout().lock();
    let _ = stdout.write_all(text.as_bytes());
    let _ = stdout.flush();
}
