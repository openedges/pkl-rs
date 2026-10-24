"""Integrity and semantic boundary checks for Stage 60 Java Bytes authority."""
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
import bytes_values_fixtures as fixture


class BytesValuesFixtureTests(unittest.TestCase):
    def test_sealed_authority_is_offline(self):
        with patch("subprocess.Popen", side_effect=AssertionError("offline verifier launched Java")):
            result = fixture.verify()
        self.assertEqual(result["rows"], 300)
        self.assertEqual(result["statuses"], {"success": 223, "error": 77})
        self.assertEqual(result["candidate_compatibility_passes_claimed"], 0)

    def test_signed_index_and_unsigned_list_are_separate_raw_observations(self):
        cases = json.loads((fixture.DEFAULT / "cases.json").read_bytes())
        positions = {row["id"]: i for i, row in enumerate(cases["rows"])}
        with tarfile.open(fixture.DEFAULT / "oracle-evidence.tar.gz", "r:gz") as archive:
            def raw(row_id):
                i = positions[row_id]
                name = f"{fixture.ARCHIVE_PREFIX}/repeat-0/{i:03d}-{row_id}/result.stdout.bin"
                return archive.extractfile(name).read()
            self.assertEqual(raw("conversion-signed-subscript"), b"-1")
            self.assertEqual(raw("conversion-signed-get"), b"-1")
            self.assertEqual(raw("conversion-unsigned-list"), b"255")
            self.assertEqual(raw("conversion-unsigned-list-roundtrip"), b"255")

    def test_duplicate_id_and_query_reorder_rejected(self):
        cases = json.loads((fixture.DEFAULT / "cases.json").read_bytes())
        queries = (fixture.DEFAULT / "queries.tsv").read_bytes()
        duplicate = copy.deepcopy(cases["rows"])
        duplicate[1]["id"] = duplicate[0]["id"]
        with self.assertRaisesRegex(ValueError, "case rows differ"):
            fixture.validate_rows(duplicate, queries)
        with self.assertRaisesRegex(ValueError, "query order differs"):
            fixture.validate_rows(cases["rows"], b"fake\tBytes()\n")

    def test_missing_raw_exception_metadata_rejected(self):
        folder = "repeat-0/000-empty-length/"
        files = {folder + name: b"" for name in fixture.ROW_CHANNELS}
        fixture.validate_row_channels(files, folder)
        files.pop(folder + "result.cause-class.code-units.bin")
        with self.assertRaisesRegex(ValueError, "row channel set differs"):
            fixture.validate_row_channels(files, folder)

    def test_resigned_root_query_drift_rejected(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "fixture"
            shutil.copytree(fixture.DEFAULT, root)
            query = root / "queries.tsv"
            query.write_bytes(query.read_bytes() + b"fake\tBytes()\n")
            manifest = json.loads((root / "manifest.json").read_bytes())
            manifest["files"]["queries.tsv"] = fixture.sha(query.read_bytes())
            (root / "manifest.json").write_text(json.dumps(manifest))
            with self.assertRaisesRegex(ValueError, "authored input differs"):
                fixture.verify(root)

    def test_undeclared_nested_manifest_and_empty_directory_rejected(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "fixture"
            shutil.copytree(fixture.DEFAULT, root)
            extra = root / "extra"
            extra.mkdir()
            with self.assertRaisesRegex(ValueError, "undeclared or nonregular fixture path"):
                fixture.verify(root)
            (extra / "manifest.json").write_text("{}")
            with self.assertRaisesRegex(ValueError, "undeclared or nonregular fixture path"):
                fixture.verify(root)

    def test_archive_append_and_manifest_refresh_rejected(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "fixture"
            shutil.copytree(fixture.DEFAULT, root)
            archive = root / "oracle-evidence.tar.gz"
            archive.write_bytes(archive.read_bytes() + b"fake-pass")
            manifest = json.loads((root / "manifest.json").read_bytes())
            manifest["files"]["oracle-evidence.tar.gz"] = fixture.sha(archive.read_bytes())
            (root / "manifest.json").write_text(json.dumps(manifest))
            with self.assertRaisesRegex(ValueError, "archive SHA differs"):
                fixture.verify(root)

    def test_full_process_protocol_detects_missing_call(self):
        with tarfile.open(fixture.DEFAULT / "oracle-evidence.tar.gz", "r:gz") as archive:
            raw = archive.extractfile(fixture.ARCHIVE_PREFIX + "/repeat-0/process.stdout.bin").read()
        lines = raw.decode().splitlines()
        fixture.validate_process_protocol(raw, lines)
        with self.assertRaisesRegex(ValueError, "complete process protocol differs"):
            fixture.validate_process_protocol(raw.replace(b"call-enter=string-0-empty-length\n", b"", 1), lines)


if __name__ == "__main__":
    unittest.main()
