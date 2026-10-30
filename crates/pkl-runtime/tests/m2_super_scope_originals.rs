//! Unchanged Stage67 CLI authority for the original invalid and allowed super calls.
#[path = "support/module_keys.rs"]
mod module_keys;
use pkl_runtime::evaluation::frontend::{
    DiagnosticProfile, ModuleEvalOptions, ModuleSourceInput, evaluate_module_output_with_host,
};
use pkl_runtime::modules::{ModuleCompletion, ModuleRequestKind, ModuleSource, ResolvedModule};

struct Original {
    label: &'static str,
    source: &'static str,
    uri: &'static str,
    stdout: &'static [u8],
    stderr: &'static [u8],
    exit: i32,
}
macro_rules! original {
    ($label:literal) => {
        Original {
            label: $label,
            source: include_str!(concat!(
                "../../../tests/fixtures/m2-super-scope-originals/",
                $label,
                "/module.pkl"
            )),
            uri: include_str!(concat!(
                "../../../tests/fixtures/m2-super-scope-originals/",
                $label,
                "/uri.txt"
            )),
            stdout: include_bytes!(concat!(
                "../../../tests/fixtures/m2-super-scope-originals/",
                $label,
                "/stdout.bin"
            )),
            stderr: include_bytes!(concat!(
                "../../../tests/fixtures/m2-super-scope-originals/",
                $label,
                "/stderr.bin"
            )),
            exit: 0,
        }
    };
}
#[test]
fn original_super_scope_modules_match_complete_java_cli_bytes_twice() {
    let mut originals = [
        original!("original-object-super-method"),
        original!("class-member-allowed"),
        original!("module-extends-allowed-one"),
        original!("module-extends-allowed-two"),
        original!("class-let-allowed"),
    ];
    originals[0].exit = 1; // Pinned Stage67 Java process exit.
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
    for original in &originals {
        for attempt in 0..2 {
            let name = original
                .uri
                .rsplit('/')
                .next()
                .unwrap()
                .strip_suffix(".pkl")
                .unwrap();
            let result = evaluate_module_output_with_host(
                ModuleSourceInput {
                    text: original.source,
                    uri: original.uri,
                    inferred_name: name,
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
                            let name = resolved
                                .original_uri
                                .as_str()
                                .expect("fixture URI must contain Unicode scalar values")
                                .rsplit('/')
                                .next()
                                .unwrap();
                            let source = originals
                                .iter()
                                .find(|row| row.uri.ends_with(name))
                                .expect("known sibling source");
                            ModuleCompletion::Source(Ok(ModuleSource {
                                text: source.source.into(),
                                inferred_name: name.strip_suffix(".pkl").unwrap().into(),
                            }))
                        }
                        _ => panic!("unexpected module request: {:?}", request.kind),
                    }
                },
            );
            assert_eq!(
                result.exit_code, original.exit,
                "{} attempt {} exit",
                original.label, attempt
            );
            assert_eq!(
                result.stdout, original.stdout,
                "{} attempt {} stdout",
                original.label, attempt
            );
            assert_eq!(
                result.stderr, original.stderr,
                "{} attempt {} stderr",
                original.label, attempt
            );
        }
    }
}
