//! Independent Java prerequisite values; native typed bits are not export API proof.
#[path = "support/module_keys.rs"]
mod module_keys;
use pkl_runtime::{
    parser::parse_module,
    probe::SourceProbe,
    resolver::resolve_module,
    value::Value,
    vm::{Vm, VmPoll, compile_module},
};
fn compare(uri: &str, source: &str, rows: &[(&str, &str, &str, &str)]) {
    let mut probe = SourceProbe::new_module_evaluator("compact", false, false, false).unwrap();
    let mut failures = Vec::new();
    for &(id, expression, kind, expected) in rows {
        probe
            .evaluate_module_source(uri, Some(source), expression, 0, None)
            .unwrap();
        let event = loop {
            probe.collect();
            let event = probe.poll(0, 16).unwrap();
            probe.collect();
            if event.kind == "yield" {
                continue;
            }
            if event.kind == "module-request"
                && module_keys::complete_probe(&mut probe, event.token.as_deref().unwrap(), 0)
                    .unwrap()
            {
                continue;
            }
            break event;
        };
        let actual = if event.kind == "value" {
            event.value
        } else {
            event.error
        };
        let expected_units = expected.encode_utf16().collect::<Vec<_>>();
        if event.kind != kind || actual.as_ref() != Some(&expected_units) {
            failures.push(format!(
                "{id}: {expression}\nexpected {kind}: {expected}\nactual {}: {}",
                event.kind,
                actual
                    .map(|s| String::from_utf16_lossy(&s))
                    .unwrap_or_default()
            ));
        }
    }
    probe.close();
    assert_eq!(probe.collect().slots, 0);
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}
#[test]
fn complete_java_list_add_prerequisite() {
    compare(
        "file:///stage61/list-add-prerequisite.pkl",
        include_str!("../../../tests/fixtures/m2-int-seq-list-add/module.pkl"),
        &[
            (
                r########"empty-add"########,
                r########"List().add(1)"########,
                r########"value"########,
                r########"List(1)"########,
            ),
            (
                r########"nonempty-add"########,
                r########"List(1,2).add(3)"########,
                r########"value"########,
                r########"List(1, 2, 3)"########,
            ),
            (
                r########"empty-length"########,
                r########"List().add(1).length"########,
                r########"value"########,
                r########"1"########,
            ),
            (
                r########"nonempty-last"########,
                r########"List(1,2).add(3).last"########,
                r########"value"########,
                r########"3"########,
            ),
            (
                r########"nested-add"########,
                r########"List().add(1).add(2)"########,
                r########"value"########,
                r########"List(1, 2)"########,
            ),
            (
                r########"add-list-element"########,
                r########"List().add(List(1,2))"########,
                r########"value"########,
                r########"List(List(1, 2))"########,
            ),
            (
                r########"add-null-empty"########,
                r########"List().add(null)"########,
                r########"value"########,
                r########"List(null)"########,
            ),
            (
                r########"add-null-nonempty"########,
                r########"List(1).add(null)"########,
                r########"value"########,
                r########"List(1, null)"########,
            ),
            (
                r########"null-last"########,
                r########"List(1).add(null).last == null"########,
                r########"value"########,
                r########"true"########,
            ),
            (
                r########"object-first"########,
                r########"List(new Item { value = 4 }).add(new Item { value = 5 })[0].value"########,
                r########"value"########,
                r########"4"########,
            ),
            (
                r########"object-last"########,
                r########"List(new Item { value = 4 }).add(new Item { value = 5 })[1].value"########,
                r########"value"########,
                r########"5"########,
            ),
            (
                r########"object-reference-value"########,
                r########"let (x = new Item { value = 42 }) List(x).add(x)[1].value"########,
                r########"value"########,
                r########"42"########,
            ),
            (
                r########"lazy-first-skipped"########,
                r########"List(new Item { value = throw("lazy-first") }).add(new Item { value = 4 }).last.value"########,
                r########"value"########,
                r########"4"########,
            ),
            (
                r########"lazy-first-forced"########,
                r########"List(new Item { value = throw("lazy-first") }).add(new Item { value = 4 }).first.value"########,
                r########"error"########,
                r########"–– Pkl Error ––
lazy-first

1 | List(new Item { value = throw("lazy-first") }).add(new Item { value = 4 }).first.value
                            ^^^^^^^^^^^^^^^^^^^
at repl:text.value (repl:text)

1 | List(new Item { value = throw("lazy-first") }).add(new Item { value = 4 }).first.value
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
            ),
            (
                r########"wrong-arity-empty"########,
                r########"List().add()"########,
                r########"error"########,
                r########"–– Pkl Error ––
Expected 1 function arguments but got 0.

3217 | external function add<Other>(element: Other): List<Element | Other>
       ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at pkl.base#List.add (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L3217)

1 | List().add()
    ^^^^^^^^^^^^
at  (repl:text)
"########,
            ),
            (
                r########"wrong-arity-two"########,
                r########"List().add(1,2)"########,
                r########"error"########,
                r########"–– Pkl Error ––
Expected 1 function arguments but got 2.

3217 | external function add<Other>(element: Other): List<Element | Other>
       ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at pkl.base#List.add (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L3217)

1 | List().add(1,2)
    ^^^^^^^^^^^^^^^
at  (repl:text)
"########,
            ),
            (
                r########"argument-error"########,
                r########"List().add(throw("add-error"))"########,
                r########"error"########,
                r########"–– Pkl Error ––
add-error

1 | List().add(throw("add-error"))
               ^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
            ),
            (
                r########"first-add-error"########,
                r########"List().add(throw("first-add")).add(throw("second-add"))"########,
                r########"error"########,
                r########"–– Pkl Error ––
first-add

1 | List().add(throw("first-add")).add(throw("second-add"))
               ^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
            ),
            (
                r########"list-before-add-error"########,
                r########"List(throw("list-first")).add(throw("add-second"))"########,
                r########"error"########,
                r########"–– Pkl Error ––
list-first

1 | List(throw("list-first")).add(throw("add-second"))
         ^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
            ),
            (
                r########"add-before-fold-error"########,
                r########"List(1).add(throw("add-first")).fold(0,(acc,n)->throw("fold-second"))"########,
                r########"error"########,
                r########"–– Pkl Error ––
add-first

1 | List(1).add(throw("add-first")).fold(0,(acc,n)->throw("fold-second"))
                ^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
            ),
            (
                r########"fold-after-add-error"########,
                r########"List(1).add(2).fold(0,(acc,n)->throw("fold-callback"))"########,
                r########"error"########,
                r########"–– Pkl Error ––
fold-callback

1 | List(1).add(2).fold(0,(acc,n)->throw("fold-callback"))
                                   ^^^^^^^^^^^^^^^^^^^^^^
at repl:text.<function#1> (repl:text)

1 | List(1).add(2).fold(0,(acc,n)->throw("fold-callback"))
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
            ),
        ],
    );
}
#[test]
fn all_java_math_constant_strings() {
    compare(
        "file:///stage61/math-constants.pkl",
        include_str!("../../../tests/fixtures/m2-int-seq-math-constants/module.pkl"),
        &[
            (
                r########"minInt"########,
                r########"minInt"########,
                r########"value"########,
                r########"-9223372036854775808"########,
            ),
            (
                r########"minInt8"########,
                r########"minInt8"########,
                r########"value"########,
                r########"-128"########,
            ),
            (
                r########"minInt16"########,
                r########"minInt16"########,
                r########"value"########,
                r########"-32768"########,
            ),
            (
                r########"minInt32"########,
                r########"minInt32"########,
                r########"value"########,
                r########"-2147483648"########,
            ),
            (
                r########"maxInt"########,
                r########"maxInt"########,
                r########"value"########,
                r########"9223372036854775807"########,
            ),
            (
                r########"maxInt8"########,
                r########"maxInt8"########,
                r########"value"########,
                r########"127"########,
            ),
            (
                r########"maxInt16"########,
                r########"maxInt16"########,
                r########"value"########,
                r########"32767"########,
            ),
            (
                r########"maxInt32"########,
                r########"maxInt32"########,
                r########"value"########,
                r########"2147483647"########,
            ),
            (
                r########"maxUInt"########,
                r########"maxUInt"########,
                r########"value"########,
                r########"9223372036854775807"########,
            ),
            (
                r########"maxUInt8"########,
                r########"maxUInt8"########,
                r########"value"########,
                r########"255"########,
            ),
            (
                r########"maxUInt16"########,
                r########"maxUInt16"########,
                r########"value"########,
                r########"65535"########,
            ),
            (
                r########"maxUInt32"########,
                r########"maxUInt32"########,
                r########"value"########,
                r########"4294967295"########,
            ),
            (
                r########"minFiniteFloat"########,
                r########"minFiniteFloat"########,
                r########"value"########,
                r########"-1.7976931348623157E308"########,
            ),
            (
                r########"maxFiniteFloat"########,
                r########"maxFiniteFloat"########,
                r########"value"########,
                r########"1.7976931348623157E308"########,
            ),
            (
                r########"minPositiveFloat"########,
                r########"minPositiveFloat"########,
                r########"value"########,
                r########"4.9E-324"########,
            ),
            (
                r########"e"########,
                r########"e"########,
                r########"value"########,
                r########"2.718281828459045"########,
            ),
            (
                r########"pi"########,
                r########"pi"########,
                r########"value"########,
                r########"3.141592653589793"########,
            ),
        ],
    );
}
fn ready(vm: &mut Vm) -> Value {
    for _ in 0..200_000 {
        vm.collect(&[]);
        match vm.poll(1) {
            VmPoll::Ready(value) => return value.unwrap(),
            VmPoll::Yielded => {}
            VmPoll::Module(request) => vm
                .complete_module(request.id, module_keys::complete(&request))
                .unwrap(),
            event => panic!("unexpected {event:?}"),
        }
    }
    panic!("math constant poll bound")
}
#[test]
fn native_math_values_match_java_long_and_double_bits() {
    let parsed = parse_module(include_str!(
        "../../../tests/fixtures/m2-int-seq-math-constants/module.pkl"
    ))
    .unwrap();
    let mut program = compile_module(&resolve_module(&parsed).unwrap()).unwrap();
    std::sync::Arc::make_mut(&mut program)
        .register_primary_source(
            std::sync::Arc::new(pkl_runtime::value::RuntimeSource {
                text: parsed.source.clone(),
                uri: "file:///stage61/math-constants.pkl".into(),
                module_name: "math-constants".into(),
            }),
            pkl_runtime::modules::ModuleIdentity {
                original_uri: "file:///stage61/math-constants.pkl".into(),
                resolved_uri: "file:///stage61/math-constants.pkl".into(),
            },
        )
        .unwrap();
    let mut vm = Vm::new(program.clone());
    vm.start(program.entry, vec![]).unwrap();
    let module = ready(&mut vm).as_object().unwrap();
    for (name, java_type, bits) in [
        (
            r########"minInt"########,
            r########"java.lang.Long"########,
            0x8000000000000000u64,
        ),
        (
            r########"minInt8"########,
            r########"java.lang.Long"########,
            0xffffffffffffff80u64,
        ),
        (
            r########"minInt16"########,
            r########"java.lang.Long"########,
            0xffffffffffff8000u64,
        ),
        (
            r########"minInt32"########,
            r########"java.lang.Long"########,
            0xffffffff80000000u64,
        ),
        (
            r########"maxInt"########,
            r########"java.lang.Long"########,
            0x7fffffffffffffffu64,
        ),
        (
            r########"maxInt8"########,
            r########"java.lang.Long"########,
            0x000000000000007fu64,
        ),
        (
            r########"maxInt16"########,
            r########"java.lang.Long"########,
            0x0000000000007fffu64,
        ),
        (
            r########"maxInt32"########,
            r########"java.lang.Long"########,
            0x000000007fffffffu64,
        ),
        (
            r########"maxUInt"########,
            r########"java.lang.Long"########,
            0x7fffffffffffffffu64,
        ),
        (
            r########"maxUInt8"########,
            r########"java.lang.Long"########,
            0x00000000000000ffu64,
        ),
        (
            r########"maxUInt16"########,
            r########"java.lang.Long"########,
            0x000000000000ffffu64,
        ),
        (
            r########"maxUInt32"########,
            r########"java.lang.Long"########,
            0x00000000ffffffffu64,
        ),
        (
            r########"minFiniteFloat"########,
            r########"java.lang.Double"########,
            0xffefffffffffffffu64,
        ),
        (
            r########"maxFiniteFloat"########,
            r########"java.lang.Double"########,
            0x7fefffffffffffffu64,
        ),
        (
            r########"minPositiveFloat"########,
            r########"java.lang.Double"########,
            0x0000000000000001u64,
        ),
        (
            r########"e"########,
            r########"java.lang.Double"########,
            0x4005bf0a8b145769u64,
        ),
        (
            r########"pi"########,
            r########"java.lang.Double"########,
            0x400921fb54442d18u64,
        ),
    ] {
        vm.start_member(module, name).unwrap();
        let value = ready(&mut vm);
        let (actual_type, actual_bits) = match value {
            Value::Int(value) => ("java.lang.Long", value.get() as u64),
            Value::Float(value) => ("java.lang.Double", value.to_bits()),
            value => panic!("unexpected {value:?}"),
        };
        assert_eq!((actual_type, actual_bits), (java_type, bits), "{name}");
    }
    vm.close();
    assert_eq!(vm.collect(&[]).slots, 0);
}

