"""Offline integrity and literal-NUL/line-separator authority checks."""
import hashlib
import json
from pathlib import Path
import shutil
import sys
import tarfile
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import collection_property_names_fixtures as fixture


class CollectionPropertyNamesFixtureTests(unittest.TestCase):
    def rows(self):
        return json.loads((fixture.DEFAULT / "cases.json").read_bytes())["rows"]

    def read(self, archive, row_id, suffix="result.stdout.code-units.bin"):
        index = {row["id"]: i for i, row in enumerate(self.rows())}[row_id]
        name = f"{fixture.ARCHIVE_PREFIX}/repeat-0/{index:03d}-{row_id}/{suffix}"
        return archive.extractfile(name).read()

    def value(self, archive, row_id):
        return self.read(archive, row_id).decode("utf-16-be", "surrogatepass")

    def test_offline_sealed_authority(self):
        with patch("subprocess.Popen", side_effect=AssertionError("offline verifier launched Java")):
            result = fixture.verify()
        self.assertEqual((result["rows"], result["statuses"]), (26, {"success": 26}))
        self.assertEqual(result["candidate_compatibility_passes_claimed"], 0)

    def test_first_ten_queries_preserve_literal_nul_native_audit_input(self):
        queries = (fixture.DEFAULT / "queries.tsv").read_bytes()
        first_ten = queries[:1001]
        self.assertEqual(first_ten.count(b"\x00"), 10)
        self.assertEqual(hashlib.sha256(first_ten).hexdigest(),
                         "3267c61c9b9b6b5d7c1f92a5baec2622798a22cbde0be2f0685b646ebfe07c4b")
        self.assertEqual(len(self.rows()), 26)

    def test_authored_nul_and_surrogate_remain_distinct(self):
        with tarfile.open(fixture.DEFAULT / "oracle-evidence.tar.gz", "r:gz") as archive:
            self.assertEqual(self.value(archive, "authored-direct"), "2")
            self.assertEqual(self.value(archive, "authored-has"), "true")
            self.assertEqual(self.value(archive, "authored-get"), "2")
            self.assertEqual(self.value(archive, "authored-get-or-null"), "2")
            self.assertEqual(self.value(archive, "amended-surrogate-get"), "1")
            self.assertEqual(self.value(archive, "amended-authored-get"), "2")
            expected = 'Map("\ud800", 1, "\x00property:[55296]", 2)'
            self.assertEqual(self.value(archive, "amended-to-map"), expected)
            self.assertEqual(self.value(archive, "synthetic-two-keys"), expected)

    def test_lf_cr_and_nonidentifier_string_keys_remain_distinct(self):
        with tarfile.open(fixture.DEFAULT / "oracle-evidence.tar.gz", "r:gz") as archive:
            self.assertEqual(self.value(archive, "lf-high-map"),
                             'Map("\ud800", 1, "\\nproperty:[55296]", 3)')
            self.assertEqual(self.value(archive, "lf-high-length"), "2")
            self.assertEqual(self.value(archive, "lf-high-get"), "1")
            self.assertEqual(self.value(archive, "lf-prefix-get"), "3")
            self.assertEqual(self.value(archive, "lf-high-render"),
                             "new Dynamic { `\ud800` = 1; `\nproperty:[55296]` = 3 }")
            self.assertEqual(self.value(archive, "triple-high-nul-lf"),
                             'Map("\ud800", 1, "\x00property:[55296]", 2, "\\nproperty:[55296]", 3)')
            self.assertEqual(self.value(archive, "low-lf-map"),
                             'Map("\udc00", 4, "\\nproperty:[56320]", 5)')
            self.assertEqual(self.value(archive, "cr-high-map"),
                             'Map("\ud800", 1, "\\rproperty:[55296]", 6)')
            self.assertEqual(self.value(archive, "backtick-key-get"), "7")
            self.assertIn("\u2028property:[55296]", self.value(archive, "line-separator-high-map"))
            self.assertIn("\u2029property:[55296]", self.value(archive, "paragraph-separator-high-map"))

    def test_utf8_cannot_substitute_for_code_unit_comparison(self):
        with tarfile.open(fixture.DEFAULT / "oracle-evidence.tar.gz", "r:gz") as archive:
            raw = self.read(archive, "amended-to-map", "result.stdout.bin")
            units = self.read(archive, "amended-to-map")
            self.assertIn(b'"?"', raw)
            self.assertIn(b"\xd8\x00", units)
            self.assertIn(b"\x00\x00", units)

    def test_extra_root_path_and_raw_jdk_tamper_rejected(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "fixture"
            shutil.copytree(fixture.DEFAULT, root)
            (root / "extra").mkdir()
            with self.assertRaisesRegex(ValueError, "undeclared or nonregular"):
                fixture.verify(root)
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
