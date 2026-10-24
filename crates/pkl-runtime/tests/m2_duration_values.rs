//! Complete string boundary observations from the pinned Java 0.32.0 oracle.
//! Expected values and diagnostics are copied byte-for-byte from the independently
//! acquired capture-02/repeat-0; repeat-1 matched in the fixture validator.
//! Authority: tests/fixtures/m2-duration-values/oracle-evidence.tar.gz.
#[path = "support/module_keys.rs"]
mod module_keys;

use pkl_runtime::probe::{SourceEvent, SourceProbe};

fn query(probe: &mut SourceProbe, expression: &str) -> SourceEvent {
    probe
        .evaluate_module_source(
            "file:///stage59/duration-values.pkl",
            Some(include_str!(
                "../../../tests/fixtures/m2-duration-values/module.pkl"
            )),
            expression,
            0,
            None,
        )
        .unwrap();
    for _ in 0..200_000 {
        probe.collect();
        let event = probe.poll(0, 1).unwrap();
        probe.collect();
        if event.kind == "yield" {
            continue;
        }
        if event.kind == "module-request"
            && module_keys::complete_probe(probe, event.token.as_deref().unwrap(), 0).unwrap()
        {
            continue;
        }
        return event;
    }
    panic!("Duration public-query poll bound")
}

