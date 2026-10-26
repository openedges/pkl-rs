//! Class corrections grounded in the sealed Stage56 Java public-query capture.
#[path = "support/module_keys.rs"]
mod module_keys;

use pkl_runtime::probe::{SourceEvent, SourceProbe};

fn probe(source: &str, expressions: &[&str]) -> SourceProbe {
    SourceProbe::new_expressions(
        source,
        "file:///stage56-class-values/main.pkl",
        "main",
        &expressions
            .iter()
            .map(|text| (*text).into())
            .collect::<Vec<_>>(),
        "compact",
        false,
        false,
        true,
    )
    .unwrap()
}

fn evaluate(probe: &mut SourceProbe, index: u32) -> SourceEvent {
    probe.start_path(index, 0, None).unwrap();
    for _ in 0..100_000 {
        let event = probe.poll(0, 1).unwrap();
        probe.collect();
        if event.kind == "module-request"
            && module_keys::complete_probe(probe, event.token.as_deref().unwrap(), 0).unwrap()
        {
            continue;
        }
        if event.kind != "yield" {
            return event;
        }
    }
    panic!("Class query did not settle")
}

fn value(probe: &mut SourceProbe, index: u32, expected: &str) {
    let event = evaluate(probe, index);
    assert_eq!(event.kind, "value", "{event:?}");
    assert_eq!(
        event.value.unwrap(),
        expected.encode_utf16().collect::<Vec<_>>()
    );
}

#[test]
fn builtin_class_values_resolve_without_constructing_primitive_instances() {
    let mut probe = probe(
        "stable=42",
        &[
            "Object.simpleName",
            "Typed.simpleName",
            "DataSize.simpleName",
            "IntSeq.simpleName",
            "Regex.simpleName",
            "Typed.toString()",
            "Object.getClass() == Class",
            "DataSize.getClass() == Class",
        ],
    );
    for (index, expected) in [
        "Object", "Typed", "DataSize", "IntSeq", "Regex", "Typed", "true", "true",
    ]
    .into_iter()
    .enumerate()
    {
        value(&mut probe, index as u32, expected);
    }
    probe.close();
    probe.collect();
    assert_eq!(probe.heap_slots(), 0);
}

#[test]
fn typed_type_uses_runtime_representation_independently_of_nominal_parent() {
    let mut probe = probe(
        include_str!("../../../tests/fixtures/m2-class-values/inputs/external-any-pkl-prefix.pkl"),
        &[
            "C.simpleName",
            "(new C {}).getClass() == C",
            "(new C {}) is Typed",
            "(new C {}) is Any",
        ],
    );
    for (index, expected) in ["C", "true", "true", "true"].into_iter().enumerate() {
        value(&mut probe, index as u32, expected);
    }
}

#[test]
fn intrinsic_type_selection_does_not_merge_same_named_class_values() {
    let mut probe = probe(
        include_str!("../../../tests/fixtures/m2-class-values/inputs/builtin-name-spoof.pkl"),
        &[
            "declaredInt == Int",
            "declaredInt.simpleName",
            "(new Int {}).getClass() == Int",
            "instance.x",
            "declaredInt == Int",
            "Int == 1.getClass()",
        ],
    );
    value(&mut probe, 0, "false");
    value(&mut probe, 1, "Int");
    let query = evaluate(&mut probe, 2);
    // main/builtin-name-spoof-query-boundary outer-2 stderr SHA256
    // e667d6c3008c90b520d7e53d5be483068e0c05f6cc0ec9f69b2a0e3b9d925810.
    let expected = "–– Pkl Error ––\nCannot instantiate, or amend an instance of, external class `Int`.\n\n1 | (new Int {}).getClass() == Int\n         ^^^\nat  (repl:text)\n\nTo create an instance of an external class, use its literal syntax or constructor method.\nExamples:\n* `123` uses literal syntax to create an instance of class `Int`.\n* `Pair(1, 2)` uses the `Pair()` constructor method to create an instance of class `Pair`.\n";
    assert_eq!(
        query.error.unwrap(),
        expected.encode_utf16().collect::<Vec<_>>()
    );
    let authored = evaluate(&mut probe, 3);
    assert_eq!(authored.kind, "error");
    value(&mut probe, 4, "false");
    value(&mut probe, 5, "true");
}

