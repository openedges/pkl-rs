use pkl_runtime::evaluation::frontend::{
    ModuleEvalOptions, ModuleSourceInput, evaluate_module_output,
};

fn output(source: &str) -> Vec<u8> {
    let result = evaluate_module_output(
        ModuleSourceInput {
            text: source,
            uri: "file:///output.pkl",
            inferred_name: "output",
        },
        &ModuleEvalOptions::default(),
    );
    assert_eq!(
        result.exit_code,
        0,
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(result.stderr.is_empty());
    result.stdout
}

#[test]
fn real_module_output_is_lazy_and_bytes_override_bypasses_text_and_renderer() {
    assert_eq!(output("result = 1\n"), b"result = 1\n");
    assert_eq!(
        output("bad = throw(\"unused\")\noutput { text = \"override\" }\n"),
        b"override"
    );
    assert_eq!(
        output(
            "output { bytes = \"BYTES\".encodeToBytes(\"UTF-8\"); text = throw(\"text\"); renderer = throw(\"renderer\"); value = throw(\"value\") }\n"
        ),
        b"BYTES"
    );
}

#[test]
fn converter_precedes_child_forcing_and_preserves_parent_first_render_order() {
    assert_eq!(
        output(
            "output { value = new Dynamic { child = new Dynamic { x = throw(\"unused\") } }; renderer = new PcfRenderer { converters { [\"child\"] = (_) -> \"masked\" } } }"
        ),
        b"child = \"masked\"\n"
    );
    assert_eq!(
        output("local parent = new Dynamic { a = 1 }\noutput { value = (parent) { b = 2 } }"),
        b"a = 1\nb = 2\n"
    );
}

#[test]
fn module_prototype_and_nearest_explicit_property_type_remain_coherent() {
    assert_eq!(
        output("result = new { a = 1; b = \"hello\" }"),
        b"result {\n  a = 1\n  b = \"hello\"\n}\n"
    );
    assert_eq!(
        output(
            "open class Parent { x: Int = 1 }\nclass Child extends Parent { x: Any = \"value\" }\nresult = new Child {}"
        ),
        b"result {\n  x = \"value\"\n}\n"
    );
    let result = evaluate_module_output(
        ModuleSourceInput {
            text: "output: Any = new Dynamic { x = 1 }",
            uri: "file:///output.pkl",
            inferred_name: "output",
        },
        &ModuleEvalOptions::default(),
    );
    assert_eq!(result.exit_code, 1);
    assert_eq!(result.stderr,b"\xe2\x80\x93\xe2\x80\x93 Pkl Error \xe2\x80\x93\xe2\x80\x93\nExpected `output` of module `file:///output.pkl` to be of type `ModuleOutput`, but got type `Dynamic`.\n\n1 | output: Any = new Dynamic { x = 1 }\n                  ^^^^^^^^^^^^^^^^^^^^^\nat output (file:///output.pkl, line 1)\n");
}

#[test]
fn pair_uses_boxed_component_equality_and_resolves_real_hash_collisions() {
    assert_eq!(
        output("scalar = 1 == 1.0\nresult = Pair(1, 2) == Pair(1.0, 2)\n"),
        b"scalar = true\nresult = false\n"
    );
    assert_eq!(
        output("scalar = NaN == NaN\nresult = Pair(NaN, 1) == Pair(NaN, 1)\n"),
        b"scalar = false\nresult = true\n"
    );
    assert_eq!(
        output("scalar = 0.0 == -0.0\nresult = Pair(0.0, 1) == Pair(-0.0, 1)\n"),
        b"scalar = true\nresult = false\n"
    );
    assert_eq!(
        output(
            "local m = Map(Pair(0, 31), \"left\", Pair(1, 0), \"right\")\nresult = new Dynamic { size = m.length; left = m[Pair(0, 31)]; right = m[Pair(1, 0)]; absent = m.containsKey(Pair(2, -31)) }"
        ),
        b"result {\n  size = 2\n  left = \"left\"\n  right = \"right\"\n  absent = false\n}\n"
    );
}

#[test]
fn pair_renderer_bypasses_child_converters_but_accepts_a_whole_value_converter() {
    assert_eq!(
        output(
            "output { value = new Dynamic { pair = Pair(1, 2); direct = 1 }; renderer = new PcfRenderer { converters { [Int] = (v) -> v + 100 } } }"
        ),
        b"pair = Pair(1, 2)\ndirect = 101\n"
    );
    assert_eq!(
        output(
            "output { value = new Dynamic { pair = Pair(new Dynamic { bad = throw(\"unused\") }, 2) }; renderer = new PcfRenderer { converters { [Pair] = (_) -> \"masked\" } } }"
        ),
        b"pair = \"masked\"\n"
    );
}

#[test]
fn property_transformer_uses_a_real_mixin_and_keeps_original_converter_path() {
    assert_eq!(
        output(
            r#"
open class Tag extends ConvertProperty {
 suffix: String = "A"
 render = (p, _) -> Pair(p.key + suffix, p.value + 1)
}
class SubTag extends Tag
class Holder { @SubTag {} x = 7 }
output {
 value = new Holder {}
 renderer = new PcfRenderer {
  convertPropertyTransformers { [Tag] { suffix = "T" } }
  converters { ["x"] = (v) -> v + 10; ["xT"] = (_) -> throw("renamed path") }
 }
}
"#
        ),
        b"xT = 18\n"
    );
}

#[test]
fn primitive_implicit_constraints_matches_pinned_output() {
    assert_eq!(
        output(
            r####"a: Int(isEven, isNonZero) = 6
b: Int(isOdd, isPositive) = 3
c: Number(isBetween(-1, 2)) = 1.5
"####
        ),
        r####"a = 6
b = 3
c = 1.5
"####
            .as_bytes()
    );
}

#[test]
fn primitive_between_values_matches_pinned_output() {
    assert_eq!(
        output(
            r####"result = new Dynamic { lower = (1).isBetween(1.0, 2); upper = (2.0).isBetween(1, 2); outside = (3).isBetween(1, 2); reversed = (1).isBetween(2, 0); nanValue = NaN.isBetween(0, 2); nanLower = (1).isBetween(NaN, 2); nanUpper = (1).isBetween(0, NaN) }
"####
        ),
        r####"result {
  lower = true
  upper = true
  outside = false
  reversed = false
  nanValue = false
  nanLower = false
  nanUpper = false
}
"####
            .as_bytes()
    );
}

