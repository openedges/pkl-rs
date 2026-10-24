"""Offline integrity and code-unit checks for Java Dynamic rendering authority."""
import json
from pathlib import Path
import shutil
import sys
import tarfile
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import collection_dynamic_render_fixtures as fixture


class CollectionDynamicRenderFixtureTests(unittest.TestCase):
    def rows(self):
        return json.loads((fixture.DEFAULT / "cases.json").read_bytes())["rows"]

    def read(self, archive, row_id, suffix="result.stdout.code-units.bin"):
        index = {row["id"]: i for i, row in enumerate(self.rows())}[row_id]
        name = f"{fixture.ARCHIVE_PREFIX}/repeat-0/{index:03d}-{row_id}/{suffix}"
        return archive.extractfile(name).read()

    def test_offline_sealed_authority(self):
        with patch("subprocess.Popen", side_effect=AssertionError("offline verifier launched Java")):
            result = fixture.verify()
        self.assertEqual((result["rows"], result["statuses"]), (10, {"success": 10}))
        self.assertEqual(result["candidate_compatibility_passes_claimed"], 0)

    def test_unpaired_surrogates_keep_distinct_utf16_property_names(self):
        with tarfile.open(fixture.DEFAULT / "oracle-evidence.tar.gz", "r:gz") as archive:
            def value(row_id):
                return self.read(archive, row_id).decode("utf-16-be", "surrogatepass")
            self.assertEqual(value("high-to-string"), "new Dynamic { `\ud800` = 7 }")
            self.assertEqual(value("low-to-string"), "new Dynamic { `\udc00` = 8 }")
            self.assertEqual(value("replacement-to-string"), "new Dynamic { `\ufffd` = 9 }")
            self.assertEqual(value("high-replacement-to-string"),
                             "new Dynamic { `\ud800` = 7; `\ufffd` = 8 }")
            self.assertEqual(value("high-direct-string"), value("high-to-string"))
            self.assertEqual(value("high-to-map"), 'Map("\ud800", 7)')

    def test_nul_identifier_and_utf8_loss_boundary(self):
        with tarfile.open(fixture.DEFAULT / "oracle-evidence.tar.gz", "r:gz") as archive:
            nul = self.read(archive, "nul-to-string").decode("utf-16-be", "surrogatepass")
            internal = self.read(archive, "internal-like-to-string").decode("utf-16-be")
            self.assertEqual(nul, "new Dynamic { `\x00` = 10 }")
            self.assertEqual(internal, "new Dynamic { $internal = 11 }")
            self.assertEqual(self.read(archive, "high-to-string", "result.stdout.bin"),
                             b"new Dynamic { `?` = 7 }")

    def test_undeclared_root_path_rejected(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "fixture"
            shutil.copytree(fixture.DEFAULT, root)
            (root / "extra").mkdir()
            with self.assertRaisesRegex(ValueError, "undeclared or nonregular"):
                fixture.verify(root)

    def test_raw_code_unit_tamper_rejected(self):
        with tarfile.open(fixture.DEFAULT / "oracle-evidence.tar.gz", "r:gz") as archive:
            files = {member.name: archive.extractfile(member).read() for member in archive}
        name = f"{fixture.ARCHIVE_PREFIX}/repeat-0/000-high-to-string/result.stdout.code-units.bin"
        files[name] = files[name].replace(b"\xd8\x00", b"\xff\xfd", 1)
        with self.assertRaisesRegex(ValueError, "UTF-8/UTF-16 producer channels differ"):
            fixture.validate_capture(files, fixture.ARCHIVE_PREFIX, mode="string",
                                     rows_count=fixture.ROWS, expected_statuses=fixture.STATUSES,
                                     expected_raw_files=fixture.RAW_FILES)


if __name__ == "__main__":
    unittest.main()
