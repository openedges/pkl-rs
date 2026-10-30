//! Shared probe boundaries: complete actual Java code units, repeats and collection.
//! Constructed callback controls below are adapter invariants, not new Java captures.
use pkl_runtime::{
    modules::{
        self, ModuleCapabilities, ModuleCompletion, ModuleKey, ModuleKeyId, ModuleKeyKind,
        ModuleRequest, ModuleRequestKind, ResolvedModule, ResolvedModuleKey,
    },
    probe::{SourceEvent, SourceModuleRequest, SourceProbe, SourceTextModuleRequest},
    source::SourceText,
    value::{ErrorKind, PklString},
};

fn units(bytes: &[u8]) -> Vec<u16> {
    let (pairs, tail) = bytes.as_chunks::<2>();
    assert!(tail.is_empty());
    pairs.iter().map(|pair| u16::from_be_bytes(*pair)).collect()
}

fn complete(probe: &mut SourceProbe, token: &str) {
    let request = probe.module_request(token).unwrap();
    let completion = modules::default_completion(&request).unwrap_or_else(|| match &request.kind {
        ModuleRequestKind::SelectKey { id, uri, .. } => {
            ModuleCompletion::KeySelected(Ok(ModuleKey {
                id: *id,
                uri: uri.clone(),
                kind: ModuleKeyKind::Host,
                handle: "adapter-fixture".into(),
            }))
        }
        ModuleRequestKind::QueryCachePolicy { .. } => ModuleCompletion::CachePolicy(Ok(true)),
        ModuleRequestKind::Capabilities { key: Some(_), .. } => {
            ModuleCompletion::Capabilities(Ok(ModuleCapabilities {
                local: true,
                hierarchical: true,
                fragment_paths: false,
            }))
        }
        ModuleRequestKind::Resolve {
            key,
            resolved_key_id,
            ..
        }
        | ModuleRequestKind::ResolveKey {
            key,
            resolved_key_id,
            ..
        } => ModuleCompletion::KeyResolved(Ok(ResolvedModuleKey {
            id: *resolved_key_id,
            key: key.clone(),
            handle: key.handle.clone(),
            module: ResolvedModule {
                original_uri: key.uri.clone(),
                resolved_uri: key.uri.clone(),
                source_uri: key.uri.clone(),
            },
        })),
        effect => panic!("unconfigured fixture effect: {effect:?}"),
    });
    probe.complete_module(token, completion, 0).unwrap();
}

#[test]
fn actual_java_high_low_mixed_values_pass_lossless_probe_repeats_and_gc() {
    type Case<'a> = (&'a [u8], &'a [u8], &'a [u8], &'a [u16]);
    let cases: &[Case<'_>] = &[
        (
            include_bytes!("fixtures/m2-glob-lossless-source/high.source.utf16be"),
            include_bytes!("fixtures/m2-glob-lossless-source/high.value.utf16be"),
            include_bytes!("fixtures/m2-lossless-probe-adapters/high.value.java-utf8.bin"),
            &[0xd800],
        ),
        (
            include_bytes!("fixtures/m2-glob-lossless-source/low.source.utf16be"),
            include_bytes!("fixtures/m2-glob-lossless-source/low.value.utf16be"),
            include_bytes!("fixtures/m2-lossless-probe-adapters/low.value.java-utf8.bin"),
            &[0xdfff],
        ),
        (
            include_bytes!("fixtures/m2-glob-lossless-source/mixed.source.utf16be"),
            include_bytes!("fixtures/m2-glob-lossless-source/mixed.value.utf16be"),
            include_bytes!("fixtures/m2-lossless-probe-adapters/mixed.value.java-utf8.bin"),
            &[0x78, 0xd800, 0x79],
        ),
    ];
    for &(source, expected, expected_utf8, name) in cases {
        let name = PklString::from_utf16(name.to_vec());
        let uri = PklString::from("gmod:").concat(&name);
        let mut probe = SourceProbe::new_expressions_source(
            SourceText::from_utf16(units(source)),
            uri,
            name,
            &[SourceText::from("name")],
            "compact",
            false,
            false,
            false,
        )
        .unwrap();
        for _ in 0..2 {
            probe.start_path(0, 0, None).unwrap();
            let mut actual = None;
            for _ in 0..31_250 {
                let event = probe.poll_text(0, 64).unwrap();
                probe.collect();
                match event.kind {
                    "yield" => (),
                    "module-request" => complete(&mut probe, event.token.as_deref().unwrap()),
                    "value" => {
                        actual = Some(event.value.unwrap());
                        break;
                    }
                    _ => panic!("unexpected actual fixture event: {event:?}"),
                }
            }
            let actual = actual.expect("bounded fixture result");
            assert_eq!(actual, units(expected));
            // Java UTF8 is an explicit byte channel, separate from transport units.
            assert_eq!(
                PklString::from_utf16(actual).to_java_utf8().as_bytes(),
                expected_utf8
            );
        }
        probe.close();
        assert_eq!(probe.heap_slots(), 0);
    }
}