#[test]
fn delayed_frontend_errors_keep_query_and_class_owners() {
    let cases = [
        (
            include_str!("../../../tests/fixtures/m2-class-values/inputs/values.pkl"),
            "((a,b,c,d,e,f) -> a).getClass().simpleName",
            "–– Pkl Error ––\nFunction literals can have at most five parameters.\n\n1 | ((a,b,c,d,e,f) -> a).getClass().simpleName\n     ^^^^^^^^^^^^^^^^^^\nat repl:text (repl:text)\n",
        ),
        (
            include_str!("../../../tests/fixtures/m2-class-values/inputs/superclass-union.pkl"),
            "C.simpleName",
            "–– Pkl Error ––\n`Typed|Dynamic` is not a valid supertype.\n\n1 | class C extends (Typed|Dynamic) {}\n                     ^^^^^^^^^^^^^\nat main#C (file:///stage56-class-values/main.pkl)\n",
        ),
        (
            include_str!("../../../tests/fixtures/m2-class-values/inputs/superclass-nullable.pkl"),
            "C.simpleName",
            "–– Pkl Error ––\n`Typed?` is not a valid supertype.\n\n1 | class C extends Typed? {}\n                    ^^^^^^\nat main#C (file:///stage56-class-values/main.pkl)\n",
        ),
    ];
    // Complete Java outer-0 stderr bytes, preserved in oracle-evidence.tar.gz
    // main/cases/{function-six-parameters,superclass-union,superclass-nullable}.
    for (source, expression, expected) in cases {
        let mut probe = probe(source, &[expression, expression]);
        for index in 0..2 {
            let event = evaluate(&mut probe, index);
            assert_eq!(event.kind, "error");
            assert_eq!(
                event.error.unwrap(),
                expected.encode_utf16().collect::<Vec<_>>()
            );
        }
    }
}

#[test]
fn every_pinned_base_class_has_one_default_scope_and_module_export_identity() {
    // Derive the inventory from the pinned declaration source, including classes
    // whose external value operations remain unimplemented. This checks that
    // descriptor coverage cannot shrink to the currently implemented primitives.
    let source = include_str!("../../../external/pkl/stdlib/base.pkl");
    let parsed = pkl_runtime::parser::parse_module(source).unwrap();
    let names: Vec<_> = parsed
        .members
        .iter()
        .filter(|member| {
            matches!(
                member.expr.kind,
                pkl_runtime::syntax::ExprKind::Class { .. }
            )
        })
        .map(|member| member.name.as_str())
        .collect();
    let expressions: Vec<_> = names
        .iter()
        .flat_map(|name| {
            [
                format!("{name}.simpleName"),
                format!("base.{name} == {name}"),
            ]
        })
        .collect();
    let mut probe = probe(
        "base = import(\"pkl:base\")",
        &expressions.iter().map(String::as_str).collect::<Vec<_>>(),
    );
    for (index, name) in names.into_iter().enumerate() {
        value(&mut probe, (index * 2) as u32, name);
        value(&mut probe, (index * 2 + 1) as u32, "true");
    }
    probe.close();
    probe.collect();
    assert_eq!(probe.heap_slots(), 0);
}

// Exact authored input from the additive Java metadata control; external
// renderer methods are deliberately not exercised by this authority.
const METADATA_SOURCE: &str =
    include_str!("../../../tests/fixtures/m2-class-metadata-completion/inputs/metadata.pkl");

fn metadata_probe(expressions: &[&str]) -> SourceProbe {
    SourceProbe::new_expressions(
        METADATA_SOURCE,
        "file:///stage56-class-metadata-completion/main.pkl",
        "main",
        &expressions.iter().map(|s| (*s).into()).collect::<Vec<_>>(),
        "compact",
        false,
        false,
        false,
    )
    .unwrap()
}