#[test]
fn complete_java_list_accessor_fold_prerequisite() {
    compare(
        "file:///stage61/list-accessor-fold.pkl",
        include_str!("../../../tests/fixtures/m2-int-seq-list-accessor-fold/module.pkl"),
        &[
            (
                r########"empty-first"########,
                r########"List().first"########,
                r########"error"########,
                r########"–– Pkl Error ––
Expected a non-empty collection.
Collection: List()

1 | List().first
    ^^^^^^^^^^^^
at  (repl:text)
"########,
            ),
            (
                r########"empty-first-or-null"########,
                r########"List().firstOrNull"########,
                r########"value"########,
                r########"null"########,
            ),
            (
                r########"empty-last"########,
                r########"List().last"########,
                r########"error"########,
                r########"–– Pkl Error ––
Expected a non-empty collection.
Collection: List()

1 | List().last
    ^^^^^^^^^^^
at  (repl:text)
"########,
            ),
            (
                r########"empty-last-or-null"########,
                r########"List().lastOrNull"########,
                r########"value"########,
                r########"null"########,
            ),
            (
                r########"single-first"########,
                r########"List(7).first"########,
                r########"value"########,
                r########"7"########,
            ),
            (
                r########"single-first-or-null"########,
                r########"List(7).firstOrNull"########,
                r########"value"########,
                r########"7"########,
            ),
            (
                r########"single-last"########,
                r########"List(7).last"########,
                r########"value"########,
                r########"7"########,
            ),
            (
                r########"single-last-or-null"########,
                r########"List(7).lastOrNull"########,
                r########"value"########,
                r########"7"########,
            ),
            (
                r########"many-first"########,
                r########"List(1,2,3).first"########,
                r########"value"########,
                r########"1"########,
            ),
            (
                r########"many-first-or-null"########,
                r########"List(1,2,3).firstOrNull"########,
                r########"value"########,
                r########"1"########,
            ),
            (
                r########"many-last"########,
                r########"List(1,2,3).last"########,
                r########"value"########,
                r########"3"########,
            ),
            (
                r########"many-last-or-null"########,
                r########"List(1,2,3).lastOrNull"########,
                r########"value"########,
                r########"3"########,
            ),
            (
                r########"null-first"########,
                r########"List(null).first"########,
                r########"value"########,
                r########"null"########,
            ),
            (
                r########"null-first-or-null"########,
                r########"List(null).firstOrNull"########,
                r########"value"########,
                r########"null"########,
            ),
            (
                r########"null-last"########,
                r########"List(null).last"########,
                r########"value"########,
                r########"null"########,
            ),
            (
                r########"null-last-or-null"########,
                r########"List(null).lastOrNull"########,
                r########"value"########,
                r########"null"########,
            ),
            (
                r########"mixed-first-null"########,
                r########"List(null,1).first"########,
                r########"value"########,
                r########"null"########,
            ),
            (
                r########"mixed-last-value"########,
                r########"List(null,1).last"########,
                r########"value"########,
                r########"1"########,
            ),
            (
                r########"mixed-first-value"########,
                r########"List(1,null).first"########,
                r########"value"########,
                r########"1"########,
            ),
            (
                r########"mixed-last-null"########,
                r########"List(1,null).last"########,
                r########"value"########,
                r########"null"########,
            ),
            (
                r########"mixed-first-or-null"########,
                r########"List(null,1).firstOrNull"########,
                r########"value"########,
                r########"null"########,
            ),
            (
                r########"mixed-last-or-null"########,
                r########"List(1,null).lastOrNull"########,
                r########"value"########,
                r########"null"########,
            ),
            (
                r########"lazy-first-skipped"########,
                r########"List(new Item { value = throw("lazy-first") },new Item { value = 7 }).last.value"########,
                r########"value"########,
                r########"7"########,
            ),
            (
                r########"lazy-first-forced"########,
                r########"List(new Item { value = throw("lazy-first") },new Item { value = 7 }).first.value"########,
                r########"error"########,
                r########"–– Pkl Error ––
lazy-first

1 | List(new Item { value = throw("lazy-first") },new Item { value = 7 }).first.value
                            ^^^^^^^^^^^^^^^^^^^
at repl:text.value (repl:text)

1 | List(new Item { value = throw("lazy-first") },new Item { value = 7 }).first.value
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
            ),
            (
                r########"lazy-first-or-null-forced"########,
                r########"List(new Item { value = throw("lazy-first") }).firstOrNull.value"########,
                r########"error"########,
                r########"–– Pkl Error ––
lazy-first

1 | List(new Item { value = throw("lazy-first") }).firstOrNull.value
                            ^^^^^^^^^^^^^^^^^^^
at repl:text.value (repl:text)

1 | List(new Item { value = throw("lazy-first") }).firstOrNull.value
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
            ),
            (
                r########"lazy-last-skipped"########,
                r########"List(new Item { value = 7 },new Item { value = throw("lazy-last") }).first.value"########,
                r########"value"########,
                r########"7"########,
            ),
            (
                r########"lazy-last-forced"########,
                r########"List(new Item { value = 7 },new Item { value = throw("lazy-last") }).last.value"########,
                r########"error"########,
                r########"–– Pkl Error ––
lazy-last

1 | List(new Item { value = 7 },new Item { value = throw("lazy-last") }).last.value
                                                   ^^^^^^^^^^^^^^^^^^
at repl:text.value (repl:text)

1 | List(new Item { value = 7 },new Item { value = throw("lazy-last") }).last.value
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
            ),
            (
                r########"lazy-last-or-null-forced"########,
                r########"List(new Item { value = throw("lazy-last") }).lastOrNull.value"########,
                r########"error"########,
                r########"–– Pkl Error ––
lazy-last

1 | List(new Item { value = throw("lazy-last") }).lastOrNull.value
                            ^^^^^^^^^^^^^^^^^^
at repl:text.value (repl:text)

1 | List(new Item { value = throw("lazy-last") }).lastOrNull.value
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
            ),
            (
                r########"first-call"########,
                r########"List(1).first()"########,
                r########"error"########,
                r########"–– Pkl Error ––
Cannot find method `first` in class `List`.

1 | List(1).first()
    ^^^^^^^^^^^^^^^
at  (repl:text)

Did you mean any of the following?
first
firstOrNull
findLast(predicate)
"########,
            ),
            (
                r########"first-or-null-call"########,
                r########"List(1).firstOrNull()"########,
                r########"error"########,
                r########"–– Pkl Error ––
Cannot find method `firstOrNull` in class `List`.

1 | List(1).firstOrNull()
    ^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)

Did you mean any of the following?
firstOrNull
restOrNull
first
findLastOrNull(predicate)
findOrNull(predicate)
findLastIndexOrNull(predicate)
splitOrNull(index)
sublistOrNull(start, exclusiveEnd)
findIndexOrNull(predicate)
minWithOrNull(comparator)
maxWithOrNull(comparator)
filterNonNull()
"########,
            ),
            (
                r########"last-call"########,
                r########"List(1).last()"########,
                r########"error"########,
                r########"–– Pkl Error ––
Cannot find method `last` in class `List`.

1 | List(1).last()
    ^^^^^^^^^^^^^^
at  (repl:text)

Did you mean any of the following?
last
lastIndex
lastOrNull
lastIndexOf(element)
lastIndexOfOrNull(element)
"########,
            ),
            (
                r########"last-or-null-call"########,
                r########"List(1).lastOrNull()"########,
                r########"error"########,
                r########"–– Pkl Error ––
Cannot find method `lastOrNull` in class `List`.

1 | List(1).lastOrNull()
    ^^^^^^^^^^^^^^^^^^^^
at  (repl:text)

Did you mean any of the following?
lastOrNull
lastIndexOfOrNull(element)
last
splitOrNull(index)
restOrNull
findLastOrNull(predicate)
sublistOrNull(start, exclusiveEnd)
maxOrNull
getOrNull(index)
maxWithOrNull(comparator)
singleOrNull
maxByOrNull(selector)
"########,
            ),
            (
                r########"first-call-empty"########,
                r########"List().first()"########,
                r########"error"########,
                r########"–– Pkl Error ––
Cannot find method `first` in class `List`.

1 | List().first()
    ^^^^^^^^^^^^^^
at  (repl:text)

Did you mean any of the following?
first
firstOrNull
findLast(predicate)
"########,
            ),
            (
                r########"last-call-empty"########,
                r########"List().last()"########,
                r########"error"########,
                r########"–– Pkl Error ––
Cannot find method `last` in class `List`.

1 | List().last()
    ^^^^^^^^^^^^^
at  (repl:text)

Did you mean any of the following?
last
lastIndex
lastOrNull
lastIndexOf(element)
lastIndexOfOrNull(element)
"########,
            ),
            (
                r########"fold-empty-int"########,
                r########"List().fold(17,(acc,x)->acc+x)"########,
                r########"value"########,
                r########"17"########,
            ),
            (
                r########"fold-empty-list"########,
                r########"List().fold(List(9),(acc,x)->acc.add(x))"########,
                r########"value"########,
                r########"List(9)"########,
            ),
            (
                r########"fold-empty-null"########,
                r########"List().fold(null,(acc,x)->x)"########,
                r########"value"########,
                r########"null"########,
            ),
            (
                r########"fold-empty-object"########,
                r########"List().fold(new Item { value = 5 },(acc,x)->acc).value"########,
                r########"value"########,
                r########"5"########,
            ),
            (
                r########"fold-single-sum"########,
                r########"List(7).fold(3,(acc,x)->acc+x)"########,
                r########"value"########,
                r########"10"########,
            ),
            (
                r########"fold-many-sum"########,
                r########"List(1,2,3).fold(0,(acc,x)->acc+x)"########,
                r########"value"########,
                r########"6"########,
            ),
            (
                r########"fold-many-order"########,
                r########"List(1,2,3).fold(0,(acc,x)->acc*10+x)"########,
                r########"value"########,
                r########"123"########,
            ),
            (
                r########"fold-list-collect"########,
                r########"List(1,2,3).fold(List(),(acc,x)->acc.add(x))"########,
                r########"value"########,
                r########"List(1, 2, 3)"########,
            ),
            (
                r########"fold-list-initial"########,
                r########"List(1,2).fold(List(0),(acc,x)->acc.add(x))"########,
                r########"value"########,
                r########"List(0, 1, 2)"########,
            ),
            (
                r########"fold-change-acc-type"########,
                r########"List(1,2).fold(null,(acc,x)->if (acc == null) x.toString() else acc + x.toString())"########,
                r########"value"########,
                r########"12"########,
            ),
            (
                r########"fold-nested"########,
                r########"List(List(1,2),List(3)).fold(List(),(acc,x)->x.fold(acc,(inner,y)->inner.add(y)))"########,
                r########"value"########,
                r########"List(1, 2, 3)"########,
            ),
            (
                r########"fold-null-element"########,
                r########"List(null,2).fold(0,(acc,x)->if (x == null) acc else acc+x)"########,
                r########"value"########,
                r########"2"########,
            ),
            (
                r########"fold-object-values"########,
                r########"List(new Item { value = 2 },new Item { value = 3 }).fold(0,(acc,x)->acc+x.value)"########,
                r########"value"########,
                r########"5"########,
            ),
            (
                r########"fold-lazy-ignore"########,
                r########"List(new Item { value = throw("lazy-value") }).fold(11,(acc,x)->acc)"########,
                r########"value"########,
                r########"11"########,
            ),
            (
                r########"fold-lazy-force"########,
                r########"List(new Item { value = throw("lazy-value") }).fold(11,(acc,x)->acc+x.value)"########,
                r########"error"########,
                r########"–– Pkl Error ––
lazy-value

1 | List(new Item { value = throw("lazy-value") }).fold(11,(acc,x)->acc+x.value)
                            ^^^^^^^^^^^^^^^^^^^
at repl:text.value (repl:text)

1 | List(new Item { value = throw("lazy-value") }).fold(11,(acc,x)->acc+x.value)
                                                                        ^^^^^^^
at repl:text.<function#1> (repl:text)

1 | List(new Item { value = throw("lazy-value") }).fold(11,(acc,x)->acc+x.value)
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
            ),
            (
                r########"fold-list-add"########,
                r########"List(1,2).fold(List(),(acc,x)->acc.add(x)).last"########,
                r########"value"########,
                r########"2"########,
            ),
            (
                r########"fold-no-args"########,
                r########"List().fold()"########,
                r########"error"########,
                r########"–– Pkl Error ––
Expected 2 function arguments but got 0.

3331 | external function fold<Result>(initial: Result, operator: (Result, Element) -> Result): Result
       ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at pkl.base#List.fold (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L3331)

1 | List().fold()
    ^^^^^^^^^^^^^
at  (repl:text)
"########,
            ),
            (
                r########"fold-one-arg"########,
                r########"List().fold(0)"########,
                r########"error"########,
                r########"–– Pkl Error ––
Expected 2 function arguments but got 1.

3331 | external function fold<Result>(initial: Result, operator: (Result, Element) -> Result): Result
       ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at pkl.base#List.fold (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L3331)

1 | List().fold(0)
    ^^^^^^^^^^^^^^
at  (repl:text)
"########,
            ),
            (
                r########"fold-three-args"########,
                r########"List().fold(0,(acc,x)->acc,1)"########,
                r########"error"########,
                r########"–– Pkl Error ––
Expected 2 function arguments but got 3.

3331 | external function fold<Result>(initial: Result, operator: (Result, Element) -> Result): Result
       ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at pkl.base#List.fold (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L3331)

1 | List().fold(0,(acc,x)->acc,1)
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
            ),
            (
                r########"fold-empty-wrong-callback-type"########,
                r########"List().fold(0,42)"########,
                r########"error"########,
                r########"–– Pkl Error ––
Expected value of type `Function2`, but got type `Int`.
Value: 42

3331 | external function fold<Result>(initial: Result, operator: (Result, Element) -> Result): Result
                                                                 ^^^^^^^^^^^^^^^^^^^^^^^^^^^
at pkl.base#List.fold (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L3331)

1 | List().fold(0,42)
    ^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
            ),
            (
                r########"fold-single-wrong-callback-type"########,
                r########"List(1).fold(0,42)"########,
                r########"error"########,
                r########"–– Pkl Error ––
Expected value of type `Function2`, but got type `Int`.
Value: 42

3331 | external function fold<Result>(initial: Result, operator: (Result, Element) -> Result): Result
                                                                 ^^^^^^^^^^^^^^^^^^^^^^^^^^^
at pkl.base#List.fold (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L3331)

1 | List(1).fold(0,42)
    ^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
            ),
            (
                r########"fold-empty-callback-one-param"########,
                r########"List().fold(0,(acc)->acc)"########,
                r########"error"########,
                r########"–– Pkl Error ––
Expected value of type `Function2`, but got type `Function1`.
Value: new Function1 {}

3331 | external function fold<Result>(initial: Result, operator: (Result, Element) -> Result): Result
                                                                 ^^^^^^^^^^^^^^^^^^^^^^^^^^^
at pkl.base#List.fold (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L3331)

1 | List().fold(0,(acc)->acc)
    ^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
            ),
            (
                r########"fold-empty-callback-three-param"########,
                r########"List().fold(0,(acc,x,y)->acc)"########,
                r########"error"########,
                r########"–– Pkl Error ––
Expected value of type `Function2`, but got type `Function3`.
Value: new Function3 {}

3331 | external function fold<Result>(initial: Result, operator: (Result, Element) -> Result): Result
                                                                 ^^^^^^^^^^^^^^^^^^^^^^^^^^^
at pkl.base#List.fold (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L3331)

1 | List().fold(0,(acc,x,y)->acc)
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
            ),
            (
                r########"fold-single-callback-one-param"########,
                r########"List(1).fold(0,(acc)->acc)"########,
                r########"error"########,
                r########"–– Pkl Error ––
Expected value of type `Function2`, but got type `Function1`.
Value: new Function1 {}

3331 | external function fold<Result>(initial: Result, operator: (Result, Element) -> Result): Result
                                                                 ^^^^^^^^^^^^^^^^^^^^^^^^^^^
at pkl.base#List.fold (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L3331)

1 | List(1).fold(0,(acc)->acc)
    ^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
            ),
            (
                r########"fold-initial-throw"########,
                r########"List().fold(throw("initial-error"),(acc,x)->acc)"########,
                r########"error"########,
                r########"–– Pkl Error ––
initial-error

1 | List().fold(throw("initial-error"),(acc,x)->acc)
                ^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
            ),
            (
                r########"fold-callback-arg-throw"########,
                r########"List().fold(0,throw("operator-error"))"########,
                r########"error"########,
                r########"–– Pkl Error ––
operator-error

1 | List().fold(0,throw("operator-error"))
                  ^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
            ),
            (
                r########"fold-first-callback-throw"########,
                r########"List(1,2).fold(0,(acc,x)->throw("first-callback"))"########,
                r########"error"########,
                r########"–– Pkl Error ––
first-callback

1 | List(1,2).fold(0,(acc,x)->throw("first-callback"))
                              ^^^^^^^^^^^^^^^^^^^^^^^
at repl:text.<function#1> (repl:text)

1 | List(1,2).fold(0,(acc,x)->throw("first-callback"))
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
            ),
            (
                r########"fold-callback-before-lazy"########,
                r########"List(new Item { value = 1 },new Item { value = throw("later-value") }).fold(0,(acc,x)->throw("first-callback"))"########,
                r########"error"########,
                r########"–– Pkl Error ––
first-callback

1 | List(new Item { value = 1 },new Item { value = throw("later-value") }).fold(0,(acc,x)->throw("first-callback"))
                                                                                           ^^^^^^^^^^^^^^^^^^^^^^^
at repl:text.<function#1> (repl:text)

1 | List(new Item { value = 1 },new Item { value = throw("later-value") }).fold(0,(acc,x)->throw("first-callback"))
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
            ),
            (
                r########"fold-lazy-after-first"########,
                r########"List(new Item { value = 1 },new Item { value = throw("later-value") }).fold(0,(acc,x)->acc+x.value)"########,
                r########"error"########,
                r########"–– Pkl Error ––
later-value

1 | List(new Item { value = 1 },new Item { value = throw("later-value") }).fold(0,(acc,x)->acc+x.value)
                                                   ^^^^^^^^^^^^^^^^^^^^
at repl:text.value (repl:text)

1 | List(new Item { value = 1 },new Item { value = throw("later-value") }).fold(0,(acc,x)->acc+x.value)
                                                                                               ^^^^^^^
at repl:text.<function#1> (repl:text)

1 | List(new Item { value = 1 },new Item { value = throw("later-value") }).fold(0,(acc,x)->acc+x.value)
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
            ),
            (
                r########"fold-list-before-initial"########,
                r########"List(throw("list-error")).fold(throw("initial-error"),(acc,x)->acc)"########,
                r########"error"########,
                r########"–– Pkl Error ––
list-error

1 | List(throw("list-error")).fold(throw("initial-error"),(acc,x)->acc)
         ^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
            ),
            (
                r########"fold-initial-before-operator"########,
                r########"List().fold(throw("initial-error"),throw("operator-error"))"########,
                r########"error"########,
                r########"–– Pkl Error ––
initial-error

1 | List().fold(throw("initial-error"),throw("operator-error"))
                ^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
            ),
        ],
    );
}
