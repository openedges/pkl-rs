//! Exact complete Java original outputs and separate signature-timing controls.
#[path = "support/module_keys.rs"]
mod module_keys;
use pkl_runtime::{
    evaluation::frontend::{ModuleEvalOptions, ModuleSourceInput},
    probe::{SourceEvent, SourceProbe},
};
fn next(probe: &mut SourceProbe) -> SourceEvent {
    for _ in 0..500_000 {
        probe.collect();
        let event = probe.poll(0, 1).unwrap();
        probe.collect();
        if event.kind == "yield" {
            continue;
        }
        if event.kind == "read" {
            probe
                .respond_configured(event.token.as_deref().unwrap(), 0)
                .unwrap();
            continue;
        }
        if event.kind == "module-request"
            && module_keys::complete_probe(probe, event.token.as_deref().unwrap(), 0).unwrap()
        {
            continue;
        }
        return event;
    }
    panic!("method timing exceeded continuation bound")
}
fn original(source: &str, name: &str, expected: &[u8]) {
    let uri = format!("file:///method-timing/{name}.pkl");
    let mut probe = SourceProbe::new_output(
        ModuleSourceInput {
            text: source,
            uri: &uri,
            inferred_name: name,
        },
        &[],
        &ModuleEvalOptions::default(),
        SourceProbe::options("compact", false, false, false).unwrap(),
    )
    .unwrap();
    for _ in 0..2 {
        probe.start_output_bytes(0, None).unwrap();
        let e = next(&mut probe);
        assert_eq!(e.kind, "bytes", "{e:?}");
        assert_eq!(e.bytes.as_deref(), Some(expected));
    }
    probe.close();
    assert_eq!(probe.collect().slots, 0);
}
#[test]
fn original_localmethodtyped() {
    original(
        include_str!("../../../tests/fixtures/m2-object-method-timing/localMethodTyped.pkl"),
        "localMethodTyped",
        include_bytes!(
            "../../../tests/fixtures/m2-object-method-timing/localMethodTyped.stdout.bin"
        ),
    );
}
#[test]
fn original_localmethodtyped2() {
    original(
        include_str!("../../../tests/fixtures/m2-object-method-timing/localMethodTyped2.pkl"),
        "localMethodTyped2",
        include_bytes!(
            "../../../tests/fixtures/m2-object-method-timing/localMethodTyped2.stdout.bin"
        ),
    );
}
#[test]
fn original_localmethodtyped3() {
    original(
        include_str!("../../../tests/fixtures/m2-object-method-timing/localMethodTyped3.pkl"),
        "localMethodTyped3",
        include_bytes!(
            "../../../tests/fixtures/m2-object-method-timing/localMethodTyped3.stdout.bin"
        ),
    );
}
#[test]
fn original_localmethodtyped4() {
    original(
        include_str!("../../../tests/fixtures/m2-object-method-timing/localMethodTyped4.pkl"),
        "localMethodTyped4",
        include_bytes!(
            "../../../tests/fixtures/m2-object-method-timing/localMethodTyped4.stdout.bin"
        ),
    );
}
#[test]
fn original_localmethodtyped5() {
    original(
        include_str!("../../../tests/fixtures/m2-object-method-timing/localMethodTyped5.pkl"),
        "localMethodTyped5",
        include_bytes!(
            "../../../tests/fixtures/m2-object-method-timing/localMethodTyped5.stdout.bin"
        ),
    );
}
#[test]
fn original_localmethodtyped6() {
    original(
        include_str!("../../../tests/fixtures/m2-object-method-timing/localMethodTyped6.pkl"),
        "localMethodTyped6",
        include_bytes!(
            "../../../tests/fixtures/m2-object-method-timing/localMethodTyped6.stdout.bin"
        ),
    );
}
#[test]
fn java_object_signature_timing() {
    let mut probe = SourceProbe::new_module_evaluator("compact", false, false, false).unwrap();
    probe
        .evaluate_module_source(
            "file:///stage68/object-method-object.pkl",
            Some(include_str!(
                "../../../tests/fixtures/m2-object-method-timing/object/module.pkl"
            )),
            "foo.safe",
            0,
            None,
        )
        .unwrap();
    let expected = include_bytes!(
        "../../../tests/fixtures/m2-object-method-timing/object/repeat-0/000-object-0/result.stdout.code-units.bin"
    );
    assert_eq!(expected.len() % 2, 0);
    let expected = expected
        .as_chunks::<2>()
        .0
        .iter()
        .map(|p| u16::from_be_bytes(*p))
        .collect::<Vec<_>>();
    let e = next(&mut probe);
    assert_eq!(e.kind, "value", "object/object-0: {e:?}");
    assert_eq!(e.value, Some(expected), "object/object-0");
    probe
        .evaluate_module_source(
            "file:///stage68/object-method-object.pkl",
            Some(include_str!(
                "../../../tests/fixtures/m2-object-method-timing/object/module.pkl"
            )),
            "foo.callValue",
            0,
            None,
        )
        .unwrap();
    let expected = include_bytes!(
        "../../../tests/fixtures/m2-object-method-timing/object/repeat-0/001-object-1/result.stderr.code-units.bin"
    );
    assert_eq!(expected.len() % 2, 0);
    let expected = expected
        .as_chunks::<2>()
        .0
        .iter()
        .map(|p| u16::from_be_bytes(*p))
        .collect::<Vec<_>>();
    let e = next(&mut probe);
    assert_eq!(e.kind, "error", "object/object-1: {e:?}");
    assert_eq!(e.error, Some(expected), "object/object-1");
    probe
        .evaluate_module_source(
            "file:///stage68/object-method-object.pkl",
            Some(include_str!(
                "../../../tests/fixtures/m2-object-method-timing/object/module.pkl"
            )),
            "foo.callThrow",
            0,
            None,
        )
        .unwrap();
    let expected = include_bytes!(
        "../../../tests/fixtures/m2-object-method-timing/object/repeat-0/002-object-2/result.stderr.code-units.bin"
    );
    assert_eq!(expected.len() % 2, 0);
    let expected = expected
        .as_chunks::<2>()
        .0
        .iter()
        .map(|p| u16::from_be_bytes(*p))
        .collect::<Vec<_>>();
    let e = next(&mut probe);
    assert_eq!(e.kind, "error", "object/object-2: {e:?}");
    assert_eq!(e.error, Some(expected), "object/object-2");
    probe
        .evaluate_module_source(
            "file:///stage68/object-method-object.pkl",
            Some(include_str!(
                "../../../tests/fixtures/m2-object-method-timing/object/module.pkl"
            )),
            "foo.nested.callValue",
            0,
            None,
        )
        .unwrap();
    let expected = include_bytes!(
        "../../../tests/fixtures/m2-object-method-timing/object/repeat-0/003-object-3/result.stderr.code-units.bin"
    );
    assert_eq!(expected.len() % 2, 0);
    let expected = expected
        .as_chunks::<2>()
        .0
        .iter()
        .map(|p| u16::from_be_bytes(*p))
        .collect::<Vec<_>>();
    let e = next(&mut probe);
    assert_eq!(e.kind, "error", "object/object-3: {e:?}");
    assert_eq!(e.error, Some(expected), "object/object-3");
    probe.close();
    assert_eq!(probe.collect().slots, 0);
}
#[test]
fn java_module_signature_timing() {
    let mut probe = SourceProbe::new_module_evaluator("compact", false, false, false).unwrap();
    probe
        .evaluate_module_source(
            "file:///stage68/object-method-module.pkl",
            Some(include_str!(
                "../../../tests/fixtures/m2-object-method-timing/module/module.pkl"
            )),
            "safe",
            0,
            None,
        )
        .unwrap();
    let expected = include_bytes!(
        "../../../tests/fixtures/m2-object-method-timing/module/repeat-0/000-module-0/result.stderr.code-units.bin"
    );
    assert_eq!(expected.len() % 2, 0);
    let expected = expected
        .as_chunks::<2>()
        .0
        .iter()
        .map(|p| u16::from_be_bytes(*p))
        .collect::<Vec<_>>();
    let e = next(&mut probe);
    assert_eq!(e.kind, "error", "module/module-0: {e:?}");
    assert_eq!(e.error, Some(expected), "module/module-0");
    probe
        .evaluate_module_source(
            "file:///stage68/object-method-module.pkl",
            Some(include_str!(
                "../../../tests/fixtures/m2-object-method-timing/module/module.pkl"
            )),
            "callValue",
            0,
            None,
        )
        .unwrap();
    let expected = include_bytes!(
        "../../../tests/fixtures/m2-object-method-timing/module/repeat-0/001-module-1/result.stderr.code-units.bin"
    );
    assert_eq!(expected.len() % 2, 0);
    let expected = expected
        .as_chunks::<2>()
        .0
        .iter()
        .map(|p| u16::from_be_bytes(*p))
        .collect::<Vec<_>>();
    let e = next(&mut probe);
    assert_eq!(e.kind, "error", "module/module-1: {e:?}");
    assert_eq!(e.error, Some(expected), "module/module-1");
    probe.close();
    assert_eq!(probe.collect().slots, 0);
}
#[test]
fn java_class_signature_timing() {
    let mut probe = SourceProbe::new_module_evaluator("compact", false, false, false).unwrap();
    probe
        .evaluate_module_source(
            "file:///stage68/object-method-class.pkl",
            Some(include_str!(
                "../../../tests/fixtures/m2-object-method-timing/class/module.pkl"
            )),
            "safe",
            0,
            None,
        )
        .unwrap();
    let expected = include_bytes!(
        "../../../tests/fixtures/m2-object-method-timing/class/repeat-0/000-class-0/result.stdout.code-units.bin"
    );
    assert_eq!(expected.len() % 2, 0);
    let expected = expected
        .as_chunks::<2>()
        .0
        .iter()
        .map(|p| u16::from_be_bytes(*p))
        .collect::<Vec<_>>();
    let e = next(&mut probe);
    assert_eq!(e.kind, "value", "class/class-0: {e:?}");
    assert_eq!(e.value, Some(expected), "class/class-0");
    probe
        .evaluate_module_source(
            "file:///stage68/object-method-class.pkl",
            Some(include_str!(
                "../../../tests/fixtures/m2-object-method-timing/class/module.pkl"
            )),
            "new C {}",
            0,
            None,
        )
        .unwrap();
    let expected = include_bytes!(
        "../../../tests/fixtures/m2-object-method-timing/class/repeat-0/001-class-1/result.stderr.code-units.bin"
    );
    assert_eq!(expected.len() % 2, 0);
    let expected = expected
        .as_chunks::<2>()
        .0
        .iter()
        .map(|p| u16::from_be_bytes(*p))
        .collect::<Vec<_>>();
    let e = next(&mut probe);
    assert_eq!(e.kind, "error", "class/class-1: {e:?}");
    assert_eq!(e.error, Some(expected), "class/class-1");
    probe.close();
    assert_eq!(probe.collect().slots, 0);
}