#[test]
fn established_scalar_probe_source_and_event_types_keep_actual_java_recovery_value() {
    let source = include_str!("../../../tests/fixtures/m2-imported-ast-owner-oracle/recovery.pkl");
    let uri = include_str!("../../../tests/fixtures/m2-imported-ast-owner-oracle/recovery-uri.txt");
    let expected = include_bytes!(
        "../../../tests/fixtures/m2-imported-ast-owner-oracle/observations/default/round-0/q0001/string.code-units.bin"
    );
    let mut probe = SourceProbe::new_module_evaluator("compact", false, false, false).unwrap();
    for _ in 0..2 {
        // Option<&str> is the established native source convenience.
        probe
            .evaluate_module_source(uri, Some(source), "stable", 0, None)
            .unwrap();
        let mut actual = None;
        for _ in 0..31_250 {
            let event: SourceEvent = probe.poll(0, 64).unwrap();
            let _: Option<String> = event.uri.clone();
            probe.collect();
            match event.kind {
                "yield" => (),
                "module-request" => complete(&mut probe, event.token.as_deref().unwrap()),
                "value" => {
                    actual = Some(event.value.unwrap());
                    break;
                }
                _ => panic!("unexpected scalar fixture event: {event:?}"),
            }
        }
        let actual = actual.expect("bounded scalar fixture result");
        assert_eq!(actual, units(expected));
        assert_eq!(
            PklString::from_utf16(actual).to_java_utf8().as_bytes(),
            include_bytes!(
                "../../../tests/fixtures/m2-imported-ast-owner-oracle/observations/default/round-0/q0001/string.bin"
            )
        );
    }
    probe.close();
    assert_eq!(probe.heap_slots(), 0);
}

#[test]
fn callback_uri_and_opaque_handle_keep_units_while_scalar_conversion_rejects_loss() {
    let uri_units = vec![0x67, 0x3a, 0xd800, 0x2f, 0xdfff];
    let handle_units = vec![0x68, 0x3a, 0xdfff, 0xd800];
    let raw = SourceTextModuleRequest::from(ModuleRequest {
        id: 7,
        kind: ModuleRequestKind::QueryCachePolicy {
            key: ModuleKey {
                id: ModuleKeyId(11),
                kind: ModuleKeyKind::Synthetic,
                uri: PklString::from_utf16(uri_units.clone()),
                handle: PklString::from_utf16(handle_units.clone()),
            },
        },
    });
    assert_eq!(
        raw.original_uri.encode_utf16().collect::<Vec<_>>(),
        uri_units
    );
    assert_eq!(
        raw.key_handle
            .as_ref()
            .unwrap()
            .encode_utf16()
            .collect::<Vec<_>>(),
        handle_units
    );
    assert_eq!(raw.id, "7");
    assert_eq!(raw.key_id.as_deref(), Some("11"));
    assert_eq!(
        SourceModuleRequest::try_from(raw).unwrap_err().kind,
        ErrorKind::Unsupported
    );
}

#[test]
fn actual_java_identifier_message_keeps_raw_error_units_before_legacy_byte_encoding() {
    let source = SourceText::from_utf16(units(include_bytes!(
        "fixtures/m2-supplementary-identifier/source.code-units.bin"
    )));
    let raw = pkl_runtime::probe::evaluate_member_text(source.clone(), "unused").unwrap_err();
    assert_eq!(
        raw.encode_utf16().collect::<Vec<_>>(),
        units(include_bytes!(
            "fixtures/m2-supplementary-identifier/parser/error.message.code-units.bin"
        ))
    );
    let byte_error = pkl_runtime::probe::evaluate_member(source, "unused").unwrap_err();
    assert_eq!(
        byte_error.as_bytes(),
        include_bytes!("fixtures/m2-supplementary-identifier/parser/error.message.bin")
    );
}
