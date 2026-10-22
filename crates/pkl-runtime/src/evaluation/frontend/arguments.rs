//! Supported original module-eval argv profile, shared by all development adapters.
use super::{DiagnosticProfile, ModuleEvalOptions};

#[derive(Debug)]
pub struct ModuleEvalInvocation {
    pub options: ModuleEvalOptions,
    pub source_path: String,
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
    let mut source_path = None;
    let mut arguments = argv[1..].iter();
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--no-cache" => options.no_cache = true,
            "--no-project" => options.no_project = true,
            "--settings" => {
                options.settings = arguments
                    .next()
                    .ok_or("module-eval adapter requires a settings value")?
                    .clone();
            }
            "--property" | "--env-var" => {
                let value = arguments
                    .next()
                    .ok_or("module-eval adapter requires a key=value argument")?;
                let (key, value) = value
                    .split_once('=')
                    .ok_or("module-eval adapter requires a key=value argument")?;
                let values = if argument == "--property" {
                    &mut options.properties
                } else {
                    &mut options.environment
                };
                values.push((key.to_owned(), value.to_owned()));
            }
            value if value.starts_with('-') => {
                return Err(format!(
                    "module-eval adapter does not support option {value}"
                ));
            }
            value => {
                if source_path.replace(value.to_owned()).is_some() {
                    return Err("module-eval adapter requires exactly one source path".into());
                }
            }
        }
    }
    Ok(ModuleEvalInvocation {
        options,
        source_path: source_path.ok_or("module-eval adapter requires a source path")?,
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
            "--property",
            "name=second=tail",
            "--env-var",
            "foo bar=foo bar",
            "--env-var",
            "file:///foo/bar=file:///foo/bar",
            "external/pkl/input.pkl",
        ])
        .unwrap();
        assert_eq!(invocation.source_path, "external/pkl/input.pkl");
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
            &["eval", "a.pkl", "b.pkl"],
            &["eval", "--expression", "x", "a.pkl"],
            &["eval", "--settings"],
            &["eval", "--property", "missing-equals", "a.pkl"],
            &["eval", "--env-var"],
            &["test", "a.pkl"],
        ] {
            assert!(parse(args).is_err(), "{args:?}");
        }
        let invocation = parse(&["eval", "--settings", "custom.pkl", "a.pkl"]).unwrap();
        assert_eq!(invocation.options.settings, "custom.pkl");
        assert!(!invocation.options.no_cache && !invocation.options.no_project);
    }
}
