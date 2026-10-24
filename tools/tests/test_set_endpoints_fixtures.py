"""Stage63 Set endpoint Java authority and adversarial integrity checks."""
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
import set_endpoints_fixtures as fixture
from collection_conversions_fixtures import ROW_CHANNELS


class SetEndpointFixtureTests(unittest.TestCase):
    def test_offline_sealed_capture(self):
        with patch("subprocess.Popen", side_effect=AssertionError("offline verifier launched Java")):
            result = fixture.verify()
        self.assertEqual(result["rows"], 70)
        self.assertEqual(result["statuses"], {"success": 47, "error": 23})
        self.assertEqual(result["candidate_compatibility_passes_claimed"], 0)

    def test_order_representative_lazy_and_error_bytes(self):
        rows = json.loads((fixture.DEFAULT / "cases.json").read_bytes())["rows"]
        indices = {row["id"]: i for i, row in enumerate(rows)}
        with tarfile.open(fixture.DEFAULT / "oracle-evidence.tar.gz", "r:gz") as archive:
            def read(id, channel):
                folder = f"{fixture.ARCHIVE_PREFIX}/repeat-0/{indices[id]:03d}-{id}/"
                return archive.extractfile(folder + channel + ".bin").read()
            self.assertEqual(read("set-empty-first", "result.status"), b"error\n")
            self.assertIn(b"Expected a non-empty collection.", read("set-empty-first", "result.stderr"))
            self.assertEqual(read("set-empty-first-or-null", "result.stdout"), b"null")
            self.assertEqual(read("set-many-first", "result.stdout"), b"3")
            self.assertEqual(read("set-many-last", "result.stdout"), b"2")
            self.assertEqual(read("union-list-last-or-null", "result.stdout"), b"3")
            self.assertEqual(read("set-duration-first-unit", "result.stdout"), b"min")
            self.assertEqual(read("set-duration-reverse-first-unit", "result.stdout"), b"s")
            self.assertEqual(read("list-lazy-first-class", "result.stdout"), b"Dynamic")
            self.assertIn(b"nested", read("list-lazy-first-read", "result.stderr"))
            self.assertIn(b"Cannot find method `first` in class `Set`.",
                          read("set-method-argument-throw", "result.stderr"))

    def test_duplicate_id_and_query_reorder_rejected(self):
        rows = json.loads((fixture.DEFAULT / "cases.json").read_bytes())["rows"]
        queries = (fixture.DEFAULT / "queries.tsv").read_bytes()
        duplicate = copy.deepcopy(rows)
        duplicate[1]["id"] = duplicate[0]["id"]
        with self.assertRaisesRegex(ValueError, "case rows differ"):
            fixture.validate_rows(duplicate, queries, count=fixture.ROWS)
        with self.assertRaisesRegex(ValueError, "query order differs"):
            fixture.validate_rows(rows, b"string\tfake\tSet().first\n", count=fixture.ROWS)

    def test_missing_utf16_or_exception_channel_rejected(self):
        folder = f"{fixture.ARCHIVE_PREFIX}/repeat-0/000-set-empty-first/"
        files = {folder + name: b"" for name in ROW_CHANNELS}
        fixture.validate_row_channels(files, folder)
        files.pop(folder + "result.cause-class.code-units.bin")
        with self.assertRaisesRegex(ValueError, "row channel set differs"):
            fixture.validate_row_channels(files, folder)

    def test_raw_jdk_identity_mutation_rejected(self):
        with tarfile.open(fixture.DEFAULT / "oracle-evidence.tar.gz", "r:gz") as archive:
            files = {member.name: archive.extractfile(member).read() for member in archive}
        name = f"{fixture.ARCHIVE_PREFIX}/java-identity.stderr.bin"
        files[name] = files[name].replace(b"21.0.12.1", b"21.0.12.2", 1)
        from collection_conversions_fixtures import validate_capture
        from set_endpoints_oracle import SOURCES
        with self.assertRaisesRegex(ValueError, "raw Java identity differs"):
            validate_capture(files, fixture.ARCHIVE_PREFIX, mode="string", rows_count=fixture.ROWS,
                             expected_statuses=fixture.STATUSES, expected_raw_files=fixture.RAW_FILES,
                             module_uri=fixture.MODULE_URI, source_names=SOURCES)

    def test_undeclared_directory_or_nested_manifest_rejected(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "fixture"
            shutil.copytree(fixture.DEFAULT, root)
            (root / "extra").mkdir()
            with self.assertRaisesRegex(ValueError, "undeclared or nonregular"):
                fixture.verify(root)
            (root / "extra" / "manifest.json").write_text("{}")
            with self.assertRaisesRegex(ValueError, "undeclared or nonregular"):
                fixture.verify(root)

    def test_mutated_input_with_rewritten_manifest_rejected(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "fixture"
            shutil.copytree(fixture.DEFAULT, root)
            (root / "module.pkl").write_bytes((root / "module.pkl").read_bytes() + b"other = 1\n")
            files = {p.name: fixture.sha(p.read_bytes()) for p in root.iterdir()
                     if p.name != "manifest.json"}
            (root / "manifest.json").write_text(
                json.dumps({"schema": 1, "baseline": "Pkl 0.32.0", "files": files}))
            with self.assertRaisesRegex(ValueError, "authored input differs"):
                fixture.verify(root)


if __name__ == "__main__":
    unittest.main()
