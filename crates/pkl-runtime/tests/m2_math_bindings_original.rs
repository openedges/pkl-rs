//! Whole unchanged Java mathModule original; no floating tolerances or partial output checks.
#[path = "support/module_keys.rs"]
mod module_keys;
use pkl_runtime::evaluation::frontend::{
    ModuleEvalOptions, ModuleSourceInput, evaluate_module_output_with_host,
};
use pkl_runtime::modules::{ModuleCompletion, ModuleRequestKind, ModuleSource, ResolvedModule};

#[test]
fn unchanged_math_original_matches_complete_java_channels_and_exit() {
    const PARENT_URI: &str =
        include_str!("../../../tests/fixtures/m2-math-bindings-original/parent-uri.txt");
    let options = ModuleEvalOptions {
        properties: [
            ("name1", "value1"),
            ("name2", "value2"),
            ("/foo/bar", "foobar"),
        ]
        .map(|(key, value)| (key.into(), value.into()))
        .to_vec(),
        environment: [
            ("NAME1", "value1"),
            ("NAME2", "value2"),
            ("/foo/bar", "foobar"),
            ("foo bar", "foo bar"),
            ("file:///foo/bar", "file:///foo/bar"),
        ]
        .map(|(key, value)| (key.into(), value.into()))
        .to_vec(),
        ..Default::default()
    };
    let result = evaluate_module_output_with_host(
        ModuleSourceInput {
            text: include_str!("../../../tests/fixtures/m2-math-bindings-original/input.pkl"),
            uri: include_str!("../../../tests/fixtures/m2-math-bindings-original/uri.txt"),
            inferred_name: "mathModule",
        },
        &options,
        |request| {
            if let Some(completion) = module_keys::defaults(request) {
                return completion;
            }
            match &request.kind {
                ModuleRequestKind::Resolve { original_uri, .. } => {
                    assert_eq!(original_uri.clone(), PARENT_URI);
                    ModuleCompletion::Resolved(Ok(ResolvedModule {
                        original_uri: original_uri.clone(),
                        resolved_uri: original_uri.clone(),
                        source_uri: original_uri.clone(),
                    }))
                }
                ModuleRequestKind::LoadSource { resolved, .. } => {
                    assert_eq!(resolved.source_uri.clone(), PARENT_URI);
                    ModuleCompletion::Source(Ok(ModuleSource {
                        text: include_str!(
                            "../../../tests/fixtures/m2-math-bindings-original/parent.pkl"
                        )
                        .into(),
                        inferred_name: "snippetTest".into(),
                    }))
                }
                other => panic!("unexpected original module request: {other:?}"),
            }
        },
    );
    assert_eq!(result.exit_code, 0, "{result:?}");
    assert_eq!(
        result.stdout,
        include_bytes!("../../../tests/fixtures/m2-math-bindings-original/stdout.bin")
    );
    assert_eq!(
        result.stderr,
        include_bytes!("../../../tests/fixtures/m2-math-bindings-original/stderr.bin")
    );
}

fn unhex(text: &str) -> Vec<u8> {
    assert_eq!(text.len() % 2, 0);
    text.as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect()
}

#[test]
fn reused_shared_kernels_keep_existing_complete_java_primitive_edge_payloads() {
    // This is the retained M1 bit protocol, including its explicit Double.doubleToLongBits
    // NaN rule. It is separate from whole-module Pkl output, with no new tolerance.
    let mut count = 0;
    for line in
        include_str!("../../../tests/fixtures/m2-math-bindings-original/primitive-edges.tsv")
            .split_terminator('\n')
    {
        let fields = line.split('\t').collect::<Vec<_>>();
        assert_eq!(fields.len(), 3);
        let input = String::from_utf8(unhex(fields[1])).unwrap();
        let actual = pkl_runtime::numeric::probe(&input).unwrap();
        assert_eq!(actual.as_bytes(), unhex(fields[2]), "{}", fields[0]);
        count += 1;
    }
    assert_eq!(count, 49);
}
