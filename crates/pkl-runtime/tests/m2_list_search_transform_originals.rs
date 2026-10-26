//! Six unchanged upstream modules; complete recorded Java CLI stdout/stderr.
#[path = "support/module_keys.rs"]
mod module_keys;

use pkl_runtime::evaluation::frontend::{
    DiagnosticProfile, ModuleEvalOptions, ModuleSourceInput, evaluate_module_output_with_host,
};
use pkl_runtime::modules::{
    ModuleCapabilities, ModuleCompletion, ModuleRequestKind, ModuleSource, ResolvedModule,
};

fn check(name: &str, source: &str, uri: &str, stdout: &[u8], stderr: &[u8]) {
    let snippet_uri = include_str!(
        "../../../tests/fixtures/m2-list-search-transform-originals/snippetTest.uri.txt"
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
    for attempt in 0..2 {
        let result = evaluate_module_output_with_host(
            ModuleSourceInput {
                text: source,
                uri,
                inferred_name: name,
            },
            &options,
            |request| {
                if let Some(completion) = module_keys::defaults(request) {
                    return completion;
                }
                match &request.kind {
                    ModuleRequestKind::Capabilities { .. } => {
                        ModuleCompletion::Capabilities(Ok(ModuleCapabilities {
                            local: true,
                            hierarchical: true,
                            fragment_paths: false,
                        }))
                    }
                    ModuleRequestKind::HasElement { element_uri, .. } => {
                        ModuleCompletion::HasElement(Ok(element_uri.as_ref() == snippet_uri))
                    }
                    ModuleRequestKind::Resolve { original_uri, .. }
                        if original_uri.as_ref() == snippet_uri => {
                        ModuleCompletion::Resolved(Ok(ResolvedModule {
                            original_uri: original_uri.clone(),
                            resolved_uri: original_uri.clone(),
                            source_uri: original_uri.clone(),
                        }))
                    }
                    ModuleRequestKind::LoadSource { resolved, .. }
                        if resolved.original_uri.as_ref() == snippet_uri => {
                        ModuleCompletion::Source(Ok(ModuleSource {
                            text: include_str!("../../../tests/fixtures/m2-list-search-transform-originals/snippetTest.pkl").into(),
                            inferred_name: "snippetTest".into(),
                        }))
                    }
                    _ => panic!("unexpected fixture request: {:?}", request.kind),
                }
            },
        );
        assert_eq!(
            result.exit_code, 0,
            "{name} attempt {attempt}: {:?}",
            result.stderr
        );
        assert_eq!(
            result.stdout, stdout,
            "{name} complete stdout, attempt {attempt}"
        );
        assert_eq!(
            result.stderr, stderr,
            "{name} complete stderr, attempt {attempt}"
        );
    }
}

macro_rules! original {
    ($test:ident, $name:literal) => {
        #[test]
        fn $test() {
            check(
                $name,
                include_str!(concat!(
                    "../../../tests/fixtures/m2-list-search-transform-originals/",
                    $name,
                    "/module.pkl"
                )),
                include_str!(concat!(
                    "../../../tests/fixtures/m2-list-search-transform-originals/",
                    $name,
                    "/uri.txt"
                )),
                include_bytes!(concat!(
                    "../../../tests/fixtures/m2-list-search-transform-originals/",
                    $name,
                    "/stdout.bin"
                )),
                include_bytes!(concat!(
                    "../../../tests/fixtures/m2-list-search-transform-originals/",
                    $name,
                    "/stderr.bin"
                )),
            );
        }
    };
}

original!(predicate_dynamic_listing, "predicateMembersDynamicListing");
original!(predicate_dynamic_mapping, "predicateMembersDynamicMapping");
original!(predicate_listing, "predicateMembersListing");
original!(predicate_mapping, "predicateMembersMapping");
original!(for_generator_lexical_scope, "forGeneratorLexicalScope");
original!(nested_flat_map_lambda, "lambda4");
