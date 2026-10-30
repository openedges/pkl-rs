//! Entire retained Java 0.32 Resource original: reader bytes and in-language defaults.
#[path = "support/module_keys.rs"]
mod module_keys;

use pkl_runtime::evaluation::frontend::{
    ModuleEvalOptions, ModuleSourceInput, evaluate_module_output_with_hosts,
};
use pkl_runtime::modules::{ModuleCompletion, ModuleRequestKind, ModuleSource, ResolvedModule};
use pkl_runtime::resources::{ReadResult, Resource};

#[test]
fn resource_digest_defaults_delegate_to_raw_bytes_for_readers_and_language_values() {
    const PARENT_URI: &str =
        include_str!("../../../tests/fixtures/m2-resource-digests-original/parent-uri.txt");
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
    let result = evaluate_module_output_with_hosts(
        ModuleSourceInput {
            text: include_str!("../../../tests/fixtures/m2-resource-digests-original/input.pkl"),
            uri: include_str!("../../../tests/fixtures/m2-resource-digests-original/uri.txt"),
            inferred_name: "Resource",
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
                            "../../../tests/fixtures/m2-resource-digests-original/parent.pkl"
                        )
                        .into(),
                        inferred_name: "snippetTest".into(),
                    }))
                }
                other => panic!("unexpected original module request: {other:?}"),
            }
        },
        |request| {
            let authored_uri = request.uri.clone();
            let bytes: &[u8] = match authored_uri.as_str() {
                Some("empty.txt") => {
                    include_bytes!("../../../tests/fixtures/m2-resource-digests-original/empty.bin")
                }
                Some("fox.txt") => {
                    include_bytes!("../../../tests/fixtures/m2-resource-digests-original/fox.bin")
                }
                Some("foo+bar///?baz=buz") => return Some(Ok(ReadResult::Missing)),
                other => panic!("unexpected original resource request: {other:?}"),
            };
            Some(Ok(ReadResult::Resource(Resource {
                uri: request.resolved_uri().expect("original resource URI"),
                bytes: bytes.into(),
            })))
        },
    );
    assert_eq!(result.exit_code, 0, "{result:?}");
    assert_eq!(
        result.stdout,
        include_bytes!("../../../tests/fixtures/m2-resource-digests-original/stdout.bin")
    );
    assert_eq!(
        result.stderr,
        include_bytes!("../../../tests/fixtures/m2-resource-digests-original/stderr.bin")
    );
}
