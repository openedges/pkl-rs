//! Regression authority: exact Stage67 Java scalar case and Stage47/48 raw passes.
//! The captured URI is a virtual module identity; source is supplied in memory.
#[path = "support/module_keys.rs"]
mod module_keys;
use pkl_runtime::probe::SourceProbe;

#[test]
fn empty_reference_amendment_rejects_before_reference_rendering() {
    let mut probe = SourceProbe::new_module_evaluator("compact", false, false, false).unwrap();
    let expected = r########"–– Pkl Error ––
Cannot instantiate, or amend an instance of, external class `pkl.ref#Reference`.

5 | result = (r) {}
              ^
at reference-stage10-reference-amendment-invalid#result (file:///Users/cody.ij.hwang/Projects/pkl-rs.stage57/tests/fixtures/m2/reference-stage10-reference-amendment-invalid.pkl)

To create an instance of an external class, use its literal syntax or constructor method.
Examples:
* `123` uses literal syntax to create an instance of class `Int`.
* `Pair(1, 2)` uses the `Pair()` constructor method to create an instance of class `Pair`.

1 | result
    ^^^^^^
at  (repl:text)
"########.encode_utf16().collect::<Vec<_>>();
    // Repeat the failed read with GC between polls; no successful "ok" value may be cached.
    for _ in 0..2 {
        probe.evaluate_module_source(
            r########"file:///Users/cody.ij.hwang/Projects/pkl-rs.stage57/tests/fixtures/m2/reference-stage10-reference-amendment-invalid.pkl"########,
            Some(include_str!("../../../tests/fixtures/m2/reference-stage10-reference-amendment-invalid.pkl")),
            "result", 0, None,
        ).unwrap();
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
        let event = terminal.expect("Reference amendment poll bound");
        assert_eq!(event.kind, "error");
        assert_eq!(event.error, Some(expected.clone()));
    }
    probe.close();
    assert_eq!(probe.collect().slots, 0);
}