#[test]
fn java_object_signature_precedes_arity_after_arguments() {
    let mut probe = SourceProbe::new_module_evaluator("compact", false, false, false).unwrap();
    probe
        .evaluate_module_source(
            "file:///stage68/object-method-arity.pkl",
            Some(include_str!(
                "../../../tests/fixtures/m2-object-method-timing/arity/module.pkl"
            )),
            "foo.safe",
            0,
            None,
        )
        .unwrap();
    let expected = include_bytes!(
        "../../../tests/fixtures/m2-object-method-timing/arity/repeat-0/000-object-safe/result.stdout.code-units.bin"
    );
    let expected = expected
        .as_chunks::<2>()
        .0
        .iter()
        .map(|p| u16::from_be_bytes(*p))
        .collect::<Vec<_>>();
    let e = next(&mut probe);
    assert_eq!(e.kind, "value", "object-safe: {e:?}");
    assert_eq!(e.value, Some(expected), "object-safe");
    probe
        .evaluate_module_source(
            "file:///stage68/object-method-arity.pkl",
            Some(include_str!(
                "../../../tests/fixtures/m2-object-method-timing/arity/module.pkl"
            )),
            "foo.callOne",
            0,
            None,
        )
        .unwrap();
    let expected = include_bytes!(
        "../../../tests/fixtures/m2-object-method-timing/arity/repeat-0/001-object-one/result.stderr.code-units.bin"
    );
    let expected = expected
        .as_chunks::<2>()
        .0
        .iter()
        .map(|p| u16::from_be_bytes(*p))
        .collect::<Vec<_>>();
    let e = next(&mut probe);
    assert_eq!(e.kind, "error", "object-one: {e:?}");
    assert_eq!(e.error, Some(expected), "object-one");
    probe
        .evaluate_module_source(
            "file:///stage68/object-method-arity.pkl",
            Some(include_str!(
                "../../../tests/fixtures/m2-object-method-timing/arity/module.pkl"
            )),
            "foo.callZero",
            0,
            None,
        )
        .unwrap();
    let expected = include_bytes!(
        "../../../tests/fixtures/m2-object-method-timing/arity/repeat-0/002-object-zero/result.stderr.code-units.bin"
    );
    let expected = expected
        .as_chunks::<2>()
        .0
        .iter()
        .map(|p| u16::from_be_bytes(*p))
        .collect::<Vec<_>>();
    let e = next(&mut probe);
    assert_eq!(e.kind, "error", "object-zero: {e:?}");
    assert_eq!(e.error, Some(expected), "object-zero");
    probe
        .evaluate_module_source(
            "file:///stage68/object-method-arity.pkl",
            Some(include_str!(
                "../../../tests/fixtures/m2-object-method-timing/arity/module.pkl"
            )),
            "foo.callMany",
            0,
            None,
        )
        .unwrap();
    let expected = include_bytes!(
        "../../../tests/fixtures/m2-object-method-timing/arity/repeat-0/003-object-many/result.stderr.code-units.bin"
    );
    let expected = expected
        .as_chunks::<2>()
        .0
        .iter()
        .map(|p| u16::from_be_bytes(*p))
        .collect::<Vec<_>>();
    let e = next(&mut probe);
    assert_eq!(e.kind, "error", "object-many: {e:?}");
    assert_eq!(e.error, Some(expected), "object-many");
    probe
        .evaluate_module_source(
            "file:///stage68/object-method-arity.pkl",
            Some(include_str!(
                "../../../tests/fixtures/m2-object-method-timing/arity/module.pkl"
            )),
            "foo.callManyThrow",
            0,
            None,
        )
        .unwrap();
    let expected = include_bytes!(
        "../../../tests/fixtures/m2-object-method-timing/arity/repeat-0/004-object-many-throw/result.stderr.code-units.bin"
    );
    let expected = expected
        .as_chunks::<2>()
        .0
        .iter()
        .map(|p| u16::from_be_bytes(*p))
        .collect::<Vec<_>>();
    let e = next(&mut probe);
    assert_eq!(e.kind, "error", "object-many-throw: {e:?}");
    assert_eq!(e.error, Some(expected), "object-many-throw");
    probe.close();
    assert_eq!(probe.collect().slots, 0);
}

