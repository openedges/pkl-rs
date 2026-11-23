//! `pkl repl` subcommand wiring `pkl_repl::run_cli_repl`.

use crate::env::{
    build_effective_settings, build_native_host_config, discover_optional_project,
    effective_working_dir, load_user_settings, resolve_use_color,
};
use crate::help::HELP_REPL;
use crate::options::{CliError, SubcommandKind, parse_options};
use pkl_repl::{ReplCliConfig, run_cli_repl};

pub fn cmd_repl(args: &[String]) -> Result<i32, CliError> {
    let (opts, positionals) = parse_options(SubcommandKind::Repl, args)?;
    if opts.show_help {
        crate::write_help(HELP_REPL);
        return Ok(0);
    }
    if !positionals.is_empty() {
        let msg = if positionals.len() == 1 {
            format!("got unexpected extra argument ({})", positionals[0])
        } else {
            format!("got unexpected extra arguments ({})", positionals.join(" "))
        };
        return Err(CliError::usage(SubcommandKind::Repl.usage_line(), &msg));
    }

    let working_dir = effective_working_dir(&opts)?;
    let host_env: Vec<(String, String)> = std::env::vars().collect();
    let user_settings = load_user_settings(&opts, &working_dir, &host_env)?;
    let project = discover_optional_project(&opts, &working_dir, &host_env)?;
    let (effective, effective_headers) = build_effective_settings(
        &opts,
        &working_dir,
        project.as_ref(),
        &user_settings,
        &host_env,
    );

    let use_color = resolve_use_color(effective.color);
    let host_config = build_native_host_config(
        &opts,
        &working_dir,
        project.as_ref(),
        &effective,
        effective_headers.as_ref(),
        &[],
    )?;

    Ok(run_cli_repl(ReplCliConfig {
        host_config,
        working_dir,
        output_format: opts.output_format,
        color: use_color,
    }))
}
