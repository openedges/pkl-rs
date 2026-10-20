"""Regression tests for inventory extraction and fail-closed freshness checks."""

import importlib.util
import json
from pathlib import Path
import subprocess
import tempfile
import unittest

SPEC = importlib.util.spec_from_file_location("inventory", Path(__file__).parents[1] / "inventory.py")
inventory = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(inventory)


class ExtractionTests(unittest.TestCase):
    def test_comments_strings_and_nested_blocks_do_not_declare_members(self):
        text = '''module pkl.demo
// external function fake(): Int
/* one /* external function fake2(): Int */ still comment */
message = #"external function fake3(): Int { }"#
class Visible {
  external function real(): Int
  object = new {
    fake4: Int = 2
  }
}
'''
        symbols = {item["symbol"] for item in inventory.pkl_declarations("stdlib/demo.pkl", text)}
        self.assertEqual(symbols, {"pkl.demo", "pkl.demo#message", "pkl.demo#Visible",
                                   "pkl.demo#Visible.real", "pkl.demo#Visible.object"})

    def test_same_name_property_and_function_keep_separate_identity(self):
        items = inventory.pkl_declarations("stdlib/base.pkl", '''module pkl.base
external class IntSeq {
  external step: Int
  external function step(value: Int): IntSeq
}
''')
        steps = [item for item in items if item["symbol"] == "pkl.base#IntSeq.step"]
        self.assertEqual({item["kind"] for item in steps}, {"property", "function"})
        self.assertEqual(len({item["id"] for item in steps}), 2)

    def test_local_external_is_retained_for_builtin_accounting(self):
        items = inventory.pkl_declarations("stdlib/demo.pkl", '''module pkl.demo
external local function resolvePath(path: String): String
''')
        self.assertTrue(items[1]["external"])
        self.assertEqual(items[1]["visibility"], "local")

    def test_bodyless_class_does_not_claim_following_object_members(self):
        items = inventory.pkl_declarations("stdlib/demo.pkl", '''module pkl.demo
external class Empty
value = new {
  nested: Int = 1
}
''')
        self.assertEqual({item["symbol"] for item in items},
                         {"pkl.demo", "pkl.demo#Empty", "pkl.demo#value"})

    def test_java_nested_visibility_interface_members_and_overloads(self):
        items = inventory.java_declarations("Api.java", '''package example;
public interface Api {
  String value();
  public class Nested {
    public Nested() {}
    public int get(int x) { return x; }
    public int get(String x) { return 1; }
    private void secret() {}
    private class Hidden { public void leak() {} }
  }
}
class HiddenTop { public void leak() {} }
''')
        symbols = [item["symbol"] for item in items]
        self.assertIn("example.Api.value", symbols)
        self.assertIn("example.Api.Nested.Nested", symbols)
        self.assertEqual(symbols.count("example.Api.Nested.get"), 2)
        self.assertFalse(any("leak" in symbol or "secret" in symbol for symbol in symbols))
        self.assertEqual(len(items), len({item["id"] for item in items}))

    def test_annotations_do_not_become_java_methods(self):
        items = inventory.java_declarations("Api.java", '''public class Api {
  @Deprecated(since = "1")
  public String value() { return ""; }
}
''')
        self.assertEqual([item["symbol"] for item in items], ["Api", "Api.value"])

    def test_java_record_components_enum_constants_and_annotation(self):
        items = inventory.java_declarations("Api.java", '''package example;
public interface Api {
  record Pair(String first, java.util.Map<String, Integer> second) {}
  enum Choice { ONE(1), TWO(2); Choice(int n) {} }
  @interface Marker { String value(); }
}
''')
        symbols = {item["symbol"] for item in items}
        self.assertTrue({"example.Api.Pair.first", "example.Api.Pair.second",
                         "example.Api.Choice.ONE", "example.Api.Choice.TWO",
                         "example.Api.Marker.value"} <= symbols)

    def test_kotlin_inline_primary_constructor_properties(self):
        items = inventory.kotlin_declarations("Api.kt", '''package example
data class Api(val name: String, private val secret: Int, val mapping: Map<String, Int>)
''')
        self.assertEqual({item["symbol"] for item in items},
                         {"example.Api", "example.Api.name", "example.Api.mapping"})

    def test_kotlin_private_internal_local_and_receiver(self):
        items = inventory.kotlin_declarations("Api.kt", '''package example
internal class Hidden {
  fun leak() = 1
}
class Api {
  private val secret = 0
  fun visible() {
    val local = 1
  }
}
fun String.extension() = this
''')
        self.assertEqual([item["symbol"] for item in items],
                         ["example.Api", "example.Api.visible", "example.extension"])

    def test_builtin_pklname_and_specialized_subclass_excludes_helper(self):
        path = "pkl-core/src/main/java/org/pkl/core/stdlib/demo/ExampleNodes.java"
        text = '''@PklName("Renamed") public final class ExampleNodes {
  public abstract static class Helper extends ExternalMethod1Node {
    protected int helper() { return 1; }
  }
  @PklName("while")
  public abstract static class whileMethod extends Helper {
    @Specialization protected int eval(int value) { return value; }
  }
}'''
        texts = {str(Path(path).parent / "package-info.java"): '@PklName("Demo") package demo;'}
        items = inventory.builtin_declarations(path, text, texts)
        self.assertEqual(len(items), 1)
        self.assertEqual(items[0]["symbol"], "pkl.Demo#Renamed.while")
        self.assertEqual(items[0]["kind"], "ExternalMethod1Node")

    def test_cli_options_include_negative_alias_exclude_comment_and_help(self):
        items = inventory.specific_declarations("pkl-cli/src/main/kotlin/commands/Demo.kt", '''
// option("--fake")
val power = option(names = arrayOf("-p", "--power"), help = "turn power on")
  .flag("--no-power", default = true)
''')
        self.assertEqual({item["symbol"] for item in items}, {"-p", "--power", "--no-power"})

    def test_command_names_exclude_argument_names_and_keep_tool_commands(self):
        items = inventory.specific_declarations("pkl-codegen-java/src/main/kotlin/Main.kt", '''
class ToolCommand : ModulesCommand(name = "pkl-codegen-java") {
  val paths = argument(name = "paths")
}
''')
        self.assertEqual([(item["category"], item["symbol"]) for item in items],
                         [("cli_command", "pkl-codegen-java"), ("cli_argument", "paths")])

    def test_package_private_parser_implementation_is_enumerated(self):
        items = inventory.specific_declarations("pkl-parser/src/main/java/org/pkl/parser/ParserImpl.java", '''
final class ParserImpl {
  Expr parseExpr() { return null; }
  private Type parseType() { return null; }
}
''')
        self.assertEqual({item["symbol"] for item in items}, {"parseExpr", "parseType"})

    def test_specification_and_codec_keys(self):
        items = inventory.specific_declarations("docs/modules/language-reference/pages/index.adoc",
                                                "= Language\n== Names\n=== Names\n")
        self.assertEqual([item["level"] for item in items], [1, 2, 3])
        items = inventory.specific_declarations("ServerMessagePackDecoder.kt",
                                                'get(map, "requestId"); get(map, "requestId")')
        self.assertEqual([item["symbol"] for item in items], ["requestId"])


