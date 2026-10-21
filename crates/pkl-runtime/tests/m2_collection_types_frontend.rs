//! Collection type syntax and scopes; runtime and oracle evidence are separate.
use pkl_runtime::parser::parse_module;
use pkl_runtime::resolver::{Binding, resolve_module};
use pkl_runtime::source::Span;
use pkl_runtime::syntax::{ExprKind, Type, TypeKind};

fn named<'a>(ty: &'a Type, expected: &str) -> Vec<&'a Type> {
    let TypeKind::Named {
        path, arguments, ..
    } = &ty.kind
    else {
        panic!("expected named type {expected}")
    };
    assert_eq!(path, &[expected]);
    arguments.iter().collect()
}

fn span(source: &str, fragment: &str) -> Span {
    let start = source.find(fragment).unwrap();
    Span::new(start, start + fragment.len())
}

#[test]
fn nested_collection_arguments_keep_alias_parameters_and_authored_type_spans() {
    let source =
        "typealias Rows<K, V> = List<Mapping<K, Listing<V?>>>\nresult: Rows<String, Int> = List()";
    let module = parse_module(source).unwrap();
    let resolved = resolve_module(&module).unwrap();
    let ExprKind::TypeAlias { ty, .. } = &module.members[0].expr.kind else {
        panic!("alias")
    };
    assert_eq!(
        &source[ty.span.start..ty.span.end],
        "List<Mapping<K, Listing<V?>>>"
    );
    let mapping = named(ty, "List")[0];
    let arguments = named(mapping, "Mapping");
    let key_start = span(source, "K, Listing").start;
    assert_eq!(arguments[0].span, Span::new(key_start, key_start + 1));
    let listing = named(arguments[1], "Listing")[0];
    let TypeKind::Nullable(value) = &listing.kind else {
        panic!("nullable value")
    };
    named(value, "V");
    assert!(!resolved.names.contains_key(&arguments[0].span));
    assert!(!resolved.names.contains_key(&value.span));
    let use_site = module.members[1].ty.as_ref().unwrap();
    assert_eq!(
        resolved.names[&use_site.span],
        Binding::Property {
            owner_depth: 0,
            local: false
        }
    );
    assert_eq!(named(use_site, "Rows").len(), 2);
}

#[test]
fn collection_argument_type_names_ignore_method_parameters_but_constraints_capture_them() {
    let source = "class Item {}\nfunction check(Item, bound, value: Mapping<Item, Listing<Int(this > bound)>>) = value";
    let module = parse_module(source).unwrap();
    let resolved = resolve_module(&module).unwrap();
    let ExprKind::Function { parameters, .. } = &module.members[1].expr.kind else {
        panic!("method")
    };
    let arguments = named(parameters[2].ty.as_ref().unwrap(), "Mapping");
    assert_eq!(
        resolved.names[&arguments[0].span],
        Binding::Property {
            owner_depth: 0,
            local: false
        }
    );
    let bound_start = span(source, "bound)>>").start;
    assert_eq!(
        resolved.names[&Span::new(bound_start, bound_start + 5)],
        Binding::Lexical { depth: 1, slot: 0 }
    );
    assert!(resolved.custom_this.contains(&span(source, "this")));
}

#[test]
fn inferred_new_follows_collection_values_and_if_let_bodies_only() {
    for source in [
        "class Item {}\nresult: Listing<Item> = new { new {} }",
        "class Item {}\nresult: Mapping<String, Item> = new { [\"key\"] = if (true) let (x = 1) new {} else new {} }",
        "class Item {}\nresult: Listing<Item> = new { for (x in List(1)) { new {} } }",
    ] {
        resolve_module(&parse_module(source).unwrap()).unwrap();
    }
    for source in [
        "class Item {}\nresult: Listing<Item> = new { (new {}) }",
        "class Item {}\nresult: Mapping<String, Item> = new { [new {}] = new Item {} }",
        "class Item {}\nresult: Listing<Item> = new { let (x: Item = new {}) x }",
        "class Item {}\nresult: Listing<Item> = new { ...new {} }",
    ] {
        let error = resolve_module(&parse_module(source).unwrap()).unwrap_err();
        assert_eq!(
            error.message, "Cannot tell which parent to amend.",
            "{source}"
        );
        assert_eq!(&source[error.span.start..error.span.end], "new");
    }
}
