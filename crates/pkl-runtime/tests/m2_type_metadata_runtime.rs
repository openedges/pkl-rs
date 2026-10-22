//! Declaration identity, expansion, and function metadata. Java byte comparisons
//! are collected separately by the language and persistent sequence harnesses.
use pkl_runtime::heap::LazyState;
use pkl_runtime::parser::parse_module;
use pkl_runtime::resolver::resolve_module;
use pkl_runtime::value::{ObjectId, RuntimeError, Value};
use pkl_runtime::vm::{Vm, VmPoll, compile_module};

fn finish(vm: &mut Vm) -> Result<Value, RuntimeError> {
    for _ in 0..1_000 {
        match vm.poll(100) {
            VmPoll::Ready(result) => return result,
            VmPoll::Yielded => {}
            VmPoll::Log(_) => panic!("unexpected logger request"),
            VmPoll::Pending(_) => panic!("unexpected Host request"),
        }
    }
    panic!("type metadata instruction budget exceeded")
}
fn module(source: &str) -> (Vm, ObjectId) {
    let parsed = parse_module(source).unwrap();
    let program = compile_module(&resolve_module(&parsed).unwrap()).unwrap();
    let mut vm = Vm::new(program.clone());
    vm.start(program.entry, vec![]).unwrap();
    let module = finish(&mut vm).unwrap().as_object().unwrap();
    (vm, module)
}
fn member(vm: &mut Vm, module: ObjectId, name: &str) -> Result<Value, RuntimeError> {
    vm.start_member(module, name)?;
    finish(vm)
}

#[test]
fn alias_declarations_keep_distinct_identity_names_and_mapping_keys() {
    let (mut vm, module) = module(
        "module metadata\ntypealias A = String\ntypealias B = String\nsame = A == A\ndistinct = A != B\nkind = A.getClass() == TypeAlias\ntyped: TypeAlias = A\nname = A.toString()\ninterpolated = \"alias=\\(A)\"\nkeys = new Mapping<TypeAlias, Int> { [A] = 1; [B] = 2 }\nlookup = keys[B]\nlength = keys.length",
    );
    for name in ["same", "distinct", "kind"] {
        assert_eq!(member(&mut vm, module, name), Ok(Value::Bool(true)));
    }
    assert_eq!(
        member(&mut vm, module, "typed"),
        member(&mut vm, module, "A")
    );
    assert_eq!(
        member(&mut vm, module, "name"),
        Ok(Value::string("metadata#A"))
    );
    assert_eq!(
        member(&mut vm, module, "interpolated"),
        Ok(Value::string("alias=metadata#A"))
    );
    for name in ["lookup", "length"] {
        assert_eq!(member(&mut vm, module, name), Ok(Value::Int(2)));
    }
}

#[test]
fn failed_alias_initialization_publishes_identity_without_caching_wrapper_failure() {
    let (mut vm, module) = module("typealias A = Missing\nwrapper = A\nsame = A == wrapper");
    assert_eq!(
        member(&mut vm, module, "wrapper").unwrap_err().message,
        "Cannot find type `Missing`."
    );
    for name in ["A", "wrapper"] {
        assert_eq!(
            vm.member_state(module, name).unwrap(),
            LazyState::Unevaluated
        );
    }
    let value = member(&mut vm, module, "wrapper").unwrap();
    let Value::TypeAlias(id) = value else {
        panic!("missing alias value")
    };
    assert_eq!(member(&mut vm, module, "A"), Ok(Value::TypeAlias(id)));
    assert_eq!(member(&mut vm, module, "same"), Ok(Value::Bool(true)));
}

#[test]
fn mutual_alias_cycles_publish_both_declarations_and_preserve_partial_identity() {
    let (mut vm, module) = module("typealias A = B\ntypealias B = A\nsame = A == A && B == B");
    assert_eq!(
        member(&mut vm, module, "A").unwrap_err().message,
        "Type alias definitions must not be cyclic."
    );
    let a = member(&mut vm, module, "A").unwrap();
    let b = member(&mut vm, module, "B").unwrap();
    assert_ne!(a, b);
    assert_eq!(member(&mut vm, module, "same"), Ok(Value::Bool(true)));
}

