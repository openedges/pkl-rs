"""Offline integrity and semantic boundary checks for Stage 62 Java lookups."""
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
import collection_lookups_fixtures as fixture


class CollectionLookupsFixtureTests(unittest.TestCase):
    def test_offline_sealed_authority(self):
        with patch("subprocess.Popen", side_effect=AssertionError("offline verifier launched Java")):
            result = fixture.verify()
        self.assertEqual((result["rows"], result["statuses"]),
                         (133, {"success": 82, "error": 51}))
        self.assertEqual(result["candidate_compatibility_passes_claimed"], 0)

    def test_dynamic_cached_string_entry_is_distinct_from_property_lookup(self):
        rows = json.loads((fixture.DEFAULT / "cases.json").read_bytes())["rows"]
        positions = {row["id"]: i for i, row in enumerate(rows)}
        with tarfile.open(fixture.DEFAULT / "oracle-evidence.tar.gz", "r:gz") as archive:
            def read(id, channel="result.stdout"):
                folder = (f"{fixture.ARCHIVE_PREFIX}/repeat-0/"
                          f"{positions[id]:03d}-{id}/")
                return archive.extractfile(folder + channel + ".bin").read()
            self.assertEqual(read("dyn-cache-module-before"), b"false")
            self.assertEqual(read("dyn-cache-module-read-entry"), b"13")
            self.assertEqual(read("dyn-cache-module-after"), b"true")
            self.assertEqual(read("dyn-cache-module-ornull-after"), b"null")
            self.assertIn(b"Cannot find property `cached`",
                          read("dyn-cache-module-get-after", "result.stderr"))
            self.assertEqual(read("dyn-cache-let-after-read"), b"true")
            self.assertEqual(read("dyn-cache-let-after-map"), b"true")

    def test_hidden_default_and_external_method_are_separate(self):
        rows = json.loads((fixture.DEFAULT / "cases.json").read_bytes())["rows"]
        positions = {row["id"]: i for i, row in enumerate(rows)}
        with tarfile.open(fixture.DEFAULT / "oracle-evidence.tar.gz", "r:gz") as archive:
            def value(id):
                name = (f"{fixture.ARCHIVE_PREFIX}/repeat-0/"
                        f"{positions[id]:03d}-{id}/result.stdout.bin")
                return archive.extractfile(name).read()
            self.assertEqual(value("dyn-has-hidden-default"), b"true")
            self.assertEqual(value("dyn-has-external-method-name"), b"false")
            self.assertEqual(value("dyn-has-hidden-typed"), b"false")
            self.assertEqual(value("dyn-has-local-typed"), b"false")

    def test_default_and_generic_controls(self):
        rows = json.loads((fixture.DEFAULT / "cases.json").read_bytes())["rows"]
        positions = {row["id"]: i for i, row in enumerate(rows)}
        with tarfile.open(fixture.DEFAULT / "oracle-evidence.tar.gz", "r:gz") as archive:
            def read(id, channel="result.stdout"):
                folder = (f"{fixture.ARCHIVE_PREFIX}/repeat-0/"
                          f"{positions[id]:03d}-{id}/")
                return archive.extractfile(folder + channel + ".bin").read()
            self.assertEqual(read("list-default-custom-positive"), b"20")
            self.assertEqual(read("list-default-custom-null"), b"null")
            self.assertEqual(read("map-default-custom-string"), b"fallback-y")
            self.assertEqual(read("map-ornull-typed-wrong-key"), b"null")
            self.assertIn(b"value is undefined",
                          read("map-default-typed-missing", "result.stderr"))

    def test_duplicate_id_and_order_rejected(self):
        rows = json.loads((fixture.DEFAULT / "cases.json").read_bytes())["rows"]
        queries = (fixture.DEFAULT / "queries.tsv").read_bytes()
        duplicate = copy.deepcopy(rows)
        duplicate[1]["id"] = duplicate[0]["id"]
        with self.assertRaisesRegex(ValueError, "case rows differ"):
            fixture.validate_rows(duplicate, queries)
        with self.assertRaisesRegex(ValueError, "query order differs"):
            fixture.validate_rows(rows, b"string\tfake\t1\n")

    def test_undeclared_root_path_rejected(self):
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
