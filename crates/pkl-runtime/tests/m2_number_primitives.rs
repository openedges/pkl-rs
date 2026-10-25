//! Pinned Java 0.32.0 numeric authority, two independent module/evaluator profiles.
//! Java status and UTF-8/UTF-16 result bytes are copied, never generated from Rust.
//! Java exception/cause classes and both raw JVM repeats are retained in the oracle fixture.
#[path = "support/module_keys.rs"]
mod module_keys;
use pkl_runtime::probe::SourceProbe;

type JavaRow<'a> = (&'a str, &'a str, &'a str, &'a [u8], &'a [u8]);

fn run_profile(uri: &str, module: &str, rows: &[JavaRow<'_>]) {
    let mut probe = SourceProbe::new_module_evaluator("compact", false, false, false).unwrap();
    let mut failures = Vec::new();
    for &(id, expression, expected_kind, expected_be, expected_utf8) in rows {
        probe
            .evaluate_module_source(uri, Some(module), expression, 0, None)
            .unwrap();
        let mut terminal = None;
        for _ in 0..200_000 {
            probe.collect();
            let event = probe.poll(0, 16).unwrap();
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
            terminal = Some(event);
            break;
        }
        let event = terminal.expect("number primitive query poll bound");
        let actual = if event.kind == "value" {
            event.value
        } else {
            event.error
        };
        let (pairs, remainder) = expected_be.as_chunks::<2>();
        assert!(remainder.is_empty(), "odd Java UTF-16BE result for {id}");
        let expected_units: Vec<u16> = pairs.iter().map(|pair| u16::from_be_bytes(*pair)).collect();
        let utf16_matches = actual.as_ref() == Some(&expected_units);
        let utf8_matches = actual
            .as_ref()
            .and_then(|units| String::from_utf16(units).ok())
            .is_some_and(|text| text.as_bytes() == expected_utf8);
        if event.kind != expected_kind || !utf16_matches || !utf8_matches {
            failures.push(format!(
                "{id}: {expression}: expected {expected_kind}/{} UTF-16 units/{} UTF-8 bytes, got {}/{} units",
                expected_units.len(), expected_utf8.len(), event.kind, actual.as_ref().map_or(0, Vec::len)
            ));
        }
    }
    probe.close();
    assert_eq!(probe.collect().slots, 0);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn java_main48_matrix() {
    let rows: [JavaRow<'_>; 48] = [
        (
            r########"int-sign-pos"########,
            r########"123.sign"########,
            "value",
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/main48/expected/000-int-sign-pos.utf16be.bin"
            ) as &[u8],
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/main48/expected/000-int-sign-pos.utf8.bin"
            ) as &[u8],
        ),
        (
            r########"int-sign-neg"########,
            r########"(-123).sign"########,
            "value",
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/main48/expected/001-int-sign-neg.utf16be.bin"
            ) as &[u8],
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/main48/expected/001-int-sign-neg.utf8.bin"
            ) as &[u8],
        ),
        (
            r########"int-sign-zero"########,
            r########"0.sign"########,
            "value",
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/main48/expected/002-int-sign-zero.utf16be.bin"
            ) as &[u8],
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/main48/expected/002-int-sign-zero.utf8.bin"
            ) as &[u8],
        ),
        (
            r########"int-abs-neg"########,
            r########"(-123).abs"########,
            "value",
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/main48/expected/003-int-abs-neg.utf16be.bin"
            ) as &[u8],
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/main48/expected/003-int-abs-neg.utf8.bin"
            ) as &[u8],
        ),
        (
            r########"int-abs-min"########,
            r########"module.minimumInt.abs"########,
            "error",
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/main48/expected/004-int-abs-min.utf16be.bin"
            ) as &[u8],
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/main48/expected/004-int-abs-min.utf8.bin"
            ) as &[u8],
        ),
        (
            r########"int-ceil"########,
            r########"(-123).ceil"########,
            "value",
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/main48/expected/005-int-ceil.utf16be.bin"
            ) as &[u8],
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/main48/expected/005-int-ceil.utf8.bin"
            ) as &[u8],
        ),
        (
            r########"int-floor"########,
            r########"123.floor"########,
            "value",
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/main48/expected/006-int-floor.utf16be.bin"
            ) as &[u8],
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/main48/expected/006-int-floor.utf8.bin"
            ) as &[u8],
        ),
        (
            r########"int-round"########,
            r########"(-123).round()"########,
            "value",
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/main48/expected/007-int-round.utf16be.bin"
            ) as &[u8],
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/main48/expected/007-int-round.utf8.bin"
            ) as &[u8],
        ),
        (
            r########"int-truncate"########,
            r########"123.truncate()"########,
            "value",
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/main48/expected/008-int-truncate.utf16be.bin"
            ) as &[u8],
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/main48/expected/008-int-truncate.utf8.bin"
            ) as &[u8],
        ),
        (
            r########"int-sign-as-method"########,
            r########"123.sign()"########,
            "error",
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/main48/expected/009-int-sign-as-method.utf16be.bin"
            ) as &[u8],
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/main48/expected/009-int-sign-as-method.utf8.bin"
            ) as &[u8],
        ),
        (
            r########"int-round-as-property"########,
            r########"123.round"########,
            "error",
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/main48/expected/010-int-round-as-property.utf16be.bin"
            ) as &[u8],
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/main48/expected/010-int-round-as-property.utf8.bin"
            ) as &[u8],
        ),
        (
            r########"int-round-extra-arg"########,
            r########"123.round(1)"########,
            "error",
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/main48/expected/011-int-round-extra-arg.utf16be.bin"
            ) as &[u8],
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/main48/expected/011-int-round-extra-arg.utf8.bin"
            ) as &[u8],
        ),
        (
            r########"int-truncate-extra-arg"########,
            r########"123.truncate(1)"########,
            "error",
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/main48/expected/012-int-truncate-extra-arg.utf16be.bin"
            ) as &[u8],
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/main48/expected/012-int-truncate-extra-arg.utf8.bin"
            ) as &[u8],
        ),
        (
            r########"float-sign-pos"########,
            r########"2.5.sign"########,
            "value",
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/main48/expected/013-float-sign-pos.utf16be.bin"
            ) as &[u8],
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/main48/expected/013-float-sign-pos.utf8.bin"
            ) as &[u8],
        ),
        (
            r########"float-sign-neg"########,
            r########"(-2.5).sign"########,
            "value",
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/main48/expected/014-float-sign-neg.utf16be.bin"
            ) as &[u8],
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/main48/expected/014-float-sign-neg.utf8.bin"
            ) as &[u8],
        ),
        (
            r########"float-sign-plus-zero"########,
            r########"0.0.sign"########,
            "value",
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/main48/expected/015-float-sign-plus-zero.utf16be.bin"
            ) as &[u8],
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/main48/expected/015-float-sign-plus-zero.utf8.bin"
            ) as &[u8],
        ),
        (
            r########"float-sign-minus-zero"########,
            r########"(-0.0).sign"########,
            "value",
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/main48/expected/016-float-sign-minus-zero.utf16be.bin"
            ) as &[u8],
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/main48/expected/016-float-sign-minus-zero.utf8.bin"
            ) as &[u8],
        ),
        (
            r########"float-sign-nan"########,
            r########"NaN.sign"########,
            "value",
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/main48/expected/017-float-sign-nan.utf16be.bin"
            ) as &[u8],
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/main48/expected/017-float-sign-nan.utf8.bin"
            ) as &[u8],
        ),
        (
            r########"float-sign-pos-inf"########,
            r########"Infinity.sign"########,
            "value",
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/main48/expected/018-float-sign-pos-inf.utf16be.bin"
            ) as &[u8],
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/main48/expected/018-float-sign-pos-inf.utf8.bin"
            ) as &[u8],
        ),
        (
            r########"float-sign-neg-inf"########,
            r########"(-Infinity).sign"########,
            "value",
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/main48/expected/019-float-sign-neg-inf.utf16be.bin"
            ) as &[u8],
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/main48/expected/019-float-sign-neg-inf.utf8.bin"
            ) as &[u8],
        ),
        (
            r########"float-abs-minus-zero"########,
            r########"(-0.0).abs"########,
            "value",
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/main48/expected/020-float-abs-minus-zero.utf16be.bin"
            ) as &[u8],
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/main48/expected/020-float-abs-minus-zero.utf8.bin"
            ) as &[u8],
        ),
        (
            r########"float-abs-nan"########,
            r########"NaN.abs"########,
            "value",
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/main48/expected/021-float-abs-nan.utf16be.bin"
            ) as &[u8],
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/main48/expected/021-float-abs-nan.utf8.bin"
            ) as &[u8],
        ),
        (
            r########"float-abs-neg-inf"########,
            r########"(-Infinity).abs"########,
            "value",
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/main48/expected/022-float-abs-neg-inf.utf16be.bin"
            ) as &[u8],
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/main48/expected/022-float-abs-neg-inf.utf8.bin"
            ) as &[u8],
        ),
        (
            r########"float-ceil-minus-zero"########,
            r########"(-0.0).ceil"########,
            "value",
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/main48/expected/023-float-ceil-minus-zero.utf16be.bin"
            ) as &[u8],
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/main48/expected/023-float-ceil-minus-zero.utf8.bin"
            ) as &[u8],
        ),
        (
            r########"float-ceil-small-neg"########,
            r########"(-0.2).ceil"########,
            "value",
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/main48/expected/024-float-ceil-small-neg.utf16be.bin"
            ) as &[u8],
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/main48/expected/024-float-ceil-small-neg.utf8.bin"
            ) as &[u8],
        ),
        (
            r########"float-floor-plus-zero"########,
            r########"0.0.floor"########,
            "value",
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/main48/expected/025-float-floor-plus-zero.utf16be.bin"
            ) as &[u8],
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/main48/expected/025-float-floor-plus-zero.utf8.bin"
            ) as &[u8],
        ),
        (
            r########"float-floor-small-pos"########,
            r########"0.2.floor"########,
            "value",
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/main48/expected/026-float-floor-small-pos.utf16be.bin"
            ) as &[u8],
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/main48/expected/026-float-floor-small-pos.utf8.bin"
            ) as &[u8],
        ),
        (
            r########"float-round-half-zero"########,
            r########"0.5.round()"########,
            "value",
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/main48/expected/027-float-round-half-zero.utf16be.bin"
            ) as &[u8],
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/main48/expected/027-float-round-half-zero.utf8.bin"
            ) as &[u8],
        ),
        (
            r########"float-round-half-negzero"########,
            r########"(-0.5).round()"########,
            "value",
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/main48/expected/028-float-round-half-negzero.utf16be.bin"
            ) as &[u8],
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/main48/expected/028-float-round-half-negzero.utf8.bin"
            ) as &[u8],
        ),
        (
            r########"float-round-half-even"########,
            r########"2.5.round()"########,
            "value",
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/main48/expected/029-float-round-half-even.utf16be.bin"
            ) as &[u8],
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/main48/expected/029-float-round-half-even.utf8.bin"
            ) as &[u8],
        ),
        (
            r########"float-round-half-odd"########,
            r########"3.5.round()"########,
            "value",
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/main48/expected/030-float-round-half-odd.utf16be.bin"
            ) as &[u8],
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/main48/expected/030-float-round-half-odd.utf8.bin"
            ) as &[u8],
        ),
        (
            r########"float-round-neg-half-even"########,
            r########"(-2.5).round()"########,
            "value",
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/main48/expected/031-float-round-neg-half-even.utf16be.bin"
            ) as &[u8],
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/main48/expected/031-float-round-neg-half-even.utf8.bin"
            ) as &[u8],
        ),
        (
            r########"float-round-neg-half-odd"########,
            r########"(-3.5).round()"########,
            "value",
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/main48/expected/032-float-round-neg-half-odd.utf16be.bin"
            ) as &[u8],
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/main48/expected/032-float-round-neg-half-odd.utf8.bin"
            ) as &[u8],
        ),
        (
            r########"float-round-nan"########,
            r########"NaN.round()"########,
            "value",
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/main48/expected/033-float-round-nan.utf16be.bin"
            ) as &[u8],
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/main48/expected/033-float-round-nan.utf8.bin"
            ) as &[u8],
        ),
        (
            r########"float-round-inf"########,
            r########"Infinity.round()"########,
            "value",
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/main48/expected/034-float-round-inf.utf16be.bin"
            ) as &[u8],
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/main48/expected/034-float-round-inf.utf8.bin"
            ) as &[u8],
        ),
        (
            r########"float-truncate-small-neg"########,
            r########"(-0.2).truncate()"########,
            "value",
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/main48/expected/035-float-truncate-small-neg.utf16be.bin"
            ) as &[u8],
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/main48/expected/035-float-truncate-small-neg.utf8.bin"
            ) as &[u8],
        ),
        (
            r########"float-truncate-minus-zero"########,
            r########"(-0.0).truncate()"########,
            "value",
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/main48/expected/036-float-truncate-minus-zero.utf16be.bin"
            ) as &[u8],
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/main48/expected/036-float-truncate-minus-zero.utf8.bin"
            ) as &[u8],
        ),
        (
            r########"float-truncate-nan"########,
            r########"NaN.truncate()"########,
            "value",
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/main48/expected/037-float-truncate-nan.utf16be.bin"
            ) as &[u8],
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/main48/expected/037-float-truncate-nan.utf8.bin"
            ) as &[u8],
        ),
        (
            r########"float-truncate-neg-inf"########,
            r########"(-Infinity).truncate()"########,
            "value",
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/main48/expected/038-float-truncate-neg-inf.utf16be.bin"
            ) as &[u8],
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/main48/expected/038-float-truncate-neg-inf.utf8.bin"
            ) as &[u8],
        ),
        (
            r########"float-sign-as-method"########,
            r########"2.5.sign()"########,
            "error",
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/main48/expected/039-float-sign-as-method.utf16be.bin"
            ) as &[u8],
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/main48/expected/039-float-sign-as-method.utf8.bin"
            ) as &[u8],
        ),
        (
            r########"float-round-as-property"########,
            r########"2.5.round"########,
            "error",
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/main48/expected/040-float-round-as-property.utf16be.bin"
            ) as &[u8],
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/main48/expected/040-float-round-as-property.utf8.bin"
            ) as &[u8],
        ),
        (
            r########"float-round-extra-arg"########,
            r########"2.5.round(1)"########,
            "error",
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/main48/expected/041-float-round-extra-arg.utf16be.bin"
            ) as &[u8],
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/main48/expected/041-float-round-extra-arg.utf8.bin"
            ) as &[u8],
        ),
        (
            r########"ref-int-sign-type"########,
            r########"module.refInt.sign is ref.Reference<D, Int>"########,
            "value",
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/main48/expected/042-ref-int-sign-type.utf16be.bin"
            ) as &[u8],
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/main48/expected/042-ref-int-sign-type.utf8.bin"
            ) as &[u8],
        ),
        (
            r########"ref-int-abs-type"########,
            r########"module.refInt.abs is ref.Reference<D, Int>"########,
            "value",
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/main48/expected/043-ref-int-abs-type.utf16be.bin"
            ) as &[u8],
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/main48/expected/043-ref-int-abs-type.utf8.bin"
            ) as &[u8],
        ),
        (
            r########"ref-float-sign-type"########,
            r########"module.refFloat.sign is ref.Reference<D, Float>"########,
            "value",
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/main48/expected/044-ref-float-sign-type.utf16be.bin"
            ) as &[u8],
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/main48/expected/044-ref-float-sign-type.utf8.bin"
            ) as &[u8],
        ),
        (
            r########"ref-float-floor-type"########,
            r########"module.refFloat.floor is ref.Reference<D, Float>"########,
            "value",
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/main48/expected/045-ref-float-floor-type.utf16be.bin"
            ) as &[u8],
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/main48/expected/045-ref-float-floor-type.utf8.bin"
            ) as &[u8],
        ),
        (
            r########"ref-int-round-method"########,
            r########"module.refInt.round()"########,
            "error",
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/main48/expected/046-ref-int-round-method.utf16be.bin"
            ) as &[u8],
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/main48/expected/046-ref-int-round-method.utf8.bin"
            ) as &[u8],
        ),
        (
            r########"ref-float-truncate-method"########,
            r########"module.refFloat.truncate()"########,
            "error",
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/main48/expected/047-ref-float-truncate-method.utf16be.bin"
            ) as &[u8],
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/main48/expected/047-ref-float-truncate-method.utf8.bin"
            ) as &[u8],
        ),
    ];
    run_profile(
        "file:///stage78/number-primitives.pkl",
        include_str!("../../../tests/fixtures/m2-number-primitives/main48/module.pkl"),
        &rows,
    );
}