class FreshnessTests(unittest.TestCase):
    def make_repository(self, root):
        upstream = root / "external/pkl"
        (upstream / "stdlib").mkdir(parents=True)
        (upstream / "stdlib/demo.pkl").write_text("module pkl.demo\nvalue: Int = 1\n")
        for command in (["init", "-q"], ["add", "."],
                        ["-c", "user.name=Inventory Test", "-c", "user.email=test@example.invalid",
                         "commit", "-qm", "fixture"]):
            subprocess.run(["git", "-C", str(upstream), *command], check=True, capture_output=True)
        revision = subprocess.check_output(["git", "-C", str(upstream), "rev-parse", "HEAD"], text=True).strip()
        (root / "references.lock").write_text(
            f"pkl.path=external/pkl\npkl.url=https://example.invalid/pkl.git\npkl.revision={revision}\npkl.version=0.32.0\n")
        return upstream

    def test_missing_deleted_item_and_source_addition_are_rejected(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            upstream = self.make_repository(root)
            output = root / "inventory.json"
            generated = inventory.generate(root)
            expected = inventory.encoded(generated)
            with self.assertRaisesRegex(ValueError, "missing inventory"):
                inventory.check_bytes(output, expected)
            output.write_bytes(expected)
            inventory.check_bytes(output, expected)
            omitted = json.loads(expected)
            omitted["items"].pop()
            output.write_bytes(inventory.encoded(omitted))
            with self.assertRaisesRegex(ValueError, "stale or incomplete"):
                inventory.check_bytes(output, expected)
            output.write_bytes(expected)
            # An untracked production source must not fall outside the census.
            (upstream / "stdlib/added.pkl").write_text("module pkl.added\nnewMember = 1\n")
            with self.assertRaisesRegex(ValueError, "stale or incomplete"):
                inventory.check_bytes(output, inventory.encoded(inventory.generate(root)))

    def test_source_drift_detected_even_without_declaration_change(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            upstream = self.make_repository(root)
            output = root / "inventory.json"
            before = inventory.generate(root)
            output.write_bytes(inventory.encoded(before))
            (upstream / "stdlib/demo.pkl").write_text("module pkl.demo\nvalue: Int = 2\n")
            after = inventory.generate(root)
            self.assertEqual(before["counts"], after["counts"])
            self.assertNotEqual(before["sources"][0]["sha256"], after["sources"][0]["sha256"])
            with self.assertRaisesRegex(ValueError, "stale or incomplete"):
                inventory.check_bytes(output, inventory.encoded(after))

    def test_wrong_revision_and_empty_census_are_errors(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            upstream = self.make_repository(root)
            pin = root / "references.lock"
            original = pin.read_text()
            revision = subprocess.check_output(["git", "-C", str(upstream), "rev-parse", "HEAD"], text=True).strip()
            pin.write_text(original.replace(revision, "0" * 40))
            with self.assertRaisesRegex(ValueError, "HEAD does not match"):
                inventory.generate(root)
            pin.write_text(original)
            subprocess.run(["git", "-C", str(upstream), "rm", "--cached", "stdlib/demo.pkl"],
                           check=True, capture_output=True)
            (upstream / "stdlib/demo.pkl").unlink()
            with self.assertRaisesRegex(ValueError, "empty upstream"):
                inventory.generate(root)

    def test_no_inventory_item_claims_implementation_or_tests(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            self.make_repository(root)
            for item in inventory.generate(root)["items"]:
                self.assertEqual(item["implementation"], {"state": "unimplemented", "links": []})
                self.assertEqual(item["tests"], {"state": "unmapped", "case_ids": []})


if __name__ == "__main__":
    unittest.main()