#[test]
fn java_object_signature_and_dynamic_constraint_receiver() {
    let mut probe = SourceProbe::new_module_evaluator("compact", false, false, false).unwrap();
    probe
        .evaluate_module_source(
            "file:///stage68/object-method-owner.pkl",
            Some(include_str!(
                "../../../tests/fixtures/m2-object-method-timing/owner/module.pkl"
            )),
            "prototype.limit",
            0,
            None,
        )
        .unwrap();
    let expected = include_bytes!(
        "../../../tests/fixtures/m2-object-method-timing/owner/repeat-0/000-prototype-limit/result.stdout.code-units.bin"
    );
    let expected = expected
        .as_chunks::<2>()
        .0
        .iter()
        .map(|p| u16::from_be_bytes(*p))
        .collect::<Vec<_>>();
    let e = next(&mut probe);
    assert_eq!(e.kind, "value", "prototype-limit: {e:?}");
    assert_eq!(e.value, Some(expected), "prototype-limit");
    probe
        .evaluate_module_source(
            "file:///stage68/object-method-owner.pkl",
            Some(include_str!(
                "../../../tests/fixtures/m2-object-method-timing/owner/module.pkl"
            )),
            "derived.limit",
            0,
            None,
        )
        .unwrap();
    let expected = include_bytes!(
        "../../../tests/fixtures/m2-object-method-timing/owner/repeat-0/001-derived-limit/result.stdout.code-units.bin"
    );
    let expected = expected
        .as_chunks::<2>()
        .0
        .iter()
        .map(|p| u16::from_be_bytes(*p))
        .collect::<Vec<_>>();
    let e = next(&mut probe);
    assert_eq!(e.kind, "value", "derived-limit: {e:?}");
    assert_eq!(e.value, Some(expected), "derived-limit");
    probe
        .evaluate_module_source(
            "file:///stage68/object-method-owner.pkl",
            Some(include_str!(
                "../../../tests/fixtures/m2-object-method-timing/owner/module.pkl"
            )),
            "derived.good",
            0,
            None,
        )
        .unwrap();
    let expected = include_bytes!(
        "../../../tests/fixtures/m2-object-method-timing/owner/repeat-0/002-derived-good-first/result.stdout.code-units.bin"
    );
    let expected = expected
        .as_chunks::<2>()
        .0
        .iter()
        .map(|p| u16::from_be_bytes(*p))
        .collect::<Vec<_>>();
    let e = next(&mut probe);
    assert_eq!(e.kind, "value", "derived-good-first: {e:?}");
    assert_eq!(e.value, Some(expected), "derived-good-first");
    probe
        .evaluate_module_source(
            "file:///stage68/object-method-owner.pkl",
            Some(include_str!(
                "../../../tests/fixtures/m2-object-method-timing/owner/module.pkl"
            )),
            "prototype.good",
            0,
            None,
        )
        .unwrap();
    let expected = include_bytes!(
        "../../../tests/fixtures/m2-object-method-timing/owner/repeat-0/003-prototype-good/result.stdout.code-units.bin"
    );
    let expected = expected
        .as_chunks::<2>()
        .0
        .iter()
        .map(|p| u16::from_be_bytes(*p))
        .collect::<Vec<_>>();
    let e = next(&mut probe);
    assert_eq!(e.kind, "value", "prototype-good: {e:?}");
    assert_eq!(e.value, Some(expected), "prototype-good");
    probe
        .evaluate_module_source(
            "file:///stage68/object-method-owner.pkl",
            Some(include_str!(
                "../../../tests/fixtures/m2-object-method-timing/owner/module.pkl"
            )),
            "derived.result",
            0,
            None,
        )
        .unwrap();
    let expected = include_bytes!(
        "../../../tests/fixtures/m2-object-method-timing/owner/repeat-0/004-derived-result/result.stderr.code-units.bin"
    );
    let expected = expected
        .as_chunks::<2>()
        .0
        .iter()
        .map(|p| u16::from_be_bytes(*p))
        .collect::<Vec<_>>();
    let e = next(&mut probe);
    assert_eq!(e.kind, "error", "derived-result: {e:?}");
    assert_eq!(e.error, Some(expected), "derived-result");
    probe
        .evaluate_module_source(
            "file:///stage68/object-method-owner.pkl",
            Some(include_str!(
                "../../../tests/fixtures/m2-object-method-timing/owner/module.pkl"
            )),
            "prototype.result",
            0,
            None,
        )
        .unwrap();
    let expected = include_bytes!(
        "../../../tests/fixtures/m2-object-method-timing/owner/repeat-0/005-prototype-result/result.stdout.code-units.bin"
    );
    let expected = expected
        .as_chunks::<2>()
        .0
        .iter()
        .map(|p| u16::from_be_bytes(*p))
        .collect::<Vec<_>>();
    let e = next(&mut probe);
    assert_eq!(e.kind, "value", "prototype-result: {e:?}");
    assert_eq!(e.value, Some(expected), "prototype-result");
    probe
        .evaluate_module_source(
            "file:///stage68/object-method-owner.pkl",
            Some(include_str!(
                "../../../tests/fixtures/m2-object-method-timing/owner/module.pkl"
            )),
            "derived.bad",
            0,
            None,
        )
        .unwrap();
    let expected = include_bytes!(
        "../../../tests/fixtures/m2-object-method-timing/owner/repeat-0/006-derived-bad/result.stderr.code-units.bin"
    );
    let expected = expected
        .as_chunks::<2>()
        .0
        .iter()
        .map(|p| u16::from_be_bytes(*p))
        .collect::<Vec<_>>();
    let e = next(&mut probe);
    assert_eq!(e.kind, "error", "derived-bad: {e:?}");
    assert_eq!(e.error, Some(expected), "derived-bad");
    probe
        .evaluate_module_source(
            "file:///stage68/object-method-owner.pkl",
            Some(include_str!(
                "../../../tests/fixtures/m2-object-method-timing/owner/module.pkl"
            )),
            "derived.good",
            0,
            None,
        )
        .unwrap();
    let expected = include_bytes!(
        "../../../tests/fixtures/m2-object-method-timing/owner/repeat-0/007-derived-good-after-failure/result.stdout.code-units.bin"
    );
    let expected = expected
        .as_chunks::<2>()
        .0
        .iter()
        .map(|p| u16::from_be_bytes(*p))
        .collect::<Vec<_>>();
    let e = next(&mut probe);
    assert_eq!(e.kind, "value", "derived-good-after-failure: {e:?}");
    assert_eq!(e.value, Some(expected), "derived-good-after-failure");
    probe
        .evaluate_module_source(
            "file:///stage68/object-method-owner.pkl",
            Some(include_str!(
                "../../../tests/fixtures/m2-object-method-timing/owner/module.pkl"
            )),
            "prototype.result",
            0,
            None,
        )
        .unwrap();
    let expected = include_bytes!(
        "../../../tests/fixtures/m2-object-method-timing/owner/repeat-0/008-prototype-result-after-failure/result.stdout.code-units.bin"
    );
    let expected = expected
        .as_chunks::<2>()
        .0
        .iter()
        .map(|p| u16::from_be_bytes(*p))
        .collect::<Vec<_>>();
    let e = next(&mut probe);
    assert_eq!(e.kind, "value", "prototype-result-after-failure: {e:?}");
    assert_eq!(e.value, Some(expected), "prototype-result-after-failure");
    probe.close();
    assert_eq!(probe.collect().slots, 0);
}