#[test]
fn renderer_class_prototypes_preserve_java_ancestry_and_lazy_defaults() {
    let cases = [
        // Java renderer-ancestry-defaults/outer-0 stdout SHA256 b5bea41b6c623f7c09f1bf24dcae58ebab3c0cdd90ad966bc43a45b44867e12b
        ("yaml is ValueRenderer", "true"),
        // Java renderer-ancestry-defaults/outer-1 stdout SHA256 b5bea41b6c623f7c09f1bf24dcae58ebab3c0cdd90ad966bc43a45b44867e12b
        ("yaml is BaseValueRenderer", "true"),
        // Java renderer-ancestry-defaults/outer-2 stdout SHA256 b5bea41b6c623f7c09f1bf24dcae58ebab3c0cdd90ad966bc43a45b44867e12b
        ("yaml is Typed", "true"),
        // Java renderer-ancestry-defaults/outer-3 stdout SHA256 b5bea41b6c623f7c09f1bf24dcae58ebab3c0cdd90ad966bc43a45b44867e12b
        ("plist is ValueRenderer", "true"),
        // Java renderer-ancestry-defaults/outer-4 stdout SHA256 b5bea41b6c623f7c09f1bf24dcae58ebab3c0cdd90ad966bc43a45b44867e12b
        ("plist is BaseValueRenderer", "true"),
        // Java renderer-ancestry-defaults/outer-5 stdout SHA256 b5bea41b6c623f7c09f1bf24dcae58ebab3c0cdd90ad966bc43a45b44867e12b
        ("plist is Typed", "true"),
        // Java renderer-ancestry-defaults/outer-6 stdout SHA256 b5bea41b6c623f7c09f1bf24dcae58ebab3c0cdd90ad966bc43a45b44867e12b
        ("properties is ValueRenderer", "true"),
        // Java renderer-ancestry-defaults/outer-7 stdout SHA256 b5bea41b6c623f7c09f1bf24dcae58ebab3c0cdd90ad966bc43a45b44867e12b
        ("properties is BaseValueRenderer", "true"),
        // Java renderer-ancestry-defaults/outer-8 stdout SHA256 b5bea41b6c623f7c09f1bf24dcae58ebab3c0cdd90ad966bc43a45b44867e12b
        ("properties is Typed", "true"),
        // Java renderer-ancestry-defaults/outer-9 stdout SHA256 9831daaaa0a94144fc3378fde30109ecfc1639ae1d738f35bedbb6ac80210f85
        ("yaml.extension", "yaml"),
        // Java renderer-ancestry-defaults/outer-10 stdout SHA256 2b33a9570d42f8af1c84dbe8b8c9bb50664f9b19ca8f7ee100548b9945a52f5d
        ("yaml.mode", "compat"),
        // Java renderer-ancestry-defaults/outer-11 stdout SHA256 d4735e3a265e16eee03f59718b9b5d03019c07d8b6c51f90da3a666eec13ab35
        ("yaml.indentWidth", "2"),
        // Java renderer-ancestry-defaults/outer-12 stdout SHA256 b5bea41b6c623f7c09f1bf24dcae58ebab3c0cdd90ad966bc43a45b44867e12b
        ("yaml.omitNullProperties", "true"),
        // Java renderer-ancestry-defaults/outer-13 stdout SHA256 fcbcf165908dd18a9e49f7ff27810176db8e9f63b4352213741664245224f8aa
        ("yaml.isStream", "false"),
        // Java renderer-ancestry-defaults/outer-14 stdout SHA256 9ceec13202afbf12ee3abb994c669c711749c18e194326734db6123e94947e04
        ("plist.extension", "plist"),
        // Java renderer-ancestry-defaults/outer-15 stdout SHA256 6c179f21e6f62b629055d8ab40f454ed02e48b68563913473b857d3638e23b28
        ("plist.indent", "  "),
        // Java renderer-ancestry-defaults/outer-16 stdout SHA256 96adb92452863058fa1616efcc89c6e7c92a0c49148dfbd3e127ce829a45406e
        ("properties.extension", "properties"),
        // Java renderer-ancestry-defaults/outer-17 stdout SHA256 b5bea41b6c623f7c09f1bf24dcae58ebab3c0cdd90ad966bc43a45b44867e12b
        ("properties.omitNullProperties", "true"),
        // Java renderer-ancestry-defaults/outer-18 stdout SHA256 fcbcf165908dd18a9e49f7ff27810176db8e9f63b4352213741664245224f8aa
        ("properties.restrictCharset", "false"),
        // Java renderer-ancestry-defaults/outer-19 stdout SHA256 b5bea41b6c623f7c09f1bf24dcae58ebab3c0cdd90ad966bc43a45b44867e12b
        ("yaml.converters.isEmpty", "true"),
        // Java renderer-ancestry-defaults/outer-20 stdout SHA256 b5bea41b6c623f7c09f1bf24dcae58ebab3c0cdd90ad966bc43a45b44867e12b
        ("plist.converters.isEmpty", "true"),
        // Java renderer-ancestry-defaults/outer-21 stdout SHA256 b5bea41b6c623f7c09f1bf24dcae58ebab3c0cdd90ad966bc43a45b44867e12b
        ("properties.converters.isEmpty", "true"),
    ];
    let mut probe = metadata_probe(
        &cases
            .iter()
            .map(|(expression, _)| *expression)
            .collect::<Vec<_>>(),
    );
    for (index, (_, expected)) in cases.into_iter().enumerate() {
        value(&mut probe, index as u32, expected);
    }
    probe.close();
    probe.collect();
    assert_eq!(probe.heap_slots(), 0);
}

