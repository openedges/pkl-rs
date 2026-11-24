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

#[test]
fn inherited_module_self_type_tracks_sub_module_receiver() {
    let base_src = "open module Base\nx: module = this\nitems: Listing<module> = new { module }\n";
    let sub_direct_src =
        "open module Sub\nextends \"Base.pkl\"\nimport \"Base.pkl\"\nx = new Base {}\n";
    let sub_listing_src = "open module SubListing\nextends \"Base.pkl\"\nimport \"Base.pkl\"\nitems { new Base {} }\n";
    let files: &[(&str, &str)] = &[
        ("Base", base_src),
        ("Sub", sub_direct_src),
        ("SubListing", sub_listing_src),
    ];

    for (entry_text, expected_err) in [
        (
            "import \"Base.pkl\"\nimport \"Sub.pkl\"\na = Base.x\nb = Sub.x\n",
            "Expected value of type `Sub`, but got type `Base`.",
        ),
        (
            "import \"Base.pkl\"\nimport \"SubListing.pkl\"\na = Base.items.length\nb = SubListing.items[1]\n",
            "Expected value of type `SubListing`, but got type `Base`.",
        ),
    ] {
        let result = evaluate_module_output_with_host(
            ModuleSourceInput {
                text: entry_text,
                uri: "file:///corpus/modules/Run.pkl",
                inferred_name: "Run",
            },
            &ModuleEvalOptions::default(),
            |request| match &request.kind {
                ModuleRequestKind::SelectKey { .. }
                | ModuleRequestKind::QueryCachePolicy { .. }
                | ModuleRequestKind::QueryFragmentPaths { .. }
                | ModuleRequestKind::ResolveImportUri { .. } => module_keys::complete(request),
                ModuleRequestKind::CheckImport { .. } => {
                    module_keys::defaults(request).expect("default import policy")
                }
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
                        .expect("utf8 uri")
                        .rsplit('/')
                        .next()
                        .unwrap()
                        .strip_suffix(".pkl")
                        .unwrap();
                    let text = files.iter().find(|(k, _)| *k == name).unwrap().1;
                    ModuleCompletion::Source(Ok(ModuleSource {
                        text: text.into(),
                        inferred_name: name.into(),
                    }))
                }
                other => panic!("unexpected module request: {other:?}"),
            },
        );
        assert_ne!(result.exit_code, 0, "expected type mismatch error");
        let stderr = String::from_utf8_lossy(&result.stderr);
        assert!(
            stderr.contains(expected_err),
            "expected `{expected_err}`, got:\n{stderr}"
        );
    }
}

#[test]
fn fast_type_checks_resolve_map_val_and_named_alias_arguments() {
    let cases = [
        "import \"pkl:base\"\nres = Map(\"a\", 1) is Map<Int, base.DoesNotExist>\n",
        "import \"pkl:base\"\nx: Map<Int, base.DoesNotExist> | Map<String, Int> = Map(\"a\", 1)\n",
        "import \"pkl:base\"\ntypealias IgnoreInt<T> = Int\nr = 1 is IgnoreInt<base.DoesNotExist>\n",
        "import \"pkl:base\"\ntypealias IgnoreInt<T> = Int\nr = null is IgnoreInt<base.DoesNotExist>?\n",
        "import \"pkl:base\"\ntypealias IgnoreList<T> = List<Int>\nr = List(1) is IgnoreList<base.DoesNotExist>\n",
        "import \"pkl:base\"\ntypealias IgnoreAny<T> = Any\nr = List(1) is List<IgnoreAny<base.DoesNotExist>>\n",
        "import \"pkl:base\"\ntypealias IgnoreAny<T> = Any\nr = Set(1) is Set<IgnoreAny<base.DoesNotExist>>\n",
        "import \"pkl:base\"\ntypealias IgnoreAny<T> = Any\nr = Map(\"a\", 1) is Map<IgnoreAny<base.DoesNotExist>, Any>\n",
    ];

    for (idx, source) in cases.iter().enumerate() {
        let result = evaluate_module_output_with_host(
            ModuleSourceInput {
                text: source,
                uri: &format!("file:///corpus/modules/fast-type-case-{idx}.pkl"),
                inferred_name: "fast_type_case",
            },
            &ModuleEvalOptions::default(),
            |request| match &request.kind {
                ModuleRequestKind::SelectKey { .. }
                | ModuleRequestKind::QueryCachePolicy { .. }
                | ModuleRequestKind::QueryFragmentPaths { .. }
                | ModuleRequestKind::ResolveImportUri { .. } => module_keys::complete(request),
                ModuleRequestKind::CheckImport { .. } => {
                    module_keys::defaults(request).expect("default import policy")
                }
                other => panic!("unexpected module request: {other:?}"),
            },
        );
        assert_ne!(
            result.exit_code,
            0,
            "case {idx} unexpectedly succeeded:\n{source}\nstdout={}",
            String::from_utf8_lossy(&result.stdout)
        );
        let stderr = String::from_utf8_lossy(&result.stderr);
        assert!(
            stderr.contains("Cannot find type `DoesNotExist` in module `pkl.base`."),
            "case {idx} unexpected error:\n{stderr}"
        );
    }
}

