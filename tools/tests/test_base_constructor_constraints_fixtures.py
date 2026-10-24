"""Pinned Java base-constructor constraint authority and integrity tests."""
import copy
import json
from pathlib import Path
import shutil
import sys
import tarfile
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import base_constructor_constraints_fixtures as fixture
from collection_conversions_fixtures import ROW_CHANNELS, validate_capture
from base_constructor_constraints_oracle import SOURCES


class BaseConstructorConstraintsFixtureTests(unittest.TestCase):
    def test_offline_sealed_capture(self):
        with patch("subprocess.Popen", side_effect=AssertionError("offline verifier launched Java")):
            result = fixture.verify()
        self.assertEqual(result["rows"], 36)
        self.assertEqual(result["statuses"], {"success": 20, "error": 16})
        self.assertEqual(result["candidate_compatibility_passes_claimed"], 0)

    def test_all_seven_base_calls_nested_and_first_errors(self):
        rows = json.loads((fixture.DEFAULT / "cases.json").read_bytes())["rows"]
        index = {row["id"]: i for i, row in enumerate(rows)}
        with tarfile.open(fixture.DEFAULT / "oracle-evidence.tar.gz", "r:gz") as archive:
            def read(name, channel):
                folder = f"{fixture.ARCHIVE_PREFIX}/repeat-0/{index[name]:03d}-{name}/"
                return archive.extractfile(folder + channel + ".bin").read()
            for name in ("list", "set", "map", "bytes", "pair", "intseq",
                         "null-object", "null-function", "nested-list", "nested-set", "nested-map"):
                self.assertEqual(read(name, "result.status"), b"success\n")
            self.assertIn(b"multiple of two", read("map-odd-before-throw", "result.stderr"))
            self.assertNotIn(b"\nvalue\n", read("map-odd-before-throw", "result.stderr"))
            self.assertIn(b"value", read("map-even-value-throw", "result.stderr"))
            self.assertIn(b"isBetween(0, 255)", read("bytes-range-before-throw", "result.stderr"))
            self.assertIn(b"first", read("bytes-throw-before-range", "result.stderr"))
            self.assertIn(b"Object | Function<Object>", read("null-invalid-type", "result.stderr"))

    def test_authored_shadow_and_external_query_scopes_differ(self):
        rows = json.loads((fixture.DEFAULT / "cases.json").read_bytes())["rows"]
        index = {row["id"]: i for i, row in enumerate(rows)}
        with tarfile.open(fixture.DEFAULT / "oracle-evidence.tar.gz", "r:gz") as archive:
            def read(name, channel):
                folder = f"{fixture.ARCHIVE_PREFIX}/repeat-0/{index[name]:03d}-{name}/"
                return archive.extractfile(folder + channel + ".bin").read()
            self.assertEqual(read("authored-public-direct", "result.stdout"), b"42")
            self.assertEqual(read("authored-public-constraint", "result.stdout"), b"1")
            self.assertEqual(read("authored-class-local-direct", "result.stdout"), b"43")
            self.assertEqual(read("authored-class-local-constraint", "result.stdout"), b"1")
            self.assertEqual(read("external-list-direct", "result.stdout"), b"List(1)")
            self.assertEqual(read("module-qualified", "result.stdout"), b"42")
            self.assertEqual(read("constraint-qualified", "result.stdout"), b"1")
            self.assertEqual(read("external-class-local-call", "result.status"), b"error\n")
            self.assertIn(b"Cannot find method `List`", read("external-class-local-call", "result.stderr"))

    def test_duplicate_or_reordered_query_rejected(self):
        rows = json.loads((fixture.DEFAULT / "cases.json").read_bytes())["rows"]
        queries = (fixture.DEFAULT / "queries.tsv").read_bytes()
        duplicate = copy.deepcopy(rows)
        duplicate[1]["id"] = duplicate[0]["id"]
        with self.assertRaisesRegex(ValueError, "case rows differ"):
            fixture.validate_rows(duplicate, queries, count=fixture.ROWS)
        with self.assertRaisesRegex(ValueError, "query order differs"):
            fixture.validate_rows(rows, b"string\tfake\tList(1)\n", count=fixture.ROWS)

    def test_missing_utf16_or_raw_jdk_mutation_rejected(self):
        folder = f"{fixture.ARCHIVE_PREFIX}/repeat-0/000-list/"
        files = {folder + name: b"" for name in ROW_CHANNELS}
        fixture.validate_row_channels(files, folder)
        files.pop(folder + "result.stderr.code-units.bin")
        with self.assertRaisesRegex(ValueError, "row channel set differs"):
            fixture.validate_row_channels(files, folder)
        with tarfile.open(fixture.DEFAULT / "oracle-evidence.tar.gz", "r:gz") as archive:
            files = {member.name: archive.extractfile(member).read() for member in archive}
        name = f"{fixture.ARCHIVE_PREFIX}/java-identity.stderr.bin"
        files[name] = files[name].replace(b"21.0.12.1", b"21.0.12.2", 1)
        with self.assertRaisesRegex(ValueError, "raw Java identity differs"):
            validate_capture(files, fixture.ARCHIVE_PREFIX, mode="string", rows_count=fixture.ROWS,
                             expected_statuses=fixture.STATUSES, expected_raw_files=fixture.RAW_FILES,
                             module_uri=fixture.MODULE_URI, source_names=SOURCES)

    def test_undeclared_root_or_mutated_input_rejected(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "fixture"
            shutil.copytree(fixture.DEFAULT, root)
            (root / "extra").mkdir()
            with self.assertRaisesRegex(ValueError, "undeclared or nonregular"):
                fixture.verify(root)
            (root / "extra").rmdir()
            (root / "module.pkl").write_bytes((root / "module.pkl").read_bytes() + b"other = 1\n")
            files = {p.name: fixture.sha(p.read_bytes()) for p in root.iterdir()
                     if p.name != "manifest.json"}
            (root / "manifest.json").write_text(
                json.dumps({"schema": 1, "baseline": "Pkl 0.32.0", "files": files}))
            with self.assertRaisesRegex(ValueError, "authored input differs"):
                fixture.verify(root)


if __name__ == "__main__":
    unittest.main()
