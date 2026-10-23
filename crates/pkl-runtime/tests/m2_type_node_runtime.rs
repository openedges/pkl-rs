//! Persistent type-use ownership; expectations are also covered by the pinned
//! Java sequence acquisitions, rather than generated from candidate output.
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
            VmPoll::Module(_) => panic!("unexpected module request"),
            VmPoll::Pending(_) => panic!("unexpected Host request"),
        }
    }
    panic!("instruction budget exceeded")
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

fn const_failure(result: Result<Value, RuntimeError>, kind: &str, name: &str) {
    let verb = if kind == "method" {
        "call"
    } else {
        "reference"
    };
    assert_eq!(
        result.unwrap_err().message,
        format!("Cannot {verb} {kind} `{name}` from here because it is not `const`.")
    );
}

#[test]
fn inline_and_lambda_type_uses_publish_independent_read_nodes() {
    for constraint in ["this < threshold", "(n) -> n < threshold"] {
        let (mut vm, module) = module(&format!(
            "local threshold=0\ntypealias A=Int({constraint})\nfirst:A=-1\nsecond:A=-2\ntest=-3 is A\ncast=-4 as A"
        ));
        for name in ["first", "second", "test", "cast"] {
            const_failure(member(&mut vm, module, name), "property", "threshold");
        }
        for (name, value) in [
            ("first", Value::Int((-1).into())),
            ("second", Value::Int((-2).into())),
            ("test", Value::Bool(true)),
            ("cast", Value::Int((-4).into())),
        ] {
            assert_eq!(member(&mut vm, module, name), Ok(value));
        }
    }
}

#[test]
fn let_root_shares_its_read_node_across_distinct_type_uses() {
    let (mut vm, module) = module(
        "local threshold=0\ntypealias A=Int(let(n=this)n<threshold)\nfirst:A=-1\nsecond:A=-2\ntest=-3 is A\ncast=-4 as A",
    );
    const_failure(member(&mut vm, module, "first"), "property", "threshold");
    for (name, value) in [
        ("second", Value::Int((-2).into())),
        ("test", Value::Bool(true)),
        ("cast", Value::Int((-4).into())),
        ("first", Value::Int((-1).into())),
    ] {
        assert_eq!(member(&mut vm, module, name), Ok(value));
    }
}

#[test]
fn recursive_type_uses_retain_distinct_reached_depths() {
    let (mut vm, module) = module(
        "local threshold=0\ntypealias A=Int(this<0||((this-1)is A&&this<threshold))\nvalue:A=2\nother:A=0\nstable=42",
    );
    assert_eq!(member(&mut vm, module, "stable"), Ok(Value::Int(42.into())));
    for _ in 0..3 {
        const_failure(member(&mut vm, module, "value"), "property", "threshold");
    }
    for _ in 0..3 {
        assert_eq!(member(&mut vm, module, "value"), Ok(Value::Int(2.into())));
    }
    const_failure(member(&mut vm, module, "other"), "property", "threshold");
    assert_eq!(member(&mut vm, module, "other"), Ok(Value::Int(0.into())));
    assert_eq!(member(&mut vm, module, "stable"), Ok(Value::Int(42.into())));
}

#[test]
fn ordinary_property_and_both_method_kinds_keep_failing() {
    let (mut vm, module) = module(
        "threshold=0\nlocal function lower()=0\nfunction upper()=0\ntypealias A=Int(this<threshold)\ntypealias B=Int(this<lower())\ntypealias C=Int(this<upper())\na:A=-1\nb:B=-1\nc:C=-1",
    );
    for _ in 0..3 {
        const_failure(member(&mut vm, module, "a"), "property", "threshold");
        const_failure(member(&mut vm, module, "b"), "method", "lower");
        const_failure(member(&mut vm, module, "c"), "method", "upper");
    }
}