#[test]
fn verified_strict_collections_invalidated_on_amends_receiver_overwrite() {
    let b_list = concat!(
        "@ModuleInfo { minPklVersion = if (xs is List<Dynamic>) \"0.1.0\" else \"0.1.0\" }\n",
        "open module b\n",
        "import \"a.pkl\" as a\n",
        "const hidden xs = List(a, a, a, a)\n",
        "hidden checked: List<Dynamic> = xs\n",
        "x = checked.length\n",
    );
    let b_map = concat!(
        "@ModuleInfo { minPklVersion = if (xs is Map<Int, Dynamic>) \"0.1.0\" else \"0.1.0\" }\n",
        "open module b\n",
        "import \"a.pkl\" as a\n",
        "const hidden xs = Map(1, a, 2, a, 3, a, 4, a)\n",
        "hidden checked: Map<Int, Dynamic> = xs\n",
        "x = checked.length\n",
    );
    let a_src = "amends \"b.pkl\"\n";

    for (label, b_src) in [("list", b_list), ("map", b_map)] {
        let result = evaluate_module_output_with_host(
            ModuleSourceInput {
                text: a_src,
                uri: "file:///corpus/modules/a.pkl",
                inferred_name: "a",
            },
            &ModuleEvalOptions::default(),
            |request| match &request.kind {
                ModuleRequestKind::SelectKey { .. }
                | ModuleRequestKind::QueryCachePolicy { .. }
                | ModuleRequestKind::QueryFragmentPaths { .. }
                | ModuleRequestKind::ResolveImportUri { .. } => module_keys::complete(request),
                ModuleRequestKind::CheckImport { .. } => {
                    module_keys::defaults(request).expect("default import policy")
                }
                ModuleRequestKind::Resolve { original_uri, .. } => {
                    ModuleCompletion::Resolved(Ok(ResolvedModule {
                        original_uri: original_uri.clone(),
                        resolved_uri: original_uri.clone(),
                        source_uri: original_uri.clone(),
                    }))
                }
                ModuleRequestKind::LoadSource { resolved, .. } => {
                    let uri = resolved.original_uri.as_str().expect("utf8 uri");
                    let (name, text) = if uri.ends_with("/b.pkl") {
                        ("b", b_src)
                    } else if uri.ends_with("/a.pkl") {
                        ("a", a_src)
                    } else {
                        panic!("unexpected module load: {uri}");
                    };
                    ModuleCompletion::Source(Ok(ModuleSource {
                        text: text.into(),
                        inferred_name: name.into(),
                    }))
                }
                other => panic!("unexpected module request: {other:?}"),
            },
        );
        assert_ne!(
            result.exit_code,
            0,
            "{label}: expected Pkl 0.32.0 type mismatch error, got stdout:\n{}",
            String::from_utf8_lossy(&result.stdout)
        );
        let stderr = String::from_utf8_lossy(&result.stderr);
        assert!(
            stderr.contains("Expected value of type `Dynamic`, but got type `b`."),
            "{label}: unexpected stderr:\n{stderr}"
        );
    }
}

#[test]
fn generator_prefix_compile_errors_preserve_pass1_entry_numbering() {
    // Verified against pinned Apple Pkl 0.32.0:
    // - Compile errors in the prefix before the first generator abort during pass 1 (#2, not #5).
    // - Runtime errors in a valid prefix and compile errors after the first generator use pass-2 numbering.
    let cases = [
        (
            "prefix-element-compile-error",
            "x = new {\n  10\n  super.f()\n  20\n  when (true) { 2 }\n}\n",
            "at repro#x[#2] ",
        ),
        (
            "prefix-unlabeled-entry-compile-error",
            "x = new {\n  10\n  [1 + 2] = super.f()\n  20\n  when (true) { 2 }\n}\n",
            "at repro#x[#2] ",
        ),
        (
            "prefix-nested-object-compile-error",
            "x = new {\n  10\n  new { super.f() }\n  20\n  when (true) { 2 }\n}\n",
            "at repro#x[#2][#1] ",
        ),
        (
            "prefix-lambda-compile-error",
            "x = new {\n  10\n  () -> super.f()\n  20\n  when (true) { 2 }\n}\n",
            "at repro#x[#2].<function#1> ",
        ),
        (
            "prefix-let-compile-error",
            "x = new {\n  10\n  let (a = 1) super.f()\n  20\n  when (true) { 2 }\n}\n",
            "at repro#x[#2].<let expr> ",
        ),
        (
            "prefix-runtime-error-uses-pass2",
            "x = new {\n  10\n  throw(\"boom\")\n  20\n  when (true) { 2 }\n}\n",
            "at repro#x[#5] ",
        ),
        (
            "post-generator-compile-error-uses-pass2",
            "x = new {\n  10\n  20\n  when (true) { 2 }\n  super.f()\n}\n",
            "at repro#x[#6] ",
        ),
    ];

    for (label, source, expected_frame) in cases {
        let result = evaluate_module_output_with_host(
            ModuleSourceInput {
                text: source,
                uri: "file:///corpus/modules/repro.pkl",
                inferred_name: "repro",
            },
            &ModuleEvalOptions::default(),
            |request| match &request.kind {
                ModuleRequestKind::SelectKey { .. }
                | ModuleRequestKind::QueryCachePolicy { .. }
                | ModuleRequestKind::QueryFragmentPaths { .. }
                | ModuleRequestKind::ResolveImportUri { .. } => module_keys::complete(request),
                ModuleRequestKind::CheckImport { .. } => {
                    module_keys::defaults(request).expect("default import policy")
                }
                other => panic!("unexpected module request in {label}: {other:?}"),
            },
        );
        assert_ne!(
            result.exit_code,
            0,
            "{label}: expected failure, got stdout:\n{}",
            String::from_utf8_lossy(&result.stdout)
        );
        let stderr = String::from_utf8_lossy(&result.stderr);
        assert!(
            stderr.contains(expected_frame),
            "{label}: expected frame `{expected_frame}` in stderr:\n{stderr}"
        );
    }
}
