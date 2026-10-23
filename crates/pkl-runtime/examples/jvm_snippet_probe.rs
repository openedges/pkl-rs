//! Development JVM snippet profile using the shared Rust evaluator and Host.
use pkl_runtime::evaluation::frontend::{
    DiagnosticProfile, ModuleEvalOptions, ModuleSourceInput, evaluate_module_output_with_host,
};
use std::{io::Write, path::Path};

#[path = "support/classpath_host.rs"]
mod classpath_host;
#[path = "support/module_host.rs"]
mod module_host;

fn run(arguments: &[String]) -> Result<i32, String> {
    let [source_path, classpath_root] = arguments else {
        return Err("jvm-snippet adapter requires source-path and classpath-root".into());
    };
    let path = std::path::absolute(source_path).map_err(|error| error.to_string())?;
    let text = std::fs::read_to_string(&path).map_err(|error| error.to_string())?;
    let uri = pkl_runtime::modules::original_file_uri(
        path.to_str()
            .ok_or("jvm-snippet adapter requires a UTF-8 source path")?,
    )
    .map_err(|error| format!("{error:?}"))?;
    let inferred_name = path
        .file_stem()
        .and_then(|value| value.to_str())
        .ok_or("jvm-snippet adapter requires a UTF-8 module name")?;
    let host = classpath_host::DirectoryClassPath::new(Path::new(classpath_root))
        .map_err(|error| format!("{error:?}"))?;
    let pairs = |items: &[(&str, &str)]| {
        items
            .iter()
            .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
            .collect()
    };
    // LanguageSnippetTestsEngine's explicit maps do not inherit process env.
    let options = ModuleEvalOptions {
        diagnostic_profile: DiagnosticProfile::ApiIdentity,
        environment: pairs(&[
            ("NAME1", "value1"),
            ("NAME2", "value2"),
            ("/foo/bar", "foobar"),
            ("foo bar", "foo bar"),
            ("file:///foo/bar", "file:///foo/bar"),
        ]),
        properties: pairs(&[
            ("name1", "value1"),
            ("name2", "value2"),
            ("/foo/bar", "foobar"),
        ]),
        ..ModuleEvalOptions::default()
    };
    let result = evaluate_module_output_with_host(
        ModuleSourceInput {
            text: &text,
            uri: &uri,
            inferred_name,
        },
        &options,
        |request| host.complete(request),
    );
    std::io::stdout()
        .write_all(&result.stdout)
        .map_err(|error| error.to_string())?;
    std::io::stderr()
        .write_all(&result.stderr)
        .map_err(|error| error.to_string())?;
    Ok(result.exit_code)
}

fn main() {
    let arguments = std::env::args_os()
        .skip(1)
        .map(|argument| {
            argument
                .into_string()
                .map_err(|_| "jvm-snippet adapter requires UTF-8 arguments".to_owned())
        })
        .collect::<Result<Vec<_>, _>>();
    match arguments.and_then(|arguments| run(&arguments)) {
        Ok(code) => std::process::exit(code),
        Err(error) => {
            let _ = writeln!(
                std::io::stderr(),
                "jvm-snippet adapter internal failure: {error}"
            );
            std::process::exit(4);
        }
    }
}
