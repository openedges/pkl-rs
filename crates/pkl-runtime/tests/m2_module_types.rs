//! Original module-type and inheritance contracts use the same evaluator and Host.
#[path = "support/module_keys.rs"]
mod module_keys;
use pkl_runtime::evaluation::frontend::{
    ModuleEvalOptions, ModuleSourceInput, evaluate_module_output_with_host,
};
use pkl_runtime::modules::{ModuleCompletion, ModuleRequestKind, ModuleSource, ResolvedModule};

const SOURCES: &[(&str, &str)] = &[
    (
        "recursiveModule1",
        include_str!(
            "../../../external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/modules/recursiveModule1.pkl"
        ),
    ),
    (
        "recursiveModule2",
        include_str!(
            "../../../external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/modules/recursiveModule2.pkl"
        ),
    ),
    (
        "library",
        include_str!(
            "../../../external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/modules/library.pkl"
        ),
    ),
    (
        "extendModule1",
        include_str!(
            "../../../external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/modules/extendModule1.pkl"
        ),
    ),
    (
        "amendModule1",
        include_str!(
            "../../../external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/modules/amendModule1.pkl"
        ),
    ),
];

fn evaluate_original(name: &str, expected: &[u8]) -> Vec<String> {
    let text = SOURCES.iter().find(|(key, _)| *key == name).unwrap().1;
    let mut loads = Vec::new();
    let result = evaluate_module_output_with_host(
        ModuleSourceInput {
            text,
            uri: &format!("file:///corpus/modules/{name}.pkl"),
            inferred_name: name,
        },
        &ModuleEvalOptions::default(),
        |request| match &request.kind {
            ModuleRequestKind::ResolveKey { .. } => {
                panic!("public lifecycle request in supplied-source test")
            }
            ModuleRequestKind::SelectKey { .. }
            | ModuleRequestKind::QueryCachePolicy { .. }
            | ModuleRequestKind::QueryFragmentPaths { .. }
            | ModuleRequestKind::ResolveImportUri { .. } => module_keys::complete(request),
            ModuleRequestKind::CheckImport { .. } => {
                module_keys::defaults(request).expect("default import policy")
            }
            ModuleRequestKind::Capabilities { .. } | ModuleRequestKind::HasElement { .. } => {
                panic!("unexpected ModuleKey preparation in this fixture")
            }
            ModuleRequestKind::Resolve { original_uri, .. } => {
                ModuleCompletion::Resolved(Ok(ResolvedModule {
                    original_uri: original_uri.clone(),
                    resolved_uri: original_uri.clone(),
                    source_uri: original_uri.clone(),
                }))
            }
            ModuleRequestKind::LoadSource { resolved, .. } => {
                loads.push(resolved.original_uri.to_string());
                let name = resolved
                    .original_uri
                    .as_str()
                    .expect("fixture URI must contain Unicode scalar values")
                    .rsplit('/')
                    .next()
                    .unwrap()
                    .strip_suffix(".pkl")
                    .unwrap();
                let text = SOURCES.iter().find(|(key, _)| *key == name).unwrap().1;
                ModuleCompletion::Source(Ok(ModuleSource {
                    text: text.into(),
                    inferred_name: name.into(),
                }))
            }
        },
    );
    assert_eq!(
        result.exit_code,
        0,
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(result.stdout, expected);
    assert!(result.stderr.is_empty());
    loads
}

#[test]
fn original_mutually_recursive_module_types_render_through_one_heap() {
    assert_eq!(
        evaluate_original(
            "recursiveModule1",
            include_bytes!(
                "../../../external/pkl/pkl-core/src/test/files/LanguageSnippetTests/output/modules/recursiveModule1.pcf"
            )
        ),
        ["file:///corpus/modules/recursiveModule2.pkl"]
    );
}

#[test]
fn original_recursive_second_entry_has_no_output_members() {
    // Actual CLI original30 case ffcbc1ead8787f2b765abdd2 has empty stdout in both Java runs.
    assert_eq!(
        evaluate_original("recursiveModule2", b""),
        ["file:///corpus/modules/recursiveModule1.pkl"]
    );
}

#[test]
fn original_extending_module_keeps_parent_and_declaration_owners() {
    assert_eq!(
        evaluate_original(
            "extendModule1",
            include_bytes!(
                "../../../external/pkl/pkl-core/src/test/files/LanguageSnippetTests/output/modules/extendModule1.pcf"
            )
        ),
        ["file:///corpus/modules/library.pkl"]
    );
}

#[test]
fn original_amending_module_adopts_the_parent_class() {
    assert_eq!(
        evaluate_original(
            "amendModule1",
            include_bytes!(
                "../../../external/pkl/pkl-core/src/test/files/LanguageSnippetTests/output/modules/amendModule1.pcf"
            )
        ),
        ["file:///corpus/modules/library.pkl"]
    );
}
