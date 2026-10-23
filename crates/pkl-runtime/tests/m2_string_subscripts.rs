//! Whole Java expression results, including unmatched UTF-16 and exact errors.
use pkl_runtime::probe::{SourceEvent, SourceProbe};
use pkl_runtime::value::PklString;

struct Case {
    expression: &'static str,
    status: &'static str,
    stdout: &'static [u8],
    stderr: &'static [u8],
    units: &'static [u8],
}

macro_rules! case {
    ($index:literal, $channel:literal) => {
        Case {
            expression: include_str!(concat!(
                "../../../tests/fixtures/m2-string-subscripts/raw/0/outer-",
                $index,
                ".expression.bin"
            )),
            status: include_str!(concat!(
                "../../../tests/fixtures/m2-string-subscripts/raw/0/outer-",
                $index,
                ".status.bin"
            )),
            stdout: include_bytes!(concat!(
                "../../../tests/fixtures/m2-string-subscripts/raw/0/outer-",
                $index,
                ".stdout.bin"
            )),
            stderr: include_bytes!(concat!(
                "../../../tests/fixtures/m2-string-subscripts/raw/0/outer-",
                $index,
                ".stderr.bin"
            )),
            units: include_bytes!(concat!(
                "../../../tests/fixtures/m2-string-subscripts/raw/0/outer-",
                $index,
                ".",
                $channel,
                ".code-units.bin"
            )),
        }
    };
}

fn next(probe: &mut SourceProbe) -> SourceEvent {
    for _ in 0..100_000 {
        let event = probe.poll(0, 1).unwrap();
        probe.collect();
        if event.kind != "yield" {
            return event;
        }
    }
    panic!("string subscript did not finish");
}

#[test]
fn code_point_indexing_and_diagnostics_match_full_java_results() {
    let cases = [
        case!("0", "stdout"),
        case!("1", "stdout"),
        case!("2", "stdout"),
        case!("3", "stderr"),
        case!("4", "stdout"),
        case!("5", "stdout"),
        case!("6", "stdout"),
        case!("7", "stdout"),
        case!("8", "stderr"),
        case!("9", "stderr"),
        case!("10", "stderr"),
        case!("11", "stderr"),
        case!("12", "stderr"),
        case!("13", "stderr"),
    ];
    let expressions = cases
        .iter()
        .map(|case| case.expression.to_owned())
        .collect::<Vec<_>>();
    for _ in 0..2 {
        let mut probe = SourceProbe::new_expressions(
            include_str!("../../../tests/fixtures/m2-string-subscripts/main.pkl"),
            "file:///Users/cody.ij.hwang/Projects/pkl-rs.stage42/target/m2/stage42-string-subscript-oracle-01/workspace/main.pkl",
            "main", &expressions, "compact", false, false, false,
        ).unwrap();
        for (index, case) in cases.iter().enumerate() {
            probe
                .start_path(index.try_into().unwrap(), 0, None)
                .unwrap();
            let event = next(&mut probe);
            let (actual, expected, bytes) = match case.status {
                "success\n" => {
                    assert_eq!(event.kind, "value", "{}: {event:?}", case.expression);
                    assert!(event.error.is_none());
                    assert!(case.stderr.is_empty());
                    (event.value.unwrap(), case.units, case.stdout)
                }
                "error\n" => {
                    assert_eq!(event.kind, "error", "{}: {event:?}", case.expression);
                    assert_eq!(event.disposition, Some("language-error"));
                    assert!(event.value.is_none());
                    assert!(case.stdout.is_empty());
                    (event.error.unwrap(), case.units, case.stderr)
                }
                status => panic!("unexpected Java outcome {status:?}"),
            };
            let (pairs, remainder) = expected.as_chunks::<2>();
            assert!(remainder.is_empty());
            let expected = pairs
                .iter()
                .map(|pair| u16::from_be_bytes(*pair))
                .collect::<Vec<_>>();
            assert_eq!(
                actual,
                expected,
                "{}: {}",
                case.expression,
                String::from_utf16_lossy(&actual)
            );
            assert_eq!(
                PklString::from_utf16(actual).to_java_utf8().as_bytes(),
                bytes,
                "{}",
                case.expression
            );
        }
        probe.close();
        assert_eq!(probe.collect().slots, 0);
    }
}
