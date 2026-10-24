//! Complete pinned Java 0.32.0 string observations; typed exports are separate.
//! Authority: m2-int-seq-values/oracle-evidence.tar.gz, capture-01/repeat-0.
#[path = "support/module_keys.rs"]
mod module_keys;
use pkl_runtime::probe::{SourceEvent, SourceProbe};
fn query(probe: &mut SourceProbe, expression: &str) -> SourceEvent {
    probe
        .evaluate_module_source(
            "file:///stage61/intseq-values.pkl",
            Some(include_str!(
                "../../../tests/fixtures/m2-int-seq-values/module.pkl"
            )),
            expression,
            0,
            None,
        )
        .unwrap();
    for _ in 0..200_000 {
        probe.collect();
        let event = probe.poll(0, 16).unwrap();
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
    panic!("IntSeq public-query poll bound")
}
#[test]
fn complete_java_int_seq_string_matrix() {
    let mut probe = SourceProbe::new_module_evaluator("compact", false, false, false).unwrap();
    let mut mismatches = Vec::new();
    let rows = [
        (
            r########"asc-start"########,
            r########"IntSeq(1, 4).start"########,
            r########"value"########,
            r########"1"########,
        ),
        (
            r########"asc-end"########,
            r########"IntSeq(1, 4).end"########,
            r########"value"########,
            r########"4"########,
        ),
        (
            r########"asc-step"########,
            r########"IntSeq(1, 4).step"########,
            r########"value"########,
            r########"1"########,
        ),
        (
            r########"asc-string"########,
            r########"IntSeq(1, 4).toString()"########,
            r########"value"########,
            r########"IntSeq(1, 4)"########,
        ),
        (
            r########"zero-start"########,
            r########"IntSeq(0, 0).start"########,
            r########"value"########,
            r########"0"########,
        ),
        (
            r########"zero-end"########,
            r########"IntSeq(0, 0).end"########,
            r########"value"########,
            r########"0"########,
        ),
        (
            r########"zero-step"########,
            r########"IntSeq(0, 0).step"########,
            r########"value"########,
            r########"1"########,
        ),
        (
            r########"zero-string"########,
            r########"IntSeq(0, 0).toString()"########,
            r########"value"########,
            r########"IntSeq(0, 0)"########,
        ),
        (
            r########"one-start"########,
            r########"IntSeq(1, 1).start"########,
            r########"value"########,
            r########"1"########,
        ),
        (
            r########"one-end"########,
            r########"IntSeq(1, 1).end"########,
            r########"value"########,
            r########"1"########,
        ),
        (
            r########"one-step"########,
            r########"IntSeq(1, 1).step"########,
            r########"value"########,
            r########"1"########,
        ),
        (
            r########"one-string"########,
            r########"IntSeq(1, 1).toString()"########,
            r########"value"########,
            r########"IntSeq(1, 1)"########,
        ),
        (
            r########"empty-start"########,
            r########"IntSeq(5, 2).start"########,
            r########"value"########,
            r########"5"########,
        ),
        (
            r########"empty-end"########,
            r########"IntSeq(5, 2).end"########,
            r########"value"########,
            r########"2"########,
        ),
        (
            r########"empty-step"########,
            r########"IntSeq(5, 2).step"########,
            r########"value"########,
            r########"1"########,
        ),
        (
            r########"empty-string"########,
            r########"IntSeq(5, 2).toString()"########,
            r########"value"########,
            r########"IntSeq(5, 2)"########,
        ),
        (
            r########"empty-neg-start"########,
            r########"IntSeq(0, 2).step(-1).start"########,
            r########"value"########,
            r########"0"########,
        ),
        (
            r########"empty-neg-end"########,
            r########"IntSeq(0, 2).step(-1).end"########,
            r########"value"########,
            r########"2"########,
        ),
        (
            r########"empty-neg-step"########,
            r########"IntSeq(0, 2).step(-1).step"########,
            r########"value"########,
            r########"-1"########,
        ),
        (
            r########"empty-neg-string"########,
            r########"IntSeq(0, 2).step(-1).toString()"########,
            r########"value"########,
            r########"IntSeq(0, 2).step(-1)"########,
        ),
        (
            r########"desc-start"########,
            r########"IntSeq(5, 1).step(-1).start"########,
            r########"value"########,
            r########"5"########,
        ),
        (
            r########"desc-end"########,
            r########"IntSeq(5, 1).step(-1).end"########,
            r########"value"########,
            r########"1"########,
        ),
        (
            r########"desc-step"########,
            r########"IntSeq(5, 1).step(-1).step"########,
            r########"value"########,
            r########"-1"########,
        ),
        (
            r########"desc-string"########,
            r########"IntSeq(5, 1).step(-1).toString()"########,
            r########"value"########,
            r########"IntSeq(5, 1).step(-1)"########,
        ),
        (
            r########"desc-two-start"########,
            r########"IntSeq(5, 0).step(-2).start"########,
            r########"value"########,
            r########"5"########,
        ),
        (
            r########"desc-two-end"########,
            r########"IntSeq(5, 0).step(-2).end"########,
            r########"value"########,
            r########"0"########,
        ),
        (
            r########"desc-two-step"########,
            r########"IntSeq(5, 0).step(-2).step"########,
            r########"value"########,
            r########"-2"########,
        ),
        (
            r########"desc-two-string"########,
            r########"IntSeq(5, 0).step(-2).toString()"########,
            r########"value"########,
            r########"IntSeq(5, 0).step(-2)"########,
        ),
        (
            r########"asc-two-start"########,
            r########"IntSeq(0, 5).step(2).start"########,
            r########"value"########,
            r########"0"########,
        ),
        (
            r########"asc-two-end"########,
            r########"IntSeq(0, 5).step(2).end"########,
            r########"value"########,
            r########"5"########,
        ),
        (
            r########"asc-two-step"########,
            r########"IntSeq(0, 5).step(2).step"########,
            r########"value"########,
            r########"2"########,
        ),
        (
            r########"asc-two-string"########,
            r########"IntSeq(0, 5).step(2).toString()"########,
            r########"value"########,
            r########"IntSeq(0, 5).step(2)"########,
        ),
        (
            r########"asc-over-start"########,
            r########"IntSeq(0, 1).step(3).start"########,
            r########"value"########,
            r########"0"########,
        ),
        (
            r########"asc-over-end"########,
            r########"IntSeq(0, 1).step(3).end"########,
            r########"value"########,
            r########"1"########,
        ),
        (
            r########"asc-over-step"########,
            r########"IntSeq(0, 1).step(3).step"########,
            r########"value"########,
            r########"3"########,
        ),
        (
            r########"asc-over-string"########,
            r########"IntSeq(0, 1).step(3).toString()"########,
            r########"value"########,
            r########"IntSeq(0, 1).step(3)"########,
        ),
        (
            r########"cross-start"########,
            r########"IntSeq(-3, 2).step(2).start"########,
            r########"value"########,
            r########"-3"########,
        ),
        (
            r########"cross-end"########,
            r########"IntSeq(-3, 2).step(2).end"########,
            r########"value"########,
            r########"2"########,
        ),
        (
            r########"cross-step"########,
            r########"IntSeq(-3, 2).step(2).step"########,
            r########"value"########,
            r########"2"########,
        ),
        (
            r########"cross-string"########,
            r########"IntSeq(-3, 2).step(2).toString()"########,
            r########"value"########,
            r########"IntSeq(-3, 2).step(2)"########,
        ),
        (
            r########"min-single-start"########,
            r########"IntSeq(minInt, minInt).start"########,
            r########"value"########,
            r########"-9223372036854775808"########,
        ),
        (
            r########"min-single-end"########,
            r########"IntSeq(minInt, minInt).end"########,
            r########"value"########,
            r########"-9223372036854775808"########,
        ),
        (
            r########"min-single-step"########,
            r########"IntSeq(minInt, minInt).step"########,
            r########"value"########,
            r########"1"########,
        ),
        (
            r########"min-single-string"########,
            r########"IntSeq(minInt, minInt).toString()"########,
            r########"value"########,
            r########"IntSeq(-9223372036854775808, -9223372036854775808)"########,
        ),
        (
            r########"max-single-start"########,
            r########"IntSeq(maxInt, maxInt).start"########,
            r########"value"########,
            r########"9223372036854775807"########,
        ),
        (
            r########"max-single-end"########,
            r########"IntSeq(maxInt, maxInt).end"########,
            r########"value"########,
            r########"9223372036854775807"########,
        ),
        (
            r########"max-single-step"########,
            r########"IntSeq(maxInt, maxInt).step"########,
            r########"value"########,
            r########"1"########,
        ),
        (
            r########"max-single-string"########,
            r########"IntSeq(maxInt, maxInt).toString()"########,
            r########"value"########,
            r########"IntSeq(9223372036854775807, 9223372036854775807)"########,
        ),
        (
            r########"extreme-up-start"########,
            r########"IntSeq(minInt, maxInt).step(maxInt).start"########,
            r########"value"########,
            r########"-9223372036854775808"########,
        ),
        (
            r########"extreme-up-end"########,
            r########"IntSeq(minInt, maxInt).step(maxInt).end"########,
            r########"value"########,
            r########"9223372036854775807"########,
        ),
        (
            r########"extreme-up-step"########,
            r########"IntSeq(minInt, maxInt).step(maxInt).step"########,
            r########"value"########,
            r########"9223372036854775807"########,
        ),
        (
            r########"extreme-up-string"########,
            r########"IntSeq(minInt, maxInt).step(maxInt).toString()"########,
            r########"value"########,
            r########"IntSeq(-9223372036854775808, 9223372036854775807).step(9223372036854775807)"########,
        ),
        (
            r########"extreme-down-start"########,
            r########"IntSeq(maxInt, minInt).step(minInt).start"########,
            r########"value"########,
            r########"9223372036854775807"########,
        ),
        (
            r########"extreme-down-end"########,
            r########"IntSeq(maxInt, minInt).step(minInt).end"########,
            r########"value"########,
            r########"-9223372036854775808"########,
        ),
        (
            r########"extreme-down-step"########,
            r########"IntSeq(maxInt, minInt).step(minInt).step"########,
            r########"value"########,
            r########"-9223372036854775808"########,
        ),
        (
            r########"extreme-down-string"########,
            r########"IntSeq(maxInt, minInt).step(minInt).toString()"########,
            r########"value"########,
            r########"IntSeq(9223372036854775807, -9223372036854775808).step(-9223372036854775808)"########,
        ),
        (
            r########"min-step-empty-start"########,
            r########"IntSeq(0, 1).step(minInt).start"########,
            r########"value"########,
            r########"0"########,
        ),
        (
            r########"min-step-empty-end"########,
            r########"IntSeq(0, 1).step(minInt).end"########,
            r########"value"########,
            r########"1"########,
        ),
        (
            r########"min-step-empty-step"########,
            r########"IntSeq(0, 1).step(minInt).step"########,
            r########"value"########,
            r########"-9223372036854775808"########,
        ),
        (
            r########"min-step-empty-string"########,
            r########"IntSeq(0, 1).step(minInt).toString()"########,
            r########"value"########,
            r########"IntSeq(0, 1).step(-9223372036854775808)"########,
        ),
        (
            r########"asc-list"########,
            r########"IntSeq(1, 4).toList()"########,
            r########"value"########,
            r########"List(1, 2, 3, 4)"########,
        ),
        (
            r########"asc-map"########,
            r########"IntSeq(1, 4).map((n) -> n)"########,
            r########"value"########,
            r########"List(1, 2, 3, 4)"########,
        ),
        (
            r########"asc-fold"########,
            r########"IntSeq(1, 4).fold(List(), (acc, n) -> acc.add(n))"########,
            r########"value"########,
            r########"List(1, 2, 3, 4)"########,
        ),
        (
            r########"asc-listing"########,
            r########"IntSeq(1, 4).toListing()"########,
            r########"value"########,
            r########"new Listing { 1; 2; 3; 4 }"########,
        ),
        (
            r########"zero-list"########,
            r########"IntSeq(0, 0).toList()"########,
            r########"value"########,
            r########"List(0)"########,
        ),
        (
            r########"zero-map"########,
            r########"IntSeq(0, 0).map((n) -> n)"########,
            r########"value"########,
            r########"List(0)"########,
        ),
        (
            r########"zero-fold"########,
            r########"IntSeq(0, 0).fold(List(), (acc, n) -> acc.add(n))"########,
            r########"value"########,
            r########"List(0)"########,
        ),
        (
            r########"zero-listing"########,
            r########"IntSeq(0, 0).toListing()"########,
            r########"value"########,
            r########"new Listing { 0 }"########,
        ),
        (
            r########"one-list"########,
            r########"IntSeq(1, 1).toList()"########,
            r########"value"########,
            r########"List(1)"########,
        ),
        (
            r########"one-map"########,
            r########"IntSeq(1, 1).map((n) -> n)"########,
            r########"value"########,
            r########"List(1)"########,
        ),
        (
            r########"one-fold"########,
            r########"IntSeq(1, 1).fold(List(), (acc, n) -> acc.add(n))"########,
            r########"value"########,
            r########"List(1)"########,
        ),
        (
            r########"one-listing"########,
            r########"IntSeq(1, 1).toListing()"########,
            r########"value"########,
            r########"new Listing { 1 }"########,
        ),
        (
            r########"empty-list"########,
            r########"IntSeq(5, 2).toList()"########,
            r########"value"########,
            r########"List()"########,
        ),
        (
            r########"empty-map"########,
            r########"IntSeq(5, 2).map((n) -> n)"########,
            r########"value"########,
            r########"List()"########,
        ),
        (
            r########"empty-fold"########,
            r########"IntSeq(5, 2).fold(List(), (acc, n) -> acc.add(n))"########,
            r########"value"########,
            r########"List()"########,
        ),
        (
            r########"empty-listing"########,
            r########"IntSeq(5, 2).toListing()"########,
            r########"value"########,
            r########"new Listing {}"########,
        ),
        (
            r########"empty-neg-list"########,
            r########"IntSeq(0, 2).step(-1).toList()"########,
            r########"value"########,
            r########"List()"########,
        ),
        (
            r########"empty-neg-map"########,
            r########"IntSeq(0, 2).step(-1).map((n) -> n)"########,
            r########"value"########,
            r########"List()"########,
        ),
        (
            r########"empty-neg-fold"########,
            r########"IntSeq(0, 2).step(-1).fold(List(), (acc, n) -> acc.add(n))"########,
            r########"value"########,
            r########"List()"########,
        ),
        (
            r########"empty-neg-listing"########,
            r########"IntSeq(0, 2).step(-1).toListing()"########,
            r########"value"########,
            r########"new Listing {}"########,
        ),
        (
            r########"desc-list"########,
            r########"IntSeq(5, 1).step(-1).toList()"########,
            r########"value"########,
            r########"List(5, 4, 3, 2, 1)"########,
        ),
        (
            r########"desc-map"########,
            r########"IntSeq(5, 1).step(-1).map((n) -> n)"########,
            r########"value"########,
            r########"List(5, 4, 3, 2, 1)"########,
        ),
        (
            r########"desc-fold"########,
            r########"IntSeq(5, 1).step(-1).fold(List(), (acc, n) -> acc.add(n))"########,
            r########"value"########,
            r########"List(5, 4, 3, 2, 1)"########,
        ),
        (
            r########"desc-listing"########,
            r########"IntSeq(5, 1).step(-1).toListing()"########,
            r########"value"########,
            r########"new Listing { 5; 4; 3; 2; 1 }"########,
        ),
        (
            r########"desc-two-list"########,
            r########"IntSeq(5, 0).step(-2).toList()"########,
            r########"value"########,
            r########"List(5, 3, 1)"########,
        ),
        (
            r########"desc-two-map"########,
            r########"IntSeq(5, 0).step(-2).map((n) -> n)"########,
            r########"value"########,
            r########"List(5, 3, 1)"########,
        ),
        (
            r########"desc-two-fold"########,
            r########"IntSeq(5, 0).step(-2).fold(List(), (acc, n) -> acc.add(n))"########,
            r########"value"########,
            r########"List(5, 3, 1)"########,
        ),
        (
            r########"desc-two-listing"########,
            r########"IntSeq(5, 0).step(-2).toListing()"########,
            r########"value"########,
            r########"new Listing { 5; 3; 1 }"########,
        ),
        (
            r########"asc-two-list"########,
            r########"IntSeq(0, 5).step(2).toList()"########,
            r########"value"########,
            r########"List(0, 2, 4)"########,
        ),
        (
            r########"asc-two-map"########,
            r########"IntSeq(0, 5).step(2).map((n) -> n)"########,
            r########"value"########,
            r########"List(0, 2, 4)"########,
        ),
        (
            r########"asc-two-fold"########,
            r########"IntSeq(0, 5).step(2).fold(List(), (acc, n) -> acc.add(n))"########,
            r########"value"########,
            r########"List(0, 2, 4)"########,
        ),
        (
            r########"asc-two-listing"########,
            r########"IntSeq(0, 5).step(2).toListing()"########,
            r########"value"########,
            r########"new Listing { 0; 2; 4 }"########,
        ),
        (
            r########"asc-over-list"########,
            r########"IntSeq(0, 1).step(3).toList()"########,
            r########"value"########,
            r########"List(0)"########,
        ),
        (
            r########"asc-over-map"########,
            r########"IntSeq(0, 1).step(3).map((n) -> n)"########,
            r########"value"########,
            r########"List(0)"########,
        ),
        (
            r########"asc-over-fold"########,
            r########"IntSeq(0, 1).step(3).fold(List(), (acc, n) -> acc.add(n))"########,
            r########"value"########,
            r########"List(0)"########,
        ),
        (
            r########"asc-over-listing"########,
            r########"IntSeq(0, 1).step(3).toListing()"########,
            r########"value"########,
            r########"new Listing { 0 }"########,
        ),
        (
            r########"cross-list"########,
            r########"IntSeq(-3, 2).step(2).toList()"########,
            r########"value"########,
            r########"List(-3, -1, 1)"########,
        ),
        (
            r########"cross-map"########,
            r########"IntSeq(-3, 2).step(2).map((n) -> n)"########,
            r########"value"########,
            r########"List(-3, -1, 1)"########,
        ),
        (
            r########"cross-fold"########,
            r########"IntSeq(-3, 2).step(2).fold(List(), (acc, n) -> acc.add(n))"########,
            r########"value"########,
            r########"List(-3, -1, 1)"########,
        ),
        (
            r########"cross-listing"########,
            r########"IntSeq(-3, 2).step(2).toListing()"########,
            r########"value"########,
            r########"new Listing { -3; -1; 1 }"########,
        ),
        (
            r########"min-single-list"########,
            r########"IntSeq(minInt, minInt).toList()"########,
            r########"value"########,
            r########"List(-9223372036854775808)"########,
        ),
        (
            r########"min-single-map"########,
            r########"IntSeq(minInt, minInt).map((n) -> n)"########,
            r########"value"########,
            r########"List(-9223372036854775808)"########,
        ),
        (
            r########"min-single-fold"########,
            r########"IntSeq(minInt, minInt).fold(List(), (acc, n) -> acc.add(n))"########,
            r########"value"########,
            r########"List(-9223372036854775808)"########,
        ),
        (
            r########"min-single-listing"########,
            r########"IntSeq(minInt, minInt).toListing()"########,
            r########"value"########,
            r########"new Listing { -9223372036854775808 }"########,
        ),
        (
            r########"max-single-list"########,
            r########"IntSeq(maxInt, maxInt).toList()"########,
            r########"value"########,
            r########"List(9223372036854775807)"########,
        ),
        (
            r########"max-single-map"########,
            r########"IntSeq(maxInt, maxInt).map((n) -> n)"########,
            r########"value"########,
            r########"List(9223372036854775807)"########,
        ),
        (
            r########"max-single-fold"########,
            r########"IntSeq(maxInt, maxInt).fold(List(), (acc, n) -> acc.add(n))"########,
            r########"value"########,
            r########"List(9223372036854775807)"########,
        ),
        (
            r########"max-single-listing"########,
            r########"IntSeq(maxInt, maxInt).toListing()"########,
            r########"value"########,
            r########"new Listing { 9223372036854775807 }"########,
        ),
        (
            r########"extreme-up-list"########,
            r########"IntSeq(minInt, maxInt).step(maxInt).toList()"########,
            r########"value"########,
            r########"List(-9223372036854775808, -1, 9223372036854775806)"########,
        ),
        (
            r########"extreme-up-map"########,
            r########"IntSeq(minInt, maxInt).step(maxInt).map((n) -> n)"########,
            r########"value"########,
            r########"List(-9223372036854775808, -1, 9223372036854775806)"########,
        ),
        (
            r########"extreme-up-fold"########,
            r########"IntSeq(minInt, maxInt).step(maxInt).fold(List(), (acc, n) -> acc.add(n))"########,
            r########"value"########,
            r########"List(-9223372036854775808, -1, 9223372036854775806)"########,
        ),
        (
            r########"extreme-down-list"########,
            r########"IntSeq(maxInt, minInt).step(minInt).toList()"########,
            r########"value"########,
            r########"List(9223372036854775807, -1)"########,
        ),
        (
            r########"extreme-down-map"########,
            r########"IntSeq(maxInt, minInt).step(minInt).map((n) -> n)"########,
            r########"value"########,
            r########"List(9223372036854775807, -1)"########,
        ),
        (
            r########"extreme-down-fold"########,
            r########"IntSeq(maxInt, minInt).step(minInt).fold(List(), (acc, n) -> acc.add(n))"########,
            r########"value"########,
            r########"List(9223372036854775807, -1)"########,
        ),
        (
            r########"min-step-empty-list"########,
            r########"IntSeq(0, 1).step(minInt).toList()"########,
            r########"value"########,
            r########"List()"########,
        ),
        (
            r########"min-step-empty-map"########,
            r########"IntSeq(0, 1).step(minInt).map((n) -> n)"########,
            r########"value"########,
            r########"List()"########,
        ),
        (
            r########"min-step-empty-fold"########,
            r########"IntSeq(0, 1).step(minInt).fold(List(), (acc, n) -> acc.add(n))"########,
            r########"value"########,
            r########"List()"########,
        ),
        (
            r########"min-step-empty-listing"########,
            r########"IntSeq(0, 1).step(minInt).toListing()"########,
            r########"value"########,
            r########"new Listing {}"########,
        ),
        (
            r########"eq-same"########,
            r########"IntSeq(2,5) == IntSeq(2,5)"########,
            r########"value"########,
            r########"true"########,
        ),
        (
            r########"neq-same"########,
            r########"IntSeq(2,5) != IntSeq(2,5)"########,
            r########"value"########,
            r########"false"########,
        ),
        (
            r########"eq-different"########,
            r########"IntSeq(2,5) == IntSeq(2,6)"########,
            r########"value"########,
            r########"false"########,
        ),
        (
            r########"neq-different"########,
            r########"IntSeq(2,5) != IntSeq(2,6)"########,
            r########"value"########,
            r########"true"########,
        ),
        (
            r########"eq-empty-origins"########,
            r########"IntSeq(5,2) == IntSeq(100,0)"########,
            r########"value"########,
            r########"true"########,
        ),
        (
            r########"neq-empty-origins"########,
            r########"IntSeq(5,2) != IntSeq(100,0)"########,
            r########"value"########,
            r########"false"########,
        ),
        (
            r########"eq-empty-direction"########,
            r########"IntSeq(5,2) == IntSeq(0,2).step(-1)"########,
            r########"value"########,
            r########"true"########,
        ),
        (
            r########"neq-empty-direction"########,
            r########"IntSeq(5,2) != IntSeq(0,2).step(-1)"########,
            r########"value"########,
            r########"false"########,
        ),
        (
            r########"eq-aligned-end-up"########,
            r########"IntSeq(0,4).step(2) == IntSeq(0,5).step(2)"########,
            r########"value"########,
            r########"true"########,
        ),
        (
            r########"neq-aligned-end-up"########,
            r########"IntSeq(0,4).step(2) != IntSeq(0,5).step(2)"########,
            r########"value"########,
            r########"false"########,
        ),
        (
            r########"eq-aligned-end-down"########,
            r########"IntSeq(5,1).step(-2) == IntSeq(5,0).step(-2)"########,
            r########"value"########,
            r########"true"########,
        ),
        (
            r########"neq-aligned-end-down"########,
            r########"IntSeq(5,1).step(-2) != IntSeq(5,0).step(-2)"########,
            r########"value"########,
            r########"false"########,
        ),
        (
            r########"eq-singleton-step"########,
            r########"IntSeq(1,1) == IntSeq(1,1).step(2)"########,
            r########"value"########,
            r########"false"########,
        ),
        (
            r########"neq-singleton-step"########,
            r########"IntSeq(1,1) != IntSeq(1,1).step(2)"########,
            r########"value"########,
            r########"true"########,
        ),
        (
            r########"eq-singleton-negative"########,
            r########"IntSeq(1,1) == IntSeq(1,1).step(-1)"########,
            r########"value"########,
            r########"false"########,
        ),
        (
            r########"neq-singleton-negative"########,
            r########"IntSeq(1,1) != IntSeq(1,1).step(-1)"########,
            r########"value"########,
            r########"true"########,
        ),
        (
            r########"eq-nondivisible-step"########,
            r########"IntSeq(0,5).step(2) == IntSeq(0,4).step(2)"########,
            r########"value"########,
            r########"true"########,
        ),
        (
            r########"neq-nondivisible-step"########,
            r########"IntSeq(0,5).step(2) != IntSeq(0,4).step(2)"########,
            r########"value"########,
            r########"false"########,
        ),
        (
            r########"eq-opposite-direction"########,
            r########"IntSeq(1,3) == IntSeq(3,1).step(-1)"########,
            r########"value"########,
            r########"false"########,
        ),
        (
            r########"neq-opposite-direction"########,
            r########"IntSeq(1,3) != IntSeq(3,1).step(-1)"########,
            r########"value"########,
            r########"true"########,
        ),
        (
            r########"eq-cross-type-list"########,
            r########"IntSeq(1,3) == List(1,2,3)"########,
            r########"value"########,
            r########"false"########,
        ),
        (
            r########"neq-cross-type-list"########,
            r########"IntSeq(1,3) != List(1,2,3)"########,
            r########"value"########,
            r########"true"########,
        ),
        (
            r########"eq-extreme-up-self"########,
            r########"IntSeq(minInt,maxInt).step(maxInt) == IntSeq(minInt,maxInt).step(maxInt)"########,
            r########"value"########,
            r########"true"########,
        ),
        (
            r########"neq-extreme-up-self"########,
            r########"IntSeq(minInt,maxInt).step(maxInt) != IntSeq(minInt,maxInt).step(maxInt)"########,
            r########"value"########,
            r########"false"########,
        ),
        (
            r########"set-empty"########,
            r########"Set(IntSeq(5,2),IntSeq(100,0)).length"########,
            r########"value"########,
            r########"1"########,
        ),
        (
            r########"set-aligned"########,
            r########"Set(IntSeq(0,4).step(2),IntSeq(0,5).step(2)).length"########,
            r########"value"########,
            r########"1"########,
        ),
        (
            r########"set-singleton-step"########,
            r########"Set(IntSeq(1,1),IntSeq(1,1).step(2)).length"########,
            r########"value"########,
            r########"2"########,
        ),
        (
            r########"map-empty"########,
            r########"Map(IntSeq(5,2),"a",IntSeq(100,0),"b").length"########,
            r########"value"########,
            r########"1"########,
        ),
        (
            r########"map-aligned"########,
            r########"Map(IntSeq(0,4).step(2),"a",IntSeq(0,5).step(2),"b").length"########,
            r########"value"########,
            r########"1"########,
        ),
        (
            r########"map-singleton-step"########,
            r########"Map(IntSeq(1,1),"a",IntSeq(1,1).step(2),"b").length"########,
            r########"value"########,
            r########"2"########,
        ),
        (
            r########"set-threshold-five"########,
            r########"Set(IntSeq(1,1),IntSeq(2,2),IntSeq(3,3),IntSeq(4,4),IntSeq(1,1)).length"########,
            r########"value"########,
            r########"4"########,
        ),
        (
            r########"for-values-asc"########,
            r########"new Listing { for (n in IntSeq(1,3)) { n } }"########,
            r########"value"########,
            r########"new Listing { 1; 2; 3 }"########,
        ),
        (
            r########"for-pairs-asc"########,
            r########"new Listing { for (i, n in IntSeq(1,3)) { Pair(i,n) } }"########,
            r########"value"########,
            r########"new Listing { Pair(0, 1); Pair(1, 2); Pair(2, 3) }"########,
        ),
        (
            r########"spread-listing-asc"########,
            r########"new Listing { ...IntSeq(1,3) }"########,
            r########"value"########,
            r########"new Listing { 1; 2; 3 }"########,
        ),
        (
            r########"spread-dynamic-asc"########,
            r########"new Dynamic { ...IntSeq(1,3) }"########,
            r########"value"########,
            r########"new Dynamic { 1; 2; 3 }"########,
        ),
        (
            r########"for-values-empty"########,
            r########"new Listing { for (n in IntSeq(3,1)) { n } }"########,
            r########"value"########,
            r########"new Listing {}"########,
        ),
        (
            r########"for-pairs-empty"########,
            r########"new Listing { for (i, n in IntSeq(3,1)) { Pair(i,n) } }"########,
            r########"value"########,
            r########"new Listing {}"########,
        ),
        (
            r########"spread-listing-empty"########,
            r########"new Listing { ...IntSeq(3,1) }"########,
            r########"value"########,
            r########"new Listing {}"########,
        ),
        (
            r########"spread-dynamic-empty"########,
            r########"new Dynamic { ...IntSeq(3,1) }"########,
            r########"value"########,
            r########"new Dynamic {}"########,
        ),
        (
            r########"for-values-desc"########,
            r########"new Listing { for (n in IntSeq(3,1).step(-1)) { n } }"########,
            r########"value"########,
            r########"new Listing { 3; 2; 1 }"########,
        ),
        (
            r########"for-pairs-desc"########,
            r########"new Listing { for (i, n in IntSeq(3,1).step(-1)) { Pair(i,n) } }"########,
            r########"value"########,
            r########"new Listing { Pair(0, 3); Pair(1, 2); Pair(2, 1) }"########,
        ),
        (
            r########"spread-listing-desc"########,
            r########"new Listing { ...IntSeq(3,1).step(-1) }"########,
            r########"value"########,
            r########"new Listing { 3; 2; 1 }"########,
        ),
        (
            r########"spread-dynamic-desc"########,
            r########"new Dynamic { ...IntSeq(3,1).step(-1) }"########,
            r########"value"########,
            r########"new Dynamic { 3; 2; 1 }"########,
        ),
        (
            r########"for-values-step"########,
            r########"new Listing { for (n in IntSeq(0,5).step(2)) { n } }"########,
            r########"value"########,
            r########"new Listing { 0; 2; 4 }"########,
        ),
        (
            r########"for-pairs-step"########,
            r########"new Listing { for (i, n in IntSeq(0,5).step(2)) { Pair(i,n) } }"########,
            r########"value"########,
            r########"new Listing { Pair(0, 0); Pair(1, 2); Pair(2, 4) }"########,
        ),
        (
            r########"spread-listing-step"########,
            r########"new Listing { ...IntSeq(0,5).step(2) }"########,
            r########"value"########,
            r########"new Listing { 0; 2; 4 }"########,
        ),
        (
            r########"spread-dynamic-step"########,
            r########"new Dynamic { ...IntSeq(0,5).step(2) }"########,
            r########"value"########,
            r########"new Dynamic { 0; 2; 4 }"########,
        ),
        (
            r########"for-values-extreme-up"########,
            r########"new Listing { for (n in IntSeq(minInt, maxInt).step(maxInt)) { n } }"########,
            r########"value"########,
            r########"new Listing { -9223372036854775808 }"########,
        ),
        (
            r########"for-pairs-extreme-up"########,
            r########"new Listing { for (i, n in IntSeq(minInt, maxInt).step(maxInt)) { Pair(i,n) } }"########,
            r########"value"########,
            r########"new Listing { Pair(0, -9223372036854775808) }"########,
        ),
        (
            r########"spread-listing-extreme-up"########,
            r########"new Listing { ...IntSeq(minInt, maxInt).step(maxInt) }"########,
            r########"value"########,
            r########"new Listing { -9223372036854775808; -1; 9223372036854775806 }"########,
        ),
        (
            r########"spread-dynamic-extreme-up"########,
            r########"new Dynamic { ...IntSeq(minInt, maxInt).step(maxInt) }"########,
            r########"value"########,
            r########"new Dynamic { -9223372036854775808; -1; 9223372036854775806 }"########,
        ),
        (
            r########"for-values-extreme-down"########,
            r########"new Listing { for (n in IntSeq(maxInt, minInt).step(minInt)) { n } }"########,
            r########"value"########,
            r########"new Listing { 9223372036854775807 }"########,
        ),
        (
            r########"for-pairs-extreme-down"########,
            r########"new Listing { for (i, n in IntSeq(maxInt, minInt).step(minInt)) { Pair(i,n) } }"########,
            r########"value"########,
            r########"new Listing { Pair(0, 9223372036854775807) }"########,
        ),
        (
            r########"spread-listing-extreme-down"########,
            r########"new Listing { ...IntSeq(maxInt, minInt).step(minInt) }"########,
            r########"value"########,
            r########"new Listing { 9223372036854775807; -1 }"########,
        ),
        (
            r########"spread-dynamic-extreme-down"########,
            r########"new Dynamic { ...IntSeq(maxInt, minInt).step(minInt) }"########,
            r########"value"########,
            r########"new Dynamic { 9223372036854775807; -1 }"########,
        ),
        (
            r########"typed-for-key"########,
            r########"new Listing { for (i: String, n: Int in IntSeq(1,3)) { n } }"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected value of type `String`, but got type `Int`.
Value: 0

1 | new Listing { for (i: String, n: Int in IntSeq(1,3)) { n } }
                          ^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"typed-for-value"########,
            r########"new Listing { for (i: Int, n: String in IntSeq(1,3)) { n } }"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected value of type `String`, but got type `Int`.
Value: 1

1 | new Listing { for (i: Int, n: String in IntSeq(1,3)) { n } }
                                  ^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"typed-for-empty-key"########,
            r########"new Listing { for (i: String, n: Int in IntSeq(3,1)) { n } }"########,
            r########"value"########,
            r########"new Listing {}"########,
        ),
        (
            r########"typed-for-empty-value"########,
            r########"new Listing { for (i: Int, n: String in IntSeq(3,1)) { n } }"########,
            r########"value"########,
            r########"new Listing {}"########,
        ),
        (
            r########"spread-mapping"########,
            r########"new Mapping { ...IntSeq(1,3) }"########,
            r########"error"########,
            r########"–– Pkl Error ––
Cannot spread value of type `IntSeq` into object of type `Mapping`.
Value: IntSeq(1, 3)

1 | new Mapping { ...IntSeq(1,3) }
                  ^^^^^^^^^^^^^^
at  (repl:text)

`IntSeq` can only be spread into objects of type `Dynamic` and `Listing`.
"########,
        ),
        (
            r########"spread-empty-mapping"########,
            r########"new Mapping { ...IntSeq(3,1) }"########,
            r########"error"########,
            r########"–– Pkl Error ––
Cannot spread value of type `IntSeq` into object of type `Mapping`.
Value: IntSeq(3, 1)

1 | new Mapping { ...IntSeq(3,1) }
                  ^^^^^^^^^^^^^^
at  (repl:text)

`IntSeq` can only be spread into objects of type `Dynamic` and `Listing`.
"########,
        ),
        (
            r########"spread-typed-object"########,
            r########"new SeqHolder { ...IntSeq(1,3) }"########,
            r########"error"########,
            r########"–– Pkl Error ––
Cannot spread value of type `IntSeq` into object of type `intseq-values#SeqHolder`.
Value: IntSeq(1, 3)

1 | new SeqHolder { ...IntSeq(1,3) }
                    ^^^^^^^^^^^^^^
at  (repl:text)

`IntSeq` can only be spread into objects of type `Dynamic` and `Listing`.
"########,
        ),
        (
            r########"spread-empty-typed-object"########,
            r########"new SeqHolder { ...IntSeq(3,1) }"########,
            r########"error"########,
            r########"–– Pkl Error ––
Cannot spread value of type `IntSeq` into object of type `intseq-values#SeqHolder`.
Value: IntSeq(3, 1)

1 | new SeqHolder { ...IntSeq(3,1) }
                    ^^^^^^^^^^^^^^
at  (repl:text)

`IntSeq` can only be spread into objects of type `Dynamic` and `Listing`.
"########,
        ),
        (
            r########"class"########,
            r########"IntSeq(1,3).getClass().simpleName"########,
            r########"value"########,
            r########"IntSeq"########,
        ),
        (
            r########"string"########,
            r########"IntSeq(1,3).toString()"########,
            r########"value"########,
            r########"IntSeq(1, 3)"########,
        ),
        (
            r########"ifnonnull-start"########,
            r########"IntSeq(1,3).ifNonNull((x) -> x.start)"########,
            r########"value"########,
            r########"1"########,
        ),
        (
            r########"ifnonnull-length"########,
            r########"IntSeq(1,3).ifNonNull((x) -> x.length)"########,
            r########"error"########,
            r########"–– Pkl Error ––
Cannot find property `length` in object of type `IntSeq`.

1 | IntSeq(1,3).ifNonNull((x) -> x.length)
                                 ^^^^^^^^
at repl:text.<function#1> (repl:text)

Available properties:
end
start
step

1 | IntSeq(1,3).ifNonNull((x) -> x.length)
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"is-intseq"########,
            r########"IntSeq(1,3) is IntSeq"########,
            r########"value"########,
            r########"true"########,
        ),
        (
            r########"is-list"########,
            r########"IntSeq(1,3) is List"########,
            r########"value"########,
            r########"false"########,
        ),
        (
            r########"as-intseq"########,
            r########"IntSeq(1,3) as IntSeq"########,
            r########"value"########,
            r########"IntSeq(1, 3)"########,
        ),
        (
            r########"missing-length"########,
            r########"IntSeq(1,3).length"########,
            r########"error"########,
            r########"–– Pkl Error ––
Cannot find property `length` in object of type `IntSeq`.

1 | IntSeq(1,3).length
    ^^^^^^^^^^^^^^^^^^
at  (repl:text)

Available properties:
end
start
step
"########,
        ),
        (
            r########"missing-isempty"########,
            r########"IntSeq(1,3).isEmpty"########,
            r########"error"########,
            r########"–– Pkl Error ––
Cannot find property `isEmpty` in object of type `IntSeq`.

1 | IntSeq(1,3).isEmpty
    ^^^^^^^^^^^^^^^^^^^
at  (repl:text)

Available properties:
end
start
step
"########,
        ),
        (
            r########"missing-subscript"########,
            r########"IntSeq(1,3)[0]"########,
            r########"error"########,
            r########"–– Pkl Error ––
Operator `[]` is not defined for operand types `IntSeq` and `Int`.
Left operand : IntSeq(1, 3)
Right operand: 0

1 | IntSeq(1,3)[0]
    ^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"missing-plus"########,
            r########"IntSeq(1,3) + IntSeq(4,5)"########,
            r########"error"########,
            r########"–– Pkl Error ––
Operator `+` is not defined for operand types `IntSeq` and `IntSeq`.
Left operand : IntSeq(1, 3)
Right operand: IntSeq(4, 5)

1 | IntSeq(1,3) + IntSeq(4,5)
    ^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"missing-member"########,
            r########"IntSeq(1,3).missingField"########,
            r########"error"########,
            r########"–– Pkl Error ––
Cannot find property `missingField` in object of type `IntSeq`.

1 | IntSeq(1,3).missingField
    ^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)

Available properties:
end
start
step
"########,
        ),
        (
            r########"bytes-basic-all"########,
            r########"IntSeq(0,255).toList().toBytes().sha256"########,
            r########"value"########,
            r########"40aff2e9d2d8922e47afd4648e6967497158785fbd1da870e7110266bf944880"########,
        ),
        (
            r########"bytes-basic-length"########,
            r########"IntSeq(0,255).toList().toBytes().length"########,
            r########"value"########,
            r########"256"########,
        ),
        (
            r########"bytes-basic-high-index"########,
            r########"IntSeq(0,255).toList().toBytes()[255]"########,
            r########"value"########,
            r########"-1"########,
        ),
        (
            r########"bytes-api-mega-length"########,
            r########"IntSeq(0,999).map((_) -> 0).toBytes().length"########,
            r########"value"########,
            r########"1000"########,
        ),
        (
            r########"bytes-api-mega-size"########,
            r########"IntSeq(0,999).map((_) -> 0).toBytes().size"########,
            r########"value"########,
            r########"1.kb"########,
        ),
        (
            r########"bytes-api-mega-hash"########,
            r########"IntSeq(0,999).map((_) -> 0).toBytes().sha256"########,
            r########"value"########,
            r########"541b3e9daa09b20bf85fa273e5cbd3e80185aa4ec298e765db87742b70138a53"########,
        ),
        (
            r########"for-full-extreme-up"########,
            r########"new Listing { for (n in IntSeq(minInt,maxInt)) { n } }"########,
            r########"value"########,
            r########"new Listing { -9223372036854775808; -9223372036854775807 }"########,
        ),
        (
            r########"for-full-extreme-down"########,
            r########"new Listing { for (n in IntSeq(maxInt,minInt).step(-1)) { n } }"########,
            r########"value"########,
            r########"new Listing { 9223372036854775807; 9223372036854775806 }"########,
        ),
        (
            r########"ctor-arity-none"########,
            r########"IntSeq()"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected 2 function arguments but got 0.

2931 | external const function IntSeq(start: Int, end: Int): IntSeq
       ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at pkl.base#IntSeq (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L2931)

1 | IntSeq()
    ^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"ctor-arity-one"########,
            r########"IntSeq(1)"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected 2 function arguments but got 1.

2931 | external const function IntSeq(start: Int, end: Int): IntSeq
       ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at pkl.base#IntSeq (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L2931)

1 | IntSeq(1)
    ^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"ctor-arity-three"########,
            r########"IntSeq(1,2,3)"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected 2 function arguments but got 3.

2931 | external const function IntSeq(start: Int, end: Int): IntSeq
       ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at pkl.base#IntSeq (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L2931)

1 | IntSeq(1,2,3)
    ^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"ctor-string-first"########,
            r########"IntSeq("1",2)"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected value of type `Int`, but got type `String`.
Value: "1"

2931 | external const function IntSeq(start: Int, end: Int): IntSeq
                                             ^^^
at pkl.base#IntSeq (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L2931)

1 | IntSeq("1",2)
    ^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"ctor-string-second"########,
            r########"IntSeq(1,"2")"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected value of type `Int`, but got type `String`.
Value: "2"

2931 | external const function IntSeq(start: Int, end: Int): IntSeq
                                                       ^^^
at pkl.base#IntSeq (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L2931)

1 | IntSeq(1,"2")
    ^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"ctor-null-first"########,
            r########"IntSeq(null,2)"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected value of type `Int`, but got `null`.

2931 | external const function IntSeq(start: Int, end: Int): IntSeq
                                             ^^^
at pkl.base#IntSeq (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L2931)

1 | IntSeq(null,2)
    ^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"ctor-throw-first"########,
            r########"IntSeq(throw("first"),throw("second"))"########,
            r########"error"########,
            r########"–– Pkl Error ––
first

1 | IntSeq(throw("first"),throw("second"))
           ^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"ctor-throw-second"########,
            r########"IntSeq(1,throw("second"))"########,
            r########"error"########,
            r########"–– Pkl Error ––
second

1 | IntSeq(1,throw("second"))
             ^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"step-zero"########,
            r########"IntSeq(1,3).step(0)"########,
            r########"error"########,
            r########"–– Pkl Error ––
Type constraint `isNonZero` violated.
Value: 0

3020 | external function step(newValue: Int(isNonZero)): IntSeq
                                            ^^^^^^^^^
at pkl.base#IntSeq.step (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L3020)

1 | IntSeq(1,3).step(0)
    ^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"step-string"########,
            r########"IntSeq(1,3).step("2")"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected value of type `Int`, but got type `String`.
Value: "2"

3020 | external function step(newValue: Int(isNonZero)): IntSeq
                                        ^^^
at pkl.base#IntSeq.step (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L3020)

1 | IntSeq(1,3).step("2")
    ^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"step-null"########,
            r########"IntSeq(1,3).step(null)"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected value of type `Int`, but got `null`.

3020 | external function step(newValue: Int(isNonZero)): IntSeq
                                        ^^^
at pkl.base#IntSeq.step (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L3020)

1 | IntSeq(1,3).step(null)
    ^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"step-missing"########,
            r########"IntSeq(1,3).step()"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected 1 function arguments but got 0.

3020 | external function step(newValue: Int(isNonZero)): IntSeq
       ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at pkl.base#IntSeq.step (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L3020)

1 | IntSeq(1,3).step()
    ^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"step-extra"########,
            r########"IntSeq(1,3).step(1,2)"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected 1 function arguments but got 2.

3020 | external function step(newValue: Int(isNonZero)): IntSeq
       ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at pkl.base#IntSeq.step (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L3020)

1 | IntSeq(1,3).step(1,2)
    ^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"step-throw"########,
            r########"IntSeq(1,3).step(throw("step"))"########,
            r########"error"########,
            r########"–– Pkl Error ––
step

1 | IntSeq(1,3).step(throw("step"))
                     ^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"map-missing"########,
            r########"IntSeq(1,3).map()"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected 1 function arguments but got 0.

3033 | external function map<Result>(mapper: (Int) -> Result): List<Result>
       ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at pkl.base#IntSeq.map (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L3033)

1 | IntSeq(1,3).map()
    ^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"map-wrong-type"########,
            r########"IntSeq(1,3).map(1)"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected value of type `Function1`, but got type `Int`.
Value: 1

3033 | external function map<Result>(mapper: (Int) -> Result): List<Result>
                                             ^^^^^^^^^^^^^^^
at pkl.base#IntSeq.map (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L3033)

1 | IntSeq(1,3).map(1)
    ^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"map-wrong-arity"########,
            r########"IntSeq(1,3).map((a,b) -> a+b)"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected value of type `Function1`, but got type `Function2`.
Value: new Function2 {}

3033 | external function map<Result>(mapper: (Int) -> Result): List<Result>
                                             ^^^^^^^^^^^^^^^
at pkl.base#IntSeq.map (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L3033)

1 | IntSeq(1,3).map((a,b) -> a+b)
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"map-empty-wrong-arity"########,
            r########"IntSeq(3,1).map((a,b) -> a+b)"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected value of type `Function1`, but got type `Function2`.
Value: new Function2 {}

3033 | external function map<Result>(mapper: (Int) -> Result): List<Result>
                                             ^^^^^^^^^^^^^^^
at pkl.base#IntSeq.map (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L3033)

1 | IntSeq(3,1).map((a,b) -> a+b)
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"map-callback-throw"########,
            r########"IntSeq(1,3).map((n) -> throw(n.toString()))"########,
            r########"error"########,
            r########"–– Pkl Error ––
1

1 | IntSeq(1,3).map((n) -> throw(n.toString()))
                           ^^^^^^^^^^^^^^^^^^^
at repl:text.<function#1> (repl:text)

1 | IntSeq(1,3).map((n) -> throw(n.toString()))
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"fold-missing"########,
            r########"IntSeq(1,3).fold(0)"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected 2 function arguments but got 1.

3023 | external function fold<Result>(initial: Result, operator: (Result, Int) -> Result): Result
       ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at pkl.base#IntSeq.fold (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L3023)

1 | IntSeq(1,3).fold(0)
    ^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"fold-wrong-type"########,
            r########"IntSeq(1,3).fold(0,1)"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected value of type `Function2`, but got type `Int`.
Value: 1

3023 | external function fold<Result>(initial: Result, operator: (Result, Int) -> Result): Result
                                                                 ^^^^^^^^^^^^^^^^^^^^^^^
at pkl.base#IntSeq.fold (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L3023)

1 | IntSeq(1,3).fold(0,1)
    ^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"fold-wrong-arity"########,
            r########"IntSeq(1,3).fold(0,(a)->a)"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected value of type `Function2`, but got type `Function1`.
Value: new Function1 {}

3023 | external function fold<Result>(initial: Result, operator: (Result, Int) -> Result): Result
                                                                 ^^^^^^^^^^^^^^^^^^^^^^^
at pkl.base#IntSeq.fold (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L3023)

1 | IntSeq(1,3).fold(0,(a)->a)
    ^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"fold-empty-wrong-arity"########,
            r########"IntSeq(3,1).fold(0,(a)->a)"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected value of type `Function2`, but got type `Function1`.
Value: new Function1 {}

3023 | external function fold<Result>(initial: Result, operator: (Result, Int) -> Result): Result
                                                                 ^^^^^^^^^^^^^^^^^^^^^^^
at pkl.base#IntSeq.fold (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L3023)

1 | IntSeq(3,1).fold(0,(a)->a)
    ^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"fold-empty-initial"########,
            r########"IntSeq(3,1).fold("seed",(a,n)->a+n.toString())"########,
            r########"value"########,
            r########"seed"########,
        ),
        (
            r########"fold-callback-throw"########,
            r########"IntSeq(1,3).fold(0,(a,n)->throw(n.toString()))"########,
            r########"error"########,
            r########"–– Pkl Error ––
1

1 | IntSeq(1,3).fold(0,(a,n)->throw(n.toString()))
                              ^^^^^^^^^^^^^^^^^^^
at repl:text.<function#1> (repl:text)

1 | IntSeq(1,3).fold(0,(a,n)->throw(n.toString()))
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"tolist-extra"########,
            r########"IntSeq(1,3).toList(1)"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected 0 function arguments but got 1.

3036 | external function toList(): List<Int>
       ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at pkl.base#IntSeq.toList (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L3036)

1 | IntSeq(1,3).toList(1)
    ^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"tolisting-extra"########,
            r########"IntSeq(1,3).toListing(1)"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected 0 function arguments but got 1.

3039 | external function toListing(): Listing<Int>
       ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at pkl.base#IntSeq.toListing (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L3039)

1 | IntSeq(1,3).toListing(1)
    ^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"new-intseq"########,
            r########"new IntSeq {}"########,
            r########"error"########,
            r########"–– Pkl Error ––
Cannot instantiate, or amend an instance of, external class `IntSeq`.

1 | new IntSeq {}
        ^^^^^^
at  (repl:text)

To create an instance of an external class, use its literal syntax or constructor method.
Examples:
* `123` uses literal syntax to create an instance of class `Int`.
* `Pair(1, 2)` uses the `Pair()` constructor method to create an instance of class `Pair`.
"########,
        ),
        (
            r########"extreme-up-listing"########,
            r########"IntSeq(minInt,maxInt).step(maxInt).toListing()"########,
            r########"value"########,
            r########"new Listing { -9223372036854775808; -1; 9223372036854775806 }"########,
        ),
        (
            r########"extreme-down-listing"########,
            r########"IntSeq(maxInt,minInt).step(minInt).toListing()"########,
            r########"value"########,
            r########"new Listing { 9223372036854775807; -1 }"########,
        ),
        (
            r########"render-direct-asc"########,
            r########"IntSeq(1,4)"########,
            r########"value"########,
            r########"IntSeq(1, 4)"########,
        ),
        (
            r########"render-direct-empty"########,
            r########"IntSeq(5,2)"########,
            r########"value"########,
            r########"IntSeq(5, 2)"########,
        ),
        (
            r########"render-direct-desc"########,
            r########"IntSeq(5,1).step(-1)"########,
            r########"value"########,
            r########"IntSeq(5, 1).step(-1)"########,
        ),
        (
            r########"render-direct-extreme"########,
            r########"IntSeq(minInt,maxInt).step(maxInt)"########,
            r########"value"########,
            r########"IntSeq(-9223372036854775808, 9223372036854775807).step(9223372036854775807)"########,
        ),
        (
            r########"explicit-unit-step-string"########,
            r########"IntSeq(1,3).step(1).toString()"########,
            r########"value"########,
            r########"IntSeq(1, 3)"########,
        ),
        (
            r########"chained-step-string"########,
            r########"IntSeq(1,3).step(2).step(-1).toString()"########,
            r########"value"########,
            r########"IntSeq(1, 3).step(-1)"########,
        ),
    ];
    assert_eq!(rows.len(), 237);
    for (id, expression, kind, expected) in rows {
        let event = query(&mut probe, expression);
        let actual = if event.kind == "value" {
            event.value.clone()
        } else {
            event.error.clone()
        };
        let wanted = expected.encode_utf16().collect::<Vec<_>>();
        if event.kind != kind || actual.as_ref() != Some(&wanted) {
            mismatches.push(format!(
                "{id}: {expression}\nexpected {kind}: {expected}\nactual {}: {}",
                event.kind,
                actual
                    .as_ref()
                    .map(|v| String::from_utf16_lossy(v))
                    .unwrap_or_default()
            ));
        }
    }
    assert!(
        mismatches.is_empty(),
        "{} Java mismatches:\n{}",
        mismatches.len(),
        mismatches.join("\n\n")
    );
}
