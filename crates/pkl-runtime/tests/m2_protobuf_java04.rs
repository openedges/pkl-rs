//! Exact Java04 output fixtures; authored source and expected bytes are pinned.
//! Source-only regression preparation. No Rust run is claimed.
use pkl_runtime::evaluation::frontend::{
    ModuleEvalOptions, ModuleSourceInput, evaluate_module_output,
};
macro_rules! case {
    ($name:ident, $id:literal) => {
        #[test]
        fn $name() {
            let result = evaluate_module_output(
                ModuleSourceInput {
                    text: include_str!(concat!(
                        "../../../tests/fixtures/m2-protobuf-java04/",
                        $id,
                        "/module.pkl"
                    )),
                    uri: concat!("file:///stage145-protobuf/", $id, "/main.pkl"),
                    inferred_name: "main",
                },
                &ModuleEvalOptions::default(),
            );
            assert_eq!(
                result.exit_code,
                0,
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
            assert_eq!(
                result.stdout,
                include_bytes!(concat!(
                    "../../../tests/fixtures/m2-protobuf-java04/",
                    $id,
                    "/on.result.stdout.bin"
                ))
            );
            assert_eq!(
                result.stderr,
                include_bytes!(concat!(
                    "../../../tests/fixtures/m2-protobuf-java04/",
                    $id,
                    "/on.result.stderr.bin"
                ))
            );
        }
    };
}
case!(sort_same_final, "sort-same-final");
case!(sort_same_open, "sort-same-open");
case!(sort_alias_same_open, "sort-alias-same-open");
case!(sort_alias_same_child, "sort-alias-same-child");
case!(sort_long_mixed_40, "sort-long-mixed-40");
case!(sort_long_mixed_alias_64, "sort-long-mixed-alias-64");
case!(subclass_base_first, "subclass-base-first");
case!(subclass_derived_first, "subclass-derived-first");
case!(union_int, "union-int");
case!(union_string, "union-string");
case!(union_boolean, "union-boolean");
case!(string_union_nested, "string-union-nested");
case!(collection_union_list, "collection-union-list");
case!(collection_union_alias_list, "collection-union-alias-list");
case!(nested_list, "nested-list");
case!(nested_map, "nested-map");
case!(nested_mixed, "nested-mixed");
case!(nullable_direct, "nullable-direct");
case!(nullable_after_conversion, "nullable-after-conversion");
case!(rename_union_absent, "rename-union-absent");
case!(rename_union_other, "rename-union-other");
case!(render_type_declared, "render-type-declared");
case!(render_type_nullable, "render-type-nullable");
case!(render_type_union, "render-type-union");
case!(render_type_literal, "render-type-literal");
case!(primitive_negative_zero, "primitive-negative-zero");
case!(primitive_int_min, "primitive-int-min");
case!(string_escapes, "string-escapes");
case!(indent_empty, "indent-empty");
case!(duration_negative, "duration-negative");
case!(duration_huge, "duration-huge");
case!(duration_negative_huge, "duration-negative-huge");
