//! Shebang tokens preserve module grammar and the original Java CLI payloads.
use pkl_runtime::evaluation::frontend::{
    ModuleEvalOptions, ModuleSourceInput, evaluate_module_output,
};
use pkl_runtime::parser::{parse_expression, parse_module};
use pkl_runtime::syntax::ExprKind;

type JavaCase<'a> = (&'a str, &'a str, &'a [u8], &'a [u8], i32);

#[test]
fn original_shebang_modules_match_complete_java_outputs() {
    let cases: &[JavaCase<'_>] = &[
        (
            include_str!("../../../tests/fixtures/m2-shebang/syntax/input.pkl"),
            include_str!("../../../tests/fixtures/m2-shebang/syntax/uri.txt"),
            include_bytes!("../../../tests/fixtures/m2-shebang/syntax/stdout.bin"),
            include_bytes!("../../../tests/fixtures/m2-shebang/syntax/stderr.bin"),
            0,
        ),
        (
            include_str!("../../../tests/fixtures/m2-shebang/errors/input.pkl"),
            include_str!("../../../tests/fixtures/m2-shebang/errors/uri.txt"),
            include_bytes!("../../../tests/fixtures/m2-shebang/errors/stdout.bin"),
            include_bytes!("../../../tests/fixtures/m2-shebang/errors/stderr.bin"),
            1,
        ),
    ];
    for &(source, uri, stdout, stderr, exit_code) in cases {
        let actual = evaluate_module_output(
            ModuleSourceInput {
                text: source,
                uri,
                inferred_name: "shebang",
            },
            &ModuleEvalOptions::default(),
        );
        assert_eq!(actual.exit_code, exit_code, "{uri}");
        assert_eq!(actual.stdout, stdout, "{uri}");
        assert_eq!(actual.stderr, stderr, "{uri}");
    }
}

#[test]
fn shebang_is_accepted_once_before_the_module_header() {
    // Pinned ParserImpl.parseModule consumes SHEBANG after forceNext skips
    // ordinary whitespace/comments; documentation is part of the header.
    for prefix in ["", " \t\n", "// before\n", "/* before */\n"] {
        for newline in ["\n", "\r", "\r\n"] {
            let source = format!("{prefix}#!/usr/bin/env pkl eval{newline}foo = 1{newline}");
            let module = parse_module(&source).unwrap();
            assert_eq!(module.members.len(), 1);
            assert_eq!(module.members[0].name, "foo");
        }
    }
    for source in ["#!", "#!/usr/bin/env pkl eval", "#!\n"] {
        assert!(parse_module(source).unwrap().members.is_empty());
    }
    let module = parse_module("#! executable\n/// Header\nmodule example\nfoo = 1\n").unwrap();
    assert_eq!(module.declaration.unwrap().name.unwrap(), ["example"]);
    for source in [
        "#! first\n#! second\nfoo = 1\n",
        "/// Header\n#! executable\nfoo = 1\n",
        "module example\n#! executable\nfoo = 1\n",
    ] {
        assert!(parse_module(source).is_err(), "{source:?}");
    }
}

#[test]
fn shebang_is_not_a_comment_or_an_expression() {
    for source in ["#! expression\n1", "foo = #! value\n1"] {
        assert!(parse_expression(source).is_err(), "{source:?}");
    }
    assert!(parse_module("foo = new { #! body\nbar = 1 }\n").is_err());
    assert!(parse_module("foo = \"\\(#! interpolation\n1)\"\n").is_err());
    assert!(matches!(
        parse_expression("#\"#! literal\"#").unwrap().kind,
        ExprKind::String(_)
    ));
    assert_eq!(
        parse_module("`#! name` = 1\n").unwrap().members[0].name,
        "#! name"
    );
}
