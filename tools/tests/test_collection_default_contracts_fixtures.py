"""Offline integrity checks for the pinned Java lookup default contract fixture."""
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
import collection_default_contracts_fixtures as fixture


class CollectionDefaultContractsFixtureTests(unittest.TestCase):
    def rows(self):
        return json.loads((fixture.DEFAULT / "cases.json").read_bytes())["rows"]

    def read(self, archive, row_id, channel="result.stdout"):
        index = {row["id"]: i for i, row in enumerate(self.rows())}[row_id]
        name = f"{fixture.ARCHIVE_PREFIX}/repeat-0/{index:03d}-{row_id}/{channel}.bin"
        return archive.extractfile(name).read()

    def test_offline_sealed_authority(self):
        with patch("subprocess.Popen", side_effect=AssertionError("offline verifier launched Java")):
            result = fixture.verify()
        self.assertEqual((result["rows"], result["statuses"]),
                         (22, {"success": 12, "error": 10}))
        self.assertEqual(result["candidate_compatibility_passes_claimed"], 0)

    def test_missing_requires_function_one_and_present_skips_default(self):
        with tarfile.open(fixture.DEFAULT / "oracle-evidence.tar.gz", "r:gz") as archive:
            for kind in ("list", "map"):
                for label, actual in (("nonfunction", b"Int"),
                                      ("arity-zero", b"Function0"),
                                      ("arity-two", b"Function2")):
                    error = self.read(archive, f"{kind}-{label}-missing", "result.stderr")
                    self.assertIn(b"Expected value of type `Function1`", error)
                    self.assertIn(actual, error)
                    self.assertEqual(self.read(archive, f"{kind}-{label}-present"), b"1")

    def test_key_and_value_error_precedence_and_null(self):
        with tarfile.open(fixture.DEFAULT / "oracle-evidence.tar.gz", "r:gz") as archive:
            for kind in ("list", "map"):
                for suffix, marker in (("present-throw", b"present-first"),
                                       ("key-throw", b"key-first")):
                    error = self.read(archive, f"{kind}-nonfunction-{suffix}", "result.stderr")
                    self.assertIn(marker, error)
                    self.assertNotIn(b"Expected value of type `Function1`", error)
                self.assertEqual(self.read(archive, f"{kind}-nonfunction-null-present"), b"null")
                self.assertEqual(self.read(archive, f"{kind}-typed-inherited-default"), b"2")

    def test_duplicate_id_and_order_rejected(self):
        rows = self.rows()
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