#[test]
fn omitted_generic_alias_arguments_are_unknown_and_explicit_arguments_validate() {
    let (mut vm, module) = module(
        "typealias Identity<T> = T\nloose: Identity = 42\nstrict: Identity<String> = 42\nvalue: Identity<List<Int>> = List(1, 2)\nfirst = value[0]",
    );
    assert_eq!(member(&mut vm, module, "loose"), Ok(Value::Int(42)));
    assert_eq!(member(&mut vm, module, "first"), Ok(Value::Int(1)));
    assert_eq!(
        member(&mut vm, module, "strict").unwrap_err().message,
        "Expected value of type `String`, but got type `Int`.\nValue: 42"
    );
}

#[test]
fn function_types_check_arity_and_leave_lambda_annotations_responsible_for_calls() {
    let (mut vm, module) = module(
        "f: (Int) -> String = (x) -> 3\ng: Function1<Int, String> = (x: String) -> x\nuncheckedResult = f.apply(false)\ncheckedArgument = g.apply(4)\nkind = f.getClass() == Function1\ntext = f.toString()\nwrong: (Int) -> String = () -> 1\nanyArity: Function<String> = () -> 2\nanyResult = anyArity.apply()",
    );
    assert_eq!(
        member(&mut vm, module, "uncheckedResult"),
        Ok(Value::Int(3))
    );
    assert_eq!(member(&mut vm, module, "anyResult"), Ok(Value::Int(2)));
    assert_eq!(member(&mut vm, module, "kind"), Ok(Value::Bool(true)));
    assert_eq!(
        member(&mut vm, module, "text"),
        Ok(Value::string("new Function1 {}"))
    );
    assert_eq!(
        member(&mut vm, module, "checkedArgument")
            .unwrap_err()
            .message,
        "Expected value of type `String`, but got type `Int`.\nValue: 4"
    );
    assert_eq!(
        member(&mut vm, module, "wrong").unwrap_err().message,
        "Expected value of type `Function1`, but got type `Function0`.\nValue: new Function0 {}"
    );
}

#[test]
fn reference_function_syntax_metadata_is_distinct_from_function_class_subtyping() {
    let (mut vm, module) = module(
        "import \"pkl:ref\"\nclass D extends ref.Domain {}\nd = new D {}\nclass Holder { arrow: (Int) -> String; named: Function1<Int, String> }\nr = ref.Reference(d, Holder, null)\narrow = r.arrow is ref.Reference<D, (Int) -> String>\nnamed = r.named is ref.Reference<D, Function1<Int, String>>\nany = r.arrow is ref.Reference<D, Any>\nmissing = r.arrow.missing",
    );
    assert_eq!(member(&mut vm, module, "arrow"), Ok(Value::Bool(false)));
    assert_eq!(member(&mut vm, module, "named"), Ok(Value::Bool(true)));
    assert_eq!(member(&mut vm, module, "any"), Ok(Value::Bool(true)));
    assert_eq!(
        member(&mut vm, module, "missing")
            .unwrap_err()
            .hint
            .as_deref(),
        Some("Cannot find property `missing` in type `(Int) -> String`.")
    );
}

#[test]
fn alias_constraints_remain_deferred_when_alias_values_and_references_are_observed() {
    let (mut vm, module) = module(
        "import \"pkl:ref\"\nclass D extends ref.Domain {}\nd = new D {}\ntypealias A = String(throw(\"constraint executed\"))\nclass Holder { p: A }\nidentity = A == A\nreference = ref.Reference(d, Holder, null).p is ref.Reference<D, String>\nchecked: A = \"value\"",
    );
    assert_eq!(member(&mut vm, module, "identity"), Ok(Value::Bool(true)));
    assert_eq!(member(&mut vm, module, "reference"), Ok(Value::Bool(true)));
    assert_eq!(
        member(&mut vm, module, "checked").unwrap_err().message,
        "constraint executed"
    );
}

