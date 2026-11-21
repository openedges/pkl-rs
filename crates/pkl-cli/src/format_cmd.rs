//! `pkl format` subcommand wiring `pkl_formatter::cli`.

use crate::help::HELP_FORMAT;
use crate::options::CliError;
use pkl_formatter::GrammarVersion;
use pkl_formatter::cli::{FormatCliOptions, run_cli as run_formatter_cli};
use std::io;
use std::path::PathBuf;

const USAGE_FORMAT: &str = "Usage: pkl format [<options>] [<paths>]...";

pub fn cmd_format(args: &[String]) -> Result<i32, CliError> {
    let mut opts = FormatCliOptions {
        paths: Vec::new(),
        grammar_version: GrammarVersion::V2,
        write: false,
        diff_name_only: false,
        silent: false,
    };

    let mut i = 0usize;
    let mut end_of_flags = false;
    while i < args.len() {
        let raw = &args[i];
        if end_of_flags || raw == "-" || !raw.starts_with('-') {
            opts.paths.push(PathBuf::from(raw));
            i += 1;
            continue;
        }
        if raw == "--" {
            end_of_flags = true;
            i += 1;
            continue;
        }

        let (flag, inline_val) = if raw.starts_with("--") {
            match raw.split_once('=') {
                Some((k, v)) => (k, Some(v)),
                None => (raw.as_str(), None),
            }
        } else {
            (raw.as_str(), None)
        };

        match flag {
            "-h" | "--help" => {
                print!("{HELP_FORMAT}");
                return Ok(0);
            }
            "-w" | "--write" => {
                if inline_val.is_some() {
                    return Err(CliError::usage(
                        USAGE_FORMAT,
                        &format!("option {flag} does not take a value"),
                    ));
                }
                opts.write = true;
            }
            "--diff-name-only" => {
                if inline_val.is_some() {
                    return Err(CliError::usage(
                        USAGE_FORMAT,
                        "option --diff-name-only does not take a value",
                    ));
                }
                opts.diff_name_only = true;
            }
            "-s" | "--silent" => {
                if inline_val.is_some() {
                    return Err(CliError::usage(
                        USAGE_FORMAT,
                        &format!("option {flag} does not take a value"),
                    ));
                }
                opts.silent = true;
            }
            "--grammar-version" => {
                let val = if let Some(v) = inline_val {
                    v.to_string()
                } else {
                    i += 1;
                    args.get(i).cloned().ok_or_else(|| {
                        CliError::usage(USAGE_FORMAT, "option --grammar-version requires a value")
                    })?
                };
                opts.grammar_version = match val.as_str() {
                    "1" => GrammarVersion::V1,
                    "2" => GrammarVersion::V2,
                    _ => {
                        return Err(CliError::usage(
                            USAGE_FORMAT,
                            &format!(
                                "invalid value for --grammar-version: invalid choice: {val}. (choose from 1, 2)"
                            ),
                        ));
                    }
                };
            }
            short if short.starts_with('-') && !short.starts_with("--") && short.len() > 2 => {
                // Bundled short flags like `-ws`, `-sw`, `-sh`
                for ch in short[1..].chars() {
                    match ch {
                        'h' => {
                            print!("{HELP_FORMAT}");
                            return Ok(0);
                        }
                        'w' => opts.write = true,
                        's' => opts.silent = true,
                        other => {
                            return Err(CliError::usage(
                                USAGE_FORMAT,
                                &format!("no such option -{other}"),
                            ));
                        }
                    }
                }
            }
            other => {
                return Err(CliError::usage(
                    USAGE_FORMAT,
                    &format!("no such option {other}"),
                ));
            }
        }
        i += 1;
    }

    let stdin = io::stdin();
    let stdout = io::stdout();
    let stderr = io::stderr();
    let mut stdin_lock = stdin.lock();
    let mut stdout_lock = stdout.lock();
    let mut stderr_lock = stderr.lock();
    Ok(run_formatter_cli(
        &opts,
        &mut stdin_lock,
        &mut stdout_lock,
        &mut stderr_lock,
    ))
}
