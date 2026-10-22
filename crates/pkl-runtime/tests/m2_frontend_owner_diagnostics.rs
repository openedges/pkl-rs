//! Rejected object annotations carry authored owners before any value executes.
//! Complete rendered payloads are independently compared with pinned Java.
use pkl_runtime::diagnostic::{Diagnostic, DiagnosticKind};
use pkl_runtime::evaluation::scalar_expression;
use pkl_runtime::parser::parse_module;
use pkl_runtime::resolver::resolve_module;
use pkl_runtime::source::SourceMap;

fn rejection(source: &str) -> Diagnostic {
    resolve_module(&parse_module(source).unwrap()).unwrap_err()
}

fn annotation_owner(source: &str, owner: &str, annotation: &str) {
    let error = rejection(source);
    assert_eq!(error.kind, DiagnosticKind::Syntax);
    assert_eq!(
        error.message,
        "A non-local object property cannot have a type annotation."
    );
    assert_eq!(&source[error.span.start..error.span.end], annotation);
    assert_eq!(
        error
            .member
            .as_deref()
            .unwrap()
            .encode_utf16()
            .collect::<Vec<_>>(),
        owner.encode_utf16().collect::<Vec<_>>(),
        "{source}"
    );
    assert!(error.frames.is_empty());
    assert!(error.hint().is_none());
}

#[test]
fn rejected_annotations_keep_nested_authored_owners() {
    for (source, owner) in [
        (
            "x = new { middle = new { bad: Int = 1 } }\nresult = 1\n",
            "x.middle.bad",
        ),
        (
            "function build() = new Dynamic { bad: Int = 1 }\nresult = 1\n",
            "build.bad",
        ),
        (
            "x = new { local function build() = new Dynamic { bad: Int = 1 } }\nresult = 1\n",
            "x.build.bad",
        ),
        (
            "f = () -> new Dynamic { bad: Int = 1 }\nresult = 1\n",
            "f.<function#1>.bad",
        ),
        (
            "f = () -> (() -> new Dynamic { bad: Int = 1 })\nresult = 1\n",
            "f.<function#2>.bad",
        ),
        (
            "x = new { [\"key\"] = new { bad: Int = 1 } }\nresult = 1\n",
            "x[\"key\"].bad",
        ),
        (
            "x = new { new { bad: Int = 1 } }\nresult = 1\n",
            "x[#1].bad",
        ),
        (
            "x = new { 0; new { bad: Int = 1 } }\nresult = 1\n",
            "x[#2].bad",
        ),
        (
            "x = new { [new Dynamic { bad: Int = 1 }] = 2 }\nresult = 1\n",
            "x.bad",
        ),
        (
            "x = new { for (v in List(1)) { [v] = new Dynamic { bad: Int = v } } }\nresult = 1\n",
            "x[#1].bad",
        ),
        (
            "x = new { 0; for (v in List(1)) { [v] = new Dynamic { bad: Int = 1 } } }\nresult = 1\n",
            "x[#3].bad",
        ),
        (
            "x = new { [1.s] = new Dynamic { bad: Int = 1 } }\nresult = 1\n",
            "x[1.s].bad",
        ),
        (
            "x = new { [1.kb] = new Dynamic { bad: Int = 1 } }\nresult = 1\n",
            "x[1.kb].bad",
        ),
        (
            "x = new { [--1] = new Dynamic { bad: Int = 1 } }\nresult = 1\n",
            "x[#1].bad",
        ),
        (
            "x = new { [-1] = new Dynamic { bad: Int = 1 } }\nresult = 1\n",
            "x[-1].bad",
        ),
        (
            "x = new { [-(1)] = new Dynamic { bad: Int = 1 } }\nresult = 1\n",
            "x[#1].bad",
        ),
        (
            "x = new { [(-1)] = new Dynamic { bad: Int = 1 } }\nresult = 1\n",
            "x[-1].bad",
        ),
        (
            "x = new { [(--1).s] = new Dynamic { bad: Int = 1 } }\nresult = 1\n",
            "x[#1].bad",
        ),
        (
            "f = List(() -> 1, () -> new Dynamic { bad: Int = 1 })\nresult = 1\n",
            "f.<function#2>.bad",
        ),
        (
            "x = new { 0; when (true) { [\"k\"] = new Dynamic { bad: Int = 1 } } }\nresult = 1\n",
            "x[\"k\"].bad",
        ),
        (
            "x = new { when (false) { bad: Int = 1 } }\nresult = 1\n",
            "x.bad",
        ),
        (
            "base = new { middle = new { bad = 1 } }\nx = (base) { middle { bad: Int = 2 } }\nresult = 1\n",
            "x.middle.bad",
        ),
        (
            "x = let (v = 1) new { bad: Int = v }\nresult = 1\n",
            "x.<let expr>.bad",
        ),
        (
            "x = let (v = new Dynamic { bad: Int = 1 }) v\nresult = 1\n",
            "x.bad",
        ),
        (
            "typealias T = Int((new Dynamic { bad: Int = 1 }).bad == this)\nresult: T = 1\n",
            "T.bad",
        ),
    ] {
        annotation_owner(source, owner, "Int");
    }
}

