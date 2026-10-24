"""Stage66 reversed List subscript diagnostic Java fixture tests."""
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
import reversal_subscript_fixtures as fixture
from collection_conversions_fixtures import ROW_CHANNELS, validate_capture
from reversal_subscript_oracle import SOURCES


class ReversalSubscriptFixtureTests(unittest.TestCase):
    def test_offline_capture(self):
        with patch("subprocess.Popen", side_effect=AssertionError("offline verifier started Java")):
            result = fixture.verify()
        self.assertEqual(result["rows"], 19)
        self.assertEqual(result["statuses"], {"success": 1, "error": 18})
        self.assertEqual(result["candidate_compatibility_passes_claimed"], 0)

    def test_collection_render_and_error_precedence(self):
        rows = json.loads((fixture.DEFAULT / "cases.json").read_bytes())["rows"]
        indices = {row["id"]: i for i, row in enumerate(rows)}
        with tarfile.open(fixture.DEFAULT / "oracle-evidence.tar.gz", "r:gz") as archive:
            def read(name, channel):
                folder = f"{fixture.ARCHIVE_PREFIX}/repeat-0/{indices[name]:03d}-{name}/"
                return archive.extractfile(folder + channel + ".bin").read()
            self.assertIn(b"Collection: List()", read("empty-zero", "result.stderr"))
            self.assertIn(b"`1,000`", read("one-large", "result.stderr"))
            lazy = read("lazy-out", "result.stderr")
            self.assertIn(b"Collection: List(new Dynamic { value = ? })", lazy)
            self.assertNotIn(b"nested\n", lazy)
            self.assertIn(b"nested", read("lazy-in", "result.stderr"))
            self.assertNotIn(b"Collection:", read("listing-upper", "result.stderr"))
            self.assertIn(b"Value: Bytes(1)", read("bytes-upper", "result.stderr"))
            self.assertIn(b"key", read("key-throw", "result.stderr"))

    def test_raw_surrogate_error_units_remain_distinct(self):
        rows = json.loads((fixture.DEFAULT / "cases.json").read_bytes())["rows"]
        indices = {row["id"]: i for i, row in enumerate(rows)}
        with tarfile.open(fixture.DEFAULT / "oracle-evidence.tar.gz", "r:gz") as archive:
            def read(name, channel):
                folder = f"{fixture.ARCHIVE_PREFIX}/repeat-0/{indices[name]:03d}-{name}/"
                return archive.extractfile(folder + channel + ".bin").read()
            high = read("high-surrogate-out", "result.stderr.code-units")
            low = read("low-surrogate-out", "result.stderr.code-units")
            replacement = read("replacement-out", "result.stderr.code-units")
            self.assertNotEqual(high, low)
            self.assertNotEqual(high, replacement)
            self.assertNotEqual(low, replacement)
            self.assertEqual(read("high-surrogate-in", "result.stdout"), b"?")
            self.assertEqual(read("high-surrogate-in", "result.stdout.code-units"), bytes.fromhex("d800"))

    def test_duplicate_or_reordered_query_rejected(self):
        rows = json.loads((fixture.DEFAULT / "cases.json").read_bytes())["rows"]
        queries = (fixture.DEFAULT / "queries.tsv").read_bytes()
        duplicate = copy.deepcopy(rows)
        duplicate[1]["id"] = duplicate[0]["id"]
        with self.assertRaisesRegex(ValueError, "case rows differ"):
            fixture.validate_rows(duplicate, queries, count=fixture.ROWS)
        with self.assertRaisesRegex(ValueError, "query order differs"):
            fixture.validate_rows(rows, b"string\tfake\tList().reverse()[0]\n", count=fixture.ROWS)

    def test_missing_utf16_and_jdk_mutation_rejected(self):
        folder = f"{fixture.ARCHIVE_PREFIX}/repeat-0/000-empty-zero/"
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

    def test_extra_root_or_mutated_input_rejected(self):
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