#[test]
fn resolved_local_member_does_not_turn_its_failed_value_into_a_success() {
    let (mut vm, module) = module(
        "local threshold=throw(\"body\")\ntypealias A=Int(this<threshold)\nvalue:A=-1\nstable=42",
    );
    const_failure(member(&mut vm, module, "value"), "property", "threshold");
    for _ in 0..3 {
        assert_eq!(
            member(&mut vm, module, "value").unwrap_err().message,
            "body"
        );
        assert_eq!(member(&mut vm, module, "stable"), Ok(Value::Int(42.into())));
    }
}

#[test]
fn lazy_collection_elements_share_the_annotation_argument_node() {
    for collection in ["Listing", "Mapping"] {
        let (body, first, second) = if collection == "Listing" {
            ("-1; -2", "0", "1")
        } else {
            ("[\"a\"]=-1; [\"b\"]=-2", "\"a\"", "\"b\"")
        };
        let arguments = if collection == "Mapping" {
            "String,A"
        } else {
            "A"
        };
        let (mut vm, module) = module(&format!(
            "local threshold=0\ntypealias A=Int(this<threshold)\nvalues:{collection}<{arguments}> =new {collection}{{{body}}}\none=values[{first}]\ntwo=values[{second}]"
        ));
        const_failure(member(&mut vm, module, "one"), "property", "threshold");
        assert_eq!(member(&mut vm, module, "two"), Ok(Value::Int((-2).into())));
        assert_eq!(member(&mut vm, module, "one"), Ok(Value::Int((-1).into())));
    }
}

#[test]
fn inherited_annotation_uses_the_declaring_property_node() {
    let (mut vm, module) = module(
        "local threshold=0\ntypealias A=Int(this<threshold)\nopen class Base{value:A=-1}\nclass Child extends Base{value=-2}\na=new Child{}\nb=new Base{}\none=a.value\ntwo=b.value",
    );
    const_failure(member(&mut vm, module, "one"), "property", "threshold");
    assert_eq!(member(&mut vm, module, "two"), Ok(Value::Int((-1).into())));
    assert_eq!(member(&mut vm, module, "one"), Ok(Value::Int((-2).into())));
}

#[test]
fn shared_constraint_code_does_not_elide_a_distinct_lazy_cast_node() {
    let (mut vm, module) = module(
        "local threshold=0\ntypealias A=Int(let(n=this)n<threshold)\nvalues:Listing<A> =new Listing{-1}\ncast=values as Listing<A>\none=values[0]\ntwo=cast[0]",
    );
    // TypeNode.isEquivalentTo checks identity before constrained types return
    // false. VmListingOrMapping therefore keeps both authored cast nodes even
    // though their let constraint call target is shared.
    let first = member(&mut vm, module, "values").unwrap();
    let second = member(&mut vm, module, "cast").unwrap();
    assert_ne!(first, second);
    const_failure(member(&mut vm, module, "one"), "property", "threshold");
    assert_eq!(member(&mut vm, module, "one"), Ok(Value::Int((-1).into())));
    assert_eq!(member(&mut vm, module, "two"), Ok(Value::Int((-1).into())));
}

#[test]
fn earlier_conditional_checks_do_not_change_later_literal_ownership() {
    for (literal, access) in [
        ("(n)->n is A", ".apply(-1)"),
        ("new Dynamic { local typed:A=-1; value=typed }", ".value"),
    ] {
        for first_branch in [true, false] {
            let (mut vm, module) = module(&format!(
                "local threshold=0\ntypealias A=Int(this<threshold)\nfunction make(flag)=if(flag||(-1 is Int(this<0))){literal} else throw(\"unreachable\")\nfirst=make({first_branch}){access}\nsecond=make({}){access}",
                !first_branch,
            ));
            let result = if access.starts_with(".apply") {
                Value::Bool(true)
            } else {
                Value::Int((-1).into())
            };
            const_failure(member(&mut vm, module, "first"), "property", "threshold");
            assert_eq!(member(&mut vm, module, "first"), Ok(result.clone()));
            assert_eq!(member(&mut vm, module, "second"), Ok(result));
        }
    }
}
