"""Boundary and integrity checks for Stage 61 List accessor/fold Java evidence."""
import json
from pathlib import Path
import shutil
import sys
import tarfile
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import int_seq_list_accessor_fold_fixtures as fixture


class ListAccessorFoldFixtureTests(unittest.TestCase):
    def test_sealed_authority_is_offline(self):
        with patch("subprocess.Popen", side_effect=AssertionError("offline verifier launched Java")):
            result = fixture.verify()
        self.assertEqual((result["rows"], result["statuses"]),
                         (65, {"success": 37, "error": 28}))
        self.assertEqual(result["candidate_compatibility_passes_claimed"], 0)

    def test_empty_accessors_and_or_null_are_separate_raw_observations(self):
        cases = json.loads((fixture.DEFAULT / "cases.json").read_bytes())
        positions = {row["id"]: i for i, row in enumerate(cases["rows"])}
        with tarfile.open(fixture.DEFAULT / "oracle-evidence.tar.gz", "r:gz") as archive:
            def raw(row_id, channel):
                index = positions[row_id]
                path = (f"{fixture.ARCHIVE_PREFIX}/repeat-0/"
                        f"{index:03d}-{row_id}/result.{channel}.bin")
                return archive.extractfile(path).read()
            self.assertIn(b"Expected a non-empty collection.", raw("empty-first", "stderr"))
            self.assertIn(b"Expected a non-empty collection.", raw("empty-last", "stderr"))
            self.assertEqual(raw("empty-first-or-null", "stdout"), b"null")
            self.assertEqual(raw("empty-last-or-null", "stdout"), b"null")

    def test_empty_fold_callback_validation_and_value_order(self):
        cases = json.loads((fixture.DEFAULT / "cases.json").read_bytes())
        positions = {row["id"]: i for i, row in enumerate(cases["rows"])}
        with tarfile.open(fixture.DEFAULT / "oracle-evidence.tar.gz", "r:gz") as archive:
            def raw(row_id, channel):
                index = positions[row_id]
                path = (f"{fixture.ARCHIVE_PREFIX}/repeat-0/"
                        f"{index:03d}-{row_id}/result.{channel}.bin")
                return archive.extractfile(path).read()
            self.assertEqual(raw("fold-empty-int", "stdout"), b"17")
            self.assertIn(b"Expected value of type `Function2`", raw("fold-empty-wrong-callback-type", "stderr"))
            self.assertEqual(raw("fold-many-order", "stdout"), b"123")
            self.assertEqual(raw("fold-nested", "stdout"), b"List(1, 2, 3)")
            self.assertEqual(raw("fold-change-acc-type", "stdout"), b"12")

    def test_lazy_property_and_first_error_order(self):
        cases = json.loads((fixture.DEFAULT / "cases.json").read_bytes())
        positions = {row["id"]: i for i, row in enumerate(cases["rows"])}
        with tarfile.open(fixture.DEFAULT / "oracle-evidence.tar.gz", "r:gz") as archive:
            def raw(row_id, channel):
                index = positions[row_id]
                path = (f"{fixture.ARCHIVE_PREFIX}/repeat-0/"
                        f"{index:03d}-{row_id}/result.{channel}.bin")
                return archive.extractfile(path).read()
            self.assertEqual(raw("lazy-first-skipped", "stdout"), b"7")
            self.assertIn(b"lazy-first", raw("lazy-first-forced", "stderr"))
            self.assertEqual(raw("fold-lazy-ignore", "stdout"), b"11")
            self.assertIn(b"first-callback", raw("fold-callback-before-lazy", "stderr"))
            self.assertIn(b"later-value", raw("fold-lazy-after-first", "stderr"))

    def test_archive_append_with_refreshed_manifest_rejected(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "fixture"
            shutil.copytree(fixture.DEFAULT, root)
            archive = root / "oracle-evidence.tar.gz"
            archive.write_bytes(archive.read_bytes() + b"fake-pass")
            manifest = json.loads((root / "manifest.json").read_bytes())
            manifest["files"]["oracle-evidence.tar.gz"] = fixture.common.sha(archive.read_bytes())
            (root / "manifest.json").write_text(json.dumps(manifest))
            with self.assertRaisesRegex(ValueError, "archive SHA differs"):
                fixture.verify(root)

    def test_undeclared_nested_path_rejected(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "fixture"
            shutil.copytree(fixture.DEFAULT, root)
            extra = root / "extra"
            extra.mkdir()
            (extra / "manifest.json").write_text("{}")
            with self.assertRaisesRegex(ValueError, "undeclared or nonregular fixture path"):
                fixture.verify(root)


if __name__ == "__main__":
    unittest.main()
