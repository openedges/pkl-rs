//! Native status/full UTF-16 checks from two independent Java control captures.
//! Authority: reports/m2/stage64-java-evidence.tar.gz, the local-controls and
//! eof-newline capture-01/repeat-0 directories (18 + 3 rows).
//! Archive SHA-256: 6589ad61464daaa0796d108221d0d2299299b4fa97b9dbd9444b1c94a4cc73d1.
//! These rows do not replace complete process or portable comparisons.
#[path = "support/module_keys.rs"]
mod module_keys;
use pkl_runtime::probe::SourceProbe;

#[test]
fn complete_java_local_amendment_and_eof_controls() {
    let rows = [
        (
            r########"local-repeated-empty"########,
            r########"file:///stage64/amendment-local-controls.pkl"########,
            r########"base = new Dynamic {}
"########,
            r########"new Dynamic { local item {} {} }"########,
            r########"error"########,
            r########"–– Pkl Error ––
A local property definition cannot be amended.

1 | new Dynamic { local item {} {} }
                             ^^
at repl:text.item (repl:text)

Use definition syntax instead, for example `local person = new { ... }` instead of `local person { ... }`.
"########,
        ),
        (
            r########"local-single-body"########,
            r########"file:///stage64/amendment-local-controls.pkl"########,
            r########"base = new Dynamic {}
"########,
            r########"new Dynamic { local item { value = 1 } }"########,
            r########"error"########,
            r########"–– Pkl Error ––
A local property definition cannot be amended.

1 | new Dynamic { local item { value = 1 } }
                             ^^^^^^^^^^^^^
at repl:text.item (repl:text)

Use definition syntax instead, for example `local person = new { ... }` instead of `local person { ... }`.
"########,
        ),
        (
            r########"const-local-body"########,
            r########"file:///stage64/amendment-local-controls.pkl"########,
            r########"base = new Dynamic {}
"########,
            r########"new Dynamic { const local item { value = 1 } }"########,
            r########"error"########,
            r########"–– Pkl Error ––
A local property definition cannot be amended.

1 | new Dynamic { const local item { value = 1 } }
                                   ^^^^^^^^^^^^^
at repl:text.item (repl:text)

Use definition syntax instead, for example `local person = new { ... }` instead of `local person { ... }`.
"########,
        ),
        (
            r########"local-body-throw"########,
            r########"file:///stage64/amendment-local-controls.pkl"########,
            r########"base = new Dynamic {}
"########,
            r########"new Dynamic { local item { value = throw("body") } }"########,
            r########"error"########,
            r########"–– Pkl Error ––
A local property definition cannot be amended.

1 | new Dynamic { local item { value = throw("body") } }
                             ^^^^^^^^^^^^^^^^^^^^^^^^^
at repl:text.item (repl:text)

Use definition syntax instead, for example `local person = new { ... }` instead of `local person { ... }`.
"########,
        ),
        (
            r########"local-later-body"########,
            r########"file:///stage64/amendment-local-controls.pkl"########,
            r########"base = new Dynamic {}
"########,
            r########"new Dynamic { local item {} { value = throw("later-body") } }"########,
            r########"error"########,
            r########"–– Pkl Error ––
A local property definition cannot be amended.

1 | new Dynamic { local item {} { value = throw("later-body") } }
                             ^^
at repl:text.item (repl:text)

Use definition syntax instead, for example `local person = new { ... }` instead of `local person { ... }`.
"########,
        ),
        (
            r########"local-missing-inner-brace"########,
            r########"file:///stage64/amendment-local-controls.pkl"########,
            r########"base = new Dynamic {}
"########,
            r########"new Dynamic { local item { value = 1 }"########,
            r########"error"########,
            r########"–– Pkl Error ––
Missing `}` delimiter.

1 | new Dynamic { local item { value = 1 }
    ^
at repl:text (repl:text)
"########,
        ),
        (
            r########"local-later-missing-body"########,
            r########"file:///stage64/amendment-local-controls.pkl"########,
            r########"base = new Dynamic {}
"########,
            r########"new Dynamic { local item {} } {"########,
            r########"error"########,
            r########"–– Pkl Error ––
Missing `}` delimiter.

1 | new Dynamic { local item {} } {
    ^
at repl:text (repl:text)
"########,
        ),
        (
            r########"local-assignment-valid"########,
            r########"file:///stage64/amendment-local-controls.pkl"########,
            r########"base = new Dynamic {}
"########,
            r########"new Dynamic { local item = new Dynamic { value = 1 } }"########,
            r########"value"########,
            r########"new Dynamic {}"########,
        ),
        (
            r########"local-parent-outer-amend"########,
            r########"file:///stage64/amendment-local-controls.pkl"########,
            r########"base = new Dynamic {}
"########,
            r########"(new Dynamic { local item = new Dynamic { x = 1 } }) { item { y = 2 } }"########,
            r########"value"########,
            r########"new Dynamic { item { y = 2 } }"########,
        ),
        (
            r########"eof-amend-body"########,
            r########"file:///stage64/amendment-local-controls.pkl"########,
            r########"base = new Dynamic {}
"########,
            r########"new Dynamic { value = 1 } {"########,
            r########"error"########,
            r########"–– Pkl Error ––
Missing `}` delimiter.

1 | new Dynamic { value = 1 } {
    ^
at repl:text (repl:text)
"########,
        ),
        (
            r########"eof-amend-body-space"########,
            r########"file:///stage64/amendment-local-controls.pkl"########,
            r########"base = new Dynamic {}
"########,
            r########"new Dynamic { value = 1 } { "########,
            r########"error"########,
            r########"–– Pkl Error ––
Missing `}` delimiter.

1 | new Dynamic { value = 1 } {
                               ^
at repl:text (repl:text)
"########,
        ),
        (
            r########"eof-amend-body-comment"########,
            r########"file:///stage64/amendment-local-controls.pkl"########,
            r########"base = new Dynamic {}
"########,
            r########"new Dynamic { value = 1 } { // comment"########,
            r########"error"########,
            r########"–– Pkl Error ––
Missing `}` delimiter.

1 | new Dynamic { value = 1 } { // comment
                               ^
at repl:text (repl:text)
"########,
        ),
        (
            r########"eof-object-body"########,
            r########"file:///stage64/amendment-local-controls.pkl"########,
            r########"base = new Dynamic {}
"########,
            r########"new Dynamic {"########,
            r########"error"########,
            r########"–– Pkl Error ––
Missing `}` delimiter.

1 | new Dynamic {
    ^
at repl:text (repl:text)
"########,
        ),
        (
            r########"eof-object-body-space"########,
            r########"file:///stage64/amendment-local-controls.pkl"########,
            r########"base = new Dynamic {}
"########,
            r########"new Dynamic { "########,
            r########"error"########,
            r########"–– Pkl Error ––
Missing `}` delimiter.

1 | new Dynamic {
                 ^
at repl:text (repl:text)
"########,
        ),
        (
            r########"eof-object-body-comment"########,
            r########"file:///stage64/amendment-local-controls.pkl"########,
            r########"base = new Dynamic {}
"########,
            r########"new Dynamic { // comment"########,
            r########"error"########,
            r########"–– Pkl Error ––
Missing `}` delimiter.

1 | new Dynamic { // comment
                 ^
at repl:text (repl:text)
"########,
        ),
        (
            r########"eof-plus"########,
            r########"file:///stage64/amendment-local-controls.pkl"########,
            r########"base = new Dynamic {}
"########,
            r########"1 +"########,
            r########"error"########,
            r########"–– Pkl Error ––
Unexpected end of file.

1 | 1 +
    ^
at repl:text (repl:text)
"########,
        ),
        (
            r########"eof-empty-expression"########,
            r########"file:///stage64/amendment-local-controls.pkl"########,
            r########"base = new Dynamic {}
"########,
            r########""########,
            r########"error"########,
            r########"–– Pkl Error ––
Unexpected end of file.

1 | 
    ^
at repl:text (repl:text)
"########,
        ),
        (
            r########"eof-class-expression"########,
            r########"file:///stage64/amendment-local-controls.pkl"########,
            r########"base = new Dynamic {}
"########,
            r########"class Missing {"########,
            r########"error"########,
            r########"–– Pkl Error ––
Unexpected token `class`.

1 | class Missing {
    ^^^^^
at repl:text (repl:text)
"########,
        ),
        (
            r########"amend-newline"########,
            r########"file:///stage64/amendment-eof-newline.pkl"########,
            r########"base = new Dynamic {}
"########,
            r########"new Dynamic { value = 1 } {
"########,
            r########"error"########,
            r########"–– Pkl Error ––
Missing `}` delimiter.

1 | new Dynamic { value = 1 } {
                               ^
at repl:text (repl:text)
"########,
        ),
        (
            r########"object-newline"########,
            r########"file:///stage64/amendment-eof-newline.pkl"########,
            r########"base = new Dynamic {}
"########,
            r########"new Dynamic {
"########,
            r########"error"########,
            r########"–– Pkl Error ––
Missing `}` delimiter.

1 | new Dynamic {
                 ^
at repl:text (repl:text)
"########,
        ),
        (
            r########"plus-newline"########,
            r########"file:///stage64/amendment-eof-newline.pkl"########,
            r########"base = new Dynamic {}
"########,
            r########"1 +
"########,
            r########"error"########,
            r########"–– Pkl Error ––
Unexpected end of file.

1 | 1 +
       ^
at repl:text (repl:text)
"########,
        ),
    ];
    assert_eq!(rows.len(), 21);
    let mut failures = Vec::new();
    for (id, uri, source, expression, kind, expected) in rows {
        let mut probe = SourceProbe::new_module_evaluator("compact", false, false, false).unwrap();
        probe
            .evaluate_module_source(uri, Some(source), expression, 0, None)
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
        let event = terminal.expect("frontend control poll bound");
        let actual = if event.kind == "value" {
            event.value
        } else {
            event.error
        };
        if event.kind != kind
            || actual.as_ref() != Some(&expected.encode_utf16().collect::<Vec<_>>())
        {
            failures.push(format!(
                "{id}: {expression:?}\nexpected {kind}: {expected}\nactual {}: {}",
                event.kind,
                actual
                    .map(|units| String::from_utf16_lossy(&units))
                    .unwrap_or_default()
            ));
        }
        probe.close();
        assert_eq!(probe.collect().slots, 0, "{id}: evaluator cleanup");
    }
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}