#[test]
fn annotation_rejection_precedes_type_and_value_evaluation() {
    for expression in ["-1", "-1.0", "(-1)", "-(1)"] {
        let source = format!(
            "typealias I = Int(this > 0)\nx = new {{ bad: I = {expression} }}\nresult = x.bad\n"
        );
        annotation_owner(&source, "x.bad", "I");
    }
    annotation_owner(
        "x = new { first: Missing = throw(\"body\"); second: String = \"a\" }\nresult = 1\n",
        "x.first",
        "Missing",
    );
    annotation_owner(
        "function unused() = new Dynamic { bad: Int = throw(\"body\") }\nresult = 1\n",
        "unused.bad",
        "Int",
    );
    for (source, message, fragment) in [
        (
            "x = new { hidden bad: Int = 1 }\n",
            "Modifier `hidden` is not applicable to object members.",
            "hidden",
        ),
        (
            "x = new { const bad: Int = 1 }\n",
            "Modifier `const` can only be applied to object members that are also `local`.",
            "const",
        ),
        (
            "x = new { for (v in List(1)) { bad: Int = v } }\n",
            "A for-generator cannot generate object properties (only entries and elements).",
            "for",
        ),
    ] {
        let error = rejection(source);
        assert_eq!(error.message, message);
        assert_eq!(&source[error.span.start..error.span.end], fragment);
        // The earlier const-modifier owner's existing mismatch remains a
        // separate raw Java observation; this annotation change preserves it.
        if fragment != "const" {
            assert!(
                error
                    .member
                    .as_ref()
                    .unwrap()
                    .encode_utf16()
                    .eq("x".encode_utf16())
            );
        } else {
            assert!(error.member.is_none());
        }
    }
}

#[test]
fn quoted_names_and_supplementary_positions_keep_utf16_identity() {
    for (source, owner) in [
        (
            "`outer name` = new { `bad.name`: Int = 1 }\nresult = 1\n",
            "`outer name`.`bad.name`",
        ),
        ("`😀` = new { `🚀`: Int = 1 }\nresult = 1\n", "`😀`.`🚀`"),
        (
            "x = new { [\"😀\"] = new { bad: Int = 1 } }\nresult = 1\n",
            "x[\"😀\"].bad",
        ),
        (
            "`\\u{D800}` = new { `\\u{D801}`: Int = 1 }\nresult = 1\n",
            "`\\u{D800}`.`\\u{D801}`",
        ),
    ] {
        annotation_owner(source, owner, "Int");
        let error = rejection(source);
        let mut sources = SourceMap::default();
        let id = sources.add("file:///owner.pkl", source);
        let position = sources.get(id).unwrap().position(error.span.start).unwrap();
        assert_eq!(
            position.column_utf16,
            source[..error.span.start].encode_utf16().count() + 1
        );
        assert_eq!(
            sources.get(id).unwrap().utf16_span(error.span).unwrap().1,
            3
        );
    }
    let source = "x = new { [\"\\u{D800}\"] = new { bad: Int = 1 } }\nresult = 1\n";
    let error = rejection(source);
    let mut expected = "x[\"".encode_utf16().collect::<Vec<_>>();
    expected.push(0xd800);
    expected.extend("\"].bad".encode_utf16());
    assert_eq!(
        error.member.unwrap().encode_utf16().collect::<Vec<_>>(),
        expected
    );
}

#[test]
fn module_class_and_local_annotations_remain_accepted_and_lazy() {
    for (source, expected) in [
        ("x: Int = 1\nresult = x\n", "1"),
        ("class C { x: Int = 1 }\nresult = new C {}.x\n", "1"),
        (
            "x = new { local good: Int = 1; value = good }\nresult = x.value\n",
            "1",
        ),
        (
            "x = new { local unused: Int = throw(\"unused\"); value = 2 }\nresult = x.value\n",
            "2",
        ),
    ] {
        assert_eq!(
            scalar_expression(source, "file:///owner.pkl", "owner", "result"),
            Ok(expected.to_owned())
        );
    }
}
