//! Complete independent Java 0.32.0 string observations, capture-02/repeat-0.
//! Authority: m2-collection-property-names/oracle-evidence.tar.gz.
#[path = "support/module_keys.rs"]
mod module_keys;
use pkl_runtime::probe::SourceProbe;
#[test]
fn complete_java_matrix() {
    let mut probe = SourceProbe::new_module_evaluator("compact", false, false, false).unwrap();
    let mut failures = Vec::new();
    let rows = [
        (
            r########"authored-direct"########,
            concat!(
                r########"new Dynamic { `"########,
                "\0",
                r########"property:[55296]` = 2 }.`"########,
                "\0",
                r########"property:[55296]`"########
            ),
            r########"value"########,
            r########"2"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"authored-has"########,
            concat!(
                r########"new Dynamic { `"########,
                "\0",
                r########"property:[55296]` = 2 }.hasProperty("\u{0}property:[55296]")"########
            ),
            r########"value"########,
            r########"true"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"authored-get"########,
            concat!(
                r########"new Dynamic { `"########,
                "\0",
                r########"property:[55296]` = 2 }.getProperty("\u{0}property:[55296]")"########
            ),
            r########"value"########,
            r########"2"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"authored-get-or-null"########,
            concat!(
                r########"new Dynamic { `"########,
                "\0",
                r########"property:[55296]` = 2 }.getPropertyOrNull("\u{0}property:[55296]")"########
            ),
            r########"value"########,
            r########"2"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"surrogate-control"########,
            r########"Map("\u{d800}", 1).toDynamic().getProperty("\u{d800}")"########,
            r########"value"########,
            r########"1"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"amended-surrogate-get"########,
            concat!(
                r########"(Map("\u{d800}", 1).toDynamic()) { `"########,
                "\0",
                r########"property:[55296]` = 2 }.getProperty("\u{d800}")"########
            ),
            r########"value"########,
            r########"1"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"amended-authored-get"########,
            concat!(
                r########"(Map("\u{d800}", 1).toDynamic()) { `"########,
                "\0",
                r########"property:[55296]` = 2 }.getProperty("\u{0}property:[55296]")"########
            ),
            r########"value"########,
            r########"2"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"amended-authored-direct"########,
            concat!(
                r########"(Map("\u{d800}", 1).toDynamic()) { `"########,
                "\0",
                r########"property:[55296]` = 2 }.`"########,
                "\0",
                r########"property:[55296]`"########
            ),
            r########"value"########,
            r########"2"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"amended-to-map"########,
            concat!(
                r########"(Map("\u{d800}", 1).toDynamic()) { `"########,
                "\0",
                r########"property:[55296]` = 2 }.toMap()"########
            ),
            r########"value"########,
            vec![
                77, 97, 112, 40, 34, 55296, 34, 44, 32, 49, 44, 32, 34, 0, 112, 114, 111, 112, 101,
                114, 116, 121, 58, 91, 53, 53, 50, 57, 54, 93, 34, 44, 32, 50, 41,
            ],
        ),
        (
            r########"synthetic-two-keys"########,
            r########"Map("\u{d800}",1,"\u{0}property:[55296]",2).toDynamic().toMap()"########,
            r########"value"########,
            vec![
                77, 97, 112, 40, 34, 55296, 34, 44, 32, 49, 44, 32, 34, 0, 112, 114, 111, 112, 101,
                114, 116, 121, 58, 91, 53, 53, 50, 57, 54, 93, 34, 44, 32, 50, 41,
            ],
        ),
        (
            r########"lf-only"########,
            r########"Map("\u{a}property:[55296]",3).toDynamic().toMap()"########,
            r########"value"########,
            r########"Map("\nproperty:[55296]", 3)"########
                .encode_utf16()
                .collect::<Vec<_>>(),
        ),
        (
            r########"lf-alone"########,
            r########"Map("\u{a}",3).toDynamic().toMap()"########,
            r########"value"########,
            r########"Map("\n", 3)"########
                .encode_utf16()
                .collect::<Vec<_>>(),
        ),
        (
            r########"lf-high-map"########,
            r########"Map("\u{d800}",1,"\u{a}property:[55296]",3).toDynamic().toMap()"########,
            r########"value"########,
            vec![
                77, 97, 112, 40, 34, 55296, 34, 44, 32, 49, 44, 32, 34, 92, 110, 112, 114, 111,
                112, 101, 114, 116, 121, 58, 91, 53, 53, 50, 57, 54, 93, 34, 44, 32, 51, 41,
            ],
        ),
        (
            r########"lf-high-length"########,
            r########"Map("\u{d800}",1,"\u{a}property:[55296]",3).toDynamic().toMap().length"########,
            r########"value"########,
            r########"2"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"lf-high-get"########,
            r########"Map("\u{d800}",1,"\u{a}property:[55296]",3).toDynamic().getProperty("\u{d800}")"########,
            r########"value"########,
            r########"1"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"lf-prefix-get"########,
            r########"Map("\u{d800}",1,"\u{a}property:[55296]",3).toDynamic().getProperty("\u{a}property:[55296]")"########,
            r########"value"########,
            r########"3"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"lf-high-has"########,
            r########"Map("\u{d800}",1,"\u{a}property:[55296]",3).toDynamic().hasProperty("\u{d800}")"########,
            r########"value"########,
            r########"true"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"lf-prefix-has"########,
            r########"Map("\u{d800}",1,"\u{a}property:[55296]",3).toDynamic().hasProperty("\u{a}property:[55296]")"########,
            r########"value"########,
            r########"true"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"lf-high-render"########,
            r########"Map("\u{d800}",1,"\u{a}property:[55296]",3).toDynamic().toString()"########,
            r########"value"########,
            vec![
                110, 101, 119, 32, 68, 121, 110, 97, 109, 105, 99, 32, 123, 32, 96, 55296, 96, 32,
                61, 32, 49, 59, 32, 96, 10, 112, 114, 111, 112, 101, 114, 116, 121, 58, 91, 53, 53,
                50, 57, 54, 93, 96, 32, 61, 32, 51, 32, 125,
            ],
        ),
        (
            r########"triple-high-nul-lf"########,
            r########"Map("\u{d800}",1,"\u{0}property:[55296]",2,"\u{a}property:[55296]",3).toDynamic().toMap()"########,
            r########"value"########,
            vec![
                77, 97, 112, 40, 34, 55296, 34, 44, 32, 49, 44, 32, 34, 0, 112, 114, 111, 112, 101,
                114, 116, 121, 58, 91, 53, 53, 50, 57, 54, 93, 34, 44, 32, 50, 44, 32, 34, 92, 110,
                112, 114, 111, 112, 101, 114, 116, 121, 58, 91, 53, 53, 50, 57, 54, 93, 34, 44, 32,
                51, 41,
            ],
        ),
        (
            r########"low-lf-map"########,
            r########"Map("\u{dc00}",4,"\u{a}property:[56320]",5).toDynamic().toMap()"########,
            r########"value"########,
            vec![
                77, 97, 112, 40, 34, 56320, 34, 44, 32, 52, 44, 32, 34, 92, 110, 112, 114, 111,
                112, 101, 114, 116, 121, 58, 91, 53, 54, 51, 50, 48, 93, 34, 44, 32, 53, 41,
            ],
        ),
        (
            r########"cr-high-map"########,
            r########"Map("\u{d800}",1,"\u{d}property:[55296]",6).toDynamic().toMap()"########,
            r########"value"########,
            vec![
                77, 97, 112, 40, 34, 55296, 34, 44, 32, 49, 44, 32, 34, 92, 114, 112, 114, 111,
                112, 101, 114, 116, 121, 58, 91, 53, 53, 50, 57, 54, 93, 34, 44, 32, 54, 41,
            ],
        ),
        (
            r########"backtick-high-map"########,
            r########"Map("\u{d800}",1,"\u{60}property:[55296]",7).toDynamic().toMap()"########,
            r########"value"########,
            vec![
                77, 97, 112, 40, 34, 55296, 34, 44, 32, 49, 44, 32, 34, 96, 112, 114, 111, 112,
                101, 114, 116, 121, 58, 91, 53, 53, 50, 57, 54, 93, 34, 44, 32, 55, 41,
            ],
        ),
        (
            r########"backtick-key-get"########,
            r########"Map("\u{d800}",1,"\u{60}property:[55296]",7).toDynamic().getProperty("\u{60}property:[55296]")"########,
            r########"value"########,
            r########"7"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"line-separator-high-map"########,
            r########"Map("\u{d800}",1,"\u{2028}property:[55296]",8).toDynamic().toMap()"########,
            r########"value"########,
            vec![
                77, 97, 112, 40, 34, 55296, 34, 44, 32, 49, 44, 32, 34, 8232, 112, 114, 111, 112,
                101, 114, 116, 121, 58, 91, 53, 53, 50, 57, 54, 93, 34, 44, 32, 56, 41,
            ],
        ),
        (
            r########"paragraph-separator-high-map"########,
            r########"Map("\u{d800}",1,"\u{2029}property:[55296]",9).toDynamic().toMap()"########,
            r########"value"########,
            vec![
                77, 97, 112, 40, 34, 55296, 34, 44, 32, 49, 44, 32, 34, 8233, 112, 114, 111, 112,
                101, 114, 116, 121, 58, 91, 53, 53, 50, 57, 54, 93, 34, 44, 32, 57, 41,
            ],
        ),
    ];
    assert_eq!(rows.len(), 26);
    for (id, expression, kind, expected) in rows {
        probe
            .evaluate_module_source(
                "file:///stage62/collection-property-names.pkl",
                Some(include_str!(
                    "../../../tests/fixtures/m2-collection-property-names/module.pkl"
                )),
                expression,
                0,
                None,
            )
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
        let event = terminal.expect("conversion query poll bound");
        let actual = if event.kind == "value" {
            event.value
        } else {
            event.error
        };
        if event.kind != kind || actual.as_ref() != Some(&expected) {
            failures.push(format!(
                "{id}: {expression}\nexpected {kind}: {expected:?}\nactual {}: {}",
                event.kind,
                actual
                    .map(|v| String::from_utf16_lossy(&v))
                    .unwrap_or_default()
            ));
        }
    }
    probe.close();
    assert_eq!(probe.collect().slots, 0);
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}
