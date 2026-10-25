//! Two unchanged Stage67 Java CLI originals with complete stdout/stderr comparison.
#[path = "support/module_keys.rs"]
mod module_keys;

use pkl_runtime::evaluation::frontend::{
    DiagnosticProfile, ModuleEvalOptions, ModuleSourceInput, evaluate_module_output_with_host,
};
use pkl_runtime::modules::{ModuleCompletion, ModuleRequestKind, ModuleSource, ResolvedModule};

struct Original<'a> {
    name: &'a str,
    source: &'a str,
    uri: &'a str,
    stdout: &'a [u8],
    stderr: &'a [u8],
}

fn options() -> ModuleEvalOptions {
    ModuleEvalOptions {
        no_cache: true,
        no_project: true,
        settings: "pkl:settings".into(),
        properties: vec![
            ("name1".into(), "value1".into()),
            ("name2".into(), "value2".into()),
            ("/foo/bar".into(), "foobar".into()),
        ],
        environment: vec![
            ("NAME1".into(), "value1".into()),
            ("NAME2".into(), "value2".into()),
            ("/foo/bar".into(), "foobar".into()),
            ("foo bar".into(), "foo bar".into()),
            ("file:///foo/bar".into(), "file:///foo/bar".into()),
        ],
        diagnostic_profile: DiagnosticProfile::CliBundledSettings,
    }
}

fn check(original: Original<'_>) {
    let helper_uri = original
        .uri
        .rsplit_once('/')
        .expect("pinned file URI")
        .0
        .to_owned()
        + "/typeMismatchHelper.pkl";
    let result = evaluate_module_output_with_host(
        ModuleSourceInput {
            text: original.source,
            uri: original.uri,
            inferred_name: original.name,
        },
        &options(),
        |request| {
            if let Some(completion) = module_keys::defaults(request) {
                return completion;
            }
            match &request.kind {
                ModuleRequestKind::Resolve { original_uri, .. }
                    if original_uri.as_ref() == helper_uri.as_str() =>
                {
                    ModuleCompletion::Resolved(Ok(ResolvedModule {
                        original_uri: original_uri.clone(),
                        resolved_uri: original_uri.clone(),
                        source_uri: original_uri.clone(),
                    }))
                }
                ModuleRequestKind::LoadSource { resolved, .. }
                    if resolved.original_uri.as_ref() == helper_uri.as_str() =>
                {
                    ModuleCompletion::Source(Ok(ModuleSource {
                        text: include_str!(
                            "../../../tests/fixtures/m2-type-version-conflict-originals/typeMismatchHelper.pkl"
                        )
                        .into(),
                        inferred_name: "typeMismatchHelper".into(),
                    }))
                }
                _ => panic!("unexpected Host module request: {:?}", request.kind),
            }
        },
    );
    assert_eq!(result.exit_code, 1, "{} exit", original.name);
    assert_eq!(
        result.stdout, original.stdout,
        "{} complete stdout bytes",
        original.name
    );
    assert_eq!(
        result.stderr, original.stderr,
        "{} complete stderr bytes",
        original.name
    );
}

#[test]
fn class_with_same_qualified_name_reports_distinct_module_uris() {
    check(Original {
        name: "typeMismatchWithSameQualifiedClassName",
        source: include_str!(
            "../../../tests/fixtures/m2-type-version-conflict-originals/typeMismatchWithSameQualifiedClassName/input.pkl"
        ),
        uri: include_str!(
            "../../../tests/fixtures/m2-type-version-conflict-originals/typeMismatchWithSameQualifiedClassName/uri.txt"
        ),
        stdout: include_bytes!(
            "../../../tests/fixtures/m2-type-version-conflict-originals/typeMismatchWithSameQualifiedClassName/stdout.bin"
        ),
        stderr: include_bytes!(
            "../../../tests/fixtures/m2-type-version-conflict-originals/typeMismatchWithSameQualifiedClassName/stderr.bin"
        ),
    });
}

#[test]
fn module_with_same_qualified_name_reports_distinct_module_uris() {
    check(Original {
        name: "typeMismatchWithSameQualifiedModuleName",
        source: include_str!(
            "../../../tests/fixtures/m2-type-version-conflict-originals/typeMismatchWithSameQualifiedModuleName/input.pkl"
        ),
        uri: include_str!(
            "../../../tests/fixtures/m2-type-version-conflict-originals/typeMismatchWithSameQualifiedModuleName/uri.txt"
        ),
        stdout: include_bytes!(
            "../../../tests/fixtures/m2-type-version-conflict-originals/typeMismatchWithSameQualifiedModuleName/stdout.bin"
        ),
        stderr: include_bytes!(
            "../../../tests/fixtures/m2-type-version-conflict-originals/typeMismatchWithSameQualifiedModuleName/stderr.bin"
        ),
    });
}
