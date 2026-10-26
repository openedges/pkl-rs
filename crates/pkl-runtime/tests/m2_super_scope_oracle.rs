//! Stage89 CLI oracle controls. Every expected byte comes from two pinned Java invocations.
#[path = "support/module_keys.rs"]
mod module_keys;
use pkl_runtime::evaluation::frontend::{
    DiagnosticProfile, ModuleEvalOptions, ModuleSourceInput, evaluate_module_output_with_host,
};
use pkl_runtime::modules::{ModuleCompletion, ModuleRequestKind, ModuleSource, ResolvedModule};

struct Expected<'a> {
    uri: &'a str,
    stdout: &'a [u8],
    stderr: &'a [u8],
    exit: &'a str,
}

fn check(id: &str, source: &str, base: Option<&str>, repeats: &[Expected<'_>; 2]) {
    let options = ModuleEvalOptions {
        no_cache: true,
        no_project: true,
        settings: "pkl:settings".into(),
        properties: Vec::new(),
        environment: Vec::new(),
        diagnostic_profile: DiagnosticProfile::CliBundledSettings,
    };
    for (attempt, expected) in repeats.iter().enumerate() {
        let result = evaluate_module_output_with_host(
            ModuleSourceInput {
                text: source,
                uri: expected.uri,
                inferred_name: "main",
            },
            &options,
            |request| {
                if let Some(completion) = module_keys::defaults(request) {
                    return completion;
                }
                match &request.kind {
                    ModuleRequestKind::Resolve { original_uri, .. } => {
                        ModuleCompletion::Resolved(Ok(ResolvedModule {
                            original_uri: original_uri.clone(),
                            resolved_uri: original_uri.clone(),
                            source_uri: original_uri.clone(),
                        }))
                    }
                    ModuleRequestKind::LoadSource { resolved, .. } => {
                        assert_eq!(resolved.original_uri.rsplit('/').next(), Some("base.pkl"));
                        ModuleCompletion::Source(Ok(ModuleSource {
                            text: base.expect("authored base module").into(),
                            inferred_name: "base".into(),
                        }))
                    }
                    _ => panic!("unexpected module request: {:?}", request.kind),
                }
            },
        );
        assert_eq!(
            result.exit_code,
            expected.exit.parse::<i32>().unwrap(),
            "{id} repeat {attempt} exit"
        );
        assert_eq!(
            result.stdout, expected.stdout,
            "{id} repeat {attempt} complete stdout"
        );
        assert_eq!(
            result.stderr, expected.stderr,
            "{id} repeat {attempt} complete stderr"
        );
    }
}

macro_rules! cli_case {
    ($test:ident, $id:literal, $base:expr) => {
        #[test]
        fn $test() {
            check(
                $id,
                include_str!(concat!(
                    "../../../tests/fixtures/m2-super-scope-oracle/",
                    $id,
                    "/main.pkl"
                )),
                $base,
                &[
                    Expected {
                        uri: include_str!(concat!(
                            "../../../tests/fixtures/m2-super-scope-oracle/",
                            $id,
                            "/repeat-0/uri.txt"
                        )),
                        stdout: include_bytes!(concat!(
                            "../../../tests/fixtures/m2-super-scope-oracle/",
                            $id,
                            "/repeat-0/stdout.bin"
                        )),
                        stderr: include_bytes!(concat!(
                            "../../../tests/fixtures/m2-super-scope-oracle/",
                            $id,
                            "/repeat-0/stderr.bin"
                        )),
                        exit: include_str!(concat!(
                            "../../../tests/fixtures/m2-super-scope-oracle/",
                            $id,
                            "/repeat-0/exit.txt"
                        )),
                    },
                    Expected {
                        uri: include_str!(concat!(
                            "../../../tests/fixtures/m2-super-scope-oracle/",
                            $id,
                            "/repeat-1/uri.txt"
                        )),
                        stdout: include_bytes!(concat!(
                            "../../../tests/fixtures/m2-super-scope-oracle/",
                            $id,
                            "/repeat-1/stdout.bin"
                        )),
                        stderr: include_bytes!(concat!(
                            "../../../tests/fixtures/m2-super-scope-oracle/",
                            $id,
                            "/repeat-1/stderr.bin"
                        )),
                        exit: include_str!(concat!(
                            "../../../tests/fixtures/m2-super-scope-oracle/",
                            $id,
                            "/repeat-1/exit.txt"
                        )),
                    },
                ],
            );
        }
    };
}

cli_case!(
    amends_module_forbidden,
    "amends-module-forbidden",
    Some(include_str!(
        "../../../tests/fixtures/m2-super-scope-oracle/amends-module-forbidden/base.pkl"
    ))
);
cli_case!(nested_object_forbidden, "nested-object-forbidden", None);
cli_case!(class_lambda_allowed, "class-lambda-allowed", None);
cli_case!(object_lambda_forbidden, "object-lambda-forbidden", None);
cli_case!(object_super_property, "object-super-property", None);
cli_case!(object_super_entry, "object-super-entry", None);
cli_case!(bad_argument_precedence, "bad-argument-precedence", None);
cli_case!(unused_body_timing, "unused-body-timing", None);
cli_case!(
    parenthesized_call_boundary,
    "parenthesized-call-boundary",
    None
);
