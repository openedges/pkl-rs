"""Integrity checks for separate Stage 61 math and List.add Java supplements."""
import json
from pathlib import Path
import shutil
import sys
import tarfile
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import int_seq_math_constants_fixtures as math_fixture
import int_seq_list_add_fixtures as list_fixture


class IntSeqPrerequisiteFixtureTests(unittest.TestCase):
    def test_math_capture_is_offline_and_numeric_bits_are_pinned(self):
        with patch("subprocess.Popen", side_effect=AssertionError("offline verifier launched Java")):
            result = math_fixture.verify()
        self.assertEqual((result["rows"], result["public_calls_per_session"], result["statuses"]),
                         (17, 34, {"success": 34}))
        self.assertEqual(result["candidate_compatibility_passes_claimed"], 0)

    def test_math_min_positive_float_and_pi_raw_export_bits(self):
        names = (math_fixture.DEFAULT / "names.txt").read_text().splitlines()
        with tarfile.open(math_fixture.DEFAULT / "oracle-evidence.tar.gz", "r:gz") as archive:
            def raw(name):
                index = names.index(name)
                path = (f"{math_fixture.ARCHIVE_PREFIX}/repeat-0/"
                        f"{index:03d}-{name}/typed.raw-be8.bin")
                return archive.extractfile(path).read()
            self.assertEqual(raw("minPositiveFloat").hex(), "0000000000000001")
            self.assertEqual(raw("pi").hex(), "400921fb54442d18")

    def test_list_add_capture_is_offline_and_preserves_error_order(self):
        with patch("subprocess.Popen", side_effect=AssertionError("offline verifier launched Java")):
            result = list_fixture.verify()
        self.assertEqual((result["rows"], result["statuses"]),
                         (21, {"success": 13, "error": 8}))
        cases = json.loads((list_fixture.DEFAULT / "cases.json").read_bytes())
        positions = {row["id"]: i for i, row in enumerate(cases["rows"])}
        with tarfile.open(list_fixture.DEFAULT / "oracle-evidence.tar.gz", "r:gz") as archive:
            def raw(row_id, channel):
                index = positions[row_id]
                path = (f"{list_fixture.ARCHIVE_PREFIX}/repeat-0/"
                        f"{index:03d}-{row_id}/result.{channel}.bin")
                return archive.extractfile(path).read()
            self.assertEqual(raw("lazy-first-skipped", "stdout"), b"4")
            self.assertIn(b"lazy-first", raw("lazy-first-forced", "stderr"))
            self.assertIn(b"add-first", raw("add-before-fold-error", "stderr"))
            self.assertIn(b"fold-callback", raw("fold-after-add-error", "stderr"))

    def test_math_archive_append_with_refreshed_manifest_rejected(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "fixture"
            shutil.copytree(math_fixture.DEFAULT, root)
            archive = root / "oracle-evidence.tar.gz"
            archive.write_bytes(archive.read_bytes() + b"fake-pass")
            manifest = json.loads((root / "manifest.json").read_bytes())
            manifest["files"]["oracle-evidence.tar.gz"] = math_fixture.sha(archive.read_bytes())
            (root / "manifest.json").write_text(json.dumps(manifest))
            with self.assertRaisesRegex(ValueError, "archive SHA differs"):
                math_fixture.verify(root)

    def test_list_undeclared_nested_manifest_rejected(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "fixture"
            shutil.copytree(list_fixture.DEFAULT, root)
            extra = root / "extra"
            extra.mkdir()
            (extra / "manifest.json").write_text("{}")
            with self.assertRaisesRegex(ValueError, "undeclared or nonregular fixture path"):
                list_fixture.verify(root)


if __name__ == "__main__":
    unittest.main()
