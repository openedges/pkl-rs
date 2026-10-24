"""Stage66 implicit reversal type-constraint Java fixture tests."""
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
import reversal_implicit_fixtures as fixture
from collection_conversions_fixtures import ROW_CHANNELS, validate_capture
from reversal_implicit_oracle import SOURCES


class ReversalImplicitFixtureTests(unittest.TestCase):
    def test_offline_capture(self):
        with patch("subprocess.Popen", side_effect=AssertionError("offline verifier launched Java")):
            result = fixture.verify()
        self.assertEqual(result["rows"], 8)
        self.assertEqual(result["statuses"], {"success": 6, "error": 2})
        self.assertEqual(result["candidate_compatibility_passes_claimed"], 0)

    def test_implicit_and_explicit_receivers_and_failure_frames(self):
        rows = json.loads((fixture.DEFAULT / "cases.json").read_bytes())["rows"]
        indices = {row["id"]: i for i, row in enumerate(rows)}
        with tarfile.open(fixture.DEFAULT / "oracle-evidence.tar.gz", "r:gz") as archive:
            def read(name, channel):
                folder = f"{fixture.ARCHIVE_PREFIX}/repeat-0/{indices[name]:03d}-{name}/"
                return archive.extractfile(folder + channel + ".bin").read()
            for name, value in (("list-implicit", b"List(1, 2)"),
                                ("list-this", b"List(1, 2)"),
                                ("set-implicit", b"Set(1, 2)"),
                                ("set-this", b"Set(1, 2)"),
                                ("string-implicit", b"ab"),
                                ("string-this", b"ab")):
                self.assertEqual(read(name, "result.stdout"), value)
            for name in ("list-implicit-false", "set-implicit-false"):
                self.assertEqual(read(name, "result.status"), b"error\n")
                self.assertIn(b"Type constraint `reverse() == List(1,2)` violated.",
                              read(name, "result.stderr"))

    def test_duplicate_or_reordered_query_rejected(self):
        rows = json.loads((fixture.DEFAULT / "cases.json").read_bytes())["rows"]
        queries = (fixture.DEFAULT / "queries.tsv").read_bytes()
        duplicate = copy.deepcopy(rows)
        duplicate[1]["id"] = duplicate[0]["id"]
        with self.assertRaisesRegex(ValueError, "case rows differ"):
            fixture.validate_rows(duplicate, queries, count=fixture.ROWS)
        with self.assertRaisesRegex(ValueError, "query order differs"):
            fixture.validate_rows(rows, b"string\tfake\tList(1).reverse()\n", count=fixture.ROWS)

    def test_missing_raw_utf16_channel_rejected(self):
        folder = f"{fixture.ARCHIVE_PREFIX}/repeat-0/000-list-implicit/"
        files = {folder + name: b"" for name in ROW_CHANNELS}
        fixture.validate_row_channels(files, folder)
        files.pop(folder + "result.stderr.code-units.bin")
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

    def test_undeclared_root_or_mutated_input_rejected(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "fixture"
            shutil.copytree(fixture.DEFAULT, root)
            (root / "extra").mkdir()
            with self.assertRaisesRegex(ValueError, "undeclared or nonregular"):
                fixture.verify(root)
            (root / "extra").rmdir()
            (root / "queries.tsv").write_bytes((root / "queries.tsv").read_bytes() + b"x")
            files = {p.name: fixture.sha(p.read_bytes()) for p in root.iterdir()
                     if p.name != "manifest.json"}
            (root / "manifest.json").write_text(
                json.dumps({"schema": 1, "baseline": "Pkl 0.32.0", "files": files}))
            with self.assertRaisesRegex(ValueError, "authored input differs"):
                fixture.verify(root)


if __name__ == "__main__":
    unittest.main()
