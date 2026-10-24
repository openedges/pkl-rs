"""Offline integrity and boundary checks for Stage 61 Java IntSeq authority."""
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
import int_seq_values_fixtures as fixture


class IntSeqValuesFixtureTests(unittest.TestCase):
    def test_sealed_authority_is_offline_and_boundaries_are_separate(self):
        with patch("subprocess.Popen", side_effect=AssertionError("offline verifier launched Java")):
            result = fixture.verify()
        self.assertEqual((result["rows"], result["statuses"]),
                         (237, {"success": 198, "error": 39}))
        self.assertEqual((result["export_supplement_rows"], result["export_supplement_statuses"]),
                         (3, {"error": 3}))
        self.assertEqual(result["candidate_compatibility_passes_claimed"], 0)

    def test_extreme_iterator_and_generator_are_distinct_raw_observations(self):
        cases = json.loads((fixture.DEFAULT / "cases.json").read_bytes())
        positions = {row["id"]: i for i, row in enumerate(cases["rows"])}
        with tarfile.open(fixture.DEFAULT / "oracle-evidence.tar.gz", "r:gz") as archive:
            def raw(row_id):
                index = positions[row_id]
                name = (f"{fixture.ARCHIVE_PREFIX}/repeat-0/"
                        f"{index:03d}-{row_id}/result.stdout.bin")
                return archive.extractfile(name).read()
            self.assertEqual(raw("extreme-up-list"),
                             b"List(-9223372036854775808, -1, 9223372036854775806)")
            self.assertEqual(raw("for-values-extreme-up"),
                             b"new Listing { -9223372036854775808 }")
            self.assertEqual(raw("spread-listing-extreme-up"),
                             b"new Listing { -9223372036854775808; -1; 9223372036854775806 }")

    def test_missing_member_is_semantic_error_and_export_is_producer_only(self):
        cases = json.loads((fixture.DEFAULT / "cases.json").read_bytes())
        index = next(i for i, row in enumerate(cases["rows"]) if row["id"] == "missing-member")
        export_cases = json.loads((fixture.DEFAULT / "export-cases.json").read_bytes())
        self.assertEqual(export_cases["boundary"], "typed-export-supplement")
        self.assertEqual(len(export_cases["rows"]), 3)
        with tarfile.open(fixture.DEFAULT / "oracle-evidence.tar.gz", "r:gz") as archive:
            prefix = f"{fixture.ARCHIVE_PREFIX}/repeat-0/{index:03d}-missing-member/"
            stderr = archive.extractfile(prefix + "result.stderr.bin").read()
            self.assertIn(b"Cannot find property `missingField` in object of type `IntSeq`.", stderr)
            export = archive.extractfile(
                f"{fixture.EXPORT_PREFIX}/repeat-0/000-export-asc/result.stderr.bin").read()
            self.assertIn(b"A value of type `IntSeq` cannot be exported.", export)

    def test_duplicate_id_query_reorder_and_mixed_boundary_rejected(self):
        cases = json.loads((fixture.DEFAULT / "cases.json").read_bytes())
        queries = (fixture.DEFAULT / "queries.tsv").read_bytes()
        duplicate = copy.deepcopy(cases["rows"])
        duplicate[1]["id"] = duplicate[0]["id"]
        with self.assertRaisesRegex(ValueError, "case rows differ"):
            fixture.validate_rows(duplicate, queries)
        mixed = copy.deepcopy(cases["rows"])
        mixed[0]["mode"] = "export"
        with self.assertRaisesRegex(ValueError, "case rows differ"):
            fixture.validate_rows(mixed, queries)
        with self.assertRaisesRegex(ValueError, "query order differs"):
            fixture.validate_rows(cases["rows"], b"string\tfake\tIntSeq(1,2)\n")

    def test_missing_raw_exception_metadata_rejected(self):
        folder = "capture-01/repeat-0/000-asc-start/"
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
            query.write_bytes(query.read_bytes() + b"string\tfake\tIntSeq(1,2)\n")
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
            raw = archive.extractfile(
                fixture.ARCHIVE_PREFIX + "/repeat-0/process.stdout.bin").read()
        lines = raw.decode().splitlines()
        fixture.validate_process_protocol(raw, lines)
        with self.assertRaisesRegex(ValueError, "complete process protocol differs"):
            fixture.validate_process_protocol(raw.replace(b"call-enter=string-0-asc-start\n", b"", 1), lines)


if __name__ == "__main__":
    unittest.main()