#[test]
fn duration_values_operations_and_complete_diagnostics_match_java() {
    let mut probe = SourceProbe::new_module_evaluator("compact", false, false, false).unwrap();
    let mut mismatches = Vec::new();
    for (id, expression, kind, expected) in [
        (
            r####"int-ns"####,
            r####"7.ns"####,
            r####"value"####,
            r####"7.ns"####,
        ),
        (
            r####"float-ns"####,
            r####"7.25.ns"####,
            r####"value"####,
            r####"7.25.ns"####,
        ),
        (
            r####"negative-ns"####,
            r####"(-3).ns"####,
            r####"value"####,
            r####"-3.ns"####,
        ),
        (
            r####"value-ns"####,
            r####"7.25.ns.value"####,
            r####"value"####,
            r####"7.25"####,
        ),
        (
            r####"unit-ns"####,
            r####"7.25.ns.unit"####,
            r####"value"####,
            r####"ns"####,
        ),
        (
            r####"iso-ns"####,
            r####"7.25.ns.isoString"####,
            r####"value"####,
            r####"PT0.000000007S"####,
        ),
        (
            r####"positive-ns"####,
            r####"7.25.ns.isPositive"####,
            r####"value"####,
            r####"true"####,
        ),
        (
            r####"int-to-duration-ns"####,
            r####"7.toDuration("ns")"####,
            r####"value"####,
            r####"7.ns"####,
        ),
        (
            r####"float-to-duration-ns"####,
            r####"7.25.toDuration("ns")"####,
            r####"value"####,
            r####"7.25.ns"####,
        ),
        (
            r####"int-us"####,
            r####"7.us"####,
            r####"value"####,
            r####"7.us"####,
        ),
        (
            r####"float-us"####,
            r####"7.25.us"####,
            r####"value"####,
            r####"7.25.us"####,
        ),
        (
            r####"negative-us"####,
            r####"(-3).us"####,
            r####"value"####,
            r####"-3.us"####,
        ),
        (
            r####"value-us"####,
            r####"7.25.us.value"####,
            r####"value"####,
            r####"7.25"####,
        ),
        (
            r####"unit-us"####,
            r####"7.25.us.unit"####,
            r####"value"####,
            r####"us"####,
        ),
        (
            r####"iso-us"####,
            r####"7.25.us.isoString"####,
            r####"value"####,
            r####"PT0.00000725S"####,
        ),
        (
            r####"positive-us"####,
            r####"7.25.us.isPositive"####,
            r####"value"####,
            r####"true"####,
        ),
        (
            r####"int-to-duration-us"####,
            r####"7.toDuration("us")"####,
            r####"value"####,
            r####"7.us"####,
        ),
        (
            r####"float-to-duration-us"####,
            r####"7.25.toDuration("us")"####,
            r####"value"####,
            r####"7.25.us"####,
        ),
        (
            r####"int-ms"####,
            r####"7.ms"####,
            r####"value"####,
            r####"7.ms"####,
        ),
        (
            r####"float-ms"####,
            r####"7.25.ms"####,
            r####"value"####,
            r####"7.25.ms"####,
        ),
        (
            r####"negative-ms"####,
            r####"(-3).ms"####,
            r####"value"####,
            r####"-3.ms"####,
        ),
        (
            r####"value-ms"####,
            r####"7.25.ms.value"####,
            r####"value"####,
            r####"7.25"####,
        ),
        (
            r####"unit-ms"####,
            r####"7.25.ms.unit"####,
            r####"value"####,
            r####"ms"####,
        ),
        (
            r####"iso-ms"####,
            r####"7.25.ms.isoString"####,
            r####"value"####,
            r####"PT0.00725S"####,
        ),
        (
            r####"positive-ms"####,
            r####"7.25.ms.isPositive"####,
            r####"value"####,
            r####"true"####,
        ),
        (
            r####"int-to-duration-ms"####,
            r####"7.toDuration("ms")"####,
            r####"value"####,
            r####"7.ms"####,
        ),
        (
            r####"float-to-duration-ms"####,
            r####"7.25.toDuration("ms")"####,
            r####"value"####,
            r####"7.25.ms"####,
        ),
        (
            r####"int-s"####,
            r####"7.s"####,
            r####"value"####,
            r####"7.s"####,
        ),
        (
            r####"float-s"####,
            r####"7.25.s"####,
            r####"value"####,
            r####"7.25.s"####,
        ),
        (
            r####"negative-s"####,
            r####"(-3).s"####,
            r####"value"####,
            r####"-3.s"####,
        ),
        (
            r####"value-s"####,
            r####"7.25.s.value"####,
            r####"value"####,
            r####"7.25"####,
        ),
        (
            r####"unit-s"####,
            r####"7.25.s.unit"####,
            r####"value"####,
            r####"s"####,
        ),
        (
            r####"iso-s"####,
            r####"7.25.s.isoString"####,
            r####"value"####,
            r####"PT7.25S"####,
        ),
        (
            r####"positive-s"####,
            r####"7.25.s.isPositive"####,
            r####"value"####,
            r####"true"####,
        ),
        (
            r####"int-to-duration-s"####,
            r####"7.toDuration("s")"####,
            r####"value"####,
            r####"7.s"####,
        ),
        (
            r####"float-to-duration-s"####,
            r####"7.25.toDuration("s")"####,
            r####"value"####,
            r####"7.25.s"####,
        ),
        (
            r####"int-min"####,
            r####"7.min"####,
            r####"value"####,
            r####"7.min"####,
        ),
        (
            r####"float-min"####,
            r####"7.25.min"####,
            r####"value"####,
            r####"7.25.min"####,
        ),
        (
            r####"negative-min"####,
            r####"(-3).min"####,
            r####"value"####,
            r####"-3.min"####,
        ),
        (
            r####"value-min"####,
            r####"7.25.min.value"####,
            r####"value"####,
            r####"7.25"####,
        ),
        (
            r####"unit-min"####,
            r####"7.25.min.unit"####,
            r####"value"####,
            r####"min"####,
        ),
        (
            r####"iso-min"####,
            r####"7.25.min.isoString"####,
            r####"value"####,
            r####"PT7M15S"####,
        ),
        (
            r####"positive-min"####,
            r####"7.25.min.isPositive"####,
            r####"value"####,
            r####"true"####,
        ),
        (
            r####"int-to-duration-min"####,
            r####"7.toDuration("min")"####,
            r####"value"####,
            r####"7.min"####,
        ),
        (
            r####"float-to-duration-min"####,
            r####"7.25.toDuration("min")"####,
            r####"value"####,
            r####"7.25.min"####,
        ),
        (
            r####"int-h"####,
            r####"7.h"####,
            r####"value"####,
            r####"7.h"####,
        ),
        (
            r####"float-h"####,
            r####"7.25.h"####,
            r####"value"####,
            r####"7.25.h"####,
        ),
        (
            r####"negative-h"####,
            r####"(-3).h"####,
            r####"value"####,
            r####"-3.h"####,
        ),
        (
            r####"value-h"####,
            r####"7.25.h.value"####,
            r####"value"####,
            r####"7.25"####,
        ),
        (
            r####"unit-h"####,
            r####"7.25.h.unit"####,
            r####"value"####,
            r####"h"####,
        ),
        (
            r####"iso-h"####,
            r####"7.25.h.isoString"####,
            r####"value"####,
            r####"PT7H15M"####,
        ),
        (
            r####"positive-h"####,
            r####"7.25.h.isPositive"####,
            r####"value"####,
            r####"true"####,
        ),
        (
            r####"int-to-duration-h"####,
            r####"7.toDuration("h")"####,
            r####"value"####,
            r####"7.h"####,
        ),
        (
            r####"float-to-duration-h"####,
            r####"7.25.toDuration("h")"####,
            r####"value"####,
            r####"7.25.h"####,
        ),
        (
            r####"int-d"####,
            r####"7.d"####,
            r####"value"####,
            r####"7.d"####,
        ),
        (
            r####"float-d"####,
            r####"7.25.d"####,
            r####"value"####,
            r####"7.25.d"####,
        ),
        (
            r####"negative-d"####,
            r####"(-3).d"####,
            r####"value"####,
            r####"-3.d"####,
        ),
        (
            r####"value-d"####,
            r####"7.25.d.value"####,
            r####"value"####,
            r####"7.25"####,
        ),
        (
            r####"unit-d"####,
            r####"7.25.d.unit"####,
            r####"value"####,
            r####"d"####,
        ),
        (
            r####"iso-d"####,
            r####"7.25.d.isoString"####,
            r####"value"####,
            r####"PT174H"####,
        ),
        (
            r####"positive-d"####,
            r####"7.25.d.isPositive"####,
            r####"value"####,
            r####"true"####,
        ),
        (
            r####"int-to-duration-d"####,
            r####"7.toDuration("d")"####,
            r####"value"####,
            r####"7.d"####,
        ),
        (
            r####"float-to-duration-d"####,
            r####"7.25.toDuration("d")"####,
            r####"value"####,
            r####"7.25.d"####,
        ),
        (
            r####"ns-to-ns"####,
            r####"1.25.ns.toUnit("ns")"####,
            r####"value"####,
            r####"1.25.ns"####,
        ),
        (
            r####"ns-to-us"####,
            r####"1.25.ns.toUnit("us")"####,
            r####"value"####,
            r####"0.00125.us"####,
        ),
        (
            r####"ns-to-ms"####,
            r####"1.25.ns.toUnit("ms")"####,
            r####"value"####,
            r####"1.25E-6.ms"####,
        ),
        (
            r####"ns-to-s"####,
            r####"1.25.ns.toUnit("s")"####,
            r####"value"####,
            r####"1.25E-9.s"####,
        ),
        (
            r####"ns-to-min"####,
            r####"1.25.ns.toUnit("min")"####,
            r####"value"####,
            r####"2.0833333333333332E-11.min"####,
        ),
        (
            r####"ns-to-h"####,
            r####"1.25.ns.toUnit("h")"####,
            r####"value"####,
            r####"3.472222222222222E-13.h"####,
        ),
        (
            r####"ns-to-d"####,
            r####"1.25.ns.toUnit("d")"####,
            r####"value"####,
            r####"1.446759259259259E-14.d"####,
        ),
        (
            r####"us-to-ns"####,
            r####"1.25.us.toUnit("ns")"####,
            r####"value"####,
            r####"1250.ns"####,
        ),
        (
            r####"us-to-us"####,
            r####"1.25.us.toUnit("us")"####,
            r####"value"####,
            r####"1.25.us"####,
        ),
        (
            r####"us-to-ms"####,
            r####"1.25.us.toUnit("ms")"####,
            r####"value"####,
            r####"0.00125.ms"####,
        ),
        (
            r####"us-to-s"####,
            r####"1.25.us.toUnit("s")"####,
            r####"value"####,
            r####"1.25E-6.s"####,
        ),
        (
            r####"us-to-min"####,
            r####"1.25.us.toUnit("min")"####,
            r####"value"####,
            r####"2.0833333333333335E-8.min"####,
        ),
        (
            r####"us-to-h"####,
            r####"1.25.us.toUnit("h")"####,
            r####"value"####,
            r####"3.4722222222222225E-10.h"####,
        ),
        (
            r####"us-to-d"####,
            r####"1.25.us.toUnit("d")"####,
            r####"value"####,
            r####"1.4467592592592592E-11.d"####,
        ),
        (
            r####"ms-to-ns"####,
            r####"1.25.ms.toUnit("ns")"####,
            r####"value"####,
            r####"1250000.ns"####,
        ),
        (
            r####"ms-to-us"####,
            r####"1.25.ms.toUnit("us")"####,
            r####"value"####,
            r####"1250.us"####,
        ),
        (
            r####"ms-to-ms"####,
            r####"1.25.ms.toUnit("ms")"####,
            r####"value"####,
            r####"1.25.ms"####,
        ),
        (
            r####"ms-to-s"####,
            r####"1.25.ms.toUnit("s")"####,
            r####"value"####,
            r####"0.00125.s"####,
        ),
        (
            r####"ms-to-min"####,
            r####"1.25.ms.toUnit("min")"####,
            r####"value"####,
            r####"2.0833333333333333E-5.min"####,
        ),
        (
            r####"ms-to-h"####,
            r####"1.25.ms.toUnit("h")"####,
            r####"value"####,
            r####"3.4722222222222224E-7.h"####,
        ),
        (
            r####"ms-to-d"####,
            r####"1.25.ms.toUnit("d")"####,
            r####"value"####,
            r####"1.4467592592592593E-8.d"####,
        ),
        (
            r####"s-to-ns"####,
            r####"1.25.s.toUnit("ns")"####,
            r####"value"####,
            r####"1250000000.ns"####,
        ),
        (
            r####"s-to-us"####,
            r####"1.25.s.toUnit("us")"####,
            r####"value"####,
            r####"1250000.us"####,
        ),
        (
            r####"s-to-ms"####,
            r####"1.25.s.toUnit("ms")"####,
            r####"value"####,
            r####"1250.ms"####,
        ),
        (
            r####"s-to-s"####,
            r####"1.25.s.toUnit("s")"####,
            r####"value"####,
            r####"1.25.s"####,
        ),
        (
            r####"s-to-min"####,
            r####"1.25.s.toUnit("min")"####,
            r####"value"####,
            r####"0.020833333333333332.min"####,
        ),
        (
            r####"s-to-h"####,
            r####"1.25.s.toUnit("h")"####,
            r####"value"####,
            r####"3.4722222222222224E-4.h"####,
        ),
        (
            r####"s-to-d"####,
            r####"1.25.s.toUnit("d")"####,
            r####"value"####,
            r####"1.4467592592592593E-5.d"####,
        ),
        (
            r####"min-to-ns"####,
            r####"1.25.min.toUnit("ns")"####,
            r####"value"####,
            r####"75000000000.ns"####,
        ),
        (
            r####"min-to-us"####,
            r####"1.25.min.toUnit("us")"####,
            r####"value"####,
            r####"75000000.us"####,
        ),
        (
            r####"min-to-ms"####,
            r####"1.25.min.toUnit("ms")"####,
            r####"value"####,
            r####"75000.ms"####,
        ),
        (
            r####"min-to-s"####,
            r####"1.25.min.toUnit("s")"####,
            r####"value"####,
            r####"75.s"####,
        ),
        (
            r####"min-to-min"####,
            r####"1.25.min.toUnit("min")"####,
            r####"value"####,
            r####"1.25.min"####,
        ),
        (
            r####"min-to-h"####,
            r####"1.25.min.toUnit("h")"####,
            r####"value"####,
            r####"0.020833333333333332.h"####,
        ),
        (
            r####"min-to-d"####,
            r####"1.25.min.toUnit("d")"####,
            r####"value"####,
            r####"8.680555555555555E-4.d"####,
        ),
        (
            r####"h-to-ns"####,
            r####"1.25.h.toUnit("ns")"####,
            r####"value"####,
            r####"4500000000000.ns"####,
        ),
        (
            r####"h-to-us"####,
            r####"1.25.h.toUnit("us")"####,
            r####"value"####,
            r####"4500000000.us"####,
        ),
        (
            r####"h-to-ms"####,
            r####"1.25.h.toUnit("ms")"####,
            r####"value"####,
            r####"4500000.ms"####,
        ),
        (
            r####"h-to-s"####,
            r####"1.25.h.toUnit("s")"####,
            r####"value"####,
            r####"4500.s"####,
        ),
        (
            r####"h-to-min"####,
            r####"1.25.h.toUnit("min")"####,
            r####"value"####,
            r####"75.min"####,
        ),
        (
            r####"h-to-h"####,
            r####"1.25.h.toUnit("h")"####,
            r####"value"####,
            r####"1.25.h"####,
        ),
        (
            r####"h-to-d"####,
            r####"1.25.h.toUnit("d")"####,
            r####"value"####,
            r####"0.052083333333333336.d"####,
        ),
        (
            r####"d-to-ns"####,
            r####"1.25.d.toUnit("ns")"####,
            r####"value"####,
            r####"108000000000000.ns"####,
        ),
        (
            r####"d-to-us"####,
            r####"1.25.d.toUnit("us")"####,
            r####"value"####,
            r####"108000000000.us"####,
        ),
        (
            r####"d-to-ms"####,
            r####"1.25.d.toUnit("ms")"####,
            r####"value"####,
            r####"108000000.ms"####,
        ),
        (
            r####"d-to-s"####,
            r####"1.25.d.toUnit("s")"####,
            r####"value"####,
            r####"108000.s"####,
        ),
        (
            r####"d-to-min"####,
            r####"1.25.d.toUnit("min")"####,
            r####"value"####,
            r####"1800.min"####,
        ),
        (
            r####"d-to-h"####,
            r####"1.25.d.toUnit("h")"####,
            r####"value"####,
            r####"30.h"####,
        ),
        (
            r####"d-to-d"####,
            r####"1.25.d.toUnit("d")"####,
            r####"value"####,
            r####"1.25.d"####,
        ),
        (
            r####"positive-zero"####,
            r####"0.0.s"####,
            r####"value"####,
            r####"0.s"####,
        ),
        (
            r####"negative-zero"####,
            r####"(-0.0).s"####,
            r####"value"####,
            r####"0.s"####,
        ),
        (
            r####"min-subnormal"####,
            r####"(5e-324).s"####,
            r####"value"####,
            r####"4.9E-324.s"####,
        ),
        (
            r####"negative-subnormal"####,
            r####"(-5e-324).s"####,
            r####"value"####,
            r####"-4.9E-324.s"####,
        ),
        (
            r####"max-finite"####,
            r####"(1.7976931348623157e308).s"####,
            r####"value"####,
            r####"9223372036854775807.s"####,
        ),
        (
            r####"huge-finite-day"####,
            r####"(1.7976931348623157e308).d"####,
            r####"value"####,
            r####"9223372036854775807.d"####,
        ),
        (
            r####"positive-infinity"####,
            r####"(1.0/0.0).s"####,
            r####"value"####,
            r####"Infinity.s"####,
        ),
        (
            r####"negative-infinity"####,
            r####"(-1.0/0.0).s"####,
            r####"value"####,
            r####"-Infinity.s"####,
        ),
        (
            r####"nan"####,
            r####"(0.0/0.0).s"####,
            r####"value"####,
            r####"NaN.s"####,
        ),
        (
            r####"positive-zero-value"####,
            r####"0.0.s.value"####,
            r####"value"####,
            r####"0"####,
        ),
        (
            r####"negative-zero-value"####,
            r####"(-0.0).s.value"####,
            r####"value"####,
            r####"0"####,
        ),
        (
            r####"nan-value"####,
            r####"(0.0/0.0).s.value"####,
            r####"value"####,
            r####"NaN"####,
        ),
        (
            r####"inf-value"####,
            r####"(1.0/0.0).s.value"####,
            r####"value"####,
            r####"Infinity"####,
        ),
        (
            r####"nan-positive"####,
            r####"(0.0/0.0).s.isPositive"####,
            r####"value"####,
            r####"false"####,
        ),
        (
            r####"negative-zero-positive"####,
            r####"(-0.0).s.isPositive"####,
            r####"value"####,
            r####"true"####,
        ),
        (
            r####"negative-positive"####,
            r####"(-1).s.isPositive"####,
            r####"value"####,
            r####"false"####,
        ),
        (
            r####"subnormal-to-days"####,
            r####"(5e-324).ns.toUnit("d")"####,
            r####"value"####,
            r####"0.d"####,
        ),
        (
            r####"max-day-to-ns"####,
            r####"(1.7976931348623157e308).d.toUnit("ns")"####,
            r####"value"####,
            r####"Infinity.ns"####,
        ),
        (
            r####"max-ns-to-day"####,
            r####"(1.7976931348623157e308).ns.toUnit("d")"####,
            r####"value"####,
            r####"9223372036854775807.d"####,
        ),
        (
            r####"negative-zero-to-ns"####,
            r####"(-0.0).s.toUnit("ns")"####,
            r####"value"####,
            r####"0.ns"####,
        ),
        (
            r####"nan-to-min"####,
            r####"(0.0/0.0).s.toUnit("min")"####,
            r####"value"####,
            r####"NaN.min"####,
        ),
        (
            r####"inf-to-day"####,
            r####"(1.0/0.0).s.toUnit("d")"####,
            r####"value"####,
            r####"Infinity.d"####,
        ),
        (
            r####"iso-zero"####,
            r####"0.s.isoString"####,
            r####"value"####,
            r####"PT0S"####,
        ),
        (
            r####"iso-negative-zero"####,
            r####"(-0.0).s.isoString"####,
            r####"value"####,
            r####"PT0S"####,
        ),
        (
            r####"iso-sub-ns"####,
            r####"(5e-324).ns.isoString"####,
            r####"value"####,
            r####"PT0S"####,
        ),
        (
            r####"iso-negative-sub-ns"####,
            r####"(-5e-324).ns.isoString"####,
            r####"value"####,
            r####"PT0S"####,
        ),
        (
            r####"iso-fractional-ns"####,
            r####"1.5.ns.isoString"####,
            r####"value"####,
            r####"PT0.000000001S"####,
        ),
        (
            r####"iso-fractional-us"####,
            r####"1.234567.us.isoString"####,
            r####"value"####,
            r####"PT0.000001234S"####,
        ),
        (
            r####"iso-fractional-ms"####,
            r####"1.234567.ms.isoString"####,
            r####"value"####,
            r####"PT0.001234567S"####,
        ),
        (
            r####"iso-59-999"####,
            r####"59.999999999.s.isoString"####,
            r####"value"####,
            r####"PT59.999999999S"####,
        ),
        (
            r####"iso-60"####,
            r####"60.s.isoString"####,
            r####"value"####,
            r####"PT1M"####,
        ),
        (
            r####"iso-3600"####,
            r####"3600.s.isoString"####,
            r####"value"####,
            r####"PT1H"####,
        ),
        (
            r####"iso-negative-fraction"####,
            r####"(-1.5).h.isoString"####,
            r####"value"####,
            r####"-PT1H30M"####,
        ),
        (
            r####"iso-precision-boundary"####,
            r####"1000000000.1234567.s.isoString"####,
            r####"value"####,
            r####"PT277777H46M40.123456768S"####,
        ),
        (
            r####"iso-huge-seconds"####,
            r####"(1e20).s.isoString"####,
            r####"value"####,
            r####"PT27777777777777776H12M40S"####,
        ),
        (
            r####"iso-huge-days"####,
            r####"(1e20).d.isoString"####,
            r####"value"####,
            r####"PT9223372036854775807H7M36S"####,
        ),
        (
            r####"iso-max-finite-ns"####,
            r####"(1.7976931348623157e308).ns.isoString"####,
            r####"value"####,
            r####"PT9223372036854775807H7M4S"####,
        ),
        (
            r####"iso-max-finite-days"####,
            r####"(1.7976931348623157e308).d.isoString"####,
            r####"error"####,
            r####"–– Pkl Error ––
Cannot convert duration `9223372036854775807.d` to ISO 8601 duration.

1 | (1.7976931348623157e308).d.isoString
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"####,
        ),
        (
            r####"iso-overflow-days"####,
            r####"(1e308).d.isoString"####,
            r####"error"####,
            r####"–– Pkl Error ––
Cannot convert duration `9223372036854775807.d` to ISO 8601 duration.

1 | (1e308).d.isoString
    ^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"####,
        ),
        (
            r####"iso-nan"####,
            r####"(0.0/0.0).s.isoString"####,
            r####"error"####,
            r####"–– Pkl Error ––
Cannot convert duration `NaN.s` to ISO 8601 duration.

1 | (0.0/0.0).s.isoString
    ^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"####,
        ),
        (
            r####"iso-pos-inf"####,
            r####"(1.0/0.0).s.isoString"####,
            r####"error"####,
            r####"–– Pkl Error ––
Cannot convert duration `Infinity.s` to ISO 8601 duration.

1 | (1.0/0.0).s.isoString
    ^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"####,
        ),
        (
            r####"iso-neg-inf"####,
            r####"(-1.0/0.0).s.isoString"####,
            r####"error"####,
            r####"–– Pkl Error ––
Cannot convert duration `-Infinity.s` to ISO 8601 duration.

1 | (-1.0/0.0).s.isoString
    ^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"####,
        ),
        (
            r####"iso-huge-finite-s"####,
            r####"(1e100).s.isoString"####,
            r####"value"####,
            r####"PT9223372036854775807H7M4S"####,
        ),
        (
            r####"iso-huge-finite-ns"####,
            r####"(1e300).ns.isoString"####,
            r####"value"####,
            r####"PT9223372036854775807H7M56S"####,
        ),
        (
            r####"iso-one-nanosecond-s"####,
            r####"(1e-9).s.isoString"####,
            r####"value"####,
            r####"PT0.000000001S"####,
        ),
        (
            r####"iso-one-nanosecond-ns"####,
            r####"1.ns.isoString"####,
            r####"value"####,
            r####"PT0.000000001S"####,
        ),
        (
            r####"iso-just-below-second"####,
            r####"0.999999999.s.isoString"####,
            r####"value"####,
            r####"PT0.999999999S"####,
        ),
        (
            r####"iso-just-above-second"####,
            r####"1.000000001.s.isoString"####,
            r####"value"####,
            r####"PT1.000000001S"####,
        ),
        (
            r####"add-s-min"####,
            r####"1.s + 1.min"####,
            r####"value"####,
            r####"1.0166666666666666.min"####,
        ),
        (
            r####"add-min-s"####,
            r####"1.min + 1.s"####,
            r####"value"####,
            r####"1.0166666666666666.min"####,
        ),
        (
            r####"subtract-s-min"####,
            r####"1.s - 1.min"####,
            r####"value"####,
            r####"-0.9833333333333333.min"####,
        ),
        (
            r####"subtract-min-s"####,
            r####"1.min - 1.s"####,
            r####"value"####,
            r####"0.9833333333333333.min"####,
        ),
        (
            r####"add-ns-d"####,
            r####"1.ns + 1.d"####,
            r####"value"####,
            r####"1.0000000000000115.d"####,
        ),
        (
            r####"add-d-ns"####,
            r####"1.d + 1.ns"####,
            r####"value"####,
            r####"1.0000000000000115.d"####,
        ),
        (
            r####"cancel-coarse"####,
            r####"1.d + (-86400).s"####,
            r####"value"####,
            r####"0.d"####,
        ),
        (
            r####"cancel-fine"####,
            r####"(-86400).s + 1.d"####,
            r####"value"####,
            r####"0.d"####,
        ),
        (
            r####"add-near-max"####,
            r####"(1e308).d + (1e308).d"####,
            r####"value"####,
            r####"Infinity.d"####,
        ),
        (
            r####"add-inf-neg-inf"####,
            r####"(1.0/0.0).s + (-1.0/0.0).s"####,
            r####"value"####,
            r####"NaN.s"####,
        ),
        (
            r####"multiply-int"####,
            r####"1.5.s * 2"####,
            r####"value"####,
            r####"3.s"####,
        ),
        (
            r####"multiply-float"####,
            r####"1.5.s * 2.5"####,
            r####"value"####,
            r####"3.75.s"####,
        ),
        (
            r####"multiply-reverse"####,
            r####"2 * 1.5.s"####,
            r####"value"####,
            r####"3.s"####,
        ),
        (
            r####"multiply-negative-zero"####,
            r####"(-0.0).s * (-1)"####,
            r####"value"####,
            r####"0.s"####,
        ),
        (
            r####"divide-int"####,
            r####"3.s / 2"####,
            r####"value"####,
            r####"1.5.s"####,
        ),
        (
            r####"divide-float"####,
            r####"3.s / 2.0"####,
            r####"value"####,
            r####"1.5.s"####,
        ),
        (
            r####"ratio-s-min"####,
            r####"1.s / 1.min"####,
            r####"value"####,
            r####"0.016666666666666666"####,
        ),
        (
            r####"ratio-min-s"####,
            r####"1.min / 1.s"####,
            r####"value"####,
            r####"60.0"####,
        ),
        (
            r####"ratio-zero"####,
            r####"1.s / 0.s"####,
            r####"value"####,
            r####"Infinity"####,
        ),
        (
            r####"divide-zero"####,
            r####"1.s / 0"####,
            r####"value"####,
            r####"Infinity.s"####,
        ),
        (
            r####"trunc-int"####,
            r####"3.s ~/ 2"####,
            r####"value"####,
            r####"1.s"####,
        ),
        (
            r####"trunc-float"####,
            r####"3.s ~/ 2.0"####,
            r####"value"####,
            r####"1.s"####,
        ),
        (
            r####"trunc-ratio"####,
            r####"1.min ~/ 1.s"####,
            r####"value"####,
            r####"60"####,
        ),
        (
            r####"trunc-ratio-reverse"####,
            r####"1.s ~/ 1.min"####,
            r####"value"####,
            r####"0"####,
        ),
        (
            r####"trunc-zero"####,
            r####"1.s ~/ 0"####,
            r####"error"####,
            r####"–– Pkl Error ––
Cannot convert Float `1.0` to Int because it is too large.

1 | 1.s ~/ 0
    ^^^^^^^^
at  (repl:text)
"####,
        ),
        (
            r####"trunc-ratio-zero"####,
            r####"1.s ~/ 0.s"####,
            r####"error"####,
            r####"–– Pkl Error ––
Cannot convert Float `1.0` to Int because it is too large.

1 | 1.s ~/ 0.s
    ^^^^^^^^^^
at  (repl:text)
"####,
        ),
        (
            r####"remainder-int"####,
            r####"5.s % 2"####,
            r####"value"####,
            r####"1.s"####,
        ),
        (
            r####"remainder-float"####,
            r####"5.s % 2.5"####,
            r####"value"####,
            r####"0.s"####,
        ),
        (
            r####"remainder-zero"####,
            r####"5.s % 0"####,
            r####"value"####,
            r####"NaN.s"####,
        ),
        (
            r####"remainder-negative"####,
            r####"(-5).s % 2"####,
            r####"value"####,
            r####"-1.s"####,
        ),
        (
            r####"pow-two"####,
            r####"2.s ** 3"####,
            r####"value"####,
            r####"8.s"####,
        ),
        (
            r####"pow-half"####,
            r####"4.s ** 0.5"####,
            r####"value"####,
            r####"2.s"####,
        ),
        (
            r####"pow-neg"####,
            r####"2.s ** -1"####,
            r####"value"####,
            r####"0.5.s"####,
        ),
        (
            r####"pow-nan-zero"####,
            r####"(0.0/0.0).s ** 0"####,
            r####"value"####,
            r####"1.s"####,
        ),
        (
            r####"pow-negative-zero"####,
            r####"(-0.0).s ** 3"####,
            r####"value"####,
            r####"0.s"####,
        ),
        (
            r####"unary-plus"####,
            r####"+1.s"####,
            r####"error"####,
            r####"–– Pkl Error ––
Unexpected token `+`.

1 | +1.s
    ^
at repl:text (repl:text)
"####,
        ),
        (
            r####"unary-minus"####,
            r####"-1.s"####,
            r####"value"####,
            r####"-1.s"####,
        ),
        (
            r####"unary-minus-zero"####,
            r####"-0.0.s"####,
            r####"value"####,
            r####"0.s"####,
        ),
        (
            r####"equal-mixed"####,
            r####"1.min == 60.s"####,
            r####"value"####,
            r####"true"####,
        ),
        (
            r####"unequal-mixed"####,
            r####"1.min == 59.s"####,
            r####"value"####,
            r####"false"####,
        ),
        (
            r####"equal-signed-zero"####,
            r####"(-0.0).s == 0.0.s"####,
            r####"value"####,
            r####"true"####,
        ),
        (
            r####"nan-self"####,
            r####"nanDuration == nanDuration"####,
            r####"value"####,
            r####"true"####,
        ),
        (
            r####"nan-fresh"####,
            r####"(0.0/0.0).s == (0.0/0.0).s"####,
            r####"value"####,
            r####"false"####,
        ),
        (
            r####"less-mixed"####,
            r####"59.s < 1.min"####,
            r####"value"####,
            r####"true"####,
        ),
        (
            r####"greater-mixed"####,
            r####"1.min > 59.s"####,
            r####"value"####,
            r####"true"####,
        ),
        (
            r####"less-equal"####,
            r####"60.s <= 1.min"####,
            r####"value"####,
            r####"true"####,
        ),
        (
            r####"greater-equal"####,
            r####"1.min >= 60.s"####,
            r####"value"####,
            r####"true"####,
        ),
        (
            r####"less-zero"####,
            r####"(-0.0).s < 0.0.s"####,
            r####"value"####,
            r####"true"####,
        ),
        (
            r####"greater-zero"####,
            r####"(-0.0).s > 0.0.s"####,
            r####"value"####,
            r####"false"####,
        ),
        (
            r####"less-nan"####,
            r####"(0.0/0.0).s < 1.s"####,
            r####"value"####,
            r####"false"####,
        ),
        (
            r####"greater-nan"####,
            r####"(0.0/0.0).s > 1.s"####,
            r####"value"####,
            r####"true"####,
        ),
        (
            r####"between-mixed"####,
            r####"1.min.isBetween(59.s,61.s)"####,
            r####"value"####,
            r####"true"####,
        ),
        (
            r####"between-equal"####,
            r####"1.min.isBetween(60.s,60.s)"####,
            r####"value"####,
            r####"true"####,
        ),
        (
            r####"between-reverse"####,
            r####"1.min.isBetween(2.min,0.min)"####,
            r####"value"####,
            r####"false"####,
        ),
        (
            r####"between-zero"####,
            r####"(-0.0).s.isBetween(0.0.s,0.0.s)"####,
            r####"value"####,
            r####"false"####,
        ),
        (
            r####"between-nan"####,
            r####"(0.0/0.0).s.isBetween(0.s,1.s)"####,
            r####"value"####,
            r####"false"####,
        ),
        (
            r####"between-throw-first"####,
            r####"1.s.isBetween(throw("first"),throw("second"))"####,
            r####"error"####,
            r####"–– Pkl Error ––
first

1 | 1.s.isBetween(throw("first"),throw("second"))
                  ^^^^^^^^^^^^^^
at  (repl:text)
"####,
        ),
        (
            r####"between-throw-second"####,
            r####"1.s.isBetween(0.s,throw("second"))"####,
            r####"error"####,
            r####"–– Pkl Error ––
second

1 | 1.s.isBetween(0.s,throw("second"))
                      ^^^^^^^^^^^^^^^
at  (repl:text)
"####,
        ),
        (
            r####"set-equal"####,
            r####"Set(1.min,60.s).length"####,
            r####"value"####,
            r####"1"####,
        ),
        (
            r####"set-zero"####,
            r####"Set((-0.0).s,0.0.s).length"####,
            r####"value"####,
            r####"1"####,
        ),
        (
            r####"set-nan-self"####,
            r####"Set(nanDuration,nanDuration).length"####,
            r####"value"####,
            r####"1"####,
        ),
        (
            r####"set-nan-fresh"####,
            r####"Set((0.0/0.0).s,(0.0/0.0).s).length"####,
            r####"value"####,
            r####"2"####,
        ),
        (
            r####"map-equal"####,
            r####"Map(1.min,"a",60.s,"b").length"####,
            r####"value"####,
            r####"1"####,
        ),
        (
            r####"map-zero"####,
            r####"Map((-0.0).s,"a",0.0.s,"b").length"####,
            r####"value"####,
            r####"1"####,
        ),
        (
            r####"map-nan"####,
            r####"Map(nanDuration,"a",nanDuration,"b").length"####,
            r####"value"####,
            r####"1"####,
        ),
        (
            r####"set-threshold-four"####,
            r####"Set(1.min,2.s,3.s,60.s).length"####,
            r####"value"####,
            r####"3"####,
        ),
        (
            r####"set-threshold-five"####,
            r####"Set(1.min,2.s,3.s,4.s,60.s).length"####,
            r####"value"####,
            r####"4"####,
        ),
        (
            r####"map-threshold-four"####,
            r####"Map(1.min,1,2.s,2,3.s,3,60.s,4).length"####,
            r####"value"####,
            r####"3"####,
        ),
        (
            r####"map-threshold-five"####,
            r####"Map(1.min,1,2.s,2,3.s,3,4.s,4,60.s,5).length"####,
            r####"value"####,
            r####"4"####,
        ),
        (
            r####"same-unit-fresh"####,
            r####"Set(1.s,1.s.toUnit("s")).length"####,
            r####"value"####,
            r####"1"####,
        ),
        (
            r####"same-unit-equal"####,
            r####"1.s == 1.s.toUnit("s")"####,
            r####"value"####,
            r####"true"####,
        ),
        (
            r####"nan-converted-self"####,
            r####"nanDuration == nanDuration.toUnit("s")"####,
            r####"value"####,
            r####"false"####,
        ),
        (
            r####"nan-between-nan"####,
            r####"nanDuration.isBetween(nanDuration,nanDuration)"####,
            r####"value"####,
            r####"true"####,
        ),
        (
            r####"nan-less-equal-self"####,
            r####"nanDuration <= nanDuration"####,
            r####"value"####,
            r####"true"####,
        ),
        (
            r####"nan-greater-equal-self"####,
            r####"nanDuration >= nanDuration"####,
            r####"value"####,
            r####"true"####,
        ),
        (
            r####"between-nan-lower"####,
            r####"1.s.isBetween((0.0/0.0).s,2.s)"####,
            r####"value"####,
            r####"false"####,
        ),
        (
            r####"between-nan-upper"####,
            r####"1.s.isBetween(0.s,(0.0/0.0).s)"####,
            r####"value"####,
            r####"true"####,
        ),
        (
            r####"between-arg-first-throw"####,
            r####"1.s.isBetween(throw("lower"),throw("upper"))"####,
            r####"error"####,
            r####"–– Pkl Error ––
lower

1 | 1.s.isBetween(throw("lower"),throw("upper"))
                  ^^^^^^^^^^^^^^
at  (repl:text)
"####,
        ),
        (
            r####"between-arg-upper-throw"####,
            r####"1.s.isBetween(0.s,throw("upper"))"####,
            r####"error"####,
            r####"–– Pkl Error ––
upper

1 | 1.s.isBetween(0.s,throw("upper"))
                      ^^^^^^^^^^^^^^
at  (repl:text)
"####,
        ),
        (
            r####"set-zero-threshold-four"####,
            r####"Set((-0.0).s,1.s,2.s,0.0.s).length"####,
            r####"value"####,
            r####"3"####,
        ),
        (
            r####"set-zero-threshold-five"####,
            r####"Set((-0.0).s,1.s,2.s,3.s,0.0.s).length"####,
            r####"value"####,
            r####"4"####,
        ),
        (
            r####"map-zero-threshold-four"####,
            r####"Map((-0.0).s,1,1.s,2,2.s,3,0.0.s,4).length"####,
            r####"value"####,
            r####"3"####,
        ),
        (
            r####"map-zero-threshold-five"####,
            r####"Map((-0.0).s,1,1.s,2,2.s,3,3.s,4,0.0.s,5).length"####,
            r####"value"####,
            r####"4"####,
        ),
        (
            r####"duration-unit-alias"####,
            r####"DurationUnit"####,
            r####"value"####,
            r####"DurationUnit"####,
        ),
        (
            r####"duration-unit-alias-type"####,
            r####"DurationUnit is TypeAlias"####,
            r####"value"####,
            r####"true"####,
        ),
        (
            r####"duration-unit-alias-string"####,
            r####"DurationUnit.toString()"####,
            r####"value"####,
            r####"DurationUnit"####,
        ),
        (
            r####"default"####,
            r####"Duration.default"####,
            r####"error"####,
            r####"–– Pkl Error ––
Cannot find property `default` in object of type `Class`.

1 | Duration.default
    ^^^^^^^^^^^^^^^^
at  (repl:text)

Available properties:
simpleName
"####,
        ),
        (
            r####"duration-class"####,
            r####"1.s.getClass().simpleName"####,
            r####"value"####,
            r####"Duration"####,
        ),
        (
            r####"duration-string"####,
            r####"1.25.s.toString()"####,
            r####"value"####,
            r####"1.25.s"####,
        ),
        (
            r####"duration-if-non-null"####,
            r####"1.s.ifNonNull((it) -> it.toUnit("min"))"####,
            r####"value"####,
            r####"0.016666666666666666.min"####,
        ),
        (
            r####"duration-if-non-null-lazy"####,
            r####"1.s.ifNonNull((it) -> it.value)"####,
            r####"value"####,
            r####"1"####,
        ),
        (
            r####"nullable-duration"####,
            r####"Null(1.s)"####,
            r####"error"####,
            r####"–– Pkl Error ––
Expected value of type `Object | Function<Object>`, but got type `Duration`.
Value: 1.s

2380 | external const function Null(defaultValue: Object | Function<Object>): Null
                                                  ^^^^^^^^^^^^^^^^^^^^^^^^^
at pkl.base#Null (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L2380)

1 | Null(1.s)
    ^^^^^^^^^
at  (repl:text)
"####,
        ),
        (
            r####"type-test"####,
            r####"1.s is Duration"####,
            r####"value"####,
            r####"true"####,
        ),
        (
            r####"type-mismatch"####,
            r####"1.s is DataSize"####,
            r####"value"####,
            r####"false"####,
        ),
        (
            r####"to-unit-invalid"####,
            r####"1.s.toUnit("fortnight")"####,
            r####"error"####,
            r####"–– Pkl Error ––
Expected value of type `"ns"|"us"|"ms"|"s"|"min"|"h"|"d"`, but got `"fortnight"`.

1758 | typealias DurationUnit = "ns" | "us" | "ms" | "s" | "min" | "h" | "d"
                                ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at pkl.base#Duration.toUnit (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L1758)

1 | 1.s.toUnit("fortnight")
    ^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"####,
        ),
        (
            r####"to-unit-case"####,
            r####"1.s.toUnit("S")"####,
            r####"error"####,
            r####"–– Pkl Error ––
Expected value of type `"ns"|"us"|"ms"|"s"|"min"|"h"|"d"`, but got `"S"`.

1758 | typealias DurationUnit = "ns" | "us" | "ms" | "s" | "min" | "h" | "d"
                                ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at pkl.base#Duration.toUnit (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L1758)

1 | 1.s.toUnit("S")
    ^^^^^^^^^^^^^^^
at  (repl:text)
"####,
        ),
        (
            r####"to-unit-surrogate"####,
            r####"1.s.toUnit("\uD800")"####,
            r####"error"####,
            r####"–– Pkl Error ––
Unexpected character `D`. Did you mean `{`?

1 | 1.s.toUnit("\uD800")
                  ^
at repl:text (repl:text)
"####,
        ),
        (
            r####"to-unit-int"####,
            r####"1.s.toUnit(1)"####,
            r####"error"####,
            r####"–– Pkl Error ––
Expected value of type `"ns"|"us"|"ms"|"s"|"min"|"h"|"d"`, but got type `Int`.
Value: 1

1758 | typealias DurationUnit = "ns" | "us" | "ms" | "s" | "min" | "h" | "d"
                                ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at pkl.base#Duration.toUnit (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L1758)

1 | 1.s.toUnit(1)
    ^^^^^^^^^^^^^
at  (repl:text)
"####,
        ),
        (
            r####"to-unit-no-arg"####,
            r####"1.s.toUnit()"####,
            r####"error"####,
            r####"–– Pkl Error ––
Expected 1 function arguments but got 0.

1830 | external function toUnit(unit: DurationUnit): Duration
       ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at pkl.base#Duration.toUnit (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L1830)

1 | 1.s.toUnit()
    ^^^^^^^^^^^^
at  (repl:text)
"####,
        ),
        (
            r####"to-unit-extra"####,
            r####"1.s.toUnit("s","h")"####,
            r####"error"####,
            r####"–– Pkl Error ––
Expected 1 function arguments but got 2.

1830 | external function toUnit(unit: DurationUnit): Duration
       ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at pkl.base#Duration.toUnit (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L1830)

1 | 1.s.toUnit("s","h")
    ^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"####,
        ),
        (
            r####"between-int"####,
            r####"1.s.isBetween(0,2.s)"####,
            r####"error"####,
            r####"–– Pkl Error ––
Expected value of type `Duration`, but got type `Int`.
Value: 0

1821 | external function isBetween(start: Duration, inclusiveEnd: Duration): Boolean
                                          ^^^^^^^^
at pkl.base#Duration.isBetween (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L1821)

1 | 1.s.isBetween(0,2.s)
    ^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"####,
        ),
        (
            r####"between-no-arg"####,
            r####"1.s.isBetween()"####,
            r####"error"####,
            r####"–– Pkl Error ––
Expected 2 function arguments but got 0.

1821 | external function isBetween(start: Duration, inclusiveEnd: Duration): Boolean
       ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at pkl.base#Duration.isBetween (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L1821)

1 | 1.s.isBetween()
    ^^^^^^^^^^^^^^^
at  (repl:text)
"####,
        ),
        (
            r####"between-one-arg"####,
            r####"1.s.isBetween(0.s)"####,
            r####"error"####,
            r####"–– Pkl Error ––
Expected 2 function arguments but got 1.

1821 | external function isBetween(start: Duration, inclusiveEnd: Duration): Boolean
       ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at pkl.base#Duration.isBetween (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L1821)

1 | 1.s.isBetween(0.s)
    ^^^^^^^^^^^^^^^^^^
at  (repl:text)
"####,
        ),
        (
            r####"between-extra"####,
            r####"1.s.isBetween(0.s,2.s,3.s)"####,
            r####"error"####,
            r####"–– Pkl Error ––
Expected 2 function arguments but got 3.

1821 | external function isBetween(start: Duration, inclusiveEnd: Duration): Boolean
       ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at pkl.base#Duration.isBetween (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L1821)

1 | 1.s.isBetween(0.s,2.s,3.s)
    ^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"####,
        ),
        (
            r####"to-duration-invalid"####,
            r####"1.toDuration("fortnight")"####,
            r####"error"####,
            r####"–– Pkl Error ––
Expected value of type `"ns"|"us"|"ms"|"s"|"min"|"h"|"d"`, but got `"fortnight"`.

1758 | typealias DurationUnit = "ns" | "us" | "ms" | "s" | "min" | "h" | "d"
                                ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at pkl.base#Int.toDuration (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L1758)

1 | 1.toDuration("fortnight")
    ^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"####,
        ),
        (
            r####"to-duration-int"####,
            r####"1.toDuration(1)"####,
            r####"error"####,
            r####"–– Pkl Error ––
Expected value of type `"ns"|"us"|"ms"|"s"|"min"|"h"|"d"`, but got type `Int`.
Value: 1

1758 | typealias DurationUnit = "ns" | "us" | "ms" | "s" | "min" | "h" | "d"
                                ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at pkl.base#Int.toDuration (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L1758)

1 | 1.toDuration(1)
    ^^^^^^^^^^^^^^^
at  (repl:text)
"####,
        ),
        (
            r####"to-duration-no-arg"####,
            r####"1.toDuration()"####,
            r####"error"####,
            r####"–– Pkl Error ––
Expected 1 function arguments but got 0.

1048 | external function toDuration(unit: DurationUnit): Duration
       ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at pkl.base#Int.toDuration (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L1048)

1 | 1.toDuration()
    ^^^^^^^^^^^^^^
at  (repl:text)
"####,
        ),
        (
            r####"to-duration-extra"####,
            r####"1.toDuration("s","h")"####,
            r####"error"####,
            r####"–– Pkl Error ––
Expected 1 function arguments but got 2.

1048 | external function toDuration(unit: DurationUnit): Duration
       ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at pkl.base#Int.toDuration (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L1048)

1 | 1.toDuration("s","h")
    ^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"####,
        ),
        (
            r####"add-wrong"####,
            r####"1.s + 1"####,
            r####"error"####,
            r####"–– Pkl Error ––
Operator `+` is not defined for operand types `Duration` and `Int`.
Left operand : 1.s
Right operand: 1

1 | 1.s + 1
    ^^^^^^^
at  (repl:text)
"####,
        ),
        (
            r####"subtract-wrong"####,
            r####"1.s - 1"####,
            r####"error"####,
            r####"–– Pkl Error ––
Operator `-` is not defined for operand types `Duration` and `Int`.
Left operand : 1.s
Right operand: 1

1 | 1.s - 1
    ^^^^^^^
at  (repl:text)
"####,
        ),
        (
            r####"divide-wrong"####,
            r####"1 / 1.s"####,
            r####"error"####,
            r####"–– Pkl Error ––
Operator `/` is not defined for operand types `Int` and `Duration`.
Left operand : 1
Right operand: 1.s

1 | 1 / 1.s
    ^^^^^^^
at  (repl:text)
"####,
        ),
        (
            r####"remainder-wrong"####,
            r####"1.s % 1.s"####,
            r####"error"####,
            r####"–– Pkl Error ––
Operator `%` is not defined for operand types `Duration` and `Duration`.
Left operand : 1.s
Right operand: 1.s

1 | 1.s % 1.s
    ^^^^^^^^^
at  (repl:text)
"####,
        ),
        (
            r####"pow-wrong"####,
            r####"1.s ** 1.s"####,
            r####"error"####,
            r####"–– Pkl Error ––
Operator `**` is not defined for operand types `Duration` and `Duration`.
Left operand : 1.s
Right operand: 1.s

1 | 1.s ** 1.s
    ^^^^^^^^^^
at  (repl:text)
"####,
        ),
        (
            r####"property-wrong"####,
            r####"1.s.noSuchProperty"####,
            r####"error"####,
            r####"–– Pkl Error ––
Cannot find property `noSuchProperty` in object of type `Duration`.

1 | 1.s.noSuchProperty
    ^^^^^^^^^^^^^^^^^^
at  (repl:text)

Available properties:
isPositive
isoString
unit
value
"####,
        ),
    ] {
        let event = query(&mut probe, expression);
        let actual_kind = event.kind;
        let actual = if actual_kind == "error" {
            event.error
        } else {
            event.value
        };
        let expected_units: Vec<_> = expected.encode_utf16().collect();
        if actual_kind != kind || actual.as_deref() != Some(expected_units.as_slice()) {
            mismatches.push(format!(
                "{id}: expected {kind} {expected:?}; actual {actual_kind} {:?}",
                actual.map(|text| String::from_utf16_lossy(&text)),
            ));
        }
    }
    probe.close();
    assert_eq!(probe.collect().slots, 0);
    assert!(mismatches.is_empty(), "{}", mismatches.join("\n\n"));
}
