//! Fixed parsed expression entries share the existing module/Host/heap.
//! Alias expected bytes are the unchanged pinned Java SPI acquisition.
use pkl_runtime::{
    probe::{SourceEvent, SourceProbe},
    value::{PklString, Value},
};

fn expression_probe(source: &str, expressions: &[&str], logger: bool) -> SourceProbe {
    SourceProbe::new_expressions(
        source,
        "file:///modules/main.pkl",
        "main",
        &expressions.iter().map(|s| (*s).into()).collect::<Vec<_>>(),
        "compact",
        logger,
        false,
        true,
    )
    .unwrap()
}
fn next(probe: &mut SourceProbe) -> SourceEvent {
    for _ in 0..100_000 {
        let event = probe.poll(0, 1).unwrap();
        probe.collect();
        if event.kind == "module-request"
            && probe
                .complete_default_module(event.token.as_deref().unwrap(), 0)
                .unwrap()
        {
            continue;
        }
        if event.kind != "yield" {
            return event;
        }
    }
    panic!("expression did not settle")
}
fn utf8(units: Vec<u16>) -> Vec<u8> {
    String::from_utf16(&units).unwrap().into_bytes()
}

#[test]
fn parsed_alias_equality_reuses_one_loaded_module_and_preserves_original_observations() {
    let mut probe = expression_probe(
        "canonical=import(\"life:/canonical\")\na=import(\"life:/alias\")\nb=import(\"life:/alias\")\n",
        &["canonical.value", "a == canonical", "b == canonical"],
        false,
    );
    let expected: [&[u8]; 3] = [
        include_bytes!(
            "../../../tests/fixtures/m2-module-loading/origins/stage41-loader-oracle-01/cases/spi-resolved-alias-no-original-insert/0/calls/outer-0.stdout.bin"
        ),
        include_bytes!(
            "../../../tests/fixtures/m2-module-loading/origins/stage41-loader-oracle-01/cases/spi-resolved-alias-no-original-insert/0/calls/outer-1.stdout.bin"
        ),
        include_bytes!(
            "../../../tests/fixtures/m2-module-loading/origins/stage41-loader-oracle-01/cases/spi-resolved-alias-no-original-insert/0/calls/outer-2.stdout.bin"
        ),
    ];
    let (mut resolves, mut loads) = (0, 0);
    for (index, expected) in expected.into_iter().enumerate() {
        probe
            .start_path(index.try_into().unwrap(), 0, None)
            .unwrap();
        loop {
            let event = next(&mut probe);
            if event.kind == "value" {
                assert_eq!(utf8(event.value.unwrap()), expected);
                break;
            }
            assert_eq!(event.kind, "module-request");
            let module = event.module.unwrap();
            let token = event.token.unwrap();
            if module.kind == "resolve" {
                resolves += 1;
                probe
                    .complete_module_resolution(
                        &token,
                        &module.original_uri,
                        "life:/canonical",
                        &module.original_uri,
                        0,
                    )
                    .unwrap();
            } else {
                loads += 1;
                probe
                    .complete_module_source(&token, "value=9\n", "child", 0)
                    .unwrap();
            }
        }
    }
    assert_eq!((resolves, loads), (3, 1));
    probe.close();
    probe.collect();
    assert_eq!(probe.heap_slots(), 0);
}

#[test]
fn synthetic_scope_cannot_expose_local_declarations_but_public_methods_keep_their_owner() {
    let mut probe = expression_probe(
        "local secret=4\npublicValue=7\nfunction plus(x)=x+secret\n",
        &["plus(publicValue)", "secret", "publicValue + 1"],
        false,
    );
    probe.start_path(0, 0, None).unwrap();
    assert_eq!(utf8(next(&mut probe).value.unwrap()), b"11");
    probe.start_path(1, 0, None).unwrap();
    let failed = next(&mut probe);
    assert_eq!(failed.kind, "error");
    let raw = PklString::from_utf16(failed.error.unwrap());
    assert!(raw.to_string().contains("repl:text"));
    probe.start_path(2, 0, None).unwrap();
    assert_eq!(utf8(next(&mut probe).value.unwrap()), b"8");
}

#[test]
fn parsed_expression_callbacks_resume_the_same_deadline_and_receiver() {
    let mut probe = expression_probe(
        "stable=9\nvalue=read(\"life:value\")\n",
        &["value + \"!\"", "stable + 1"],
        false,
    );
    probe.start_path(0, 0, Some(100)).unwrap();
    let request = next(&mut probe);
    assert_eq!(request.kind, "read");
    let token = request.token.unwrap();
    probe.reenter_path(&token, 1, 0, None).unwrap();
    assert_eq!(utf8(next(&mut probe).value.unwrap()), b"10");
    assert_eq!(next(&mut probe).token.as_deref(), Some(token.as_str()));
    probe.respond(&token, Value::String("x".into()), 0).unwrap();
    assert_eq!(utf8(next(&mut probe).value.unwrap()), b"x!");
}

#[test]
fn full_expression_failure_keeps_utf8_offsets_in_its_separate_repl_source() {
    let mut probe = expression_probe("stable=9\n", &["\"é\" + missing", "let (x=3) x * x"], false);
    probe.start_path(0, 0, None).unwrap();
    let failed = next(&mut probe);
    assert_eq!(failed.kind, "error");
    let raw = String::from_utf16(&failed.error.unwrap()).unwrap();
    assert!(raw.contains("1 | \"é\" + missing\n"));
    assert!(raw.contains("at  (repl:text)"));
    assert!(!raw.contains("at main (file:///modules/main.pkl)"));
    probe.start_path(1, 0, None).unwrap();
    assert_eq!(utf8(next(&mut probe).value.unwrap()), b"9");
}