#[test]
fn ordinary_class_members_preserve_java_values_and_overrides() {
    let cases = [
        // Java ordinary-valid-values/outer-0 stdout SHA256 dabd3aff769f07eb2965401eb029974ebba3407afd02b26ddb564ea5f8efae72
        ("validDirective.before", "<"),
        // Java ordinary-valid-values/outer-1 stdout SHA256 73475cb40a568e8da8a045ced110137e159f890ac4da883b6b17dc651b3a8049
        ("validDirective.value", "42"),
        // Java ordinary-valid-values/outer-2 stdout SHA256 62b67e1f685b7fef51102005dddd27774be3fee38c42965c53aab035d0b6b221
        ("validDirective.after", ">"),
        // Java ordinary-valid-values/outer-3 stdout SHA256 2703e137a6ccfe7f4fb8bf5a695fd3d15ec8c8ccb9bdb6ae54ad8063b3184287
        ("validMatch.value", "a𐐀"),
        // Java ordinary-valid-values/outer-4 stdout SHA256 6b86b273ff34fce19d6b804eff5a3f5747ada4eaa22f1d49c01e52ddb7875b4b
        ("validMatch.start", "1"),
        // Java ordinary-valid-values/outer-5 stdout SHA256 4b227777d4dd1fc61c6f884f48641d02b4d121d3fd328cb08b5531fcacdabf8a
        ("validMatch.end", "4"),
        // Java ordinary-valid-values/outer-6 stdout SHA256 d4735e3a265e16eee03f59718b9b5d03019c07d8b6c51f90da3a666eec13ab35
        ("validMatch.groups.length", "2"),
        // Java ordinary-valid-values/outer-7 stdout SHA256 b5bea41b6c623f7c09f1bf24dcae58ebab3c0cdd90ad966bc43a45b44867e12b
        ("validMatch.groups[0] == null", "true"),
        // Java ordinary-valid-values/outer-8 stdout SHA256 3e23e8160039594a33894f6564e1b1348bbd7a0088d42c4acb73eeaed59c009d
        ("validMatch.groups[1]!!.toString()", "b"),
        // Java ordinary-valid-values/outer-9 stdout SHA256 2703e137a6ccfe7f4fb8bf5a695fd3d15ec8c8ccb9bdb6ae54ad8063b3184287
        ("validMatch.toString()", "a𐐀"),
        // Java ordinary-valid-values/outer-10 stdout SHA256 b05e244762b1e472be89a93800cc3ee326743cecb55984bf12813addb8de66d0
        ("yaml11.mode", "1.1"),
        // Java ordinary-valid-values/outer-11 stdout SHA256 4e07408562bedb8b60ce05c1decfe3ad16b72230967de01f640b7e4729b49fce
        ("yaml11.indentWidth", "3"),
        // Java ordinary-valid-values/outer-12 stdout SHA256 fcbcf165908dd18a9e49f7ff27810176db8e9f63b4352213741664245224f8aa
        ("yaml11.omitNullProperties", "false"),
        // Java ordinary-valid-values/outer-13 stdout SHA256 b5bea41b6c623f7c09f1bf24dcae58ebab3c0cdd90ad966bc43a45b44867e12b
        ("yaml11.isStream", "true"),
        // Java ordinary-valid-values/outer-14 stdout SHA256 2b4c342f5433ebe591a1da77e013d1b72475562d48578dca8b84bac6651c3cb9
        ("plistTab.indent", "\t"),
        // Java ordinary-valid-values/outer-15 stdout SHA256 fcbcf165908dd18a9e49f7ff27810176db8e9f63b4352213741664245224f8aa
        ("propertiesAscii.omitNullProperties", "false"),
        // Java ordinary-valid-values/outer-16 stdout SHA256 b5bea41b6c623f7c09f1bf24dcae58ebab3c0cdd90ad966bc43a45b44867e12b
        ("propertiesAscii.restrictCharset", "true"),
    ];
    let mut probe = metadata_probe(
        &cases
            .iter()
            .map(|(expression, _)| *expression)
            .collect::<Vec<_>>(),
    );
    for (index, (_, expected)) in cases.into_iter().enumerate() {
        value(&mut probe, index as u32, expected);
    }
    probe.close();
    probe.collect();
    assert_eq!(probe.heap_slots(), 0);
}