#[test]
fn java_reference24_matrix() {
    let rows: [JavaRow<'_>; 24] = [
        (
            r########"int-sign-not-float"########,
            r########"module.refInt.sign is ref.Reference<D, Float>"########,
            "value",
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/reference24/expected/000-int-sign-not-float.utf16be.bin"
            ) as &[u8],
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/reference24/expected/000-int-sign-not-float.utf8.bin"
            ) as &[u8],
        ),
        (
            r########"int-sign-is-number"########,
            r########"module.refInt.sign is ref.Reference<D, Number>"########,
            "value",
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/reference24/expected/001-int-sign-is-number.utf16be.bin"
            ) as &[u8],
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/reference24/expected/001-int-sign-is-number.utf8.bin"
            ) as &[u8],
        ),
        (
            r########"int-abs-not-float"########,
            r########"module.refInt.abs is ref.Reference<D, Float>"########,
            "value",
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/reference24/expected/002-int-abs-not-float.utf16be.bin"
            ) as &[u8],
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/reference24/expected/002-int-abs-not-float.utf8.bin"
            ) as &[u8],
        ),
        (
            r########"int-abs-is-number"########,
            r########"module.refInt.abs is ref.Reference<D, Number>"########,
            "value",
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/reference24/expected/003-int-abs-is-number.utf16be.bin"
            ) as &[u8],
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/reference24/expected/003-int-abs-is-number.utf8.bin"
            ) as &[u8],
        ),
        (
            r########"float-sign-not-int"########,
            r########"module.refFloat.sign is ref.Reference<D, Int>"########,
            "value",
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/reference24/expected/004-float-sign-not-int.utf16be.bin"
            ) as &[u8],
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/reference24/expected/004-float-sign-not-int.utf8.bin"
            ) as &[u8],
        ),
        (
            r########"float-sign-is-number"########,
            r########"module.refFloat.sign is ref.Reference<D, Number>"########,
            "value",
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/reference24/expected/005-float-sign-is-number.utf16be.bin"
            ) as &[u8],
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/reference24/expected/005-float-sign-is-number.utf8.bin"
            ) as &[u8],
        ),
        (
            r########"float-floor-not-int"########,
            r########"module.refFloat.floor is ref.Reference<D, Int>"########,
            "value",
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/reference24/expected/006-float-floor-not-int.utf16be.bin"
            ) as &[u8],
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/reference24/expected/006-float-floor-not-int.utf8.bin"
            ) as &[u8],
        ),
        (
            r########"float-floor-is-number"########,
            r########"module.refFloat.floor is ref.Reference<D, Number>"########,
            "value",
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/reference24/expected/007-float-floor-is-number.utf16be.bin"
            ) as &[u8],
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/reference24/expected/007-float-floor-is-number.utf8.bin"
            ) as &[u8],
        ),
        (
            r########"number-sign-is-number"########,
            r########"module.refNumber.sign is ref.Reference<D, Number>"########,
            "value",
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/reference24/expected/008-number-sign-is-number.utf16be.bin"
            ) as &[u8],
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/reference24/expected/008-number-sign-is-number.utf8.bin"
            ) as &[u8],
        ),
        (
            r########"number-sign-is-int"########,
            r########"module.refNumber.sign is ref.Reference<D, Int>"########,
            "value",
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/reference24/expected/009-number-sign-is-int.utf16be.bin"
            ) as &[u8],
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/reference24/expected/009-number-sign-is-int.utf8.bin"
            ) as &[u8],
        ),
        (
            r########"number-sign-is-float"########,
            r########"module.refNumber.sign is ref.Reference<D, Float>"########,
            "value",
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/reference24/expected/010-number-sign-is-float.utf16be.bin"
            ) as &[u8],
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/reference24/expected/010-number-sign-is-float.utf8.bin"
            ) as &[u8],
        ),
        (
            r########"number-sign-is-string"########,
            r########"module.refNumber.sign is ref.Reference<D, String>"########,
            "value",
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/reference24/expected/011-number-sign-is-string.utf16be.bin"
            ) as &[u8],
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/reference24/expected/011-number-sign-is-string.utf8.bin"
            ) as &[u8],
        ),
        (
            r########"number-abs-is-number"########,
            r########"module.refNumber.abs is ref.Reference<D, Number>"########,
            "value",
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/reference24/expected/012-number-abs-is-number.utf16be.bin"
            ) as &[u8],
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/reference24/expected/012-number-abs-is-number.utf8.bin"
            ) as &[u8],
        ),
        (
            r########"number-abs-is-int"########,
            r########"module.refNumber.abs is ref.Reference<D, Int>"########,
            "value",
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/reference24/expected/013-number-abs-is-int.utf16be.bin"
            ) as &[u8],
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/reference24/expected/013-number-abs-is-int.utf8.bin"
            ) as &[u8],
        ),
        (
            r########"number-abs-is-float"########,
            r########"module.refNumber.abs is ref.Reference<D, Float>"########,
            "value",
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/reference24/expected/014-number-abs-is-float.utf16be.bin"
            ) as &[u8],
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/reference24/expected/014-number-abs-is-float.utf8.bin"
            ) as &[u8],
        ),
        (
            r########"number-abs-is-string"########,
            r########"module.refNumber.abs is ref.Reference<D, String>"########,
            "value",
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/reference24/expected/015-number-abs-is-string.utf16be.bin"
            ) as &[u8],
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/reference24/expected/015-number-abs-is-string.utf8.bin"
            ) as &[u8],
        ),
        (
            r########"number-ceil-is-number"########,
            r########"module.refNumber.ceil is ref.Reference<D, Number>"########,
            "value",
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/reference24/expected/016-number-ceil-is-number.utf16be.bin"
            ) as &[u8],
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/reference24/expected/016-number-ceil-is-number.utf8.bin"
            ) as &[u8],
        ),
        (
            r########"number-ceil-is-int"########,
            r########"module.refNumber.ceil is ref.Reference<D, Int>"########,
            "value",
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/reference24/expected/017-number-ceil-is-int.utf16be.bin"
            ) as &[u8],
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/reference24/expected/017-number-ceil-is-int.utf8.bin"
            ) as &[u8],
        ),
        (
            r########"number-ceil-is-float"########,
            r########"module.refNumber.ceil is ref.Reference<D, Float>"########,
            "value",
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/reference24/expected/018-number-ceil-is-float.utf16be.bin"
            ) as &[u8],
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/reference24/expected/018-number-ceil-is-float.utf8.bin"
            ) as &[u8],
        ),
        (
            r########"number-ceil-is-string"########,
            r########"module.refNumber.ceil is ref.Reference<D, String>"########,
            "value",
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/reference24/expected/019-number-ceil-is-string.utf16be.bin"
            ) as &[u8],
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/reference24/expected/019-number-ceil-is-string.utf8.bin"
            ) as &[u8],
        ),
        (
            r########"number-floor-is-number"########,
            r########"module.refNumber.floor is ref.Reference<D, Number>"########,
            "value",
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/reference24/expected/020-number-floor-is-number.utf16be.bin"
            ) as &[u8],
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/reference24/expected/020-number-floor-is-number.utf8.bin"
            ) as &[u8],
        ),
        (
            r########"number-floor-is-int"########,
            r########"module.refNumber.floor is ref.Reference<D, Int>"########,
            "value",
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/reference24/expected/021-number-floor-is-int.utf16be.bin"
            ) as &[u8],
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/reference24/expected/021-number-floor-is-int.utf8.bin"
            ) as &[u8],
        ),
        (
            r########"number-floor-is-float"########,
            r########"module.refNumber.floor is ref.Reference<D, Float>"########,
            "value",
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/reference24/expected/022-number-floor-is-float.utf16be.bin"
            ) as &[u8],
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/reference24/expected/022-number-floor-is-float.utf8.bin"
            ) as &[u8],
        ),
        (
            r########"number-floor-is-string"########,
            r########"module.refNumber.floor is ref.Reference<D, String>"########,
            "value",
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/reference24/expected/023-number-floor-is-string.utf16be.bin"
            ) as &[u8],
            include_bytes!(
                "../../../tests/fixtures/m2-number-primitives/reference24/expected/023-number-floor-is-string.utf8.bin"
            ) as &[u8],
        ),
    ];
    run_profile(
        "file:///stage78/number-reference-supplement.pkl",
        include_str!("../../../tests/fixtures/m2-number-primitives/reference24/module.pkl"),
        &rows,
    );
}
