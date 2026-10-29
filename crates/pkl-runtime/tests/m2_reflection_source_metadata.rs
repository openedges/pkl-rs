//! Declaration source retention checks; Java mirror compatibility is separate.
//! These checks must not execute property or method initializers.
use pkl_runtime::parser::parse_module;
use pkl_runtime::resolver::resolve_module;
use pkl_runtime::syntax::ExprKind;
use pkl_runtime::vm::compile_module;

#[test]
fn comments_keep_exact_sections_across_crlf_and_non_doc_separators() {
    let source = "/// module text🙂\r\nmodule metadata\r\n\
        ///  first\t \r\n// omitted\r\n///\t\tsecond\r\n\
        /* omitted block */\r\n///\x0c\x0cthird\r\nclass C {}\r\n";
    let module = parse_module(source).unwrap();
    let docs = module
        .declaration
        .as_ref()
        .unwrap()
        .doc_comment
        .as_ref()
        .unwrap();
    assert_eq!(docs.text(source).unwrap(), "module text🙂");
    assert_eq!(
        &source[docs.sections[0].start..docs.sections[0].end],
        "/// module text🙂"
    );
    let docs = module.members[0].doc_comment.as_ref().unwrap();
    let raw: Vec<_> = docs
        .sections
        .iter()
        .map(|s| &source[s.start..s.end])
        .collect();
    assert_eq!(raw, ["///  first\t ", "///\t\tsecond", "///\x0c\x0cthird"]);
    assert_eq!(docs.text(source).unwrap(), " first\t \n\tsecond\n\x0cthird");
}

#[test]
fn declaration_headers_and_nested_docs_survive_iterative_expression_clone() {
    let source = "/// first declaration\nopen class C {\n\
        /// property documentation\n hidden value: String = throw(\"must stay lazy\")\n\
        /// method documentation\n function method(x: Int): String = throw(\"must stay lazy\")\n}\n";
    let module = parse_module(source).unwrap();
    assert!(module.declaration.is_none());
    let class = &module.members[0];
    assert_eq!(
        class.doc_comment.as_ref().unwrap().text(source).unwrap(),
        "first declaration"
    );
    let original = match &class.expr.kind {
        ExprKind::Class { members, .. } => members,
        _ => panic!("class declaration"),
    };
    let clone = class.expr.clone();
    let cloned = match &clone.kind {
        ExprKind::Class { members, .. } => members,
        _ => panic!("class declaration clone"),
    };
    assert_eq!(original, cloned);
    for (member, header, docs) in [
        (&cloned[0], "hidden value: String", "property documentation"),
        (
            &cloned[1],
            "function method(x: Int): String",
            "method documentation",
        ),
    ] {
        let metadata = member.declaration_source();
        assert_eq!(
            &source[metadata.header_span.start..metadata.header_span.end],
            header
        );
        assert_eq!(metadata.doc_comment.unwrap().text(source).unwrap(), docs);
    }
    let header = class.declaration_source().header_span;
    assert_eq!(&source[header.start..header.end], "open class C");
}

#[test]
fn compiled_declarations_preserve_source_without_evaluating_initializers() {
    let source = "module metadata\n/// class docs\nclass C {\n\
        /// property docs\n value: String = throw(\"property body\")\n\
        /// method docs\n function method(x: Int): String = throw(\"method body\")\n}\n\
        /// alias docs\ntypealias Names<T> = List<T>\n";
    let parsed = parse_module(source).unwrap();
    let program = compile_module(&resolve_module(&parsed).unwrap()).unwrap();
    for (name, expected_header, expected_doc) in [
        ("C", "class C", "class docs"),
        ("value", "value: String", "property docs"),
        ("method", "function method(x: Int): String", "method docs"),
        ("Names", "typealias Names<T>", "alias docs"),
    ] {
        let found: Vec<_> = program
            .declaration_sources
            .iter()
            .filter(|(_, d)| d.name == name)
            .collect();
        assert_eq!(found.len(), 1, "{name}");
        let (body, metadata) = found[0];
        assert!(*body < program.blocks.len());
        assert_eq!(
            &source[metadata.header_span.start..metadata.header_span.end],
            expected_header
        );
        assert_eq!(
            metadata.doc_comment.as_ref().unwrap().text(source).unwrap(),
            expected_doc
        );
    }
}
