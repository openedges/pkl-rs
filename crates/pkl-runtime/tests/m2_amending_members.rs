//! Whole raw pinned Java AST validation controls for amending modules.
use pkl_runtime::evaluation::frontend::{
    DiagnosticProfile, FailureDisposition, ModuleEvalOptions, ModuleSourceInput,
    evaluate_module_frontend,
};
#[test]
fn amending_module_ast_errors_match_full_java_bytes_and_order() {
    let options = ModuleEvalOptions {
        diagnostic_profile: DiagnosticProfile::ApiIdentity,
        ..Default::default()
    };
    let cases = [
        (
            r########"amends "life:/child"
class C {}
"########,
            r########"file:///Users/cody.ij.hwang/Projects/pkl-rs.stage42/target/m2/stage42-amending-members-oracle-01/workspace/class-needs-local/main.pkl"########,
            r########"–– Pkl Error ––
Class needs a `local` modifier.

2 | class C {}
    ^^^^^^^
at main (file:///Users/cody.ij.hwang/Projects/pkl-rs.stage42/target/m2/stage42-amending-members-oracle-01/workspace/class-needs-local/main.pkl)

To define a non-local class, extend rather than amend the parent module (which must be `open` for extension).
"########,
        ),
        (
            r########"amends "life:/child"
typealias A = String
"########,
            r########"file:///Users/cody.ij.hwang/Projects/pkl-rs.stage42/target/m2/stage42-amending-members-oracle-01/workspace/alias-needs-local/main.pkl"########,
            r########"–– Pkl Error ––
Type alias needs a `local` modifier.

2 | typealias A = String
    ^^^^^^^^^^^
at main (file:///Users/cody.ij.hwang/Projects/pkl-rs.stage42/target/m2/stage42-amending-members-oracle-01/workspace/alias-needs-local/main.pkl)

To define a non-local type alias, extend rather than amend the parent module (which must be `open` for extension).
"########,
        ),
        (
            r########"amends "life:/child"
value: Int = 1
"########,
            r########"file:///Users/cody.ij.hwang/Projects/pkl-rs.stage42/target/m2/stage42-amending-members-oracle-01/workspace/property-type-forbidden/main.pkl"########,
            r########"–– Pkl Error ––
A non-local object property cannot have a type annotation.

2 | value: Int = 1
           ^^^
at main#value (file:///Users/cody.ij.hwang/Projects/pkl-rs.stage42/target/m2/stage42-amending-members-oracle-01/workspace/property-type-forbidden/main.pkl)
"########,
        ),
        (
            r########"amends "life:/child"
function f() = 1
"########,
            r########"file:///Users/cody.ij.hwang/Projects/pkl-rs.stage42/target/m2/stage42-amending-members-oracle-01/workspace/method-needs-local/main.pkl"########,
            r########"–– Pkl Error ––
Method needs a `local` modifier.

2 | function f() = 1
    ^^^^^^^^^^^^
at main (file:///Users/cody.ij.hwang/Projects/pkl-rs.stage42/target/m2/stage42-amending-members-oracle-01/workspace/method-needs-local/main.pkl)

To define a non-local method, extend rather than amend the parent module (which must be `open` for extension).
"########,
        ),
        (
            r########"amends "life:/child"
function f() = 1
value: Int = 1
typealias A = String
class C {}
"########,
            r########"file:///Users/cody.ij.hwang/Projects/pkl-rs.stage42/target/m2/stage42-amending-members-oracle-01/workspace/grouped-class-before-alias-property-method/main.pkl"########,
            r########"–– Pkl Error ––
Class needs a `local` modifier.

5 | class C {}
    ^^^^^^^
at main (file:///Users/cody.ij.hwang/Projects/pkl-rs.stage42/target/m2/stage42-amending-members-oracle-01/workspace/grouped-class-before-alias-property-method/main.pkl)

To define a non-local class, extend rather than amend the parent module (which must be `open` for extension).
"########,
        ),
        (
            r########"amends "life:/child"
class C { hidden function f() = 1 }
"########,
            r########"file:///Users/cody.ij.hwang/Projects/pkl-rs.stage42/target/m2/stage42-amending-members-oracle-01/workspace/class-body-before-local-restriction/main.pkl"########,
            r########"–– Pkl Error ––
Modifier `hidden` is not applicable to methods.

2 | class C { hidden function f() = 1 }
              ^^^^^^
at main#C (file:///Users/cody.ij.hwang/Projects/pkl-rs.stage42/target/m2/stage42-amending-members-oracle-01/workspace/class-body-before-local-restriction/main.pkl)
"########,
        ),
        (
            r########"amends "life:/child"
typealias A = foo.bar.Baz
"########,
            r########"file:///Users/cody.ij.hwang/Projects/pkl-rs.stage42/target/m2/stage42-amending-members-oracle-01/workspace/alias-body-before-local-restriction/main.pkl"########,
            r########"–– Pkl Error ––
Invalid type name `foo.bar.Baz`.

2 | typealias A = foo.bar.Baz
                  ^^^^^^^^^^^
at main#A (file:///Users/cody.ij.hwang/Projects/pkl-rs.stage42/target/m2/stage42-amending-members-oracle-01/workspace/alias-body-before-local-restriction/main.pkl)

A qualified type name consists of a module import name and a type name.
Example: `importedModule.SomeClass`
"########,
        ),
        (
            r########"amends "life:/child"
function f<T>(x: foo.bar.Baz) = 1
"########,
            r########"file:///Users/cody.ij.hwang/Projects/pkl-rs.stage42/target/m2/stage42-amending-members-oracle-01/workspace/method-local-before-generic-type/main.pkl"########,
            r########"–– Pkl Error ––
Method needs a `local` modifier.

2 | function f<T>(x: foo.bar.Baz) = 1
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at main (file:///Users/cody.ij.hwang/Projects/pkl-rs.stage42/target/m2/stage42-amending-members-oracle-01/workspace/method-local-before-generic-type/main.pkl)

To define a non-local method, extend rather than amend the parent module (which must be `open` for extension).
"########,
        ),
        (
            r########"amends "life:/child"
local function f<T>(x) = x
"########,
            r########"file:///Users/cody.ij.hwang/Projects/pkl-rs.stage42/target/m2/stage42-amending-members-oracle-01/workspace/local-method-generic-rejected/main.pkl"########,
            r########"–– Pkl Error ––
Only standard library members can have type parameters.

2 | local function f<T>(x) = x
                    ^^^
at main#f (file:///Users/cody.ij.hwang/Projects/pkl-rs.stage42/target/m2/stage42-amending-members-oracle-01/workspace/local-method-generic-rejected/main.pkl)
"########,
        ),
        (
            r########"amends "life:/child"
hidden value: Int = 1
"########,
            r########"file:///Users/cody.ij.hwang/Projects/pkl-rs.stage42/target/m2/stage42-amending-members-oracle-01/workspace/property-modifier-before-type/main.pkl"########,
            r########"–– Pkl Error ––
Modifier `hidden` is not applicable to object members.

2 | hidden value: Int = 1
    ^^^^^^
at main (file:///Users/cody.ij.hwang/Projects/pkl-rs.stage42/target/m2/stage42-amending-members-oracle-01/workspace/property-modifier-before-type/main.pkl)
"########,
        ),
        (
            r########"amends "life:/child"
const value: Int = 1
"########,
            r########"file:///Users/cody.ij.hwang/Projects/pkl-rs.stage42/target/m2/stage42-amending-members-oracle-01/workspace/const-property-before-type/main.pkl"########,
            r########"–– Pkl Error ––
Modifier `const` can only be applied to object members that are also `local`.

2 | const value: Int = 1
    ^^^^^
at main (file:///Users/cody.ij.hwang/Projects/pkl-rs.stage42/target/m2/stage42-amending-members-oracle-01/workspace/const-property-before-type/main.pkl)
"########,
        ),
        (
            r########"amends "life:/child"
typealias A< /* header */ T > = T
"########,
            r########"file:///Users/cody.ij.hwang/Projects/pkl-rs.stage42/target/m2/stage42-amending-members-edges-oracle-01/workspace/generic-alias-header/main.pkl"########,
            r########"–– Pkl Error ––
Type alias needs a `local` modifier.

2 | typealias A< /* header */ T > = T
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at main (file:///Users/cody.ij.hwang/Projects/pkl-rs.stage42/target/m2/stage42-amending-members-edges-oracle-01/workspace/generic-alias-header/main.pkl)

To define a non-local type alias, extend rather than amend the parent module (which must be `open` for extension).
"########,
        ),
        (
            r########"amends "life:/child"
local value: Int
"########,
            r########"file:///Users/cody.ij.hwang/Projects/pkl-rs.stage42/target/m2/stage42-amending-members-edges-oracle-01/workspace/local-typed-property-needs-value/main.pkl"########,
            r########"–– Pkl Error ––
Missing property value.

2 | local value: Int
                    ^
at main#value (file:///Users/cody.ij.hwang/Projects/pkl-rs.stage42/target/m2/stage42-amending-members-edges-oracle-01/workspace/local-typed-property-needs-value/main.pkl)
"########,
        ),
        (
            r########"amends "life:/child"
const function f() = 1
"########,
            r########"file:///Users/cody.ij.hwang/Projects/pkl-rs.stage42/target/m2/stage42-amending-method-const-oracle-01/workspace/const-method-needs-local/main.pkl"########,
            r########"–– Pkl Error ––
Method needs a `local` modifier.

2 | const function f() = 1
    ^^^^^^^^^^^^^^^^^^
at main (file:///Users/cody.ij.hwang/Projects/pkl-rs.stage42/target/m2/stage42-amending-method-const-oracle-01/workspace/const-method-needs-local/main.pkl)

To define a non-local method, extend rather than amend the parent module (which must be `open` for extension).
"########,
        ),
    ];
    let mut mismatches = Vec::new();
    for (source, uri, expected) in cases {
        let result = evaluate_module_frontend(
            ModuleSourceInput {
                text: source,
                uri,
                inferred_name: "main",
            },
            &options,
        );
        assert_eq!(result.exit_code, 1, "{uri}");
        assert_eq!(
            result.disposition,
            FailureDisposition::LanguageError,
            "{uri}"
        );
        assert!(result.stdout.is_empty(), "{uri}");
        if result.stderr != expected.as_bytes() {
            mismatches.push((uri, result.stderr, expected.as_bytes()));
        }
    }
    assert!(
        mismatches.is_empty(),
        "Full raw diagnostic differences: {mismatches:?}"
    );
}
#[test]
fn local_amending_declarations_and_alias_header_metadata_remain_valid() {
    let source = r########"amends "life:/child"
local class C {}
local typealias A = String
local value: Int = 1
local function f() = value
"########;
    let parsed = pkl_runtime::parser::parse_module(source).unwrap();
    pkl_runtime::resolver::resolve_module(&parsed).unwrap();
    let source = "amends \"life:/child\"\ntypealias A< /* header */ T > = T\n";
    let parsed = pkl_runtime::parser::parse_module(source).unwrap();
    let pkl_runtime::syntax::ExprKind::TypeAlias { header_span, .. } = &parsed.members[0].expr.kind
    else {
        panic!("alias")
    };
    assert_eq!(
        &source[header_span.start..header_span.end],
        "typealias A< /* header */ T >"
    );
    assert_eq!(parsed, parsed.clone());
}

#[test]
fn amending_object_members_do_not_visit_class_member_annotations() {
    // Actual Java edge control passes AST construction then reports a missing
    // non-local property at evaluation. Its annotation bodies are not visited.
    // This assertion is AST scope only, not a successful output claim.
    let source = r########"amends "life:/child"
@Deprecated { message = module }
value=1
@Deprecated { message = module }
local function f() = value
"########;
    let parsed = pkl_runtime::parser::parse_module(source).unwrap();
    pkl_runtime::resolver::resolve_module(&parsed).unwrap();
}

#[test]
fn local_const_amending_method_remains_valid() {
    let source = r########"amends "life:/child"
local const function f() = 1
"########;
    let parsed = pkl_runtime::parser::parse_module(source).unwrap();
    pkl_runtime::resolver::resolve_module(&parsed).unwrap();
}
