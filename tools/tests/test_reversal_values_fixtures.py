"""Stage66 pinned Java reversal fixture integrity and byte-boundary tests."""
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
import reversal_values_fixtures as fixture
from collection_conversions_fixtures import ROW_CHANNELS, validate_capture
from reversal_values_oracle import SOURCES


class ReversalValuesFixtureTests(unittest.TestCase):
    def test_offline_sealed_capture(self):
        with patch("subprocess.Popen", side_effect=AssertionError("offline verifier started Java")):
            result = fixture.verify()
        self.assertEqual(result["rows"], 78)
        self.assertEqual(result["statuses"], {"success": 45, "error": 33})
        self.assertEqual(result["candidate_compatibility_passes_claimed"], 0)

    def test_raw_utf16_surrogate_and_order_controls(self):
        rows = json.loads((fixture.DEFAULT / "cases.json").read_bytes())["rows"]
        indices = {row["id"]: index for index, row in enumerate(rows)}
        with tarfile.open(fixture.DEFAULT / "oracle-evidence.tar.gz", "r:gz") as archive:
            def read(name, channel):
                prefix = f"{fixture.ARCHIVE_PREFIX}/repeat-0/{indices[name]:03d}-{name}/"
                return archive.extractfile(prefix + channel + ".bin").read()
            self.assertEqual(read("str-high", "result.stdout"), b"b?a")
            self.assertEqual(read("str-low", "result.stdout"), b"b?a")
            self.assertEqual(read("str-high", "result.stdout.code-units"), bytes.fromhex("0062d8000061"))
            self.assertEqual(read("str-low", "result.stdout.code-units"), bytes.fromhex("0062dc000061"))
            self.assertEqual(read("str-low-high", "result.stdout.code-units"), bytes.fromhex("d800dc00"))
            self.assertEqual(read("str-double-low-high", "result.stdout.code-units"), bytes.fromhex("d800dc00"))
            self.assertEqual(read("set-duplicates", "result.stdout"), b"List(2, 1, 3)")
            self.assertEqual(read("set-duration-first", "result.stdout"), b"min")
            self.assertEqual(read("set-duration-second", "result.stdout"), b"s")
            self.assertEqual(read("list-lazy-length", "result.stdout"), b"2")
            self.assertEqual(read("set-fn-length", "result.stdout"), b"1")
            self.assertEqual(read("set-construction-child-length", "result.status"), b"error\n")
            self.assertIn(b"set-child", read("set-construction-child-length", "result.stderr"))

    def test_duplicate_id_and_query_reorder_rejected(self):
        rows = json.loads((fixture.DEFAULT / "cases.json").read_bytes())["rows"]
        queries = (fixture.DEFAULT / "queries.tsv").read_bytes()
        duplicate = copy.deepcopy(rows)
        duplicate[1]["id"] = duplicate[0]["id"]
        with self.assertRaisesRegex(ValueError, "case rows differ"):
            fixture.validate_rows(duplicate, queries, count=fixture.ROWS)
        with self.assertRaisesRegex(ValueError, "query order differs"):
            fixture.validate_rows(rows, b"string\tfake\tList(1).reverse()\n", count=fixture.ROWS)

    def test_missing_utf16_channel_rejected(self):
        folder = f"{fixture.ARCHIVE_PREFIX}/repeat-0/000-str-empty/"
        files = {folder + name: b"" for name in ROW_CHANNELS}
        fixture.validate_row_channels(files, folder)
        files.pop(folder + "result.stdout.code-units.bin")
        with self.assertRaisesRegex(ValueError, "row channel set differs"):
            fixture.validate_row_channels(files, folder)

    def test_raw_jdk_identity_mutation_rejected(self):
        with tarfile.open(fixture.DEFAULT / "oracle-evidence.tar.gz", "r:gz") as archive:
            files = {member.name: archive.extractfile(member).read() for member in archive}
        name = f"{fixture.ARCHIVE_PREFIX}/java-identity.stderr.bin"
        files[name] = files[name].replace(b"21.0.12.1", b"21.0.12.2", 1)
        with self.assertRaisesRegex(ValueError, "raw Java identity differs"):
            validate_capture(files, fixture.ARCHIVE_PREFIX, mode="string", rows_count=fixture.ROWS,
                             expected_statuses=fixture.STATUSES, expected_raw_files=fixture.RAW_FILES,
                             module_uri=fixture.MODULE_URI, source_names=SOURCES)

    def test_extra_root_and_mutated_authored_input_rejected(self):
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
