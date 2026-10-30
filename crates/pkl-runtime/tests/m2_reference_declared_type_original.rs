//! Unchanged upstream reference stdout and CLI Logger bytes from Stage67 Java.
#[path = "support/module_keys.rs"]
mod module_keys;
use pkl_runtime::{
    evaluation::frontend::{ModuleEvalOptions, ModuleSourceInput},
    modules::{ModuleCompletion, ModuleRequestKind, ModuleSource, ResolvedModule},
    probe::SourceProbe,
    value::PklString,
};

#[test]
fn original_reference_full_output_matches_java() {
    // This is an in-memory source with the captured diagnostic identity. No
    // physical access to the historical workspace is needed by this module.
    const URI: &str = "file:///Users/cody.ij.hwang/Projects/pkl-rs.stage57/target/m2/stage67-full-regression-01-workspace/external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/basic/reference.pkl";
    for _ in 0..2 {
        let mut probe = SourceProbe::new_output(
            ModuleSourceInput {
                text: include_str!(
                    "../../../tests/fixtures/m2-reference-declared-type/original-output/module.pkl"
                ),
                uri: URI,
                inferred_name: "reference",
            },
            &[],
            &ModuleEvalOptions::default(),
            SourceProbe::options("compact", true, false, false).unwrap(),
        )
        .unwrap();
        probe.start_output_bytes(0, None).unwrap();
        let mut stderr = Vec::new();
        let mut terminal = None;
        for _ in 0..500_000 {
            probe.collect();
            let event = probe.poll(0, 1).unwrap();
            probe.collect();
            if event.kind == "yield" {
                continue;
            }
            if event.kind == "module-request"
                && module_keys::complete_probe(&mut probe, event.token.as_deref().unwrap(), 0)
                    .unwrap()
            {
                continue;
            }
            if event.kind == "module-request" {
                let token = event.token.as_deref().unwrap();
                let request = probe.module_request(token).unwrap();
                const SNIPPET_URI: &str = "file:///Users/cody.ij.hwang/Projects/pkl-rs.stage57/target/m2/stage67-full-regression-01-workspace/external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/snippetTest.pkl";
                let completion = match &request.kind {
                    ModuleRequestKind::Resolve { original_uri, .. }
                        if original_uri == SNIPPET_URI =>
                    {
                        ModuleCompletion::Resolved(Ok(ResolvedModule {
                            original_uri: original_uri.clone(),
                            resolved_uri: original_uri.clone(),
                            source_uri: original_uri.clone(),
                        }))
                    }
                    ModuleRequestKind::LoadSource { resolved, .. }
                        if resolved.original_uri == SNIPPET_URI =>
                    {
                        ModuleCompletion::Source(Ok(ModuleSource {
                            text: include_str!("../../../tests/fixtures/m2-reference-declared-type/original-output/snippetTest.pkl").into(),
                            inferred_name: "snippetTest".into(),
                        }))
                    }
                    _ => panic!("unexpected original dependency request: {request:?}"),
                };
                probe.complete_module(token, completion, 0).unwrap();
                continue;
            }
            if event.kind == "read" {
                probe
                    .respond_configured(event.token.as_deref().unwrap(), 0)
                    .unwrap();
                continue;
            }
            if event.kind == "log" {
                let log = event.log.as_ref().unwrap();
                // Project the actual Logger event to the CLI's public byte
                // channel; the separate 20-row test compares all raw fields.
                stderr.extend_from_slice(
                    format!(
                        "pkl: {}: {} ({}, line {})\n",
                        log.level.to_ascii_uppercase(),
                        PklString::from_utf16(log.message.clone()).to_java_utf8(),
                        PklString::from_utf16(log.module_uri.clone()).to_java_utf8(),
                        log.start_line
                    )
                    .as_bytes(),
                );
                probe
                    .acknowledge_log(event.token.as_deref().unwrap(), None, 0)
                    .unwrap();
                continue;
            }
            terminal = Some(event);
            break;
        }
        let event = terminal.expect("bounded original Reference output");
        assert_eq!(event.kind, "bytes", "{event:?}");
        assert_eq!(
            event.bytes.as_deref(),
            Some(
                include_bytes!(
                    "../../../tests/fixtures/m2-reference-declared-type/original-output/stdout.bin"
                )
                .as_slice()
            )
        );
        assert_eq!(
            stderr,
            include_bytes!(
                "../../../tests/fixtures/m2-reference-declared-type/original-output/stderr.bin"
            )
        );
        probe.close();
        assert_eq!(probe.collect().slots, 0);
    }
}
