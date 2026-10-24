"""Integrity and boundary regression checks for the Stage 62 Java fixture."""
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
import collection_conversions_fixtures as fixture


class CollectionConversionsFixtureTests(unittest.TestCase):
    def test_offline_sealed_capture(self):
        with patch("subprocess.Popen", side_effect=AssertionError("offline verifier launched Java")):
            result = fixture.verify()
        self.assertEqual(result["rows"], 236)
        self.assertEqual(result["statuses"], {"success": 158, "error": 78})
        self.assertEqual(result["candidate_compatibility_passes_claimed"], 0)

    def test_signed_for_and_unsigned_spread_and_lazy_order(self):
        rows = json.loads((fixture.DEFAULT / "cases.json").read_bytes())["rows"]
        indices = {row["id"]: i for i, row in enumerate(rows)}
        with tarfile.open(fixture.DEFAULT / "oracle-evidence.tar.gz", "r:gz") as archive:
            def read(id, channel):
                folder = f"{fixture.ARCHIVE_PREFIX}/repeat-0/{indices[id]:03d}-{id}/"
                return archive.extractfile(folder + channel + ".bin").read()
            self.assertEqual(read("bytes-for-high", "result.stdout"), b"new Listing { -128; -1 }")
            self.assertEqual(read("bytes-spread-high", "result.stdout"), b"new Listing { 128; 255 }")
            self.assertEqual(read("dynamic-child-before-parent-error", "result.status"), b"error\n")
            self.assertIn(b"child-bang", read("dynamic-child-before-parent-error", "result.stderr"))
            self.assertEqual(read("typed-map-nested-lazy", "result.stdout"), b"1")
            self.assertIn(b"typed-nested", read("typed-map-nested-forced", "result.stderr"))

    def test_duplicate_id_and_query_reorder_rejected(self):
        rows = json.loads((fixture.DEFAULT / "cases.json").read_bytes())["rows"]
        queries = (fixture.DEFAULT / "queries.tsv").read_bytes()
        duplicate = copy.deepcopy(rows)
        duplicate[1]["id"] = duplicate[0]["id"]
        with self.assertRaisesRegex(ValueError, "case rows differ"):
            fixture.validate_rows(duplicate, queries)
        with self.assertRaisesRegex(ValueError, "query order differs"):
            fixture.validate_rows(rows, b"string\tfake\tList().toMap()\n")

    def test_missing_exception_metadata_rejected(self):
        folder = f"{fixture.ARCHIVE_PREFIX}/repeat-0/000-typed-empty-map/"
        files = {folder + name: b"" for name in fixture.ROW_CHANNELS}
        fixture.validate_row_channels(files, folder)
        files.pop(folder + "result.cause-class.code-units.bin")
        with self.assertRaisesRegex(ValueError, "row channel set differs"):
            fixture.validate_row_channels(files, folder)

    def test_raw_java_identity_must_match_recorded_properties(self):
        with tarfile.open(fixture.DEFAULT / "oracle-evidence.tar.gz", "r:gz") as archive:
            files = {member.name: archive.extractfile(member).read() for member in archive}
        name = f"{fixture.ARCHIVE_PREFIX}/java-identity.stderr.bin"
        files[name] = files[name].replace(b"21.0.12.1", b"21.0.12.2", 1)
        with self.assertRaisesRegex(ValueError, "raw Java identity differs"):
            fixture.validate_capture(files, fixture.ARCHIVE_PREFIX, mode="string",
                                     rows_count=fixture.ROWS, expected_statuses=fixture.STATUSES,
                                     expected_raw_files=fixture.RAW_FILES)

    def test_undeclared_nested_path_and_empty_directory_rejected(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "fixture"
            shutil.copytree(fixture.DEFAULT, root)
            (root / "extra").mkdir()
            with self.assertRaisesRegex(ValueError, "undeclared or nonregular"):
                fixture.verify(root)
            (root / "extra").rmdir()
            (root / "extra").mkdir()
            (root / "extra" / "manifest.json").write_text("{}")
            with self.assertRaisesRegex(ValueError, "undeclared or nonregular"):
                fixture.verify(root)

    def test_root_input_mutation_rejected_even_with_new_manifest(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "fixture"
            shutil.copytree(fixture.DEFAULT, root)
            (root / "queries.tsv").write_bytes((root / "queries.tsv").read_bytes() + b"string\textra\t1\n")
            files = {p.name: fixture.sha(p.read_bytes()) for p in root.iterdir() if p.name != "manifest.json"}
            (root / "manifest.json").write_text(
                json.dumps({"schema": 1, "baseline": "Pkl 0.32.0", "files": files}))
            with self.assertRaisesRegex(ValueError, "authored input differs"):
                fixture.verify(root)


if __name__ == "__main__":
    unittest.main()
