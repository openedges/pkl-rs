//! Unchanged constraints14 and its two sibling modules, with complete Stage67 Java CLI bytes.
#[path = "support/module_keys.rs"]
mod module_keys;

use pkl_runtime::evaluation::frontend::{
    DiagnosticProfile, ModuleEvalOptions, ModuleSourceInput, evaluate_module_output_with_host,
};
use pkl_runtime::modules::{
    ModuleCapabilities, ModuleCompletion, ModuleRequestKind, ModuleSource, ResolvedModule,
};

const SOURCE: &str =
    include_str!("../../../tests/fixtures/m2-typed-equality-constraint-original/constraints14.pkl");
const URI: &str =
    include_str!("../../../tests/fixtures/m2-typed-equality-constraint-original/uri.txt");
const MY_CLASS: &str =
    include_str!("../../../tests/fixtures/m2-typed-equality-constraint-original/MyClass.pkl");
const MY_CLASS_URI: &str =
    include_str!("../../../tests/fixtures/m2-typed-equality-constraint-original/MyClass.uri.txt");
const MY_CLASS_ONE: &str =
    include_str!("../../../tests/fixtures/m2-typed-equality-constraint-original/myClass1.pkl");
const MY_CLASS_ONE_URI: &str =
    include_str!("../../../tests/fixtures/m2-typed-equality-constraint-original/myClass1.uri.txt");

#[test]
fn typed_module_constraint_short_circuits_before_an_in_progress_later_cache_slot() {
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
    // Each observation uses a fresh evaluator. Expected bytes remain the
    // unchanged Java authority; no output normalization or Rust-derived oracle.
    for attempt in 0..2 {
        let mut loaded = Vec::new();
        let actual = evaluate_module_output_with_host(
            ModuleSourceInput {
                text: SOURCE,
                uri: URI,
                inferred_name: "constraints14",
            },
            &options,
            |request| match &request.kind {
                ModuleRequestKind::Capabilities { .. } => {
                    ModuleCompletion::Capabilities(Ok(ModuleCapabilities {
                        local: true,
                        hierarchical: true,
                        fragment_paths: false,
                    }))
                }
                ModuleRequestKind::HasElement { element_uri, .. } => {
                    ModuleCompletion::HasElement(Ok(element_uri.as_ref() == MY_CLASS_URI
                        || element_uri.as_ref() == MY_CLASS_ONE_URI))
                }
                ModuleRequestKind::Resolve { original_uri, .. } => {
                    assert!(
                        original_uri.as_ref() == MY_CLASS_URI
                            || original_uri.as_ref() == MY_CLASS_ONE_URI
                    );
                    ModuleCompletion::Resolved(Ok(ResolvedModule {
                        original_uri: original_uri.clone(),
                        resolved_uri: original_uri.clone(),
                        source_uri: original_uri.clone(),
                    }))
                }
                ModuleRequestKind::LoadSource { resolved, .. } => {
                    let (text, name) = match resolved.original_uri.as_ref() {
                        uri if uri == MY_CLASS_URI => (MY_CLASS, "MyClass"),
                        uri if uri == MY_CLASS_ONE_URI => (MY_CLASS_ONE, "myClass1"),
                        other => panic!("unexpected fixture URI: {other}"),
                    };
                    loaded.push(resolved.original_uri.to_string());
                    ModuleCompletion::Source(Ok(ModuleSource {
                        text: text.into(),
                        inferred_name: name.into(),
                    }))
                }
                ModuleRequestKind::SelectKey { .. }
                | ModuleRequestKind::QueryCachePolicy { .. }
                | ModuleRequestKind::QueryFragmentPaths { .. }
                | ModuleRequestKind::ResolveImportUri { .. }
                | ModuleRequestKind::CheckImport { .. } => module_keys::complete(request),
                ModuleRequestKind::ResolveKey { .. } => panic!("unexpected public query key"),
            },
        );
        assert!(loaded.iter().any(|uri| uri == MY_CLASS_URI));
        assert!(loaded.iter().any(|uri| uri == MY_CLASS_ONE_URI));
        assert_eq!(actual.exit_code, 0, "attempt {attempt}");
        assert_eq!(
            actual.stdout,
            include_bytes!(
                "../../../tests/fixtures/m2-typed-equality-constraint-original/stdout.bin"
            ),
            "complete stdout, attempt {attempt}"
        );
        assert_eq!(
            actual.stderr,
            include_bytes!(
                "../../../tests/fixtures/m2-typed-equality-constraint-original/stderr.bin"
            ),
            "complete stderr, attempt {attempt}"
        );
    }
}
