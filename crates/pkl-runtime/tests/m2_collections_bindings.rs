//! Collection constructors retain Pkl's separate method and property namespaces.
use pkl_runtime::probe::evaluate_member;
use pkl_runtime::value::Value;

#[test]
fn list_constructor_dispatch_uses_method_bindings_and_ignores_property_shadows() {
    for source in [
        "result = List(42)[0]",
        "List = 1; result = List(42)[0]",
        "local List = 1; result = List(42)[0]",
        "local function List(x) = x + 1; result = List(41)",
        "function List(x) = x + 1; result = List(41)",
        "List = 1; local function List(x) = x + 1; result = List(41)",
        "o = new Dynamic { local function List(x) = x + 1; result = List(41) }; result = o.result",
        "class C { function List(x) = x + 1; result = List(41) }; result = new C {}.result",
    ] {
        assert_eq!(
            evaluate_member(source, "result"),
            Ok(Value::Int(42.into())),
            "{source}"
        );
    }
}
