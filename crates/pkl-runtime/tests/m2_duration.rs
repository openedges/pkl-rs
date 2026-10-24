//! Duration value foundation; the separately acquired Java cohort is the oracle.
use pkl_runtime::{
    duration::{Duration, DurationUnit},
    evaluation::frontend::{ModuleEvalOptions, ModuleSourceInput, evaluate_module_output},
    parser::parse_module,
    resolver::resolve_module,
    value::Value,
    vm::{Vm, VmPoll, compile_module},
};

fn output(source: &str) -> Vec<u8> {
    let result = evaluate_module_output(
        ModuleSourceInput {
            text: source,
            uri: "file:///duration.pkl",
            inferred_name: "duration",
        },
        &ModuleEvalOptions::default(),
    );
    assert_eq!(
        result.exit_code,
        0,
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    result.stdout
}

#[test]
fn numeric_unit_properties_preserve_units_and_java_double_widening() {
    for unit in ["ns", "us", "ms", "s", "min", "h", "d"] {
        assert_eq!(
            output(&format!(
                r#"
integer = 3
fraction = 1.25
value: Duration = integer.{unit}
output {{ text = List(value.toString(), fraction.{unit}.toString(), value.value, value.unit, value is Duration, value is Any, value is Number, value.getClass() == Duration).toString() }}
"#
            )),
            format!("List(\"3.{unit}\", \"1.25.{unit}\", 3, \"{unit}\", true, true, false, true)")
                .as_bytes()
        );
    }
    assert_eq!(
        output(
            r#"output { text = List(9007199254740993.s, 9223372036854775807.s, (-0.0).ms, (1.25).h.value, (-0.0).s.value, Infinity.s, NaN.d).toString() }"#
        ),
        b"List(9007199254740992.s, 9223372036854775807.s, 0.ms, 1.25, 0, Infinity.s, NaN.d)"
    );
}

#[test]
fn duration_display_and_collection_results_share_the_real_value_type() {
    assert_eq!(
        output("one=1.s\nfraction=1.5.ms\n"),
        b"one = 1.s\nfraction = 1.5.ms\n"
    );
    assert_eq!(
        output(r#"output { text = "time=\(1.25.h)" }"#),
        b"time=1.25.h"
    );
    assert_eq!(
        output(
            r#"
first=1.s
same=1000.ms
values=List(1,2).map((x)->x.s)
keys=Set(first,same).toList().map((x)->x.unit)
table=Map(first,1,same,2)
output { text = List(first==same, first==1, values, keys, table.keys.toList().map((x)->x.unit), table.values).toString() }
"#
        ),
        b"List(true, false, List(1.s, 2.s), List(\"s\"), List(\"s\"), List(2))"
    );
}

#[test]
fn normalized_duration_keys_preserve_java_nan_zero_and_overflow_observations() {
    // Java07 duration-equality-hash calls 2..17, acquired independently.
    // Keep hash routing separate from numeric equality: signed zero hashes
    // differ even though these actual Paguro leaf lookups compare them equal.
    for (expression, expected) in [
        ("(nan==nan).toString()", "true"),
        ("(nan==nanOther).toString()", "false"),
        ("(NaN.s==NaN.s).toString()", "false"),
        ("(zero==0.s).toString()", "true"),
        ("Set(nan,nan,NaN.s,NaN.s).length.toString()", "3"),
        ("Set(zero,0.s).length.toString()", "1"),
        (
            "Map(zero,\"negative\",0.min,\"positive\").toString()",
            "Map(0.s, \"positive\")",
        ),
        (
            "Map(nan,1,nan,2,nanOther,3).toString()",
            "Map(NaN.s, 2, NaN.s, 3)",
        ),
        (
            "Set(1e300.d,Infinity.s).toString()",
            "Set(9223372036854775807.d)",
        ),
    ] {
        assert_eq!(
            output(&format!(
                "nan=NaN.s\nnanOther=NaN.s\nzero=(-0.0).s\noutput {{ text={expression} }}"
            )),
            expected.as_bytes(),
            "{expression}"
        );
    }
}

#[test]
fn duration_callbacks_survive_collection_at_every_poll_boundary() {
    let source =
        r#"result = List(1,2).map((x)->read("duration:\(x)")).map((x:Duration)->x.unit).join("|")"#;
    let program = compile_module(&resolve_module(&parse_module(source).unwrap()).unwrap()).unwrap();
    let mut vm = Vm::new(program.clone());
    vm.start(program.entry, vec![]).unwrap();
    let event = |vm: &mut Vm| {
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
        panic!("duration poll bound")
    };
    let VmPoll::Ready(Ok(module)) = event(&mut vm) else {
        panic!("module")
    };
    vm.start_member(module.as_object().unwrap(), "result")
        .unwrap();
    for (index, unit) in [(1, DurationUnit::Seconds), (2, DurationUnit::Days)] {
        let VmPoll::Pending(request) = event(&mut vm) else {
            panic!("read")
        };
        assert_eq!(&*request.uri, format!("duration:{index}"));
        vm.resume(request.id, Ok(Value::Duration(Duration::new(1.5, unit))))
            .unwrap();
    }
    assert_eq!(event(&mut vm), VmPoll::Ready(Ok(Value::string("s|d"))));
    vm.close();
    assert_eq!(vm.heap_stats().slots, 0);
}
