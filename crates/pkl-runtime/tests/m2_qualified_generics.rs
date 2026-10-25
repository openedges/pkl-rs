//! Full independent Java qualified-generic values/errors, including retained known boundaries.
#[path = "support/module_keys.rs"]
mod module_keys;
use pkl_runtime::{probe::SourceProbe, value::PklString};
struct Row {
    id: &'static str,
    expression: &'static str,
    kind: &'static str,
    utf8: &'static [u8],
    utf16: &'static [u8],
}
struct Program {
    id: &'static str,
    uri: &'static str,
    source: &'static str,
    rows: &'static [Row],
}
fn units(bytes: &[u8]) -> Vec<u16> {
    let (pairs, tail) = bytes.as_chunks::<2>();
    assert!(tail.is_empty());
    pairs.iter().map(|pair| u16::from_be_bytes(*pair)).collect()
}
#[test]
fn java_qualified_generic_arity_argument_order_and_erasure() {
    let programs = &[
        Program {
            id: "builtin",
            uri: "file:///stage70/qualified-generics.pkl",
            source: include_str!("../../../tests/fixtures/m2-qualified-generics/00-builtin.pkl"),
            rows: &[
                Row {
                    id: "list-int-yes",
                    expression: "List(1) is b.List<Int>",
                    kind: "value",
                    utf8: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/000-list-int-yes.utf8.bin"
                    ),
                    utf16: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/000-list-int-yes.utf16be.bin"
                    ),
                },
                Row {
                    id: "list-int-no",
                    expression: "List(\"x\") is b.List<Int>",
                    kind: "value",
                    utf8: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/001-list-int-no.utf8.bin"
                    ),
                    utf16: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/001-list-int-no.utf16be.bin"
                    ),
                },
                Row {
                    id: "list-wrong-many",
                    expression: "List(1) is b.List<Int, Missing>",
                    kind: "error",
                    utf8: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/002-list-wrong-many.utf8.bin"
                    ),
                    utf16: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/002-list-wrong-many.utf16be.bin"
                    ),
                },
                Row {
                    id: "list-valid-missing",
                    expression: "List(1) is b.List<Missing>",
                    kind: "error",
                    utf8: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/003-list-valid-missing.utf8.bin"
                    ),
                    utf16: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/003-list-valid-missing.utf16be.bin"
                    ),
                },
                Row {
                    id: "set-int-yes",
                    expression: "Set(1) is b.Set<Int>",
                    kind: "value",
                    utf8: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/004-set-int-yes.utf8.bin"
                    ),
                    utf16: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/004-set-int-yes.utf16be.bin"
                    ),
                },
                Row {
                    id: "set-int-no",
                    expression: "Set(\"x\") is b.Set<Int>",
                    kind: "value",
                    utf8: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/005-set-int-no.utf8.bin"
                    ),
                    utf16: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/005-set-int-no.utf16be.bin"
                    ),
                },
                Row {
                    id: "set-wrong-many",
                    expression: "Set(1) is b.Set<Int, Missing>",
                    kind: "error",
                    utf8: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/006-set-wrong-many.utf8.bin"
                    ),
                    utf16: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/006-set-wrong-many.utf16be.bin"
                    ),
                },
                Row {
                    id: "listing-int-yes",
                    expression: "(new Listing { 1 }) is b.Listing<Int>",
                    kind: "value",
                    utf8: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/007-listing-int-yes.utf8.bin"
                    ),
                    utf16: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/007-listing-int-yes.utf16be.bin"
                    ),
                },
                Row {
                    id: "listing-int-no",
                    expression: "(new Listing { \"x\" }) is b.Listing<Int>",
                    kind: "value",
                    utf8: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/008-listing-int-no.utf8.bin"
                    ),
                    utf16: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/008-listing-int-no.utf16be.bin"
                    ),
                },
                Row {
                    id: "listing-wrong-many",
                    expression: "(new Listing { 1 }) is b.Listing<Int, Missing>",
                    kind: "error",
                    utf8: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/009-listing-wrong-many.utf8.bin"
                    ),
                    utf16: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/009-listing-wrong-many.utf16be.bin"
                    ),
                },
                Row {
                    id: "collection-int-yes",
                    expression: "List(1) is b.Collection<Int>",
                    kind: "value",
                    utf8: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/010-collection-int-yes.utf8.bin"
                    ),
                    utf16: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/010-collection-int-yes.utf16be.bin"
                    ),
                },
                Row {
                    id: "collection-int-no",
                    expression: "List(\"x\") is b.Collection<Int>",
                    kind: "value",
                    utf8: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/011-collection-int-no.utf8.bin"
                    ),
                    utf16: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/011-collection-int-no.utf16be.bin"
                    ),
                },
                Row {
                    id: "collection-wrong-many",
                    expression: "List(1) is b.Collection<Int, Missing>",
                    kind: "error",
                    utf8: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/012-collection-wrong-many.utf8.bin"
                    ),
                    utf16: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/012-collection-wrong-many.utf16be.bin"
                    ),
                },
                Row {
                    id: "map-yes",
                    expression: "Map(\"a\", 1) is b.Map<String, Int>",
                    kind: "value",
                    utf8: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/013-map-yes.utf8.bin"
                    ),
                    utf16: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/013-map-yes.utf16be.bin"
                    ),
                },
                Row {
                    id: "map-no",
                    expression: "Map(\"a\", \"x\") is b.Map<String, Int>",
                    kind: "value",
                    utf8: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/014-map-no.utf8.bin"
                    ),
                    utf16: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/014-map-no.utf16be.bin"
                    ),
                },
                Row {
                    id: "map-wrong-few",
                    expression: "Map(\"a\", 1) is b.Map<Missing>",
                    kind: "error",
                    utf8: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/015-map-wrong-few.utf8.bin"
                    ),
                    utf16: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/015-map-wrong-few.utf16be.bin"
                    ),
                },
                Row {
                    id: "map-valid-missing",
                    expression: "Map(\"a\", 1) is b.Map<Missing, Int>",
                    kind: "error",
                    utf8: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/016-map-valid-missing.utf8.bin"
                    ),
                    utf16: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/016-map-valid-missing.utf16be.bin"
                    ),
                },
                Row {
                    id: "mapping-yes",
                    expression: "(new Mapping { [\"a\"] = 1 }) is b.Mapping<String, Int>",
                    kind: "value",
                    utf8: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/017-mapping-yes.utf8.bin"
                    ),
                    utf16: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/017-mapping-yes.utf16be.bin"
                    ),
                },
                Row {
                    id: "mapping-no",
                    expression: "(new Mapping { [\"a\"] = \"x\" }) is b.Mapping<String, Int>",
                    kind: "value",
                    utf8: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/018-mapping-no.utf8.bin"
                    ),
                    utf16: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/018-mapping-no.utf16be.bin"
                    ),
                },
                Row {
                    id: "mapping-wrong-few",
                    expression: "(new Mapping { [\"a\"] = 1 }) is b.Mapping<Missing>",
                    kind: "error",
                    utf8: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/019-mapping-wrong-few.utf8.bin"
                    ),
                    utf16: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/019-mapping-wrong-few.utf16be.bin"
                    ),
                },
                Row {
                    id: "pair-yes",
                    expression: "Pair(\"a\", 1) is b.Pair<String, Int>",
                    kind: "value",
                    utf8: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/020-pair-yes.utf8.bin"
                    ),
                    utf16: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/020-pair-yes.utf16be.bin"
                    ),
                },
                Row {
                    id: "pair-no",
                    expression: "Pair(\"a\", \"x\") is b.Pair<String, Int>",
                    kind: "value",
                    utf8: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/021-pair-no.utf8.bin"
                    ),
                    utf16: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/021-pair-no.utf16be.bin"
                    ),
                },
                Row {
                    id: "pair-wrong-few",
                    expression: "Pair(\"a\", 1) is b.Pair<Missing>",
                    kind: "error",
                    utf8: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/022-pair-wrong-few.utf8.bin"
                    ),
                    utf16: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/022-pair-wrong-few.utf16be.bin"
                    ),
                },
                Row {
                    id: "class-erased-yes",
                    expression: "Int is b.Class<Missing>",
                    kind: "value",
                    utf8: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/023-class-erased-yes.utf8.bin"
                    ),
                    utf16: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/023-class-erased-yes.utf16be.bin"
                    ),
                },
                Row {
                    id: "class-erased-no",
                    expression: "1 is b.Class<Missing>",
                    kind: "value",
                    utf8: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/024-class-erased-no.utf8.bin"
                    ),
                    utf16: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/024-class-erased-no.utf16be.bin"
                    ),
                },
                Row {
                    id: "class-wrong-many",
                    expression: "Int is b.Class<Missing, OtherMissing>",
                    kind: "error",
                    utf8: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/025-class-wrong-many.utf8.bin"
                    ),
                    utf16: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/025-class-wrong-many.utf16be.bin"
                    ),
                },
                Row {
                    id: "class-wrong-few",
                    expression: "Int is b.Class<>",
                    kind: "error",
                    utf8: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/026-class-wrong-few.utf8.bin"
                    ),
                    utf16: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/026-class-wrong-few.utf16be.bin"
                    ),
                },
                Row {
                    id: "function-result-yes",
                    expression: "((x) -> 1) is b.Function<Int>",
                    kind: "value",
                    utf8: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/027-function-result-yes.utf8.bin"
                    ),
                    utf16: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/027-function-result-yes.utf16be.bin"
                    ),
                },
                Row {
                    id: "function-result-no",
                    expression: "1 is b.Function<Int>",
                    kind: "value",
                    utf8: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/028-function-result-no.utf8.bin"
                    ),
                    utf16: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/028-function-result-no.utf16be.bin"
                    ),
                },
                Row {
                    id: "function-wrong-many",
                    expression: "((x) -> 1) is b.Function<Int, Missing>",
                    kind: "error",
                    utf8: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/029-function-wrong-many.utf8.bin"
                    ),
                    utf16: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/029-function-wrong-many.utf16be.bin"
                    ),
                },
                Row {
                    id: "function-valid-missing",
                    expression: "((x) -> 1) is b.Function<Missing>",
                    kind: "error",
                    utf8: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/030-function-valid-missing.utf8.bin"
                    ),
                    utf16: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/030-function-valid-missing.utf16be.bin"
                    ),
                },
                Row {
                    id: "varargs-valid",
                    expression: "1 is b.VarArgs<Int>",
                    kind: "error",
                    utf8: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/031-varargs-valid.utf8.bin"
                    ),
                    utf16: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/031-varargs-valid.utf16be.bin"
                    ),
                },
                Row {
                    id: "varargs-wrong-many",
                    expression: "1 is b.VarArgs<Int, Missing>",
                    kind: "error",
                    utf8: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/032-varargs-wrong-many.utf8.bin"
                    ),
                    utf16: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/032-varargs-wrong-many.utf16be.bin"
                    ),
                },
                Row {
                    id: "missing-type-before-arg-again",
                    expression: "1 is b.NoSuchType<Missing>",
                    kind: "error",
                    utf8: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/033-missing-type-before-arg-again.utf8.bin"
                    ),
                    utf16: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/033-missing-type-before-arg-again.utf16be.bin"
                    ),
                },
                Row {
                    id: "function0-yes",
                    expression: "(() -> 1) is b.Function0<Int>",
                    kind: "value",
                    utf8: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/034-function0-yes.utf8.bin"
                    ),
                    utf16: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/034-function0-yes.utf16be.bin"
                    ),
                },
                Row {
                    id: "function0-no",
                    expression: "1 is b.Function0<Int>",
                    kind: "value",
                    utf8: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/035-function0-no.utf8.bin"
                    ),
                    utf16: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/035-function0-no.utf16be.bin"
                    ),
                },
                Row {
                    id: "function0-wrong",
                    expression: "(() -> 1) is b.Function0<Int, Missing>",
                    kind: "error",
                    utf8: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/036-function0-wrong.utf8.bin"
                    ),
                    utf16: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/036-function0-wrong.utf16be.bin"
                    ),
                },
                Row {
                    id: "function1-yes",
                    expression: "((x0) -> 1) is b.Function1<Int, Int>",
                    kind: "value",
                    utf8: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/037-function1-yes.utf8.bin"
                    ),
                    utf16: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/037-function1-yes.utf16be.bin"
                    ),
                },
                Row {
                    id: "function1-no",
                    expression: "1 is b.Function1<Int, Int>",
                    kind: "value",
                    utf8: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/038-function1-no.utf8.bin"
                    ),
                    utf16: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/038-function1-no.utf16be.bin"
                    ),
                },
                Row {
                    id: "function1-wrong",
                    expression: "((x0) -> 1) is b.Function1<Missing>",
                    kind: "error",
                    utf8: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/039-function1-wrong.utf8.bin"
                    ),
                    utf16: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/039-function1-wrong.utf16be.bin"
                    ),
                },
                Row {
                    id: "function2-yes",
                    expression: "((x0, x1) -> 1) is b.Function2<Int, Int, Int>",
                    kind: "value",
                    utf8: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/040-function2-yes.utf8.bin"
                    ),
                    utf16: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/040-function2-yes.utf16be.bin"
                    ),
                },
                Row {
                    id: "function2-no",
                    expression: "1 is b.Function2<Int, Int, Int>",
                    kind: "value",
                    utf8: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/041-function2-no.utf8.bin"
                    ),
                    utf16: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/041-function2-no.utf16be.bin"
                    ),
                },
                Row {
                    id: "function2-wrong",
                    expression: "((x0, x1) -> 1) is b.Function2<Missing>",
                    kind: "error",
                    utf8: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/042-function2-wrong.utf8.bin"
                    ),
                    utf16: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/042-function2-wrong.utf16be.bin"
                    ),
                },
                Row {
                    id: "function3-yes",
                    expression: "((x0, x1, x2) -> 1) is b.Function3<Int, Int, Int, Int>",
                    kind: "value",
                    utf8: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/043-function3-yes.utf8.bin"
                    ),
                    utf16: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/043-function3-yes.utf16be.bin"
                    ),
                },
                Row {
                    id: "function3-no",
                    expression: "1 is b.Function3<Int, Int, Int, Int>",
                    kind: "value",
                    utf8: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/044-function3-no.utf8.bin"
                    ),
                    utf16: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/044-function3-no.utf16be.bin"
                    ),
                },
                Row {
                    id: "function3-wrong",
                    expression: "((x0, x1, x2) -> 1) is b.Function3<Missing>",
                    kind: "error",
                    utf8: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/045-function3-wrong.utf8.bin"
                    ),
                    utf16: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/045-function3-wrong.utf16be.bin"
                    ),
                },
                Row {
                    id: "function4-yes",
                    expression: "((x0, x1, x2, x3) -> 1) is b.Function4<Int, Int, Int, Int, Int>",
                    kind: "value",
                    utf8: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/046-function4-yes.utf8.bin"
                    ),
                    utf16: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/046-function4-yes.utf16be.bin"
                    ),
                },
                Row {
                    id: "function4-no",
                    expression: "1 is b.Function4<Int, Int, Int, Int, Int>",
                    kind: "value",
                    utf8: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/047-function4-no.utf8.bin"
                    ),
                    utf16: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/047-function4-no.utf16be.bin"
                    ),
                },
                Row {
                    id: "function4-wrong",
                    expression: "((x0, x1, x2, x3) -> 1) is b.Function4<Missing>",
                    kind: "error",
                    utf8: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/048-function4-wrong.utf8.bin"
                    ),
                    utf16: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/048-function4-wrong.utf16be.bin"
                    ),
                },
                Row {
                    id: "function5-yes",
                    expression: "((x0, x1, x2, x3, x4) -> 1) is b.Function5<Int, Int, Int, Int, Int, Int>",
                    kind: "value",
                    utf8: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/049-function5-yes.utf8.bin"
                    ),
                    utf16: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/049-function5-yes.utf16be.bin"
                    ),
                },
                Row {
                    id: "function5-no",
                    expression: "1 is b.Function5<Int, Int, Int, Int, Int, Int>",
                    kind: "value",
                    utf8: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/050-function5-no.utf8.bin"
                    ),
                    utf16: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/050-function5-no.utf16be.bin"
                    ),
                },
                Row {
                    id: "function5-wrong",
                    expression: "((x0, x1, x2, x3, x4) -> 1) is b.Function5<Missing>",
                    kind: "error",
                    utf8: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/051-function5-wrong.utf8.bin"
                    ),
                    utf16: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/051-function5-wrong.utf16be.bin"
                    ),
                },
            ],
        },
        Program {
            id: "aliases",
            uri: "file:///stage70/qualified-generics.pkl",
            source: include_str!("../../../tests/fixtures/m2-qualified-generics/01-aliases.pkl"),
            rows: &[
                Row {
                    id: "alias-list-yes",
                    expression: "List(1) is L<Int>",
                    kind: "value",
                    utf8: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/052-alias-list-yes.utf8.bin"
                    ),
                    utf16: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/052-alias-list-yes.utf16be.bin"
                    ),
                },
                Row {
                    id: "alias-list-no",
                    expression: "List(\"x\") is L<Int>",
                    kind: "value",
                    utf8: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/053-alias-list-no.utf8.bin"
                    ),
                    utf16: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/053-alias-list-no.utf16be.bin"
                    ),
                },
                Row {
                    id: "alias-list-wrong-many",
                    expression: "List(1) is L<Int, Missing>",
                    kind: "error",
                    utf8: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/054-alias-list-wrong-many.utf8.bin"
                    ),
                    utf16: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/054-alias-list-wrong-many.utf16be.bin"
                    ),
                },
                Row {
                    id: "alias-list-valid-missing",
                    expression: "List(1) is L<Missing>",
                    kind: "error",
                    utf8: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/055-alias-list-valid-missing.utf8.bin"
                    ),
                    utf16: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/055-alias-list-valid-missing.utf16be.bin"
                    ),
                },
                Row {
                    id: "alias-class-erasure",
                    expression: "Int is C<Missing>",
                    kind: "error",
                    utf8: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/056-alias-class-erasure.utf8.bin"
                    ),
                    utf16: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/056-alias-class-erasure.utf16be.bin"
                    ),
                },
                Row {
                    id: "alias-class-wrong-many",
                    expression: "Int is C<Missing, OtherMissing>",
                    kind: "error",
                    utf8: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/057-alias-class-wrong-many.utf8.bin"
                    ),
                    utf16: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/057-alias-class-wrong-many.utf16be.bin"
                    ),
                },
                Row {
                    id: "alias-nullable",
                    expression: "null is L<Int>?",
                    kind: "value",
                    utf8: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/058-alias-nullable.utf8.bin"
                    ),
                    utf16: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/058-alias-nullable.utf16be.bin"
                    ),
                },
                Row {
                    id: "alias-constrained",
                    expression: "List(1) is L<Int>(length == 1)",
                    kind: "value",
                    utf8: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/059-alias-constrained.utf8.bin"
                    ),
                    utf16: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/059-alias-constrained.utf16be.bin"
                    ),
                },
            ],
        },
        Program {
            id: "user-shadow",
            uri: "file:///stage70/qualified-generics.pkl",
            source: include_str!(
                "../../../tests/fixtures/m2-qualified-generics/02-user-shadow.pkl"
            ),
            rows: &[
                Row {
                    id: "user-list-yes",
                    expression: "(new List {}) is List",
                    kind: "value",
                    utf8: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/060-user-list-yes.utf8.bin"
                    ),
                    utf16: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/060-user-list-yes.utf16be.bin"
                    ),
                },
                Row {
                    id: "user-list-not-builtin",
                    expression: "(new List {}) is b.List<Int>",
                    kind: "value",
                    utf8: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/061-user-list-not-builtin.utf8.bin"
                    ),
                    utf16: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/061-user-list-not-builtin.utf16be.bin"
                    ),
                },
                Row {
                    id: "builtin-list-still-qualified",
                    expression: "List(1) is b.List<Int>",
                    kind: "value",
                    utf8: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/062-builtin-list-still-qualified.utf8.bin"
                    ),
                    utf16: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/062-builtin-list-still-qualified.utf16be.bin"
                    ),
                },
                Row {
                    id: "user-list-param",
                    expression: "(new List {}) is List<Missing>",
                    kind: "error",
                    utf8: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/063-user-list-param.utf8.bin"
                    ),
                    utf16: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/063-user-list-param.utf16be.bin"
                    ),
                },
            ],
        },
        Program {
            id: "reference",
            uri: "file:///stage70/qualified-generics.pkl",
            source: include_str!("../../../tests/fixtures/m2-qualified-generics/03-reference.pkl"),
            rows: &[
                Row {
                    id: "reference-yes",
                    expression: "ref.Reference(d, Int, null) is ref.Reference<D, Int>",
                    kind: "error",
                    utf8: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/064-reference-yes.utf8.bin"
                    ),
                    utf16: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/064-reference-yes.utf16be.bin"
                    ),
                },
                Row {
                    id: "reference-no",
                    expression: "1 is ref.Reference<D, Int>",
                    kind: "value",
                    utf8: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/065-reference-no.utf8.bin"
                    ),
                    utf16: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/065-reference-no.utf16be.bin"
                    ),
                },
                Row {
                    id: "reference-wrong-few",
                    expression: "ref.Reference(d, Int, null) is ref.Reference<Missing>",
                    kind: "error",
                    utf8: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/066-reference-wrong-few.utf8.bin"
                    ),
                    utf16: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/066-reference-wrong-few.utf16be.bin"
                    ),
                },
                Row {
                    id: "reference-valid-missing",
                    expression: "ref.Reference(d, Int, null) is ref.Reference<D, Missing>",
                    kind: "error",
                    utf8: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/067-reference-valid-missing.utf8.bin"
                    ),
                    utf16: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/067-reference-valid-missing.utf16be.bin"
                    ),
                },
                Row {
                    id: "reference-constrained",
                    expression: "ref.Reference(d, Int, null) is ref.Reference<D, Int(isPositive)>",
                    kind: "error",
                    utf8: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/068-reference-constrained.utf8.bin"
                    ),
                    utf16: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/068-reference-constrained.utf16be.bin"
                    ),
                },
            ],
        },
        Program {
            id: "qualifier-order",
            uri: "file:///stage70/qualified-generics.pkl",
            source: include_str!(
                "../../../tests/fixtures/m2-qualified-generics/04-qualifier-order.pkl"
            ),
            rows: &[
                Row {
                    id: "absent-before-arg",
                    expression: "1 is absent.List<Missing>",
                    kind: "error",
                    utf8: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/069-absent-before-arg.utf8.bin"
                    ),
                    utf16: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/069-absent-before-arg.utf16be.bin"
                    ),
                },
                Row {
                    id: "local-before-arg",
                    expression: "1 is localBase.List<Missing>",
                    kind: "error",
                    utf8: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/070-local-before-arg.utf8.bin"
                    ),
                    utf16: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/070-local-before-arg.utf16be.bin"
                    ),
                },
                Row {
                    id: "nontype-before-arg",
                    expression: "1 is math.pi<Missing>",
                    kind: "error",
                    utf8: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/071-nontype-before-arg.utf8.bin"
                    ),
                    utf16: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/071-nontype-before-arg.utf16be.bin"
                    ),
                },
                Row {
                    id: "missing-type-before-arg",
                    expression: "1 is b.NoSuchType<Missing>",
                    kind: "error",
                    utf8: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/072-missing-type-before-arg.utf8.bin"
                    ),
                    utf16: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/072-missing-type-before-arg.utf16be.bin"
                    ),
                },
            ],
        },
        Program {
            id: "module-reference",
            uri: "file:///stage70/qualified-generics.pkl",
            source: include_str!(
                "../../../tests/fixtures/m2-qualified-generics/05-module-reference.pkl"
            ),
            rows: &[
                Row {
                    id: "module-reference-value",
                    expression: "referenceValue",
                    kind: "value",
                    utf8: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/073-module-reference-value.utf8.bin"
                    ),
                    utf16: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/073-module-reference-value.utf16be.bin"
                    ),
                },
                Row {
                    id: "module-reference-yes",
                    expression: "referenceValue is ref.Reference<D, Int>",
                    kind: "value",
                    utf8: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/074-module-reference-yes.utf8.bin"
                    ),
                    utf16: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/074-module-reference-yes.utf16be.bin"
                    ),
                },
                Row {
                    id: "module-reference-no",
                    expression: "referenceValue is ref.Reference<D, String>",
                    kind: "value",
                    utf8: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/075-module-reference-no.utf8.bin"
                    ),
                    utf16: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/075-module-reference-no.utf16be.bin"
                    ),
                },
                Row {
                    id: "module-reference-domain-any",
                    expression: "referenceValue is ref.Reference<Any, Int>",
                    kind: "value",
                    utf8: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/076-module-reference-domain-any.utf8.bin"
                    ),
                    utf16: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/076-module-reference-domain-any.utf16be.bin"
                    ),
                },
                Row {
                    id: "module-reference-wrong-few",
                    expression: "referenceValue is ref.Reference<Missing>",
                    kind: "error",
                    utf8: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/077-module-reference-wrong-few.utf8.bin"
                    ),
                    utf16: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/077-module-reference-wrong-few.utf16be.bin"
                    ),
                },
                Row {
                    id: "module-reference-valid-missing",
                    expression: "referenceValue is ref.Reference<D, Missing>",
                    kind: "error",
                    utf8: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/078-module-reference-valid-missing.utf8.bin"
                    ),
                    utf16: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/078-module-reference-valid-missing.utf16be.bin"
                    ),
                },
                Row {
                    id: "module-reference-constrained",
                    expression: "referenceValue is ref.Reference<D, Int(isPositive)>",
                    kind: "error",
                    utf8: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/079-module-reference-constrained.utf8.bin"
                    ),
                    utf16: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/079-module-reference-constrained.utf16be.bin"
                    ),
                },
                Row {
                    id: "module-reference-path",
                    expression: "referenceValue.getPath().length",
                    kind: "value",
                    utf8: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/080-module-reference-path.utf8.bin"
                    ),
                    utf16: include_bytes!(
                        "../../../tests/fixtures/m2-qualified-generics/080-module-reference-path.utf16be.bin"
                    ),
                },
            ],
        },
    ];
    let mut failures = Vec::new();
    for program in programs {
        for repeat in 0..2 {
            let mut probe =
                SourceProbe::new_module_evaluator("compact", true, false, false).unwrap();
            for row in program.rows {
                probe
                    .evaluate_module_source(
                        program.uri,
                        Some(program.source),
                        row.expression,
                        0,
                        None,
                    )
                    .unwrap();
                let mut terminal = None;
                let mut logs = 0;
                for _ in 0..200_000 {
                    probe.collect();
                    let event = probe.poll(0, 16).unwrap();
                    probe.collect();
                    if event.kind == "yield" {
                        continue;
                    }
                    if event.kind == "module-request"
                        && module_keys::complete_probe(
                            &mut probe,
                            event.token.as_deref().unwrap(),
                            0,
                        )
                        .unwrap()
                    {
                        continue;
                    }
                    if event.kind == "log" {
                        logs += 1;
                        probe
                            .acknowledge_log(event.token.as_deref().unwrap(), None, 0)
                            .unwrap();
                        continue;
                    }
                    terminal = Some(event);
                    break;
                }
                let event = terminal.expect("bounded qualified-generic evaluation");
                let actual = if event.kind == "value" {
                    event.value.as_ref()
                } else {
                    event.error.as_ref()
                };
                let expected = units(row.utf16);
                if event.kind != row.kind
                    || logs != 0
                    || actual.is_none_or(|value| {
                        *value != expected
                            || PklString::from_utf16(value.clone())
                                .to_java_utf8()
                                .as_bytes()
                                != row.utf8
                    })
                {
                    failures.push(format!(
                        "{} / {} repeat{}: expected {} {:?}, got {} {:?}, logs{}",
                        program.id,
                        row.id,
                        repeat,
                        row.kind,
                        String::from_utf16_lossy(&expected),
                        event.kind,
                        actual.map(|v| String::from_utf16_lossy(v)),
                        logs
                    ));
                }
            }
            probe.close();
            assert_eq!(probe.collect().slots, 0, "{} close", program.id);
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
