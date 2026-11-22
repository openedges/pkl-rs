//! DataSize regression observations from the independently acquired Java 0.32.0
//! split string/export oracle03. The permanent authority is tests/fixtures/m2-data-size.
//! These selected assertions do not replace the ordered public API comparator.
#[path = "support/module_keys.rs"]
mod module_keys;

use pkl_runtime::{
    data_size::{DataSize, DataSizeUnit},
    evaluation::frontend::{ModuleEvalOptions, ModuleSourceInput, evaluate_module_output},
    parser::parse_module,
    resolver::resolve_module,
    value::{RuntimeError, Value},
    vm::{Vm, VmPoll, compile_module},
};

const SOURCE: &str = "// Stage57 DataSize public API source. All expressions are supplied separately.\nnanSize = (0.0 / 0.0).mb\ntypealias DataSizeAlias = DataSize\nclass UnitHolder { x: DataSizeUnit = \"mb\" }\nclass SizeHolder { x: DataSize = 1.mb }\nclass RequiredSize { x: DataSize }\nfunction useDataSize(x: DataSize): String = x.unit\nmarker = \"stage57\"\n";

fn output(expression: &str) -> Vec<u8> {
    output_with_source(SOURCE, expression)
}

fn output_with_source(input: &str, expression: &str) -> Vec<u8> {
    let source = format!("{input}\noutput {{ text = ({expression}).toString() }}");
    let result = evaluate_module_output(
        ModuleSourceInput {
            text: &source,
            uri: "file:///stage57/datasize-oracle.pkl",
            inferred_name: "datasize-oracle",
        },
        &ModuleEvalOptions::default(),
    );
    assert_eq!(
        result.exit_code,
        0,
        "{expression}: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    result.stdout
}

fn event(vm: &mut Vm) -> VmPoll {
    for _ in 0..100_000 {
        vm.collect(&[]);
        let event = vm.poll(1);
        let roots = match &event {
            VmPoll::Ready(Ok(value)) => vec![value.clone()],
            _ => vec![],
        };
        vm.collect(&roots);
        if !matches!(event, VmPoll::Yielded) {
            return event;
        }
    }
    panic!("DataSize poll bound")
}

fn value(expression: &str) -> Value {
    result(expression).unwrap()
}

fn result(expression: &str) -> Result<Value, RuntimeError> {
    let source = format!("{SOURCE}\nresult = {expression}");
    let program =
        compile_module(&resolve_module(&parse_module(&source).unwrap()).unwrap()).unwrap();
    let mut vm = Vm::new(program.clone());
    vm.start(program.entry, vec![]).unwrap();
    let VmPoll::Ready(Ok(module)) = event(&mut vm) else {
        panic!("module")
    };
    vm.start_member(module.as_object().unwrap(), "result")
        .unwrap();
    let VmPoll::Ready(result) = event(&mut vm) else {
        panic!("result")
    };
    vm.close();
    assert_eq!(vm.heap_stats().slots, 0);
    result
}

#[test]
fn units_members_arithmetic_ordering_and_types_match_java_observations() {
    // Entries retain the acquisition's row IDs. Expected bytes come from its
    // string/repeat-0/result.stdout.bin; the second Java process matched exactly.
    for (id, expression, expected) in [
        ("int-b", "7.b", "7.b"),
        ("to-data-size-b", "7.toDataSize(\"b\")", "7.b"),
        ("value-b", "7.25.b.value", "7.25"),
        ("unit-b", "7.25.b.unit", "b"),
        ("binary-b", "1.b.isBinaryUnit", "true"),
        ("decimal-b", "1.b.isDecimalUnit", "true"),
        ("unit-b-to-b", "1.b.toUnit(\"b\")", "1.b"),
        ("b-to-unit-b", "1.b.toUnit(\"b\")", "1.b"),
        ("b-to-binary", "1.b.toBinaryUnit()", "1.b"),
        ("b-to-decimal", "1.b.toDecimalUnit()", "1.b"),
        ("int-kb", "7.kb", "7.kb"),
        ("to-data-size-kb", "7.toDataSize(\"kb\")", "7.kb"),
        ("value-kb", "7.25.kb.value", "7.25"),
        ("unit-kb", "7.25.kb.unit", "kb"),
        ("binary-kb", "1.kb.isBinaryUnit", "false"),
        ("decimal-kb", "1.kb.isDecimalUnit", "true"),
        ("unit-kb-to-b", "1.kb.toUnit(\"b\")", "1000.b"),
        ("b-to-unit-kb", "1.b.toUnit(\"kb\")", "0.001.kb"),
        ("kb-to-binary", "1.kb.toBinaryUnit()", "0.9765625.kib"),
        ("kb-to-decimal", "1.kb.toDecimalUnit()", "1.kb"),
        ("int-kib", "7.kib", "7.kib"),
        ("to-data-size-kib", "7.toDataSize(\"kib\")", "7.kib"),
        ("value-kib", "7.25.kib.value", "7.25"),
        ("unit-kib", "7.25.kib.unit", "kib"),
        ("binary-kib", "1.kib.isBinaryUnit", "true"),
        ("decimal-kib", "1.kib.isDecimalUnit", "false"),
        ("unit-kib-to-b", "1.kib.toUnit(\"b\")", "1024.b"),
        ("b-to-unit-kib", "1.b.toUnit(\"kib\")", "9.765625E-4.kib"),
        ("kib-to-binary", "1.kib.toBinaryUnit()", "1.kib"),
        ("kib-to-decimal", "1.kib.toDecimalUnit()", "1.024.kb"),
        ("int-mb", "7.mb", "7.mb"),
        ("to-data-size-mb", "7.toDataSize(\"mb\")", "7.mb"),
        ("value-mb", "7.25.mb.value", "7.25"),
        ("unit-mb", "7.25.mb.unit", "mb"),
        ("binary-mb", "1.mb.isBinaryUnit", "false"),
        ("decimal-mb", "1.mb.isDecimalUnit", "true"),
        ("unit-mb-to-b", "1.mb.toUnit(\"b\")", "1000000.b"),
        ("b-to-unit-mb", "1.b.toUnit(\"mb\")", "1.0E-6.mb"),
        (
            "mb-to-binary",
            "1.mb.toBinaryUnit()",
            "0.95367431640625.mib",
        ),
        ("mb-to-decimal", "1.mb.toDecimalUnit()", "1.mb"),
        ("int-mib", "7.mib", "7.mib"),
        ("to-data-size-mib", "7.toDataSize(\"mib\")", "7.mib"),
        ("value-mib", "7.25.mib.value", "7.25"),
        ("unit-mib", "7.25.mib.unit", "mib"),
        ("binary-mib", "1.mib.isBinaryUnit", "true"),
        ("decimal-mib", "1.mib.isDecimalUnit", "false"),
        ("unit-mib-to-b", "1.mib.toUnit(\"b\")", "1048576.b"),
        (
            "b-to-unit-mib",
            "1.b.toUnit(\"mib\")",
            "9.5367431640625E-7.mib",
        ),
        ("mib-to-binary", "1.mib.toBinaryUnit()", "1.mib"),
        ("mib-to-decimal", "1.mib.toDecimalUnit()", "1.048576.mb"),
        ("int-gb", "7.gb", "7.gb"),
        ("to-data-size-gb", "7.toDataSize(\"gb\")", "7.gb"),
        ("value-gb", "7.25.gb.value", "7.25"),
        ("unit-gb", "7.25.gb.unit", "gb"),
        ("binary-gb", "1.gb.isBinaryUnit", "false"),
        ("decimal-gb", "1.gb.isDecimalUnit", "true"),
        ("unit-gb-to-b", "1.gb.toUnit(\"b\")", "1000000000.b"),
        ("b-to-unit-gb", "1.b.toUnit(\"gb\")", "1.0E-9.gb"),
        (
            "gb-to-binary",
            "1.gb.toBinaryUnit()",
            "0.9313225746154785.gib",
        ),
        ("gb-to-decimal", "1.gb.toDecimalUnit()", "1.gb"),
        ("int-gib", "7.gib", "7.gib"),
        ("to-data-size-gib", "7.toDataSize(\"gib\")", "7.gib"),
        ("value-gib", "7.25.gib.value", "7.25"),
        ("unit-gib", "7.25.gib.unit", "gib"),
        ("binary-gib", "1.gib.isBinaryUnit", "true"),
        ("decimal-gib", "1.gib.isDecimalUnit", "false"),
        ("unit-gib-to-b", "1.gib.toUnit(\"b\")", "1073741824.b"),
        (
            "b-to-unit-gib",
            "1.b.toUnit(\"gib\")",
            "9.313225746154785E-10.gib",
        ),
        ("gib-to-binary", "1.gib.toBinaryUnit()", "1.gib"),
        ("gib-to-decimal", "1.gib.toDecimalUnit()", "1.073741824.gb"),
        ("int-tb", "7.tb", "7.tb"),
        ("to-data-size-tb", "7.toDataSize(\"tb\")", "7.tb"),
        ("value-tb", "7.25.tb.value", "7.25"),
        ("unit-tb", "7.25.tb.unit", "tb"),
        ("binary-tb", "1.tb.isBinaryUnit", "false"),
        ("decimal-tb", "1.tb.isDecimalUnit", "true"),
        ("unit-tb-to-b", "1.tb.toUnit(\"b\")", "1000000000000.b"),
        ("b-to-unit-tb", "1.b.toUnit(\"tb\")", "1.0E-12.tb"),
        (
            "tb-to-binary",
            "1.tb.toBinaryUnit()",
            "0.9094947017729282.tib",
        ),
        ("tb-to-decimal", "1.tb.toDecimalUnit()", "1.tb"),
        ("int-tib", "7.tib", "7.tib"),
        ("to-data-size-tib", "7.toDataSize(\"tib\")", "7.tib"),
        ("value-tib", "7.25.tib.value", "7.25"),
        ("unit-tib", "7.25.tib.unit", "tib"),
        ("binary-tib", "1.tib.isBinaryUnit", "true"),
        ("decimal-tib", "1.tib.isDecimalUnit", "false"),
        ("unit-tib-to-b", "1.tib.toUnit(\"b\")", "1099511627776.b"),
        (
            "b-to-unit-tib",
            "1.b.toUnit(\"tib\")",
            "9.094947017729282E-13.tib",
        ),
        ("tib-to-binary", "1.tib.toBinaryUnit()", "1.tib"),
        (
            "tib-to-decimal",
            "1.tib.toDecimalUnit()",
            "1.099511627776.tb",
        ),
        ("int-pb", "7.pb", "7.pb"),
        ("to-data-size-pb", "7.toDataSize(\"pb\")", "7.pb"),
        ("value-pb", "7.25.pb.value", "7.25"),
        ("unit-pb", "7.25.pb.unit", "pb"),
        ("binary-pb", "1.pb.isBinaryUnit", "false"),
        ("decimal-pb", "1.pb.isDecimalUnit", "true"),
        ("unit-pb-to-b", "1.pb.toUnit(\"b\")", "1000000000000000.b"),
        ("b-to-unit-pb", "1.b.toUnit(\"pb\")", "1.0E-15.pb"),
        (
            "pb-to-binary",
            "1.pb.toBinaryUnit()",
            "0.8881784197001252.pib",
        ),
        ("pb-to-decimal", "1.pb.toDecimalUnit()", "1.pb"),
        ("int-pib", "7.pib", "7.pib"),
        ("to-data-size-pib", "7.toDataSize(\"pib\")", "7.pib"),
        ("value-pib", "7.25.pib.value", "7.25"),
        ("unit-pib", "7.25.pib.unit", "pib"),
        ("binary-pib", "1.pib.isBinaryUnit", "true"),
        ("decimal-pib", "1.pib.isDecimalUnit", "false"),
        ("unit-pib-to-b", "1.pib.toUnit(\"b\")", "1125899906842624.b"),
        (
            "b-to-unit-pib",
            "1.b.toUnit(\"pib\")",
            "8.881784197001252E-16.pib",
        ),
        ("pib-to-binary", "1.pib.toBinaryUnit()", "1.pib"),
        (
            "pib-to-decimal",
            "1.pib.toDecimalUnit()",
            "1.125899906842624.pb",
        ),
        ("large-int", "9007199254740993.b", "9007199254740992.b"),
        ("max-int", "9223372036854775807.b", "9223372036854775807.b"),
        (
            "min-int",
            "(-9223372036854775808).b",
            "-9223372036854775808.b",
        ),
        ("positive-zero", "0.0.mb", "0.mb"),
        ("negative-zero", "(-0.0).mb", "0.mb"),
        ("positive-infinity", "(1.0 / 0.0).mb", "Infinity.mb"),
        ("negative-infinity", "(-1.0 / 0.0).mb", "-Infinity.mb"),
        ("nan", "(0.0 / 0.0).mb", "NaN.mb"),
        ("subnormal", "(1.0e-320).pib", "1.0E-320.pib"),
        ("max-double", "(1.0e308).pib", "9223372036854775807.pib"),
        ("very-small", "(1.0e-300).pib", "1.0E-300.pib"),
        ("integer-value", "4.0.mb.value", "4"),
        ("negative-zero-value", "(-0.0).mb.value", "0"),
        ("large-value", "(1.0e100).mb.value", "9223372036854775807"),
        ("nan-value", "(0.0/0.0).mb.value", "NaN"),
        (
            "mixed-decimal-binary",
            "1.mib.toUnit(\"mb\")",
            "1.048576.mb",
        ),
        ("roundtrip", "1.pib.toUnit(\"b\").toUnit(\"pib\")", "1.pib"),
        ("positive-zero-predicate", "0.0.mb.isPositive", "true"),
        ("negative-zero-predicate", "(-0.0).mb.isPositive", "true"),
        ("nan-predicate", "(0.0/0.0).mb.isPositive", "false"),
        (
            "is-between-inclusive",
            "3.kb.isBetween(2000.b,3000.b)",
            "true",
        ),
        ("is-between-reversed", "3.kb.isBetween(4.kb,2.kb)", "false"),
        ("is-between-mixed", "1.mib.isBetween(1000.kb,2.mb)", "true"),
        ("add", "1.mb + 1.kb", "1.001.mb"),
        ("add-reverse", "1.kb + 1.mb", "1.001.mb"),
        ("add-mixed", "1.mib + 1.mb", "1.95367431640625.mib"),
        ("add-mixed-reverse", "1.mb + 1.mib", "1.95367431640625.mib"),
        ("subtract", "1.mb - 1000.kb", "0.mb"),
        ("subtract-reverse", "1000.kb - 1.mb", "0.mb"),
        ("multiply-int", "2.mb * 3", "6.mb"),
        ("multiply-int-reverse", "3 * 2.mb", "6.mb"),
        ("multiply-float", "2.mb * 1.25", "2.5.mb"),
        ("multiply-float-reverse", "1.25 * 2.mb", "2.5.mb"),
        ("divide-scalar", "5.mb / 2", "2.5.mb"),
        ("divide-size", "1.mib / 1.kb", "1048.576"),
        ("divide-size-reverse", "1.kb / 1.mib", "9.5367431640625E-4"),
        ("int-divide-scalar", "5.mb ~/ 2", "2.mb"),
        ("int-divide-size", "5.mb ~/ 2.mb", "2"),
        ("int-divide-mixed", "1.mib ~/ 1.kb", "1048"),
        ("remainder", "5.mb % 2", "1.mb"),
        ("power-int", "2.mb ** 3", "8.mb"),
        ("power-float", "2.mb ** 0.5", "1.4142135623730951.mb"),
        ("unary-minus", "-2.mb", "-2.mb"),
        ("less-mixed", "1.mb < 1.mib", "true"),
        ("less-reverse", "1.mib < 1.mb", "false"),
        ("le-equal", "1.mb <= 1000.kb", "true"),
        ("greater", "2.kb > 1999.b", "true"),
        ("ge", "1.mib >= 1.mb", "true"),
        ("nan-less", "(0.0/0.0).mb < 1.mb", "false"),
        ("nan-greater", "(0.0/0.0).mb > 1.mb", "true"),
        ("zero-order", "(-0.0).mb < 0.0.mb", "true"),
        ("pow-neg-zero", "(-0.0).mb ** 3", "0.mb"),
        ("pow-negative-exponent", "2.mb ** -1", "0.5.mb"),
        ("pow-nan-zero", "(0.0/0.0).mb ** 0", "1.mb"),
        ("pow-negative-fraction", "(-2).mb ** 0.5", "NaN.mb"),
        ("pow-overflow", "2.mb ** 1024", "Infinity.mb"),
        ("trunc-negative", "(-1).mb ~/ 2", "0.mb"),
        ("trunc-mixed-reverse", "1.kb ~/ 1.mib", "0"),
        (
            "fraction-kib-kb",
            "1.2345678901234567.kib.toUnit(\"kb\")",
            "1.2641975194864197.kb",
        ),
        (
            "fraction-kb-kib",
            "1.2345678901234567.kb.toUnit(\"kib\")",
            "1.2056327051986881.kib",
        ),
        ("large-pib-b", "(1.0e308).pib.toUnit(\"b\")", "Infinity.b"),
        (
            "small-pib-b",
            "(1.0e-320).pib.toUnit(\"b\")",
            "1.1258873724046439E-305.b",
        ),
        (
            "large-int-kb-b",
            "9007199254740993.kb.toUnit(\"b\")",
            "9007199254740992000.b",
        ),
        ("negative-zero-kb-b", "(-0.0).kb.toUnit(\"b\")", "0.b"),
        ("nan-mb-kb", "(0.0/0.0).mb.toUnit(\"kb\")", "NaN.kb"),
        ("same-unit", "1.mb == 1.mb", "true"),
        ("cross-unit", "1.mb == 1000.kb", "true"),
        ("cross-unit-reverse", "1000.kb == 1.mb", "true"),
        ("different", "1.mb != 2.mb", "true"),
        ("nan-self", "nanSize == nanSize", "true"),
        ("nan-distinct", "(0.0/0.0).mb == (0.0/0.0).mb", "false"),
        ("zero-equality", "(-0.0).mb == 0.0.mb", "true"),
        ("set-equal", "Set(1.mb,1000.kb).length", "1"),
        ("set-different", "Set(1.mb,1.mib).length", "2"),
        ("set-nan-self", "Set(nanSize,nanSize).length", "1"),
        (
            "set-nan-distinct",
            "Set((0.0/0.0).mb,(0.0/0.0).mb).length",
            "2",
        ),
        ("set-zero", "Set((-0.0).mb,0.0.mb).length", "1"),
        (
            "map-equal",
            "Map(1.mb,\"first\",1000.kb,\"second\").length",
            "1",
        ),
        ("map-nan-self", "Map(nanSize,1,nanSize,2).length", "1"),
        (
            "list-equality",
            "List(1.mb,2.kb) == List(1000.kb,2.kb)",
            "true",
        ),
        (
            "typed-object-equality",
            "new Dynamic { x = 1.mb } == new Dynamic { x = 1000.kb }",
            "true",
        ),
        ("class", "1.mb.getClass().simpleName", "DataSize"),
        ("is-datasize", "1.mb is DataSize", "true"),
        ("is-any", "1.mb is Any", "true"),
        ("is-number", "1.mb is Number", "false"),
        ("is-typed", "1.mb is Typed", "false"),
        ("as-datasize", "1.mb as DataSize", "1.mb"),
        ("nullable", "1.mb is DataSize?", "true"),
        ("union", "1.mb is DataSize|String", "true"),
        ("unit-valid", "new UnitHolder {}.x", "mb"),
        ("typed-property", "new SizeHolder {}.x", "1.mb"),
        ("typed-method", "useDataSize(1.mb)", "mb"),
        ("alias", "(1.mb is DataSizeAlias)", "true"),
    ] {
        assert_eq!(
            output(expression),
            expected.as_bytes(),
            "Java row {id}: {expression}"
        );
    }
}

#[test]
fn payload_bits_preserve_java_widening_conversion_order_and_strict_power() {
    // Raw bits are copied from export/repeat-0/call-0.raw-bits-hex.bin.
    for (id, expression, bits, unit) in [
        (
            "large-int",
            "9007199254740993.b",
            0x4340000000000000u64,
            "b",
        ),
        (
            "max-int",
            "9223372036854775807.b",
            0x43e0000000000000u64,
            "b",
        ),
        (
            "min-int",
            "(-9223372036854775808).b",
            0xc3e0000000000000u64,
            "b",
        ),
        ("positive-zero", "0.0.mb", 0x0000000000000000u64, "mb"),
        ("negative-zero", "(-0.0).mb", 0x8000000000000000u64, "mb"),
        (
            "positive-infinity",
            "(1.0 / 0.0).mb",
            0x7ff0000000000000u64,
            "mb",
        ),
        (
            "negative-infinity",
            "(-1.0 / 0.0).mb",
            0xfff0000000000000u64,
            "mb",
        ),
        ("nan", "(0.0 / 0.0).mb", 0x7ff8000000000000u64, "mb"),
        ("subnormal", "(1.0e-320).pib", 0x00000000000007e8u64, "pib"),
        ("max-double", "(1.0e308).pib", 0x7fe1ccf385ebc8a0u64, "pib"),
        ("very-small", "(1.0e-300).pib", 0x01a56e1fc2f8f359u64, "pib"),
        (
            "mixed-decimal-binary",
            "1.mib.toUnit(\"mb\")",
            0x3ff0c6f7a0b5ed8du64,
            "mb",
        ),
        (
            "roundtrip",
            "1.pib.toUnit(\"b\").toUnit(\"pib\")",
            0x3ff0000000000000u64,
            "pib",
        ),
        ("add", "1.mb + 1.kb", 0x3ff004189374bc6au64, "mb"),
        ("add-reverse", "1.kb + 1.mb", 0x3ff004189374bc6au64, "mb"),
        ("add-mixed", "1.mib + 1.mb", 0x3fff424000000000u64, "mib"),
        (
            "add-mixed-reverse",
            "1.mb + 1.mib",
            0x3fff424000000000u64,
            "mib",
        ),
        ("subtract", "1.mb - 1000.kb", 0x0000000000000000u64, "mb"),
        (
            "subtract-reverse",
            "1000.kb - 1.mb",
            0x0000000000000000u64,
            "mb",
        ),
        ("multiply-int", "2.mb * 3", 0x4018000000000000u64, "mb"),
        (
            "multiply-int-reverse",
            "3 * 2.mb",
            0x4018000000000000u64,
            "mb",
        ),
        ("multiply-float", "2.mb * 1.25", 0x4004000000000000u64, "mb"),
        (
            "multiply-float-reverse",
            "1.25 * 2.mb",
            0x4004000000000000u64,
            "mb",
        ),
        ("divide-scalar", "5.mb / 2", 0x4004000000000000u64, "mb"),
        (
            "int-divide-scalar",
            "5.mb ~/ 2",
            0x4000000000000000u64,
            "mb",
        ),
        ("remainder", "5.mb % 2", 0x3ff0000000000000u64, "mb"),
        ("power-int", "2.mb ** 3", 0x4020000000000000u64, "mb"),
        ("power-float", "2.mb ** 0.5", 0x3ff6a09e667f3bcdu64, "mb"),
        ("unary-minus", "-2.mb", 0xc000000000000000u64, "mb"),
        (
            "pow-neg-zero",
            "(-0.0).mb ** 3",
            0x8000000000000000u64,
            "mb",
        ),
        (
            "pow-negative-exponent",
            "2.mb ** -1",
            0x3fe0000000000000u64,
            "mb",
        ),
        (
            "pow-nan-zero",
            "(0.0/0.0).mb ** 0",
            0x3ff0000000000000u64,
            "mb",
        ),
        (
            "pow-negative-fraction",
            "(-2).mb ** 0.5",
            0x7ff8000000000000u64,
            "mb",
        ),
        ("pow-overflow", "2.mb ** 1024", 0x7ff0000000000000u64, "mb"),
        (
            "trunc-negative",
            "(-1).mb ~/ 2",
            0x0000000000000000u64,
            "mb",
        ),
        (
            "fraction-kib-kb",
            "1.2345678901234567.kib.toUnit(\"kb\")",
            0x3ff43a272d9e0e51u64,
            "kb",
        ),
        (
            "fraction-kb-kib",
            "1.2345678901234567.kb.toUnit(\"kib\")",
            0x3ff34a4584fd0fdfu64,
            "kib",
        ),
        (
            "large-pib-b",
            "(1.0e308).pib.toUnit(\"b\")",
            0x7ff0000000000000u64,
            "b",
        ),
        (
            "small-pib-b",
            "(1.0e-320).pib.toUnit(\"b\")",
            0x009fa00000000000u64,
            "b",
        ),
        (
            "large-int-kb-b",
            "9007199254740993.kb.toUnit(\"b\")",
            0x43df400000000000u64,
            "b",
        ),
        (
            "negative-zero-kb-b",
            "(-0.0).kb.toUnit(\"b\")",
            0x8000000000000000u64,
            "b",
        ),
        (
            "nan-mb-kb",
            "(0.0/0.0).mb.toUnit(\"kb\")",
            0x7ff8000000000000u64,
            "kb",
        ),
    ] {
        let Value::DataSize(value) = value(expression) else {
            panic!("Java row {id}: DataSize")
        };
        let actual_bits = if value.value().is_nan() {
            // Java Double.doubleToLongBits canonicalizes architecture-dependent NaN sign bits
            // (x86_64 SSE `0.0 / 0.0` produces 0xfff8_0000_0000_0000, ARM64 produces 0x7ff8_0000_0000_0000).
            0x7ff8_0000_0000_0000u64
        } else {
            value.value().to_bits()
        };
        assert_eq!(actual_bits, bits, "Java row {id}: {expression}");
        assert_eq!(value.unit().symbol(), unit, "Java row {id}");
    }
}

#[test]
fn callback_values_survive_suspension_collection_and_close() {
    let source = r#"result = List(1,2).map((x)->read("size:\(x)")).map((x:DataSize)->x.toUnit("b").unit).join("|")"#;
    let program = compile_module(&resolve_module(&parse_module(source).unwrap()).unwrap()).unwrap();
    let mut vm = Vm::new(program.clone());
    vm.start(program.entry, vec![]).unwrap();
    let VmPoll::Ready(Ok(module)) = event(&mut vm) else {
        panic!("module")
    };
    vm.start_member(module.as_object().unwrap(), "result")
        .unwrap();
    for (index, unit) in [(1, DataSizeUnit::Kilobytes), (2, DataSizeUnit::Mebibytes)] {
        let VmPoll::Pending(request) = event(&mut vm) else {
            panic!("read")
        };
        assert_eq!(request.uri, format!("size:{index}"));
        vm.resume(request.id, Ok(Value::DataSize(DataSize::new(1.5, unit))))
            .unwrap();
    }
    assert_eq!(event(&mut vm), VmPoll::Ready(Ok(Value::string("b|b"))));
    vm.close();
    assert_eq!(vm.heap_stats().slots, 0);
}

#[test]
fn java_hash_buckets_keep_signed_zero_separate_after_the_small_map_boundary() {
    // Additive oracle02/hash-threshold, independently acquired twice. This
    // cohort is distinct from the canonical oracle03 ordered API session.
    for (id, expression, expected) in [
        ("set-zero-small", "Set((-0.0).b,0.0.b).length", "1"),
        (
            "set-zero-fifth",
            "Set(1.b,2.b,3.b,(-0.0).b,0.0.b).length",
            "5",
        ),
        (
            "set-zero-sixth",
            "Set(1.b,2.b,3.b,4.b,(-0.0).b,0.0.b).length",
            "6",
        ),
        (
            "set-zero-sixth-reverse",
            "Set(1.b,2.b,3.b,4.b,0.0.b,(-0.0).b).length",
            "6",
        ),
        (
            "set-nan-same-fifth",
            "Set(1.b,2.b,3.b,4.b,nanSize,nanSize).length",
            "5",
        ),
        (
            "set-nan-distinct-fifth",
            "Set(1.b,2.b,3.b,4.b,(0.0/0.0).mb,(0.0/0.0).mb).length",
            "6",
        ),
        (
            "set-cross-unit",
            "Set(1.mb,1000.kb,2.mb,2000.kb,3.mb).length",
            "3",
        ),
        (
            "map-zero-sixth",
            "Map(1.b,1,2.b,2,3.b,3,4.b,4,(-0.0).b,5,0.0.b,6).length",
            "6",
        ),
        (
            "map-zero-sixth-reverse",
            "Map(1.b,1,2.b,2,3.b,3,4.b,4,0.0.b,5,(-0.0).b,6).length",
            "6",
        ),
        (
            "map-nan-same-sixth",
            "Map(1.b,1,2.b,2,3.b,3,4.b,4,nanSize,5,nanSize,6).length",
            "5",
        ),
        (
            "map-nan-distinct-sixth",
            "Map(1.b,1,2.b,2,3.b,3,4.b,4,(0.0/0.0).mb,5,(0.0/0.0).mb,6).length",
            "6",
        ),
        (
            "map-cross-unit",
            "Map(1.mb,1,1000.kb,2,2.mb,3,2000.kb,4,3.mb,5).length",
            "3",
        ),
    ] {
        assert_eq!(output(expression), expected.as_bytes(), "Java row {id}");
    }
}

#[test]
fn data_size_has_no_implicit_default() {
    let error = result("new RequiredSize {}.x").unwrap_err();
    assert_eq!(
        error.message,
        "Tried to read property `x` but its value is undefined."
    );
}

#[test]
fn byte_factory_size_alias_values_and_inherited_callbacks_match_additive_java() {
    let source = "// Stage57 DataSize public API source. All expressions are supplied separately.\nnanSize = (0.0 / 0.0).mb\ntypealias DataSizeAlias = DataSize\nclass UnitHolder { x: DataSizeUnit = \"mb\" }\nclass SizeHolder { x: DataSize = 1.mb }\nclass RequiredSize { x: DataSize }\nfunction useDataSize(x: DataSize): String = x.unit\nmarker = \"stage57\"\nclass UnitNullable { x: DataSizeUnit? }\nclass UnitUnion { x: DataSizeUnit|Int }\nclass UnitRequired { x: DataSizeUnit }\nbyteValue = Bytes(1,2,3)\n";
    for (id, expression, expected) in [
        ("ifnonnull-unit", "1.mb.ifNonNull((it) -> it.unit)", "mb"),
        ("ifnonnull-value", "1.mb.ifNonNull((it) -> it.value)", "1"),
        ("alias-kind", "DataSizeUnit is TypeAlias", "true"),
        ("alias-string", "DataSizeUnit.toString()", "DataSizeUnit"),
        ("alias-valid", "\"mb\" is DataSizeUnit", "true"),
        ("alias-invalid", "\"custom\" is DataSizeUnit", "false"),
        ("nullable-null", "new UnitNullable { x = null }.x", "null"),
        ("union-int", "new UnitUnion { x = 1 }.x", "1"),
        ("unit-holder-default", "new UnitHolder {}.x", "mb"),
        ("bytes-empty", "Bytes()", "Bytes()"),
        ("bytes-zero", "Bytes(0)", "Bytes(0)"),
        ("bytes-max", "Bytes(255)", "Bytes(255)"),
        ("bytes-pair", "Bytes(0,255)", "Bytes(0, 255)"),
        ("bytes-equal", "Bytes(1,2,3)==Bytes(1,2,3)", "true"),
        ("bytes-empty-equal", "Bytes()==Bytes()", "true"),
        ("bytes-size-value", "byteValue.size.value", "3"),
        ("bytes-size-unit", "byteValue.size.unit", "b"),
        ("bytes-size-type", "byteValue.size is DataSize", "true"),
        (
            "bytes-size-repeat",
            "byteValue.size == byteValue.size",
            "true",
        ),
        (
            "encoded-size-0",
            "\"x\".repeat(0).encodeToBytes(\"UTF-8\").size",
            "0.b",
        ),
        (
            "encoded-size-1",
            "\"x\".repeat(1).encodeToBytes(\"UTF-8\").size",
            "1.b",
        ),
        (
            "encoded-size-999",
            "\"x\".repeat(999).encodeToBytes(\"UTF-8\").size",
            "999.b",
        ),
        (
            "encoded-size-1000",
            "\"x\".repeat(1000).encodeToBytes(\"UTF-8\").size",
            "1.kb",
        ),
        (
            "encoded-size-1024",
            "\"x\".repeat(1024).encodeToBytes(\"UTF-8\").size",
            "1.024.kb",
        ),
    ] {
        assert_eq!(
            output_with_source(source, expression),
            expected.as_bytes(),
            "additive Java row {id}"
        );
    }
}

// These public-query regressions copy the complete Java string boundary,
// including error text and UTF-16 code units. Fresh split JVM captures 05 and
// 07/08/09 remain separate from the main oracle03 matrix.
fn public_query(
    probe: &mut pkl_runtime::probe::SourceProbe,
    source: &str,
    uri: &str,
    expression: &str,
) -> pkl_runtime::probe::SourceEvent {
    probe
        .evaluate_module_source(uri, Some(source), expression, 0, None)
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
    panic!("DataSize public-query poll bound")
}

#[test]
fn undefined_alias_default_reports_property_name() {
    let source = "// Stage57 DataSize public API source. All expressions are supplied separately.\nnanSize = (0.0 / 0.0).mb\ntypealias DataSizeAlias = DataSize\nclass UnitHolder { x: DataSizeUnit = \"mb\" }\nclass SizeHolder { x: DataSize = 1.mb }\nclass RequiredSize { x: DataSize }\nfunction useDataSize(x: DataSize): String = x.unit\nmarker = \"stage57\"\nclass UnitNullable { x: DataSizeUnit? }\nclass UnitUnion { x: DataSizeUnit|Int }\nclass UnitRequired { x: DataSizeUnit }\nbyteValue = Bytes(1,2,3)\n";
    let uri = "file:///stage57/datasize-additive-oracle.pkl";
    let mut probe =
        pkl_runtime::probe::SourceProbe::new_module_evaluator("compact", false, false, false)
            .unwrap();
    let expected = "–– Pkl Error ––\nTried to read property `x` but its value is undefined.\n\n11 | class UnitRequired { x: DataSizeUnit }\n                          ^\nat datasize-additive-oracle#UnitRequired.x (file:///stage57/datasize-additive-oracle.pkl)\n\n1 | new UnitRequired {}.x\n    ^^^^^^^^^^^^^^^^^^^^^\nat  (repl:text)\n";
    let event = public_query(&mut probe, source, uri, "new UnitRequired {}.x");
    assert_eq!(event.kind, "error", "{event:?}");
    assert_eq!(
        event.error.unwrap(),
        expected.encode_utf16().collect::<Vec<_>>(),
        "Java row unit-required-missing"
    );
    probe.close();
    assert_eq!(probe.collect().slots, 0);
}

#[test]
fn bytes_public_query_factory_ignores_module_method() {
    let source = "// Stage57 public resolution shadow control; separate fresh module.\nfunction Bytes(x: Int): Int = x\n";
    let uri = "file:///stage57/datasize-bytes-shadow-oracle.pkl";
    let mut probe =
        pkl_runtime::probe::SourceProbe::new_module_evaluator("compact", false, false, false)
            .unwrap();
    for (id, expression, kind, expected) in [
        (
            "module-function-overflow",
            "Bytes(256)",
            "error",
            "–– Pkl Error ––\nType constraint `isBetween(0, 255)` violated.\nValue: 256\n\n    isBetween(0, 255)\n    │\n    false\n\n1109 | typealias UInt8 = Int(isBetween(0, 255))\n                             ^^^^^^^^^^^^^^^^^\nat  (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L1109)\n\n1 | Bytes(256)\n          ^^^\nat  (repl:text)\n",
        ),
        (
            "module-function-negative",
            "Bytes(-1)",
            "error",
            "–– Pkl Error ––\nType constraint `isBetween(0, 255)` violated.\nValue: -1\n\n    isBetween(0, 255)\n    │\n    false\n\n1109 | typealias UInt8 = Int(isBetween(0, 255))\n                             ^^^^^^^^^^^^^^^^^\nat  (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L1109)\n\n1 | Bytes(-1)\n           ^\nat  (repl:text)\n",
        ),
        ("module-function-zero", "Bytes(0)", "value", "Bytes(0)"),
        ("module-function-missing", "Bytes()", "value", "Bytes()"),
        (
            "module-function-extra",
            "Bytes(1,2)",
            "value",
            "Bytes(1, 2)",
        ),
        (
            "module-function-float",
            "Bytes(1.0)",
            "error",
            "–– Pkl Error ––\nExpected value of type `Int`, but got type `Float`.\nValue: 1.0\n\n1109 | typealias UInt8 = Int(isBetween(0, 255))\n                         ^^^\nat  (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L1109)\n\n1 | Bytes(1.0)\n          ^^^\nat  (repl:text)\n",
        ),
        (
            "module-function-let",
            "let (Bytes = (x) -> x) Bytes(256)",
            "error",
            "–– Pkl Error ––\nType constraint `isBetween(0, 255)` violated.\nValue: 256\n\n    isBetween(0, 255)\n    │\n    false\n\n1109 | typealias UInt8 = Int(isBetween(0, 255))\n                             ^^^^^^^^^^^^^^^^^\nat  (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L1109)\n\n1 | let (Bytes = (x) -> x) Bytes(256)\n    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^\nat repl:text.<let expr> (repl:text)\n",
        ),
    ] {
        let event = public_query(&mut probe, source, uri, expression);
        assert_eq!(event.kind, kind, "Java row {id}: {event:?}");
        let actual = if kind == "error" {
            event.error
        } else {
            event.value
        };
        assert_eq!(
            actual.unwrap(),
            expected.encode_utf16().collect::<Vec<_>>(),
            "Java row {id}"
        );
    }
    probe.close();
    assert_eq!(probe.collect().slots, 0);
}

#[test]
fn bytes_qualified_query_dispatches_module_method() {
    let source = "// Stage57 public resolution shadow control; separate fresh module.\nfunction Bytes(x: Int): Int = x\n";
    let uri = "file:///stage57/datasize-bytes-qualified-shadow.pkl";
    let mut probe =
        pkl_runtime::probe::SourceProbe::new_module_evaluator("compact", false, false, false)
            .unwrap();
    for (id, expression, kind, expected) in [
        ("this-call-overflow", "this.Bytes(256)", "value", "256"),
        ("this-call-zero", "this.Bytes(0)", "value", "0"),
        (
            "this-call-missing",
            "this.Bytes()",
            "error",
            "–– Pkl Error ––\nExpected 1 function arguments but got 0.\n\n2 | function Bytes(x: Int): Int = x\n    ^^^^^^^^^^^^^^^^^^^^^^^^^^^\nat datasize-bytes-qualified-shadow#Bytes (file:///stage57/datasize-bytes-qualified-shadow.pkl)\n\n1 | this.Bytes()\n    ^^^^^^^^^^^^\nat  (repl:text)\n",
        ),
        ("module-call-overflow", "module.Bytes(256)", "value", "256"),
        ("module-call-zero", "module.Bytes(0)", "value", "0"),
        (
            "module-property",
            "this.Bytes",
            "error",
            "–– Pkl Error ––\nCannot find property `Bytes` in module `datasize-bytes-qualified-shadow`.\n\n1 | this.Bytes\n    ^^^^^^^^^^\nat  (repl:text)\n\nAvailable properties in module `datasize-bytes-qualified-shadow`:\noutput\n",
        ),
        (
            "unqualified-overflow",
            "Bytes(256)",
            "error",
            "–– Pkl Error ––\nType constraint `isBetween(0, 255)` violated.\nValue: 256\n\n    isBetween(0, 255)\n    │\n    false\n\n1109 | typealias UInt8 = Int(isBetween(0, 255))\n                             ^^^^^^^^^^^^^^^^^\nat  (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L1109)\n\n1 | Bytes(256)\n          ^^^\nat  (repl:text)\n",
        ),
    ] {
        let event = public_query(&mut probe, source, uri, expression);
        assert_eq!(event.kind, kind, "Java row {id}: {event:?}");
        let actual = if kind == "error" {
            event.error
        } else {
            event.value
        };
        assert_eq!(
            actual.unwrap(),
            expected.encode_utf16().collect::<Vec<_>>(),
            "Java row {id}"
        );
    }
    probe.close();
    assert_eq!(probe.collect().slots, 0);
}

#[test]
fn bytes_lexical_calls_and_public_queries_have_distinct_resolution() {
    let source = "// Stage57 lexical Bytes function resolution inside a module source.\nfunction Bytes(x: Int): Int = x\ninside = Bytes(256)\ninsideLet = let (Bytes = (x) -> x) Bytes(256)\ninsideQualified = this.Bytes(256)\n";
    let uri = "file:///stage57/datasize-bytes-lexical-shadow.pkl";
    let mut probe =
        pkl_runtime::probe::SourceProbe::new_module_evaluator("compact", false, false, false)
            .unwrap();
    for (id, expression, kind, expected) in [
        ("lexical-inside", "inside", "value", "256"),
        ("lexical-let", "insideLet", "value", "256"),
        ("lexical-qualified", "insideQualified", "value", "256"),
        (
            "query-unqualified",
            "Bytes(256)",
            "error",
            "–– Pkl Error ––\nType constraint `isBetween(0, 255)` violated.\nValue: 256\n\n    isBetween(0, 255)\n    │\n    false\n\n1109 | typealias UInt8 = Int(isBetween(0, 255))\n                             ^^^^^^^^^^^^^^^^^\nat  (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L1109)\n\n1 | Bytes(256)\n          ^^^\nat  (repl:text)\n",
        ),
        ("query-this", "this.Bytes(256)", "value", "256"),
        ("query-module", "module.Bytes(256)", "value", "256"),
    ] {
        let event = public_query(&mut probe, source, uri, expression);
        assert_eq!(event.kind, kind, "Java row {id}: {event:?}");
        let actual = if kind == "error" {
            event.error
        } else {
            event.value
        };
        assert_eq!(
            actual.unwrap(),
            expected.encode_utf16().collect::<Vec<_>>(),
            "Java row {id}"
        );
    }
    probe.close();
    assert_eq!(probe.collect().slots, 0);
}
