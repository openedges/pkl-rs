//! Stage67 Java CLI authority for three unchanged upstream version-check originals.
//! Proposed test only: this target-only draft has not been compiled or executed.
#[path = "support/module_keys.rs"]
mod module_keys;

use pkl_runtime::evaluation::frontend::{
    DiagnosticProfile, ModuleEvalOptions, ModuleSourceInput, evaluate_module_output_with_host,
};
use pkl_runtime::modules::{ModuleCompletion, ModuleRequestKind, ModuleSource, ResolvedModule};

struct Original<'a> {
    label: &'a str,
    source: &'a str,
    uri: &'a str,
    java_stdout: &'a [u8],
    java_stderr: &'a [u8],
}

#[test]
fn original_import_amends_extends_version_errors_match_complete_java_cli_bytes() {
    let originals = [
        Original {
            label: "amends",
            source: include_str!(
                "../../../tests/fixtures/m2-module-version-frames/amends/input.pkl"
            ),
            uri: include_str!("../../../tests/fixtures/m2-module-version-frames/amends/uri.txt"),
            java_stdout: include_bytes!(
                "../../../tests/fixtures/m2-module-version-frames/amends/stdout.bin"
            ),
            java_stderr: include_bytes!(
                "../../../tests/fixtures/m2-module-version-frames/amends/stderr.bin"
            ),
        },
        Original {
            label: "extends",
            source: include_str!(
                "../../../tests/fixtures/m2-module-version-frames/extends/input.pkl"
            ),
            uri: include_str!("../../../tests/fixtures/m2-module-version-frames/extends/uri.txt"),
            java_stdout: include_bytes!(
                "../../../tests/fixtures/m2-module-version-frames/extends/stdout.bin"
            ),
            java_stderr: include_bytes!(
                "../../../tests/fixtures/m2-module-version-frames/extends/stderr.bin"
            ),
        },
        Original {
            label: "import",
            source: include_str!(
                "../../../tests/fixtures/m2-module-version-frames/import/input.pkl"
            ),
            uri: include_str!("../../../tests/fixtures/m2-module-version-frames/import/uri.txt"),
            java_stdout: include_bytes!(
                "../../../tests/fixtures/m2-module-version-frames/import/stdout.bin"
            ),
            java_stderr: include_bytes!(
                "../../../tests/fixtures/m2-module-version-frames/import/stderr.bin"
            ),
        },
    ];
    let sibling = include_str!(
        "../../../tests/fixtures/m2-module-version-frames/moduleWithHighMinPklVersion.pkl"
    );
    let options = ModuleEvalOptions {
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
    };
    for original in originals {
        let name = format!("module{}VersionCheck", {
            let mut chars = original.label.chars();
            format!(
                "{}{}",
                chars.next().unwrap().to_ascii_uppercase(),
                chars.as_str()
            )
        });
        let mut resolves = 0;
        let mut loads = 0;
        let actual = evaluate_module_output_with_host(
            ModuleSourceInput {
                text: original.source,
                uri: original.uri,
                inferred_name: &name,
            },
            &options,
            |request| match &request.kind {
                ModuleRequestKind::ResolveKey { .. } => panic!("unexpected public key lookup"),
                ModuleRequestKind::SelectKey { .. }
                | ModuleRequestKind::QueryCachePolicy { .. }
                | ModuleRequestKind::QueryFragmentPaths { .. }
                | ModuleRequestKind::ResolveImportUri { .. } => module_keys::complete(request),
                ModuleRequestKind::CheckImport { .. } => {
                    module_keys::defaults(request).expect("default import policy")
                }
                ModuleRequestKind::Capabilities { .. } | ModuleRequestKind::HasElement { .. } => {
                    panic!("unexpected module-key preparation")
                }
                ModuleRequestKind::Resolve { original_uri, .. } => {
                    assert!(original_uri.ends_with("/moduleWithHighMinPklVersion.pkl"));
                    resolves += 1;
                    ModuleCompletion::Resolved(Ok(ResolvedModule {
                        original_uri: original_uri.clone(),
                        resolved_uri: original_uri.clone(),
                        source_uri: original_uri.clone(),
                    }))
                }
                ModuleRequestKind::LoadSource { resolved, .. } => {
                    assert!(
                        resolved
                            .original_uri
                            .ends_with("/moduleWithHighMinPklVersion.pkl")
                    );
                    loads += 1;
                    ModuleCompletion::Source(Ok(ModuleSource {
                        text: sibling.into(),
                        inferred_name: "moduleWithHighMinPklVersion".into(),
                    }))
                }
            },
        );
        assert_eq!(
            (resolves, loads),
            (1, 1),
            "{} Host boundary",
            original.label
        );
        assert_eq!(actual.exit_code, 1, "{} exit", original.label);
        assert_eq!(
            actual.stdout, original.java_stdout,
            "{} complete stdout",
            original.label
        );
        assert_eq!(
            actual.stderr, original.java_stderr,
            "{} complete stderr",
            original.label
        );
    }
}
