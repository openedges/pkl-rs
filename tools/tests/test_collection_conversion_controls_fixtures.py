"""Integrity checks for the separate Stage 62 converted-object Java controls."""
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
import collection_conversion_controls_fixtures as fixture


class CollectionConversionControlsFixtureTests(unittest.TestCase):
    def test_offline_sealed_capture(self):
        with patch("subprocess.Popen", side_effect=AssertionError("offline verifier launched Java")):
            result = fixture.verify()
        self.assertEqual((result["rows"], result["statuses"]),
                         (58, {"success": 44, "error": 14}))
        self.assertEqual(result["candidate_compatibility_passes_claimed"], 0)

    def test_prototype_defaults_and_property_versus_entry(self):
        rows = json.loads((fixture.DEFAULT / "cases.json").read_bytes())["rows"]
        positions = {row["id"]: index for index, row in enumerate(rows)}
        with tarfile.open(fixture.DEFAULT / "oracle-evidence.tar.gz", "r:gz") as archive:
            def value(id, channel="result.stdout"):
                name = (f"{fixture.ARCHIVE_PREFIX}/repeat-0/"
                        f"{positions[id]:03d}-{id}/{channel}.bin")
                return archive.extractfile(name).read()
            self.assertEqual(value("dynamic-dependent-source-name-read"), b"source")
            self.assertEqual(value("dynamic-dependent-source-name"), b"hello-base")
            self.assertEqual(value("map-dependent-source-name"), b"hello-base")
            self.assertEqual(value("map-dynamic-present-property"), b"1")
            self.assertIn(b"Cannot find key", value("map-dynamic-present-subscript", "result.stderr"))

    def test_surrogate_units_are_preserved_and_distinct(self):
        rows = json.loads((fixture.DEFAULT / "cases.json").read_bytes())["rows"]
        positions = {row["id"]: index for index, row in enumerate(rows)}
        with tarfile.open(fixture.DEFAULT / "oracle-evidence.tar.gz", "r:gz") as archive:
            def value(id, channel):
                name = (f"{fixture.ARCHIVE_PREFIX}/repeat-0/"
                        f"{positions[id]:03d}-{id}/{channel}")
                return archive.extractfile(name).read()
            high_units = value("high-key-map", "result.stdout.code-units.bin")
            self.assertIn(b"\xd8\x00", high_units)
            self.assertEqual(value("high-key-map", "result.stdout.bin"), b'Map("?", 7)')
            self.assertEqual(value("high-replacement-distinct-length", "result.stdout.bin"), b"2")
            distinct_units = value("high-replacement-distinct-keys",
                                   "result.stdout.code-units.bin")
            self.assertIn(b"\xd8\x00", distinct_units)
            self.assertIn(b"\xff\xfd", distinct_units)

    def test_bad_row_identity_or_order_rejected(self):
        rows = json.loads((fixture.DEFAULT / "cases.json").read_bytes())["rows"]
        queries = (fixture.DEFAULT / "queries.tsv").read_bytes()
        duplicate = copy.deepcopy(rows)
        duplicate[1]["id"] = duplicate[0]["id"]
        with self.assertRaisesRegex(ValueError, "case rows differ"):
            fixture.validate_rows(duplicate, queries)
        with self.assertRaisesRegex(ValueError, "query order differs"):
            fixture.validate_rows(rows, b"string\tfake\t1\n")

    def test_root_extra_path_rejected(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "fixture"
            shutil.copytree(fixture.DEFAULT, root)
            (root / "extra").mkdir()
            with self.assertRaisesRegex(ValueError, "undeclared or nonregular"):
                fixture.verify(root)

    def test_raw_java_identity_tamper_rejected(self):
        with tarfile.open(fixture.DEFAULT / "oracle-evidence.tar.gz", "r:gz") as archive:
            files = {member.name: archive.extractfile(member).read() for member in archive}
        name = f"{fixture.ARCHIVE_PREFIX}/java-identity.stderr.bin"
        files[name] = files[name].replace(b"21.0.12.1", b"21.0.12.2", 1)
        with self.assertRaisesRegex(ValueError, "raw Java identity differs"):
            fixture.validate_capture(files, fixture.ARCHIVE_PREFIX, mode="string",
                                     rows_count=fixture.ROWS, expected_statuses=fixture.STATUSES,
                                     expected_raw_files=fixture.RAW_FILES)


if __name__ == "__main__":
    unittest.main()