#[test]
fn primitive_uppercase_values_matches_pinned_output() {
    assert_eq!(
        output(
            r####"result = new Dynamic { ascii = "abc XYZ".toUpperCase(); sharpS = "straße".toUpperCase(); greek = "σς".toUpperCase(); dotted = "iıİ".toUpperCase(); supplementary = "\u{10428}".toUpperCase(); high = "\u{D800}".toUpperCase(); low = "\u{DC00}".toUpperCase() }
"####
        ),
        r####"result {
  ascii = "ABC XYZ"
  sharpS = "STRASSE"
  greek = "ΣΣ"
  dotted = "IIİ"
  supplementary = "𐐀"
  high = "?"
  low = "?"
}
"####
            .as_bytes()
    );
}

#[test]
fn primitive_contains_values_matches_pinned_output() {
    assert_eq!(
        output(
            r####"result = new Dynamic { found = "abc".contains("b"); absent = "abc".contains("B"); empty = "".contains(""); whole = "\u{1F600}".contains("\u{1F600}"); highHalf = "\u{1F600}".contains("\u{D83D}"); lowHalf = "\u{1F600}".contains("\u{DE00}"); lone = "\u{D800}".contains("\u{D800}"); wrongUnit = "\u{D800}".contains("?") }
"####
        ),
        r####"result {
  found = true
  absent = false
  empty = true
  whole = true
  highHalf = true
  lowHalf = true
  lone = true
  wrongUnit = false
}
"####
            .as_bytes()
    );
}

#[test]
fn external_parent_amendment_preserves_the_parent_expression_error_span() {
    // Pinned Stage24 full Java error also exercised by the standard platforms.
    let actual = pkl_runtime::evaluation::scalar_expression(
        r####"typealias C<T> = Collection<T>
empty:C<Int>
explicit=new C<Int> {}
inferred:C<Int> = new {}
nullable:C<Int>?
amended=(nullable){}
badAmended=(nullable){1}
stable=42
observation0=empty.length
observation1=explicit.length
observation2=inferred.length
observation3=amended.length
observation4=badAmended.length
"####,
        "file:///m2-platform.pkl",
        "m2-platform",
        "observation4",
    )
    .unwrap_err();
    assert_eq!(
        actual,
        r####"–– Pkl Error ––
Cannot instantiate, or amend an instance of, external class `List`.

7 | badAmended=(nullable){1}
                ^^^^^^^^
at m2-platform#badAmended (file:///m2-platform.pkl)

To create an instance of an external class, use its literal syntax or constructor method.
Examples:
* `123` uses literal syntax to create an instance of class `Int`.
* `Pair(1, 2)` uses the `Pair()` constructor method to create an instance of class `Pair`.

13 | observation4=badAmended.length
                  ^^^^^^^^^^
at m2-platform#observation4 (file:///m2-platform.pkl)

1 | observation4
    ^^^^^^^^^^^^
at  (repl:text)
"####
    );
}