#[test]
fn expression_parser_metadata_and_original_path_boundary_are_both_preserved() {
    let mut probe = expression_probe(
        "stable=9\n",
        &["\"x\\(stable + 1)\"", "((x) -> x + 2).apply(3)"],
        false,
    );
    probe.start_path(0, 0, None).unwrap();
    assert_eq!(utf8(next(&mut probe).value.unwrap()), b"x10");
    probe.start_path(1, 0, None).unwrap();
    assert_eq!(utf8(next(&mut probe).value.unwrap()), b"5");
    assert!(
        SourceProbe::new(
            "stable=9",
            "file:///main.pkl",
            "main",
            &["stable + 1".into()],
            "compact",
            false,
            false,
            false
        )
        .err()
        .unwrap()
        .contains("property paths only")
    );
    let mut path = SourceProbe::new(
        "stable=9",
        "file:///main.pkl",
        "main",
        &["stable".into()],
        "compact",
        false,
        false,
        false,
    )
    .unwrap();
    path.start_path(0, 0, None).unwrap();
    assert_eq!(utf8(next(&mut path).value.unwrap()), b"9");
    let mut typed = expression_probe("class C {}", &["new C {}"], false);
    typed.start_path(0, 0, None).unwrap();
    // Exact unchanged source/expression/URI captured by two fresh pinned Java
    // Stage42 session-type-boundary-oracle-02 evaluators: UTF8 6e65772043207b7d.
    let result = next(&mut typed);
    assert_eq!(result.kind, "value");
    assert_eq!(
        result.value.unwrap(),
        vec![110, 101, 119, 32, 67, 32, 123, 125]
    );
}

#[test]
fn rejected_later_entry_does_not_replace_earlier_calls_or_module_initialization_errors() {
    let mut probe = expression_probe("stable=7", &["stable+1", ")", "stable+2"], false);
    probe.start_path(0, 0, None).unwrap();
    assert_eq!(utf8(next(&mut probe).value.unwrap()), b"8");
    probe.start_path(1, 0, None).unwrap();
    let rejected = next(&mut probe);
    assert_eq!(rejected.phase, Some("parsing"));
    // Pinned Java expression-context-oracle-01/expression-parse-error-between-successes
    // outer-1.stderr.bin SHA256 332628e249ea094247796e301a0a87d68b980ec831977c107f8fc1ba8cde527b.
    assert_eq!(
        String::from_utf16(&rejected.error.unwrap()).unwrap(),
        r####"–– Pkl Error ––
Unexpected token `)`.

1 | )
    ^
at repl:text (repl:text)
"####
    );
    probe.start_path(2, 0, None).unwrap();
    assert_eq!(utf8(next(&mut probe).value.unwrap()), b"9");
    let mut setup = expression_probe(
        "@Deprecated { when(throw(\"setup\")) { message=\"x\" } }\nstable=7",
        &["stable", ")"],
        false,
    );
    setup.start_path(1, 0, None).unwrap();
    let failure = next(&mut setup);
    let raw = String::from_utf16(&failure.error.unwrap()).unwrap();
    assert!(raw.contains("setup"));
    assert!(!raw.contains("Unexpected token"));
}

#[test]
fn expression_import_uses_synthetic_key_while_declaration_imports_keep_their_owner() {
    let mut probe = expression_probe("stable=7", &["import(\"life:/child\").value"], false);
    probe.start_path(0, 0, None).unwrap();
    let request = next(&mut probe);
    assert_eq!(request.kind, "module-request");
    let module = request.module.unwrap();
    assert_eq!(module.importer_original_uri.as_deref(), Some("repl:text"));
    assert_eq!(module.importer_resolved_uri.as_deref(), Some("repl:text"));
    probe
        .complete_module_resolution(
            request.token.as_deref().unwrap(),
            "life:/child",
            "life:/child",
            "life:/child",
            0,
        )
        .unwrap();
    let request = next(&mut probe);
    probe
        .complete_module_source(request.token.as_deref().unwrap(), "value=7", "child", 0)
        .unwrap();
    assert_eq!(utf8(next(&mut probe).value.unwrap()), b"7");
}

#[test]
fn parsed_let_error_retains_the_complete_java_synthetic_source_and_nested_owner() {
    // Java expression-context-oracle-01 receiver-privacy-method-closure outer-6
    // stderr SHA256 960791ca9200a3447e474a55d584435e6ece2c989e1bd991454d332aee5bed3c.
    let mut probe = SourceProbe::new_expressions(
        r####"import "life:/child" as hiddenImport
local secret=11
visible=7
function add(x)=x+visible
function make(x)=(y)->x+y+visible
"####,
        r####"file:///Users/cody.ij.hwang/Projects/pkl-rs.stage41/target/m2/stage41-expression-context-oracle-01/workspace/receiver-privacy-method-closure/main.pkl"####,
        "main", &["let (f=make(3)) f(4)".into()], "compact", false, false, false,
    ).unwrap();
    probe.start_path(0, 0, None).unwrap();
    let error = next(&mut probe);
    assert_eq!(error.kind, "error");
    assert_eq!(
        String::from_utf16(&error.error.unwrap()).unwrap(),
        r####"–– Pkl Error ––
Cannot find method `f`.

1 | let (f=make(3)) f(4)
                    ^^^^
at  (repl:text)

1 | let (f=make(3)) f(4)
    ^^^^^^^^^^^^^^^^^^^^
at repl:text.<let expr> (repl:text)
"####
    );
}
