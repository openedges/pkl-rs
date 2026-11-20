//! Supported original module-eval argv profile, shared by all development adapters.
use super::{DiagnosticProfile, ModuleEvalOptions};

#[derive(Debug)]
pub struct ModuleEvalInvocation {
    pub options: ModuleEvalOptions,
    pub project_dir: Option<String>,
    pub source_path: String,
    pub source_paths: Vec<String>,
    pub output_format: Option<String>,
    pub output_path: Option<String>,
    pub multiple_file_output_path: Option<String>,
    pub expression: Option<String>,
    pub module_output_separator: String,
}

impl ModuleEvalInvocation {
    /// Enforce upstream `EvalCommand` mutual-exclusion rules (`EvalCommand.kt:67-71`).
    pub fn validate_cli(&self) -> Result<(), String> {
        if self.multiple_file_output_path.is_some()
            && (self.output_path.is_some() || self.expression.is_some())
        {
            return Err(
                "Usage: pkl eval [<options>] <modules>...\n\nError: invalid value for --multiple-file-output-path: Option is mutually exclusive with -o, --output-path and -x, --expression."
                    .into(),
            );
        }
        Ok(())
    }
}

/// Parse the bounded CLI profile without changing source arguments or map values.
/// Unsupported options remain adapter errors rather than invented Pkl diagnostics.
pub fn parse_module_eval_args(argv: &[String]) -> Result<ModuleEvalInvocation, String> {
    if argv.first().map(String::as_str) != Some("eval") {
        return Err("module-eval adapter requires the eval command".into());
    }
    let mut options = ModuleEvalOptions {
        no_cache: false,
        no_project: false,
        diagnostic_profile: DiagnosticProfile::CliBundledSettings,
        ..ModuleEvalOptions::default()
    };
    let mut project_dir = None;
    let mut source_paths = Vec::new();
    let mut output_format: Option<String> = None;
    let mut output_path: Option<String> = None;
    let mut multiple_file_output_path: Option<String> = None;
    let mut expression: Option<String> = None;
    let mut module_output_separator = "---".to_owned();
    let mut arguments = argv[1..].iter();
    let mut end_of_options = false;

    while let Some(argument) = arguments.next() {
        if end_of_options {
            source_paths.push(argument.clone());
            continue;
        }
        let arg = argument.as_str();
        if arg == "--" {
            end_of_options = true;
            continue;
        }
        if let Some(dir) = arg.strip_prefix("--project-dir=") {
            options.no_project = false;
            project_dir = Some(dir.to_owned());
            continue;
        }
        if let Some(settings) = arg.strip_prefix("--settings=") {
            options.settings = settings.to_owned();
            continue;
        }
        if let Some(pair) = arg.strip_prefix("--property=") {
            let (key, value) = pair
                .split_once('=')
                .ok_or("module-eval adapter requires a key=value argument")?;
            options.properties.push((key.to_owned(), value.to_owned()));
            continue;
        }
        if let Some(pair) = arg.strip_prefix("--env-var=") {
            let (key, value) = pair
                .split_once('=')
                .ok_or("module-eval adapter requires a key=value argument")?;
            options.environment.push((key.to_owned(), value.to_owned()));
            continue;
        }
        if let Some(fmt) = arg.strip_prefix("--format=") {
            output_format = Some(fmt.to_owned());
            continue;
        }
        if let Some(path) = arg.strip_prefix("--output-path=") {
            output_path = Some(path.to_owned());
            continue;
        }
        if let Some(path) = arg.strip_prefix("--multiple-file-output-path=") {
            multiple_file_output_path = Some(path.to_owned());
            continue;
        }
        if let Some(expr) = arg.strip_prefix("--expression=") {
            expression = Some(expr.to_owned());
            continue;
        }
        if let Some(sep) = arg.strip_prefix("--module-output-separator=") {
            module_output_separator = sep.to_owned();
            continue;
        }

        match arg {
            "--no-cache" => options.no_cache = true,
            "--no-project" => options.no_project = true,
            "--project-dir" => {
                let dir = arguments
                    .next()
                    .ok_or("module-eval adapter requires a project-dir value")?
                    .clone();
                options.no_project = false;
                project_dir = Some(dir);
            }
            // The bounded diagnostic renderer always emits plain text. Keep
            // this original explicit spelling rather than rewriting argv.
            "--color=never" => {}
            "--color" => {
                let mode = arguments
                    .next()
                    .ok_or("module-eval adapter requires a color value")?;
                if mode != "never" {
                    return Err(format!(
                        "module-eval adapter does not support option --color={mode}"
                    ));
                }
            }
            "--settings" => {
                options.settings = arguments
                    .next()
                    .ok_or("module-eval adapter requires a settings value")?
                    .clone();
            }
            "-p" | "--property" => {
                let value = arguments
                    .next()
                    .ok_or("module-eval adapter requires a key=value argument")?;
                let (key, value) = value
                    .split_once('=')
                    .ok_or("module-eval adapter requires a key=value argument")?;
                options.properties.push((key.to_owned(), value.to_owned()));
            }
            "-e" | "--env-var" => {
                let value = arguments
                    .next()
                    .ok_or("module-eval adapter requires a key=value argument")?;
                let (key, value) = value
                    .split_once('=')
                    .ok_or("module-eval adapter requires a key=value argument")?;
                options.environment.push((key.to_owned(), value.to_owned()));
            }
            "-f" | "--format" => {
                output_format = Some(
                    arguments
                        .next()
                        .ok_or("module-eval adapter requires a format value")?
                        .clone(),
                );
            }
            "-o" | "--output-path" => {
                output_path = Some(
                    arguments
                        .next()
                        .ok_or("module-eval adapter requires an output-path value")?
                        .clone(),
                );
            }
            "-m" | "--multiple-file-output-path" => {
                multiple_file_output_path = Some(
                    arguments
                        .next()
                        .ok_or("module-eval adapter requires a multiple-file-output-path value")?
                        .clone(),
                );
            }
            "-x" | "--expression" => {
                expression = Some(
                    arguments
                        .next()
                        .ok_or("module-eval adapter requires an expression value")?
                        .clone(),
                );
            }
            "--module-output-separator" => {
                module_output_separator = arguments
                    .next()
                    .ok_or("module-eval adapter requires a module-output-separator value")?
                    .clone();
            }
            value if value.starts_with('-') => {
                return Err(format!(
                    "module-eval adapter does not support option {value}"
                ));
            }
            value => {
                source_paths.push(value.to_owned());
            }
        }
    }

    // Upstream `VmContext.java:91-94`: `--format`/`-f` is injected as external
    // property `pkl.outputFormat`, overriding any `-p pkl.outputFormat=...`.
    if let Some(format) = &output_format {
        options.properties.retain(|(k, _)| k != "pkl.outputFormat");
        options
            .properties
            .push(("pkl.outputFormat".into(), format.clone()));
    }

    let source_path = source_paths
        .first()
        .cloned()
        .ok_or("module-eval adapter requires a source path")?;
    Ok(ModuleEvalInvocation {
        options,
        project_dir,
        source_path,
        source_paths,
        output_format,
        output_path,
        multiple_file_output_path,
        expression,
        module_output_separator,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<ModuleEvalInvocation, String> {
        parse_module_eval_args(&args.iter().map(|x| (*x).to_owned()).collect::<Vec<_>>())
    }

    #[test]
    fn original_maps_preserve_order_duplicates_and_only_split_first_equals() {
        let invocation = parse(&[
            "eval",
            "--no-cache",
            "--no-project",
            "--settings",
            "pkl:settings",
            "--property",
            "name=first",
            "-p",
            "name=second=tail",
            "--env-var",
            "foo bar=foo bar",
            "-e",
            "file:///foo/bar=file:///foo/bar",
            "external/pkl/input.pkl",
        ])
        .unwrap();
        assert_eq!(invocation.source_path, "external/pkl/input.pkl");
        assert_eq!(invocation.source_paths, ["external/pkl/input.pkl"]);
        assert!(invocation.options.no_cache && invocation.options.no_project);
        assert_eq!(
            invocation.options.properties,
            [
                ("name".into(), "first".into()),
                ("name".into(), "second=tail".into())
            ]
        );
        assert_eq!(
            invocation.options.environment,
            [
                ("foo bar".into(), "foo bar".into()),
                ("file:///foo/bar".into(), "file:///foo/bar".into())
            ]
        );
    }

    #[test]
    fn unsupported_and_incomplete_invocations_do_not_disappear() {
        for args in [
            &["eval"][..],
            &["eval", "--color=always", "a.pkl"],
            &["eval", "--color=auto", "a.pkl"],
            &["eval", "--settings"],
            &["eval", "--property", "missing-equals", "a.pkl"],
            &["eval", "-p", "missing-equals", "a.pkl"],
            &["eval", "--env-var"],
            &["eval", "-e"],
            &["eval", "-f"],
            &["eval", "-m"],
            &["eval", "-o"],
            &["eval", "-x"],
            &["eval", "--unknown-flag", "a.pkl"],
            &["test", "a.pkl"],
        ] {
            assert!(parse(args).is_err(), "{args:?}");
        }
        let invocation = parse(&["eval", "--settings", "custom.pkl", "a.pkl"]).unwrap();
        assert_eq!(invocation.options.settings, "custom.pkl");
        assert!(!invocation.options.no_cache && !invocation.options.no_project);
    }

    #[test]
    fn format_flag_injects_and_overrides_pkl_output_format_property_in_both_orders() {
        for args in [
            &["eval", "-p", "pkl.outputFormat=yaml", "-f", "json", "a.pkl"][..],
            &[
                "eval",
                "--format=json",
                "--property",
                "pkl.outputFormat=yaml",
                "a.pkl",
            ][..],
        ] {
            let invocation = parse(args).unwrap();
            assert_eq!(invocation.output_format.as_deref(), Some("json"));
            assert_eq!(
                invocation.options.properties,
                [("pkl.outputFormat".into(), "json".into())]
            );
        }
    }

    #[test]
    fn multi_file_output_expression_output_path_and_multi_module_flags_parse() {
        let multi = parse(&[
            "eval",
            "--no-project",
            "-m",
            "out/dir",
            "--format",
            "yaml",
            "b.pkl",
            "a.pkl",
        ])
        .unwrap();
        assert_eq!(multi.multiple_file_output_path.as_deref(), Some("out/dir"));
        assert_eq!(multi.output_format.as_deref(), Some("yaml"));
        assert_eq!(multi.source_path, "b.pkl");
        assert_eq!(multi.source_paths, ["b.pkl", "a.pkl"]);
        assert!(multi.validate_cli().is_ok());

        let expr = parse(&[
            "eval",
            "-x",
            "foo.bar",
            "-o",
            "out.txt",
            "--module-output-separator=//sep",
            "a.pkl",
            "b.pkl",
        ])
        .unwrap();
        assert_eq!(expr.expression.as_deref(), Some("foo.bar"));
        assert_eq!(expr.output_path.as_deref(), Some("out.txt"));
        assert_eq!(expr.module_output_separator, "//sep");
        assert!(expr.validate_cli().is_ok());

        let conflict_o = parse(&["eval", "-m", "out", "-o", "f.txt", "a.pkl"]).unwrap();
        assert!(conflict_o.validate_cli().is_err());

        let conflict_x = parse(&[
            "eval",
            "--multiple-file-output-path=out",
            "-x",
            "x",
            "a.pkl",
        ])
        .unwrap();
        assert!(conflict_x.validate_cli().is_err());
    }

    #[test]
    fn original_plain_t40_argv_preserves_enabled_package_cache_setting() {
        let invocation = parse(&[
            "eval",
            "--settings",
            "pkl:settings",
            "--color=never",
            "--no-project",
            "tests/fixtures/rev1/class_authority/t40/repro.pkl",
        ])
        .unwrap();
        assert!(!invocation.options.no_cache);
        assert!(invocation.options.no_project);
        assert_eq!(invocation.options.settings, "pkl:settings");
        assert_eq!(
            invocation.source_path,
            "tests/fixtures/rev1/class_authority/t40/repro.pkl"
        );
    }
}