#[test]
fn ordinary_class_type_failures_preserve_complete_java_diagnostics() {
    let cases = [
        // Java ordinary-type-constraints/outer-0 stderr SHA256 88921b03d4ca3aa44ee515a896ec9f6a8d8130c5fb2e7762fd526d72f9a01c28
        (
            "badMode.mode",
            "–– Pkl Error ––\nExpected value of type `\"compat\"|\"1.1\"|\"1.2\"`, but got `\"1.3\"`.\n\n581 | mode: \"compat\" | \"1.1\" | \"1.2\" = \"compat\"\n            ^^^^^^^^^^^^^^^^^^^^^^^^\nat pkl.base#YamlRenderer.mode (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L581)\n\n13 | badMode = new YamlRenderer { mode = \"1.3\" }\n                                         ^^^^^\nat stage56.metadataCompletion#badMode.mode (file:///stage56-class-metadata-completion/main.pkl)\n\n1 | badMode.mode\n    ^^^^^^^^^^^^\nat  (repl:text)\n",
        ),
        // Java ordinary-type-constraints/outer-2 stderr SHA256 f6dd9f0af4abdeed08a13852cc29eb5ab299c89d6c8ce229d41b10a1cabc60d4
        (
            "badIndentOne.indentWidth",
            "–– Pkl Error ––\nType constraint `this > 1` violated.\nValue: 1\n\n584 | indentWidth: Int(this > 1) = 2\n                       ^^^^^^^^\nat pkl.base#YamlRenderer.indentWidth (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L584)\n\n14 | badIndentOne = new YamlRenderer { indentWidth = 1 }\n                                                     ^\nat stage56.metadataCompletion#badIndentOne.indentWidth (file:///stage56-class-metadata-completion/main.pkl)\n\n1 | badIndentOne.indentWidth\n    ^^^^^^^^^^^^^^^^^^^^^^^^\nat  (repl:text)\n",
        ),
        // Java ordinary-type-constraints/outer-3 stderr SHA256 84d54627b0c22fe00104265e3c0d8804a1f5b8ec0ca087b1247ae5a2216cfff6
        (
            "badIndentZero.indentWidth",
            "–– Pkl Error ––\nType constraint `this > 1` violated.\nValue: 0\n\n584 | indentWidth: Int(this > 1) = 2\n                       ^^^^^^^^\nat pkl.base#YamlRenderer.indentWidth (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L584)\n\n15 | badIndentZero = new YamlRenderer { indentWidth = 0 }\n                                                      ^\nat stage56.metadataCompletion#badIndentZero.indentWidth (file:///stage56-class-metadata-completion/main.pkl)\n\n1 | badIndentZero.indentWidth\n    ^^^^^^^^^^^^^^^^^^^^^^^^^\nat  (repl:text)\n",
        ),
        // Java ordinary-type-constraints/outer-4 stderr SHA256 4f96dc94053e2b8830440c7ebc3e909d483311dbb83f16cf55e5931a2776b2c0
        (
            "badIndentType.indentWidth",
            "–– Pkl Error ––\nExpected value of type `Int`, but got type `String`.\nValue: \"two\"\n\n584 | indentWidth: Int(this > 1) = 2\n                   ^^^\nat pkl.base#YamlRenderer.indentWidth (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L584)\n\n16 | badIndentType = new YamlRenderer { indentWidth = \"two\" }\n                                                      ^^^^^\nat stage56.metadataCompletion#badIndentType.indentWidth (file:///stage56-class-metadata-completion/main.pkl)\n\n1 | badIndentType.indentWidth\n    ^^^^^^^^^^^^^^^^^^^^^^^^^\nat  (repl:text)\n",
        ),
        // Java ordinary-type-constraints/outer-5 stderr SHA256 0f07f886bb60fb76c512c6c89eb7534e1985dbb6887bd618dfbc00e50fead53e
        (
            "badGroups.groups",
            "–– Pkl Error ––\nExpected value of type `RegexMatch`, but got type `Int`.\nValue: 1\n\n1751 | groups: List<RegexMatch?>\n                    ^^^^^^^^^^\nat pkl.base#RegexMatch.groups (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L1751)\n\n17 | badGroups = new RegexMatch { groups = List(1) }\n                                           ^^^^^^^\nat stage56.metadataCompletion#badGroups.groups (file:///stage56-class-metadata-completion/main.pkl)\n\n1 | badGroups.groups\n    ^^^^^^^^^^^^^^^^\nat  (repl:text)\n",
        ),
        // Java ordinary-type-constraints/outer-7 stderr SHA256 61633bbbec08b315426176ffcc97d66cd0f7139f57521d95080487994524d821
        (
            "badMatchValue.toString()",
            "–– Pkl Error ––\nExpected value of type `String`, but got type `Int`.\nValue: 1\n\n1736 | value: String\n              ^^^^^^\nat pkl.base#RegexMatch.value (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L1736)\n\n18 | badMatchValue = new RegexMatch { value = 1 }\n                                              ^\nat stage56.metadataCompletion#badMatchValue.value (file:///stage56-class-metadata-completion/main.pkl)\n\n1754 | function toString(): String = value\n                                     ^^^^^\nat pkl.base#RegexMatch.toString (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L1754)\n\n1 | badMatchValue.toString()\n    ^^^^^^^^^^^^^^^^^^^^^^^^\nat  (repl:text)\n",
        ),
        // Java ordinary-type-constraints/outer-8 stderr SHA256 0761102acaf5832ebc7e68c8d27dbc024b262aa8db4f0e19b18f165e30790afb
        (
            "badDirectiveBefore.before",
            "–– Pkl Error ––\nExpected value of type `String`, but got type `Int`.\nValue: 1\n\n519 | before: String?\n              ^^^^^^\nat pkl.base#PcfRenderDirective.before (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L519)\n\n19 | badDirectiveBefore = new PcfRenderDirective { before = 1 }\n                                                            ^\nat stage56.metadataCompletion#badDirectiveBefore.before (file:///stage56-class-metadata-completion/main.pkl)\n\n1 | badDirectiveBefore.before\n    ^^^^^^^^^^^^^^^^^^^^^^^^^\nat  (repl:text)\n",
        ),
    ];
    let mut probe = metadata_probe(
        &cases
            .iter()
            .map(|(expression, _)| *expression)
            .collect::<Vec<_>>(),
    );
    for (index, (_, expected)) in cases.into_iter().enumerate() {
        let event = evaluate(&mut probe, index as u32);
        assert_eq!(event.kind, "error", "{event:?}");
        assert_eq!(
            event.error.unwrap(),
            expected.encode_utf16().collect::<Vec<_>>(),
            "query {index}"
        );
    }
    probe.close();
    probe.collect();
    assert_eq!(probe.heap_slots(), 0);
}