#[test]
fn exported_arrow_equality_preserves_alias_identity_and_exact_constraint_text() {
    let (mut vm, module) = module(
        r#"import "pkl:ref"
class D extends ref.Domain {}
d = new D {}
typealias Text = String
class Holder {
  a: (Text) -> Int
  b: (String) -> Int
  c: (Int(this > 0)) -> String
  d: (Int(this > 0)) -> String
  e: (Int(this>0)) -> String
}
r = ref.Reference(d, Holder, null)
alias = r.a == (r.b)
// Equal paths are required, so compare referents at the same property across classes.
class A { p: (Text) -> Int }
class B { p: (String) -> Int }
class C { p: (Int(this > 0)) -> String }
class E { p: (Int(this>0)) -> String }
class F { p: (Int(this > 0)) -> String }
left = ref.Reference(d, A, null).p
right = ref.Reference(d, B, null).p
constrained = ref.Reference(d, C, null).p
sameConstraint = ref.Reference(d, F, null).p
spaced = ref.Reference(d, E, null).p
aliasDistinct = left != right
constraintEqual = constrained == sameConstraint
constraintDistinct = constrained != spaced
mapping = new Mapping { [constrained] = 7 }
lookup = mapping[sameConstraint]
"#,
    );
    for name in ["aliasDistinct", "constraintEqual", "constraintDistinct"] {
        assert_eq!(member(&mut vm, module, name), Ok(Value::Bool(true)));
    }
    assert_eq!(member(&mut vm, module, "lookup"), Ok(Value::Int(7)));
}

#[test]
fn string_constraint_members_count_codepoints_through_custom_and_explicit_this() {
    let (mut vm, module) = module(
        "typealias Text = String(length == 2, !isEmpty)\nvalue: Text = \"a😀\"\nexplicit = value.length\nwrong: Text = \"abc\"",
    );
    assert_eq!(member(&mut vm, module, "explicit"), Ok(Value::Int(2)));
    assert!(member(&mut vm, module, "wrong").is_err());
}

#[test]
fn reference_function_classes_preserve_exported_arity_and_variance() {
    let (mut vm, module) = module(
        r#"import "pkl:ref"
class D extends ref.Domain {}
d = new D {}
class Holder {
  one: Function1<Int, String>
  zero: Function0<String>
  nested: Function1<(Int) -> Int, String>
  contra: Function1<Any, String>
}
r = ref.Reference(d, Holder, null)
one = r.one is ref.Reference<D, Function<String>>
zero = r.zero is ref.Reference<D, Function<String>>
nested = r.nested is ref.Reference<D, Function1<(Int) -> Int, String>>
contra = r.contra is ref.Reference<D, Function1<Int, Any>>
covariant = r.zero is ref.Reference<D, Function0<Any>>
"#,
    );
    for (name, expected) in [
        ("one", false),
        ("zero", true),
        ("nested", false),
        ("contra", true),
        ("covariant", true),
    ] {
        assert_eq!(member(&mut vm, module, name), Ok(Value::Bool(expected)));
    }
}

#[test]
fn failed_alias_type_use_checks_partial_identity_before_replaying_expansion_errors() {
    for definition in ["Missing", "A"] {
        let (mut vm, module) = module(&format!(
            "typealias A = {definition}\nclass Holder {{ typed: A = 1 }}\nuse = new Holder {{}}.typed\ntest = 1 is A\ncast = 1 as A\nlocalValue = let (x: A = 1) x\nlambda = (x: A) -> x"
        ));
        assert!(member(&mut vm, module, "A").is_err());
        for name in ["use", "test", "cast", "localValue", "lambda"] {
            assert_eq!(
                member(&mut vm, module, name).unwrap_err().message,
                "Type alias definitions must not be cyclic."
            );
        }
        assert_eq!(
            member(&mut vm, module, "use").unwrap_err().message,
            "Cannot find property `typed` in object of type `#Holder`."
        );
        assert!(matches!(
            member(&mut vm, module, "A"),
            Ok(Value::TypeAlias(_))
        ));
    }
}

#[test]
fn failed_alias_constructors_and_superclasses_resolve_the_published_declaration() {
    let (mut vm, module) = module(
        "typealias A = Missing\nconstruction = new A {}\nclass Child extends A {}\nchild = Child",
    );
    assert_eq!(
        member(&mut vm, module, "A").unwrap_err().message,
        "Cannot find type `Missing`."
    );
    for name in ["construction", "child"] {
        assert_eq!(
            member(&mut vm, module, name).unwrap_err().message,
            "Type alias definitions must not be cyclic."
        );
    }
    assert!(matches!(
        member(&mut vm, module, "A"),
        Ok(Value::TypeAlias(_))
    ));
    assert!(matches!(
        member(&mut vm, module, "child"),
        Ok(Value::Class(_))
    ));
}