#[test]
fn ordinary_lazy_defaults_survive_undefined_member_failures() {
    let cases = [
        // Java outer-0 stdout SHA256 74234e98afe7498fb5daf1f36ac2d78acc339464f950703b8c019892f982b90b
        ("directive.before", false, "null"),
        // Java outer-1 stdout SHA256 74234e98afe7498fb5daf1f36ac2d78acc339464f950703b8c019892f982b90b
        ("directive.after", false, "null"),
        // Java outer-2 stderr SHA256 426831ba94fa6d39ba44ed365852192b98b3fa0c3c9c145d10b3acfe90ae305d
        (
            "directive.value",
            true,
            "–– Pkl Error ––\nTried to read property `value` but its value is undefined.\n\n522 | value: Any\n      ^^^^^\nat pkl.base#PcfRenderDirective.value (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L522)\n\n1 | directive.value\n    ^^^^^^^^^^^^^^^\nat  (repl:text)\n",
        ),
        // Java outer-3 stdout SHA256 74234e98afe7498fb5daf1f36ac2d78acc339464f950703b8c019892f982b90b
        ("directive.before", false, "null"),
        // Java outer-4 stdout SHA256 5feceb66ffc86f38d952786c6d696c79c2dbc239dd4e91b46729d73a27fb57e9
        ("match.groups.length", false, "0"),
        // Java outer-5 stderr SHA256 a01fc3abc907f3546375b147d73ae414e0b185a5cf28c7aeb50821bf4e347cfb
        (
            "match.value",
            true,
            "–– Pkl Error ––\nTried to read property `value` but its value is undefined.\n\n1736 | value: String\n       ^^^^^\nat pkl.base#RegexMatch.value (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L1736)\n\n1 | match.value\n    ^^^^^^^^^^^\nat  (repl:text)\n",
        ),
        // Java outer-6 stderr SHA256 79a409d9a8b2af4afb298c0f610d7a95c0ac86b2a2df4d9312f48c61d53e46d8
        (
            "match.start",
            true,
            "–– Pkl Error ––\nTried to read property `start` but its value is undefined.\n\n1739 | start: Int\n       ^^^^^\nat pkl.base#RegexMatch.start (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L1739)\n\n1 | match.start\n    ^^^^^^^^^^^\nat  (repl:text)\n",
        ),
        // Java outer-7 stderr SHA256 a909f673b7d373a8e2482f327753354d7347938e01061b42f514d07560056b4d
        (
            "match.end",
            true,
            "–– Pkl Error ––\nTried to read property `end` but its value is undefined.\n\n1742 | end: Int\n       ^^^\nat pkl.base#RegexMatch.end (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L1742)\n\n1 | match.end\n    ^^^^^^^^^\nat  (repl:text)\n",
        ),
        // Java outer-8 stderr SHA256 f349f630846af76ad9f1defac33d29c181fc66043bb1a96f459ace00f155f523
        (
            "match.toString()",
            true,
            "–– Pkl Error ––\nTried to read property `value` but its value is undefined.\n\n1736 | value: String\n       ^^^^^\nat pkl.base#RegexMatch.value (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L1736)\n\n1754 | function toString(): String = value\n                                     ^^^^^\nat pkl.base#RegexMatch.toString (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L1754)\n\n1 | match.toString()\n    ^^^^^^^^^^^^^^^^\nat  (repl:text)\n",
        ),
        // Java outer-9 stdout SHA256 5feceb66ffc86f38d952786c6d696c79c2dbc239dd4e91b46729d73a27fb57e9
        ("match.groups.length", false, "0"),
        // Java outer-10 stdout SHA256 b5bea41b6c623f7c09f1bf24dcae58ebab3c0cdd90ad966bc43a45b44867e12b
        ("PcfRenderDirective != RenderDirective", false, "true"),
        // Java outer-11 stdout SHA256 fcbcf165908dd18a9e49f7ff27810176db8e9f63b4352213741664245224f8aa
        ("directive is RenderDirective", false, "false"),
    ];
    let mut probe = metadata_probe(
        &cases
            .iter()
            .map(|(expression, _, _)| *expression)
            .collect::<Vec<_>>(),
    );
    for (index, (_, error, expected)) in cases.into_iter().enumerate() {
        let event = evaluate(&mut probe, index as u32);
        assert_eq!(event.kind, if error { "error" } else { "value" });
        let actual = if error { event.error } else { event.value }.unwrap();
        assert_eq!(
            actual,
            expected.encode_utf16().collect::<Vec<_>>(),
            "query {index}"
        );
    }
    probe.close();
    probe.collect();
    assert_eq!(probe.heap_slots(), 0);
}

#[test]
fn unimplemented_renderer_algorithms_remain_explicit_at_invocation() {
    // Implemented Properties/YAML bytes are covered by their Java oracle tests.
    let mut probe = metadata_probe(&[
        "yaml.extension",
        "plist.renderDocument(1)",
        "plist.renderValue(1)",
        "yaml.extension",
    ]);
    value(&mut probe, 0, "yaml");
    for index in 1..3 {
        let event = evaluate(&mut probe, index);
        assert_eq!(event.kind, "error");
        assert_eq!(event.disposition, Some("unsupported"));
    }
    value(&mut probe, 3, "yaml");
}
